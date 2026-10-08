//! Private matched-archive exports. Parent wiring requires BOTH private cfgs.
//! Observations borrow the live context and never clear its failure channels.

use super::*;
use std::mem::{align_of, offset_of, size_of};

const MAX_BYTES: usize = 1024 * 1024;
const MAX_EVENTS: usize = 8192;

#[repr(C)]
pub(super) struct ObjectManifest {
    layout: *const u8,
    layout_length: u64,
    entry_function: u32,
    entry_signature: u32,
    entry_scope: u32,
    reserved: u32,
}

#[repr(C)]
pub(super) struct ArchiveInfo {
    runtime_abi: u32,
    test_protocol: u32,
    layout_wire: u32,
    pointer_bits: u32,
    archive_profile: u32,
    reserved: [u32; 3],
}

const _: () = {
    assert!(size_of::<ObjectManifest>() == 32 && align_of::<ObjectManifest>() == 8);
    assert!(offset_of!(ObjectManifest, entry_function) == 16);
    assert!(size_of::<ArchiveInfo>() == 32 && align_of::<ArchiveInfo>() == 4);
    assert!(size_of::<NativeResourceCompletion>() == 24);
    assert!(size_of::<NativeResourceCounts>() == 72);
    assert!(size_of::<NativeTestEvent>() == 24);
};

#[derive(Clone, Copy)]
struct Region {
    start: usize,
    end: usize,
}

fn region<T>(pointer: *const T, count: usize) -> ResourceResult<Region> {
    if pointer.is_null() || (pointer as usize) % align_of::<T>() != 0 {
        return Err(NativeResourceError::WrongOperation);
    }
    let length = size_of::<T>()
        .checked_mul(count)
        .filter(|length| *length <= isize::MAX as usize)
        .ok_or(NativeResourceError::Capacity)?;
    let start = pointer as usize;
    let end = start
        .checked_add(length)
        .ok_or(NativeResourceError::Capacity)?;
    Ok(Region { start, end })
}

fn disjoint(regions: &[Region]) -> ResourceResult<()> {
    for (index, left) in regions.iter().enumerate() {
        for right in &regions[index + 1..] {
            if left.start < right.end && right.start < left.end {
                return Err(NativeResourceError::WrongOperation);
            }
        }
    }
    Ok(())
}

fn checked_length(length: u64, maximum: usize) -> ResourceResult<usize> {
    usize::try_from(length)
        .ok()
        .filter(|length| *length <= maximum)
        .ok_or(NativeResourceError::Capacity)
}

fn result_call(
    out_result: *mut JettRuntimeResultV1,
    operation: impl FnOnce() -> ResourceResult<()>,
) -> JettRuntimeStatusV1 {
    if region(out_result, 1).is_err() {
        return JettRuntimeStatusV1::INVALID_ARGUMENT;
    }
    complete_call(out_result, || match operation() {
        Ok(()) => JettRuntimeResultV1::ok(),
        Err(error) => JettRuntimeResultV1::failure(error.status(), REFUSED),
    })
}

fn outputs<T>(
    context: *const JettRuntimeContextV1,
    output: *mut T,
    out_result: *mut JettRuntimeResultV1,
) -> ResourceResult<()> {
    disjoint(&[
        region(context, 1)?,
        region(output, 1)?,
        region(out_result, 1)?,
    ])
}

