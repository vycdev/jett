//! Whole unchanged Source17 canonical suites; available observations only.
//! Deliberately does not award full acceptance without missing runtime telemetry.
use super::*;
use jett_comptime::verify::collect_checked_property_cases_with_checked_values;
use jett_hir::{ExpressionKind, StatementKind};
use serde_json::{Value as Json, json};

const SOURCE17: &str = include_str!(
    "../../../../jett_comptime/src/resource_execution/fixtures/17_absent_aggregate_shapes.jett"
);
const SUPPORT: &str = include_str!("../resource/resource_probe.jett");
const SOURCE17_SHA256: &str = "c8e74848356a5b3d09ab6659318b1e1292718d34443f7f37222bafd6f3eed0ff";
const NORMAL_RECEIPT_ENV: &str = "JETT_SOURCE17_NORMAL_ARCHIVE_RECEIPT_V1";
const NORMAL_RECEIPT_SHA256: &str =
    "2b2b9cd529c451c216072dc2b563253149f409bd3ef9ea867a1a86da1c637069";
const EVIDENCE_ENV: &str = "JETT_SOURCE17_SUITE_EVIDENCE_V1";
const UNMEASURED: &[&str] = &[
    "total attempted provider acquisitions, including rejected attempts before event recording",
    "successful runtime Property context set/call/clear history and complete trial order",
    "individual ordinary storage-kind live counts beyond aggregate ordinary_empty",
    "individual private artifact-retainer counts beyond checked-program and caller input-Arc release",
    "normal-launcher custody counts and same-context reentry",
];

#[derive(Clone, Copy)]
enum Purpose {
    Verify,
    Property,
}
impl Purpose {
    fn name(self) -> &'static str {
        match self {
            Self::Verify => "verify",
            Self::Property => "property",
        }
    }
}
struct Prepared {
    purpose: Purpose,
    release: bool,
    object: jett_codegen_cranelift::ObjectArtifact,
    entry: jett_hir::FunctionId,
    expected: Json,
}

fn checked_suites(project: &Path, support: &Path, release: bool) -> Arc<CheckedResourceProgram> {
    let source = fs::read(project).unwrap();
    let support_source = fs::read_to_string(support).unwrap();
    assert_eq!(
        source,
        SOURCE17.as_bytes(),
        "whole Source17 without a main wrapper"
    );
    assert_eq!(digest(&source), SOURCE17_SHA256);
    assert_eq!(support_source, SUPPORT);
    let primary = FileId::new(0);
    let support_file = FileId::new(10_000);
    let mut parsed = parse(&support_source, support_file);
    let source = parse(SOURCE17, primary);
    assert!(parsed.errors.is_empty() && source.errors.is_empty());
    assert!(
        !source.module.items.iter().any(|item| {
            matches!(item, Item::Function(function) if function.name.name == "main")
        })
    );
    let resources = parsed
        .module
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Resource(resource) => Some(resource.name.span),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [resource] = resources.as_slice() else {
        panic!("unique original trusted Resource declaration");
    };
    let catalog = [
        ("kernel_create", ResourceKernelRecipe::NetworkFactory),
        ("kernel_borrow", ResourceKernelRecipe::NetworkBorrow),
        ("kernel_close", ResourceKernelRecipe::Finalize),
    ]
    .into_iter()
    .map(|(member, recipe)| ResourceKernelSpec {
        resource_declaration: *resource,
        member: member.into(),
        recipe,
    })
    .collect::<Vec<_>>();
    parsed.module.items.extend(source.module.items);
    Arc::new(
        CheckedResourceProgram::prepare(
            parsed,
            HashMap::from([
                (primary, SourceOrigin::Project),
                (support_file, SourceOrigin::Stdlib),
            ]),
            &catalog,
            CheckOptions { release },
        )
        .unwrap(),
    )
}

