use super::*;
use jett_hir::ExpressionKind;
use std::collections::HashMap;

const PRELUDE: &str = r#"namespace app
interface Named:
    function name(view self: Named) returns string
implement Named for int64:
    function name(view self: int64) returns string:
        return "member"
struct Packet:
    member: Named
function report(view item: Named, extra: int64) returns string:
    return Named.name(view item)
"#;

fn source_program(body: &str) -> (Program, TypeInterner) {
    let file = jett_common::FileId::new(0);
    let source = format!("{PRELUDE}{body}");
    let parsed = jett_parser::parse(&source, file);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let resolved = jett_resolve::resolve(&parsed.module);
    let checked = jett_typecheck::check(&parsed.module, &resolved);
    assert!(
        checked
            .diagnostics
            .iter()
            .all(|item| item.severity != jett_diagnostics::Severity::Error),
        "{:?}",
        checked.diagnostics
    );
    let hir = hir::lower(
        &parsed.module,
        &resolved,
        &checked,
        &HashMap::from([(file, jett_common::SourceOrigin::Project)]),
    )
    .expect("checked HIR");
    (
        lower(&hir, &checked.interner).expect("checked MIR"),
        checked.interner,
    )
}

fn exercise(program: &Program) -> &Function {
    program
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "exercise")
        .unwrap()
}

fn exercise_mut(program: &mut Program) -> &mut Function {
    program
        .functions
        .iter_mut()
        .find(|function| function.identity.declaration.name == "exercise")
        .unwrap()
}

fn stages(function: &Function) -> Vec<LocalId> {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.statements)
        .filter_map(|statement| match statement.kind {
            StatementKind::BeginCallView { local, .. } => Some(local),
            _ => None,
        })
        .collect()
}

const POST_CALL: &str = r#"function exercise(packet: Packet, incoming: optional[int64]) returns Packet:
    string answer = report(view packet.member, incoming handle:
        default 3
    )
    return packet
"#;

#[test]
fn call_views_end_after_actual_direct_and_indirect_calls_before_owner_moves() {
    for indirect in [false, true] {
        let body = if indirect {
            POST_CALL.replace("    string answer = report", "    function(view Named, int64) returns string callback = report\n    string answer = callback")
        } else {
            POST_CALL.to_owned()
        };
        let (program, types) = source_program(&body);
        let function = exercise(&program);
        let scope = stages(function);
        assert_eq!(scope.len(), 1);
        let stage = scope[0];
        let owner = function.params[0].local;
        assert_eq!(function.local(stage).unwrap().view_source, Some(owner));
        let plan = crate::move_values::MoveValuePlan::analyze(&program, function, &types).unwrap();
        assert!(!plan.owned_locals.contains(&(stage.index() as usize)));
        assert!(plan.owned_locals.contains(&(owner.index() as usize)));
        let entry = &function.blocks[function.entry.index() as usize];
        let begin = entry
            .statements
            .iter()
            .position(|statement| matches!(statement.kind, StatementKind::BeginCallView { .. }))
            .unwrap();
        let sum = entry
            .statements
            .iter()
            .position(|statement| matches!(statement.kind, StatementKind::SumTag { .. }))
            .unwrap();
        assert!(begin < sum);
        assert!(
            plan.live_after_statement[function.entry.index() as usize][begin]
                .contains(&(owner.index() as usize))
        );
        let end_block = function.blocks.iter().find(|block| block.statements.iter().any(|statement| matches!(statement.kind, StatementKind::EndCallView { local } if local == stage))).unwrap();
        let end = end_block
            .statements
            .iter()
            .position(|statement| matches!(statement.kind, StatementKind::EndCallView { .. }))
            .unwrap();
        assert!(
            matches!(&end_block.statements[end - 1].kind, StatementKind::Let { value, .. }
            if matches!(value.kind, ExpressionKind::Call { .. } | ExpressionKind::IndirectCall { .. }))
        );
        let mut renamed = program.clone();
        exercise_mut(&mut renamed).locals[stage.index() as usize].name = "ordinary_name".into();
        validate(exercise(&renamed), &types).unwrap();
    }
}

