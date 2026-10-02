use super::run_bounded;
use super::suite_options::launcher_for_options;
use jett_driver::BuildOptions;
use jett_driver::native::{
    build_host_executable_with_options, build_host_property_suite_executable_with_options,
    build_host_verify_suite_executable_with_options,
};
use std::fs;

struct NestedCase {
    name: &'static str,
    source: &'static str,
    stdout: &'static str,
    debug: &'static str,
    message: Option<&'static str>,
}

macro_rules! case {
    ($name:literal) => {
        case!($name, None)
    };
    ($name:literal, $message:expr) => {
        NestedCase {
            name: $name,
            source: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/native/reflected_nested/",
                $name,
                ".jett"
            )),
            stdout: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/native/reflected_nested/",
                $name,
                ".stdout"
            )),
            debug: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/native/reflected_nested/",
                $name,
                ".debug"
            )),
            message: $message,
        }
    };
}

const POSITIVE_FALSE: &str = "runtime error: refinement type constraint failed for 'app.Positive'";
const TEXT_FALSE: &str = "runtime error: refinement type constraint failed for 'app.Text'";
const READY_LIST: &str = "runtime error: reflected field refinement: expected a ready list value";
const READY_SET: &str = "runtime error: reflected field refinement: expected a ready set value";
const READY_MAP: &str = "runtime error: reflected field refinement: expected a ready map value";
const READY_OPTIONAL: &str =
    "runtime error: reflected field refinement: expected a ready optional value";
const READY_RESULT: &str =
    "runtime error: reflected field refinement: expected a ready result value";

fn piped_getters(source: &str) -> String {
    let mut count = 0;
    let mut output = String::new();
    for line in source.lines() {
        if let Some(call) = line.strip_prefix("    return type.") {
            if let Some(callee) = call.strip_suffix("(view source, view field)") {
                if callee.starts_with("field_value[")
                    || callee.starts_with("variant_field_value[")
                    || callee.starts_with("machine_field_value[")
                {
                    output.push_str(&format!(
                        "    return source into view type.{callee}(view field)\n"
                    ));
                    count += 1;
                    continue;
                }
            }
        }
        output.push_str(line);
        output.push('\n');
    }
    for (owner, shape, getter) in [
        ("Holder", "record", "field_value"),
        ("Event", "event", "variant_field_value"),
        ("Session", "session", "machine_field_value"),
    ] {
        let direct = format!(
            "type.{getter}[{owner}, list[Positive]](view {shape}_operand(view source), view field_operand(view field))"
        );
        let pipeline = format!(
            "{shape}_operand(view source) into view type.{getter}[{owner}, list[Positive]](view field_operand(view field))"
        );
        if output.contains(&direct) {
            output = output.replace(&direct, &pipeline);
            count += 1;
        }
    }
    assert!(
        count >= 3,
        "fixture did not exercise all three getter pipelines"
    );
    output
}

fn first_getter(source: &str, shape: &str) -> String {
    let original = "string first = summarize(read_record(view record, view record_selector))";
    assert!(
        source.contains(original),
        "terminal fixture needs a selectable first getter"
    );
    source.replace(
        original,
        &format!("string first = summarize(read_{shape}(view {shape}, view {shape}_selector))"),
    )
}

fn later_handler(source: &str) -> String {
    let helpers = r#"function fallback(view stdout: Stdout) returns string:
    Stdout.write(view stdout, "handler\n")
    return "fallback"
function later(view stdout: Stdout) returns result[string, string]:
    Stdout.write(view stdout, "later\n")
    return fail("later")
"#;
    let with_helpers = source.replace("function main(", &format!("{helpers}function main("));
    assert_ne!(source, with_helpers);
    let sentinel = "    string unreachable = \"{later(view stdout) handle error: default fallback(view stdout)}\"";
    let call = if source.contains("    string value = report()") {
        "    string value = report()"
    } else {
        assert!(
            source.contains("    report(view stdout)"),
            "terminal fixture needs its report boundary"
        );
        "    report(view stdout)"
    };
    let guarded = with_helpers.replace(call, &format!("{call}\n{sentinel}"));
    assert_ne!(
        with_helpers, guarded,
        "terminal report sentinel was not inserted"
    );
    guarded
}

