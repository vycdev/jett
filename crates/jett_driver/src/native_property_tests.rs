//! Inject a backend-only failure after ordinary frontend validation.
use super::*;

#[test]
fn native_property_shrinking_checks_refinement_chains_and_nested_predicates() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("refined-inputs.jett");
    let text = r#"namespace constraints
function positive(value: int64) returns bool:
    if value == 0:
        string rejected = string.repeat("ab", 9223372036854775807)
        return string.char_count(rejected) > 0
    return value > 0
export type Positive = int64 where positive(value)
export type Large = Positive where value > 1
namespace app
function positive(value: int64) returns bool:
    return false
struct Boxed[T]:
    value: T
type Nonempty = list[constraints.Large] where list.length[constraints.Large](value) > 0
property refined:
    given number: constraints.Large
    given record: Boxed[constraints.Positive]
    given items: Nonempty
    assert true "property sentinel"
"#;
    fs::write(&source, text).unwrap();
    let mut lowered = lower_file_for_native_property_suite(&source).unwrap();
    let mut changed = 0;
    for function in &mut lowered.hir.functions {
        if function.identity.declaration.kind != jett_hir::DeclarationKind::Property {
            continue;
        }
        for statement in &mut function.body.statements {
            if let jett_hir::StatementKind::Assert { condition, .. } = &mut statement.kind {
                assert_eq!(condition.kind, jett_hir::ExpressionKind::Bool(true));
                condition.kind = jett_hir::ExpressionKind::Bool(false);
                changed += 1;
            }
        }
    }
    assert_eq!(changed, 1);
    lowered.mir = jett_mir::lower(&lowered.hir, &lowered.interner).unwrap();
    fs::write(&source, text.replace("assert true", "assert false")).unwrap();
    let oracle = crate::test_file(&source).unwrap();
    assert_eq!(oracle.failed, 1);
    let expected = oracle.blocks[0].error.as_deref().unwrap();
    assert!(expected.contains("number = 2"), "{expected}");
    assert!(expected.contains("value: 1"), "{expected}");
    assert!(expected.contains("items = list(2)"), "{expected}");
    fs::remove_file(&source).unwrap();
    let launcher = launcher();
    for optimize in [false, true] {
        let result = property_runner::run_lowered(
            &source,
            &lowered,
            &launcher,
            NativePropertyOptions {
                optimize,
                ..NativePropertyOptions::default()
            },
        )
        .unwrap();
        let failure = result.failure.unwrap();
        assert_eq!(result.trials, 1);
        assert_eq!(failure.trial, 1);
        assert_eq!(
            format!(
                "property sentinel (counterexample: {})",
                failure.counterexample
            ),
            expected
        );
        assert!(result.stdout.is_empty());
        assert_eq!(
            result.stderr,
            b"runtime error: property 'refined' trial 1: property sentinel\n"
        );
    }
}

#[test]
fn native_property_shrinking_preserves_narrow_boundaries_and_nested_inputs() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("integer-boundary.jett");
    let text = r#"namespace app
struct Boxed[T]:
    value: T
property boundary:
    given number: int8
    given record: Boxed[int8]
    given numbers: list[int8]
    given lookup: map[int8, Boxed[int8]]
    int8 minimum = -128
    assert number >= minimum "minimum sentinel"
"#;
    fs::write(&source, text).unwrap();
    let mut lowered = lower_file_for_native_property_suite(&source).unwrap();
    let mut changed = 0;
    for function in &mut lowered.hir.functions {
        if function.identity.declaration.kind != jett_hir::DeclarationKind::Property {
            continue;
        }
        for statement in &mut function.body.statements {
            if let jett_hir::StatementKind::Assert { condition, .. } = &mut statement.kind {
                if let jett_hir::ExpressionKind::Binary { op, .. } = &mut condition.kind {
                    assert_eq!(*op, jett_hir::BinaryOp::GreaterEqual);
                    *op = jett_hir::BinaryOp::NotEqual;
                    changed += 1;
                }
            }
        }
    }
    assert_eq!(changed, 1);
    lowered.mir = jett_mir::lower(&lowered.hir, &lowered.interner).unwrap();
    fs::write(&source, text.replace(">= minimum", "!= minimum")).unwrap();
    let oracle = crate::test_file(&source).unwrap();
    assert_eq!(oracle.failed, 1);
    assert_eq!(oracle.blocks[0].iterations, Some(8));
    let expected = oracle.blocks[0].error.as_deref().unwrap();
    assert!(expected.contains("number = -128"), "{expected}");
    assert!(!expected.contains("number = 128"), "{expected}");
    fs::remove_file(&source).unwrap();
    let launcher = launcher();
    for optimize in [false, true] {
        let result = property_runner::run_lowered(
            &source,
            &lowered,
            &launcher,
            NativePropertyOptions {
                optimize,
                ..NativePropertyOptions::default()
            },
        )
        .unwrap();
        let failure = result.failure.unwrap();
        assert_eq!(result.trials, 8);
        assert_eq!(failure.trial, 8);
        assert_eq!(failure.name, "boundary");
        assert_eq!(
            format!(
                "minimum sentinel (counterexample: {})",
                failure.counterexample
            ),
            expected
        );
        assert!(result.stdout.is_empty());
        assert_eq!(
            result.stderr,
            b"runtime error: property 'boundary' trial 8: minimum sentinel\n"
        );
    }
}

