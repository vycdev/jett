//! Context-bound production and scripted Random providers.

use std::collections::VecDeque;

use rand::{RngCore, SeedableRng, rngs::StdRng};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RandomTestSample {
    Bounded(u64),
    Unit53(u64),
    Boolean(bool),
}

pub const TEST_SCRIPT_ENV: &str = "JETT_NATIVE_TEST_RANDOM_SCRIPT_V1";
pub const INVALID_SCRIPT: &str = "Random: invalid native test script";
pub const ENTROPY_UNAVAILABLE: &str = "Random: entropy unavailable";
pub const TEST_EXHAUSTED: &str = "Random: test provider exhausted";
pub const INVALID_SAMPLE: &str = "Random: invalid test sample";

pub fn encode_test_script(samples: &[RandomTestSample]) -> String {
    samples
        .iter()
        .map(|sample| match sample {
            RandomTestSample::Bounded(value) => format!("B,{value}"),
            RandomTestSample::Unit53(value) => format!("U,{value}"),
            RandomTestSample::Boolean(false) => "F".to_string(),
            RandomTestSample::Boolean(true) => "T".to_string(),
        })
        .collect::<Vec<_>>()
        .join(";")
}

pub fn decode_test_script(script: &str) -> Result<VecDeque<RandomTestSample>, &'static str> {
    if script.is_empty() {
        return Ok(VecDeque::new());
    }
    script
        .split(';')
        .map(|entry| {
            if entry == "F" {
                return Ok(RandomTestSample::Boolean(false));
            }
            if entry == "T" {
                return Ok(RandomTestSample::Boolean(true));
            }
            let (kind, value) = entry.split_once(',').ok_or(INVALID_SCRIPT)?;
            let value = value.parse::<u64>().map_err(|_| INVALID_SCRIPT)?;
            match kind {
                "B" => Ok(RandomTestSample::Bounded(value)),
                "U" => Ok(RandomTestSample::Unit53(value)),
                _ => Err(INVALID_SCRIPT),
            }
        })
        .collect()
}

pub enum RandomProvider {
    Production(StdRng),
    Scripted(VecDeque<RandomTestSample>),
}

impl RandomProvider {
    pub fn production() -> Result<Self, &'static str> {
        let mut seed = <StdRng as SeedableRng>::Seed::default();
        rand::rngs::OsRng
            .try_fill_bytes(seed.as_mut())
            .map_err(|_| ENTROPY_UNAVAILABLE)?;
        Ok(Self::Production(StdRng::from_seed(seed)))
    }

    pub fn scripted(samples: VecDeque<RandomTestSample>) -> Self {
        Self::Scripted(samples)
    }

    pub fn scripted_samples_remaining(&self) -> Option<usize> {
        match self {
            Self::Production(_) => None,
            Self::Scripted(samples) => Some(samples.len()),
        }
    }

    pub fn bounded_offset(&mut self, width: u64) -> Result<u64, &'static str> {
        debug_assert!(width > 0);
        match self {
            Self::Production(rng) => Ok(unbiased_bounded_offset(width, || rng.next_u64())),
            Self::Scripted(samples) => match samples.front().copied() {
                None => Err(TEST_EXHAUSTED),
                Some(RandomTestSample::Bounded(offset)) if offset < width => {
                    samples.pop_front();
                    Ok(offset)
                }
                Some(_) => Err(INVALID_SAMPLE),
            },
        }
    }

    pub fn unit53(&mut self) -> Result<u64, &'static str> {
        match self {
            Self::Production(rng) => Ok(rng.next_u64() >> 11),
            Self::Scripted(samples) => match samples.front().copied() {
                None => Err(TEST_EXHAUSTED),
                Some(RandomTestSample::Unit53(bits)) if bits < (1_u64 << 53) => {
                    samples.pop_front();
                    Ok(bits)
                }
                Some(_) => Err(INVALID_SAMPLE),
            },
        }
    }

    pub fn boolean(&mut self) -> Result<bool, &'static str> {
        match self {
            Self::Production(rng) => Ok(rng.next_u32() & 1 == 1),
            Self::Scripted(samples) => match samples.front().copied() {
                None => Err(TEST_EXHAUSTED),
                Some(RandomTestSample::Boolean(value)) => {
                    samples.pop_front();
                    Ok(value)
                }
                Some(_) => Err(INVALID_SAMPLE),
            },
        }
    }
}

pub fn unbiased_bounded_offset(width: u64, mut next_word: impl FnMut() -> u64) -> u64 {
    debug_assert!(width > 0);
    let rejection_threshold = width.wrapping_neg() % width;
    loop {
        let word = next_word();
        if word >= rejection_threshold {
            return word % width;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripted_samples_roundtrip_and_validate_before_advancing() {
        let samples = [
            RandomTestSample::Bounded(u64::MAX),
            RandomTestSample::Unit53(0),
            RandomTestSample::Boolean(false),
            RandomTestSample::Boolean(true),
        ];
        assert_eq!(
            decode_test_script(&encode_test_script(&samples)),
            Ok(samples.into())
        );
        for invalid in ["B", "U,no", "B,-1", "B,1,2", "T;"] {
            assert_eq!(decode_test_script(invalid), Err(INVALID_SCRIPT));
        }
        let mut provider = RandomProvider::scripted(VecDeque::from([RandomTestSample::Bounded(4)]));
        assert_eq!(provider.bounded_offset(4), Err(INVALID_SAMPLE));
        assert_eq!(provider.scripted_samples_remaining(), Some(1));
    }
}
