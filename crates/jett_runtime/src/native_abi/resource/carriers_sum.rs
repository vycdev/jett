//! V3 adapters keep absent legacy sums attached to their carrier projection.
use super::carriers::{
    NativeCarrierLoan, NativeCarrierSumOrigin, NativeCarrierTree, selected, take_selected,
};
use super::*;
use crate::resource_custody::{
    NativeCarrierLoanSource, NativeCarrierNode, NativeCarrierOperation, NativeCarrierPath,
    NativeCarrierSelector, NativeCarrierSource,
};

#[derive(Clone, Copy)]
enum AbsentCompanion {
    None,
    Fail { shape: u32, bits: u64 },
}

fn adapter_projection_matches(
    expected: &[NativeCarrierPath],
    actual: &[NativeCarrierPath],
) -> bool {
    expected.len() == actual.len()
        && expected
            .iter()
            .zip(actual)
            .all(|(expected, actual)| match (expected, actual) {
                (
                    NativeCarrierPath::List(NativeCarrierSelector::Dynamic),
                    NativeCarrierPath::List(NativeCarrierSelector::Static(_)),
                )
                | (
                    NativeCarrierPath::MapKey(NativeCarrierSelector::Dynamic),
                    NativeCarrierPath::MapKey(NativeCarrierSelector::Static(_)),
                )
                | (
                    NativeCarrierPath::MapValue(NativeCarrierSelector::Dynamic),
                    NativeCarrierPath::MapValue(NativeCarrierSelector::Static(_)),
                ) => true,
                _ => expected == actual,
            })
}

impl NativeResourceState {
    fn carrier_absence(&self, tree: &NativeCarrierTree) -> ResourceResult<AbsentCompanion> {
        match self.layout.carriers().node(tree.node)? {
            NativeCarrierNode::Optional { .. }
                if tree.selector == 0 && tree.children.is_empty() && tree.ordinary.is_none() =>
            {
                Ok(AbsentCompanion::None)
            }
            NativeCarrierNode::Result { fail, .. }
                if tree.selector == 0 && tree.children.len() == 1 && tree.ordinary.is_none() =>
            {
                let child = tree.children[0]
                    .as_ref()
                    .ok_or(NativeResourceError::WrongOperation)?;
                let NativeCarrierNode::Ordinary { shape } = self.layout.carriers().node(*fail)?
                else {
                    return Err(NativeResourceError::UnsupportedCarrierCustody);
                };
                if child.node != *fail || !child.children.is_empty() {
                    return Err(NativeResourceError::WrongFamily);
                }
                let (actual, bits) = child.ordinary.ok_or(NativeResourceError::WrongFamily)?;
                if actual != *shape {
                    return Err(NativeResourceError::WrongFamily);
                }
                Ok(AbsentCompanion::Fail {
                    shape: *shape,
                    bits,
                })
            }
            NativeCarrierNode::Optional { .. } | NativeCarrierNode::Result { .. } => {
                Err(NativeResourceError::UnsupportedCarrierCustody)
            }
            _ => Err(NativeResourceError::WrongFamily),
        }
    }

