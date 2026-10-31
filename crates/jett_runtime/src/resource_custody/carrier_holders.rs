//! Private typed carrier holders. Physical tree bits never issue ownership.
use super::*;

/// Issued only by the validated NativeLayoutInstallation child.
#[derive(Debug, Clone)]
pub(crate) struct RegisteredCarrierSlot {
    pub(super) program: Arc<ProgramLayout>,
    pub(super) graph: Arc<NativeCarrierLayout>,
    pub(super) index: usize,
}

/// Neither Clone/Copy nor a finalizing Rust Drop. A move consumes this identity.
#[derive(Debug)]
pub(crate) struct CarrierRootToken {
    pub(super) core: u64,
    pub(super) index: usize,
    pub(super) generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CarrierPhase {
    Building,
    Ready,
    Retired,
}

struct CarrierHolderPath {
    path: Vec<NativeCarrierPath>,
    kind: usize,
}

pub(super) struct CarrierHolderRoot {
    frame: usize,
    slot: RegisteredCarrierSlot,
    generation: u64,
    phase: CarrierPhase,
    paths: Vec<CarrierHolderPath>,
}

impl ResourceCustody {
    fn carrier_root_record(
        &self,
        token: &CarrierRootToken,
    ) -> Result<&CarrierHolderRoot, CustodyError> {
        if token.core != self.id {
            return Err(CustodyError::WrongProgram);
        }
        self.carrier_roots
            .get(token.index)
            .filter(|r| r.generation == token.generation && r.phase != CarrierPhase::Retired)
            .ok_or(CustodyError::StaleHolderGeneration)
    }

    pub(crate) fn begin_carrier_root(
        &mut self,
        frame: &ResourceFrameToken,
        slot: &RegisteredCarrierSlot,
    ) -> Result<CarrierRootToken, CustodyError> {
        let frame = self.frame_token(frame)?;
        self.runtime_frame(frame)?;
        if !Arc::ptr_eq(&slot.program, &self.program.0) {
            return Err(CustodyError::WrongProgram);
        }
        slot.graph
            .slots
            .get(slot.index)
            .ok_or(CustodyError::UnknownSlot)?;
        if self.carrier_roots.iter().any(|r| {
            r.phase != CarrierPhase::Retired && r.frame == frame && r.slot.index == slot.index
        }) {
            return Err(CustodyError::OccupiedHolder);
        }
        self.carrier_roots
            .try_reserve(1)
            .map_err(|_| CustodyError::CapacityExhausted)?;
        let index = self.carrier_roots.len();
        self.carrier_roots.push(CarrierHolderRoot {
            frame,
            slot: slot.clone(),
            generation: 1,
            phase: CarrierPhase::Building,
            paths: Vec::new(),
        });
        Ok(CarrierRootToken {
            core: self.id,
            index,
            generation: 1,
        })
    }

    fn carrier_holder(
        &mut self,
        token: &CarrierRootToken,
        path: &[NativeCarrierPath],
    ) -> Result<ResourceHolderToken, CustodyError> {
        let record = self.carrier_root_record(token)?;
        self.runtime_frame(record.frame)?;
        // A dynamic selection must be normalized by the exact reached runtime row.
        if path.iter().any(|p| {
            matches!(
                p,
                NativeCarrierPath::List(NativeCarrierSelector::Dynamic)
                    | NativeCarrierPath::MapKey(NativeCarrierSelector::Dynamic)
                    | NativeCarrierPath::MapValue(NativeCarrierSelector::Dynamic)
            )
        }) {
            return Err(CustodyError::UnknownSlot);
        }
        let node = record.slot.graph.slots[record.slot.index].node;
        let leaf = record
            .slot
            .graph
            .path_node(node, path)
            .map_err(|_| CustodyError::UnknownSlot)?;
        let NativeCarrierNode::Resource { kind } = record
            .slot
            .graph
            .node(leaf)
            .map_err(|_| CustodyError::UnknownSlot)?
        else {
            return Err(CustodyError::WrongKind);
        };
        let kind = *kind as usize;
        self.program
            .0
            .kinds
            .get(kind)
            .ok_or(CustodyError::WrongKind)?;
        let frame = record.frame;
        let path_index = if let Some(i) = record.paths.iter().position(|p| p.path == path) {
            if record.paths[i].kind != kind {
                return Err(CustodyError::WrongKind);
            }
            i
        } else {
            let mut owned_path = Vec::new();
            owned_path
                .try_reserve_exact(path.len())
                .map_err(|_| CustodyError::CapacityExhausted)?;
            owned_path.extend_from_slice(path);
            let record = &mut self.carrier_roots[token.index];
            record
                .paths
                .try_reserve(1)
                .map_err(|_| CustodyError::CapacityExhausted)?;
            let i = record.paths.len();
            record.paths.push(CarrierHolderPath {
                path: owned_path,
                kind,
            });
            i
        };
        Ok(ResourceHolderToken {
            core: self.id,
            holder: Holder {
                frame,
                address: HolderAddress::Carrier {
                    root: token.index,
                    path: path_index,
                },
            },
        })
    }

