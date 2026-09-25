use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use jett_common::FileId;
use jett_parser::ast::Item;
use jett_runtime::{environment, graphics};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Obligation {
    Lower,
    ObjectEmit,
    MainExecute,
    RuntimeContract,
}

impl Obligation {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "lower" => Ok(Self::Lower),
            "object_emit" => Ok(Self::ObjectEmit),
            "main_execute" => Ok(Self::MainExecute),
            "runtime_contract" => Ok(Self::RuntimeContract),
            unknown => Err(format!("unknown obligation `{unknown}`")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpectedOutcome {
    LowerOnly,
    Success,
    WrappingSuccess,
    ExpectedFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FixtureCategory {
    RunPass,
    RuntimeFail,
}

impl ExpectedOutcome {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "lower_only" => Ok(Self::LowerOnly),
            "success" => Ok(Self::Success),
            "wrapping_success" => Ok(Self::WrappingSuccess),
            "expected_failure" => Ok(Self::ExpectedFailure),
            unknown => Err(format!("unknown expected outcome `{unknown}`")),
        }
    }
}

#[derive(Debug)]
struct Fixture {
    obligations: BTreeSet<Obligation>,
    expected_outcome: ExpectedOutcome,
}

#[derive(Debug)]
struct Manifest {
    fixtures: BTreeMap<String, Fixture>,
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root should resolve")
}

fn require_exact_fields(
    object: &Map<String, Value>,
    expected: &[&str],
    context: &str,
) -> Result<(), String> {
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let missing = expected.difference(&actual).copied().collect::<Vec<_>>();
    let unknown = actual.difference(&expected).copied().collect::<Vec<_>>();
    if missing.is_empty() && unknown.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{context} has invalid fields; missing={missing:?}, unknown={unknown:?}"
        ))
    }
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    context: &str,
) -> Result<&'a str, String> {
    object[field]
        .as_str()
        .ok_or_else(|| format!("{context}.{field} must be a string"))
}

fn validate_fixture_path(path: &str, context: &str) -> Result<FixtureCategory, String> {
    if path.contains('\\') || Path::new(path).is_absolute() {
        return Err(format!(
            "{context}.path must be a relative forward-slash path"
        ));
    }
    let segments = path.split('/').collect::<Vec<_>>();
    let ["tests", category, file_name] = segments.as_slice() else {
        return Err(format!(
            "{context}.path must be `tests/run_pass/*.jett` or `tests/runtime_fail/*.jett`"
        ));
    };
    if file_name.is_empty() || !file_name.ends_with(".jett") {
        return Err(format!("{context}.path is not a supported fixture path"));
    }
    match *category {
        "run_pass" => Ok(FixtureCategory::RunPass),
        "runtime_fail" => Ok(FixtureCategory::RuntimeFail),
        _ => Err(format!("{context}.path is not a supported fixture path")),
    }
}

