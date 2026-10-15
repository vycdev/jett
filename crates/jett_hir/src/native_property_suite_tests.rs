use super::*;
use jett_common::SourceOrigin;
use jett_comptime::checked_types::CheckedExpressionTypes;
use std::collections::HashMap;
const NAMED: &str = "namespace app\ntype Positive = int64 where value > 0\nstruct CallbackHolder:\n    callback: function() returns int64\nfunction five() returns int64:\n    return 5\nfunction main() returns nothing:\n    CallbackHolder holder = CallbackHolder(callback: five)\n    for field in type.fields[CallbackHolder]():\n        function() returns Positive observed = type.field_value[CallbackHolder, function() returns Positive](view holder, view field)\n        Positive value = observed()\n        println(coarsen value)\n    return nothing\n\nfunction ready_five() returns Positive:\n    return 5\nverify lazy_callback_result:\n    Positive baked = comptime ready_five()\n    CallbackHolder holder = CallbackHolder(callback: five)\n    for field in type.fields[CallbackHolder]():\n        function() returns Positive observed = type.field_value[CallbackHolder, function() returns Positive](view holder, view field)\n        assert coarsen observed() == coarsen baked\nproperty lazy_callback_trials:\n    given repeat: bool\n    Positive baked = comptime ready_five()\n    CallbackHolder holder = CallbackHolder(callback: five)\n    for field in type.fields[CallbackHolder]():\n        function() returns Positive observed = type.field_value[CallbackHolder, function() returns Positive](view holder, view field)\n        assert coarsen observed() == coarsen baked\n        if repeat:\n            assert coarsen observed() == 5\n";
const CAPTURED: &str = "namespace app\ntype Positive = int64 where value > 0\nstruct CallbackHolder:\n    callback: function() returns int64\nfunction five() returns int64:\n    return 5\nfunction main() returns nothing:\n    CallbackHolder holder = CallbackHolder(callback: five)\n    for field in type.fields[CallbackHolder]():\n        function() returns Positive observed = type.field_value[CallbackHolder, function() returns Positive](view holder, view field)\n        Positive value = observed()\n        println(coarsen value)\n    return nothing\n\nverify captured_callback_result:\n    int64 seed = 7\n    CallbackHolder holder = CallbackHolder(callback: function() returns int64: return seed)\n    for field in type.fields[CallbackHolder]():\n        function() returns Positive observed = type.field_value[CallbackHolder, function() returns Positive](view holder, view field)\n        assert coarsen observed() == 7\nproperty captured_callback_trials:\n    given repeat: bool\n    int64 seed = 7\n    CallbackHolder holder = CallbackHolder(callback: function() returns int64: return seed)\n    for field in type.fields[CallbackHolder]():\n        function() returns Positive observed = type.field_value[CallbackHolder, function() returns Positive](view holder, view field)\n        assert coarsen observed() == 7\n        if repeat:\n            assert coarsen observed() == 7\n";

