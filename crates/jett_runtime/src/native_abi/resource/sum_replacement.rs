//! Whole-binding replacement for two exact live direct-sum shells.
//! Plain replacement retains its original protocol in operations.rs.
use super::*;

struct PreparedSumReplacement {
    old: ResourceHandleId,
    replacement: ResourceHandleId,
    old_slot: u32,
    replacement_slot: u32,
    destination_frame: ResourceHandleId,
    output: ResourceHandleId,
}

enum SumRetirement {
    Retired(ResourceResult<()>),
    Refused(NativeResourceError),
}

impl NativeResourceState {
    pub(super) fn replace(
        &mut self,
        ordinary: &mut values::NativeValues,
        registry: &mut ResourceRegistry,
        operation: u32,
        frame: ResourceHandleId,
        old: ResourceHandleId,
        replacement: ResourceHandleId,
    ) -> ResourceResult<ResourceHandleId> {
        // Selecting a lane reads only the installed row. The original Plain lane
        // keeps its existing validation and error order, including wrong-purpose.
        let sum = match self.operation(operation) {
            Ok(NativeOperation::Replace { old, .. }) => self
                .layout
                .slots()
                .get(*old as usize)
                .and_then(|slot| self.layout.shapes().get(slot.shape() as usize))
                .is_some_and(|shape| {
                    matches!(
                        shape,
                        NativeShape::Optional { .. } | NativeShape::Result { .. }
                    )
                }),
            _ => false,
        };
        if !sum {
            return self.replace_plain(ordinary, registry, operation, frame, old, replacement);
        }
        let prepared =
            self.prepare_sum_replacement(ordinary, registry, operation, frame, old, replacement)?;
        match self.retire_replaced_sum(ordinary, registry, prepared.old) {
            SumRetirement::Refused(error) => return Err(error),
            SumRetirement::Retired(Err(error)) => {
                // Old cleanup has consumed its token/shell. Retire the separately
                // owned RHS without replacing the old failure with a later one.
                let _ = self.retire_replaced_sum(ordinary, registry, prepared.replacement);
                return Err(error);
            }
            SumRetirement::Retired(Ok(())) => {}
        }
        // This checked mover revalidates the actual source holder. If a late
        // recheck refuses, it retains RHS at that holder for terminal cleanup.
        self.move_carrier_reserved(
            registry,
            prepared.replacement,
            prepared.replacement_slot,
            prepared.old_slot,
            prepared.destination_frame,
            prepared.output,
        )
    }

    fn prepare_sum_replacement(
        &mut self,
        ordinary: &values::NativeValues,
        registry: &ResourceRegistry,
        operation: u32,
        frame: ResourceHandleId,
        old_handle: ResourceHandleId,
        replacement_handle: ResourceHandleId,
    ) -> ResourceResult<PreparedSumReplacement> {
        self.running(ordinary)?;
        self.operation_frame(operation, frame)?;
        let (old_slot, replacement_slot) = match self.operation(operation)? {
            NativeOperation::Replace {
                old, replacement, ..
            } => (*old, *replacement),
            _ => return Err(NativeResourceError::WrongOperation),
        };
        if old_handle == replacement_handle || old_slot == replacement_slot {
            return Err(NativeResourceError::WrongOperation);
        }
        let old_row = &self.layout.slots()[old_slot as usize];
        let replacement_row = &self.layout.slots()[replacement_slot as usize];
        if old_row.shape() != replacement_row.shape()
            || old_row.path() != replacement_row.path()
            || old_row.frame() != replacement_row.frame()
            || old_row.frame() != self.frame(frame)?.template
            || self.layout.frames()[old_row.frame() as usize].role() != NativeFrameRole::Scope
        {
            return Err(NativeResourceError::WrongOperation);
        }
        let old = self.replacement_sum(ordinary, registry, frame, old_handle, old_slot)?;
        let replacement = self.replacement_sum(
            ordinary,
            registry,
            frame,
            replacement_handle,
            replacement_slot,
        )?;
        if old.frame != frame || replacement.frame != frame {
            return Err(NativeResourceError::WrongFrame);
        }
        let destination_frame = old.frame;
        let old_companion = Self::replacement_companion(old);
        let replacement_companion = Self::replacement_companion(replacement);
        let occupied = Self::replacement_token(old).is_some()
            || Self::replacement_token(replacement).is_some();
        if occupied {
            self.runtime_purpose()?;
        }
        ordinary
            .preflight_resource_sum_companions(&self.layout, old_companion, replacement_companion)
            .map_err(ordinary_error)?;
        let destination = self
            .custody
            .holder(&self.frame(frame)?.token, &self.layout.slot(old_slot)?)?;
        let source = self.custody.holder(
            &self.frame(frame)?.token,
            &self.layout.slot(replacement_slot)?,
        )?;
        let (Some(NativeResourceEntry::Sum(old)), Some(NativeResourceEntry::Sum(replacement))) = (
            self.handles.get(&old_handle),
            self.handles.get(&replacement_handle),
        ) else {
            return Err(NativeResourceError::WrongFamily);
        };
        // Borrow tokens directly from the disjoint handle table; no owner is
        // copied into this ephemeral prepared transaction.
        self.custody.preflight_direct_sum_replacement(
            Self::replacement_token(old),
            Self::replacement_token(replacement),
            &destination,
            &source,
            registry,
        )?;
        let mut ids = self.reserve(1)?;
        Ok(PreparedSumReplacement {
            old: old_handle,
            replacement: replacement_handle,
            old_slot,
            replacement_slot,
            destination_frame,
            output: next_handle(&mut ids)?,
        })
    }

