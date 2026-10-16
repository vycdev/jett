//! Focused controls for actual initial actor callable sealing epochs.
//! Every positive epoch comes from the unchanged checked Source and real lowerer.
use super::*;
use crate::{DeclarationKind, ExpressionKind, FunctionId};
use jett_common::{FileId, SourceOrigin};
use jett_typecheck::{CheckOptions, CheckedResourceProgram};
use std::{collections::HashMap, sync::Arc};

const SOURCE: &str = include_str!(
    "../../jett_comptime/src/resource_execution/fixtures/actor_handler_direct_getter.jett"
);

// Separate closed Source control for an intentionally unrepresented initial peer
// composition. The genuine direct-getter fixture remains byte-identical.
const TEMPORARY_SOURCE: &str = r#"namespace app
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

fn setup(release: bool) -> (Program, Arc<CheckedResourceProgram>, Arc<Epoch>) {
    let file = FileId::new(0);
    let checked = Arc::new(
        CheckedResourceProgram::prepare(
            jett_parser::parse(SOURCE, file),
            HashMap::from([(file, SourceOrigin::Project)]),
            &[],
            CheckOptions { release },
        )
        .unwrap_or_else(|error| {
            panic!(
                "unchanged direct-handler Source; release={release}: {error:?}; {:?}",
                error.diagnostics()
            )
        }),
    );
    let program = crate::lower_checked_ordinary_program(&checked)
        .unwrap_or_else(|errors| panic!("genuine initial sealing; release={release}: {errors:?}"));
    crate::validate(&program).unwrap();
    crate::validate_backend_types(&program, &checked.checked().interner).unwrap();
    program
        .resource_source
        .validate_current(&program, &checked.checked().interner)
        .unwrap();
    let handler = program
        .functions
        .iter()
        .find(|function| {
            function.identity.declaration.kind == DeclarationKind::ActorHandler
                && function.identity.declaration.name == "Worker.read"
        })
        .expect("actual checked Worker.read");
    let edge = epoch(handler)
        .expect("actual initial token edits require an actor epoch")
        .clone();
    assert!(Arc::ptr_eq(&edge.checked, &checked));
    assert!(edge.affected.contains(&handler.id));
    (program, checked, edge)
}

fn same_program(left: &Program, right: &Program) -> bool {
    left.resource_source == right.resource_source
        && left.resource_manifest == right.resource_manifest
        && left.equality_methods == right.equality_methods
        && crate::resource_materialization::functions_equal(&left.functions, &right.functions)
}

#[test]
fn actual_initial_epoch_restores_only_its_exact_historical_actor_metadata() {
    for release in [false, true] {
        let (program, checked, edge) = setup(release);
        // These retained tables must precede attachment of their own edge.
        assert!(
            edge.sources
                .iter()
                .all(|function| epoch(function).is_none())
        );
        assert!(
            edge.after
                .functions
                .iter()
                .all(|function| epoch(function).is_none())
        );
        assert_eq!(edge.after.resource_source, ResourceSourceArchive::empty());
        let view = source_view(&edge.sources, &program, &program.functions, &checked).unwrap();
        assert!(crate::reflected_callable::source_functions_equal(
            &view,
            &edge.sources
        ));
        assert!(!crate::resource_materialization::functions_equal(
            &view,
            &program.functions
        ));
        for id in &edge.affected {
            let index = id.index() as usize;
            assert_eq!(view[index].actor_handler, edge.sources[index].actor_handler);
            assert_eq!(
                view[index].actor_state_views,
                edge.sources[index].actor_state_views
            );
            let mut preserve_actual = program.functions[index].clone();
            preserve_actual.actor_handler = view[index].actor_handler.clone();
            preserve_actual.actor_state_views = view[index].actor_state_views.clone();
            assert!(crate::resource_materialization::functions_equal(
                &[preserve_actual],
                &[view[index].clone()]
            ));
        }
    }
}

