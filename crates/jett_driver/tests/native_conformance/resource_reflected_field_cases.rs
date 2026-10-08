//! Standalone reflected-field Sources; existing191case catalog remains unchanged.
//! Exact tuples are checked by single-entry and same-grant native execution.
use super::cases::{Case, Event, ReferenceOutcome, Script};

const READY_SCRIPT: &[Script] = &[
    Script::Construct(871),
    Script::Borrow(871, 17),
    Script::Borrow(871, 17),
];
const READY_EVENTS: &[Event] = &[
    Event::Constructed(871),
    Event::Borrowed(871),
    Event::Borrowed(871),
    Event::Finalized(871),
];
pub(crate) const CASES: &[Case] = &[
    Case {
        name: "reflected_field_original",
        source: include_str!("resource/reflected_field_native/01_original.jett"),
        script: READY_SCRIPT,
        events: READY_EVENTS,
        reference: ReferenceOutcome::Clean,
        body: 0,
        cleanup: 0,
        channel: 0,
        ordinary_message: b"",
        resource_message: b"",
        exit: 0,
    },
    Case {
        name: "reflected_field_index_distinct",
        source: include_str!("resource/reflected_field_native/02_index_distinct.jett"),
        script: READY_SCRIPT,
        events: READY_EVENTS,
        reference: ReferenceOutcome::Clean,
        body: 0,
        cleanup: 0,
        channel: 0,
        ordinary_message: b"",
        resource_message: b"",
        exit: 0,
    },
    Case {
        name: "reflected_field_name_distinct",
        source: include_str!("resource/reflected_field_native/03_name_distinct.jett"),
        script: READY_SCRIPT,
        events: READY_EVENTS,
        reference: ReferenceOutcome::Clean,
        body: 0,
        cleanup: 0,
        channel: 0,
        ordinary_message: b"",
        resource_message: b"",
        exit: 0,
    },
    Case {
        name: "reflected_field_ordinary_helper",
        source: include_str!("resource/reflected_field_native/04_ordinary_helper.jett"),
        script: READY_SCRIPT,
        events: READY_EVENTS,
        reference: ReferenceOutcome::Clean,
        body: 0,
        cleanup: 0,
        channel: 0,
        ordinary_message: b"",
        resource_message: b"",
        exit: 0,
    },
];
