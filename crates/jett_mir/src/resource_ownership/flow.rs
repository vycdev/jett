//! Independent current-CFG occupancy proof. Abstract states are metadata,
//! never runtime tokens and never an ordinary linearity classifier.
use super::*;
mod normalized;
use hir::ExpressionKind as E;
use jett_typecheck::{
    CheckedCalleeAccess as Access, CheckedCallerEffect as Effect, CheckedCallerSyntax as Syntax,
};
use normalized::{ActiveCall, PreparedCall};
use std::collections::VecDeque;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Value {
    shape: ResourceShape,
    occupancy: ResourceOccupancy,
    owner: Option<ResourceOwnerSlotId>,
    loan: Option<ResourceLoanId>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct State {
    values: Vec<Option<Value>>,
    moved: Vec<bool>,
    descriptors: Vec<Option<hir::ResourceHookRef>>,
    named_callables: Vec<Option<usize>>,
    tags: BTreeMap<u32, LocalId>,
    guards: BTreeMap<u32, (LocalId, bool)>,
    leases: BTreeSet<ResourceLoanId>,
    aborted: bool,
    calls: Vec<ActiveCall>,
}
type Key = (u32, u8, usize, usize);
fn key(site: ResourceSite, ordinal: usize) -> Key {
    match site.position {
        ResourcePosition::Statement(index) => (site.block.index(), 0, index, ordinal),
        ResourcePosition::Terminator => (site.block.index(), 1, 0, ordinal),
    }
}
struct Analysis<'p> {
    program: &'p Program,
    function: &'p Function,
    types: &'p TypeInterner,
    plan: ResourceFunctionPlan,
    local_slots: Vec<Option<ResourceOwnerSlotId>>,
    expression_slots: BTreeMap<(Key, Option<usize>), ResourceOwnerSlotId>,
    operation_frames: BTreeMap<Key, ResourceFrameId>,
    loan_sites: BTreeMap<(Key, usize, ResourceLoanSource), ResourceLoanId>,
    site: ResourceSite,
    ordinal: usize,
    expression: Option<usize>,
    emit: bool,
    active_frame: ResourceFrameId,
    return_frame: Option<ResourceFrameId>,
    normalized_calls: BTreeMap<ResourceCallRegionId, PreparedCall>,
}
impl<'p> Analysis<'p> {
    fn record_execution_frame(&mut self) -> Result<(), String> {
        if self.emit {
            if self
                .plan
                .execution_frames
                .iter()
                .any(|(site, _)| *site == self.site)
            {
                return Err("Resource execution frame has duplicate current-site authority".into());
            }
            self.plan
                .execution_frames
                .push((self.site, self.active_frame));
        }
        Ok(())
    }
    fn operation(&mut self, frame: ResourceFrameId, role: ResourceOperationRole) {
        if self.emit {
            let id = ResourceOperationId(self.plan.operations.len());
            self.plan.operations.push(ResourceOperation {
                id,
                frame,
                site: self.site,
                ordinal: self.ordinal,
                role,
                expression: self.expression,
                named_indirect: None,
                indirect_hook: None,
            });
        }
    }
    fn frame(&mut self, ordinal: usize, role: ResourceFrameRole) -> ResourceFrameId {
        let entry = key(self.site, ordinal);
        if let Some(id) = self.operation_frames.get(&entry) {
            return *id;
        }
        let id = ResourceFrameId(self.plan.frames.len());
        self.plan.frames.push(ResourceFrame {
            id,
            role,
            site: self.site,
            parent: Some(self.active_frame),
            ordinal,
        });
        self.operation_frames.insert(entry, id);
        id
    }
    fn result_slot(
        &mut self,
        ordinal: usize,
        shape: ResourceShape,
        frame: ResourceFrameId,
    ) -> ResourceOwnerSlotId {
        let entry = (key(self.site, ordinal), None);
        if let Some(id) = self.expression_slots.get(&entry) {
            return *id;
        }
        let id = ResourceOwnerSlotId(self.plan.slots.len());
        self.plan.slots.push(ResourceOwnerSlot {
            id,
            frame,
            shape,
            storage: ResourceSlotStorage::Expression {
                site: self.site,
                ordinal,
            },
        });
        self.expression_slots.insert(entry, id);
        id
    }
    fn argument_slot(
        &mut self,
        ordinal: usize,
        parameter: usize,
        shape: ResourceShape,
        frame: ResourceFrameId,
    ) -> ResourceOwnerSlotId {
        let entry = (key(self.site, ordinal), Some(parameter));
        if let Some(id) = self.expression_slots.get(&entry) {
            return *id;
        }
        let id = ResourceOwnerSlotId(self.plan.slots.len());
        self.plan.slots.push(ResourceOwnerSlot {
            id,
            frame,
            shape,
            storage: ResourceSlotStorage::Argument {
                site: self.site,
                call_ordinal: ordinal,
                parameter,
            },
        });
        self.expression_slots.insert(entry, id);
        id
    }
    fn loan(
        &mut self,
        ordinal: usize,
        parameter: usize,
        frame: ResourceFrameId,
        source: ResourceLoanSource,
    ) -> ResourceLoanId {
        let entry = (key(self.site, ordinal), parameter, source);
        if let Some(id) = self.loan_sites.get(&entry) {
            return *id;
        }
        let id = ResourceLoanId(self.plan.loans.len());
        let shape = match source {
            ResourceLoanSource::Owner(owner) => self.plan.slots[owner.index()].shape.clone(),
            ResourceLoanSource::ProjectedSumPayload { parent, .. } => ResourceShape::Plain {
                kind: self.plan.loans[parent.index()].shape.kind().clone(),
            },
            ResourceLoanSource::IncomingViewFormal { .. } => {
                unreachable!("incoming formal loans are initialized from their exact typed header")
            }
        };
        self.plan.loans.push(ResourceLoan {
            id,
            frame,
            source,
            shape,
            parameter: (parameter != usize::MAX).then_some(parameter),
        });
        self.loan_sites.insert(entry, id);
        id
    }
    fn borrow(&mut self, frame: ResourceFrameId, loan: ResourceLoanId) {
        let role = if self.plan.loans[loan.index()].shape.conditional() {
            ResourceOperationRole::BorrowSum { loan }
        } else {
            ResourceOperationRole::Borrow { loan }
        };
        self.operation(frame, role);
    }
    fn end_borrow(&mut self, frame: ResourceFrameId, loan: ResourceLoanId) {
        let role = if self.plan.loans[loan.index()].shape.conditional() {
            ResourceOperationRole::EndSumBorrow { loan }
        } else {
            ResourceOperationRole::EndBorrow { loan }
        };
        self.operation(frame, role);
    }
    fn clear(&self, state: &mut State, local: LocalId) {
        state
            .tags
            .retain(|target, source| *target != local.index() && *source != local);
        state.guards.remove(&local.index());
        state.descriptors[local.index() as usize] = None;
        state.named_callables[local.index() as usize] = None;
    }
    fn descriptor_value(&self, state: &State, value: &Expression) -> Option<hir::ResourceHookRef> {
        let expected = self.plan.descriptor_value(value)?;
        match &value.kind {
            E::Local(local) => (state.descriptors[local.index() as usize].as_ref()
                == Some(expected))
            .then(|| expected.clone()),
            E::Clone(inner) => (self.descriptor_value(state, inner).as_ref() == Some(expected))
                .then(|| expected.clone()),
            E::Call { .. } | E::ResourceHookValue { .. } => Some(expected.clone()),
            _ => None,
        }
    }
    fn named_binding(&self, state: &State, local: LocalId, value: &Expression) -> Option<usize> {
        let witness = self.function.resource_lowering.as_ref()?;
        witness
            .named_callables
            .iter()
            .enumerate()
            .find_map(|(index, binding)| {
                if binding.local() != local
                    || !crate::breakpoint_regions::expressions_equal(binding.initializer(), value)
                {
                    return None;
                }
                if let Some(parent) = binding.parent() {
                    let parent_local = witness.named_callables[parent].local();
                    (state.named_callables[parent_local.index() as usize] == Some(parent))
                        .then_some(index)
                } else {
                    matches!(value.kind, E::FunctionRef(_)).then_some(index)
                }
            })
    }
    fn named_indirect_target(
        &self,
        state: &State,
        expression: &Expression,
    ) -> Result<Option<ResourceNamedCallableProof>, String> {
        let E::IndirectCall {
            callee,
            ownership: hir::CallOwnership::Source(source),
            ..
        } = &expression.kind
        else {
            return Ok(None);
        };
        let E::Local(local) = callee.kind else {
            return Ok(None);
        };
        let Some(index) = state.named_callables[local.index() as usize] else {
            return Ok(None);
        };
        let witness = self
            .function
            .resource_lowering
            .as_ref()
            .ok_or("Resource named call lost its constructor witness")?;
        let binding = witness
            .named_callables
            .get(index)
            .ok_or("Resource named call has no exact private producer")?;
        let proof = binding.proof();
        let association = witness
            .calls
            .iter()
            .find(|association| {
                crate::breakpoint_regions::expressions_equal(&association.current, expression)
            })
            .ok_or("Resource named call has no exact original Indirect occurrence")?;
        let E::IndirectCall {
            callee: original_callee,
            ..
        } = &association.original.kind
        else {
            return Err("Resource named call changed its original invocation kind".into());
        };
        if binding.local() != local
            || !binding.original_callee_matches(original_callee)
            || callee.ty != proof.signature_type()
            || source.target
                != (hir::CallTarget::Indirect {
                    signature_type: proof.signature_type(),
                })
            || source.bridge != hir::CallBridge::Direct
            || !source.generated_operands.is_empty()
        {
            return Err("Resource named call differs from its exact immutable callee or original Source signature".into());
        }
        let target = self
            .program
            .functions
            .get(proof.function().index() as usize)
            .filter(|target| target.id == proof.function())
            .ok_or("Resource named call lost its selected declaration")?;
        let selected = target
            .resource_lowering
            .as_ref()
            .ok_or("Resource named call has no selected original Source body")?;
        if selected.source != witness.source || target.identity != *proof.identity() {
            return Err("Resource named call selected a foreign archive or declaration".into());
        }
        Ok(Some(proof.clone()))
    }
    fn read(
        &self,
        state: &mut State,
        local: LocalId,
        taking: bool,
    ) -> Result<Option<Value>, String> {
        let index = local.index() as usize;
        let metadata = self
            .function
            .local(local)
            .ok_or("Resource read has no exact dense local header")?;
        if shape(&self.program.resource_manifest, self.types, metadata.ty)?.is_none() {
            return Ok(None);
        }
        let value = state.values[index]
            .as_ref()
            .ok_or("Resource owner is uninitialized, moved or retired at this current CFG site")?
            .clone();
        if taking {
            if value.loan.is_some() {
                return Err(
                    "Resource borrowed formal or alias cannot become an owned value".into(),
                );
            }
            let owner = value.owner;
            if state.leases.iter().any(|loan| {
                Some(self.plan.loans[loan.0].source) == owner.map(ResourceLoanSource::Owner)
            }) {
                return Err("Resource owner cannot move, close or replace while its exact lease remains live".into());
            }
            state.values[index] = None;
            state.moved[index] = true;
            self.clear(state, local);
        }
        Ok(Some(value))
    }
    fn store(
        &mut self,
        state: &mut State,
        local: LocalId,
        value: Option<Value>,
        replace: bool,
        self_rebind: bool,
    ) -> Result<(), String> {
        let index = local.index() as usize;
        let metadata = self
            .function
            .local(local)
            .ok_or("Resource destination has no exact local header")?;
        let expected = shape(&self.program.resource_manifest, self.types, metadata.ty)?;
        let Some(expected) = expected else {
            if value.is_some() {
                return Err("Resource custody cannot be stored in an ordinary destination".into());
            }
            self.clear(state, local);
            return Ok(());
        };
        let mut value =
            value.ok_or("Resource destination has no checked owning or resident-loan producer")?;
        if value.shape != expected {
            return Err("Resource destination changes its nominal kind or active shape".into());
        }
        if self.function.is_view_local(local) {
            if replace {
                return Err(
                    "Resource view alias replacement requires its separate lease transport".into(),
                );
            }
            if !value.shape.conditional() && value.occupancy != ResourceOccupancy::Occupied {
                return Err("pending ResourceOwnershipPlan: conditional sum borrowing needs exact occupied-arm lease transport".into());
            }
            if value.loan.is_none() {
                let source = value
                    .owner
                    .ok_or("Resource alias has no exact owned backing")?;
                let loan = self.loan(
                    self.ordinal,
                    usize::MAX,
                    ResourceFrameId(0),
                    ResourceLoanSource::Owner(source),
                );
                state.leases.insert(loan);
                self.borrow(ResourceFrameId(0), loan);
                value.loan = Some(loan);
            }
            state.values[index] = Some(value);
            state.moved[index] = false;
            self.clear(state, local);
            return Ok(());
        }
        if value.loan.is_some() {
            return Err("Resource owning storage cannot adopt a borrowed carrier".into());
        }
        let destination = self.local_slots[index]
            .ok_or("Resource owning destination lacks its dedicated slot")?;
        if replace && !metadata.mutable {
            return Err("Resource assignment requires its exact mutable owning header".into());
        }
        if let Some(old) = state.values[index].as_ref() {
            if !replace || !metadata.mutable {
                return Err(
                    "Resource live destination requires its exact mutable replacement constructor"
                        .into(),
                );
            }
            if state.leases.iter().any(|loan| {
                self.plan.loans[loan.0].source == ResourceLoanSource::Owner(destination)
            }) {
                return Err("Resource replacement has a live old-holder lease".into());
            }
            self.operation(
                ResourceFrameId(0),
                ResourceOperationRole::Replace {
                    destination,
                    replacement: value
                        .owner
                        .ok_or("Resource replacement lacks its evaluated RHS owner slot")?,
                    old: old.occupancy,
                },
            );
            self.operation(
                ResourceFrameId(0),
                ResourceOperationRole::Drop {
                    source: destination,
                    occupancy: old.occupancy,
                },
            );
        }
        if let Some(source) = value.owner {
            if source != destination {
                self.operation(
                    self.active_frame,
                    ResourceOperationRole::Transfer {
                        source,
                        destination,
                    },
                );
            } else if replace {
                if !self_rebind
                    || !matches!(value.shape, ResourceShape::Plain { .. })
                    || value.occupancy != ResourceOccupancy::Occupied
                    || state.values[index].is_some()
                {
                    return Err("pending ResourceOwnershipPlan: self-rebind needs the exact occupied plain Local reseat".into());
                }
                self.operation(
                    ResourceFrameId(0),
                    ResourceOperationRole::SelfRebind { slot: destination },
                );
            }
        }
        value.owner = Some(destination);
        state.values[index] = Some(value);
        state.moved[index] = false;
        self.clear(state, local);
        Ok(())
    }
    fn finish(&mut self, state: &mut State, frame: ResourceFrameId, outcome: ResourceCompletion) {
        // Incoming formal records denote resident caller leases, not child-owned tokens.
        let loans = state.leases.iter().copied().collect::<Vec<_>>();
        for loan in loans.into_iter().rev() {
            self.end_borrow(frame, loan);
            state.leases.remove(&loan);
        }
        for value in state.values.iter_mut().rev() {
            if let Some(value) = value.take() {
                if value.loan.is_none()
                    && let Some(source) = value.owner
                {
                    self.operation(
                        frame,
                        ResourceOperationRole::Drop {
                            source,
                            occupancy: value.occupancy,
                        },
                    );
                }
            }
        }
        self.operation(frame, ResourceOperationRole::Complete { outcome });
    }
    fn lexical_exit(&mut self, state: &mut State) -> Result<(), String> {
        let Some(exit) = self
            .function
            .resource_lexical_exit(self.site.block, self.site.position)?
        else {
            return Err("Resource lexical retirement has no exact constructor exit".into());
        };
        let witness = self
            .function
            .resource_lowering
            .as_ref()
            .ok_or("Resource lexical retirement has no Source witness")?;
        let mut clear_locals = Vec::new();
        let mut loans = BTreeSet::new();
        for original in exit.declarations() {
            let row = witness
                .borrowed_sums
                .iter()
                .find(|row| row.original_local() == original)
                .ok_or("Resource lexical retirement lost an exact original alias")?;
            for local in row.retirement_locals() {
                if !clear_locals.contains(&local) {
                    clear_locals.push(local);
                }
                if let Some(value) = &state.values[local.index() as usize]
                    && let Some(loan) = value.loan
                    && state.leases.contains(&loan)
                {
                    // A forwarded incoming/outer lease is never adopted by this declaration.
                    let same_site = |site: Key, current: ResourceSite| {
                        let expected = key(current, 0);
                        (site.0, site.1, site.2) == (expected.0, expected.1, expected.2)
                    };
                    let created_here = self.loan_sites.iter().any(|((site, _, _), id)| {
                        *id == loan
                            && (same_site(*site, row.initialize_site())
                                || same_site(*site, row.success_site()))
                    });
                    if created_here {
                        loans.insert(loan);
                    }
                }
            }
        }
        let start = self.plan.operations.len();
        for loan in loans.into_iter().rev() {
            if state.leases.iter().any(|child| matches!(self.plan.loans[child.index()].source, ResourceLoanSource::ProjectedSumPayload { parent, .. } if parent == loan)) {
                return Err("Resource lexical parent retirement has a live child outside its exact exit".into());
            }
            self.end_borrow(self.active_frame, loan);
            state.leases.remove(&loan);
        }
        for local in &clear_locals {
            state.values[local.index() as usize] = None;
            state.moved[local.index() as usize] = false;
            self.clear(state, *local);
        }
        if self.emit {
            self.plan.lexical_exits.push(ResourceLexicalExitPlan::new(
                self.site,
                exit.kind(),
                (start..self.plan.operations.len())
                    .map(ResourceOperationId)
                    .collect(),
                clear_locals,
            ));
        }
        Ok(())
    }
    fn expression(
        &mut self,
        state: &mut State,
        expression: &Expression,
        taking: bool,
    ) -> Result<Option<Value>, String> {
        let previous = self
            .expression
            .replace(std::ptr::from_ref(expression).addr());
        let result = self.expression_inner(state, expression, taking);
        self.expression = previous;
        result
    }
    fn expression_inner(
        &mut self,
        state: &mut State,
        expression: &Expression,
        taking: bool,
    ) -> Result<Option<Value>, String> {
        if state.aborted {
            return Ok(None);
        }
        self.ordinal += 1;
        let ordinal = self.ordinal;
        let expected = shape(&self.program.resource_manifest, self.types, expression.ty)?;
        match &expression.kind {
            E::Local(local) => {
                if self.plan.descriptor_value(expression).is_some() && self.descriptor_value(state, expression).is_none() {
                    return Err("Resource descriptor has no live exact producer at this current CFG read".into());
                }
                self.read(state, *local, taking)
            }
            E::Clone(inner) if self.plan.descriptor_value(expression).is_some() => {
                if self.descriptor_value(state, expression).is_none() { return Err("Resource descriptor alias lost its current exact producer".into()); }
                self.expression(state, inner, false)
            }
            E::View(inner) => {
                if taking && expected.is_some() { return Err("Resource View cannot be adopted as an owned result".into()); }
                self.expression(state, inner, false)
            }
            E::ResourceHookValue { hook } => {
                if !self.program.resource_manifest.contains_hook(hook) || expression.ty != hook.function_type() { return Err("Resource descriptor differs from its exact original manifest signature".into()); }
                self.operation(ResourceFrameId(0), ResourceOperationRole::Descriptor { hook: hook.clone() });
                Ok(None)
            }
            E::ResourceInvoke { hook, args, evaluation_order, ownership } => self.invocation(state, expression, Some(hook.clone()), None, args, evaluation_order, ownership, ordinal, None),
            E::Call { function, args, evaluation_order, ownership } => self.invocation(state, expression, None, Some(*function), args, evaluation_order, ownership, ordinal, None),
            E::IndirectCall { callee, args, evaluation_order, ownership } => {
                let hook = self.descriptor_value(state, callee);
                if expected.is_some() || args.iter().any(|arg| resource_type_pending(self.types, arg.ty)) {
                    if let Some(hook) = hook {
                        if callee.ty != hook.function_type() { return Err("Resource indirect descriptor has a different exact signature".into()); }
                        self.invocation(state, expression, Some(hook), None, args, evaluation_order, ownership, ordinal, None)
                    } else if let Some(proof) = self.named_indirect_target(state, expression)? {
                        self.invocation(state, expression, None, Some(proof.function()), args, evaluation_order, ownership, ordinal, Some(proof))
                    } else {
                        Err("pending ResourceOwnershipPlan: indirect custody call lacks an exact descriptor producer".into())
                    }
                } else {
                    for &index in evaluation_order { self.expression(state, &args[index], true)?; }
                    if !state.aborted { self.expression(state, callee, false)?; }
                    Ok(None)
                }
            }
            E::OptionalNone if expected.is_some() => {
                let shape = expected.unwrap();
                if !matches!(shape, ResourceShape::Optional { .. }) { return Err("Resource absence is not its exact Optional shape".into()); }
                let destination = self.result_slot(ordinal, shape.clone(), self.active_frame);
                self.operation(self.active_frame, ResourceOperationRole::CreateAbsentSum { destination });
                Ok(Some(Value { shape, occupancy: ResourceOccupancy::Empty, owner: Some(destination), loan: None }))
            },
            E::OptionalSome(inner) | E::ResultOk(inner) if expected.is_some() => {
                let payload = self.expression(state, inner, true)?;
                if state.aborted { return Ok(None); }
                let mut value = payload.ok_or("Resource sum payload has no exact owning producer")?;
                if value.loan.is_some() || value.shape.kind() != expected.as_ref().unwrap().kind() { return Err("Resource sum payload is borrowed or from another nominal kind".into()); }
                let shape = expected.unwrap();
                let destination = self.result_slot(ordinal, shape.clone(), self.active_frame);
                let source = value.owner.ok_or("Resource sum adoption has no sole owning payload")?;
                self.operation(self.active_frame, ResourceOperationRole::SumAdopt { source, destination });
                value.shape = shape;
                value.owner = Some(destination);
                Ok(Some(value))
            }
            E::ResultFail(inner) if expected.is_some() => {
                if self.expression(state, inner, true)?.is_some() { return Err("Resource result failure companion cannot carry custody".into()); }
                if state.aborted { return Ok(None); }
                let shape = expected.unwrap();
                let ResourceShape::Result { failure, .. } = &shape else { return Err("Resource failure is not its exact Result shape".into()); };
                if inner.ty != *failure { return Err("Resource failure companion changes its declared exact type".into()); }
                let destination = self.result_slot(ordinal, shape.clone(), self.active_frame);
                self.operation(self.active_frame, ResourceOperationRole::CreateFailureSum { destination, failure: *failure });
                Ok(Some(Value { shape, occupancy: ResourceOccupancy::Empty, owner: Some(destination), loan: None }))
            }
            E::RuntimeFailure(_) | E::RuntimeFailureMessage(_) => {
                if let E::RuntimeFailureMessage(message) = &expression.kind { self.expression(state, message, false)?; }
                state.aborted = true;
                Ok(None)
            }
            _ if expected.is_some() => Err("pending ResourceOwnershipPlan: Resource expression needs its dedicated canonical producer or conversion transport".into()),
            _ => {
                let mut has_resource = false;
                walk::expression(expression, &mut |value| {
                    if !std::ptr::eq(value, expression) {
                        has_resource |= resource_type_pending(self.types, value.ty) && !matches!(self.types.resolve(value.ty), Type::Function { .. });
                        if let E::Call { function, .. } = value.kind {
                            has_resource |= self.program.functions.get(function.index() as usize).is_some_and(|callee| callee.id == function && has_execution_records(callee, self.types));
                        }
                    }
                });
                if has_resource { return Err("pending ResourceOwnershipPlan: nested Resource evaluation requires exact source-order canonical staging".into()); }
                Ok(None)
            }
        }
    }
    fn invocation(
        &mut self,
        state: &mut State,
        expression: &Expression,
        hook: Option<hir::ResourceHookRef>,
        function: Option<FunctionId>,
        args: &[Expression],
        order: &[usize],
        ownership: &hir::CallOwnership,
        ordinal: usize,
        named_indirect: Option<ResourceNamedCallableProof>,
    ) -> Result<Option<Value>, String> {
        let resource = hook.is_some()
            || function.is_some_and(|id| {
                self.program
                    .functions
                    .get(id.index() as usize)
                    .is_some_and(|callee| {
                        callee.id == id && has_execution_records(callee, self.types)
                    })
            })
            || shape(&self.program.resource_manifest, self.types, expression.ty)?.is_some()
            || args.iter().any(|arg| {
                shape(&self.program.resource_manifest, self.types, arg.ty)
                    .ok()
                    .flatten()
                    .is_some()
            });
        if !resource {
            for &index in order {
                self.expression(state, &args[index], true)?;
            }
            return Ok(None);
        }
        let hir::CallOwnership::Source(source) = ownership else {
            return Err("Resource invocation cannot use Generated authority".into());
        };
        if source.arguments.len() != args.len() || order.len() != args.len() {
            return Err("Resource invocation changed its original source tuple count".into());
        }
        if let Some(hook) = &hook {
            if !self.program.resource_manifest.contains_hook(hook) {
                return Err("Resource invocation hook belongs to another checked program".into());
            }
        }
        if let Some(function) = function {
            let callee = self
                .program
                .functions
                .get(function.index() as usize)
                .filter(|callee| callee.id == function)
                .ok_or("Resource Source callee has no exact function")?;
            if !has_execution_records(callee, self.types) {
                return Err("Resource Source callee lacks its original custody witness".into());
            }
        }
        let frame = self.frame(ordinal, ResourceFrameRole::Operation);
        let parent = self.active_frame;
        self.active_frame = frame;
        let mut operands = Vec::new();
        let mut loans = Vec::new();
        let mut temporary_owners = Vec::new();
        for &parameter in order {
            let arg = &args[parameter];
            let fact = source
                .arguments
                .iter()
                .find(|fact| fact.parameter_index == parameter)
                .ok_or("Resource actual has no exact original formal tuple")?;
            let Some(_) = shape(&self.program.resource_manifest, self.types, arg.ty)? else {
                self.expression(state, arg, true)?;
                operands.push(ResourceCallOperand::Ordinary {
                    parameter,
                    ty: arg.ty,
                });
                if state.aborted {
                    break;
                }
                continue;
            };
            if fact.staging != hir::ArgumentStaging::Original {
                return Err("pending ResourceOwnershipPlan: resource actual uses an ordinary staging record".into());
            }
            let retaining = fact.effect == Effect::RetainBorrow;
            if !retaining && !matches!(fact.effect, Effect::RelinquishOwned | Effect::TransferOwned)
            {
                return Err("Resource custody cannot use Copy or ObserveData".into());
            }
            let input = if !retaining
                && fact.syntax == Syntax::Bare
                && fact.physical_access == Access::View
            {
                if let E::View(inner) = &arg.kind {
                    inner.as_ref()
                } else {
                    arg
                }
            } else {
                arg
            };
            let evaluated = self.expression(state, input, !retaining)?;
            if state.aborted {
                break;
            }
            let mut value =
                evaluated.ok_or("Resource actual has no exact live owner or resident lease")?;
            if retaining {
                if fact.syntax != Syntax::WrittenView || fact.callee_access != Access::View {
                    return Err(
                        "Resource retained loan differs from written View/formal access".into(),
                    );
                }
                let loan = if let Some(loan) = value.loan {
                    self.operation(
                        frame,
                        ResourceOperationRole::BoundedBorrowUse { loan, parameter },
                    );
                    loan
                } else {
                    if !value.shape.conditional() && value.occupancy != ResourceOccupancy::Occupied
                    {
                        return Err("pending ResourceOwnershipPlan: sum View actual needs its occupied-arm loan transport".into());
                    }
                    let owner = value
                        .owner
                        .ok_or("Resource retained actual has no exact holder")?;
                    let loan =
                        self.loan(ordinal, parameter, frame, ResourceLoanSource::Owner(owner));
                    state.leases.insert(loan);
                    loans.push(loan);
                    self.borrow(frame, loan);
                    loan
                };
                operands.push(ResourceCallOperand::Borrowed { parameter, loan });
            } else {
                if value.loan.is_some() {
                    return Err("Resource owned actual cannot consume a resident loan".into());
                }
                let temporary = self.argument_slot(ordinal, parameter, value.shape.clone(), frame);
                if let Some(owner) = value.owner {
                    self.operation(
                        frame,
                        ResourceOperationRole::Transfer {
                            source: owner,
                            destination: temporary,
                        },
                    );
                }
                value.owner = Some(temporary);
                if fact.callee_access == Access::View {
                    if !value.shape.conditional() && value.occupancy != ResourceOccupancy::Occupied
                    {
                        return Err("pending ResourceOwnershipPlan: bare sum to View needs its dedicated occupied transport".into());
                    }
                    let loan = self.loan(
                        ordinal,
                        parameter,
                        frame,
                        ResourceLoanSource::Owner(temporary),
                    );
                    loans.push(loan);
                    self.borrow(frame, loan);
                    operands.push(ResourceCallOperand::Borrowed { parameter, loan });
                    temporary_owners.push((temporary, value.occupancy));
                } else {
                    operands.push(ResourceCallOperand::Owned {
                        parameter,
                        slot: temporary,
                    });
                }
            }
        }
        if !state.aborted
            && let Some(proof) = &named_indirect
        {
            let E::IndirectCall { callee, .. } = &expression.kind else {
                return Err(
                    "Resource named body proof is not attached to an original Indirect call".into(),
                );
            };
            self.expression(state, callee, false)?;
            if self.named_indirect_target(state, expression)?.as_ref() != Some(proof) {
                return Err(
                    "Resource named callee lost its exact provenance after actual evaluation"
                        .into(),
                );
            }
        }
        if !state.aborted
            && hook.is_some()
            && let E::IndirectCall { callee, .. } = &expression.kind
        {
            self.expression(state, callee, false)?;
            if !state.aborted && self.descriptor_value(state, callee).as_ref() != hook.as_ref() {
                return Err("Resource indirect hook lost its exact descriptor provenance after actual evaluation".into());
            }
            if source.target
                != (hir::CallTarget::Indirect {
                    signature_type: callee.ty,
                })
                || source.bridge != hir::CallBridge::Direct
                || !source.generated_operands.is_empty()
            {
                return Err(
                    "Resource indirect hook changed its original Indirect Source authority".into(),
                );
            }
        }
        let outcome_shape = shape(&self.program.resource_manifest, self.types, expression.ty)?;
        let output = outcome_shape
            .as_ref()
            .map(|shape| self.result_slot(ordinal, shape.clone(), parent));
        if !state.aborted {
            let result = match output {
                Some(slot) => ResourceCallResult::Owned { slot },
                None => ResourceCallResult::Ordinary { ty: expression.ty },
            };
            if let Some(hook) = hook {
                self.operation(
                    frame,
                    ResourceOperationRole::InvokeHook {
                        hook: hook.clone(),
                        source: source.clone(),
                        evaluation_order: order.to_vec(),
                        operands: operands.clone(),
                        result,
                    },
                );
                if self.emit && matches!(expression.kind, E::IndirectCall { .. }) {
                    self.plan
                        .operations
                        .last_mut()
                        .ok_or("Resource indirect hook lost its Invoke operation")?
                        .indirect_hook = Some(hook.clone());
                }
                match hook.recipe() {
                    jett_types::ResourceKernelRecipe::NetworkFactory => {
                        let destination = output
                            .ok_or("Resource factory has no exact success-bearing destination")?;
                        self.operation(frame, ResourceOperationRole::Acquire { hook, destination });
                    }
                    jett_types::ResourceKernelRecipe::Finalize => {
                        let owner = operands
                            .iter()
                            .find_map(|operand| {
                                if let ResourceCallOperand::Owned { slot, .. } = operand {
                                    Some(*slot)
                                } else {
                                    None
                                }
                            })
                            .ok_or("Resource Close has no consumed owning actual")?;
                        self.operation(
                            frame,
                            ResourceOperationRole::Close {
                                hook,
                                source: owner,
                            },
                        );
                    }
                    jett_types::ResourceKernelRecipe::NetworkBorrow => {}
                }
            } else {
                self.operation(
                    frame,
                    ResourceOperationRole::InvokeSourceFunction {
                        function: function
                            .ok_or("Resource call has neither an exact hook nor a Source callee")?,
                        formals: source
                            .arguments
                            .iter()
                            .map(ResourceCallFormal::original)
                            .collect(),
                        source: source.clone(),
                        evaluation_order: order.to_vec(),
                        operands,
                        result,
                    },
                );
                if self.emit
                    && let Some(proof) = named_indirect
                {
                    let invocation = self
                        .plan
                        .operations
                        .last_mut()
                        .ok_or("Resource named call has no invocation operation")?;
                    invocation.named_indirect = Some(proof);
                }
            }
        } else {
            // Every earlier owned actual must retain a cleanup destination even if the callee never runs.
            for operand in &operands {
                if let ResourceCallOperand::Owned { slot, .. } = operand {
                    temporary_owners.push((*slot, ResourceOccupancy::Occupied));
                }
            }
        }
        for loan in loans.into_iter().rev() {
            state.leases.remove(&loan);
            self.end_borrow(frame, loan);
        }
        for (source, occupancy) in temporary_owners.into_iter().rev() {
            self.operation(frame, ResourceOperationRole::Drop { source, occupancy });
        }
        self.operation(
            frame,
            ResourceOperationRole::Complete {
                outcome: if state.aborted {
                    ResourceCompletion::Abort
                } else {
                    ResourceCompletion::Normal
                },
            },
        );
        self.active_frame = parent;
        Ok(if state.aborted {
            None
        } else {
            outcome_shape.map(|shape| Value {
                occupancy: if shape.conditional() {
                    ResourceOccupancy::Conditional
                } else {
                    ResourceOccupancy::Occupied
                },
                shape,
                owner: output,
                loan: None,
            })
        })
    }
    fn block(
        &mut self,
        block: &BasicBlock,
        mut state: State,
    ) -> Result<Vec<(BlockId, State)>, String> {
        self.active_frame = state
            .calls
            .last()
            .map_or(ResourceFrameId(0), |active| active.frame);
        for (index, statement) in block.statements.iter().enumerate() {
            self.site = ResourceSite {
                function: self.function.id,
                block: block.id,
                position: ResourcePosition::Statement(index),
            };
            self.ordinal = 0;
            if state.aborted {
                break;
            }
            self.record_execution_frame()?;
            match &statement.kind {
                StatementKind::ResourceLexicalExit(_) => self.lexical_exit(&mut state)?,
                StatementKind::ResourceCall(node) => self.normalized_node(&mut state, node)?,
                StatementKind::Let { local, value } => {
                    let named = self.named_binding(&state, *local, value);
                    let descriptor = self.descriptor_value(&state, value);
                    let taking = !self.function.is_view_local(*local);
                    let result = self.expression(&mut state, value, taking)?;
                    if !state.aborted {
                        self.store(&mut state, *local, result, false, false)?;
                        let expected_descriptor = self.function.resource_lowering.as_ref().and_then(|witness| witness.descriptors.binding(*local, value));
                        if descriptor.as_ref() != expected_descriptor { return Err("Resource descriptor binding lost its exact original immutable initializer".into()); }
                        state.descriptors[local.index() as usize] = descriptor;
                        state.named_callables[local.index() as usize] = named;
                        if self.emit && let Some(index) = named
                            && matches!(value.kind, E::FunctionRef(_))
                        {
                            let proof = self.function.resource_lowering.as_ref()
                                .and_then(|witness| witness.named_callables.get(index))
                                .ok_or("Resource named producer lost its constructor record")?.proof().clone();
                            self.plan.named_callable_producers.push((std::ptr::from_ref(value).addr(), proof));
                        }
                    }
                }
                StatementKind::Assign { target, value } => {
                    let E::Local(local) = target.kind else { return Err("pending ResourceOwnershipPlan: projected replacement needs its aggregate transport".into()); };
                    let result = self.expression(&mut state, value, true)?;
                    if !state.aborted {
                        let self_rebind = matches!(value.kind, E::Local(source) if source == local)
                            && target.ty == value.ty
                            && self.function.local(local).map(|header| header.ty) == Some(value.ty);
                        self.store(&mut state, local, result, true, self_rebind)?;
                    }
                }
                StatementKind::Evaluate(value) | StatementKind::HandleDefault(value) => {
                    if let Some(value) = self.expression(&mut state, value, true)? {
                        if value.loan.is_some() { return Err("Resource borrowed result cannot be discarded as owned".into()); }
                        if let Some(source) = value.owner { self.operation(ResourceFrameId(0), ResourceOperationRole::Drop { source, occupancy: value.occupancy }); }
                    }
                }
                StatementKind::SumTag { source, target } => {
                    if let Some(value) = self.read(&mut state, *source, false)? {
                        if !value.shape.conditional() { return Err("Resource tag needs an exact supported sum".into()); }
                        if self.function.local(*target).map(|local| local.ty) != Some(TypeInterner::BOOL) { return Err("Resource sum tag is not its exact Bool destination".into()); }
                        if let Some(loan) = value.loan {
                            let row = self.function.resource_borrowed_sum_projection(self.site.block, self.site.position)?.ok_or("borrowed Resource sum tag lacks its original Handle constructor relation")?;
                            if row.source() != *source || row.tag() != *target { return Err("borrowed Resource sum tag changed its exact source/output headers".into()); }
                            self.operation(self.active_frame, ResourceOperationRole::ObserveSumView { source: loan, target: *target });
                        }
                        state.tags.insert(target.index(), *source);
                    }
                }
                StatementKind::SumTake { source, target, success } => {
                    if let Some(value) = self.read(&mut state, *source, false)? {
                        let (tag, selected) = state.guards.get(&source.index()).copied().ok_or("Resource SumTake has no exact selecting typed tag edge")?;
                        if selected != *success || (selected && value.occupancy != ResourceOccupancy::Occupied) || (!selected && value.occupancy != ResourceOccupancy::Empty) { return Err("Resource SumTake differs from its selected occupied arm".into()); }
                        if let Some(parent) = value.loan {
                            let row = self.function.resource_borrowed_sum_projection(self.site.block, self.site.position)?.ok_or("borrowed Resource sum extraction lacks its exact constructor relation")?;
                            if row.source() != *source || row.tag() != tag { return Err("borrowed Resource sum extraction changed its exact tag/header relation".into()); }
                            if selected {
                                if row.output() != *target || row.success_site() != self.site || !self.function.is_view_local(*target) || self.local_slots[target.index() as usize].is_some() { return Err("borrowed Resource payload gained an owning or foreign output".into()); }
                                let path = row.path();
                                let loan = self.loan(self.ordinal, usize::MAX, ResourceFrameId(0), ResourceLoanSource::ProjectedSumPayload { parent, tag, path });
                                state.leases.insert(loan);
                                self.store(&mut state, *target, Some(Value { shape: ResourceShape::Plain { kind: value.shape.kind().clone() }, occupancy: ResourceOccupancy::Occupied, owner: None, loan: Some(loan) }), false, false)?;
                                self.operation(self.active_frame, ResourceOperationRole::ProjectSumView { source: parent, destination: loan, tag, path });
                            } else {
                                let ResourceShape::Result { failure, .. } = value.shape else { return Err("borrowed Optional has no failure companion".into()); };
                                let header = self.function.local(*target).ok_or("borrowed Resource failure read has no header")?.clone();
                                if failure != TypeInterner::STRING || header.ty != failure || header.view_source.is_some() || row.failure_site() != Some(self.site) { return Err("borrowed Resource failure read lacks its exact String companion".into()); }
                                self.operation(self.active_frame, ResourceOperationRole::ReadFailureCompanion { source: parent, target: header, failure, tag });
                            }
                            continue;
                        }
                        if selected {
                            let owner = value.owner.ok_or("Resource sum payload lacks its holder")?;
                            let mut payload = self.read(&mut state, *source, true)?.ok_or("Resource sum source was consumed before its exact take")?;
                            payload.shape = ResourceShape::Plain { kind: payload.shape.kind().clone() };
                            payload.owner = Some(self.local_slots[target.index() as usize].ok_or("Resource sum success has no exact output slot")?);
                            self.store(&mut state, *target, Some(payload), false, false)?;
                            self.operation(self.active_frame, ResourceOperationRole::SumTake { source: owner, destination: self.local_slots[target.index() as usize].ok_or("Resource sum success has no dedicated output slot")?, tag, success: true });
                        } else {
                            let ResourceShape::Result { failure, .. } = &value.shape else { return Err("pending ResourceOwnershipPlan: Optional empty-arm extraction has no ordinary companion".into()); };
                            let header = self.function.local(*target).ok_or("Resource failure take has no exact destination")?.clone();
                            if header.ty != *failure { return Err("Resource failure take changes its exact companion type".into()); }
                            let owner = value.owner.ok_or("Resource failure companion has no exact sum shell holder")?;
                            self.operation(self.active_frame, ResourceOperationRole::TakeFailureCompanion { source: owner, target: header, failure: *failure, tag });
                            self.read(&mut state, *source, true)?;
                            if shape(&self.program.resource_manifest, self.types, self.function.local(*target).ok_or("Resource failure take has no destination")?.ty)?.is_some() { return Err("Resource failure companion cannot become an occupied owner".into()); }
                        }
                    }
                }
                StatementKind::Assert { condition, message } => {
                    self.expression(&mut state, condition, false)?;
                    if let Some(message) = message { self.expression(&mut state, message, false)?; }
                }
                StatementKind::Breakpoint { condition, bindings } => {
                    if bindings.iter().any(|local| state.values[local.index() as usize].is_some()) { return Err("pending ResourceOwnershipPlan: Resource debugger observation transport is unproved".into()); }
                    if let Some(condition) = condition { self.expression(&mut state, condition, false)?; }
                }
                StatementKind::Trace(local) => {
                    if state.values[local.index() as usize].is_some() { return Err("Resource trace cannot manufacture observation or copied custody".into()); }
                }
                StatementKind::BeginCallView { local, value } => {
                    if shape(&self.program.resource_manifest, self.types, self.function.local(*local).ok_or("loan local missing")?.ty)?.is_some() || resource_type_pending(self.types, value.ty) { return Err("pending ResourceOwnershipPlan: ordinary BeginCallView is not a Resource lease constructor".into()); }
                }
                StatementKind::EndCallView { .. } | StatementKind::OpenCallOwnerGeneration { .. } | StatementKind::ReplaceCallOwnerGeneration { .. } | StatementKind::CloseCallOwnerGeneration { .. } => {}
                StatementKind::CheckRefinement { call, .. } => { self.expression(&mut state, call, false)?; }
                StatementKind::ReflectedContainerReady { .. } | StatementKind::SequenceLength { .. } | StatementKind::SequenceGet { .. } | StatementKind::IterationBorrow { .. } => return Err("pending ResourceOwnershipPlan: sequence or aggregate custody needs its dedicated normalization".into()),
            }
        }
        self.site = ResourceSite {
            function: self.function.id,
            block: block.id,
            position: ResourcePosition::Terminator,
        };
        self.ordinal = 0;
        if state.aborted {
            if !state.calls.is_empty() {
                return Err("pending Resource call region: infrastructure abort needs its exact acquired-prefix retirement".into());
            }
            self.finish(&mut state, ResourceFrameId(0), ResourceCompletion::Abort);
            return Ok(Vec::new());
        }
        self.record_execution_frame()?;
        match &block.terminator.kind {
            TerminatorKind::Return(value) => {
                if !state.calls.is_empty() { return Err("Resource Source Return reaches an unretired normalized operation suffix".into()); }
                let mut outgoing = None;
                if let Some(value) = value {
                    let descriptor = self.descriptor_value(&state, value);
                    let returned = self.expression(&mut state, value, true)?;
                    if !state.aborted && let Some(expected) = self.plan.descriptor_return() {
                        let witnessed = self.function.resource_lowering.as_ref().is_some_and(|witness| witness.descriptors.return_matches(value));
                        if descriptor.as_ref() != Some(expected) || !witnessed { return Err("Resource descriptor Return differs from its exact current producer and archived return summary".into()); }
                    }
                    if let Some(returned) = returned {
                        if returned.loan.is_some() { return Err("Resource resident lease cannot escape as an owning return".into()); }
                        let expected = shape(&self.program.resource_manifest, self.types, self.function.return_type)?.ok_or("Resource return has an ordinary declared result")?;
                        if returned.shape != expected { return Err("Resource return changes its exact shape or kind".into()); }
                        if self.function.resource_lexical_exit(self.site.block, self.site.position)?.is_some() { self.lexical_exit(&mut state)?; }
                        let frame = self.return_frame.ok_or("Resource return lacks its initially selected provisional frame")?;
                        let destination = self.result_slot(usize::MAX, expected, frame);
                        self.plan.slots[destination.0].storage = ResourceSlotStorage::Return { site: self.site };
                        if let Some(source) = returned.owner { self.operation(frame, ResourceOperationRole::Transfer { source, destination }); }
                        outgoing = Some(destination);
                    } else if !state.aborted && shape(&self.program.resource_manifest, self.types, self.function.return_type)?.is_some() { return Err("Resource return has no exact owned output".into()); }
                }
                if outgoing.is_none() && !state.aborted && self.function.resource_lexical_exit(self.site.block, self.site.position)?.is_some() { self.lexical_exit(&mut state)?; }
                let completion = if state.aborted { ResourceCompletion::Abort } else { ResourceCompletion::Return };
                self.finish(&mut state, ResourceFrameId(0), completion);
                if let Some(source) = outgoing { self.operation(self.return_frame.ok_or("Resource return frame disappeared")?, ResourceOperationRole::CompleteReturnAfterCleanup { source }); }
                Ok(Vec::new())
            }
            TerminatorKind::Unreachable => { if !state.calls.is_empty() { return Err("pending Resource call region: unreachable exit needs its exact operation suffix retirement".into()); } self.finish(&mut state, ResourceFrameId(0), ResourceCompletion::Abort); Ok(Vec::new()) }
            TerminatorKind::Goto(target) => Ok(vec![(*target, state)]),
            TerminatorKind::Branch { condition, then_block, else_block } => {
                self.expression(&mut state, condition, false)?;
                if state.aborted { self.finish(&mut state, ResourceFrameId(0), ResourceCompletion::Abort); return Ok(Vec::new()); }
                let mut yes = state.clone(); let mut no = state;
                if let E::Local(tag) = condition.kind && let Some(source) = yes.tags.get(&tag.index()).copied() {
                    let value = yes.values[source.index() as usize].as_ref().ok_or("Resource tag source was consumed before its selecting edge")?;
                    if !value.shape.conditional() { return Err("Resource selecting edge lost its exact sum tag source".into()); }
                    // Plan both authenticated CFG edges. A known empty/occupied
                    // producer does not remove either runtime SumTag edge;
                    // the exact selecting guard still gates each SumTake.
                    if let Some(value) = &mut yes.values[source.index() as usize] { value.occupancy = ResourceOccupancy::Occupied; }
                    if let Some(value) = &mut no.values[source.index() as usize] { value.occupancy = ResourceOccupancy::Empty; }
                    yes.guards.insert(source.index(), (tag, true)); no.guards.insert(source.index(), (tag, false));
                }
                Ok(vec![(*then_block, yes), (*else_block, no)])
            }
            TerminatorKind::Respond(_) | TerminatorKind::Switch { .. } | TerminatorKind::ForEach { .. } | TerminatorKind::ReflectedTypeDispatch { .. } => Err("pending ResourceOwnershipPlan: selected control-flow custody normalization is unproved".into()),
        }
    }
}

