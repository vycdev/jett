use super::*;
use crate::resource_execution::{ProviderEvent, ScriptOperation, tests::program};
use jett_types::ResourceHookKind;

fn runtime(script: Vec<ScriptOperation>) -> ResourceTransport {
    let checked = program(
        include_str!("../fixtures/04_bare_to_view_operation_cleanup.jett"),
        false,
    );
    let mut runtime =
        ResourceTransport::checked_only(checked, ExecutionPurpose::ReferenceRuntime, 1).unwrap();
    runtime.checked_source_active = true;
    runtime.install_script(script).unwrap();
    runtime
}

fn construct(label: i64) -> ScriptOperation {
    ScriptOperation::Construct {
        label,
        outcome: Ok(()),
    }
}

fn constructed(runtime: &mut ResourceTransport, label: i64) -> EvaluatedValue {
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
        _ => panic!("selected private provider"),
    };
    let mut value = runtime
        .invoke_hook(
            &invocation,
            &descriptor,
            vec![
                EvaluatedValue::ordinary(Value::GrantedNetwork(grant)),
                EvaluatedValue::ordinary(Value::Int64(label)),
            ],
        )
        .unwrap();
    let Value::ResultOk(payload) = value.value else {
        panic!("occupied real construction");
    };
    value.value = *payload;
    value.remove_prefix(PayloadStep::Ok).unwrap();
    value
}

#[test]
fn return_prefix_retires_interleaved_operations_and_retains_scope_owner_and_loan() {
    let mut runtime = runtime(vec![construct(903), construct(901), construct(902)]);
    let retained = constructed(&mut runtime, 903);
    let retained_borrow = runtime.borrow(&retained).unwrap();
    let scope = runtime.scope_frame().unwrap();
    runtime
        .ledger
        .hold_borrows(&retained_borrow.custody, scope)
        .unwrap();
    let caller = runtime.begin_operation().unwrap();
    runtime.push_scope();
    runtime.enter_return(caller.frame);
    let first = runtime.begin_operation().unwrap();
    let _first_owner = constructed(&mut runtime, 901);
    runtime.push_scope();
    let second = runtime.begin_operation().unwrap();
    let _second_owner = constructed(&mut runtime, 902);
    runtime.push_scope();

    runtime.retire_return_operations().unwrap();
    assert_eq!(runtime.operations, [caller.frame]);
    assert_eq!(runtime.scopes.len(), 4);
    assert!(runtime.ledger.frame_is_live(scope));
    assert!(runtime.ledger.has_live_borrows());
    assert!(
        runtime
            .ledger
            .borrowed_carrier(&retained_borrow.custody)
            .is_ok()
    );
    assert_eq!(
        (runtime.live_owners(), runtime.registry_live_count()),
        (1, 1)
    );
    assert_eq!(
        runtime.provider_events().unwrap(),
        [
            ProviderEvent::Constructed(903),
            ProviderEvent::Constructed(901),
            ProviderEvent::Constructed(902),
            ProviderEvent::Finalized(902),
            ProviderEvent::Finalized(901),
        ]
    );
    assert!(runtime.acknowledge_return_operation(&second).unwrap());
    assert!(runtime.acknowledge_return_operation(&first).unwrap());
    runtime.leave_return();
    runtime.unwind_all().unwrap();
    assert_eq!(
        (runtime.live_owners(), runtime.registry_live_count()),
        (0, 0)
    );
    assert_eq!(
        runtime.provider_events().unwrap().last(),
        Some(&ProviderEvent::Finalized(903))
    );
}