fn parse_fixture(value: &Value, index: usize) -> Result<(String, Fixture), String> {
    let context = format!("fixtures[{index}]");
    let object = value
        .as_object()
        .ok_or_else(|| format!("{context} must be an object"))?;
    let mut fields = vec!["path", "obligations", "expected_outcome"];
    if object.contains_key("clock_test_samples") {
        fields.push("clock_test_samples");
    }
    if object.contains_key("random_test_samples") {
        fields.push("random_test_samples");
    }
    if object.contains_key("environment_test_snapshot") {
        fields.push("environment_test_snapshot");
    }
    if object.contains_key("graphics_test_events") {
        fields.push("graphics_test_events");
    }
    require_exact_fields(object, &fields, &context)?;
    let scripted_count = [
        "clock_test_samples",
        "random_test_samples",
        "environment_test_snapshot",
        "graphics_test_events",
    ]
    .into_iter()
    .filter(|field| object.contains_key(*field))
    .count();
    if scripted_count > 1 {
        return Err(format!(
            "{context} cannot script multiple capabilities together"
        ));
    }
    if let Some(snapshot) = object.get("environment_test_snapshot") {
        environment::decode_test_snapshot(&snapshot.to_string())
            .map_err(|error| format!("{context}.environment_test_snapshot: {error}"))?;
    }
    if let Some(events) = object.get("graphics_test_events") {
        graphics::decode_test_script(&events.to_string())
            .map_err(|error| format!("{context}.graphics_test_events: {error}"))?;
    }

    if let Some(samples) = object.get("clock_test_samples") {
        let samples = samples
            .as_array()
            .ok_or_else(|| format!("{context}.clock_test_samples must be an array"))?;
        for (sample_index, sample) in samples.iter().enumerate() {
            let sample_context = format!("{context}.clock_test_samples[{sample_index}]");
            if sample == "unavailable" {
                continue;
            }
            let sample = sample.as_object().ok_or_else(|| {
                format!("{sample_context} must be a wall sample or `unavailable`")
            })?;
            require_exact_fields(sample, &["wall"], &sample_context)?;
            let wall = sample["wall"]
                .as_object()
                .ok_or_else(|| format!("{sample_context}.wall must be an object"))?;
            require_exact_fields(wall, &["unix_seconds", "nanoseconds"], &sample_context)?;
            wall["unix_seconds"]
                .as_str()
                .and_then(|value| value.parse::<i128>().ok())
                .ok_or_else(|| {
                    format!("{sample_context}.wall.unix_seconds must be an i128 decimal string")
                })?;
            wall["nanoseconds"]
                .as_u64()
                .and_then(|value| u32::try_from(value).ok())
                .ok_or_else(|| {
                    format!("{sample_context}.wall.nanoseconds must be a u32 integer")
                })?;
        }
    }
    if let Some(samples) = object.get("random_test_samples") {
        let samples = samples
            .as_array()
            .ok_or_else(|| format!("{context}.random_test_samples must be an array"))?;
        for (sample_index, sample) in samples.iter().enumerate() {
            let sample_context = format!("{context}.random_test_samples[{sample_index}]");
            let sample = sample
                .as_object()
                .ok_or_else(|| format!("{sample_context} must be a Random sample object"))?;
            if sample.contains_key("bounded") {
                require_exact_fields(sample, &["bounded"], &sample_context)?;
                sample["bounded"]
                    .as_str()
                    .and_then(|v| v.parse::<u64>().ok())
                    .ok_or_else(|| {
                        format!("{sample_context}.bounded must be a u64 decimal string")
                    })?;
            } else if sample.contains_key("unit53") {
                require_exact_fields(sample, &["unit53"], &sample_context)?;
                sample["unit53"]
                    .as_str()
                    .and_then(|v| v.parse::<u64>().ok())
                    .ok_or_else(|| {
                        format!("{sample_context}.unit53 must be a u64 decimal string")
                    })?;
            } else {
                require_exact_fields(sample, &["boolean"], &sample_context)?;
                sample["boolean"]
                    .as_bool()
                    .ok_or_else(|| format!("{sample_context}.boolean must be a bool"))?;
            }
        }
    }

    let path = required_string(object, "path", &context)?.to_owned();
    let category = validate_fixture_path(&path, &context)?;
    let obligation_values = object["obligations"]
        .as_array()
        .ok_or_else(|| format!("{context}.obligations must be an array"))?;
    if obligation_values.is_empty() {
        return Err(format!("{context}.obligations must not be empty"));
    }
    let mut obligations = BTreeSet::new();
    for (obligation_index, value) in obligation_values.iter().enumerate() {
        let value = value
            .as_str()
            .ok_or_else(|| format!("{context}.obligations[{obligation_index}] must be a string"))?;
        let obligation = Obligation::parse(value)?;
        if !obligations.insert(obligation) {
            return Err(format!(
                "{context}.obligations contains duplicate `{value}`"
            ));
        }
    }
    let expected_outcome =
        ExpectedOutcome::parse(required_string(object, "expected_outcome", &context)?)?;

    let classification_obligations = obligations
        .iter()
        .copied()
        .filter(|obligation| *obligation != Obligation::ObjectEmit)
        .collect::<BTreeSet<_>>();
    let object_emit_is_valid = !obligations.contains(&Obligation::ObjectEmit)
        || (category == FixtureCategory::RunPass && obligations.contains(&Obligation::Lower));
    let lower_only = BTreeSet::from([Obligation::Lower]);
    let main_execute = BTreeSet::from([Obligation::Lower, Obligation::MainExecute]);
    let runtime_contract = BTreeSet::from([Obligation::RuntimeContract]);
    let classification_is_valid = if classification_obligations == lower_only {
        category == FixtureCategory::RunPass && expected_outcome == ExpectedOutcome::LowerOnly
    } else if classification_obligations == main_execute {
        category == FixtureCategory::RunPass
            && matches!(
                expected_outcome,
                ExpectedOutcome::Success | ExpectedOutcome::ExpectedFailure
            )
    } else if classification_obligations == runtime_contract {
        category == FixtureCategory::RuntimeFail
            && matches!(
                expected_outcome,
                ExpectedOutcome::WrappingSuccess | ExpectedOutcome::ExpectedFailure
            )
    } else {
        false
    };
    if !object_emit_is_valid || !classification_is_valid {
        return Err(format!(
            "{context} has an invalid path/obligation/outcome combination"
        ));
    }

    Ok((
        path,
        Fixture {
            obligations,
            expected_outcome,
        },
    ))
}

