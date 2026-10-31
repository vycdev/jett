//! Selected machine payloads inside optional, result, and string-keyed map wrappers.
use super::{Oracle, run_case};

#[test]
fn native_qualified_machine_json_wrapper_optional_direct_serialize_matches_reference_in_both_profiles_without_source()
 {
    run_case(
        "wrapper_optional_direct_serialize",
        include_str!("wrappers/optional_direct_serialize.jett"),
        Oracle::Exact("{\"state\":\"ready\",\"payload\":{\"count\":41}}\nnull\nretained:41:none\n"),
    );
}

#[test]
fn native_qualified_machine_json_wrapper_optional_pipeline_serialize_matches_reference_in_both_profiles_without_source()
 {
    run_case(
        "wrapper_optional_pipeline_serialize",
        include_str!("wrappers/optional_pipeline_serialize.jett"),
        Oracle::Exact("{\"state\":\"ready\",\"payload\":{\"count\":41}}\nnull\nretained:41:none\n"),
    );
}

#[test]
fn native_qualified_machine_json_wrapper_optional_direct_serialize_public_matches_reference_in_both_profiles_without_source()
 {
    run_case(
        "wrapper_optional_direct_serialize_public",
        include_str!("wrappers/optional_direct_serialize_public.jett"),
        Oracle::Exact("{\"state\":\"ready\",\"payload\":{\"count\":41}}\nnull\nretained:41:none\n"),
    );
}

#[test]
fn native_qualified_machine_json_wrapper_optional_pipeline_serialize_public_matches_reference_in_both_profiles_without_source()
 {
    run_case(
        "wrapper_optional_pipeline_serialize_public",
        include_str!("wrappers/optional_pipeline_serialize_public.jett"),
        Oracle::Exact("{\"state\":\"ready\",\"payload\":{\"count\":41}}\nnull\nretained:41:none\n"),
    );
}

#[test]
fn native_qualified_machine_json_wrapper_optional_direct_parse_matches_reference_in_both_profiles_without_source()
 {
    run_case(
        "wrapper_optional_direct_parse",
        include_str!("wrappers/optional_direct_parse.jett"),
        Oracle::Exact("parsed:41:none\n"),
    );
}

#[test]
fn native_qualified_machine_json_wrapper_optional_pipeline_parse_matches_reference_in_both_profiles_without_source()
 {
    run_case(
        "wrapper_optional_pipeline_parse",
        include_str!("wrappers/optional_pipeline_parse.jett"),
        Oracle::Exact("parsed:41:none\n"),
    );
}

#[test]
fn native_qualified_machine_json_wrapper_optional_direct_parse_exact_matches_reference_in_both_profiles_without_source()
 {
    run_case(
        "wrapper_optional_direct_parse_exact",
        include_str!("wrappers/optional_direct_parse_exact.jett"),
        Oracle::Exact("parsed:41:none\n"),
    );
}

#[test]
fn native_qualified_machine_json_wrapper_optional_pipeline_parse_exact_matches_reference_in_both_profiles_without_source()
 {
    run_case(
        "wrapper_optional_pipeline_parse_exact",
        include_str!("wrappers/optional_pipeline_parse_exact.jett"),
        Oracle::Exact("parsed:41:none\n"),
    );
}

#[test]
fn native_qualified_machine_json_wrapper_result_direct_serialize_matches_reference_in_both_profiles_without_source()
 {
    run_case(
        "wrapper_result_direct_serialize",
        include_str!("wrappers/result_direct_serialize.jett"),
        Oracle::Exact(
            "{\"ok\":{\"state\":\"ready\",\"payload\":{\"count\":51}}}\n{\"fail\":{\"state\":\"ready\",\"payload\":{\"count\":52}}}\nretained:ok:51:fail:52\n",
        ),
    );
}

#[test]
fn native_qualified_machine_json_wrapper_result_pipeline_serialize_matches_reference_in_both_profiles_without_source()
 {
    run_case(
        "wrapper_result_pipeline_serialize",
        include_str!("wrappers/result_pipeline_serialize.jett"),
        Oracle::Exact(
            "{\"ok\":{\"state\":\"ready\",\"payload\":{\"count\":51}}}\n{\"fail\":{\"state\":\"ready\",\"payload\":{\"count\":52}}}\nretained:ok:51:fail:52\n",
        ),
    );
}

#[test]
fn native_qualified_machine_json_wrapper_result_direct_serialize_public_matches_reference_in_both_profiles_without_source()
 {
    run_case(
        "wrapper_result_direct_serialize_public",
        include_str!("wrappers/result_direct_serialize_public.jett"),
        Oracle::Exact(
            "{\"ok\":{\"state\":\"ready\",\"payload\":{\"count\":51}}}\n{\"fail\":{\"state\":\"ready\",\"payload\":{\"count\":52}}}\nretained:ok:51:fail:52\n",
        ),
    );
}

