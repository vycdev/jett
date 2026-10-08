use super::*;
use crate::copy_values::CopyValuePlan;
use crate::move_values::MoveValuePlan;
use jett_common::{FileId, SourceOrigin};
use jett_hir::{ExpressionKind as E, HandleKind};
use jett_types::{Type, TypeInterner};
use std::collections::HashMap;

const SOURCES: [&str; 12] = [
    include_str!(
        "../../../jett_driver/tests/native_conformance/ordinary_borrowed_sums/01_optional_branches.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/ordinary_borrowed_sums/02_result_branches.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/ordinary_borrowed_sums/03_owned_scoped_forwarded.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/ordinary_borrowed_sums/04_named_prefix_return.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/ordinary_borrowed_sums/05_optional_terminal_prefix.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/ordinary_borrowed_sums/06_result_terminal_prefix.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/ordinary_borrowed_sums/07_outer_pending_some.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/ordinary_borrowed_sums/08_outer_pending_none.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/ordinary_borrowed_sums/09_outer_pending_ok.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/ordinary_borrowed_sums/10_outer_pending_fail.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/ordinary_borrowed_sums/11_pending_optional_payload.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/ordinary_borrowed_sums/12_pending_result_payload.jett"
    ),
];

fn lowered(source: &str, release: bool) -> (crate::Program, TypeInterner) {
    let project = FileId::new(0);
    let list = FileId::new(jett_common::STDLIB_FILE_ID_START);
    let bytes = FileId::new(jett_common::STDLIB_FILE_ID_START + 1);
    let mut parsed = jett_parser::parse(include_str!("../../../../stdlib/list.jett"), list);
    let mut byte_module = jett_parser::parse(include_str!("../../../../stdlib/bytes.jett"), bytes);
    let mut primary = jett_parser::parse(source, project);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    assert!(byte_module.errors.is_empty(), "{:?}", byte_module.errors);
    assert!(primary.errors.is_empty(), "{:?}", primary.errors);
    parsed.module.items.append(&mut byte_module.module.items);
    parsed.module.items.append(&mut primary.module.items);
    let resolved = jett_resolve::resolve(&parsed.module);
    assert!(
        resolved
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != jett_diagnostics::Severity::Error),
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
            .all(|diagnostic| diagnostic.severity != jett_diagnostics::Severity::Error),
        "{:?}",
        checked.diagnostics
    );
    let high = jett_hir::lower(
        &parsed.module,
        &resolved,
        &checked,
        &HashMap::from([
            (project, SourceOrigin::Project),
            (list, SourceOrigin::Stdlib),
            (bytes, SourceOrigin::Stdlib),
        ]),
    )
    .expect("genuine frozen Source and shipped stdlib HIR");
    (
        crate::lower(&high, &checked.interner).expect("genuine borrowed Handle MIR"),
        checked.interner,
    )
}

fn guarded_function(program: &crate::Program) -> &Function {
    program
        .functions
        .iter()
        .find(|function| {
            function.identity.declaration.namespace == "app"
                && function.ordinary_borrowed_sums.is_some()
        })
        .expect("original ordinary borrowed declaration")
}

fn guarded_function_mut(program: &mut crate::Program) -> &mut Function {
    program
        .functions
        .iter_mut()
        .find(|function| {
            function.identity.declaration.namespace == "app"
                && function.ordinary_borrowed_sums.is_some()
        })
        .expect("original ordinary borrowed declaration")
}

