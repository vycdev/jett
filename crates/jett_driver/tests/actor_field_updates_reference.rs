//! Actor persistent field updates from checked Source.
use std::path::{Path, PathBuf};

use jett_driver::{build_file, lower_file_for_backend, run_file_capture_outcome};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native")
        .join(name)
}

fn assert_main(name: &str, stdout: &str) {
    let path = fixture(name);
    let checked = build_file(&path);
    assert!(!checked.has_errors, "{name}: {:?}", checked.diagnostics);
    let outcome = run_file_capture_outcome(&path)
        .unwrap_or_else(|failure| panic!("{name}: {}", failure.message));
    assert_eq!(outcome.stdout, stdout, "{name}");
    assert!(outcome.debug_events.is_empty(), "{name}: {outcome:?}");
    lower_file_for_backend(&path)
        .unwrap_or_else(|error| panic!("{name}: checked actor lowering: {error}"));
}

#[test]
fn original_actor_handler_field_update_executes_from_unchanged_source() {
    assert_main(
        "field_update_reference/24_actor_handler_future.jett",
        "actor:1:9\n",
    );
}

#[test]
fn actor_state_updates_keep_owning_values_instances_and_message_bindings_independent() {
    for (name, stdout) in [
        (
            "01_nested_owners_independent_actors.jett",
            "first:4:18:left!!:2|1:9:start:9|sender:2:7:left\nafter:4:18:left!!:2|2:7:right:9\n",
        ),
        (
            "10_nonshadowing_message_and_helper.jett",
            "message:110|1:initial|9:helper|210|9:helper\n",
        ),
        (
            "03_early_respond_preserves_old_field.jett",
            "early:4:4:9:9:9\n",
        ),
        (
            "16_nonresponding_return_preserves_state.jett",
            "void-return:7:changed|7:changed|9:selected|1:initial\n",
        ),
    ] {
        assert_main(&format!("actor_field_updates/{name}"), stdout);
    }
}

#[test]
fn actor_field_updates_execute_in_verify_and_property_bodies() {
    let report = jett_driver::test_file(&fixture(
        "actor_field_updates/03_early_respond_preserves_old_field.jett",
    ))
    .expect("actor field update verification bodies");
    assert_eq!(report.total, 2);
    assert_eq!(report.passed, 2);
    assert_eq!(report.failed, 0);
    assert!(
        report
            .blocks
            .iter()
            .any(|block| block.is_property && block.iterations.is_some_and(|count| count > 0))
    );
}

#[test]
fn actor_field_updates_reject_consumed_readonly_and_borrowed_roots() {
    for name in [
        "04_consumed_message_root_rejected.jett",
        "05_persistent_state_not_restored_rejected.jett",
        "06_view_message_root_rejected.jett",
        "07_immutable_state_root_rejected.jett",
        "08_live_whole_state_loan_rejected.jett",
        "09_live_state_field_loan_rejected.jett",
        "12_persistent_alias_owner_replacement_rejected.jett",
    ] {
        let checked = build_file(&fixture(&format!("actor_field_updates/{name}")));
        assert!(
            checked.has_errors,
            "{name}: invalid actor ownership admitted"
        );
        let errors = checked
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == jett_diagnostics::Severity::Error)
            .collect::<Vec<_>>();
        assert!(
            errors
                .iter()
                .all(|diagnostic| (400..500).contains(&diagnostic.code.code())),
            "{name}: expected ownership diagnostic, got {:?}",
            checked.diagnostics
        );
    }
}

#[test]
fn actor_message_parameters_preserve_the_language_no_shadowing_rule() {
    let checked = build_file(&fixture(
        "actor_field_updates/02_shadowed_message_and_ordinary_helper.jett",
    ));
    assert!(checked.has_errors);
    let errors = checked
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == jett_diagnostics::Severity::Error)
        .collect::<Vec<_>>();
    assert!(
        errors
            .iter()
            .any(|diagnostic| diagnostic.code.code() == 201),
        "{:?}",
        checked.diagnostics
    );
}

#[test]
fn actor_persistent_views_keep_their_exact_backing_state_through_independent_updates() {
    assert_main(
        "actor_field_updates/11_persistent_aliases_and_independent_field.jett",
        "before:2:5:2:5:2|2:21:2:21:10\nafter:2:5:2:5:19|2:21:2:21:32\n",
    );
}

#[test]
fn actor_respond_is_a_terminal_handle_exit_without_an_unreachable_default_marker() {
    assert_main(
        "actor_field_updates/13_respond_is_terminal_handle_exit.jett",
        "early:4:4:9:9:9\n",
    );
}

#[test]
fn actor_projected_persistent_views_replay_their_checked_field_path() {
    assert_main(
        "actor_field_updates/14_projected_persistent_alias.jett",
        concat!(
            "before-first:envelope-2:102:2:inner-2:outer-2:1002|2:inner-2|2:inner-2|2\n",
            "before-second:envelope-7:107:7:inner-7:outer-7:1007|7:inner-7|7:inner-7|7\n",
            "after-first:envelope-2:102:2:inner-2:outer-2:1002|2:inner-2|2:inner-2|13\n",
            "after-second:envelope-7:107:7:inner-7:outer-7:1007|7:inner-7|7:inner-7|15\n",
        ),
    );
}

#[test]
fn existing_actor_message_receiver_handlers_preserve_checked_evaluation_order() {
    let path = fixture("nested_actor_message_handle.jett");
    let checked = build_file(&path);
    assert!(!checked.has_errors, "{:?}", checked.diagnostics);
    let outcome = run_file_capture_outcome(&path).expect("existing handled actor receiver");
    assert!(outcome.stdout.is_empty());
    lower_file_for_backend(&path).expect("existing handled actor receiver lowers");
}

#[test]
fn actor_owning_handle_reply_is_acquired_before_state_publication() {
    assert_main(
        "actor_field_updates/15_owning_handle_reply.jett",
        "reply:2:5|1:9|3\n",
    );
}
