//! Exact compiler-internal Resource leaves. No provider installation or key adoption.
use super::*;

/// Addresses cannot establish lifetime/exclusivity; those remain the unsafe C
/// caller contract. Reject null/misaligned/overflow/known context overlaps before effect.
pub(super) fn output_ok<T>(context: *const JettRuntimeContextV1, out: *mut T) -> bool {
    let start = out.addr();
    let Some(end) = start.checked_add(std::mem::size_of::<T>()) else {
        return false;
    };
    let ctx = context.addr();
    let Some(ctx_end) = ctx.checked_add(std::mem::size_of::<JettRuntimeContextV1>()) else {
        return false;
    };
    !out.is_null() && start % std::mem::align_of::<T>() == 0 && (end <= ctx || ctx_end <= start)
}
pub(super) fn disjoint<A, B>(a: *mut A, b: *mut B) -> bool {
    let Some(a_end) = a.addr().checked_add(std::mem::size_of::<A>()) else {
        return false;
    };
    let Some(b_end) = b.addr().checked_add(std::mem::size_of::<B>()) else {
        return false;
    };
    a_end <= b.addr() || b_end <= a.addr()
}
fn status(
    context: *const JettRuntimeContextV1,
    op: impl FnOnce(
        &mut NativeResourceState,
        &mut values::NativeValues,
        &mut ResourceRegistry,
    ) -> ResourceResult<()>,
) -> JettRuntimeStatusV1 {
    match catch_unwind(AssertUnwindSafe(|| {
        AuthenticatedResourceContext::acquire(context)?.with_resource(op)
    })) {
        Ok(Ok(())) => JettRuntimeStatusV1::OK,
        Ok(Err(error)) => error.status(),
        Err(panic) => {
            discard_panic_payload(panic);
            JettRuntimeStatusV1::PANIC
        }
    }
}
unsafe fn output<T>(
    context: *const JettRuntimeContextV1,
    out: *mut T,
    op: impl FnOnce(
        &mut NativeResourceState,
        &mut values::NativeValues,
        &mut ResourceRegistry,
    ) -> ResourceResult<T>,
) -> JettRuntimeStatusV1 {
    if !output_ok(context, out) {
        return JettRuntimeStatusV1::INVALID_ARGUMENT;
    }
    status(context, |state, ordinary, registry| {
        let value = op(state, ordinary, registry)?;
        // SAFETY: the C caller guarantees live exclusive storage; shape/overlap was checked above.
        unsafe {
            ptr::write(out, value);
        }
        Ok(())
    })
}
fn id(raw: u64) -> ResourceResult<ResourceHandleId> {
    ResourceHandleId::new(raw)
}

/// # Safety
/// Context and outputs satisfy the stationary-context and exclusive output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_scope_validate(
    context: *const JettRuntimeContextV1,
    scope: u64,
    function: u32,
    signature: u32,
    template: u32,
) -> JettRuntimeStatusV1 {
    status(context, |s, v, r| {
        s.scope_validate(v, id(scope)?, function, signature, template)
    })
}

/// # Safety
/// Context and outputs satisfy the stationary-context and exclusive output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_operation_begin(
    context: *const JettRuntimeContextV1,
    parent: u64,
    template: u32,
    out_value: *mut u64,
) -> JettRuntimeStatusV1 {
    unsafe {
        output(context, out_value, |s, v, r| {
            s.begin_operation_frame(v, template, id(parent)?)
                .map(ResourceHandleId::raw)
        })
    }
}

/// # Safety
/// Context and outputs satisfy the stationary-context and exclusive output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_operation_complete(
    context: *const JettRuntimeContextV1,
    frame: u64,
    operation: u32,
) -> JettRuntimeStatusV1 {
    status(context, |s, v, r| {
        s.operation_frame(operation, id(frame)?)?;
        if !matches!(s.operation(operation)?, NativeOperation::Complete { .. }) {
            return Err(NativeResourceError::WrongOperation);
        }
        s.end_operation_frame(v, r, id(frame)?)
    })
}

/// # Safety
/// Context and outputs satisfy the stationary-context and exclusive output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_hook_prepare(
    context: *const JettRuntimeContextV1,
    frame: u64,
    operation: u32,
    descriptor: u64,
    out_value: *mut u64,
) -> JettRuntimeStatusV1 {
    unsafe {
        output(context, out_value, |s, v, r| {
            s.prepare_hook(
                v,
                operation,
                id(frame)?,
                if descriptor == 0 {
                    None
                } else {
                    Some(id(descriptor)?)
                },
            )
            .map(ResourceHandleId::raw)
        })
    }
}

