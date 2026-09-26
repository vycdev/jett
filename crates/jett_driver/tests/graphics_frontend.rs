use std::path::PathBuf;

use jett_diagnostics::Severity;
use jett_driver::{build_file, build_source, test_file};

fn fixture(kind: &str, name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join(kind)
        .join(name)
}

// A valid capability-taking nested closure can occur outside a graphics callback
// graph. Inside it, the conservative audit rejects its signature, effect call,
// and capability use, even if that inner closure is not invoked. Keep the outer
// perform() calls so equality/method/helper dispatch still has to be followed.
fn assert_typed_callback_effect(source: &str) {
    let outcome = build_source(source, "graphics-typed-callback-effect.jett");
    let errors = outcome
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect::<Vec<_>>();
    assert_eq!(
        errors.iter().map(|d| d.code.code()).collect::<Vec<_>>(),
        vec![372, 500, 372],
        "{:?}",
        outcome.diagnostics
    );
    assert!(errors[0].message.contains("callback closures must be pure"));
    assert!(errors[1].message.contains("<graphics callback>"));
    assert!(errors[1].message.contains("app.effect"));
    assert!(
        errors[2]
            .message
            .contains("callback closures cannot contain opaque capability values")
    );
}

fn without_nested_effect(source: &str) -> String {
    let line = source
        .lines()
        .find(|line| line.contains("emit = function("))
        .expect("typed nested effect probe must exist");
    let indent = &line[..line.len() - line.trim_start().len()];
    source.replace(line, &format!("{indent}function(int64) returns int64 emit = function(value: int64) returns int64: return value"))
}

fn assert_graphics_codes(source: &str, expected: &[u16]) {
    let outcome = build_source(source, "graphics-policy-variant.jett");
    let mut codes = outcome
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| d.code.code())
        .collect::<Vec<_>>();
    codes.sort_unstable();
    assert_eq!(codes, expected, "{:?}", outcome.diagnostics);
}

#[test]
fn graphics_scene_and_callbacks_compile_without_opening_a_window() {
    let outcome = test_file(&fixture("run_pass", "graphics_scene.jett"))
        .expect("graphics callbacks and scene should compile");
    assert_eq!(outcome.total, 1);
    assert_eq!(outcome.passed, outcome.total);
}

#[test]
fn graphics_frontend_policy_failures() {
    for (name, expected) in [
        ("graphics_capability_not_constructible.jett", vec![313]),
        ("graphics_capability_ownership.jett", vec![502, 503]),
        ("graphics_private_kernel.jett", vec![303, 313]),
        ("graphics_callback_types.jett", vec![300]),
        ("graphics_callback_policy.jett", vec![300, 372, 372, 372]),
        ("graphics_capability_required.jett", vec![303, 500]),
        ("graphics_verify_forbidden.jett", vec![303, 501]),
        ("graphics_property_forbidden.jett", vec![303, 372]),
        ("graphics_namespace_collision.jett", vec![204, 205]),
        ("graphics_fake_authority.jett", vec![300, 372]),
        ("graphics_comptime_forbidden.jett", vec![504, 504]),
        ("graphics_inline_callback_effect.jett", vec![372, 372, 500]),
        ("graphics_callback_capture_capability.jett", vec![402]),
        ("graphics_opaque_data.jett", vec![300, 372]),
        (
            "graphics_named_callback_nested_effect.jett",
            vec![372, 372, 500],
        ),
        (
            "graphics_transitive_callback_effect.jett",
            vec![372, 372, 500],
        ),
        (
            "graphics_inline_transitive_effect.jett",
            vec![372, 372, 500],
        ),
        ("graphics_callback_capability_alias.jett", vec![300, 372]),
        ("graphics_private_kernel_values.jett", vec![303, 313, 313]),
        ("graphics_state_callback_authority.jett", vec![372, 402]),
        (
            "graphics_pipeline_callback_effect.jett",
            vec![372, 372, 500],
        ),
        (
            "graphics_pipeline_inferred_callback_effect.jett",
            vec![372, 372, 500],
        ),
        ("graphics_pipeline_authority.jett", vec![300, 372]),
        ("graphics_pipeline_state_authority.jett", vec![372]),
        ("graphics_mutual_method_effect.jett", vec![372, 372, 500]),
        ("graphics_generic_method_effect.jett", vec![372, 372, 500]),
        ("graphics_inline_method_effect.jett", vec![372, 372, 500]),
        ("graphics_equality_effect.jett", vec![372, 372, 500]),
    ] {
        let outcome = build_file(&fixture("compile_fail", name));
        let mut codes = outcome
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Error)
            .map(|diagnostic| diagnostic.code.code())
            .collect::<Vec<_>>();
        codes.sort_unstable();
        assert_eq!(codes, expected, "{name}: {:?}", outcome.diagnostics);
    }
}

