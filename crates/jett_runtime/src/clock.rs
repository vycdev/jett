//! Wall-clock conversion shared by interpreted and native Clock providers.

use std::collections::VecDeque;

/// One deterministic sample supplied by a Clock conformance test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockTestSample {
    Wall {
        unix_seconds: i128,
        subsecond_nanoseconds: u32,
    },
    Unavailable,
}

/// Opt-in test script consumed by the native launcher before program entry.
pub const TEST_SCRIPT_ENV: &str = "JETT_NATIVE_TEST_CLOCK_SCRIPT_V1";
pub const INVALID_SCRIPT: &str = "Clock: invalid native test script";

pub fn encode_test_script(samples: &[ClockTestSample]) -> String {
    samples
        .iter()
        .map(|sample| match sample {
            ClockTestSample::Wall {
                unix_seconds,
                subsecond_nanoseconds,
            } => format!("W,{unix_seconds},{subsecond_nanoseconds}"),
            ClockTestSample::Unavailable => "U".to_string(),
        })
        .collect::<Vec<_>>()
        .join(";")
}

pub fn decode_test_script(script: &str) -> Result<VecDeque<ClockTestSample>, &'static str> {
    if script.is_empty() {
        return Ok(VecDeque::new());
    }
    script
        .split(';')
        .map(|entry| {
            if entry == "U" {
                return Ok(ClockTestSample::Unavailable);
            }
            let Some(wall) = entry.strip_prefix("W,") else {
                return Err(INVALID_SCRIPT);
            };
            let Some((seconds, nanoseconds)) = wall.split_once(',') else {
                return Err(INVALID_SCRIPT);
            };
            let unix_seconds = seconds.parse().map_err(|_| INVALID_SCRIPT)?;
            let subsecond_nanoseconds = nanoseconds.parse().map_err(|_| INVALID_SCRIPT)?;
            Ok(ClockTestSample::Wall {
                unix_seconds,
                subsecond_nanoseconds,
            })
        })
        .collect()
}

pub const INVALID_SAMPLE: &str = "Clock.now: invalid test sample";
pub const OUT_OF_RANGE: &str = "Clock.now: timestamp is outside int64 millisecond range";

pub fn checked_clock_milliseconds(
    unix_seconds: i128,
    nanoseconds: u32,
) -> Result<i64, &'static str> {
    if nanoseconds >= 1_000_000_000 {
        return Err(INVALID_SAMPLE);
    }
    let milliseconds = unix_seconds
        .checked_mul(1_000)
        .and_then(|value| value.checked_add(i128::from(nanoseconds / 1_000_000)))
        .ok_or(OUT_OF_RANGE)?;
    i64::try_from(milliseconds).map_err(|_| OUT_OF_RANGE)
}

pub fn raw_wall_clock_sample(value: std::time::SystemTime) -> (i128, u32) {
    match value.duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => (i128::from(duration.as_secs()), duration.subsec_nanos()),
        Err(error) => {
            let duration = error.duration();
            let seconds = i128::from(duration.as_secs());
            let nanoseconds = duration.subsec_nanos();
            if nanoseconds == 0 {
                (-seconds, 0)
            } else {
                (-seconds - 1, 1_000_000_000 - nanoseconds)
            }
        }
    }
}

pub fn production_wall_clock_sample() -> (i128, u32) {
    raw_wall_clock_sample(std::time::SystemTime::now())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_test_script_roundtrips_signed_extremes_and_unavailable_samples() {
        let samples = [
            ClockTestSample::Wall {
                unix_seconds: i128::MIN,
                subsecond_nanoseconds: 1_000_000_000,
            },
            ClockTestSample::Unavailable,
            ClockTestSample::Wall {
                unix_seconds: i128::MAX,
                subsecond_nanoseconds: 0,
            },
        ];
        assert_eq!(
            decode_test_script(&encode_test_script(&samples)),
            Ok(samples.into())
        );
        assert_eq!(decode_test_script(""), Ok(VecDeque::new()));
        for invalid in ["W", "W,1", "W,1,2,3", "U;", "W,no,1", "W,1,-1"] {
            assert_eq!(decode_test_script(invalid), Err(INVALID_SCRIPT));
        }
    }
}
