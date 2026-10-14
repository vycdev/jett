//! Host protocol negative controls. These tests are not Source admission evidence.
use super::*;

fn entry() -> NativeEntry {
    NativeEntry {
        function: 0,
        signature: 0,
        scope: 0,
    }
}
fn context(test: impl FnOnce(&AuthenticatedResourceContext)) {
    super::super::tests::context(|auth| {
        install_disabled(
            auth,
            &crate::resource_custody::carrier_runtime_fixture_bytes(),
            entry(),
        )
        .unwrap();
        test(auth);
    });
}
fn refused_cleanup_context(test: impl FnOnce(&AuthenticatedResourceContext)) {
    let mut context = Box::new(JettRuntimeContextV1::retired());
    let status = unsafe {
        jett_rt_v1_context_create(
            JETT_RUNTIME_ABI_VERSION_V1,
            &mut *context,
            &mut JettRuntimeResultV1::ok(),
        )
    };
    assert_eq!(status, JettRuntimeStatusV1::OK);
    let authenticated = AuthenticatedResourceContext::acquire(&*context).unwrap();
    install_disabled(
        &authenticated,
        &crate::resource_custody::carrier_runtime_fixture_bytes(),
        entry(),
    )
    .unwrap();
    test(&authenticated);
    drop(authenticated);
    let status =
        unsafe { jett_rt_v1_context_destroy(&mut *context, &mut JettRuntimeResultV1::ok()) };
    assert_eq!(status, JettRuntimeStatusV1::INVALID_ARGUMENT);
}
fn op(state: &NativeResourceState, predicate: impl Fn(&Op) -> bool) -> u32 {
    state
        .layout
        .operations()
        .iter()
        .enumerate()
        .find_map(|(i, row)| {
            let NativeOperation::Carrier { record } = row.operation() else {
                return None;
            };
            predicate(&state.layout.carriers().operations[*record as usize].operation)
                .then(|| u32::try_from(i).unwrap())
        })
        .expect("fixture carrier operation")
}
fn start(
    state: &mut NativeResourceState,
    ordinary: &mut values::NativeValues,
    registry: &ResourceRegistry,
) -> ResourceResult<(ResourceHandleId, ResourceHandleId)> {
    let attempt = state.begin_entry(ordinary, registry, entry(), ResourcePurpose::Runtime)?;
    let root = state.root_frame(attempt)?;
    let frame = state.begin_operation_frame(ordinary, 1, root)?;
    Ok((attempt, frame))
}
fn none(
    state: &mut NativeResourceState,
    ordinary: &mut values::NativeValues,
    frame: ResourceHandleId,
    slot: u32,
) -> ResourceResult<ResourceHandleId> {
    let operation = op(
        state,
        |r| matches!(r,Op::Construct{destination,selector:0,children,..} if *destination==slot&&children.is_empty()),
    );
    let builder = state.carrier_construct_begin(ordinary, frame, operation)?;
    state.carrier_commit(ordinary, builder)
}
fn record(
    state: &mut NativeResourceState,
    ordinary: &mut values::NativeValues,
    frame: ResourceHandleId,
    marker: u64,
) -> ResourceResult<ResourceHandleId> {
    let optional = none(state, ordinary, frame, 0)?;
    let operation = op(
        state,
        |r| matches!(r,Op::Construct{destination:2,children,..} if children.iter().any(|c|matches!(c.source,ChildSource::Move{slot:0}))),
    );
    let builder = state.carrier_construct_begin(ordinary, frame, operation)?;
    state.carrier_child(ordinary, builder, 0, marker)?;
    state.carrier_child(ordinary, builder, 1, optional.raw())?;
    state.carrier_commit(ordinary, builder)
}
fn finish(
    state: &mut NativeResourceState,
    ordinary: &mut values::NativeValues,
    registry: &mut ResourceRegistry,
    attempt: ResourceHandleId,
) -> ResourceResult<()> {
    let result = state.complete_entry(ordinary, registry, attempt, 0, false)?;
    assert_eq!(result.selected_kind, 0);
    assert_eq!(
        state.counts(ordinary, registry),
        NativeResourceCounts {
            ordinary_empty: 1,
            ..NativeResourceCounts::default()
        }
    );
    Ok(())
}

