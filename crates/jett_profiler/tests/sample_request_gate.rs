use jett_profiler::{CpuSampleRequestGate, CpuTickOutcome};

#[test]
fn sample_requests_coalesce_until_the_runtime_takes_the_pending_tick() {
    let gate = CpuSampleRequestGate::new();

    assert_eq!(gate.request_tick(), CpuTickOutcome::Queued);
    assert_eq!(gate.request_tick(), CpuTickOutcome::Coalesced);
    assert!(gate.take_pending());
    assert!(!gate.take_pending());
    assert_eq!(gate.request_tick(), CpuTickOutcome::Queued);

    let counts = gate.counts();
    assert_eq!(counts.requested_ticks, 3);
    assert_eq!(counts.coalesced_ticks, 1);
    assert!(counts.pending);
}
