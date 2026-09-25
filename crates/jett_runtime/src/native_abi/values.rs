//! Typed native leaf operations. See docs/active/native_value_abi.md.
//! Every pointer must refer to a live stationary ABI context, except literal
//! bytes which are borrowed for the call. No Rust value crosses this ABI.
use super::*;
use crate::clock;
use crate::crypto;
use crate::csv;
use crate::encoding;
use crate::environment::{self, LaunchEnvironmentSnapshot};
use crate::math;
use crate::random::{self, RandomProvider};
use std::cmp::Ordering as CompareOrdering;
use std::sync::atomic::{AtomicU64, Ordering};
use subtle::ConstantTimeEq;
use unicode_segmentation::UnicodeSegmentation;

pub type NativeHandle = u64;
type Failure = (JettRuntimeStatusV1, &'static [u8]);
type LeafResult<T> = Result<T, Failure>;
const INVALID_HANDLE: Failure = (
    JettRuntimeStatusV1::INVALID_ARGUMENT,
    b"invalid native string handle",
);
const INVALID_STRING_SCALAR_INDEX: Failure = (
    JettRuntimeStatusV1::INVALID_ARGUMENT,
    b"invalid native string scalar index",
);
const INVALID_STRUCT: Failure = (
    JettRuntimeStatusV1::INVALID_ARGUMENT,
    b"invalid native struct handle or field",
);
const INVALID_ACTOR: Failure = (
    JettRuntimeStatusV1::INVALID_ARGUMENT,
    b"invalid native actor handle or field",
);
const INVALID_CONSTRUCTION: Failure = (
    JettRuntimeStatusV1::INVALID_ARGUMENT,
    b"invalid native TypeConstruction builder",
);
const INVALID_CONSTRUCTION_LAYOUT: Failure = (
    JettRuntimeStatusV1::INVALID_ARGUMENT,
    b"invalid native TypeConstruction layout",
);
const INVALID_REFLECTED_FIELD: Failure = (
    JettRuntimeStatusV1::INVALID_ARGUMENT,
    b"type.field_value: field metadata does not match the checked owner and requested type",
);
const INVALID_REFLECTED_VARIANT_FIELD: Failure = (
    JettRuntimeStatusV1::INVALID_ARGUMENT,
    b"type.variant_field_value: field metadata does not match the active variant and requested type",
);
const INVALID_REFLECTED_MACHINE_FIELD: Failure = (
    JettRuntimeStatusV1::INVALID_ARGUMENT,
    b"type.machine_field_value: field metadata does not match the active state and requested type",
);
const INVALID_TYPE_INFO: Failure = (
    JettRuntimeStatusV1::INVALID_ARGUMENT,
    b"invalid reflected TypeInfo for native type dispatch",
);
const INVALID_TYPE_ARG_INDEX: Failure = (
    JettRuntimeStatusV1::INVALID_ARGUMENT,
    b"type.arg index is out of range for the checked type",
);
const INVALID_LIST: Failure = (
    JettRuntimeStatusV1::INVALID_ARGUMENT,
    b"invalid native list handle",
);
const INVALID_SET: Failure = (
    JettRuntimeStatusV1::INVALID_ARGUMENT,
    b"invalid native set handle or element",
);
const INVALID_MAP: Failure = (
    JettRuntimeStatusV1::INVALID_ARGUMENT,
    b"invalid native map handle or entry",
);
const INVALID_BYTES: Failure = (
    JettRuntimeStatusV1::INVALID_ARGUMENT,
    b"invalid native bytes handle",
);
const INVALID_BITFIELD_LAYOUT: Failure = (
    JettRuntimeStatusV1::INVALID_ARGUMENT,
    b"invalid native bitfield layout",
);
const INVALID_TRACE_LABEL: Failure = (
    JettRuntimeStatusV1::INVALID_ARGUMENT,
    b"invalid native trace label",
);
const INVALID_SUM: Failure = (
    JettRuntimeStatusV1::INVALID_ARGUMENT,
    b"invalid native sum handle or tag",
);
const EXHAUSTED: Failure = (
    JettRuntimeStatusV1::RESOURCE_EXHAUSTED,
    b"native value capacity exhausted",
);

struct NativeString {
    text: String,
    references: u64,
}
/// Stable discriminants for optional and result storage (not terminal status).
pub const SUM_FAILURE: u32 = 0;
pub const SUM_SUCCESS: u32 = 1;
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeSortKind {
    Int8,
    Int16,
    Int32,
    Int64,
    Uint8,
    Uint16,
    Uint32,
    Uint64,
    Float32,
    Float64,
    Bool,
    String,
}
impl NativeSortKind {
    fn from_raw(value: u32) -> LeafResult<Self> {
        Ok(match value {
            0 => Self::Int8,
            1 => Self::Int16,
            2 => Self::Int32,
            3 => Self::Int64,
            4 => Self::Uint8,
            5 => Self::Uint16,
            6 => Self::Uint32,
            7 => Self::Uint64,
            8 => Self::Float32,
            9 => Self::Float64,
            10 => Self::Bool,
            11 => Self::String,
            _ => return Err(INVALID_LIST),
        })
    }
    fn from_comparison_raw(value: u32) -> LeafResult<Option<Self>> {
        if value == u32::MAX {
            return Ok(None);
        }
        let kind = Self::from_raw(value)?;
        if matches!(
            kind,
            Self::Int64 | Self::Uint64 | Self::Float64 | Self::Bool | Self::String
        ) {
            Ok(Some(kind))
        } else {
            Err(INVALID_LIST)
        }
    }
    fn compare(self, left: u64, right: u64) -> CompareOrdering {
        match self {
            Self::Int8 => (left as u8 as i8).cmp(&(right as u8 as i8)),
            Self::Int16 => (left as u16 as i16).cmp(&(right as u16 as i16)),
            Self::Int32 => (left as u32 as i32).cmp(&(right as u32 as i32)),
            Self::Int64 => (left as i64).cmp(&(right as i64)),
            Self::Uint8 | Self::Uint16 | Self::Uint32 | Self::Uint64 | Self::Bool => {
                left.cmp(&right)
            }
            Self::Float32 => f32::from_bits(left as u32)
                .partial_cmp(&f32::from_bits(right as u32))
                .unwrap_or(CompareOrdering::Equal),
            Self::Float64 => f64::from_bits(left)
                .partial_cmp(&f64::from_bits(right))
                .unwrap_or(CompareOrdering::Equal),
            Self::String => unreachable!("string ordering uses the string registry"),
        }
    }
    fn valid_bits(self, bits: u64) -> bool {
        match self {
            Self::Int8 | Self::Uint8 => bits <= u8::MAX as u64,
            Self::Int16 | Self::Uint16 => bits <= u16::MAX as u64,
            Self::Int32 | Self::Uint32 | Self::Float32 => bits <= u32::MAX as u64,
            Self::Bool => bits <= 1,
            Self::Int64 | Self::Uint64 | Self::Float64 | Self::String => true,
        }
    }
}
#[derive(Clone, Copy)]
struct NativeField {
    bits: u64,
    owned: bool,
}
struct NativeStruct {
    fields: Vec<Option<NativeField>>,
}
#[derive(Clone)]
struct NativeBuilderInfo {
    owner: String,
    metadata_owner: String,
    unsupported_kind: Option<String>,
    variant: Option<String>,
    state: Option<String>,
    target_state: Option<String>,
    field_offset: usize,
    field_names: Vec<String>,
    field_type_names: Vec<String>,
    field_types: Vec<String>,
    validation: Vec<BuilderFieldValidation>,
}
#[derive(Clone)]
enum BuilderFieldValidation {
    None,
    SignedBits(u32),
    UnsignedBits(u32),
    EnumBits(u32, String, Vec<(String, i64)>),
}
struct NativeList {
    elements: Vec<Option<u64>>,
    owned: bool,
}
struct NativeSet {
    elements: Vec<Option<u64>>,
    strings: bool,
}
#[derive(Clone, Copy)]
struct NativeMapEntry {
    key: u64,
    value: u64,
    key_taken: bool,
}
struct NativeMap {
    entries: Vec<Option<NativeMapEntry>>,
    key_strings: bool,
    value_owned: bool,
}
struct NativeSum {
    tag: u32,
    bits: u64,
    owned: bool,
}
struct NativeBitfieldLayout {
    name: String,
    network_order: bool,
    fields: Vec<NativeBitfieldField>,
}
enum NativeBitfieldField {
    Bits {
        name: String,
        width: u8,
        enum_type: Option<(String, Vec<i64>)>,
    },
    Payload {
        name: String,
    },
}
enum DecodedBitfieldField {
    Plain(u64),
    Enum(u32),
    Payload(Vec<u8>),
}
struct BitfieldLayoutCursor<'a> {
    bytes: &'a [u8],
    position: usize,
}
impl BitfieldLayoutCursor<'_> {
    fn take(&mut self, count: usize) -> LeafResult<&[u8]> {
        let end = self
            .position
            .checked_add(count)
            .ok_or(INVALID_BITFIELD_LAYOUT)?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or(INVALID_BITFIELD_LAYOUT)?;
        self.position = end;
        Ok(value)
    }
    fn byte(&mut self) -> LeafResult<u8> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> LeafResult<u32> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().expect("four bytes"),
        ))
    }
    fn i64(&mut self) -> LeafResult<i64> {
        Ok(i64::from_le_bytes(
            self.take(8)?.try_into().expect("eight bytes"),
        ))
    }
    fn name(&mut self) -> LeafResult<String> {
        let length = usize::try_from(self.u32()?).map_err(|_| INVALID_BITFIELD_LAYOUT)?;
        let bytes = self.take(length)?;
        Ok(std::str::from_utf8(bytes)
            .map_err(|_| INVALID_BITFIELD_LAYOUT)?
            .to_owned())
    }
}
impl NativeBitfieldLayout {
    fn parse(bytes: &[u8]) -> LeafResult<Self> {
        let mut cursor = BitfieldLayoutCursor { bytes, position: 0 };
        if cursor.take(3)? != b"JB\x01" {
            return Err(INVALID_BITFIELD_LAYOUT);
        }
        let network_order = match cursor.byte()? {
            0 => false,
            1 => true,
            _ => return Err(INVALID_BITFIELD_LAYOUT),
        };
        let name = cursor.name()?;
        let count = usize::try_from(cursor.u32()?).map_err(|_| INVALID_BITFIELD_LAYOUT)?;
        if count > bytes.len() {
            return Err(INVALID_BITFIELD_LAYOUT);
        }
        let mut fields = Vec::new();
        fields.try_reserve_exact(count).map_err(|_| EXHAUSTED)?;
        for _ in 0..count {
            let kind = cursor.byte()?;
            let width = cursor.byte()?;
            let field_name = cursor.name()?;
            let field = match kind {
                0 if (1..=64).contains(&width) => NativeBitfieldField::Bits {
                    name: field_name,
                    width,
                    enum_type: None,
                },
                1 if (1..=64).contains(&width) => {
                    let enum_name = cursor.name()?;
                    let variant_count =
                        usize::try_from(cursor.u32()?).map_err(|_| INVALID_BITFIELD_LAYOUT)?;
                    if variant_count > bytes.len() / 8 {
                        return Err(INVALID_BITFIELD_LAYOUT);
                    }
                    let mut discriminants = Vec::new();
                    discriminants
                        .try_reserve_exact(variant_count)
                        .map_err(|_| EXHAUSTED)?;
                    for _ in 0..variant_count {
                        discriminants.push(cursor.i64()?);
                    }
                    NativeBitfieldField::Bits {
                        name: field_name,
                        width,
                        enum_type: Some((enum_name, discriminants)),
                    }
                }
                2 if width == 0 => NativeBitfieldField::Payload { name: field_name },
                _ => return Err(INVALID_BITFIELD_LAYOUT),
            };
            fields.push(field);
        }
        if cursor.position != bytes.len() {
            return Err(INVALID_BITFIELD_LAYOUT);
        }
        Ok(Self {
            name,
            network_order,
            fields,
        })
    }
    fn decode(&self, bytes: &[u8]) -> Result<Vec<DecodedBitfieldField>, String> {
        let mut bit_index = 0_usize;
        let mut fields = Vec::with_capacity(self.fields.len());
        for field in &self.fields {
            match field {
                NativeBitfieldField::Bits {
                    name,
                    width,
                    enum_type,
                } => {
                    let width = *width as usize;
                    let numeric = if width > 8
                        && width % 8 == 0
                        && !self.network_order
                        && bit_index % 8 == 0
                    {
                        let end = bit_index / 8 + width / 8;
                        if end > bytes.len() {
                            return Err(format!(
                                "bitfield '{}.from_bytes' expected at least {} byte(s), got {}",
                                self.name,
                                end,
                                bytes.len()
                            ));
                        }
                        let mut numeric = 0_u64;
                        for (shift, byte) in bytes[bit_index / 8..end].iter().enumerate() {
                            numeric |= u64::from(*byte) << (shift * 8);
                        }
                        bit_index += width;
                        numeric
                    } else {
                        let mut numeric = 0_u64;
                        for _ in 0..width {
                            if bit_index / 8 >= bytes.len() {
                                return Err(format!(
                                    "bitfield '{}.from_bytes' expected {} bit(s), got {} byte(s)",
                                    self.name,
                                    bit_index + width,
                                    bytes.len()
                                ));
                            }
                            let bit = (bytes[bit_index / 8] >> (7 - bit_index % 8)) & 1;
                            numeric = (numeric << 1) | u64::from(bit);
                            bit_index += 1;
                        }
                        numeric
                    };
                    if let Some((enum_name, discriminants)) = enum_type {
                        let variant = discriminants
                            .iter()
                            .position(|candidate| *candidate >= 0 && *candidate as u64 == numeric);
                        let Some(variant) = variant else {
                            return Err(format!(
                                "bitfield '{}' field '{}': enum '{}' has no variant for value {}",
                                self.name, name, enum_name, numeric
                            ));
                        };
                        fields.push(DecodedBitfieldField::Enum(variant as u32));
                    } else {
                        fields.push(DecodedBitfieldField::Plain(numeric));
                    }
                }
                NativeBitfieldField::Payload { name } => {
                    if bit_index % 8 != 0 {
                        return Err(format!(
                            "bitfield '{}' payload field '{}' must begin on a byte boundary",
                            self.name, name
                        ));
                    }
                    fields.push(DecodedBitfieldField::Payload(
                        bytes[bit_index / 8..].to_vec(),
                    ));
                    bit_index = bytes.len() * 8;
                }
            }
        }
        let consumed = bit_index.div_ceil(8);
        if consumed != bytes.len() {
            return Err(format!(
                "bitfield '{}.from_bytes' expected {} byte(s), got {}",
                self.name,
                consumed,
                bytes.len()
            ));
        }
        Ok(fields)
    }
}
#[derive(Default)]
pub(super) struct NativeValues {
    #[cfg(test)]
    allocation_budget: Option<usize>,
    strings: HashMap<NativeHandle, NativeString>,
    bytes: HashMap<NativeHandle, Vec<u8>>,
    sums: HashMap<NativeHandle, NativeSum>,
    lists: HashMap<NativeHandle, NativeList>,
    sets: HashMap<NativeHandle, NativeSet>,
    maps: HashMap<NativeHandle, NativeMap>,
    structs: HashMap<NativeHandle, NativeStruct>,
    actors: std::collections::HashSet<NativeHandle>,
    builders: HashMap<NativeHandle, NativeBuilderInfo>,
    structs_created: u64,
    structs_destroyed: u64,
    lists_created: u64,
    lists_destroyed: u64,
    sets_created: u64,
    sets_destroyed: u64,
    maps_created: u64,
    maps_destroyed: u64,
    sums_created: u64,
    sums_destroyed: u64,
    bytes_created: u64,
    bytes_destroyed: u64,
    failure: Option<Failure>,
    pub(super) cleanup_failed: bool,
    stdout: Option<u64>,
    clock: Option<u64>,
    clock_script: Option<std::collections::VecDeque<clock::ClockTestSample>>,
    random: Option<u64>,
    random_provider: Option<RandomProvider>,
    environment: Option<u64>,
    environment_snapshot: Option<LaunchEnvironmentSnapshot>,
    graphics: Option<u64>,
}
impl NativeValues {
    #[cfg(test)]
    fn allocation_checkpoint(&mut self) -> LeafResult<()> {
        if let Some(budget) = &mut self.allocation_budget {
            *budget = budget.checked_sub(1).ok_or(EXHAUSTED)?;
        }
        Ok(())
    }
    pub(super) fn is_empty(&self) -> bool {
        self.structs.is_empty()
            && self.actors.is_empty()
            && self.builders.is_empty()
            && self.structs_created == self.structs_destroyed
            && self.strings.is_empty()
            && self.bytes.is_empty()
            && self.bytes_created == self.bytes_destroyed
            && self.sums.is_empty()
            && self.sums_created == self.sums_destroyed
            && self.lists.is_empty()
            && self.lists_created == self.lists_destroyed
            && self.sets.is_empty()
            && self.sets_created == self.sets_destroyed
            && self.maps.is_empty()
            && self.maps_created == self.maps_destroyed
    }
    fn insert(&mut self, text: String) -> LeafResult<u64> {
        #[cfg(test)]
        self.allocation_checkpoint()?;
        let id = next_identity()?;
        self.strings.insert(
            id,
            NativeString {
                text,
                references: 1,
            },
        );
        Ok(id)
    }
    fn insert_bytes(&mut self, bytes: Vec<u8>) -> LeafResult<u64> {
        #[cfg(test)]
        self.allocation_checkpoint()?;
        let id = next_identity()?;
        self.bytes.insert(id, bytes);
        self.bytes_created += 1;
        Ok(id)
    }
    fn bytes(&self, id: u64) -> LeafResult<&[u8]> {
        self.bytes.get(&id).map(Vec::as_slice).ok_or(INVALID_BYTES)
    }
    fn write_bitfield_bits(
        &mut self,
        id: u64,
        numeric: u64,
        width: u32,
        network_order: u32,
        bit_offset: u64,
    ) -> LeafResult<u32> {
        if !(1..=64).contains(&width)
            || network_order > 1
            || (width < 64 && numeric >= (1_u64 << width))
        {
            return Err(INVALID_STRUCT);
        }
        let offset = usize::try_from(bit_offset).map_err(|_| EXHAUSTED)?;
        let end = offset.checked_add(width as usize).ok_or(EXHAUSTED)?;
        let byte_count = end.checked_add(7).ok_or(EXHAUSTED)? / 8;
        let bytes = self.bytes.get_mut(&id).ok_or(INVALID_BYTES)?;
        if byte_count > bytes.len() {
            bytes
                .try_reserve_exact(byte_count - bytes.len())
                .map_err(|_| EXHAUSTED)?;
            bytes.resize(byte_count, 0);
        }
        let little_endian_bytes =
            width > 8 && width % 8 == 0 && network_order == 0 && offset % 8 == 0;
        for bit in 0..width as usize {
            let shift = if little_endian_bytes {
                (bit / 8) * 8 + (7 - bit % 8)
            } else {
                width as usize - 1 - bit
            };
            if (numeric >> shift) & 1 != 0 {
                let position = offset + bit;
                bytes[position / 8] |= 1 << (7 - position % 8);
            }
        }
        Ok(0)
    }
    fn extend_bitfield_payload(&mut self, id: u64, payload: u64) -> LeafResult<u32> {
        let source = self.lists.get(&payload).ok_or(INVALID_LIST)?;
        if source.owned
            || source
                .elements
                .iter()
                .any(|element| element.is_none_or(|value| value > u8::MAX as u64))
        {
            return Err(INVALID_LIST);
        }
        let bytes = self.bytes.get_mut(&id).ok_or(INVALID_BYTES)?;
        bytes
            .try_reserve_exact(source.elements.len())
            .map_err(|_| EXHAUSTED)?;
        for element in &source.elements {
            bytes.push(element.expect("validated payload element") as u8);
        }
        Ok(0)
    }
    fn decode_bitfield(&mut self, input: u64, descriptor: &[u8]) -> LeafResult<u64> {
        let layout = NativeBitfieldLayout::parse(descriptor)?;
        let decoded = layout.decode(self.bytes(input)?);
        match decoded {
            Err(message) => {
                let text = self.insert(message)?;
                match self.sum(SUM_FAILURE, text, true) {
                    Ok(result) => Ok(result),
                    Err(error) => {
                        self.drop_value(text)?;
                        Err(error)
                    }
                }
            }
            Ok(fields) => {
                let record = self.new_struct(fields.len() as u64)?;
                let materialize = (|| -> LeafResult<()> {
                    for (index, field) in fields.into_iter().enumerate() {
                        let value = match field {
                            DecodedBitfieldField::Plain(bits) => NativeField { bits, owned: false },
                            DecodedBitfieldField::Enum(variant) => {
                                let nested = self.new_struct(1)?;
                                self.structs.get_mut(&nested).ok_or(INVALID_STRUCT)?.fields[0] =
                                    Some(NativeField {
                                        bits: u64::from(variant),
                                        owned: false,
                                    });
                                NativeField {
                                    bits: nested,
                                    owned: true,
                                }
                            }
                            DecodedBitfieldField::Payload(bytes) => {
                                let mut elements = Vec::new();
                                elements
                                    .try_reserve_exact(bytes.len())
                                    .map_err(|_| EXHAUSTED)?;
                                elements
                                    .extend(bytes.into_iter().map(|byte| Some(u64::from(byte))));
                                let list = self.new_list(false)?;
                                self.lists.get_mut(&list).ok_or(INVALID_LIST)?.elements = elements;
                                NativeField {
                                    bits: list,
                                    owned: true,
                                }
                            }
                        };
                        self.structs.get_mut(&record).ok_or(INVALID_STRUCT)?.fields[index] =
                            Some(value);
                    }
                    Ok(())
                })();
                if let Err(error) = materialize {
                    self.drop_value(record)?;
                    return Err(error);
                }
                match self.sum(SUM_SUCCESS, record, true) {
                    Ok(result) => Ok(result),
                    Err(error) => {
                        self.drop_value(record)?;
                        Err(error)
                    }
                }
            }
        }
    }
    fn sum(&mut self, tag: u32, bits: u64, owned: bool) -> LeafResult<u64> {
        #[cfg(test)]
        self.allocation_checkpoint()?;
        if tag > SUM_SUCCESS {
            return Err(INVALID_SUM);
        }
        let id = next_identity()?;
        self.sums.insert(id, NativeSum { tag, bits, owned });
        self.sums_created += 1;
        Ok(id)
    }
    fn parsed_sum(&mut self, parsed: Result<u64, String>) -> LeafResult<u64> {
        match parsed {
            Ok(bits) => self.sum(SUM_SUCCESS, bits, false),
            Err(error) => {
                let payload = self.insert(error)?;
                match self.sum(SUM_FAILURE, payload, true) {
                    Ok(value) => Ok(value),
                    Err(error) => {
                        self.drop_value(payload)?;
                        Err(error)
                    }
                }
            }
        }
    }
    fn byte_result(&mut self, decoded: Result<Vec<u8>, String>) -> LeafResult<u64> {
        let (tag, payload) = match decoded {
            Ok(bytes) => (SUM_SUCCESS, self.insert_bytes(bytes)?),
            Err(error) => (SUM_FAILURE, self.insert(error)?),
        };
        match self.sum(tag, payload, true) {
            Ok(value) => Ok(value),
            Err(error) => {
                self.drop_value(payload)?;
                Err(error)
            }
        }
    }
    fn string_result(&mut self, decoded: Result<String, String>) -> LeafResult<u64> {
        let (tag, payload) = match decoded {
            Ok(text) => (SUM_SUCCESS, self.insert(text)?),
            Err(error) => (SUM_FAILURE, self.insert(error)?),
        };
        match self.sum(tag, payload, true) {
            Ok(value) => Ok(value),
            Err(error) => {
                self.drop_value(payload)?;
                Err(error)
            }
        }
    }
    fn owned_sum(&mut self, tag: u32, payload: u64) -> LeafResult<u64> {
        match self.sum(tag, payload, true) {
            Ok(value) => Ok(value),
            Err(error) => {
                self.drop_value(payload)?;
                Err(error)
            }
        }
    }
    fn push_owned_list_element(&mut self, id: u64, element: u64) -> LeafResult<()> {
        let result = self
            .lists
            .get_mut(&id)
            .ok_or(INVALID_LIST)
            .and_then(|list| {
                if !list.owned {
                    return Err(INVALID_LIST);
                }
                list.elements.try_reserve(1).map_err(|_| EXHAUSTED)?;
                list.elements.push(Some(element));
                Ok(())
            });
        if result.is_err() {
            self.drop_value(element)?;
        }
        result
    }
    fn csv_rows(&mut self, rows: Vec<Vec<String>>) -> LeafResult<u64> {
        let outer = self.new_list(true)?;
        for row in rows {
            let result = self
                .string_list(row)
                .and_then(|inner| self.push_owned_list_element(outer, inner));
            if let Err(error) = result {
                self.drop_value(outer)?;
                return Err(error);
            }
        }
        Ok(outer)
    }
    fn csv_string_pairs(&mut self, entries: Vec<(String, String)>) -> LeafResult<u64> {
        let map = self.new_map(1, 1)?;
        for (key, value) in entries {
            let pair = (|| {
                let key = self.insert(key)?;
                let value = match self.insert(value) {
                    Ok(value) => value,
                    Err(error) => {
                        self.drop_value(key)?;
                        return Err(error);
                    }
                };
                if let Err(error) = self.map_append_literal(map, key, value) {
                    self.drop_value(key)?;
                    self.drop_value(value)?;
                    return Err(error);
                }
                Ok(())
            })();
            if let Err(error) = pair {
                self.drop_value(map)?;
                return Err(error);
            }
        }
        Ok(map)
    }
    fn csv_header_rows(&mut self, rows: Vec<Vec<(String, String)>>) -> LeafResult<u64> {
        let outer = self.new_list(true)?;
        for entries in rows {
            let result = self
                .csv_string_pairs(entries)
                .and_then(|map| self.push_owned_list_element(outer, map));
            if let Err(error) = result {
                self.drop_value(outer)?;
                return Err(error);
            }
        }
        Ok(outer)
    }
    fn csv_records(&self, rows: u64) -> LeafResult<Vec<Vec<String>>> {
        let outer = self.lists.get(&rows).ok_or(INVALID_LIST)?;
        if !outer.owned {
            return Err(INVALID_LIST);
        }
        let mut records = Vec::new();
        records
            .try_reserve_exact(outer.elements.len())
            .map_err(|_| EXHAUSTED)?;
        for row in &outer.elements {
            let inner = self
                .lists
                .get(&row.ok_or(INVALID_LIST)?)
                .ok_or(INVALID_LIST)?;
            if !inner.owned {
                return Err(INVALID_LIST);
            }
            let mut fields = Vec::new();
            fields
                .try_reserve_exact(inner.elements.len())
                .map_err(|_| EXHAUSTED)?;
            for field in &inner.elements {
                fields.push(self.text(field.ok_or(INVALID_LIST)?)?.to_owned());
            }
            records.push(fields);
        }
        Ok(records)
    }
    fn new_list(&mut self, owned: bool) -> LeafResult<u64> {
        #[cfg(test)]
        self.allocation_checkpoint()?;
        let id = next_identity()?;
        self.lists.insert(
            id,
            NativeList {
                elements: Vec::new(),
                owned,
            },
        );
        self.lists_created += 1;
        Ok(id)
    }
    fn new_set(&mut self, strings: u32) -> LeafResult<u64> {
        if strings > 1 {
            return Err(INVALID_SET);
        }
        #[cfg(test)]
        self.allocation_checkpoint()?;
        let id = next_identity()?;
        self.sets.insert(
            id,
            NativeSet {
                elements: Vec::new(),
                strings: strings != 0,
            },
        );
        self.sets_created += 1;
        Ok(id)
    }
    fn new_map(&mut self, key_strings: u32, value_owned: u32) -> LeafResult<u64> {
        if key_strings > 1 || value_owned > 1 {
            return Err(INVALID_MAP);
        }
        #[cfg(test)]
        self.allocation_checkpoint()?;
        let id = next_identity()?;
        self.maps.insert(
            id,
            NativeMap {
                entries: Vec::new(),
                key_strings: key_strings != 0,
                value_owned: value_owned != 0,
            },
        );
        self.maps_created += 1;
        Ok(id)
    }
    fn map_position(&self, id: u64, key: u64) -> LeafResult<Option<usize>> {
        let map = self.maps.get(&id).ok_or(INVALID_MAP)?;
        if map.key_strings {
            let key_text = self.text(key)?;
            for (index, entry) in map.entries.iter().enumerate() {
                if let Some(entry) = entry {
                    if !entry.key_taken && self.text(entry.key)? == key_text {
                        return Ok(Some(index));
                    }
                }
            }
            Ok(None)
        } else {
            Ok(map
                .entries
                .iter()
                .position(|entry| entry.is_some_and(|entry| !entry.key_taken && entry.key == key)))
        }
    }
    fn map_insert(&mut self, id: u64, key: u64, value: u64) -> LeafResult<u64> {
        let position = self.map_position(id, key)?;
        let map = self.maps.get_mut(&id).ok_or(INVALID_MAP)?;
        let key_strings = map.key_strings;
        let value_owned = map.value_owned;
        if position.is_none() {
            map.entries.try_reserve(1).map_err(|_| EXHAUSTED)?;
            map.entries.push(Some(NativeMapEntry {
                key,
                value,
                key_taken: false,
            }));
        } else {
            let old = map.entries[position.ok_or(INVALID_MAP)?]
                .as_mut()
                .ok_or(INVALID_MAP)?;
            let old_value = std::mem::replace(&mut old.value, value);
            if key_strings {
                self.drop_value(key)?;
            }
            if value_owned {
                self.drop_value(old_value)?;
            }
        }
        Ok(id)
    }
    fn map_append_literal(&mut self, id: u64, key: u64, value: u64) -> LeafResult<u64> {
        let map = self.maps.get(&id).ok_or(INVALID_MAP)?;
        if map.key_strings {
            self.text(key)?;
        }
        let map = self.maps.get_mut(&id).ok_or(INVALID_MAP)?;
        map.entries.try_reserve(1).map_err(|_| EXHAUSTED)?;
        map.entries.push(Some(NativeMapEntry {
            key,
            value,
            key_taken: false,
        }));
        Ok(id)
    }
    fn map_remove(&mut self, id: u64, key: u64) -> LeafResult<u64> {
        while let Some(index) = self.map_position(id, key)? {
            let map = self.maps.get_mut(&id).ok_or(INVALID_MAP)?;
            let key_strings = map.key_strings;
            let value_owned = map.value_owned;
            let entry = map.entries.remove(index).ok_or(INVALID_MAP)?;
            if key_strings {
                self.drop_value(entry.key)?;
            }
            if value_owned {
                self.drop_value(entry.value)?;
            }
        }
        Ok(id)
    }
    fn map_get(&mut self, id: u64, key: u64) -> LeafResult<u64> {
        let Some(index) = self.map_position(id, key)? else {
            return self.sum(SUM_FAILURE, 0, false);
        };
        let map = self.maps.get(&id).ok_or(INVALID_MAP)?;
        let value_owned = map.value_owned;
        let bits = map.entries[index].ok_or(INVALID_MAP)?.value;
        let bits = if value_owned {
            self.clone_value(bits)?
        } else {
            bits
        };
        match self.sum(SUM_SUCCESS, bits, value_owned) {
            Ok(result) => Ok(result),
            Err(error) => {
                if value_owned {
                    self.drop_value(bits)?;
                }
                Err(error)
            }
        }
    }
    fn map_element(&mut self, id: u64, index: i64, key: bool, consume: bool) -> LeafResult<u64> {
        let map = self.maps.get(&id).ok_or(INVALID_MAP)?;
        let index = usize::try_from(index).map_err(|_| INVALID_MAP)?;
        let entry = map
            .entries
            .get(index)
            .copied()
            .flatten()
            .ok_or(INVALID_MAP)?;
        if entry.key_taken != (!key && consume) {
            return Err(INVALID_MAP);
        }
        let (bits, owned) = if key {
            (entry.key, map.key_strings)
        } else {
            (entry.value, map.value_owned)
        };
        if !consume {
            return if owned {
                self.clone_value(bits)
            } else {
                Ok(bits)
            };
        }
        let map = self.maps.get_mut(&id).ok_or(INVALID_MAP)?;
        let slot = map.entries.get_mut(index).ok_or(INVALID_MAP)?;
        if key {
            slot.as_mut().ok_or(INVALID_MAP)?.key_taken = true;
        } else {
            *slot = None;
        }
        Ok(bits)
    }
    fn clone_map(&mut self, id: u64) -> LeafResult<u64> {
        let map = self.maps.get(&id).ok_or(INVALID_MAP)?;
        let (entries, key_strings, value_owned) =
            (map.entries.clone(), map.key_strings, map.value_owned);
        let output = self.new_map(u32::from(key_strings), u32::from(value_owned))?;
        if let Err(error) = self
            .maps
            .get_mut(&output)
            .ok_or(INVALID_MAP)?
            .entries
            .try_reserve_exact(entries.len())
            .map_err(|_| EXHAUSTED)
        {
            self.drop_value(output)?;
            return Err(error);
        }
        for entry in entries {
            let Some(entry) = entry else {
                self.maps
                    .get_mut(&output)
                    .ok_or(INVALID_MAP)?
                    .entries
                    .push(None);
                continue;
            };
            if entry.key_taken {
                self.drop_value(output)?;
                return Err(INVALID_MAP);
            }
            let key = if key_strings {
                match self.clone_value(entry.key) {
                    Ok(key) => key,
                    Err(error) => {
                        self.drop_value(output)?;
                        return Err(error);
                    }
                }
            } else {
                entry.key
            };
            let value = if value_owned {
                match self.clone_value(entry.value) {
                    Ok(value) => value,
                    Err(error) => {
                        if key_strings {
                            self.drop_value(key)?;
                        }
                        self.drop_value(output)?;
                        return Err(error);
                    }
                }
            } else {
                entry.value
            };
            self.maps
                .get_mut(&output)
                .ok_or(INVALID_MAP)?
                .entries
                .push(Some(NativeMapEntry {
                    key,
                    value,
                    key_taken: false,
                }));
        }
        Ok(output)
    }
    fn map_from_lists(
        &mut self,
        keys: u64,
        values: u64,
        key_strings: u32,
        value_owned: u32,
    ) -> LeafResult<u64> {
        if key_strings > 1 || value_owned > 1 {
            return Err(INVALID_MAP);
        }
        let key_list = self.lists.get(&keys).ok_or(INVALID_LIST)?;
        let value_list = self.lists.get(&values).ok_or(INVALID_LIST)?;
        if key_list.owned != (key_strings != 0) || value_list.owned != (value_owned != 0) {
            return Err(INVALID_MAP);
        }
        let (key_elements, value_elements) =
            (key_list.elements.clone(), value_list.elements.clone());
        let output = self.new_map(key_strings, value_owned)?;
        for (key, value) in key_elements.into_iter().zip(value_elements) {
            let Some(key) = key else {
                self.drop_value(output)?;
                return Err(INVALID_LIST);
            };
            let Some(value) = value else {
                self.drop_value(output)?;
                return Err(INVALID_LIST);
            };
            let key = if key_strings != 0 {
                match self.clone_value(key) {
                    Ok(key) => key,
                    Err(error) => {
                        self.drop_value(output)?;
                        return Err(error);
                    }
                }
            } else {
                key
            };
            let value = if value_owned != 0 {
                match self.clone_value(value) {
                    Ok(value) => value,
                    Err(error) => {
                        if key_strings != 0 {
                            self.drop_value(key)?;
                        }
                        self.drop_value(output)?;
                        return Err(error);
                    }
                }
            } else {
                value
            };
            if let Err(error) = self.map_insert(output, key, value) {
                if key_strings != 0 {
                    self.drop_value(key)?;
                }
                if value_owned != 0 {
                    self.drop_value(value)?;
                }
                self.drop_value(output)?;
                return Err(error);
            }
        }
        self.drop_value(keys)?;
        self.drop_value(values)?;
        Ok(output)
    }
    fn set_position(&self, id: u64, key: u64) -> LeafResult<Option<usize>> {
        let set = self.sets.get(&id).ok_or(INVALID_SET)?;
        if set.strings {
            let key_text = self.text(key)?;
            for (index, &element) in set.elements.iter().enumerate() {
                if let Some(element) = element {
                    if self.text(element)? == key_text {
                        return Ok(Some(index));
                    }
                }
            }
            Ok(None)
        } else {
            Ok(set
                .elements
                .iter()
                .position(|element| *element == Some(key)))
        }
    }
    fn set_add(&mut self, id: u64, key: u64) -> LeafResult<u64> {
        let duplicate = self.set_position(id, key)?.is_some();
        if duplicate {
            if self.sets.get(&id).ok_or(INVALID_SET)?.strings {
                self.drop_value(key)?;
            }
            return Ok(id);
        }
        let set = self.sets.get_mut(&id).ok_or(INVALID_SET)?;
        set.elements.try_reserve(1).map_err(|_| EXHAUSTED)?;
        set.elements.push(Some(key));
        Ok(id)
    }
    fn set_remove(&mut self, id: u64, key: u64) -> LeafResult<u64> {
        if let Some(index) = self.set_position(id, key)? {
            let set = self.sets.get_mut(&id).ok_or(INVALID_SET)?;
            let old = set.elements.remove(index);
            if set.strings {
                self.drop_value(old.ok_or(INVALID_SET)?)?;
            }
        }
        Ok(id)
    }
    fn clone_set(&mut self, id: u64) -> LeafResult<u64> {
        let set = self.sets.get(&id).ok_or(INVALID_SET)?;
        let (elements, strings) = (set.elements.clone(), set.strings);
        let output = self.new_set(u32::from(strings))?;
        if let Err(error) = self
            .sets
            .get_mut(&output)
            .ok_or(INVALID_SET)?
            .elements
            .try_reserve_exact(elements.len())
            .map_err(|_| EXHAUSTED)
        {
            self.drop_value(output)?;
            return Err(error);
        }
        for element in elements {
            let value = if strings {
                match element.map(|element| self.clone_value(element)).transpose() {
                    Ok(value) => value,
                    Err(error) => {
                        self.drop_value(output)?;
                        return Err(error);
                    }
                }
            } else {
                element
            };
            self.sets
                .get_mut(&output)
                .ok_or(INVALID_SET)?
                .elements
                .push(value);
        }
        Ok(output)
    }
    fn sort_list(&mut self, id: u64, raw_kind: u32) -> LeafResult<u64> {
        let kind = NativeSortKind::from_raw(raw_kind)?;
        let list = self.lists.get_mut(&id).ok_or(INVALID_LIST)?;
        if list.owned != (kind == NativeSortKind::String)
            || list.elements.iter().any(|element| {
                element.is_none_or(|bits| {
                    !kind.valid_bits(bits)
                        || (kind == NativeSortKind::String && !self.strings.contains_key(&bits))
                })
            })
        {
            return Err(INVALID_LIST);
        }
        if kind == NativeSortKind::String {
            list.elements.sort_by(|left, right| {
                let left = &self.strings[&left.expect("validated list element")].text;
                let right = &self.strings[&right.expect("validated list element")].text;
                left.cmp(right)
            });
        } else {
            list.elements.sort_by(|left, right| {
                kind.compare(
                    left.expect("validated list element"),
                    right.expect("validated list element"),
                )
            });
        }
        Ok(id)
    }