fn fixture(
    source: &str,
) -> (
    Program,
    Arc<CheckedResourceProgram>,
    Arc<ExplicitComptimeValues>,
    CheckedPropertyCases,
) {
    let file = FileId::new(0);
    let checked = Arc::new(
        CheckedResourceProgram::prepare(
            jett_parser::parse(source, file),
            HashMap::from([(file, SourceOrigin::Project)]),
            &[],
            jett_typecheck::CheckOptions::default(),
        )
        .unwrap(),
    );
    let evaluation = jett_comptime::evaluate_explicit_comptime_expressions_capture(
        checked.module(),
        checked.checked().reflection_metadata.clone(),
        Arc::new(CheckedExpressionTypes::from_checked_program(&checked)),
        Arc::new(checked.checked().breakpoint_exclusions.clone()),
    );
    assert!(
        evaluation.diagnostics.is_empty(),
        "{:?}",
        evaluation.diagnostics
    );
    let values = Arc::new(evaluation.values);
    let mut program = crate::lower_checked_ordinary_program_with_test_bodies(&checked).unwrap();
    crate::bake_required_values(
        &mut program,
        &values,
        &checked.checked().interner,
        &checked.checked().reflection_metadata,
        &checked.checked().method_value_definitions,
    )
    .unwrap();
    let cases = jett_comptime::verify::collect_checked_property_cases_with_checked_values(
        checked.clone(),
        values.clone(),
    )
    .unwrap();
    (program, checked, values, cases)
}
fn convert(program: &mut Program, checked: &Arc<CheckedResourceProgram>) {
    crate::complete_value_conversions(program, &checked.checked().interner).unwrap();
    crate::validate(program).unwrap();
    crate::validate_backend_types(program, &checked.checked().interner).unwrap();
}
fn calls(function: &Function) -> Vec<Expression> {
    function
        .body
        .statements
        .iter()
        .filter_map(|statement| match &statement.kind {
            StatementKind::Expression(expression)
                if matches!(expression.kind, ExpressionKind::Call { .. }) =>
            {
                Some(expression.clone())
            }
            _ => None,
        })
        .collect()
}
#[test]
fn checked_property_suite_original_named_and_captured_sources_keep_exact_100_trials() {
    for source in [NAMED, CAPTURED] {
        let (mut program, checked, _, cases) = fixture(source);
        let before = program.clone();
        let plan =
            append_checked_native_property_suite(&mut program, &checked, FileId::new(0), &cases)
                .unwrap()
                .unwrap();
        assert_eq!(plan.trials().len(), PROPERTY_DEFAULT_ITERATIONS);
        assert!(crate::resource_materialization::functions_equal(
            &before.functions,
            &program.functions[..before.functions.len()]
        ));
        assert_eq!(program.resource_manifest, before.resource_manifest);
        assert_eq!(program.equality_methods, before.equality_methods);
        let suite = &program.functions[plan.entry().index() as usize];
        assert!(suite.params.is_empty());
        assert_eq!(suite.capture_count, 0);
        assert_eq!(suite.return_type, TypeInterner::NOTHING);
        assert_eq!(calls(suite).len(), PROPERTY_DEFAULT_ITERATIONS);
        for (ordinal, (call, trial)) in calls(suite).iter().zip(plan.trials()).enumerate() {
            assert_eq!(trial.case().iteration, ordinal);
            let ExpressionKind::Call {
                function,
                args,
                evaluation_order,
                ..
            } = &call.kind
            else {
                unreachable!()
            };
            assert_eq!(*function, trial.function());
            assert_eq!(args.len(), 1);
            assert_eq!(evaluation_order, &[0]);
        }
        convert(&mut program, &checked);
        let replay = plan.trials()[7..9]
            .iter()
            .map(|trial| trial.case().clone())
            .collect::<Vec<_>>();
        let mut replayed = rebuild_checked_native_property_replay(
            &program,
            &plan,
            &replay,
            &checked.checked().interner,
        )
        .unwrap();
        assert_eq!(
            calls(&replayed.functions[plan.entry().index() as usize]).len(),
            2
        );
        convert(&mut replayed, &checked);
    }
}
#[test]
fn checked_property_suite_refuses_foreign_cache_and_original_body_or_header_forgery() {
    let (program, checked, _, cases) = fixture(NAMED);
    let (_, _, _, foreign_cases) = fixture(NAMED);
    let mut foreign = program.clone();
    assert!(
        append_checked_native_property_suite(
            &mut foreign,
            &checked,
            FileId::new(0),
            &foreign_cases
        )
        .is_err()
    );
    for case in 0..5 {
        let mut changed = program.clone();
        match case {
            0 => changed.resource_source = crate::ResourceSourceArchive::empty(),
            1 => changed
                .functions
                .iter_mut()
                .find(|function| function.identity.declaration.name == "five")
                .unwrap()
                .body
                .statements
                .clear(),
            2 => {
                changed
                    .functions
                    .iter_mut()
                    .find(|function| {
                        function.identity.declaration.kind == DeclarationKind::Property
                    })
                    .unwrap()
                    .params[0]
                    .ty = TypeInterner::STRING
            }
            3 => {
                changed
                    .functions
                    .iter_mut()
                    .find(|function| {
                        function.identity.declaration.kind == DeclarationKind::Property
                    })
                    .unwrap()
                    .identity
                    .declaration
                    .namespace = "other".into()
            }
            4 => {
                let mut function = changed.functions[0].clone();
                function.id = FunctionId::new(u32::try_from(changed.functions.len()).unwrap());
                changed.functions.push(function);
            }
            _ => unreachable!(),
        }
        let before = changed.clone();
        assert!(
            append_checked_native_property_suite(&mut changed, &checked, FileId::new(0), &cases)
                .is_err(),
            "case {case}"
        );
        assert!(crate::resource_materialization::functions_equal(
            &before.functions,
            &changed.functions
        ));
        assert_eq!(before.resource_source, changed.resource_source);
    }
}
#[test]
fn checked_property_suite_refuses_public_wrapper_mutation_and_foreign_plan_replay() {
    let (mut program, checked, _, cases) = fixture(CAPTURED);
    let plan = append_checked_native_property_suite(&mut program, &checked, FileId::new(0), &cases)
        .unwrap()
        .unwrap();
    convert(&mut program, &checked);
    let selected = vec![plan.trials()[0].case().clone()];
    for case in 0..5 {
        let mut changed = program.clone();
        let suite = &mut changed.functions[plan.entry().index() as usize];
        match case {
            0 => suite.body.statements.swap(0, 1),
            1 => suite.body.statements.push(suite.body.statements[0].clone()),
            2 => suite.return_type = TypeInterner::BOOL,
            3 => suite.identity.declaration.namespace = "other".into(),
            4 => {
                let StatementKind::Expression(expression) = &mut suite.body.statements[0].kind
                else {
                    unreachable!()
                };
                expression.kind = ExpressionKind::PropertyCaseContext(None);
            }
            _ => unreachable!(),
        }
        assert!(
            rebuild_checked_native_property_replay(
                &changed,
                &plan,
                &selected,
                &checked.checked().interner
            )
            .is_err(),
            "mutation {case}"
        );
        assert!(
            crate::complete_value_conversions(&mut changed, &checked.checked().interner).is_err()
        );
    }
    let (mut foreign, foreign_checked, _, foreign_cases) = fixture(CAPTURED);
    let foreign_plan = append_checked_native_property_suite(
        &mut foreign,
        &foreign_checked,
        FileId::new(0),
        &foreign_cases,
    )
    .unwrap()
    .unwrap();
    assert!(
        rebuild_checked_native_property_replay(
            &program,
            &foreign_plan,
            &selected,
            &checked.checked().interner
        )
        .is_err()
    );
}
#[test]
fn checked_property_replay_refuses_forged_order_purpose_shape_and_validates_changed_bool_input() {
    let (mut program, checked, _, cases) = fixture(NAMED);
    let plan = append_checked_native_property_suite(&mut program, &checked, FileId::new(0), &cases)
        .unwrap()
        .unwrap();
    convert(&mut program, &checked);
    let original = plan.trials()[0].case().clone();
    let mut changed = original.clone();
    changed.arguments[0] = Value::Bool(!matches!(original.arguments[0], Value::Bool(true)));
    let mut replay = rebuild_checked_native_property_replay(
        &program,
        &plan,
        &[changed],
        &checked.checked().interner,
    )
    .unwrap();
    convert(&mut replay, &checked);
    let mut wrong_type = original.clone();
    wrong_type.arguments[0] = Value::String("forged".into());
    let mut wrong_purpose = original.clone();
    wrong_purpose.property_span.file = FileId::new(77);
    let mut wrong_arity = original.clone();
    wrong_arity.arguments.clear();
    for forged in [
        vec![wrong_type],
        vec![wrong_purpose],
        vec![wrong_arity],
        vec![original.clone(), original.clone()],
        vec![plan.trials()[1].case().clone(), original],
    ] {
        assert!(
            rebuild_checked_native_property_replay(
                &program,
                &plan,
                &forged,
                &checked.checked().interner
            )
            .is_err()
        );
    }
}
#[test]
fn checked_property_replay_revalidates_same_payload_without_original_refinement_snapshot() {
    let source = NAMED
        .replace("given repeat: bool", "given repeat: Positive")
        .replace("if repeat:", "if coarsen repeat > 0:");
    let (mut program, checked, _, cases) = fixture(&source);
    let plan = append_checked_native_property_suite(&mut program, &checked, FileId::new(0), &cases)
        .unwrap()
        .unwrap();
    convert(&mut program, &checked);
    let mut changed = plan.trials()[0].case().clone();
    let original = changed.arguments[0].clone();
    changed.arguments[0] = Value::Int64(-1);
    assert!(!original.same_checked_snapshot(&changed.arguments[0]));
    let replay = rebuild_checked_native_property_replay(
        &program,
        &plan,
        &[changed],
        &checked.checked().interner,
    )
    .unwrap();
    let mut body = replay.functions[plan.entry().index() as usize].body.clone();
    let mut validation = 0;
    crate::resource_materialization::walk_block(&mut body, &mut |expression| {
        if matches!(
            expression.kind,
            ExpressionKind::Handle {
                kind: crate::HandleKind::Refinement { .. },
                ..
            }
        ) {
            validation += 1;
        }
        Ok(())
    })
    .unwrap();
    assert!(
        validation > 0,
        "changed input must execute the existing checked refinement validation before the property consumer"
    );
}

