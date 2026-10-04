use super::*;
use std::collections::HashMap;

const SOURCE: &str = r#"namespace app
struct Item:
    code: int64
function inspect(view item: Item) returns int64:
    return item.code
function exercise(view items: list[Item], view other: list[Item]) returns int64:
    mutable int64 total = 0
    for item in view items:
        Item alias = view item
        total = total + inspect(view alias)
    return total
"#;

fn source_program(source: &str) -> (Program, TypeInterner) {
    let file = jett_common::FileId::new(0);
    let parsed = jett_parser::parse(source, file);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let resolved = jett_resolve::resolve(&parsed.module);
    let checked = jett_typecheck::check(&parsed.module, &resolved);
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
        &HashMap::from([(file, jett_common::SourceOrigin::Project)]),
    )
    .expect("source HIR");
    (
        crate::lower(&high, &checked.interner).expect("source MIR"),
        checked.interner,
    )
}

fn exercise(program: &Program) -> &Function {
    program
        .functions
        .iter()
        .find(|f| f.identity.declaration.name == "exercise")
        .unwrap()
}

fn exercise_mut(program: &mut Program) -> &mut Function {
    program
        .functions
        .iter_mut()
        .find(|f| f.identity.declaration.name == "exercise")
        .unwrap()
}

fn rejects(program: &Program, types: &TypeInterner, case: impl std::fmt::Display) {
    let error = match crate::validate_call_ownership(program, types) {
        Err(error) => error,
        Ok(()) => panic!("forged iteration provenance accepted case {case}"),
    };
    assert!(
        error.iter().any(|e| e.message.contains("call ownership")
            || e.message.contains("call view")
            || e.message.contains("viewed iteration")),
        "{error:?}"
    );
}

#[test]
fn viewed_iteration_provenance_survives_native_sequence_preparation() {
    let (mut program, types) = source_program(SOURCE);
    assert!(exercise(&program).prepared_view_iterations.is_empty());
    crate::validate_call_ownership(&program, &types).expect("original typed loop");
    crate::prepare_native_sequences(&mut program, &types);
    let function = exercise(&program);
    assert_eq!(function.prepared_view_iterations.len(), 1);
    let record = &function.prepared_view_iterations[0];
    let alias = function.locals.iter().find(|l| l.name == "alias").unwrap();
    let proved = scopes(function, &types).unwrap();
    ensure_scope(function, &proved, alias.id, record.original.body)
        .expect("forwarded alias inside exact loop");
    assert!(ensure_scope(function, &proved, alias.id, record.original.exit).is_err());
    crate::validate_call_ownership(&program, &types).expect("prepared typed loop");
    let once = program.clone();
    crate::prepare_native_sequences(&mut program, &types);
    assert_eq!(program, once, "preparation is idempotent");
    crate::sequences::prune::unused_locals(exercise_mut(&mut program));
    crate::validate_call_ownership(&program, &types).expect("metadata follows local compaction");
}

#[test]
fn viewed_iteration_prepared_association_rejects_changed_extraction_and_loans() {
    let (mut original, types) = source_program(SOURCE);
    crate::prepare_native_sequences(&mut original, &types);
    assert_eq!(exercise(&original).prepared_view_iterations.len(), 1);
    for case in 0..12 {
        let mut program = original.clone();
        let function = exercise_mut(&mut program);
        let record = function.prepared_view_iterations[0].clone();
        match case {
            0 => function.prepared_view_iterations.clear(),
            1 => function.prepared_view_iterations.push(record.clone()),
            2..=6 => {
                let StatementKind::SequenceGet {
                    consume,
                    source,
                    index,
                    target,
                    part,
                } = &mut function.blocks[record.original.body.index() as usize].statements[0].kind
                else {
                    panic!("exact extraction")
                };
                match case {
                    2 => *consume = true,
                    3 => *source = SequenceSource::Local(function.params[1].local),
                    4 => *index = record.length,
                    5 => *target = function.params[0].local,
                    6 => *part = SequencePart::Value,
                    _ => unreachable!(),
                }
            }
            7 => {
                let start = function.blocks[record.preheader.index() as usize]
                    .statements
                    .iter_mut()
                    .find(|s| matches!(s.kind, StatementKind::IterationBorrow { start: true, .. }))
                    .unwrap();
                let StatementKind::IterationBorrow { token, .. } = &mut start.kind else {
                    unreachable!()
                };
                *token = record.length;
            }
            8 => {
                let end = function.blocks.iter_mut().find(|b| b.statements.iter().any(|s|
                    matches!(s.kind, StatementKind::IterationBorrow { token, start: false, .. } if token == record.cursor))).unwrap();
                end.statements.clear();
            }
            9 => function.prepared_view_iterations[0].original.function = FunctionId::new(999),
            10 => {
                let TerminatorKind::Branch { else_block, .. } = &mut function.blocks
                    [record.original.header.index() as usize]
                    .terminator
                    .kind
                else {
                    unreachable!()
                };
                *else_block = record.original.body;
            }
            11 => {
                let end = function.blocks.iter_mut().find(|b| b.statements.iter().any(|s|
                    matches!(s.kind, StatementKind::IterationBorrow { token, start: false, .. } if token == record.cursor))).unwrap();
                end.terminator.kind = TerminatorKind::Goto(record.original.header);
            }
            _ => unreachable!(),
        }
        rejects(&program, &types, case);
    }
}

#[test]
fn viewed_iteration_original_association_rejects_wrong_binder_and_mode() {
    let (original, types) = source_program(SOURCE);
    for case in 0..3 {
        let mut program = original.clone();
        let function = exercise_mut(&mut program);
        let header = function
            .blocks
            .iter_mut()
            .find(|b| matches!(b.terminator.kind, TerminatorKind::ForEach { .. }))
            .unwrap();
        let TerminatorKind::ForEach {
            key,
            by_view,
            iterable,
            ..
        } = &mut header.terminator.kind
        else {
            unreachable!()
        };
        match case {
            0 => *by_view = false,
            1 => *key = function.params[0].local,
            2 => iterable.ty = TypeInterner::STRING,
            _ => unreachable!(),
        }
        rejects(&program, &types, case);
    }
}

#[test]
fn viewed_iteration_map_roles_and_nested_binders_remain_exact() {
    let source = r#"namespace app
struct Item:
    code: int64
function inspect(view item: Item) returns int64:
    return item.code
function exercise(view groups: map[string, list[Item]]) returns int64:
    mutable int64 total = 0
    for key, items in view groups:
        for item in view items:
            total = total + inspect(view item)
    return total
"#;
    let (mut program, types) = source_program(source);
    crate::prepare_native_sequences(&mut program, &types);
    assert_eq!(exercise(&program).prepared_view_iterations.len(), 2);
    crate::validate_call_ownership(&program, &types).expect("nested exact regions and map roles");
    let function = exercise_mut(&mut program);
    let outer = function
        .prepared_view_iterations
        .iter()
        .find(|r| r.original.bindings.len() == 2)
        .unwrap()
        .clone();
    let StatementKind::SequenceGet { part, .. } =
        &mut function.blocks[outer.original.body.index() as usize].statements[1].kind
    else {
        panic!("map value extraction")
    };
    *part = SequencePart::Key;
    rejects(&program, &types, "map value role");
}
