use super::*;
use jett_common::{FileId, SourceOrigin};
use jett_hir::ExpressionKind;
use jett_parser::{ast::Item, parse};
use jett_resolve::ResourceKernelSpec;
use jett_typecheck::CheckedCallerEffect as Effect;
use jett_typecheck::{CheckOptions, CheckedResourceProgram};
use jett_types::ResourceKernelRecipe;
use std::{collections::HashMap, sync::Arc};

pub(super) const SUPPORT: &str = r#"namespace resource_probe
export resource TestHandle
export function create(view net: Network, label: int64) returns result[TestHandle, string]:
    return kernel_create(label: label, net: view net)
export function borrow(view net: Network, view token: TestHandle) returns result[int64, string]:
    return kernel_borrow(view net, view token)
export function close(token: TestHandle) returns nothing:
    kernel_close(token)
    return nothing
export function empty() returns optional[TestHandle]:
    return none
export function descriptor() returns function(TestHandle) returns nothing:
    return kernel_close
export function terminal_label() returns int64:
    list[int64] numbers = list(1)
    list[int64] removed = list.__remove_at[int64](numbers, -1)
    return 1
"#;
const SOURCE: &str = r#"namespace app
function pass_owner(token: resource_probe.TestHandle) returns resource_probe.TestHandle:
    return token
function pass_view(view token: resource_probe.TestHandle, view net: Network) returns result[int64, string]:
    use resource_probe
    return resource_probe.borrow(view net, view token)
function consume_pair(label: int64, token: resource_probe.TestHandle) returns nothing:
    use resource_probe
    resource_probe.close(token)
    return nothing
function scenario(view net: Network) returns nothing:
    use resource_probe
    resource_probe.TestHandle token = resource_probe.create(view net, 1) handle error:
        return nothing
    resource_probe.TestHandle moved = pass_owner(token)
    result[int64, string] observed = pass_view(view moved, view net)
    resource_probe.close(moved)
    return nothing
