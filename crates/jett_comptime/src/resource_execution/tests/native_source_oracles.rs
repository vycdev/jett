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
        cases::Script::Borrow(label, value) => ScriptOperation::Borrow {
            label,
            outcome: Ok(value),
        },
    }
}
fn event(input: &cases::Event) -> ProviderEvent {
    match *input {
        cases::Event::Constructed(label) => ProviderEvent::Constructed(label),
        cases::Event::ConstructionFailed(label) => ProviderEvent::ConstructionFailed(label),
        cases::Event::Borrowed(label) => ProviderEvent::Borrowed(label),
        cases::Event::Finalized(label) => ProviderEvent::Finalized(label),
    }
}
#[test]
fn native_resource_source_cases_match_real_reference_events_and_cleanup_before_teardown() {
    for release in [false, true] {
        for case in cases::CASES {
            let checked = checked_case(case, release);
            let definition = entry(&checked, "main");
            let mut interpreter = Interpreter::from_checked_resource_program(
                checked,
                ExecutionPurpose::ReferenceRuntime,
            )
            .unwrap();
            let grant = interpreter
                .install_resource_test_script(case.script.iter().copied().map(script).collect())
                .unwrap();
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                interpreter.call_checked_program_entry(definition, vec![grant])
            }));
            match case.reference {
                cases::ReferenceOutcome::Clean => assert_eq!(
                    outcome.unwrap().unwrap(),
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
