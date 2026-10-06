//! Active Source handoffs. Immutable rows describe joins; only live core tokens own values.
use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SourcePhase {
    Prepared,
    Entered,
    ScopeCompleted,
    Completed,
    Aborted,
}
#[derive(Clone, Copy)]
struct SourceCompletion {
    scope: ResourceHandleId,
    return_frame: Option<ResourceHandleId>,
    original_body_status: u32,
    failure: Option<NativeResourceError>,
}

pub(super) struct NativeSourceCall {
    pub(super) frame: ResourceHandleId,
    operation: u32,
    actuals: Vec<Option<u64>>,
    parameters: Vec<u64>,
    next_source: usize,
    scope: Option<ResourceHandleId>,
    return_frame: Option<ResourceHandleId>,
    destination: Option<ResourceHandleId>,
    body_status: u32,
    phase: SourcePhase,
    completion: Option<SourceCompletion>,
}

impl NativeResourceState {
    fn source_call(&self, handle: ResourceHandleId) -> ResourceResult<&NativeSourceCall> {
        match self.handles.get(&handle) {
            Some(NativeResourceEntry::SourceCall(call)) => Ok(call),
            _ => Err(NativeResourceError::WrongFamily),
        }
    }
    fn source_row(
        &self,
        operation: u32,
    ) -> ResourceResult<(u32, u32, u32, NativeSourceInvocation)> {
        match self.operation(operation)? {
            NativeOperation::InvokeSourceFunction {
                callee,
                signature,
                callee_scope,
                source: Some(source),
                ..
            } if self.layout.wire_version() == 2 => {
                Ok((*callee, *signature, *callee_scope, source.clone()))
            }
            _ => Err(NativeResourceError::UnsupportedSourceBoundary),
        }
    }
    fn call_for_scope(&self, scope: ResourceHandleId) -> ResourceResult<ResourceHandleId> {
        let mut found = None;
        for (&id, entry) in &self.handles {
            if let NativeResourceEntry::SourceCall(call) = entry {
                if call.scope == Some(scope)
                    && matches!(
                        call.phase,
                        SourcePhase::Entered | SourcePhase::ScopeCompleted
                    )
                {
                    if found.replace(id).is_some() {
                        return Err(NativeResourceError::WrongFrame);
                    }
                }
            }
        }
        found.ok_or(NativeResourceError::WrongFrame)
    }
    /// Resolve within this activation only. A recursive caller with the same
    /// template/function is separated by the current Scope boundary.
    pub(super) fn activation_frame(
        &self,
        start: ResourceHandleId,
        template: u32,
    ) -> ResourceResult<ResourceHandleId> {
        let function = self.layout.frames()[self.frame(start)?.template as usize]
            .site()
            .function();
        let mut current = start;
        loop {
            let header = self.frame(current)?;
            let row = &self.layout.frames()[header.template as usize];
            if row.site().function() != function {
                return Err(NativeResourceError::WrongFrame);
            }
            if header.template == template {
                return Ok(current);
            }
            let parent = header.parent.ok_or(NativeResourceError::WrongFrame)?;
            if row.role() == NativeFrameRole::Scope {
                let parent_header = self.frame(parent)?;
                let parent_row = &self.layout.frames()[parent_header.template as usize];
                if parent_row.role() == NativeFrameRole::Return
                    && parent_row.site().function() == function
                    && parent_header.template == template
                {
                    return Ok(parent);
                }
                return Err(NativeResourceError::WrongFrame);
            }
            current = parent;
        }
    }
    pub(super) fn validate_resident(
        &self,
        loan: ResourceHandleId,
        scope: u32,
        parameter: u32,
    ) -> ResourceResult<()> {
        let start = *self
            .active_frames
            .last()
            .ok_or(NativeResourceError::WrongFrame)?;
        let scope_handle = self.activation_frame(start, scope)?;
        self.resident_at(scope_handle, loan, parameter)
    }
    fn resident_at(
        &self,
        scope: ResourceHandleId,
        loan: ResourceHandleId,
        parameter: u32,
    ) -> ResourceResult<()> {
        let header = self.frame(scope)?;
        let resident = header
            .incoming
            .get(parameter as usize)
            .and_then(Option::as_ref)
            .ok_or(NativeResourceError::WrongOperation)?;
        if resident.parent_loan != loan || resident.callee_parameter != parameter {
            return Err(NativeResourceError::WrongOperation);
        }
        self.frame(resident.caller_frame)?;
        let call = self.source_call(self.call_for_scope(scope)?)?;
        if call.operation != resident.invoke_operation
            || call.frame != resident.caller_frame
            || call.phase != SourcePhase::Entered
        {
            return Err(NativeResourceError::WrongFrame);
        }
        match self.handles.get(&loan) {
            Some(NativeResourceEntry::Loan(value)) => {
                self.frame(value.frame)?;
                self.frame(value.source_frame)?;
                Ok(())
            }
            _ => Err(NativeResourceError::WrongFamily),
        }
    }
    fn source_loan(
        &self,
        frame: ResourceHandleId,
        handle: ResourceHandleId,
        source: NativeLoanSource,
        registry: &ResourceRegistry,
    ) -> ResourceResult<()> {
        match source {
            NativeLoanSource::ExistingBorrow { operation } => {
                let loan = self.exact_loan(handle, source)?;
                let mut ancestor = frame;
                loop {
                    if ancestor == loan.frame {
                        break;
                    }
                    ancestor = self
                        .frame(ancestor)?
                        .parent
                        .ok_or(NativeResourceError::WrongFrame)?;
                }
                if self.layout.operations()[operation as usize]
                    .site()
                    .function()
                    != self.layout.frames()[self.frame(frame)?.template as usize]
                        .site()
                        .function()
                {
                    return Err(NativeResourceError::WrongOperation);
                }
                self.custody.validate_borrowed(&loan.token, registry)?;
            }
            NativeLoanSource::IncomingViewFormal { scope, parameter } => {
                let scope = self.activation_frame(frame, scope)?;
                self.resident_at(scope, handle, parameter)?;
                let Some(NativeResourceEntry::Loan(loan)) = self.handles.get(&handle) else {
                    return Err(NativeResourceError::WrongFamily);
                };
                self.custody.validate_borrowed(&loan.token, registry)?;
            }
        }
        Ok(())
    }
    fn ordinary_actual(
        &self,
        ordinary: &values::NativeValues,
        value: u64,
        shape: u32,
    ) -> ResourceResult<()> {
        if let Some(NativeShape::HookDescriptor { hook }) = self.layout.shapes().get(shape as usize)
        {
            let signature = self.layout.hooks()[*hook as usize].signature();
            return match self.handles.get(&ResourceHandleId::new(value)?) {
                Some(NativeResourceEntry::Descriptor {
                    hook: actual,
                    signature: actual_signature,
                }) if actual == hook && *actual_signature == signature => Ok(()),
                _ => Err(NativeResourceError::WrongFamily),
            };
        }
        ordinary
            .validate_resource_ordinary(value, &self.layout, shape)
            .map_err(ordinary_error)
    }
    fn carrier(
        &self,
        handle: ResourceHandleId,
        slot: u32,
        registry: &ResourceRegistry,
    ) -> ResourceResult<()> {
        match self.handles.get(&handle) {
            Some(NativeResourceEntry::Owner(owner)) if owner.slot == slot => {
                self.frame(owner.frame)?;
                if !matches!(
                    self.layout.shapes()[self.layout.slots()[slot as usize].shape() as usize],
                    NativeShape::Resource { .. }
                ) {
                    return Err(NativeResourceError::WrongFamily);
                }
                self.custody.validate_owned(&owner.token, registry)?;
            }
            Some(NativeResourceEntry::Sum(sum))
                if sum.slot == slot && sum.shape == self.layout.slots()[slot as usize].shape() =>
            {
                self.frame(sum.frame)?;
                match &sum.payload {
                    NativeSumPayload::Some(token) | NativeSumPayload::Ok(token) => {
                        self.custody.validate_owned(token, registry)?;
                    }
                    NativeSumPayload::None | NativeSumPayload::Fail { .. } => {}
                }
            }
            _ => return Err(NativeResourceError::WrongFamily),
        }
        Ok(())
    }
    pub(super) fn carrier_frame_at(
        &self,
        current: ResourceHandleId,
        handle: ResourceHandleId,
        slot: u32,
    ) -> ResourceResult<()> {
        let actual = match self.handles.get(&handle) {
            Some(NativeResourceEntry::Owner(owner)) if owner.slot == slot => owner.frame,
            Some(NativeResourceEntry::Sum(sum)) if sum.slot == slot => sum.frame,
            _ => return Err(NativeResourceError::WrongFamily),
        };
        let expected =
            self.activation_frame(current, self.layout.slots()[slot as usize].frame())?;
        if actual != expected {
            return Err(NativeResourceError::WrongFrame);
        }
        Ok(())
    }
    pub(super) fn slot_empty(&self, frame: ResourceHandleId, slot: u32) -> ResourceResult<()> {
        self.slot_frame(slot, frame)?;
        if self.handles.values().any(|entry| match entry {
            NativeResourceEntry::Owner(owner) => owner.frame == frame && owner.slot == slot,
            NativeResourceEntry::Sum(sum) => sum.frame == frame && sum.slot == slot,
            NativeResourceEntry::Prepared(value) => {
                value.frame == frame && value.destination == slot
            }
            _ => false,
        }) {
            return Err(NativeResourceError::WrongOperation);
        }
        Ok(())
    }
    /// This private join is used only after an exact operation or Source boundary
    /// has selected both slots. It never turns an empty shell into an owner.
    pub(super) fn move_carrier(
        &mut self,
        registry: &ResourceRegistry,
        handle: ResourceHandleId,
        source: u32,
        destination: u32,
        frame: ResourceHandleId,
    ) -> ResourceResult<ResourceHandleId> {
        let mut ids = self.reserve(1)?;
        let output = next_handle(&mut ids)?;
        self.move_carrier_reserved(registry, handle, source, destination, frame, output)
    }
    fn move_carrier_reserved(
        &mut self,
        registry: &ResourceRegistry,
        handle: ResourceHandleId,
        source: u32,
        destination: u32,
        frame: ResourceHandleId,
        output: ResourceHandleId,
    ) -> ResourceResult<ResourceHandleId> {
        self.carrier(handle, source, registry)?;
        self.slot_empty(frame, destination)?;
        let source_row = &self.layout.slots()[source as usize];
        let destination_row = &self.layout.slots()[destination as usize];
        if source_row.shape() != destination_row.shape()
            || source_row.path() != destination_row.path()
        {
            return Err(NativeResourceError::WrongOperation);
        }
        let holder = self
            .custody
            .holder(&self.frame(frame)?.token, &self.layout.slot(destination)?)?;
        let mut entry = self
            .handles
            .remove(&handle)
            .ok_or(NativeResourceError::InvalidHandle)?;
        let outcome = match &mut entry {
            NativeResourceEntry::Owner(owner) => {
                self.custody.transfer(&mut owner.token, &holder, registry)
            }
            NativeResourceEntry::Sum(sum) => match &mut sum.payload {
                NativeSumPayload::Some(token) | NativeSumPayload::Ok(token) => {
                    self.custody.transfer(token, &holder, registry)
                }
                NativeSumPayload::None | NativeSumPayload::Fail { .. } => Ok(()),
            },
            _ => {
                self.handles.insert(handle, entry);
                return Err(NativeResourceError::WrongFamily);
            }
        };
        if let Err(error) = outcome {
            self.handles.insert(handle, entry);
            return Err(error.into());
        }
        match &mut entry {
            NativeResourceEntry::Owner(owner) => {
                owner.frame = frame;
                owner.slot = destination;
            }
            NativeResourceEntry::Sum(sum) => {
                sum.frame = frame;
                sum.slot = destination;
            }
            _ => unreachable!(),
        }
        self.handles.insert(output, entry);
        Ok(output)
    }
    pub(super) fn destination_frame(
        &self,
        frame: ResourceHandleId,
        operation: u32,
    ) -> ResourceResult<ResourceHandleId> {
        self.operation_frame(operation, frame)?;
        let destination = match self.operation(operation)? {
            NativeOperation::Acquire { destination, .. }
            | NativeOperation::Transfer { destination, .. }
            | NativeOperation::SumAdopt { destination, .. }
            | NativeOperation::SumTake { destination, .. } => *destination,
            _ => return Err(NativeResourceError::WrongOperation),
        };
        self.activation_frame(frame, self.layout.slots()[destination as usize].frame())
    }
    pub(super) fn scope_validate(
        &self,
        ordinary: &values::NativeValues,
        scope: ResourceHandleId,
        function: u32,
        signature: u32,
        template: u32,
    ) -> ResourceResult<()> {
        self.running(ordinary)?;
        let header = self.frame(scope)?;
        let row = &self.layout.frames()[header.template as usize];
        if self.active_frames.last() != Some(&scope)
            || header.template != template
            || row.role() != NativeFrameRole::Scope
            || row.site().function() != function
            || row.signature() != signature
        {
            return Err(NativeResourceError::WrongFrame);
        }
        if header.parent.is_none() {
            if self.attempt.as_ref().map(|a| a.root_frame) != Some(scope) {
                return Err(NativeResourceError::WrongFrame);
            }
        } else {
            let call = self.source_call(self.call_for_scope(scope)?)?;
            let (callee, sig, target, _) = self.source_row(call.operation)?;
            if call.phase != SourcePhase::Entered
                || callee != function
                || sig != signature
                || target != template
            {
                return Err(NativeResourceError::WrongFrame);
            }
        }
        Ok(())
    }
    pub(super) fn source_prepare(
        &mut self,
        ordinary: &values::NativeValues,
        frame: ResourceHandleId,
        operation: u32,
    ) -> ResourceResult<ResourceHandleId> {
        self.running(ordinary)?;
        self.operation_frame(operation, frame)?;
        let (_, _, _, row) = self.source_row(operation)?;
        let destination = match row.result {
            NativeSourceResult::Owned {
                caller_destination_frame,
                caller_destination_slot,
                ..
            } => {
                let destination = self.activation_frame(frame, caller_destination_frame)?;
                self.slot_empty(destination, caller_destination_slot)?;
                Some(destination)
            }
            NativeSourceResult::Ordinary { .. } => None,
        };
        let mut actuals = Vec::new();
        let mut parameters = Vec::new();
        actuals
            .try_reserve_exact(row.formals.len())
            .map_err(|_| NativeResourceError::Capacity)?;
        parameters
            .try_reserve_exact(row.formals.len())
            .map_err(|_| NativeResourceError::Capacity)?;
        actuals.resize(row.formals.len(), None);
        parameters.resize(row.formals.len(), 0);
        let mut ids = self.reserve(1)?;
        let handle = next_handle(&mut ids)?;
        self.handles.insert(
            handle,
            NativeResourceEntry::SourceCall(NativeSourceCall {
                frame,
                operation,
                actuals,
                parameters,
                next_source: 0,
                scope: None,
                return_frame: None,
                destination,
                body_status: 0,
                phase: SourcePhase::Prepared,
                completion: None,
            }),
        );
        Ok(handle)
    }
    pub(super) fn source_actual(
        &mut self,
        ordinary: &values::NativeValues,
        registry: &ResourceRegistry,
        handle: ResourceHandleId,
        source_index: u32,
        parameter: u32,
        value: u64,
    ) -> ResourceResult<()> {
        self.running(ordinary)?;
        let call = self.source_call(handle)?;
        self.operation_frame(call.operation, call.frame)?;
        let (_, _, _, row) = self.source_row(call.operation)?;
        if call.phase != SourcePhase::Prepared
            || call.next_source != source_index as usize
            || row.evaluation_order.get(source_index as usize) != Some(&parameter)
        {
            return Err(NativeResourceError::WrongOperation);
        }
        let formal = row
            .formals
            .get(parameter as usize)
            .ok_or(NativeResourceError::WrongOperation)?;
        if formal.source_index != source_index || call.actuals[parameter as usize].is_some() {
            return Err(NativeResourceError::WrongOperation);
        }
        match formal.value {
            NativeSourceValue::Ordinary => {
                self.ordinary_actual(ordinary, value, formal.actual_shape)?
            }
            NativeSourceValue::Owned {
                caller_argument_slot,
                ..
            } => {
                let id = ResourceHandleId::new(value)?;
                self.carrier(id, caller_argument_slot, registry)?;
                match self.handles.get(&id) {
                    Some(NativeResourceEntry::Owner(owner)) if owner.frame == call.frame => {}
                    Some(NativeResourceEntry::Sum(sum)) if sum.frame == call.frame => {}
                    _ => return Err(NativeResourceError::WrongFrame),
                }
            }
            NativeSourceValue::ResidentView { source } => {
                self.source_loan(call.frame, ResourceHandleId::new(value)?, source, registry)?
            }
        }
        let Some(NativeResourceEntry::SourceCall(call)) = self.handles.get_mut(&handle) else {
            return Err(NativeResourceError::WrongFamily);
        };
        call.actuals[parameter as usize] = Some(value);
        call.next_source += 1;
        Ok(())
    }
    fn begin_source_frame(
        &mut self,
        template: u32,
        parent: ResourceHandleId,
        role: NativeFrameRole,
        handle: ResourceHandleId,
    ) -> ResourceResult<ResourceHandleId> {
        if self.active_frames.last() != Some(&parent) {
            return Err(NativeResourceError::WrongFrame);
        }
        let attempt = self
            .attempt
            .as_ref()
            .ok_or(NativeResourceError::InvalidEntry)?;
        let purpose = attempt.purpose;
        let attempt_id = attempt.id;
        let row = &self.layout.frames()[template as usize];
        if row.role() != role
            || !row
                .parents()
                .contains(&NativeParent::Frame(self.frame(parent)?.template))
        {
            return Err(NativeResourceError::WrongFrame);
        }
        self.active_frames
            .try_reserve(1)
            .map_err(|_| NativeResourceError::Capacity)?;
        let kind = match role {
            NativeFrameRole::Scope => ResourceFrameKind::Scope,
            NativeFrameRole::Return => ResourceFrameKind::Return,
            _ => return Err(NativeResourceError::WrongFrame),
        };
        let token = self.custody.begin_frame(kind, purpose)?;
        self.handles.insert(
            handle,
            NativeResourceEntry::Frame(NativeFrameEntry {
                template,
                attempt: attempt_id,
                parent: Some(parent),
                token,
                incoming: Vec::new(),
            }),
        );
        self.active_frames.push(handle);
        Ok(handle)
    }
    pub(super) fn source_enter(
        &mut self,
        ordinary: &mut values::NativeValues,
        registry: &mut ResourceRegistry,
        handle: ResourceHandleId,
    ) -> ResourceResult<ResourceHandleId> {
        self.running(ordinary)?;
        let call = self.source_call(handle)?;
        let frame = call.frame;
        let operation = call.operation;
        self.operation_frame(operation, frame)?;
        let (_, _, template, row) = self.source_row(operation)?;
        if call.phase != SourcePhase::Prepared
            || call.next_source != row.formals.len()
            || call.actuals.iter().any(Option::is_none)
        {
            return Err(NativeResourceError::WrongOperation);
        }
        // Allocate every adapter-owned carrier before opening a callee or moving a token.
        let mut actuals = Vec::new();
        actuals
            .try_reserve_exact(call.actuals.len())
            .map_err(|_| NativeResourceError::Capacity)?;
        actuals.extend(call.actuals.iter().map(|v| v.unwrap_or(0)));
        let mut ids = self.reserve(2 + actuals.len())?;
        let reserved_return = next_handle(&mut ids)?;
        let reserved_scope = next_handle(&mut ids)?;
        let mut outputs = Vec::new();
        outputs
            .try_reserve_exact(actuals.len())
            .map_err(|_| NativeResourceError::Capacity)?;
        let mut incoming = Vec::new();
        incoming
            .try_reserve_exact(actuals.len())
            .map_err(|_| NativeResourceError::Capacity)?;
        for formal in &row.formals {
            outputs.push(next_handle(&mut ids)?);
            incoming.push(
                if let NativeSourceValue::ResidentView { .. } = formal.value {
                    Some(NativeResidentLoan {
                        parent_loan: ResourceHandleId::new(actuals[formal.parameter as usize])?,
                        caller_frame: frame,
                        invoke_operation: operation,
                        callee_parameter: formal.parameter,
                    })
                } else {
                    None
                },
            );
        }
        self.active_frames
            .try_reserve(2)
            .map_err(|_| NativeResourceError::Capacity)?;
        for formal in &row.formals {
            let actual = actuals[formal.parameter as usize];
            match formal.value {
                NativeSourceValue::Owned {
                    caller_argument_slot,
                    ..
                } => self.carrier(
                    ResourceHandleId::new(actual)?,
                    caller_argument_slot,
                    registry,
                )?,
                NativeSourceValue::ResidentView { source } => {
                    self.source_loan(frame, ResourceHandleId::new(actual)?, source, registry)?
                }
                NativeSourceValue::Ordinary => {
                    self.ordinary_actual(ordinary, actual, formal.actual_shape)?
                }
            }
        }
        let return_frame = if let Some(template) = row.callee_return {
            Some(self.begin_source_frame(
                template,
                frame,
                NativeFrameRole::Return,
                reserved_return,
            )?)
        } else {
            None
        };
        let scope_result = self.begin_source_frame(
            template,
            return_frame.unwrap_or(frame),
            NativeFrameRole::Scope,
            reserved_scope,
        );
        let scope = match scope_result {
            Ok(scope) => scope,
            Err(error) => {
                if let Some(return_frame) = return_frame {
                    self.retire_frame(ordinary, registry, return_frame)?;
                }
                return Err(error);
            }
        };
        if let Some(NativeResourceEntry::SourceCall(call)) = self.handles.get_mut(&handle) {
            call.scope = Some(scope);
            call.return_frame = return_frame;
            call.phase = SourcePhase::Entered;
        }
        let Some(NativeResourceEntry::Frame(header)) = self.handles.get_mut(&scope) else {
            return Err(NativeResourceError::WrongFrame);
        };
        header.incoming = incoming;
        // All holders are checked empty and all core transfer bookkeeping is reserved
        // before the first owner leaves its caller argument slot.
        let preflight: ResourceResult<()> = (|| {
            let mut slots = Vec::new();
            slots
                .try_reserve_exact(row.formals.len())
                .map_err(|_| NativeResourceError::Capacity)?;
            let mut token_handles = Vec::new();
            token_handles
                .try_reserve_exact(row.formals.len())
                .map_err(|_| NativeResourceError::Capacity)?;
            for formal in &row.formals {
                if let NativeSourceValue::Owned {
                    callee_parameter_slot,
                    ..
                } = formal.value
                {
                    self.slot_empty(scope, callee_parameter_slot)?;
                    let actual = ResourceHandleId::new(actuals[formal.parameter as usize])?;
                    match self.handles.get(&actual) {
                        Some(NativeResourceEntry::Owner(_))
                        | Some(NativeResourceEntry::Sum(NativeResourceSum {
                            payload: NativeSumPayload::Some(_) | NativeSumPayload::Ok(_),
                            ..
                        })) => {
                            token_handles.push(actual);
                            slots.push(self.layout.slot(callee_parameter_slot)?);
                        }
                        _ => {} // None/Fail shells have no owner generation to transfer.
                    }
                }
            }
            let mut transfers = Vec::new();
            transfers
                .try_reserve_exact(slots.len())
                .map_err(|_| NativeResourceError::Capacity)?;
            for (index, handle) in token_handles.iter().enumerate() {
                let token = match self.handles.get(handle) {
                    Some(NativeResourceEntry::Owner(owner)) => &owner.token,
                    Some(NativeResourceEntry::Sum(NativeResourceSum {
                        payload: NativeSumPayload::Some(token) | NativeSumPayload::Ok(token),
                        ..
                    })) => token,
                    _ => return Err(NativeResourceError::WrongFamily),
                };
                transfers.push((token, &slots[index]));
            }
            let Some(NativeResourceEntry::Frame(header)) = self.handles.get(&scope) else {
                return Err(NativeResourceError::WrongFrame);
            };
            self.custody
                .preflight_transfer_batch(&header.token, &transfers, registry)?;
            Ok(())
        })();
        if let Err(error) = preflight {
            self.abort_source(ordinary, registry, handle)?;
            return Err(error);
        }
        for formal in &row.formals {
            let actual = actuals[formal.parameter as usize];
            let acquired = match formal.value {
                NativeSourceValue::Owned {
                    caller_argument_slot,
                    callee_parameter_slot,
                } => self
                    .move_carrier_reserved(
                        registry,
                        ResourceHandleId::new(actual)?,
                        caller_argument_slot,
                        callee_parameter_slot,
                        scope,
                        outputs[formal.parameter as usize],
                    )
                    .map(ResourceHandleId::raw),
                NativeSourceValue::ResidentView { .. } => Ok(actual),
                NativeSourceValue::Ordinary => Ok(actual),
            };
            match acquired {
                Ok(value) => {
                    let Some(NativeResourceEntry::SourceCall(call)) = self.handles.get_mut(&handle)
                    else {
                        return Err(NativeResourceError::WrongFamily);
                    };
                    call.parameters[formal.parameter as usize] = value;
                }
                Err(error) => {
                    self.abort_source(ordinary, registry, handle)?;
                    return Err(error);
                }
            }
        }
        Ok(scope)
    }
    fn abort_source(
        &mut self,
        ordinary: &mut values::NativeValues,
        registry: &mut ResourceRegistry,
        handle: ResourceHandleId,
    ) -> ResourceResult<()> {
        let call = self.source_call(handle)?;
        let scope = call.scope;
        let return_frame = call.return_frame;
        let mut first = None;
        for frame in [scope, return_frame].into_iter().flatten() {
            if let Err(error) = self.retire_frame(ordinary, registry, frame) {
                first.get_or_insert(error);
            }
        }
        if let Some(NativeResourceEntry::SourceCall(call)) = self.handles.get_mut(&handle) {
            call.phase = SourcePhase::Aborted;
        }
        first.map_or(Ok(()), Err)
    }
    pub(super) fn source_parameter(
        &self,
        ordinary: &values::NativeValues,
        scope: ResourceHandleId,
        parameter: u32,
    ) -> ResourceResult<u64> {
        self.running(ordinary)?;
        self.frame(scope)?;
        let call = self.source_call(self.call_for_scope(scope)?)?;
        if call.phase != SourcePhase::Entered {
            return Err(NativeResourceError::WrongFrame);
        }
        let value = call
            .parameters
            .get(parameter as usize)
            .copied()
            .ok_or(NativeResourceError::WrongOperation)?;
        let (_, _, _, row) = self.source_row(call.operation)?;
        let formal = row
            .formals
            .get(parameter as usize)
            .ok_or(NativeResourceError::WrongOperation)?;
        match formal.value {
            NativeSourceValue::Owned {
                callee_parameter_slot,
                ..
            } => match self.handles.get(&ResourceHandleId::new(value)?) {
                Some(NativeResourceEntry::Owner(owner))
                    if owner.frame == scope && owner.slot == callee_parameter_slot => {}
                Some(NativeResourceEntry::Sum(sum))
                    if sum.frame == scope && sum.slot == callee_parameter_slot => {}
                _ => return Err(NativeResourceError::WrongFamily),
            },
            NativeSourceValue::ResidentView { .. } => {
                self.resident_at(scope, ResourceHandleId::new(value)?, parameter)?
            }
            NativeSourceValue::Ordinary => {
                self.ordinary_actual(ordinary, value, formal.callee_shape)?
            }
        }
        Ok(value)
    }
    fn record_source_completion(
        &mut self,
        handle: ResourceHandleId,
        failure: Option<NativeResourceError>,
    ) -> ResourceResult<()> {
        let call = self.source_call(handle)?;
        let scope = call.scope.ok_or(NativeResourceError::WrongFrame)?;
        if !matches!(call.phase, SourcePhase::Completed | SourcePhase::Aborted)
            || self.handles.contains_key(&scope)
            || self.active_frames.contains(&scope)
            || call.return_frame.is_some_and(|frame| {
                self.handles.contains_key(&frame) || self.active_frames.contains(&frame)
            })
        {
            // Refused retirement and pending publication carry no completion authority.
            return Ok(());
        }
        let completion = SourceCompletion {
            scope,
            return_frame: call.return_frame,
            original_body_status: call.body_status,
            failure,
        };
        let Some(NativeResourceEntry::SourceCall(call)) = self.handles.get_mut(&handle) else {
            return Err(NativeResourceError::WrongFamily);
        };
        call.completion = Some(completion);
        Ok(())
    }