fn expected_contexts(
    program: &jett_hir::Program,
    plan: &jett_hir::CheckedNativePropertyPlan,
) -> Json {
    let contexts = program.functions[plan.entry().index() as usize]
        .body
        .statements
        .iter()
        .filter_map(|statement| match &statement.kind {
            StatementKind::Expression(expression) => match &expression.kind {
                ExpressionKind::PropertyCaseContext(context) => Some(context.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(contexts.len(), plan.trials().len() * 2);
    let mut expected = Vec::new();
    for (ordinal, (trial, pair)) in plan
        .trials()
        .iter()
        .zip(contexts.chunks_exact(2))
        .enumerate()
    {
        let one_based = u64::try_from(trial.case().iteration).unwrap() + 1;
        assert_eq!(trial.name(), "absent_aggregate_property");
        assert_eq!(trial.case().iteration, ordinal);
        assert_eq!(trial.given_names(), &["marker".to_owned()]);
        assert_eq!(
            pair[0],
            Some(jett_hir::NativePropertyCase {
                name: trial.name().into(),
                trial: one_based,
            })
        );
        assert_eq!(pair[1], None);
        expected.push(json!({
            "ordinal_zero_based": ordinal,
            "function": trial.function().index(),
            "property_span": format!("{:?}", trial.case().property_span),
            "name_span": format!("{:?}", trial.span()),
            "given_names": trial.given_names(),
            "checked_arguments": format!("{:?}", trial.case().arguments),
            "context_set_bytes_utf8": format!("property '{}' trial {}: ", trial.name(), one_based),
            "context_clear_after_call": true,
        }));
    }
    json!({
        "expected_only_not_runtime_observation": true,
        "trial_count": plan.trials().len(),
        "trials_in_checked_order": expected,
    })
}

fn prepare(project: &Path, support: &Path, release: bool) -> Vec<Prepared> {
    let checked = checked_suites(project, support, release);
    let checked_weak = Arc::downgrade(&checked);
    let evaluation = jett_driver::evaluate_checked_resource_required_values(&checked);
    assert!(
        evaluation.diagnostics.is_empty(),
        "{:?}",
        evaluation.diagnostics
    );
    assert!(evaluation.debug_events.is_empty());
    let values = Arc::new(evaluation.values);
    let values_weak = Arc::downgrade(&values);
    values.checked_required_values(&checked).unwrap();
    assert!(!values.is_empty());
    assert!(
        values
            .values()
            .all(|value| matches!(value, jett_comptime::Value::Int64(17)))
    );
    let mut hir = jett_hir::lower_checked_resource_program_with_test_bodies(&checked).unwrap();
    jett_driver::bake_checked_resource_values(&mut hir, &checked, &values).unwrap();
    for name in [
        "empty_tokens",
        "absent_tokens",
        "empty_map",
        "absent_map",
        "empty_result_tokens",
        "failed_result_tokens",
        "absent_envelope",
        "absent_box",
        "empty_choice",
        "absent_choice",
        "absent_state",
        "absent_exact_state",
        "refined_shape_contract",
        "absent_required_controls",
    ] {
        assert!(
            hir.functions.iter().any(|function| {
                function.identity.declaration.origin == SourceOrigin::Project
                    && function.identity.declaration.namespace == "app"
                    && function.identity.declaration.name == name
            }),
            "whole unused/exported Source declaration retained: {name}"
        );
    }
    let manifest = hir.resource_manifest.clone();
    let originals = hir
        .functions
        .iter()
        .map(|function| (function.id, function.identity.clone(), function.span))
        .collect::<Vec<_>>();
    let verify = jett_hir::append_checked_native_verify_suite(&mut hir, &checked, FileId::new(0))
        .unwrap()
        .unwrap();
    let cases = collect_checked_property_cases_with_checked_values(checked.clone(), values.clone())
        .unwrap();
    let plan =
        jett_hir::append_checked_native_property_suite(&mut hir, &checked, FileId::new(0), &cases)
            .unwrap()
            .unwrap();
    assert!(values.same_checked_cache(cases.required_values()));
    assert_eq!(plan.trials().len(), cases.cases().len());
    // This Source17 example uses the current checked default; never truncate a plan to 100.
    assert_eq!(
        plan.trials().len(),
        jett_comptime::verify::PROPERTY_DEFAULT_ITERATIONS
    );
    for (trial, original) in plan.trials().iter().zip(cases.cases()) {
        assert_eq!(trial.case().property_span, original.property_span);
        assert_eq!(trial.case().iteration, original.iteration);
        assert_eq!(trial.case().arguments.len(), original.arguments.len());
        assert!(
            trial
                .case()
                .arguments
                .iter()
                .zip(&original.arguments)
                .all(|(left, right)| left.same_checked_snapshot(right))
        );
    }
    let property_expected = expected_contexts(&hir, &plan);
    let verify_targets = checked
        .module()
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Verify(verify) if verify.span.file == FileId::new(0) => Some(json!({
                "name": verify.name.name,
                "span": format!("{:?}", verify.span),
            })),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(verify_targets.len(), 1);
    let verify_expected = json!({
        "expected_only_not_runtime_observation": true,
        "original_targets_in_checked_order": verify_targets,
    });
    jett_hir::complete_value_conversions(&mut hir, &checked.checked().interner).unwrap();
    jett_hir::validate(&hir).unwrap();
    hir.resource_source
        .validate_current(&hir, &checked.checked().interner)
        .unwrap();
    assert_eq!(hir.resource_manifest, manifest);
    drop(manifest);
    assert_eq!(expected_contexts(&hir, &plan), property_expected);
    for (id, identity, span) in originals {
        let function = &hir.functions[id.index() as usize];
        assert_eq!(function.id, id);
        assert_eq!(function.identity, identity);
        assert_eq!(function.span, span);
    }
    let mir = jett_mir::lower(&hir, &checked.checked().interner).unwrap();
    jett_mir::validate(&mir).unwrap();
    let output = [
        (Purpose::Verify, verify, verify_expected),
        (Purpose::Property, plan.entry(), property_expected),
    ]
    .into_iter()
    .map(|(purpose, entry, expected)| {
        let object = jett_codegen_cranelift::emit_host_program_object_with_options(
            &mir,
            &checked.checked().interner,
            entry,
            jett_codegen_cranelift::CodegenOptions { optimize: release },
        )
        .unwrap();
        assert!(!object.bytes.is_empty());
        assert_eq!(object.target, jett_driver::native::host_target());
        assert!(
            object
                .symbols
                .iter()
                .any(|name| name == "jett_aot_resource_v1_manifest")
        );
        assert!(
            object
                .symbols
                .iter()
                .any(|name| name == jett_codegen_cranelift::JETT_AOT_ENTRY_SYMBOL_V1)
        );
        Prepared {
            purpose,
            release,
            object,
            entry,
            expected,
        }
    })
    .collect::<Vec<_>>();
    // Real lifetime observation for these roots, without exposing opaque plan internals.
    // It does not enumerate all private artifact strong counts.
    drop(mir);
    drop(hir);
    drop(plan);
    drop(cases);
    drop(values);
    drop(checked);
    assert!(
        checked_weak.upgrade().is_none(),
        "no retained checked-root cycle"
    );
    assert!(
        values_weak.upgrade().is_none(),
        "required-value input Arc released"
    );
    output
}

fn normal_archive(release: bool, private: &Archive) -> Archive {
    let path = PathBuf::from(
        std::env::var_os(NORMAL_RECEIPT_ENV)
            .expect("Root must supply the exact measured normal receipt; no runtime builds"),
    );
    assert!(path.is_absolute());
    let bytes = fs::read(path).unwrap();
    assert!(bytes.len() <= 1024 * 1024);
    assert_eq!(digest(&bytes), NORMAL_RECEIPT_SHA256);
    let receipt: Json = serde_json::from_slice(&bytes).unwrap();
    let profile = if release { "release" } else { "debug" };
    let rows = receipt["archives"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| field(row, "profile") == profile && field(row, "kind") == "launcher")
        .collect::<Vec<_>>();
    let [row] = rows.as_slice() else {
        panic!("unique measured normal launcher");
    };
    assert_eq!(row["private_exports"].as_u64(), Some(0));
    assert_eq!(row["selected_package_private_cfgs"], json!([]));
    assert_eq!(row["main_definitions"].as_u64(), Some(1));
    assert_eq!(field(row, "target"), private.target);
    assert_eq!(field(row, "crt_mode"), "static");
    let measured = measured_file(&row["archive"], "path", "sha256");
    let manifest = measured_file(&row["manifest"], "path", "sha256");
    let supplied = super::super::measured_launcher::supplied(release)
        .expect("both normal Debug/Release manifests must already be supplied");
    let selected =
        jett_driver::native::NativeLauncherBundle::from_manifest(&manifest, release).unwrap();
    assert_eq!(
        supplied, &selected,
        "supplied manifest must select the pinned measured bundle"
    );
    assert_eq!(
        fs::canonicalize(&selected.archive_path).unwrap(),
        fs::canonicalize(&measured).unwrap()
    );
    assert_eq!(selected.runtime_abi_version, 1);
    assert_eq!(
        selected.crt_mode,
        jett_driver::native::NativeCrtMode::Static
    );
    let arguments = selected
        .native_library_args
        .iter()
        .map(|arg| arg.to_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        json!(arguments),
        row["selected_manifest_native_library_args"]
    );
    let build_environment = &row["build"]["environment_overrides"];
    assert_eq!(
        fs::canonicalize(field(
            build_environment,
            "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER"
        ))
        .unwrap(),
        fs::canonicalize(&private.linker).unwrap(),
    );
    assert_eq!(
        Some(OsString::from(field(build_environment, "LIB"))),
        private.sdk_lib,
        "same measured native linker environment",
    );
    Archive {
        path: measured,
        archive_sha256: field(&row["archive"], "sha256").into(),
        target: selected.target,
        profile: profile.into(),
        linker: private.linker.clone(),
        linker_sha256: private.linker_sha256.clone(),
        libraries: selected.native_library_args,
        sdk_lib: private.sdk_lib.clone(),
    }
}

fn empty_two_entry_script() -> String {
    let mut bytes = b"JTRST001".to_vec();
    bytes.extend(1u32.to_le_bytes());
    bytes.extend(0u32.to_le_bytes());
    bytes.extend(32u64.to_le_bytes());
    bytes.extend(2u32.to_le_bytes());
    bytes.extend(0u32.to_le_bytes());
    assert_eq!(bytes.len(), 32);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn observe_private(observed: &report::Report) -> Json {
    assert_eq!(
        observed.attempts.len(),
        2,
        "same context/grant/provider, two entry calls"
    );
    let mut previous = 0;
    let attempts = observed
        .attempts
        .iter()
        .map(|attempt| {
            assert!(attempt.completion.attempt > previous);
            previous = attempt.completion.attempt;
            assert_eq!(attempt.completion.body_status, 0);
            assert_eq!(attempt.completion.cleanup_status, 0);
            assert_eq!(
                attempt.completion.selected_channel,
                report::FailureChannel::Clean
            );
            assert!(attempt.ordinary_message.is_empty() && attempt.resource_message.is_empty());
            assert!(attempt.counts.resources_retired());
            assert!(attempt.counts.ordinary_empty);
            assert_eq!(attempt.event_sequence_end, 0);
            json!({
                "attempt": attempt.completion.attempt,
                "body_status": attempt.completion.body_status,
                "cleanup_status": attempt.completion.cleanup_status,
                "channel": format!("{:?}", attempt.completion.selected_channel),
                "resource_owners": attempt.counts.owners,
                "resource_loans": attempt.counts.loans,
                "resource_frames": attempt.counts.frames,
                "registry_entries": attempt.counts.registry_entries,
                "owner_handles": attempt.counts.owner_handles,
                "loan_handles": attempt.counts.loan_handles,
                "frame_handles": attempt.counts.frame_handles,
                "provisional_returns": attempt.counts.provisional_returns,
                "ordinary_storage_empty": attempt.counts.ordinary_empty,
                "observed_before_context_destruction": true,
                "event_sequence_end": attempt.event_sequence_end,
                "total_provider_acquisition_attempts": null,
                "successful_property_context_history": null,
            })
        })
        .collect::<Vec<_>>();
    assert!(observed.events.is_empty());
    assert_eq!(observed.script_remaining, 0);
    assert!(observed.observations_ready_for_acceptance());
    assert!(observed.all_attempts_succeeded());
    assert_eq!(
        (
            observed.created,
            observed.destroy_attempts,
            observed.destroy_status,
            observed.write_status
        ),
        (true, 1, 0, 0)
    );
    json!({
        "attempts": attempts,
        "provider_events": [],
        "script_remaining": observed.script_remaining,
        "created": observed.created,
        "destroy_attempts": observed.destroy_attempts,
        "destroy_status": observed.destroy_status,
        "report_write_status": observed.write_status,
        "same_context_reentry_observed_by_matched_launcher": true,
        "zero_total_provider_acquisition_attempts_measured": false,
    })
}

#[test]
#[ignore = "requires pinned measured normal/private archives; records available observations only"]
fn source17_whole_canonical_suites_record_available_native_observations() {
    assert!(
        cfg!(windows),
        "candidate pins existing measured MSVC archives"
    );
    assert_eq!(digest(SOURCE17.as_bytes()), SOURCE17_SHA256);
    let evidence =
        PathBuf::from(std::env::var_os(EVIDENCE_ENV).expect("fresh absolute evidence directory"));
    assert!(evidence.is_absolute());
    fs::create_dir(&evidence).expect("evidence directory must not already exist");
    let directory = tempfile::tempdir().unwrap();
    let project_dir = directory.path().join("project");
    let support_dir = directory.path().join("synthetic-stdlib");
    fs::create_dir(&project_dir).unwrap();
    fs::create_dir(&support_dir).unwrap();
    let project = project_dir.join("source17.jett");
    let support = support_dir.join("resource_probe.jett");
    fs::write(&project, SOURCE17).unwrap();
    fs::write(&support, SUPPORT).unwrap();
    let mut prepared = Vec::new();
    for release in [false, true] {
        prepared.extend(prepare(&project, &support, release));
    }
    assert_eq!(prepared.len(), 4);
    fs::remove_dir_all(&project_dir).unwrap();
    fs::remove_dir_all(&support_dir).unwrap();
    assert!(!project.exists() && !support.exists());
    assert!(!project_dir.exists() && !support_dir.exists());
    let mut launches = 0;
    let mut private_attempts = 0;
    for prepared in prepared {
        let profile = if prepared.release { "release" } else { "debug" };
        let matched = archive(prepared.release);
        let normal = normal_archive(prepared.release, &matched);
        for (private, runtime) in [(false, &normal), (true, &matched)] {
            let kind = if private { "private" } else { "normal" };
            let case = evidence.join(format!("{}-{profile}-{kind}", prepared.purpose.name()));
            fs::create_dir(&case).unwrap();
            let execution = tempfile::tempdir().unwrap();
            let object = execution.path().join("suite.obj");
            let binary = execution.path().join("suite.exe");
            fs::write(&object, &prepared.object.bytes).unwrap();
            let object_sha256 = digest(&fs::read(&object).unwrap());
            assert_eq!(object_sha256, digest(&prepared.object.bytes));
            assert_eq!(prepared.object.target, runtime.target);
            link(runtime, &object, &binary, execution.path());
            fs::remove_file(&object).unwrap();
            assert!(!object.exists());
            let executable_sha256 = digest(&fs::read(&binary).unwrap());
            assert!(!project_dir.exists() && !support_dir.exists());
            let script = empty_two_entry_script();
            let output = super::super::run_bounded_with_env(
                &binary,
                execution.path(),
                if private {
                    Some(("JETT_RESOURCE_NATIVE_TEST_SCRIPT_V1", script.as_str()))
                } else {
                    None
                },
            );
            // The existing bounded runner reaps the child before returning.
            fs::remove_file(&binary).unwrap();
            assert!(!binary.exists());
            let pdb = binary.with_extension("pdb");
            if pdb.exists() {
                fs::remove_file(&pdb).unwrap();
            }
            let observation = if private {
                let report_path = execution.path().join("resource-native-report-v1.bin");
                assert!(report_path.metadata().unwrap().len() <= 16 * 1024 * 1024);
                let bytes = fs::read(&report_path).unwrap();
                fs::write(case.join("resource-native-report-v1.bin"), &bytes).unwrap();
                let observed = report::decode(&bytes).unwrap();
                let available = observe_private(&observed);
                private_attempts += observed.attempts.len();
                available
            } else {
                json!({
                    "live_custody_observations": null,
                    "same_context_reentry": null,
                    "normal_launcher_observability_available": false,
                })
            };
            fs::write(case.join("stdout.bin"), &output.stdout).unwrap();
            fs::write(case.join("stderr.bin"), &output.stderr).unwrap();
            fs::write(
                case.join("observation.json"),
                serde_json::to_vec_pretty(&json!({
                    "schema": "jett-source17-suite-available-observations-v1",
                    "full_acceptance": false,
                    "remaining_instrumentation_obligations": UNMEASURED,
                    "whole_source_sha256": SOURCE17_SHA256,
                    "source_deleted_before_launch": !project.exists() && !support.exists(),
                    "source_build_profile": profile,
                    "runtime_profile": runtime.profile,
                    "runtime_kind": kind,
                    "runtime_archive_sha256": runtime.archive_sha256,
                    "entry": prepared.entry.index(),
                    "purpose": prepared.purpose.name(),
                    "object_sha256": object_sha256,
                    "object_deleted_before_launch": !object.exists(),
                    "executable_sha256": executable_sha256,
                    "child_reaped_executable_and_pdb_removed": !binary.exists() && !pdb.exists(),
                    "compiler_checked_program_weak_root_released": true,
                    "required_value_input_arc_released": true,
                    "opaque_successful_cache_individual_weak_observation_available": false,
                    "expected_checked_plan": prepared.expected,
                    "runtime_observations": observation,
                    "status": output.status.code(),
                }))
                .unwrap(),
            )
            .unwrap();
            assert_eq!(
                output.status.code(),
                Some(0),
                "{profile} {kind}: {output:?}"
            );
            assert!(
                output.stdout.is_empty() && output.stderr.is_empty(),
                "{output:?}"
            );
            launches += 1;
        }
    }
    assert_eq!(launches, 8);
    assert_eq!(private_attempts, 8);
    fs::write(evidence.join("summary.json"), serde_json::to_vec_pretty(&json!({
        "objects": 4, "processes": launches, "private_entry_attempts": private_attempts,
        "full_acceptance": false,
        "remaining_instrumentation_obligations": UNMEASURED,
        "note": "Successful local test means available measurements passed; it cannot close full acceptance.",
    })).unwrap()).unwrap();
}
