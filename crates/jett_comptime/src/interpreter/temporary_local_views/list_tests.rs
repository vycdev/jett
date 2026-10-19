//! Checked Source controls for borrowed List transport; no native execution credit.
use super::*;
use jett_common::{FileId, STDLIB_FILE_ID_START, SourceOrigin};
use jett_typecheck::{CheckOptions, CheckedResourceProgram};

const SOURCE: &str = r#"namespace app
struct Packet:
    left: list[int64]
    right: list[string]
function make_packet() returns Packet:
    return Packet(left: list(2, 3, 5), right: list("Ada", "Lin"))
mutual:
    function observe[T](view items: list[T]) returns int64
function observe[T](view items: list[T]) returns int64:
    return list.length[T](view items)
function scenario() returns int64:
    list[int64] numbers = view make_packet().left
    list[int64] forwarded = numbers
    int64 length = observe[int64](view forwarded)
    int64 inferred = observe(view numbers)
    int64 total = list.sum[int64](view numbers)
    bool empty = list.is_empty[int64](view forwarded)
    optional[int64] first = list.get[int64](view numbers, 0)
    optional[int64] absent = list.get[int64](view numbers, -1)
    list[int64] copied = clone numbers
    int64 copied_total = list.sum[int64](view copied)
    return length + inferred + total + copied_total
function names() returns int64:
    list[string] values = view make_packet().right
    optional[string] first = list.get[string](view values, 0)
    return observe[string](view values)
function copied_first() returns optional[string]:
    list[string] values = view make_packet().right
    return list.get[string](view values, 0)
function absent() returns optional[int64]:
    list[int64] values = view make_packet().left
    return list.get[int64](view values, -1)
"#;

fn program() -> Arc<CheckedResourceProgram> {
    let project = FileId::new(0);
    let stdlib = FileId::new(STDLIB_FILE_ID_START);
    let mut module = jett_parser::parse(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../stdlib/list.jett"
        )),
        stdlib,
    );
    // Retain actual stdlib declarations and their complete bodies. Unrelated
    // list algorithms need other stdlib namespaces and are outside this control.
    assert!(module.errors.is_empty(), "{:?}", module.errors);
    module.module.items.retain(|item| match item {
        Item::Namespace(_) => true,
        Item::Function(function) => matches!(
            function.name.name.as_str(),
            "length" | "get" | "is_empty" | "sum"
        ),
        _ => false,
    });
    let project_source = jett_parser::parse(SOURCE, project);
    assert!(
        project_source.errors.is_empty(),
        "{:?}",
        project_source.errors
    );
    module.module.items.extend(project_source.module.items);
    Arc::new(
        CheckedResourceProgram::prepare(
            module,
            HashMap::from([
                (project, SourceOrigin::Project),
                (stdlib, SourceOrigin::Stdlib),
            ]),
            &[],
            CheckOptions::default(),
        )
        .expect("checked temporary List Source"),
    )
}

fn interpreter(program: &Arc<CheckedResourceProgram>) -> Interpreter {
    let mut interpreter = Interpreter::from_checked_resource_program(
        program.clone(),
        crate::ExecutionPurpose::ReferenceRuntime,
    )
    .unwrap();
    interpreter.checked_expression_types = Some(Arc::new(
        CheckedExpressionTypes::from_checked_program(program),
    ));
    interpreter
}

fn function(program: &Arc<CheckedResourceProgram>, name: &str) -> FunctionReceipt {
    let source = program
        .module()
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name.name == name => Some(function),
            _ => None,
        })
        .unwrap();
    receipts::function_receipt(program, &format!("app.{name}"), &Some("app".into()), source)
        .unwrap()
        .unwrap()
}

fn calls(function: &FunctionReceipt) -> Vec<receipts::CallReceipt> {
    receipts::direct_expressions(&function.original().unwrap().body)
        .into_iter()
        .filter_map(|expression| match expression {
            Expr::Call(callee, arguments, span) | Expr::GenericCall(callee, _, arguments, span) => {
                Some(receipts::checked_call(function, callee, arguments, *span).unwrap())
            }
            _ => None,
        })
        .collect()
}

