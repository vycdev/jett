use super::*;
use jett_common::{FileId, SourceOrigin};
use jett_comptime::checked_types::CheckedExpressionTypes;
use std::{collections::HashMap, sync::Arc};
const SOURCE: &str = r#"namespace app
actor Counter:
    mutable int64 count = 1
    receive read responds int64:
        respond count
function make() returns Counter:
    return comptime spawn Counter()
function main() returns nothing:
    Counter worker = make()
    int64 observed = ask worker.read
    println(observed)
    return nothing
"#;
fn prepared() -> Arc<jett_typecheck::CheckedResourceProgram> {
    prepared_source(SOURCE)
}
fn prepared_source(source: &str) -> Arc<jett_typecheck::CheckedResourceProgram> {
    let file = FileId::new(0);
    Arc::new(
        jett_typecheck::CheckedResourceProgram::prepare(
            jett_parser::parse(source, file),
            HashMap::from([(file, SourceOrigin::Project)]),
            &[],
            jett_typecheck::CheckOptions { release: false },
        )
        .expect("checked graph Source"),
    )
}
fn values(
    checked: &Arc<jett_typecheck::CheckedResourceProgram>,
) -> jett_comptime::ExplicitComptimeValues {
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
    assert!(evaluation.debug_events.is_empty());
    evaluation.values
}
fn baked(
    checked: &Arc<jett_typecheck::CheckedResourceProgram>,
    values: &jett_comptime::ExplicitComptimeValues,
) -> crate::Program {
    let mut hir = crate::lower_checked_ordinary_program(checked).unwrap();
    crate::bake_required_values(
        &mut hir,
        values,
        &checked.checked().interner,
        &checked.checked().reflection_metadata,
        &checked.checked().method_value_definitions,
    )
    .unwrap();
    crate::complete_value_conversions(&mut hir, &checked.checked().interner).unwrap();
    hir
}
fn plans(program: &crate::Program) -> Vec<ActorGraphPlan> {
    let mut plans = Vec::new();
    for function in &program.functions {
        let mut body = function.body.clone();
        visit_actor_graph_plans(&mut body, |plan| plans.push(plan.clone())).unwrap();
    }
    plans
}
#[test]
fn exact_private_graph_plan_is_at_required_occurrence_and_keeps_constructor_proof() {
    let checked = prepared();
    let values = values(&checked);
    let program = baked(&checked, &values);
    let plans = plans(&program);
    assert_eq!(plans.len(), 1);
    let plan = &plans[0];
    let owner = &program.functions[plan.function().index() as usize];
    plan.validate(owner, &checked.checked().interner).unwrap();
    let captures = plan.actor_capture_bindings().unwrap();
    assert_eq!(captures.len(), 1);
    let captured = &captures[0];
    assert_eq!((captured.node(), captured.field()), (0, 0));
    assert!(captured.mutable());
    let handler = &program.functions[captured.handler().index() as usize];
    assert_eq!(handler.params[0], *captured.parameter());
    assert_eq!(captured.field_type(), TypeInterner::INT64);
    assert!(matches!(
        plan.instructions()[0].operation(),
        ActorGraphOperation::Begin
    ));
    assert!(matches!(
        plan.instructions()[plan.instructions().len() - 2].operation(),
        ActorGraphOperation::Commit
    ));
    assert_eq!(
        plan.instructions()
            .iter()
            .filter(|step| matches!(step.operation(), ActorGraphOperation::ReserveBase { .. }))
            .count(),
        1
    );
    assert_eq!(
        plan.instructions()
            .iter()
            .filter(|step| matches!(step.operation(), ActorGraphOperation::Fill { .. }))
            .count(),
        1
    );
    validate_actor_graph_execution(&program, &checked.checked().interner).unwrap();
    assert!(
        program
            .functions
            .iter()
            .any(|function| function.identity.declaration.kind
                == crate::DeclarationKind::ActorConstructor),
        "normal checked constructor remains available"
    );
}
#[test]
fn public_span_mirror_cannot_replace_private_graph_snapshot() {
    let checked = prepared();
    let mut values = values(&checked);
    let proof = values.checked_required_values(&checked).unwrap().remove(0);
    values.insert(
        proof.source_span(),
        jett_comptime::ComptimeContext::default(),
        jett_comptime::Value::Actor(777),
    );
    let program = baked(&checked, &values);
    let plans = plans(&program);
    let step = plans[0]
        .instructions()
        .iter()
        .find(|step| matches!(step.operation(), ActorGraphOperation::Fill { .. }))
        .unwrap();
    assert_eq!(step.field_value().unwrap(), &jett_comptime::Value::Int64(1));
}
#[test]
fn raw_public_actor_value_cannot_mint_checked_graph_import() {
    let checked = prepared();
    let actual = values(&checked);
    let proof = actual.checked_required_values(&checked).unwrap().remove(0);
    let mut forged = jett_comptime::ExplicitComptimeValues::default();
    forged.insert(
        proof.source_span(),
        jett_comptime::ComptimeContext::default(),
        jett_comptime::Value::Actor(0),
    );
    let mut program = crate::lower_checked_ordinary_program(&checked).unwrap();
    assert!(
        crate::bake_required_values(
            &mut program,
            &forged,
            &checked.checked().interner,
            &checked.checked().reflection_metadata,
            &checked.checked().method_value_definitions
        )
        .is_err()
    );
}
#[test]
fn changed_current_hir_or_missing_nominal_table_refuses_before_mir() {
    let checked = prepared();
    let values = values(&checked);
    let program = baked(&checked, &values);
    let plan = plans(&program).remove(0);
    let mut changed = program.clone();
    let owner = &mut changed.functions[plan.function().index() as usize];
    owner.locals[plan.output().index() as usize].ty = TypeInterner::BOOL;
    assert!(validate_actor_graph_execution(&changed, &checked.checked().interner).is_err());
    let mut missing = TypeInterner::new();
    for ty in checked.checked().interner.type_ids() {
        assert_eq!(
            missing.intern(checked.checked().interner.resolve(ty).clone()),
            ty
        );
    }
    assert!(
        plan.validate(
            &program.functions[plan.function().index() as usize],
            &missing
        )
        .is_err()
    );
}
#[test]
fn instruction_copy_retains_exact_owner_operand_and_type_guard() {
    let checked = prepared();
    let values = values(&checked);
    let program = baked(&checked, &values);
    let plan = plans(&program).remove(0);
    let owner = &program.functions[plan.function().index() as usize];
    let step = plan
        .instructions()
        .iter()
        .find(|step| matches!(step.operation(), ActorGraphOperation::Fill { .. }))
        .unwrap();
    let original = plan
        .body()
        .statements
        .iter()
        .find_map(|statement| match &statement.kind {
            crate::StatementKind::Expression(Expression {
                kind: E::ActorGraphStep { instruction, args },
                ty,
                ..
            }) if instruction == step => Some((args.clone(), *ty)),
            _ => None,
        })
        .unwrap();
    step.validate(
        owner.id,
        &owner.identity,
        &original.0,
        original.1,
        &checked.checked().interner,
    )
    .unwrap();
    let mut changed = original.0;
    changed[0].kind = E::Int(0);
    assert!(
        step.validate(
            owner.id,
            &owner.identity,
            &changed,
            original.1,
            &checked.checked().interner
        )
        .is_err()
    );
}

