use super::*;
use jett_resolve::ResourceKernelSpec;
use jett_typecheck::{CheckOptions, CheckedResourceProgram};
use jett_types::ResourceKernelRecipe;

const SUPPORT: &str =
    include_str!("../../jett_driver/tests/native_conformance/resource/resource_probe.jett");
const OCCUPIED: &str = include_str!(
    "../../jett_driver/tests/native_conformance/resource/47_view_optional_some_written.jett"
);

fn checked_resource_source(source: &str, release: bool) -> Arc<CheckedResourceProgram> {
    let support = FileId::new(10_000);
    let project = FileId::new(0);
    let mut parsed = jett_parser::parse(SUPPORT, support);
    let primary = jett_parser::parse(source, project);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    assert!(primary.errors.is_empty(), "{:?}", primary.errors);
    let declarations = parsed
        .module
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Resource(value) => Some(value.name.span),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [declaration] = declarations.as_slice() else {
        panic!("one Resource declaration");
    };
    let catalog = [
        ("kernel_create", ResourceKernelRecipe::NetworkFactory),
        ("kernel_borrow", ResourceKernelRecipe::NetworkBorrow),
        ("kernel_close", ResourceKernelRecipe::Finalize),
    ]
    .into_iter()
    .map(|(member, recipe)| ResourceKernelSpec {
        resource_declaration: *declaration,
        member: member.into(),
        recipe,
    })
    .collect::<Vec<_>>();
    parsed.module.items.extend(primary.module.items);
    Arc::new(
        CheckedResourceProgram::prepare(
            parsed,
            HashMap::from([
                (support, SourceOrigin::Stdlib),
                (project, SourceOrigin::Project),
            ]),
            &catalog,
            CheckOptions { release },
        )
        .unwrap_or_else(|error| panic!("{error:?}: {:?}", error.diagnostics())),
    )
}

fn observer(program: &Program) -> &Function {
    program
        .functions
        .iter()
        .find(|function| {
            ["observe_optional", "observe_result"]
                .contains(&function.identity.declaration.name.as_str())
        })
        .expect("genuine observer function")
}

fn alias_initializer(function: &Function) -> (&Local, &Expression) {
    let alias = function
        .locals
        .iter()
        .find(|local| local.name == "token")
        .expect("borrowed payload local");
    let value = function
        .body
        .statements
        .iter()
        .find_map(|statement| match &statement.kind {
            StatementKind::Let { local, value } if *local == alias.id => Some(value),
            _ => None,
        })
        .expect("original payload initializer");
    (alias, value)
}

#[test]
fn borrowed_sum_views_lower_shared_checked_sources() {
    let sources = [
        include_str!(
            "../../jett_driver/tests/native_conformance/resource/45_view_optional_none_written.jett"
        ),
        include_str!(
            "../../jett_driver/tests/native_conformance/resource/46_view_optional_none_bare.jett"
        ),
        OCCUPIED,
        include_str!(
            "../../jett_driver/tests/native_conformance/resource/48_view_optional_some_bare.jett"
        ),
        include_str!(
            "../../jett_driver/tests/native_conformance/resource/49_view_result_fail_written.jett"
        ),
        include_str!(
            "../../jett_driver/tests/native_conformance/resource/50_view_result_fail_bare.jett"
        ),
        include_str!(
            "../../jett_driver/tests/native_conformance/resource/51_view_result_ok_written.jett"
        ),
        include_str!(
            "../../jett_driver/tests/native_conformance/resource/52_view_result_ok_bare.jett"
        ),
        include_str!(
            "../../jett_driver/tests/native_conformance/resource/53_view_optional_written_argument_abort.jett"
        ),
        include_str!(
            "../../jett_driver/tests/native_conformance/resource/54_view_result_bare_argument_abort.jett"
        ),
        include_str!(
            "../../jett_driver/tests/native_conformance/resource/55_view_optional_written_borrow_fail.jett"
        ),
        include_str!(
            "../../jett_driver/tests/native_conformance/resource/56_view_result_bare_borrow_fail.jett"
        ),
    ];
    for release in [false, true] {
        for source in sources {
            let checked = checked_resource_source(source, release);
            let program = lower_checked_resource_program(&checked).unwrap();
            validate_backend_types(&program, &checked.checked().interner).unwrap();
            assert!(program.resource_source.belongs_to(&checked));
            assert_eq!(program.resource_source.functions(), program.functions);
            let function = observer(&program);
            let (alias, initializer) = alias_initializer(function);
            let backing = function
                .locals
                .iter()
                .find(|local| local.name == "outcome")
                .unwrap();
            assert_eq!(alias.view_source, Some(backing.id));
            assert_ne!(alias.ty, backing.ty);
            assert!(function.params.iter().any(|parameter| {
                parameter.local == backing.id && parameter.mode == ParamMode::View
            }));
            let handle = borrowed_sum_view_initializer(
                initializer,
                backing.id,
                backing.ty,
                alias.ty,
                &checked.checked().interner,
            )
            .unwrap()
            .unwrap();
            assert!(matches!(handle.kind, ExpressionKind::Handle { .. }));
        }
    }
}

