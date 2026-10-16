use super::*;

const SOURCE: &str = "namespace app\nstruct Packet:\n    data: list[int64]\n    sibling: list[int64]\nfunction make_packet() returns Packet:\n    return Packet(data: list(1, 2), sibling: list(3, 4))\nfunction inspect(view data: list[int64]) returns nothing:\n    return nothing\nfunction main() returns nothing:\n    list[int64] borrowed = view make_packet().data\n    inspect(view borrowed)\n    list[int64] forwarded = borrowed\n    inspect(view forwarded)\n    list[int64] independent = clone forwarded\n";

fn high(source: &str, release: bool) -> (Program, Arc<jett_typecheck::CheckedResourceProgram>) {
    let file = FileId::new(0);
    let checked = Arc::new(
        jett_typecheck::CheckedResourceProgram::prepare(
            jett_parser::parse(source, file),
            HashMap::from([(file, SourceOrigin::Project)]),
            &[],
            jett_typecheck::CheckOptions { release },
        )
        .unwrap(),
    );
    let program = lower_checked_ordinary_program(&checked).unwrap();
    (program, checked)
}
fn main(program: &Program) -> &Function {
    program
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "main")
        .unwrap()
}
fn main_mut(program: &mut Program) -> &mut Function {
    program
        .functions
        .iter_mut()
        .find(|function| function.identity.declaration.name == "main")
        .unwrap()
}

#[test]
fn whole_owner_and_other_fact_are_retained_for_unused_and_read_aliases() {
    let unused = SOURCE.replace("    inspect(view borrowed)\n    list[int64] forwarded = borrowed\n    inspect(view forwarded)\n    list[int64] independent = clone forwarded\n", "");
    for release in [false, true] {
        for source in [SOURCE, unused.as_str()] {
            let (program, checked) = high(source, release);
            let function = main(&program);
            let receipts =
                temporary_local_backings(&program, function, &checked.checked().interner).unwrap();
            let [receipt] = receipts.as_slice() else {
                panic!("one original temporary occurrence");
            };
            assert!(matches!(
                receipt.0.declaration.mode,
                CheckedBindingMode::View {
                    source: CheckedViewSource::Other
                }
            ));
            assert_eq!(receipt.original_scope().statement_index(), 0);
            assert_eq!(receipt.original_scope().body_span(), function.body.span);
            assert_eq!(
                function.locals[receipt.alias().index() as usize].view_source,
                Some(receipt.owner())
            );
            assert!(
                function.locals[receipt.owner().index() as usize]
                    .name
                    .is_empty()
            );
            assert_ne!(
                receipt.owner_initializer().ty,
                receipt.rewritten_initializer().ty
            );
            validate_backend_types(&program, &checked.checked().interner).unwrap();
        }
    }
}

#[test]
fn retained_archive_rejects_erased_foreign_and_duplicated_markers_before_pruning() {
    let (original, checked) = high(SOURCE, false);
    let (foreign, _) = high(SOURCE, false);
    let foreign_proof = markers(main(&foreign))[0].clone();
    for alteration in ["erase", "foreign", "duplicate", "owner", "alias", "scope"] {
        let mut changed = original.clone();
        let function = main_mut(&mut changed);
        match alteration {
            "erase" | "foreign" => {
                let StatementKind::Let { value, .. } = &mut function.body.statements[0].kind else {
                    panic!();
                };
                let ExpressionKind::Call {
                    ownership: CallOwnership::Source(source),
                    ..
                } = &mut value.kind
                else {
                    panic!();
                };
                source.clear_temporary_local_backing();
                if alteration == "foreign" {
                    source.set_temporary_local_backing(foreign_proof.clone());
                }
            }
            "duplicate" => {
                let statement = function.body.statements[0].clone();
                function.body.statements.push(statement);
            }
            "owner" => {
                function.body.statements.remove(0);
            }
            "alias" => {
                function.body.statements.remove(1);
            }
            "scope" => function.body.span.end += 1,
            _ => unreachable!(),
        }
        assert!(
            temporary_local_backings(&changed, main(&changed), &checked.checked().interner)
                .is_err(),
            "{alteration}"
        );
        assert!(
            validate_backend_types(&changed, &checked.checked().interner).is_err(),
            "{alteration}"
        );
    }
    let mut no_archive = original.clone();
    no_archive.resource_source = ResourceSourceArchive::empty();
    assert!(validate_backend_types(&no_archive, &checked.checked().interner).is_err());
    let mut erased_functions = original.clone();
    erased_functions.functions.clear();
    assert!(validate_backend_types(&erased_functions, &checked.checked().interner).is_err());
}

