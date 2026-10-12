use super::*;
use jett_common::FileId;

const SOURCE: &str = include_str!("../../fixtures/33_owned_struct_scope_drop_entry.jett");

fn execution(source: &str, name: &str) -> (CheckedExecution, CheckedBodyReference) {
    let program = crate::resource_execution::tests::program(source, false);
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
    let mut found = Vec::new();
    walk_region(
        body.region().unwrap(),
        &mut |_| {},
        &mut |expression| {
            if let Expr::Call(callee, _, _) = expression {
                if matches!(callee.as_ref(), Expr::Ident(name) if name.name == "OwnedEnvelope") {
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
