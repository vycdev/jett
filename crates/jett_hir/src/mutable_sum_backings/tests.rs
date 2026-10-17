use super::*;
use jett_diagnostics::Severity;

const SOURCE: &str = r#"namespace app
function borrowed() returns list[int64]:
    mutable optional[list[int64]] source = some(list(2, 3))
    list[int64] alias = view((view source) handle: return list())
    return clone alias
function copied() returns string:
    mutable result[string, string] source = ok("kept")
    string alias = view((view source) handle error: return error)
    source = ok("new")
    return alias
"#;

fn checked(
    source: &str,
) -> (
    jett_parser::ParseResult,
    jett_resolve::ResolveResult,
    CheckResult,
) {
    let parsed = jett_parser::parse(source, FileId::new(0));
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let resolved = jett_resolve::resolve(&parsed.module);
    let checked = jett_typecheck::check(&parsed.module, &resolved);
    assert!(
        resolved
            .diagnostics
            .iter()
            .chain(&checked.diagnostics)
            .all(|error| error.severity != Severity::Error),
        "{:?} {:?}",
        resolved.diagnostics,
        checked.diagnostics
    );
    (parsed, resolved, checked)
}
fn high(source: &str) -> (Program, CheckResult) {
    let (parsed, resolved, checked) = checked(source);
    let high = lower(
        &parsed.module,
        &resolved,
        &checked,
        &HashMap::from([(FileId::new(0), SourceOrigin::Project)]),
    )
    .unwrap();
    (high, checked)
}

#[test]
fn canonical_copy_and_view_declarations_both_retain_exact_owned_backing() {
    let (high, checked) = high(SOURCE);
    validate(&high).unwrap();
    validate_backend_types(&high, &checked.interner).unwrap();
    assert_eq!(high.functions.len(), 2);
    for function in &high.functions {
        assert_eq!(function.mutable_sum_backings.len(), 1);
        let proof = &function.mutable_sum_backings[0];
        assert!(proof.backing_fact.mutable);
        assert_eq!(proof.backing_fact.mode, CheckedBindingMode::Owned);
        let alias = &function.locals[proof.alias.index() as usize];
        assert_eq!(
            alias.view_source.is_none(),
            function.identity.declaration.name == "copied"
        );
        assert_eq!(
            function
                .mutable_sum_backing(proof.alias, &proof.initializer, proof.backing)
                .unwrap(),
            Some(proof)
        );
    }
}

#[test]
fn copied_destination_cannot_bypass_selected_owner_fact_authentication() {
    for invalid in ["missing", "mode", "mutable", "type", "definition"] {
        let (parsed, resolved, mut checked) = checked(SOURCE);
        let key = *checked
            .binding_facts
            .iter()
            .find(|(_, fact)| {
                fact.mutable && matches!(checked.interner.resolve(fact.ty), Type::Result(_, _))
            })
            .unwrap()
            .0;
        let other = checked
            .binding_facts
            .values()
            .find(|fact| fact.declaration_span != key)
            .unwrap()
            .definition;
        if invalid == "missing" {
            checked.binding_facts.remove(&key);
        } else {
            let fact = checked.binding_facts.get_mut(&key).unwrap();
            match invalid {
                "mode" => {
                    fact.mode = CheckedBindingMode::View {
                        source: CheckedViewSource::Other,
                    }
                }
                "mutable" => fact.mutable = false,
                "type" => fact.ty = TypeInterner::STRING,
                "definition" => fact.definition = other,
                _ => unreachable!(),
            }
        }
        let errors = lower(
            &parsed.module,
            &resolved,
            &checked,
            &HashMap::from([(FileId::new(0), SourceOrigin::Project)]),
        )
        .unwrap_err();
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("selected owned backing")),
            "{invalid}: {errors:?}"
        );
    }
}

#[test]
fn receipts_refuse_foreign_function_changed_initializer_and_alias_modes() {
    let (original, checked) = high(SOURCE);
    for invalid in ["foreign", "owner", "alias", "body", "duplicate", "reseal"] {
        let mut changed = original.clone();
        let proof = changed.functions[0].mutable_sum_backings[0].clone();
        match invalid {
            "foreign" => changed.functions[1].mutable_sum_backings = vec![proof],
            "owner" => changed.functions[0].params.push(Param {
                local: proof.backing,
                name: "foreign".into(),
                ty: proof.backing_fact.ty,
                mode: ParamMode::View,
                mutable: true,
                span: proof.backing_fact.declaration_span,
            }),
            "alias" => changed.functions[0].locals[proof.alias.index() as usize].mutable = true,
            "body" | "reseal" => changed.functions[0].body.statements.reverse(),
            "duplicate" => changed.functions[0].mutable_sum_backings.push(proof),
            _ => unreachable!(),
        }
        assert!(validate(&changed).is_err(), "{invalid}");
        if invalid == "reseal" {
            assert!(seal_program(&mut changed, &checked.interner).is_err());
        }
    }
}

#[test]
fn canonical_conversion_refreshes_only_the_authenticated_original_body() {
    let (mut high, checked) = high(SOURCE);
    interface_values::complete_value_conversions(&mut high, &checked.interner).unwrap();
    validate(&high).unwrap();
    let mut changed = high.clone();
    changed.functions[0].body.statements.reverse();
    assert!(interface_values::complete_value_conversions(&mut changed, &checked.interner).is_err());
}

#[test]
fn first_sum_snapshot_follows_original_reflected_callable_sealing() {
    let source = r#"namespace app
type Positive = int64 where value > 0
struct Holder:
    callback: function() returns int64
function inspect(view holder: Holder, view field: TypeField) returns nothing:
    mutable optional[int64] source = some(1)
    int64 copied = view((view source) handle: return nothing)
    function() returns Positive callback = type.field_value[Holder, function() returns Positive](view holder, view field)
    return nothing
"#;
    let parsed = jett_parser::parse(source, FileId::new(0));
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let checked = Arc::new(
        jett_typecheck::CheckedResourceProgram::prepare(
            parsed,
            HashMap::from([(FileId::new(0), SourceOrigin::Project)]),
            &[],
            jett_typecheck::CheckOptions { release: false },
        )
        .unwrap(),
    );
    let mut high = lower_checked_ordinary_program(&checked).unwrap();
    let owner = high
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "inspect")
        .unwrap();
    assert_eq!(owner.mutable_sum_backings.len(), 1);
    let plans = reflected_callable_plans(owner);
    assert_eq!(plans.len(), 1);
    assert_eq!(
        reflected_callable_plans(owner.mutable_sum_backings[0].original.as_ref().unwrap()),
        plans,
        "sum snapshot pins the genuine sealed callable tokens",
    );
    validate(&high).unwrap();
    validate_backend_types(&high, &checked.checked().interner).unwrap();
    for _ in 0..2 {
        interface_values::complete_value_conversions(&mut high, &checked.checked().interner)
            .unwrap();
        validate(&high).unwrap();
        validate_backend_types(&high, &checked.checked().interner).unwrap();
        let owner = high
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "inspect")
            .unwrap();
        assert_eq!(reflected_callable_plans(owner), plans);
    }
    let before = high.clone();
    let mut changed = before.clone();
    changed
        .functions
        .iter_mut()
        .find(|function| function.identity.declaration.name == "inspect")
        .unwrap()
        .body
        .statements
        .reverse();
    assert!(callable_sealing(&mut changed, &before, &[]).is_err());
}