#[test]
fn native_property_runner_shrinks_native_failures_from_the_checked_session() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("shrink.jett");
    let text = "namespace app\nproperty first:\n    given value: int8\n    assert value == value\nproperty target:\n    given number: int8\n    given items: list[string]\n    assert number <= 127 \"backend sentinel\"\n";
    fs::write(&source, text).unwrap();
    let mut lowered = lower_file_for_native_property_suite(&source).unwrap();
    let launcher = launcher();
    let success =
        run_host_property_suite(&source, &launcher, NativePropertyOptions::default()).unwrap();
    assert!(success.failure.is_none());
    assert_eq!(success.trials, 200);
    assert!(success.stdout.is_empty() && success.stderr.is_empty());

    // Keep frontend validation mandatory, then inject a backend-only failure
    // into the retained checked HIR. All later executions are native.
    let mut changed = 0;
    for function in &mut lowered.hir.functions {
        if function.identity.declaration.kind != jett_hir::DeclarationKind::Property {
            continue;
        }
        for statement in &mut function.body.statements {
            if let jett_hir::StatementKind::Assert { condition, .. } = &mut statement.kind {
                if let jett_hir::ExpressionKind::Binary { right, .. } = &mut condition.kind {
                    if right.kind == jett_hir::ExpressionKind::Int(127) {
                        right.kind = jett_hir::ExpressionKind::Int(2);
                        changed += 1;
                    }
                }
            }
        }
    }
    assert_eq!(changed, 1);
    lowered.mir = jett_mir::lower(&lowered.hir, &lowered.interner).unwrap();

    // The source oracle must reject the corresponding failing property, while
    // a retained native session can replay even after its source is removed.
    fs::write(&source, text.replace("<= 127", "<= 2")).unwrap();
    let oracle = crate::test_file(&source).unwrap();
    assert!(matches!(
        run_host_property_suite(&source, &launcher, NativePropertyOptions::default()),
        Err(NativePropertyRunError::Lowering(
            BackendLoweringError::Build(_)
        ))
    ));
    fs::remove_file(&source).unwrap();
    for optimize in [false, true] {
        let result = property_runner::run_lowered(
            &source,
            &lowered,
            &launcher,
            NativePropertyOptions {
                optimize,
                ..NativePropertyOptions::default()
            },
        )
        .unwrap();
        let failure = result.failure.expect("the native assertion must fail");
        assert_eq!(result.trials, 104);
        assert_eq!(failure.name, "target");
        assert_eq!(failure.trial, 4);
        assert_eq!(failure.counterexample, "number = 3, items = list()");
        assert_eq!(failure.span.start as usize, text.find("target:").unwrap());
        assert!(result.stdout.is_empty());
        assert_eq!(
            result.stderr,
            b"runtime error: property 'target' trial 4: backend sentinel\n"
        );
        let expected = format!(
            "backend sentinel (counterexample: {})",
            failure.counterexample
        );
        assert_eq!(oracle.failed, 1);
        assert_eq!(oracle.blocks[1].error.as_deref(), Some(expected.as_str()));
        assert_eq!(oracle.blocks[1].iterations, Some(failure.trial));
    }
}

#[test]
fn native_property_runner_does_not_shrink_timeouts_or_unreproducible_failures() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("replay.jett");
    fs::write(&source, "property stable:\n    assert true\n").unwrap();
    let launcher = launcher();
    let mut lowered = lower_file_for_native_property_suite(&source).unwrap();
    // A fault present only in the initial MIR cannot be reproduced by checked
    // HIR replay. Do not return a passing result or fabricate a counterexample.
    let property = lowered
        .mir
        .functions
        .iter_mut()
        .find(|function| {
            function.identity.declaration.kind == jett_hir::DeclarationKind::Property
                && !function
                    .identity
                    .declaration
                    .name
                    .starts_with("__native_property_suite:")
        })
        .unwrap();
    let mut changed = 0;
    for block in &mut property.blocks {
        for statement in &mut block.statements {
            if let jett_mir::StatementKind::Assert { condition, .. } = &mut statement.kind {
                condition.kind = jett_hir::ExpressionKind::Bool(false);
                changed += 1;
            }
        }
    }
    assert_eq!(changed, 1);
    let error = property_runner::run_lowered(
        &source,
        &lowered,
        &launcher,
        NativePropertyOptions::default(),
    )
    .unwrap_err();
    assert!(matches!(
        &error,
        NativePropertyRunError::ReplayPassed { .. }
    ));
    assert!(
        error
            .to_string()
            .contains("runtime error: property 'stable' trial 1: assertion failed")
    );

    let property = lowered
        .hir
        .functions
        .iter_mut()
        .find(|function| {
            function.identity.declaration.kind == jett_hir::DeclarationKind::Property
                && !function
                    .identity
                    .declaration
                    .name
                    .starts_with("__native_property_suite:")
        })
        .unwrap();
    property.body.statements.push(jett_hir::Statement {
        kind: jett_hir::StatementKind::While {
            condition: jett_hir::Expression {
                kind: jett_hir::ExpressionKind::Bool(true),
                ty: jett_types::TypeInterner::BOOL,
                span: property.span,
            },
            body: jett_hir::Block {
                statements: Vec::new(),
                span: property.span,
            },
        },
        span: property.span,
    });
    lowered.mir = jett_mir::lower(&lowered.hir, &lowered.interner).unwrap();
    let error = property_runner::run_lowered(
        &source,
        &lowered,
        &launcher,
        NativePropertyOptions {
            attempt_timeout: Duration::from_millis(25),
            ..NativePropertyOptions::default()
        },
    )
    .unwrap_err();
    assert!(
        matches!(
            error,
            NativePropertyRunError::Execution(NativeBuildError::LinkTimedOut { .. })
        ),
        "{error}"
    );
}

