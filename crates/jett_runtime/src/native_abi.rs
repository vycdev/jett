//! Version 1 of the native compiler/runtime ABI.
//!
//! This module is the narrow C ABI used by ahead-of-time generated code. Rust
//! ABI values never cross this boundary: every shared value has a fixed-width
//! scalar or `repr(C)` layout, runtime state is reached through an opaque
//! token. Lifecycle operations write an explicit [`JettRuntimeResultV1`] out
//! parameter; the typed leaves in [`values`] use the context-local terminal
//! failure channel, checked by generated code after every fallible call.
//!
//! # Ownership and lifetimes
//!
//! - [`jett_rt_v1_context_create`] initializes one caller-owned context record.
//!   The record must remain at that address and immutable until its successful
//!   destruction and all calls using it have returned.
//! - The record contains only scalar identity. Runtime-owned state stays in a
//!   synchronized registry, so copied, moved, forged, stale, and repeatedly
//!   destroyed records fail without interpreting caller bytes as a pointer.
//! - Destroying a context atomically retires its registry entry, prevents new
//!   calls from acquiring leases, waits for existing leases, and then releases
//!   internal state exactly once. The record bytes remain unchanged and may be
//!   reused after destruction returns.
//! - [`JettRuntimeStringSliceV1`] is always borrowed. Its bytes are never
//!   retained by a runtime call. Message slices written to
//!   [`JettRuntimeResultV1`] refer to immutable process-lifetime storage and
//!   must not be freed or mutated by the caller.
//! - All non-null pointer arguments must satisfy the validity requirements
//!   documented on their functions. The runtime can reject invalid record
//!   contents, but it cannot validate arbitrary, dangling, or unreadable
//!   addresses. A zero-length input string may use a null data pointer.
//! - Callable symbols return only a transparent 32-bit status. Records are
//!   passed by pointer so generated calls do not depend on target-specific
//!   aggregate argument or return conventions.
//!
//! Exported calls catch recoverable Rust panics implemented by unwinding and
//! map them to [`JettRuntimeStatusV1::PANIC`]. Aborting panic hooks, allocator
//! failure, hardware faults, and other process-aborting conditions cannot be
//! recovered here. No Rust unwind is allowed to cross this ABI.

#[cfg(not(target_pointer_width = "64"))]
compile_error!("native runtime ABI v1 requires a 64-bit target");

#[cfg(not(panic = "unwind"))]
compile_error!("native runtime ABI v1 requires panic=unwind");

use std::collections::HashMap;
use std::io::{self, Write};
use std::mem::{align_of, offset_of, size_of};
use std::num::NonZeroU64;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::slice;
use std::str;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};

use crate::{ResourceRegistry, discard_panic_payload};

pub mod values;

/// The native runtime ABI version implemented by this module.
pub const JETT_RUNTIME_ABI_VERSION_V1: u32 = 1;

const LIVE_CONTEXT_TAG: u32 = 0x4a52_5431;

const ABI_MISMATCH_MESSAGE: &[u8] = b"runtime ABI version mismatch";
const CONTEXT_OUTPUT_NULL_MESSAGE: &[u8] = b"context output pointer is null";
const CONTEXT_NULL_MESSAGE: &[u8] = b"runtime context pointer is null";
const CONTEXT_INVALID_MESSAGE: &[u8] = b"runtime context is not live";
const CONTEXT_EXHAUSTED_MESSAGE: &[u8] = b"runtime context capacity exhausted";
const STRING_SLICE_NULL_MESSAGE: &[u8] = b"string slice pointer is null";
const STRING_NULL_MESSAGE: &[u8] = b"string data pointer is null";
const STRING_LENGTH_MESSAGE: &[u8] = b"string byte length exceeds host address space";
const STRING_UTF8_MESSAGE: &[u8] = b"string data is not valid UTF-8";
const STDOUT_WRITE_MESSAGE: &[u8] = b"stdout write failed";
const STDERR_WRITE_MESSAGE: &[u8] = b"stderr write failed";
const PANIC_MESSAGE: &[u8] = b"runtime operation panicked";

/// A stable, fixed-width status code returned by native runtime operations.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JettRuntimeStatusV1(u32);

impl JettRuntimeStatusV1 {
    pub const OK: Self = Self(0);
    pub const INVALID_ARGUMENT: Self = Self(1);
    pub const ABI_VERSION_MISMATCH: Self = Self(2);
    pub const INVALID_CONTEXT: Self = Self(3);
    pub const INVALID_UTF8: Self = Self(4);
    pub const IO_FAILURE: Self = Self(5);
    pub const LENGTH_OUT_OF_RANGE: Self = Self(6);
    pub const RESOURCE_EXHAUSTED: Self = Self(7);
    pub const PANIC: Self = Self(255);

