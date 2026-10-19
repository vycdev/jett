//! Whole unchanged Resource canonical suite fixtures; available observations only.
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
    "be980ae18006737304ae69af4d254f78cad15c51fe8e6226c6a2b9ca9cf4b6b4";
const EVIDENCE_ENV: &str = "JETT_SOURCE17_SUITE_EVIDENCE_V1";
const UNMEASURED: &[&str] = &[
    "individual ordinary storage-kind live counts beyond aggregate ordinary_empty",
    "individual private artifact-retainer counts beyond checked-program and caller input-Arc release",
    "normal-launcher custody counts and same-context reentry",
];

const PURE_REPLACEMENTS: &str = include_str!(
    "../../../../jett_codegen_cranelift/src/emit/resource_execution/replacement/pure_replacement_suites.jett"
);
const PURE_REPLACEMENTS_SHA256: &str =
    "0801c8b24add5deb9ea7071f3fc2ca5a4573dbe00a62df38d1a474fadc3774ab";

#[derive(Clone, Copy)]
enum Fixture {
    Source17,
    PureReplacements,
}
impl Fixture {
    fn source(self) -> &'static str {
        match self {
            Self::Source17 => SOURCE17,
            Self::PureReplacements => PURE_REPLACEMENTS,
        }
    }
    fn sha256(self) -> &'static str {
        match self {
            Self::Source17 => SOURCE17_SHA256,
            Self::PureReplacements => PURE_REPLACEMENTS_SHA256,
        }
    }
    fn file_name(self) -> &'static str {
        match self {
            Self::Source17 => "source17.jett",
            Self::PureReplacements => "pure_replacement_suites.jett",
        }
    }
    fn evidence_env(self) -> &'static str {
        match self {
            Self::Source17 => EVIDENCE_ENV,
            Self::PureReplacements => "JETT_PURE_REPLACEMENT_SUITE_EVIDENCE_V1",
        }
    }
    fn evidence_schema(self) -> &'static str {
        match self {
            Self::Source17 => "jett-source17-suite-available-observations-v1",
            Self::PureReplacements => "jett-pure-replacement-suite-available-observations-v1",
        }
    }
    fn property_name(self) -> &'static str {
        match self {
            Self::Source17 => "absent_aggregate_property",
            Self::PureReplacements => "pure_replacements_trials",
        }
    }
    fn given_name(self) -> &'static str {
        match self {
            Self::Source17 => "marker",
            Self::PureReplacements => "input",
        }
    }
    fn main_count(self) -> usize {
        match self {
            Self::Source17 => 0,
            Self::PureReplacements => 1,
        }
    }
    fn retained_functions(self) -> &'static [&'static str] {
        match self {
            Self::Source17 => &[
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
            ],
            Self::PureReplacements => {
                &["optional_marker", "string_marker", "integer_marker", "main"]
            }
        }
    }
}

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