#[test]
fn aborted_call_ends_internal_view_before_the_owned_return_operand() {
    let body = POST_CALL.replace("        default 3", "        return packet");
    let (program, types) = source_program(&body);
    let function = exercise(&program);
    let stage = stages(function)[0];
    let owner = function.params[0].local;
    crate::move_values::MoveValuePlan::analyze(&program, function, &types).unwrap();
    let returned = function.blocks.iter().filter(|block| matches!(block.terminator.kind,
        TerminatorKind::Return(Some(Expression { kind: ExpressionKind::Local(local), .. })) if local == owner)).collect::<Vec<_>>();
    assert_eq!(returned.len(), 2);
    for block in returned {
        assert!(block.statements.iter().any(|statement| matches!(statement.kind, StatementKind::EndCallView { local } if local == stage)));
    }
}

#[test]
fn call_view_keeps_pending_owned_and_forwarded_parameter_origins_exact() {
    for borrowed in [false, true] {
        let body = if borrowed {
            "struct Envelope:\n    packet: Packet\nfunction exercise(view source: Envelope, incoming: optional[int64]) returns string:\n    Packet selected = view source.packet\n    Packet forwarded = selected\n    return report(view forwarded.member, incoming handle:\n        default 3\n    )\n"
        } else {
            "function exercise(incoming: optional[int64]) returns Packet:\n    Packet packet = run run Packet(member: 7)\n    string answer = report(view packet.member, incoming handle:\n        default 3\n    )\n    return packet\n"
        };
        let (program, types) = source_program(body);
        let function = exercise(&program);
        let stage = stages(function)[0];
        let source = function
            .locals
            .iter()
            .find(|local| local.name == if borrowed { "forwarded" } else { "packet" })
            .unwrap()
            .id;
        assert_eq!(function.local(stage).unwrap().view_source, Some(source));
        let plan = crate::move_values::MoveValuePlan::analyze(&program, function, &types).unwrap();
        assert!(!plan.owned_locals.contains(&(stage.index() as usize)));
        assert_eq!(
            plan.owned_locals.contains(&(source.index() as usize)),
            !borrowed
        );
        let entry = &function.blocks[function.entry.index() as usize];
        let begin = entry
            .statements
            .iter()
            .position(|statement| matches!(statement.kind, StatementKind::BeginCallView { .. }))
            .unwrap();
        assert!(
            plan.live_after_statement[function.entry.index() as usize][begin]
                .contains(&(source.index() as usize))
        );
        assert!(
            matches!(&entry.statements[begin].kind, StatementKind::BeginCallView { value: Expression { kind: ExpressionKind::View(value), .. }, .. }
            if matches!(value.kind, ExpressionKind::Field { base: ref owner, .. } if matches!(owner.kind, ExpressionKind::Local(local) if local == source)))
        );
    }
}

#[test]
fn call_view_scope_respects_loop_exit_depth_and_preserves_source_alias_loans() {
    for body in [
        r#"function exercise(packet: Packet, incoming: optional[int64]) returns Packet:
    mutable int64 count = 0
    while count < 2:
        count = count + 1
        string answer = report(view packet.member, incoming handle:
            if count == 1:
                continue
            default 3
        )
    return packet
"#,
        r#"function exercise(packet: Packet, incoming: optional[int64]) returns Packet:
    string answer = report(view packet.member, incoming handle:
        mutable int64 count = 0
        while count < 1:
            count = count + 1
            continue
        default 3
    )
    return packet
"#,
    ] {
        let (program, types) = source_program(body);
        assert_eq!(stages(exercise(&program)).len(), 1);
        crate::move_values::MoveValuePlan::analyze(&program, exercise(&program), &types).unwrap();
    }
    let body = POST_CALL.replace(
        "    string answer = report(view packet.member,",
        "    Named borrowed = view packet.member\n    string answer = report(view borrowed,",
    );
    let (program, types) = source_program(&body);
    let function = exercise(&program);
    let active_stages = stages(function);
    assert_eq!(active_stages.len(), 1);
    let borrowed = function
        .locals
        .iter()
        .find(|local| local.name == "borrowed")
        .unwrap();
    assert_eq!(
        function.local(active_stages[0]).unwrap().view_source,
        Some(borrowed.id)
    );
    assert!(
        crate::move_values::MoveValuePlan::analyze(&program, exercise(&program), &types)
            .unwrap_err()
            .contains("after creating a local view alias")
    );
}

