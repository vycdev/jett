//! Generated-object startup. No Source provider or runtime-purpose selector.
use super::*;
use std::mem::{align_of, size_of};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EntryOwner {
    External,
    Generated,
}

#[repr(C)]
struct ObjectManifest {
    layout: *const u8,
    layout_length: u64,
    function: u32,
    signature: u32,
    scope: u32,
    reserved: u32,
}
const _: () = assert!(size_of::<ObjectManifest>() == 32 && align_of::<ObjectManifest>() == 8);

#[derive(Clone, Copy)]
struct Region {
    start: usize,
    end: usize,
}
fn region<T>(pointer: *const T, count: usize) -> ResourceResult<Region> {
    let length = size_of::<T>()
        .checked_mul(count)
        .filter(|length| *length <= isize::MAX as usize)
        .ok_or(NativeResourceError::Capacity)?;
    let start = pointer.addr();
    if pointer.is_null() || start % align_of::<T>() != 0 {
        return Err(NativeResourceError::WrongOperation);
    }
    let end = start
        .checked_add(length)
        .ok_or(NativeResourceError::Capacity)?;
    Ok(Region { start, end })
}
fn separate(regions: &[Region]) -> ResourceResult<()> {
    for (index, left) in regions.iter().enumerate() {
        for right in &regions[index + 1..] {
            if left.start < right.end && right.start < left.end {
                return Err(NativeResourceError::WrongOperation);
            }
        }
    }
    Ok(())
}

fn begin(
    authenticated: &AuthenticatedResourceContext,
    wire: &[u8],
    entry: NativeEntry,
) -> ResourceResult<u64> {
    authenticated.with_state(|key, state| {
        if state.resource_state.is_none() {
            install_candidate(key, state, wire, entry, InstalledProvider::Disabled)?;
            state
                .resource_state
                .as_mut()
                .ok_or(NativeResourceError::MissingInstallation)?
                .entry_owner = EntryOwner::Generated;
        }
        let resource = state
            .resource_state
            .as_mut()
            .ok_or(NativeResourceError::MissingInstallation)?;
        resource.validate_installation()?;
        if resource.entry != entry || resource.installed_wire != wire {
            return Err(NativeResourceError::InvalidEntry);
        }
        if resource.entry_owner == EntryOwner::External {
            // The matched test main already owns begin/complete. This is never
            // an alternate production installation or attempt-adoption path.
            #[cfg(all(test, jett_resource_native_test_archive))]
            if matches!(resource.provider, InstalledProvider::Scripted(_))
                && resource.attempt.as_ref().is_some_and(|attempt| {
                    attempt.phase == AttemptPhase::Running
                        && attempt.entry == entry
                        && attempt.purpose == ResourcePurpose::Runtime
                })
            {
                return Ok(0);
            }
            return Err(NativeResourceError::ActiveAttempt);
        }
        let attempt = resource.begin_entry(
            &mut state.values,
            &state.resources,
            entry,
            ResourcePurpose::Runtime,
        )?;
        Ok(attempt.raw())
    })
}

/// # Safety
/// The stationary context is live. Manifest/layout remain readable for this
/// call; output records are distinct, aligned, exclusive writable storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_program_begin(
    context: *const JettRuntimeContextV1,
    manifest: *const ObjectManifest,
    out_attempt: *mut u64,
    out_result: *mut JettRuntimeResultV1,
) -> JettRuntimeStatusV1 {
    let regions = (|| {
        let records = [
            region(context, 1)?,
            region(manifest, 1)?,
            region(out_attempt, 1)?,
            region(out_result, 1)?,
        ];
        separate(&records)?;
        // SAFETY: caller supplies readable storage; known overlaps were refused.
        let manifest = unsafe { &*manifest };
        let length =
            usize::try_from(manifest.layout_length).map_err(|_| NativeResourceError::Capacity)?;
        let layout = region(manifest.layout, length)?;
        for record in records {
            separate(&[layout, record])?;
        }
        if manifest.reserved != 0 || length == 0 {
            return Err(NativeResourceError::InvalidEntry);
        }
        Ok(length)
    })();
    let Ok(length) = regions else {
        return JettRuntimeStatusV1::INVALID_ARGUMENT;
    };
    complete_call(out_result, || {
        // SAFETY: caller keeps the validated readable regions stable for the call.
        let manifest = unsafe { &*manifest };
        let wire = unsafe { std::slice::from_raw_parts(manifest.layout, length) };
        let entry = NativeEntry {
            function: manifest.function,
            signature: manifest.signature,
            scope: manifest.scope,
        };
        match AuthenticatedResourceContext::acquire(context)
            .and_then(|authenticated| begin(&authenticated, wire, entry))
        {
            Ok(attempt) => {
                // SAFETY: validated exclusive output; no effect follows the write.
                unsafe { ptr::write(out_attempt, attempt) };
                JettRuntimeResultV1::ok()
            }
            Err(error) => JettRuntimeResultV1::failure(error.status(), REFUSED),
        }
    })
}

