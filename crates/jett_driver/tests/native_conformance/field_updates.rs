use super::*;

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/field_update_reference")
        .join(name)
}

fn assert_main(name: &str, stdout: &str) {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("main.jett");
    fs::copy(fixture(name), &source).unwrap();
    let expected = jett_driver::run_file_capture_output(&source)
        .unwrap_or_else(|error| panic!("{name}: {error}"));
    assert_eq!(expected.stdout, stdout, "{name}");
    assert!(expected.debug_events.is_empty(), "{name}");
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory.path().join(format!("fields_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            suite_options::launcher_for_options(release),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .unwrap_or_else(|error| panic!("{name}, release={release}: {error}"));
        binaries.push(binary);
    }
    fs::remove_file(&source).unwrap();
    for binary in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{name}: {actual:?}");
        assert_eq!(actual.stdout, stdout.as_bytes(), "{name}");
        assert!(actual.stderr.is_empty(), "{name}: {actual:?}");
    }
}

#[test]
fn native_owned_field_updates_preserve_values_and_clone_independence_without_source() {
    for (name, stdout) in [
        (
            "01_called_baselines.jett",
            "original:1:before\nchanged:9:after\n",
        ),
        ("02_owning_clone_fields.jett", "2:1:before:3:3:after:true\n"),
        (
            "03_rhs_order_and_early_return.jett",
            "rhs\nchanged:9:before\nunchanged:1:before\n",
        ),
        ("04_exact_machine_state.jett", "machine:8:after:true\n"),
        (
            "07_pending_children_transport.jett",
            "children:17:5:again\n",
        ),
        ("09_generic_alias.jett", "1:9:number:new:text\n"),
        ("10_refined_field_replacement.jett", "2:stable\n"),
        ("21_guarded_field_control.jett", "guarded:4\n"),
        (
            "32_borrowed_match_owner_preservation.jett",
            "matches:6:6:6:6:6:6:6\n",
        ),
        ("22_branch_loop_lifetimes.jett", "5:first:5:second\n"),
        ("23_required_comptime.jett", "baked:9\n"),
        (
            "25_secret_owned_root.jett",
            "secret:1:before:2|9:after:8|1:before:2\n",
        ),
        ("26_for_loop_update.jett", "loop:6:1:stable\n"),
        ("27_nested_handler_update.jett", "handler:1:7:stable\n"),
        (
            "28_metadata_holder_replacement.jett",
            "metadata:string:stable\n",
        ),
        (
            "29_inline_owned_parameter.jett",
            "inline:1:stable|7:changed\n",
        ),
        (
            "30_returned_inline_owned_parameter.jett",
            "returned:1:8:stable\n",
        ),
        (
            "31_reflected_iteration_update.jett",
            "reflected:6:int64|string|int64|:1:stable\n",
        ),
    ] {
        assert_main(name, stdout);
    }
}

#[test]
fn native_field_update_refinement_and_bitfield_barriers_preserve_existing_publications() {
    use jett_driver::native::{NativeBuildError, NativeLauncherBundle};

    for (name, message) in [
        (
            "11_refined_root_rejected.jett",
            "cannot assign through a nominal refinement; replace the complete validated value instead",
        ),
        (
            "12_refined_intermediate_rejected.jett",
            "cannot assign through a nominal refinement; replace the complete validated value instead",
        ),
        (
            "13_bitfield_rejected.jett",
            "cannot assign through a bitfield; construct and rebind the complete value instead",
        ),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("main.jett");
        fs::copy(fixture(name), &source).unwrap();
        let binary = directory.path().join("preserved.exe");
        let sentinel = b"existing field-update output";
        fs::write(&binary, sentinel).unwrap();
        let launcher = if cfg!(windows) {
            NativeLauncherBundle::windows_msvc_static_v1(directory.path().join("unused.lib"))
        } else {
            NativeLauncherBundle::linux_gnu_v1(directory.path().join("unused.a"))
        };
        let check = |error: NativeBuildError| {
            let NativeBuildError::Lowering { source, .. } = error else {
                panic!("{name}: expected frontend rejection before archive lookup: {error}");
            };
            let jett_driver::BackendLoweringError::Build(built) = *source else {
                panic!("{name}: expected the selected field-write diagnostic before HIR");
            };
            let errors = built
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.severity == jett_diagnostics::Severity::Error)
                .collect::<Vec<_>>();
            assert!(built.has_errors, "{name}");
            assert_eq!(errors.len(), 1, "{name}: {:?}", built.diagnostics);
            assert_eq!(errors[0].code.code(), 404, "{name}");
            assert_eq!(errors[0].message, message, "{name}");
            assert_eq!(fs::read(&binary).unwrap(), sentinel, "{name}");
        };
        for release in [false, true] {
            let options = jett_driver::BuildOptions { release };
            check(
                jett_driver::native::build_host_executable_with_options(
                    &source, &launcher, &binary, options,
                )
                .expect_err("invalid field update prevents native main publication"),
            );
            check(
                jett_driver::native::build_host_verify_suite_executable_with_options(
                    &source, &launcher, &binary, options,
                )
                .expect_err("invalid field update prevents native verify publication"),
            );
            check(
                jett_driver::native::build_host_property_suite_executable_with_options(
                    &source, &launcher, &binary, options,
                )
                .expect_err("invalid field update prevents native property publication"),
            );
        }
    }
}