fn join(left: &mut State, right: &State) -> Result<bool, String> {
    let before = left.clone();
    if left.calls != right.calls {
        return Err("Resource CFG join changed its exact operation stack, arrived prefix or retirement obligations".into());
    }
    if left.leases != right.leases {
        return Err(
            "Resource CFG join cannot extend or silently end a loan from only one path".into(),
        );
    }
    for index in 0..left.values.len() {
        match (&mut left.values[index], &right.values[index]) {
            (Some(a), Some(b)) if a.shape == b.shape && a.owner == b.owner && a.loan == b.loan => {
                if a.occupancy != b.occupancy { a.occupancy = ResourceOccupancy::Conditional; }
            }
            (None, None) => {}
            _ => return Err("pending ResourceOwnershipPlan: moved/uninitialized versus live CFG join requires conditional cleanup proof".into()),
        }
        left.moved[index] |= right.moved[index];
        if left.descriptors[index] != right.descriptors[index] {
            left.descriptors[index] = None;
        }
        if left.named_callables[index] != right.named_callables[index] {
            left.named_callables[index] = None;
        }
    }
    left.tags
        .retain(|id, source| right.tags.get(id) == Some(source));
    left.guards
        .retain(|id, guard| right.guards.get(id) == Some(guard));
    Ok(*left != before)
}

