//! Four original checked Sources through strict final emit and matched native execution.
use super::{
    archive, cases, fs, link, object, report, run_native_cases, run_object_preflight, script,
};

#[path = "resource_reflected_field_cases.rs"]
mod reflected_field_cases;

#[test]
fn native_resource_reflected_field_four_sources_emit_in_both_profiles() {
    assert_eq!(reflected_field_cases::CASES.len(), 4);
    run_object_preflight(reflected_field_cases::CASES, "reflected-field");
}

#[test]
#[ignore = "requires exact Root-measured double-cfg Debug/Release single-runtime archive receipt"]
fn native_resource_reflected_field_four_sources_retire_before_teardown() {
    assert_eq!(reflected_field_cases::CASES.len(), 4);
    run_native_cases(reflected_field_cases::CASES);
    eprintln!(
        "Resource reflected-field native acceptance: 4 cases; 8 Source-deleted executions; profiles=debug,release"
    );
}

fn two_clean_entries(case: &cases::Case) -> String {
    assert_eq!(
        (case.body, case.cleanup, case.channel, case.exit),
        (0, 0, 0, 0)
    );
    let encoded = script(case);
    let mut bytes = encoded
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(&bytes[..8], b"JTRST001");
    assert_eq!(u32::from_le_bytes(bytes[24..28].try_into().unwrap()), 1);
    assert_eq!(case.script.len(), 3);
    assert_eq!(bytes.len(), 84);
    let operations = bytes[32..].to_vec();
    bytes.extend(operations);
    bytes[24..28].copy_from_slice(&2u32.to_le_bytes());
    bytes[28..32].copy_from_slice(&6u32.to_le_bytes());
    let length = bytes.len() as u64;
    bytes[16..24].copy_from_slice(&length.to_le_bytes());
    assert_eq!(bytes.len(), 136);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn assert_clean_reentry(case: &cases::Case, observed: &report::Report) {
    assert_eq!(observed.attempts.len(), 2, "{}: {observed:?}", case.name);
    let mut previous = 0;
    for (index, attempt) in observed.attempts.iter().enumerate() {
        assert!(
            attempt.completion.attempt > previous,
            "{} attempt identity",
            case.name
        );
        previous = attempt.completion.attempt;
        assert_eq!(
            (
                attempt.completion.body_status,
                attempt.completion.cleanup_status,
                attempt.completion.selected_channel
            ),
            (0, 0, report::FailureChannel::Clean),
            "{} entry {index}: {observed:?}",
            case.name
        );
        assert!(attempt.ordinary_message.is_empty() && attempt.resource_message.is_empty());
        assert!(
            attempt.counts.resources_retired() && attempt.counts.ordinary_empty,
            "{} entry {index} BEFORE teardown: {observed:?}",
            case.name
        );
        assert_eq!(
            attempt.event_sequence_end,
            ((index + 1) * case.events.len()) as u64
        );
    }
    assert_eq!(observed.events.len(), 2 * case.events.len());
    for (index, (actual, expected)) in observed
        .events
        .iter()
        .zip(case.events.iter().cycle().take(2 * case.events.len()))
        .enumerate()
    {
        let (label, kind) = super::event(*expected);
        assert_eq!(
            (actual.sequence, actual.label, actual.kind),
            ((index + 1) as u64, label, kind),
            "{} event {index}",
            case.name
        );
    }
    assert_eq!(observed.script_remaining, 0);
    assert_eq!(
        (
            observed.created,
            observed.destroy_attempts,
            observed.destroy_status,
            observed.write_status
        ),
        (true, 1, 0, 0)
    );
    assert!(observed.observations_ready_for_acceptance());
    assert!(observed.all_attempts_succeeded());
}

#[test]
#[ignore = "requires exact Root-measured double-cfg Debug/Release single-runtime archive receipt"]
fn native_resource_reflected_field_four_sources_clean_same_grant_reentry() {
    for release in [false, true] {
        let archive = archive(release);
        for case in reflected_field_cases::CASES {
            let directory = tempfile::tempdir().unwrap();
            let root = directory.path();
            let project = root.join("project");
            let stdlib = root.join("synthetic-stdlib");
            fs::create_dir(&project).unwrap();
            fs::create_dir(&stdlib).unwrap();
            let entry = project.join("main.jett");
            let support = stdlib.join("resource_probe.jett");
            fs::write(&entry, case.source).unwrap();
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
            let encoded = two_clean_entries(case);
            let actual = super::super::run_bounded_with_env(
                &binary,
                root,
                Some(("JETT_RESOURCE_NATIVE_TEST_SCRIPT_V1", &encoded)),
            );
            let path = root.join("resource-native-report-v1.bin");
            assert!(path.metadata().unwrap().len() <= 16 * 1024 * 1024);
            let observed = report::decode(&fs::read(path).unwrap()).unwrap();
            assert_eq!(
                actual.status.code(),
                Some(0),
                "{} {}: {actual:?}; {observed:?}",
                case.name,
                archive.profile
            );
            assert!(
                actual.stdout.is_empty() && actual.stderr.is_empty(),
                "{actual:?}"
            );
            assert_clean_reentry(case, &observed);
        }
    }
    eprintln!(
        "Resource reflected-field clean reentry acceptance: 4 cases; 8 Source-deleted executions; 16 entries on the same per-executable provider/grant; profiles=debug,release"
    );
}

#[test]
fn native_resource_reflected_field_source06_exceptional_scripts_emit_in_both_profiles() {
    assert_eq!(reflected_field_cases::SOURCE06_EXCEPTIONAL_CASES.len(), 8);
    run_object_preflight(
        reflected_field_cases::SOURCE06_EXCEPTIONAL_CASES,
        "reflected-field-source06-exceptional",
    );
}

#[test]
#[ignore = "requires exact Root-measured double-cfg Debug/Release single-runtime archive receipt"]
fn native_resource_reflected_field_source06_exceptional_scripts_retire_before_teardown() {
    assert_eq!(reflected_field_cases::SOURCE06_EXCEPTIONAL_CASES.len(), 8);
    run_native_cases(reflected_field_cases::SOURCE06_EXCEPTIONAL_CASES);
    eprintln!(
        "Resource reflected-field exceptional single-entry acceptance: 8 cases; 16 Source-deleted executions; profiles=debug,release; exceptional same-grant reentry remains separate"
    );
}

#[path = "resource_reflected_field_reentry.rs"]
mod source06_reentry;
