//! Checked occupied constructor staging; a general public MIR reseal cannot enter it.
use super::*;
use crate::resource_ownership::constructors::ResourceOperationRegion;

impl Builder<'_> {
    pub(super) fn lower_resource_constructor(
        &mut self,
        original: &Expression,
    ) -> Result<Expression, String> {
        let checkpoint = self.clone();
        match self.try_lower_resource_constructor(original) {
            Ok(v) => Ok(v),
            Err(e) => {
                *self = checkpoint;
                Err(e)
            }
        }
    }

    fn try_lower_resource_constructor(
        &mut self,
        original: &Expression,
    ) -> Result<Expression, String> {
        let ExpressionKind::StructConstruct {
            fields,
            evaluation_order,
            ..
        } = &original.kind
        else {
            return Err("Resource constructor changed its original node".into());
        };
        let begin = self.resource_site();
        let constructor = self.resource_capture.begin_constructor_region(
            original,
            self.resource_operation_scopes.last().copied(),
            begin,
            self.types,
        )?;
        self.push(
            StatementKind::ResourceConstructor(ResourceConstructorNode::Begin { constructor }),
            original.span,
        );
        self.resource_constructor_scopes.push(constructor);
        self.resource_operation_scopes
            .push(ResourceOperationRegion::Constructor(constructor));
        for (source_index, &field) in evaluation_order.iter().enumerate() {
            let start = self.resource_site();
            let first_local = self.locals.len();
            let actual = fields
                .get(field)
                .ok_or("Resource constructor has a foreign field permutation")?;
            let endpoint = self.lower_value(actual);
            if let Some(error) = &self.resource_error {
                return Err(error.message.clone());
            }
            let stage = self.resource_site();
            self.resource_capture.constructor_field(
                constructor,
                source_index,
                field,
                actual,
                &endpoint,
                start,
                stage,
                &self.locals[first_local..],
            )?;
            self.push(
                StatementKind::ResourceConstructor(ResourceConstructorNode::Stage {
                    constructor,
                    source_index,
                    field,
                    value: endpoint,
                }),
                actual.span,
            );
        }
        let output = self.temporary(original.ty, original.span);
        self.push(
            StatementKind::ResourceConstructor(ResourceConstructorNode::Commit {
                constructor,
                output,
            }),
            original.span,
        );
        self.push(
            StatementKind::ResourceConstructor(ResourceConstructorNode::End {
                constructor,
                outcome: ResourceCompletion::Normal,
            }),
            original.span,
        );
        if self.resource_constructor_scopes.pop() != Some(constructor)
            || self.resource_operation_scopes.pop()
                != Some(ResourceOperationRegion::Constructor(constructor))
        {
            return Err("Resource constructor lost its exact active operation suffix".into());
        }
        Ok(Expression {
            kind: ExpressionKind::Local(output),
            ty: original.ty,
            span: original.span,
        })
    }
}
