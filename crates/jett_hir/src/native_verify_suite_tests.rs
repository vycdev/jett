use super::*;
use jett_common::SourceOrigin;
use jett_comptime::checked_types::CheckedExpressionTypes;
use std::{collections::HashMap, sync::Arc};

const SOURCE: &str = "namespace app\nactor Counter:\n    mutable int64 count = 1\n    receive read responds int64:\n        respond count\nfunction make() returns Counter:\n    return spawn Counter()\nverify first:\n    Counter counter = make()\n    int64 observed = ask counter.read\n    assert observed == 1\nverify second:\n    assert true\n";

fn prepared(source: &str) -> (Program, Arc<jett_typecheck::CheckedResourceProgram>) {
    let file = FileId::new(0);
    let checked = Arc::new(
        jett_typecheck::CheckedResourceProgram::prepare(
            jett_parser::parse(source, file),
            HashMap::from([(file, SourceOrigin::Project)]),
            &[],
            jett_typecheck::CheckOptions { release: false },
        )
        .unwrap(),
    );
    let evaluation = jett_comptime::evaluate_explicit_comptime_expressions_capture(
        checked.module(),
        checked.checked().reflection_metadata.clone(),
        Arc::new(CheckedExpressionTypes {
            resource_program: Some(checked.clone()),
            expressions: checked
                .checked()
                .type_map
                .iter()
                .map(|(span, ty)| (*span, checked.checked().interner.type_name(*ty)))
                .collect(),
            ..Default::default()
        }),
        Arc::new(HashMap::new()),
    );
    assert!(
        evaluation.diagnostics.is_empty(),
        "{:?}",
        evaluation.diagnostics
    );
    let mut program = crate::lower_checked_ordinary_program_with_test_bodies(&checked).unwrap();
    crate::bake_required_values(
        &mut program,
        &evaluation.values,
        &checked.checked().interner,
        &checked.checked().reflection_metadata,
        &checked.checked().method_value_definitions,
    )
    .unwrap();
    (program, checked)
}

