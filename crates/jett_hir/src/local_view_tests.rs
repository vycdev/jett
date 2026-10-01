use super::*;
use jett_diagnostics::Severity;

fn checked_source(source: &str, tests: bool) -> Result<(Program, CheckResult), Vec<LowerError>> {
    let file = FileId::new(0);
    let parsed = jett_parser::parse(source, file);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let resolved = jett_resolve::resolve(&parsed.module);
    assert!(
        !resolved
            .diagnostics
            .iter()
            .any(|error| error.severity == Severity::Error),
        "{:?}",
        resolved.diagnostics
    );
    let checked = jett_typecheck::check(&parsed.module, &resolved);
    assert!(
        !checked
            .diagnostics
            .iter()
            .any(|error| error.severity == Severity::Error),
        "{:?}",
        checked.diagnostics
    );
    let origins = HashMap::from([(file, SourceOrigin::Project)]);
    let program = if tests {
        lower_with_test_bodies(&parsed.module, &resolved, &checked, &origins)
    } else {
        lower(&parsed.module, &resolved, &checked, &origins)
    }?;
    Ok((program, checked))
}

fn local<'a>(function: &'a Function, name: &str) -> &'a Local {
    function
        .locals
        .iter()
        .find(|local| local.name == name)
        .unwrap()
}

#[test]
fn local_views_preserve_immediate_origins_and_owned_copies() {
    let (program, _) = checked_source(
        r#"namespace app
struct Record:
    values: list[int64]
    number: int64
    text: string
function inspect(view source: Record) returns nothing:
    Record direct = view source
    Record forwarded = direct
    Record wrapped = (view forwarded)
    Record owned = clone wrapped
    list[int64] field_copy = source.values
    list[int64] parent_view_field_copy = (view source).values
    int64 number_copy = view source.number
    string text_copy = view source.text
    trace wrapped
    return nothing
"#,
        false,
    )
    .unwrap();
    let function = &program.functions[0];
    let source = local(function, "source");
    let direct = local(function, "direct");
    let forwarded = local(function, "forwarded");
    assert_eq!(direct.view_source, Some(source.id));
    assert_eq!(forwarded.view_source, Some(direct.id));
    assert_eq!(local(function, "wrapped").view_source, Some(forwarded.id));
    assert_eq!(
        local_view_root(&function.locals, local(function, "wrapped").id),
        Some(source.id)
    );
    assert!(is_borrowed_local(
        &function.locals,
        &function.params,
        source.id
    ));
    for name in [
        "owned",
        "field_copy",
        "parent_view_field_copy",
        "number_copy",
        "text_copy",
    ] {
        assert_eq!(local(function, name).view_source, None, "{name}");
    }
}

#[test]
fn local_views_preserve_secret_wrapper_origins_and_reject_allocating_coercions() {
    let (program, mut checked) = checked_source(
        "namespace app\nfunction inspect(view source: secret[list[int64]]) returns nothing:\n    list[int64] alias = declassify view source\n    trace alias\n    return nothing\n",
        false,
    ).unwrap();
    let function = &program.functions[0];
    let source = local(function, "source").id;
    let alias = local(function, "alias");
    assert_eq!(alias.view_source, Some(source));
    let StatementKind::Let { value, .. } = &function.body.statements[0].kind else {
        panic!("alias initializer");
    };
    validate_local_view_initializer(value, source, alias.ty, &checked.interner).unwrap();
    let changed = checked.interner.intern(Type::List(TypeInterner::STRING));
    let conversion = Expression {
        kind: ExpressionKind::interface_coerce(Box::new(value.clone())),
        ty: changed,
        span: value.span,
    };
    assert!(
        validate_local_view_initializer(&conversion, source, changed, &checked.interner)
            .unwrap_err()
            .contains("allocate")
    );
}

#[test]
fn local_views_keep_same_named_sibling_bindings_distinct() {
    let (program, _) = checked_source(
        r#"namespace app
function inspect() returns nothing:
    if true:
        list[int64] source = list(1)
        list[int64] alias = view source
        trace alias
    else:
        list[int64] source = list(2)
        list[int64] alias = view source
        trace alias
    return nothing
"#,
        false,
    )
    .unwrap();
    let function = &program.functions[0];
    let sources = function
        .locals
        .iter()
        .filter(|local| local.name == "source")
        .collect::<Vec<_>>();
    assert_eq!(sources.len(), 2);
    let aliases = function
        .locals
        .iter()
        .filter(|local| local.name == "alias")
        .collect::<Vec<_>>();
    assert_eq!(aliases.len(), 2);
    assert_eq!(aliases[0].view_source, Some(sources[0].id));
    assert_eq!(aliases[1].view_source, Some(sources[1].id));
    assert_ne!(sources[0].id, sources[1].id);
}

