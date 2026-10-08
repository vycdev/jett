//! Opaque v3 zero-leaf trees. Live leaf custody is deliberately not synthesized.
use super::*;
use crate::resource_custody::{
    NativeCarrierChildSource as ChildSource, NativeCarrierDestination as Destination,
    NativeCarrierLoanSource as LoanSource, NativeCarrierNode as Node,
    NativeCarrierObservation as Observation, NativeCarrierOperation as Op,
    NativeCarrierPath as Path, NativeCarrierSelector as Selector, NativeCarrierSource as Source,
};

pub(super) struct NativeCarrierTree {
    pub(super) node: u32,
    pub(super) selector: u32,
    pub(super) ordinary: Option<(u32, u64)>,
    pub(super) children: Vec<Option<NativeCarrierTree>>,
    pub(super) acquisition: Vec<usize>,
}
pub(super) struct NativeCarrierRoot {
    pub(super) frame: ResourceHandleId,
    pub(super) slot: u32,
    pub(super) generation: u64,
    pub(super) tree: NativeCarrierTree,
}
pub(super) struct NativeCarrierLoan {
    pub(super) frame: ResourceHandleId,
    pub(super) operation: u32,
    pub(super) root: ResourceHandleId,
    pub(super) generation: u64,
    pub(super) path: Vec<Path>,
    pub(super) parent: Option<ResourceHandleId>,
    pub(super) incoming: Option<(u32, u32, ResourceHandleId)>,
}
pub(super) struct NativeCarrierBuilder {
    pub(super) frame: ResourceHandleId,
    pub(super) operation: u32,
    pub(super) destination: u32,
    pub(super) selector: u32,
    pub(super) children: Vec<Option<NativeCarrierTree>>,
    pub(super) acquisition: Vec<usize>,
    pub(super) next: usize,
}
pub(super) struct NativeCarrierSumOrigin {
    pub(super) loan: ResourceHandleId,
    pub(super) operation: u32,
    pub(super) sum_loan: ResourceHandleId,
}
fn index(selector: Selector, dynamic: u64) -> ResourceResult<usize> {
    usize::try_from(match selector {
        Selector::Static(i) => u64::from(i),
        Selector::Dynamic => dynamic,
    })
    .map_err(|_| NativeResourceError::WrongOperation)
}
fn normalized(path: &[Path], dynamic: u64) -> ResourceResult<Vec<Path>> {
    let mut out = Vec::new();
    out.try_reserve_exact(path.len())
        .map_err(|_| NativeResourceError::Capacity)?;
    for p in path {
        out.push(match *p {
            Path::List(s) => Path::List(Selector::Static(
                u32::try_from(index(s, dynamic)?)
                    .map_err(|_| NativeResourceError::WrongOperation)?,
            )),
            Path::MapKey(s) => Path::MapKey(Selector::Static(
                u32::try_from(index(s, dynamic)?)
                    .map_err(|_| NativeResourceError::WrongOperation)?,
            )),
            Path::MapValue(s) => Path::MapValue(Selector::Static(
                u32::try_from(index(s, dynamic)?)
                    .map_err(|_| NativeResourceError::WrongOperation)?,
            )),
            p => p,
        });
    }
    Ok(out)
}
fn child_index(tree: &NativeCarrierTree, step: Path) -> ResourceResult<usize> {
    Ok(match step {
        Path::Field(i) => i as usize,
        Path::EnumPayload { variant, field } if variant == tree.selector => field as usize,
        Path::MachinePayload { state, field } if state == tree.selector => field as usize,
        Path::List(s) => index(s, 0)?,
        Path::MapKey(s) => index(s, 0)?
            .checked_mul(2)
            .ok_or(NativeResourceError::Capacity)?,
        Path::MapValue(s) => index(s, 0)?
            .checked_mul(2)
            .and_then(|i| i.checked_add(1))
            .ok_or(NativeResourceError::Capacity)?,
        Path::Some | Path::Ok if tree.selector == 1 => 0,
        Path::Fail if tree.selector == 0 => 0,
        _ => return Err(NativeResourceError::WrongOperation),
    })
}
pub(super) fn selected<'a>(
    mut tree: &'a NativeCarrierTree,
    path: &[Path],
) -> ResourceResult<&'a NativeCarrierTree> {
    for p in path {
        let i = child_index(tree, *p)?;
        tree = tree
            .children
            .get(i)
            .and_then(Option::as_ref)
            .ok_or(NativeResourceError::WrongOperation)?;
    }
    Ok(tree)
}
pub(super) fn take_selected(
    tree: &mut NativeCarrierTree,
    path: &[Path],
) -> ResourceResult<NativeCarrierTree> {
    let (last, prefix) = path
        .split_last()
        .ok_or(NativeResourceError::WrongOperation)?;
    let mut current = tree;
    for p in prefix {
        let i = child_index(current, *p)?;
        current = current
            .children
            .get_mut(i)
            .and_then(Option::as_mut)
            .ok_or(NativeResourceError::WrongOperation)?;
    }
    let i = child_index(current, *last)?;
    current
        .children
        .get_mut(i)
        .and_then(Option::take)
        .ok_or(NativeResourceError::WrongOperation)
}
impl NativeResourceState {
    pub(super) fn carrier_operation(
        &self,
        operation: u32,
        frame: ResourceHandleId,
    ) -> ResourceResult<Op> {
        if self.layout.wire_version() != 3 {
            return Err(NativeResourceError::UnsupportedSourceBoundary);
        }
        let NativeOperation::Carrier { record } = *self.operation(operation)? else {
            return Err(NativeResourceError::WrongOperation);
        };
        let row = self
            .layout
            .carriers()
            .operations
            .get(record as usize)
            .ok_or(NativeResourceError::WrongOperation)?;
        if row.site != self.layout.operations()[operation as usize].site() {
            return Err(NativeResourceError::WrongOperation);
        }
        self.operation_frame(operation, frame)?;
        if self.frame(frame)?.template != row.operation.frame() {
            return Err(NativeResourceError::WrongFrame);
        }
        Ok(row.operation.clone())
    }
    pub(super) fn carrier_slot_empty(
        &self,
        frame: ResourceHandleId,
        slot: u32,
    ) -> ResourceResult<()> {
        let s = self
            .layout
            .carriers()
            .slots
            .get(slot as usize)
            .ok_or(NativeResourceError::WrongOperation)?;
        if s.frame != self.frame(frame)?.template {
            return Err(NativeResourceError::WrongFrame);
        }
        if self.handles.values().any(|e|matches!(e,NativeResourceEntry::CarrierRoot(r) if r.frame==frame&&r.slot==slot)||matches!(e,NativeResourceEntry::CarrierBuilder(b) if b.frame==frame&&b.destination==slot)){return Err(NativeResourceError::WrongOperation);}
        Ok(())
    }
    fn validate_tree(
        &self,
        ordinary: &values::NativeValues,
        tree: &NativeCarrierTree,
    ) -> ResourceResult<()> {
        let mut pending = Vec::new();
        pending
            .try_reserve(1)
            .map_err(|_| NativeResourceError::Capacity)?;
        pending.push(tree);
        while let Some(t) = pending.pop() {
            let n = self.layout.carriers().node(t.node)?;
            match n {
                Node::Resource { .. } => {
                    return Err(NativeResourceError::UnsupportedCarrierCustody);
                }
                Node::Refinement { .. } => {
                    return Err(NativeResourceError::UnprovedCarrierRefinement);
                }
                Node::Ordinary { shape } => {
                    if !t.children.is_empty() || t.ordinary.map(|v| v.0) != Some(*shape) {
                        return Err(NativeResourceError::WrongFamily);
                    }
                    ordinary
                        .validate_resource_ordinary(
                            t.ordinary.ok_or(NativeResourceError::WrongFamily)?.1,
                            &self.layout,
                            *shape,
                        )
                        .map_err(ordinary_error)?;
                    continue;
                }
                Node::List { element } => {
                    if t.selector != 0
                        || t.ordinary.is_some()
                        || t.children.iter().flatten().any(|c| c.node != *element)
                    {
                        return Err(NativeResourceError::WrongFamily);
                    }
                }
                Node::Map { key, value } => {
                    if t.selector != 0
                        || t.ordinary.is_some()
                        || t.children.len() % 2 != 0
                        || t.children.iter().enumerate().any(|(i, c)| {
                            c.as_ref()
                                .is_some_and(|c| c.node != if i % 2 == 0 { *key } else { *value })
                        })
                    {
                        return Err(NativeResourceError::WrongFamily);
                    }
                }
                _ => {
                    let children = self
                        .layout
                        .carriers()
                        .selected_children(t.node, t.selector)?;
                    if t.ordinary.is_some()
                        || t.children.len() != children.len()
                        || t.children
                            .iter()
                            .zip(children)
                            .any(|(c, n)| c.as_ref().is_some_and(|c| c.node != n))
                    {
                        return Err(NativeResourceError::WrongFamily);
                    }
                }
            }
            pending
                .try_reserve(t.children.len())
                .map_err(|_| NativeResourceError::Capacity)?;
            pending.extend(t.children.iter().flatten());
        }
        Ok(())
    }
    pub(super) fn validate_complete_tree(
        &self,
        ordinary: &values::NativeValues,
        tree: &NativeCarrierTree,
    ) -> ResourceResult<()> {
        self.validate_tree(ordinary, tree)?;
        let mut pending = Vec::new();
        pending
            .try_reserve(1)
            .map_err(|_| NativeResourceError::Capacity)?;
        pending.push(tree);
        while let Some(t) = pending.pop() {
            if t.children.iter().any(Option::is_none) {
                return Err(NativeResourceError::WrongOperation);
            }
            pending
                .try_reserve(t.children.len())
                .map_err(|_| NativeResourceError::Capacity)?;
            pending.extend(t.children.iter().flatten());
        }
        Ok(())
    }
    pub(super) fn carrier_root(
        &self,
        ordinary: &values::NativeValues,
        frame: ResourceHandleId,
        handle: ResourceHandleId,
        slot: u32,
    ) -> ResourceResult<&NativeCarrierRoot> {
        let Some(NativeResourceEntry::CarrierRoot(root)) = self.handles.get(&handle) else {
            return Err(NativeResourceError::WrongFamily);
        };
        let row = self
            .layout
            .carriers()
            .slots
            .get(slot as usize)
            .ok_or(NativeResourceError::WrongOperation)?;
        if root.slot != slot
            || root.generation != handle.raw()
            || root.tree.node != row.node
            || root.frame != self.activation_frame(frame, row.frame)?
        {
            return Err(NativeResourceError::WrongFrame);
        }
        self.frame(root.frame)?;
        self.validate_tree(ordinary, &root.tree)?;
        Ok(root)
    }
    pub(super) fn carrier_loan(
        &self,
        ordinary: &values::NativeValues,
        frame: ResourceHandleId,
        handle: ResourceHandleId,
        source: LoanSource,
    ) -> ResourceResult<&NativeCarrierLoan> {
        let Some(NativeResourceEntry::CarrierLoan(loan)) = self.handles.get(&handle) else {
            return Err(NativeResourceError::WrongFamily);
        };
        self.frame(loan.frame)?;
        let Some(NativeResourceEntry::CarrierRoot(root)) = self.handles.get(&loan.root) else {
            return Err(NativeResourceError::InvalidHandle);
        };
        if root.generation != loan.generation {
            return Err(NativeResourceError::InvalidHandle);
        }
        self.frame(root.frame)?;
        self.validate_tree(ordinary, selected(&root.tree, &loan.path)?)?;
        if let Some(parent) = loan.parent {
            let Some(NativeResourceEntry::CarrierLoan(parent_loan)) = self.handles.get(&parent)
            else {
                return Err(NativeResourceError::InvalidHandle);
            };
            if parent_loan.root != loan.root
                || parent_loan.generation != loan.generation
                || !loan.path.starts_with(&parent_loan.path)
            {
                return Err(NativeResourceError::WrongOperation);
            }
        }
        match source {
            LoanSource::ExistingBorrow { operation } => {
                if loan.operation != operation
                    || loan.incoming.is_some()
                    || self.activation_frame(frame, self.frame(loan.frame)?.template)? != loan.frame
                {
                    return Err(NativeResourceError::WrongOperation);
                }
            }
            LoanSource::IncomingViewFormal { scope, parameter } => {
                let Some((expected, p, call)) = loan.incoming else {
                    return Err(NativeResourceError::WrongOperation);
                };
                let scope_frame = self.activation_frame(frame, scope)?;
                if expected != scope || p != parameter || loan.frame != scope_frame {
                    return Err(NativeResourceError::WrongFrame);
                }
                let active = self.source_call(call)?;
                if active.scope != Some(scope_frame) || active.phase != source::SourcePhase::Entered
                {
                    return Err(NativeResourceError::WrongFrame);
                }
            }
        }
        Ok(loan)
    }
    pub(super) fn carrier_location(
        &self,
        ordinary: &values::NativeValues,
        frame: ResourceHandleId,
        handle: ResourceHandleId,
        source: Source,
        path: &[Path],
        dynamic: u64,
    ) -> ResourceResult<(ResourceHandleId, u64, Vec<Path>, Option<ResourceHandleId>)> {
        let (root, generation, mut full, parent) = match source {
            Source::Slot(slot) => {
                let r = self.carrier_root(ordinary, frame, handle, slot)?;
                (handle, r.generation, Vec::new(), None)
            }
            Source::Loan(source) => {
                let l = self.carrier_loan(ordinary, frame, handle, source)?;
                (l.root, l.generation, l.path.clone(), Some(handle))
            }
        };
        let tail = normalized(path, dynamic)?;
        full.try_reserve(tail.len())
            .map_err(|_| NativeResourceError::Capacity)?;
        full.extend(tail);
        let Some(NativeResourceEntry::CarrierRoot(r)) = self.handles.get(&root) else {
            return Err(NativeResourceError::WrongFamily);
        };
        self.layout.carriers().path_node(r.tree.node, &full)?;
        selected(&r.tree, &full)?;
        Ok((root, generation, full, parent))
    }
    pub(super) fn carrier_unborrowed(&self, root: ResourceHandleId) -> ResourceResult<()> {
        if self
            .handles
            .values()
            .any(|e| matches!(e,NativeResourceEntry::CarrierLoan(l) if l.root==root))
        {
            return Err(CustodyError::ActiveBorrow.into());
        }
        Ok(())
    }
    pub(super) fn carrier_construct_begin(
        &mut self,
        ordinary: &values::NativeValues,
        frame: ResourceHandleId,
        operation: u32,
    ) -> ResourceResult<ResourceHandleId> {
        self.running(ordinary)?;
        let Op::Construct {
            destination,
            selector,
            children,
            ..
        } = self.carrier_operation(operation, frame)?
        else {
            return Err(NativeResourceError::WrongOperation);
        };
        self.carrier_slot_empty(frame, destination)?;
        let mut values = Vec::new();
        let mut acquisition = Vec::new();
        values
            .try_reserve_exact(children.len())
            .map_err(|_| NativeResourceError::Capacity)?;
        acquisition
            .try_reserve_exact(children.len())
            .map_err(|_| NativeResourceError::Capacity)?;
        values.resize_with(children.len(), || None);
        let mut ids = self.reserve(1)?;
        let handle = next_handle(&mut ids)?;
        self.handles.insert(
            handle,
            NativeResourceEntry::CarrierBuilder(NativeCarrierBuilder {
                frame,
                operation,
                destination,
                selector,
                children: values,
                acquisition,
                next: 0,
            }),
        );
        Ok(handle)
    }
    pub(super) fn carrier_child(
        &mut self,
        ordinary: &values::NativeValues,
        builder: ResourceHandleId,
        ordinal: u32,
        bits: u64,
    ) -> ResourceResult<()> {
        self.running(ordinary)?;
        let Some(NativeResourceEntry::CarrierBuilder(b)) = self.handles.get(&builder) else {
            return Err(NativeResourceError::WrongFamily);
        };
        let (frame, operation, next) = (b.frame, b.operation, b.next);
        let Op::Construct { children, .. } = self.carrier_operation(operation, frame)? else {
            return Err(NativeResourceError::WrongOperation);
        };
        if ordinal as usize != next {
            return Err(NativeResourceError::WrongOperation);
        }
        let child = children
            .get(next)
            .ok_or(NativeResourceError::WrongOperation)?;
        if b.children
            .get(child.index as usize)
            .ok_or(NativeResourceError::WrongOperation)?
            .is_some()
        {
            return Err(NativeResourceError::WrongOperation);
        }
        let tree = match child.source {
            ChildSource::Ordinary { shape } => {
                ordinary
                    .validate_resource_ordinary(bits, &self.layout, shape)
                    .map_err(ordinary_error)?;
                NativeCarrierTree {
                    node: child.node,
                    selector: 0,
                    ordinary: Some((shape, bits)),
                    children: Vec::new(),
                    acquisition: Vec::new(),
                }
            }
            ChildSource::Move { slot } => {
                let handle = ResourceHandleId::new(bits)?;
                let root = self.carrier_root(ordinary, frame, handle, slot)?;
                self.validate_complete_tree(ordinary, &root.tree)?;
                self.carrier_unborrowed(handle)?;
                let Some(NativeResourceEntry::CarrierRoot(r)) = self.handles.remove(&handle) else {
                    return Err(NativeResourceError::WrongFamily);
                };
                r.tree
            }
            ChildSource::LeafSum { slot } => self.carrier_take_leaf_sum(
                ordinary,
                frame,
                ResourceHandleId::new(bits)?,
                slot,
                child.node,
            )?,
        };
        let Some(NativeResourceEntry::CarrierBuilder(b)) = self.handles.get_mut(&builder) else {
            return Err(NativeResourceError::WrongFamily);
        };
        let target = b
            .children
            .get_mut(child.index as usize)
            .ok_or(NativeResourceError::WrongOperation)?;
        *target = Some(tree);
        b.acquisition.push(child.index as usize);
        b.next += 1;
        Ok(())
    }
    pub(super) fn carrier_commit(
        &mut self,
        ordinary: &values::NativeValues,
        builder: ResourceHandleId,
    ) -> ResourceResult<ResourceHandleId> {
        self.running(ordinary)?;
        let Some(NativeResourceEntry::CarrierBuilder(b)) = self.handles.get(&builder) else {
            return Err(NativeResourceError::WrongFamily);
        };
        let Op::Construct { children, .. } = self.carrier_operation(b.operation, b.frame)? else {
            return Err(NativeResourceError::WrongOperation);
        };
        if b.next != children.len() || b.children.iter().any(Option::is_none) {
            return Err(NativeResourceError::WrongOperation);
        }
        let node = self.layout.carriers().slots[b.destination as usize].node;
        for child in b.children.iter().flatten() {
            self.validate_complete_tree(ordinary, child)?;
        }
        let is_ordinary = matches!(self.layout.carriers().node(node)?, Node::Ordinary { .. });
        let mut ids = self.reserve(1)?;
        let output = next_handle(&mut ids)?;
        let Some(NativeResourceEntry::CarrierBuilder(mut b)) = self.handles.remove(&builder) else {
            return Err(NativeResourceError::WrongFamily);
        };
        let tree = if is_ordinary {
            b.children
                .get_mut(0)
                .and_then(Option::take)
                .ok_or(NativeResourceError::WrongFamily)?
        } else {
            NativeCarrierTree {
                node,
                selector: b.selector,
                ordinary: None,
                children: b.children,
                acquisition: b.acquisition,
            }
        };
        self.handles.insert(
            output,
            NativeResourceEntry::CarrierRoot(NativeCarrierRoot {
                frame: b.frame,
                slot: b.destination,
                generation: output.raw(),
                tree,
            }),
        );
        Ok(output)
    }
    pub(super) fn carrier_move_reserved(
        &mut self,
        ordinary: &values::NativeValues,
        handle: ResourceHandleId,
        source: u32,
        destination: u32,
        source_frame: ResourceHandleId,
        frame: ResourceHandleId,
        output: ResourceHandleId,
    ) -> ResourceResult<ResourceHandleId> {
        let root = self.carrier_root(ordinary, source_frame, handle, source)?;
        self.validate_complete_tree(ordinary, &root.tree)?;
        self.carrier_unborrowed(handle)?;
        self.carrier_slot_empty(frame, destination)?;
        let expected = self.layout.carriers().slots[destination as usize].node;
        let Some(NativeResourceEntry::CarrierRoot(root)) = self.handles.get(&handle) else {
            return Err(NativeResourceError::WrongFamily);
        };
        if root.tree.node != expected {
            return Err(NativeResourceError::WrongFamily);
        }
        let Some(NativeResourceEntry::CarrierRoot(mut root)) = self.handles.remove(&handle) else {
            return Err(NativeResourceError::WrongFamily);
        };
        root.frame = frame;
        root.slot = destination;
        root.generation = output.raw();
        self.handles
            .insert(output, NativeResourceEntry::CarrierRoot(root));
        Ok(output)
    }
    pub(super) fn carrier_transfer(
        &mut self,
        ordinary: &values::NativeValues,
        frame: ResourceHandleId,
        operation: u32,
        handle: ResourceHandleId,
    ) -> ResourceResult<ResourceHandleId> {
        self.running(ordinary)?;
        let Op::Transfer {
            source,
            destination,
            ..
        } = self.carrier_operation(operation, frame)?
        else {
            return Err(NativeResourceError::WrongOperation);
        };
        let target = self.activation_frame(
            frame,
            self.layout.carriers().slots[destination as usize].frame,
        )?;
        let mut ids = self.reserve(1)?;
        let output = next_handle(&mut ids)?;
        self.carrier_move_reserved(ordinary, handle, source, destination, frame, target, output)
    }
    pub(super) fn carrier_qualify(
        &mut self,
        ordinary: &values::NativeValues,
        frame: ResourceHandleId,
        operation: u32,
        handle: ResourceHandleId,
    ) -> ResourceResult<ResourceHandleId> {
        self.running(ordinary)?;
        let Op::QualifyMachine {
            source,
            destination,
            machine,
            state,
            ..
        } = self.carrier_operation(operation, frame)?
        else {
            return Err(NativeResourceError::WrongOperation);
        };
        let root = self.carrier_root(ordinary, frame, handle, source)?;
        self.validate_complete_tree(ordinary, &root.tree)?;
        self.carrier_unborrowed(handle)?;
        if root.tree.selector != state
            || self.layout.carriers().node(root.tree.node)?
                != &(Node::MachineState { machine, state })
            || self.layout.carriers().slots[destination as usize].node != machine
        {
            return Err(NativeResourceError::WrongFamily);
        }
        let target = self.activation_frame(
            frame,
            self.layout.carriers().slots[destination as usize].frame,
        )?;
        self.carrier_slot_empty(target, destination)?;
        let mut ids = self.reserve(1)?;
        let output = next_handle(&mut ids)?;
        let Some(NativeResourceEntry::CarrierRoot(mut root)) = self.handles.remove(&handle) else {
            return Err(NativeResourceError::WrongFamily);
        };
        root.tree.node = machine;
        root.slot = destination;
        root.frame = target;
        root.generation = output.raw();
        self.handles
            .insert(output, NativeResourceEntry::CarrierRoot(root));
        Ok(output)
    }
    pub(super) fn carrier_borrow(
        &mut self,
        ordinary: &values::NativeValues,
        frame: ResourceHandleId,
        operation: u32,
        handle: ResourceHandleId,
        dynamic: u64,
    ) -> ResourceResult<ResourceHandleId> {
        self.running(ordinary)?;
        let Op::Borrow {
            source,
            path,
            lease_frame,
            ..
        } = self.carrier_operation(operation, frame)?
        else {
            return Err(NativeResourceError::WrongOperation);
        };
        if lease_frame != self.frame(frame)?.template {
            return Err(NativeResourceError::WrongFrame);
        }
        let (root, generation, path, parent) =
            self.carrier_location(ordinary, frame, handle, source, &path, dynamic)?;
        let mut ids = self.reserve(1)?;
        let output = next_handle(&mut ids)?;
        self.handles.insert(
            output,
            NativeResourceEntry::CarrierLoan(NativeCarrierLoan {
                frame,
                operation,
                root,
                generation,
                path,
                parent,
                incoming: None,
            }),
        );
        Ok(output)
    }
    pub(super) fn carrier_end_loan(
        &mut self,
        frame: ResourceHandleId,
        handle: ResourceHandleId,
    ) -> ResourceResult<()> {
        let Some(NativeResourceEntry::CarrierLoan(loan)) = self.handles.get(&handle) else {
            return Err(NativeResourceError::WrongFamily);
        };
        if loan.frame != frame
            || self
                .handles
                .values()
                .any(|e| matches!(e,NativeResourceEntry::CarrierLoan(l) if l.parent==Some(handle)))
            || self
                .carrier_adapters
                .values()
                .any(|a| a.loan == handle && self.handles.contains_key(&a.sum_loan))
        {
            return Err(CustodyError::ActiveBorrow.into());
        }
        self.handles.remove(&handle);
        Ok(())
    }
    pub(super) fn carrier_end(
        &mut self,
        frame: ResourceHandleId,
        operation: u32,
        handle: ResourceHandleId,
    ) -> ResourceResult<()> {
        let Op::EndBorrow { borrow, .. } = self.carrier_operation(operation, frame)? else {
            return Err(NativeResourceError::WrongOperation);
        };
        let Some(NativeResourceEntry::CarrierLoan(l)) = self.handles.get(&handle) else {
            return Err(NativeResourceError::WrongFamily);
        };
        if l.operation != borrow || l.incoming.is_some() {
            return Err(NativeResourceError::WrongOperation);
        }
        self.carrier_end_loan(frame, handle)
    }
    pub(super) fn carrier_observe(
        &self,
        ordinary: &mut values::NativeValues,
        frame: ResourceHandleId,
        operation: u32,
        handle: ResourceHandleId,
        dynamic: u64,
    ) -> ResourceResult<u64> {
        self.running(ordinary)?;
        let Op::Observe {
            source,
            path,
            observation,
            ..
        } = self.carrier_operation(operation, frame)?
        else {
            return Err(NativeResourceError::WrongOperation);
        };
        let (root, _, path, _) =
            self.carrier_location(ordinary, frame, handle, source, &path, dynamic)?;
        let Some(NativeResourceEntry::CarrierRoot(r)) = self.handles.get(&root) else {
            return Err(NativeResourceError::WrongFamily);
        };
        let tree = selected(&r.tree, &path)?;
        match observation {
            Observation::Length => Ok(u64::try_from(
                match self.layout.carriers().node(tree.node)? {
                    Node::List { .. } => tree.children.len(),
                    Node::Map { .. } => tree.children.len() / 2,
                    _ => return Err(NativeResourceError::WrongFamily),
                },
            )
            .map_err(|_| NativeResourceError::Capacity)?),
            Observation::Tag => match self.layout.carriers().node(tree.node)? {
                Node::Enum { variants, .. } => Ok(variants
                    .get(tree.selector as usize)
                    .ok_or(NativeResourceError::WrongFamily)?
                    .discriminant as u64),
                Node::Optional { .. } | Node::Result { .. } => Ok(u64::from(tree.selector)),
                _ => Err(NativeResourceError::WrongFamily),
            },
            Observation::State => Ok(u64::from(tree.selector)),
            Observation::Ordinary { shape, clone } => {
                let (actual, bits) = tree.ordinary.ok_or(NativeResourceError::WrongFamily)?;
                if actual != shape {
                    return Err(NativeResourceError::WrongFamily);
                }
                self.carrier_observe_ordinary(ordinary, shape, clone, bits)
            }
        }
    }
    fn carrier_observe_ordinary(
        &self,
        ordinary: &mut values::NativeValues,
        shape: u32,
        clone: bool,
        bits: u64,
    ) -> ResourceResult<u64> {
        let required = Observation::ordinary_clone_policy(self.layout.shapes(), shape)
            .ok_or(NativeResourceError::UnsupportedCarrierObservation)?;
        if clone != required {
            return Err(NativeResourceError::WrongOperation);
        }
        ordinary
            .validate_resource_ordinary(bits, &self.layout, shape)
            .map_err(ordinary_error)?;
        if required {
            ordinary
                .clone_resource_string_companion(&self.layout, shape, bits)
                .map_err(ordinary_error)
        } else {
            Ok(bits)
        }
    }
    pub(super) fn carrier_extract(
        &mut self,
        ordinary: &values::NativeValues,
        frame: ResourceHandleId,
        operation: u32,
        handle: ResourceHandleId,
        dynamic: u64,
    ) -> ResourceResult<u64> {
        self.running(ordinary)?;
        let Op::Extract {
            source,
            path,
            destination,
            ..
        } = self.carrier_operation(operation, frame)?
        else {
            return Err(NativeResourceError::WrongOperation);
        };
        let (_, _, path, _) = self.carrier_location(
            ordinary,
            frame,
            handle,
            Source::Slot(source),
            &path,
            dynamic,
        )?;
        self.carrier_unborrowed(handle)?;
        let Some(NativeResourceEntry::CarrierRoot(root)) = self.handles.get(&handle) else {
            return Err(NativeResourceError::WrongFamily);
        };
        let projected = selected(&root.tree, &path)?;
        self.validate_complete_tree(ordinary, projected)?;
        match destination {
            Destination::Slot(slot) => {
                if projected.node != self.layout.carriers().slots[slot as usize].node {
                    return Err(NativeResourceError::WrongFamily);
                }
            }
            Destination::Ordinary { shape } => {
                if projected.ordinary.map(|v| v.0) != Some(shape) {
                    return Err(NativeResourceError::WrongFamily);
                }
            }
        }
        if path.is_empty() {
            return Err(NativeResourceError::WrongOperation);
        }
        let output = match destination {
            Destination::Slot(slot) => {
                let target = self
                    .activation_frame(frame, self.layout.carriers().slots[slot as usize].frame)?;
                self.carrier_slot_empty(target, slot)?;
                let mut ids = self.reserve(1)?;
                Some((next_handle(&mut ids)?, target, slot))
            }
            Destination::Ordinary { .. } => None,
        };
        let Some(NativeResourceEntry::CarrierRoot(root)) = self.handles.get_mut(&handle) else {
            return Err(NativeResourceError::WrongFamily);
        };
        let tree = take_selected(&mut root.tree, &path)?;
        if let Some((out, target, slot)) = output {
            self.handles.insert(
                out,
                NativeResourceEntry::CarrierRoot(NativeCarrierRoot {
                    frame: target,
                    slot,
                    generation: out.raw(),
                    tree,
                }),
            );
            Ok(out.raw())
        } else {
            let Destination::Ordinary { shape } = destination else {
                return Err(NativeResourceError::WrongFamily);
            };
            let (actual, bits) = tree.ordinary.ok_or(NativeResourceError::WrongFamily)?;
            if actual != shape {
                return Err(NativeResourceError::WrongFamily);
            }
            Ok(bits)
        }
    }
    fn carrier_take_leaf_sum(
        &mut self,
        ordinary: &values::NativeValues,
        frame: ResourceHandleId,
        handle: ResourceHandleId,
        slot: u32,
        node: u32,
    ) -> ResourceResult<NativeCarrierTree> {
        self.carrier_frame_at(frame, handle, slot)?;
        self.sum_unborrowed(handle)?;
        let Some(NativeResourceEntry::Sum(sum)) = self.handles.get(&handle) else {
            return Err(NativeResourceError::WrongFamily);
        };
        if sum.slot != slot
            || self.layout.slots().get(slot as usize).map(|s| s.shape()) != Some(sum.shape)
            || self.carrier_adapters.contains_key(&handle)
            || !self.layout.carrier_equivalent_sum(node, sum.shape)
        {
            return Err(NativeResourceError::WrongFamily);
        }
        let (selector, child) = match sum.payload {
            NativeSumPayload::None => (0, None),
            NativeSumPayload::Fail { shape, string } => {
                ordinary
                    .validate_resource_ordinary(string, &self.layout, shape)
                    .map_err(ordinary_error)?;
                let Node::Result { fail, .. } = self.layout.carriers().node(node)? else {
                    return Err(NativeResourceError::WrongFamily);
                };
                (
                    0,
                    Some(NativeCarrierTree {
                        node: *fail,
                        selector: 0,
                        ordinary: Some((shape, string)),
                        children: Vec::new(),
                        acquisition: Vec::new(),
                    }),
                )
            }
            _ => return Err(NativeResourceError::UnsupportedCarrierCustody),
        };
        let mut children = Vec::new();
        let mut acquisition = Vec::new();
        children
            .try_reserve_exact(usize::from(child.is_some()))
            .map_err(|_| NativeResourceError::Capacity)?;
        acquisition
            .try_reserve_exact(usize::from(child.is_some()))
            .map_err(|_| NativeResourceError::Capacity)?;
        if let Some(child) = child {
            children.push(Some(child));
            acquisition.push(0);
        }
        self.handles.remove(&handle);
        Ok(NativeCarrierTree {
            node,
            selector,
            ordinary: None,
            children,
            acquisition,
        })
    }
    fn prepare_carrier_cleanup<'a>(
        &self,
        trees: impl Iterator<Item = &'a NativeCarrierTree>,
    ) -> ResourceResult<Vec<NativeCarrierTree>> {
        let mut scan = Vec::new();
        for tree in trees {
            scan.try_reserve(1)
                .map_err(|_| NativeResourceError::Capacity)?;
            scan.push(tree);
        }
        let mut count = 0usize;
        while let Some(tree) = scan.pop() {
            count = count.checked_add(1).ok_or(NativeResourceError::Capacity)?;
            scan.try_reserve(tree.children.len())
                .map_err(|_| NativeResourceError::Capacity)?;
            scan.extend(tree.children.iter().flatten());
        }
        let mut pending = Vec::new();
        pending
            .try_reserve_exact(count)
            .map_err(|_| NativeResourceError::Capacity)?;
        Ok(pending)
    }
    fn drop_carrier_tree_prepared(
        &self,
        ordinary: &mut values::NativeValues,
        tree: NativeCarrierTree,
        mut pending: Vec<NativeCarrierTree>,
    ) -> ResourceResult<()> {
        pending.push(tree);
        let mut first = None;
        while let Some(mut tree) = pending.pop() {
            if let Some((shape, bits)) = tree.ordinary {
                if let Err(e) = ordinary
                    .drop_resource_typed_companion(&self.layout, shape, bits)
                    .map_err(ordinary_error)
                {
                    first.get_or_insert(e);
                }
            }
            // LIFO visiting makes the latest lexical acquisition retire first.
            for i in tree.acquisition {
                if let Some(child) = tree.children.get_mut(i).and_then(Option::take) {
                    pending.push(child);
                }
            }
        }
        first.map_or(Ok(()), Err)
    }
    pub(super) fn carrier_retire(
        &mut self,
        ordinary: &mut values::NativeValues,
        frame: ResourceHandleId,
        operation: u32,
        handle: ResourceHandleId,
    ) -> ResourceResult<()> {
        let Op::Retire { source, .. } = self.carrier_operation(operation, frame)? else {
            return Err(NativeResourceError::WrongOperation);
        };
        let root = self.carrier_root(ordinary, frame, handle, source)?;
        let pending = self.prepare_carrier_cleanup(std::iter::once(&root.tree))?;
        self.carrier_unborrowed(handle)?;
        let Some(NativeResourceEntry::CarrierRoot(root)) = self.handles.remove(&handle) else {
            return Err(NativeResourceError::WrongFamily);
        };
        self.drop_carrier_tree_prepared(ordinary, root.tree, pending)
    }
    pub(super) fn carrier_reset_iteration(
        &mut self,
        ordinary: &mut values::NativeValues,
        frame: ResourceHandleId,
        operation: u32,
        handle: ResourceHandleId,
    ) -> ResourceResult<()> {
        self.running(ordinary)?;
        let Op::ResetIteration {
            source,
            carrier_binders,
            leaf_sum_binders,
            ..
        } = self.carrier_operation(operation, frame)?
        else {
            return Err(NativeResourceError::WrongOperation);
        };
        self.carrier_root(ordinary, frame, handle, source)?;
        self.carrier_unborrowed(handle)?;
        let mut holders = Vec::new();
        holders
            .try_reserve_exact(carrier_binders.len() + leaf_sum_binders.len())
            .map_err(|_| NativeResourceError::Capacity)?;
        for slot in carrier_binders {
            let target =
                self.activation_frame(frame, self.layout.carriers().slots[slot as usize].frame)?;
            if self.handles.values().any(|e|matches!(e,NativeResourceEntry::CarrierBuilder(b) if b.frame==target&&b.destination==slot)){return Err(NativeResourceError::WrongOperation);}
            if let Some(id) = self.handles.iter().find_map(|(id, e)| {
                matches!(e,NativeResourceEntry::CarrierRoot(r) if r.frame==target&&r.slot==slot)
                    .then_some(*id)
            }) {
                let root = self.carrier_root(ordinary, frame, id, slot)?;
                self.carrier_unborrowed(id)?;
                holders.push((
                    id,
                    Some(self.prepare_carrier_cleanup(std::iter::once(&root.tree))?),
                ));
            }
        }
        for slot in leaf_sum_binders {
            let target =
                self.activation_frame(frame, self.layout.slots()[slot as usize].frame())?;
            let id = self.handles.iter().find_map(|(id, e)| {
                matches!(e,NativeResourceEntry::Sum(s) if s.frame==target&&s.slot==slot)
                    .then_some(*id)
            });
            if let Some(id)=id{
                self.carrier_frame_at(frame,id,slot)?;self.sum_unborrowed(id)?;
                let Some(NativeResourceEntry::Sum(sum))=self.handles.get(&id) else{return Err(NativeResourceError::WrongFamily);};
                if self.carrier_adapters.contains_key(&id)||sum.shape!=self.layout.slots()[slot as usize].shape(){return Err(NativeResourceError::WrongFamily);}
                match sum.payload{NativeSumPayload::None=>{},NativeSumPayload::Fail{shape,string}=>ordinary.validate_resource_ordinary(string,&self.layout,shape).map_err(ordinary_error)?,_=>return Err(NativeResourceError::UnsupportedCarrierCustody)}
                holders.push((id,None));
            }else if self.handles.values().any(|e|matches!(e,NativeResourceEntry::Owner(o) if o.frame==target&&o.slot==slot)||matches!(e,NativeResourceEntry::Prepared(p) if p.destination_frame==target&&p.destination==slot)){return Err(NativeResourceError::UnsupportedCarrierCustody);}
        }
        // Issued identities reflect the lexical acquisition order across both slot families.
        holders.sort_by_key(|(id, _)| std::cmp::Reverse(id.raw()));
        let mut first = None;
        for (id, pending) in holders {
            let result = match self.handles.remove(&id) {
                Some(NativeResourceEntry::CarrierRoot(root)) => self.drop_carrier_tree_prepared(
                    ordinary,
                    root.tree,
                    pending.ok_or(NativeResourceError::WrongFamily)?,
                ),
                Some(NativeResourceEntry::Sum(NativeResourceSum {
                    payload: NativeSumPayload::Fail { shape, string },
                    ..
                })) => ordinary
                    .drop_resource_typed_companion(&self.layout, shape, string)
                    .map_err(ordinary_error),
                Some(NativeResourceEntry::Sum(NativeResourceSum {
                    payload: NativeSumPayload::None,
                    ..
                })) => Ok(()),
                _ => Err(NativeResourceError::WrongFamily),
            };
            if let Err(error) = result {
                first.get_or_insert(error);
            }
        }
        first.map_or(Ok(()), Err)
    }
    pub(super) fn retire_carrier_frame(
        &mut self,
        ordinary: &mut values::NativeValues,
        frame: ResourceHandleId,
    ) -> ResourceResult<()> {
        if self.handles.values().any(|e|matches!(e,NativeResourceEntry::CarrierLoan(l) if l.frame!=frame && matches!(self.handles.get(&l.root),Some(NativeResourceEntry::CarrierRoot(r)) if r.frame==frame))){return Err(CustodyError::ActiveBorrow.into());}
        loop {
            let next = self.handles.iter().find_map(|(id, e)| {
                if matches!(e,NativeResourceEntry::CarrierLoan(l) if l.frame==frame)
                    && !self.handles.values().any(
                        |e| matches!(e,NativeResourceEntry::CarrierLoan(l) if l.parent==Some(*id)),
                    )
                {
                    Some(*id)
                } else {
                    None
                }
            });
            let Some(id) = next else {
                break;
            };
            self.carrier_end_loan(frame, id)?;
        }
        if self
            .handles
            .values()
            .any(|e| matches!(e,NativeResourceEntry::CarrierLoan(l) if l.frame==frame))
        {
            return Err(CustodyError::ActiveBorrow.into());
        }
        let mut holders = Vec::new();
        holders
            .try_reserve(self.handles.len())
            .map_err(|_| NativeResourceError::Capacity)?;
        holders.extend(self.handles.iter().filter_map(|(id, e)| {
            if matches!(e,NativeResourceEntry::CarrierRoot(r) if r.frame==frame)
                || matches!(e,NativeResourceEntry::CarrierBuilder(b) if b.frame==frame)
            {
                Some(*id)
            } else {
                None
            }
        }));
        holders.sort_by_key(|id| std::cmp::Reverse(id.raw()));
        let mut first = None;
        for id in holders {
            let pending = match self.handles.get(&id) {
                Some(NativeResourceEntry::CarrierRoot(r)) => {
                    self.prepare_carrier_cleanup(std::iter::once(&r.tree))?
                }
                Some(NativeResourceEntry::CarrierBuilder(b)) => {
                    let mut pending = self.prepare_carrier_cleanup(b.children.iter().flatten())?;
                    pending
                        .try_reserve(1)
                        .map_err(|_| NativeResourceError::Capacity)?;
                    pending
                }
                _ => continue,
            };
            let tree = match self.handles.remove(&id) {
                Some(NativeResourceEntry::CarrierRoot(r)) => r.tree,
                Some(NativeResourceEntry::CarrierBuilder(b)) => NativeCarrierTree {
                    node: 0,
                    selector: b.selector,
                    ordinary: None,
                    children: b.children,
                    acquisition: b.acquisition,
                },
                _ => continue,
            };
            if let Err(e) = self.drop_carrier_tree_prepared(ordinary, tree, pending) {
                first.get_or_insert(e);
            }
        }
        first.map_or(Ok(()), Err)
    }
    pub(super) fn safety_retire_carriers(
        &mut self,
        ordinary: &mut values::NativeValues,
    ) -> ResourceResult<()> {
        // The context has already been retired and waited for its leases. This
        // backstop never publishes data or grants a reusable Source loan.
        self.handles
            .retain(|_, entry| !matches!(entry, NativeResourceEntry::CarrierLoan(_)));
        let mut holders = Vec::new();
        holders
            .try_reserve(self.handles.len())
            .map_err(|_| NativeResourceError::Capacity)?;
        holders.extend(self.handles.iter().filter_map(|(id, entry)| {
            matches!(
                entry,
                NativeResourceEntry::CarrierRoot(_) | NativeResourceEntry::CarrierBuilder(_)
            )
            .then_some(*id)
        }));
        holders.sort_by_key(|id| std::cmp::Reverse(id.raw()));
        let mut first = None;
        for id in holders {
            let pending = match self.handles.get(&id) {
                Some(NativeResourceEntry::CarrierRoot(root)) => {
                    self.prepare_carrier_cleanup(std::iter::once(&root.tree))?
                }
                Some(NativeResourceEntry::CarrierBuilder(builder)) => {
                    let mut pending =
                        self.prepare_carrier_cleanup(builder.children.iter().flatten())?;
                    pending
                        .try_reserve(1)
                        .map_err(|_| NativeResourceError::Capacity)?;
                    pending
                }
                _ => continue,
            };
            let tree = match self.handles.remove(&id) {
                Some(NativeResourceEntry::CarrierRoot(root)) => root.tree,
                Some(NativeResourceEntry::CarrierBuilder(builder)) => NativeCarrierTree {
                    node: 0,
                    selector: builder.selector,
                    ordinary: None,
                    children: builder.children,
                    acquisition: builder.acquisition,
                },
                _ => continue,
            };
            if let Err(error) = self.drop_carrier_tree_prepared(ordinary, tree, pending) {
                first.get_or_insert(error);
            }
        }
        first.map_or(Ok(()), Err)
    }
}

#[cfg(test)]
#[path = "carriers_tests.rs"]
mod tests;