#[test]
fn checked_native_verify_suite_preserves_actor_and_graph_tables_and_original_order() {
    for source in [
        SOURCE.to_owned(),
        SOURCE.replace("return spawn", "return comptime spawn"),
    ] {
        let (mut program, checked) = prepared(&source);
        let before = program.clone();
        let expected = before
            .resource_source
            .functions()
            .iter()
            .filter(|function| function.identity.declaration.kind == DeclarationKind::Verify)
            .map(|function| (function.id, function.span))
            .collect::<Vec<_>>();
        let entry = append_checked_native_verify_suite(&mut program, &checked, FileId::new(0))
            .unwrap()
            .unwrap();
        assert!(crate::resource_materialization::functions_equal(
            &before.functions,
            &program.functions[..before.functions.len()]
        ));
        assert_eq!(program.resource_manifest, before.resource_manifest);
        assert_eq!(program.equality_methods, before.equality_methods);
        let suite = &program.functions[entry.index() as usize];
        assert!(suite.params.is_empty() && suite.locals.is_empty());
        assert_eq!(suite.capture_count, 0);
        assert_eq!(suite.return_type, TypeInterner::NOTHING);
        let actual = suite
            .body
            .statements
            .iter()
            .map(|statement| {
                let StatementKind::Expression(Expression {
                    kind:
                        ExpressionKind::Call {
                            function,
                            args,
                            evaluation_order,
                            ..
                        },
                    ty,
                    span,
                }) = &statement.kind
                else {
                    panic!("canonical suite call");
                };
                assert!(args.is_empty() && evaluation_order.is_empty());
                assert_eq!(*ty, TypeInterner::NOTHING);
                (*function, *span)
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
        crate::complete_value_conversions(&mut program, &checked.checked().interner).unwrap();
        crate::validate(&program).unwrap();
        crate::validate_backend_types(&program, &checked.checked().interner).unwrap();
        program
            .resource_source
            .validate_current(&program, &checked.checked().interner)
            .unwrap();
        assert!(
            append_checked_native_verify_suite(&mut program, &checked, FileId::new(0)).is_err()
        );
    }
}

#[test]
fn checked_native_verify_suite_refuses_changed_original_body_and_foreign_or_empty_archive() {
    for source in [
        SOURCE.to_owned(),
        SOURCE.replace("return spawn", "return comptime spawn"),
    ] {
        let (original, checked) = prepared(&source);
        let (foreign, _) = prepared(&source);
        for case in 0..7 {
            let mut program = original.clone();
            match case {
                0 => {
                    let owner = program
                        .functions
                        .iter_mut()
                        .find(|function| function.identity.declaration.name == "make")
                        .unwrap();
                    owner.body.statements.clear();
                }
                1 => program.resource_source = foreign.resource_source.clone(),
                2 => program.resource_source = crate::ResourceSourceArchive::empty(),
                3 => {
                    let verify = program
                        .functions
                        .iter_mut()
                        .find(|function| {
                            function.identity.declaration.kind == DeclarationKind::Verify
                        })
                        .unwrap();
                    verify.return_type = TypeInterner::BOOL;
                }
                4 => {
                    let verify = program
                        .functions
                        .iter_mut()
                        .find(|function| {
                            function.identity.declaration.kind == DeclarationKind::Verify
                        })
                        .unwrap();
                    verify.body.statements.clear();
                }
                5 => {
                    let mut forged = program.functions.last().unwrap().clone();
                    forged.id = FunctionId::new(u32::try_from(program.functions.len()).unwrap());
                    forged.identity.declaration.name = "__native_verify_suite:forged".into();
                    program.functions.push(forged);
                }
                6 => {
                    program
                        .equality_methods
                        .insert(TypeInterner::INT64, FunctionId::new(0));
                }
                _ => unreachable!(),
            }
            let before = program.clone();
            assert!(
                append_checked_native_verify_suite(&mut program, &checked, FileId::new(0)).is_err(),
                "case {case}"
            );
            assert!(crate::resource_materialization::functions_equal(
                &before.functions,
                &program.functions
            ));
            assert_eq!(program.resource_source, before.resource_source);
        }
    }
}

#[test]
fn checked_native_verify_suite_rejects_extra_reordered_mutated_calls_and_headers() {
    for source in [
        SOURCE.to_owned(),
        SOURCE.replace("return spawn", "return comptime spawn"),
    ] {
        let (mut original, checked) = prepared(&source);
        let entry = append_checked_native_verify_suite(&mut original, &checked, FileId::new(0))
            .unwrap()
            .unwrap();
        for case in 0..6 {
            let mut program = original.clone();
            let suite = &mut program.functions[entry.index() as usize];
            match case {
                0 => suite.body.statements.push(suite.body.statements[0].clone()),
                1 => suite.body.statements.swap(0, 1),
                2 => {
                    let StatementKind::Expression(expression) = &mut suite.body.statements[0].kind
                    else {
                        unreachable!();
                    };
                    expression.ty = TypeInterner::BOOL;
                }
                3 => suite.identity.declaration.namespace = "other".into(),
                4 => suite.return_type = TypeInterner::BOOL,
                5 => {
                    let StatementKind::Expression(Expression {
                        kind: ExpressionKind::Call { function, .. },
                        ..
                    }) = &mut suite.body.statements[0].kind
                    else {
                        unreachable!();
                    };
                    *function = entry;
                }
                _ => unreachable!(),
            }
            assert!(
                crate::complete_value_conversions(&mut program, &checked.checked().interner)
                    .is_err(),
                "mutated canonical suite {case}"
            );
            assert!(
                append_checked_native_verify_suite(&mut program, &checked, FileId::new(0)).is_err(),
                "mutated suite cannot renew custody {case}"
            );
        }
    }
}

#[test]
fn checked_native_verify_suite_without_entry_file_verifies_keeps_the_exact_archive() {
    let (mut program, checked) = prepared(SOURCE);
    let before = program.clone();
    assert_eq!(
        append_checked_native_verify_suite(&mut program, &checked, FileId::new(1)).unwrap(),
        None
    );
    assert!(crate::resource_materialization::functions_equal(
        &before.functions,
        &program.functions
    ));
    assert_eq!(program.resource_source, before.resource_source);
}
