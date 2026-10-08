//! Readonly proof of one successful required value in the private checked cache.
//! Source names, spans and mirrored values cannot construct this association.

use super::*;
use crate::Value;
use crate::explicit::ExplicitComptimeValues;
use jett_typecheck::CheckedGenericSpecialization;

/// The retained declaration that owns a successful required occurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CheckedRequiredOwner {
    Function {
        definition: DefId,
        declaration: Span,
    },
    Verify {
        declaration: Span,
    },
    Property {
        declaration: Span,
    },
    NamespaceConstant {
        declaration: Span,
    },
}

/// One exact selected scoped type context, in outer-to-inner source order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedRequiredScope {
    name: String,
    owner: Span,
    index: usize,
    bound_type: TypeId,
    selection: CheckedComptimeTypeSelection,
    reflection: ReflectionTypeInfo,
}

impl CheckedRequiredScope {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn owner(&self) -> Span {
        self.owner
    }
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn bound_type(&self) -> TypeId {
        self.bound_type
    }
    pub fn selection(&self) -> CheckedComptimeTypeSelection {
        self.selection
    }
    pub fn reflection(&self) -> &ReflectionTypeInfo {
        &self.reflection
    }
}

#[derive(Debug, Clone)]
struct RequiredValueSite {
    source_span: Span,
    owner: CheckedRequiredOwner,
    type_arguments: Vec<TypeId>,
    specialization: CheckedGenericSpecialization,
    scopes: Vec<CheckedRequiredScope>,
    explicit: bool,
}

/// Opaque association between a retained required occurrence, complete checked
/// context, and its successful provider-free value. There is no public mint.
#[derive(Debug, Clone)]
pub struct CheckedRequiredValue {
    key: CheckedAttemptKey,
    value: Value,
    site: RequiredValueSite,
}

impl CheckedRequiredValue {
    pub(crate) fn from_cache(
        cache: &ExplicitComptimeValues,
        key: &CheckedAttemptKey,
        program: &Arc<CheckedResourceProgram>,
    ) -> Result<Self, String> {
        if !Arc::ptr_eq(&key.program, program) {
            return Err(ResourceExecutionError::ForeignProgram.to_string());
        }
        let value = cache
            .checked_get(key)
            .ok_or_else(|| "required value has no successful checked cache entry".to_string())?;
        if value.contains_live_resource_or_grant() {
            return Err(
                "required value contains runtime authority or Resource custody".to_string(),
            );
        }
        let site = key.value_site().map_err(|error| error.to_string())?;
        let proof = Self {
            key: key.clone(),
            value: value.clone(),
            site,
        };
        // Hook metadata must belong to the same retained program as the worker
        // occurrence. Its nominal identity and exact signature stay checked.
        proof.checked_hook(program)?;
        Ok(proof)
    }

    pub fn value(&self) -> &Value {
        &self.value
    }
    pub fn belongs_to(&self, program: &Arc<CheckedResourceProgram>) -> bool {
        Arc::ptr_eq(&self.key.program, program)
    }
    pub fn source_span(&self) -> Span {
        self.site.source_span
    }
    pub fn owner(&self) -> CheckedRequiredOwner {
        self.site.owner
    }
    pub fn generic_index(&self) -> Option<usize> {
        self.key.node.generic
    }
    pub fn type_arguments(&self) -> &[TypeId] {
        &self.site.type_arguments
    }
    pub fn specialization(&self) -> &CheckedGenericSpecialization {
        &self.site.specialization
    }
    pub fn scoped_bindings(&self) -> &[CheckedRequiredScope] {
        &self.site.scopes
    }
    pub fn is_explicit_comptime(&self) -> bool {
        self.site.explicit
    }

    /// Return the original outer `comptime` node retained by this proof. A raw
    /// namespace initializer has a required value but no such outer node.
    pub fn original_comptime(&self) -> Result<&Expr, String> {
        self.key
            .original_comptime()
            .map_err(|error| error.to_string())?
            .ok_or_else(|| {
                "required namespace initializer has no explicit comptime node".to_string()
            })
    }

    pub fn matches_original_comptime(&self, expression: &Expr) -> bool {
        self.original_comptime()
            .is_ok_and(|original| std::ptr::eq(original, expression))
    }

    /// Project only an opaque hook descriptor bound to this exact checked
    /// program. Reading the metadata does not invoke a runtime hook.
    pub fn checked_hook<'a>(
        &'a self,
        program: &Arc<CheckedResourceProgram>,
    ) -> Result<Option<&'a CheckedResourceHook>, String> {
        if !self.belongs_to(program) {
            return Err(ResourceExecutionError::ForeignProgram.to_string());
        }
        let Value::ResourceHook(descriptor) = self.value.payload() else {
            return Ok(None);
        };
        if !Arc::ptr_eq(&descriptor.program, &self.key.program) {
            return Err(ResourceExecutionError::ForeignProgram.to_string());
        }
        descriptor
            .program
            .checked()
            .resource_hooks
            .get(&descriptor.definition)
            .map(Some)
            .ok_or_else(|| ResourceExecutionError::InvalidInvocation.to_string())
    }
}

