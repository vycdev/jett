use std::collections::HashSet;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

use jett_resolve::DefId;
use jett_runtime::ResourceRegistry;

use super::{ResourceCarrier, ResourceExecutionError};
use crate::value::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct FrameId(usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct OwnerId(usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OwnerHolder {
    Temporary { frame: FrameId, slot: usize },
    Binding { frame: FrameId, definition: DefId },
}

impl OwnerHolder {
    fn frame(self) -> FrameId {
        match self {
            Self::Temporary { frame, .. } | Self::Binding { frame, .. } => frame,
        }
    }
}

/// One transferable obligation. No Clone, Copy, or finalizing Rust Drop.
#[derive(Debug)]
pub(crate) struct OwnerTicket {
    id: OwnerId,
    holder_generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OwnerState {
    Live,
    Retired,
}

struct OwnerRecord {
    carrier: ResourceCarrier,
    current_holder: OwnerHolder,
    current_holder_generation: u64,
    state: OwnerState,
}

#[derive(Debug, Clone, Copy)]
struct AcquisitionEntry {
    owner: OwnerId,
    holder: OwnerHolder,
    holder_generation: u64,
}

struct FrameLedger {
    live: bool,
    acquisitions: Vec<AcquisitionEntry>,
    borrows: HashSet<OwnerId>,
}

/// Structural location, never a schema-only request to manufacture an owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PayloadStep {
    Typed,
    Some,
    Ok,
    Fail,
    Field(usize),
    Element(usize),
    MapKey(usize),
    MapValue(usize),
}

#[derive(Debug)]
pub(crate) struct PathOwner {
    pub(crate) path: Vec<PayloadStep>,
    pub(crate) ticket: OwnerTicket,
}

#[derive(Debug, Clone)]
pub(crate) struct PathBorrow {
    pub(crate) path: Vec<PayloadStep>,
    backing: OwnerId,
    carrier: ResourceCarrier,
}

/// Physical Value copies cannot clone this ownership bundle.
#[derive(Debug, Default)]
pub(crate) struct ValueCustody {
    owned: Vec<PathOwner>,
    borrowed: Vec<PathBorrow>,
}

#[derive(Debug)]
pub(crate) struct EvaluatedValue {
    pub(crate) value: Value,
    pub(crate) custody: ValueCustody,
}

impl ValueCustody {
    pub(crate) fn is_empty(&self) -> bool {
        self.owned.is_empty() && self.borrowed.is_empty()
    }
    pub(crate) fn has_owners(&self) -> bool {
        !self.owned.is_empty()
    }
    pub(crate) fn has_borrows(&self) -> bool {
        !self.borrowed.is_empty()
    }
}

impl EvaluatedValue {
    pub(crate) fn ordinary(value: Value) -> Self {
        Self {
            value,
            custody: ValueCustody::default(),
        }
    }

    pub(crate) fn prefix(mut self, step: PayloadStep) -> Self {
        for owner in &mut self.custody.owned {
            owner.path.insert(0, step.clone());
        }
        for borrow in &mut self.custody.borrowed {
            borrow.path.insert(0, step.clone());
        }
        self
    }

    pub(crate) fn remove_prefix(
        &mut self,
        step: PayloadStep,
    ) -> Result<(), ResourceExecutionError> {
        if self
            .custody
            .owned
            .iter()
            .any(|owner| owner.path.first() != Some(&step))
            || self
                .custody
                .borrowed
                .iter()
                .any(|borrow| borrow.path.first() != Some(&step))
        {
            return Err(ResourceExecutionError::InvalidPayloadPath);
        }
        for owner in &mut self.custody.owned {
            owner.path.remove(0);
        }
        for borrow in &mut self.custody.borrowed {
            borrow.path.remove(0);
        }
        Ok(())
    }
}

#[derive(Default)]
pub(crate) struct OwnerLedger {
    records: Vec<OwnerRecord>,
    frames: Vec<FrameLedger>,
}

impl OwnerLedger {
    pub(crate) fn frame(&mut self) -> FrameId {
        let id = FrameId(self.frames.len());
        self.frames.push(FrameLedger {
            live: true,
            acquisitions: Vec::new(),
            borrows: HashSet::new(),
        });
        id
    }

    fn live_frame(&self, frame: FrameId) -> Result<(), ResourceExecutionError> {
        if self.frames.get(frame.0).is_some_and(|frame| frame.live) {
            return Ok(());
        }
        Err(ResourceExecutionError::InvalidFrame)
    }

    pub(crate) fn frame_is_live(&self, frame: FrameId) -> bool {
        self.frames.get(frame.0).is_some_and(|frame| frame.live)
    }

    pub(crate) fn acquire(
        &mut self,
        carrier: ResourceCarrier,
        holder: OwnerHolder,
    ) -> Result<ValueCustody, ResourceExecutionError> {
        self.live_frame(holder.frame())?;
        if self
            .records
            .iter()
            .any(|record| record.carrier.key == carrier.key && record.state == OwnerState::Live)
        {
            return Err(ResourceExecutionError::InvalidOwner);
        }
        let id = OwnerId(self.records.len());
        let generation = 1;
        self.records.push(OwnerRecord {
            carrier,
            current_holder: holder,
            current_holder_generation: generation,
            state: OwnerState::Live,
        });
        self.frames[holder.frame().0]
            .acquisitions
            .push(AcquisitionEntry {
                owner: id,
                holder,
                holder_generation: generation,
            });
        Ok(ValueCustody {
            owned: vec![PathOwner {
                path: Vec::new(),
                ticket: OwnerTicket {
                    id,
                    holder_generation: generation,
                },
            }],
            borrowed: Vec::new(),
        })
    }

    fn record(&self, ticket: &OwnerTicket) -> Result<&OwnerRecord, ResourceExecutionError> {
        let record = self
            .records
            .get(ticket.id.0)
            .ok_or(ResourceExecutionError::InvalidOwner)?;
        if record.state != OwnerState::Live
            || record.current_holder_generation != ticket.holder_generation
        {
            return Err(ResourceExecutionError::InvalidOwner);
        }
        self.live_frame(record.current_holder.frame())?;
        Ok(record)
    }

    fn has_active_borrow(&self, owner: OwnerId) -> bool {
        self.frames
            .iter()
            .any(|frame| frame.live && frame.borrows.contains(&owner))
    }

    pub(crate) fn hold_borrows(
        &mut self,
        custody: &ValueCustody,
        frame: FrameId,
    ) -> Result<(), ResourceExecutionError> {
        self.live_frame(frame)?;
        for borrow in &custody.borrowed {
            self.validate_borrow(borrow)?;
        }
        for borrow in &custody.borrowed {
            self.frames[frame.0].borrows.insert(borrow.backing);
        }
        Ok(())
    }

    pub(crate) fn transfer(
        &mut self,
        custody: &mut ValueCustody,
        destination: OwnerHolder,
    ) -> Result<(), ResourceExecutionError> {
        self.live_frame(destination.frame())?;
        let mut owners = HashSet::new();
        // Validate every member before changing any holder. The vector's order
        // is lexical acquisition order, never formal/field/hash ordering.
        for owner in &custody.owned {
            if !owners.insert(owner.ticket.id) {
                return Err(ResourceExecutionError::InvalidOwner);
            }
            if self.has_active_borrow(owner.ticket.id) {
                return Err(ResourceExecutionError::InvalidBorrow);
            }
            self.record(&owner.ticket)?
                .current_holder_generation
                .checked_add(1)
                .ok_or(ResourceExecutionError::IdentityExhausted)?;
        }
        for owner in &mut custody.owned {
            let record = &mut self.records[owner.ticket.id.0];
            record.current_holder_generation += 1;
            record.current_holder = destination;
            owner.ticket.holder_generation = record.current_holder_generation;
            self.frames[destination.frame().0]
                .acquisitions
                .push(AcquisitionEntry {
                    owner: owner.ticket.id,
                    holder: destination,
                    holder_generation: record.current_holder_generation,
                });
        }
        Ok(())
    }

    pub(crate) fn borrow(
        &self,
        value: &EvaluatedValue,
    ) -> Result<EvaluatedValue, ResourceExecutionError> {
        let mut borrowed = value.custody.borrowed.clone();
        for borrow in &borrowed {
            self.validate_borrow(borrow)?;
        }
        for owner in &value.custody.owned {
            let record = self.record(&owner.ticket)?;
            borrowed.push(PathBorrow {
                path: owner.path.clone(),
                backing: owner.ticket.id,
                carrier: record.carrier.clone(),
            });
        }
        Ok(EvaluatedValue {
            value: value.value.clone(),
            custody: ValueCustody {
                owned: Vec::new(),
                borrowed,
            },
        })
    }

    fn validate_borrow(&self, borrow: &PathBorrow) -> Result<(), ResourceExecutionError> {
        let record = self
            .records
            .get(borrow.backing.0)
            .ok_or(ResourceExecutionError::InvalidBorrow)?;
        if record.state != OwnerState::Live || record.carrier.key != borrow.carrier.key {
            return Err(ResourceExecutionError::InvalidBorrow);
        }
        self.live_frame(record.current_holder.frame())?;
        Ok(())
    }

    pub(crate) fn borrowed_carrier(
        &self,
        custody: &ValueCustody,
    ) -> Result<&ResourceCarrier, ResourceExecutionError> {
        if !custody.owned.is_empty() {
            return Err(ResourceExecutionError::InvalidBorrow);
        }
        let [borrow] = custody.borrowed.as_slice() else {
            return Err(ResourceExecutionError::InvalidBorrow);
        };
        if !borrow.path.is_empty() {
            return Err(ResourceExecutionError::InvalidPayloadPath);
        }
        self.validate_borrow(borrow)?;
        Ok(&self.records[borrow.backing.0].carrier)
    }

    pub(crate) fn validate_positions(
        &self,
        custody: &ValueCustody,
        positions: &[(Vec<PayloadStep>, &ResourceCarrier)],
    ) -> Result<(), ResourceExecutionError> {
        if positions.len() != custody.owned.len() + custody.borrowed.len() {
            return Err(ResourceExecutionError::InvalidPayloadPath);
        }
        let mut claimed = HashSet::new();
        for owner in &custody.owned {
            let record = self.record(&owner.ticket)?;
            let candidates = positions
                .iter()
                .enumerate()
                .filter(|(_, (path, carrier))| {
                    path.as_slice() == owner.path.as_slice()
                        && carrier.key == record.carrier.key
                        && carrier.resource == record.carrier.resource
                        && std::sync::Arc::ptr_eq(&carrier.program, &record.carrier.program)
                })
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            let [index] = candidates.as_slice() else {
                return Err(ResourceExecutionError::InvalidPayloadPath);
            };
            if !claimed.insert(*index) {
                return Err(ResourceExecutionError::InvalidOwner);
            }
        }
        for borrow in &custody.borrowed {
            self.validate_borrow(borrow)?;
            let candidates = positions
                .iter()
                .enumerate()
                .filter(|(_, (path, carrier))| {
                    path.as_slice() == borrow.path.as_slice()
                        && carrier.key == borrow.carrier.key
                        && carrier.resource == borrow.carrier.resource
                        && std::sync::Arc::ptr_eq(&carrier.program, &borrow.carrier.program)
                })
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            let [index] = candidates.as_slice() else {
                return Err(ResourceExecutionError::InvalidPayloadPath);
            };
            if !claimed.insert(*index) {
                return Err(ResourceExecutionError::InvalidBorrow);
            }
        }
        Ok(())
    }

    pub(crate) fn close(
        &mut self,
        custody: ValueCustody,
        registry: &mut ResourceRegistry,
    ) -> Result<(), ResourceExecutionError> {
        if !custody.borrowed.is_empty() {
            return Err(ResourceExecutionError::InvalidOwner);
        }
        let [owner] = custody.owned.as_slice() else {
            return Err(ResourceExecutionError::MissingOwner);
        };
        if !owner.path.is_empty() {
            return Err(ResourceExecutionError::InvalidPayloadPath);
        }
        if self.has_active_borrow(owner.ticket.id) {
            return Err(ResourceExecutionError::InvalidBorrow);
        }
        let record = self.record(&owner.ticket)?;
        let key = record.carrier.key;
        let resource_type = record.carrier.resource.registry_type;
        // Retire before the finalizer, including if it raises a host panic.
        self.records[owner.ticket.id.0].state = OwnerState::Retired;
        registry.close(key, resource_type)?;
        Ok(())
    }

    pub(crate) fn unwind(
        &mut self,
        frame: FrameId,
        registry: &mut ResourceRegistry,
    ) -> Result<(), ResourceExecutionError> {
        self.live_frame(frame)?;
        self.frames[frame.0].borrows.clear();
        let protected = self.frames[frame.0].acquisitions.iter().any(|entry| {
            let record = &self.records[entry.owner.0];
            record.state == OwnerState::Live
                && record.current_holder == entry.holder
                && record.current_holder_generation == entry.holder_generation
                && self.has_active_borrow(entry.owner)
        });
        if protected {
            return Err(ResourceExecutionError::InvalidBorrow);
        }
        let entries = std::mem::take(&mut self.frames[frame.0].acquisitions);
        let mut failure = None;
        let mut panic = None;
        for entry in entries.into_iter().rev() {
            let record = &mut self.records[entry.owner.0];
            if record.state != OwnerState::Live
                || record.current_holder != entry.holder
                || record.current_holder_generation != entry.holder_generation
            {
                continue;
            }
            record.state = OwnerState::Retired;
            let finalized = catch_unwind(AssertUnwindSafe(|| {
                registry.close(record.carrier.key, record.carrier.resource.registry_type)
            }));
            match finalized {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    failure.get_or_insert(ResourceExecutionError::Registry(error));
                }
                Err(payload) if panic.is_none() => panic = Some(payload),
                Err(payload) => std::mem::forget(payload),
            }
        }
        self.frames[frame.0].live = false;
        if let Some(payload) = panic {
            resume_unwind(payload);
        }
        failure.map_or(Ok(()), Err)
    }

    pub(crate) fn live_owners(&self) -> usize {
        self.records
            .iter()
            .filter(|record| record.state == OwnerState::Live)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource_execution::{ResourceTypeBinding, tests::program};
    use jett_runtime::{AuthorityProvenance, ResourceTypeId};
    use std::sync::{Arc, Mutex};

    fn carrier(
        registry: &mut ResourceRegistry,
        label: i64,
        events: Arc<Mutex<Vec<i64>>>,
        panic: bool,
    ) -> ResourceCarrier {
        let program = program(include_str!("fixtures/02_reverse_scope_drop.jett"), false);
        let hook = program.checked().resource_hooks.values().next().unwrap();
        let resource = ResourceTypeBinding {
            definition: hook.resource_definition,
            checked_type: hook.resource_type,
            registry_type: ResourceTypeId::new(0),
        };
        let key = registry
            .insert(
                resource.registry_type,
                label,
                AuthorityProvenance::new(1, 1),
                move |label| {
                    events.lock().unwrap().push(label);
                    if panic {
                        panic!("selected test finalizer");
                    }
                },
            )
            .unwrap();
        ResourceCarrier {
            program,
            resource,
            key,
        }
    }

    #[test]
    fn holder_generation_prevents_old_entries_from_finalizing_returned_storage() {
        let mut registry = ResourceRegistry::new();
        let mut ledger = OwnerLedger::default();
        let events = Arc::new(Mutex::new(Vec::new()));
        let outer = ledger.frame();
        let inner = ledger.frame();
        let original = OwnerHolder::Temporary {
            frame: outer,
            slot: 0,
        };
        let mut first = ledger
            .acquire(carrier(&mut registry, 1, events.clone(), false), original)
            .unwrap();
        let second = ledger
            .acquire(
                carrier(&mut registry, 2, events.clone(), false),
                OwnerHolder::Temporary {
                    frame: outer,
                    slot: 1,
                },
            )
            .unwrap();
        ledger
            .transfer(
                &mut first,
                OwnerHolder::Temporary {
                    frame: inner,
                    slot: 0,
                },
            )
            .unwrap();
        ledger.transfer(&mut first, original).unwrap();
        ledger.unwind(inner, &mut registry).unwrap();
        assert!(events.lock().unwrap().is_empty());
        ledger.unwind(outer, &mut registry).unwrap();
        assert_eq!(*events.lock().unwrap(), [1, 2]); // Latest holder acquisitions, not original registry order.
        assert_eq!(ledger.live_owners(), 0);
        assert_eq!(registry.live_count(), 0);
        drop((first, second));
        assert_eq!(*events.lock().unwrap(), [1, 2]);
    }

    #[test]
    fn borrowed_operation_protects_owner_before_transfer_or_close_effect() {
        let mut registry = ResourceRegistry::new();
        let mut ledger = OwnerLedger::default();
        let events = Arc::new(Mutex::new(Vec::new()));
        let owner = ledger.frame();
        let operation = ledger.frame();
        let carrier = carrier(&mut registry, 3, events.clone(), false);
        let mut value = EvaluatedValue {
            value: Value::Resource(carrier.clone()),
            custody: ledger
                .acquire(
                    carrier,
                    OwnerHolder::Temporary {
                        frame: owner,
                        slot: 0,
                    },
                )
                .unwrap(),
        };
        let borrowed = ledger.borrow(&value).unwrap();
        ledger.hold_borrows(&borrowed.custody, operation).unwrap();
        assert_eq!(
            ledger.transfer(
                &mut value.custody,
                OwnerHolder::Temporary {
                    frame: operation,
                    slot: 0
                }
            ),
            Err(ResourceExecutionError::InvalidBorrow)
        );
        assert!(events.lock().unwrap().is_empty());
        assert_eq!(registry.live_count(), 1);
        ledger.unwind(operation, &mut registry).unwrap();
        ledger.close(value.custody, &mut registry).unwrap();
        assert_eq!(*events.lock().unwrap(), [3]);
        assert_eq!(ledger.live_owners(), 0);
        assert_eq!(
            ledger.borrowed_carrier(&borrowed.custody).unwrap_err(),
            ResourceExecutionError::InvalidBorrow
        );
        ledger.unwind(owner, &mut registry).unwrap();
        assert_eq!(*events.lock().unwrap(), [3]);
    }

    #[test]
    fn copied_carrier_or_foreign_payload_cannot_create_custody() {
        let mut registry = ResourceRegistry::new();
        let mut ledger = OwnerLedger::default();
        let events = Arc::new(Mutex::new(Vec::new()));
        let frame = ledger.frame();
        let carrier = carrier(&mut registry, 4, events.clone(), false);
        let custody = ledger
            .acquire(carrier.clone(), OwnerHolder::Temporary { frame, slot: 0 })
            .unwrap();
        assert_eq!(
            ledger.validate_positions(&ValueCustody::default(), &[(Vec::new(), &carrier)]),
            Err(ResourceExecutionError::InvalidPayloadPath)
        );
        assert_eq!(
            ledger
                .acquire(carrier.clone(), OwnerHolder::Temporary { frame, slot: 1 })
                .unwrap_err(),
            ResourceExecutionError::InvalidOwner
        );
        let foreign = ResourceCarrier {
            program: program(include_str!("fixtures/02_reverse_scope_drop.jett"), false),
            ..carrier.clone()
        };
        assert_eq!(
            ledger.validate_positions(&custody, &[(Vec::new(), &foreign)]),
            Err(ResourceExecutionError::InvalidPayloadPath)
        );
        ledger.unwind(frame, &mut registry).unwrap();
        assert_eq!(*events.lock().unwrap(), [4]);
    }

    #[test]
    fn finalizer_panic_retires_before_panic_and_continues_every_remaining_owner() {
        let mut registry = ResourceRegistry::new();
        let mut ledger = OwnerLedger::default();
        let events = Arc::new(Mutex::new(Vec::new()));
        let frame = ledger.frame();
        let first = ledger
            .acquire(
                carrier(&mut registry, 5, events.clone(), false),
                OwnerHolder::Temporary { frame, slot: 0 },
            )
            .unwrap();
        let second = ledger
            .acquire(
                carrier(&mut registry, 6, events.clone(), true),
                OwnerHolder::Temporary { frame, slot: 1 },
            )
            .unwrap();
        assert!(catch_unwind(AssertUnwindSafe(|| ledger.unwind(frame, &mut registry))).is_err());
        assert_eq!(*events.lock().unwrap(), [6, 5]);
        assert_eq!(ledger.live_owners(), 0);
        assert_eq!(registry.live_count(), 0);
        assert_eq!(
            ledger.unwind(frame, &mut registry),
            Err(ResourceExecutionError::InvalidFrame)
        );
        drop((first, second));
        drop(registry);
        assert_eq!(*events.lock().unwrap(), [6, 5]);
    }
}