fn parse_manifest(source: &str) -> Result<Manifest, String> {
    let value: Value = serde_json::from_str(source)
        .map_err(|error| format!("manifest is not valid JSON: {error}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| "manifest root must be an object".to_owned())?;
    require_exact_fields(object, &["version", "fixtures"], "manifest")?;
    if object["version"].as_u64() != Some(1) {
        return Err("manifest.version must be the integer 1".to_owned());
    }
    let fixture_values = object["fixtures"]
        .as_array()
        .ok_or_else(|| "manifest.fixtures must be an array".to_owned())?;
    let mut fixtures = BTreeMap::new();
    for (index, value) in fixture_values.iter().enumerate() {
        let (path, fixture) = parse_fixture(value, index)?;
        if fixtures.insert(path.clone(), fixture).is_some() {
            return Err(format!("manifest contains duplicate fixture path `{path}`"));
        }
    }
    Ok(Manifest { fixtures })
}

fn load_manifest() -> Manifest {
    let source = fs::read_to_string(workspace_root().join("tests/native_parity.json"))
        .expect("native parity manifest should be readable");
    parse_manifest(&source).expect("native parity manifest should be valid")
}

fn discovered_fixture_paths(kind: &str) -> BTreeSet<String> {
    let directory = workspace_root().join("tests").join(kind);
    fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", directory.display()))
        .map(|entry| entry.expect("fixture directory entry should be readable"))
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "jett")
        })
        .map(|entry| {
            format!(
                "tests/{kind}/{}",
                entry
                    .file_name()
                    .to_str()
                    .expect("fixture file names should be UTF-8")
            )
        })
        .collect()
}

fn manifest_paths_with_obligation(manifest: &Manifest, obligation: Obligation) -> BTreeSet<String> {
    manifest
        .fixtures
        .iter()
        .filter(|(_, fixture)| fixture.obligations.contains(&obligation))
        .map(|(path, _)| path.clone())
        .collect()
}

fn ast_main_paths(run_pass_paths: &BTreeSet<String>) -> BTreeSet<String> {
    let root = workspace_root();
    run_pass_paths
        .iter()
        .filter_map(|path| {
            let source = fs::read_to_string(root.join(path))
                .unwrap_or_else(|error| panic!("failed to read {path}: {error}"));
            let parsed = jett_parser::parse(&source, FileId::new(0));
            assert!(
                parsed.errors.is_empty(),
                "fixture {path} has parse errors: {:?}",
                parsed.errors
            );
            parsed
                .module
                .items
                .iter()
                .any(
                    |item| matches!(item, Item::Function(function) if function.name.name == "main"),
                )
                .then(|| path.clone())
        })
        .collect()
}

