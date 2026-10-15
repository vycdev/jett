//! Ordinary field writes reuse original executable identity, never custody.
use super::*;
#[cfg(test)]
#[path = "field_updates/tests.rs"]
mod tests;
use crate::checked_types::CheckedFunctionTypes;
use crate::value::ClosureTypeArgument;
use jett_parser::ast::Ident;
use jett_typecheck::assignment_places::{
    CheckedAssignmentPlace, CheckedFieldUpdateResourceRequirement, checked_field_owner,
    validate_assignment_place_shape,
};

fn source_field_path<'source>(
    expression: &'source Expr,
    path: &mut Vec<(&'source Ident, Span)>,
) -> Option<&'source Ident> {
    match expression {
        Expr::Ident(root) => Some(root),
        Expr::Paren(inner, _) => source_field_path(inner, path),
        Expr::FieldAccess(base, field, span) => {
            let root = source_field_path(base, path)?;
            path.push((field, *span));
            Some(root)
        }
        _ => None,
    }
}

fn nested_binding_needed(
    scopes: &HashMap<Span, Vec<CheckedComptimeTypeBinding>>,
    declaration: Span,
) -> bool {
    scopes.values().flatten().any(|scope| {
        scope
            .body
            .assignment_places
            .values()
            .any(|place| place.root.declaration_span == declaration)
            || nested_binding_needed(&scope.body.comptime_type_bindings, declaration)
    })
}

pub(crate) struct PreparedFieldAssignment<'source> {
    source: &'source AssignStmt,
    place: CheckedAssignmentPlace,
    key: CheckedAttemptKey,
}

impl PreparedFieldAssignment<'_> {
    pub(crate) fn place(&self) -> &CheckedAssignmentPlace {
        &self.place
    }

    pub(crate) fn accepts_binding(&self, binding: &PreparedFieldBinding) -> bool {
        self.place.root == binding.fact && self.key.is_lexical_descendant_of(&binding.key)
    }
}

pub(crate) struct PreparedFieldBinding {
    fact: CheckedBindingFact,
    key: CheckedAttemptKey,
}

impl PreparedFieldBinding {
    pub(crate) fn fact(&self) -> CheckedBindingFact {
        self.fact
    }
}

impl CheckedExecution {
    pub(crate) fn install_field_function_body(
        &mut self,
        reference: &CheckedBodyReference,
    ) -> Result<(), ResourceExecutionError> {
        self.validate_field_executable_cursor(&reference.cursor)?;
        self.body = reference.cursor.clone();
        Ok(())
    }

    pub(crate) fn restore_field_cursor(
        &mut self,
        cursor: CheckedBodyCursor,
    ) -> Result<(), ResourceExecutionError> {
        self.facts(&cursor)?;
        if cursor.executable.is_some() {
            self.validate_field_executable_cursor(&cursor)?;
        }
        self.body = cursor;
        Ok(())
    }