const INDEPENDENT_GRAPH_AND_REFLECTED: &str = r#"namespace app
type Positive = int64 where value > 0
function five() returns int64:
    return 5
function positive() returns Positive:
    return 5
struct Holder:
    callback: function() returns int64
function adapted(view holder: Holder) returns function() returns Positive:
    for field in type.fields[Holder]():
        return type.field_value[Holder, function() returns Positive](view holder, view field)
    return positive
actor Counter:
    mutable int64 count = 1
    receive read responds int64:
        respond count
function observe() returns int64:
    Counter worker = comptime spawn Counter()
    Holder holder = Holder(callback: five)
    for field in type.fields[Holder]():
        function() returns Positive callback = type.field_value[Holder, function() returns Positive](view holder, view field)
        Positive validated = callback()
        int64 count = ask worker.read
        return count + coarsen validated
    return 0
"#;

const ADAPTED_CALLBACK_GRAPH_FIELD: &str = r#"namespace app
type Positive = int64 where value > 0
function five() returns int64:
    return 5
function positive() returns Positive:
    return 5
struct Holder:
    callback: function() returns int64
function adapted(view holder: Holder) returns function() returns Positive:
    for field in type.fields[Holder]():
        return type.field_value[Holder, function() returns Positive](view holder, view field)
    return positive