#[test]
fn native_qualified_machine_json_wrapper_result_pipeline_serialize_public_matches_reference_in_both_profiles_without_source()
 {
    run_case(
        "wrapper_result_pipeline_serialize_public",
        include_str!("wrappers/result_pipeline_serialize_public.jett"),
        Oracle::Exact(
            "{\"ok\":{\"state\":\"ready\",\"payload\":{\"count\":51}}}\n{\"fail\":{\"state\":\"ready\",\"payload\":{\"count\":52}}}\nretained:ok:51:fail:52\n",
        ),
    );
}

#[test]
fn native_qualified_machine_json_wrapper_result_direct_parse_matches_reference_in_both_profiles_without_source()
 {
    run_case(
        "wrapper_result_direct_parse",
        include_str!("wrappers/result_direct_parse.jett"),
        Oracle::Exact("parsed:ok:51:fail:52\n"),
    );
}

#[test]
fn native_qualified_machine_json_wrapper_result_pipeline_parse_matches_reference_in_both_profiles_without_source()
 {
    run_case(
        "wrapper_result_pipeline_parse",
        include_str!("wrappers/result_pipeline_parse.jett"),
        Oracle::Exact("parsed:ok:51:fail:52\n"),
    );
}

#[test]
fn native_qualified_machine_json_wrapper_result_direct_parse_exact_matches_reference_in_both_profiles_without_source()
 {
    run_case(
        "wrapper_result_direct_parse_exact",
        include_str!("wrappers/result_direct_parse_exact.jett"),
        Oracle::Exact("parsed:ok:51:fail:52\n"),
    );
}

#[test]
fn native_qualified_machine_json_wrapper_result_pipeline_parse_exact_matches_reference_in_both_profiles_without_source()
 {
    run_case(
        "wrapper_result_pipeline_parse_exact",
        include_str!("wrappers/result_pipeline_parse_exact.jett"),
        Oracle::Exact("parsed:ok:51:fail:52\n"),
    );
}

#[test]
fn native_qualified_machine_json_wrapper_map_direct_serialize_matches_reference_in_both_profiles_without_source()
 {
    run_case(
        "wrapper_map_direct_serialize",
        include_str!("wrappers/map_direct_serialize.jett"),
        Oracle::Exact(
            "{\"entry\":{\"state\":\"ready\",\"payload\":{\"count\":61}}}\n{}\nretained:61:-1\n",
        ),
    );
}

#[test]
fn native_qualified_machine_json_wrapper_map_pipeline_serialize_matches_reference_in_both_profiles_without_source()
 {
    run_case(
        "wrapper_map_pipeline_serialize",
        include_str!("wrappers/map_pipeline_serialize.jett"),
        Oracle::Exact(
            "{\"entry\":{\"state\":\"ready\",\"payload\":{\"count\":61}}}\n{}\nretained:61:-1\n",
        ),
    );
}

#[test]
fn native_qualified_machine_json_wrapper_map_direct_serialize_public_matches_reference_in_both_profiles_without_source()
 {
    run_case(
        "wrapper_map_direct_serialize_public",
        include_str!("wrappers/map_direct_serialize_public.jett"),
        Oracle::Exact(
            "{\"entry\":{\"state\":\"ready\",\"payload\":{\"count\":61}}}\n{}\nretained:61:-1\n",
        ),
    );
}

#[test]
fn native_qualified_machine_json_wrapper_map_pipeline_serialize_public_matches_reference_in_both_profiles_without_source()
 {
    run_case(
        "wrapper_map_pipeline_serialize_public",
        include_str!("wrappers/map_pipeline_serialize_public.jett"),
        Oracle::Exact(
            "{\"entry\":{\"state\":\"ready\",\"payload\":{\"count\":61}}}\n{}\nretained:61:-1\n",
        ),
    );
}

#[test]
fn native_qualified_machine_json_wrapper_map_direct_parse_matches_reference_in_both_profiles_without_source()
 {
    run_case(
        "wrapper_map_direct_parse",
        include_str!("wrappers/map_direct_parse.jett"),
        Oracle::Exact("parsed:61:-1\n"),
    );
}

#[test]
fn native_qualified_machine_json_wrapper_map_pipeline_parse_matches_reference_in_both_profiles_without_source()
 {
    run_case(
        "wrapper_map_pipeline_parse",
        include_str!("wrappers/map_pipeline_parse.jett"),
        Oracle::Exact("parsed:61:-1\n"),
    );
}

#[test]
fn native_qualified_machine_json_wrapper_map_direct_parse_exact_matches_reference_in_both_profiles_without_source()
 {
    run_case(
        "wrapper_map_direct_parse_exact",
        include_str!("wrappers/map_direct_parse_exact.jett"),
        Oracle::Exact("parsed:61:-1\n"),
    );
}

#[test]
fn native_qualified_machine_json_wrapper_map_pipeline_parse_exact_matches_reference_in_both_profiles_without_source()
 {
    run_case(
        "wrapper_map_pipeline_parse_exact",
        include_str!("wrappers/map_pipeline_parse_exact.jett"),
        Oracle::Exact("parsed:61:-1\n"),
    );
}
