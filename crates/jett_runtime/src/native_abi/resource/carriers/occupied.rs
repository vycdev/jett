//! Occupied carrier transport joins typed paths to the Core ownership ledger.
use super::*;

fn child_path(
    graph: &crate::resource_custody::NativeCarrierLayout,
    node: u32,
    selector: u32,
    index: usize,
) -> ResourceResult<Path> {
    let i = u32::try_from(index).map_err(|_| NativeResourceError::Capacity)?;
    Ok(match graph.node(node)? {
        Node::Struct { .. } => Path::Field(i),
        Node::Optional { .. } if selector == 1 && index == 0 => Path::Some,
        Node::Result { .. } if index == 0 => {
            if selector == 1 {
                Path::Ok
            } else {
                Path::Fail
            }
        }
        Node::Enum { .. } => Path::EnumPayload {
            variant: selector,
            field: i,
        },
        Node::Machine { .. } => Path::MachinePayload {
            state: selector,
            field: i,
        },
        Node::MachineState { state, .. } if *state == selector => Path::MachinePayload {
            state: selector,
            field: i,
        },
        Node::List { .. } => Path::List(Selector::Static(i)),
        Node::Map { .. } if index % 2 == 0 => Path::MapKey(Selector::Static(i / 2)),
        Node::Map { .. } => Path::MapValue(Selector::Static(i / 2)),
        _ => return Err(NativeResourceError::WrongFamily),
    })
}

fn copied_path(path: &[Path], next: Option<Path>) -> ResourceResult<Vec<Path>> {
    let mut out = Vec::new();
    out.try_reserve_exact(path.len() + usize::from(next.is_some()))
        .map_err(|_| NativeResourceError::Capacity)?;
    out.extend_from_slice(path);
    out.extend(next);
    Ok(out)
}

fn owners<'a>(
    graph: &crate::resource_custody::NativeCarrierLayout,
    tree: &'a NativeCarrierTree,
    path: &[Path],
    out: &mut Vec<(&'a OwnedResourceToken, Vec<Path>)>,
) -> ResourceResult<()> {
    if let Some(owner) = &tree.owner {
        out.try_reserve(1)
            .map_err(|_| NativeResourceError::Capacity)?;
        out.push((owner, copied_path(path, None)?));
    }
    for i in &tree.acquisition {
        if let Some(child) = tree.children.get(*i).and_then(Option::as_ref) {
            owners(
                graph,
                child,
                &copied_path(path, Some(child_path(graph, tree.node, tree.selector, *i)?))?,
                out,
            )?;
        }
    }
    Ok(())
}

// Borrow each distinct child once in physical order, then sort its rank vector
// into lexical acquisition order. No owner is copied or inferred from bits.
fn owners_mut<'a>(
    graph: &crate::resource_custody::NativeCarrierLayout,
    tree: &'a mut NativeCarrierTree,
    from: &[Path],
    prefix: &[Path],
    rank: &[usize],
    out: &mut Vec<(&'a mut OwnedResourceToken, Vec<Path>, Vec<Path>, Vec<usize>)>,
) -> ResourceResult<()> {
    if let Some(owner) = &mut tree.owner {
        let mut order = Vec::new();
        order
            .try_reserve_exact(rank.len())
            .map_err(|_| NativeResourceError::Capacity)?;
        order.extend_from_slice(rank);
        let mut to = copied_path(prefix, None)?;
        to.try_reserve(from.len())
            .map_err(|_| NativeResourceError::Capacity)?;
        to.extend_from_slice(from);
        out.try_reserve(1)
            .map_err(|_| NativeResourceError::Capacity)?;
        out.push((owner, copied_path(from, None)?, to, order));
    }
    for (i, child) in tree.children.iter_mut().enumerate() {
        if let Some(child) = child {
            let order = tree
                .acquisition
                .iter()
                .position(|p| *p == i)
                .ok_or(NativeResourceError::WrongOperation)?;
            let mut next_rank = Vec::new();
            next_rank
                .try_reserve_exact(rank.len() + 1)
                .map_err(|_| NativeResourceError::Capacity)?;
            next_rank.extend_from_slice(rank);
            next_rank.push(order);
            owners_mut(
                graph,
                child,
                &copied_path(from, Some(child_path(graph, tree.node, tree.selector, i)?))?,
                prefix,
                &next_rank,
                out,
            )?;
        }
    }
    Ok(())
}

