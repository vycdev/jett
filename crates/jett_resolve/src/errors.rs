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

/// E0208: Namespaced declaration used without a namespace qualifier.
pub fn namespace_qualifier_required(
    name: &str,
    namespace: &str,
    qualified_name: &str,
    use_span: Span,
    def_span: Span,
) -> Diagnostic {
    Diagnostic::error(
        208,
        format!("`{name}` belongs to namespace `{namespace}`; use `{qualified_name}`"),
        use_span,
    )
    .with_label(def_span, "namespaced declaration defined here")
}

/// E0209: Root type aliases are not supported.
pub fn invalid_root_export(message: impl Into<String>, span: Span) -> Diagnostic {
    Diagnostic::error(209, message, span)
}

/// E0210: External project/dependency namespace used without a local import.
pub fn namespace_import_required(
    name: &str,
    namespace: &str,
    use_span: Span,
    def_span: Span,
) -> Diagnostic {
    Diagnostic::error(
        210,
        format!(
            "namespace `{namespace}` must be imported before using `{name}`; add `use {namespace}` at the top of this block"
        ),
        use_span,
    )
    .with_label(def_span, "external declaration defined here")
}

/// E0211: Global constant initializer depends on another project namespace.
pub fn global_namespace_dependency(
    name: &str,
    namespace: &str,
    use_span: Span,
    def_span: Span,
) -> Diagnostic {
    Diagnostic::error(
        211,
        format!(
            "global constant initializer cannot use `{name}` from project namespace `{namespace}`; project constants cannot depend on another project or vendored namespace"
        ),
        use_span,
    )
    .with_label(def_span, "external declaration defined here")
}

/// E0212: User-defined type names must use the canonical PascalCase form.
pub fn type_name_must_be_pascal_case(name: &str, suggested_name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        212,
        format!(
            "user-defined type name `{name}` must use PascalCase; rename it to `{suggested_name}`"
        ),
        span,
    )
}

/// E0213: Opaque runtime resources are compiler-shipped declarations.
pub fn resource_declaration_requires_stdlib(span: Span) -> Diagnostic {
    Diagnostic::error(
        213,
        "resource declarations are reserved for compiler-shipped standard library files",
        span,
    )
}
