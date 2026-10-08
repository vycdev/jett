use super::*;
use crate::resource_execution::tests::program;

const CONSTRUCT: &str = include_str!("../../../fixtures/18_pipeline_construct_move_close.jett");
const RETAIN: &str = include_str!("../../../fixtures/19_pipeline_written_view_retains_owner.jett");
const ABORT: &str = include_str!("../../../fixtures/21_pipeline_named_actual_abort.jett");

fn scenario(program: &CheckedResourceProgram) -> DefId {
    let functions = program
        .module()
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Function(function)
                if function.name.span.file == jett_common::FileId::new(0)
                    && function.name.name == "scenario" =>
            {
                Some(function)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [function] = functions.as_slice() else {
        panic!("one original source scenario");
    };
    let definitions = program
        .resolved()
        .scope_table
        .definitions
        .iter()
        .filter(|info| {
            info.kind == jett_resolve::DefKind::Function
                && info.span == function.name.span
                && info.namespace.as_deref() == Some("app")
        })
        .map(|info| info.id)
        .collect::<Vec<_>>();
    let [definition] = definitions.as_slice() else {
        panic!("one exact primary declaration");
    };
    *definition
}

fn pipelines(reference: &CheckedBodyReference) -> Vec<&Expr> {
    let mut found = Vec::new();
    walk_block(reference.block().unwrap(), &mut |_| {}, &mut |expression| {
        if matches!(expression, Expr::Pipeline(..)) {
            found.push(expression);
        }
    });
    found
}

#[test]
fn resource_pipeline_reference_joins_original_step_inputs_outputs_and_source_permutation() {
    for release in [false, true] {
        for source in [CONSTRUCT, ABORT] {
            let program = program(source, release);
            let mut checked =
                CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
            let entry = checked.entry(scenario(&program)).unwrap();
            let reference = checked.prepare_function_body(&entry).unwrap();
            checked.install_function_body(&reference).unwrap();
            for pipeline in pipelines(&reference) {
                let Expr::Pipeline(initial, steps, _) = pipeline else {
                    unreachable!();
                };
                for index in 0..steps.len() {
                    let prepared = checked.prepare_pipeline_step(pipeline, index).unwrap();
                    let input_span = if index == 0 {
                        initial.span()
                    } else {
                        steps[index - 1].span
                    };
                    assert_eq!(prepared.invocation().arguments()[0].source_span, input_span);
                    let facts = checked.facts(&checked.body).unwrap();
                    assert_eq!(
                        facts.pipeline_inputs[&steps[index].span],
                        prepared.input_type()
                    );
                    assert_eq!(
                        facts.pipeline_calls[&steps[index].span],
                        prepared.raw_call_type()
                    );
                    assert_eq!(
                        facts.source_types[&steps[index].span],
                        prepared.output_type()
                    );
                    if index > 0 {
                        assert_eq!(
                            prepared.invocation().arguments()[0].origin,
                            CheckedCallerOrigin::OwnedExpression
                        );
                    }
                    if source == ABORT {
                        assert_eq!(
                            prepared
                                .invocation()
                                .arguments()
                                .iter()
                                .map(|argument| (argument.source_index, argument.parameter_index))
                                .collect::<Vec<_>>(),
                            vec![(0, 0), (1, 2), (2, 1)]
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn resource_pipeline_reference_refuses_foreign_clones_missing_body_and_invalid_step_index() {
    for release in [false, true] {
        let program = program(RETAIN, release);
        let mut checked =
            CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
        let entry = checked.entry(scenario(&program)).unwrap();
        let reference = checked.prepare_function_body(&entry).unwrap();
        checked.install_function_body(&reference).unwrap();
        let found = pipelines(&reference);
        let [original] = found.as_slice() else {
            panic!("one exact pipeline source");
        };
        let prepared = checked.prepare_pipeline_step(original, 0).unwrap();
        assert_eq!(
            prepared.invocation().arguments()[0].syntax,
            CheckedCallerSyntax::WrittenView
        );
        assert_eq!(
            prepared.invocation().arguments()[0].effect,
            CheckedCallerEffect::RetainBorrow
        );
        let CheckedCallerOrigin::Binding(binding) = &prepared.invocation().arguments()[0].origin
        else {
            panic!("exact original root binding");
        };
        assert_eq!(prepared.initial_borrow(), Some(binding.definition));
        let other_program = crate::resource_execution::tests::program(RETAIN, release);
        let mut foreign =
            CheckedExecution::new(other_program, ExecutionPurpose::ReferenceRuntime).unwrap();
        assert!(matches!(
            foreign.install_function_body(&reference),
            Err(ResourceExecutionError::ForeignProgram)
        ));
        let cloned = (**original).clone();
        assert!(matches!(
            checked.prepare_pipeline_step(&cloned, 0),
            Err(ResourceExecutionError::MissingCheckedBody)
        ));
        assert!(matches!(
            checked.prepare_pipeline_step(original, usize::MAX),
            Err(ResourceExecutionError::MissingCheckedInvocation)
        ));
        let fabricated = Expr::Call(
            Box::new(prepared.callee().clone()),
            prepared.extra_arguments().to_vec(),
            prepared.original().span,
        );
        assert!(matches!(
            checked.prepare_pipeline_step(&fabricated, 0),
            Err(ResourceExecutionError::MissingCheckedBody)
        ));
        checked.enter_ordinary();
        assert!(matches!(
            checked.prepare_pipeline_step(original, 0),
            Err(ResourceExecutionError::MissingCheckedBody)
        ));
    }
}

const REQUIRED_PIPELINES: &str = include_str!("../../../fixtures/28_pipeline_absence_regions.jett");

fn region_pipelines(reference: &CheckedBodyReference) -> Vec<&Expr> {
    let mut found = Vec::new();
    walk_region(
        reference.region().unwrap(),
        &mut |_| {},
        &mut |expression| {
            if matches!(expression, Expr::Pipeline(..)) {
                found.push(expression);
            }
        },
        &mut |_| {},
    );
    found
}

fn verify_region_steps(checked: &CheckedExecution, reference: &CheckedBodyReference) {
    let found = region_pipelines(reference);
    assert_eq!(found.len(), 1);
    let original = found[0];
    let Expr::Pipeline(initial, steps, _) = original else {
        unreachable!();
    };
    assert_eq!(steps.len(), 2);
    for index in 0..steps.len() {
        let prepared = checked.prepare_pipeline_step(original, index).unwrap();
        let facts = checked.facts(&checked.body).unwrap();
        assert_eq!(
            prepared.invocation().arguments()[0].source_span,
            if index == 0 {
                initial.span()
            } else {
                steps[index - 1].span
            }
        );
        assert_eq!(
            prepared.input_type(),
            facts.pipeline_inputs[&steps[index].span]
        );
        assert_eq!(
            prepared.raw_call_type(),
            facts.pipeline_calls[&steps[index].span]
        );
        assert_eq!(
            prepared.output_type(),
            facts.source_types[&steps[index].span]
        );
    }
    let cloned = (*original).clone();
    assert!(matches!(
        checked.prepare_pipeline_step(&cloned, 0),
        Err(ResourceExecutionError::MissingCheckedBody)
    ));
}

#[test]
fn resource_pipeline_reference_authenticates_namespace_and_explicit_expression_regions() {
    for release in [false, true] {
        let program = program(REQUIRED_PIPELINES, release);
        let mut checked =
            CheckedExecution::new(program.clone(), ExecutionPurpose::NamespaceConstant).unwrap();
        let declaration = program
            .module()
            .items
            .iter()
            .find_map(|item| match item {
                Item::VarDecl(value)
                    if value.name.name == "namespace_marker"
                        && value.name.span.file == jett_common::FileId::new(0) =>
                {
                    Some(value)
                }
                _ => None,
            })
            .unwrap();
        let initializer = checked.prepare_namespace_initializer(declaration).unwrap();
        let reference = initializer.reference();
        assert!(matches!(
            reference.region().unwrap(),
            OriginalRegion::Expression(_)
        ));
        checked.install_function_body(reference).unwrap();
        verify_region_steps(&checked, reference);
        let namespace_pipeline = region_pipelines(reference)[0];

        checked.replace_purpose(ExecutionPurpose::ExplicitComptime);
        let function = program
            .module()
            .items
            .iter()
            .find_map(|item| match item {
                Item::Function(value) if value.name.name == "explicit_marker" => Some(value),
                _ => None,
            })
            .unwrap();
        let returned = function
            .body
            .stmts
            .iter()
            .find_map(|statement| match statement {
                Stmt::Return(value) => value.value.as_ref(),
                _ => None,
            })
            .unwrap();
        let Expr::Comptime(expression, span) = returned else {
            panic!("original required expression");
        };
        let contexts = checked
            .prepare_explicit_contexts(expression, *span, Some(function.name.span), &[])
            .unwrap();
        let [context] = contexts.as_slice() else {
            panic!("one exact ordinary required context");
        };
        let explicit = context.reference();
        assert!(matches!(
            explicit.region().unwrap(),
            OriginalRegion::Expression(_)
        ));
        checked.install_function_body(explicit).unwrap();
        verify_region_steps(&checked, explicit);
        assert!(matches!(
            checked.prepare_pipeline_step(namespace_pipeline, 0),
            Err(ResourceExecutionError::MissingCheckedBody)
        ));
        let mut foreign = CheckedExecution::new(
            crate::resource_execution::tests::program(REQUIRED_PIPELINES, release),
            ExecutionPurpose::ExplicitComptime,
        )
        .unwrap();
        assert!(matches!(
            foreign.install_function_body(explicit),
            Err(ResourceExecutionError::ForeignProgram)
        ));
    }
}

#[test]
fn resource_pipeline_reference_authenticates_verify_and_property_original_regions() {
    for release in [false, true] {
        let program = program(REQUIRED_PIPELINES, release);
        let mut checked = CheckedExecution::new(program.clone(), ExecutionPurpose::Verify).unwrap();
        let verify = program
            .module()
            .items
            .iter()
            .find_map(|item| match item {
                Item::Verify(value) => Some(value),
                _ => None,
            })
            .unwrap();
        let reference = checked.prepare_verify_body(verify).unwrap();
        checked.install_function_body(&reference).unwrap();
        verify_region_steps(&checked, &reference);
        let verify_pipeline = region_pipelines(&reference)[0];
        checked.replace_purpose(ExecutionPurpose::Property);
        let property = program
            .module()
            .items
            .iter()
            .find_map(|item| match item {
                Item::Property(value) => Some(value),
                _ => None,
            })
            .unwrap();
        let property_reference = checked.prepare_property_body(property).unwrap();
        checked.install_function_body(&property_reference).unwrap();
        verify_region_steps(&checked, &property_reference);
        assert!(matches!(
            checked.prepare_pipeline_step(verify_pipeline, 0),
            Err(ResourceExecutionError::MissingCheckedBody)
        ));
    }
}
