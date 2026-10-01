//! Linear native places, call loans, and immutable nonowning local aliases.
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

/// Interface boxes have an erased identity. A refinement over an interface
/// instead has its own nominal identity, despite sharing the handle layout.
pub fn is_erased_interface(types: &TypeInterner, mut ty: TypeId) -> bool {
    for _ in 0..types.len() {
        if ty.index() as usize >= types.len() {
            return false;
        }
        match types.resolve(ty) {
            Type::Secret(inner) => ty = *inner,
            Type::Interface(_) => return true,
            _ => return false,
        }
    }
    false
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
            | Type::Interface(_)
            | Type::Enum(_)
            | Type::Bitfield(_)
            | Type::Machine(_)
            | Type::MachineState { .. }
    )
}

pub fn intrinsic_borrows(id: IntrinsicId, index: usize, argument: &Expression) -> bool {
    if matches!(id, IntrinsicId::Print | IntrinsicId::Println) {
        let mut value = argument;
        while let ExpressionKind::Coarsen(inner) | ExpressionKind::Declassify(inner) = &value.kind {
            value = inner;
        }
        // Debug output accepts explicit views without taking their owner.
        // Ordinary arguments retain their checked consumption behavior.
        return matches!(value.kind, ExpressionKind::View(_));
    }
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
        let mut plan = CopyValuePlan::analyze_storage(function, types, Some(program))?;
        if function.identity.declaration.kind == jett_hir::DeclarationKind::ActorHandler {
            // Captured state is written back after a return/respond terminator.
            // Keep its owning slots live even when the source body stops reading it.
            let captures = function
                .params
                .iter()
                .take(function.capture_count)
                .map(|param| param.local.index() as usize)
                .collect::<Set>();
            for live in plan.live_in.iter_mut().chain(&mut plan.live_out) {
                live.extend(&captures);
            }
            for block in &mut plan.live_after_statement {
                for live in block {
                    live.extend(&captures);
                }
            }
        }
        let cfg = ControlFlowGraph::analyze(function).map_err(|e| format!("{e:?}"))?;
        let all: Set = (0..function.locals.len()).collect();
        let params: Set = function
            .params
            .iter()
            .map(|p| p.local.index() as usize)
            .collect();
        let mut outgoing = vec![all.clone(); function.blocks.len()];
        let mut outgoing_loans = vec![Set::new(); function.blocks.len()];
        let mut outgoing_aliases = vec![Set::new(); function.blocks.len()];
        let mut borrow_sources = BTreeMap::new();
        for statement in function.blocks.iter().flat_map(|block| &block.statements) {
            if let StatementKind::IterationBorrow {
                source,
                token,
                start: true,
            } = &statement.kind
            {
                let root = function
                    .view_root(source.root())
                    .ok_or("iteration source has an invalid borrowed origin")?;
                borrow_sources.insert(token.index() as usize, root.index() as usize);
            }
        }
        let mut alias_sources = BTreeMap::new();
        for local in &function.locals {
            if local.view_source.is_some() {
                let root = function
                    .view_root(local.id)
                    .ok_or("borrowed local origin is invalid or cyclic")?;
                alias_sources.insert(local.id.index() as usize, root.index() as usize);
            }
        }
        let incoming_loans = |id, outgoing: &[Set]| {
            cfg.predecessors(id)
                .iter()
                .filter(|p| cfg.reverse_postorder().contains(p))
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
                let (next, active, aliases) = Flow {
                    program,
                    types,
                    function,
                    state,
                    loans: Set::new(),
                    active: incoming_loans(id, &outgoing_loans),
                    aliases: incoming_loans(id, &outgoing_aliases),
                    borrow_sources: &borrow_sources,
                    alias_sources: &alias_sources,
                    validate: false,
                }
                .block(id)?;
                changed |= next != outgoing[id.index() as usize]
                    || active != outgoing_loans[id.index() as usize]
                    || aliases != outgoing_aliases[id.index() as usize];
                outgoing_loans[id.index() as usize] = active;
                outgoing_aliases[id.index() as usize] = aliases;
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
                aliases: incoming_loans(id, &outgoing_aliases),
                borrow_sources: &borrow_sources,
                alias_sources: &alias_sources,
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
    // May-created aliases never expire in this bounded native implementation.
    // Choosing lexical or last-use loan expiration remains a separate contract.
    aliases: Set,
    borrow_sources: &'a BTreeMap<usize, usize>,
    alias_sources: &'a BTreeMap<usize, usize>,
    validate: bool,
}
impl Flow<'_> {
    fn block(mut self, id: crate::BlockId) -> Result<(Set, Set, Set), String> {
        let block = &self.function.blocks[id.index() as usize];
        for statement in &block.statements {
            match &statement.kind {
                StatementKind::IterationBorrow {
                    source,
                    token,
                    start,
                } => {
                    self.read(source.root(), "iteration source")?;
                    if *start {
                        self.active.insert(token.index() as usize);
                    } else {
                        self.active.remove(&(token.index() as usize));
                    }
                }
                StatementKind::SequenceLength { source, target }
                | StatementKind::SequenceGet { source, target, .. } => {
                    let root = self.read(source.root(), "sequence source")?;
                    if let StatementKind::SequenceGet { index, .. } = statement.kind {
                        self.read(index, "sequence index")?;
                    }
                    if let StatementKind::SequenceGet { consume: true, .. } = statement.kind {
                        if matches!(source, crate::SequenceSource::Projected { .. }) {
                            return Err("cannot consume a projected sequence field".into());
                        }
                        if self.function.is_view_local(source.root()) {
                            return Err("cannot take element from borrowed sequence".into());
                        }
                        self.reject_aliased_owner_change(root)?;
                        if self.validate
                            && self
                                .active
                                .iter()
                                .any(|token| self.borrow_sources.get(token) == Some(&root))
                        {
                            return Err("cannot take element while sequence is borrowed".into());
                        }
                    }
                    self.require_owned_definition(*target)?;
                    self.state.insert(target.index() as usize);
                }
                StatementKind::SumTag { source, target }
                | StatementKind::SumTake { source, target, .. } => {
                    let source_id = source.index() as usize;
                    let root = self.read(*source, "sum source")?;
                    if matches!(statement.kind, StatementKind::SumTake { .. }) {
                        if self.function.is_view_local(*source) {
                            return Err("cannot take payload from borrowed sum".into());
                        }
                        self.reject_aliased_owner_change(root)?;
                        self.state.remove(&source_id);
                    }
                    self.require_owned_definition(*target)?;
                    self.state.insert(target.index() as usize);
                }
                StatementKind::Let { local, value } => {
                    let definition = self
                        .function
                        .local(*local)
                        .ok_or("native local definition is outside its function")?;
                    if let Some(source) = definition.view_source {
                        jett_hir::validate_local_view_initializer(
                            value,
                            source,
                            definition.ty,
                            self.types,
                        )?;
                        self.expr(value, true)?;
                        self.aliases.insert(local.index() as usize);
                    } else {
                        self.reject_aliased_owner_change(local.index() as usize)?;
                        self.expr(value, false)?;
                    }
                    self.state.insert(local.index() as usize);
                }
                StatementKind::CheckRefinement { local, call, .. } => {
                    self.require_owned_definition(*local)?;
                    self.reject_aliased_owner_change(local.index() as usize)?;
                    self.expr(call, false)?;
                    self.state.insert(local.index() as usize);
                }
                StatementKind::Assign { target, value } => {
                    self.expr(value, false)?;
                    let ExpressionKind::Local(local) = target.kind else {
                        return Err("assignment needs a materialized place".into());
                    };
                    self.require_owned_definition(local)?;
                    self.reject_aliased_owner_change(local.index() as usize)?;
                    if self.validate
                        && self.active.iter().any(|token| {
                            self.borrow_sources.get(token) == Some(&(local.index() as usize))
                        })
                    {
                        return Err("cannot overwrite a borrowed iteration source".into());
                    }
                    if self.function.is_view_local(local)
                        && (is_linear(self.types, target.ty) || is_function(self.types, target.ty))
                    {
                        return Err("cannot overwrite a borrowed native place".into());
                    }
                    self.state.insert(local.index() as usize);
                }
                StatementKind::Evaluate(value) => self.expr(value, false)?,
                StatementKind::Assert { condition, message } => {
                    self.expr(condition, false)?;
                    if let Some(message) = message {
                        self.expr(message, false)?;
                    }
                }
                StatementKind::Trace(local) => {
                    self.read(*local, "trace target")?;
                }
                StatementKind::Breakpoint {
                    condition,
                    bindings,
                } => {
                    if let Some(condition) = condition {
                        self.expr(condition, false)?;
                    }
                    for local in bindings {
                        self.read(*local, "breakpoint binding")?;
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
        Ok((self.state, self.active, self.aliases))
    }

    fn read(&self, local: jett_hir::LocalId, description: &str) -> Result<usize, String> {
        let root = self
            .function
            .view_root(local)
            .ok_or("borrowed local origin is invalid or cyclic")?;
        let mut current = local;
        loop {
            if self.validate && !self.state.contains(&(current.index() as usize)) {
                return Err(format!("{description} is moved or uninitialized"));
            }
            let metadata = self
                .function
                .local(current)
                .ok_or("borrowed local origin is outside its function")?;
            match metadata.view_source {
                Some(source) => current = source,
                None => return Ok(root.index() as usize),
            }
        }
    }

    fn require_owned_definition(&self, local: jett_hir::LocalId) -> Result<(), String> {
        let metadata = self
            .function
            .local(local)
            .ok_or("native local definition is outside its function")?;
        if metadata.view_source.is_some() {
            return Err("borrowed local alias requires its matching initialization".into());
        }
        Ok(())
    }

    fn reject_aliased_owner_change(&self, root: usize) -> Result<(), String> {
        if self.validate
            && self
                .aliases
                .iter()
                .any(|alias| self.alias_sources.get(alias) == Some(&root))
        {
            return Err(format!(
                "native owner {root} consumption or rebinding after creating a local view alias is not implemented"
            ));
        }
        Ok(())
    }

    fn expr(&mut self, value: &Expression, borrowed: bool) -> Result<(), String> {
        match &value.kind {
            ExpressionKind::Local(local) => {
                let id = local.index() as usize;
                let root = self.read(*local, &format!("native place {id}"))?;
                let borrowed_local = self.function.is_view_local(*local)
                    && (self
                        .function
                        .local(*local)
                        .is_some_and(|local| local.view_source.is_some())
                        || is_linear(self.types, value.ty)
                        || is_function(self.types, value.ty));
                if !borrowed && borrowed_local {
                    return Err(format!("cannot move borrowed native place {id}"));
                }
                if borrowed
                    && (is_linear(self.types, value.ty) || is_function(self.types, value.ty))
                {
                    self.loans.insert(root);
                }
                // Verify/property owner reads clone, but the borrowed-local
                // rejection above applies before that independent-owner path.
                if is_linear(self.types, value.ty)
                    && !borrowed
                    && !matches!(
                        self.function.identity.declaration.kind,
                        jett_hir::DeclarationKind::Verify | jett_hir::DeclarationKind::Property
                    )
                {
                    self.reject_aliased_owner_change(root)?;
                    if self.validate
                        && (self.loans.contains(&root)
                            || self
                                .active
                                .iter()
                                .any(|token| self.borrow_sources.get(token) == Some(&root)))
                    {
                        return Err(format!("cannot move native place {id} while borrowed"));
                    }
                    self.state.remove(&id);
                }
            }
            ExpressionKind::View(v) => {
                if !borrowed
                    && (is_linear(self.types, value.ty)
                        || is_function(self.types, value.ty)
                        || (is_string(self.types, value.ty)
                            && !wrapper_copies_result(self.types, value.ty)))
                {
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
            ExpressionKind::ActorSpawn {
                args,
                evaluation_order,
                constructor,
                ..
            } => {
                let saved = self.loans.clone();
                for &index in evaluation_order {
                    let view = constructor.is_some_and(|id| {
                        self.program.functions[id.index() as usize].params[index].mode
                            == ParamMode::View
                    });
                    let explicit_view = matches!(args[index].kind, ExpressionKind::View(_))
                        && is_linear(self.types, args[index].ty);
                    self.expr(&args[index], view || explicit_view)?;
                }
                self.loans = saved;
            }
            ExpressionKind::ActorMessage {
                actor,
                handler,
                args,
                evaluation_order,
                ..
            } => {
                self.expr(actor, false)?;
                let target = &self.program.functions[handler.index() as usize];
                let saved = self.loans.clone();
                for &index in evaluation_order {
                    let view = target.params[target.capture_count + index].mode == ParamMode::View;
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
                let Type::Function { view_params, .. } = self
                    .types
                    .resolve(representation_type(self.types, callee.ty))
                else {
                    return Err("indirect call has no function parameter modes".into());
                };
                if view_params.len() != args.len() {
                    return Err("indirect call argument and parameter counts differ".into());
                }
                let saved = self.loans.clone();
                for &index in evaluation_order {
                    let explicit_view = matches!(args[index].kind, ExpressionKind::View(_))
                        && is_linear(self.types, args[index].ty);
                    self.expr(&args[index], view_params[index] || explicit_view)?;
                }
                // Indirect calls evaluate arguments before the callee. Invoking
                // a descriptor observes it without transferring its ownership.
                self.expr(callee, true)?;
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
                    self.expr(
                        &args[index],
                        intrinsic_borrows(*intrinsic, index, &args[index]),
                    )?;
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
            ExpressionKind::Declassify(inner) | ExpressionKind::Coarsen(inner) => {
                // These wrappers can expose an implicitly copied primitive from
                // a nominal or secret view without transferring the source.
                self.expr(
                    inner,
                    borrowed || wrapper_copies_result(self.types, value.ty),
                )?;
            }
            ExpressionKind::RefinementValidated(value) => self.expr(value, borrowed)?,
            ExpressionKind::Run(value)
            | ExpressionKind::Join(value)
            | ExpressionKind::Cancel(value)
            | ExpressionKind::Unary { value, .. }
            | ExpressionKind::ResultOk(value)
            | ExpressionKind::ResultFail(value)
            | ExpressionKind::OptionalSome(value) => self.expr(value, false)?,
            ExpressionKind::InterfaceType(value) => self.expr(value, true)?,
            ExpressionKind::FunctionAdapter { value, .. } => self.expr(value, false)?,
            ExpressionKind::InterfaceCoerce { value: inner, .. } => {
                let unbox = is_erased_interface(self.types, inner.ty)
                    && !is_erased_interface(self.types, value.ty);
                self.expr(inner, borrowed || unbox)?;
            }
            ExpressionKind::OptionalNone => {}
            ExpressionKind::StructConstruct {
                fields,
                evaluation_order,
                ..
            }
            | ExpressionKind::EnumConstruct {
                payloads: fields,
                evaluation_order,
                ..
            } => {
                for &index in evaluation_order {
                    self.expr(&fields[index], false)?;
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
                for capture in captures {
                    self.read(*capture, "native capture")?;
                    let local = self
                        .function
                        .local(*capture)
                        .ok_or("invalid capture local")?;
                    if self.function.is_view_local(*capture)
                        && (local.view_source.is_some()
                            || is_linear(self.types, local.ty)
                            || is_function(self.types, local.ty))
                    {
                        return Err("cannot capture a borrowed native place".into());
                    }
                }
            }
            ExpressionKind::Int(_)
            | ExpressionKind::Float(_)
            | ExpressionKind::Bool(_)
            | ExpressionKind::String(_)
            | ExpressionKind::FunctionRef(_)
            | ExpressionKind::Nothing
            | ExpressionKind::PropertyCaseContext(_)
            | ExpressionKind::RuntimeFailure(_) => {}
            _ => return Err("expression needs explicit native ownership lowering".into()),
        }
        Ok(())
    }
}

fn wrapper_copies_result(types: &TypeInterner, ty: TypeId) -> bool {
    matches!(
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
            | Type::String
            | Type::Bool
            | Type::Nothing
    )
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use jett_common::{FileId, SourceOrigin};
    use jett_hir::{DeclarationKind, LocalId};

    use super::*;

    const ALIASES: &str = r#"function inspect(flag: bool) returns list[int64]:
    list[int64] source = list(1)
    list[int64] borrowed = view source
    list[int64] forwarded = borrowed
    if flag:
        trace forwarded
    return clone forwarded
"#;

    fn lower_source(source: &str) -> (Program, TypeInterner) {
        let file = FileId::new(0);
        let parsed = jett_parser::parse(source, file);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let resolved = jett_resolve::resolve(&parsed.module);
        let checked = jett_typecheck::check(&parsed.module, &resolved);
        assert!(
            checked
                .diagnostics
                .iter()
                .all(|diagnostic| { diagnostic.severity != jett_diagnostics::Severity::Error }),
            "{:?}",
            checked.diagnostics
        );
        let hir = jett_hir::lower(
            &parsed.module,
            &resolved,
            &checked,
            &HashMap::from([(file, SourceOrigin::Project)]),
        )
        .expect("HIR lowering");
        let mir = crate::lower(&hir, &checked.interner).expect("MIR lowering");
        (mir, checked.interner)
    }

    fn inspected(program: &Program) -> usize {
        program
            .functions
            .iter()
            .position(|function| function.identity.declaration.name == "inspect")
            .unwrap()
    }

    fn local_named(function: &Function, name: &str) -> LocalId {
        function
            .locals
            .iter()
            .find(|local| local.name == name)
            .unwrap()
            .id
    }

    fn local_expression(function: &Function, local: LocalId) -> Expression {
        let metadata = function.local(local).unwrap();
        Expression {
            kind: ExpressionKind::Local(local),
            ty: metadata.ty,
            span: metadata.span,
        }
    }

    fn replace_return(function: &mut Function, value: Expression) {
        for block in &mut function.blocks {
            if matches!(block.terminator.kind, TerminatorKind::Return(Some(_))) {
                block.terminator.kind = TerminatorKind::Return(Some(value.clone()));
            }
        }
    }

    #[test]
    fn local_view_alias_plan_keeps_transitive_origins_live_across_cfg_and_debug_reads() {
        let (mut program, types) = lower_source(ALIASES);
        let index = inspected(&program);
        let function = &mut program.functions[index];
        let source = local_named(function, "source");
        let borrowed = local_named(function, "borrowed");
        let forwarded = local_named(function, "forwarded");
        let debug_block = function
            .blocks
            .iter_mut()
            .find(|block| {
                block
                    .statements
                    .iter()
                    .any(|statement| matches!(statement.kind, StatementKind::Trace(_)))
            })
            .unwrap();
        debug_block.statements.push(crate::Statement {
            kind: StatementKind::Breakpoint {
                condition: None,
                bindings: vec![forwarded],
            },
            span: debug_block.terminator.span,
        });
        let function = &program.functions[index];
        let plan = MoveValuePlan::analyze(&program, function, &types).unwrap();
        assert!(plan.owned_locals.contains(&(source.index() as usize)));
        assert!(!plan.owned_locals.contains(&(borrowed.index() as usize)));
        assert!(!plan.owned_locals.contains(&(forwarded.index() as usize)));
        let mut observed = false;
        for live in plan.live_in.iter().chain(&plan.live_out).chain(
            plan.live_after_statement
                .iter()
                .flat_map(|block| block.iter()),
        ) {
            if live.contains(&(forwarded.index() as usize)) {
                observed = true;
                assert!(live.contains(&(borrowed.index() as usize)));
                assert!(live.contains(&(source.index() as usize)));
            }
        }
        assert!(observed);
    }

    #[test]
    fn local_view_alias_plan_rejects_invalid_origins_and_initializer_backing() {
        for invalid in [
            "missing",
            "cycle",
            "different initializer",
            "allocating initializer",
        ] {
            let (mut program, types) = lower_source(ALIASES);
            let index = inspected(&program);
            let function = &mut program.functions[index];
            let source = local_named(function, "source");
            let borrowed = local_named(function, "borrowed");
            let forwarded = local_named(function, "forwarded");
            match invalid {
                "missing" => {
                    function.locals[borrowed.index() as usize].view_source =
                        Some(LocalId::new(u32::MAX))
                }
                "cycle" => function.locals[borrowed.index() as usize].view_source = Some(forwarded),
                _ => {
                    let replacement = local_expression(function, source);
                    let value = function
                        .blocks
                        .iter_mut()
                        .flat_map(|block| &mut block.statements)
                        .find_map(|statement| match &mut statement.kind {
                            StatementKind::Let { local, value } if *local == forwarded => {
                                Some(value)
                            }
                            _ => None,
                        })
                        .unwrap();
                    *value = if invalid == "allocating initializer" {
                        Expression {
                            kind: ExpressionKind::Clone(Box::new(replacement.clone())),
                            ..replacement
                        }
                    } else {
                        replacement
                    };
                }
            }
            let error =
                MoveValuePlan::analyze(&program, &program.functions[index], &types).unwrap_err();
            assert!(
                error.contains("origin") || error.contains("initializer"),
                "{invalid}: {error}"
            );
        }
    }

    #[test]
    fn local_view_alias_plan_rejects_owned_escape_before_test_copy_exceptions() {
        for kind in [
            DeclarationKind::Function,
            DeclarationKind::Verify,
            DeclarationKind::Property,
        ] {
            let (mut program, types) = lower_source(ALIASES);
            let index = inspected(&program);
            let function = &mut program.functions[index];
            let borrowed = local_named(function, "forwarded");
            let value = local_expression(function, borrowed);
            replace_return(function, value);
            function.identity.declaration.kind = kind;
            let error =
                MoveValuePlan::analyze(&program, &program.functions[index], &types).unwrap_err();
            assert!(
                error.contains("cannot move borrowed native place"),
                "{kind:?}: {error}"
            );
        }
    }

    #[test]
    fn local_view_alias_plan_rejects_wrapped_qualified_string_owned_escapes() {
        for ty in ["secret[string]", "Label"] {
            let source = format!(
                "type Label = string where true\nfunction consume(value: {ty}) returns nothing:\n    return nothing\nfunction inspect(view source: {ty}) returns {ty}:\n    {ty} borrowed = view source\n    return clone borrowed\n"
            );
            for kind in [
                DeclarationKind::Function,
                DeclarationKind::Verify,
                DeclarationKind::Property,
            ] {
                for call in [false, true] {
                    let (mut program, types) = lower_source(&source);
                    let index = inspected(&program);
                    let consume = program
                        .functions
                        .iter()
                        .find(|function| function.identity.declaration.name == "consume")
                        .unwrap()
                        .id;
                    let function = &program.functions[index];
                    MoveValuePlan::analyze(&program, function, &types).unwrap();
                    let borrowed = local_named(function, "borrowed");
                    let local = local_expression(function, borrowed);
                    assert!(is_string(&types, local.ty));
                    assert_eq!(
                        wrapper_copies_result(&types, local.ty),
                        jett_typecheck::ownership::is_implicitly_copyable(&types, local.ty)
                    );
                    assert!(!wrapper_copies_result(&types, local.ty));
                    let view = Expression {
                        kind: ExpressionKind::View(Box::new(local.clone())),
                        ..local
                    };
                    let function = &mut program.functions[index];
                    function.identity.declaration.kind = kind;
                    if call {
                        let value = Expression {
                            span: view.span,
                            ty: TypeInterner::NOTHING,
                            kind: ExpressionKind::Call {
                                function: consume,
                                args: vec![view],
                                evaluation_order: vec![0],
                            },
                        };
                        let exit = function
                            .blocks
                            .iter_mut()
                            .find(|block| {
                                matches!(block.terminator.kind, TerminatorKind::Return(Some(_)))
                            })
                            .unwrap();
                        exit.statements.push(crate::Statement {
                            span: value.span,
                            kind: StatementKind::Evaluate(value),
                        });
                    } else {
                        replace_return(function, view);
                    }
                    let error = MoveValuePlan::analyze(&program, &program.functions[index], &types)
                        .unwrap_err();
                    assert!(
                        error.contains("native view cannot escape into an owning value"),
                        "{ty}, {kind:?}, call={call}: {error}"
                    );
                }
            }
        }
    }

    #[test]
    fn local_view_alias_plan_preserves_explicit_primitive_and_string_copies() {
        let (program, types) = lower_source(
            r#"function inspect(view number: int64, view text: string) returns string:
    int64 number_copy = view number
    string text_copy = view text
    return text_copy
"#,
        );
        let function = &program.functions[inspected(&program)];
        let plan = MoveValuePlan::analyze(&program, function, &types).unwrap();
        for name in ["number_copy", "text_copy"] {
            let local = function.local(local_named(function, name)).unwrap();
            assert!(local.view_source.is_none());
            assert_eq!(
                wrapper_copies_result(&types, local.ty),
                jett_typecheck::ownership::is_implicitly_copyable(&types, local.ty)
            );
            assert!(wrapper_copies_result(&types, local.ty));
        }
        assert!(
            plan.owned_locals
                .contains(&(local_named(function, "text_copy").index() as usize))
        );
    }

    #[test]
    fn local_view_alias_plan_borrows_callback_descriptors_and_rejects_their_owned_escape() {
        let source = r#"function increment(value: int64) returns int64:
    return value + 1
function inspect() returns function(int64) returns int64:
    function(int64) returns int64 source = increment
    function(int64) returns int64 borrowed = view source
    function(int64) returns int64 copied = clone borrowed
    int64 called = borrowed(3)
    return copied
"#;
        for kind in [
            DeclarationKind::Function,
            DeclarationKind::Verify,
            DeclarationKind::Property,
        ] {
            let (mut program, types) = lower_source(source);
            let index = inspected(&program);
            let function = &program.functions[index];
            let borrowed = local_named(function, "borrowed");
            let plan = MoveValuePlan::analyze(&program, function, &types).unwrap();
            assert!(!plan.owned_locals.contains(&(borrowed.index() as usize)));
            assert!(
                plan.owned_locals
                    .contains(&(local_named(function, "source").index() as usize))
            );
            assert!(
                plan.owned_locals
                    .contains(&(local_named(function, "copied").index() as usize))
            );
            let value = local_expression(function, borrowed);
            let function = &mut program.functions[index];
            replace_return(function, value);
            function.identity.declaration.kind = kind;
            let error =
                MoveValuePlan::analyze(&program, &program.functions[index], &types).unwrap_err();
            assert!(
                error.contains("cannot move borrowed native place"),
                "{kind:?}: {error}"
            );
        }
    }

    #[test]
    fn local_view_alias_plan_limits_owner_changes_without_selecting_alias_expiry() {
        for rebind in [false, true] {
            let (mut program, types) = lower_source(ALIASES);
            let index = inspected(&program);
            let function = &mut program.functions[index];
            let source = local_named(function, "source");
            let value = local_expression(function, source);
            if rebind {
                let borrowed = local_named(function, "borrowed");
                let copied = local_expression(function, borrowed);
                let assignment = crate::Statement {
                    span: value.span,
                    kind: StatementKind::Assign {
                        target: value,
                        value: Expression {
                            kind: ExpressionKind::Clone(Box::new(copied.clone())),
                            ..copied
                        },
                    },
                };
                let exit = function
                    .blocks
                    .iter_mut()
                    .find(|block| matches!(block.terminator.kind, TerminatorKind::Return(Some(_))))
                    .unwrap();
                exit.statements.push(assignment);
            } else {
                replace_return(function, value);
            }
            let error =
                MoveValuePlan::analyze(&program, &program.functions[index], &types).unwrap_err();
            assert!(
                error.contains("after creating a local view alias is not implemented"),
                "{rebind}: {error}"
            );
        }
        let (program, types) = lower_source(
            r#"function inspect(flag: bool) returns list[int64]:
    list[int64] source = list(1)
    if flag:
        return source
    list[int64] borrowed = view source
    return clone borrowed
"#,
        );
        MoveValuePlan::analyze(&program, &program.functions[inspected(&program)], &types).unwrap();
    }
}
