use super::*;
use crate::resource_execution::{ProviderEvent, ScriptOperation};
use crate::{Interpreter, Value};
use jett_common::FileId;

const SOURCE: &str = "namespace app
function close_selected(token: resource_probe.TestHandle) returns nothing:
    use resource_probe
    resource_probe.close(token)
    return nothing
function wrong_target(token: resource_probe.TestHandle) returns nothing:
    use resource_probe
    resource_probe.terminal_failure()
    return nothing
export function scenario(view net: Network) returns nothing:
    use resource_probe
    function(resource_probe.TestHandle) returns nothing dispose = close_selected
    function(resource_probe.TestHandle) returns nothing alias = dispose
    resource_probe.TestHandle token = resource_probe.create(view net, 701) handle error:
        return nothing
    alias(token)
    return nothing
";

fn execution(release: bool) -> (CheckedExecution, CheckedBodyReference, DefId) {
    execution_of(SOURCE, "scenario", release)
}

fn execution_of(
    source: &str,
    name: &str,
    release: bool,
) -> (CheckedExecution, CheckedBodyReference, DefId) {
    let program = crate::resource_execution::tests::program(source, release);
    let mut checked =
        CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
    let scenario = program
        .module()
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function)
                if function.name.span.file == FileId::new(0) && function.name.name == name =>
            {
                Some(function)
            }
            _ => None,
        })
        .unwrap();
    let entry = checked.declaration_definition(scenario.name.span).unwrap();
    let body = checked
        .prepare_function_body(&checked.entry(entry).unwrap())
        .unwrap();
    checked.install_function_body(&body).unwrap();
    (checked, body, entry)
}

fn indirect(body: &CheckedBodyReference) -> (&Expr, Span) {
    let mut found = Vec::new();
    walk_block(
        &body.function().unwrap().body,
        &mut |_| {},
        &mut |expression| {
            if let Expr::Call(callee, _, span) = expression {
                if matches!(callee.as_ref(), Expr::Ident(identifier) if identifier.name == "alias")
                {
                    found.push((callee.as_ref(), *span));
                }
            }
        },
    );
    let [call] = found.as_slice() else {
        panic!("exact original indirect call");
    };
    *call
}

#[test]
fn checked_named_indirect_keeps_original_packet_and_exact_same_signature_body() {
    for release in [false, true] {
        let (checked, body, _) = execution(release);
        let (callee, span) = indirect(&body);
        let source = checked.invocation(span).unwrap();
        let target = checked.prepare_named_indirect(&source, callee).unwrap();
        assert!(
            matches!(source.target(), CheckedInvocationTarget::Indirect(signature) if *signature == target.signature)
        );
        assert_eq!(target.aliases.len(), 2);
        let selected = checked
            .prepare_function_body(&FunctionInvocation::NamedSource {
                source: &source,
                target: &target,
            })
            .unwrap();
        assert_eq!(selected.function().unwrap().name.name, "close_selected");
        let wrong = checked
            .program
            .module()
            .items
            .iter()
            .find_map(|item| match item {
                Item::Function(function) if function.name.name == "wrong_target" => Some(function),
                _ => None,
            })
            .unwrap();
        let wrong_definition = checked.declaration_definition(wrong.name.span).unwrap();
        assert_eq!(
            checked
                .program
                .checked()
                .definition_types
                .get(&wrong_definition),
            Some(&target.signature)
        );
        assert_ne!(target.definition(), wrong_definition);
    }
}