actor Keeper(callback: function() returns Positive):
    receive read responds Positive:
        respond callback()
function create() returns Keeper:
    Holder holder = Holder(callback: five)
    return spawn Keeper(callback: adapted(view holder))
function observe() returns int64:
    Keeper worker = comptime create()
    Positive observed = ask worker.read
    return coarsen observed
"#;

const TWO_GRAPH_SITES: &str = r#"namespace app
actor Counter:
    mutable int64 count = 1
    receive add(amount: int64):
        count = count + amount
    receive read responds int64:
        respond count
function observe() returns int64:
    Counter first = comptime spawn Counter()
    Counter second = comptime spawn Counter()
    send first.add(4)
    int64 left = ask first.read
    int64 right = ask second.read
    return left * 10 + right
"#;

const GRAPH_AND_ORDINARY_REFINEMENT: &str = r#"namespace app
type Positive = int64 where value > 0
function positive() returns Positive:
    return 5
actor Counter:
    mutable int64 count = 1
    receive read responds int64:
        respond count
function observe() returns int64:
    Counter worker = comptime spawn Counter()
    Positive cached = comptime positive()
    int64 ordinary = comptime 7
    int64 observed = ask worker.read
    return observed + coarsen cached + ordinary
"#;

const SAME_SIGNATURE_FOREIGN_GRAPH_LEAF: &str = r#"namespace app
type Positive = int64 where value > 0
function five() returns int64:
    return 5
function positive() returns Positive:
    return 5
struct Holder:
    callback: function() returns int64
function adapted(view holder: Holder) returns function() returns Positive:
    for field in type.fields[Holder]():
        return type.field_value[Holder, function() returns Positive](view holder, view field)
    return positive
actor Keeper(callback: function() returns Positive):
    receive read responds Positive:
        respond callback()
function create() returns Keeper:
    Holder holder = Holder(callback: five)
    return spawn Keeper(callback: adapted(view holder))
function observe() returns int64:
    Keeper first = comptime create()
    Keeper second = comptime create()
    Positive left = ask first.read
    Positive right = ask second.read
    return coarsen left + coarsen right
"#;

fn reference_observations(
    checked: &Arc<jett_typecheck::CheckedResourceProgram>,
    values: &jett_comptime::ExplicitComptimeValues,
    expected: i64,
) {
    let mut runtime = jett_comptime::Interpreter::new();
    runtime
        .install_checked_resource_program(
            checked.clone(),
            jett_comptime::ExecutionPurpose::ReferenceRuntime,
        )
        .unwrap();
    runtime.set_reflection_metadata(checked.checked().reflection_metadata.clone());
    runtime.set_checked_expression_types(Arc::new(CheckedExpressionTypes {
        resource_program: Some(checked.clone()),
        expressions: checked
            .checked()
            .type_map
            .iter()
            .map(|(span, ty)| (*span, checked.checked().interner.type_name(*ty)))
            .collect(),
        ..Default::default()
    }));
    runtime.register_module(checked.module());
    runtime.set_explicit_comptime_values(Arc::new(values.clone()));
    for _ in 0..2 {
        assert_eq!(
            runtime.call_function("app.observe", Vec::new()).unwrap(),
            Value::Int64(expected),
        );
        assert!(runtime.take_debug_events().is_empty());
    }
}

fn validate_interop(
    program: &crate::Program,
    checked: &Arc<jett_typecheck::CheckedResourceProgram>,
) {
    crate::validate(program).unwrap();
    crate::validate_backend_types(program, &checked.checked().interner).unwrap();
    validate_actor_graph_execution(program, &checked.checked().interner).unwrap();
}

