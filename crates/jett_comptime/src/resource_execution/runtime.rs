use std::collections::HashMap;
use std::sync::Arc;

use jett_common::Span;
use jett_resolve::DefId;
use jett_runtime::ResourceRegistry;
use jett_typecheck::{
    CheckedCalleeAccess, CheckedCallerEffect, CheckedInvocationTarget, CheckedResourceProgram,
};
use jett_types::ResourceKernelRecipe;

use super::checked::{CheckedBodyCursor, CheckedInvocation, FunctionInvocation};
use super::custody::PayloadStep;
use super::provider::InstalledResourceProvider;
use super::{
    CheckedExecution, EvaluatedValue, ExecutionPurpose, FrameId, OwnerHolder, OwnerLedger,
    ResourceCarrier, ResourceExecutionError, ResourceHookDescriptor,
};
use crate::value::Value;

pub(crate) struct OperationFrame {
    pub(crate) frame: FrameId,
    parent: FrameId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveFrame {
    Scope(FrameId),
    Operation(FrameId),
}

impl ActiveFrame {
    fn id(self) -> FrameId {
        match self {
            Self::Scope(frame) | Self::Operation(frame) => frame,
        }
    }
}

/// Internal transport only. Neither ordinary Value copies nor a raw interpreter
/// constructor can install a provider or mint an owning ticket.
pub(crate) struct ResourceTransport {
    pub(crate) checked: CheckedExecution,
    pub(crate) checked_source_active: bool,
    ledger: OwnerLedger,
    registry: ResourceRegistry,
    provider: InstalledResourceProvider,
    scopes: Vec<(FrameId, HashMap<DefId, EvaluatedValue>)>,
    operations: Vec<FrameId>,
    active_frames: Vec<ActiveFrame>,
    returns: Vec<FrameId>,
    defaults: Vec<FrameId>,
    next_temporary: usize,
    cleanup_error: Option<ResourceExecutionError>,
}

impl ResourceTransport {
    pub(crate) fn checked_only(
        program: Arc<CheckedResourceProgram>,
        purpose: ExecutionPurpose,
        scope_depth: usize,
    ) -> Result<Self, ResourceExecutionError> {
        let checked = CheckedExecution::new(program, purpose)?;
        let mut transport = Self {
            checked,
            checked_source_active: false,
            ledger: OwnerLedger::default(),
            registry: ResourceRegistry::new(),
            provider: InstalledResourceProvider::Disabled,
            scopes: Vec::new(),
            operations: Vec::new(),
            active_frames: Vec::new(),
            returns: Vec::new(),
            defaults: Vec::new(),
            next_temporary: 0,
            cleanup_error: None,
        };
        for _ in 0..scope_depth {
            transport.push_scope();
        }
        Ok(transport)
    }

    pub(crate) fn push_scope(&mut self) {
        let frame = self.ledger.frame();
        self.scopes.push((frame, HashMap::new()));
        self.active_frames.push(ActiveFrame::Scope(frame));
    }

    pub(crate) fn pop_scope(&mut self) {
        let Some((frame, _)) = self.scopes.last() else {
            self.cleanup_error
                .get_or_insert(ResourceExecutionError::InvalidFrame);
            return;
        };
        let frame = *frame;
        if self.active_frames.last() != Some(&ActiveFrame::Scope(frame)) {
            self.cleanup_error
                .get_or_insert(ResourceExecutionError::InvalidFrame);
            return;
        }
        let result = self.ledger.unwind(frame, &mut self.registry);
        if !self.ledger.frame_is_live(frame) {
            self.scopes.pop();
            self.active_frames.pop();
        }
        if let Err(error) = result {
            self.cleanup_error.get_or_insert(error);
        }
    }

    pub(crate) fn check_cleanup(&mut self) -> Result<(), ResourceExecutionError> {
        self.cleanup_error.take().map_or(Ok(()), Err)
    }

    fn scope_frame(&self) -> Result<FrameId, ResourceExecutionError> {
        self.scopes
            .last()
            .map(|(frame, _)| *frame)
            .ok_or(ResourceExecutionError::InvalidFrame)
    }

    fn current_frame(&self) -> Result<FrameId, ResourceExecutionError> {
        self.operations
            .last()
            .copied()
            .map_or_else(|| self.scope_frame(), Ok)
    }

    fn temporary(&mut self, frame: FrameId) -> Result<OwnerHolder, ResourceExecutionError> {
        let slot = self.next_temporary;
        self.next_temporary = slot
            .checked_add(1)
            .ok_or(ResourceExecutionError::IdentityExhausted)?;
        Ok(OwnerHolder::Temporary { frame, slot })
    }

    pub(crate) fn begin_operation(&mut self) -> Result<OperationFrame, ResourceExecutionError> {
        let parent = self.current_frame()?;
        let frame = self.ledger.frame();
        self.operations.push(frame);
        self.active_frames.push(ActiveFrame::Operation(frame));
        Ok(OperationFrame { frame, parent })
    }