#[test]
fn staged_constructor_checks_lexical_prefix_before_consuming_children() {
    context(|auth| {
        auth.with_resource(|state,ordinary,registry|{
        let (attempt,frame)=start(state,ordinary,registry)?;let optional=none(state,ordinary,frame,0)?;
        let constructor=op(state,|r|matches!(r,Op::Construct{destination:2,children,..} if children.iter().any(|c|matches!(c.source,ChildSource::Move{slot:0}))));
        let builder=state.carrier_construct_begin(ordinary,frame,constructor)?;
        assert_eq!(state.carrier_child(ordinary,builder,1,optional.raw()),Err(NativeResourceError::WrongOperation));
        assert_eq!(state.carrier_commit(ordinary,builder),Err(NativeResourceError::WrongOperation));
        assert!(state.handles.contains_key(&optional));state.carrier_child(ordinary,builder,0,41)?;
        state.carrier_child(ordinary,builder,1,optional.raw())?;let root=state.carrier_commit(ordinary,builder)?;
        assert!(!state.handles.contains_key(&builder));assert!(!state.handles.contains_key(&optional));
        assert_eq!(state.carrier_commit(ordinary,builder),Err(NativeResourceError::WrongFamily));
        assert_eq!(state.carrier_root(ordinary,frame,root,2)?.tree.node,8);assert_eq!(registry.live_count(),0);
        finish(state,ordinary,registry,attempt)
    }).unwrap();
    });
}

#[test]
fn transfer_issues_a_fresh_generation_and_preserves_the_exact_nominal_stamp() {
    context(|auth| {
        auth.with_resource(|state,ordinary,registry|{
        let (attempt,frame)=start(state,ordinary,registry)?;let input=record(state,ordinary,frame,17)?;
        let transfer=op(state,|r|matches!(r,Op::Transfer{source:2,destination:9,..}));
        let output=state.carrier_transfer(ordinary,frame,transfer,input)?;assert_ne!(input,output);
        assert_eq!(state.carrier_root(ordinary,frame,input,2).err(),Some(NativeResourceError::WrongFamily));
        let observer=op(state,|r|matches!(r,Op::Observe{source:Source::Slot(9),path,observation:Observation::Ordinary{shape:0,clone:false},..} if path==&[Path::Field(0)]));
        assert_eq!(state.carrier_observe(ordinary,frame,observer,output,0)?,17);
        assert_eq!(state.carrier_root(ordinary,frame,output,3).err(),Some(NativeResourceError::WrongFrame));
        let Some(NativeResourceEntry::CarrierRoot(root))=state.handles.get_mut(&output) else{panic!("root")};root.tree.node=16;
        assert_eq!(state.carrier_root(ordinary,frame,output,9).err(),Some(NativeResourceError::WrongFrame));
        let Some(NativeResourceEntry::CarrierRoot(root))=state.handles.get_mut(&output) else{panic!("root")};root.tree.node=8;
        finish(state,ordinary,registry,attempt)
    }).unwrap();
    });
}

#[test]
fn carrier_scalar_task_transport_keeps_all_layers_across_typed_transfer_and_observation() {
    context(|auth| {
        auth.with_resource(|state, ordinary, registry| {
            let (attempt, frame) = start(state, ordinary, registry)?;
            let optional = none(state, ordinary, frame, 0)?;
            let constructor = op(state, |row| matches!(row, Op::Construct { destination: 2, children, .. } if children.iter().any(|child| matches!(child.source, ChildSource::Move { slot: 0 }))));
            let builder = state.carrier_construct_begin(ordinary, frame, constructor)?;
            state.carrier_child_scalar_task(ordinary, builder, 0, 17, 3)?;
            assert_eq!(state.carrier_child_scalar_task(ordinary, builder, 1, optional.raw(), 1), Err(NativeResourceError::WrongOperation));
            assert!(state.handles.contains_key(&optional), "a refused scalar endpoint must not adopt another family");
            state.carrier_child(ordinary, builder, 1, optional.raw())?;
            let input = state.carrier_commit(ordinary, builder)?;
            let transfer = op(state, |row| matches!(row, Op::Transfer { source: 2, destination: 9, .. }));
            let output = state.carrier_transfer(ordinary, frame, transfer, input)?;
            let observer = op(state, |row| matches!(row, Op::Observe { source: Source::Slot(9), path, observation: Observation::Ordinary { shape: 0, clone: false }, .. } if path == &[Path::Field(0)]));
            assert_eq!(state.carrier_observe_scalar_task(ordinary, frame, observer, output, 0)?, (17, 3));
            assert_eq!(state.carrier_observe(ordinary, frame, observer, output, 0), Err(NativeResourceError::WrongFamily), "the old one-word endpoint cannot discard layers");
            assert_eq!(state.carrier_observe_scalar_task(ordinary, frame, observer, output, 0)?, (17, 3));
            finish(state, ordinary, registry, attempt)
        }).unwrap();
    });
}

