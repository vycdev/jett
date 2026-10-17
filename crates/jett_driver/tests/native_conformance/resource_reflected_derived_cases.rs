//! Shared exact derived Source/script observations; no execution or custody authority.
use super::cases::{Case, Event, ReferenceOutcome, Script};

const BREAK_SOURCE: &str = include_str!(
    "../../../jett_comptime/src/resource_execution/tests/native_source_oracles/source06_exceptional/07_reflected_field_optional_break.jett"
);
const CONTINUE_SOURCE: &str = include_str!(
    "../../../jett_comptime/src/resource_execution/tests/native_source_oracles/source06_exceptional/08_reflected_field_optional_continue.jett"
);
const DEFAULT_SOURCE: &str = include_str!(
    "../../../jett_comptime/src/resource_execution/tests/native_source_oracles/source06_exceptional/09_reflected_field_optional_scalar_default.jett"
);

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

// First ten rows preserve the existing reference DERIVED_INPUTS exactly.
pub(super) const CASES: &[Case] = &[
    Case {
        name: "reflected_resource_break_ready",
        source: BREAK_SOURCE,
        script: &[Script::Construct(871), Script::Borrow(871, 17)],
        events: &[
            Event::Constructed(871),
            Event::Borrowed(871),
            Event::Finalized(871),
        ],
        reference: ReferenceOutcome::Clean,
        body: 0,
        cleanup: 0,
        channel: 0,
        ordinary_message: b"",
        resource_message: b"",
        exit: 0,
    },
    Case {
        name: "reflected_resource_break_first_borrow_fail",
        source: BREAK_SOURCE,
        script: &[
            Script::Construct(871),
            Script::BorrowFail(871, "borrow sentinel"),
        ],
        events: &[
            Event::Constructed(871),
            Event::BorrowFailed(871),
            Event::Finalized(871),
        ],
        reference: ReferenceOutcome::Clean,
        body: 0,
        cleanup: 0,
        channel: 0,
        ordinary_message: b"",
        resource_message: b"",
        exit: 0,
    },
    Case {
        name: "reflected_resource_break_cleanup_panic",
        source: BREAK_SOURCE,
        script: &[Script::FinalizerPanic(871), Script::Borrow(871, 17)],
        events: &[
            Event::Constructed(871),
            Event::Borrowed(871),
            Event::Finalized(871),
        ],
        reference: ReferenceOutcome::CleanupPanic("selected test resource cleanup panic"),
        body: 255,
        cleanup: 255,
        channel: 3,
        ordinary_message: b"",
        resource_message: b"native Resource cleanup failed",
        exit: 71,
    },
    Case {
        name: "reflected_resource_continue_ready",
        source: CONTINUE_SOURCE,
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
        name: "reflected_resource_continue_first_borrow_fail",
        source: CONTINUE_SOURCE,
        script: &[
            Script::Construct(871),
            Script::BorrowFail(871, "borrow sentinel"),
        ],
        events: &[
            Event::Constructed(871),
            Event::BorrowFailed(871),
            Event::Finalized(871),
        ],
        reference: ReferenceOutcome::Clean,
        body: 0,
        cleanup: 0,
        channel: 0,
        ordinary_message: b"",
        resource_message: b"",
        exit: 0,
    },
    Case {
        name: "reflected_resource_continue_later_borrow_fail",
        source: CONTINUE_SOURCE,
        script: &[
            Script::Construct(871),
            Script::Borrow(871, 17),
            Script::BorrowFail(871, "later borrow sentinel"),
        ],
        events: &[
            Event::Constructed(871),
            Event::Borrowed(871),
            Event::BorrowFailed(871),
            Event::Finalized(871),
        ],
        reference: ReferenceOutcome::Clean,
        body: 0,
        cleanup: 0,
        channel: 0,
        ordinary_message: b"",
        resource_message: b"",
        exit: 0,
    },
    Case {
        name: "reflected_resource_continue_cleanup_panic",
        source: CONTINUE_SOURCE,
        script: &[
            Script::FinalizerPanic(871),
            Script::Borrow(871, 17),
            Script::Borrow(871, 17),
        ],
        events: READY_EVENTS,
        reference: ReferenceOutcome::CleanupPanic("selected test resource cleanup panic"),
        body: 255,
        cleanup: 255,
        channel: 3,
        ordinary_message: b"",
        resource_message: b"native Resource cleanup failed",
        exit: 71,
    },
    Case {
        name: "reflected_resource_scalar_default_first_borrow_fail",
        source: DEFAULT_SOURCE,
        script: &[
            Script::Construct(871),
            Script::BorrowFail(871, "borrow sentinel"),
            Script::Borrow(871, 17),
        ],
        events: &[
            Event::Constructed(871),
            Event::BorrowFailed(871),
            Event::Borrowed(871),
            Event::Finalized(871),
        ],
        reference: ReferenceOutcome::Clean,
        body: 0,
        cleanup: 0,
        channel: 0,
        ordinary_message: b"",
        resource_message: b"",
        exit: 0,
    },
    Case {
        name: "reflected_resource_scalar_default_later_borrow_fail",
        source: DEFAULT_SOURCE,
        script: &[
            Script::Construct(871),
            Script::Borrow(871, 17),
            Script::BorrowFail(871, "later borrow sentinel"),
        ],
        events: &[
            Event::Constructed(871),
            Event::Borrowed(871),
            Event::BorrowFailed(871),
            Event::Finalized(871),
        ],
        reference: ReferenceOutcome::Clean,
        body: 0,
        cleanup: 0,
        channel: 0,
        ordinary_message: b"",
        resource_message: b"",
        exit: 0,
    },
    Case {
        name: "reflected_resource_scalar_default_cleanup_panic",
        source: DEFAULT_SOURCE,
        script: &[
            Script::FinalizerPanic(871),
            Script::BorrowFail(871, "borrow sentinel"),
            Script::Borrow(871, 17),
        ],
        events: &[
            Event::Constructed(871),
            Event::BorrowFailed(871),
            Event::Borrowed(871),
            Event::Finalized(871),
        ],
        reference: ReferenceOutcome::CleanupPanic("selected test resource cleanup panic"),
        body: 255,
        cleanup: 255,
        channel: 3,
        ordinary_message: b"",
        resource_message: b"native Resource cleanup failed",
        exit: 71,
    },
    // Additional same-Source successful tail for scalar Default reentry.
    Case {
        name: "reflected_resource_scalar_default_ready",
        source: DEFAULT_SOURCE,
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

pub(super) fn original_cases() -> &'static [Case] {
    &CASES[..10]
}

pub(super) fn ready_for(case: &Case) -> &'static Case {
    if case.source == BREAK_SOURCE {
        &CASES[0]
    } else if case.source == CONTINUE_SOURCE {
        &CASES[3]
    } else {
        assert_eq!(case.source, DEFAULT_SOURCE);
        &CASES[10]
    }
}
