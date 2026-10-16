use super::*;

const OWNED: &str = include_str!(
    "../../../jett_driver/tests/native_conformance/ordinary_borrowed_sums_mutable/03_mutable_owned_scoped_forwarded.jett"
);
const COPIED: &str = include_str!(
    "../../../jett_driver/tests/native_conformance/ordinary_borrowed_sums_mutable/15_mutable_backing_copied_payloads.jett"
);
const REPLACED: &str = include_str!(
    "../../../jett_driver/tests/native_conformance/ordinary_borrowed_sums_mutable/16_copied_payload_survives_shell_replacement.jett"
);

fn prepare_source03_for_ownership_plans(program: &mut crate::Program, types: &TypeInterner) {
    let has_for = |program: &crate::Program| {
        program
            .functions
            .iter()
            .filter(|function| function.identity.declaration.namespace == "app")
            .flat_map(|function| &function.blocks)
            .any(|block| matches!(block.terminator.kind, TerminatorKind::ForEach { .. }))
    };
    assert!(
        has_for(program),
        "Source03 retains its genuine For loop before preparation"
    );
    crate::validate_call_ownership(program, types).unwrap();
    for function in &program.functions {
        validate(function, types).unwrap();
    }
    // Like the original Source03 test, defer Copy/Move plans until canonical
    // sequence lowering has replaced the real ForEach terminator.
    crate::prepare_native_sequences(program, types);
    assert!(
        !has_for(program),
        "Source03 must complete its actual For lowering"
    );
}

#[test]
fn fresh_mutable_sum_lowering_requires_the_real_hir_owner_receipt() {
    let source = r#"namespace app
function first() returns string:
    mutable result[string, string] source = ok("first")
    string alias = view((view source) handle error: return error)
    return alias
function second() returns string:
    mutable result[string, string] source = ok("second")
    string alias = view((view source) handle error: return error)
    return alias
"#;
    let file = FileId::new(0);
    let parsed = jett_parser::parse(source, file);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let resolved = jett_resolve::resolve(&parsed.module);
    let checked = jett_typecheck::check(&parsed.module, &resolved);
    assert!(
        resolved
            .diagnostics
            .iter()
            .chain(&checked.diagnostics)
            .all(|error| error.severity != jett_diagnostics::Severity::Error)
    );
    let high = hir::lower(
        &parsed.module,
        &resolved,
        &checked,
        &HashMap::from([(file, SourceOrigin::Project)]),
    )
    .unwrap();
    crate::lower(&high, &checked.interner).unwrap();
    for invalid in ["missing", "foreign"] {
        let mut changed = high.clone();
        if invalid == "missing" {
            changed.functions[0].mutable_sum_backings.clear();
        } else {
            changed.functions[0].mutable_sum_backings =
                high.functions[1].mutable_sum_backings.clone();
        }
        let errors = crate::lower(&changed, &checked.interner).unwrap_err();
        assert!(
            errors.iter().any(|error| error.message.contains("mutable")
                && (error.message.contains("receipt") || error.message.contains("context"))),
            "{invalid}: {errors:?}"
        );
    }
}