#[test]
fn borrowed_sum_views_reject_changed_hir_projection() {
    let checked = checked_resource_source(OCCUPIED, false);
    let program = lower_checked_resource_program(&checked).unwrap();
    let function = observer(&program);
    let (alias, _) = alias_initializer(function);
    let alias_id = alias.id;
    let backing_id = alias.view_source.unwrap();
    let foreign_id = function.params[0].local;
    for change in [
        "kind",
        "source",
        "source type",
        "endpoint",
        "inner view",
        "outer view",
        "error binding",
        "default",
        "conditional default",
        "orphan",
    ] {
        let mut changed = program.clone();
        let function = changed
            .functions
            .iter_mut()
            .find(|function| function.identity.declaration.name == "observe_optional")
            .unwrap();
        if change == "orphan" {
            function.body.statements.retain(|statement| {
                !matches!(statement.kind, StatementKind::Let { local, .. } if local == alias_id)
            });
        } else {
            let source_type = function.locals[backing_id.index() as usize].ty;
            if change == "endpoint" {
                function.locals[alias_id.index() as usize].ty = source_type;
            }
            let value = function
                .body
                .statements
                .iter_mut()
                .find_map(|statement| match &mut statement.kind {
                    StatementKind::Let { local, value } if *local == alias_id => Some(value),
                    _ => None,
                })
                .unwrap();
            if change == "outer view" {
                let ExpressionKind::View(inner) = &value.kind else {
                    panic!("original outer view");
                };
                *value = inner.as_ref().clone();
            } else {
                if change == "endpoint" {
                    value.ty = source_type;
                }
                let ExpressionKind::View(handle) = &mut value.kind else {
                    panic!("original outer view");
                };
                if change == "endpoint" {
                    handle.ty = source_type;
                }
                let ExpressionKind::Handle {
                    target,
                    kind,
                    error_local,
                    failure,
                } = &mut handle.kind
                else {
                    panic!("original sum Handle");
                };
                match change {
                    "kind" => *kind = HandleKind::Result,
                    "error binding" => *error_local = Some(foreign_id),
                    "inner view" => {
                        let ExpressionKind::View(inner) = &target.kind else {
                            panic!("original inner view");
                        };
                        *target = inner.clone();
                    }
                    "source" | "source type" => {
                        let ExpressionKind::View(backing) = &mut target.kind else {
                            panic!("original inner view");
                        };
                        if change == "source" {
                            backing.kind = ExpressionKind::Local(foreign_id);
                        } else {
                            backing.ty = alias.ty;
                        }
                    }
                    "default" | "conditional default" => {
                        let alternate = Statement {
                            kind: StatementKind::HandleDefault(Expression {
                                kind: ExpressionKind::Local(foreign_id),
                                ty: alias.ty,
                                span: handle.span,
                            }),
                            span: handle.span,
                        };
                        let alternate = if change == "conditional default" {
                            Statement {
                                kind: StatementKind::If {
                                    condition: Expression {
                                        kind: ExpressionKind::Bool(true),
                                        ty: TypeInterner::BOOL,
                                        span: handle.span,
                                    },
                                    then_block: Block {
                                        statements: vec![alternate],
                                        span: handle.span,
                                    },
                                    else_block: None,
                                },
                                span: handle.span,
                            }
                        } else {
                            alternate
                        };
                        failure.statements.insert(0, alternate);
                    }
                    "endpoint" => {}
                    _ => panic!("bounded mutation"),
                }
            }
        }
        let errors = validate_backend_types(&changed, &checked.checked().interner).unwrap_err();
        assert!(
            errors.iter().any(|error| {
                error.message.contains("borrowed Handle")
                    || error.message.contains("borrowed alias")
                    || error.message.contains("no validated initializer")
            }),
            "{change}: {errors:?}"
        );
    }
}

