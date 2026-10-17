use super::*;

// Exact original Source19/20 bytes; no Driver fixture path dependency.
const NAMED: &str = "namespace app\ntype Positive = int64 where value > 0\nstruct CallbackHolder:\n    callback: function() returns int64\nfunction five() returns int64:\n    return 5\nfunction main() returns nothing:\n    CallbackHolder holder = CallbackHolder(callback: five)\n    for field in type.fields[CallbackHolder]():\n        function() returns Positive observed = type.field_value[CallbackHolder, function() returns Positive](view holder, view field)\n        Positive value = observed()\n        println(coarsen value)\n    return nothing\n\nfunction ready_five() returns Positive:\n    return 5\nverify lazy_callback_result:\n    Positive baked = comptime ready_five()\n    CallbackHolder holder = CallbackHolder(callback: five)\n    for field in type.fields[CallbackHolder]():\n        function() returns Positive observed = type.field_value[CallbackHolder, function() returns Positive](view holder, view field)\n        assert coarsen observed() == coarsen baked\nproperty lazy_callback_trials:\n    given repeat: bool\n    Positive baked = comptime ready_five()\n    CallbackHolder holder = CallbackHolder(callback: five)\n    for field in type.fields[CallbackHolder]():\n        function() returns Positive observed = type.field_value[CallbackHolder, function() returns Positive](view holder, view field)\n        assert coarsen observed() == coarsen baked\n        if repeat:\n            assert coarsen observed() == 5\n";
const CAPTURED: &str = "namespace app\ntype Positive = int64 where value > 0\nstruct CallbackHolder:\n    callback: function() returns int64\nfunction five() returns int64:\n    return 5\nfunction main() returns nothing:\n    CallbackHolder holder = CallbackHolder(callback: five)\n    for field in type.fields[CallbackHolder]():\n        function() returns Positive observed = type.field_value[CallbackHolder, function() returns Positive](view holder, view field)\n        Positive value = observed()\n        println(coarsen value)\n    return nothing\n\nverify captured_callback_result:\n    int64 seed = 7\n    CallbackHolder holder = CallbackHolder(callback: function() returns int64: return seed)\n    for field in type.fields[CallbackHolder]():\n        function() returns Positive observed = type.field_value[CallbackHolder, function() returns Positive](view holder, view field)\n        assert coarsen observed() == 7\nproperty captured_callback_trials:\n    given repeat: bool\n    int64 seed = 7\n    CallbackHolder holder = CallbackHolder(callback: function() returns int64: return seed)\n    for field in type.fields[CallbackHolder]():\n        function() returns Positive observed = type.field_value[CallbackHolder, function() returns Positive](view holder, view field)\n        assert coarsen observed() == 7\n        if repeat:\n            assert coarsen observed() == 7\n";

fn checked(source: &str) -> Arc<jett_typecheck::CheckedResourceProgram> {
    let file = FileId::new(0);
    Arc::new(
        jett_typecheck::CheckedResourceProgram::prepare(
            jett_parser::parse(source, file),
            HashMap::from([(file, SourceOrigin::Project)]),
            &[],
            jett_typecheck::CheckOptions::default(),
        )
        .unwrap(),
    )
}

fn fixture(source: &str) -> (Program, Arc<jett_typecheck::CheckedResourceProgram>) {
    let checked = checked(source);
    let program = lower_checked_ordinary_program_with_test_bodies(&checked)
        .expect("original checked suite holders lower");
    validate(&program).unwrap();
    validate_backend_types(&program, &checked.checked().interner).unwrap();
    (program, checked)
}

fn rejects_current_graph(program: &Program, checked: &jett_typecheck::CheckedResourceProgram) {
    // This is the complete mandatory input gate used by MIR::lower, including
    // the immutable reflected source graph before structural type validation.
    let errors = validate(program)
        .and_then(|()| validate_backend_types(program, &checked.checked().interner))
        .expect_err("changed current descriptor graph must not acquire snapshot authority");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("reflected callable")
                || error.message.contains("call ownership")),
        "{errors:?}"
    );
}

fn reads(function: &Function) -> Vec<Expression> {
    let mut body = function.body.clone();
    let mut values = Vec::new();
    crate::resource_materialization::walk_block(&mut body, &mut |value| {
        if matches!(
            value.kind,
            ExpressionKind::Intrinsic {
                intrinsic: IntrinsicId::TypeFieldValue,
                ..
            }
        ) {
            values.push(value.clone());
        }
        Ok(())
    })
    .unwrap();
    values
}