    fn replacement_sum(
        &self,
        ordinary: &values::NativeValues,
        registry: &ResourceRegistry,
        frame: ResourceHandleId,
        handle: ResourceHandleId,
        slot: u32,
    ) -> ResourceResult<&NativeResourceSum> {
        self.carrier(handle, slot, registry)?;
        self.carrier_frame_at(frame, handle, slot)?;
        self.sum_unborrowed(handle)?;
        let Some(NativeResourceEntry::Sum(sum)) = self.handles.get(&handle) else {
            return Err(NativeResourceError::WrongFamily);
        };
        let row = &self.layout.slots()[slot as usize];
        let valid = match (
            self.layout.shapes().get(row.shape() as usize),
            row.path(),
            &sum.payload,
        ) {
            (
                Some(NativeShape::Optional { child }),
                [NativePayloadStep::Some],
                NativeSumPayload::None | NativeSumPayload::Some(_),
            ) => matches!(
                self.layout.shapes().get(*child as usize),
                Some(NativeShape::Resource { .. })
            ),
            (
                Some(NativeShape::Result { ok, .. }),
                [NativePayloadStep::Ok],
                NativeSumPayload::Ok(_),
            ) => matches!(
                self.layout.shapes().get(*ok as usize),
                Some(NativeShape::Resource { .. })
            ),
            (
                Some(NativeShape::Result { ok, fail }),
                [NativePayloadStep::Ok],
                NativeSumPayload::Fail { shape, string },
            ) if shape == fail => {
                ordinary
                    .validate_resource_ordinary(*string, &self.layout, *shape)
                    .map_err(ordinary_error)?;
                matches!(
                    self.layout.shapes().get(*ok as usize),
                    Some(NativeShape::Resource { .. })
                )
            }
            _ => false,
        };
        if !valid {
            return Err(NativeResourceError::WrongFamily);
        }
        Ok(sum)
    }

    fn replacement_token(sum: &NativeResourceSum) -> Option<&OwnedResourceToken> {
        match &sum.payload {
            NativeSumPayload::Some(token) | NativeSumPayload::Ok(token) => Some(token),
            NativeSumPayload::None | NativeSumPayload::Fail { .. } => None,
        }
    }

    fn replacement_companion(sum: &NativeResourceSum) -> Option<(u32, u64)> {
        match &sum.payload {
            NativeSumPayload::Fail { shape, string } => Some((*shape, *string)),
            NativeSumPayload::None | NativeSumPayload::Some(_) | NativeSumPayload::Ok(_) => None,
        }
    }

    fn retire_replaced_sum(
        &mut self,
        ordinary: &mut values::NativeValues,
        registry: &mut ResourceRegistry,
        handle: ResourceHandleId,
    ) -> SumRetirement {
        let Some(NativeResourceEntry::Sum(mut sum)) = self.handles.remove(&handle) else {
            return SumRetirement::Refused(NativeResourceError::WrongFamily);
        };
        match &mut sum.payload {
            NativeSumPayload::None => SumRetirement::Retired(Ok(())),
            NativeSumPayload::Fail { shape, string } => {
                match ordinary.drop_resource_typed_companion(&self.layout, *shape, *string) {
                    Ok(()) => SumRetirement::Retired(Ok(())),
                    Err(error) => {
                        self.handles.insert(handle, NativeResourceEntry::Sum(sum));
                        ordinary.cleanup_failed = true;
                        SumRetirement::Refused(ordinary_error(error))
                    }
                }
            }
            NativeSumPayload::Some(token) | NativeSumPayload::Ok(token) => {
                let cleanup = self.custody.close(token, registry);
                if self.custody.validate_owned(token, registry).is_ok() {
                    self.handles.insert(handle, NativeResourceEntry::Sum(sum));
                    return SumRetirement::Refused(
                        self.cleanup(cleanup)
                            .err()
                            .unwrap_or(NativeResourceError::WrongOperation),
                    );
                }
                SumRetirement::Retired(self.cleanup(cleanup))
            }
        }
    }
}
