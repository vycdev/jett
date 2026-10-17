//! Host protocol controls; Source admission and native acceptance are separate gates.
use super::super::*;
use super::{assert_empty, context, events, script, words};

const REPLACE: u32 = 7;
const DROP_OLD: u32 = 8;
const DROP_RHS: u32 = 9;
const BORROW_OLD: u32 = 12;
const BORROW_RHS: u32 = 13;
const END_OLD: u32 = 14;
const END_RHS: u32 = 15;
const SENTINEL: u64 = 0x5555_6666_7777_8888;

#[derive(Clone, Copy, Debug)]
enum Failure {
    String,
    Integer,
    OptionalString,
}
#[derive(Clone, Copy, Debug)]
enum Family {
    Optional,
    Result(Failure),
}
impl Family {
    fn shape(self) -> u32 {
        match self {
            Self::Optional => 5,
            Self::Result(Failure::String) => 6,
            Self::Result(Failure::Integer) => 9,
            Self::Result(Failure::OptionalString) => 10,
        }
    }
    fn failure_shape(self) -> Option<u32> {
        match self {
            Self::Optional => None,
            Self::Result(Failure::String) => Some(2),
            Self::Result(Failure::Integer) => Some(1),
            Self::Result(Failure::OptionalString) => Some(8),
        }
    }
    fn path(self) -> u32 {
        match self {
            Self::Optional => 1,
            Self::Result(_) => 2,
        }
    }
}
fn entry() -> NativeEntry {
    NativeEntry {
        function: 0,
        signature: 3,
        scope: 0,
    }
}
fn fixture(family: Family) -> Vec<u8> {
    // Independent canonical wire, authenticated by the real registration issuer.
    // Both replacement slots belong to the exact same Scope, not setup Operation.
    let signatures = [
        vec![0, 2, 6, 0, 2, 1, 1],
        vec![1, 2, 7, 0, 2, 4, 2],
        vec![2, 1, 3, 4, 1],
        vec![3, 1, 3, 0, 2],
    ];
    let shapes = [
        vec![0, 6],
        vec![1, 1, 64, 1],
        vec![2, 4],
        vec![3, 5],
        vec![4, 7, 0],
        vec![5, 8, 4],
        vec![6, 9, 4, 2],
        vec![7, 9, 1, 2],
        vec![8, 8, 2],
        vec![9, 9, 4, 1],
        vec![10, 9, 4, 8],
    ];
    let frames = [
        vec![0, 1, 0, 0, 0, 0, 3, 1, 0, 0],
        vec![1, 2, 0, 0, 0, 1, 3, 1, 1, 0],
    ];
    let slots = [
        vec![0, 0, family.shape(), 1, family.path()],
        vec![1, 0, family.shape(), 1, family.path()],
        vec![2, 1, 6, 1, 2],
        vec![3, 1, 4, 0],
        vec![4, 1, family.shape(), 1, family.path()],
    ];
    let operations = [
        vec![1, 1, 0, 2],  // 0 factory into Operation-local result
        vec![10, 1, 2, 3], // 1 take the one occupied factory token
        vec![9, 1, 3, 0],  // 2 adopt into old Scope slot
        vec![9, 1, 3, 1],  // 3 adopt into RHS Scope slot
        match family.failure_shape() {
            None => vec![19, 1, 4],
            Some(shape) => vec![20, 1, 4, shape],
        }, // 4 create absent Operation-local shell
        vec![2, 1, 4, 0],  // 5 move absence into old Scope slot
        vec![2, 1, 4, 1],  // 6 move absence into RHS Scope slot
        vec![12, 0, 0, 1], // 7 one compound Scope replacement
        vec![11, 0, 0],
        vec![11, 0, 1], // 8/9 drop exact sum
        vec![13, 1],
        vec![13, 0], // 10/11 complete Operation/Scope
        vec![21, 0, 0, 0],
        vec![21, 0, 1, 0], // 12/13 shell leases
        vec![25, 0, 12],
        vec![25, 0, 13], // 14/15 end exact leases
    ];
    let mut bytes = b"JTRSC001".to_vec();
    words(&mut bytes, &[2, 0]);
    bytes.extend_from_slice(&0u64.to_le_bytes());
    words(
        &mut bytes,
        &[
            1,
            3,
            signatures.len() as u32,
            shapes.len() as u32,
            frames.len() as u32,
            slots.len() as u32,
            operations.len() as u32,
            0,
        ],
    );
    words(&mut bytes, &[0]);
    for row in [[0, 0, 1, 0], [1, 0, 2, 1], [2, 0, 3, 2]] {
        words(&mut bytes, &row);
    }
    for row in signatures
        .into_iter()
        .chain(shapes)
        .chain(frames)
        .chain(slots)
    {
        words(&mut bytes, &row);
    }
    for (index, row) in operations.into_iter().enumerate() {
        words(&mut bytes, &[index as u32, 0, 0, 0, 100 + index as u32]);
        words(&mut bytes, &row);
    }
    let length = bytes.len() as u64;
    bytes[16..24].copy_from_slice(&length.to_le_bytes());
    bytes
}
fn pointer(auth: &AuthenticatedResourceContext) -> *const JettRuntimeContextV1 {
    auth.key.owner_address as *const JettRuntimeContextV1
}
fn text(auth: &AuthenticatedResourceContext, value: &str) -> u64 {
    auth.with_resource(|_, ordinary, _| {
        let mut ids = ordinary
            .prepare_resource_ordinary_output()
            .map_err(ordinary_error)?;
        ordinary
            .publish_resource_error(value.to_owned(), &mut ids)
            .map_err(ordinary_error)
    })
    .unwrap()
}
fn companion(auth: &AuthenticatedResourceContext, family: Family, label: i64) -> u64 {
    match family {
        Family::Optional => 0,
        Family::Result(Failure::Integer) => u64::from_ne_bytes(label.to_ne_bytes()),
        Family::Result(Failure::String) => text(auth, &format!("failure-{label}")),
        Family::Result(Failure::OptionalString) => {
            let payload = text(auth, &format!("nested-failure-{label}"));
            let value = unsafe { values::jett_rt_v1_sum_new(pointer(auth), 1, payload, 1) };
            assert_ne!(value, 0);
            value
        }
    }
}
fn start(
    auth: &AuthenticatedResourceContext,
    purpose: ResourcePurpose,
) -> (ResourceHandleId, ResourceHandleId, ResourceHandleId, u64) {
    auth.with_resource(|state, ordinary, registry| {
        let attempt = state.begin_entry(ordinary, registry, entry(), purpose)?;
        let root = state.root_frame(attempt)?;
        let operation = state.begin_operation_frame(ordinary, 1, root)?;
        let network = if purpose == ResourcePurpose::Runtime {
            state.entry_network(attempt, 0)?
        } else {
            0
        };
        Ok((attempt, root, operation, network))
    })
    .unwrap()
}
fn make(
    auth: &AuthenticatedResourceContext,
    family: Family,
    root: ResourceHandleId,
    operation: ResourceHandleId,
    network: u64,
    slot: u32,
    occupied: bool,
    label: i64,
    supplied_companion: Option<u64>,
) -> ResourceHandleId {
    let failure = if occupied {
        0
    } else {
        supplied_companion.unwrap_or_else(|| companion(auth, family, label))
    };
    auth.with_resource(|state, ordinary, registry| {
        if occupied {
            let call = state.prepare_hook(ordinary, 0, operation, None)?;
            let prepared = state.prepare_factory(ordinary, registry, call, network)?;
            let produced = state.factory(ordinary, registry, prepared, network, label)?;
            assert_eq!(produced.domain, RESOURCE_DOMAIN_OK);
            let owner = state.sum_take(
                ordinary,
                registry,
                1,
                operation,
                ResourceHandleId::new(produced.value)?,
                operation,
            )?;
            state.sum_adopt(ordinary, registry, 2 + slot, operation, owner, root)
        } else {
            let sum = if family.failure_shape().is_some() {
                state.failure_sum(ordinary, operation, 4, failure)?
            } else {
                state.absent_sum(ordinary, operation, 4)?
            };
            state.transfer(ordinary, registry, 5 + slot, operation, sum, root)
        }
    })
    .unwrap()
}
fn finish_setup(auth: &AuthenticatedResourceContext, operation: ResourceHandleId) {
    auth.with_resource(|state, ordinary, registry| {
        state.end_operation_frame(ordinary, registry, operation)
    })
    .unwrap();
}
fn replace(
    auth: &AuthenticatedResourceContext,
    root: ResourceHandleId,
    old: ResourceHandleId,
    rhs: ResourceHandleId,
) -> (JettRuntimeStatusV1, u64) {
    let mut output = SENTINEL;
    let status = unsafe {
        leaves::jett_rt_v1_resource_replace(
            pointer(auth),
            root.raw(),
            REPLACE,
            old.raw(),
            rhs.raw(),
            &mut output,
        )
    };
    (status, output)
}
fn shell_rows(state: &NativeResourceState, registry: &ResourceRegistry) -> Vec<String> {
    let mut rows = state
        .handles
        .iter()
        .filter_map(|(id, entry)| {
            let NativeResourceEntry::Sum(sum) = entry else {
                return None;
            };
            let payload = match &sum.payload {
                NativeSumPayload::None => "none".to_owned(),
                NativeSumPayload::Fail { shape, string } => format!("fail:{shape}:{string}"),
                NativeSumPayload::Some(token) | NativeSumPayload::Ok(token) => {
                    let key = state.custody.validate_owned(token, registry).unwrap();
                    format!(
                        "{}:{key:?}:{token:?}",
                        if matches!(&sum.payload, NativeSumPayload::Some(_)) {
                            "some"
                        } else {
                            "ok"
                        }
                    )
                }
            };
            Some(format!(
                "{}:{}:{}:{}:{payload}",
                id.raw(),
                sum.frame.raw(),
                sum.slot,
                sum.shape
            ))
        })
        .collect::<Vec<_>>();
    rows.sort();
    rows
}
fn snapshot(
    auth: &AuthenticatedResourceContext,
) -> (NativeResourceCounts, Vec<String>, usize, Vec<(i64, u32)>) {
    let (counts, shells, remaining) = auth
        .with_resource(|state, ordinary, registry| {
            Ok((
                state.counts(ordinary, registry),
                shell_rows(state, registry),
                state.provider.remaining(),
            ))
        })
        .unwrap();
    (counts, shells, remaining, events(auth))
}
fn assert_shell_count(auth: &AuthenticatedResourceContext, expected: usize) {
    auth.with_resource(|state, _, registry| {
        assert_eq!(shell_rows(state, registry).len(), expected);
        Ok(())
    })
    .unwrap();
}
fn complete(
    auth: &AuthenticatedResourceContext,
    attempt: ResourceHandleId,
    selected: u32,
    cleanup: u32,
) {
    let outcome = auth
        .with_resource(|state, ordinary, registry| {
            state.complete_entry(ordinary, registry, attempt, 0, false)
        })
        .unwrap();
    assert_eq!(
        (
            outcome.body_status,
            outcome.cleanup_status,
            outcome.selected_kind
        ),
        (0, cleanup, selected)
    );
    assert_empty(auth);
    assert_shell_count(auth, 0);
    auth.with_resource(|state, _, _| {
        assert_eq!(state.provider.remaining(), 0);
        Ok(())
    })
    .unwrap();
}
fn key(auth: &AuthenticatedResourceContext, handle: ResourceHandleId) -> Option<String> {
    auth.with_resource(|state, _, registry| {
        let Some(NativeResourceEntry::Sum(sum)) = state.handles.get(&handle) else {
            panic!("the exact live sum shell");
        };
        Ok(match &sum.payload {
            NativeSumPayload::Some(token) | NativeSumPayload::Ok(token) => Some(format!(
                "{:?}",
                state.custody.validate_owned(token, registry)?
            )),
            NativeSumPayload::None | NativeSumPayload::Fail { .. } => None,
        })
    })
    .unwrap()
}
fn assert_published(
    auth: &AuthenticatedResourceContext,
    family: Family,
    root: ResourceHandleId,
    old: ResourceHandleId,
    rhs: ResourceHandleId,
    installed: ResourceHandleId,
    occupied: bool,
    rhs_key: Option<String>,
) {
    assert_ne!(installed, old);
    assert_ne!(installed, rhs);
    auth.with_resource(|state, _, _| {
        assert!(!state.handles.contains_key(&old));
        assert!(!state.handles.contains_key(&rhs));
        let Some(NativeResourceEntry::Sum(sum)) = state.handles.get(&installed) else {
            panic!("one fresh published sum shell");
        };
        assert_eq!((sum.frame, sum.slot, sum.shape), (root, 0, family.shape()));
        assert_eq!(state.sum_tag(installed)?, u32::from(occupied));
        Ok(())
    })
    .unwrap();
    assert_eq!(key(auth, installed), rhs_key);
    assert_shell_count(auth, 1);
}

