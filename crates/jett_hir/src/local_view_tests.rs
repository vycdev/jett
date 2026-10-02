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
    let source_type = function.locals[source.index() as usize].ty;
    validate_local_view_initializer(value, source, source_type, alias.ty, &checked.interner)
        .unwrap();
    let changed = checked.interner.intern(Type::List(TypeInterner::STRING));
    let conversion = Expression {
        kind: ExpressionKind::interface_coerce(Box::new(value.clone())),
        ty: changed,
        span: value.span,
    };
    assert!(
        validate_local_view_initializer(
            &conversion,
            source,
            source_type,
            changed,
            &checked.interner
        )
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
fn projected_local_views_keep_typed_endpoints_and_immediate_owner_dependencies() {
    let (program, checked) = checked_source(
        r#"namespace app
struct Packet:
    data: bytes
    items: list[int64]
    hidden: secret[list[int64]]
    number: int64
struct Envelope:
    packet: Packet
function inspect(view source: Envelope) returns nothing:
    Packet projected_record = view source.packet
    bytes projected_bytes = view source.packet.data
    list[int64] projected_items = view projected_record.items
    list[int64] forwarded = projected_items
    list[int64] revealed = declassify view source.packet.hidden
    list[int64] copied = source.packet.items
    list[int64] cloned = clone forwarded
    int64 scalar = view source.packet.number
    trace forwarded
    return nothing
"#,
        false,
    )
    .unwrap();
    validate_backend_types(&program, &checked.interner).unwrap();
    let function = &program.functions[0];
    let source = local(function, "source");
    for name in ["projected_record", "projected_bytes", "revealed"] {
        let alias = local(function, name);
        assert_eq!(alias.view_source, Some(source.id), "{name}");
        assert_ne!(
            alias.ty, source.ty,
            "field endpoint is distinct from its owner"
        );
        assert_eq!(local_view_root(&function.locals, alias.id), Some(source.id));
    }
    assert_eq!(
        local(function, "projected_items").view_source,
        Some(local(function, "projected_record").id)
    );
    assert_eq!(
        local(function, "forwarded").view_source,
        Some(local(function, "projected_items").id)
    );
    for name in ["copied", "cloned", "scalar"] {
        assert_eq!(local(function, name).view_source, None, "{name}");
    }
}

#[test]
fn projected_local_views_transport_generic_and_reflected_binding_contexts() {
    let (program, checked) = checked_source(
        r#"namespace app
struct Holder[T]:
    value: T
struct Metadata:
    number: int64
function inspect[T](view source: Holder[T]) returns nothing:
    T alias = view source.value
    trace alias
    return nothing
function reflected[T](view metadata: T, view source: Holder[list[int64]]) returns nothing:
    for field in type.fields[T]():
        comptime type Field = field.type_info:
            list[int64] reflected_alias = view source.value
            trace reflected_alias
    list[int64] after_alias = view source.value
    trace after_alias
    return nothing
function main() returns nothing:
    inspect[int64](view Holder[int64](value: 7))
    inspect[list[int64]](view Holder[list[int64]](value: list(2, 3)))
    reflected[Metadata](view Metadata(number: 1), view Holder[list[int64]](value: list(5)))
"#,
        false,
    )
    .unwrap();
    validate_backend_types(&program, &checked.interner).unwrap();
    let inspect = program
        .functions
        .iter()
        .filter(|function| function.identity.declaration.name == "inspect")
        .collect::<Vec<_>>();
    assert_eq!(inspect.len(), 2);
    for function in inspect {
        let alias = local(function, "alias");
        if alias.ty == TypeInterner::INT64 {
            assert_eq!(alias.view_source, None);
        } else {
            assert_eq!(alias.view_source, Some(local(function, "source").id));
        }
    }
    let reflected = program
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "reflected")
        .unwrap();
    for name in ["reflected_alias", "after_alias"] {
        assert_eq!(
            local(reflected, name).view_source,
            Some(local(reflected, "source").id)
        );
    }
}