    fn list_is_sorted(&self, id: u64, raw_kind: u32) -> LeafResult<bool> {
        let kind = NativeSortKind::from_comparison_raw(raw_kind)?;
        let list = self.lists.get(&id).ok_or(INVALID_LIST)?;
        let Some(kind) = kind else {
            return Ok(true);
        };
        if list.owned != (kind == NativeSortKind::String) {
            return Err(INVALID_LIST);
        }
        let mut previous = None;
        for element in &list.elements {
            let bits = element.ok_or(INVALID_LIST)?;
            if !kind.valid_bits(bits)
                || (kind == NativeSortKind::String && !self.strings.contains_key(&bits))
            {
                return Err(INVALID_LIST);
            }
            if let Some(left) = previous {
                let ordering = if kind == NativeSortKind::String {
                    self.strings[&left].text.cmp(&self.strings[&bits].text)
                } else {
                    kind.compare(left, bits)
                };
                if ordering == CompareOrdering::Greater {
                    return Ok(false);
                }
            }
            previous = Some(bits);
        }
        Ok(true)
    }

    fn sort_list_by_index(&mut self, id: u64, index: i64, raw_kind: u32) -> LeafResult<u64> {
        let kind = NativeSortKind::from_comparison_raw(raw_kind)?;
        let outer = self.lists.get(&id).ok_or(INVALID_LIST)?;
        if !outer.owned {
            return Err(INVALID_LIST);
        }
        let mut keyed = Vec::new();
        keyed
            .try_reserve_exact(outer.elements.len())
            .map_err(|_| EXHAUSTED)?;
        let index = usize::try_from(index).ok();
        for element in &outer.elements {
            let row_id = element.ok_or(INVALID_LIST)?;
            let row = self.lists.get(&row_id).ok_or(INVALID_LIST)?;
            if let Some(kind) = kind
                && row.owned != (kind == NativeSortKind::String)
            {
                return Err(INVALID_LIST);
            }
            let key = index.and_then(|index| row.elements.get(index)).copied();
            let key = match key {
                Some(Some(bits)) => {
                    if let Some(kind) = kind
                        && (!kind.valid_bits(bits)
                            || (kind == NativeSortKind::String
                                && !self.strings.contains_key(&bits)))
                    {
                        return Err(INVALID_LIST);
                    }
                    Some(bits)
                }
                Some(None) => return Err(INVALID_LIST),
                None => None,
            };
            keyed.push((row_id, key));
        }
        if let Some(kind) = kind {
            keyed.sort_by(|(_, left), (_, right)| match (left, right) {
                (Some(left), Some(right)) if kind == NativeSortKind::String => {
                    self.strings[left].text.cmp(&self.strings[right].text)
                }
                (Some(left), Some(right)) => kind.compare(*left, *right),
                _ => CompareOrdering::Equal,
            });
        }
        let outer = self.lists.get_mut(&id).ok_or(INVALID_LIST)?;
        for (slot, (row_id, _)) in outer.elements.iter_mut().zip(keyed) {
            *slot = Some(row_id);
        }
        Ok(id)
    }

