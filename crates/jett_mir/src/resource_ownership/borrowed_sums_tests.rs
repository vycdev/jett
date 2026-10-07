use super::tests::{checked_support, lowered};
use super::*;

const SUPPORT: &str =
    include_str!("../../../jett_driver/tests/native_conformance/resource/resource_probe.jett");
const SOME: &str = include_str!(
    "../../../jett_driver/tests/native_conformance/resource/47_view_optional_some_written.jett"
);
const SOURCES: [&str; 12] = [
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/45_view_optional_none_written.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/46_view_optional_none_bare.jett"
    ),
    SOME,
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/48_view_optional_some_bare.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/49_view_result_fail_written.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/50_view_result_fail_bare.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/51_view_result_ok_written.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/52_view_result_ok_bare.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/53_view_optional_written_argument_abort.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/54_view_result_bare_argument_abort.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/55_view_optional_written_borrow_fail.jett"
    ),
    include_str!(
        "../../../jett_driver/tests/native_conformance/resource/56_view_result_bare_borrow_fail.jett"
    ),
];

fn observer(program: &Program) -> &Function {
    program
        .functions
        .iter()
        .find(|function| {
            function.identity.declaration.namespace == "app"
                && function.identity.declaration.name.starts_with("observe_")
        })
        .unwrap()
}
fn observer_mut(program: &mut Program) -> &mut Function {
    program
        .functions
        .iter_mut()
        .find(|function| {
            function.identity.declaration.namespace == "app"
                && function.identity.declaration.name.starts_with("observe_")
        })
        .unwrap()
}

#[test]
fn resource_borrowed_sums_loan_order_preserves_exact_tag_path_and_parent() {
    let projected = |parent, tag, path| ResourceLoanSource::ProjectedSumPayload {
        parent: ResourceLoanId(parent),
        tag: LocalId::new(tag),
        path,
    };
    let sources = [
        ResourceLoanSource::Owner(ResourceOwnerSlotId(0)),
        ResourceLoanSource::IncomingViewFormal {
            scope: ResourceFrameId(0),
            parameter: 0,
        },
        ResourceLoanSource::IncomingViewFormal {
            scope: ResourceFrameId(0),
            parameter: 1,
        },
        ResourceLoanSource::IncomingViewFormal {
            scope: ResourceFrameId(1),
            parameter: 0,
        },
        projected(0, 0, ResourceSumPayloadPath::OptionalSome),
        projected(0, 1, ResourceSumPayloadPath::OptionalSome),
        projected(0, 0, ResourceSumPayloadPath::ResultOk),
        projected(1, 0, ResourceSumPayloadPath::OptionalSome),
    ];
    for left in sources {
        for right in sources {
            assert_eq!(left.cmp(&right).is_eq(), left == right);
        }
    }
    assert_eq!(
        sources.into_iter().collect::<BTreeSet<_>>().len(),
        sources.len()
    );
}

#[test]
fn resource_borrowed_sums_preserve_nonowning_headers_and_all_source_call_shapes() {
    for release in [false, true] {
        for (fixture, source) in SOURCES.into_iter().enumerate() {
            let checked = checked_support(source, SUPPORT, release);
            let types = &checked.checked().interner;
            let mut program = lowered(&checked);
            for function in &mut program.functions {
                assert!(
                    crate::sequences::prune::unreachable(function),
                    "borrowed canonical fixture {} release={release}",
                    fixture + 45
                );
            }
            let ownership = validate_resource_ownership(&program, types).unwrap_or_else(|errors| {
                panic!(
                    "borrowed fixture {} release={release}: {errors:?}",
                    fixture + 45
                )
            });
            let current = observer(&program);
            let plan = ownership.function(current.id).unwrap();
            let projected = plan
                .operations()
                .iter()
                .find_map(|operation| match operation.role() {
                    ResourceOperationRole::ProjectSumView {
                        source,
                        destination,
                        tag,
                        path,
                    } => Some((*source, *destination, *tag, *path, operation.site())),
                    _ => None,
                })
                .expect("exact guarded borrowed payload projection");
            let (parent, child, tag, path, site) = projected;
            let row = current
                .resource_borrowed_sum_projection(site.block(), site.position())
                .unwrap()
                .unwrap();
            assert_eq!(row.tag(), tag);
            assert_eq!(row.path(), path);
            assert!(matches!(
                plan.loans()[parent.index()].source(),
                ResourceLoanSource::IncomingViewFormal { .. }
            ));
            assert!(plan.loans()[parent.index()].shape().conditional());
            assert_eq!(
                plan.loans()[child.index()].source(),
                ResourceLoanSource::ProjectedSumPayload { parent, tag, path }
            );
            assert!(!plan.loans()[child.index()].shape().conditional());
            for local in [row.source(), row.output(), row.alias()] {
                assert!(current.is_view_local(local));
                assert!(!current.local(local).unwrap().mutable);
                assert!(!plan.owner_slots().iter().any(|slot| matches!(slot.storage(), ResourceSlotStorage::Local { header } if header.id == local)));
            }
            assert!(!plan.operations().iter().any(|operation| matches!(operation.role(), ResourceOperationRole::EndSumBorrow { loan } if *loan == parent)));
            assert!(plan.operations().iter().any(|operation| matches!(operation.role(), ResourceOperationRole::EndBorrow { loan } if *loan == child)));
            if fixture >= 4 && fixture != 8 && fixture != 10 {
                assert!(plan.operations().iter().any(|operation| matches!(operation.role(), ResourceOperationRole::ReadFailureCompanion { failure, .. } if *failure == TypeInterner::STRING)));
            }
            for function in ownership.functions() {
                let companion = ResourceCompanionPlan::analyze(&ownership, function.function())
                    .unwrap_or_else(|error| {
                        panic!(
                            "borrowed companion fixture {} release={release}: {error}",
                            fixture + 45
                        )
                    });
                let current = &program.functions[function.function().index() as usize];
                assert!(
                    current
                        .locals
                        .iter()
                        .filter(|local| resource_type_pending(types, local.ty))
                        .all(|local| !companion
                            .storage()
                            .owned_locals
                            .contains(&(local.id.index() as usize)))
                );
            }
            let main = ownership
                .functions()
                .iter()
                .find(|function| {
                    function.identity().declaration.namespace == "app"
                        && function.identity().declaration.name == "main"
                })
                .unwrap();
            assert!(main.operations().iter().any(|operation| matches!(
                operation.role(),
                ResourceOperationRole::BorrowSum { .. }
                    | ResourceOperationRole::PrepareSourceSumBorrow { .. }
            )));
            assert!(main.operations().iter().any(|operation| matches!(
                operation.role(),
                ResourceOperationRole::EndSumBorrow { .. }
            )));
            if fixture == 8 || fixture == 9 {
                assert!(main.operations().iter().any(|operation| matches!(
                    operation.role(),
                    ResourceOperationRole::PrepareSourceSumBorrow { .. }
                )));
            }
        }
    }
}