#[test]
fn native_resource_sum_replace_eight_transitions_preserve_exact_rhs_identity_and_old_cleanup_order()
{
    for family in [Family::Optional, Family::Result(Failure::String)] {
        for old_occupied in [false, true] {
            for rhs_occupied in [false, true] {
                context(|auth| {
                    let mut rows = Vec::new();
                    if old_occupied {
                        rows.push((1, 1101, 0, ""));
                    }
                    if rhs_occupied {
                        rows.push((1, 1102, 0, ""));
                    }
                    install_scripted(auth, &fixture(family), entry(), script(&rows, 1)).unwrap();
                    let (attempt, root, operation, network) = start(auth, ResourcePurpose::Runtime);
                    let old = make(
                        auth,
                        family,
                        root,
                        operation,
                        network,
                        0,
                        old_occupied,
                        1101,
                        None,
                    );
                    let rhs = make(
                        auth,
                        family,
                        root,
                        operation,
                        network,
                        1,
                        rhs_occupied,
                        1102,
                        None,
                    );
                    finish_setup(auth, operation);
                    assert_shell_count(auth, 2);
                    let rhs_key = key(auth, rhs);
                    let rhs_generation = auth
                        .with_resource(|state, _, _| {
                            let Some(NativeResourceEntry::Sum(sum)) = state.handles.get(&rhs)
                            else {
                                panic!("exact RHS shell");
                            };
                            Ok(match &sum.payload {
                                NativeSumPayload::Some(token) | NativeSumPayload::Ok(token) => {
                                    Some(format!("{token:?}"))
                                }
                                NativeSumPayload::None | NativeSumPayload::Fail { .. } => None,
                            })
                        })
                        .unwrap();
                    let (status, output) = replace(auth, root, old, rhs);
                    assert_eq!(
                        status,
                        JettRuntimeStatusV1::OK,
                        "{family:?}/{old_occupied}/{rhs_occupied}"
                    );
                    let installed = ResourceHandleId::new(output).unwrap();
                    assert_published(
                        auth,
                        family,
                        root,
                        old,
                        rhs,
                        installed,
                        rhs_occupied,
                        rhs_key,
                    );
                    if let Some(generation) = rhs_generation {
                        auth.with_resource(|state, _, _| {
                            let Some(NativeResourceEntry::Sum(sum)) = state.handles.get(&installed)
                            else {
                                panic!("fresh destination shell");
                            };
                            let token = match &sum.payload {
                                NativeSumPayload::Some(token) | NativeSumPayload::Ok(token) => {
                                    token
                                }
                                _ => panic!("preserved occupied payload"),
                            };
                            assert_ne!(
                                format!("{token:?}"),
                                generation,
                                "the checked holder transfer advances its exact token generation"
                            );
                            Ok(())
                        })
                        .unwrap();
                    }
                    let mut expected = rows.iter().map(|row| (row.1, 1)).collect::<Vec<_>>();
                    if old_occupied {
                        expected.push((1101, 5));
                    }
                    assert_eq!(events(auth), expected);
                    auth.with_resource(|state, ordinary, registry| {
                        state.sum_drop(ordinary, registry, DROP_OLD, root, installed)
                    })
                    .unwrap();
                    if rhs_occupied {
                        expected.push((1102, 5));
                    }
                    assert_eq!(events(auth), expected);
                    assert_shell_count(auth, 0);
                    complete(auth, attempt, 0, 0);
                });
            }
        }
    }
}

