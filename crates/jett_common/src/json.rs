/// Argument shapes for public raw JSON facades backed by `JsonTree` hooks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonRawFacadeArgs {
    RawString,
    Tree,
    TreeAndString,
    TreeAndInt64,
}

/// Shared policy for a public raw JSON facade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JsonRawFacadeSpec {
    pub hook: &'static str,
    pub args: JsonRawFacadeArgs,
}

/// Returns the trusted stdlib hook and argument shape for a raw JSON facade.
pub fn json_raw_facade_spec(name: &str) -> Option<JsonRawFacadeSpec> {
    let spec = match name {
        "json.parse_raw" => JsonRawFacadeSpec {
            hook: "json.json_tree_parse",
            args: JsonRawFacadeArgs::RawString,
        },
        "json.serialize_raw" => JsonRawFacadeSpec {
            hook: "json.json_tree_serialize",
            args: JsonRawFacadeArgs::Tree,
        },
        "json.kind" => JsonRawFacadeSpec {
            hook: "json.json_tree_kind",
            args: JsonRawFacadeArgs::Tree,
        },
        "json.is_null" => JsonRawFacadeSpec {
            hook: "json.json_tree_is_null",
            args: JsonRawFacadeArgs::Tree,
        },
        "json.is_bool" => JsonRawFacadeSpec {
            hook: "json.json_tree_is_bool",
            args: JsonRawFacadeArgs::Tree,
        },
        "json.is_number" => JsonRawFacadeSpec {
            hook: "json.json_tree_is_number",
            args: JsonRawFacadeArgs::Tree,
        },
        "json.is_string" => JsonRawFacadeSpec {
            hook: "json.json_tree_is_string",
            args: JsonRawFacadeArgs::Tree,
        },
        "json.is_array" => JsonRawFacadeSpec {
            hook: "json.json_tree_is_array",
            args: JsonRawFacadeArgs::Tree,
        },
        "json.is_object" => JsonRawFacadeSpec {
            hook: "json.json_tree_is_object",
            args: JsonRawFacadeArgs::Tree,
        },
        "json.field" => JsonRawFacadeSpec {
            hook: "json.json_tree_field",
            args: JsonRawFacadeArgs::TreeAndString,
        },
        "json.index" => JsonRawFacadeSpec {
            hook: "json.json_tree_index",
            args: JsonRawFacadeArgs::TreeAndInt64,
        },
        "json.array_length" => JsonRawFacadeSpec {
            hook: "json.json_tree_array_length",
            args: JsonRawFacadeArgs::Tree,
        },
        "json.object_keys" => JsonRawFacadeSpec {
            hook: "json.json_tree_object_keys",
            args: JsonRawFacadeArgs::Tree,
        },
        "json.as_string" => JsonRawFacadeSpec {
            hook: "json.json_tree_as_string",
            args: JsonRawFacadeArgs::Tree,
        },
        "json.as_int64" => JsonRawFacadeSpec {
            hook: "json.json_tree_as_int64",
            args: JsonRawFacadeArgs::Tree,
        },
        "json.as_float64" => JsonRawFacadeSpec {
            hook: "json.json_tree_as_float64",
            args: JsonRawFacadeArgs::Tree,
        },
        "json.as_bool" => JsonRawFacadeSpec {
            hook: "json.json_tree_as_bool",
            args: JsonRawFacadeArgs::Tree,
        },
        _ => return None,
    };
    Some(spec)
}

/// Returns true for the public raw JSON facade functions backed by
/// `stdlib/json.jett`.
pub fn is_json_raw_facade(name: &str) -> bool {
    json_raw_facade_spec(name).is_some()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_facade_specs_pin_trusted_hooks_and_argument_shapes() {
        assert_eq!(
            json_raw_facade_spec("json.parse_raw"),
            Some(JsonRawFacadeSpec {
                hook: "json.json_tree_parse",
                args: JsonRawFacadeArgs::RawString,
            })
        );
        assert_eq!(
            json_raw_facade_spec("json.field"),
            Some(JsonRawFacadeSpec {
                hook: "json.json_tree_field",
                args: JsonRawFacadeArgs::TreeAndString,
            })
        );
        assert_eq!(
            json_raw_facade_spec("json.index"),
            Some(JsonRawFacadeSpec {
                hook: "json.json_tree_index",
                args: JsonRawFacadeArgs::TreeAndInt64,
            })
        );
        assert_eq!(
            json_raw_facade_spec("json.as_bool"),
            Some(JsonRawFacadeSpec {
                hook: "json.json_tree_as_bool",
                args: JsonRawFacadeArgs::Tree,
            })
        );
        assert_eq!(json_raw_facade_spec("json.json_tree_parse"), None);
    }

    #[test]
    fn raw_facade_and_implicit_view_sets_share_policy() {
        assert!(is_json_raw_facade("json.serialize_raw"));
        assert!(is_json_implicit_view_facade("json.serialize_raw"));
        assert!(is_json_implicit_view_facade("json.json_tree_serialize"));
        assert!(!is_json_raw_facade("json.json_tree_serialize"));
        assert!(!is_json_implicit_view_facade("json.parse"));
    }
}
