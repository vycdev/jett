use super::*;
use crate::resource_execution::{ProviderEvent, ScriptOperation, tests::program};
use jett_common::FileId;
use jett_parser::ast::{Item, Stmt};
use jett_types::ResourceHookKind;

const SOURCE: &str = include_str!("../fixtures/33_owned_struct_scope_drop_entry.jett");

fn setup() -> (
    ResourceTransport,
    crate::resource_execution::CheckedBodyReference,
) {
    let program = program(SOURCE, false);
    let function = program
        .module()
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function)
                if function.name.span.file == FileId::new(0)
                    && function.name.name == "retire_owned" =>
            {
                Some(function)
            }
            _ => None,
        })
        .unwrap();
    let mut runtime =
        ResourceTransport::checked_only(program.clone(), ExecutionPurpose::ReferenceRuntime, 1)
            .unwrap();
    runtime.checked_source_active = true;
    let target = runtime
        .checked
        .declaration_definition(function.name.span)
        .unwrap();
    let body = runtime
        .checked
        .prepare_function_body(&runtime.checked.entry(target).unwrap())
        .unwrap();
    runtime.checked.install_function_body(&body).unwrap();
    runtime
        .install_script(vec![ScriptOperation::Construct {
            label: 1001,
            outcome: Ok(()),
        }])
        .unwrap();
    (runtime, body)
}

fn constructed(runtime: &mut ResourceTransport) -> EvaluatedValue {
    let definition = runtime
        .checked
        .program()
        .checked()
        .resource_hooks
        .values()
        .find(|hook| hook.kind == ResourceHookKind::Construct)
        .unwrap()
        .definition;
    let span = *runtime
        .checked
        .program()
        .checked()
        .call_ownership
        .iter()
        .find(|(_, packet)| packet.target == CheckedInvocationTarget::Resolved(definition))
        .unwrap()
        .0;
    let invocation = runtime.checked.invocation(span).unwrap();
    let descriptor = runtime.checked.descriptor(definition).unwrap();
    let grant = match &runtime.provider {
        InstalledResourceProvider::Scripted(provider) => provider.grant(),
        _ => panic!("private scripted provider"),
    };
    let mut value = runtime
        .invoke_hook(
            &invocation,
            &descriptor,
            vec![
                EvaluatedValue::ordinary(Value::GrantedNetwork(grant)),
                EvaluatedValue::ordinary(Value::Int64(1001)),
            ],
        )
        .unwrap();
    let Value::ResultOk(payload) = value.value else {
        panic!("actual constructed owner")
    };
    value.value = *payload;
    value.remove_prefix(PayloadStep::Ok).unwrap();
    value
}

fn declaration(
    body: &crate::resource_execution::CheckedBodyReference,
) -> &jett_parser::ast::VarDecl {
    let Stmt::VarDecl(declaration) = &body.function().unwrap().body.stmts[0] else {
        panic!("exact occupied struct binding")
    };
    declaration
}

