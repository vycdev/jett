//! Genuine Source regression for the initial actor/callable sealing boundary.
//! This module creates no actor, reflected callable or sealing proof manually.
use crate::{
    CallOwnership, DeclarationKind, Expression, ExpressionKind, Function, HandleKind, LocalId,
    Program, ReflectedCallablePlan, ReflectedFieldAction, ReflectedFieldValidation, StatementKind,
};
use jett_common::{FileId, SourceOrigin};
use jett_resolve::DefKind;
use jett_typecheck::{CheckOptions, CheckedActorHandlerTarget, CheckedResourceProgram};
use jett_types::Type;
use std::{collections::HashMap, sync::Arc};

const SOURCE: &str = include_str!(
    "../../jett_comptime/src/resource_execution/fixtures/actor_handler_direct_getter.jett"
);

fn checked(release: bool) -> Arc<CheckedResourceProgram> {
    let file = FileId::new(0);
    Arc::new(
        CheckedResourceProgram::prepare(
            jett_parser::parse(SOURCE, file),
            HashMap::from([(file, SourceOrigin::Project)]),
            &[],
            CheckOptions { release },
        )
        .unwrap_or_else(|error| {
            panic!(
                "direct-handler Source must check before lowering; release={release}: {error:?}; {:?}",
                error.diagnostics()
            )
        }),
    )
}

fn same_program(left: &Program, right: &Program) -> bool {
    left.resource_source == right.resource_source
        && left.resource_manifest == right.resource_manifest
        && left.equality_methods == right.equality_methods
        && crate::resource_materialization::functions_equal(&left.functions, &right.functions)
}

