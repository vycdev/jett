use std::collections::{HashMap, HashSet};
use std::fmt;

use jett_common::{FileId, SourceOrigin, Span};
use jett_parser::ast::{Item, Module};
use jett_types::ResourceKernelRecipe;

use crate::{DefId, DefKind, DefVisibility, ResolveResult};

/// Privileged compiler input. Source/configuration cannot supply this catalog.
#[derive(Debug, Clone)]
pub struct ResourceKernelSpec {
    pub resource_declaration: Span,
    pub member: String,
    pub recipe: ResourceKernelRecipe,
}

/// The immutable exact declaration association; no runtime provider authority.
#[derive(Debug, Clone)]
pub struct ResolvedResourceKernel {
    pub definition: DefId,
    pub resource_definition: DefId,
    pub recipe: ResourceKernelRecipe,
    resource_declaration: Span,
    member: String,
    namespace: String,
}

/// Sealed resolver evidence, in source declaration order.
#[derive(Debug, Default)]
pub struct ResolvedResourceKernels {
    entries: Vec<ResolvedResourceKernel>,
    requested: Vec<ResourceKernelSpec>,
    origins: HashMap<FileId, SourceOrigin>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceKernelError {
    MissingResource(Span),
    UntrustedOrigin(Span),
    MissingNamespace(Span),
    InvalidMember(Span),
    DuplicateCatalogEntry(Span),
    UnresolvedResource(Span),
    InvalidDefinition(DefId),
    InvalidAssociation(DefId),
}

impl fmt::Display for ResourceKernelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid compiler resource kernel catalog: {self:?}"
        )
    }
}

impl std::error::Error for ResourceKernelError {}

impl ResolvedResourceKernels {
    pub fn iter(&self) -> impl Iterator<Item = &ResolvedResourceKernel> {
        self.entries.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty() && self.requested.is_empty()
    }

    pub fn contains_definition(&self, definition: DefId) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.definition == definition)
    }

    pub(crate) fn with_catalog(
        origins: &HashMap<FileId, SourceOrigin>,
        specs: &[ResourceKernelSpec],
    ) -> Self {
        Self {
            entries: Vec::new(),
            requested: specs.to_vec(),
            origins: origins.clone(),
        }
    }

    pub(crate) fn insert(
        &mut self,
        spec: &ResourceKernelSpec,
        definition: DefId,
        resource_definition: DefId,
        namespace: String,
    ) {
        self.entries.push(ResolvedResourceKernel {
            definition,
            resource_definition,
            recipe: spec.recipe,
            resource_declaration: spec.resource_declaration,
            member: spec.member.clone(),
            namespace,
        });
    }
}

fn declared_resources(module: &Module) -> HashMap<Span, (String, Option<String>)> {
    let mut resources = HashMap::new();
    let mut current_file = None;
    let mut namespace = None;
    for item in &module.items {
        let file = match item {
            Item::Namespace(decl) => decl.span.file,
            Item::Resource(decl) => decl.span.file,
            Item::Function(decl) => decl.span.file,
            Item::Mutual(decl) => decl.span.file,
            Item::Interface(decl) => decl.span.file,
            Item::Implement(decl) => decl.span.file,
            Item::Struct(decl) => decl.span.file,
            Item::Bitfield(decl) => decl.span.file,
            Item::Enum(decl) => decl.span.file,
            Item::Machine(decl) => decl.span.file,
            Item::Actor(decl) => decl.span.file,
            Item::VarDecl(decl) => decl.span.file,
            Item::Verify(decl) => decl.span.file,
            Item::Property(decl) => decl.span.file,
            Item::TypeAlias(decl) => decl.span.file,
        };
        if current_file.is_some_and(|previous| previous != file) {
            namespace = None;
        }
        current_file = Some(file);
        match item {
            Item::Namespace(decl) => namespace = Some(decl.name.name.clone()),
            Item::Resource(decl) => {
                resources.insert(decl.name.span, (decl.name.name.clone(), namespace.clone()));
            }
            _ => {}
        }
    }
    resources
}

