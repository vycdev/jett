//! Original Sources and additive callable-return controls. No Resource launcher.
use super::*;

struct Case {
    name: &'static str,
    source: &'static str,
    stdout: &'static str,
    message: Option<&'static str>,
    markers: &'static [&'static str],
}

const CASES: &[Case] = &[
    Case {
        name: "original_wider",
        source: include_str!("reflected_callable_sources/01_original_wider.jett"),
        stdout: "",
        message: None,
        markers: &[],
    },
    Case {
        name: "original_exact",
        source: include_str!("reflected_callable_sources/02_original_exact.jett"),
        stdout: "",
        message: None,
        markers: &[],
    },
    Case {
        name: "read_without_invocation",
        source: include_str!("reflected_callable_sources/03_read_without_invocation.jett"),
        stdout: "stored\n",
        message: None,
        markers: &[],
    },
    Case {
        name: "named_twice_source_unchanged",
        source: include_str!("reflected_callable_sources/04_named_twice_source_unchanged.jett"),
        stdout: "5:5:5\n",
        message: None,
        markers: &["source", "predicate", "source", "predicate", "source"],
    },
    Case {
        name: "captured_twice",
        source: include_str!("reflected_callable_sources/05_captured_twice.jett"),
        stdout: "7:7\n",
        message: None,
        markers: &["source", "predicate", "source", "predicate"],
    },
    Case {
        name: "invalid_return",
        source: include_str!("reflected_callable_sources/06_invalid_return.jett"),
        stdout: "",
        message: Some("runtime error: refinement type constraint failed for 'app.Positive'"),
        markers: &["source", "predicate"],
    },
    Case {
        name: "pending_result",
        source: include_str!("reflected_callable_sources/07_pending_result.jett"),
        stdout: "",
        message: Some("runtime error: reflected callback return: expected a ready value"),
        markers: &["source"],
    },
    Case {
        name: "pending_callable",
        source: include_str!("reflected_callable_sources/08_pending_callable.jett"),
        stdout: "",
        message: Some(
            "runtime error: expected function value, got pending(function(app.source_five))",
        ),
        markers: &[],
    },
    Case {
        name: "wrong_selector_owner",
        source: include_str!("reflected_callable_sources/09_wrong_selector_owner.jett"),
        stdout: "",
        message: Some(
            "runtime error: type.field_value: field metadata belongs to 'app.Other', expected 'app.Holder'",
        ),
        markers: &[],
    },
    Case {
        name: "optional_return",
        source: include_str!("reflected_callable_sources/10_optional_return.jett"),
        stdout: "5\n",
        message: None,
        markers: &["source", "predicate"],
    },
    Case {
        name: "ready_refined_unit",
        source: include_str!("reflected_callable_sources/11_ready_refined_unit.jett"),
        stdout: "ready\n",
        message: None,
        markers: &["source", "predicate"],
    },
    Case {
        name: "pending_refined_unit",
        source: include_str!("reflected_callable_sources/12_pending_refined_unit.jett"),
        stdout: "",
        message: Some("runtime error: reflected callback return: expected a ready value"),
        markers: &["source"],
    },
    Case {
        name: "exact_pending_unit",
        source: include_str!("reflected_callable_sources/13_exact_pending_unit.jett"),
        stdout: "",
        message: None,
        markers: &["source"],
    },
    Case {
        name: "exact_proved_pending_unit",
        source: include_str!("reflected_callable_sources/14_exact_proved_pending_unit.jett"),
        stdout: "",
        message: None,
        markers: &["source", "predicate"],
    },
    Case {
        name: "ancestor_proved_pending_unit",
        source: include_str!("reflected_callable_sources/15_ancestor_proved_pending_unit.jett"),
        stdout: "",
        message: None,
        markers: &["source", "predicate", "higher"],
    },
    Case {
        name: "captured_pending_unit",
        source: include_str!("reflected_callable_sources/16_captured_pending_unit.jett"),
        stdout: "",
        message: Some("runtime error: reflected callback return: expected a ready value"),
        markers: &["source"],
    },
    Case {
        name: "optional_pending_unit",
        source: include_str!("reflected_callable_sources/17_optional_pending_unit.jett"),
        stdout: "",
        message: Some("runtime error: reflected callback return: expected a ready value"),
        markers: &["source"],
    },
    Case {
        name: "invalid_refined_unit",
        source: include_str!("reflected_callable_sources/18_invalid_refined_unit.jett"),
        stdout: "",
        message: Some("runtime error: refinement type constraint failed for 'app.RejectedUnit'"),
        markers: &["source", "rejected"],
    },
];