    fn math_numbers(&self, id: u64, raw_kind: u32, empty: Failure) -> LeafResult<Vec<f64>> {
        let kind = NativeSortKind::from_raw(raw_kind)?;
        if !matches!(
            kind,
            NativeSortKind::Int64 | NativeSortKind::Uint64 | NativeSortKind::Float64
        ) {
            return Err(INVALID_LIST);
        }
        let list = self.lists.get(&id).ok_or(INVALID_LIST)?;
        if list.owned {
            return Err(INVALID_LIST);
        }
        if list.elements.is_empty() {
            return Err(empty);
        }
        list.elements
            .iter()
            .map(|value| {
                let bits = (*value).ok_or(INVALID_LIST)?;
                Ok(match kind {
                    NativeSortKind::Int64 => (bits as i64) as f64,
                    NativeSortKind::Uint64 => bits as f64,
                    NativeSortKind::Float64 => f64::from_bits(bits),
                    _ => unreachable!("numeric kind checked above"),
                })
            })
            .collect()
    }
    fn string_list(&mut self, parts: Vec<String>) -> LeafResult<u64> {
        let mut elements = Vec::new();
        elements
            .try_reserve_exact(parts.len())
            .map_err(|_| EXHAUSTED)?;
        let id = self.new_list(true)?;
        for part in parts {
            match self.insert(part) {
                Ok(value) => elements.push(Some(value)),
                Err(error) => {
                    self.lists.get_mut(&id).ok_or(INVALID_LIST)?.elements = elements;
                    self.drop_value(id)?;
                    return Err(error);
                }
            }
        }
        self.lists.get_mut(&id).ok_or(INVALID_LIST)?.elements = elements;
        Ok(id)
    }
    fn join_strings(&mut self, value: u64, separator: u64) -> LeafResult<u64> {
        let list = self.lists.get(&value).ok_or(INVALID_LIST)?;
        if !list.owned {
            return Err(INVALID_LIST);
        }
        let separator = self.text(separator)?;
        let mut parts = Vec::new();
        parts
            .try_reserve_exact(list.elements.len())
            .map_err(|_| EXHAUSTED)?;
        let mut length = separator
            .len()
            .checked_mul(list.elements.len().saturating_sub(1))
            .ok_or(EXHAUSTED)?;
        for element in &list.elements {
            let text = self.text(element.ok_or(INVALID_LIST)?)?;
            length = length.checked_add(text.len()).ok_or(EXHAUSTED)?;
            parts.push(text);
        }
        let mut output = String::new();
        output.try_reserve_exact(length).map_err(|_| EXHAUSTED)?;
        for (index, part) in parts.into_iter().enumerate() {
            if index != 0 {
                output.push_str(separator);
            }
            output.push_str(part);
        }
        self.insert(output)
    }
    fn range(&mut self, start: i64, end: i64, step: i64) -> LeafResult<u64> {
        if step == 0 {
            return Err((
                JettRuntimeStatusV1::INVALID_ARGUMENT,
                b"range step cannot be zero",
            ));
        }
        let too_large = (
            JettRuntimeStatusV1::RESOURCE_EXHAUSTED,
            b"range: requested output is too large" as &'static [u8],
        );
        let distance = if step > 0 && start < end {
            (i128::from(end) - i128::from(start)) as u128
        } else if step < 0 && start > end {
            (i128::from(start) - i128::from(end)) as u128
        } else {
            0
        };
        let count = distance.div_ceil(i128::from(step).unsigned_abs());
        let capacity = usize::try_from(count).map_err(|_| too_large)?;
        let mut elements = Vec::new();
        elements
            .try_reserve_exact(capacity)
            .map_err(|_| too_large)?;
        let mut value = start;
        while (step > 0 && value < end) || (step < 0 && value > end) {
            elements.push(Some(value as u64));
            let Some(next) = value.checked_add(step) else {
                break;
            };
            value = next;
        }
        let id = self.new_list(false)?;
        self.lists.get_mut(&id).ok_or(INVALID_LIST)?.elements = elements;
        Ok(id)
    }
    fn clone_list(&mut self, id: u64) -> LeafResult<u64> {
        let list = self.lists.get(&id).ok_or(INVALID_LIST)?;
        let (elements, owned) = (list.elements.clone(), list.owned);
        let mut output = Vec::new();
        output
            .try_reserve_exact(elements.len())
            .map_err(|_| EXHAUSTED)?;
        for bits in elements {
            let Some(bits) = bits else {
                output.push(None);
                continue;
            };
            if owned {
                match self.clone_value(bits) {
                    Ok(value) => output.push(Some(value)),
                    Err(error) => {
                        for v in output.into_iter().flatten() {
                            self.drop_value(v)?;
                        }
                        return Err(error);
                    }
                }
            } else {
                output.push(Some(bits));
            }
        }
        let id = match self.new_list(owned) {
            Ok(id) => id,
            Err(error) => {
                if owned {
                    for v in output.into_iter().flatten() {
                        self.drop_value(v)?;
                    }
                }
                return Err(error);
            }
        };
        self.lists.get_mut(&id).ok_or(INVALID_LIST)?.elements = output;
        Ok(id)
    }
    fn new_struct(&mut self, count: u64) -> LeafResult<u64> {
        #[cfg(test)]
        self.allocation_checkpoint()?;
        let count = usize::try_from(count).map_err(|_| EXHAUSTED)?;
        let mut fields = Vec::new();
        fields.try_reserve_exact(count).map_err(|_| EXHAUSTED)?;
        fields.resize(count, None);
        let id = next_identity()?;
        self.structs.insert(id, NativeStruct { fields });
        self.structs_created += 1;
        Ok(id)
    }
    fn register_actor(&mut self, id: u64) -> LeafResult<u64> {
        let Some(record) = self.structs.get(&id) else {
            return Err(INVALID_ACTOR);
        };
        if record.fields.iter().any(Option::is_none) || !self.actors.insert(id) {
            return Err(INVALID_ACTOR);
        }
        Ok(id)
    }
    fn replace_actor_field(
        &mut self,
        id: u64,
        index: u64,
        bits: u64,
        owned: bool,
    ) -> LeafResult<u32> {
        if !self.actors.contains(&id) {
            return Err(INVALID_ACTOR);
        }
        let slot = self
            .structs
            .get_mut(&id)
            .and_then(|record| {
                usize::try_from(index)
                    .ok()
                    .and_then(|i| record.fields.get_mut(i))
            })
            .ok_or(INVALID_ACTOR)?;
        let old = slot
            .replace(NativeField { bits, owned })
            .ok_or(INVALID_ACTOR)?;
        if old.owned {
            self.drop_value(old.bits)?;
        }
        Ok(0)
    }
    pub(super) fn release_actors(&mut self) -> LeafResult<()> {
        for id in std::mem::take(&mut self.actors) {
            self.drop_value(id)?;
        }
        Ok(())
    }
    fn new_builder(&mut self, layout: &[u8]) -> LeafResult<u64> {
        let mut cursor = BitfieldLayoutCursor {
            bytes: layout,
            position: 0,
        };
        let version: [u8; 3] = cursor
            .take(3)
            .map_err(|_| INVALID_CONSTRUCTION_LAYOUT)?
            .try_into()
            .expect("three bytes");
        if &version == b"JC\x05" {
            let owner = cursor.name().map_err(|_| INVALID_CONSTRUCTION_LAYOUT)?;
            let kind = cursor.name().map_err(|_| INVALID_CONSTRUCTION_LAYOUT)?;
            if cursor.position != layout.len() {
                return Err(INVALID_CONSTRUCTION_LAYOUT);
            }
            let id = self.new_struct(0)?;
            self.builders.insert(
                id,
                NativeBuilderInfo {
                    metadata_owner: owner.clone(),
                    owner,
                    unsupported_kind: Some(kind),
                    variant: None,
                    state: None,
                    target_state: None,
                    field_offset: 0,
                    field_names: Vec::new(),
                    field_type_names: Vec::new(),
                    field_types: Vec::new(),
                    validation: Vec::new(),
                },
            );
            return Ok(id);
        }
        if !matches!(&version, b"JC\x02" | b"JC\x03") {
            return Err(INVALID_CONSTRUCTION_LAYOUT);
        }
        let owner = cursor.name().map_err(|_| INVALID_CONSTRUCTION_LAYOUT)?;
        let count = usize::try_from(cursor.u32().map_err(|_| INVALID_CONSTRUCTION_LAYOUT)?)
            .map_err(|_| INVALID_CONSTRUCTION_LAYOUT)?;
        if count > layout.len() {
            return Err(INVALID_CONSTRUCTION_LAYOUT);
        }
        let mut field_names = Vec::new();
        let mut field_type_names = Vec::new();
        let mut field_types = Vec::new();
        field_names
            .try_reserve_exact(count)
            .map_err(|_| EXHAUSTED)?;
        field_types
            .try_reserve_exact(count)
            .map_err(|_| EXHAUSTED)?;
        field_type_names
            .try_reserve_exact(count)
            .map_err(|_| EXHAUSTED)?;
        for _ in 0..count {
            field_names.push(cursor.name().map_err(|_| INVALID_CONSTRUCTION_LAYOUT)?);
            field_type_names.push(cursor.name().map_err(|_| INVALID_CONSTRUCTION_LAYOUT)?);
            field_types.push(cursor.name().map_err(|_| INVALID_CONSTRUCTION_LAYOUT)?);
        }
        let mut validation = Vec::new();
        validation.try_reserve_exact(count).map_err(|_| EXHAUSTED)?;
        if &version == b"JC\x03" {
            for _ in 0..count {
                let kind = cursor.byte().map_err(|_| INVALID_CONSTRUCTION_LAYOUT)?;
                let rule = match kind {
                    0 => BuilderFieldValidation::None,
                    1 | 2 | 3 => {
                        let width = cursor.u32().map_err(|_| INVALID_CONSTRUCTION_LAYOUT)?;
                        if width > 64 {
                            return Err(INVALID_CONSTRUCTION_LAYOUT);
                        }
                        match kind {
                            1 => BuilderFieldValidation::SignedBits(width),
                            2 => BuilderFieldValidation::UnsignedBits(width),
                            _ => {
                                let name =
                                    cursor.name().map_err(|_| INVALID_CONSTRUCTION_LAYOUT)?;
                                let variants = usize::try_from(
                                    cursor.u32().map_err(|_| INVALID_CONSTRUCTION_LAYOUT)?,
                                )
                                .map_err(|_| INVALID_CONSTRUCTION_LAYOUT)?;
                                if variants > layout.len() {
                                    return Err(INVALID_CONSTRUCTION_LAYOUT);
                                }
                                let mut values = Vec::new();
                                values.try_reserve_exact(variants).map_err(|_| EXHAUSTED)?;
                                for _ in 0..variants {
                                    values.push((
                                        cursor.name().map_err(|_| INVALID_CONSTRUCTION_LAYOUT)?,
                                        cursor.i64().map_err(|_| INVALID_CONSTRUCTION_LAYOUT)?,
                                    ));
                                }
                                BuilderFieldValidation::EnumBits(width, name, values)
                            }
                        }
                    }
                    _ => return Err(INVALID_CONSTRUCTION_LAYOUT),
                };
                validation.push(rule);
            }
        } else {
            validation.resize(count, BuilderFieldValidation::None);
        }
        if cursor.position != layout.len() {
            return Err(INVALID_CONSTRUCTION_LAYOUT);
        }
        let id = self.new_struct(count as u64)?;
        self.builders.insert(
            id,
            NativeBuilderInfo {
                metadata_owner: owner.clone(),
                owner,
                unsupported_kind: None,
                variant: None,
                state: None,
                target_state: None,
                field_offset: 0,
                field_names,
                field_type_names,
                field_types,
                validation,
            },
        );
        Ok(id)
    }
    fn builder_start_failure(&mut self, message: String) -> LeafResult<u64> {
        let error = self.insert(message)?;
        match self.sum(SUM_FAILURE, error, true) {
            Ok(result) => Ok(result),
            Err(failure) => {
                self.drop_value(error)?;
                Err(failure)
            }
        }
    }
    fn new_member_builder(
        &mut self,
        layout: &[u8],
        metadata: u64,
        machine: bool,
    ) -> LeafResult<u64> {
        let mut cursor = BitfieldLayoutCursor {
            bytes: layout,
            position: 0,
        };
        let version = cursor.take(3).map_err(|_| INVALID_CONSTRUCTION_LAYOUT)?;
        if version != if machine { b"JC\x05" } else { b"JC\x04" } {
            return Err(INVALID_CONSTRUCTION_LAYOUT);
        }
        let owner = cursor.name().map_err(|_| INVALID_CONSTRUCTION_LAYOUT)?;
        let expected_metadata_owner = if machine {
            cursor.name().map_err(|_| INVALID_CONSTRUCTION_LAYOUT)?
        } else {
            owner.clone()
        };
        let target_state = if machine {
            let target = cursor.name().map_err(|_| INVALID_CONSTRUCTION_LAYOUT)?;
            (!target.is_empty()).then_some(target)
        } else {
            None
        };
        let count = usize::try_from(cursor.u32().map_err(|_| INVALID_CONSTRUCTION_LAYOUT)?)
            .map_err(|_| INVALID_CONSTRUCTION_LAYOUT)?;
        if count > layout.len() {
            return Err(INVALID_CONSTRUCTION_LAYOUT);
        }
        let index = usize::try_from(self.struct_field(metadata, 0)?.bits)
            .map_err(|_| INVALID_CONSTRUCTION)?;
        let metadata_owner = self.text(self.struct_field(metadata, 1)?.bits)?.to_owned();
        let metadata_name = self.text(self.struct_field(metadata, 2)?.bits)?.to_owned();
        let metadata_discriminant = if machine {
            0
        } else {
            self.struct_field(metadata, 3)?.bits as i64
        };
        let field_list = self
            .struct_field(metadata, if machine { 4 } else { 5 })?
            .bits;
        if metadata_owner != expected_metadata_owner {
            return self.builder_start_failure(if machine {
                format!("type.construct_machine_start: state metadata belongs to machine '{metadata_owner}', expected '{expected_metadata_owner}'")
            } else {
                format!("type.construct_variant_start: variant metadata belongs to '{metadata_owner}', expected '{expected_metadata_owner}'")
            });
        }
        let mut selected = None;
        for variant_index in 0..count {
            let name = cursor.name().map_err(|_| INVALID_CONSTRUCTION_LAYOUT)?;
            let discriminant = if machine {
                0
            } else {
                cursor.i64().map_err(|_| INVALID_CONSTRUCTION_LAYOUT)?
            };
            let field_count =
                usize::try_from(cursor.u32().map_err(|_| INVALID_CONSTRUCTION_LAYOUT)?)
                    .map_err(|_| INVALID_CONSTRUCTION_LAYOUT)?;
            if field_count > layout.len() {
                return Err(INVALID_CONSTRUCTION_LAYOUT);
            }
            let mut names = Vec::new();
            let mut type_names = Vec::new();
            let mut types = Vec::new();
            for _ in 0..field_count {
                let field_name = cursor.name().map_err(|_| INVALID_CONSTRUCTION_LAYOUT)?;
                let type_name = cursor.name().map_err(|_| INVALID_CONSTRUCTION_LAYOUT)?;
                let canonical_type = cursor.name().map_err(|_| INVALID_CONSTRUCTION_LAYOUT)?;
                if variant_index == index {
                    names.push(field_name);
                    type_names.push(type_name);
                    types.push(canonical_type);
                }
            }
            if variant_index == index {
                selected = Some((name, discriminant, names, type_names, types));
            }
        }
        if cursor.position != layout.len() {
            return Err(INVALID_CONSTRUCTION_LAYOUT);
        }
        let Some((name, discriminant, field_names, field_type_names, field_types)) = selected
        else {
            return self.builder_start_failure(if machine {
                format!("type.construct_machine_start: machine '{expected_metadata_owner}' has no state at index {index}")
            } else {
                format!("type.construct_variant_start: enum '{owner}' has no variant at index {index}")
            });
        };
        if name != metadata_name {
            return self.builder_start_failure(if machine {
                format!("type.construct_machine_start: state metadata '{metadata_name}' does not match state '{name}' on '{expected_metadata_owner}'")
            } else {
                format!("type.construct_variant_start: variant metadata '{metadata_name}' does not match variant '{name}' on '{owner}'")
            });
        }
        if !machine && discriminant != metadata_discriminant {
            return self.builder_start_failure(format!(
                "type.construct_variant_start: variant '{owner}.{name}' has discriminant {discriminant}, metadata reports {metadata_discriminant}"
            ));
        }
        let metadata_fields = &self
            .lists
            .get(&field_list)
            .ok_or(INVALID_CONSTRUCTION)?
            .elements;
        if metadata_fields.len() != field_names.len() {
            return self.builder_start_failure(if machine {
                format!("type.construct_machine_start: state '{expected_metadata_owner}.{name}' expects {} payload field(s), metadata reports {}", field_names.len(), metadata_fields.len())
            } else {
                format!("type.construct_variant_start: variant '{owner}.{name}' expects {} payload field(s), metadata reports {}", field_names.len(), metadata_fields.len())
            });
        }
        for (position, field) in metadata_fields.iter().enumerate() {
            let (field_index, field_owner, member, field_name, type_name) =
                self.builder_field_metadata(field.ok_or(INVALID_CONSTRUCTION)?)?;
            if field_owner != expected_metadata_owner || member.as_deref() != Some(name.as_str()) {
                let actual = member.map_or(field_owner.clone(), |member| {
                    format!("{field_owner}.{member}")
                });
                return self.builder_start_failure(if machine {
                    format!("type.construct_machine_start: field metadata belongs to '{actual}', expected '{expected_metadata_owner}.{name}'")
                } else {
                    format!("type.construct_variant_start: field metadata belongs to '{actual}', expected '{expected_metadata_owner}.{name}'")
                });
            }
            if field_index != position
                || field_name != field_names[position]
                || type_name != field_type_names[position]
            {
                return self.builder_start_failure(if machine {
                    format!("type.construct_machine_start: payload field metadata at index {position} does not match state '{expected_metadata_owner}.{name}'")
                } else {
                    format!("type.construct_variant_start: payload field metadata at index {position} does not match variant '{owner}.{name}'")
                });
            }
        }
        let builder = self.new_struct((field_names.len() + 1) as u64)?;
        self.structs
            .get_mut(&builder)
            .ok_or(INVALID_CONSTRUCTION)?
            .fields[0] = Some(NativeField {
            bits: index as u64,
            owned: false,
        });
        self.builders.insert(
            builder,
            NativeBuilderInfo {
                owner,
                metadata_owner: expected_metadata_owner,
                unsupported_kind: None,
                variant: (!machine).then(|| name.clone()),
                state: machine.then_some(name),
                target_state,
                field_offset: 1,
                validation: vec![BuilderFieldValidation::None; field_names.len()],
                field_names,
                field_type_names,
                field_types,
            },
        );
        match self.sum(SUM_SUCCESS, builder, true) {
            Ok(result) => Ok(result),
            Err(failure) => {
                self.drop_value(builder)?;
                Err(failure)
            }
        }
    }
    fn builder_field_metadata(
        &self,
        field: u64,
    ) -> LeafResult<(usize, String, Option<String>, String, String)> {
        let index =
            usize::try_from(self.struct_field(field, 0)?.bits).map_err(|_| INVALID_CONSTRUCTION)?;
        let owner = self.text(self.struct_field(field, 1)?.bits)?.to_owned();
        let member = self
            .sums
            .get(&self.struct_field(field, 2)?.bits)
            .ok_or(INVALID_CONSTRUCTION)?;
        let member = if member.tag == SUM_SUCCESS {
            Some(self.text(member.bits)?.to_owned())
        } else if member.tag == SUM_FAILURE {
            None
        } else {
            return Err(INVALID_CONSTRUCTION);
        };
        let name = self.text(self.struct_field(field, 3)?.bits)?.to_owned();
        let field_type = self.text(self.struct_field(field, 4)?.bits)?.to_owned();
        Ok((index, owner, member, name, field_type))
    }
    fn builder_failure(
        &mut self,
        message: String,
        builder: u64,
        bits: u64,
        owned: bool,
    ) -> LeafResult<u64> {
        let error = self.insert(message)?;
        let result = match self.sum(SUM_FAILURE, error, true) {
            Ok(result) => result,
            Err(failure) => {
                self.drop_value(error)?;
                return Err(failure);
            }
        };
        self.drop_value(builder)?;
        if owned {
            self.drop_value(bits)?;
        }
        Ok(result)
    }
    fn builder_put(
        &mut self,
        builder: u64,
        field: u64,
        bits: u64,
        owned: bool,
        expected_owner: &str,
        provided_type: &str,
        canonical_type: &str,
    ) -> LeafResult<u64> {
        let info = self
            .builders
            .get(&builder)
            .cloned()
            .ok_or(INVALID_CONSTRUCTION)?;
        if info.owner == expected_owner && info.unsupported_kind.is_some() {
            return self.builder_failure(
                format!("type.construct_put supports only structs, bitfields, enums, and machines, got '{expected_owner}'"),
                builder,
                bits,
                owned,
            );
        }
        let (index, actual_owner, member, name, metadata_type) =
            self.builder_field_metadata(field)?;
        let expected_member = info.variant.as_ref().or(info.state.as_ref());
        let expected_label = expected_member.map_or_else(
            || info.metadata_owner.clone(),
            |member| format!("{}.{member}", info.metadata_owner),
        );
        let slot = index.saturating_add(info.field_offset);
        let error = if info.owner != expected_owner {
            Some(format!(
                "type.construct_put: builder for '{}' cannot construct '{}'",
                info.owner, expected_owner
            ))
        } else if actual_owner != info.metadata_owner
            || member.as_deref() != expected_member.map(String::as_str)
        {
            let actual_label = member.as_ref().map_or(actual_owner.clone(), |member| {
                format!("{actual_owner}.{member}")
            });
            Some(format!(
                "type.construct_put: field metadata belongs to '{actual_label}', expected '{expected_label}'"
            ))
        } else if index >= info.field_names.len() {
            Some(if info.variant.is_some() {
                format!(
                    "type.construct_put: variant '{expected_label}' has no payload field at index {index}"
                )
            } else if info.state.is_some() {
                format!(
                    "type.construct_put: state '{expected_label}' has no payload field at index {index}"
                )
            } else {
                format!("type.construct_put: type '{expected_owner}' has no field at index {index}")
            })
        } else if info.field_names[index] != name {
            Some(if info.variant.is_some() {
                format!(
                    "type.construct_put: field metadata '{name}' does not match payload field '{}' on variant '{expected_label}'",
                    info.field_names[index]
                )
            } else if info.state.is_some() {
                format!(
                    "type.construct_put: field metadata '{name}' does not match payload field '{}' on state '{expected_label}'",
                    info.field_names[index]
                )
            } else {
                format!(
                    "type.construct_put: field metadata '{name}' does not match field '{}' on '{expected_owner}'",
                    info.field_names[index]
                )
            })
        } else if info.field_type_names[index] != metadata_type
            && info.field_types[index] != metadata_type
        {
            Some(format!(
                "type.construct_put: field metadata for '{name}' has type '{metadata_type}', but '{expected_owner}' reports '{}'",
                info.field_type_names[index]
            ))
        } else if info.field_types[index] != canonical_type {
            Some(format!(
                "type.construct_put: field '{name}' has type '{}', provided as '{provided_type}'",
                info.field_type_names[index]
            ))
        } else if self
            .structs
            .get(&builder)
            .and_then(|record| record.fields.get(slot))
            .and_then(|slot| slot.as_ref())
            .is_some()
        {
            Some(format!(
                "type.construct_put: field '{name}' was provided more than once"
            ))
        } else {
            None
        };
        if let Some(error) = error {
            return self.builder_failure(error, builder, bits, owned);
        }
        let result = self.sum(SUM_SUCCESS, builder, true)?;
        self.structs
            .get_mut(&builder)
            .ok_or(INVALID_CONSTRUCTION)?
            .fields[slot] = Some(NativeField { bits, owned });
        Ok(result)
    }
    fn builder_finish(&mut self, builder: u64, expected_owner: &str) -> LeafResult<u64> {
        let info = self
            .builders
            .get(&builder)
            .cloned()
            .ok_or(INVALID_CONSTRUCTION)?;
        if info.owner != expected_owner {
            return self.builder_failure(
                format!(
                    "type.construct_finish: builder for '{}' cannot construct '{}'",
                    info.owner, expected_owner
                ),
                builder,
                0,
                false,
            );
        }
        if info.unsupported_kind.is_some() {
            return self.builder_failure(
                format!("type.construct_finish supports only structs, bitfields, enums, and machines, got '{expected_owner}'"),
                builder,
                0,
                false,
            );
        }
        if let (Some(state), Some(target)) = (&info.state, &info.target_state)
            && state != target
        {
            return self.builder_failure(
                format!(
                    "type.construct_finish: machine target '{} at {target}' cannot finish state '{state}'",
                    info.metadata_owner
                ),
                builder,
                0,
                false,
            );
        }
        let record = self.structs.get(&builder).ok_or(INVALID_CONSTRUCTION)?;
        if let Some((slot, _)) = record
            .fields
            .iter()
            .enumerate()
            .skip(info.field_offset)
            .find(|(_, field)| field.is_none())
        {
            let index = slot - info.field_offset;
            return self.builder_failure(
                if let Some(variant) = &info.variant {
                    format!("type.construct_finish: variant '{expected_owner}.{variant}' is missing required payload field '{}'", info.field_names[index])
                } else if let Some(state) = &info.state {
                    format!("type.construct_finish: state '{}.{state}' is missing required payload field '{}'", info.metadata_owner, info.field_names[index])
                } else {
                    format!("type.construct_finish: '{expected_owner}' is missing required field '{}'", info.field_names[index])
                },
                builder,
                0,
                false,
            );
        }
        for (index, rule) in info.validation.iter().enumerate() {
            let bits = self
                .struct_field(builder, (index + info.field_offset) as u64)?
                .bits;
            let field_name = &info.field_names[index];
            let bitfield_name = &info.owner;
            let message = match rule {
                BuilderFieldValidation::None => None,
                BuilderFieldValidation::SignedBits(width) => {
                    let value = bits as i64;
                    let max = if *width == 64 {
                        u64::MAX
                    } else {
                        (1_u64 << width) - 1
                    };
                    (value < 0 || bits > max).then(|| format!(
                        "bitfield '{bitfield_name}' field '{field_name}' is {width} bit(s) wide and cannot hold '{value}'"
                    ))
                }
                BuilderFieldValidation::UnsignedBits(width) => {
                    let max = if *width == 64 {
                        u64::MAX
                    } else {
                        (1_u64 << width) - 1
                    };
                    (bits > max).then(|| format!(
                        "bitfield '{bitfield_name}' field '{field_name}' is {width} bit(s) wide and cannot hold '{bits}'"
                    ))
                }
                BuilderFieldValidation::EnumBits(width, enum_name, variants) => {
                    let tag = usize::try_from(self.struct_field(bits, 0)?.bits)
                        .map_err(|_| INVALID_CONSTRUCTION)?;
                    let (variant_name, discriminant) =
                        variants.get(tag).ok_or(INVALID_CONSTRUCTION)?;
                    if *discriminant < 0 {
                        Some(format!(
                            "enum '{enum_name}.{variant_name}' has negative discriminant {discriminant}"
                        ))
                    } else {
                        let max = if *width == 64 {
                            u64::MAX
                        } else {
                            (1_u64 << width) - 1
                        };
                        ((*discriminant as u64) > max).then(|| format!(
                            "bitfield '{bitfield_name}' field '{field_name}' is {width} bit(s) wide and cannot hold enum variant '{enum_name}.{variant_name}'"
                        ))
                    }
                }
            };
            if let Some(message) = message {
                return self.builder_failure(message, builder, 0, false);
            }
        }
        let result = self.sum(SUM_SUCCESS, builder, true)?;
        self.builders.remove(&builder);
        Ok(result)
    }
    fn struct_field(&self, id: u64, index: u64) -> LeafResult<NativeField> {
        self.structs
            .get(&id)
            .and_then(|s| usize::try_from(index).ok().and_then(|i| s.fields.get(i)))
            .copied()
            .flatten()
            .ok_or(INVALID_STRUCT)
    }
    fn enum_equal(&self, left: u64, right: u64, layout: &[u8]) -> LeafResult<u32> {
        let mut cursor = BitfieldLayoutCursor {
            bytes: layout,
            position: 0,
        };
        if cursor.take(3).map_err(|_| INVALID_STRUCT)? != b"JE\x01" {
            return Err(INVALID_STRUCT);
        }
        let count = usize::try_from(cursor.u32().map_err(|_| INVALID_STRUCT)?)
            .map_err(|_| INVALID_STRUCT)?;
        if count > layout.len() {
            return Err(INVALID_STRUCT);
        }
        let left_tag =
            usize::try_from(self.struct_field(left, 0)?.bits).map_err(|_| INVALID_STRUCT)?;
        let right_tag =
            usize::try_from(self.struct_field(right, 0)?.bits).map_err(|_| INVALID_STRUCT)?;
        if left_tag >= count || right_tag >= count {
            return Err(INVALID_STRUCT);
        }
        let mut equal = left_tag == right_tag;
        for variant in 0..count {
            let fields = usize::try_from(cursor.u32().map_err(|_| INVALID_STRUCT)?)
                .map_err(|_| INVALID_STRUCT)?;
            let kinds = cursor.take(fields).map_err(|_| INVALID_STRUCT)?;
            if variant != left_tag || !equal {
                continue;
            }
            for (index, kind) in kinds.iter().enumerate() {
                let slot = u64::try_from(index + 1).map_err(|_| INVALID_STRUCT)?;
                let left_value = self.struct_field(left, slot)?.bits;
                let right_value = self.struct_field(right, slot)?.bits;
                let same = match kind {
                    b'i' => left_value == right_value,
                    b'f' => f32::from_bits(left_value as u32) == f32::from_bits(right_value as u32),
                    b'd' => f64::from_bits(left_value) == f64::from_bits(right_value),
                    b's' => self.text(left_value)? == self.text(right_value)?,
                    _ => return Err(INVALID_STRUCT),
                };
                if !same {
                    equal = false;
                }
            }
        }
        if cursor.position != layout.len() {
            return Err(INVALID_STRUCT);
        }
        Ok(u32::from(equal))
    }
    fn reflected_field_index(
        &self,
        actual: u64,
        expected: u64,
        mismatch: Failure,
    ) -> LeafResult<u64> {
        if expected == 0 {
            return Err(mismatch);
        }
        let index = self.struct_field(actual, 0)?.bits;
        if index != self.struct_field(expected, 0)?.bits {
            return Err(mismatch);
        }
        for position in [1, 3, 4] {
            let actual_text = self.text(self.struct_field(actual, position)?.bits)?;
            let expected_text = self.text(self.struct_field(expected, position)?.bits)?;
            if actual_text != expected_text {
                return Err(mismatch);
            }
        }
        let actual_member = self
            .sums
            .get(&self.struct_field(actual, 2)?.bits)
            .ok_or(mismatch)?;
        let expected_member = self
            .sums
            .get(&self.struct_field(expected, 2)?.bits)
            .ok_or(mismatch)?;
        if actual_member.tag != expected_member.tag
            || (actual_member.tag == SUM_SUCCESS
                && self.text(actual_member.bits)? != self.text(expected_member.bits)?)
        {
            return Err(mismatch);
        }
        Ok(index)
    }
    fn type_info_identity(&self, value: u64, depth: usize) -> LeafResult<String> {
        if depth >= 64 {
            return Err(INVALID_TYPE_INFO);
        }
        let type_name = self.text(self.struct_field(value, 0)?.bits)?;
        let kind = self.text(self.struct_field(value, 1)?.bits)?;
        let secret = self.struct_field(value, 4)?.bits;
        if secret > 1 {
            return Err(INVALID_TYPE_INFO);
        }
        let arguments = self
            .lists
            .get(&self.struct_field(value, 5)?.bits)
            .ok_or(INVALID_TYPE_INFO)?;
        if kind == "alias" {
            let [Some(base)] = arguments.elements.as_slice() else {
                return Err(INVALID_TYPE_INFO);
            };
            return self.type_info_identity(*base, depth + 1);
        }
        let name = match kind {
            "list" | "set" | "map" | "optional" | "result" | "secret" | "function" => "",
            "struct" if !arguments.elements.is_empty() => type_name
                .split_once('[')
                .map_or(type_name, |(base, _)| base),
            _ => type_name,
        };
        let mut identity = format!(
            "{}:{}{}:{}{}:{}",
            kind.len(),
            kind,
            name.len(),
            name,
            secret,
            arguments.elements.len(),
        );
        for argument in &arguments.elements {
            identity
                .push_str(&self.type_info_identity(argument.ok_or(INVALID_TYPE_INFO)?, depth + 1)?);
        }
        Ok(identity)
    }
    fn take_struct_field(&mut self, id: u64, index: u64) -> LeafResult<NativeField> {
        self.structs
            .get_mut(&id)
            .and_then(|s| {
                usize::try_from(index)
                    .ok()
                    .and_then(|i| s.fields.get_mut(i))
            })
            .and_then(Option::take)
            .ok_or(INVALID_STRUCT)
    }
    fn clone_struct(&mut self, id: u64) -> LeafResult<u64> {
        let fields = self.structs.get(&id).ok_or(INVALID_STRUCT)?.fields.clone();
        let builder = self.builders.get(&id).cloned();
        let output = self.new_struct(fields.len() as u64)?;
        for (index, field) in fields.into_iter().enumerate() {
            let Some(mut field) = field else {
                continue;
            };
            if field.owned {
                match self.clone_value(field.bits) {
                    Ok(bits) => field.bits = bits,
                    Err(error) => {
                        self.drop_value(output)?;
                        return Err(error);
                    }
                }
            }
            self.structs.get_mut(&output).ok_or(INVALID_STRUCT)?.fields[index] = Some(field);
        }
        if let Some(builder) = builder {
            self.builders.insert(output, builder);
        }
        Ok(output)
    }
    fn clone_value(&mut self, id: u64) -> LeafResult<u64> {
        if self.structs.contains_key(&id) {
            return self.clone_struct(id);
        }
        if self.lists.contains_key(&id) {
            return self.clone_list(id);
        }
        if self.sets.contains_key(&id) {
            return self.clone_set(id);
        }
        if self.maps.contains_key(&id) {
            return self.clone_map(id);
        }
        if let Some(sum) = self.sums.get(&id) {
            let (tag, bits, owned) = (sum.tag, sum.bits, sum.owned);
            let bits = if owned { self.clone_value(bits)? } else { bits };
            return match self.sum(tag, bits, owned) {
                Ok(id) => Ok(id),
                Err(error) => {
                    if owned {
                        self.drop_value(bits)?;
                    }
                    Err(error)
                }
            };
        }
        if let Some(bytes) = self.bytes.get(&id) {
            return self.insert_bytes(bytes.clone());
        }
        self.retain(id)
    }
    fn drop_value(&mut self, id: u64) -> LeafResult<u32> {
        if self.actors.contains(&id) {
            return Err(INVALID_ACTOR);
        }
        if let Some(value) = self.structs.remove(&id) {
            self.builders.remove(&id);
            self.structs_destroyed += 1;
            for field in value.fields.into_iter().flatten() {
                if field.owned {
                    self.drop_value(field.bits)?;
                }
            }
            return Ok(0);
        }
        if let Some(list) = self.lists.remove(&id) {
            self.lists_destroyed += 1;
            if list.owned {
                for value in list.elements.into_iter().flatten() {
                    self.drop_value(value)?;
                }
            }
            return Ok(0);
        }
        if let Some(set) = self.sets.remove(&id) {
            self.sets_destroyed += 1;
            if set.strings {
                for value in set.elements.into_iter().flatten() {
                    self.drop_value(value)?;
                }
            }
            return Ok(0);
        }
        if let Some(map) = self.maps.remove(&id) {
            self.maps_destroyed += 1;
            for entry in map.entries.into_iter().flatten() {
                if map.key_strings && !entry.key_taken {
                    self.drop_value(entry.key)?;
                }
                if map.value_owned {
                    self.drop_value(entry.value)?;
                }
            }
            return Ok(0);
        }
        if let Some(sum) = self.sums.remove(&id) {
            self.sums_destroyed += 1;
            if sum.owned {
                self.drop_value(sum.bits)?;
            }
            return Ok(0);
        }
        if self.bytes.remove(&id).is_some() {
            self.bytes_destroyed += 1;
            return Ok(0);
        }
        self.release(id)
    }
    fn text(&self, id: u64) -> LeafResult<&str> {
        self.strings
            .get(&id)
            .map(|s| s.text.as_str())
            .ok_or(INVALID_HANDLE)
    }
    fn retain(&mut self, id: u64) -> LeafResult<u64> {
        let value = self.strings.get_mut(&id).ok_or(INVALID_HANDLE)?;
        value.references = value.references.checked_add(1).ok_or(EXHAUSTED)?;
        Ok(id)
    }
    fn release(&mut self, id: u64) -> LeafResult<u32> {
        if id == 0 {
            return Ok(0);
        }
        let value = self.strings.get_mut(&id).ok_or(INVALID_HANDLE)?;
        value.references -= 1;
        if value.references == 0 {
            self.strings.remove(&id);
        }
        Ok(0)
    }
}
fn native_split<'a>(haystack: &'a str, delimiter: &str) -> Vec<&'a str> {
    if delimiter.is_empty() {
        let graphemes = native_graphemes(haystack);
        let mut parts = Vec::with_capacity(graphemes.len() + 2);
        parts.push("");
        parts.extend(graphemes);
        parts.push("");
        return parts;
    }

    let mut parts = Vec::new();
    let mut part_start = 0;
    native_scan_grapheme_matches(haystack, delimiter, |start, end, _| {
        parts.push(&haystack[part_start..start]);
        part_start = end;
        true
    });
    parts.push(&haystack[part_start..]);
    parts
}