#[test]
fn checked_property_authenticates_every_emitted_nested_callback_capture() {
    let source = format!(
        "{NAMED}\nfunction original_inner(seed: int64) returns function() returns int64:\n    return function() returns int64: return seed\n"
    );
    let (program, checked, _, _) = fixture(&source);
    let mut interpreter = jett_comptime::Interpreter::from_checked_resource_program(
        checked.clone(),
        jett_comptime::ExecutionPurpose::ReferenceRuntime,
    )
    .unwrap();
    interpreter.set_reflection_metadata(checked.checked().reflection_metadata.clone());
    interpreter.set_checked_expression_types(Arc::new(
        CheckedExpressionTypes::from_checked_program(&checked),
    ));
    interpreter
        .set_breakpoint_exclusions(Arc::new(checked.checked().breakpoint_exclusions.clone()));
    let inner = interpreter
        .call_function("app.original_inner", vec![Value::Int64(5)])
        .unwrap();
    let types = &checked.checked().interner;
    let definition = checked
        .resolved()
        .scope_table
        .definitions
        .iter()
        .find(|definition| definition.name == "app.original_inner")
        .unwrap();
    let Type::Function {
        return_type: callback_ty,
        ..
    } = types.resolve(checked.checked().definition_types[&definition.id])
    else {
        unreachable!()
    };
    let holder_ty = types
        .type_ids()
        .find(|ty| types.type_name(*ty) == "app.CallbackHolder")
        .unwrap();
    let holder = Value::Struct {
        type_name: "app.CallbackHolder".into(),
        concrete_type: None,
        fields: vec![("callback".into(), inner.clone())],
    };
    // Source captures only the ordinary int64 seed. Moving the resulting
    // callback into its declared holder does not capture a move-only value.
    authenticate_callbacks(&program, &holder, holder_ty, &checked).unwrap();
    let functions = function_value_candidates(
        &program.functions,
        &checked.checked().method_value_definitions,
    );
    let mut locals = Vec::new();
    let mut bindings = Vec::new();
    let emitted = value_expression(
        &holder,
        holder_ty,
        checked.module().span,
        &mut ValueContext {
            actor_seeds: None,
            types,
            reflection: &checked.checked().reflection_metadata,
            functions: &functions,
            refinement_functions: None,
            locals: &mut locals,
            bindings: &mut bindings,
        },
    )
    .unwrap();
    let ExpressionKind::StructConstruct { fields, .. } = &emitted.kind else {
        panic!("typed holder")
    };
    let ExpressionKind::ClosureRef { captures, .. } = &fields[0].kind else {
        panic!("genuine callback descriptor")
    };
    assert_eq!(captures.len(), 1);
    assert_eq!(locals.len(), 1);
    assert_eq!(locals[0].ty, TypeInterner::INT64);
    assert!(matches!(
        &bindings[0].kind,
        StatementKind::Let {
            value: Expression {
                kind: ExpressionKind::Int(5),
                ..
            },
            ..
        }
    ));
    let mut forged = inner;
    let Value::Function { body, .. } = &mut forged else {
        panic!("source inline callback")
    };
    let jett_parser::ast::Stmt::Return(statement) = &mut body.stmts[0] else {
        panic!("original return")
    };
    let value = statement.value.as_mut().unwrap();
    *value = jett_parser::ast::Expr::IntLiteral(0, value.span());
    // The ordinary span/context selector can still find its original HIR
    // candidate. Recursive Source authentication must refuse the changed body.
    assert!(
        crate::evaluated_values::selected_function(&forged, *callback_ty, types, &functions)
            .is_ok()
    );
    let forged_holder = Value::Struct {
        type_name: "app.CallbackHolder".into(),
        concrete_type: None,
        fields: vec![("callback".into(), forged)],
    };
    assert!(authenticate_callbacks(&program, &forged_holder, holder_ty, &checked).is_err());
}