fn reference(case: &Case, source: &Path) -> Vec<jett_driver::DebugEvent> {
    let events = match (jett_driver::run_file_capture_outcome(source), case.message) {
        (Ok(output), None) => {
            assert_eq!(output.stdout, case.stdout, "{}", case.name);
            output.debug_events
        }
        (Err(error), Some(message)) => {
            assert_eq!(error.message, message, "{}", case.name);
            assert_eq!(error.output.stdout, case.stdout, "{}", case.name);
            error.output.debug_events
        }
        (actual, message) => panic!("{}: {actual:?}, expected {message:?}", case.name),
    };
    let expected = case
        .markers
        .iter()
        .map(|marker| format!("trace marker: string = {marker}"))
        .collect::<Vec<_>>();
    assert_eq!(debug_trace_lines(&events), expected, "{}", case.name);
    if matches!(
        case.name,
        "exact_pending_unit" | "exact_proved_pending_unit" | "ancestor_proved_pending_unit"
    ) {
        assert_eq!(
            debug_print_text(&events),
            "pending(pending(nothing))\n",
            "{} retains both existing pending layers without new validation",
            case.name
        );
    }
    if matches!(case.name, "original_wider" | "original_exact") {
        assert_eq!(
            jett_driver::render_debug_events(&events),
            "5\n",
            "{}",
            case.name
        );
    }
    events
}

#[test]
fn reflected_callable_sources_reference_lazy_results_and_selector_precedence() {
    for case in CASES {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("main.jett");
        fs::write(&source, case.source).unwrap();
        reference(case, &source);
    }
}

#[test]
fn reflected_callable_sources_emit_objects_and_reject_changed_source_and_predicate_bodies() {
    for case in CASES {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("main.jett");
        fs::write(&source, case.source).unwrap();
        let lowered = jett_driver::lower_file_for_backend_with_options(
            &source,
            jett_driver::BuildOptions { release: false },
        )
        .unwrap_or_else(|error| panic!("{}: {error}", case.name));
        let entry = lowered.program_entry.expect("checked ordinary main");
        let object = jett_codegen_cranelift::emit_host_program_object_with_options(
            &lowered.mir,
            &lowered.interner,
            entry,
            jett_codegen_cranelift::CodegenOptions { optimize: false },
        )
        .unwrap_or_else(|error| panic!("{}: {error}", case.name));
        assert!(!object.bytes.is_empty(), "{}", case.name);
        if case.name == "original_wider" {
            for predicate in [false, true] {
                let mut forged = lowered.mir.clone();
                let function = forged
                    .functions
                    .iter_mut()
                    .find(|function| {
                        if predicate {
                            function.identity.declaration.kind
                                == jett_hir::DeclarationKind::RefinementPredicate
                                && function.identity.declaration.namespace == "app"
                        } else {
                            function.identity.declaration.name == "five"
                                && function.identity.declaration.namespace == "app"
                        }
                    })
                    .expect("original source or exact predicate body");
                let returned = function
                    .blocks
                    .iter_mut()
                    .find_map(|block| match &mut block.terminator.kind {
                        jett_mir::TerminatorKind::Return(Some(value)) => Some(value),
                        _ => None,
                    })
                    .expect("original typed return");
                returned.kind = if predicate {
                    jett_hir::ExpressionKind::Bool(false)
                } else {
                    jett_hir::ExpressionKind::Int(6)
                };
                let outcome = jett_codegen_cranelift::emit_host_program_object_with_options(
                    &forged,
                    &lowered.interner,
                    entry,
                    jett_codegen_cranelift::CodegenOptions { optimize: false },
                );
                let error = match outcome {
                    Err(error) => error,
                    Ok(object) => panic!(
                        "public MIR changed {} body but emitted {} object bytes",
                        if predicate { "predicate" } else { "source" },
                        object.bytes.len()
                    ),
                };
                assert!(
                    matches!(&error, jett_codegen_cranelift::CodegenError::InvalidMir(errors)
                    if errors.iter().any(|error| error.message.contains("reflected callable"))),
                    "{error:?}"
                );
            }
        }
    }
}

