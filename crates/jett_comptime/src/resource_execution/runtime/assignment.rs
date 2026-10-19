//! A declared slot can outlive its current owner; replacement never copies one.
use super::super::checked::CheckedAssignment;
use super::*;
use jett_parser::ast::AssignStmt;

pub(crate) struct PreparedResourceAssignment<'source> {
    checked: CheckedAssignment<'source>,
    scope_index: usize,
    frame: FrameId,
}

impl PreparedResourceAssignment<'_> {
    pub(crate) fn scope_index(&self) -> usize {
        self.scope_index
    }
}

impl ResourceTransport {
    pub(crate) fn prepare_resource_assignment<'source>(
        &self,
        source: &'source AssignStmt,
    ) -> Result<PreparedResourceAssignment<'source>, ResourceExecutionError> {
        let checked = self.checked.prepare_assignment(source)?;
        let fact = checked.fact();
        let scope_index = self
            .scopes
            .iter()
            .rposition(|(_, bindings)| bindings.contains_key(&fact.definition))
            .ok_or(ResourceExecutionError::InvalidOwner)?;
        let (frame, bindings) = &self.scopes[scope_index];
        let slot = bindings
            .get(&fact.definition)
            .ok_or(ResourceExecutionError::InvalidOwner)?;
        if slot.fact != fact || !self.ledger.frame_is_live(*frame) {
            return Err(ResourceExecutionError::InvalidOwner);
        }
        Ok(PreparedResourceAssignment {
            checked,
            scope_index,
            frame: *frame,
        })
    }

    fn assignment_slot_mut(
        &mut self,
        destination: &PreparedResourceAssignment<'_>,
    ) -> Result<&mut ResourceBindingSlot, ResourceExecutionError> {
        let fact = destination.checked.fact();
        let (frame, bindings) = self
            .scopes
            .get_mut(destination.scope_index)
            .ok_or(ResourceExecutionError::InvalidFrame)?;
        if *frame != destination.frame || !self.ledger.frame_is_live(*frame) {
            return Err(ResourceExecutionError::InvalidFrame);
        }
        let slot = bindings
            .get_mut(&fact.definition)
            .ok_or(ResourceExecutionError::InvalidOwner)?;
        if slot.fact != fact {
            return Err(ResourceExecutionError::InvalidOwner);
        }
        Ok(slot)
    }

    pub(crate) fn replace_resource_assignment(
        &mut self,
        destination: PreparedResourceAssignment<'_>,
        mut value: EvaluatedValue,
    ) -> Result<(), ResourceExecutionError> {
        self.checked.revalidate_assignment(&destination.checked)?;
        self.assignment_slot_mut(&destination)?;
        let fact = destination.checked.fact();
        self.validate_value_type(&value, fact.ty)?;
        if value.custody.has_borrows() {
            return Err(ResourceExecutionError::InvalidOwner);
        }
        // Keep the new owner in the active RHS operation until old-value
        // cleanup completes. A finalizer panic leaves both cleanup paths live.
        self.hold_actual(&mut value)?;
        let previous = self.assignment_slot_mut(&destination)?.value.take();
        if let Some(previous) = previous {
            self.discard_value(previous)?;
        }
        self.ledger.transfer(
            &mut value.custody,
            OwnerHolder::Binding {
                frame: destination.frame,
                definition: fact.definition,
            },
        )?;
        self.assignment_slot_mut(&destination)?.value = Some(value);
        Ok(())
    }
}