#[test]
fn checked_named_indirect_refuses_cloned_foreign_and_same_signature_substituted_authority() {
    for release in [false, true] {
        let (checked, body, _) = execution(release);
        let (callee, span) = indirect(&body);
        let source = checked.invocation(span).unwrap();
        assert!(matches!(
            checked.prepare_named_indirect(&source, &callee.clone()),
            Err(ResourceExecutionError::MissingCheckedBody)
        ));
        let mut target = checked.prepare_named_indirect(&source, callee).unwrap();
        let wrong = checked
            .program
            .module()
            .items
            .iter()
            .find_map(|item| match item {
                Item::Function(function) if function.name.name == "wrong_target" => Some(function),
                _ => None,
            })
            .unwrap();
        target.definition = checked.declaration_definition(wrong.name.span).unwrap();
        assert!(matches!(
            checked.prepare_function_body(&FunctionInvocation::NamedSource {
                source: &source,
                target: &target
            }),
            Err(ResourceExecutionError::InvalidInvocation)
        ));
        let (foreign, foreign_body, _) = execution(release);
        let (foreign_callee, foreign_span) = indirect(&foreign_body);
        let foreign_source = foreign.invocation(foreign_span).unwrap();
        let foreign_target = foreign
            .prepare_named_indirect(&foreign_source, foreign_callee)
            .unwrap();
        assert!(matches!(
            checked.prepare_function_body(&FunctionInvocation::NamedSource {
                source: &source,
                target: &foreign_target
            }),
            Err(ResourceExecutionError::InvalidInvocation)
        ));
        let mut target = checked.prepare_named_indirect(&source, callee).unwrap();
        target.aliases.pop();
        assert!(matches!(
            checked.prepare_function_body(&FunctionInvocation::NamedSource {
                source: &source,
                target: &target
            }),
            Err(ResourceExecutionError::InvalidInvocation)
        ));
        let target = checked.prepare_named_indirect(&source, callee).unwrap();
        let mut corrupt = checked.invocation(span).unwrap();
        corrupt.packet.arguments[0].parameter_type = jett_types::TypeInterner::INT64;
        assert!(matches!(
            checked.prepare_function_body(&FunctionInvocation::NamedSource {
                source: &corrupt,
                target: &target
            }),
            Err(ResourceExecutionError::InvalidInvocation)
        ));
    }
}

#[test]
fn checked_source_named_indirect_immutable_alias_closes_once() {
    for release in [false, true] {
        let (checked, _, entry) = execution(release);
        let mut interpreter = Interpreter::from_checked_resource_program(
            checked.program.clone(),
            ExecutionPurpose::ReferenceRuntime,
        )
        .unwrap();
        let grant = interpreter
            .install_resource_test_script(vec![ScriptOperation::Construct {
                label: 701,
                outcome: Ok(()),
            }])
            .unwrap();
        assert_eq!(
            interpreter
                .call_checked_program_entry(entry, vec![grant])
                .unwrap(),
            Value::Nothing
        );
        assert_eq!(
            interpreter.resource_test_observations().unwrap(),
            (
                vec![
                    ProviderEvent::Constructed(701),
                    ProviderEvent::Finalized(701)
                ],
                0,
                0
            )
        );
    }
}

const NAMESPACE_SOURCE: &str = "namespace controls
export function wrong_target(token: resource_probe.TestHandle) returns nothing:
    use resource_probe
    resource_probe.terminal_failure()
    return nothing
export function generic_close[T](token: resource_probe.TestHandle, unused: T) returns nothing:
    use resource_probe
    resource_probe.close(token)
    return nothing
namespace app
struct CallbackHolder:
    callback: function(resource_probe.TestHandle) returns nothing
struct Observer:
    value: int64
    function close(view self: Observer, token: resource_probe.TestHandle) returns nothing:
        use resource_probe
        resource_probe.close(token)
        return nothing
function field_control(view holder: CallbackHolder) returns nothing:
    function(resource_probe.TestHandle) returns nothing candidate = clone holder.callback
    return nothing
function method_control() returns nothing:
    function(view Observer, resource_probe.TestHandle) returns nothing candidate = Observer.close
    return nothing
function generic_control(token: resource_probe.TestHandle) returns nothing:
    use controls
    controls.generic_close[int64](token, 7)
    return nothing
export function scenario(view net: Network) returns nothing:
    use resource_probe as probe
    function(resource_probe.TestHandle) returns nothing dispose = probe.close
    function(resource_probe.TestHandle) returns nothing alias = dispose
    resource_probe.TestHandle token = probe.create(view net, 711) handle error:
        return nothing
    alias(token)
    return nothing
";

