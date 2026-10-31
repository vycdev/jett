use super::*;
use jett_common::{FileId, SourceOrigin};

fn fixture(
    source_return: &str,
    requested_return: &str,
) -> (Program, Arc<jett_typecheck::CheckedResourceProgram>) {
    let source = format!(
        "namespace app\ntype Positive = int64 where value > 0\ntype Higher = Positive where value > 10\nfunction five() returns int64:\n    return 5\nstruct Holder:\n    callback: function() returns {source_return}\nfunction inspect(view holder: Holder, view field: TypeField) returns function() returns {requested_return}:\n    return type.field_value[Holder, function() returns {requested_return}](view holder, view field)\n"
    );
    source_fixture(&source)
}

fn source_fixture(source: &str) -> (Program, Arc<jett_typecheck::CheckedResourceProgram>) {
    let file = FileId::new(0);
    let parsed = jett_parser::parse(source, file);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let checked = Arc::new(
        jett_typecheck::CheckedResourceProgram::prepare(
            parsed,
            HashMap::from([(file, SourceOrigin::Project)]),
            &[],
            jett_typecheck::CheckOptions { release: false },
        )
        .unwrap(),
    );
    let hir = hir::lower_checked_ordinary_program(&checked).expect("ordinary original checked HIR");
    let mir = lower(&hir, &checked.checked().interner).expect("canonical reflected MIR");
    (mir, checked)
}

fn adapter(program: &Program) -> usize {
    program
        .functions
        .iter()
        .position(|function| {
            function.blocks.iter().any(|block| {
                block.statements.iter().any(|statement| {
                    matches!(statement.kind, StatementKind::ReflectedValueReady { .. })
                })
            })
        })
        .expect("private adapter")
}

#[test]
fn read_is_lazy_and_invocation_orders_actual_result_preflight_predicate_publication() {
    let (program, checked) = fixture("int64", "Positive");
    validate_reflected_callable_plans(&program, &checked.checked().interner).unwrap();
    let reader = program
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "inspect")
        .unwrap();
    assert!(
        !reader
            .blocks
            .iter()
            .flat_map(|block| &block.statements)
            .any(|statement| matches!(
                statement.kind,
                StatementKind::CheckRefinement { .. } | StatementKind::ReflectedValueReady { .. }
            ))
    );
    let mut reads = 0usize;
    for block in &reader.blocks {
        resource_ownership::walk::mir_block(block, &mut |value| {
            if matches!(
                value.kind,
                hir::ExpressionKind::Intrinsic {
                    field_validation: Some(hir::ReflectedFieldValidation::ReadCallable(_)),
                    ..
                }
            ) {
                reads += 1;
            }
        });
    }
    assert_eq!(reads, 1);
    let function = &program.functions[adapter(&program)];
    let statements = function
        .blocks
        .iter()
        .flat_map(|block| &block.statements)
        .collect::<Vec<_>>();
    let call = statements
        .iter()
        .position(|statement| {
            matches!(
                statement.kind,
                StatementKind::Let {
                    value: Expression {
                        kind: hir::ExpressionKind::IndirectCall { .. },
                        ..
                    },
                    ..
                }
            )
        })
        .unwrap();
    assert_eq!(
        statements
            .iter()
            .filter(|statement| matches!(
                statement.kind,
                StatementKind::Let {
                    value: Expression {
                        kind: hir::ExpressionKind::IndirectCall { .. },
                        ..
                    },
                    ..
                }
            ))
            .count(),
        1,
        "actual source invocation once"
    );
    let ready = statements
        .iter()
        .position(|statement| {
            matches!(
                statement.kind,
                StatementKind::ReflectedValueReady {
                    inspect_pending: true,
                    ..
                }
            )
        })
        .unwrap();
    let predicate = statements
        .iter()
        .position(|statement| matches!(statement.kind, StatementKind::CheckRefinement { .. }))
        .unwrap();
    assert!(call < ready && ready < predicate);
    assert!(function.blocks.iter().any(|block| matches!(
        &block.terminator.kind,
        TerminatorKind::Return(Some(Expression {
            kind: hir::ExpressionKind::RefinementValidated(_),
            ..
        }))
    )));
}

