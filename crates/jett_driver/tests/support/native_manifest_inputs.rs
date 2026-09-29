//! Scripted provider inputs shared by inventory execution tests and the report tool.
use jett_runtime::clock::ClockTestSample;
use jett_runtime::environment::{self, EnvironmentTestSnapshot};
use jett_runtime::graphics::{self, TestEvent as GraphicsTestEvent};
use jett_runtime::random::RandomTestSample;
use serde_json::Value;

pub(crate) fn clock_samples(fixture: &Value) -> Result<Option<Vec<ClockTestSample>>, String> {
    let Some(samples) = fixture.get("clock_test_samples") else {
        return Ok(None);
    };
    let samples = samples
        .as_array()
        .ok_or("clock_test_samples must be an array")?;
    samples
        .iter()
        .map(|sample| {
            if sample == "unavailable" {
                return Ok(ClockTestSample::Unavailable);
            }
            let wall = sample.get("wall").ok_or("invalid Clock sample")?;
            let unix_seconds = wall["unix_seconds"]
                .as_str()
                .ok_or("Clock seconds must be a decimal string")?
                .parse::<i128>()
                .map_err(|_| "invalid Clock seconds")?;
            let subsecond_nanoseconds = wall["nanoseconds"]
                .as_u64()
                .ok_or("Clock nanoseconds must be an unsigned integer")?
                .try_into()
                .map_err(|_| "Clock nanoseconds exceed u32")?;
            Ok(ClockTestSample::Wall {
                unix_seconds,
                subsecond_nanoseconds,
            })
        })
        .collect::<Result<Vec<_>, &str>>()
        .map(Some)
        .map_err(str::to_owned)
}

pub(crate) fn random_samples(fixture: &Value) -> Result<Option<Vec<RandomTestSample>>, String> {
    let Some(samples) = fixture.get("random_test_samples") else {
        return Ok(None);
    };
    let samples = samples
        .as_array()
        .ok_or("random_test_samples must be an array")?;
    samples
        .iter()
        .map(|sample| {
            let object = sample.as_object().ok_or("invalid Random sample")?;
            if let Some(value) = object.get("bounded") {
                return value
                    .as_str()
                    .ok_or("bounded Random sample must be a decimal string")?
                    .parse::<u64>()
                    .map(RandomTestSample::Bounded)
                    .map_err(|_| "invalid bounded Random sample");
            }
            if let Some(value) = object.get("unit53") {
                return value
                    .as_str()
                    .ok_or("unit53 Random sample must be a decimal string")?
                    .parse::<u64>()
                    .map(RandomTestSample::Unit53)
                    .map_err(|_| "invalid unit53 Random sample");
            }
            if let Some(value) = object.get("boolean") {
                return value
                    .as_bool()
                    .map(RandomTestSample::Boolean)
                    .ok_or("invalid boolean Random sample");
            }
            Err("invalid Random sample")
        })
        .collect::<Result<Vec<_>, &str>>()
        .map(Some)
        .map_err(str::to_owned)
}

pub(crate) fn environment_snapshot(
    fixture: &Value,
) -> Result<Option<EnvironmentTestSnapshot>, String> {
    fixture
        .get("environment_test_snapshot")
        .map(|value| environment::decode_test_snapshot(&value.to_string()).map_err(str::to_owned))
        .transpose()
}

pub(crate) fn graphics_events(fixture: &Value) -> Result<Option<Vec<GraphicsTestEvent>>, String> {
    fixture
        .get("graphics_test_events")
        .map(|events| graphics::decode_test_script(&events.to_string()).map(Vec::from))
        .transpose()
}
