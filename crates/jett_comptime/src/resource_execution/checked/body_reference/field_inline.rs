//! Unique ordinary inline-body selection from the existing capture snapshot.
//! Resource entry validation is deliberately unchanged and still rejects a
//! runtime InlineBody cursor. This module installs no provider or custody.
use super::*;
use crate::value::{ClosureTypeContext, closure_capture::CheckedClosureCapture};

fn inline_paths(
    region: OriginalRegion<'_>,
    target: &Expr,
) -> Result<Vec<Vec<RegionStep>>, ResourceExecutionError> {
    let mut scopes = Vec::new();
    let mut inline = Vec::new();
    walk_region(
        region,
        &mut |scope| scopes.push(scope),
        &mut |node| {
            if matches!(node, Expr::InlineFn(..)) {
                inline.push(node);
            }
        },
        &mut |_| {},
    );
    let mut paths = Vec::new();
    for scope in scopes {
        for mut path in inline_paths(OriginalRegion::Block(&scope.body), target)? {
            path.insert(0, RegionStep::Scope(scope.span));
            paths.push(path);
        }
    }
    for node in inline {
        let index = expression_index(region, node)?;
        if std::ptr::eq(node, target) {
            paths.push(vec![RegionStep::InlineBody(index)]);
        } else {
            for mut path in inline_paths(
                OriginalRegion::Block(region_inline_body(region, index)?),
                target,
            )? {
                path.insert(0, RegionStep::InlineBody(index));
                paths.push(path);
            }
        }
    }
    Ok(paths)
}

impl CheckedBodyReference {
    pub(crate) fn field_inline_parts(
        &self,
    ) -> Result<(&[jett_parser::ast::Param], &Block), ResourceExecutionError> {
        let identity = self
            .cursor
            .executable
            .as_ref()
            .ok_or(ResourceExecutionError::MissingCheckedBody)?;
        let Some((RegionStep::InlineBody(index), prefix)) = identity.path.split_last() else {
            return Err(ResourceExecutionError::MissingCheckedBody);
        };
        match region_expression(self.region_at_path_len(prefix.len())?, *index)? {
            Expr::InlineFn(parameters, _, body, _) => Ok((parameters, body)),
            _ => Err(ResourceExecutionError::MissingCheckedBody),
        }
    }
}