fn context_call<T>(
    context: *const JettRuntimeContextV1,
    output: *mut T,
    out_result: *mut JettRuntimeResultV1,
    operation: impl FnOnce(&AuthenticatedResourceContext) -> ResourceResult<()>,
) -> JettRuntimeStatusV1 {
    // Do not let complete_call write through a pointer overlapping the context.
    if outputs(context, output, out_result).is_err() {
        return JettRuntimeStatusV1::INVALID_ARGUMENT;
    }
    result_call(out_result, || {
        operation(&AuthenticatedResourceContext::acquire(context)?)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_test_archive_info(
    out_info: *mut ArchiveInfo,
    out_result: *mut JettRuntimeResultV1,
) -> JettRuntimeStatusV1 {
    if region(out_info, 1)
        .and_then(|info| disjoint(&[info, region(out_result, 1)?]))
        .is_err()
    {
        return JettRuntimeStatusV1::INVALID_ARGUMENT;
    }
    result_call(out_result, || {
        // This shared codec constant is a prerequisite of the v2 runtime packet.
        unsafe {
            ptr::write(
                out_info,
                ArchiveInfo {
                    runtime_abi: JETT_RUNTIME_ABI_VERSION_V1,
                    test_protocol: 1,
                    layout_wire: crate::resource_custody::NATIVE_RESOURCE_LAYOUT_WIRE_VERSION,
                    pointer_bits: usize::BITS,
                    archive_profile: u32::from(!cfg!(debug_assertions)),
                    reserved: [0; 3],
                },
            );
        }
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_test_install(
    context: *const JettRuntimeContextV1,
    manifest: *const ObjectManifest,
    script: *const u8,
    script_length: u64,
    out_result: *mut JettRuntimeResultV1,
) -> JettRuntimeStatusV1 {
    // Validate result/context separation before the result writer is entered.
    if outputs(context, manifest.cast_mut(), out_result).is_err() {
        return JettRuntimeStatusV1::INVALID_ARGUMENT;
    }
    let manifest = unsafe { &*manifest };
    let lengths = (|| {
        let layout_length = checked_length(manifest.layout_length, MAX_BYTES)?;
        let script_length = checked_length(script_length, MAX_BYTES)?;
        if layout_length == 0 || script_length == 0 {
            return Err(NativeResourceError::WrongOperation);
        }
        disjoint(&[
            region(context, 1)?,
            region(manifest, 1)?,
            region(out_result, 1)?,
            region(manifest.layout, layout_length)?,
            region(script, script_length)?,
        ])?;
        Ok((layout_length, script_length))
    })();
    let (layout_length, script_length) = match lengths {
        Ok(lengths) => lengths,
        // A refusal must not overwrite any readonly input through out_result.
        Err(_) => return JettRuntimeStatusV1::INVALID_ARGUMENT,
    };
    result_call(out_result, || {
        let authenticated = AuthenticatedResourceContext::acquire(context)?;
        if manifest.reserved != 0 {
            return Err(NativeResourceError::InvalidEntry);
        }
        let layout = unsafe { slice::from_raw_parts(manifest.layout, layout_length) };
        let version = layout.get(8..12).ok_or(NativeResourceError::InvalidEntry)?;
        if ![2u32, 3].iter().any(|known| version == known.to_le_bytes()) {
            return Err(NativeResourceError::InvalidEntry);
        }
        let script =
            DecodedScript::decode(unsafe { slice::from_raw_parts(script, script_length) })?;
        install_scripted(
            &authenticated,
            layout,
            NativeEntry {
                function: manifest.entry_function,
                signature: manifest.entry_signature,
                scope: manifest.entry_scope,
            },
            script,
        )?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_test_entry_begin(
    context: *const JettRuntimeContextV1,
    out_attempt: *mut u64,
    out_result: *mut JettRuntimeResultV1,
) -> JettRuntimeStatusV1 {
    context_call(context, out_attempt, out_result, |authenticated| {
        authenticated.with_state(|_, state| {
            let resource = state
                .resource_state
                .as_mut()
                .ok_or(NativeResourceError::MissingInstallation)?;
            let attempt = resource.begin_entry(
                &mut state.values,
                &state.resources,
                resource.entry,
                ResourcePurpose::Runtime,
            )?;
            unsafe { ptr::write(out_attempt, attempt.raw()) };
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_test_entry_complete(
    context: *const JettRuntimeContextV1,
    attempt: u64,
    body_status: u32,
    body_panicked: u32,
    out_completion: *mut NativeResourceCompletion,
    out_result: *mut JettRuntimeResultV1,
) -> JettRuntimeStatusV1 {
    context_call(context, out_completion, out_result, |authenticated| {
        if body_panicked > 1 {
            return Err(NativeResourceError::WrongOperation);
        }
        let attempt = ResourceHandleId::new(attempt)?;
        authenticated.with_state(|_, state| {
            let resource = state
                .resource_state
                .as_mut()
                .ok_or(NativeResourceError::MissingInstallation)?;
            let completion = resource.complete_entry(
                &mut state.values,
                &mut state.resources,
                attempt,
                body_status,
                body_panicked != 0,
            )?;
            unsafe { ptr::write(out_completion, completion) };
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_test_counts(
    context: *const JettRuntimeContextV1,
    out_counts: *mut NativeResourceCounts,
    out_result: *mut JettRuntimeResultV1,
) -> JettRuntimeStatusV1 {
    context_call(context, out_counts, out_result, |authenticated| {
        authenticated.with_state(|_, state| {
            let counts = match &state.resource_state {
                Some(resource) => resource.counts(&state.values, &state.resources),
                None => NativeResourceCounts {
                    registry: state.resources.live_count() as u64,
                    ordinary_empty: u32::from(state.values.is_empty()),
                    ..NativeResourceCounts::default()
                },
            };
            unsafe { ptr::write(out_counts, counts) };
            Ok(())
        })
    })
}

fn copy_regions<T>(
    context: *const JettRuntimeContextV1,
    buffer: *mut T,
    capacity: u64,
    maximum: usize,
    out_count: *mut u64,
    out_result: *mut JettRuntimeResultV1,
) -> ResourceResult<usize> {
    let capacity = checked_length(capacity, maximum)?;
    let regions = [
        region(context, 1)?,
        region(out_count, 1)?,
        region(out_result, 1)?,
    ];
    disjoint(&regions)?;
    if capacity != 0 {
        let buffer = region(buffer, capacity)?;
        for other in regions {
            disjoint(&[buffer, other])?;
        }
    }
    Ok(capacity)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_test_events_copy(
    context: *const JettRuntimeContextV1,
    buffer: *mut NativeTestEvent,
    capacity: u64,
    out_count: *mut u64,
    out_result: *mut JettRuntimeResultV1,
) -> JettRuntimeStatusV1 {
    let capacity = match copy_regions(context, buffer, capacity, MAX_EVENTS, out_count, out_result)
    {
        Ok(capacity) => capacity,
        Err(_) => return JettRuntimeStatusV1::INVALID_ARGUMENT,
    };
    result_call(out_result, || {
        let authenticated = AuthenticatedResourceContext::acquire(context)?;
        authenticated.with_state(|_, state| {
            let copy = |events: &[NativeTestEvent]| {
                unsafe { ptr::write(out_count, events.len() as u64) };
                if capacity == 0 {
                    return Ok(());
                }
                if capacity < events.len() {
                    return Err(NativeResourceError::Capacity);
                }
                unsafe { ptr::copy_nonoverlapping(events.as_ptr(), buffer, events.len()) };
                Ok(())
            };
            match &state.resource_state {
                Some(resource) => resource.observe_events(copy),
                None => copy(&[]),
            }
        })
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_test_failure_copy(
    context: *const JettRuntimeContextV1,
    attempt: u64,
    buffer: *mut u8,
    capacity: u64,
    out_length: *mut u64,
    out_result: *mut JettRuntimeResultV1,
) -> JettRuntimeStatusV1 {
    let capacity = match copy_regions(context, buffer, capacity, 65_536, out_length, out_result) {
        Ok(capacity) => capacity,
        Err(_) => return JettRuntimeStatusV1::INVALID_ARGUMENT,
    };
    result_call(out_result, || {
        let authenticated = AuthenticatedResourceContext::acquire(context)?;
        let attempt = ResourceHandleId::new(attempt)?;
        authenticated.with_state(|_, state| {
            let resource = state
                .resource_state
                .as_ref()
                .ok_or(NativeResourceError::MissingInstallation)?;
            let (resource, ordinary) = resource.completed_messages(attempt)?;
            let message = if resource.is_empty() {
                ordinary
            } else {
                resource
            };
            unsafe { ptr::write(out_length, message.len() as u64) };
            if capacity == 0 {
                return Ok(());
            }
            if capacity < message.len() {
                return Err(NativeResourceError::Capacity);
            }
            unsafe { ptr::copy_nonoverlapping(message.as_ptr(), buffer, message.len()) };
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_test_script_remaining(
    context: *const JettRuntimeContextV1,
    out_remaining: *mut u64,
    out_result: *mut JettRuntimeResultV1,
) -> JettRuntimeStatusV1 {
    context_call(context, out_remaining, out_result, |authenticated| {
        authenticated.with_state(|_, state| {
            let resource = state
                .resource_state
                .as_ref()
                .ok_or(NativeResourceError::MissingInstallation)?;
            unsafe { ptr::write(out_remaining, resource.script_remaining() as u64) };
            Ok(())
        })
    })
}