    pub(crate) fn end_operation(
        &mut self,
        operation: OperationFrame,
        value: Option<&mut EvaluatedValue>,
    ) -> Result<(), ResourceExecutionError> {
        if self.operations.last() != Some(&operation.frame)
            || self.active_frames.last() != Some(&ActiveFrame::Operation(operation.frame))
        {
            return Err(ResourceExecutionError::InvalidFrame);
        }
        let preserved = match value {
            Some(value) => match self.temporary(operation.parent) {
                Ok(holder) => self.ledger.transfer(&mut value.custody, holder),
                Err(error) => Err(error),
            },
            None => Ok(()),
        };
        // A failed output adoption still unwinds the original frame. Never
        // remove its only cleanup path before a fallible holder transfer.
        let cleanup = self.ledger.unwind(operation.frame, &mut self.registry);
        if !self.ledger.frame_is_live(operation.frame) {
            self.operations.pop();
            self.active_frames.pop();
        }
        match (preserved, cleanup, self.check_cleanup()) {
            (_, Err(error), _) | (_, Ok(()), Err(error)) => Err(error),
            (Err(error), Ok(()), Ok(())) => Err(error),
            (Ok(()), Ok(()), Ok(())) => Ok(()),
        }
    }

    pub(crate) fn discard_value(
        &mut self,
        mut value: EvaluatedValue,
    ) -> Result<(), ResourceExecutionError> {
        let operation = self.begin_operation()?;
        let adopted = self.hold_actual(&mut value);
        let completed = self.end_operation(operation, None);
        match (adopted, completed) {
            (_, Err(error)) => Err(error),
            (Err(error), Ok(())) => Err(error),
            (Ok(()), Ok(())) => Ok(()),
        }
    }

    pub(crate) fn hold_actual(
        &mut self,
        value: &mut EvaluatedValue,
    ) -> Result<(), ResourceExecutionError> {
        let holder = self.temporary(self.current_frame()?)?;
        self.ledger.transfer(&mut value.custody, holder)?;
        self.ledger
            .hold_borrows(&value.custody, self.current_frame()?)
    }

    pub(crate) fn borrow(
        &self,
        value: &EvaluatedValue,
    ) -> Result<EvaluatedValue, ResourceExecutionError> {
        self.ledger.borrow(value)
    }

    pub(crate) fn captured_resource_reference(&self, name: &str, body: Span) -> Option<bool> {
        let mut found = false;
        for (_, bindings) in self.scopes.iter().rev() {
            for definition in bindings.keys() {
                if self
                    .checked
                    .program()
                    .resolved()
                    .scope_table
                    .definitions
                    .get(definition.index() as usize)
                    .is_some_and(|info| info.name == name)
                {
                    found = true;
                    if self.checked.referenced_in(*definition, body) {
                        return Some(true);
                    }
                }
            }
        }
        found.then_some(false)
    }

    pub(crate) fn has_binding(&self, definition: DefId) -> bool {
        self.scopes
            .iter()
            .rev()
            .any(|(_, bindings)| bindings.contains_key(&definition))
    }

    pub(crate) fn binding(
        &mut self,
        definition: DefId,
        borrow: bool,
    ) -> Result<EvaluatedValue, ResourceExecutionError> {
        let index = self
            .scopes
            .iter()
            .rposition(|(_, bindings)| bindings.contains_key(&definition))
            .ok_or(ResourceExecutionError::InvalidOwner)?;
        if borrow {
            let value = self.scopes[index]
                .1
                .get(&definition)
                .ok_or(ResourceExecutionError::InvalidOwner)?;
            self.ledger.borrow(value)
        } else {
            let mut value = self.scopes[index]
                .1
                .remove(&definition)
                .ok_or(ResourceExecutionError::InvalidOwner)?;
            // A borrowed alias is a physical carrier only; it cannot mint a
            // ticket merely by being used with owned syntax.
            if !value.custody.is_empty() && !value.custody.has_owners() {
                self.scopes[index].1.insert(definition, value);
                return Err(ResourceExecutionError::InvalidOwner);
            }
            self.hold_actual(&mut value)?;
            Ok(value)
        }
    }

    pub(crate) fn install_binding(
        &mut self,
        declaration: Span,
        mut value: EvaluatedValue,
    ) -> Result<DefId, ResourceExecutionError> {
        let fact = self.checked.binding(declaration)?;
        self.validate_value_type(&value, fact.ty)?;
        let frame = self.scope_frame()?;
        if self
            .scopes
            .last()
            .is_some_and(|(_, bindings)| bindings.contains_key(&fact.definition))
        {
            return Err(ResourceExecutionError::InvalidOwner);
        }
        self.ledger.transfer(
            &mut value.custody,
            OwnerHolder::Binding {
                frame,
                definition: fact.definition,
            },
        )?;
        self.ledger.hold_borrows(&value.custody, frame)?;
        self.scopes
            .last_mut()
            .ok_or(ResourceExecutionError::InvalidFrame)?
            .1
            .insert(fact.definition, value);
        Ok(fact.definition)
    }

    pub(crate) fn enter_return(&mut self, destination: FrameId) {
        self.returns.push(destination);
    }
    pub(crate) fn leave_return(&mut self) {
        self.returns.pop();
    }
    pub(crate) fn preserve_return(
        &mut self,
        value: &mut EvaluatedValue,
    ) -> Result<(), ResourceExecutionError> {
        if value.custody.has_borrows() {
            return Err(ResourceExecutionError::InvalidOwner);
        }
        let destination = self
            .returns
            .last()
            .copied()
            .ok_or(ResourceExecutionError::InvalidFrame)?;
        let holder = self.temporary(destination)?;
        self.ledger.transfer(&mut value.custody, holder)
    }