fn native_replace(haystack: &str, needle: &str, replacement: &str) -> LeafResult<String> {
    let parts = native_split(haystack, needle);
    let separators = parts.len().saturating_sub(1);
    let mut length = replacement.len().checked_mul(separators).ok_or(EXHAUSTED)?;
    for part in &parts {
        length = length.checked_add(part.len()).ok_or(EXHAUSTED)?;
    }
    let mut output = String::new();
    output.try_reserve_exact(length).map_err(|_| EXHAUSTED)?;
    for (index, part) in parts.into_iter().enumerate() {
        if index != 0 {
            output.push_str(replacement);
        }
        output.push_str(part);
    }
    Ok(output)
}

fn native_change_first(value: &str, case: impl Fn(char) -> String) -> String {
    let Some(first) = value.graphemes(true).next() else {
        return String::new();
    };
    let mut chars = first.chars();
    let changed = chars.next().map(case).unwrap_or_default();
    format!("{changed}{}{}", chars.as_str(), &value[first.len()..])
}

fn native_slugify(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Visit non-overlapping matches whose start and end are grapheme boundaries.
/// Returning false stops after the current match, as index_of requires.
fn native_scan_grapheme_matches(
    haystack: &str,
    needle: &str,
    mut visit: impl FnMut(usize, usize, usize) -> bool,
) {
    if needle.is_empty() || needle.len() > haystack.len() {
        return;
    }
    let boundaries = native_grapheme_boundaries(haystack);
    let needle = needle.as_bytes();
    let mut prefix = vec![0; needle.len()];
    for index in 1..needle.len() {
        let mut matched = prefix[index - 1];
        while matched > 0 && needle[index] != needle[matched] {
            matched = prefix[matched - 1];
        }
        if needle[index] == needle[matched] {
            matched += 1;
        }
        prefix[index] = matched;
    }

    let mut matched = 0;
    let mut start_boundary = 0;
    let mut end_boundary = 0;
    // KMP avoids comparing a long near-match again at every grapheme boundary.
    // Both boundary cursors move only forward, so the entire scan is linear.
    for (index, &byte) in haystack.as_bytes().iter().enumerate() {
        while matched > 0 && byte != needle[matched] {
            matched = prefix[matched - 1];
        }
        if byte == needle[matched] {
            matched += 1;
        }
        if matched != needle.len() {
            continue;
        }

        let end = index + 1;
        let start = end - needle.len();
        while boundaries[start_boundary] < start {
            start_boundary += 1;
        }
        while boundaries[end_boundary] < end {
            end_boundary += 1;
        }
        if boundaries[start_boundary] == start && boundaries[end_boundary] == end {
            if !visit(start, end, start_boundary) {
                return;
            }
            matched = 0;
        } else {
            // A rejected byte match can overlap a later valid grapheme match.
            matched = prefix[matched - 1];
        }
    }
}

fn native_grapheme_boundaries(s: &str) -> Vec<usize> {
    let mut boundaries = vec![0];
    let mut offset = 0;
    for cluster in native_graphemes(s) {
        offset += cluster.len();
        boundaries.push(offset);
    }
    boundaries
}

fn native_graphemes(s: &str) -> Vec<&str> {
    UnicodeSegmentation::graphemes(s, true).collect()
}

fn native_lines(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut parts = Vec::new();
    let mut start = 0;
    let mut index = 0;
    while index < bytes.len() {
        if matches!(bytes[index], 13 | 10) {
            parts.push(text[start..index].to_owned());
            if bytes[index] == 13 && bytes.get(index + 1) == Some(&10) {
                index += 1;
            }
            index += 1;
            start = index;
        } else {
            index += 1;
        }
    }
    if start < bytes.len() {
        parts.push(text[start..].to_owned());
    }
    parts
}
fn next_identity() -> LeafResult<u64> {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
        .map_err(|_| EXHAUSTED)
}

// Hold the existing context lease through the operation. Cleanup is permitted
// after failure; normal operations cannot run once the first failure is set.
trait FailureDefault {
    fn failure_default() -> Self;
}
impl FailureDefault for u64 {
    fn failure_default() -> Self {
        0
    }
}
impl FailureDefault for u32 {
    fn failure_default() -> Self {
        JettRuntimeStatusV1::INVALID_CONTEXT.code()
    }
}
impl FailureDefault for i64 {
    fn failure_default() -> Self {
        0
    }
}
impl FailureDefault for f64 {
    fn failure_default() -> Self {
        0.0
    }
}
fn leaf<T: FailureDefault>(
    context: *const JettRuntimeContextV1,
    cleanup: bool,
    operation: impl FnOnce(&mut NativeValues) -> LeafResult<T>,
) -> T {
    let Ok(key) = context_key(context) else {
        return T::failure_default();
    };
    let Ok(lease) = acquire_context(key) else {
        return T::failure_default();
    };
    let mut state = lock_unpoisoned(&lease.entry.state);
    let Some(state) = state.as_mut() else {
        return T::failure_default();
    };
    if state.values.failure.is_some() && !cleanup {
        return T::failure_default();
    }
    match catch_unwind(AssertUnwindSafe(|| operation(&mut state.values))) {
        Ok(Ok(value)) => value,
        outcome => {
            let error = match outcome {
                Ok(Err(error)) => error,
                Err(payload) => {
                    discard_panic_payload(payload);
                    (JettRuntimeStatusV1::PANIC, PANIC_MESSAGE)
                }
                Ok(Ok(_)) => unreachable!(),
            };
            state.values.cleanup_failed |= cleanup;
            state.values.failure.get_or_insert(error);
            T::failure_default()
        }
    }
}

/// Configure an opt-in deterministic Clock provider before native program entry.
/// An empty script selects an exhausted provider rather than the wall clock.
///
/// # Safety
/// `data` must point to `length` readable bytes when `length` is nonzero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_clock_configure_scripted(
    context: *const JettRuntimeContextV1,
    data: *const u8,
    length: u64,
) -> u32 {
    leaf(context, false, |s| {
        let invalid = (
            JettRuntimeStatusV1::INVALID_ARGUMENT,
            clock::INVALID_SCRIPT.as_bytes(),
        );
        if s.clock.is_some() || s.clock_script.is_some() {
            return Err(invalid);
        }
        let length = usize::try_from(length).map_err(|_| invalid)?;
        if length > isize::MAX as usize || (length != 0 && data.is_null()) {
            return Err(invalid);
        }
        let bytes = if length == 0 {
            &[][..]
        } else {
            // SAFETY: the caller supplies a readable slice for this call.
            unsafe { std::slice::from_raw_parts(data, length) }
        };
        let script = std::str::from_utf8(bytes).map_err(|_| invalid)?;
        s.clock_script = Some(clock::decode_test_script(script).map_err(|_| invalid)?);
        Ok(0)
    })
}

/// Configure a deterministic Random provider before the capability is granted.
///
/// # Safety
/// `data` must point to `length` readable bytes when `length` is nonzero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_random_configure_scripted(
    context: *const JettRuntimeContextV1,
    data: *const u8,
    length: u64,
) -> u32 {
    leaf(context, false, |s| {
        let invalid = (
            JettRuntimeStatusV1::INVALID_ARGUMENT,
            random::INVALID_SCRIPT.as_bytes(),
        );
        if s.random.is_some() || s.random_provider.is_some() {
            return Err(invalid);
        }
        let length = usize::try_from(length).map_err(|_| invalid)?;
        if length > isize::MAX as usize || (length != 0 && data.is_null()) {
            return Err(invalid);
        }
        let bytes = if length == 0 {
            &[][..]
        } else {
            // SAFETY: the caller supplies a readable slice for this call.
            unsafe { std::slice::from_raw_parts(data, length) }
        };
        let script = std::str::from_utf8(bytes).map_err(|_| invalid)?;
        s.random_provider = Some(RandomProvider::scripted(
            random::decode_test_script(script).map_err(|_| invalid)?,
        ));
        Ok(0)
    })
}

/// Configure an isolated Environment launch snapshot before program entry.
///
/// # Safety
/// `data` must point to `length` readable bytes when `length` is nonzero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_environment_configure_snapshot(
    context: *const JettRuntimeContextV1,
    data: *const u8,
    length: u64,
) -> u32 {
    leaf(context, false, |s| {
        let invalid = (
            JettRuntimeStatusV1::INVALID_ARGUMENT,
            environment::INVALID_SNAPSHOT.as_bytes(),
        );
        if s.environment.is_some() || s.environment_snapshot.is_some() {
            return Err(invalid);
        }
        let length = usize::try_from(length).map_err(|_| invalid)?;
        if length > isize::MAX as usize || (length != 0 && data.is_null()) {
            return Err(invalid);
        }
        let bytes = if length == 0 {
            &[][..]
        } else {
            // SAFETY: the caller supplies a readable slice for this call.
            unsafe { std::slice::from_raw_parts(data, length) }
        };
        let script = std::str::from_utf8(bytes).map_err(|_| invalid)?;
        let snapshot = environment::decode_test_snapshot(script).map_err(|_| invalid)?;
        s.environment_snapshot = Some(
            LaunchEnvironmentSnapshot::injected(snapshot)
                .map_err(|message| (JettRuntimeStatusV1::INVALID_ARGUMENT, message.as_bytes()))?,
        );
        Ok(0)
    })
}

