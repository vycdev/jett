//! Checked field paths, not runtime ownership or mutation authority.
//!
//! Facts belong to one exact checked body, including its concrete generic and
//! lexical comptime selections. A consumer must authenticate original/current
//! Source and independently establish live storage, generation and absence of
//! dependent loans before preparing or committing any replacement.

use std::collections::{HashMap, HashSet};

use jett_common::Span;
use jett_types::{MachineId, MachineStateId, StructId, Type, TypeId, TypeInterner};

use crate::{CheckedBindingFact, CheckedBindingMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedFieldOwner {
    Struct(StructId),
    MachineState {
        machine: MachineId,
        state: MachineStateId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedFieldUpdateResourceRequirement {
    Ordinary,
    RequiresCustody,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedFieldProjection {
    /// Original projection use; no field declaration span is fabricated.
    pub source_span: Span,
    /// Exact checked parent type, including its Secret qualification.
    pub owner_type: TypeId,
    pub owner: CheckedFieldOwner,
    pub field_index: usize,
    /// Complete stored type, retaining nominal refinements.
    pub declared_field_type: TypeId,
    /// Checked access type after qualification propagation.
    pub field_type: TypeId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedAssignmentPlace {
    pub assignment_span: Span,
    /// Declaration identity and declared storage type, not a liveness proof.
    pub root: CheckedBindingFact,
    pub root_use_span: Span,
    /// Current checked type; may narrow declared Machine storage to one state.
    pub root_type: TypeId,
    pub projections: Vec<CheckedFieldProjection>,
    /// Entire root classification, including unchanged Resource siblings.
    pub resource: CheckedFieldUpdateResourceRequirement,
    /// Set only after successful independent ownership traversal of this exact
    /// concrete/scoped context and successful checking of the whole program.
    pub(crate) ownership_seal: Option<AssignmentOwnershipSeal>,
}

impl CheckedAssignmentPlace {
    pub fn ownership_validated(&self) -> bool {
        self.ownership_seal
            .as_ref()
            .is_some_and(|seal| seal.matches(self))
    }
}

/// This snapshots checked metadata only, never a Source runtime value/owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AssignmentOwnershipSeal {
    assignment_span: Span,
    root: CheckedBindingFact,
    root_use_span: Span,
    root_type: TypeId,
    projections: Vec<CheckedFieldProjection>,
    resource: CheckedFieldUpdateResourceRequirement,
}

impl AssignmentOwnershipSeal {
    fn matches(&self, place: &CheckedAssignmentPlace) -> bool {
        self.assignment_span == place.assignment_span
            && self.root == place.root
            && self.root_use_span == place.root_use_span
            && self.root_type == place.root_type
            && self.projections == place.projections
            && self.resource == place.resource
    }

    fn capture(place: &CheckedAssignmentPlace) -> Self {
        Self {
            assignment_span: place.assignment_span,
            root: place.root,
            root_use_span: place.root_use_span,
            root_type: place.root_type,
            projections: place.projections.clone(),
            resource: place.resource,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub(crate) enum AssignmentContextRoot {
    #[default]
    Ordinary,
    Generic(usize),
    ActorHandler(crate::CheckedActorHandlerTarget),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub(crate) struct AssignmentContext {
    pub root: AssignmentContextRoot,
    pub scopes: Vec<(Span, usize)>,
}

impl AssignmentContext {
    pub fn scoped(&self, binding: Span, index: usize) -> Self {
        let mut context = self.clone();
        context.scopes.push((binding, index));
        context
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct AssignmentSite {
    pub context: AssignmentContext,
    pub span: Span,
}

/// Stamp only facts independently visited in their exact selected context.
/// The caller supplies an empty set whenever any Source checking error exists.
pub(crate) fn seal_assignment_ownership(
    places: &mut HashMap<Span, CheckedAssignmentPlace>,
    bindings: &mut HashMap<Span, Vec<crate::CheckedComptimeTypeBinding>>,
    context: &AssignmentContext,
    validated: &HashSet<AssignmentSite>,
) {
    for (span, place) in places {
        let admitted = validated.contains(&AssignmentSite {
            context: context.clone(),
            span: *span,
        });
        place.ownership_seal = admitted.then(|| AssignmentOwnershipSeal::capture(place));
    }
    for (span, bodies) in bindings {
        for (index, body) in bodies.iter_mut().enumerate() {
            seal_assignment_ownership(
                &mut body.body.assignment_places,
                &mut body.body.comptime_type_bindings,
                &context.scoped(*span, index),
                validated,
            );
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedAssignmentPlaceError {
    EmptyPath,
    RootBinding,
    SourceAssociation,
    StorageType,
    RefinedOwner,
    BitfieldOwner,
    CompilerMetadataOwner,
    OwnerType,
    FieldOrdinal,
    FieldType,
    Qualification,
    ResourceClassification,
}

/// Inspect physical owned data, not phantom generic arguments or actor state.
/// This classification supplies no Resource transport/replacement permission.
pub fn field_update_resource_requirement(
    types: &TypeInterner,
    root: TypeId,
) -> CheckedFieldUpdateResourceRequirement {
    fn contains(types: &TypeInterner, ty: TypeId, visited: &mut HashSet<TypeId>) -> bool {
        if !visited.insert(ty) {
            return false;
        }
        match types.resolve(ty) {
            Type::Resource(_) => true,
            Type::List(inner)
            | Type::Set(inner)
            | Type::Optional(inner)
            | Type::Secret(inner)
            | Type::Refinement { base: inner, .. } => contains(types, *inner, visited),
            Type::Map(left, right) | Type::Result(left, right) => {
                contains(types, *left, visited) || contains(types, *right, visited)
            }
            Type::Struct(id) => types
                .resolve_struct(*id)
                .fields
                .iter()
                .any(|(_, ty)| contains(types, *ty, visited)),
            Type::Bitfield(id) => types
                .resolve_bitfield(*id)
                .fields
                .iter()
                .any(|field| contains(types, field.ty, visited)),
            Type::Enum(id) => types
                .resolve_enum(*id)
                .variants
                .iter()
                .flat_map(|variant| &variant.fields)
                .any(|(_, ty)| contains(types, *ty, visited)),
            Type::Machine(id) => types
                .resolve_machine(*id)
                .states
                .iter()
                .flat_map(|state| &state.fields)
                .any(|(_, ty)| contains(types, *ty, visited)),
            Type::MachineState { machine, state } => types
                .resolve_machine(*machine)
                .state(*state)
                .is_some_and(|state| {
                    state
                        .fields
                        .iter()
                        .any(|(_, ty)| contains(types, *ty, visited))
                }),
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
            | Type::Actor(_)
            | Type::Function { .. }
            | Type::Error => false,
        }
    }
    if contains(types, root, &mut HashSet::new()) {
        CheckedFieldUpdateResourceRequirement::RequiresCustody
    } else {
        CheckedFieldUpdateResourceRequirement::Ordinary
    }
}

fn inside(child: Span, parent: Span) -> bool {
    child.file == parent.file && child.start >= parent.start && child.end <= parent.end
}

fn checked_type(types: &TypeInterner, ty: TypeId) -> Result<&Type, CheckedAssignmentPlaceError> {
    if usize::try_from(ty.index()).is_ok_and(|index| index < types.len()) {
        Ok(types.resolve(ty))
    } else {
        Err(CheckedAssignmentPlaceError::OwnerType)
    }
}

fn nominal_owner_type(
    types: &TypeInterner,
    ty: TypeId,
) -> Result<(TypeId, bool), CheckedAssignmentPlaceError> {
    match checked_type(types, ty)? {
        Type::Secret(inner) => match checked_type(types, *inner)? {
            Type::Refinement { .. } => Err(CheckedAssignmentPlaceError::RefinedOwner),
            Type::Bitfield(_) => Err(CheckedAssignmentPlaceError::BitfieldOwner),
            Type::Secret(_) => Err(CheckedAssignmentPlaceError::OwnerType),
            _ => Ok((*inner, true)),
        },
        Type::Refinement { .. } => Err(CheckedAssignmentPlaceError::RefinedOwner),
        Type::Bitfield(_) => Err(CheckedAssignmentPlaceError::BitfieldOwner),
        _ => Ok((ty, false)),
    }
}

/// Resolve only a canonical owner and ordinal. This does not inspect a Value.
pub fn checked_field_owner(
    types: &TypeInterner,
    owner_type: TypeId,
) -> Result<(CheckedFieldOwner, &[(String, TypeId)]), CheckedAssignmentPlaceError> {
    let (nominal, _) = nominal_owner_type(types, owner_type)?;
    match checked_type(types, nominal)? {
        Type::Struct(id) if types.is_compiler_metadata_struct(*id) => {
            Err(CheckedAssignmentPlaceError::CompilerMetadataOwner)
        }
        Type::Struct(id) => Ok((
            CheckedFieldOwner::Struct(*id),
            &types.resolve_struct(*id).fields,
        )),
        Type::MachineState { machine, state } => {
            let fields = &types
                .resolve_machine(*machine)
                .state(*state)
                .ok_or(CheckedAssignmentPlaceError::OwnerType)?
                .fields;
            Ok((
                CheckedFieldOwner::MachineState {
                    machine: *machine,
                    state: *state,
                },
                fields,
            ))
        }
        _ => Err(CheckedAssignmentPlaceError::OwnerType),
    }
}

fn storage_accepts_checked_root(types: &TypeInterner, storage: TypeId, current: TypeId) -> bool {
    if storage == current {
        return true;
    }
    match (checked_type(types, storage), checked_type(types, current)) {
        (Ok(Type::Machine(expected)), Ok(Type::MachineState { machine, .. })) => {
            expected == machine
        }
        (Ok(Type::Secret(expected)), Ok(Type::Secret(actual))) => {
            storage_accepts_checked_root(types, *expected, *actual)
        }
        _ => false,
    }
}

fn field_qualification_matches(
    types: &TypeInterner,
    secret_owner: bool,
    declared: TypeId,
    selected: TypeId,
) -> bool {
    if !secret_owner || matches!(types.resolve(declared), Type::Secret(_)) {
        declared == selected
    } else {
        matches!(types.resolve(selected), Type::Secret(inner) if *inner == declared)
    }
}

/// Revalidate shape against this session's canonical definitions after remapping.
/// Resource classification is checked against declared whole-root storage;
/// it never supplies custody authority. The interner must itself belong to the
/// authenticated checked program, not a caller-fabricated nominal registry.
pub fn validate_assignment_place_shape(
    place: &CheckedAssignmentPlace,
    types: &TypeInterner,
) -> Result<(), CheckedAssignmentPlaceError> {
    if place.projections.is_empty() {
        return Err(CheckedAssignmentPlaceError::EmptyPath);
    }
    if !place.root.mutable || place.root.mode != CheckedBindingMode::Owned {
        return Err(CheckedAssignmentPlaceError::RootBinding);
    }
    if !inside(place.root_use_span, place.assignment_span) {
        return Err(CheckedAssignmentPlaceError::SourceAssociation);
    }
    checked_type(types, place.root.ty)?;
    checked_type(types, place.root_type)?;
    if !storage_accepts_checked_root(types, place.root.ty, place.root_type) {
        return Err(CheckedAssignmentPlaceError::StorageType);
    }
    if field_update_resource_requirement(types, place.root.ty) != place.resource {
        return Err(CheckedAssignmentPlaceError::ResourceClassification);
    }
    let mut current = place.root_type;
    for projection in &place.projections {
        if !inside(projection.source_span, place.assignment_span) {
            return Err(CheckedAssignmentPlaceError::SourceAssociation);
        }
        if projection.owner_type != current {
            return Err(CheckedAssignmentPlaceError::OwnerType);
        }
        let (owner, fields) = checked_field_owner(types, current)?;
        if projection.owner != owner {
            return Err(CheckedAssignmentPlaceError::OwnerType);
        }
        let (_, declared) = fields
            .get(projection.field_index)
            .ok_or(CheckedAssignmentPlaceError::FieldOrdinal)?;
        checked_type(types, projection.field_type)?;
        if projection.declared_field_type != *declared {
            return Err(CheckedAssignmentPlaceError::FieldType);
        }
        let (_, secret) = nominal_owner_type(types, current)?;
        if !field_qualification_matches(types, secret, *declared, projection.field_type) {
            return Err(CheckedAssignmentPlaceError::Qualification);
        }
        current = projection.field_type;
    }
    Ok(())
}
