//! ABI v1 opaque handles; each operation is joined to an installed v3 carrier row.
use super::*;

/// # Safety
/// Context and output obey the stationary-context and exclusive-output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_carrier_construct_begin(
    context: *const JettRuntimeContextV1,
    frame: u64,
    operation: u32,
    out: *mut u64,
) -> JettRuntimeStatusV1 {
    unsafe {
        output(context, out, |s, v, _| {
            s.carrier_construct_begin(v, id(frame)?, operation)
                .map(ResourceHandleId::raw)
        })
    }
}
/// # Safety
/// Context obeys the stationary-context contract; bits conveys the row's owned child.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_carrier_child(
    context: *const JettRuntimeContextV1,
    builder: u64,
    ordinal: u32,
    bits: u64,
) -> JettRuntimeStatusV1 {
    status(context, |s, v, _| {
        s.carrier_child(v, id(builder)?, ordinal, bits)
    })
}
/// # Safety
/// Context and output obey the stationary-context and exclusive-output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_carrier_commit(
    context: *const JettRuntimeContextV1,
    builder: u64,
    out: *mut u64,
) -> JettRuntimeStatusV1 {
    unsafe {
        output(context, out, |s, v, _| {
            s.carrier_commit(v, id(builder)?).map(ResourceHandleId::raw)
        })
    }
}
/// # Safety
/// Context and output obey the stationary-context and exclusive-output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_carrier_transfer(
    context: *const JettRuntimeContextV1,
    frame: u64,
    operation: u32,
    input: u64,
    out: *mut u64,
) -> JettRuntimeStatusV1 {
    unsafe {
        output(context, out, |s, v, _| {
            s.carrier_transfer(v, id(frame)?, operation, id(input)?)
                .map(ResourceHandleId::raw)
        })
    }
}
/// # Safety
/// Context and output obey the stationary-context and exclusive-output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_carrier_qualify(
    context: *const JettRuntimeContextV1,
    frame: u64,
    operation: u32,
    input: u64,
    out: *mut u64,
) -> JettRuntimeStatusV1 {
    unsafe {
        output(context, out, |s, v, _| {
            s.carrier_qualify(v, id(frame)?, operation, id(input)?)
                .map(ResourceHandleId::raw)
        })
    }
}
/// # Safety
/// Context and output obey the stationary-context and exclusive-output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_carrier_borrow(
    context: *const JettRuntimeContextV1,
    frame: u64,
    operation: u32,
    input: u64,
    selection: u64,
    out: *mut u64,
) -> JettRuntimeStatusV1 {
    unsafe {
        output(context, out, |s, v, _| {
            s.carrier_borrow(v, id(frame)?, operation, id(input)?, selection)
                .map(ResourceHandleId::raw)
        })
    }
}
/// # Safety
/// Context and output obey the stationary-context and exclusive-output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_carrier_observe(
    context: *const JettRuntimeContextV1,
    frame: u64,
    operation: u32,
    input: u64,
    selection: u64,
    out: *mut u64,
) -> JettRuntimeStatusV1 {
    unsafe {
        output(context, out, |s, v, _| {
            s.carrier_observe(v, id(frame)?, operation, id(input)?, selection)
        })
    }
}
/// # Safety
/// Context and output obey the stationary-context and exclusive-output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_carrier_extract(
    context: *const JettRuntimeContextV1,
    frame: u64,
    operation: u32,
    input: u64,
    selection: u64,
    out: *mut u64,
) -> JettRuntimeStatusV1 {
    unsafe {
        output(context, out, |s, v, _| {
            s.carrier_extract(v, id(frame)?, operation, id(input)?, selection)
        })
    }
}
/// # Safety
/// Context and output obey the stationary-context and exclusive-output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_carrier_adapt(
    context: *const JettRuntimeContextV1,
    frame: u64,
    operation: u32,
    input: u64,
    selection: u64,
    out: *mut u64,
) -> JettRuntimeStatusV1 {
    unsafe {
        output(context, out, |s, v, _| {
            s.carrier_adapt(v, id(frame)?, operation, id(input)?, selection)
                .map(ResourceHandleId::raw)
        })
    }
}
/// # Safety
/// Context obeys the stationary-context contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_carrier_end(
    context: *const JettRuntimeContextV1,
    frame: u64,
    operation: u32,
    input: u64,
) -> JettRuntimeStatusV1 {
    status(context, |s, _, _| {
        s.carrier_end(id(frame)?, operation, id(input)?)
    })
}
/// # Safety
/// Context obeys the stationary-context contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_carrier_retire(
    context: *const JettRuntimeContextV1,
    frame: u64,
    operation: u32,
    input: u64,
) -> JettRuntimeStatusV1 {
    status(context, |s, v, _| {
        s.carrier_retire(v, id(frame)?, operation, id(input)?)
    })
}
/// # Safety
/// Context obeys the stationary-context contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_carrier_reset_iteration(
    context: *const JettRuntimeContextV1,
    frame: u64,
    operation: u32,
    input: u64,
) -> JettRuntimeStatusV1 {
    status(context, |s, v, _| {
        s.carrier_reset_iteration(v, id(frame)?, operation, id(input)?)
    })
}
/// # Safety
/// Context and output obey the stationary-context and exclusive-output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_resource_carrier_publish(
    context: *const JettRuntimeContextV1,
    scope: u64,
    operation: u32,
    input: u64,
    out: *mut u64,
) -> JettRuntimeStatusV1 {
    unsafe {
        output(context, out, |s, v, r| {
            s.carrier_publish(v, r, id(scope)?, operation, id(input)?)
                .map(ResourceHandleId::raw)
        })
    }
}