fn assert_plans(program: &crate::Program, types: &TypeInterner, fixture: usize) -> usize {
    crate::validate_call_ownership(program, types).unwrap();
    let mut list_rows = 0;
    for function in &program.functions {
        let Some(witness) = &function.ordinary_borrowed_sums else {
            continue;
        };
        let copy = CopyValuePlan::analyze_storage(function, types, Some(program))
            .unwrap_or_else(|error| panic!("Source{fixture:02} Copy: {error}"));
        let plan = MoveValuePlan::analyze(program, function, types)
            .unwrap_or_else(|error| panic!("Source{fixture:02} Move: {error}"));
        assert_eq!(plan.owned_locals, copy.owned_locals);
        for row in &witness.rows {
            list_rows += usize::from(matches!(types.resolve(row.payload_type()), Type::List(_)));
            assert_eq!(function.local(row.backing()).unwrap().ty, row.sum_type());
            for local in [row.source(), row.output()] {
                let header = function.local(local).unwrap();
                assert!(!header.mutable);
                assert_eq!(header.view_source, Some(row.backing()));
                assert!(function.is_view_local(local));
                assert!(!plan.owned_locals.contains(&(local.index() as usize)));
                assert!(!copy.owned_locals.contains(&(local.index() as usize)));
            }
            let alias = function.local(row.alias()).unwrap();
            assert_eq!(alias.mutable, row.original_alias.mutable);
            if alias.view_source.is_some() {
                assert!(
                    !alias.mutable,
                    "a true borrowed final alias remains immutable"
                );
            }
            assert_eq!(
                alias.view_source,
                row.original_alias.view_source.map(|_| row.backing())
            );
            let owned_alias = alias.view_source.is_none() && alias.ty == TypeInterner::STRING;
            assert_eq!(
                plan.owned_locals.contains(&(alias.id.index() as usize)),
                owned_alias
            );
            assert_eq!(
                copy.owned_locals.contains(&(alias.id.index() as usize)),
                owned_alias
            );
            assert_eq!(function.local(row.source()).unwrap().ty, row.sum_type());
            assert_eq!(function.local(row.output()).unwrap().ty, row.payload_type());
            assert_eq!(function.local(row.alias()).unwrap().ty, row.payload_type());
            let initialization = row.initialize_site();
            assert!(
                matches!(&function.blocks[initialization.block().index() as usize].statements[initialization.statement_index()].kind, StatementKind::Let { local, value } if *local == row.source() && matches!(&value.kind, E::View(inner) if matches!(inner.kind, E::Local(backing) if backing == row.backing())))
            );
            let observed = row.observe_site();
            assert!(
                matches!(function.blocks[observed.block().index() as usize].statements[observed.statement_index()].kind, StatementKind::SumTag { source, target } if source == row.source() && target == row.tag())
            );
            let projected = row.success_site();
            assert!(
                matches!(function.blocks[projected.block().index() as usize].statements[projected.statement_index()].kind, StatementKind::SumTake { source, target, success: true } if source == row.source() && target == row.output())
            );
            assert_eq!(
                function
                    .ordinary_borrowed_sum_projection(
                        projected.block(),
                        projected.statement_index()
                    )
                    .unwrap()
                    .unwrap()
                    .alias(),
                row.alias()
            );
            let alias_value = function
                .blocks
                .iter()
                .flat_map(|block| &block.statements)
                .find_map(|statement| match &statement.kind {
                    StatementKind::Let { local, value } if *local == row.alias() => Some(value),
                    _ => None,
                })
                .unwrap();
            assert!(
                function
                    .ordinary_borrowed_sum_alias(row.alias(), alias_value)
                    .unwrap()
            );
            let E::Handle { kind, .. } = &row.original_handle().kind else {
                panic!("complete original Handle")
            };
            match (
                types.resolve(row.sum_type()),
                kind,
                row.error(),
                row.failure_site(),
            ) {
                (Type::Optional(payload), HandleKind::Optional, None, None) => {
                    assert_eq!(*payload, row.payload_type())
                }
                (Type::Result(payload, failure), HandleKind::Result, Some(error), Some(site)) => {
                    assert_eq!(*payload, row.payload_type());
                    assert_eq!(*failure, TypeInterner::STRING);
                    let header = function.local(error).unwrap();
                    assert_eq!(header.ty, TypeInterner::STRING);
                    assert!(header.view_source.is_none());
                    assert!(plan.owned_locals.contains(&(error.index() as usize)));
                    assert!(copy.owned_locals.contains(&(error.index() as usize)));
                    assert!(copy.temporary_slots > 0);
                    assert!(
                        matches!(function.blocks[site.block().index() as usize].statements[site.statement_index()].kind, StatementKind::SumTake { source, target, success: false } if source == row.source() && target == error)
                    );
                    assert_eq!(
                        function
                            .ordinary_borrowed_sum_projection(site.block(), site.statement_index())
                            .unwrap()
                            .unwrap()
                            .alias(),
                        row.alias()
                    );
                }
                _ => panic!("exact sum, payload and owned failure companion"),
            }
            for (planner, live_in, live_out, live_after) in [
                (
                    "Move",
                    &plan.live_in,
                    &plan.live_out,
                    &plan.live_after_statement,
                ),
                (
                    "Copy",
                    &copy.live_in,
                    &copy.live_out,
                    &copy.live_after_statement,
                ),
            ] {
                let mut observed_live = false;
                for live in live_in
                    .iter()
                    .chain(live_out)
                    .chain(live_after.iter().flatten())
                {
                    if (alias.view_source.is_some()
                        && live.contains(&(row.alias().index() as usize)))
                        || live.contains(&(row.output().index() as usize))
                        || live.contains(&(row.source().index() as usize))
                    {
                        observed_live = true;
                        assert!(
                            live.contains(&(row.backing().index() as usize)),
                            "Source{fixture:02} {planner} backing must outlive its payload view"
                        );
                    }
                }
                assert!(
                    observed_live,
                    "Source{fixture:02} {planner} visits a live projection"
                );
            }
            assert_eq!(
                plan.owned_locals
                    .contains(&(row.backing().index() as usize)),
                !function.is_view_local(row.backing())
            );
        }
    }
    list_rows
}

#[test]
fn ordinary_borrowed_sums_all_frozen_sources_keep_nonowning_payload_and_owned_failure_plans() {
    let expected = [2, 2, 9, 2, 1, 1, 1, 1, 1, 1, 1, 1];
    for release in [false, true] {
        for (index, source) in SOURCES.into_iter().enumerate() {
            let (mut program, types) = lowered(source, release);
            if index != 2 {
                assert_eq!(assert_plans(&program, &types, index + 1), expected[index]);
            } else {
                crate::validate_call_ownership(&program, &types).unwrap();
            }
            crate::prepare_native_sequences(&mut program, &types);
            for function in &mut program.functions {
                assert!(
                    crate::sequences::prune::unreachable(function),
                    "Source{:02} release={release}",
                    index + 1
                );
                assert!(
                    crate::sequences::prune::unused_locals(function),
                    "Source{:02} local remap release={release}",
                    index + 1
                );
            }
            assert_eq!(assert_plans(&program, &types, index + 1), expected[index]);
        }
    }
}