pub(crate) fn validate_specs(
    module: &Module,
    origins: &HashMap<FileId, SourceOrigin>,
    specs: &[ResourceKernelSpec],
) -> Result<(), ResourceKernelError> {
    let resources = declared_resources(module);
    let mut selected = HashSet::new();
    for spec in specs {
        let span = spec.resource_declaration;
        let (_, namespace) = resources
            .get(&span)
            .ok_or(ResourceKernelError::MissingResource(span))?;
        if !span.file.is_stdlib() || origins.get(&span.file) != Some(&SourceOrigin::Stdlib) {
            return Err(ResourceKernelError::UntrustedOrigin(span));
        }
        if namespace.is_none() {
            return Err(ResourceKernelError::MissingNamespace(span));
        }
        let mut characters = spec.member.chars();
        if !characters
            .next()
            .is_some_and(|character| character.is_ascii_alphabetic() || character == '_')
            || !characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
        {
            return Err(ResourceKernelError::InvalidMember(span));
        }
        if !selected.insert((span, spec.member.as_str())) {
            return Err(ResourceKernelError::DuplicateCatalogEntry(span));
        }
    }
    Ok(())
}

/// Validate every association before checker authority, including unused hooks.
pub fn validate_resource_kernels(
    module: &Module,
    resolved: &ResolveResult,
) -> Result<(), ResourceKernelError> {
    let resources = declared_resources(module);
    let table = &resolved.resource_kernels;
    validate_specs(module, &table.origins, &table.requested)?;
    for spec in &table.requested {
        if !table.entries.iter().any(|entry| {
            entry.resource_declaration == spec.resource_declaration
                && entry.member == spec.member
                && entry.recipe == spec.recipe
        }) {
            return Err(ResourceKernelError::UnresolvedResource(
                spec.resource_declaration,
            ));
        }
    }
    if table.entries.len() != table.requested.len() {
        return Err(ResourceKernelError::InvalidAssociation(
            table.entries[0].definition,
        ));
    }
    let mut seen = HashSet::new();
    for entry in table.iter() {
        let span = entry.resource_declaration;
        let (name, namespace) = resources
            .get(&span)
            .ok_or(ResourceKernelError::MissingResource(span))?;
        if !span.file.is_stdlib() || table.origins.get(&span.file) != Some(&SourceOrigin::Stdlib) {
            return Err(ResourceKernelError::UntrustedOrigin(span));
        }
        if namespace.as_deref() != Some(entry.namespace.as_str()) {
            return Err(ResourceKernelError::InvalidAssociation(entry.definition));
        }
        let definitions = &resolved.scope_table.definitions;
        let resource = definitions
            .get(entry.resource_definition.index() as usize)
            .filter(|definition| definition.id == entry.resource_definition)
            .ok_or(ResourceKernelError::InvalidDefinition(
                entry.resource_definition,
            ))?;
        let function = definitions
            .get(entry.definition.index() as usize)
            .filter(|definition| definition.id == entry.definition)
            .ok_or(ResourceKernelError::InvalidDefinition(entry.definition))?;
        let resource_name = format!("{}.{name}", entry.namespace);
        let function_name = format!("{}.{}", entry.namespace, entry.member);
        let bindings = resolved
            .scope_table
            .scopes
            .first()
            .map(|scope| &scope.bindings);
        if resource.kind != DefKind::Resource
            || resource.name != resource_name
            || resource.span != span
            || resource.namespace.as_deref() != Some(entry.namespace.as_str())
            || resolved.resolutions.get(&span) != Some(&entry.resource_definition)
            || function.kind != DefKind::Function
            || function.name != function_name
            || function.span != span
            || function.namespace.as_deref() != Some(entry.namespace.as_str())
            || function.visibility != DefVisibility::Private
            || !bindings.is_some_and(|bindings| {
                bindings.get(&function_name) == Some(&entry.definition)
                    && bindings.get(&resource_name) == Some(&entry.resource_definition)
            })
            || !seen.insert(entry.definition)
        {
            return Err(ResourceKernelError::InvalidAssociation(entry.definition));
        }
    }
    Ok(())
}