fn run_form(
    name: &str,
    source_text: &str,
    stdout: &str,
    debug_text: &str,
    message: Option<&str>,
    suites: bool,
) {
    let stdout = stdout.replace("\r\n", "\n");
    let stdout = stdout.as_str();
    let debug_text = debug_text.replace("\r\n", "\n");
    let debug_text = debug_text.as_str();
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("main.jett");
    fs::write(&source, source_text).unwrap();
    match (jett_driver::run_file_capture_outcome(&source), message) {
        (Ok(actual), None) => {
            assert_eq!(actual.stdout, stdout, "{name}");
            assert_eq!(
                jett_driver::render_debug_events(&actual.debug_events),
                debug_text,
                "{name}"
            );
            assert!(
                actual
                    .debug_events
                    .iter()
                    .all(|event| event.kind == jett_driver::DebugEventKind::Trace),
                "{name}"
            );
        }
        (Err(actual), Some(message)) => {
            assert_eq!(actual.message, message, "{name}");
            assert_eq!(actual.output.stdout, stdout, "{name}");
            assert_eq!(
                jett_driver::render_debug_events(&actual.output.debug_events),
                debug_text,
                "{name}"
            );
            assert!(
                actual
                    .output
                    .debug_events
                    .iter()
                    .all(|event| event.kind == jett_driver::DebugEventKind::Trace),
                "{name}"
            );
        }
        (actual, expected) => panic!("{name}: {actual:?}; expected terminal message {expected:?}"),
    }
    let mut binaries = Vec::new();
    let mut suite_binaries = Vec::new();
    for release in [false, true] {
        let binary = directory.path().join(format!("nested_{release}.exe"));
        build_host_executable_with_options(
            &source,
            launcher_for_options(release),
            &binary,
            BuildOptions { release },
        )
        .unwrap_or_else(|error| panic!("{name}: {error}"));
        binaries.push((binary, release));
        if suites {
            let verify = directory
                .path()
                .join(format!("nested_verify_{release}.exe"));
            let property = directory
                .path()
                .join(format!("nested_property_{release}.exe"));
            build_host_verify_suite_executable_with_options(
                &source,
                launcher_for_options(release),
                &verify,
                BuildOptions { release },
            )
            .unwrap_or_else(|error| panic!("{name}: {error}"));
            build_host_property_suite_executable_with_options(
                &source,
                launcher_for_options(release),
                &property,
                BuildOptions { release },
            )
            .unwrap_or_else(|error| panic!("{name}: {error}"));
            suite_binaries.extend([verify, property]);
        }
    }
    fs::remove_file(&source).unwrap();
    for (binary, release) in binaries {
        let actual = run_bounded(&binary, directory.path());
        // 72 is the cleanup-override status; it must never replace terminal 71.
        assert_eq!(
            actual.status.code(),
            Some(if message.is_some() { 71 } else { 0 }),
            "{name}: {actual:?}"
        );
        assert_eq!(actual.stdout, stdout.as_bytes(), "{name}");
        let mut stderr = if release {
            String::new()
        } else {
            debug_text.to_owned()
        };
        if let Some(message) = message {
            stderr.push_str(message);
            stderr.push('\n');
        }
        assert_eq!(actual.stderr, stderr.as_bytes(), "{name}: {actual:?}");
    }
    for binary in suite_binaries {
        let actual = run_bounded(&binary, directory.path());
        assert_eq!(actual.status.code(), Some(0), "{name}: {actual:?}");
        assert!(actual.stdout.is_empty(), "{name}: {actual:?}");
        assert!(actual.stderr.is_empty(), "{name}: {actual:?}");
    }
}

fn run_case(case: NestedCase, all_getters: bool, suites: bool) {
    let shapes: &[&str] = if all_getters {
        &["record", "event", "session"]
    } else {
        &[""]
    };
    for shape in shapes {
        let source = if all_getters {
            first_getter(case.source, shape)
        } else {
            case.source.to_owned()
        };
        let source = if case.message.is_some() {
            later_handler(&source)
        } else {
            source
        };
        run_form(
            &format!("{}_{shape}_direct", case.name),
            &source,
            case.stdout,
            case.debug,
            case.message,
            suites,
        );
        let piped = piped_getters(&source);
        run_form(
            &format!("{}_{shape}_pipeline", case.name),
            &piped,
            case.stdout,
            case.debug,
            case.message,
            suites,
        );
    }
}

#[test]
fn native_nested_reflected_ready_and_empty_builtin_wrappers_match_reference() {
    for case in [
        case!("list_ready_success"),
        case!("list_empty_success"),
        case!("set_ready_success"),
        case!("set_empty_success"),
        case!("map_ready_success"),
        case!("map_empty_success"),
        case!("optional_ready_success"),
        case!("optional_none_success"),
        case!("result_ok_success"),
        case!("result_fail_success"),
        case!("repeated_map_refinement_comparator_control"),
        case!("repeated_result_refinement_comparator_control"),
        case!("inactive_secret_arm_success"),
    ] {
        run_case(case, false, false);
    }
}

#[test]
fn native_nested_reflected_proof_prefixes_owned_unicode_and_operand_order() {
    for case in [
        case!("proof_prefixes"),
        case!("owned_proof_prefixes"),
        case!("owned_unicode_independence"),
        case!("recursive_wrappers"),
        case!("operand_order"),
    ] {
        assert!(
            case.source
                .as_bytes()
                .windows("Agent-λ🙂".len())
                .any(|slice| slice == "Agent-λ🙂".as_bytes())
                || matches!(case.name, "proof_prefixes" | "operand_order")
        );
        run_case(case, false, false);
    }
}