fn paths_with_outcome(
    manifest: &Manifest,
    obligation: Obligation,
    outcome: ExpectedOutcome,
) -> BTreeSet<String> {
    manifest
        .fixtures
        .iter()
        .filter(|(_, fixture)| {
            fixture.obligations.contains(&obligation) && fixture.expected_outcome == outcome
        })
        .map(|(path, _)| path.clone())
        .collect()
}

#[test]
fn native_parity_manifest_matches_fixture_inventory() {
    let manifest = load_manifest();

    let discovered_run_pass = discovered_fixture_paths("run_pass");
    let manifested_lower = manifest_paths_with_obligation(&manifest, Obligation::Lower);
    assert_eq!(
        manifested_lower, discovered_run_pass,
        "lowering manifest must exactly match tests/run_pass"
    );
    assert_eq!(manifested_lower.len(), 182, "lowering denominator changed");

    let manifested_object_emit = manifest_paths_with_obligation(&manifest, Obligation::ObjectEmit);
    let expected_object_emit = BTreeSet::from([
        "tests/run_pass/bytes_boundary_helpers.jett".to_owned(),
        "tests/run_pass/bytes_operations.jett".to_owned(),
        "tests/run_pass/error_handling.jett".to_owned(),
        "tests/run_pass/escape_sequences.jett".to_owned(),
        "tests/run_pass/explicit_comptime_expression.jett".to_owned(),
        "tests/run_pass/fibonacci.jett".to_owned(),
        "tests/run_pass/graphics_callback_runtime_error.jett".to_owned(),
        "tests/run_pass/graphics_pipeline_scripted.jett".to_owned(),
        "tests/run_pass/graphics_scene.jett".to_owned(),
        "tests/run_pass/graphics_scripted.jett".to_owned(),
        "tests/run_pass/handle_result_optional.jett".to_owned(),
        "tests/run_pass/hello_print.jett".to_owned(),
        "tests/run_pass/integer_wrapping_and_float_ieee.jett".to_owned(),
        "tests/run_pass/list_access_source.jett".to_owned(),
        "tests/run_pass/logical_ops.jett".to_owned(),
        "tests/run_pass/math_advanced.jett".to_owned(),
        "tests/run_pass/math_error_helpers.jett".to_owned(),
        "tests/run_pass/math_operations.jett".to_owned(),
        "tests/run_pass/math_sum_source.jett".to_owned(),
        "tests/run_pass/math_trig.jett".to_owned(),
        "tests/run_pass/multi_verify.jett".to_owned(),
        "tests/run_pass/named_argument_runtime_order.jett".to_owned(),
        "tests/run_pass/namespace_qualified_functions.jett".to_owned(),
        "tests/run_pass/namespace_runtime_main_context.jett".to_owned(),
        "tests/run_pass/namespace_runtime_verify_context.jett".to_owned(),
        "tests/run_pass/native_scalar_entry.jett".to_owned(),
        "tests/run_pass/simple.jett".to_owned(),
        "tests/run_pass/stdlib_loading.jett".to_owned(),
        "tests/run_pass/string_format.jett".to_owned(),
        "tests/run_pass/string_indic_grapheme.jett".to_owned(),
        "tests/run_pass/string_interpolation.jett".to_owned(),
        "tests/run_pass/string_layout_helpers.jett".to_owned(),
        "tests/run_pass/verify_test.jett".to_owned(),
    ]);
    assert_eq!(
        manifested_object_emit, expected_object_emit,
        "object-emission coverage must name exactly the fixtures proven by the native object gate"
    );
    assert_eq!(
        manifested_object_emit.len(),
        33,
        "native object-emission coverage changed"
    );

    let discovered_mains = ast_main_paths(&discovered_run_pass);
    let manifested_mains = manifest_paths_with_obligation(&manifest, Obligation::MainExecute);
    assert_eq!(
        manifested_mains, discovered_mains,
        "main-execution manifest must exactly match AST-discovered top-level main functions"
    );
    assert_eq!(manifested_mains.len(), 30, "main denominator changed");

    let discovered_runtime = discovered_fixture_paths("runtime_fail");
    let manifested_runtime = manifest_paths_with_obligation(&manifest, Obligation::RuntimeContract);
    assert_eq!(
        manifested_runtime, discovered_runtime,
        "runtime-contract manifest must exactly match tests/runtime_fail"
    );
    assert_eq!(
        manifested_runtime.len(),
        25,
        "runtime-contract denominator changed"
    );

    let expected_runtime_failures = [
        "clock_provider_failure.jett",
        "math_clamp_nan_bound.jett",
        "math_clamp_nan_upper_bound.jett",
        "math_clamp_reversed_float_bounds.jett",
        "random_invalid_test_sample.jett",
        "random_provider_exhausted.jett",
        "range_capacity_overflow.jett",
        "string_repeat_capacity_overflow.jett",
    ]
    .map(|name| format!("tests/runtime_fail/{name}"))
    .into_iter()
    .collect::<BTreeSet<_>>();
    let manifested_runtime_failures = paths_with_outcome(
        &manifest,
        Obligation::RuntimeContract,
        ExpectedOutcome::ExpectedFailure,
    );
    assert_eq!(manifested_runtime_failures, expected_runtime_failures);
    assert_eq!(manifested_runtime_failures.len(), 8);

    let expected_wrapping_success = discovered_runtime
        .difference(&expected_runtime_failures)
        .cloned()
        .collect::<BTreeSet<_>>();
    let manifested_wrapping_success = paths_with_outcome(
        &manifest,
        Obligation::RuntimeContract,
        ExpectedOutcome::WrappingSuccess,
    );
    assert_eq!(manifested_wrapping_success, expected_wrapping_success);
    assert_eq!(manifested_wrapping_success.len(), 17);

    let expected_main_failures =
        BTreeSet::from(["tests/run_pass/graphics_callback_runtime_error.jett".to_owned()]);
    let manifested_main_failures = paths_with_outcome(
        &manifest,
        Obligation::MainExecute,
        ExpectedOutcome::ExpectedFailure,
    );
    assert_eq!(manifested_main_failures, expected_main_failures);
    assert_eq!(
        paths_with_outcome(&manifest, Obligation::MainExecute, ExpectedOutcome::Success).len(),
        29
    );
}

