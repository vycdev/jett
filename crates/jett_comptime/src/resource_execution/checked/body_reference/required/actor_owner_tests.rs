use super::*;
use crate::checked_types::CheckedExpressionTypes;
use jett_common::{FileId, SourceOrigin};
use jett_diagnostics::Severity;
use jett_typecheck::CheckOptions;
use jett_types::TypeInterner;

const WITNESS: &str = include_str!("../../../fixtures/actor_handler_required_callable.jett");

fn checked(source: &str, release: bool) -> Arc<CheckedResourceProgram> {
    let parsed = jett_parser::parse(source, FileId::new(0));
    assert!(
        !parsed
            .errors
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error),
        "{:?}",
        parsed.errors
    );
    Arc::new(
        CheckedResourceProgram::prepare(
            parsed,
            HashMap::from([(FileId::new(0), SourceOrigin::Project)]),
            &[],
            CheckOptions { release },
        )
        .unwrap_or_else(|error| panic!("{error:?}; {:?}", error.diagnostics())),
    )
}

fn required(program: &Arc<CheckedResourceProgram>) -> crate::ExplicitComptimeEvaluation {
    crate::evaluate_explicit_comptime_expressions_capture(
        program.module(),
        program.checked().reflection_metadata.clone(),
        Arc::new(CheckedExpressionTypes::from_checked_program(program)),
        Arc::new(program.checked().breakpoint_exclusions.clone()),
    )
}

fn actor<'a>(program: &'a CheckedResourceProgram, name: &str) -> &'a jett_parser::ast::ActorDef {
    program
        .module()
        .items
        .iter()
        .find_map(|item| match item {
            Item::Actor(actor) if actor.name.name == name => Some(actor),
            _ => None,
        })
        .unwrap()
}

fn occurrence(handler: &jett_parser::ast::ReceiveHandler) -> (&Expr, Span) {
    let mut found = Vec::new();
    walk_region(
        OriginalRegion::Block(&handler.body),
        &mut |_| {},
        &mut |node| {
            if let Expr::Comptime(expression, span) = node {
                found.push((expression.as_ref(), *span));
            }
        },
        &mut |_| {},
    );
    let [value] = found.as_slice() else {
        panic!("one original required actor occurrence")
    };
    *value
}

#[test]
fn actor_required_witness_retains_exact_successful_cache_and_callable() {
    for release in [false, true] {
        let program = checked(WITNESS, release);
        assert!(program.checked().resource_hooks.is_empty());
        let evaluated = required(&program);
        assert!(
            evaluated.diagnostics.is_empty(),
            "{:?}",
            evaluated.diagnostics
        );
        assert!(evaluated.debug_events.is_empty());
        assert_eq!(evaluated.values.len(), 1);
        assert!(
            evaluated
                .values
                .same_checked_cache(&evaluated.values.clone())
        );
        let proofs = evaluated.values.checked_required_values(&program).unwrap();
        let [proof] = proofs.as_slice() else {
            panic!("one privately cached actor required value")
        };
        let actor = actor(&program, "Worker");
        let handler = &actor.handlers[0];
        let (original, span) = occurrence(handler);
        let checked =
            CheckedExecution::new(program.clone(), ExecutionPurpose::ExplicitComptime).unwrap();
        let contexts = checked
            .prepare_explicit_contexts(original, span, None, &[])
            .unwrap();
        let [context] = contexts.as_slice() else {
            panic!("one exact actor required context")
        };
        let BodyRoot::ActorHandler(target) = context.reference.cursor.root else {
            panic!("actor metadata root")
        };
        assert_eq!(
            proof.owner(),
            CheckedRequiredOwner::ActorHandler {
                target,
                declaration: handler.name.span
            }
        );
        assert_eq!(proof.source_span(), span);
        assert!(proof.generic_index().is_none());
        assert!(proof.type_arguments().is_empty());
        assert!(proof.scoped_bindings().is_empty());
        assert!(proof.original_inline_function().unwrap().is_none());
        assert!(
            matches!(proof.original_comptime().unwrap(), Expr::Comptime(inner, occurrence)
            if *occurrence == span && std::ptr::eq(inner.as_ref(), original))
        );
        assert!(!proof.matches_original_comptime(&proof.original_comptime().unwrap().clone()));
        let crate::Value::ReflectedCallable(callable) = proof.value().payload() else {
            panic!("actual lazily adapted reflected callback")
        };
        assert!(Arc::ptr_eq(callable.checked_program(), &program));
        assert_ne!(callable.source_type(), callable.requested_type());
        let definition = callable.source_definition().unwrap();
        assert_eq!(
            program.resolved().scope_table.def(definition).name,
            "app.five"
        );
        assert_eq!(
            program.checked().definition_types.get(&definition),
            Some(&callable.source_type())
        );
        assert!(std::ptr::eq(context.expression().unwrap(), original));
        assert_eq!(context.result_type(), callable.requested_type());
        assert_eq!(
            checked.validate_executable_cursor(&context.reference.cursor),
            Err(ResourceExecutionError::WrongPurpose)
        );
    }
}