#[test]
fn genuine_unmarked_same_shape_source_call_does_not_replace_expected_receipt() {
    let source = SOURCE.replace(
        "function main()",
        "function spare() returns nothing:\n    Packet owned = make_packet()\nfunction main()",
    );
    let (mut program, checked) = high(&source, false);
    let spare = program
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "spare")
        .unwrap();
    let StatementKind::Let {
        value: spare_value, ..
    } = &spare.body.statements[0].kind
    else {
        panic!();
    };
    let ExpressionKind::Call { ownership, .. } = &spare_value.kind else {
        panic!();
    };
    let unmarked = ownership.clone();
    let StatementKind::Let { value, .. } = &mut main_mut(&mut program).body.statements[0].kind
    else {
        panic!();
    };
    let ExpressionKind::Call { ownership, .. } = &mut value.kind else {
        panic!();
    };
    *ownership = unmarked;
    assert!(
        temporary_local_backings(&program, main(&program), &checked.checked().interner).is_err()
    );
}

#[test]
fn raw_lowering_cannot_capture_the_archive_or_stamp_a_temporary_owner() {
    let file = FileId::new(0);
    let parsed = jett_parser::parse(SOURCE, file);
    let resolved = jett_resolve::resolve(&parsed.module);
    let checked = jett_typecheck::check(&parsed.module, &resolved);
    let errors = lower(
        &parsed.module,
        &resolved,
        &checked,
        &HashMap::from([(file, SourceOrigin::Project)]),
    )
    .unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("temporary views remain unsupported"))
    );
}

#[test]
fn no_edit_replay_does_not_authenticate_a_changed_body() {
    let (before, _) = high(SOURCE, false);
    let mut changed = before.clone();
    main_mut(&mut changed).body.statements.pop();
    assert!(callable_sealing(&mut changed, &before, &[]).is_err());
}

#[test]
fn original_canonical_capture_retains_coerced_direct_producer_and_is_single_use() {
    let source = "namespace app\ninterface Named:\n    function name(view self: Named) returns string\nimplement Named for int64:\n    function name(view self: int64) returns string:\n        return \"number\"\nstruct Packet:\n    data: list[int64]\n    sibling: list[int64]\nfunction make_packet(view input: Named) returns Packet:\n    return Packet(data: list(1, 2), sibling: list(3, 4))\nfunction main() returns nothing:\n    int64 number = 7\n    list[int64] borrowed = view make_packet(view number).data\n";
    for release in [false, true] {
        let (program, checked) = high(source, release);
        let receipts =
            temporary_local_backings(&program, main(&program), &checked.checked().interner)
                .unwrap();
        let [receipt] = receipts.as_slice() else {
            panic!();
        };
        assert!(receipt.0.canonical_producer.get().is_some());
        assert!(!resource_materialization::expression_equal(
            receipt.owner_initializer(),
            &receipt.0.producer
        ));
        let ExpressionKind::Call { args, .. } = &receipt.owner_initializer().kind else {
            panic!();
        };
        assert!(matches!(
            args[0].kind,
            ExpressionKind::InterfaceCoerce { .. }
        ));
        let archived = program.resource_source.clone();
        assert!(seal_original_program(&program, &checked.checked().interner).is_err());
        assert_eq!(program.resource_source, archived);
        validate_backend_types(&program, &checked.checked().interner).unwrap();
    }
}

