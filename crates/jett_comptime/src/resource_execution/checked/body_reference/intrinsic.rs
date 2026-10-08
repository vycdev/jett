//! Concrete intrinsic operands from one original call and accepted body frame.
use super::*;
use jett_parser::ast::TypeExpr;

pub(crate) struct PreparedIntrinsicArguments {
    program: Arc<CheckedResourceProgram>,
    key: CheckedAttemptKey,
    packet: jett_typecheck::CheckedCallOwnership,
    span: Span,
    intrinsic: IntrinsicId,
    types: Vec<TypeId>,
    reflections: Vec<ReflectionTypeInfo>,
}

impl PreparedIntrinsicArguments {
    pub(crate) fn intrinsic(&self) -> IntrinsicId {
        self.intrinsic
    }
    pub(crate) fn types(&self) -> &[TypeId] {
        &self.types
    }
    pub(crate) fn reflections(&self) -> &[ReflectionTypeInfo] {
        &self.reflections
    }
    pub(crate) fn validate(
        &self,
        checked: &CheckedExecution,
        invocation: &CheckedInvocation,
    ) -> Result<(), ResourceExecutionError> {
        if !Arc::ptr_eq(&self.program, checked.program()) {
            return Err(ResourceExecutionError::ForeignProgram);
        }
        if self.key != checked.attempt_key(&invocation.body)?
            || self.packet != invocation.packet
            || self.span != invocation.span
        {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        let facts = checked.facts(&invocation.body)?;
        let (intrinsic, types, reflections) = intrinsic_records(
            &facts,
            invocation,
            self.types.len(),
            &checked.program.checked().interner,
        )?;
        if intrinsic != self.intrinsic || types != self.types || reflections != self.reflections {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        Ok(())
    }
}

impl CheckedExecution {
    pub(crate) fn prepare_intrinsic_arguments(
        &self,
        callee: &Expr,
        source_types: &[TypeExpr],
        source_arguments: &[CallArg],
        invocation: &CheckedInvocation,
    ) -> Result<PreparedIntrinsicArguments, ResourceExecutionError> {
        self.intrinsic_current_body(invocation)?;
        let reference = CheckedBodyReference {
            cursor: invocation.body.clone(),
        };
        let mut matches = 0usize;
        walk_region(
            reference.region()?,
            &mut |_| {},
            &mut |expression| {
                let (original_callee, types, arguments, span) = match expression {
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
                if span == invocation.span
                    && std::ptr::eq(original_callee, callee)
                    && same_slice(arguments, source_arguments)
                    && same_slice(types, source_types)
                {
                    matches += 1;
                }
            },
            &mut |_| {},
        );
        if matches != 1 {
            return Err(ResourceExecutionError::MissingCheckedInvocation);
        }
        self.prepared_intrinsic(invocation, source_types.len())
    }

    pub(crate) fn prepare_pipeline_intrinsic_arguments(
        &self,
        prepared: &PreparedPipelineStep<'_>,
    ) -> Result<PreparedIntrinsicArguments, ResourceExecutionError> {
        self.intrinsic_current_body(prepared.invocation())?;
        let reference = CheckedBodyReference {
            cursor: prepared.invocation().body.clone(),
        };
        let mut found = None;
        let mut count = 0usize;
        walk_region(
            reference.region()?,
            &mut |_| {},
            &mut |expression| {
                let Expr::Pipeline(_, steps, _) = expression else {
                    return;
                };
                for (index, step) in steps.iter().enumerate() {
                    if std::ptr::eq(step, prepared.original()) {
                        found = Some((expression, index));
                        count += 1;
                    }
                }
            },
            &mut |_| {},
        );
        if count != 1 {
            return Err(ResourceExecutionError::MissingCheckedInvocation);
        }
        let (pipeline, index) = found.ok_or(ResourceExecutionError::MissingCheckedInvocation)?;
        let original = self.prepare_pipeline_step(pipeline, index)?;
        if original.invocation().packet != prepared.invocation().packet
            || !std::ptr::eq(original.callee(), prepared.callee())
            || !same_slice(original.type_arguments(), prepared.type_arguments())
            || !same_slice(original.extra_arguments(), prepared.extra_arguments())
        {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        self.prepared_intrinsic(prepared.invocation(), prepared.type_arguments().len())
    }

    fn intrinsic_current_body(
        &self,
        invocation: &CheckedInvocation,
    ) -> Result<(), ResourceExecutionError> {
        if self.attempt_key(&self.body)? != self.attempt_key(&invocation.body)? {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        Ok(())
    }

    fn prepared_intrinsic(
        &self,
        invocation: &CheckedInvocation,
        arity: usize,
    ) -> Result<PreparedIntrinsicArguments, ResourceExecutionError> {
        let facts = self.facts(&invocation.body)?;
        let (intrinsic, types, reflections) =
            intrinsic_records(&facts, invocation, arity, &self.program.checked().interner)?;
        Ok(PreparedIntrinsicArguments {
            program: self.program.clone(),
            key: self.attempt_key(&invocation.body)?,
            packet: invocation.packet.clone(),
            span: invocation.span,
            intrinsic,
            types: types.to_vec(),
            reflections: reflections.to_vec(),
        })
    }
}

fn same_slice<T>(left: &[T], right: &[T]) -> bool {
    left.len() == right.len() && (left.is_empty() || std::ptr::eq(left.as_ptr(), right.as_ptr()))
}

fn intrinsic_records<'a>(
    facts: &BodyFacts<'a>,
    invocation: &CheckedInvocation,
    arity: usize,
    types: &jett_types::TypeInterner,
) -> Result<(IntrinsicId, &'a [TypeId], &'a [ReflectionTypeInfo]), ResourceExecutionError> {
    if facts.calls.get(&invocation.span) != Some(&invocation.packet) {
        return Err(ResourceExecutionError::InvalidInvocation);
    }
    let CheckedInvocationTarget::Intrinsic(intrinsic) = invocation.target() else {
        return Err(ResourceExecutionError::InvalidInvocation);
    };
    let CheckedInvocationShape::Intrinsic {
        intrinsic: shape,
        result_type,
        operands,
    } = &invocation.packet.shape
    else {
        return Err(ResourceExecutionError::InvalidInvocation);
    };
    if shape != intrinsic
        || facts.intrinsics.get(&invocation.span) != Some(intrinsic)
        || operands.len() != invocation.arguments().len()
        || result_type.index() as usize >= types.len()
    {
        return Err(ResourceExecutionError::InvalidInvocation);
    }
    let concrete = facts
        .intrinsic_types
        .get(&invocation.span)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if concrete.len() != arity
        || (arity != 0 && !facts.intrinsic_types.contains_key(&invocation.span))
    {
        return Err(ResourceExecutionError::MissingCheckedBody);
    }
    for &ty in concrete {
        concrete_type(types, ty)?;
    }
    let reflected = facts
        .intrinsic_reflections
        .get(&invocation.span)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if (metadata_leaf(*intrinsic) && (arity != 1 || reflected.len() != 1))
        || (!reflected.is_empty() && reflected.len() != arity)
    {
        return Err(ResourceExecutionError::MissingCheckedBody);
    }
    Ok((*intrinsic, concrete, reflected))
}

fn metadata_leaf(intrinsic: IntrinsicId) -> bool {
    matches!(
        intrinsic,
        IntrinsicId::TypeName
            | IntrinsicId::TypeKind
            | IntrinsicId::TypeKindTag
            | IntrinsicId::TypePrimitiveTag
            | IntrinsicId::TypeHasSecret
            | IntrinsicId::TypeInfo
            | IntrinsicId::TypeArg
    )
}

fn concrete_type(
    types: &jett_types::TypeInterner,
    ty: TypeId,
) -> Result<(), ResourceExecutionError> {
    let mut pending = vec![ty];
    let mut seen = std::collections::HashSet::new();
    while let Some(ty) = pending.pop() {
        if ty.index() as usize >= types.len() {
            return Err(ResourceExecutionError::InvalidCheckedProgram);
        }
        if !seen.insert(ty) {
            continue;
        }
        pending.extend_from_slice(types.nominal_type_arguments(ty));
        match types.resolve(ty) {
            Type::Error => return Err(ResourceExecutionError::MissingCheckedBody),
            Type::List(inner)
            | Type::Set(inner)
            | Type::Optional(inner)
            | Type::Secret(inner)
            | Type::Refinement { base: inner, .. } => pending.push(*inner),
            Type::Map(key, value) | Type::Result(key, value) => {
                pending.push(*key);
                pending.push(*value);
            }
            Type::Function {
                params,
                return_type,
                ..
            } => {
                pending.extend_from_slice(params);
                pending.push(*return_type);
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "intrinsic/tests.rs"]
mod tests;
