use super::*;
use crate::resource_execution::tests::program as checked_program;
use crate::{Interpreter, Value};
use jett_types::{ReflectionMetadata, TypeInterner};

const SOURCE: &str = r#"namespace app
type Label = int64
function label[T]() returns string:
    return type.name[T]()
export function integer_label() returns string:
    return label[int64]()
export function string_label() returns string:
    return label[string]()
export function alias_label() returns string:
    return label[Label]()
export function alias_kind() returns string:
    return type.kind[Label]()
export function alias_info() returns string:
    return type.info[Label]().type_name
export function scoped_label() returns string:
    comptime type Outer = type.info[list[int64]]():
        comptime type Element = type.arg[Outer](0):
            return type.name[Element]()
    return "unreached"
export function pipeline_label() returns string:
    return (0 into type.arg[list[int64]]()).type_name
export function secret_flag() returns bool:
    return type.has_secret[secret[int64]]()
export function list_label() returns string:
    return type.name[list[int64]]()
"#;

fn function<'a>(program: &'a Arc<CheckedResourceProgram>, name: &str) -> &'a FunctionDef {
    program
        .module()
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function)
                if function.name.name == name
                    && function.name.span.file == jett_common::FileId::new(0) =>
            {
                Some(function)
            }
            _ => None,
        })
        .expect("one retained fixture function")
}

fn entry(checked: &mut CheckedExecution, program: &Arc<CheckedResourceProgram>, name: &str) {
    let function = function(program, name);
    let definition = checked.declaration_definition(function.name.span).unwrap();
    let signature = program.checked().definition_types[&definition];
    let reference = checked
        .prepare_function_body(&FunctionInvocation::Entry {
            definition,
            signature,
        })
        .unwrap();
    checked.install_function_body(&reference).unwrap();
}

fn first_call(function: &FunctionDef) -> &Expr {
    let mut found = None;
    walk_block(&function.body, &mut |_| {}, &mut |expression| {
        if found.is_none() && matches!(expression, Expr::Call(..) | Expr::GenericCall(..)) {
            found = Some(expression);
        }
    });
    found.expect("actual original call")
}

fn parts(call: &Expr) -> (&Expr, &[TypeExpr], &[CallArg], Span) {
    match call {
        Expr::Call(callee, arguments, span) => (callee.as_ref(), &[], arguments, *span),
        Expr::GenericCall(callee, types, arguments, span) => {
            (callee.as_ref(), types, arguments, *span)
        }
        _ => panic!("original source call"),
    }
}

fn generic_invocation(
    checked: &mut CheckedExecution,
    program: &Arc<CheckedResourceProgram>,
    wrapper: &str,
) -> CheckedInvocation {
    entry(checked, program, wrapper);
    let (_, _, _, span) = parts(first_call(function(program, wrapper)));
    let caller = checked.invocation(span).unwrap();
    let body = checked
        .prepare_function_body(&FunctionInvocation::Source(&caller))
        .unwrap();
    checked.install_function_body(&body).unwrap();
    checked
        .invocation(first_call(function(program, "label")).span())
        .unwrap()
}

#[test]
fn resource_intrinsic_source_calls_preserve_concrete_alias_and_nested_metadata_without_ambient_maps()
 {
    for release in [false, true] {
        let program = checked_program(SOURCE, release);
        let checked =
            CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
        let mut interpreter = Interpreter::from_checked_resource_program(
            program.clone(),
            ExecutionPurpose::ReferenceRuntime,
        )
        .unwrap();
        // Empty legacy reflection metadata cannot stand in for the exact body records.
        interpreter.set_reflection_metadata(Arc::new(ReflectionMetadata::new()));
        for (name, expected) in [
            ("integer_label", Value::String("int64".to_string())),
            ("string_label", Value::String("string".to_string())),
            ("alias_label", Value::String("app.Label".to_string())),
            ("alias_kind", Value::String("alias".to_string())),
            ("alias_info", Value::String("app.Label".to_string())),
            ("scoped_label", Value::String("int64".to_string())),
            ("pipeline_label", Value::String("int64".to_string())),
            ("secret_flag", Value::Bool(true)),
            ("list_label", Value::String("list[int64]".to_string())),
            ("integer_label", Value::String("int64".to_string())),
        ] {
            let definition = checked
                .declaration_definition(function(&program, name).name.span)
                .unwrap();
            assert_eq!(
                interpreter
                    .call_checked_program_entry(definition, Vec::new())
                    .unwrap(),
                expected,
                "{name} release={release}"
            );
            assert_eq!(interpreter.resource_test_custody_counts().unwrap(), (0, 0));
            assert!(interpreter.take_debug_events().is_empty());
        }
    }
}

