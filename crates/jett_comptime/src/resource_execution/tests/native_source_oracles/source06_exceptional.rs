//! Reflected field Resource execution, failure cleanup and same-grant reentry.
//! These assertions describe reference execution; native admission is separate.
use super::*;

const SOURCE: &str =
    include_str!("source06_exceptional/06_reflected_field_owned_optional_reuse.jett");
const BREAK_SOURCE: &str =
    include_str!("source06_exceptional/07_reflected_field_optional_break.jett");
const CONTINUE_SOURCE: &str =
    include_str!("source06_exceptional/08_reflected_field_optional_continue.jett");
const DEFAULT_SOURCE: &str =
    include_str!("source06_exceptional/09_reflected_field_optional_scalar_default.jett");

struct Input {
    name: &'static str,
    script: &'static [cases::Script],
    events: &'static [cases::Event],
    outcome: cases::ReferenceOutcome,
}

const READY_SCRIPT: &[cases::Script] = &[
    cases::Script::Construct(871),
    cases::Script::Borrow(871, 17),
    cases::Script::Borrow(871, 17),
];
const READY_EVENTS: &[cases::Event] = &[
    cases::Event::Constructed(871),
    cases::Event::Borrowed(871),
    cases::Event::Borrowed(871),
    cases::Event::Finalized(871),
];

const INPUTS: &[Input] = &[
    Input {
        name: "reflected_field_ready",
        script: READY_SCRIPT,
        events: READY_EVENTS,
        outcome: cases::ReferenceOutcome::Clean,
    },
    Input {
        name: "reflected_field_construct_fail",
        script: &[cases::Script::ConstructFail(871, "nested factory sentinel")],
        events: &[cases::Event::ConstructionFailed(871)],
        outcome: cases::ReferenceOutcome::Clean,
    },
    Input {
        name: "reflected_field_first_borrow_fail",
        script: &[
            cases::Script::Construct(871),
            cases::Script::BorrowFail(871, "borrow sentinel"),
        ],
        events: &[
            cases::Event::Constructed(871),
            cases::Event::BorrowFailed(871),
            cases::Event::Finalized(871),
        ],
        outcome: cases::ReferenceOutcome::Clean,
    },
    Input {
        name: "reflected_field_later_borrow_fail",
        script: &[
            cases::Script::Construct(871),
            cases::Script::Borrow(871, 17),
            cases::Script::BorrowFail(871, "later borrow sentinel"),
        ],
        events: &[
            cases::Event::Constructed(871),
            cases::Event::Borrowed(871),
            cases::Event::BorrowFailed(871),
            cases::Event::Finalized(871),
        ],
        outcome: cases::ReferenceOutcome::Clean,
    },
    Input {
        name: "reflected_field_ready_cleanup_panic",
        script: &[
            cases::Script::FinalizerPanic(871),
            cases::Script::Borrow(871, 17),
            cases::Script::Borrow(871, 17),
        ],
        events: READY_EVENTS,
        outcome: cases::ReferenceOutcome::CleanupPanic("selected test resource cleanup panic"),
    },
    Input {
        name: "reflected_field_first_borrow_provider_panic",
        script: &[
            cases::Script::Construct(871),
            cases::Script::BorrowPanic(871),
        ],
        events: &[
            cases::Event::Constructed(871),
            cases::Event::Borrowed(871),
            cases::Event::Finalized(871),
        ],
        outcome: cases::ReferenceOutcome::ProviderPanic("selected test borrow provider panic"),
    },
    Input {
        name: "reflected_field_later_borrow_provider_panic",
        script: &[
            cases::Script::Construct(871),
            cases::Script::Borrow(871, 17),
            cases::Script::BorrowPanic(871),
        ],
        events: READY_EVENTS,
        outcome: cases::ReferenceOutcome::ProviderPanic("selected test borrow provider panic"),
    },
    Input {
        name: "reflected_field_cleanup_wins_provider_panic",
        script: &[
            cases::Script::FinalizerPanic(871),
            cases::Script::BorrowPanic(871),
        ],
        events: &[
            cases::Event::Constructed(871),
            cases::Event::Borrowed(871),
            cases::Event::Finalized(871),
        ],
        outcome: cases::ReferenceOutcome::CleanupPanic("selected test resource cleanup panic"),
    },
    Input {
        name: "reflected_field_cleanup_wins_domain_failure_return",
        script: &[
            cases::Script::FinalizerPanic(871),
            cases::Script::BorrowFail(871, "borrow sentinel"),
        ],
        events: &[
            cases::Event::Constructed(871),
            cases::Event::BorrowFailed(871),
            cases::Event::Finalized(871),
        ],
        outcome: cases::ReferenceOutcome::CleanupPanic("selected test resource cleanup panic"),
    },
];

