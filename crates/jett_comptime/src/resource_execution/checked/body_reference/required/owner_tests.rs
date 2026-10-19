use super::*;
use crate::checked_types::CheckedExpressionTypes;
use crate::resource_execution::tests::program as checked_program;
use crate::{Interpreter, Value};

const SOURCE: &str = "namespace app
function factory[T]() returns function() returns string:
    return function() returns string: return comptime type.name[T]()
function scoped[T]() returns function() returns string:
    comptime type Bound = type.info[T]():
        return function() returns string: return comptime type.name[Bound]()
type Positive = int64 where value > 0 and (comptime type.name[int64]()) == \"int64\"
export function scenario() returns nothing:
    function() returns string first = factory[int8]()
    function() returns string second = factory[string]()
    function() returns string third = scoped[int8]()
    function() returns string fourth = scoped[string]()
    return nothing
";

fn function<'a>(program: &'a CheckedResourceProgram, name: &str) -> &'a FunctionDef {
    program
        .module()
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(value) if value.name.name == name => Some(value),
            _ => None,
        })
        .unwrap()
}

fn inline_comptime(block: &Block) -> (&Expr, &Expr, Span) {
    let Some(Stmt::Return(ret)) = block.stmts.first() else {
        panic!("original inline return")
    };
    let Some(inline @ Expr::InlineFn(_, _, body, _)) = ret.value.as_ref() else {
        panic!("original inline body")
    };
    let Some(Stmt::Return(ret)) = body.stmts.first() else {
        panic!("original required return")
    };
    let Some(Expr::Comptime(expression, occurrence)) = ret.value.as_ref() else {
        panic!("original required occurrence")
    };
    (inline, expression, *occurrence)
}

fn expression_types(program: &Arc<CheckedResourceProgram>) -> Arc<CheckedExpressionTypes> {
    Arc::new(CheckedExpressionTypes {
        resource_program: Some(program.clone()),
        expressions: program
            .checked()
            .type_map
            .iter()
            .map(|(span, ty)| (*span, program.checked().interner.type_name(*ty)))
            .collect(),
        ..Default::default()
    })
}

#[test]
fn required_inline_owner_keeps_original_node_concrete_generic_and_scope_keys() {
    for release in [false, true] {
        let program = checked_program(SOURCE, release);
        let checked =
            CheckedExecution::new(program.clone(), ExecutionPurpose::ExplicitComptime).unwrap();
        let factory = function(&program, "factory");
        let (inline, expression, occurrence) = inline_comptime(&factory.body);
        let contexts = checked
            .prepare_explicit_contexts(expression, occurrence, Some(factory.name.span), &[])
            .unwrap();
        assert_eq!(contexts.len(), 2);
        assert_ne!(contexts[0].key(), contexts[1].key());
        let mut enclosing_key = contexts[0].key();
        enclosing_key.node.path.clear();
        assert!(!contexts[0].key().is_lexical_descendant_of(&enclosing_key));
        assert!(
            contexts[0]
                .key()
                .is_lexical_descendant_of(&contexts[0].key())
        );
        assert!(
            !contexts[0]
                .key()
                .is_lexical_descendant_of(&contexts[1].key())
        );
        for context in &contexts {
            assert!(std::ptr::eq(context.expression().unwrap(), expression));
            assert_eq!(context.reference.cursor.scopes.len(), 0);
            let parent = context.reference.cursor.executable.as_ref().unwrap();
            let Some(RegionStep::InlineBody(index)) = parent.path.first() else {
                panic!("private inline region step")
            };
            let root = CheckedBodyReference {
                cursor: CheckedBodyCursor {
                    root: context.reference.cursor.root.clone(),
                    scopes: Vec::new(),
                    executable: Some(ExecutableIdentity {
                        program: program.clone(),
                        origin: parent.origin,
                        path: Vec::new(),
                    }),
                },
            };
            assert!(std::ptr::eq(
                region_expression(root.region().unwrap(), *index).unwrap(),
                inline
            ));
        }
        let scoped = function(&program, "scoped");
        let Some(Stmt::ComptimeTypeBind(binding)) = scoped.body.stmts.first() else {
            panic!("original scope")
        };
        let (_, expression, occurrence) = inline_comptime(&binding.body);
        let scoped_contexts = checked
            .prepare_explicit_contexts(
                expression,
                occurrence,
                Some(scoped.name.span),
                &[(binding.span, binding.name.name.clone())],
            )
            .unwrap();
        assert_eq!(scoped_contexts.len(), 2);
        assert_ne!(scoped_contexts[0].key(), scoped_contexts[1].key());
        for context in scoped_contexts {
            assert_eq!(context.bindings.len(), 1);
            assert!(matches!(
                context
                    .reference
                    .cursor
                    .executable
                    .as_ref()
                    .unwrap()
                    .path
                    .as_slice(),
                [
                    RegionStep::Scope(_),
                    RegionStep::InlineBody(_),
                    RegionStep::Expression(_)
                ]
            ));
        }
        let required = crate::evaluate_explicit_comptime_expressions_capture(
            program.module(),
            Arc::new(jett_types::ReflectionMetadata::new()),
            expression_types(&program),
            Arc::new(HashMap::new()),
        );
        assert!(
            required.diagnostics.is_empty(),
            "{:?}",
            required.diagnostics
        );
        let proofs = required.values.checked_required_values(&program).unwrap();
        let inline_proofs = proofs
            .iter()
            .filter(|proof| proof.original_inline_function().unwrap().is_some())
            .collect::<Vec<_>>();
        assert_eq!(inline_proofs.len(), 4);
        assert!(inline_proofs.iter().all(|proof| matches!(proof.value(), Value::String(value) if value == "int8" || value == "string")));
    }
}

