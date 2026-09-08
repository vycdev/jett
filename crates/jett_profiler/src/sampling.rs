use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuTickOutcome {
    Queued,
    Coalesced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuSampleRequestCounts {
    pub requested_ticks: u64,
    pub coalesced_ticks: u64,
    pub pending: bool,
}

/// The constant-space handoff between a sampling timer and one runtime worker.
///
/// A timer can request as often as needed without allocating or growing a queue.
/// The runtime worker consumes the pending bit only at a safe point.
#[derive(Debug, Default)]
pub struct CpuSampleRequestGate {
    pending: AtomicBool,
    requested_ticks: AtomicU64,
    coalesced_ticks: AtomicU64,
}

impl CpuSampleRequestGate {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn request_tick(&self) -> CpuTickOutcome {
        saturating_increment(&self.requested_ticks);
        if self.pending.swap(true, Ordering::Release) {
            saturating_increment(&self.coalesced_ticks);
            CpuTickOutcome::Coalesced
        } else {
            CpuTickOutcome::Queued
        }
    }

    pub fn take_pending(&self) -> bool {
        self.pending.swap(false, Ordering::AcqRel)
    }

    pub fn counts(&self) -> CpuSampleRequestCounts {
        CpuSampleRequestCounts {
            requested_ticks: self.requested_ticks.load(Ordering::Acquire),
            coalesced_ticks: self.coalesced_ticks.load(Ordering::Acquire),
            pending: self.pending.load(Ordering::Acquire),
        }
    }
}

fn saturating_increment(counter: &AtomicU64) {
    let _ = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
        Some(value.saturating_add(1))
    });
}
