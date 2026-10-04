//! Layout consistency only. Original Source/CFG proof is the compiler's obligation.

use super::ResourceLayoutError;
use super::schema::*;
use super::wire::storage;
use std::collections::{HashMap, HashSet};

fn get<T>(values: &[T], ordinal: u32) -> Result<&T, ResourceLayoutError> {
    values
        .get(ordinal as usize)
        .ok_or(ResourceLayoutError::InvalidReference)
}

fn validate_shapes(layout: &WireLayout) -> Result<(), ResourceLayoutError> {
    let mut unique = HashSet::new();
    unique
        .try_reserve(layout.shapes.len())
        .map_err(|_| ResourceLayoutError::CapacityExhausted)?;
    let mut occupied = storage::<bool>(layout.shapes.len())?;
    for (index, shape) in layout.shapes.iter().enumerate() {
        if !unique.insert(shape) {
            return Err(ResourceLayoutError::NonCanonical);
        }
        let contains_resource = match *shape {
            NativeShape::Integer { bits, .. } if [8, 16, 32, 64].contains(&bits) => false,
            NativeShape::Float { bits } if [32, 64].contains(&bits) => false,
            NativeShape::Integer { .. } | NativeShape::Float { .. } => {
                return Err(ResourceLayoutError::UnsupportedLayout);
            }
            NativeShape::Bool
            | NativeShape::String
            | NativeShape::Nothing
            | NativeShape::Network => false,
            NativeShape::Resource { kind } => {
                get(&layout.kinds, kind)?;
                true
            }
            NativeShape::Optional { child } => {
                if child as usize >= index {
                    return Err(ResourceLayoutError::InvalidReference);
                }
                let has_resource = occupied[child as usize];
                if has_resource
                    && !matches!(layout.shapes[child as usize], NativeShape::Resource { .. })
                {
                    return Err(ResourceLayoutError::UnsupportedLayout);
                }
                has_resource
            }
            NativeShape::Result { ok, fail } => {
                if ok as usize >= index || fail as usize >= index {
                    return Err(ResourceLayoutError::InvalidReference);
                }
                if occupied[fail as usize]
                    || (occupied[ok as usize]
                        && !matches!(layout.shapes[ok as usize], NativeShape::Resource { .. }))
                {
                    return Err(ResourceLayoutError::UnsupportedLayout);
                }
                occupied[ok as usize]
            }
            NativeShape::HookDescriptor { hook } => {
                get(&layout.hooks, hook)?;
                false
            }
        };
        occupied.push(contains_resource);
    }
    Ok(())
}

fn validate_signatures(layout: &WireLayout) -> Result<(), ResourceLayoutError> {
    let mut unique = HashSet::new();
    unique
        .try_reserve(layout.signatures.len())
        .map_err(|_| ResourceLayoutError::CapacityExhausted)?;
    for signature in &layout.signatures {
        get(&layout.shapes, signature.result)?;
        for formal in &signature.parameters {
            get(&layout.shapes, formal.shape)?;
        }
        if !unique.insert((signature.parameters.as_slice(), signature.result)) {
            return Err(ResourceLayoutError::NonCanonical);
        }
    }
    Ok(())
}

fn shape_is(layout: &WireLayout, ordinal: u32, expected: NativeShape) -> bool {
    layout.shapes.get(ordinal as usize) == Some(&expected)
}

fn validate_hooks(layout: &WireLayout) -> Result<(), ResourceLayoutError> {
    for hook in &layout.hooks {
        get(&layout.kinds, hook.kind)?;
        let signature = get(&layout.signatures, hook.signature)?;
        let formal = |index: usize, shape: NativeShape, access| {
            signature.parameters.get(index).is_some_and(|parameter| {
                parameter.access == access && shape_is(layout, parameter.shape, shape)
            })
        };
        let resource = NativeShape::Resource { kind: hook.kind };
        let integer = NativeShape::Integer {
            bits: 64,
            signed: true,
        };
        let returns_result = |ok: NativeShape| {
            let NativeShape::Result { ok: value, fail } = layout.shapes[signature.result as usize]
            else {
                return false;
            };
            shape_is(layout, value, ok) && shape_is(layout, fail, NativeShape::String)
        };
        let matches = match hook.recipe {
            NativeRecipe::NetworkFactory => {
                signature.parameters.len() == 2
                    && formal(0, NativeShape::Network, NativeAccess::View)
                    && formal(1, integer, NativeAccess::Owned)
                    && returns_result(resource)
            }
            NativeRecipe::NetworkBorrow => {
                signature.parameters.len() == 2
                    && formal(0, NativeShape::Network, NativeAccess::View)
                    && formal(1, resource, NativeAccess::View)
                    && returns_result(integer)
            }
            NativeRecipe::Finalize => {
                signature.parameters.len() == 1
                    && formal(0, resource, NativeAccess::Owned)
                    && shape_is(layout, signature.result, NativeShape::Nothing)
            }
        };
        if !matches {
            return Err(ResourceLayoutError::RecipeMismatch);
        }
    }
    Ok(())
}