#[test]
fn actor_required_initializers_keep_distinct_source_owners() {
    const SOURCE: &str = "namespace app
function five() returns int64:
    return 5
actor Counter:
    mutable int64 first = comptime five()
    mutable int64 second = comptime five()
    receive read responds int64:
        respond first + second
function main() returns nothing:
    Counter counter = spawn Counter()
    int64 value = ask counter.read
    return nothing
";
    for release in [false, true] {
        let program = checked(SOURCE, release);
        let evaluated = required(&program);
        assert!(
            evaluated.diagnostics.is_empty(),
            "{:?}",
            evaluated.diagnostics
        );
        assert!(evaluated.debug_events.is_empty());
        let proofs = evaluated.values.checked_required_values(&program).unwrap();
        assert_eq!(proofs.len(), 2);
        let counter = actor(&program, "Counter");
        let execution =
            CheckedExecution::new(program.clone(), ExecutionPurpose::ExplicitComptime).unwrap();
        let item = program
            .module()
            .items
            .iter()
            .position(
                |item| matches!(item, Item::Actor(original) if std::ptr::eq(original, counter)),
            )
            .unwrap();
        let definition = execution.required_actor_declaration(item).unwrap().0;
        for (field, initializer) in counter.state_fields.iter().enumerate() {
            let proof = proofs
                .iter()
                .find(|proof| {
                    proof.owner()
                        == CheckedRequiredOwner::ActorInitializer {
                            definition,
                            declaration: initializer.name.span,
                            field,
                        }
                })
                .unwrap();
            assert_eq!(proof.value(), &crate::Value::Int64(5));
            assert!(proof.matches_original_comptime(&initializer.value));
            assert!(!proof.matches_original_comptime(&counter.state_fields[1 - field].value));
            assert!(proof.original_inline_function().unwrap().is_none());
        }
    }
}

