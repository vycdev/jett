//! Guarded future Source-deleted execution over the new exact shared cases.
use super::*;
#[path = "../resource_occupied_struct_reentry.rs"]
mod reentry;
#[path = "../resource_sum_replacement_cases.rs"]
mod sum_cases;

fn explicit_inputs() {
    lightweight_observations::assert_fresh_root();
    assert!(
        std::env::var_os(RECEIPT_ENV).is_some(),
        "supply the fresh measured private receipt"
    );
    assert!(
        option_env!("JETT_RESOURCE_NATIVE_TEST_ARCHIVE_EXPECTED_SHA256_V1").is_some(),
        "compile this test image with that exact measured receipt pin"
    );
    assert!(
        std::env::var_os("JETT_RESOURCE_NATIVE_EVIDENCE_DIR_V1").is_none(),
        "the new matrix must not copy every object/executable"
    );
}

fn expected_counters(case: &cases::Case) -> [u64; 5] {
    let acquisitions = u64::try_from(
        case.script
            .iter()
            .filter(|operation| {
                matches!(
                    operation,
                    cases::Script::Construct(_)
                        | cases::Script::ConstructFail(_, _)
                        | cases::Script::FinalizerPanic(_)
                )
            })
            .count(),
    )
    .unwrap();
    let borrows = u64::try_from(
        case.script
            .iter()
            .filter(|operation| matches!(operation, cases::Script::Borrow(_, _)))
            .count(),
    )
    .unwrap();
    // These whole Sources close immediately after their one successful borrow.
    // No scripted BorrowFail/Panic case is authored in this finite matrix.
    assert!(case.script.iter().all(|operation| !matches!(
        operation,
        cases::Script::BorrowFail(_, _) | cases::Script::BorrowPanic(_)
    )));
    assert!(borrows <= 1);
    let closes = u64::from(borrows == 1);
    [
        acquisitions + borrows + closes,
        acquisitions,
        acquisitions,
        acquisitions,
        acquisitions,
    ]
}

fn assert_counters(case: &cases::Case, actual: &report::Report) {
    let expected = expected_counters(case);
    assert_eq!(
        actual.observations.counters.values, expected,
        "{} actual cumulative ingress/dispatch",
        case.name
    );
    assert_eq!(actual.observations.attempts.len(), 1);
    let attempt = &actual.observations.attempts[0];
    assert_eq!(attempt.begin.counters.values, [0; 5]);
    assert_eq!(attempt.end.unwrap().counters.values, expected);
}

fn assert_reentry_counters(first: &cases::Case, ready: &cases::Case, actual: &report::Report) {
    let first_expected = expected_counters(first);
    let ready_expected = expected_counters(ready);
    let total: [u64; 5] =
        std::array::from_fn(|index| first_expected[index] + ready_expected[index]);
    assert_eq!(actual.observations.attempts.len(), 2);
    assert_eq!(
        actual.observations.counters.values, total,
        "{} retained cumulative ingress/dispatch",
        first.name
    );
    let first_attempt = &actual.observations.attempts[0];
    let ready_attempt = &actual.observations.attempts[1];
    assert_eq!(first_attempt.begin.counters.values, [0; 5]);
    assert_eq!(first_attempt.end.unwrap().counters.values, first_expected);
    assert_eq!(ready_attempt.begin.counters.values, first_expected);
    assert_eq!(ready_attempt.end.unwrap().counters.values, total);
    assert_eq!(
        first_attempt.identity.id,
        actual.attempts[0].completion.attempt
    );
    assert_eq!(
        ready_attempt.identity.id,
        actual.attempts[1].completion.attempt
    );
    assert_eq!(
        (
            first_attempt.identity.ordinal,
            ready_attempt.identity.ordinal
        ),
        (0, 1)
    );
}

#[test]
fn native_resource_direct_sum_replacement_sources_emit_with_original_endpoints() {
    assert_eq!(sum_cases::CASES.len(), 19);
    let ready = sum_cases::CASES
        .iter()
        .filter(|case| case.name.ends_with("_ready"))
        .collect::<Vec<_>>();
    assert_eq!(ready.len(), 6);
    for case in ready {
        run_object_preflight(std::slice::from_ref(case), case.name);
    }
}

#[test]
#[ignore = "requires fresh sum-capable protocol2 measured private Debug/Release archives and compiled pin"]
fn native_resource_direct_sum_replacements_retire_before_teardown_with_measured_attempts() {
    explicit_inputs();
    assert_eq!(sum_cases::CASES.len(), 19);
    run_native_cases_observing(sum_cases::CASES, cases::SUPPORT, assert_counters);
    eprintln!(
        "Resource direct sum replacement: 19 cases; 38 Source-deleted executions; paired Debug/Release; counters measured; per-kind shell telemetry unavailable"
    );
}

#[test]
#[ignore = "requires fresh sum-capable protocol2 measured private Debug/Release archives and compiled pin"]
fn native_resource_direct_sum_replacements_reenter_with_the_same_provider_grant() {
    explicit_inputs();
    assert_eq!(sum_cases::CASES.len(), 19);
    for release in [false, true] {
        for first in sum_cases::CASES {
            let ready = sum_cases::ready_for(first);
            reentry::run_original_then_ready_observing(
                first,
                ready,
                release,
                cases::SUPPORT,
                |actual| {
                    assert_reentry_counters(first, ready, actual);
                },
            );
        }
    }
    eprintln!(
        "Resource direct sum reentry: 19 first/ready pairs per profile; exact cumulative acquisition/dispatch counters; retained actual attempt identities and original completion/error records"
    );
}
