use super::*;

use super::suite_options::launcher_for_options;

#[derive(Clone, Copy)]
enum ProjectedOutcome {
    Success(&'static str),
    RuntimeFailure {
        stdout: &'static str,
        message: &'static str,
    },
}

fn run_case(name: &str, source_text: &str, expected: ProjectedOutcome) {
    run_case_with_traces(name, source_text, expected, &[]);
}

fn run_case_with_traces(
    name: &str,
    source_text: &str,
    expected: ProjectedOutcome,
    expected_traces: &[&str],
) {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join(format!("{name}.jett"));
    fs::write(&source, source_text).unwrap();
    let (reference, status, terminal_stderr) = match expected {
        ProjectedOutcome::Success(stdout) => {
            let output = jett_driver::run_file_capture_outcome(&source)
                .unwrap_or_else(|error| panic!("{name}: checked reference fixture: {error:?}"));
            assert_eq!(output.stdout, stdout, "{name}");
            (output, 0, String::new())
        }
        ProjectedOutcome::RuntimeFailure { stdout, message } => {
            let failure = jett_driver::run_file_capture_outcome(&source)
                .expect_err("expected terminal projected view fixture failure");
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
        let artifact = jett_driver::native::build_host_executable_with_options(
            &source,
            launcher_for_options(release),
            &binary,
            jett_driver::BuildOptions { release },
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
fn native_stable_projected_local_view_exact_packet_matches_reference_without_source() {
    run_case(
        "01_exact_packet",
        include_str!("local_view_aliases/01_exact_packet.jett"),
        ProjectedOutcome::Success(""),
    );
}

#[test]
fn native_stable_projected_local_view_owned_bytes_and_forwarding_preserve_original_owner() {
    run_case(
        "02_owned_bytes_forwarded",
        include_str!("local_view_aliases/02_owned_bytes_forwarded.jett"),
        ProjectedOutcome::Success("4142:4142:414243:4142\n"),
    );
}

#[test]
fn native_stable_projected_local_view_parameter_clone_preserves_callers_owner() {
    run_case(
        "03_view_parameter",
        include_str!("local_view_aliases/03_view_parameter.jett"),
        ProjectedOutcome::Success("4142:4142\n"),
    );
}

#[test]
fn native_stable_projected_local_view_nested_field_path_preserves_owner() {
    run_case(
        "04_nested_local",
        include_str!("local_view_aliases/04_nested_local.jett"),
        ProjectedOutcome::Success("4344:4344:4344\n"),
    );
}

#[test]
fn native_stable_projected_local_view_failure_preserves_argument_order_and_cleanup() {
    run_case(
        "05_projected_list_failure",
        include_str!("local_view_aliases/05_projected_list_failure.jett"),
        ProjectedOutcome::RuntimeFailure {
            stdout: "before\nborrowed:2\nfailure\n",
            message: "runtime error: list.__remove_at: index -1 out of bounds",
        },
    );
}

#[test]
fn native_stable_projected_local_view_direct_alias_control_remains_supported() {
    run_case(
        "06_direct_alias_control",
        include_str!("local_view_aliases/06_direct_alias_control.jett"),
        ProjectedOutcome::Success("4142:4142\n"),
    );
}

#[test]
fn native_stable_projected_local_view_owned_field_copy_remains_independent() {
    run_case(
        "07_owned_field_copy_control",
        include_str!("local_view_aliases/07_owned_field_copy_control.jett"),
        ProjectedOutcome::Success("414243:4142\n"),
    );
}

#[test]
fn native_stable_projected_local_view_struct_generic_and_qualified_endpoints_keep_checked_types() {
    run_case(
        "10_typed_endpoints",
        include_str!("local_view_aliases/10_typed_endpoints.jett"),
        ProjectedOutcome::Success(
            "nested:4142:4142:4142\ngeneric:7:7:4344:434445:4344\nqualified:2:2:3:2:2:3\n",
        ),
    );
}

#[test]
fn native_unstable_projected_local_view_aliases_reject_before_publication() {
    use jett_driver::native::{NativeBuildError, build_host_executable_with_options};
    use jett_driver::{BackendLoweringError, BuildOptions, build_file_with_options};

    let cases = [
        (
            "mutable_root",
            include_str!("local_view_aliases/08_mutable_root_boundary.jett"),
            "native borrowed alias requires immutable bindings along its stable origin",
        ),
        (
            "temporary_root",
            include_str!("local_view_aliases/09_temporary_root_boundary.jett"),
            "native borrowed alias requires a stable local origin; temporary views remain unsupported",
        ),
    ];
    for (name, source_text, expected_message) in cases {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join(format!("{name}.jett"));
        fs::write(&source, source_text).unwrap();
        let output = directory.path().join("preserved.exe");
        let sentinel = b"existing native publication";
        fs::write(&output, sentinel).unwrap();
        let launcher = if cfg!(windows) {
            NativeLauncherBundle::windows_msvc_static_v1(directory.path().join("unused.lib"))
        } else {
            NativeLauncherBundle::linux_gnu_v1(directory.path().join("unused.a"))
        };
        assert!(!launcher.archive_path.exists());

        for release in [false, true] {
            let options = BuildOptions { release };
            let checked = build_file_with_options(&source, options);
            assert!(
                !checked.has_errors,
                "{name}: excluded views remain frontend-valid: {:?}",
                checked.diagnostics
            );
            let error = build_host_executable_with_options(&source, &launcher, &output, options)
                .expect_err("unstable origin must fail before emission or archive lookup");
            let NativeBuildError::Lowering { source, .. } = error else {
                panic!("{name}: expected HIR admission failure: {error}");
            };
            let errors = match *source {
                BackendLoweringError::Hir(errors) => errors,
                error => panic!("{name}: expected HIR admission failure: {error}"),
            };
            assert_eq!(errors.len(), 1, "{name}: {errors:?}");
            assert_eq!(errors[0].message, expected_message, "{name}: {errors:?}");
            assert_eq!(fs::read(&output).unwrap(), sentinel);
        }
    }
}

#[test]
fn native_local_view_aliases_match_interpreter_in_both_profiles() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/native/local_view_aliases.jett");
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("main.jett");
    fs::copy(&fixture, &source).unwrap();
    let expected =
        jett_driver::run_file_capture_output(&source).expect("local view alias reference oracle");
    assert_eq!(
        expected.stdout,
        concat!(
            "lists:10:17:3:10:10:3\n",
            "bytes:4142:4142:2\n",
            "strings:secret:secret:secret:refined:refined:refined\n",
            "records:record:18:18:18\n",
            "interfaces:named:named:named\n",
            "functions:10:11:12:11:12\n",
            "generics:7:7:5:5\n",
            "sums:5:5:5\n",
            "handlers:3:4:secret:3:secret:4:refined:5\n",
            "collections:10:11:secret:owned:sum\n",
            "scopes:27:5\n",
            "reflection:values:app.Record;label:app.Record;reflection:7\n",
            "pending:18:18\n",
            "debug-root:10:10\n",
        )
    );
    assert_eq!(
        debug_trace_lines(&expected.debug_events),
        [
            "trace source: list[int64] = list(2, 3, 5)",
            "trace borrowed: list[int64] = list(2, 3, 5)",
            "trace forwarded: list[int64] = list(2, 3, 5)",
            "trace hidden_alias: secret[string] = [redacted]",
        ]
    );
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory
            .path()
            .join(format!("local_aliases_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .expect("native immutable local view aliases");
        binaries.push((binary, release));
    }
    let verify_binary = directory.path().join("local_aliases_verify.exe");
    build_host_verify_suite_executable(&source, launcher(), &verify_binary)
        .expect("compiled local view alias verify suite");
    let property_binary = directory.path().join("local_aliases_property.exe");
    build_host_property_suite_executable(&source, launcher(), &property_binary)
        .expect("compiled local view alias property suite");
    fs::remove_file(&source).unwrap();
    for (binary, release) in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes());
        let debug = if release {
            String::new()
        } else {
            jett_driver::render_debug_events(&expected.debug_events)
        };
        assert_eq!(actual.stderr, debug.as_bytes(), "{actual:?}");
    }
    for binary in [verify_binary, property_binary] {
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert!(actual.stdout.is_empty(), "{actual:?}");
        assert!(actual.stderr.is_empty(), "{actual:?}");
    }
}

#[test]
fn native_local_view_alias_failure_preserves_order_and_cleanup() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("main.jett");
    fs::write(
        &source,
        r#"namespace app
function first(view stdout: Stdout, view source: list[string]) returns int64:
    list[string] borrowed = view source
    list[string] forwarded = borrowed
    Stdout.write(view stdout, "borrowed:{list.length(view forwarded)}\n")
    return list.length(view source)
function failing(view stdout: Stdout) returns list[string]:
    Stdout.write(view stdout, "failure\n")
    return list.remove_at[string](list("owned", "other"), -1)
function consume(amount: int64, values: list[string]) returns nothing:
    return nothing
function main(stdout: Stdout) returns nothing:
    list[string] source = list("keep", "source")
    bytes payload = bytes.from_string("cleanup")
    list[string] borrowed = view source
    list[string] forwarded = borrowed
    Stdout.write(view stdout, "before\n")
    consume(first(view stdout, view forwarded), failing(view stdout))
    Stdout.write(view stdout, "must not run")
"#,
    )
    .unwrap();
    let expected = jett_driver::run_file_capture_outcome(&source)
        .expect_err("later owned argument fails after alias observation");
    assert_eq!(expected.output.stdout, "before\nborrowed:2\nfailure\n");
    assert!(expected.output.debug_events.is_empty());
    assert_eq!(
        expected.message,
        "runtime error: list.__remove_at: index -1 out of bounds"
    );
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory
            .path()
            .join(format!("alias_failure_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .expect("native alias borrow followed by terminal owned argument failure");
        binaries.push(binary);
    }
    fs::remove_file(&source).unwrap();
    for binary in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert_eq!(actual.status.code(), Some(71), "{actual:?}");
        assert_eq!(actual.stdout, expected.output.stdout.as_bytes());
        assert_eq!(
            actual.stderr,
            format!("{}\n", expected.message).as_bytes(),
            "{actual:?}"
        );
    }
}

#[test]
fn native_nonstring_qualified_local_aliases_preserve_owners_and_cleanup_in_both_profiles() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("main.jett");
    fs::write(
        &source,
        r#"namespace app
type Numbers = list[int64] where true
function peek(view values: list[int64], index: int64) returns int64:
    return list.get[int64](view values, index) handle: default -1
function refined_copy(view source: Numbers) returns list[int64]:
    list[int64] borrowed = coarsen view source
    list[int64] forwarded = borrowed
    return clone forwarded
function revealed_copy(view source: secret[list[int64]]) returns list[int64]:
    list[int64] borrowed = declassify view source
    list[int64] forwarded = borrowed
    return clone forwarded
function main(stdout: Stdout) returns nothing:
    list[string] earlier = list("owned", "cleanup")
    Numbers refined = list(2, 3) handle error:
        Stdout.write(view stdout, "unexpected:{error}\n")
        return nothing
    mutable list[int64] refined_clone = refined_copy(view refined)
    refined_clone = list.append[int64](refined_clone, 13)
    list[int64] coarse = coarsen view refined
    Stdout.write(view stdout, "coarsen:{list.length(view coarse)}:{list.length(view refined_clone)}:{peek(view coarse, 0)}:{peek(view coarse, 1)}:{peek(view refined_clone, 2)}\n")
    list[int64] public_values = list(5, 7)
    secret[list[int64]] hidden = public_values
    mutable list[int64] hidden_clone = revealed_copy(view hidden)
    hidden_clone = list.append[int64](hidden_clone, 17)
    list[int64] revealed = declassify view hidden
    Stdout.write(view stdout, "declassify:{list.length(view revealed)}:{list.length(view hidden_clone)}:{peek(view revealed, 0)}:{peek(view revealed, 1)}:{peek(view hidden_clone, 2)}\n")
    string title = list.get[string](view earlier, 0) handle: default "missing"
    Stdout.write(view stdout, "earlier:{title}:{list.length(view earlier)}\n")
    list[string] unreachable = list.remove_at[string](earlier, -1)
    Stdout.write(view stdout, "must not run")
"#,
    )
    .unwrap();
    let expected = jett_driver::run_file_capture_outcome(&source)
        .expect_err("terminal list failure follows qualified aliases and independent clones");
    assert_eq!(
        expected.output.stdout,
        "coarsen:2:3:2:3:13\ndeclassify:2:3:5:7:17\nearlier:owned:2\n"
    );
    assert!(expected.output.debug_events.is_empty());
    assert_eq!(
        expected.message,
        "runtime error: list.__remove_at: index -1 out of bounds"
    );
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory
            .path()
            .join(format!("qualified_alias_cleanup_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .expect("native nonstring coarsened and declassified local view aliases");
        binaries.push(binary);
    }
    fs::remove_file(&source).unwrap();
    for binary in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert_eq!(actual.status.code(), Some(71), "{actual:?}");
        assert_eq!(actual.stdout, expected.output.stdout.as_bytes());
        assert_eq!(
            actual.stderr,
            format!("{}\n", expected.message).as_bytes(),
            "{actual:?}"
        );
    }
}

#[test]
fn native_machine_projected_local_view_exact_packet_matches_reference_without_source() {
    run_case(
        "machine_01_exact_packet",
        include_str!("local_view_aliases/machines/01_exact_packet.jett"),
        ProjectedOutcome::Success(""),
    );
}

#[test]
fn native_machine_projected_local_view_payloads_and_forwarding_preserve_original_owner() {
    run_case(
        "machine_02_owned_payloads_forwarded",
        include_str!("local_view_aliases/machines/02_owned_payloads_forwarded.jett"),
        ProjectedOutcome::Success("bytes:4142:4142:414243:4142\nitems:2:2:3:2\n"),
    );
}

#[test]
fn native_machine_projected_local_view_parameter_clones_preserve_callers_owner() {
    run_case(
        "machine_03_view_parameter",
        include_str!("local_view_aliases/machines/03_view_parameter.jett"),
        ProjectedOutcome::Success("bytes:414243:4142\nitems:3:2\n"),
    );
}

#[test]
fn native_machine_projected_local_view_struct_to_machine_path_preserves_owner() {
    run_case(
        "machine_04_struct_to_machine",
        include_str!("local_view_aliases/machines/04_struct_to_machine.jett"),
        ProjectedOutcome::Success("bytes:4344:4344:4344\nitems:2:2:2\n"),
    );
}

#[test]
fn native_machine_projected_local_view_machine_to_struct_path_preserves_owner() {
    run_case(
        "machine_05_machine_to_struct",
        include_str!("local_view_aliases/machines/05_machine_to_struct.jett"),
        ProjectedOutcome::Success("bytes:4546:4546:4546\nitems:2:2:2\n"),
    );
}

#[test]
fn native_machine_projected_local_view_failure_preserves_argument_order_and_cleanup() {
    run_case(
        "machine_06_later_argument_failure",
        include_str!("local_view_aliases/machines/06_later_argument_failure.jett"),
        ProjectedOutcome::RuntimeFailure {
            stdout: "before\nborrowed:2\nfailure\n",
            message: "runtime error: list.__remove_at: index -1 out of bounds",
        },
    );
}

#[test]
fn native_machine_projected_local_view_owned_field_copy_remains_independent() {
    run_case(
        "machine_07_owned_field_copy_control",
        include_str!("local_view_aliases/machines/07_owned_field_copy_control.jett"),
        ProjectedOutcome::Success("414243:4142\n"),
    );
}

#[test]
fn native_machine_projected_local_view_owner_changes_reject_before_publication() {
    use jett_driver::native::{NativeBuildError, build_host_executable_with_options};
    use jett_driver::{BuildOptions, build_file_with_options};

    let cases = [
        (
            "machine_owner_consumption",
            include_str!("local_view_aliases/machines/08_owner_consumption_boundary.jett"),
        ),
        (
            "machine_owner_transition",
            include_str!("local_view_aliases/machines/09_owner_transition_boundary.jett"),
        ),
    ];
    for (name, source_text) in cases {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join(format!("{name}.jett"));
        fs::write(&source, source_text).unwrap();
        let reference = jett_driver::run_file_capture_outcome(&source)
            .unwrap_or_else(|error| panic!("{name}: admitted reference boundary: {error:?}"));
        assert_eq!(reference.stdout, "4142\n", "{name}");
        assert!(reference.debug_events.is_empty(), "{name}: {reference:?}");
        assert!(
            reference.frontend_debug_observations.is_empty(),
            "{name}: {reference:?}"
        );
        let output = directory.path().join("preserved.exe");
        let sentinel = b"existing native publication";
        fs::write(&output, sentinel).unwrap();
        let launcher = if cfg!(windows) {
            NativeLauncherBundle::windows_msvc_static_v1(directory.path().join("unused.lib"))
        } else {
            NativeLauncherBundle::linux_gnu_v1(directory.path().join("unused.a"))
        };
        assert!(!launcher.archive_path.exists());

        for release in [false, true] {
            let options = BuildOptions { release };
            let checked = build_file_with_options(&source, options);
            assert!(
                !checked.has_errors,
                "{name}: the source boundary stays frontend-admitted: {:?}",
                checked.diagnostics
            );
            let error = build_host_executable_with_options(&source, &launcher, &output, options)
                .expect_err(
                    "persistent alias owner must fail before archive lookup and publication",
                );
            assert!(error.debug_observations().is_empty(), "{name}: {error:?}");
            let NativeBuildError::Codegen { source, .. } = error else {
                panic!("{name}: expected persistent alias ownership failure: {error:?}");
            };
            let jett_codegen_cranelift::CodegenError::InvalidMirContract { message, .. } = source
            else {
                panic!("{name}: expected persistent alias ownership contract: {source:?}");
            };
            let owner = message
                .strip_prefix("native owner ")
                .and_then(|rest| {
                    rest.strip_suffix(
                        " consumption or rebinding after creating a local view alias is not implemented",
                    )
                })
                .unwrap_or_else(|| panic!("{name}: unexpected ownership contract: {message}"));
            owner
                .parse::<usize>()
                .unwrap_or_else(|error| panic!("{name}: invalid owner identity {owner}: {error}"));
            assert_eq!(fs::read(&output).unwrap(), sentinel, "{name}");
            assert!(!launcher.archive_path.exists());
        }
    }
}

#[test]
fn native_machine_projected_local_view_pending_whole_owner_fails_before_field_observation() {
    run_case_with_traces(
        "machine_10_pending_whole_machine",
        include_str!("local_view_aliases/machines/10_pending_whole_machine.jett"),
        ProjectedOutcome::RuntimeFailure {
            stdout: "before:whole\n",
            message: "runtime error: field access is not supported on pending(pending(app.Packet@ready(bytes(65, 66), list(2, 3))))",
        },
        &[],
    );
}

#[test]
fn native_machine_projected_local_view_pending_struct_to_machine_owner_preserves_depth() {
    run_case_with_traces(
        "machine_11_pending_struct_to_machine",
        include_str!("local_view_aliases/machines/11_pending_struct_to_machine.jett"),
        ProjectedOutcome::RuntimeFailure {
            stdout: "before:struct-machine\nintermediate\n",
            message: "runtime error: field access is not supported on pending(pending(app.Packet@ready(bytes(67, 68), list(4, 5))))",
        },
        &[
            "trace forwarded_machine: app.Packet at ready = pending(pending(app.Packet@ready(bytes(67, 68), list(4, 5))))\n",
        ],
    );
}

#[test]
fn native_machine_projected_local_view_pending_machine_to_struct_owner_preserves_depth() {
    run_case_with_traces(
        "machine_12_pending_machine_to_struct",
        include_str!("local_view_aliases/machines/12_pending_machine_to_struct.jett"),
        ProjectedOutcome::RuntimeFailure {
            stdout: "before:machine-struct\nintermediate\n",
            message: "runtime error: field access is not supported on pending(pending(app.Payload(data: bytes(69, 70), items: list(6, 7))))",
        },
        &[
            "trace forwarded_struct: app.Payload = pending(pending(app.Payload(data: bytes(69, 70), items: list(6, 7))))\n",
        ],
    );
}

#[test]
fn native_machine_projected_local_view_pending_endpoint_clones_join_without_changing_owner() {
    run_case_with_traces(
        "machine_13_pending_endpoint_copies",
        include_str!("local_view_aliases/machines/13_pending_endpoint_copies.jett"),
        ProjectedOutcome::Success("bytes:414243:4142\nitems:3:2\n"),
        &[
            "trace forwarded: bytes = pending(pending(bytes(65, 66)))\n",
            "trace once: bytes = pending(bytes(65, 66))\n",
            "trace forwarded: bytes = pending(pending(bytes(65, 66)))\n",
            "trace once: bytes = pending(bytes(65, 66))\n",
            "trace forwarded: list[int64] = pending(pending(list(2, 3)))\n",
            "trace once: list[int64] = pending(list(2, 3))\n",
            "trace forwarded: list[int64] = pending(pending(list(2, 3)))\n",
            "trace once: list[int64] = pending(list(2, 3))\n",
            "trace packet: app.Packet at ready = app.Packet@ready(pending(pending(bytes(65, 66))), pending(pending(list(2, 3))))\n",
        ],
    );
}

#[test]
fn native_machine_projected_local_view_pending_whole_owner_clones_join_before_projection() {
    run_case_with_traces(
        "machine_14_pending_whole_owner_copies",
        include_str!("local_view_aliases/machines/14_pending_whole_owner_copies.jett"),
        ProjectedOutcome::Success("bytes:4142:414243:4142\nitems:2:3:2\n"),
        &[
            "trace forwarded: app.Packet at ready = pending(pending(app.Packet@ready(bytes(65, 66), list(2, 3))))\n",
            "trace once: app.Packet at ready = pending(app.Packet@ready(bytes(65, 66), list(2, 3)))\n",
            "trace forwarded: app.Packet at ready = pending(pending(app.Packet@ready(bytes(65, 66), list(2, 3))))\n",
            "trace once: app.Packet at ready = pending(app.Packet@ready(bytes(65, 66), list(2, 3)))\n",
            "trace packet: app.Packet at ready = pending(pending(app.Packet@ready(bytes(65, 66), list(2, 3))))\n",
        ],
    );
}

#[test]
fn native_machine_projected_local_view_generic_and_qualified_endpoints_keep_exact_types() {
    run_case(
        "machine_15_typed_endpoints",
        include_str!("local_view_aliases/machines/15_typed_endpoints.jett"),
        ProjectedOutcome::Success(concat!(
            "scalar:7:7:2:3:2:5:11:2:3:2:3:13\n",
            "bytes:414243:4142:2:3:2:5:11:2:3:2:3:13:2:3:2:5:11:2:3:2:3:13\n",
            "items:3:2:2:3:2:5:11:2:3:2:3:13\n",
        )),
    );
}