#[test]
fn ordinary_borrowed_sums_owned_scoped_forwarded_source_keeps_original_owner_across_real_loop_preparation()
 {
    for release in [false, true] {
        let (mut program, types) = lowered(SOURCES[2], release);
        let has_for = |program: &crate::Program| {
            program
                .functions
                .iter()
                .filter(|function| function.identity.declaration.namespace == "app")
                .flat_map(|function| &function.blocks)
                .any(|block| matches!(block.terminator.kind, TerminatorKind::ForEach { .. }))
        };
        assert!(has_for(&program));
        crate::validate_call_ownership(&program, &types).unwrap();
        crate::prepare_native_sequences(&mut program, &types);
        assert!(
            !has_for(&program),
            "Source03 must complete its actual For lowering"
        );
        for function in &program.functions {
            if function.identity.declaration.namespace != "app"
                || !matches!(
                    function.identity.declaration.name.as_str(),
                    "inspect_optional" | "inspect_result"
                )
            {
                continue;
            }
            let named = |name: &str| {
                function
                    .locals
                    .iter()
                    .find(|local| local.name == name)
                    .unwrap()
                    .id
            };
            let source = named("source");
            let alias = named("alias");
            let forwarded = named("forwarded");
            let copied = named("copied");
            assert_eq!(function.local(alias).unwrap().view_source, Some(source));
            assert_eq!(function.local(forwarded).unwrap().view_source, Some(alias));
            assert!(function.local(copied).unwrap().view_source.is_none());
            let plan = MoveValuePlan::analyze(&program, function, &types).unwrap();
            assert!(plan.owned_locals.contains(&(source.index() as usize)));
            assert!(plan.owned_locals.contains(&(copied.index() as usize)));
            assert!(!plan.owned_locals.contains(&(alias.index() as usize)));
            assert!(!plan.owned_locals.contains(&(forwarded.index() as usize)));
            let mut used = false;
            for live in plan.live_in.iter().chain(&plan.live_out).chain(
                plan.live_after_statement
                    .iter()
                    .flat_map(|block| block.iter()),
            ) {
                if live.contains(&(forwarded.index() as usize)) {
                    used = true;
                    assert!(live.contains(&(alias.index() as usize)));
                    assert!(live.contains(&(source.index() as usize)));
                }
            }
            assert!(used);
        }
        assert_eq!(assert_plans(&program, &types, 3), 9);
    }
}

#[test]
fn ordinary_borrowed_sums_scalar_count_preserves_checked_copyable_alias_mode() {
    for release in [false, true] {
        let (mut program, types) = lowered(SOURCES[2], release);
        crate::prepare_native_sequences(&mut program, &types);
        assert_eq!(assert_plans(&program, &types, 3), 9);
        let scalar_rows = program
            .functions
            .iter()
            .filter_map(|function| {
                function
                    .ordinary_borrowed_sums
                    .as_ref()
                    .map(|witness| (function, witness))
            })
            .flat_map(|(function, witness)| witness.rows.iter().map(move |row| (function, row)))
            .filter(|(_, row)| row.payload_type() == TypeInterner::INT64)
            .collect::<Vec<_>>();
        assert_eq!(
            scalar_rows.len(),
            1,
            "Source03 count[int64] has its own constructor record"
        );
        let (function, row) = scalar_rows[0];
        assert_eq!(function.identity.declaration.name, "count");
        assert!(row.original_alias.view_source.is_none());
        assert!(function.local(row.alias()).unwrap().view_source.is_none());
        assert_eq!(
            function.local(row.output()).unwrap().view_source,
            Some(row.backing())
        );
        assert_eq!(
            function.local(row.source()).unwrap().view_source,
            Some(row.backing())
        );
        let move_plan = MoveValuePlan::analyze(&program, function, &types).unwrap();
        assert!(
            !move_plan
                .owned_locals
                .contains(&(row.alias().index() as usize))
        );
        assert!(
            !move_plan
                .owned_locals
                .contains(&(row.output().index() as usize))
        );
    }
}

const STRING_COPY: &str = r#"namespace app
function inspect(view source: optional[string]) returns string:
    string alias = view((view source) handle:
        return "absent"
    )
    return alias
function main() returns string:
    optional[string] source = some("present")
    return inspect(view source)
"#;

#[test]
fn ordinary_borrowed_sums_string_success_copies_into_owned_final_alias() {
    for release in [false, true] {
        let (mut program, types) = lowered(STRING_COPY, release);
        assert_eq!(assert_plans(&program, &types, 0), 0);
        let function = guarded_function(&program);
        let row = &function.ordinary_borrowed_sums.as_ref().unwrap().rows[0];
        assert_eq!(row.payload_type(), TypeInterner::STRING);
        assert!(row.original_alias.view_source.is_none());
        assert!(function.local(row.alias()).unwrap().view_source.is_none());
        assert!(!function.is_view_local(row.alias()));
        assert!(function.is_view_local(row.output()));
        let copy = CopyValuePlan::analyze_storage(function, &types, Some(&program)).unwrap();
        let moved = MoveValuePlan::analyze(&program, function, &types).unwrap();
        assert!(copy.owned_locals.contains(&(row.alias().index() as usize)));
        assert!(moved.owned_locals.contains(&(row.alias().index() as usize)));
        assert!(!copy.owned_locals.contains(&(row.output().index() as usize)));
        assert!(
            !moved
                .owned_locals
                .contains(&(row.output().index() as usize))
        );
        assert!(
            copy.temporary_slots > 0,
            "final String View takes its ordinary owned copy"
        );
        crate::prepare_native_sequences(&mut program, &types);
        for function in &mut program.functions {
            assert!(crate::sequences::prune::unreachable(function));
        }
        assert_eq!(assert_plans(&program, &types, 0), 0);
    }
}

const CALLBACK_COMPACTION: &str = r#"namespace app
function inspect(view source: optional[list[int64]]) returns int64:
    list[int64] alias = view((view source) handle:
        return 0
        int64 dead = 7
        return 0
    )
    int64 retained = 3
    function(int64) returns int64 callback = function(value: int64) returns int64: return retained + value
    return list.sum[int64](view alias) + callback(2)
function main() returns int64:
    optional[list[int64]] source = some(list(4, 5))
    return inspect(view source)
"#;

const COPY_THEN_MOVE_OPTIONAL: &str = r#"namespace app
function consume(source: optional[int64]) returns int64:
    return source handle: return 0
