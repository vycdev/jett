use super::*;

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
        expected.debug_output,
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
            format!("{}\n", expected.debug_output.join("\n"))
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
    assert!(expected.output.debug_output.is_empty());
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