fn validate_frames(layout: &WireLayout) -> Result<(), ResourceLayoutError> {
    let mut signatures = HashMap::new();
    signatures
        .try_reserve(layout.frames.len())
        .map_err(|_| ResourceLayoutError::CapacityExhausted)?;
    for frame in &layout.frames {
        get(&layout.signatures, frame.signature)?;
        if let Some(previous) = signatures.insert(frame.site.function, frame.signature) {
            if previous != frame.signature {
                return Err(ResourceLayoutError::FrameMismatch);
            }
        }
        if frame.parents.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(ResourceLayoutError::NonCanonical);
        }
        for parent in &frame.parents {
            if let NativeParent::Frame(index) = parent {
                get(&layout.frames, *index)?;
            }
        }
    }
    Ok(())
}

pub(super) fn slot_kind(
    layout: &WireLayout,
    slot: &NativeSlot,
) -> Result<u32, ResourceLayoutError> {
    let mut shape = get(&layout.shapes, slot.shape)?;
    for step in &slot.path {
        shape = match (shape, step) {
            (NativeShape::Optional { child }, NativePayloadStep::Some) => {
                get(&layout.shapes, *child)?
            }
            (NativeShape::Result { ok, .. }, NativePayloadStep::Ok) => get(&layout.shapes, *ok)?,
            (NativeShape::Result { fail, .. }, NativePayloadStep::Fail) => {
                get(&layout.shapes, *fail)?
            }
            _ => return Err(ResourceLayoutError::PayloadPath),
        };
    }
    if let NativeShape::Resource { kind } = shape {
        Ok(*kind)
    } else {
        Err(ResourceLayoutError::PayloadPath)
    }
}

