/// Shared policy for public typed JSON bridges with compiler-owned checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JsonPublicBridgeSpec {
    pub hook: &'static str,
}

const JSON_PUBLIC_BRIDGE_SPECS: &[(&str, JsonPublicBridgeSpec)] = &[
    (
        "json.parse",
        JsonPublicBridgeSpec {
            hook: "json.json_parse_reflected",
        },
    ),
    (
        "json.parse_exact",
        JsonPublicBridgeSpec {
            hook: "json.json_parse_exact_reflected",
        },
    ),
    (
        "json.serialize",
        JsonPublicBridgeSpec {
            hook: "json.json_serialize_reflected",
        },
    ),
    (
        "json.serialize_public",
        JsonPublicBridgeSpec {
            hook: "json.json_serialize_public_reflected",
        },
    ),
];

const JSON_RAW_FACADE_NAMES: &[&str] = &[
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
    "json.object_field",
    "json.array_index",
    "json.require_field",
    "json.require_index",
];

const JSON_VIEW_TREE_HELPER_NAMES: &[&str] = &[
    "json.json_tree_serialize",
    "json.json_tree_kind",
    "json.json_tree_is_null",
    "json.json_tree_is_bool",
    "json.json_tree_is_number",
    "json.json_tree_is_string",
    "json.json_tree_is_array",
    "json.json_tree_is_object",
    "json.json_tree_field",
    "json.json_tree_index",
    "json.json_tree_array_length",
    "json.json_tree_object_keys",
    "json.json_tree_as_string",
    "json.json_tree_as_int64",
    "json.json_tree_as_float64",
    "json.json_tree_as_bool",
];

/// Returns the trusted stdlib hook for a compiler-policy JSON bridge.
pub fn json_public_bridge_spec(name: &str) -> Option<JsonPublicBridgeSpec> {
    JSON_PUBLIC_BRIDGE_SPECS
        .iter()
        .find_map(|(facade, spec)| (*facade == name).then_some(*spec))
}

/// Returns true for the public raw JSON facade functions backed by the stdlib
/// JSON module.
pub fn is_json_raw_facade(name: &str) -> bool {
    JSON_RAW_FACADE_NAMES.contains(&name)
}

/// Returns true for JSON facade functions whose first argument should be
/// treated as an implicit view by ownership analysis.
pub fn is_json_implicit_view_facade(name: &str) -> bool {
    is_json_raw_facade(name) || JSON_VIEW_TREE_HELPER_NAMES.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_facade_names_pin_public_stdlib_surface() {
        assert!(is_json_raw_facade("json.parse_raw"));
        assert!(is_json_raw_facade("json.field"));
        assert!(is_json_raw_facade("json.index"));
        assert!(is_json_raw_facade("json.as_bool"));
        assert!(is_json_raw_facade("json.require_field"));
        assert!(!is_json_raw_facade("json.json_tree_parse"));
    }

    #[test]
    fn public_bridge_specs_pin_trusted_hooks() {
        assert_eq!(
            json_public_bridge_spec("json.parse"),
            Some(JsonPublicBridgeSpec {
                hook: "json.json_parse_reflected",
            })
        );
        assert_eq!(
            json_public_bridge_spec("json.parse_exact"),
            Some(JsonPublicBridgeSpec {
                hook: "json.json_parse_exact_reflected",
            })
        );
        assert_eq!(
            json_public_bridge_spec("json.serialize"),
            Some(JsonPublicBridgeSpec {
                hook: "json.json_serialize_reflected",
            })
        );
        assert_eq!(
            json_public_bridge_spec("json.serialize_public"),
            Some(JsonPublicBridgeSpec {
                hook: "json.json_serialize_public_reflected",
            })
        );
        assert_eq!(json_public_bridge_spec("json.parse_raw"), None);
    }

    #[test]
    fn raw_facade_and_implicit_view_sets_share_policy() {
        assert!(is_json_raw_facade("json.serialize_raw"));
        assert!(is_json_implicit_view_facade("json.serialize_raw"));
        assert!(is_json_implicit_view_facade("json.json_tree_serialize"));
        assert!(!is_json_raw_facade("json.json_tree_serialize"));
        assert!(!is_json_implicit_view_facade("json.json_tree_parse"));
        assert!(!is_json_implicit_view_facade("json.parse"));
    }
}
