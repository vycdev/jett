//! V3 carrier consistency records. None of these records is Source authority.
use super::{ResourceLayoutError, schema::*};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct NativeCarrierField {
    pub(crate) name: Vec<u8>,
    pub(crate) node: u32,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct NativeCarrierMember {
    pub(crate) name: Vec<u8>,
    pub(crate) discriminant: i64,
    pub(crate) fields: Vec<NativeCarrierField>,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum NativeCarrierNode {
    Ordinary {
        shape: u32,
    },
    Resource {
        kind: u32,
    },
    Optional {
        child: u32,
    },
    Result {
        ok: u32,
        fail: u32,
    },
    List {
        element: u32,
    },
    Map {
        key: u32,
        value: u32,
    },
    Struct {
        identity: Vec<u8>,
        arguments: Vec<u32>,
        fields: Vec<NativeCarrierField>,
    },
    Enum {
        identity: Vec<u8>,
        arguments: Vec<u32>,
        variants: Vec<NativeCarrierMember>,
    },
    Machine {
        identity: Vec<u8>,
        states: Vec<NativeCarrierMember>,
        transitions: Vec<(u32, u32)>,
    },
    MachineState {
        machine: u32,
        state: u32,
    },
    Refinement {
        identity: Vec<u8>,
        base: u32,
        predicate: Vec<u8>,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum NativeCarrierSelector {
    Static(u32),
    Dynamic,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum NativeCarrierPath {
    Field(u32),
    EnumPayload { variant: u32, field: u32 },
    MachinePayload { state: u32, field: u32 },
    List(NativeCarrierSelector),
    MapKey(NativeCarrierSelector),
    MapValue(NativeCarrierSelector),
    Some,
    Ok,
    Fail,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum NativeCarrierLoanSource {
    ExistingBorrow { operation: u32 },
    IncomingViewFormal { scope: u32, parameter: u32 },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum NativeCarrierSource {
    Slot(u32),
    Loan(NativeCarrierLoanSource),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum NativeCarrierChildSource {
    Ordinary { shape: u32 },
    Move { slot: u32 },
    LeafSum { slot: u32 },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeCarrierChild {
    pub(crate) index: u32,
    pub(crate) node: u32,
    pub(crate) source: NativeCarrierChildSource,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeCarrierObservation {
    Length,
    Tag,
    State,
    Ordinary { shape: u32, clone: bool },
}
impl NativeCarrierObservation {
    /// Borrowed observation may copy scalar bits or clone one exact String.
    /// Other ordinary owners require their own typed cloning transport.
    pub(crate) fn ordinary_clone_policy(shapes: &[NativeShape], shape: u32) -> Option<bool> {
        match shapes.get(shape as usize)? {
            NativeShape::String => Some(true),
            NativeShape::Integer { .. }
            | NativeShape::Float { .. }
            | NativeShape::Bool
            | NativeShape::Nothing => Some(false),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeCarrierDestination {
    Slot(u32),
    Ordinary { shape: u32 },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeCarrierSlot {
    pub(crate) frame: u32,
    pub(crate) node: u32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NativeCarrierOperation {
    Construct {
        frame: u32,
        destination: u32,
        selector: u32,
        children: Vec<NativeCarrierChild>,
    },
    Transfer {
        frame: u32,
        source: u32,
        destination: u32,
    },
    Borrow {
        frame: u32,
        source: NativeCarrierSource,
        path: Vec<NativeCarrierPath>,
        node: u32,
        lease_frame: u32,
    },
    EndBorrow {
        frame: u32,
        borrow: u32,
    },
    Observe {
        frame: u32,
        source: NativeCarrierSource,
        path: Vec<NativeCarrierPath>,
        observation: NativeCarrierObservation,
    },
    Extract {
        frame: u32,
        source: u32,
        path: Vec<NativeCarrierPath>,
        destination: NativeCarrierDestination,
    },
    AdaptSum {
        frame: u32,
        source: NativeCarrierSource,
        path: Vec<NativeCarrierPath>,
        sum_slot: u32,
        lease_frame: u32,
        borrowed: bool,
    },
    Retire {
        frame: u32,
        source: u32,
    },
    PublishReturn {
        frame: u32,
        source: u32,
    },
    ResetIteration {
        frame: u32,
        source: u32,
        carrier_binders: Vec<u32>,
        leaf_sum_binders: Vec<u32>,
    },
    QualifyMachine {
        frame: u32,
        source: u32,
        destination: u32,
        machine: u32,
        state: u32,
    },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeCarrierOperationRecord {
    pub(crate) site: NativeSite,
    pub(crate) operation: NativeCarrierOperation,
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct NativeCarrierLayout {
    pub(crate) nodes: Vec<NativeCarrierNode>,
    pub(crate) slots: Vec<NativeCarrierSlot>,
    pub(crate) operations: Vec<NativeCarrierOperationRecord>,
}
impl NativeCarrierOperation {
    pub(crate) fn frame(&self) -> u32 {
        match self {
            Self::Construct { frame, .. }
            | Self::Transfer { frame, .. }
            | Self::Borrow { frame, .. }
            | Self::EndBorrow { frame, .. }
            | Self::Observe { frame, .. }
            | Self::Extract { frame, .. }
            | Self::AdaptSum { frame, .. }
            | Self::Retire { frame, .. }
            | Self::PublishReturn { frame, .. }
            | Self::ResetIteration { frame, .. }
            | Self::QualifyMachine { frame, .. } => *frame,
        }
    }
}
impl NativeCarrierLayout {
    pub(crate) fn node(&self, node: u32) -> Result<&NativeCarrierNode, ResourceLayoutError> {
        self.nodes
            .get(node as usize)
            .ok_or(ResourceLayoutError::InvalidReference)
    }
    pub(crate) fn selected_children(
        &self,
        node: u32,
        selector: u32,
    ) -> Result<Vec<u32>, ResourceLayoutError> {
        let fields = |fields: &[NativeCarrierField]| fields.iter().map(|f| f.node).collect();
        Ok(match self.node(node)? {
            NativeCarrierNode::Ordinary { .. } if selector == 0 => vec![node],
            NativeCarrierNode::Optional { .. } if selector == 0 => vec![],
            NativeCarrierNode::Optional { child } if selector == 1 => vec![*child],
            NativeCarrierNode::Result { fail, .. } if selector == 0 => vec![*fail],
            NativeCarrierNode::Result { ok, .. } if selector == 1 => vec![*ok],
            NativeCarrierNode::Struct { fields: fs, .. } if selector == 0 => fields(fs),
            NativeCarrierNode::Enum { variants, .. } => fields(
                &variants
                    .get(selector as usize)
                    .ok_or(ResourceLayoutError::InvalidReference)?
                    .fields,
            ),
            NativeCarrierNode::Machine { states, .. } => fields(
                &states
                    .get(selector as usize)
                    .ok_or(ResourceLayoutError::InvalidReference)?
                    .fields,
            ),
            NativeCarrierNode::MachineState { machine, state } if selector == *state => {
                self.selected_children(*machine, selector)?
            }
            NativeCarrierNode::List { .. } | NativeCarrierNode::Map { .. } if selector == 0 => {
                vec![]
            }
            _ => return Err(ResourceLayoutError::UnsupportedLayout),
        })
    }
    pub(crate) fn path_node(
        &self,
        mut node: u32,
        path: &[NativeCarrierPath],
    ) -> Result<u32, ResourceLayoutError> {
        for step in path {
            node = match (self.node(node)?, step) {
                (NativeCarrierNode::Struct { fields, .. }, NativeCarrierPath::Field(i)) => {
                    fields
                        .get(*i as usize)
                        .ok_or(ResourceLayoutError::PayloadPath)?
                        .node
                }
                (
                    NativeCarrierNode::Enum { variants, .. },
                    NativeCarrierPath::EnumPayload { variant, field },
                ) => {
                    variants
                        .get(*variant as usize)
                        .and_then(|v| v.fields.get(*field as usize))
                        .ok_or(ResourceLayoutError::PayloadPath)?
                        .node
                }
                (
                    NativeCarrierNode::Machine { states, .. },
                    NativeCarrierPath::MachinePayload { state, field },
                ) => {
                    states
                        .get(*state as usize)
                        .and_then(|s| s.fields.get(*field as usize))
                        .ok_or(ResourceLayoutError::PayloadPath)?
                        .node
                }
                (
                    NativeCarrierNode::MachineState {
                        machine,
                        state: expected,
                    },
                    NativeCarrierPath::MachinePayload { state, field },
                ) if state == expected => self.path_node(
                    *machine,
                    &[NativeCarrierPath::MachinePayload {
                        state: *state,
                        field: *field,
                    }],
                )?,
                (NativeCarrierNode::List { element }, NativeCarrierPath::List(_)) => *element,
                (NativeCarrierNode::Map { key, .. }, NativeCarrierPath::MapKey(_)) => *key,
                (NativeCarrierNode::Map { value, .. }, NativeCarrierPath::MapValue(_)) => *value,
                (NativeCarrierNode::Optional { child }, NativeCarrierPath::Some) => *child,
                (NativeCarrierNode::Result { ok, .. }, NativeCarrierPath::Ok) => *ok,
                (NativeCarrierNode::Result { fail, .. }, NativeCarrierPath::Fail) => *fail,
                _ => return Err(ResourceLayoutError::PayloadPath),
            };
        }
        Ok(node)
    }
}

pub(super) fn validate(layout: &WireLayout) -> Result<(), ResourceLayoutError> {
    let graph = &layout.carriers;
    if layout.version != 3 && graph != &NativeCarrierLayout::default() {
        return Err(ResourceLayoutError::UnsupportedLayout);
    }
    let mut unique = HashSet::new();
    let mut identities = HashSet::new();
    unique
        .try_reserve(graph.nodes.len())
        .map_err(|_| ResourceLayoutError::CapacityExhausted)?;
    identities
        .try_reserve(graph.nodes.len())
        .map_err(|_| ResourceLayoutError::CapacityExhausted)?;
    for (i, node) in graph.nodes.iter().enumerate() {
        if !unique.insert(node) {
            return Err(ResourceLayoutError::NonCanonical);
        }
        let child = |id: u32| {
            if (id as usize) < i {
                Ok(())
            } else {
                Err(ResourceLayoutError::InvalidReference)
            }
        };
        let fields = |fs: &[NativeCarrierField]| {
            let mut names = HashSet::new();
            names
                .try_reserve(fs.len())
                .map_err(|_| ResourceLayoutError::CapacityExhausted)?;
            for f in fs {
                if f.name.is_empty()
                    || std::str::from_utf8(&f.name).is_err()
                    || !names.insert(&f.name)
                {
                    return Err(ResourceLayoutError::NonCanonical);
                }
                child(f.node)?;
            }
            Ok(())
        };
        let members = |ms: &[NativeCarrierMember]| {
            let mut names = HashSet::new();
            names
                .try_reserve(ms.len())
                .map_err(|_| ResourceLayoutError::CapacityExhausted)?;
            for m in ms {
                if m.name.is_empty()
                    || std::str::from_utf8(&m.name).is_err()
                    || !names.insert(&m.name)
                {
                    return Err(ResourceLayoutError::NonCanonical);
                }
                fields(&m.fields)?;
            }
            Ok(())
        };
        match node {
            NativeCarrierNode::Ordinary { shape: shape_id } => {
                let shape = layout
                    .shapes
                    .get(*shape_id as usize)
                    .ok_or(ResourceLayoutError::InvalidReference)?;
                if super::source_validation::occupied(layout, *shape_id)?
                    || matches!(shape, NativeShape::HookDescriptor { .. })
                {
                    return Err(ResourceLayoutError::ShapeMismatch);
                }
            }
            NativeCarrierNode::Resource { kind } => {
                layout
                    .kinds
                    .get(*kind as usize)
                    .ok_or(ResourceLayoutError::InvalidReference)?;
            }
            NativeCarrierNode::Optional { child: id } | NativeCarrierNode::List { element: id } => {
                child(*id)?
            }
            NativeCarrierNode::Result { ok, fail } => {
                child(*ok)?;
                child(*fail)?;
            }
            NativeCarrierNode::Map { key, value } => {
                child(*key)?;
                child(*value)?;
                if !map_key(layout, *key)? {
                    return Err(ResourceLayoutError::UnsupportedLayout);
                }
            }
            NativeCarrierNode::Struct {
                identity,
                arguments,
                fields: fs,
            } => {
                if identity.is_empty() || !identities.insert(identity) {
                    return Err(ResourceLayoutError::NonCanonical);
                }
                for a in arguments {
                    child(*a)?;
                }
                fields(fs)?;
            }
            NativeCarrierNode::Enum {
                identity,
                arguments,
                variants,
            } => {
                if identity.is_empty() || !identities.insert(identity) {
                    return Err(ResourceLayoutError::NonCanonical);
                }
                for a in arguments {
                    child(*a)?;
                }
                members(variants)?;
            }
            NativeCarrierNode::Machine {
                identity,
                states,
                transitions,
            } => {
                if identity.is_empty() || !identities.insert(identity) {
                    return Err(ResourceLayoutError::NonCanonical);
                }
                members(states)?;
                let mut pairs = HashSet::new();
                for (a, b) in transitions {
                    if *a as usize >= states.len()
                        || *b as usize >= states.len()
                        || !pairs.insert((*a, *b))
                    {
                        return Err(ResourceLayoutError::InvalidReference);
                    }
                }
            }
            NativeCarrierNode::MachineState { machine, state } => {
                child(*machine)?;
                if !matches!(graph.node(*machine)?,NativeCarrierNode::Machine{states,..} if (*state as usize)<states.len())
                {
                    return Err(ResourceLayoutError::ShapeMismatch);
                }
            }
            NativeCarrierNode::Refinement {
                identity,
                base,
                predicate,
            } => {
                child(*base)?;
                if identity.is_empty() || predicate.is_empty() || !identities.insert(identity) {
                    return Err(ResourceLayoutError::NonCanonical);
                }
            }
        }
    }
    for s in &graph.slots {
        graph.node(s.node)?;
        layout
            .frames
            .get(s.frame as usize)
            .ok_or(ResourceLayoutError::InvalidReference)?;
    }
    for (i, row) in graph.operations.iter().enumerate() {
        let frame = layout
            .frames
            .get(row.operation.frame() as usize)
            .ok_or(ResourceLayoutError::InvalidReference)?;
        if frame.site.function != row.site.function {
            return Err(ResourceLayoutError::FrameMismatch);
        }
        let wrappers = layout
            .operations
            .iter()
            .filter(|r| {
                r.site == row.site
                    && matches!(r.operation,NativeOperation::Carrier{record} if record as usize==i)
            })
            .count();
        if wrappers != 1 {
            return Err(ResourceLayoutError::OperationMismatch);
        }
        validate_operation(layout, row)?;
    }
    Ok(())
}

pub(crate) fn source_node(
    layout: &WireLayout,
    source: NativeCarrierSource,
) -> Result<u32, ResourceLayoutError> {
    match source {
        NativeCarrierSource::Slot(slot) => layout
            .carriers
            .slots
            .get(slot as usize)
            .map(|s| s.node)
            .ok_or(ResourceLayoutError::InvalidReference),
        NativeCarrierSource::Loan(NativeCarrierLoanSource::ExistingBorrow { operation }) => {
            let NativeOperation::Carrier { record } = layout
                .operations
                .get(operation as usize)
                .ok_or(ResourceLayoutError::InvalidReference)?
                .operation
            else {
                return Err(ResourceLayoutError::OperationMismatch);
            };
            match layout
                .carriers
                .operations
                .get(record as usize)
                .ok_or(ResourceLayoutError::InvalidReference)?
                .operation
            {
                NativeCarrierOperation::Borrow { node, .. } => Ok(node),
                _ => Err(ResourceLayoutError::OperationMismatch),
            }
        }
        NativeCarrierSource::Loan(NativeCarrierLoanSource::IncomingViewFormal {
            scope,
            parameter,
        }) => {
            let f = layout
                .frames
                .get(scope as usize)
                .ok_or(ResourceLayoutError::InvalidReference)?;
            let formal = layout
                .signatures
                .get(f.signature as usize)
                .and_then(|s| s.parameters.get(parameter as usize))
                .ok_or(ResourceLayoutError::InvalidReference)?;
            if f.role != NativeFrameRole::Scope || formal.access != NativeAccess::View {
                return Err(ResourceLayoutError::OperationMismatch);
            }
            match layout.shapes.get(formal.shape as usize) {
                Some(NativeShape::Carrier { node }) => Ok(*node),
                _ => Err(ResourceLayoutError::ShapeMismatch),
            }
        }
    }
}
fn validate_operation(
    layout: &WireLayout,
    row: &NativeCarrierOperationRecord,
) -> Result<(), ResourceLayoutError> {
    let graph = &layout.carriers;
    let slot = |id: u32| {
        graph
            .slots
            .get(id as usize)
            .ok_or(ResourceLayoutError::InvalidReference)
    };
    let path = |source, path: &[NativeCarrierPath]| {
        if path
            .iter()
            .filter(|s| {
                matches!(
                    s,
                    NativeCarrierPath::List(NativeCarrierSelector::Dynamic)
                        | NativeCarrierPath::MapKey(NativeCarrierSelector::Dynamic)
                        | NativeCarrierPath::MapValue(NativeCarrierSelector::Dynamic)
                )
            })
            .count()
            > 1
        {
            return Err(ResourceLayoutError::PayloadPath);
        }
        graph.path_node(source_node(layout, source)?, path)
    };
    let source = |source: NativeCarrierSource| {
        let function = match source {
            NativeCarrierSource::Slot(id) => {
                layout
                    .frames
                    .get(slot(id)?.frame as usize)
                    .ok_or(ResourceLayoutError::InvalidReference)?
                    .site
                    .function
            }
            NativeCarrierSource::Loan(NativeCarrierLoanSource::ExistingBorrow { operation }) => {
                layout
                    .operations
                    .get(operation as usize)
                    .ok_or(ResourceLayoutError::InvalidReference)?
                    .site
                    .function
            }
            NativeCarrierSource::Loan(NativeCarrierLoanSource::IncomingViewFormal {
                scope,
                ..
            }) => {
                layout
                    .frames
                    .get(scope as usize)
                    .ok_or(ResourceLayoutError::InvalidReference)?
                    .site
                    .function
            }
        };
        if function != row.site.function {
            return Err(ResourceLayoutError::FrameMismatch);
        }
        Ok(())
    };
    match &row.operation {
        NativeCarrierOperation::Construct { children, .. } => {
            for child in children {
                match child.source {
                    NativeCarrierChildSource::Move { slot } => {
                        source(NativeCarrierSource::Slot(slot))?
                    }
                    NativeCarrierChildSource::LeafSum { slot } => {
                        let s = layout
                            .slots
                            .get(slot as usize)
                            .ok_or(ResourceLayoutError::InvalidReference)?;
                        if layout.frames[s.frame as usize].site.function != row.site.function {
                            return Err(ResourceLayoutError::FrameMismatch);
                        }
                    }
                    _ => {}
                }
            }
        }
        NativeCarrierOperation::Transfer {
            source: id,
            destination,
            ..
        }
        | NativeCarrierOperation::QualifyMachine {
            source: id,
            destination,
            ..
        } => {
            source(NativeCarrierSource::Slot(*id))?;
            source(NativeCarrierSource::Slot(*destination))?;
        }
        NativeCarrierOperation::Borrow { source: id, .. }
        | NativeCarrierOperation::Observe { source: id, .. }
        | NativeCarrierOperation::AdaptSum { source: id, .. } => source(*id)?,
        NativeCarrierOperation::EndBorrow { borrow, .. } => {
            if layout
                .operations
                .get(*borrow as usize)
                .ok_or(ResourceLayoutError::InvalidReference)?
                .site
                .function
                != row.site.function
            {
                return Err(ResourceLayoutError::FrameMismatch);
            }
        }
        NativeCarrierOperation::Extract {
            source: id,
            destination,
            ..
        } => {
            source(NativeCarrierSource::Slot(*id))?;
            if let NativeCarrierDestination::Slot(id) = destination {
                source(NativeCarrierSource::Slot(*id))?;
            }
        }
        NativeCarrierOperation::Retire { source: id, .. }
        | NativeCarrierOperation::PublishReturn { source: id, .. }
        | NativeCarrierOperation::ResetIteration { source: id, .. } => {
            source(NativeCarrierSource::Slot(*id))?
        }
    }
    match &row.operation {
        NativeCarrierOperation::Construct {
            frame,
            destination,
            selector,
            children,
        } => {
            let target = slot(*destination)?;
            if target.frame != *frame {
                return Err(ResourceLayoutError::FrameMismatch);
            }
            let mut indices = HashSet::new();
            let expected = graph.selected_children(target.node, *selector)?;
            for c in children {
                graph.node(c.node)?;
                if !indices.insert(c.index) {
                    return Err(ResourceLayoutError::NonCanonical);
                }
                match c.source {
                    NativeCarrierChildSource::Ordinary { shape } => {
                        if graph.node(c.node)? != &(NativeCarrierNode::Ordinary { shape }) {
                            return Err(ResourceLayoutError::ShapeMismatch);
                        }
                    }
                    NativeCarrierChildSource::Move { slot: id } => {
                        if slot(id)?.node != c.node {
                            return Err(ResourceLayoutError::ShapeMismatch);
                        }
                    }
                    NativeCarrierChildSource::LeafSum { slot: id } => {
                        let s = layout
                            .slots
                            .get(id as usize)
                            .ok_or(ResourceLayoutError::InvalidReference)?;
                        if !equivalent_sum(layout, c.node, s.shape) {
                            return Err(ResourceLayoutError::ShapeMismatch);
                        }
                    }
                }
            }
            match graph.node(target.node)? {
                NativeCarrierNode::List { element } => {
                    if children
                        .iter()
                        .enumerate()
                        .any(|(i, c)| c.index as usize != i || c.node != *element)
                    {
                        return Err(ResourceLayoutError::ShapeMismatch);
                    }
                }
                NativeCarrierNode::Map { key, value } => {
                    if children.len() % 2 != 0
                        || children.iter().enumerate().any(|(i, c)| {
                            c.index as usize != i
                                || c.node != if i % 2 == 0 { *key } else { *value }
                        })
                    {
                        return Err(ResourceLayoutError::ShapeMismatch);
                    }
                }
                _ => {
                    if children.len() != expected.len()
                        || children
                            .iter()
                            .any(|c| expected.get(c.index as usize) != Some(&c.node))
                    {
                        return Err(ResourceLayoutError::ShapeMismatch);
                    }
                }
            }
        }
        NativeCarrierOperation::Transfer {
            source,
            destination,
            ..
        } => {
            if source == destination || slot(*source)?.node != slot(*destination)?.node {
                return Err(ResourceLayoutError::ShapeMismatch);
            }
        }
        NativeCarrierOperation::Borrow {
            source,
            path: p,
            node,
            lease_frame,
            frame,
        } => {
            if path(*source, p)? != *node || lease_frame != frame {
                return Err(ResourceLayoutError::ShapeMismatch);
            }
        }
        NativeCarrierOperation::EndBorrow { borrow, .. } => {
            let NativeOperation::Carrier { record } = layout
                .operations
                .get(*borrow as usize)
                .ok_or(ResourceLayoutError::InvalidReference)?
                .operation
            else {
                return Err(ResourceLayoutError::OperationMismatch);
            };
            if !matches!(
                graph.operations.get(record as usize).map(|r| &r.operation),
                Some(NativeCarrierOperation::Borrow { .. })
            ) {
                return Err(ResourceLayoutError::OperationMismatch);
            }
        }
        NativeCarrierOperation::Observe {
            source,
            path: p,
            observation,
            ..
        } => {
            let n = graph.node(path(*source, p)?)?;
            let ok = match observation {
                NativeCarrierObservation::Length => matches!(
                    n,
                    NativeCarrierNode::List { .. } | NativeCarrierNode::Map { .. }
                ),
                NativeCarrierObservation::Tag => matches!(
                    n,
                    NativeCarrierNode::Optional { .. }
                        | NativeCarrierNode::Result { .. }
                        | NativeCarrierNode::Enum { .. }
                ),
                NativeCarrierObservation::State => matches!(
                    n,
                    NativeCarrierNode::Machine { .. } | NativeCarrierNode::MachineState { .. }
                ),
                NativeCarrierObservation::Ordinary { shape, clone } => {
                    if NativeCarrierObservation::ordinary_clone_policy(&layout.shapes, *shape)
                        != Some(*clone)
                    {
                        return Err(ResourceLayoutError::OperationMismatch);
                    }
                    n == &(NativeCarrierNode::Ordinary { shape: *shape })
                }
            };
            if !ok {
                return Err(ResourceLayoutError::ShapeMismatch);
            }
        }
        NativeCarrierOperation::Extract {
            source,
            path: p,
            destination,
            ..
        } => {
            let node = path(NativeCarrierSource::Slot(*source), p)?;
            match destination {
                NativeCarrierDestination::Slot(id) => {
                    if slot(*id)?.node != node {
                        return Err(ResourceLayoutError::ShapeMismatch);
                    }
                }
                NativeCarrierDestination::Ordinary { shape } => {
                    if graph.node(node)? != &(NativeCarrierNode::Ordinary { shape: *shape }) {
                        return Err(ResourceLayoutError::ShapeMismatch);
                    }
                }
            }
        }
        NativeCarrierOperation::AdaptSum {
            source,
            path: p,
            sum_slot,
            lease_frame,
            frame,
            borrowed,
        } => {
            let node = path(*source, p)?;
            let sum = layout
                .slots
                .get(*sum_slot as usize)
                .ok_or(ResourceLayoutError::InvalidReference)?;
            if lease_frame != frame
                || sum.frame != *frame
                || (!borrowed && matches!(source, NativeCarrierSource::Loan(_)))
                || !equivalent_sum(layout, node, sum.shape)
            {
                return Err(ResourceLayoutError::ShapeMismatch);
            }
        }
        NativeCarrierOperation::Retire { source, .. } => {
            slot(*source)?;
        }
        NativeCarrierOperation::PublishReturn { source, frame } => {
            let source = slot(*source)?;
            let f = &layout.frames[*frame as usize];
            let r = &layout.frames[source.frame as usize];
            if f.role != NativeFrameRole::Scope
                || r.role != NativeFrameRole::Return
                || f.site.function != r.site.function
                || f.signature != r.signature
                || !f.parents.contains(&NativeParent::Frame(source.frame))
                || !matches!(layout.shapes.get(layout.signatures[f.signature as usize].result as usize),Some(NativeShape::Carrier{node}) if *node==source.node)
            {
                return Err(ResourceLayoutError::OperationMismatch);
            }
        }
        NativeCarrierOperation::ResetIteration {
            source,
            carrier_binders,
            leaf_sum_binders,
            frame,
        } => {
            let source_slot = slot(*source)?;
            if !matches!(
                graph.node(source_slot.node)?,
                NativeCarrierNode::List { .. } | NativeCarrierNode::Map { .. }
            ) {
                return Err(ResourceLayoutError::ShapeMismatch);
            }
            let mut unique = HashSet::new();
            for id in carrier_binders {
                if *id == *source || !unique.insert(*id) || slot(*id)?.frame != *frame {
                    return Err(ResourceLayoutError::OperationMismatch);
                }
            }
            let mut unique = HashSet::new();
            for id in leaf_sum_binders {
                let leaf = layout
                    .slots
                    .get(*id as usize)
                    .ok_or(ResourceLayoutError::InvalidReference)?;
                if !unique.insert(*id)
                    || leaf.frame != *frame
                    || !matches!(
                        layout.shapes.get(leaf.shape as usize),
                        Some(NativeShape::Optional { .. } | NativeShape::Result { .. })
                    )
                {
                    return Err(ResourceLayoutError::OperationMismatch);
                }
            }
        }
        NativeCarrierOperation::QualifyMachine {
            source,
            destination,
            machine,
            state,
            ..
        } => {
            if source == destination
                || graph.node(slot(*source)?.node)?
                    != &(NativeCarrierNode::MachineState {
                        machine: *machine,
                        state: *state,
                    })
                || slot(*destination)?.node != *machine
                || !matches!(graph.node(*machine)?,NativeCarrierNode::Machine{states,..} if (*state as usize)<states.len())
            {
                return Err(ResourceLayoutError::ShapeMismatch);
            }
        }
    }
    Ok(())
}
fn map_key(layout: &WireLayout, mut node: u32) -> Result<bool, ResourceLayoutError> {
    for _ in 0..=layout.carriers.nodes.len() {
        match layout.carriers.node(node)? {
            NativeCarrierNode::Refinement { base, .. } => node = *base,
            NativeCarrierNode::Ordinary { shape } => {
                return Ok(matches!(
                    layout.shapes.get(*shape as usize),
                    Some(NativeShape::Integer { .. } | NativeShape::String | NativeShape::Bool)
                ));
            }
            _ => return Ok(false),
        }
    }
    Err(ResourceLayoutError::InvalidReference)
}
pub(crate) fn equivalent_sum(layout: &WireLayout, node: u32, shape: u32) -> bool {
    match (
        layout.carriers.node(node),
        layout.shapes.get(shape as usize),
    ) {
        (
            Ok(NativeCarrierNode::Optional { child }),
            Some(NativeShape::Optional { child: shape }),
        ) => {
            matches!((layout.carriers.node(*child),layout.shapes.get(*shape as usize)),(Ok(NativeCarrierNode::Resource{kind:a}),Some(NativeShape::Resource{kind:b})) if a==b)
        }
        (
            Ok(NativeCarrierNode::Result { ok, fail }),
            Some(NativeShape::Result {
                ok: sok,
                fail: sfail,
            }),
        ) => {
            matches!((layout.carriers.node(*ok),layout.shapes.get(*sok as usize),layout.carriers.node(*fail)),(Ok(NativeCarrierNode::Resource{kind:a}),Some(NativeShape::Resource{kind:b}),Ok(NativeCarrierNode::Ordinary{shape})) if a==b&&shape==sfail)
        }
        _ => false,
    }
}

pub(super) fn validate_formal(
    layout: &WireLayout,
    record: &NativeOperationRecord,
    caller_frame: u32,
    callee_scope: u32,
    formal: &NativeSourceFormal,
    caller_slots: &mut HashSet<u32>,
    callee_slots: &mut HashSet<u32>,
) -> Result<(), ResourceLayoutError> {
    if layout.version != 3 || formal.actual_shape != formal.callee_shape {
        return Err(ResourceLayoutError::ShapeMismatch);
    }
    let Some(NativeShape::Carrier { node }) = layout.shapes.get(formal.actual_shape as usize)
    else {
        return Err(ResourceLayoutError::ShapeMismatch);
    };
    match formal.value {
        NativeSourceValue::CarrierOwned {
            caller_argument_slot,
            callee_parameter_slot,
        } => {
            let a = layout
                .carriers
                .slots
                .get(caller_argument_slot as usize)
                .ok_or(ResourceLayoutError::InvalidReference)?;
            let b = layout
                .carriers
                .slots
                .get(callee_parameter_slot as usize)
                .ok_or(ResourceLayoutError::InvalidReference)?;
            if formal.syntax != NativeSourceSyntax::Bare
                || formal.effect != NativeSourceEffect::TransferOwned
                || formal.access != NativeAccess::Owned
                || a.node != *node
                || b.node != *node
                || a.frame != caller_frame
                || b.frame != callee_scope
                || !caller_slots.insert(caller_argument_slot)
                || !callee_slots.insert(callee_parameter_slot)
            {
                return Err(ResourceLayoutError::OperationMismatch);
            }
        }
        NativeSourceValue::CarrierView { source } => {
            if formal.access != NativeAccess::View
                || !matches!(
                    (formal.syntax, formal.effect),
                    (
                        NativeSourceSyntax::Bare,
                        NativeSourceEffect::RelinquishOwned
                    ) | (
                        NativeSourceSyntax::WrittenView,
                        NativeSourceEffect::RetainBorrow
                    )
                )
                || source_node(layout, NativeCarrierSource::Loan(source))? != *node
            {
                return Err(ResourceLayoutError::OperationMismatch);
            }
            let function = match source {
                NativeCarrierLoanSource::ExistingBorrow { operation } => {
                    layout
                        .operations
                        .get(operation as usize)
                        .ok_or(ResourceLayoutError::InvalidReference)?
                        .site
                        .function
                }
                NativeCarrierLoanSource::IncomingViewFormal { scope, .. } => {
                    layout
                        .frames
                        .get(scope as usize)
                        .ok_or(ResourceLayoutError::InvalidReference)?
                        .site
                        .function
                }
            };
            if function != record.site.function {
                return Err(ResourceLayoutError::FrameMismatch);
            }
        }
        _ => return Err(ResourceLayoutError::OperationMismatch),
    }
    Ok(())
}
pub(super) fn validate_result(
    layout: &WireLayout,
    record: &NativeOperationRecord,
    caller_frame: u32,
    callee: u32,
    signature: &NativeSignature,
    scope: &NativeFrame,
    source: &NativeSourceInvocation,
) -> Result<(), ResourceLayoutError> {
    let NativeSourceResult::Carrier {
        shape,
        caller_destination_frame,
        caller_destination_slot,
        callee_return_frame,
        permitted_return_slots,
    } = &source.result
    else {
        return Err(ResourceLayoutError::OperationMismatch);
    };
    let Some(NativeShape::Carrier { node }) = layout.shapes.get(*shape as usize) else {
        return Err(ResourceLayoutError::ShapeMismatch);
    };
    let destination = layout
        .carriers
        .slots
        .get(*caller_destination_slot as usize)
        .ok_or(ResourceLayoutError::InvalidReference)?;
    let return_frame = layout
        .frames
        .get(*callee_return_frame as usize)
        .ok_or(ResourceLayoutError::InvalidReference)?;
    let destination_frame = layout
        .frames
        .get(*caller_destination_frame as usize)
        .ok_or(ResourceLayoutError::InvalidReference)?;
    if layout.version != 3
        || *shape != signature.result
        || source.callee_return != Some(*callee_return_frame)
        || return_frame.role != NativeFrameRole::Return
        || return_frame.site.function != callee
        || return_frame.signature != scope.signature
        || !return_frame
            .parents
            .contains(&NativeParent::Frame(caller_frame))
        || !scope
            .parents
            .contains(&NativeParent::Frame(*callee_return_frame))
        || destination.frame != *caller_destination_frame
        || destination.node != *node
        || destination_frame.site.function != record.site.function
        || permitted_return_slots.is_empty()
        || permitted_return_slots.windows(2).any(|p| p[0] >= p[1])
    {
        return Err(ResourceLayoutError::OperationMismatch);
    }
    for id in permitted_return_slots {
        let s = layout
            .carriers
            .slots
            .get(*id as usize)
            .ok_or(ResourceLayoutError::InvalidReference)?;
        if s.frame != *callee_return_frame || s.node != *node {
            return Err(ResourceLayoutError::ShapeMismatch);
        }
    }
    Ok(())
}
