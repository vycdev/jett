//! Complete Source wire consistency. The compiler retains Source/CFG authority.
use super::{ResourceLayoutError, schema::*, validation::slot_kind};
use std::collections::HashSet;

fn get<T>(values: &[T], id: u32) -> Result<&T, ResourceLayoutError> {
    values
        .get(id as usize)
        .ok_or(ResourceLayoutError::InvalidReference)
}
fn occupied(layout: &WireLayout, shape: u32) -> Result<bool, ResourceLayoutError> {
    // Shape validation already established child-before-parent and Resource-free
    // Result failure arms. Walk iteratively so bounded foreign nesting cannot
    // turn table preflight into recursive stack growth.
    let mut current = shape;
    loop {
        match *get(&layout.shapes, current)? {
            NativeShape::Resource { .. } => return Ok(true),
            NativeShape::Optional { child } => current = child,
            NativeShape::Result { ok, .. } => current = ok,
            _ => return Ok(false),
        }
    }
}
fn frame<'a>(
    layout: &'a WireLayout,
    id: u32,
    function: u32,
    role: NativeFrameRole,
) -> Result<&'a NativeFrame, ResourceLayoutError> {
    let result = get(&layout.frames, id)?;
    if result.site.function != function || result.role != role {
        return Err(ResourceLayoutError::FrameMismatch);
    }
    Ok(result)
}
fn source_kind(
    layout: &WireLayout,
    source: NativeLoanSource,
    function: u32,
) -> Result<u32, ResourceLayoutError> {
    match source {
        NativeLoanSource::ExistingBorrow { operation } => {
            let record = get(&layout.operations, operation)?;
            if record.site.function != function {
                return Err(ResourceLayoutError::FrameMismatch);
            }
            let NativeOperation::Borrow { source, .. } = record.operation else {
                return Err(ResourceLayoutError::OperationMismatch);
            };
            slot_kind(layout, get(&layout.slots, source)?)
        }
        NativeLoanSource::IncomingViewFormal { scope, parameter } => {
            let scope = frame(layout, scope, function, NativeFrameRole::Scope)?;
            let signature = get(&layout.signatures, scope.signature)?;
            let formal = get(&signature.parameters, parameter)?;
            if formal.access != NativeAccess::View {
                return Err(ResourceLayoutError::OperationMismatch);
            }
            match *get(&layout.shapes, formal.shape)? {
                NativeShape::Resource { kind } => Ok(kind),
                _ => Err(ResourceLayoutError::ShapeMismatch),
            }
        }
    }
}

pub(super) fn invocation(
    layout: &WireLayout,
    record: &NativeOperationRecord,
    caller_frame: u32,
    callee: u32,
    signature: u32,
    callee_scope: u32,
    source: &NativeSourceInvocation,
) -> Result<(), ResourceLayoutError> {
    if layout.version != 2 {
        return Err(ResourceLayoutError::OperationMismatch);
    }
    frame(
        layout,
        caller_frame,
        record.site.function,
        NativeFrameRole::Operation,
    )?;
    let scope = frame(layout, callee_scope, callee, NativeFrameRole::Scope)?;
    let signature_record = get(&layout.signatures, signature)?;
    if scope.signature != signature
        || source.formals.len() != signature_record.parameters.len()
        || source.evaluation_order.len() != source.formals.len()
    {
        return Err(ResourceLayoutError::OperationMismatch);
    }
    let mut source_indices = super::wire::storage::<bool>(source.formals.len())?;
    source_indices.resize(source.formals.len(), false);
    let mut caller_slots = HashSet::new();
    let mut callee_slots = HashSet::new();
    caller_slots
        .try_reserve(source.formals.len())
        .map_err(|_| ResourceLayoutError::CapacityExhausted)?;
    callee_slots
        .try_reserve(source.formals.len())
        .map_err(|_| ResourceLayoutError::CapacityExhausted)?;
    for (index, formal) in source.formals.iter().enumerate() {
        let target = &signature_record.parameters[index];
        let source_index = formal.source_index as usize;
        if formal.parameter as usize != index
            || source_index >= source.formals.len()
            || source_indices[source_index]
            || source.evaluation_order[source_index] != formal.parameter
            || formal.callee_shape != target.shape
            || formal.access != target.access
        {
            return Err(ResourceLayoutError::OperationMismatch);
        }
        source_indices[source_index] = true;
        get(&layout.shapes, formal.actual_shape)?;
        match formal.value {
            NativeSourceValue::Ordinary => {
                if occupied(layout, formal.actual_shape)? || occupied(layout, formal.callee_shape)?
                {
                    return Err(ResourceLayoutError::ShapeMismatch);
                }
            }
            NativeSourceValue::Owned {
                caller_argument_slot,
                callee_parameter_slot,
            } => {
                let caller = get(&layout.slots, caller_argument_slot)?;
                let target = get(&layout.slots, callee_parameter_slot)?;
                if formal.syntax != NativeSourceSyntax::Bare
                    || formal.effect != NativeSourceEffect::TransferOwned
                    || formal.access != NativeAccess::Owned
                    || formal.actual_shape != formal.callee_shape
                    || caller.frame != caller_frame
                    || target.frame != callee_scope
                    || caller.shape != formal.actual_shape
                    || target.shape != formal.callee_shape
                    || caller.path != target.path
                    || !caller_slots.insert(caller_argument_slot)
                    || !callee_slots.insert(callee_parameter_slot)
                {
                    return Err(ResourceLayoutError::OperationMismatch);
                }
                if slot_kind(layout, caller)? != slot_kind(layout, target)? {
                    return Err(ResourceLayoutError::ShapeMismatch);
                }
            }
            NativeSourceValue::ResidentView { source } => {
                let NativeShape::Resource { kind } = *get(&layout.shapes, formal.callee_shape)?
                else {
                    return Err(ResourceLayoutError::ShapeMismatch);
                };
                let tuple = matches!(
                    (formal.syntax, formal.effect),
                    (
                        NativeSourceSyntax::Bare,
                        NativeSourceEffect::RelinquishOwned
                    ) | (
                        NativeSourceSyntax::WrittenView,
                        NativeSourceEffect::RetainBorrow
                    )
                );
                if !tuple
                    || formal.access != NativeAccess::View
                    || formal.actual_shape != formal.callee_shape
                    || source_kind(layout, source, record.site.function)? != kind
                {
                    return Err(ResourceLayoutError::OperationMismatch);
                }
            }
        }
    }
    match &source.result {
        NativeSourceResult::Ordinary { shape } => {
            if *shape != signature_record.result
                || occupied(layout, *shape)?
                || source.callee_return.is_some()
                || !scope.parents.contains(&NativeParent::Frame(caller_frame))
            {
                return Err(ResourceLayoutError::OperationMismatch);
            }
        }
        NativeSourceResult::Owned {
            shape,
            caller_destination_frame,
            caller_destination_slot,
            callee_return_frame,
            permitted_return_slots,
        } => {
            let return_frame = frame(
                layout,
                *callee_return_frame,
                callee,
                NativeFrameRole::Return,
            )?;
            let destination = get(&layout.slots, *caller_destination_slot)?;
            let destination_frame = get(&layout.frames, *caller_destination_frame)?;
            if *shape != signature_record.result
                || !occupied(layout, *shape)?
                || source.callee_return != Some(*callee_return_frame)
                || return_frame.signature != signature
                || !return_frame
                    .parents
                    .contains(&NativeParent::Frame(caller_frame))
                || !scope
                    .parents
                    .contains(&NativeParent::Frame(*callee_return_frame))
                || destination.frame != *caller_destination_frame
                || destination.shape != *shape
                || destination_frame.site.function != record.site.function
                || permitted_return_slots.is_empty()
                || permitted_return_slots
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
            {
                return Err(ResourceLayoutError::OperationMismatch);
            }
            for id in permitted_return_slots {
                let slot = get(&layout.slots, *id)?;
                if slot.frame != *callee_return_frame
                    || slot.shape != *shape
                    || slot.path != destination.path
                    || slot_kind(layout, slot)? != slot_kind(layout, destination)?
                {
                    return Err(ResourceLayoutError::ShapeMismatch);
                }
            }
        }
    }
    Ok(())
}

