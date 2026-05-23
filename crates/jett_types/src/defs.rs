use crate::types::TypeId;

/// Handle to a struct definition in the type registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StructId(pub(crate) u32);

impl StructId {
    pub fn index(self) -> u32 {
        self.0
    }
}

/// Handle to a bitfield definition in the type registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BitfieldId(pub(crate) u32);

impl BitfieldId {
    pub fn index(self) -> u32 {
        self.0
    }
}

/// Handle to an enum definition in the type registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EnumId(pub(crate) u32);

impl EnumId {
    pub fn index(self) -> u32 {
        self.0
    }
}

/// Handle to an interface definition in the type registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InterfaceId(pub(crate) u32);

impl InterfaceId {
    pub fn index(self) -> u32 {
        self.0
    }
}

/// Describes a user-defined struct type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructDef {
    pub name: String,
    /// (field_name, field_type)
    pub fields: Vec<(String, TypeId)>,
    pub methods: Vec<FunctionSig>,
}

/// A field in a user-defined bitfield.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitfieldFieldDef {
    pub name: String,
    pub ty: TypeId,
    pub kind: BitfieldFieldKind,
}

/// Shape metadata for a bitfield field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BitfieldFieldKind {
    Bits { width: u16 },
    Payload,
}

/// Describes a user-defined bitfield type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitfieldDef {
    pub name: String,
    pub network_order: bool,
    pub fields: Vec<BitfieldFieldDef>,
}

/// Describes a user-defined enum type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumDef {
    pub name: String,
    pub variants: Vec<VariantDef>,
}

/// Describes a user-defined interface type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceDef {
    pub name: String,
    pub methods: Vec<FunctionSig>,
}

/// A single variant of an enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariantDef {
    pub name: String,
    /// (field_name, field_type) — empty for unit variants.
    pub fields: Vec<(String, TypeId)>,
    pub discriminant: i64,
}

/// A function signature (used for methods on structs, etc.).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionSig {
    pub name: String,
    /// (param_name, param_type, is_view)
    pub params: Vec<(String, TypeId, bool)>,
    pub return_type: TypeId,
    /// True when the function takes no capability (view) parameters.
    pub is_pure: bool,
}

// ---------------------------------------------------------------------------
// Actor definitions
// ---------------------------------------------------------------------------

/// Handle to an actor definition in the type registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ActorId(pub(crate) u32);

impl ActorId {
    pub fn index(self) -> u32 {
        self.0
    }
}

/// A single message handler on an actor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorMessageDef {
    pub name: String,
    /// (param_name, param_type)
    pub params: Vec<(String, TypeId)>,
    /// Return type for `responds T`; `TypeInterner::NOTHING` if the handler has no response.
    pub responds: TypeId,
}

/// Describes a user-defined actor type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorDef {
    pub name: String,
    /// Capability parameters received at spawn time: (param_name, param_type).
    pub capability_params: Vec<(String, TypeId)>,
    /// Mutable state fields: (field_name, field_type).
    pub state_fields: Vec<(String, TypeId)>,
    pub messages: Vec<ActorMessageDef>,
}

// ---------------------------------------------------------------------------
// State machine definitions
// ---------------------------------------------------------------------------

/// Handle to a state machine definition in the type registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MachineId(pub(crate) u32);

impl MachineId {
    pub fn index(self) -> u32 {
        self.0
    }
}

/// Handle to a state within a machine definition.
///
/// State identifiers are local to their owning [`MachineId`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MachineStateId(pub(crate) u32);

impl MachineStateId {
    pub fn index(self) -> u32 {
        self.0
    }
}

/// A single state in a checked machine definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineStateDef {
    pub name: String,
    /// Payload fields available when the machine value is at this state.
    pub fields: Vec<(String, TypeId)>,
}

/// An allowed transition edge between two states in the same machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineTransitionDef {
    pub from: MachineStateId,
    pub to: MachineStateId,
}

/// Describes a user-defined state machine type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineDef {
    pub name: String,
    pub states: Vec<MachineStateDef>,
    pub transitions: Vec<MachineTransitionDef>,
}

impl MachineDef {
    pub fn state_id(&self, name: &str) -> Option<MachineStateId> {
        self.states
            .iter()
            .position(|state| state.name == name)
            .map(|index| MachineStateId(index as u32))
    }

    pub fn state(&self, id: MachineStateId) -> Option<&MachineStateDef> {
        self.states.get(id.0 as usize)
    }

    pub fn has_transition(&self, from: MachineStateId, to: MachineStateId) -> bool {
        self.transitions
            .iter()
            .any(|transition| transition.from == from && transition.to == to)
    }
}