    pub(crate) fn enter_default(&mut self) -> Result<(), ResourceExecutionError> {
        self.defaults.push(self.current_frame()?);
        Ok(())
    }
    pub(crate) fn leave_default(&mut self) {
        self.defaults.pop();
    }
    pub(crate) fn preserve_default(
        &mut self,
        value: &mut EvaluatedValue,
    ) -> Result<(), ResourceExecutionError> {
        if value.custody.has_borrows() {
            return Err(ResourceExecutionError::InvalidOwner);
        }
        let destination = self
            .defaults
            .last()
            .copied()
            .ok_or(ResourceExecutionError::InvalidFrame)?;
        let holder = self.temporary(destination)?;
        self.ledger.transfer(&mut value.custody, holder)
    }

    pub(crate) fn cursor(&self) -> CheckedBodyCursor {
        self.checked.cursor()
    }
    pub(crate) fn enter_callee(
        &mut self,
        invocation: &FunctionInvocation,
    ) -> Result<(), ResourceExecutionError> {
        let FunctionInvocation::Source(invocation) = invocation else {
            self.checked.enter_ordinary();
            return Ok(());
        };
        match invocation.target() {
            CheckedInvocationTarget::Resolved(_) => {
                self.checked.enter_ordinary();
                Ok(())
            }
            CheckedInvocationTarget::Generic(call) => self.checked.enter_generic(call),
            CheckedInvocationTarget::Method(_)
            | CheckedInvocationTarget::Interface(_)
            | CheckedInvocationTarget::Indirect(_)
            | CheckedInvocationTarget::Intrinsic(_) => {
                Err(ResourceExecutionError::InvalidInvocation)
            }
        }
    }

    pub(crate) fn formal_value(
        &self,
        value: &EvaluatedValue,
        access: CheckedCalleeAccess,
    ) -> Result<Option<EvaluatedValue>, ResourceExecutionError> {
        match access {
            CheckedCalleeAccess::View => self.ledger.borrow(value).map(Some),
            CheckedCalleeAccess::Owned => Ok(None),
        }
    }

    pub(crate) fn validate_actual(
        &self,
        value: &EvaluatedValue,
        effect: CheckedCallerEffect,
    ) -> Result<(), ResourceExecutionError> {
        if value.custody.is_empty() {
            return Ok(());
        }
        match effect {
            CheckedCallerEffect::Copy | CheckedCallerEffect::ObserveData => {
                Err(ResourceExecutionError::InvalidOwner)
            }
            CheckedCallerEffect::TransferOwned | CheckedCallerEffect::RelinquishOwned => {
                if value.custody.has_owners() {
                    Ok(())
                } else {
                    Err(ResourceExecutionError::InvalidOwner)
                }
            }
            CheckedCallerEffect::RetainBorrow => {
                // Written view of a producer may still have an operation-owned
                // backing; the callee receives only the derived borrow.
                self.ledger.borrow(value).map(|_| ())
            }
        }
    }

