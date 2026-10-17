use super::*;

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native")
        .join(name)
}

#[test]
fn native_actor_identity_original_and_nested_sources_match_current_runtime_profile_without_source()
{
    for (name, debug_text) in [
        ("actor_enum_equality.jett", "true false\n"),
        (
            "actor_pending_alias_baseline.jett",
            "actor#0 pending(actor#0)\nactor#0 actor#0\n",
        ),
        (
            "actor_pending_alias_identity.jett",
            "true false\ntrue false false true false true\ntrue false 10 10\n",
        ),
        (
            "actor_direct_identity.jett",
            "true false false true\ntrue false\n",
        ),
        (
            "actor_nested_identity.jett",
            "true false false true\ntrue true\ntrue false\n",
        ),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("main.jett");
        fs::copy(fixture(name), &source).unwrap();
        let expected = jett_driver::run_file_capture_output(&source)
            .expect("selected actor identity reference");
        assert!(expected.stdout.is_empty(), "{name}");
        assert_eq!(
            debug_print_text(&expected.debug_events),
            debug_text,
            "{name}"
        );
        assert!(debug_trace_lines(&expected.debug_events).is_empty());
        let binary = directory.path().join("actor.exe");
        // Original08 uses println, whose checked Source policy is debug. The
        // Cargo test profile selects the matching normal runtime archive.
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release: false },
        )
        .unwrap_or_else(|error| panic!("{name}: {error}"));
        fs::remove_file(&source).unwrap();
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{name}: {actual:?}");
        assert!(actual.stdout.is_empty(), "{name}: {actual:?}");
        assert_eq!(
            actual.stderr,
            jett_driver::render_debug_events(&expected.debug_events).as_bytes(),
            "{name}"
        );
    }
}

#[test]
fn native_actor_identity_pending_direct_operations_keep_reference_errors_in_current_runtime_profile()
 {
    let template = fs::read_to_string(fixture("actor_pending_identity.jett")).unwrap();
    for (operator, label) in [("==", "Eq"), ("!=", "NotEq")] {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("main.jett");
        fs::write(
            &source,
            template.replace("pending == pending", &format!("pending {operator} pending")),
        )
        .unwrap();
        let expected = jett_driver::run_file_capture_outcome(&source)
            .expect_err("direct pending actor equality remains unsupported");
        let message = format!(
            "runtime error: unsupported binary operation: pending(actor#0) {label} pending(actor#0)"
        );
        assert_eq!(expected.message, message);
        assert!(expected.output.stdout.is_empty());
        assert!(debug_trace_lines(&expected.output.debug_events).is_empty());
        let binary = directory.path().join("actor_pending.exe");
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release: false },
        )
        .expect("native pending actor comparison");
        fs::remove_file(&source).unwrap();
        let actual = run_bounded(&binary, directory.path());
        assert_eq!(actual.status.code(), Some(71), "{actual:?}");
        assert!(actual.stdout.is_empty(), "{actual:?}");
        assert_eq!(
            actual.stderr,
            format!("{message}\n").as_bytes(),
            "{actual:?}"
        );
    }
}

#[test]
fn native_actor_identity_production_output_matches_with_optimized_objects_without_source() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("main.jett");
    fs::copy(fixture("actor_optimized_identity.jett"), &source).unwrap();
    let expected =
        jett_driver::run_file_capture_output(&source).expect("production actor identity reference");
    assert_eq!(
        expected.stdout,
        concat!(
            "true false false true\n",
            "true false false true\n",
            "true false false true false true\n",
            "true false 10 10\n",
            "true false\n",
        )
    );
    assert!(expected.debug_events.is_empty());
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory
            .path()
            .join(format!("actor_identity_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .expect("native production actor identity");
        binaries.push(binary);
    }
    fs::remove_file(&source).unwrap();
    for binary in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes(), "{actual:?}");
        assert!(actual.stderr.is_empty(), "{actual:?}");
    }
}
