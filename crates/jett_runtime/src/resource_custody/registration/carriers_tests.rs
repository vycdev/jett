//! Registration consistency tests only. Graph rows do not authorize live custody
//! or prove a refinement predicate; those remain compiler/runtime obligations.
use super::schema::WireLayout;
use super::wire::{decode, encode_records};
use super::*;
use crate::ResourceRegistry;

const INTEGER: u32 = 0;
const STRING: u32 = 1;
const NOTHING: u32 = 2;
const RESOURCE_SHAPE: u32 = 4;
const OPTIONAL_SHAPE: u32 = 5;
const RESULT_SHAPE: u32 = 6;
const WHOLE_SHAPE: u32 = 7;
const RESOURCE_NODE: u32 = 3;
const OPTIONAL_NODE: u32 = 4;
const RESULT_NODE: u32 = 5;
const LIST_NODE: u32 = 6;
const MAP_NODE: u32 = 7;
const RECORD_NODE: u32 = 8;
const ENUM_NODE: u32 = 9;
const MACHINE_NODE: u32 = 10;
const STATE_NODE: u32 = 11;
const REFINEMENT_NODE: u32 = 12;
const GENERIC_RESOURCE_NODE: u32 = 13;
const GENERIC_OPTIONAL_NODE: u32 = 14;
const WHOLE_NODE: u32 = 15;
const OTHER_RECORD_NODE: u32 = 16;

fn site(index: u32) -> NativeSite {
    NativeSite {
        function: 0,
        block: 0,
        position: NativePosition::Statement(index),
    }
}

fn field(name: &str, node: u32) -> NativeCarrierField {
    NativeCarrierField {
        name: name.as_bytes().to_vec(),
        node,
    }
}

fn member(name: &str, discriminant: i64, fields: Vec<NativeCarrierField>) -> NativeCarrierMember {
    NativeCarrierMember {
        name: name.as_bytes().to_vec(),
        discriminant,
        fields,
    }
}

fn fixture() -> WireLayout {
    let record_fields = vec![field("marker", 0), field("channel", OPTIONAL_NODE)];
    WireLayout {
        version: 3,
        kinds: vec![0],
        hooks: vec![],
        signatures: vec![NativeSignature {
            ordinal: 0,
            parameters: vec![],
            result: NOTHING,
        }],
        shapes: vec![
            NativeShape::Integer {
                bits: 64,
                signed: true,
            },
            NativeShape::String,
            NativeShape::Nothing,
            NativeShape::Bool,
            NativeShape::Resource { kind: 0 },
            NativeShape::Optional {
                child: RESOURCE_SHAPE,
            },
            NativeShape::Result {
                ok: RESOURCE_SHAPE,
                fail: STRING,
            },
            NativeShape::Carrier { node: WHOLE_NODE },
        ],
        frames: vec![
            NativeFrame {
                ordinal: 0,
                role: NativeFrameRole::Scope,
                site: site(0),
                signature: 0,
                parents: vec![NativeParent::Root],
            },
            NativeFrame {
                ordinal: 1,
                role: NativeFrameRole::Operation,
                site: site(1),
                signature: 0,
                parents: vec![NativeParent::Frame(0)],
            },
        ],
        slots: vec![
            NativeSlot {
                ordinal: 0,
                frame: 1,
                shape: OPTIONAL_SHAPE,
                path: vec![NativePayloadStep::Some],
            },
            NativeSlot {
                ordinal: 1,
                frame: 1,
                shape: RESULT_SHAPE,
                path: vec![NativePayloadStep::Ok],
            },
        ],
        operations: vec![],
        carriers: NativeCarrierLayout {
            nodes: vec![
                NativeCarrierNode::Ordinary { shape: INTEGER },
                NativeCarrierNode::Ordinary { shape: STRING },
                NativeCarrierNode::Ordinary { shape: 3 },
                NativeCarrierNode::Resource { kind: 0 },
                NativeCarrierNode::Optional {
                    child: RESOURCE_NODE,
                },
                NativeCarrierNode::Result {
                    ok: RESOURCE_NODE,
                    fail: 1,
                },
                NativeCarrierNode::List {
                    element: OPTIONAL_NODE,
                },
                NativeCarrierNode::Map {
                    key: 0,
                    value: RESULT_NODE,
                },
                NativeCarrierNode::Struct {
                    identity: b"transport.Record".to_vec(),
                    arguments: vec![],
                    fields: record_fields.clone(),
                },
                NativeCarrierNode::Enum {
                    identity: b"transport.ChannelState".to_vec(),
                    arguments: vec![RESOURCE_NODE],
                    variants: vec![
                        member("Closed", -7, vec![field("marker", 0)]),
                        member("Open", 41, vec![field("channel", RESOURCE_NODE)]),
                    ],
                },
                NativeCarrierNode::Machine {
                    identity: b"transport.ChannelMachine".to_vec(),
                    states: vec![
                        member("Empty", 11, vec![field("marker", 0)]),
                        member("Live", 22, vec![field("channel", RESOURCE_NODE)]),
                    ],
                    transitions: vec![(0, 1), (1, 0)],
                },
                NativeCarrierNode::MachineState {
                    machine: MACHINE_NODE,
                    state: 0,
                },
                NativeCarrierNode::Refinement {
                    identity: b"transport.PositiveMarker".to_vec(),
                    base: 0,
                    predicate: b"value > 0".to_vec(),
                },
                NativeCarrierNode::Struct {
                    identity: b"transport.Cell[transport.Channel]".to_vec(),
                    arguments: vec![RESOURCE_NODE],
                    fields: vec![field("channel", OPTIONAL_NODE)],
                },
                NativeCarrierNode::Struct {
                    identity: b"transport.Cell[optional[transport.Channel]]".to_vec(),
                    arguments: vec![OPTIONAL_NODE],
                    fields: vec![field("channel", OPTIONAL_NODE)],
                },
                NativeCarrierNode::Struct {
                    identity: b"transport.Whole".to_vec(),
                    arguments: vec![],
                    fields: vec![
                        field("record", RECORD_NODE),
                        field("entries", LIST_NODE),
                        field("lookup", MAP_NODE),
                        field("state", ENUM_NODE),
                        field("machine", MACHINE_NODE),
                        field("empty", STATE_NODE),
                    ],
                },
                NativeCarrierNode::Struct {
                    identity: b"other.Record".to_vec(),
                    arguments: vec![],
                    fields: record_fields,
                },
            ],
            slots: vec![
                NativeCarrierSlot {
                    frame: 1,
                    node: OPTIONAL_NODE,
                },
                NativeCarrierSlot {
                    frame: 1,
                    node: RESULT_NODE,
                },
                NativeCarrierSlot {
                    frame: 1,
                    node: RECORD_NODE,
                },
                NativeCarrierSlot {
                    frame: 1,
                    node: OTHER_RECORD_NODE,
                },
                NativeCarrierSlot {
                    frame: 1,
                    node: GENERIC_RESOURCE_NODE,
                },
                NativeCarrierSlot {
                    frame: 1,
                    node: GENERIC_OPTIONAL_NODE,
                },
                NativeCarrierSlot {
                    frame: 1,
                    node: LIST_NODE,
                },
                NativeCarrierSlot {
                    frame: 1,
                    node: MAP_NODE,
                },
                NativeCarrierSlot {
                    frame: 1,
                    node: WHOLE_NODE,
                },
            ],
            operations: vec![],
        },
    }
}

