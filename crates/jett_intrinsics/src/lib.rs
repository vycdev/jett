//! Closed identities for compiler-owned Jett operations.
//!
//! Source and stdlib functions remain ordinary functions. An operation enters
//! this set only when the compiler supplies its implementation.

macro_rules! define_intrinsics {
    ($( $variant:ident => $name:literal, )+) => {
        /// One compiler-owned operation with a stable canonical identity.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum IntrinsicId {
            $( $variant, )+
        }

        impl IntrinsicId {
            /// Every intrinsic in canonical-name order.
            pub const ALL: &'static [Self] = &[
                $( Self::$variant, )+
            ];

            /// The stable spelling used in diagnostics and durable symbols.
            pub const fn canonical_name(self) -> &'static str {
                match self {
                    $( Self::$variant => $name, )+
                }
            }

            /// Classify an exact canonical spelling.
            pub fn from_canonical_name(name: &str) -> Option<Self> {
                match name {
                    $( $name => Some(Self::$variant), )+
                    _ => None,
                }
            }
        }
    };
}

define_intrinsics! {
    BitfieldFromBytes => "bitfield.from_bytes",
    BitfieldToBytes => "bitfield.to_bytes",
    BytesConcat => "bytes.__concat",
    BytesFromHex => "bytes.__from_hex",
    BytesFromString => "bytes.__from_string",
    BytesGet => "bytes.__get",
    BytesLength => "bytes.__length",
    BytesNew => "bytes.__new",
    BytesSlice => "bytes.__slice",
    BytesToHex => "bytes.__to_hex",
    BytesToString => "bytes.__to_string",
    ClockNow => "Clock.__now",
    CryptoHmacSha256 => "crypto.__hmac_sha256",
    CryptoMd5 => "crypto.__md5",
    CryptoSha256 => "crypto.__sha256",
    CryptoSha512 => "crypto.__sha512",
    CsvParse => "csv.__parse",
    CsvParseWithHeader => "csv.__parse_with_header",
    CsvStringify => "csv.__stringify",
    EncodingBase64Decode => "encoding.__base64_decode",
    EncodingBase64Encode => "encoding.__base64_encode",
    EncodingFormDecode => "encoding.__form_decode",
    EncodingFormEncode => "encoding.__form_encode",
    EncodingHexDecode => "encoding.__hex_decode",
    EncodingUrlDecode => "encoding.__url_decode",
    EncodingUrlEncode => "encoding.__url_encode",
    EnvironmentArgs => "Environment.__args",
    EnvironmentGet => "Environment.__get",
    FilesystemReadFile => "Filesystem.read_file",
    FilesystemWriteFile => "Filesystem.write_file",
    Float64FromInt64 => "float64.from_int64",
    Float64FromString => "float64.from_string",
    GraphicsRun => "graphics.__run",
    Int64FromFloat64 => "int64.from_float64",
    Int64FromString => "int64.from_string",
    JsonParse => "json.parse",
    JsonParseExact => "json.parse_exact",
    JsonSerialize => "json.serialize",
    JsonSerializePublic => "json.serialize_public",
    ListAppend => "list.__append",
    ListGetClone => "list.__get_clone",
    ListGroupBy => "list.__group_by",
    ListInsertAt => "list.__insert_at",
    ListIsSorted => "list.__is_sorted",
    ListLength => "list.__length",
    ListNew => "list.__new",
    ListRemoveAt => "list.__remove_at",
    ListSort => "list.__sort",
    ListSortBy => "list.__sort_by",
    ListSortByIndex => "list.__sort_by_index",
    ListSum => "list.__sum",
    ListSwap => "list.__swap",
    LogEmit => "log.__emit",
    MapFromLists => "map.__from_lists",
    MapGet => "map.__get",
    MapHas => "map.__has",
    MapInsert => "map.__insert",
    MapLength => "map.__length",
    MapNew => "map.__new",
    MapRemove => "map.__remove",
    MathAbs => "math.abs",
    MathAverage => "math.__average",
    MathCeil => "math.__ceil",
    MathClamp => "math.__clamp",
    MathCos => "math.__cos",
    MathE => "math.__e",
    MathFactorial => "math.__factorial",
    MathFloor => "math.__floor",
    MathGcd => "math.__gcd",
    MathKernelAbs => "math.__abs",
    MathKernelMax => "math.__max",
    MathKernelMin => "math.__min",
    MathLcm => "math.__lcm",
    MathLog => "math.__log",
    MathLog10 => "math.__log10",
    MathLog2 => "math.__log2",
    MathMax => "math.max",
    MathMedian => "math.__median",
    MathMin => "math.min",
    MathMod => "math.__mod",
    MathPi => "math.__pi",
    MathPow => "math.__pow",
    MathRound => "math.__round",
    MathSin => "math.__sin",
    MathSqrt => "math.__sqrt",
    MathTan => "math.__tan",
    Print => "print",
    Println => "println",
    RandomBool => "random.__bool",
    RandomBounded => "random.__bounded",
    RandomUnitFloat64 => "random.__unit_float64",
    Range => "range",
    SecretCompare => "secret.compare",
    SecretRedact => "secret.redact",
    SetAdd => "set.__add",
    SetContains => "set.__contains",
    SetLength => "set.__length",
    SetNew => "set.__new",
    SetRemove => "set.__remove",
    StdoutWrite => "Stdout.write",
    StringCharCount => "string.__char_count",
    StringChars => "string.__chars",
    StringCount => "string.__count",
    StringFromBool => "string.__from_bool",
    StringFromFloat64 => "string.__from_float64",
    StringFromInt64 => "string.__from_int64",
    StringFromUint64 => "string.__from_uint64",
    StringIndexOf => "string.__index_of",
    StringIsAlpha => "string.__is_alpha",
    StringIsNumeric => "string.__is_numeric",
    StringJoin => "string.__join",
    StringLines => "string.__lines",
    StringLower => "string.__lower",
    StringRepeat => "string.__repeat",
    StringReplace => "string.__replace",
    StringSlice => "string.__slice",
    StringSlugify => "string.__slugify",
    StringSplit => "string.__split",
    StringToLowerFirst => "string.__to_lower_first",
    StringToUpperFirst => "string.__to_upper_first",
    StringTrim => "string.__trim",
    StringTrimEnd => "string.__trim_end",
    StringTrimStart => "string.__trim_start",
    StringUpper => "string.__upper",
    StringWords => "string.__words",
    TestMockClock => "test.mock.__clock",
    TestMockEnvironment => "test.mock.__environment",
    TestMockRandom => "test.mock.__random",
    TypeArg => "type.arg",
    TypeBitfieldFields => "type.bitfield_fields",
    TypeBitfieldLayout => "type.bitfield_layout",
    TypeConstructFinish => "type.construct_finish",
    TypeConstructMachineStart => "type.construct_machine_start",
    TypeConstructPut => "type.construct_put",
    TypeConstructStart => "type.construct_start",
    TypeConstructVariantStart => "type.construct_variant_start",
    TypeFieldValue => "type.field_value",
    TypeFields => "type.fields",
    TypeHasSecret => "type.has_secret",
    TypeInfo => "type.info",
    TypeKind => "type.kind",
    TypeKindTag => "type.kind_tag",
    TypeMachineFieldValue => "type.machine_field_value",
    TypeMachineLayout => "type.machine_layout",
    TypeMachineStateValue => "type.machine_state_value",
    TypeMachineStates => "type.machine_states",
    TypeMachineTransitions => "type.machine_transitions",
    TypeName => "type.name",
    TypePrimitiveTag => "type.primitive_tag",
    TypeVariantFieldValue => "type.variant_field_value",
    TypeVariantValue => "type.variant_value",
    TypeVariants => "type.variants",
    Uint64FromString => "uint64.from_string",
    UuidNew => "uuid.new",
}

