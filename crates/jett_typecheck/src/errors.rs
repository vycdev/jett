use jett_common::Span;
use jett_diagnostics::Diagnostic;

// Diagnostic codes E0300–E0399 are reserved for type checking.

/// E0300: Type mismatch — expected one type, got another.
pub fn type_mismatch(expected: &str, got: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        300,
        format!("type mismatch: expected `{expected}`, got `{got}`"),
        span,
    )
}

/// E0301: Binary operator applied to incompatible types.
pub fn binary_op_mismatch(op: &str, lhs: &str, rhs: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        301,
        format!("cannot apply `{op}` to `{lhs}` and `{rhs}`"),
        span,
    )
}

/// E0302: Unary operator applied to incompatible type.
pub fn unary_op_mismatch(op: &str, ty: &str, span: Span) -> Diagnostic {
    Diagnostic::error(302, format!("cannot apply `{op}` to `{ty}`"), span)
}

/// E0303: Wrong number of arguments in function call.
pub fn argument_count_mismatch(
    func_name: &str,
    expected: usize,
    got: usize,
    span: Span,
) -> Diagnostic {
    Diagnostic::error(
        303,
        format!("function `{func_name}` expects {expected} argument(s), but {got} were provided"),
        span,
    )
}

/// E0304: Argument type does not match parameter type.
pub fn argument_type_mismatch(
    param_name: &str,
    expected: &str,
    got: &str,
    span: Span,
) -> Diagnostic {
    Diagnostic::error(
        304,
        format!("argument `{param_name}` expects `{expected}`, got `{got}`"),
        span,
    )
}

/// E0305: Return type does not match function signature.
pub fn return_type_mismatch(expected: &str, got: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        305,
        format!("return type mismatch: expected `{expected}`, got `{got}`"),
        span,
    )
}

/// E0306: Condition expression must be bool.
pub fn condition_not_bool(got: &str, span: Span) -> Diagnostic {
    Diagnostic::error(306, format!("condition must be `bool`, got `{got}`"), span)
}

/// E0307: For-loop iterable must be a list type.
pub fn not_iterable(got: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        307,
        format!("for-loop requires `list[T]`, got `{got}`"),
        span,
    )
}

/// E0308: Handle block requires result or optional type.
pub fn handle_requires_result_or_optional(got: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        308,
        format!("handle block requires `result[T, E]` or `optional[T]`, got `{got}`"),
        span,
    )
}

/// E0309: Unknown type name.
pub fn unknown_type(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(309, format!("unknown type: `{name}`"), span)
}

/// E0310: Unresolved name (no DefId found for identifier).
pub fn unresolved_name(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(310, format!("unresolved name: `{name}`"), span)
}

/// E0311: Variable declaration type mismatch.
pub fn var_decl_type_mismatch(var_name: &str, expected: &str, got: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        311,
        format!("variable `{var_name}` declared as `{expected}`, but initializer has type `{got}`"),
        span,
    )
}

/// E0312: Assignment type mismatch.
pub fn assign_type_mismatch(expected: &str, got: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        312,
        format!("cannot assign `{got}` to target of type `{expected}`"),
        span,
    )
}

/// E0313: Called expression is not a function.
pub fn not_callable(got: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        313,
        format!("expression of type `{got}` is not callable"),
        span,
    )
}

/// E0314: Assert condition must be bool.
pub fn assert_condition_not_bool(got: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        314,
        format!("assert condition must be `bool`, got `{got}`"),
        span,
    )
}

/// E0315: `default` may only appear inside a handle block.
pub fn default_outside_handle(span: Span) -> Diagnostic {
    Diagnostic::error(
        315,
        "`default` may only appear inside a `handle` block".to_string(),
        span,
    )
}

/// E0316: `result[T, E]` requires `handle error:`.
pub fn result_requires_handle_error(span: Span) -> Diagnostic {
    Diagnostic::error(
        316,
        "`result[T, E]` values must use `handle error:`".to_string(),
        span,
    )
}

/// E0317: `optional[T]` requires bare `handle:`.
pub fn optional_requires_bare_handle(span: Span) -> Diagnostic {
    Diagnostic::error(
        317,
        "`optional[T]` values must use bare `handle:`".to_string(),
        span,
    )
}

/// E0318: Handle blocks must terminate explicitly.
pub fn handle_block_requires_return_or_default(span: Span) -> Diagnostic {
    Diagnostic::error(
        318,
        "handle block must end with `return` or `default`".to_string(),
        span,
    )
}