fn legacy(version: u32) -> WireLayout {
    let mut layout = fixture();
    layout.version = version;
    layout.carriers = NativeCarrierLayout::default();
    assert_eq!(
        layout.shapes.pop(),
        Some(NativeShape::Carrier { node: WHOLE_NODE })
    );
    layout
}

fn parsed(layout: &WireLayout) -> Result<WireLayout, ResourceLayoutError> {
    let decoded = decode(&encode_records(layout))?;
    validation::validate(&decoded)?;
    Ok(decoded)
}

fn operation(layout: &mut WireLayout, value: NativeCarrierOperation) -> u32 {
    let ordinal = u32::try_from(layout.operations.len()).unwrap();
    let record = u32::try_from(layout.carriers.operations.len()).unwrap();
    let site = site(100 + ordinal);
    layout.operations.push(NativeOperationRecord {
        ordinal,
        site,
        operation: NativeOperation::Carrier { record },
    });
    layout
        .carriers
        .operations
        .push(NativeCarrierOperationRecord {
            site,
            operation: value,
        });
    ordinal
}

fn ordinary_child(index: u32, node: u32, shape: u32) -> NativeCarrierChild {
    NativeCarrierChild {
        index,
        node,
        source: NativeCarrierChildSource::Ordinary { shape },
    }
}

fn carrier_child(index: u32, node: u32, slot: u32) -> NativeCarrierChild {
    NativeCarrierChild {
        index,
        node,
        source: NativeCarrierChildSource::Move { slot },
    }
}

fn suffix_offset(bytes: &[u8]) -> usize {
    bytes
        .windows(8)
        .position(|part| part == b"JTCAR003")
        .unwrap()
}