#[test]
fn call_view_preflight_rejects_malformed_unused_metadata_and_scope_paths() {
    let (original, types) = source_program(POST_CALL);
    let stage = stages(exercise(&original))[0];
    for fault in [
        "field",
        "endpoint",
        "mutable",
        "origin",
        "clone",
        "missing_begin",
        "missing_end",
        "early_end",
        "duplicate_end",
        "source_let",
    ] {
        let mut program = original.clone();
        let function = exercise_mut(&mut program);
        match fault {
            "mutable" => function.locals[stage.index() as usize].mutable = true,
            "origin" => function.locals[stage.index() as usize].view_source = Some(stage),
            "missing_begin" => {
                for block in &mut function.blocks {
                    block.statements.retain(|statement| {
                        !matches!(statement.kind, StatementKind::BeginCallView { .. })
                    });
                }
            }
            "missing_end" => {
                for block in &mut function.blocks {
                    block.statements.retain(|statement| {
                        !matches!(statement.kind, StatementKind::EndCallView { .. })
                    });
                }
            }
            "early_end" | "duplicate_end" => {
                if fault == "early_end" {
                    for block in &mut function.blocks {
                        block.statements.retain(|statement| {
                            !matches!(statement.kind, StatementKind::EndCallView { .. })
                        });
                    }
                }
                let entry = &mut function.blocks[function.entry.index() as usize];
                let begin = entry
                    .statements
                    .iter()
                    .position(|statement| {
                        matches!(statement.kind, StatementKind::BeginCallView { .. })
                    })
                    .unwrap();
                entry.statements.insert(
                    begin + 1,
                    Statement {
                        kind: StatementKind::EndCallView { local: stage },
                        span: function.span,
                    },
                );
            }
            _ => {
                let statement = function
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.statements)
                    .find(|statement| matches!(statement.kind, StatementKind::BeginCallView { .. }))
                    .unwrap();
                let StatementKind::BeginCallView { local, value } = &mut statement.kind else {
                    unreachable!()
                };
                if fault == "source_let" {
                    statement.kind = StatementKind::Let {
                        local: *local,
                        value: value.clone(),
                    };
                } else if fault == "clone" {
                    value.kind = ExpressionKind::Clone(Box::new(value.clone()));
                } else {
                    let ExpressionKind::View(inner) = &mut value.kind else {
                        unreachable!()
                    };
                    if fault == "endpoint" {
                        inner.ty = TypeInterner::INT64;
                    } else {
                        let ExpressionKind::Field { field, .. } = &mut inner.kind else {
                            unreachable!()
                        };
                        *field = FieldId::new(99);
                        // Invalid unused field metadata must fail before pruning.
                        function.blocks[function.entry.index() as usize]
                            .terminator
                            .kind = TerminatorKind::Unreachable;
                    }
                }
            }
        }
        assert!(
            crate::move_values::validate_local_view_initializers(function, &types).is_err(),
            "{fault}"
        );
    }
}

