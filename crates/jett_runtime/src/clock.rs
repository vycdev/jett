//! Wall-clock conversion shared by interpreted and native Clock providers.

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