#[test]
fn canonical_snapshot_keeps_foreign_argument_receipts_without_an_arc_cycle() {
    let source = "namespace app\nstruct Packet:\n    data: list[int64]\n    sibling: list[int64]\nfunction first_packet() returns Packet:\n    return Packet(data: list(1, 2), sibling: list(3, 4))\nfunction next_packet(view input: list[int64]) returns Packet:\n    return Packet(data: clone input, sibling: list(5, 6))\nfunction main() returns nothing:\n    list[int64] first = view first_packet().data\n    list[int64] second = view next_packet(view first).data\n";
    let (program, checked) = high(source, false);
    let receipts =
        temporary_local_backings(&program, main(&program), &checked.checked().interner).unwrap();
    let [first, second] = receipts.as_slice() else {
        panic!("two exact temporary occurrences");
    };
    assert!(marker(first.owner_initializer()).is_none());
    assert!(marker(second.owner_initializer()).is_none());
    let ExpressionKind::Call { ownership, .. } = &second.owner_initializer().kind else {
        panic!();
    };
    assert!(
        ownership.has_temporary_local_receipts(),
        "stripping the producer marker must keep the first alias's argument receipt"
    );
    let ExpressionKind::Call {
        ownership: original_ownership,
        ..
    } = &second.0.producer.kind
    else {
        panic!();
    };
    assert_eq!(
        ownership, original_ownership,
        "the exact foreign argument receipt and full Source certificate survive capture"
    );
    validate_backend_types(&program, &checked.checked().interner).unwrap();
    let first_weak = Arc::downgrade(&first.0);
    let second_weak = Arc::downgrade(&second.0);
    drop(receipts);
    drop(program);
    drop(checked);
    assert!(
        second_weak.upgrade().is_none(),
        "the canonical producer must not own its own marker"
    );
    assert!(
        first_weak.upgrade().is_none(),
        "foreign argument receipts form a forward-free dependency chain"
    );
}

fn unsealed_copy(proof: &TemporaryLocalBacking) -> TemporaryLocalBacking {
    let data = &proof.0;
    TemporaryLocalBacking(Arc::new(Original {
        checked: data.checked.clone(),
        declaration: data.declaration,
        declaration_address: data.declaration_address,
        owner: data.owner.clone(),
        alias: data.alias.clone(),
        scope: TemporaryLocalScope {
            body: data.scope.body,
            statement: data.scope.statement,
        },
        initializer: data.initializer.clone(),
        producer: data.producer.clone(),
        canonical_producer: OnceLock::new(),
        rewritten: data.rewritten.clone(),
        required_scopes: data.required_scopes.clone(),
        scoped_types: data.scoped_types.clone(),
    }))
}

#[test]
fn unsealed_and_tampered_original_capture_do_not_gain_storage_authority() {
    let (original, checked) = high(SOURCE, false);
    let proof = markers(main(&original))[0].clone();
    let StatementKind::Let { value: owner, .. } = &main(&original).body.statements[0].kind else {
        panic!();
    };
    let unsealed = unsealed_copy(&proof);
    assert!(!unsealed.matches_owner_initializer(owner));
    for alteration in ["target", "span", "alias", "scope", "marker"] {
        // Test-only synthetic preimage of the private original-finalization
        // gate. It is not a second compiler-generated Source occurrence.
        let candidate_proof = unsealed_copy(&proof);
        let mut candidate = original.clone();
        candidate.resource_source = ResourceSourceArchive::empty();
        let function = main_mut(&mut candidate);
        let StatementKind::Let { value, .. } = &mut function.body.statements[0].kind else {
            panic!();
        };
        let ExpressionKind::Call {
            ownership: CallOwnership::Source(source),
            function: target,
            ..
        } = &mut value.kind
        else {
            panic!();
        };
        source.set_temporary_local_backing(candidate_proof.clone());
        match alteration {
            "target" => *target = FunctionId(target.index() + 1),
            "span" => value.span.end += 1,
            "alias" => function.locals[candidate_proof.alias().index() as usize].mutable = true,
            "scope" => function.body.span.end += 1,
            "marker" => source.set_temporary_local_backing(proof.clone()),
            _ => unreachable!(),
        }
        assert!(
            seal_original_program(&candidate, &checked.checked().interner).is_err(),
            "{alteration}"
        );
        assert!(
            candidate_proof.0.canonical_producer.get().is_none(),
            "{alteration}"
        );
    }
}

fn required_values(
    checked: &Arc<jett_typecheck::CheckedResourceProgram>,
) -> jett_comptime::ExplicitComptimeValues {
    let evaluation = jett_comptime::evaluate_explicit_comptime_expressions_capture(
        checked.module(),
        checked.checked().reflection_metadata.clone(),
        Arc::new(
            jett_comptime::checked_types::CheckedExpressionTypes::from_checked_program(checked),
        ),
        Arc::new(checked.checked().breakpoint_exclusions.clone()),
    );
    assert!(
        evaluation.diagnostics.is_empty(),
        "{:?}",
        evaluation.diagnostics
    );
    evaluation.values
}