#[test]
fn native_resource_sum_replace_primitive_and_nested_ordinary_failure_transitions() {
    for failure in [Failure::Integer, Failure::OptionalString] {
        let family = Family::Result(failure);
        for old_occupied in [false, true] {
            for rhs_occupied in [false, true] {
                context(|auth| {
                    let mut rows = Vec::new();
                    if old_occupied {
                        rows.push((1, 1201, 0, ""));
                    }
                    if rhs_occupied {
                        rows.push((1, 1202, 0, ""));
                    }
                    install_scripted(auth, &fixture(family), entry(), script(&rows, 1)).unwrap();
                    let (attempt, root, operation, network) = start(auth, ResourcePurpose::Runtime);
                    let old = make(
                        auth,
                        family,
                        root,
                        operation,
                        network,
                        0,
                        old_occupied,
                        1201,
                        None,
                    );
                    let rhs = make(
                        auth,
                        family,
                        root,
                        operation,
                        network,
                        1,
                        rhs_occupied,
                        1202,
                        None,
                    );
                    finish_setup(auth, operation);
                    let rhs_key = key(auth, rhs);
                    let (status, output) = replace(auth, root, old, rhs);
                    assert_eq!(
                        status,
                        JettRuntimeStatusV1::OK,
                        "{failure:?}/{old_occupied}/{rhs_occupied}"
                    );
                    assert_published(
                        auth,
                        family,
                        root,
                        old,
                        rhs,
                        ResourceHandleId::new(output).unwrap(),
                        rhs_occupied,
                        rhs_key,
                    );
                    complete(auth, attempt, 0, 0);
                });
            }
        }
    }
}

