//! Required workers select original retained nodes and complete typed contexts.
//! Legacy type-name projections are data only; they cannot mint this identity.

use super::*;
use crate::checked_types::CheckedFunctionTypes;
use crate::value::ClosureScopedTypeBinding;
use std::hash::{Hash, Hasher};

pub use value_proof::{
    ActorGraphHookRoot, CheckedActorGraphHook, CheckedRequiredOwner, CheckedRequiredResourceHook,
    CheckedRequiredScope, CheckedRequiredValue,
};
#[path = "required/value_proof.rs"]
mod value_proof;

#[path = "required/actor_owners.rs"]
mod actor_owners;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct AttemptNode {
    origin: ExecutableOrigin,
    path: Vec<RegionStep>,
    generic: Option<usize>,
    actor: Option<jett_typecheck::CheckedActorHandlerTarget>,
    scopes: Vec<(Span, usize, TypeId, Option<usize>, ReflectionTypeInfo)>,
}

/// Private program-bound key; its program Arc keeps numeric/path identity live.
#[derive(Debug, Clone)]
pub(crate) struct CheckedAttemptKey {
    program: Arc<CheckedResourceProgram>,
    node: AttemptNode,
}

impl CheckedAttemptKey {
    /// A selected lexical scope retains every concrete ancestor selection.
    /// Another program, callable, generic instance or sibling is never an ancestor.
    pub(super) fn is_lexical_descendant_of(&self, parent: &Self) -> bool {
        Arc::ptr_eq(&self.program, &parent.program)
            && self.node.origin == parent.node.origin
            && self.node.generic == parent.node.generic
            && self.node.actor == parent.node.actor
            && self
                .node
                .path
                .strip_prefix(parent.node.path.as_slice())
                .is_some_and(|suffix| {
                    !suffix
                        .iter()
                        .any(|step| matches!(step, RegionStep::InlineBody(_)))
                })
            && self.node.scopes.starts_with(&parent.node.scopes)
    }
}

impl PartialEq for CheckedAttemptKey {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.program, &other.program) && self.node == other.node
    }
}
impl Eq for CheckedAttemptKey {}
impl Hash for CheckedAttemptKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.program).hash(state);
        self.node.hash(state);
    }
}

#[derive(Clone)]
pub(crate) struct PreparedRequiredExpression {
    reference: CheckedBodyReference,
    purpose: ExecutionPurpose,
    key: CheckedAttemptKey,
    result_type: TypeId,
    function: Option<Arc<CheckedFunctionTypes>>,
    scope: Option<Arc<CheckedScopedTypes>>,
    bindings: Vec<ClosureScopedTypeBinding>,
}

impl PreparedRequiredExpression {
    pub(crate) fn purpose(&self) -> ExecutionPurpose {
        self.purpose
    }
    pub(crate) fn key(&self) -> CheckedAttemptKey {
        self.key.clone()
    }
    pub(crate) fn result_type(&self) -> TypeId {
        self.result_type
    }
    pub(crate) fn checked_program(&self) -> &Arc<CheckedResourceProgram> {
        &self.key.program
    }
    pub(crate) fn function_projection(&self) -> Option<Arc<CheckedFunctionTypes>> {
        self.function.clone()
    }
    pub(crate) fn scope_projection(&self) -> Option<Arc<CheckedScopedTypes>> {
        self.scope.clone()
    }
    pub(crate) fn bindings(&self) -> Vec<ClosureScopedTypeBinding> {
        self.bindings.clone()
    }
    pub(crate) fn reference(&self) -> &CheckedBodyReference {
        &self.reference
    }
    pub(crate) fn expression(&self) -> Result<&Expr, ResourceExecutionError> {
        match self.reference.region()? {
            OriginalRegion::Expression(expression) => Ok(expression),
            _ => Err(ResourceExecutionError::MissingCheckedBody),
        }
    }
}

impl CheckedExecution {
    pub(super) fn attempt_key(
        &self,
        cursor: &CheckedBodyCursor,
    ) -> Result<CheckedAttemptKey, ResourceExecutionError> {
        self.validate_executable_cursor(cursor)?;
        self.attempt_key_after_identity(cursor)
    }

