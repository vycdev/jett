use super::*;
use jett_driver::BuildOptions;

#[test]
fn native_interface_equality_is_rejected_before_publication() {
    for (name, count) in [
        ("interface_equality.jett", 7),
        ("interface_equality_generic.jett", 3),
        ("interface_equality_comptime.jett", 2),
        ("interface_equality_enum_payload.jett", 4),
    ] {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/compile_fail")
            .join(name);
        let text = fs::read_to_string(fixture).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("main.jett");
        fs::write(
            &source,
            format!("{text}function main() returns nothing:\n    return nothing\n"),
        )
        .unwrap();
        for release in [false, true] {
            let binary = directory.path().join("preserved.exe");
            fs::write(&binary, b"existing output").unwrap();
            let error = jett_driver::native::build_host_executable_with_options(
                &source,
                super::suite_options::launcher_for_options(release),
                &binary,
                BuildOptions { release },
            )
            .unwrap_err();
            let jett_driver::native::NativeBuildError::Lowering {
                source: lowering, ..
            } = error
            else {
                panic!("{name}: expected frontend rejection, got {error}");
            };
            let jett_driver::BackendLoweringError::Build(checked) = *lowering else {
                panic!("{name}: expected diagnostics before HIR");
            };
            let errors = checked
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.severity == jett_diagnostics::Severity::Error)
                .collect::<Vec<_>>();
            assert_eq!(errors.len(), count, "{name}: {errors:?}");
            for diagnostic in errors {
                assert_eq!(diagnostic.code.code(), 376, "{name}: {diagnostic:?}");
                assert!(diagnostic.message.contains("interface-typed values"));
                assert!(diagnostic.message.contains("compare before erasure"));
            }
            assert_eq!(fs::read(&binary).unwrap(), b"existing output");
        }
    }
}

#[test]
fn native_explicit_interface_comparison_matches_both_profiles_without_source() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/interface_explicit_comparison.jett");
    let text = fs::read_to_string(fixture).unwrap();
    for release in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("main.jett");
        fs::write(&source, &text).unwrap();
        let expected = jett_driver::run_file_capture_output(&source)
            .expect("explicit domain comparison reference");
        assert_eq!(expected.stdout, "true false\ntrue true\n7 7 8\n");
        assert!(expected.debug_events.is_empty());
        let binary = directory.path().join("comparison.exe");
        jett_driver::native::build_host_executable_with_options(
            &source,
            super::suite_options::launcher_for_options(release),
            &binary,
            BuildOptions { release },
        )
        .expect("compile explicit domain comparisons");
        fs::remove_file(&source).unwrap();
        let actual = run_bounded(&binary, directory.path());
        assert_eq!(actual.status.code(), Some(0), "{actual:?}");
        assert_eq!(actual.stdout, expected.stdout.as_bytes(), "{actual:?}");
        assert!(actual.stderr.is_empty(), "{actual:?}");
        fs::remove_file(&binary).unwrap();
    }
}