function main() returns int64:
    optional[int64] source = some(5)
    int64 alias = view((view source) handle: return 0)
    int64 owned = consume(source)
    return alias + owned
"#;

const COPY_THEN_MOVE_RESULT: &str = r#"namespace app
function consume(source: result[string, string]) returns string:
    return source handle error: return error
function main() returns string:
    result[string, string] source = ok("present")
    string alias = view((view source) handle error: return error)
    string owned = consume(source)
    return "{alias}:{owned}"
"#;

// Exact genuine Source15 from Root's mutable-copy reference control.
const MUTABLE_COPIED_PAYLOADS: &str = r#"namespace app
function copied_int(view stdout: Stdout) returns nothing:
    optional[int64] source = some(3)
    mutable int64 copied = view((view source) handle:
        return nothing
    )
    copied = copied + 1
    int64 original = source handle:
        return nothing
    Stdout.write(view stdout, "integer:{copied}:{original}\n")
    return nothing
function copied_string(view stdout: Stdout) returns nothing:
    result[string, string] source = ok("kept")
    mutable string copied = view((view source) handle error:
        return nothing
    )
    copied = "changed"
    string original = source handle error:
        return nothing
    Stdout.write(view stdout, "string:{copied}:{original}\n")
    return nothing
function main(stdout: Stdout) returns nothing:
    copied_int(view stdout)
    copied_string(view stdout)
    return nothing
"#;

#[test]
fn ordinary_borrowed_sums_mutable_copied_payloads_rebind_before_original_owning_unwrap() {
    for release in [false, true] {
        let (mut program, types) = lowered(MUTABLE_COPIED_PAYLOADS, release);
        for prepared in [false, true] {
            if prepared {
                crate::prepare_native_sequences(&mut program, &types);
                for function in &mut program.functions {
                    assert!(crate::sequences::prune::unreachable(function));
                    assert!(crate::sequences::prune::unused_locals(function));
                }
            }
            assert_eq!(assert_plans(&program, &types, 15), 0);
            let mut copied_types = Vec::new();
            for function in &program.functions {
                let Some(witness) = &function.ordinary_borrowed_sums else {
                    continue;
                };
                assert_eq!(witness.rows.len(), 1);
                let row = &witness.rows[0];
                copied_types.push(row.payload_type());
                assert!(row.original_alias.mutable);
                assert!(row.original_alias.view_source.is_none());
                assert!(function.local(row.alias()).unwrap().mutable);
                assert!(function.local(row.alias()).unwrap().view_source.is_none());
                assert!(!function.local(row.backing()).unwrap().mutable);
                assert!(!function.is_view_local(row.backing()));
                for intermediate in [row.source(), row.output()] {
                    let header = function.local(intermediate).unwrap();
                    assert!(!header.mutable);
                    assert_eq!(header.view_source, Some(row.backing()));
                }
                let continuation = &function.blocks[row.alias_site.block.index() as usize];
                let (assigned_at, replacement) = continuation.statements.iter().enumerate().find_map(|(index, statement)| match &statement.kind {
                    StatementKind::Assign { target, value } if matches!(target.kind, E::Local(local) if local == row.alias()) => Some((index, value)),
                    _ => None,
                }).expect("the exact Source rebinds its final copied payload");
                assert!(assigned_at > row.alias_site.statement_index);
                if row.payload_type() == TypeInterner::INT64 {
                    assert_eq!(function.identity.declaration.name, "copied_int");
                    assert!(
                        matches!(&replacement.kind, E::Binary { left, right, .. } if matches!(left.kind, E::Local(local) if local == row.alias()) && matches!(right.kind, E::Int(1)))
                    );
                } else {
                    assert_eq!(row.payload_type(), TypeInterner::STRING);
                    assert_eq!(function.identity.declaration.name, "copied_string");
                    assert!(matches!(&replacement.kind, E::String(value) if value == "changed"));
                }
                let later_owning_take = function
                    .blocks
                    .iter()
                    .find_map(|block| {
                        block
                            .statements
                            .iter()
                            .find_map(|statement| match statement.kind {
                                StatementKind::SumTake {
                                    source,
                                    target,
                                    success: true,
                                } if source != row.source()
                                    && !function.is_view_local(source)
                                    && function.local(source).unwrap().ty == row.sum_type()
                                    && function.local(target).unwrap().ty == row.payload_type() =>
                                {
                                    Some((block.id, source))
                                }
                                _ => None,
                            })
                    })
                    .expect("later bare owning Handle unwraps its ordinary shell snapshot");
                assert_ne!(later_owning_take.0, row.project.block);
                let cfg = ControlFlowGraph::analyze(function).unwrap();
                assert!(cfg.reverse_postorder().contains(&later_owning_take.0));
                let copy =
                    CopyValuePlan::analyze_storage(function, &types, Some(&program)).unwrap();
                let moved = MoveValuePlan::analyze(&program, function, &types)
                    .expect("rebinding a copied payload does not create or preserve a source loan");
                for live_after in [&copy.live_after_statement, &moved.live_after_statement] {
                    let after_rebind =
                        &live_after[row.alias_site.block.index() as usize][assigned_at];
                    assert!(after_rebind.contains(&(row.alias().index() as usize)));
                    assert!(!after_rebind.contains(&(row.source().index() as usize)));
                    assert!(!after_rebind.contains(&(row.output().index() as usize)));
                }
            }
            copied_types.sort_by_key(|ty| ty.index());
            let mut expected = vec![TypeInterner::INT64, TypeInterner::STRING];
            expected.sort_by_key(|ty| ty.index());
            assert_eq!(copied_types, expected);
        }
    }
}