    pub const fn code(self) -> u32 {
        self.0
    }
}

/// A borrowed UTF-8 string represented as a pointer and byte length.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JettRuntimeStringSliceV1 {
    pub data: *const u8,
    pub byte_length: u64,
}

impl JettRuntimeStringSliceV1 {
    pub const fn empty() -> Self {
        Self {
            data: ptr::null(),
            byte_length: 0,
        }
    }

    fn from_static(value: &'static [u8]) -> Self {
        let byte_length = u64::try_from(value.len()).unwrap_or(u64::MAX);
        Self {
            data: value.as_ptr(),
            byte_length,
        }
    }
}

/// The uniform result record written by every native runtime operation.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JettRuntimeResultV1 {
    pub status: JettRuntimeStatusV1,
    pub message: JettRuntimeStringSliceV1,
}

impl JettRuntimeResultV1 {
    fn ok() -> Self {
        Self {
            status: JettRuntimeStatusV1::OK,
            message: JettRuntimeStringSliceV1::empty(),
        }
    }

    fn failure(status: JettRuntimeStatusV1, message: &'static [u8]) -> Self {
        Self {
            status,
            message: JettRuntimeStringSliceV1::from_static(message),
        }
    }
}

/// A caller-owned scalar identity record for per-program runtime state.
#[repr(C)]
#[derive(Debug)]
pub struct JettRuntimeContextV1 {
    token: u64,
    abi_version: u32,
    state_tag: u32,
}

impl JettRuntimeContextV1 {
    const fn retired() -> Self {
        Self {
            token: 0,
            abi_version: 0,
            state_tag: 0,
        }
    }

    fn has_live_shape(&self) -> bool {
        self.token != 0
            && self.abi_version == JETT_RUNTIME_ABI_VERSION_V1
            && self.state_tag == LIVE_CONTEXT_TAG
    }
}

const _: () = {
    assert!(size_of::<*const ()>() == 8);
    assert!(size_of::<JettRuntimeStatusV1>() == 4);
    assert!(align_of::<JettRuntimeStatusV1>() == 4);
    assert!(size_of::<JettRuntimeStringSliceV1>() == 16);
    assert!(align_of::<JettRuntimeStringSliceV1>() == 8);
    assert!(offset_of!(JettRuntimeStringSliceV1, data) == 0);
    assert!(offset_of!(JettRuntimeStringSliceV1, byte_length) == 8);
    assert!(size_of::<JettRuntimeResultV1>() == 24);
    assert!(align_of::<JettRuntimeResultV1>() == 8);
    assert!(offset_of!(JettRuntimeResultV1, status) == 0);
    assert!(offset_of!(JettRuntimeResultV1, message) == 8);
    assert!(size_of::<JettRuntimeContextV1>() == 16);
    assert!(align_of::<JettRuntimeContextV1>() == 8);
    assert!(offset_of!(JettRuntimeContextV1, token) == 0);
    assert!(offset_of!(JettRuntimeContextV1, abi_version) == 8);
    assert!(offset_of!(JettRuntimeContextV1, state_tag) == 12);
};

struct NativeContextState {
    values: values::NativeValues,
    _resources: ResourceRegistry,
}

