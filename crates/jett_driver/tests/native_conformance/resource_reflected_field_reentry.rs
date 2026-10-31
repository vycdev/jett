//! Original Source06 first script followed by READY on one retained Session.
//! Every completion remains observable; exceptional reentry is never exit zero.
use super::{archive, cases, fs, link, object, reflected_field_cases, report, script};

fn original_cases() -> impl Iterator<Item = &'static cases::Case> {
    std::iter::once(&reflected_field_cases::CASES[0])
        .chain(reflected_field_cases::SOURCE06_EXCEPTIONAL_CASES.iter())
}

fn script_bytes(case: &cases::Case) -> Vec<u8> {
    let encoded = script(case);
    let bytes = encoded
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(&bytes[..8], b"JTRST001");
    assert_eq!(u32::from_le_bytes(bytes[24..28].try_into().unwrap()), 1);
    assert_eq!(
        u32::from_le_bytes(bytes[28..32].try_into().unwrap()) as usize,
        case.script.len()
    );
    assert_eq!(
        u64::from_le_bytes(bytes[16..24].try_into().unwrap()) as usize,
        bytes.len()
    );
    bytes
}

fn original_then_ready_script(first: &cases::Case, ready: &cases::Case) -> String {
    assert_eq!(first.source.as_bytes(), ready.source.as_bytes());
    assert_eq!(
        (ready.body, ready.cleanup, ready.channel, ready.exit),
        (0, 0, 0, 0)
    );
    let mut bytes = script_bytes(first);
    let ready_bytes = script_bytes(ready);
    assert_eq!(&bytes[..16], &ready_bytes[..16]);
    bytes.extend_from_slice(&ready_bytes[32..]);
    bytes[24..28].copy_from_slice(&2u32.to_le_bytes());
    let operations = u32::try_from(first.script.len() + ready.script.len()).unwrap();
    bytes[28..32].copy_from_slice(&operations.to_le_bytes());
    let length = u64::try_from(bytes.len()).unwrap();
    bytes[16..24].copy_from_slice(&length.to_le_bytes());
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn channel(expected: u32) -> report::FailureChannel {
    match expected {
        0 => report::FailureChannel::Clean,
        1 => report::FailureChannel::Body,
        2 => report::FailureChannel::HostPanic,
        3 => report::FailureChannel::Cleanup,
        _ => panic!("invalid native completion oracle"),
    }
}

fn assert_completion(case: &cases::Case, attempt: &report::Attempt, end: usize) {
    assert_eq!(
        (
            attempt.completion.body_status,
            attempt.completion.cleanup_status,
            attempt.completion.selected_channel,
        ),
        (case.body, case.cleanup, channel(case.channel)),
        "{} retained completion: {attempt:?}",
        case.name
    );
    assert_eq!(attempt.ordinary_message, case.ordinary_message);
    assert_eq!(attempt.resource_message, case.resource_message);
    assert!(
        attempt.counts.resources_retired() && attempt.counts.ordinary_empty,
        "{} BEFORE teardown: {attempt:?}",
        case.name
    );
    assert_eq!(attempt.event_sequence_end, u64::try_from(end).unwrap());
}

fn assert_original_then_ready(first: &cases::Case, ready: &cases::Case, observed: &report::Report) {
    assert_eq!(observed.attempts.len(), 2, "{}: {observed:?}", first.name);
    let first_id = observed.attempts[0].completion.attempt;
    let second_id = observed.attempts[1].completion.attempt;
    assert!(first_id != 0 && second_id > first_id);
    assert_completion(first, &observed.attempts[0], first.events.len());
    assert_completion(
        ready,
        &observed.attempts[1],
        first.events.len() + ready.events.len(),
    );
    assert_eq!(
        observed.events.len(),
        first.events.len() + ready.events.len()
    );
    for (index, (actual, expected)) in observed
        .events
        .iter()
        .zip(first.events.iter().chain(ready.events.iter()))
        .enumerate()
    {
        let (label, kind) = super::super::event(*expected);
        assert_eq!(
            (actual.sequence, actual.label, actual.kind),
            ((index + 1) as u64, label, kind),
            "{} event {index}",
            first.name
        );
    }
    assert_eq!(observed.script_remaining, 0);
    assert_eq!(
        (
            observed.created,
            observed.destroy_attempts,
            observed.destroy_status,
            observed.write_status,
        ),
        (true, 1, 0, 0)
    );
    assert!(observed.observations_ready_for_acceptance());
    assert_eq!(
        observed.all_attempts_succeeded(),
        first.body == 0 && first.cleanup == 0 && first.channel == 0
    );
}

pub(super) fn run_original_then_ready(first: &cases::Case, ready: &cases::Case, release: bool) {
    let archive = archive(release);
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let project = root.join("project");
    let stdlib = root.join("synthetic-stdlib");
    fs::create_dir(&project).unwrap();
    fs::create_dir(&stdlib).unwrap();
    let entry = project.join("main.jett");
    let support = stdlib.join("resource_probe.jett");
    fs::write(&entry, first.source).unwrap();
    fs::write(&support, cases::SUPPORT).unwrap();
    let artifact = object(&entry, &support, release);
    assert_eq!(artifact.target, archive.target);
    assert!(
        artifact
            .symbols
            .iter()
            .any(|name| name == jett_codegen_cranelift::JETT_AOT_ENTRY_SYMBOL_V1)
    );
    assert!(
        artifact
            .symbols
            .iter()
            .any(|name| name == "jett_aot_resource_v1_manifest")
    );
    let object_path = root.join("resource.obj");
    let binary = root.join(if cfg!(windows) {
        "resource.exe"
    } else {
        "resource"
    });
    fs::write(&object_path, artifact.bytes).unwrap();
    link(&archive, &object_path, &binary, root);
    fs::remove_dir_all(&project).unwrap();
    fs::remove_dir_all(&stdlib).unwrap();
    assert!(!entry.exists() && !support.exists() && !project.exists() && !stdlib.exists());
    let encoded = original_then_ready_script(first, ready);
    let actual = super::super::super::run_bounded_with_env(
        &binary,
        root,
        Some(("JETT_RESOURCE_NATIVE_TEST_SCRIPT_V1", &encoded)),
    );
    let path = root.join("resource-native-report-v1.bin");
    assert!(path.metadata().unwrap().len() <= 16 * 1024 * 1024);
    let observed = report::decode(&fs::read(path).unwrap()).unwrap();
    assert_eq!(
        actual.status.code(),
        Some(first.exit),
        "{} {} retained first exit: {actual:?}; {observed:?}",
        first.name,
        archive.profile
    );
    assert!(
        actual.stdout.is_empty() && actual.stderr.is_empty(),
        "{actual:?}"
    );
    assert_original_then_ready(first, ready, &observed);
}

#[test]
#[ignore = "requires fresh independently measured reentry-capable Debug/Release single-runtime archives"]
fn native_resource_reflected_field_source06_all_scripts_reenter_with_same_provider_grant() {
    let ready = &reflected_field_cases::CASES[0];
    assert_eq!(ready.name, "reflected_field_original");
    assert_eq!(reflected_field_cases::SOURCE06_EXCEPTIONAL_CASES.len(), 8);
    assert_eq!(original_cases().count(), 9);
    for release in [false, true] {
        for first in original_cases() {
            run_original_then_ready(first, ready, release);
        }
    }
    eprintln!(
        "Resource reflected Source06 same-grant reentry acceptance: 9 original scripts then READY; 18 Source-deleted executions; 36 entries; profiles=debug,release; retained original completions and first nonzero exits"
    );
}
