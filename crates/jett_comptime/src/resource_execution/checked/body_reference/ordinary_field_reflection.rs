//! Canonical owner data from one authenticated ordinary field intrinsic.
//! No returned TypeId grants Resource custody or refinement proof authority.
use super::*;
use jett_parser::ast::{PipelineStep, TypeExpr};

impl CheckedExecution {
    pub(crate) fn field_intrinsic_owner(
        &self,
        callee: &Expr,
        source_types: &[TypeExpr],
        source_arguments: &[CallArg],
        span: Span,
    ) -> Result<Option<TypeId>, ResourceExecutionError> {
        let facts = self.facts(&self.body)?;
        if !facts
            .intrinsics
            .get(&span)
            .copied()
            .is_some_and(field_intrinsic)
        {
            return Ok(None);
        }
        self.field_attempt_key(&self.body)?;
        let reference = CheckedBodyReference {
            cursor: self.body.clone(),
        };
        let mut count = 0;
        walk_region(
            reference.region()?,
            &mut |_| {},
            &mut |expression| {
                let (original, types, arguments, occurrence) = match expression {
                    Expr::Call(callee, arguments, span) => {
                        (callee.as_ref(), &[][..], arguments.as_slice(), *span)
                    }
                    Expr::GenericCall(callee, types, arguments, span) => (
                        callee.as_ref(),
                        types.as_slice(),
                        arguments.as_slice(),
                        *span,
                    ),
                    _ => return,
                };
                if occurrence == span
                    && std::ptr::eq(original, callee)
                    && same_slice(types, source_types)
                    && same_slice(arguments, source_arguments)
                {
                    count += 1;
                }
            },
            &mut |_| {},
        );
        if count != 1 {
            return Err(ResourceExecutionError::MissingCheckedInvocation);
        }
        self.field_intrinsic_owner_at(span, source_types.len())
    }

    pub(crate) fn field_pipeline_intrinsic_owner(
        &self,
        source: &PipelineStep,
    ) -> Result<Option<TypeId>, ResourceExecutionError> {
        let facts = self.facts(&self.body)?;
        if !facts
            .intrinsics
            .get(&source.span)
            .copied()
            .is_some_and(field_intrinsic)
        {
            return Ok(None);
        }
        self.field_attempt_key(&self.body)?;
        let reference = CheckedBodyReference {
            cursor: self.body.clone(),
        };
        let mut count = 0;
        walk_region(
            reference.region()?,
            &mut |_| {},
            &mut |expression| {
                if let Expr::Pipeline(_, steps, _) = expression {
                    count += steps
                        .iter()
                        .filter(|step| std::ptr::eq(*step, source))
                        .count();
                }
            },
            &mut |_| {},
        );
        if count != 1 {
            return Err(ResourceExecutionError::MissingCheckedInvocation);
        }
        let function = match &source.function {
            Expr::View(inner, _) => inner.as_ref(),
            other => other,
        };
        let arity = match function {
            Expr::GenericCall(_, types, _, _) => types.len(),
            _ => 0,
        };
        self.field_intrinsic_owner_at(source.span, arity)
    }

    fn field_intrinsic_owner_at(
        &self,
        span: Span,
        arity: usize,
    ) -> Result<Option<TypeId>, ResourceExecutionError> {
        let invocation = self.invocation(span)?;
        let facts = self.facts(&self.body)?;
        let (_, types, reflections) = intrinsic::intrinsic_records(
            &facts,
            &invocation,
            arity,
            &self.program.checked().interner,
        )?;
        let owner = *types
            .first()
            .ok_or(ResourceExecutionError::MissingCheckedBody)?;
        let reflection = reflections
            .first()
            .ok_or(ResourceExecutionError::MissingCheckedBody)?;
        if reflections.len() != types.len() {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        checked_field_reflection_owner(&self.program.checked().interner, owner, reflection)
    }
}

fn same_slice<T>(left: &[T], right: &[T]) -> bool {
    left.len() == right.len() && (left.is_empty() || std::ptr::eq(left.as_ptr(), right.as_ptr()))
}

pub(super) fn checked_field_reflection_owner(
    types: &jett_types::TypeInterner,
    owner: TypeId,
    source: &ReflectionTypeInfo,
) -> Result<Option<TypeId>, ResourceExecutionError> {
    let kind = match source.kind.as_str() {
        "struct" => "struct",
        "bitfield" => "bitfield",
        _ => return Ok(None),
    };
    let canonical = match types.resolve(owner) {
        Type::Struct(_) => "struct",
        Type::Bitfield(_) => "bitfield",
        _ => return Err(ResourceExecutionError::MissingCheckedBody),
    };
    if kind != canonical {
        return Err(ResourceExecutionError::MissingCheckedBody);
    }
    Ok(Some(owner))
}

pub(super) fn field_intrinsic(intrinsic: IntrinsicId) -> bool {
    matches!(
        intrinsic,
        IntrinsicId::TypeFields
            | IntrinsicId::TypeFieldValue
            | IntrinsicId::TypeConstructStart
            | IntrinsicId::TypeConstructPut
            | IntrinsicId::TypeConstructFinish
    )
}

#[cfg(test)]
#[path = "ordinary_field_reflection/tests.rs"]
mod tests;
