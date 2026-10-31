use super::*;

const SOURCE: &str = "namespace app\ntype Positive = int64 where value > 0\nfunction five() returns int64:\n    return 5\nfunction zero() returns int64:\n    return 0\nstruct Holder:\n    first: function() returns int64\n    second: function() returns int64\nfunction inspect(view holder: Holder, view field: TypeField) returns function() returns Positive:\n    return type.field_value[Holder, function() returns Positive](view holder, view field)\n";

fn checked(source: &str) -> Arc<jett_typecheck::CheckedResourceProgram> {
    let file = FileId::new(0);
    let parsed = jett_parser::parse(source, file);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    Arc::new(
        jett_typecheck::CheckedResourceProgram::prepare(
            parsed,
            HashMap::from([(file, SourceOrigin::Project)]),
            &[],
            jett_typecheck::CheckOptions { release: false },
        )
        .unwrap(),
    )
}

fn fixture() -> (Program, Arc<jett_typecheck::CheckedResourceProgram>) {
    let checked = checked(SOURCE);
    let hir =
        lower_checked_ordinary_program(&checked).expect("ordinary checked reflected lowering");
    (hir, checked)
}

#[test]
fn ordinary_checked_constants_and_raw_refinements_have_no_reflected_source_archive() {
    let checked = checked(
        "namespace app\ntype Positive = int64 where value > 0\nfunction positive() returns Positive:\n    return 5\nPositive cached = comptime positive()\nfunction main() returns nothing:\n    int64 ordinary = comptime 7\n    println(coarsen cached + ordinary)\n    return nothing\n",
    );
    let program = lower_checked_ordinary_program(&checked).expect("ordinary checked constants");
    assert!(!has_reflected_callable_plans(&program));
    assert!(program.resource_source.manifest().is_none());
    assert!(program.resource_source.functions().is_empty());
    assert!(program.resource_manifest.kinds().is_empty());
    assert!(program.resource_manifest.hooks().next().is_none());
    let unsealed = lower(
        checked.module(),
        checked.resolved(),
        checked.checked(),
        checked.source_origins(),
    )
    .expect("existing ordinary lowering");
    assert!(super::super::resource_materialization::functions_equal(
        &program.functions,
        &unsealed.functions,
    ));
}

#[test]
fn checked_read_is_lazy_and_each_ordinal_owns_an_original_adapter() {
    let (program, checked) = fixture();
    assert!(program.resource_source.belongs_to(&checked));
    assert!(program.resource_manifest.kinds().is_empty());
    assert!(program.resource_manifest.hooks().next().is_none());
    validate(&program).unwrap();
    validate_backend_types(&program, &checked.checked().interner).unwrap();
    let owner = program
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "inspect")
        .unwrap();
    let plans = reflected_callable_plans(owner);
    assert_eq!(plans.len(), 2);
    assert_ne!(plans[0].function(), plans[1].function());
    for plan in plans {
        assert!(plan.validate_original());
        assert_eq!(plan.original_owner(), Some(owner.id));
        let adapter = &program.functions[plan.function().index() as usize];
        assert_eq!(adapter.capture_count, 1);
        assert_eq!(adapter.params[0].ty, plan.source_type());
        let StatementKind::Return(Some(Expression {
            kind:
                ExpressionKind::Handle {
                    target,
                    kind:
                        HandleKind::ReflectedReturn {
                            callable: Some(root),
                            ..
                        },
                    ..
                },
            ..
        })) = &adapter.body.statements[0].kind
        else {
            panic!("canonical return root")
        };
        assert_eq!(root, &plan);
        let ExpressionKind::IndirectCall {
            callee,
            args,
            evaluation_order,
            ownership: CallOwnership::Generated(generated),
        } = &target.kind
        else {
            panic!("one original source invocation")
        };
        assert_eq!(target.ty, plan.checked_plan().source_return());
        assert!(matches!(callee.kind, ExpressionKind::Local(LocalId(0))));
        assert!(args.is_empty() && evaluation_order.is_empty());
        assert!(
            matches!(&generated.operation,GeneratedOperation::ReflectedCallableAdapter {plan:admission,..} if admission == plan.checked_plan())
        );
    }
    let mut owner = owner.clone();
    let mut read_calls = 0;
    super::super::resource_materialization::walk_block(&mut owner.body, &mut |value| {
        if matches!(
            value.kind,
            ExpressionKind::Intrinsic {
                field_validation: Some(ReflectedFieldValidation::Validate(_)),
                ..
            }
        ) {
            read_calls += 1;
        }
        assert!(!matches!(
            value.kind,
            ExpressionKind::Handle {
                kind: HandleKind::ReflectedReturn { .. },
                ..
            }
        ));
        Ok(())
    })
    .unwrap();
    assert_eq!(
        read_calls, 1,
        "read constructs descriptors; invocation owns validation"
    );
}

