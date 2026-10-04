use super::*;
use jett_common::{FileId, SourceOrigin};
use std::collections::HashMap;

const CONDITIONAL: &str = r#"namespace app
struct User:
    name: string
struct Alternate:
    code: int64
function inspect_builder(view builder: TypeConstruction, extra: int64) returns int64:
    trace builder
    return extra
function exercise(incoming: optional[int64]) returns int64:
    mutable TypeConstruction switchable = type.construct_start[User]()
    int64 selected = inspect_builder(view switchable, incoming handle:
        trace switchable
        switchable = type.construct_start[Alternate]()
        trace switchable
        default 7
    )
    trace switchable
    return selected
function main() returns nothing:
    int64 absent = exercise(none)
    trace absent
    int64 present = exercise(some(11))
    trace present
    return nothing
"#;

const ABORT: &str = r#"namespace app
struct User:
    name: string
struct Alternate:
    code: int64
function inspect_builder(view builder: TypeConstruction, extra: int64) returns int64:
    trace builder
    return extra
function exercise(incoming: optional[int64]) returns int64:
    mutable TypeConstruction switchable = type.construct_start[User]()
    int64 selected = inspect_builder(view switchable, incoming handle:
        switchable = type.construct_start[Alternate]()
        trace switchable
        return 13
    )
    trace switchable
    return selected
function main() returns nothing:
    int64 aborted = exercise(none)
    trace aborted
    return nothing
"#;

fn source_program(source: &str, release: bool) -> (Program, TypeInterner) {
    let file = FileId::new(0);
    let parsed = jett_parser::parse(source, file);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let resolved = jett_resolve::resolve(&parsed.module);
    assert!(
        resolved
            .diagnostics
            .iter()
            .all(|d| d.severity != jett_diagnostics::Severity::Error),
        "{:?}",
        resolved.diagnostics
    );
    let checked = jett_typecheck::check_with_options(
        &parsed.module,
        &resolved,
        jett_typecheck::CheckOptions { release },
    );
    assert!(
        checked
            .diagnostics
            .iter()
            .all(|d| d.severity != jett_diagnostics::Severity::Error),
        "{:?}",
        checked.diagnostics
    );
    let high = hir::lower(
        &parsed.module,
        &resolved,
        &checked,
        &HashMap::from([(file, SourceOrigin::Project)]),
    )
    .expect("checked Source HIR");
    let program = crate::lower(&high, &checked.interner).expect("constructor-owned generation MIR");
    crate::validate_call_ownership(&program, &checked.interner)
        .expect("all original Source and generation proofs");
    (program, checked.interner)
}
fn exercise(program: &Program) -> &Function {
    program
        .functions
        .iter()
        .find(|f| f.identity.declaration.name == "exercise")
        .expect("exercise")
}
fn exercise_mut(program: &mut Program) -> &mut Function {
    program
        .functions
        .iter_mut()
        .find(|f| f.identity.declaration.name == "exercise")
        .expect("exercise")
}
fn prepared(program: &mut Program, types: &TypeInterner) {
    crate::prepare_native_sequences(program, types);
    crate::prepare_native_uninhabited_sums(program, types);
    crate::prepare_native_generated_functions(program, types);
    crate::validate_call_ownership(program, types)
        .expect("fresh prepared Source and generation proofs");
}
fn rejects(program: &Program, types: &TypeInterner, expected: &str) {
    let message = validate(exercise(program), types)
        .expect_err("corrupted generation must fail independently");
    assert!(message.contains(expected), "{message}");
    assert!(
        crate::validate_call_ownership(program, types).is_err(),
        "outer complete gate also refuses"
    );
}