/// # Safety
/// Context and outputs satisfy the stationary-context and exclusive output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_factory_prepare(
    context: *const JettRuntimeContextV1,
    call: u64,
    network: u64,
    out_value: *mut u64,
) -> JettRuntimeStatusV1 {
    unsafe {
        output(context, out_value, |s, v, r| {
            s.prepare_factory(v, r, id(call)?, network)
                .map(ResourceHandleId::raw)
        })
    }
}

/// # Safety
/// Context and outputs satisfy the stationary-context and exclusive output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_factory_commit(
    context: *const JettRuntimeContextV1,
    prepared: u64,
    network: u64,
    label: i64,
    out_value: *mut JettResourceCallResultV1,
) -> JettRuntimeStatusV1 {
    unsafe {
        output(context, out_value, |s, v, r| {
            s.factory(v, r, id(prepared)?, network, label)
        })
    }
}

/// # Safety
/// Context and outputs satisfy the stationary-context and exclusive output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_borrow_begin(
    context: *const JettRuntimeContextV1,
    frame: u64,
    operation: u32,
    owner: u64,
    out_value: *mut u64,
) -> JettRuntimeStatusV1 {
    unsafe {
        output(context, out_value, |s, v, r| {
            s.borrow_begin(v, r, operation, id(frame)?, id(owner)?)
                .map(ResourceHandleId::raw)
        })
    }
}

/// # Safety
/// Context and outputs satisfy the stationary-context and exclusive output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_borrow_commit(
    context: *const JettRuntimeContextV1,
    call: u64,
    network: u64,
    loan: u64,
    out_value: *mut u64,
) -> JettRuntimeStatusV1 {
    unsafe {
        output(context, out_value, |s, v, r| {
            s.invoke_borrow(v, r, id(call)?, network, id(loan)?)
        })
    }
}

/// # Safety
/// Context and outputs satisfy the stationary-context and exclusive output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_borrow_end(
    context: *const JettRuntimeContextV1,
    frame: u64,
    operation: u32,
    loan: u64,
) -> JettRuntimeStatusV1 {
    status(context, |s, v, r| {
        s.borrow_end(operation, id(frame)?, id(loan)?)
    })
}

/// # Safety
/// Context and outputs satisfy the stationary-context and exclusive output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_close(
    context: *const JettRuntimeContextV1,
    frame: u64,
    operation: u32,
    owner: u64,
) -> JettRuntimeStatusV1 {
    status(context, |s, v, r| {
        s.close_or_drop(r, operation, id(frame)?, id(owner)?)
    })
}

/// # Safety
/// Context and outputs satisfy the stationary-context and exclusive output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_descriptor_close(
    context: *const JettRuntimeContextV1,
    call: u64,
    owner: u64,
) -> JettRuntimeStatusV1 {
    status(context, |s, v, r| {
        let call_id = id(call)?;
        let call = s.checked_call(call_id)?;
        let frame = call.frame;
        let operation = call.target_operation;
        if call.descriptor.is_none() {
            return Err(NativeResourceError::WrongOperation);
        }
        s.close_or_drop(r, operation, frame, id(owner)?)?;
        s.handles.remove(&call_id);
        Ok(())
    })
}

/// # Safety
/// Context and outputs satisfy the stationary-context and exclusive output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_descriptor(
    context: *const JettRuntimeContextV1,
    operation: u32,
    out_value: *mut u64,
) -> JettRuntimeStatusV1 {
    unsafe {
        output(context, out_value, |s, v, r| {
            s.descriptor(v, operation).map(ResourceHandleId::raw)
        })
    }
}

/// # Safety
/// Context and outputs satisfy the stationary-context and exclusive output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_transfer(
    context: *const JettRuntimeContextV1,
    frame: u64,
    operation: u32,
    value: u64,
    destination_frame: u64,
    out_value: *mut u64,
) -> JettRuntimeStatusV1 {
    unsafe {
        output(context, out_value, |s, v, r| {
            if s.destination_frame(id(frame)?, operation)? != id(destination_frame)? {
                return Err(NativeResourceError::WrongFrame);
            }
            s.transfer(
                v,
                r,
                operation,
                id(frame)?,
                id(value)?,
                id(destination_frame)?,
            )
            .map(ResourceHandleId::raw)
        })
    }
}

