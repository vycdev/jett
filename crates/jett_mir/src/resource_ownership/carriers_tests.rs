use super::carriers::{
    ResourceCarrierConstructor, ResourceCarrierEdge, ResourceCarrierFunctionPlan,
    ResourceCarrierIndex, ResourceCarrierLoanSource, ResourceCarrierNode,
    ResourceCarrierObservation, ResourceCarrierOperationRole, ResourceCarrierProjectionPath,
    ResourceCarrierShape, ResourceCarrierShapeId,
};
use super::tests::{SUPPORT, checked_support};
use super::*;
use jett_comptime::checked_types::CheckedExpressionTypes;
use jett_typecheck::CheckedResourceProgram;
use std::{collections::HashMap, sync::Arc};

const SOURCE: &str = include_str!("fixtures/17_absent_aggregate_shapes.jett");
const CONSTRUCTORS: [&str; 12] = [
    "empty_tokens",
    "absent_tokens",
    "empty_map",
    "absent_map",
    "empty_result_tokens",
    "failed_result_tokens",
    "absent_envelope",
    "absent_box",
    "empty_choice",
    "absent_choice",
    "absent_state",
    "absent_exact_state",
];
const CONSUMING_SUFFIX: &str = r#"
function source17_absent_optional(view candidate: optional[resource_probe.TestHandle]) returns bool:
    resource_probe.TestHandle token = view((view candidate) handle:
        return true
    )
    return false
export function consuming_absent_list() returns int64:
    list[optional[resource_probe.TestHandle]] values = absent_tokens()
    mutable int64 absent_count = 0
    for candidate in values:
        if source17_absent_optional(view candidate):
            absent_count = absent_count + 1
    return absent_count
export function consuming_absent_map() returns int64:
    map[string, optional[resource_probe.TestHandle]] values = absent_map()
    mutable int64 absent_count = 0
    for key, candidate in values:
        if key != "empty":
            return -1
        if source17_absent_optional(view candidate):
            absent_count = absent_count + 1
    return absent_count
export function borrowed_absent_fallback() returns int64:
    TokenEnvelope value = absent_envelope()
    if source17_absent_optional(view value.fallback):
        return value.marker
    return 0
export function matching_absent_choice() returns int64:
    TokenChoice value = absent_choice()
    match value:
        vacant(marker, tokens):
            return marker
        other:
            return 0
    return -1
"#;

fn materialized(checked: &Arc<CheckedResourceProgram>) -> hir::Program {
    let expression_types = Arc::new(CheckedExpressionTypes {
        resource_program: Some(checked.clone()),
        expressions: checked
            .checked()
            .type_map
            .iter()
            .map(|(span, ty)| (*span, checked.checked().interner.type_name(*ty)))
            .collect(),
        ..Default::default()
    });
    let required = jett_comptime::evaluate_explicit_comptime_expressions_capture(
        checked.module(),
        Arc::new(jett_types::ReflectionMetadata::new()),
        expression_types,
        Arc::new(HashMap::new()),
    );
    assert!(
        required.diagnostics.is_empty(),
        "{:?}",
        required.diagnostics
    );
    assert!(required.debug_events.is_empty());
    required.values.checked_required_values(checked).unwrap();
    assert!(!required.values.is_empty());
    assert!(
        required
            .values
            .values()
            .all(|value| matches!(value, jett_comptime::Value::Int64(17)))
    );
    let mut program = hir::lower_checked_resource_program(checked).unwrap();
    hir::materialize_checked_required_values(
        &mut program,
        &required.values,
        &checked.checked().interner,
    )
    .unwrap();
    hir::complete_value_conversions(&mut program, &checked.checked().interner).unwrap();
    program
}

fn lowered(checked: &Arc<CheckedResourceProgram>) -> Program {
    let program = materialized(checked);
    lower(&program, &checked.checked().interner).unwrap()
}

fn named<'a>(plan: &'a ResourceOwnershipPlan<'_>, name: &str) -> &'a ResourceFunctionPlan {
    plan.functions()
        .iter()
        .find(|function| {
            function.identity().declaration.namespace == "app"
                && function.identity().declaration.name == name
        })
        .unwrap_or_else(|| panic!("retained app.{name} Resource plan"))
}

fn function<'a>(program: &'a Program, name: &str) -> &'a Function {
    program
        .functions
        .iter()
        .find(|function| {
            function.identity.declaration.namespace == "app"
                && function.identity.declaration.name == name
        })
        .unwrap_or_else(|| panic!("retained app.{name} MIR function"))
}

fn function_mut<'a>(program: &'a mut Program, name: &str) -> &'a mut Function {
    program
        .functions
        .iter_mut()
        .find(|function| {
            function.identity.declaration.namespace == "app"
                && function.identity.declaration.name == name
        })
        .unwrap_or_else(|| panic!("retained app.{name} MIR function"))
}

fn carrier(plan: &ResourceFunctionPlan) -> &ResourceCarrierFunctionPlan {
    plan.carriers()
        .expect("typed aggregate transport, including latent occupied arms")
}

fn result_shape(plan: &ResourceFunctionPlan) -> &ResourceCarrierShape {
    carrier(plan).shape_for_type(plan.return_type()).unwrap()
}

fn child(plan: &ResourceCarrierFunctionPlan, id: ResourceCarrierShapeId) -> &ResourceCarrierShape {
    plan.shape(id)
        .expect("a checked graph edge retains its exact target type")
}

fn consuming_program(release: bool) -> (Arc<CheckedResourceProgram>, Program) {
    let source = format!("{SOURCE}{CONSUMING_SUFFIX}");
    assert!(source.starts_with(SOURCE));
    let checked = checked_support(&source, SUPPORT, release);
    let program = lowered(&checked);
    (checked, program)
}

fn original_for(
    program: &Program,
    name: &str,
) -> (BlockId, BlockId, BlockId, Vec<LocalId>, TypeId) {
    function(program, name)
        .blocks
        .iter()
        .find_map(|block| match &block.terminator.kind {
            TerminatorKind::ForEach {
                key,
                value,
                iterable,
                body,
                exit,
                ..
            } => {
                let mut binders = vec![*key];
                if let Some(value) = value {
                    binders.push(*value);
                }
                Some((block.id, *body, *exit, binders, iterable.ty))
            }
            _ => None,
        })
        .expect("one original consuming For header")
}

fn assert_leaf_binder(plan: &ResourceFunctionPlan, function: &Function, local: LocalId) {
    let header = function.local(local).unwrap();
    assert!(
        plan.owner_slots()
            .iter()
            .any(|slot| matches!(slot.storage(),
                ResourceSlotStorage::Local { header: captured } if captured == header
            ) && matches!(slot.shape(), ResourceShape::Optional { .. })),
        "the original Optional[Resource] binder needs its exact dedicated leaf-sum slot"
    );
}

