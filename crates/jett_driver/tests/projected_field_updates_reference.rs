//! Source regressions for owned field replacement and its checked boundaries.
use std::path::PathBuf;

use jett_driver::{build_file, run_file_capture_outcome, test_file};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/field_update_reference")
        .join(name)
}

fn require_admitted(name: &str) -> PathBuf {
    let path = fixture(name);
    let built = build_file(&path);
    assert!(!built.has_errors, "{name}: {:?}", built.diagnostics);
    path
}

#[test]
fn ordinary_field_updates_execute_from_genuine_source() {
    for (name, expected) in [
        (
            "01_called_baselines.jett",
            "original:1:before\nchanged:9:after\n",
        ),
        ("02_owning_clone_fields.jett", "2:1:before:3:3:after:true\n"),
        (
            "03_rhs_order_and_early_return.jett",
            "rhs\nchanged:9:before\nunchanged:1:before\n",
        ),
        ("04_exact_machine_state.jett", "machine:8:after:true\n"),
        (
            "07_pending_children_transport.jett",
            "children:17:5:again\n",
        ),
        ("09_generic_alias.jett", "1:9:number:new:text\n"),
        ("10_refined_field_replacement.jett", "2:stable\n"),
        ("22_branch_loop_lifetimes.jett", "5:first:5:second\n"),
        (
            "25_secret_owned_root.jett",
            "secret:1:before:2|9:after:8|1:before:2\n",
        ),
        ("26_for_loop_update.jett", "loop:6:1:stable\n"),
        ("27_nested_handler_update.jett", "handler:1:7:stable\n"),
        (
            "28_metadata_holder_replacement.jett",
            "metadata:string:stable\n",
        ),
        (
            "29_inline_owned_parameter.jett",
            "inline:1:stable|7:changed\n",
        ),
        (
            "30_returned_inline_owned_parameter.jett",
            "returned:1:8:stable\n",
        ),
        (
            "31_reflected_iteration_update.jett",
            "reflected:6:int64|string|int64|:1:stable\n",
        ),
    ] {
        let path = require_admitted(name);
        let outcome = run_file_capture_outcome(&path)
            .unwrap_or_else(|failure| panic!("{name}: {}", failure.message));
        assert_eq!(outcome.stdout, expected, "{name}");
        assert!(outcome.debug_events.is_empty(), "{name}");
    }
}

#[test]
fn field_update_verify_and_property_bodies_share_the_contract() {
    for (name, count) in [
        ("01_called_baselines.jett", 2),
        ("02_owning_clone_fields.jett", 1),
        ("03_rhs_order_and_early_return.jett", 1),
        ("04_exact_machine_state.jett", 1),
        ("09_generic_alias.jett", 1),
        ("10_refined_field_replacement.jett", 1),
        ("22_branch_loop_lifetimes.jett", 1),
        ("25_secret_owned_root.jett", 1),
        ("26_for_loop_update.jett", 1),
        ("27_nested_handler_update.jett", 1),
        ("28_metadata_holder_replacement.jett", 1),
        ("29_inline_owned_parameter.jett", 1),
        ("30_returned_inline_owned_parameter.jett", 1),
        ("31_reflected_iteration_update.jett", 1),
    ] {
        let path = require_admitted(name);
        let report = test_file(&path).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(report.total, count, "{name}");
        assert_eq!(report.passed, count, "{name}");
        assert_eq!(report.failed, 0, "{name}");
        if name == "01_called_baselines.jett" {
            assert!(report.blocks.iter().any(|block| {
                block.is_property && block.iterations.is_some_and(|iterations| iterations > 0)
            }));
        }
    }
}

#[test]
fn field_update_required_comptime_has_an_independent_build_and_execution_gate() {
    let path = require_admitted("23_required_comptime.jett");
    let outcome = run_file_capture_outcome(&path).expect("required comptime field update");
    assert_eq!(outcome.stdout, "baked:9\n");
    assert!(outcome.debug_events.is_empty());
    let report = test_file(&path).expect("required comptime verify body");
    assert_eq!(report.total, 1);
    assert_eq!(report.passed, 1);
    assert_eq!(report.failed, 0);
}

#[test]
fn pending_ancestors_refuse_before_rhs_and_stored_child_depth_is_preserved() {
    for (name, expected_stdout) in [
        ("05_pending_root_preflight.jett", "before:root\n"),
        ("06_pending_intermediate_preflight.jett", "before:inner\n"),
        (
            "08_stored_child_depth_refusal.jett",
            "before:remaining-depth\n",
        ),
    ] {
        let path = require_admitted(name);
        let failure = run_file_capture_outcome(&path)
            .expect_err("pending inspection must not complete successfully");
        assert_eq!(failure.output.stdout, expected_stdout, "{name}");
        assert!(failure.output.debug_events.is_empty(), "{name}");
        assert!(!failure.message.is_empty(), "{name}");
    }
}

#[test]
fn selected_field_write_barriers_reject_at_the_source_boundary() {
    for name in [
        "11_refined_root_rejected.jett",
        "12_refined_intermediate_rejected.jett",
        "13_bitfield_rejected.jett",
        "14_consumed_root_rejected.jett",
        "15_whole_loan_rejected.jett",
        "16_field_loan_rejected.jett",
        "17_rhs_root_consumption_rejected.jett",
        "19_view_alias_rejected.jett",
        "20_unknown_machine_state_rejected.jett",
    ] {
        let path = fixture(name);
        let built = build_file(&path);
        assert!(built.has_errors, "{name}: checked invalid field update");
    }
}

#[test]
fn changing_a_field_invalidates_an_existing_nonzero_fact() {
    // Establish the positive guard before crediting the stale-fact rejection.
    let path = require_admitted("21_guarded_field_control.jett");
    let outcome = run_file_capture_outcome(&path).expect("guarded field control");
    assert_eq!(outcome.stdout, "guarded:4\n");
    let built = build_file(&fixture("18_stale_nonzero_rejected.jett"));
    assert!(
        built.has_errors,
        "stale field nonzero proof survived replacement"
    );
}