#[test]
fn ordinary_borrowed_sums_genuine_mutable_list_payload_still_refuses_native_borrow() {
    let source = r#"namespace app
function inspect(view source: optional[list[int64]]) returns nothing:
    mutable list[int64] alias = view((view source) handle: return nothing)
    trace alias
    return nothing
function main() returns nothing:
    optional[list[int64]] source = some(list(3))
    inspect(view source)
    return nothing
"#;
    for release in [false, true] {
        let file = FileId::new(0);
        let parsed = jett_parser::parse(source, file);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let resolved = jett_resolve::resolve(&parsed.module);
        assert!(
            resolved
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.severity != jett_diagnostics::Severity::Error),
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
                .all(|diagnostic| diagnostic.severity != jett_diagnostics::Severity::Error),
            "{:?}",
            checked.diagnostics
        );
        let errors = jett_hir::lower(
            &parsed.module,
            &resolved,
            &checked,
            &HashMap::from([(file, SourceOrigin::Project)]),
        )
        .expect_err("genuine move-only payload View cannot declare a mutable borrowed final alias");
        assert!(
            errors.iter().any(|error| error
                .message
                .contains("native borrowed alias requires immutable bindings")),
            "{errors:?}"
        );
    }
}

#[test]
fn ordinary_borrowed_sums_copied_final_alias_retires_projection_loans_before_original_shell_transfer()
 {
    for release in [false, true] {
        for source in [COPY_THEN_MOVE_OPTIONAL, COPY_THEN_MOVE_RESULT] {
            let (mut program, types) = lowered(source, release);
            for prepared in [false, true] {
                if prepared {
                    crate::prepare_native_sequences(&mut program, &types);
                    for function in &mut program.functions {
                        assert!(crate::sequences::prune::unreachable(function));
                        assert!(crate::sequences::prune::unused_locals(function));
                    }
                }
                assert_eq!(assert_plans(&program, &types, 0), 0);
                let function = guarded_function(&program);
                let witness = function.ordinary_borrowed_sums.as_ref().unwrap();
                assert_eq!(witness.rows.len(), 1);
                let row = &witness.rows[0];
                assert!(row.original_alias.view_source.is_none());
                assert!(function.local(row.alias()).unwrap().view_source.is_none());
                assert!(
                    !function.is_view_local(row.backing()),
                    "the original sum shell is owned"
                );
                assert_eq!(
                    function.local(row.source()).unwrap().view_source,
                    Some(row.backing())
                );
                assert_eq!(
                    function.local(row.output()).unwrap().view_source,
                    Some(row.backing())
                );
                let (call_block, call_index, call) = function.blocks.iter().find_map(|block| {
                    block.statements.iter().enumerate().find_map(|(index, statement)| match &statement.kind {
                        StatementKind::Let { value, .. } if matches!(value.kind, E::Call { function: target, .. } if program.functions[target.index() as usize].identity.declaration.name == "consume") => Some((block.id, index, value)),
                        _ => None,
                    })
                }).expect("actual owning helper invocation");
                assert_eq!(call_block, row.alias_site.block);
                assert!(
                    call_index > row.alias_site.statement_index,
                    "copy precedes the source transfer"
                );
                let acquisitions =
                    crate::validate_caller_acquisitions(&program, function, &types).unwrap();
                let [acquired] = acquisitions
                    .arguments(call)
                    .expect("current owning call acquisition")
                else {
                    panic!("one original owned shell argument")
                };
                assert_eq!(acquired.parameter_index, 0);
                assert_eq!(acquired.source.binding, Some(row.backing()));
                assert_eq!(
                    acquired.source.effect,
                    jett_typecheck::CheckedCallerEffect::TransferOwned
                );
                assert_eq!(acquired.source.actual_type, row.sum_type());
                assert_eq!(acquired.source.storage_type, row.sum_type());
                let E::Call {
                    function: target,
                    args,
                    ..
                } = &call.kind
                else {
                    panic!("owning helper call")
                };
                assert_eq!(
                    program.functions[target.index() as usize].params[0].mode,
                    ParamMode::Owned
                );
                assert_eq!(acquisitions.argument_binding(&args[0]), Some(row.backing()));
                let copy =
                    CopyValuePlan::analyze_storage(function, &types, Some(&program)).unwrap();
                let moved = MoveValuePlan::analyze(&program, function, &types).expect("a copied final alias leaves no active projection loan that can block moving its original shell");
                for live_after in [&copy.live_after_statement, &moved.live_after_statement] {
                    let live = &live_after[row.alias_site.block.index() as usize]
                        [row.alias_site.statement_index];
                    assert!(
                        live.contains(&(row.alias().index() as usize)),
                        "the final copy is used after source transfer"
                    );
                    assert!(
                        live.contains(&(row.backing().index() as usize)),
                        "the original backing is acquired by the later call"
                    );
                    assert!(!live.contains(&(row.source().index() as usize)));
                    assert!(!live.contains(&(row.output().index() as usize)));
                }
            }
        }
    }
}