"#;
pub(super) fn checked(source: &str, release: bool) -> Arc<CheckedResourceProgram> {
    checked_support(source, SUPPORT, release)
}
pub(super) fn checked_support(
    source: &str,
    support: &str,
    release: bool,
) -> Arc<CheckedResourceProgram> {
    let stdlib = FileId::new(10_000);
    let project = FileId::new(0);
    let mut parsed = parse(support, stdlib);
    let primary = parse(source, project);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    assert!(primary.errors.is_empty(), "{:?}", primary.errors);
    let declaration = parsed
        .module
        .items
        .iter()
        .find_map(|item| match item {
            Item::Resource(resource) => Some(resource.name.span),
            _ => None,
        })
        .unwrap();
    parsed.module.items.extend(primary.module.items);
    let catalog = [
        ("kernel_create", ResourceKernelRecipe::NetworkFactory),
        ("kernel_borrow", ResourceKernelRecipe::NetworkBorrow),
        ("kernel_close", ResourceKernelRecipe::Finalize),
    ]
    .into_iter()
    .map(|(member, recipe)| ResourceKernelSpec {
        resource_declaration: declaration,
        member: member.into(),
        recipe,
    })
    .collect::<Vec<_>>();
    Arc::new(
        CheckedResourceProgram::prepare(
            parsed,
            HashMap::from([
                (stdlib, SourceOrigin::Stdlib),
                (project, SourceOrigin::Project),
            ]),
            &catalog,
            CheckOptions { release },
        )
        .unwrap_or_else(|error| panic!("{error:?}; {:?}", error.diagnostics())),
    )
}
pub(super) fn lowered(checked: &Arc<CheckedResourceProgram>) -> Program {
    let hir = hir::lower_checked_resource_program(checked).unwrap();
    lower(&hir, &checked.checked().interner).unwrap()
}
fn named<'a>(plan: &'a ResourceOwnershipPlan<'_>, name: &str) -> &'a ResourceFunctionPlan {
    plan.functions()
        .iter()
        .find(|function| {
            function.identity().declaration.namespace == "app"
                && function.identity().declaration.name == name
        })
        .expect("exact source test function")
}
#[test]
fn resource_ownership_source_factory_move_return_borrow_close_and_sum_cfg() {
    for release in [false, true] {
        let original = checked(SOURCE, release);
        let types = &original.checked().interner;
        let mir = lowered(&original);
        let plan = validate_resource_ownership(&mir, types).unwrap();
        let scenario = named(&plan, "scenario");
        assert!(scenario.operations().iter().any(|operation| matches!(
            operation.role(),
            ResourceOperationRole::SumTake { success: true, .. }
        )));
        assert!(
            scenario
                .operations()
                .iter()
                .any(|operation| matches!(operation.role(), ResourceOperationRole::Borrow { .. }))
        );
        assert!(scenario.operations().iter().any(|operation| matches!(
            operation.role(),
            ResourceOperationRole::InvokeSourceFunction { .. }
        )));
        let retained = named(&plan, "pass_view");
        assert!(retained.loans().iter().any(|loan| matches!(
            loan.source(),
            ResourceLoanSource::IncomingViewFormal { parameter: 0, .. }
        )));
        assert!(retained.operations().iter().any(|operation| matches!(
            operation.role(),
            ResourceOperationRole::BoundedBorrowUse { parameter: 1, .. }
        )));
        assert!(
            !retained
                .operations()
                .iter()
                .any(|operation| matches!(operation.role(), ResourceOperationRole::Acquire { .. }))
        );
        let returned = named(&plan, "pass_owner");
        let return_frame = returned
            .frames()
            .iter()
            .find(|frame| frame.role() == ResourceFrameRole::Return)
            .unwrap();
        assert_eq!(returned.frames()[0].parent(), Some(return_frame.id()));
        assert!(returned.operations().iter().any(|operation| matches!(
            operation.role(),
            ResourceOperationRole::CompleteReturnAfterCleanup { .. }
        )));
        assert!(
            plan.functions()
                .iter()
                .flat_map(|function| function.operations())
                .any(|operation| matches!(operation.role(), ResourceOperationRole::Close { .. }))
        );
        // Metadata proof does not grant ordinary Move/Copy or native Resource admission.
        let function = &mir.functions[scenario.function().index() as usize];
        assert!(
            crate::move_values::MoveValuePlan::analyze(&mir, function, types)
                .unwrap_err()
                .contains("pending ResourceOwnershipPlan")
        );
        assert!(
            crate::copy_values::CopyValuePlan::analyze(function, types)
                .unwrap_err()
                .contains("pending ResourceOwnershipPlan")
        );
    }
}
#[test]
fn resource_ownership_source_bare_to_view_consumes_one_call_holder_and_ends_its_loan() {
    const SOURCE: &str = r#"namespace app
function read_once(view token: resource_probe.TestHandle, view net: Network) returns result[int64, string]:
    use resource_probe
    return resource_probe.borrow(view net, view token)
function scenario(view net: Network) returns nothing:
    use resource_probe
    resource_probe.TestHandle token = resource_probe.create(view net, 2) handle error:
        return nothing
    result[int64, string] observed = read_once(token, view net)
    return nothing
"#;
    for release in [false, true] {
        let original = checked(SOURCE, release);
        let mir = lowered(&original);
        let plan = validate_resource_ownership(&mir, &original.checked().interner).unwrap();
        let function = named(&plan, "scenario");
        let operations = function.operations();
        let (invoke, frame, loan) = operations
            .iter()
            .enumerate()
            .find_map(|(index, operation)| {
                let ResourceOperationRole::InvokeSourceFunction {
                    source, operands, ..
                } = operation.role()
                else {
                    return None;
                };
                if !source
                    .arguments
                    .iter()
                    .any(|argument| argument.effect == Effect::RelinquishOwned)
                {
                    return None;
                }
                let loan = operands.iter().find_map(|operand| {
                    if let ResourceCallOperand::Borrowed { parameter: 0, loan } = operand {
                        Some(*loan)
                    } else {
                        None
                    }
                })?;
                Some((index, operation.frame(), loan))
            })
            .expect("original bare owner to View formal");
        let source = function.loans()[loan.index()].source();
        let ResourceLoanSource::Owner(holder) = source else {
            panic!("no owner from resident formal");
        };
        assert_eq!(function.owner_slots()[holder.index()].frame(), frame);
        assert!(matches!(
            function.owner_slots()[holder.index()].storage(),
            ResourceSlotStorage::Argument { parameter: 0, .. }
        ));
        let ended = operations.iter().position(|operation| matches!(operation.role(), ResourceOperationRole::EndBorrow { loan: actual } if *actual == loan)).unwrap();
        let dropped = operations.iter().position(|operation| matches!(operation.role(), ResourceOperationRole::Drop { source, .. } if *source == holder)).unwrap();
        assert!(invoke < ended && ended < dropped);
    }
}
#[test]
fn resource_ownership_source_named_order_holds_earlier_owner_for_later_terminal_actual() {
    const SOURCE: &str = r#"namespace app
function consume_pair(label: int64, token: resource_probe.TestHandle) returns nothing:
    use resource_probe
    resource_probe.close(token)
    return nothing
function scenario(view net: Network) returns nothing:
    use resource_probe
    resource_probe.TestHandle token = resource_probe.create(view net, 3) handle error:
        return nothing
    consume_pair(token: token, label: resource_probe.terminal_label())
    return nothing
"#;
    for release in [false, true] {
        let original = checked(SOURCE, release);
        let mir = lowered(&original);
        let plan = validate_resource_ownership(&mir, &original.checked().interner).unwrap();
        let function = named(&plan, "scenario");
        let invocation = function.operations().iter().find(|operation| matches!(operation.role(), ResourceOperationRole::InvokeSourceFunction { evaluation_order, .. } if evaluation_order == &[1, 0])).unwrap();
        let ResourceOperationRole::InvokeSourceFunction { operands, .. } = invocation.role() else {
            unreachable!()
        };
        let holder = operands
            .iter()
            .find_map(|operand| {
                if let ResourceCallOperand::Owned { parameter: 1, slot } = operand {
                    Some(*slot)
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(
            function.owner_slots()[holder.index()].frame(),
            invocation.frame()
        );
        assert!(function.operations().iter().take(invocation.id().index()).any(|operation| matches!(operation.role(), ResourceOperationRole::Transfer { destination, .. } if *destination == holder)));
        assert!(
            function
                .frames()
                .iter()
                .any(|frame| frame.id() == invocation.frame()
                    && frame.parent() == Some(ResourceFrameId(0)))
        );
        // The actual failing Source helper is not executed here; its caller holder
        // remains an operation cleanup destination, distinct from callee custody.
    }
}
#[test]
fn resource_ownership_source_current_graph_and_original_witness_mutations_fail_closed() {
    for release in [false, true] {
        let original = checked(SOURCE, release);
        let types = &original.checked().interner;
        let mir = lowered(&original);
        validate_resource_ownership(&mir, types).unwrap();
        for mutation in 0..7 {
            let mut changed = mir.clone();
            let function = changed
                .functions
                .iter_mut()
                .find(|function| {
                    function.identity.declaration.namespace == "app"
                        && function.identity.declaration.name == "scenario"
                })
                .unwrap();
            match mutation {
                0 => function.resource_lowering = None,
                1 => function.span.start += 1,
                2 => function.locals[0].mutable = !function.locals[0].mutable,
                3 => function.blocks[0].terminator.span.start += 1,
                4 => function.blocks[0].statements[0].span.start += 1,
                5 => function.identity.declaration.name.push_str("_foreign"),
                6 => changed.resource_manifest = hir::ResourceManifest::empty(),
                _ => unreachable!(),
            }
            assert!(
                validate_resource_ownership(&changed, types).is_err(),
                "mutation {mutation}"
            );
        }
    }
}
#[test]
fn resource_ownership_source_checked_compaction_keeps_archive_and_dense_current_graph() {
    for release in [false, true] {
        let original = checked(SOURCE, release);
        let types = &original.checked().interner;
        let mut mir = lowered(&original);
        let archives = mir
            .functions
            .iter()
            .filter_map(|function| {
                function
                    .resource_lowering
                    .as_ref()
                    .map(|witness| (function.id, witness.original.clone()))
            })
            .collect::<Vec<_>>();
        crate::prepare_native_generated_functions(&mut mir, types);
        validate_resource_ownership(&mir, types).unwrap();
        for (id, archived) in archives {
            let witness = mir.functions[id.index() as usize]
                .resource_lowering
                .as_ref()
                .unwrap();
            assert!(crate::breakpoint_regions::hir_blocks_equal(
                &archived.body,
                &witness.original.body
            ));
            assert_eq!(archived.locals, witness.original.locals);
            for (index, local) in mir.functions[id.index() as usize].locals.iter().enumerate() {
                assert_eq!(local.id.index() as usize, index);
            }
        }
    }
}
#[test]
fn resource_ownership_bitwise_source_archive_and_constructor_capture_reject_replaced_nodes() {
    let original = checked(SOURCE, false);
    let types = &original.checked().interner;
    let hir = hir::lower_checked_resource_program(&original).unwrap();
    let function = hir
        .functions
        .iter()
        .find(|function| {
            function.identity.declaration.namespace == "resource_probe"
                && function.identity.declaration.name == "create"
        })
        .unwrap();
    let mut capture = Capture::authenticated(
        function,
        &hir.resource_manifest,
        &hir.resource_source,
        types,
        &authenticate_original(&hir, types).unwrap(),
    );
    let hir::StatementKind::Return(Some(value)) = &function.body.statements[0].kind else {
        panic!("source factory return");
    };
    assert!(capture.preserve_original_call(value, types).unwrap());
    let mut replaced = value.clone();
    replaced.span.start += 1;
    assert!(capture.preserve_original_call(&replaced, types).is_err());
    let mut a = Expression {
        kind: hir::ExpressionKind::Float(f64::from_bits(0x7ff8_0000_0000_0011)),
        ty: TypeInterner::FLOAT64,
        span: value.span,
    };
    assert!(crate::breakpoint_regions::expressions_equal(&a, &a.clone()));
    let mut b = a.clone();
    b.kind = hir::ExpressionKind::Float(f64::from_bits(0x7ff8_0000_0000_0012));
    assert!(!crate::breakpoint_regions::expressions_equal(&a, &b));
    a.kind = hir::ExpressionKind::Float(0.0);
    b.kind = hir::ExpressionKind::Float(-0.0);
    assert!(!crate::breakpoint_regions::expressions_equal(&a, &b));
}
#[test]
fn resource_ownership_pending_nested_region_keeps_source_validity_and_no_fake_root_lifetime() {
    const SOURCE: &str = r#"namespace app
function scenario(view net: Network) returns nothing:
    use resource_probe
    if true:
        resource_probe.TestHandle token = resource_probe.create(view net, 4) handle error:
            return nothing
        resource_probe.close(token)
    return nothing
"#;
    for release in [false, true] {
        let original = checked(SOURCE, release);
        let mir = lowered(&original);
        assert!(
            validate_resource_ownership(&mir, &original.checked().interner)
                .unwrap_err()
                .iter()
                .any(|error| error.message.contains("nested Resource declarations"))
        );
        // This is an implementation prerequisite, never a checker language exclusion.
        validate_call_ownership(&mir, &original.checked().interner).unwrap();
    }
}

#[test]
fn resource_ownership_original_hir_archive_cannot_authenticate_edited_body_or_empty_archive() {
    for release in [false, true] {
        let original = checked(SOURCE, release);
        let types = &original.checked().interner;
        let initial = hir::lower_checked_resource_program(&original).unwrap();
        for mutation in 0..3 {
            let mut changed = initial.clone();
            if mutation == 0 {
                let function = changed
                    .functions
                    .iter_mut()
                    .find(|function| {
                        function.identity.declaration.namespace == "app"
                            && function.identity.declaration.name == "scenario"
                    })
                    .unwrap();
                let hir::StatementKind::Let { value, .. } = &mut function.body.statements[0].kind
                else {
                    panic!("original token declaration");
                };
                let hir::ExpressionKind::Handle { target, .. } = &mut value.kind else {
                    panic!("original factory handle");
                };
                let hir::ExpressionKind::Call { args, .. } = &mut target.kind else {
                    panic!("original factory Source call");
                };
                let hir::ExpressionKind::Int(number) = &mut args[1].kind else {
                    panic!("original factory label occurrence");
                };
                *number += 1;
            } else if mutation == 1 {
                changed.resource_source = hir::ResourceSourceArchive::empty();
            } else {
                changed.functions[0].debug_kind = hir::FunctionDebugKind::Inline;
            }
            // Existing public HIR gates do not themselves mint an original-body archive.
            hir::validate(&changed).unwrap();
            hir::validate_backend_types(&changed, types).unwrap();
            assert!(
                lower(&changed, types)
                    .unwrap_err()
                    .iter()
                    .any(|error| error.message.contains("original checked HIR archive"))
            );
        }
    }
}
#[test]
fn resource_ownership_original_type_graph_detects_resource_field_removal_and_accepts_additions() {
    const SOURCE: &str = r#"namespace app
struct Holder:
    token: optional[resource_probe.TestHandle]
function inspect(holder: Holder) returns nothing:
    return nothing
"#;
    for release in [false, true] {
        let original = checked(SOURCE, release);
        let original_types = &original.checked().interner;
        let hir = hir::lower_checked_resource_program(&original).unwrap();
        // Rebuild original nominal tables before copying their stable TypeIds.
        let mut types = TypeInterner::new();
        let inspected = hir
            .functions
            .iter()
            .find(|function| {
                function.identity.declaration.namespace == "app"
                    && function.identity.declaration.name == "inspect"
            })
            .unwrap();
        let Type::Struct(id) = original_types.resolve(inspected.params[0].ty) else {
            panic!("the checked inspect formal must be the Source Holder");
        };
        let id = *id;
        let definition = original_types.resolve_struct(id).clone();
        assert!(matches!(
            original_types.resolve(definition.fields[0].1),
            Type::Optional(_)
        ));
        macro_rules! copy_nominals {
            ($variant:ident, $resolve:ident, $add:ident) => {
                let definitions = original_types
                    .type_ids()
                    .filter_map(|ty| match original_types.resolve(ty) {
                        Type::$variant(definition_id) => Some((
                            definition_id.index(),
                            (
                                *definition_id,
                                original_types.$resolve(*definition_id).clone(),
                            ),
                        )),
                        _ => None,
                    })
                    .collect::<std::collections::BTreeMap<_, _>>();
                for (definition_id, original_definition) in definitions.into_values() {
                    assert_eq!(types.$add(original_definition), definition_id);
                }
            };
        }
        copy_nominals!(Struct, resolve_struct, add_struct);
        copy_nominals!(Bitfield, resolve_bitfield, add_bitfield);
        copy_nominals!(Enum, resolve_enum, add_enum);
        copy_nominals!(Interface, resolve_interface, add_interface);
        copy_nominals!(Actor, resolve_actor, add_actor);
        copy_nominals!(Machine, resolve_machine, add_machine);
        for ty in original_types.type_ids() {
            assert_eq!(types.intern(original_types.resolve(ty).clone()), ty);
        }
        for ty in original_types.type_ids() {
            let args = original_types.nominal_type_arguments(ty);
            if !args.is_empty() {
                types
                    .register_nominal_type_arguments(ty, args.to_vec())
                    .unwrap();
            }
        }
        hir.resource_source.validate_types(&types).unwrap();
        types.intern(Type::Optional(TypeInterner::UINT64));
        hir.resource_source.validate_types(&types).unwrap();
        let mut erased = definition.clone();
        erased.fields.clear();
        types.update_struct(id, erased);
        assert!(
            hir.resource_source
                .validate_types(&types)
                .unwrap_err()
                .contains("type meaning or nominal metadata")
        );
        assert!(
            lower(&hir, &types)
                .unwrap_err()
                .iter()
                .any(|error| error.message.contains("type meaning or nominal metadata"))
        );
        let mut foreign = definition;
        foreign.fields[0].1 = TypeInterner::STRING;
        types.update_struct(id, foreign);
        assert!(hir.resource_source.validate_types(&types).is_err());
    }
}

const MUTABLE_REPLACEMENT_SOURCE: &str = include_str!(
    "../../../jett_codegen_cranelift/src/emit/resource_execution/replacement/mutable_assignment.jett"
);

#[test]
fn resource_ownership_replacement_seals_the_actual_rhs_and_original_mutable_site() {
    for release in [false, true] {
        let original = checked(MUTABLE_REPLACEMENT_SOURCE, release);
        let program = lowered(&original);
        let plan = validate_resource_ownership(&program, &original.checked().interner).unwrap();
        for name in ["replace_live", "failed_rhs_keeps_owner"] {
            let function = named(&plan, name);
            let replacements = function
                .operations()
                .iter()
                .filter(|operation| {
                    matches!(operation.role(), ResourceOperationRole::Replace { .. })
                })
                .collect::<Vec<_>>();
            assert_eq!(replacements.len(), 1, "{name}");
            let operation = replacements[0];
            let ResourceOperationRole::Replace {
                destination,
                replacement,
                old,
            } = operation.role()
            else {
                unreachable!();
            };
            assert_ne!(destination, replacement);
            assert_eq!(*old, ResourceOccupancy::Occupied);
            assert!(!operation.is_expression_operation());
            let current = &program.functions[function.function().index() as usize];
            let ResourcePosition::Statement(index) = operation.site().position() else {
                panic!("assignment statement site");
            };
            let StatementKind::Assign { target, value } =
                &current.blocks[operation.site().block().index() as usize].statements[index].kind
            else {
                panic!("exact original assignment");
            };
            let (ExpressionKind::Local(target), ExpressionKind::Local(rhs)) =
                (&target.kind, &value.kind)
            else {
                panic!("canonical handled Source RHS and local destination");
            };
            let ResourceSlotStorage::Local { header } =
                function.owner_slots()[destination.index()].storage()
            else {
                panic!("original mutable owning header");
            };
            assert_eq!(header.id, *target);
            assert!(header.mutable);
            assert_eq!(
                function.owner_slots()[destination.index()].shape(),
                function.owner_slots()[replacement.index()].shape()
            );
            let ResourceSlotStorage::Local { header: rhs_header } =
                function.owner_slots()[replacement.index()].storage()
            else {
                panic!("evaluated canonical RHS local");
            };
            assert_eq!(rhs_header.id, *rhs);
            assert!(function.operations().iter().any(|other| other.site() == operation.site()
                && matches!(other.role(), ResourceOperationRole::Transfer { source, destination: selected }
                    if source == replacement && selected == destination)));
        }
    }
}

#[test]
fn resource_ownership_self_rebind_and_closed_destination_keep_distinct_exact_roles() {
    for release in [false, true] {
        let original = checked(MUTABLE_REPLACEMENT_SOURCE, release);
        let program = lowered(&original);
        let plan = validate_resource_ownership(&program, &original.checked().interner).unwrap();
        let function = named(&plan, "rebind_self");
        let reseats = function
            .operations()
            .iter()
            .filter(|operation| {
                matches!(operation.role(), ResourceOperationRole::SelfRebind { .. })
            })
            .collect::<Vec<_>>();
        assert_eq!(reseats.len(), 1);
        let operation = reseats[0];
        let ResourceOperationRole::SelfRebind { slot } = operation.role() else {
            unreachable!()
        };
        let ResourceSlotStorage::Local { header } = function.owner_slots()[slot.index()].storage()
        else {
            panic!("owning local")
        };
        assert!(header.mutable);
        let current = &program.functions[function.function().index() as usize];
        let ResourcePosition::Statement(index) = operation.site().position() else {
            panic!("assignment")
        };
        let StatementKind::Assign { target, value } =
            &current.blocks[operation.site().block().index() as usize].statements[index].kind
        else {
            panic!("self assignment")
        };
        assert!(matches!(target.kind, ExpressionKind::Local(local) if local == header.id));
        assert!(matches!(value.kind, ExpressionKind::Local(local) if local == header.id));
        assert!(
            !function
                .operations()
                .iter()
                .any(|other| other.site() == operation.site()
                    && matches!(
                        other.role(),
                        ResourceOperationRole::Replace { .. }
                            | ResourceOperationRole::Transfer { .. }
                            | ResourceOperationRole::Drop { .. }
                    ))
        );
        let closed = named(&plan, "rebind_after_close");
        assert!(!closed.operations().iter().any(|operation| matches!(
            operation.role(),
            ResourceOperationRole::Replace { .. } | ResourceOperationRole::SelfRebind { .. }
        )));
        let current = &program.functions[closed.function().index() as usize];
        assert!(closed.operations().iter().any(|operation| {
            let ResourcePosition::Statement(index) = operation.site().position() else {
                return false;
            };
            matches!(
                current.blocks[operation.site().block().index() as usize].statements[index].kind,
                StatementKind::Assign { .. }
            ) && matches!(operation.role(), ResourceOperationRole::Transfer { .. })
        }));
    }
}

#[test]
fn resource_ownership_replacement_and_reseat_reject_changed_public_source_or_header() {
    for release in [false, true] {
        let original = checked(MUTABLE_REPLACEMENT_SOURCE, release);
        let program = lowered(&original);
        let types = &original.checked().interner;
        for mutation in 0..3 {
            let mut changed = program.clone();
            let function = changed
                .functions
                .iter_mut()
                .find(|function| {
                    function.identity.declaration.namespace == "app"
                        && function.identity.declaration.name == "rebind_self"
                })
                .unwrap();
            let assignment = function
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.statements)
                .find(|statement| matches!(statement.kind, StatementKind::Assign { .. }))
                .unwrap();
            let StatementKind::Assign { target, value } = &mut assignment.kind else {
                unreachable!()
            };
            let ExpressionKind::Local(local) = target.kind else {
                unreachable!()
            };
            if mutation == 0 {
                function.locals[local.index() as usize].mutable = false;
            } else if mutation == 1 {
                value.kind = ExpressionKind::View(Box::new(value.clone()));
            } else {
                value.span.start += 1;
            }
            assert!(validate_resource_ownership(&changed, types).is_err());
            assert!(validate_witnesses(&changed, types).is_err());
        }
    }
}