    pub(super) fn field_attempt_key(
        &self,
        cursor: &CheckedBodyCursor,
    ) -> Result<CheckedAttemptKey, ResourceExecutionError> {
        self.validate_field_executable_cursor(cursor)?;
        self.attempt_key_after_identity(cursor)
    }

    fn attempt_key_after_identity(
        &self,
        cursor: &CheckedBodyCursor,
    ) -> Result<CheckedAttemptKey, ResourceExecutionError> {
        let executable = cursor
            .executable
            .as_ref()
            .ok_or(ResourceExecutionError::MissingCheckedBody)?;
        Ok(CheckedAttemptKey {
            program: self.program.clone(),
            node: AttemptNode {
                origin: executable.origin.clone(),
                path: executable.path.clone(),
                generic: match cursor.root {
                    BodyRoot::Ordinary => None,
                    BodyRoot::Generic(index) => Some(index),
                    BodyRoot::ActorHandler(_) => None,
                },
                actor: match cursor.root {
                    BodyRoot::ActorHandler(target) => Some(target),
                    _ => None,
                },
                scopes: cursor
                    .scopes
                    .iter()
                    .map(|selection| {
                        (
                            selection.owner,
                            selection.index,
                            selection.bound_type,
                            match selection.selection {
                                CheckedComptimeTypeSelection::Unconditional => None,
                                CheckedComptimeTypeSelection::ReflectedIteration(index) => {
                                    Some(index)
                                }
                            },
                            selection.reflection.clone(),
                        )
                    })
                    .collect(),
            },
        })
    }

    fn required_expression(
        &self,
        mut cursor: CheckedBodyCursor,
        expression: &Expr,
        bindings: Vec<ClosureScopedTypeBinding>,
    ) -> Result<PreparedRequiredExpression, ResourceExecutionError> {
        self.validate_required_source_cursor(&cursor)?;
        self.validate_actor_required_expression(&cursor, expression)?;
        let reference = CheckedBodyReference {
            cursor: cursor.clone(),
        };
        let index = expression_index(reference.region()?, expression)?;
        cursor
            .executable
            .as_mut()
            .ok_or(ResourceExecutionError::MissingCheckedBody)?
            .path
            .push(RegionStep::Expression(index));
        self.validate_required_source_cursor(&cursor)?;
        let key = self.attempt_key_after_identity(&cursor)?;
        let result_type = *self
            .facts(&cursor)?
            .types
            .get(&expression.span())
            .ok_or(ResourceExecutionError::MissingCheckedBody)?;
        let function = match cursor.root {
            BodyRoot::Ordinary => None,
            BodyRoot::Generic(index) => Some(function_projection(self, index)?),
            BodyRoot::ActorHandler(_) => None,
        };
        let scope = if cursor.scopes.is_empty() {
            match cursor.root {
                BodyRoot::ActorHandler(target) => Some(self.required_actor_projection(target)?),
                BodyRoot::Ordinary | BodyRoot::Generic(_) => None,
            }
        } else {
            let mut scopes = match cursor.root {
                BodyRoot::Ordinary => &self.program.checked().comptime_type_bindings,
                BodyRoot::Generic(index) => {
                    &self.program.checked().generic_function_instantiations[index]
                        .comptime_type_bindings
                }
                BodyRoot::ActorHandler(target) => {
                    &self
                        .program
                        .checked()
                        .actor_handler(target)
                        .ok_or(ResourceExecutionError::MissingCheckedBody)?
                        .body()
                        .comptime_type_bindings
                }
            };
            let mut selected = None;
            for selection in &cursor.scopes {
                let candidate = scopes
                    .get(&selection.owner)
                    .and_then(|values| values.get(selection.index))
                    .ok_or(ResourceExecutionError::MissingCheckedBody)?;
                selected = Some(scoped_projection(
                    candidate,
                    &self.program.checked().interner,
                ));
                scopes = &candidate.body.comptime_type_bindings;
            }
            selected
        };
        Ok(PreparedRequiredExpression {
            reference: CheckedBodyReference { cursor },
            purpose: self.purpose,
            key,
            result_type,
            function,
            scope,
            bindings,
        })
    }