#[test]
fn checked_named_and_captured_suite_holders_have_exact_storage_proofs() {
    for (source, captured) in [(NAMED, false), (CAPTURED, true)] {
        let (program, checked) = fixture(source);
        assert!(program.resource_source.belongs_to(&checked));
        for kind in [DeclarationKind::Verify, DeclarationKind::Property] {
            let owner = program
                .functions
                .iter()
                .find(|function| function.identity.declaration.kind == kind)
                .unwrap();
            let values = reads(owner);
            assert_eq!(values.len(), 1);
            let ExpressionKind::Intrinsic {
                ownership: CallOwnership::Source(call),
                ..
            } = &values[0].kind
            else {
                panic!("original reflected call")
            };
            let observation = call.arguments[0].observation_proof().unwrap();
            let proof = observation
                .purpose_proof()
                .expect("constructor-owned purpose proof");
            proof.validate_read(&values[0]).unwrap();
            assert!(!crate::call_ownership::observation_data_type(
                &checked.checked().interner,
                call.arguments[0].actual_type
            ));
            assert_eq!(observation.captures().is_empty(), !captured);
            assert_eq!(proof.root.mode, CallerBindingMode::Owned);
            assert!(!proof.root.mutable);
            assert_eq!(proof.bindings.len(), 1);
            let mut metadata = Vec::new();
            CallOwnership::Source(call.clone()).metadata_local_ids(|id| metadata.push(id));
            assert!(metadata.contains(&proof.root.local));
            for &(capture, _) in observation.captures() {
                assert!(metadata.contains(&capture));
            }
        }
    }
}

#[test]
fn checked_holder_observation_refuses_same_signature_descriptor_and_body_substitution() {
    let source = format!("{NAMED}\nfunction zero() returns int64:\n    return 0\n");
    let (program, checked) = fixture(&source);
    let zero = program
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "zero")
        .unwrap()
        .id;
    let verify = program
        .functions
        .iter()
        .position(|function| function.identity.declaration.kind == DeclarationKind::Verify)
        .unwrap();
    let mut substituted = program.clone();
    let StatementKind::Let {
        value:
            Expression {
                kind: ExpressionKind::StructConstruct { fields, .. },
                ..
            },
        ..
    } = &mut substituted.functions[verify].body.statements[1].kind
    else {
        panic!("original holder initializer")
    };
    assert!(matches!(fields[0].kind, ExpressionKind::FunctionRef(_)));
    fields[0].kind = ExpressionKind::FunctionRef(zero);
    rejects_current_graph(&substituted, &checked);
    let mut body = program.clone();
    let five = body
        .functions
        .iter_mut()
        .find(|function| function.identity.declaration.name == "five")
        .unwrap();
    let StatementKind::Return(Some(value)) = &mut five.body.statements[0].kind else {
        panic!("source callback return")
    };
    value.kind = ExpressionKind::Int(0);
    rejects_current_graph(&body, &checked);
}

#[test]
fn checked_captured_holder_refuses_capture_producer_and_owner_changes() {
    let (program, checked) = fixture(CAPTURED);
    let index = program
        .functions
        .iter()
        .position(|function| function.identity.declaration.kind == DeclarationKind::Verify)
        .unwrap();
    for mutation in 0..4 {
        let mut changed = program.clone();
        let owner = &mut changed.functions[index];
        match mutation {
            0 => {
                let StatementKind::Let { value, .. } = &mut owner.body.statements[0].kind else {
                    panic!("original capture producer")
                };
                value.kind = ExpressionKind::Int(8);
            }
            1 => owner.identity.declaration.kind = DeclarationKind::Function,
            2 => {
                let holder = owner
                    .locals
                    .iter_mut()
                    .find(|local| local.name == "holder")
                    .unwrap();
                holder.mutable = true;
            }
            3 => {
                let seed = owner
                    .locals
                    .iter()
                    .find(|local| local.name == "seed")
                    .unwrap()
                    .id;
                let holder = owner
                    .locals
                    .iter_mut()
                    .find(|local| local.name == "holder")
                    .unwrap();
                holder.view_source = Some(seed);
            }
            _ => unreachable!(),
        }
        rejects_current_graph(&changed, &checked);
    }
}

