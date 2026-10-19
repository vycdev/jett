//! Exact original occupied Sources; receipt guards and retirement oracle are shared.
use super::{
    archive, cases, fs, link, object, report, run_native_cases, run_native_cases_with_support,
    run_object_preflight, run_object_preflight_with_support, script,
};
#[path = "resource_occupied_struct_cases.rs"]
mod occupied_struct_cases;
#[path = "resource_primitive_companion_cases.rs"]
mod primitive_companion_cases;
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

#[test]
fn native_resource_primitive_companions_sources_emit_in_both_profiles() {
    assert_eq!(primitive_companion_cases::CASES.len(), 23);
    run_object_preflight_with_support(
        primitive_companion_cases::CASES,
        "primitive-companions",
        &companion_support(),
    );
}

#[test]
#[ignore = "requires fresh independently measured scalar-task-capable Debug/Release single-runtime archives"]
fn native_resource_primitive_companions_retire_before_teardown() {
    run_native_cases_with_support(primitive_companion_cases::CASES, &companion_support());
}

#[test]
#[ignore = "requires fresh independently measured scalar-task-capable Debug/Release single-runtime archives"]
fn native_resource_primitive_companions_reenter_with_same_provider_grant() {
    for release in [false, true] {
        for first in primitive_companion_cases::CASES {
            reentry::run_original_then_ready_with_support(
                first,
                primitive_companion_cases::ready_for(first),
                release,
                &companion_support(),
            );
        }
    }
}

fn companion_support() -> String {
    format!(
        "{}\n{}",
        cases::SUPPORT,
        include_str!(
            "../../../jett_comptime/src/resource_execution/fixtures/resource_companion_probe.jett"
        )
    )
}
