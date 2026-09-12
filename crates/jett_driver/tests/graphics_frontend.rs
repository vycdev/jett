use std::path::PathBuf;

use jett_diagnostics::Severity;
use jett_driver::{build_file, test_file};

fn fixture(kind: &str, name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join(kind)
        .join(name)
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
        ("graphics_callback_policy.jett", vec![372, 372, 372]),
        ("graphics_capability_required.jett", vec![303, 500]),
        ("graphics_verify_forbidden.jett", vec![303, 501]),
        ("graphics_property_forbidden.jett", vec![303, 372]),
        ("graphics_namespace_collision.jett", vec![204, 205]),
        ("graphics_fake_authority.jett", vec![372]),
        ("graphics_comptime_forbidden.jett", vec![504, 504]),
        ("graphics_inline_callback_effect.jett", vec![500]),
        ("graphics_callback_capture_capability.jett", vec![402]),
        ("graphics_opaque_data.jett", vec![372]),
        ("graphics_named_callback_nested_effect.jett", vec![500]),
        ("graphics_transitive_callback_effect.jett", vec![500]),
        ("graphics_inline_transitive_effect.jett", vec![500]),
        ("graphics_callback_capability_alias.jett", vec![372]),
        ("graphics_private_kernel_values.jett", vec![303, 313, 313]),
        ("graphics_state_callback_authority.jett", vec![372]),
        ("graphics_pipeline_callback_effect.jett", vec![500]),
        ("graphics_pipeline_inferred_callback_effect.jett", vec![500]),
        ("graphics_pipeline_authority.jett", vec![372]),
        ("graphics_pipeline_state_authority.jett", vec![372]),
        ("graphics_mutual_method_effect.jett", vec![500]),
        ("graphics_generic_method_effect.jett", vec![500]),
        ("graphics_inline_method_effect.jett", vec![500]),
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