#[test]
fn ancestor_transport_has_no_new_pending_inspection_or_predicate() {
    let (program, checked) = fixture("Higher", "Positive");
    let function = &program.functions[adapter(&program)];
    assert!(
        function
            .blocks
            .iter()
            .flat_map(|block| &block.statements)
            .any(|statement| matches!(
                statement.kind,
                StatementKind::ReflectedValueReady {
                    inspect_pending: false,
                    ..
                }
            ))
    );
    assert!(
        !function
            .blocks
            .iter()
            .flat_map(|block| &block.statements)
            .any(|statement| matches!(
                statement.kind,
                StatementKind::ReflectedValueReady {
                    inspect_pending: true,
                    ..
                } | StatementKind::CheckRefinement { .. }
            ))
    );
    let returned = function
        .blocks
        .iter()
        .find_map(|block| match &block.terminator.kind {
            TerminatorKind::Return(Some(value)) => Some(value),
            _ => None,
        })
        .expect("ancestor adapter return");
    let hir::ExpressionKind::Coarsen(inner) = &returned.kind else {
        panic!("ancestor proof must be transported by typed coarsening");
    };
    assert!(
        matches!(checked.checked().interner.resolve(inner.ty), Type::Refinement { base, .. } if *base == returned.ty)
    );
    validate_reflected_callable_plans(&program, &checked.checked().interner).unwrap();
}

#[test]
fn pending_scalar_and_unit_ancestor_returns_transport_the_original_proof_and_cannot_relabel_it() {
    for (base, value) in [("int64", "0"), ("nothing", "nothing")] {
        let source = format!(
            "namespace app\ntype Ready = {base} where true\ntype Higher = Ready where true\nstruct Holder:\n    callback: function() returns Higher\nfunction pending_source() returns Higher:\n    Higher proved = {value} handle error:\n        return pending_source()\n    return run run proved\nfunction inspect(view holder: Holder, view field: TypeField) returns function() returns Ready:\n    return type.field_value[Holder, function() returns Ready](view holder, view field)\n"
        );
        let (mut program, checked) = source_fixture(&source);
        let types = &checked.checked().interner;
        let index = adapter(&program);
        for prepared in [false, true] {
            if prepared {
                prepare_native_sequences(&mut program, types);
                prepare_native_uninhabited_sums(&mut program, types);
                prepare_native_generated_functions(&mut program, types);
            }
            validate_reflected_callable_plans(&program, types).unwrap();
            let function = &program.functions[index];
            assert!(
                !function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.statements)
                    .any(|statement| matches!(
                        statement.kind,
                        StatementKind::CheckRefinement { .. }
                            | StatementKind::ReflectedValueReady {
                                inspect_pending: true,
                                ..
                            }
                    ))
            );
            let mut joins = 0;
            let mut new_proofs = 0;
            for block in &function.blocks {
                resource_ownership::walk::mir_block(block, &mut |expression| {
                    joins += usize::from(matches!(expression.kind, hir::ExpressionKind::Join(_)));
                    new_proofs += usize::from(matches!(
                        expression.kind,
                        hir::ExpressionKind::RefinementValidated(_)
                    ));
                });
            }
            assert_eq!(joins, 0, "{base} pending layers stay intact");
            assert_eq!(
                new_proofs, 0,
                "{base} existing ancestor proof is not revalidated"
            );
            let returned = function
                .blocks
                .iter()
                .find_map(|block| match &block.terminator.kind {
                    TerminatorKind::Return(Some(value)) => Some(value),
                    _ => None,
                })
                .expect("transported ancestor return");
            let hir::ExpressionKind::Coarsen(inner) = &returned.kind else {
                panic!("{base} exact ancestor transport needs Coarsen");
            };
            assert!(matches!(inner.kind, hir::ExpressionKind::Local(_)));
            assert!(
                matches!(types.resolve(inner.ty), Type::Refinement { base, .. } if *base == returned.ty)
            );
            let mut forged = program.clone();
            let returned = forged.functions[index]
                .blocks
                .iter_mut()
                .find_map(|block| match &mut block.terminator.kind {
                    TerminatorKind::Return(Some(value)) => Some(value),
                    _ => None,
                })
                .unwrap();
            let hir::ExpressionKind::Coarsen(inner) = &returned.kind else {
                unreachable!()
            };
            returned.kind = hir::ExpressionKind::RefinementValidated(inner.clone());
            assert!(
                validate_reflected_callable_plans(&forged, types).is_err(),
                "{base} public return relabel cannot refresh the private original graph"
            );
        }
    }
}

