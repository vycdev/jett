use std::collections::HashMap;
use std::fmt;

use jett_parser::ast::Module;
use jett_resolve::{DefId, ResolveResult, ResourceKernelError, validate_resource_kernels};
use jett_types::{ResourceHookKind, Type, TypeId};

use crate::checker::CheckResult;

/// Immutable checked identity. A matching callable name never grants authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedResourceHook {
    pub definition: DefId,
    pub resource_definition: DefId,
    pub resource_type: TypeId,
    pub kind: ResourceHookKind,
    pub function_type: TypeId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceHookError {
    Resolution(ResourceKernelError),
    SourceResolutionFailed,
    SourceCheckingFailed,
    MissingResourceType(DefId),
    InvalidResourceType(DefId),
    MissingFunctionType(DefId),
    InvalidFunctionShape(DefId),
    InvalidCheckedIdentity(DefId),
    UnexpectedCheckedHooks,
}

impl From<ResourceKernelError> for ResourceHookError {
    fn from(error: ResourceKernelError) -> Self {
        Self::Resolution(error)
    }
}

impl fmt::Display for ResourceHookError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid checked resource hook metadata: {self:?}"
        )
    }
}

impl std::error::Error for ResourceHookError {}

/// Validate all facts, including unused hooks, before a subsequent phase uses them.
pub fn validate_resource_hooks(
    module: &Module,
    resolved: &ResolveResult,
    checked: &CheckResult,
) -> Result<(), ResourceHookError> {
    validate_resource_hook_identities(module, resolved, checked)?;
    if checked
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == jett_diagnostics::Severity::Error)
    {
        return Err(ResourceHookError::SourceCheckingFailed);
    }
    Ok(())
}

/// Check exact declaration/type joins while ordinary source diagnostics stay inspectable.
/// This helper is not an execution-phase acceptance gate.
pub(crate) fn validate_resource_hook_identities(
    module: &Module,
    resolved: &ResolveResult,
    checked: &CheckResult,
) -> Result<(), ResourceHookError> {
    validate_resource_kernels(module, resolved)?;
    if resolved
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == jett_diagnostics::Severity::Error)
    {
        return Err(ResourceHookError::SourceResolutionFailed);
    }
    let mut expected = HashMap::new();
    for entry in resolved.resource_kernels.iter() {
        let resource = *checked
            .definition_types
            .get(&entry.resource_definition)
            .ok_or(ResourceHookError::MissingResourceType(
                entry.resource_definition,
            ))?;
        if resource.index() as usize >= checked.interner.len() {
            return Err(ResourceHookError::InvalidResourceType(
                entry.resource_definition,
            ));
        }
        let declaration =
            &resolved.scope_table.definitions[entry.resource_definition.index() as usize];
        if !matches!(checked.interner.resolve(resource), Type::Resource(name) if name == &declaration.name)
        {
            return Err(ResourceHookError::InvalidResourceType(
                entry.resource_definition,
            ));
        }
        let function_type = *checked
            .definition_types
            .get(&entry.definition)
            .ok_or(ResourceHookError::MissingFunctionType(entry.definition))?;
        if !entry
            .recipe
            .matches_signature(&checked.interner, resource, function_type)
        {
            return Err(ResourceHookError::InvalidFunctionShape(entry.definition));
        }
        expected.insert(
            entry.definition,
            CheckedResourceHook {
                definition: entry.definition,
                resource_definition: entry.resource_definition,
                resource_type: resource,
                kind: entry.recipe.kind(),
                function_type,
            },
        );
    }
    if expected.len() != checked.resource_hooks.len() {
        return Err(ResourceHookError::UnexpectedCheckedHooks);
    }
    for (definition, hook) in expected {
        if checked.resource_hooks.get(&definition) != Some(&hook) {
            return Err(ResourceHookError::InvalidCheckedIdentity(definition));
        }
    }
    Ok(())
}