    pub(crate) fn invoke_hook(
        &mut self,
        invocation: &CheckedInvocation,
        descriptor: &ResourceHookDescriptor,
        arguments: Vec<EvaluatedValue>,
    ) -> Result<EvaluatedValue, ResourceExecutionError> {
        if !self.checked_source_active {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        let (hook, resource, recipe) = self.checked.reached_hook(invocation, descriptor)?;
        let jett_types::Type::Function { params, .. } = self
            .checked
            .program()
            .checked()
            .interner
            .resolve(hook.function_type)
        else {
            return Err(ResourceExecutionError::InvalidInvocation);
        };
        if params.len() != arguments.len() {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        for (argument, ty) in arguments.iter().zip(params) {
            self.validate_value_type(argument, *ty)?;
        }
        self.provider.ensure_installed()?;
        if recipe == ResourceKernelRecipe::Finalize {
            let [token] = <[EvaluatedValue; 1]>::try_from(arguments)
                .map_err(|_| ResourceExecutionError::InvalidInvocation)?;
            self.ledger.close(token.custody, &mut self.registry)?;
            return Ok(EvaluatedValue::ordinary(Value::Nothing));
        }
        match (recipe, arguments.as_slice()) {
            (ResourceKernelRecipe::NetworkFactory, [network, label]) => {
                let (Value::GrantedNetwork(grant), Value::Int64(label)) =
                    (network.value.payload(), label.value.payload())
                else {
                    return Err(ResourceExecutionError::InvalidGrant);
                };
                // All fallible holder allocation precedes provider publication.
                // A successful key is adopted immediately or explicitly closed.
                let holder = self.temporary(self.current_frame()?)?;
                match self
                    .provider
                    .construct(&mut self.registry, resource, grant, *label)?
                {
                    Ok(key) => {
                        let carrier = ResourceCarrier {
                            program: self.checked.program().clone(),
                            resource,
                            key,
                        };
                        let custody = match self.ledger.acquire(carrier.clone(), holder) {
                            Ok(custody) => custody,
                            Err(error) => {
                                self.registry.close(key, resource.registry_type)?;
                                return Err(error);
                            }
                        };
                        Ok(EvaluatedValue {
                            value: Value::ResultOk(Box::new(Value::Resource(carrier))),
                            custody,
                        }
                        .prefix(PayloadStep::Ok))
                    }
                    Err(error) => Ok(EvaluatedValue::ordinary(Value::ResultFail(Box::new(
                        Value::String(error),
                    )))),
                }
            }
            (ResourceKernelRecipe::NetworkBorrow, [network, token]) => {
                let Value::GrantedNetwork(grant) = network.value.payload() else {
                    return Err(ResourceExecutionError::InvalidGrant);
                };
                let carrier = self.ledger.borrowed_carrier(&token.custody)?;
                if !Arc::ptr_eq(&carrier.program, self.checked.program())
                    || carrier.resource != resource
                {
                    return Err(ResourceExecutionError::ForeignProgram);
                }
                let outcome =
                    self.provider
                        .borrow(&mut self.registry, resource, grant, carrier.key)?;
                Ok(EvaluatedValue::ordinary(match outcome {
                    Ok(value) => Value::ResultOk(Box::new(Value::Int64(value))),
                    Err(error) => Value::ResultFail(Box::new(Value::String(error))),
                }))
            }
            _ => Err(ResourceExecutionError::InvalidInvocation),
        }
    }

    pub(crate) fn validate_value_type(
        &self,
        value: &EvaluatedValue,
        ty: jett_types::TypeId,
    ) -> Result<(), ResourceExecutionError> {
        fn positions<'a>(
            value: &'a Value,
            path: &mut Vec<PayloadStep>,
            out: &mut Vec<(Vec<PayloadStep>, &'a ResourceCarrier)>,
        ) -> Result<(), ResourceExecutionError> {
            let (step, child) = match value {
                Value::Resource(carrier) => {
                    out.push((path.clone(), carrier));
                    return Ok(());
                }
                Value::Typed { value, .. } => (PayloadStep::Typed, value.as_ref()),
                Value::ResultOk(value) => (PayloadStep::Ok, value.as_ref()),
                Value::ResultFail(value) => (PayloadStep::Fail, value.as_ref()),
                Value::OptionalSome(value) => (PayloadStep::Some, value.as_ref()),
                value
                    if !value.contains_live_resource_or_grant()
                        || matches!(value, Value::GrantedNetwork(_)) =>
                {
                    return Ok(());
                }
                _ => return Err(ResourceExecutionError::InvalidPayloadPath),
            };
            path.push(step);
            positions(child, path, out)?;
            path.pop();
            Ok(())
        }
        // Absence is a value/layout proof, not an exemption based on the lack
        // of a carrier. Recurse through every occupied slot of the exact type.
        fn absent_shape(
            value: &Value,
            ty: jett_types::TypeId,
            checked: &CheckedExecution,
        ) -> Result<(), ResourceExecutionError> {
            use jett_types::Type;
            let types = &checked.program().checked().interner;
            if ty.index() as usize >= types.len() {
                return Err(ResourceExecutionError::InvalidCheckedProgram);
            }
            if value.contains_live_resource_or_grant() {
                return Err(ResourceExecutionError::InvalidPayloadPath);
            }
            let kind = types.resolve(ty);
            // A layout match cannot replace the runtime predicate boundary.
            // Resource-bearing return normalization does not prove it here.
            if matches!(kind, Type::Refinement { .. }) {
                return Err(ResourceExecutionError::InvalidPayloadPath);
            }
            if let Value::Typed { type_name, value } = value {
                if type_name != &types.type_name(ty) {
                    return Err(ResourceExecutionError::InvalidPayloadPath);
                }
                let inner = match kind {
                    Type::Secret(inner) => *inner,
                    _ => ty,
                };
                return absent_shape(value, inner, checked);
            }
            let fields = |values: &[(String, Value)], expected: &[(String, jett_types::TypeId)]| {
                if values.len() != expected.len() {
                    return Err(ResourceExecutionError::InvalidPayloadPath);
                }
                for ((name, value), (expected_name, field_type)) in values.iter().zip(expected) {
                    if name != expected_name {
                        return Err(ResourceExecutionError::InvalidPayloadPath);
                    }
                    absent_shape(value, *field_type, checked)?;
                }
                Ok(())
            };
            let payloads = |values: &[Value], expected: &[(String, jett_types::TypeId)]| {
                if values.len() != expected.len() {
                    return Err(ResourceExecutionError::InvalidPayloadPath);
                }
                for (value, (_, field_type)) in values.iter().zip(expected) {
                    absent_shape(value, *field_type, checked)?;
                }
                Ok(())
            };
            match (value, kind) {
                (value, Type::Secret(inner)) => absent_shape(value, *inner, checked),
                (Value::Int64(number), Type::Int8) if i8::try_from(*number).is_ok() => Ok(()),
                (Value::Int64(number), Type::Int16) if i16::try_from(*number).is_ok() => Ok(()),
                (Value::Int64(number), Type::Int32) if i32::try_from(*number).is_ok() => Ok(()),
                (Value::Int64(_), Type::Int64) => Ok(()),
                (Value::Int64(number), Type::Uint8) if u8::try_from(*number).is_ok() => Ok(()),
                (Value::Int64(number), Type::Uint16) if u16::try_from(*number).is_ok() => Ok(()),
                (Value::Int64(number), Type::Uint32) if u32::try_from(*number).is_ok() => Ok(()),
                (Value::Uint64(_), Type::Uint64)
                | (Value::Float64(_), Type::Float32 | Type::Float64)
                | (Value::String(_), Type::String)
                | (Value::Bool(_), Type::Bool)
                | (Value::Bytes(_), Type::Bytes)
                | (Value::Nothing, Type::Nothing) => Ok(()),
                (Value::List(values), Type::List(inner))
                | (Value::Set(values), Type::Set(inner)) => {
                    for value in values {
                        absent_shape(value, *inner, checked)?;
                    }
                    Ok(())
                }
                (Value::Map(values), Type::Map(key, item)) => {
                    for (key_value, item_value) in values {
                        absent_shape(key_value, *key, checked)?;
                        absent_shape(item_value, *item, checked)?;
                    }
                    Ok(())
                }
                (Value::OptionalNone, Type::Optional(_)) => Ok(()),
                (Value::OptionalSome(value), Type::Optional(inner)) => {
                    absent_shape(value, *inner, checked)
                }
                (Value::ResultOk(value), Type::Result(success, _)) => {
                    absent_shape(value, *success, checked)
                }
                (Value::ResultFail(value), Type::Result(_, error)) => {
                    absent_shape(value, *error, checked)
                }
                (
                    Value::Struct {
                        type_name,
                        concrete_type,
                        fields: values,
                    },
                    Type::Struct(owner),
                ) => {
                    let definition = types.resolve_struct(*owner);
                    let arguments = types.nominal_type_arguments(ty);
                    let header_matches = if arguments.is_empty() {
                        type_name == &definition.name
                            && concrete_type
                                .as_ref()
                                .is_none_or(|name| name == &definition.name)
                    } else {
                        let physical_instance = format!(
                            "{}[{}]",
                            type_name,
                            arguments
                                .iter()
                                .map(|argument| types.type_name(*argument))
                                .collect::<Vec<_>>()
                                .join(", ")
                        );
                        physical_instance == definition.name
                            && concrete_type.as_deref() == Some(definition.name.as_str())
                    };
                    if !header_matches {
                        return Err(ResourceExecutionError::InvalidPayloadPath);
                    }
                    fields(values, &definition.fields)
                }
                (
                    Value::Enum {
                        type_name,
                        variant,
                        fields: values,
                    },
                    Type::Enum(owner),
                ) => {
                    let definition = types.resolve_enum(*owner);
                    if type_name != &definition.name {
                        return Err(ResourceExecutionError::InvalidPayloadPath);
                    }
                    let mut variants = definition
                        .variants
                        .iter()
                        .filter(|candidate| candidate.name == *variant);
                    let selected = variants
                        .next()
                        .ok_or(ResourceExecutionError::InvalidPayloadPath)?;
                    if variants.next().is_some() {
                        return Err(ResourceExecutionError::InvalidCheckedProgram);
                    }
                    payloads(values, &selected.fields)
                }
                (
                    Value::Machine {
                        type_name,
                        state,
                        fields: values,
                    },
                    Type::Machine(owner),
                ) => {
                    let definition = types.resolve_machine(*owner);
                    if type_name != &definition.name {
                        return Err(ResourceExecutionError::InvalidPayloadPath);
                    }
                    let mut states = definition
                        .states
                        .iter()
                        .filter(|candidate| candidate.name == *state);
                    let selected = states
                        .next()
                        .ok_or(ResourceExecutionError::InvalidPayloadPath)?;
                    if states.next().is_some() {
                        return Err(ResourceExecutionError::InvalidCheckedProgram);
                    }
                    payloads(values, &selected.fields)
                }
                (
                    Value::Machine {
                        type_name,
                        state,
                        fields: values,
                    },
                    Type::MachineState {
                        machine,
                        state: expected,
                    },
                ) => {
                    let definition = types.resolve_machine(*machine);
                    let selected = definition
                        .state(*expected)
                        .ok_or(ResourceExecutionError::InvalidCheckedProgram)?;
                    if type_name != &definition.name || state != &selected.name {
                        return Err(ResourceExecutionError::InvalidPayloadPath);
                    }
                    payloads(values, &selected.fields)
                }
                (Value::ResourceHook(descriptor), Type::Function { .. })
                    if Arc::ptr_eq(&descriptor.program, checked.program())
                        && checked
                            .program()
                            .checked()
                            .resource_hooks
                            .get(&descriptor.definition)
                            .is_some_and(|hook| hook.function_type == ty) =>
                {
                    Ok(())
                }
                _ => Err(ResourceExecutionError::InvalidPayloadPath),
            }
        }
        fn shape(
            value: &Value,
            ty: jett_types::TypeId,
            checked: &CheckedExecution,
        ) -> Result<(), ResourceExecutionError> {
            use jett_types::{CapabilityKind, Type, TypeInterner};
            let types = &checked.program().checked().interner;
            if ty.index() as usize >= types.len() {
                return Err(ResourceExecutionError::InvalidCheckedProgram);
            }
            if !value.contains_live_resource_or_grant() && checked.type_contains_resource(ty)? {
                return absent_shape(value, ty, checked);
            }
            match (value, types.resolve(ty)) {
                (Value::Resource(carrier), Type::Resource(_))
                    if Arc::ptr_eq(&carrier.program, checked.program())
                        && carrier.resource.checked_type == ty =>
                {
                    Ok(())
                }
                (Value::GrantedNetwork(_), Type::Capability(CapabilityKind::Network)) => Ok(()),
                (Value::ResultOk(value), Type::Result(success, _)) => {
                    shape(value, *success, checked)
                }
                (Value::ResultFail(value), Type::Result(_, error)) => shape(value, *error, checked),
                (Value::OptionalSome(value), Type::Optional(success)) => {
                    shape(value, *success, checked)
                }
                (Value::OptionalNone, Type::Optional(_)) => Ok(()),
                (Value::ResourceHook(descriptor), Type::Function { .. })
                    if Arc::ptr_eq(&descriptor.program, checked.program())
                        && checked
                            .program()
                            .checked()
                            .resource_hooks
                            .get(&descriptor.definition)
                            .is_some_and(|hook| hook.function_type == ty) =>
                {
                    Ok(())
                }
                (Value::Int64(_), _) if ty == TypeInterner::INT64 => Ok(()),
                (Value::String(_), _) if ty == TypeInterner::STRING => Ok(()),
                (Value::Nothing, _) if ty == TypeInterner::NOTHING => Ok(()),
                (value, _)
                    if !value.contains_live_resource_or_grant()
                        && !checked.type_contains_resource(ty)? =>
                {
                    Ok(())
                }
                _ => Err(ResourceExecutionError::InvalidPayloadPath),
            }
        }
        shape(&value.value, ty, &self.checked)?;
        let mut carriers = Vec::new();
        positions(&value.value, &mut Vec::new(), &mut carriers)?;
        self.ledger.validate_positions(&value.custody, &carriers)
    }

    pub(crate) fn unwind_all(&mut self) -> Result<(), ResourceExecutionError> {
        use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
        let mut first_panic = None;
        let mut first_error = None;
        let mut retry = Vec::new();
        // One chronological stack preserves the scope/operation interleaving.
        // A callee's borrowed formal is released before its caller owner.
        while let Some(active) = self.active_frames.pop() {
            let frame = active.id();
            let role_matches = match active {
                ActiveFrame::Scope(_) => {
                    if self.scopes.last().map(|(id, _)| *id) == Some(frame) {
                        self.scopes.pop();
                        true
                    } else {
                        false
                    }
                }
                ActiveFrame::Operation(_) => {
                    if self.operations.last() == Some(&frame) {
                        self.operations.pop();
                        true
                    } else {
                        false
                    }
                }
            };
            if !role_matches {
                first_error.get_or_insert(ResourceExecutionError::InvalidFrame);
            }
            // A finalizer panic can leave a physically retired frame on the
            // activation stack; do not finalize any member a second time.
            if !self.ledger.frame_is_live(frame) {
                continue;
            }
            match catch_unwind(AssertUnwindSafe(|| {
                self.ledger.unwind(frame, &mut self.registry)
            })) {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    first_error.get_or_insert(error);
                }
                Err(payload) if first_panic.is_none() => first_panic = Some(payload),
                Err(payload) => std::mem::forget(payload),
            }
            if self.ledger.frame_is_live(frame) {
                retry.push(frame);
            }
        }
        // Retain failed cleanup obligations until older exact frames have
        // released their leases. This does not make the invalid relation valid.
        for frame in retry {
            match catch_unwind(AssertUnwindSafe(|| {
                self.ledger.unwind(frame, &mut self.registry)
            })) {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    first_error.get_or_insert(error);
                }
                Err(payload) if first_panic.is_none() => first_panic = Some(payload),
                Err(payload) => std::mem::forget(payload),
            }
        }
        if !self.operations.is_empty() || !self.scopes.is_empty() {
            first_error.get_or_insert(ResourceExecutionError::InvalidFrame);
        }
        if let Some(payload) = first_panic {
            resume_unwind(payload);
        }
        first_error.map_or_else(|| self.check_cleanup(), Err)
    }