#[test]
fn required_inline_owner_refuses_clones_foreign_owner_generic_and_runtime_entry() {
    for release in [false, true] {
        let program = checked_program(SOURCE, release);
        let mut checked =
            CheckedExecution::new(program.clone(), ExecutionPurpose::ExplicitComptime).unwrap();
        let factory = function(&program, "factory");
        let (_, expression, occurrence) = inline_comptime(&factory.body);
        let contexts = checked
            .prepare_explicit_contexts(expression, occurrence, Some(factory.name.span), &[])
            .unwrap();
        let original_cursor = checked.cursor();
        assert!(matches!(
            checked.prepare_explicit_contexts(
                &expression.clone(),
                occurrence,
                Some(factory.name.span),
                &[]
            ),
            Err(ResourceExecutionError::MissingCheckedBody)
        ));
        assert!(matches!(
            checked.prepare_explicit_contexts(
                expression,
                occurrence,
                Some(function(&program, "scenario").name.span),
                &[]
            ),
            Err(ResourceExecutionError::MissingCheckedBody)
        ));
        let foreign = CheckedExecution::new(
            checked_program(SOURCE, release),
            ExecutionPurpose::ExplicitComptime,
        )
        .unwrap();
        assert!(matches!(
            foreign.prepare_explicit_contexts(expression, occurrence, Some(factory.name.span), &[]),
            Err(ResourceExecutionError::MissingCheckedBody)
        ));
        assert_eq!(
            foreign.validate_executable_cursor(&contexts[0].reference.cursor),
            Err(ResourceExecutionError::ForeignProgram)
        );
        let mut wrong = contexts[0].reference.cursor.clone();
        wrong.executable.as_mut().unwrap().path[0] = RegionStep::InlineBody(usize::MAX);
        assert_eq!(
            checked.validate_executable_cursor(&wrong),
            Err(ResourceExecutionError::MissingCheckedBody)
        );
        let other_definition = checked
            .declaration_definition(function(&program, "scoped").name.span)
            .unwrap();
        let other_index = program
            .checked()
            .generic_function_instantiations
            .iter()
            .position(|instance| instance.definition == other_definition)
            .unwrap();
        let mut wrong = contexts[0].reference.cursor.clone();
        wrong.root = BodyRoot::Generic(other_index);
        assert_eq!(
            checked.validate_executable_cursor(&wrong),
            Err(ResourceExecutionError::MissingCheckedBody)
        );
        assert_eq!(
            checked.attempt_key(&original_cursor).err(),
            Some(ResourceExecutionError::MissingCheckedBody)
        );
        assert!(checked.cursor().executable.is_none());
        checked.replace_purpose(ExecutionPurpose::ReferenceRuntime);
        assert_eq!(
            checked.validate_executable_cursor(&contexts[0].reference.cursor),
            Err(ResourceExecutionError::WrongPurpose)
        );
        assert!(matches!(
            checked.prepare_explicit_contexts(expression, occurrence, Some(factory.name.span), &[]),
            Err(ResourceExecutionError::WrongPurpose)
        ));
    }
}

