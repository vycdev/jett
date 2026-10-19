use super::local_view_aliases::{ProjectedOutcome, run_case};

#[test]
fn native_mutable_optional_sum_preserves_named_argument_order_and_terminal_cleanup() {
    run_case(
        "mutable_sum_optional_failure",
        include_str!(
            "ordinary_borrowed_sums_mutable/13_mutable_optional_named_handle_failure.jett"
        ),
        ProjectedOutcome::RuntimeFailure {
            stdout: concat!(
                "before:optional:8\nprefix:optional:5\nlater:optional\n",
                "default:optional\nfailure:optional\n",
            ),
            message: "runtime error: list.__remove_at: index -1 out of bounds",
        },
    );
}
#[test]
fn native_mutable_result_sum_preserves_named_argument_order_and_terminal_cleanup() {
    run_case(
        "mutable_sum_result_failure",
        include_str!("ordinary_borrowed_sums_mutable/14_mutable_result_named_handle_failure.jett"),
        ProjectedOutcome::RuntimeFailure {
            stdout: concat!(
                "before:result:8\nprefix:result:12\nlater:result\n",
                "default:result\nfailure:result\n",
            ),
            message: "runtime error: list.__remove_at: index -1 out of bounds",
        },
    );
}

#[test]
fn native_mutable_sum_backing_keeps_original_scoped_forwarded_behaviour() {
    run_case(
        "mutable_sum_owned",
        include_str!("ordinary_borrowed_sums_mutable/03_mutable_owned_scoped_forwarded.jett"),
        ProjectedOutcome::Success(concat!(
            "optional:nested:5\noptional:5:18:15:5\n",
            "result:nested:12\nresult:12:29:36:12\ngeneric:1:1\n"
        )),
    );
}
#[test]
fn native_mutable_sum_backing_keeps_original_copy_destinations_and_owning_unwrap() {
    run_case(
        "mutable_sum_copied",
        include_str!("ordinary_borrowed_sums_mutable/15_mutable_backing_copied_payloads.jett"),
        ProjectedOutcome::Success("integer:4:3\nstring:changed:kept\n"),
    );
}
#[test]
fn native_copied_payload_survives_mutable_sum_shell_replacement() {
    run_case(
        "mutable_sum_replaced",
        include_str!(
            "ordinary_borrowed_sums_mutable/16_copied_payload_survives_shell_replacement.jett"
        ),
        ProjectedOutcome::Success("integer:3:9\nstring:kept:new\n"),
    );
}
