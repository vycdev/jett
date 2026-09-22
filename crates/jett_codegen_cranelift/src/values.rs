use crate::CodegenError;
use cranelift_codegen::ir::{self, AbiParam};
use cranelift_module::{FuncId, Linkage, Module};
use cranelift_object::ObjectModule;
use jett_hir::{Expression, IntrinsicId};
use jett_runtime::native_abi::values::{AbiScalar, NativeLeaf};
use jett_types::{Type, TypeId, TypeInterner};

pub(crate) fn abi_type(ty: AbiScalar) -> ir::Type {
    match ty {
        AbiScalar::Pointer | AbiScalar::I64 => ir::types::I64,
        AbiScalar::I32 => ir::types::I32,
        AbiScalar::F64 => ir::types::F64,
    }
}
pub(crate) fn declare_leaf(
    module: &mut ObjectModule,
    leaf: NativeLeaf,
) -> Result<FuncId, CodegenError> {
    let mut signature = module.make_signature();
    signature.params.extend(
        leaf.parameters()
            .iter()
            .map(|t| AbiParam::new(abi_type(*t))),
    );
    signature
        .returns
        .push(AbiParam::new(abi_type(leaf.result())));
    module
        .declare_function(leaf.symbol(), Linkage::Import, &signature)
        .map_err(|e| CodegenError::Backend(e.to_string()))
}
pub(crate) fn is_formattable(types: &TypeInterner, ty: TypeId) -> bool {
    matches!(
        types.resolve(ty),
        Type::String
            | Type::Bool
            | Type::Int8
            | Type::Int16
            | Type::Int32
            | Type::Int64
            | Type::Uint8
            | Type::Uint16
            | Type::Uint32
            | Type::Uint64
            | Type::Float32
            | Type::Float64
            | Type::Nothing
    )
}
pub(crate) fn verify_intrinsic(
    id: IntrinsicId,
    args: &[Expression],
    result: TypeId,
    types: &TypeInterner,
) -> Result<(), String> {
    use TypeInterner as T;
    let (parameters, expected): (&[TypeId], TypeId) = match id {
        IntrinsicId::StringCharCount => (&[T::STRING], T::INT64),
        IntrinsicId::StringSlice => (&[T::STRING, T::INT64, T::INT64], T::STRING),
        IntrinsicId::StringUpper
        | IntrinsicId::StringLower
        | IntrinsicId::StringTrim
        | IntrinsicId::StringTrimStart
        | IntrinsicId::StringTrimEnd => (&[T::STRING], T::STRING),
        IntrinsicId::StringIsAlpha | IntrinsicId::StringIsNumeric => (&[T::STRING], T::BOOL),
        IntrinsicId::StringRepeat => (&[T::STRING, T::INT64], T::STRING),
        IntrinsicId::StdoutWrite => (&[T::STDOUT, T::STRING], T::NOTHING),
        IntrinsicId::StringFromInt64 => (&[T::INT64], T::STRING),
        IntrinsicId::StringFromUint64 => (&[T::UINT64], T::STRING),
        IntrinsicId::StringFromFloat64 => (&[T::FLOAT64], T::STRING),
        IntrinsicId::StringFromBool => (&[T::BOOL], T::STRING),
        IntrinsicId::Print | IntrinsicId::Println => {
            if result == T::NOTHING && args.iter().all(|a| is_formattable(types, a.ty)) {
                return Ok(());
            }
            return Err("invalid native print signature".into());
        }
        _ => return Err(format!("unsupported native intrinsic {id}")),
    };
    if result != expected || args.iter().map(|a| a.ty).ne(parameters.iter().copied()) {
        return Err(format!("invalid native signature for {id}"));
    }
    Ok(())
}

/// Exact checked identities, not canonical-name inference.
pub(crate) fn string_leaf(id: IntrinsicId) -> Option<NativeLeaf> {
    Some(match id {
        IntrinsicId::StringCharCount => NativeLeaf::CharCount,
        IntrinsicId::StringSlice => NativeLeaf::Slice,
        IntrinsicId::StringUpper => NativeLeaf::Upper,
        IntrinsicId::StringLower => NativeLeaf::Lower,
        IntrinsicId::StringTrim => NativeLeaf::Trim,
        IntrinsicId::StringTrimStart => NativeLeaf::TrimStart,
        IntrinsicId::StringTrimEnd => NativeLeaf::TrimEnd,
        IntrinsicId::StringIsAlpha => NativeLeaf::IsAlpha,
        IntrinsicId::StringIsNumeric => NativeLeaf::IsNumeric,
        IntrinsicId::StringRepeat => NativeLeaf::Repeat,
        _ => return None,
    })
}
