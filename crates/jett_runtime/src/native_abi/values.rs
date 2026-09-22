//! Typed native leaf operations. See docs/active/native_value_abi.md.
//! Every pointer must refer to a live stationary ABI context, except literal
//! bytes which are borrowed for the call. No Rust value crosses this ABI.
use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
use unicode_segmentation::UnicodeSegmentation;

pub type NativeHandle = u64;
type Failure = (JettRuntimeStatusV1, &'static [u8]);
type LeafResult<T> = Result<T, Failure>;
const INVALID_HANDLE: Failure = (
    JettRuntimeStatusV1::INVALID_ARGUMENT,
    b"invalid native string handle",
);
const INVALID_LIST: Failure = (
    JettRuntimeStatusV1::INVALID_ARGUMENT,
    b"invalid native list handle",
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
struct NativeList {
    elements: Vec<u64>,
    owned: bool,
}
struct NativeSum {
    tag: u32,
    bits: u64,
    owned: bool,
}
#[derive(Default)]
pub(super) struct NativeValues {
    strings: HashMap<NativeHandle, NativeString>,
    bytes: HashMap<NativeHandle, Vec<u8>>,
    sums: HashMap<NativeHandle, NativeSum>,
    lists: HashMap<NativeHandle, NativeList>,
    lists_created: u64,
    lists_destroyed: u64,
    sums_created: u64,
    sums_destroyed: u64,
    bytes_created: u64,
    bytes_destroyed: u64,
    failure: Option<Failure>,
    pub(super) cleanup_failed: bool,
    stdout: Option<u64>,
}
impl NativeValues {
    pub(super) fn is_empty(&self) -> bool {
        self.strings.is_empty()
            && self.bytes.is_empty()
            && self.bytes_created == self.bytes_destroyed
            && self.sums.is_empty()
            && self.sums_created == self.sums_destroyed
            && self.lists.is_empty()
            && self.lists_created == self.lists_destroyed
    }
    fn insert(&mut self, text: String) -> LeafResult<u64> {
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
        let id = next_identity()?;
        self.bytes.insert(id, bytes);
        self.bytes_created += 1;
        Ok(id)
    }
    fn bytes(&self, id: u64) -> LeafResult<&[u8]> {
        self.bytes.get(&id).map(Vec::as_slice).ok_or((
            JettRuntimeStatusV1::INVALID_ARGUMENT,
            b"invalid native bytes handle",
        ))
    }
    fn sum(&mut self, tag: u32, bits: u64, owned: bool) -> LeafResult<u64> {
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
    fn new_list(&mut self, owned: bool) -> LeafResult<u64> {
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
    fn clone_list(&mut self, id: u64) -> LeafResult<u64> {
        let list = self.lists.get(&id).ok_or(INVALID_LIST)?;
        let (elements, owned) = (list.elements.clone(), list.owned);
        let mut output = Vec::new();
        output
            .try_reserve_exact(elements.len())
            .map_err(|_| EXHAUSTED)?;
        for bits in elements {
            if owned {
                match self.clone_value(bits) {
                    Ok(value) => output.push(value),
                    Err(error) => {
                        for v in output {
                            self.drop_value(v)?;
                        }
                        return Err(error);
                    }
                }
            } else {
                output.push(bits);
            }
        }
        let id = match self.new_list(owned) {
            Ok(id) => id,
            Err(error) => {
                if owned {
                    for v in output {
                        self.drop_value(v)?;
                    }
                }
                return Err(error);
            }
        };
        self.lists.get_mut(&id).ok_or(INVALID_LIST)?.elements = output;
        Ok(id)
    }
    fn clone_value(&mut self, id: u64) -> LeafResult<u64> {
        if self.lists.contains_key(&id) {
            return self.clone_list(id);
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
        if let Some(list) = self.lists.remove(&id) {
            self.lists_destroyed += 1;
            if list.owned {
                for value in list.elements {
                    self.drop_value(value)?;
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
    ListElementClone, jett_rt_v1_list_element_clone, false, (value: u64 => I64, index: i64 => I64), u64 => I64,
        |s| { let list = s.lists.get(&value).ok_or(INVALID_LIST)?;
            let bits = usize::try_from(index).ok().and_then(|i| list.elements.get(i)).copied().ok_or(INVALID_LIST)?;
            if list.owned { s.clone_value(bits) } else { Ok(bits) } };

    ListSumInt, jett_rt_v1_list_sum_int64, false, (value: u64 => I64), i64 => I64,
        |s| { let list = s.lists.get(&value).ok_or(INVALID_LIST)?;
            if list.owned { return Err(INVALID_LIST); }
            Ok(list.elements.iter().fold(0_i64, |acc, bits| acc.wrapping_add(*bits as i64))) };

    ListNew, jett_rt_v1_list_new, false, (owned: u32 => I32), u64 => I64,
        |s| { if owned > 1 { return Err(INVALID_LIST); } s.new_list(owned != 0) };
    ListLength, jett_rt_v1_list_length, false, (value: u64 => I64), i64 => I64,
        |s| Ok(s.lists.get(&value).ok_or(INVALID_LIST)?.elements.len() as i64);
    ListAppend, jett_rt_v1_list_append, false, (value: u64 => I64, bits: u64 => I64), u64 => I64,
        |s| { let list = s.lists.get_mut(&value).ok_or(INVALID_LIST)?;
            list.elements.try_reserve(1).map_err(|_| EXHAUSTED)?;
            list.elements.push(bits); Ok(value) };
    ListGet, jett_rt_v1_list_get_clone, false, (value: u64 => I64, index: i64 => I64), u64 => I64,
        |s| { let list = s.lists.get(&value).ok_or(INVALID_LIST)?;
            let owned = list.owned;
            let found = usize::try_from(index).ok().and_then(|i| list.elements.get(i)).copied();
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

    DropValue, jett_rt_v1_value_drop, true, (value: u64 => I64), u32 => I32,
        |s| s.drop_value(value);
    BytesNew, jett_rt_v1_bytes_new, false, (), u64 => I64,
        |s| s.insert_bytes(Vec::new());
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
    Stdout, jett_rt_v1_string_stdout, false, (authority: u64 => I64, value: u64 => I64), u32 => I32,
        |s| {
            if s.stdout != Some(authority) { return Err((JettRuntimeStatusV1::INVALID_ARGUMENT, b"invalid Stdout authority")); }
            { let mut stdout = io::stdout().lock(); write_all_bytes(&mut stdout, s.text(value)?.as_bytes()).and_then(|_| stdout.flush()).map_err(|_| (JettRuntimeStatusV1::IO_FAILURE, STDOUT_WRITE_MESSAGE))?; } Ok(0)
        };
    DebugPrint, jett_rt_v1_string_debug_print, false, (value: u64 => I64), u32 => I32,
        |s| { { let mut stdout = io::stdout().lock(); write_all_bytes(&mut stdout, s.text(value)?.as_bytes()).and_then(|_| stdout.flush()).map_err(|_| (JettRuntimeStatusV1::IO_FAILURE, STDOUT_WRITE_MESSAGE))?; } Ok(0) };
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
    fn list_only_leak_fails_context_destruction() {
        let context = Context::new();
        unsafe {
            jett_rt_v1_list_new(context.pointer(), 0);
        }
        context.destroy(JettRuntimeStatusV1::INVALID_ARGUMENT);
    }
}
