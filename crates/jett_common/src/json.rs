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