#[test]
fn native_field_update_suites_execute_checked_verify_and_property_bodies_without_source() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("main.jett");
    fs::copy(fixture("01_called_baselines.jett"), &source).unwrap();
    let report = jett_driver::test_file(&source).expect("reference field update suites");
    assert_eq!(report.total, 2);
    assert_eq!(report.passed, 2);
    assert_eq!(report.failed, 0);
    assert!(report.blocks.iter().any(|block| {
        block.is_property && block.iterations.is_some_and(|iterations| iterations > 0)
    }));
    let mut binaries = Vec::new();
    for release in [false, true] {
        let launcher = suite_options::launcher_for_options(release);
        let options = jett_driver::BuildOptions { release };
        let verify = directory.path().join(format!("verify_{release}.exe"));
        jett_driver::native::build_host_verify_suite_executable_with_options(
            &source, launcher, &verify, options,
        )
        .expect("native field update verify suite");
        let property = directory.path().join(format!("property_{release}.exe"));
        jett_driver::native::build_host_property_suite_executable_with_options(
            &source, launcher, &property, options,
        )
        .expect("native field update property suite");
        binaries.extend([verify, property]);
    }
    fs::remove_file(&source).unwrap();
    for binary in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert!(actual.stdout.is_empty(), "{actual:?}");
        assert!(actual.stderr.is_empty(), "{actual:?}");
    }
}

#[test]
fn native_field_update_preflight_precedes_rhs_and_preserves_pending_child_depth_without_source() {
    for (name, stdout, diagnostic) in [
        (
            "05_pending_root_preflight.jett",
            "before:root\n",
            "field access is not supported on ",
        ),
        (
            "06_pending_intermediate_preflight.jett",
            "before:inner\n",
            "field access is not supported on ",
        ),
        (
            "08_stored_child_depth_refusal.jett",
            "before:remaining-depth\n",
            "unsupported binary operation: ",
        ),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("main.jett");
        fs::copy(fixture(name), &source).unwrap();
        let expected = jett_driver::run_file_capture_outcome(&source)
            .expect_err("pending field inspection must fail");
        assert_eq!(expected.output.stdout, stdout, "{name}");
        assert!(expected.output.debug_events.is_empty(), "{name}");
        assert!(
            expected.message.starts_with("runtime error: "),
            "{name}: {}",
            expected.message
        );
        assert!(
            expected.message.contains(diagnostic),
            "{name}: {}",
            expected.message
        );
        let stderr = format!("{}\n", expected.message);
        let mut binaries = Vec::new();
        for release in [false, true] {
            let binary = directory.path().join(format!("pending_{release}.exe"));
            jett_driver::native::build_host_executable_with_options(
                &source,
                suite_options::launcher_for_options(release),
                &binary,
                jett_driver::BuildOptions { release },
            )
            .unwrap_or_else(|error| panic!("{name}, release={release}: {error}"));
            binaries.push(binary);
        }
        fs::remove_file(&source).unwrap();
        for binary in binaries {
            let actual = run_bounded(&binary, directory.path());
            assert_eq!(actual.status.code(), Some(71), "{name}: {actual:?}");
            assert_eq!(actual.stdout, stdout.as_bytes(), "{name}");
            assert_eq!(actual.stderr, stderr.as_bytes(), "{name}: {actual:?}");
        }
    }
}
