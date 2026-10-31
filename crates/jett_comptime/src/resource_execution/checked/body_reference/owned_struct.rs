//! Constructor identity only; runtime tickets are supplied by reached actuals.
use super::*;

pub(crate) struct PreparedOwnedStruct {
    key: CheckedAttemptKey,
    expression: usize,
    definition: DefId,
    ty: TypeId,
    name: String,
    fields: Vec<(String, TypeId)>,
    source_fields: Vec<usize>,
}

impl PreparedOwnedStruct {
    /// Canonical aliases share these checked primitive kinds. Qualifications
    /// remain distinct and cannot acquire constructor authority by peeling.
    pub(crate) fn supports_companion(kind: &Type) -> bool {
        matches!(
            kind,
            Type::Int8
                | Type::Int16
                | Type::Int32
                | Type::Int64
                | Type::Uint8
                | Type::Uint16
                | Type::Uint32
                | Type::Uint64
                | Type::Float32
                | Type::Float64
                | Type::String
                | Type::Bool
                | Type::Nothing
        )
    }

    pub(crate) fn ty(&self) -> TypeId {
        self.ty
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn fields(&self) -> &[(String, TypeId)] {
        &self.fields
    }

    pub(crate) fn source_fields(&self) -> &[usize] {
        &self.source_fields
    }
}

impl CheckedExecution {
    pub(crate) fn prepare_owned_struct(
        &self,
        expression: &Expr,
    ) -> Result<Option<PreparedOwnedStruct>, ResourceExecutionError> {
        self.validate_executable_cursor(&self.body)?;
        let Expr::Call(callee, arguments, span) = expression else {
            return Ok(None);
        };
        let facts = self.facts(&self.body)?;
        let Some(construction) = facts.constructions.get(span) else {
            return Ok(None);
        };
        let ty = construction.struct_type;
        if !self.type_contains_resource(ty)? {
            return Ok(None);
        }
        // Refinement evaluation and generic declaration association need their
        // own connected transport. They remain Source requirements.
        if construction.validates_refinements
            || !matches!(self.body.root, BodyRoot::Ordinary)
            || !self.body.scopes.is_empty()
        {
            return Ok(None);
        }
        let reference = CheckedBodyReference {
            cursor: self.body.clone(),
        };
        let index = expression_index(reference.region()?, expression)?;
        if self.source_call_dispatch(callee, arguments, *span)?
            || self.expression_type(*span)? != ty
        {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        let definition = self.resolved_definition(callee.span())?;
        let info = self
            .program
            .resolved()
            .scope_table
            .definitions
            .get(definition.index() as usize)
            .ok_or(ResourceExecutionError::InvalidInvocation)?;
        let Type::Struct(owner) = self.program.checked().interner.resolve(ty) else {
            return Err(ResourceExecutionError::InvalidInvocation);
        };
        let structure = self.program.checked().interner.resolve_struct(*owner);
        if info.id != definition
            || info.kind != jett_resolve::DefKind::Struct
            || info.name != structure.name
            || self.program.checked().definition_types.get(&definition) != Some(&ty)
            || !self
                .program
                .checked()
                .interner
                .nominal_type_arguments(ty)
                .is_empty()
        {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        let declarations = self
            .program
            .module()
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Struct(declaration) if declaration.name.span == info.span => {
                    Some(declaration)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let [declaration] = declarations.as_slice() else {
            return Err(ResourceExecutionError::MissingCheckedBody);
        };
        if !declaration.type_params.is_empty() {
            return Ok(None);
        }
        if declaration.fields.len() != structure.fields.len()
            || declaration
                .fields
                .iter()
                .zip(&structure.fields)
                .any(|(original, field)| original.name.name != field.0)
        {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        // This vertical has direct nominal leaves and scalar companions only.
        // Do not accidentally grant a nested/capability/refined carrier path.
        if structure.fields.iter().any(|(_, field)| {
            let kind = self.program.checked().interner.resolve(*field);
            !matches!(kind, Type::Resource(_)) && !PreparedOwnedStruct::supports_companion(kind)
        }) {
            return Ok(None);
        }
        if arguments.len() != structure.fields.len() {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        let mut assigned = vec![false; structure.fields.len()];
        let mut source_fields = Vec::with_capacity(arguments.len());
        for argument in arguments {
            let ordinal = match &argument.name {
                Some(name) => structure
                    .fields
                    .iter()
                    .position(|(field, _)| field == &name.name),
                None => assigned.iter().position(|assigned| !assigned),
            }
            .ok_or(ResourceExecutionError::InvalidInvocation)?;
            if assigned[ordinal]
                || self.expression_type(argument.value.span())? != structure.fields[ordinal].1
            {
                return Err(ResourceExecutionError::InvalidInvocation);
            }
            assigned[ordinal] = true;
            source_fields.push(ordinal);
        }
        Ok(Some(PreparedOwnedStruct {
            key: self.attempt_key(&self.body)?,
            expression: index,
            definition,
            ty,
            name: structure.name.clone(),
            fields: structure.fields.clone(),
            source_fields,
        }))
    }

    pub(crate) fn revalidate_owned_struct(
        &self,
        proof: &PreparedOwnedStruct,
        expression: &Expr,
    ) -> Result<(), ResourceExecutionError> {
        let current = self
            .prepare_owned_struct(expression)?
            .ok_or(ResourceExecutionError::InvalidInvocation)?;
        if current.key != proof.key
            || current.expression != proof.expression
            || current.definition != proof.definition
            || current.ty != proof.ty
            || current.name != proof.name
            || current.fields != proof.fields
            || current.source_fields != proof.source_fields
        {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "owned_struct_tests.rs"]
mod tests;