    pub(crate) fn live_owners(&self) -> usize {
        self.ledger.live_owners()
    }
    pub(crate) fn registry_live_count(&self) -> usize {
        self.registry.live_count()
    }

    #[cfg(test)]
    pub(crate) fn install_script(
        &mut self,
        operations: Vec<super::provider::ScriptOperation>,
    ) -> Result<super::provider::GrantedNetwork, ResourceExecutionError> {
        if self.checked.purpose() != ExecutionPurpose::ReferenceRuntime {
            return Err(ResourceExecutionError::WrongPurpose);
        }
        if !matches!(self.provider, InstalledResourceProvider::Disabled)
            || self.ledger.live_owners() != 0
        {
            return Err(ResourceExecutionError::InvalidGrant);
        }
        let provider = super::provider::ScriptedResourceProvider::new(operations)?;
        let grant = provider.grant();
        self.provider = InstalledResourceProvider::Scripted(provider);
        Ok(grant)
    }

    #[cfg(test)]
    pub(crate) fn provider_events(
        &self,
    ) -> Result<Vec<super::provider::ProviderEvent>, ResourceExecutionError> {
        match &self.provider {
            InstalledResourceProvider::Disabled => Err(ResourceExecutionError::ProviderDisabled),
            InstalledResourceProvider::Scripted(provider) => provider.events(),
        }
    }
}

#[cfg(test)]
mod absence_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource_execution::{ProviderEvent, ScriptOperation, tests::program};
    use jett_types::ResourceHookKind;

