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
    let program = crate::resource_execution::tests::program(SOURCE, release);
    let mut checked =
        CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
    let scenario = program
        .module()
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function)
                if function.name.span.file == FileId::new(0)
                    && function.name.name == "scenario" =>
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
