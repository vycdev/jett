//! Linear native places. Views are call-bounded loans, never owning aliases.
//! Definite availability is an intersection fixed point over actual CFG edges.
use crate::copy_values::{CopyValuePlan, switch_bindings_on_edge};
use crate::{ControlFlowGraph, Function, ParamMode, Program, StatementKind, TerminatorKind};
use jett_hir::{BinaryOp, Expression, ExpressionKind, IntrinsicId, StringSegment};
use jett_types::{Type, TypeId, TypeInterner};
use std::collections::{BTreeMap, BTreeSet};
type Set = BTreeSet<usize>;

pub fn is_linear(types: &TypeInterner, ty: TypeId) -> bool {
    matches!(
        types.resolve(ty),
        Type::Bytes
            | Type::Result(..)
            | Type::Optional(_)
            | Type::List(_)
            | Type::Struct(_)
            | Type::Enum(_)
            | Type::Bitfield(_)
    )
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
                | IntrinsicId::ListLength
                | IntrinsicId::ListGetClone
                | IntrinsicId::ListSum
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
        let mut outgoing_loans = vec![Set::new(); function.blocks.len()];
        let borrow_sources = function
            .blocks
            .iter()
            .flat_map(|b| &b.statements)
            .filter_map(|s| match s.kind {
                StatementKind::IterationBorrow {
                    source,
                    token,
                    start: true,
                } => Some((token.index() as usize, source.index() as usize)),
                _ => None,
            })
            .collect::<BTreeMap<_, _>>();
        let incoming_loans = |id, outgoing: &[Set]| {
            cfg.predecessors(id)
                .iter()
                .flat_map(|p| outgoing[p.index() as usize].iter().copied())
                .collect::<Set>()
        };
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
                let edge = switch_bindings_on_edge(function, *pred, id);
                let produced = outgoing[pred.index() as usize]
                    .union(&edge)
                    .copied()
                    .collect::<Set>();
                state = state.intersection(&produced).copied().collect();
            }
            state
        };
        loop {
            let mut changed = false;
            for &id in cfg.reverse_postorder() {
                let state = incoming(id, &outgoing);
                let (next, active) = Flow {
                    program,
                    types,
                    function,
                    state,
                    loans: Set::new(),
                    active: incoming_loans(id, &outgoing_loans),
                    borrow_sources: &borrow_sources,
                    validate: false,
                }
                .block(id)?;
                changed |= next != outgoing[id.index() as usize]
                    || active != outgoing_loans[id.index() as usize];
                outgoing_loans[id.index() as usize] = active;
                outgoing[id.index() as usize] = next;
            }
            if !changed {
                break;
            }
        }
        for &id in cfg.reverse_postorder() {
            Flow {
                program,
                types,
                function,
                state: incoming(id, &outgoing),
                loans: Set::new(),
                active: incoming_loans(id, &outgoing_loans),
                borrow_sources: &borrow_sources,
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
    types: &'a TypeInterner,
    state: Set,
    loans: Set,
    active: Set,
    borrow_sources: &'a BTreeMap<usize, usize>,
    validate: bool,
}
impl Flow<'_> {
    fn block(mut self, id: crate::BlockId) -> Result<(Set, Set), String> {
        let block = &self.function.blocks[id.index() as usize];
        for statement in &block.statements {
            match &statement.kind {
                StatementKind::IterationBorrow {
                    source,
                    token,
                    start,
                } => {
                    if self.validate && !self.state.contains(&(source.index() as usize)) {
                        return Err("iteration source is moved or uninitialized".into());
                    }
                    if *start {
                        self.active.insert(token.index() as usize);
                    } else {
                        self.active.remove(&(token.index() as usize));
                    }
                }
                StatementKind::SequenceLength { source, target }
                | StatementKind::SequenceGet { source, target, .. } => {
                    if self.validate && !self.state.contains(&(source.index() as usize)) {
                        return Err("sequence source is moved or uninitialized".into());
                    }
                    if let StatementKind::SequenceGet { index, .. } = statement.kind
                        && self.validate
                        && !self.state.contains(&(index.index() as usize))
                    {
                        return Err("sequence index uninitialized".into());
                    }
                    if let StatementKind::SequenceGet { consume: true, .. } = statement.kind {
                        if self
                            .function
                            .parameter_for_local(*source)
                            .is_some_and(|p| p.mode == ParamMode::View)
                        {
                            return Err("cannot take element from borrowed sequence".into());
                        }
                        if self.validate
                            && self.active.iter().any(|token| {
                                self.borrow_sources.get(token) == Some(&(source.index() as usize))
                            })
                        {
                            return Err("cannot take element while sequence is borrowed".into());
                        }
                    }
                    self.state.insert(target.index() as usize);
                }
                StatementKind::SumTag { source, target }
                | StatementKind::SumTake { source, target, .. } => {
                    let source_id = source.index() as usize;
                    if self.validate && !self.state.contains(&source_id) {
                        return Err("sum source is moved or uninitialized".into());
                    }
                    if matches!(statement.kind, StatementKind::SumTake { .. }) {
                        if self
                            .function
                            .parameter_for_local(*source)
                            .is_some_and(|p| p.mode == ParamMode::View)
                        {
                            return Err("cannot take payload from borrowed sum".into());
                        }
                        self.state.remove(&source_id);
                    }
                    self.state.insert(target.index() as usize);
                }
                StatementKind::Let { local, value } => {
                    self.expr(value, false)?;
                    self.state.insert(local.index() as usize);
                }
                StatementKind::Assign { target, value } => {
                    self.expr(value, false)?;
                    let ExpressionKind::Local(local) = target.kind else {
                        return Err("assignment needs a materialized place".into());
                    };
                    if self.validate
                        && self.active.iter().any(|token| {
                            self.borrow_sources.get(token) == Some(&(local.index() as usize))
                        })
                    {
                        return Err("cannot overwrite a borrowed iteration source".into());
                    }
                    if self
                        .function
                        .parameter_for_local(local)
                        .is_some_and(|p| p.mode == ParamMode::View)
                        && is_linear(self.types, target.ty)
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
            TerminatorKind::Switch { scrutinee, .. } => self.expr(scrutinee, false)?,
            TerminatorKind::Return(None)
            | TerminatorKind::Goto(_)
            | TerminatorKind::Unreachable => {}
            _ => return Err("terminator needs explicit native ownership lowering".into()),
        }
        Ok((self.state, self.active))
    }
    fn expr(&mut self, value: &Expression, borrowed: bool) -> Result<(), String> {
        match &value.kind {
            ExpressionKind::Local(local) => {
                let id = local.index() as usize;
                if self.validate && !self.state.contains(&id) {
                    return Err(format!("native place {id} is moved or uninitialized"));
                }
                if is_linear(self.types, value.ty) {
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
                        if self.validate
                            && (self.loans.contains(&id)
                                || self
                                    .active
                                    .iter()
                                    .any(|token| self.borrow_sources.get(token) == Some(&id)))
                        {
                            return Err(format!("cannot move native place {id} while borrowed"));
                        }
                        self.state.remove(&id);
                    }
                }
            }
            ExpressionKind::View(v) => {
                if !borrowed && is_linear(self.types, value.ty) {
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
            ExpressionKind::Unary { value, .. }
            | ExpressionKind::ResultOk(value)
            | ExpressionKind::ResultFail(value)
            | ExpressionKind::OptionalSome(value) => self.expr(value, false)?,
            ExpressionKind::OptionalNone => {}
            ExpressionKind::StructConstruct {
                fields,
                evaluation_order,
                ..
            } => {
                for &index in evaluation_order {
                    self.expr(&fields[index], false)?;
                }
            }
            ExpressionKind::EnumConstruct { payloads, .. } => {
                for payload in payloads {
                    self.expr(payload, false)?;
                }
            }
            ExpressionKind::BitfieldConstruct {
                fields,
                evaluation_order,
                ..
            } => {
                for &index in evaluation_order {
                    self.expr(&fields[index], false)?;
                }
            }
            ExpressionKind::Field { base, .. } => {
                if !borrowed && is_linear(self.types, value.ty) {
                    return Err(
                        "move-only field projection requires a view or explicit clone".into(),
                    );
                }
                let saved = self.loans.clone();
                self.expr(base, true)?;
                // A copied scalar/string cannot keep a parent loan alive.
                if !borrowed || !is_linear(self.types, value.ty) {
                    self.loans = saved;
                }
            }
            ExpressionKind::ListConstruct { elements } => {
                for element in elements {
                    self.expr(element, false)?;
                }
            }
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