#[test]
fn mutable_sum_roots_preserve_original_rows_and_nonowning_payload_liveness() {
    for (fixture, source) in [OWNED, COPIED, REPLACED].into_iter().enumerate() {
        for release in [false, true] {
            let (mut program, types) = lowered(source, release);
            if fixture == 0 {
                prepare_source03_for_ownership_plans(&mut program, &types);
            }
            for prepared in [false, true] {
                if prepared {
                    crate::prepare_native_sequences(&mut program, &types);
                    for function in &mut program.functions {
                        assert!(crate::sequences::prune::unreachable(function));
                        assert!(crate::sequences::prune::unused_locals(function));
                    }
                }
                assert_plans(&program, &types, fixture + 100);
                let mut mutable_rows = 0;
                for function in &program.functions {
                    let Some(witness) = &function.ordinary_borrowed_sums else {
                        continue;
                    };
                    let plan = MoveValuePlan::analyze(&program, function, &types).unwrap();
                    for row in &witness.rows {
                        if !row.original_backing.mutable {
                            continue;
                        }
                        mutable_rows += 1;
                        assert!(row.mutable_backing.is_some());
                        assert!(function.local(row.backing()).unwrap().mutable);
                        assert!(
                            plan.owned_locals
                                .contains(&(row.backing().index() as usize))
                        );
                        for local in [row.source(), row.output()] {
                            assert!(!function.local(local).unwrap().mutable);
                            assert_eq!(
                                function.local(local).unwrap().view_source,
                                Some(row.backing())
                            );
                            assert!(!plan.owned_locals.contains(&(local.index() as usize)));
                        }
                        let alias = function.local(row.alias()).unwrap();
                        if alias.view_source.is_some() {
                            assert!(!alias.mutable);
                            assert!(!plan.owned_locals.contains(&(alias.id.index() as usize)));
                            let mut observed = false;
                            for live in plan
                                .live_in
                                .iter()
                                .chain(&plan.live_out)
                                .chain(plan.live_after_statement.iter().flatten())
                            {
                                if live.contains(&(row.alias().index() as usize)) {
                                    observed = true;
                                    assert!(live.contains(&(row.backing().index() as usize)));
                                }
                            }
                            assert!(observed);
                        } else {
                            assert_eq!(
                                plan.owned_locals.contains(&(alias.id.index() as usize)),
                                alias.ty == TypeInterner::STRING
                            );
                            let live = &plan.live_after_statement
                                [row.alias_site.block.index() as usize]
                                [row.alias_site.statement_index];
                            assert!(!live.contains(&(row.source().index() as usize)));
                            assert!(!live.contains(&(row.output().index() as usize)));
                        }
                    }
                }
                assert!(
                    mutable_rows > 0,
                    "Source variant must reach the mutable guard"
                );
            }
        }
    }
}

#[test]
fn mutable_sum_rows_require_original_proof_even_after_test_only_current_reseal() {
    let (program, types) = lowered(OWNED, false);
    for invalid in ["missing", "foreign", "header", "owning output"] {
        let mut changed = program.clone();
        let selected = changed
            .functions
            .iter()
            .position(|function| {
                function
                    .ordinary_borrowed_sums
                    .as_ref()
                    .is_some_and(|witness| {
                        witness.rows.iter().any(|row| row.original_backing.mutable)
                    })
            })
            .unwrap();
        let function = &mut changed.functions[selected];
        let witness = function.ordinary_borrowed_sums.as_mut().unwrap();
        let index = witness
            .rows
            .iter()
            .position(|row| row.original_backing.mutable)
            .unwrap();
        let row = witness.rows[index].clone();
        match invalid {
            "missing" => witness.rows[index].mutable_backing = None,
            "foreign" => {
                witness.rows[index].mutable_backing = witness
                    .rows
                    .iter()
                    .find(|other| other.alias() != row.alias())
                    .and_then(|other| other.mutable_backing.clone())
            }
            "header" => function.locals[row.backing().index() as usize].mutable = false,
            "owning output" => function.locals[row.output().index() as usize].view_source = None,
            _ => unreachable!(),
        }
        let rows = function
            .ordinary_borrowed_sums
            .as_ref()
            .unwrap()
            .rows
            .clone();
        let resealed = Arc::new(CurrentSeal::new(function, &rows));
        function.ordinary_borrowed_sums.as_mut().unwrap().sealed = resealed;
        assert!(validate(function, &types).is_err(), "{invalid}");
        assert!(
            MoveValuePlan::analyze(&changed, &changed.functions[selected], &types).is_err(),
            "{invalid}"
        );
    }
}