fn retirement_owners<'a>(
    graph: &crate::resource_custody::NativeCarrierLayout,
    tree: &'a mut NativeCarrierTree,
) -> ResourceResult<Vec<(&'a mut OwnedResourceToken, Vec<Path>)>> {
    let mut ordered = Vec::new();
    owners_mut(graph, tree, &[], &[], &[], &mut ordered)?;
    ordered.sort_by(|a, b| a.3.cmp(&b.3));
    let mut result = Vec::new();
    result
        .try_reserve_exact(ordered.len())
        .map_err(|_| NativeResourceError::Capacity)?;
    result.extend(ordered.into_iter().map(|(owner, from, _, _)| (owner, from)));
    Ok(result)
}

impl NativeResourceState {
    pub(super) fn validate_carrier_ownership(
        &self,
        root: Option<&CarrierRootToken>,
        tree: &NativeCarrierTree,
    ) -> ResourceResult<()> {
        let mut tokens = Vec::new();
        owners(self.layout.carriers(), tree, &[], &mut tokens)?;
        match root {
            Some(root) => self
                .custody
                .validate_carrier_inventory(root, &tokens)
                .map_err(Into::into),
            None if tokens.is_empty() => Ok(()),
            None => Err(NativeResourceError::WrongFamily),
        }
    }

    pub(super) fn validate_carrier_builder(
        &self,
        ordinary: &values::NativeValues,
        builder: &NativeCarrierBuilder,
    ) -> ResourceResult<()> {
        let Op::Construct { children, .. } =
            self.carrier_operation(builder.operation, builder.frame)?
        else {
            return Err(NativeResourceError::WrongOperation);
        };
        if builder.next > children.len()
            || builder.acquisition.len() != builder.next
            || builder
                .acquisition
                .iter()
                .zip(&children)
                .any(|(i, c)| *i != c.index as usize)
            || builder.children.iter().enumerate().any(|(i, c)| {
                c.is_some()
                    != children[..builder.next]
                        .iter()
                        .any(|c| c.index as usize == i)
            })
        {
            return Err(NativeResourceError::WrongOperation);
        }
        let node = self.layout.carriers().slots[builder.destination as usize].node;
        let mut tokens = Vec::new();
        for i in &builder.acquisition {
            let tree = builder.children[*i]
                .as_ref()
                .ok_or(NativeResourceError::WrongOperation)?;
            if children[..builder.next]
                .iter()
                .find(|c| c.index as usize == *i)
                .is_none_or(|c| c.node != tree.node)
            {
                return Err(NativeResourceError::WrongFamily);
            }
            self.validate_complete_tree(ordinary, tree)?;
            let prefix = if matches!(self.layout.carriers().node(node)?, Node::Ordinary { .. }) {
                Vec::new()
            } else {
                vec![child_path(
                    self.layout.carriers(),
                    node,
                    builder.selector,
                    *i,
                )?]
            };
            owners(self.layout.carriers(), tree, &prefix, &mut tokens)?;
        }
        match &builder.custody {
            Some(root) => {
                self.custody.validate_carrier_location(
                    root,
                    &self.frame(builder.frame)?.token,
                    &self.layout.carrier_slot(builder.destination)?,
                    true,
                )?;
                self.custody.validate_carrier_inventory(root, &tokens)?;
            }
            None if tokens.is_empty() => {}
            None => return Err(NativeResourceError::WrongFamily),
        }
        Ok(())
    }

