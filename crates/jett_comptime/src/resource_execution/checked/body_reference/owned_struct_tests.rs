use super::*;
use jett_common::FileId;

const SOURCE: &str = include_str!("../../fixtures/33_owned_struct_scope_drop_entry.jett");

fn execution(source: &str, name: &str) -> (CheckedExecution, CheckedBodyReference) {
    let program = crate::resource_execution::tests::program(source, false);
    execution_program(program, name)
}

fn execution_program(
    program: Arc<jett_typecheck::CheckedResourceProgram>,
    name: &str,
) -> (CheckedExecution, CheckedBodyReference) {
    let mut checked =
        CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
    let function = program
        .module()
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function)
                if function.name.span.file == FileId::new(0) && function.name.name == name =>
            {
                Some(function)
            }
            _ => None,
        })
        .unwrap();
    let definition = checked.declaration_definition(function.name.span).unwrap();
    let body = checked
        .prepare_function_body(&checked.entry(definition).unwrap())
        .unwrap();
    checked.install_function_body(&body).unwrap();
    (checked, body)
}

fn constructor(body: &CheckedBodyReference) -> &Expr {
    constructor_named(body, "OwnedEnvelope")
}

fn constructor_named<'a>(body: &'a CheckedBodyReference, selected: &str) -> &'a Expr {
    let mut found = Vec::new();
    walk_region(
        body.region().unwrap(),
        &mut |_| {},
        &mut |expression| {
            if let Expr::Call(callee, _, _) = expression {
                if matches!(callee.as_ref(), Expr::Ident(name) if name.name == selected) {
                    found.push(expression);
                }
            }
        },
        &mut |_| {},
    );
    assert_eq!(found.len(), 1);
    found[0]
}

#[test]
fn occupied_struct_primitive_companion_proofs_keep_exact_types_aliases_and_original_order() {
    let sources = [
        (
            include_str!("../../fixtures/36_owned_struct_primitive_companions.jett"),
            "retire_primitives",
            "PrimitiveEnvelope",
        ),
        (
            include_str!("../../fixtures/37_owned_struct_alias_companions.jett"),
            "retire_aliases",
            "AliasEnvelope",
        ),
        (
            include_str!("../../fixtures/38_owned_struct_pending_int64_transport.jett"),
            "retire_pending",
            "PendingEnvelope",
        ),
        (
            include_str!("../../fixtures/40_owned_struct_pending_companions.jett"),
            "retire_pending_primitives",
            "PendingPrimitives",
        ),
        (
            include_str!("../../fixtures/42_owned_struct_pending_primitive_matrix.jett"),
            "retire_pending_matrix",
            "PendingMatrix",
        ),
    ];
    for (source, helper, name) in sources {
        let (checked, body) = execution_program(
            crate::resource_execution::tests::companion_program(source, false),
            helper,
        );
        let expression = constructor_named(&body, name);
        let proof = checked.prepare_owned_struct(expression).unwrap().unwrap();
        checked.revalidate_owned_struct(&proof, expression).unwrap();
        assert!(checked.prepare_owned_struct(&expression.clone()).is_err());
        assert!(proof.fields().iter().all(|(_, ty)| matches!(
            checked.program.checked().interner.resolve(*ty),
            Type::Resource(_)
        ) || PreparedOwnedStruct::supports_companion(
            checked.program.checked().interner.resolve(*ty)
        )));
        if name == "PrimitiveEnvelope" {
            assert_eq!(&proof.source_fields()[..3], &[8, 9, 0]);
            assert_eq!(proof.fields().len(), 14);
        }
        if name == "AliasEnvelope" {
            assert_eq!(proof.fields()[1].1, jett_types::TypeInterner::INT8);
            assert_eq!(proof.fields()[2].1, jett_types::TypeInterner::UINT64);
            assert_eq!(proof.fields()[3].1, jett_types::TypeInterner::FLOAT32);
        }
        let mut redirected = proof;
        redirected.fields[1].1 = jett_types::TypeInterner::STRING;
        assert!(
            checked
                .revalidate_owned_struct(&redirected, expression)
                .is_err()
        );
    }
}

#[test]
fn owned_struct_original_constructor_has_exact_type_and_source_field_mapping() {
    let (checked, body) = execution(SOURCE, "retire_owned");
    let expression = constructor(&body);
    let proof = checked.prepare_owned_struct(expression).unwrap().unwrap();
    assert_eq!(proof.source_fields(), &[0, 1]);
    assert_eq!(proof.fields()[0].0, "token");
    assert_eq!(
        proof.fields()[1],
        ("marker".into(), jett_types::TypeInterner::INT64)
    );
    checked.revalidate_owned_struct(&proof, expression).unwrap();
}

#[test]
fn owned_struct_constructor_copied_foreign_wrong_body_and_redirected_proofs_are_refused() {
    let (checked, body) = execution(SOURCE, "retire_owned");
    let expression = constructor(&body);
    let proof = checked.prepare_owned_struct(expression).unwrap().unwrap();
    assert!(checked.prepare_owned_struct(&expression.clone()).is_err());
    let (foreign, foreign_body) = execution(SOURCE, "retire_owned");
    assert!(
        checked
            .revalidate_owned_struct(&proof, constructor(&foreign_body))
            .is_err()
    );
    assert!(
        foreign
            .revalidate_owned_struct(&proof, constructor(&foreign_body))
            .is_err()
    );
    let (other, _) = execution(SOURCE, "main");
    assert!(other.revalidate_owned_struct(&proof, expression).is_err());
    for mutation in 0..4 {
        let mut forged = checked.prepare_owned_struct(expression).unwrap().unwrap();
        match mutation {
            0 => forged.source_fields.swap(0, 1),
            1 => forged.ty = jett_types::TypeInterner::INT64,
            2 => forged.fields[0].1 = jett_types::TypeInterner::INT64,
            3 => forged.expression += 1,
            _ => unreachable!(),
        }
        assert!(
            checked
                .revalidate_owned_struct(&forged, expression)
                .is_err()
        );
    }
}

#[test]
fn occupied_struct_same_layout_declaration_cannot_substitute_for_selected_constructor() {
    let source = SOURCE.replace(
        "function retire_owned(",
        "struct OtherEnvelope:\n    token: resource_probe.TestHandle\n    marker: int64\nfunction retire_owned(",
    );
    let (checked, body) = execution(&source, "retire_owned");
    let expression = constructor(&body);
    let mut proof = checked.prepare_owned_struct(expression).unwrap().unwrap();
    let other = checked
        .program
        .resolved()
        .scope_table
        .definitions
        .iter()
        .find(|info| info.kind == jett_resolve::DefKind::Struct && info.name == "app.OtherEnvelope")
        .unwrap();
    assert_ne!(other.id, proof.definition);
    proof.definition = other.id;
    assert!(checked.revalidate_owned_struct(&proof, expression).is_err());
}
