//! New original Sources/scripts; no row is runtime authority or accepted evidence.
use super::cases::{Case, Event, ReferenceOutcome, Script};

const ORIGINAL: &str = include_str!(
    "../../../jett_codegen_cranelift/src/emit/resource_execution/replacement/original_optional_assignment.jett"
);
const OPTIONAL: &str = include_str!(
    "../../../jett_codegen_cranelift/src/emit/resource_execution/replacement/optional_transitions.jett"
);
const RESULT: &str = include_str!(
    "../../../jett_codegen_cranelift/src/emit/resource_execution/replacement/result_transitions.jett"
);
const PRIMITIVE: &str = include_str!(
    "../../../jett_codegen_cranelift/src/emit/resource_execution/replacement/primitive_failure.jett"
);
const LOOP: &str = include_str!(
    "../../../jett_codegen_cranelift/src/emit/resource_execution/replacement/conditional_optional.jett"
);
const CONDITIONAL: &str = include_str!(
    "../../../jett_codegen_cranelift/src/emit/resource_execution/replacement/conditional_result.jett"
);

const fn clean(
    name: &'static str,
    source: &'static str,
    script: &'static [Script],
    events: &'static [Event],
) -> Case {
    Case {
        name,
        source,
        script,
        events,
        reference: ReferenceOutcome::Clean,
        body: 0,
        cleanup: 0,
        channel: 0,
        ordinary_message: b"",
        resource_message: b"",
        exit: 0,
    }
}
const fn cleanup_panic(
    name: &'static str,
    source: &'static str,
    script: &'static [Script],
    events: &'static [Event],
) -> Case {
    Case {
        name,
        source,
        script,
        events,
        reference: ReferenceOutcome::CleanupPanic("selected test resource cleanup panic"),
        body: 255,
        cleanup: 255,
        channel: 3,
        ordinary_message: b"",
        resource_message: b"native Resource cleanup failed",
        exit: 71,
    }
}
pub(super) const CASES: &[Case] = &[
    clean("sum_original_none_ready", ORIGINAL, &[], &[]),
    clean(
        "sum_optional_ready",
        OPTIONAL,
        &[
            Script::Construct(2501),
            Script::Construct(2502),
            Script::Construct(2503),
            Script::Borrow(2503, 7),
        ],
        &[
            Event::Constructed(2501),
            Event::Finalized(2501),
            Event::Constructed(2502),
            Event::Constructed(2503),
            Event::Finalized(2502),
            Event::Borrowed(2503),
            Event::Finalized(2503),
        ],
    ),
    clean(
        "sum_optional_rhs_failure",
        OPTIONAL,
        &[
            Script::Construct(2501),
            Script::Construct(2502),
            Script::ConstructFail(2503, "rhs sentinel"),
        ],
        &[
            Event::Constructed(2501),
            Event::Finalized(2501),
            Event::Constructed(2502),
            Event::ConstructionFailed(2503),
            Event::Finalized(2502),
        ],
    ),
    cleanup_panic(
        "sum_optional_old_to_none_cleanup_failure",
        OPTIONAL,
        &[Script::FinalizerPanic(2501)],
        &[Event::Constructed(2501), Event::Finalized(2501)],
    ),
    cleanup_panic(
        "sum_optional_old_to_some_cleanup_failure",
        OPTIONAL,
        &[
            Script::Construct(2501),
            Script::FinalizerPanic(2502),
            Script::Construct(2503),
        ],
        &[
            Event::Constructed(2501),
            Event::Finalized(2501),
            Event::Constructed(2502),
            Event::Constructed(2503),
            Event::Finalized(2502),
            Event::Finalized(2503),
        ],
    ),
    cleanup_panic(
        "sum_optional_both_cleanup_failure",
        OPTIONAL,
        &[
            Script::Construct(2501),
            Script::FinalizerPanic(2502),
            Script::FinalizerPanic(2503),
        ],
        &[
            Event::Constructed(2501),
            Event::Finalized(2501),
            Event::Constructed(2502),
            Event::Constructed(2503),
            Event::Finalized(2502),
            Event::Finalized(2503),
        ],
    ),
    cleanup_panic(
        "sum_optional_incoming_late_cleanup_failure",
        OPTIONAL,
        &[
            Script::Construct(2501),
            Script::Construct(2502),
            Script::FinalizerPanic(2503),
            Script::Borrow(2503, 7),
        ],
        &[
            Event::Constructed(2501),
            Event::Finalized(2501),
            Event::Constructed(2502),
            Event::Constructed(2503),
            Event::Finalized(2502),
            Event::Borrowed(2503),
            Event::Finalized(2503),
        ],
    ),
    clean(
        "sum_result_ready",
        RESULT,
        &[
            Script::Construct(2601),
            Script::Construct(2602),
            Script::Construct(2603),
            Script::Borrow(2603, 7),
        ],
        &[
            Event::Constructed(2601),
            Event::Finalized(2601),
            Event::Constructed(2602),
            Event::Constructed(2603),
            Event::Finalized(2602),
            Event::Borrowed(2603),
            Event::Finalized(2603),
        ],
    ),
    clean(
        "sum_result_rhs_failure",
        RESULT,
        &[
            Script::Construct(2601),
            Script::Construct(2602),
            Script::ConstructFail(2603, "rhs sentinel"),
        ],
        &[
            Event::Constructed(2601),
            Event::Finalized(2601),
            Event::Constructed(2602),
            Event::ConstructionFailed(2603),
            Event::Finalized(2602),
        ],
    ),
    cleanup_panic(
        "sum_result_old_to_fail_cleanup_failure",
        RESULT,
        &[Script::FinalizerPanic(2601)],
        &[Event::Constructed(2601), Event::Finalized(2601)],
    ),
    cleanup_panic(
        "sum_result_old_to_ok_cleanup_failure",
        RESULT,
        &[
            Script::Construct(2601),
            Script::FinalizerPanic(2602),
            Script::Construct(2603),
        ],
        &[
            Event::Constructed(2601),
            Event::Finalized(2601),
            Event::Constructed(2602),
            Event::Constructed(2603),
            Event::Finalized(2602),
            Event::Finalized(2603),
        ],
    ),
    cleanup_panic(
        "sum_result_both_cleanup_failure",
        RESULT,
        &[
            Script::Construct(2601),
            Script::FinalizerPanic(2602),
            Script::FinalizerPanic(2603),
        ],
        &[
            Event::Constructed(2601),
            Event::Finalized(2601),
            Event::Constructed(2602),
            Event::Constructed(2603),
            Event::Finalized(2602),
            Event::Finalized(2603),
        ],
    ),
    cleanup_panic(
        "sum_result_incoming_late_cleanup_failure",
        RESULT,
        &[
            Script::Construct(2601),
            Script::Construct(2602),
            Script::FinalizerPanic(2603),
            Script::Borrow(2603, 7),
        ],
        &[
            Event::Constructed(2601),
            Event::Finalized(2601),
            Event::Constructed(2602),
            Event::Constructed(2603),
            Event::Finalized(2602),
            Event::Borrowed(2603),
            Event::Finalized(2603),
        ],
    ),
    clean(
        "sum_primitive_ready",
        PRIMITIVE,
        &[Script::Construct(2701)],
        &[Event::Constructed(2701), Event::Finalized(2701)],
    ),
    clean(
        "sum_loop_ready",
        LOOP,
        &[
            Script::Construct(2801),
            Script::Construct(2802),
            Script::Borrow(2802, 7),
        ],
        &[
            Event::Constructed(2801),
            Event::Finalized(2801),
            Event::Constructed(2802),
            Event::Borrowed(2802),
            Event::Finalized(2802),
        ],
    ),
    clean(
        "sum_conditional_result_ready",
        CONDITIONAL,
        &[
            Script::Construct(2901),
            Script::Construct(2902),
            Script::Borrow(2902, 7),
        ],
        &[
            Event::Constructed(2901),
            Event::Constructed(2902),
            Event::Finalized(2901),
            Event::Borrowed(2902),
            Event::Finalized(2902),
        ],
    ),
    clean(
        "sum_conditional_result_fail_ok",
        CONDITIONAL,
        &[
            Script::ConstructFail(2901, "first"),
            Script::Construct(2902),
            Script::Borrow(2902, 7),
        ],
        &[
            Event::ConstructionFailed(2901),
            Event::Constructed(2902),
            Event::Borrowed(2902),
            Event::Finalized(2902),
        ],
    ),
    clean(
        "sum_conditional_result_ok_fail",
        CONDITIONAL,
        &[
            Script::Construct(2901),
            Script::ConstructFail(2902, "second"),
        ],
        &[
            Event::Constructed(2901),
            Event::ConstructionFailed(2902),
            Event::Finalized(2901),
        ],
    ),
    clean(
        "sum_conditional_result_fail_fail",
        CONDITIONAL,
        &[
            Script::ConstructFail(2901, "first"),
            Script::ConstructFail(2902, "second"),
        ],
        &[
            Event::ConstructionFailed(2901),
            Event::ConstructionFailed(2902),
        ],
    ),
];
pub(super) fn ready_for(first: &Case) -> &'static Case {
    CASES
        .iter()
        .find(|case| {
            case.source.as_bytes() == first.source.as_bytes() && case.name.ends_with("_ready")
        })
        .unwrap()
}