#[test]
fn call_views_never_authorize_an_ordinary_alias_end_or_erased_owner_overlap() {
    let source = POST_CALL.replace(
        "    string answer = report(view packet.member,",
        "    Named borrowed = view packet.member\n    string answer = report(view borrowed,",
    );
    let (mut program, types) = source_program(&source);
    let function = exercise_mut(&mut program);
    let declared = function
        .locals
        .iter()
        .find(|local| local.name == "borrowed")
        .unwrap()
        .id;
    function.locals[declared.index() as usize].name = "$callView999".into();
    function.blocks[function.entry.index() as usize]
        .statements
        .push(Statement {
            kind: StatementKind::EndCallView { local: declared },
            span: function.span,
        });
    assert!(
        validate(function, &types)
            .unwrap_err()
            .contains("does not identify an internal stage")
    );

    let body = r#"function consume(packet: Packet) returns int64:
    return 3
function exercise(packet: Packet, incoming: optional[int64]) returns string:
    return report(view packet.member, incoming handle:
        default consume(packet)
    )
"#;
    let (program, types) = source_program(body);
    assert_eq!(stages(exercise(&program)).len(), 1);
    assert!(
        crate::move_values::MoveValuePlan::analyze(&program, exercise(&program), &types)
            .unwrap_err()
            .contains("while borrowed")
    );
}

#[test]
fn scoped_view_origin_and_end_survive_native_local_compaction() {
    let body = r#"function exercise(packet: Packet, incoming: optional[int64]) returns Packet:
    function(int64) returns int64 unrelated = function(value: int64) returns int64: return value
    Packet current = packet
    string answer = report(view current.member, incoming handle:
        default 3
    )
    return current
"#;
    let (mut program, types) = source_program(&body);
    let original = exercise(&program);
    let stage_before = stages(original)[0];
    let owner_before = original.local(stage_before).unwrap().view_source.unwrap();
    let count_before = original.locals.len();
    crate::prepare_native_generated_functions(&mut program, &types);
    let function = exercise(&program);
    validate(function, &types).unwrap();
    crate::move_values::MoveValuePlan::analyze(&program, function, &types).unwrap();
    assert_eq!(stages(function).len(), 1);
    let stage = stages(function)[0];
    let owner = function.local(stage).unwrap().view_source.unwrap();
    assert!(function.locals.len() < count_before);
    assert!(stage.index() < stage_before.index());
    assert!(owner.index() < owner_before.index());
    assert_eq!(function.local(owner).unwrap().name, "current");
    assert!(
        function
            .blocks
            .iter()
            .flat_map(|block| &block.statements)
            .any(|statement| {
                matches!(statement.kind, StatementKind::EndCallView { local } if local == stage)
            })
    );
}

#[test]
fn scoped_call_view_borrows_a_mutable_terminal_owner_until_post_call_rebinding() {
    let body = r#"function exercise(packet: Packet, incoming: optional[int64]) returns Packet:
    mutable Packet current = packet
    string answer = report(view current.member, incoming handle:
        default 3
    )
    current = Packet(member: 9)
    return current
"#;
    let (program, types) = source_program(body);
    let function = exercise(&program);
    let stage = stages(function)[0];
    let owner = function
        .locals
        .iter()
        .find(|local| local.name == "current")
        .unwrap();
    assert!(owner.mutable && owner.view_source.is_none());
    assert_eq!(function.local(stage).unwrap().view_source, Some(owner.id));
    crate::validate(&program).unwrap();
    validate(function, &types).unwrap();
    let plan = crate::move_values::MoveValuePlan::analyze(&program, function, &types).unwrap();
    assert!(plan.owned_locals.contains(&(owner.id.index() as usize)));
    assert!(!plan.owned_locals.contains(&(stage.index() as usize)));
    let block = function
        .blocks
        .iter()
        .find(|block| {
            block.statements.iter().any(|statement| {
                matches!(&statement.kind, StatementKind::Assign { target, .. }
            if matches!(target.kind, ExpressionKind::Local(local) if local == owner.id))
            })
        })
        .unwrap();
    let end = block.statements.iter().position(|statement| {
        matches!(statement.kind, StatementKind::EndCallView { local } if local == stage)
    }).unwrap();
    let assignment = block
        .statements
        .iter()
        .position(|statement| {
            matches!(&statement.kind, StatementKind::Assign { target, .. }
            if matches!(target.kind, ExpressionKind::Local(local) if local == owner.id))
        })
        .unwrap();
    assert!(end < assignment);
}