#[test]
fn native_reflected_callable_sources_delete_source_in_both_normal_runtime_profiles() {
    let measured =
        std::env::var_os("JETT_REFLECTED_CALLABLE_NORMAL_ARCHIVES").map(std::path::PathBuf::from);
    let evidence =
        std::env::var_os("JETT_REFLECTED_CALLABLE_NATIVE_EVIDENCE").map(std::path::PathBuf::from);
    if let Some(directory) = &evidence {
        fs::create_dir(directory).expect("new native evidence directory");
    }
    for case in CASES {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("main.jett");
        fs::write(&source, case.source).unwrap();
        let events = reference(case, &source);
        let object = jett_driver::native::emit_host_program_object_for_file_with_options(
            &source,
            // Source/debug policy is independent of the runtime archive profile.
            jett_driver::BuildOptions { release: false },
        )
        .unwrap_or_else(|error| panic!("{}: {error}", case.name));
        let case_evidence = evidence.as_ref().map(|root| root.join(case.name));
        if let Some(path) = &case_evidence {
            fs::create_dir(path).unwrap();
            fs::write(path.join("source.snapshot"), case.source).unwrap();
            fs::write(path.join("object.obj"), object.bytes()).unwrap();
        }
        let mut executables = Vec::new();
        for runtime_release in [false, true] {
            let profile = if runtime_release { "release" } else { "debug" };
            let executable = directory
                .path()
                .join(format!("callback_{runtime_release}.exe"));
            let launcher = measured.as_ref().map(|root| {
                jett_driver::native::NativeLauncherBundle::from_manifest(
                    &root.join(format!("{profile}-bundle/launcher.json")),
                    runtime_release,
                )
                .expect("exact measured normal runtime profile manifest")
            });
            let launcher = launcher
                .as_ref()
                .unwrap_or_else(|| suite_options::launcher_for_options(runtime_release));
            jett_driver::native::link_host_object(&object, launcher, &executable)
                .unwrap_or_else(|error| panic!("{}: {error}", case.name));
            if let Some(path) = &case_evidence {
                fs::copy(&executable, path.join(format!("{profile}.exe"))).unwrap();
            }
            executables.push((executable, profile));
        }
        fs::remove_file(&source).unwrap();
        assert!(
            !source.exists(),
            "Source must be absent before either launch"
        );
        for (executable, profile) in executables {
            let output = run_bounded(&executable, directory.path());
            if let Some(path) = &case_evidence {
                fs::write(
                    path.join(format!("{profile}-observation.json")),
                    serde_json::to_vec_pretty(&serde_json::json!({
                        "case": case.name,
                        "runtime_profile": profile,
                        "source_build_release": false,
                        "source_deleted_before_launch": !source.exists(),
                        "object_target": object.target(),
                        "object_entry": object.program_entry().index(),
                        "status": output.status.code(),
                        "stdout": output.stdout,
                        "stderr": output.stderr,
                        "public_ordinary_context_retirement_observation": null,
                        "retirement_observation_available": false,
                    }))
                    .unwrap(),
                )
                .unwrap();
            }
            assert_eq!(
                output.status.code(),
                Some(if case.message.is_some() { 71 } else { 0 }),
                "{}: {output:?}",
                case.name
            );
            assert_eq!(output.stdout, case.stdout.as_bytes(), "{}", case.name);
            let mut stderr = jett_driver::render_debug_events(&events);
            if let Some(message) = case.message {
                stderr.push_str(message);
                stderr.push('\n');
            }
            assert_eq!(
                output.stderr,
                stderr.as_bytes(),
                "{}: {output:?}",
                case.name
            );
        }
    }
}