fn reflected_graph_field(
    plan: &ActorGraphPlan,
) -> (
    &ActorGraphInstruction,
    &jett_comptime::value::ReflectedCallableValue,
) {
    plan.instructions()
        .iter()
        .find_map(|instruction| {
            if !matches!(instruction.operation(), ActorGraphOperation::Fill { .. }) {
                return None;
            }
            let Value::ReflectedCallable(reference) = instruction.field_value().ok()?.payload()
            else {
                return None;
            };
            Some((instruction, reference.as_ref()))
        })
        .expect("actual frozen Actor field contains the admitted reflected callable")
}

#[test]
fn independent_graph_and_reflected_getter_preserve_one_source_owner() {
    let checked = prepared_source(INDEPENDENT_GRAPH_AND_REFLECTED);
    let values = values(&checked);
    let program = baked(&checked, &values);
    assert_eq!(plans(&program).len(), 1);
    let owner = program
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "observe")
        .unwrap();
    let ordinary = crate::reflected_callable_plans(owner);
    assert!(
        !ordinary.is_empty(),
        "ordinary checked getter stays in the Graph owner"
    );
    assert!(
        ordinary
            .iter()
            .all(|plan| plan.is_sealed() && plan.validate_original())
    );
    validate_interop(&program, &checked);
    reference_observations(&checked, &values, 6);
}

#[test]
fn adapted_callback_in_actor_field_selects_its_sealed_graph_leaf() {
    let checked = prepared_source(ADAPTED_CALLBACK_GRAPH_FIELD);
    let values = values(&checked);
    let program = baked(&checked, &values);
    let plans = plans(&program);
    assert_eq!(plans.len(), 1);
    let (instruction, reference) = reflected_graph_field(&plans[0]);
    let selected = instruction
        .reflected_callable(reference)
        .expect("exact frozen leaf selects its protected HIR plan");
    assert!(selected.is_sealed() && selected.validate_original());
    assert_eq!(selected.checked_plan(), reference.checked_plan());
    assert_eq!(selected.checked_plan().source_return(), TypeInterner::INT64);
    assert_ne!(
        selected.checked_plan().requested_return(),
        TypeInterner::INT64
    );
    assert!(selected.canonical_adapter().is_some());
    validate_interop(&program, &checked);
    reference_observations(&checked, &values, 5);
}

#[test]
fn multiple_graph_sites_in_one_owner_reserve_independent_activations() {
    let checked = prepared_source(TWO_GRAPH_SITES);
    let values = values(&checked);
    let program = baked(&checked, &values);
    let plans = plans(&program);
    assert_eq!(plans.len(), 2);
    assert_eq!(plans[0].function(), plans[1].function());
    assert_ne!(
        plans[0].required_proof().source_span(),
        plans[1].required_proof().source_span()
    );
    assert!(!plans[0].instructions()[0].same_import(&plans[1].instructions()[0]));
    for plan in &plans {
        let field = plan
            .instructions()
            .iter()
            .find(|instruction| matches!(instruction.operation(), ActorGraphOperation::Fill { .. }))
            .unwrap();
        assert_eq!(field.field_value().unwrap(), &Value::Int64(1));
    }
    validate_interop(&program, &checked);
    // 51 proves mutation of the first actor does not reach the second. Repeating
    // the call proves each source activation starts from the same frozen value.
    reference_observations(&checked, &values, 51);
}

#[test]
fn graph_and_ordinary_required_refinement_share_authenticated_materialization() {
    let checked = prepared_source(GRAPH_AND_ORDINARY_REFINEMENT);
    let values = values(&checked);
    let proofs = values.checked_required_values(&checked).unwrap();
    assert_eq!(proofs.len(), 3);
    assert_eq!(
        proofs
            .iter()
            .filter(|proof| proof.actor_graph().is_some())
            .count(),
        1
    );
    let program = baked(&checked, &values);
    assert!(!crate::has_reflected_callable_plans(&program));
    for function in &program.functions {
        let mut body = function.body.clone();
        crate::resource_materialization::walk_block(&mut body, &mut |expression| {
            assert!(!matches!(
                expression.kind,
                E::Constant { .. } | E::Comptime { .. }
            ));
            Ok(())
        })
        .unwrap();
    }
    validate_interop(&program, &checked);
    reference_observations(&checked, &values, 13);
}

