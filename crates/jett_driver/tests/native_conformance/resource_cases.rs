//! Identical original Source and finite script/oracle inputs for both test harnesses.
//! No record contains a runtime token, Resource key or installer authority.
#![allow(dead_code)] // Each harness consumes its own side of the shared observation oracle.
pub(crate) const SUPPORT: &str = include_str!("resource/resource_probe.jett");
#[derive(Clone, Copy, Debug)]
pub(crate) enum Script {
    Construct(i64),
    ConstructFail(i64, &'static str),
    FinalizerPanic(i64),
    Borrow(i64, i64),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Event {
    Constructed(i64),
    ConstructionFailed(i64),
    Borrowed(i64),
    Finalized(i64),
}
#[derive(Clone, Copy, Debug)]
pub(crate) enum ReferenceOutcome {
    Clean,
    Error(&'static str),
    CleanupPanic(&'static str),
}
pub(crate) struct Case {
    pub(crate) name: &'static str,
    pub(crate) source: &'static str,
    pub(crate) script: &'static [Script],
    pub(crate) events: &'static [Event],
    pub(crate) reference: ReferenceOutcome,
    // Native status/channel literals are separate from the reference error representation.
    pub(crate) body: u32,
    pub(crate) cleanup: u32,
    pub(crate) channel: u32,
    pub(crate) ordinary_message: &'static [u8],
    pub(crate) resource_message: &'static [u8],
    pub(crate) exit: i32,
}
pub(crate) const CASES: &[Case] = &[
    Case {
        name: "connected",
        source: include_str!("resource/01_connected_entry.jett"),
        script: &[Script::Construct(1), Script::Borrow(1, 7)],
        events: &[
            Event::Constructed(1),
            Event::Borrowed(1),
            Event::Finalized(1),
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
        name: "factory_fail",
        source: include_str!("resource/01_connected_entry.jett"),
        script: &[Script::ConstructFail(1, "declined")],
        events: &[Event::ConstructionFailed(1)],
        reference: ReferenceOutcome::Clean,
        body: 0,
        cleanup: 0,
        channel: 0,
        ordinary_message: b"",
        resource_message: b"",
        exit: 0,
    },
    Case {
        name: "later_actual",
        source: include_str!("resource/02_partial_actual_failure.jett"),
        script: &[Script::Construct(1)],
        events: &[Event::Constructed(1), Event::Finalized(1)],
        reference: ReferenceOutcome::Error("list.__remove_at: index -1 out of bounds"),
        body: 1,
        cleanup: 0,
        channel: 1,
        ordinary_message: b"list.__remove_at: index -1 out of bounds",
        resource_message: b"native Resource protocol refused",
        exit: 71,
    },
    Case {
        name: "implicit_drop",
        source: include_str!("resource/03_implicit_drop.jett"),
        script: &[Script::Construct(1)],
        events: &[Event::Constructed(1), Event::Finalized(1)],
        reference: ReferenceOutcome::Clean,
        body: 0,
        cleanup: 0,
        channel: 0,
        ordinary_message: b"",
        resource_message: b"",
        exit: 0,
    },
    Case {
        name: "finalizer_panic",
        source: include_str!("resource/03_implicit_drop.jett"),
        script: &[Script::FinalizerPanic(1)],
        events: &[Event::Constructed(1), Event::Finalized(1)],
        reference: ReferenceOutcome::CleanupPanic("selected test resource cleanup panic"),
        body: 0,
        cleanup: 255,
        channel: 3,
        ordinary_message: b"",
        resource_message: b"native Resource cleanup failed",
        exit: 71,
    },
];
