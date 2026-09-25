//! Ownership planning for the copyable scalar/string MIR subset.
//! This deliberately rejects unextracted handlers rather than walking hidden
//! control flow. Resources and other move-only values need a distinct
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
                        reads.insert(source.root().index() as usize);
                        None
                    }
                    StatementKind::SequenceLength { source, target }
                    | StatementKind::SequenceGet { source, target, .. } => {
                        reads.insert(source.root().index() as usize);
                        if let StatementKind::SequenceGet { index, .. } = statement.kind {
                            reads.insert(index.index() as usize);
                            temporaries += usize::from(
                                crate::move_values::is_copy_owned(
                                    types,
                                    function.locals[target.index() as usize].ty,
                                ) || crate::move_values::is_linear(
                                    types,
                                    function.locals[target.index() as usize].ty,
                                ),
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
                                crate::move_values::is_copy_owned(types, ty)
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
                    StatementKind::Trace(local) => {
                        reads.insert(local.index() as usize);
                        None
                    }
                    StatementKind::Breakpoint {
                        condition,
                        bindings,
                    } => {
                        if let Some(condition) = condition {
                            visit(
                                condition,
                                &mut reads,
                                &mut temporaries,
                                types,
                                program,
                                false,
                            )?;
                        }
                        reads.extend(bindings.iter().map(|local| local.index() as usize));
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
                TerminatorKind::ReflectedTypeDispatch { type_info, .. } if program.is_some() => {
                    visit(
                        type_info,
                        &mut reads,
                        &mut temporaries,
                        types,
                        program,
                        true,
                    )?;
                }
                TerminatorKind::Switch {
                    scrutinee,
                    variants,
                    ..
                } if program.is_some() => {
                    visit(
                        scrutinee,
                        &mut reads,
                        &mut temporaries,
                        types,
                        program,
                        false,
                    )?;
                    temporaries += variants
                        .iter()
                        .map(|(_, _, bindings)| {
                            bindings
                                .iter()
                                .filter(|binding| {
                                    let ty = function.locals[binding.index() as usize].ty;
                                    crate::move_values::is_copy_owned(types, ty)
                                        || crate::move_values::is_linear(types, ty)
                                })
                                .count()
                        })
                        .max()
                        .unwrap_or(0);
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
                        let edge = switch_bindings_on_edge(function, *pred, id);
                        let produced = initialized_out[pred.index() as usize]
                            .union(&edge)
                            .copied()
                            .collect::<Set>();
                        incoming = incoming.intersection(&produced).copied().collect();
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
                    .flat_map(|s| {
                        let edge = switch_bindings_on_edge(function, id, *s);
                        live_in[s.index() as usize]
                            .difference(&edge)
                            .copied()
                            .collect::<Set>()
                    })
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
                    crate::move_values::is_copy_owned(types, l.ty)
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
pub(crate) fn switch_bindings_on_edge(
    function: &Function,
    source: crate::BlockId,
    target: crate::BlockId,
) -> Set {
    let TerminatorKind::Switch {
        variants,
        otherwise,
        ..
    } = &function.blocks[source.index() as usize].terminator.kind
    else {
        return Set::new();
    };
    let mut candidates = variants
        .iter()
        .filter(|(_, block, _)| *block == target)
        .map(|(_, _, bindings)| {
            bindings
                .iter()
                .map(|local| local.index() as usize)
                .collect::<Set>()
        })
        .collect::<Vec<_>>();
    if *otherwise == Some(target) {
        candidates.push(Set::new());
    }
    let Some(mut common) = candidates.pop() else {
        return Set::new();
    };
    for candidate in candidates {
        common = common.intersection(&candidate).copied().collect();
    }
    common
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
            | ExpressionKind::Join(_)
            | ExpressionKind::ListConstruct { .. }
            | ExpressionKind::MapConstruct { .. }
            | ExpressionKind::StructConstruct { .. }
            | ExpressionKind::EnumConstruct { .. } => true,
            ExpressionKind::Field { .. } => !borrowed,
            ExpressionKind::MachineConstruct { .. } | ExpressionKind::MachineTransition { .. } => {
                true
            }
            ExpressionKind::BitfieldConstruct { .. } => true,
            _ => false,
        });
    }
    // Count owning emitter operations, not copy-owned AST nodes. Children
    // accumulate until full-expression cleanup; even short-circuit alternatives
    // receive distinct slots during emission. View/clone add no ownership.
    match &value.kind {
        ExpressionKind::String(_)
        | ExpressionKind::Local(_)
        | ExpressionKind::Call { .. }
        | ExpressionKind::IndirectCall { .. }
        | ExpressionKind::Field { .. }
        | ExpressionKind::FunctionRef(_)
            if crate::move_values::is_copy_owned(types, value.ty) =>
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
                *temporaries += args
                    .iter()
                    .filter(|a| !crate::move_values::is_string(types, a.ty))
                    .count();
            } else if crate::move_values::is_copy_owned(types, value.ty) {
                // Copy-owned leaves and scalar conversion each own once.
                *temporaries += 1;
            }
        }
        ExpressionKind::StringInterpolation(segments) => {
            // Initial empty literal plus one concatenation per segment.
            *temporaries += 1 + segments.len();
        }
        ExpressionKind::ClosureRef { function, captures } => {
            // The descriptor and its environment are both owned temporaries.
            *temporaries += 2;
            let closure = program
                .and_then(|program| program.functions.get(function.index() as usize))
                .ok_or("closure capture needs its checked function")?;
            if closure.capture_count != captures.len() {
                return Err("closure capture count disagrees with its function".into());
            }
            for (capture, param) in captures.iter().zip(&closure.params) {
                if param.local != *capture {
                    return Err("closure capture order disagrees with its function".into());
                }
                *temporaries += usize::from(crate::move_values::is_copy_owned(types, param.ty));
            }
        }
        _ => {}
    }
    match &value.kind {
        ExpressionKind::Local(l) => {
            reads.insert(l.index() as usize);
        }
        ExpressionKind::ClosureRef { captures, .. } => {
            reads.extend(captures.iter().map(|local| local.index() as usize));
        }
        ExpressionKind::Int(_)
        | ExpressionKind::Float(_)
        | ExpressionKind::Bool(_)
        | ExpressionKind::String(_)
        | ExpressionKind::FunctionRef(_)
        | ExpressionKind::Nothing => {}
        ExpressionKind::Binary { left, right, .. } => {
            visit(left, reads, temporaries, types, program, false)?;
            visit(right, reads, temporaries, types, program, false)?;
        }
        ExpressionKind::OptionalNone if program.is_some() => {}
        ExpressionKind::StructConstruct {
            fields,
            validates_refinements,
            ..
        } if program.is_some() => {
            // A validated construction owns the record before wrapping it in
            // a separately owned success sum. The expression type counts the
            // sum above; reserve one more slot for the intermediate record.
            *temporaries += usize::from(*validates_refinements);
            for field in fields {
                visit(field, reads, temporaries, types, program, false)?;
            }
        }
        ExpressionKind::EnumConstruct { payloads, .. } if program.is_some() => {
            for payload in payloads {
                visit(payload, reads, temporaries, types, program, false)?;
            }
        }
        ExpressionKind::MachineConstruct { payloads, .. } if program.is_some() => {
            for payload in payloads {
                visit(payload, reads, temporaries, types, program, false)?;
            }
        }
        ExpressionKind::MachineTransition {
            source, payloads, ..
        } if program.is_some() => {
            visit(source, reads, temporaries, types, program, false)?;
            for payload in payloads {
                visit(payload, reads, temporaries, types, program, false)?;
            }
        }
        ExpressionKind::StateIs { value, .. } if program.is_some() => {
            visit(value, reads, temporaries, types, program, true)?;
        }
        ExpressionKind::BitfieldConstruct { fields, .. } if program.is_some() => {
            for field in fields {
                visit(field, reads, temporaries, types, program, false)?;
            }
        }
        ExpressionKind::Field { base, .. } if program.is_some() => {
            visit(base, reads, temporaries, types, program, true)?;
        }
        ExpressionKind::ListConstruct { elements } if program.is_some() => {
            for element in elements {
                visit(element, reads, temporaries, types, program, false)?;
            }
        }
        ExpressionKind::MapConstruct { entries } if program.is_some() => {
            for entry in entries {
                visit(&entry.key, reads, temporaries, types, program, false)?;
                visit(&entry.value, reads, temporaries, types, program, false)?;
            }
        }
        ExpressionKind::ResultOk(value)
        | ExpressionKind::ResultFail(value)
        | ExpressionKind::OptionalSome(value)
            if program.is_some() =>
        {
            visit(value, reads, temporaries, types, program, false)?
        }
        ExpressionKind::Declassify(value)
        | ExpressionKind::Coarsen(value)
        | ExpressionKind::RefinementValidated(value)
        | ExpressionKind::Run(value)
        | ExpressionKind::Join(value)
        | ExpressionKind::Cancel(value)
        | ExpressionKind::Unary { value, .. } => {
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
                if !borrowed
                    && matches!(v.kind, ExpressionKind::View(_))
                    && crate::move_values::is_linear(types, v.ty)
                {
                    *temporaries += 1;
                }
            }
        }
        ExpressionKind::IndirectCall { callee, args, .. } => {
            visit(callee, reads, temporaries, types, program, false)?;
            for argument in args {
                visit(argument, reads, temporaries, types, program, false)?;
                if matches!(argument.kind, ExpressionKind::View(_))
                    && crate::move_values::is_linear(types, argument.ty)
                {
                    *temporaries += 1;
                }
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
                        *temporaries += usize::from(!crate::move_values::is_string(types, v.ty));
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
    if let Type::Secret(inner) | Type::Refinement { base: inner, .. } = types.resolve(ty) {
        return copy_plan_type(types, *inner);
    }
    if let Type::Function {
        params,
        return_type,
    } = types.resolve(ty)
    {
        for param in params {
            copy_plan_type(types, *param)?;
        }
        return copy_plan_type(types, *return_type);
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
            | Type::Capability(CapabilityKind::Clock)
            | Type::Capability(CapabilityKind::Random)
            | Type::Capability(CapabilityKind::Environment)
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
    plan_type_inner(types, ty, program, &mut BTreeSet::new())
}
fn plan_type_inner(
    types: &TypeInterner,
    ty: TypeId,
    program: Option<&crate::Program>,
    seen: &mut BTreeSet<u32>,
) -> Result<(), String> {
    if ty.index() as usize >= types.len() {
        return Err("invalid native type".into());
    }
    if !seen.insert(ty.index()) {
        return Ok(());
    }
    if program.is_some() {
        match types.resolve(ty) {
            Type::Secret(inner) | Type::Refinement { base: inner, .. } => {
                return plan_type_inner(types, *inner, program, seen);
            }
            Type::Function {
                params,
                return_type,
            } => {
                for param in params {
                    plan_type_inner(types, *param, program, seen)?;
                }
                return plan_type_inner(types, *return_type, program, seen);
            }
            Type::Bytes | Type::TypeConstruction => return Ok(()),
            Type::Struct(id) => {
                for (_, field) in &types.resolve_struct(*id).fields {
                    plan_type_inner(types, *field, program, seen)?;
                }
                return Ok(());
            }
            Type::Enum(id) => {
                for variant in &types.resolve_enum(*id).variants {
                    for (_, field) in &variant.fields {
                        plan_type_inner(types, *field, program, seen)?;
                    }
                }
                return Ok(());
            }
            Type::Machine(id) | Type::MachineState { machine: id, .. } => {
                for state in &types.resolve_machine(*id).states {
                    for (_, field) in &state.fields {
                        plan_type_inner(types, *field, program, seen)?;
                    }
                }
                return Ok(());
            }
            Type::Bitfield(id) => {
                for field in &types.resolve_bitfield(*id).fields {
                    plan_type_inner(types, field.ty, program, seen)?;
                }
                return Ok(());
            }
            Type::List(inner) if *inner == TypeInterner::NEVER => return Ok(()),
            Type::Optional(inner) | Type::List(inner) | Type::Set(inner) => {
                return plan_type_inner(types, *inner, program, seen);
            }
            Type::Map(key, value) => {
                plan_type_inner(types, *key, program, seen)?;
                return plan_type_inner(types, *value, program, seen);
            }
            Type::Result(ok, error) => {
                plan_type_inner(types, *ok, program, seen)?;
                return plan_type_inner(types, *error, program, seen);
            }
            _ => {}
        }
    }
    copy_plan_type(types, ty)
}