/// # Safety
/// Context and outputs satisfy the stationary-context and exclusive output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_destination_frame(
    context: *const JettRuntimeContextV1,
    frame: u64,
    operation: u32,
    out_value: *mut u64,
) -> JettRuntimeStatusV1 {
    unsafe {
        output(context, out_value, |s, v, r| {
            s.running(v)?;
            s.destination_frame(id(frame)?, operation)
                .map(ResourceHandleId::raw)
        })
    }
}

/// # Safety
/// Context and outputs satisfy the stationary-context and exclusive output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_sum_adopt(
    context: *const JettRuntimeContextV1,
    frame: u64,
    operation: u32,
    owner: u64,
    destination_frame: u64,
    out_value: *mut u64,
) -> JettRuntimeStatusV1 {
    unsafe {
        output(context, out_value, |s, v, r| {
            if s.destination_frame(id(frame)?, operation)? != id(destination_frame)? {
                return Err(NativeResourceError::WrongFrame);
            }
            s.sum_adopt(
                v,
                r,
                operation,
                id(frame)?,
                id(owner)?,
                id(destination_frame)?,
            )
            .map(ResourceHandleId::raw)
        })
    }
}

/// # Safety
/// Context and outputs satisfy the stationary-context and exclusive output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_sum_tag(
    context: *const JettRuntimeContextV1,
    frame: u64,
    value: u64,
    out_value: *mut u32,
) -> JettRuntimeStatusV1 {
    unsafe {
        output(context, out_value, |s, v, r| {
            s.running(v)?;
            s.frame(id(frame)?)?;
            if s.active_frames.last() != Some(&id(frame)?) {
                return Err(NativeResourceError::WrongFrame);
            }
            s.sum_tag(id(value)?)
        })
    }
}

/// # Safety
/// Context and outputs satisfy the stationary-context and exclusive output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_sum_take(
    context: *const JettRuntimeContextV1,
    frame: u64,
    operation: u32,
    value: u64,
    destination_frame: u64,
    out_value: *mut u64,
) -> JettRuntimeStatusV1 {
    unsafe {
        output(context, out_value, |s, v, r| {
            if s.destination_frame(id(frame)?, operation)? != id(destination_frame)? {
                return Err(NativeResourceError::WrongFrame);
            }
            s.sum_take(
                v,
                r,
                operation,
                id(frame)?,
                id(value)?,
                id(destination_frame)?,
            )
            .map(ResourceHandleId::raw)
        })
    }
}

/// # Safety
/// Context and outputs satisfy the stationary-context and exclusive output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_sum_drop(
    context: *const JettRuntimeContextV1,
    frame: u64,
    operation: u32,
    value: u64,
) -> JettRuntimeStatusV1 {
    status(context, |s, v, r| {
        s.sum_drop(v, r, operation, id(frame)?, id(value)?)
    })
}

/// # Safety
/// Context and outputs satisfy the stationary-context and exclusive output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_failure_companion_take(
    context: *const JettRuntimeContextV1,
    frame: u64,
    operation: u32,
    sum: u64,
    out_value: *mut u64,
) -> JettRuntimeStatusV1 {
    unsafe {
        output(context, out_value, |s, v, r| {
            s.failure_companion_take(v, id(frame)?, operation, id(sum)?)
        })
    }
}

/// # Safety
/// Context and outputs satisfy the stationary-context and exclusive output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_absent_sum(
    context: *const JettRuntimeContextV1,
    frame: u64,
    operation: u32,
    out_value: *mut u64,
) -> JettRuntimeStatusV1 {
    unsafe {
        output(context, out_value, |s, v, r| {
            s.absent_sum(v, id(frame)?, operation)
                .map(ResourceHandleId::raw)
        })
    }
}

/// # Safety
/// Context and outputs satisfy the stationary-context and exclusive output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_failure_sum(
    context: *const JettRuntimeContextV1,
    frame: u64,
    operation: u32,
    companion: u64,
    out_value: *mut u64,
) -> JettRuntimeStatusV1 {
    unsafe {
        output(context, out_value, |s, v, r| {
            s.failure_sum(v, id(frame)?, operation, companion)
                .map(ResourceHandleId::raw)
        })
    }
}

/// # Safety
/// Context and outputs satisfy the stationary-context and exclusive output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_replace(
    context: *const JettRuntimeContextV1,
    frame: u64,
    operation: u32,
    old: u64,
    replacement: u64,
    out_value: *mut u64,
) -> JettRuntimeStatusV1 {
    unsafe {
        output(context, out_value, |s, v, r| {
            s.replace(v, r, operation, id(frame)?, id(old)?, id(replacement)?)
                .map(ResourceHandleId::raw)
        })
    }
}