#[test]
fn generation_conditional_replacement_preserves_current_reads_and_captured_owner() {
    for release in [false, true] {
        let (mut program, types) = source_program(CONDITIONAL, release);
        let function = exercise(&program);
        assert_eq!(function.call_owner_generations.len(), 1);
        let record = &function.call_owner_generations[0];
        assert_eq!(record.captures.len(), 1);
        assert_eq!(record.replacements.len(), 1);
        let loan = record.captures[0].loan;
        let root = record.root;
        // The actual capture is exactly a View(Local), never a new owner clone.
        let capture = &function.blocks[record.captures[0].begin.block.index() as usize].statements
            [record.captures[0].begin.statement];
        assert!(
            matches!(&capture.kind, StatementKind::BeginCallView { value, .. } if matches!(&value.kind, E::View(raw) if matches!(raw.kind, E::Local(local) if local == root)))
        );
        let plan = crate::move_values::MoveValuePlan::analyze(&program, function, &types)
            .expect("scoped old owner survives replacement without cloning");
        let slot = plan.generation_storage().slots()[0];
        assert_eq!(slot.root(), root);
        assert_eq!(slot.generation().index(), 0);
        assert_eq!(slot.escrow().index(), 0);
        assert!(
            plan.generation_storage()
                .covers_loan(slot.generation(), loan)
        );
        prepared(&mut program, &types);
        crate::move_values::MoveValuePlan::analyze(&program, exercise(&program), &types)
            .expect("fresh prepared generation and liveness");
    }
}

#[test]
fn generation_abort_ends_loans_before_cleanup_and_never_requires_callee_entry() {
    for release in [false, true] {
        let (mut program, types) = source_program(ABORT, release);
        let function = exercise(&program);
        let record = &function.call_owner_generations[0];
        let abort = function.blocks.iter().find(|block| matches!(&block.terminator.kind, TerminatorKind::Return(Some(value)) if matches!(value.kind, E::Int(13)))).expect("source handler return13");
        let close = abort.statements.iter().position(|s| matches!(s.kind, StatementKind::CloseCallOwnerGeneration { generation } if generation == record.id)).expect("abort closes escrow");
        assert!(
            close > 0
                && matches!(abort.statements[close - 1].kind, StatementKind::EndCallView { local } if local == record.captures[0].loan)
        );
        crate::move_values::MoveValuePlan::analyze(&program, function, &types)
            .expect("abort cleanup has no retained owning capture");
        prepared(&mut program, &types);
        crate::move_values::MoveValuePlan::analyze(&program, exercise(&program), &types)
            .expect("prepared abort cleanup");
    }
}

#[test]
fn generation_repeated_replacements_keep_one_captured_escrow() {
    let source = CONDITIONAL.replace("        trace switchable\n        default 7", "        trace switchable\n        switchable = type.construct_start[User]()\n        trace switchable\n        default 7");
    for release in [false, true] {
        let (program, types) = source_program(&source, release);
        let function = exercise(&program);
        assert_eq!(function.call_owner_generations.len(), 1);
        assert_eq!(function.call_owner_generations[0].replacements.len(), 2);
        let plan = crate::move_values::MoveValuePlan::analyze(&program, function, &types)
            .expect("two replacements preserve the first captured generation");
        assert_eq!(plan.generation_storage().slots().len(), 1);
    }
}

