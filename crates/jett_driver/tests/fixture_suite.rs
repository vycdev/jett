use std::fs;
use std::path::{Path, PathBuf};

use jett_common::FileId;
use jett_diagnostics::Severity;
use jett_driver::{
    build_file, build_source, completions, completions_at, hover_type, run_file,
    run_file_capture_stdout, test_file,
};
use jett_parser::ast::Item;
use jett_parser::parse;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("workspace root should resolve")
}

fn fixture_path(kind: &str, name: &str) -> PathBuf {
    workspace_root().join("tests").join(kind).join(name)
}

fn fixture_source(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
}

fn fixture_has_main(path: &Path) -> bool {
    let source = fixture_source(path);
    let parsed = parse(&source, FileId::new(0));
    parsed.module.items.iter().any(|item| {
        matches!(
            item,
            Item::Function(func) if func.name.name == "main"
        )
    })
}

fn expected_error_codes(source: &str) -> Vec<u32> {
    source
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            let rest = trimmed.strip_prefix("# ERROR:")?.trim();
            let digits = rest
                .strip_prefix('E')
                .unwrap_or(rest)
                .chars()
                .take_while(|ch| ch.is_ascii_digit())
                .collect::<String>();
            digits.parse::<u32>().ok()
        })
        .collect()
}

fn error_messages(path: &Path, diagnostics: &[jett_diagnostics::Diagnostic]) -> String {
    diagnostics
        .iter()
        .filter(|diag| diag.severity == Severity::Error)
        .map(|diag| format!("{}: {}", diag.code, diag.message))
        .collect::<Vec<_>>()
        .join("\n")
        .if_empty_then(format!("{} produced no error diagnostics", path.display()))
}

trait IfEmptyThen {
    fn if_empty_then(self, fallback: String) -> String;
}

impl IfEmptyThen for String {
    fn if_empty_then(self, fallback: String) -> String {
        if self.is_empty() { fallback } else { self }
    }
}

fn assert_compile_pass(name: &str) {
    let path = fixture_path("compile_pass", name);
    let result = build_file(&path);
    assert!(
        !result.has_errors,
        "expected {} to compile successfully:\n{}",
        path.display(),
        error_messages(&path, &result.diagnostics)
    );
}

fn assert_compile_fail(name: &str) {
    let path = fixture_path("compile_fail", name);
    let source = fixture_source(&path);
    let expected_codes = expected_error_codes(&source);
    let result = build_file(&path);

    assert!(
        result.has_errors,
        "expected {} to fail compilation",
        path.display()
    );

    let actual_codes: Vec<u32> = result
        .diagnostics
        .iter()
        .filter(|diag| diag.severity == Severity::Error)
        .map(|diag| u32::from(diag.code.code()))
        .collect();

    for expected in expected_codes {
        assert!(
            actual_codes.contains(&expected),
            "expected {} to report E{:04}, got:\n{}",
            path.display(),
            expected,
            error_messages(&path, &result.diagnostics)
        );
    }
}

fn assert_run_pass(name: &str) {
    let path = fixture_path("run_pass", name);
    if fixture_has_main(&path) {
        run_file(&path)
            .unwrap_or_else(|err| panic!("expected {} to run successfully: {err}", path.display()));
    } else {
        let result = build_file(&path);
        assert!(
            !result.has_errors,
            "expected {} verify/property fixture to succeed:\n{}",
            path.display(),
            error_messages(&path, &result.diagnostics)
        );
    }
}

fn assert_run_stdout(name: &str, expected: &str) {
    let path = fixture_path("run_pass", name);
    let output = run_file_capture_stdout(&path).unwrap_or_else(|err| {
        panic!(
            "expected {} to run successfully with captured stdout: {err}",
            path.display()
        )
    });
    assert_eq!(output, expected, "unexpected stdout for {}", path.display());
}

macro_rules! compile_pass_fixture {
    ($test_name:ident, $file_name:literal) => {
        #[test]
        fn $test_name() {
            assert_compile_pass($file_name);
        }
    };
}

macro_rules! compile_fail_fixture {
    ($test_name:ident, $file_name:literal) => {
        #[test]
        fn $test_name() {
            assert_compile_fail($file_name);
        }
    };
}

macro_rules! run_pass_fixture {
    ($test_name:ident, $file_name:literal) => {
        #[test]
        fn $test_name() {
            assert_run_pass($file_name);
        }
    };
}

