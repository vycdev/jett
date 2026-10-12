//! Exact original occupied Sources; receipt guards and retirement oracle are shared.
use super::{
    archive, cases, fs, link, object, report, run_native_cases, run_object_preflight, script,
};
#[path = "resource_occupied_struct_cases.rs"]
mod occupied_struct_cases;
#[path = "resource_occupied_struct_reentry.rs"]
mod reentry;

#[test]
fn native_resource_occupied_struct_three_sources_emit_in_both_profiles() {
    assert_eq!(occupied_struct_cases::CASES.len(), 11);
    run_object_preflight(occupied_struct_cases::CASES, "occupied-struct");
}
#[test]
#[ignore = "requires fresh independently measured occupied-capable Debug/Release single-runtime archives"]
fn native_resource_occupied_struct_scripts_retire_before_teardown() {
    assert_eq!(occupied_struct_cases::CASES.len(), 11);
    run_native_cases(occupied_struct_cases::CASES);
}
#[test]
#[ignore = "requires fresh independently measured occupied-capable Debug/Release single-runtime archives"]
fn native_resource_occupied_struct_scripts_reenter_with_same_provider_grant() {
    for release in [false, true] {
        for first in occupied_struct_cases::CASES {
            reentry::run_original_then_ready(
                first,
                occupied_struct_cases::ready_for(first),
                release,
            );
        }
    }
}
