//! Checked caller disposition, independent of physical callee access.
//!
//! These facts belong to one resolution session and one concrete checked body.
//! An inserted physical borrow never supplies a source `view` witness.
use jett_common::Span;
use jett_intrinsics::IntrinsicId;
use jett_resolve::scope::DefId;
use jett_types::TypeId;

use crate::{
    CheckedBindingMode, CheckedGenericCall, CheckedInterfaceCall, CheckedMethodCall,
    CheckedViewSource,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedCallerSyntax {
    Bare,
    WrittenView,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedCalleeAccess {
    Owned,
    View,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedCallerEffect {
    Copy,
    RetainBorrow,
    TransferOwned,
    RelinquishOwned,
    ObserveData,
}

/// This context describes the source occurrence, never its instantiating caller.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CheckedOwnershipContext {
    #[default]
    Ordinary,
    Verify,
    Property,
    BreakpointExpression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedBindingFact {
    pub definition: DefId,
    pub declaration_span: Span,
    pub ty: TypeId,
    pub mode: CheckedBindingMode,
    pub mutable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedCallerOrigin {
    Binding(CheckedBindingFact),
    BorrowedProjection { source: CheckedViewSource },
    OwnedFieldCopy { parent: CheckedViewSource },
    OwnedExpression,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedArgumentOwnership {
    pub source_span: Span,
    pub source_index: usize,
    pub parameter_index: usize,
    pub actual_type: TypeId,
    pub parameter_type: TypeId,
    pub syntax: CheckedCallerSyntax,
    pub origin: CheckedCallerOrigin,
    pub callee_access: CheckedCalleeAccess,
    pub effect: CheckedCallerEffect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedInvocationTarget {
    Resolved(DefId),
    Generic(CheckedGenericCall),
    Method(CheckedMethodCall),
    Interface(CheckedInterfaceCall),
    Indirect(TypeId),
    Intrinsic(IntrinsicId),
}

/// Each role is checked against a closed intrinsic's source operand contract.
/// HIR-added metadata is separate from these original source occurrences.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedIntrinsicOperandRole {
    Copy { ty: TypeId },
    Owned { ty: TypeId },
    View { ty: TypeId },
    PrintArgument { ty: TypeId },
}

impl CheckedIntrinsicOperandRole {
    pub fn ty(self) -> TypeId {
        match self {
            Self::Copy { ty }
            | Self::Owned { ty }
            | Self::View { ty }
            | Self::PrintArgument { ty } => ty,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedInvocationShape {
    Function {
        signature_type: TypeId,
    },
    Intrinsic {
        intrinsic: IntrinsicId,
        result_type: TypeId,
        operands: Vec<CheckedIntrinsicOperandRole>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedCallOwnership {
    pub target: CheckedInvocationTarget,
    pub shape: CheckedInvocationShape,
    pub context: CheckedOwnershipContext,
    /// Original occurrences in lexical source order, with explicit formal joins.
    pub arguments: Vec<CheckedArgumentOwnership>,
}

/// Closed source operand access; this never authorizes retaining a bare caller.
/// The successful checker signature supplies types, while this table seals arity
/// and access independently from backend physical borrowing.
pub fn intrinsic_operand_access(
    id: IntrinsicId,
    index: usize,
    arity: usize,
) -> Option<CheckedCalleeAccess> {
    use IntrinsicId::*;
    let accepted_arity = match id {
        Print | Println => true,
        Range => (1..=3).contains(&arity),
        BytesNew
        | ListNew
        | MapNew
        | SetNew
        | MathPi
        | MathE
        | TypeBitfieldFields
        | TypeBitfieldLayout
        | TypeConstructStart
        | TypeFields
        | TypeHasSecret
        | TypeInfo
        | TypeKind
        | TypeKindTag
        | TypeMachineLayout
        | TypeMachineStates
        | TypeMachineTransitions
        | TypeName
        | TypePrimitiveTag
        | TypeVariants
        | UuidNew => arity == 0,
        BitfieldFromBytes
        | BitfieldToBytes
        | BytesFromHex
        | BytesFromString
        | BytesLength
        | BytesToHex
        | BytesToString
        | ClockNow
        | CryptoMd5
        | CryptoSha256
        | CryptoSha512
        | CsvParse
        | CsvParseWithHeader
        | CsvStringify
        | EncodingBase64Decode
        | EncodingBase64Encode
        | EncodingFormDecode
        | EncodingFormEncode
        | EncodingHexDecode
        | EncodingUrlDecode
        | EncodingUrlEncode
        | EnvironmentArgs
        | Float32FromFloat64
        | Float64FromInt64
        | Float64FromString
        | Int64FromFloat64
        | Int64FromString
        | JsonParse
        | JsonParseExact
        | JsonSerialize
        | JsonSerializePublic
        | ListIsSorted
        | ListLength
        | ListSort
        | ListSum
        | MapLength
        | MathAbs
        | MathAverage
        | MathCeil
        | MathCos
        | MathFactorial
        | MathFloor
        | MathKernelAbs
        | MathLog
        | MathLog10
        | MathLog2
        | MathMedian
        | MathRound
        | MathSin
        | MathSqrt
        | MathTan
        | SecretRedact
        | SetLength
        | StringCharCount
        | StringChars
        | StringFromBool
        | StringFromFloat64
        | StringFromInt64
        | StringFromUint64
        | StringIsAlpha
        | StringIsNumeric
        | StringLines
        | StringLower
        | StringSlugify
        | StringToLowerFirst
        | StringToUpperFirst
        | StringTrim
        | StringTrimEnd
        | StringTrimStart
        | StringUpper
        | StringWords
        | TestMockClock
        | TestMockRandom
        | TypeArg
        | TypeConstructFinish
        | TypeConstructMachineStart
        | TypeConstructVariantStart
        | TypeMachineStateValue
        | TypeVariantValue
        | Uint64FromString
        | RandomBool
        | RandomUnitFloat64 => arity == 1,
        BytesConcat
        | BytesGet
        | CryptoHmacSha256
        | EnvironmentGet
        | FilesystemReadFile
        | ListAppend
        | ListGetClone
        | ListGroupBy
        | ListRemoveAt
        | ListSortBy
        | ListSortByIndex
        | LogEmit
        | MapFromLists
        | MapGet
        | MapHas
        | MapRemove
        | MathGcd
        | MathKernelMax
        | MathKernelMin
        | MathLcm
        | MathMax
        | MathMin
        | MathMod
        | MathPow
        | SecretCompare
        | SetAdd
        | SetContains
        | SetRemove
        | StdoutWrite
        | StringCount
        | StringIndexOf
        | StringJoin
        | StringRepeat
        | StringSplit
        | TestMockEnvironment
        | TypeFieldValue
        | TypeMachineFieldValue
        | TypeVariantFieldValue => arity == 2,
        BytesSlice | FilesystemWriteFile | ListInsertAt | ListSwap | MapInsert | MathClamp
        | StringReplace | StringSlice | TypeConstructPut | RandomBounded => arity == 3,
        GraphicsRun => arity == 5,
    };
    if !accepted_arity || index >= arity {
        return None;
    }
    let view = match id {
        Print | Println => false,
        BytesLength
        | BytesSlice
        | BytesToString
        | BytesGet
        | BytesToHex
        | EncodingBase64Encode
        | ListLength
        | ListGetClone
        | ListIsSorted
        | ListSum
        | MathAverage
        | MathMedian
        | MapLength
        | SetLength
        | JsonSerialize
        | JsonSerializePublic
        | TypeVariantValue
        | TypeMachineStateValue
        | TypeConstructVariantStart
        | TypeConstructMachineStart
        | ClockNow
        | EnvironmentGet
        | EnvironmentArgs
        | FilesystemReadFile
        | FilesystemWriteFile
        | StdoutWrite
        | RandomBounded
        | RandomBool
        | RandomUnitFloat64
        | LogEmit
        | GraphicsRun => index == 0,
        MapGet
        | MapHas
        | SetContains
        | TypeFieldValue
        | TypeVariantFieldValue
        | TypeMachineFieldValue
        | CryptoHmacSha256
        | SecretCompare => true,
        CryptoMd5 | CryptoSha256 | CryptoSha512 | SecretRedact => true,
        MapRemove | SetRemove | TypeConstructPut => index == 1,
        BitfieldFromBytes
        | BitfieldToBytes
        | BytesConcat
        | BytesFromHex
        | BytesFromString
        | BytesNew
        | CsvParse
        | CsvParseWithHeader
        | CsvStringify
        | EncodingBase64Decode
        | EncodingFormDecode
        | EncodingFormEncode
        | EncodingHexDecode
        | EncodingUrlDecode
        | EncodingUrlEncode
        | Float32FromFloat64
        | Float64FromInt64
        | Float64FromString
        | Int64FromFloat64
        | Int64FromString
        | JsonParse
        | JsonParseExact
        | ListAppend
        | ListGroupBy
        | ListInsertAt
        | ListNew
        | ListRemoveAt
        | ListSort
        | ListSortBy
        | ListSortByIndex
        | ListSwap
        | MapFromLists
        | MapInsert
        | MapNew
        | MathAbs
        | MathCeil
        | MathClamp
        | MathCos
        | MathE
        | MathFactorial
        | MathFloor
        | MathGcd
        | MathKernelAbs
        | MathKernelMax
        | MathKernelMin
        | MathLcm
        | MathLog
        | MathLog10
        | MathLog2
        | MathMax
        | MathMin
        | MathMod
        | MathPi
        | MathPow
        | MathRound
        | MathSin
        | MathSqrt
        | MathTan
        | Range
        | SetAdd
        | SetNew
        | StringCharCount
        | StringChars
        | StringCount
        | StringFromBool
        | StringFromFloat64
        | StringFromInt64
        | StringFromUint64
        | StringIndexOf
        | StringIsAlpha
        | StringIsNumeric
        | StringJoin
        | StringLines
        | StringLower
        | StringRepeat
        | StringReplace
        | StringSlice
        | StringSlugify
        | StringSplit
        | StringToLowerFirst
        | StringToUpperFirst
        | StringTrim
        | StringTrimEnd
        | StringTrimStart
        | StringUpper
        | StringWords
        | TestMockClock
        | TestMockEnvironment
        | TestMockRandom
        | TypeArg
        | TypeBitfieldFields
        | TypeBitfieldLayout
        | TypeConstructFinish
        | TypeConstructStart
        | TypeFields
        | TypeHasSecret
        | TypeInfo
        | TypeKind
        | TypeKindTag
        | TypeMachineLayout
        | TypeMachineStates
        | TypeMachineTransitions
        | TypeName
        | TypePrimitiveTag
        | TypeVariants
        | Uint64FromString
        | UuidNew => false,
    };
    Some(if view {
        CheckedCalleeAccess::View
    } else {
        CheckedCalleeAccess::Owned
    })
}