#[test]
fn projected_local_views_require_initializer_proofs_even_when_unused() {
    let (program, checked) = checked_source(
        "namespace app\nstruct Packet:\n    data: bytes\nfunction inspect(view source: Packet) returns nothing:\n    bytes unused_alias = view source.data\n    return nothing\n",
        false,
    ).unwrap();
    validate_backend_types(&program, &checked.interner).unwrap();
    let mut orphan = program.clone();
    orphan.functions[0].body.statements.remove(0);
    assert!(
        validate_backend_types(&orphan, &checked.interner)
            .unwrap_err()
            .iter()
            .any(|error| error.message.contains("no validated initializer"))
    );
    inline_functions::extract_inline_functions(&mut orphan.functions, &checked.interner);
    assert!(
        complete_value_conversions(&mut orphan, &checked.interner)
            .unwrap_err()
            .iter()
            .any(|error| error.message.contains("no validated initializer"))
    );
    let mut forged = program;
    let StatementKind::Let { value, .. } = &mut forged.functions[0].body.statements[0].kind else {
        panic!("unused projected initializer");
    };
    let ExpressionKind::View(field) = &mut value.kind else {
        panic!("explicit projected view");
    };
    let ExpressionKind::Field { field, .. } = &mut field.kind else {
        panic!("typed projection");
    };
    *field = FieldId(u32::MAX);
    assert!(
        validate_backend_types(&forged, &checked.interner)
            .unwrap_err()
            .iter()
            .any(|error| error.message.contains("invalid field index"))
    );
}

#[test]
fn projected_local_views_keep_unused_inline_bindings_and_clear_copied_origins() {
    let (program, checked) = checked_source(
        r#"namespace app
struct Packet:
    data: bytes
function inspect(view source: Packet) returns nothing:
    bytes outer_alias = view source.data
    function(view Packet) returns nothing callback = function(view packet: Packet) returns nothing:
        bytes inner_alias = view packet.data
        return nothing
    bytes later_alias = view source.data
    callback(view source)
    return nothing
"#,
        false,
    )
    .unwrap();
    validate_backend_types(&program, &checked.interner).unwrap();
    let outer = program
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "inspect")
        .unwrap();
    assert!(local(outer, "outer_alias").view_source.is_some());
    assert!(local(outer, "later_alias").view_source.is_some());
    assert!(local(outer, "inner_alias").view_source.is_none());
    let inline = program
        .functions
        .iter()
        .find(|function| function.debug_kind == FunctionDebugKind::Inline)
        .unwrap();
    assert!(local(inline, "inner_alias").view_source.is_some());
    assert!(local(inline, "outer_alias").view_source.is_none());
    assert!(local(inline, "later_alias").view_source.is_none());
}

