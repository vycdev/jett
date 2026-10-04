use super::*;

#[test]
fn checked_source_mutable_assignment_frontend_prerequisites() {
    for release in [false, true] {
        let checked = program(include_str!("fixtures/24_mutable_assignment.jett"), release);
        for name in [
            "replace_live",
            "rebind_after_close",
            "rebind_self",
            "failed_rhs_keeps_owner",
        ] {
            let _ = entry(&checked, name);
        }
    }
}

#[test]
fn checked_source_pipeline_frontend_prerequisites() {
    for release in [false, true] {
        for source in [
            include_str!("fixtures/18_pipeline_construct_move_close.jett"),
            include_str!("fixtures/19_pipeline_written_view_retains_owner.jett"),
            include_str!("fixtures/20_pipeline_bare_owner_view_operation.jett"),
            include_str!("fixtures/21_pipeline_named_actual_abort.jett"),
            include_str!("fixtures/22_pipeline_domain_failure_stops_next_step.jett"),
            include_str!("fixtures/23_pipeline_indirect_close_descriptor.jett"),
        ] {
            let checked = program(source, release);
            let _ = entry(&checked, "scenario");
        }
    }
}