fn rejected_fake_leaf(
    state: &mut NativeResourceState,
    ordinary: &mut values::NativeValues,
    registry: &mut ResourceRegistry,
) -> ResourceResult<(ResourceHandleId, ResourceHandleId, ResourceHandleId)> {
    let (attempt, frame) = start(state, ordinary, registry)?;
    let mut ids = state.reserve(1)?;
    let fake = next_handle(&mut ids)?;
    state.handles.insert(
        fake,
        NativeResourceEntry::CarrierRoot(NativeCarrierRoot {
            frame,
            slot: 10,
            generation: fake.raw(),
            custody: None,
            tree: NativeCarrierTree {
                node: 3,
                selector: 0,
                ordinary: None,
                pending_depth: 0,
                owner: None,
                children: vec![],
                acquisition: vec![],
            },
        }),
    );
    let constructor = op(state, |r| {
        matches!(
            r,
            Op::Construct {
                destination: 0,
                selector: 1,
                ..
            }
        )
    });
    let builder = state.carrier_construct_begin(ordinary, frame, constructor)?;
    assert_eq!(
        state.carrier_child(ordinary, builder, 0, fake.raw()),
        Err(NativeResourceError::WrongFamily)
    );
    let Some(NativeResourceEntry::CarrierBuilder(builder)) = state.handles.get(&builder) else {
        panic!("builder")
    };
    assert_eq!(builder.next, 0);
    assert!(builder.acquisition.is_empty());
    assert!(state.handles.contains_key(&fake));
    assert_eq!(registry.live_count(), 0);
    Ok((attempt, frame, fake))
}

