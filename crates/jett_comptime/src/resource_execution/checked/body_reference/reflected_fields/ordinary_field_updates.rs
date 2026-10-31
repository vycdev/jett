//! Ordinary field scope identity reuses genuine reflected loop/iteration proofs.
//! This creates no Resource entry, provider, ledger, or custody authority.
use super::*;

impl CheckedExecution {
    pub(crate) fn prepare_field_reflected_loop(
        &self,
        source: &ForStmt,
    ) -> Result<Option<Arc<PreparedReflectedFieldLoop>>, ResourceExecutionError> {
        self.validate_field_executable_cursor(&self.body)?;
        let parent = CheckedBodyReference {
            cursor: self.body.clone(),
        };
        let original = original_for(parent.region()?, source.span)?;
        if !std::ptr::eq(original, source) {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        let Expr::GenericCall(_, source_types, arguments, occurrence) = &original.iterable else {
            return Ok(None);
        };
        let facts = self.facts(&self.body)?;
        if facts.intrinsics.get(occurrence) != Some(&IntrinsicId::TypeFields) {
            return Ok(None);
        }
        if source_types.len() != 1 || !arguments.is_empty() || original.value_variable.is_some() {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        // The retained For node supplies the exact physical call. Rejoin its
        // checked packet and concrete operands without Resource entry helpers,
        // which deliberately reject runtime InlineBody cursors.
        let invocation = self.invocation(*occurrence)?;
        let (intrinsic, types, reflections) = super::super::intrinsic::intrinsic_records(
            &facts,
            &invocation,
            source_types.len(),
            &self.program.checked().interner,
        )?;
        if intrinsic != IntrinsicId::TypeFields {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        let [owner] = types else {
            return Err(ResourceExecutionError::MissingCheckedBody);
        };
        let reflection = reflections
            .first()
            .ok_or(ResourceExecutionError::MissingCheckedBody)?;
        let fields = checked_owner_fields_for_reflection(self.program(), *owner, reflection)?;
        let mut variables = self
            .program
            .resolved()
            .scope_table
            .definitions
            .iter()
            .filter(|definition| {
                definition.kind == DefKind::Variable
                    && definition.span == original.variable.span
                    && definition.name == original.variable.name
            });
        let variable = variables
            .next()
            .ok_or(ResourceExecutionError::MissingCheckedBody)?;
        if variables.next().is_some() {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        let loop_span = original.span;
        Ok(Some(Arc::new(PreparedReflectedFieldLoop {
            parent,
            key: self.field_attempt_key(&self.body)?,
            loop_span,
            variable: variable.id,
            owner: *owner,
            fields,
        })))
    }

    pub(crate) fn prepare_field_reflected_scope(
        &self,
        bind: &ComptimeTypeBindStmt,
        iteration: &PreparedReflectedFieldIteration,
    ) -> Result<PreparedDirectScope, ResourceExecutionError> {
        let prepared = self.prepare_original_reflected_field_scope(bind, iteration)?;
        self.validate_field_executable_cursor(&prepared.reference.cursor)?;
        Ok(prepared)
    }
}