    fn constructed(runtime: &mut ResourceTransport, label: i64) -> EvaluatedValue {
        let definition = runtime
            .checked
            .program()
            .checked()
            .resource_hooks
            .values()
            .find(|hook| hook.kind == ResourceHookKind::Construct)
            .unwrap()
            .definition;
        let span = *runtime
            .checked
            .program()
            .checked()
            .call_ownership
            .iter()
            .find(|(_, packet)| packet.target == CheckedInvocationTarget::Resolved(definition))
            .unwrap()
            .0;
        let invocation = runtime.checked.invocation(span).unwrap();
        let descriptor = runtime.checked.descriptor(definition).unwrap();
        let grant = match &runtime.provider {
            InstalledResourceProvider::Scripted(provider) => provider.grant(),
            _ => panic!("selected private provider"),
        };
        let mut value = runtime
            .invoke_hook(
                &invocation,
                &descriptor,
                vec![
                    EvaluatedValue::ordinary(Value::GrantedNetwork(grant)),
                    EvaluatedValue::ordinary(Value::Int64(label)),
                ],
            )
            .unwrap();
        let Value::ResultOk(payload) = value.value else {
            panic!("occupied real construction");
        };
        value.value = *payload;
        value.remove_prefix(PayloadStep::Ok).unwrap();
        value
    }