#[test]
fn required_refinement_owner_is_exact_retained_constraint_and_provider_free() {
    for release in [false, true] {
        let program = checked_program(SOURCE, release);
        let mut checked =
            CheckedExecution::new(program.clone(), ExecutionPurpose::ExplicitComptime).unwrap();
        let alias = program
            .module()
            .items
            .iter()
            .find_map(|item| match item {
                Item::TypeAlias(alias) if alias.name.name == "Positive" => Some(alias),
                _ => None,
            })
            .unwrap();
        let mut occurrence = None;
        walk_region(
            OriginalRegion::Expression(alias.constraint.as_ref().unwrap()),
            &mut |_| {},
            &mut |node| {
                if let Expr::Comptime(expression, span) = node {
                    occurrence = Some((expression.as_ref(), *span));
                }
            },
            &mut |_| {},
        );
        let (expression, span) = occurrence.unwrap();
        let contexts = checked
            .prepare_explicit_contexts(expression, span, None, &[])
            .unwrap();
        assert_eq!(contexts.len(), 1);
        assert!(std::ptr::eq(contexts[0].expression().unwrap(), expression));
        assert!(matches!(
            checked.prepare_explicit_contexts(&expression.clone(), span, None, &[]),
            Err(ResourceExecutionError::MissingCheckedBody)
        ));
        let required = crate::evaluate_explicit_comptime_expressions_capture(
            program.module(),
            Arc::new(jett_types::ReflectionMetadata::new()),
            expression_types(&program),
            Arc::new(HashMap::new()),
        );
        assert!(
            required.diagnostics.is_empty(),
            "{:?}",
            required.diagnostics
        );
        let proof = required
            .values
            .checked_required_values(&program)
            .unwrap()
            .into_iter()
            .find(|proof| proof.source_span() == span)
            .unwrap();
        assert_eq!(
            proof.owner(),
            CheckedRequiredOwner::RefinementPredicate {
                definition: checked.refinement_definition(alias).unwrap(),
                declaration: alias.name.span
            }
        );
        assert_eq!(proof.value(), &Value::String("int64".into()));
        assert!(proof.original_inline_function().unwrap().is_none());
        let mut wrong = contexts[0].reference.cursor.clone();
        let ExecutableOrigin::RefinementPredicate { definition, .. } =
            &mut wrong.executable.as_mut().unwrap().origin
        else {
            panic!("retained refinement owner")
        };
        *definition = checked
            .declaration_definition(function(&program, "scenario").name.span)
            .unwrap();
        assert_eq!(
            checked.validate_executable_cursor(&wrong),
            Err(ResourceExecutionError::MissingCheckedBody)
        );
        checked.replace_purpose(ExecutionPurpose::Verify);
        assert_eq!(
            checked.validate_executable_cursor(&contexts[0].reference.cursor),
            Err(ResourceExecutionError::WrongPurpose)
        );
        assert!(checked.cursor().executable.is_none());
    }
}