impl NativeContextState {
    fn new() -> Self {
        Self {
            _resources: ResourceRegistry::new(),
            values: values::NativeValues::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ContextRegistryKey {
    owner_address: usize,
    token: u64,
    abi_version: u32,
}

#[derive(Default)]
struct NativeContextLifecycle {
    active_leases: usize,
}

struct NativeContextEntry {
    lifecycle: Mutex<NativeContextLifecycle>,
    leases_drained: Condvar,
    state: Mutex<Option<NativeContextState>>,
}

impl NativeContextEntry {
    fn new() -> Self {
        Self {
            lifecycle: Mutex::new(NativeContextLifecycle::default()),
            leases_drained: Condvar::new(),
            state: Mutex::new(Some(NativeContextState::new())),
        }
    }

    fn wait_for_leases_and_take_state(&self) -> Option<NativeContextState> {
        let mut lifecycle = lock_unpoisoned(&self.lifecycle);
        while lifecycle.active_leases != 0 {
            lifecycle = self
                .leases_drained
                .wait(lifecycle)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
        drop(lifecycle);
        lock_unpoisoned(&self.state).take()
    }
}

struct NativeContextLease {
    entry: Arc<NativeContextEntry>,
}

impl Drop for NativeContextLease {
    fn drop(&mut self) {
        let mut lifecycle = lock_unpoisoned(&self.entry.lifecycle);
        if lifecycle.active_leases == 0 {
            return;
        }
        lifecycle.active_leases -= 1;
        if lifecycle.active_leases == 0 {
            self.entry.leases_drained.notify_all();
        }
    }
}

struct NativeContextRegistry {
    contexts: HashMap<ContextRegistryKey, Arc<NativeContextEntry>>,
    next_token: Option<NonZeroU64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RegisterContextError {
    OwnerAlreadyLive,
    TokenExhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AcquireContextError {
    Invalid,
    LeaseExhausted,
}

impl NativeContextRegistry {
    fn new() -> Self {
        Self {
            contexts: HashMap::new(),
            next_token: NonZeroU64::new(1),
        }
    }

    fn take_token(&mut self) -> Option<u64> {
        let token = self.next_token?;
        self.next_token = token.get().checked_add(1).and_then(NonZeroU64::new);
        Some(token.get())
    }

    fn has_live_owner(&self, owner_address: usize) -> bool {
        self.contexts
            .keys()
            .any(|key| key.owner_address == owner_address)
    }

    fn register(
        &mut self,
        owner_address: usize,
        abi_version: u32,
        entry: Arc<NativeContextEntry>,
    ) -> Result<ContextRegistryKey, RegisterContextError> {
        if self.has_live_owner(owner_address) {
            return Err(RegisterContextError::OwnerAlreadyLive);
        }
        let token = self
            .take_token()
            .ok_or(RegisterContextError::TokenExhausted)?;
        let key = ContextRegistryKey {
            owner_address,
            token,
            abi_version,
        };
        self.contexts.insert(key, entry);
        Ok(key)
    }

    fn retire(&mut self, key: ContextRegistryKey) -> Option<Arc<NativeContextEntry>> {
        self.contexts.remove(&key)
    }
}

fn native_context_registry() -> &'static Mutex<NativeContextRegistry> {
    static REGISTRY: OnceLock<Mutex<NativeContextRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(NativeContextRegistry::new()))
}

fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn context_key(
    context: *const JettRuntimeContextV1,
) -> Result<ContextRegistryKey, JettRuntimeStatusV1> {
    let Some(record) = (unsafe { context.as_ref() }) else {
        return Err(JettRuntimeStatusV1::INVALID_ARGUMENT);
    };
    if !record.has_live_shape() {
        return Err(JettRuntimeStatusV1::INVALID_CONTEXT);
    }
    Ok(ContextRegistryKey {
        owner_address: context.addr(),
        token: record.token,
        abi_version: record.abi_version,
    })
}

fn acquire_context(key: ContextRegistryKey) -> Result<NativeContextLease, AcquireContextError> {
    let registry = lock_unpoisoned(native_context_registry());
    let entry = registry
        .contexts
        .get(&key)
        .cloned()
        .ok_or(AcquireContextError::Invalid)?;
    let mut lifecycle = lock_unpoisoned(&entry.lifecycle);
    let active_leases = lifecycle
        .active_leases
        .checked_add(1)
        .ok_or(AcquireContextError::LeaseExhausted)?;
    lifecycle.active_leases = active_leases;
    drop(lifecycle);
    drop(registry);
    Ok(NativeContextLease { entry })
}

fn retire_context(key: ContextRegistryKey) -> Option<Arc<NativeContextEntry>> {
    lock_unpoisoned(native_context_registry()).retire(key)
}

#[cfg(test)]
fn context_is_registered(context: *const JettRuntimeContextV1) -> bool {
    let Ok(key) = context_key(context) else {
        return false;
    };
    lock_unpoisoned(native_context_registry())
        .contexts
        .contains_key(&key)
}

#[unsafe(export_name = "jett_rt_v1_abi_version")]
pub static JETT_RT_V1_ABI_VERSION_SYMBOL: u32 = JETT_RUNTIME_ABI_VERSION_V1;

/// Check whether the caller's ABI version matches this runtime.
///
/// # Safety
///
/// If non-null, `out_result` must be aligned and valid for one exclusive write
/// of a [`JettRuntimeResultV1`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_check_abi(
    expected_version: u32,
    out_result: *mut JettRuntimeResultV1,
) -> JettRuntimeStatusV1 {
    complete_call(out_result, || {
        if expected_version == JETT_RUNTIME_ABI_VERSION_V1 {
            JettRuntimeResultV1::ok()
        } else {
            JettRuntimeResultV1::failure(
                JettRuntimeStatusV1::ABI_VERSION_MISMATCH,
                ABI_MISMATCH_MESSAGE,
            )
        }
    })
}

/// Initialize one caller-owned runtime context record.
///
/// The output must be exclusively writable, nonoverlapping storage. Concurrent
/// creation at the same address is not permitted. On success the record becomes
/// immutable until destruction and all concurrent calls return.
///
/// # Safety
///
/// Non-null pointers must be aligned, valid for their records, and satisfy the
/// exclusivity, lifetime, and nonoverlap requirements above.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_context_create(
    expected_version: u32,
    out_context: *mut JettRuntimeContextV1,
    out_result: *mut JettRuntimeResultV1,
) -> JettRuntimeStatusV1 {
    complete_call(out_result, || {
        if out_context.is_null() {
            return JettRuntimeResultV1::failure(
                JettRuntimeStatusV1::INVALID_ARGUMENT,
                CONTEXT_OUTPUT_NULL_MESSAGE,
            );
        }

        let owner_address = out_context.addr();
        if lock_unpoisoned(native_context_registry()).has_live_owner(owner_address) {
            return JettRuntimeResultV1::failure(
                JettRuntimeStatusV1::INVALID_CONTEXT,
                CONTEXT_INVALID_MESSAGE,
            );
        }
        unsafe { ptr::write(out_context, JettRuntimeContextV1::retired()) };
        if expected_version != JETT_RUNTIME_ABI_VERSION_V1 {
            return JettRuntimeResultV1::failure(
                JettRuntimeStatusV1::ABI_VERSION_MISMATCH,
                ABI_MISMATCH_MESSAGE,
            );
        }

        let entry = Arc::new(NativeContextEntry::new());
        let registration = lock_unpoisoned(native_context_registry()).register(
            owner_address,
            JETT_RUNTIME_ABI_VERSION_V1,
            entry,
        );
        let key = match registration {
            Ok(key) => key,
            Err(RegisterContextError::OwnerAlreadyLive) => {
                return JettRuntimeResultV1::failure(
                    JettRuntimeStatusV1::INVALID_CONTEXT,
                    CONTEXT_INVALID_MESSAGE,
                );
            }
            Err(RegisterContextError::TokenExhausted) => {
                return JettRuntimeResultV1::failure(
                    JettRuntimeStatusV1::RESOURCE_EXHAUSTED,
                    CONTEXT_EXHAUSTED_MESSAGE,
                );
            }
        };
        unsafe {
            ptr::write(
                out_context,
                JettRuntimeContextV1 {
                    token: key.token,
                    abi_version: key.abi_version,
                    state_tag: LIVE_CONTEXT_TAG,
                },
            )
        };
        JettRuntimeResultV1::ok()
    })
}

/// Destroy one live runtime context without modifying its identity record.
///
/// Retirement is atomic with lease acquisition. Existing calls finish before
/// cleanup; copied, moved, forged, stale, and repeated records are rejected.
///
/// # Safety
///
/// Non-null pointers must be aligned, initialized, readable, immutable for the
/// call, alive until return, and nonoverlapping with the writable result.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_context_destroy(
    context: *mut JettRuntimeContextV1,
    out_result: *mut JettRuntimeResultV1,
) -> JettRuntimeStatusV1 {
    complete_call(out_result, || {
        if context.is_null() {
            return JettRuntimeResultV1::failure(
                JettRuntimeStatusV1::INVALID_ARGUMENT,
                CONTEXT_NULL_MESSAGE,
            );
        }
        let key = match context_key(context.cast_const()) {
            Ok(key) => key,
            Err(status) => {
                return JettRuntimeResultV1::failure(status, CONTEXT_INVALID_MESSAGE);
            }
        };
        let Some(entry) = retire_context(key) else {
            return JettRuntimeResultV1::failure(
                JettRuntimeStatusV1::INVALID_CONTEXT,
                CONTEXT_INVALID_MESSAGE,
            );
        };
        let Some(state) = entry.wait_for_leases_and_take_state() else {
            return JettRuntimeResultV1::failure(JettRuntimeStatusV1::PANIC, PANIC_MESSAGE);
        };
        let leaked = !state.values.is_empty();
        let cleanup_failed = state.values.cleanup_failed;
        drop(state);
        if cleanup_failed {
            return JettRuntimeResultV1::failure(
                JettRuntimeStatusV1::INVALID_ARGUMENT,
                b"native value cleanup failed",
            );
        }
        if leaked {
            return JettRuntimeResultV1::failure(
                JettRuntimeStatusV1::INVALID_ARGUMENT,
                b"native value ownership leak",
            );
        }
        JettRuntimeResultV1::ok()
    })
}

/// Write one borrowed Jett string to the process stdout byte stream.
///
/// # Safety
///
/// Non-null record pointers must be aligned, initialized, readable, immutable,
/// alive through the call, and nonoverlapping with the writable result. A
/// nonempty byte range must be contained in one readable allocation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_stdout_write(
    context: *const JettRuntimeContextV1,
    string: *const JettRuntimeStringSliceV1,
    out_result: *mut JettRuntimeResultV1,
) -> JettRuntimeStatusV1 {
    complete_call(out_result, || {
        if context.is_null() {
            return JettRuntimeResultV1::failure(
                JettRuntimeStatusV1::INVALID_ARGUMENT,
                CONTEXT_NULL_MESSAGE,
            );
        }
        let key = match context_key(context) {
            Ok(key) => key,
            Err(status) => {
                return JettRuntimeResultV1::failure(status, CONTEXT_INVALID_MESSAGE);
            }
        };
        let _lease = match acquire_context(key) {
            Ok(lease) => lease,
            Err(AcquireContextError::Invalid) => {
                return JettRuntimeResultV1::failure(
                    JettRuntimeStatusV1::INVALID_CONTEXT,
                    CONTEXT_INVALID_MESSAGE,
                );
            }
            Err(AcquireContextError::LeaseExhausted) => {
                return JettRuntimeResultV1::failure(
                    JettRuntimeStatusV1::RESOURCE_EXHAUSTED,
                    CONTEXT_EXHAUSTED_MESSAGE,
                );
            }
        };
        let Some(string) = (unsafe { string.as_ref() }) else {
            return JettRuntimeResultV1::failure(
                JettRuntimeStatusV1::INVALID_ARGUMENT,
                STRING_SLICE_NULL_MESSAGE,
            );
        };
        let Ok(byte_length) = usize::try_from(string.byte_length) else {
            return JettRuntimeResultV1::failure(
                JettRuntimeStatusV1::LENGTH_OUT_OF_RANGE,
                STRING_LENGTH_MESSAGE,
            );
        };
        if byte_length > isize::MAX as usize {
            return JettRuntimeResultV1::failure(
                JettRuntimeStatusV1::LENGTH_OUT_OF_RANGE,
                STRING_LENGTH_MESSAGE,
            );
        }
        let bytes = if byte_length == 0 {
            &[]
        } else {
            if string.data.is_null() {
                return JettRuntimeResultV1::failure(
                    JettRuntimeStatusV1::INVALID_ARGUMENT,
                    STRING_NULL_MESSAGE,
                );
            }
            if string.data.addr().checked_add(byte_length).is_none() {
                return JettRuntimeResultV1::failure(
                    JettRuntimeStatusV1::LENGTH_OUT_OF_RANGE,
                    STRING_LENGTH_MESSAGE,
                );
            }
            unsafe { slice::from_raw_parts(string.data, byte_length) }
        };
        if str::from_utf8(bytes).is_err() {
            return JettRuntimeResultV1::failure(
                JettRuntimeStatusV1::INVALID_UTF8,
                STRING_UTF8_MESSAGE,
            );
        }
        match write_all_bytes(io::stdout().lock(), bytes) {
            Ok(()) => JettRuntimeResultV1::ok(),
            Err(_) => {
                JettRuntimeResultV1::failure(JettRuntimeStatusV1::IO_FAILURE, STDOUT_WRITE_MESSAGE)
            }
        }
    })
}

