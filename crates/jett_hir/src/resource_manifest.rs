//! Original checked Resource declarations and hooks, including unused entries.
//! Public references cannot be constructed from a type, name or raw lowering input.
use std::sync::Arc;

use jett_common::{SourceOrigin, Span};
use jett_parser::ast::Item;
use jett_resolve::{DefId, DefKind};
use jett_typecheck::{CheckedResourceHook, CheckedResourceProgram, validate_resource_hooks};
use jett_types::{ResourceKernelRecipe, Type, TypeId, TypeInterner};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceKindId(u32);
impl ResourceKindId {
    pub fn index(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceKind {
    definition: DefId,
    ty: TypeId,
    declaration: Span,
    name: String,
    namespace: Option<String>,
}
impl ResourceKind {
    pub fn definition(&self) -> DefId {
        self.definition
    }
    pub fn ty(&self) -> TypeId {
        self.ty
    }
    pub fn declaration(&self) -> Span {
        self.declaration
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn namespace(&self) -> Option<&str> {
        self.namespace.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Hook {
    checked: CheckedResourceHook,
    kind: ResourceKindId,
    recipe: ResourceKernelRecipe,
}

#[derive(Debug)]
struct ManifestData {
    original: Arc<CheckedResourceProgram>,
    kinds: Vec<ResourceKind>,
    hooks: Vec<Hook>,
}

#[derive(Debug, Clone, Default)]
pub struct ResourceManifest {
    data: Option<Arc<ManifestData>>,
}
impl PartialEq for ResourceManifest {
    fn eq(&self, other: &Self) -> bool {
        match (&self.data, &other.data) {
            (None, None) => true,
            (Some(left), Some(right)) => Arc::ptr_eq(left, right),
            _ => false,
        }
    }
}
impl Eq for ResourceManifest {}

/// Exact nominal provenance, separate from the dense index used for table lookup.
#[derive(Debug, Clone)]
pub struct ResourceKindRef {
    data: Arc<ManifestData>,
    index: usize,
}
impl PartialEq for ResourceKindRef {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && Arc::ptr_eq(&self.data, &other.data)
    }
}
impl Eq for ResourceKindRef {}
impl ResourceKindRef {
    pub fn id(&self) -> ResourceKindId {
        ResourceKindId(self.index as u32)
    }
    pub fn ty(&self) -> TypeId {
        self.data.kinds[self.index].ty
    }
    pub fn declaration(&self) -> Span {
        self.data.kinds[self.index].declaration
    }
    pub fn definition(&self) -> DefId {
        self.data.kinds[self.index].definition
    }
    pub fn validate(&self, types: &TypeInterner) -> Result<(), String> {
        ResourceManifest {
            data: Some(self.data.clone()),
        }
        .validate(types)?;
        self.data
            .kinds
            .get(self.index)
            .ok_or("Resource kind reference is outside its original manifest")?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ResourceHookRef {
    data: Arc<ManifestData>,
    index: usize,
}
impl PartialEq for ResourceHookRef {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && Arc::ptr_eq(&self.data, &other.data)
    }
}
impl Eq for ResourceHookRef {}
impl ResourceHookRef {
    pub fn definition(&self) -> DefId {
        self.data.hooks[self.index].checked.definition
    }
    pub fn resource_type(&self) -> TypeId {
        self.data.hooks[self.index].checked.resource_type
    }
    pub fn function_type(&self) -> TypeId {
        self.data.hooks[self.index].checked.function_type
    }
    pub fn kind(&self) -> ResourceKindId {
        self.data.hooks[self.index].kind
    }
    pub fn resource_kind(&self) -> ResourceKindRef {
        ResourceKindRef {
            data: self.data.clone(),
            index: self.kind().index() as usize,
        }
    }

    pub fn recipe(&self) -> ResourceKernelRecipe {
        self.data.hooks[self.index].recipe
    }
    pub fn validate(&self, types: &TypeInterner) -> Result<(), String> {
        ResourceManifest {
            data: Some(self.data.clone()),
        }
        .validate(types)?;
        self.data
            .hooks
            .get(self.index)
            .ok_or("Resource hook reference is outside its original manifest")?;
        Ok(())
    }
    pub fn metadata_types(&self, mut visit: impl FnMut(TypeId)) {
        visit(self.resource_type());
        visit(self.function_type());
        if let Type::Function {
            params,
            return_type,
            ..
        } = self
            .data
            .original
            .checked()
            .interner
            .resolve(self.function_type())
        {
            for &ty in params {
                visit(ty);
            }
            visit(*return_type);
        }
    }
}

impl ResourceManifest {
    pub fn empty() -> Self {
        Self::default()
    }
    pub fn kinds(&self) -> &[ResourceKind] {
        self.data.as_ref().map_or(&[], |data| data.kinds.as_slice())
    }
    pub fn hooks(&self) -> impl Iterator<Item = ResourceHookRef> + '_ {
        self.data.iter().flat_map(|data| {
            (0..data.hooks.len()).map(move |index| ResourceHookRef {
                data: data.clone(),
                index,
            })
        })
    }
    pub fn contains_type(&self, ty: TypeId) -> bool {
        self.kinds().iter().any(|kind| kind.ty == ty)
    }
    pub fn kind_for_type(&self, ty: TypeId) -> Option<ResourceKindRef> {
        let data = self.data.as_ref()?;
        let index = data.kinds.iter().position(|kind| kind.ty == ty)?;
        Some(ResourceKindRef {
            data: data.clone(),
            index,
        })
    }
    pub fn contains_kind(&self, kind: &ResourceKindRef) -> bool {
        self.data
            .as_ref()
            .is_some_and(|data| Arc::ptr_eq(data, &kind.data) && kind.index < data.kinds.len())
    }

    pub fn contains_hook(&self, hook: &ResourceHookRef) -> bool {
        self.data
            .as_ref()
            .is_some_and(|data| Arc::ptr_eq(data, &hook.data) && hook.index < data.hooks.len())
    }
    pub(crate) fn hook_for_definition(&self, definition: DefId) -> Option<ResourceHookRef> {
        self.hooks().find(|hook| hook.definition() == definition)
    }
    pub(crate) fn checked(original: &Arc<CheckedResourceProgram>) -> Result<Self, String> {
        let (kinds, hooks) = records(original)?;
        let manifest = Self {
            data: Some(Arc::new(ManifestData {
                original: original.clone(),
                kinds,
                hooks,
            })),
        };
        manifest.validate(&original.checked().interner)?;
        Ok(manifest)
    }
    /// Reject unknown Resource kinds anywhere in a checked layout or signature.
    /// This validates provenance only; it does not grant aggregate custody.
    pub fn validate_type(&self, types: &TypeInterner, ty: TypeId) -> Result<(), String> {
        let mut pending = vec![ty];
        let mut seen = std::collections::HashSet::new();
        while let Some(ty) = pending.pop() {
            if ty.index() as usize >= types.len() {
                return Err("Resource manifest type visitor is outside its interner".into());
            }
            if !seen.insert(ty) {
                continue;
            }
            pending.extend_from_slice(types.nominal_type_arguments(ty));
            match types.resolve(ty) {
                Type::Resource(_) if !self.contains_type(ty) => {
                    return Err(
                        "Resource type has no original checked nominal manifest entry".into(),
                    );
                }
                Type::List(inner)
                | Type::Set(inner)
                | Type::Optional(inner)
                | Type::Secret(inner)
                | Type::Refinement { base: inner, .. } => pending.push(*inner),
                Type::Map(key, value) | Type::Result(key, value) => {
                    pending.push(*key);
                    pending.push(*value);
                }
                Type::Function {
                    params,
                    return_type,
                    ..
                } => {
                    pending.extend_from_slice(params);
                    pending.push(*return_type);
                }
                Type::Struct(id) => {
                    let definition = types.resolve_struct(*id);
                    pending.extend(definition.fields.iter().map(|(_, ty)| *ty));
                    for method in &definition.methods {
                        pending.extend(method.params.iter().map(|(_, ty, _)| *ty));
                        pending.push(method.return_type);
                    }
                }
                Type::Enum(id) => {
                    for variant in &types.resolve_enum(*id).variants {
                        pending.extend(variant.fields.iter().map(|(_, ty)| *ty));
                    }
                }
                Type::Machine(id) | Type::MachineState { machine: id, .. } => {
                    for state in &types.resolve_machine(*id).states {
                        pending.extend(state.fields.iter().map(|(_, ty)| *ty));
                    }
                }
                Type::Interface(id) => {
                    for method in &types.resolve_interface(*id).methods {
                        pending.extend(method.params.iter().map(|(_, ty, _)| *ty));
                        pending.push(method.return_type);
                    }
                }
                Type::Actor(id) => {
                    let actor = types.resolve_actor(*id);
                    pending.extend(
                        actor
                            .capability_params
                            .iter()
                            .chain(&actor.state_fields)
                            .map(|(_, ty)| *ty),
                    );
                    for message in &actor.messages {
                        pending.extend(message.params.iter().map(|(_, ty)| *ty));
                        pending.push(message.responds);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
    pub fn validate(&self, types: &TypeInterner) -> Result<(), String> {
        let Some(data) = &self.data else {
            return Ok(());
        };
        let (kinds, hooks) = records(&data.original)?;
        if kinds != data.kinds || hooks != data.hooks {
            return Err(
                "Resource manifest differs from its original checked declarations or unused hooks"
                    .into(),
            );
        }
        for kind in &data.kinds {
            if kind.ty.index() as usize >= types.len()
                || types.resolve(kind.ty) != data.original.checked().interner.resolve(kind.ty)
                || !matches!(types.resolve(kind.ty), Type::Resource(name) if name == &kind.name)
            {
                return Err(
                    "Resource manifest nominal type differs from its original declaration".into(),
                );
            }
        }
        for hook in &data.hooks {
            if !hook.recipe.matches_signature(
                types,
                hook.checked.resource_type,
                hook.checked.function_type,
            ) || types.resolve(hook.checked.function_type)
                != data
                    .original
                    .checked()
                    .interner
                    .resolve(hook.checked.function_type)
            {
                return Err(
                    "Resource manifest hook signature differs from its exact checked recipe".into(),
                );
            }
        }
        Ok(())
    }
}

fn records(
    original: &Arc<CheckedResourceProgram>,
) -> Result<(Vec<ResourceKind>, Vec<Hook>), String> {
    validate_resource_hooks(original.module(), original.resolved(), original.checked())
        .map_err(|error| error.to_string())?;
    let mut kinds = Vec::new();
    for item in &original.module().items {
        let Item::Resource(resource) = item else {
            continue;
        };
        let declarations = original
            .resolved()
            .scope_table
            .definitions
            .iter()
            .filter(|definition| {
                definition.kind == DefKind::Resource && definition.span == resource.name.span
            })
            .collect::<Vec<_>>();
        let [declaration] = declarations.as_slice() else {
            return Err(
                "Resource manifest requires one exact original Resource declaration".into(),
            );
        };
        if original.source_origins().get(&resource.name.span.file) != Some(&SourceOrigin::Stdlib) {
            return Err(
                "Resource manifest declaration has no compiler-shipped source origin".into(),
            );
        }
        let ty = *original
            .checked()
            .definition_types
            .get(&declaration.id)
            .ok_or("Resource manifest declaration has no checked nominal type")?;
        if ty.index() as usize >= original.checked().interner.len()
            || !matches!(original.checked().interner.resolve(ty), Type::Resource(name) if name == &declaration.name)
        {
            return Err("Resource manifest declaration has another nominal type".into());
        }
        if kinds
            .iter()
            .any(|kind: &ResourceKind| kind.definition == declaration.id || kind.ty == ty)
        {
            return Err("Resource manifest duplicates a nominal declaration".into());
        }
        kinds.push(ResourceKind {
            definition: declaration.id,
            ty,
            declaration: resource.name.span,
            name: declaration.name.clone(),
            namespace: declaration.namespace.clone(),
        });
    }
    let mut hooks = Vec::new();
    for kernel in original.resolved().resource_kernels.iter() {
        let checked = original
            .checked()
            .resource_hooks
            .get(&kernel.definition)
            .ok_or("Resource manifest kernel has no checked hook")?
            .clone();
        let kind = kinds
            .iter()
            .position(|kind| {
                kind.definition == kernel.resource_definition && kind.ty == checked.resource_type
            })
            .ok_or("Resource manifest hook has no original nominal declaration")?;
        hooks.push(Hook {
            checked,
            kind: ResourceKindId(kind as u32),
            recipe: kernel.recipe,
        });
    }
    hooks.sort_by_key(|hook| hook.checked.definition.index());
    Ok((kinds, hooks))
}

#[cfg(test)]
#[path = "resource_manifest_tests.rs"]
mod tests;
