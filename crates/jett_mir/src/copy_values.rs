//! Ownership planning for the copyable scalar/string MIR subset.
//! This deliberately rejects unextracted handlers/closures rather than walking
//! hidden control flow. Resources and other move-only values need a distinct
//! move/borrow/drop analysis before their backend support can be enabled.
use crate::{ControlFlowGraph, Function, StatementKind, TerminatorKind};
use jett_hir::{Expression, ExpressionKind, StringSegment};
use jett_types::{Type, TypeInterner};
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
        let cfg = ControlFlowGraph::analyze(function).map_err(|e| format!("{e:?}"))?;
        let n = function.blocks.len();
        let mut facts = Vec::new();
        let mut max_nodes = 0;
        for block in &function.blocks {
            let mut statements = Vec::new();
            for statement in &block.statements {
                let mut reads = Set::new();
                let mut nodes = 0;
                let definition = match &statement.kind {
                    StatementKind::Let { local, value } => {
                        visit(value, &mut reads, &mut nodes)?;
                        Some(local.index() as usize)
                    }
                    StatementKind::Assign { target, value } => {
                        let ExpressionKind::Local(local) = target.kind else {
                            return Err("nonlocal assignment needs place ownership".into());
                        };
                        visit(value, &mut reads, &mut nodes)?;
                        Some(local.index() as usize)
                    }
                    StatementKind::Evaluate(value) => {
                        visit(value, &mut reads, &mut nodes)?;
                        None
                    }
                    _ => return Err("statement needs explicit ownership lowering".into()),
                };
                max_nodes = max_nodes.max(nodes);
                statements.push((reads, definition));
            }
            let mut reads = Set::new();
            let mut nodes = 0;
            match &block.terminator.kind {
                TerminatorKind::Return(Some(v)) | TerminatorKind::Branch { condition: v, .. } => {
                    visit(v, &mut reads, &mut nodes)?
                }
                TerminatorKind::Return(None)
                | TerminatorKind::Goto(_)
                | TerminatorKind::Unreachable => {}
                _ => return Err("terminator needs explicit ownership lowering".into()),
            }
            max_nodes = max_nodes.max(nodes);
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
                .filter(|l| matches!(types.resolve(l.ty), Type::String))
                .map(|l| l.id.index() as usize)
                .collect(),
            live_in,
            live_out,
            live_after_statement: after,
            temporary_slots: if max_nodes == 0 { 0 } else { max_nodes * 4 + 8 },
        })
    }
}
fn visit(value: &Expression, reads: &mut Set, nodes: &mut usize) -> Result<(), String> {
    if value.ty == TypeInterner::STRING {
        *nodes += 1;
    }
    if let ExpressionKind::Intrinsic { args, .. } = &value.kind {
        *nodes += args.len() + 1;
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
            visit(left, reads, nodes)?;
            visit(right, reads, nodes)?;
        }
        ExpressionKind::Unary { value, .. }
        | ExpressionKind::View(value)
        | ExpressionKind::Clone(value) => visit(value, reads, nodes)?,
        ExpressionKind::Call { args, .. } | ExpressionKind::Intrinsic { args, .. } => {
            for v in args {
                visit(v, reads, nodes)?;
            }
        }
        ExpressionKind::StringInterpolation(segments) => {
            for s in segments {
                match s {
                    StringSegment::Value(v) => visit(v, reads, nodes)?,
                    StringSegment::Text(_) => *nodes += 1,
                }
            }
        }
        _ => return Err("expression needs explicit ownership lowering".into()),
    }
    Ok(())
}