#[test]
fn local_views_reject_unstable_native_origins_without_changing_checking() {
    for body in [
        "mutable list[int64] source = list(1)\n    list[int64] alias = view source",
        "list[int64] alias = view list(1)",
        "mutable Record source = Record(values: list(1))\n    list[int64] alias = view source.values",
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
fn projected_local_views_preserve_existing_refinement_ancestors_and_secret_promotions() {
    let (program, checked) = checked_source(
        r#"namespace app
type Values = list[int64] where true
type NarrowValues = Values where true
struct Packet:
    plain: list[int64]
    refined: NarrowValues
    hidden: secret[NarrowValues]
function inspect(view source: Packet, view values: list[int64]) returns nothing:
    NarrowValues direct = view source.refined
    Values ancestor = coarsen view source.refined
    list[int64] base = coarsen view source.refined
    NarrowValues revealed = declassify view source.hidden
    secret[list[int64]] promoted_field = view source.plain
    secret[list[int64]] promoted_local = view values
    return nothing
"#,
        false,
    )
    .unwrap();
    validate_backend_types(&program, &checked.interner).unwrap();
    let function = program
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "inspect")
        .unwrap();
    let source = local(function, "source");
    for name in ["direct", "ancestor", "base", "revealed", "promoted_field"] {
        assert_eq!(local(function, name).view_source, Some(source.id), "{name}");
    }
    assert_eq!(
        local(function, "promoted_local").view_source,
        Some(local(function, "values").id)
    );
    let direct = local(function, "direct").ty;
    let ancestor = local(function, "ancestor").ty;
    let base = local(function, "base").ty;
    assert!(
        matches!(checked.interner.resolve(direct), Type::Refinement { base: inner, .. } if *inner == ancestor)
    );
    assert!(
        matches!(checked.interner.resolve(ancestor), Type::Refinement { base: inner, .. } if *inner == base)
    );
    assert_eq!(local(function, "revealed").ty, direct);
    for name in ["promoted_field", "promoted_local"] {
        assert!(
            matches!(checked.interner.resolve(local(function, name).ty), Type::Secret(inner) if *inner == base),
            "{name}"
        );
    }
}

#[test]
fn unused_projected_local_views_cannot_invent_or_discard_nominal_refinement_proofs() {
    let source = r#"namespace app
type Proven = list[int64] where true
type Rejected = list[int64] where false
struct Packet:
    items: Proven
function inspect(view source: Packet) returns nothing:
    Proven borrowed = view source.items
    return nothing
"#;
    for invalid in [
        "target",
        "discard target",
        "view",
        "conversion",
        "discard conversion",
        "coarsen",
        "declassify",
    ] {
        let (mut program, checked) = checked_source(source, false).unwrap();
        let rejected = checked.interner.type_ids().find(|ty| {
            matches!(checked.interner.resolve(*ty), Type::Refinement { name, .. } if name == "app.Rejected")
        }).expect("declared Rejected refinement is interned");
        let function = program
            .functions
            .iter_mut()
            .find(|function| function.identity.declaration.name == "inspect")
            .unwrap();
        let borrowed = local(function, "borrowed").id;
        let proven = function.locals[borrowed.index() as usize].ty;
        let Type::Refinement { base, .. } = checked.interner.resolve(proven) else {
            panic!("declared predicate-bearing endpoint");
        };
        let target = if invalid.starts_with("discard") {
            *base
        } else {
            rejected
        };
        function.locals[borrowed.index() as usize].ty = target;
        let StatementKind::Let { value, .. } = &mut function.body.statements[0].kind else {
            panic!("unused projected alias still has an initializer");
        };
        if !matches!(invalid, "target" | "discard target") {
            let inner = Box::new(value.clone());
            value.kind = match invalid {
                "view" => ExpressionKind::View(inner),
                "conversion" | "discard conversion" => ExpressionKind::interface_coerce(inner),
                "coarsen" => ExpressionKind::Coarsen(inner),
                "declassify" => ExpressionKind::Declassify(inner),
                _ => unreachable!(),
            };
            value.ty = target;
        }
        // The owning Packet, field index/type and source identity remain valid.
        // Only the alleged nominal predicate proof was changed.
        validate(&program).unwrap();
        let errors = validate_backend_types(&program, &checked.interner).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("native borrowed alias")),
            "{invalid}: {errors:?}"
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

#[test]
fn bitfield_local_views_compose_with_struct_fields_and_keep_owned_controls() {
    let (program, checked) = checked_source(
        r#"namespace app
bitfield network Packet:
    version: 4 bits
    flags: 4 bits
    payload: list[uint8]
struct Envelope:
    packet: Packet
function inspect(view source: Packet, view envelope: Envelope) returns nothing:
    list[uint8] direct = view source.payload
    list[uint8] forwarded = direct
    Packet projected_packet = view envelope.packet
    list[uint8] projected_payload = view projected_packet.payload
    list[uint8] mixed = view envelope.packet.payload
    list[uint8] copied = source.payload
    list[uint8] cloned = clone forwarded
    int64 scalar = view source.version
    return nothing
function owned() returns nothing:
    uint8 first = 7
    uint8 second = 255
    Packet packet = Packet(version: 4, flags: 0, payload: list(first, second))
    list[uint8] borrowed = view packet.payload
    list[uint8] forwarded = borrowed
    list[uint8] independent = clone forwarded
    return nothing
"#,
        false,
    )
    .unwrap();
    validate_backend_types(&program, &checked.interner).unwrap();
    let inspect = program
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "inspect")
        .unwrap();
    let source = local(inspect, "source");
    let envelope = local(inspect, "envelope");
    assert!(matches!(
        checked.interner.resolve(source.ty),
        Type::Bitfield(_)
    ));
    assert_eq!(local(inspect, "direct").view_source, Some(source.id));
    assert_eq!(
        local(inspect, "forwarded").view_source,
        Some(local(inspect, "direct").id)
    );
    for name in ["projected_packet", "mixed"] {
        assert_eq!(
            local(inspect, name).view_source,
            Some(envelope.id),
            "{name}"
        );
    }
    assert_eq!(local(inspect, "projected_packet").ty, source.ty);
    assert_eq!(
        local(inspect, "projected_payload").view_source,
        Some(local(inspect, "projected_packet").id)
    );
    assert_eq!(
        local_view_root(&inspect.locals, local(inspect, "projected_payload").id),
        Some(envelope.id)
    );
    for name in ["copied", "cloned", "scalar"] {
        assert_eq!(local(inspect, name).view_source, None, "{name}");
    }
    let owned = program
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "owned")
        .unwrap();
    assert_eq!(
        local(owned, "borrowed").view_source,
        Some(local(owned, "packet").id)
    );
    assert_eq!(
        local(owned, "forwarded").view_source,
        Some(local(owned, "borrowed").id)
    );
    assert_eq!(local(owned, "independent").view_source, None);
}