fn launcher() -> NativeLauncherBundle {
    let executable = std::env::current_exe().unwrap();
    let profile_directory = executable.parent().unwrap().parent().unwrap();
    let profile = profile_directory.file_name().unwrap().to_str().unwrap();
    let target = profile_directory
        .parent()
        .unwrap()
        .join("native-values-launcher");
    let host = host_target();
    let status = Command::new(env!("CARGO"))
        .args([
            "build",
            "--locked",
            "-q",
            "-p",
            "jett_native_launcher",
            "--target",
            &host,
        ])
        .args([
            "--profile",
            if profile == "debug" { "test" } else { profile },
        ])
        .arg("--target-dir")
        .arg(&target)
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .status()
        .unwrap();
    assert!(status.success(), "launcher build: {status}");
    let directory = target.join(host).join(profile);
    if cfg!(windows) {
        NativeLauncherBundle::windows_msvc_static_v1(directory.join("jett_native_launcher.lib"))
    } else {
        NativeLauncherBundle::linux_gnu_v1(directory.join("libjett_native_launcher.a"))
    }
}

#[test]
fn native_property_failure_identifies_the_trial_and_cleans_owned_values() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("property.jett");
    fs::write(
        &source,
        r#"namespace app
property first:
    given value: int8
    assert value == value
property target:
    given number: int8
    given items: list[string]
    assert number <= 127 "backend sentinel"
"#,
    )
    .unwrap();
    let launcher = launcher();
    for (optimize, fail_in_trial) in [(false, true), (true, true), (false, false)] {
        let mut lowered = lower_file_for_native_property_suite(&source)
            .expect("the original property must pass frontend validation");
        let entry = lowered.native_property_entry.unwrap();
        let mut changed = 0;
        for function in &mut lowered.mir.functions {
            if !fail_in_trial
                || function.identity.declaration.kind != jett_hir::DeclarationKind::Property
            {
                continue;
            }
            for block in &mut function.blocks {
                for statement in &mut block.statements {
                    if let jett_mir::StatementKind::Assert { condition, .. } = &mut statement.kind {
                        if let jett_hir::ExpressionKind::Binary { right, .. } = &mut condition.kind
                        {
                            if right.kind == jett_hir::ExpressionKind::Int(127) {
                                // A backend divergence, after all source validation, makes
                                // the fourth deterministic int8 trial (42) fail.
                                right.kind = jett_hir::ExpressionKind::Int(2);
                                changed += 1;
                            }
                        }
                    }
                }
            }
        }
        if fail_in_trial {
            assert_eq!(changed, 1, "alter exactly the checked target assertion");
        } else {
            let suite = lowered
                .mir
                .functions
                .iter_mut()
                .find(|f| f.id == entry)
                .unwrap();
            let exits = suite
                .blocks
                .iter_mut()
                .filter(|block| {
                    matches!(block.terminator.kind, jett_mir::TerminatorKind::Return(_))
                })
                .collect::<Vec<_>>();
            assert_eq!(exits.len(), 1);
            for exit in exits {
                exit.statements.push(jett_mir::Statement {
                    kind: jett_mir::StatementKind::Evaluate(jett_hir::Expression {
                        kind: jett_hir::ExpressionKind::RuntimeFailure("after suite".to_owned()),
                        ty: jett_types::TypeInterner::NOTHING,
                        span: suite.span,
                    }),
                    span: suite.span,
                });
            }
        }
        let object = emit_program_object_from_lowering(&source, lowered, entry, optimize).unwrap();
        let binary = directory
            .path()
            .join(format!("property-{optimize}-{fail_in_trial}.exe"));
        link_host_object(&object, &launcher, &binary).unwrap();
        let output =
            run_command_with_timeout(Command::new(&binary), &binary, Duration::from_secs(60))
                .unwrap();
        assert_eq!(output.status.code(), Some(71), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        assert_eq!(
            String::from_utf8(output.stderr).unwrap(),
            if fail_in_trial {
                "runtime error: property 'target' trial 4: backend sentinel\n"
            } else {
                "runtime error: after suite\n"
            }
        );
    }
}