#[test]
fn generation_operations_need_exact_private_sites_identity_and_independent_rhs() {
    let (program, types) = source_program(CONDITIONAL, false);
    for mutation in 0..6 {
        let mut altered = program.clone();
        let function = exercise_mut(&mut altered);
        let record = function.call_owner_generations[0].clone();
        match mutation {
            0 => {
                function.blocks[record.start.block.index() as usize]
                    .statements
                    .remove(record.start.statement);
            }
            1 => {
                let value = function.blocks[record.start.block.index() as usize].statements
                    [record.start.statement]
                    .clone();
                function.blocks[record.start.block.index() as usize]
                    .statements
                    .push(value);
            }
            2 => {
                let site = record.replacements[0].replacement;
                let StatementKind::ReplaceCallOwnerGeneration { rhs_owner, .. } =
                    &mut function.blocks[site.block.index() as usize].statements[site.statement]
                        .kind
                else {
                    panic!("Replace")
                };
                *rhs_owner = record.root;
            }
            3 => {
                let site = record.replacements[0].acquired;
                let StatementKind::Let { value, .. } =
                    &mut function.blocks[site.block.index() as usize].statements[site.statement]
                        .kind
                else {
                    panic!("acquired Let")
                };
                value.kind = E::Local(record.captures[0].loan);
            }
            4 => {
                let last = function
                    .blocks
                    .iter_mut()
                    .find(|block| {
                        block.statements.iter().any(|s| {
                            matches!(s.kind, StatementKind::CloseCallOwnerGeneration { .. })
                        })
                    })
                    .expect("close block");
                let value = last
                    .statements
                    .iter()
                    .find(|s| matches!(s.kind, StatementKind::CloseCallOwnerGeneration { .. }))
                    .unwrap()
                    .clone();
                last.statements.push(value);
            }
            5 => {
                function.call_owner_generations[0].owner = program
                    .functions
                    .iter()
                    .find(|f| f.id != function.id)
                    .unwrap()
                    .id;
            }
            _ => unreachable!(),
        }
        rejects(
            &altered,
            &types,
            if mutation == 5 {
                "foreign"
            } else {
                "constructor-owned"
            },
        );
    }
}

#[test]
fn generation_disconnected_copy_and_alternate_scope_entry_are_not_authority() {
    let (program, types) = source_program(CONDITIONAL, false);
    let mut copy = program.clone();
    let function = exercise_mut(&mut copy);
    let record = function.call_owner_generations[0].clone();
    let close = function
        .blocks
        .iter()
        .flat_map(|b| &b.statements)
        .find(|s| matches!(s.kind, StatementKind::CloseCallOwnerGeneration { .. }))
        .unwrap()
        .clone();
    let block = BlockId(function.blocks.len() as u32);
    function.blocks.push(BasicBlock {
        id: block,
        statements: vec![close],
        terminator: Terminator {
            kind: TerminatorKind::Unreachable,
            span: function.span,
        },
    });
    rejects(&copy, &types, "constructor-owned");

    let mut entry = program.clone();
    let function = exercise_mut(&mut entry);
    let block = BlockId(function.blocks.len() as u32);
    function.blocks.push(BasicBlock {
        id: block,
        statements: Vec::new(),
        terminator: Terminator {
            kind: TerminatorKind::Goto(record.replacements[0].replacement.block),
            span: function.span,
        },
    });
    rejects(&entry, &types, "CFG");
}

#[test]
fn generation_canonical_loop_and_dense_compaction_preserve_archives_without_roots() {
    let source = CONDITIONAL.replace("        trace switchable\n        default 7", "        trace switchable\n        for item in view values:\n            trace switchable\n        default 7").replace("    int64 selected =", "    list[int64] values = list(1, 2)\n    int64 selected =");
    for release in [false, true] {
        let (mut program, types) = source_program(&source, release);
        let original = exercise(&program).call_owner_generations[0].clone();
        let function = exercise_mut(&mut program);
        let extra = LocalId::new(function.locals.len() as u32);
        function.locals.push(Local {
            id: extra,
            name: "unused_generation_control".to_string(),
            debug_type_name: None,
            ty: TypeInterner::INT64,
            debug_ty: TypeInterner::INT64,
            mutable: false,
            span: function.span,
            view_source: None,
        });
        assert!(
            function
                .local(extra)
                .is_some_and(|local| local.name == "unused_generation_control")
        );
        let block = BlockId(function.blocks.len() as u32);
        function.blocks.push(BasicBlock {
            id: block,
            statements: Vec::new(),
            terminator: Terminator {
                kind: TerminatorKind::Unreachable,
                span: function.span,
            },
        });
        crate::validate_call_ownership(&program, &types)
            .expect("unused metadata creates no execution authority");
        prepared(&mut program, &types);
        let function = exercise(&program);
        let record = &function.call_owner_generations[0];
        assert!(
            crate::breakpoint_regions::expression_equal(
                &original.original_call,
                &record.original_call
            ),
            "original call archive is immutable"
        );
        assert_eq!(
            record.original_root, original.original_root,
            "Source binding archive is immutable"
        );
        assert_eq!(
            record.original_root_header, original.original_root_header,
            "full local header archive is immutable"
        );
        assert_eq!(
            record
                .replacements
                .iter()
                .map(|replacement| &replacement.original)
                .collect::<Vec<_>>(),
            original
                .replacements
                .iter()
                .map(|replacement| &replacement.original)
                .collect::<Vec<_>>(),
            "original source assignment archives are immutable"
        );
        // Sequence lowering adds live cursor/length locals, so total count
        // cannot prove whether this particular unused seed was removed.
        assert!(
            function
                .locals
                .iter()
                .all(|local| local.name != "unused_generation_control")
        );
        assert!(
            function
                .locals
                .iter()
                .enumerate()
                .all(|(index, local)| local.id == LocalId::new(index as u32)),
            "all surviving current local IDs remain dense"
        );
        assert!(
            function
                .blocks
                .iter()
                .all(|b| !matches!(b.terminator.kind, TerminatorKind::ForEach { .. }))
        );
        crate::move_values::MoveValuePlan::analyze(&program, function, &types)
            .expect("canonical sequence addition and compaction rejoin every generation site");
    }
}