#[test]
fn unused_bitfield_local_views_require_exact_field_and_root_proofs() {
    let (program, mut checked) = checked_source(
        r#"namespace app
bitfield network Packet:
    version: 4 bits
    flags: 4 bits
    payload: list[uint8]
bitfield network Other:
    version: 4 bits
    flags: 4 bits
    payload: list[uint8]
function inspect(view source: Packet, view other: Packet, view foreign: Other) returns nothing:
    list[uint8] unused_alias = view source.payload
    return nothing
"#,
        false,
    )
    .unwrap();
    validate_backend_types(&program, &checked.interner).unwrap();
    let function = program
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "inspect")
        .unwrap();
    let source = local(function, "source").id;
    let other = local(function, "other").id;
    let foreign_type = local(function, "foreign").ty;
    let alias = local(function, "unused_alias").id;
    let different_items = checked.interner.intern(Type::List(TypeInterner::STRING));
    for (invalid, expected) in [
        ("field index", "invalid field index"),
        ("numeric field", "invalid field endpoint type"),
        ("field endpoint", "invalid field endpoint type"),
        ("field owner", "invalid field owner"),
        ("source identity", "stable backing local"),
        ("source type", "stable backing local"),
        ("foreign bitfield", "stable backing local"),
        ("orphan", "no validated initializer"),
    ] {
        let mut forged = program.clone();
        let function = forged
            .functions
            .iter_mut()
            .find(|function| function.identity.declaration.name == "inspect")
            .unwrap();
        if invalid == "orphan" {
            function.body.statements.remove(0);
        } else {
            if invalid == "source type" {
                function.locals[source.index() as usize].ty = foreign_type;
            }
            if invalid == "field endpoint" {
                function.locals[alias.index() as usize].ty = different_items;
            }
            let StatementKind::Let { value, .. } = &mut function.body.statements[0].kind else {
                panic!("unused bitfield alias initializer");
            };
            if invalid == "field endpoint" {
                value.ty = different_items;
            }
            let ExpressionKind::View(projection) = &mut value.kind else {
                panic!("explicit field view");
            };
            if invalid == "field endpoint" {
                projection.ty = different_items;
            }
            let ExpressionKind::Field {
                base,
                owner_type,
                field,
            } = &mut projection.kind
            else {
                panic!("checked bitfield field projection");
            };
            if invalid == "field owner" {
                *owner_type = foreign_type;
            }
            if invalid == "foreign bitfield" {
                *owner_type = foreign_type;
                base.ty = foreign_type;
            }
            if invalid == "field index" {
                *field = FieldId(u32::MAX);
            }
            if invalid == "numeric field" {
                *field = FieldId(0);
            }
            if invalid == "source identity" {
                base.kind = ExpressionKind::Local(other);
            }
        }
        let errors = validate_backend_types(&forged, &checked.interner).unwrap_err();
        assert!(
            errors.iter().any(|error| error.message.contains(expected)),
            "{invalid}: {errors:?}"
        );
    }
}