#[test]
fn graphics_equality_operators_audit_implicit_methods_in_concrete_instantiations() {
    let fixture = include_str!("../../../tests/compile_fail/graphics_equality_effect.jett");
    for operator in ["==", "!="] {
        for generic in [false, true] {
            let comparison = format!("left {operator} right");
            let source = if generic {
                fixture
                    .replace(
                        "function update(",
                        &format!("function compare[T](view left: T, view right: T) returns bool:\n    return {comparison}\nfunction update("),
                    )
                    .replace("if left == right:", "if compare[Number](view left, view right):")
            } else {
                fixture.replace("left == right", &comparison)
            };
            let outcome = build_source(&source, "graphics-equality-effect.jett");
            let codes = outcome
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.severity == Severity::Error)
                .map(|diagnostic| diagnostic.code.code())
                .collect::<Vec<_>>();
            assert_eq!(
                codes,
                vec![372, 500, 372],
                "{operator}, generic={generic}: {:?}",
                outcome.diagnostics
            );
            assert_typed_callback_effect(&source);
            assert_graphics_codes(&without_nested_effect(&source), &[]);
        }
    }
}

#[test]
fn graphics_equality_audit_does_not_follow_unrelated_generic_instantiations() {
    let fixture =
        include_str!("../../../tests/compile_pass/graphics_equality_generic_isolation.jett");
    for operator in ["==", "!="] {
        let source = fixture.replace(
            "return left == right",
            &format!("return left {operator} right"),
        );
        let outcome = build_source(&source, "graphics-equality-isolation.jett");
        assert!(!outcome.has_errors, "{operator}: {:?}", outcome.diagnostics);
        let reachable_effect = source
            .replace(
                "Pure left = Pure(value: state)",
                "Effectful left = Effectful(value: state)",
            )
            .replace(
                "Pure right = Pure(value: state)",
                "Effectful right = Effectful(value: state)",
            )
            .replace("compare[Pure]", "compare[Effectful]");
        assert_ne!(reachable_effect, source);
        assert_typed_callback_effect(&reachable_effect);
    }
}

#[test]
fn graphics_method_authority_compiles_without_opening_a_window() {
    let outcome = build_file(&fixture("compile_pass", "graphics_method_authority.jett"));
    assert!(!outcome.has_errors, "{:?}", outcome.diagnostics);
}

#[test]
fn graphics_pipeline_and_mutual_callbacks_compile_without_opening_a_window() {
    for name in [
        "graphics_pipeline_mutual_callbacks.jett",
        "graphics_generic_method_isolation.jett",
    ] {
        let outcome = build_file(&fixture("compile_pass", name));
        assert!(!outcome.has_errors, "{name}: {:?}", outcome.diagnostics);
    }
}