#[test]
fn same_signature_source_body_and_ordinal_and_root_substitution_are_rejected() {
    let (original, checked) = fixture();
    for mutation in 0..5 {
        let mut program = original.clone();
        let owner_index = program
            .functions
            .iter()
            .position(|function| function.identity.declaration.name == "inspect")
            .unwrap();
        let plans = reflected_callable_plans(&program.functions[owner_index]);
        match mutation {
            0 => {
                let function = program
                    .functions
                    .iter_mut()
                    .find(|function| function.identity.declaration.name == "five")
                    .unwrap();
                let StatementKind::Return(Some(value)) = &mut function.body.statements[0].kind
                else {
                    panic!("source return")
                };
                value.kind = ExpressionKind::Int(0);
            }
            1 => {
                super::super::resource_materialization::walk_block(
                    &mut program.functions[owner_index].body,
                    &mut |value| {
                        if let ExpressionKind::Intrinsic {
                            field_validation: Some(ReflectedFieldValidation::Validate(plans)),
                            ..
                        } = &mut value.kind
                        {
                            plans.swap(0, 1);
                        }
                        Ok(())
                    },
                )
                .unwrap();
            }
            2 => {
                let adapter = &mut program.functions[plans[0].function().index() as usize];
                let StatementKind::Return(Some(Expression {
                    kind:
                        ExpressionKind::Handle {
                            kind: HandleKind::ReflectedReturn { callable, .. },
                            ..
                        },
                    ..
                })) = &mut adapter.body.statements[0].kind
                else {
                    panic!("return root")
                };
                *callable = Some(plans[1].clone());
            }
            3 => {
                let predicate = plans[0].original_predicates()[0].id;
                let StatementKind::Return(Some(value)) = &mut program.functions
                    [predicate.index() as usize]
                    .body
                    .statements[0]
                    .kind
                else {
                    panic!("predicate return")
                };
                value.kind = ExpressionKind::Bool(true);
            }
            _ => {
                program.functions[owner_index]
                    .identity
                    .scoped_type_bindings
                    .clear();
                program.functions[owner_index]
                    .identity
                    .declaration
                    .namespace = "other".into();
            }
        }
        assert!(validate(&program).is_err(), "forgery {mutation}");
        assert!(
            complete_value_conversions(&mut program, &checked.checked().interner).is_err(),
            "forgery cannot reseal {mutation}"
        );
    }
}

#[test]
fn checked_plan_rejects_view_parameter_and_carrier_strengthening() {
    let checked = checked(
        "namespace app\ntype Positive = int64 where value > 0\ntype Text = string where value != \"\"\nstruct Holder:\n    callback: function(int64) returns int64\n    validated: function(int64) returns Positive\n    refined_parameter: function(Positive) returns Positive\n    view_parameter: function(view int64) returns Positive\n    foreign_return: function(int64) returns Text\nfunction inspect(view holder: Holder, view field: TypeField) returns function(int64) returns Positive:\n    return type.field_value[Holder, function(int64) returns Positive](view holder, view field)\nfunction unrelated(value: Text) returns nothing:\n    return nothing\n",
    );
    let types = &checked.checked().interner;
    let positive = types
        .type_ids()
        .find(
            |ty| matches!(types.resolve(*ty), Type::Refinement {name,..} if name == "app.Positive"),
        )
        .unwrap();
    let text = types
        .type_ids()
        .find(|ty| matches!(types.resolve(*ty), Type::Refinement {name,..} if name == "app.Text"))
        .unwrap();
    let function_type = |parameter, view, returned| {
        types
            .type_ids()
            .find(|ty| {
                matches!(types.resolve(*ty), Type::Function {params,view_params,return_type}
            if params == &[parameter] && view_params == &[view] && *return_type == returned)
            })
            .unwrap()
    };
    let actual = function_type(TypeInterner::INT64, false, TypeInterner::INT64);
    let requested = function_type(TypeInterner::INT64, false, positive);
    let refined_parameter = function_type(positive, false, positive);
    let view_parameter = function_type(TypeInterner::INT64, true, positive);
    let foreign_return = function_type(TypeInterner::INT64, false, text);
    assert!(
        checked
            .checked()
            .reflected_callable_plan(actual, requested)
            .is_some()
    );
    assert!(
        checked
            .checked()
            .reflected_callable_plan(requested, requested)
            .is_some(),
        "the exact callable contract remains admitted"
    );
    assert!(
        checked
            .checked()
            .reflected_callable_plan(actual, refined_parameter)
            .is_none(),
        "a refined caller parameter is not implicitly coarsened to int64"
    );
    assert!(
        checked
            .checked()
            .reflected_callable_plan(refined_parameter, actual)
            .is_none(),
        "an int64 caller parameter cannot implicitly establish Positive"
    );
    assert!(
        checked
            .checked()
            .reflected_callable_plan(actual, view_parameter)
            .is_none(),
        "owned and view parameter contracts must match"
    );
    assert!(
        checked
            .checked()
            .reflected_callable_plan(view_parameter, requested)
            .is_none(),
        "view mismatch is refused in both directions"
    );
    assert!(
        checked
            .checked()
            .reflected_callable_plan(actual, foreign_return)
            .is_none(),
        "an int64 result cannot establish a string-backed refinement"
    );
    let (program, _) = fixture();
    assert!(has_reflected_callable_plans(&program));
}