struct DerivedInput {
    source: &'static str,
    input: Input,
}

// These are explicit adaptations of Source06, never claims of unchanged Source.
const DERIVED_INPUTS: &[DerivedInput] = &[
    DerivedInput {
        source: BREAK_SOURCE,
        input: Input {
            name: "reflected_resource_break_ready",
            script: &[
                cases::Script::Construct(871),
                cases::Script::Borrow(871, 17),
            ],
            events: &[
                cases::Event::Constructed(871),
                cases::Event::Borrowed(871),
                cases::Event::Finalized(871),
            ],
            outcome: cases::ReferenceOutcome::Clean,
        },
    },
    DerivedInput {
        source: BREAK_SOURCE,
        input: Input {
            name: "reflected_resource_break_first_borrow_fail",
            script: &[
                cases::Script::Construct(871),
                cases::Script::BorrowFail(871, "borrow sentinel"),
            ],
            events: &[
                cases::Event::Constructed(871),
                cases::Event::BorrowFailed(871),
                cases::Event::Finalized(871),
            ],
            outcome: cases::ReferenceOutcome::Clean,
        },
    },
    DerivedInput {
        source: BREAK_SOURCE,
        input: Input {
            name: "reflected_resource_break_cleanup_panic",
            script: &[
                cases::Script::FinalizerPanic(871),
                cases::Script::Borrow(871, 17),
            ],
            events: &[
                cases::Event::Constructed(871),
                cases::Event::Borrowed(871),
                cases::Event::Finalized(871),
            ],
            outcome: cases::ReferenceOutcome::CleanupPanic("selected test resource cleanup panic"),
        },
    },
    DerivedInput {
        source: CONTINUE_SOURCE,
        input: Input {
            name: "reflected_resource_continue_ready",
            script: READY_SCRIPT,
            events: READY_EVENTS,
            outcome: cases::ReferenceOutcome::Clean,
        },
    },
    DerivedInput {
        source: CONTINUE_SOURCE,
        input: Input {
            name: "reflected_resource_continue_first_borrow_fail",
            script: &[
                cases::Script::Construct(871),
                cases::Script::BorrowFail(871, "borrow sentinel"),
            ],
            events: &[
                cases::Event::Constructed(871),
                cases::Event::BorrowFailed(871),
                cases::Event::Finalized(871),
            ],
            outcome: cases::ReferenceOutcome::Clean,
        },
    },
    DerivedInput {
        source: CONTINUE_SOURCE,
        input: Input {
            name: "reflected_resource_continue_later_borrow_fail",
            script: &[
                cases::Script::Construct(871),
                cases::Script::Borrow(871, 17),
                cases::Script::BorrowFail(871, "later borrow sentinel"),
            ],
            events: &[
                cases::Event::Constructed(871),
                cases::Event::Borrowed(871),
                cases::Event::BorrowFailed(871),
                cases::Event::Finalized(871),
            ],
            outcome: cases::ReferenceOutcome::Clean,
        },
    },
    DerivedInput {
        source: CONTINUE_SOURCE,
        input: Input {
            name: "reflected_resource_continue_cleanup_panic",
            script: &[
                cases::Script::FinalizerPanic(871),
                cases::Script::Borrow(871, 17),
                cases::Script::Borrow(871, 17),
            ],
            events: READY_EVENTS,
            outcome: cases::ReferenceOutcome::CleanupPanic("selected test resource cleanup panic"),
        },
    },
    DerivedInput {
        source: DEFAULT_SOURCE,
        input: Input {
            name: "reflected_resource_scalar_default_first_borrow_fail",
            script: &[
                cases::Script::Construct(871),
                cases::Script::BorrowFail(871, "borrow sentinel"),
                cases::Script::Borrow(871, 17),
            ],
            events: &[
                cases::Event::Constructed(871),
                cases::Event::BorrowFailed(871),
                cases::Event::Borrowed(871),
                cases::Event::Finalized(871),
            ],
            outcome: cases::ReferenceOutcome::Clean,
        },
    },
    DerivedInput {
        source: DEFAULT_SOURCE,
        input: Input {
            name: "reflected_resource_scalar_default_later_borrow_fail",
            script: &[
                cases::Script::Construct(871),
                cases::Script::Borrow(871, 17),
                cases::Script::BorrowFail(871, "later borrow sentinel"),
            ],
            events: &[
                cases::Event::Constructed(871),
                cases::Event::Borrowed(871),
                cases::Event::BorrowFailed(871),
                cases::Event::Finalized(871),
            ],
            outcome: cases::ReferenceOutcome::Clean,
        },
    },
    DerivedInput {
        source: DEFAULT_SOURCE,
        input: Input {
            name: "reflected_resource_scalar_default_cleanup_panic",
            script: &[
                cases::Script::FinalizerPanic(871),
                cases::Script::BorrowFail(871, "borrow sentinel"),
                cases::Script::Borrow(871, 17),
            ],
            events: &[
                cases::Event::Constructed(871),
                cases::Event::BorrowFailed(871),
                cases::Event::Borrowed(871),
                cases::Event::Finalized(871),
            ],
            outcome: cases::ReferenceOutcome::CleanupPanic("selected test resource cleanup panic"),
        },
    },
];