    #[test]
    fn failed_discard_adoption_completes_its_frame_without_retiring_protected_owner() {
        let checked = program(
            include_str!("fixtures/04_bare_to_view_operation_cleanup.jett"),
            false,
        );
        let mut runtime =
            ResourceTransport::checked_only(checked, ExecutionPurpose::ReferenceRuntime, 1)
                .unwrap();
        runtime.checked_source_active = true;
        runtime
            .install_script(vec![ScriptOperation::Construct {
                label: 401,
                outcome: Ok(()),
            }])
            .unwrap();
        let value = constructed(&mut runtime, 401);
        let outer = runtime.begin_operation().unwrap();
        let mut borrow = runtime.borrow(&value).unwrap();
        runtime.hold_actual(&mut borrow).unwrap();
        assert_eq!(
            runtime.discard_value(value),
            Err(ResourceExecutionError::InvalidBorrow)
        );
        assert_eq!(runtime.operations, [outer.frame]);
        assert_eq!(
            runtime.active_frames.last(),
            Some(&ActiveFrame::Operation(outer.frame))
        );
        assert_eq!(
            runtime.provider_events().unwrap(),
            [ProviderEvent::Constructed(401)]
        );
        assert_eq!(runtime.live_owners(), 1);
        runtime.end_operation(outer, None).unwrap();
        runtime.unwind_all().unwrap();
        assert_eq!(
            runtime.provider_events().unwrap(),
            [
                ProviderEvent::Constructed(401),
                ProviderEvent::Finalized(401)
            ]
        );
        assert_eq!(
            (runtime.live_owners(), runtime.registry_live_count()),
            (0, 0)
        );
    }

    #[test]
    fn interleaved_scope_and_operation_cleanup_preserves_reverse_holder_order() {
        let checked = program(include_str!("fixtures/02_reverse_scope_drop.jett"), false);
        let mut runtime =
            ResourceTransport::checked_only(checked, ExecutionPurpose::ReferenceRuntime, 1)
                .unwrap();
        runtime.checked_source_active = true;
        runtime
            .install_script(vec![
                ScriptOperation::Construct {
                    label: 201,
                    outcome: Ok(()),
                },
                ScriptOperation::Construct {
                    label: 202,
                    outcome: Ok(()),
                },
            ])
            .unwrap();
        let operation = runtime.begin_operation().unwrap();
        let first = constructed(&mut runtime, 201);
        runtime.push_scope();
        let mut second = constructed(&mut runtime, 202);
        let holder = runtime.temporary(runtime.scope_frame().unwrap()).unwrap();
        runtime
            .ledger
            .transfer(&mut second.custody, holder)
            .unwrap();
        assert!(matches!(
            runtime.active_frames.last(),
            Some(ActiveFrame::Scope(_))
        ));
        runtime.unwind_all().unwrap();
        assert_eq!(
            runtime.provider_events().unwrap(),
            [
                ProviderEvent::Constructed(201),
                ProviderEvent::Constructed(202),
                ProviderEvent::Finalized(202),
                ProviderEvent::Finalized(201)
            ]
        );
        assert_eq!(
            (runtime.live_owners(), runtime.registry_live_count()),
            (0, 0)
        );
        assert!(
            runtime.operations.is_empty()
                && runtime.scopes.is_empty()
                && runtime.active_frames.is_empty()
        );
        drop((operation, first, second));
    }

