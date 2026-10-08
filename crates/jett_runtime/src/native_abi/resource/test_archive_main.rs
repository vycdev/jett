//! Linked only into the single matched private compiler-test runtime archive.
//! The parent module must gate this file on both test and the private archive cfg.
#![cfg(all(test, jett_resource_native_test_archive))]

use crate::native_abi::{
    JETT_RUNTIME_ABI_VERSION_V1, JettRuntimeContextV1, JettRuntimeResultV1, JettRuntimeStatusV1,
    jett_rt_v1_context_create, jett_rt_v1_context_destroy,
};
use std::ffi::{c_char, c_int, c_void};
use std::mem::{MaybeUninit, align_of, offset_of, size_of};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;

#[path = "test_archive_main/report.rs"]
mod report;
use report::{Attempt, Report};

const EXIT_CREATE: c_int = 70;
const EXIT_ENTRY: c_int = 71;
const EXIT_DESTROY: c_int = 72;
const EXIT_PANIC: c_int = 73;
const EXIT_INFRASTRUCTURE: c_int = 74;
const MAX_SCRIPT: usize = 1_048_576;

#[repr(C)]
struct ObjectManifest {
    layout: *const u8,
    length: u64,
    function: u32,
    signature: u32,
    scope: u32,
    reserved: u32,
}
#[repr(C)]
struct ArchiveInfo {
    runtime_abi: u32,
    protocol: u32,
    wire: u32,
    pointer_bits: u32,
    profile: u32,
    reserved: [u32; 3],
}
#[repr(C)]
struct Completion {
    attempt: u64,
    body_status: u32,
    cleanup_status: u32,
    selected_kind: u32,
    reserved: u32,
}
#[repr(C)]
struct Counts {
    owners: u64,
    loans: u64,
    frames: u64,
    registry_entries: u64,
    owner_handles: u64,
    loan_handles: u64,
    frame_handles: u64,
    provisional_returns: u64,
    ordinary_empty: u32,
    reserved: u32,
}
impl Counts {
    fn clean(&self) -> bool {
        self.reserved == 0
            && self.ordinary_empty == 1
            && [
                self.owners,
                self.loans,
                self.frames,
                self.registry_entries,
                self.owner_handles,
                self.loan_handles,
                self.frame_handles,
                self.provisional_returns,
            ]
            .into_iter()
            .all(|value| value == 0)
    }
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Event {
    sequence: u64,
    label: i64,
    kind: u32,
    reserved: u32,
}

const _: () = {
    assert!(size_of::<ObjectManifest>() == 32 && align_of::<ObjectManifest>() == 8);
    assert!(offset_of!(ObjectManifest, function) == 16 && offset_of!(ObjectManifest, scope) == 24);
    assert!(size_of::<ArchiveInfo>() == 32 && align_of::<ArchiveInfo>() == 4);
    assert!(size_of::<Completion>() == 24 && offset_of!(Completion, body_status) == 8);
    assert!(size_of::<Counts>() == 72 && offset_of!(Counts, ordinary_empty) == 64);
    assert!(size_of::<Event>() == 24 && offset_of!(Event, kind) == 16);
};

unsafe extern "C" {
    fn jett_aot_v1_entry(context: *mut c_void) -> u32;
    fn jett_aot_resource_v1_manifest(out: *mut ObjectManifest) -> u32;
    fn jett_rt_v1_resource_test_archive_info(
        out: *mut ArchiveInfo,
        result: *mut JettRuntimeResultV1,
    ) -> JettRuntimeStatusV1;
    fn jett_rt_v1_resource_test_install(
        context: *const JettRuntimeContextV1,
        manifest: *const ObjectManifest,
        script: *const u8,
        length: u64,
        result: *mut JettRuntimeResultV1,
    ) -> JettRuntimeStatusV1;
    fn jett_rt_v1_resource_test_entry_begin(
        context: *const JettRuntimeContextV1,
        attempt: *mut u64,
        result: *mut JettRuntimeResultV1,
    ) -> JettRuntimeStatusV1;
    fn jett_rt_v1_resource_test_entry_complete(
        context: *const JettRuntimeContextV1,
        attempt: u64,
        body: u32,
        panicked: u32,
        completion: *mut Completion,
        result: *mut JettRuntimeResultV1,
    ) -> JettRuntimeStatusV1;
    fn jett_rt_v1_resource_test_counts(
        context: *const JettRuntimeContextV1,
        counts: *mut Counts,
        result: *mut JettRuntimeResultV1,
    ) -> JettRuntimeStatusV1;
    fn jett_rt_v1_resource_test_events_copy(
        context: *const JettRuntimeContextV1,
        events: *mut Event,
        capacity: u64,
        count: *mut u64,
        result: *mut JettRuntimeResultV1,
    ) -> JettRuntimeStatusV1;
    fn jett_rt_v1_resource_test_failure_copy(
        context: *const JettRuntimeContextV1,
        attempt: u64,
        buffer: *mut u8,
        capacity: u64,
        length: *mut u64,
        result: *mut JettRuntimeResultV1,
    ) -> JettRuntimeStatusV1;
    fn jett_rt_v1_resource_test_script_remaining(
        context: *const JettRuntimeContextV1,
        remaining: *mut u64,
        result: *mut JettRuntimeResultV1,
    ) -> JettRuntimeStatusV1;
}

fn contained<T>(operation: impl FnOnce() -> T) -> Result<T, ()> {
    catch_unwind(AssertUnwindSafe(operation)).map_err(crate::discard_panic_payload)
}
fn ok(status: JettRuntimeStatusV1) -> Result<(), ()> {
    if status == JettRuntimeStatusV1::OK {
        Ok(())
    } else {
        Err(())
    }
}

struct ActiveAttempt {
    id: u64,
    index: usize,
    body_status: Option<u32>,
    panicked: u32,
    completion_attempted: bool,
}
struct Session {
    context: Box<MaybeUninit<JettRuntimeContextV1>>,
    created: bool,
    destroy_attempts: u32,
    destroy_status: u32,
    active: Option<ActiveAttempt>,
}
impl Session {
    fn new() -> Self {
        Self {
            context: Box::new(MaybeUninit::uninit()),
            created: false,
            destroy_attempts: 0,
            destroy_status: 0,
            active: None,
        }
    }
    fn pointer(&mut self) -> *mut JettRuntimeContextV1 {
        self.context.as_mut_ptr()
    }
    fn create(&mut self) -> Result<(), ()> {
        let mut result = MaybeUninit::uninit();
        // Distinct aligned outputs; the Box keeps the context stationary until destroy.
        ok(unsafe {
            jett_rt_v1_context_create(
                JETT_RUNTIME_ABI_VERSION_V1,
                self.pointer(),
                result.as_mut_ptr(),
            )
        })?;
        self.created = true;
        Ok(())
    }
    fn destroy_once(&mut self) {
        if !self.created || self.destroy_attempts != 0 {
            return;
        }
        self.destroy_attempts = 1;
        let mut result = MaybeUninit::uninit();
        self.destroy_status = match contained(|| unsafe {
            jett_rt_v1_context_destroy(self.pointer(), result.as_mut_ptr())
        }) {
            Ok(status) => status.code(),
            Err(()) => JettRuntimeStatusV1::PANIC.code(),
        };
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.destroy_once();
    }
}

fn script() -> Result<(Vec<u8>, usize), ()> {
    let encoded = std::env::var("JETT_RESOURCE_NATIVE_TEST_SCRIPT_V1").map_err(|_| ())?;
    if encoded.len() > MAX_SCRIPT * 2 || encoded.len() % 2 != 0 {
        return Err(());
    }
    let mut decoded = Vec::new();
    decoded
        .try_reserve_exact(encoded.len() / 2)
        .map_err(|_| ())?;
    fn nibble(value: u8) -> Result<u8, ()> {
        match value {
            b'0'..=b'9' => Ok(value - b'0'),
            b'a'..=b'f' => Ok(value - b'a' + 10),
            b'A'..=b'F' => Ok(value - b'A' + 10),
            _ => Err(()),
        }
    }
    for pair in encoded.as_bytes().chunks_exact(2) {
        decoded.push((nibble(pair[0])? << 4) | nibble(pair[1])?);
    }
    if decoded.len() < 32
        || &decoded[..8] != b"JTRST001"
        || decoded[8..12] != 1u32.to_le_bytes()
        || decoded[12..16] != [0; 4]
        || u64::from_le_bytes(decoded[16..24].try_into().map_err(|_| ())?) != decoded.len() as u64
    {
        return Err(());
    }
    let attempts = u32::from_le_bytes(decoded[24..28].try_into().map_err(|_| ())?) as usize;
    let operations = u32::from_le_bytes(decoded[28..32].try_into().map_err(|_| ())?);
    if !(1..=report::MAX_ATTEMPTS).contains(&attempts) || operations > 4096 {
        return Err(());
    }
    // The sole runtime decoder validates every closed operation before installation.
    Ok((decoded, attempts))
}

fn install(session: &mut Session, script: &[u8]) -> Result<(), ()> {
    let mut result = MaybeUninit::uninit();
    let mut info = MaybeUninit::uninit();
    ok(unsafe { jett_rt_v1_resource_test_archive_info(info.as_mut_ptr(), result.as_mut_ptr()) })?;
    let info = unsafe { info.assume_init() };
    if info.runtime_abi != JETT_RUNTIME_ABI_VERSION_V1
        || info.protocol != 1
        || info.wire != crate::resource_custody::NATIVE_RESOURCE_LAYOUT_WIRE_VERSION
        || info.pointer_bits != 64
        || info.profile != u32::from(!cfg!(debug_assertions))
        || info.reserved != [0; 3]
    {
        return Err(());
    }
    let mut manifest = MaybeUninit::uninit();
    if unsafe { jett_aot_resource_v1_manifest(manifest.as_mut_ptr()) } != 0 {
        return Err(());
    }
    let manifest = unsafe { manifest.assume_init() };
    if manifest.reserved != 0
        || manifest.layout.is_null()
        || manifest.length == 0
        || manifest.length > isize::MAX as u64
    {
        return Err(());
    }
    ok(unsafe {
        jett_rt_v1_resource_test_install(
            session.pointer(),
            &manifest,
            script.as_ptr(),
            script.len() as u64,
            result.as_mut_ptr(),
        )
    })
}

type CopyMessage = unsafe extern "C" fn(
    *const JettRuntimeContextV1,
    *mut u8,
    u64,
    *mut u64,
    *mut JettRuntimeResultV1,
) -> JettRuntimeStatusV1;
fn message(context: *const JettRuntimeContextV1, copy: CopyMessage) -> Result<Vec<u8>, ()> {
    let mut result = MaybeUninit::uninit();
    let mut length = 0;
    ok(unsafe {
        copy(
            context,
            ptr::null_mut(),
            0,
            &mut length,
            result.as_mut_ptr(),
        )
    })?;
    let required = usize::try_from(length).map_err(|_| ())?;
    if required > report::MAX_MESSAGE {
        return Err(());
    }
    let mut output = Vec::new();
    output.try_reserve_exact(required).map_err(|_| ())?;
    output.resize(required, 0);
    let mut copied = 0;
    ok(unsafe {
        copy(
            context,
            output.as_mut_ptr(),
            length,
            &mut copied,
            result.as_mut_ptr(),
        )
    })?;
    if copied != length {
        return Err(());
    }
    Ok(output)
}
fn resource_message(context: *const JettRuntimeContextV1, attempt: u64) -> Result<Vec<u8>, ()> {
    let mut result = MaybeUninit::uninit();
    let mut length = 0;
    ok(unsafe {
        jett_rt_v1_resource_test_failure_copy(
            context,
            attempt,
            ptr::null_mut(),
            0,
            &mut length,
            result.as_mut_ptr(),
        )
    })?;
    let required = usize::try_from(length).map_err(|_| ())?;
    if required > report::MAX_MESSAGE {
        return Err(());
    }
    let mut output = Vec::new();
    output.try_reserve_exact(required).map_err(|_| ())?;
    output.resize(required, 0);
    let mut copied = 0;
    ok(unsafe {
        jett_rt_v1_resource_test_failure_copy(
            context,
            attempt,
            output.as_mut_ptr(),
            length,
            &mut copied,
            result.as_mut_ptr(),
        )
    })?;
    if copied != length {
        return Err(());
    }
    Ok(output)
}
fn events(context: *const JettRuntimeContextV1) -> Result<Vec<Event>, ()> {
    let mut result = MaybeUninit::uninit();
    let mut count = 0;
    ok(unsafe {
        jett_rt_v1_resource_test_events_copy(
            context,
            ptr::null_mut(),
            0,
            &mut count,
            result.as_mut_ptr(),
        )
    })?;
    let required = usize::try_from(count).map_err(|_| ())?;
    if required > report::MAX_EVENTS {
        return Err(());
    }
    let mut output = Vec::new();
    output.try_reserve_exact(required).map_err(|_| ())?;
    output.resize(
        required,
        Event {
            sequence: 0,
            label: 0,
            kind: 0,
            reserved: 0,
        },
    );
    let mut copied = 0;
    ok(unsafe {
        jett_rt_v1_resource_test_events_copy(
            context,
            output.as_mut_ptr(),
            count,
            &mut copied,
            result.as_mut_ptr(),
        )
    })?;
    if copied != count {
        return Err(());
    }
    let mut previous = 0;
    for event in &output {
        if event.sequence <= previous || event.reserved != 0 || !(1..=5).contains(&event.kind) {
            return Err(());
        }
        previous = event.sequence;
    }
    Ok(output)
}

// Every query is independent. One refused copy must not suppress later
// pre-destroy observations or erase a prefix already collected.
fn counts(context: *const JettRuntimeContextV1) -> Result<Counts, ()> {
    let mut result = MaybeUninit::uninit();
    let mut output = MaybeUninit::uninit();
    ok(unsafe {
        jett_rt_v1_resource_test_counts(context, output.as_mut_ptr(), result.as_mut_ptr())
    })?;
    Ok(unsafe { output.assume_init() })
}
fn remaining(context: *const JettRuntimeContextV1) -> Result<u64, ()> {
    let mut result = MaybeUninit::uninit();
    let mut output = 0;
    ok(unsafe {
        jett_rt_v1_resource_test_script_remaining(context, &mut output, result.as_mut_ptr())
    })?;
    Ok(output)
}
fn ordinary_status(context: *const JettRuntimeContextV1) -> Result<u32, ()> {
    let mut output = MaybeUninit::uninit();
    // This read returns the current first failure without taking or resetting it.
    let status = unsafe {
        crate::native_abi::values::jett_rt_v1_value_failure(context, output.as_mut_ptr())
    };
    let output = unsafe { output.assume_init() };
    if output.status != status {
        return Err(());
    }
    Ok(status.code())
}
fn query<T>(operation: impl FnOnce() -> Result<T, ()>) -> Result<T, ()> {
    contained(operation)?
}
fn complete_active(session: &mut Session, report: &mut Report) {
    let Some(active) = session.active.as_mut() else {
        return;
    };
    if active.completion_attempted {
        return;
    }
    active.completion_attempted = true;
    let id = active.id;
    let index = active.index;
    let body = active
        .body_status
        .unwrap_or(JettRuntimeStatusV1::PANIC.code());
    let panicked = if active.body_status.is_some() {
        active.panicked
    } else {
        1
    };
    // An orchestration abort before an observed entry result still requires
    // cleanup, but cannot produce an accepted v1 body-status observation.
    if active.body_status.is_none() {
        report.refused = true;
    }
    let completion = query(|| {
        let mut result = MaybeUninit::uninit();
        let mut output = MaybeUninit::uninit();
        ok(unsafe {
            jett_rt_v1_resource_test_entry_complete(
                session.pointer(),
                id,
                body,
                panicked,
                output.as_mut_ptr(),
                result.as_mut_ptr(),
            )
        })?;
        Ok(unsafe { output.assume_init() })
    });
    match completion {
        Ok(value) => {
            // Preserve the completed header before any fallible later copy.
            let valid = value.attempt == id
                && value.body_status == body
                && value.reserved == 0
                && value.selected_kind <= 3;
            report.attempts[index].completion = Some(value);
            if !valid {
                report.refused = true;
            }
        }
        Err(()) => report.refused = true,
    }
    // A refused completion is never retried. Destroy's safety cleanup remains
    // the final fallback and its pre-destroy counts must expose any live state.
    session.active = None;
}
fn observe_attempt(session: &mut Session, report: &mut Report, index: usize) {
    let context = session.pointer();
    let id = report.attempts[index].id;
    match query(|| counts(context)) {
        Ok(value) => report.attempts[index].counts = Some(value),
        Err(()) => report.refused = true,
    }
    match query(|| ordinary_status(context)) {
        Ok(value) => report.attempts[index].ordinary_status = Some(value),
        Err(()) => report.refused = true,
    }
    match query(|| {
        message(
            context,
            crate::native_abi::values::jett_rt_v1_value_failure_copy,
        )
    }) {
        Ok(value) => report.attempts[index].ordinary_message = Some(value),
        Err(()) => report.refused = true,
    }
    match query(|| resource_message(context, id)) {
        Ok(value) => report.attempts[index].resource_message = Some(value),
        Err(()) => report.refused = true,
    }
    match query(|| events(context)) {
        Ok(value) => {
            report.attempts[index].event_sequence_end =
                Some(value.last().map_or(0, |event| event.sequence));
            report.events = Some(value);
        }
        Err(()) => report.refused = true,
    }
    match query(|| remaining(context)) {
        Ok(value) => report.remaining = Some(value),
        Err(()) => report.refused = true,
    }
}
fn observe_final(session: &mut Session, report: &mut Report) {
    if !session.created {
        return;
    }
    if contained(|| complete_active(session, report)).is_err() {
        report.refused = true;
    }
    if let Some(index) = report.attempts.len().checked_sub(1) {
        if !report.attempts[index].observations_complete()
            && contained(|| observe_attempt(session, report, index)).is_err()
        {
            report.refused = true;
        }
    }
    let context = session.pointer();
    match query(|| counts(context)) {
        Ok(value) => report.final_counts = Some(value),
        Err(()) => report.refused = true,
    }
    match query(|| ordinary_status(context)) {
        Ok(value) => report.final_ordinary_status = Some(value),
        Err(()) => report.refused = true,
    }
    match query(|| {
        message(
            context,
            crate::native_abi::values::jett_rt_v1_value_failure_copy,
        )
    }) {
        Ok(value) => report.final_ordinary_message = Some(value),
        Err(()) => report.refused = true,
    }
    match query(|| events(context)) {
        Ok(value) => report.events = Some(value),
        Err(()) => report.refused = true,
    }
    match query(|| remaining(context)) {
        Ok(value) => report.remaining = Some(value),
        Err(()) => report.refused = true,
    }
}
struct AttemptOutcome {
    exit: c_int,
    reenter: bool,
}
fn run_attempt(session: &mut Session, report: &mut Report) -> Result<AttemptOutcome, ()> {
    report.attempts.try_reserve(1).map_err(|_| ())?;
    let mut result = MaybeUninit::uninit();
    let mut attempt = 0;
    ok(unsafe {
        jett_rt_v1_resource_test_entry_begin(session.pointer(), &mut attempt, result.as_mut_ptr())
    })?;
    if attempt == 0 {
        return Err(());
    }
    let index = report.attempts.len();
    report.attempts.push(Attempt::pending(attempt));
    session.active = Some(ActiveAttempt {
        id: attempt,
        index,
        body_status: None,
        panicked: 0,
        completion_attempted: false,
    });
    let (body_status, panicked) =
        match contained(|| unsafe { jett_aot_v1_entry(session.pointer().cast()) }) {
            Ok(status) => (status, 0),
            Err(()) => (JettRuntimeStatusV1::PANIC.code(), 1),
        };
    report.attempts[index].observed_body_status = Some(body_status);
    let active = session.active.as_mut().ok_or(())?;
    active.body_status = Some(body_status);
    active.panicked = panicked;
    // Completion follows the entry immediately, including a caught entry panic.
    complete_active(session, report);
    observe_attempt(session, report, index);
    if report.refused {
        return Err(());
    }
    let observed = &report.attempts[index];
    let completion = observed.completion.as_ref().ok_or(())?;
    let counts = observed.counts.as_ref().ok_or(())?;
    let ordinary = observed.ordinary_status.ok_or(())?;
    if !counts.clean() {
        return Err(());
    }
    let exit = match completion.selected_kind {
        0 => 0,
        1 | 3 => EXIT_ENTRY,
        2 => EXIT_PANIC,
        _ => return Err(()),
    };
    // A completed Resource cleanup failure remains an immutable observation.
    // Clean retirement permits the declared next entry; entry_begin independently
    // rejects ordinary failures, ordinary cleanup failure and residual custody.
    // No reset or failure-taking API is invoked by the launcher.
    Ok(AttemptOutcome {
        exit,
        reenter: ordinary == 0,
    })
}
fn run(session: &mut Session, report: &mut Report) -> c_int {
    let (script, attempts) = match script() {
        Ok(value) => value,
        Err(()) => {
            report.refused = true;
            return EXIT_INFRASTRUCTURE;
        }
    };
    if session.create().is_err() {
        report.refused = true;
        return EXIT_CREATE;
    }
    if install(session, &script).is_err() {
        report.refused = true;
        return EXIT_INFRASTRUCTURE;
    }
    let mut exit = 0;
    for _ in 0..attempts {
        match run_attempt(session, report) {
            Ok(outcome) => {
                if exit == 0 {
                    exit = outcome.exit;
                }
                if !outcome.reenter {
                    break;
                }
            }
            Err(()) => {
                report.refused = true;
                return EXIT_INFRASTRUCTURE;
            }
        }
    }
    // The final observation pass also runs for every earlier return or panic.
    exit
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn main(_argc: c_int, _argv: *mut *mut c_char) -> c_int {
    // The file itself and its parent require both private cfgs. Ordinary runtime
    // and normal unit-test binaries retain Rust's existing panic hook.
    let outer = contained(|| {
        // Do not leave an earlier successful report after a refused current run.
        match std::fs::remove_file("resource-native-report-v1.bin") {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return EXIT_INFRASTRUCTURE,
        }
        std::panic::set_hook(Box::new(|_| {}));
        let mut session = Session::new();
        let mut report = Report::default();
        let mut exit = match contained(|| run(&mut session, &mut report)) {
            Ok(exit) => exit,
            Err(()) => {
                report.refused = true;
                EXIT_INFRASTRUCTURE
            }
        };
        // Observations precede the sole teardown even after orchestration panic.
        if contained(|| observe_final(&mut session, &mut report)).is_err() {
            report.refused = true;
            exit = EXIT_INFRASTRUCTURE;
        }
        if report.refused {
            exit = EXIT_INFRASTRUCTURE;
        }
        if exit == 0
            && (report.remaining != Some(0)
                || !report.final_counts.as_ref().is_some_and(Counts::clean))
        {
            report.refused = true;
            exit = EXIT_INFRASTRUCTURE;
        }
        session.destroy_once();
        report.created = session.created;
        report.destroy_attempts = session.destroy_attempts;
        report.destroy_status = session.destroy_status;
        if session.created && session.destroy_status != 0 {
            exit = EXIT_DESTROY;
        }
        // v1 cannot encode unknown counts/messages/sequence/remaining. Preserve
        // the internal completed prefix, refuse the report, and exit nonzero.
        let bytes = match report.encode() {
            Ok(bytes) => bytes,
            Err(()) => {
                return if exit == EXIT_DESTROY {
                    exit
                } else {
                    EXIT_INFRASTRUCTURE
                };
            }
        };
        if std::fs::write("resource-native-report-v1.bin", bytes).is_ok() {
            return exit;
        }
        report.write_status = JettRuntimeStatusV1::IO_FAILURE.code();
        if let Ok(bytes) = report.encode() {
            let _ = std::fs::write("resource-native-report-v1.bin", bytes);
        }
        EXIT_INFRASTRUCTURE
    });
    outer.unwrap_or(EXIT_INFRASTRUCTURE)
}
