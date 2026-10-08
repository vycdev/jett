//! Real original checked-source counterpart to the linked private archive cases.
#[path = "../../../../jett_driver/tests/native_conformance/resource_cases.rs"]
mod cases;
use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn checked_case(case: &cases::Case, release: bool) -> Arc<CheckedResourceProgram> {
    let stdlib = FileId::new(10_000);
    let primary = FileId::new(0);
    let mut parsed = parse(cases::SUPPORT, stdlib);
    let project = parse(case.source, primary);
    assert!(parsed.errors.is_empty(), "support: {:?}", parsed.errors);
    assert!(
        project.errors.is_empty(),
        "{}: {:?}",
        case.name,
        project.errors
    );
    let resources = parsed
        .module
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Resource(item) => Some(item.name.span),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [declaration] = resources.as_slice() else {
        panic!("exact synthetic Resource declaration is not unique");
    };
    let catalog = [
        ("kernel_create", ResourceKernelRecipe::NetworkFactory),
        ("kernel_borrow", ResourceKernelRecipe::NetworkBorrow),
        ("kernel_close", ResourceKernelRecipe::Finalize),
    ]
    .into_iter()
    .map(|(member, recipe)| ResourceKernelSpec {
        resource_declaration: *declaration,
        member: member.into(),
        recipe,
    })
    .collect::<Vec<_>>();
    parsed.module.items.extend(project.module.items);
    Arc::new(
        CheckedResourceProgram::prepare(
            parsed,
            HashMap::from([
                (stdlib, SourceOrigin::Stdlib),
                (primary, SourceOrigin::Project),
            ]),
            &catalog,
            CheckOptions { release },
        )
        .unwrap_or_else(|error| {
            panic!(
                "{} reference prerequisite: {error:?}; {:?}",
                case.name,
                error.diagnostics()
            )
        }),
    )
}
fn script(input: cases::Script) -> ScriptOperation {
    match input {
        cases::Script::Construct(label) => ScriptOperation::Construct {
            label,
            outcome: Ok(()),
        },
        cases::Script::ConstructFail(label, error) => ScriptOperation::Construct {
            label,
            outcome: Err(error.into()),
        },
        cases::Script::FinalizerPanic(label) => ScriptOperation::ConstructFinalizerPanic { label },
        cases::Script::BorrowPanic(label) => ScriptOperation::BorrowPanic { label },
        cases::Script::Borrow(label, value) => ScriptOperation::Borrow {
            label,
            outcome: Ok(value),
        },
        cases::Script::BorrowFail(label, error) => ScriptOperation::Borrow {
            label,
            outcome: Err(error.into()),
        },
    }
}
fn event(input: &cases::Event) -> ProviderEvent {
    match *input {
        cases::Event::Constructed(label) => ProviderEvent::Constructed(label),
        cases::Event::ConstructionFailed(label) => ProviderEvent::ConstructionFailed(label),
        cases::Event::Borrowed(label) => ProviderEvent::Borrowed(label),
        cases::Event::BorrowFailed(label) => ProviderEvent::BorrowFailed(label),
        cases::Event::Finalized(label) => ProviderEvent::Finalized(label),
    }
}