pub(super) fn analyze(
    program: &Program,
    function: &Function,
    types: &TypeInterner,
) -> Result<ResourceFunctionPlan, String> {
    let original = &function
        .resource_lowering
        .as_ref()
        .ok_or("Resource plan lacks its authenticated Source")?
        .original;
    let admitted = function
        .resource_lowering
        .as_ref()
        .ok_or("Resource plan has no constructor witness")?
        .borrowed_sums
        .iter()
        .map(|row| row.original_local())
        .collect::<Vec<_>>();
    if walk::nested_resource_declaration(&original.body, original, types, false, &admitted) {
        return Err("pending ResourceOwnershipPlan: nested Resource declarations need exact constructor Scope records".into());
    }
    if function.capture_count != 0 {
        return Err(
            "pending ResourceOwnershipPlan: Resource-bearing capture transport is unproved".into(),
        );
    }
    ControlFlowGraph::analyze(function)
        .map_err(|errors| format!("Resource CFG is malformed: {errors:?}"))?;
    let site = ResourceSite {
        function: function.id,
        block: function.entry,
        position: ResourcePosition::Terminator,
    };
    let return_frame =
        shape(&program.resource_manifest, types, function.return_type)?.map(|_| ResourceFrameId(1));
    let mut frames = vec![ResourceFrame {
        id: ResourceFrameId(0),
        role: ResourceFrameRole::Scope,
        site,
        parent: return_frame,
        ordinal: 0,
    }];
    if let Some(id) = return_frame {
        frames.push(ResourceFrame {
            id,
            role: ResourceFrameRole::Return,
            site,
            parent: None,
            ordinal: 0,
        });
    }
    let plan = ResourceFunctionPlan {
        function: function.id,
        identity: function.identity.clone(),
        parameters: function.params.clone(),
        return_type: function.return_type,
        frames,
        slots: Vec::new(),
        loans: Vec::new(),
        operations: Vec::new(),
        execution_frames: Vec::new(),
        lexical_exits: Vec::new(),
        descriptor_return: function
            .resource_lowering
            .as_ref()
            .and_then(|witness| witness.descriptors.returned())
            .cloned(),
        descriptor_values: returned_descriptors::current_values(
            function
                .resource_lowering
                .as_ref()
                .ok_or("Resource descriptors lost their constructor witness")?,
            function,
        )?,
        descriptor_locals: function
            .resource_lowering
            .as_ref()
            .map_or_else(Vec::new, |witness| {
                witness
                    .descriptors
                    .locals()
                    .map(|(local, hook)| (local, hook.clone()))
                    .collect()
            }),
        named_callable_producers: Vec::new(),
        named_callable_values: named_callables::current_values(
            function
                .resource_lowering
                .as_ref()
                .ok_or("Resource named values lost their constructor witness")?,
            function,
        )?,
    };
    let mut analysis = Analysis {
        program,
        function,
        types,
        plan,
        local_slots: vec![None; function.locals.len()],
        expression_slots: BTreeMap::new(),
        operation_frames: BTreeMap::new(),
        loan_sites: BTreeMap::new(),
        site,
        ordinal: 0,
        expression: None,
        emit: false,
        active_frame: ResourceFrameId(0),
        return_frame,
        normalized_calls: BTreeMap::new(),
    };
    let mut initial = State {
        values: vec![None; function.locals.len()],
        moved: vec![false; function.locals.len()],
        descriptors: vec![None; function.locals.len()],
        named_callables: vec![None; function.locals.len()],
        tags: BTreeMap::new(),
        guards: BTreeMap::new(),
        leases: BTreeSet::new(),
        aborted: false,
        calls: Vec::new(),
    };
    for local in &function.locals {
        if let Some(shape) = shape(&program.resource_manifest, types, local.ty)?
            && !function.is_view_local(local.id)
        {
            let id = ResourceOwnerSlotId(analysis.plan.slots.len());
            analysis.local_slots[local.id.index() as usize] = Some(id);
            analysis.plan.slots.push(ResourceOwnerSlot {
                id,
                frame: ResourceFrameId(0),
                shape,
                storage: ResourceSlotStorage::Local {
                    header: local.clone(),
                },
            });
        }
    }
    for (parameter, param) in function.params.iter().enumerate() {
        if let Some(shape) = shape(&program.resource_manifest, types, param.ty)? {
            let mut value = Value {
                occupancy: if shape.conditional() {
                    ResourceOccupancy::Conditional
                } else {
                    ResourceOccupancy::Occupied
                },
                shape,
                owner: analysis.local_slots[param.local.index() as usize],
                loan: None,
            };
            if param.mode == ParamMode::View {
                let loan = ResourceLoanId(analysis.plan.loans.len());
                analysis.plan.loans.push(ResourceLoan {
                    id: loan,
                    frame: ResourceFrameId(0),
                    shape: value.shape.clone(),
                    parameter: None,
                    source: ResourceLoanSource::IncomingViewFormal {
                        scope: ResourceFrameId(0),
                        parameter,
                    },
                });
                value.loan = Some(loan);
            }
            initial.values[param.local.index() as usize] = Some(value);
        }
    }
    let mut states = vec![None; function.blocks.len()];
    states[function.entry.index() as usize] = Some(initial);
    let mut queue = VecDeque::from([function.entry]);
    let mut steps = 0usize;
    let limit = function
        .blocks
        .len()
        .saturating_mul(function.locals.len().saturating_add(1))
        .saturating_mul(8)
        .saturating_add(1);
    while let Some(block) = queue.pop_front() {
        steps += 1;
        if steps > limit {
            return Err("pending ResourceOwnershipPlan: custody fixed point exceeds the finite normalization bound".into());
        }
        let input = states[block.index() as usize]
            .as_ref()
            .ok_or("Resource reachable block has no exact incoming state")?
            .clone();
        for (target, output) in analysis.block(&function.blocks[block.index() as usize], input)? {
            let changed = if let Some(current) = &mut states[target.index() as usize] {
                join(current, &output)?
            } else {
                states[target.index() as usize] = Some(output);
                true
            };
            if changed {
                queue.push_back(target);
            }
        }
    }
    analysis.emit = true;
    for block in &function.blocks {
        if let Some(state) = &states[block.id.index() as usize] {
            analysis.block(block, state.clone())?;
        } else if walk::mir_block_has_resource(block)
            || walk::mir_block_has_custody(block, function, types)
        {
            return Err(
                "Resource custody operation is disconnected from its authenticated entry CFG"
                    .into(),
            );
        }
    }
    Ok(analysis.plan)
}