#[test]
fn qualified_machine_local_views_compose_with_struct_fields_and_exact_qualifications() {
    let (program, checked) = checked_source(
        r#"namespace app
type Values = list[int64] where true
type NarrowValues = Values where true
struct Payload:
    items: list[int64]
    refined: NarrowValues
machine Session:
    states:
        ready(payload: Payload, hidden: secret[NarrowValues], number: int64)
        empty
struct Envelope:
    session: Session at ready
function inspect(view source: Session at ready, view envelope: Envelope) returns nothing:
    Payload projected_record = view source.payload
    list[int64] machine_struct = view source.payload.items
    list[int64] record_items = view projected_record.items
    list[int64] forwarded = record_items
    Session at ready projected_machine = view envelope.session
    list[int64] struct_machine = view projected_machine.payload.items
    list[int64] mixed = view envelope.session.payload.items
    NarrowValues direct = view source.payload.refined
    Values ancestor = coarsen view source.payload.refined
    NarrowValues revealed = declassify view source.hidden
    secret[list[int64]] promoted = view source.payload.items
    list[int64] copied = source.payload.items
    list[int64] cloned = clone forwarded
    int64 scalar = view source.number
    return nothing
"#,
        false,
    )
    .unwrap();
    validate_backend_types(&program, &checked.interner).unwrap();
    let function = program
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "inspect")
        .unwrap();
    let source = local(function, "source");
    let envelope = local(function, "envelope");
    for name in [
        "projected_record",
        "machine_struct",
        "direct",
        "ancestor",
        "revealed",
        "promoted",
    ] {
        assert_eq!(local(function, name).view_source, Some(source.id), "{name}");
        assert_eq!(
            local_view_root(&function.locals, local(function, name).id),
            Some(source.id)
        );
    }
    for name in ["projected_machine", "mixed"] {
        assert_eq!(
            local(function, name).view_source,
            Some(envelope.id),
            "{name}"
        );
    }
    assert_eq!(
        local(function, "record_items").view_source,
        Some(local(function, "projected_record").id)
    );
    assert_eq!(
        local(function, "forwarded").view_source,
        Some(local(function, "record_items").id)
    );
    assert_eq!(
        local(function, "struct_machine").view_source,
        Some(local(function, "projected_machine").id)
    );
    assert!(matches!(
        checked.interner.resolve(source.ty),
        Type::MachineState { .. }
    ));
    assert_eq!(local(function, "projected_machine").ty, source.ty);
    assert_ne!(local(function, "machine_struct").ty, source.ty);
    assert_eq!(local(function, "direct").ty, local(function, "revealed").ty);
    for name in ["copied", "cloned", "scalar"] {
        assert_eq!(local(function, name).view_source, None, "{name}");
    }
}

#[test]
fn qualified_machine_local_views_transport_generic_binding_contexts() {
    let (program, checked) = checked_source(
        r#"namespace app
struct Payload:
    items: list[int64]
machine Session:
    states:
        ready(payload: Payload)
        empty
struct Envelope[T]:
    session: Session at ready
    extra: T
function inspect[T](view source: Envelope[T]) returns nothing:
    list[int64] alias = view source.session.payload.items
    T generic_alias = view source.extra
    list[int64] forwarded = alias
    trace forwarded
    return nothing
function main() returns nothing:
    inspect[int64](view Envelope[int64](session: Session(ready, Payload(items: list(1))), extra: 7))
    inspect[list[int64]](view Envelope[list[int64]](session: Session(ready, Payload(items: list(2))), extra: list(3)))
"#,
        false,
    )
    .unwrap();
    validate_backend_types(&program, &checked.interner).unwrap();
    let inspect = program
        .functions
        .iter()
        .filter(|function| function.identity.declaration.name == "inspect")
        .collect::<Vec<_>>();
    assert_eq!(inspect.len(), 2);
    for function in inspect {
        let source = local(function, "source");
        let alias = local(function, "alias");
        assert_eq!(alias.view_source, Some(source.id));
        assert_eq!(local(function, "forwarded").view_source, Some(alias.id));
        let generic_alias = local(function, "generic_alias");
        if generic_alias.ty == TypeInterner::INT64 {
            assert_eq!(generic_alias.view_source, None);
        } else {
            assert_eq!(generic_alias.ty, alias.ty);
            assert_eq!(generic_alias.view_source, Some(source.id));
        }
    }
}

