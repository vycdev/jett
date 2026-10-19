use super::*;
use std::collections::HashMap;

const SOURCE: &str = r#"namespace app
struct Leaf:
    code: int64
struct Item:
    leaf: Leaf
enum Choice:
    first(item: Item)
    second(item: Item)
function read_leaf(view leaf: Leaf) returns int64:
    return leaf.code
function read(view item: Item) returns int64:
    return read_leaf(view item.leaf)
function take(item: Item) returns int64:
    return item.leaf.code
function identity(choice: Choice) returns Choice:
    return choice
function exercise(view choice: Choice, view steps: list[int64]) returns int64:
    mutable int64 total = 0
    match view choice:
        first(item):
            Item alias = view item
            total = read(view item) + read_leaf(view item.leaf)
            for step in view steps:
                total = total + read(view alias) + step
        second(item):
            total = read(view item)
    return total
function called(view choice: Choice, view steps: list[int64]) returns int64:
    mutable int64 total = 0
    for step in view steps:
        match identity(clone choice):
            first(item):
                total = total + read(view item) + step
            second(item):
                total = total + read_leaf(view item.leaf) + step
    return total
function owned(choice: Choice) returns int64:
    match choice:
        first(item):
            return take(item)
        second(item):
            return take(item)
"#;

fn source_program() -> (Program, TypeInterner) {
    let file = jett_common::FileId::new(0);
    let parsed = jett_parser::parse(SOURCE, file);
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
    .expect("exact checked HIR");
    (
        crate::lower(&high, &checked.interner).expect("exact original match MIR"),
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

#[test]
fn match_binding_transport_preserves_owned_modes_and_native_preparation() {
    let (mut program, types) = source_program();
    crate::validate_call_ownership(&program, &types)
        .expect("direct, projected, alias and owned binders");
    let function = exercise(&program);
    let witness = function.match_bindings.as_ref().unwrap();
    let arm = &witness.matches[0].arms[0];
    let proved = scopes(function, &types).unwrap();
    let binder = arm.bindings[0].0.id;
    let alias = function
        .locals
        .iter()
        .find(|local| local.name == "alias")
        .unwrap()
        .id;
    ensure_scope(function, &proved, binder, arm.entry).unwrap();
    ensure_scope(function, &proved, alias, arm.entry).unwrap();
    let outside = function
        .blocks
        .iter()
        .find(|block| !arm.region.contains(&block.id) && block.id != witness.matches[0].header)
        .unwrap()
        .id;
    assert!(ensure_scope(function, &proved, alias, outside).is_err());
    let owned = program
        .functions
        .iter()
        .find(|f| f.identity.declaration.name == "owned")
        .unwrap();
    crate::move_values::MoveValuePlan::analyze(&program, owned, &types)
        .expect("MatchBinding alone does not turn Source Owned into borrowed");
    crate::prepare_native_sequences(&mut program, &types);
    assert!(
        exercise(&program)
            .blocks
            .iter()
            .all(|block| !matches!(block.terminator.kind, TerminatorKind::ForEach { .. }))
    );
    let called = program
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "called")
        .unwrap();
    assert!(
        called
            .blocks
            .iter()
            .all(|block| !matches!(block.terminator.kind, TerminatorKind::ForEach { .. })),
        "call-produced scrutinee composition must actually prepare its viewed loop"
    );
    scopes(called, &types).expect("exact selecting call survives canonical sequence edits");
    crate::prepare_native_uninhabited_sums(&mut program, &types);
    crate::prepare_native_generated_functions(&mut program, &types);
    crate::validate_call_ownership(&program, &types).expect("canonical match region remaps");
    let function = exercise_mut(&mut program);
    assert!(crate::sequences::prune::unused_locals(function));
    scopes(function, &types).expect("match proof survives dense local remap");
    crate::validate_call_ownership(&program, &types).unwrap();
}

#[test]
fn original_match_transport_rejects_public_selector_binding_and_scope_corruption() {
    let (original, types) = source_program();
    for case in 0..7 {
        let mut program = original.clone();
        let function = exercise_mut(&mut program);
        let record = function.match_bindings.as_ref().unwrap().matches[0].clone();
        match case {
            0 => function.match_bindings = None,
            1 => {
                let TerminatorKind::Switch { variants, .. } = &mut function.blocks
                    [record.header.index() as usize]
                    .terminator
                    .kind
                else {
                    unreachable!()
                };
                variants[0].0 = variants[1].0;
            }
            2 => {
                let TerminatorKind::Switch { variants, .. } = &mut function.blocks
                    [record.header.index() as usize]
                    .terminator
                    .kind
                else {
                    unreachable!()
                };
                let first = variants[0].2[0];
                variants[0].2[0] = variants[1].2[0];
                variants[1].2[0] = first;
            }
            3 => {
                let binder = record.arms[0].bindings[0].0.id;
                function.locals[binder.index() as usize].span = record.arms[1].bindings[0].0.span;
            }
            4 => {
                let binder = record.arms[0].bindings[0].0.id;
                function.blocks[record.arms[0].entry.index() as usize]
                    .statements
                    .insert(
                        0,
                        Statement {
                            kind: StatementKind::Let {
                                local: binder,
                                value: Expression {
                                    kind: hir::ExpressionKind::Nothing,
                                    ty: function.locals[binder.index() as usize].ty,
                                    span: function.locals[binder.index() as usize].span,
                                },
                            },
                            span: record.selector.span,
                        },
                    );
            }
            5 => {
                // The exact original invocation moves beyond its arm. No proof
                // may be recreated from the destination block's types or names.
                let arm = &record.arms[1];
                let entry = arm.entry.index() as usize;
                let statement = function.blocks[entry]
                    .statements
                    .iter()
                    .position(|statement| {
                        let mut call = false;
                        let block = BasicBlock {
                            id: arm.entry,
                            statements: vec![statement.clone()],
                            terminator: Terminator {
                                kind: TerminatorKind::Unreachable,
                                span: statement.span,
                            },
                        };
                        crate::resource_ownership::visit_block(&block, &mut |value| {
                            call |= matches!(value.kind, hir::ExpressionKind::Call { .. })
                        });
                        call
                    })
                    .expect("second arm has its original invocation");
                let moved = function.blocks[entry].statements.remove(statement);
                let join = function
                    .blocks
                    .iter()
                    .find(|block| {
                        !record.arms.iter().any(|arm| arm.region.contains(&block.id))
                            && block.id != record.header
                            && matches!(block.terminator.kind, TerminatorKind::Return(_))
                    })
                    .unwrap()
                    .id;
                function.blocks[join.index() as usize]
                    .statements
                    .insert(0, moved);
            }
            _ => {
                let entry = record.arms[0].entry;
                // An external CFG entry cannot acquire the selected variant.
                function.entry = entry;
            }
        }
        if (1..=4).contains(&case) {
            assert!(
                scopes(function, &types).is_err(),
                "independent original match association must reject case {case}"
            );
        }
        assert!(
            crate::validate_call_ownership(&program, &types).is_err(),
            "forged match case {case}"
        );
    }
}

#[test]
fn transparent_match_views_snapshot_the_original_owner_before_payload_take() {
    for (declaration, input, scrutinee) in [
        ("", "Choice", "(view choice)"),
        ("", "secret[Choice]", "declassify(view choice)"),
        (
            "type RefinedChoice = Choice where true\n",
            "RefinedChoice",
            "coarsen(view choice)",
        ),
    ] {
        // Same actual transparent-view Source forms as the HIR controls. Add
        // a second invocation to keep original ownership observably necessary.
        let source = format!(
            "namespace app\nstruct Payload:\n    values: list[int64]\nenum Choice:\n    item(data: Payload)\n{declaration}function observe(view data: Payload) returns int64:\n    return list.length[int64](view data.values)\nfunction inspect(view choice: {input}) returns int64:\n    match {scrutinee}:\n        item(data):\n            return observe(view data)\nfunction twice(view choice: {input}) returns int64:\n    return inspect(view choice) + inspect(view choice)\n"
        );
        let project = jett_common::FileId::new(0);
        let list = jett_common::FileId::new(jett_common::STDLIB_FILE_ID_START);
        let mut parsed = jett_parser::parse(include_str!("../../../../stdlib/list.jett"), list);
        let mut primary = jett_parser::parse(&source, project);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        assert!(primary.errors.is_empty(), "{:?}", primary.errors);
        parsed.module.items.append(&mut primary.module.items);
        let resolved = jett_resolve::resolve(&parsed.module);
        let checked = jett_typecheck::check(&parsed.module, &resolved);
        assert!(
            checked
                .diagnostics
                .iter()
                .all(|d| d.severity != jett_diagnostics::Severity::Error),
            "{scrutinee}: {:?}",
            checked.diagnostics
        );
        let high = hir::lower(
            &parsed.module,
            &resolved,
            &checked,
            &HashMap::from([
                (project, jett_common::SourceOrigin::Project),
                (list, jett_common::SourceOrigin::Stdlib),
            ]),
        )
        .expect("actual transparent-view Source HIR");
        let mut program =
            crate::lower(&high, &checked.interner).expect("original owner match snapshot MIR");
        let inspect = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "inspect")
            .unwrap();
        let parameter = inspect.params[0].local;
        let selector = inspect
            .blocks
            .iter()
            .find_map(|block| match &block.terminator.kind {
                TerminatorKind::Switch { scrutinee, .. } => Some(scrutinee),
                _ => None,
            })
            .unwrap();
        let hir::ExpressionKind::Clone(snapshot) = &selector.kind else {
            panic!("{scrutinee}: payload Switch must own an explicit independent snapshot")
        };
        let mut backing = snapshot.as_ref();
        while let hir::ExpressionKind::View(value)
        | hir::ExpressionKind::Coarsen(value)
        | hir::ExpressionKind::Declassify(value) = &backing.kind
        {
            backing = value;
        }
        assert!(matches!(backing.kind, hir::ExpressionKind::Local(local) if local == parameter));
        crate::move_values::MoveValuePlan::analyze(&program, inspect, &checked.interner)
            .expect("Switch clone observes and preserves the original view owner");
        let twice = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "twice")
            .unwrap();
        crate::move_values::MoveValuePlan::analyze(&program, twice, &checked.interner)
            .expect("the same source owner remains available to both inspections");
        crate::prepare_native_sequences(&mut program, &checked.interner);
        crate::validate_call_ownership(&program, &checked.interner)
            .expect("transparent snapshot retains exact original arm authority");
    }
}
