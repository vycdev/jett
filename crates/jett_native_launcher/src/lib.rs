//! Process launcher for version 1 Jett AOT objects.
//!
//! The static library supplies the C `main` symbol that owns one runtime
//! context for the duration of `jett_aot_v1_entry`. All external calls use the
//! fixed version 1 C ABI; no Rust ABI value crosses the object boundary.

#[cfg(not(test))]
use std::ffi::c_char;
use std::ffi::{c_int, c_void};
#[cfg(not(test))]
use std::io;
use std::io::Write;
use std::mem::{self, MaybeUninit};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::slice;

use jett_runtime::native_abi::{
    JETT_RUNTIME_ABI_VERSION_V1, JettRuntimeContextV1, JettRuntimeResultV1, JettRuntimeStatusV1,
    JettRuntimeStringSliceV1, jett_rt_v1_context_create, jett_rt_v1_context_destroy,
};

/// Successful status returned by the version 1 AOT entry point.
pub const JETT_AOT_ENTRY_SUCCESS_V1: u32 = 0;

/// The launcher completed successfully.
pub const JETT_LAUNCHER_EXIT_SUCCESS: c_int = 0;
/// Runtime context creation failed.
pub const JETT_LAUNCHER_EXIT_CREATE_FAILURE: c_int = 70;
/// The generated Jett entry point returned a failure status.
pub const JETT_LAUNCHER_EXIT_ENTRY_FAILURE: c_int = 71;
/// Runtime context destruction failed after the context was created.
pub const JETT_LAUNCHER_EXIT_DESTROY_FAILURE: c_int = 72;
/// A recoverable unwind was caught inside the launcher.
pub const JETT_LAUNCHER_EXIT_PANIC: c_int = 73;

const CREATE_PANIC_MESSAGE: &[u8] = b"jett launcher: runtime context creation panicked\n";
const ENTRY_PANIC_MESSAGE: &[u8] = b"jett launcher: program entry panicked\n";
const DESTROY_PANIC_MESSAGE: &[u8] = b"jett launcher: runtime context destruction panicked\n";
#[cfg(not(test))]
const LAUNCHER_PANIC_MESSAGE: &[u8] = b"jett launcher: launcher panicked\n";
const INVALID_RUNTIME_MESSAGE: &[u8] = b"invalid runtime failure message";

#[cfg(not(test))]
unsafe extern "C" {
    fn jett_aot_v1_entry(context: *mut c_void) -> u32;
}

#[derive(Debug, Clone, Copy)]
struct RuntimeCall {
    status: JettRuntimeStatusV1,
    result: JettRuntimeResultV1,
}

trait RuntimeLifecycle {
    fn create(&mut self, out_context: *mut JettRuntimeContextV1) -> RuntimeCall;
    fn destroy(&mut self, context: *mut JettRuntimeContextV1) -> RuntimeCall;
    fn failure(&mut self, _context: *mut JettRuntimeContextV1) -> Option<RuntimeCall> {
        None
    }
    fn failure_message(&mut self, _context: *mut JettRuntimeContextV1) -> Option<Vec<u8>> {
        None
    }
}

#[derive(Default)]
struct NativeRuntime;

impl RuntimeLifecycle for NativeRuntime {
    fn create(&mut self, out_context: *mut JettRuntimeContextV1) -> RuntimeCall {
        let mut result = MaybeUninit::<JettRuntimeResultV1>::uninit();
        // SAFETY: `out_context` is supplied by `launch_with` as aligned,
        // exclusively writable storage, and `result` is a distinct writable
        // result record. The runtime initializes the result on every return.
        let status = unsafe {
            jett_rt_v1_context_create(
                JETT_RUNTIME_ABI_VERSION_V1,
                out_context,
                result.as_mut_ptr(),
            )
        };
        RuntimeCall {
            status,
            // SAFETY: the runtime v1 call contract initializes the result
            // record for every returned status.
            result: unsafe { result.assume_init() },
        }
    }

    fn failure(&mut self, context: *mut JettRuntimeContextV1) -> Option<RuntimeCall> {
        let mut result = MaybeUninit::uninit();
        // SAFETY: launch_with retains the stationary context until destruction.
        let status = unsafe {
            jett_runtime::native_abi::values::jett_rt_v1_value_failure(context, result.as_mut_ptr())
        };
        if status == JettRuntimeStatusV1::OK {
            None
        } else {
            Some(RuntimeCall {
                status,
                result: unsafe { result.assume_init() },
            })
        }
    }

