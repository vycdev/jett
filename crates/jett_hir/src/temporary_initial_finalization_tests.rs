//! Controls observe the real shared sealer and fresh Lowerer preparation cut.
//! No positive receipt, open marker, or construction is supplied by a test.
use super::*;
use jett_common::{FileId, SourceOrigin};
use jett_typecheck::{CheckOptions, CheckedResourceProgram};

const SOURCE: &str = r#"namespace app
type Positive = int64 where value > 0
struct Holder:
    callback: function() returns int64
struct Packet:
    data: list[int64]
    sibling: list[int64]
function five() returns int64:
    return 5
function fallback() returns Positive:
    return 7
function make_packet() returns Packet:
    return Packet(data: list(1, 2), sibling: list(3, 4))
actor Worker:
    receive read responds int64:
        Holder holder = Holder(callback: five)
        mutable function() returns Positive selected = fallback
        for field in type.fields[Holder]():
            selected = type.field_value[Holder, function() returns Positive](view holder, view field)
        int64 value = coarsen selected()
        respond value
function main(stdout: Stdout) returns nothing:
    list[int64] borrowed = view make_packet().data
    Worker worker = spawn Worker()
    int64 observed = ask worker.read
    Stdout.write(view stdout, "observed:{observed}\n")
    return nothing
"#;
const CHAIN_SOURCE: &str = r#"namespace app
type Positive = int64 where value > 0
struct Holder:
    callback: function() returns int64
struct Packet:
    data: list[int64]
    sibling: list[int64]
function five() returns int64:
    return 5
function fallback() returns Positive:
    return 7
function make_packet() returns Packet:
    return Packet(data: list(1, 2), sibling: list(3, 4))
function next_packet(view first: list[int64], view again: list[int64]) returns Packet:
    return Packet(data: clone first, sibling: clone again)
actor Worker:
    receive read responds int64:
        Holder holder = Holder(callback: five)
        mutable function() returns Positive selected = fallback
        for field in type.fields[Holder]():
            selected = type.field_value[Holder, function() returns Positive](view holder, view field)
        int64 value = coarsen selected()
        respond value
function main(stdout: Stdout) returns nothing:
    list[int64] borrowed = view make_packet().data
    list[int64] second = view next_packet(view borrowed, view borrowed).data
    Worker worker = spawn Worker()
    int64 observed = ask worker.read
    Stdout.write(view stdout, "observed:{observed}\n")
    return nothing
"#;

const INTERFACE_SOURCE: &str = r#"namespace app
type Positive = int64 where value > 0
interface Named:
    function name(view self: Named) returns string
implement Named for int64:
    function name(view self: int64) returns string:
        return "number"
struct Holder:
    callback: function() returns int64
struct Packet:
    data: list[int64]
    sibling: list[int64]
function five() returns int64:
    return 5
function fallback() returns Positive:
    return 7
function make_packet(view input: Named) returns Packet:
    return Packet(data: list(1, 2), sibling: list(3, 4))
actor Worker:
    receive read responds int64:
        Holder holder = Holder(callback: five)
        mutable function() returns Positive selected = fallback
        for field in type.fields[Holder]():
            selected = type.field_value[Holder, function() returns Positive](view holder, view field)
        int64 value = coarsen selected()
        respond value
function main(stdout: Stdout) returns nothing:
    int64 number = 7
    list[int64] borrowed = view make_packet(view number).data
    Worker worker = spawn Worker()
    int64 observed = ask worker.read
    Stdout.write(view stdout, "observed:{observed}\n")
    return nothing
"#;

