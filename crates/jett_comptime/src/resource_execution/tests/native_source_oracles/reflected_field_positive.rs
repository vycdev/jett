//! Reference counterpart of the standalone four-Source clean native rows.
use super::*;

#[path = "../../../../../jett_driver/tests/native_conformance/resource_reflected_field_cases.rs"]
mod reflected_field_cases;

#[test]
fn reflected_field_four_positive_sources_match_real_reference() {
    assert_eq!(reflected_field_cases::CASES.len(), 4);
    run_reference_cases(reflected_field_cases::CASES);
}

#[test]
fn reflected_field_four_positive_sources_clean_same_grant_reentry() {
    for release in [false, true] {
        for case in reflected_field_cases::CASES {
            let checked = checked_case(case, release);
            let definition = entry(&checked, "main");
            let mut interpreter = reference_with_required_values(&checked, case, release);
            let grant = interpreter
                .install_resource_test_script(
                    case.script
                        .iter()
                        .chain(case.script.iter())
                        .copied()
                        .map(script)
                        .collect(),
                )
                .unwrap();
            let context = interpreter.resource_test_entry_context().unwrap();
            let mut expected = Vec::new();
            for _ in 0..2 {
                assert_eq!(
                    interpreter
                        .call_checked_program_entry(definition, vec![grant.clone()])
                        .unwrap(),
                    Value::Nothing,
                    "{} release={release}",
                    case.name
                );
                expected.extend(case.events.iter().map(event));
                assert_eq!(interpreter.resource_test_entry_context().unwrap(), context);
                assert_eq!(
                    interpreter.resource_test_observations().unwrap(),
                    (expected.clone(), 0, 0)
                );
                assert_eq!(interpreter.resource_test_custody_counts().unwrap(), (0, 0));
                assert!(interpreter.take_debug_events().is_empty());
            }
        }
    }
}

#[test]
fn reflected_field_source06_exceptional_native_inputs_match_real_reference() {
    assert_eq!(reflected_field_cases::SOURCE06_EXCEPTIONAL_CASES.len(), 8);
    run_reference_cases(reflected_field_cases::SOURCE06_EXCEPTIONAL_CASES);
}

#[path = "reflected_field_positive/reentry.rs"]
mod source06_native_reentry;

#[path = "../../../../../jett_driver/tests/native_conformance/resource_reflected_derived_cases.rs"]
mod reflected_derived_cases;

#[path = "reflected_field_positive/derived_controls.rs"]
mod derived_controls;

#[path = "../../../../../jett_driver/tests/native_conformance/resource_reflected_return_cases.rs"]
mod reflected_return_cases;

#[path = "reflected_field_positive/reflected_returns.rs"]
mod reflected_returns;