#[test]
fn actor_required_metadata_refuses_changed_owner_target_clone_foreign_and_runtime_entry() {
    const SOURCE: &str = "namespace app
function five() returns int64:
    return 5
actor Worker:
    mutable int64 count = 0
    receive first responds int64:
        int64 value = comptime five()
        respond value
    receive second responds int64:
        int64 value = comptime five()
        respond value
function main() returns nothing:
    Worker worker = spawn Worker()
    int64 first = ask worker.first
    int64 second = ask worker.second
    return nothing
";
    for release in [false, true] {
        let program = checked(SOURCE, release);
        let mut execution =
            CheckedExecution::new(program.clone(), ExecutionPurpose::ExplicitComptime).unwrap();
        let original_actor = actor(&program, "Worker");
        let (expression, span) = occurrence(&original_actor.handlers[0]);
        let first = execution
            .prepare_explicit_contexts(expression, span, None, &[])
            .unwrap()
            .remove(0);
        let (sibling, sibling_span) = occurrence(&original_actor.handlers[1]);
        let second = execution
            .prepare_explicit_contexts(sibling, sibling_span, None, &[])
            .unwrap()
            .remove(0);
        assert_ne!(first.key(), second.key());
        assert!(!first.key().is_lexical_descendant_of(&second.key()));
        assert!(!second.key().is_lexical_descendant_of(&first.key()));
        assert!(
            execution
                .prepare_explicit_contexts(&expression.clone(), span, None, &[])
                .is_err()
        );
        assert!(
            execution
                .prepare_explicit_contexts(expression, sibling_span, None, &[])
                .is_err()
        );
        assert!(
            execution
                .prepare_explicit_contexts(expression, span, Some(original_actor.name.span), &[])
                .is_err()
        );
        let foreign =
            CheckedExecution::new(checked(SOURCE, release), ExecutionPurpose::ExplicitComptime)
                .unwrap();
        assert!(
            foreign
                .prepare_explicit_contexts(expression, span, None, &[])
                .is_err()
        );
        assert_eq!(
            foreign.validate_required_source_cursor(&first.reference.cursor),
            Err(ResourceExecutionError::ForeignProgram)
        );
        let mut wrong = first.reference.cursor.clone();
        let BodyRoot::ActorHandler(target) = &mut wrong.root else {
            panic!("actor metadata root")
        };
        target.handler_index = 1;
        assert!(execution.validate_required_source_cursor(&wrong).is_err());
        let mut wrong = first.reference.cursor.clone();
        let BodyRoot::ActorHandler(target) = &mut wrong.root else {
            panic!("actor metadata root")
        };
        target.actor_definition = program
            .resolved()
            .scope_table
            .definitions
            .iter()
            .find(|definition| {
                definition.kind == jett_resolve::DefKind::Function && definition.name == "app.five"
            })
            .unwrap()
            .id;
        assert!(execution.validate_required_source_cursor(&wrong).is_err());
        let mut wrong = first.reference.cursor.clone();
        let ExecutableOrigin::ActorHandler { handler, .. } =
            &mut wrong.executable.as_mut().unwrap().origin
        else {
            panic!("actor source origin")
        };
        *handler = usize::MAX;
        assert!(execution.validate_required_source_cursor(&wrong).is_err());
        let mut wrong = first.reference.cursor.clone();
        wrong.scopes.push(ScopedSelection {
            owner: span,
            index: 0,
            bound_type: TypeInterner::INT64,
            selection: CheckedComptimeTypeSelection::Unconditional,
            reflection: ReflectionTypeInfo {
                type_name: "int64".into(),
                kind: "primitive".into(),
                primitive_tag: Some("int64".into()),
                has_secret: false,
                args: Vec::new(),
            },
        });
        assert!(execution.validate_required_source_cursor(&wrong).is_err());
        let mut base = first.reference.cursor.clone();
        base.executable.as_mut().unwrap().path.clear();
        let Stmt::Respond(response) = &original_actor.handlers[0].body.stmts[1] else {
            panic!("original runtime response")
        };
        assert!(
            execution
                .required_expression(base.clone(), &response.value, Vec::new())
                .is_err()
        );
        let mut wrong = first.reference.cursor.clone();
        *wrong.executable.as_mut().unwrap().path.last_mut().unwrap() =
            RegionStep::Expression(usize::MAX);
        assert!(execution.validate_required_source_cursor(&wrong).is_err());
        assert_eq!(
            execution.install_function_body(&first.reference),
            Err(ResourceExecutionError::WrongPurpose)
        );
        assert_eq!(
            execution.validate_executable_cursor(&base),
            Err(ResourceExecutionError::WrongPurpose)
        );
        assert!(execution.cursor().executable.is_none());
        execution.replace_purpose(ExecutionPurpose::ReferenceRuntime);
        assert_eq!(
            execution.validate_required_source_cursor(&first.reference.cursor),
            Err(ResourceExecutionError::WrongPurpose)
        );
        assert!(
            execution
                .prepare_explicit_contexts(expression, span, None, &[])
                .is_err()
        );
        assert!(execution.cursor().executable.is_none());
    }
}