fn write_word(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn set_length(bytes: &mut [u8]) {
    let length = u64::try_from(bytes.len()).unwrap();
    bytes[16..24].copy_from_slice(&length.to_le_bytes());
}

#[test]
fn carrier_registration_roundtrips_complete_unused_nominal_and_latent_metadata() {
    let layout = fixture();
    assert_eq!(parsed(&layout).unwrap(), layout);
    let registry = ResourceRegistry::new();
    let mut installation = NativeLayoutInstallation::new(&registry);
    let mut bytes = encode_records(&layout);
    let installed = installation.install(&registry, &bytes).unwrap();
    bytes.fill(0);
    assert_eq!(installed.carriers(), &layout.carriers);
    assert_eq!(installed.wire_version(), 3);
    assert_eq!(installed.kinds_len(), 1);
    assert_eq!(installed.slots().len(), 2);
    assert_eq!(registry.live_count(), 0);
    assert!(matches!(
        installed.carriers().node(REFINEMENT_NODE).unwrap(),
        NativeCarrierNode::Refinement { identity, base: 0, predicate }
            if identity.as_slice() == b"transport.PositiveMarker" && predicate.as_slice() == b"value > 0"
    ));
    assert!(source_validation::occupied(&layout, WHOLE_SHAPE).unwrap());
}

#[test]
fn carrier_registration_keeps_legacy_wire_versions_and_refuses_the_v3_suffix() {
    for version in [1, 2] {
        let layout = legacy(version);
        let bytes = encode_records(&layout);
        assert_eq!(parsed(&layout).unwrap(), layout);
        assert!(!bytes.windows(8).any(|part| part == b"JTCAR003"));
        let mut trailing = bytes;
        trailing.extend_from_slice(b"JTCAR003");
        trailing.extend_from_slice(&[0; 16]);
        set_length(&mut trailing);
        assert_eq!(
            decode(&trailing).unwrap_err(),
            ResourceLayoutError::Trailing
        );
    }
}

#[test]
fn carrier_registration_new_shape_and_operation_tags_require_wire_v3() {
    for version in [1, 2] {
        let mut shape = legacy(version);
        shape.shapes.push(NativeShape::Carrier { node: 0 });
        assert_eq!(
            decode(&encode_records(&shape)).unwrap_err(),
            ResourceLayoutError::UnknownTag
        );
        let mut op = legacy(version);
        op.operations.push(NativeOperationRecord {
            ordinal: 0,
            site: site(100),
            operation: NativeOperation::Carrier { record: 0 },
        });
        assert_eq!(
            decode(&encode_records(&op)).unwrap_err(),
            ResourceLayoutError::UnknownTag
        );
    }
}

#[test]
fn carrier_registration_new_source_roles_and_adapter_loan_require_wire_v3() {
    for value in [
        NativeSourceValue::CarrierOwned {
            caller_argument_slot: 0,
            callee_parameter_slot: 0,
        },
        NativeSourceValue::CarrierView {
            source: NativeCarrierLoanSource::ExistingBorrow { operation: 0 },
        },
    ] {
        let mut layout = legacy(2);
        layout.operations.push(NativeOperationRecord {
            ordinal: 0,
            site: site(100),
            operation: NativeOperation::InvokeSourceFunction {
                frame: 1,
                callee: 0,
                signature: 0,
                callee_scope: 0,
                source: Some(NativeSourceInvocation {
                    callee_return: None,
                    evaluation_order: vec![0],
                    formals: vec![NativeSourceFormal {
                        parameter: 0,
                        source_index: 0,
                        actual_shape: INTEGER,
                        callee_shape: INTEGER,
                        syntax: NativeSourceSyntax::Bare,
                        effect: NativeSourceEffect::TransferOwned,
                        access: NativeAccess::Owned,
                        value,
                    }],
                    result: NativeSourceResult::Ordinary { shape: NOTHING },
                }),
            },
        });
        assert_eq!(
            decode(&encode_records(&layout)).unwrap_err(),
            ResourceLayoutError::UnknownTag
        );
    }
    let mut result = legacy(2);
    result.operations.push(NativeOperationRecord {
        ordinal: 0,
        site: site(100),
        operation: NativeOperation::InvokeSourceFunction {
            frame: 1,
            callee: 0,
            signature: 0,
            callee_scope: 0,
            source: Some(NativeSourceInvocation {
                callee_return: Some(0),
                evaluation_order: vec![],
                formals: vec![],
                result: NativeSourceResult::Carrier {
                    shape: INTEGER,
                    caller_destination_frame: 1,
                    caller_destination_slot: 0,
                    callee_return_frame: 0,
                    permitted_return_slots: vec![0],
                },
            }),
        },
    });
    assert_eq!(
        decode(&encode_records(&result)).unwrap_err(),
        ResourceLayoutError::UnknownTag
    );
    let mut adapter = legacy(2);
    adapter.operations.push(NativeOperationRecord {
        ordinal: 0,
        site: site(100),
        operation: NativeOperation::ObserveSumView {
            frame: 1,
            source: NativeSumLoanSource::CarrierProjected { operation: 0 },
        },
    });
    assert_eq!(
        decode(&encode_records(&adapter)).unwrap_err(),
        ResourceLayoutError::UnknownTag
    );
}

#[test]
fn carrier_registration_suffix_rejects_missing_magic_reserved_counts_tags_and_dense_rows() {
    let bytes = encode_records(&fixture());
    let suffix = suffix_offset(&bytes);
    for end in suffix..bytes.len() {
        let mut prefix = bytes[..end].to_vec();
        set_length(&mut prefix);
        assert!(decode(&prefix).is_err(), "carrier suffix prefix {end}");
    }
    for (offset, value, expected) in [
        (suffix + 20, 1, ResourceLayoutError::Reserved),
        (suffix + 8, 4097, ResourceLayoutError::WireLimit),
        (suffix + 24, 1, ResourceLayoutError::DenseOrdinal),
        (suffix + 28, 99, ResourceLayoutError::UnknownTag),
    ] {
        let mut altered = bytes.clone();
        write_word(&mut altered, offset, value);
        assert_eq!(decode(&altered).unwrap_err(), expected);
    }
    let mut magic = bytes.clone();
    magic[suffix] = b'X';
    assert_eq!(decode(&magic).unwrap_err(), ResourceLayoutError::Header);
    let mut trailing = bytes;
    trailing.push(0);
    set_length(&mut trailing);
    assert_eq!(
        decode(&trailing).unwrap_err(),
        ResourceLayoutError::Trailing
    );
    let mut oversized = fixture();
    let NativeCarrierNode::Struct { identity, .. } = &mut oversized.carriers.nodes[8] else {
        unreachable!()
    };
    *identity = vec![b'x'; 4097];
    assert_eq!(
        decode(&encode_records(&oversized)).unwrap_err(),
        ResourceLayoutError::WireLimit
    );
}

#[test]
fn carrier_registration_validates_every_unused_graph_reference_and_ordinary_leaf() {
    for case in 0..7 {
        let mut layout = fixture();
        let expected = match case {
            0 => {
                layout.carriers.nodes[4] = NativeCarrierNode::Optional { child: 4 };
                ResourceLayoutError::InvalidReference
            }
            1 => {
                layout.carriers.nodes[3] = NativeCarrierNode::Resource { kind: 1 };
                ResourceLayoutError::InvalidReference
            }
            2 => {
                layout.carriers.nodes[0] = NativeCarrierNode::Ordinary {
                    shape: RESOURCE_SHAPE,
                };
                ResourceLayoutError::ShapeMismatch
            }
            3 => {
                layout.carriers.nodes[0] = NativeCarrierNode::Ordinary {
                    shape: OPTIONAL_SHAPE,
                };
                ResourceLayoutError::ShapeMismatch
            }
            4 => {
                layout.carriers.nodes[0] = NativeCarrierNode::Ordinary { shape: WHOLE_SHAPE };
                ResourceLayoutError::ShapeMismatch
            }
            5 => {
                layout
                    .carriers
                    .nodes
                    .push(NativeCarrierNode::Resource { kind: 0 });
                ResourceLayoutError::NonCanonical
            }
            6 => {
                layout.carriers.slots[0].node = 99;
                ResourceLayoutError::InvalidReference
            }
            _ => unreachable!(),
        };
        assert_eq!(
            parsed(&layout).unwrap_err(),
            expected,
            "unused graph case {case}"
        );
    }
}

#[test]
fn carrier_registration_rejects_duplicate_names_invalid_machine_state_and_unbound_refinement() {
    for case in 0..7 {
        let mut layout = fixture();
        match case {
            0 => {
                let NativeCarrierNode::Struct { fields, .. } = &mut layout.carriers.nodes[8] else {
                    unreachable!()
                };
                fields[1].name = fields[0].name.clone();
            }
            1 => {
                let NativeCarrierNode::Enum { variants, .. } = &mut layout.carriers.nodes[9] else {
                    unreachable!()
                };
                variants[1].name = variants[0].name.clone();
            }
            2 => {
                let NativeCarrierNode::Machine { transitions, .. } = &mut layout.carriers.nodes[10]
                else {
                    unreachable!()
                };
                transitions.push((0, 1));
            }
            3 => {
                layout.carriers.nodes[11] = NativeCarrierNode::MachineState {
                    machine: MACHINE_NODE,
                    state: 2,
                }
            }
            4 => {
                layout.carriers.nodes[11] = NativeCarrierNode::MachineState {
                    machine: ENUM_NODE,
                    state: 0,
                }
            }
            5 => {
                let NativeCarrierNode::Refinement { predicate, .. } =
                    &mut layout.carriers.nodes[12]
                else {
                    unreachable!()
                };
                predicate.clear();
            }
            6 => {
                let NativeCarrierNode::Refinement { identity, .. } = &mut layout.carriers.nodes[12]
                else {
                    unreachable!()
                };
                *identity = b"transport.Record".to_vec();
            }
            _ => unreachable!(),
        }
        assert!(parsed(&layout).is_err(), "nominal metadata case {case}");
    }
}

#[test]
fn carrier_registration_latent_live_alternatives_do_not_reject_absent_constructor_metadata() {
    let mut layout = fixture();
    operation(
        &mut layout,
        NativeCarrierOperation::Construct {
            frame: 1,
            destination: 0,
            selector: 0,
            children: vec![],
        },
    );
    assert_eq!(parsed(&layout).unwrap(), layout);
    assert_eq!(
        layout.carriers.selected_children(OPTIONAL_NODE, 0).unwrap(),
        Vec::<u32>::new()
    );
    assert_eq!(
        layout.carriers.selected_children(OPTIONAL_NODE, 1).unwrap(),
        vec![RESOURCE_NODE]
    );
    assert_eq!(
        layout.carriers.selected_children(ENUM_NODE, 0).unwrap(),
        vec![0]
    );
    assert_eq!(
        layout.carriers.selected_children(ENUM_NODE, 1).unwrap(),
        vec![RESOURCE_NODE]
    );
    assert_eq!(
        layout.carriers.selected_children(STATE_NODE, 0).unwrap(),
        vec![0]
    );
    assert!(layout.carriers.selected_children(STATE_NODE, 1).is_err());
}

#[test]
fn carrier_registration_constructor_lexical_order_preserves_explicit_storage_indices() {
    let mut layout = fixture();
    operation(
        &mut layout,
        NativeCarrierOperation::Construct {
            frame: 1,
            destination: 2,
            selector: 0,
            children: vec![
                carrier_child(1, OPTIONAL_NODE, 0),
                ordinary_child(0, 0, INTEGER),
            ],
        },
    );
    assert_eq!(parsed(&layout).unwrap(), layout);
    for case in 0..4 {
        let mut invalid = layout.clone();
        let NativeCarrierOperation::Construct { children, .. } =
            &mut invalid.carriers.operations[0].operation
        else {
            unreachable!()
        };
        match case {
            0 => children[1].index = 1,
            1 => children[1].index = 2,
            2 => children[1] = ordinary_child(0, 1, STRING),
            3 => children[1].source = NativeCarrierChildSource::Ordinary { shape: STRING },
            _ => unreachable!(),
        }
        assert!(parsed(&invalid).is_err(), "constructor row case {case}");
    }
}

#[test]
fn carrier_registration_collection_constructor_rejects_reordered_elements_and_map_halves() {
    let mut list = fixture();
    operation(
        &mut list,
        NativeCarrierOperation::Construct {
            frame: 1,
            destination: 6,
            selector: 0,
            children: vec![carrier_child(0, OPTIONAL_NODE, 0)],
        },
    );
    assert!(parsed(&list).is_ok());
    let NativeCarrierOperation::Construct { children, .. } =
        &mut list.carriers.operations[0].operation
    else {
        unreachable!()
    };
    children[0].index = 1;
    assert!(parsed(&list).is_err());
    let mut map = fixture();
    operation(
        &mut map,
        NativeCarrierOperation::Construct {
            frame: 1,
            destination: 7,
            selector: 0,
            children: vec![
                ordinary_child(0, 0, INTEGER),
                carrier_child(1, RESULT_NODE, 1),
            ],
        },
    );
    assert!(parsed(&map).is_ok());
    for case in 0..3 {
        let mut invalid = map.clone();
        let NativeCarrierOperation::Construct { children, .. } =
            &mut invalid.carriers.operations[0].operation
        else {
            unreachable!()
        };
        match case {
            0 => {
                children.pop();
            }
            1 => children.swap(0, 1),
            2 => children[0] = ordinary_child(0, 1, STRING),
            _ => unreachable!(),
        }
        assert!(parsed(&invalid).is_err(), "map constructor case {case}");
    }
}

#[test]
fn carrier_registration_nominal_and_generic_slots_cannot_transfer_by_matching_fields() {
    for (source, destination) in [(2, 3), (4, 5), (0, 0)] {
        let mut layout = fixture();
        operation(
            &mut layout,
            NativeCarrierOperation::Transfer {
                frame: 1,
                source,
                destination,
            },
        );
        assert!(
            parsed(&layout).is_err(),
            "carrier slot substitution {source} -> {destination}"
        );
    }
}

#[test]
fn carrier_registration_carrier_rows_require_one_exact_site_wrapper() {
    let mut layout = fixture();
    operation(
        &mut layout,
        NativeCarrierOperation::Retire {
            frame: 1,
            source: 0,
        },
    );
    assert!(parsed(&layout).is_ok());
    for case in 0..4 {
        let mut invalid = layout.clone();
        match case {
            0 => invalid.operations.clear(),
            1 => invalid.operations[0].site.position = NativePosition::Statement(200),
            2 => {
                let mut duplicate = invalid.operations[0].clone();
                duplicate.ordinal = 1;
                invalid.operations.push(duplicate);
            }
            3 => invalid.carriers.operations[0].site.function = 1,
            _ => unreachable!(),
        }
        assert!(
            parsed(&invalid).is_err(),
            "carrier site wrapper case {case}"
        );
    }
}

#[test]
fn carrier_registration_borrow_and_projection_nodes_are_exact_and_bounded() {
    let mut layout = fixture();
    let borrow = operation(
        &mut layout,
        NativeCarrierOperation::Borrow {
            frame: 1,
            source: NativeCarrierSource::Slot(2),
            path: vec![NativeCarrierPath::Field(1)],
            node: OPTIONAL_NODE,
            lease_frame: 1,
        },
    );
    operation(
        &mut layout,
        NativeCarrierOperation::Observe {
            frame: 1,
            source: NativeCarrierSource::Loan(NativeCarrierLoanSource::ExistingBorrow {
                operation: borrow,
            }),
            path: vec![],
            observation: NativeCarrierObservation::Tag,
        },
    );
    operation(
        &mut layout,
        NativeCarrierOperation::EndBorrow { frame: 1, borrow },
    );
    assert!(parsed(&layout).is_ok());
    for case in 0..4 {
        let mut invalid = layout.clone();
        match case {
            0 => {
                let NativeCarrierOperation::Borrow { node, .. } =
                    &mut invalid.carriers.operations[0].operation
                else {
                    unreachable!()
                };
                *node = RESOURCE_NODE;
            }
            1 => {
                let NativeCarrierOperation::Borrow { path, .. } =
                    &mut invalid.carriers.operations[0].operation
                else {
                    unreachable!()
                };
                *path = vec![NativeCarrierPath::Field(99)];
            }
            2 => {
                let NativeCarrierOperation::Borrow { lease_frame, .. } =
                    &mut invalid.carriers.operations[0].operation
                else {
                    unreachable!()
                };
                *lease_frame = 0;
            }
            3 => {
                let NativeCarrierOperation::EndBorrow { borrow, .. } =
                    &mut invalid.carriers.operations[2].operation
                else {
                    unreachable!()
                };
                *borrow = 1;
            }
            _ => unreachable!(),
        }
        assert!(parsed(&invalid).is_err(), "carrier borrow row case {case}");
    }
    let mut dynamic = fixture();
    operation(
        &mut dynamic,
        NativeCarrierOperation::Observe {
            frame: 1,
            source: NativeCarrierSource::Slot(6),
            path: vec![NativeCarrierPath::List(NativeCarrierSelector::Dynamic)],
            observation: NativeCarrierObservation::Tag,
        },
    );
    assert!(parsed(&dynamic).is_ok());
    let NativeCarrierOperation::Observe { path, .. } =
        &mut dynamic.carriers.operations[0].operation
    else {
        unreachable!()
    };
    path.push(NativeCarrierPath::List(NativeCarrierSelector::Dynamic));
    assert_eq!(
        parsed(&dynamic).unwrap_err(),
        ResourceLayoutError::PayloadPath
    );
}

#[test]
fn carrier_registration_ordinary_observations_do_not_reclassify_resource_nodes() {
    let mut layout = fixture();
    operation(
        &mut layout,
        NativeCarrierOperation::Observe {
            frame: 1,
            source: NativeCarrierSource::Slot(2),
            path: vec![NativeCarrierPath::Field(0)],
            observation: NativeCarrierObservation::Ordinary {
                shape: INTEGER,
                clone: false,
            },
        },
    );
    assert!(parsed(&layout).is_ok());
    let valid = layout.clone();
    let NativeCarrierOperation::Observe { path, .. } = &mut layout.carriers.operations[0].operation
    else {
        unreachable!()
    };
    *path = vec![NativeCarrierPath::Field(1), NativeCarrierPath::Some];
    for (shape, clone, expected) in [
        (
            RESOURCE_SHAPE,
            false,
            ResourceLayoutError::OperationMismatch,
        ),
        (RESOURCE_SHAPE, true, ResourceLayoutError::OperationMismatch),
        (INTEGER, false, ResourceLayoutError::ShapeMismatch),
        (STRING, true, ResourceLayoutError::ShapeMismatch),
    ] {
        let NativeCarrierOperation::Observe { observation, .. } =
            &mut layout.carriers.operations[0].operation
        else {
            unreachable!()
        };
        *observation = NativeCarrierObservation::Ordinary { shape, clone };
        let bytes = encode_records(&layout);
        assert_eq!(
            decode(&bytes).unwrap(),
            layout,
            "forgery has a valid wire encoding"
        );
        assert_eq!(parsed(&layout).unwrap_err(), expected);
        let registry = ResourceRegistry::new();
        let mut installation = NativeLayoutInstallation::new(&registry);
        assert_eq!(
            installation.install(&registry, &bytes).unwrap_err(),
            expected
        );
        assert!(installation.installed().is_none());
        assert_eq!(registry.live_count(), 0);
        let installed = installation
            .install(&registry, &encode_records(&valid))
            .unwrap();
        assert_eq!(
            installed.carriers().node(RESOURCE_NODE).unwrap(),
            &NativeCarrierNode::Resource { kind: 0 }
        );
        assert_eq!(registry.live_count(), 0);
    }
}

#[test]
fn carrier_registration_leaf_sum_adapter_requires_matching_kind_and_shape() {
    let mut layout = fixture();
    operation(
        &mut layout,
        NativeCarrierOperation::AdaptSum {
            frame: 1,
            source: NativeCarrierSource::Slot(2),
            path: vec![NativeCarrierPath::Field(1)],
            sum_slot: 0,
            lease_frame: 1,
            borrowed: true,
        },
    );
    operation(
        &mut layout,
        NativeCarrierOperation::Construct {
            frame: 1,
            destination: 2,
            selector: 0,
            children: vec![
                ordinary_child(0, 0, INTEGER),
                NativeCarrierChild {
                    index: 1,
                    node: OPTIONAL_NODE,
                    source: NativeCarrierChildSource::LeafSum { slot: 0 },
                },
            ],
        },
    );
    assert!(parsed(&layout).is_ok());
    for case in 0..3 {
        let mut invalid = layout.clone();
        match case {
            0 => {
                let NativeCarrierOperation::AdaptSum { sum_slot, .. } =
                    &mut invalid.carriers.operations[0].operation
                else {
                    unreachable!()
                };
                *sum_slot = 1;
            }
            1 => {
                let NativeCarrierOperation::Construct { children, .. } =
                    &mut invalid.carriers.operations[1].operation
                else {
                    unreachable!()
                };
                children[1].source = NativeCarrierChildSource::LeafSum { slot: 1 };
            }
            2 => {
                invalid.kinds.push(1);
                invalid.carriers.nodes[3] = NativeCarrierNode::Resource { kind: 1 };
            }
            _ => unreachable!(),
        }
        assert!(parsed(&invalid).is_err(), "leaf sum adapter case {case}");
    }
}

#[test]
fn carrier_registration_iteration_reset_names_only_distinct_binder_slots() {
    let mut layout = fixture();
    operation(
        &mut layout,
        NativeCarrierOperation::ResetIteration {
            frame: 1,
            source: 6,
            carrier_binders: vec![0],
            leaf_sum_binders: vec![0],
        },
    );
    assert_eq!(parsed(&layout).unwrap(), layout);
    for case in 0..6 {
        let mut invalid = layout.clone();
        match case {
            0 => {
                let NativeCarrierOperation::ResetIteration { source, .. } =
                    &mut invalid.carriers.operations[0].operation
                else {
                    unreachable!()
                };
                *source = 2;
            }
            1 => {
                let NativeCarrierOperation::ResetIteration {
                    carrier_binders, ..
                } = &mut invalid.carriers.operations[0].operation
                else {
                    unreachable!()
                };
                carrier_binders.push(0);
            }
            2 => {
                let NativeCarrierOperation::ResetIteration {
                    carrier_binders, ..
                } = &mut invalid.carriers.operations[0].operation
                else {
                    unreachable!()
                };
                *carrier_binders = vec![6];
            }
            3 => {
                let NativeCarrierOperation::ResetIteration {
                    leaf_sum_binders, ..
                } = &mut invalid.carriers.operations[0].operation
                else {
                    unreachable!()
                };
                leaf_sum_binders.push(0);
            }
            4 => invalid.carriers.slots[0].frame = 0,
            5 => invalid.slots[0].shape = NOTHING,
            _ => unreachable!(),
        }
        assert!(
            parsed(&invalid).is_err(),
            "iteration reset binder case {case}"
        );
    }
}

#[test]
fn carrier_registration_publish_requires_matching_scope_return_and_result_node() {
    let mut layout = fixture();
    layout.signatures[0].result = WHOLE_SHAPE;
    layout.frames[0].parents.push(NativeParent::Frame(2));
    layout.frames.push(NativeFrame {
        ordinal: 2,
        role: NativeFrameRole::Return,
        site: site(2),
        signature: 0,
        parents: vec![NativeParent::Root],
    });
    layout.carriers.slots.push(NativeCarrierSlot {
        frame: 2,
        node: WHOLE_NODE,
    });
    operation(
        &mut layout,
        NativeCarrierOperation::PublishReturn {
            frame: 0,
            source: 9,
        },
    );
    assert_eq!(parsed(&layout).unwrap(), layout);
    for case in 0..5 {
        let mut invalid = layout.clone();
        match case {
            0 => invalid.frames[0].parents = vec![NativeParent::Root],
            1 => invalid.frames[2].role = NativeFrameRole::Operation,
            2 => invalid.carriers.slots[9].node = RECORD_NODE,
            3 => invalid.frames[2].site.function = 1,
            4 => invalid.signatures[0].result = NOTHING,
            _ => unreachable!(),
        }
        assert!(
            parsed(&invalid).is_err(),
            "carrier publication linkage case {case}"
        );
    }
}

fn source_owned() -> WireLayout {
    let mut layout = fixture();
    layout.signatures.push(NativeSignature {
        ordinal: 1,
        parameters: vec![NativeFormal {
            shape: WHOLE_SHAPE,
            access: NativeAccess::Owned,
        }],
        result: WHOLE_SHAPE,
    });
    let callee_site = |index| NativeSite {
        function: 1,
        block: 0,
        position: NativePosition::Statement(index),
    };
    layout.frames.extend([
        NativeFrame {
            ordinal: 2,
            role: NativeFrameRole::Return,
            site: callee_site(0),
            signature: 1,
            parents: vec![NativeParent::Frame(1)],
        },
        NativeFrame {
            ordinal: 3,
            role: NativeFrameRole::Scope,
            site: callee_site(1),
            signature: 1,
            parents: vec![NativeParent::Frame(2)],
        },
    ]);
    layout.carriers.slots.extend([
        NativeCarrierSlot {
            frame: 0,
            node: WHOLE_NODE,
        },
        NativeCarrierSlot {
            frame: 2,
            node: WHOLE_NODE,
        },
        NativeCarrierSlot {
            frame: 3,
            node: WHOLE_NODE,
        },
    ]);
    layout.operations.push(NativeOperationRecord {
        ordinal: 0,
        site: site(100),
        operation: NativeOperation::InvokeSourceFunction {
            frame: 1,
            callee: 1,
            signature: 1,
            callee_scope: 3,
            source: Some(NativeSourceInvocation {
                callee_return: Some(2),
                evaluation_order: vec![0],
                formals: vec![NativeSourceFormal {
                    parameter: 0,
                    source_index: 0,
                    actual_shape: WHOLE_SHAPE,
                    callee_shape: WHOLE_SHAPE,
                    syntax: NativeSourceSyntax::Bare,
                    effect: NativeSourceEffect::TransferOwned,
                    access: NativeAccess::Owned,
                    value: NativeSourceValue::CarrierOwned {
                        caller_argument_slot: 8,
                        callee_parameter_slot: 11,
                    },
                }],
                result: NativeSourceResult::Carrier {
                    shape: WHOLE_SHAPE,
                    caller_destination_frame: 0,
                    caller_destination_slot: 9,
                    callee_return_frame: 2,
                    permitted_return_slots: vec![10],
                },
            }),
        },
    });
    layout
}

#[test]
fn carrier_registration_source_boundary_keeps_exact_owned_slots_and_return_permission() {
    let layout = source_owned();
    assert_eq!(parsed(&layout).unwrap(), layout);
    for case in 0..10 {
        let mut invalid = layout.clone();
        let NativeOperation::InvokeSourceFunction {
            source: Some(source),
            ..
        } = &mut invalid.operations[0].operation
        else {
            unreachable!()
        };
        match case {
            0 => source.formals[0].value = NativeSourceValue::Ordinary,
            1 => source.formals[0].actual_shape = INTEGER,
            2 => {
                source.formals[0].value = NativeSourceValue::CarrierOwned {
                    caller_argument_slot: 7,
                    callee_parameter_slot: 11,
                }
            }
            3 => {
                source.formals[0].value = NativeSourceValue::CarrierOwned {
                    caller_argument_slot: 8,
                    callee_parameter_slot: 8,
                }
            }
            4 => source.formals[0].syntax = NativeSourceSyntax::WrittenView,
            5 => source.formals[0].effect = NativeSourceEffect::Copy,
            6 => source.result = NativeSourceResult::Ordinary { shape: WHOLE_SHAPE },
            7 => {
                let NativeSourceResult::Carrier {
                    permitted_return_slots,
                    ..
                } = &mut source.result
                else {
                    unreachable!()
                };
                *permitted_return_slots = vec![9];
            }
            8 => {
                let NativeSourceResult::Carrier {
                    permitted_return_slots,
                    ..
                } = &mut source.result
                else {
                    unreachable!()
                };
                permitted_return_slots.clear();
            }
            9 => source.callee_return = Some(0),
            _ => unreachable!(),
        }
        assert!(
            parsed(&invalid).is_err(),
            "carrier Source boundary case {case}"
        );
    }
}

#[test]
fn carrier_registration_source_view_uses_the_exact_existing_carrier_borrow() {
    let mut layout = source_owned();
    let mut call = layout.operations.pop().unwrap();
    layout.signatures[1].parameters[0].access = NativeAccess::View;
    let borrow = operation(
        &mut layout,
        NativeCarrierOperation::Borrow {
            frame: 1,
            source: NativeCarrierSource::Slot(8),
            path: vec![],
            node: WHOLE_NODE,
            lease_frame: 1,
        },
    );
    let NativeOperation::InvokeSourceFunction {
        source: Some(source),
        ..
    } = &mut call.operation
    else {
        unreachable!()
    };
    source.formals[0].syntax = NativeSourceSyntax::WrittenView;
    source.formals[0].effect = NativeSourceEffect::RetainBorrow;
    source.formals[0].access = NativeAccess::View;
    source.formals[0].value = NativeSourceValue::CarrierView {
        source: NativeCarrierLoanSource::ExistingBorrow { operation: borrow },
    };
    call.ordinal = 1;
    call.site = site(101);
    layout.operations.push(call);
    assert_eq!(parsed(&layout).unwrap(), layout);
    let NativeOperation::InvokeSourceFunction {
        source: Some(source),
        ..
    } = &mut layout.operations[1].operation
    else {
        unreachable!()
    };
    source.formals[0].value = NativeSourceValue::CarrierView {
        source: NativeCarrierLoanSource::ExistingBorrow { operation: 1 },
    };
    assert_eq!(
        parsed(&layout).unwrap_err(),
        ResourceLayoutError::OperationMismatch
    );
}

/// Exact slot indices for tests of the registered runtime fixture below.
pub(crate) const RUNTIME_CARRIER_FIXTURE_SLOTS: [(&str, u32); 9] = [
    ("optional", 0),
    ("result", 1),
    ("record", 2),
    ("list", 6),
    ("map", 7),
    ("transferred_record", 9),
    ("latent_resource", 10),
    ("second_optional", 11),
    ("iteration_binder", 12),
];

fn native_operation(layout: &mut WireLayout, function: u32, value: NativeOperation) -> u32 {
    let ordinal = u32::try_from(layout.operations.len()).unwrap();
    layout.operations.push(NativeOperationRecord {
        ordinal,
        site: NativeSite {
            function,
            block: 0,
            position: NativePosition::Statement(100 + ordinal),
        },
        operation: value,
    });
    ordinal
}

/// Test bytes use the real registrar and decoder. This helper never constructs a
/// production registry identity, owner token, loan token, or Source proof.
pub(crate) fn runtime_fixture_bytes() -> Vec<u8> {
    let mut layout = fixture();
    let slots = RUNTIME_CARRIER_FIXTURE_SLOTS;
    let optional = slots[0].1;
    let result = slots[1].1;
    let record = slots[2].1;
    let list = slots[3].1;
    let map = slots[4].1;
    let transferred_record = slots[5].1;
    let latent_resource = slots[6].1;
    let second_optional = slots[7].1;
    let iteration_binder = slots[8].1;
    layout.carriers.slots.extend([
        NativeCarrierSlot {
            frame: 1,
            node: RECORD_NODE,
        },
        NativeCarrierSlot {
            frame: 1,
            node: RESOURCE_NODE,
        },
        NativeCarrierSlot {
            frame: 1,
            node: OPTIONAL_NODE,
        },
        NativeCarrierSlot {
            frame: 1,
            node: OPTIONAL_NODE,
        },
    ]);
    for destination in [optional, second_optional, iteration_binder] {
        operation(
            &mut layout,
            NativeCarrierOperation::Construct {
                frame: 1,
                destination,
                selector: 0,
                children: vec![],
            },
        );
    }
    // A selected live arm is consistent metadata, but runtime construction of a
    // reached Resource node must refuse before acquiring an aggregate owner.
    operation(
        &mut layout,
        NativeCarrierOperation::Construct {
            frame: 1,
            destination: optional,
            selector: 1,
            children: vec![carrier_child(0, RESOURCE_NODE, latent_resource)],
        },
    );
    operation(
        &mut layout,
        NativeCarrierOperation::Construct {
            frame: 1,
            destination: record,
            selector: 0,
            children: vec![
                ordinary_child(0, 0, INTEGER),
                carrier_child(1, OPTIONAL_NODE, optional),
            ],
        },
    );
    operation(
        &mut layout,
        NativeCarrierOperation::Construct {
            frame: 1,
            destination: record,
            selector: 0,
            children: vec![
                ordinary_child(0, 0, INTEGER),
                NativeCarrierChild {
                    index: 1,
                    node: OPTIONAL_NODE,
                    source: NativeCarrierChildSource::LeafSum { slot: 0 },
                },
            ],
        },
    );
    operation(
        &mut layout,
        NativeCarrierOperation::Transfer {
            frame: 1,
            source: record,
            destination: transferred_record,
        },
    );
    operation(
        &mut layout,
        NativeCarrierOperation::Transfer {
            frame: 1,
            source: transferred_record,
            destination: record,
        },
    );
    let full_borrow = operation(
        &mut layout,
        NativeCarrierOperation::Borrow {
            frame: 1,
            source: NativeCarrierSource::Slot(transferred_record),
            path: vec![],
            node: RECORD_NODE,
            lease_frame: 1,
        },
    );
    let child_borrow = operation(
        &mut layout,
        NativeCarrierOperation::Borrow {
            frame: 1,
            source: NativeCarrierSource::Loan(NativeCarrierLoanSource::ExistingBorrow {
                operation: full_borrow,
            }),
            path: vec![NativeCarrierPath::Field(1)],
            node: OPTIONAL_NODE,
            lease_frame: 1,
        },
    );
    let adapter = operation(
        &mut layout,
        NativeCarrierOperation::AdaptSum {
            frame: 1,
            source: NativeCarrierSource::Slot(transferred_record),
            path: vec![NativeCarrierPath::Field(1)],
            sum_slot: 0,
            lease_frame: 1,
            borrowed: true,
        },
    );
    let nested_adapter = operation(
        &mut layout,
        NativeCarrierOperation::AdaptSum {
            frame: 1,
            source: NativeCarrierSource::Loan(NativeCarrierLoanSource::ExistingBorrow {
                operation: child_borrow,
            }),
            path: vec![],
            sum_slot: 0,
            lease_frame: 1,
            borrowed: true,
        },
    );
    operation(
        &mut layout,
        NativeCarrierOperation::EndBorrow {
            frame: 1,
            borrow: child_borrow,
        },
    );
    operation(
        &mut layout,
        NativeCarrierOperation::EndBorrow {
            frame: 1,
            borrow: full_borrow,
        },
    );
    for operation in [adapter, nested_adapter] {
        native_operation(
            &mut layout,
            0,
            NativeOperation::ObserveSumView {
                frame: 1,
                source: NativeSumLoanSource::CarrierProjected { operation },
            },
        );
        native_operation(
            &mut layout,
            0,
            NativeOperation::EndSumBorrow {
                frame: 1,
                borrow: operation,
            },
        );
    }
    operation(
        &mut layout,
        NativeCarrierOperation::AdaptSum {
            frame: 1,
            source: NativeCarrierSource::Slot(transferred_record),
            path: vec![NativeCarrierPath::Field(1)],
            sum_slot: 0,
            lease_frame: 1,
            borrowed: false,
        },
    );
    operation(
        &mut layout,
        NativeCarrierOperation::Extract {
            frame: 1,
            source: transferred_record,
            path: vec![NativeCarrierPath::Field(0)],
            destination: NativeCarrierDestination::Ordinary { shape: INTEGER },
        },
    );
    for source in [record, transferred_record] {
        operation(
            &mut layout,
            NativeCarrierOperation::Observe {
                frame: 1,
                source: NativeCarrierSource::Slot(source),
                path: vec![NativeCarrierPath::Field(0)],
                observation: NativeCarrierObservation::Ordinary {
                    shape: INTEGER,
                    clone: false,
                },
            },
        );
    }
    operation(
        &mut layout,
        NativeCarrierOperation::Observe {
            frame: 1,
            source: NativeCarrierSource::Loan(NativeCarrierLoanSource::ExistingBorrow {
                operation: child_borrow,
            }),
            path: vec![],
            observation: NativeCarrierObservation::Tag,
        },
    );
    operation(
        &mut layout,
        NativeCarrierOperation::Observe {
            frame: 1,
            source: NativeCarrierSource::Slot(list),
            path: vec![],
            observation: NativeCarrierObservation::Length,
        },
    );
    for children in [
        vec![],
        vec![carrier_child(0, OPTIONAL_NODE, optional)],
        vec![
            carrier_child(0, OPTIONAL_NODE, optional),
            carrier_child(1, OPTIONAL_NODE, second_optional),
        ],
    ] {
        operation(
            &mut layout,
            NativeCarrierOperation::Construct {
                frame: 1,
                destination: list,
                selector: 0,
                children,
            },
        );
    }
    operation(
        &mut layout,
        NativeCarrierOperation::Construct {
            frame: 1,
            destination: result,
            selector: 0,
            children: vec![ordinary_child(0, 1, STRING)],
        },
    );
    operation(
        &mut layout,
        NativeCarrierOperation::Construct {
            frame: 1,
            destination: map,
            selector: 0,
            children: vec![
                ordinary_child(0, 0, INTEGER),
                carrier_child(1, RESULT_NODE, result),
            ],
        },
    );
    operation(
        &mut layout,
        NativeCarrierOperation::Extract {
            frame: 1,
            source: list,
            path: vec![NativeCarrierPath::List(NativeCarrierSelector::Dynamic)],
            destination: NativeCarrierDestination::Slot(iteration_binder),
        },
    );
    operation(
        &mut layout,
        NativeCarrierOperation::AdaptSum {
            frame: 1,
            source: NativeCarrierSource::Slot(iteration_binder),
            path: vec![],
            sum_slot: 0,
            lease_frame: 1,
            borrowed: false,
        },
    );
    operation(
        &mut layout,
        NativeCarrierOperation::ResetIteration {
            frame: 1,
            source: list,
            carrier_binders: vec![iteration_binder],
            leaf_sum_binders: vec![0],
        },
    );
    for source in [
        optional,
        result,
        record,
        transferred_record,
        list,
        map,
        second_optional,
        iteration_binder,
    ] {
        operation(
            &mut layout,
            NativeCarrierOperation::Retire { frame: 1, source },
        );
    }
    native_operation(
        &mut layout,
        0,
        NativeOperation::CreateAbsentSum {
            frame: 1,
            destination_slot: 0,
        },
    );
    let sum_borrow = native_operation(
        &mut layout,
        0,
        NativeOperation::BorrowSum {
            frame: 1,
            source_sum_slot: 0,
            lease_frame: 1,
        },
    );
    native_operation(
        &mut layout,
        0,
        NativeOperation::ObserveSumView {
            frame: 1,
            source: NativeSumLoanSource::ExistingBorrow {
                operation: sum_borrow,
            },
        },
    );
    native_operation(
        &mut layout,
        0,
        NativeOperation::EndSumBorrow {
            frame: 1,
            borrow: sum_borrow,
        },
    );
    native_operation(
        &mut layout,
        0,
        NativeOperation::SumDrop {
            frame: 1,
            source: 0,
        },
    );
    native_operation(&mut layout, 0, NativeOperation::Complete { frame: 1 });
    native_operation(&mut layout, 0, NativeOperation::Complete { frame: 0 });

    layout.signatures.push(NativeSignature {
        ordinal: 1,
        parameters: vec![NativeFormal {
            shape: OPTIONAL_SHAPE,
            access: NativeAccess::View,
        }],
        result: NOTHING,
    });
    layout.frames.push(NativeFrame {
        ordinal: 2,
        role: NativeFrameRole::Scope,
        site: NativeSite {
            function: 1,
            block: 0,
            position: NativePosition::Statement(0),
        },
        signature: 1,
        parents: vec![NativeParent::Frame(1)],
    });
    native_operation(
        &mut layout,
        0,
        NativeOperation::InvokeSourceFunction {
            frame: 1,
            callee: 1,
            signature: 1,
            callee_scope: 2,
            source: Some(NativeSourceInvocation {
                callee_return: None,
                evaluation_order: vec![0],
                formals: vec![NativeSourceFormal {
                    parameter: 0,
                    source_index: 0,
                    actual_shape: OPTIONAL_SHAPE,
                    callee_shape: OPTIONAL_SHAPE,
                    syntax: NativeSourceSyntax::WrittenView,
                    effect: NativeSourceEffect::RetainBorrow,
                    access: NativeAccess::View,
                    value: NativeSourceValue::ResidentSumView {
                        source: NativeSumLoanSource::CarrierProjected { operation: adapter },
                    },
                }],
                result: NativeSourceResult::Ordinary { shape: NOTHING },
            }),
        },
    );
    native_operation(
        &mut layout,
        1,
        NativeOperation::ObserveSumView {
            frame: 2,
            source: NativeSumLoanSource::IncomingViewFormal {
                scope: 2,
                parameter: 0,
            },
        },
    );
    native_operation(&mut layout, 1, NativeOperation::Complete { frame: 2 });
    layout.carriers.slots.extend([
        NativeCarrierSlot {
            frame: 1,
            node: STATE_NODE,
        },
        NativeCarrierSlot {
            frame: 1,
            node: MACHINE_NODE,
        },
        NativeCarrierSlot {
            frame: 1,
            node: ENUM_NODE,
        },
    ]);
    operation(
        &mut layout,
        NativeCarrierOperation::Construct {
            frame: 1,
            destination: 13,
            selector: 0,
            children: vec![ordinary_child(0, 0, INTEGER)],
        },
    );
    operation(
        &mut layout,
        NativeCarrierOperation::QualifyMachine {
            frame: 1,
            source: 13,
            destination: 14,
            machine: MACHINE_NODE,
            state: 0,
        },
    );
    operation(
        &mut layout,
        NativeCarrierOperation::Observe {
            frame: 1,
            source: NativeCarrierSource::Slot(14),
            path: vec![],
            observation: NativeCarrierObservation::State,
        },
    );
    operation(
        &mut layout,
        NativeCarrierOperation::Construct {
            frame: 1,
            destination: 15,
            selector: 0,
            children: vec![ordinary_child(0, 0, INTEGER)],
        },
    );
    operation(
        &mut layout,
        NativeCarrierOperation::Observe {
            frame: 1,
            source: NativeCarrierSource::Slot(15),
            path: vec![],
            observation: NativeCarrierObservation::Tag,
        },
    );
    encode_records(&layout)
}

#[test]
fn carrier_registration_runtime_fixture_is_a_closed_v3_layout() {
    let decoded = decode(&runtime_fixture_bytes()).unwrap();
    assert_eq!(decoded.version, 3);
    assert_eq!(decoded.signatures[0].parameters, vec![]);
    assert_eq!(decoded.signatures[0].result, NOTHING);
    assert_eq!(validation::validate(&decoded).unwrap(), vec![0, 0]);
    let registry = ResourceRegistry::new();
    NativeLayoutInstallation::new(&registry)
        .install(&registry, &runtime_fixture_bytes())
        .unwrap();
    assert_eq!(registry.live_count(), 0);
}

fn ordinary_observation_fixture(shape: NativeShape, clone: bool) -> WireLayout {
    let mut layout = fixture();
    let shape = if let Some(index) = layout.shapes.iter().position(|row| row == &shape) {
        u32::try_from(index).unwrap()
    } else {
        let index = u32::try_from(layout.shapes.len()).unwrap();
        layout.shapes.push(shape);
        index
    };
    let ordinary = NativeCarrierNode::Ordinary { shape };
    let node = if let Some(index) = layout
        .carriers
        .nodes
        .iter()
        .position(|row| row == &ordinary)
    {
        u32::try_from(index).unwrap()
    } else {
        let index = u32::try_from(layout.carriers.nodes.len()).unwrap();
        layout.carriers.nodes.push(ordinary);
        index
    };
    let record = u32::try_from(layout.carriers.nodes.len()).unwrap();
    layout.carriers.nodes.push(NativeCarrierNode::Struct {
        identity: b"transport.ObservedOrdinaryLeaf".to_vec(),
        arguments: vec![],
        fields: vec![field("value", node), field("channel", OPTIONAL_NODE)],
    });
    let slot = u32::try_from(layout.carriers.slots.len()).unwrap();
    layout.carriers.slots.push(NativeCarrierSlot {
        frame: 1,
        node: record,
    });
    operation(
        &mut layout,
        NativeCarrierOperation::Observe {
            frame: 1,
            source: NativeCarrierSource::Slot(slot),
            path: vec![NativeCarrierPath::Field(0)],
            observation: NativeCarrierObservation::Ordinary { shape, clone },
        },
    );
    layout
}

#[test]
fn carrier_registration_scalar_observation_refuses_a_forged_clone_flag() {
    for shape in [
        NativeShape::Integer {
            bits: 8,
            signed: true,
        },
        NativeShape::Integer {
            bits: 64,
            signed: true,
        },
        NativeShape::Integer {
            bits: 64,
            signed: false,
        },
        NativeShape::Float { bits: 32 },
        NativeShape::Float { bits: 64 },
        NativeShape::Bool,
        NativeShape::Nothing,
    ] {
        let valid = ordinary_observation_fixture(shape.clone(), false);
        assert_eq!(parsed(&valid).unwrap(), valid, "scalar shape {shape:?}");
        let forged = ordinary_observation_fixture(shape.clone(), true);
        assert_eq!(
            decode(&encode_records(&forged)).unwrap(),
            forged,
            "clone flag is a valid wire flag for {shape:?}"
        );
        assert_eq!(
            parsed(&forged).unwrap_err(),
            ResourceLayoutError::OperationMismatch,
            "scalar observation must copy bits for {shape:?}"
        );
    }
}

#[test]
fn carrier_registration_string_observation_refuses_a_forged_borrowed_owner_flag() {
    let valid = ordinary_observation_fixture(NativeShape::String, true);
    assert_eq!(parsed(&valid).unwrap(), valid);
    let forged = ordinary_observation_fixture(NativeShape::String, false);
    assert_eq!(decode(&encode_records(&forged)).unwrap(), forged);
    assert_eq!(
        parsed(&forged).unwrap_err(),
        ResourceLayoutError::OperationMismatch,
        "string observation must create its independent ordinary owner"
    );
}

#[test]
fn carrier_registration_owning_ordinary_sum_observation_refuses_both_clone_flags() {
    for shape in [
        NativeShape::Optional { child: INTEGER },
        NativeShape::Result {
            ok: INTEGER,
            fail: STRING,
        },
    ] {
        let mut metadata = ordinary_observation_fixture(shape.clone(), false);
        metadata.operations.clear();
        metadata.carriers.operations.clear();
        assert_eq!(
            parsed(&metadata).unwrap(),
            metadata,
            "ordinary sum metadata itself is consistent for {shape:?}"
        );
        for clone in [false, true] {
            let forged = ordinary_observation_fixture(shape.clone(), clone);
            assert_eq!(decode(&encode_records(&forged)).unwrap(), forged);
            assert_eq!(
                parsed(&forged).unwrap_err(),
                ResourceLayoutError::OperationMismatch,
                "owning sum observation remains pending for {shape:?}, clone={clone}"
            );
        }
    }
}
