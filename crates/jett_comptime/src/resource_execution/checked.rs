#[path = "checked/assignment.rs"]
mod assignment;
pub(crate) use assignment::CheckedAssignment;

#[path = "checked/body_reference.rs"]
mod body_reference;
pub(crate) use body_reference::{
    CheckedAttemptKey, CheckedBodyReference, PreparedDirectScope, PreparedIntrinsicArguments,
    PreparedNamedCallable, PreparedPipelineStep, PreparedRequiredExpression,
};
use std::collections::HashMap;
use std::sync::Arc;

use jett_common::Span;
use jett_resolve::DefId;
use jett_runtime::ResourceTypeId;
use jett_typecheck::{
    CheckedArgumentOwnership, CheckedBindingFact, CheckedBodyFacts, CheckedCallOwnership,
    CheckedComptimeTypeBinding, CheckedComptimeTypeSelection, CheckedGenericCall,
    CheckedInvocationShape, CheckedInvocationTarget, CheckedResourceHook, CheckedResourceProgram,
    validate_resource_hooks,
};
use jett_types::{ReflectionTypeInfo, ResourceHookKind, ResourceKernelRecipe, Type, TypeId};

use super::{ResourceExecutionError, ResourceHookDescriptor, ResourceTypeBinding};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionPurpose {
    ReferenceRuntime,
    NamespaceConstant,
    ExplicitComptime,
    Verify,
    Property,
}

#[derive(Debug, Clone)]
enum BodyRoot {
    Ordinary,
    Generic(usize),
}

#[derive(Debug, Clone)]
struct ScopedSelection {
    owner: Span,
    index: usize,
    bound_type: TypeId,
    selection: CheckedComptimeTypeSelection,
    reflection: ReflectionTypeInfo,
}

#[derive(Debug, Clone)]
pub(crate) struct CheckedBodyCursor {
    root: BodyRoot,
    scopes: Vec<ScopedSelection>,
    executable: Option<body_reference::ExecutableIdentity>,
}

impl CheckedBodyCursor {
    fn ordinary() -> Self {
        Self {
            root: BodyRoot::Ordinary,
            scopes: Vec::new(),
            executable: None,
        }
    }
}

pub(crate) struct CheckedInvocation {
    packet: CheckedCallOwnership,
    body: CheckedBodyCursor,
    span: Span,
}

impl CheckedInvocation {
    pub(crate) fn packet(&self) -> &CheckedCallOwnership {
        &self.packet
    }
    pub(crate) fn arguments(&self) -> &[CheckedArgumentOwnership] {
        &self.packet.arguments
    }
    pub(crate) fn target(&self) -> &CheckedInvocationTarget {
        &self.packet.target
    }
    pub(crate) fn span(&self) -> Span {
        self.span
    }
}

pub(crate) struct FunctionParameter {
    pub(crate) index: usize,
    pub(crate) ty: TypeId,
    pub(crate) access: jett_typecheck::CheckedCalleeAccess,
}

/// External checked entry is not a fabricated source call packet.
pub(crate) enum FunctionInvocation<'a> {
    Source(&'a CheckedInvocation),
    NamedSource {
        source: &'a CheckedInvocation,
        target: &'a PreparedNamedCallable,
    },
    Entry {
        definition: DefId,
        signature: TypeId,
    },
}

impl FunctionInvocation<'_> {
    pub(crate) fn source(&self) -> Option<&CheckedInvocation> {
        match self {
            Self::Source(source) | Self::NamedSource { source, .. } => Some(source),
            Self::Entry { .. } => None,
        }
    }

    pub(crate) fn signature(
        &self,
        checked: &CheckedExecution,
    ) -> Result<TypeId, ResourceExecutionError> {
        match self {
            Self::Source(invocation)
            | Self::NamedSource {
                source: invocation, ..
            } => match &invocation.packet.shape {
                CheckedInvocationShape::Function { signature_type } => Ok(*signature_type),
                CheckedInvocationShape::Intrinsic { .. } => {
                    Err(ResourceExecutionError::InvalidInvocation)
                }
            },
            Self::Entry {
                definition,
                signature,
            } if checked.program.checked().definition_types.get(definition) == Some(signature) => {
                Ok(*signature)
            }
            Self::Entry { .. } => Err(ResourceExecutionError::InvalidInvocation),
        }
    }
    pub(crate) fn parameters(
        &self,
        checked: &CheckedExecution,
    ) -> Result<Vec<FunctionParameter>, ResourceExecutionError> {
        let signature = self.signature(checked)?;
        let Type::Function {
            params,
            view_params,
            ..
        } = checked.program.checked().interner.resolve(signature)
        else {
            return Err(ResourceExecutionError::InvalidInvocation);
        };
        match self {
            Self::Source(invocation)
            | Self::NamedSource {
                source: invocation, ..
            } => Ok(invocation
                .arguments()
                .iter()
                .map(|argument| FunctionParameter {
                    index: argument.parameter_index,
                    ty: argument.parameter_type,
                    access: argument.callee_access,
                })
                .collect()),
            Self::Entry { .. } => Ok(params
                .iter()
                .zip(view_params)
                .enumerate()
                .map(|(index, (ty, view))| FunctionParameter {
                    index,
                    ty: *ty,
                    access: if *view {
                        jett_typecheck::CheckedCalleeAccess::View
                    } else {
                        jett_typecheck::CheckedCalleeAccess::Owned
                    },
                })
                .collect()),
        }
    }
}

