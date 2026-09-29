//! Inject a backend-only failure after ordinary frontend validation.
use super::*;

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