#[test]
fn ordinary_borrowed_sums_genuine_callback_capture_survives_local_compaction() {
    for release in [false, true] {
        let (mut program, types) = lowered(CALLBACK_COMPACTION, release);
        assert_eq!(assert_plans(&program, &types, 0), 1);
        let original = guarded_function(&program).clone();
        let dead = original
            .locals
            .iter()
            .find(|local| local.name == "dead")
            .unwrap()
            .id;
        let retained = original
            .locals
            .iter()
            .find(|local| local.name == "retained")
            .unwrap()
            .id;
        let callback = original
            .locals
            .iter()
            .find(|local| local.name == "callback")
            .unwrap()
            .id;
        let inline_parameter = original
            .locals
            .iter()
            .find(|local| local.name == "value")
            .unwrap()
            .id;
        assert!(dead.index() < retained.index());
        assert!(retained.index() < inline_parameter.index());
        assert!(inline_parameter.index() < callback.index());
        assert!(!original.blocks.iter().flat_map(|block| &block.statements).any(|statement| matches!(statement.kind, StatementKind::Let { local, .. } if local == dead)));
        let lifted = program
            .functions
            .iter()
            .filter(|function| function.debug_kind == hir::FunctionDebugKind::Inline)
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(lifted.len(), 1);
        assert_eq!(lifted[0].capture_count, 1);
        assert_eq!(lifted[0].params[0].name, "retained");
        assert!(crate::sequences::prune::unused_locals(
            guarded_function_mut(&mut program)
        ));
        let function = guarded_function(&program);
        assert_eq!(function.locals.len(), original.locals.len() - 2);
        assert!(!function.locals.iter().any(|local| local.name == "dead"));
        assert!(!function.locals.iter().any(|local| local.name == "value"));
        let moved_retained = function
            .locals
            .iter()
            .find(|local| local.name == "retained")
            .unwrap()
            .id;
        let moved_callback = function
            .locals
            .iter()
            .find(|local| local.name == "callback")
            .unwrap()
            .id;
        assert_eq!(moved_retained.index(), retained.index() - 1);
        assert_eq!(moved_callback.index(), callback.index() - 2);
        assert!(function.blocks.iter().flat_map(|block| &block.statements).any(|statement| matches!(&statement.kind, StatementKind::Let { local, value } if *local == moved_callback && matches!(&value.kind, E::ClosureRef { function, captures } if *function == lifted[0].id && captures == &[moved_retained]))));
        assert_eq!(
            program
                .functions
                .iter()
                .filter(|function| function.debug_kind == hir::FunctionDebugKind::Inline)
                .cloned()
                .collect::<Vec<_>>(),
            lifted,
            "caller compaction preserves separately lifted callback ABI"
        );
        assert_eq!(assert_plans(&program, &types, 0), 1);
        let compacted = program.clone();
        assert!(crate::sequences::prune::unused_locals(
            guarded_function_mut(&mut program)
        ));
        assert!(
            program == compacted,
            "second unused_locals must preserve the exact compacted callback program; release={release}"
        );
    }
}

const COMPOSED_ABSENT: &str = r#"namespace app
function inspect[T](view source: optional[list[int64]], absent: optional[T]) returns int64:
    list[int64] alias = view((view source) handle: return 0)
    T impossible = absent handle: return list.sum[int64](view alias)
    trace impossible
    return list.sum[int64](view alias)
function main() returns int64:
    optional[list[int64]] source = some(list(4, 5))
    return inspect(view source, none)
"#;

#[test]
fn ordinary_borrowed_sums_unrelated_optional_never_preparation_keeps_complete_valid_function() {
    for release in [false, true] {
        let (mut program, types) = lowered(COMPOSED_ABSENT, release);
        assert_eq!(assert_plans(&program, &types, 0), 1);
        let original = guarded_function(&program).clone();
        assert!(
            original
                .locals
                .iter()
                .any(|local| local.ty == TypeInterner::NEVER)
        );
        let absent_site = original.blocks.iter().find_map(|block| block.statements.iter().find_map(|statement| match statement.kind {
            StatementKind::SumTake { source, success: true, .. } if matches!(types.resolve(original.local(source).unwrap().ty), Type::Optional(payload) if *payload == TypeInterner::NEVER) => Some(block.id),
            _ => None,
        })).unwrap();
        crate::prepare_native_uninhabited_sums(&mut program, &types);
        let prepared = guarded_function(&program);
        crate::validate_call_ownership(&program, &types).unwrap();
        assert_eq!(assert_plans(&program, &types, 0), 1);
        if prepared != &original {
            assert!(!prepared.blocks.iter().flat_map(|block| &block.statements).any(|statement| matches!(statement.kind, StatementKind::SumTake { source, success: true, .. } if matches!(types.resolve(prepared.local(source).unwrap().ty), Type::Optional(payload) if *payload == TypeInterner::NEVER))));
            assert!(
                ControlFlowGraph::analyze(prepared)
                    .unwrap()
                    .reverse_postorder()
                    .iter()
                    .all(|id| (id.index() as usize) < prepared.blocks.len())
            );
        } else {
            assert!(
                ControlFlowGraph::analyze(prepared)
                    .unwrap()
                    .reverse_postorder()
                    .contains(&absent_site),
                "a rollback preserves the complete original graph"
            );
        }
        let once = program.clone();
        crate::prepare_native_uninhabited_sums(&mut program, &types);
        assert_eq!(program, once);
    }
}

fn assert_refused(program: &crate::Program, types: &TypeInterner) {
    assert!(crate::validate_call_ownership(program, types).is_err());
    let function = program
        .functions
        .iter()
        .find(|function| {
            function.identity.declaration.namespace == "app"
                && function.identity.declaration.name == "inspect"
        })
        .unwrap();
    assert!(CopyValuePlan::analyze_storage(function, types, Some(program)).is_err());
    assert!(MoveValuePlan::analyze(program, function, types).is_err());
    if let Some(witness) = &function.ordinary_borrowed_sums {
        if let Some(row) = witness.rows.first() {
            assert!(
                function
                    .ordinary_borrowed_sum_projection(
                        row.project.block,
                        row.project.statement_index
                    )
                    .is_err()
            );
        }
        let mut changed = function.clone();
        assert!(!crate::sequences::prune::unreachable(&mut changed));
        assert_eq!(
            changed, *function,
            "failed block pruning rolls back the exact malformed input"
        );
        assert!(!crate::sequences::prune::unused_locals(&mut changed));
        assert_eq!(
            changed, *function,
            "failed local compaction rolls back the exact malformed input"
        );
    }
    let mut changed = program.clone();
    crate::prepare_native_sequences(&mut changed, types);
    assert_eq!(
        changed, *program,
        "validation refusal happens before preparation can hide the mutation"
    );
}

