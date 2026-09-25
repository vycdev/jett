//! Linear native places. Views are call-bounded loans, never owning aliases.
//! Definite availability is an intersection fixed point over actual CFG edges.
use crate::copy_values::{CopyValuePlan, switch_bindings_on_edge};
use crate::{ControlFlowGraph, Function, ParamMode, Program, StatementKind, TerminatorKind};
use jett_hir::{BinaryOp, Expression, ExpressionKind, IntrinsicId, StringSegment};
use jett_types::{Type, TypeId, TypeInterner};
use std::collections::{BTreeMap, BTreeSet};
type Set = BTreeSet<usize>;

pub fn representation_type(types: &TypeInterner, ty: TypeId) -> TypeId {
    let mut current = ty;
    for _ in 0..types.len() {
        if current.index() as usize >= types.len() {
            break;
        }
        current = match types.resolve(current) {
            Type::Secret(inner) | Type::Refinement { base: inner, .. } => *inner,
            _ => break,
        };
    }
    current
}

pub fn is_string(types: &TypeInterner, ty: TypeId) -> bool {
    let underlying = representation_type(types, ty);
    (underlying.index() as usize) < types.len() && matches!(types.resolve(underlying), Type::String)
}

pub fn is_function(types: &TypeInterner, ty: TypeId) -> bool {
    let underlying = representation_type(types, ty);
    (underlying.index() as usize) < types.len()
        && matches!(types.resolve(underlying), Type::Function { .. })
}

pub fn is_copy_owned(types: &TypeInterner, ty: TypeId) -> bool {
    is_string(types, ty) || is_function(types, ty)
}

pub fn is_secret(types: &TypeInterner, ty: TypeId) -> bool {
    let mut current = ty;
    for _ in 0..types.len() {
        if current.index() as usize >= types.len() {
            break;
        }
        current = match types.resolve(current) {
            Type::Secret(_) => return true,
            Type::Refinement { base, .. } => *base,
            _ => break,
        };
    }
    false
}

pub fn is_linear(types: &TypeInterner, ty: TypeId) -> bool {
    let ty = representation_type(types, ty);
    if ty.index() as usize >= types.len() {
        return false;
    }
    matches!(
        types.resolve(ty),
        Type::Bytes
            | Type::TypeConstruction
            | Type::Result(..)
            | Type::Optional(_)
            | Type::List(_)
            | Type::Set(_)
            | Type::Map(..)
            | Type::Struct(_)
            | Type::Enum(_)
            | Type::Bitfield(_)
            | Type::Machine(_)
            | Type::MachineState { .. }
    )
}