#[test]
fn return_prefix_nested_callee_boundary_preserves_callers_pending_actuals() {
    let mut runtime = runtime(vec![construct(901), construct(902)]);
    let caller = runtime.begin_operation().unwrap();
    let _caller_owner = constructed(&mut runtime, 901);
    runtime.enter_return(caller.frame);
    let pending = runtime.begin_operation().unwrap();
    let _pending_owner = constructed(&mut runtime, 902);
    runtime.push_scope();
    runtime.enter_return(pending.frame);

    runtime.retire_return_operations().unwrap();
    assert_eq!(runtime.operations, [caller.frame, pending.frame]);
    assert_eq!(runtime.live_owners(), 2);
    assert_eq!(
        runtime.provider_events().unwrap(),
        [
            ProviderEvent::Constructed(901),
            ProviderEvent::Constructed(902),
        ]
    );
    runtime.leave_return();
    runtime.pop_scope();
    runtime.end_operation(pending, None).unwrap();
    runtime.retire_return_operations().unwrap();
    assert_eq!(runtime.operations, [caller.frame]);
    assert_eq!(runtime.live_owners(), 1);
    runtime.leave_return();
    runtime.end_operation(caller, None).unwrap();
    runtime.unwind_all().unwrap();
    assert_eq!(
        (runtime.live_owners(), runtime.registry_live_count()),
        (0, 0)
    );
}

#[test]
fn return_prefix_receipt_rejects_wrong_parent_output_adoption_and_reuse() {
    let mut runtime = runtime(vec![construct(901)]);
    let scope = runtime.scope_frame().unwrap();
    let caller = runtime.begin_operation().unwrap();
    runtime.enter_return(caller.frame);
    let pending = runtime.begin_operation().unwrap();
    let _owner = constructed(&mut runtime, 901);
    runtime.retire_return_operations().unwrap();

    let wrong = OperationFrame {
        frame: pending.frame,
        parent: scope,
    };
    assert_eq!(
        runtime.acknowledge_return_operation(&wrong),
        Err(ResourceExecutionError::InvalidFrame)
    );
    let copied = OperationFrame {
        frame: pending.frame,
        parent: pending.parent,
    };
    let mut output = EvaluatedValue::ordinary(Value::Nothing);
    assert_eq!(
        runtime.end_operation(copied, Some(&mut output)),
        Err(ResourceExecutionError::InvalidFrame)
    );
    assert!(runtime.acknowledge_return_operation(&pending).unwrap());
    assert!(!runtime.acknowledge_return_operation(&pending).unwrap());
    assert_eq!(
        runtime.end_operation(pending, None),
        Err(ResourceExecutionError::InvalidFrame)
    );
    runtime.leave_return();
    runtime.end_operation(caller, None).unwrap();
    runtime.unwind_all().unwrap();
    assert_eq!(
        (runtime.live_owners(), runtime.registry_live_count()),
        (0, 0)
    );
}

#[test]
fn return_prefix_cleanup_panic_continues_suffix_and_expires_receipts_after_teardown() {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    let mut runtime = runtime(vec![
        construct(903),
        construct(901),
        ScriptOperation::ConstructFinalizerPanic { label: 902 },
    ]);
    let _retained = constructed(&mut runtime, 903);
    let caller = runtime.begin_operation().unwrap();
    runtime.enter_return(caller.frame);
    let _first = runtime.begin_operation().unwrap();
    let _first_owner = constructed(&mut runtime, 901);
    runtime.push_scope();
    let _second = runtime.begin_operation().unwrap();
    let _second_owner = constructed(&mut runtime, 902);
    runtime.push_scope();

    let panic = catch_unwind(AssertUnwindSafe(|| runtime.retire_return_operations()))
        .expect_err("actual prefix cleanup panic");
    assert_eq!(
        panic
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| panic.downcast_ref::<&str>().copied()),
        Some("selected test resource cleanup panic")
    );
    assert_eq!(runtime.operations, [caller.frame]);
    assert_eq!(runtime.abandoned_operations.len(), 2);
    assert_eq!(runtime.live_owners(), 1);
    assert_eq!(
        runtime.provider_events().unwrap(),
        [
            ProviderEvent::Constructed(903),
            ProviderEvent::Constructed(901),
            ProviderEvent::Constructed(902),
            ProviderEvent::Finalized(902),
            ProviderEvent::Finalized(901),
        ]
    );
    runtime.unwind_all().unwrap();
    assert_eq!(
        (runtime.live_owners(), runtime.registry_live_count()),
        (0, 0)
    );
    assert_eq!(runtime.abandoned_operations.len(), 2);
    runtime.restore_entry_scopes(1).unwrap();
    runtime.validate_entry_scopes(1).unwrap();
    assert!(runtime.abandoned_operations.is_empty());
    assert_eq!(
        runtime.provider_events().unwrap().last(),
        Some(&ProviderEvent::Finalized(903))
    );
}