    #[test]
    fn reached_capability_free_close_refuses_worker_purpose_before_owner_retirement() {
        for purpose in [
            ExecutionPurpose::NamespaceConstant,
            ExecutionPurpose::ExplicitComptime,
            ExecutionPurpose::Verify,
            ExecutionPurpose::Property,
        ] {
            let program = program(
                include_str!("fixtures/09_capability_free_close_target.jett"),
                false,
            );
            let mut runtime = ResourceTransport::checked_only(
                program.clone(),
                ExecutionPurpose::ReferenceRuntime,
                1,
            )
            .unwrap();
            runtime.checked_source_active = true;
            let grant = runtime
                .install_script(vec![ScriptOperation::Construct {
                    label: 901,
                    outcome: Ok(()),
                }])
                .unwrap();
            let definition = |kind| {
                program
                    .checked()
                    .resource_hooks
                    .values()
                    .find(|hook| hook.kind == kind)
                    .unwrap()
                    .definition
            };
            let occurrence = |definition| {
                *program
                    .checked()
                    .call_ownership
                    .iter()
                    .find(|(_, packet)| {
                        packet.target == CheckedInvocationTarget::Resolved(definition)
                    })
                    .unwrap()
                    .0
            };
            // Exact source kernel packets and a real registry publication create
            // custody; no Resource carrier or owning ticket is fabricated.
            let create = runtime
                .checked
                .invocation(occurrence(definition(ResourceHookKind::Construct)))
                .unwrap();
            let create_descriptor = runtime
                .checked
                .descriptor(definition(ResourceHookKind::Construct))
                .unwrap();
            let mut token = runtime
                .invoke_hook(
                    &create,
                    &create_descriptor,
                    vec![
                        EvaluatedValue::ordinary(Value::GrantedNetwork(grant)),
                        EvaluatedValue::ordinary(Value::Int64(901)),
                    ],
                )
                .unwrap();
            let Value::ResultOk(value) = token.value else {
                panic!("occupied factory result");
            };
            token.value = *value;
            token.remove_prefix(PayloadStep::Ok).unwrap();
            let close = runtime
                .checked
                .invocation(occurrence(definition(ResourceHookKind::Close)))
                .unwrap();
            let close_descriptor = runtime
                .checked
                .descriptor(definition(ResourceHookKind::Close))
                .unwrap();
            runtime.checked.replace_purpose(purpose);
            assert_eq!(
                runtime
                    .invoke_hook(&close, &close_descriptor, vec![token])
                    .unwrap_err(),
                ResourceExecutionError::WrongPurpose
            );
            assert_eq!(
                runtime.provider_events().unwrap(),
                [ProviderEvent::Constructed(901)]
            );
            assert_eq!(runtime.live_owners(), 1);
            assert_eq!(runtime.registry_live_count(), 1);
            runtime.unwind_all().unwrap();
            assert_eq!(
                runtime.provider_events().unwrap(),
                [
                    ProviderEvent::Constructed(901),
                    ProviderEvent::Finalized(901)
                ]
            );
            assert_eq!(runtime.live_owners(), 0);
            assert_eq!(runtime.registry_live_count(), 0);
        }
    }

    #[test]
    fn operation_cleanup_error_overrides_failed_output_preservation_and_retires_once() {
        let checked = program(
            include_str!("fixtures/04_bare_to_view_operation_cleanup.jett"),
            false,
        );
        let mut runtime =
            ResourceTransport::checked_only(checked, ExecutionPurpose::ReferenceRuntime, 1)
                .unwrap();
        runtime.checked_source_active = true;
        runtime
            .install_script(vec![ScriptOperation::Construct {
                label: 401,
                outcome: Ok(()),
            }])
            .unwrap();
        let operation = runtime.begin_operation().unwrap();
        let mut value = constructed(&mut runtime, 401);
        let Value::Resource(carrier) = &value.value else {
            panic!("actual occupied resource construction");
        };
        // Private corruption control: retire the actual registry entry without
        // retiring the ledger obligation. No source program can perform this.
        runtime
            .registry
            .close(carrier.key, carrier.resource.registry_type)
            .unwrap();
        runtime.next_temporary = usize::MAX;
        assert_eq!(
            runtime.end_operation(operation, Some(&mut value)),
            Err(ResourceExecutionError::Registry(
                jett_runtime::RegistryError::StaleGeneration
            ))
        );
        assert!(runtime.operations.is_empty());
        assert_eq!(
            runtime.provider_events().unwrap(),
            [
                ProviderEvent::Constructed(401),
                ProviderEvent::Finalized(401)
            ]
        );
        assert_eq!(
            (runtime.live_owners(), runtime.registry_live_count()),
            (0, 0)
        );
        runtime.unwind_all().unwrap();
        assert!(
            runtime.active_frames.is_empty()
                && runtime.operations.is_empty()
                && runtime.scopes.is_empty()
        );
        assert_eq!(
            runtime.provider_events().unwrap(),
            [
                ProviderEvent::Constructed(401),
                ProviderEvent::Finalized(401)
            ]
        );
    }

    #[test]
    fn discard_cleanup_error_overrides_adoption_error_without_losing_parent_custody() {
        let checked = program(
            include_str!("fixtures/04_bare_to_view_operation_cleanup.jett"),
            false,
        );
        let mut runtime =
            ResourceTransport::checked_only(checked, ExecutionPurpose::ReferenceRuntime, 1)
                .unwrap();
        runtime.checked_source_active = true;
        runtime
            .install_script(vec![ScriptOperation::Construct {
                label: 401,
                outcome: Ok(()),
            }])
            .unwrap();
        let value = constructed(&mut runtime, 401);
        // Private completion fault: holder allocation fails while a distinct
        // earlier cleanup error remains queued. Neither is provider authority.
        runtime.next_temporary = usize::MAX;
        runtime.cleanup_error = Some(ResourceExecutionError::InvalidFrame);
        assert_eq!(
            runtime.discard_value(value),
            Err(ResourceExecutionError::InvalidFrame)
        );
        assert!(runtime.operations.is_empty());
        assert_eq!(runtime.active_frames.len(), 1);
        assert_eq!(
            (runtime.live_owners(), runtime.registry_live_count()),
            (1, 1)
        );
        assert_eq!(
            runtime.provider_events().unwrap(),
            [ProviderEvent::Constructed(401)]
        );
        runtime.unwind_all().unwrap();
        assert!(runtime.active_frames.is_empty() && runtime.scopes.is_empty());
        assert_eq!(
            runtime.provider_events().unwrap(),
            [
                ProviderEvent::Constructed(401),
                ProviderEvent::Finalized(401)
            ]
        );
        assert_eq!(
            (runtime.live_owners(), runtime.registry_live_count()),
            (0, 0)
        );
    }
}