#[test]
fn resource_intrinsic_packet_rejects_cloned_original_foreign_and_other_concrete_contexts() {
    for release in [false, true] {
        let program = checked_program(SOURCE, release);
        let mut checked =
            CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
        let integer = generic_invocation(&mut checked, &program, "integer_label");
        let (callee, types, arguments, _) = parts(first_call(function(&program, "label")));
        let prepared = checked
            .prepare_intrinsic_arguments(callee, types, arguments, &integer)
            .unwrap();
        assert_eq!(prepared.types(), &[TypeInterner::INT64]);
        assert_eq!(prepared.reflections()[0].type_name, "int64");
        prepared.validate(&checked, &integer).unwrap();
        assert!(
            checked
                .prepare_intrinsic_arguments(callee, &types.to_vec(), arguments, &integer)
                .is_err()
        );
        let clone = first_call(function(&program, "label")).clone();
        let (callee_clone, types_clone, arguments_clone, _) = parts(&clone);
        assert!(
            checked
                .prepare_intrinsic_arguments(callee_clone, types_clone, arguments_clone, &integer)
                .is_err()
        );
        let text = generic_invocation(&mut checked, &program, "string_label");
        assert_eq!(integer.span(), text.span());
        assert_eq!(integer.packet(), text.packet()); // Only the saved concrete body distinguishes this call.
        assert!(
            checked
                .prepare_intrinsic_arguments(callee, types, arguments, &integer)
                .is_err()
        );
        assert!(prepared.validate(&checked, &text).is_err());
        let foreign_program = checked_program(SOURCE, release);
        let mut foreign =
            CheckedExecution::new(foreign_program.clone(), ExecutionPurpose::ReferenceRuntime)
                .unwrap();
        let foreign_call = generic_invocation(&mut foreign, &foreign_program, "integer_label");
        assert_eq!(
            prepared.validate(&foreign, &foreign_call),
            Err(ResourceExecutionError::ForeignProgram)
        );
        assert!(
            foreign
                .prepare_intrinsic_arguments(callee, types, arguments, &integer)
                .is_err()
        );
    }
}

