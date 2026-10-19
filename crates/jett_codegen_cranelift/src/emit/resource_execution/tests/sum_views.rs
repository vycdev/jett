use super::*;
#[path = "../../../../../jett_driver/tests/native_conformance/resource_cases.rs"]
mod cases;

#[test]
fn resource_borrowed_sum_objects_emit_checked_nonconsuming_leaves_in_both_profiles() {
    let cases = cases::CASES
        .iter()
        .filter(|case| {
            case.name.starts_with("view_optional_") || case.name.starts_with("view_result_")
        })
        .collect::<Vec<_>>();
    assert_eq!(cases.len(), 12);
    for release in [false, true] {
        for case in &cases {
            let artifact = emitted(case.source, release);
            let (_, imports) = symbols(&artifact);
            for leaf in [
                "jett_rt_v1_resource_sum_borrow",
                "jett_rt_v1_resource_sum_view_tag",
                "jett_rt_v1_resource_sum_view_project",
                "jett_rt_v1_resource_sum_borrow_end",
            ] {
                assert!(
                    imports.iter().any(|name| name == leaf),
                    "{} release={release} missing {leaf}",
                    case.name
                );
            }
            if case.name.starts_with("view_result_") {
                assert!(
                    imports
                        .iter()
                        .any(|name| name == "jett_rt_v1_resource_sum_failure_read"),
                    "{} release={release}",
                    case.name
                );
            }
        }
    }
}