#[test]
fn native_resource_sum_replace_explicit_string_clone_survives_old_companion_retirement() {
    context(|auth| {
        let family = Family::Result(Failure::String);
        install_scripted(auth, &fixture(family), entry(), script(&[], 1)).unwrap();
        let (attempt, root, operation, network) = start(auth, ResourcePurpose::Runtime);
        let original = text(auth, "shared ordinary failure");
        let copied = auth
            .with_resource(|state, ordinary, _| {
                ordinary
                    .clone_resource_string_companion(&state.layout, 2, original)
                    .map_err(ordinary_error)
            })
            .unwrap();
        assert_eq!(
            copied, original,
            "the existing explicit String clone retains one identity"
        );
        let old = make(
            auth,
            family,
            root,
            operation,
            network,
            0,
            false,
            0,
            Some(original),
        );
        let rhs = make(
            auth,
            family,
            root,
            operation,
            network,
            1,
            false,
            0,
            Some(copied),
        );
        finish_setup(auth, operation);
        let (status, installed) = replace(auth, root, old, rhs);
        assert_eq!(status, JettRuntimeStatusV1::OK);
        auth.with_resource(|state, ordinary, registry| {
            ordinary
                .validate_resource_ordinary(copied, &state.layout, 2)
                .map_err(ordinary_error)?;
            state.sum_drop(
                ordinary,
                registry,
                DROP_OLD,
                root,
                ResourceHandleId::new(installed)?,
            )?;
            assert!(
                ordinary
                    .validate_resource_ordinary(copied, &state.layout, 2)
                    .is_err()
            );
            Ok(())
        })
        .unwrap();
        complete(auth, attempt, 0, 0);
        assert!(events(auth).is_empty());
    });
}

