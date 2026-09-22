//! Ownership planning for the copyable scalar/string MIR subset.
//! This deliberately rejects unextracted handlers/closures rather than walking
//! hidden control flow. Resources and other move-only values need a distinct
//! move/borrow/drop analysis before their backend support can be enabled.
use crate::{ControlFlowGraph, Function, StatementKind, TerminatorKind};
use jett_hir::{Expression, ExpressionKind, IntrinsicId, StringSegment};
use jett_types::{CapabilityKind, Type, TypeId, TypeInterner};
use std::collections::BTreeSet;
type Set = BTreeSet<usize>;

#[derive(Debug)]
pub struct CopyValuePlan {
    pub owned_locals: Vec<usize>,
    pub live_in: Vec<Set>,
    pub live_out: Vec<Set>,
    pub live_after_statement: Vec<Vec<Set>>,
    /// Bound on owned intermediates in one full expression, including formatting.
    pub temporary_slots: usize,
}
impl CopyValuePlan {
    pub fn analyze(function: &Function, types: &TypeInterner) -> Result<Self, String> {
        Self::analyze_storage(function, types, None)
    }
    pub(crate) fn analyze_storage(
        function: &Function,
        types: &TypeInterner,
        program: Option<&crate::Program>,
    ) -> Result<Self, String> {
        plan_type(types, function.return_type, program)?;
        for local in &function.locals {
            plan_type(types, local.ty, program)?;
        }
        let cfg = ControlFlowGraph::analyze(function).map_err(|e| format!("{e:?}"))?;
        let n = function.blocks.len();
        let mut facts = Vec::new();
        let mut max_temporaries = 0;
        for block in &function.blocks {
            let mut statements = Vec::new();
            for statement in &block.statements {
                let mut reads = Set::new();
                let mut temporaries = 0;
                let definition = match &statement.kind {
                    StatementKind::IterationBorrow { source, .. } => {
                        reads.insert(source.index() as usize);
                        None
                    }
                    StatementKind::SequenceLength { source, target }
                    | StatementKind::SequenceGet { source, target, .. } => {
                        reads.insert(source.index() as usize);
                        if let StatementKind::SequenceGet { index, .. } = statement.kind {
                            reads.insert(index.index() as usize);
                            temporaries += usize::from(
                                function.locals[target.index() as usize].ty == TypeInterner::STRING,
                            );
                        }
                        Some(target.index() as usize)
                    }
                    StatementKind::SumTag { source, target }
                    | StatementKind::SumTake { source, target, .. }
                        if program.is_some() =>
                    {
                        reads.insert(source.index() as usize);
                        if matches!(statement.kind, StatementKind::SumTake { .. }) {
                            let ty = function.locals[target.index() as usize].ty;
                            temporaries += usize::from(
                                ty == TypeInterner::STRING
                                    || crate::move_values::is_linear(types, ty),
                            );
                        }
                        Some(target.index() as usize)
                    }
                    StatementKind::Let { local, value } => {
                        visit(value, &mut reads, &mut temporaries, types, program, false)?;
                        Some(local.index() as usize)
                    }
                    StatementKind::Assign { target, value } => {
                        let ExpressionKind::Local(local) = target.kind else {
                            return Err("nonlocal assignment needs place ownership".into());
                        };
                        visit(value, &mut reads, &mut temporaries, types, program, false)?;
                        Some(local.index() as usize)
                    }
                    StatementKind::Evaluate(value) => {
                        visit(value, &mut reads, &mut temporaries, types, program, false)?;
                        None
                    }
                    _ => return Err("statement needs explicit ownership lowering".into()),
                };
                max_temporaries = max_temporaries.max(temporaries);
                statements.push((reads, definition));
            }
            let mut reads = Set::new();
            let mut temporaries = 0;
            match &block.terminator.kind {
                TerminatorKind::Return(Some(v)) | TerminatorKind::Branch { condition: v, .. } => {
                    visit(v, &mut reads, &mut temporaries, types, program, false)?
                }
                TerminatorKind::Return(None)
                | TerminatorKind::Goto(_)
                | TerminatorKind::Unreachable => {}
                _ => return Err("terminator needs explicit ownership lowering".into()),
            }
            max_temporaries = max_temporaries.max(temporaries);
            facts.push((statements, reads));
        }
        // Definite initialization is an intersection fixed point, including
        // backedges. Reading a zero-initialized native slot is not a substitute.
        let all: Set = (0..function.locals.len()).collect();
        let params: Set = function
            .params
            .iter()
            .map(|p| p.local.index() as usize)
            .collect();
        let mut initialized_in = vec![all.clone(); n];
        let mut initialized_out = vec![all; n];
        loop {
            let mut changed = false;
            for &id in cfg.reverse_postorder() {
                let i = id.index() as usize;
                let incoming = if id == function.entry {
                    params.clone()
                } else {
                    let mut incoming = (0..function.locals.len()).collect::<Set>();
                    for pred in cfg
                        .predecessors(id)
                        .iter()
                        .filter(|p| cfg.reverse_postorder().contains(p))
                    {
                        incoming = incoming
                            .intersection(&initialized_out[pred.index() as usize])
                            .copied()
                            .collect();
                    }
                    incoming
                };
                let mut outgoing = incoming.clone();
                for (_, definition) in &facts[i].0 {
                    if let Some(d) = definition {
                        outgoing.insert(*d);
                    }
                }
                changed |= incoming != initialized_in[i] || outgoing != initialized_out[i];
                initialized_in[i] = incoming;
                initialized_out[i] = outgoing;
            }
            if !changed {
                break;
            }
        }
        for &id in cfg.reverse_postorder() {
            let i = id.index() as usize;
            let mut initialized = initialized_in[i].clone();
            for (reads, definition) in &facts[i].0 {
                if !reads.is_subset(&initialized) {
                    return Err(format!("read before definite initialization in block {i}"));
                }
                if let Some(d) = definition {
                    initialized.insert(*d);
                }
            }
            if !facts[i].1.is_subset(&initialized) {
                return Err(format!(
                    "terminator read before definite initialization in block {i}"
                ));
            }
        }
        let mut live_in = vec![Set::new(); n];
        let mut live_out = live_in.clone();
        let mut after = facts
            .iter()
            .map(|(ss, _)| vec![Set::new(); ss.len()])
            .collect::<Vec<_>>();
        loop {
            let mut changed = false;
            for &id in cfg.reverse_postorder().iter().rev() {
                let i = id.index() as usize;
                let out: Set = cfg
                    .successors(id)
                    .iter()
                    .flat_map(|s| live_in[s.index() as usize].iter().copied())
                    .collect();
                let mut live = out.union(&facts[i].1).copied().collect::<Set>();
                for (j, (reads, definition)) in facts[i].0.iter().enumerate().rev() {
                    after[i][j] = live.clone();
                    if let Some(d) = definition {
                        live.remove(d);
                    }
                    live.extend(reads);
                }
                changed |= live != live_in[i] || out != live_out[i];
                live_in[i] = live;
                live_out[i] = out;
            }
            if !changed {
                break;
            }
        }
        Ok(Self {
            owned_locals: function
                .locals
                .iter()
                .filter(|l| {
                    matches!(types.resolve(l.ty), Type::String)
                        || (program.is_some()
                            && crate::move_values::is_linear(types, l.ty)
                            && !function
                                .parameter_for_local(l.id)
                                .is_some_and(|p| p.mode == crate::ParamMode::View))
                })
                .map(|l| l.id.index() as usize)
                .collect(),
            live_in,
            live_out,
            live_after_statement: after,
            temporary_slots: max_temporaries,
        })
    }
}
fn visit(
    value: &Expression,
    reads: &mut Set,
    temporaries: &mut usize,
    types: &TypeInterner,
    program: Option<&crate::Program>,
    borrowed: bool,
) -> Result<(), String> {
    plan_type(types, value.ty, program)?;
    if program.is_some() && crate::move_values::is_linear(types, value.ty) {
        *temporaries += usize::from(match &value.kind {
            ExpressionKind::Local(_) => !borrowed,
            ExpressionKind::Call { .. }
            | ExpressionKind::Intrinsic { .. }
            | ExpressionKind::Clone(_)
            | ExpressionKind::ResultOk(_)
            | ExpressionKind::ResultFail(_)
            | ExpressionKind::OptionalSome(_)
            | ExpressionKind::OptionalNone
            | ExpressionKind::ListConstruct { .. } => true,
            _ => false,
        });
    }
    // Count owning emitter operations, not string-typed AST nodes. Children
    // accumulate until full-expression cleanup; even short-circuit alternatives
    // receive distinct slots during emission. View/clone add no ownership.
    match &value.kind {
        ExpressionKind::String(_) | ExpressionKind::Local(_) | ExpressionKind::Call { .. }
            if value.ty == TypeInterner::STRING =>
        {
            *temporaries += 1
        }
        ExpressionKind::Intrinsic {
            intrinsic: id,
            args,
            ..
        } => {
            if matches!(id, IntrinsicId::Print | IntrinsicId::Println) {
                // Empty output, argument concatenations, inter-argument spaces
                // (literal + concat), and the optional newline (literal + concat).
                *temporaries += 1 + args.len() + 2 * args.len().saturating_sub(1);
                *temporaries += usize::from(*id == IntrinsicId::Println) * 2;
                *temporaries += args.iter().filter(|a| a.ty != TypeInterner::STRING).count();
            } else if value.ty == TypeInterner::STRING {
                // String-returning leaves and scalar conversion each own once.
                *temporaries += 1;
            }
        }
        ExpressionKind::StringInterpolation(segments) => {
            // Initial empty literal plus one concatenation per segment.
            *temporaries += 1 + segments.len();
        }
        _ => {}
    }
    match &value.kind {
        ExpressionKind::Local(l) => {
            reads.insert(l.index() as usize);
        }
        ExpressionKind::Int(_)
        | ExpressionKind::Float(_)
        | ExpressionKind::Bool(_)
        | ExpressionKind::String(_)
        | ExpressionKind::Nothing => {}
        ExpressionKind::Binary { left, right, .. } => {
            visit(left, reads, temporaries, types, program, false)?;
            visit(right, reads, temporaries, types, program, false)?;
        }
        ExpressionKind::OptionalNone if program.is_some() => {}
        ExpressionKind::ListConstruct { elements } if program.is_some() => {
            for element in elements {
                visit(element, reads, temporaries, types, program, false)?;
            }
        }
        ExpressionKind::ResultOk(value)
        | ExpressionKind::ResultFail(value)
        | ExpressionKind::OptionalSome(value)
            if program.is_some() =>
        {
            visit(value, reads, temporaries, types, program, false)?
        }
        ExpressionKind::Unary { value, .. } => {
            visit(value, reads, temporaries, types, program, false)?
        }
        ExpressionKind::View(value) | ExpressionKind::Clone(value) => {
            visit(value, reads, temporaries, types, program, true)?
        }
        ExpressionKind::Call { function, args, .. } => {
            for (index, v) in args.iter().enumerate() {
                let borrowed = program.is_some_and(|p| {
                    p.functions[function.index() as usize].params[index].mode
                        == crate::ParamMode::View
                });
                visit(v, reads, temporaries, types, program, borrowed)?;
            }
        }
        ExpressionKind::Intrinsic {
            intrinsic, args, ..
        } => {
            for (index, v) in args.iter().enumerate() {
                visit(
                    v,
                    reads,
                    temporaries,
                    types,
                    program,
                    crate::move_values::intrinsic_borrows(*intrinsic, index),
                )?;
            }
        }
        ExpressionKind::StringInterpolation(segments) => {
            for s in segments {
                match s {
                    StringSegment::Value(v) => {
                        visit(v, reads, temporaries, types, program, false)?;
                        // Scalar formatting owns a new string; string formatting
                        // passes through the ownership already counted in v.
                        *temporaries += usize::from(v.ty != TypeInterner::STRING);
                    }
                    StringSegment::Text(_) => *temporaries += 1,
                }
            }
        }
        _ => return Err("expression needs explicit ownership lowering".into()),
    }
    Ok(())
}