#[test]
fn return_prefix_ordinary_callable_has_own_floor_and_no_resource_destination() {
    let mut runtime = runtime(vec![construct(901), construct(902)]);
    let mut retained = constructed(&mut runtime, 901);
    let caller = runtime.begin_operation().unwrap();
    runtime.enter_return(caller.frame);
    let invocation = runtime.begin_operation().unwrap();
    let _invocation_owner = constructed(&mut runtime, 902);
    runtime.push_scope();
    assert!(runtime.enter_ordinary_return());

    runtime.retire_return_operations().unwrap();
    assert_eq!(runtime.operations, [caller.frame, invocation.frame]);
    assert_eq!(runtime.live_owners(), 2);
    assert_eq!(runtime.returns.last().unwrap().destination, None);
    assert_eq!(runtime.returns.last().unwrap().operation_floor, 2);
    assert_eq!(
        runtime.preserve_return(&mut retained),
        Err(ResourceExecutionError::InvalidFrame)
    );
    assert!(runtime.borrow(&retained).is_ok());
    assert!(runtime.abandoned_operations.is_empty());
    assert_eq!(
        runtime.provider_events().unwrap(),
        [
            ProviderEvent::Constructed(901),
            ProviderEvent::Constructed(902)
        ]
    );

    runtime.leave_return();
    runtime.pop_scope();
    runtime.end_operation(invocation, None).unwrap();
    runtime.retire_return_operations().unwrap();
    assert_eq!(runtime.operations, [caller.frame]);
    runtime.leave_return();
    runtime.end_operation(caller, None).unwrap();
    runtime.unwind_all().unwrap();
    assert_eq!(
        (runtime.live_owners(), runtime.registry_live_count()),
        (0, 0)
    );
    assert_eq!(
        runtime.provider_events().unwrap(),
        [
            ProviderEvent::Constructed(901),
            ProviderEvent::Constructed(902),
            ProviderEvent::Finalized(902),
            ProviderEvent::Finalized(901)
        ]
    );
}

#[test]
fn return_prefix_checked_source_named_ordinary_callable_retains_independent_owner() {
    const SOURCE: &str = "namespace app\nfunction read_value() returns int64:\n    return 7\nexport function scenario(view net: Network) returns int64:\n    use resource_probe\n    resource_probe.TestHandle token = resource_probe.create(view net, 1002) handle error:\n        return -1\n    function() returns int64 callback = read_value\n    int64 observed = callback()\n    resource_probe.close(token)\n    return observed\n";
    for release in [false, true] {
        let checked = program(SOURCE, release);
        let declaration = checked
            .module()
            .items
            .iter()
            .find_map(|item| match item {
                jett_parser::ast::Item::Function(function) if function.name.name == "scenario" => {
                    Some(function.name.span)
                }
                _ => None,
            })
            .unwrap();
        let definitions = checked
            .resolved()
            .scope_table
            .definitions
            .iter()
            .filter(|definition| {
                definition.kind == jett_resolve::DefKind::Function
                    && definition.span == declaration
                    && definition.namespace.as_deref() == Some("app")
            })
            .map(|definition| definition.id)
            .collect::<Vec<_>>();
        let [target] = definitions.as_slice() else {
            panic!("unique checked scenario")
        };
        let mut interpreter = crate::Interpreter::from_checked_resource_program(
            checked,
            ExecutionPurpose::ReferenceRuntime,
        )
        .unwrap();
        let grant = interpreter
            .install_resource_test_script(vec![construct(1002)])
            .unwrap();
        assert_eq!(
            interpreter
                .call_checked_program_entry(*target, vec![grant])
                .unwrap(),
            Value::Int64(7)
        );
        assert_eq!(
            interpreter.resource_test_observations().unwrap(),
            (
                vec![
                    ProviderEvent::Constructed(1002),
                    ProviderEvent::Finalized(1002)
                ],
                0,
                0
            )
        );
        assert!(interpreter.take_debug_events().is_empty());
    }
}