#[test]
fn native_resource_sum_replace_clean_reentry_uses_one_retained_context_and_grant() {
    context(|auth| {
        let family = Family::Result(Failure::String);
        install_scripted(
            auth,
            &fixture(family),
            entry(),
            script(&[(1, 1301, 0, ""), (1, 1302, 0, "")], 2),
        )
        .unwrap();
        let installation = auth
            .with_resource(|state, _, _| Ok((state.program_handle, state.network)))
            .unwrap();
        let mut previous_attempt = None;
        for label in [1301, 1302] {
            let (attempt, root, operation, network) = start(auth, ResourcePurpose::Runtime);
            assert_ne!(Some(attempt), previous_attempt);
            previous_attempt = Some(attempt);
            assert_eq!(
                auth.with_resource(|state, _, _| Ok((state.program_handle, state.network)))
                    .unwrap(),
                installation
            );
            let old = make(
                auth, family, root, operation, network, 0, false, label, None,
            );
            let rhs = make(auth, family, root, operation, network, 1, true, label, None);
            finish_setup(auth, operation);
            let (status, _) = replace(auth, root, old, rhs);
            assert_eq!(status, JettRuntimeStatusV1::OK);
            let outcome = auth
                .with_resource(|state, ordinary, registry| {
                    state.complete_entry(ordinary, registry, attempt, 0, false)
                })
                .unwrap();
            assert_eq!(
                (
                    outcome.body_status,
                    outcome.cleanup_status,
                    outcome.selected_kind
                ),
                (0, 0, 0)
            );
            assert_empty(auth);
            assert_shell_count(auth, 0);
        }
        assert_eq!(
            events(auth),
            vec![(1301, 1), (1301, 5), (1302, 1), (1302, 5)]
        );
        auth.with_resource(|state, _, _| {
            assert_eq!(state.provider.remaining(), 0);
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn native_resource_sum_replace_absent_old_and_rhs_shell_leases_refuse_before_effects() {
    for family in [Family::Optional, Family::Result(Failure::String)] {
        for borrow_old in [false, true] {
            context(|auth| {
                install_scripted(auth, &fixture(family), entry(), script(&[], 1)).unwrap();
                let (attempt, root, operation, network) = start(auth, ResourcePurpose::Runtime);
                let old = make(auth, family, root, operation, network, 0, false, 1401, None);
                let rhs = make(auth, family, root, operation, network, 1, false, 1402, None);
                finish_setup(auth, operation);
                let loan = auth
                    .with_resource(|state, ordinary, registry| {
                        state.sum_borrow(
                            ordinary,
                            registry,
                            if borrow_old { BORROW_OLD } else { BORROW_RHS },
                            root,
                            if borrow_old { old } else { rhs },
                        )
                    })
                    .unwrap();
                let before = snapshot(auth);
                let (status, output) = replace(auth, root, old, rhs);
                assert_eq!(
                    status,
                    NativeResourceError::Custody(CustodyError::ActiveBorrow).status()
                );
                assert_eq!(output, SENTINEL);
                assert_eq!(snapshot(auth), before);
                auth.with_resource(|state, _, registry| {
                    state.sum_borrow_end(
                        registry,
                        if borrow_old { END_OLD } else { END_RHS },
                        root,
                        loan,
                    )
                })
                .unwrap();
                // An exported refusal retains the existing first-error latch.
                complete(auth, attempt, 1, 0);
                assert!(events(auth).is_empty());
            });
        }
    }
}

#[test]
fn native_resource_sum_replace_occupied_payload_loan_on_either_operand_is_preflight_only() {
    for family in [Family::Optional, Family::Result(Failure::String)] {
        for borrow_old in [false, true] {
            context(|auth| {
                install_scripted(
                    auth,
                    &fixture(family),
                    entry(),
                    script(&[(1, 1501, 0, ""), (1, 1502, 0, "")], 1),
                )
                .unwrap();
                let (attempt, root, operation, network) = start(auth, ResourcePurpose::Runtime);
                let old = make(auth, family, root, operation, network, 0, true, 1501, None);
                let rhs = make(auth, family, root, operation, network, 1, true, 1502, None);
                finish_setup(auth, operation);
                let mut loan = auth
                    .with_resource(|state, _, registry| {
                        let Some(NativeResourceEntry::Sum(sum)) =
                            state.handles.get(&if borrow_old { old } else { rhs })
                        else {
                            panic!("occupied sum");
                        };
                        let token = match &sum.payload {
                            NativeSumPayload::Some(token) | NativeSumPayload::Ok(token) => token,
                            _ => panic!("occupied payload"),
                        };
                        let Some(NativeResourceEntry::Frame(borrower)) = state.handles.get(&root)
                        else {
                            panic!("exact Scope borrower");
                        };
                        state
                            .custody
                            .begin_borrow(token, &borrower.token, registry)
                            .map_err(NativeResourceError::from)
                    })
                    .unwrap();
                let before = snapshot(auth);
                let (status, output) = replace(auth, root, old, rhs);
                assert_eq!(
                    status,
                    NativeResourceError::Custody(CustodyError::ActiveBorrow).status()
                );
                assert_eq!(output, SENTINEL);
                assert_eq!(snapshot(auth), before);
                auth.with_resource(|state, _, _| {
                    state
                        .custody
                        .end_borrow(&mut loan)
                        .map_err(NativeResourceError::from)
                })
                .unwrap();
                complete(auth, attempt, 1, 0);
                assert_eq!(
                    events(auth),
                    vec![(1501, 1), (1502, 1), (1502, 5), (1501, 5)]
                );
            });
        }
    }
}

#[test]
fn native_resource_sum_replace_stale_duplicate_wrong_family_wrong_row_and_wrong_frame_have_no_cleanup_effect()
 {
    for invalid in 0..5 {
        context(|auth| {
            let family = Family::Result(Failure::String);
            install_scripted(
                auth,
                &fixture(family),
                entry(),
                script(&[(1, 1601, 0, ""), (1, 1602, 0, "")], 1),
            )
            .unwrap();
            let (attempt, root, operation, network) = start(auth, ResourcePurpose::Runtime);
            let old = make(auth, family, root, operation, network, 0, true, 1601, None);
            let rhs = make(auth, family, root, operation, network, 1, true, 1602, None);
            finish_setup(auth, operation);
            let before = snapshot(auth);
            let mut output = SENTINEL;
            let (frame, row, candidate) = match invalid {
                0 => (root.raw(), REPLACE, operation.raw()), // retired setup frame
                1 => (root.raw(), REPLACE, old.raw()),       // duplicate input
                2 => (
                    root.raw(),
                    REPLACE,
                    auth.with_resource(|state, _, _| Ok(state.program_handle.raw()))
                        .unwrap(),
                ),
                3 => (root.raw(), DROP_RHS, rhs.raw()), // wrong operation family
                4 => (old.raw(), REPLACE, rhs.raw()),   // nonframe input
                _ => unreachable!(),
            };
            let status = unsafe {
                leaves::jett_rt_v1_resource_replace(
                    pointer(auth),
                    frame,
                    row,
                    old.raw(),
                    candidate,
                    &mut output,
                )
            };
            assert_ne!(status, JettRuntimeStatusV1::OK);
            assert_eq!(output, SENTINEL);
            assert_eq!(snapshot(auth), before);
            complete(auth, attempt, 1, 0);
            assert_eq!(
                events(auth),
                vec![(1601, 1), (1602, 1), (1602, 5), (1601, 5)]
            );
        });
    }
}

#[test]
fn native_resource_sum_replace_output_preflight_precedes_lookup_and_preserves_a_running_attempt() {
    context(|auth| {
        let family = Family::Optional;
        install_scripted(auth, &fixture(family), entry(), script(&[], 1)).unwrap();
        let (attempt, root, operation, network) = start(auth, ResourcePurpose::Runtime);
        let old = make(auth, family, root, operation, network, 0, false, 0, None);
        let rhs = make(auth, family, root, operation, network, 1, false, 0, None);
        finish_setup(auth, operation);
        let before = snapshot(auth);
        let mut sentinel = SENTINEL;
        unsafe {
            for output in [
                std::ptr::null_mut(),
                pointer(auth).cast_mut().cast::<u64>(),
                (&mut sentinel as *mut u64)
                    .cast::<u8>()
                    .add(1)
                    .cast::<u64>(),
            ] {
                assert_eq!(
                    leaves::jett_rt_v1_resource_replace(pointer(auth), 0, u32::MAX, 0, 0, output),
                    JettRuntimeStatusV1::INVALID_ARGUMENT
                );
                assert_eq!(sentinel, SENTINEL);
                assert_eq!(snapshot(auth), before);
            }
        }
        let (status, _) = replace(auth, root, old, rhs);
        assert_eq!(status, JettRuntimeStatusV1::OK);
        complete(auth, attempt, 0, 0);
    });
}

#[test]
fn native_resource_sum_replace_pure_absence_is_permitted_for_each_existing_purpose() {
    for purpose in [
        ResourcePurpose::Runtime,
        ResourcePurpose::NamespaceConstant,
        ResourcePurpose::ExplicitComptime,
        ResourcePurpose::Verify,
        ResourcePurpose::Property,
    ] {
        for family in [
            Family::Optional,
            Family::Result(Failure::String),
            Family::Result(Failure::Integer),
            Family::Result(Failure::OptionalString),
        ] {
            context(|auth| {
                install_scripted(auth, &fixture(family), entry(), script(&[], 1)).unwrap();
                let (attempt, root, operation, network) = start(auth, purpose);
                let old = make(auth, family, root, operation, network, 0, false, 1701, None);
                let rhs = make(auth, family, root, operation, network, 1, false, 1702, None);
                finish_setup(auth, operation);
                assert_shell_count(auth, 2);
                let (status, installed) = replace(auth, root, old, rhs);
                assert_eq!(status, JettRuntimeStatusV1::OK, "{purpose:?}/{family:?}");
                assert_published(
                    auth,
                    family,
                    root,
                    old,
                    rhs,
                    ResourceHandleId::new(installed).unwrap(),
                    false,
                    None,
                );
                complete(auth, attempt, 0, 0);
                assert!(events(auth).is_empty());
            });
        }
    }
}

#[test]
fn native_resource_sum_replace_malformed_occupied_payload_never_gains_a_pure_purpose_bypass() {
    for purpose in [
        ResourcePurpose::NamespaceConstant,
        ResourcePurpose::ExplicitComptime,
        ResourcePurpose::Verify,
        ResourcePurpose::Property,
    ] {
        for family in [Family::Optional, Family::Result(Failure::String)] {
            for occupied_old in [false, true] {
                context(|auth| {
                    install_scripted(
                        auth,
                        &fixture(family),
                        entry(),
                        script(&[(1, 1801, 0, "")], 1),
                    )
                    .unwrap();
                    let (attempt, root, operation, network) = start(auth, ResourcePurpose::Runtime);
                    let old = make(
                        auth,
                        family,
                        root,
                        operation,
                        network,
                        0,
                        occupied_old,
                        1801,
                        None,
                    );
                    let rhs = make(
                        auth,
                        family,
                        root,
                        operation,
                        network,
                        1,
                        !occupied_old,
                        1801,
                        None,
                    );
                    finish_setup(auth, operation);
                    // Malformed-state control only: pure Source cannot mint this token.
                    auth.with_resource(|state, _, _| {
                        state.attempt.as_mut().unwrap().purpose = purpose;
                        Ok(())
                    })
                    .unwrap();
                    let before = snapshot(auth);
                    let (status, output) = replace(auth, root, old, rhs);
                    assert_eq!(status, NativeResourceError::WrongPurpose.status());
                    assert_eq!(output, SENTINEL);
                    assert_eq!(snapshot(auth), before);
                    auth.with_resource(|state, _, _| {
                        state.attempt.as_mut().unwrap().purpose = ResourcePurpose::Runtime;
                        Ok(())
                    })
                    .unwrap();
                    complete(auth, attempt, 1, 0);
                    assert_eq!(events(auth), vec![(1801, 1), (1801, 5)]);
                });
            }
        }
    }
}

#[test]
fn native_resource_sum_replace_old_finalizer_panic_retires_rhs_without_publication_and_keeps_first_cleanup()
 {
    for family in [Family::Optional, Family::Result(Failure::String)] {
        for rhs_panics in [false, true] {
            context(|auth| {
                install_scripted(
                    auth,
                    &fixture(family),
                    entry(),
                    script(
                        &[
                            (3, 1901, 0, ""),
                            (if rhs_panics { 3 } else { 1 }, 1902, 0, ""),
                        ],
                        1,
                    ),
                )
                .unwrap();
                let (attempt, root, operation, network) = start(auth, ResourcePurpose::Runtime);
                let old = make(auth, family, root, operation, network, 0, true, 1901, None);
                let rhs = make(auth, family, root, operation, network, 1, true, 1902, None);
                finish_setup(auth, operation);
                let (status, output) = replace(auth, root, old, rhs);
                assert_eq!(status, JettRuntimeStatusV1::PANIC);
                assert_eq!(output, SENTINEL);
                assert_eq!(
                    events(auth),
                    vec![(1901, 1), (1902, 1), (1901, 5), (1902, 5)]
                );
                assert_shell_count(auth, 0);
                auth.with_resource(|state, _, _| {
                    assert!(!state.handles.contains_key(&old));
                    assert!(!state.handles.contains_key(&rhs));
                    assert_eq!(
                        state.attempt.as_ref().unwrap().cleanup,
                        Some(ResourceCleanupFailure::FinalizerPanic)
                    );
                    Ok(())
                })
                .unwrap();
                complete(auth, attempt, 3, JettRuntimeStatusV1::PANIC.code());
                auth.with_resource(|state, _, _| {
                    assert_eq!(state.completed_messages(attempt)?.0, CLEANUP_FAILED);
                    Ok(())
                })
                .unwrap();
            });
        }
    }
}

#[test]
fn native_resource_sum_replace_invalid_failure_companion_is_rejected_before_old_finalizer() {
    context(|auth| {
        let family = Family::Result(Failure::String);
        install_scripted(
            auth,
            &fixture(family),
            entry(),
            script(&[(1, 2001, 0, "")], 1),
        )
        .unwrap();
        let (attempt, root, operation, network) = start(auth, ResourcePurpose::Runtime);
        let old = make(auth, family, root, operation, network, 0, true, 2001, None);
        let rhs = make(auth, family, root, operation, network, 1, false, 2002, None);
        finish_setup(auth, operation);
        let companion = auth
            .with_resource(|state, ordinary, _| {
                let Some(NativeResourceEntry::Sum(sum)) = state.handles.get(&rhs) else {
                    panic!("RHS shell");
                };
                let NativeSumPayload::Fail { shape, string } = sum.payload else {
                    panic!("failure companion");
                };
                ordinary
                    .drop_resource_typed_companion(&state.layout, shape, string)
                    .map_err(ordinary_error)?;
                Ok((shape, string))
            })
            .unwrap();
        let before = snapshot(auth);
        let (status, output) = replace(auth, root, old, rhs);
        assert_ne!(status, JettRuntimeStatusV1::OK);
        assert_eq!(output, SENTINEL);
        assert_eq!(snapshot(auth), before);
        assert_eq!(events(auth), vec![(2001, 1)]);
        // Repair only for exact terminal cleanup, never to retry or authorize Replace.
        let fresh = text(auth, "repair only for cleanup");
        auth.with_resource(|state, _, _| {
            let Some(NativeResourceEntry::Sum(sum)) = state.handles.get_mut(&rhs) else {
                panic!("retained malformed RHS");
            };
            let NativeSumPayload::Fail { shape, string } = &mut sum.payload else {
                panic!("retained failure companion");
            };
            assert_eq!((*shape, *string), companion);
            *string = fresh;
            Ok(())
        })
        .unwrap();
        complete(auth, attempt, 1, 0);
        assert_eq!(events(auth), vec![(2001, 1), (2001, 5)]);
    });
}

#[test]
fn native_resource_sum_replace_old_finalizer_panic_also_retires_absent_rhs_shell_and_ordinary_companion()
 {
    for family in [
        Family::Optional,
        Family::Result(Failure::String),
        Family::Result(Failure::Integer),
        Family::Result(Failure::OptionalString),
    ] {
        context(|auth| {
            install_scripted(
                auth,
                &fixture(family),
                entry(),
                script(&[(3, 2101, 0, "")], 1),
            )
            .unwrap();
            let (attempt, root, operation, network) = start(auth, ResourcePurpose::Runtime);
            let old = make(auth, family, root, operation, network, 0, true, 2101, None);
            let rhs = make(auth, family, root, operation, network, 1, false, 2102, None);
            finish_setup(auth, operation);
            assert_shell_count(auth, 2);
            let (status, output) = replace(auth, root, old, rhs);
            assert_eq!(status, JettRuntimeStatusV1::PANIC);
            assert_eq!(output, SENTINEL);
            assert_shell_count(auth, 0);
            auth.with_resource(|state, ordinary, registry| {
                assert!(!state.handles.contains_key(&old));
                assert!(!state.handles.contains_key(&rhs));
                assert_eq!(state.counts(ordinary, registry).ordinary_empty, 1);
                Ok(())
            })
            .unwrap();
            assert_eq!(events(auth), vec![(2101, 1), (2101, 5)]);
            complete(auth, attempt, 3, JettRuntimeStatusV1::PANIC.code());
        });
    }
}

#[test]
fn native_resource_sum_replace_incoming_finalizer_failure_occurs_at_later_scope_cleanup() {
    for family in [Family::Optional, Family::Result(Failure::String)] {
        context(|auth| {
            install_scripted(
                auth,
                &fixture(family),
                entry(),
                script(&[(1, 2201, 0, ""), (3, 2202, 0, "")], 1),
            )
            .unwrap();
            let (attempt, root, operation, network) = start(auth, ResourcePurpose::Runtime);
            let old = make(auth, family, root, operation, network, 0, true, 2201, None);
            let rhs = make(auth, family, root, operation, network, 1, true, 2202, None);
            finish_setup(auth, operation);
            let rhs_key = key(auth, rhs);
            let (status, output) = replace(auth, root, old, rhs);
            assert_eq!(status, JettRuntimeStatusV1::OK);
            assert_published(
                auth,
                family,
                root,
                old,
                rhs,
                ResourceHandleId::new(output).unwrap(),
                true,
                rhs_key,
            );
            assert_eq!(events(auth), vec![(2201, 1), (2202, 1), (2201, 5)]);
            auth.with_resource(|state, _, _| {
                assert!(state.attempt.as_ref().unwrap().body.is_none());
                assert!(state.attempt.as_ref().unwrap().cleanup.is_none());
                Ok(())
            })
            .unwrap();
            complete(auth, attempt, 3, JettRuntimeStatusV1::PANIC.code());
            assert_eq!(
                events(auth),
                vec![(2201, 1), (2202, 1), (2201, 5), (2202, 5)]
            );
        });
    }
}

#[test]
fn native_resource_sum_replace_shared_nested_ordinary_sum_is_not_an_explicit_string_clone() {
    context(|auth| {
        let family = Family::Result(Failure::OptionalString);
        install_scripted(auth, &fixture(family), entry(), script(&[], 1)).unwrap();
        let (attempt, root, operation, network) = start(auth, ResourcePurpose::Runtime);
        let old_companion = companion(auth, family, 2301);
        let rhs_companion = companion(auth, family, 2302);
        let old = make(
            auth,
            family,
            root,
            operation,
            network,
            0,
            false,
            0,
            Some(old_companion),
        );
        let rhs = make(
            auth,
            family,
            root,
            operation,
            network,
            1,
            false,
            0,
            Some(rhs_companion),
        );
        finish_setup(auth, operation);
        // Unsafe ABI-state alias: one ordinary sum identity cannot own two edges.
        auth.with_resource(|state, _, _| {
            let Some(NativeResourceEntry::Sum(sum)) = state.handles.get_mut(&rhs) else {
                panic!("RHS shell");
            };
            let NativeSumPayload::Fail { string, .. } = &mut sum.payload else {
                panic!("RHS failure");
            };
            *string = old_companion;
            Ok(())
        })
        .unwrap();
        let before = snapshot(auth);
        let (status, output) = replace(auth, root, old, rhs);
        assert_ne!(status, JettRuntimeStatusV1::OK);
        assert_eq!(output, SENTINEL);
        assert_eq!(snapshot(auth), before);
        auth.with_resource(|state, ordinary, _| {
            ordinary
                .validate_resource_ordinary(old_companion, &state.layout, 8)
                .map_err(ordinary_error)?;
            ordinary
                .validate_resource_ordinary(rhs_companion, &state.layout, 8)
                .map_err(ordinary_error)?;
            // Restore distinct independently owned edges only for terminal cleanup.
            let Some(NativeResourceEntry::Sum(sum)) = state.handles.get_mut(&rhs) else {
                panic!("RHS shell");
            };
            let NativeSumPayload::Fail { string, .. } = &mut sum.payload else {
                panic!("RHS failure");
            };
            assert_eq!(*string, old_companion);
            *string = rhs_companion;
            Ok(())
        })
        .unwrap();
        complete(auth, attempt, 1, 0);
        assert!(events(auth).is_empty());
    });
}