#[test]
fn borrowed_sum_views_refuse_temporaries_and_outer_handler_defaults() {
    for initializer in [
        "view((view temporary()) handle:\n        return nothing\n    )",
        "view((view source) handle:\n        default list(9)\n    )",
        "view((view source) handle:\n        if flag:\n            default list(9)\n        return nothing\n    )",
        "view(source handle:\n        return nothing\n    )",
    ] {
        let source = format!(
            "namespace app\nfunction temporary() returns optional[list[int64]]:\n    return some(list(1))\nfunction inspect(view source: optional[list[int64]], flag: bool) returns nothing:\n    list[int64] alias = {initializer}\n    return nothing\n"
        );
        let errors = crate::local_view_tests::checked_source(&source, false).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("native borrowed alias")),
            "{initializer}: {errors:?}"
        );
    }
}

#[test]
fn borrowed_sum_views_keep_nested_handle_defaults_and_generic_payload_identity() {
    let source = r#"namespace app
function inspect[T](view source: optional[T], view result_source: result[T, string]) returns nothing:
    T alias = view((view source) handle:
        optional[int64] missing = none
        int64 marker = missing handle:
            default 7
        return nothing
    )
    T result_alias = view((view result_source) handle error:
        return nothing
    )
    return nothing
function main() returns nothing:
    inspect[list[int64]](view some(list(1)), view ok(list(2)))
    inspect[int64](view some(3), view ok(4))
    return nothing
"#;
    let (program, checked) = crate::local_view_tests::checked_source(source, false).unwrap();
    validate_backend_types(&program, &checked.interner).unwrap();
    let instances = program
        .functions
        .iter()
        .filter(|function| function.identity.declaration.name == "inspect")
        .collect::<Vec<_>>();
    assert_eq!(instances.len(), 2);
    for function in instances {
        for (name, source_name) in [("alias", "source"), ("result_alias", "result_source")] {
            let alias = function
                .locals
                .iter()
                .find(|local| local.name == name)
                .unwrap();
            let backing = function
                .locals
                .iter()
                .find(|local| local.name == source_name)
                .unwrap();
            if alias.ty == TypeInterner::INT64 {
                assert_eq!(alias.view_source, None, "implicitly copyable payload");
            } else {
                assert_eq!(alias.view_source, Some(backing.id));
                let value = function
                    .body
                    .statements
                    .iter()
                    .find_map(|statement| match &statement.kind {
                        StatementKind::Let { local, value } if *local == alias.id => Some(value),
                        _ => None,
                    })
                    .unwrap();
                assert!(
                    borrowed_sum_view_initializer(
                        value,
                        backing.id,
                        backing.ty,
                        alias.ty,
                        &checked.interner
                    )
                    .unwrap()
                    .is_some()
                );
            }
        }
    }
}
