use super::*;

impl NativeResourceState {
    pub(super) fn root_frame(&self, attempt: ResourceHandleId) -> ResourceResult<ResourceHandleId> {
        let running = self
            .attempt
            .as_ref()
            .ok_or(NativeResourceError::InvalidEntry)?;
        if running.id != attempt || running.phase != AttemptPhase::Running {
            return Err(NativeResourceError::InvalidEntry);
        }
        Ok(running.root_frame)
    }
    pub(super) fn begin_operation_frame(
        &mut self,
        ordinary: &values::NativeValues,
        template: u32,
        parent: ResourceHandleId,
    ) -> ResourceResult<ResourceHandleId> {
        let attempt = self.running(ordinary)?.id;
        if self.active_frames.last() != Some(&parent) {
            return Err(NativeResourceError::WrongFrame);
        }
        let parent_template = self.frame(parent)?.template;
        let frame = self
            .layout
            .frames()
            .get(template as usize)
            .ok_or(NativeResourceError::WrongFrame)?;
        if frame.role() != NativeFrameRole::Operation {
            // A Source invocation/Return constructor must supply its separate checked handoff.
            return Err(NativeResourceError::UnsupportedSourceBoundary);
        }
        if !frame
            .parents()
            .contains(&NativeParent::Frame(parent_template))
        {
            return Err(NativeResourceError::WrongFrame);
        }
        let mut ids = self.reserve(1)?;
        self.active_frames
            .try_reserve(1)
            .map_err(|_| NativeResourceError::Capacity)?;
        let handle = next_handle(&mut ids)?;
        let purpose = self
            .attempt
            .as_ref()
            .ok_or(NativeResourceError::InvalidEntry)?
            .purpose;
        let token = self
            .custody
            .begin_frame(ResourceFrameKind::Operation, purpose)?;
        self.handles.insert(
            handle,
            NativeResourceEntry::Frame(NativeFrameEntry {
                template,
                attempt,
                parent: Some(parent),
                token,
                incoming: Vec::new(),
            }),
        );
        self.active_frames.push(handle);
        Ok(handle)
    }
    pub(super) fn end_operation_frame(
        &mut self,
        ordinary: &mut values::NativeValues,
        registry: &mut ResourceRegistry,
        frame: ResourceHandleId,
    ) -> ResourceResult<()> {
        if self.active_frames.last() != Some(&frame) {
            return Err(NativeResourceError::WrongFrame);
        }
        if self.layout.frames()[self.frame(frame)?.template as usize].role()
            != NativeFrameRole::Operation
        {
            return Err(NativeResourceError::WrongFrame);
        }
        self.retire_frame(ordinary, registry, frame)
    }
    pub(super) fn retire_frame(
        &mut self,
        ordinary: &mut values::NativeValues,
        registry: &mut ResourceRegistry,
        frame: ResourceHandleId,
    ) -> ResourceResult<()> {
        if self.active_frames.last() != Some(&frame) {
            return Err(NativeResourceError::WrongFrame);
        }
        let Some(NativeResourceEntry::Frame(header)) = self.handles.get(&frame) else {
            return Err(NativeResourceError::WrongFrame);
        };
        let cleanup = self.custody.end_frame(&header.token, registry);
        if self.custody.active_frames() == self.active_frames.len() {
            // A refused cleanup has not retired this frame. Keep its tokens available.
            return self.cleanup(cleanup);
        }
        let mut ordinary_failed = false;
        self.handles.retain(|_, entry| match entry {
            NativeResourceEntry::Frame(_) => true,
            NativeResourceEntry::Owner(owner) => owner.frame != frame,
            NativeResourceEntry::Loan(loan) => loan.frame != frame,
            NativeResourceEntry::Prepared(prepared) => prepared.frame != frame,
            NativeResourceEntry::Call(call) => call.frame != frame,
            NativeResourceEntry::SourceCall(call) => call.frame != frame,
            NativeResourceEntry::Sum(sum) if sum.frame == frame => {
                if let NativeSumPayload::Fail { shape, string } = &sum.payload {
                    ordinary_failed |= ordinary
                        .drop_resource_typed_companion(&self.layout, *shape, *string)
                        .is_err();
                }
                false
            }
            _ => true,
        });
        self.handles.remove(&frame);
        self.active_frames.pop();
        if ordinary_failed {
            ordinary.cleanup_failed = true;
        }
        self.cleanup(cleanup)
    }
    pub(super) fn runtime_purpose(&self) -> ResourceResult<()> {
        if self.attempt.as_ref().map(|attempt| attempt.purpose) != Some(ResourcePurpose::Runtime) {
            return Err(NativeResourceError::WrongPurpose);
        }
        Ok(())
    }
    pub(super) fn owner(
        &self,
        handle: ResourceHandleId,
        slot: u32,
    ) -> ResourceResult<&NativeOwnerEntry> {
        match self.handles.get(&handle) {
            Some(NativeResourceEntry::Owner(owner)) if owner.slot == slot => {
                self.frame(owner.frame)?;
                let current = *self
                    .active_frames
                    .last()
                    .ok_or(NativeResourceError::WrongFrame)?;
                self.carrier_frame_at(current, handle, slot)?;
                Ok(owner)
            }
            Some(NativeResourceEntry::Owner(_)) => Err(NativeResourceError::WrongOperation),
            Some(_) => Err(NativeResourceError::WrongFamily),
            None => Err(NativeResourceError::InvalidHandle),
        }
    }
    pub(super) fn slot_frame(&self, slot: u32, frame: ResourceHandleId) -> ResourceResult<()> {
        let layout_slot = self
            .layout
            .slots()
            .get(slot as usize)
            .ok_or(NativeResourceError::WrongOperation)?;
        if layout_slot.frame() != self.frame(frame)?.template {
            return Err(NativeResourceError::WrongFrame);
        }
        Ok(())
    }
    fn grant(
        &self,
        ordinary: &values::NativeValues,
        network_bits: u64,
    ) -> ResourceResult<(ResourceHandleId, AuthorityProvenance)> {
        let handle = self.network.ok_or(NativeResourceError::WrongGrant)?;
        let Some(NativeResourceEntry::NetworkGrant(grant)) = self.handles.get(&handle) else {
            return Err(NativeResourceError::WrongGrant);
        };
        if grant.context != self.context
            || !Arc::ptr_eq(&grant.layout, &self.layout)
            || !self.provider.matches(grant.provider, handle)
            || grant.ordinary_network != network_bits
            || !ordinary.validates_resource_network(network_bits)
            || grant.provenance
                != AuthorityProvenance::new(grant.provider.raw(), grant.restriction.raw())
        {
            return Err(NativeResourceError::WrongGrant);
        }
        Ok((handle, grant.provenance))
    }
    pub(super) fn descriptor(
        &mut self,
        ordinary: &values::NativeValues,
        operation: u32,
    ) -> ResourceResult<ResourceHandleId> {
        self.running(ordinary)?;
        let hook = match self.operation(operation)? {
            NativeOperation::Descriptor { hook } => *hook,
            _ => return Err(NativeResourceError::WrongOperation),
        };
        let signature = self
            .layout
            .hooks()
            .get(hook as usize)
            .ok_or(NativeResourceError::WrongOperation)?
            .signature();
        let mut ids = self.reserve(1)?;
        let handle = next_handle(&mut ids)?;
        self.handles
            .insert(handle, NativeResourceEntry::Descriptor { hook, signature });
        Ok(handle)
    }
    pub(super) fn prepare_hook(
        &mut self,
        ordinary: &values::NativeValues,
        operation: u32,
        frame: ResourceHandleId,
        descriptor: Option<ResourceHandleId>,
    ) -> ResourceResult<ResourceHandleId> {
        self.running(ordinary)?;
        self.runtime_purpose()?;
        let (target, hook) = match (self.operation(operation)?, descriptor) {
            (
                NativeOperation::Acquire { hook, .. }
                | NativeOperation::InvokeBorrow { hook, .. }
                | NativeOperation::Close { hook, .. },
                None,
            ) => (operation, *hook),
            (
                NativeOperation::InvokeDescriptor {
                    hook,
                    signature,
                    target,
                },
                Some(handle),
            ) => {
                match self.handles.get(&handle) {
                    Some(NativeResourceEntry::Descriptor {
                        hook: actual_hook,
                        signature: actual_signature,
                    }) if actual_hook == hook && actual_signature == signature => {}
                    _ => return Err(NativeResourceError::WrongFamily),
                }
                (*target, *hook)
            }
            _ => return Err(NativeResourceError::WrongOperation),
        };
        self.operation_frame(target, frame)?;
        let mut ids = self.reserve(1)?;
        let handle = next_handle(&mut ids)?;
        self.handles.insert(
            handle,
            NativeResourceEntry::Call(NativePreparedCall {
                operation,
                target_operation: target,
                hook,
                frame,
                descriptor,
            }),
        );
        Ok(handle)
    }
    pub(super) fn checked_call(
        &self,
        handle: ResourceHandleId,
    ) -> ResourceResult<&NativePreparedCall> {
        let Some(NativeResourceEntry::Call(call)) = self.handles.get(&handle) else {
            return Err(NativeResourceError::WrongFamily);
        };
        self.operation_frame(call.target_operation, call.frame)?;
        match (self.operation(call.operation)?, call.descriptor) {
            (
                NativeOperation::InvokeDescriptor {
                    hook,
                    signature,
                    target,
                },
                Some(descriptor),
            ) if *target == call.target_operation && *hook == call.hook => {
                match self.handles.get(&descriptor) {
                    Some(NativeResourceEntry::Descriptor {
                        hook: actual,
                        signature: actual_signature,
                    }) if actual == hook && actual_signature == signature => {}
                    _ => return Err(NativeResourceError::WrongFamily),
                }
            }
            (
                NativeOperation::Acquire { hook, .. }
                | NativeOperation::InvokeBorrow { hook, .. }
                | NativeOperation::Close { hook, .. },
                None,
            ) if *hook == call.hook && call.operation == call.target_operation => {}
            _ => return Err(NativeResourceError::WrongOperation),
        }
        Ok(call)
    }
    pub(super) fn prepare_factory(
        &mut self,
        ordinary: &mut values::NativeValues,
        registry: &mut ResourceRegistry,
        call_handle: ResourceHandleId,
        network_bits: u64,
    ) -> ResourceResult<ResourceHandleId> {
        self.running(ordinary)?;
        self.runtime_purpose()?;
        let call = self.checked_call(call_handle)?;
        let frame = call.frame;
        let hook = call.hook;
        let destination = match self.operation(call.target_operation)? {
            NativeOperation::Acquire { destination, .. } => *destination,
            _ => return Err(NativeResourceError::WrongOperation),
        };
        let hook = self
            .layout
            .hooks()
            .get(hook as usize)
            .ok_or(NativeResourceError::WrongOperation)?;
        if hook.recipe() != NativeRecipe::NetworkFactory {
            return Err(NativeResourceError::WrongOperation);
        }
        let kind = hook.kind();
        let destination_frame =
            self.destination_frame(frame, self.checked_call(call_handle)?.target_operation)?;
        self.slot_empty(destination_frame, destination)?;
        let slot = &self.layout.slots()[destination as usize];
        if slot.shape() != self.layout.signatures()[hook.signature() as usize].result()
            || slot.path() != [NativePayloadStep::Ok]
        {
            return Err(NativeResourceError::WrongOperation);
        }
        let network = self.grant(ordinary, network_bits)?.0;
        self.layout.preflight_physical_acquisition(registry)?;
        // Both Resource result branches and ordinary error storage are reserved before the provider.
        let publication_ids = self.reserve(1)?;
        let mut prepared_ids = self.reserve(1)?;
        let handle = next_handle(&mut prepared_ids)?;
        ordinary
            .prepare_resource_ordinary_output()
            .map_err(ordinary_error)?; // capacity only; unused identity range is burned
        let frame_token = match self.handles.get(&destination_frame) {
            Some(NativeResourceEntry::Frame(frame)) => &frame.token,
            _ => return Err(NativeResourceError::WrongFrame),
        };
        let holder = self
            .custody
            .holder(frame_token, &self.layout.slot(destination)?)?;
        let token = self
            .custody
            .prepare_acquisition(&self.layout.kind(kind)?, &holder)?;
        self.handles.insert(
            handle,
            NativeResourceEntry::Prepared(NativePreparedEntry {
                call: call_handle,
                frame,
                destination,
                destination_frame,
                kind,
                network,
                token,
                publication_ids,
            }),
        );
        Ok(handle)
    }
    pub(super) fn factory(
        &mut self,
        ordinary: &mut values::NativeValues,
        registry: &mut ResourceRegistry,
        prepared_handle: ResourceHandleId,
        network_bits: u64,
        label: i64,
    ) -> ResourceResult<JettResourceCallResultV1> {
        self.running(ordinary)?;
        self.runtime_purpose()?;
        let grant = self.grant(ordinary, network_bits)?;
        let Some(NativeResourceEntry::Prepared(prepared)) = self.handles.get(&prepared_handle)
        else {
            return Err(NativeResourceError::WrongFamily);
        };
        let call = self.checked_call(prepared.call)?;
        let actual_destination = match self.operation(call.target_operation)? {
            NativeOperation::Acquire { destination, .. } => *destination,
            _ => return Err(NativeResourceError::WrongOperation),
        };
        if prepared.network != grant.0
            || prepared.frame != call.frame
            || prepared.destination != actual_destination
            || self.layout.hooks()[call.hook as usize].kind() != prepared.kind
        {
            return Err(NativeResourceError::WrongOperation);
        }
        self.layout.preflight_physical_acquisition(registry)?;
        let mut ordinary_ids = ordinary
            .prepare_resource_ordinary_output()
            .map_err(ordinary_error)?;
        // Rejoin occupancy immediately before the effect; this is not token reconstruction.
        self.slot_frame(prepared.destination, prepared.destination_frame)?;
        let destination_frame = self.destination_frame(call.frame, call.target_operation)?;
        if destination_frame != prepared.destination_frame {
            return Err(NativeResourceError::WrongFrame);
        }
        if self.handles.values().any(|entry| matches!(entry, NativeResourceEntry::Owner(owner)
            if owner.frame == destination_frame && owner.slot == prepared.destination)
            || matches!(entry, NativeResourceEntry::Sum(sum) if sum.frame == destination_frame && sum.slot == prepared.destination)) {
            return Err(NativeResourceError::WrongOperation);
        }
        let frame = match self.handles.get(&prepared.destination_frame) {
            Some(NativeResourceEntry::Frame(frame)) => &frame.token,
            _ => return Err(NativeResourceError::WrongFrame),
        };
        let holder = self
            .custody
            .holder(frame, &self.layout.slot(prepared.destination)?)?;
        self.custody
            .prepare_acquisition(&self.layout.kind(prepared.kind)?, &holder)?;
        let result = catch_unwind(AssertUnwindSafe(|| self.provider.construct(label)));
        let result = match result {
            Ok(Ok(result)) => result,
            Ok(Err(error)) => {
                self.record_failure(error);
                return Err(error);
            }
            Err(panic) => {
                discard_panic_payload(panic);
                if let Some(attempt) = &mut self.attempt {
                    attempt.body.get_or_insert(NativeBodyFailure::HostPanic);
                }
                return Err(NativeResourceError::BodyFailed);
            }
        };
        let Some(NativeResourceEntry::Prepared(mut prepared)) =
            self.handles.remove(&prepared_handle)
        else {
            return Err(NativeResourceError::WrongFamily);
        };
        self.handles.remove(&prepared.call);
        let handle = next_handle(&mut prepared.publication_ids)?;
        let shape = self.layout.slots()[prepared.destination as usize].shape();
        let (domain, payload) = match result {
            Ok(constructed) => {
                let provider::ConstructedPayload { payload, finalizer } = constructed;
                let token = match self.custody.commit_acquisition(
                    registry,
                    prepared.token,
                    grant.1,
                    payload,
                    finalizer,
                ) {
                    Ok(token) => token,
                    Err(ResourceTransitionFailure::Rejected(error)) => {
                        let error = NativeResourceError::Custody(error);
                        self.record_failure(error);
                        return Err(error);
                    }
                    Err(ResourceTransitionFailure::Cleanup(error)) => {
                        let error = NativeResourceError::Cleanup(error);
                        self.record_failure(error);
                        return Err(error);
                    }
                };
                (RESOURCE_DOMAIN_OK, NativeSumPayload::Ok(token))
            }
            Err(error) => {
                let fail_shape = match self.layout.shapes().get(shape as usize) {
                    Some(NativeShape::Result { fail, .. }) => *fail,
                    _ => return Err(NativeResourceError::WrongOperation),
                };
                let string = ordinary
                    .publish_resource_error(error, &mut ordinary_ids)
                    .map_err(ordinary_error)?;
                (
                    RESOURCE_DOMAIN_ERROR,
                    NativeSumPayload::Fail {
                        shape: fail_shape,
                        string,
                    },
                )
            }
        };
        self.handles.insert(
            handle,
            NativeResourceEntry::Sum(NativeResourceSum {
                frame: prepared.destination_frame,
                slot: prepared.destination,
                shape,
                payload,
            }),
        );
        Ok(JettResourceCallResultV1 {
            domain,
            reserved: 0,
            value: handle.raw(),
        })
    }
    pub(super) fn sum_tag(&self, handle: ResourceHandleId) -> ResourceResult<u32> {
        match self.handles.get(&handle) {
            Some(NativeResourceEntry::Sum(sum)) => {
                self.frame(sum.frame)?;
                let current = *self
                    .active_frames
                    .last()
                    .ok_or(NativeResourceError::WrongFrame)?;
                self.carrier_frame_at(current, handle, sum.slot)?;
                Ok(match &sum.payload {
                    NativeSumPayload::None | NativeSumPayload::Fail { .. } => 0,
                    _ => 1,
                })
            }
            Some(_) => Err(NativeResourceError::WrongFamily),
            None => Err(NativeResourceError::InvalidHandle),
        }
    }
    pub(super) fn sum_adopt(
        &mut self,
        ordinary: &values::NativeValues,
        registry: &ResourceRegistry,
        operation: u32,
        frame: ResourceHandleId,
        owner_handle: ResourceHandleId,
        destination_frame: ResourceHandleId,
    ) -> ResourceResult<ResourceHandleId> {
        self.running(ordinary)?;
        self.operation_frame(operation, frame)?;
        let (source, destination) = match self.operation(operation)? {
            NativeOperation::SumAdopt {
                source,
                destination,
                ..
            } => (*source, *destination),
            _ => return Err(NativeResourceError::WrongOperation),
        };
        self.owner(owner_handle, source)?;
        self.slot_empty(destination_frame, destination)?;
        let slot = &self.layout.slots()[destination as usize];
        let shape = slot.shape();
        let step = match slot.path() {
            [NativePayloadStep::Some] => NativePayloadStep::Some,
            [NativePayloadStep::Ok] => NativePayloadStep::Ok,
            _ => return Err(NativeResourceError::WrongOperation),
        };
        let mut ids = self.reserve(1)?;
        let handle = next_handle(&mut ids)?;
        let holder = self.custody.holder(
            &self.frame(destination_frame)?.token,
            &self.layout.slot(destination)?,
        )?;
        let Some(NativeResourceEntry::Owner(mut owner)) = self.handles.remove(&owner_handle) else {
            return Err(NativeResourceError::WrongFamily);
        };
        if let Err(error) = self.custody.transfer(&mut owner.token, &holder, registry) {
            self.handles
                .insert(owner_handle, NativeResourceEntry::Owner(owner));
            return Err(error.into());
        }
        let payload = if step == NativePayloadStep::Some {
            NativeSumPayload::Some(owner.token)
        } else {
            NativeSumPayload::Ok(owner.token)
        };
        self.handles.insert(
            handle,
            NativeResourceEntry::Sum(NativeResourceSum {
                frame: destination_frame,
                slot: destination,
                shape,
                payload,
            }),
        );
        Ok(handle)
    }
    pub(super) fn replace(
        &mut self,
        ordinary: &values::NativeValues,
        registry: &mut ResourceRegistry,
        operation: u32,
        frame: ResourceHandleId,
        old_handle: ResourceHandleId,
        replacement_handle: ResourceHandleId,
    ) -> ResourceResult<ResourceHandleId> {
        self.running(ordinary)?;
        self.runtime_purpose()?;
        self.operation_frame(operation, frame)?;
        let (old_slot, replacement_slot) = match self.operation(operation)? {
            NativeOperation::Replace {
                old, replacement, ..
            } => (*old, *replacement),
            _ => return Err(NativeResourceError::WrongOperation),
        };
        if old_handle == replacement_handle {
            return Err(NativeResourceError::WrongOperation);
        }
        let old = self.owner(old_handle, old_slot)?;
        let old_frame = old.frame;
        self.custody.validate_owned(&old.token, registry)?;
        let replacement = self.owner(replacement_handle, replacement_slot)?;
        self.custody.validate_owned(&replacement.token, registry)?;
        let mut ids = self.reserve(1)?;
        let handle = next_handle(&mut ids)?;
        let holder = self
            .custody
            .holder(&self.frame(old_frame)?.token, &self.layout.slot(old_slot)?)?;
        let Some(NativeResourceEntry::Owner(mut old)) = self.handles.remove(&old_handle) else {
            return Err(NativeResourceError::WrongFamily);
        };
        let cleanup = self.custody.close(&mut old.token, registry);
        if self.custody.validate_owned(&old.token, registry).is_ok() {
            self.handles
                .insert(old_handle, NativeResourceEntry::Owner(old));
            // The RHS still has its one independent owner, available for exact scope cleanup.
            return self
                .cleanup(cleanup)
                .and(Err(NativeResourceError::WrongOperation));
        }
        let Some(NativeResourceEntry::Owner(mut replacement)) =
            self.handles.remove(&replacement_handle)
        else {
            return Err(NativeResourceError::WrongFamily);
        };
        if cleanup.failure().is_some() {
            let replacement_cleanup = self.custody.close(&mut replacement.token, registry);
            if self
                .custody
                .validate_owned(&replacement.token, registry)
                .is_ok()
            {
                self.handles
                    .insert(replacement_handle, NativeResourceEntry::Owner(replacement));
            }
            self.cleanup(cleanup)?;
            return self
                .cleanup(replacement_cleanup)
                .and(Err(NativeResourceError::BodyFailed));
        }
        if let Err(error) = self
            .custody
            .transfer(&mut replacement.token, &holder, registry)
        {
            self.handles
                .insert(replacement_handle, NativeResourceEntry::Owner(replacement));
            return Err(error.into());
        }
        replacement.frame = old_frame;
        replacement.slot = old_slot;
        self.handles
            .insert(handle, NativeResourceEntry::Owner(replacement));
        Ok(handle)
    }
    pub(super) fn sum_take(
        &mut self,
        ordinary: &values::NativeValues,
        registry: &ResourceRegistry,
        operation: u32,
        frame: ResourceHandleId,
        sum_handle: ResourceHandleId,
        destination_frame: ResourceHandleId,
    ) -> ResourceResult<ResourceHandleId> {
        self.running(ordinary)?;
        self.operation_frame(operation, frame)?;
        let (source, destination) = match self.operation(operation)? {
            NativeOperation::SumTake {
                source,
                destination,
                ..
            } => (*source, *destination),
            _ => return Err(NativeResourceError::WrongOperation),
        };
        self.slot_empty(destination_frame, destination)?;
        let Some(NativeResourceEntry::Sum(sum)) = self.handles.get(&sum_handle) else {
            return Err(NativeResourceError::WrongFamily);
        };
        if sum.slot != source {
            return Err(NativeResourceError::WrongOperation);
        }
        self.carrier_frame_at(frame, sum_handle, source)?;
        let token = match &sum.payload {
            NativeSumPayload::Some(token) | NativeSumPayload::Ok(token) => token,
            _ => return Err(NativeResourceError::WrongOperation),
        };
        self.custody.validate_owned(token, registry)?;
        let mut ids = self.reserve(1)?;
        let new_handle = next_handle(&mut ids)?;
        let holder = self.custody.holder(
            &self.frame(destination_frame)?.token,
            &self.layout.slot(destination)?,
        )?;
        let Some(NativeResourceEntry::Sum(mut sum)) = self.handles.remove(&sum_handle) else {
            return Err(NativeResourceError::WrongFamily);
        };
        let token = match &mut sum.payload {
            NativeSumPayload::Some(token) | NativeSumPayload::Ok(token) => token,
            _ => {
                self.handles
                    .insert(sum_handle, NativeResourceEntry::Sum(sum));
                return Err(NativeResourceError::WrongOperation);
            }
        };
        if let Err(error) = self.custody.transfer(token, &holder, registry) {
            self.handles
                .insert(sum_handle, NativeResourceEntry::Sum(sum));
            return Err(error.into());
        }
        let token = match sum.payload {
            NativeSumPayload::Some(token) | NativeSumPayload::Ok(token) => token,
            _ => return Err(NativeResourceError::WrongOperation),
        };
        self.handles.insert(
            new_handle,
            NativeResourceEntry::Owner(NativeOwnerEntry {
                frame: destination_frame,
                slot: destination,
                token,
            }),
        );
        Ok(new_handle)
    }
    pub(super) fn transfer(
        &mut self,
        ordinary: &values::NativeValues,
        registry: &ResourceRegistry,
        operation: u32,
        frame: ResourceHandleId,
        owner_handle: ResourceHandleId,
        destination_frame: ResourceHandleId,
    ) -> ResourceResult<ResourceHandleId> {
        self.running(ordinary)?;
        self.operation_frame(operation, frame)?;
        let (source, destination) = match self.operation(operation)? {
            NativeOperation::Transfer {
                source,
                destination,
                ..
            } => (*source, *destination),
            _ => return Err(NativeResourceError::WrongOperation),
        };
        self.carrier_frame_at(frame, owner_handle, source)?;
        self.move_carrier(
            registry,
            owner_handle,
            source,
            destination,
            destination_frame,
        )
    }

