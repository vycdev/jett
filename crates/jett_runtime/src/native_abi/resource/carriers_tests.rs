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
    ordinary: &values::NativeValues,
    registry: &ResourceRegistry,
) -> ResourceResult<(ResourceHandleId, ResourceHandleId)> {
    let attempt = state.begin_entry(ordinary, registry, entry(), ResourcePurpose::Runtime)?;
    let root = state.root_frame(attempt)?;
    let frame = state.begin_operation_frame(ordinary, 1, root)?;
    Ok((attempt, frame))
}
fn none(
    state: &mut NativeResourceState,
    ordinary: &values::NativeValues,
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
    ordinary: &values::NativeValues,
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
fn selected_live_leaf_refuses_before_a_fake_handle_is_consumed() {
    context(|auth| {
        auth.with_resource(|state, ordinary, registry| {
            let (attempt, frame) = start(state, ordinary, registry)?;
            let mut ids = state.reserve(1)?;
            let fake = next_handle(&mut ids)?;
            state.handles.insert(
                fake,
                NativeResourceEntry::CarrierRoot(NativeCarrierRoot {
                    frame,
                    slot: 10,
                    generation: fake.raw(),
                    tree: NativeCarrierTree {
                        node: 3,
                        selector: 0,
                        ordinary: None,
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
                Err(NativeResourceError::UnsupportedCarrierCustody)
            );
            let Some(NativeResourceEntry::CarrierBuilder(builder)) = state.handles.get(&builder)
            else {
                panic!("builder")
            };
            assert_eq!(builder.next, 0);
            assert!(builder.acquisition.is_empty());
            assert!(state.handles.contains_key(&fake));
            assert_eq!(registry.live_count(), 0);
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
        let scalar=||NativeCarrierTree{node:0,selector:0,ordinary:Some((0,17)),children:vec![],acquisition:vec![]};
        let absent=||NativeCarrierTree{node:4,selector:0,ordinary:None,children:vec![],acquisition:vec![]};
        let generic=NativeCarrierTree{node:13,selector:0,ordinary:None,children:vec![Some(absent())],acquisition:vec![0]};
        state.validate_complete_tree(ordinary,&generic)?;
        assert!(matches!(state.layout.carriers().node(13)?,Node::Struct{arguments,..} if arguments==&[3]));
        assert!(matches!(state.layout.carriers().node(14)?,Node::Struct{arguments,..} if arguments==&[4]));
        let state_value=NativeCarrierTree{node:11,selector:0,ordinary:None,children:vec![Some(scalar())],acquisition:vec![0]};state.validate_complete_tree(ordinary,&state_value)?;
        let wrong_state=NativeCarrierTree{selector:1,..state_value};assert!(state.validate_complete_tree(ordinary,&wrong_state).is_err());
        let refined=NativeCarrierTree{node:12,selector:0,ordinary:None,children:vec![Some(scalar())],acquisition:vec![0]};
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