fn complete(
    authenticated: &AuthenticatedResourceContext,
    attempt: ResourceHandleId,
    body_status: u32,
    panicked: bool,
) -> ResourceResult<JettRuntimeStatusV1> {
    authenticated.with_state(|_, state| {
        let resource = state
            .resource_state
            .as_mut()
            .ok_or(NativeResourceError::MissingInstallation)?;
        resource.validate_installation()?;
        if resource.entry_owner != EntryOwner::Generated {
            return Err(NativeResourceError::WrongOperation);
        }
        resource.runtime_purpose()?;
        resource.complete_entry(
            &mut state.values,
            &mut state.resources,
            attempt,
            body_status,
            panicked,
        )?;
        let completed = resource
            .completed
            .last()
            .ok_or(NativeResourceError::InvalidEntry)?;
        if completed.completion.attempt != attempt.raw() {
            return Err(NativeResourceError::InvalidEntry);
        }
        Ok(completed.terminal_status)
    })
}

fn status_call(
    operation: impl FnOnce() -> ResourceResult<JettRuntimeStatusV1>,
) -> JettRuntimeStatusV1 {
    match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(status)) => status,
        Ok(Err(error)) => error.status(),
        Err(panic) => {
            discard_panic_payload(panic);
            JettRuntimeStatusV1::PANIC
        }
    }
}

/// Retire precisely the generated attempt, preserving original body status.
/// # Safety
/// `context` is the live stationary context from the successful begin call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_program_complete(
    context: *const JettRuntimeContextV1,
    attempt: u64,
    body_status: u32,
) -> JettRuntimeStatusV1 {
    status_call(|| {
        complete(
            &AuthenticatedResourceContext::acquire(context)?,
            ResourceHandleId::new(attempt)?,
            body_status,
            false,
        )
    })
}

/// Launcher fallback after a recoverable entry panic. Ordinary contexts and
/// externally managed test attempts are no-ops. Completing custody is never retried.
/// # Safety
/// `context` is live and stationary; call only after entry control has returned.
/// `out_selected_kind` is distinct aligned exclusive writable storage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_program_unwind(
    context: *const JettRuntimeContextV1,
    body_status: u32,
    panicked: u32,
    out_selected_kind: *mut u32,
) -> JettRuntimeStatusV1 {
    if !leaves::output_ok(context, out_selected_kind) {
        return JettRuntimeStatusV1::INVALID_ARGUMENT;
    }
    status_call(|| {
        if panicked > 1 {
            return Err(NativeResourceError::WrongOperation);
        }
        let authenticated = AuthenticatedResourceContext::acquire(context)?;
        let attempt = authenticated.with_state(|_, state| {
            let Some(resource) = state.resource_state.as_ref() else {
                return Ok(None);
            };
            if resource.entry_owner != EntryOwner::Generated {
                return Ok(None);
            }
            match resource.attempt.as_ref() {
                Some(attempt) if attempt.phase == AttemptPhase::Running => Ok(Some(attempt.id)),
                Some(_) => Err(NativeResourceError::ActiveAttempt),
                None => Ok(None),
            }
        })?;
        let status = match attempt {
            Some(attempt) => complete(&authenticated, attempt, body_status, panicked != 0)?,
            None => JettRuntimeStatusV1::OK,
        };
        let kind = if attempt.is_some() {
            observation(context, |completed| {
                Ok(completed.map(|c| c.completion.selected_kind).unwrap_or(0))
            })?
        } else {
            0
        };
        // SAFETY: the caller supplies checked exclusive output storage.
        unsafe { ptr::write(out_selected_kind, kind) };
        Ok(status)
    })
}