fn assert_actual_handler_getter_and_adapter(
    program: &Program,
    checked: &Arc<CheckedResourceProgram>,
) -> ReflectedCallablePlan {
    let types = &checked.checked().interner;
    crate::validate(program).unwrap();
    crate::validate_backend_types(program, types).unwrap();
    assert!(program.resource_source.belongs_to(checked));
    program
        .resource_source
        .validate_current(program, types)
        .unwrap();

    let actors = checked
        .resolved()
        .scope_table
        .definitions
        .iter()
        .filter(|definition| definition.kind == DefKind::Actor && definition.name == "app.Worker")
        .collect::<Vec<_>>();
    let [actor] = actors.as_slice() else {
        panic!("one exact original Worker declaration");
    };
    let actor_type = checked.checked().definition_types[&actor.id];
    let Type::Actor(actor_id) = types.resolve(actor_type) else {
        panic!("original Worker must retain its checked actor type");
    };
    let target = CheckedActorHandlerTarget {
        actor_definition: actor.id,
        actor: *actor_id,
        handler_index: 0,
    };
    let handlers = program
        .functions
        .iter()
        .filter(|function| {
            function.identity.declaration.kind == DeclarationKind::ActorHandler
                && function.identity.declaration.name == "Worker.read"
        })
        .collect::<Vec<_>>();
    let [handler] = handlers.as_slice() else {
        panic!("one genuine Worker.read native entry");
    };
    let handler: &Function = handler;
    let entry = handler
        .actor_handler
        .as_ref()
        .expect("original actor entry receipt");
    assert_eq!(entry.target(), target);
    entry.validate(handler).unwrap();
    assert!(
        checked
            .checked()
            .actor_handler(target)
            .unwrap()
            .ownership_validated()
    );
    assert!(entry.roles().is_empty());
    assert!(handler.params.is_empty());
    assert_eq!(handler.capture_count, 0);
    assert!(handler.source_definition.is_none());

    let mut reads = Vec::new();
    let mut body = handler.body.clone();
    crate::resource_materialization::walk_block(&mut body, &mut |expression| {
        if let ExpressionKind::Intrinsic {
            field_validation: Some(ReflectedFieldValidation::Validate(fields)),
            ..
        } = &expression.kind
        {
            for (ordinal, field) in fields.iter().enumerate() {
                if let ReflectedFieldAction::CallableReturn(plan) = &field.action {
                    reads.push((ordinal, expression.clone(), plan.clone()));
                }
            }
        }
        Ok(())
    })
    .unwrap();
    let [(ordinal, getter, plan)] = reads.as_slice() else {
        panic!("the sole Holder field must create one real handler-owned reflected getter");
    };
    assert_eq!(*ordinal, 0);
    assert!(plan.is_sealed());
    assert!(!plan.is_materialized(), "this probe has no comptime bake");
    assert!(plan.validate_original());
    plan.validate_types(types).unwrap();
    plan.validate_current(program, handler, getter, Some(*ordinal))
        .unwrap();
    assert_eq!(plan.original_owner(), Some(handler.id));
    let original_getter = plan.original_getter().expect("actual getter seal");
    assert_eq!(original_getter.span, getter.span);
    assert_eq!(original_getter.ty, getter.ty);
    assert_ne!(plan.source_type(), plan.requested_type());

    let adapter = &program.functions[plan.function().index() as usize];
    assert_eq!(adapter.id, plan.function());
    assert_ne!(adapter.id, handler.id);
    assert_eq!(plan.canonical_adapter().unwrap().id, adapter.id);
    assert!(adapter.source_definition.is_none());
    assert!(adapter.actor_handler.is_none());
    assert_eq!(adapter.capture_count, 1);
    assert_eq!(adapter.params.len(), 1);
    assert_eq!(adapter.params[0].local, LocalId::new(0));
    assert_eq!(adapter.params[0].ty, plan.source_type());
    assert_eq!(adapter.return_type, plan.checked_plan().requested_return());
    let [statement] = adapter.body.statements.as_slice() else {
        panic!("one canonical adapter return");
    };
    let StatementKind::Return(Some(Expression {
        kind:
            ExpressionKind::Handle {
                target,
                kind:
                    HandleKind::ReflectedReturn {
                        plan: returned,
                        callable: Some(stored),
                    },
                ..
            },
        ..
    })) = &statement.kind
    else {
        panic!("actual reflected adapter must retain its sealed return plan");
    };
    assert_eq!(
        stored, plan,
        "getter and adapter must retain the same actual token"
    );
    assert_eq!(returned.as_ref(), plan.return_plan());
    let ExpressionKind::IndirectCall {
        callee,
        args,
        evaluation_order,
        ownership: CallOwnership::Generated(generated),
    } = &target.kind
    else {
        panic!("canonical adapter must invoke its original source descriptor");
    };
    assert_eq!(target.ty, plan.checked_plan().source_return());
    assert!(matches!(callee.kind, ExpressionKind::Local(local) if local == LocalId::new(0)));
    assert!(args.is_empty());
    assert!(evaluation_order.is_empty());
    assert!(matches!(
        &generated.operation,
        crate::GeneratedOperation::ReflectedCallableAdapter { callee, plan: admission }
            if *callee == LocalId::new(0) && admission == plan.checked_plan()
    ));
    plan.clone()
}

#[test]
fn direct_handler_reflected_getter_survives_initial_sealing_and_idempotent_completion() {
    for release in [false, true] {
        let checked = checked(release);
        // Do not bypass the genuine initial lowering or anticipate an error string.
        // The first real failing phase establishes the RED result.
        let mut program =
            crate::lower_checked_ordinary_program(&checked).unwrap_or_else(|errors| {
                panic!(
                    "genuine direct-handler initial lowering failed; release={release}: {errors:?}"
                )
            });
        let plan = assert_actual_handler_getter_and_adapter(&program, &checked);
        let original_source = program.resource_source.functions().to_vec();
        let initial = program.clone();
        let types = &checked.checked().interner;
        for _ in 0..2 {
            crate::complete_value_conversions(&mut program, types).unwrap();
            let current = assert_actual_handler_getter_and_adapter(&program, &checked);
            assert_eq!(
                current, plan,
                "completion must preserve the actual sealed token"
            );
            assert!(
                same_program(&program, &initial),
                "completion must be idempotent"
            );
            assert!(crate::resource_materialization::functions_equal(
                &original_source,
                program.resource_source.functions(),
            ));
        }
    }
}
