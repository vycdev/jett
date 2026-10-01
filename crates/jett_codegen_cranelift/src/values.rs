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
    if id == IntrinsicId::GraphicsRun {
        let valid = if let [display, config, state, update, render] = args {
            let state_type = state.ty;
            let config_is_checked = matches!(types.resolve(config.ty), Type::Struct(id)
                if types.resolve_struct(*id).name == "graphics.Config");
            let key = types.type_ids().find(|ty| {
                matches!(types.resolve(*ty), Type::Enum(id)
                if types.resolve_enum(*id).name == "graphics.Key")
            });
            let scene = types.type_ids().find(|ty| {
                matches!(types.resolve(*ty), Type::Struct(id)
                if types.resolve_struct(*id).name == "graphics.Scene")
            });
            display.ty == T::GRAPHICS
                && config_is_checked
                && key.is_some_and(|key| {
                    matches!(types.resolve(update.ty), Type::Function { params, view_params, return_type }
                    if params.as_slice() == [state_type, key]
                        && view_params.as_slice() == [false, false]
                        && *return_type == state_type)
                })
                && scene.is_some_and(|scene| {
                    matches!(types.resolve(render.ty), Type::Function { params, view_params, return_type }
                    if params.as_slice() == [state_type]
                        && view_params.as_slice() == [true]
                        && *return_type == scene)
                })
                && matches!(types.resolve(result), Type::Result(ok, error)
                    if *ok == T::NOTHING && *error == T::STRING)
        } else {
            false
        };
        return if valid {
            Ok(())
        } else {
            Err("invalid checked Graphics runtime intrinsic".into())
        };
    }
    if id == IntrinsicId::EnvironmentGet {
        return if args.len() == 2
            && args[0].ty == T::ENVIRONMENT
            && args[1].ty == T::STRING
            && matches!(types.resolve(result), Type::Result(ok, error)
                if *error == T::STRING && matches!(types.resolve(*ok), Type::Optional(inner) if *inner == T::STRING))
        {
            Ok(())
        } else {
            Err("invalid native Environment.get signature".into())
        };
    }
    if id == IntrinsicId::EnvironmentArgs {
        return if args.len() == 1
            && args[0].ty == T::ENVIRONMENT
            && matches!(types.resolve(result), Type::List(inner) if *inner == T::STRING)
        {
            Ok(())
        } else {
            Err("invalid native Environment.args signature".into())
        };
    }
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
    if id == IntrinsicId::SecretRedact {
        return if args.len() == 1
            && jett_mir::move_values::is_secret(types, args[0].ty)
            && result == T::STRING
        {
            Ok(())
        } else {
            Err("invalid native secret redaction signature".into())
        };
    }
    if id == IntrinsicId::SecretCompare {
        let same_payload = args.len() == 2
            && jett_mir::move_values::representation_type(types, args[0].ty)
                == jett_mir::move_values::representation_type(types, args[1].ty);
        let supported_payload = args.first().is_some_and(|arg| {
            matches!(
                types.resolve(jett_mir::move_values::representation_type(types, arg.ty)),
                Type::String | Type::Bytes
            )
        });
        return if same_payload
            && supported_payload
            && args
                .iter()
                .all(|arg| jett_mir::move_values::is_secret(types, arg.ty))
            && result == T::BOOL
        {
            Ok(())
        } else {
            Err("invalid native secret comparison signature".into())
        };
    }
    if matches!(
        id,
        IntrinsicId::CryptoSha256
            | IntrinsicId::CryptoSha512
            | IntrinsicId::CryptoMd5
            | IntrinsicId::CryptoHmacSha256
    ) {
        let valid = if id == IntrinsicId::CryptoHmacSha256 {
            args.len() == 2
                && jett_mir::move_values::is_secret(types, args[0].ty)
                && jett_mir::move_values::representation_type(types, args[0].ty) == T::BYTES
                && args[1].ty == T::BYTES
                && matches!(types.resolve(result), Type::Secret(inner) if *inner == T::BYTES)
        } else {
            args.len() == 1 && args[0].ty == T::BYTES && result == T::BYTES
        };
        return if valid {
            Ok(())
        } else {
            Err("invalid native crypto intrinsic signature".into())
        };
    }
    if matches!(
        id,
        IntrinsicId::CsvParse | IntrinsicId::CsvParseWithHeader | IntrinsicId::CsvStringify
    ) {
        let string_rows = |ty| matches!(types.resolve(ty), Type::List(row) if matches!(types.resolve(*row), Type::List(field) if *field == T::STRING));
        let map_rows = |ty| matches!(types.resolve(ty), Type::List(row) if matches!(types.resolve(*row), Type::Map(key, value) if *key == T::STRING && *value == T::STRING));
        let valid = match id {
            IntrinsicId::CsvParse => {
                args.len() == 1
                    && args[0].ty == T::STRING
                    && matches!(types.resolve(result), Type::Result(rows, error) if *error == T::STRING && string_rows(*rows))
            }
            IntrinsicId::CsvParseWithHeader => {
                args.len() == 1
                    && args[0].ty == T::STRING
                    && matches!(types.resolve(result), Type::Result(rows, error) if *error == T::STRING && map_rows(*rows))
            }
            IntrinsicId::CsvStringify => {
                args.len() == 1 && string_rows(args[0].ty) && result == T::STRING
            }
            _ => false,
        };
        return if valid {
            Ok(())
        } else {
            Err("invalid native CSV intrinsic signature".into())
        };
    }
    if matches!(id, IntrinsicId::MathAverage | IntrinsicId::MathMedian) {
        return if args.len() == 1
            && result == T::FLOAT64
            && math_aggregate_kind(types, args[0].ty).is_some()
        {
            Ok(())
        } else {
            Err(format!("invalid native numeric list signature for {id}"))
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
                args.len() == 1
                    && list(args[0].ty)
                    && list_sum_kind(types, element).is_some()
                    && result == element
            }
            IntrinsicId::ListNew => args.is_empty() && list(result),
            IntrinsicId::ListAppend => {
                args.len() == 2 && list(args[0].ty) && args[1].ty == element && list(result)
            }
            IntrinsicId::ListInsertAt => {
                args.len() == 3
                    && list(args[0].ty)
                    && args[1].ty == T::INT64
                    && args[2].ty == element
                    && list(result)
            }
            IntrinsicId::ListRemoveAt => {
                args.len() == 2 && list(args[0].ty) && args[1].ty == T::INT64 && list(result)
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
            IntrinsicId::ListSortByIndex => {
                args.len() == 2
                    && matches!(types.resolve(element), Type::List(_))
                    && list(args[0].ty)
                    && args[1].ty == T::INT64
                    && list(result)
            }
            IntrinsicId::ListIsSorted => args.len() == 1 && list(args[0].ty) && result == T::BOOL,
            IntrinsicId::ListSwap => {
                args.len() == 3
                    && list(args[0].ty)
                    && args[1].ty == T::INT64
                    && args[2].ty == T::INT64
                    && list(result)
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
        IntrinsicId::Float64FromInt64 => {
            Some((vec![T::INT64], Type::Result(T::FLOAT64, T::STRING)))
        }
        IntrinsicId::Int64FromFloat64 => {
            Some((vec![T::FLOAT64], Type::Result(T::INT64, T::STRING)))
        }
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
        IntrinsicId::EncodingBase64Decode | IntrinsicId::EncodingHexDecode => {
            Some((vec![T::STRING], Type::Result(T::BYTES, T::STRING)))
        }
        IntrinsicId::EncodingUrlDecode | IntrinsicId::EncodingFormDecode => {
            Some((vec![T::STRING], Type::Result(T::STRING, T::STRING)))
        }
        _ => None,
    };
    if let Some((parameters, expected)) = sum_signature {
        if types.resolve(result) == &expected && args.iter().map(|a| a.ty).eq(parameters) {
            return Ok(());
        }
        return Err(format!("invalid native sum leaf signature for {id}"));
    }
    let (parameters, expected): (&[TypeId], TypeId) = match id {
        IntrinsicId::Float32FromFloat64 => (&[T::FLOAT64], T::FLOAT32),
        IntrinsicId::BytesNew => (&[], T::BYTES),
        IntrinsicId::BytesLength => (&[T::BYTES], T::INT64),
        IntrinsicId::BytesFromString => (&[T::STRING], T::BYTES),
        IntrinsicId::BytesConcat => (&[T::BYTES, T::BYTES], T::BYTES),
        IntrinsicId::BytesSlice => (&[T::BYTES, T::INT64, T::INT64], T::BYTES),
        IntrinsicId::BytesToHex => (&[T::BYTES], T::STRING),
        IntrinsicId::EncodingBase64Encode => (&[T::BYTES], T::STRING),
        IntrinsicId::EncodingUrlEncode | IntrinsicId::EncodingFormEncode => {
            (&[T::STRING], T::STRING)
        }
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
        IntrinsicId::ClockNow => (&[T::CLOCK], T::INT64),
        IntrinsicId::RandomBounded => (&[T::RANDOM, T::INT64, T::INT64], T::INT64),
        IntrinsicId::RandomUnitFloat64 => (&[T::RANDOM], T::FLOAT64),
        IntrinsicId::RandomBool => (&[T::RANDOM], T::BOOL),
        IntrinsicId::UuidNew => (&[], T::STRING),
        IntrinsicId::StringFromInt64 => (&[T::INT64], T::STRING),
        IntrinsicId::StringFromUint64 => (&[T::UINT64], T::STRING),
        IntrinsicId::StringFromFloat64 => (&[T::FLOAT64], T::STRING),
        IntrinsicId::StringFromBool => (&[T::BOOL], T::STRING),
        IntrinsicId::Print | IntrinsicId::Println => {
            if result == T::NOTHING
                && args.iter().all(|a| {
                    !jett_mir::move_values::is_secret(types, a.ty)
                        && crate::emit::debug::debug_layout(types, a.ty).is_some()
                })
            {
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
        IntrinsicId::EncodingBase64Encode => NativeLeaf::EncodingBase64Encode,
        IntrinsicId::EncodingBase64Decode => NativeLeaf::EncodingBase64Decode,
        IntrinsicId::EncodingHexDecode => NativeLeaf::EncodingHexDecode,
        IntrinsicId::EncodingUrlEncode => NativeLeaf::EncodingUrlEncode,
        IntrinsicId::EncodingUrlDecode => NativeLeaf::EncodingUrlDecode,
        IntrinsicId::EncodingFormEncode => NativeLeaf::EncodingFormEncode,
        IntrinsicId::EncodingFormDecode => NativeLeaf::EncodingFormDecode,
        IntrinsicId::CsvParse => NativeLeaf::CsvParse,
        IntrinsicId::CsvParseWithHeader => NativeLeaf::CsvParseWithHeader,
        IntrinsicId::CsvStringify => NativeLeaf::CsvStringify,
        IntrinsicId::CryptoSha256 => NativeLeaf::CryptoSha256,
        IntrinsicId::CryptoSha512 => NativeLeaf::CryptoSha512,
        IntrinsicId::CryptoMd5 => NativeLeaf::CryptoMd5,
        IntrinsicId::CryptoHmacSha256 => NativeLeaf::CryptoHmacSha256,
        _ => return None,
    })
}

pub(crate) fn list_intrinsic(id: IntrinsicId) -> bool {
    matches!(
        id,
        IntrinsicId::ListNew
            | IntrinsicId::ListLength
            | IntrinsicId::ListAppend
            | IntrinsicId::ListInsertAt
            | IntrinsicId::ListRemoveAt
            | IntrinsicId::ListGetClone
            | IntrinsicId::ListSum
            | IntrinsicId::ListSort
            | IntrinsicId::ListSortByIndex
            | IntrinsicId::ListIsSorted
            | IntrinsicId::ListSwap
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
    let mut current = element;
    for _ in 0..types.len() {
        match types.resolve(current) {
            Type::Refinement { base, .. } => current = *base,
            Type::Int8
            | Type::Int16
            | Type::Int32
            | Type::Int64
            | Type::Uint8
            | Type::Uint16
            | Type::Uint32
            | Type::Uint64
            | Type::Bool
            | Type::String => return true,
            _ => return false,
        }
    }
    false
}
pub(crate) fn list_sort_kind(types: &TypeInterner, element: TypeId) -> Option<NativeSortKind> {
    let mut current = element;
    for _ in 0..types.len() {
        match types.resolve(current) {
            Type::Refinement { base, .. } => current = *base,
            _ => return primitive_list_kind(types, current),
        }
    }
    None
}

fn primitive_list_kind(types: &TypeInterner, element: TypeId) -> Option<NativeSortKind> {
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

pub(crate) fn list_sum_kind(types: &TypeInterner, element: TypeId) -> Option<NativeSortKind> {
    // Sorting preserves existing refined values; summing can violate their predicates.
    let kind = primitive_list_kind(types, element)?;
    match kind {
        NativeSortKind::Int8
        | NativeSortKind::Int16
        | NativeSortKind::Int32
        | NativeSortKind::Int64
        | NativeSortKind::Uint8
        | NativeSortKind::Uint16
        | NativeSortKind::Uint32
        | NativeSortKind::Uint64
        | NativeSortKind::Float32
        | NativeSortKind::Float64 => Some(kind),
        NativeSortKind::Bool | NativeSortKind::String => None,
    }
}

// Narrow numeric values use the interpreter's wider value variants but retain
// their checked carrier width here. Unsupported shapes compare equal/sorted.
pub(crate) fn list_comparison_kind(
    types: &TypeInterner,
    element: TypeId,
) -> Option<NativeSortKind> {
    list_sort_kind(types, element)
}

pub(crate) fn math_aggregate_kind(types: &TypeInterner, list: TypeId) -> Option<NativeSortKind> {
    let Type::List(element) = types.resolve(list) else {
        return None;
    };
    Some(match *element {
        TypeInterner::INT64 => NativeSortKind::Int64,
        TypeInterner::UINT64 => NativeSortKind::Uint64,
        TypeInterner::FLOAT64 => NativeSortKind::Float64,
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

#[cfg(test)]
mod tests {
    use super::*;
    use jett_common::{FileId, Span};
    use jett_hir::ExpressionKind;

    #[test]
    fn list_sort_preserves_primitive_carriers_through_refinements() {
        let mut types = TypeInterner::new();
        for (base, expected) in [
            (TypeInterner::INT8, NativeSortKind::Int8),
            (TypeInterner::INT16, NativeSortKind::Int16),
            (TypeInterner::INT32, NativeSortKind::Int32),
            (TypeInterner::INT64, NativeSortKind::Int64),
            (TypeInterner::UINT8, NativeSortKind::Uint8),
            (TypeInterner::UINT16, NativeSortKind::Uint16),
            (TypeInterner::UINT32, NativeSortKind::Uint32),
            (TypeInterner::UINT64, NativeSortKind::Uint64),
            (TypeInterner::FLOAT32, NativeSortKind::Float32),
            (TypeInterner::FLOAT64, NativeSortKind::Float64),
            (TypeInterner::BOOL, NativeSortKind::Bool),
            (TypeInterner::STRING, NativeSortKind::String),
        ] {
            let refined = types.intern(Type::Refinement {
                name: "Refined".into(),
                base,
            });
            let nested = types.intern(Type::Refinement {
                name: "Nested".into(),
                base: refined,
            });
            for element in [base, refined, nested] {
                let arg = Expression {
                    kind: ExpressionKind::Nothing,
                    ty: types.intern(Type::List(element)),
                    span: Span::new(FileId::new(0), 0, 1),
                };
                assert_eq!(list_sort_kind(&types, element), Some(expected));
                assert_eq!(list_comparison_kind(&types, element), Some(expected));
                assert_eq!(
                    verify_intrinsic(
                        IntrinsicId::ListSort,
                        std::slice::from_ref(&arg),
                        arg.ty,
                        &types
                    ),
                    Ok(())
                );
                assert_eq!(
                    verify_intrinsic(
                        IntrinsicId::ListIsSorted,
                        std::slice::from_ref(&arg),
                        TypeInterner::BOOL,
                        &types,
                    ),
                    Ok(())
                );
                let rows = Expression {
                    kind: ExpressionKind::Nothing,
                    ty: types.intern(Type::List(arg.ty)),
                    span: arg.span,
                };
                let index = Expression {
                    kind: ExpressionKind::Nothing,
                    ty: TypeInterner::INT64,
                    span: arg.span,
                };
                let result = rows.ty;
                assert_eq!(
                    verify_intrinsic(IntrinsicId::ListSortByIndex, &[rows, index], result, &types),
                    Ok(())
                );
                if element != base {
                    assert_eq!(list_sum_kind(&types, element), None);
                    assert!(
                        verify_intrinsic(
                            IntrinsicId::ListSum,
                            std::slice::from_ref(&arg),
                            element,
                            &types,
                        )
                        .is_err()
                    );
                }
            }
        }
    }

    #[test]
    fn list_sort_requires_the_identical_nominal_list_result() {
        let mut types = TypeInterner::new();
        let positive = types.intern(Type::Refinement {
            name: "Positive".into(),
            base: TypeInterner::INT64,
        });
        let nonnegative = types.intern(Type::Refinement {
            name: "Nonnegative".into(),
            base: TypeInterner::INT64,
        });
        let nested = types.intern(Type::Refinement {
            name: "SmallPositive".into(),
            base: positive,
        });
        for element in [TypeInterner::INT64, positive, nonnegative, nested] {
            let arg = Expression {
                kind: ExpressionKind::Nothing,
                ty: types.intern(Type::List(element)),
                span: Span::new(FileId::new(0), 0, 1),
            };
            for result_element in [TypeInterner::INT64, positive, nonnegative, nested] {
                let result = types.intern(Type::List(result_element));
                assert_eq!(
                    verify_intrinsic(
                        IntrinsicId::ListSort,
                        std::slice::from_ref(&arg),
                        result,
                        &types
                    )
                    .is_ok(),
                    element == result_element
                );
            }
        }
    }

    #[test]
    fn list_sort_rejects_secret_and_aggregate_carriers_through_refinements() {
        let mut types = TypeInterner::new();
        let secret = types.intern(Type::Secret(TypeInterner::INT64));
        let list = types.intern(Type::List(TypeInterner::INT64));
        let optional = types.intern(Type::Optional(TypeInterner::STRING));
        for base in [
            TypeInterner::NOTHING,
            TypeInterner::BYTES,
            secret,
            list,
            optional,
        ] {
            let refined = types.intern(Type::Refinement {
                name: "Unsupported".into(),
                base,
            });
            for element in [base, refined] {
                let arg = Expression {
                    kind: ExpressionKind::Nothing,
                    ty: types.intern(Type::List(element)),
                    span: Span::new(FileId::new(0), 0, 1),
                };
                assert_eq!(list_sort_kind(&types, element), None);
                assert_eq!(list_comparison_kind(&types, element), None);
                assert!(
                    verify_intrinsic(
                        IntrinsicId::ListSort,
                        std::slice::from_ref(&arg),
                        arg.ty,
                        &types,
                    )
                    .is_err()
                );
            }
        }
    }

    #[test]
    fn list_sum_admits_only_matching_primitive_numeric_results() {
        let mut types = TypeInterner::new();
        for element in [
            TypeInterner::INT8,
            TypeInterner::INT16,
            TypeInterner::INT32,
            TypeInterner::INT64,
            TypeInterner::UINT8,
            TypeInterner::UINT16,
            TypeInterner::UINT32,
            TypeInterner::UINT64,
            TypeInterner::FLOAT32,
            TypeInterner::FLOAT64,
        ] {
            let arg = Expression {
                kind: ExpressionKind::Nothing,
                ty: types.intern(Type::List(element)),
                span: Span::new(FileId::new(0), 0, 1),
            };
            assert!(list_sum_kind(&types, element).is_some());
            assert_eq!(
                verify_intrinsic(IntrinsicId::ListSum, &[arg.clone()], element, &types),
                Ok(())
            );
            assert!(
                verify_intrinsic(IntrinsicId::ListSum, &[arg], TypeInterner::BOOL, &types).is_err()
            );
        }
        let refinement = types.intern(Type::Refinement {
            name: "Positive".into(),
            base: TypeInterner::INT64,
        });
        for element in [
            TypeInterner::BOOL,
            TypeInterner::STRING,
            TypeInterner::BYTES,
            refinement,
        ] {
            let arg = Expression {
                kind: ExpressionKind::Nothing,
                ty: types.intern(Type::List(element)),
                span: Span::new(FileId::new(0), 0, 1),
            };
            assert_eq!(list_sum_kind(&types, element), None);
            assert!(verify_intrinsic(IntrinsicId::ListSum, &[arg], element, &types).is_err());
        }
    }
}
