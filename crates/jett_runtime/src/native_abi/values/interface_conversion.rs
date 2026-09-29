//! Compiler-described conversions across nested interface-compatible boundaries.
use super::*;

/// A finite checked conversion tree. Nominal payloads stop at a box or copy;
/// this descriptor never searches declarations or executes source expressions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeInterfaceConversion {
    Copy {
        owned: bool,
    },
    Box {
        concrete: u64,
        owned: bool,
        nothing: bool,
        layout: Vec<u8>,
    },
    Unbox {
        concrete: u64,
        owned: bool,
        nothing: bool,
    },
    List(Box<Self>),
    Optional(Box<Self>),
    Result(Box<Self>, Box<Self>),
    Map(Box<Self>, bool),
    FunctionAdapter {
        code: u64,
    },
}

impl NativeInterfaceConversion {
    pub fn encode(&self) -> Vec<u8> {
        self.encode_with_function_offsets().0
    }
    /// Returns function-address fields for the compiler's object relocations.
    /// At runtime these fields contain linked addresses, never function IDs.
    pub fn encode_with_function_offsets(&self) -> (Vec<u8>, Vec<(usize, u64)>) {
        let mut bytes = Vec::new();
        let mut functions = Vec::new();
        self.append(&mut bytes, &mut functions);
        (bytes, functions)
    }
    fn append(&self, bytes: &mut Vec<u8>, functions: &mut Vec<(usize, u64)>) {
        match self {
            Self::Copy { owned } => bytes.extend_from_slice(&[0, u8::from(*owned)]),
            Self::Box {
                concrete,
                owned,
                nothing,
                layout,
            } => {
                bytes.extend_from_slice(&[1, u8::from(*owned), u8::from(*nothing)]);
                bytes.extend_from_slice(&concrete.to_le_bytes());
                bytes.extend_from_slice(&(layout.len() as u64).to_le_bytes());
                bytes.extend_from_slice(layout);
            }
            Self::Unbox {
                concrete,
                owned,
                nothing,
            } => {
                bytes.extend_from_slice(&[2, u8::from(*owned), u8::from(*nothing)]);
                bytes.extend_from_slice(&concrete.to_le_bytes());
            }
            Self::List(inner) | Self::Optional(inner) => {
                bytes.push(match self {
                    Self::List(_) => 3,
                    _ => 4,
                });
                inner.append(bytes, functions);
            }
            Self::Map(inner, strings) => {
                bytes.extend_from_slice(&[6, u8::from(*strings)]);
                inner.append(bytes, functions);
            }
            Self::Result(ok, error) => {
                bytes.push(5);
                ok.append(bytes, functions);
                error.append(bytes, functions);
            }
            Self::FunctionAdapter { code } => {
                bytes.push(7);
                functions.push((bytes.len(), *code));
                bytes.extend_from_slice(&code.to_le_bytes());
            }
        }
    }
    pub(super) fn parse(bytes: &[u8]) -> LeafResult<Self> {
        fn take<'a>(bytes: &mut &'a [u8], count: usize) -> LeafResult<&'a [u8]> {
            let (head, tail) = bytes.split_at_checked(count).ok_or(INVALID_STRUCT)?;
            *bytes = tail;
            Ok(head)
        }
        fn number(bytes: &mut &[u8]) -> LeafResult<u64> {
            Ok(u64::from_le_bytes(
                take(bytes, 8)?.try_into().map_err(|_| INVALID_STRUCT)?,
            ))
        }
        fn flag(bytes: &mut &[u8]) -> LeafResult<bool> {
            match take(bytes, 1)?[0] {
                0 => Ok(false),
                1 => Ok(true),
                _ => Err(INVALID_STRUCT),
            }
        }
        use NativeInterfaceConversion as C;
        enum Parent {
            List,
            Optional,
            Map(bool),
            ResultOk,
            ResultError(C),
        }
        let mut remaining = bytes;
        let mut parents = Vec::new();
        loop {
            let mut value = match take(&mut remaining, 1)?[0] {
                0 => C::Copy {
                    owned: flag(&mut remaining)?,
                },
                tag @ (1 | 2) => {
                    let owned = flag(&mut remaining)?;
                    let nothing = flag(&mut remaining)?;
                    if owned && nothing {
                        return Err(INVALID_STRUCT);
                    }
                    let concrete = number(&mut remaining)?;
                    if tag == 1 {
                        let length =
                            usize::try_from(number(&mut remaining)?).map_err(|_| INVALID_STRUCT)?;
                        let layout = take(&mut remaining, length)?.to_vec();
                        NativeDebugLayout::parse(&layout)?;
                        C::Box {
                            concrete,
                            owned,
                            nothing,
                            layout,
                        }
                    } else {
                        C::Unbox {
                            concrete,
                            owned,
                            nothing,
                        }
                    }
                }
                3 => {
                    parents.push(Parent::List);
                    continue;
                }
                4 => {
                    parents.push(Parent::Optional);
                    continue;
                }
                5 => {
                    parents.push(Parent::ResultOk);
                    continue;
                }
                6 => {
                    parents.push(Parent::Map(flag(&mut remaining)?));
                    continue;
                }
                7 => {
                    let code = number(&mut remaining)?;
                    if code == 0 {
                        return Err(INVALID_STRUCT);
                    }
                    C::FunctionAdapter { code }
                }
                _ => return Err(INVALID_STRUCT),
            };
            loop {
                value = match parents.pop() {
                    Some(Parent::List) => C::List(Box::new(value)),
                    Some(Parent::Optional) => C::Optional(Box::new(value)),
                    Some(Parent::Map(strings)) => C::Map(Box::new(value), strings),
                    Some(Parent::ResultOk) => {
                        parents.push(Parent::ResultError(value));
                        break;
                    }
                    Some(Parent::ResultError(ok)) => C::Result(Box::new(ok), Box::new(value)),
                    None => {
                        if !remaining.is_empty() {
                            return Err(INVALID_STRUCT);
                        }
                        return Ok(value);
                    }
                };
            }
        }
    }

    fn owned(&self) -> bool {
        match self {
            Self::Copy { owned } | Self::Unbox { owned, .. } => *owned,
            _ => true,
        }
    }
    pub(super) fn convert(
        &self,
        values: &mut NativeValues,
        input: NativeField,
    ) -> LeafResult<NativeField> {
        let output = match self {
            Self::Copy { owned } => {
                if input.owned != *owned {
                    return Err(INVALID_STRUCT);
                }
                return Ok(NativeField {
                    bits: if *owned {
                        values.clone_value(input.bits)?
                    } else {
                        input.bits
                    },
                    ..input
                });
            }
            Self::Box {
                concrete,
                owned,
                nothing,
                layout,
            } => {
                if input.owned != *owned {
                    return Err(INVALID_STRUCT);
                }
                let (bits, depth) = if *nothing {
                    (0, input.bits)
                } else {
                    (input.bits, input.pending_depth)
                };
                values.interface_box(*concrete, bits, *owned, depth, layout)?
            }
            Self::Unbox {
                concrete,
                owned,
                nothing,
            } => {
                if !input.owned || values.struct_field(input.bits, 0)?.bits != *concrete {
                    return Err(INVALID_STRUCT);
                }
                let payload = values.struct_field(input.bits, 1)?;
                if payload.owned != *owned {
                    return Err(INVALID_STRUCT);
                }
                let depth = values
                    .owned_pending_depth(input.bits)?
                    .checked_add(input.pending_depth)
                    .ok_or(EXHAUSTED)?;
                return Ok(NativeField {
                    bits: if *owned {
                        values.clone_with_pending_depth(payload.bits, depth)?
                    } else if *nothing {
                        depth
                    } else {
                        payload.bits
                    },
                    owned: *owned,
                    pending_depth: if *owned || *nothing { 0 } else { depth },
                });
            }
            Self::List(element) => element.convert_list(values, input.bits)?,
            Self::Map(element, strings) => element.convert_map(values, input.bits, *strings)?,
            Self::Optional(ok) => Self::convert_sum(values, input.bits, ok, None)?,
            Self::Result(ok, error) => Self::convert_sum(values, input.bits, ok, Some(error))?,
            Self::FunctionAdapter { code } => {
                if !input.owned {
                    return Err(INVALID_STRUCT);
                }
                values.function_adapter(input.bits, *code)?
            }
        };
        Ok(NativeField {
            bits: output,
            owned: true,
            pending_depth: 0,
        })
    }
    fn convert_list(&self, values: &mut NativeValues, source: u64) -> LeafResult<u64> {
        let list = values.lists.get(&source).ok_or(INVALID_LIST)?;
        let (elements, depths, owned, pending) = (
            list.elements.clone(),
            list.element_pending_depths.clone(),
            list.owned,
            list.pending_depth,
        );
        let output = values.new_list(self.owned())?;
        let result = (|| {
            values
                .lists
                .get_mut(&output)
                .ok_or(INVALID_LIST)?
                .elements
                .try_reserve_exact(elements.len())
                .map_err(|_| EXHAUSTED)?;
            for (index, bits) in elements.into_iter().enumerate() {
                let Some(bits) = bits else {
                    values
                        .lists
                        .get_mut(&output)
                        .ok_or(INVALID_LIST)?
                        .elements
                        .push(None);
                    continue;
                };
                let converted = self.convert(
                    values,
                    NativeField {
                        bits,
                        owned,
                        pending_depth: depths.get(&index).copied().unwrap_or(0),
                    },
                )?;
                let list = values.lists.get_mut(&output).ok_or(INVALID_LIST)?;
                list.elements.push(Some(converted.bits));
                if converted.pending_depth != 0 {
                    list.element_pending_depths
                        .insert(index, converted.pending_depth);
                }
            }
            values
                .lists
                .get_mut(&output)
                .ok_or(INVALID_LIST)?
                .pending_depth = pending;
            Ok(output)
        })();
        if result.is_err() {
            let _ = values.drop_value(output);
        }
        result
    }
    fn convert_map(
        &self,
        values: &mut NativeValues,
        source: u64,
        target_strings: bool,
    ) -> LeafResult<u64> {
        let map = values.maps.get(&source).ok_or(INVALID_MAP)?;
        let (entries, strings, owned, pending) = (
            map.entries.clone(),
            map.key_strings,
            map.value_owned,
            map.pending_depth,
        );
        if strings != target_strings && entries.iter().any(Option::is_some) {
            return Err(INVALID_MAP);
        }
        let output = values.new_map(u32::from(target_strings), u32::from(self.owned()))?;
        let result = (|| {
            values
                .maps
                .get_mut(&output)
                .ok_or(INVALID_MAP)?
                .entries
                .try_reserve_exact(entries.len())
                .map_err(|_| EXHAUSTED)?;
            for entry in entries {
                let Some(entry) = entry else {
                    values
                        .maps
                        .get_mut(&output)
                        .ok_or(INVALID_MAP)?
                        .entries
                        .push(None);
                    continue;
                };
                if entry.key_taken {
                    return Err(INVALID_MAP);
                }
                let key = if strings {
                    values.clone_value(entry.key)?
                } else {
                    entry.key
                };
                let converted = match self.convert(
                    values,
                    NativeField {
                        bits: entry.value,
                        owned,
                        pending_depth: entry.value_pending_depth,
                    },
                ) {
                    Ok(value) => value,
                    Err(error) => {
                        if strings {
                            values.drop_value(key)?;
                        }
                        return Err(error);
                    }
                };
                values
                    .maps
                    .get_mut(&output)
                    .ok_or(INVALID_MAP)?
                    .entries
                    .push(Some(NativeMapEntry {
                        key,
                        value: converted.bits,
                        value_pending_depth: converted.pending_depth,
                        ..entry
                    }));
            }
            values
                .maps
                .get_mut(&output)
                .ok_or(INVALID_MAP)?
                .pending_depth = pending;
            Ok(output)
        })();
        if result.is_err() {
            let _ = values.drop_value(output);
        }
        result
    }
    fn convert_sum(
        values: &mut NativeValues,
        source: u64,
        ok: &Self,
        error: Option<&Self>,
    ) -> LeafResult<u64> {
        let sum = values.sums.get(&source).ok_or(INVALID_SUM)?;
        let (tag, pending, input) = (
            sum.tag,
            sum.pending_depth,
            NativeField {
                bits: sum.bits,
                owned: sum.owned,
                pending_depth: sum.payload_pending_depth,
            },
        );
        let conversion = if tag == SUM_SUCCESS { Some(ok) } else { error };
        let payload = match conversion {
            Some(conversion) => conversion.convert(values, input)?,
            None => NativeField {
                bits: 0,
                owned: false,
                pending_depth: 0,
            },
        };
        let output = match values.sum(tag, payload.bits, payload.owned) {
            Ok(output) => output,
            Err(error) => {
                if payload.owned {
                    values.drop_value(payload.bits)?;
                }
                return Err(error);
            }
        };
        let sum = values.sums.get_mut(&output).ok_or(INVALID_SUM)?;
        sum.pending_depth = pending;
        sum.payload_pending_depth = payload.pending_depth;
        Ok(output)
    }
}