#[test]
fn same_signature_foreign_graph_leaf_and_return_plan_cannot_substitute() {
    let checked = prepared_source(SAME_SIGNATURE_FOREIGN_GRAPH_LEAF);
    let values = values(&checked);
    let program = baked(&checked, &values);
    let plans = plans(&program);
    assert_eq!(plans.len(), 2);
    let (first_instruction, first_reference) = reflected_graph_field(&plans[0]);
    let (second_instruction, second_reference) = reflected_graph_field(&plans[1]);
    assert_eq!(
        first_reference.source_type(),
        second_reference.source_type()
    );
    assert_eq!(
        first_reference.requested_type(),
        second_reference.requested_type()
    );
    assert_eq!(
        first_reference.checked_plan(),
        second_reference.checked_plan()
    );
    let first = first_instruction
        .reflected_callable(first_reference)
        .unwrap();
    let second = second_instruction
        .reflected_callable(second_reference)
        .unwrap();
    assert_ne!(first, second);
    assert!(
        first_instruction
            .reflected_callable(second_reference)
            .is_none()
    );
    assert!(
        second_instruction
            .reflected_callable(first_reference)
            .is_none()
    );
    validate_interop(&program, &checked);
    reference_observations(&checked, &values, 10);

    let mut changed = program.clone();
    let adapter = &mut changed.functions[first.function().index() as usize];
    let crate::StatementKind::Return(Some(Expression {
        kind:
            E::Handle {
                kind: crate::HandleKind::ReflectedReturn { callable, .. },
                ..
            },
        ..
    })) = &mut adapter.body.statements[0].kind
    else {
        panic!("canonical adapted return root");
    };
    *callable = Some(second.clone());
    assert!(
        crate::validate(&changed).is_err(),
        "same signature does not transfer the sealed return authority"
    );
    assert!(crate::complete_value_conversions(&mut changed, &checked.checked().interner).is_err());
}

#[test]
fn ordinary_required_value_keeps_the_retained_actor_archive_and_refuses_edits() {
    const ORDINARY: &str = "namespace app\nactor Counter:\n    mutable int64 count = 1\nfunction main() returns nothing:\n    int64 answer = comptime 7\n    println(answer)\n    return nothing\n";
    let checked = prepared_source(ORDINARY);
    let required = values(&checked);
    assert!(!required.has_actor_graphs());
    let types = &checked.checked().interner;
    let original = crate::lower_checked_ordinary_program(&checked).unwrap();
    assert!(!crate::has_reflected_callable_plans(&original));
    assert!(original.resource_source.checked_program().is_some());
    let program = baked(&checked, &required);
    program
        .resource_source
        .validate_current(&program, types)
        .unwrap();
    assert!(plans(&program).is_empty());
    let mut converted = program.clone();
    crate::complete_value_conversions(&mut converted, types).unwrap();
    converted
        .resource_source
        .validate_current(&converted, types)
        .unwrap();
    let mut before_forgery = original.clone();
    before_forgery.functions[0].span.end += 1;
    assert!(
        crate::bake_required_values(
            &mut before_forgery,
            &required,
            types,
            &checked.checked().reflection_metadata,
            &checked.checked().method_value_definitions
        )
        .is_err()
    );
    let foreign = prepared_source(ORDINARY);
    let foreign_required = values(&foreign);
    let mut foreign_forgery = original;
    assert!(
        crate::bake_required_values(
            &mut foreign_forgery,
            &foreign_required,
            types,
            &checked.checked().reflection_metadata,
            &checked.checked().method_value_definitions
        )
        .is_err()
    );
    let mut current_forgery = program;
    current_forgery.functions[0].span.end += 1;
    assert!(crate::complete_value_conversions(&mut current_forgery, types).is_err());
}
