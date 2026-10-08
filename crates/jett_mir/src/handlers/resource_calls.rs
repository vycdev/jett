//! Canonical constructor extraction; no public MIR graph may enter this path.
use super::*;

impl Builder<'_> {
    fn resource_site(&self) -> ResourceSite {
        self.resource_capture
            .site(
                self.current,
                self.blocks[self.current.index() as usize].statements.len(),
            )
            .expect("authenticated Resource constructor")
    }

    pub(super) fn lower_resource_call(
        &mut self,
        original: &Expression,
    ) -> Result<Expression, String> {
        let checkpoint = self.clone();
        match self.try_lower_resource_call(original) {
            Ok(value) => Ok(value),
            Err(error) => {
                *self = checkpoint;
                Err(error)
            }
        }
    }

    fn try_lower_resource_call(&mut self, original: &Expression) -> Result<Expression, String> {
        let ExpressionKind::Call {
            args,
            evaluation_order,
            ownership: hir::CallOwnership::Source(source),
            ..
        } = &original.kind
        else {
            return Err("pending Resource call region: handled indirect/resource-hook invocation timing is unproved".into());
        };
        // Authenticate the original tuple before any extraction or generated Local.
        let begin = self.resource_site();
        let region = self.resource_capture.begin_normalized_call(
            original,
            self.resource_call_scopes.last().copied(),
            begin,
            self.types,
        )?;
        self.push(
            StatementKind::ResourceCall(ResourceCallNode::Begin { region }),
            original.span,
        );
        self.resource_call_scopes.push(region);
        for (source_index, &parameter) in evaluation_order.iter().enumerate() {
            let actual = args
                .get(parameter)
                .ok_or("Resource call source permutation is invalid")?;
            let fact = source
                .arguments
                .iter()
                .find(|fact| fact.parameter_index == parameter)
                .ok_or("Resource call actual lost its original formal")?;
            if fact.source_index != source_index {
                return Err("Resource call actual changes its checked lexical order".into());
            }
            let start = self.resource_site();
            let first_local = self.locals.len();
            let mut unsupported = false;
            crate::resource_ownership::visit_expression(actual, &mut |value| {
                unsupported |= matches!(
                    value.kind,
                    ExpressionKind::Handle {
                        kind: HandleKind::Refinement { .. },
                        ..
                    }
                );
            });
            if unsupported {
                return Err(
                    "pending Resource call region: refinement handlers need their exact exit proof"
                        .into(),
                );
            }
            let endpoint = self.lower_value(actual);
            if let Some(error) = &self.resource_error {
                return Err(error.message.clone());
            }
            let ordinary = if crate::resource_type_pending(self.types, endpoint.ty) {
                None
            } else {
                Some(self.temporary(endpoint.ty, endpoint.span))
            };
            let stage = self.resource_site();
            self.resource_capture.normalized_actual(
                region,
                source_index,
                parameter,
                actual,
                &endpoint,
                start,
                stage,
                &self.locals[first_local..],
                ordinary,
            )?;
            self.push(
                StatementKind::ResourceCall(ResourceCallNode::Stage {
                    region,
                    source_index,
                    parameter,
                    value: endpoint,
                    ordinary,
                }),
                actual.span,
            );
        }
        let output = self.temporary(original.ty, original.span);
        self.push(
            StatementKind::ResourceCall(ResourceCallNode::Invoke { region, output }),
            original.span,
        );
        self.push(
            StatementKind::ResourceCall(ResourceCallNode::End {
                region,
                outcome: ResourceCompletion::Normal,
            }),
            original.span,
        );
        if self.resource_call_scopes.pop() != Some(region) {
            return Err("Resource call constructor lost its exact operation stack".into());
        }
        Ok(Expression {
            kind: ExpressionKind::Local(output),
            ty: original.ty,
            span: original.span,
        })
    }

    /// Suspend only the active operation suffix before a real Source Return read.
    pub(crate) fn abandon_resource_calls(&mut self, span: Span) -> Vec<ResourceCallRegionId> {
        let scopes = std::mem::take(&mut self.resource_call_scopes);
        for &region in scopes.iter().rev() {
            self.push(
                StatementKind::ResourceCall(ResourceCallNode::End {
                    region,
                    outcome: ResourceCompletion::Abort,
                }),
                span,
            );
        }
        scopes
    }
}