#[test]
fn local_views_transport_generic_inline_and_test_body_facts() {
    let (program, _) = checked_source(r#"namespace app
function inspect[T](view source: list[T]) returns nothing:
    list[T] alias = view source
    trace alias
    return nothing
function main() returns nothing:
    function(view list[int64]) returns nothing callback = function(view source: list[int64]) returns nothing:
        list[int64] inline_alias = view source
        trace inline_alias
        return nothing
    inspect[int64](view list(1))
    callback(view list(2))
verify local_aliases:
    list[int64] source = list(3)
    list[int64] test_alias = view source
    trace test_alias
    assert true
property generated_aliases:
    given number: int64
    list[int64] source = list(number)
    list[int64] test_alias = view source
    trace test_alias
    assert true
"#, true).unwrap();
    let mut contexts = Vec::new();
    for function in &program.functions {
        if let Some(alias) = function.locals.iter().find(|local| {
            ["alias", "inline_alias", "test_alias"].contains(&local.name.as_str())
                && local.view_source.is_some()
                && (function.debug_kind == FunctionDebugKind::Inline
                    || local.name != "inline_alias")
        }) {
            assert!(local_view_root(&function.locals, alias.id).is_some());
            contexts.push(function.identity.declaration.kind);
        }
    }
    assert_eq!(contexts.len(), 4);
    assert!(contexts.contains(&DeclarationKind::Verify));
    assert!(contexts.contains(&DeclarationKind::Property));
}

#[test]
fn local_views_transport_reflected_body_facts() {
    let (program, _) = checked_source(
        r#"namespace app
struct Record:
    number: int64
function inspect[T](view value: T, view source: list[int64]) returns nothing:
    for field in type.fields[T]():
        comptime type Field = field.type_info:
            list[int64] reflected_alias = view source
            trace reflected_alias
    list[int64] after_alias = view source
    trace after_alias
    return nothing
function main() returns nothing:
    inspect[Record](view Record(number: 1), view list(2))
"#,
        false,
    )
    .unwrap();
    let function = program
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "inspect")
        .unwrap();
    let source = local(function, "source").id;
    assert_eq!(local(function, "reflected_alias").view_source, Some(source));
    assert_eq!(local(function, "after_alias").view_source, Some(source));
}

#[test]
fn local_views_reject_unstable_native_origins_without_changing_checking() {
    for body in [
        "mutable list[int64] source = list(1)\n    list[int64] alias = view source",
        "list[int64] alias = view list(1)",
        "Record source = Record(values: list(1))\n    list[int64] alias = view source.values",
        "list[int64] source = list(1)\n    mutable list[int64] alias = view source",
    ] {
        let source = format!(
            "namespace app\nstruct Record:\n    values: list[int64]\nfunction inspect() returns nothing:\n    {body}\n    trace alias\n    return nothing\n"
        );
        let errors =
            checked_source(&source, false).expect_err("unsupported native alias must fail HIR");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("native borrowed alias")),
            "{body}: {errors:?}"
        );
    }
}

#[test]
fn local_views_validate_origins_types_and_conversion_rebuilding() {
    let (mut program, checked) = checked_source("namespace app\nfunction inspect(view source: list[int64]) returns nothing:\n    list[int64] alias = view source\n    trace alias\n    return nothing\n", false).unwrap();
    let original = program.clone();
    program.functions[0].locals[1].view_source = Some(LocalId::new(1));
    assert!(
        validate(&program)
            .unwrap_err()
            .iter()
            .any(|error| error.message.contains("cycle"))
    );
    program = original.clone();
    program.functions[0].locals[1].view_source = Some(LocalId::new(90));
    assert!(validate(&program).is_err());
    program = original.clone();
    program.functions[0].locals[0].mutable = true;
    assert!(validate_local_views(&program, &checked.interner).is_err());
    program = original.clone();
    program.functions[0].locals[1].ty = TypeInterner::INT64;
    assert!(validate_local_views(&program, &checked.interner).is_err());
    program = original;
    let StatementKind::Let { value, .. } = &mut program.functions[0].body.statements[0].kind else {
        panic!("alias initializer");
    };
    let original_value = value.clone();
    value.kind = ExpressionKind::Clone(Box::new(original_value));
    let errors = complete_value_conversions(&mut program, &checked.interner).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("stable backing local"))
    );
}