#[derive(Clone, Copy)]
enum Case {
    State,
    Context,
    Dependencies,
    Cells,
    Payload,
    Coercion,
}
std::thread_local! {
    static CASE: std::cell::Cell<Option<Case>> = const { std::cell::Cell::new(None) };
    static HITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static COMMITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
fn observed(case: Case, source: &str, release: bool) -> (Program, Arc<CheckedResourceProgram>) {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            CASE.with(|v| v.set(None));
        }
    }
    CASE.with(|v| assert!(v.replace(Some(case)).is_none()));
    HITS.with(|v| v.set(0));
    COMMITS.with(|v| v.set(0));
    let reset = Reset;
    let result = high(source, release);
    assert_eq!(
        HITS.with(|v| v.get()),
        1,
        "real original preparation observed exactly once"
    );
    assert_eq!(
        COMMITS.with(|v| v.get()),
        1,
        "real original commit observed exactly once"
    );
    drop(reset);
    result
}
fn checked(source: &str, release: bool) -> Arc<CheckedResourceProgram> {
    let file = FileId::new(0);
    Arc::new(
        CheckedResourceProgram::prepare(
            jett_parser::parse(source, file),
            HashMap::from([(file, SourceOrigin::Project)]),
            &[],
            CheckOptions { release },
        )
        .unwrap_or_else(|error| panic!("complete genuine Source must check: {error:?}")),
    )
}
fn high(source: &str, release: bool) -> (Program, Arc<CheckedResourceProgram>) {
    let checked = checked(source, release);
    let program = crate::lower_checked_ordinary_program(&checked)
        .unwrap_or_else(|errors| panic!("real initial lowering; release={release}: {errors:?}"));
    crate::validate(&program).unwrap();
    crate::validate_backend_types(&program, &checked.checked().interner).unwrap();
    program
        .resource_source
        .validate_current(&program, &checked.checked().interner)
        .unwrap();
    (program, checked)
}
fn main(program: &Program) -> &Function {
    program
        .functions
        .iter()
        .find(|f| f.identity.declaration.name == "main")
        .unwrap()
}
fn unsealed_copy(proof: &TemporaryLocalBacking) -> TemporaryLocalBacking {
    let d = &proof.0;
    TemporaryLocalBacking(Arc::new(Original {
        checked: d.checked.clone(),
        declaration: d.declaration,
        declaration_address: d.declaration_address,
        owner: d.owner.clone(),
        alias: d.alias.clone(),
        scope: TemporaryLocalScope {
            body: d.scope.body,
            statement: d.scope.statement,
        },
        initializer: d.initializer.clone(),
        producer: d.producer.clone(),
        canonical_producer: OnceLock::new(),
        rewritten: d.rewritten.clone(),
        required_scopes: d.required_scopes.clone(),
        scoped_types: d.scoped_types.clone(),
    }))
}

