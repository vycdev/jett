//! Selected accepted-print redaction; terminal diagnostics remain independent.
use super::{launcher, run_bounded, suite_options::launcher_for_options};
use jett_driver::native::{build_host_executable, build_host_executable_with_options};
use jett_driver::{BuildOptions, DebugEvent, DebugEventKind, run_file_capture_outcome};
use std::fs;
use std::path::Path;

struct Case {
    name: &'static str,
    source: &'static [u8],
    events: &'static [&'static str],
}

const CASES: &[Case] = &[
    Case {
        name: "07_hidden_secret_debug_print",
        source: include_bytes!("hidden_secret_print/07_hidden_secret_debug_print.jett"),
        events: &["app.Hidden(token: [redacted])\n"],
    },
    Case {
        name: "15_public_erased_debug_print",
        source: include_bytes!("hidden_secret_print/15_public_erased_debug_print.jett"),
        events: &["app.Public(token: sentinel-public)\n"],
    },
    Case {
        name: "16_nested_generic",
        source: include_bytes!("hidden_secret_print/16_nested_generic.jett"),
        events: &[
            "app.Boxed(token: [redacted], value: 7)\n",
            "list(app.Boxed(token: [redacted], value: 7)) app.Boxed(token: [redacted], value: 7)\n",
        ],
    },
    Case {
        name: "17_nested_sums",
        source: include_bytes!("hidden_secret_print/17_nested_sums.jett"),
        events: &[
            "list(some(app.Child(token: [redacted], label: [redacted])), none) ok(app.Child(token: [redacted], label: [redacted])) fail([redacted])\n",
            "app.Choice.item(app.Child(token: [redacted], label: [redacted])) app.Session@active([redacted], 4)\n",
        ],
    },
    Case {
        name: "18_incomplete_builder",
        source: include_bytes!("hidden_secret_print/18_incomplete_builder.jett"),
        events: &[
            "TypeConstruction[app.Vault]()\n",
            "TypeConstruction[app.Vault](token: [redacted])\n",
            "TypeConstruction[app.Vault](token: [redacted], label: public)\n",
        ],
    },
    Case {
        name: "19_pending_erased_collection",
        source: include_bytes!("hidden_secret_print/19_pending_erased_collection.jett"),
        events: &["pending(pending(list([redacted])))\n"],
    },
    Case {
        name: "21_pipeline_and_owned",
        source: include_bytes!("hidden_secret_print/21_pipeline_and_owned.jett"),
        events: &[
            "app.Hidden(token: [redacted])\n",
            "app.Hidden(token: [redacted])\n",
        ],
    },
];

fn assert_native_profiles(source: &Path, name: &str, status: i32, stderr: &[u8]) {
    let directory = source.parent().unwrap();
    let binaries = [false, true].map(|release| {
        let profile = if release { "release" } else { "debug" };
        let binary = directory.join(format!("{name}-{profile}.exe"));
        let launcher = launcher_for_options(release);
        eprintln!(
            "JETT-HIDDEN-PRINT-NATIVE start case={name} profile={profile} source={} binary={} launcher={launcher:?}",
            source.display(),
            binary.display(),
        );
        // Runtime archive profile is independent of the default debug Source policy.
        build_host_executable(source, launcher, &binary)
            .unwrap_or_else(|error| panic!("{name} ({profile}): {error:?}"));
        (profile, binary)
    });
    fs::remove_file(source).unwrap();
    assert!(!source.exists());
    for (profile, binary) in binaries {
        let actual = run_bounded(&binary, directory);
        assert_eq!(
            actual.status.code(),
            Some(status),
            "{name} ({profile}): {actual:?}"
        );
        assert!(actual.stdout.is_empty(), "{name} ({profile}): {actual:?}");
        assert_eq!(actual.stderr, stderr, "{name} ({profile})");
        eprintln!(
            "JETT-HIDDEN-PRINT-NATIVE done case={name} profile={profile} source_deleted={} status={:?} stdout={:?} stderr={:?}",
            !source.exists(),
            actual.status.code(),
            actual.stdout,
            actual.stderr,
        );
    }
}

#[test]
fn native_hidden_secret_print_original_and_nested_sources_keep_selected_exact_events() {
    for case in CASES {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join(format!("{}.jett", case.name));
        fs::write(&source, case.source).unwrap();
        let reference = run_file_capture_outcome(&source)
            .unwrap_or_else(|error| panic!("{}: {error:?}", case.name));
        assert!(reference.stdout.is_empty(), "{}", case.name);
        assert!(
            reference.frontend_debug_observations.is_empty(),
            "{}",
            case.name
        );
        let expected: Vec<_> = case
            .events
            .iter()
            .map(|text| DebugEvent {
                kind: DebugEventKind::Println,
                text: (*text).into(),
            })
            .collect();
        assert_eq!(reference.debug_events, expected, "{}", case.name);
        assert_native_profiles(&source, case.name, 0, case.events.join("").as_bytes());
    }
}