    fn builder_root(&mut self, builder: &mut NativeCarrierBuilder) -> ResourceResult<()> {
        if builder.custody.is_none() {
            let slot = self.layout.carrier_slot(builder.destination)?;
            let Some(NativeResourceEntry::Frame(frame)) = self.handles.get(&builder.frame) else {
                return Err(NativeResourceError::WrongFrame);
            };
            builder.custody = Some(self.custody.begin_carrier_root(&frame.token, &slot)?);
        }
        Ok(())
    }

    pub(in crate::native_abi::resource) fn carrier_child_with_registry(
        &mut self,
        ordinary: &values::NativeValues,
        registry: &ResourceRegistry,
        builder_id: ResourceHandleId,
        ordinal: u32,
        bits: u64,
    ) -> ResourceResult<()> {
        self.running(ordinary)?;
        let Some(NativeResourceEntry::CarrierBuilder(builder)) = self.handles.get(&builder_id)
        else {
            return Err(NativeResourceError::WrongFamily);
        };
        self.validate_carrier_builder(ordinary, builder)?;
        let Op::Construct { children, .. } =
            self.carrier_operation(builder.operation, builder.frame)?
        else {
            return Err(NativeResourceError::WrongOperation);
        };
        if ordinal as usize != builder.next {
            return Err(NativeResourceError::WrongOperation);
        }
        let child = children
            .get(builder.next)
            .ok_or(NativeResourceError::WrongOperation)?
            .clone();
        if !matches!(child.source, ChildSource::LeafOwner { .. }) {
            // Whole occupied carrier children use the same typed root migration.
            if let ChildSource::Move { slot } = child.source {
                let input = ResourceHandleId::new(bits)?;
                let root = self.carrier_root(ordinary, builder.frame, input, slot)?;
                self.validate_complete_tree(ordinary, &root.tree)?;
                if root.custody.is_some() {
                    return self.carrier_occupied_child(
                        ordinary, registry, builder_id, ordinal, input, child,
                    );
                }
            }
            return self.carrier_child(ordinary, builder_id, ordinal, bits);
        }
        let ChildSource::LeafOwner { slot } = child.source else {
            unreachable!()
        };
        let input = ResourceHandleId::new(bits)?;
        self.carrier_frame_at(builder.frame, input, slot)?;
        let owner = self.owner(input, slot)?;
        let Node::Resource { kind } = self.layout.carriers().node(child.node)? else {
            return Err(NativeResourceError::WrongFamily);
        };
        let shape = self.layout.slots()[slot as usize].shape();
        if self.layout.shapes().get(shape as usize) != Some(&NativeShape::Resource { kind: *kind })
        {
            return Err(NativeResourceError::WrongFamily);
        }
        self.custody.validate_owned(&owner.token, registry)?;
        let Some(NativeResourceEntry::CarrierBuilder(mut builder)) =
            self.handles.remove(&builder_id)
        else {
            return Err(NativeResourceError::WrongFamily);
        };
        let result = (|| {
            self.builder_root(&mut builder)?;
            let node = self.layout.carriers().slots[builder.destination as usize].node;
            let path = [child_path(
                self.layout.carriers(),
                node,
                builder.selector,
                child.index as usize,
            )?];
            let Some(NativeResourceEntry::Owner(owner)) = self.handles.get_mut(&input) else {
                return Err(NativeResourceError::WrongFamily);
            };
            self.custody.transfer_into_carrier(
                &mut owner.token,
                builder
                    .custody
                    .as_ref()
                    .ok_or(NativeResourceError::WrongFamily)?,
                &path,
                registry,
            )?;
            let Some(NativeResourceEntry::Owner(owner)) = self.handles.remove(&input) else {
                unreachable!()
            };
            builder.children[child.index as usize] = Some(NativeCarrierTree {
                node: child.node,
                selector: 0,
                ordinary: None,
                owner: Some(owner.token),
                children: Vec::new(),
                acquisition: Vec::new(),
            });
            builder.acquisition.push(child.index as usize);
            builder.next += 1;
            Ok(())
        })();
        self.handles
            .insert(builder_id, NativeResourceEntry::CarrierBuilder(builder));
        result
    }

