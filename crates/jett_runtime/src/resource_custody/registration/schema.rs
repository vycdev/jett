//! Plain immutable wire records. None is a Source or custody authority.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum NativeRecipe {
    NetworkFactory,
    NetworkBorrow,
    Finalize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum NativeAccess {
    Owned,
    View,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum NativeFrameRole {
    Scope,
    Operation,
    Return,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum NativePayloadStep {
    Some,
    Ok,
    Fail,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum NativePosition {
    Statement(u32),
    Terminator,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct NativeSite {
    pub(super) function: u32,
    pub(super) block: u32,
    pub(super) position: NativePosition,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum NativeParent {
    Root,
    Frame(u32),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum NativeShape {
    Integer { bits: u32, signed: bool },
    Float { bits: u32 },
    Bool,
    String,
    Nothing,
    Network,
    Resource { kind: u32 },
    Optional { child: u32 },
    Result { ok: u32, fail: u32 },
    HookDescriptor { hook: u32 },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct NativeFormal {
    pub(super) shape: u32,
    pub(super) access: NativeAccess,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeSignature {
    pub(super) ordinal: u32,
    pub(super) parameters: Vec<NativeFormal>,
    pub(super) result: u32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeHook {
    pub(super) ordinal: u32,
    pub(super) kind: u32,
    pub(super) recipe: NativeRecipe,
    pub(super) signature: u32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeFrame {
    pub(super) ordinal: u32,
    pub(super) role: NativeFrameRole,
    pub(super) site: NativeSite,
    pub(super) signature: u32,
    pub(super) parents: Vec<NativeParent>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeSlot {
    pub(super) ordinal: u32,
    pub(super) frame: u32,
    pub(super) shape: u32,
    pub(super) path: Vec<NativePayloadStep>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeLoanSource {
    ExistingBorrow { operation: u32 },
    IncomingViewFormal { scope: u32, parameter: u32 },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeSourceSyntax {
    Bare,
    WrittenView,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeSourceEffect {
    Copy,
    TransferOwned,
    RelinquishOwned,
    RetainBorrow,
    ObserveData,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeSourceValue {
    Ordinary,
    Owned {
        caller_argument_slot: u32,
        callee_parameter_slot: u32,
    },
    ResidentView {
        source: NativeLoanSource,
    },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeSourceFormal {
    pub(crate) parameter: u32,
    pub(crate) source_index: u32,
    pub(crate) actual_shape: u32,
    pub(crate) callee_shape: u32,
    pub(crate) syntax: NativeSourceSyntax,
    pub(crate) effect: NativeSourceEffect,
    pub(crate) access: NativeAccess,
    pub(crate) value: NativeSourceValue,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NativeSourceResult {
    Ordinary {
        shape: u32,
    },
    Owned {
        shape: u32,
        caller_destination_frame: u32,
        caller_destination_slot: u32,
        callee_return_frame: u32,
        permitted_return_slots: Vec<u32>,
    },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeSourceInvocation {
    pub(crate) callee_return: Option<u32>,
    pub(crate) evaluation_order: Vec<u32>,
    pub(crate) formals: Vec<NativeSourceFormal>,
    pub(crate) result: NativeSourceResult,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NativeOperation {
    Acquire {
        frame: u32,
        hook: u32,
        destination: u32,
    },
    Transfer {
        frame: u32,
        source: u32,
        destination: u32,
    },
    Borrow {
        frame: u32,
        source: u32,
        lease_frame: u32,
    },
    BoundedBorrowUse {
        frame: u32,
        source: NativeLoanSource,
        callee_frame: u32,
    },
    EndBorrow {
        frame: u32,
        borrow: u32,
    },
    InvokeBorrow {
        frame: u32,
        hook: u32,
        source: NativeLoanSource,
    },
    Close {
        frame: u32,
        hook: u32,
        source: u32,
    },
    Drop {
        frame: u32,
        source: u32,
    },
    SumAdopt {
        frame: u32,
        source: u32,
        destination: u32,
    },
    SumTake {
        frame: u32,
        source: u32,
        destination: u32,
    },
    SumDrop {
        frame: u32,
        source: u32,
    },
    Replace {
        frame: u32,
        old: u32,
        replacement: u32,
    },
    Complete {
        frame: u32,
    },
    Descriptor {
        hook: u32,
    },
    InvokeDescriptor {
        hook: u32,
        signature: u32,
        target: u32,
    },
    InvokeSourceFunction {
        frame: u32,
        callee: u32,
        signature: u32,
        callee_scope: u32,
        /// None is strict historical v1 metadata; it never enables Source activation.
        source: Option<NativeSourceInvocation>,
    },
    TakeFailureCompanion {
        frame: u32,
        source_sum_slot: u32,
        failure_shape: u32,
    },
    PublishReturn {
        frame: u32,
        source_return_slot: u32,
    },
    CreateAbsentSum {
        frame: u32,
        destination_slot: u32,
    },
    CreateFailureSum {
        frame: u32,
        destination_slot: u32,
        failure_shape: u32,
    },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeOperationRecord {
    pub(super) ordinal: u32,
    pub(super) site: NativeSite,
    pub(super) operation: NativeOperation,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct WireLayout {
    pub(super) version: u32,
    pub(super) kinds: Vec<u32>,
    pub(super) hooks: Vec<NativeHook>,
    pub(super) signatures: Vec<NativeSignature>,
    pub(super) shapes: Vec<NativeShape>,
    pub(super) frames: Vec<NativeFrame>,
    pub(super) slots: Vec<NativeSlot>,
    pub(super) operations: Vec<NativeOperationRecord>,
}

impl NativeSite {
    pub(crate) fn function(&self) -> u32 {
        self.function
    }
    pub(crate) fn block(&self) -> u32 {
        self.block
    }
    pub(crate) fn position(&self) -> NativePosition {
        self.position
    }
}
impl NativeFormal {
    pub(crate) fn shape(&self) -> u32 {
        self.shape
    }
    pub(crate) fn access(&self) -> NativeAccess {
        self.access
    }
}
impl NativeSignature {
    pub(crate) fn ordinal(&self) -> u32 {
        self.ordinal
    }
    pub(crate) fn parameters(&self) -> &[NativeFormal] {
        &self.parameters
    }
    pub(crate) fn result(&self) -> u32 {
        self.result
    }
}
impl NativeHook {
    pub(crate) fn ordinal(&self) -> u32 {
        self.ordinal
    }
    pub(crate) fn kind(&self) -> u32 {
        self.kind
    }
    pub(crate) fn recipe(&self) -> NativeRecipe {
        self.recipe
    }
    pub(crate) fn signature(&self) -> u32 {
        self.signature
    }
}
impl NativeFrame {
    pub(crate) fn ordinal(&self) -> u32 {
        self.ordinal
    }
    pub(crate) fn role(&self) -> NativeFrameRole {
        self.role
    }
    pub(crate) fn site(&self) -> NativeSite {
        self.site
    }
    pub(crate) fn signature(&self) -> u32 {
        self.signature
    }
    pub(crate) fn parents(&self) -> &[NativeParent] {
        &self.parents
    }
}
impl NativeSlot {
    pub(crate) fn ordinal(&self) -> u32 {
        self.ordinal
    }
    pub(crate) fn frame(&self) -> u32 {
        self.frame
    }
    pub(crate) fn shape(&self) -> u32 {
        self.shape
    }
    pub(crate) fn path(&self) -> &[NativePayloadStep] {
        &self.path
    }
}
impl NativeOperationRecord {
    pub(crate) fn ordinal(&self) -> u32 {
        self.ordinal
    }
    pub(crate) fn site(&self) -> NativeSite {
        self.site
    }
    pub(crate) fn operation(&self) -> &NativeOperation {
        &self.operation
    }
}