    pub(crate) fn prepare_namespace_initializer(
        &self,
        declaration: &jett_parser::ast::VarDecl,
    ) -> Result<PreparedRequiredExpression, ResourceExecutionError> {
        if self.purpose != ExecutionPurpose::NamespaceConstant {
            return Err(ResourceExecutionError::WrongPurpose);
        }
        let items = self
            .program
            .module()
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| match item {
                Item::VarDecl(original) if std::ptr::eq(original, declaration) => Some(index),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [item] = items.as_slice() else {
            return Err(ResourceExecutionError::MissingCheckedBody);
        };
        if !self
            .program
            .checked()
            .type_map
            .contains_key(&declaration.name.span)
        {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        let cursor = CheckedBodyCursor {
            root: BodyRoot::Ordinary,
            scopes: Vec::new(),
            executable: Some(ExecutableIdentity {
                program: self.program.clone(),
                origin: ExecutableOrigin::NamespaceInitializer { item: *item },
                path: Vec::new(),
            }),
        };
        let expression = match &declaration.value {
            Expr::Comptime(value, _) => value.as_ref(),
            value => value,
        };
        self.required_expression(cursor, expression, Vec::new())
    }

    pub(crate) fn prepare_verify_body(
        &self,
        original: &jett_parser::ast::VerifyBlock,
    ) -> Result<CheckedBodyReference, ResourceExecutionError> {
        if self.purpose != ExecutionPurpose::Verify {
            return Err(ResourceExecutionError::WrongPurpose);
        }
        let items = self
            .program
            .module()
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| match item {
                Item::Verify(value) if std::ptr::eq(value, original) => Some(index),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [item] = items.as_slice() else {
            return Err(ResourceExecutionError::MissingCheckedBody);
        };
        self.required_body(ExecutableOrigin::Verify { item: *item })
    }

    pub(crate) fn prepare_property_body(
        &self,
        original: &jett_parser::ast::PropertyBlock,
    ) -> Result<CheckedBodyReference, ResourceExecutionError> {
        if self.purpose != ExecutionPurpose::Property {
            return Err(ResourceExecutionError::WrongPurpose);
        }
        let items = self
            .program
            .module()
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| match item {
                Item::Property(value) if std::ptr::eq(value, original) => Some(index),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [item] = items.as_slice() else {
            return Err(ResourceExecutionError::MissingCheckedBody);
        };
        self.required_body(ExecutableOrigin::Property { item: *item })
    }

    fn required_body(
        &self,
        origin: ExecutableOrigin,
    ) -> Result<CheckedBodyReference, ResourceExecutionError> {
        let reference = CheckedBodyReference {
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
        self.validate_executable_cursor(&reference.cursor)?;
        Ok(reference)
    }

    pub(crate) fn prepare_explicit_contexts(
        &self,
        expression: &Expr,
        occurrence: Span,
        owner: Option<Span>,
        bindings: &[(Span, String)],
    ) -> Result<Vec<PreparedRequiredExpression>, ResourceExecutionError> {
        if self.purpose != ExecutionPurpose::ExplicitComptime {
            return Err(ResourceExecutionError::WrongPurpose);
        }
        let mut bases = Vec::new();
        let mut source_is_original = if owner.is_none() {
            self.actor_explicit_bases(expression, occurrence, bindings, &mut bases)?
        } else {
            false
        };
        for (item, original) in self.program.module().items.iter().enumerate() {
            let origin = match original {
                Item::Function(function) if owner == Some(function.name.span) => {
                    let definition = self.declaration_definition(function.name.span)?;
                    ExecutableOrigin::Function { definition, item }
                }
                Item::VarDecl(_) if owner.is_none() => {
                    ExecutableOrigin::NamespaceInitializer { item }
                }
                Item::Verify(_) if owner.is_none() => ExecutableOrigin::Verify { item },
                Item::Property(_) if owner.is_none() => ExecutableOrigin::Property { item },
                Item::TypeAlias(alias) if owner.is_none() && alias.constraint.is_some() => {
                    ExecutableOrigin::RefinementPredicate {
                        definition: self.refinement_definition(alias)?,
                        item,
                    }
                }
                _ => continue,
            };
            // Authenticate syntax independently of concrete execution eligibility.
            // This structural cursor is never installed or used to obtain facts.
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
            let paths =
                explicit_region_paths(structural.region()?, expression, occurrence, bindings)?;
            if paths.is_empty() {
                continue;
            }
            let [path] = paths.as_slice() else {
                return Err(ResourceExecutionError::MissingCheckedBody);
            };
            source_is_original = true;
            let roots = match (origin, original) {
                (ExecutableOrigin::Function { definition, .. }, Item::Function(function))
                    if !function.type_params.is_empty() =>
                {
                    self.program
                        .checked()
                        .generic_function_instantiations
                        .iter()
                        .enumerate()
                        .filter(|(_, body)| body.definition == definition)
                        .map(|(index, _)| BodyRoot::Generic(index))
                        .collect::<Vec<_>>()
                }
                _ => vec![BodyRoot::Ordinary],
            };
            for root in roots {
                bases.push((
                    CheckedBodyCursor {
                        root,
                        scopes: Vec::new(),
                        executable: Some(ExecutableIdentity {
                            program: self.program.clone(),
                            origin: origin.clone(),
                            path: Vec::new(),
                        }),
                    },
                    path.clone(),
                ));
            }
        }
        let mut prepared = Vec::new();
        for (base, path) in bases {
            let mut contexts = vec![(base, Vec::new())];
            for step in path {
                let mut expanded = Vec::new();
                for (cursor, prior_bindings) in contexts {
                    let reference = CheckedBodyReference {
                        cursor: cursor.clone(),
                    };
                    match step {
                        RegionStep::InlineBody(index) => {
                            region_inline_body(reference.region()?, index)?;
                            let mut next = cursor;
                            next.executable
                                .as_mut()
                                .ok_or(ResourceExecutionError::MissingCheckedBody)?
                                .path
                                .push(RegionStep::InlineBody(index));
                            self.validate_required_source_cursor(&next)?;
                            expanded.push((next, prior_bindings));
                        }
                        RegionStep::Scope(owner) => {
                            let binding = region_binding(reference.region()?, owner)?;
                            let Some(candidates) = self.facts(&cursor)?.scopes.get(&owner) else {
                                continue;
                            };
                            for (index, candidate) in candidates.iter().enumerate() {
                                let mut next = cursor.clone();
                                next.scopes.push(ScopedSelection {
                                    owner,
                                    index,
                                    bound_type: candidate.bound_type,
                                    selection: candidate.selection,
                                    reflection: candidate.reflection.clone(),
                                });
                                next.executable
                                    .as_mut()
                                    .ok_or(ResourceExecutionError::MissingCheckedBody)?
                                    .path
                                    .push(RegionStep::Scope(owner));
                                self.validate_required_source_cursor(&next)?;
                                let mut projected = prior_bindings.clone();
                                projected.push(ClosureScopedTypeBinding {
                                    name: binding.name.name.clone(),
                                    canonical_name: self
                                        .program
                                        .checked()
                                        .interner
                                        .type_name(candidate.bound_type),
                                    reflection: Some(candidate.reflection.clone()),
                                });
                                expanded.push((next, projected));
                            }
                        }
                        RegionStep::Expression(_) => {
                            return Err(ResourceExecutionError::MissingCheckedBody);
                        }
                    }
                }
                contexts = expanded;
            }
            for (cursor, projected) in contexts {
                if self.facts(&cursor)?.types.contains_key(&occurrence) {
                    prepared.push(self.required_expression(cursor, expression, projected)?);
                }
            }
        }
        if !source_is_original {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        // A retained original expression with no accepted concrete/scoped type
        // occurrence has no evaluation attempt, as on the ordinary route.
        // Eligible contexts still require exact packets when they execute.
        Ok(prepared)
    }

    pub(crate) fn original_assignment_key(
        &self,
        assignment: &AssignStmt,
    ) -> Result<CheckedAttemptKey, ResourceExecutionError> {
        self.original_assignment(assignment)?;
        self.attempt_key(&self.body)
    }

    pub(super) fn field_body_key(&self) -> Result<CheckedAttemptKey, ResourceExecutionError> {
        self.field_attempt_key(&self.body)
    }

    pub(crate) fn original_comptime_key(
        &self,
        expression: &Expr,
    ) -> Result<CheckedAttemptKey, ResourceExecutionError> {
        let Expr::Comptime(inner, span) = expression else {
            return Err(ResourceExecutionError::MissingCheckedBody);
        };
        self.validate_executable_cursor(&self.body)?;
        let reference = CheckedBodyReference {
            cursor: self.body.clone(),
        };
        expression_index(reference.region()?, expression)?;
        if !self.facts(&self.body)?.types.contains_key(span) {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        Ok(self
            .required_expression(self.body.clone(), inner, Vec::new())?
            .key)
    }
}

/// Select pure required syntax without making callable bodies part of the
/// ordinary executable walker. Every path is to one physical retained node.
fn explicit_region_paths(
    region: OriginalRegion<'_>,
    expression: &Expr,
    occurrence: Span,
    bindings: &[(Span, String)],
) -> Result<Vec<Vec<RegionStep>>, ResourceExecutionError> {
    let mut scopes = Vec::new();
    let mut inline = Vec::new();
    let mut owns = false;
    walk_region(
        region,
        &mut |scope| scopes.push(scope),
        &mut |node| {
            if let Expr::Comptime(inner, span) = node {
                owns |= *span == occurrence && std::ptr::eq(inner.as_ref(), expression);
            }
            if matches!(node, Expr::InlineFn(..)) {
                inline.push(node);
            }
        },
        &mut |_| {},
    );
    let mut paths = Vec::new();
    if owns && bindings.is_empty() {
        paths.push(Vec::new());
    }
    if let Some(((owner, name), rest)) = bindings.split_first() {
        for scope in scopes {
            if scope.span != *owner {
                continue;
            }
            if scope.name.name != *name {
                return Err(ResourceExecutionError::MissingCheckedBody);
            }
            for mut path in explicit_region_paths(
                OriginalRegion::Block(&scope.body),
                expression,
                occurrence,
                rest,
            )? {
                path.insert(0, RegionStep::Scope(*owner));
                paths.push(path);
            }
        }
    }
    for node in inline {
        let index = expression_index(region, node)?;
        let body = region_inline_body(region, index)?;
        for mut path in explicit_region_paths(
            OriginalRegion::Block(body),
            expression,
            occurrence,
            bindings,
        )? {
            path.insert(0, RegionStep::InlineBody(index));
            paths.push(path);
        }
    }
    Ok(paths)
}

fn function_projection(
    checked: &CheckedExecution,
    index: usize,
) -> Result<Arc<CheckedFunctionTypes>, ResourceExecutionError> {
    let body = checked
        .program
        .checked()
        .generic_function_instantiations
        .get(index)
        .ok_or(ResourceExecutionError::MissingCheckedBody)?;
    let types = &checked.program.checked().interner;
    let mut expressions = body
        .type_map
        .iter()
        .map(|(span, ty)| (*span, types.type_name(*ty)))
        .collect();
    remove_descendant_expression_names(&mut expressions, &body.comptime_type_bindings);
    Ok(Arc::new(CheckedFunctionTypes {
        bindings: body
            .comptime_type_bindings
            .iter()
            .map(|(span, candidates)| {
                (
                    *span,
                    candidates
                        .iter()
                        .map(|candidate| scoped_projection(candidate, types))
                        .collect(),
                )
            })
            .collect(),
        type_arguments: body
            .concrete_args
            .iter()
            .map(|ty| types.type_name(*ty))
            .collect(),
        type_argument_reflections: body.specialization.type_argument_reflections.clone(),
        type_info_kinds: body.specialization.type_info_kinds.clone(),
        type_info_primitives: body.specialization.type_info_primitives.clone(),
        type_kind_values: body.specialization.type_kind_values.clone(),
        type_primitive_values: body.specialization.type_primitive_values.clone(),
        expressions: Arc::new(expressions),
    }))
}

#[cfg(test)]
#[path = "required/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "required/owner_tests.rs"]
mod owner_tests;

#[cfg(test)]
#[path = "required/actor_owner_tests.rs"]
mod actor_owner_tests;