#[test]
fn native_hidden_secret_print_argument_failure_emits_no_outer_event_and_retires_owned_argument() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("20_argument_failure.jett");
    fs::write(
        &source,
        include_bytes!("hidden_secret_print/20_argument_failure.jett"),
    )
    .unwrap();
    let failure = run_file_capture_outcome(&source).unwrap_err();
    assert!(failure.output.stdout.is_empty());
    assert!(failure.output.frontend_debug_observations.is_empty());
    assert_eq!(
        failure.output.debug_events,
        [
            DebugEvent {
                kind: DebugEventKind::Print,
                text: "before:".into()
            },
            DebugEvent {
                kind: DebugEventKind::Print,
                text: "argument:".into()
            },
            DebugEvent {
                kind: DebugEventKind::Println,
                text: "failing\n".into()
            },
        ]
    );
    assert_eq!(
        failure.message,
        "runtime error: string.repeat: requested output is too large"
    );
    // Status 72 would expose an unretired ordinary owner at entry completion.
    assert_native_profiles(
        &source,
        "20_argument_failure",
        71,
        b"before:argument:failing\nruntime error: string.repeat: requested output is too large\n",
    );
}

#[test]
fn native_hidden_secret_print_source_release_rejection_preserves_publication() {
    for case in CASES {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join(format!("{}.jett", case.name));
        let binary = directory.path().join("preserved.exe");
        let sentinel = b"existing native publication";
        fs::write(&source, case.source).unwrap();
        fs::write(&binary, sentinel).unwrap();
        let result = jett_driver::build_file_with_options(&source, BuildOptions { release: true });
        assert!(result.has_errors, "{}", case.name);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code.code() == 362),
            "{}: {:?}",
            case.name,
            result.diagnostics
        );
        build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            BuildOptions { release: true },
        )
        .expect_err("Source release Print/Println must fail before publication");
        assert_eq!(fs::read(&binary).unwrap(), sentinel, "{}", case.name);
    }
}

#[test]
fn native_hidden_secret_print_direct_and_refinement_qualified_source_restrictions_remain() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("direct.jett");
    fs::write(
        &source,
        r#"namespace app
type TokenAlias = secret[string]
type Token = TokenAlias where true
function directly(view token: secret[string]) returns nothing:
    print(view token)
function refined(view token: Token) returns nothing:
    println(view token)
function main() returns nothing:
    return nothing
"#,
    )
    .unwrap();
    let result = jett_driver::build_file(&source);
    assert!(result.has_errors);
    assert_eq!(
        result
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code.code() == 600)
            .count(),
        2,
        "{:?}",
        result.diagnostics
    );
    let release = jett_driver::build_file_with_options(&source, BuildOptions { release: true });
    assert_eq!(
        release
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code.code() == 600)
            .count(),
        2,
        "{:?}",
        release.diagnostics
    );
    assert_eq!(
        release
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code.code() == 362)
            .count(),
        2,
        "{:?}",
        release.diagnostics
    );
}

#[test]
fn native_hidden_secret_print_preserves_once_order_without_invoking_getters_or_callbacks() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory
        .path()
        .join("22_once_order_and_noninvocation.jett");
    fs::write(
        &source,
        include_bytes!("hidden_secret_print/22_once_order_and_noninvocation.jett"),
    )
    .unwrap();
    let expected = [
        (DebugEventKind::Print, "first:"),
        (DebugEventKind::Print, "second:"),
        (
            DebugEventKind::Println,
            "app.Hidden(token: [redacted]) app.Hidden(token: [redacted])\n",
        ),
        (DebugEventKind::Println, "app.Hidden(token: [redacted])\n"),
        (DebugEventKind::Print, "getter:"),
        (DebugEventKind::Println, "public\n"),
        (DebugEventKind::Println, "function(app.callback)\n"),
        (DebugEventKind::Print, "callback:"),
        (DebugEventKind::Print, "predicate:"),
        (DebugEventKind::Println, "7\n"),
    ]
    .into_iter()
    .map(|(kind, text)| DebugEvent {
        kind,
        text: text.into(),
    })
    .collect::<Vec<_>>();
    let reference = run_file_capture_outcome(&source).expect("authenticated print source");
    assert!(reference.stdout.is_empty());
    assert!(reference.frontend_debug_observations.is_empty());
    assert_eq!(reference.debug_events, expected);
    assert_native_profiles(
        &source,
        "22_once_order_and_noninvocation",
        0,
        jett_driver::render_debug_events(&expected).as_bytes(),
    );
}