    /// This checks the private origin as well as the public legacy sum metadata.
    /// An ended adapter remains sealed, but cannot be used as a live view.
    pub(super) fn carrier_adapter_validate(
        &self,
        shell: ResourceHandleId,
        sum_loan: ResourceHandleId,
    ) -> ResourceResult<()> {
        let origin = self
            .carrier_adapters
            .get(&shell)
            .ok_or(NativeResourceError::WrongOperation)?;
        if origin.sum_loan != sum_loan {
            return Err(NativeResourceError::WrongOperation);
        }
        let Some(NativeResourceEntry::SumLoan(sum_lease)) = self.handles.get(&sum_loan) else {
            return Err(NativeResourceError::InvalidHandle);
        };
        let Some(NativeResourceEntry::CarrierLoan(loan)) = self.handles.get(&origin.loan) else {
            return Err(NativeResourceError::InvalidHandle);
        };
        let Some(NativeResourceEntry::CarrierRoot(root)) = self.handles.get(&loan.root) else {
            return Err(NativeResourceError::InvalidHandle);
        };
        let Some(NativeResourceEntry::Sum(sum)) = self.handles.get(&shell) else {
            return Err(NativeResourceError::InvalidHandle);
        };
        self.frame(loan.frame)?;
        self.frame(root.frame)?;
        if loan.generation != root.generation
            || root.generation != loan.root.raw()
            || loan.operation != origin.operation
            || loan.incoming.is_some()
            || sum_lease.shell != shell
            || sum_lease.token.is_some()
            || sum_lease.frame != loan.frame
            || sum.frame != loan.frame
            || sum_lease.source_frame != sum.frame
            || sum_lease.source_slot != sum.slot
            || sum_lease.shape != sum.shape
            || sum_lease.borrow_operation != origin.operation
        {
            return Err(NativeResourceError::WrongOperation);
        }
        if let Some(parent) = loan.parent {
            let Some(NativeResourceEntry::CarrierLoan(parent)) = self.handles.get(&parent) else {
                return Err(NativeResourceError::InvalidHandle);
            };
            self.frame(parent.frame)?;
            if parent.root != loan.root
                || parent.generation != loan.generation
                || !loan.path.starts_with(&parent.path)
            {
                return Err(NativeResourceError::WrongOperation);
            }
        }
        let NativeOperation::Carrier { record } = *self.operation(origin.operation)? else {
            return Err(NativeResourceError::WrongOperation);
        };
        let row = self
            .layout
            .carriers()
            .operations
            .get(usize::try_from(record).map_err(|_| NativeResourceError::WrongOperation)?)
            .ok_or(NativeResourceError::WrongOperation)?;
        let NativeCarrierOperation::AdaptSum {
            frame: template,
            source,
            path,
            sum_slot,
            lease_frame,
            borrowed: true,
        } = &row.operation
        else {
            return Err(NativeResourceError::WrongOperation);
        };
        // A caller lease remains valid while its callee is the active frame.
        // Checking an origin must not require the issuing frame to be on top.
        if self.layout.wire_version() != 3
            || row.site
                != self.layout.operations()[usize::try_from(origin.operation)
                    .map_err(|_| NativeResourceError::WrongOperation)?]
                .site()
            || sum.slot != *sum_slot
            || template != lease_frame
            || *lease_frame != self.frame(loan.frame)?.template
        {
            return Err(NativeResourceError::WrongOperation);
        }
        let tail = match source {
            NativeCarrierSource::Slot(slot) => {
                if root.slot != *slot || loan.parent.is_some() {
                    return Err(NativeResourceError::WrongOperation);
                }
                loan.path.as_slice()
            }
            NativeCarrierSource::Loan(source) => {
                let parent_handle = loan.parent.ok_or(NativeResourceError::WrongOperation)?;
                let Some(NativeResourceEntry::CarrierLoan(parent)) =
                    self.handles.get(&parent_handle)
                else {
                    return Err(NativeResourceError::InvalidHandle);
                };
                match source {
                    NativeCarrierLoanSource::ExistingBorrow { operation } => {
                        if parent.operation != *operation || parent.incoming.is_some() {
                            return Err(NativeResourceError::WrongOperation);
                        }
                    }
                    NativeCarrierLoanSource::IncomingViewFormal { scope, parameter } => {
                        let Some((actual_scope, actual_parameter, call)) = parent.incoming else {
                            return Err(NativeResourceError::WrongOperation);
                        };
                        let active_call = self.source_call(call)?;
                        if actual_scope != *scope
                            || actual_parameter != *parameter
                            || self.frame(parent.frame)?.template != *scope
                            || active_call.scope != Some(parent.frame)
                            || active_call.phase != source::SourcePhase::Entered
                        {
                            return Err(NativeResourceError::WrongOperation);
                        }
                    }
                }
                loan.path
                    .get(parent.path.len()..)
                    .ok_or(NativeResourceError::WrongOperation)?
            }
        };
        if !adapter_projection_matches(path, tail) {
            return Err(NativeResourceError::WrongOperation);
        }
        let tree = selected(&root.tree, &loan.path)?;
        if !self.layout.carrier_equivalent_sum(tree.node, sum.shape) {
            return Err(NativeResourceError::WrongFamily);
        }
        match (self.carrier_absence(tree)?, &sum.payload) {
            (AbsentCompanion::None, NativeSumPayload::None) => Ok(()),
            (
                AbsentCompanion::Fail { shape, bits },
                NativeSumPayload::Fail {
                    shape: actual,
                    string,
                },
            ) if shape == *actual && bits == *string => Ok(()),
            _ => Err(NativeResourceError::WrongFamily),
        }
    }

    pub(super) fn carrier_adapter_source(
        &self,
        frame: ResourceHandleId,
        handle: ResourceHandleId,
        operation: u32,
    ) -> ResourceResult<()> {
        let Some(NativeResourceEntry::SumLoan(loan)) = self.handles.get(&handle) else {
            return Err(NativeResourceError::WrongFamily);
        };
        if self.layout.wire_version() != 3 || loan.borrow_operation != operation {
            return Err(NativeResourceError::WrongOperation);
        }
        self.carrier_adapter_validate(loan.shell, handle)?;
        if self.activation_frame(frame, self.frame(loan.frame)?.template)? != loan.frame {
            return Err(NativeResourceError::WrongFrame);
        }
        Ok(())
    }

