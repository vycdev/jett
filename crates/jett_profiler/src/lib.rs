use std::collections::{BTreeMap, BTreeSet};

pub const DEFAULT_THRESHOLD_BASIS_POINTS: u16 = 500;
pub const DEFAULT_BOTTLENECK_LIMIT: u16 = 10;
pub const DEFAULT_CPU_RATE_HZ: u16 = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileMode {
    Cpu,
    Memory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileRequest {
    pub mode: ProfileMode,
    pub threshold_basis_points: u16,
    pub limit: u16,
    pub cpu_rate_hz: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileRequestError {
    OptionsRequireMode,
    CpuRateRequiresCpuMode,
}

impl ProfileRequest {
    pub fn from_cli(
        mode: Option<ProfileMode>,
        threshold_basis_points: Option<u16>,
        limit: Option<u16>,
        cpu_rate_hz: Option<u16>,
    ) -> Result<Option<Self>, ProfileRequestError> {
        let Some(mode) = mode else {
            if threshold_basis_points.is_some() || limit.is_some() || cpu_rate_hz.is_some() {
                return Err(ProfileRequestError::OptionsRequireMode);
            }
            return Ok(None);
        };
        if mode == ProfileMode::Memory && cpu_rate_hz.is_some() {
            return Err(ProfileRequestError::CpuRateRequiresCpuMode);
        }
        Ok(Some(Self {
            mode,
            threshold_basis_points: threshold_basis_points
                .unwrap_or(DEFAULT_THRESHOLD_BASIS_POINTS),
            limit: limit.unwrap_or(DEFAULT_BOTTLENECK_LIMIT),
            cpu_rate_hz: match mode {
                ProfileMode::Cpu => Some(cpu_rate_hz.unwrap_or(DEFAULT_CPU_RATE_HZ)),
                ProfileMode::Memory => None,
            },
        }))
    }
}

pub fn parse_threshold_basis_points(value: &str) -> Result<u16, String> {
    let mut parts = value.split('.');
    let whole = parts.next().unwrap_or_default();
    let fraction = parts.next();
    if whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || fraction.is_some_and(|digits| {
            digits.is_empty()
                || digits.len() > 2
                || !digits.bytes().all(|byte| byte.is_ascii_digit())
        })
        || parts.next().is_some()
    {
        return Err(
            "profile threshold must be a percentage with at most two decimal places".to_string(),
        );
    }
    let whole = whole
        .parse::<u16>()
        .map_err(|_| "profile threshold must be between 0 and 100".to_string())?;
    let fractional = match fraction {
        Some(digits) if digits.len() == 1 => digits.parse::<u16>().unwrap_or(0) * 10,
        Some(digits) => digits.parse::<u16>().unwrap_or(0),
        None => 0,
    };
    if whole > 100 || (whole == 100 && fractional != 0) {
        return Err("profile threshold must be between 0 and 100".to_string());
    }
    Ok(whole * 100 + fractional)
}

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
            threshold_basis_points: DEFAULT_THRESHOLD_BASIS_POINTS,
            limit: usize::from(DEFAULT_BOTTLENECK_LIMIT),
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
}