#[test]
fn recursive_produced_return_completes_all_occupied_preflight_before_any_predicate() {
    let (program, checked) = fixture("list[optional[int64]]", "list[optional[Positive]]");
    let function = &program.functions[adapter(&program)];
    let mut visited = HashSet::new();
    let mut pending = vec![(function.entry, false)];
    while let Some((block, predicate_seen)) = pending.pop() {
        if !visited.insert((block, predicate_seen)) {
            continue;
        }
        let block = &function.blocks[block.index() as usize];
        let mut seen = predicate_seen;
        for statement in &block.statements {
            if matches!(statement.kind, StatementKind::CheckRefinement { .. }) {
                seen = true;
            }
            assert!(
                !(seen
                    && matches!(
                        statement.kind,
                        StatementKind::ReflectedValueReady {
                            inspect_pending: true,
                            ..
                        } | StatementKind::ReflectedContainerReady { .. }
                    )),
                "preflight must not run after an earlier predicate"
            );
        }
        match &block.terminator.kind {
            TerminatorKind::Goto(next) => pending.push((*next, seen)),
            TerminatorKind::Branch {
                then_block,
                else_block,
                ..
            } => {
                pending.push((*then_block, seen));
                pending.push((*else_block, seen));
            }
            _ => {}
        }
    }
    assert!(
        function
            .blocks
            .iter()
            .flat_map(|block| &block.statements)
            .any(|statement| matches!(
                statement.kind,
                StatementKind::ReflectedValueReady {
                    inspect_pending: true,
                    ..
                }
            ))
    );
    validate_reflected_callable_plans(&program, &checked.checked().interner).unwrap();
}

#[test]
fn graph_and_tuple_and_body_and_witness_forgery_cannot_reseal() {
    let (original, checked) = fixture("int64", "Positive");
    let types = &checked.checked().interner;
    for mutation in 0..7 {
        let mut program = original.clone();
        let index = adapter(&program);
        match mutation {
            0 => {
                let statement = program.functions[index]
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.statements)
                    .find(|statement| {
                        matches!(
                            statement.kind,
                            StatementKind::ReflectedValueReady {
                                inspect_pending: true,
                                ..
                            }
                        )
                    })
                    .unwrap();
                let StatementKind::ReflectedValueReady {
                    inspect_pending, ..
                } = &mut statement.kind
                else {
                    unreachable!()
                };
                *inspect_pending = false;
            }
            1 => {
                program.functions[index].reflected_callable = None;
            }
            2 => {
                for function in &mut program.functions {
                    function.reflected_callable = None;
                }
            }
            3 => {
                let function = program
                    .functions
                    .iter_mut()
                    .find(|function| function.identity.declaration.name == "five")
                    .unwrap();
                let TerminatorKind::Return(Some(value)) = &mut function.blocks
                    [function.entry.index() as usize]
                    .terminator
                    .kind
                else {
                    panic!("source return")
                };
                value.kind = hir::ExpressionKind::Int(0);
            }
            4 => {
                program.functions[index].identity.declaration.namespace = "other".into();
            }
            5 => {
                let statement = program.functions[index]
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.statements)
                    .find(|statement| {
                        matches!(statement.kind, StatementKind::CheckRefinement { .. })
                    })
                    .unwrap();
                let StatementKind::CheckRefinement { call, .. } = &mut statement.kind else {
                    unreachable!()
                };
                let hir::ExpressionKind::Call { args, .. } = &mut call.kind else {
                    panic!("predicate tuple")
                };
                args[0].kind = hir::ExpressionKind::Int(7);
            }
            _ => {
                let function = program
                    .functions
                    .iter_mut()
                    .find(|function| {
                        function.identity.declaration.kind
                            == hir::DeclarationKind::RefinementPredicate
                            && function.identity.declaration.namespace == "app"
                    })
                    .unwrap();
                let returned = function
                    .blocks
                    .iter_mut()
                    .find_map(|block| match &mut block.terminator.kind {
                        TerminatorKind::Return(Some(value)) => Some(value),
                        _ => None,
                    })
                    .expect("original predicate result");
                returned.kind = hir::ExpressionKind::Bool(false);
            }
        }
        assert!(
            validate_reflected_callable_plans(&program, types).is_err(),
            "forgery {mutation}"
        );
        prepare_native_generated_functions(&mut program, types);
        prepare_native_sequences(&mut program, types);
        prepare_native_uninhabited_sums(&mut program, types);
        assert!(
            validate_reflected_callable_plans(&program, types).is_err(),
            "preparation cannot reseal forgery {mutation}"
        );
    }
}

#[test]
fn canonical_preparation_preserves_private_original_and_current_graphs() {
    let (mut program, checked) = fixture("int64", "Positive");
    let types = &checked.checked().interner;
    prepare_native_generated_functions(&mut program, types);
    validate_reflected_callable_plans(&program, types).unwrap();
    prepare_native_sequences(&mut program, types);
    validate_reflected_callable_plans(&program, types).unwrap();
    prepare_native_uninhabited_sums(&mut program, types);
    validate_reflected_callable_plans(&program, types).unwrap();
}