#[test]
fn scoped_call_view_materializes_the_original_owned_temporary_once_before_the_handler() {
    for initializer in ["Packet(member: 7)", "run run Packet(member: 7)"] {
        let body = format!(
            r#"function make() returns Packet:
    return {initializer}
function exercise(incoming: optional[int64]) returns string:
    return report(view make().member, incoming handle:
        default 3
    )
"#
        );
        let (program, types) = source_program(&body);
        let function = exercise(&program);
        let stage = stages(function)[0];
        let owner = function.local(stage).unwrap().view_source.unwrap();
        assert!(function.local(owner).unwrap().view_source.is_none());
        let make = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "make")
            .unwrap()
            .id;
        let initializers = function.blocks.iter().flat_map(|block| &block.statements).filter(|statement| {
            matches!(&statement.kind, StatementKind::Let { local, value }
                if *local == owner && matches!(value.kind, ExpressionKind::Call { function, .. } if function == make))
        }).count();
        assert_eq!(initializers, 1);
        let entry = &function.blocks[function.entry.index() as usize];
        let owned = entry.statements.iter().position(|statement| {
            matches!(statement.kind, StatementKind::Let { local, .. } if local == owner)
        }).unwrap();
        let begin = entry.statements.iter().position(|statement| {
            matches!(statement.kind, StatementKind::BeginCallView { local, .. } if local == stage)
        }).unwrap();
        let sum = entry
            .statements
            .iter()
            .position(|statement| matches!(statement.kind, StatementKind::SumTag { .. }))
            .unwrap();
        assert!(owned < begin && begin < sum);
        let StatementKind::BeginCallView { value, .. } = &entry.statements[begin].kind else {
            unreachable!()
        };
        assert!(
            matches!(borrowed_root(value).unwrap().kind, ExpressionKind::Local(local) if local == owner)
        );
        let plan = crate::move_values::MoveValuePlan::analyze(&program, function, &types).unwrap();
        assert_eq!(
            plan.owned_locals
                .iter()
                .filter(|local| **local == owner.index() as usize)
                .count(),
            1
        );
        assert!(!plan.owned_locals.contains(&(stage.index() as usize)));
        assert!(
            plan.live_after_statement[function.entry.index() as usize][begin]
                .contains(&(owner.index() as usize))
        );

        let mut missing = program.clone();
        let invalid = exercise_mut(&mut missing);
        for block in &mut invalid.blocks {
            block.statements.retain(|statement| !matches!(statement.kind, StatementKind::Let { local, .. } if local == owner));
        }
        assert!(
            crate::move_values::MoveValuePlan::analyze(&missing, exercise(&missing), &types)
                .is_err()
        );
    }
}

#[test]
fn scoped_mutable_root_keeps_active_overlap_and_persistent_alias_safety() {
    let body = r#"function exercise(packet: Packet, incoming: optional[int64]) returns string:
    mutable Packet current = packet
    return report(view current.member, incoming handle:
        current = Packet(member: 9)
        default 3
    )
"#;
    let (program, types) = source_program(body);
    assert_eq!(stages(exercise(&program)).len(), 1);
    assert!(
        crate::move_values::MoveValuePlan::analyze(&program, exercise(&program), &types)
            .unwrap_err()
            .contains("while borrowed")
    );

    let forwarded = "struct Envelope:\n    packet: Packet\nfunction exercise(view source: Envelope, incoming: optional[int64]) returns string:\n    Packet selected = view source.packet\n    return report(view selected.member, incoming handle:\n        default 3\n    )\n";
    let (mut program, types) = source_program(forwarded);
    let function = exercise_mut(&mut program);
    let selected = function
        .locals
        .iter()
        .find(|local| local.name == "selected")
        .unwrap()
        .id;
    function.locals[selected.index() as usize].mutable = true;
    assert!(validate(function, &types).is_err());

    let (mut program, types) = source_program(POST_CALL);
    let function = exercise_mut(&mut program);
    let stage = stages(function)[0];
    let mut escaped = function.local(stage).unwrap().clone();
    escaped.id = LocalId::new(function.locals.len() as u32);
    escaped.name = "unused_alias".into();
    escaped.view_source = Some(stage);
    function.locals.push(escaped);
    assert!(validate(function, &types).unwrap_err().contains("escape"));
}

