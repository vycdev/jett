use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FrameIdentity {
    pub namespace: String,
    pub function: String,
    pub path: String,
    pub line: u32,
    pub column: u32,
}

impl FrameIdentity {
    pub fn new(
        namespace: impl Into<String>,
        function: impl Into<String>,
        path: impl Into<String>,
        line: u32,
        column: u32,
    ) -> Self {
        Self {
            namespace: namespace.into(),
            function: function.into(),
            path: path.into(),
            line,
            column,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuSampleState {
    Jett,
    Runtime,
    Waiting,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuSample {
    pub state: CpuSampleState,
    pub stack: Vec<FrameIdentity>,
}

impl CpuSample {
    pub fn jett(stack: Vec<FrameIdentity>) -> Self {
        Self {
            state: CpuSampleState::Jett,
            stack,
        }
    }

    pub fn runtime() -> Self {
        Self::without_stack(CpuSampleState::Runtime)
    }

    pub fn waiting() -> Self {
        Self::without_stack(CpuSampleState::Waiting)
    }

    pub fn unavailable() -> Self {
        Self::without_stack(CpuSampleState::Unavailable)
    }

    fn without_stack(state: CpuSampleState) -> Self {
        Self {
            state,
            stack: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuConfig {
    pub threshold_basis_points: u16,
    pub limit: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuConfigError {
    ThresholdOutOfRange,
    LimitOutOfRange,
}

impl CpuConfig {
    pub fn new(threshold_basis_points: u16, limit: usize) -> Result<Self, CpuConfigError> {
        if threshold_basis_points > 10_000 {
            return Err(CpuConfigError::ThresholdOutOfRange);
        }
        if !(1..=100).contains(&limit) {
            return Err(CpuConfigError::LimitOutOfRange);
        }
        Ok(Self {
            threshold_basis_points,
            limit,
        })
    }
}

impl Default for CpuConfig {
    fn default() -> Self {
        Self {
            threshold_basis_points: 500,
            limit: 10,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CpuTotals {
    pub requested_ticks: u64,
    pub recorded_samples: u64,
    pub attributed_samples: u64,
    pub runtime_samples: u64,
    pub waiting_samples: u64,
    pub unavailable_samples: u64,
    pub coalesced_ticks: u64,
    pub collector_dropped_ticks: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CpuSuggestionRule {
    HighSelf,
    CalleeDominated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuBottleneck {
    pub frame: FrameIdentity,
    pub inclusive_samples: u64,
    pub self_samples: u64,
    pub cpu_percent_hundredths: u16,
    pub suggestion: CpuSuggestionRule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuProfile {
    pub config: CpuConfig,
    pub totals: CpuTotals,
    pub eligible_bottlenecks: usize,
    pub truncated_bottlenecks: usize,
    pub bottlenecks: Vec<CpuBottleneck>,
}

impl CpuProfile {
    pub fn aggregate(
        config: CpuConfig,
        requested_ticks: u64,
        coalesced_ticks: u64,
        collector_dropped_ticks: u64,
        samples: Vec<CpuSample>,
    ) -> Self {
        let mut totals = CpuTotals {
            requested_ticks,
            recorded_samples: samples.len() as u64,
            coalesced_ticks,
            collector_dropped_ticks,
            ..CpuTotals::default()
        };
        let mut counts: BTreeMap<FrameIdentity, (u64, u64)> = BTreeMap::new();

        for sample in samples {
            match sample.state {
                CpuSampleState::Jett if !sample.stack.is_empty() => {
                    totals.attributed_samples += 1;
                    let unique_frames: BTreeSet<&FrameIdentity> = sample.stack.iter().collect();
                    for frame in unique_frames {
                        counts.entry(frame.clone()).or_default().0 += 1;
                    }
                    if let Some(frame) = sample.stack.last() {
                        counts.entry(frame.clone()).or_default().1 += 1;
                    }
                }
                CpuSampleState::Jett | CpuSampleState::Unavailable => {
                    totals.unavailable_samples += 1;
                }
                CpuSampleState::Runtime => totals.runtime_samples += 1,
                CpuSampleState::Waiting => totals.waiting_samples += 1,
            }
        }

        let mut bottlenecks: Vec<CpuBottleneck> = counts
            .into_iter()
            .filter(|(_, (inclusive, _))| {
                totals.attributed_samples != 0
                    && *inclusive * 10_000
                        >= totals.attributed_samples * u64::from(config.threshold_basis_points)
            })
            .map(|(frame, (inclusive_samples, self_samples))| CpuBottleneck {
                frame,
                inclusive_samples,
                self_samples,
                cpu_percent_hundredths: rounded_percent_hundredths(
                    inclusive_samples,
                    totals.attributed_samples,
                ),
                suggestion: if self_samples.saturating_mul(2) >= inclusive_samples {
                    CpuSuggestionRule::HighSelf
                } else {
                    CpuSuggestionRule::CalleeDominated
                },
            })
            .collect();
        bottlenecks.sort_by(|left, right| {
            right
                .inclusive_samples
                .cmp(&left.inclusive_samples)
                .then_with(|| right.self_samples.cmp(&left.self_samples))
                .then_with(|| left.frame.cmp(&right.frame))
        });
        let eligible_bottlenecks = bottlenecks.len();
        bottlenecks.truncate(config.limit);

        Self {
            config,
            totals,
            eligible_bottlenecks,
            truncated_bottlenecks: eligible_bottlenecks - bottlenecks.len(),
            bottlenecks,
        }
    }
}

fn rounded_percent_hundredths(part: u64, total: u64) -> u16 {
    if total == 0 {
        return 0;
    }
    let scaled = u128::from(part) * 10_000;
    let total = u128::from(total);
    let rounded = (scaled + total / 2) / total;
    u16::try_from(rounded).unwrap_or(10_000)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryConfig {
    pub threshold_basis_points: u16,
    pub limit: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryConfigError {
    ThresholdOutOfRange,
    LimitOutOfRange,
}

impl MemoryConfig {
    pub fn new(threshold_basis_points: u16, limit: usize) -> Result<Self, MemoryConfigError> {
        if threshold_basis_points > 10_000 {
            return Err(MemoryConfigError::ThresholdOutOfRange);
        }
        if !(1..=100).contains(&limit) {
            return Err(MemoryConfigError::LimitOutOfRange);
        }
        Ok(Self {
            threshold_basis_points,
            limit,
        })
    }
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            threshold_basis_points: 500,
            limit: 10,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocationOperation {
    Allocate { size: u64 },
    Resize { new_size: u64 },
    Free,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocationEvent {
    pub allocation_id: u64,
    pub operation: AllocationOperation,
    pub stack: Vec<FrameIdentity>,
}

impl AllocationEvent {
    pub fn allocate(allocation_id: u64, size: u64, stack: Vec<FrameIdentity>) -> Self {
        Self {
            allocation_id,
            operation: AllocationOperation::Allocate { size },
            stack,
        }
    }

    pub fn resize(allocation_id: u64, new_size: u64, stack: Vec<FrameIdentity>) -> Self {
        Self {
            allocation_id,
            operation: AllocationOperation::Resize { new_size },
            stack,
        }
    }

    pub fn free(allocation_id: u64, stack: Vec<FrameIdentity>) -> Self {
        Self {
            allocation_id,
            operation: AllocationOperation::Free,
            stack,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MemoryTotals {
    pub allocation_count: u64,
    pub resize_count: u64,
    pub allocated_bytes: u64,
    pub freed_bytes: u64,
    pub live_bytes: u64,
    pub peak_live_bytes: u64,
    pub retained_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum MemorySuggestionRule {
    AllocationPressure,
    Retained,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryBottleneck {
    pub frame: FrameIdentity,
    pub allocation_count: u64,
    pub resize_count: u64,
    pub allocated_bytes: u64,
    pub freed_bytes: u64,
    pub retained_bytes: u64,
    pub live_at_peak_bytes: u64,
    pub allocation_percent_hundredths: u16,
    pub suggestions: Vec<MemorySuggestionRule>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryProfile {
    pub config: MemoryConfig,
    pub totals: MemoryTotals,
    pub eligible_bottlenecks: usize,
    pub truncated_bottlenecks: usize,
    pub bottlenecks: Vec<MemoryBottleneck>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryProfileError {
    DuplicateAllocation(u64),
    UnknownAllocation(u64),
    CounterOverflow,
}

#[derive(Debug, Clone)]
struct LiveAllocation {
    size: u64,
    creation_site: Option<FrameIdentity>,
}

#[derive(Debug, Clone, Default)]
struct MemorySiteCounts {
    allocation_count: u64,
    resize_count: u64,
    allocated_bytes: u64,
    freed_bytes: u64,
    retained_bytes: u64,
    live_at_peak_bytes: u64,
}

impl MemoryProfile {
    pub fn aggregate(
        config: MemoryConfig,
        events: Vec<AllocationEvent>,
    ) -> Result<Self, MemoryProfileError> {
        let mut totals = MemoryTotals::default();
        let mut known_ids = BTreeSet::new();
        let mut live_allocations: BTreeMap<u64, LiveAllocation> = BTreeMap::new();
        let mut sites: BTreeMap<FrameIdentity, MemorySiteCounts> = BTreeMap::new();
        let mut peak_event = None;

        for (index, event) in events.iter().enumerate() {
            let active_site = event.stack.last().cloned();
            match event.operation {
                AllocationOperation::Allocate { size } => {
                    if !known_ids.insert(event.allocation_id) {
                        return Err(MemoryProfileError::DuplicateAllocation(event.allocation_id));
                    }
                    totals.allocation_count = checked_add(totals.allocation_count, 1)?;
                    totals.allocated_bytes = checked_add(totals.allocated_bytes, size)?;
                    totals.live_bytes = checked_add(totals.live_bytes, size)?;
                    if let Some(site) = &active_site {
                        let counts = sites.entry(site.clone()).or_default();
                        counts.allocation_count = checked_add(counts.allocation_count, 1)?;
                        counts.allocated_bytes = checked_add(counts.allocated_bytes, size)?;
                    }
                    live_allocations.insert(
                        event.allocation_id,
                        LiveAllocation {
                            size,
                            creation_site: active_site,
                        },
                    );
                }
                AllocationOperation::Resize { new_size } => {
                    let allocation = live_allocations
                        .get_mut(&event.allocation_id)
                        .ok_or(MemoryProfileError::UnknownAllocation(event.allocation_id))?;
                    totals.resize_count = checked_add(totals.resize_count, 1)?;
                    if let Some(site) = &active_site {
                        let counts = sites.entry(site.clone()).or_default();
                        counts.resize_count = checked_add(counts.resize_count, 1)?;
                    }
                    if new_size >= allocation.size {
                        let growth = new_size - allocation.size;
                        totals.allocated_bytes = checked_add(totals.allocated_bytes, growth)?;
                        totals.live_bytes = checked_add(totals.live_bytes, growth)?;
                        if let Some(site) = &active_site {
                            let counts = sites.entry(site.clone()).or_default();
                            counts.allocated_bytes = checked_add(counts.allocated_bytes, growth)?;
                        }
                    } else {
                        let released = allocation.size - new_size;
                        totals.freed_bytes = checked_add(totals.freed_bytes, released)?;
                        totals.live_bytes -= released;
                        if let Some(site) = &allocation.creation_site {
                            let counts = sites.entry(site.clone()).or_default();
                            counts.freed_bytes = checked_add(counts.freed_bytes, released)?;
                        }
                    }
                    allocation.size = new_size;
                }
                AllocationOperation::Free => {
                    let allocation = live_allocations
                        .remove(&event.allocation_id)
                        .ok_or(MemoryProfileError::UnknownAllocation(event.allocation_id))?;
                    totals.freed_bytes = checked_add(totals.freed_bytes, allocation.size)?;
                    totals.live_bytes -= allocation.size;
                    if let Some(site) = allocation.creation_site {
                        let counts = sites.entry(site).or_default();
                        counts.freed_bytes = checked_add(counts.freed_bytes, allocation.size)?;
                    }
                }
            }

            if totals.live_bytes > totals.peak_live_bytes {
                totals.peak_live_bytes = totals.live_bytes;
                peak_event = Some(index);
            }
        }

        totals.retained_bytes = totals.live_bytes;
        for allocation in live_allocations.values() {
            if let Some(site) = &allocation.creation_site {
                let counts = sites.entry(site.clone()).or_default();
                counts.retained_bytes = checked_add(counts.retained_bytes, allocation.size)?;
            }
        }
        drop(live_allocations);
        // Reconstruct the single global-peak snapshot once. Rescanning every
        // live allocation at each increasing peak makes allocation-only traces quadratic.
        if let Some(index) = peak_event {
            populate_peak_snapshot(&events[..=index], &mut sites)?;
        }

        let attributed_allocated_bytes = sites.values().try_fold(0_u64, |total, counts| {
            checked_add(total, counts.allocated_bytes)
        })?;
        let mut bottlenecks: Vec<MemoryBottleneck> = sites
            .into_iter()
            .filter(|(_, counts)| {
                attributed_allocated_bytes != 0
                    && u128::from(counts.allocated_bytes) * 10_000
                        >= u128::from(attributed_allocated_bytes)
                            * u128::from(config.threshold_basis_points)
            })
            .map(|(frame, counts)| {
                let mut suggestions = vec![MemorySuggestionRule::AllocationPressure];
                if attributed_allocated_bytes != 0
                    && u128::from(counts.retained_bytes) * 10_000
                        >= u128::from(attributed_allocated_bytes)
                            * u128::from(config.threshold_basis_points)
                {
                    suggestions.push(MemorySuggestionRule::Retained);
                }
                MemoryBottleneck {
                    frame,
                    allocation_count: counts.allocation_count,
                    resize_count: counts.resize_count,
                    allocated_bytes: counts.allocated_bytes,
                    freed_bytes: counts.freed_bytes,
                    retained_bytes: counts.retained_bytes,
                    live_at_peak_bytes: counts.live_at_peak_bytes,
                    allocation_percent_hundredths: rounded_percent_hundredths(
                        counts.allocated_bytes,
                        attributed_allocated_bytes,
                    ),
                    suggestions,
                }
            })
            .collect();
        bottlenecks.sort_by(|left, right| {
            right
                .allocated_bytes
                .cmp(&left.allocated_bytes)
                .then_with(|| right.allocation_count.cmp(&left.allocation_count))
                .then_with(|| left.frame.cmp(&right.frame))
        });
        let eligible_bottlenecks = bottlenecks.len();
        bottlenecks.truncate(config.limit);

        Ok(Self {
            config,
            totals,
            eligible_bottlenecks,
            truncated_bottlenecks: eligible_bottlenecks - bottlenecks.len(),
            bottlenecks,
        })
    }
}

fn populate_peak_snapshot(
    events: &[AllocationEvent],
    sites: &mut BTreeMap<FrameIdentity, MemorySiteCounts>,
) -> Result<(), MemoryProfileError> {
    let mut live: BTreeMap<u64, LiveAllocation> = BTreeMap::new();
    for event in events {
        match event.operation {
            AllocationOperation::Allocate { size } => {
                live.insert(
                    event.allocation_id,
                    LiveAllocation {
                        size,
                        creation_site: event.stack.last().cloned(),
                    },
                );
            }
            AllocationOperation::Resize { new_size } => {
                live.get_mut(&event.allocation_id)
                    .expect("validated allocation")
                    .size = new_size;
            }
            AllocationOperation::Free => {
                live.remove(&event.allocation_id);
            }
        }
    }
    for allocation in live.values() {
        if let Some(site) = &allocation.creation_site {
            let counts = sites.get_mut(site).expect("validated creation site");
            counts.live_at_peak_bytes = checked_add(counts.live_at_peak_bytes, allocation.size)?;
        }
    }
    Ok(())
}

fn checked_add(left: u64, right: u64) -> Result<u64, MemoryProfileError> {
    left.checked_add(right)
        .ok_or(MemoryProfileError::CounterOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(namespace: &str, function: &str) -> FrameIdentity {
        FrameIdentity::new(namespace, function, "src/main.jett", 1, 1)
    }

    #[test]
    fn cpu_profile_aggregates_inclusive_and_self_samples() {
        let root = frame("app", "main");
        let work = frame("app", "work");
        let samples = vec![
            CpuSample::jett(vec![root.clone(), work.clone()]),
            CpuSample::jett(vec![root.clone(), work.clone()]),
            CpuSample::jett(vec![root.clone()]),
            CpuSample::runtime(),
            CpuSample::waiting(),
        ];

        let profile = CpuProfile::aggregate(CpuConfig::default(), 8, 2, 1, samples);

        assert_eq!(profile.totals.requested_ticks, 8);
        assert_eq!(profile.totals.recorded_samples, 5);
        assert_eq!(profile.totals.attributed_samples, 3);
        assert_eq!(profile.totals.runtime_samples, 1);
        assert_eq!(profile.totals.waiting_samples, 1);
        assert_eq!(profile.totals.coalesced_ticks, 2);
        assert_eq!(profile.totals.collector_dropped_ticks, 1);
        assert_eq!(profile.bottlenecks.len(), 2);
        assert_eq!(profile.bottlenecks[0].frame, root);
        assert_eq!(profile.bottlenecks[0].inclusive_samples, 3);
        assert_eq!(profile.bottlenecks[0].self_samples, 1);
        assert_eq!(profile.bottlenecks[1].frame, work);
        assert_eq!(profile.bottlenecks[1].inclusive_samples, 2);
        assert_eq!(profile.bottlenecks[1].self_samples, 2);
    }

    #[test]
    fn recursive_frames_count_once_per_sample() {
        let recursive = frame("app", "recurse");
        let samples = vec![CpuSample::jett(vec![
            recursive.clone(),
            recursive.clone(),
            recursive.clone(),
        ])];

        let profile = CpuProfile::aggregate(CpuConfig::default(), 1, 0, 0, samples);

        assert_eq!(profile.bottlenecks.len(), 1);
        assert_eq!(profile.bottlenecks[0].inclusive_samples, 1);
        assert_eq!(profile.bottlenecks[0].self_samples, 1);
    }

    #[test]
    fn cpu_config_rejects_out_of_contract_bounds() {
        assert_eq!(
            CpuConfig::new(10_001, 10),
            Err(CpuConfigError::ThresholdOutOfRange)
        );
        assert_eq!(CpuConfig::new(500, 0), Err(CpuConfigError::LimitOutOfRange));
        assert_eq!(
            CpuConfig::new(500, 101),
            Err(CpuConfigError::LimitOutOfRange)
        );
        assert_eq!(
            CpuConfig::new(500, 10),
            Ok(CpuConfig {
                threshold_basis_points: 500,
                limit: 10,
            })
        );
    }

    #[test]
    fn cpu_percent_uses_only_attributed_samples_and_rounds_to_hundredths() {
        let root = frame("app", "main");
        let leaf = frame("app", "leaf");
        let samples = vec![
            CpuSample::jett(vec![root.clone(), leaf]),
            CpuSample::jett(vec![root.clone()]),
            CpuSample::jett(vec![root]),
            CpuSample::runtime(),
            CpuSample::waiting(),
        ];

        let profile = CpuProfile::aggregate(CpuConfig::default(), 5, 0, 0, samples);

        assert_eq!(profile.bottlenecks[0].cpu_percent_hundredths, 10_000);
        assert_eq!(profile.bottlenecks[1].cpu_percent_hundredths, 3_333);
    }

    #[test]
    fn cpu_profile_assigns_deterministic_suggestion_rules() {
        let root = frame("app", "main");
        let leaf = frame("app", "leaf");
        let samples = vec![
            CpuSample::jett(vec![root.clone(), leaf.clone()]),
            CpuSample::jett(vec![root, leaf]),
        ];

        let profile = CpuProfile::aggregate(CpuConfig::default(), 2, 0, 0, samples);
        let main = profile
            .bottlenecks
            .iter()
            .find(|entry| entry.frame.function == "main")
            .expect("main bottleneck");
        let leaf = profile
            .bottlenecks
            .iter()
            .find(|entry| entry.frame.function == "leaf")
            .expect("leaf bottleneck");

        assert_eq!(main.suggestion, CpuSuggestionRule::CalleeDominated);
        assert_eq!(leaf.suggestion, CpuSuggestionRule::HighSelf);
    }

    #[test]
    fn memory_profile_tracks_resize_free_peak_and_retention_by_creation_site() {
        let creator = frame("app", "create");
        let grower = frame("app", "grow");
        let events = vec![
            AllocationEvent::allocate(1, 100, vec![creator.clone()]),
            AllocationEvent::resize(1, 150, vec![grower.clone()]),
            AllocationEvent::allocate(2, 20, vec![creator.clone()]),
            AllocationEvent::resize(2, 5, vec![grower.clone()]),
            AllocationEvent::free(2, vec![grower.clone()]),
        ];

        let profile = MemoryProfile::aggregate(MemoryConfig::default(), events).unwrap();

        assert_eq!(
            profile.totals,
            MemoryTotals {
                allocation_count: 2,
                resize_count: 2,
                allocated_bytes: 170,
                freed_bytes: 20,
                live_bytes: 150,
                peak_live_bytes: 170,
                retained_bytes: 150,
            }
        );
        let creator_entry = profile
            .bottlenecks
            .iter()
            .find(|entry| entry.frame == creator)
            .expect("creator bottleneck");
        assert_eq!(creator_entry.allocation_count, 2);
        assert_eq!(creator_entry.resize_count, 0);
        assert_eq!(creator_entry.allocated_bytes, 120);
        assert_eq!(creator_entry.freed_bytes, 20);
        assert_eq!(creator_entry.retained_bytes, 150);
        assert_eq!(creator_entry.live_at_peak_bytes, 170);
        assert_eq!(creator_entry.allocation_percent_hundredths, 7_059);

        let grower_entry = profile
            .bottlenecks
            .iter()
            .find(|entry| entry.frame == grower)
            .expect("grower bottleneck");
        assert_eq!(grower_entry.allocation_count, 0);
        assert_eq!(grower_entry.resize_count, 2);
        assert_eq!(grower_entry.allocated_bytes, 50);
        assert_eq!(grower_entry.freed_bytes, 0);
        assert_eq!(grower_entry.retained_bytes, 0);
        assert_eq!(grower_entry.live_at_peak_bytes, 0);
        assert_eq!(grower_entry.allocation_percent_hundredths, 2_941);
    }

    #[test]
    fn memory_profile_applies_exact_threshold_ordering_and_limit() {
        let alpha = frame("app", "alpha");
        let beta = frame("app", "beta");
        let gamma = frame("app", "gamma");
        let config = MemoryConfig::new(3_000, 2).unwrap();
        let events = vec![
            AllocationEvent::allocate(1, 40, vec![beta.clone()]),
            AllocationEvent::allocate(2, 30, vec![gamma]),
            AllocationEvent::allocate(3, 30, vec![alpha.clone()]),
        ];

        let profile = MemoryProfile::aggregate(config, events).unwrap();

        assert_eq!(profile.eligible_bottlenecks, 3);
        assert_eq!(profile.truncated_bottlenecks, 1);
        assert_eq!(profile.bottlenecks.len(), 2);
        assert_eq!(profile.bottlenecks[0].frame, beta);
        assert_eq!(profile.bottlenecks[1].frame, alpha);
    }

    #[test]
    fn memory_profile_rejects_invalid_allocation_lifecycles() {
        let site = frame("app", "allocate");
        let duplicate = vec![
            AllocationEvent::allocate(1, 1, vec![site.clone()]),
            AllocationEvent::allocate(1, 2, vec![site.clone()]),
        ];
        let unknown_resize = vec![AllocationEvent::resize(7, 2, vec![site.clone()])];
        let unknown_free = vec![AllocationEvent::free(9, vec![site])];

        assert_eq!(
            MemoryProfile::aggregate(MemoryConfig::default(), duplicate),
            Err(MemoryProfileError::DuplicateAllocation(1))
        );
        assert_eq!(
            MemoryProfile::aggregate(MemoryConfig::default(), unknown_resize),
            Err(MemoryProfileError::UnknownAllocation(7))
        );
        assert_eq!(
            MemoryProfile::aggregate(MemoryConfig::default(), unknown_free),
            Err(MemoryProfileError::UnknownAllocation(9))
        );
    }

    #[test]
    fn memory_peak_snapshot_scales_with_long_allocation_traces() {
        let creator = frame("app", "create");
        let mut events = (0..20_000)
            .map(|id| AllocationEvent::allocate(id, 1, vec![creator.clone()]))
            .collect::<Vec<_>>();
        events.extend((0..10_000).map(|id| AllocationEvent::free(id, Vec::new())));
        let profile = MemoryProfile::aggregate(MemoryConfig::default(), events).unwrap();
        assert_eq!(profile.totals.peak_live_bytes, 20_000);
        assert_eq!(profile.bottlenecks[0].live_at_peak_bytes, 20_000);
        assert_eq!(profile.bottlenecks[0].retained_bytes, 10_000);
    }
}