    fn original_field_assignment_key(
        &self,
        assignment: &AssignStmt,
    ) -> Result<CheckedAttemptKey, ResourceExecutionError> {
        self.validate_field_executable_cursor(&self.body)?;
        let reference = CheckedBodyReference {
            cursor: self.body.clone(),
        };
        let mut found = 0usize;
        walk_region(reference.region()?, &mut |_| {}, &mut |_| {}, &mut |node| {
            if let Stmt::Assign(original) = node
                && std::ptr::eq(original, assignment)
            {
                found += 1;
            }
        });
        if found != 1 {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        self.field_attempt_key(&self.body)
    }
    pub(crate) fn field_scope_has_updates(
        &self,
        span: Span,
    ) -> Result<bool, ResourceExecutionError> {
        fn contains(scopes: &HashMap<Span, Vec<CheckedComptimeTypeBinding>>) -> bool {
            scopes.values().flatten().any(|scope| {
                !scope.body.assignment_places.is_empty()
                    || contains(&scope.body.comptime_type_bindings)
            })
        }
        let facts = self.facts(&self.body)?;
        Ok(facts.scopes.get(&span).is_some_and(|candidates| {
            candidates.iter().any(|scope| {
                !scope.body.assignment_places.is_empty()
                    || contains(&scope.body.comptime_type_bindings)
            })
        }))
    }
    pub(crate) fn prepare_field_test_body(
        &self,
        block: &Block,
        purpose: ExecutionPurpose,
    ) -> Result<CheckedBodyReference, ResourceExecutionError> {
        for item in &self.program.module().items {
            match (item, purpose) {
                (Item::Verify(original), ExecutionPurpose::Verify)
                    if std::ptr::eq(&original.body, block) =>
                {
                    return self.prepare_verify_body(original);
                }
                (Item::Property(original), ExecutionPurpose::Property)
                    if std::ptr::eq(&original.body, block) =>
                {
                    return self.prepare_property_body(original);
                }
                _ => {}
            }
        }
        Err(ResourceExecutionError::MissingCheckedBody)
    }
    pub(crate) fn field_binding_needed(
        &self,
        declaration: Span,
    ) -> Result<bool, ResourceExecutionError> {
        let facts = self.facts(&self.body)?;
        Ok(facts
            .assignment_places
            .values()
            .any(|place| place.root.declaration_span == declaration)
            || nested_binding_needed(facts.scopes, declaration))
    }
    /// A registered clone may identify a retained declaration only after the
    /// existing full original/current AST comparison. Execution uses the retained
    /// function below; the clone does not become an original-node authority.
    pub(crate) fn prepare_field_function_body(
        &self,
        name: &str,
        namespace: &Option<String>,
        current: &FunctionDef,
        projection: Option<&CheckedFunctionTypes>,
        arguments: &[ClosureTypeArgument],
    ) -> Result<CheckedBodyReference, ResourceExecutionError> {
        crate::value::closure_capture::OriginalClosureOwner::registered(
            &self.program,
            name,
            current,
            namespace,
        )
        .ok_or(ResourceExecutionError::MissingCheckedBody)?;
        let definition = self.declaration_definition(current.name.span)?;
        let original = self.retained_function(definition)?;
        let root = if original.type_params.is_empty() {
            if projection.is_some() || !arguments.is_empty() {
                return Err(ResourceExecutionError::MissingCheckedBody);
            }
            BodyRoot::Ordinary
        } else {
            let projection = projection.ok_or(ResourceExecutionError::MissingCheckedBody)?;
            let matching = self
                .program
                .checked()
                .generic_function_instantiations
                .iter()
                .enumerate()
                .filter(|(_, instance)| {
                    instance.definition == definition
                        && crate::value::closure_capture::matches_field_function_projection(
                            projection,
                            arguments,
                            instance,
                            &self.program.checked().interner,
                        )
                })
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            let [index] = matching.as_slice() else {
                return Err(ResourceExecutionError::MissingCheckedBody);
            };
            BodyRoot::Generic(*index)
        };
        let items = self
            .program
            .module()
            .items
            .iter()
            .enumerate()
            .filter_map(|(item, node)| {
                matches!(node, Item::Function(function) if std::ptr::eq(function, original))
                    .then_some(item)
            })
            .collect::<Vec<_>>();
        let [item] = items.as_slice() else {
            return Err(ResourceExecutionError::MissingCheckedBody);
        };
        let reference = CheckedBodyReference {
            cursor: CheckedBodyCursor {
                root,
                scopes: Vec::new(),
                executable: Some(ExecutableIdentity {
                    program: self.program.clone(),
                    origin: ExecutableOrigin::Function {
                        definition,
                        item: *item,
                    },
                    path: Vec::new(),
                }),
            },
        };
        self.validate_executable_cursor(&reference.cursor)?;
        Ok(reference)
    }

    /// Only an original declaration/parameter under the selected complete body
    /// can install the physical lexical slot used by a later field assignment.
    pub(crate) fn prepare_field_binding(
        &self,
        name: &Ident,
    ) -> Result<PreparedFieldBinding, ResourceExecutionError> {
        self.validate_field_executable_cursor(&self.body)?;
        let reference = CheckedBodyReference {
            cursor: self.body.clone(),
        };
        let mut matches = 0usize;
        walk_region(
            reference.region()?,
            &mut |_| {},
            &mut |_| {},
            &mut |statement| {
                if let Stmt::VarDecl(declaration) = statement
                    && std::ptr::eq(&declaration.name, name)
                {
                    matches += 1;
                }
            },
        );
        if let Ok((parameters, _)) = reference.field_inline_parts() {
            matches += parameters
                .iter()
                .filter(|parameter| std::ptr::eq(&parameter.name, name))
                .count();
        } else if self.body.scopes.is_empty()
            && let Ok(function) = reference.function()
        {
            matches += function
                .params
                .iter()
                .filter(|parameter| std::ptr::eq(&parameter.name, name))
                .count();
        }
        let fact = self.binding(name.span)?;
        let info = self
            .program
            .resolved()
            .scope_table
            .definitions
            .get(
                usize::try_from(fact.definition.index())
                    .map_err(|_| ResourceExecutionError::InvalidOwner)?,
            )
            .ok_or(ResourceExecutionError::InvalidOwner)?;
        if matches != 1
            || info.span != name.span
            || info.id != fact.definition
            || fact.declaration_span != name.span
            || !matches!(
                info.kind,
                jett_resolve::DefKind::Variable | jett_resolve::DefKind::Param
            )
        {
            return Err(ResourceExecutionError::InvalidOwner);
        }
        Ok(PreparedFieldBinding {
            fact,
            key: self.field_body_key()?,
        })
    }

    pub(crate) fn prepare_field_assignment<'source>(
        &self,
        source: &'source AssignStmt,
    ) -> Result<PreparedFieldAssignment<'source>, ResourceExecutionError> {
        let key = self.original_field_assignment_key(source)?;
        let place = self
            .facts(&self.body)?
            .assignment_places
            .get(&source.span)
            .ok_or(ResourceExecutionError::MissingCheckedBody)?;
        // This private stamp belongs to this exact selected body/generic/scoped
        // record. One same-span specialization cannot bless another one.
        if !place.ownership_validated() {
            return Err(ResourceExecutionError::InvalidOwner);
        }
        validate_assignment_place_shape(place, &self.program.checked().interner)
            .map_err(|_| ResourceExecutionError::InvalidOwner)?;
        let mut source_path = Vec::new();
        let root = source_field_path(&source.target, &mut source_path)
            .ok_or(ResourceExecutionError::InvalidOwner)?;
        if root.span != place.root_use_span || source_path.len() != place.projections.len() {
            return Err(ResourceExecutionError::InvalidOwner);
        }
        for ((field, span), projection) in source_path.iter().zip(&place.projections) {
            let (_, fields) =
                checked_field_owner(&self.program.checked().interner, projection.owner_type)
                    .map_err(|_| ResourceExecutionError::InvalidOwner)?;
            if *span != projection.source_span
                || fields
                    .get(projection.field_index)
                    .map(|(name, _)| name.as_str())
                    != Some(field.name.as_str())
            {
                return Err(ResourceExecutionError::InvalidOwner);
            }
        }
        if place.assignment_span != source.span
            || self.resolved_definition(place.root_use_span)? != place.root.definition
            || self.expression_type(place.root_use_span)? != place.root_type
            || self.expression_type(source.target.span())?
                != place
                    .projections
                    .last()
                    .ok_or(ResourceExecutionError::InvalidOwner)?
                    .field_type
        {
            return Err(ResourceExecutionError::InvalidOwner);
        }
        let contains_resource = self.type_contains_resource(place.root.ty)?;
        if contains_resource
            != (place.resource == CheckedFieldUpdateResourceRequirement::RequiresCustody)
        {
            return Err(ResourceExecutionError::InvalidOwner);
        }
        Ok(PreparedFieldAssignment {
            source,
            place: place.clone(),
            key,
        })
    }

    pub(crate) fn revalidate_field_assignment(
        &self,
        prepared: &PreparedFieldAssignment<'_>,
    ) -> Result<(), ResourceExecutionError> {
        let current = self.prepare_field_assignment(prepared.source)?;
        if current.key != prepared.key || current.place != prepared.place {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        Ok(())
    }
}