    pub(crate) fn transfer_into_carrier(
        &mut self,
        owner: &mut OwnedResourceToken,
        root: &CarrierRootToken,
        path: &[NativeCarrierPath],
        registry: &ResourceRegistry,
    ) -> Result<(), CustodyError> {
        if self.carrier_root_record(root)?.phase != CarrierPhase::Building {
            return Err(CustodyError::InvalidOwner);
        }
        let destination = self.carrier_holder(root, path)?;
        self.transfer(owner, &destination, registry)
    }

    pub(crate) fn validate_carrier_ticket(
        &self,
        root: &CarrierRootToken,
        path: &[NativeCarrierPath],
        owner: &OwnedResourceToken,
    ) -> Result<(), CustodyError> {
        let record = self.carrier_root_record(root)?;
        let owner = self.owner(owner)?;
        let HolderAddress::Carrier {
            root: index,
            path: p,
        } = owner.holder.address
        else {
            return Err(CustodyError::InvalidOwner);
        };
        if index != root.index
            || owner.holder.frame != record.frame
            || record
                .paths
                .get(p)
                .is_none_or(|r| r.path != path || r.kind != owner.kind)
        {
            return Err(CustodyError::InvalidOwner);
        }
        Ok(())
    }

    /// The complete physical tree and Core inventory must match in both directions.
    pub(crate) fn validate_carrier_inventory(
        &self,
        root: &CarrierRootToken,
        owners: &[(&OwnedResourceToken, Vec<NativeCarrierPath>)],
    ) -> Result<(), CustodyError> {
        self.carrier_root_record(root)?;
        for (i, (owner, path)) in owners.iter().enumerate() {
            self.validate_carrier_ticket(root, path, owner)?;
            if owners[..i]
                .iter()
                .any(|(previous, p)| previous.owner == owner.owner || p == path)
            {
                return Err(CustodyError::OccupiedHolder);
            }
        }
        let live = self.owners.iter().filter(|o| o.live
            && matches!(o.holder.address, HolderAddress::Carrier { root: r, .. } if r == root.index)).count();
        if live != owners.len() {
            return Err(CustodyError::InvalidOwner);
        }
        Ok(())
    }

    pub(crate) fn complete_carrier_root(
        &mut self,
        root: &CarrierRootToken,
    ) -> Result<(), CustodyError> {
        if self.carrier_root_record(root)?.phase != CarrierPhase::Building {
            return Err(CustodyError::InvalidOwner);
        }
        self.carrier_roots[root.index].phase = CarrierPhase::Ready;
        Ok(())
    }

    pub(crate) fn validate_carrier_location(
        &self,
        root: &CarrierRootToken,
        frame: &ResourceFrameToken,
        slot: &RegisteredCarrierSlot,
        building: bool,
    ) -> Result<(), CustodyError> {
        let frame = self.frame_token(frame)?;
        let record = self.carrier_root_record(root)?;
        if !Arc::ptr_eq(&slot.program, &self.program.0)
            || record.slot.index != slot.index
            || record.slot.graph.as_ref() != slot.graph.as_ref()
            || record.frame != frame
            || record.phase
                != if building {
                    CarrierPhase::Building
                } else {
                    CarrierPhase::Ready
                }
        {
            return Err(CustodyError::InvalidOwner);
        }
        Ok(())
    }

    /// A reached tree is transferred in its lexical acquisition order. All checks
    /// and capacity reservation precede the first owning-token mutation.
    pub(crate) fn move_carrier_inventory(
        &mut self,
        source: &CarrierRootToken,
        destination: &CarrierRootToken,
        owners: &mut [(
            &mut OwnedResourceToken,
            Vec<NativeCarrierPath>,
            Vec<NativeCarrierPath>,
        )],
        registry: &ResourceRegistry,
        complete_destination: bool,
    ) -> Result<(), CustodyError> {
        if self.carrier_root_record(source)?.phase != CarrierPhase::Ready {
            return Err(CustodyError::InvalidOwner);
        }
        let target = self.carrier_root_record(destination)?;
        if target.phase != CarrierPhase::Building || source.index == destination.index {
            return Err(CustodyError::OccupiedHolder);
        }
        let target_frame = target.frame;
        let mut checked = Vec::new();
        checked
            .try_reserve_exact(owners.len())
            .map_err(|_| CustodyError::CapacityExhausted)?;
        for (token, from, to) in owners.iter() {
            self.validate_carrier_ticket(source, from, token)?;
            let holder = self.carrier_holder(destination, to)?;
            let (holder, generation) = self.checked_transfer(token, &holder, registry)?;
            if checked
                .iter()
                .any(|(previous, h, _)| *previous == token.owner || *h == holder)
            {
                return Err(CustodyError::OccupiedHolder);
            }
            checked.push((token.owner, holder, generation));
        }
        let live = self.owners.iter().filter(|o| o.live
            && matches!(o.holder.address, HolderAddress::Carrier { root: r, .. } if r == source.index)).count();
        if live != owners.len() {
            return Err(CustodyError::InvalidOwner);
        }
        self.frames[target_frame]
            .acquisitions
            .try_reserve(owners.len())
            .map_err(|_| CustodyError::CapacityExhausted)?;
        for ((token, _, _), (owner, holder, generation)) in owners.iter_mut().zip(checked) {
            self.owners[owner].holder = holder;
            self.owners[owner].generation = generation;
            self.frames[target_frame].acquisitions.push(Acquisition {
                owner,
                holder,
                generation,
            });
            token.holder_generation = generation;
        }
        self.carrier_roots[source.index].phase = CarrierPhase::Retired;
        if complete_destination {
            self.carrier_roots[destination.index].phase = CarrierPhase::Ready;
        }
        Ok(())
    }

