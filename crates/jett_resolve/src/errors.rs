use jett_common::Span;
use jett_diagnostics::Diagnostic;

// Diagnostic codes E0200–E0299 are reserved for name resolution.

/// E0200: Reference to an undefined variable, function, or type.
pub fn undefined_name(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(200, format!("undefined name: `{name}`"), span)
}

/// E0201: Variable shadowing is not allowed.
pub fn variable_shadowing(name: &str, new_span: Span, original_span: Span) -> Diagnostic {
    Diagnostic::error(
        201,
        format!("variable `{name}` shadows an existing binding"),
        new_span,
    )
    .with_label(original_span, "original binding defined here")
}

/// E0202: Unused variable warning.
pub fn unused_variable(name: &str, span: Span) -> Diagnostic {
    Diagnostic::warning(202, format!("unused variable: `{name}`"), span)
}

/// E0203: Unused import warning.
pub fn unused_import(name: &str, span: Span) -> Diagnostic {
    Diagnostic::warning(203, format!("unused import: `{name}`"), span)
}

/// E0204: Duplicate definition in the same scope.
pub fn duplicate_definition(name: &str, new_span: Span, original_span: Span) -> Diagnostic {
    Diagnostic::error(204, format!("duplicate definition: `{name}`"), new_span)
        .with_label(original_span, "previously defined here")
}

/// E0205: Forward reference — using a name before it is defined.
pub fn forward_reference(name: &str, use_span: Span, def_span: Span) -> Diagnostic {
    Diagnostic::error(
        205,
        format!("`{name}` is referenced before its definition"),
        use_span,
    )
    .with_label(def_span, "defined here (must appear before use)")
}

/// E0206: `use` declaration must appear at the top of a function body.
pub fn use_not_at_top(span: Span) -> Diagnostic {
    Diagnostic::error(
        206,
        "`use` declarations must appear at the top of the function body",
        span,
    )
}

/// E0207: Private namespaced declaration used outside its namespace.
pub fn private_definition(
    name: &str,
    namespace: &str,
    use_span: Span,
    def_span: Span,
) -> Diagnostic {
    Diagnostic::error(
        207,
        format!("`{name}` is private to namespace `{namespace}`"),
        use_span,
    )
    .with_label(def_span, "private declaration defined here")
}
