use std::fs;
use std::path::{Path, PathBuf};

use jett_common::FileId;
use jett_diagnostics::Severity;
use jett_driver::{build_file, run_file};
use jett_parser::ast::Item;
use jett_parser::parse;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("workspace root should resolve")
}

fn fixture_path(kind: &str, name: &str) -> PathBuf {
    workspace_root().join("tests").join(kind).join(name)
}

fn fixture_source(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
}

fn fixture_has_main(path: &Path) -> bool {
    let source = fixture_source(path);
    let parsed = parse(&source, FileId::new(0));
    parsed.module.items.iter().any(|item| {
        matches!(
            item,
            Item::Function(func) if func.name.name == "main"
        )
    })
}

fn expected_error_codes(source: &str) -> Vec<u32> {
    source
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            let rest = trimmed.strip_prefix("# ERROR:")?.trim();
            let digits = rest
                .strip_prefix('E')
                .unwrap_or(rest)
                .chars()
                .take_while(|ch| ch.is_ascii_digit())
                .collect::<String>();
            digits.parse::<u32>().ok()
        })
        .collect()
}

fn error_messages(path: &Path, diagnostics: &[jett_diagnostics::Diagnostic]) -> String {
    diagnostics
        .iter()
        .filter(|diag| diag.severity == Severity::Error)
        .map(|diag| format!("{}: {}", diag.code, diag.message))
        .collect::<Vec<_>>()
        .join("\n")
        .if_empty_then(format!("{} produced no error diagnostics", path.display()))
}

trait IfEmptyThen {
    fn if_empty_then(self, fallback: String) -> String;
}

impl IfEmptyThen for String {
    fn if_empty_then(self, fallback: String) -> String {
        if self.is_empty() {
            fallback
        } else {
            self
        }
    }
}

fn assert_compile_pass(name: &str) {
    let path = fixture_path("compile_pass", name);
    let result = build_file(&path);
    assert!(
        !result.has_errors,
        "expected {} to compile successfully:\n{}",
        path.display(),
        error_messages(&path, &result.diagnostics)
    );
}

fn assert_compile_fail(name: &str) {
    let path = fixture_path("compile_fail", name);
    let source = fixture_source(&path);
    let expected_codes = expected_error_codes(&source);
    let result = build_file(&path);

    assert!(
        result.has_errors,
        "expected {} to fail compilation",
        path.display()
    );

    let actual_codes: Vec<u32> = result
        .diagnostics
        .iter()
        .filter(|diag| diag.severity == Severity::Error)
        .map(|diag| u32::from(diag.code.code()))
        .collect();

    for expected in expected_codes {
        assert!(
            actual_codes.contains(&expected),
            "expected {} to report E{:04}, got:\n{}",
            path.display(),
            expected,
            error_messages(&path, &result.diagnostics)
        );
    }
}

fn assert_run_pass(name: &str) {
    let path = fixture_path("run_pass", name);
    if fixture_has_main(&path) {
        run_file(&path)
            .unwrap_or_else(|err| panic!("expected {} to run successfully: {err}", path.display()));
    } else {
        let result = build_file(&path);
        assert!(
            !result.has_errors,
            "expected {} verify/property fixture to succeed:\n{}",
            path.display(),
            error_messages(&path, &result.diagnostics)
        );
    }
}

macro_rules! compile_pass_fixture {
    ($test_name:ident, $file_name:literal) => {
        #[test]
        fn $test_name() {
            assert_compile_pass($file_name);
        }
    };
}

macro_rules! compile_fail_fixture {
    ($test_name:ident, $file_name:literal) => {
        #[test]
        fn $test_name() {
            assert_compile_fail($file_name);
        }
    };
}

macro_rules! run_pass_fixture {
    ($test_name:ident, $file_name:literal) => {
        #[test]
        fn $test_name() {
            assert_run_pass($file_name);
        }
    };
}

compile_pass_fixture!(compile_pass_basic, "basic.jett");
compile_pass_fixture!(compile_pass_hello, "hello.jett");
compile_pass_fixture!(
    compile_pass_interface_displayable,
    "interface_displayable.jett"
);
compile_pass_fixture!(compile_pass_handle_refinement, "handle_refinement.jett");
compile_pass_fixture!(compile_pass_generic_struct, "generic_struct.jett");

compile_fail_fixture!(compile_fail_type_mismatch, "type_mismatch.jett");
compile_fail_fixture!(compile_fail_secret_stdout, "secret_stdout.jett");
compile_fail_fixture!(
    compile_fail_refinement_requires_handle,
    "refinement_requires_handle.jett"
);

run_pass_fixture!(run_pass_simple, "simple.jett");
run_pass_fixture!(run_pass_fibonacci, "fibonacci.jett");
run_pass_fixture!(run_pass_hello_print, "hello_print.jett");
run_pass_fixture!(run_pass_string_interpolation, "string_interpolation.jett");
run_pass_fixture!(run_pass_verify_test, "verify_test.jett");
run_pass_fixture!(run_pass_multi_verify, "multi_verify.jett");
run_pass_fixture!(
    run_pass_handle_result_optional,
    "handle_result_optional.jett"
);
run_pass_fixture!(run_pass_bitfield_roundtrip, "bitfield_roundtrip.jett");
run_pass_fixture!(run_pass_generic_struct, "generic_struct.jett");
run_pass_fixture!(run_pass_generic_function, "generic_function.jett");
run_pass_fixture!(run_pass_actor_counter, "actor_counter.jett");