#[test]
fn selected_live_leaf_refuses_before_a_fake_handle_is_consumed() {
    refused_cleanup_context(|auth| {
        auth.with_resource(|state, ordinary, registry| {
            let (attempt, frame, fake) = rejected_fake_leaf(state, ordinary, registry)?;
            assert_eq!(
                finish(state, ordinary, registry, attempt),
                Err(NativeResourceError::WrongFamily)
            );
            assert!(state.handles.contains_key(&fake));
            assert_eq!(state.active_frames.last(), Some(&frame));
            assert_eq!(
                state.attempt.as_ref().unwrap().phase,
                AttemptPhase::Completing
            );
            assert!(ordinary.cleanup_failed);
            assert_eq!(registry.live_count(), 0);
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn restored_private_fake_leaf_allows_clean_builder_completion() {
    context(|auth| {
        auth.with_resource(|state, ordinary, registry| {
            let (attempt, frame, fake) = rejected_fake_leaf(state, ordinary, registry)?;
            assert!(matches!(
                state.carrier_root(ordinary, frame, fake, 10),
                Err(NativeResourceError::WrongFamily)
            ));
            assert!(matches!(
                state.handles.remove(&fake),
                Some(NativeResourceEntry::CarrierRoot(_))
            ));
            finish(state, ordinary, registry, attempt)
        })
        .unwrap();
    });
}

#[test]
fn partial_string_prefix_retires_once_after_a_later_stage_refuses() {
    context(|auth| {
        auth.with_resource(|state, ordinary, registry| {
            let (attempt, frame) = start(state, ordinary, registry)?;
            let mut ids = ordinary
                .prepare_resource_ordinary_output()
                .map_err(ordinary_error)?;
            let text = ordinary
                .publish_resource_error("kept prefix".to_owned(), &mut ids)
                .map_err(ordinary_error)?;
            let constructor = op(state, |r| {
                matches!(
                    r,
                    Op::Construct {
                        destination: 1,
                        selector: 0,
                        ..
                    }
                )
            });
            let builder = state.carrier_construct_begin(ordinary, frame, constructor)?;
            state.carrier_child(ordinary, builder, 0, text)?;
            assert_eq!(
                state.carrier_child(ordinary, builder, 1, 0),
                Err(NativeResourceError::WrongOperation)
            );
            assert!(!ordinary.is_empty());
            finish(state, ordinary, registry, attempt)?;
            assert!(
                ordinary
                    .drop_resource_typed_companion(&state.layout, 1, text)
                    .is_err()
            );
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn latent_generic_state_and_refinement_metadata_never_synthesizes_live_custody() {
    context(|auth| {
        auth.with_resource(|state,ordinary,registry|{
        let (attempt,_frame)=start(state,ordinary,registry)?;
        let scalar=||NativeCarrierTree{node:0,selector:0,ordinary:Some((0,17)),pending_depth:0,owner:None,children:vec![],acquisition:vec![]};
        let absent=||NativeCarrierTree{node:4,selector:0,ordinary:None,pending_depth:0,owner:None,children:vec![],acquisition:vec![]};
        let generic=NativeCarrierTree{node:13,selector:0,ordinary:None,pending_depth:0,owner:None,children:vec![Some(absent())],acquisition:vec![0]};
        state.validate_complete_tree(ordinary,&generic)?;
        assert!(matches!(state.layout.carriers().node(13)?,Node::Struct{arguments,..} if arguments==&[3]));
        assert!(matches!(state.layout.carriers().node(14)?,Node::Struct{arguments,..} if arguments==&[4]));
        let state_value=NativeCarrierTree{node:11,selector:0,ordinary:None,pending_depth:0,owner:None,children:vec![Some(scalar())],acquisition:vec![0]};state.validate_complete_tree(ordinary,&state_value)?;
        let wrong_state=NativeCarrierTree{selector:1,..state_value};assert!(state.validate_complete_tree(ordinary,&wrong_state).is_err());
        let refined=NativeCarrierTree{node:12,selector:0,ordinary:None,pending_depth:0,owner:None,children:vec![Some(scalar())],acquisition:vec![0]};
        assert_eq!(state.validate_complete_tree(ordinary,&refined),Err(NativeResourceError::UnprovedCarrierRefinement));
        assert_eq!(registry.live_count(),0);finish(state,ordinary,registry,attempt)
    }).unwrap();
    });
}

#[test]
fn extraction_tombstones_each_path_once_and_iteration_retains_original_length() {
    context(|auth| {
        auth.with_resource(|state, ordinary, registry| {
            let (attempt, frame) = start(state, ordinary, registry)?;
            let a = none(state, ordinary, frame, 0)?;
            let b = none(state, ordinary, frame, 11)?;
            let constructor = op(
                state,
                |r| matches!(r,Op::Construct{destination:6,children,..} if children.len()==2),
            );
            let builder = state.carrier_construct_begin(ordinary, frame, constructor)?;
            state.carrier_child(ordinary, builder, 0, a.raw())?;
            state.carrier_child(ordinary, builder, 1, b.raw())?;
            let list = state.carrier_commit(ordinary, builder)?;
            let length = op(state, |r| {
                matches!(
                    r,
                    Op::Observe {
                        source: Source::Slot(6),
                        observation: Observation::Length,
                        ..
                    }
                )
            });
            let extract = op(state, |r| {
                matches!(
                    r,
                    Op::Extract {
                        source: 6,
                        destination: Destination::Slot(12),
                        ..
                    }
                )
            });
            let reset = op(state, |r| matches!(r, Op::ResetIteration { source: 6, .. }));
            state.carrier_reset_iteration(ordinary, frame, reset, list)?;
            let first = state.carrier_extract(ordinary, frame, extract, list, 0)?;
            assert_eq!(state.carrier_observe(ordinary, frame, length, list, 0)?, 2);
            assert_eq!(
                state.carrier_extract(ordinary, frame, extract, list, 0),
                Err(NativeResourceError::WrongOperation)
            );
            state.carrier_reset_iteration(ordinary, frame, reset, list)?;
            assert!(!state.handles.contains_key(&ResourceHandleId::new(first)?));
            let second = state.carrier_extract(ordinary, frame, extract, list, 1)?;
            assert_ne!(first, second);
            state.carrier_reset_iteration(ordinary, frame, reset, list)?;
            assert_eq!(state.carrier_observe(ordinary, frame, length, list, 0)?, 2);
            assert_eq!(
                state.carrier_extract(ordinary, frame, extract, list, 1),
                Err(NativeResourceError::WrongOperation)
            );
            let Some(NativeResourceEntry::CarrierRoot(root)) = state.handles.get(&list) else {
                panic!("iterator")
            };
            assert_eq!(
                state.validate_complete_tree(ordinary, &root.tree),
                Err(NativeResourceError::WrongOperation)
            );
            finish(state, ordinary, registry, attempt)
        })
        .unwrap();
    });
}

#[test]
fn checked_machine_widening_preserves_state_and_enum_tag_uses_its_discriminant() {
    context(|auth| {
        auth.with_resource(|state, ordinary, registry| {
            let (attempt, frame) = start(state, ordinary, registry)?;
            let constructor = op(state, |r| {
                matches!(
                    r,
                    Op::Construct {
                        destination: 13,
                        ..
                    }
                )
            });
            let builder = state.carrier_construct_begin(ordinary, frame, constructor)?;
            state.carrier_child(ordinary, builder, 0, 17)?;
            let qualified = state.carrier_commit(ordinary, builder)?;
            let qualify = op(state, |r| {
                matches!(
                    r,
                    Op::QualifyMachine {
                        source: 13,
                        destination: 14,
                        machine: 10,
                        state: 0,
                        ..
                    }
                )
            });
            let general = state.carrier_qualify(ordinary, frame, qualify, qualified)?;
            assert_ne!(general, qualified);
            assert_eq!(
                state.carrier_root(ordinary, frame, qualified, 13).err(),
                Some(NativeResourceError::WrongFamily)
            );
            assert_eq!(
                state.carrier_root(ordinary, frame, general, 14)?.tree.node,
                10
            );
            let observe_state = op(state, |r| {
                matches!(
                    r,
                    Op::Observe {
                        source: Source::Slot(14),
                        observation: Observation::State,
                        ..
                    }
                )
            });
            assert_eq!(
                state.carrier_observe(ordinary, frame, observe_state, general, 0)?,
                0
            );
            assert_eq!(
                state.carrier_qualify(ordinary, frame, qualify, general),
                Err(NativeResourceError::WrongFrame)
            );
            let constructor = op(state, |r| {
                matches!(
                    r,
                    Op::Construct {
                        destination: 15,
                        ..
                    }
                )
            });
            let builder = state.carrier_construct_begin(ordinary, frame, constructor)?;
            state.carrier_child(ordinary, builder, 0, 17)?;
            let enumeration = state.carrier_commit(ordinary, builder)?;
            let tag = op(state, |r| {
                matches!(
                    r,
                    Op::Observe {
                        source: Source::Slot(15),
                        observation: Observation::Tag,
                        ..
                    }
                )
            });
            assert_eq!(
                state.carrier_observe(ordinary, frame, tag, enumeration, 0)?,
                (-7i64) as u64
            );
            finish(state, ordinary, registry, attempt)
        })
        .unwrap();
    });
}

#[test]
fn forged_scalar_clone_flag_refuses_without_changing_the_parent_or_obligation() {
    context(|auth| {
        auth.with_resource(|state, ordinary, registry| {
            let (attempt, frame) = start(state, ordinary, registry)?;
            let parent = record(state, ordinary, frame, 17)?;
            let before = state.counts(ordinary, registry);
            assert_eq!(
                state.carrier_observe_ordinary(ordinary, 0, true, 17),
                Err(NativeResourceError::WrongOperation)
            );
            assert_eq!(state.counts(ordinary, registry), before);
            let marker = state
                .carrier_root(ordinary, frame, parent, 2)?
                .tree
                .children[0]
                .as_ref()
                .unwrap()
                .ordinary
                .unwrap();
            assert_eq!(marker, (0, 17));
            assert_eq!(state.carrier_observe_ordinary(ordinary, 0, false, 17)?, 17);
            assert_eq!(state.carrier_observe_ordinary(ordinary, 3, false, 1)?, 1);
            assert_eq!(state.carrier_observe_ordinary(ordinary, 2, false, 0)?, 0);
            finish(state, ordinary, registry, attempt)
        })
        .unwrap();
    });
}

#[test]
fn forged_string_alias_refuses_and_a_real_clone_retires_independently() {
    context(|auth| {
        auth.with_resource(|state, ordinary, registry| {
            let (attempt, frame) = start(state, ordinary, registry)?;
            let mut ids = ordinary
                .prepare_resource_ordinary_output()
                .map_err(ordinary_error)?;
            let text = ordinary
                .publish_resource_error("parent-owned text".to_owned(), &mut ids)
                .map_err(ordinary_error)?;
            let constructor = op(state, |row| {
                matches!(
                    row,
                    Op::Construct {
                        destination: 1,
                        selector: 0,
                        ..
                    }
                )
            });
            let builder = state.carrier_construct_begin(ordinary, frame, constructor)?;
            state.carrier_child(ordinary, builder, 0, text)?;
            let parent = state.carrier_commit(ordinary, builder)?;
            let before = state.counts(ordinary, registry);
            assert_eq!(
                state.carrier_observe_ordinary(ordinary, 1, false, text),
                Err(NativeResourceError::WrongOperation)
            );
            assert_eq!(state.counts(ordinary, registry), before);
            let cloned = state.carrier_observe_ordinary(ordinary, 1, true, text)?;
            ordinary
                .drop_resource_typed_companion(&state.layout, 1, cloned)
                .map_err(ordinary_error)?;
            ordinary
                .validate_resource_ordinary(text, &state.layout, 1)
                .map_err(ordinary_error)?;
            assert!(state.carrier_root(ordinary, frame, parent, 1).is_ok());
            finish(state, ordinary, registry, attempt)?;
            assert!(
                ordinary
                    .drop_resource_typed_companion(&state.layout, 1, text)
                    .is_err()
            );
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn unsupported_owning_observation_flags_refuse_before_inspecting_a_fake_handle() {
    context(|auth| {
        auth.with_resource(|state, ordinary, registry| {
            let (attempt, _) = start(state, ordinary, registry)?;
            let before = state.counts(ordinary, registry);
            for shape in [5, 6] {
                for clone in [false, true] {
                    assert_eq!(
                        state.carrier_observe_ordinary(ordinary, shape, clone, u64::MAX),
                        Err(NativeResourceError::UnsupportedCarrierObservation)
                    );
                }
            }
            assert_eq!(state.counts(ordinary, registry), before);
            assert!(ordinary.is_empty());
            finish(state, ordinary, registry, attempt)
        })
        .unwrap();
    });
}

unsafe extern "C" {
    fn jett_rt_v1_resource_carrier_child_scalar_task(
        context: *const JettRuntimeContextV1,
        builder: u64,
        ordinal: u32,
        bits: u64,
        pending_depth: u64,
    ) -> JettRuntimeStatusV1;
    fn jett_rt_v1_resource_carrier_observe_scalar_task(
        context: *const JettRuntimeContextV1,
        frame: u64,
        operation: u32,
        input: u64,
        selection: u64,
        out_bits: *mut u64,
        out_depth: *mut u64,
    ) -> JettRuntimeStatusV1;
}

#[test]
fn carrier_scalar_task_c_outputs_are_atomic_on_refusal_and_preserve_all_layers() {
    // Refused operations record their first error, so each case owns a fresh attempt.
    for case in 0..12 {
        context(|auth| scalar_task_c_output_case(auth, case));
    }
}

fn scalar_task_c_output_case(auth: &AuthenticatedResourceContext, case: u32) {
    let pointer = auth.key.owner_address as *const JettRuntimeContextV1;
    let (attempt, frame, builder, constructor, optional) = auth
        .with_resource(|state, ordinary, registry| {
            let (attempt, frame) = start(state, ordinary, registry)?;
            let optional = none(state, ordinary, frame, 0)?;
            let constructor = op(state, |row| {
                matches!(row, Op::Construct { destination: 2, children, .. }
                    if children.iter().any(|child| matches!(child.source, ChildSource::Move { slot: 0 })))
            });
            let builder = state.carrier_construct_begin(ordinary, frame, constructor)?;
            Ok((attempt, frame, builder, constructor, optional))
        })
        .unwrap();
    assert_eq!(
        unsafe { jett_rt_v1_resource_carrier_child_scalar_task(pointer, builder.raw(), 0, 17, 3) },
        JettRuntimeStatusV1::OK
    );
    let (output, observer) = auth
        .with_resource(|state, ordinary, _| {
            state.carrier_child(ordinary, builder, 1, optional.raw())?;
            let input = state.carrier_commit(ordinary, builder)?;
            let transfer = op(state, |row| {
                matches!(
                    row,
                    Op::Transfer {
                        source: 2,
                        destination: 9,
                        ..
                    }
                )
            });
            let output = state.carrier_transfer(ordinary, frame, transfer, input)?;
            let observer = op(state, |row| {
                matches!(row, Op::Observe { source: Source::Slot(9), path,
                    observation: Observation::Ordinary { shape: 0, clone: false }, .. }
                    if path == &[Path::Field(0)])
            });
            Ok((output, observer))
        })
        .unwrap();
    let retired = JettRuntimeContextV1::retired();
    let mut words = [0x0123_4567_89ab_cdefu64, 0xfedc_ba98_7654_3210];
    let sentinel = words;
    let bits = std::ptr::addr_of_mut!(words[0]);
    let depth = std::ptr::addr_of_mut!(words[1]);
    let (ctx, call_frame, operation, input, out_bits, out_depth) = match case {
        0 => (
            std::ptr::null(),
            frame.raw(),
            observer,
            output.raw(),
            bits,
            depth,
        ),
        1 => (
            &retired as *const _,
            frame.raw(),
            observer,
            output.raw(),
            bits,
            depth,
        ),
        2 => (pointer, frame.raw(), observer, 0, bits, depth),
        3 => (pointer, frame.raw(), observer, u64::MAX, bits, depth),
        4 => (pointer, 0, observer, output.raw(), bits, depth),
        5 => (pointer, u64::MAX, observer, output.raw(), bits, depth),
        6 => (pointer, frame.raw(), constructor, output.raw(), bits, depth),
        7 => (
            pointer,
            frame.raw(),
            observer,
            output.raw(),
            std::ptr::null_mut(),
            depth,
        ),
        8 => (
            pointer,
            frame.raw(),
            observer,
            output.raw(),
            bits,
            std::ptr::null_mut(),
        ),
        9 => (pointer, frame.raw(), observer, output.raw(), bits, bits),
        10 => (pointer, frame.raw(), observer, output.raw(), depth, depth),
        _ => (pointer, frame.raw(), observer, output.raw(), bits, depth),
    };
    let status = unsafe {
        jett_rt_v1_resource_carrier_observe_scalar_task(
            ctx, call_frame, operation, input, 0, out_bits, out_depth,
        )
    };
    if case == 11 {
        assert_eq!(status, JettRuntimeStatusV1::OK);
        assert_eq!(words, [17, 3]);
    } else {
        let expected = if case <= 1 {
            JettRuntimeStatusV1::INVALID_CONTEXT
        } else {
            JettRuntimeStatusV1::INVALID_ARGUMENT
        };
        assert_eq!(status, expected, "case {case}");
        assert_eq!(
            words, sentinel,
            "both outputs remain unchanged in case {case}"
        );
    }
    let recorded_failure = (2..=6).contains(&case);
    if !recorded_failure {
        assert_eq!(
            unsafe {
                jett_rt_v1_resource_carrier_observe_scalar_task(
                    pointer,
                    frame.raw(),
                    observer,
                    output.raw(),
                    0,
                    bits,
                    depth,
                )
            },
            JettRuntimeStatusV1::OK
        );
        assert_eq!(words, [17, 3]);
    }
    auth.with_resource(|state, ordinary, registry| {
        assert!(
            state.handles.contains_key(&output),
            "refusal never consumes the carrier"
        );
        let completion = state.complete_entry(ordinary, registry, attempt, 0, false)?;
        assert_eq!(completion.selected_kind, u32::from(recorded_failure));
        assert_eq!(completion.cleanup_status, 0);
        assert_eq!(
            state.counts(ordinary, registry),
            NativeResourceCounts {
                ordinary_empty: 1,
                ..NativeResourceCounts::default()
            }
        );
        Ok(())
    })
    .unwrap();
}