compile_pass_fixture!(compile_pass_basic, "basic.jett");
compile_pass_fixture!(compile_pass_hello, "hello.jett");
compile_pass_fixture!(
    compile_pass_interface_displayable,
    "interface_displayable.jett"
);
compile_pass_fixture!(compile_pass_handle_refinement, "handle_refinement.jett");
compile_pass_fixture!(compile_pass_generic_struct, "generic_struct.jett");
compile_pass_fixture!(
    compile_pass_json_parse_struct_handle,
    "json_parse_struct_handle.jett"
);
compile_pass_fixture!(
    compile_pass_json_parse_map_handle,
    "json_parse_map_handle.jett"
);
compile_pass_fixture!(
    compile_pass_json_serialize_public_secret_fields,
    "json_serialize_public_secret_fields.jett"
);
compile_pass_fixture!(
    compile_pass_generic_reflection_local_fact_specialization,
    "generic_reflection_local_fact_specialization.jett"
);
compile_pass_fixture!(
    compile_pass_generic_reflection_runtime_guard_deferral,
    "generic_reflection_runtime_guard_deferral.jett"
);

compile_fail_fixture!(compile_fail_type_mismatch, "type_mismatch.jett");
compile_fail_fixture!(compile_fail_secret_stdout, "secret_stdout.jett");
compile_fail_fixture!(
    compile_fail_refinement_requires_handle,
    "refinement_requires_handle.jett"
);

run_pass_fixture!(run_pass_simple, "simple.jett");
run_pass_fixture!(run_pass_fibonacci, "fibonacci.jett");
run_pass_fixture!(run_pass_hello_print, "hello_print.jett");
run_pass_fixture!(run_pass_string_interpolation, "string_interpolation.jett");
run_pass_fixture!(run_pass_stdlib_loading, "stdlib_loading.jett");
run_pass_fixture!(run_pass_verify_test, "verify_test.jett");
run_pass_fixture!(run_pass_multi_verify, "multi_verify.jett");
run_pass_fixture!(
    run_pass_property_generic_lists,
    "property_generic_lists.jett"
);

#[test]
fn run_file_capture_stdout_captures_capability_writes() {
    assert_run_stdout("hello_print.jett", "Hello, Jett!42");
    assert_run_stdout("string_interpolation.jett", "Hello, World!2 + 3 = 5");
}

#[test]
fn run_file_capture_stdout_captures_json_runtime_output() {
    assert_run_stdout(
        "json_tree_parse_runtime.jett",
        r#"{"city":"Cluj","scores":[1,true,null]}"#,
    );
}

