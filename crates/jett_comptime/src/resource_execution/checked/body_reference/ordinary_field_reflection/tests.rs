use super::*;
use crate::checked_types::CheckedExpressionTypes;
use crate::{Interpreter, Value};
use jett_common::{FileId, SourceOrigin};
use jett_typecheck::{CheckOptions, CheckedResourceProgram};

const SOURCE: &str = r#"namespace app
type Label = int64
struct Box[T]:
    item: T
type WholeAlias = Box[Label]
type Positive = int64 where value > 0
function reflected() returns int64:
    Box[Label] source = Box[Label](item: 7)
    mutable TypeConstruction builder = type.construct_start[Box[Label]]()
    for field in type.fields[Box[Label]]():
        comptime type Field = field.type_info:
            Field value = type.field_value[Box[Label], Field](view source, view field)
            builder = type.construct_put[Box[Label], Field](builder, view field, value) handle error:
                return -1
    Box[Label] built = type.construct_finish[Box[Label]](builder) handle error:
        return -2
    return built.item
function alias_fields() returns int64:
    mutable int64 count = 0
    for field in type.fields[WholeAlias]():
        count = count + 1
    for field in type.fields[Positive]():
        count = count + 1
    return count
function alias_kind() returns string:
    return type.arg[Box[Label]](0).kind
function pipeline() returns result[Box[Label], string]:
    return type.construct_start[Box[Label]]() into type.construct_finish[Box[Label]]()
function main() returns int64:
    return comptime reflected()
"#;

fn program(release: bool) -> Arc<CheckedResourceProgram> {
    let file = FileId::new(0);
    Arc::new(
        CheckedResourceProgram::prepare(
            jett_parser::parse(SOURCE, file),
            HashMap::from([(file, SourceOrigin::Project)]),
            &[],
            CheckOptions { release },
        )
        .unwrap_or_else(|error| {
            panic!("reflection Source prerequisite: {:?}", error.diagnostics())
        }),
    )
}

fn function<'a>(program: &'a CheckedResourceProgram, name: &str) -> &'a FunctionDef {
    program
        .module()
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name.name == name => Some(function),
            _ => None,
        })
        .expect("genuine Source function")
}

fn enter(checked: &mut CheckedExecution, name: &str) {
    let original = function(checked.program(), name);
    let definition = checked
        .program()
        .resolved()
        .scope_table
        .definitions
        .iter()
        .find(|definition| definition.span == original.name.span)
        .unwrap();
    let body = checked
        .prepare_field_function_body(&definition.name, &definition.namespace, original, None, &[])
        .unwrap();
    checked.install_field_function_body(&body).unwrap();
}

#[test]
fn actorless_reflected_alias_owner_runs_closed_comptime_and_preserves_source_metadata() {
    for release in [false, true] {
        let program = program(release);
        assert!(program.checked().actor_handlers.is_empty());
        assert!(program.checked().assignment_places.is_empty());
        assert!(program.checked().resource_hooks.is_empty());
        let types = Arc::new(CheckedExpressionTypes::from_checked_program(&program));
        let capture = crate::evaluate_explicit_comptime_expressions_capture(
            program.module(),
            program.checked().reflection_metadata.clone(),
            types.clone(),
            Arc::new(program.checked().breakpoint_exclusions.clone()),
        );
        assert!(capture.diagnostics.is_empty(), "{:?}", capture.diagnostics);
        assert!(
            capture
                .values
                .values()
                .any(|value| value.payload() == &Value::Int64(7))
        );
        let mut interpreter = Interpreter::from_checked_resource_program(
            program.clone(),
            ExecutionPurpose::ReferenceRuntime,
        )
        .unwrap();
        interpreter.set_reflection_metadata(program.checked().reflection_metadata.clone());
        interpreter.set_checked_expression_types(types);
        for (name, value) in [
            ("reflected", Value::Int64(7)),
            ("alias_fields", Value::Int64(0)),
            ("alias_kind", Value::String("alias".into())),
        ] {
            assert_eq!(
                interpreter
                    .call_checked_function_in_namespace(Some("app"), name, vec![])
                    .unwrap()
                    .payload(),
                &value
            );
        }
    }
}

#[test]
fn ordinary_field_owner_requires_the_exact_physical_source_call() {
    let program = program(false);
    let mut checked =
        CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
    enter(&mut checked, "reflected");
    let mut call = None;
    walk_block(
        &function(&program, "reflected").body,
        &mut |_| {},
        &mut |expression| {
            if let Expr::GenericCall(_, _, _, span) = expression {
                if program.checked().intrinsic_ids.get(span)
                    == Some(&IntrinsicId::TypeConstructStart)
                {
                    call = Some(expression);
                }
            }
        },
    );
    let Expr::GenericCall(callee, types, arguments, span) = call.unwrap() else {
        unreachable!()
    };
    let owner = checked
        .field_intrinsic_owner(callee, types, arguments, *span)
        .unwrap()
        .unwrap();
    assert_eq!(
        program.checked().interner.type_name(owner),
        "app.Box[int64]"
    );
    let copied_callee = callee.clone();
    assert!(
        checked
            .field_intrinsic_owner(&copied_callee, types, arguments, *span)
            .is_err()
    );
    assert!(
        checked
            .field_intrinsic_owner(callee, &types.to_vec(), arguments, *span)
            .is_err()
    );
}

#[test]
fn ordinary_field_owner_requires_the_exact_physical_pipeline_step() {
    let program = program(false);
    let mut checked =
        CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
    enter(&mut checked, "pipeline");
    let mut step = None;
    walk_block(
        &function(&program, "pipeline").body,
        &mut |_| {},
        &mut |expression| {
            if let Expr::Pipeline(_, steps, _) = expression {
                step = steps.first();
            }
        },
    );
    let original = step.unwrap();
    let owner = checked
        .field_pipeline_intrinsic_owner(original)
        .unwrap()
        .unwrap();
    assert_eq!(
        program.checked().interner.type_name(owner),
        "app.Box[int64]"
    );
    assert!(
        checked
            .field_pipeline_intrinsic_owner(&original.clone())
            .is_err()
    );
}