#[test]
fn graphics_nested_effect_probes_have_well_typed_pure_controls() {
    for name in [
        "graphics_inline_callback_effect.jett",
        "graphics_named_callback_nested_effect.jett",
        "graphics_transitive_callback_effect.jett",
        "graphics_inline_transitive_effect.jett",
        "graphics_pipeline_callback_effect.jett",
        "graphics_pipeline_inferred_callback_effect.jett",
        "graphics_mutual_method_effect.jett",
        "graphics_generic_method_effect.jett",
        "graphics_inline_method_effect.jett",
        "graphics_equality_effect.jett",
    ] {
        let source = std::fs::read_to_string(fixture("compile_fail", name)).unwrap();
        assert_typed_callback_effect(&source);
        assert_graphics_codes(&without_nested_effect(&source), &[]);
    }
}

#[test]
fn graphics_generic_method_isolation_rejects_reachable_effects() {
    let source = include_str!("../../../tests/compile_pass/graphics_generic_method_isolation.jett");
    assert_graphics_codes(source, &[]);
    let reachable_effect = source
        .replace(
            "Pure pure = Pure(value: state)",
            "Effectful pure = Effectful(value: state)",
        )
        .replace("evaluate[Pure]", "evaluate[Effectful]");
    assert_ne!(reachable_effect, source);
    assert_typed_callback_effect(&reachable_effect);
}

#[test]
fn graphics_callback_signature_and_policy_are_independent_obligations() {
    let source = include_str!("../../../tests/compile_fail/graphics_callback_policy.jett");
    let pipeline = source.replace(
        "graphics.run[int64](view display, ",
        "display into view graphics.run[int64](",
    );
    for source in [
        source.to_owned(),
        source.replace("graphics.run[int64]", "graphics.run"),
        pipeline.clone(),
        pipeline.replace("graphics.run[int64]", "graphics.run"),
    ] {
        assert_graphics_codes(&source, &[300, 372, 372, 372]);
        // A pure but wrong Key type owes E0300, not the impure-update E0372.
        let wrong_key = source.replace(
            "impure_update(state: int64, view display: Graphics)",
            "impure_update(state: int64, key: string)",
        );
        assert_ne!(wrong_key, source);
        assert_graphics_codes(&wrong_key, &[300, 372, 372]);
        // Correct types with a borrowed Key owe only the parameter-mode E0372.
        let borrowed_key = source.replace(
            "impure_update(state: int64, view display: Graphics)",
            "impure_update(state: int64, view key: graphics.Key)",
        );
        assert_graphics_codes(&borrowed_key, &[372, 372, 372]);
        let valid_update = borrowed_key.replace("view key: graphics.Key", "key: graphics.Key");
        assert_graphics_codes(&valid_update, &[372, 372]);
    }
}

#[test]
fn graphics_authority_diagnostics_have_independent_controls() {
    for (name, before, after, expected) in [
        (
            "graphics_callback_capability_alias.jett",
            "type Display = Graphics",
            "type Display = int64",
            &[][..],
        ),
        (
            "graphics_opaque_data.jett",
            "view display, display, 0",
            "view display, 0, 0",
            &[300][..],
        ),
        (
            "graphics_state_callback_authority.jett",
            "return effect(view stdout)",
            "return 0",
            &[372][..],
        ),
        (
            "graphics_callback_capture_capability.jett",
            "Graphics captured = display",
            "int64 captured = state",
            &[][..],
        ),
    ] {
        let source = std::fs::read_to_string(fixture("compile_fail", name)).unwrap();
        let variant = source.replace(before, after);
        assert_ne!(source, variant, "{name}");
        assert_graphics_codes(&variant, expected);
    }
    // Removing the forged type alone still leaves the explicit-view obligation.
    let source = include_str!("../../../tests/compile_fail/graphics_fake_authority.jett");
    let typed = source.replace(r#""Graphics""#, "display");
    assert_ne!(typed, source);
    assert_graphics_codes(&typed, &[372]);
    assert_graphics_codes(&source.replace(r#""Graphics""#, "view display"), &[]);

    let source = include_str!("../../../tests/compile_fail/graphics_pipeline_authority.jett");
    let typed = source.replace(r#""fake authority""#, "display");
    assert_ne!(typed, source);
    assert_graphics_codes(&typed, &[]);
    assert_graphics_codes(&typed.replace("into view", "into"), &[372]);
}