fn write_all_bytes(mut output: impl Write, bytes: &[u8]) -> io::Result<()> {
    output.write_all(bytes)
}

fn complete_call(
    out_result: *mut JettRuntimeResultV1,
    operation: impl FnOnce() -> JettRuntimeResultV1,
) -> JettRuntimeStatusV1 {
    if out_result.is_null() {
        return JettRuntimeStatusV1::INVALID_ARGUMENT;
    }
    let result = match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(result) => result,
        Err(payload) => {
            discard_panic_payload(payload);
            JettRuntimeResultV1::failure(JettRuntimeStatusV1::PANIC, PANIC_MESSAGE)
        }
    };
    let status = result.status;
    unsafe { ptr::write(out_result, result) };
    status
}

#[cfg(test)]
mod tests {
    use std::mem::MaybeUninit;
    use std::panic::panic_any;
    use std::ptr::NonNull;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Barrier, mpsc};
    use std::thread;
    use std::time::{Duration, Instant};

    use crate::{AuthorityProvenance, ResourceTypeId};

    use super::*;

    fn message(result: JettRuntimeResultV1) -> &'static str {
        if result.message.byte_length == 0 {
            return "";
        }
        let length = usize::try_from(result.message.byte_length).expect("test message fits usize");
        let bytes = unsafe { slice::from_raw_parts(result.message.data, length) };
        str::from_utf8(bytes).expect("runtime diagnostic is UTF-8")
    }

    fn check_abi(expected_version: u32) -> (JettRuntimeStatusV1, JettRuntimeResultV1) {
        let mut result = MaybeUninit::uninit();
        let status = unsafe { jett_rt_v1_check_abi(expected_version, result.as_mut_ptr()) };
        (status, unsafe { result.assume_init() })
    }

    fn create_context(
        expected_version: u32,
        context: *mut JettRuntimeContextV1,
    ) -> (JettRuntimeStatusV1, JettRuntimeResultV1) {
        let mut result = MaybeUninit::uninit();
        let status =
            unsafe { jett_rt_v1_context_create(expected_version, context, result.as_mut_ptr()) };
        (status, unsafe { result.assume_init() })
    }

    fn destroy_context(
        context: *mut JettRuntimeContextV1,
    ) -> (JettRuntimeStatusV1, JettRuntimeResultV1) {
        let mut result = MaybeUninit::uninit();
        let status = unsafe { jett_rt_v1_context_destroy(context, result.as_mut_ptr()) };
        (status, unsafe { result.assume_init() })
    }

    fn write_stdout(
        context: *const JettRuntimeContextV1,
        string: *const JettRuntimeStringSliceV1,
    ) -> (JettRuntimeStatusV1, JettRuntimeResultV1) {
        let mut result = MaybeUninit::uninit();
        let status = unsafe { jett_rt_v1_stdout_write(context, string, result.as_mut_ptr()) };
        (status, unsafe { result.assume_init() })
    }

    fn install_cleanup_probe(context: *const JettRuntimeContextV1, count: Arc<AtomicUsize>) {
        let key = context_key(context).unwrap();
        let lease = acquire_context(key).unwrap();
        let mut state = lock_unpoisoned(&lease.entry.state);
        state
            .as_mut()
            .unwrap()
            ._resources
            .insert(
                ResourceTypeId::new(1),
                (),
                AuthorityProvenance::new(1, 1),
                move |_| {
                    count.fetch_add(1, Ordering::SeqCst);
                },
            )
            .unwrap();
    }

    #[test]
    fn abi_version_status_tags_and_layouts_are_stable() {
        assert_eq!(JETT_RUNTIME_ABI_VERSION_V1, 1);
        assert_eq!(JETT_RT_V1_ABI_VERSION_SYMBOL, 1);
        for (status, code) in [
            (JettRuntimeStatusV1::OK, 0),
            (JettRuntimeStatusV1::INVALID_ARGUMENT, 1),
            (JettRuntimeStatusV1::ABI_VERSION_MISMATCH, 2),
            (JettRuntimeStatusV1::INVALID_CONTEXT, 3),
            (JettRuntimeStatusV1::INVALID_UTF8, 4),
            (JettRuntimeStatusV1::IO_FAILURE, 5),
            (JettRuntimeStatusV1::LENGTH_OUT_OF_RANGE, 6),
            (JettRuntimeStatusV1::RESOURCE_EXHAUSTED, 7),
            (JettRuntimeStatusV1::PANIC, 255),
        ] {
            assert_eq!(status.code(), code);
        }
        assert_eq!(size_of::<JettRuntimeContextV1>(), 16);
        assert_eq!(align_of::<JettRuntimeContextV1>(), 8);
        assert_eq!(offset_of!(JettRuntimeContextV1, token), 0);
        assert_eq!(offset_of!(JettRuntimeContextV1, abi_version), 8);
        assert_eq!(offset_of!(JettRuntimeContextV1, state_tag), 12);
    }

    #[test]
    fn version_check_reports_a_deterministic_mismatch() {
        assert_eq!(check_abi(1).0, JettRuntimeStatusV1::OK);
        let (status, result) = check_abi(2);
        assert_eq!(status, JettRuntimeStatusV1::ABI_VERSION_MISMATCH);
        assert_eq!(message(result), "runtime ABI version mismatch");
    }

    #[test]
    fn context_lifecycle_keeps_record_immutable() {
        let mut context = JettRuntimeContextV1::retired();
        assert_eq!(create_context(1, &mut context).0, JettRuntimeStatusV1::OK);
        let published = (context.token, context.abi_version, context.state_tag);
        assert!(context_is_registered(&context));
        assert_eq!(
            write_stdout(&context, &JettRuntimeStringSliceV1::empty()).0,
            JettRuntimeStatusV1::OK
        );
        assert_eq!(destroy_context(&mut context).0, JettRuntimeStatusV1::OK);
        assert_eq!(
            (context.token, context.abi_version, context.state_tag),
            published
        );
        assert_eq!(
            destroy_context(&mut context).0,
            JettRuntimeStatusV1::INVALID_CONTEXT
        );
    }

    #[test]
    fn copied_moved_and_forged_records_cannot_reach_state() {
        let mut original = JettRuntimeContextV1::retired();
        assert_eq!(create_context(1, &mut original).0, JettRuntimeStatusV1::OK);
        let mut copied = JettRuntimeContextV1 {
            token: original.token,
            abi_version: original.abi_version,
            state_tag: original.state_tag,
        };
        let empty = JettRuntimeStringSliceV1::empty();
        assert_eq!(
            write_stdout(&copied, &empty).0,
            JettRuntimeStatusV1::INVALID_CONTEXT
        );
        assert_eq!(
            destroy_context(&mut copied).0,
            JettRuntimeStatusV1::INVALID_CONTEXT
        );
        assert!(context_is_registered(&original));

        let token = original.token;
        original.token = token.wrapping_add(1).max(1);
        assert_eq!(
            write_stdout(&original, &empty).0,
            JettRuntimeStatusV1::INVALID_CONTEXT
        );
        original.token = token;
        assert_eq!(destroy_context(&mut original).0, JettRuntimeStatusV1::OK);
    }

    #[test]
    fn stale_token_fails_after_recreating_at_same_address() {
        let mut context = JettRuntimeContextV1::retired();
        assert_eq!(create_context(1, &mut context).0, JettRuntimeStatusV1::OK);
        let stale_token = context.token;
        assert_eq!(destroy_context(&mut context).0, JettRuntimeStatusV1::OK);
        assert_eq!(create_context(1, &mut context).0, JettRuntimeStatusV1::OK);
        let live_token = context.token;
        assert!(live_token > stale_token);
        context.token = stale_token;
        assert_eq!(
            write_stdout(&context, &JettRuntimeStringSliceV1::empty()).0,
            JettRuntimeStatusV1::INVALID_CONTEXT
        );
        context.token = live_token;
        assert_eq!(destroy_context(&mut context).0, JettRuntimeStatusV1::OK);
    }

    #[test]
    fn tokens_are_unique_and_exhaustion_never_wraps() {
        let mut first = JettRuntimeContextV1::retired();
        let mut second = JettRuntimeContextV1::retired();
        assert_eq!(create_context(1, &mut first).0, JettRuntimeStatusV1::OK);
        assert_eq!(create_context(1, &mut second).0, JettRuntimeStatusV1::OK);
        assert!(second.token > first.token);
        assert_eq!(destroy_context(&mut first).0, JettRuntimeStatusV1::OK);
        assert_eq!(destroy_context(&mut second).0, JettRuntimeStatusV1::OK);

        let mut registry = NativeContextRegistry {
            contexts: HashMap::new(),
            next_token: NonZeroU64::new(u64::MAX),
        };
        assert_eq!(registry.take_token(), Some(u64::MAX));
        assert_eq!(registry.take_token(), None);
        assert_eq!(registry.take_token(), None);
    }

    #[test]
    fn destroy_retires_then_waits_for_in_flight_lease() {
        let mut context = Box::new(JettRuntimeContextV1::retired());
        assert_eq!(create_context(1, &mut *context).0, JettRuntimeStatusV1::OK);
        let cleanup_count = Arc::new(AtomicUsize::new(0));
        install_cleanup_probe(&*context, Arc::clone(&cleanup_count));
        let key = context_key(&*context).unwrap();
        let lease = acquire_context(key).unwrap();
        let address = (&*context as *const JettRuntimeContextV1).addr();
        let (sender, receiver) = mpsc::channel();
        let destroyer = thread::spawn(move || {
            sender
                .send(destroy_context(address as *mut JettRuntimeContextV1).0)
                .unwrap();
        });

        let deadline = Instant::now() + Duration::from_secs(2);
        while context_is_registered(&*context) {
            assert!(Instant::now() < deadline, "destroy did not retire context");
            thread::yield_now();
        }
        assert_eq!(
            acquire_context(key).err(),
            Some(AcquireContextError::Invalid)
        );
        assert!(receiver.recv_timeout(Duration::from_millis(25)).is_err());
        assert_eq!(cleanup_count.load(Ordering::SeqCst), 0);
        drop(lease);
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(2)).unwrap(),
            JettRuntimeStatusV1::OK
        );
        destroyer.join().unwrap();
        assert_eq!(cleanup_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn concurrent_double_destroy_cleans_once() {
        let mut context = Box::new(JettRuntimeContextV1::retired());
        assert_eq!(create_context(1, &mut *context).0, JettRuntimeStatusV1::OK);
        let cleanup_count = Arc::new(AtomicUsize::new(0));
        install_cleanup_probe(&*context, Arc::clone(&cleanup_count));
        let address = (&*context as *const JettRuntimeContextV1).addr();
        let barrier = Arc::new(Barrier::new(3));
        let mut workers = Vec::new();
        for _ in 0..2 {
            let barrier = Arc::clone(&barrier);
            workers.push(thread::spawn(move || {
                barrier.wait();
                destroy_context(address as *mut JettRuntimeContextV1)
                    .0
                    .code()
            }));
        }
        barrier.wait();
        let mut statuses = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect::<Vec<_>>();
        statuses.sort_unstable();
        assert_eq!(statuses, [0, 3]);
        assert_eq!(cleanup_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn cleanup_panic_is_reported_once_after_retirement() {
        let mut context = JettRuntimeContextV1::retired();
        assert_eq!(create_context(1, &mut context).0, JettRuntimeStatusV1::OK);
        let lease = acquire_context(context_key(&context).unwrap()).unwrap();
        let count = Arc::new(AtomicUsize::new(0));
        let finalizer_count = Arc::clone(&count);
        lock_unpoisoned(&lease.entry.state)
            .as_mut()
            .unwrap()
            ._resources
            .insert(
                ResourceTypeId::new(1),
                (),
                AuthorityProvenance::new(1, 1),
                move |_| {
                    finalizer_count.fetch_add(1, Ordering::SeqCst);
                    panic!("cleanup failure");
                },
            )
            .unwrap();
        drop(lease);
        assert_eq!(destroy_context(&mut context).0, JettRuntimeStatusV1::PANIC);
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(
            destroy_context(&mut context).0,
            JettRuntimeStatusV1::INVALID_CONTEXT
        );
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn invalid_inputs_and_lengths_have_stable_failures() {
        assert_eq!(
            create_context(1, ptr::null_mut()).0,
            JettRuntimeStatusV1::INVALID_ARGUMENT
        );
        let mut context = JettRuntimeContextV1::retired();
        assert_eq!(
            create_context(9, &mut context).0,
            JettRuntimeStatusV1::ABI_VERSION_MISMATCH
        );
        assert!(!context.has_live_shape());
        assert_eq!(
            destroy_context(ptr::null_mut()).0,
            JettRuntimeStatusV1::INVALID_ARGUMENT
        );
        assert_eq!(
            unsafe { jett_rt_v1_check_abi(1, ptr::null_mut()) },
            JettRuntimeStatusV1::INVALID_ARGUMENT
        );

        assert_eq!(create_context(1, &mut context).0, JettRuntimeStatusV1::OK);
        let null_data = JettRuntimeStringSliceV1 {
            data: ptr::null(),
            byte_length: 1,
        };
        assert_eq!(
            write_stdout(&context, &null_data).0,
            JettRuntimeStatusV1::INVALID_ARGUMENT
        );
        let too_long = JettRuntimeStringSliceV1 {
            data: NonNull::<u8>::dangling().as_ptr(),
            byte_length: (isize::MAX as u64) + 1,
        };
        let (status, result) = write_stdout(&context, &too_long);
        assert_eq!(status, JettRuntimeStatusV1::LENGTH_OUT_OF_RANGE);
        assert_eq!(
            message(result),
            "string byte length exceeds host address space"
        );
        let invalid_utf8_bytes = [0xff];
        let invalid_utf8 = JettRuntimeStringSliceV1 {
            data: invalid_utf8_bytes.as_ptr(),
            byte_length: 1,
        };
        assert_eq!(
            write_stdout(&context, &invalid_utf8).0,
            JettRuntimeStatusV1::INVALID_UTF8
        );
        assert_eq!(destroy_context(&mut context).0, JettRuntimeStatusV1::OK);
    }

    #[test]
    fn stdout_writer_preserves_exact_bytes_without_newline() {
        let mut output = Vec::new();
        write_all_bytes(&mut output, b"hello, world").unwrap();
        assert_eq!(output, b"hello, world");
    }

    struct PanicOnDrop(Arc<AtomicUsize>);

    impl Drop for PanicOnDrop {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
            panic!("panic payload must not be dropped");
        }
    }

    #[test]
    fn recoverable_panic_is_contained_without_dropping_payload() {
        let drops = Arc::new(AtomicUsize::new(0));
        let panic_drops = Arc::clone(&drops);
        let mut result = MaybeUninit::uninit();
        let status = complete_call(result.as_mut_ptr(), || panic_any(PanicOnDrop(panic_drops)));
        let result = unsafe { result.assume_init() };
        assert_eq!(status, JettRuntimeStatusV1::PANIC);
        assert_eq!(message(result), "runtime operation panicked");
        assert_eq!(drops.load(Ordering::SeqCst), 0);
    }
}