fn reference_with_required_values(
    checked: &Arc<CheckedResourceProgram>,
    case: &cases::Case,
    release: bool,
) -> Interpreter {
    // Check the worker's existing purpose boundary without executing Source or
    // installing a provider. The real required worker below receives no grant.
    let mut worker_guard = Interpreter::new();
    worker_guard
        .install_checked_resource_program(checked.clone(), ExecutionPurpose::ExplicitComptime)
        .unwrap();
    worker_guard
        .authorize_checked_resource_worker(checked.module(), ExecutionPurpose::ExplicitComptime)
        .unwrap();
    assert!(
        worker_guard
            .install_resource_test_script(Vec::new())
            .is_err(),
        "{} release={release}: required purpose must refuse a runtime provider",
        case.name
    );
    assert_eq!(worker_guard.resource_test_custody_counts().unwrap(), (0, 0));
    assert!(worker_guard.resource_test_observations().is_err());

    let types = Arc::new(crate::checked_types::CheckedExpressionTypes {
        resource_program: Some(checked.clone()),
        expressions: checked
            .checked()
            .type_map
            .iter()
            .map(|(span, ty)| (*span, checked.checked().interner.type_name(*ty)))
            .collect(),
        ..Default::default()
    });
    let metadata = Arc::new(jett_types::ReflectionMetadata::new());
    let exclusions = Arc::new(HashMap::new());
    let required = crate::evaluate_explicit_comptime_expressions_capture(
        checked.module(),
        metadata.clone(),
        types.clone(),
        exclusions.clone(),
    );
    assert!(
        required.diagnostics.is_empty(),
        "{} release={release}: required evaluation failed: {:?}",
        case.name,
        required.diagnostics
    );
    assert!(
        required.debug_events.is_empty(),
        "{} release={release}: unexpected required-phase debug observations: {:?}",
        case.name,
        required.debug_events
    );
    assert!(required.values.checked_values_are_mirrored());
    assert_eq!(required.values.len(), required.values.values().count());
    assert!(
        required
            .values
            .values()
            .all(|value| !value.contains_live_resource_or_grant()),
        "{} release={release}: required values cannot import runtime authority",
        case.name
    );

    // Install even an empty checked cache. Expr::Comptime then refuses a missing
    // exact checked key instead of falling back to evaluating its runtime body.
    // Match the driver's order so namespace constants cannot replay at register.
    let mut interpreter = Interpreter::new();
    interpreter.set_reflection_metadata(metadata);
    interpreter.set_checked_expression_types(types);
    interpreter.set_breakpoint_exclusions(exclusions);
    interpreter.set_explicit_comptime_values(Arc::new(required.values));
    interpreter.register_module(checked.module());
    interpreter
        .install_checked_resource_program(checked.clone(), ExecutionPurpose::ReferenceRuntime)
        .unwrap();
    assert_eq!(interpreter.resource_test_custody_counts().unwrap(), (0, 0));
    assert!(interpreter.resource_test_observations().is_err());
    interpreter
}

fn run_reference_cases(selected: &[cases::Case]) {
    for release in [false, true] {
        for case in selected {
            let checked = checked_case(case, release);
            let definition = entry(&checked, "main");
            let mut interpreter = reference_with_required_values(&checked, case, release);
            let grant = interpreter
                .install_resource_test_script(case.script.iter().copied().map(script).collect())
                .unwrap();
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                interpreter.call_checked_program_entry(definition, vec![grant])
            }));
            match case.reference {
                cases::ReferenceOutcome::Clean => assert_eq!(
                    outcome
                        .unwrap()
                        .unwrap_or_else(|error| panic!("{} release={release}: {error}", case.name)),
                    Value::Nothing,
                    "{} release={release}",
                    case.name
                ),
                cases::ReferenceOutcome::Error(expected) => assert_eq!(
                    outcome.unwrap().unwrap_err(),
                    expected,
                    "{} release={release}",
                    case.name
                ),
                cases::ReferenceOutcome::ProviderPanic(expected) => {
                    let panic = outcome.expect_err("actual Source borrow provider panic");
                    let text = panic
                        .downcast_ref::<String>()
                        .map(String::as_str)
                        .or_else(|| panic.downcast_ref::<&str>().copied())
                        .unwrap();
                    assert_eq!(text, expected, "{} release={release}", case.name);
                }
                cases::ReferenceOutcome::CleanupPanic(expected) => {
                    let panic = outcome.expect_err("actual Source finalizer panic");
                    let text = panic
                        .downcast_ref::<String>()
                        .map(String::as_str)
                        .or_else(|| panic.downcast_ref::<&str>().copied())
                        .unwrap();
                    assert_eq!(text, expected, "{} release={release}", case.name);
                }
            }
            assert_eq!(
                interpreter.resource_test_observations().unwrap(),
                (case.events.iter().map(event).collect(), 0, 0),
                "{} release={release}",
                case.name
            );
            assert!(
                interpreter.take_debug_events().is_empty(),
                "{} release={release}",
                case.name
            );
        }
    }
}

#[test]
fn native_resource_source_cases_match_real_reference_events_and_cleanup_before_teardown() {
    run_reference_cases(cases::CASES);
}

#[test]
fn native_resource_pipeline_source_cases_match_real_reference_events_and_cleanup_before_teardown() {
    run_reference_cases(cases::pipeline_cases());
}

#[test]
fn native_resource_control_flow_source_cases_match_real_reference_events_and_cleanup_before_teardown()
 {
    run_reference_cases(cases::control_flow_cases());
}