#[test]
fn mutable_sum_persistent_alias_still_blocks_root_replacement_after_last_read() {
    let (mut program, types) = lowered(OWNED, false);
    prepare_source03_for_ownership_plans(&mut program, &types);
    let index = program
        .functions
        .iter()
        .position(|function| function.identity.declaration.name == "inspect_optional")
        .unwrap();
    MoveValuePlan::analyze(&program, &program.functions[index], &types)
        .expect("the prepared unmodified baseline must pass before the illegal owner write");
    let mut changed = program.clone();
    let function = &mut changed.functions[index];
    let row = function.ordinary_borrowed_sums.as_ref().unwrap().rows[0].clone();
    let target = Expression {
        kind: E::Local(row.backing()),
        ty: row.sum_type(),
        span: function.span,
    };
    let replacement = Expression {
        kind: E::OptionalNone,
        ty: row.sum_type(),
        span: function.span,
    };
    let failure_blocks = function
        .ordinary_borrowed_sums
        .as_ref()
        .unwrap()
        .rows
        .iter()
        .map(|row| row.failed)
        .collect::<std::collections::HashSet<_>>();
    let exit = function.blocks.iter_mut().filter(|block|
        matches!(block.terminator.kind, TerminatorKind::Return(Some(ref value)) if matches!(value.kind, E::Nothing))
        && !failure_blocks.contains(&block.id)).last().unwrap();
    exit.statements.push(Statement {
        kind: StatementKind::Assign {
            target,
            value: replacement,
        },
        span: function.span,
    });
    // Only a unit test may bypass the complete current-tree equality. The real
    // original declaration/row proof stays intact, and Flow must still refuse
    // mutation with its persistent user alias, even after that alias's last read.
    let rows = function
        .ordinary_borrowed_sums
        .as_ref()
        .unwrap()
        .rows
        .clone();
    let resealed = Arc::new(CurrentSeal::new(function, &rows));
    function.ordinary_borrowed_sums.as_mut().unwrap().sealed = resealed;
    let error = MoveValuePlan::analyze(&changed, &changed.functions[index], &types).unwrap_err();
    assert!(
        error.contains("after creating a local view alias"),
        "{error}"
    );
}

#[test]
fn mutable_owned_sum_receipt_survives_real_nonidentity_callback_compaction() {
    // Reuse the canonical dead-slot/callback composition. Only the original
    // parameter's access/acquisition and its actual call change to owned mutable.
    let source = CALLBACK_COMPACTION
        .replace("inspect(view source:", "inspect(mutable source:")
        .replace("inspect(view source)", "inspect(source)");
    for release in [false, true] {
        let (mut program, types) = lowered(&source, release);
        let original = guarded_function(&program).clone();
        let original_row = original.ordinary_borrowed_sums.as_ref().unwrap().rows[0].clone();
        assert!(original_row.original_backing.mutable);
        assert!(original_row.mutable_backing.is_some());
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
        assert!(crate::sequences::prune::unused_locals(
            guarded_function_mut(&mut program)
        ));
        let compacted = guarded_function(&program);
        let row = &compacted.ordinary_borrowed_sums.as_ref().unwrap().rows[0];
        assert_eq!(compacted.locals.len(), original.locals.len() - 2);
        assert!(
            !compacted
                .locals
                .iter()
                .any(|local| local.name == "dead" || local.name == "value")
        );
        assert_eq!(
            compacted
                .locals
                .iter()
                .find(|local| local.name == "retained")
                .unwrap()
                .id
                .index(),
            retained.index() - 1
        );
        assert_eq!(
            compacted
                .locals
                .iter()
                .find(|local| local.name == "callback")
                .unwrap()
                .id
                .index(),
            callback.index() - 2
        );
        // The generated projection headers follow the removed Source slots.
        // These comparisons make an identity/no-op preparation fail the test.
        assert_eq!(row.source().index(), original_row.source().index() - 2);
        assert_eq!(row.output().index(), original_row.output().index() - 2);
        assert_ne!(row.output(), original_row.output());
        assert_eq!(row.mutable_backing, original_row.mutable_backing);
        assert_eq!(assert_plans(&program, &types, 116), 1);
        crate::prepare_native_sequences(&mut program, &types);
        assert_eq!(assert_plans(&program, &types, 116), 1);
    }
}
