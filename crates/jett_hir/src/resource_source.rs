//! Original checked HIR and type meanings; raw lowering cannot mint this archive.
use crate::{
    Expression, Function, FunctionId, FunctionIdentity, Program, ResourceHookRef, ResourceManifest,
};
use jett_comptime::CheckedRequiredValue;
use jett_typecheck::CheckedResourceProgram;
use jett_types::{Type, TypeId, TypeInterner};
use std::{collections::HashMap, sync::Arc};

#[derive(Debug)]
enum Nominal {
    Struct(jett_types::StructId, jett_types::StructDef),
    Bitfield(jett_types::BitfieldId, jett_types::BitfieldDef),
    Enum(jett_types::EnumId, jett_types::EnumDef),
    Interface(jett_types::InterfaceId, jett_types::InterfaceDef),
    Actor(jett_types::ActorId, jett_types::ActorDef),
    Machine(jett_types::MachineId, jett_types::MachineDef),
}
impl Nominal {
    fn capture(types: &TypeInterner, ty: &Type) -> Option<Self> {
        Some(match ty {
            Type::Struct(id) => Self::Struct(*id, types.resolve_struct(*id).clone()),
            Type::Bitfield(id) => Self::Bitfield(*id, types.resolve_bitfield(*id).clone()),
            Type::Enum(id) => Self::Enum(*id, types.resolve_enum(*id).clone()),
            Type::Interface(id) => Self::Interface(*id, types.resolve_interface(*id).clone()),
            Type::Actor(id) => Self::Actor(*id, types.resolve_actor(*id).clone()),
            Type::Machine(id) | Type::MachineState { machine: id, .. } => {
                Self::Machine(*id, types.resolve_machine(*id).clone())
            }
            _ => return None,
        })
    }
    fn unchanged(&self, types: &TypeInterner) -> bool {
        match self {
            Self::Struct(id, original) => types.resolve_struct(*id) == original,
            Self::Bitfield(id, original) => types.resolve_bitfield(*id) == original,
            Self::Enum(id, original) => types.resolve_enum(*id) == original,
            Self::Interface(id, original) => types.resolve_interface(*id) == original,
            Self::Actor(id, original) => types.resolve_actor(*id) == original,
            Self::Machine(id, original) => types.resolve_machine(*id) == original,
        }
    }
}
#[derive(Debug)]
struct TypeRow {
    ty: TypeId,
    original: Type,
    arguments: Vec<TypeId>,
    nominal: Option<Nominal>,
}
#[derive(Debug)]
struct Original {
    checked: Arc<CheckedResourceProgram>,
    manifest: ResourceManifest,
    functions: Vec<Function>,
    equality_methods: HashMap<TypeId, FunctionId>,
    types: Vec<TypeRow>,
    exported: Vec<FunctionId>,
}