fn checked_suites(
    fixture: Fixture,
    project: &Path,
    support: &Path,
    release: bool,
) -> Arc<CheckedResourceProgram> {
    let source = fs::read(project).unwrap();
    let support_source = fs::read_to_string(support).unwrap();
    assert_eq!(
        source,
        fixture.source().as_bytes(),
        "whole unchanged original Source without a main wrapper"
    );
    assert_eq!(digest(&source), fixture.sha256());
    assert_eq!(support_source, SUPPORT);
    let primary = FileId::new(0);
    let support_file = FileId::new(10_000);
    let mut parsed = parse(&support_source, support_file);
    let source = parse(fixture.source(), primary);
    assert!(parsed.errors.is_empty() && source.errors.is_empty());
    assert_eq!(
        source
            .module
            .items
            .iter()
            .filter(|item| {
                matches!(item, Item::Function(function) if function.name.name == "main")
            })
            .count(),
        fixture.main_count(),
        "retain the original Source main without using it as a suite wrapper"
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
    fixture: Fixture,
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
        assert_eq!(trial.name(), fixture.property_name());
        assert_eq!(trial.case().iteration, ordinal);
        assert_eq!(trial.given_names(), &[fixture.given_name().to_owned()]);
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
            "checked_argument_int64_bits": trial.case().arguments.iter().map(|value| match value {
                jett_comptime::Value::Int64(value) => *value as u64,
                _ => panic!("checked given value must retain its int64 domain"),
            }).collect::<Vec<_>>(),
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

fn prepare(fixture: Fixture, project: &Path, support: &Path, release: bool) -> Vec<Prepared> {
    let checked = checked_suites(fixture, project, support, release);
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
    match fixture {
        Fixture::Source17 => assert!(
            values
                .values()
                .all(|value| matches!(value, jett_comptime::Value::Int64(17)))
        ),
        Fixture::PureReplacements => {
            // Same original-span required-value oracle as the existing object test.
            assert_eq!(values.len(), 6);
            let mut constants = 0;
            for item in &checked.module().items {
                let Item::VarDecl(declaration) = item else {
                    continue;
                };
                if declaration.name.span.file != FileId::new(0) {
                    continue;
                }
                let expected = match declaration.name.name.as_str() {
                    "baked_optional" => jett_comptime::Value::Int64(7),
                    "baked_string" => jett_comptime::Value::String("after".into()),
                    "baked_integer" => jett_comptime::Value::Int64(23),
                    name => panic!("unexpected original Source constant: {name}"),
                };
                let jett_parser::ast::Expr::Comptime(_, span) = &declaration.value else {
                    panic!("original required initializer");
                };
                assert_eq!(
                    values.get(*span, &jett_comptime::ComptimeContext::default()),
                    Some(&expected)
                );
                constants += 1;
            }
            assert_eq!(constants, 3);
        }
    }
    let mut hir = jett_hir::lower_checked_resource_program_with_test_bodies(&checked).unwrap();
    jett_driver::bake_checked_resource_values(&mut hir, &checked, &values).unwrap();
    for &name in fixture.retained_functions() {
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
    // Each unchanged fixture uses the checked default; never truncate a plan to 100.
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
    let property_expected = expected_contexts(fixture, &hir, &plan);
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
    assert_eq!(expected_contexts(fixture, &hir, &plan), property_expected);
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

fn acquisition_counters(counts: &report::Counters) -> Json {
    json!({
        "hook_prepare_ingress": counts.values[0],
        "acquire_hook_prepare_ingress": counts.values[1],
        "factory_prepare_ingress": counts.values[2],
        "factory_commit_ingress": counts.values[3],
        "provider_construct_dispatch": counts.values[4],
    })
}

fn observe_context_history(observed: &report::Report, purpose: Purpose, expected: &Json) -> Json {
    let telemetry = &observed.observations;
    assert_eq!(
        telemetry.flags, 0,
        "incomplete private telemetry is never acceptance"
    );
    assert_eq!(
        telemetry.counters.values, [0; 5],
        "measured dispatches, not provider-event inference"
    );
    assert_eq!(telemetry.attempts.len(), observed.attempts.len());
    assert!(
        telemetry
            .history
            .iter()
            .all(|event| matches!(event.channel, report::Channel::Attempt(_))),
        "these suites have no legacy/session context mutations"
    );
    let mut attempts = Vec::new();
    for (ordinal, (boundary, attempt)) in telemetry
        .attempts
        .iter()
        .zip(&observed.attempts)
        .enumerate()
    {
        assert_eq!(boundary.identity.id, attempt.completion.attempt);
        assert_eq!(boundary.identity.ordinal as usize, ordinal);
        let end = boundary.end.as_ref().expect("known genuine sealed attempt");
        assert_eq!(boundary.begin.counters.values, [0; 5]);
        assert_eq!(end.counters.values, [0; 5]);
        let events = telemetry
            .history
            .iter()
            .filter(|event| event.channel == report::Channel::Attempt(boundary.identity))
            .collect::<Vec<_>>();
        assert_eq!(
            events.first().map(|event| event.sequence),
            boundary.begin.sequence
        );
        assert_eq!(events.last().map(|event| event.sequence), end.sequence);
        assert!(matches!(
            events.first().unwrap().kind,
            report::HistoryKind::Begin
        ));
        assert!(matches!(
            events.last().unwrap().kind,
            report::HistoryKind::End
        ));
        assert_eq!(events.first().unwrap().context, None);
        assert_eq!(events.last().unwrap().context, None);
        let mut trials = Vec::new();
        match purpose {
            Purpose::Verify => assert_eq!(
                events.len(),
                2,
                "Verify must have no Property mutations/calls"
            ),
            Purpose::Property => {
                let expected_trials = expected["trials_in_checked_order"].as_array().unwrap();
                assert_eq!(
                    expected_trials.len() as u64,
                    expected["trial_count"].as_u64().unwrap()
                );
                let mut cursor = 1;
                for trial in expected_trials {
                    let bytes = trial["context_set_bytes_utf8"].as_str().unwrap().as_bytes();
                    let target = u32::try_from(trial["function"].as_u64().unwrap()).unwrap();
                    let arguments = trial["checked_argument_int64_bits"].as_array().unwrap();
                    let set = events
                        .get(cursor)
                        .expect("every checked trial has a runtime Set");
                    assert!(matches!(set.kind, report::HistoryKind::Set));
                    assert_eq!(set.context.as_deref(), Some(bytes));
                    cursor += 1;
                    let entered = events.get(cursor).expect("real target prologue after Set");
                    assert_eq!(entered.context.as_deref(), Some(bytes));
                    let report::HistoryKind::CallEnter { call, parameters } = &entered.kind else {
                        panic!("expected actual target entry")
                    };
                    assert_eq!(call.function, target);
                    assert_eq!(parameters.len(), arguments.len());
                    for (ordinal, (parameter, expected_bits)) in
                        parameters.iter().zip(arguments).enumerate()
                    {
                        assert_eq!(parameter.parameter as usize, ordinal);
                        assert_eq!(
                            parameter.value,
                            report::ParameterValue::Int64 {
                                bits: expected_bits.as_u64().unwrap()
                            },
                            "actual installed marker bits must match the checked generated trial"
                        );
                    }
                    let target_call = *call;
                    let entered_sequence = entered.sequence;
                    cursor += 1;
                    let mut complete = None;
                    let mut nested = 0;
                    loop {
                        let event = events
                            .get(cursor)
                            .expect("target completion and Clear must be present");
                        if matches!(event.kind, report::HistoryKind::Clear) {
                            break;
                        }
                        assert_eq!(event.context.as_deref(), Some(bytes));
                        assert!(
                            complete.is_none(),
                            "no additional call occurs after target completion before Clear"
                        );
                        match &event.kind {
                            report::HistoryKind::CallEnter { call, .. } => {
                                assert_ne!(
                                    call.function, target,
                                    "extra Property target occurrence"
                                );
                                nested += 1;
                            }
                            report::HistoryKind::CallComplete {
                                call,
                                entered_sequence: actual_entry,
                                body_status,
                                failure_status,
                            } => {
                                assert_eq!(*body_status, 0);
                                assert_eq!(*failure_status, None);
                                if *call == target_call {
                                    assert_eq!(*actual_entry, Some(entered_sequence));
                                    complete = Some(event.sequence);
                                }
                            }
                            _ => panic!("unexpected mutation/boundary within checked trial"),
                        }
                        cursor += 1;
                    }
                    let completed = complete.expect("genuine successful target completion");
                    let cleared = events[cursor];
                    assert_eq!(cleared.context, None);
                    trials.push(json!({
                        "set_sequence": set.sequence, "entered_sequence": entered_sequence,
                        "completed_sequence": completed, "clear_sequence": cleared.sequence,
                        "function": target_call.function, "signature": target_call.signature,
                        "template": target_call.template, "operation": target_call.operation,
                        "call": target_call.call, "scope": target_call.scope,
                        "actual_installed_int64_bits": parameters.iter().map(|parameter|
                            match parameter.value { report::ParameterValue::Int64 { bits } => bits,
                                report::ParameterValue::Unobserved => panic!("checked given value is unobserved") }).collect::<Vec<_>>(),
                        "actual_parameter_shapes": parameters.iter().map(|parameter| parameter.shape).collect::<Vec<_>>(),
                        "exact_context_bytes": entered.context,
                        "nested_resource_calls_retained": nested,
                    }));
                    cursor += 1;
                }
                assert_eq!(
                    cursor + 1,
                    events.len(),
                    "no missing, duplicated or extra trials"
                );
            }
        }
        attempts.push(json!({
            "attempt": boundary.identity.id, "ordinal": boundary.identity.ordinal,
            "begin_sequence": boundary.begin.sequence, "end_sequence": end.sequence,
            "begin_counters": acquisition_counters(&boundary.begin.counters),
            "end_counters": acquisition_counters(&end.counters),
            "trials": trials,
        }));
    }
    json!({
        "scope": "Resource ABI validated prologue and actual first completion publication",
        "ordinary_abi_calls_measured": false,
        "non_int64_parameter_values_measured": false,
        "final_counters": acquisition_counters(&telemetry.counters),
        "incomplete_flags": telemetry.flags,
        "history_records": telemetry.history.len(),
        "history_context_bytes": telemetry.context_bytes,
        "history_final_sequence": telemetry.final_sequence,
        "attempts": attempts,
        "raw_history": telemetry.history.iter().map(|event| json!({
            "sequence": event.sequence, "channel": format!("{:?}", event.channel),
            "exact_context_bytes": event.context, "event": format!("{:?}", event.kind),
        })).collect::<Vec<_>>(),
    })
}

fn observe_private(observed: &report::Report, purpose: Purpose, expected: &Json) -> Json {
    assert_eq!(
        observed.attempts.len(),
        2,
        "same context/grant/provider, two entry calls"
    );
    let context_history = observe_context_history(observed, purpose, expected);
    let mut previous = 0;
    let attempts = observed.attempts.iter().map(|attempt| {
        assert!(attempt.completion.attempt > previous);
        previous = attempt.completion.attempt;
        assert_eq!(attempt.completion.body_status, 0);
        assert_eq!(attempt.completion.cleanup_status, 0);
        assert_eq!(attempt.completion.selected_channel, report::FailureChannel::Clean);
        assert!(attempt.ordinary_message.is_empty() && attempt.resource_message.is_empty());
        assert!(attempt.counts.resources_retired() && attempt.counts.ordinary_empty);
        assert_eq!(attempt.event_sequence_end, 0);
        json!({
            "attempt": attempt.completion.attempt,
            "body_status": attempt.completion.body_status,
            "cleanup_status": attempt.completion.cleanup_status,
            "channel": format!("{:?}", attempt.completion.selected_channel),
            "resource_owners": attempt.counts.owners, "resource_loans": attempt.counts.loans,
            "resource_frames": attempt.counts.frames, "registry_entries": attempt.counts.registry_entries,
            "owner_handles": attempt.counts.owner_handles, "loan_handles": attempt.counts.loan_handles,
            "frame_handles": attempt.counts.frame_handles, "provisional_returns": attempt.counts.provisional_returns,
            "ordinary_storage_empty": attempt.counts.ordinary_empty,
            "observed_before_context_destruction": true, "event_sequence_end": attempt.event_sequence_end,
        })
    }).collect::<Vec<_>>();
    assert!(observed.events.is_empty());
    assert_eq!(observed.script_remaining, 0);
    assert!(observed.observations_ready_for_acceptance() && observed.all_attempts_succeeded());
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
        "attempts": attempts, "provider_events": [], "script_remaining": observed.script_remaining,
        "created": observed.created, "destroy_attempts": observed.destroy_attempts,
        "destroy_status": observed.destroy_status, "report_write_status": observed.write_status,
        "same_context_reentry_observed_by_matched_launcher": true,
        "zero_all_acquisition_dispatch_counters_measured": true,
        "property_context_and_installed_int64_arguments": context_history,
    })
}

fn run_whole_canonical_suites(fixture: Fixture) {
    assert!(
        cfg!(windows),
        "candidate pins existing measured MSVC archives"
    );
    assert_eq!(digest(fixture.source().as_bytes()), fixture.sha256());
    let evidence = PathBuf::from(
        std::env::var_os(fixture.evidence_env()).expect("fresh absolute evidence directory"),
    );
    assert!(evidence.is_absolute());
    fs::create_dir(&evidence).expect("evidence directory must not already exist");
    let directory = tempfile::tempdir().unwrap();
    let project_dir = directory.path().join("project");
    let support_dir = directory.path().join("synthetic-stdlib");
    fs::create_dir(&project_dir).unwrap();
    fs::create_dir(&support_dir).unwrap();
    let project = project_dir.join(fixture.file_name());
    let support = support_dir.join("resource_probe.jett");
    fs::write(&project, fixture.source()).unwrap();
    fs::write(&support, SUPPORT).unwrap();
    let mut prepared = Vec::new();
    for release in [false, true] {
        prepared.extend(prepare(fixture, &project, &support, release));
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
                let available = observe_private(&observed, prepared.purpose, &prepared.expected);
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
                    "schema": fixture.evidence_schema(),
                    "full_acceptance": false,
                    "remaining_instrumentation_obligations": UNMEASURED,
                    "whole_source_sha256": fixture.sha256(),
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

#[test]
#[ignore = "requires pinned measured normal/private archives; records available observations only"]
fn source17_whole_canonical_suites_record_available_native_observations() {
    run_whole_canonical_suites(Fixture::Source17);
}

#[test]
fn pure_replacement_canonical_suite_preparation_preserves_original_required_cache() {
    assert_eq!(
        digest(PURE_REPLACEMENTS.as_bytes()),
        PURE_REPLACEMENTS_SHA256
    );
    let directory = tempfile::tempdir().unwrap();
    let project = directory.path().join("pure_replacement_suites.jett");
    let support = directory.path().join("resource_probe.jett");
    fs::write(&project, PURE_REPLACEMENTS).unwrap();
    fs::write(&support, SUPPORT).unwrap();
    for release in [false, true] {
        let prepared = prepare(Fixture::PureReplacements, &project, &support, release);
        assert_eq!(prepared.len(), 2);
        assert!(matches!(prepared[0].purpose, Purpose::Verify));
        assert!(matches!(prepared[1].purpose, Purpose::Property));
        assert_ne!(prepared[0].entry, prepared[1].entry);
    }
    directory.close().unwrap();
}

#[test]
#[ignore = "requires current measured normal/private archives; unchanged pure replacement suites"]
fn pure_replacement_whole_canonical_suites_record_available_native_observations() {
    run_whole_canonical_suites(Fixture::PureReplacements);
}

#[test]
fn source17_property_history_rejects_wrong_values_order_and_incomplete_calls() {
    fn fixture() -> (report::Report, Json) {
        let identity = report::AttemptIdentity { id: 41, ordinal: 0 };
        let mut history = vec![report::HistoryEvent {
            sequence: 1,
            channel: report::Channel::Attempt(identity),
            context: None,
            kind: report::HistoryKind::Begin,
        }];
        let mut expected = Vec::new();
        for (ordinal, bits) in [u64::MAX, 23].into_iter().enumerate() {
            let context = format!(
                "property 'absent_aggregate_property' trial {}: ",
                ordinal + 1
            )
            .into_bytes();
            let sequence = history.len() as u64 + 1;
            let call = report::CallIdentity {
                call: 51 + ordinal as u64 * 2,
                scope: 52 + ordinal as u64 * 2,
                function: 7,
                signature: 8,
                template: 9,
                operation: 10,
            };
            history.push(report::HistoryEvent {
                sequence,
                channel: report::Channel::Attempt(identity),
                context: Some(context.clone()),
                kind: report::HistoryKind::Set,
            });
            history.push(report::HistoryEvent {
                sequence: sequence + 1,
                channel: report::Channel::Attempt(identity),
                context: Some(context.clone()),
                kind: report::HistoryKind::CallEnter {
                    call,
                    parameters: vec![report::Parameter {
                        parameter: 0,
                        shape: 3,
                        value: report::ParameterValue::Int64 { bits },
                    }],
                },
            });
            history.push(report::HistoryEvent {
                sequence: sequence + 2,
                channel: report::Channel::Attempt(identity),
                context: Some(context.clone()),
                kind: report::HistoryKind::CallComplete {
                    call,
                    entered_sequence: Some(sequence + 1),
                    body_status: 0,
                    failure_status: None,
                },
            });
            history.push(report::HistoryEvent {
                sequence: sequence + 3,
                channel: report::Channel::Attempt(identity),
                context: None,
                kind: report::HistoryKind::Clear,
            });
            expected.push(
                json!({ "function": 7, "checked_argument_int64_bits": [bits],
                "context_set_bytes_utf8": String::from_utf8(context).unwrap() }),
            );
        }
        let end = history.len() as u64 + 1;
        history.push(report::HistoryEvent {
            sequence: end,
            channel: report::Channel::Attempt(identity),
            context: None,
            kind: report::HistoryKind::End,
        });
        let context_bytes = history
            .iter()
            .map(|event| event.context.as_ref().map_or(0, Vec::len) as u64)
            .sum();
        let counters = report::Counters { values: [0; 5] };
        let observed = report::Report {
            attempts: vec![report::Attempt {
                completion: report::Completion {
                    attempt: 41,
                    body_status: 0,
                    cleanup_status: 0,
                    selected_channel: report::FailureChannel::Clean,
                },
                counts: report::Counts {
                    owners: 0,
                    loans: 0,
                    frames: 0,
                    registry_entries: 0,
                    owner_handles: 0,
                    loan_handles: 0,
                    frame_handles: 0,
                    provisional_returns: 0,
                    ordinary_empty: true,
                },
                ordinary_message: Vec::new(),
                resource_message: Vec::new(),
                event_sequence_end: 0,
            }],
            events: Vec::new(),
            script_remaining: 0,
            destroy_status: 0,
            created: true,
            destroy_attempts: 1,
            write_status: 0,
            observations: report::Snapshot {
                counters,
                flags: 0,
                context_bytes,
                final_sequence: end,
                attempts: vec![report::AttemptObservation {
                    identity,
                    begin: report::Boundary {
                        sequence: Some(1),
                        counters,
                    },
                    end: Some(report::Boundary {
                        sequence: Some(end),
                        counters,
                    }),
                }],
                history,
            },
        };
        (
            observed,
            json!({"trial_count": 2, "trials_in_checked_order": expected}),
        )
    }
    // Authenticate the complete baseline before testing any corruption.
    let (baseline, expected) = fixture();
    observe_context_history(&baseline, Purpose::Property, &expected);
    for mutation in 0..5 {
        let (mut observed, expected) = fixture();
        match mutation {
            0 => match &mut observed.observations.history[2].kind {
                report::HistoryKind::CallEnter { parameters, .. } => {
                    parameters[0].value = report::ParameterValue::Int64 { bits: 0 }
                }
                _ => unreachable!(),
            },
            1 => observed.observations.history.swap(1, 5),
            2 => {
                observed.observations.history.remove(3);
            }
            3 => observed.observations.flags = 1,
            4 => match &mut observed.observations.history[2].kind {
                report::HistoryKind::CallEnter { call, .. } => call.function = 8,
                _ => unreachable!(),
            },
            _ => unreachable!(),
        }
        assert!(
            std::panic::catch_unwind(|| observe_context_history(
                &observed,
                Purpose::Property,
                &expected
            ))
            .is_err(),
            "corrupt telemetry mutation {mutation} must fail the checked-plan oracle"
        );
    }
}
