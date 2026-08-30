//! Backend-neutral runtime services for Jett execution contexts.

use std::any::Any;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_CONTEXT_ID: AtomicU64 = AtomicU64::new(1);

type ErasedPayload = Box<dyn Any + Send>;
type Finalizer = Box<dyn FnOnce(ErasedPayload) + Send>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResourceTypeId(u32);

impl ResourceTypeId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AuthorityProvenance {
    provider: u64,
    restriction: u64,
}

impl AuthorityProvenance {
    pub const fn new(provider: u64, restriction: u64) -> Self {
        Self {
            provider,
            restriction,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResourceKey {
    context_id: u64,
    resource_type: ResourceTypeId,
    slot: u32,
    generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OperationKey {
    resource: ResourceKey,
    generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryError {
    WrongContext,
    WrongType,
    AuthorityMismatch,
    PayloadTypeMismatch,
    PendingOperation,
    NoPendingOperation,
    StaleOperation,
    UnknownSlot,
    StaleGeneration,
    Retired,
    ShuttingDown,
}

struct PendingOperation {
    generation: u64,
    detach: Box<dyn FnOnce() + Send>,
}

struct Entry {
    resource_type: ResourceTypeId,
    authority: AuthorityProvenance,
    payload: ErasedPayload,
    finalizer: Finalizer,
    creation_sequence: u64,
    next_operation_generation: u64,
    pending: Option<PendingOperation>,
}

struct Slot {
    generation: u64,
    entry: Option<Entry>,
}

pub struct ResourceRegistry {
    context_id: u64,
    slots: Vec<Slot>,
    free_slots: Vec<usize>,
    live_count: usize,
    next_creation_sequence: u64,
    shutting_down: bool,
}

impl Default for ResourceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceRegistry {
    pub fn new() -> Self {
        Self {
            context_id: NEXT_CONTEXT_ID.fetch_add(1, Ordering::Relaxed),
            slots: Vec::new(),
            free_slots: Vec::new(),
            live_count: 0,
            next_creation_sequence: 1,
            shutting_down: false,
        }
    }

    pub fn insert<T, F>(
        &mut self,
        resource_type: ResourceTypeId,
        payload: T,
        authority: AuthorityProvenance,
        finalizer: F,
    ) -> Result<ResourceKey, RegistryError>
    where
        T: Any + Send,
        F: FnOnce(T) + Send + 'static,
    {
        if self.shutting_down {
            return Err(RegistryError::ShuttingDown);
        }

        let erased_finalizer: Finalizer = Box::new(move |payload| {
            let payload = payload
                .downcast::<T>()
                .expect("resource payload type changed before finalization");
            finalizer(*payload);
        });
        let entry = Entry {
            resource_type,
            authority,
            payload: Box::new(payload),
            finalizer: erased_finalizer,
            creation_sequence: self.next_creation_sequence,
            next_operation_generation: 1,
            pending: None,
        };
        self.next_creation_sequence = self.next_creation_sequence.wrapping_add(1).max(1);
        let slot_index = if let Some(slot_index) = self.free_slots.pop() {
            self.slots[slot_index].entry = Some(entry);
            slot_index
        } else {
            self.slots.push(Slot {
                generation: 1,
                entry: Some(entry),
            });
            self.slots.len() - 1
        };
        self.live_count += 1;
        Ok(ResourceKey {
            context_id: self.context_id,
            resource_type,
            slot: slot_index as u32,
            generation: self.slots[slot_index].generation,
        })
    }

    pub fn close(
        &mut self,
        key: ResourceKey,
        expected_type: ResourceTypeId,
    ) -> Result<(), RegistryError> {
        let slot_index = self.validate_slot(key, expected_type)?;
        self.finalize_slot(slot_index);
        Ok(())
    }

    pub fn access<T, R, F>(
        &mut self,
        key: ResourceKey,
        expected_type: ResourceTypeId,
        authority: &AuthorityProvenance,
        operation: F,
    ) -> Result<R, RegistryError>
    where
        T: Any + Send,
        F: FnOnce(&mut T) -> R,
    {
        let slot_index = self.validate_slot(key, expected_type)?;
        let entry = self.slots[slot_index]
            .entry
            .as_mut()
            .expect("validated resource entry disappeared");
        if entry.authority != *authority {
            return Err(RegistryError::AuthorityMismatch);
        }
        let payload = entry
            .payload
            .downcast_mut::<T>()
            .ok_or(RegistryError::PayloadTypeMismatch)?;
        Ok(operation(payload))
    }

    pub fn begin_pending<F>(
        &mut self,
        key: ResourceKey,
        expected_type: ResourceTypeId,
        authority: &AuthorityProvenance,
        detach: F,
    ) -> Result<OperationKey, RegistryError>
    where
        F: FnOnce() + Send + 'static,
    {
        let slot_index = self.validate_slot(key, expected_type)?;
        let entry = self.slots[slot_index]
            .entry
            .as_mut()
            .expect("validated resource entry disappeared");
        if entry.authority != *authority {
            return Err(RegistryError::AuthorityMismatch);
        }
        if entry.pending.is_some() {
            return Err(RegistryError::PendingOperation);
        }
        let generation = entry.next_operation_generation;
        entry.next_operation_generation = generation.wrapping_add(1).max(1);
        entry.pending = Some(PendingOperation {
            generation,
            detach: Box::new(detach),
        });
        Ok(OperationKey {
            resource: key,
            generation,
        })
    }

    pub fn complete_pending(&mut self, operation: OperationKey) -> Result<(), RegistryError> {
        let slot_index =
            self.validate_slot(operation.resource, operation.resource.resource_type)?;
        let entry = self.slots[slot_index]
            .entry
            .as_mut()
            .expect("validated resource entry disappeared");
        let pending = entry
            .pending
            .as_ref()
            .ok_or(RegistryError::NoPendingOperation)?;
        if pending.generation != operation.generation {
            return Err(RegistryError::StaleOperation);
        }
        entry.pending.take();
        Ok(())
    }

    pub fn cancel_pending(&mut self, operation: OperationKey) -> Result<(), RegistryError> {
        let slot_index =
            self.validate_slot(operation.resource, operation.resource.resource_type)?;
        let entry = self.slots[slot_index]
            .entry
            .as_mut()
            .expect("validated resource entry disappeared");
        let pending = entry
            .pending
            .as_ref()
            .ok_or(RegistryError::NoPendingOperation)?;
        if pending.generation != operation.generation {
            return Err(RegistryError::StaleOperation);
        }
        let pending = entry
            .pending
            .take()
            .expect("validated pending operation disappeared");
        (pending.detach)();
        Ok(())
    }

    pub fn live_count(&self) -> usize {
        self.live_count
    }

    pub fn shutdown(&mut self) {
        if self.shutting_down {
            return;
        }
        self.shutting_down = true;
        let mut live_slots = self
            .slots
            .iter()
            .enumerate()
            .filter_map(|(slot_index, slot)| {
                slot.entry
                    .as_ref()
                    .map(|entry| (entry.creation_sequence, slot_index))
            })
            .collect::<Vec<_>>();
        live_slots.sort_unstable_by_key(|(sequence, _)| std::cmp::Reverse(*sequence));
        for (_, slot_index) in live_slots {
            self.finalize_slot(slot_index);
        }
    }

    fn validate_slot(
        &self,
        key: ResourceKey,
        expected_type: ResourceTypeId,
    ) -> Result<usize, RegistryError> {
        if key.context_id != self.context_id {
            return Err(RegistryError::WrongContext);
        }
        if key.resource_type != expected_type {
            return Err(RegistryError::WrongType);
        }
        let slot_index = key.slot as usize;
        let Some(slot) = self.slots.get(slot_index) else {
            return Err(RegistryError::UnknownSlot);
        };
        if slot.generation != key.generation {
            return Err(RegistryError::StaleGeneration);
        }
        let Some(entry) = &slot.entry else {
            return Err(RegistryError::Retired);
        };
        if entry.resource_type != expected_type {
            return Err(RegistryError::WrongType);
        }
        Ok(slot_index)
    }

    fn retire_slot(&mut self, slot_index: usize) {
        let slot = &mut self.slots[slot_index];
        slot.generation = slot.generation.wrapping_add(1).max(1);
        self.free_slots.push(slot_index);
        self.live_count -= 1;
    }

    fn finalize_slot(&mut self, slot_index: usize) {
        let mut entry = self.slots[slot_index]
            .entry
            .take()
            .expect("live resource entry disappeared before finalization");
        self.retire_slot(slot_index);
        if let Some(pending) = entry.pending.take() {
            (pending.detach)();
        }
        (entry.finalizer)(entry.payload);
    }
}

impl Drop for ResourceRegistry {
    fn drop(&mut self) {
        self.shutdown();
    }
}