pub(crate) struct CheckedExecution {
    program: Arc<CheckedResourceProgram>,
    purpose: ExecutionPurpose,
    body: CheckedBodyCursor,
    resources: HashMap<DefId, ResourceTypeBinding>,
}

struct BodyFacts<'a> {
    calls: &'a HashMap<Span, CheckedCallOwnership>,
    bindings: &'a HashMap<Span, CheckedBindingFact>,
    types: &'a HashMap<Span, TypeId>,
    source_types: &'a HashMap<Span, TypeId>,
    pipeline_inputs: &'a HashMap<Span, TypeId>,
    pipeline_calls: &'a HashMap<Span, TypeId>,
    scopes: &'a HashMap<Span, Vec<CheckedComptimeTypeBinding>>,
    intrinsics: &'a HashMap<Span, jett_intrinsics::IntrinsicId>,
    intrinsic_types: &'a HashMap<Span, Vec<TypeId>>,
    intrinsic_reflections: &'a HashMap<Span, Vec<ReflectionTypeInfo>>,
    constructions: &'a HashMap<Span, jett_typecheck::CheckedStructConstruction>,
}

impl<'a> BodyFacts<'a> {
    fn scoped(body: &'a CheckedBodyFacts) -> Self {
        Self {
            calls: &body.call_ownership,
            bindings: &body.binding_facts,
            types: &body.type_map,
            source_types: &body.source_type_map,
            pipeline_inputs: &body.pipeline_step_input_types,
            pipeline_calls: &body.pipeline_step_call_types,
            scopes: &body.comptime_type_bindings,
            intrinsics: &body.intrinsic_ids,
            intrinsic_types: &body.intrinsic_type_arguments,
            intrinsic_reflections: &body.intrinsic_reflection_arguments,
            constructions: &body.struct_constructions,
        }
    }
}

// A present resolution is authoritative only after it rejoins this exact
// retained executable declaration. Ordinary declarations have no such key.
fn retained_declaration_definition(
    module: &jett_parser::ast::Module,
    definitions: &[jett_resolve::DefInfo],
    resolutions: &HashMap<Span, DefId>,
    declaration: Span,
) -> Result<DefId, ResourceExecutionError> {
    use jett_parser::ast::Item;
    use jett_resolve::DefKind;

    let mut actuals = module
        .items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| match item {
            Item::Function(function) if function.name.span == declaration => {
                Some((index, function))
            }
            _ => None,
        });
    let (index, function) = actuals
        .next()
        .ok_or(ResourceExecutionError::InvalidInvocation)?;
    if actuals.next().is_some() {
        return Err(ResourceExecutionError::InvalidInvocation);
    }
    let namespace_at = |index: usize| {
        module.items[..index]
            .iter()
            .filter_map(|item| match item {
                Item::Namespace(namespace) if namespace.span.file == declaration.file => {
                    Some(namespace.name.name.as_str())
                }
                _ => None,
            })
            .last()
    };
    let namespace = namespace_at(index);
    let definition = match resolutions.get(&declaration) {
        Some(definition) => *definition,
        None => {
            let mut candidates = definitions
                .iter()
                .filter(|info| info.kind == DefKind::Function && info.span == declaration);
            let info = candidates
                .next()
                .ok_or(ResourceExecutionError::InvalidInvocation)?;
            if candidates.next().is_some() {
                return Err(ResourceExecutionError::InvalidInvocation);
            }
            info.id
        }
    };
    let info = definitions
        .get(definition.index() as usize)
        .ok_or(ResourceExecutionError::InvalidInvocation)?;
    let canonical_name = namespace
        .map(|namespace| format!("{namespace}.{}", function.name.name))
        .unwrap_or_else(|| function.name.name.clone());
    if info.id != definition
        || info.kind != DefKind::Function
        || info.namespace.as_deref() != namespace
        || info.name != canonical_name
    {
        return Err(ResourceExecutionError::InvalidInvocation);
    }
    if info.span != declaration {
        // Mutual definitions resolve to their original forward declaration.
        // Rejoin that exact retained node, not another same-signature function.
        let forwards = module
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| match item {
                Item::Mutual(block) => Some((index, block)),
                _ => None,
            })
            .flat_map(|(index, block)| {
                block
                    .declarations
                    .iter()
                    .map(move |forward| (index, forward))
            })
            .filter(|(index, forward)| {
                forward.name.span == info.span
                    && forward.name.name == function.name.name
                    && namespace_at(*index) == namespace
            })
            .count();
        if forwards != 1 {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
    }
    Ok(definition)
}

