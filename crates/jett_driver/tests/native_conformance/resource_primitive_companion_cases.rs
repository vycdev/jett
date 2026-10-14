//! Additional exact Source controls; the original occupied corpus stays intact.
use super::cases::{Case, Event, ReferenceOutcome, Script};

const PRIMITIVES: &str = include_str!(
    "../../../jett_comptime/src/resource_execution/fixtures/36_owned_struct_primitive_companions.jett"
);
const ALIASES: &str = include_str!(
    "../../../jett_comptime/src/resource_execution/fixtures/37_owned_struct_alias_companions.jett"
);
const PENDING: &str = include_str!(
    "../../../jett_comptime/src/resource_execution/fixtures/38_owned_struct_pending_int64_transport.jett"
);
const INSPECTION: &str = include_str!(
    "../../../jett_comptime/src/resource_execution/fixtures/39_owned_struct_pending_int64_inspection.jett"
);
const PENDING_PRIMITIVES: &str = include_str!(
    "../../../jett_comptime/src/resource_execution/fixtures/40_owned_struct_pending_companions.jett"
);
const PREFIX: &str = include_str!(
    "../../../jett_comptime/src/resource_execution/fixtures/41_owned_struct_pending_prior_failure.jett"
);
const MATRIX: &str = include_str!(
    "../../../jett_comptime/src/resource_execution/fixtures/42_owned_struct_pending_primitive_matrix.jett"
);
const UNIT_INSPECTION: &str = include_str!(
    "../../../jett_comptime/src/resource_execution/fixtures/43_owned_struct_pending_nothing_inspection.jett"
);
const TEXT_INSPECTION: &str = include_str!(
    "../../../jett_comptime/src/resource_execution/fixtures/44_owned_struct_pending_string_inspection.jett"
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
const fn panic(
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
const fn body_error(
    name: &'static str,
    source: &'static str,
    script: &'static [Script],
    events: &'static [Event],
    message: &'static str,
) -> Case {
    Case {
        name,
        source,
        script,
        events,
        reference: ReferenceOutcome::Error(message),
        body: 1,
        cleanup: 0,
        channel: 1,
        ordinary_message: message.as_bytes(),
        resource_message: b"",
        exit: 71,
    }
}

pub(super) const CASES: &[Case] = &[
    body_error(
        "pending_unit_inspection_error",
        UNIT_INSPECTION,
        &[Script::Construct(1107)],
        &[Event::Constructed(1107), Event::Finalized(1107)],
        "unsupported binary operation: pending(nothing) Eq nothing",
    ),
    clean(
        "pending_unit_inspection_factory_failure_ready",
        UNIT_INSPECTION,
        &[Script::ConstructFail(1107, "declined")],
        &[Event::ConstructionFailed(1107)],
    ),
    body_error(
        "pending_text_inspection_error",
        TEXT_INSPECTION,
        &[Script::Construct(1108)],
        &[Event::Constructed(1108), Event::Finalized(1108)],
        "unsupported binary operation: pending(retained) Eq retained",
    ),
    clean(
        "pending_text_inspection_factory_failure_ready",
        TEXT_INSPECTION,
        &[Script::ConstructFail(1108, "declined")],
        &[Event::ConstructionFailed(1108)],
    ),
    clean(
        "primitive_companions_ready",
        PRIMITIVES,
        &[Script::Construct(1101)],
        &[Event::Constructed(1101), Event::Finalized(1101)],
    ),
    clean(
        "primitive_companions_factory_fail",
        PRIMITIVES,
        &[Script::ConstructFail(1101, "declined")],
        &[Event::ConstructionFailed(1101)],
    ),
    panic(
        "primitive_companions_cleanup_panic",
        PRIMITIVES,
        &[Script::FinalizerPanic(1101)],
        &[Event::Constructed(1101), Event::Finalized(1101)],
    ),
    clean(
        "alias_companions_ready",
        ALIASES,
        &[Script::Construct(1102)],
        &[Event::Constructed(1102), Event::Finalized(1102)],
    ),
    clean(
        "alias_companions_factory_fail",
        ALIASES,
        &[Script::ConstructFail(1102, "declined")],
        &[Event::ConstructionFailed(1102)],
    ),
    panic(
        "alias_companions_cleanup_panic",
        ALIASES,
        &[Script::FinalizerPanic(1102)],
        &[Event::Constructed(1102), Event::Finalized(1102)],
    ),
    clean(
        "pending_int64_ready",
        PENDING,
        &[Script::Construct(1103)],
        &[Event::Constructed(1103), Event::Finalized(1103)],
    ),
    clean(
        "pending_int64_factory_fail",
        PENDING,
        &[Script::ConstructFail(1103, "declined")],
        &[Event::ConstructionFailed(1103)],
    ),
    panic(
        "pending_int64_cleanup_panic",
        PENDING,
        &[Script::FinalizerPanic(1103)],
        &[Event::Constructed(1103), Event::Finalized(1103)],
    ),
    clean(
        "pending_primitives_ready",
        PENDING_PRIMITIVES,
        &[Script::Construct(1104)],
        &[Event::Constructed(1104), Event::Finalized(1104)],
    ),
    clean(
        "pending_primitives_factory_fail",
        PENDING_PRIMITIVES,
        &[Script::ConstructFail(1104, "declined")],
        &[Event::ConstructionFailed(1104)],
    ),
    panic(
        "pending_primitives_cleanup_panic",
        PENDING_PRIMITIVES,
        &[Script::FinalizerPanic(1104)],
        &[Event::Constructed(1104), Event::Finalized(1104)],
    ),
    clean(
        "pending_matrix_ready",
        MATRIX,
        &[Script::Construct(1106)],
        &[Event::Constructed(1106), Event::Finalized(1106)],
    ),
    clean(
        "pending_matrix_factory_fail",
        MATRIX,
        &[Script::ConstructFail(1106, "declined")],
        &[Event::ConstructionFailed(1106)],
    ),
    panic(
        "pending_matrix_cleanup_panic",
        MATRIX,
        &[Script::FinalizerPanic(1106)],
        &[Event::Constructed(1106), Event::Finalized(1106)],
    ),
    body_error(
        "pending_inspection_error",
        INSPECTION,
        &[Script::Construct(1103)],
        &[Event::Constructed(1103), Event::Finalized(1103)],
        "unsupported binary operation: pending(17) Eq 17",
    ),
    clean(
        "pending_inspection_factory_failure_ready",
        INSPECTION,
        &[Script::ConstructFail(1103, "declined")],
        &[Event::ConstructionFailed(1103)],
    ),
    body_error(
        "pending_prefix_error",
        PREFIX,
        &[Script::Construct(1105)],
        &[Event::Constructed(1105), Event::Finalized(1105)],
        "list.__remove_at: index -1 out of bounds",
    ),
    clean(
        "pending_prefix_factory_failure_ready",
        PREFIX,
        &[Script::ConstructFail(1105, "declined")],
        &[Event::ConstructionFailed(1105)],
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