    pub(crate) fn retire_carrier_metadata(
        &mut self,
        root: &CarrierRootToken,
    ) -> Result<(), CustodyError> {
        self.carrier_root_record(root)?;
        if self.owners.iter().any(|o| o.live
            && matches!(o.holder.address, HolderAddress::Carrier { root: r, .. } if r == root.index)) {
            return Err(CustodyError::InvalidOwner);
        }
        self.carrier_roots[root.index].phase = CarrierPhase::Retired;
        Ok(())
    }

    pub(crate) fn preflight_carrier_retirement(
        &self,
        root: &CarrierRootToken,
        owners: &[(&OwnedResourceToken, Vec<NativeCarrierPath>)],
        registry: &ResourceRegistry,
    ) -> Result<(), CustodyError> {
        self.validate_carrier_inventory(root, owners)?;
        for (token, _) in owners {
            self.validate_registry_owner(self.owner(token)?, registry)?;
            if self.borrowed(token.owner) {
                return Err(CustodyError::ActiveBorrow);
            }
        }
        Ok(())
    }

    pub(crate) fn close_carrier_inventory(
        &mut self,
        root: &CarrierRootToken,
        owners: &mut [(&mut OwnedResourceToken, Vec<NativeCarrierPath>)],
        registry: &mut ResourceRegistry,
    ) -> ResourceCleanupOutcome {
        // The caller preflights the exact inventory before removing its shell.
        // Recheck all identity/count/borrow facts without allocating a receipt.
        let mut outcome = ResourceCleanupOutcome::default();
        let checked = (|| {
            self.carrier_root_record(root)?;
            for (i, (token, path)) in owners.iter().enumerate() {
                self.validate_carrier_ticket(root, path, token)?;
                self.validate_registry_owner(self.owner(token)?, registry)?;
                if self.borrowed(token.owner) {
                    return Err(CustodyError::ActiveBorrow);
                }
                if owners[..i]
                    .iter()
                    .any(|(p, old)| p.owner == token.owner || old == path)
                {
                    return Err(CustodyError::OccupiedHolder);
                }
            }
            let live = self
                .owners
                .iter()
                .filter(|o| {
                    o.live
                        && matches!(o.holder.address,
                HolderAddress::Carrier { root: r, .. } if r == root.index)
                })
                .count();
            if live != owners.len() {
                return Err(CustodyError::InvalidOwner);
            }
            Ok(())
        })();
        if let Err(error) = checked {
            outcome.fail(ResourceCleanupFailure::Custody(error));
            return outcome;
        }
        for (token, _) in owners.iter_mut().rev() {
            outcome.merge(self.close(token, registry));
        }
        if let Err(error) = self.retire_carrier_metadata(root) {
            outcome.fail(ResourceCleanupFailure::Custody(error));
        }
        outcome
    }

    pub(super) fn holder_kind(&self, holder: Holder) -> Result<usize, CustodyError> {
        self.frame(holder.frame)?;
        match holder.address {
            HolderAddress::Slot(slot) => self
                .program
                .0
                .slots
                .get(slot)
                .copied()
                .ok_or(CustodyError::UnknownSlot),
            HolderAddress::Carrier { root, path } => {
                let r = self
                    .carrier_roots
                    .get(root)
                    .filter(|r| r.phase != CarrierPhase::Retired && r.frame == holder.frame)
                    .ok_or(CustodyError::StaleHolderGeneration)?;
                r.paths
                    .get(path)
                    .map(|p| p.kind)
                    .ok_or(CustodyError::UnknownSlot)
            }
        }
    }

    pub(crate) fn live_carrier_roots(&self) -> usize {
        self.carrier_roots
            .iter()
            .filter(|r| r.phase != CarrierPhase::Retired)
            .count()
    }

    pub(crate) fn carrier_retirement_completed(&self, root: &CarrierRootToken) -> bool {
        root.core == self.id
            && self.carrier_roots.get(root.index).is_some_and(|record| {
                record.generation == root.generation && record.phase == CarrierPhase::Retired
            })
    }
}