#[test]
fn scoped_direct_named_view_keeps_mutable_storage_borrowed_until_post_call_rebinding() {
    let body = r#"function exercise(incoming: optional[int64]) returns Named:
    mutable Named current = 7
    string answer = report(view current, incoming handle:
        default 3
    )
    current = 9
    return current
"#;
    let (program, types) = source_program(body);
    let function = exercise(&program);
    let stage = stages(function)[0];
    let owner = function
        .locals
        .iter()
        .find(|local| local.name == "current")
        .unwrap();
    assert!(owner.mutable && owner.view_source.is_none());
    assert_eq!(function.local(stage).unwrap().view_source, Some(owner.id));
    let value = function
        .blocks
        .iter()
        .flat_map(|block| &block.statements)
        .find_map(|statement| match &statement.kind {
            StatementKind::BeginCallView { value, .. } => Some(value),
            _ => None,
        })
        .unwrap();
    assert!(matches!(&value.kind, ExpressionKind::View(inner)
        if matches!(inner.kind, ExpressionKind::Local(local) if local == owner.id)));
    assert_eq!(value.ty, owner.ty);
    crate::validate(&program).unwrap();
    validate(function, &types).unwrap();
    let plan = crate::move_values::MoveValuePlan::analyze(&program, function, &types).unwrap();
    assert!(plan.owned_locals.contains(&(owner.id.index() as usize)));
    assert!(!plan.owned_locals.contains(&(stage.index() as usize)));
}

#[test]
fn scoped_direct_owned_named_return_is_observed_once_without_a_field_or_readiness_guard() {
    for pending in [false, true] {
        let source = if pending {
            "    Named original = run run make_named()\n    return report(view original,"
        } else {
            "    return report(view make_named(),"
        };
        let body = format!(
            r#"function make_named() returns Named:
    return 7
function exercise(incoming: optional[int64]) returns string:
{source} incoming handle:
        default 3
    )
"#
        );
        let (program, types) = source_program(&body);
        let function = exercise(&program);
        let stage = stages(function)[0];
        let owner = function.local(stage).unwrap().view_source.unwrap();
        let entry = &function.blocks[function.entry.index() as usize];
        let begin = entry.statements.iter().position(|statement| {
            matches!(statement.kind, StatementKind::BeginCallView { local, .. } if local == stage)
        }).unwrap();
        let StatementKind::BeginCallView { value, .. } = &entry.statements[begin].kind else {
            unreachable!()
        };
        assert!(matches!(&value.kind, ExpressionKind::View(inner)
            if matches!(inner.kind, ExpressionKind::Local(local) if local == owner)));
        assert_eq!(value.ty, function.local(owner).unwrap().ty);
        assert!(!entry.statements[..=begin].iter().any(|statement| {
            matches!(
                statement.kind,
                StatementKind::ReflectedContainerReady { .. }
            )
        }));
        let plan = crate::move_values::MoveValuePlan::analyze(&program, function, &types).unwrap();
        assert!(plan.owned_locals.contains(&(owner.index() as usize)));
        assert!(!plan.owned_locals.contains(&(stage.index() as usize)));
        assert!(
            plan.live_after_statement[function.entry.index() as usize][begin]
                .contains(&(owner.index() as usize))
        );
        if !pending {
            let make = program
                .functions
                .iter()
                .find(|function| function.identity.declaration.name == "make_named")
                .unwrap()
                .id;
            assert_eq!(function.blocks.iter().flat_map(|block| &block.statements).filter(|statement| {
                matches!(&statement.kind, StatementKind::Let { local, value }
                    if *local == owner && matches!(value.kind, ExpressionKind::Call { function, .. } if function == make))
            }).count(), 1);
        }
    }
}