fn initializer<'a>(body: &'a CheckedBodyReference, name: &str) -> &'a Expr {
    let declarations = body
        .function()
        .unwrap()
        .body
        .stmts
        .iter()
        .filter_map(|statement| match statement {
            Stmt::VarDecl(declaration) if declaration.name.name == name => Some(&declaration.value),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [expression] = declarations.as_slice() else {
        panic!("exact original initializer `{name}`");
    };
    expression
}

#[test]
fn checked_namespace_callable_joins_full_producer_span_and_retained_body() {
    for release in [false, true] {
        let (checked, body, _) = execution_of(NAMESPACE_SOURCE, "scenario", release);
        let producer = initializer(&body, "dispose");
        let Expr::FieldAccess(base, member, span) = producer else {
            panic!("original namespace producer");
        };
        let target = checked.prepare_named_callable(producer).unwrap().unwrap();
        assert_eq!(checked.resolved_definition(*span), Ok(target.definition));
        assert!(
            !checked
                .program
                .resolved()
                .resolutions
                .contains_key(&base.span())
        );
        assert!(
            !checked
                .program
                .resolved()
                .resolutions
                .contains_key(&member.span)
        );
        assert_eq!(member.name, "close");
        let (callee, span) = indirect(&body);
        let source = checked.invocation(span).unwrap();
        let target = checked.prepare_named_indirect(&source, callee).unwrap();
        assert_eq!(target.aliases.len(), 2);
        assert!(
            matches!(source.target(), CheckedInvocationTarget::Indirect(signature) if *signature == target.signature)
        );
        let selected = checked
            .prepare_function_body(&FunctionInvocation::NamedSource {
                source: &source,
                target: &target,
            })
            .unwrap();
        assert_eq!(selected.function().unwrap().name.name, "close");
        assert_eq!(
            selected.function().unwrap().name.span.file,
            FileId::new(10_000)
        );
        let wrong = checked
            .program
            .resolved()
            .scope_table
            .definitions
            .iter()
            .find(|definition| definition.name == "controls.wrong_target")
            .unwrap();
        assert_eq!(
            checked.program.checked().definition_types.get(&wrong.id),
            Some(&target.signature)
        );
        assert_ne!(wrong.id, target.definition);
    }
}

#[test]
fn checked_namespace_callable_refuses_cloned_foreign_target_and_signature_substitution() {
    for release in [false, true] {
        let (checked, body, _) = execution_of(NAMESPACE_SOURCE, "scenario", release);
        let producer = initializer(&body, "dispose");
        assert!(matches!(
            checked.prepare_named_callable(&producer.clone()),
            Err(ResourceExecutionError::MissingCheckedBody)
        ));
        let (callee, span) = indirect(&body);
        let source = checked.invocation(span).unwrap();
        let wrong = checked
            .program
            .resolved()
            .scope_table
            .definitions
            .iter()
            .find(|definition| definition.name == "controls.wrong_target")
            .unwrap();
        let mut target = checked.prepare_named_indirect(&source, callee).unwrap();
        target.definition = wrong.id;
        assert!(matches!(
            checked.validate_named_indirect(&source, &target),
            Err(ResourceExecutionError::InvalidInvocation)
        ));
        let mut target = checked.prepare_named_indirect(&source, callee).unwrap();
        target.signature = jett_types::TypeInterner::INT64;
        assert!(matches!(
            checked.validate_named_indirect(&source, &target),
            Err(ResourceExecutionError::InvalidInvocation)
        ));
        let target = checked.prepare_named_callable(producer).unwrap().unwrap();
        assert!(matches!(
            checked.validate_named_indirect(&source, &target),
            Err(ResourceExecutionError::InvalidInvocation)
        ));
        let (foreign, foreign_body, _) = execution_of(NAMESPACE_SOURCE, "scenario", release);
        let (foreign_callee, foreign_span) = indirect(&foreign_body);
        let foreign_source = foreign.invocation(foreign_span).unwrap();
        let foreign_target = foreign
            .prepare_named_indirect(&foreign_source, foreign_callee)
            .unwrap();
        assert!(matches!(
            checked.validate_named_indirect(&source, &foreign_target),
            Err(ResourceExecutionError::InvalidInvocation)
        ));
    }
}

#[test]
fn checked_namespace_callable_refuses_original_instance_method_generic_and_hook_producers() {
    for release in [false, true] {
        let (checked, body, _) = execution_of(NAMESPACE_SOURCE, "scenario", release);
        let signature = checked
            .expression_type(initializer(&body, "dispose").span())
            .unwrap();
        let (field_checked, field_body, _) =
            execution_of(NAMESPACE_SOURCE, "field_control", release);
        let Expr::Clone(field, _) = initializer(&field_body, "candidate") else {
            panic!("original function field read");
        };
        assert!(
            matches!(field.as_ref(), Expr::FieldAccess(_, member, _) if member.name == "callback")
        );
        assert_eq!(
            field_checked.expression_type(field.span()).unwrap(),
            signature
        );
        assert!(
            !field_checked
                .program
                .resolved()
                .resolutions
                .contains_key(&field.span())
        );
        assert!(
            field_checked
                .prepare_named_callable(field)
                .unwrap()
                .is_none()
        );
        let (method_checked, method_body, _) =
            execution_of(NAMESPACE_SOURCE, "method_control", release);
        let method = initializer(&method_body, "candidate");
        assert!(
            method_checked
                .program
                .checked()
                .method_values
                .contains_key(&method.span())
        );
        assert!(
            method_checked
                .prepare_named_callable(method)
                .unwrap()
                .is_none()
        );
        let (generic_checked, generic_body, _) =
            execution_of(NAMESPACE_SOURCE, "generic_control", release);
        let Stmt::Expr(statement) = &generic_body.function().unwrap().body.stmts[1] else {
            panic!("original generic call");
        };
        let Expr::GenericCall(callee, _, _, _) = &statement.expr else {
            panic!("original generic callee");
        };
        let definition = generic_checked.resolved_definition(callee.span()).unwrap();
        assert!(
            !generic_checked
                .retained_function(definition)
                .unwrap()
                .type_params
                .is_empty()
        );
        assert!(
            generic_checked
                .prepare_named_callable(callee)
                .unwrap()
                .is_none()
        );
        let mut hook_checked =
            CheckedExecution::new(checked.program.clone(), ExecutionPurpose::ReferenceRuntime)
                .unwrap();
        let hook_function = checked
            .program
            .module()
            .items
            .iter()
            .find_map(|item| match item {
                Item::Function(function) if function.name.name == "close_descriptor" => {
                    Some(function)
                }
                _ => None,
            })
            .unwrap();
        let entry = hook_checked
            .declaration_definition(hook_function.name.span)
            .unwrap();
        let hook_body = hook_checked
            .prepare_function_body(&hook_checked.entry(entry).unwrap())
            .unwrap();
        hook_checked.install_function_body(&hook_body).unwrap();
        let Stmt::Return(statement) = &hook_body.function().unwrap().body.stmts[0] else {
            panic!("original hook descriptor return");
        };
        let hook = statement.value.as_ref().unwrap();
        assert!(hook_checked.has_hook(hook_checked.resolved_definition(hook.span()).unwrap()));
        assert!(hook_checked.prepare_named_callable(hook).unwrap().is_none());
    }
}

#[test]
fn checked_source_namespace_callable_and_immutable_alias_close_once() {
    for release in [false, true] {
        let (checked, _, entry) = execution_of(NAMESPACE_SOURCE, "scenario", release);
        let mut interpreter = Interpreter::from_checked_resource_program(
            checked.program.clone(),
            ExecutionPurpose::ReferenceRuntime,
        )
        .unwrap();
        let grant = interpreter
            .install_resource_test_script(vec![ScriptOperation::Construct {
                label: 711,
                outcome: Ok(()),
            }])
            .unwrap();
        assert_eq!(
            interpreter
                .call_checked_program_entry(entry, vec![grant])
                .unwrap(),
            Value::Nothing
        );
        assert_eq!(
            interpreter.resource_test_observations().unwrap(),
            (
                vec![
                    ProviderEvent::Constructed(711),
                    ProviderEvent::Finalized(711)
                ],
                0,
                0
            )
        );
    }
}