#[test]
fn purpose_observation_requires_same_checked_arc_and_sealed_callable_read() {
    let (program, _) = fixture(NAMED);
    let owner = program
        .functions
        .iter()
        .find(|function| function.identity.declaration.kind == DeclarationKind::Verify)
        .unwrap();
    let value = reads(owner).remove(0);
    let ExpressionKind::Intrinsic {
        ownership: CallOwnership::Source(call),
        ..
    } = &value.kind
    else {
        panic!("checked read")
    };
    let proof = call.arguments[0]
        .observation_proof()
        .unwrap()
        .purpose_proof()
        .unwrap();
    let mut foreign = proof.clone();
    foreign.checked = checked(NAMED);
    assert!(foreign.validate_read(&value).is_err());
    let mut missing = value.clone();
    let ExpressionKind::Intrinsic {
        field_validation, ..
    } = &mut missing.kind
    else {
        unreachable!()
    };
    *field_validation = Some(ReflectedFieldValidation::Read);
    assert!(proof.validate_read(&missing).is_err());
    let property = program
        .functions
        .iter()
        .find(|function| function.identity.declaration.kind == DeclarationKind::Property)
        .unwrap();
    let property_read = reads(property).remove(0);
    let ExpressionKind::Intrinsic {
        field_validation: Some(validation),
        ..
    } = property_read.kind
    else {
        panic!("another genuine checked plan")
    };
    let mut transplanted = value.clone();
    let ExpressionKind::Intrinsic {
        field_validation, ..
    } = &mut transplanted.kind
    else {
        unreachable!()
    };
    *field_validation = Some(validation);
    assert!(proof.validate_read(&transplanted).is_err());
    let mut moved = value.clone();
    moved.span.start += 1;
    assert!(proof.validate_read(&moved).is_err());
}

#[test]
fn checked_immutable_holder_alias_retains_both_original_constructor_headers() {
    let source = NAMED.replace(
        "    CallbackHolder holder = CallbackHolder(callback: five)\n",
        "    CallbackHolder holder = CallbackHolder(callback: five)\n    CallbackHolder alias = holder\n",
    ).replace("(view holder, view field)", "(view alias, view field)");
    let (program, _) = fixture(&source);
    for kind in [DeclarationKind::Verify, DeclarationKind::Property] {
        let owner = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.kind == kind)
            .unwrap();
        let value = reads(owner).remove(0);
        let ExpressionKind::Intrinsic {
            ownership: CallOwnership::Source(call),
            ..
        } = &value.kind
        else {
            panic!("checked alias read")
        };
        let proof = call.arguments[0]
            .observation_proof()
            .unwrap()
            .purpose_proof()
            .unwrap();
        assert_eq!(proof.bindings.len(), 2);
        assert_eq!(
            proof.root.local,
            owner
                .locals
                .iter()
                .find(|local| local.name == "alias")
                .unwrap()
                .id
        );
        assert_eq!(
            proof.bindings[1].current.local,
            owner
                .locals
                .iter()
                .find(|local| local.name == "holder")
                .unwrap()
                .id
        );
        proof.validate_read(&value).unwrap();
    }
}

#[test]
fn purpose_observation_metadata_remap_preserves_original_definition_identity() {
    let (program, _) = fixture(CAPTURED);
    let owner = program
        .functions
        .iter()
        .find(|function| function.identity.declaration.kind == DeclarationKind::Property)
        .unwrap();
    let value = reads(owner).remove(0);
    let ExpressionKind::Intrinsic {
        ownership: CallOwnership::Source(call),
        ..
    } = &value.kind
    else {
        panic!("checked read")
    };
    let original = call.arguments[0]
        .observation_proof()
        .unwrap()
        .purpose_proof()
        .unwrap();
    let mut remapped = original.clone();
    remapped
        .remap_locals(&mut |id| Ok::<_, ()>(LocalId(id.index() + 100)))
        .unwrap();
    assert_eq!(
        remapped.root.local.index(),
        original.root.local.index() + 100
    );
    assert_eq!(remapped.bindings[0].original, original.bindings[0].original);
    assert_eq!(remapped.bindings[0].current.local, remapped.root.local);
    let mut metadata = Vec::new();
    remapped.metadata_locals(|id| metadata.push(id));
    assert!(metadata.iter().all(|id| id.index() >= 100));
}