impl CheckedExecution {
    pub(crate) fn new(
        program: Arc<CheckedResourceProgram>,
        purpose: ExecutionPurpose,
    ) -> Result<Self, ResourceExecutionError> {
        validate_resource_hooks(program.module(), program.resolved(), program.checked())
            .map_err(|_| ResourceExecutionError::InvalidCheckedProgram)?;
        let mut resources: HashMap<DefId, ResourceTypeBinding> = HashMap::new();
        for hook in program.checked().resource_hooks.values() {
            if let Some(existing) = resources.get(&hook.resource_definition) {
                if existing.checked_type != hook.resource_type {
                    return Err(ResourceExecutionError::InvalidCheckedProgram);
                }
                continue;
            }
            // A per-program registry type. The registry itself binds a runtime
            // context, so equal numeric IDs in independent sessions confer none.
            let number = u32::try_from(resources.len())
                .map_err(|_| ResourceExecutionError::IdentityExhausted)?;
            resources.insert(
                hook.resource_definition,
                ResourceTypeBinding {
                    definition: hook.resource_definition,
                    checked_type: hook.resource_type,
                    registry_type: ResourceTypeId::new(number),
                },
            );
        }
        Ok(Self {
            program,
            purpose,
            body: CheckedBodyCursor::ordinary(),
            resources,
        })
    }

    pub(crate) fn program(&self) -> &Arc<CheckedResourceProgram> {
        &self.program
    }
    pub(crate) fn purpose(&self) -> ExecutionPurpose {
        self.purpose
    }
    pub(crate) fn replace_purpose(&mut self, purpose: ExecutionPurpose) -> ExecutionPurpose {
        std::mem::replace(&mut self.purpose, purpose)
    }
    pub(crate) fn cursor(&self) -> CheckedBodyCursor {
        self.body.clone()
    }