/// E0319: Type has no such field or method.
pub fn type_has_no_member(type_name: &str, member: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        319,
        format!("type `{type_name}` has no field or method `{member}`"),
        span,
    )
}

/// E0320: Struct constructor field was provided more than once.
pub fn duplicate_constructor_field(type_name: &str, field: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        320,
        format!("constructor for `{type_name}` received field `{field}` more than once"),
        span,
    )
}

/// E0321: Struct constructor is missing a required field.
pub fn missing_constructor_field(type_name: &str, field: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        321,
        format!("constructor for `{type_name}` is missing required field `{field}`"),
        span,
    )
}

/// E0322: Match expressions require enum values.
pub fn match_requires_enum(got: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        322,
        format!("match requires an enum value, got `{got}`"),
        span,
    )
}

/// E0323: Variant pattern binding count mismatch.
pub fn variant_binding_count_mismatch(
    variant: &str,
    expected: usize,
    got: usize,
    span: Span,
) -> Diagnostic {
    Diagnostic::error(
        323,
        format!(
            "pattern for variant `{variant}` expects {expected} binding(s), but {got} were provided"
        ),
        span,
    )
}

/// E0324: Match is not exhaustive for the enum's variants.
pub fn non_exhaustive_match(enum_name: &str, missing_variant: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        324,
        format!("match on `{enum_name}` is not exhaustive; missing variant `{missing_variant}`"),
        span,
    )
}

/// E0325: Mutual declaration has no corresponding function body.
pub fn mutual_function_missing_definition(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        325,
        format!("mutual declaration for `{name}` has no matching function definition"),
        span,
    )
}

/// E0326: Function definition does not match its mutual declaration.
pub fn mutual_signature_mismatch(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        326,
        format!("function `{name}` does not match its `mutual` declaration"),
        span,
    )
}

// Diagnostic codes E0500–E0599 are reserved for capability / purity checking.

/// E0327: `implement` target must name an interface.
pub fn expected_interface(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(327, format!("`{name}` is not an interface"), span)
}

/// E0328: Interface implementation provides the same method more than once.
pub fn duplicate_implemented_method(type_name: &str, member: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        328,
        format!(
            "type `{type_name}` implements method `{member}` more than once for this interface"
        ),
        span,
    )
}

/// E0329: Interface does not declare a requested method.
pub fn interface_has_no_member(interface_name: &str, member: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        329,
        format!("interface `{interface_name}` has no method `{member}`"),
        span,
    )
}

/// E0330: Implemented method signature does not match the interface contract.
pub fn implemented_method_signature_mismatch(
    interface_name: &str,
    type_name: &str,
    member: &str,
    span: Span,
) -> Diagnostic {
    Diagnostic::error(
        330,
        format!("method `{type_name}.{member}` does not match interface `{interface_name}`"),
        span,
    )
}

/// E0331: Interface implementation is missing a required method.
pub fn missing_implemented_method(
    interface_name: &str,
    type_name: &str,
    member: &str,
    span: Span,
) -> Diagnostic {
    Diagnostic::error(
        331,
        format!(
            "type `{type_name}` is missing required method `{member}` for interface `{interface_name}`"
        ),
        span,
    )
}

/// E0332: Type is used where an interface implementation is required.
pub fn type_does_not_implement_interface(
    type_name: &str,
    interface_name: &str,
    span: Span,
) -> Diagnostic {
    Diagnostic::error(
        332,
        format!("type `{type_name}` does not implement interface `{interface_name}`"),
        span,
    )
}

/// E0500: Pure function calls impure function.
pub fn pure_calls_impure(caller: &str, callee: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        500,
        format!(
            "pure function `{caller}` cannot call impure function `{callee}`; \
             add the required capability parameters to `{caller}` or remove the call"
        ),
        span,
    )
}

/// E0501: Verify block calls impure function.
pub fn verify_calls_impure(verify_name: &str, callee: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        501,
        format!(
            "verify block `{verify_name}` cannot call impure function `{callee}`; \
             verify blocks may only call pure functions"
        ),
        span,
    )
}

// Diagnostic codes E0600-E0699 are reserved for secret-type checking.

/// E0600: Secret value reaches an output boundary without declassification.
pub fn secret_exposure(boundary: &str, type_name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        600,
        format!(
            "cannot pass `{type_name}` to `{boundary}`; use `declassify` to make the exposure explicit"
        ),
        span,
    )
}

/// E0601: `declassify` requires a secret value.
pub fn declassify_requires_secret(got: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        601,
        format!("`declassify` requires `secret[T]`, got `{got}`"),
        span,
    )
}