impl IntrinsicId {
    /// Classify a checked call spelling.
    ///
    /// Bitfield conversion methods carry a nominal owner in source. Their
    /// closed operation identity is independent of that owner; the HIR value
    /// and result types retain the concrete bitfield type.
    pub fn from_callable_name(name: &str) -> Option<Self> {
        Self::from_canonical_name(name).or_else(|| {
            let (_, method) = name.rsplit_once('.')?;
            match method {
                "from_bytes" => Some(Self::BitfieldFromBytes),
                "to_bytes" => Some(Self::BitfieldToBytes),
                _ => None,
            }
        })
    }
}

impl std::fmt::Display for IntrinsicId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.canonical_name())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn canonical_names_are_unique_and_round_trip() {
        let mut names = HashSet::new();
        for &intrinsic in IntrinsicId::ALL {
            let name = intrinsic.canonical_name();
            assert!(names.insert(name), "duplicate intrinsic name `{name}`");
            assert_eq!(IntrinsicId::from_canonical_name(name), Some(intrinsic));
            assert_eq!(IntrinsicId::from_callable_name(name), Some(intrinsic));
        }
    }

    #[test]
    fn checked_bitfield_method_names_have_closed_identities() {
        assert_eq!(
            IntrinsicId::from_callable_name("packet.Header.to_bytes"),
            Some(IntrinsicId::BitfieldToBytes)
        );
        assert_eq!(
            IntrinsicId::from_callable_name("packet.Header.from_bytes"),
            Some(IntrinsicId::BitfieldFromBytes)
        );
    }

    #[test]
    fn unknown_names_are_not_intrinsics() {
        assert_eq!(IntrinsicId::from_callable_name("app.user_function"), None);
        assert_eq!(
            IntrinsicId::from_callable_name("type.future_operation"),
            None
        );
    }
}