/// This test-only observer has no production constructor or setter. It borrows
/// only a genuinely prepared real receipt before the actual producer commits.
pub(super) fn observe(
    prepared: &Prepared,
    before: &Program,
    output: &Program,
    types: &TypeInterner,
    receipt: &CheckedCallableSealing<'_>,
) {
    let Some(case) = CASE.with(|v| v.take()) else {
        return;
    };
    open(&prepared.rows).unwrap();
    assert!(Arc::ptr_eq(&prepared.sources, receipt.source_epoch()));
    let current = inventory(output, &prepared.construction, &prepared.checked, types).unwrap();
    assert!(same_rows(&prepared.rows, &current));
    match case {
        Case::State => {
            assert_eq!(prepared.rows.len(), 1);
            assert!(marker(&prepared.rows[0].canonical).is_none());
            assert!(
                payload_dependencies(&prepared.rows[0].canonical)
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(prepared.construction.records.len(), 1);
            closed(&prepared.rows).unwrap();
        }
        Case::Context => {
            let foreign = checked(SOURCE, false);
            assert!(!Arc::ptr_eq(&foreign, &prepared.checked));
            assert!(!std::ptr::eq(types, &foreign.checked().interner));
            assert!(
                inventory(
                    before,
                    &prepared.construction,
                    &foreign,
                    &foreign.checked().interner
                )
                .is_err()
            );
            assert!(
                receipt
                    .authenticate_output(before, output, &foreign.checked().interner)
                    .is_err()
            );
            let empty = Construction::new(prepared.checked.module(), prepared.checked.checked());
            assert!(prepare(before, output, types, receipt, &prepared.checked, empty).is_err());
            for input in [true, false] {
                let mut changed = if input {
                    before.clone()
                } else {
                    output.clone()
                };
                changed.functions[0].span.end += 1;
                assert!(
                    receipt
                        .authenticate_output(
                            if input { &changed } else { before },
                            if input { output } else { &changed },
                            types,
                        )
                        .is_err()
                );
            }
            let mut missing = before.clone();
            let owner = missing
                .functions
                .iter_mut()
                .find(|f| f.id == prepared.rows[0].function)
                .unwrap();
            owner.body.statements.remove(0);
            assert!(inventory(&missing, &prepared.construction, &prepared.checked, types).is_err());
            let mut appended = before.clone();
            appended.functions.push(before.functions[0].clone());
            assert!(
                receipt
                    .authenticate_output(&appended, output, types)
                    .is_err()
            );
        }
        Case::Dependencies => {
            assert_eq!(prepared.rows.len(), 2);
            let first = &prepared.rows[0];
            let second = &prepared.rows[1];
            let deps = payload_dependencies(&second.canonical).unwrap();
            assert_eq!(
                deps,
                vec![first.proof.clone()],
                "two genuine argument uses deduplicate to one earlier proof"
            );
            closed(&prepared.rows).unwrap();
            dependency(second, &first.proof, &prepared.rows).unwrap();
            dependency(second, &first.proof, &prepared.rows).unwrap();
            assert!(dependency(first, &second.proof, &prepared.rows).is_err());
            assert!(dependency(second, &second.proof, &prepared.rows).is_err());
            assert!(closed(&prepared.rows[1..]).is_err());
            let mut wrong_body = second.clone();
            wrong_body.function = FunctionId(u32::MAX);
            assert!(dependency(&wrong_body, &first.proof, &prepared.rows).is_err());
            let ExpressionKind::Call {
                ownership: CallOwnership::Source(source),
                ..
            } = &second.canonical.kind
            else {
                panic!();
            };
            let argument = source
                .arguments
                .iter()
                .find_map(|arg| arg.source_witness().temporary_local_origin())
                .unwrap();
            let mut wrong = argument.clone();
            wrong.alias = LocalId(u32::MAX);
            assert!(argument_proof(&wrong).is_err());
            let foreign = unsealed_copy(&first.proof);
            assert!(dependency(second, &foreign, &prepared.rows).is_err());
        }
        Case::Cells => {
            let mut fake_row = prepared.rows[0].clone();
            fake_row.proof = unsealed_copy(&fake_row.proof);
            fake_row
                .proof
                .0
                .canonical_producer
                .set(fake_row.canonical.clone())
                .unwrap();
            assert!(open(&[fake_row]).is_err());
        }
        Case::Coercion => {
            assert_eq!(prepared.rows.len(), 1);
            let row = &prepared.rows[0];
            assert!(!resource_materialization::expression_equal(
                &row.canonical,
                &row.proof.0.producer
            ));
            let ExpressionKind::Call { args, .. } = &row.canonical.kind else {
                panic!();
            };
            assert!(matches!(
                args[0].kind,
                ExpressionKind::InterfaceCoerce { .. }
            ));
            assert!(same_rows(&prepared.rows, &current));
        }
        Case::Payload => {
            let mut sealed_getter = None;
            for function in &output.functions {
                let mut body = function.body.clone();
                resource_materialization::walk_block(&mut body, &mut |value| {
                    if let ExpressionKind::Intrinsic { field_validation: Some(ReflectedFieldValidation::Validate(plans)), .. } = &value.kind {
                        if plans.iter().any(|p| matches!(&p.action, ReflectedFieldAction::CallableReturn(plan) if plan.is_sealed())) {
                            sealed_getter = Some(value.clone());
                        }
                    }
                    Ok(())
                }).unwrap();
            }
            let sealed_getter = sealed_getter.expect("real output has a genuinely sealed getter");
            assert!(payload_dependencies(&sealed_getter).is_err());
            let mut changed = prepared.rows.clone();
            changed[0].canonical = sealed_getter;
            assert!(!same_rows(&prepared.rows, &changed));
        }
    }
    // Negative operations above never commit or prefill any genuine cell.
    open(&prepared.rows).unwrap();
    HITS.with(|v| v.set(v.get() + 1));
    CASE.with(|v| v.set(Some(case)));
}

/// The checkpoint below also comes only from the real original actor finish.
pub(super) fn observe_commit(
    prepared: &Prepared,
    program: &Program,
    types: &TypeInterner,
    sources: &Arc<Vec<Function>>,
    checkpoint: &crate::actor_handlers::InitialPeerCheckpoint,
) {
    let Some(case) = CASE.with(|v| v.take()) else {
        return;
    };
    open(&prepared.rows).unwrap();
    prepared
        .preflight_commit(program, types, sources, checkpoint)
        .unwrap();
    if matches!(case, Case::Cells) {
        let copied = Arc::new(sources.as_ref().clone());
        assert!(!Arc::ptr_eq(&copied, sources));
        assert!(
            prepared
                .preflight_commit(program, types, &copied, checkpoint)
                .is_err()
        );
        let mut changed = program.clone();
        let owner = changed
            .functions
            .iter_mut()
            .find(|f| f.id == prepared.rows[0].function)
            .unwrap();
        let StatementKind::Let { value, .. } = &mut owner.body.statements[0].kind else {
            panic!();
        };
        value.span.end += 1;
        assert!(
            prepared
                .preflight_commit(&changed, types, sources, checkpoint)
                .is_err()
        );
        let mut sibling = program.clone();
        let unrelated = sibling
            .functions
            .iter_mut()
            .find(|f| f.id != prepared.rows[0].function)
            .unwrap();
        unrelated.span.end += 1;
        assert!(
            prepared
                .preflight_commit(&sibling, types, sources, checkpoint)
                .is_err(),
            "whole sibling header is part of the actual checkpoint"
        );
        let mut appended = program.clone();
        appended.functions.push(program.functions[0].clone());
        assert!(
            prepared
                .preflight_commit(&appended, types, sources, checkpoint)
                .is_err()
        );
    }
    open(&prepared.rows).unwrap();
    COMMITS.with(|v| v.set(v.get() + 1));
    CASE.with(|v| v.set(Some(case)));
}

#[test]
fn packet_original_finalization_uses_genuine_before_values_and_is_idempotent() {
    for release in [false, true] {
        let (mut program, checked) = observed(Case::State, SOURCE, release);
        let types = &checked.checked().interner;
        let proofs = temporary_local_backings(&program, main(&program), types).unwrap();
        assert_eq!(proofs.len(), 1);
        let canonical = proofs[0].owner_initializer().clone();
        assert!(proofs[0].0.canonical_producer.get().is_some());
        crate::complete_value_conversions(&mut program, types).unwrap();
        let first = program.clone();
        crate::complete_value_conversions(&mut program, types).unwrap();
        assert!(resource_materialization::functions_equal(
            &first.functions,
            &program.functions
        ));
        crate::validate_backend_types(&program, types).unwrap();
        program
            .resource_source
            .validate_current(&program, types)
            .unwrap();
        assert!(resource_materialization::expression_equal(
            &canonical,
            proofs[0].owner_initializer()
        ));
        assert!(seal_original_program(&program, types).is_err());
        assert!(resource_materialization::expression_equal(
            &canonical,
            proofs[0].owner_initializer()
        ));
    }
}
#[test]
fn actual_preflight_refuses_foreign_context_inventory_and_table_without_cell_writes() {
    for release in [false, true] {
        observed(Case::Context, SOURCE, release);
    }
}
#[test]
fn hidden_earlier_argument_closure_allows_repeated_uses_and_rejects_forward_edges() {
    for release in [false, true] {
        observed(Case::Dependencies, CHAIN_SOURCE, release);
    }
}
#[test]
fn actual_finalization_preflight_checks_open_cells_exact_source_arc_and_current_producer() {
    for release in [false, true] {
        observed(Case::Cells, SOURCE, release);
    }
}
#[test]
fn sealed_current_payload_cannot_replace_the_genuine_before_canonical() {
    for release in [false, true] {
        observed(Case::Payload, SOURCE, release);
    }
}
#[test]
fn genuine_actor_and_earlier_temporary_graphs_expire_after_all_archive_owners_drop() {
    for release in [false, true] {
        let (program, checked) = high(CHAIN_SOURCE, release);
        let proofs =
            temporary_local_backings(&program, main(&program), &checked.checked().interner)
                .unwrap();
        assert_eq!(proofs.len(), 2);
        let weak = proofs
            .iter()
            .map(|p| Arc::downgrade(&p.0))
            .collect::<Vec<_>>();
        let archive = program.resource_source.clone();
        let clone = program.clone();
        drop(proofs);
        drop(program);
        drop(checked);
        assert!(weak.iter().all(|w| w.upgrade().is_some()));
        drop(clone);
        assert!(
            weak.iter().all(|w| w.upgrade().is_some()),
            "real archive retains the proofs"
        );
        drop(archive);
        assert!(
            weak.iter().all(|w| w.upgrade().is_none()),
            "frozen earlier proofs and actor edge have no ownership cycle"
        );
    }
}

#[test]
fn actual_original_finalization_retains_preseal_interface_coercion() {
    for release in [false, true] {
        let (mut program, checked) = observed(Case::Coercion, INTERFACE_SOURCE, release);
        let types = &checked.checked().interner;
        let proofs = temporary_local_backings(&program, main(&program), types).unwrap();
        let [proof] = proofs.as_slice() else {
            panic!();
        };
        assert!(proof.0.canonical_producer.get().is_some());
        let ExpressionKind::Call { args, .. } = &proof.owner_initializer().kind else {
            panic!();
        };
        assert!(matches!(
            args[0].kind,
            ExpressionKind::InterfaceCoerce { .. }
        ));
        assert!(!resource_materialization::expression_equal(
            proof.owner_initializer(),
            &proof.0.producer
        ));
        crate::complete_value_conversions(&mut program, types).unwrap();
        crate::validate_backend_types(&program, types).unwrap();
        program
            .resource_source
            .validate_current(&program, types)
            .unwrap();
    }
}