fn observation<T>(
    context: *const JettRuntimeContextV1,
    read: impl FnOnce(Option<&NativeCompletedAttempt>) -> ResourceResult<T>,
) -> ResourceResult<T> {
    AuthenticatedResourceContext::acquire(context)?.with_state(|_, state| {
        let completed = state
            .resource_state
            .as_ref()
            .filter(|resource| {
                resource.entry_owner == EntryOwner::Generated && resource.attempt.is_none()
            })
            .and_then(|resource| resource.completed.last());
        if let Some(error) = completed.and_then(|completed| completed.report_error) {
            return Err(error);
        }
        read(completed)
    })
}

/// Read the retained terminal outcome without changing any first-error channel.
/// # Safety
/// Live stationary context and distinct aligned writable result record.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_program_failure(
    context: *const JettRuntimeContextV1,
    out_result: *mut JettRuntimeResultV1,
) -> JettRuntimeStatusV1 {
    if !leaves::output_ok(context, out_result) {
        return JettRuntimeStatusV1::INVALID_ARGUMENT;
    }
    complete_call(out_result, || {
        match observation(context, |completed| {
            Ok(completed
                .map(|c| c.terminal_status)
                .unwrap_or(JettRuntimeStatusV1::OK))
        }) {
            Ok(status) if status == JettRuntimeStatusV1::OK => JettRuntimeResultV1::ok(),
            Ok(status) => JettRuntimeResultV1::failure(status, REFUSED),
            Err(error) => JettRuntimeResultV1::failure(error.status(), REFUSED),
        }
    })
}

fn message(completed: &NativeCompletedAttempt) -> &[u8] {
    let message =
        if completed.completion.selected_kind >= 2 || completed.ordinary_message.is_empty() {
            &completed.resource_message
        } else {
            &completed.ordinary_message
        };
    if message.is_empty() && completed.terminal_status != JettRuntimeStatusV1::OK {
        b"native Resource entry failed"
    } else {
        message
    }
}

/// Copy retained diagnostics while their context is alive. Null/zero queries length.
/// # Safety
/// Live stationary context; output records and any buffer are writable, distinct,
/// and exclusive for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_program_failure_copy(
    context: *const JettRuntimeContextV1,
    buffer: *mut u8,
    capacity: u64,
    out_length: *mut u64,
    out_result: *mut JettRuntimeResultV1,
) -> JettRuntimeStatusV1 {
    let checked = (|| {
        let records = [
            region(context, 1)?,
            region(out_length, 1)?,
            region(out_result, 1)?,
        ];
        separate(&records)?;
        let capacity = usize::try_from(capacity).map_err(|_| NativeResourceError::Capacity)?;
        if !buffer.is_null() {
            let buffer = region(buffer, capacity)?;
            for record in records {
                separate(&[buffer, record])?;
            }
        } else if capacity != 0 {
            return Err(NativeResourceError::WrongOperation);
        }
        Ok(capacity)
    })();
    let Ok(capacity) = checked else {
        return JettRuntimeStatusV1::INVALID_ARGUMENT;
    };
    complete_call(out_result, || {
        match observation(context, |completed| {
            let message = completed.map(message).unwrap_or_default();
            if !buffer.is_null() && capacity < message.len() {
                return Err(NativeResourceError::Capacity);
            }
            // SAFETY: validated exclusive regions; borrowed bytes remain pinned by the lease.
            unsafe {
                ptr::write(out_length, message.len() as u64);
                if !buffer.is_null() {
                    ptr::copy_nonoverlapping(message.as_ptr(), buffer, message.len());
                }
            }
            Ok(())
        }) {
            Ok(()) => JettRuntimeResultV1::ok(),
            Err(error) => JettRuntimeResultV1::failure(error.status(), REFUSED),
        }
    })
}

#[cfg(test)]
mod tests;
