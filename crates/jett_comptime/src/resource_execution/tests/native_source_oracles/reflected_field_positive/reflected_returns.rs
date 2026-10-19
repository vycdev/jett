//! Genuine checked reference counterpart of all shared dedicated Return inputs.
use super::*;

#[test]
fn reflected_body_and_owned_return_single_inputs_match_real_reference() {
    assert_eq!(reflected_return_cases::CASES.len(), 12);
    run_reference_cases(reflected_return_cases::CASES);
}

#[test]
fn reflected_body_and_owned_return_reentry_inputs_match_real_reference() {
    assert_eq!(reflected_return_cases::CASES.len(), 12);
    for release in [false, true] {
        for first in reflected_return_cases::CASES {
            let ready = reflected_return_cases::ready_for(first);
            source06_native_reentry::original_then_ready(first, ready, release);
        }
    }
}
