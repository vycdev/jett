//! Nonowning conditional carriers. Shell identity stays separate from payload custody.
use super::*;

impl NativeResourceState {
    pub(super) fn validate_sum_parent(
        &self,
        handle: ResourceHandleId,
        registry: &ResourceRegistry,
    ) -> ResourceResult<()> {
        self.sum_lease(handle, registry).map(|_| ())
    }
    pub(super) fn projected_sum_parent(
        &self,
        frame: ResourceHandleId,
        handle: ResourceHandleId,
        source: NativeSumLoanSource,
    ) -> ResourceResult<()> {
        let Some(NativeResourceEntry::SumLoan(loan)) = self.handles.get(&handle) else {
            return Err(NativeResourceError::WrongFamily);
        };
        self.frame(loan.frame)?;
        self.frame(loan.source_frame)?;
        let Some(NativeResourceEntry::Sum(sum)) = self.handles.get(&loan.shell) else {
            return Err(NativeResourceError::InvalidHandle);
        };
        if sum.slot != loan.source_slot || sum.frame != loan.source_frame || sum.shape != loan.shape
        {
            return Err(NativeResourceError::WrongOperation);
        }
        if self.carrier_adapters.contains_key(&loan.shell) {
            self.carrier_adapter_validate(loan.shell, handle)?;
        }
        match source {
            NativeSumLoanSource::ExistingBorrow { operation } => {
                if operation != loan.borrow_operation
                    || !matches!(
                        self.operation(operation)?,
                        NativeOperation::BorrowSum { .. }
                    )
                {
                    return Err(NativeResourceError::WrongOperation);
                }
                if self.activation_frame(frame, self.frame(loan.frame)?.template)? != loan.frame {
                    return Err(NativeResourceError::WrongFrame);
                }
            }
            NativeSumLoanSource::IncomingViewFormal { scope, parameter } => {
                self.resident_sum_at(self.activation_frame(frame, scope)?, handle, parameter)?;
            }
            NativeSumLoanSource::CarrierProjected { operation } => {
                self.carrier_adapter_source(frame, handle, operation)?;
            }
        }
        Ok(())
    }
    fn sum_lease(
        &self,
        handle: ResourceHandleId,
        registry: &ResourceRegistry,
    ) -> ResourceResult<&NativeSumLoan> {
        let Some(NativeResourceEntry::SumLoan(loan)) = self.handles.get(&handle) else {
            return Err(NativeResourceError::WrongFamily);
        };
        self.frame(loan.frame)?;
        self.frame(loan.source_frame)?;
        let Some(NativeResourceEntry::Sum(sum)) = self.handles.get(&loan.shell) else {
            return Err(NativeResourceError::InvalidHandle);
        };
        if sum.frame != loan.source_frame || sum.slot != loan.source_slot || sum.shape != loan.shape
        {
            return Err(NativeResourceError::WrongOperation);
        }
        if self.carrier_adapters.contains_key(&loan.shell) {
            self.carrier_adapter_validate(loan.shell, handle)?;
        }
        self.carrier_frame_at(loan.frame, loan.shell, loan.source_slot)?;
        match (&sum.payload, &loan.token) {
            (NativeSumPayload::Some(owner) | NativeSumPayload::Ok(owner), Some(token)) => {
                if self.custody.validate_owned(owner, registry)?
                    != self.custody.validate_borrowed(token, registry)?
                {
                    return Err(NativeResourceError::WrongOperation);
                }
            }
            (NativeSumPayload::None | NativeSumPayload::Fail { .. }, None) => {}
            _ => return Err(NativeResourceError::WrongFamily),
        }
        Ok(loan)
    }
    pub(super) fn source_sum_loan(
        &self,
        frame: ResourceHandleId,
        handle: ResourceHandleId,
        source: NativeSumLoanSource,
        shape: Option<u32>,
        registry: &ResourceRegistry,
    ) -> ResourceResult<&NativeSumLoan> {
        let loan = self.sum_lease(handle, registry)?;
        if shape.is_some_and(|shape| shape != loan.shape) {
            return Err(NativeResourceError::WrongFamily);
        }
        match source {
            NativeSumLoanSource::ExistingBorrow { operation } => {
                if loan.borrow_operation != operation
                    || !matches!(
                        self.operation(operation)?,
                        NativeOperation::BorrowSum { .. }
                    )
                {
                    return Err(NativeResourceError::WrongOperation);
                }
                if self.activation_frame(frame, self.frame(loan.frame)?.template)? != loan.frame {
                    return Err(NativeResourceError::WrongFrame);
                }
            }
            NativeSumLoanSource::IncomingViewFormal { scope, parameter } => {
                let scope = self.activation_frame(frame, scope)?;
                self.resident_sum_at(scope, handle, parameter)?;
            }
            NativeSumLoanSource::CarrierProjected { operation } => {
                self.carrier_adapter_source(frame, handle, operation)?;
            }
        }
        Ok(loan)
    }
    pub(super) fn sum_unborrowed(&self, shell: ResourceHandleId) -> ResourceResult<()> {
        if self
            .handles
            .values()
            .any(|entry| matches!(entry, NativeResourceEntry::SumLoan(loan) if loan.shell == shell))
        {
            return Err(CustodyError::ActiveBorrow.into());
        }
        Ok(())
    }
    pub(super) fn frame_shells_unborrowed(&self, frame: ResourceHandleId) -> ResourceResult<()> {
        for entry in self.handles.values() {
            if let NativeResourceEntry::SumLoan(loan) = entry {
                if loan.source_frame == frame && loan.frame != frame {
                    return Err(CustodyError::ActiveBorrow.into());
                }
            }
        }
        Ok(())
    }
    pub(super) fn sum_borrow(
        &mut self,
        ordinary: &values::NativeValues,
        registry: &ResourceRegistry,
        operation: u32,
        frame: ResourceHandleId,
        shell: ResourceHandleId,
    ) -> ResourceResult<ResourceHandleId> {
        self.running(ordinary)?;
        self.runtime_purpose()?;
        self.operation_frame(operation, frame)?;
        let (source, lease_frame) = match self.operation(operation)? {
            NativeOperation::BorrowSum {
                source_sum_slot,
                lease_frame,
                ..
            } => (*source_sum_slot, *lease_frame),
            _ => return Err(NativeResourceError::WrongOperation),
        };
        if lease_frame != self.frame(frame)?.template {
            return Err(NativeResourceError::WrongFrame);
        }
        self.carrier(shell, source, registry)?;
        self.carrier_frame_at(frame, shell, source)?;
        let mut ids = self.reserve(1)?;
        let handle = next_handle(&mut ids)?;
        let Some(NativeResourceEntry::Sum(sum)) = self.handles.get(&shell) else {
            return Err(NativeResourceError::WrongFamily);
        };
        let (source_frame, shape) = (sum.frame, sum.shape);
        let token = match &sum.payload {
            NativeSumPayload::Some(owner) | NativeSumPayload::Ok(owner) => {
                let Some(NativeResourceEntry::Frame(header)) = self.handles.get(&frame) else {
                    return Err(NativeResourceError::WrongFrame);
                };
                Some(self.custody.begin_borrow(owner, &header.token, registry)?)
            }
            NativeSumPayload::Fail { shape, string } => {
                ordinary
                    .validate_resource_ordinary(*string, &self.layout, *shape)
                    .map_err(ordinary_error)?;
                None
            }
            NativeSumPayload::None => None,
        };
        self.handles.insert(
            handle,
            NativeResourceEntry::SumLoan(NativeSumLoan {
                frame,
                borrow_operation: operation,
                shell,
                source_frame,
                source_slot: source,
                shape,
                token,
            }),
        );
        Ok(handle)
    }
    pub(super) fn sum_view_tag(
        &self,
        ordinary: &values::NativeValues,
        registry: &ResourceRegistry,
        operation: u32,
        frame: ResourceHandleId,
        handle: ResourceHandleId,
    ) -> ResourceResult<u32> {
        self.running(ordinary)?;
        self.operation_frame(operation, frame)?;
        let source = match self.operation(operation)? {
            NativeOperation::ObserveSumView { source, .. } => *source,
            _ => return Err(NativeResourceError::WrongOperation),
        };
        let loan = self.source_sum_loan(frame, handle, source, None, registry)?;
        let Some(NativeResourceEntry::Sum(sum)) = self.handles.get(&loan.shell) else {
            return Err(NativeResourceError::WrongFamily);
        };
        Ok(u32::from(matches!(
            sum.payload,
            NativeSumPayload::Some(_) | NativeSumPayload::Ok(_)
        )))
    }
    pub(super) fn sum_view_project(
        &mut self,
        ordinary: &values::NativeValues,
        registry: &ResourceRegistry,
        operation: u32,
        frame: ResourceHandleId,
        handle: ResourceHandleId,
    ) -> ResourceResult<ResourceHandleId> {
        self.running(ordinary)?;
        self.runtime_purpose()?;
        self.operation_frame(operation, frame)?;
        let (source, path, lease_frame) = match self.operation(operation)? {
            NativeOperation::ProjectSumView {
                source,
                path,
                lease_frame,
                ..
            } => (*source, *path, *lease_frame),
            _ => return Err(NativeResourceError::WrongOperation),
        };
        if lease_frame != self.frame(frame)?.template {
            return Err(NativeResourceError::WrongFrame);
        }
        let parent = self.source_sum_loan(frame, handle, source, None, registry)?;
        let (shell, source_slot, source_frame) =
            (parent.shell, parent.source_slot, parent.source_frame);
        let mut ids = self.reserve(1)?;
        let output = next_handle(&mut ids)?;
        let Some(NativeResourceEntry::Sum(sum)) = self.handles.get(&shell) else {
            return Err(NativeResourceError::WrongFamily);
        };
        let owner = match (&sum.payload, path) {
            (NativeSumPayload::Some(token), NativePayloadStep::Some)
            | (NativeSumPayload::Ok(token), NativePayloadStep::Ok) => token,
            _ => return Err(NativeResourceError::WrongOperation),
        };
        let Some(NativeResourceEntry::Frame(header)) = self.handles.get(&frame) else {
            return Err(NativeResourceError::WrongFrame);
        };
        let token = self.custody.begin_borrow(owner, &header.token, registry)?;
        self.handles.insert(
            output,
            NativeResourceEntry::Loan(NativeLoanEntry {
                frame,
                borrow_operation: operation,
                source_slot,
                source_frame,
                token,
                parent_sum: Some(handle),
            }),
        );
        Ok(output)
    }
    pub(super) fn sum_failure_read(
        &mut self,
        ordinary: &mut values::NativeValues,
        registry: &ResourceRegistry,
        operation: u32,
        frame: ResourceHandleId,
        handle: ResourceHandleId,
    ) -> ResourceResult<u64> {
        self.running(ordinary)?;
        self.operation_frame(operation, frame)?;
        let (source, shape) = match self.operation(operation)? {
            NativeOperation::ReadFailureCompanion {
                source,
                failure_shape,
                ..
            } => (*source, *failure_shape),
            _ => return Err(NativeResourceError::WrongOperation),
        };
        let parent = self.source_sum_loan(frame, handle, source, None, registry)?;
        let Some(NativeResourceEntry::Sum(sum)) = self.handles.get(&parent.shell) else {
            return Err(NativeResourceError::WrongFamily);
        };
        let NativeSumPayload::Fail {
            shape: actual,
            string,
        } = sum.payload
        else {
            return Err(NativeResourceError::WrongOperation);
        };
        if actual != shape {
            return Err(NativeResourceError::WrongOperation);
        }
        ordinary
            .clone_resource_string_companion(&self.layout, shape, string)
            .map_err(ordinary_error)
    }
    fn sum_borrow_end_inner(&mut self, handle: ResourceHandleId) -> ResourceResult<()> {
        if self.handles.values().any(|entry| matches!(entry, NativeResourceEntry::Loan(loan) if loan.parent_sum == Some(handle))) { return Err(CustodyError::ActiveBorrow.into()); }
        let Some(NativeResourceEntry::SumLoan(loan)) = self.handles.get(&handle) else {
            return Err(NativeResourceError::WrongFamily);
        };
        if self.carrier_adapters.contains_key(&loan.shell) {
            self.carrier_adapter_validate(loan.shell, handle)?;
        }
        let Some(NativeResourceEntry::SumLoan(mut loan)) = self.handles.remove(&handle) else {
            return Err(NativeResourceError::WrongFamily);
        };
        if let Some(token) = &mut loan.token {
            if let Err(error) = self.custody.end_borrow(token) {
                self.handles
                    .insert(handle, NativeResourceEntry::SumLoan(loan));
                return Err(error.into());
            }
        }
        if let Some(origin) = self.carrier_adapters.get(&loan.shell) {
            if origin.sum_loan != handle {
                self.handles
                    .insert(handle, NativeResourceEntry::SumLoan(loan));
                return Err(NativeResourceError::WrongOperation);
            }
            let carrier_loan = origin.loan;
            if let Err(error) = self.carrier_end_loan(loan.frame, carrier_loan) {
                self.handles
                    .insert(handle, NativeResourceEntry::SumLoan(loan));
                return Err(error);
            }
            // Retain the shell's seal until its lease frame retires. Ending a
            // view does not authorize adoption by an ordinary legacy slot.
        }
        Ok(())
    }
    pub(super) fn sum_borrow_end(
        &mut self,
        registry: &ResourceRegistry,
        operation: u32,
        frame: ResourceHandleId,
        handle: ResourceHandleId,
    ) -> ResourceResult<()> {
        self.operation_frame(operation, frame)?;
        let borrow = match self.operation(operation)? {
            NativeOperation::EndSumBorrow { borrow, .. } => *borrow,
            _ => return Err(NativeResourceError::WrongOperation),
        };
        let source = match self.operation(borrow)? {
            NativeOperation::BorrowSum { .. } => {
                NativeSumLoanSource::ExistingBorrow { operation: borrow }
            }
            NativeOperation::Carrier { .. } => {
                NativeSumLoanSource::CarrierProjected { operation: borrow }
            }
            _ => return Err(NativeResourceError::WrongOperation),
        };
        let loan = self.source_sum_loan(frame, handle, source, None, registry)?;
        if loan.frame != frame {
            return Err(NativeResourceError::WrongFrame);
        }
        self.sum_borrow_end_inner(handle)
    }
    /// Complete only this frame's child projections, then its parent shell leases.
    /// Incoming parent handles belong to the caller and are never adopted here.
    pub(super) fn retire_sum_views(&mut self, frame: ResourceHandleId) -> ResourceResult<()> {
        self.frame_shells_unborrowed(frame)?;
        if !self.handles.values().any(|entry| matches!(entry, NativeResourceEntry::Loan(loan) if loan.frame == frame && loan.parent_sum.is_some()) || matches!(entry, NativeResourceEntry::SumLoan(loan) if loan.frame == frame)) { return Ok(()); }
        let mut children = Vec::new();
        let mut parents = Vec::new();
        children
            .try_reserve(self.handles.len())
            .map_err(|_| NativeResourceError::Capacity)?;
        parents
            .try_reserve(self.handles.len())
            .map_err(|_| NativeResourceError::Capacity)?;
        for (&id, entry) in &self.handles {
            match entry {
                NativeResourceEntry::Loan(loan)
                    if loan.frame == frame && loan.parent_sum.is_some() =>
                {
                    children.push(id)
                }
                NativeResourceEntry::SumLoan(loan) if loan.frame == frame => parents.push(id),
                _ => {}
            }
        }
        let mut first: Option<NativeResourceError> = None;
        for handle in children.into_iter().rev() {
            let Some(NativeResourceEntry::Loan(mut loan)) = self.handles.remove(&handle) else {
                return Err(NativeResourceError::WrongFamily);
            };
            if let Err(error) = self.custody.end_borrow(&mut loan.token) {
                self.handles.insert(handle, NativeResourceEntry::Loan(loan));
                first.get_or_insert(error.into());
            }
        }
        for handle in parents.into_iter().rev() {
            if let Err(error) = self.sum_borrow_end_inner(handle) {
                first.get_or_insert(error);
            }
        }
        first.map_or(Ok(()), Err)
    }
}