/// # Safety
/// Context and outputs satisfy the stationary-context and exclusive output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_source_prepare(
    context: *const JettRuntimeContextV1,
    frame: u64,
    operation: u32,
    out_value: *mut u64,
) -> JettRuntimeStatusV1 {
    unsafe {
        output(context, out_value, |s, v, r| {
            s.source_prepare(v, id(frame)?, operation)
                .map(ResourceHandleId::raw)
        })
    }
}

/// # Safety
/// Context and outputs satisfy the stationary-context and exclusive output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_source_actual(
    context: *const JettRuntimeContextV1,
    call: u64,
    source_index: u32,
    parameter: u32,
    value: u64,
) -> JettRuntimeStatusV1 {
    status(context, |s, v, r| {
        s.source_actual(v, r, id(call)?, source_index, parameter, value)
    })
}

/// # Safety
/// Context and outputs satisfy the stationary-context and exclusive output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_source_enter(
    context: *const JettRuntimeContextV1,
    call: u64,
    out_value: *mut u64,
) -> JettRuntimeStatusV1 {
    unsafe {
        output(context, out_value, |s, v, r| {
            s.source_enter(v, r, id(call)?).map(ResourceHandleId::raw)
        })
    }
}

/// # Safety
/// Context and outputs satisfy the stationary-context and exclusive output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_source_parameter(
    context: *const JettRuntimeContextV1,
    scope: u64,
    parameter: u32,
    out_value: *mut u64,
) -> JettRuntimeStatusV1 {
    unsafe {
        output(context, out_value, |s, v, r| {
            s.source_parameter(v, id(scope)?, parameter)
        })
    }
}

/// # Safety
/// Context and outputs satisfy the stationary-context and exclusive output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_source_status(
    context: *const JettRuntimeContextV1,
    call: u64,
) -> JettRuntimeStatusV1 {
    status(context, |s, v, r| s.source_status(v, id(call)?))
}

/// # Safety
/// Context and outputs satisfy the stationary-context and exclusive output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_scope_complete(
    context: *const JettRuntimeContextV1,
    scope: u64,
    operation: u32,
    body_status: u32,
) -> JettRuntimeStatusV1 {
    status(context, |s, v, r| {
        s.scope_complete(v, r, id(scope)?, operation, body_status)
    })
}

/// # Safety
/// Context and outputs satisfy the stationary-context and exclusive output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_return_publish(
    context: *const JettRuntimeContextV1,
    scope: u64,
    operation: u32,
    provisional: u64,
    out_value: *mut u64,
) -> JettRuntimeStatusV1 {
    unsafe {
        output(context, out_value, |s, v, r| {
            s.return_publish(v, r, id(scope)?, operation, id(provisional)?)
                .map(ResourceHandleId::raw)
        })
    }
}

/// # Safety
/// Output records are aligned, live, exclusive and mutually disjoint.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_entry_scope(
    context: *const JettRuntimeContextV1,
    out_scope: *mut u64,
    out_result: *mut JettRuntimeResultV1,
) -> JettRuntimeStatusV1 {
    unsafe {
        entry_lookup(context, out_scope, out_result, |s| {
            s.validate_installation()?;
            s.runtime_purpose()?;
            let attempt = s
                .attempt
                .as_ref()
                .ok_or(NativeResourceError::InvalidEntry)?;
            if attempt.phase != AttemptPhase::Running || s.entry != attempt.entry {
                return Err(NativeResourceError::InvalidEntry);
            }
            let root = s.frame(attempt.root_frame)?;
            if root.parent.is_some() || root.template != s.entry.scope {
                return Err(NativeResourceError::WrongFrame);
            }
            Ok(attempt.root_frame.raw())
        })
    }
}
/// # Safety
/// Output records are aligned, live, exclusive and mutually disjoint.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_entry_network(
    context: *const JettRuntimeContextV1,
    parameter: u32,
    out_capability: *mut u64,
    out_result: *mut JettRuntimeResultV1,
) -> JettRuntimeStatusV1 {
    unsafe {
        entry_lookup(context, out_capability, out_result, |s| {
            let attempt = s
                .attempt
                .as_ref()
                .ok_or(NativeResourceError::InvalidEntry)?
                .id;
            s.entry_network(attempt, parameter)
        })
    }
}
unsafe fn entry_lookup(
    context: *const JettRuntimeContextV1,
    out: *mut u64,
    out_result: *mut JettRuntimeResultV1,
    lookup: impl FnOnce(&NativeResourceState) -> ResourceResult<u64>,
) -> JettRuntimeStatusV1 {
    if !output_ok(context, out) || !output_ok(context, out_result) || !disjoint(out, out_result) {
        return JettRuntimeStatusV1::INVALID_ARGUMENT;
    }
    complete_call(out_result, || {
        let result = AuthenticatedResourceContext::acquire(context).and_then(|context| {
            context.with_resource(|s, v, _| {
                s.running(v)?;
                lookup(s)
            })
        });
        match result {
            Ok(value) => {
                unsafe { ptr::write(out, value) };
                JettRuntimeResultV1::ok()
            }
            Err(error) => JettRuntimeResultV1::failure(error.status(), REFUSED),
        }
    })
}

