//! Shared original Source observations. These rows grant no runtime authority.
use super::cases::{Case, Event, ReferenceOutcome, Script};
const SINGLE: &str = include_str!(
    "../../../jett_comptime/src/resource_execution/fixtures/33_owned_struct_scope_drop_entry.jett"
);
const PAIR: &str = include_str!(
    "../../../jett_comptime/src/resource_execution/fixtures/34_owned_struct_reverse_entry.jett"
);
const PREFIX: &str = include_str!(
    "../../../jett_comptime/src/resource_execution/fixtures/35_owned_struct_abandoned_prefix_entry.jett"
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
pub(super) const CASES: &[Case] = &[
    clean(
        "occupied_struct_single_ready",
        SINGLE,
        &[Script::Construct(1001)],
        &[Event::Constructed(1001), Event::Finalized(1001)],
    ),
    clean(
        "occupied_struct_single_factory_fail",
        SINGLE,
        &[Script::ConstructFail(1001, "declined")],
        &[Event::ConstructionFailed(1001)],
    ),
    panic(
        "occupied_struct_single_cleanup_panic",
        SINGLE,
        &[Script::FinalizerPanic(1001)],
        &[Event::Constructed(1001), Event::Finalized(1001)],
    ),
    clean(
        "occupied_struct_pair_ready",
        PAIR,
        &[Script::Construct(1002), Script::Construct(1003)],
        &[
            Event::Constructed(1002),
            Event::Constructed(1003),
            Event::Finalized(1002),
            Event::Finalized(1003),
        ],
    ),
    clean(
        "occupied_struct_pair_first_factory_fail",
        PAIR,
        &[Script::ConstructFail(1002, "declined")],
        &[Event::ConstructionFailed(1002)],
    ),
    clean(
        "occupied_struct_pair_second_factory_fail",
        PAIR,
        &[
            Script::Construct(1002),
            Script::ConstructFail(1003, "declined"),
        ],
        &[
            Event::Constructed(1002),
            Event::ConstructionFailed(1003),
            Event::Finalized(1002),
        ],
    ),
    panic(
        "occupied_struct_pair_first_cleanup_panic",
        PAIR,
        &[Script::FinalizerPanic(1002), Script::Construct(1003)],
        &[
            Event::Constructed(1002),
            Event::Constructed(1003),
            Event::Finalized(1002),
            Event::Finalized(1003),
        ],
    ),
    panic(
        "occupied_struct_pair_second_cleanup_panic",
        PAIR,
        &[Script::Construct(1002), Script::FinalizerPanic(1003)],
        &[
            Event::Constructed(1002),
            Event::Constructed(1003),
            Event::Finalized(1002),
            Event::Finalized(1003),
        ],
    ),
    clean(
        "occupied_struct_prefix_ready",
        PREFIX,
        &[Script::Construct(1004)],
        &[Event::Constructed(1004), Event::Finalized(1004)],
    ),
    clean(
        "occupied_struct_prefix_factory_fail",
        PREFIX,
        &[Script::ConstructFail(1004, "declined")],
        &[Event::ConstructionFailed(1004)],
    ),
    panic(
        "occupied_struct_prefix_cleanup_panic",
        PREFIX,
        &[Script::FinalizerPanic(1004)],
        &[Event::Constructed(1004), Event::Finalized(1004)],
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
