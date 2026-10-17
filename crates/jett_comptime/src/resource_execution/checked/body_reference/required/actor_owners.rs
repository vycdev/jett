//! Pure required-value selection is metadata, never actor Resource execution.
use super::*;
use jett_typecheck::CheckedActorHandlerTarget;
use jett_types::ActorId;

impl CheckedExecution {
    /// Only explicit required workers may project actor facts. This intentionally
    /// does not change validate_executable_cursor or install_function_body: a
    /// runtime actor handler still cannot install a Resource execution cursor.
    pub(super) fn validate_required_source_cursor(
        &self,
        cursor: &CheckedBodyCursor,
    ) -> Result<(), ResourceExecutionError> {
        if cursor.executable.as_ref().is_some_and(|identity| {
            matches!(
                identity.origin,
                ExecutableOrigin::ActorInitializer { .. } | ExecutableOrigin::ActorHandler { .. }
            )
        }) {
            if self.purpose != ExecutionPurpose::ExplicitComptime {
                return Err(ResourceExecutionError::WrongPurpose);
            }
            self.validate_field_executable_cursor(cursor)
        } else {
            self.validate_executable_cursor(cursor)
        }
    }

    pub(super) fn validate_actor_required_expression(
        &self,
        cursor: &CheckedBodyCursor,
        expression: &Expr,
    ) -> Result<(), ResourceExecutionError> {
        let actor_origin = cursor.executable.as_ref().is_some_and(|identity| {
            matches!(
                identity.origin,
                ExecutableOrigin::ActorInitializer { .. } | ExecutableOrigin::ActorHandler { .. }
            )
        });
        if !actor_origin {
            return Ok(());
        }
        self.validate_required_source_cursor(cursor)?;
        let reference = CheckedBodyReference {
            cursor: cursor.clone(),
        };
        let mut occurrences = Vec::new();
        walk_region(
            reference.region()?,
            &mut |_| {},
            &mut |node| {
                if let Expr::Comptime(inner, span) = node {
                    if std::ptr::eq(inner.as_ref(), expression) {
                        occurrences.push(*span);
                    }
                }
            },
            &mut |_| {},
        );
        let [occurrence] = occurrences.as_slice() else {
            return Err(ResourceExecutionError::MissingCheckedBody);
        };
        if !self.facts(cursor)?.types.contains_key(occurrence) {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        Ok(())
    }

    pub(super) fn required_actor_declaration(
        &self,
        item: usize,
    ) -> Result<(DefId, ActorId), ResourceExecutionError> {
        let Some(Item::Actor(actor)) = self.program.module().items.get(item) else {
            return Err(ResourceExecutionError::MissingCheckedBody);
        };
        let declarations = self
            .program
            .resolved()
            .scope_table
            .definitions
            .iter()
            .filter(|definition| {
                definition.kind == jett_resolve::DefKind::Actor
                    && definition.span == actor.name.span
            })
            .collect::<Vec<_>>();
        let [definition] = declarations.as_slice() else {
            return Err(ResourceExecutionError::MissingCheckedBody);
        };
        let ty = self
            .program
            .checked()
            .definition_types
            .get(&definition.id)
            .ok_or(ResourceExecutionError::MissingCheckedBody)?;
        let Type::Actor(actor_id) = self.program.checked().interner.resolve(*ty) else {
            return Err(ResourceExecutionError::MissingCheckedBody);
        };
        let origin =
            CheckedActorInstanceOrigin::from_checked_actor(self.program.clone(), *actor_id)?;
        if !std::ptr::eq(origin.original()?, actor) {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        Ok((definition.id, *actor_id))
    }

    pub(super) fn required_actor_projection(
        &self,
        target: CheckedActorHandlerTarget,
    ) -> Result<Arc<CheckedScopedTypes>, ResourceExecutionError> {
        let handler = self
            .program
            .checked()
            .actor_handler(target)
            .ok_or(ResourceExecutionError::MissingCheckedBody)?;
        let types = &self.program.checked().interner;
        let (expressions, bindings) = crate::checked_types::original_body_types(
            &handler.body().type_map,
            &handler.body().comptime_type_bindings,
            types,
        );
        Ok(Arc::new(CheckedScopedTypes {
            bound_type: types.resolve_actor(target.actor).name.clone(),
            reflection: None,
            expressions,
            bindings,
        }))
    }

    /// Authenticate the physical original occurrence before selecting a checked
    /// actor ordinal. Equal values, names, spans or caller AST clones confer no key.
    pub(super) fn actor_explicit_bases(
        &self,
        expression: &Expr,
        occurrence: Span,
        bindings: &[(Span, String)],
        bases: &mut Vec<(CheckedBodyCursor, Vec<RegionStep>)>,
    ) -> Result<bool, ResourceExecutionError> {
        if self.purpose != ExecutionPurpose::ExplicitComptime {
            return Err(ResourceExecutionError::WrongPurpose);
        }
        let mut original = false;
        for (item, node) in self.program.module().items.iter().enumerate() {
            let Item::Actor(actor) = node else { continue };
            for (field, initializer) in actor.state_fields.iter().enumerate() {
                let paths = explicit_region_paths(
                    OriginalRegion::Expression(&initializer.value),
                    expression,
                    occurrence,
                    bindings,
                )?;
                if paths.is_empty() {
                    continue;
                }
                let [path] = paths.as_slice() else {
                    return Err(ResourceExecutionError::MissingCheckedBody);
                };
                self.required_actor_declaration(item)?;
                let cursor = CheckedBodyCursor {
                    root: BodyRoot::Ordinary,
                    scopes: Vec::new(),
                    executable: Some(ExecutableIdentity {
                        program: self.program.clone(),
                        origin: ExecutableOrigin::ActorInitializer { item, field },
                        path: Vec::new(),
                    }),
                };
                self.validate_required_source_cursor(&cursor)?;
                bases.push((cursor, path.clone()));
                original = true;
            }
            for (handler_index, handler) in actor.handlers.iter().enumerate() {
                let paths = explicit_region_paths(
                    OriginalRegion::Block(&handler.body),
                    expression,
                    occurrence,
                    bindings,
                )?;
                if paths.is_empty() {
                    continue;
                }
                let [path] = paths.as_slice() else {
                    return Err(ResourceExecutionError::MissingCheckedBody);
                };
                let (definition, actor_id) = self.required_actor_declaration(item)?;
                let target = CheckedActorHandlerTarget {
                    actor_definition: definition,
                    actor: actor_id,
                    handler_index,
                };
                self.validate_actor_handler_identity(target, item)?;
                let cursor = CheckedBodyCursor {
                    root: BodyRoot::ActorHandler(target),
                    scopes: Vec::new(),
                    executable: Some(ExecutableIdentity {
                        program: self.program.clone(),
                        origin: ExecutableOrigin::ActorHandler {
                            definition,
                            item,
                            handler: handler_index,
                        },
                        path: Vec::new(),
                    }),
                };
                self.validate_required_source_cursor(&cursor)?;
                bases.push((cursor, path.clone()));
                original = true;
            }
        }
        Ok(original)
    }
}