/// Hook projection of a successful checked required value. It remains metadata,
/// separate from a physical Resource owner or a runtime capability.
#[derive(Debug, Clone)]
pub struct CheckedRequiredResourceHook {
    required: CheckedRequiredValue,
}

impl CheckedRequiredResourceHook {
    pub(crate) fn from_value(required: CheckedRequiredValue) -> Option<Self> {
        matches!(required.value.payload(), Value::ResourceHook(_)).then_some(Self { required })
    }
    pub fn value(&self) -> &Value {
        self.required.value()
    }
    pub fn belongs_to(&self, program: &Arc<CheckedResourceProgram>) -> bool {
        self.required.belongs_to(program)
    }
    pub fn source_span(&self) -> Span {
        self.required.source_span()
    }
    pub fn owner(&self) -> CheckedRequiredOwner {
        self.required.owner()
    }
    pub fn generic_index(&self) -> Option<usize> {
        self.required.generic_index()
    }
    pub fn type_arguments(&self) -> &[TypeId] {
        self.required.type_arguments()
    }
    pub fn specialization(&self) -> &CheckedGenericSpecialization {
        self.required.specialization()
    }
    pub fn scoped_bindings(&self) -> &[CheckedRequiredScope] {
        self.required.scoped_bindings()
    }
    pub fn is_explicit_comptime(&self) -> bool {
        self.required.is_explicit_comptime()
    }
    pub fn original_comptime(&self) -> Result<&Expr, String> {
        self.required.original_comptime()
    }
    pub fn matches_original_comptime(&self, expression: &Expr) -> bool {
        self.required.matches_original_comptime(expression)
    }
    pub fn checked_hook<'a>(
        &'a self,
        program: &Arc<CheckedResourceProgram>,
    ) -> Result<&'a CheckedResourceHook, String> {
        self.required
            .checked_hook(program)?
            .ok_or_else(|| "required value is not a checked Resource hook descriptor".to_string())
    }
}

impl CheckedAttemptKey {
    fn cursor(&self) -> CheckedBodyCursor {
        CheckedBodyCursor {
            root: match self.node.generic {
                Some(index) => BodyRoot::Generic(index),
                None => BodyRoot::Ordinary,
            },
            scopes: self
                .node
                .scopes
                .iter()
                .map(
                    |(owner, index, bound_type, selection, reflection)| ScopedSelection {
                        owner: *owner,
                        index: *index,
                        bound_type: *bound_type,
                        selection: match selection {
                            Some(index) => CheckedComptimeTypeSelection::ReflectedIteration(*index),
                            None => CheckedComptimeTypeSelection::Unconditional,
                        },
                        reflection: reflection.clone(),
                    },
                )
                .collect(),
            executable: Some(ExecutableIdentity {
                program: self.program.clone(),
                origin: self.node.origin,
                path: self.node.path.clone(),
            }),
        }
    }