#[test]
fn resource_intrinsic_packet_rejects_missing_records_counts_ids_shapes_and_unresolved_types() {
    for release in [false, true] {
        let program = checked_program(SOURCE, release);
        let mut checked =
            CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
        let invocation = generic_invocation(&mut checked, &program, "integer_label");
        for mutation in 0..7 {
            let original = checked.facts(&invocation.body).unwrap();
            let mut concrete = original.intrinsic_types.clone();
            let mut reflections = original.intrinsic_reflections.clone();
            let mut ids = original.intrinsics.clone();
            let mut calls = original.calls.clone();
            let mut copied = CheckedInvocation {
                packet: invocation.packet.clone(),
                body: invocation.body.clone(),
                span: invocation.span,
            };
            match mutation {
                0 => {
                    concrete.remove(&invocation.span);
                }
                1 => {
                    reflections.remove(&invocation.span);
                }
                2 => {
                    ids.insert(invocation.span, IntrinsicId::TypeKind);
                }
                3 => {
                    concrete.insert(
                        invocation.span,
                        vec![TypeInterner::INT64, TypeInterner::STRING],
                    );
                }
                4 => {
                    let extra = reflections[&invocation.span][0].clone();
                    reflections.get_mut(&invocation.span).unwrap().push(extra);
                }
                5 => {
                    concrete.insert(invocation.span, vec![TypeInterner::ERROR]);
                }
                _ => {
                    let CheckedInvocationShape::Intrinsic { intrinsic, .. } =
                        &mut copied.packet.shape
                    else {
                        unreachable!()
                    };
                    *intrinsic = IntrinsicId::TypeKind;
                    calls.insert(invocation.span, copied.packet.clone());
                }
            }
            let facts = BodyFacts {
                calls: &calls,
                bindings: original.bindings,
                types: original.types,
                source_types: original.source_types,
                pipeline_inputs: original.pipeline_inputs,
                pipeline_calls: original.pipeline_calls,
                scopes: original.scopes,
                intrinsics: &ids,
                intrinsic_types: &concrete,
                intrinsic_reflections: &reflections,
                constructions: original.constructions,
                selections: original.selections,
            };
            assert!(
                intrinsic_records(&facts, &copied, 1, &program.checked().interner).is_err(),
                "mutation {mutation}"
            );
        }
        let (callee, types, arguments, _) = parts(first_call(function(&program, "label")));
        for mutation in 0..3 {
            let mut prepared = checked
                .prepare_intrinsic_arguments(callee, types, arguments, &invocation)
                .unwrap();
            match mutation {
                0 => prepared.types[0] = TypeInterner::STRING,
                1 => prepared.reflections[0].type_name = "forged".to_string(),
                _ => prepared.intrinsic = IntrinsicId::TypeKind,
            }
            assert!(prepared.validate(&checked, &invocation).is_err());
        }
        entry(&mut checked, &program, "list_label");
        let (callee, types, arguments, span) = parts(first_call(function(&program, "list_label")));
        let invocation = checked.invocation(span).unwrap();
        let prepared = checked
            .prepare_intrinsic_arguments(callee, types, arguments, &invocation)
            .unwrap();
        let empty = TypeInterner::new();
        assert!(prepared.types()[0].index() as usize >= empty.len());
        assert_eq!(
            concrete_type(&empty, prepared.types()[0]),
            Err(ResourceExecutionError::InvalidCheckedProgram)
        );
    }
}

#[test]
fn resource_intrinsic_pipeline_packet_uses_original_step_and_cannot_accept_a_cloned_step() {
    for release in [false, true] {
        let program = checked_program(SOURCE, release);
        let mut checked =
            CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
        entry(&mut checked, &program, "pipeline_label");
        let mut pipeline = None;
        walk_block(
            &function(&program, "pipeline_label").body,
            &mut |_| {},
            &mut |expression| {
                if matches!(expression, Expr::Pipeline(..)) {
                    pipeline = Some(expression);
                }
            },
        );
        let pipeline = pipeline.unwrap();
        let step = checked.prepare_pipeline_step(pipeline, 0).unwrap();
        let prepared = checked.prepare_pipeline_intrinsic_arguments(&step).unwrap();
        assert_eq!(prepared.intrinsic(), IntrinsicId::TypeArg);
        assert_eq!(prepared.reflections()[0].args[0].type_name, "int64");
        prepared.validate(&checked, step.invocation()).unwrap();
        let clone = pipeline.clone();
        assert!(checked.prepare_pipeline_step(&clone, 0).is_err());
        let foreign_program = checked_program(SOURCE, release);
        let mut foreign =
            CheckedExecution::new(foreign_program.clone(), ExecutionPurpose::ReferenceRuntime)
                .unwrap();
        entry(&mut foreign, &foreign_program, "pipeline_label");
        assert!(foreign.prepare_pipeline_intrinsic_arguments(&step).is_err());
        entry(&mut checked, &program, "integer_label");
        assert!(checked.prepare_pipeline_intrinsic_arguments(&step).is_err());
    }
}
