//! Exact derived reflected Sources through object, single-entry and reentry gates.
use super::{reflected_derived_cases, run_native_cases, run_object_preflight};

#[test]
fn native_resource_reflected_derived_controls_emit_in_both_profiles() {
    assert_eq!(reflected_derived_cases::CASES.len(), 11);
    assert_eq!(reflected_derived_cases::original_cases().len(), 10);
    run_object_preflight(reflected_derived_cases::CASES, "reflected-derived-controls");
}

#[test]
#[ignore = "requires exact independently measured reentry-capable Debug/Release single-runtime archive receipt"]
fn native_resource_reflected_derived_controls_retire_before_teardown() {
    assert_eq!(reflected_derived_cases::CASES.len(), 11);
    run_native_cases(reflected_derived_cases::CASES);
    eprintln!(
        "Resource reflected derived single-entry acceptance: 10 original scripts plus scalar-default READY baseline; 22 Source-deleted executions; profiles=debug,release"
    );
}

#[test]
#[ignore = "requires exact independently measured reentry-capable Debug/Release single-runtime archive receipt"]
fn native_resource_reflected_derived_controls_reenter_with_same_provider_grant() {
    assert_eq!(reflected_derived_cases::original_cases().len(), 10);
    for release in [false, true] {
        for first in reflected_derived_cases::original_cases() {
            let ready = reflected_derived_cases::ready_for(first);
            super::source06_reentry::run_original_then_ready(first, ready, release);
        }
    }
    eprintln!(
        "Resource reflected derived same-grant reentry acceptance: 10 original scripts then their same-Source READY; 20 Source-deleted executions; 40 entries; profiles=debug,release; retained completions and first nonzero exits"
    );
}