    fn migrate_carrier_tree(
        &mut self,
        tree: &mut NativeCarrierTree,
        source: &CarrierRootToken,
        destination: &CarrierRootToken,
        prefix: &[Path],
        registry: &ResourceRegistry,
        complete: bool,
    ) -> ResourceResult<()> {
        let mut ordered = Vec::new();
        owners_mut(self.layout.carriers(), tree, &[], prefix, &[], &mut ordered)?;
        ordered.sort_by(|a, b| a.3.cmp(&b.3));
        let mut transfers = Vec::new();
        transfers
            .try_reserve_exact(ordered.len())
            .map_err(|_| NativeResourceError::Capacity)?;
        transfers.extend(
            ordered
                .into_iter()
                .map(|(token, from, to, _)| (token, from, to)),
        );
        self.custody.move_carrier_inventory(
            source,
            destination,
            &mut transfers,
            registry,
            complete,
        )?;
        Ok(())
    }

    fn carrier_occupied_child(
        &mut self,
        ordinary: &values::NativeValues,
        registry: &ResourceRegistry,
        builder_id: ResourceHandleId,
        _ordinal: u32,
        input: ResourceHandleId,
        child: crate::resource_custody::NativeCarrierChild,
    ) -> ResourceResult<()> {
        self.carrier_unborrowed(input)?;
        let Some(NativeResourceEntry::CarrierBuilder(mut builder)) =
            self.handles.remove(&builder_id)
        else {
            return Err(NativeResourceError::WrongFamily);
        };
        let result = (|| {
            self.builder_root(&mut builder)?;
            let node = self.layout.carriers().slots[builder.destination as usize].node;
            let path = [child_path(
                self.layout.carriers(),
                node,
                builder.selector,
                child.index as usize,
            )?];
            let Some(NativeResourceEntry::CarrierRoot(mut root)) = self.handles.remove(&input)
            else {
                return Err(NativeResourceError::WrongFamily);
            };
            let result = match (root.custody.as_ref(), builder.custody.as_ref()) {
                (Some(source), Some(destination)) => self.migrate_carrier_tree(
                    &mut root.tree,
                    source,
                    destination,
                    &path,
                    registry,
                    false,
                ),
                _ => Err(NativeResourceError::WrongFamily),
            };
            if let Err(error) = result {
                self.handles
                    .insert(input, NativeResourceEntry::CarrierRoot(root));
                return Err(error);
            }
            builder.children[child.index as usize] = Some(root.tree);
            builder.acquisition.push(child.index as usize);
            builder.next += 1;
            Ok(())
        })();
        self.handles
            .insert(builder_id, NativeResourceEntry::CarrierBuilder(builder));
        result
    }

    pub(in crate::native_abi::resource) fn carrier_move_reserved_with_registry(
        &mut self,
        ordinary: &values::NativeValues,
        registry: &ResourceRegistry,
        input: ResourceHandleId,
        source: u32,
        destination: u32,
        source_frame: ResourceHandleId,
        frame: ResourceHandleId,
        output: ResourceHandleId,
    ) -> ResourceResult<ResourceHandleId> {
        let root = self.carrier_root(ordinary, source_frame, input, source)?;
        self.validate_complete_tree(ordinary, &root.tree)?;
        self.carrier_unborrowed(input)?;
        self.carrier_slot_empty(frame, destination)?;
        if root.tree.node != self.layout.carriers().slots[destination as usize].node {
            return Err(NativeResourceError::WrongFamily);
        }
        if root.custody.is_none() {
            return self.carrier_move_reserved(
                ordinary,
                input,
                source,
                destination,
                source_frame,
                frame,
                output,
            );
        }
        let slot = self.layout.carrier_slot(destination)?;
        let Some(NativeResourceEntry::Frame(header)) = self.handles.get(&frame) else {
            return Err(NativeResourceError::WrongFrame);
        };
        let destination_root = self.custody.begin_carrier_root(&header.token, &slot)?;
        let Some(NativeResourceEntry::CarrierRoot(mut root)) = self.handles.remove(&input) else {
            unreachable!()
        };
        let result = match root.custody.as_ref() {
            Some(source_root) => self.migrate_carrier_tree(
                &mut root.tree,
                source_root,
                &destination_root,
                &[],
                registry,
                true,
            ),
            None => Err(NativeResourceError::WrongFamily),
        };
        if let Err(error) = result {
            self.handles
                .insert(input, NativeResourceEntry::CarrierRoot(root));
            self.custody.retire_carrier_metadata(&destination_root)?;
            return Err(error);
        }
        root.custody = Some(destination_root);
        root.frame = frame;
        root.slot = destination;
        root.generation = output.raw();
        self.handles
            .insert(output, NativeResourceEntry::CarrierRoot(root));
        Ok(output)
    }