#[test]
fn unused_qualified_machine_local_views_require_exact_state_field_and_root_proofs() {
    let (program, mut checked) = checked_source(
        r#"namespace app
machine Session:
    states:
        ready(items: list[int64])
        cached(items: list[string])
        empty
machine Other:
    states:
        ready(items: list[int64])
function inspect(view source: Session at ready, view other: Session at ready, view foreign: Other at ready) returns nothing:
    list[int64] unused_alias = view source.items
    return nothing
"#,
        false,
    )
    .unwrap();
    validate_backend_types(&program, &checked.interner).unwrap();
    let function = program
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "inspect")
        .unwrap();
    let source = local(function, "source").id;
    let other = local(function, "other").id;
    let foreign_type = local(function, "foreign").ty;
    let alias = local(function, "unused_alias").id;
    let source_type = local(function, "source").ty;
    let Type::MachineState { machine, .. } = *checked.interner.resolve(source_type) else {
        panic!("statically qualified root");
    };
    let cached = checked
        .interner
        .resolve_machine(machine)
        .state_id("cached")
        .unwrap();
    let empty = checked
        .interner
        .resolve_machine(machine)
        .state_id("empty")
        .unwrap();
    let cached_type = checked.interner.intern(Type::MachineState {
        machine,
        state: cached,
    });
    let empty_type = checked.interner.intern(Type::MachineState {
        machine,
        state: empty,
    });
    let stale_type = checked.interner.intern(Type::MachineState {
        machine,
        state: jett_types::MachineStateId::new(u32::MAX),
    });
    let bare_type = checked.interner.intern(Type::Machine(machine));
    let different_items = checked.interner.intern(Type::List(TypeInterner::STRING));
    for (invalid, expected) in [
        ("stale state", "invalid machine state"),
        ("different state", "invalid field endpoint type"),
        ("empty state", "invalid field index"),
        ("field index", "invalid field index"),
        ("field endpoint", "invalid field endpoint type"),
        ("field owner", "invalid field owner"),
        ("source identity", "stable backing local"),
        ("source type", "stable backing local"),
        ("bare owner", "state-qualified machine fields"),
        ("foreign machine", "stable backing local"),
        ("orphan", "no validated initializer"),
    ] {
        let mut forged = program.clone();
        let function = forged
            .functions
            .iter_mut()
            .find(|function| function.identity.declaration.name == "inspect")
            .unwrap();
        if invalid == "orphan" {
            function.body.statements.remove(0);
        } else {
            if invalid == "source type" {
                function.locals[source.index() as usize].ty = bare_type;
            }
            if invalid == "field endpoint" {
                function.locals[alias.index() as usize].ty = different_items;
            }
            let StatementKind::Let { value, .. } = &mut function.body.statements[0].kind else {
                panic!("unused machine field alias initializer");
            };
            if invalid == "field endpoint" {
                value.ty = different_items;
            }
            let ExpressionKind::View(projection) = &mut value.kind else {
                panic!("explicit field view");
            };
            if invalid == "field endpoint" {
                projection.ty = different_items;
            }
            let ExpressionKind::Field {
                base,
                owner_type,
                field,
            } = &mut projection.kind
            else {
                panic!("checked machine field projection");
            };
            let changed_owner = match invalid {
                "stale state" => Some(stale_type),
                "different state" => Some(cached_type),
                "empty state" => Some(empty_type),
                "bare owner" => Some(bare_type),
                "foreign machine" => Some(foreign_type),
                _ => None,
            };
            if let Some(changed_owner) = changed_owner {
                *owner_type = changed_owner;
                base.ty = changed_owner;
            }
            if invalid == "field owner" {
                *owner_type = cached_type;
            }
            if invalid == "field index" {
                *field = FieldId(u32::MAX);
            }
            if invalid == "source identity" {
                base.kind = ExpressionKind::Local(other);
            }
        }
        let errors = validate_backend_types(&forged, &checked.interner).unwrap_err();
        assert!(
            errors.iter().any(|error| error.message.contains(expected)),
            "{invalid}: {errors:?}"
        );
    }
}