pub fn intrinsic_borrows(id: IntrinsicId, index: usize) -> bool {
    if matches!(
        id,
        IntrinsicId::TypeConstructVariantStart | IntrinsicId::TypeConstructMachineStart
    ) && index == 0
    {
        return true;
    }
    if matches!(id, IntrinsicId::SecretCompare | IntrinsicId::SecretRedact) {
        return true;
    }
    if matches!(
        id,
        IntrinsicId::CryptoSha256
            | IntrinsicId::CryptoSha512
            | IntrinsicId::CryptoMd5
            | IntrinsicId::CryptoHmacSha256
    ) {
        return true;
    }
    (index == 1
        && matches!(
            id,
            IntrinsicId::SetRemove
                | IntrinsicId::SetContains
                | IntrinsicId::MapRemove
                | IntrinsicId::MapHas
                | IntrinsicId::MapGet
                | IntrinsicId::TypeFieldValue
                | IntrinsicId::TypeConstructPut
                | IntrinsicId::TypeVariantFieldValue
                | IntrinsicId::TypeMachineFieldValue
        ))
        || (index == 0
            && matches!(
                id,
                IntrinsicId::BytesLength
                    | IntrinsicId::BytesSlice
                    | IntrinsicId::BytesToHex
                    | IntrinsicId::BytesToString
                    | IntrinsicId::BytesGet
                    | IntrinsicId::EncodingBase64Encode
                    | IntrinsicId::ListLength
                    | IntrinsicId::ListIsSorted
                    | IntrinsicId::ListGetClone
                    | IntrinsicId::ListSum
                    | IntrinsicId::MathAverage
                    | IntrinsicId::MathMedian
                    | IntrinsicId::SetLength
                    | IntrinsicId::SetContains
                    | IntrinsicId::MapLength
                    | IntrinsicId::MapHas
                    | IntrinsicId::MapGet
                    | IntrinsicId::TypeVariantValue
                    | IntrinsicId::TypeMachineStateValue
                    | IntrinsicId::TypeFieldValue
                    | IntrinsicId::TypeVariantFieldValue
                    | IntrinsicId::TypeMachineFieldValue
            ))
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
            .filter_map(|s| match &s.kind {
                StatementKind::IterationBorrow {
                    source,
                    token,
                    start: true,
                } => Some((token.index() as usize, source.root().index() as usize)),
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
                    if self.validate && !self.state.contains(&(source.root().index() as usize)) {
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
                    if self.validate && !self.state.contains(&(source.root().index() as usize)) {
                        return Err("sequence source is moved or uninitialized".into());
                    }
                    if let StatementKind::SequenceGet { index, .. } = statement.kind
                        && self.validate
                        && !self.state.contains(&(index.index() as usize))
                    {
                        return Err("sequence index uninitialized".into());
                    }
                    if let StatementKind::SequenceGet { consume: true, .. } = statement.kind {
                        if matches!(source, crate::SequenceSource::Projected { .. }) {
                            return Err("cannot consume a projected sequence field".into());
                        }
                        if self
                            .function
                            .parameter_for_local(source.root())
                            .is_some_and(|p| p.mode == ParamMode::View)
                        {
                            return Err("cannot take element from borrowed sequence".into());
                        }
                        if self.validate
                            && self.active.iter().any(|token| {
                                self.borrow_sources.get(token)
                                    == Some(&(source.root().index() as usize))
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
                StatementKind::Trace(local) => {
                    if self.validate && !self.state.contains(&(local.index() as usize)) {
                        return Err("trace target is moved or uninitialized".into());
                    }
                }
                StatementKind::Breakpoint {
                    condition,
                    bindings,
                } => {
                    if let Some(condition) = condition {
                        self.expr(condition, false)?;
                    }
                    if self.validate
                        && bindings
                            .iter()
                            .any(|local| !self.state.contains(&(local.index() as usize)))
                    {
                        return Err("breakpoint binding is moved or uninitialized".into());
                    }
                }
                _ => return Err("statement needs explicit native ownership lowering".into()),
            }
            self.loans.clear();
        }
        match &block.terminator.kind {
            TerminatorKind::Return(Some(v))
            | TerminatorKind::Respond(v)
            | TerminatorKind::Branch { condition: v, .. } => self.expr(v, false)?,
            TerminatorKind::Switch { scrutinee, .. } => self.expr(scrutinee, false)?,
            TerminatorKind::ReflectedTypeDispatch { type_info, .. } => {
                self.expr(type_info, true)?
            }
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
                    let explicit_view = matches!(args[index].kind, ExpressionKind::View(_))
                        && is_linear(self.types, args[index].ty);
                    self.expr(&args[index], view || explicit_view)?;
                }
                self.loans = saved;
            }
            ExpressionKind::IndirectCall {
                callee,
                args,
                evaluation_order,
            } => {
                self.expr(callee, false)?;
                let saved = self.loans.clone();
                for &index in evaluation_order {
                    let explicit_view = matches!(args[index].kind, ExpressionKind::View(_))
                        && is_linear(self.types, args[index].ty);
                    self.expr(&args[index], explicit_view)?;
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
                let operand_type = representation_type(self.types, left.ty);
                let enum_equality = matches!(op, BinaryOp::Equal | BinaryOp::NotEqual)
                    && (operand_type.index() as usize) < self.types.len()
                    && matches!(self.types.resolve(operand_type), Type::Enum(_));
                self.expr(left, enum_equality)?;
                let before = self.state.clone();
                self.expr(right, enum_equality)?;
                if matches!(op, BinaryOp::And | BinaryOp::Or) {
                    self.state = self.state.intersection(&before).copied().collect();
                }
            }
            ExpressionKind::Declassify(value)
            | ExpressionKind::Coarsen(value)
            | ExpressionKind::RefinementValidated(value)
            | ExpressionKind::Run(value)
            | ExpressionKind::Join(value)
            | ExpressionKind::Cancel(value)
            | ExpressionKind::Unary { value, .. }
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
            ExpressionKind::MachineConstruct { payloads, .. } => {
                for payload in payloads {
                    self.expr(payload, false)?;
                }
            }
            ExpressionKind::MachineTransition {
                source, payloads, ..
            } => {
                self.expr(source, false)?;
                for payload in payloads {
                    self.expr(payload, false)?;
                }
            }
            ExpressionKind::StateIs { value, .. } => self.expr(value, true)?,
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
                let saved = self.loans.clone();
                self.expr(base, true)?;
                // An owned field read clones the selected value before ending
                // the parent loan; a projected view retains the loan.
                if !borrowed || !is_linear(self.types, value.ty) {
                    self.loans = saved;
                }
            }
            ExpressionKind::ListConstruct { elements } => {
                for element in elements {
                    self.expr(element, false)?;
                }
            }
            ExpressionKind::MapConstruct { entries } => {
                for entry in entries {
                    self.expr(&entry.key, false)?;
                    self.expr(&entry.value, false)?;
                }
            }
            ExpressionKind::StringInterpolation(segments) => {
                for segment in segments {
                    if let StringSegment::Value(v) = segment {
                        self.expr(v, false)?;
                    }
                }
            }
            ExpressionKind::ClosureRef { captures, .. } => {
                if self.validate {
                    for capture in captures {
                        let id = capture.index() as usize;
                        if !self.state.contains(&id) {
                            return Err(format!("native capture {id} is moved or uninitialized"));
                        }
                    }
                }
            }
            ExpressionKind::Int(_)
            | ExpressionKind::Float(_)
            | ExpressionKind::Bool(_)
            | ExpressionKind::String(_)
            | ExpressionKind::FunctionRef(_)
            | ExpressionKind::Nothing => {}
            _ => return Err("expression needs explicit native ownership lowering".into()),
        }
        Ok(())
    }
}