fn reseal_only_blocks(function: &mut Function) {
    let blocks = function.blocks.clone();
    Arc::make_mut(&mut function.ordinary_borrowed_sums.as_mut().unwrap().sealed).blocks = blocks;
}

#[test]
fn ordinary_borrowed_sums_changed_headers_tag_edge_and_failure_are_refused() {
    for source in [SOURCES[0], SOURCES[1]] {
        let (program, types) = lowered(source, false);
        assert_eq!(assert_plans(&program, &types, 0), 2);
        for corruption in 0..9 {
            let mut changed = program.clone();
            let function = guarded_function_mut(&mut changed);
            let row = function.ordinary_borrowed_sums.as_ref().unwrap().rows[0].clone();
            match corruption {
                0 => function.locals[row.source().index() as usize].view_source = None,
                1 => {
                    function.locals[row.output().index() as usize].view_source = Some(row.source())
                }
                2 => function.locals[row.alias().index() as usize].mutable = true,
                3 => function.locals[row.backing().index() as usize].ty = row.payload_type(),
                4 => function.locals[row.tag().index() as usize].ty = TypeInterner::INT64,
                5 => {
                    function.blocks[row.observe.block.index() as usize].statements
                        [row.observe.statement_index]
                        .kind = StatementKind::SumTag {
                        source: row.backing(),
                        target: row.tag(),
                    }
                }
                6 => {
                    let TerminatorKind::Branch { condition, .. } = &mut function.blocks
                        [row.observe.block.index() as usize]
                        .terminator
                        .kind
                    else {
                        panic!("genuine selecting edge")
                    };
                    condition.kind = E::Bool(true);
                }
                7 => {
                    let TerminatorKind::Branch {
                        then_block,
                        else_block,
                        ..
                    } = &mut function.blocks[row.observe.block.index() as usize]
                        .terminator
                        .kind
                    else {
                        panic!("genuine selecting edge")
                    };
                    std::mem::swap(then_block, else_block);
                }
                _ => {
                    if let Some(error) = row.error() {
                        function.locals[error.index() as usize].view_source = Some(row.backing());
                    } else {
                        function.blocks[row.project.block.index() as usize].statements
                            [row.project.statement_index]
                            .kind = StatementKind::SumTake {
                            source: row.source(),
                            target: row.output(),
                            success: false,
                        };
                    }
                }
            }
            assert_refused(&changed, &types);
        }
    }
}

#[test]
fn ordinary_borrowed_sums_copied_missing_and_disconnected_projection_survive_cache_reseal_refusal()
{
    let (program, types) = lowered(SOURCES[1], false);
    assert_eq!(assert_plans(&program, &types, 0), 2);
    for corruption in 0..4 {
        let mut changed = program.clone();
        let function = guarded_function_mut(&mut changed);
        let row = function.ordinary_borrowed_sums.as_ref().unwrap().rows[0].clone();
        assert_eq!(function.local(row.output()).unwrap().ty, row.payload_type());
        assert_eq!(
            function.local(row.output()).unwrap().view_source,
            Some(row.backing())
        );
        match corruption {
            0 => {
                let copied = function.blocks[row.project.block.index() as usize].statements
                    [row.project.statement_index]
                    .clone();
                function.blocks[row.project.block.index() as usize]
                    .statements
                    .push(copied);
            }
            1 => {
                function.blocks[row.project.block.index() as usize]
                    .statements
                    .remove(row.project.statement_index);
            }
            2 => {
                let TerminatorKind::Branch { then_block, .. } = &mut function.blocks
                    [row.observe.block.index() as usize]
                    .terminator
                    .kind
                else {
                    panic!("genuine selecting edge")
                };
                *then_block = row.continuation;
                let cfg = ControlFlowGraph::analyze(function).unwrap();
                assert!(!cfg.reverse_postorder().contains(&row.project.block));
                assert!(
                    matches!(function.blocks[row.project.block.index() as usize].statements[row.project.statement_index].kind, StatementKind::SumTake { source, target, success: true } if source == row.source() && target == row.output())
                );
            }
            _ => {
                let site = row.failure.unwrap();
                function.blocks[site.block.index() as usize].statements[site.statement_index]
                    .kind = StatementKind::SumTake {
                    source: row.source(),
                    target: row.error().unwrap(),
                    success: true,
                };
            }
        }
        assert_refused(&changed, &types);
        reseal_only_blocks(guarded_function_mut(&mut changed));
        assert_refused(&changed, &types);
    }
}