run_pass_fixture!(
    run_pass_namespace_qualified_functions,
    "namespace_qualified_functions.jett"
);
run_pass_fixture!(
    run_pass_namespace_qualified_types,
    "namespace_qualified_types.jett"
);
run_pass_fixture!(
    run_pass_namespace_dotted_qualified_types,
    "namespace_dotted_qualified_types.jett"
);
run_pass_fixture!(
    run_pass_namespace_duplicate_leaf_types,
    "namespace_duplicate_leaf_types.jett"
);
run_pass_fixture!(
    run_pass_namespace_duplicate_leaf_interfaces,
    "namespace_duplicate_leaf_interfaces.jett"
);
run_pass_fixture!(run_pass_namespace_use_alias, "namespace_use_alias.jett");
run_pass_fixture!(
    run_pass_namespace_comptime_reflection_aliases,
    "namespace_comptime_reflection_aliases.jett"
);
run_pass_fixture!(
    run_pass_namespace_exports_syntax,
    "namespace_exports_syntax.jett"
);
run_pass_fixture!(
    run_pass_namespace_qualified_interface_implement,
    "namespace_qualified_interface_implement.jett"
);
run_pass_fixture!(
    run_pass_namespace_qualified_actors,
    "namespace_qualified_actors.jett"
);
run_pass_fixture!(
    run_pass_namespace_runtime_verify_context,
    "namespace_runtime_verify_context.jett"
);
run_pass_fixture!(
    run_pass_namespace_runtime_main_context,
    "namespace_runtime_main_context.jett"
);
run_pass_fixture!(
    run_pass_handle_result_optional,
    "handle_result_optional.jett"
);
run_pass_fixture!(run_pass_bitfield_roundtrip, "bitfield_roundtrip.jett");
run_pass_fixture!(
    run_pass_bitfield_payload_roundtrip,
    "bitfield_payload_roundtrip.jett"
);
run_pass_fixture!(run_pass_generic_struct, "generic_struct.jett");
run_pass_fixture!(run_pass_generic_function, "generic_function.jett");
run_pass_fixture!(run_pass_comptime_type_bind, "comptime_type_bind.jett");
run_pass_fixture!(run_pass_type_reflection, "type_reflection.jett");
run_pass_fixture!(run_pass_type_info_reflection, "type_info_reflection.jett");
run_pass_fixture!(
    run_pass_reflection_type_id_duplicate_generic_owners,
    "reflection_type_id_duplicate_generic_owners.jett"
);
run_pass_fixture!(
    run_pass_reflection_type_id_duplicate_construction,
    "reflection_type_id_duplicate_construction.jett"
);
run_pass_fixture!(
    run_pass_reflection_type_id_duplicate_named_owners,
    "reflection_type_id_duplicate_named_owners.jett"
);
run_pass_fixture!(
    run_pass_type_construction_builder,
    "type_construction_builder.jett"
);
run_pass_fixture!(
    run_pass_type_construction_bitfield,
    "type_construction_bitfield.jett"
);
run_pass_fixture!(
    run_pass_type_construction_enum,
    "type_construction_enum.jett"
);
run_pass_fixture!(run_pass_actor_counter, "actor_counter.jett");
run_pass_fixture!(
    run_pass_numeric_literal_contexts,
    "numeric_literal_contexts.jett"
);
run_pass_fixture!(
    run_pass_generic_reflection_branch_specialization,
    "generic_reflection_branch_specialization.jett"
);
run_pass_fixture!(
    run_pass_generic_reflection_match_specialization,
    "generic_reflection_match_specialization.jett"
);
run_pass_fixture!(
    run_pass_structured_concurrency,
    "structured_concurrency.jett"
);
run_pass_fixture!(run_pass_map_operations, "map_operations.jett");
run_pass_fixture!(run_pass_list_operations, "list_operations.jett");
run_pass_fixture!(run_pass_math_operations, "math_operations.jett");
run_pass_fixture!(run_pass_json_serialize, "json_serialize.jett");
run_pass_fixture!(run_pass_json_sized_primitives, "json_sized_primitives.jett");
run_pass_fixture!(run_pass_json_serialize_public, "json_serialize_public.jett");
run_pass_fixture!(
    run_pass_json_serialize_public_omits_secret_fields,
    "json_serialize_public_omits_secret_fields.jett"
);
run_pass_fixture!(run_pass_json_parse, "json_parse.jett");
run_pass_fixture!(
    run_pass_json_parse_error_parity,
    "json_parse_error_parity.jett"
);
run_pass_fixture!(run_pass_json_parse_raw, "json_parse_raw.jett");
run_pass_fixture!(
    run_pass_json_raw_facade_tree_surface,
    "json_raw_facade_tree_surface.jett"
);
run_pass_fixture!(
    run_pass_json_raw_value_access_edges,
    "json_raw_value_access_edges.jett"
);
run_pass_fixture!(run_pass_json_raw_tree_parity, "json_raw_tree_parity.jett");
run_pass_fixture!(
    run_pass_json_value_tree_compatibility,
    "json_value_tree_compatibility.jett"
);
run_pass_fixture!(run_pass_json_enum_shapes, "json_enum_shapes.jett");
run_pass_fixture!(run_pass_json_bitfield_shapes, "json_bitfield_shapes.jett");
run_pass_fixture!(run_pass_json_roundtrip_user, "json_roundtrip_user.jett");
run_pass_fixture!(
    run_pass_json_parse_refinement_valid,
    "json_parse_refinement_valid.jett"
);
run_pass_fixture!(
    run_pass_json_reflection_flat_serializer,
    "json_reflection_flat_serializer.jett"
);
run_pass_fixture!(
    run_pass_json_reflection_bridge_parity,
    "json_reflection_bridge_parity.jett"
);
run_pass_fixture!(
    run_pass_json_reflection_nested_serializer,
    "json_reflection_nested_serializer.jett"
);
run_pass_fixture!(
    run_pass_json_reflection_flat_decoder,
    "json_reflection_flat_decoder.jett"
);
run_pass_fixture!(
    run_pass_json_reflection_nested_decoder,
    "json_reflection_nested_decoder.jett"
);
run_pass_fixture!(
    run_pass_json_reflection_parse_wrapper,
    "json_reflection_parse_wrapper.jett"
);
run_pass_fixture!(
    run_pass_json_public_secret_policy,
    "json_public_secret_policy.jett"
);
run_pass_fixture!(run_pass_json_unknown_fields, "json_unknown_fields.jett");
run_pass_fixture!(
    run_pass_json_stdlib_bridge_delegation,
    "json_stdlib_bridge_delegation.jett"
);
run_pass_fixture!(run_pass_json_tree_value, "json_tree_value.jett");
run_pass_fixture!(
    run_pass_json_tree_parse_scalar,
    "json_tree_parse_scalar.jett"
);
run_pass_fixture!(
    run_pass_json_tree_parse_compound,
    "json_tree_parse_compound.jett"
);
run_pass_fixture!(run_pass_json_tree_accessors, "json_tree_accessors.jett");
run_pass_fixture!(
    run_pass_json_tree_parse_runtime,
    "json_tree_parse_runtime.jett"
);
run_pass_fixture!(
    run_pass_json_runtime_reflection_metadata,
    "json_runtime_reflection_metadata.jett"
);
run_pass_fixture!(
    run_pass_json_tree_reflection_parse_wrapper,
    "json_tree_reflection_parse_wrapper.jett"
);
run_pass_fixture!(run_pass_escape_sequences, "escape_sequences.jett");
run_pass_fixture!(run_pass_list_higher_order, "list_higher_order.jett");
run_pass_fixture!(run_pass_inline_functions, "inline_functions.jett");
run_pass_fixture!(run_pass_string_operations, "string_operations.jett");
run_pass_fixture!(run_pass_string_extra, "string_extra.jett");
run_pass_fixture!(run_pass_math_extra, "math_extra.jett");
run_pass_fixture!(run_pass_pipeline_into, "pipeline_into.jett");
run_pass_fixture!(run_pass_string_iteration, "string_iteration.jett");
run_pass_fixture!(run_pass_list_map_extra, "list_map_extra.jett");
run_pass_fixture!(run_pass_encoding, "encoding.jett");
run_pass_fixture!(run_pass_string_chars, "string_chars.jett");
run_pass_fixture!(run_pass_crypto, "crypto.jett");
run_pass_fixture!(run_pass_closures, "closures.jett");
run_pass_fixture!(run_pass_use_imports, "use_imports.jett");
run_pass_fixture!(run_pass_loops, "loops.jett");
run_pass_fixture!(run_pass_conversions, "conversions.jett");
run_pass_fixture!(run_pass_set_operations, "set_operations.jett");
run_pass_fixture!(run_pass_error_handling, "error_handling.jett");