#[test]
fn resource_borrowed_sums_refuse_header_tag_and_disconnected_projection_resealing() {
    for release in [false, true] {
        let checked = checked_support(SOME, SUPPORT, release);
        let types = &checked.checked().interner;
        let baseline = lowered(&checked);
        let current = observer(&baseline);
        let row = current
            .resource_lowering
            .as_ref()
            .unwrap()
            .borrowed_sums
            .first()
            .unwrap();
        let output = row.output();
        let source = row.source();
        let backing = row.backing();
        let site = row.success_site();
        for mutation in 0..8 {
            let mut forged = baseline.clone();
            let current = observer_mut(&mut forged);
            match mutation {
                0 => current.locals[output.index() as usize].view_source = None,
                1 => current.locals[output.index() as usize].mutable = true,
                2 => {
                    let ResourcePosition::Statement(index) = site.position() else {
                        unreachable!()
                    };
                    let StatementKind::SumTake { source, .. } =
                        &mut current.blocks[site.block().index() as usize].statements[index].kind
                    else {
                        unreachable!()
                    };
                    *source = backing;
                }
                3 => {
                    let ResourcePosition::Statement(index) = site.position() else {
                        unreachable!()
                    };
                    let StatementKind::SumTake { success, .. } =
                        &mut current.blocks[site.block().index() as usize].statements[index].kind
                    else {
                        unreachable!()
                    };
                    *success = false;
                }
                4 => {
                    let row = current
                        .resource_lowering
                        .as_ref()
                        .unwrap()
                        .borrowed_sums
                        .first()
                        .unwrap();
                    let tag = row.tag();
                    let branch = current.blocks.iter_mut().find(|block| matches!(&block.terminator.kind, TerminatorKind::Branch { condition, .. } if matches!(condition.kind, hir::ExpressionKind::Local(local) if local == tag))).unwrap();
                    let TerminatorKind::Branch {
                        then_block,
                        else_block,
                        ..
                    } = &mut branch.terminator.kind
                    else {
                        unreachable!()
                    };
                    *then_block = *else_block;
                }
                5 => {
                    let ResourcePosition::Statement(index) = site.position() else {
                        unreachable!()
                    };
                    let statement =
                        current.blocks[site.block().index() as usize].statements[index].clone();
                    current.blocks.push(BasicBlock {
                        id: BlockId(current.blocks.len() as u32),
                        statements: vec![statement],
                        terminator: Terminator {
                            kind: TerminatorKind::Unreachable,
                            span: current.span,
                        },
                    });
                }
                6 => current
                    .resource_lowering
                    .as_mut()
                    .unwrap()
                    .borrowed_sums
                    .clear(),
                7 => {
                    current.locals[source.index() as usize].ty =
                        current.locals[output.index() as usize].ty
                }
                _ => unreachable!(),
            }
            // Public general cache resealing must not replace independent transport.
            let witness = current.resource_lowering.as_mut().unwrap();
            witness.blocks = current.blocks.clone();
            witness.locals = current.locals.clone();
            assert!(
                validate_resource_ownership(&forged, types).is_err(),
                "mutation={mutation} release={release}"
            );
            assert!(
                !crate::sequences::prune::unreachable(observer_mut(&mut forged)),
                "canonical mutation={mutation} release={release}"
            );
        }
        let mut missing = baseline.clone();
        observer_mut(&mut missing).resource_lowering = None;
        assert!(validate_resource_ownership(&missing, types).is_err());
        let mut foreign = baseline.clone();
        let other = checked_support(SOME, SUPPORT, release);
        observer_mut(&mut foreign).resource_lowering =
            observer(&lowered(&other)).resource_lowering.clone();
        assert!(validate_resource_ownership(&foreign, types).is_err());
    }
}