#[test]
fn required_inline_scoped_attempt_restores_metadata_before_foreign_refusal_and_reentry() {
    const FAILURE_SOURCE: &str = "function required_failure[T]() returns nothing:
    string bound = type.name[T]()
    resource_probe.terminal_failure()
    return nothing
function scoped_failure[T]() returns function() returns nothing:
    comptime type Bound = type.info[T]():
        return function() returns nothing: return comptime required_failure[Bound]()
function select_failure_contexts() returns nothing:
    function() returns nothing first = scoped_failure[int8]()
    function() returns nothing second = scoped_failure[string]()
    return nothing
";
    let source = format!("{SOURCE}{FAILURE_SOURCE}");
    for release in [false, true] {
        let program = checked_program(&source, release);
        let checked =
            CheckedExecution::new(program.clone(), ExecutionPurpose::ExplicitComptime).unwrap();
        let scoped = function(&program, "scoped");
        let Some(Stmt::ComptimeTypeBind(binding)) = scoped.body.stmts.first() else {
            panic!("original scope")
        };
        let (_, expression, occurrence) = inline_comptime(&binding.body);
        let contexts = checked
            .prepare_explicit_contexts(
                expression,
                occurrence,
                Some(scoped.name.span),
                &[(binding.span, binding.name.name.clone())],
            )
            .unwrap();
        let mut interpreter = Interpreter::from_checked_resource_program(
            program.clone(),
            ExecutionPurpose::ExplicitComptime,
        )
        .unwrap();
        interpreter
            .authorize_checked_resource_worker(program.module(), ExecutionPurpose::ExplicitComptime)
            .unwrap();
        let baseline = interpreter.resource_test_entry_context().unwrap();
        let foreign_program = checked_program(&source, release);
        let foreign =
            CheckedExecution::new(foreign_program.clone(), ExecutionPurpose::ExplicitComptime)
                .unwrap();
        let foreign_function = function(&foreign_program, "scoped");
        let Some(Stmt::ComptimeTypeBind(foreign_binding)) = foreign_function.body.stmts.first()
        else {
            panic!("foreign scope")
        };
        let (_, foreign_expression, foreign_occurrence) = inline_comptime(&foreign_binding.body);
        let foreign_context = foreign
            .prepare_explicit_contexts(
                foreign_expression,
                foreign_occurrence,
                Some(foreign_function.name.span),
                &[(foreign_binding.span, foreign_binding.name.name.clone())],
            )
            .unwrap();
        assert!(
            interpreter
                .eval_checked_required_expression(
                    &foreign_context[0],
                    Some("app"),
                    &HashMap::new(),
                    &["T".into()]
                )
                .is_err()
        );
        assert_eq!(interpreter.resource_test_entry_context().unwrap(), baseline);
        let failing = function(&program, "scoped_failure");
        let Some(Stmt::ComptimeTypeBind(failure_binding)) = failing.body.stmts.first() else {
            panic!("original failing scope")
        };
        let (_, failure_expression, failure_occurrence) = inline_comptime(&failure_binding.body);
        let failure_contexts = checked
            .prepare_explicit_contexts(
                failure_expression,
                failure_occurrence,
                Some(failing.name.span),
                &[(failure_binding.span, failure_binding.name.name.clone())],
            )
            .unwrap();
        assert_eq!(failure_contexts.len(), contexts.len());
        let mut observed = Vec::new();
        for (failure, context) in failure_contexts.iter().zip(&contexts) {
            assert_eq!(failure.bindings.len(), 1);
            assert!(failure.function.is_some());
            assert!(failure.scope.is_some());
            assert!(Arc::ptr_eq(failure.checked_program(), &program));
            assert!(std::ptr::eq(
                failure.expression().unwrap(),
                failure_expression
            ));
            assert!(matches!(
                failure
                    .reference
                    .cursor
                    .executable
                    .as_ref()
                    .unwrap()
                    .path
                    .as_slice(),
                [
                    RegionStep::Scope(_),
                    RegionStep::InlineBody(_),
                    RegionStep::Expression(_)
                ]
            ));
            let error = interpreter
                .eval_checked_required_expression(
                    failure,
                    Some("app"),
                    &HashMap::new(),
                    &["T".into()],
                )
                .unwrap_err();
            // This ordinary Bounds error is produced only after the original
            // required region and its typed generic/scoped metadata are entered.
            assert_eq!(error, "list.__remove_at: index -1 out of bounds");
            assert_eq!(interpreter.resource_test_entry_context().unwrap(), baseline);
            assert_eq!(interpreter.resource_test_custody_counts().unwrap(), (0, 0));
            assert!(interpreter.take_debug_events().is_empty());
            observed.push(
                interpreter
                    .eval_checked_required_expression(
                        context,
                        Some("app"),
                        &HashMap::new(),
                        &["T".into()],
                    )
                    .unwrap(),
            );
            assert_eq!(interpreter.resource_test_entry_context().unwrap(), baseline);
            assert_eq!(interpreter.resource_test_custody_counts().unwrap(), (0, 0));
            assert!(interpreter.take_debug_events().is_empty());
        }
        observed.sort_by_key(|value| match value {
            Value::String(value) => value.clone(),
            _ => panic!("required string"),
        });
        assert_eq!(
            observed,
            vec![Value::String("int8".into()), Value::String("string".into())]
        );
    }
}