compile_fail_fixture!(compile_fail_unhandled_result, "unhandled_result.jett");

run_pass_fixture!(run_pass_string_search, "string_search.jett");
run_pass_fixture!(run_pass_time_and_os, "time_and_os.jett");
run_pass_fixture!(run_pass_math_trig, "math_trig.jett");
run_pass_fixture!(run_pass_logical_ops, "logical_ops.jett");
run_pass_fixture!(run_pass_trace_basic, "trace_basic.jett");
run_pass_fixture!(run_pass_breakpoint_basic, "breakpoint_basic.jett");
run_pass_fixture!(run_pass_closures_advanced, "closures_advanced.jett");
run_pass_fixture!(run_pass_math_advanced, "math_advanced.jett");
run_pass_fixture!(run_pass_csv_operations, "csv_operations.jett");
run_pass_fixture!(run_pass_string_format, "string_format.jett");
run_pass_fixture!(run_pass_list_extras, "list_extras.jett");
run_pass_fixture!(run_pass_map_advanced, "map_advanced.jett");
run_pass_fixture!(run_pass_bytes_operations, "bytes_operations.jett");
run_pass_fixture!(run_pass_enum_advanced, "enum_advanced.jett");

compile_fail_fixture!(
    compile_fail_type_mismatch_return,
    "type_mismatch_return.jett"
);
compile_fail_fixture!(
    compile_fail_non_exhaustive_match,
    "non_exhaustive_match.jett"
);
compile_fail_fixture!(
    compile_fail_assert_outside_verify,
    "assert_outside_verify.jett"
);
compile_fail_fixture!(
    compile_fail_breakpoint_condition_not_bool,
    "breakpoint_condition_not_bool.jett"
);
compile_fail_fixture!(compile_fail_unhandled_optional, "unhandled_optional.jett");
compile_fail_fixture!(compile_fail_trace_unknown, "trace_unknown.jett");
compile_fail_fixture!(compile_fail_unknown_type, "unknown_type.jett");
compile_fail_fixture!(
    compile_fail_pipeline_builtin_input_mismatch,
    "pipeline_builtin_input_mismatch.jett"
);
compile_fail_fixture!(
    compile_fail_pipeline_extra_arg_mismatch,
    "pipeline_extra_arg_mismatch.jett"
);
compile_fail_fixture!(
    compile_fail_pipeline_return_type_mismatch,
    "pipeline_return_type_mismatch.jett"
);
compile_fail_fixture!(
    compile_fail_pipeline_step_not_callable,
    "pipeline_step_not_callable.jett"
);
compile_fail_fixture!(
    compile_fail_function_statement_limit,
    "function_statement_limit.jett"
);
compile_fail_fixture!(
    compile_fail_function_nesting_depth_limit,
    "function_nesting_depth_limit.jett"
);
compile_fail_fixture!(
    compile_fail_function_cyclomatic_complexity_limit,
    "function_cyclomatic_complexity_limit.jett"
);
compile_fail_fixture!(
    compile_fail_namespace_use_alias_unknown_target,
    "namespace_use_alias_unknown_target.jett"
);
compile_fail_fixture!(
    compile_fail_namespace_private_function,
    "namespace_private_function.jett"
);
compile_fail_fixture!(
    compile_fail_namespace_private_type,
    "namespace_private_type.jett"
);
compile_fail_fixture!(
    compile_fail_namespace_private_alias,
    "namespace_private_alias.jett"
);
compile_fail_fixture!(
    compile_fail_namespace_private_interface,
    "namespace_private_interface.jett"
);
compile_fail_fixture!(
    compile_fail_namespace_exported_flat_function,
    "namespace_exported_flat_function.jett"
);
compile_fail_fixture!(
    compile_fail_namespace_exported_flat_type,
    "namespace_exported_flat_type.jett"
);
compile_fail_fixture!(
    compile_fail_namespace_exported_flat_type_alias,
    "namespace_exported_flat_type_alias.jett"
);
compile_fail_fixture!(
    compile_fail_namespace_exported_flat_enum,
    "namespace_exported_flat_enum.jett"
);
compile_fail_fixture!(
    compile_fail_namespace_exported_flat_bitfield,
    "namespace_exported_flat_bitfield.jett"
);
compile_fail_fixture!(
    compile_fail_namespace_exported_flat_interface,
    "namespace_exported_flat_interface.jett"
);
compile_fail_fixture!(
    compile_fail_namespace_exported_flat_mutual_function,
    "namespace_exported_flat_mutual_function.jett"
);
compile_fail_fixture!(
    compile_fail_namespace_private_mutual_function,
    "namespace_private_mutual_function.jett"
);
compile_fail_fixture!(
    compile_fail_namespace_private_mutual_alias,
    "namespace_private_mutual_alias.jett"
);
compile_fail_fixture!(
    compile_fail_namespace_private_mutual_flat,
    "namespace_private_mutual_flat.jett"
);
compile_fail_fixture!(
    compile_fail_namespace_actor_body_checked,
    "namespace_actor_body_checked.jett"
);
compile_fail_fixture!(
    compile_fail_namespace_private_actor,
    "namespace_private_actor.jett"
);
compile_fail_fixture!(
    compile_fail_namespace_exported_flat_actor,
    "namespace_exported_flat_actor.jett"
);
compile_fail_fixture!(
    compile_fail_stdlib_namespace_collision,
    "stdlib_namespace_collision.jett"
);
compile_fail_fixture!(
    compile_fail_map_literal_key_type_mismatch,
    "map_literal_key_type_mismatch.jett"
);
compile_fail_fixture!(
    compile_fail_map_literal_value_type_mismatch,
    "map_literal_value_type_mismatch.jett"
);
compile_fail_fixture!(
    compile_fail_list_literal_value_type_mismatch,
    "list_literal_value_type_mismatch.jett"
);
compile_fail_fixture!(
    compile_fail_uint8_literal_out_of_range,
    "uint8_literal_out_of_range.jett"
);
compile_fail_fixture!(
    compile_fail_uint8_binary_literal_out_of_range,
    "uint8_binary_literal_out_of_range.jett"
);
compile_fail_fixture!(
    compile_fail_uint8_binary_expression_out_of_range,
    "uint8_binary_expression_out_of_range.jett"
);
compile_fail_fixture!(
    compile_fail_int8_negative_literal_out_of_range,
    "int8_negative_literal_out_of_range.jett"
);
compile_fail_fixture!(
    compile_fail_integer_literal_overflow,
    "integer_literal_overflow.jett"
);
compile_fail_fixture!(
    compile_fail_enum_discriminant_overflow,
    "enum_discriminant_overflow.jett"
);
compile_fail_fixture!(
    compile_fail_uint8_handle_default_out_of_range,
    "uint8_handle_default_out_of_range.jett"
);
compile_fail_fixture!(
    compile_fail_generic_reflection_branch_reachable_then_error,
    "generic_reflection_branch_reachable_then_error.jett"
);
compile_fail_fixture!(
    compile_fail_generic_reflection_branch_reachable_else_error,
    "generic_reflection_branch_reachable_else_error.jett"
);
compile_fail_fixture!(
    compile_fail_generic_reflection_local_fact_reachable_error,
    "generic_reflection_local_fact_reachable_error.jett"
);
compile_fail_fixture!(
    compile_fail_generic_reflection_helper_kind_fact_cache,
    "generic_reflection_helper_kind_fact_cache.jett"
);
compile_fail_fixture!(
    compile_fail_generic_reflection_helper_primitive_fact_cache,
    "generic_reflection_helper_primitive_fact_cache.jett"
);
compile_fail_fixture!(
    compile_fail_generic_reflection_guarded_type_arg_error,
    "generic_reflection_guarded_type_arg_error.jett"
);
compile_fail_fixture!(
    compile_fail_generic_reflection_guarded_unknown_fact,
    "generic_reflection_guarded_unknown_fact.jett"
);
compile_fail_fixture!(
    compile_fail_generic_reflection_top_level_type_arg_error,
    "generic_reflection_top_level_type_arg_error.jett"
);
compile_fail_fixture!(
    compile_fail_generic_reflection_match_reachable_arm_error,
    "generic_reflection_match_reachable_arm_error.jett"
);
compile_fail_fixture!(
    compile_fail_generic_reflection_match_unknown_fact,
    "generic_reflection_match_unknown_fact.jett"
);
compile_fail_fixture!(
    compile_fail_actor_message_arg_type_mismatch,
    "actor_message_arg_type_mismatch.jett"
);
compile_fail_fixture!(
    compile_fail_actor_respond_type_mismatch,
    "actor_respond_type_mismatch.jett"
);
compile_fail_fixture!(
    compile_fail_actor_spawn_arg_type_mismatch,
    "actor_spawn_arg_type_mismatch.jett"
);
compile_fail_fixture!(
    compile_fail_generic_function_secret_argument_mismatch,
    "generic_function_secret_argument_mismatch.jett"
);
compile_fail_fixture!(
    compile_fail_generic_function_body_type_error,
    "generic_function_body_type_error.jett"
);
compile_fail_fixture!(
    compile_fail_result_ok_payload_requires_handle,
    "result_ok_payload_requires_handle.jett"
);
compile_fail_fixture!(
    compile_fail_result_fail_payload_requires_handle,
    "result_fail_payload_requires_handle.jett"
);
compile_fail_fixture!(
    compile_fail_some_payload_requires_handle,
    "some_payload_requires_handle.jett"
);
compile_fail_fixture!(
    compile_fail_property_given_type_binding,
    "property_given_type_binding.jett"
);
compile_fail_fixture!(
    compile_fail_json_private_tree_parse_scalar,
    "json_private_tree_parse_scalar.jett"
);
compile_fail_fixture!(
    compile_fail_json_private_parse_reflected,
    "json_private_parse_reflected.jett"
);
compile_fail_fixture!(
    compile_fail_json_private_decode_tree_reflected,
    "json_private_decode_tree_reflected.jett"
);
compile_fail_fixture!(
    compile_fail_json_private_serialize_reflected,
    "json_private_serialize_reflected.jett"
);
compile_fail_fixture!(compile_fail_duplicate_field, "duplicate_field.jett");
compile_fail_fixture!(
    compile_fail_json_serialize_secret_struct_blocked,
    "json_serialize_secret_struct_blocked.jett"
);
compile_fail_fixture!(
    compile_fail_json_serialize_map_key_must_be_string,
    "json_serialize_map_key_must_be_string.jett"
);
compile_fail_fixture!(
    compile_fail_json_serialize_public_map_key_must_be_string,
    "json_serialize_public_map_key_must_be_string.jett"
);
compile_fail_fixture!(
    compile_fail_json_serialize_requires_view,
    "json_serialize_requires_view.jett"
);
compile_fail_fixture!(
    compile_fail_json_serialize_public_requires_view,
    "json_serialize_public_requires_view.jett"
);
compile_fail_fixture!(
    compile_fail_json_serialize_public_top_level_secret_blocked,
    "json_serialize_public_top_level_secret_blocked.jett"
);
compile_fail_fixture!(
    compile_fail_json_serialize_secret_generic_blocked,
    "json_serialize_secret_generic_blocked.jett"
);
compile_fail_fixture!(
    compile_fail_json_parse_map_key_must_be_string,
    "json_parse_map_key_must_be_string.jett"
);
compile_fail_fixture!(
    compile_fail_json_parse_requires_type_arg,
    "json_parse_requires_type_arg.jett"
);
compile_fail_fixture!(
    compile_fail_json_parse_requires_handle,
    "json_parse_requires_handle.jett"
);
compile_fail_fixture!(
    compile_fail_json_parse_raw_requires_handle,
    "json_parse_raw_requires_handle.jett"
);
compile_fail_fixture!(
    compile_fail_json_parse_raw_wrong_type_arg,
    "json_parse_raw_wrong_type_arg.jett"
);
compile_fail_fixture!(
    compile_fail_json_raw_facade_argument_shapes,
    "json_raw_facade_argument_shapes.jett"
);
compile_fail_fixture!(
    compile_fail_json_raw_cast_requires_handle,
    "json_raw_cast_requires_handle.jett"
);
compile_fail_fixture!(
    compile_fail_json_value_user_json_tree_incompatible,
    "json_value_user_json_tree_incompatible.jett"
);
compile_fail_fixture!(
    compile_fail_serialize_annotation_requires_string,
    "serialize_annotation_requires_string.jett"
);
compile_fail_fixture!(
    compile_fail_comptime_type_bind_requires_direct_type_info,
    "comptime_type_bind_requires_direct_type_info.jett"
);
compile_fail_fixture!(
    compile_fail_comptime_type_bind_requires_direct_field_loop,
    "comptime_type_bind_requires_direct_field_loop.jett"
);
compile_fail_fixture!(
    compile_fail_comptime_type_bind_requires_trusted_args_loop,
    "comptime_type_bind_requires_trusted_args_loop.jett"
);
compile_fail_fixture!(
    compile_fail_comptime_type_bind_requires_literal_type_arg_index,
    "comptime_type_bind_requires_literal_type_arg_index.jett"
);
compile_fail_fixture!(
    compile_fail_comptime_type_bind_requires_direct_variant_field_loop,
    "comptime_type_bind_requires_direct_variant_field_loop.jett"
);
compile_fail_fixture!(
    compile_fail_comptime_type_bind_scope,
    "comptime_type_bind_scope.jett"
);
compile_fail_fixture!(
    compile_fail_type_info_wrong_arity,
    "type_info_wrong_arity.jett"
);
compile_fail_fixture!(
    compile_fail_type_arg_wrong_arity,
    "type_arg_wrong_arity.jett"
);
compile_fail_fixture!(
    compile_fail_type_variants_wrong_arity,
    "type_variants_wrong_arity.jett"
);
compile_fail_fixture!(
    compile_fail_type_variant_value_wrong_arity,
    "type_variant_value_wrong_arity.jett"
);
compile_fail_fixture!(
    compile_fail_type_variant_value_wrong_value_arg,
    "type_variant_value_wrong_value_arg.jett"
);
compile_fail_fixture!(
    compile_fail_type_variant_field_value_wrong_arity,
    "type_variant_field_value_wrong_arity.jett"
);
compile_fail_fixture!(
    compile_fail_type_variant_field_value_wrong_field_arg,
    "type_variant_field_value_wrong_field_arg.jett"
);
compile_fail_fixture!(
    compile_fail_type_variant_field_value_wrong_requested_type,
    "type_variant_field_value_wrong_requested_type.jett"
);
compile_fail_fixture!(
    compile_fail_type_bitfield_layout_wrong_arity,
    "type_bitfield_layout_wrong_arity.jett"
);
compile_fail_fixture!(
    compile_fail_type_bitfield_fields_wrong_arity,
    "type_bitfield_fields_wrong_arity.jett"
);
compile_fail_fixture!(
    compile_fail_type_field_value_wrong_arity,
    "type_field_value_wrong_arity.jett"
);
compile_fail_fixture!(
    compile_fail_type_field_value_wrong_field_arg,
    "type_field_value_wrong_field_arg.jett"
);
compile_fail_fixture!(
    compile_fail_type_field_value_wrong_value_arg,
    "type_field_value_wrong_value_arg.jett"
);
compile_fail_fixture!(
    compile_fail_type_field_value_wrong_requested_type,
    "type_field_value_wrong_requested_type.jett"
);
compile_fail_fixture!(
    compile_fail_type_field_value_mismatched_metadata,
    "type_field_value_mismatched_metadata.jett"
);
compile_fail_fixture!(
    compile_fail_type_field_value_non_struct_owner,
    "type_field_value_non_struct_owner.jett"
);
compile_fail_fixture!(
    compile_fail_type_kind_tag_wrong_arity,
    "type_kind_tag_wrong_arity.jett"
);
compile_fail_fixture!(
    compile_fail_type_primitive_tag_wrong_arity,
    "type_primitive_tag_wrong_arity.jett"
);
compile_fail_fixture!(
    compile_fail_type_construct_put_wrong_value_type,
    "type_construct_put_wrong_value_type.jett"
);
compile_fail_fixture!(
    compile_fail_type_construct_start_wrong_arity,
    "type_construct_start_wrong_arity.jett"
);
compile_fail_fixture!(
    compile_fail_type_construct_variant_start_non_enum_owner,
    "type_construct_variant_start_non_enum_owner.jett"
);
compile_fail_fixture!(
    compile_fail_type_construct_finish_non_struct_owner,
    "type_construct_finish_non_struct_owner.jett"
);