#[test]
fn native_parity_object_emit_obligations_emit_deterministic_host_objects() {
    const SIMPLE_APP_ADD_SYMBOL: &str =
        "jett_v0_a9076f8b4f10f051ac43be7e160564e56e68c5278c8dd064137a42f152d4bf4f";

    let root = workspace_root();
    let manifest = load_manifest();
    let fixture_paths = manifest_paths_with_obligation(&manifest, Obligation::ObjectEmit);
    assert!(
        !fixture_paths.is_empty(),
        "object-emission coverage must not pass vacuously"
    );

    for path in fixture_paths {
        let fixture = root.join(&path);
        let first = jett_driver::native::emit_host_object_for_file(&fixture)
            .unwrap_or_else(|error| panic!("failed to emit native object for {path}: {error}"));
        let second =
            jett_driver::native::emit_host_object_for_file(&fixture).unwrap_or_else(|error| {
                panic!("failed to repeat native object emission for {path}: {error}")
            });

        assert_eq!(first.target(), jett_driver::native::host_target());
        assert!(!first.bytes().is_empty(), "{path} emitted an empty object");
        assert_eq!(
            first.bytes(),
            second.bytes(),
            "{path} object bytes are not deterministic"
        );
        assert!(
            !first.symbols().is_empty(),
            "{path} emitted no reachable symbols"
        );
        assert_eq!(
            first.symbols(),
            second.symbols(),
            "{path} object symbols are not deterministic"
        );
        if path == "tests/run_pass/simple.jett" {
            assert_eq!(
                first.symbols(),
                [SIMPLE_APP_ADD_SYMBOL],
                "{path} must emit the stable symbol derived from `app.add`"
            );
        }
    }
}