#[test]
fn native_nested_reflected_preflight_precedes_all_predicates() {
    for case in [
        case!("list_later_optional_pending", Some(READY_OPTIONAL)),
        case!("map_later_optional_pending", Some(READY_OPTIONAL)),
        case!("optional_inner_list_pending", Some(READY_LIST)),
        case!("result_inner_map_pending", Some(READY_MAP)),
        case!(
            "later_occupied_secret_refusal",
            Some(
                "runtime error: reflected field refinement: cannot establish new invariants beneath secret"
            )
        ),
    ] {
        run_case(case, true, false);
    }
}

#[test]
fn native_nested_reflected_predicates_stop_at_first_failure_in_value_order() {
    for case in [
        case!("list_middle_false", Some(POSITIVE_FALSE)),
        case!("list_owned_text_false", Some(TEXT_FALSE)),
        case!("optional_present_false", Some(POSITIVE_FALSE)),
        case!("result_occupied_false", Some(POSITIVE_FALSE)),
        case!("map_key_value_order_false", Some(POSITIVE_FALSE)),
        case!(
            "list_child_pending",
            Some(
                "runtime error: error evaluating refinement constraint for 'app.Positive': unsupported binary operation: pending(9) Gt 0"
            )
        ),
    ] {
        run_case(case, true, false);
    }
}

#[test]
fn native_nested_reflected_exact_pending_schemas_preserve_both_depths() {
    for case in [
        case!("list_exact_pending_1"),
        case!("list_exact_pending_2"),
        case!("set_exact_pending_1"),
        case!("set_exact_pending_2"),
        case!("map_exact_pending_1"),
        case!("map_exact_pending_2"),
        case!("optional_exact_pending_1"),
        case!("optional_exact_pending_2"),
        case!("result_exact_pending_1"),
        case!("result_exact_pending_2"),
        case!("child_exact_pending_1"),
        case!("child_exact_pending_2"),
        case!("declared_root_ancestor_pending"),
        case!("child_ancestor_ready"),
    ] {
        run_case(case, false, false);
    }
}

#[test]
fn native_nested_reflected_changed_pending_wrappers_do_not_inherit_child_exactness() {
    for case in [
        case!("list_outer_pending_1", Some(READY_LIST)),
        case!("list_outer_pending_2", Some(READY_LIST)),
        case!("set_outer_pending_1", Some(READY_SET)),
        case!("set_outer_pending_2", Some(READY_SET)),
        case!("map_outer_pending_1", Some(READY_MAP)),
        case!("map_outer_pending_2", Some(READY_MAP)),
        case!("optional_outer_pending_1", Some(READY_OPTIONAL)),
        case!("optional_outer_pending_2", Some(READY_OPTIONAL)),
        case!("result_outer_pending_1", Some(READY_RESULT)),
        case!("result_outer_pending_2", Some(READY_RESULT)),
        case!("child_ancestor_pending_1", Some(READY_LIST)),
        case!("child_ancestor_pending_2", Some(READY_LIST)),
    ] {
        run_case(case, true, false);
    }
}

#[test]
fn native_nested_reflected_original_selector_errors_precede_container_preflight() {
    for case in [
        case!(
            "selector_wrong_owner_record",
            Some(
                "runtime error: type.field_value: field metadata belongs to 'app.Other', expected 'app.Holder'"
            )
        ),
        case!(
            "selector_wrong_owner_event",
            Some(
                "runtime error: type.variant_field_value: field metadata belongs to 'app.Other', expected 'app.Event.content'"
            )
        ),
        case!(
            "selector_wrong_owner_session",
            Some(
                "runtime error: type.machine_field_value: field metadata belongs to 'app.Other', expected 'app.Session.content'"
            )
        ),
        case!(
            "selector_wrong_member_event",
            Some(
                "runtime error: type.variant_field_value: field metadata belongs to 'app.Event.mirror', expected 'app.Event.content'"
            )
        ),
        case!(
            "selector_wrong_member_session",
            Some(
                "runtime error: type.machine_field_value: field metadata belongs to 'app.Session.mirror', expected 'app.Session.content'"
            )
        ),
        case!(
            "selector_wrong_request_record",
            Some(
                "runtime error: type.field_value: field 'text' has type 'string', requested 'list[app.Positive]'"
            )
        ),
        case!(
            "selector_wrong_request_event",
            Some(
                "runtime error: type.variant_field_value: field 'text' has type 'string', requested 'list[app.Positive]'"
            )
        ),
        case!(
            "selector_wrong_request_session",
            Some(
                "runtime error: type.machine_field_value: field 'text' has type 'string', requested 'list[app.Positive]'"
            )
        ),
    ] {
        run_case(case, false, false);
    }
}