    pub(super) fn borrow_begin(
        &mut self,
        ordinary: &values::NativeValues,
        registry: &ResourceRegistry,
        operation: u32,
        frame: ResourceHandleId,
        owner_handle: ResourceHandleId,
    ) -> ResourceResult<ResourceHandleId> {
        self.running(ordinary)?;
        self.runtime_purpose()?;
        self.operation_frame(operation, frame)?;
        let (source, lease_frame) = match self.operation(operation)? {
            NativeOperation::Borrow {
                source,
                lease_frame,
                ..
            } => (*source, *lease_frame),
            _ => return Err(NativeResourceError::WrongOperation),
        };
        if self.frame(frame)?.template != lease_frame {
            return Err(NativeResourceError::WrongFrame);
        }
        self.owner(owner_handle, source)?;
        let mut ids = self.reserve(1)?;
        let handle = next_handle(&mut ids)?;
        let Some(NativeResourceEntry::Owner(owner)) = self.handles.get(&owner_handle) else {
            return Err(NativeResourceError::WrongFamily);
        };
        let source_frame = owner.frame;
        let Some(NativeResourceEntry::Frame(frame_header)) = self.handles.get(&frame) else {
            return Err(NativeResourceError::WrongFrame);
        };
        let token = self
            .custody
            .begin_borrow(&owner.token, &frame_header.token, registry)?;
        self.handles.insert(
            handle,
            NativeResourceEntry::Loan(NativeLoanEntry {
                frame,
                borrow_operation: operation,
                source_slot: source,
                source_frame,
                token,
            }),
        );
        Ok(handle)
    }
    pub(super) fn exact_loan(
        &self,
        handle: ResourceHandleId,
        source: NativeLoanSource,
    ) -> ResourceResult<&NativeLoanEntry> {
        let Some(NativeResourceEntry::Loan(loan)) = self.handles.get(&handle) else {
            return Err(NativeResourceError::WrongFamily);
        };
        match source {
            NativeLoanSource::ExistingBorrow { operation }
                if operation == loan.borrow_operation =>
            {
                let current = *self
                    .active_frames
                    .last()
                    .ok_or(NativeResourceError::WrongFrame)?;
                if self.activation_frame(current, self.frame(loan.frame)?.template)? != loan.frame {
                    return Err(NativeResourceError::WrongFrame);
                }
            }
            NativeLoanSource::IncomingViewFormal { scope, parameter } => {
                self.validate_resident(handle, scope, parameter)?;
            }
            _ => return Err(NativeResourceError::WrongOperation),
        }
        self.frame(loan.frame)?;
        self.frame(loan.source_frame)?;
        Ok(loan)
    }
    pub(super) fn invoke_borrow(
        &mut self,
        ordinary: &mut values::NativeValues,
        registry: &mut ResourceRegistry,
        call_handle: ResourceHandleId,
        network_bits: u64,
        loan_handle: ResourceHandleId,
    ) -> ResourceResult<u64> {
        self.running(ordinary)?;
        self.runtime_purpose()?;
        let call = self.checked_call(call_handle)?;
        let (hook, source) = match self.operation(call.target_operation)? {
            NativeOperation::InvokeBorrow { hook, source, .. } => (*hook, *source),
            _ => return Err(NativeResourceError::WrongOperation),
        };
        let hook = self
            .layout
            .hooks()
            .get(hook as usize)
            .ok_or(NativeResourceError::WrongOperation)?;
        if hook.recipe() != NativeRecipe::NetworkBorrow {
            return Err(NativeResourceError::WrongOperation);
        }
        let kind = hook.kind();
        let loan = self.exact_loan(loan_handle, source)?;
        if self.layout.slot_kind(loan.source_slot)? != kind {
            return Err(NativeResourceError::WrongOperation);
        }
        let key = self.custody.validate_borrowed(&loan.token, registry)?;
        let provenance = self.grant(ordinary, network_bits)?.1;
        let kind_id = self.layout.kind_id(kind)?;
        let mut output_ids = ordinary
            .prepare_resource_ordinary_output()
            .map_err(ordinary_error)?;
        let result = catch_unwind(AssertUnwindSafe(|| {
            registry.access::<provider::NativePayload, _, _>(key, kind_id, &provenance, |payload| {
                self.provider.borrow(payload)
            })
        }));
        self.handles.remove(&call_handle);
        let result = match result {
            Ok(Ok(Ok(result))) => result,
            Ok(Ok(Err(error))) => {
                self.record_failure(error);
                return Err(error);
            }
            Ok(Err(error)) => {
                let error = NativeResourceError::Registry(error);
                self.record_failure(error);
                return Err(error);
            }
            Err(panic) => {
                discard_panic_payload(panic);
                if let Some(attempt) = &mut self.attempt {
                    attempt.body.get_or_insert(NativeBodyFailure::HostPanic);
                }
                return Err(NativeResourceError::BodyFailed);
            }
        };
        ordinary
            .publish_resource_borrow_result(result, &mut output_ids)
            .map_err(ordinary_error)
    }
    pub(super) fn borrow_end(
        &mut self,
        operation: u32,
        frame: ResourceHandleId,
        handle: ResourceHandleId,
    ) -> ResourceResult<()> {
        self.operation_frame(operation, frame)?;
        let borrow = match self.operation(operation)? {
            NativeOperation::EndBorrow { borrow, .. } => *borrow,
            _ => return Err(NativeResourceError::WrongOperation),
        };
        self.exact_loan(
            handle,
            NativeLoanSource::ExistingBorrow { operation: borrow },
        )?;
        let Some(NativeResourceEntry::Loan(mut loan)) = self.handles.remove(&handle) else {
            return Err(NativeResourceError::WrongFamily);
        };
        if let Err(error) = self.custody.end_borrow(&mut loan.token) {
            self.handles.insert(handle, NativeResourceEntry::Loan(loan));
            return Err(error.into());
        }
        Ok(())
    }
    pub(super) fn close_or_drop(
        &mut self,
        registry: &mut ResourceRegistry,
        operation: u32,
        frame: ResourceHandleId,
        owner_handle: ResourceHandleId,
    ) -> ResourceResult<()> {
        self.runtime_purpose()?;
        self.operation_frame(operation, frame)?;
        let source = match self.operation(operation)? {
            NativeOperation::Close { hook, source, .. }
                if self
                    .layout
                    .hooks()
                    .get(*hook as usize)
                    .is_some_and(|hook| hook.recipe() == NativeRecipe::Finalize) =>
            {
                *source
            }
            NativeOperation::Drop { source, .. } => *source,
            _ => return Err(NativeResourceError::WrongOperation),
        };
        self.owner(owner_handle, source)?;
        let Some(NativeResourceEntry::Owner(mut owner)) = self.handles.remove(&owner_handle) else {
            return Err(NativeResourceError::WrongFamily);
        };
        let cleanup = self.custody.close(&mut owner.token, registry);
        if self.custody.validate_owned(&owner.token, registry).is_ok() {
            self.handles
                .insert(owner_handle, NativeResourceEntry::Owner(owner));
        }
        self.cleanup(cleanup)
    }
    pub(super) fn sum_drop(
        &mut self,
        ordinary: &mut values::NativeValues,
        registry: &mut ResourceRegistry,
        operation: u32,
        frame: ResourceHandleId,
        handle: ResourceHandleId,
    ) -> ResourceResult<()> {
        self.operation_frame(operation, frame)?;
        let source = match self.operation(operation)? {
            NativeOperation::SumDrop { source, .. } => *source,
            _ => return Err(NativeResourceError::WrongOperation),
        };
        match self.handles.get(&handle) {
            Some(NativeResourceEntry::Sum(sum)) if sum.slot == source => {}
            _ => return Err(NativeResourceError::WrongOperation),
        }
        self.carrier_frame_at(frame, handle, source)?;
        let Some(NativeResourceEntry::Sum(mut sum)) = self.handles.remove(&handle) else {
            return Err(NativeResourceError::WrongFamily);
        };
        let cleanup = match &mut sum.payload {
            NativeSumPayload::Ok(token) | NativeSumPayload::Some(token) => {
                self.custody.close(token, registry)
            }
            NativeSumPayload::Fail { shape, string } => {
                if let Err(error) =
                    ordinary.drop_resource_typed_companion(&self.layout, *shape, *string)
                {
                    self.handles.insert(handle, NativeResourceEntry::Sum(sum));
                    ordinary.cleanup_failed = true;
                    return Err(ordinary_error(error));
                }
                ResourceCleanupOutcome::default()
            }
            NativeSumPayload::None => ResourceCleanupOutcome::default(),
        };
        if let NativeSumPayload::Ok(token) | NativeSumPayload::Some(token) = &sum.payload {
            if self.custody.validate_owned(token, registry).is_ok() {
                self.handles.insert(handle, NativeResourceEntry::Sum(sum));
            }
        }
        self.cleanup(cleanup)
    }
}