    pub(in crate::native_abi::resource) fn carrier_transfer_with_registry(
        &mut self,
        ordinary: &values::NativeValues,
        registry: &ResourceRegistry,
        frame: ResourceHandleId,
        operation: u32,
        input: ResourceHandleId,
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
        self.carrier_move_reserved_with_registry(
            ordinary,
            registry,
            input,
            source,
            destination,
            frame,
            target,
            output,
        )
    }

    pub(in crate::native_abi::resource) fn carrier_retire_with_registry(
        &mut self,
        ordinary: &mut values::NativeValues,
        registry: &mut ResourceRegistry,
        frame: ResourceHandleId,
        operation: u32,
        input: ResourceHandleId,
    ) -> ResourceResult<()> {
        let Op::Retire { source, .. } = self.carrier_operation(operation, frame)? else {
            return Err(NativeResourceError::WrongOperation);
        };
        let root = self.carrier_root(ordinary, frame, input, source)?;
        self.carrier_unborrowed(input)?;
        if root.custody.is_none() {
            return self.carrier_retire(ordinary, frame, operation, input);
        }
        let mut tokens = Vec::new();
        owners(self.layout.carriers(), &root.tree, &[], &mut tokens)?;
        self.custody.preflight_carrier_retirement(
            root.custody
                .as_ref()
                .ok_or(NativeResourceError::WrongFamily)?,
            &tokens,
            registry,
        )?;
        let pending = self.prepare_carrier_cleanup(std::iter::once(&root.tree))?;
        let Some(NativeResourceEntry::CarrierRoot(mut root)) = self.handles.remove(&input) else {
            unreachable!()
        };
        let preparation = retirement_owners(self.layout.carriers(), &mut root.tree);
        let cleanup = match preparation {
            Ok(mut owners) => match root.custody.as_ref() {
                Some(custody) => {
                    self.custody
                        .close_carrier_inventory(custody, &mut owners, registry)
                }
                None => {
                    self.handles
                        .insert(input, NativeResourceEntry::CarrierRoot(root));
                    return Err(NativeResourceError::WrongFamily);
                }
            },
            Err(error) => {
                self.handles
                    .insert(input, NativeResourceEntry::CarrierRoot(root));
                return Err(error);
            }
        };
        if root
            .custody
            .as_ref()
            .is_none_or(|custody| !self.custody.carrier_retirement_completed(custody))
        {
            self.handles
                .insert(input, NativeResourceEntry::CarrierRoot(root));
            return match self.cleanup(cleanup) {
                Err(error) => Err(error),
                Ok(()) => Err(NativeResourceError::WrongFamily),
            };
        }
        // Core marks each owner retired before invoking its trusted finalizer.
        let core_result = self.cleanup(cleanup);
        let ordinary_result = self.drop_carrier_tree_prepared(ordinary, root.tree, pending);
        core_result?;
        ordinary_result
    }