#[test]
fn native_nested_reflected_closed_comptime_verify_and_property_match_in_both_profiles() {
    for case in [
        case!("pure_proof_prefixes"),
        case!("pure_owned_proof_prefixes"),
        case!("pure_recursive_wrappers"),
    ] {
        run_case(case, false, true);
    }
}

fn run_root_case(case: NestedCase) {
    let checked_constructor = "    Record source = Record(values: seed()) handle error:\n        Stdout.write(view stdout, error)\n        return nothing";
    let constructor = if case.source.contains(checked_constructor) {
        checked_constructor
    } else {
        "    Record source = Record(values: seed())"
    };
    assert!(case.source.contains(constructor));
    for shape in ["record", "event", "machine"] {
        let mut source = case.source.to_owned();
        let mut debug = case.debug.to_owned();
        if shape != "record" {
            let (owner, construct, selector) = if shape == "event" {
                (
                    "Event",
                    "Event.content(seed())",
                    "event_selector(view source)",
                )
            } else {
                (
                    "Session",
                    "Session(content, seed())",
                    "machine_selector(view source)",
                )
            };
            source = source
                .replace(constructor, &format!("    {owner} source = {construct}"))
                .replace(
                    "TypeField field = record_selector()",
                    &format!("TypeField field = {selector}"),
                )
                .replace(
                    "extracted = record_direct(view source, view field)",
                    &format!("extracted = {shape}_direct(view source, view field)"),
                );
            let value_prefix = if shape == "event" {
                "app.Event.content("
            } else {
                "app.Session@content("
            };
            debug = debug.replace(
                "trace source: app.Record = app.Record(values: ",
                &format!("trace source: app.{owner} = {value_prefix}"),
            );
        }
        if case.message.is_some() {
            source = later_handler(&source);
        }
        for pipeline in [false, true] {
            let text = if pipeline {
                source.replace(
                    &format!("extracted = {shape}_direct(view source, view field)"),
                    &format!("extracted = {shape}_pipe(view source, view field)"),
                )
            } else {
                source.clone()
            };
            run_form(
                &format!("{}_{shape}_{pipeline}", case.name),
                &text,
                case.stdout,
                &debug,
                case.message,
                false,
            );
        }
    }
}

#[test]
fn native_nested_reflected_root_to_generic_base_checks_readiness_without_leaf_rechecks() {
    for case in [
        case!("root_base_list_depth0"),
        case!("root_base_list_depth1", Some(READY_LIST)),
        case!("root_base_list_depth2", Some(READY_LIST)),
        case!("root_base_set_depth0"),
        case!("root_base_set_depth1", Some(READY_SET)),
        case!("root_base_set_depth2", Some(READY_SET)),
        case!("root_base_map_depth0"),
        case!("root_base_map_depth1", Some(READY_MAP)),
        case!("root_base_map_depth2", Some(READY_MAP)),
        case!("root_base_optional_depth0"),
        case!("root_base_optional_depth1", Some(READY_OPTIONAL)),
        case!("root_base_optional_depth2", Some(READY_OPTIONAL)),
        case!("root_base_result_depth0"),
        case!("root_base_result_depth1", Some(READY_RESULT)),
        case!("root_base_result_depth2", Some(READY_RESULT)),
        case!("root_generic_exact_pending2"),
        case!("root_named_exact_pending2"),
        case!("root_to_plain_pending2"),
        case!("root_new_sibling_pending2", Some(READY_LIST)),
        case!("root_shared_named_ancestor_pending2"),
    ] {
        run_root_case(case);
    }
}

#[test]
fn native_nested_reflected_nominal_argument_facts_preserve_pending_leaf_proofs() {
    for case in [
        case!("root_nominal_ordinary_element_ready"),
        case!("root_nominal_holder_positive_ready"),
        case!("root_nominal_unused_positive_ready"),
        case!("root_nominal_unused_alias_ready"),
        case!("root_nominal_unused_plain_ready"),
        case!("root_nominal_ordinary_element_pending2"),
        case!("root_nominal_holder_positive_pending2", Some(READY_LIST)),
        case!("root_nominal_unused_positive_pending2", Some(READY_LIST)),
        case!("root_nominal_unused_alias_pending2", Some(READY_LIST)),
        case!("root_nominal_unused_plain_pending2"),
    ] {
        run_root_case(case);
    }
}

// TypeField index/name/type forgery cannot be expressed in admitted source.
// Existing HIR/MIR corrupted-selector tests cover those refusal boundaries.
// Changes beneath callable/nominal/erased interface types remain outside the
// ready builtin relation. Root refinement over an existing secret/nominal base
// is a separate unaudited contract; these fixtures do not claim its closure.