pub(super) fn operation(
    layout: &WireLayout,
    record: &NativeOperationRecord,
) -> Result<(), ResourceLayoutError> {
    if layout.version != 2 {
        return Err(ResourceLayoutError::OperationMismatch);
    }
    let get_frame = |id| -> Result<&NativeFrame, ResourceLayoutError> {
        let result = get(&layout.frames, id)?;
        if result.site.function != record.site.function {
            return Err(ResourceLayoutError::FrameMismatch);
        }
        Ok(result)
    };
    match record.operation {
        NativeOperation::TakeFailureCompanion {
            frame,
            source_sum_slot,
            failure_shape,
        } => {
            get_frame(frame)?;
            let slot = get(&layout.slots, source_sum_slot)?;
            let NativeShape::Result { fail, .. } = *get(&layout.shapes, slot.shape)? else {
                return Err(ResourceLayoutError::ShapeMismatch);
            };
            if fail != failure_shape
                || occupied(layout, fail)?
                || slot.path != [NativePayloadStep::Ok]
                || get(&layout.frames, slot.frame)?.site.function != record.site.function
            {
                return Err(ResourceLayoutError::ShapeMismatch);
            }
        }
        NativeOperation::PublishReturn {
            frame,
            source_return_slot,
        } => {
            let scope = get_frame(frame)?;
            let slot = get(&layout.slots, source_return_slot)?;
            let return_frame = get_frame(slot.frame)?;
            if scope.role != NativeFrameRole::Scope
                || return_frame.role != NativeFrameRole::Return
                || scope.signature != return_frame.signature
                || slot.shape != get(&layout.signatures, return_frame.signature)?.result
                || !scope.parents.contains(&NativeParent::Frame(slot.frame))
            {
                return Err(ResourceLayoutError::FrameMismatch);
            }
        }
        NativeOperation::CreateAbsentSum {
            frame,
            destination_slot,
        } => {
            get_frame(frame)?;
            let slot = get(&layout.slots, destination_slot)?;
            if slot.frame != frame
                || slot.path != [NativePayloadStep::Some]
                || !matches!(get(&layout.shapes,slot.shape)?,NativeShape::Optional { child }
                    if matches!(get(&layout.shapes,*child)?,NativeShape::Resource { .. }))
            {
                return Err(ResourceLayoutError::ShapeMismatch);
            }
        }
        NativeOperation::CreateFailureSum {
            frame,
            destination_slot,
            failure_shape,
        } => {
            get_frame(frame)?;
            let slot = get(&layout.slots, destination_slot)?;
            let NativeShape::Result { fail, .. } = *get(&layout.shapes, slot.shape)? else {
                return Err(ResourceLayoutError::ShapeMismatch);
            };
            if slot.frame != frame
                || slot.path != [NativePayloadStep::Ok]
                || fail != failure_shape
                || occupied(layout, fail)?
            {
                return Err(ResourceLayoutError::ShapeMismatch);
            }
        }
        _ => return Err(ResourceLayoutError::OperationMismatch),
    }
    Ok(())
}