/// # Safety
/// Context is stationary and output storage is aligned, live and exclusive.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_entry_outcome(
    context: *const JettRuntimeContextV1,
    root_scope: u64,
    function: u32,
    signature: u32,
    template: u32,
    out_body: *mut u32,
) -> JettRuntimeStatusV1 {
    if !output_ok(context, out_body) {
        return JettRuntimeStatusV1::INVALID_ARGUMENT;
    }
    // Refusal is observation only: with_resource would latch it as a body fault.
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        AuthenticatedResourceContext::acquire(context)?.with_state(|_, state| {
            state
                .resource_state
                .as_ref()
                .ok_or(NativeResourceError::MissingInstallation)?
                .entry_outcome(id(root_scope)?, function, signature, template)
        })
    }));
    match outcome {
        Ok(Ok(body)) => {
            // SAFETY: the caller supplies live exclusive storage; preflight
            // refused known overlap/range/alignment faults before any state read.
            unsafe { ptr::write(out_body, body) };
            JettRuntimeStatusV1::OK
        }
        Ok(Err(error)) => error.status(),
        Err(panic) => {
            discard_panic_payload(panic);
            JettRuntimeStatusV1::PANIC
        }
    }
}

/// # Safety
/// Context and output satisfy the stationary-context and exclusive output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_sum_borrow(
    context: *const JettRuntimeContextV1,
    frame: u64,
    operation: u32,
    sum: u64,
    out_loan: *mut u64,
) -> JettRuntimeStatusV1 {
    unsafe {
        output(context, out_loan, |s, v, r| {
            s.sum_borrow(v, r, operation, id(frame)?, id(sum)?)
                .map(ResourceHandleId::raw)
        })
    }
}
/// # Safety
/// Context and output satisfy the stationary-context and exclusive output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_sum_view_tag(
    context: *const JettRuntimeContextV1,
    frame: u64,
    operation: u32,
    loan: u64,
    out_tag: *mut u32,
) -> JettRuntimeStatusV1 {
    unsafe {
        output(context, out_tag, |s, v, r| {
            s.sum_view_tag(v, r, operation, id(frame)?, id(loan)?)
        })
    }
}
/// # Safety
/// Context and output satisfy the stationary-context and exclusive output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_sum_view_project(
    context: *const JettRuntimeContextV1,
    frame: u64,
    operation: u32,
    loan: u64,
    out_child: *mut u64,
) -> JettRuntimeStatusV1 {
    unsafe {
        output(context, out_child, |s, v, r| {
            s.sum_view_project(v, r, operation, id(frame)?, id(loan)?)
                .map(ResourceHandleId::raw)
        })
    }
}
/// # Safety
/// Context and output satisfy the stationary-context and exclusive output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_sum_failure_read(
    context: *const JettRuntimeContextV1,
    frame: u64,
    operation: u32,
    loan: u64,
    out_companion: *mut u64,
) -> JettRuntimeStatusV1 {
    unsafe {
        output(context, out_companion, |s, v, r| {
            s.sum_failure_read(v, r, operation, id(frame)?, id(loan)?)
        })
    }
}
/// # Safety
/// Context satisfies the stationary-context contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_sum_borrow_end(
    context: *const JettRuntimeContextV1,
    frame: u64,
    operation: u32,
    loan: u64,
) -> JettRuntimeStatusV1 {
    status(context, |s, _v, r| {
        s.sum_borrow_end(r, operation, id(frame)?, id(loan)?)
    })
}