/// Scalar signature schema consumed by Cranelift, never inferred from names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AbiScalar {
    Pointer,
    I32,
    I64,
    F64,
}
macro_rules! leaves {
    ($( $variant:ident, $name:ident, $cleanup:literal, ($($arg:ident: $rust:ty => $abi:ident),*), $ret:ty => $retabi:ident, $body:expr; )*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum NativeLeaf { $( $variant, )* }
        impl NativeLeaf {
            pub fn symbol(self) -> &'static str { match self { $( Self::$variant => stringify!($name), )* } }
            pub fn parameters(self) -> &'static [AbiScalar] { match self { $( Self::$variant => &[AbiScalar::Pointer, $( AbiScalar::$abi, )*], )* } }
            pub fn result(self) -> AbiScalar { match self { $( Self::$variant => AbiScalar::$retabi, )* } }
        }
        $(
            /// Typed leaf operation; borrowed inputs, owned handle results.
            /// # Safety
            /// Context must be readable, stationary and live for the call.
            /// Pointer/length inputs must describe one readable allocation.
            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn $name(context: *const JettRuntimeContextV1, $( $arg: $rust ),*) -> $ret {
                leaf(context, $cleanup, $body)
            }
        )*
    }
}
leaves! {
    BuilderNew, jett_rt_v1_builder_new, false, (layout_pointer: u64 => I64, layout_length: u64 => I64), u64 => I64,
        |s| { if layout_pointer == 0 { return Err(INVALID_CONSTRUCTION_LAYOUT); }
            let length = usize::try_from(layout_length).map_err(|_| INVALID_CONSTRUCTION_LAYOUT)?;
            if length > isize::MAX as usize { return Err(INVALID_CONSTRUCTION_LAYOUT); }
            let layout = unsafe { std::slice::from_raw_parts(layout_pointer as *const u8, length) };
            s.new_builder(layout) };
    BuilderVariantNew, jett_rt_v1_builder_variant_new, false, (layout_pointer: u64 => I64, layout_length: u64 => I64, metadata: u64 => I64), u64 => I64,
        |s| { if layout_pointer == 0 { return Err(INVALID_CONSTRUCTION_LAYOUT); }
            let length = usize::try_from(layout_length).map_err(|_| INVALID_CONSTRUCTION_LAYOUT)?;
            if length > isize::MAX as usize { return Err(INVALID_CONSTRUCTION_LAYOUT); }
            let layout = unsafe { std::slice::from_raw_parts(layout_pointer as *const u8, length) };
            s.new_member_builder(layout, metadata, false) };
    BuilderMachineNew, jett_rt_v1_builder_machine_new, false, (layout_pointer: u64 => I64, layout_length: u64 => I64, metadata: u64 => I64), u64 => I64,
        |s| { if layout_pointer == 0 { return Err(INVALID_CONSTRUCTION_LAYOUT); }
            let length = usize::try_from(layout_length).map_err(|_| INVALID_CONSTRUCTION_LAYOUT)?;
            if length > isize::MAX as usize { return Err(INVALID_CONSTRUCTION_LAYOUT); }
            let layout = unsafe { std::slice::from_raw_parts(layout_pointer as *const u8, length) };
            s.new_member_builder(layout, metadata, true) };
    BuilderPut, jett_rt_v1_builder_put, false, (builder: u64 => I64, field: u64 => I64, bits: u64 => I64, owned: u32 => I32, owner_pointer: u64 => I64, owner_length: u64 => I64, type_pointer: u64 => I64, type_length: u64 => I64, canonical_pointer: u64 => I64, canonical_length: u64 => I64), u64 => I64,
        |s| { if owned > 1 || owner_pointer == 0 || type_pointer == 0 || canonical_pointer == 0 { return Err(INVALID_CONSTRUCTION); }
            let owner_length = usize::try_from(owner_length).map_err(|_| INVALID_CONSTRUCTION)?;
            let type_length = usize::try_from(type_length).map_err(|_| INVALID_CONSTRUCTION)?;
            let canonical_length = usize::try_from(canonical_length).map_err(|_| INVALID_CONSTRUCTION)?;
            if owner_length > isize::MAX as usize || type_length > isize::MAX as usize || canonical_length > isize::MAX as usize { return Err(INVALID_CONSTRUCTION); }
            let owner = unsafe { std::slice::from_raw_parts(owner_pointer as *const u8, owner_length) };
            let provided_type = unsafe { std::slice::from_raw_parts(type_pointer as *const u8, type_length) };
            let canonical_type = unsafe { std::slice::from_raw_parts(canonical_pointer as *const u8, canonical_length) };
            let owner = std::str::from_utf8(owner).map_err(|_| INVALID_CONSTRUCTION)?;
            let provided_type = std::str::from_utf8(provided_type).map_err(|_| INVALID_CONSTRUCTION)?;
            let canonical_type = std::str::from_utf8(canonical_type).map_err(|_| INVALID_CONSTRUCTION)?;
            s.builder_put(builder, field, bits, owned != 0, owner, provided_type, canonical_type) };
    BuilderFinish, jett_rt_v1_builder_finish, false, (builder: u64 => I64, owner_pointer: u64 => I64, owner_length: u64 => I64), u64 => I64,
        |s| { if owner_pointer == 0 { return Err(INVALID_CONSTRUCTION); }
            let owner_length = usize::try_from(owner_length).map_err(|_| INVALID_CONSTRUCTION)?;
            if owner_length > isize::MAX as usize { return Err(INVALID_CONSTRUCTION); }
            let owner = unsafe { std::slice::from_raw_parts(owner_pointer as *const u8, owner_length) };
            let owner = std::str::from_utf8(owner).map_err(|_| INVALID_CONSTRUCTION)?;
            s.builder_finish(builder, owner) };
    StructNew, jett_rt_v1_struct_new, false, (count: u64 => I64), u64 => I64,
        |s| s.new_struct(count);
    StructInit, jett_rt_v1_struct_init, false, (value: u64 => I64, index: u64 => I64, bits: u64 => I64, owned: u32 => I32), u32 => I32,
        |s| { if owned > 1 { return Err(INVALID_STRUCT); }
            let slot = s.structs.get_mut(&value).and_then(|v| usize::try_from(index).ok().and_then(|i| v.fields.get_mut(i))).ok_or(INVALID_STRUCT)?;
            if slot.is_some() { return Err(INVALID_STRUCT); }
            *slot = Some(NativeField { bits, owned: owned != 0 }); Ok(0) };
    StructField, jett_rt_v1_struct_field, false, (value: u64 => I64, index: u64 => I64), u64 => I64,
        |s| Ok(s.struct_field(value, index)?.bits);
    ActorRegister, jett_rt_v1_actor_register, false, (value: u64 => I64), u64 => I64,
        |s| s.register_actor(value);
    ActorReplace, jett_rt_v1_actor_replace, false, (value: u64 => I64, index: u64 => I64, bits: u64 => I64, owned: u32 => I32), u32 => I32,
        |s| { if owned > 1 { return Err(INVALID_ACTOR); }
            s.replace_actor_field(value, index, bits, owned != 0) };
    ReflectedFieldIndex, jett_rt_v1_reflected_field_index, false, (actual: u64 => I64, expected: u64 => I64), u64 => I64,
        |s| s.reflected_field_index(actual, expected, INVALID_REFLECTED_FIELD);
    ReflectedVariantFieldIndex, jett_rt_v1_reflected_variant_field_index, false, (actual: u64 => I64, expected: u64 => I64), u64 => I64,
        |s| s.reflected_field_index(actual, expected, INVALID_REFLECTED_VARIANT_FIELD);
    ReflectedMachineFieldIndex, jett_rt_v1_reflected_machine_field_index, false, (actual: u64 => I64, expected: u64 => I64), u64 => I64,
        |s| s.reflected_field_index(actual, expected, INVALID_REFLECTED_MACHINE_FIELD);
    TypeInfoMatches, jett_rt_v1_type_info_matches, false, (actual: u64 => I64, expected: *const u8 => Pointer, length: u64 => I64), u32 => I32,
        |s| { let length = usize::try_from(length).map_err(|_| INVALID_TYPE_INFO)?;
            if length > isize::MAX as usize || (length != 0 && expected.is_null()) { return Err(INVALID_TYPE_INFO); }
            let expected = if length == 0 { &[][..] } else { unsafe { std::slice::from_raw_parts(expected, length) } };
            Ok(u32::from(s.type_info_identity(actual, 0)?.as_bytes() == expected)) };
    TypeArgIndex, jett_rt_v1_type_arg_index, false, (index: i64 => I64, count: u64 => I64), u64 => I64,
        |_s| { let index = u64::try_from(index).map_err(|_| INVALID_TYPE_ARG_INDEX)?;
            if index >= count { return Err(INVALID_TYPE_ARG_INDEX); }
            Ok(index) };
    MachineExpectState, jett_rt_v1_machine_expect_state, false, (value: u64 => I64, state: u64 => I64), u32 => I32,
        |s| { if s.struct_field(value, 0)?.bits == state { Ok(0) }
            else { Err((JettRuntimeStatusV1::INVALID_ARGUMENT, b"machine state does not match narrowed type")) } };
    StructTake, jett_rt_v1_struct_take, false, (value: u64 => I64, index: u64 => I64), u64 => I64,
        |s| Ok(s.take_struct_field(value, index)?.bits);
    StructClone, jett_rt_v1_struct_clone, false, (value: u64 => I64), u64 => I64,
        |s| s.clone_struct(value);
    StringChars, jett_rt_v1_string_chars, false, (value: u64 => I64), u64 => I64,
        |s| { let parts = s.text(value)?.graphemes(true).map(str::to_owned).collect(); s.string_list(parts) };
    StringScalarCount, jett_rt_v1_string_scalar_count, false, (value: u64 => I64), i64 => I64,
        |s| Ok(s.text(value)?.chars().count() as i64);
    StringScalarAt, jett_rt_v1_string_scalar_at, false, (value: u64 => I64, index: i64 => I64), u64 => I64,
        |s| { let index = usize::try_from(index).map_err(|_| INVALID_STRING_SCALAR_INDEX)?;
            let scalar = s.text(value)?.chars().nth(index).ok_or(INVALID_STRING_SCALAR_INDEX)?;
            s.insert(scalar.to_string()) };
    StringWords, jett_rt_v1_string_words, false, (value: u64 => I64), u64 => I64,
        |s| { let parts = s.text(value)?.split_whitespace().map(str::to_owned).collect(); s.string_list(parts) };
    StringLines, jett_rt_v1_string_lines, false, (value: u64 => I64), u64 => I64,
        |s| { let parts = native_lines(s.text(value)?); s.string_list(parts) };
    StringSplit, jett_rt_v1_string_split, false, (value: u64 => I64, delimiter: u64 => I64), u64 => I64,
        |s| { let parts = native_split(s.text(value)?, s.text(delimiter)?).into_iter().map(str::to_owned).collect(); s.string_list(parts) };
    StringJoin, jett_rt_v1_string_join, false, (value: u64 => I64, separator: u64 => I64), u64 => I64,
        |s| s.join_strings(value, separator);
    Range, jett_rt_v1_range_int64, false, (start: i64 => I64, end: i64 => I64, step: i64 => I64), u64 => I64,
        |s| s.range(start, end, step);
    ListElementTake, jett_rt_v1_list_element_take, false, (value: u64 => I64, index: i64 => I64), u64 => I64,
        |s| { let list = s.lists.get_mut(&value).ok_or(INVALID_LIST)?;
            usize::try_from(index).ok().and_then(|i| list.elements.get_mut(i)).and_then(Option::take).ok_or(INVALID_LIST) };

    ListElementClone, jett_rt_v1_list_element_clone, false, (value: u64 => I64, index: i64 => I64), u64 => I64,
        |s| { let list = s.lists.get(&value).ok_or(INVALID_LIST)?;
            let bits = usize::try_from(index).ok().and_then(|i| list.elements.get(i)).copied().flatten().ok_or(INVALID_LIST)?;
            if list.owned { s.clone_value(bits) } else { Ok(bits) } };

    ListSumInt, jett_rt_v1_list_sum_int64, false, (value: u64 => I64), i64 => I64,
        |s| { let list = s.lists.get(&value).ok_or(INVALID_LIST)?;
            if list.owned { return Err(INVALID_LIST); }
            Ok(list.elements.iter().flatten().fold(0_i64, |acc, bits| acc.wrapping_add(*bits as i64))) };

    MathAverage, jett_rt_v1_math_average, false, (value: u64 => I64, kind: u32 => I32), f64 => F64,
        |s| { let numbers = s.math_numbers(value, kind, (JettRuntimeStatusV1::INVALID_ARGUMENT, b"math.average: list is empty"))?;
            Ok(math::float_average(&numbers)) };
    MathMedian, jett_rt_v1_math_median, false, (value: u64 => I64, kind: u32 => I32), f64 => F64,
        |s| { let mut numbers = s.math_numbers(value, kind, (JettRuntimeStatusV1::INVALID_ARGUMENT, b"math.median: list is empty"))?;
            numbers.sort_by(|a, b| a.partial_cmp(b).unwrap_or(CompareOrdering::Equal));
            let middle = numbers.len() / 2;
            Ok(if numbers.len() % 2 == 0 { math::float_midpoint(numbers[middle - 1], numbers[middle]) } else { numbers[middle] }) };

    ListNew, jett_rt_v1_list_new, false, (owned: u32 => I32), u64 => I64,
        |s| { if owned > 1 { return Err(INVALID_LIST); } s.new_list(owned != 0) };
    ListInsertAt, jett_rt_v1_list_insert_at, false, (value: u64 => I64, index: i64 => I64, bits: u64 => I64), u64 => I64,
        |s| { let invalid = (JettRuntimeStatusV1::INVALID_ARGUMENT, b"list.__insert_at: index out of bounds".as_slice());
            let index = usize::try_from(index).map_err(|_| invalid)?;
            let list = s.lists.get_mut(&value).ok_or(INVALID_LIST)?;
            if index > list.elements.len() { return Err(invalid); }
            list.elements.try_reserve(1).map_err(|_| EXHAUSTED)?;
            list.elements.insert(index, Some(bits)); Ok(value) };
    ListRemoveAt, jett_rt_v1_list_remove_at, false, (value: u64 => I64, index: i64 => I64), u64 => I64,
        |s| { let invalid = (JettRuntimeStatusV1::INVALID_ARGUMENT, b"list.__remove_at: index out of bounds".as_slice());
            let index = usize::try_from(index).map_err(|_| invalid)?;
            let list = s.lists.get(&value).ok_or(INVALID_LIST)?;
            let bits = list.elements.get(index).copied().flatten().ok_or(invalid)?;
            if list.owned { s.drop_value(bits)?; }
            s.lists.get_mut(&value).ok_or(INVALID_LIST)?.elements.remove(index);
            Ok(value) };
    ListSort, jett_rt_v1_list_sort, false, (value: u64 => I64, kind: u32 => I32), u64 => I64,
        |s| s.sort_list(value, kind);
    ListSortByIndex, jett_rt_v1_list_sort_by_index, false, (value: u64 => I64, index: i64 => I64, kind: u32 => I32), u64 => I64,
        |s| s.sort_list_by_index(value, index, kind);
    ListIsSorted, jett_rt_v1_list_is_sorted, false, (value: u64 => I64, kind: u32 => I32), u32 => I32,
        |s| s.list_is_sorted(value, kind).map(u32::from);
    ListSwap, jett_rt_v1_list_swap, false, (value: u64 => I64, first: i64 => I64, second: i64 => I64), u64 => I64,
        |s| { let list = s.lists.get_mut(&value).ok_or(INVALID_LIST)?;
            let invalid = (JettRuntimeStatusV1::INVALID_ARGUMENT, b"list.__swap: index out of bounds".as_slice());
            let first = usize::try_from(first).map_err(|_| invalid)?;
            let second = usize::try_from(second).map_err(|_| invalid)?;
            if first >= list.elements.len() || second >= list.elements.len() { return Err(invalid); }
            list.elements.swap(first, second); Ok(value) };
    SetNew, jett_rt_v1_set_new, false, (strings: u32 => I32), u64 => I64,
        |s| s.new_set(strings);
    SetAdd, jett_rt_v1_set_add, false, (value: u64 => I64, key: u64 => I64), u64 => I64,
        |s| s.set_add(value, key);
    SetRemove, jett_rt_v1_set_remove, false, (value: u64 => I64, key: u64 => I64), u64 => I64,
        |s| s.set_remove(value, key);
    SetContains, jett_rt_v1_set_contains, false, (value: u64 => I64, key: u64 => I64), u32 => I32,
        |s| Ok(u32::from(s.set_position(value, key)?.is_some()));
    SetLength, jett_rt_v1_set_length, false, (value: u64 => I64), i64 => I64,
        |s| Ok(s.sets.get(&value).ok_or(INVALID_SET)?.elements.len() as i64);
    SetClone, jett_rt_v1_set_clone, false, (value: u64 => I64), u64 => I64,
        |s| s.clone_set(value);
    SetElementTake, jett_rt_v1_set_element_take, false, (value: u64 => I64, index: i64 => I64), u64 => I64,
        |s| s.sets.get_mut(&value).ok_or(INVALID_SET)?
            .elements.get_mut(usize::try_from(index).map_err(|_| INVALID_SET)?)
            .and_then(Option::take).ok_or(INVALID_SET);
    SetElementClone, jett_rt_v1_set_element_clone, false, (value: u64 => I64, index: i64 => I64), u64 => I64,
        |s| { let set = s.sets.get(&value).ok_or(INVALID_SET)?;
            let element = set.elements.get(usize::try_from(index).map_err(|_| INVALID_SET)?)
                .copied().flatten().ok_or(INVALID_SET)?;
            if set.strings { s.retain(element) } else { Ok(element) } };
    MapNew, jett_rt_v1_map_new, false, (key_strings: u32 => I32, value_owned: u32 => I32), u64 => I64,
        |s| s.new_map(key_strings, value_owned);
    MapInsert, jett_rt_v1_map_insert, false, (map: u64 => I64, key: u64 => I64, value: u64 => I64), u64 => I64,
        |s| s.map_insert(map, key, value);
    MapAppendLiteral, jett_rt_v1_map_append_literal, false, (map: u64 => I64, key: u64 => I64, value: u64 => I64), u64 => I64,
        |s| s.map_append_literal(map, key, value);
    MapRemove, jett_rt_v1_map_remove, false, (map: u64 => I64, key: u64 => I64), u64 => I64,
        |s| s.map_remove(map, key);
    MapHas, jett_rt_v1_map_has, false, (map: u64 => I64, key: u64 => I64), u32 => I32,
        |s| Ok(u32::from(s.map_position(map, key)?.is_some()));
    MapLength, jett_rt_v1_map_length, false, (map: u64 => I64), i64 => I64,
        |s| Ok(s.maps.get(&map).ok_or(INVALID_MAP)?.entries.len() as i64);
    MapGet, jett_rt_v1_map_get, false, (map: u64 => I64, key: u64 => I64), u64 => I64,
        |s| s.map_get(map, key);
    MapClone, jett_rt_v1_map_clone, false, (map: u64 => I64), u64 => I64,
        |s| s.clone_map(map);
    MapFromLists, jett_rt_v1_map_from_lists, false, (keys: u64 => I64, values: u64 => I64, key_strings: u32 => I32, value_owned: u32 => I32), u64 => I64,
        |s| s.map_from_lists(keys, values, key_strings, value_owned);
    MapKeyTake, jett_rt_v1_map_key_take, false, (map: u64 => I64, index: i64 => I64), u64 => I64,
        |s| s.map_element(map, index, true, true);
    MapKeyClone, jett_rt_v1_map_key_clone, false, (map: u64 => I64, index: i64 => I64), u64 => I64,
        |s| s.map_element(map, index, true, false);
    MapValueTake, jett_rt_v1_map_value_take, false, (map: u64 => I64, index: i64 => I64), u64 => I64,
        |s| s.map_element(map, index, false, true);
    MapValueClone, jett_rt_v1_map_value_clone, false, (map: u64 => I64, index: i64 => I64), u64 => I64,
        |s| s.map_element(map, index, false, false);
    ListLength, jett_rt_v1_list_length, false, (value: u64 => I64), i64 => I64,
        |s| Ok(s.lists.get(&value).ok_or(INVALID_LIST)?.elements.len() as i64);
    ListAppend, jett_rt_v1_list_append, false, (value: u64 => I64, bits: u64 => I64), u64 => I64,
        |s| { let list = s.lists.get_mut(&value).ok_or(INVALID_LIST)?;
            list.elements.try_reserve(1).map_err(|_| EXHAUSTED)?;
            list.elements.push(Some(bits)); Ok(value) };
    ListGet, jett_rt_v1_list_get_clone, false, (value: u64 => I64, index: i64 => I64), u64 => I64,
        |s| { let list = s.lists.get(&value).ok_or(INVALID_LIST)?;
            let owned = list.owned;
            let found = usize::try_from(index).ok().and_then(|i| list.elements.get(i)).copied().flatten();
            let Some(bits) = found else { return s.sum(SUM_FAILURE, 0, false); };
            let bits = if owned { s.clone_value(bits)? } else { bits };
            match s.sum(SUM_SUCCESS, bits, owned) { Ok(v) => Ok(v), Err(e) => { if owned { s.drop_value(bits)?; } Err(e) } } };
    ListClone, jett_rt_v1_list_clone, false, (value: u64 => I64), u64 => I64,
        |s| s.clone_list(value);

    ParseInt, jett_rt_v1_parse_int, false, (value: u64 => I64), u64 => I64,
        |s| { let text = s.text(value)?; let parsed = text.parse::<i64>().map(|v| v as u64).map_err(|_| format!("int64.from_string: cannot parse '{text}' as int64")); s.parsed_sum(parsed) };
    ParseUint, jett_rt_v1_parse_uint, false, (value: u64 => I64), u64 => I64,
        |s| { let text = s.text(value)?; let parsed = text.parse::<u64>().map_err(|_| format!("uint64.from_string: cannot parse '{text}' as uint64")); s.parsed_sum(parsed) };
    ParseFloat, jett_rt_v1_parse_float, false, (value: u64 => I64), u64 => I64,
        |s| { let text = s.text(value)?; let parsed = text.parse::<f64>().map(f64::to_bits).map_err(|_| format!("float64.from_string: cannot parse '{text}' as float64")); s.parsed_sum(parsed) };
    FloatFromInt, jett_rt_v1_float64_from_int64, false, (value: i64 => I64), u64 => I64,
        |s| { let converted = value as f64;
            let parsed = if converted as i128 == i128::from(value) { Ok(converted.to_bits()) }
                else { Err("float64.from_int64: value is not exactly representable as float64".into()) };
            s.parsed_sum(parsed) };
    IntFromFloat, jett_rt_v1_int64_from_float64, false, (value: f64 => F64), u64 => I64,
        |s| { let parsed = if value.is_finite() && value.fract() == 0.0
                && value >= i64::MIN as f64 && value < 9_223_372_036_854_775_808.0 {
                Ok((value as i64) as u64)
            } else { Err("int64.from_float64: value is not exactly representable as int64".into()) };
            s.parsed_sum(parsed) };

    SumNew, jett_rt_v1_sum_new, false, (tag: u32 => I32, bits: u64 => I64, owned: u32 => I32), u64 => I64,
        |s| { if owned > 1 { return Err(INVALID_SUM); } s.sum(tag, bits, owned != 0) };
    SumTag, jett_rt_v1_sum_tag, false, (value: u64 => I64), u32 => I32,
        |s| s.sums.get(&value).map(|v| v.tag).ok_or(INVALID_SUM);
    SumTake, jett_rt_v1_sum_take, false, (value: u64 => I64, tag: u32 => I32), u64 => I64,
        |s| { if s.sums.get(&value).is_none_or(|v| v.tag != tag) { return Err(INVALID_SUM); }
            let sum = s.sums.remove(&value).ok_or(INVALID_SUM)?;
            s.sums_destroyed += 1; Ok(sum.bits) };
    SumClone, jett_rt_v1_sum_clone, false, (value: u64 => I64), u64 => I64,
        |s| { if !s.sums.contains_key(&value) { return Err(INVALID_SUM); } s.clone_value(value) };
    BytesGet, jett_rt_v1_bytes_get, false, (value: u64 => I64, index: i64 => I64), u64 => I64,
        |s| { let found = usize::try_from(index).ok().and_then(|i| s.bytes(value).ok()?.get(i)).copied();
            s.bytes(value)?;
            s.sum(u32::from(found.is_some()), found.unwrap_or(0) as u64, false) };
    BytesToString, jett_rt_v1_bytes_to_string, false, (value: u64 => I64), u64 => I64,
        |s| { let decoded = String::from_utf8(s.bytes(value)?.to_vec());
            let (tag, text) = match decoded { Ok(text) => (SUM_SUCCESS, text), Err(e) => (SUM_FAILURE, format!("invalid UTF-8: {e}")) };
            let payload = s.insert(text)?;
            match s.sum(tag, payload, true) { Ok(v) => Ok(v), Err(e) => { s.drop_value(payload)?; Err(e) } } };
    BytesFromHex, jett_rt_v1_bytes_from_hex, false, (value: u64 => I64), u64 => I64,
        |s| { let raw = s.text(value)?; let raw = raw.strip_prefix("0x").unwrap_or(raw);
            let decoded = if raw.len() % 2 != 0 { Err("bytes.from_hex: expected even-length hex string") }
                else if !raw.bytes().all(|b| b.is_ascii_hexdigit()) { Err("bytes.from_hex: expected hex string") }
                else { (0..raw.len()).step_by(2).map(|i| u8::from_str_radix(&raw[i..i+2], 16).map_err(|_| "bytes.from_hex: expected hex string")).collect::<Result<Vec<_>, _>>() };
            let (tag, payload) = match decoded { Ok(data) => (SUM_SUCCESS, s.insert_bytes(data)?), Err(error) => (SUM_FAILURE, s.insert(error.to_owned())?) };
            match s.sum(tag, payload, true) { Ok(v) => Ok(v), Err(e) => { s.drop_value(payload)?; Err(e) } } };

    EncodingBase64Encode, jett_rt_v1_encoding_base64_encode, false, (value: u64 => I64), u64 => I64,
        |s| { let encoded = encoding::base64_encode(s.bytes(value)?); s.insert(encoded) };
    EncodingBase64Decode, jett_rt_v1_encoding_base64_decode, false, (value: u64 => I64), u64 => I64,
        |s| { let decoded = encoding::base64_decode(s.text(value)?).map_err(str::to_owned); s.byte_result(decoded) };
    EncodingHexDecode, jett_rt_v1_encoding_hex_decode, false, (value: u64 => I64), u64 => I64,
        |s| { let decoded = encoding::encoding_hex_decode(s.text(value)?).map_err(str::to_owned); s.byte_result(decoded) };
    EncodingUrlEncode, jett_rt_v1_encoding_url_encode, false, (value: u64 => I64), u64 => I64,
        |s| { let encoded = encoding::percent_encode(s.text(value)?, false); s.insert(encoded) };
    EncodingUrlDecode, jett_rt_v1_encoding_url_decode, false, (value: u64 => I64), u64 => I64,
        |s| { let decoded = encoding::percent_decode(s.text(value)?, false).map_err(str::to_owned); s.string_result(decoded) };
    EncodingFormEncode, jett_rt_v1_encoding_form_encode, false, (value: u64 => I64), u64 => I64,
        |s| { let encoded = encoding::percent_encode(s.text(value)?, true); s.insert(encoded) };
    EncodingFormDecode, jett_rt_v1_encoding_form_decode, false, (value: u64 => I64), u64 => I64,
        |s| { let decoded = encoding::percent_decode(s.text(value)?, true).map_err(str::to_owned); s.string_result(decoded) };

    CsvParse, jett_rt_v1_csv_parse, false, (value: u64 => I64), u64 => I64,
        |s| { match csv::parse_csv_records(s.text(value)?) {
            Ok(rows) => { let payload = s.csv_rows(rows)?; s.owned_sum(SUM_SUCCESS, payload) },
            Err(error) => { let payload = s.insert(error)?; s.owned_sum(SUM_FAILURE, payload) }
        } };
    CsvParseWithHeader, jett_rt_v1_csv_parse_with_header, false, (value: u64 => I64), u64 => I64,
        |s| { match csv::parse_csv_with_header(s.text(value)?) {
            Ok(rows) => { let payload = s.csv_header_rows(rows)?; s.owned_sum(SUM_SUCCESS, payload) },
            Err(error) => { let payload = s.insert(error)?; s.owned_sum(SUM_FAILURE, payload) }
        } };
    CsvStringify, jett_rt_v1_csv_stringify, false, (value: u64 => I64), u64 => I64,
        |s| { let rows = s.csv_records(value)?; s.insert(csv::stringify_csv_records(&rows)) };

    SecretCompareString, jett_rt_v1_secret_compare_string, false, (left: u64 => I64, right: u64 => I64), u32 => I32,
        |s| { let left = s.text(left)?.as_bytes(); let right = s.text(right)?.as_bytes();
            Ok(u32::from(left.len() == right.len() && bool::from(left.ct_eq(right)))) };
    SecretCompareBytes, jett_rt_v1_secret_compare_bytes, false, (left: u64 => I64, right: u64 => I64), u32 => I32,
        |s| { let left = s.bytes(left)?; let right = s.bytes(right)?;
            Ok(u32::from(left.len() == right.len() && bool::from(left.ct_eq(right)))) };

    CryptoSha256, jett_rt_v1_crypto_sha256, false, (value: u64 => I64), u64 => I64,
        |s| { let digest = crypto::sha256_digest(s.bytes(value)?); s.insert_bytes(digest) };
    CryptoSha512, jett_rt_v1_crypto_sha512, false, (value: u64 => I64), u64 => I64,
        |s| { let digest = crypto::sha512_digest(s.bytes(value)?); s.insert_bytes(digest) };
    CryptoMd5, jett_rt_v1_crypto_md5, false, (value: u64 => I64), u64 => I64,
        |s| { let digest = crypto::md5_digest(s.bytes(value)?); s.insert_bytes(digest) };
    CryptoHmacSha256, jett_rt_v1_crypto_hmac_sha256, false, (key: u64 => I64, message: u64 => I64), u64 => I64,
        |s| { let digest = crypto::hmac_sha256_digest(s.bytes(key)?, s.bytes(message)?); s.insert_bytes(digest) };

    DropValue, jett_rt_v1_value_drop, true, (value: u64 => I64), u32 => I32,
        |s| s.drop_value(value);
    BytesNew, jett_rt_v1_bytes_new, false, (), u64 => I64,
        |s| s.insert_bytes(Vec::new());
    BitfieldWriteBits, jett_rt_v1_bitfield_write_bits, false, (value: u64 => I64, numeric: u64 => I64, width: u32 => I32, network_order: u32 => I32, bit_offset: u64 => I64), u32 => I32,
        |s| s.write_bitfield_bits(value, numeric, width, network_order, bit_offset);
    BitfieldExtendPayload, jett_rt_v1_bitfield_extend_payload, false, (value: u64 => I64, payload: u64 => I64), u32 => I32,
        |s| s.extend_bitfield_payload(value, payload);
    BitfieldDecode, jett_rt_v1_bitfield_decode, false, (value: u64 => I64, layout_pointer: u64 => I64, layout_length: u64 => I64), u64 => I64,
        |s| { if layout_pointer == 0 { return Err(INVALID_BITFIELD_LAYOUT); }
            let length = usize::try_from(layout_length).map_err(|_| INVALID_BITFIELD_LAYOUT)?;
            let layout = unsafe { std::slice::from_raw_parts(layout_pointer as *const u8, length) };
            s.decode_bitfield(value, layout) };
    BytesClone, jett_rt_v1_bytes_clone, false, (value: u64 => I64), u64 => I64,
        |s| { let data = s.bytes(value)?.to_vec(); s.insert_bytes(data) };
    BytesLength, jett_rt_v1_bytes_length, false, (value: u64 => I64), i64 => I64,
        |s| Ok(s.bytes(value)?.len() as i64);
    BytesFromString, jett_rt_v1_bytes_from_string, false, (value: u64 => I64), u64 => I64,
        |s| { let data = s.text(value)?.as_bytes().to_vec(); s.insert_bytes(data) };
    BytesSlice, jett_rt_v1_bytes_slice, false, (value: u64 => I64, start: i64 => I64, end: i64 => I64), u64 => I64,
        |s| { let data = s.bytes(value)?; let len = data.len() as i64;
            let start = start.clamp(0, len) as usize; let end = end.clamp(0, len) as usize;
            let result = data[start.min(end)..end].to_vec(); s.insert_bytes(result) };
    BytesConcat, jett_rt_v1_bytes_concat, false, (first: u64 => I64, second: u64 => I64), u64 => I64,
        |s| { let a = s.bytes(first)?; let b = s.bytes(second)?;
            let len = a.len().checked_add(b.len()).ok_or(EXHAUSTED)?;
            let mut result = Vec::new(); result.try_reserve_exact(len).map_err(|_| EXHAUSTED)?;
            result.extend_from_slice(a); result.extend_from_slice(b); s.insert_bytes(result) };
    BytesToHex, jett_rt_v1_bytes_to_hex, false, (value: u64 => I64), u64 => I64,
        |s| { use std::fmt::Write; let bytes = s.bytes(value)?;
            let mut text = String::new(); text.try_reserve_exact(bytes.len().checked_mul(2).ok_or(EXHAUSTED)?).map_err(|_| EXHAUSTED)?;
            for byte in bytes { write!(text, "{byte:02x}").map_err(|_| EXHAUSTED)?; }
            s.insert(text) };

    Sqrt, jett_rt_v1_math_sqrt, false, (value: f64 => F64), f64 => F64,
        |_| Ok(value.sqrt());
    Floor, jett_rt_v1_math_floor, false, (value: f64 => F64), f64 => F64,
        |_| Ok(value.floor());
    Ceil, jett_rt_v1_math_ceil, false, (value: f64 => F64), f64 => F64,
        |_| Ok(value.ceil());
    Round, jett_rt_v1_math_round, false, (value: f64 => F64), f64 => F64,
        |_| Ok(value.round());
    Log, jett_rt_v1_math_log, false, (value: f64 => F64), f64 => F64,
        |_| Ok(value.ln());
    Log2, jett_rt_v1_math_log2, false, (value: f64 => F64), f64 => F64,
        |_| Ok(value.log2());
    Log10, jett_rt_v1_math_log10, false, (value: f64 => F64), f64 => F64,
        |_| Ok(value.log10());
    Sin, jett_rt_v1_math_sin, false, (value: f64 => F64), f64 => F64,
        |_| Ok(value.sin());
    Cos, jett_rt_v1_math_cos, false, (value: f64 => F64), f64 => F64,
        |_| Ok(value.cos());
    Tan, jett_rt_v1_math_tan, false, (value: f64 => F64), f64 => F64,
        |_| Ok(value.tan());
    FloatAbs, jett_rt_v1_math_floatabs, false, (value: f64 => F64), f64 => F64,
        |_| Ok(value.abs());
    FloatMin, jett_rt_v1_math_floatmin, false, (first: f64 => F64, second: f64 => F64), f64 => F64,
        |_| Ok(first.min(second));
    FloatMax, jett_rt_v1_math_floatmax, false, (first: f64 => F64, second: f64 => F64), f64 => F64,
        |_| Ok(first.max(second));
    Pow, jett_rt_v1_math_pow, false, (first: f64 => F64, second: f64 => F64), f64 => F64,
        |_| Ok(first.powf(second));
    Pi, jett_rt_v1_math_pi, false, (), f64 => F64,
        |_| Ok(std::f64::consts::PI);
    E, jett_rt_v1_math_e, false, (), f64 => F64,
        |_| Ok(std::f64::consts::E);
    IntAbs, jett_rt_v1_math_int_abs, false, (value: i64 => I64), i64 => I64,
        |_| Ok(value.wrapping_abs());
    IntMin, jett_rt_v1_math_int_min, false, (first: i64 => I64, second: i64 => I64), i64 => I64,
        |_| Ok(first.min(second));
    IntMax, jett_rt_v1_math_int_max, false, (first: i64 => I64, second: i64 => I64), i64 => I64,
        |_| Ok(first.max(second));
    Mod, jett_rt_v1_math_mod, false, (first: i64 => I64, second: i64 => I64), i64 => I64,
        |_| if second == 0 { Err((JettRuntimeStatusV1::INVALID_ARGUMENT, b"math.mod: division by zero")) } else { Ok(first.wrapping_rem(second)) };
    Gcd, jett_rt_v1_math_gcd, false, (first: i64 => I64, second: i64 => I64), i64 => I64,
        |_| Ok(unsigned_gcd(first.unsigned_abs(), second.unsigned_abs()) as i64);
    Lcm, jett_rt_v1_math_lcm, false, (first: i64 => I64, second: i64 => I64), i64 => I64,
        |_| {
            let a = first.unsigned_abs(); let b = second.unsigned_abs();
            if a == 0 || b == 0 { Ok(0) } else { Ok((a / unsigned_gcd(a,b)).wrapping_mul(b) as i64) }
        };
    Factorial, jett_rt_v1_math_factorial, false, (value: i64 => I64), i64 => I64,
        |_| {
            if value < 0 { return Err((JettRuntimeStatusV1::INVALID_ARGUMENT, b"math.factorial: argument must be non-negative")); }
            let mut result = 1_i64;
            for n in 2..=value { result = result.wrapping_mul(n); if result == 0 { break; } }
            Ok(result)
        };
    Clamp, jett_rt_v1_math_clamp, false, (value: f64 => F64, lower: f64 => F64, upper: f64 => F64), f64 => F64,
        |_| {
            if lower.is_nan() || upper.is_nan() { Err((JettRuntimeStatusV1::INVALID_ARGUMENT, b"math.clamp bounds must not be NaN")) }
            else if lower > upper { Err((JettRuntimeStatusV1::INVALID_ARGUMENT, b"math.clamp requires lower bound <= upper bound")) }
            else { Ok(value.clamp(lower, upper)) }
        };
    Status, jett_rt_v1_value_status, true, (), u32 => I32,
        |s| Ok(s.failure.map_or(0, |e| e.0.code()));
    Retain, jett_rt_v1_string_retain, false, (value: u64 => I64), u64 => I64,
        |s| s.retain(value);
    Release, jett_rt_v1_string_release, true, (value: u64 => I64), u32 => I32,
        |s| s.release(value);
    Literal, jett_rt_v1_string_literal, false, (data: *const u8 => Pointer, length: u64 => I64), u64 => I64,
        |s| {
            if length > isize::MAX as u64 { return Err((JettRuntimeStatusV1::LENGTH_OUT_OF_RANGE, STRING_LENGTH_MESSAGE)); }
            if length != 0 && data.is_null() { return Err((JettRuntimeStatusV1::INVALID_ARGUMENT, STRING_NULL_MESSAGE)); }
            let bytes = if length == 0 { &[] } else { unsafe { slice::from_raw_parts(data, length as usize) } };
            let text = str::from_utf8(bytes).map_err(|_| (JettRuntimeStatusV1::INVALID_UTF8, STRING_UTF8_MESSAGE))?;
            s.insert(text.to_owned())
        };
    Concat, jett_rt_v1_string_concat, false, (left: u64 => I64, right: u64 => I64), u64 => I64,
        |s| {
            let a = s.text(left)?; let b = s.text(right)?;
            let length = a.len().checked_add(b.len()).ok_or(EXHAUSTED)?;
            let mut text = String::new(); text.try_reserve_exact(length).map_err(|_| EXHAUSTED)?;
            text.push_str(a); text.push_str(b); s.insert(text)
        };
    CharCount, jett_rt_v1_string_char_count, false, (value: u64 => I64), u64 => I64,
        |s| Ok(s.text(value)?.graphemes(true).count() as u64);
    StringIndexOf, jett_rt_v1_string_index_of, false, (value: u64 => I64, needle: u64 => I64), u64 => I64,
        |s| {
            let mut index = if s.text(needle)?.is_empty() { Some(0) } else { None };
            native_scan_grapheme_matches(s.text(value)?, s.text(needle)?, |_, _, offset| {
                index = Some(offset);
                false
            });
            match index {
                Some(offset) => s.sum(SUM_SUCCESS, offset as u64, false),
                None => s.sum(SUM_FAILURE, 0, false),
            }
        };
    StringCount, jett_rt_v1_string_count, false, (value: u64 => I64, needle: u64 => I64), u64 => I64,
        |s| {
            let mut count = 0_u64;
            native_scan_grapheme_matches(s.text(value)?, s.text(needle)?, |_, _, _| {
                count += 1;
                true
            });
            Ok(count)
        };
    StringReplace, jett_rt_v1_string_replace, false, (value: u64 => I64, needle: u64 => I64, replacement: u64 => I64), u64 => I64,
        |s| { let output = native_replace(s.text(value)?, s.text(needle)?, s.text(replacement)?)?; s.insert(output) };
    StringSlugify, jett_rt_v1_string_slugify, false, (value: u64 => I64), u64 => I64,
        |s| { let output = native_slugify(s.text(value)?); s.insert(output) };
    StringToUpperFirst, jett_rt_v1_string_to_upper_first, false, (value: u64 => I64), u64 => I64,
        |s| { let output = native_change_first(s.text(value)?, |first| first.to_uppercase().collect()); s.insert(output) };
    StringToLowerFirst, jett_rt_v1_string_to_lower_first, false, (value: u64 => I64), u64 => I64,
        |s| { let output = native_change_first(s.text(value)?, |first| first.to_lowercase().collect()); s.insert(output) };
    Slice, jett_rt_v1_string_slice, false, (value: u64 => I64, start: i64 => I64, end: i64 => I64), u64 => I64,
        |s| {
            let parts = s.text(value)?.graphemes(true).collect::<Vec<_>>();
            let length = parts.len() as i64;
            let start = start.clamp(0, length) as usize;
            let end = end.clamp(0, length) as usize;
            let text = parts[start.min(end)..end].concat();
            s.insert(text)
        };
    Upper, jett_rt_v1_string_upper, false, (value: u64 => I64), u64 => I64,
        |s| { let text = s.text(value)?.to_uppercase(); s.insert(text) };
    Lower, jett_rt_v1_string_lower, false, (value: u64 => I64), u64 => I64,
        |s| { let text = s.text(value)?.to_lowercase(); s.insert(text) };
    Trim, jett_rt_v1_string_trim, false, (value: u64 => I64), u64 => I64,
        |s| { let text = s.text(value)?.trim().to_owned(); s.insert(text) };
    TrimStart, jett_rt_v1_string_trim_start, false, (value: u64 => I64), u64 => I64,
        |s| { let text = s.text(value)?.trim_start().to_owned(); s.insert(text) };
    TrimEnd, jett_rt_v1_string_trim_end, false, (value: u64 => I64), u64 => I64,
        |s| { let text = s.text(value)?.trim_end().to_owned(); s.insert(text) };
    IsAlpha, jett_rt_v1_string_is_alpha, false, (value: u64 => I64), u32 => I32,
        |s| { let text = s.text(value)?; Ok(u32::from(!text.is_empty() && text.chars().all(char::is_alphabetic))) };
    IsNumeric, jett_rt_v1_string_is_numeric, false, (value: u64 => I64), u32 => I32,
        |s| { let text = s.text(value)?; Ok(u32::from(!text.is_empty() && text.chars().all(|c| c.is_ascii_digit()))) };
    Repeat, jett_rt_v1_string_repeat, false, (value: u64 => I64, count: i64 => I64), u64 => I64,
        |s| {
            let text = s.text(value)?;
            let count = usize::try_from(count.max(0)).unwrap_or(usize::MAX);
            if text.is_empty() || count == 0 { return s.insert(String::new()); }
            let error = (JettRuntimeStatusV1::RESOURCE_EXHAUSTED, b"string.repeat: requested output is too large".as_slice());
            let length = text.len().checked_mul(count).ok_or(error)?;
            let mut result = String::new(); result.try_reserve_exact(length).map_err(|_| error)?;
            for _ in 0..count { result.push_str(text); }
            s.insert(result)
        };
    Equal, jett_rt_v1_string_equal, false, (left: u64 => I64, right: u64 => I64), u32 => I32,
        |s| Ok(u32::from(s.text(left)? == s.text(right)?));
    EnumEqual, jett_rt_v1_enum_equal, false, (left: u64 => I64, right: u64 => I64, layout_pointer: u64 => I64, layout_length: u64 => I64), u32 => I32,
        |s| { if layout_pointer == 0 { return Err(INVALID_STRUCT); }
            let length = usize::try_from(layout_length).map_err(|_| INVALID_STRUCT)?;
            if length > isize::MAX as usize { return Err(INVALID_STRUCT); }
            let layout = unsafe { std::slice::from_raw_parts(layout_pointer as *const u8, length) };
            s.enum_equal(left, right, layout) };
    FromInt, jett_rt_v1_string_from_int, false, (value: i64 => I64), u64 => I64,
        |s| s.insert(value.to_string());
    FromUint, jett_rt_v1_string_from_uint, false, (value: u64 => I64), u64 => I64,
        |s| s.insert(value.to_string());
    FromFloat, jett_rt_v1_string_from_float, false, (value: f64 => F64), u64 => I64,
        |s| s.insert(value.to_string());
    FromBool, jett_rt_v1_string_from_bool, false, (value: u32 => I32), u64 => I64,
        |s| if value > 1 { Err((JettRuntimeStatusV1::INVALID_ARGUMENT, b"invalid native bool")) } else { s.insert((value != 0).to_string()) };
    GrantStdout, jett_rt_v1_grant_stdout, false, (), u64 => I64,
        |s| { if let Some(token) = s.stdout { return Ok(token); } let token = next_identity()?; s.stdout = Some(token); Ok(token) };
    GrantClock, jett_rt_v1_grant_clock, false, (), u64 => I64,
        |s| { if let Some(token) = s.clock { return Ok(token); } let token = next_identity()?; s.clock = Some(token); Ok(token) };
    ClockNow, jett_rt_v1_clock_now, false, (authority: u64 => I64), i64 => I64,
        |s| { if s.clock != Some(authority) { return Err((JettRuntimeStatusV1::INVALID_ARGUMENT, b"invalid Clock authority")); }
            if let Some(script) = &mut s.clock_script {
                let sample = script.front().copied().ok_or((JettRuntimeStatusV1::INVALID_ARGUMENT, b"Clock.now: test clock exhausted".as_slice()))?;
                match sample {
                    clock::ClockTestSample::Unavailable => {
                        script.pop_front();
                        Err((JettRuntimeStatusV1::INVALID_ARGUMENT, b"Clock.now: wall clock unavailable"))
                    }
                    clock::ClockTestSample::Wall { unix_seconds, subsecond_nanoseconds } => {
                        let value = clock::checked_clock_milliseconds(unix_seconds, subsecond_nanoseconds)
                            .map_err(|message| (JettRuntimeStatusV1::INVALID_ARGUMENT, message.as_bytes()))?;
                        script.pop_front();
                        Ok(value)
                    }
                }
            } else {
                let (seconds, nanoseconds) = clock::production_wall_clock_sample();
                clock::checked_clock_milliseconds(seconds, nanoseconds).map_err(|message| (JettRuntimeStatusV1::INVALID_ARGUMENT, message.as_bytes()))
            } };
    GrantRandom, jett_rt_v1_grant_random, false, (), u64 => I64,
        |s| { if let Some(token) = s.random { return Ok(token); }
            if s.random_provider.is_none() {
                s.random_provider = Some(RandomProvider::production().map_err(|message| (JettRuntimeStatusV1::INVALID_ARGUMENT, message.as_bytes()))?);
            }
            let token = next_identity()?; s.random = Some(token); Ok(token) };
    RandomBounded, jett_rt_v1_random_bounded, false, (authority: u64 => I64, lower: i64 => I64, upper: i64 => I64), i64 => I64,
        |s| { if s.random != Some(authority) { return Err((JettRuntimeStatusV1::INVALID_ARGUMENT, b"invalid Random authority")); }
            let width = u64::try_from(i128::from(upper) - i128::from(lower))
                .ok().filter(|width| *width > 0)
                .ok_or((JettRuntimeStatusV1::INVALID_ARGUMENT, b"random.__bounded received invalid bounds".as_slice()))?;
            let offset = s.random_provider.as_mut().ok_or((JettRuntimeStatusV1::INVALID_ARGUMENT, random::ENTROPY_UNAVAILABLE.as_bytes()))?
                .bounded_offset(width).map_err(|message| (JettRuntimeStatusV1::INVALID_ARGUMENT, message.as_bytes()))?;
            i64::try_from(i128::from(lower) + i128::from(offset))
                .map_err(|_| (JettRuntimeStatusV1::INVALID_ARGUMENT, b"random.__bounded received invalid bounds".as_slice())) };
    RandomUnit53, jett_rt_v1_random_unit53, false, (authority: u64 => I64), f64 => F64,
        |s| { if s.random != Some(authority) { return Err((JettRuntimeStatusV1::INVALID_ARGUMENT, b"invalid Random authority")); }
            let bits = s.random_provider.as_mut().ok_or((JettRuntimeStatusV1::INVALID_ARGUMENT, random::ENTROPY_UNAVAILABLE.as_bytes()))?
                .unit53().map_err(|message| (JettRuntimeStatusV1::INVALID_ARGUMENT, message.as_bytes()))?;
            Ok(bits as f64 / 9_007_199_254_740_992.0) };
    RandomBool, jett_rt_v1_random_bool, false, (authority: u64 => I64), u32 => I32,
        |s| { if s.random != Some(authority) { return Err((JettRuntimeStatusV1::INVALID_ARGUMENT, b"invalid Random authority")); }
            let value = s.random_provider.as_mut().ok_or((JettRuntimeStatusV1::INVALID_ARGUMENT, random::ENTROPY_UNAVAILABLE.as_bytes()))?
                .boolean().map_err(|message| (JettRuntimeStatusV1::INVALID_ARGUMENT, message.as_bytes()))?;
            Ok(u32::from(value)) };
    UuidNew, jett_rt_v1_uuid_new, false, (), u64 => I64,
        |s| { let value = crate::uuid::new_v4()
            .map_err(|message| (JettRuntimeStatusV1::INVALID_ARGUMENT, message.as_bytes()))?;
            s.insert(value) };
    GrantEnvironment, jett_rt_v1_grant_environment, false, (), u64 => I64,
        |s| { if let Some(token) = s.environment { return Ok(token); }
            if s.environment_snapshot.is_none() {
                s.environment_snapshot = Some(LaunchEnvironmentSnapshot::production()
                    .map_err(|message| (JettRuntimeStatusV1::INVALID_ARGUMENT, message.as_bytes()))?);
            }
            let token = next_identity()?; s.environment = Some(token); Ok(token) };
    GrantGraphics, jett_rt_v1_grant_graphics, false, (), u64 => I64,
        |s| { if let Some(token) = s.graphics { return Ok(token); }
            let token = next_identity()?; s.graphics = Some(token); Ok(token) };
    EnvironmentArgs, jett_rt_v1_environment_args, false, (authority: u64 => I64), u64 => I64,
        |s| { if s.environment != Some(authority) { return Err((JettRuntimeStatusV1::INVALID_ARGUMENT, b"invalid Environment authority")); }
            let arguments = s.environment_snapshot.as_ref().ok_or((JettRuntimeStatusV1::INVALID_ARGUMENT, b"Environment: launch data unavailable".as_slice()))?
                .arguments().to_vec();
            s.string_list(arguments) };
    EnvironmentGet, jett_rt_v1_environment_get, false, (authority: u64 => I64, key: u64 => I64), u64 => I64,
        |s| { if s.environment != Some(authority) { return Err((JettRuntimeStatusV1::INVALID_ARGUMENT, b"invalid Environment authority")); }
            let found = s.environment_snapshot.as_ref().ok_or((JettRuntimeStatusV1::INVALID_ARGUMENT, b"Environment: launch data unavailable".as_slice()))?
                .get(s.text(key)?);
            match found {
                Ok(Some(value)) => { let text = s.insert(value)?;
                    let optional = s.owned_sum(SUM_SUCCESS, text)?;
                    s.owned_sum(SUM_SUCCESS, optional) },
                Ok(None) => { let optional = s.sum(SUM_FAILURE, 0, false)?;
                    s.owned_sum(SUM_SUCCESS, optional) },
                Err(message) => { let text = s.insert(message.to_owned())?;
                    s.owned_sum(SUM_FAILURE, text) },
            } };
    Stdout, jett_rt_v1_string_stdout, false, (authority: u64 => I64, value: u64 => I64), u32 => I32,
        |s| {
            if s.stdout != Some(authority) { return Err((JettRuntimeStatusV1::INVALID_ARGUMENT, b"invalid Stdout authority")); }
            { let mut stdout = io::stdout().lock(); write_all_bytes(&mut stdout, s.text(value)?.as_bytes()).and_then(|_| stdout.flush()).map_err(|_| (JettRuntimeStatusV1::IO_FAILURE, STDOUT_WRITE_MESSAGE))?; } Ok(0)
        };
    DebugPrint, jett_rt_v1_string_debug_print, false, (value: u64 => I64), u32 => I32,
        |s| { { let mut stdout = io::stdout().lock(); write_all_bytes(&mut stdout, s.text(value)?.as_bytes()).and_then(|_| stdout.flush()).map_err(|_| (JettRuntimeStatusV1::IO_FAILURE, STDOUT_WRITE_MESSAGE))?; } Ok(0) };
    TraceInt64, jett_rt_v1_trace_int64, false, (prefix_pointer: u64 => I64, prefix_length: u64 => I64, value: i64 => I64), u32 => I32,
        |_s| { if prefix_pointer == 0 { return Err(INVALID_TRACE_LABEL); }
            let length = usize::try_from(prefix_length).map_err(|_| INVALID_TRACE_LABEL)?;
            let prefix = unsafe { std::slice::from_raw_parts(prefix_pointer as *const u8, length) };
            let mut stderr = io::stderr().lock();
            write_all_bytes(&mut stderr, prefix)
                .and_then(|_| write_all_bytes(&mut stderr, value.to_string().as_bytes()))
                .and_then(|_| write_all_bytes(&mut stderr, b"\n"))
                .and_then(|_| stderr.flush())
                .map_err(|_| (JettRuntimeStatusV1::IO_FAILURE, STDERR_WRITE_MESSAGE))?;
            Ok(0) };
    BreakpointEmpty, jett_rt_v1_breakpoint_empty, false, (condition: u32 => I32), u32 => I32,
        |_s| { if condition != 0 {
            let mut stderr = io::stderr().lock();
            write_all_bytes(&mut stderr, b"breakpoint hit\n")
                .and_then(|_| stderr.flush())
                .map_err(|_| (JettRuntimeStatusV1::IO_FAILURE, STDERR_WRITE_MESSAGE))?;
        } Ok(0) };
    BreakpointInt64, jett_rt_v1_breakpoint_int64, false, (condition: u32 => I32, prefix_pointer: u64 => I64, prefix_length: u64 => I64, value: i64 => I64), u32 => I32,
        |_s| { if condition != 0 {
            if prefix_pointer == 0 { return Err(INVALID_TRACE_LABEL); }
            let length = usize::try_from(prefix_length).map_err(|_| INVALID_TRACE_LABEL)?;
            let prefix = unsafe { std::slice::from_raw_parts(prefix_pointer as *const u8, length) };
            let mut stderr = io::stderr().lock();
            write_all_bytes(&mut stderr, prefix)
                .and_then(|_| write_all_bytes(&mut stderr, value.to_string().as_bytes()))
                .and_then(|_| write_all_bytes(&mut stderr, b"\n"))
                .and_then(|_| stderr.flush())
                .map_err(|_| (JettRuntimeStatusV1::IO_FAILURE, STDERR_WRITE_MESSAGE))?;
        } Ok(0) };
}