#[test]
fn actor_required_closedness_rejects_free_state_and_capability() {
    const FREE_STATE: &str = "namespace app
actor Worker:
    mutable int64 count = 0
    receive read responds int64:
        int64 value = comptime count
        respond value
";
    const FREE_CAPABILITY: &str = "namespace app
function read(view net: Network) returns int64:
    return 5
actor Worker(net: Network):
    receive value responds int64:
        int64 answer = comptime read(view net)
        respond answer
";
    for release in [false, true] {
        let state = checked(FREE_STATE, release);
        let evaluated = required(&state);
        assert_eq!(
            evaluated.diagnostics.len(),
            1,
            "{:?}",
            evaluated.diagnostics
        );
        assert_eq!(evaluated.diagnostics[0].code.code(), 9001);
        assert!(
            evaluated.diagnostics[0]
                .message
                .contains("must be closed and evaluable")
        );
        assert!(evaluated.diagnostics[0].message.contains("count"));
        assert!(evaluated.values.is_empty());
        assert!(evaluated.debug_events.is_empty());
        let parsed = jett_parser::parse(FREE_CAPABILITY, FileId::new(0));
        assert!(
            !parsed
                .errors
                .iter()
                .any(|diagnostic| diagnostic.severity == Severity::Error),
            "{:?}",
            parsed.errors
        );
        let rejected = CheckedResourceProgram::prepare(
            parsed,
            HashMap::from([(FileId::new(0), SourceOrigin::Project)]),
            &[],
            CheckOptions { release },
        )
        .expect_err("comptime cannot call a capability-requiring function");
        assert!(
            rejected.diagnostics().is_some_and(|diagnostics| diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code.code() == 504)),
            "{:?}",
            rejected.diagnostics()
        );
    }
}

#[test]
fn actor_required_scopes_select_handler_facts() {
    const SOURCE: &str = "namespace app
actor Worker:
    receive read responds string:
        comptime type Bound = type.info[int64]():
            string value = comptime type.name[Bound]()
            respond value
function main() returns nothing:
    Worker worker = spawn Worker()
    string value = ask worker.read
    return nothing
";
    for release in [false, true] {
        let program = checked(SOURCE, release);
        let evaluated = required(&program);
        assert!(
            evaluated.diagnostics.is_empty(),
            "{:?}",
            evaluated.diagnostics
        );
        assert!(evaluated.debug_events.is_empty());
        let proofs = evaluated.values.checked_required_values(&program).unwrap();
        let [proof] = proofs.as_slice() else {
            panic!("one scoped actor required value")
        };
        assert_eq!(proof.value(), &crate::Value::String("int64".into()));
        assert_eq!(proof.scoped_bindings().len(), 1);
        assert_eq!(proof.scoped_bindings()[0].name(), "Bound");
        assert_eq!(proof.scoped_bindings()[0].bound_type(), TypeInterner::INT64);
        assert!(proof.generic_index().is_none());
        assert!(matches!(
            proof.owner(),
            CheckedRequiredOwner::ActorHandler { .. }
        ));
    }
}

#[test]
fn actor_required_returned_closure_rejects_unavailable_actor_state_capture() {
    const SOURCE: &str = include_str!("../../../fixtures/actor_required_free_capture.jett");
    for release in [false, true] {
        let program = checked(SOURCE, release);
        let evaluated = required(&program);
        assert_eq!(
            evaluated.diagnostics.len(),
            1,
            "{:?}",
            evaluated.diagnostics
        );
        assert_eq!(evaluated.diagnostics[0].code.code(), 9001);
        assert!(evaluated.diagnostics[0].message.contains("count"));
        assert!(
            evaluated.values.is_empty(),
            "unclosed closure must not be reusable"
        );
        assert!(
            evaluated
                .values
                .checked_required_values(&program)
                .unwrap()
                .is_empty()
        );
        assert!(evaluated.debug_events.is_empty());
    }
}