/// A checked required occurrence and the expression derived from its opaque value.
#[derive(Clone)]
pub struct RequiredMaterialization {
    pub(crate) function: FunctionId,
    pub(crate) identity: FunctionIdentity,
    pub(crate) original: Expression,
    pub(crate) current: Expression,
    pub(crate) hook: Option<ResourceHookRef>,
    pub(crate) proof: CheckedRequiredValue,
}
impl std::fmt::Debug for RequiredMaterialization {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RequiredMaterialization")
            .field("function", &self.function)
            .field("original", &self.original)
            .field("current", &self.current)
            .finish()
    }
}
impl RequiredMaterialization {
    pub fn function(&self) -> FunctionId {
        self.function
    }
    pub fn identity(&self) -> &FunctionIdentity {
        &self.identity
    }
    pub fn original(&self) -> &Expression {
        &self.original
    }
    pub fn current(&self) -> &Expression {
        &self.current
    }
    pub fn hook(&self) -> Option<&ResourceHookRef> {
        self.hook.as_ref()
    }
}
#[derive(Debug)]
pub(crate) struct Materialized {
    pub(crate) functions: Vec<Function>,
    pub(crate) values: Vec<RequiredMaterialization>,
    pub(crate) required_only: Vec<FunctionId>,
}
/// Clone shares immutable records; empty/default supplies no Source authority.
#[derive(Clone, Default)]
pub struct ResourceSourceArchive {
    data: Option<Arc<Original>>,
    execution: Option<Arc<Materialized>>,
}
impl std::fmt::Debug for ResourceSourceArchive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResourceSourceArchive")
            .field("authenticated", &self.data.is_some())
            .finish()
    }
}
impl PartialEq for ResourceSourceArchive {
    fn eq(&self, other: &Self) -> bool {
        match (&self.data, &other.data) {
            (None, None) => true,
            (Some(a), Some(b)) => {
                Arc::ptr_eq(a, b)
                    && match (&self.execution, &other.execution) {
                        (None, None) => true,
                        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                        _ => false,
                    }
            }
            _ => false,
        }
    }
}
impl Eq for ResourceSourceArchive {}
impl ResourceSourceArchive {
    pub fn empty() -> Self {
        Self::default()
    }
    // Called only at successful completion of the original checked lowering.
    pub(crate) fn checked(
        program: &Program,
        checked: &Arc<CheckedResourceProgram>,
        types: &TypeInterner,
    ) -> Self {
        let rows = types
            .type_ids()
            .map(|ty| TypeRow {
                ty,
                original: types.resolve(ty).clone(),
                arguments: types.nominal_type_arguments(ty).to_vec(),
                nominal: Nominal::capture(types, types.resolve(ty)),
            })
            .collect();
        Self {
            data: Some(Arc::new(Original {
                checked: checked.clone(),
                manifest: program.resource_manifest.clone(),
                functions: program.functions.clone(),
                equality_methods: program.equality_methods.clone(),
                types: rows,
                exported: super::resource_materialization::exported_functions(program, checked),
            })),
            execution: None,
        }
    }
    /// Original checked functions stay immutable; this view contains only checked replacements.
    pub fn execution_functions(&self) -> &[Function] {
        self.execution
            .as_ref()
            .map_or_else(|| self.functions(), |view| view.functions.as_slice())
    }
    /// Exact checked session identity; equal source text cannot replace it.
    pub fn belongs_to(&self, checked: &Arc<CheckedResourceProgram>) -> bool {
        self.data
            .as_ref()
            .is_some_and(|data| Arc::ptr_eq(&data.checked, checked))
    }
    pub fn required_materializations(&self) -> &[RequiredMaterialization] {
        self.execution
            .as_ref()
            .map_or(&[], |view| view.values.as_slice())
    }
    pub fn required_only_function_ids(&self) -> &[FunctionId] {
        self.execution
            .as_ref()
            .map_or(&[], |view| view.required_only.as_slice())
    }
    pub fn exported_function_ids(&self) -> &[FunctionId] {
        self.data
            .as_ref()
            .map_or(&[], |data| data.exported.as_slice())
    }
    pub fn materialized_hook(
        &self,
        function: FunctionId,
        original: &Expression,
        current: &Expression,
    ) -> Option<&ResourceHookRef> {
        self.required_materializations()
            .iter()
            .find(|row| {
                row.function == function
                    && super::resource_materialization::expression_equal(&row.original, original)
                    && super::resource_materialization::expression_equal(&row.current, current)
            })?
            .hook()
    }
    pub(crate) fn checked_program(&self) -> Option<&Arc<CheckedResourceProgram>> {
        self.data.as_ref().map(|data| &data.checked)
    }
    pub(crate) fn install_materialized(&mut self, view: Materialized) {
        self.execution = Some(Arc::new(view));
    }
    pub fn functions(&self) -> &[Function] {
        self.data
            .as_ref()
            .map_or(&[], |data| data.functions.as_slice())
    }
    pub fn manifest(&self) -> Option<&ResourceManifest> {
        self.data.as_ref().map(|data| &data.manifest)
    }
    pub fn equality_methods(&self) -> Option<&HashMap<TypeId, FunctionId>> {
        self.data.as_ref().map(|data| &data.equality_methods)
    }
    /// Additional canonical IDs cannot alter any original type's meaning.
    /// Structural/backend type validation remains mandatory before this check.
    pub fn validate_types(&self, types: &TypeInterner) -> Result<(), String> {
        let data = self
            .data
            .as_ref()
            .ok_or("Resource ownership has no original checked HIR archive")?;
        if types.len() < data.types.len() {
            return Err("Resource original checked type archive lost an original TypeId".into());
        }
        for row in &data.types {
            if types.resolve(row.ty) != &row.original
                || types.nominal_type_arguments(row.ty) != row.arguments.as_slice()
            {
                return Err(
                    "Resource original checked type meaning or nominal metadata changed".into(),
                );
            }
            if !types.contains_nominal_definition(types.resolve(row.ty)) {
                return Err(
                    "Resource original checked type archive has a missing nominal definition"
                        .into(),
                );
            }
            if row
                .nominal
                .as_ref()
                .is_some_and(|nominal| !nominal.unchanged(types))
            {
                return Err(
                    "Resource original checked type meaning or nominal metadata changed".into(),
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jett_common::{FileId, SourceOrigin};
    #[test]
    fn original_unused_nominal_row_with_missing_table_returns_error() {
        for release in [false, true] {
            let file = FileId::new(0);
            let original = Arc::new(
                jett_typecheck::CheckedResourceProgram::prepare(
                    jett_parser::parse(
                        "function main() returns nothing:\n    return nothing\n",
                        file,
                    ),
                    HashMap::from([(file, SourceOrigin::Project)]),
                    &[],
                    jett_typecheck::CheckOptions { release },
                )
                .unwrap(),
            );
            let program = crate::lower_checked_resource_program(&original).unwrap();
            let types = &original.checked().interner;
            assert!(
                types
                    .type_ids()
                    .any(|ty| matches!(types.resolve(ty), Type::Enum(_))),
                "original unused reflection enum row"
            );
            let mut missing = TypeInterner::new();
            for ty in types.type_ids() {
                assert_eq!(missing.intern(types.resolve(ty).clone()), ty);
            }
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                program.resource_source.validate_types(&missing)
            }));
            assert_eq!(
                result.unwrap().unwrap_err(),
                "Resource original checked type archive has a missing nominal definition"
            );
        }
    }
}