fn copy_plan_type(types: &TypeInterner, ty: TypeId) -> Result<(), String> {
    if ty.index() as usize >= types.len() {
        return Err("invalid type in native ownership plan".into());
    }
    if matches!(
        types.resolve(ty),
        Type::Int8
            | Type::Int16
            | Type::Int32
            | Type::Int64
            | Type::Uint8
            | Type::Uint16
            | Type::Uint32
            | Type::Uint64
            | Type::Float32
            | Type::Float64
            | Type::Bool
            | Type::Nothing
            | Type::String
            | Type::Capability(CapabilityKind::Stdout)
    ) {
        return Ok(());
    }
    Err(format!(
        "type {} requires separate move/borrow/drop analysis",
        types.type_name(ty)
    ))
}

fn plan_type(
    types: &TypeInterner,
    ty: TypeId,
    program: Option<&crate::Program>,
) -> Result<(), String> {
    if program.is_some() {
        match types.resolve(ty) {
            Type::Bytes => return Ok(()),
            Type::Optional(inner) | Type::List(inner) => return plan_type(types, *inner, program),
            Type::Result(ok, error) => {
                plan_type(types, *ok, program)?;
                return plan_type(types, *error, program);
            }
            _ => {}
        }
    }
    copy_plan_type(types, ty)
}