    pub(in crate::native_abi::resource) fn prepare_carrier_frame(
        &mut self,
        ordinary: &values::NativeValues,
        frame: ResourceHandleId,
    ) -> ResourceResult<PreparedCarrierFrame> {
        if self.handles.values().any(|e| matches!(e, NativeResourceEntry::CarrierLoan(l) if l.frame != frame
            && matches!(self.handles.get(&l.root), Some(NativeResourceEntry::CarrierRoot(r)) if r.frame == frame))) {
            return Err(CustodyError::ActiveBorrow.into());
        }
        loop {
            let next = self.handles.iter().find_map(|(id, e)|
                (matches!(e, NativeResourceEntry::CarrierLoan(l) if l.frame == frame)
                && !self.handles.values().any(|e| matches!(e, NativeResourceEntry::CarrierLoan(l) if l.parent == Some(*id)))).then_some(*id));
            let Some(id) = next else {
                break;
            };
            self.carrier_end_loan(frame, id)?;
        }
        if self
            .handles
            .values()
            .any(|e| matches!(e, NativeResourceEntry::CarrierLoan(l) if l.frame == frame))
        {
            return Err(CustodyError::ActiveBorrow.into());
        }
        let mut holders = Vec::new();
        holders
            .try_reserve_exact(self.handles.len())
            .map_err(|_| NativeResourceError::Capacity)?;
        for (id, entry) in &self.handles {
            let pending = match entry {
                NativeResourceEntry::CarrierRoot(root) if root.frame == frame => {
                    self.carrier_root(ordinary, frame, *id, root.slot)?;
                    self.prepare_carrier_cleanup(std::iter::once(&root.tree))?
                }
                NativeResourceEntry::CarrierBuilder(builder) if builder.frame == frame => {
                    self.validate_carrier_builder(ordinary, builder)?;
                    let mut pending =
                        self.prepare_carrier_cleanup(builder.children.iter().flatten())?;
                    pending
                        .try_reserve(1)
                        .map_err(|_| NativeResourceError::Capacity)?;
                    pending
                }
                _ => continue,
            };
            holders.push((*id, pending));
        }
        holders.sort_by_key(|(id, _)| std::cmp::Reverse(id.raw()));
        Ok(PreparedCarrierFrame { frame, holders })
    }

    pub(in crate::native_abi::resource) fn retire_carrier_frame_prepared(
        &mut self,
        ordinary: &mut values::NativeValues,
        prepared: PreparedCarrierFrame,
    ) -> ResourceResult<()> {
        // This runs only after unified Core end_frame has discharged every scalar
        // and carrier ticket in one acquisition order. It never finalizes owners.
        let mut first = None;
        for (id, pending) in prepared.holders {
            match self.handles.get(&id) {
                Some(NativeResourceEntry::CarrierRoot(r)) if r.frame == prepared.frame => {
                    if let Some(root) = &r.custody {
                        self.custody.retire_carrier_metadata(root)?;
                    }
                }
                Some(NativeResourceEntry::CarrierBuilder(b)) if b.frame == prepared.frame => {
                    if let Some(root) = &b.custody {
                        self.custody.retire_carrier_metadata(root)?;
                    }
                }
                _ => return Err(NativeResourceError::WrongFrame),
            }
            let tree = match self.handles.remove(&id) {
                Some(NativeResourceEntry::CarrierRoot(r)) => r.tree,
                Some(NativeResourceEntry::CarrierBuilder(b)) => NativeCarrierTree {
                    node: self.layout.carriers().slots[b.destination as usize].node,
                    selector: b.selector,
                    ordinary: None,
                    owner: None,
                    children: b.children,
                    acquisition: b.acquisition,
                },
                _ => unreachable!(),
            };
            if let Err(error) = self.drop_carrier_tree_prepared(ordinary, tree, pending) {
                first.get_or_insert(error);
            }
        }
        first.map_or(Ok(()), Err)
    }
}

pub(in crate::native_abi::resource) struct PreparedCarrierFrame {
    frame: ResourceHandleId,
    holders: Vec<(ResourceHandleId, Vec<NativeCarrierTree>)>,
}
