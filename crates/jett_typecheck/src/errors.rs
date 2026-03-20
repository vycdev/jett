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
    Diagnostic::error(
        302,
        format!("cannot apply `{op}` to `{ty}`"),
        span,
    )
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
        format!(
            "function `{func_name}` expects {expected} argument(s), but {got} were provided"
        ),
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
    Diagnostic::error(
        306,
        format!("condition must be `bool`, got `{got}`"),
        span,
    )
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
    Diagnostic::error(
        309,
        format!("unknown type: `{name}`"),
        span,
    )
}

/// E0310: Unresolved name (no DefId found for identifier).
pub fn unresolved_name(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        310,
        format!("unresolved name: `{name}`"),
        span,
    )
}

/// E0311: Variable declaration type mismatch.
pub fn var_decl_type_mismatch(
    var_name: &str,
    expected: &str,
    got: &str,
    span: Span,
) -> Diagnostic {
    Diagnostic::error(
        311,
        format!(
            "variable `{var_name}` declared as `{expected}`, but initializer has type `{got}`"
        ),
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