#[test]
fn occupied_struct_root_paths_migrate_through_operation_and_binding_without_old_finalization() {
    let (mut runtime, body) = setup();
    let declaration = declaration(&body);
    let expression = &declaration.value;
    let proof = runtime
        .checked
        .prepare_owned_struct(expression)
        .unwrap()
        .unwrap();
    let operation = runtime.begin_operation().unwrap();
    let token = constructed(&mut runtime);
    let mut aggregate = runtime
        .assemble_owned_struct(
            &proof,
            expression,
            vec![token, EvaluatedValue::ordinary(Value::Int64(17))],
        )
        .unwrap();
    assert_eq!(runtime.live_owners(), 1);
    let physical_copy = EvaluatedValue::ordinary(aggregate.value.clone());
    assert!(matches!(
        runtime.validate_value_type(&physical_copy, proof.ty()),
        Err(ResourceExecutionError::InvalidPayloadPath)
    ));
    runtime
        .end_operation(operation, Some(&mut aggregate))
        .unwrap();
    assert_eq!(
        runtime.provider_events().unwrap(),
        [ProviderEvent::Constructed(1001)]
    );
    runtime
        .install_binding(declaration.name.span, aggregate)
        .unwrap();
    let binding = runtime.checked.binding(declaration.name.span).unwrap();
    let viewed = runtime.binding(binding.definition, true).unwrap();
    runtime.validate_value_type(&viewed, proof.ty()).unwrap();
    assert!(viewed.custody.has_borrows());
    assert!(!viewed.custody.has_owners());
    runtime.unwind_all().unwrap();
    assert_eq!(
        (runtime.live_owners(), runtime.registry_live_count()),
        (0, 0)
    );
    assert_eq!(
        runtime.provider_events().unwrap(),
        [
            ProviderEvent::Constructed(1001),
            ProviderEvent::Finalized(1001)
        ]
    );
}

#[test]
fn occupied_struct_raw_or_borrowed_members_cannot_mint_owner_paths() {
    for raw_copy in [false, true] {
        let (mut runtime, body) = setup();
        let expression = &declaration(&body).value;
        let proof = runtime
            .checked
            .prepare_owned_struct(expression)
            .unwrap()
            .unwrap();
        let operation = runtime.begin_operation().unwrap();
        let owner = constructed(&mut runtime);
        let bad = if raw_copy {
            EvaluatedValue::ordinary(owner.value.clone())
        } else {
            runtime.borrow(&owner).unwrap()
        };
        assert!(
            runtime
                .assemble_owned_struct(
                    &proof,
                    expression,
                    vec![bad, EvaluatedValue::ordinary(Value::Int64(17)),]
                )
                .is_err()
        );
        assert_eq!(runtime.live_owners(), 1);
        assert_eq!(
            runtime.provider_events().unwrap(),
            [ProviderEvent::Constructed(1001)]
        );
        runtime.end_operation(operation, None).unwrap();
        assert_eq!(
            (runtime.live_owners(), runtime.registry_live_count()),
            (0, 0)
        );
        runtime.unwind_all().unwrap();
    }
}

#[test]
fn occupied_struct_wrong_scalar_and_redirected_physical_field_fail_before_publication() {
    let (mut runtime, body) = setup();
    let expression = &declaration(&body).value;
    let proof = runtime
        .checked
        .prepare_owned_struct(expression)
        .unwrap()
        .unwrap();
    let operation = runtime.begin_operation().unwrap();
    let owner = constructed(&mut runtime);
    assert!(
        runtime
            .assemble_owned_struct(
                &proof,
                expression,
                vec![
                    owner,
                    EvaluatedValue::ordinary(Value::String("wrong scalar".into())),
                ]
            )
            .is_err()
    );
    assert_eq!(runtime.live_owners(), 1);
    runtime.end_operation(operation, None).unwrap();
    runtime.unwind_all().unwrap();

    let (mut runtime, body) = setup();
    let expression = &declaration(&body).value;
    let proof = runtime
        .checked
        .prepare_owned_struct(expression)
        .unwrap()
        .unwrap();
    let operation = runtime.begin_operation().unwrap();
    let owner = constructed(&mut runtime);
    let mut aggregate = runtime
        .assemble_owned_struct(
            &proof,
            expression,
            vec![owner, EvaluatedValue::ordinary(Value::Int64(17))],
        )
        .unwrap();
    let Value::Struct { fields, .. } = &mut aggregate.value else {
        panic!("occupied struct")
    };
    fields.swap(0, 1);
    assert!(runtime.validate_value_type(&aggregate, proof.ty()).is_err());
    runtime.end_operation(operation, None).unwrap();
    runtime.unwind_all().unwrap();
    assert_eq!(
        (runtime.live_owners(), runtime.registry_live_count()),
        (0, 0)
    );
}