    pub(super) fn source_status(
        &self,
        ordinary: &values::NativeValues,
        handle: ResourceHandleId,
    ) -> ResourceResult<()> {
        let call = self.source_call(handle)?;
        self.operation_frame(call.operation, call.frame)?;
        self.source_row(call.operation)?;
        let completion = call.completion.ok_or(NativeResourceError::BodyFailed)?;
        if call.scope != Some(completion.scope)
            || call.return_frame != completion.return_frame
            || call.body_status != completion.original_body_status
            || self.handles.contains_key(&completion.scope)
            || self.active_frames.contains(&completion.scope)
            || completion.return_frame.is_some_and(|frame| {
                self.handles.contains_key(&frame) || self.active_frames.contains(&frame)
            })
        {
            return Err(NativeResourceError::WrongFrame);
        }
        match call.phase {
            SourcePhase::Aborted => {
                Err(completion.failure.ok_or(NativeResourceError::BodyFailed)?)
            }
            SourcePhase::Completed
                if completion.original_body_status == 0 && completion.failure.is_none() =>
            {
                self.running(ordinary)?;
                Ok(())
            }
            _ => Err(NativeResourceError::BodyFailed),
        }
    }

    #[cfg(test)]
    pub(super) fn source_completed_body_status(
        &self,
        handle: ResourceHandleId,
    ) -> ResourceResult<Option<u32>> {
        Ok(self
            .source_call(handle)?
            .completion
            .map(|completion| completion.original_body_status))
    }
    pub(super) fn scope_complete(
        &mut self,
        ordinary: &mut values::NativeValues,
        registry: &mut ResourceRegistry,
        scope: ResourceHandleId,
        operation: u32,
        body_status: u32,
    ) -> ResourceResult<()> {
        self.operation_frame(operation, scope)?;
        if !matches!(self.operation(operation)?, NativeOperation::Complete { .. })
            || self.layout.frames()[self.frame(scope)?.template as usize].role()
                != NativeFrameRole::Scope
        {
            return Err(NativeResourceError::WrongOperation);
        }
        let root = self.frame(scope)?.parent.is_none();
        if root {
            let attempt = self
                .attempt
                .as_ref()
                .ok_or(NativeResourceError::InvalidEntry)?;
            let header = self.frame(scope)?;
            let row = &self.layout.frames()[header.template as usize];
            if attempt.phase != AttemptPhase::Running
                || attempt.root_frame != scope
                || attempt.entry != self.entry
                || attempt.root_body_status.is_some()
                || header.template != self.entry.scope
                || row.site().function() != self.entry.function
                || row.signature() != self.entry.signature
            {
                return Err(NativeResourceError::InvalidEntry);
            }
        }
        let call = if !root {
            Some(self.call_for_scope(scope)?)
        } else {
            None
        };
        if let Some(handle) = call {
            let Some(NativeResourceEntry::SourceCall(call)) = self.handles.get_mut(&handle) else {
                return Err(NativeResourceError::WrongFamily);
            };
            call.body_status = body_status;
        }
        let mut cleanup = self.retire_frame(ordinary, registry, scope);
        // A refused cleanup can keep its frame live. Retain the original
        // status only after this exact root has actually retired, even if its
        // finalizer failed. Do not substitute the selected cleanup/body error.
        if root && !self.handles.contains_key(&scope) && !self.active_frames.contains(&scope) {
            let attempt = self
                .attempt
                .as_mut()
                .ok_or(NativeResourceError::InvalidEntry)?;
            attempt.root_body_status = Some(body_status);
        }
        let body = if body_status != 0 {
            Err(NativeResourceError::BodyFailed)
        } else {
            self.running(ordinary).map(|_| ())
        };
        if let Some(handle) = call {
            let return_frame = self.source_call(handle)?.return_frame;
            if cleanup.is_err() || body.is_err() {
                if let Some(frame) = return_frame {
                    if let Err(error) = self.retire_frame(ordinary, registry, frame) {
                        if cleanup.is_ok() {
                            cleanup = Err(error);
                        }
                    }
                }
                if let Some(NativeResourceEntry::SourceCall(call)) = self.handles.get_mut(&handle) {
                    call.phase = SourcePhase::Aborted;
                }
            } else if let Some(NativeResourceEntry::SourceCall(call)) =
                self.handles.get_mut(&handle)
            {
                call.phase = if return_frame.is_some() {
                    SourcePhase::ScopeCompleted
                } else {
                    SourcePhase::Completed
                };
            }
        }
        if let Some(handle) = call {
            // Preserve the original nonzero body independently of selected cleanup.
            // A clean body whose real Scope cleanup failed must still stop its caller.
            let failure = if body_status != 0 {
                Some(NativeResourceError::SourceBodyStatus(JettRuntimeStatusV1(
                    body_status,
                )))
            } else {
                cleanup
                    .as_ref()
                    .err()
                    .copied()
                    .or_else(|| body.as_ref().err().copied())
            };
            self.record_source_completion(handle, failure)?;
        }
        cleanup?;
        body
    }
    pub(super) fn return_publish(
        &mut self,
        ordinary: &mut values::NativeValues,
        registry: &mut ResourceRegistry,
        scope: ResourceHandleId,
        operation: u32,
        provisional: ResourceHandleId,
    ) -> ResourceResult<ResourceHandleId> {
        self.running(ordinary)?;
        let handle = self.call_for_scope(scope)?;
        let call = self.source_call(handle)?;
        if call.phase != SourcePhase::ScopeCompleted || call.body_status != 0 {
            return Err(NativeResourceError::WrongFrame);
        }
        let (_, _, template, row) = self.source_row(call.operation)?;
        let source = match self.operation(operation)? {
            NativeOperation::PublishReturn {
                frame,
                source_return_slot,
            } if *frame == template => *source_return_slot,
            _ => return Err(NativeResourceError::WrongOperation),
        };
        let NativeSourceResult::Owned {
            caller_destination_slot,
            permitted_return_slots,
            ..
        } = row.result
        else {
            return Err(NativeResourceError::WrongOperation);
        };
        if !permitted_return_slots.contains(&source) {
            return Err(NativeResourceError::WrongOperation);
        }
        let return_frame = call.return_frame.ok_or(NativeResourceError::WrongFrame)?;
        let destination = call.destination.ok_or(NativeResourceError::WrongFrame)?;
        if self.active_frames.last() != Some(&return_frame) {
            return Err(NativeResourceError::WrongFrame);
        }
        self.carrier(provisional, source, registry)?;
        self.carrier_frame_at(return_frame, provisional, source)?;
        if self.handles.iter().any(|(&id, entry)| {
            id != provisional
                && match entry {
                    NativeResourceEntry::Owner(owner) => owner.frame == return_frame,
                    NativeResourceEntry::Sum(sum) => sum.frame == return_frame,
                    NativeResourceEntry::Loan(loan) => loan.frame == return_frame,
                    NativeResourceEntry::Prepared(prepared) => prepared.frame == return_frame,
                    _ => false,
                }
        }) {
            return Err(NativeResourceError::WrongOperation);
        }
        let output = self.move_carrier(
            registry,
            provisional,
            source,
            caller_destination_slot,
            destination,
        )?;
        self.retire_frame(ordinary, registry, return_frame)?;
        let Some(NativeResourceEntry::SourceCall(call)) = self.handles.get_mut(&handle) else {
            return Err(NativeResourceError::WrongFamily);
        };
        call.phase = SourcePhase::Completed;
        self.record_source_completion(handle, None)?;
        Ok(output)
    }
    pub(super) fn absent_sum(
        &mut self,
        ordinary: &values::NativeValues,
        frame: ResourceHandleId,
        operation: u32,
    ) -> ResourceResult<ResourceHandleId> {
        self.running(ordinary)?;
        self.operation_frame(operation, frame)?;
        let slot = match self.operation(operation)? {
            NativeOperation::CreateAbsentSum {
                destination_slot, ..
            } => *destination_slot,
            _ => return Err(NativeResourceError::WrongOperation),
        };
        self.slot_empty(frame, slot)?;
        let mut ids = self.reserve(1)?;
        let output = next_handle(&mut ids)?;
        self.handles.insert(
            output,
            NativeResourceEntry::Sum(NativeResourceSum {
                frame,
                slot,
                shape: self.layout.slots()[slot as usize].shape(),
                payload: NativeSumPayload::None,
            }),
        );
        Ok(output)
    }
    pub(super) fn failure_sum(
        &mut self,
        ordinary: &values::NativeValues,
        frame: ResourceHandleId,
        operation: u32,
        companion: u64,
    ) -> ResourceResult<ResourceHandleId> {
        self.running(ordinary)?;
        self.operation_frame(operation, frame)?;
        let (slot, shape) = match self.operation(operation)? {
            NativeOperation::CreateFailureSum {
                destination_slot,
                failure_shape,
                ..
            } => (*destination_slot, *failure_shape),
            _ => return Err(NativeResourceError::WrongOperation),
        };
        ordinary
            .validate_resource_ordinary(companion, &self.layout, shape)
            .map_err(ordinary_error)?;
        self.slot_empty(frame, slot)?;
        let mut ids = self.reserve(1)?;
        let output = next_handle(&mut ids)?;
        self.handles.insert(
            output,
            NativeResourceEntry::Sum(NativeResourceSum {
                frame,
                slot,
                shape: self.layout.slots()[slot as usize].shape(),
                payload: NativeSumPayload::Fail {
                    shape,
                    string: companion,
                },
            }),
        );
        Ok(output)
    }
    pub(super) fn failure_companion_take(
        &mut self,
        ordinary: &values::NativeValues,
        frame: ResourceHandleId,
        operation: u32,
        handle: ResourceHandleId,
    ) -> ResourceResult<u64> {
        self.running(ordinary)?;
        self.operation_frame(operation, frame)?;
        let (source, shape) = match self.operation(operation)? {
            NativeOperation::TakeFailureCompanion {
                source_sum_slot,
                failure_shape,
                ..
            } => (*source_sum_slot, *failure_shape),
            _ => return Err(NativeResourceError::WrongOperation),
        };
        let Some(NativeResourceEntry::Sum(sum)) = self.handles.get(&handle) else {
            return Err(NativeResourceError::WrongFamily);
        };
        if sum.slot != source {
            return Err(NativeResourceError::WrongOperation);
        }
        self.carrier_frame_at(frame, handle, source)?;
        let NativeSumPayload::Fail {
            shape: actual,
            string,
        } = sum.payload
        else {
            return Err(NativeResourceError::WrongFamily);
        };
        if actual != shape {
            return Err(NativeResourceError::WrongOperation);
        }
        ordinary
            .validate_resource_ordinary(string, &self.layout, shape)
            .map_err(ordinary_error)?;
        self.handles.remove(&handle);
        Ok(string)
    }
}