    fn failure_message(&mut self, context: *mut JettRuntimeContextV1) -> Option<Vec<u8>> {
        let mut length = 0_u64;
        let mut result = MaybeUninit::uninit();
        // SAFETY: the live context and distinct outputs remain valid through
        // this call. A null, zero-capacity buffer queries the required length.
        let status = unsafe {
            jett_runtime::native_abi::values::jett_rt_v1_value_failure_copy(
                context,
                std::ptr::null_mut(),
                0,
                &mut length,
                result.as_mut_ptr(),
            )
        };
        if status != JettRuntimeStatusV1::OK {
            return None;
        }
        let required = length;
        let length = usize::try_from(required).ok()?;
        let mut message = Vec::new();
        message.try_reserve_exact(length).ok()?;
        message.resize(length, 0);
        let mut copied = 0_u64;
        // SAFETY: `message` owns `length` writable bytes, and all output
        // records are distinct. The runtime copies while the context is live.
        let status = unsafe {
            jett_runtime::native_abi::values::jett_rt_v1_value_failure_copy(
                context,
                message.as_mut_ptr(),
                required,
                &mut copied,
                result.as_mut_ptr(),
            )
        };
        (status == JettRuntimeStatusV1::OK && copied == required).then_some(message)
    }

    fn destroy(&mut self, context: *mut JettRuntimeContextV1) -> RuntimeCall {
        let mut result = MaybeUninit::<JettRuntimeResultV1>::uninit();
        // SAFETY: `launch_with` calls destroy only for the same initialized,
        // stationary context whose creation succeeded. The result storage is
        // distinct and exclusively writable for this call.
        let status = unsafe { jett_rt_v1_context_destroy(context, result.as_mut_ptr()) };
        RuntimeCall {
            status,
            // SAFETY: the runtime v1 call contract initializes the result
            // record for every returned status.
            result: unsafe { result.assume_init() },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntryOutcome {
    Status(u32),
    Panicked,
}

#[derive(Debug, Clone, Copy)]
enum DestroyOutcome {
    Completed(RuntimeCall),
    Panicked,
}

fn catch_operation<T>(operation: impl FnOnce() -> T) -> Result<T, ()> {
    catch_unwind(AssertUnwindSafe(operation)).map_err(|payload| {
        // A panic payload may have a panicking destructor. Leaking it is the
        // only bounded way to guarantee that this C-facing path does not start
        // a second unwind while handling the first one.
        mem::forget(payload);
    })
}

fn launch_with<R, E, W>(runtime: &mut R, entry: E, stderr: &mut W) -> c_int
where
    R: RuntimeLifecycle,
    E: FnOnce(*mut c_void) -> u32,
    W: Write,
{
    let mut context = MaybeUninit::<JettRuntimeContextV1>::uninit();
    let create = match catch_operation(|| runtime.create(context.as_mut_ptr())) {
        Ok(call) => call,
        Err(()) => {
            write_bytes(stderr, CREATE_PANIC_MESSAGE);
            return JETT_LAUNCHER_EXIT_PANIC;
        }
    };
    if create.status != JettRuntimeStatusV1::OK {
        write_runtime_failure(stderr, b"runtime context creation", create);
        return JETT_LAUNCHER_EXIT_CREATE_FAILURE;
    }

    // The context must remain at the exact address registered by create. Keep
    // it in its original `MaybeUninit` storage rather than moving it through
    // `assume_init`.
    let context_pointer = context.as_mut_ptr().cast::<c_void>();
    let entry_outcome = match catch_operation(|| entry(context_pointer)) {
        Ok(status) => EntryOutcome::Status(status),
        Err(()) => EntryOutcome::Panicked,
    };

    let entry_failure = runtime.failure(context.as_mut_ptr());
    let entry_message = entry_failure
        .as_ref()
        .and_then(|_| runtime.failure_message(context.as_mut_ptr()));

    // Cleanup is attempted exactly once after every successful creation,
    // including entry failures and recoverable entry panics.
    let destroy_outcome = match catch_operation(|| runtime.destroy(context.as_mut_ptr())) {
        Ok(call) => DestroyOutcome::Completed(call),
        Err(()) => DestroyOutcome::Panicked,
    };

    let mut exit = match entry_outcome {
        EntryOutcome::Status(JETT_AOT_ENTRY_SUCCESS_V1) => JETT_LAUNCHER_EXIT_SUCCESS,
        EntryOutcome::Status(status) => {
            if let Some(failure) = entry_failure {
                let _ = stderr.write_all(b"runtime error: ");
                let _ = stderr.write_all(
                    entry_message
                        .as_deref()
                        .or_else(|| runtime_message(failure.result.message))
                        .unwrap_or(INVALID_RUNTIME_MESSAGE),
                );
                let _ = stderr.write_all(b"\n");
            } else {
                write_entry_failure(stderr, status);
            }
            JETT_LAUNCHER_EXIT_ENTRY_FAILURE
        }
        EntryOutcome::Panicked => {
            write_bytes(stderr, ENTRY_PANIC_MESSAGE);
            JETT_LAUNCHER_EXIT_PANIC
        }
    };

    match destroy_outcome {
        DestroyOutcome::Completed(call) if call.status == JettRuntimeStatusV1::OK => {}
        DestroyOutcome::Completed(call) => {
            write_runtime_failure(stderr, b"runtime context destruction", call);
            // A cleanup failure means the runtime could not uphold its
            // exactly-once resource contract. Treat it as the terminal
            // infrastructure outcome even when the Jett entry failed first.
            exit = JETT_LAUNCHER_EXIT_DESTROY_FAILURE;
        }
        DestroyOutcome::Panicked => {
            write_bytes(stderr, DESTROY_PANIC_MESSAGE);
            exit = JETT_LAUNCHER_EXIT_PANIC;
        }
    }

    exit
}

fn write_entry_failure<W: Write>(stderr: &mut W, status: u32) {
    let _ = writeln!(
        stderr,
        "jett launcher: program entry failed (status {status})"
    );
}

fn write_runtime_failure<W: Write>(stderr: &mut W, phase: &[u8], call: RuntimeCall) {
    let _ = stderr.write_all(b"jett launcher: ");
    let _ = stderr.write_all(phase);
    let _ = write!(stderr, " failed (status {})", call.status.code());
    let message = runtime_message(call.result.message).unwrap_or(INVALID_RUNTIME_MESSAGE);
    if !message.is_empty() {
        let _ = stderr.write_all(b": ");
        let _ = stderr.write_all(message);
    }
    let _ = stderr.write_all(b"\n");
}

fn runtime_message(message: JettRuntimeStringSliceV1) -> Option<&'static [u8]> {
    let length = usize::try_from(message.byte_length).ok()?;
    if length == 0 {
        return Some(&[]);
    }
    if message.data.is_null() {
        return None;
    }
    // SAFETY: the runtime ABI promises that result messages point to immutable
    // process-lifetime storage containing `byte_length` readable bytes.
    Some(unsafe { slice::from_raw_parts(message.data, length) })
}

fn write_bytes<W: Write>(stderr: &mut W, message: &[u8]) {
    let _ = stderr.write_all(message);
}

#[cfg(not(test))]
fn run_native_launcher<W: Write>(stderr: &mut W) -> c_int {
    let mut runtime = NativeRuntime;
    let clock_script = std::env::var(jett_runtime::clock::TEST_SCRIPT_ENV).ok();
    let random_script = std::env::var(jett_runtime::random::TEST_SCRIPT_ENV).ok();
    let environment_snapshot = std::env::var(jett_runtime::environment::TEST_SNAPSHOT_ENV).ok();
    let graphics_script = std::env::var(jett_runtime::graphics::TEST_SCRIPT_ENV).ok();
    launch_with(
        &mut runtime,
        |context| {
            if let Some(script) = &clock_script {
                // SAFETY: `context` is live and stationary, and `script` remains
                // readable for the duration of the configuration call.
                let status = unsafe {
                    jett_runtime::native_abi::values::jett_rt_v1_clock_configure_scripted(
                        context.cast(),
                        script.as_ptr(),
                        script.len() as u64,
                    )
                };
                if status != JETT_AOT_ENTRY_SUCCESS_V1 {
                    return status;
                }
            }
            if let Some(script) = &random_script {
                // SAFETY: `context` is live and stationary, and `script` remains
                // readable for the duration of the configuration call.
                let status = unsafe {
                    jett_runtime::native_abi::values::jett_rt_v1_random_configure_scripted(
                        context.cast(),
                        script.as_ptr(),
                        script.len() as u64,
                    )
                };
                if status != JETT_AOT_ENTRY_SUCCESS_V1 {
                    return status;
                }
            }
            if let Some(snapshot) = &environment_snapshot {
                // SAFETY: the context is live and `snapshot` is readable for
                // the duration of this configuration call.
                let status = unsafe {
                    jett_runtime::native_abi::values::jett_rt_v1_environment_configure_snapshot(
                        context.cast(),
                        snapshot.as_ptr(),
                        snapshot.len() as u64,
                    )
                };
                if status != JETT_AOT_ENTRY_SUCCESS_V1 {
                    return status;
                }
            }
            if let Some(script) = &graphics_script {
                // SAFETY: the context is live and `script` is readable for
                // the duration of this configuration call.
                let status = unsafe {
                    jett_runtime::native_abi::values::jett_rt_v1_graphics_configure_scripted(
                        context.cast(),
                        script.as_ptr(),
                        script.len() as u64,
                    )
                };
                if status != JETT_AOT_ENTRY_SUCCESS_V1 {
                    return status;
                }
            }
            // SAFETY: the generated object implements the committed version 1
            // C entry ABI, and `context` remains live and stationary until the
            // entry call returns.
            let status = unsafe { jett_aot_v1_entry(context) };
            if status != JETT_AOT_ENTRY_SUCCESS_V1 {
                return status;
            }
            // A successful program must consume every scripted provider input.
            // The check records a terminal runtime failure with the same
            // provider priority and message as the interpreter runner.
            unsafe {
                jett_runtime::native_abi::values::jett_rt_v1_validate_scripted_consumption(
                    context.cast(),
                )
            }
        },
        stderr,
    )
}

#[cfg(not(test))]
fn report_top_level_panic() {
    if let Err(payload) = catch_unwind(AssertUnwindSafe(|| {
        let stderr = io::stderr();
        write_bytes(&mut stderr.lock(), LAUNCHER_PANIC_MESSAGE);
    })) {
        mem::forget(payload);
    }
}

/// C process entry point linked with one generated Jett AOT object.
///
/// Version 1 does not expose process arguments to generated code yet, so
/// `argc` and `argv` are deliberately ignored after the C runtime supplies
/// them. They remain in the signature to preserve the platform C entry ABI.
#[cfg(not(test))]
#[unsafe(no_mangle)]
pub extern "C" fn main(_argc: c_int, _argv: *mut *mut c_char) -> c_int {
    match catch_unwind(AssertUnwindSafe(|| {
        let stderr = io::stderr();
        run_native_launcher(&mut stderr.lock())
    })) {
        Ok(exit) => exit,
        Err(payload) => {
            mem::forget(payload);
            report_top_level_panic();
            JETT_LAUNCHER_EXIT_PANIC
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CREATE_FAILURE_MESSAGE: &[u8] = b"test create failure";
    const DESTROY_FAILURE_MESSAGE: &[u8] = b"test destroy failure";

    #[derive(Default)]
    struct TrackingRuntime {
        native: NativeRuntime,
        create_failure: bool,
        destroy_failure: bool,
        panic_on_destroy: bool,
        create_calls: usize,
        destroy_calls: usize,
    }

    impl RuntimeLifecycle for TrackingRuntime {
        fn create(&mut self, out_context: *mut JettRuntimeContextV1) -> RuntimeCall {
            self.create_calls += 1;
            if self.create_failure {
                return failed_call(
                    JettRuntimeStatusV1::RESOURCE_EXHAUSTED,
                    CREATE_FAILURE_MESSAGE,
                );
            }
            self.native.create(out_context)
        }

        fn destroy(&mut self, context: *mut JettRuntimeContextV1) -> RuntimeCall {
            self.destroy_calls += 1;
            let native = self.native.destroy(context);
            assert_eq!(native.status, JettRuntimeStatusV1::OK);
            if self.panic_on_destroy {
                panic!("injected destroy panic");
            }
            if self.destroy_failure {
                return failed_call(
                    JettRuntimeStatusV1::INVALID_CONTEXT,
                    DESTROY_FAILURE_MESSAGE,
                );
            }
            native
        }
    }

    fn failed_call(status: JettRuntimeStatusV1, message: &'static [u8]) -> RuntimeCall {
        RuntimeCall {
            status,
            result: JettRuntimeResultV1 {
                status,
                message: JettRuntimeStringSliceV1 {
                    data: message.as_ptr(),
                    byte_length: u64::try_from(message.len()).unwrap_or(u64::MAX),
                },
            },
        }
    }

    #[test]
    fn successful_entry_creates_and_destroys_one_context() {
        let mut runtime = TrackingRuntime::default();
        let mut stderr = Vec::new();

        let exit = launch_with(&mut runtime, |_| JETT_AOT_ENTRY_SUCCESS_V1, &mut stderr);

        assert_eq!(exit, JETT_LAUNCHER_EXIT_SUCCESS);
        assert_eq!(runtime.create_calls, 1);
        assert_eq!(runtime.destroy_calls, 1);
        assert!(stderr.is_empty());
    }

    #[test]
    fn entry_failure_still_destroys_the_context() {
        let mut runtime = TrackingRuntime::default();
        let mut stderr = Vec::new();

        let exit = launch_with(&mut runtime, |_| 19, &mut stderr);

        assert_eq!(exit, JETT_LAUNCHER_EXIT_ENTRY_FAILURE);
        assert_eq!(runtime.create_calls, 1);
        assert_eq!(runtime.destroy_calls, 1);
        assert_eq!(stderr, b"jett launcher: program entry failed (status 19)\n");
    }

    #[test]
    fn dynamic_assertion_message_is_copied_before_context_destruction() {
        let mut runtime = NativeRuntime;
        let mut stderr = Vec::new();
        let exit = launch_with(
            &mut runtime,
            |context| {
                let context = context.cast::<JettRuntimeContextV1>();
                let message = "expected 42, got 🧪";
                // SAFETY: launch_with supplies a live context and the source
                // string remains readable for the literal call.
                unsafe {
                    let handle = jett_runtime::native_abi::values::jett_rt_v1_string_literal(
                        context,
                        message.as_ptr(),
                        message.len() as u64,
                    );
                    let status = jett_runtime::native_abi::values::jett_rt_v1_assert_fail_message(
                        context, handle,
                    );
                    assert_eq!(
                        jett_runtime::native_abi::values::jett_rt_v1_string_release(
                            context, handle,
                        ),
                        0
                    );
                    status
                }
            },
            &mut stderr,
        );
        assert_eq!(exit, JETT_LAUNCHER_EXIT_ENTRY_FAILURE);
        assert_eq!(stderr, "runtime error: expected 42, got 🧪\n".as_bytes());
    }

    #[test]
    fn creation_failure_skips_entry_and_destruction() {
        let mut runtime = TrackingRuntime {
            create_failure: true,
            ..TrackingRuntime::default()
        };
        let mut stderr = Vec::new();
        let mut entry_called = false;

        let exit = launch_with(
            &mut runtime,
            |_| {
                entry_called = true;
                JETT_AOT_ENTRY_SUCCESS_V1
            },
            &mut stderr,
        );

        assert_eq!(exit, JETT_LAUNCHER_EXIT_CREATE_FAILURE);
        assert!(!entry_called);
        assert_eq!(runtime.create_calls, 1);
        assert_eq!(runtime.destroy_calls, 0);
        assert_eq!(
            stderr,
            b"jett launcher: runtime context creation failed (status 7): test create failure\n"
        );
    }

    #[test]
    fn destruction_failure_has_its_own_exit_code() {
        let mut runtime = TrackingRuntime {
            destroy_failure: true,
            ..TrackingRuntime::default()
        };
        let mut stderr = Vec::new();

        let exit = launch_with(&mut runtime, |_| JETT_AOT_ENTRY_SUCCESS_V1, &mut stderr);

        assert_eq!(exit, JETT_LAUNCHER_EXIT_DESTROY_FAILURE);
        assert_eq!(runtime.destroy_calls, 1);
        assert_eq!(
            stderr,
            b"jett launcher: runtime context destruction failed (status 3): test destroy failure\n"
        );
    }

    #[test]
    fn destruction_failure_overrides_an_entry_failure() {
        let mut runtime = TrackingRuntime {
            destroy_failure: true,
            ..TrackingRuntime::default()
        };
        let mut stderr = Vec::new();

        let exit = launch_with(&mut runtime, |_| 19, &mut stderr);

        assert_eq!(exit, JETT_LAUNCHER_EXIT_DESTROY_FAILURE);
        assert_eq!(runtime.destroy_calls, 1);
        assert_eq!(
            stderr,
            b"jett launcher: program entry failed (status 19)\njett launcher: runtime context destruction failed (status 3): test destroy failure\n"
        );
    }

    #[test]
    fn recoverable_entry_panic_still_runs_cleanup() {
        let mut runtime = TrackingRuntime::default();
        let mut stderr = Vec::new();

        let exit = launch_with(
            &mut runtime,
            |_| panic!("injected entry panic"),
            &mut stderr,
        );

        assert_eq!(exit, JETT_LAUNCHER_EXIT_PANIC);
        assert_eq!(runtime.destroy_calls, 1);
        assert_eq!(stderr, ENTRY_PANIC_MESSAGE);
    }

    #[test]
    fn recoverable_destroy_panic_is_contained() {
        let mut runtime = TrackingRuntime {
            panic_on_destroy: true,
            ..TrackingRuntime::default()
        };
        let mut stderr = Vec::new();

        let exit = launch_with(&mut runtime, |_| JETT_AOT_ENTRY_SUCCESS_V1, &mut stderr);

        assert_eq!(exit, JETT_LAUNCHER_EXIT_PANIC);
        assert_eq!(runtime.destroy_calls, 1);
        assert_eq!(stderr, DESTROY_PANIC_MESSAGE);
    }

    #[test]
    fn native_owned_value_leak_cannot_report_successful_failure_cleanup() {
        let mut runtime = NativeRuntime;
        let mut stderr = Vec::new();
        let exit = launch_with(
            &mut runtime,
            |context| {
                let text = b"deliberately unreleased";
                let value = unsafe {
                    jett_runtime::native_abi::values::jett_rt_v1_string_literal(
                        context.cast(),
                        text.as_ptr(),
                        text.len() as u64,
                    )
                };
                assert_ne!(value, 0);
                19
            },
            &mut stderr,
        );
        assert_eq!(exit, JETT_LAUNCHER_EXIT_DESTROY_FAILURE);
        assert_eq!(stderr, b"jett launcher: program entry failed (status 19)\njett launcher: runtime context destruction failed (status 1): native value ownership leak\n");
    }

    #[test]
    fn native_terminal_failure_releases_values_and_preserves_message() {
        let mut runtime = NativeRuntime;
        let mut stderr = Vec::new();
        let exit = launch_with(
            &mut runtime,
            |context| {
                use jett_runtime::native_abi::values::*;
                unsafe {
                    let context = context.cast();
                    let value = jett_rt_v1_string_literal(context, b"ab".as_ptr(), 2);
                    assert_eq!(jett_rt_v1_string_repeat(context, value, i64::MAX), 0);
                    jett_rt_v1_string_release(context, value);
                    jett_rt_v1_value_status(context)
                }
            },
            &mut stderr,
        );
        assert_eq!(exit, JETT_LAUNCHER_EXIT_ENTRY_FAILURE);
        assert_eq!(
            stderr,
            b"runtime error: string.repeat: requested output is too large\n"
        );
    }
    #[test]
    fn native_double_drop_overrides_terminal_failure_without_erasing_it() {
        let mut runtime = NativeRuntime;
        let mut stderr = Vec::new();
        let exit = launch_with(
            &mut runtime,
            |context| {
                use jett_runtime::native_abi::values::*;
                unsafe {
                    let context = context.cast();
                    let value = jett_rt_v1_bytes_new(context);
                    let text = jett_rt_v1_string_literal(context, b"ab".as_ptr(), 2);
                    jett_rt_v1_string_repeat(context, text, i64::MAX);
                    jett_rt_v1_value_drop(context, value);
                    jett_rt_v1_value_drop(context, value);
                    jett_rt_v1_value_drop(context, text);
                    jett_rt_v1_value_status(context)
                }
            },
            &mut stderr,
        );
        assert_eq!(exit, JETT_LAUNCHER_EXIT_DESTROY_FAILURE);
        assert_eq!(stderr, b"runtime error: string.repeat: requested output is too large\njett launcher: runtime context destruction failed (status 1): native value cleanup failed\n");
    }
}
