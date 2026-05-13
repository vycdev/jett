/// Returns true for the legacy raw JSON facade functions that are now backed
/// by `stdlib/json.jett`.
pub fn is_json_raw_facade(name: &str) -> bool {
    matches!(
        name,
        "json.parse_raw"
            | "json.serialize_raw"
            | "json.kind"
            | "json.is_null"
            | "json.is_bool"
            | "json.is_number"
            | "json.is_string"
            | "json.is_array"
            | "json.is_object"
            | "json.field"
            | "json.index"
            | "json.array_length"
            | "json.object_keys"
            | "json.as_string"
            | "json.as_int64"
            | "json.as_float64"
            | "json.as_bool"
    )
}

/// Returns true for JSON facade functions whose first argument should be
/// treated as an implicit view by ownership analysis.
pub fn is_json_implicit_view_facade(name: &str) -> bool {
    is_json_raw_facade(name)
        || matches!(
            name,
            "json.json_tree_serialize"
                | "json.json_tree_kind"
                | "json.json_tree_is_null"
                | "json.json_tree_is_bool"
                | "json.json_tree_is_number"
                | "json.json_tree_is_string"
                | "json.json_tree_is_array"
                | "json.json_tree_is_object"
                | "json.json_tree_field"
                | "json.json_tree_index"
                | "json.json_tree_array_length"
                | "json.json_tree_object_keys"
                | "json.json_tree_as_string"
                | "json.json_tree_as_int64"
                | "json.json_tree_as_float64"
                | "json.json_tree_as_bool"
        )
}
