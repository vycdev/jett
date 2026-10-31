//! Nonconsuming conditional Resource projections from the fresh current-CFG plan.
use super::*;

impl Translator<'_, '_> {
    pub(super) fn resource_borrowed_sum_statement(
        &mut self,
        statement: &Statement,
        operations: &[&jett_mir::ResourceOperation],
    ) -> Result<bool, CodegenError> {
        match &statement.kind {
            StatementKind::SumTag { target, .. } => {
                let mut matching = operations.iter().copied().filter(|operation|
                    matches!(operation.role(), Role::ObserveSumView { target: exact, .. } if exact == target));
                let Some(operation) = matching.next() else {
                    return Ok(false);
                };
                if matching.next().is_some() {
                    return Err(pending("sum view tag has ambiguous current authority"));
                }
                self.resource_sum_view_tag(operation, *target, statement.span)?;
                Ok(true)
            }
            StatementKind::SumTake {
                target, success, ..
            } => {
                let mut matching = operations.iter().copied().filter(|operation| {
                    if *success { matches!(operation.role(), Role::ProjectSumView { .. }) }
                    else { matches!(operation.role(), Role::ReadFailureCompanion { target: exact, .. } if exact.id == *target) }
                });
                let Some(operation) = matching.next() else {
                    return Ok(false);
                };
                if matching.next().is_some() {
                    return Err(pending(
                        "sum view projection has ambiguous current authority",
                    ));
                }
                self.resource_sum_view_projection(operation, *target, statement.span)?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }
    fn resource_sum_view_tag(
        &mut self,
        operation: &jett_mir::ResourceOperation,
        target: jett_mir::LocalId,
        span: Span,
    ) -> Result<(), CodegenError> {
        let resource = self
            .resource
            .ok_or_else(|| pending("sum view tag has no exact family"))?;
        let Role::ObserveSumView { source, .. } = operation.role() else {
            return Err(pending("sum view tag selected another role"));
        };
        let value = self.resource_loan(*source)?;
        let (context, frame, ordinal) = self.resource_operation(operation)?;
        let tag = Leaf::SumViewTag.output(
            self.module,
            self.builder,
            &[context, frame, ordinal, value],
            resource.failure,
            4,
        )?;
        let tag = self.builder.ins().ireduce(ir::types::I8, tag);
        self.define_local(target, LoweredValue::Scalar(tag), span)
    }
    fn resource_sum_view_projection(
        &mut self,
        operation: &jett_mir::ResourceOperation,
        target: jett_mir::LocalId,
        span: Span,
    ) -> Result<(), CodegenError> {
        let resource = self
            .resource
            .ok_or_else(|| pending("sum view extraction has no exact family"))?;
        let (source, leaf) = match operation.role() {
            Role::ProjectSumView { source, .. } => (*source, Leaf::SumViewProject),
            Role::ReadFailureCompanion { source, .. } => (*source, Leaf::SumFailureRead),
            _ => return Err(pending("sum view extraction selected another role")),
        };
        let value = self.resource_loan(source)?;
        let (context, frame, ordinal) = self.resource_operation(operation)?;
        let bits = leaf.output(
            self.module,
            self.builder,
            &[context, frame, ordinal, value],
            resource.failure,
            8,
        )?;
        let lowered = if let Role::ProjectSumView { destination, .. } = operation.role() {
            self.builder
                .ins()
                .stack_store(bits, resource.loans[destination.index()], 0);
            LoweredValue::Scalar(bits)
        } else {
            self.own(bits)?
        };
        self.define_local(target, lowered, span)
    }
}