#[test]
fn temporary_execution_joins_genuine_graph_and_baked_peer_transitions() {
    let prefix = "namespace app\ntype Positive = int64 where value > 0\nstruct CallbackHolder:\n    callback: function() returns int64\nfunction five() returns int64:\n    return 5\nfunction reflected(view holder: CallbackHolder, view field: TypeField) returns function() returns Positive:\n    return type.field_value[CallbackHolder, function() returns Positive](view holder, view field)\nactor Counter:\n    mutable int64 count = 1\n    receive read responds int64:\n        respond count\nfunction make_counter() returns Counter:\n    return comptime spawn Counter()\n";
    let source = format!(
        "{prefix}{}",
        SOURCE.strip_prefix("namespace app\n").unwrap()
    );
    for release in [false, true] {
        let (mut program, checked) = high(&source, release);
        let values = required_values(&checked);
        crate::bake_required_values(
            &mut program,
            &values,
            &checked.checked().interner,
            &checked.checked().reflection_metadata,
            &checked.checked().method_value_definitions,
        )
        .unwrap();
        assert!(program.resource_source.graph_values.is_some());
        assert!(program.resource_source.reflected_values.is_some());
        let before = program.clone();
        let mut replay = before.clone();
        let receipt = interface_values::recorded_coercion_for_test(
            &mut replay,
            &before,
            &checked.checked().interner,
        )
        .unwrap();
        let view = conversion(&replay, &before, &checked.checked().interner, &receipt)
            .unwrap()
            .unwrap();
        assert_eq!(
            replay.resource_source, before.resource_source,
            "peer transitions need the unchanged input archive"
        );
        assert_eq!(
            view.required_only,
            before.resource_source.required_only_function_ids()
        );
        assert_eq!(
            view.values.len(),
            before.resource_source.required_materializations().len()
        );
        crate::complete_value_conversions(&mut program, &checked.checked().interner).unwrap();
        program
            .resource_source
            .validate_current(&program, &checked.checked().interner)
            .unwrap();
        assert_eq!(
            temporary_local_backings(&program, main(&program), &checked.checked().interner)
                .unwrap()
                .len(),
            1
        );
        let mut changed = before.clone();
        main_mut(&mut changed).body.statements.pop();
        assert!(
            crate::complete_value_conversions(&mut changed, &checked.checked().interner).is_err()
        );
    }
}

#[test]
fn genuine_appended_verify_entry_has_no_original_temporary_inventory() {
    let graph = "namespace app\nactor Counter:\n    mutable int64 count = 1\n    receive read responds int64:\n        respond count\nfunction make_counter() returns Counter:\n    return comptime spawn Counter()\n";
    let source = format!(
        "{graph}{}\nverify independent:\n    assert true\n",
        SOURCE.strip_prefix("namespace app\n").unwrap()
    );
    let file = FileId::new(0);
    let checked = Arc::new(
        jett_typecheck::CheckedResourceProgram::prepare(
            jett_parser::parse(&source, file),
            HashMap::from([(file, SourceOrigin::Project)]),
            &[],
            jett_typecheck::CheckOptions::default(),
        )
        .unwrap(),
    );
    let values = required_values(&checked);
    let mut program = crate::lower_checked_ordinary_program_with_test_bodies(&checked).unwrap();
    crate::bake_required_values(
        &mut program,
        &values,
        &checked.checked().interner,
        &checked.checked().reflection_metadata,
        &checked.checked().method_value_definitions,
    )
    .unwrap();
    assert!(program.resource_source.graph_values.is_some());
    let entry = crate::append_checked_native_verify_suite(&mut program, &checked, file)
        .unwrap()
        .unwrap();
    assert!(
        program
            .resource_source
            .functions()
            .iter()
            .all(|function| function.id != entry)
    );
    crate::complete_value_conversions(&mut program, &checked.checked().interner).unwrap();
    assert!(
        temporary_local_backings(
            &program,
            &program.functions[entry.index() as usize],
            &checked.checked().interner
        )
        .unwrap()
        .is_empty()
    );
    validate_backend_types(&program, &checked.checked().interner).unwrap();
}
