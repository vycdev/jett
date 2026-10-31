//! Immutable launch snapshot shared by interpreted and native Environment providers.

use serde_json::{Value, json};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvironmentTestText {
    Unicode(String),
    InvalidUnicode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvironmentTestEntry {
    pub name: EnvironmentTestText,
    pub value: EnvironmentTestText,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvironmentTestSnapshot {
    pub arguments: Vec<EnvironmentTestText>,
    pub entries: Vec<EnvironmentTestEntry>,
}

pub const TEST_SNAPSHOT_ENV: &str = "JETT_NATIVE_TEST_ENVIRONMENT_SNAPSHOT_V1";
pub const INVALID_SNAPSHOT: &str = "Environment: invalid native test snapshot";

fn text_json(value: &EnvironmentTestText) -> Value {
    match value {
        EnvironmentTestText::Unicode(value) => json!({"unicode": value}),
        EnvironmentTestText::InvalidUnicode => json!({"invalid_unicode": true}),
    }
}

pub fn encode_test_snapshot(snapshot: &EnvironmentTestSnapshot) -> String {
    let arguments = snapshot.arguments.iter().map(text_json).collect::<Vec<_>>();
    let entries = snapshot
        .entries
        .iter()
        .map(|entry| json!({"name": text_json(&entry.name), "value": text_json(&entry.value)}))
        .collect::<Vec<_>>();
    json!({"arguments": arguments, "entries": entries}).to_string()
}

fn test_text(value: &Value) -> Result<EnvironmentTestText, &'static str> {
    let object = value.as_object().ok_or(INVALID_SNAPSHOT)?;
    if object.len() != 1 {
        return Err(INVALID_SNAPSHOT);
    }
    if let Some(value) = object.get("unicode") {
        return value
            .as_str()
            .map(|value| EnvironmentTestText::Unicode(value.to_owned()))
            .ok_or(INVALID_SNAPSHOT);
    }
    if object.get("invalid_unicode") == Some(&Value::Bool(true)) {
        return Ok(EnvironmentTestText::InvalidUnicode);
    }
    Err(INVALID_SNAPSHOT)
}

pub fn decode_test_snapshot(script: &str) -> Result<EnvironmentTestSnapshot, &'static str> {
    let value: Value = serde_json::from_str(script).map_err(|_| INVALID_SNAPSHOT)?;
    let object = value.as_object().ok_or(INVALID_SNAPSHOT)?;
    if object.len() != 2 {
        return Err(INVALID_SNAPSHOT);
    }
    let arguments = object
        .get("arguments")
        .and_then(Value::as_array)
        .ok_or(INVALID_SNAPSHOT)?
        .iter()
        .map(test_text)
        .collect::<Result<Vec<_>, _>>()?;
    let entries = object
        .get("entries")
        .and_then(Value::as_array)
        .ok_or(INVALID_SNAPSHOT)?
        .iter()
        .map(|entry| {
            let object = entry.as_object().ok_or(INVALID_SNAPSHOT)?;
            if object.len() != 2 {
                return Err(INVALID_SNAPSHOT);
            }
            Ok(EnvironmentTestEntry {
                name: test_text(object.get("name").ok_or(INVALID_SNAPSHOT)?)?,
                value: test_text(object.get("value").ok_or(INVALID_SNAPSHOT)?)?,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(EnvironmentTestSnapshot { arguments, entries })
}

#[derive(Debug, Clone)]
enum FrozenLaunchText {
    Unicode(String),
    InvalidUnicode,
}

#[derive(Debug, Clone)]
struct FrozenEnvironmentEntry {
    name: FrozenLaunchText,
    value: FrozenLaunchText,
}

#[derive(Debug, Clone)]
pub struct LaunchEnvironmentSnapshot {
    arguments: Vec<String>,
    entries: Vec<FrozenEnvironmentEntry>,
}

impl LaunchEnvironmentSnapshot {
    pub fn production() -> Result<Self, &'static str> {
        let arguments = std::env::args_os()
            .skip(1)
            .map(frozen_launch_text_from_os)
            .map(|argument| match argument {
                FrozenLaunchText::Unicode(argument) => Ok(argument),
                FrozenLaunchText::InvalidUnicode => {
                    Err("Environment: argument is not valid Unicode")
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let entries = std::env::vars_os()
            .map(|(name, value)| FrozenEnvironmentEntry {
                name: frozen_launch_text_from_os(name),
                value: frozen_launch_text_from_os(value),
            })
            .collect();
        Ok(Self { arguments, entries })
    }

    pub fn injected(snapshot: EnvironmentTestSnapshot) -> Result<Self, &'static str> {
        let arguments = snapshot
            .arguments
            .into_iter()
            .map(|argument| match argument {
                EnvironmentTestText::Unicode(argument) => Ok(argument),
                EnvironmentTestText::InvalidUnicode => {
                    Err("Environment: argument is not valid Unicode")
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let entries = snapshot
            .entries
            .into_iter()
            .map(|entry| FrozenEnvironmentEntry {
                name: match entry.name {
                    EnvironmentTestText::Unicode(name) => FrozenLaunchText::Unicode(name),
                    EnvironmentTestText::InvalidUnicode => FrozenLaunchText::InvalidUnicode,
                },
                value: match entry.value {
                    EnvironmentTestText::Unicode(value) => FrozenLaunchText::Unicode(value),
                    EnvironmentTestText::InvalidUnicode => FrozenLaunchText::InvalidUnicode,
                },
            })
            .collect();
        Ok(Self { arguments, entries })
    }

    pub fn arguments(&self) -> &[String] {
        &self.arguments
    }

    pub fn get(&self, key: &str) -> Result<Option<String>, &'static str> {
        if key.is_empty() || key.contains('=') || key.contains('\0') {
            return Err("Environment.get: invalid variable name");
        }
        let matching_entry = self.entries.iter().find(|entry| match &entry.name {
            FrozenLaunchText::Unicode(name) => environment_names_equal(name, key),
            FrozenLaunchText::InvalidUnicode => false,
        });
        match matching_entry.map(|entry| &entry.value) {
            None => Ok(None),
            Some(FrozenLaunchText::Unicode(value)) => Ok(Some(value.clone())),
            Some(FrozenLaunchText::InvalidUnicode) => {
                Err("Environment.get: value is not valid Unicode")
            }
        }
    }
}

fn frozen_launch_text_from_os(value: std::ffi::OsString) -> FrozenLaunchText {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        return match String::from_utf8(value.into_vec()) {
            Ok(value) => FrozenLaunchText::Unicode(value),
            Err(_) => FrozenLaunchText::InvalidUnicode,
        };
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        return match String::from_utf16(&value.encode_wide().collect::<Vec<_>>()) {
            Ok(value) => FrozenLaunchText::Unicode(value),
            Err(_) => FrozenLaunchText::InvalidUnicode,
        };
    }
}

fn environment_names_equal(captured: &str, requested: &str) -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Globalization::{CSTR_EQUAL, CompareStringOrdinal};
        let captured = captured.encode_utf16().collect::<Vec<_>>();
        let requested = requested.encode_utf16().collect::<Vec<_>>();
        let result = unsafe {
            CompareStringOrdinal(
                captured.as_ptr(),
                captured.len() as i32,
                requested.as_ptr(),
                requested.len() as i32,
                1,
            )
        };
        result == CSTR_EQUAL
    }
    #[cfg(not(windows))]
    {
        captured == requested
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injected_snapshot_roundtrips_invalid_values_and_first_duplicate() {
        let snapshot = EnvironmentTestSnapshot {
            arguments: vec![EnvironmentTestText::Unicode(String::new())],
            entries: vec![
                EnvironmentTestEntry {
                    name: EnvironmentTestText::Unicode("KEY".into()),
                    value: EnvironmentTestText::InvalidUnicode,
                },
                EnvironmentTestEntry {
                    name: EnvironmentTestText::Unicode("KEY".into()),
                    value: EnvironmentTestText::Unicode("later".into()),
                },
            ],
        };
        let decoded = decode_test_snapshot(&encode_test_snapshot(&snapshot)).unwrap();
        assert_eq!(decoded, snapshot);
        let frozen = LaunchEnvironmentSnapshot::injected(decoded).unwrap();
        assert_eq!(frozen.arguments(), &[String::new()]);
        assert_eq!(
            frozen.get("KEY"),
            Err("Environment.get: value is not valid Unicode")
        );
        assert_eq!(frozen.get("MISSING"), Ok(None));
        assert_eq!(
            frozen.get(""),
            Err("Environment.get: invalid variable name")
        );
        assert_eq!(
            LaunchEnvironmentSnapshot::injected(EnvironmentTestSnapshot {
                arguments: vec![EnvironmentTestText::InvalidUnicode],
                entries: vec![],
            })
            .err(),
            Some("Environment: argument is not valid Unicode")
        );
    }
}