fn source_kind(layout: &WireLayout, source: NativeLoanSource) -> Result<u32, ResourceLayoutError> {
    match source {
        NativeLoanSource::ExistingBorrow { operation } => {
            let record = get(&layout.operations, operation)?;
            let NativeOperation::Borrow { source, .. } = record.operation else {
                return Err(ResourceLayoutError::OperationMismatch);
            };
            slot_kind(layout, get(&layout.slots, source)?)
        }
        NativeLoanSource::IncomingViewFormal { scope, parameter } => {
            let frame = get(&layout.frames, scope)?;
            if frame.role != NativeFrameRole::Scope {
                return Err(ResourceLayoutError::FrameMismatch);
            }
            let signature = get(&layout.signatures, frame.signature)?;
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

fn validate_operation(
    layout: &WireLayout,
    record: &NativeOperationRecord,
) -> Result<(), ResourceLayoutError> {
    let frame = |index| -> Result<&NativeFrame, ResourceLayoutError> {
        let frame = get(&layout.frames, index)?;
        if frame.site.function != record.site.function {
            return Err(ResourceLayoutError::FrameMismatch);
        }
        Ok(frame)
    };
    let slot = |index| get(&layout.slots, index);
    let kind = |index| slot_kind(layout, slot(index)?);
    let hook = |index, recipe, kind| -> Result<(), ResourceLayoutError> {
        let hook = get(&layout.hooks, index)?;
        if hook.recipe != recipe || hook.kind != kind {
            return Err(ResourceLayoutError::OperationMismatch);
        }
        Ok(())
    };
    let same_slots = |a, b| -> Result<(), ResourceLayoutError> {
        if a == b {
            return Err(ResourceLayoutError::OperationMismatch);
        }
        let (a, b) = (slot(a)?, slot(b)?);
        if a.shape != b.shape || a.path != b.path {
            return Err(ResourceLayoutError::ShapeMismatch);
        }
        Ok(())
    };
    use NativeOperation::*;
    match record.operation {
        Acquire {
            frame: f,
            hook: h,
            destination,
        } => {
            frame(f)?;
            hook(h, NativeRecipe::NetworkFactory, kind(destination)?)?;
        }
        Transfer {
            frame: f,
            source,
            destination,
        } => {
            frame(f)?;
            same_slots(source, destination)?;
        }
        Borrow {
            frame: f,
            source,
            lease_frame,
        } => {
            frame(f)?;
            kind(source)?;
            get(&layout.frames, lease_frame)?;
        }
        BoundedBorrowUse {
            frame: f,
            source,
            callee_frame,
        } => {
            frame(f)?;
            let kind = source_kind(layout, source)?;
            let callee = get(&layout.frames, callee_frame)?;
            if callee.role != NativeFrameRole::Scope {
                return Err(ResourceLayoutError::FrameMismatch);
            }
            let signature = get(&layout.signatures, callee.signature)?;
            if !signature.parameters.iter().any(|p| {
                p.access == NativeAccess::View
                    && shape_is(layout, p.shape, NativeShape::Resource { kind })
            }) {
                return Err(ResourceLayoutError::OperationMismatch);
            }
        }
        EndBorrow { frame: f, borrow } => {
            frame(f)?;
            if !matches!(get(&layout.operations, borrow)?.operation, Borrow { .. }) {
                return Err(ResourceLayoutError::OperationMismatch);
            }
        }
        InvokeBorrow {
            frame: f,
            hook: h,
            source,
        } => {
            frame(f)?;
            hook(h, NativeRecipe::NetworkBorrow, source_kind(layout, source)?)?;
        }
        Close {
            frame: f,
            hook: h,
            source,
        } => {
            frame(f)?;
            hook(h, NativeRecipe::Finalize, kind(source)?)?;
        }
        Drop { frame: f, source } => {
            frame(f)?;
            kind(source)?;
        }
        SumAdopt {
            frame: f,
            source,
            destination,
        } => {
            frame(f)?;
            let (source_slot, destination_slot) = (slot(source)?, slot(destination)?);
            if !source_slot.path.is_empty()
                || destination_slot.path.len() != 1
                || kind(source)? != kind(destination)?
            {
                return Err(ResourceLayoutError::ShapeMismatch);
            }
        }
        SumTake {
            frame: f,
            source,
            destination,
        } => {
            frame(f)?;
            let (source_slot, destination_slot) = (slot(source)?, slot(destination)?);
            if source_slot.path.len() != 1
                || !destination_slot.path.is_empty()
                || kind(source)? != kind(destination)?
            {
                return Err(ResourceLayoutError::ShapeMismatch);
            }
        }
        SumDrop { frame: f, source } => {
            frame(f)?;
            if slot(source)?.path.len() != 1 {
                return Err(ResourceLayoutError::ShapeMismatch);
            }
        }
        Replace {
            frame: f,
            old,
            replacement,
        } => {
            frame(f)?;
            same_slots(old, replacement)?;
        }
        Complete { frame: f } => {
            frame(f)?;
        }
        Descriptor { hook } => {
            get(&layout.hooks, hook)?;
        }
        InvokeDescriptor {
            hook,
            signature,
            target,
        } => {
            let declared = get(&layout.hooks, hook)?;
            if declared.signature != signature {
                return Err(ResourceLayoutError::OperationMismatch);
            }
            let target = get(&layout.operations, target)?;
            let (target_hook, recipe) = match target.operation {
                Acquire { hook, .. } => (hook, NativeRecipe::NetworkFactory),
                InvokeBorrow { hook, .. } => (hook, NativeRecipe::NetworkBorrow),
                Close { hook, .. } => (hook, NativeRecipe::Finalize),
                _ => return Err(ResourceLayoutError::OperationMismatch),
            };
            if target_hook != hook || declared.recipe != recipe {
                return Err(ResourceLayoutError::OperationMismatch);
            }
        }
        InvokeSourceFunction {
            frame: f,
            callee,
            signature,
            callee_scope,
        } => {
            frame(f)?;
            let scope = get(&layout.frames, callee_scope)?;
            if scope.role != NativeFrameRole::Scope
                || scope.site.function != callee
                || scope.signature != signature
            {
                return Err(ResourceLayoutError::FrameMismatch);
            }
        }
    }
    Ok(())
}

/// Returns the complete slot-kind projection before the issuer allocates identities.
pub(super) fn validate(layout: &WireLayout) -> Result<Vec<usize>, ResourceLayoutError> {
    validate_shapes(layout)?;
    validate_signatures(layout)?;
    validate_hooks(layout)?;
    validate_frames(layout)?;
    let mut slot_kinds = storage(layout.slots.len())?;
    for slot in &layout.slots {
        get(&layout.frames, slot.frame)?;
        slot_kinds.push(slot_kind(layout, slot)? as usize);
    }
    for operation in &layout.operations {
        validate_operation(layout, operation)?;
    }
    Ok(slot_kinds)
}