#[test]
fn actor_required_closed_closures_keep_named_constant_local_parameter_and_nested_captures() {
    const SOURCE: &str = include_str!("../../../fixtures/actor_required_closed_captures.jett");
    for release in [false, true] {
        let program = checked(SOURCE, release);
        let evaluated = required(&program);
        assert!(
            evaluated.diagnostics.is_empty(),
            "{:?}",
            evaluated.diagnostics
        );
        assert!(evaluated.debug_events.is_empty());
        let proofs = evaluated.values.checked_required_values(&program).unwrap();
        let actor_values = proofs
            .iter()
            .filter(|proof| matches!(proof.owner(), CheckedRequiredOwner::ActorHandler { .. }))
            .collect::<Vec<_>>();
        assert_eq!(actor_values.len(), 4);
        for handler in 0..4 {
            let proof = actor_values
                .iter()
                .find(|proof| {
                    matches!(proof.owner(), CheckedRequiredOwner::ActorHandler { target, .. }
                    if target.handler_index == handler)
                })
                .unwrap();
            match handler {
                0 => {
                    let crate::Value::Function {
                        captures, params, ..
                    } = proof.value().payload()
                    else {
                        panic!("own-parameter closure must remain a real checked value");
                    };
                    assert_eq!(params.len(), 1);
                    assert_eq!(params[0].name.name, "delta");
                    assert!(!captures.contains_key("delta"));
                    assert!(!captures.contains_key("local"));
                    assert!(!captures.contains_key("bias"));
                    assert!(!captures.contains_key("app.bias"));
                    assert!(!captures.contains_key("five"));
                    assert!(!captures.contains_key("app.five"));
                }
                1 | 2 => {
                    let crate::Value::Function { captures, .. } = proof.value().payload() else {
                        panic!("pure worker must retain the actual available captures");
                    };
                    let seed = if handler == 1 { 3 } else { 4 };
                    assert_eq!(
                        captures.get("seed").map(crate::Value::payload),
                        Some(&crate::Value::Int64(seed))
                    );
                    assert_eq!(
                        captures.get("local").map(crate::Value::payload),
                        Some(&crate::Value::Int64(seed + 1))
                    );
                }
                3 => assert_eq!(proof.value().payload(), &crate::Value::Int64(29)),
                _ => unreachable!(),
            }
        }
    }
}

#[test]
fn actor_required_free_captures_refuse_same_name_constant_parameter_and_nested_body() {
    const SOURCES: &[(&str, &str)] = &[
        (
            include_str!("../../../fixtures/actor_required_free_capture_same_constant.jett"),
            "count",
        ),
        (
            include_str!("../../../fixtures/actor_required_free_parameter_capture.jett"),
            "value",
        ),
        (
            include_str!("../../../fixtures/actor_required_nested_free_capture.jett"),
            "count",
        ),
    ];
    for release in [false, true] {
        for (source, missing) in SOURCES {
            let program = checked(source, release);
            let evaluated = required(&program);
            assert_eq!(
                evaluated.diagnostics.len(),
                1,
                "{:?}",
                evaluated.diagnostics
            );
            assert_eq!(evaluated.diagnostics[0].code.code(), 9001);
            assert!(evaluated.diagnostics[0].message.contains(missing));
            assert!(evaluated.debug_events.is_empty());
            // The same-name namespace constant may have its own valid primitive
            // entry. It must never manufacture a successful actor closure entry.
            let proofs = evaluated.values.checked_required_values(&program).unwrap();
            assert!(proofs.iter().all(|proof| {
                !matches!(proof.owner(), CheckedRequiredOwner::ActorHandler { .. })
            }));
        }
    }
}