fn assert_optional_view_call(plan: &ResourceFunctionPlan, helper: FunctionId) {
    let loan = plan
        .operations()
        .iter()
        .find_map(|operation| match operation.role() {
            ResourceOperationRole::InvokeSourceFunction {
                function, operands, ..
            } if *function == helper => match operands.as_slice() {
                [ResourceCallOperand::Borrowed { parameter: 0, loan }] => Some(*loan),
                _ => None,
            },
            _ => None,
        })
        .expect("the real Optional view helper uses the bounded leaf-sum call protocol");
    assert!(matches!(
        plan.loans()[loan.index()].source(),
        ResourceLoanSource::Owner(_)
    ));
    assert!(
        plan.operations()
            .iter()
            .any(|operation| matches!(operation.role(),
                ResourceOperationRole::BorrowSum { loan: actual } if *actual == loan
            ))
    );
    assert!(
        plan.operations()
            .iter()
            .any(|operation| matches!(operation.role(),
                ResourceOperationRole::EndSumBorrow { loan: actual } if *actual == loan
            ))
    );
}

#[test]
fn resource_carrier_consuming_source17_for_rows_preserve_raw_and_prepared_cursor_and_binder_transport()
 {
    for release in [false, true] {
        let (checked, raw) = consuming_program(release);
        let types = &checked.checked().interner;
        let helper = function(&raw, "source17_absent_optional").id;
        let raw_plan = validate_resource_ownership(&raw, types).unwrap();
        let mut prepared = raw.clone();
        crate::prepare_native_sequences(&mut prepared, types);
        crate::validate(&prepared).unwrap();
        let prepared_plan = validate_resource_ownership(&prepared, types).unwrap();
        for name in ["consuming_absent_list", "consuming_absent_map"] {
            let original = original_for(&raw, name);
            let raw_function = named(&raw_plan, name);
            let raw_carriers = carrier(raw_function);
            let [raw_iteration] = raw_carriers.iterations() else {
                panic!("one original For iteration row")
            };
            assert_eq!(
                (raw_iteration.header, raw_iteration.body, raw_iteration.exit),
                (original.0, original.1, original.2)
            );
            assert_eq!(raw_iteration.binders, original.3);
            assert_eq!(raw_iteration.cursor, None);
            assert_eq!(
                child(
                    raw_carriers,
                    raw_carriers.slots()[raw_iteration.source.index()].shape()
                )
                .ty(),
                original.4
            );
            let raw_edge = ResourceCarrierEdge::IterationBody {
                source: original.0,
                target: original.1,
            };
            assert!(
                raw_carriers.operations_on_edge(raw_edge).any(
                    |operation| matches!(operation.role(),
                        ResourceCarrierOperationRole::AdaptSum { path, loan: None, .. }
                            if matches!(path.as_slice(), [ResourceCarrierProjectionPath::Element {
                                index: ResourceCarrierIndex::CurrentIteration { header }
                            }] | [ResourceCarrierProjectionPath::MapValue {
                                index: ResourceCarrierIndex::CurrentIteration { header }
                            }] if *header == original.0)
                    )
                ),
                "raw consuming For extracts an absent leaf-sum shell on its exact selected body edge"
            );
            assert!(
                raw_function.loans().iter().all(|loan| !matches!(
                    loan.source(),
                    ResourceLoanSource::CarrierSumProjection { .. }
                )),
                "an owned For binder is an owner, not a borrowed carrier fallback"
            );
            let prepared_function = named(&prepared_plan, name);
            let graph = carrier(prepared_function);
            let [iteration] = graph.iterations() else {
                panic!("one prepared For iteration row")
            };
            assert_eq!(
                (iteration.header, iteration.body, iteration.exit),
                (original.0, original.1, original.2)
            );
            assert_eq!(iteration.binders, original.3);
            let cursor = iteration
                .cursor
                .expect("the exact canonical sequence cursor survives in the carrier plan");
            let current = function(&prepared, name);
            assert_eq!(current.local(cursor).unwrap().ty, TypeInterner::INT64);
            assert_eq!(
                child(graph, graph.slots()[iteration.source.index()].shape()).ty(),
                original.4
            );
            let gets = current.blocks[usize::try_from(iteration.body.index()).unwrap()]
                .statements
                .iter()
                .filter_map(|statement| match &statement.kind {
                    StatementKind::SequenceGet {
                        source,
                        index,
                        target,
                        part,
                        ..
                    } => Some((source, index, target, part)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(gets.len(), iteration.binders.len());
            for ((source, index, target, _), binder) in gets.iter().zip(&iteration.binders) {
                assert_eq!(**index, cursor);
                assert_eq!(**target, *binder);
                assert_eq!(source.ty(current), Some(original.4));
            }
            if name == "consuming_absent_map" {
                assert_eq!(*gets[0].3, SequencePart::Key);
                assert_eq!(*gets[1].3, SequencePart::Value);
                assert_eq!(
                    current.local(iteration.binders[0]).unwrap().ty,
                    TypeInterner::STRING
                );
            } else {
                assert_eq!(*gets[0].3, SequencePart::Element);
            }
            assert!(
                !graph.operations().iter().any(|operation| matches!(
                    operation.edge(),
                    Some(ResourceCarrierEdge::IterationBody { .. })
                )),
                "prepared extraction runs at canonical SequenceGet statements"
            );
            assert!(
                graph
                    .operations()
                    .iter()
                    .any(|operation| matches!(operation.role(),
                        ResourceCarrierOperationRole::AdaptSum { path, loan: None, .. }
                            if matches!(path.as_slice(), [ResourceCarrierProjectionPath::Element {
                                index: ResourceCarrierIndex::Local(actual)
                            }] | [ResourceCarrierProjectionPath::MapValue {
                                index: ResourceCarrierIndex::Local(actual)
                            }] if *actual == cursor)
                    ))
            );
            let binder = *iteration.binders.last().unwrap();
            assert_leaf_binder(
                raw_function,
                function(&raw, name),
                *raw_iteration.binders.last().unwrap(),
            );
            assert_leaf_binder(prepared_function, current, binder);
            assert_optional_view_call(raw_function, helper);
            assert_optional_view_call(prepared_function, helper);
            assert!(prepared_function.loans().iter().all(|loan| !matches!(
                loan.source(),
                ResourceLoanSource::CarrierSumProjection { .. }
            )));
        }
    }
}

#[test]
fn resource_carrier_source17_borrowed_fallback_adapts_a_parent_bound_sum_loan() {
    for release in [false, true] {
        let (checked, raw) = consuming_program(release);
        let types = &checked.checked().interner;
        for prepare in [false, true] {
            let mut program = raw.clone();
            if prepare {
                crate::prepare_native_sequences(&mut program, types);
            }
            let plan = validate_resource_ownership(&program, types).unwrap();
            let function = named(&plan, "borrowed_absent_fallback");
            let graph = carrier(function);
            let owner_type = self::function(&program, "absent_envelope").return_type;
            let projected = function
                .loans()
                .iter()
                .filter_map(|loan| match loan.source() {
                    ResourceLoanSource::CarrierSumProjection { operation } => {
                        Some((loan, operation))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let [(loan, operation)] = projected.as_slice() else {
                panic!("one exact borrowed fallback adapter")
            };
            let adapter = &graph.operations()[operation.index()];
            let ResourceCarrierOperationRole::AdaptSum {
                source,
                path,
                loan: adapted,
                lease_frame,
                ..
            } = adapter.role()
            else {
                panic!("normal leaf-sum loan is issued only by its carrier projection operation")
            };
            assert_eq!(*adapted, Some(loan.id()));
            assert_eq!(*lease_frame, loan.frame());
            assert_eq!(adapter.frame(), loan.frame());
            assert!(
                matches!(path.as_slice(), [ResourceCarrierProjectionPath::Field { owner, field }]
                if *owner == owner_type && field.index() == 1)
            );
            assert_eq!(
                child(graph, graph.loans()[source.index()].shape()).ty(),
                owner_type
            );
            let helper = self::function(&program, "source17_absent_optional").id;
            assert!(function.operations().iter().any(|operation| matches!(operation.role(),
                ResourceOperationRole::InvokeSourceFunction { function: callee, operands, .. }
                    if *callee == helper && matches!(operands.as_slice(),
                        [ResourceCallOperand::Borrowed { parameter: 0, loan: actual }] if *actual == loan.id())
            )));
        }
    }
}

#[test]
fn resource_carrier_source17_vacant_match_extracts_exact_ordinary_and_carrier_binders_on_the_selected_edge()
 {
    for release in [false, true] {
        let (checked, raw) = consuming_program(release);
        let types = &checked.checked().interner;
        for prepare in [false, true] {
            let mut program = raw.clone();
            if prepare {
                crate::prepare_native_sequences(&mut program, types);
            }
            let plan = validate_resource_ownership(&program, types).unwrap();
            let mir = function(&program, "matching_absent_choice");
            let (header, choice_type, variant, target, bindings) = mir
                .blocks
                .iter()
                .find_map(|block| {
                    let TerminatorKind::Switch {
                        scrutinee,
                        variants,
                        ..
                    } = &block.terminator.kind
                    else {
                        return None;
                    };
                    variants
                        .iter()
                        .find(|(variant, _, _)| variant.index() == 1)
                        .map(|(variant, target, bindings)| {
                            (block.id, scrutinee.ty, *variant, *target, bindings)
                        })
                })
                .unwrap();
            let [marker, tokens] = bindings.as_slice() else {
                panic!("vacant's two original typed binders")
            };
            assert_eq!(mir.local(*marker).unwrap().ty, TypeInterner::INT64);
            assert!(matches!(
                types.resolve(mir.local(*tokens).unwrap().ty),
                Type::List(_)
            ));
            let graph = carrier(named(&plan, "matching_absent_choice"));
            let edge = ResourceCarrierEdge::SwitchVariant {
                source: header,
                variant,
                target,
            };
            assert!(graph.operations_on_edge(edge).any(|operation| matches!(operation.role(),
                ResourceCarrierOperationRole::ExtractOrdinary { path, target: actual, ty, .. }
                    if *actual == *marker && *ty == TypeInterner::INT64
                        && matches!(path.as_slice(), [ResourceCarrierProjectionPath::Variant { owner, variant: selected, field: 0 }]
                            if *owner == choice_type && *selected == variant)
            )));
            assert!(graph.operations_on_edge(edge).any(|operation| matches!(operation.role(),
                ResourceCarrierOperationRole::Extract { path, destination, .. }
                    if matches!(path.as_slice(), [ResourceCarrierProjectionPath::Variant { owner, variant: selected, field: 1 }]
                        if *owner == choice_type && *selected == variant)
                        && matches!(graph.slots()[destination.index()].storage(),
                            ResourceSlotStorage::Local { header } if header == mir.local(*tokens).unwrap())
            )), "vacant tokens are a typed owned carrier binder even when their selected list is empty");
        }
    }
}

#[test]
fn resource_carrier_prepared_consuming_sequence_mutations_cannot_reuse_the_original_transport_seal()
{
    for release in [false, true] {
        let (checked, mut program) = consuming_program(release);
        let types = &checked.checked().interner;
        validate_resource_ownership(&program, types).unwrap();
        crate::prepare_native_sequences(&mut program, types);
        validate_resource_ownership(&program, types).unwrap();
        for mutation in 0..5 {
            let mut changed = program.clone();
            let function = function_mut(&mut changed, "consuming_absent_map");
            let original_source = function
                .locals
                .iter()
                .find(|local| local.name == "values")
                .unwrap()
                .id;
            let count = function
                .locals
                .iter()
                .find(|local| local.name == "absent_count")
                .unwrap()
                .id;
            let body = function
                .blocks
                .iter_mut()
                .find(|block| {
                    block.statements.iter().any(|statement| {
                        matches!(
                            statement.kind,
                            StatementKind::SequenceGet {
                                part: SequencePart::Value,
                                ..
                            }
                        )
                    })
                })
                .unwrap();
            let key_position = body
                .statements
                .iter()
                .position(|statement| {
                    matches!(
                        statement.kind,
                        StatementKind::SequenceGet {
                            part: SequencePart::Key,
                            ..
                        }
                    )
                })
                .unwrap();
            let value_position = body
                .statements
                .iter()
                .position(|statement| {
                    matches!(
                        statement.kind,
                        StatementKind::SequenceGet {
                            part: SequencePart::Value,
                            ..
                        }
                    )
                })
                .unwrap();
            if mutation == 2 {
                body.statements.swap(key_position, value_position);
            } else {
                let key_target = match body.statements[key_position].kind {
                    StatementKind::SequenceGet { target, .. } => target,
                    _ => unreachable!(),
                };
                let StatementKind::SequenceGet {
                    consume,
                    source,
                    index,
                    target,
                    ..
                } = &mut body.statements[value_position].kind
                else {
                    unreachable!()
                };
                match mutation {
                    0 => *index = count,
                    1 => *source = SequenceSource::Local(original_source),
                    3 => *consume = false,
                    4 => *target = key_target,
                    _ => unreachable!(),
                }
            }
            assert!(
                validate_resource_ownership(&changed, types).is_err(),
                "prepared index/source/key-value order/consume/binder mutation {mutation} cannot issue a new carrier proof"
            );
        }
    }
}

#[test]
fn resource_carrier_whole_source17_preserves_all_exported_constructor_shapes() {
    for release in [false, true] {
        let checked = checked_support(SOURCE, SUPPORT, release);
        let types = &checked.checked().interner;
        let program = lowered(&checked);
        crate::validate(&program).unwrap();
        let plan = validate_resource_ownership(&program, types).unwrap();
        for name in CONSTRUCTORS {
            let function = named(&plan, name);
            let graph = carrier(function);
            let shape = result_shape(function);
            assert_eq!(shape.ty(), self::function(&program, name).return_type);
            assert!(
                shape.contains_resource(),
                "empty values retain the type of {name}"
            );
            assert!(
                graph.operations().iter().any(|operation| matches!(
                    operation.role(),
                    ResourceCarrierOperationRole::PublishReturn { .. }
                )),
                "{name} publishes an exact typed return root"
            );
        }
        // This unused public view formal must not disappear because the selected
        // constructor examples have no occupied children. Required controls also
        // remain in the source family after real required-value materialization.
        function(&program, "refined_shape_contract");
        function(&program, "absent_required_controls");
        let first = carrier(named(&plan, "empty_tokens"));
        let second = carrier(named(&plan, "absent_exact_state"));
        for shape in first.shapes() {
            let same = second.shape_for_type(shape.ty()).unwrap();
            assert_eq!(shape.id(), same.id());
            assert_eq!(shape.node(), same.node());
            assert_eq!(shape.contains_resource(), same.contains_resource());
        }
    }
}

#[test]
fn resource_carrier_source17_collection_edges_keep_resource_and_ordinary_sum_arms() {
    for release in [false, true] {
        let checked = checked_support(SOURCE, SUPPORT, release);
        let program = lowered(&checked);
        let plan = validate_resource_ownership(&program, &checked.checked().interner).unwrap();
        let empty = named(&plan, "empty_tokens");
        let ResourceCarrierNode::List { element } = result_shape(empty).node() else {
            panic!("empty_tokens keeps list[TestHandle]");
        };
        let ResourceCarrierNode::Resource { kind } = child(carrier(empty), *element).node() else {
            panic!("empty list retains the original Resource leaf kind");
        };
        assert_eq!(
            Some(kind.clone()),
            program.resource_manifest.kind_for_type(kind.ty())
        );
        let resource_ty = kind.ty();
        let absent = named(&plan, "absent_tokens");
        let ResourceCarrierNode::List { element } = result_shape(absent).node() else {
            panic!("absent_tokens keeps list[optional[TestHandle]]");
        };
        let ResourceCarrierNode::Optional { payload } = child(carrier(absent), *element).node()
        else {
            panic!("two None elements retain Optional layout");
        };
        assert_eq!(child(carrier(absent), *payload).ty(), resource_ty);
        let absent_map = named(&plan, "absent_map");
        let ResourceCarrierNode::Map { key, value } = result_shape(absent_map).node() else {
            panic!("absent_map keeps both key and value types");
        };
        assert_eq!(child(carrier(absent_map), *key).ty(), TypeInterner::STRING);
        assert!(!child(carrier(absent_map), *key).contains_resource());
        assert!(matches!(
            child(carrier(absent_map), *value).node(),
            ResourceCarrierNode::Optional { .. }
        ));
        for name in ["empty_result_tokens", "failed_result_tokens"] {
            let result = named(&plan, name);
            let ResourceCarrierNode::Result { success, failure } = result_shape(result).node()
            else {
                panic!("both selected result arms retain one result type");
            };
            assert!(matches!(
                child(carrier(result), *success).node(),
                ResourceCarrierNode::List { .. }
            ));
            assert!(child(carrier(result), *success).contains_resource());
            assert_eq!(child(carrier(result), *failure).ty(), TypeInterner::STRING);
            assert!(!child(carrier(result), *failure).contains_resource());
        }
        assert_eq!(
            result_shape(named(&plan, "empty_result_tokens")).id(),
            result_shape(named(&plan, "failed_result_tokens")).id()
        );
    }
}

#[test]
fn resource_carrier_source17_nominal_generic_inactive_and_refined_layouts_are_complete() {
    for release in [false, true] {
        let checked = checked_support(SOURCE, SUPPORT, release);
        let types = &checked.checked().interner;
        let program = lowered(&checked);
        let plan = validate_resource_ownership(&program, types).unwrap();
        let envelope = named(&plan, "absent_envelope");
        let Type::Struct(expected_owner) = types.resolve(envelope.return_type()) else {
            panic!("checked TokenEnvelope nominal owner");
        };
        let ResourceCarrierNode::Struct {
            owner,
            arguments,
            fields,
        } = result_shape(envelope).node()
        else {
            panic!("TokenEnvelope is not erased into an ordinary record");
        };
        assert_eq!(owner, expected_owner);
        assert!(arguments.is_empty());
        assert_eq!(
            fields
                .iter()
                .map(|field| (field.ordinal, field.name.as_str()))
                .collect::<Vec<_>>(),
            [(0, "tokens"), (1, "fallback"), (2, "marker")]
        );
        assert!(child(carrier(envelope), fields[0].shape).contains_resource());
        assert!(child(carrier(envelope), fields[1].shape).contains_resource());
        assert_eq!(
            child(carrier(envelope), fields[2].shape).ty(),
            TypeInterner::INT64
        );
        let boxed = named(&plan, "absent_box");
        let ResourceCarrierNode::Struct {
            arguments, fields, ..
        } = result_shape(boxed).node()
        else {
            panic!("concrete TokenBox[TestHandle] layout");
        };
        assert_eq!(arguments.len(), 1);
        assert!(matches!(
            child(carrier(boxed), arguments[0]).node(),
            ResourceCarrierNode::Resource { .. }
        ));
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, "items");
        assert!(matches!(
            child(carrier(boxed), fields[0].shape).node(),
            ResourceCarrierNode::List { .. }
        ));
        let choice = named(&plan, "empty_choice");
        let ResourceCarrierNode::Enum {
            owner, variants, ..
        } = result_shape(choice).node()
        else {
            panic!("empty TokenChoice retains every variant");
        };
        let Type::Enum(expected_owner) = types.resolve(choice.return_type()) else {
            panic!("checked TokenChoice nominal owner");
        };
        assert_eq!(owner, expected_owner);
        assert_eq!(
            variants
                .iter()
                .map(|variant| (variant.ordinal, variant.name.as_str()))
                .collect::<Vec<_>>(),
            [(0, "empty"), (1, "vacant"), (2, "occupied")]
        );
        assert_eq!(
            variants
                .iter()
                .map(|variant| variant.discriminant)
                .collect::<Vec<_>>(),
            [0, 1, 2]
        );
        assert!(variants[0].fields.is_empty());
        assert_eq!(variants[1].fields.len(), 2);
        assert_eq!(variants[1].fields[0].name, "marker");
        assert_eq!(variants[1].fields[1].name, "tokens");
        assert!(child(carrier(choice), variants[1].fields[1].shape).contains_resource());
        assert!(matches!(
            child(carrier(choice), variants[2].fields[0].shape).node(),
            ResourceCarrierNode::Resource { .. }
        ));
        let state = named(&plan, "absent_state");
        let ResourceCarrierNode::Machine {
            states,
            transitions,
            ..
        } = result_shape(state).node()
        else {
            panic!("unqualified TokenState retains every state");
        };
        assert_eq!(
            states
                .iter()
                .map(|state| (state.ordinal, state.name.as_str()))
                .collect::<Vec<_>>(),
            [(0, "vacant"), (1, "waiting"), (2, "occupied")]
        );
        assert_eq!(transitions, &[(0, 1), (0, 2)]);
        assert_eq!(states[0].fields[0].name, "fallback");
        assert_eq!(states[0].fields[1].name, "marker");
        assert!(matches!(
            child(carrier(state), states[2].fields[0].shape).node(),
            ResourceCarrierNode::Resource { .. }
        ));
        let exact = named(&plan, "absent_exact_state");
        let ResourceCarrierNode::MachineState {
            machine,
            state: qualified,
        } = result_shape(exact).node()
        else {
            panic!("TokenState at vacant retains its qualification");
        };
        assert_eq!(*qualified, 0);
        assert_eq!(child(carrier(exact), *machine).ty(), state.return_type());
        assert_ne!(exact.return_type(), state.return_type());
        let refined = named(&plan, "refined_shape_contract");
        assert_eq!(refined.parameters()[0].mode, ParamMode::View);
        let graph = carrier(refined);
        let shape = graph.shape_for_type(refined.parameters()[0].ty).unwrap();
        let ResourceCarrierNode::Struct { fields, .. } = shape.node() else {
            panic!("unused RefinedEnvelope view formal retains its original fields");
        };
        assert_eq!(
            fields
                .iter()
                .map(|field| field.name.as_str())
                .collect::<Vec<_>>(),
            ["marker", "tokens"]
        );
        let ResourceCarrierNode::Refinement { name, base } = child(graph, fields[0].shape).node()
        else {
            panic!("PositiveMarker is not widened to int64");
        };
        assert!(name.ends_with("PositiveMarker"));
        assert_eq!(child(graph, *base).ty(), TypeInterner::INT64);
        assert!(!child(graph, fields[0].shape).contains_resource());
        let predicate = child(graph, fields[0].shape)
            .refinement_predicate()
            .expect("the refinement graph retains its original checked predicate body");
        assert!(hir::refinement_predicate_declaration_matches(
            &predicate.identity.declaration,
            name
        ));
        assert_eq!(predicate.return_type, TypeInterner::BOOL);
        assert_eq!(predicate.params.len(), 1);
        assert_eq!(predicate.params[0].ty, TypeInterner::INT64);
        let archived = function(&program, "refined_shape_contract")
            .resource_lowering
            .as_ref()
            .unwrap()
            .source
            .execution_functions()
            .iter()
            .find(|function| function.id == predicate.id)
            .unwrap();
        assert!(crate::breakpoint_regions::hir_blocks_equal(
            &predicate.body,
            &archived.body
        ));
        assert_eq!(predicate.locals, archived.locals);
        assert!(child(graph, fields[1].shape).contains_resource());
        assert!(graph.loans().iter().any(|loan| matches!(
            loan.source(),
            ResourceCarrierLoanSource::IncomingViewFormal { parameter: 0, .. }
        )));
        assert!(
            !graph.operations().iter().any(|operation| matches!(
                operation.role(),
                ResourceCarrierOperationRole::BeginConstructor { .. }
            )),
            "an unused view formal is metadata plus a bounded lease, never a minted owner"
        );
    }
}

#[test]
fn resource_carrier_constructor_children_keep_lexical_order_and_exact_current_occurrences() {
    for release in [false, true] {
        let checked = checked_support(SOURCE, SUPPORT, release);
        let program = lowered(&checked);
        let plan = validate_resource_ownership(&program, &checked.checked().interner).unwrap();
        let envelope = named(&plan, "absent_envelope");
        let graph = carrier(envelope);
        let destination = graph
            .operations()
            .iter()
            .find_map(|operation| match operation.role() {
                ResourceCarrierOperationRole::BeginConstructor {
                    destination,
                    constructor: ResourceCarrierConstructor::Struct,
                } => Some(*destination),
                _ => None,
            })
            .unwrap();
        let children = graph
            .operations()
            .iter()
            .filter_map(|operation| match operation.role() {
                ResourceCarrierOperationRole::ConstructorChild {
                    destination: actual,
                    child,
                } if *actual == destination => Some(child.index),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(children, [0, 1, 2]);
        let began = graph.operations().iter().position(|operation| matches!(operation.role(),
            ResourceCarrierOperationRole::BeginConstructor { destination: actual, .. } if *actual == destination
        )).unwrap();
        let committed = graph.operations().iter().position(|operation| matches!(operation.role(),
            ResourceCarrierOperationRole::CommitConstructor { destination: actual } if *actual == destination
        )).unwrap();
        assert!(began < committed);
        let expression = constructor(function(&program, "absent_envelope"), |value| {
            matches!(value.kind, hir::ExpressionKind::StructConstruct { .. })
        })
        .unwrap();
        assert!(graph.operations_for_expression(expression).any(|operation| matches!(
            operation.role(), ResourceCarrierOperationRole::CommitConstructor { destination: actual }
                if *actual == destination
        )));
        let copied = (*expression).clone();
        assert_eq!(
            graph.operations_for_expression(&copied).count(),
            0,
            "a copied public expression has no constructor-issued current occurrence"
        );
        for operation in graph.operations() {
            assert!(
                envelope
                    .frames()
                    .iter()
                    .any(|frame| frame.id() == operation.frame())
            );
            assert_eq!(operation.site().function(), envelope.function());
        }
    }
}

#[test]
fn resource_carrier_source_call_owned_view_and_ordinary_projection_keep_original_formal_tuples() {
    const SUFFIX: &str = r#"
function forward_envelope(value: TokenEnvelope) returns TokenEnvelope:
    return value
function inspect_envelope(view value: TokenEnvelope) returns int64:
    return value.marker
export function carrier_transport() returns int64:
    TokenEnvelope initial = absent_envelope()
    int64 marker = inspect_envelope(view initial)
    TokenEnvelope forwarded = forward_envelope(initial)
    int64 second = inspect_envelope(view forwarded)
    return marker + second
"#;
    for release in [false, true] {
        let source = format!("{SOURCE}{SUFFIX}");
        assert!(source.starts_with(SOURCE));
        let checked = checked_support(&source, SUPPORT, release);
        let program = lowered(&checked);
        let plan = validate_resource_ownership(&program, &checked.checked().interner).unwrap();
        let scenario = named(&plan, "carrier_transport");
        let owning = function(&program, "forward_envelope").id;
        let viewing = function(&program, "inspect_envelope").id;
        let envelope_type = function(&program, "absent_envelope").return_type;
        let owning_call = scenario
            .operations()
            .iter()
            .find_map(|operation| match operation.role() {
                ResourceOperationRole::InvokeSourceFunction {
                    function,
                    source,
                    formals,
                    operands,
                    result,
                    ..
                } if *function == owning => Some((source, formals, operands, result)),
                _ => None,
            })
            .unwrap();
        assert_eq!(owning_call.0.arguments.len(), 1);
        assert_eq!(owning_call.1.len(), 1);
        assert_eq!(owning_call.1[0].parameter(), 0);
        assert_eq!(owning_call.1[0].source_index(), 0);
        assert_eq!(owning_call.1[0].actual_type(), envelope_type);
        assert_eq!(owning_call.1[0].parameter_type(), envelope_type);
        assert_eq!(owning_call.1[0].access(), ParamMode::Owned);
        assert_eq!(owning_call.1[0].syntax(), ResourceArgumentSyntax::Bare);
        assert_eq!(
            owning_call.1[0].effect(),
            ResourceArgumentEffect::TransferOwned
        );
        assert!(matches!(
            owning_call.2.as_slice(),
            [ResourceCallOperand::CarrierOwned { parameter: 0, .. }]
        ));
        assert!(matches!(owning_call.3, ResourceCallResult::Carrier { .. }));
        let view_calls = scenario
            .operations()
            .iter()
            .filter_map(|operation| match operation.role() {
                ResourceOperationRole::InvokeSourceFunction {
                    function,
                    source,
                    formals,
                    operands,
                    ..
                } if *function == viewing => Some((source, formals, operands)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(view_calls.len(), 2);
        for (source, formals, operands) in view_calls {
            assert_eq!(source.arguments.len(), 1);
            assert_eq!(
                source.arguments[0].effect,
                jett_typecheck::CheckedCallerEffect::RetainBorrow
            );
            assert_eq!(formals.len(), 1);
            assert_eq!(formals[0].parameter(), 0);
            assert_eq!(formals[0].source_index(), 0);
            assert_eq!(formals[0].actual_type(), envelope_type);
            assert_eq!(formals[0].parameter_type(), envelope_type);
            assert_eq!(formals[0].access(), ParamMode::View);
            assert_eq!(formals[0].syntax(), ResourceArgumentSyntax::WrittenView);
            assert_eq!(formals[0].effect(), ResourceArgumentEffect::RetainBorrow);
            assert!(matches!(
                operands.as_slice(),
                [ResourceCallOperand::CarrierBorrowed { parameter: 0, .. }]
            ));
        }
        let inspected = named(&plan, "inspect_envelope");
        let graph = carrier(inspected);
        let owner = inspected.parameters()[0].ty;
        assert!(graph.loans().iter().any(|loan| matches!(
            loan.source(),
            ResourceCarrierLoanSource::IncomingViewFormal { parameter: 0, .. }
        )));
        assert!(graph.operations().iter().any(|operation| matches!(operation.role(),
            ResourceCarrierOperationRole::Observe {
                observation: ResourceCarrierObservation::Ordinary { path, ty }, ..
            } if *ty == TypeInterner::INT64
                && matches!(path.as_slice(), [ResourceCarrierProjectionPath::Field { owner: actual, field }]
                    if *actual == owner && field.index() == 2)
        )), "marker observation retains its nominal parent and exact field ordinal");
        assert!(
            carrier(named(&plan, "forward_envelope"))
                .operations()
                .iter()
                .any(|operation| matches!(
                    operation.role(),
                    ResourceCarrierOperationRole::PublishReturn { .. }
                ))
        );
    }
}

fn constructor(function: &Function, wanted: impl Fn(&Expression) -> bool) -> Option<&Expression> {
    for block in &function.blocks {
        for statement in &block.statements {
            if let StatementKind::Let { value, .. } | StatementKind::Evaluate(value) =
                &statement.kind
                && let Some(value) = matching_expression(value, &wanted)
            {
                return Some(value);
            }
        }
        if let TerminatorKind::Return(Some(value)) = &block.terminator.kind
            && let Some(value) = matching_expression(value, &wanted)
        {
            return Some(value);
        }
    }
    None
}

fn constructor_mut(
    function: &mut Function,
    wanted: impl Fn(&Expression) -> bool,
) -> &mut Expression {
    for block in &mut function.blocks {
        for statement in &mut block.statements {
            if let StatementKind::Let { value, .. } | StatementKind::Evaluate(value) =
                &mut statement.kind
                && let Some(value) = matching_expression_mut(value, &wanted)
            {
                return value;
            }
        }
        if let TerminatorKind::Return(Some(value)) = &mut block.terminator.kind
            && let Some(value) = matching_expression_mut(value, &wanted)
        {
            return value;
        }
    }
    panic!("exact original constructor occurrence");
}

fn matching_expression<'a>(
    value: &'a Expression,
    wanted: &impl Fn(&Expression) -> bool,
) -> Option<&'a Expression> {
    if wanted(value) {
        return Some(value);
    }
    match &value.kind {
        hir::ExpressionKind::View(inner) | hir::ExpressionKind::RefinementValidated(inner) => {
            matching_expression(inner, wanted)
        }
        hir::ExpressionKind::InterfaceCoerce { value, .. }
        | hir::ExpressionKind::Comptime { value, .. } => matching_expression(value, wanted),
        _ => None,
    }
}

fn matching_expression_mut<'a>(
    value: &'a mut Expression,
    wanted: &impl Fn(&Expression) -> bool,
) -> Option<&'a mut Expression> {
    if wanted(value) {
        return Some(value);
    }
    match &mut value.kind {
        hir::ExpressionKind::View(inner) | hir::ExpressionKind::RefinementValidated(inner) => {
            matching_expression_mut(inner, wanted)
        }
        hir::ExpressionKind::InterfaceCoerce { value, .. }
        | hir::ExpressionKind::Comptime { value, .. } => matching_expression_mut(value, wanted),
        _ => None,
    }
}

#[test]
fn resource_carrier_public_mir_field_tag_type_return_and_local_changes_cannot_issue_transport() {
    for release in [false, true] {
        let checked = checked_support(SOURCE, SUPPORT, release);
        let types = &checked.checked().interner;
        let program = lowered(&checked);
        validate_resource_ownership(&program, types).unwrap();
        for mutation in 0..8 {
            let mut changed = program.clone();
            match mutation {
                0 | 1 | 2 => {
                    let other_type = function(&changed, "absent_box").return_type;
                    let value =
                        constructor_mut(function_mut(&mut changed, "absent_envelope"), |value| {
                            matches!(value.kind, hir::ExpressionKind::StructConstruct { .. })
                        });
                    let hir::ExpressionKind::StructConstruct {
                        struct_type,
                        fields,
                        evaluation_order,
                        ..
                    } = &mut value.kind
                    else {
                        unreachable!()
                    };
                    match mutation {
                        0 => fields.swap(0, 1),
                        1 => evaluation_order.swap(0, 2),
                        2 => *struct_type = other_type,
                        _ => unreachable!(),
                    }
                }
                3 => {
                    let value =
                        constructor_mut(function_mut(&mut changed, "absent_choice"), |value| {
                            matches!(value.kind, hir::ExpressionKind::EnumConstruct { .. })
                        });
                    let hir::ExpressionKind::EnumConstruct { variant, .. } = &mut value.kind else {
                        unreachable!()
                    };
                    *variant = hir::VariantId::new(2);
                }
                4 => {
                    let value =
                        constructor_mut(function_mut(&mut changed, "absent_state"), |value| {
                            matches!(value.kind, hir::ExpressionKind::MachineConstruct { .. })
                        });
                    let hir::ExpressionKind::MachineConstruct { state, .. } = &mut value.kind
                    else {
                        unreachable!()
                    };
                    *state = hir::StateId::new(1);
                }
                5 => {
                    let other_type = function(&changed, "absent_tokens").return_type;
                    function_mut(&mut changed, "empty_tokens").return_type = other_type;
                }
                6 => {
                    let other_type = function(&changed, "absent_envelope").return_type;
                    let function = function_mut(&mut changed, "refined_shape_contract");
                    let local = function.params[0].local;
                    function.locals[usize::try_from(local.index()).unwrap()].ty = other_type;
                }
                7 => {
                    let other_type = function(&changed, "empty_map").return_type;
                    let value =
                        constructor_mut(function_mut(&mut changed, "absent_map"), |value| {
                            matches!(value.kind, hir::ExpressionKind::MapConstruct { .. })
                        });
                    value.ty = other_type;
                }
                _ => unreachable!(),
            }
            assert!(
                validate_witnesses(&changed, types).is_err(),
                "original constructor witness, mutation {mutation}"
            );
            assert!(
                validate_resource_ownership(&changed, types).is_err(),
                "no carrier authority, mutation {mutation}"
            );
        }
    }
}

#[test]
fn resource_carrier_foreign_checked_body_and_copied_witness_cannot_replace_the_current_archive() {
    for release in [false, true] {
        let checked = checked_support(SOURCE, SUPPORT, release);
        let foreign = checked_support(
            &SOURCE.replace("marker: 17", "marker: 19"),
            SUPPORT,
            release,
        );
        let types = &checked.checked().interner;
        let program = lowered(&checked);
        let foreign_program = lowered(&foreign);
        validate_resource_ownership(&program, types).unwrap();
        validate_resource_ownership(&foreign_program, &foreign.checked().interner).unwrap();
        let foreign_function = function(&foreign_program, "absent_envelope");
        for mutation in 0..3 {
            let mut changed = program.clone();
            let function = function_mut(&mut changed, "absent_envelope");
            match mutation {
                0 => function.blocks = foreign_function.blocks.clone(),
                1 => function.resource_lowering = foreign_function.resource_lowering.clone(),
                2 => *function = foreign_function.clone(),
                _ => unreachable!(),
            }
            assert!(
                validate_resource_ownership(&changed, types).is_err(),
                "foreign body/witness {mutation}"
            );
        }
    }
}

fn copied_types(original: &TypeInterner) -> TypeInterner {
    let mut types = TypeInterner::new();
    macro_rules! copy_nominals {
        ($variant:ident, $resolve:ident, $add:ident) => {
            let definitions = original
                .type_ids()
                .filter_map(|ty| match original.resolve(ty) {
                    Type::$variant(id) => Some((id.index(), (*id, original.$resolve(*id).clone()))),
                    _ => None,
                })
                .collect::<std::collections::BTreeMap<_, _>>();
            for (id, definition) in definitions.into_values() {
                assert_eq!(types.$add(definition), id);
            }
        };
    }
    copy_nominals!(Struct, resolve_struct, add_struct);
    copy_nominals!(Bitfield, resolve_bitfield, add_bitfield);
    copy_nominals!(Enum, resolve_enum, add_enum);
    copy_nominals!(Interface, resolve_interface, add_interface);
    copy_nominals!(Actor, resolve_actor, add_actor);
    copy_nominals!(Machine, resolve_machine, add_machine);
    for ty in original.type_ids() {
        assert_eq!(types.intern(original.resolve(ty).clone()), ty);
    }
    for ty in original.type_ids() {
        let arguments = original.nominal_type_arguments(ty);
        if !arguments.is_empty() {
            types
                .register_nominal_type_arguments(ty, arguments.to_vec())
                .unwrap();
        }
    }
    types
}

#[test]
fn resource_carrier_stored_nominal_definitions_cannot_erase_latent_payloads_or_refined_fields() {
    for release in [false, true] {
        let checked = checked_support(SOURCE, SUPPORT, release);
        let original_types = &checked.checked().interner;
        let hir = materialized(&checked);
        let program = lower(&hir, original_types).unwrap();
        let mut types = copied_types(original_types);
        hir.resource_source.validate_types(&types).unwrap();
        types.intern(Type::Optional(TypeInterner::UINT64));
        hir.resource_source.validate_types(&types).unwrap();
        let Type::Struct(envelope_id) =
            types.resolve(function(&program, "absent_envelope").return_type)
        else {
            unreachable!()
        };
        let envelope_id = *envelope_id;
        let envelope = types.resolve_struct(envelope_id).clone();
        let mut erased = envelope.clone();
        erased.fields[0].1 = TypeInterner::STRING;
        types.update_struct(envelope_id, erased);
        assert!(hir.resource_source.validate_types(&types).is_err());
        assert!(validate_resource_ownership(&program, &types).is_err());
        types.update_struct(envelope_id, envelope);
        let Type::Enum(choice_id) = types.resolve(function(&program, "empty_choice").return_type)
        else {
            unreachable!()
        };
        let choice_id = *choice_id;
        let choice = types.resolve_enum(choice_id).clone();
        let mut erased = choice.clone();
        erased.variants[2].fields.clear();
        types.update_enum(choice_id, erased);
        assert!(
            hir.resource_source.validate_types(&types).is_err(),
            "inactive occupied variant remains archived"
        );
        assert!(validate_resource_ownership(&program, &types).is_err());
        types.update_enum(choice_id, choice);
        let Type::Machine(machine_id) =
            types.resolve(function(&program, "absent_state").return_type)
        else {
            unreachable!()
        };
        let machine_id = *machine_id;
        let machine = types.resolve_machine(machine_id).clone();
        let mut erased = machine.clone();
        erased.states[2].fields.clear();
        types.update_machine(machine_id, erased);
        assert!(
            hir.resource_source.validate_types(&types).is_err(),
            "inactive occupied machine state remains archived"
        );
        types.update_machine(machine_id, machine);
        let refined = function(&program, "refined_shape_contract");
        let Type::Struct(refined_id) = types.resolve(refined.params[0].ty) else {
            unreachable!()
        };
        let refined_id = *refined_id;
        let mut erased = types.resolve_struct(refined_id).clone();
        erased.fields[0].1 = TypeInterner::INT64;
        types.update_struct(refined_id, erased);
        assert!(
            hir.resource_source.validate_types(&types).is_err(),
            "the unused formal does not erase PositiveMarker"
        );
        assert!(validate_resource_ownership(&program, &types).is_err());
    }
}

const MACHINE_OBSERVATION_SUFFIX: &str = r#"
function source17_machine_absent_optional(view candidate: optional[resource_probe.TestHandle]) returns bool:
    resource_probe.TestHandle token = view((view candidate) handle:
        return true
    )
    return false
export function observe_general_machine() returns bool:
    TokenState value = absent_state()
    if value at vacant:
        if value.marker != "empty":
            return false
        return source17_machine_absent_optional(view value.fallback)
    return false
export function observe_exact_machine() returns bool:
    TokenState at vacant value = absent_exact_state()
    if value at vacant:
        if value.marker != "empty":
            return false
        return source17_machine_absent_optional(view value.fallback)
    return false
export function observe_machine_view(view value: TokenState) returns bool:
    if value at vacant:
        if value.marker != "empty":
            return false
        return source17_machine_absent_optional(view value.fallback)
    return false
"#;

fn machine_observation_program(release: bool) -> (Arc<CheckedResourceProgram>, Program) {
    let source = format!("{SOURCE}{MACHINE_OBSERVATION_SUFFIX}");
    assert!(source.starts_with(SOURCE));
    let checked = checked_support(&source, SUPPORT, release);
    let program = lowered(&checked);
    (checked, program)
}

#[test]
fn resource_carrier_machine_state_observation_preserves_physical_root_and_guarded_field_paths() {
    for release in [false, true] {
        let (checked, raw) = machine_observation_program(release);
        let types = &checked.checked().interner;
        for prepare in [false, true] {
            let mut program = raw.clone();
            if prepare {
                crate::prepare_native_sequences(&mut program, types);
            }
            crate::validate(&program).unwrap();
            let plan = validate_resource_ownership(&program, types).unwrap();
            for name in [
                "observe_general_machine",
                "observe_exact_machine",
                "observe_machine_view",
            ] {
                let mir = function(&program, name);
                let local = mir
                    .locals
                    .iter()
                    .find(|local| local.name == "value")
                    .unwrap();
                let graph = carrier(named(&plan, name));
                let state_observations = graph
                    .operations()
                    .iter()
                    .filter_map(|operation| match operation.role() {
                        ResourceCarrierOperationRole::Observe {
                            source,
                            observation: ResourceCarrierObservation::State { state: 0 },
                            ..
                        } => Some((*source, operation)),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let [(source, operation)] = state_observations.as_slice() else {
                    panic!("one exact vacant StateIs observation for {name}");
                };
                assert_eq!(
                    child(graph, graph.loans()[source.index()].shape()).ty(),
                    local.ty
                );
                let condition = mir
                    .blocks
                    .iter()
                    .find_map(|block| {
                        let TerminatorKind::Branch { condition, .. } = &block.terminator.kind
                        else {
                            return None;
                        };
                        matches!(condition.kind, hir::ExpressionKind::StateIs { .. })
                            .then_some(condition)
                    })
                    .unwrap();
                assert!(
                    operation.is_for_expression(condition),
                    "the row retains the StateIs occurrence"
                );
                let assert_state_path = |source: super::carriers::ResourceCarrierLoanId,
                                         path: &[ResourceCarrierProjectionPath],
                                         field: usize| {
                    assert_eq!(
                        child(graph, graph.loans()[source.index()].shape()).ty(),
                        local.ty
                    );
                    let [
                        ResourceCarrierProjectionPath::State {
                            owner,
                            state,
                            field: actual,
                        },
                    ] = path
                    else {
                        panic!("machine field uses its checked State path");
                    };
                    assert_eq!((*state, *actual), (0, field));
                    let Type::MachineState { machine, state } = types.resolve(*owner) else {
                        panic!("field path retains its qualified nominal owner");
                    };
                    assert_eq!(state.index(), 0);
                    assert!(
                        matches!(types.resolve(local.ty),
                            Type::Machine(physical) if physical == machine
                        ) || matches!(types.resolve(local.ty),
                            Type::MachineState { machine: physical, state: selected }
                                if physical == machine && selected.index() == 0
                        )
                    );
                };
                let marker = graph
                    .operations()
                    .iter()
                    .find_map(|operation| match operation.role() {
                        ResourceCarrierOperationRole::Observe {
                            source,
                            observation: ResourceCarrierObservation::Ordinary { path, ty },
                            ..
                        } if *ty == TypeInterner::STRING => Some((*source, path)),
                        _ => None,
                    })
                    .unwrap();
                assert_state_path(marker.0, marker.1, 1);
                let adapter = graph
                    .operations()
                    .iter()
                    .find_map(|operation| match operation.role() {
                        ResourceCarrierOperationRole::AdaptSum {
                            source,
                            path,
                            loan: Some(loan),
                            ..
                        } => Some((*source, path, *loan, operation.id())),
                        _ => None,
                    })
                    .unwrap();
                assert_state_path(adapter.0, adapter.1, 0);
                assert!(
                    matches!(named(&plan, name).loans()[adapter.2.index()].source(),
                        ResourceLoanSource::CarrierSumProjection { operation } if operation == adapter.3
                    )
                );
                if name == "observe_machine_view" {
                    let ResourceCarrierLoanSource::Loan(parent) =
                        graph.loans()[source.index()].source()
                    else {
                        panic!("view observation borrows its physical incoming loan");
                    };
                    assert!(matches!(
                        graph.loans()[parent.index()].source(),
                        ResourceCarrierLoanSource::IncomingViewFormal { parameter: 0, .. }
                    ));
                    assert!(
                        !graph.slots().iter().any(|slot| matches!(slot.storage(),
                            ResourceSlotStorage::Local { header } if header.id == local.id
                        )),
                        "a view formal never becomes an owned carrier slot"
                    );
                }
            }
            let mir = function(&program, "observe_general_machine");
            let mut narrowed_local_base = false;
            for block in &mir.blocks {
                walk::mir_block(block, &mut |value| {
                    if let hir::ExpressionKind::Field {
                        base, owner_type, ..
                    } = &value.kind
                    {
                        narrowed_local_base |= base.ty == *owner_type
                            && matches!(types.resolve(base.ty), Type::MachineState { .. });
                    }
                });
            }
            assert!(
                narrowed_local_base,
                "the positive source exercises the checked branch-narrowed place"
            );
            for name in CONSTRUCTORS {
                function(&program, name);
            }
            function(&program, "refined_shape_contract");
            function(&program, "absent_required_controls");
        }
    }
}

fn first_machine_field_mut(value: &mut Expression) -> Option<&mut Expression> {
    if matches!(value.kind, hir::ExpressionKind::Field { .. }) {
        return Some(value);
    }
    match &mut value.kind {
        hir::ExpressionKind::Binary { left, right, .. } => {
            first_machine_field_mut(left).or_else(|| first_machine_field_mut(right))
        }
        hir::ExpressionKind::Unary { value, .. } | hir::ExpressionKind::View(value) => {
            first_machine_field_mut(value)
        }
        _ => None,
    }
}

#[test]
fn resource_carrier_machine_observation_current_guard_and_narrowed_place_mutations_refuse() {
    for release in [false, true] {
        let (checked, program) = machine_observation_program(release);
        let types = &checked.checked().interner;
        validate_resource_ownership(&program, types).unwrap();
        for mutation in 0..5 {
            let mut changed = program.clone();
            let mir = function_mut(&mut changed, "observe_general_machine");
            let local = mir
                .locals
                .iter()
                .find(|local| local.name == "value")
                .unwrap()
                .clone();
            if mutation < 2 {
                let branch = mir
                    .blocks
                    .iter_mut()
                    .find_map(|block| match &mut block.terminator.kind {
                        TerminatorKind::Branch {
                            condition,
                            then_block,
                            else_block,
                        } if matches!(condition.kind, hir::ExpressionKind::StateIs { .. }) => {
                            Some((condition, then_block, else_block))
                        }
                        _ => None,
                    })
                    .unwrap();
                if mutation == 0 {
                    let hir::ExpressionKind::StateIs { state, .. } = &mut branch.0.kind else {
                        unreachable!();
                    };
                    *state = hir::StateId::new(1);
                } else {
                    std::mem::swap(branch.1, branch.2);
                }
            } else {
                let field = mir
                    .blocks
                    .iter_mut()
                    .find_map(|block| {
                        let TerminatorKind::Branch { condition, .. } = &mut block.terminator.kind
                        else {
                            return None;
                        };
                        first_machine_field_mut(condition)
                    })
                    .unwrap();
                let hir::ExpressionKind::Field {
                    base, owner_type, ..
                } = &mut field.kind
                else {
                    unreachable!();
                };
                assert!(matches!(
                    types.resolve(*owner_type),
                    Type::MachineState { .. }
                ));
                match mutation {
                    2 => *owner_type = local.ty,
                    3 => {
                        assert_ne!(base.ty, local.ty);
                        base.ty = local.ty;
                    }
                    4 => {
                        let qualified = *owner_type;
                        mir.locals
                            .iter_mut()
                            .find(|header| header.id == local.id)
                            .unwrap()
                            .ty = qualified;
                    }
                    _ => unreachable!(),
                }
            }
            assert!(
                validate_witnesses(&changed, types).is_err(),
                "machine observational mutation {mutation} cannot replace its authenticated current occurrence"
            );
            assert!(
                validate_resource_ownership(&changed, types).is_err(),
                "machine observational mutation {mutation} cannot issue a fresh carrier permission"
            );
        }
    }
}
