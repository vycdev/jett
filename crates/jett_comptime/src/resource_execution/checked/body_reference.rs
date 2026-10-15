//! Original executable identity for the first checked scoped-entry layer.
//! No runtime ownership or provider authority is carried by these references.

use jett_intrinsics::IntrinsicId;
use jett_parser::ast::{
    AssignStmt, Block, CallArg, ComptimeTypeBindStmt, Expr, FunctionDef, Item, Stmt, StringPart,
};

use super::*;
use crate::checked_types::{CheckedScopedBindings, CheckedScopedTypes};

#[path = "body_reference/field_updates.rs"]
mod field_updates;
pub(crate) use field_updates::{PreparedFieldAssignment, PreparedFieldBinding};
#[path = "body_reference/actor_handlers.rs"]
mod actor_handlers;
#[path = "body_reference/field_inline.rs"]
mod field_inline;
pub(crate) use actor_handlers::{CheckedActorInstanceOrigin, PreparedActorHandler};

#[path = "body_reference/absent_generated.rs"]
mod absent_generated;
pub(crate) use absent_generated::PreparedAbsentBindings;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ExecutableOrigin {
    Function {
        definition: DefId,
        item: usize,
    },
    ActorInitializer {
        item: usize,
        field: usize,
    },
    ActorHandler {
        definition: DefId,
        item: usize,
        handler: usize,
    },
    NamespaceInitializer {
        item: usize,
    },
    RefinementPredicate {
        definition: DefId,
        item: usize,
    },
    Verify {
        item: usize,
    },
    Property {
        item: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum RegionStep {
    Scope(Span),
    Expression(usize),
    InlineBody(usize),
}

#[derive(Debug, Clone)]
pub(super) struct ExecutableIdentity {
    program: Arc<CheckedResourceProgram>,
    origin: ExecutableOrigin,
    path: Vec<RegionStep>,
}

#[derive(Clone, Copy)]
enum OriginalRegion<'a> {
    Block(&'a Block),
    Expression(&'a Expr),
}

#[path = "body_reference/required.rs"]
mod required;
pub use required::{
    ActorGraphHookRoot, CheckedActorGraphHook, CheckedRequiredOwner, CheckedRequiredResourceHook,
    CheckedRequiredScope, CheckedRequiredValue,
};
pub(crate) use required::{CheckedAttemptKey, PreparedRequiredExpression};
#[path = "body_reference/pipeline.rs"]
mod pipeline;
pub(crate) use pipeline::PreparedPipelineStep;
#[path = "body_reference/intrinsic.rs"]
mod intrinsic;
pub(crate) use intrinsic::PreparedIntrinsicArguments;
#[path = "body_reference/named_callable.rs"]
mod named_callable;
pub(crate) use named_callable::PreparedNamedCallable;
#[path = "body_reference/owned_struct.rs"]
mod owned_struct;
pub(crate) use owned_struct::PreparedOwnedStruct;
#[path = "body_reference/reflected_fields.rs"]
mod reflected_fields;
pub(crate) use reflected_fields::{PreparedReflectedFieldIteration, PreparedReflectedFieldLoop};

/// Minted only from one retained program and an exact accepted body selection.
#[derive(Debug, Clone)]
pub(crate) struct CheckedBodyReference {
    cursor: CheckedBodyCursor,
}

pub(crate) struct PreparedDirectScope {
    reference: CheckedBodyReference,
    projection: Arc<CheckedScopedTypes>,
    reflection: ReflectionTypeInfo,
}

impl CheckedBodyReference {
    pub(crate) fn function(&self) -> Result<&FunctionDef, ResourceExecutionError> {
        let identity = self
            .cursor
            .executable
            .as_ref()
            .ok_or(ResourceExecutionError::MissingCheckedBody)?;
        let ExecutableOrigin::Function { item, .. } = identity.origin else {
            return Err(ResourceExecutionError::MissingCheckedBody);
        };
        match identity.program.module().items.get(item) {
            Some(Item::Function(function)) => Ok(function),
            _ => Err(ResourceExecutionError::MissingCheckedBody),
        }
    }

    fn region(&self) -> Result<OriginalRegion<'_>, ResourceExecutionError> {
        let length = self
            .cursor
            .executable
            .as_ref()
            .ok_or(ResourceExecutionError::MissingCheckedBody)?
            .path
            .len();
        self.region_at_path_len(length)
    }

    fn region_at_path_len(
        &self,
        length: usize,
    ) -> Result<OriginalRegion<'_>, ResourceExecutionError> {
        let identity = self
            .cursor
            .executable
            .as_ref()
            .ok_or(ResourceExecutionError::MissingCheckedBody)?;
        let mut region = match identity.origin {
            ExecutableOrigin::Function { .. } => OriginalRegion::Block(&self.function()?.body),
            ExecutableOrigin::NamespaceInitializer { item } => {
                match identity.program.module().items.get(item) {
                    Some(Item::VarDecl(value)) => OriginalRegion::Expression(&value.value),
                    _ => return Err(ResourceExecutionError::MissingCheckedBody),
                }
            }
            ExecutableOrigin::RefinementPredicate { item, .. } => {
                match identity.program.module().items.get(item) {
                    Some(Item::TypeAlias(alias)) => OriginalRegion::Expression(
                        alias
                            .constraint
                            .as_ref()
                            .ok_or(ResourceExecutionError::MissingCheckedBody)?,
                    ),
                    _ => return Err(ResourceExecutionError::MissingCheckedBody),
                }
            }
            ExecutableOrigin::ActorInitializer { item, field } => {
                match identity.program.module().items.get(item) {
                    Some(Item::Actor(actor)) => OriginalRegion::Expression(
                        &actor
                            .state_fields
                            .get(field)
                            .ok_or(ResourceExecutionError::MissingCheckedBody)?
                            .value,
                    ),
                    _ => return Err(ResourceExecutionError::MissingCheckedBody),
                }
            }
            ExecutableOrigin::ActorHandler { item, handler, .. } => {
                let Some(Item::Actor(actor)) = identity.program.module().items.get(item) else {
                    return Err(ResourceExecutionError::MissingCheckedBody);
                };
                OriginalRegion::Block(
                    &actor
                        .handlers
                        .get(handler)
                        .ok_or(ResourceExecutionError::MissingCheckedBody)?
                        .body,
                )
            }
            ExecutableOrigin::Verify { item } => match identity.program.module().items.get(item) {
                Some(Item::Verify(value)) => OriginalRegion::Block(&value.body),
                _ => return Err(ResourceExecutionError::MissingCheckedBody),
            },
            ExecutableOrigin::Property { item } => {
                match identity.program.module().items.get(item) {
                    Some(Item::Property(value)) => OriginalRegion::Block(&value.body),
                    _ => return Err(ResourceExecutionError::MissingCheckedBody),
                }
            }
        };
        if length > identity.path.len() {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        for step in identity.path.iter().take(length) {
            region = match step {
                RegionStep::Scope(owner) => {
                    OriginalRegion::Block(&region_binding(region, *owner)?.body)
                }
                RegionStep::Expression(index) => {
                    OriginalRegion::Expression(region_expression(region, *index)?)
                }
                RegionStep::InlineBody(index) => {
                    OriginalRegion::Block(region_inline_body(region, *index)?)
                }
            };
        }
        Ok(region)
    }

    fn block(&self) -> Result<&Block, ResourceExecutionError> {
        match self.region()? {
            OriginalRegion::Block(block) => Ok(block),
            OriginalRegion::Expression(_) => Err(ResourceExecutionError::MissingCheckedBody),
        }
    }
    pub(crate) fn original_expression(&self) -> Result<&Expr, ResourceExecutionError> {
        match self.region()? {
            OriginalRegion::Expression(value) => Ok(value),
            _ => Err(ResourceExecutionError::MissingCheckedBody),
        }
    }
}

impl PreparedDirectScope {
    pub(crate) fn field_reference(&self) -> CheckedBodyReference {
        self.reference.clone()
    }
    pub(crate) fn projection(&self) -> Arc<CheckedScopedTypes> {
        self.projection.clone()
    }
    pub(crate) fn reflection(&self) -> &ReflectionTypeInfo {
        &self.reflection
    }
    pub(crate) fn bound_name(&self) -> &str {
        &self.projection.bound_type
    }
    pub(crate) fn body(&self) -> Result<&Block, ResourceExecutionError> {
        self.reference.block()
    }
}

impl CheckedExecution {
    fn refinement_definition(
        &self,
        alias: &jett_parser::ast::TypeAlias,
    ) -> Result<DefId, ResourceExecutionError> {
        let original = self
            .program
            .module()
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                matches!(item, Item::TypeAlias(value) if std::ptr::eq(value, alias))
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        let [item] = original.as_slice() else {
            return Err(ResourceExecutionError::MissingCheckedBody);
        };
        if alias.constraint.is_none() {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        let namespace = self.program.module().items[..*item]
            .iter()
            .filter_map(|item| match item {
                Item::Namespace(value) if value.span.file == alias.name.span.file => {
                    Some(value.name.name.as_str())
                }
                _ => None,
            })
            .last();
        let canonical = namespace.map_or_else(
            || alias.name.name.clone(),
            |namespace| format!("{namespace}.{}", alias.name.name),
        );
        let definitions = self
            .program
            .resolved()
            .scope_table
            .definitions
            .iter()
            .filter(|definition| {
                definition.kind == jett_resolve::DefKind::Type
                    && definition.span == alias.name.span
                    && definition.namespace.as_deref() == namespace
                    && definition.name == canonical
            })
            .collect::<Vec<_>>();
        let [definition] = definitions.as_slice() else {
            return Err(ResourceExecutionError::MissingCheckedBody);
        };
        if self.program.resolved().resolutions.get(&alias.name.span).is_some_and(|resolved| *resolved != definition.id)
            || self.program.checked().interner.type_ids().filter(|ty| {
                matches!(self.program.checked().interner.resolve(*ty), Type::Refinement { name, .. } if name == &canonical)
            }).count() != 1
        { return Err(ResourceExecutionError::MissingCheckedBody); }
        Ok(definition.id)
    }

    /// Retain original actor initializer nodes before interpreter scope mutation.
    /// Registration clones and equal spans never supply executable call authority.
    pub(crate) fn prepare_actor_initializers(
        &self,
        actor: &jett_parser::ast::ActorDef,
    ) -> Result<Vec<CheckedBodyReference>, ResourceExecutionError> {
        let matches = self
            .program
            .module()
            .items
            .iter()
            .enumerate()
            .filter_map(|(item, value)| match value {
                Item::Actor(original)
                    if original.span == actor.span && original.name.span == actor.name.span =>
                {
                    Some((item, original))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let [(item, original)] = matches.as_slice() else {
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
                    && definition.span == original.name.span
            })
            .collect::<Vec<_>>();
        let [declaration] = declarations.as_slice() else {
            return Err(ResourceExecutionError::MissingCheckedBody);
        };
        let mut registered = (*original).clone();
        registered.name.name = declaration.name.clone();
        if format!("{registered:?}") != format!("{actor:?}") {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        let mut references = Vec::new();
        for field in 0..original.state_fields.len() {
            let reference = CheckedBodyReference {
                cursor: CheckedBodyCursor {
                    root: BodyRoot::Ordinary,
                    scopes: Vec::new(),
                    executable: Some(ExecutableIdentity {
                        program: self.program.clone(),
                        origin: ExecutableOrigin::ActorInitializer { item: *item, field },
                        path: Vec::new(),
                    }),
                },
            };
            self.validate_executable_cursor(&reference.cursor)?;
            let expression = reference.original_expression()?;
            self.facts(&reference.cursor)?
                .types
                .get(&expression.span())
                .ok_or(ResourceExecutionError::MissingCheckedBody)?;
            references.push(reference);
        }
        Ok(references)
    }
    pub(crate) fn prepare_function_body(
        &self,
        invocation: &FunctionInvocation,
    ) -> Result<CheckedBodyReference, ResourceExecutionError> {
        let (definition, root) = match invocation {
            FunctionInvocation::Entry { definition, .. } => (*definition, BodyRoot::Ordinary),
            FunctionInvocation::NamedSource { source, target } => {
                self.validate_named_indirect(source, target)?;
                (target.definition(), BodyRoot::Ordinary)
            }
            FunctionInvocation::Source(invocation) => {
                self.validate_executable_cursor(&invocation.body)?;
                if self.facts(&invocation.body)?.calls.get(&invocation.span)
                    != Some(&invocation.packet)
                {
                    return Err(ResourceExecutionError::InvalidInvocation);
                }
                match invocation.target() {
                    CheckedInvocationTarget::Resolved(definition) => {
                        (*definition, BodyRoot::Ordinary)
                    }
                    CheckedInvocationTarget::Generic(call) => {
                        let indices = self
                            .program
                            .checked()
                            .generic_function_instantiations
                            .iter()
                            .enumerate()
                            .filter(|(_, body)| {
                                body.definition == call.definition
                                    && body.concrete_args == call.concrete_args
                                    && body.specialization == call.specialization
                            })
                            .map(|(index, _)| index)
                            .collect::<Vec<_>>();
                        let [index] = indices.as_slice() else {
                            return Err(ResourceExecutionError::MissingCheckedBody);
                        };
                        (call.definition, BodyRoot::Generic(*index))
                    }
                    _ => return Err(ResourceExecutionError::InvalidInvocation),
                }
            }
        };
        let function = self.retained_function(definition)?;
        let signature = invocation.signature(self)?;
        if signature.index() as usize >= self.program.checked().interner.len() {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        let Type::Function {
            params,
            view_params,
            return_type,
        } = self.program.checked().interner.resolve(signature)
        else {
            return Err(ResourceExecutionError::InvalidInvocation);
        };
        if params.len() != function.params.len()
            || view_params.len() != params.len()
            || function
                .params
                .iter()
                .zip(view_params)
                .any(|(parameter, view)| parameter.view != *view)
        {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        match root {
            BodyRoot::Ordinary => {
                if self.program.checked().definition_types.get(&definition) != Some(&signature) {
                    return Err(ResourceExecutionError::InvalidInvocation);
                }
            }
            BodyRoot::Generic(index) => {
                let concrete = self
                    .program
                    .checked()
                    .generic_function_instantiations
                    .get(index)
                    .ok_or(ResourceExecutionError::MissingCheckedBody)?;
                if concrete.parameter_types != *params || concrete.return_type != *return_type {
                    return Err(ResourceExecutionError::InvalidInvocation);
                }
            }
            BodyRoot::ActorHandler(_) => return Err(ResourceExecutionError::InvalidInvocation),
        }
        if let Some(source) = invocation.source() {
            if source.arguments().len() != params.len()
                || source.arguments().iter().any(|argument| {
                    params.get(argument.parameter_index) != Some(&argument.parameter_type)
                        || view_params.get(argument.parameter_index).copied()
                            != Some(
                                argument.callee_access == jett_typecheck::CheckedCalleeAccess::View,
                            )
                })
            {
                return Err(ResourceExecutionError::InvalidInvocation);
            }
        }
        let items = self
            .program
            .module()
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                matches!(item, Item::Function(original) if std::ptr::eq(original, function))
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        let [function_item] = items.as_slice() else {
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
                        item: *function_item,
                    },
                    path: Vec::new(),
                }),
            },
        };
        self.validate_executable_cursor(&reference.cursor)?;
        Ok(reference)
    }

    pub(super) fn validate_executable_cursor(
        &self,
        cursor: &CheckedBodyCursor,
    ) -> Result<(), ResourceExecutionError> {
        let identity = cursor
            .executable
            .as_ref()
            .ok_or(ResourceExecutionError::MissingCheckedBody)?;
        if !Arc::ptr_eq(&identity.program, &self.program) {
            return Err(ResourceExecutionError::ForeignProgram);
        }
        if matches!(identity.origin, ExecutableOrigin::ActorHandler { .. }) {
            return Err(ResourceExecutionError::WrongPurpose);
        }
        if (matches!(
            identity.origin,
            ExecutableOrigin::RefinementPredicate { .. }
        ) || identity
            .path
            .iter()
            .any(|step| matches!(step, RegionStep::InlineBody(_))))
            && self.purpose != ExecutionPurpose::ExplicitComptime
        {
            return Err(ResourceExecutionError::WrongPurpose);
        }
        self.validate_cursor_identity(cursor)
    }

    /// Ordinary field storage has no provider/custody route. Resource APIs keep
    /// calling validate_executable_cursor and retain their runtime-inline gate.
    pub(super) fn validate_field_executable_cursor(
        &self,
        cursor: &CheckedBodyCursor,
    ) -> Result<(), ResourceExecutionError> {
        self.validate_cursor_identity(cursor)
    }

    fn validate_cursor_identity(
        &self,
        cursor: &CheckedBodyCursor,
    ) -> Result<(), ResourceExecutionError> {
        let identity = cursor
            .executable
            .as_ref()
            .ok_or(ResourceExecutionError::MissingCheckedBody)?;
        if !Arc::ptr_eq(&identity.program, &self.program) {
            return Err(ResourceExecutionError::ForeignProgram);
        }
        // Required syntax selection carries no authority to execute an inline
        // callable or a refinement predicate as a runtime Resource entry.
        let reference = CheckedBodyReference {
            cursor: cursor.clone(),
        };
        match identity.origin {
            ExecutableOrigin::Function { definition, .. } => {
                let function = reference.function()?;
                if !std::ptr::eq(self.retained_function(definition)?, function) {
                    return Err(ResourceExecutionError::MissingCheckedBody);
                }
                match cursor.root {
                    BodyRoot::Ordinary if !function.type_params.is_empty() => {
                        return Err(ResourceExecutionError::MissingCheckedBody);
                    }
                    BodyRoot::Generic(index) => {
                        let instance = self
                            .program
                            .checked()
                            .generic_function_instantiations
                            .get(index)
                            .ok_or(ResourceExecutionError::MissingCheckedBody)?;
                        if instance.definition != definition
                            || instance.concrete_args.len() != function.type_params.len()
                        {
                            return Err(ResourceExecutionError::MissingCheckedBody);
                        }
                    }
                    BodyRoot::ActorHandler(_) => {
                        return Err(ResourceExecutionError::MissingCheckedBody);
                    }
                    BodyRoot::Ordinary => {}
                }
            }
            ExecutableOrigin::ActorHandler {
                definition,
                item,
                handler,
            } => {
                let BodyRoot::ActorHandler(target) = cursor.root else {
                    return Err(ResourceExecutionError::MissingCheckedBody);
                };
                if target.actor_definition != definition || target.handler_index != handler {
                    return Err(ResourceExecutionError::MissingCheckedBody);
                }
                self.validate_actor_handler_identity(target, item)?;
            }
            ExecutableOrigin::RefinementPredicate { definition, item } => {
                let Some(Item::TypeAlias(alias)) = self.program.module().items.get(item) else {
                    return Err(ResourceExecutionError::MissingCheckedBody);
                };
                if alias.constraint.is_none() || self.refinement_definition(alias)? != definition {
                    return Err(ResourceExecutionError::MissingCheckedBody);
                }
                if !matches!(cursor.root, BodyRoot::Ordinary) {
                    return Err(ResourceExecutionError::MissingCheckedBody);
                }
            }
            _ if !matches!(cursor.root, BodyRoot::Ordinary) => {
                return Err(ResourceExecutionError::MissingCheckedBody);
            }
            _ => {}
        }
        let path_scopes = identity
            .path
            .iter()
            .filter_map(|step| match step {
                RegionStep::Scope(owner) => Some(*owner),
                _ => None,
            })
            .collect::<Vec<_>>();
        if path_scopes
            != cursor
                .scopes
                .iter()
                .map(|scope| scope.owner)
                .collect::<Vec<_>>()
        {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        self.facts(cursor)?;
        reference.region()?;
        Ok(())
    }

    pub(crate) fn install_function_body(
        &mut self,
        reference: &CheckedBodyReference,
    ) -> Result<(), ResourceExecutionError> {
        self.validate_executable_cursor(&reference.cursor)?;
        self.body = reference.cursor.clone();
        Ok(())
    }

    pub(crate) fn install_direct_scope(
        &mut self,
        prepared: &PreparedDirectScope,
    ) -> Result<(), ResourceExecutionError> {
        self.install_function_body(&prepared.reference)
    }

    pub(crate) fn prepare_direct_scope(
        &self,
        bind: &ComptimeTypeBindStmt,
    ) -> Result<PreparedDirectScope, ResourceExecutionError> {
        self.validate_executable_cursor(&self.body)?;
        let prepared = self.prepare_original_direct_scope(bind)?;
        self.validate_executable_cursor(&prepared.reference.cursor)?;
        Ok(prepared)
    }

    /// Shares physical Source selection, without authorizing Resource entry.
    pub(crate) fn prepare_field_direct_scope(
        &self,
        bind: &ComptimeTypeBindStmt,
    ) -> Result<PreparedDirectScope, ResourceExecutionError> {
        self.validate_field_executable_cursor(&self.body)?;
        let prepared = self.prepare_original_direct_scope(bind)?;
        self.validate_field_executable_cursor(&prepared.reference.cursor)?;
        Ok(prepared)
    }

    fn prepare_original_direct_scope(
        &self,
        bind: &ComptimeTypeBindStmt,
    ) -> Result<PreparedDirectScope, ResourceExecutionError> {
        let parent = CheckedBodyReference {
            cursor: self.body.clone(),
        };
        let original = region_binding(parent.region()?, bind.span)?;
        if !std::ptr::eq(original, bind) {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        let Expr::GenericCall(_, source_types, arguments, occurrence) = &original.value else {
            return Err(ResourceExecutionError::MissingCheckedBody);
        };
        if source_types.len() != 1 {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        let facts = self.facts(&self.body)?;
        let intrinsic = facts
            .intrinsics
            .get(occurrence)
            .ok_or(ResourceExecutionError::MissingCheckedInvocation)?;
        let types = facts
            .intrinsic_types
            .get(occurrence)
            .ok_or(ResourceExecutionError::MissingCheckedBody)?;
        let reflections = facts
            .intrinsic_reflections
            .get(occurrence)
            .ok_or(ResourceExecutionError::MissingCheckedBody)?;
        let ([source_type], [source_reflection]) = (types.as_slice(), reflections.as_slice())
        else {
            return Err(ResourceExecutionError::MissingCheckedBody);
        };
        if source_type.index() as usize >= self.program.checked().interner.len() {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        let selected = facts
            .scopes
            .get(&original.span)
            .ok_or(ResourceExecutionError::MissingCheckedBody)?;
        let candidates = selected
            .iter()
            .enumerate()
            .filter(|(_, candidate)| {
                candidate.selection == CheckedComptimeTypeSelection::Unconditional
            })
            .collect::<Vec<_>>();
        let [(index, candidate)] = candidates.as_slice() else {
            return Err(ResourceExecutionError::MissingCheckedBody);
        };
        let reflection_matches = match intrinsic {
            IntrinsicId::TypeInfo if arguments.is_empty() => {
                candidate.bound_type == *source_type && candidate.reflection == *source_reflection
            }
            IntrinsicId::TypeArg if arguments.len() == 1 && arguments[0].name.is_none() => {
                let Expr::IntLiteral(index, _) = &arguments[0].value else {
                    return Err(ResourceExecutionError::MissingCheckedBody);
                };
                let index = usize::try_from(*index)
                    .map_err(|_| ResourceExecutionError::MissingCheckedBody)?;
                source_reflection.args.get(index) == Some(&candidate.reflection)
            }
            _ => false,
        };
        if !reflection_matches {
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
        Ok(PreparedDirectScope {
            reference: CheckedBodyReference { cursor },
            projection: scoped_projection(candidate, &self.program.checked().interner),
            reflection: candidate.reflection.clone(),
        })
    }

    /// True is mandatory checked dispatch, false is an exact checked constructor.
    /// A missing body/packet never selects the raw source-name path.
    #[cfg(test)]
    pub(crate) fn original_body_scope_depth(&self) -> Option<usize> {
        self.body
            .executable
            .as_ref()
            .map(|_| self.body.scopes.len())
    }

    pub(crate) fn original_assignment(
        &self,
        assignment: &AssignStmt,
    ) -> Result<(), ResourceExecutionError> {
        self.validate_executable_cursor(&self.body)?;
        let reference = CheckedBodyReference {
            cursor: self.body.clone(),
        };
        let mut matched = 0usize;
        walk_region(
            reference.region()?,
            &mut |_| {},
            &mut |_| {},
            &mut |statement| {
                if let Stmt::Assign(original) = statement {
                    if std::ptr::eq(original, assignment) {
                        matched += 1;
                    }
                }
            },
        );
        if matched == 1 {
            Ok(())
        } else {
            Err(ResourceExecutionError::MissingCheckedBody)
        }
    }

    pub(crate) fn source_call_dispatch(
        &self,
        callee: &Expr,
        arguments: &[CallArg],
        span: Span,
    ) -> Result<bool, ResourceExecutionError> {
        self.validate_executable_cursor(&self.body)?;
        let reference = CheckedBodyReference {
            cursor: self.body.clone(),
        };
        let mut matched = 0usize;
        walk_region(
            reference.region()?,
            &mut |_| {},
            &mut |expression| {
                if let Expr::Call(original_callee, original_args, original_span)
                | Expr::GenericCall(original_callee, _, original_args, original_span) =
                    expression
                {
                    if *original_span == span
                        && std::ptr::eq(original_callee.as_ref(), callee)
                        && original_args.len() == arguments.len()
                        && (arguments.is_empty()
                            || std::ptr::eq(original_args.as_ptr(), arguments.as_ptr()))
                    {
                        matched += 1;
                    }
                }
            },
            &mut |_| {},
        );
        if matched != 1 {
            return Err(ResourceExecutionError::MissingCheckedInvocation);
        }
        let facts = self.facts(&self.body)?;
        if facts.calls.contains_key(&span) {
            return Ok(true);
        }
        if facts.constructions.contains_key(&span) {
            return Ok(false);
        }
        if self.typed_machine_constructor(callee, arguments, span)? {
            return Ok(false);
        }
        Err(ResourceExecutionError::MissingCheckedInvocation)
    }

    // The checker records machine construction as an exact nominal/state
    // expression rather than a function invocation. Authenticate that existing
    // typed data path separately; it grants no live Resource custody transport.
    fn typed_machine_constructor(
        &self,
        callee: &Expr,
        arguments: &[CallArg],
        span: Span,
    ) -> Result<bool, ResourceExecutionError> {
        let definition = match self.resolved_definition(callee.span()) {
            Ok(definition) => definition,
            Err(_) => return Ok(false),
        };
        let info = self
            .program
            .resolved()
            .scope_table
            .definitions
            .get(definition.index() as usize)
            .ok_or(ResourceExecutionError::InvalidInvocation)?;
        if info.id != definition || info.kind != jett_resolve::DefKind::Machine {
            return Ok(false);
        }
        let types = &self.program.checked().interner;
        let declared = self
            .program
            .checked()
            .definition_types
            .get(&definition)
            .ok_or(ResourceExecutionError::MissingCheckedBody)?;
        let Type::Machine(machine) = types.resolve(*declared) else {
            return Err(ResourceExecutionError::InvalidInvocation);
        };
        let facts = self.facts(&self.body)?;
        let actual = facts
            .source_types
            .get(&span)
            .or_else(|| facts.types.get(&span))
            .ok_or(ResourceExecutionError::MissingCheckedBody)?;
        let Type::MachineState {
            machine: actual_machine,
            state,
        } = types.resolve(*actual)
        else {
            return Err(ResourceExecutionError::InvalidInvocation);
        };
        if machine != actual_machine {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        let state = types
            .resolve_machine(*machine)
            .state(*state)
            .ok_or(ResourceExecutionError::InvalidInvocation)?;
        let Some(first) = arguments.first() else {
            return Err(ResourceExecutionError::InvalidInvocation);
        };
        let Expr::Ident(selected) = &first.value else {
            return Err(ResourceExecutionError::InvalidInvocation);
        };
        if first.name.is_some()
            || selected.name != state.name
            || arguments.len() != state.fields.len() + 1
        {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        if arguments[1..]
            .iter()
            .zip(&state.fields)
            .any(|(argument, (_, expected))| {
                facts.types.get(&argument.value.span()) != Some(expected)
            })
        {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        Ok(true)
    }
}

fn scoped_projection(
    binding: &CheckedComptimeTypeBinding,
    types: &jett_types::TypeInterner,
) -> Arc<CheckedScopedTypes> {
    // Match the existing driver projection: descendants remain under their
    // lexical parent, and their expression names are not reused outside it.
    let descendants = binding
        .body
        .comptime_type_bindings
        .values()
        .flatten()
        .flat_map(|nested| nested.body.comptime_type_bindings.keys().copied())
        .collect::<std::collections::HashSet<_>>();
    let bindings: CheckedScopedBindings = binding
        .body
        .comptime_type_bindings
        .iter()
        .filter(|(owner, _)| !descendants.contains(owner))
        .map(|(owner, candidates)| {
            (
                *owner,
                candidates
                    .iter()
                    .map(|candidate| scoped_projection(candidate, types))
                    .collect(),
            )
        })
        .collect();
    let mut expressions = binding
        .body
        .type_map
        .iter()
        .map(|(span, ty)| (*span, types.type_name(*ty)))
        .collect();
    remove_descendant_expression_names(&mut expressions, &binding.body.comptime_type_bindings);
    Arc::new(CheckedScopedTypes {
        bound_type: types.type_name(binding.bound_type),
        reflection: Some(binding.reflection.clone()),
        expressions,
        bindings,
    })
}

fn remove_descendant_expression_names(
    expressions: &mut HashMap<Span, String>,
    bindings: &HashMap<Span, Vec<CheckedComptimeTypeBinding>>,
) {
    for binding in bindings.values().flatten() {
        for span in binding.body.type_map.keys() {
            expressions.remove(span);
        }
        remove_descendant_expression_names(expressions, &binding.body.comptime_type_bindings);
    }
}

fn walk_region<'a>(
    region: OriginalRegion<'a>,
    binding: &mut impl FnMut(&'a ComptimeTypeBindStmt),
    expression: &mut impl FnMut(&'a Expr),
    statement: &mut impl FnMut(&'a Stmt),
) {
    match region {
        OriginalRegion::Block(block) => walk_block_nodes(block, binding, expression, statement),
        OriginalRegion::Expression(value) => walk_expr_nodes(value, binding, expression, statement),
    }
}

fn region_binding(
    region: OriginalRegion<'_>,
    owner: Span,
) -> Result<&ComptimeTypeBindStmt, ResourceExecutionError> {
    let mut found = Vec::new();
    walk_region(
        region,
        &mut |binding| {
            if binding.span == owner {
                found.push(binding);
            }
        },
        &mut |_| {},
        &mut |_| {},
    );
    let [binding] = found.as_slice() else {
        return Err(ResourceExecutionError::MissingCheckedBody);
    };
    Ok(*binding)
}

fn region_expression(
    region: OriginalRegion<'_>,
    index: usize,
) -> Result<&Expr, ResourceExecutionError> {
    let mut found = Vec::new();
    walk_region(
        region,
        &mut |_| {},
        &mut |expression| {
            found.push(expression);
        },
        &mut |_| {},
    );
    found
        .get(index)
        .copied()
        .ok_or(ResourceExecutionError::MissingCheckedBody)
}

fn region_inline_body(
    region: OriginalRegion<'_>,
    index: usize,
) -> Result<&Block, ResourceExecutionError> {
    match region_expression(region, index)? {
        Expr::InlineFn(_, _, body, _) => Ok(body),
        _ => Err(ResourceExecutionError::MissingCheckedBody),
    }
}

fn expression_index(
    region: OriginalRegion<'_>,
    value: &Expr,
) -> Result<usize, ResourceExecutionError> {
    let mut indices = Vec::new();
    let mut ordinal = 0usize;
    walk_region(
        region,
        &mut |_| {},
        &mut |expression| {
            if std::ptr::eq(value, expression) {
                indices.push(ordinal);
            }
            ordinal += 1;
        },
        &mut |_| {},
    );
    match indices.as_slice() {
        [index] => Ok(*index),
        _ => Err(ResourceExecutionError::MissingCheckedBody),
    }
}

fn binding_in(block: &Block, owner: Span) -> Result<&ComptimeTypeBindStmt, ResourceExecutionError> {
    let mut bindings = Vec::new();
    walk_block(
        block,
        &mut |binding| {
            if binding.span == owner {
                bindings.push(binding);
            }
        },
        &mut |_| {},
    );
    let [binding] = bindings.as_slice() else {
        return Err(ResourceExecutionError::MissingCheckedBody);
    };
    Ok(*binding)
}

/// Traverse executable control-flow only; scoped and callable bodies require
/// their own selected reference. Their initializer expressions remain here.
fn walk_block<'a>(
    block: &'a Block,
    binding: &mut impl FnMut(&'a ComptimeTypeBindStmt),
    expression: &mut impl FnMut(&'a Expr),
) {
    walk_block_nodes(block, binding, expression, &mut |_| {});
}

fn walk_statements<'a>(block: &'a Block, statement: &mut impl FnMut(&'a Stmt)) {
    walk_block_nodes(block, &mut |_| {}, &mut |_| {}, statement);
}

fn walk_block_nodes<'a>(
    block: &'a Block,
    binding: &mut impl FnMut(&'a ComptimeTypeBindStmt),
    expression: &mut impl FnMut(&'a Expr),
    statement: &mut impl FnMut(&'a Stmt),
) {
    for node in &block.stmts {
        statement(node);
        match node {
            Stmt::VarDecl(value) => walk_expr_nodes(&value.value, binding, expression, statement),
            Stmt::Assign(value) => {
                walk_expr_nodes(&value.target, binding, expression, statement);
                walk_expr_nodes(&value.value, binding, expression, statement);
            }
            Stmt::Return(value) => {
                if let Some(value) = &value.value {
                    walk_expr_nodes(value, binding, expression, statement);
                }
            }
            Stmt::Respond(value) => walk_expr_nodes(&value.value, binding, expression, statement),
            Stmt::ComptimeTypeBind(value) => {
                binding(value);
                walk_expr_nodes(&value.value, binding, expression, statement);
            }
            Stmt::If(value) => {
                walk_expr_nodes(&value.condition, binding, expression, statement);
                walk_block_nodes(&value.then_block, binding, expression, statement);
                for (condition, block) in &value.else_ifs {
                    walk_expr_nodes(condition, binding, expression, statement);
                    walk_block_nodes(block, binding, expression, statement);
                }
                if let Some(block) = &value.else_block {
                    walk_block_nodes(block, binding, expression, statement);
                }
            }
            Stmt::For(value) => {
                walk_expr_nodes(&value.iterable, binding, expression, statement);
                walk_block_nodes(&value.body, binding, expression, statement);
            }
            Stmt::While(value) => {
                walk_expr_nodes(&value.condition, binding, expression, statement);
                walk_block_nodes(&value.body, binding, expression, statement);
            }
            Stmt::Match(value) => {
                walk_expr_nodes(&value.expr, binding, expression, statement);
                for arm in &value.arms {
                    walk_block_nodes(&arm.body, binding, expression, statement);
                }
            }
            Stmt::Expr(value) => walk_expr_nodes(&value.expr, binding, expression, statement),
            Stmt::Assert(value) => {
                walk_expr_nodes(&value.condition, binding, expression, statement);
                if let Some(message) = &value.message {
                    walk_expr_nodes(message, binding, expression, statement);
                }
            }
            Stmt::Breakpoint(value) => {
                if let Some(condition) = &value.condition {
                    walk_expr_nodes(condition, binding, expression, statement);
                }
            }
            Stmt::Use(_) | Stmt::Trace(_) | Stmt::Break(_) | Stmt::Continue(_) => {}
        }
    }
}

fn walk_expr_nodes<'a>(
    value: &'a Expr,
    binding: &mut impl FnMut(&'a ComptimeTypeBindStmt),
    expression: &mut impl FnMut(&'a Expr),
    statement: &mut impl FnMut(&'a Stmt),
) {
    expression(value);
    match value {
        Expr::Binary(left, _, right, _) => {
            walk_expr_nodes(left, binding, expression, statement);
            walk_expr_nodes(right, binding, expression, statement);
        }
        Expr::Unary(_, value, _)
        | Expr::FieldAccess(value, _, _)
        | Expr::Paren(value, _)
        | Expr::View(value, _)
        | Expr::Comptime(value, _)
        | Expr::Ok(value, _)
        | Expr::Fail(value, _)
        | Expr::Some(value, _)
        | Expr::Default(value, _)
        | Expr::Declassify(value, _)
        | Expr::Coarsen(value, _)
        | Expr::At(value, _, _)
        | Expr::Spawn(value, _)
        | Expr::Send(value, _)
        | Expr::Ask(value, _)
        | Expr::Clone(value, _)
        | Expr::Run(value, _)
        | Expr::Join(value, _)
        | Expr::Cancel(value, _) => walk_expr_nodes(value, binding, expression, statement),
        Expr::Call(callee, arguments, _) | Expr::GenericCall(callee, _, arguments, _) => {
            walk_expr_nodes(callee, binding, expression, statement);
            for argument in arguments {
                walk_expr_nodes(&argument.value, binding, expression, statement);
            }
        }
        Expr::ListConstruct(values, _) => {
            for value in values {
                walk_expr_nodes(value, binding, expression, statement);
            }
        }
        Expr::MapConstruct(values, _) => {
            for (key, value) in values {
                walk_expr_nodes(key, binding, expression, statement);
                walk_expr_nodes(value, binding, expression, statement);
            }
        }
        Expr::Handle(value, _, block, _) => {
            walk_expr_nodes(value, binding, expression, statement);
            walk_block_nodes(block, binding, expression, statement);
        }
        Expr::StringInterpolation(parts, _) => {
            for part in parts {
                if let StringPart::Expr(value) = part {
                    walk_expr_nodes(value, binding, expression, statement);
                }
            }
        }
        Expr::Pipeline(value, steps, _) => {
            walk_expr_nodes(value, binding, expression, statement);
            for step in steps {
                walk_expr_nodes(&step.function, binding, expression, statement);
                for argument in &step.extra_args {
                    walk_expr_nodes(&argument.value, binding, expression, statement);
                }
                if let Some(handle) = &step.handle {
                    walk_block_nodes(&handle.body, binding, expression, statement);
                }
            }
        }
        Expr::InlineFn(_, _, _, _)
        | Expr::IntLiteral(_, _)
        | Expr::FloatLiteral(_, _)
        | Expr::StringLiteral(_, _)
        | Expr::BoolLiteral(_, _)
        | Expr::Nothing(_)
        | Expr::Ident(_)
        | Expr::None(_)
        | Expr::EnumVariant(_, _, _)
        | Expr::Error(_) => {}
    }
}

#[cfg(test)]
#[path = "body_reference/tests.rs"]
mod tests;
