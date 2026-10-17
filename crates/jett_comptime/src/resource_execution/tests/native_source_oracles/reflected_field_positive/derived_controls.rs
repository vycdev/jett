//! Genuine checked reference counterpart to every shared derived native input.
use super::*;

#[test]
fn reflected_derived_native_single_inputs_match_real_reference() {
    assert_eq!(reflected_derived_cases::CASES.len(), 11);
    assert_eq!(reflected_derived_cases::original_cases().len(), 10);
    run_reference_cases(reflected_derived_cases::CASES);
}

#[test]
fn reflected_derived_native_reentry_inputs_match_real_reference() {
    assert_eq!(reflected_derived_cases::original_cases().len(), 10);
    for release in [false, true] {
        for first in reflected_derived_cases::original_cases() {
            let ready = reflected_derived_cases::ready_for(first);
            source06_native_reentry::original_then_ready(first, ready, release);
        }
    }
}