#[test]
fn equal_source_table_copies_cannot_impersonate_the_private_source_epoch() {
    for release in [false, true] {
        let (program, checked, edge) = setup(release);
        let copied = Arc::new(edge.sources.as_ref().clone());
        assert!(!Arc::ptr_eq(&copied, &edge.sources));
        assert!(crate::resource_materialization::functions_equal(
            &copied,
            &edge.sources
        ));
        assert!(source_view(&copied, &program, &program.functions, &checked).is_err());
        source_view(&edge.sources, &program, &program.functions, &checked).unwrap();
    }
}

#[test]
fn actor_epoch_clones_share_identity_and_all_private_strong_edges_expire() {
    for release in [false, true] {
        let (weak_edge, weak_sources, weak_checked) = {
            let (program, checked, edge) = setup(release);
            let cloned = program.clone();
            let handler = cloned
                .functions
                .iter()
                .find(|function| {
                    function.identity.declaration.kind == DeclarationKind::ActorHandler
                        && function.identity.declaration.name == "Worker.read"
                })
                .unwrap();
            assert!(Arc::ptr_eq(epoch(handler).unwrap(), &edge));
            let weak_edge = Arc::downgrade(&edge);
            let weak_sources = Arc::downgrade(&edge.sources);
            let weak_checked = Arc::downgrade(&checked);
            drop(program);
            drop(edge);
            assert!(
                weak_edge.upgrade().is_some(),
                "the actual Program clone keeps its original epoch"
            );
            {
                let held = weak_edge.upgrade().unwrap();
                source_view(&held.sources, &cloned, &cloned.functions, &checked).unwrap();
            }
            drop(cloned);
            assert!(
                weak_edge.upgrade().is_none(),
                "the completed epoch must not own itself"
            );
            assert!(
                weak_sources.upgrade().is_none(),
                "raw Source tables must not cycle through sealed plans"
            );
            drop(checked);
            (weak_edge, weak_sources, weak_checked)
        };
        assert!(weak_edge.upgrade().is_none());
        assert!(weak_sources.upgrade().is_none());
        assert!(
            weak_checked.upgrade().is_none(),
            "the complete dropped HIR graph must release its checked Source"
        );
    }
}

fn corrupt(functions: &mut Vec<Function>, edit: usize) {
    let sibling = functions
        .iter()
        .position(|function| {
            function.identity.declaration.kind == DeclarationKind::Function
                && function.identity.declaration.name == "five"
        })
        .unwrap();
    let handler = functions
        .iter()
        .position(|function| function.actor_handler.is_some())
        .unwrap();
    match edit {
        0 => {
            let mut changed = false;
            crate::resource_materialization::walk_block(
                &mut functions[sibling].body,
                &mut |expression| {
                    if let ExpressionKind::Int(value) = &mut expression.kind {
                        if !changed {
                            *value += 1;
                            changed = true;
                        }
                    }
                    Ok(())
                },
            )
            .unwrap();
            assert!(changed, "real five body must contain its literal");
        }
        1 => functions[sibling].capture_count += 1,
        2 => functions[handler].body.span.end += 1,
        3 => functions[handler].body.statements.reverse(),
        4 => {
            let mut extra = functions[sibling].clone();
            extra.id = FunctionId::new(functions.len() as u32);
            extra.identity.declaration.name = "$unsupported.appended".into();
            functions.push(extra);
        }
        _ => unreachable!(),
    }
}

#[test]
fn source_epoch_refuses_changed_siblings_headers_bodies_and_appended_functions() {
    for release in [false, true] {
        let (program, checked, edge) = setup(release);
        for edit in 0..5 {
            // A private epoch view is exact for the complete restored table,
            // rather than only checking the protected actor or a Source prefix.
            let mut restored = program.functions.clone();
            corrupt(&mut restored, edit);
            assert!(
                source_view(&edge.sources, &program, &restored, &checked).is_err(),
                "restored edit {edit}"
            );
            // Current HIR must independently remain at an authenticated checkpoint.
            let mut changed = program.clone();
            corrupt(&mut changed.functions, edit);
            let before = changed.clone();
            assert!(
                source_view(&edge.sources, &changed, &program.functions, &checked).is_err(),
                "current edit {edit}"
            );
            assert!(same_program(&changed, &before));
        }
        source_view(&edge.sources, &program, &program.functions, &checked).unwrap();
    }
}

