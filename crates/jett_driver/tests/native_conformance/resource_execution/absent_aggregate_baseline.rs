//! Whole Source17 through strict object emission and Source-deleted native execution.
use super::*;
#[path = "../resource_absent_aggregate_baseline_inputs.rs"]
mod inputs;

fn emit(
    input: &inputs::Input,
    release: bool,
    entered: &mut Vec<inputs::Phase>,
) -> Result<jett_codegen_cranelift::ObjectArtifact, inputs::Failure> {
    use inputs::{Phase, enter, failure};
    let (checked, span) = inputs::prepare(input, release, entered)?;
    enter(entered, Phase::RequiredCapture);
    let required = jett_driver::evaluate_checked_resource_required_values(&checked);
    if !required.diagnostics.is_empty() || !required.debug_events.is_empty() {
        return Err(failure(
            Phase::RequiredCapture,
            (&required.diagnostics, &required.debug_events),
        ));
    }
    enter(entered, Phase::RequiredCache);
    required
        .values
        .checked_required_values(&checked)
        .map_err(|error| failure(Phase::RequiredCache, error))?;
    if required.values.is_empty()
        || !required
            .values
            .values()
            .all(|value| matches!(value, jett_comptime::Value::Int64(17)))
    {
        return Err(failure(
            Phase::RequiredCache,
            "all original required values must be primitive 17",
        ));
    }
    let constants = checked
        .module()
        .items
        .iter()
        .filter_map(|item| match item {
            Item::VarDecl(declaration)
                if declaration.name.span.file == FileId::new(0)
                    && declaration.name.name == "absent_namespace_value" =>
            {
                Some(declaration.name.span)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [constant] = constants.as_slice() else {
        return Err(failure(
            Phase::RequiredCache,
            "unique original namespace constant required",
        ));
    };
    if required.values.constant(*constant) != Some(&jett_comptime::Value::Int64(17)) {
        return Err(failure(
            Phase::RequiredCache,
            "original namespace value must be baked as 17",
        ));
    }
    enter(entered, Phase::HirLowering);
    let mut hir = jett_hir::lower_checked_resource_program(&checked)
        .map_err(|error| failure(Phase::HirLowering, error))?;
    enter(entered, Phase::ValueBaking);
    jett_driver::bake_checked_resource_values(&mut hir, &checked, &required.values)
        .map_err(|error| failure(Phase::ValueBaking, error))?;
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
        if !hir.functions.iter().any(|function| {
            function.identity.declaration.origin == SourceOrigin::Project
                && function.identity.declaration.namespace == "app"
                && function.identity.declaration.name == name
        }) {
            return Err(failure(
                Phase::ValueBaking,
                ("original declaration must remain", name),
            ));
        }
    }
    enter(entered, Phase::HirEntryIdentity);
    let entries = hir
        .functions
        .iter()
        .filter(|function| {
            function.span == span
                && function.identity.declaration.origin == SourceOrigin::Project
                && function.identity.declaration.kind == jett_hir::DeclarationKind::Function
        })
        .map(|function| function.id)
        .collect::<Vec<_>>();
    let [entry] = entries.as_slice() else {
        return Err(failure(
            Phase::HirEntryIdentity,
            "unique original main -> HIR identity required",
        ));
    };
    enter(entered, Phase::ValueConversions);
    jett_hir::complete_value_conversions(&mut hir, &checked.checked().interner)
        .map_err(|error| failure(Phase::ValueConversions, error))?;
    enter(entered, Phase::MirLowering);
    let mir = jett_mir::lower(&hir, &checked.checked().interner)
        .map_err(|error| failure(Phase::MirLowering, error))?;
    enter(entered, Phase::MirValidation);
    jett_mir::validate(&mir).map_err(|error| failure(Phase::MirValidation, error))?;
    enter(entered, Phase::ObjectEmission);
    let object = jett_codegen_cranelift::emit_host_program_object_with_options(
        &mir,
        &checked.checked().interner,
        *entry,
        jett_codegen_cranelift::CodegenOptions { optimize: release },
    )
    .map_err(|error| failure(Phase::ObjectEmission, error))?;
    enter(entered, Phase::ObjectObservations);
    if object.bytes.is_empty()
        || object.target != jett_driver::native::host_target()
        || !object
            .symbols
            .iter()
            .any(|symbol| symbol == jett_codegen_cranelift::JETT_AOT_ENTRY_SYMBOL_V1)
        || !object
            .symbols
            .iter()
            .any(|symbol| symbol == "jett_aot_resource_v1_manifest")
    {
        return Err(failure(
            Phase::ObjectObservations,
            (
                &object.target,
                &object.symbols,
                "nonempty host object with entry and Resource manifest required",
            ),
        ));
    }
    Ok(object)
}

#[test]
fn native_resource_absent_aggregate_object_baseline_records_all_first_rejecting_phases() {
    assert_eq!(inputs::INPUTS.len(), 14);
    let mut emitted = 0;
    let mut failures = Vec::new();
    for release in [false, true] {
        for input in inputs::INPUTS {
            let attempt = inputs::attempt(|entered| emit(input, release, entered));
            match attempt.outcome {
                Ok(object) => {
                    emitted += 1;
                    eprintln!(
                        "ABSENT_OBJECT name={:?} release={release} entered={:?} bytes={} target={:?}",
                        input.name,
                        attempt.entered,
                        object.bytes.len(),
                        object.target
                    );
                }
                Err(failure) => {
                    eprintln!(
                        "ABSENT_OBJECT name={:?} release={release} entered={:?} failure={failure:?}",
                        input.name, attempt.entered
                    );
                    failures.push((input.name, release, failure));
                }
            }
        }
    }
    assert_eq!(emitted + failures.len(), 28);
    eprintln!(
        "ABSENT_OBJECT_TOTAL attempts=28 emitted={emitted} failed={}",
        failures.len()
    );
    assert!(
        failures.is_empty(),
        "strict object gate failures: {failures:#?}"
    );
    assert_eq!(emitted, 28);
}

struct NativeExpectation {
    body: u32,
    channel: report::FailureChannel,
    ordinary_message: &'static [u8],
    resource_message: &'static [u8],
    exit: i32,
}
fn expected(input: &inputs::Input) -> NativeExpectation {
    match input.reference {
        inputs::ReferenceExpectation::Nothing => NativeExpectation {
            body: 0,
            channel: report::FailureChannel::Clean,
            ordinary_message: b"",
            resource_message: b"",
            exit: 0,
        },
        inputs::ReferenceExpectation::Error(error) => {
            assert_eq!(input.name, "13_wrong_empty_length_control");
            assert_eq!(error, "list.__remove_at: index -1 out of bounds");
            NativeExpectation {
                body: 1,
                channel: report::FailureChannel::Body,
                ordinary_message: error.as_bytes(),
                resource_message: b"native Resource protocol refused",
                exit: 71,
            }
        }
    }
}

fn empty_script(attempts: u32) -> String {
    assert!(matches!(attempts, 1 | 2));
    let mut bytes = b"JTRST001".to_vec();
    bytes.extend(1u32.to_le_bytes());
    bytes.extend(0u32.to_le_bytes());
    bytes.extend(32u64.to_le_bytes());
    bytes.extend(attempts.to_le_bytes());
    bytes.extend(0u32.to_le_bytes());
    assert_eq!(bytes.len(), 32);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn assert_observed(input: &inputs::Input, observed: &report::Report, attempts: usize) {
    let expected = expected(input);
    assert_eq!(
        observed.attempts.len(),
        attempts,
        "{}: {observed:?}",
        input.name
    );
    let mut previous = 0;
    for (index, attempt) in observed.attempts.iter().enumerate() {
        assert!(
            attempt.completion.attempt > previous,
            "{} attempt identity",
            input.name
        );
        previous = attempt.completion.attempt;
        assert_eq!(
            attempt.completion.body_status, expected.body,
            "{} entry{index} original body: {observed:?}",
            input.name
        );
        assert_eq!(
            attempt.completion.cleanup_status, 0,
            "{} cleanup",
            input.name
        );
        assert_eq!(
            attempt.completion.selected_channel, expected.channel,
            "{} channel",
            input.name
        );
        assert_eq!(
            attempt.ordinary_message, expected.ordinary_message,
            "{} ordinary message",
            input.name
        );
        assert_eq!(
            attempt.resource_message, expected.resource_message,
            "{} resource message",
            input.name
        );
        assert!(
            attempt.counts.resources_retired() && attempt.counts.ordinary_empty,
            "{} entry{index} obligations BEFORE destruction: {observed:?}",
            input.name
        );
        assert_eq!(
            attempt.event_sequence_end, 0,
            "{} event boundary",
            input.name
        );
    }
    assert!(observed.events.is_empty(), "{}: {observed:?}", input.name);
    assert_eq!(observed.script_remaining, 0);
    assert_eq!(
        (
            observed.created,
            observed.destroy_attempts,
            observed.destroy_status,
            observed.write_status
        ),
        (true, 1, 0, 0)
    );
    assert!(
        observed.observations_ready_for_acceptance(),
        "{}: {observed:?}",
        input.name
    );
    assert_eq!(observed.all_attempts_succeeded(), expected.body == 0);
}

fn execute(input: &inputs::Input, release: bool, archive: &Archive, attempts: u32) {
    assert!(input.source.starts_with(inputs::ORIGINAL));
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let project = root.join("project");
    let stdlib = root.join("synthetic-stdlib");
    fs::create_dir(&project).unwrap();
    fs::create_dir(&stdlib).unwrap();
    let sources = [
        (project.join("main.jett"), input.source),
        (stdlib.join("resource_probe.jett"), inputs::SUPPORT),
        (stdlib.join("list_length.jett"), inputs::LIST_LENGTH),
        (stdlib.join("map_length.jett"), inputs::MAP_LENGTH),
    ];
    for (path, source) in &sources {
        fs::write(path, source).unwrap();
        assert_eq!(fs::read(path).unwrap(), source.as_bytes());
    }
    // The shared preparation authenticates these exact finite Source bytes,
    // trusted fragments and original FileId/span/DefId joins.
    let attempt = inputs::attempt(|entered| emit(input, release, entered));
    let artifact = attempt.outcome.unwrap_or_else(|failure| {
        panic!(
            "{} release={release} entered={:?}: {failure:?}",
            input.name, attempt.entered
        )
    });
    assert_eq!(artifact.target, archive.target);
    let object_path = root.join("resource.obj");
    let binary = root.join(if cfg!(windows) {
        "resource.exe"
    } else {
        "resource"
    });
    fs::write(&object_path, artifact.bytes).unwrap();
    link(archive, &object_path, &binary, root);
    fs::remove_dir_all(&project).unwrap();
    fs::remove_dir_all(&stdlib).unwrap();
    assert!(!project.exists() && !stdlib.exists());
    assert!(sources.iter().all(|(path, _)| !path.exists()));
    let encoded = empty_script(attempts);
    let actual = super::super::run_bounded_with_env(
        &binary,
        root,
        Some(("JETT_RESOURCE_NATIVE_TEST_SCRIPT_V1", &encoded)),
    );
    let report_path = root.join("resource-native-report-v1.bin");
    assert!(report_path.metadata().unwrap().len() <= 16 * 1024 * 1024);
    let observed = report::decode(&fs::read(report_path).unwrap()).unwrap();
    assert_eq!(
        actual.status.code(),
        Some(expected(input).exit),
        "{} {}: {actual:?}; {observed:?}",
        input.name,
        archive.profile
    );
    assert!(
        actual.stdout.is_empty() && actual.stderr.is_empty(),
        "{actual:?}"
    );
    assert_observed(input, &observed, usize::try_from(attempts).unwrap());
    eprintln!(
        "ABSENT_NATIVE name={:?} release={release} attempts={attempts} source_deleted=true report={observed:?}",
        input.name
    );
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NativeAttemptMode {
    Single,
    CleanReentry,
    AllReentry,
}

fn run_native(mode: NativeAttemptMode) {
    let clean_reentry = mode == NativeAttemptMode::CleanReentry;
    assert_eq!(inputs::INPUTS.len(), 14);
    let mut passed = 0;
    let mut failures = Vec::new();
    for release in [false, true] {
        let archive = archive(release);
        for input in inputs::INPUTS {
            if clean_reentry && matches!(input.reference, inputs::ReferenceExpectation::Error(_)) {
                continue;
            }
            let attempts = if mode == NativeAttemptMode::Single {
                1
            } else {
                2
            };
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                execute(input, release, &archive, attempts)
            }));
            match outcome {
                Ok(()) => passed += 1,
                Err(payload) => {
                    failures.push((input.name, release, inputs::panic_message(payload.as_ref())))
                }
            }
        }
    }
    let required = if clean_reentry { 26 } else { 28 };
    assert_eq!(passed + failures.len(), required);
    assert!(
        failures.is_empty(),
        "strict Source-deleted native gate failures: {failures:#?}"
    );
    assert_eq!(passed, required);
    let entries = required
        * if mode == NativeAttemptMode::Single {
            1
        } else {
            2
        };
    eprintln!(
        "ABSENT_NATIVE_TOTAL executables={passed} entries={entries} clean_reentry={clean_reentry}"
    );
}

#[test]
#[ignore = "requires exact Root-measured double-cfg Debug/Release single-runtime archive receipt"]
fn native_resource_absent_aggregate_all_original_sources_retire_before_teardown() {
    run_native(NativeAttemptMode::Single);
}

#[test]
#[ignore = "requires exact Root-measured double-cfg Debug/Release single-runtime archive receipt"]
fn native_resource_absent_aggregate_thirteen_clean_sources_same_grant_reentry() {
    // Source13 remains in the full single-entry gate above. Its original two-error
    // reference behavior is a separate unresolved native failure-lifetime obligation.
    run_native(NativeAttemptMode::CleanReentry);
}

#[test]
#[ignore = "requires exact Root-measured ordinary-error-reentry Debug/Release archive receipt"]
fn native_resource_absent_aggregate_all_original_sources_reenter_with_same_provider_grant() {
    // Every original row is selected, including Source13's signed bounds error
    // twice on one retained context/provider/grant. assert_observed checks both
    // original completions, exact diagnostics, zero pre-teardown obligations,
    // continuous empty events and first process failure; execute deletes Source.
    run_native(NativeAttemptMode::AllReentry);
}
