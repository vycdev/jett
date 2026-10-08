//! Independent current-CFG zero-leaf custody proof. IDs describe exact sealed
//! endpoints, never values that can grant native permission on their own.
use super::*;
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Origin {
    Formal(usize),
    Constructor(ResourceCarrierOperationId),
    Extraction(ResourceCarrierOperationId),
    Source(ResourceOperationId),
    LeafAbsence(ResourceOperationId),
}
#[derive(Debug, Clone, PartialEq, Eq)]
enum Slot {
    Vacant,
    Building {
        origin: Origin,
        expected: Vec<ResourceCarrierShapeId>,
        order: Vec<usize>,
        arrived: usize,
    },
    Live {
        origin: Origin,
        taken: Vec<Vec<ResourceCarrierProjectionPath>>,
    },
    Moved,
}
#[derive(Debug, Clone, PartialEq, Eq)]
enum Leaf {
    Vacant,
    Empty {
        origin: Origin,
    },
    Adapter {
        operation: ResourceCarrierOperationId,
    },
    Moved,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Root {
    Slot(ResourceCarrierSlotId),
    Incoming(ResourceCarrierLoanId),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Anchor {
    root: Root,
    origin: Origin,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Selector {
    Sum(bool),
    Variant(usize),
    State(usize),
    Iteration(BlockId),
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct Selection {
    anchor: Anchor,
    path: Vec<ResourceCarrierProjectionPath>,
    selector: Selector,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct Probe {
    anchor: Anchor,
    path: Vec<ResourceCarrierProjectionPath>,
    shape: ResourceCarrierShapeId,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct State {
    slots: Vec<Slot>,
    leaves: Vec<Leaf>,
    loans: BTreeSet<ResourceCarrierLoanId>,
    leaf_loans: BTreeSet<ResourceLoanId>,
    frames: Vec<ResourceFrameId>,
    returned: bool,
    selections: Vec<Selection>,
    tags: BTreeMap<u32, Probe>,
    switches: BTreeMap<u32, Probe>,
    retired_projection: Option<(ResourceCarrierLoanId, ResourceCarrierSlotId, Origin)>,
}
struct Analysis<'a> {
    function: &'a Function,
    plan: &'a ResourceFunctionPlan,
    carriers: &'a ResourceCarrierFunctionPlan,
}
impl Analysis<'_> {
    fn frame(&self, state: &mut State, frame: ResourceFrameId) -> Result<(), String> {
        if frame == ResourceFrameId(0) {
            if state.frames.last().copied() != Some(frame) {
                return Err("carrier operation lost its exact live Scope prefix".into());
            }
            return Ok(());
        }
        let row = self
            .plan
            .frames
            .get(frame.index())
            .filter(|row| row.id == frame)
            .ok_or("carrier operation frame missing")?;
        if row.role == ResourceFrameRole::Return {
            if row.parent.is_some() {
                return Err("carrier provisional Return frame has a foreign parent".into());
            }
            return Ok(());
        }
        if state.frames.last().copied() == Some(frame) {
            return Ok(());
        }
        let parent = row
            .parent
            .ok_or("carrier Operation frame lost its parent")?;
        if state.frames.last().copied() != Some(parent) {
            return Err("carrier operation changed its exact frame prefix".into());
        }
        state.frames.push(frame);
        Ok(())
    }
    fn live<'s>(&self, state: &'s State, slot: ResourceCarrierSlotId) -> Result<&'s Slot, String> {
        let value = state
            .slots
            .get(slot.index())
            .ok_or("carrier slot outside its exact plan")?;
        if !matches!(value, Slot::Live { .. }) {
            return Err("carrier source is uninitialized, moved, or incompletely constructed at this CFG site".into());
        }
        Ok(value)
    }
    fn root(&self, loan: ResourceCarrierLoanId) -> Result<Option<ResourceCarrierSlotId>, String> {
        let mut current = loan;
        let mut seen = BTreeSet::new();
        loop {
            if !seen.insert(current) {
                return Err("carrier loan parent graph is cyclic".into());
            }
            let row = self
                .carriers
                .loans
                .get(current.index())
                .filter(|row| row.id == current)
                .ok_or("carrier loan missing")?;
            match row.source {
                ResourceCarrierLoanSource::Slot(slot) => return Ok(Some(slot)),
                ResourceCarrierLoanSource::Loan(parent) => current = parent,
                ResourceCarrierLoanSource::IncomingViewFormal { .. } => return Ok(None),
            }
        }
    }
    fn unleased(&self, state: &State, slot: ResourceCarrierSlotId) -> Result<(), String> {
        for loan in &state.loans {
            if self.root(*loan)? == Some(slot) {
                return Err(
                    "carrier move/retirement has an exact live parent or child lease".into(),
                );
            }
        }
        for loan in &state.leaf_loans {
            if let ResourceLoanSource::CarrierSumProjection { operation } =
                self.plan.loans[loan.index()].source
            {
                let ResourceCarrierOperationRole::AdaptSum { source, .. } =
                    self.carriers.operations[operation.index()].role
                else {
                    return Err("carrier leaf loan lost its exact adapter role".into());
                };
                if self.root(source)? == Some(slot) {
                    return Err("carrier move/retirement has a live projected sum lease".into());
                }
            }
        }
        Ok(())
    }
    fn install(
        &self,
        state: &mut State,
        slot: ResourceCarrierSlotId,
        origin: Origin,
    ) -> Result<(), String> {
        if !matches!(
            state.slots.get(slot.index()),
            Some(Slot::Vacant | Slot::Moved)
        ) {
            return Err(
                "carrier destination is already live outside an authenticated binder reset".into(),
            );
        }
        state.slots[slot.index()] = Slot::Live {
            origin,
            taken: Vec::new(),
        };
        Ok(())
    }
    fn take(&self, state: &mut State, slot: ResourceCarrierSlotId) -> Result<Slot, String> {
        self.live(state, slot)?;
        self.unleased(state, slot)?;
        state
            .selections
            .retain(|fact| fact.anchor.root != Root::Slot(slot));
        state
            .tags
            .retain(|_, probe| probe.anchor.root != Root::Slot(slot));
        state
            .switches
            .retain(|_, probe| probe.anchor.root != Root::Slot(slot));
        Ok(std::mem::replace(
            &mut state.slots[slot.index()],
            Slot::Moved,
        ))
    }
    fn take_complete(
        &self,
        state: &mut State,
        slot: ResourceCarrierSlotId,
    ) -> Result<Slot, String> {
        if !matches!(self.live(state, slot)?, Slot::Live { taken, .. } if taken.is_empty()) {
            return Err("carrier whole-root transfer, adoption, Source transport or publication has extracted children".into());
        }
        self.take(state, slot)
    }
    fn view_complete(&self, state: &State, loan: ResourceCarrierLoanId) -> Result<(), String> {
        self.loan_live(state, loan)?;
        if let Some(root) = self.root(loan)? {
            let path = self.absolute_path(loan, &[])?;
            let Slot::Live { taken, .. } = self.live(state, root)? else {
                return Err("carrier view Source root is not installed".into());
            };
            if taken
                .iter()
                .any(|prior| prior.starts_with(&path) || path.starts_with(prior))
            {
                return Err(
                    "carrier view Source formal cannot receive an incomplete selected subtree"
                        .into(),
                );
            }
        }
        Ok(())
    }
    fn loan_live(&self, state: &State, loan: ResourceCarrierLoanId) -> Result<(), String> {
        if !state.loans.contains(&loan) {
            return Err(
                "carrier observation/projection uses an ended or unreached exact loan".into(),
            );
        }
        if let Some(root) = self.root(loan)? {
            self.live(state, root)?;
        }
        if self.projected_shape(state, loan, &[])? != self.carriers.loans[loan.index()].shape {
            return Err(
                "carrier active loan changed its exact parent generation/path shape".into(),
            );
        }
        Ok(())
    }
    fn absolute_path(
        &self,
        loan: ResourceCarrierLoanId,
        suffix: &[ResourceCarrierProjectionPath],
    ) -> Result<Vec<ResourceCarrierProjectionPath>, String> {
        let mut ancestors = Vec::new();
        let mut current = loan;
        let mut seen = BTreeSet::new();
        loop {
            if !seen.insert(current) {
                return Err("carrier path parent graph is cyclic".into());
            }
            let row = self
                .carriers
                .loans
                .get(current.index())
                .ok_or("carrier path loan missing")?;
            ancestors.push(row.path.clone());
            match row.source {
                ResourceCarrierLoanSource::Loan(parent) => current = parent,
                ResourceCarrierLoanSource::Slot(_)
                | ResourceCarrierLoanSource::IncomingViewFormal { .. } => break,
            }
        }
        let mut path = Vec::new();
        for ancestor in ancestors.into_iter().rev() {
            path.extend(ancestor);
        }
        path.extend_from_slice(suffix);
        Ok(path)
    }
    fn anchor(&self, state: &State, loan: ResourceCarrierLoanId) -> Result<Anchor, String> {
        let mut current = loan;
        let mut seen = BTreeSet::new();
        loop {
            if !seen.insert(current) {
                return Err("carrier anchor loan graph is cyclic".into());
            }
            let row = self
                .carriers
                .loans
                .get(current.index())
                .ok_or("carrier anchor loan missing")?;
            match row.source {
                ResourceCarrierLoanSource::Slot(slot) => {
                    let Slot::Live { origin, .. } = self.live(state, slot)? else {
                        return Err("carrier anchor is not live".into());
                    };
                    return Ok(Anchor {
                        root: Root::Slot(slot),
                        origin: *origin,
                    });
                }
                ResourceCarrierLoanSource::Loan(parent) => current = parent,
                ResourceCarrierLoanSource::IncomingViewFormal { parameter, .. } => {
                    return Ok(Anchor {
                        root: Root::Incoming(current),
                        origin: Origin::Formal(parameter),
                    });
                }
            }
        }
    }
    fn selection(
        &self,
        state: &State,
        anchor: Anchor,
        path: &[ResourceCarrierProjectionPath],
        selector: Selector,
    ) -> bool {
        state
            .selections
            .iter()
            .any(|fact| fact.anchor == anchor && fact.path == path && fact.selector == selector)
    }
    fn select(&self, state: &mut State, probe: &Probe, selector: Selector) {
        state
            .selections
            .retain(|fact| fact.anchor != probe.anchor || fact.path != probe.path);
        state.selections.push(Selection {
            anchor: probe.anchor,
            path: probe.path.clone(),
            selector,
        });
    }
    fn projected_shape(
        &self,
        state: &State,
        loan: ResourceCarrierLoanId,
        suffix: &[ResourceCarrierProjectionPath],
    ) -> Result<ResourceCarrierShapeId, String> {
        let anchor = self.anchor(state, loan)?;
        let mut current = match anchor.root {
            Root::Slot(slot) => self.carriers.slots[slot.index()].shape,
            Root::Incoming(loan) => self.carriers.loans[loan.index()].shape,
        };
        let path = self.absolute_path(loan, suffix)?;
        if let Root::Slot(slot) = anchor.root {
            let Slot::Live { taken, .. } = self.live(state, slot)? else {
                return Err("carrier path root is not installed".into());
            };
            if taken.iter().any(|prior| path.starts_with(prior)) {
                return Err("carrier projection reads an already extracted selected path".into());
            }
        }
        let mut prefix = Vec::new();
        for part in &path {
            let shape = self
                .carriers
                .shape(current)
                .ok_or("carrier selected path graph row missing")?;
            current = match (part, &shape.node) {
                (ResourceCarrierProjectionPath::Field { owner, field }, ResourceCarrierNode::Struct { fields, .. }) if *owner == shape.ty => fields.get(field.index() as usize).ok_or("carrier field outside its exact nominal definition")?.shape,
                (ResourceCarrierProjectionPath::Variant { owner, variant, field }, ResourceCarrierNode::Enum { variants, .. }) if *owner == shape.ty => {
                    if !self.selection(state, anchor, &prefix, Selector::Variant(variant.index() as usize)) { return Err("carrier variant projection lacks its exact current selecting Switch edge".into()); }
                    variants.get(variant.index() as usize).and_then(|variant| variant.fields.get(*field)).ok_or("carrier variant payload outside its exact graph")?.shape
                }
                (ResourceCarrierProjectionPath::State { owner, state: selected, field }, ResourceCarrierNode::MachineState { machine, state: actual }) if *owner == shape.ty && selected == actual => {
                    let ResourceCarrierNode::Machine { states, .. } = &self.carriers.shape(*machine).ok_or("carrier machine graph missing")?.node else { return Err("carrier state owner is not a machine".into()); };
                    states.get(*selected).and_then(|state| state.fields.get(*field)).ok_or("carrier state field outside its exact graph")?.shape
                }
                (ResourceCarrierProjectionPath::State { owner, state: selected, field }, ResourceCarrierNode::Machine { states, .. }) => {
                    let qualified = self.carriers.shape_for_type(*owner).ok_or("carrier field qualification graph missing")?;
                    if !matches!(qualified.node, ResourceCarrierNode::MachineState { machine, state } if machine == current && state == *selected) || !self.selection(state, anchor, &prefix, Selector::State(*selected)) { return Err("carrier state projection lacks its exact current StateIs edge/owner qualification".into()); }
                    states.get(*selected).and_then(|state| state.fields.get(*field)).ok_or("carrier state field outside its exact graph")?.shape
                }
                (ResourceCarrierProjectionPath::OptionalSome, ResourceCarrierNode::Optional { payload }) => {
                    if !self.selection(state, anchor, &prefix, Selector::Sum(true)) { return Err("carrier Optional payload lacks its exact selecting typed SumTag edge".into()); }
                    *payload
                }
                (ResourceCarrierProjectionPath::ResultOk, ResourceCarrierNode::Result { success, .. }) => {
                    if !self.selection(state, anchor, &prefix, Selector::Sum(true)) { return Err("carrier Result ok payload lacks its exact selecting typed SumTag edge".into()); }
                    *success
                }
                (ResourceCarrierProjectionPath::ResultFail, ResourceCarrierNode::Result { failure, .. }) => {
                    if !self.selection(state, anchor, &prefix, Selector::Sum(false)) { return Err("carrier Result fail payload lacks its exact selecting typed SumTag edge".into()); }
                    *failure
                }
                (ResourceCarrierProjectionPath::Element { index }, ResourceCarrierNode::List { element }) => { self.iteration_index(state, anchor, &prefix, *index)?; *element }
                (ResourceCarrierProjectionPath::MapKey { index }, ResourceCarrierNode::Map { key, .. }) => { self.iteration_index(state, anchor, &prefix, *index)?; *key }
                (ResourceCarrierProjectionPath::MapValue { index }, ResourceCarrierNode::Map { value, .. }) => { self.iteration_index(state, anchor, &prefix, *index)?; *value }
                _ => return Err("carrier selected path changes its exact graph family, nominal owner or qualification".into()),
            };
            prefix.push(part.clone());
        }
        Ok(current)
    }
    fn iteration_index(
        &self,
        state: &State,
        anchor: Anchor,
        path: &[ResourceCarrierProjectionPath],
        index: ResourceCarrierIndex,
    ) -> Result<(), String> {
        let Root::Slot(slot) = anchor.root else {
            return Err(
                "pending Resource carrier: borrowed iterator index has no owning loop root".into(),
            );
        };
        let row = self
            .carriers
            .iterations
            .iter()
            .find(|row| {
                row.source == slot
                    && match index {
                        ResourceCarrierIndex::Local(cursor) => row.cursor == Some(cursor),
                        ResourceCarrierIndex::CurrentIteration { header } => {
                            row.cursor.is_none() && row.header == header
                        }
                        ResourceCarrierIndex::Constant(_) => false,
                    }
            })
            .ok_or("carrier dynamic index is outside its exact original/current iterator cursor")?;
        if !path.is_empty() || !self.selection(state, anchor, path, Selector::Iteration(row.header))
        {
            return Err(
                "carrier indexed extraction lacks its exact selected current iterator body edge"
                    .into(),
            );
        }
        Ok(())
    }
    fn probe(&self, state: &State, loan: ResourceCarrierLoanId) -> Result<Probe, String> {
        Ok(Probe {
            anchor: self.anchor(state, loan)?,
            path: self.absolute_path(loan, &[])?,
            shape: self.projected_shape(state, loan, &[])?,
        })
    }
    fn extract(
        &self,
        state: &mut State,
        source: ResourceCarrierLoanId,
        path: &[ResourceCarrierProjectionPath],
    ) -> Result<(), String> {
        let root = self
            .root(source)?
            .ok_or("carrier owning extraction cannot consume an incoming view")?;
        let origin = match self.live(state, root)? {
            Slot::Live { origin, .. } => *origin,
            _ => return Err("carrier extraction lost its installed root".into()),
        };
        if state.retired_projection.take() != Some((source, root, origin)) {
            return Err("carrier owning extraction lacks its immediately retired exact projection lease/generation".into());
        }
        self.unleased(state, root)?;
        self.projected_shape(state, source, path)?;
        let key = self.absolute_path(source, path)?;
        let Slot::Live { taken, .. } = &mut state.slots[root.index()] else {
            return Err("carrier extraction root is not installed".into());
        };
        if taken
            .iter()
            .any(|prior| prior.starts_with(&key) || key.starts_with(prior))
        {
            return Err("carrier selected path was already extracted at this generation".into());
        }
        taken.push(key);
        Ok(())
    }
    fn constructor(
        &self,
        operation: &ResourceCarrierOperation,
        destination: ResourceCarrierSlotId,
        constructor: &ResourceCarrierConstructor,
    ) -> Result<(Vec<ResourceCarrierShapeId>, Vec<usize>), String> {
        let shape = self
            .carriers
            .shape(self.carriers.slots[destination.index()].shape)
            .ok_or("carrier constructor graph row missing")?;
        let mut dynamic = None;
        let mut written_order = None;
        let block = self
            .function
            .blocks
            .get(operation.site.block.index() as usize)
            .ok_or("carrier constructor site block missing")?;
        walk::mir_block(block, &mut |expression| {
            if !operation.is_for_expression(expression) {
                return;
            }
            match &expression.kind {
                E::ListConstruct { elements } => dynamic = Some(elements.len()),
                E::MapConstruct { entries } => dynamic = entries.len().checked_mul(2),
                E::StructConstruct {
                    evaluation_order, ..
                }
                | E::EnumConstruct {
                    evaluation_order, ..
                } => written_order = Some(evaluation_order.clone()),
                _ => {}
            }
        });
        let expected = match (constructor, &shape.node) {
            (ResourceCarrierConstructor::List, ResourceCarrierNode::List { element }) => {
                vec![
                    *element;
                    dynamic.ok_or("carrier list constructor lost its exact current arity")?
                ]
            }
            (ResourceCarrierConstructor::Map, ResourceCarrierNode::Map { key, value }) => (0
                ..dynamic.ok_or("carrier map constructor lost its exact current arity")?)
                .map(|index| if index % 2 == 0 { *key } else { *value })
                .collect(),
            (ResourceCarrierConstructor::Struct, ResourceCarrierNode::Struct { fields, .. }) => {
                fields.iter().map(|field| field.shape).collect()
            }
            (
                ResourceCarrierConstructor::Enum { variant },
                ResourceCarrierNode::Enum { variants, .. },
            ) => variants
                .get(variant.index() as usize)
                .ok_or("carrier constructor variant outside its exact graph")?
                .fields
                .iter()
                .map(|field| field.shape)
                .collect(),
            (
                ResourceCarrierConstructor::Machine { state },
                ResourceCarrierNode::Machine { states, .. },
            ) => states
                .get(*state)
                .ok_or("carrier constructor state outside its exact graph")?
                .fields
                .iter()
                .map(|field| field.shape)
                .collect(),
            (
                ResourceCarrierConstructor::Machine { state },
                ResourceCarrierNode::MachineState {
                    machine,
                    state: selected,
                },
            ) if state == selected => {
                let ResourceCarrierNode::Machine { states, .. } = &self
                    .carriers
                    .shape(*machine)
                    .ok_or("carrier qualified machine graph missing")?
                    .node
                else {
                    return Err("carrier qualified owner is not a machine".into());
                };
                states
                    .get(*state)
                    .ok_or("carrier qualified state outside its exact graph")?
                    .fields
                    .iter()
                    .map(|field| field.shape)
                    .collect()
            }
            (ResourceCarrierConstructor::OptionalNone, ResourceCarrierNode::Optional { .. }) => {
                Vec::new()
            }
            (
                ResourceCarrierConstructor::OptionalSome,
                ResourceCarrierNode::Optional { payload },
            ) => vec![*payload],
            (ResourceCarrierConstructor::ResultOk, ResourceCarrierNode::Result { success, .. }) => {
                vec![*success]
            }
            (
                ResourceCarrierConstructor::ResultFail,
                ResourceCarrierNode::Result { failure, .. },
            ) => vec![*failure],
            _ => {
                return Err(
                    "carrier constructor role differs from its exact typed graph family".into(),
                );
            }
        };
        let order = written_order.unwrap_or_else(|| (0..expected.len()).collect());
        if order.len() != expected.len()
            || order.iter().copied().collect::<BTreeSet<_>>() != (0..expected.len()).collect()
        {
            return Err(
                "carrier constructor has a noncanonical lexical arrival permutation".into(),
            );
        }
        Ok((expected, order))
    }
    fn carrier(
        &self,
        state: &mut State,
        operation: &ResourceCarrierOperation,
    ) -> Result<(), String> {
        if !matches!(
            operation.role,
            ResourceCarrierOperationRole::PublishReturn { .. }
        ) {
            self.frame(state, operation.frame)?;
        }
        if !matches!(
            operation.role,
            ResourceCarrierOperationRole::Extract { .. }
                | ResourceCarrierOperationRole::ExtractOrdinary { .. }
                | ResourceCarrierOperationRole::AdaptSum { loan: None, .. }
        ) {
            state.retired_projection = None;
        }
        match &operation.role {
            ResourceCarrierOperationRole::BeginConstructor {
                destination,
                constructor,
            } => {
                if !matches!(state.slots[destination.index()], Slot::Vacant | Slot::Moved) {
                    return Err("carrier constructor overwrites a live root".into());
                }
                let (expected, order) = self.constructor(operation, *destination, constructor)?;
                state.slots[destination.index()] = Slot::Building {
                    origin: Origin::Constructor(operation.id),
                    expected,
                    order,
                    arrived: 0,
                };
            }
            ResourceCarrierOperationRole::ConstructorChild { destination, child } => {
                let Slot::Building {
                    expected,
                    order,
                    arrived,
                    ..
                } = &state.slots[destination.index()]
                else {
                    return Err("carrier constructor child precedes its exact Begin".into());
                };
                if order.get(*arrived) != Some(&child.index)
                    || expected.get(child.index) != Some(&child.shape)
                {
                    return Err("carrier constructor child changes its exact lexical prefix or declared graph node".into());
                }
                match child.value {
                    ResourceCarrierValue::Ordinary { ty } => {
                        let shape = self
                            .carriers
                            .shape(child.shape)
                            .ok_or("carrier ordinary child graph missing")?;
                        if shape.ty != ty || shape.contains_resource {
                            return Err("carrier constructor cannot adopt custody through an ordinary payload channel".into());
                        }
                    }
                    ResourceCarrierValue::Owned { slot } => {
                        if self.carriers.slots[slot.index()].shape != child.shape {
                            return Err("carrier constructor adoption changes its child's exact installed graph node".into());
                        }
                        self.take_complete(state, slot)?;
                    }
                    ResourceCarrierValue::LeafOwned { slot } => {
                        if !matches!(state.leaves.get(slot.index()), Some(Leaf::Empty { .. })) {
                            return Err("pending Resource carrier: selected aggregate child lacks a proven zero-leaf producer".into());
                        }
                        if state.leaf_loans.iter().any(|loan| {
                            self.plan.loans[loan.index()].source == ResourceLoanSource::Owner(slot)
                        }) {
                            return Err(
                                "carrier child adoption has an exact live leaf lease".into()
                            );
                        }
                        state.leaves[slot.index()] = Leaf::Moved;
                    }
                    _ => return Err("carrier constructor cannot adopt any borrowed child".into()),
                }
                let Slot::Building { arrived, .. } = &mut state.slots[destination.index()] else {
                    return Err("carrier constructor child lost its arrived prefix".into());
                };
                *arrived += 1;
            }
            ResourceCarrierOperationRole::CommitConstructor { destination } => {
                let Slot::Building {
                    origin,
                    expected,
                    arrived,
                    ..
                } = &state.slots[destination.index()]
                else {
                    return Err("carrier constructor Commit lacks its exact arrived prefix".into());
                };
                if *arrived != expected.len() {
                    return Err(
                        "carrier constructor Commit precedes its complete exact acquired prefix"
                            .into(),
                    );
                }
                state.slots[destination.index()] = Slot::Live {
                    origin: *origin,
                    taken: Vec::new(),
                };
            }
            ResourceCarrierOperationRole::Transfer {
                source,
                destination,
            } => {
                let value = self.take_complete(state, *source)?;
                if !matches!(state.slots[destination.index()], Slot::Vacant | Slot::Moved) {
                    return Err("carrier transfer overwrites a live destination".into());
                }
                state.slots[destination.index()] = value;
            }
            ResourceCarrierOperationRole::QualifyMachine {
                source,
                destination,
                machine,
                state: selected,
            } => {
                let source_node = &self
                    .carriers
                    .shape(self.carriers.slots[source.index()].shape)
                    .ok_or("carrier qualification source graph row missing")?
                    .node;
                if !matches!(source_node, ResourceCarrierNode::MachineState { machine: owner, state } if owner == machine && state == selected)
                    || self.carriers.slots[destination.index()].shape != *machine
                {
                    return Err(
                        "carrier qualification changes its exact machine owner or checked state"
                            .into(),
                    );
                }
                let value = self.take_complete(state, *source)?;
                if !matches!(state.slots[destination.index()], Slot::Vacant | Slot::Moved) {
                    return Err("carrier qualification overwrites a live destination".into());
                }
                state.slots[destination.index()] = value;
            }
            ResourceCarrierOperationRole::Borrow { loan } => {
                let row = self
                    .carriers
                    .loans
                    .get(loan.index())
                    .ok_or("carrier Borrow lost its exact loan row")?;
                match row.source {
                    ResourceCarrierLoanSource::Slot(slot) => { self.live(state, slot)?; }
                    ResourceCarrierLoanSource::Loan(parent) => self.loan_live(state, parent)?,
                    ResourceCarrierLoanSource::IncomingViewFormal { .. } => return Err("carrier incoming formal is a resident caller lease, not a child Borrow activation".into()),
                }
                if self.projected_shape(state, *loan, &[])? != row.shape {
                    return Err("carrier Borrow declared graph node differs from its exact parent/path projection".into());
                }
                if !state.loans.insert(*loan) {
                    return Err("carrier Borrow repeats a live lease".into());
                }
            }
            ResourceCarrierOperationRole::EndBorrow { loan } => {
                if state.loans.iter().any(|child| {
                    self.carriers.loans[child.index()].source
                        == ResourceCarrierLoanSource::Loan(*loan)
                }) {
                    return Err("carrier parent lease ends before its exact child".into());
                }
                if state.leaf_loans.iter().any(|child| matches!(self.plan.loans[child.index()].source,
                    ResourceLoanSource::CarrierSumProjection { operation } if matches!(self.carriers.operations[operation.index()].role, ResourceCarrierOperationRole::AdaptSum { source, .. } if source == *loan))) { return Err("carrier parent lease ends before its sum adapter child".into()); }
                if !state.loans.remove(loan) {
                    return Err("carrier EndBorrow has no reached activation".into());
                }
                if let Some(root) = self.root(*loan)? {
                    if let Slot::Live { origin, .. } = self.live(state, root)? {
                        state.retired_projection = Some((*loan, root, *origin));
                    }
                }
            }
            ResourceCarrierOperationRole::Observe {
                source,
                observation,
                target,
            } => {
                self.loan_live(state, *source)?;
                let probe = self.probe(state, *source)?;
                let node = &self
                    .carriers
                    .shape(probe.shape)
                    .ok_or("carrier observation graph row missing")?
                    .node;
                match observation {
                    ResourceCarrierObservation::Length
                        if matches!(
                            node,
                            ResourceCarrierNode::List { .. } | ResourceCarrierNode::Map { .. }
                        ) => {}
                    ResourceCarrierObservation::Tag
                        if matches!(
                            node,
                            ResourceCarrierNode::Optional { .. }
                                | ResourceCarrierNode::Result { .. }
                                | ResourceCarrierNode::Enum { .. }
                        ) =>
                    {
                        if let Some(target) = target {
                            state.tags.insert(target.index(), probe);
                        } else if matches!(
                            self.function.blocks[operation.site.block.index() as usize]
                                .terminator
                                .kind,
                            TerminatorKind::Switch { .. }
                        ) {
                            state.switches.insert(operation.site.block.index(), probe);
                        }
                    }
                    ResourceCarrierObservation::State { state: selected } => {
                        let machine = match node {
                            ResourceCarrierNode::Machine { states, .. } => states,
                            ResourceCarrierNode::MachineState { machine, .. } => match &self
                                .carriers
                                .shape(*machine)
                                .ok_or("carrier state observation graph owner missing")?
                                .node
                            {
                                ResourceCarrierNode::Machine { states, .. } => states,
                                _ => {
                                    return Err(
                                        "carrier state observation owner is not a machine".into()
                                    );
                                }
                            },
                            _ => {
                                return Err(
                                    "carrier state observation has a different graph family".into(),
                                );
                            }
                        };
                        if *selected >= machine.len() {
                            return Err(
                                "carrier StateIs selector outside its exact machine graph".into()
                            );
                        }
                    }
                    ResourceCarrierObservation::Ordinary { path, ty } => {
                        let shape = self
                            .carriers
                            .shape(self.projected_shape(state, *source, path)?)
                            .ok_or("carrier ordinary projection row missing")?;
                        if shape.ty != *ty || shape.contains_resource {
                            return Err(
                                "carrier observation cannot expose custody as ordinary payload"
                                    .into(),
                            );
                        }
                    }
                    _ => {
                        return Err(
                            "carrier observation role changes its exact typed graph family".into(),
                        );
                    }
                }
            }
            ResourceCarrierOperationRole::Project {
                source,
                destination,
            } => {
                self.loan_live(state, *source)?;
                self.loan_live(state, *destination)?;
            }
            ResourceCarrierOperationRole::Extract {
                source,
                path,
                destination,
            } => {
                if self.projected_shape(state, *source, path)?
                    != self.carriers.slots[destination.index()].shape
                {
                    return Err("carrier extraction changes its exact selected graph node".into());
                }
                self.extract(state, *source, path)?;
                self.install(state, *destination, Origin::Extraction(operation.id))?;
            }
            ResourceCarrierOperationRole::ExtractOrdinary {
                source, path, ty, ..
            } => {
                let shape = self
                    .carriers
                    .shape(self.projected_shape(state, *source, path)?)
                    .ok_or("carrier ordinary extraction graph row missing")?;
                if shape.ty != *ty || shape.contains_resource {
                    return Err(
                        "carrier extraction cannot export custody through ordinary payload storage"
                            .into(),
                    );
                }
                self.extract(state, *source, path)?;
            }
            ResourceCarrierOperationRole::AdaptSum {
                source,
                path,
                destination,
                loan,
                ..
            } => {
                let projected = self
                    .carriers
                    .shape(self.projected_shape(state, *source, path)?)
                    .ok_or("carrier sum adapter graph row missing")?;
                let expected = match &projected.node {
                    ResourceCarrierNode::Optional { payload } => match &self.carriers.shape(*payload).ok_or("carrier sum payload graph missing")?.node { ResourceCarrierNode::Resource { kind } => ResourceShape::Optional { kind: kind.clone() }, _ => return Err("carrier adapter Optional does not have a sole exact legacy Resource leaf".into()) },
                    ResourceCarrierNode::Result { success, failure } => match &self.carriers.shape(*success).ok_or("carrier sum success graph missing")?.node { ResourceCarrierNode::Resource { kind } => ResourceShape::Result { kind: kind.clone(), failure: self.carriers.shape(*failure).ok_or("carrier sum failure graph missing")?.ty }, _ => return Err("carrier adapter Result does not have a sole exact legacy Resource leaf".into()) },
                    _ => return Err("carrier adapter selected graph node is not an existing leaf sum".into()),
                };
                if self.plan.slots[destination.index()].shape != expected {
                    return Err(
                        "carrier adapter changes its exact legacy sum kind/failure shape".into(),
                    );
                }
                if loan.is_some() {
                    self.loan_live(state, *source)?;
                } else {
                    self.extract(state, *source, path)?;
                }
                if !matches!(
                    state.leaves[destination.index()],
                    Leaf::Vacant | Leaf::Moved
                ) {
                    return Err(
                        "carrier sum adapter overwrites an installed shell outside a binder reset"
                            .into(),
                    );
                }
                state.leaves[destination.index()] = if loan.is_some() {
                    Leaf::Adapter {
                        operation: operation.id,
                    }
                } else {
                    Leaf::Empty {
                        origin: Origin::Extraction(operation.id),
                    }
                };
            }
            ResourceCarrierOperationRole::Retire { source } => {
                self.take(state, *source)?;
            }
            ResourceCarrierOperationRole::PublishReturn { source } => {
                self.live(state, *source)?;
                if !state.returned
                    || !state.frames.is_empty()
                    || self.plan.provisional_return().map(ResourceFrame::id)
                        != Some(self.carriers.slots[source.index()].frame)
                {
                    return Err(
                        "carrier Return publication precedes exact successful Scope cleanup".into(),
                    );
                }
                self.take_complete(state, *source)?;
            }
            ResourceCarrierOperationRole::RetireIteration { source, binders } => {
                self.reset_iteration(state, *source, binders)?
            }
            ResourceCarrierOperationRole::BeginIteration {
                source,
                destination,
                ..
            } => {
                let ResourceCarrierValue::Owned { slot } = source else {
                    return Err("carrier consuming iterator lacks its original owning root".into());
                };
                let value = self.take_complete(state, *slot)?;
                if !matches!(state.slots[destination.index()], Slot::Vacant | Slot::Moved) {
                    return Err("carrier iterator root installs twice".into());
                }
                state.slots[destination.index()] = value;
            }
        }
        Ok(())
    }
    fn reset_iteration(
        &self,
        state: &mut State,
        source: ResourceCarrierSlotId,
        binders: &[LocalId],
    ) -> Result<(), String> {
        self.live(state, source)?;
        self.unleased(state, source)?;
        state.selections.retain(|fact| {
            fact.anchor.root != Root::Slot(source)
                || !matches!(fact.selector, Selector::Iteration(_))
        });
        for binder in binders {
            for slot in &self.carriers.slots {
                if matches!(&slot.storage, ResourceSlotStorage::Local { header } if header.id == *binder)
                {
                    self.unleased(state, slot.id)?;
                    if matches!(state.slots[slot.id.index()], Slot::Building { .. }) {
                        return Err(
                            "carrier iteration binder has an incomplete arrived prefix".into()
                        );
                    }
                    state.slots[slot.id.index()] = Slot::Vacant;
                }
            }
            for slot in &self.plan.slots {
                if matches!(&slot.storage, ResourceSlotStorage::Local { header } if header.id == *binder)
                {
                    if state.leaf_loans.iter().any(|loan| {
                        self.plan.loans[loan.index()].source == ResourceLoanSource::Owner(slot.id)
                    }) {
                        return Err("carrier iteration binder reset has a live sum lease".into());
                    }
                    state.leaves[slot.id.index()] = Leaf::Vacant;
                }
            }
        }
        let Slot::Live { taken, .. } = &mut state.slots[source.index()] else {
            return Err("carrier iterator reset lost its installed root".into());
        };
        // Only the exact canonical iterator cursor advances these dynamic paths.
        // The physical runtime retains holes and generation checks for every index.
        let iteration = self
            .carriers
            .iterations
            .iter()
            .find(|row| row.source == source && row.binders == binders)
            .ok_or("carrier iteration reset has no exact original/current loop association")?;
        taken.retain(|path| {
            !path.iter().any(|part| match part {
                ResourceCarrierProjectionPath::Element { index }
                | ResourceCarrierProjectionPath::MapKey { index }
                | ResourceCarrierProjectionPath::MapValue { index } => match index {
                    ResourceCarrierIndex::Local(cursor) => iteration.cursor == Some(*cursor),
                    ResourceCarrierIndex::CurrentIteration { header } => {
                        iteration.cursor.is_none() && iteration.header == *header
                    }
                    ResourceCarrierIndex::Constant(_) => false,
                },
                _ => false,
            })
        });
        Ok(())
    }
    fn operation(&self, state: &mut State, operation: &ResourceOperation) -> Result<(), String> {
        if let ResourceOperationRole::Carrier { operation: id } = operation.role {
            return self.carrier(
                state,
                self.carriers
                    .operations
                    .get(id.index())
                    .ok_or("carrier operation reference missing")?,
            );
        }
        if !matches!(
            operation.role,
            ResourceOperationRole::CompleteReturnAfterCleanup { .. }
        ) {
            self.frame(state, operation.frame)?;
        }
        state.retired_projection = None;
        match &operation.role {
            ResourceOperationRole::CreateAbsentSum { destination } => {
                if !matches!(state.leaves[destination.index()], Leaf::Vacant | Leaf::Moved) { return Err("carrier absence sum installs into a live shell".into()); }
                state.leaves[destination.index()] = Leaf::Empty { origin: Origin::LeafAbsence(operation.id) };
            }
            ResourceOperationRole::Transfer { source, destination } => {
                if !matches!(state.leaves[source.index()], Leaf::Empty { .. }) || !matches!(state.leaves[destination.index()], Leaf::Vacant | Leaf::Moved) { return Err("carrier leaf transfer lacks exact empty source/vacant destination".into()); }
                if state.leaf_loans.iter().any(|loan| self.plan.loans[loan.index()].source == ResourceLoanSource::Owner(*source)) { return Err("carrier leaf transfer uses a source with an exact live sum lease".into()); }
                state.leaves[destination.index()] = std::mem::replace(&mut state.leaves[source.index()], Leaf::Moved);
            }
            ResourceOperationRole::BorrowSum { loan } => {
                match self.plan.loans[loan.index()].source {
                    ResourceLoanSource::Owner(slot) if matches!(state.leaves[slot.index()], Leaf::Empty { .. }) => {}
                    ResourceLoanSource::CarrierSumProjection { operation } => {
                        let ResourceCarrierOperationRole::AdaptSum { source, destination, loan: Some(expected), .. } = &self.carriers.operations[operation.index()].role else { return Err("carrier sum Borrow lost its exact adapter row".into()); };
                        if *expected != *loan || state.leaves[destination.index()] != (Leaf::Adapter { operation }) { return Err("carrier sum Borrow changes its exact adapted shell".into()); }
                        self.loan_live(state, *source)?;
                    }
                    _ => return Err("carrier sum Borrow has no exact zero-leaf source".into()),
                }
                if !state.leaf_loans.insert(*loan) { return Err("carrier sum Borrow repeats a live activation".into()); }
            }
            ResourceOperationRole::EndSumBorrow { loan } => {
                if !state.leaf_loans.remove(loan) { return Err("carrier sum EndBorrow has no exact activation".into()); }
                if let ResourceLoanSource::CarrierSumProjection { operation } = self.plan.loans[loan.index()].source {
                    let ResourceCarrierOperationRole::AdaptSum { destination, .. } = self.carriers.operations[operation.index()].role else { return Err("carrier sum retirement lost adapter destination".into()); };
                    state.leaves[destination.index()] = Leaf::Moved;
                }
            }
            ResourceOperationRole::InvokeSourceFunction { operands, result, .. } => {
                for operand in operands {
                    match operand {
                        ResourceCallOperand::CarrierOwned { slot, .. } => {
                            if self.carriers.slots[slot.index()].frame != operation.frame { return Err("carrier Source owned actual bypassed its exact Operation holder".into()); }
                            self.take_complete(state, *slot)?;
                        }
                        ResourceCallOperand::CarrierBorrowed { loan, .. } => self.view_complete(state, *loan)?,
                        ResourceCallOperand::Owned { slot, .. } => {
                            if !matches!(state.leaves[slot.index()], Leaf::Empty { .. }) { return Err("carrier Source leaf actual lacks an exact absent producer".into()); }
                            state.leaves[slot.index()] = Leaf::Moved;
                        }
                        ResourceCallOperand::Borrowed { loan, .. } if state.leaf_loans.contains(loan) => {}
                        ResourceCallOperand::Borrowed { .. } => return Err("carrier Source uses an ended sum lease".into()),
                        ResourceCallOperand::Ordinary { .. } => {}
                    }
                }
                match result {
                    ResourceCallResult::Carrier { slot } => self.install(state, *slot, Origin::Source(operation.id))?,
                    ResourceCallResult::Owned { .. } => return Err("pending Resource carrier: live/conditional leaf Source result needs the dedicated leaf flow".into()),
                    ResourceCallResult::Ordinary { .. } => {}
                }
            }
            ResourceOperationRole::Complete { outcome } => {
                let frame = operation.frame;
                if state.frames.last().copied() != Some(frame) { return Err("carrier completion changed its exact pending prefix".into()); }
                if state.loans.iter().any(|loan| self.carriers.loans[loan.index()].frame == frame && !matches!(self.carriers.loans[loan.index()].source, ResourceCarrierLoanSource::IncomingViewFormal { .. })) || state.leaf_loans.iter().any(|loan| self.plan.loans[loan.index()].frame == frame && !matches!(self.plan.loans[loan.index()].source, ResourceLoanSource::IncomingViewFormal { .. })) {
                    return Err("carrier completion has an unretired exact owned loan".into());
                }
                for slot in &self.carriers.slots {
                    if slot.frame == frame {
                        if matches!(state.slots[slot.id.index()], Slot::Building { .. }) && *outcome != ResourceCompletion::Abort { return Err("carrier normal completion has an incomplete acquired prefix".into()); }
                        state.slots[slot.id.index()] = if frame == ResourceFrameId(0) { Slot::Moved } else { Slot::Vacant };
                    }
                }
                for slot in &self.plan.slots { if slot.frame == frame { state.leaves[slot.id.index()] = if frame == ResourceFrameId(0) { Leaf::Moved } else { Leaf::Vacant }; } }
                state.frames.pop();
                if frame == ResourceFrameId(0) { state.returned = *outcome == ResourceCompletion::Return; }
            }
            ResourceOperationRole::CompleteReturnAfterCleanup { source } => {
                if !state.returned || !state.frames.is_empty() || self.plan.provisional_return().map(ResourceFrame::id) != Some(self.plan.slots[source.index()].frame) || !matches!(state.leaves[source.index()], Leaf::Empty { .. }) { return Err("carrier leaf Return publication lacks exact successful Scope cleanup and empty provisional holder".into()); }
                state.leaves[source.index()] = Leaf::Moved;
            }
            ResourceOperationRole::Drop { source, occupancy } => {
                if *occupancy != ResourceOccupancy::Empty || !matches!(state.leaves[source.index()], Leaf::Empty { .. }) || state.leaf_loans.iter().any(|loan| self.plan.loans[loan.index()].source == ResourceLoanSource::Owner(*source)) { return Err("carrier leaf discard lacks its initialized exact empty unleased endpoint".into()); }
                state.leaves[source.index()] = Leaf::Moved;
            }
            _ => return Err("pending Resource carrier: this leaf/control operation needs its dedicated independent flow proof".into()),
        }
        Ok(())
    }
    fn site(&self, state: &mut State, site: ResourceSite) -> Result<(), String> {
        for operation in &self.plan.operations {
            if operation.site == site {
                if let ResourceOperationRole::Carrier { operation: id } = operation.role {
                    if self.carriers.operations[id.index()].edge.is_some() {
                        continue;
                    }
                }
                self.operation(state, operation)?;
            }
        }
        state.retired_projection = None;
        Ok(())
    }
    fn edge(&self, state: &mut State, edge: ResourceCarrierEdge) -> Result<(), String> {
        for operation in self.carriers.operations_on_edge(edge) {
            self.carrier(state, operation)?;
        }
        Ok(())
    }
    fn branch(
        &self,
        state: &mut State,
        condition: &Expression,
        selected: bool,
        block: BlockId,
    ) -> Result<(), String> {
        if let E::Local(tag) = condition.kind
            && let Some(probe) = state.tags.get(&tag.index()).cloned()
        {
            if condition.ty != TypeInterner::BOOL
                || !matches!(
                    self.carriers
                        .shape(probe.shape)
                        .ok_or("carrier tag guard graph missing")?
                        .node,
                    ResourceCarrierNode::Optional { .. } | ResourceCarrierNode::Result { .. }
                )
            {
                return Err(
                    "carrier selecting Branch differs from its exact typed sum Tag destination"
                        .into(),
                );
            }
            self.select(state, &probe, Selector::Sum(selected));
        }
        if selected && let E::StateIs { .. } = condition.kind {
            let observations = self
                .carriers
                .operations_for_expression(condition)
                .filter_map(|operation| match operation.role {
                    ResourceCarrierOperationRole::Observe {
                        source,
                        observation: ResourceCarrierObservation::State { state },
                        ..
                    } if operation.site.block == block => Some((source, state)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let [(loan, state_id)] = observations.as_slice() else {
                return Err(
                    "carrier StateIs Branch has no unique exact current state observation".into(),
                );
            };
            let probe = self.probe(state, *loan)?;
            self.select(state, &probe, Selector::State(*state_id));
        }
        if selected
            && let Some(iteration) = self
                .carriers
                .iterations
                .iter()
                .find(|row| row.header == block && row.cursor.is_some())
        {
            let anchor = match self.live(state, iteration.source)? {
                Slot::Live { origin, .. } => Anchor {
                    root: Root::Slot(iteration.source),
                    origin: *origin,
                },
                _ => return Err("carrier iterator selecting edge lost its installed root".into()),
            };
            let probe = Probe {
                anchor,
                path: Vec::new(),
                shape: self.carriers.slots[iteration.source.index()].shape,
            };
            self.select(state, &probe, Selector::Iteration(block));
        }
        Ok(())
    }
    fn select_switch(
        &self,
        state: &mut State,
        source: BlockId,
        variant: VariantId,
    ) -> Result<(), String> {
        let probe =
            state.switches.get(&source.index()).cloned().ok_or(
                "carrier Switch selected an edge without its exact reached Tag observation",
            )?;
        let ResourceCarrierNode::Enum { variants, .. } = &self
            .carriers
            .shape(probe.shape)
            .ok_or("carrier Switch graph row missing")?
            .node
        else {
            return Err("carrier Switch observation is not an enum".into());
        };
        if variant.index() as usize >= variants.len() {
            return Err("carrier Switch edge selected a foreign graph variant".into());
        }
        self.select(state, &probe, Selector::Variant(variant.index() as usize));
        Ok(())
    }
    fn select_iteration(&self, state: &mut State, header: BlockId) -> Result<(), String> {
        let row = self
            .carriers
            .iterations
            .iter()
            .find(|row| row.header == header && row.cursor.is_none())
            .ok_or("carrier raw iterator edge has no exact Source/current association")?;
        let anchor = match self.live(state, row.source)? {
            Slot::Live { origin, .. } => Anchor {
                root: Root::Slot(row.source),
                origin: *origin,
            },
            _ => return Err("carrier raw iterator selecting edge lost its installed root".into()),
        };
        let probe = Probe {
            anchor,
            path: Vec::new(),
            shape: self.carriers.slots[row.source.index()].shape,
        };
        self.select(state, &probe, Selector::Iteration(header));
        Ok(())
    }
    fn normalized_entry(&self, state: &mut State, block: BlockId) -> Result<(), String> {
        for iteration in &self.carriers.iterations {
            if iteration.header != block {
                continue;
            }
            if iteration.cursor.is_none()
                && !matches!(state.slots[iteration.source.index()], Slot::Live { .. })
            {
                self.site(
                    state,
                    ResourceSite {
                        function: self.function.id,
                        block,
                        position: ResourcePosition::Terminator,
                    },
                )?;
            }
            self.reset_iteration(state, iteration.source, &iteration.binders)?;
        }
        Ok(())
    }
}

pub(super) fn validate(
    function: &Function,
    _types: &TypeInterner,
    plan: &ResourceFunctionPlan,
    carriers: &ResourceCarrierFunctionPlan,
) -> Result<(), String> {
    let analysis = Analysis {
        function,
        plan,
        carriers,
    };
    let mut initial = State {
        slots: vec![Slot::Vacant; carriers.slots.len()],
        leaves: vec![Leaf::Vacant; plan.slots.len()],
        loans: BTreeSet::new(),
        leaf_loans: BTreeSet::new(),
        frames: vec![ResourceFrameId(0)],
        returned: false,
        selections: Vec::new(),
        tags: BTreeMap::new(),
        switches: BTreeMap::new(),
        retired_projection: None,
    };
    for (parameter, param) in function.params.iter().enumerate() {
        for slot in &carriers.slots {
            if matches!(&slot.storage, ResourceSlotStorage::Local { header } if header.id == param.local)
            {
                analysis.install(&mut initial, slot.id, Origin::Formal(parameter))?;
            }
        }
    }
    for loan in &carriers.loans {
        if matches!(
            loan.source,
            ResourceCarrierLoanSource::IncomingViewFormal { .. }
        ) {
            initial.loans.insert(loan.id);
        }
    }
    for loan in &plan.loans {
        if matches!(loan.source, ResourceLoanSource::IncomingViewFormal { .. }) {
            initial.leaf_loans.insert(loan.id);
        }
    }
    analysis.normalized_entry(&mut initial, function.entry)?;
    let mut states = vec![None; function.blocks.len()];
    states[usize::try_from(function.entry.index())
        .map_err(|_| "carrier entry index is not representable")?] = Some(initial);
    let mut queue = VecDeque::from([function.entry]);
    let limit = function
        .blocks
        .len()
        .saturating_mul(
            carriers
                .slots
                .len()
                .saturating_add(carriers.loans.len())
                .saturating_add(1),
        )
        .saturating_mul(8)
        .saturating_add(1);
    let mut steps = 0usize;
    while let Some(block_id) = queue.pop_front() {
        steps = steps
            .checked_add(1)
            .ok_or("carrier fixed point step overflow")?;
        if steps > limit {
            return Err(
                "pending Resource carrier: exact custody fixed point exceeds its finite bound"
                    .into(),
            );
        }
        let index = usize::try_from(block_id.index())
            .map_err(|_| "carrier block index is not representable")?;
        let mut state = states[index]
            .as_ref()
            .ok_or("carrier reachable block has no exact incoming state")?
            .clone();
        let block = &function.blocks[index];
        for statement in 0..block.statements.len() {
            analysis.site(
                &mut state,
                ResourceSite {
                    function: function.id,
                    block: block_id,
                    position: ResourcePosition::Statement(statement),
                },
            )?;
        }
        let raw_iterator = carriers
            .iterations
            .iter()
            .any(|row| row.header == block_id && row.cursor.is_none());
        if !raw_iterator {
            analysis.site(
                &mut state,
                ResourceSite {
                    function: function.id,
                    block: block_id,
                    position: ResourcePosition::Terminator,
                },
            )?;
        }
        let mut outputs = Vec::new();
        match &block.terminator.kind {
            TerminatorKind::Goto(target) => outputs.push((*target, state)),
            TerminatorKind::Branch { condition, then_block, else_block } => {
                let mut yes = state.clone(); analysis.branch(&mut yes, condition, true, block_id)?;
                analysis.branch(&mut state, condition, false, block_id)?;
                outputs.push((*then_block, yes)); outputs.push((*else_block, state));
            }
            TerminatorKind::ForEach { body, exit, .. } => {
                let mut selected = state.clone();
                if carriers.iterations.iter().any(|row| row.header == block_id) {
                    analysis.select_iteration(&mut selected, block_id)?;
                    analysis.edge(&mut selected, ResourceCarrierEdge::IterationBody { source: block_id, target: *body })?;
                }
                outputs.push((*body, selected)); outputs.push((*exit, state));
            }
            TerminatorKind::Switch { variants, otherwise, .. } => {
                for (variant, target, _) in variants {
                    let mut selected = state.clone();
                    if carriers.operations_on_edge(ResourceCarrierEdge::SwitchVariant { source: block_id, variant: *variant, target: *target }).next().is_some() {
                        analysis.select_switch(&mut selected, block_id, *variant)?;
                        analysis.edge(&mut selected, ResourceCarrierEdge::SwitchVariant { source: block_id, variant: *variant, target: *target })?;
                    }
                    outputs.push((*target, selected));
                }
                if let Some(target) = otherwise { analysis.edge(&mut state, ResourceCarrierEdge::SwitchOtherwise { source: block_id, target: *target })?; outputs.push((*target, state)); }
            }
            TerminatorKind::Return(_) | TerminatorKind::Unreachable => {}
            _ => return Err("pending Resource carrier: this terminator requires its independent custody edge proof".into()),
        }
        for (target, mut output) in outputs {
            analysis.normalized_entry(&mut output, target)?;
            let index = usize::try_from(target.index())
                .map_err(|_| "carrier successor index is not representable")?;
            if let Some(current) = &mut states[index] {
                // Selecting facts can be forgotten at joins. Custody, generation,
                // arrived prefix and exact active lease sets must agree independently.
                let mut merged = current.clone();
                merged
                    .selections
                    .retain(|fact| output.selections.contains(fact));
                merged
                    .tags
                    .retain(|key, probe| output.tags.get(key) == Some(probe));
                merged
                    .switches
                    .retain(|key, probe| output.switches.get(key) == Some(probe));
                output.selections = merged.selections.clone();
                output.tags = merged.tags.clone();
                output.switches = merged.switches.clone();
                if merged != output {
                    return Err("pending Resource carrier: CFG join changed exact root generation, selected paths, operation prefix, or loans".into());
                }
                if *current != merged {
                    *current = merged;
                    queue.push_back(target);
                }
            } else {
                states[index] = Some(output);
                queue.push_back(target);
            }
        }
    }
    Ok(())
}
