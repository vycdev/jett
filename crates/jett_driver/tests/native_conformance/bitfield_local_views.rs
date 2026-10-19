use super::{run_bounded, suite_options::launcher_for_options};
use jett_driver::native::build_host_executable_with_options;
use jett_driver::{BuildOptions, run_file_capture_outcome};
use std::fs;

#[derive(Clone, Copy)]
enum BitfieldOutcome<'a> {
    Success(&'a str),
    RuntimeFailure { stdout: &'a str, message: &'a str },
}

fn run_case(name: &str, source_text: &str, expected_stdout: &str) {
    run_case_with_traces(
        name,
        source_text,
        BitfieldOutcome::Success(expected_stdout),
        &[],
    );
}

fn run_case_with_traces(
    name: &str,
    source_text: &str,
    expected: BitfieldOutcome<'_>,
    expected_traces: &[&str],
) {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join(format!("{name}.jett"));
    fs::write(&source, source_text).unwrap();
    let (reference, status, terminal_stderr) = match expected {
        BitfieldOutcome::Success(stdout) => {
            let output = run_file_capture_outcome(&source)
                .unwrap_or_else(|error| panic!("{name}: checked reference fixture: {error:?}"));
            assert_eq!(output.stdout, stdout, "{name}");
            (output, 0, String::new())
        }
        BitfieldOutcome::RuntimeFailure { stdout, message } => {
            let failure = run_file_capture_outcome(&source)
                .expect_err("expected terminal bitfield receiver fixture failure");
            assert_eq!(failure.output.stdout, stdout, "{name}");
            assert_eq!(failure.message, message, "{name}");
            (failure.output, 71, format!("{message}\n"))
        }
    };
    let expected_events = expected_traces
        .iter()
        .map(|text| jett_driver::DebugEvent {
            kind: jett_driver::DebugEventKind::Trace,
            text: (*text).to_owned(),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        reference.debug_events, expected_events,
        "{name}: {reference:?}"
    );
    assert!(
        reference.frontend_debug_observations.is_empty(),
        "{name}: {reference:?}"
    );
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory.path().join(format!("{name}_{release}.exe"));
        let artifact = build_host_executable_with_options(
            &source,
            launcher_for_options(release),
            &binary,
            BuildOptions { release },
        )
        .unwrap_or_else(|error| panic!("{name}, release={release}: native build: {error}"));
        assert!(
            artifact.debug_observations.is_empty(),
            "{name}: {artifact:?}"
        );
        binaries.push((binary, release));
    }
    fs::remove_file(&source).unwrap();
    assert!(!source.exists());
    for (binary, release) in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert_eq!(
            actual.status.code(),
            Some(status),
            "{name}, release={release}: {actual:?}"
        );
        assert_eq!(
            actual.stdout,
            reference.stdout.as_bytes(),
            "{name}, release={release}"
        );
        let debug = if release {
            String::new()
        } else {
            jett_driver::render_debug_events(&reference.debug_events)
        };
        let stderr = format!("{debug}{terminal_stderr}");
        assert_eq!(
            actual.stderr,
            stderr.as_bytes(),
            "{name}, release={release}: {actual:?}"
        );
    }
}

#[test]
fn native_bitfield_local_views_bitfield_payload_local_view_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "bitfield_payload_local_view",
        include_str!("bitfield_local_views/bitfield_payload_local_view.jett"),
        "2:3:2:4007ff",
    );
}

#[test]
fn native_bitfield_local_views_bitfield_payload_owned_control_matches_reference_without_source_in_both_profiles()
 {
    run_case(
        "bitfield_payload_owned_control",
        include_str!("bitfield_local_views/bitfield_payload_owned_control.jett"),
        "3:2:4007ff",
    );
}

#[test]
fn native_bitfield_local_views_view_parameter_payload_keeps_original_owner_without_source_in_both_profiles()
 {
    run_case(
        "view_parameter_payload",
        include_str!("bitfield_local_views/01_view_parameter.jett"),
        "view:2:2:3:2:4007ff\nowner:4:2\n",
    );
}

#[test]
fn native_bitfield_local_views_envelope_path_keeps_alias_and_owner_reads_without_source_in_both_profiles()
 {
    run_case(
        "envelope_path",
        include_str!("bitfield_local_views/02_envelope_path.jett"),
        "path:2:2:2:4:23:4007ff\nowner:4:2:23\n",
    );
}

#[test]
fn native_bitfield_local_views_empty_payload_keeps_independent_clone_without_source_in_both_profiles()
 {
    run_case(
        "empty_payload",
        include_str!("bitfield_local_views/03_empty_payload.jett"),
        "empty:0:0:1:0:00\nowner:0:0\n",
    );
}

#[test]
fn native_bitfield_local_views_01_pending_whole_packet_matches_reference_without_source_in_both_profiles()
 {
    run_case_with_traces(
        "01_pending_whole_packet",
        include_str!("bitfield_local_views/01_pending_whole_packet.jett"),
        BitfieldOutcome::RuntimeFailure {
            stdout: "before:whole\n",
            message: "runtime error: field access is not supported on pending(pending(app.Packet(version: 4, flags: 0, payload: list(7, 255))))",
        },
        &[],
    );
}

#[test]
fn native_bitfield_local_views_02_pending_payload_copies_matches_reference_without_source_in_both_profiles()
 {
    run_case_with_traces(
        "02_pending_payload_copies",
        include_str!("bitfield_local_views/02_pending_payload_copies.jett"),
        BitfieldOutcome::Success("before:payload\nitems:3:2\n"),
        &[
            "trace forwarded: list[uint8] = pending(pending(list(7, 255)))\n",
            "trace once: list[uint8] = pending(list(7, 255))\n",
            "trace forwarded: list[uint8] = pending(pending(list(7, 255)))\n",
            "trace once: list[uint8] = pending(list(7, 255))\n",
            "trace packet: app.Packet = app.Packet(version: 4, flags: 0, payload: pending(pending(list(7, 255))))\n",
        ],
    );
}
