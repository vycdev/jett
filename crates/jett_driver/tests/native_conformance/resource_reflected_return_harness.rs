//! Dedicated reflected Return objects, Source-deleted singles and immutable reentry.
use super::{reflected_return_cases, run_native_cases, run_object_preflight};

#[test]
fn native_resource_reflected_body_and_owned_returns_emit_in_both_profiles() {
    assert_eq!(reflected_return_cases::CASES.len(), 12);
    run_object_preflight(
        reflected_return_cases::CASES,
        "reflected-body-and-owned-returns",
    );
}

#[test]
#[ignore = "requires exact independently measured reentry-capable Debug/Release single-runtime archive receipt"]
fn native_resource_reflected_body_and_owned_returns_retire_before_teardown() {
    assert_eq!(reflected_return_cases::CASES.len(), 12);
    run_native_cases(reflected_return_cases::CASES);
    eprintln!(
        "Resource dedicated reflected Return single-entry acceptance: 12 cases; 24 Source-deleted executions; profiles=debug,release"
    );
}

#[test]
#[ignore = "requires exact independently measured reentry-capable Debug/Release single-runtime archive receipt"]
fn native_resource_reflected_body_and_owned_returns_reenter_with_same_provider_grant() {
    assert_eq!(reflected_return_cases::CASES.len(), 12);
    for release in [false, true] {
        for first in reflected_return_cases::CASES {
            let ready = reflected_return_cases::ready_for(first);
            super::source06_reentry::run_original_then_ready(first, ready, release);
        }
    }
    eprintln!(
        "Resource dedicated reflected Return same-grant reentry acceptance: 12 first scripts then same-Source READY; 24 Source-deleted executions; 48 entries; profiles=debug,release; retained original completions and first exits"
    );
}
