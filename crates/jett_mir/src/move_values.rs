//! Linear native places. Views are call-bounded loans, never owning aliases.
//! Definite availability is an intersection fixed point over actual CFG edges.
use crate::copy_values::CopyValuePlan;
use crate::{ControlFlowGraph, Function, ParamMode, Program, StatementKind, TerminatorKind};
use jett_hir::{BinaryOp, Expression, ExpressionKind, IntrinsicId, StringSegment};
use jett_types::{TypeId, TypeInterner};
use std::collections::BTreeSet;
type Set = BTreeSet<usize>;

pub fn is_linear(ty: TypeId) -> bool {
    ty == TypeInterner::BYTES
}

pub fn intrinsic_borrows(id: IntrinsicId, index: usize) -> bool {
    index == 0
        && matches!(
            id,
            IntrinsicId::BytesLength
                | IntrinsicId::BytesSlice
                | IntrinsicId::BytesToHex
                | IntrinsicId::BytesToString
                | IntrinsicId::BytesGet
        )
}

pub struct MoveValuePlan;
impl MoveValuePlan {
    pub fn analyze(
        program: &Program,
        function: &Function,
        types: &TypeInterner,
    ) -> Result<CopyValuePlan, String> {
        let plan = CopyValuePlan::analyze_storage(function, types, Some(program))?;
        let cfg = ControlFlowGraph::analyze(function).map_err(|e| format!("{e:?}"))?;
        let all: Set = (0..function.locals.len()).collect();
        let params: Set = function
            .params
            .iter()
            .map(|p| p.local.index() as usize)
            .collect();
        let mut outgoing = vec![all.clone(); function.blocks.len()];
        let incoming = |id: crate::BlockId, outgoing: &[Set]| {
            if id == function.entry {
                return params.clone();
            }
            let mut state = all.clone();
            for pred in cfg
                .predecessors(id)
                .iter()
                .filter(|p| cfg.reverse_postorder().contains(p))
            {
                state = state
                    .intersection(&outgoing[pred.index() as usize])
                    .copied()
                    .collect();
            }
            state
        };
        loop {
            let mut changed = false;
            for &id in cfg.reverse_postorder() {
                let state = incoming(id, &outgoing);
                let next = Flow {
                    program,
                    function,
                    state,
                    loans: Set::new(),
                    validate: false,
                }
                .block(id)?;
                changed |= next != outgoing[id.index() as usize];
                outgoing[id.index() as usize] = next;
            }
            if !changed {
                break;
            }
        }
        for &id in cfg.reverse_postorder() {
            Flow {
                program,
                function,
                state: incoming(id, &outgoing),
                loans: Set::new(),
                validate: true,
            }
            .block(id)?;
        }
        Ok(plan)
    }
}
struct Flow<'a> {
    program: &'a Program,
    function: &'a Function,
    state: Set,
    loans: Set,
    validate: bool,
}
impl Flow<'_> {
    fn block(mut self, id: crate::BlockId) -> Result<Set, String> {
        let block = &self.function.blocks[id.index() as usize];
        for statement in &block.statements {
            match &statement.kind {
                StatementKind::Let { local, value } => {
                    self.expr(value, false)?;
                    self.state.insert(local.index() as usize);
                }
                StatementKind::Assign { target, value } => {
                    self.expr(value, false)?;
                    let ExpressionKind::Local(local) = target.kind else {
                        return Err("assignment needs a materialized place".into());
                    };
                    if self
                        .function
                        .parameter_for_local(local)
                        .is_some_and(|p| p.mode == ParamMode::View)
                        && is_linear(target.ty)
                    {
                        return Err("cannot overwrite a borrowed native place".into());
                    }
                    self.state.insert(local.index() as usize);
                }
                StatementKind::Evaluate(value) => self.expr(value, false)?,
                _ => return Err("statement needs explicit native ownership lowering".into()),
            }
            self.loans.clear();
        }
        match &block.terminator.kind {
            TerminatorKind::Return(Some(v)) | TerminatorKind::Branch { condition: v, .. } => {
                self.expr(v, false)?
            }
            TerminatorKind::Return(None)
            | TerminatorKind::Goto(_)
            | TerminatorKind::Unreachable => {}
            _ => return Err("terminator needs explicit native ownership lowering".into()),
        }
        Ok(self.state)
    }
    fn expr(&mut self, value: &Expression, borrowed: bool) -> Result<(), String> {
        match &value.kind {
            ExpressionKind::Local(local) => {
                let id = local.index() as usize;
                if self.validate && !self.state.contains(&id) {
                    return Err(format!("native place {id} is moved or uninitialized"));
                }
                if is_linear(value.ty) {
                    if borrowed {
                        self.loans.insert(id);
                    } else {
                        if self
                            .function
                            .parameter_for_local(*local)
                            .is_some_and(|p| p.mode == ParamMode::View)
                        {
                            return Err(format!("cannot move borrowed native place {id}"));
                        }
                        if self.validate && self.loans.contains(&id) {
                            return Err(format!("cannot move native place {id} while borrowed"));
                        }
                        self.state.remove(&id);
                    }
                }
            }
            ExpressionKind::View(v) => {
                if !borrowed && is_linear(value.ty) {
                    return Err("native view cannot escape into an owning value".into());
                }
                self.expr(v, true)?;
            }
            ExpressionKind::Clone(v) => {
                let saved = self.loans.clone();
                self.expr(v, true)?;
                self.loans = saved;
            }
            ExpressionKind::Call {
                function,
                args,
                evaluation_order,
            } => {
                let saved = self.loans.clone();
                for &index in evaluation_order {
                    let view = self.program.functions[function.index() as usize].params[index].mode
                        == ParamMode::View;
                    self.expr(&args[index], view)?;
                }
                self.loans = saved;
            }
            ExpressionKind::Intrinsic {
                intrinsic,
                args,
                evaluation_order,
                ..
            } => {
                let saved = self.loans.clone();
                for &index in evaluation_order {
                    self.expr(&args[index], intrinsic_borrows(*intrinsic, index))?;
                }
                self.loans = saved;
            }
            ExpressionKind::Binary { left, op, right } => {
                self.expr(left, false)?;
                let before = self.state.clone();
                self.expr(right, false)?;
                if matches!(op, BinaryOp::And | BinaryOp::Or) {
                    self.state = self.state.intersection(&before).copied().collect();
                }
            }
            ExpressionKind::Unary { value, .. } => self.expr(value, false)?,
            ExpressionKind::StringInterpolation(segments) => {
                for segment in segments {
                    if let StringSegment::Value(v) = segment {
                        self.expr(v, false)?;
                    }
                }
            }
            ExpressionKind::Int(_)
            | ExpressionKind::Float(_)
            | ExpressionKind::Bool(_)
            | ExpressionKind::String(_)
            | ExpressionKind::Nothing => {}
            _ => return Err("expression needs explicit native ownership lowering".into()),
        }
        Ok(())
    }
}