    pub(super) fn carrier_adapt(
        &mut self,
        ordinary: &mut values::NativeValues,
        frame: ResourceHandleId,
        operation: u32,
        handle: ResourceHandleId,
        dynamic: u64,
    ) -> ResourceResult<ResourceHandleId> {
        self.running(ordinary)?;
        let NativeCarrierOperation::AdaptSum {
            source,
            path,
            sum_slot,
            lease_frame,
            borrowed,
            ..
        } = self.carrier_operation(operation, frame)?
        else {
            return Err(NativeResourceError::WrongOperation);
        };
        if lease_frame != self.frame(frame)?.template {
            return Err(NativeResourceError::WrongFrame);
        }
        // A borrowed adapter's shell is owned solely by this lease frame.
        self.slot_empty(frame, sum_slot)?;
        let shape = self.layout.slots()
            [usize::try_from(sum_slot).map_err(|_| NativeResourceError::WrongOperation)?]
        .shape();
        let (root_handle, generation, full_path, parent) =
            self.carrier_location(ordinary, frame, handle, source, &path, dynamic)?;
        let Some(NativeResourceEntry::CarrierRoot(root)) = self.handles.get(&root_handle) else {
            return Err(NativeResourceError::WrongFamily);
        };
        let tree = selected(&root.tree, &full_path)?;
        if !self.layout.carrier_equivalent_sum(tree.node, shape) {
            return Err(NativeResourceError::WrongFamily);
        }
        let companion = self.carrier_absence(tree)?;
        if let AbsentCompanion::Fail { shape, bits } = companion {
            ordinary
                .validate_resource_ordinary(bits, &self.layout, shape)
                .map_err(ordinary_error)?;
        }
        if borrowed {
            self.carrier_adapters
                .try_reserve(1)
                .map_err(|_| NativeResourceError::Capacity)?;
            let mut ids = self.reserve(3)?;
            let shell = next_handle(&mut ids)?;
            let sum_loan = next_handle(&mut ids)?;
            let carrier_loan = next_handle(&mut ids)?;
            // The only owned companion copied by this bridge is an exact String.
            let payload = match companion {
                AbsentCompanion::None => NativeSumPayload::None,
                AbsentCompanion::Fail { shape, bits } => NativeSumPayload::Fail {
                    shape,
                    string: ordinary
                        .clone_resource_string_companion(&self.layout, shape, bits)
                        .map_err(ordinary_error)?,
                },
            };
            self.handles.insert(
                carrier_loan,
                NativeResourceEntry::CarrierLoan(NativeCarrierLoan {
                    frame,
                    operation,
                    root: root_handle,
                    generation,
                    path: full_path,
                    parent,
                    incoming: None,
                }),
            );
            self.handles.insert(
                shell,
                NativeResourceEntry::Sum(NativeResourceSum {
                    frame,
                    slot: sum_slot,
                    shape,
                    payload,
                }),
            );
            self.handles.insert(
                sum_loan,
                NativeResourceEntry::SumLoan(NativeSumLoan {
                    frame,
                    borrow_operation: operation,
                    shell,
                    source_frame: frame,
                    source_slot: sum_slot,
                    shape,
                    token: None,
                }),
            );
            self.carrier_adapters.insert(
                shell,
                NativeCarrierSumOrigin {
                    loan: carrier_loan,
                    operation,
                    sum_loan,
                },
            );
            Ok(sum_loan)
        } else {
            if !matches!(source, NativeCarrierSource::Slot(_)) || parent.is_some() {
                return Err(NativeResourceError::WrongOperation);
            }
            self.carrier_unborrowed(root_handle)?;
            let mut ids = self.reserve(1)?;
            let shell = next_handle(&mut ids)?;
            if full_path.is_empty() {
                let Some(NativeResourceEntry::CarrierRoot(_)) = self.handles.remove(&root_handle)
                else {
                    return Err(NativeResourceError::WrongFamily);
                };
            } else {
                let Some(NativeResourceEntry::CarrierRoot(root)) =
                    self.handles.get_mut(&root_handle)
                else {
                    return Err(NativeResourceError::WrongFamily);
                };
                let _tree = take_selected(&mut root.tree, &full_path)?;
            }
            let payload = match companion {
                AbsentCompanion::None => NativeSumPayload::None,
                AbsentCompanion::Fail { shape, bits } => NativeSumPayload::Fail {
                    shape,
                    string: bits,
                },
            };
            self.handles.insert(
                shell,
                NativeResourceEntry::Sum(NativeResourceSum {
                    frame,
                    slot: sum_slot,
                    shape,
                    payload,
                }),
            );
            Ok(shell)
        }
    }
}

#[cfg(test)]
#[path = "carriers_sum_tests.rs"]
mod tests;