#[test]
fn multifile_cross_file_calls() {
    let path = workspace_root()
        .join("tests")
        .join("multifile")
        .join("main.jett");
    run_file(&path)
        .unwrap_or_else(|err| panic!("expected multifile test to run successfully: {err}"));
}

#[test]
fn test_file_loads_project_siblings() {
    let path = workspace_root()
        .join("tests")
        .join("multifile_verify")
        .join("checks.jett");
    let result = test_file(&path)
        .unwrap_or_else(|err| panic!("expected project verify test to run successfully: {err}"));

    assert_eq!(result.total, 1);
    assert_eq!(result.passed, 1);
    assert_eq!(result.failed, 0);
}

#[test]
fn stdlib_loaded_for_build_source() {
    let source = r#"
namespace app

verify stdlib_source:
    assert stdlib.jett_stdlib_loaded() == true
"#;
    let result = build_source(source, "memory.jett");
    assert!(
        !result.has_errors,
        "expected in-memory source to see stdlib marker:\n{}",
        result
            .diagnostics
            .iter()
            .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn stdlib_json_exports_loaded_for_build_source() {
    let source = r#"
namespace app

function build_source_json_summary() returns string:
    json.JsonTree raw = json.parse_raw("{{\"name\":\"Ada\"}}") handle error:
        return error
    json.JsonTree name = json.field(raw, "name") handle:
        return "missing"
    return json.as_string(name) handle error:
        default error

verify build_source_json:
    assert build_source_json_summary() == "Ada"
"#;
    let result = build_source(source, "memory.jett");
    assert!(
        !result.has_errors,
        "expected in-memory source to see stdlib JSON exports:\n{}",
        result
            .diagnostics
            .iter()
            .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn stdlib_json_exports_loaded_for_hover_type() {
    let source = r#"
namespace app

function hover_json() returns json.JsonTree:
    return json.parse_raw("null") handle error:
        default json.JsonTree.string_value(error)
"#;

    let ty = hover_type(source, 5, 12);
    assert_eq!(
        ty,
        Some("result[json.JsonTree, string]".to_string()),
        "expected hover to see stdlib JsonTree raw facade signature"
    );
}

#[test]
fn stdlib_json_raw_facade_hover_types_are_json_tree_first() {
    let source = r#"namespace app

function hover_raw_facades(view root: json.JsonTree) returns string:
    optional[json.JsonTree] name = json.field(root, "name")
    optional[json.JsonTree] item = json.index(root, 0)
    result[int64, string] length = json.array_length(root)
    result[string, string] text = json.as_string(root)
    string raw = json.serialize_raw(root)
    string kind = json.kind(root)
    bool nullish = json.is_null(root)
    bool boolean = json.is_bool(root)
    bool number = json.is_number(root)
    bool textish = json.is_string(root)
    bool array = json.is_array(root)
    bool object = json.is_object(root)
    result[list[string], string] keys = json.object_keys(root)
    result[int64, string] integer = json.as_int64(root)
    result[float64, string] float = json.as_float64(root)
    result[bool, string] truth = json.as_bool(root)
    return "ok"
"#;

    assert_eq!(
        hover_type(source, 4, 36),
        Some("optional[json.JsonTree]".to_string())
    );
    assert_eq!(
        hover_type(source, 5, 36),
        Some("optional[json.JsonTree]".to_string())
    );
    assert_eq!(
        hover_type(source, 6, 36),
        Some("result[int64, string]".to_string())
    );
    assert_eq!(
        hover_type(source, 7, 35),
        Some("result[string, string]".to_string())
    );
    assert_eq!(hover_type(source, 8, 18), Some("string".to_string()));
    assert_eq!(hover_type(source, 9, 19), Some("string".to_string()));
    assert_eq!(hover_type(source, 10, 20), Some("bool".to_string()));
    assert_eq!(hover_type(source, 11, 20), Some("bool".to_string()));
    assert_eq!(hover_type(source, 12, 19), Some("bool".to_string()));
    assert_eq!(hover_type(source, 13, 20), Some("bool".to_string()));
    assert_eq!(hover_type(source, 14, 18), Some("bool".to_string()));
    assert_eq!(hover_type(source, 15, 19), Some("bool".to_string()));
    assert_eq!(
        hover_type(source, 16, 41),
        Some("result[list[string], string]".to_string())
    );
    assert_eq!(
        hover_type(source, 17, 37),
        Some("result[int64, string]".to_string())
    );
    assert_eq!(
        hover_type(source, 18, 38),
        Some("result[float64, string]".to_string())
    );
    assert_eq!(
        hover_type(source, 19, 35),
        Some("result[bool, string]".to_string())
    );
}

#[test]
fn completions_hide_private_stdlib_json_hooks() {
    let source = "namespace app\n\nfunction main() returns nothing:\n    return nothing\n";
    let candidates = completions(source);

    for expected in [
        "json.JsonTree",
        "json.parse_raw",
        "json.serialize_raw",
        "json.kind",
        "json.is_null",
        "json.is_bool",
        "json.is_number",
        "json.is_string",
        "json.is_array",
        "json.is_object",
        "json.field",
        "json.index",
        "json.array_length",
        "json.object_keys",
        "json.as_string",
        "json.as_int64",
        "json.as_float64",
        "json.as_bool",
    ] {
        assert!(
            candidates.iter().any(|(name, _)| name == expected),
            "expected completions to include {expected}"
        );
    }
    assert!(
        !candidates
            .iter()
            .any(|(name, _)| name == "json.json_parse_reflected"
                || name == "json.json_decode_tree_reflected"
                || name == "json.json_serialize_public_reflected"
                || name == "json.json_serialize_reflected"),
        "private stdlib JSON hooks should not leak into completions"
    );
}

#[test]
fn completions_at_includes_same_namespace_private_helpers() {
    let source = r#"
namespace api

function private_helper() returns int64:
    return 1

export function public_helper() returns int64:
    return private_helper()

namespace app

function main() returns nothing:
    return nothing
"#;

    let api_candidates = completions_at(source, 8, 12);
    assert!(
        api_candidates
            .iter()
            .any(|(name, _)| name == "api.private_helper"),
        "same-namespace completions should include private helpers"
    );

    let app_candidates = completions_at(source, 13, 12);
    assert!(
        !app_candidates
            .iter()
            .any(|(name, _)| name == "api.private_helper"),
        "external completions should hide private namespaced helpers"
    );
    assert!(
        app_candidates
            .iter()
            .any(|(name, _)| name == "api.public_helper"),
        "external completions should include exported namespaced helpers"
    );
}

#[test]
fn stdlib_loaded_for_test_file() {
    let path = workspace_root()
        .join("tests")
        .join("run_pass")
        .join("stdlib_loading.jett");
    let result = test_file(&path)
        .unwrap_or_else(|err| panic!("expected jett test path to load stdlib: {err}"));
    assert_eq!(result.failed, 0);
}