const PRODUCTION_CASES: &[Case] = &[
    Case {
        name: "production_named_lazy",
        source: include_str!("reflected_callable_sources/production_named_lazy.jett"),
        stdout: "stored\nsource\nfirst:5\nsource\nsecond:5\nsource\noriginal:5\n",
        message: None,
        markers: &[],
    },
    Case {
        name: "production_invalid_return",
        source: include_str!("reflected_callable_sources/production_invalid_return.jett"),
        stdout: "stored\nsource\n",
        message: Some("runtime error: refinement type constraint failed for 'app.Positive'"),
        markers: &[],
    },
    Case {
        name: "production_wrong_selector_owner",
        source: include_str!("reflected_callable_sources/production_wrong_selector_owner.jett"),
        stdout: "before\n",
        message: Some(
            "runtime error: type.field_value: field metadata belongs to 'app.Other', expected 'app.Holder'",
        ),
        markers: &[],
    },
];

#[test]
fn native_reflected_callable_production_output_matches_with_optimized_objects_without_source() {
    let measured =
        std::env::var_os("JETT_REFLECTED_CALLABLE_NORMAL_ARCHIVES").map(std::path::PathBuf::from);
    let mut object_count = 0;
    let mut execution_count = 0;
    for case in PRODUCTION_CASES {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("main.jett");
        fs::write(&source, case.source).unwrap();
        let events = reference(case, &source);
        assert!(events.is_empty(), "{} uses canonical Stdout", case.name);
        let mut executables = Vec::new();
        for optimize in [false, true] {
            let object = jett_driver::native::emit_host_program_object_for_file_with_options(
                &source,
                // Production Source has no debug operations. This option reaches
                // CodegenOptions.optimize independently of the runtime profile.
                jett_driver::BuildOptions { release: optimize },
            )
            .unwrap_or_else(|error| panic!("{} optimize={optimize}: {error}", case.name));
            object_count += 1;
            for runtime_release in [false, true] {
                let profile = if runtime_release { "release" } else { "debug" };
                let executable = directory.path().join(format!(
                    "callback_production_{optimize}_{runtime_release}.exe"
                ));
                let measured_launcher = measured.as_ref().map(|root| {
                    jett_driver::native::NativeLauncherBundle::from_manifest(
                        &root.join(format!("{profile}-bundle/launcher.json")),
                        runtime_release,
                    )
                    .expect("exact measured normal runtime profile manifest")
                });
                let launcher = measured_launcher
                    .as_ref()
                    .unwrap_or_else(|| suite_options::launcher_for_options(runtime_release));
                jett_driver::native::link_host_object(&object, launcher, &executable)
                    .unwrap_or_else(|error| {
                        panic!(
                            "{} optimize={optimize} runtime={profile}: {error}",
                            case.name
                        )
                    });
                executables.push((executable, optimize, profile));
            }
        }
        fs::remove_file(&source).unwrap();
        assert!(
            !source.exists(),
            "Source must be absent before every launch"
        );
        for (executable, optimize, profile) in executables {
            let output = run_bounded(&executable, directory.path());
            assert_eq!(
                output.status.code(),
                Some(if case.message.is_some() { 71 } else { 0 }),
                "{} optimize={optimize} runtime={profile}: {output:?}",
                case.name
            );
            assert_eq!(
                output.stdout,
                case.stdout.as_bytes(),
                "{} optimize={optimize} runtime={profile}: {output:?}",
                case.name
            );
            let stderr = case
                .message
                .map(|message| format!("{message}\n"))
                .unwrap_or_default();
            assert_eq!(
                output.stderr,
                stderr.as_bytes(),
                "{} optimize={optimize} runtime={profile}: {output:?}",
                case.name
            );
            execution_count += 1;
        }
    }
    assert_eq!(object_count, PRODUCTION_CASES.len() * 2);
    assert_eq!(execution_count, PRODUCTION_CASES.len() * 4);
    println!(
        "reflected callable production controls: {} Sources, {object_count} objects, {execution_count} Source-deleted executions",
        PRODUCTION_CASES.len()
    );
}