    fn region(&self, path: &[RegionStep]) -> Result<OriginalRegion<'_>, ResourceExecutionError> {
        let mut region = match self.node.origin {
            ExecutableOrigin::Function { item, .. } => {
                match self.program.module().items.get(item) {
                    Some(Item::Function(value)) => OriginalRegion::Block(&value.body),
                    _ => return Err(ResourceExecutionError::MissingCheckedBody),
                }
            }
            ExecutableOrigin::NamespaceInitializer { item } => {
                match self.program.module().items.get(item) {
                    Some(Item::VarDecl(value)) => OriginalRegion::Expression(&value.value),
                    _ => return Err(ResourceExecutionError::MissingCheckedBody),
                }
            }
            ExecutableOrigin::Verify { item } => match self.program.module().items.get(item) {
                Some(Item::Verify(value)) => OriginalRegion::Block(&value.body),
                _ => return Err(ResourceExecutionError::MissingCheckedBody),
            },
            ExecutableOrigin::Property { item } => match self.program.module().items.get(item) {
                Some(Item::Property(value)) => OriginalRegion::Block(&value.body),
                _ => return Err(ResourceExecutionError::MissingCheckedBody),
            },
        };
        for step in path {
            region = match step {
                RegionStep::Scope(owner) => {
                    OriginalRegion::Block(&region_binding(region, *owner)?.body)
                }
                RegionStep::Expression(index) => {
                    OriginalRegion::Expression(region_expression(region, *index)?)
                }
            };
        }
        Ok(region)
    }

    fn original_comptime(&self) -> Result<Option<&Expr>, ResourceExecutionError> {
        let Some((RegionStep::Expression(index), parent)) = self.node.path.split_last() else {
            return Err(ResourceExecutionError::MissingCheckedBody);
        };
        let region = self.region(parent)?;
        let selected = region_expression(region, *index)?;
        let mut original = Vec::new();
        walk_region(
            region,
            &mut |_| {},
            &mut |expression| {
                if let Expr::Comptime(inner, _) = expression {
                    if std::ptr::eq(inner.as_ref(), selected) {
                        original.push(expression);
                    }
                }
            },
            &mut |_| {},
        );
        match original.as_slice() {
            [original] => Ok(Some(*original)),
            [] if matches!(
                self.node.origin,
                ExecutableOrigin::NamespaceInitializer { .. }
            ) =>
            {
                Ok(None)
            }
            _ => Err(ResourceExecutionError::MissingCheckedBody),
        }
    }

    fn value_site(&self) -> Result<RequiredValueSite, ResourceExecutionError> {
        let checked =
            CheckedExecution::new(self.program.clone(), ExecutionPurpose::ExplicitComptime)?;
        checked.validate_executable_cursor(&self.cursor())?;
        let owner = match self.node.origin {
            ExecutableOrigin::Function { definition, item } => {
                match self.program.module().items.get(item) {
                    Some(Item::Function(value)) => CheckedRequiredOwner::Function {
                        definition,
                        declaration: value.name.span,
                    },
                    _ => return Err(ResourceExecutionError::MissingCheckedBody),
                }
            }
            ExecutableOrigin::NamespaceInitializer { item } => {
                match self.program.module().items.get(item) {
                    Some(Item::VarDecl(value)) => CheckedRequiredOwner::NamespaceConstant {
                        declaration: value.name.span,
                    },
                    _ => return Err(ResourceExecutionError::MissingCheckedBody),
                }
            }
            ExecutableOrigin::Verify { item } => match self.program.module().items.get(item) {
                Some(Item::Verify(value)) => CheckedRequiredOwner::Verify {
                    declaration: value.name.span,
                },
                _ => return Err(ResourceExecutionError::MissingCheckedBody),
            },
            ExecutableOrigin::Property { item } => match self.program.module().items.get(item) {
                Some(Item::Property(value)) => CheckedRequiredOwner::Property {
                    declaration: value.name.span,
                },
                _ => return Err(ResourceExecutionError::MissingCheckedBody),
            },
        };
        let (type_arguments, specialization) = match self.node.generic {
            Some(index) => {
                let instance = self
                    .program
                    .checked()
                    .generic_function_instantiations
                    .get(index)
                    .ok_or(ResourceExecutionError::MissingCheckedBody)?;
                (
                    instance.concrete_args.clone(),
                    instance.specialization.clone(),
                )
            }
            None => (Vec::new(), CheckedGenericSpecialization::default()),
        };
        let mut region = self.region(&[])?;
        let mut selections = self.node.scopes.iter();
        let mut scopes = Vec::new();
        for step in &self.node.path {
            region = match step {
                RegionStep::Scope(owner) => {
                    let binding = region_binding(region, *owner)?;
                    let Some((selected_owner, index, bound_type, selection, reflection)) =
                        selections.next()
                    else {
                        return Err(ResourceExecutionError::MissingCheckedBody);
                    };
                    if owner != selected_owner {
                        return Err(ResourceExecutionError::MissingCheckedBody);
                    }
                    scopes.push(CheckedRequiredScope {
                        name: binding.name.name.clone(),
                        owner: *owner,
                        index: *index,
                        bound_type: *bound_type,
                        selection: match selection {
                            Some(index) => CheckedComptimeTypeSelection::ReflectedIteration(*index),
                            None => CheckedComptimeTypeSelection::Unconditional,
                        },
                        reflection: reflection.clone(),
                    });
                    OriginalRegion::Block(&binding.body)
                }
                RegionStep::Expression(index) => {
                    OriginalRegion::Expression(region_expression(region, *index)?)
                }
            };
        }
        if selections.next().is_some() {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        let OriginalRegion::Expression(expression) = region else {
            return Err(ResourceExecutionError::MissingCheckedBody);
        };
        let original = self.original_comptime()?;
        Ok(RequiredValueSite {
            source_span: original.map_or_else(|| expression.span(), Expr::span),
            owner,
            type_arguments,
            specialization,
            scopes,
            explicit: original.is_some(),
        })
    }
}

#[cfg(test)]
#[path = "value_proof/tests.rs"]
mod tests;