fn reference_case(source: &'static str, input: &Input) -> cases::Case {
    // checked_case/reference_with_required_values read only Source/script/name.
    // This reference-only module never supplies these placeholders to a native
    // Case gate. Native assertions must come from an independent linked run.
    cases::Case {
        name: input.name,
        source,
        script: input.script,
        events: input.events,
        reference: input.outcome,
        body: 0,
        cleanup: 0,
        channel: 0,
        ordinary_message: b"",
        resource_message: b"",
        exit: 0,
    }
}

fn panic_text(payload: &(dyn std::any::Any + Send)) -> &str {
    payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&str>().copied())
        .expect("selected provider/finalizer panic has a string payload")
}

fn assert_outcome(
    actual: std::thread::Result<Result<Value, String>>,
    expected: cases::ReferenceOutcome,
    name: &str,
    release: bool,
) {
    match expected {
        cases::ReferenceOutcome::Clean => assert_eq!(
            actual
                .unwrap()
                .unwrap_or_else(|error| panic!("{name} release={release}: {error}")),
            Value::Nothing,
            "{name} release={release}",
        ),
        cases::ReferenceOutcome::Error(expected) => assert_eq!(
            actual.unwrap().unwrap_err(),
            expected,
            "{name} release={release}",
        ),
        cases::ReferenceOutcome::ProviderPanic(expected)
        | cases::ReferenceOutcome::CleanupPanic(expected) => {
            let payload = actual.expect_err("the actual selected Source panic remains observable");
            assert_eq!(
                panic_text(payload.as_ref()),
                expected,
                "{name} release={release}"
            );
        }
    }
}

fn assert_empty_before_teardown(
    interpreter: &mut Interpreter,
    expected: &[ProviderEvent],
    name: &str,
    release: bool,
) {
    assert_eq!(
        interpreter.resource_test_observations().unwrap(),
        (expected.to_vec(), 0, 0),
        "{name} release={release}"
    );
    assert_eq!(
        interpreter.resource_test_custody_counts().unwrap(),
        (0, 0),
        "{name} release={release}"
    );
    assert!(
        interpreter.take_debug_events().is_empty(),
        "{name} release={release}"
    );
}

