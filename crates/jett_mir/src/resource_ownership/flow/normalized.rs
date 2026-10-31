//! Cross-block normalized Source operation proof. Every identity is constructor sealed.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ActiveCall {
    region: ResourceCallRegionId,
    pub(super) frame: ResourceFrameId,
    parent: ResourceFrameId,
    next: usize,
    loans: Vec<ResourceLoanId>,
    owners: Vec<(ResourceOwnerSlotId, ResourceOccupancy)>,
    invoked: bool,
}

#[derive(Clone)]
pub(super) struct PreparedCall {
    frame: ResourceFrameId,
    operands: Vec<ResourceCallOperand>,
    result: ResourceCallResult,
    created_loans: Vec<ResourceLoanId>,
}

impl Analysis<'_> {
    fn prepare_normalized(
        &mut self,
        state: &State,
        region: &ResourceCallRegion,
    ) -> Result<PreparedCall, String> {
        if let Some(prepared) = self.normalized_calls.get(&region.id()) {
            return Ok(prepared.clone());
        }
        let frame = self.frame(0, ResourceFrameRole::Operation);
        let begin = self.site;
        let mut operands = Vec::new();
        let mut created_loans = Vec::new();
        for actual in region.actuals() {
            let fact = region
                .source()
                .arguments
                .iter()
                .find(|fact| fact.parameter_index == actual.parameter())
                .ok_or("Resource normalized actual has no exact original Source tuple")?;
            self.site = actual.stage();
            let Some(shape) = shape(
                &self.program.resource_manifest,
                self.types,
                actual.endpoint().ty,
            )?
            else {
                if actual.ordinary().is_none() {
                    return Err(
                        "Resource normalized ordinary actual lacks its exact initialized endpoint"
                            .into(),
                    );
                }
                operands.push(ResourceCallOperand::Ordinary {
                    parameter: actual.parameter(),
                    ty: actual.endpoint().ty,
                });
                continue;
            };
            if actual.ordinary().is_some() || fact.staging != hir::ArgumentStaging::Original {
                return Err(
                    "Resource normalized actual gained ordinary storage or staging authority"
                        .into(),
                );
            }
            match fact.effect {
                Effect::TransferOwned | Effect::RelinquishOwned => {
                    if fact.syntax != Syntax::Bare {
                        return Err(
                            "Resource normalized owned actual changes its bare Source syntax"
                                .into(),
                        );
                    }
                    let slot = self.argument_slot(0, actual.parameter(), shape, frame);
                    if fact.callee_access == Access::View {
                        let loan = self.loan(
                            0,
                            actual.parameter(),
                            frame,
                            ResourceLoanSource::Owner(slot),
                        )?;
                        created_loans.push(loan);
                        operands.push(ResourceCallOperand::Borrowed {
                            parameter: actual.parameter(),
                            loan,
                        });
                    } else {
                        operands.push(ResourceCallOperand::Owned {
                            parameter: actual.parameter(),
                            slot,
                        });
                    }
                }
                Effect::RetainBorrow => {
                    if fact.syntax != Syntax::WrittenView || fact.callee_access != Access::View {
                        return Err(
                            "Resource normalized retained actual changed its written View tuple"
                                .into(),
                        );
                    }
                    let E::View(inner) = &actual.endpoint().kind else {
                        return Err("pending Resource call region: retained Resource expression needs a proved stable place endpoint".into());
                    };
                    let E::Local(local) = inner.kind else {
                        return Err("pending Resource call region: retained Resource expression needs its exact Local holder".into());
                    };
                    let value = state
                        .values
                        .get(local.index() as usize)
                        .and_then(Option::as_ref)
                        .ok_or(
                            "Resource normalized retained holder is not live at its initial region",
                        )?;
                    let loan = if let Some(loan) = value.loan {
                        loan
                    } else {
                        let owner = value
                            .owner
                            .ok_or("Resource normalized retained holder lacks its exact owner")?;
                        let loan = self.loan(
                            0,
                            actual.parameter(),
                            frame,
                            ResourceLoanSource::Owner(owner),
                        )?;
                        created_loans.push(loan);
                        loan
                    };
                    operands.push(ResourceCallOperand::Borrowed {
                        parameter: actual.parameter(),
                        loan,
                    });
                }
                Effect::Copy | Effect::ObserveData => {
                    return Err("Resource normalized custody cannot use Copy or ObserveData".into());
                }
            }
        }
        self.site = begin;
        let (_, output) = region
            .invocation()
            .ok_or("Resource normalized Source invocation lacks its exact output")?;
        let result = match shape(
            &self.program.resource_manifest,
            self.types,
            region.original().ty,
        )? {
            Some(_) => ResourceCallResult::Owned {
                slot: self.local_slots[output.index() as usize]
                    .ok_or("Resource normalized output lacks its exact dedicated slot")?,
            },
            None => ResourceCallResult::Ordinary {
                ty: region.original().ty,
            },
        };
        let prepared = PreparedCall {
            frame,
            operands,
            result,
            created_loans,
        };
        self.normalized_calls.insert(region.id(), prepared.clone());
        Ok(prepared)
    }

    pub(super) fn normalized_node(
        &mut self,
        state: &mut State,
        node: &ResourceCallNode,
    ) -> Result<(), String> {
        let region = self
            .function
            .resource_call_region(node.region())
            .ok_or("Resource normalized node has no constructor-owned region")?
            .clone();
        match node {
            ResourceCallNode::Begin { .. } => {
                if state.calls.last().map(|active| active.region) != region.parent() {
                    return Err(
                        "Resource normalized Begin changed its exact parent operation stack".into(),
                    );
                }
                let prepared = self.prepare_normalized(state, &region)?;
                let parent = self.active_frame;
                if self.plan.frames[prepared.frame.index()].parent != Some(parent) {
                    return Err("Resource normalized Begin changed its frame parent".into());
                }
                self.operation(
                    prepared.frame,
                    ResourceOperationRole::BeginSourceFunction {
                        region: region.id(),
                        function: region.function(),
                        source: region.source().clone(),
                        formals: region
                            .source()
                            .arguments
                            .iter()
                            .map(ResourceCallFormal::original)
                            .collect(),
                        evaluation_order: region.evaluation_order().to_vec(),
                        operands: prepared.operands.clone(),
                        result: prepared.result,
                    },
                );
                for &loan in &prepared.created_loans {
                    let parameter = self.plan.loans[loan.index()]
                        .parameter
                        .ok_or("Resource prepared Source loan lost its parameter")?;
                    let role = if self.plan.loans[loan.index()].shape.conditional() {
                        ResourceOperationRole::PrepareSourceSumBorrow { loan, parameter }
                    } else {
                        ResourceOperationRole::PrepareSourceBorrow { loan, parameter }
                    };
                    self.operation(prepared.frame, role);
                }
                state.calls.push(ActiveCall {
                    region: region.id(),
                    frame: prepared.frame,
                    parent,
                    next: 0,
                    loans: Vec::new(),
                    owners: Vec::new(),
                    invoked: false,
                });
                self.active_frame = prepared.frame;
            }
            ResourceCallNode::Stage {
                source_index,
                parameter,
                value,
                ordinary,
                ..
            } => {
                let mut active = state
                    .calls
                    .pop()
                    .ok_or("Resource normalized Stage has no active operation")?;
                if active.region != region.id()
                    || active.next != *source_index
                    || active.invoked
                    || region.evaluation_order().get(*source_index) != Some(parameter)
                {
                    return Err("Resource normalized Stage changed its exact arrived prefix".into());
                }
                let prepared = self
                    .normalized_calls
                    .get(&region.id())
                    .ok_or("Resource normalized Stage lacks its prepared call")?
                    .clone();
                let operand = prepared
                    .operands
                    .get(*source_index)
                    .ok_or("Resource normalized Stage lost its formal operand")?
                    .clone();
                let fact = region
                    .source()
                    .arguments
                    .iter()
                    .find(|fact| fact.parameter_index == *parameter)
                    .ok_or("Resource normalized Stage lost its Source fact")?;
                let retaining = fact.effect == Effect::RetainBorrow;
                let input = if !retaining && fact.syntax == Syntax::Bare {
                    match &value.kind {
                        E::View(inner) => inner.as_ref(),
                        _ => value,
                    }
                } else {
                    value
                };
                let evaluated = self.expression(state, input, !retaining)?;
                if state.aborted {
                    return Err("pending Resource call region: infrastructure abort within a staged actual needs its exact prefix exit".into());
                }
                match &operand {
                    ResourceCallOperand::CarrierOwned { .. }
                    | ResourceCallOperand::CarrierBorrowed { .. } => {
                        return Err(
                            "carrier operand reached the legacy single-leaf normalized flow".into(),
                        );
                    }
                    ResourceCallOperand::Ordinary { ty, .. } => {
                        if evaluated.is_some()
                            || ordinary
                                .and_then(|local| self.function.local(local))
                                .map(|header| header.ty)
                                != Some(*ty)
                        {
                            return Err("Resource normalized ordinary endpoint changed its exact type/header".into());
                        }
                    }
                    ResourceCallOperand::Owned { .. } | ResourceCallOperand::Borrowed { .. }
                        if !retaining =>
                    {
                        let value = evaluated
                            .ok_or("Resource normalized Stage has no live owning producer")?;
                        if value.loan.is_some()
                            || (!value.shape.conditional()
                                && value.occupancy != ResourceOccupancy::Occupied)
                        {
                            return Err(
                                "Resource normalized owned Stage is not its exact occupied owner"
                                    .into(),
                            );
                        }
                        let slot = match &operand {
                            ResourceCallOperand::CarrierOwned { .. }
                            | ResourceCallOperand::CarrierBorrowed { .. } => return Err(
                                "carrier operand reached the legacy single-leaf normalized flow"
                                    .into(),
                            ),
                            ResourceCallOperand::Owned { slot, .. } => *slot,
                            ResourceCallOperand::Borrowed { loan, .. } => {
                                match self.plan.loans[loan.index()].source {
                                    ResourceLoanSource::Owner(slot) => slot,
                                    ResourceLoanSource::IncomingViewFormal { .. }
                                    | ResourceLoanSource::CarrierSumProjection { .. }
                                    | ResourceLoanSource::ProjectedSumPayload { .. } => return Err(
                                        "Resource owned Stage changed into a resident formal loan"
                                            .into(),
                                    ),
                                }
                            }
                            ResourceCallOperand::Ordinary { .. } => {
                                return Err("Resource owned Stage lost its custody operand".into());
                            }
                        };
                        if value.shape != self.plan.slots[slot.index()].shape {
                            return Err(
                                "Resource normalized Stage changes its sealed owner shape".into()
                            );
                        }
                        self.operation(
                            active.frame,
                            ResourceOperationRole::Transfer {
                                source: value.owner.ok_or(
                                    "Resource normalized Stage lacks an exact source owner",
                                )?,
                                destination: slot,
                            },
                        );
                        active.owners.push((slot, value.occupancy));
                        if let ResourceCallOperand::Borrowed { loan, .. } = operand {
                            state.leases.insert(loan);
                            active.loans.push(loan);
                            self.borrow(active.frame, loan);
                        }
                    }
                    ResourceCallOperand::Borrowed { loan, .. } => {
                        let value = evaluated
                            .ok_or("Resource normalized retained Stage lost its holder")?;
                        if value.shape != self.plan.loans[loan.index()].shape {
                            return Err(
                                "Resource normalized retained Stage changes its sealed loan shape"
                                    .into(),
                            );
                        }
                        if let Some(resident) = value.loan {
                            if resident != *loan {
                                return Err(
                                    "Resource normalized retained Stage changed its forwarded loan"
                                        .into(),
                                );
                            }
                            self.operation(
                                active.frame,
                                ResourceOperationRole::BoundedBorrowUse {
                                    loan: *loan,
                                    parameter: *parameter,
                                },
                            );
                        } else {
                            if value.owner.map(ResourceLoanSource::Owner)
                                != Some(self.plan.loans[loan.index()].source)
                            {
                                return Err("Resource normalized retained Stage changed its original holder".into());
                            }
                            state.leases.insert(*loan);
                            active.loans.push(*loan);
                            self.borrow(active.frame, *loan);
                        }
                    }
                    ResourceCallOperand::Owned { .. } => {
                        return Err(
                            "Resource normalized retained Stage cannot own its formal".into()
                        );
                    }
                }
                self.operation(
                    active.frame,
                    ResourceOperationRole::StageSourceActual {
                        region: region.id(),
                        source_index: *source_index,
                        parameter: *parameter,
                        operand,
                        ordinary: *ordinary,
                    },
                );
                active.next += 1;
                state.calls.push(active);
            }
            ResourceCallNode::Invoke { output, .. } => {
                let active = state
                    .calls
                    .last_mut()
                    .ok_or("Resource normalized Invoke lacks its active operation")?;
                if active.region != region.id()
                    || active.invoked
                    || active.next != region.evaluation_order().len()
                {
                    return Err(
                        "Resource normalized Invoke has an incomplete or reused actual prefix"
                            .into(),
                    );
                }
                active.invoked = true;
                let frame = active.frame;
                let prepared = self
                    .normalized_calls
                    .get(&region.id())
                    .ok_or("Resource normalized Invoke lost its prepared tuple")?
                    .clone();
                self.operation(
                    frame,
                    ResourceOperationRole::InvokeSourceFunction {
                        function: region.function(),
                        formals: region
                            .source()
                            .arguments
                            .iter()
                            .map(ResourceCallFormal::original)
                            .collect(),
                        source: region.source().clone(),
                        evaluation_order: region.evaluation_order().to_vec(),
                        operands: prepared.operands,
                        result: prepared.result,
                    },
                );
                let value = shape(
                    &self.program.resource_manifest,
                    self.types,
                    region.original().ty,
                )?
                .map(|shape| -> Result<Value, String> {
                    Ok(Value {
                        occupancy: if shape.conditional() {
                            ResourceOccupancy::Conditional
                        } else {
                            ResourceOccupancy::Occupied
                        },
                        shape,
                        owner: match prepared.result {
                            ResourceCallResult::Owned { slot } => Some(slot),
                            ResourceCallResult::Ordinary { .. } => None,
                            ResourceCallResult::Carrier { .. } => {
                                return Err(
                                    "carrier result reached the legacy single-leaf normalized flow"
                                        .into(),
                                );
                            }
                        },
                        loan: None,
                    })
                })
                .transpose()?;
                self.store(state, *output, value, false, false)?;
            }
            ResourceCallNode::End { outcome, .. } => {
                let active = state
                    .calls
                    .pop()
                    .ok_or("Resource normalized End lacks its active operation")?;
                if active.region != region.id()
                    || (*outcome == ResourceCompletion::Normal) != active.invoked
                    || *outcome == ResourceCompletion::Return
                {
                    return Err(
                        "Resource normalized End changed its exact invoked/abandoned outcome"
                            .into(),
                    );
                }
                for loan in active.loans.iter().rev() {
                    state.leases.remove(loan);
                    self.end_borrow(active.frame, *loan);
                }
                let prepared = self
                    .normalized_calls
                    .get(&region.id())
                    .ok_or("Resource normalized End lost its exact prepared operands")?;
                let owners = active.owners.iter().rev().filter(|(slot, _)| *outcome == ResourceCompletion::Abort || prepared.operands.iter().any(|operand| matches!(operand, ResourceCallOperand::Borrowed { loan, .. } if self.plan.loans[loan.index()].source == ResourceLoanSource::Owner(*slot)))).copied().collect::<Vec<_>>();
                for (source, occupancy) in owners {
                    self.operation(
                        active.frame,
                        ResourceOperationRole::Drop { source, occupancy },
                    );
                }
                self.operation(
                    active.frame,
                    ResourceOperationRole::Complete { outcome: *outcome },
                );
                self.active_frame = active.parent;
            }
        }
        Ok(())
    }
}