#[test]
fn ordinary_borrowed_sums_private_inventory_and_original_association_resist_row_reseal() {
    let (program, types) = lowered(SOURCES[0], false);
    for corruption in 0..6 {
        let mut changed = program.clone();
        let function = guarded_function_mut(&mut changed);
        let witness = function.ordinary_borrowed_sums.as_mut().unwrap();
        let independent = witness.sealed.clone();
        match corruption {
            0 => {
                witness.rows.pop();
            }
            1 => {
                witness.rows.push(witness.rows[0].clone());
            }
            2 => witness.rows[0].original_path.push(usize::MAX),
            3 => {
                let sibling = witness.rows[1].clone();
                witness.rows[0].original_path = sibling.original_path;
                witness.rows[0].original_alias = sibling.original_alias;
                witness.rows[0].original_backing = sibling.original_backing;
                witness.rows[0].original = sibling.original;
            }
            4 => witness.rows[0].path = OrdinarySumPayloadPath::ResultOk,
            _ => {
                let (left, right) = witness.rows.split_at_mut(1);
                std::mem::swap(&mut left[0].original_path, &mut right[0].original_path);
                std::mem::swap(&mut left[0].original, &mut right[0].original);
                std::mem::swap(&mut left[0].original_alias, &mut right[0].original_alias);
                std::mem::swap(
                    &mut left[0].original_backing,
                    &mut right[0].original_backing,
                );
                assert_ne!(left[0].original_alias.id, right[0].original_alias.id);
            }
        }
        assert!(Arc::ptr_eq(&witness.sealed, &independent));
        assert!(!same_rows(&witness.rows, &independent.rows));
        assert_refused(&changed, &types);
        if corruption == 5 {
            // Both original keys remain genuine and unique; their association
            // with current sites is independent private authority, not a cache.
            reseal_only_blocks(guarded_function_mut(&mut changed));
            assert_refused(&changed, &types);
            continue;
        }
        let witness = guarded_function_mut(&mut changed)
            .ordinary_borrowed_sums
            .as_mut()
            .unwrap();
        let rows = witness.rows.clone();
        Arc::make_mut(&mut witness.sealed).rows = rows;
        assert!(same_rows(&witness.rows, &witness.sealed.rows));
        assert_refused(&changed, &types);
    }
}

#[test]
fn ordinary_borrowed_sums_missing_and_foreign_witnesses_cannot_authorize_typed_views() {
    let (program, types) = lowered(SOURCES[0], false);
    let (foreign, _) = lowered(SOURCES[1], false);
    for replacement in [
        None,
        guarded_function(&foreign).ordinary_borrowed_sums.clone(),
    ] {
        let mut changed = program.clone();
        let function = guarded_function_mut(&mut changed);
        let row = function.ordinary_borrowed_sums.as_ref().unwrap().rows[0].clone();
        function.ordinary_borrowed_sums = replacement;
        assert_eq!(
            function.local(row.output()).unwrap().view_source,
            Some(row.backing())
        );
        assert!(
            matches!(function.blocks[row.project.block.index() as usize].statements[row.project.statement_index].kind, StatementKind::SumTake { source, target, success: true } if source == row.source() && target == row.output())
        );
        assert_refused(&changed, &types);
    }
}

#[test]
fn ordinary_borrowed_sums_alias_query_requires_the_exact_current_initializer_reference() {
    let (program, types) = lowered(SOURCES[0], false);
    assert_eq!(assert_plans(&program, &types, 0), 2);
    let function = guarded_function(&program);
    let row = &function.ordinary_borrowed_sums.as_ref().unwrap().rows[0];
    let StatementKind::Let { value, .. } = &function.blocks[row.alias_site.block.index() as usize]
        .statements[row.alias_site.statement_index]
        .kind
    else {
        panic!("genuine alias initializer")
    };
    assert!(
        function
            .ordinary_borrowed_sum_alias(row.alias(), value)
            .unwrap()
    );
    assert!(
        !function
            .ordinary_borrowed_sum_alias(row.alias(), &value.clone())
            .unwrap()
    );
    assert!(
        !function
            .ordinary_borrowed_sum_alias(row.output(), value)
            .unwrap()
    );
}

#[test]
fn ordinary_borrowed_sums_malformed_maps_and_edits_leave_input_unchanged() {
    let (mut program, types) = lowered(SOURCES[0], false);
    for function in &mut program.functions {
        assert!(crate::sequences::prune::unreachable(function));
    }
    assert_eq!(assert_plans(&program, &types, 0), 2);
    let before = guarded_function(&program);
    let block_map = before
        .blocks
        .iter()
        .map(|block| Some(block.id))
        .collect::<Vec<_>>();
    let local_map = before
        .locals
        .iter()
        .map(|local| Some(local.id))
        .collect::<Vec<_>>();
    for corruption in 0..6 {
        let mut function = before.clone();
        let result = match corruption {
            0 => {
                let mut map = block_map.clone();
                map.pop();
                remap_blocks(&mut function, before, &map)
            }
            1 => {
                let mut map = block_map.clone();
                map[before.entry.index() as usize] = None;
                remap_blocks(&mut function, before, &map)
            }
            2 => {
                let mut map = block_map.clone();
                map.swap(0, 1);
                remap_blocks(&mut function, before, &map)
            }
            3 => {
                let mut map = local_map.clone();
                map.pop();
                remap_locals(&mut function, before, &map)
            }
            4 => {
                let mut map = local_map.clone();
                map.swap(0, 1);
                remap_locals(&mut function, before, &map)
            }
            _ => {
                let witness = before.ordinary_borrowed_sums.as_ref().unwrap();
                let mut map = local_map.clone();
                map[witness.rows[0].backing().index() as usize] = None;
                let mut next = 0;
                for local in map.iter_mut().flatten() {
                    *local = LocalId::new(next);
                    next += 1;
                }
                remap_locals(&mut function, before, &map)
            }
        };
        assert!(result.is_err(), "malformed canonical map {corruption}");
        assert_eq!(function, *before);
        validate(&function, &types).unwrap();
    }
    let mut changed = before.clone();
    let row = &before.ordinary_borrowed_sums.as_ref().unwrap().rows[0];
    changed.locals[row.output().index() as usize].view_source = None;
    let malformed = changed.clone();
    assert!(remap_blocks(&mut changed, before, &block_map).is_err());
    assert_eq!(changed, malformed);
    assert!(remap_locals(&mut changed, before, &local_map).is_err());
    assert_eq!(changed, malformed);
}