impl CheckedExecution {
    pub(crate) fn prepare_field_inline_body(
        &self,
        proof: &CheckedClosureCapture,
        context: &ClosureTypeContext,
    ) -> Result<CheckedBodyReference, ResourceExecutionError> {
        // The caller obtained this proof only through validate_source against
        // this exact program and current full closure Value snapshot.
        let target = proof
            .field_original_node()
            .ok_or(ResourceExecutionError::MissingCheckedBody)?;
        let mut selected = Vec::new();
        for (item, node) in self.program.module().items.iter().enumerate() {
            // Identify the retained physical owner before asking it for executable
            // authority. Unrelated Resource hook declarations deliberately reject
            // ordinary entry and must not block a different function's closure.
            let original_region = match node {
                Item::Function(function) => OriginalRegion::Block(&function.body),
                Item::Verify(block) => OriginalRegion::Block(&block.body),
                Item::Property(block) => OriginalRegion::Block(&block.body),
                _ => continue,
            };
            let paths = inline_paths(original_region, target)?;
            if paths.is_empty() {
                continue;
            }
            let [path] = paths.as_slice() else {
                return Err(ResourceExecutionError::MissingCheckedBody);
            };
            let (origin, roots) = match node {
                Item::Function(function) => {
                    let definition = self.declaration_definition(function.name.span)?;
                    let roots = if function.type_params.is_empty() {
                        if context.checked_function.is_some() || !context.arguments.is_empty() {
                            continue;
                        }
                        vec![BodyRoot::Ordinary]
                    } else {
                        let Some(current) = &context.checked_function else {
                            continue;
                        };
                        self.program.checked().generic_function_instantiations.iter().enumerate()
                            .filter(|(_, instance)| instance.definition == definition
                                && crate::value::closure_capture::matches_field_function_projection(
                                    current, &context.arguments, instance, &self.program.checked().interner,
                                ))
                            .map(|(index, _)| BodyRoot::Generic(index)).collect()
                    };
                    (ExecutableOrigin::Function { definition, item }, roots)
                }
                Item::Verify(_)
                    if context.checked_function.is_none() && context.arguments.is_empty() =>
                {
                    (ExecutableOrigin::Verify { item }, vec![BodyRoot::Ordinary])
                }
                Item::Property(_)
                    if context.checked_function.is_none() && context.arguments.is_empty() =>
                {
                    (
                        ExecutableOrigin::Property { item },
                        vec![BodyRoot::Ordinary],
                    )
                }
                _ => continue,
            };
            let structural = CheckedBodyReference {
                cursor: CheckedBodyCursor {
                    root: BodyRoot::Ordinary,
                    scopes: Vec::new(),
                    executable: Some(ExecutableIdentity {
                        program: self.program.clone(),
                        origin,
                        path: Vec::new(),
                    }),
                },
            };
            for root in roots {
                let mut base = structural.cursor.clone();
                base.root = root;
                let mut candidates = vec![base];
                let mut scope_depth = 0usize;
                for step in path {
                    let mut expanded = Vec::new();
                    for cursor in candidates {
                        let reference = CheckedBodyReference {
                            cursor: cursor.clone(),
                        };
                        match step {
                            RegionStep::InlineBody(index) => {
                                region_inline_body(reference.region()?, *index)?;
                                let mut next = cursor;
                                next.executable
                                    .as_mut()
                                    .ok_or(ResourceExecutionError::MissingCheckedBody)?
                                    .path
                                    .push(step.clone());
                                self.validate_field_executable_cursor(&next)?;
                                expanded.push(next);
                            }
                            RegionStep::Scope(owner) => {
                                let binding = region_binding(reference.region()?, *owner)?;
                                let Some(actual) = context.scoped_type_bindings.get(scope_depth)
                                else {
                                    continue;
                                };
                                let facts = self.facts(&cursor)?;
                                let Some(choices) = facts.scopes.get(owner) else {
                                    continue;
                                };
                                for (index, choice) in choices.iter().enumerate() {
                                    if actual.name != binding.name.name
                                        || actual.canonical_name
                                            != self
                                                .program
                                                .checked()
                                                .interner
                                                .type_name(choice.bound_type)
                                        || actual.reflection.as_ref() != Some(&choice.reflection)
                                    {
                                        continue;
                                    }
                                    let mut next = cursor.clone();
                                    next.scopes.push(ScopedSelection {
                                        owner: *owner,
                                        index,
                                        bound_type: choice.bound_type,
                                        selection: choice.selection,
                                        reflection: choice.reflection.clone(),
                                    });
                                    next.executable
                                        .as_mut()
                                        .ok_or(ResourceExecutionError::MissingCheckedBody)?
                                        .path
                                        .push(step.clone());
                                    self.validate_field_executable_cursor(&next)?;
                                    expanded.push(next);
                                }
                            }
                            RegionStep::Expression(_) => {
                                return Err(ResourceExecutionError::MissingCheckedBody);
                            }
                        }
                    }
                    if matches!(step, RegionStep::Scope(_)) {
                        scope_depth += 1;
                    }
                    candidates = expanded;
                }
                if scope_depth != context.scoped_type_bindings.len() {
                    continue;
                }
                for cursor in candidates {
                    let checked_scope = if let Some(selection) = cursor.scopes.last() {
                        let mut parent = cursor.clone();
                        parent.scopes.pop();
                        let facts = self.facts(&parent)?;
                        let choice = facts
                            .scopes
                            .get(&selection.owner)
                            .and_then(|choices| choices.get(selection.index))
                            .ok_or(ResourceExecutionError::MissingCheckedBody)?;
                        Some(scoped_projection(choice, &self.program.checked().interner))
                    } else {
                        None
                    };
                    if checked_scope != context.checked_scope {
                        continue;
                    }
                    let reference = CheckedBodyReference { cursor };
                    reference.field_inline_parts()?;
                    selected.push(reference);
                }
            }
        }
        let [reference] = selected.as_slice() else {
            // Equal callable TypeIds do not collapse distinct generic/scoped
            // ownership contexts. A missing unique producer remains unsupported.
            return Err(ResourceExecutionError::MissingCheckedBody);
        };
        Ok(reference.clone())
    }
}