#[test]
fn reflected_field_source06_exceptional_paths_restore_before_teardown() {
    assert_eq!(INPUTS.len(), 9);
    for release in [false, true] {
        for input in INPUTS {
            let case = reference_case(SOURCE, input);
            let checked = checked_case(&case, release);
            let target = entry(&checked, "main");
            let mut interpreter = reference_with_required_values(&checked, &case, release);
            let grant = interpreter
                .install_resource_test_script(input.script.iter().copied().map(script).collect())
                .unwrap();
            let context = interpreter.resource_test_entry_context().unwrap();
            let actual = catch_unwind(AssertUnwindSafe(|| {
                interpreter.call_checked_program_entry(target, vec![grant])
            }));
            assert_outcome(actual, input.outcome, input.name, release);
            let expected = input.events.iter().map(event).collect::<Vec<_>>();
            assert_eq!(
                interpreter.resource_test_entry_context().unwrap(),
                context,
                "{} release={release}: exact entry metadata/cursor restored",
                input.name
            );
            assert_empty_before_teardown(&mut interpreter, &expected, input.name, release);
        }
    }
}

#[test]
fn reflected_field_source06_reenters_after_every_exception_with_the_same_provider_grant() {
    for release in [false, true] {
        for input in INPUTS {
            let case = reference_case(SOURCE, input);
            let checked = checked_case(&case, release);
            let target = entry(&checked, "main");
            let mut interpreter = reference_with_required_values(&checked, &case, release);
            let combined = input
                .script
                .iter()
                .chain(READY_SCRIPT.iter())
                .copied()
                .map(script)
                .collect();
            let grant = interpreter.install_resource_test_script(combined).unwrap();
            let context = interpreter.resource_test_entry_context().unwrap();
            let first = catch_unwind(AssertUnwindSafe(|| {
                interpreter.call_checked_program_entry(target, vec![grant.clone()])
            }));
            assert_outcome(first, input.outcome, input.name, release);
            let mut expected = input.events.iter().map(event).collect::<Vec<_>>();
            assert_eq!(
                interpreter.resource_test_entry_context().unwrap(),
                context,
                "{} release={release}: first exceptional entry restored",
                input.name
            );
            assert_empty_before_teardown(&mut interpreter, &expected, input.name, release);

            assert_eq!(
                interpreter
                    .call_checked_program_entry(target, vec![grant])
                    .unwrap(),
                Value::Nothing,
                "{} release={release}: same-grant ready reentry",
                input.name
            );
            expected.extend(READY_EVENTS.iter().map(event));
            assert_eq!(
                interpreter.resource_test_entry_context().unwrap(),
                context,
                "{} release={release}: same-grant ready entry restored",
                input.name
            );
            assert_empty_before_teardown(&mut interpreter, &expected, input.name, release);
        }
    }
}

#[test]
fn reflected_field_resource_aliases_retire_at_break_continue_and_scalar_default() {
    assert_eq!(DERIVED_INPUTS.len(), 10);
    for release in [false, true] {
        for control in DERIVED_INPUTS {
            let input = &control.input;
            let case = reference_case(control.source, input);
            let checked = checked_case(&case, release);
            let target = entry(&checked, "main");
            let mut interpreter = reference_with_required_values(&checked, &case, release);
            let grant = interpreter
                .install_resource_test_script(input.script.iter().copied().map(script).collect())
                .unwrap();
            let context = interpreter.resource_test_entry_context().unwrap();
            let actual = catch_unwind(AssertUnwindSafe(|| {
                interpreter.call_checked_program_entry(target, vec![grant])
            }));
            assert_outcome(actual, input.outcome, input.name, release);
            assert_eq!(
                interpreter.resource_test_entry_context().unwrap(),
                context,
                "{} release={release}: selected field control restored",
                input.name
            );
            let expected = input.events.iter().map(event).collect::<Vec<_>>();
            assert_empty_before_teardown(&mut interpreter, &expected, input.name, release);
        }
    }
}