#[test]
#[ignore = "manual full-fixture native object audit"]
fn native_parity_audit_all_run_pass_objects() {
    let root = workspace_root();
    let paths = discovered_fixture_paths("run_pass");
    let mut emitted = 0;
    let mut failures = Vec::new();
    for path in &paths {
        match jett_driver::native::emit_host_object_for_file(&root.join(path)) {
            Ok(object) if !object.symbols().is_empty() => emitted += 1,
            Ok(_) => failures.push(format!("{path}: no reachable native symbols")),
            Err(error) => failures.push(format!("{path}: {error}")),
        }
    }
    println!("native objects: {emitted}/{}", paths.len());
    for failure in failures {
        println!("{failure}");
    }
}

#[test]
fn native_parity_manifest_parser_rejects_malformed_entries() {
    let invalid_manifests = [
        (
            "unknown root field",
            r#"{"version":1,"fixtures":[],"extra":true}"#,
        ),
        (
            "unknown fixture field",
            r#"{"version":1,"fixtures":[{"path":"tests/run_pass/a.jett","obligations":["lower"],"expected_outcome":"lower_only","extra":true}]}"#,
        ),
        (
            "malformed Clock sample",
            r#"{"version":1,"fixtures":[{"path":"tests/run_pass/a.jett","obligations":["lower"],"expected_outcome":"lower_only","clock_test_samples":[{"wall":{"unix_seconds":"0","nanoseconds":-1}}]}]}"#,
        ),
        (
            "unknown Clock sample field",
            r#"{"version":1,"fixtures":[{"path":"tests/run_pass/a.jett","obligations":["lower"],"expected_outcome":"lower_only","clock_test_samples":[{"wall":{"unix_seconds":"0","nanoseconds":0,"extra":true}}]}]}"#,
        ),
        (
            "malformed Random sample",
            r#"{"version":1,"fixtures":[{"path":"tests/run_pass/a.jett","obligations":["lower"],"expected_outcome":"lower_only","random_test_samples":[{"bounded":"-1"}]}]}"#,
        ),
        (
            "ambiguous Random sample",
            r#"{"version":1,"fixtures":[{"path":"tests/run_pass/a.jett","obligations":["lower"],"expected_outcome":"lower_only","random_test_samples":[{"bounded":"0","boolean":true}]}]}"#,
        ),
        (
            "unknown obligation",
            r#"{"version":1,"fixtures":[{"path":"tests/run_pass/a.jett","obligations":["compile"],"expected_outcome":"lower_only"}]}"#,
        ),
        (
            "duplicate obligation",
            r#"{"version":1,"fixtures":[{"path":"tests/run_pass/a.jett","obligations":["lower","lower"],"expected_outcome":"lower_only"}]}"#,
        ),
        (
            "duplicate fixture",
            r#"{"version":1,"fixtures":[{"path":"tests/run_pass/a.jett","obligations":["lower"],"expected_outcome":"lower_only"},{"path":"tests/run_pass/a.jett","obligations":["lower"],"expected_outcome":"lower_only"}]}"#,
        ),
        (
            "unknown outcome",
            r#"{"version":1,"fixtures":[{"path":"tests/run_pass/a.jett","obligations":["lower"],"expected_outcome":"maybe"}]}"#,
        ),
        (
            "invalid obligation combination",
            r#"{"version":1,"fixtures":[{"path":"tests/run_pass/a.jett","obligations":["main_execute"],"expected_outcome":"success"}]}"#,
        ),
        (
            "object emission without lowering",
            r#"{"version":1,"fixtures":[{"path":"tests/run_pass/a.jett","obligations":["object_emit"],"expected_outcome":"lower_only"}]}"#,
        ),
        (
            "invalid category",
            r#"{"version":1,"fixtures":[{"path":"tests/runtime_fail/a.jett","obligations":["lower"],"expected_outcome":"lower_only"}]}"#,
        ),
        ("invalid version", r#"{"version":2,"fixtures":[]}"#),
    ];

    for (name, source) in invalid_manifests {
        assert!(
            parse_manifest(source).is_err(),
            "invalid manifest case `{name}` was accepted"
        );
    }
}
