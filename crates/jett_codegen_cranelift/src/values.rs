use crate::CodegenError;
use cranelift_codegen::ir::{self, AbiParam};
use cranelift_module::{FuncId, Linkage, Module};
use cranelift_object::ObjectModule;
use jett_hir::{Expression, IntrinsicId};
use jett_runtime::native_abi::values::{AbiScalar, NativeLeaf, NativeSortKind};
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
    if id == IntrinsicId::BitfieldToBytes {
        return if args.len() == 1
            && matches!(types.resolve(args[0].ty), Type::Bitfield(_))
            && result == T::BYTES
        {
            Ok(())
        } else {
            Err("invalid native bitfield encoding signature".into())
        };
    }
    if id == IntrinsicId::BitfieldFromBytes {
        return if args.len() == 1
            && args[0].ty == T::BYTES
            && matches!(types.resolve(result), Type::Result(ok, error) if matches!(types.resolve(*ok), Type::Bitfield(_)) && *error == T::STRING)
        {
            Ok(())
        } else {
            Err("invalid native bitfield decoding signature".into())
        };
    }
    if id == IntrinsicId::Range {
        return if (1..=3).contains(&args.len())
            && args.iter().all(|a| a.ty == T::INT64)
            && matches!(types.resolve(result), Type::List(inner) if *inner == T::INT64)
        {
            Ok(())
        } else {
            Err("invalid native range signature".into())
        };
    }
    if let Some(leaf) = math_leaf(id, args.first().map(|a| a.ty)) {
        let parameters = &leaf.parameters()[1..];
        let native_type = |ty| match ty {
            AbiScalar::I64 => T::INT64,
            AbiScalar::F64 => T::FLOAT64,
            _ => T::ERROR,
        };
        if result == native_type(leaf.result())
            && args
                .iter()
                .map(|a| a.ty)
                .eq(parameters.iter().copied().map(native_type))
        {
            return Ok(());
        }
        return Err(format!("invalid native numeric signature for {id}"));
    }
    if matches!(
        id,
        IntrinsicId::StringChars
            | IntrinsicId::StringWords
            | IntrinsicId::StringLines
            | IntrinsicId::StringSplit
            | IntrinsicId::StringJoin
    ) {
        let list = |ty| matches!(types.resolve(ty), Type::List(inner) if *inner == T::STRING);
        let valid = if id == IntrinsicId::StringJoin {
            args.len() == 2 && list(args[0].ty) && args[1].ty == T::STRING && result == T::STRING
        } else {
            args.len() == if id == IntrinsicId::StringSplit { 2 } else { 1 }
                && args.iter().all(|a| a.ty == T::STRING)
                && list(result)
        };
        return if valid {
            Ok(())
        } else {
            Err("invalid native string-list signature".into())
        };
    }
    if list_intrinsic(id) {
        let element = list_element(id, args, result, types)
            .ok_or_else(|| "invalid native list type".to_string())?;
        let list = |ty| matches!(types.resolve(ty), Type::List(inner) if *inner == element);
        let valid = match id {
            IntrinsicId::ListSum => {
                args.len() == 1 && list(args[0].ty) && element == T::INT64 && result == T::INT64
            }
            IntrinsicId::ListNew => args.is_empty() && list(result),
            IntrinsicId::ListAppend => {
                args.len() == 2 && list(args[0].ty) && args[1].ty == element && list(result)
            }
            IntrinsicId::ListLength => args.len() == 1 && list(args[0].ty) && result == T::INT64,
            IntrinsicId::ListGetClone => {
                args.len() == 2
                    && list(args[0].ty)
                    && args[1].ty == T::INT64
                    && matches!(types.resolve(result), Type::Optional(inner) if *inner == element)
            }
            IntrinsicId::ListSort => {
                args.len() == 1
                    && list(args[0].ty)
                    && list(result)
                    && list_sort_kind(types, element).is_some()
            }
            _ => false,
        };
        return if valid {
            Ok(())
        } else {
            Err(format!("invalid native list signature for {id}"))
        };
    }
    if set_intrinsic(id) {
        let element = set_element(id, args, result, types)
            .ok_or_else(|| "invalid native set type".to_string())?;
        if !set_element_supported(types, element) {
            return Err("unsupported native set element".into());
        }
        let set = |ty| matches!(types.resolve(ty), Type::Set(inner) if *inner == element);
        let valid = match id {
            IntrinsicId::SetNew => args.is_empty() && set(result),
            IntrinsicId::SetAdd | IntrinsicId::SetRemove => {
                args.len() == 2 && set(args[0].ty) && args[1].ty == element && set(result)
            }
            IntrinsicId::SetContains => {
                args.len() == 2 && set(args[0].ty) && args[1].ty == element && result == T::BOOL
            }
            IntrinsicId::SetLength => args.len() == 1 && set(args[0].ty) && result == T::INT64,
            _ => false,
        };
        return if valid {
            Ok(())
        } else {
            Err(format!("invalid native set signature for {id}"))
        };
    }
    if map_intrinsic(id) {
        let (key, value) = map_types(id, args, result, types)
            .ok_or_else(|| "invalid native map type".to_string())?;
        if !set_element_supported(types, key) {
            return Err("unsupported native map key".into());
        }
        let map = |ty| matches!(types.resolve(ty), Type::Map(k, v) if *k == key && *v == value);
        let valid = match id {
            IntrinsicId::MapNew => args.is_empty() && map(result),
            IntrinsicId::MapLength => args.len() == 1 && map(args[0].ty) && result == T::INT64,
            IntrinsicId::MapHas => {
                args.len() == 2 && map(args[0].ty) && args[1].ty == key && result == T::BOOL
            }
            IntrinsicId::MapGet => {
                args.len() == 2
                    && map(args[0].ty)
                    && args[1].ty == key
                    && matches!(types.resolve(result), Type::Optional(inner) if *inner == value)
            }
            IntrinsicId::MapInsert => {
                args.len() == 3
                    && map(args[0].ty)
                    && args[1].ty == key
                    && args[2].ty == value
                    && map(result)
            }
            IntrinsicId::MapRemove => {
                args.len() == 2 && map(args[0].ty) && args[1].ty == key && map(result)
            }
            IntrinsicId::MapFromLists => {
                args.len() == 2
                    && matches!(types.resolve(args[0].ty), Type::List(inner) if *inner == key)
                    && matches!(types.resolve(args[1].ty), Type::List(inner) if *inner == value)
                    && map(result)
            }
            _ => false,
        };
        return if valid {
            Ok(())
        } else {
            Err(format!("invalid native map signature for {id}"))
        };
    }
    let sum_signature = match id {
        IntrinsicId::Int64FromString => Some((vec![T::STRING], Type::Result(T::INT64, T::STRING))),
        IntrinsicId::Uint64FromString => {
            Some((vec![T::STRING], Type::Result(T::UINT64, T::STRING)))
        }
        IntrinsicId::Float64FromString => {
            Some((vec![T::STRING], Type::Result(T::FLOAT64, T::STRING)))
        }
        IntrinsicId::BytesGet => Some((vec![T::BYTES, T::INT64], Type::Optional(T::INT64))),
        IntrinsicId::BytesToString => Some((vec![T::BYTES], Type::Result(T::STRING, T::STRING))),
        IntrinsicId::BytesFromHex => Some((vec![T::STRING], Type::Result(T::BYTES, T::STRING))),
        _ => None,
    };
    if let Some((parameters, expected)) = sum_signature {
        if types.resolve(result) == &expected && args.iter().map(|a| a.ty).eq(parameters) {
            return Ok(());
        }
        return Err(format!("invalid native sum leaf signature for {id}"));
    }
    let (parameters, expected): (&[TypeId], TypeId) = match id {
        IntrinsicId::BytesNew => (&[], T::BYTES),
        IntrinsicId::BytesLength => (&[T::BYTES], T::INT64),
        IntrinsicId::BytesFromString => (&[T::STRING], T::BYTES),
        IntrinsicId::BytesConcat => (&[T::BYTES, T::BYTES], T::BYTES),
        IntrinsicId::BytesSlice => (&[T::BYTES, T::INT64, T::INT64], T::BYTES),
        IntrinsicId::BytesToHex => (&[T::BYTES], T::STRING),
        IntrinsicId::StringCharCount => (&[T::STRING], T::INT64),
        IntrinsicId::StringIndexOf => {
            if args.len() == 2
                && args.iter().all(|arg| arg.ty == T::STRING)
                && matches!(types.resolve(result), Type::Optional(inner) if *inner == T::INT64)
            {
                return Ok(());
            }
            return Err("invalid native string index signature".into());
        }
        IntrinsicId::StringCount => (&[T::STRING, T::STRING], T::INT64),
        IntrinsicId::StringReplace => (&[T::STRING, T::STRING, T::STRING], T::STRING),
        IntrinsicId::StringSlice => (&[T::STRING, T::INT64, T::INT64], T::STRING),
        IntrinsicId::StringUpper
        | IntrinsicId::StringLower
        | IntrinsicId::StringTrim
        | IntrinsicId::StringTrimStart
        | IntrinsicId::StringTrimEnd
        | IntrinsicId::StringSlugify
        | IntrinsicId::StringToUpperFirst
        | IntrinsicId::StringToLowerFirst => (&[T::STRING], T::STRING),
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
        IntrinsicId::StringChars => NativeLeaf::StringChars,
        IntrinsicId::StringWords => NativeLeaf::StringWords,
        IntrinsicId::StringLines => NativeLeaf::StringLines,
        IntrinsicId::StringSplit => NativeLeaf::StringSplit,
        IntrinsicId::StringJoin => NativeLeaf::StringJoin,
        IntrinsicId::StringCharCount => NativeLeaf::CharCount,
        IntrinsicId::StringIndexOf => NativeLeaf::StringIndexOf,
        IntrinsicId::StringCount => NativeLeaf::StringCount,
        IntrinsicId::StringReplace => NativeLeaf::StringReplace,
        IntrinsicId::StringSlugify => NativeLeaf::StringSlugify,
        IntrinsicId::StringToUpperFirst => NativeLeaf::StringToUpperFirst,
        IntrinsicId::StringToLowerFirst => NativeLeaf::StringToLowerFirst,
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

pub(crate) fn math_leaf(id: IntrinsicId, first: Option<TypeId>) -> Option<NativeLeaf> {
    Some(match id {
        IntrinsicId::MathKernelAbs | IntrinsicId::MathAbs => match first? {
            TypeInterner::INT64 => NativeLeaf::IntAbs,
            TypeInterner::FLOAT64 => NativeLeaf::FloatAbs,
            _ => return None,
        },
        IntrinsicId::MathKernelMin | IntrinsicId::MathMin => match first? {
            TypeInterner::INT64 => NativeLeaf::IntMin,
            TypeInterner::FLOAT64 => NativeLeaf::FloatMin,
            _ => return None,
        },
        IntrinsicId::MathKernelMax | IntrinsicId::MathMax => match first? {
            TypeInterner::INT64 => NativeLeaf::IntMax,
            TypeInterner::FLOAT64 => NativeLeaf::FloatMax,
            _ => return None,
        },
        IntrinsicId::MathSqrt => NativeLeaf::Sqrt,
        IntrinsicId::MathFloor => NativeLeaf::Floor,
        IntrinsicId::MathCeil => NativeLeaf::Ceil,
        IntrinsicId::MathRound => NativeLeaf::Round,
        IntrinsicId::MathLog => NativeLeaf::Log,
        IntrinsicId::MathLog2 => NativeLeaf::Log2,
        IntrinsicId::MathLog10 => NativeLeaf::Log10,
        IntrinsicId::MathSin => NativeLeaf::Sin,
        IntrinsicId::MathCos => NativeLeaf::Cos,
        IntrinsicId::MathTan => NativeLeaf::Tan,
        IntrinsicId::MathPow => NativeLeaf::Pow,
        IntrinsicId::MathPi => NativeLeaf::Pi,
        IntrinsicId::MathE => NativeLeaf::E,
        IntrinsicId::MathClamp => NativeLeaf::Clamp,
        IntrinsicId::MathMod => NativeLeaf::Mod,
        IntrinsicId::MathGcd => NativeLeaf::Gcd,
        IntrinsicId::MathLcm => NativeLeaf::Lcm,
        IntrinsicId::MathFactorial => NativeLeaf::Factorial,
        _ => return None,
    })
}

pub(crate) fn bytes_leaf(id: IntrinsicId) -> Option<NativeLeaf> {
    Some(match id {
        IntrinsicId::Int64FromString => NativeLeaf::ParseInt,
        IntrinsicId::Uint64FromString => NativeLeaf::ParseUint,
        IntrinsicId::Float64FromString => NativeLeaf::ParseFloat,
        IntrinsicId::BytesNew => NativeLeaf::BytesNew,
        IntrinsicId::BytesLength => NativeLeaf::BytesLength,
        IntrinsicId::BytesFromString => NativeLeaf::BytesFromString,
        IntrinsicId::BytesConcat => NativeLeaf::BytesConcat,
        IntrinsicId::BytesSlice => NativeLeaf::BytesSlice,
        IntrinsicId::BytesToHex => NativeLeaf::BytesToHex,
        IntrinsicId::BytesGet => NativeLeaf::BytesGet,
        IntrinsicId::BytesToString => NativeLeaf::BytesToString,
        IntrinsicId::BytesFromHex => NativeLeaf::BytesFromHex,
        _ => return None,
    })
}

pub(crate) fn list_intrinsic(id: IntrinsicId) -> bool {
    matches!(
        id,
        IntrinsicId::ListNew
            | IntrinsicId::ListLength
            | IntrinsicId::ListAppend
            | IntrinsicId::ListGetClone
            | IntrinsicId::ListSum
            | IntrinsicId::ListSort
    )
}
pub(crate) fn set_intrinsic(id: IntrinsicId) -> bool {
    matches!(
        id,
        IntrinsicId::SetNew
            | IntrinsicId::SetAdd
            | IntrinsicId::SetRemove
            | IntrinsicId::SetContains
            | IntrinsicId::SetLength
    )
}
pub(crate) fn map_intrinsic(id: IntrinsicId) -> bool {
    matches!(
        id,
        IntrinsicId::MapNew
            | IntrinsicId::MapLength
            | IntrinsicId::MapHas
            | IntrinsicId::MapGet
            | IntrinsicId::MapInsert
            | IntrinsicId::MapRemove
            | IntrinsicId::MapFromLists
    )
}
pub(crate) fn map_types(
    id: IntrinsicId,
    args: &[Expression],
    result: TypeId,
    types: &TypeInterner,
) -> Option<(TypeId, TypeId)> {
    let ty = if matches!(id, IntrinsicId::MapNew | IntrinsicId::MapFromLists) {
        result
    } else {
        args.first()?.ty
    };
    if let Type::Map(key, value) = types.resolve(ty) {
        Some((*key, *value))
    } else {
        None
    }
}
pub(crate) fn set_element(
    id: IntrinsicId,
    args: &[Expression],
    result: TypeId,
    types: &TypeInterner,
) -> Option<TypeId> {
    let ty = if id == IntrinsicId::SetNew {
        result
    } else {
        args.first()?.ty
    };
    if let Type::Set(inner) = types.resolve(ty) {
        Some(*inner)
    } else {
        None
    }
}
fn set_element_supported(types: &TypeInterner, element: TypeId) -> bool {
    matches!(
        types.resolve(element),
        Type::Int8
            | Type::Int16
            | Type::Int32
            | Type::Int64
            | Type::Uint8
            | Type::Uint16
            | Type::Uint32
            | Type::Uint64
            | Type::Bool
            | Type::String
    )
}
pub(crate) fn list_sort_kind(types: &TypeInterner, element: TypeId) -> Option<NativeSortKind> {
    Some(match types.resolve(element) {
        Type::Int8 => NativeSortKind::Int8,
        Type::Int16 => NativeSortKind::Int16,
        Type::Int32 => NativeSortKind::Int32,
        Type::Int64 => NativeSortKind::Int64,
        Type::Uint8 => NativeSortKind::Uint8,
        Type::Uint16 => NativeSortKind::Uint16,
        Type::Uint32 => NativeSortKind::Uint32,
        Type::Uint64 => NativeSortKind::Uint64,
        Type::Float32 => NativeSortKind::Float32,
        Type::Float64 => NativeSortKind::Float64,
        Type::Bool => NativeSortKind::Bool,
        Type::String => NativeSortKind::String,
        _ => return None,
    })
}
pub(crate) fn list_element(
    id: IntrinsicId,
    args: &[Expression],
    result: TypeId,
    types: &TypeInterner,
) -> Option<TypeId> {
    let ty = if id == IntrinsicId::ListNew {
        result
    } else {
        args.first()?.ty
    };
    if let Type::List(inner) = types.resolve(ty) {
        Some(*inner)
    } else {
        None
    }
}