#[test]
fn generation_source_name_and_full_local_headers_remain_independent_and_exact() {
    for release in [false, true] {
        let (mut program, types) = source_program(CONDITIONAL, release);
        let function = exercise(&program);
        let record = &function.call_owner_generations[0];
        let source_header = record.original_root;
        let local_header = record.original_root_header.clone();
        assert_eq!(function.local(record.root), Some(&local_header));
        assert_eq!(source_header.local, local_header.id);
        assert_eq!(source_header.declaration_span.file, local_header.span.file);
        assert!(local_header.span.start < source_header.declaration_span.start);
        assert!(source_header.declaration_span.end < local_header.span.end);
        assert_eq!(
            &CONDITIONAL[source_header.declaration_span.start as usize
                ..source_header.declaration_span.end as usize],
            "switchable"
        );
        assert!(
            CONDITIONAL[local_header.span.start as usize..local_header.span.end as usize]
                .starts_with("mutable TypeConstruction switchable =")
        );
        validate(function, &types)
            .expect("each original checked header has its own exact coordinate role");
        prepared(&mut program, &types);
        let record = &exercise(&program).call_owner_generations[0];
        assert_eq!(
            record.original_root, source_header,
            "Source identifier fact stays original"
        );
        assert_eq!(
            record.original_root_header, local_header,
            "whole declaration archive never remaps"
        );
        crate::move_values::MoveValuePlan::analyze(&program, exercise(&program), &types)
            .expect("fresh prepared storage retains both independent headers");

        for mutation in 0..6 {
            let mut altered = program.clone();
            let function = exercise_mut(&mut altered);
            let root = function.call_owner_generations[0].root;
            match mutation {
                0 => {
                    // This span still contains the original name; containment
                    // alone must not replace exact sealed-header validation.
                    function.locals[root.index() as usize].span.start += 1;
                }
                1 => function.locals[root.index() as usize]
                    .name
                    .push_str("_foreign"),
                2 => {
                    function.locals[root.index() as usize].debug_type_name =
                        Some("ForeignHeader".into())
                }
                3 => {
                    function.call_owner_generations[0]
                        .original_root
                        .declaration_span
                        .start += 1
                }
                4 => {
                    let header = &mut function.call_owner_generations[0].original_root_header;
                    header.id = LocalId::new(header.id.index() + 1);
                }
                5 => {
                    function.call_owner_generations[0]
                        .original_root_header
                        .span
                        .start += 1
                }
                _ => unreachable!(),
            }
            rejects(
                &altered,
                &types,
                match mutation {
                    3 => "immutable original source witness",
                    4 => "sealed original source or local header",
                    _ => "sealed local declaration header",
                },
            );
        }
    }
}