#[test]
fn source_epoch_refuses_foreign_checked_programs_and_foreign_epoch_attachments() {
    for release in [false, true] {
        let (program, checked, edge) = setup(release);
        let (foreign, foreign_checked, foreign_edge) = setup(release);
        assert!(!Arc::ptr_eq(&checked, &foreign_checked));
        assert!(!Arc::ptr_eq(&edge, &foreign_edge));
        assert!(
            source_view(
                &edge.sources,
                &program,
                &program.functions,
                &foreign_checked
            )
            .is_err()
        );
        assert!(
            source_view(
                &foreign_edge.sources,
                &program,
                &program.functions,
                &checked
            )
            .is_err()
        );
        assert!(
            source_view(
                &edge.sources,
                &foreign,
                &foreign.functions,
                &foreign_checked
            )
            .is_err()
        );
        let mut restored = program.functions.clone();
        let handler = restored
            .iter_mut()
            .find(|function| function.actor_handler.is_some())
            .unwrap();
        handler.actor_handler.as_mut().unwrap().callable_epoch = Some(foreign_edge.clone());
        assert!(source_view(&edge.sources, &program, &restored, &checked).is_err());
        source_view(&edge.sources, &program, &program.functions, &checked).unwrap();
    }
}

#[test]
fn initial_actor_sealing_refuses_foreign_context_arena_and_later_entry_transactionally() {
    for release in [false, true] {
        let (program, checked, edge) = setup(release);
        let (_, foreign, _) = setup(release);
        // Retain the genuine producer's raw historical input. No Source syntax,
        // plan, actor entry, receipt or positive epoch is constructed manually.
        let mut before = edge.after.clone();
        before.functions = edge.sources.as_ref().clone();
        let types = &checked.checked().interner;
        assert!(!Arc::ptr_eq(&checked, &foreign));
        assert!(!std::ptr::eq(types, &foreign.checked().interner));
        let mut wrong_context = before.clone();
        assert!(
            crate::reflected_callable::seal_original_program(
                &mut wrong_context,
                Some(&foreign),
                types,
            )
            .is_err()
        );
        assert!(same_program(&wrong_context, &before));
        let mut wrong_arena = before.clone();
        assert!(
            crate::reflected_callable::seal_original_program(
                &mut wrong_arena,
                Some(&checked),
                &foreign.checked().interner,
            )
            .is_err()
        );
        assert!(same_program(&wrong_arena, &before));
        let mut later = before.clone();
        let errors = crate::reflected_callable::seal_program(&mut later, types).unwrap_err();
        assert!(errors.iter().any(|error| {
            error
                .message
                .contains("later actor sealing remains unsupported")
        }));
        assert!(same_program(&later, &before));
        let file = FileId::new(0);
        let temporary_checked = Arc::new(
            CheckedResourceProgram::prepare(
                jett_parser::parse(TEMPORARY_SOURCE, file),
                HashMap::from([(file, SourceOrigin::Project)]),
                &[],
                CheckOptions { release },
            )
            .unwrap_or_else(|error| panic!("separate temporary composition must check: {error:?}")),
        );
        let errors = crate::lower_checked_ordinary_program(&temporary_checked).unwrap_err();
        assert!(errors.iter().any(|error| error.message.contains(
            "initial actor callable sealing with temporary-owner finalization remains unsupported"
        )));
        // Refusals cannot alter the actual original program or its private edge.
        source_view(&edge.sources, &program, &program.functions, &checked).unwrap();
    }
}