    fn facts(&self, cursor: &CheckedBodyCursor) -> Result<BodyFacts<'_>, ResourceExecutionError> {
        let checked = self.program.checked();
        let mut facts = match cursor.root {
            BodyRoot::Ordinary => BodyFacts {
                calls: &checked.call_ownership,
                bindings: &checked.binding_facts,
                types: &checked.type_map,
                source_types: &checked.source_type_map,
                pipeline_inputs: &checked.pipeline_step_input_types,
                pipeline_calls: &checked.pipeline_step_call_types,
                scopes: &checked.comptime_type_bindings,
                intrinsics: &checked.intrinsic_ids,
                intrinsic_types: &checked.intrinsic_type_arguments,
                intrinsic_reflections: &checked.intrinsic_reflection_arguments,
                constructions: &checked.struct_constructions,
            },
            BodyRoot::Generic(index) => {
                let body = checked
                    .generic_function_instantiations
                    .get(index)
                    .ok_or(ResourceExecutionError::MissingCheckedBody)?;
                BodyFacts {
                    calls: &body.call_ownership,
                    bindings: &body.binding_facts,
                    types: &body.type_map,
                    source_types: &body.source_type_map,
                    pipeline_inputs: &body.pipeline_step_input_types,
                    pipeline_calls: &body.pipeline_step_call_types,
                    scopes: &body.comptime_type_bindings,
                    intrinsics: &body.intrinsic_ids,
                    intrinsic_types: &body.intrinsic_type_arguments,
                    intrinsic_reflections: &body.intrinsic_reflection_arguments,
                    constructions: &body.struct_constructions,
                }
            }
        };
        for selection in &cursor.scopes {
            let body = facts
                .scopes
                .get(&selection.owner)
                .and_then(|scopes| scopes.get(selection.index))
                .ok_or(ResourceExecutionError::MissingCheckedBody)?;
            if body.bound_type != selection.bound_type
                || body.selection != selection.selection
                || body.reflection != selection.reflection
            {
                return Err(ResourceExecutionError::MissingCheckedBody);
            }
            facts = BodyFacts::scoped(&body.body);
        }
        Ok(facts)
    }

    pub(crate) fn restore_cursor(
        &mut self,
        cursor: CheckedBodyCursor,
    ) -> Result<(), ResourceExecutionError> {
        self.facts(&cursor)?;
        if cursor.executable.is_some() {
            self.validate_executable_cursor(&cursor)?;
        }
        self.body = cursor;
        Ok(())
    }

    pub(crate) fn enter_ordinary(&mut self) {
        self.body = CheckedBodyCursor::ordinary();
    }

    pub(crate) fn enter_generic(
        &mut self,
        call: &CheckedGenericCall,
    ) -> Result<(), ResourceExecutionError> {
        let matches = self
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
        let [index] = matches.as_slice() else {
            return Err(ResourceExecutionError::MissingCheckedBody);
        };
        self.body = CheckedBodyCursor {
            root: BodyRoot::Generic(*index),
            scopes: Vec::new(),
            executable: None,
        };
        Ok(())
    }

    pub(crate) fn invocation(
        &self,
        span: Span,
    ) -> Result<CheckedInvocation, ResourceExecutionError> {
        let facts = self.facts(&self.body)?;
        let packet = facts
            .calls
            .get(&span)
            .ok_or(ResourceExecutionError::MissingCheckedInvocation)?;
        let mut sources = vec![false; packet.arguments.len()];
        let mut parameters = vec![false; packet.arguments.len()];
        for (index, argument) in packet.arguments.iter().enumerate() {
            if argument.source_index != index
                || argument.parameter_index >= parameters.len()
                || parameters[argument.parameter_index]
            {
                return Err(ResourceExecutionError::InvalidInvocation);
            }
            sources[index] = true;
            parameters[argument.parameter_index] = true;
            if facts.source_types.get(&argument.source_span) != Some(&argument.actual_type) {
                return Err(ResourceExecutionError::InvalidInvocation);
            }
        }
        if sources.iter().any(|present| !present) || parameters.iter().any(|present| !present) {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        Ok(CheckedInvocation {
            packet: packet.clone(),
            body: self.body.clone(),
            span,
        })
    }

    pub(crate) fn enum_constructor(
        &self,
        callee: &jett_parser::ast::Expr,
        invocation: &CheckedInvocation,
    ) -> Result<Option<(String, String, TypeId)>, ResourceExecutionError> {
        use jett_parser::ast::Expr;
        use jett_resolve::DefKind;
        let CheckedInvocationTarget::Indirect(signature) = invocation.target() else {
            return Ok(None);
        };
        let facts = self.facts(&invocation.body)?;
        if facts.calls.get(&invocation.span) != Some(&invocation.packet) {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        let (owner, variant) = match callee {
            Expr::EnumVariant(owner, variant, _) => (owner.span, variant),
            Expr::FieldAccess(owner, variant, _) => (owner.span(), variant),
            _ => return Ok(None),
        };
        let Some(definition) = self.program.resolved().resolutions.get(&owner) else {
            return Ok(None);
        };
        let info = self
            .program
            .resolved()
            .scope_table
            .definitions
            .get(definition.index() as usize)
            .ok_or(ResourceExecutionError::InvalidInvocation)?;
        if info.id != *definition {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        if !matches!(info.kind, DefKind::Enum | DefKind::Type) {
            return Ok(None);
        }
        if signature.index() as usize >= self.program.checked().interner.len() {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        let Type::Function {
            params,
            view_params,
            return_type,
        } = self.program.checked().interner.resolve(*signature)
        else {
            return Err(ResourceExecutionError::InvalidInvocation);
        };
        if invocation.packet.shape
            != (CheckedInvocationShape::Function {
                signature_type: *signature,
            })
            || facts.source_types.get(&invocation.span) != Some(return_type)
            || facts.types.get(&callee.span()) != Some(signature)
            || self.program.checked().definition_types.get(definition) != Some(return_type)
        {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        let Type::Enum(enum_id) = self.program.checked().interner.resolve(*return_type) else {
            return Ok(None);
        };
        let nominal = self.program.checked().interner.resolve_enum(*enum_id);
        let variants = nominal
            .variants
            .iter()
            .filter(|candidate| candidate.name == variant.name)
            .collect::<Vec<_>>();
        let [selected] = variants.as_slice() else {
            return Err(ResourceExecutionError::InvalidInvocation);
        };
        if params.len() != selected.fields.len()
            || view_params.len() != params.len()
            || view_params.iter().any(|view| *view)
            || params
                .iter()
                .zip(&selected.fields)
                .any(|(parameter, (_, field))| parameter != field)
            || invocation.arguments().len() != params.len()
            || invocation.arguments().iter().any(|argument| {
                params.get(argument.parameter_index) != Some(&argument.parameter_type)
                    || argument.callee_access != jett_typecheck::CheckedCalleeAccess::Owned
            })
        {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        Ok(Some((
            nominal.name.clone(),
            selected.name.clone(),
            *return_type,
        )))
    }

    pub(crate) fn entry(
        &self,
        definition: DefId,
    ) -> Result<FunctionInvocation<'static>, ResourceExecutionError> {
        let definition_info = self
            .program
            .resolved()
            .scope_table
            .definitions
            .get(definition.index() as usize)
            .ok_or(ResourceExecutionError::InvalidInvocation)?;
        if definition_info.id != definition
            || definition_info.kind != jett_resolve::DefKind::Function
            || self.has_hook(definition)
        {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        let signature = *self
            .program
            .checked()
            .definition_types
            .get(&definition)
            .ok_or(ResourceExecutionError::InvalidInvocation)?;
        if !matches!(
            self.program.checked().interner.resolve(signature),
            Type::Function { .. }
        ) {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        Ok(FunctionInvocation::Entry {
            definition,
            signature,
        })
    }

    pub(crate) fn referenced_in(&self, definition: DefId, body: Span) -> bool {
        self.program
            .resolved()
            .resolutions
            .iter()
            .any(|(span, resolved)| {
                *resolved == definition
                    && span.file == body.file
                    && span.start >= body.start
                    && span.end <= body.end
            })
    }

    pub(crate) fn declaration_definition(
        &self,
        declaration: Span,
    ) -> Result<DefId, ResourceExecutionError> {
        let definition = retained_declaration_definition(
            self.program.module(),
            &self.program.resolved().scope_table.definitions,
            &self.program.resolved().resolutions,
            declaration,
        )?;
        if self.has_hook(definition) {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        Ok(definition)
    }

    /// Return only the executable node owned by this exact checked program.
    /// Runtime registration clones never provide a replacement body.
    pub(crate) fn retained_function(
        &self,
        definition: DefId,
    ) -> Result<&jett_parser::ast::FunctionDef, ResourceExecutionError> {
        if self.has_hook(definition) {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        let mut matches = self
            .program
            .module()
            .items
            .iter()
            .filter_map(|item| match item {
                jett_parser::ast::Item::Function(function)
                    if self.declaration_definition(function.name.span) == Ok(definition) =>
                {
                    Some(function)
                }
                _ => None,
            });
        let function = matches
            .next()
            .ok_or(ResourceExecutionError::InvalidInvocation)?;
        if matches.next().is_some() {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        if function.type_params.is_empty() {
            let signature = self.entry(definition)?.signature(self)?;
            let Type::Function {
                params,
                view_params,
                ..
            } = self.program.checked().interner.resolve(signature)
            else {
                return Err(ResourceExecutionError::InvalidInvocation);
            };
            if params.len() != function.params.len()
                || view_params.len() != function.params.len()
                || !view_params
                    .iter()
                    .copied()
                    .eq(function.params.iter().map(|parameter| parameter.view))
            {
                return Err(ResourceExecutionError::InvalidInvocation);
            }
        } else if !self
            .program
            .checked()
            .generic_function_instantiations
            .iter()
            .any(|body| {
                body.definition == definition
                    && body.concrete_args.len() == function.type_params.len()
                    && body.parameter_types.len() == function.params.len()
            })
        {
            // Generic templates have no fabricated definition_types entry.
            // Exact concrete args/specialization/signature remain invocation
            // and enter_generic obligations, independent of this body join.
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        Ok(function)
    }

    pub(crate) fn source_function_context(
        &self,
        declaration: Span,
    ) -> Result<(Option<String>, bool), ResourceExecutionError> {
        let definition = self.declaration_definition(declaration)?;
        let info = &self.program.resolved().scope_table.definitions[definition.index() as usize];
        let trusted = self.program.source_origins().get(&declaration.file)
            == Some(&jett_common::SourceOrigin::Stdlib);
        Ok((info.namespace.clone(), trusted))
    }

    pub(crate) fn type_contains_resource(
        &self,
        ty: TypeId,
    ) -> Result<bool, ResourceExecutionError> {
        fn visit(
            types: &jett_types::TypeInterner,
            ty: TypeId,
            seen: &mut std::collections::HashSet<TypeId>,
        ) -> Result<bool, ResourceExecutionError> {
            if ty.index() as usize >= types.len() {
                return Err(ResourceExecutionError::InvalidCheckedProgram);
            }
            if !seen.insert(ty) {
                return Ok(false);
            }
            let children: Vec<TypeId> = match types.resolve(ty) {
                Type::Resource(_) => return Ok(true),
                Type::List(inner)
                | Type::Set(inner)
                | Type::Optional(inner)
                | Type::Secret(inner)
                | Type::Refinement { base: inner, .. } => vec![*inner],
                Type::Map(key, value) | Type::Result(key, value) => vec![*key, *value],
                Type::Function {
                    params,
                    return_type,
                    ..
                } => params
                    .iter()
                    .copied()
                    .chain(std::iter::once(*return_type))
                    .collect(),
                Type::Struct(owner) => types
                    .resolve_struct(*owner)
                    .fields
                    .iter()
                    .map(|field| field.1)
                    .collect(),
                Type::Enum(owner) => types
                    .resolve_enum(*owner)
                    .variants
                    .iter()
                    .flat_map(|variant| variant.fields.iter().map(|field| field.1))
                    .collect(),
                Type::Machine(owner) => types
                    .resolve_machine(*owner)
                    .states
                    .iter()
                    .flat_map(|state| state.fields.iter().map(|field| field.1))
                    .collect(),
                Type::MachineState { machine, state } => types
                    .resolve_machine(*machine)
                    .state(*state)
                    .ok_or(ResourceExecutionError::InvalidCheckedProgram)?
                    .fields
                    .iter()
                    .map(|field| field.1)
                    .collect(),
                Type::Bitfield(owner) => types
                    .resolve_bitfield(*owner)
                    .fields
                    .iter()
                    .map(|field| field.ty)
                    .collect(),
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
                | Type::Bytes
                | Type::Nothing
                | Type::TypeConstruction
                | Type::Never
                | Type::Capability(_)
                | Type::Interface(_)
                | Type::Actor(_) => Vec::new(),
                Type::Error => return Err(ResourceExecutionError::InvalidCheckedProgram),
            };
            for child in children {
                if visit(types, child, seen)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        visit(
            &self.program.checked().interner,
            ty,
            &mut std::collections::HashSet::new(),
        )
    }

    pub(crate) fn expression_type(&self, span: Span) -> Result<TypeId, ResourceExecutionError> {
        self.facts(&self.body)?
            .types
            .get(&span)
            .copied()
            .ok_or(ResourceExecutionError::MissingCheckedBody)
    }

    pub(crate) fn resolved_definition(&self, span: Span) -> Result<DefId, ResourceExecutionError> {
        self.program
            .resolved()
            .resolutions
            .get(&span)
            .copied()
            .ok_or(ResourceExecutionError::InvalidInvocation)
    }

    pub(crate) fn has_hook(&self, definition: DefId) -> bool {
        self.program
            .checked()
            .resource_hooks
            .contains_key(&definition)
    }

    pub(crate) fn has_invocation(&self, span: Span) -> Result<bool, ResourceExecutionError> {
        Ok(self.facts(&self.body)?.calls.contains_key(&span))
    }

    pub(crate) fn binding(
        &self,
        declaration: Span,
    ) -> Result<CheckedBindingFact, ResourceExecutionError> {
        self.facts(&self.body)?
            .bindings
            .get(&declaration)
            .copied()
            .ok_or(ResourceExecutionError::MissingCheckedBody)
    }

    pub(crate) fn descriptor(
        &self,
        definition: DefId,
    ) -> Result<ResourceHookDescriptor, ResourceExecutionError> {
        self.program
            .checked()
            .resource_hooks
            .get(&definition)
            .ok_or(ResourceExecutionError::InvalidInvocation)?;
        Ok(ResourceHookDescriptor {
            program: self.program.clone(),
            definition,
        })
    }

    pub(crate) fn reached_hook(
        &self,
        invocation: &CheckedInvocation,
        descriptor: &ResourceHookDescriptor,
    ) -> Result<
        (
            CheckedResourceHook,
            ResourceTypeBinding,
            ResourceKernelRecipe,
        ),
        ResourceExecutionError,
    > {
        if !Arc::ptr_eq(&self.program, &descriptor.program) {
            return Err(ResourceExecutionError::ForeignProgram);
        }
        // Retained lexical cursor is validated independently of the function
        // currently running after nested actual evaluation.
        let facts = self.facts(&invocation.body)?;
        if facts.calls.get(&invocation.span) != Some(&invocation.packet) {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        let hook = self
            .program
            .checked()
            .resource_hooks
            .get(&descriptor.definition)
            .ok_or(ResourceExecutionError::InvalidInvocation)?;
        let target_matches = match &invocation.packet.target {
            CheckedInvocationTarget::Resolved(definition) => *definition == hook.definition,
            CheckedInvocationTarget::Indirect(function_type) => {
                *function_type == hook.function_type
            }
            CheckedInvocationTarget::Generic(_)
            | CheckedInvocationTarget::Method(_)
            | CheckedInvocationTarget::Interface(_)
            | CheckedInvocationTarget::Intrinsic(_) => false,
        };
        if !target_matches
            || invocation.packet.shape
                != (CheckedInvocationShape::Function {
                    signature_type: hook.function_type,
                })
        {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        let resource = *self
            .resources
            .get(&hook.resource_definition)
            .ok_or(ResourceExecutionError::InvalidCheckedProgram)?;
        if resource.checked_type != hook.resource_type {
            return Err(ResourceExecutionError::InvalidCheckedProgram);
        }
        let recipe = match hook.kind {
            ResourceHookKind::Construct => ResourceKernelRecipe::NetworkFactory,
            ResourceHookKind::BorrowOperation => ResourceKernelRecipe::NetworkBorrow,
            ResourceHookKind::Close => ResourceKernelRecipe::Finalize,
        };
        if !recipe.matches_signature(
            &self.program.checked().interner,
            hook.resource_type,
            hook.function_type,
        ) {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        let Type::Function {
            params,
            view_params,
            ..
        } = self.program.checked().interner.resolve(hook.function_type)
        else {
            return Err(ResourceExecutionError::InvalidInvocation);
        };
        if invocation.packet.arguments.len() != params.len() {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        for argument in &invocation.packet.arguments {
            let index = argument.parameter_index;
            let view = argument.callee_access == jett_typecheck::CheckedCalleeAccess::View;
            if params[index] != argument.parameter_type || view_params[index] != view {
                return Err(ResourceExecutionError::InvalidInvocation);
            }
        }
        if self.purpose != ExecutionPurpose::ReferenceRuntime {
            return Err(ResourceExecutionError::WrongPurpose);
        }
        Ok((hook.clone(), resource, recipe))
    }
}

#[cfg(test)]
mod declaration_tests {
    use super::*;
    use jett_parser::ast::Item;
    use jett_resolve::DefKind;

    const ORDINARY: &str = r#"namespace other
export function answer() returns int64:
    return 99
namespace app
export function answer() returns int64:
    return 7
"#;

    const MUTUAL: &str = r#"namespace app
mutual:
    function first() returns int64
    function second() returns int64
function first() returns int64:
    return second()
function second() returns int64:
    return 11
export function scenario() returns int64:
    return first()
"#;

    fn original<'a>(
        program: &'a CheckedResourceProgram,
        namespace: &str,
        name: &str,
    ) -> &'a jett_parser::ast::FunctionDef {
        let mut functions = program.module().items.iter().filter_map(|item| match item {
            Item::Function(function)
                if function.name.name == name
                    && program
                        .resolved()
                        .scope_table
                        .definitions
                        .iter()
                        .any(|info| {
                            info.kind == DefKind::Function
                                && info.span == function.name.span
                                && info.namespace.as_deref() == Some(namespace)
                        }) =>
            {
                Some(function)
            }
            _ => None,
        });
        let function = functions
            .next()
            .expect("unique original ordinary declaration");
        assert!(functions.next().is_none());
        function
    }

    #[test]
    fn retained_declaration_ordinary_has_no_resolution_and_no_name_fallback() {
        for release in [false, true] {
            let program = crate::resource_execution::tests::program(ORDINARY, release);
            let original = original(&program, "app", "answer");
            assert!(
                !program
                    .resolved()
                    .resolutions
                    .contains_key(&original.name.span)
            );
            let execution =
                CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
            let definition = execution
                .declaration_definition(original.name.span)
                .unwrap();
            assert!(std::ptr::eq(
                execution.retained_function(definition).unwrap(),
                original
            ));
            let info = &program.resolved().scope_table.definitions[definition.index() as usize];
            assert_eq!(info.span, original.name.span);
            assert_eq!(info.name, "app.answer");
            assert_eq!(
                execution
                    .source_function_context(original.name.span)
                    .unwrap(),
                (Some("app".to_string()), false)
            );
        }
    }

    #[test]
    fn retained_declaration_present_malformed_resolution_never_uses_ordinary_fallback() {
        for release in [false, true] {
            let program = crate::resource_execution::tests::program(ORDINARY, release);
            let declaration = original(&program, "app", "answer").name.span;
            let foreign = original(&program, "other", "answer").name.span;
            let execution =
                CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
            let foreign_definition = execution.declaration_definition(foreign).unwrap();
            for wrong in [foreign_definition, DefId::new(u32::MAX)] {
                let mut resolutions = program.resolved().resolutions.clone();
                resolutions.insert(declaration, wrong);
                assert_eq!(
                    retained_declaration_definition(
                        program.module(),
                        &program.resolved().scope_table.definitions,
                        &resolutions,
                        declaration
                    ),
                    Err(ResourceExecutionError::InvalidInvocation)
                );
            }
            let mut definitions = program.resolved().scope_table.definitions.clone();
            let definition = execution.declaration_definition(declaration).unwrap();
            let duplicate = definitions[definition.index() as usize].clone();
            definitions.push(duplicate);
            assert_eq!(
                retained_declaration_definition(
                    program.module(),
                    &definitions,
                    &program.resolved().resolutions,
                    declaration
                ),
                Err(ResourceExecutionError::InvalidInvocation)
            );
            let mut module = program.module().clone();
            module
                .items
                .push(Item::Function(original(&program, "app", "answer").clone()));
            assert_eq!(
                retained_declaration_definition(
                    &module,
                    &program.resolved().scope_table.definitions,
                    &program.resolved().resolutions,
                    declaration
                ),
                Err(ResourceExecutionError::InvalidInvocation)
            );
        }
    }

    #[test]
    fn retained_declaration_mutual_rejoins_exact_forward_and_actual_nodes() {
        for release in [false, true] {
            let program = crate::resource_execution::tests::program(MUTUAL, release);
            let actual = program
                .module()
                .items
                .iter()
                .find_map(|item| match item {
                    Item::Function(function) if function.name.name == "first" => Some(function),
                    _ => None,
                })
                .unwrap();
            let execution =
                CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
            let definition = execution.declaration_definition(actual.name.span).unwrap();
            assert_eq!(
                program.resolved().resolutions.get(&actual.name.span),
                Some(&definition)
            );
            assert_ne!(
                program.resolved().scope_table.definitions[definition.index() as usize].span,
                actual.name.span
            );
            assert!(std::ptr::eq(
                execution.retained_function(definition).unwrap(),
                actual
            ));
            let mut resolutions = program.resolved().resolutions.clone();
            resolutions.remove(&actual.name.span);
            assert_eq!(
                retained_declaration_definition(
                    program.module(),
                    &program.resolved().scope_table.definitions,
                    &resolutions,
                    actual.name.span
                ),
                Err(ResourceExecutionError::InvalidInvocation)
            );
            let mut definitions = program.resolved().scope_table.definitions.clone();
            definitions[definition.index() as usize].span = actual.body.span;
            assert_eq!(
                retained_declaration_definition(
                    program.module(),
                    &definitions,
                    &program.resolved().resolutions,
                    actual.name.span
                ),
                Err(ResourceExecutionError::InvalidInvocation)
            );
        }
    }
}