/// Read the first terminal failure without clearing it. Static message storage
/// remains valid after context destruction, like all v1 result messages.
/// # Safety
/// Same context and result pointer requirements as the lifecycle ABI.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jett_rt_v1_value_failure(
    context: *const JettRuntimeContextV1,
    out: *mut JettRuntimeResultV1,
) -> JettRuntimeStatusV1 {
    complete_call(out, || {
        let key = match context_key(context) {
            Ok(k) => k,
            Err(status) => return JettRuntimeResultV1::failure(status, CONTEXT_INVALID_MESSAGE),
        };
        let lease = match acquire_context(key) {
            Ok(l) => l,
            Err(_) => {
                return JettRuntimeResultV1::failure(
                    JettRuntimeStatusV1::INVALID_CONTEXT,
                    CONTEXT_INVALID_MESSAGE,
                );
            }
        };
        let state = lock_unpoisoned(&lease.entry.state);
        match state.as_ref().and_then(|s| s.values.failure) {
            Some((status, message)) => JettRuntimeResultV1::failure(status, message),
            None => JettRuntimeResultV1::ok(),
        }
    })
}

fn unsigned_gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::MaybeUninit;
    struct Context(Box<JettRuntimeContextV1>);
    impl Context {
        fn new() -> Self {
            let mut value = Box::new(JettRuntimeContextV1::retired());
            let mut result = MaybeUninit::uninit();
            assert_eq!(
                unsafe { jett_rt_v1_context_create(1, &mut *value, result.as_mut_ptr()) },
                JettRuntimeStatusV1::OK
            );
            Self(value)
        }
        fn destroy(self, expected: JettRuntimeStatusV1) {
            let mut context = std::mem::ManuallyDrop::new(self);
            let mut result = MaybeUninit::uninit();
            assert_eq!(
                unsafe { jett_rt_v1_context_destroy(&mut *context.0, result.as_mut_ptr()) },
                expected
            );
            unsafe {
                drop(ptr::read(&context.0));
            }
        }
        fn pointer(&self) -> *const JettRuntimeContextV1 {
            &*self.0
        }
        fn text(&self, text: &str) -> u64 {
            unsafe { jett_rt_v1_string_literal(self.pointer(), text.as_ptr(), text.len() as u64) }
        }
        fn count(&self) -> usize {
            let lease = acquire_context(context_key(self.pointer()).unwrap()).unwrap();
            lock_unpoisoned(&lease.entry.state)
                .as_ref()
                .unwrap()
                .values
                .strings
                .len()
        }
    }
    impl Drop for Context {
        fn drop(&mut self) {
            let mut result = MaybeUninit::uninit();
            assert_eq!(
                unsafe { jett_rt_v1_context_destroy(&mut *self.0, result.as_mut_ptr()) },
                JettRuntimeStatusV1::OK
            );
        }
    }
    #[test]
    fn strings_release_immediately_and_retain_preserves_aliases() {
        let context = Context::new();
        let value = context.text("hé\0llo");
        assert_ne!(value, 0);
        assert_eq!(context.count(), 1);
        unsafe {
            assert_eq!(jett_rt_v1_string_retain(context.pointer(), value), value);
            assert_eq!(jett_rt_v1_string_release(context.pointer(), value), 0);
            assert_eq!(context.count(), 1);
            assert_eq!(jett_rt_v1_string_equal(context.pointer(), value, value), 1);
            assert_eq!(jett_rt_v1_string_release(context.pointer(), value), 0);
            assert_eq!(context.count(), 0);
            assert_eq!(jett_rt_v1_string_release(context.pointer(), 0), 0);
            assert_eq!(jett_rt_v1_value_status(context.pointer()), 0);
        }
    }
    #[test]
    fn foreign_and_stale_handles_fail_but_cleanup_remains_available() {
        let first = Context::new();
        let second = Context::new();
        let a = first.text("first");
        let b = second.text("second");
        unsafe {
            assert_eq!(jett_rt_v1_string_retain(second.pointer(), a), 0);
            assert_ne!(jett_rt_v1_value_status(second.pointer()), 0);
            assert_eq!(
                jett_rt_v1_string_literal(second.pointer(), b"no".as_ptr(), 2),
                0
            );
            assert_eq!(jett_rt_v1_string_release(second.pointer(), b), 0);
            assert_eq!(second.count(), 0);
            assert_eq!(jett_rt_v1_string_release(first.pointer(), a), 0);
            assert_eq!(jett_rt_v1_string_retain(first.pointer(), a), 0);
            assert_ne!(jett_rt_v1_value_status(first.pointer()), 0);
            assert_eq!(first.count(), 0);
            assert_ne!(jett_rt_v1_value_status(ptr::null()), 0);
        }
    }
    #[test]
    fn stdout_authority_is_context_bound_and_failure_is_first_wins() {
        let first = Context::new();
        let second = Context::new();
        let empty = second.text("");
        unsafe {
            let token = jett_rt_v1_grant_stdout(first.pointer());
            let other = jett_rt_v1_grant_stdout(second.pointer());
            assert_ne!(token, other);
            assert_eq!(jett_rt_v1_string_stdout(second.pointer(), other, empty), 0);
            jett_rt_v1_string_stdout(second.pointer(), token, empty);
            jett_rt_v1_string_release(second.pointer(), u64::MAX);
            let mut failure = MaybeUninit::uninit();
            assert_ne!(
                jett_rt_v1_value_failure(second.pointer(), failure.as_mut_ptr()),
                JettRuntimeStatusV1::OK
            );
            let failure = failure.assume_init();
            assert_eq!(
                slice::from_raw_parts(failure.message.data, failure.message.byte_length as usize),
                b"invalid Stdout authority"
            );
            assert_eq!(jett_rt_v1_string_release(second.pointer(), empty), 0);
        }
        second.destroy(JettRuntimeStatusV1::INVALID_ARGUMENT);
    }
    #[test]
    fn clock_authority_is_context_bound_and_samples_wall_time() {
        let first = Context::new();
        let second = Context::new();
        unsafe {
            let token = jett_rt_v1_grant_clock(first.pointer());
            let other = jett_rt_v1_grant_clock(second.pointer());
            assert_ne!(token, other);
            assert_eq!(token, jett_rt_v1_grant_clock(first.pointer()));
            let before = clock::production_wall_clock_sample();
            let now = jett_rt_v1_clock_now(first.pointer(), token);
            let after = clock::production_wall_clock_sample();
            let before = clock::checked_clock_milliseconds(before.0, before.1).unwrap();
            let after = clock::checked_clock_milliseconds(after.0, after.1).unwrap();
            assert!((before..=after).contains(&now));
            assert_eq!(jett_rt_v1_value_status(first.pointer()), 0);
            assert_eq!(jett_rt_v1_clock_now(second.pointer(), token), 0);
            let lease = acquire_context(context_key(second.pointer()).unwrap()).unwrap();
            let state = lock_unpoisoned(&lease.entry.state);
            assert_eq!(
                state.as_ref().unwrap().values.failure,
                Some((
                    JettRuntimeStatusV1::INVALID_ARGUMENT,
                    b"invalid Clock authority".as_slice()
                ))
            );
        }
    }
    #[test]
    fn scripted_clock_samples_are_context_bound_and_exhaustion_is_terminal() {
        let first = Context::new();
        let second = Context::new();
        let samples = [
            clock::ClockTestSample::Wall {
                unix_seconds: -1,
                subsecond_nanoseconds: 999_999_999,
            },
            clock::ClockTestSample::Wall {
                unix_seconds: 42,
                subsecond_nanoseconds: 123_456_789,
            },
        ];
        let script = clock::encode_test_script(&samples);
        unsafe {
            assert_eq!(
                jett_rt_v1_clock_configure_scripted(
                    first.pointer(),
                    script.as_ptr(),
                    script.len() as u64
                ),
                0
            );
            let token = jett_rt_v1_grant_clock(first.pointer());
            let foreign = jett_rt_v1_grant_clock(second.pointer());
            assert_eq!(jett_rt_v1_clock_now(first.pointer(), token), -1);
            assert_eq!(jett_rt_v1_clock_now(first.pointer(), token), 42_123);
            assert_eq!(jett_rt_v1_clock_now(first.pointer(), token), 0);
            assert_ne!(jett_rt_v1_value_status(first.pointer()), 0);
            let _ = jett_rt_v1_clock_now(second.pointer(), foreign);
            assert_eq!(jett_rt_v1_value_status(second.pointer()), 0);
            let lease = acquire_context(context_key(first.pointer()).unwrap()).unwrap();
            let state = lock_unpoisoned(&lease.entry.state);
            assert_eq!(
                state.as_ref().unwrap().values.failure,
                Some((
                    JettRuntimeStatusV1::INVALID_ARGUMENT,
                    b"Clock.now: test clock exhausted".as_slice()
                ))
            );
        }
    }
    #[test]
    fn scripted_random_samples_match_scalar_kernels_and_exhaustion() {
        let context = Context::new();
        let samples = [
            random::RandomTestSample::Bounded(u64::MAX - 1),
            random::RandomTestSample::Unit53((1_u64 << 53) - 1),
            random::RandomTestSample::Boolean(true),
        ];
        let script = random::encode_test_script(&samples);
        unsafe {
            assert_eq!(
                jett_rt_v1_random_configure_scripted(
                    context.pointer(),
                    script.as_ptr(),
                    script.len() as u64,
                ),
                0
            );
            let token = jett_rt_v1_grant_random(context.pointer());
            assert_ne!(token, 0);
            assert_eq!(
                jett_rt_v1_random_bounded(context.pointer(), token, i64::MIN, i64::MAX),
                i64::MAX - 1
            );
            assert_eq!(
                jett_rt_v1_random_unit53(context.pointer(), token),
                0.9999999999999999
            );
            assert_eq!(jett_rt_v1_random_bool(context.pointer(), token), 1);
            assert_eq!(
                jett_rt_v1_random_bool(context.pointer(), token),
                JettRuntimeStatusV1::INVALID_CONTEXT.code()
            );
            let lease = acquire_context(context_key(context.pointer()).unwrap()).unwrap();
            let state = lock_unpoisoned(&lease.entry.state);
            assert_eq!(
                state.as_ref().unwrap().values.failure,
                Some((
                    JettRuntimeStatusV1::INVALID_ARGUMENT,
                    random::TEST_EXHAUSTED.as_bytes()
                ))
            );
        }
    }
    #[test]
    fn context_destruction_reports_owned_value_leaks() {
        let mut context = Context::new();
        context.text("unreleased");
        let mut result = MaybeUninit::uninit();
        assert_eq!(
            unsafe { jett_rt_v1_context_destroy(&mut *context.0, result.as_mut_ptr()) },
            JettRuntimeStatusV1::INVALID_ARGUMENT
        );
        // The context has already retired; drop its storage without a second destroy.
        let context = std::mem::ManuallyDrop::new(context);
        unsafe {
            drop(ptr::read(&context.0));
        }
    }
    #[test]
    fn panic_becomes_terminal_failure_and_releases_existing_values() {
        let context = Context::new();
        let value = context.text("held");
        let result: u64 = leaf(context.pointer(), false, |_| panic!("injected leaf panic"));
        assert_eq!(result, 0);
        unsafe {
            assert_eq!(
                jett_rt_v1_value_status(context.pointer()),
                JettRuntimeStatusV1::PANIC.code()
            );
            jett_rt_v1_string_release(context.pointer(), value);
        }
        assert_eq!(context.count(), 0);
    }
    #[test]
    fn bytes_have_distinct_storage_and_exact_destruction_counts() {
        let context = Context::new();
        let text = context.text("raw");
        unsafe {
            let a = jett_rt_v1_bytes_from_string(context.pointer(), text);
            let b = jett_rt_v1_bytes_clone(context.pointer(), a);
            assert_ne!(a, b);
            let lease = acquire_context(context_key(context.pointer()).unwrap()).unwrap();
            {
                let mut state = lock_unpoisoned(&lease.entry.state);
                let values = &mut state.as_mut().unwrap().values;
                values.bytes.get_mut(&a).unwrap()[0] = 255;
                assert_eq!(values.bytes(a).unwrap(), &[255, 97, 119]);
                assert_eq!(values.bytes(b).unwrap(), b"raw");
                assert_eq!((values.bytes_created, values.bytes_destroyed), (2, 0));
            }
            jett_rt_v1_value_drop(context.pointer(), a);
            jett_rt_v1_value_drop(context.pointer(), b);
            jett_rt_v1_value_drop(context.pointer(), text);
            let state = lock_unpoisoned(&lease.entry.state);
            let values = &state.as_ref().unwrap().values;
            assert_eq!((values.bytes_created, values.bytes_destroyed), (2, 2));
            assert!(values.is_empty());
        }
    }
    #[test]
    fn bytes_registry_leak_is_not_hidden_by_empty_string_registry() {
        let mut context = Context::new();
        unsafe {
            jett_rt_v1_bytes_new(context.pointer());
        }
        assert_eq!(context.count(), 0);
        let mut result = MaybeUninit::uninit();
        assert_eq!(
            unsafe { jett_rt_v1_context_destroy(&mut *context.0, result.as_mut_ptr()) },
            JettRuntimeStatusV1::INVALID_ARGUMENT
        );
        let context = std::mem::ManuallyDrop::new(context);
        unsafe {
            drop(ptr::read(&context.0));
        }
    }
    #[test]
    fn sum_tags_take_only_selected_payload_and_cleanup_nested_owners() {
        let context = Context::new();
        unsafe {
            let data = jett_rt_v1_bytes_new(context.pointer());
            let inner = jett_rt_v1_sum_new(context.pointer(), SUM_SUCCESS, data, 1);
            let outer = jett_rt_v1_sum_new(context.pointer(), SUM_SUCCESS, inner, 1);
            let clone = jett_rt_v1_sum_clone(context.pointer(), outer);
            assert_ne!(clone, outer);
            assert_eq!(jett_rt_v1_sum_tag(context.pointer(), outer), SUM_SUCCESS);
            let taken = jett_rt_v1_sum_take(context.pointer(), outer, SUM_SUCCESS);
            assert_eq!(taken, inner);
            jett_rt_v1_value_drop(context.pointer(), taken);
            // Invalid tag extraction must not consume the still-owned record.
            assert_eq!(
                jett_rt_v1_sum_take(context.pointer(), clone, SUM_FAILURE),
                0
            );
            assert_ne!(jett_rt_v1_value_status(context.pointer()), 0);
            jett_rt_v1_value_drop(context.pointer(), clone);
            let lease = acquire_context(context_key(context.pointer()).unwrap()).unwrap();
            let state = lock_unpoisoned(&lease.entry.state);
            let values = &state.as_ref().unwrap().values;
            assert_eq!((values.bytes_created, values.bytes_destroyed), (2, 2));
            assert_eq!((values.sums_created, values.sums_destroyed), (4, 4));
            assert!(values.is_empty());
        }
    }
    #[test]
    fn sum_registry_leak_is_independent_of_payload_ownership() {
        let mut context = Context::new();
        unsafe {
            jett_rt_v1_sum_new(context.pointer(), SUM_SUCCESS, 42, 0);
        }
        assert_eq!(context.count(), 0);
        let mut result = MaybeUninit::uninit();
        assert_eq!(
            unsafe { jett_rt_v1_context_destroy(&mut *context.0, result.as_mut_ptr()) },
            JettRuntimeStatusV1::INVALID_ARGUMENT
        );
        let context = std::mem::ManuallyDrop::new(context);
        unsafe {
            drop(ptr::read(&context.0));
        }
    }
    #[test]
    fn foreign_and_stale_bytes_cannot_be_used_or_double_dropped() {
        let first = Context::new();
        let second = Context::new();
        unsafe {
            let value = jett_rt_v1_bytes_new(first.pointer());
            assert_eq!(jett_rt_v1_bytes_clone(second.pointer(), value), 0);
            assert_ne!(jett_rt_v1_value_status(second.pointer()), 0);
            jett_rt_v1_value_drop(first.pointer(), value);
            assert_eq!(jett_rt_v1_bytes_clone(first.pointer(), value), 0);
            jett_rt_v1_value_drop(first.pointer(), value);
            assert_ne!(jett_rt_v1_value_status(first.pointer()), 0);
        }
        first.destroy(JettRuntimeStatusV1::INVALID_ARGUMENT);
    }
    #[test]
    fn lists_transfer_and_clone_nested_payloads_with_exact_destruction() {
        let context = Context::new();
        unsafe {
            let payload = jett_rt_v1_bytes_new(context.pointer());
            let list = jett_rt_v1_list_new(context.pointer(), 1);
            assert_eq!(
                jett_rt_v1_list_append(context.pointer(), list, payload),
                list
            );
            let copy = jett_rt_v1_list_clone(context.pointer(), list);
            let item = jett_rt_v1_list_get_clone(context.pointer(), copy, 0);
            let taken = jett_rt_v1_sum_take(context.pointer(), item, SUM_SUCCESS);
            assert_ne!(taken, payload);
            jett_rt_v1_value_drop(context.pointer(), taken);
            jett_rt_v1_value_drop(context.pointer(), list);
            jett_rt_v1_value_drop(context.pointer(), copy);
            let lease = acquire_context(context_key(context.pointer()).unwrap()).unwrap();
            let state = lock_unpoisoned(&lease.entry.state);
            let values = &state.as_ref().unwrap().values;
            assert_eq!((values.bytes_created, values.bytes_destroyed), (3, 3));
            assert_eq!((values.lists_created, values.lists_destroyed), (2, 2));
            assert!(values.is_empty());
        }
    }
    #[test]
    fn list_insert_remove_transfers_owned_elements_and_rejects_invalid_indices() {
        let context = Context::new();
        unsafe {
            let first = jett_rt_v1_bytes_new(context.pointer());
            let second = jett_rt_v1_bytes_new(context.pointer());
            let list = jett_rt_v1_list_new(context.pointer(), 1);
            assert_eq!(
                jett_rt_v1_list_insert_at(context.pointer(), list, 0, first),
                list
            );
            assert_eq!(
                jett_rt_v1_list_insert_at(context.pointer(), list, 1, second),
                list
            );
            assert_eq!(jett_rt_v1_list_remove_at(context.pointer(), list, 0), list);
            let lease = acquire_context(context_key(context.pointer()).unwrap()).unwrap();
            let state = lock_unpoisoned(&lease.entry.state);
            let values = &state.as_ref().unwrap().values;
            assert_eq!(values.lists[&list].elements, vec![Some(second)]);
            assert_eq!((values.bytes_created, values.bytes_destroyed), (2, 1));
            drop(state);
            jett_rt_v1_value_drop(context.pointer(), list);
            let state = lock_unpoisoned(&lease.entry.state);
            let values = &state.as_ref().unwrap().values;
            assert_eq!((values.bytes_created, values.bytes_destroyed), (2, 2));
            assert!(values.is_empty());
        }

        let context = Context::new();
        unsafe {
            let list = jett_rt_v1_list_new(context.pointer(), 0);
            assert_eq!(jett_rt_v1_list_append(context.pointer(), list, 7), list);
            assert_eq!(jett_rt_v1_list_insert_at(context.pointer(), list, 2, 9), 0);
            let lease = acquire_context(context_key(context.pointer()).unwrap()).unwrap();
            let state = lock_unpoisoned(&lease.entry.state);
            let values = &state.as_ref().unwrap().values;
            assert_eq!(values.lists[&list].elements, vec![Some(7)]);
            assert!(values.failure.is_some());
            drop(state);
            jett_rt_v1_value_drop(context.pointer(), list);
        }
        context.destroy(JettRuntimeStatusV1::OK);

        let context = Context::new();
        unsafe {
            let list = jett_rt_v1_list_new(context.pointer(), 0);
            assert_eq!(jett_rt_v1_list_append(context.pointer(), list, 7), list);
            assert_eq!(jett_rt_v1_list_remove_at(context.pointer(), list, -1), 0);
            let lease = acquire_context(context_key(context.pointer()).unwrap()).unwrap();
            let state = lock_unpoisoned(&lease.entry.state);
            let values = &state.as_ref().unwrap().values;
            assert_eq!(values.lists[&list].elements, vec![Some(7)]);
            assert!(values.failure.is_some());
            drop(state);
            jett_rt_v1_value_drop(context.pointer(), list);
        }
        context.destroy(JettRuntimeStatusV1::OK);
    }
    #[test]
    fn list_only_leak_fails_context_destruction() {
        let context = Context::new();
        unsafe {
            jett_rt_v1_list_new(context.pointer(), 0);
        }
        context.destroy(JettRuntimeStatusV1::INVALID_ARGUMENT);
    }
    #[test]
    fn empty_numeric_aggregates_report_their_checked_runtime_errors() {
        for (operation, expected) in [
            (
                jett_rt_v1_math_average
                    as unsafe extern "C" fn(*const JettRuntimeContextV1, u64, u32) -> f64,
                b"math.average: list is empty".as_slice(),
            ),
            (
                jett_rt_v1_math_median
                    as unsafe extern "C" fn(*const JettRuntimeContextV1, u64, u32) -> f64,
                b"math.median: list is empty".as_slice(),
            ),
        ] {
            let context = Context::new();
            unsafe {
                let list = jett_rt_v1_list_new(context.pointer(), 0);
                assert_eq!(
                    operation(context.pointer(), list, NativeSortKind::Float64 as u32),
                    0.0
                );
                let lease = acquire_context(context_key(context.pointer()).unwrap()).unwrap();
                let state = lock_unpoisoned(&lease.entry.state);
                assert_eq!(
                    state.as_ref().unwrap().values.failure,
                    Some((JettRuntimeStatusV1::INVALID_ARGUMENT, expected))
                );
                drop(state);
                jett_rt_v1_value_drop(context.pointer(), list);
            }
        }
    }
    #[test]
    fn consuming_list_elements_transfer_identity_and_drop_only_initialized_slots() {
        let context = Context::new();
        unsafe {
            let first = jett_rt_v1_bytes_new(context.pointer());
            let second = jett_rt_v1_bytes_new(context.pointer());
            let list = jett_rt_v1_list_new(context.pointer(), 1);
            jett_rt_v1_list_append(context.pointer(), list, first);
            jett_rt_v1_list_append(context.pointer(), list, second);
            assert_eq!(
                jett_rt_v1_list_element_take(context.pointer(), list, 0),
                first
            );
            // A repeated take fails before changing either remaining owner.
            assert_eq!(jett_rt_v1_list_element_take(context.pointer(), list, 0), 0);
            assert_ne!(jett_rt_v1_value_status(context.pointer()), 0);
            jett_rt_v1_value_drop(context.pointer(), list);
            jett_rt_v1_value_drop(context.pointer(), first);
            let lease = acquire_context(context_key(context.pointer()).unwrap()).unwrap();
            let state = lock_unpoisoned(&lease.entry.state);
            let values = &state.as_ref().unwrap().values;
            assert_eq!((values.bytes_created, values.bytes_destroyed), (2, 2));
            assert_eq!((values.lists_created, values.lists_destroyed), (1, 1));
            assert!(!values.cleanup_failed);
            assert!(values.is_empty());
        }
    }
    #[test]
    fn partial_nested_list_clone_allocation_failure_releases_cloned_prefix() {
        let context = Context::new();
        unsafe {
            let a = jett_rt_v1_bytes_new(context.pointer());
            let b = jett_rt_v1_bytes_new(context.pointer());
            let inner = jett_rt_v1_list_new(context.pointer(), 1);
            jett_rt_v1_list_append(context.pointer(), inner, a);
            jett_rt_v1_list_append(context.pointer(), inner, b);
            let outer = jett_rt_v1_list_new(context.pointer(), 1);
            jett_rt_v1_list_append(context.pointer(), outer, inner);
            let lease = acquire_context(context_key(context.pointer()).unwrap()).unwrap();
            {
                let mut state = lock_unpoisoned(&lease.entry.state);
                // Clone first byte, fail allocation of second byte. The source
                // lists and both originals must remain owned by the caller.
                state.as_mut().unwrap().values.allocation_budget = Some(1);
            }
            assert_eq!(jett_rt_v1_list_clone(context.pointer(), outer), 0);
            assert_eq!(
                jett_rt_v1_value_status(context.pointer()),
                JettRuntimeStatusV1::RESOURCE_EXHAUSTED.code()
            );
            {
                let state = lock_unpoisoned(&lease.entry.state);
                let values = &state.as_ref().unwrap().values;
                assert_eq!((values.bytes_created, values.bytes_destroyed), (3, 1));
                assert!(values.bytes.contains_key(&a) && values.bytes.contains_key(&b));
                assert_eq!((values.lists_created, values.lists_destroyed), (2, 0));
            }
            jett_rt_v1_value_drop(context.pointer(), outer);
            let state = lock_unpoisoned(&lease.entry.state);
            let values = &state.as_ref().unwrap().values;
            assert!(values.is_empty());
            assert!(!values.cleanup_failed);
        }
    }
    #[test]
    fn string_segmentation_allocation_failure_cleans_partial_owned_list() {
        let context = Context::new();
        let text = context.text("abc");
        let lease = acquire_context(context_key(context.pointer()).unwrap()).unwrap();
        {
            let mut state = lock_unpoisoned(&lease.entry.state);
            state.as_mut().unwrap().values.allocation_budget = Some(2);
        }
        unsafe {
            assert_eq!(jett_rt_v1_string_chars(context.pointer(), text), 0);
            assert_eq!(
                jett_rt_v1_value_status(context.pointer()),
                JettRuntimeStatusV1::RESOURCE_EXHAUSTED.code()
            );
            {
                let state = lock_unpoisoned(&lease.entry.state);
                let values = &state.as_ref().unwrap().values;
                assert_eq!((values.lists_created, values.lists_destroyed), (1, 1));
                assert_eq!(values.strings.len(), 1);
                assert_eq!(values.text(text).unwrap(), "abc");
            }
            jett_rt_v1_value_drop(context.pointer(), text);
        }
    }
    #[test]
    fn structs_borrow_fields_and_clone_nested_owners_with_exact_destruction() {
        let context = Context::new();
        unsafe {
            let p = context.pointer();
            let text = context.text("shared");
            let bytes = jett_rt_v1_bytes_from_string(p, text);
            let inner = jett_rt_v1_struct_new(p, 3);
            assert_eq!(jett_rt_v1_struct_init(p, inner, 2, bytes, 1), 0);
            assert_eq!(jett_rt_v1_struct_init(p, inner, 0, text, 1), 0);
            assert_eq!(jett_rt_v1_struct_init(p, inner, 1, u64::MAX, 0), 0);
            let outer = jett_rt_v1_struct_new(p, 1);
            jett_rt_v1_struct_init(p, outer, 0, inner, 1);
            let copy = jett_rt_v1_struct_clone(p, outer);
            let copied_inner = jett_rt_v1_struct_field(p, copy, 0);
            assert_ne!(inner, copied_inner);
            assert_eq!(jett_rt_v1_struct_field(p, inner, 2), bytes);
            assert_ne!(jett_rt_v1_struct_field(p, copied_inner, 2), bytes);
            assert_eq!(jett_rt_v1_struct_field(p, copied_inner, 0), text);
            assert_eq!(jett_rt_v1_struct_field(p, copied_inner, 1), u64::MAX);
            jett_rt_v1_value_drop(p, outer);
            assert_ne!(jett_rt_v1_struct_field(p, copied_inner, 2), 0);
            jett_rt_v1_value_drop(p, copy);
            let lease = acquire_context(context_key(p).unwrap()).unwrap();
            let state = lock_unpoisoned(&lease.entry.state);
            let values = &state.as_ref().unwrap().values;
            assert_eq!((values.structs_created, values.structs_destroyed), (4, 4));
            assert_eq!((values.bytes_created, values.bytes_destroyed), (2, 2));
            assert!(values.is_empty());
        }
    }
    #[test]
    fn actors_replace_owned_state_and_release_it_with_the_context() {
        let context = Context::new();
        unsafe {
            let p = context.pointer();
            let initial = context.text("initial");
            let actor = jett_rt_v1_struct_new(p, 1);
            assert_eq!(jett_rt_v1_struct_init(p, actor, 0, initial, 1), 0);
            assert_eq!(jett_rt_v1_actor_register(p, actor), actor);
            let updated = context.text("updated");
            assert_eq!(jett_rt_v1_actor_replace(p, actor, 0, updated, 1), 0);
            assert_eq!(jett_rt_v1_struct_field(p, actor, 0), updated);
            assert_eq!(context.count(), 1, "old actor state must be released");
            assert_eq!(jett_rt_v1_value_status(p), 0);
        }
        context.destroy(JettRuntimeStatusV1::OK);
    }
    #[test]
    fn reflected_field_validation_checks_owner_member_name_and_type() {
        fn metadata(
            values: &mut NativeValues,
            index: u64,
            owner: &str,
            member: Option<&str>,
            name: &str,
            ty: &str,
        ) -> u64 {
            let owner = values.insert(owner.to_owned()).unwrap();
            let member = match member {
                Some(member) => {
                    let text = values.insert(member.to_owned()).unwrap();
                    values.sum(SUM_SUCCESS, text, true).unwrap()
                }
                None => values.sum(SUM_FAILURE, 0, false).unwrap(),
            };
            let name = values.insert(name.to_owned()).unwrap();
            let ty = values.insert(ty.to_owned()).unwrap();
            let record = values.new_struct(5).unwrap();
            values.structs.get_mut(&record).unwrap().fields = vec![
                Some(NativeField {
                    bits: index,
                    owned: false,
                }),
                Some(NativeField {
                    bits: owner,
                    owned: true,
                }),
                Some(NativeField {
                    bits: member,
                    owned: true,
                }),
                Some(NativeField {
                    bits: name,
                    owned: true,
                }),
                Some(NativeField {
                    bits: ty,
                    owned: true,
                }),
            ];
            record
        }

        let mut values = NativeValues::default();
        let expected = metadata(&mut values, 2, "Shape", Some("circle"), "radius", "float64");
        let matching = metadata(&mut values, 2, "Shape", Some("circle"), "radius", "float64");
        let wrong_owner = metadata(&mut values, 2, "Other", Some("circle"), "radius", "float64");
        let wrong_member = metadata(&mut values, 2, "Shape", Some("square"), "radius", "float64");
        let wrong_name = metadata(&mut values, 2, "Shape", Some("circle"), "size", "float64");
        let wrong_type = metadata(&mut values, 2, "Shape", Some("circle"), "radius", "int64");
        let wrong_index = metadata(&mut values, 1, "Shape", Some("circle"), "radius", "float64");
        assert_eq!(
            values.reflected_field_index(matching, expected, INVALID_REFLECTED_VARIANT_FIELD),
            Ok(2)
        );
        for candidate in [
            wrong_owner,
            wrong_member,
            wrong_name,
            wrong_type,
            wrong_index,
        ] {
            assert_eq!(
                values.reflected_field_index(candidate, expected, INVALID_REFLECTED_VARIANT_FIELD),
                Err(INVALID_REFLECTED_VARIANT_FIELD)
            );
        }
        for record in [
            expected,
            matching,
            wrong_owner,
            wrong_member,
            wrong_name,
            wrong_type,
            wrong_index,
        ] {
            values.drop_value(record).unwrap();
        }
        assert!(values.is_empty());
    }
    #[test]
    fn struct_take_transfers_a_field_without_dropping_it_with_the_record() {
        let context = Context::new();
        unsafe {
            let p = context.pointer();
            let text = context.text("payload");
            let record = jett_rt_v1_struct_new(p, 2);
            assert_eq!(jett_rt_v1_struct_init(p, record, 0, 7, 0), 0);
            assert_eq!(jett_rt_v1_struct_init(p, record, 1, text, 1), 0);
            assert_eq!(jett_rt_v1_struct_take(p, record, 1), text);
            jett_rt_v1_value_drop(p, record);
            let lease = acquire_context(context_key(p).unwrap()).unwrap();
            {
                let state = lock_unpoisoned(&lease.entry.state);
                let values = &state.as_ref().unwrap().values;
                assert_eq!(values.text(text).unwrap(), "payload");
                assert_eq!((values.structs_created, values.structs_destroyed), (1, 1));
            }
            jett_rt_v1_value_drop(p, text);
            let state = lock_unpoisoned(&lease.entry.state);
            assert!(state.as_ref().unwrap().values.is_empty());
        }
    }
    #[test]
    fn structs_partial_clone_rolls_back_each_allocation_boundary() {
        for budget in 0..4 {
            let context = Context::new();
            unsafe {
                let p = context.pointer();
                let outer = jett_rt_v1_struct_new(p, 2);
                let child = jett_rt_v1_struct_new(p, 1);
                let a = jett_rt_v1_bytes_new(p);
                let b = jett_rt_v1_bytes_new(p);
                jett_rt_v1_struct_init(p, child, 0, a, 1);
                jett_rt_v1_struct_init(p, outer, 0, child, 1);
                jett_rt_v1_struct_init(p, outer, 1, b, 1);
                let lease = acquire_context(context_key(p).unwrap()).unwrap();
                lock_unpoisoned(&lease.entry.state)
                    .as_mut()
                    .unwrap()
                    .values
                    .allocation_budget = Some(budget);
                assert_eq!(jett_rt_v1_struct_clone(p, outer), 0);
                assert_eq!(
                    jett_rt_v1_value_status(p),
                    JettRuntimeStatusV1::RESOURCE_EXHAUSTED.code()
                );
                {
                    let state = lock_unpoisoned(&lease.entry.state);
                    let values = &state.as_ref().unwrap().values;
                    assert_eq!(values.structs.len(), 2);
                    assert_eq!(values.bytes.len(), 2);
                    assert!(values.bytes.contains_key(&a) && values.bytes.contains_key(&b));
                }
                jett_rt_v1_value_drop(p, outer);
                let state = lock_unpoisoned(&lease.entry.state);
                assert!(state.as_ref().unwrap().values.is_empty());
                assert!(!state.as_ref().unwrap().values.cleanup_failed);
            }
        }
    }
    #[test]
    fn structs_invalid_projection_and_double_drop_preserve_first_failure() {
        let context = Context::new();
        unsafe {
            let p = context.pointer();
            let value = jett_rt_v1_struct_new(p, 2);
            let bytes = jett_rt_v1_bytes_new(p);
            jett_rt_v1_struct_init(p, value, 0, bytes, 1);
            assert_eq!(jett_rt_v1_struct_field(p, value, 1), 0);
            assert_ne!(jett_rt_v1_value_status(p), 0);
            jett_rt_v1_value_drop(p, value);
            jett_rt_v1_value_drop(p, value);
            let lease = acquire_context(context_key(p).unwrap()).unwrap();
            let state = lock_unpoisoned(&lease.entry.state);
            let values = &state.as_ref().unwrap().values;
            assert_eq!(values.failure, Some(INVALID_STRUCT));
            assert!(values.cleanup_failed);
            assert!(values.is_empty());
            assert_eq!((values.structs_created, values.structs_destroyed), (1, 1));
        }
        context.destroy(JettRuntimeStatusV1::INVALID_ARGUMENT);
    }
    #[test]
    fn structs_empty_record_is_an_owner_and_leak_fails_destruction() {
        let context = Context::new();
        unsafe {
            assert_ne!(jett_rt_v1_struct_new(context.pointer(), 0), 0);
        }
        context.destroy(JettRuntimeStatusV1::INVALID_ARGUMENT);
    }
    #[test]
    fn machine_state_guard_fails_before_projecting_a_wrong_payload() {
        let context = Context::new();
        unsafe {
            let pointer = context.pointer();
            let value = jett_rt_v1_struct_new(pointer, 1);
            assert_eq!(jett_rt_v1_struct_init(pointer, value, 0, 1, 0), 0);
            assert_eq!(jett_rt_v1_machine_expect_state(pointer, value, 1), 0);
            assert_ne!(jett_rt_v1_machine_expect_state(pointer, value, 2), 0);
            assert_ne!(jett_rt_v1_value_status(pointer), 0);
            jett_rt_v1_value_drop(pointer, value);
            let lease = acquire_context(context_key(pointer).unwrap()).unwrap();
            let state = lock_unpoisoned(&lease.entry.state);
            let values = &state.as_ref().unwrap().values;
            assert_eq!(
                values.failure,
                Some((
                    JettRuntimeStatusV1::INVALID_ARGUMENT,
                    b"machine state does not match narrowed type".as_slice()
                ))
            );
            assert!(values.is_empty());
        }
        context.destroy(JettRuntimeStatusV1::OK);
    }
    #[test]
    fn bitfield_decode_rolls_back_each_owned_allocation() {
        fn name(descriptor: &mut Vec<u8>, value: &str) {
            descriptor.extend_from_slice(&(value.len() as u32).to_le_bytes());
            descriptor.extend_from_slice(value.as_bytes());
        }
        let mut descriptor = b"JB\x01\x01".to_vec();
        name(&mut descriptor, "app.Packet");
        descriptor.extend_from_slice(&3_u32.to_le_bytes());
        descriptor.extend_from_slice(&[0, 8]);
        name(&mut descriptor, "kind");
        descriptor.extend_from_slice(&[1, 8]);
        name(&mut descriptor, "protocol");
        name(&mut descriptor, "app.Protocol");
        descriptor.extend_from_slice(&2_u32.to_le_bytes());
        descriptor.extend_from_slice(&1_i64.to_le_bytes());
        descriptor.extend_from_slice(&2_i64.to_le_bytes());
        descriptor.extend_from_slice(&[2, 0]);
        name(&mut descriptor, "payload");

        for budget in 0..4 {
            let mut values = NativeValues::default();
            let input = values.insert_bytes(vec![5, 2, 7, 9]).unwrap();
            values.allocation_budget = Some(budget);
            assert_eq!(values.decode_bitfield(input, &descriptor), Err(EXHAUSTED));
            assert_eq!(values.bytes(input).unwrap(), &[5, 2, 7, 9]);
            assert_eq!(values.structs_created, values.structs_destroyed);
            assert_eq!(values.lists_created, values.lists_destroyed);
            assert_eq!(values.sums_created, values.sums_destroyed);
            values.drop_value(input).unwrap();
            assert!(values.is_empty());
        }
        for budget in 0..2 {
            let mut values = NativeValues::default();
            let input = values.insert_bytes(vec![5]).unwrap();
            values.allocation_budget = Some(budget);
            assert_eq!(values.decode_bitfield(input, &descriptor), Err(EXHAUSTED));
            assert!(values.strings.is_empty());
            values.drop_value(input).unwrap();
            assert!(values.is_empty());
        }
    }
    #[test]
    fn list_sort_orders_all_primitive_widths_and_preserves_string_owners() {
        let cases = [
            (NativeSortKind::Int8, [255, 2, 253], [253, 255, 2]),
            (NativeSortKind::Int16, [65535, 2, 65533], [65533, 65535, 2]),
            (
                NativeSortKind::Int32,
                [u32::MAX as u64, 2, (u32::MAX - 2) as u64],
                [(u32::MAX - 2) as u64, u32::MAX as u64, 2],
            ),
            (
                NativeSortKind::Int64,
                [u64::MAX, 2, u64::MAX - 2],
                [u64::MAX - 2, u64::MAX, 2],
            ),
            (NativeSortKind::Uint8, [255, 2, 1], [1, 2, 255]),
            (NativeSortKind::Uint16, [65535, 2, 1], [1, 2, 65535]),
            (
                NativeSortKind::Uint32,
                [u32::MAX as u64, 2, 1],
                [1, 2, u32::MAX as u64],
            ),
            (NativeSortKind::Uint64, [u64::MAX, 2, 1], [1, 2, u64::MAX]),
            (
                NativeSortKind::Float32,
                [1.5_f32.to_bits() as u64, (-2.25_f32).to_bits() as u64, 0],
                [(-2.25_f32).to_bits() as u64, 0, 1.5_f32.to_bits() as u64],
            ),
            (
                NativeSortKind::Float64,
                [1.5_f64.to_bits(), (-2.25_f64).to_bits(), 0],
                [(-2.25_f64).to_bits(), 0, 1.5_f64.to_bits()],
            ),
            (NativeSortKind::Bool, [1, 0, 1], [0, 1, 1]),
        ];
        for (kind, input, expected) in cases {
            let mut values = NativeValues::default();
            let list = values.new_list(false).unwrap();
            values.lists.get_mut(&list).unwrap().elements = input.map(Some).to_vec();
            assert_eq!(values.sort_list(list, kind as u32), Ok(list));
            assert_eq!(values.lists[&list].elements, expected.map(Some));
            values.drop_value(list).unwrap();
            assert!(values.is_empty());
        }

        let mut values = NativeValues::default();
        let zebra = values.insert("zebra".into()).unwrap();
        let apple = values.insert("apple".into()).unwrap();
        let eclair = values.insert("éclair".into()).unwrap();
        let list = values.new_list(true).unwrap();
        values.lists.get_mut(&list).unwrap().elements =
            vec![Some(zebra), Some(eclair), Some(apple)];
        assert_eq!(
            values.sort_list(list, NativeSortKind::String as u32),
            Ok(list)
        );
        assert_eq!(
            values.lists[&list].elements,
            [Some(apple), Some(zebra), Some(eclair)]
        );
        values.drop_value(list).unwrap();
        assert!(values.is_empty());
    }
    #[test]
    fn indexed_list_sort_preserves_rows_and_missing_keys() {
        let mut values = NativeValues::default();
        let outer = values.new_list(true).unwrap();
        let mut rows = Vec::new();
        for text in ["b", "a", "a"] {
            let key = values.insert(text.into()).unwrap();
            let row = values.new_list(true).unwrap();
            values.lists.get_mut(&row).unwrap().elements.push(Some(key));
            values
                .lists
                .get_mut(&outer)
                .unwrap()
                .elements
                .push(Some(row));
            rows.push(row);
        }
        assert_eq!(
            values.sort_list_by_index(outer, -1, NativeSortKind::String as u32),
            Ok(outer)
        );
        assert_eq!(
            values.lists[&outer].elements,
            rows.iter().copied().map(Some).collect::<Vec<_>>()
        );
        assert_eq!(
            values.sort_list_by_index(outer, 0, NativeSortKind::String as u32),
            Ok(outer)
        );
        assert_eq!(
            values.lists[&outer].elements,
            [Some(rows[1]), Some(rows[2]), Some(rows[0])]
        );
        assert!(
            values
                .list_is_sorted(rows[1], NativeSortKind::String as u32)
                .unwrap()
        );
        values.drop_value(outer).unwrap();
        assert!(values.is_empty());

        let mut values = NativeValues::default();
        let flags = values.new_list(false).unwrap();
        values.lists.get_mut(&flags).unwrap().elements = vec![Some(1), Some(0)];
        assert_eq!(
            values.list_is_sorted(flags, NativeSortKind::Bool as u32),
            Ok(false)
        );
        assert_eq!(values.list_is_sorted(flags, u32::MAX), Ok(true));
        values.drop_value(flags).unwrap();
        assert!(values.is_empty());
    }
    #[test]
    fn set_duplicate_strings_and_clones_release_exactly_one_owner_each() {
        let mut values = NativeValues::default();
        let first = values.insert("ada".into()).unwrap();
        let duplicate = values.insert("ada".into()).unwrap();
        let set = values.new_set(1).unwrap();
        assert_eq!(values.set_add(set, first), Ok(set));
        assert_eq!(values.set_add(set, duplicate), Ok(set));
        assert_eq!(values.sets[&set].elements, [Some(first)]);
        assert!(!values.strings.contains_key(&duplicate));

        let copy = values.clone_set(set).unwrap();
        assert_eq!(values.strings[&first].references, 2);
        assert_eq!(values.set_remove(set, first), Ok(set));
        assert!(values.sets[&set].elements.is_empty());
        assert_eq!(values.strings[&first].references, 1);
        assert_eq!(values.set_position(copy, first), Ok(Some(0)));
        values.drop_value(set).unwrap();
        values.drop_value(copy).unwrap();
        assert_eq!((values.sets_created, values.sets_destroyed), (2, 2));
        assert!(values.is_empty());
    }
    #[test]
    fn partial_set_iteration_drops_only_remaining_owned_elements() {
        let mut values = NativeValues::default();
        let first = values.insert("first".into()).unwrap();
        let second = values.insert("second".into()).unwrap();
        let set = values.new_set(1).unwrap();
        values.set_add(set, first).unwrap();
        values.set_add(set, second).unwrap();
        let borrowed = values.retain(first).unwrap();
        let taken = values
            .sets
            .get_mut(&set)
            .unwrap()
            .elements
            .get_mut(0)
            .and_then(Option::take)
            .unwrap();
        assert_eq!(taken, first);
        values.drop_value(set).unwrap();
        assert!(!values.strings.contains_key(&second));
        assert_eq!(values.strings[&first].references, 2);
        values.drop_value(taken).unwrap();
        values.drop_value(borrowed).unwrap();
        assert!(values.is_empty());
    }
    #[test]
    fn map_literal_duplicates_and_insert_have_distinct_semantics_and_exact_cleanup() {
        let mut values = NativeValues::default();
        let map = values.new_map(1, 1).unwrap();
        let first = values.insert("same".into()).unwrap();
        let second = values.insert("same".into()).unwrap();
        let old = values.insert("old".into()).unwrap();
        let newer = values.insert("new".into()).unwrap();
        values.map_append_literal(map, first, old).unwrap();
        values.map_append_literal(map, second, newer).unwrap();
        assert_eq!(values.maps[&map].entries.len(), 2);
        assert_eq!(values.map_position(map, first), Ok(Some(0)));
        let replacement_key = values.insert("same".into()).unwrap();
        let replacement = values.insert("replacement".into()).unwrap();
        values
            .map_insert(map, replacement_key, replacement)
            .unwrap();
        assert!(!values.strings.contains_key(&replacement_key));
        assert!(!values.strings.contains_key(&old));
        let copy = values.clone_map(map).unwrap();
        values.map_remove(map, first).unwrap();
        assert!(values.maps[&map].entries.is_empty());
        assert_eq!(values.map_position(copy, second), Ok(Some(0)));
        values.drop_value(map).unwrap();
        values.drop_value(copy).unwrap();
        assert_eq!((values.maps_created, values.maps_destroyed), (2, 2));
        assert!(values.is_empty());
    }
    #[test]
    fn partial_map_iteration_releases_only_remaining_fields() {
        let mut values = NativeValues::default();
        let key = values.insert("key".into()).unwrap();
        let value = values.insert("value".into()).unwrap();
        let map = values.new_map(1, 1).unwrap();
        values.map_insert(map, key, value).unwrap();
        let taken_key = values.map_element(map, 0, true, true).unwrap();
        values.drop_value(map).unwrap();
        assert!(!values.strings.contains_key(&value));
        assert!(values.strings.contains_key(&taken_key));
        values.drop_value(taken_key).unwrap();
        assert!(values.is_empty());
    }
    #[test]
    fn csv_nested_values_roll_back_each_allocation_boundary() {
        for budget in 0..10 {
            let mut values = NativeValues::default();
            values.allocation_budget = Some(budget);
            match values.csv_rows(vec![vec!["a".into(), "b".into()], vec!["c".into()]]) {
                Ok(rows) => {
                    values.drop_value(rows).unwrap();
                }
                Err(error) => assert_eq!(error, EXHAUSTED),
            }
            assert!(values.is_empty(), "CSV rows leaked at budget {budget}");
        }
        for budget in 0..12 {
            let mut values = NativeValues::default();
            values.allocation_budget = Some(budget);
            match values.csv_header_rows(vec![vec![
                ("name".into(), "Ada".into()),
                ("age".into(), "30".into()),
            ]]) {
                Ok(rows) => {
                    values.drop_value(rows).unwrap();
                }
                Err(error) => assert_eq!(error, EXHAUSTED),
            }
            assert!(
                values.is_empty(),
                "CSV header rows leaked at budget {budget}"
            );
        }
        let mut values = NativeValues::default();
        let rows = values.csv_rows(vec![vec!["value".into()]]).unwrap();
        values.allocation_budget = Some(0);
        assert_eq!(values.owned_sum(SUM_SUCCESS, rows), Err(EXHAUSTED));
        assert!(values.is_empty());
    }
}
