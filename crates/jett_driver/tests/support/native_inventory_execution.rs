//! Manifest execution gates run in the portable native conformance suite.
use super::*;
use serde_json::Value;
use std::collections::BTreeSet;

#[path = "native_manifest_inputs.rs"]
mod inputs;

fn execute_fixture(row: &Value, source: &Path) -> Result<(), String> {
    let clock_samples = inputs::clock_samples(row)?;
    let random_samples = inputs::random_samples(row)?;
    let environment_snapshot = inputs::environment_snapshot(row)?;
    let graphics_events = inputs::graphics_events(row)?;
    let providers = [
        clock_samples.is_some(),
        random_samples.is_some(),
        environment_snapshot.is_some(),
        graphics_events.is_some(),
    ];
    if providers.into_iter().filter(|present| *present).count() > 1 {
        return Err("multiple scripted providers in one inventory row".into());
    }

    let (oracle, script) = if let Some(samples) = clock_samples {
        let script = clock::encode_test_script(&samples);
        (
            jett_driver::run_file_capture_outcome_with_clock_test_samples(source, samples),
            Some((clock::TEST_SCRIPT_ENV, script)),
        )
    } else if let Some(samples) = random_samples {
        let script = random::encode_test_script(&samples);
        (
            jett_driver::run_file_capture_outcome_with_random_test_samples(source, samples),
            Some((random::TEST_SCRIPT_ENV, script)),
        )
    } else if let Some(snapshot) = environment_snapshot {
        let script = environment::encode_test_snapshot(&snapshot);
        (
            jett_driver::run_file_capture_outcome_with_environment_test_snapshot(source, snapshot),
            Some((environment::TEST_SNAPSHOT_ENV, script)),
        )
    } else if let Some(events) = graphics_events {
        let script = graphics::encode_test_script(&events);
        (
            jett_driver::run_file_capture_outcome_with_graphics_test_events(source, events),
            Some((graphics::TEST_SCRIPT_ENV, script)),
        )
    } else {
        (jett_driver::run_file_capture_outcome(source), None)
    };
    let expected_failure = row["expected_outcome"] == "expected_failure";
    if oracle.is_err() != expected_failure {
        return Err(format!(
            "interpreter outcome contradicts inventory: {oracle:?}"
        ));
    }
    let (output, failure) = match oracle {
        Ok(output) => (output, None),
        Err(failure) => (failure.output, Some(failure.message)),
    };
    // Native execution observes runtime events only, in exact diagnostic order.
    // Compile-phase captures are retained separately by the frontend.
    let mut stderr = jett_driver::render_debug_events(&output.debug_events);
    if let Some(message) = failure {
        stderr.push_str(&message);
        stderr.push('\n');
    }
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let binary = directory.path().join("inventory.exe");
    let isolated_source = directory.path().join("main.jett");
    fs::copy(source, &isolated_source).map_err(|error| error.to_string())?;
    build_host_executable(&isolated_source, launcher(), &binary)
        .map_err(|error| error.to_string())?;
    fs::remove_file(&isolated_source).map_err(|error| error.to_string())?;
    let actual = run_bounded_with_env(
        &binary,
        directory.path(),
        script.as_ref().map(|(name, value)| (*name, value.as_str())),
    );
    // Entry failure 71 requires successful context destruction. Leaked owned
    // values override it with cleanup failure 72; both are tested by the launcher.
    let status_matches = if expected_failure {
        actual.status.code() == Some(71)
    } else {
        actual.status.success()
    };
    if !status_matches
        || actual.stdout != output.stdout.as_bytes()
        || actual.stderr != stderr.as_bytes()
    {
        return Err(format!(
            "native/interpreter mismatch: native={actual:?}, expected stdout={:?}, stderr={stderr:?}, failure={expected_failure}",
            output.stdout
        ));
    }
    Ok(())
}

fn check_inventory(obligation: &str, success_kind: &str, successes: usize, failures: usize) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let manifest: Value = serde_json::from_slice(
        &fs::read(root.join("tests/native_parity.json")).expect("native inventory"),
    )
    .expect("native inventory JSON");
    assert_eq!(manifest["version"], 1);
    let rows = manifest["fixtures"].as_array().expect("inventory rows");
    let rows = rows
        .iter()
        .filter(|row| {
            row["obligations"]
                .as_array()
                .expect("fixture obligations")
                .iter()
                .any(|value| value == obligation)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        rows.len(),
        successes + failures,
        "{obligation} denominator changed"
    );
    let mut paths = BTreeSet::new();
    let mut counts = [0, 0];
    let mut errors = Vec::new();
    for row in rows {
        let path = row["path"].as_str().expect("fixture path");
        assert!(paths.insert(path), "duplicate inventory path {path}");
        match row["expected_outcome"].as_str() {
            Some("expected_failure") => counts[1] += 1,
            Some(kind) if kind == success_kind => counts[0] += 1,
            other => panic!("{path}: invalid {obligation} outcome {other:?}"),
        }
        if let Err(error) = execute_fixture(row, &root.join(path)) {
            errors.push(format!("{path}: {error}"));
        }
    }
    assert_eq!(
        counts,
        [successes, failures],
        "{obligation} outcomes changed"
    );
    assert!(
        errors.is_empty(),
        "{obligation} execution failures:\n{}",
        errors.join("\n")
    );
}

#[test]
fn native_inventory_main_execution_matches_interpreter() {
    check_inventory("main_execute", "success", 29, 1);
}

#[test]
fn native_inventory_runtime_contracts_match_interpreter() {
    check_inventory("runtime_contract", "wrapping_success", 17, 8);
}