#[test]
fn checked_list_views_forward_to_generic_observers_without_snapshotting_owner() {
    let program = program();
    let mut interpreter = interpreter(&program);
    assert_eq!(
        interpreter.call_checked_function_in_namespace(Some("app"), "scenario", vec![]),
        Ok(Value::Int64(26))
    );
    let context = interpreter.temporary_local_views.as_ref().unwrap();
    assert!(context.activations.is_empty());
    assert_eq!(
        context
            .audit
            .iter()
            .filter(|event| matches!(event, storage::AuditEvent::Acquired { .. }))
            .count(),
        1
    );
    assert_eq!(
        context
            .audit
            .iter()
            .filter(|event| matches!(event, storage::AuditEvent::OwnerRetired { .. }))
            .count(),
        1
    );
    assert_eq!(
        context
            .audit
            .iter()
            .filter(|event| matches!(event, storage::AuditEvent::ExplicitClone { .. }))
            .count(),
        1
    );
}

#[test]
fn checked_string_list_get_observes_copyable_items_in_its_exact_generic_body() {
    let program = program();
    let mut interpreter = interpreter(&program);
    assert_eq!(
        interpreter.call_checked_function_in_namespace(Some("app"), "names", vec![]),
        Ok(Value::Int64(2))
    );
    assert!(
        interpreter
            .temporary_local_views
            .as_ref()
            .unwrap()
            .activations
            .is_empty()
    );
}

#[test]
fn generic_receipts_separate_same_template_specializations_and_foreign_programs() {
    let program = program();
    let scenario = function(&program, "scenario");
    let names = function(&program, "names");
    let observer = |function: &FunctionReceipt| {
        calls(function)
            .into_iter()
            .find_map(|call| {
                let target = receipts::named_target(&call).unwrap()?;
                (target.original().unwrap().name.name == "observe").then_some(target)
            })
            .unwrap()
    };
    let numbers = observer(&scenario);
    let strings = observer(&names);
    assert!(std::ptr::eq(
        numbers.original().unwrap(),
        strings.original().unwrap()
    ));
    for receipt in [&numbers, &strings] {
        let original = receipt.original().unwrap();
        let info = program.resolved().scope_table.def(receipt.definition);
        assert_ne!(info.span, original.name.span);
        assert_eq!(
            program.resolved().resolutions.get(&original.name.span),
            Some(&receipt.definition)
        );
    }
    assert!(!numbers.same(&strings));
    assert!(!storage::same_function(&numbers, &strings).unwrap());
    let foreign = self::program();
    let other = observer(&function(&foreign, "scenario"));
    assert!(!numbers.same(&other));
    assert!(!storage::same_function(&numbers, &other).unwrap());
    let source = numbers.original().unwrap();
    let info = program.resolved().scope_table.def(numbers.definition);
    assert!(
        numbers
            .validate_registered(&info.name, &info.namespace, source, None, &[])
            .is_err()
    );
}

#[test]
fn exact_generic_call_rejects_changed_specialization_before_frame_publication() {
    let program = program();
    let scenario = function(&program, "scenario");
    let mut call = calls(&scenario)
        .into_iter()
        .find(|call| {
            matches!(
                call.packet.target,
                jett_typecheck::CheckedInvocationTarget::Generic(_)
            )
        })
        .unwrap();
    let jett_typecheck::CheckedInvocationTarget::Generic(target) = &mut call.packet.target else {
        unreachable!()
    };
    target
        .specialization
        .type_argument_kinds
        .push("foreign".into());
    assert!(receipts::named_target(&call).is_err());
    assert!(
        interpreter(&program)
            .temporary_local_views
            .unwrap()
            .audit
            .is_empty()
    );
}

#[test]
fn checked_list_get_returns_owned_copyable_output_and_preserves_bounds() {
    let program = program();
    let mut interpreter = interpreter(&program);
    assert_eq!(
        interpreter.call_checked_function_in_namespace(Some("app"), "copied_first", vec![]),
        Ok(Value::OptionalSome(Box::new(Value::String("Ada".into()))))
    );
    assert_eq!(
        interpreter.call_checked_function_in_namespace(Some("app"), "absent", vec![]),
        Ok(Value::OptionalNone)
    );
    assert!(
        interpreter
            .temporary_local_views
            .as_ref()
            .unwrap()
            .activations
            .is_empty()
    );
}
