//! Original reflected field loops select ordinal-specific checked bodies.
//! Runtime metadata values remain data, never executable or provider authority.
use super::*;
use jett_parser::ast::ForStmt;
use jett_resolve::DefKind;
use jett_types::ReflectionFieldInfo;

pub(crate) struct PreparedReflectedFieldLoop {
    parent: CheckedBodyReference,
    key: CheckedAttemptKey,
    loop_span: Span,
    variable: DefId,
    owner: TypeId,
    fields: Vec<(TypeId, ReflectionFieldInfo)>,
}

#[derive(Clone)]
pub(crate) struct PreparedReflectedFieldIteration {
    owner: Arc<PreparedReflectedFieldLoop>,
    index: usize,
}

impl PreparedReflectedFieldLoop {
    pub(crate) fn owner_name(&self) -> String {
        self.parent
            .cursor
            .executable
            .as_ref()
            .expect("prepared original field loop")
            .program
            .checked()
            .interner
            .type_name(self.owner)
    }

    pub(crate) fn fields(&self) -> impl Iterator<Item = &ReflectionFieldInfo> {
        self.fields.iter().map(|(_, field)| field)
    }

    pub(crate) fn validate_count(&self, count: usize) -> Result<(), ResourceExecutionError> {
        if count != self.fields.len() {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        Ok(())
    }

    pub(crate) fn iteration(
        self: &Arc<Self>,
        index: usize,
    ) -> Result<PreparedReflectedFieldIteration, ResourceExecutionError> {
        if index >= self.fields.len() {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        Ok(PreparedReflectedFieldIteration {
            owner: self.clone(),
            index,
        })
    }

    fn original(&self) -> Result<&ForStmt, ResourceExecutionError> {
        original_for(self.parent.region()?, self.loop_span)
    }
}

impl PreparedReflectedFieldIteration {
    pub(crate) fn variable(&self) -> DefId {
        self.owner.variable
    }

    pub(crate) fn validate_value(
        &self,
        value: &crate::Value,
    ) -> Result<(), ResourceExecutionError> {
        let (_, field) = self
            .owner
            .fields
            .get(self.index)
            .ok_or(ResourceExecutionError::MissingCheckedBody)?;
        let expected =
            crate::Interpreter::reflection_field_info_value(&self.owner.owner_name(), None, field);
        if value != &expected {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        Ok(())
    }
}

impl CheckedExecution {
    pub(crate) fn checked_reflection_fields(
        &self,
        owner: TypeId,
    ) -> Result<Vec<ReflectionFieldInfo>, ResourceExecutionError> {
        Ok(checked_owner_fields(self.program(), owner)?
            .into_iter()
            .map(|(_, field)| field)
            .collect())
    }

    pub(crate) fn prepare_reflected_field_loop(
        &self,
        source: &ForStmt,
    ) -> Result<Option<Arc<PreparedReflectedFieldLoop>>, ResourceExecutionError> {
        self.validate_executable_cursor(&self.body)?;
        let parent = CheckedBodyReference {
            cursor: self.body.clone(),
        };
        let original = original_for(parent.region()?, source.span)?;
        if !std::ptr::eq(original, source) {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        let Expr::GenericCall(callee, source_types, arguments, occurrence) = &original.iterable
        else {
            return Ok(None);
        };
        if self.facts(&self.body)?.intrinsics.get(occurrence) != Some(&IntrinsicId::TypeFields) {
            return Ok(None);
        }
        if source_types.len() != 1 || !arguments.is_empty() || original.value_variable.is_some() {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        let invocation = self.invocation(*occurrence)?;
        let prepared =
            self.prepare_intrinsic_arguments(callee, source_types, arguments, &invocation)?;
        if prepared.intrinsic() != IntrinsicId::TypeFields {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        let [owner] = prepared.types() else {
            return Err(ResourceExecutionError::MissingCheckedBody);
        };
        let fields = checked_owner_fields(self.program(), *owner)?;
        let variables = self
            .program
            .resolved()
            .scope_table
            .definitions
            .iter()
            .filter(|definition| {
                definition.kind == DefKind::Variable
                    && definition.span == original.variable.span
                    && definition.name == original.variable.name
            })
            .map(|definition| definition.id)
            .collect::<Vec<_>>();
        let [variable] = variables.as_slice() else {
            return Err(ResourceExecutionError::MissingCheckedBody);
        };
        let loop_span = original.span;
        Ok(Some(Arc::new(PreparedReflectedFieldLoop {
            parent,
            key: self.attempt_key(&self.body)?,
            loop_span,
            variable: *variable,
            owner: *owner,
            fields,
        })))
    }

    pub(crate) fn prepare_reflected_field_scope(
        &self,
        bind: &ComptimeTypeBindStmt,
        iteration: &PreparedReflectedFieldIteration,
    ) -> Result<PreparedDirectScope, ResourceExecutionError> {
        let proof = &iteration.owner;
        self.validate_executable_cursor(&proof.parent.cursor)?;
        if self.attempt_key(&proof.parent.cursor)? != proof.key {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        if !self
            .attempt_key(&self.body)?
            .is_lexical_descendant_of(&proof.key)
        {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        let source_loop = proof.original()?;
        let Expr::GenericCall(_, source_types, arguments, occurrence) = &source_loop.iterable
        else {
            return Err(ResourceExecutionError::MissingCheckedBody);
        };
        let loop_facts = self.facts(&proof.parent.cursor)?;
        let variable = self
            .program
            .resolved()
            .scope_table
            .definitions
            .get(proof.variable.index() as usize)
            .ok_or(ResourceExecutionError::MissingCheckedBody)?;
        if source_types.len() != 1
            || !arguments.is_empty()
            || loop_facts.intrinsics.get(occurrence) != Some(&IntrinsicId::TypeFields)
            || loop_facts
                .intrinsic_types
                .get(occurrence)
                .map(Vec::as_slice)
                != Some(&[proof.owner][..])
            || variable.id != proof.variable
            || variable.kind != DefKind::Variable
            || variable.span != source_loop.variable.span
            || variable.name != source_loop.variable.name
        {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        let parent = CheckedBodyReference {
            cursor: self.body.clone(),
        };
        let original = region_binding(parent.region()?, bind.span)?;
        if !std::ptr::eq(original, bind) || !binding_under_loop(&source_loop.body, original) {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        let Expr::FieldAccess(base, member, _) = &original.value else {
            return Err(ResourceExecutionError::MissingCheckedBody);
        };
        let Expr::Ident(variable) = base.as_ref() else {
            return Err(ResourceExecutionError::MissingCheckedBody);
        };
        if member.name != "type_info"
            || self.program.resolved().resolutions.get(&variable.span) != Some(&proof.variable)
        {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        // Rejoin the immutable checked owner table, not a cloned runtime TypeInfo.
        let fields = checked_owner_fields(self.program(), proof.owner)?;
        if fields != proof.fields {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        let (bound_type, field) = fields
            .get(iteration.index)
            .ok_or(ResourceExecutionError::MissingCheckedBody)?;
        let facts = self.facts(&self.body)?;
        let candidates = facts
            .scopes
            .get(&original.span)
            .ok_or(ResourceExecutionError::MissingCheckedBody)?
            .iter()
            .enumerate()
            .filter(|(_, candidate)| {
                candidate.selection
                    == CheckedComptimeTypeSelection::ReflectedIteration(iteration.index)
            })
            .collect::<Vec<_>>();
        let [(index, candidate)] = candidates.as_slice() else {
            return Err(ResourceExecutionError::MissingCheckedBody);
        };
        if candidate.bound_type != *bound_type || candidate.reflection != field.type_info {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        let mut cursor = self.body.clone();
        cursor.scopes.push(ScopedSelection {
            owner: original.span,
            index: *index,
            bound_type: candidate.bound_type,
            selection: candidate.selection,
            reflection: candidate.reflection.clone(),
        });
        cursor
            .executable
            .as_mut()
            .ok_or(ResourceExecutionError::MissingCheckedBody)?
            .path
            .push(RegionStep::Scope(original.span));
        self.validate_executable_cursor(&cursor)?;
        Ok(PreparedDirectScope {
            reference: CheckedBodyReference { cursor },
            projection: scoped_projection(candidate, &self.program.checked().interner),
            reflection: candidate.reflection.clone(),
        })
    }
}

fn checked_owner_fields(
    program: &CheckedResourceProgram,
    owner: TypeId,
) -> Result<Vec<(TypeId, ReflectionFieldInfo)>, ResourceExecutionError> {
    let types = &program.checked().interner;
    if owner.index() as usize >= types.len() {
        return Err(ResourceExecutionError::InvalidCheckedProgram);
    }
    let field_types: Vec<TypeId> = match types.resolve(owner) {
        Type::Struct(id) => types
            .resolve_struct(*id)
            .fields
            .iter()
            .map(|(_, ty)| *ty)
            .collect(),
        Type::Bitfield(id) => types
            .resolve_bitfield(*id)
            .fields
            .iter()
            .map(|field| field.ty)
            .collect(),
        _ => return Ok(Vec::new()),
    };
    let fields = program
        .checked()
        .reflection_metadata
        .get_type_fields_for_id(owner)
        .ok_or(ResourceExecutionError::MissingCheckedBody)?;
    if fields.len() != field_types.len()
        || fields
            .iter()
            .enumerate()
            .any(|(index, field)| field.index != index)
        || field_types
            .iter()
            .any(|ty| ty.index() as usize >= types.len())
    {
        return Err(ResourceExecutionError::MissingCheckedBody);
    }
    Ok(field_types
        .into_iter()
        .zip(fields.iter().cloned())
        .collect())
}

fn original_for(
    region: OriginalRegion<'_>,
    span: Span,
) -> Result<&ForStmt, ResourceExecutionError> {
    let mut loops = Vec::new();
    walk_region(region, &mut |_| {}, &mut |_| {}, &mut |statement| {
        if let Stmt::For(value) = statement {
            if value.span == span {
                loops.push(value);
            }
        }
    });
    let [original] = loops.as_slice() else {
        return Err(ResourceExecutionError::MissingCheckedBody);
    };
    Ok(*original)
}

fn binding_under_loop(block: &Block, selected: &ComptimeTypeBindStmt) -> bool {
    let mut matches = 0;
    walk_block(
        block,
        &mut |binding| {
            if std::ptr::eq(binding, selected) {
                matches += 1;
            } else if binding_under_loop(&binding.body, selected) {
                matches += 1;
            }
        },
        &mut |_| {},
    );
    matches == 1
}

#[cfg(test)]
#[path = "reflected_fields/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "reflected_fields/recursion_tests.rs"]
mod recursion_tests;

#[cfg(test)]
#[path = "reflected_fields/metadata_shapes_tests.rs"]
mod metadata_shapes_tests;
