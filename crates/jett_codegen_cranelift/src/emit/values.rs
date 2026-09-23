use super::*;
use jett_hir::{IntrinsicId, MapEntry, StringSegment, VariantId};
use jett_types::BitfieldFieldKind;
use std::collections::BTreeSet;

impl Translator<'_, '_> {
    pub(super) fn leaf(
        &mut self,
        leaf: NativeLeaf,
        args: &[Value],
        check: bool,
    ) -> Result<Value, CodegenError> {
        let id = crate::values::declare_leaf(self.module, leaf)?;
        let reference = self.module.declare_func_in_func(id, self.builder.func);
        let context = self.builder.use_var(self.runtime_context);
        let mut arguments = vec![context];
        arguments.extend_from_slice(args);
        let call = self.builder.ins().call(reference, &arguments);
        let result = self.builder.func.dfg.inst_results(call)[0];
        if check {
            self.check_failure()?;
        }
        Ok(result)
    }
    pub(super) fn check_failure(&mut self) -> Result<(), CodegenError> {
        let status = self.leaf(NativeLeaf::Status, &[], false)?;
        let success = self.builder.create_block();
        self.builder
            .ins()
            .brif(status, self.failure_block, &[], success, &[]);
        self.builder.switch_to_block(success);
        Ok(())
    }
    pub(super) fn own(&mut self, value: Value) -> Result<LoweredValue, CodegenError> {
        let slot = self
            .temporary_slots
            .get(self.next_temporary)
            .ok_or_else(|| {
                CodegenError::Backend("MIR temporary ownership bound exceeded".into())
            })?;
        self.next_temporary += 1;
        self.builder.ins().stack_store(value, *slot, 0);
        Ok(LoweredValue::Owned(value, *slot))
    }
    pub(super) fn construct_struct(
        &mut self,
        fields: &[Expression],
        evaluation_order: &[usize],
        span: Span,
    ) -> Result<LoweredValue, CodegenError> {
        let count = self
            .builder
            .ins()
            .iconst(ir::types::I64, fields.len() as i64);
        let handle = self.leaf(NativeLeaf::StructNew, &[count], true)?;
        // Own the partially initialized record before evaluating any field.
        let record = self.own_linear(handle)?;
        for &index in evaluation_order {
            let value = self.expression(&fields[index])?;
            let (bits, owned) = self.payload_bits(value);
            let index = self.builder.ins().iconst(ir::types::I64, index as i64);
            let owned = self.builder.ins().iconst(ir::types::I32, i64::from(owned));
            self.leaf(NativeLeaf::StructInit, &[handle, index, bits, owned], true)?;
            if let LoweredValue::Owned(_, slot) = value {
                self.clear_slot(slot);
            }
        }
        let _ = span;
        Ok(record)
    }
    pub(super) fn construct_enum(
        &mut self,
        variant: VariantId,
        payloads: &[Expression],
        span: Span,
    ) -> Result<LoweredValue, CodegenError> {
        let count = i64::try_from(payloads.len() + 1)
            .map_err(|_| self.unsupported(span, "enum payload count"))?;
        let count = self.builder.ins().iconst(ir::types::I64, count);
        let handle = self.leaf(NativeLeaf::StructNew, &[count], true)?;
        let record = self.own_linear(handle)?;
        let zero = self.builder.ins().iconst(ir::types::I64, 0);
        let tag = self
            .builder
            .ins()
            .iconst(ir::types::I64, i64::from(variant.index()));
        let borrowed = self.builder.ins().iconst(ir::types::I32, 0);
        self.leaf(NativeLeaf::StructInit, &[handle, zero, tag, borrowed], true)?;
        for (field, payload) in payloads.iter().enumerate() {
            let value = self.expression(payload)?;
            let (bits, owned) = self.payload_bits(value);
            let index = self.builder.ins().iconst(ir::types::I64, field as i64 + 1);
            let owned = self.builder.ins().iconst(ir::types::I32, i64::from(owned));
            self.leaf(NativeLeaf::StructInit, &[handle, index, bits, owned], true)?;
            if let LoweredValue::Owned(_, slot) = value {
                self.clear_slot(slot);
            }
        }
        Ok(record)
    }
    pub(super) fn encode_bitfield(
        &mut self,
        value: LoweredValue,
        ty: TypeId,
        span: Span,
    ) -> Result<LoweredValue, CodegenError> {
        let Type::Bitfield(id) = self.types.resolve(ty) else {
            return Err(self.unsupported(span, "bitfield encoding type"));
        };
        let layout = self.types.resolve_bitfield(*id).clone();
        let record = self.scalar(value, span)?;
        let bytes = self.leaf(NativeLeaf::BytesNew, &[], true)?;
        let output = self.own_linear(bytes)?;
        let mut bit_offset = 0_u64;
        for (field_index, field) in layout.fields.iter().enumerate() {
            let index = self
                .builder
                .ins()
                .iconst(ir::types::I64, field_index as i64);
            let bits = self.leaf(NativeLeaf::StructField, &[record, index], true)?;
            match &field.kind {
                BitfieldFieldKind::Bits { width } => {
                    let numeric = if let Type::Enum(enum_id) = self.types.resolve(field.ty) {
                        let zero = self.builder.ins().iconst(ir::types::I64, 0);
                        let tag = self.leaf(NativeLeaf::StructField, &[bits, zero], true)?;
                        let variants = self.types.resolve_enum(*enum_id).variants.clone();
                        let mut numeric = zero;
                        for (index, variant) in variants.iter().enumerate() {
                            let matches =
                                self.builder.ins().icmp_imm(IntCC::Equal, tag, index as i64);
                            let discriminant = self
                                .builder
                                .ins()
                                .iconst(ir::types::I64, variant.discriminant);
                            numeric = self.builder.ins().select(matches, discriminant, numeric);
                        }
                        numeric
                    } else {
                        bits
                    };
                    let width_value = self.builder.ins().iconst(ir::types::I32, i64::from(*width));
                    let network = self
                        .builder
                        .ins()
                        .iconst(ir::types::I32, i64::from(layout.network_order));
                    let offset = self.builder.ins().iconst(ir::types::I64, bit_offset as i64);
                    self.leaf(
                        NativeLeaf::BitfieldWriteBits,
                        &[bytes, numeric, width_value, network, offset],
                        true,
                    )?;
                    bit_offset += u64::from(*width);
                }
                BitfieldFieldKind::Payload => {
                    self.leaf(NativeLeaf::BitfieldExtendPayload, &[bytes, bits], true)?;
                }
            }
        }
        Ok(output)
    }
    pub(super) fn decode_bitfield(
        &mut self,
        value: LoweredValue,
        result_type: TypeId,
        span: Span,
    ) -> Result<LoweredValue, CodegenError> {
        let Type::Result(bitfield_type, _) = self.types.resolve(result_type) else {
            return Err(self.unsupported(span, "bitfield decoding result"));
        };
        let Type::Bitfield(id) = self.types.resolve(*bitfield_type) else {
            return Err(self.unsupported(span, "bitfield decoding type"));
        };
        let layout = self.types.resolve_bitfield(*id).clone();
        fn name(data: &mut Vec<u8>, value: &str) -> Result<(), CodegenError> {
            let length = u32::try_from(value.len())
                .map_err(|_| CodegenError::Backend("bitfield layout name is too long".into()))?;
            data.extend_from_slice(&length.to_le_bytes());
            data.extend_from_slice(value.as_bytes());
            Ok(())
        }
        let mut data = vec![b'J', b'B', 1, u8::from(layout.network_order)];
        name(&mut data, &layout.name)?;
        let field_count = u32::try_from(layout.fields.len())
            .map_err(|_| CodegenError::Backend("too many bitfield fields".into()))?;
        data.extend_from_slice(&field_count.to_le_bytes());
        for field in &layout.fields {
            match &field.kind {
                BitfieldFieldKind::Bits { width } => {
                    let width = u8::try_from(*width)
                        .map_err(|_| self.unsupported(span, "bitfield decoding width"))?;
                    if let Type::Enum(enum_id) = self.types.resolve(field.ty) {
                        data.extend_from_slice(&[1, width]);
                        name(&mut data, &field.name)?;
                        let enum_layout = self.types.resolve_enum(*enum_id);
                        name(&mut data, &enum_layout.name)?;
                        let variant_count = u32::try_from(enum_layout.variants.len())
                            .map_err(|_| CodegenError::Backend("too many enum variants".into()))?;
                        data.extend_from_slice(&variant_count.to_le_bytes());
                        for variant in &enum_layout.variants {
                            data.extend_from_slice(&variant.discriminant.to_le_bytes());
                        }
                    } else {
                        data.extend_from_slice(&[0, width]);
                        name(&mut data, &field.name)?;
                    }
                }
                BitfieldFieldKind::Payload => {
                    data.extend_from_slice(&[2, 0]);
                    name(&mut data, &field.name)?;
                }
            }
        }
        let length = i64::try_from(data.len())
            .map_err(|_| CodegenError::Backend("bitfield layout is too large".into()))?;
        let item = self
            .module
            .declare_anonymous_data(false, false)
            .map_err(|error| CodegenError::Backend(error.to_string()))?;
        let mut description = cranelift_module::DataDescription::new();
        description.define(data.into_boxed_slice());
        self.module
            .define_data(item, &description)
            .map_err(|error| CodegenError::Backend(error.to_string()))?;
        let reference = self.module.declare_data_in_func(item, self.builder.func);
        let pointer = self.builder.ins().global_value(ir::types::I64, reference);
        let length = self.builder.ins().iconst(ir::types::I64, length);
        let value = self.scalar(value, span)?;
        let decoded = self.leaf(NativeLeaf::BitfieldDecode, &[value, pointer, length], true)?;
        self.own_linear(decoded)
    }
    pub(super) fn struct_field(
        &mut self,
        base: &Expression,
        index: u32,
        ty: TypeId,
        span: Span,
    ) -> Result<LoweredValue, CodegenError> {
        let parent = self.argument(base, true)?;
        let parent = self.scalar(parent, span)?;
        let index = self.builder.ins().iconst(ir::types::I64, i64::from(index));
        let bits = self.leaf(NativeLeaf::StructField, &[parent, index], true)?;
        if is_linear(self.types, ty) {
            // The owning root/temporary stays live through the bounded loan.
            return Ok(LoweredValue::Scalar(bits));
        }
        let bits = if ty == TypeInterner::STRING {
            self.leaf(NativeLeaf::Retain, &[bits], true)?
        } else {
            bits
        };
        self.unpack_payload(bits, ty, span)
    }
    pub(super) fn construct_sum(
        &mut self,
        success: bool,
        payload: Option<&Expression>,
        span: Span,
    ) -> Result<LoweredValue, CodegenError> {
        let value = if let Some(payload) = payload {
            self.expression(payload)?
        } else {
            LoweredValue::Nothing
        };
        let (bits, owned) = self.payload_bits(value);
        let tag = self
            .builder
            .ins()
            .iconst(ir::types::I32, i64::from(success));
        let owns = self.builder.ins().iconst(ir::types::I32, i64::from(owned));
        let sum = self.leaf(NativeLeaf::SumNew, &[tag, bits, owns], true)?;
        if let LoweredValue::Owned(_, slot) = value {
            self.clear_slot(slot);
        }
        let _ = span;
        self.own_linear(sum)
    }
    fn payload_bits(&mut self, value: LoweredValue) -> (Value, bool) {
        match value {
            LoweredValue::Nothing => (self.builder.ins().iconst(ir::types::I64, 0), false),
            LoweredValue::Owned(v, _) => (v, true),
            LoweredValue::Scalar(v) => {
                let ty = self.builder.func.dfg.value_type(v);
                let bits = if ty == ir::types::F64 {
                    self.builder
                        .ins()
                        .bitcast(ir::types::I64, ir::MemFlags::new(), v)
                } else if ty == ir::types::F32 {
                    let bits = self
                        .builder
                        .ins()
                        .bitcast(ir::types::I32, ir::MemFlags::new(), v);
                    self.builder.ins().uextend(ir::types::I64, bits)
                } else if ty != ir::types::I64 {
                    self.builder.ins().uextend(ir::types::I64, v)
                } else {
                    v
                };
                (bits, false)
            }
        }
    }
    pub(super) fn unpack_payload(
        &mut self,
        bits: Value,
        ty: TypeId,
        span: Span,
    ) -> Result<LoweredValue, CodegenError> {
        if ty == TypeInterner::STRING || is_linear(self.types, ty) {
            return self.own(bits);
        }
        let Some(native) = clif_type(self.types, ty, "sum payload")? else {
            return Ok(LoweredValue::Nothing);
        };
        let value = if native == ir::types::F64 {
            self.builder
                .ins()
                .bitcast(native, ir::MemFlags::new(), bits)
        } else if native == ir::types::F32 {
            let bits = self.builder.ins().ireduce(ir::types::I32, bits);
            self.builder
                .ins()
                .bitcast(native, ir::MemFlags::new(), bits)
        } else if native != ir::types::I64 {
            self.builder.ins().ireduce(native, bits)
        } else {
            bits
        };
        let _ = span;
        Ok(LoweredValue::Scalar(value))
    }
    fn list_new(&mut self, ty: TypeId, span: Span) -> Result<LoweredValue, CodegenError> {
        let Type::List(element) = self.types.resolve(ty) else {
            return Err(self.unsupported(span, "invalid list layout"));
        };
        let owned = *element == TypeInterner::STRING || is_linear(self.types, *element);
        let owned = self.builder.ins().iconst(ir::types::I32, i64::from(owned));
        let value = self.leaf(NativeLeaf::ListNew, &[owned], true)?;
        self.own_linear(value)
    }
    fn list_append(
        &mut self,
        list: LoweredValue,
        value: LoweredValue,
        span: Span,
    ) -> Result<(), CodegenError> {
        let handle = self.scalar(list, span)?;
        let (bits, _) = self.payload_bits(value);
        self.leaf(NativeLeaf::ListAppend, &[handle, bits], true)?;
        if let LoweredValue::Owned(_, slot) = value {
            self.clear_slot(slot);
        }
        Ok(())
    }
    pub(super) fn construct_list(
        &mut self,
        elements: &[Expression],
        ty: TypeId,
        span: Span,
    ) -> Result<LoweredValue, CodegenError> {
        let list = self.list_new(ty, span)?;
        for element in elements {
            let value = self.expression(element)?;
            self.list_append(list, value, span)?;
        }
        Ok(list)
    }
    fn map_new(&mut self, ty: TypeId, span: Span) -> Result<LoweredValue, CodegenError> {
        let Type::Map(key, value) = self.types.resolve(ty) else {
            return Err(self.unsupported(span, "invalid map layout"));
        };
        let key_strings = self
            .builder
            .ins()
            .iconst(ir::types::I32, i64::from(*key == TypeInterner::STRING));
        let value_owned = self.builder.ins().iconst(
            ir::types::I32,
            i64::from(*value == TypeInterner::STRING || is_linear(self.types, *value)),
        );
        let result = self.leaf(NativeLeaf::MapNew, &[key_strings, value_owned], true)?;
        self.own_linear(result)
    }
    fn map_insert(
        &mut self,
        map: LoweredValue,
        key: LoweredValue,
        value: LoweredValue,
        literal: bool,
        span: Span,
    ) -> Result<(), CodegenError> {
        let handle = self.scalar(map, span)?;
        let (key_bits, _) = self.payload_bits(key);
        let (value_bits, _) = self.payload_bits(value);
        self.leaf(
            if literal {
                NativeLeaf::MapAppendLiteral
            } else {
                NativeLeaf::MapInsert
            },
            &[handle, key_bits, value_bits],
            true,
        )?;
        if let LoweredValue::Owned(_, slot) = key {
            self.clear_slot(slot);
        }
        if let LoweredValue::Owned(_, slot) = value {
            self.clear_slot(slot);
        }
        Ok(())
    }
    pub(super) fn construct_map(
        &mut self,
        entries: &[MapEntry],
        ty: TypeId,
        span: Span,
    ) -> Result<LoweredValue, CodegenError> {
        let map = self.map_new(ty, span)?;
        for entry in entries {
            let key = self.expression(&entry.key)?;
            let value = self.expression(&entry.value)?;
            self.map_insert(map, key, value, true, span)?;
        }
        Ok(map)
    }
    fn map_intrinsic(
        &mut self,
        id: IntrinsicId,
        values: &[LoweredValue],
        result_type: TypeId,
        span: Span,
    ) -> Result<LoweredValue, CodegenError> {
        match id {
            IntrinsicId::MapNew => self.map_new(result_type, span),
            IntrinsicId::MapLength => {
                let map = self.scalar(values[0], span)?;
                Ok(LoweredValue::Scalar(self.leaf(
                    NativeLeaf::MapLength,
                    &[map],
                    true,
                )?))
            }
            IntrinsicId::MapHas => {
                let map = self.scalar(values[0], span)?;
                let key = self.scalar(values[1], span)?;
                let found = self.leaf(NativeLeaf::MapHas, &[map, key], true)?;
                Ok(LoweredValue::Scalar(
                    self.builder.ins().ireduce(ir::types::I8, found),
                ))
            }
            IntrinsicId::MapGet => {
                let map = self.scalar(values[0], span)?;
                let key = self.scalar(values[1], span)?;
                let result = self.leaf(NativeLeaf::MapGet, &[map, key], true)?;
                self.own_linear(result)
            }
            IntrinsicId::MapInsert => {
                let LoweredValue::Owned(map, slot) = values[0] else {
                    return Err(self.unsupported(span, "map insert requires owner"));
                };
                self.map_insert(values[0], values[1], values[2], false, span)?;
                self.clear_slot(slot);
                self.own_linear(map)
            }
            IntrinsicId::MapRemove => {
                let LoweredValue::Owned(map, slot) = values[0] else {
                    return Err(self.unsupported(span, "map remove requires owner"));
                };
                let key = self.scalar(values[1], span)?;
                let result = self.leaf(NativeLeaf::MapRemove, &[map, key], true)?;
                self.clear_slot(slot);
                self.own_linear(result)
            }
            IntrinsicId::MapFromLists => {
                let Type::Map(key, value) = self.types.resolve(result_type) else {
                    return Err(self.unsupported(span, "invalid map result layout"));
                };
                let key_strings = *key == TypeInterner::STRING;
                let value_owned = *value == TypeInterner::STRING || is_linear(self.types, *value);
                let LoweredValue::Owned(keys, key_slot) = values[0] else {
                    return Err(self.unsupported(span, "map keys require owning list"));
                };
                let LoweredValue::Owned(items, value_slot) = values[1] else {
                    return Err(self.unsupported(span, "map values require owning list"));
                };
                let key_flag = self
                    .builder
                    .ins()
                    .iconst(ir::types::I32, i64::from(key_strings));
                let value_flag = self
                    .builder
                    .ins()
                    .iconst(ir::types::I32, i64::from(value_owned));
                let result = self.leaf(
                    NativeLeaf::MapFromLists,
                    &[keys, items, key_flag, value_flag],
                    true,
                )?;
                self.clear_slot(key_slot);
                self.clear_slot(value_slot);
                self.own_linear(result)
            }
            _ => Err(self.unsupported(span, "map intrinsic")),
        }
    }
    fn list_intrinsic(
        &mut self,
        id: IntrinsicId,
        values: &[LoweredValue],
        result_type: TypeId,
        span: Span,
    ) -> Result<LoweredValue, CodegenError> {
        match id {
            IntrinsicId::ListNew => self.list_new(result_type, span),
            IntrinsicId::ListSum => {
                let list = self.scalar(values[0], span)?;
                Ok(LoweredValue::Scalar(self.leaf(
                    NativeLeaf::ListSumInt,
                    &[list],
                    true,
                )?))
            }
            IntrinsicId::ListLength => {
                let list = self.scalar(values[0], span)?;
                Ok(LoweredValue::Scalar(self.leaf(
                    NativeLeaf::ListLength,
                    &[list],
                    true,
                )?))
            }
            IntrinsicId::ListGetClone => {
                let list = self.scalar(values[0], span)?;
                let index = self.scalar(values[1], span)?;
                let value = self.leaf(NativeLeaf::ListGet, &[list, index], true)?;
                self.own_linear(value)
            }
            IntrinsicId::ListAppend => {
                self.list_append(values[0], values[1], span)?;
                let LoweredValue::Owned(value, slot) = values[0] else {
                    return Err(self.unsupported(span, "append requires owning list"));
                };
                self.clear_slot(slot);
                self.own_linear(value)
            }
            IntrinsicId::ListSort => {
                let Type::List(element) = self.types.resolve(result_type) else {
                    return Err(self.unsupported(span, "sort requires list result"));
                };
                let kind = crate::values::list_sort_kind(self.types, *element)
                    .ok_or_else(|| self.unsupported(span, "sort element type"))?;
                let LoweredValue::Owned(list, slot) = values[0] else {
                    return Err(self.unsupported(span, "sort requires owning list"));
                };
                let kind = self.builder.ins().iconst(ir::types::I32, kind as i64);
                let sorted = self.leaf(NativeLeaf::ListSort, &[list, kind], true)?;
                self.clear_slot(slot);
                self.own_linear(sorted)
            }
            _ => Err(self.unsupported(span, "list intrinsic")),
        }
    }
    fn set_intrinsic(
        &mut self,
        id: IntrinsicId,
        values: &[LoweredValue],
        result_type: TypeId,
        span: Span,
    ) -> Result<LoweredValue, CodegenError> {
        match id {
            IntrinsicId::SetNew => {
                let Type::Set(element) = self.types.resolve(result_type) else {
                    return Err(self.unsupported(span, "set result layout"));
                };
                let strings = self
                    .builder
                    .ins()
                    .iconst(ir::types::I32, i64::from(*element == TypeInterner::STRING));
                let value = self.leaf(NativeLeaf::SetNew, &[strings], true)?;
                self.own_linear(value)
            }
            IntrinsicId::SetLength => {
                let value = self.scalar(values[0], span)?;
                Ok(LoweredValue::Scalar(self.leaf(
                    NativeLeaf::SetLength,
                    &[value],
                    true,
                )?))
            }
            IntrinsicId::SetContains => {
                let value = self.scalar(values[0], span)?;
                let key = self.scalar(values[1], span)?;
                let found = self.leaf(NativeLeaf::SetContains, &[value, key], true)?;
                Ok(LoweredValue::Scalar(
                    self.builder.ins().ireduce(ir::types::I8, found),
                ))
            }
            IntrinsicId::SetAdd | IntrinsicId::SetRemove => {
                let LoweredValue::Owned(set, slot) = values[0] else {
                    return Err(self.unsupported(span, "set mutation requires owner"));
                };
                let (key, _) = self.payload_bits(values[1]);
                let leaf = if id == IntrinsicId::SetAdd {
                    NativeLeaf::SetAdd
                } else {
                    NativeLeaf::SetRemove
                };
                let value = self.leaf(leaf, &[set, key], true)?;
                self.clear_slot(slot);
                if id == IntrinsicId::SetAdd {
                    if let LoweredValue::Owned(_, key_slot) = values[1] {
                        self.clear_slot(key_slot);
                    }
                }
                self.own_linear(value)
            }
            _ => Err(self.unsupported(span, "set intrinsic")),
        }
    }
    pub(super) fn clear_slot(&mut self, slot: ir::StackSlot) {
        let zero = self.builder.ins().iconst(ir::types::I64, 0);
        self.builder.ins().stack_store(zero, slot, 0);
    }
    pub(super) fn own_linear(&mut self, value: Value) -> Result<LoweredValue, CodegenError> {
        let index = self.next_temporary;
        self.own(value)?;
        Ok(LoweredValue::Owned(value, self.temporary_slots[index]))
    }
    pub(super) fn argument(
        &mut self,
        expression: &Expression,
        borrowed: bool,
    ) -> Result<LoweredValue, CodegenError> {
        if borrowed && is_linear(self.types, expression.ty) {
            match &expression.kind {
                ExpressionKind::View(inner) => return self.argument(inner, true),
                ExpressionKind::Local(local) => {
                    let index = local.index() as usize;
                    let v =
                        if let Some(slot) = self.local_slots[index] {
                            self.builder.ins().stack_load(ir::types::I64, slot, 0)
                        } else {
                            self.builder.use_var(self.variables[index].ok_or_else(|| {
                                CodegenError::Backend("borrow has no place".into())
                            })?)
                        };
                    return Ok(LoweredValue::Scalar(v));
                }
                _ => {}
            }
        }
        self.expression(expression)
    }
    pub(super) fn drop_slot(&mut self, slot: ir::StackSlot) -> Result<(), CodegenError> {
        let value = self.builder.ins().stack_load(ir::types::I64, slot, 0);
        self.leaf(NativeLeaf::DropValue, &[value], false)?;
        let zero = self.builder.ins().iconst(ir::types::I64, 0);
        self.builder.ins().stack_store(zero, slot, 0);
        Ok(())
    }
    pub(super) fn drop_temporaries(&mut self) -> Result<(), CodegenError> {
        for &slot in &self.temporary_slots[..self.next_temporary] {
            self.drop_slot(slot)?;
        }
        self.next_temporary = 0;
        Ok(())
    }
    pub(super) fn drop_dead_locals(&mut self, live: &BTreeSet<usize>) -> Result<(), CodegenError> {
        for (i, slot) in self.local_slots.iter().enumerate() {
            if !live.contains(&i)
                && let Some(slot) = slot
            {
                self.drop_slot(*slot)?;
            }
        }
        Ok(())
    }
    pub(super) fn drop_all(&mut self) -> Result<(), CodegenError> {
        self.drop_temporaries()?;
        self.drop_dead_locals(&BTreeSet::new())
    }
    pub(super) fn literal(&mut self, text: &str) -> Result<LoweredValue, CodegenError> {
        let data = self
            .module
            .declare_anonymous_data(false, false)
            .map_err(|e| CodegenError::Backend(e.to_string()))?;
        let mut desc = cranelift_module::DataDescription::new();
        // Object formats require storage even for an empty literal.
        desc.define(if text.is_empty() {
            vec![0].into_boxed_slice()
        } else {
            text.as_bytes().into()
        });
        self.module
            .define_data(data, &desc)
            .map_err(|e| CodegenError::Backend(e.to_string()))?;
        let reference = self.module.declare_data_in_func(data, self.builder.func);
        let pointer = self.builder.ins().global_value(ir::types::I64, reference);
        let length = self.builder.ins().iconst(ir::types::I64, text.len() as i64);
        let value = self.leaf(NativeLeaf::Literal, &[pointer, length], true)?;
        self.own(value)
    }
    fn format_value(
        &mut self,
        value: LoweredValue,
        ty: TypeId,
        span: Span,
    ) -> Result<LoweredValue, CodegenError> {
        let kind = scalar_kind(self.types, ty, "format")?;
        if kind == ScalarKind::Nothing {
            return self.literal("nothing");
        }
        if kind == ScalarKind::String {
            return Ok(value);
        }
        let value = self.scalar(value, span)?;
        let (leaf, value) = match kind {
            ScalarKind::SignedInteger(bits) => (
                NativeLeaf::FromInt,
                if bits < 64 {
                    self.builder.ins().sextend(ir::types::I64, value)
                } else {
                    value
                },
            ),
            ScalarKind::UnsignedInteger(bits) => (
                NativeLeaf::FromUint,
                if bits < 64 {
                    self.builder.ins().uextend(ir::types::I64, value)
                } else {
                    value
                },
            ),
            ScalarKind::Float(bits) => (
                NativeLeaf::FromFloat,
                if bits < 64 {
                    self.builder.ins().fpromote(ir::types::F64, value)
                } else {
                    value
                },
            ),
            ScalarKind::Bool => (
                NativeLeaf::FromBool,
                self.builder.ins().uextend(ir::types::I32, value),
            ),
            _ => return Err(self.unsupported(span, "native formatting")),
        };
        let result = self.leaf(leaf, &[value], true)?;
        self.own(result)
    }
    fn concatenate(
        &mut self,
        left: LoweredValue,
        right: LoweredValue,
        span: Span,
    ) -> Result<LoweredValue, CodegenError> {
        let left = self.scalar(left, span)?;
        let right = self.scalar(right, span)?;
        let result = self.leaf(NativeLeaf::Concat, &[left, right], true)?;
        self.own(result)
    }
    pub(super) fn interpolate(
        &mut self,
        segments: &[StringSegment],
        span: Span,
    ) -> Result<LoweredValue, CodegenError> {
        let mut result = self.literal("")?;
        for segment in segments {
            let next = match segment {
                StringSegment::Text(text) => self.literal(text)?,
                StringSegment::Value(expr) => {
                    let v = self.expression(expr)?;
                    self.format_value(v, expr.ty, expr.span)?
                }
            };
            result = self.concatenate(result, next, span)?;
        }
        Ok(result)
    }
    pub(super) fn intrinsic(
        &mut self,
        id: IntrinsicId,
        args: &[Expression],
        order: &[usize],
        result_type: TypeId,
        span: Span,
    ) -> Result<LoweredValue, CodegenError> {
        let mut evaluated = vec![LoweredValue::Nothing; args.len()];
        for &index in order {
            evaluated[index] = self.argument(
                &args[index],
                jett_mir::move_values::intrinsic_borrows(id, index),
            )?;
        }
        if id == IntrinsicId::Range {
            let zero = self.builder.ins().iconst(ir::types::I64, 0);
            let one = self.builder.ins().iconst(ir::types::I64, 1);
            let mut native = evaluated
                .iter()
                .map(|v| self.scalar(*v, span))
                .collect::<Result<Vec<_>, _>>()?;
            if native.len() == 1 {
                native.insert(0, zero);
            }
            if native.len() == 2 {
                native.push(one);
            }
            let value = self.leaf(NativeLeaf::Range, &native, true)?;
            return self.own_linear(value);
        }
        if crate::values::list_intrinsic(id) {
            return self.list_intrinsic(id, &evaluated, result_type, span);
        }
        if crate::values::set_intrinsic(id) {
            return self.set_intrinsic(id, &evaluated, result_type, span);
        }
        if crate::values::map_intrinsic(id) {
            return self.map_intrinsic(id, &evaluated, result_type, span);
        }
        if let Some(leaf) = crate::values::bytes_leaf(id) {
            let arguments = evaluated
                .iter()
                .map(|v| self.scalar(*v, span))
                .collect::<Result<Vec<_>, _>>()?;
            let v = self.leaf(leaf, &arguments, true)?;
            return match leaf {
                NativeLeaf::BytesLength => Ok(LoweredValue::Scalar(v)),
                NativeLeaf::BytesToHex => self.own(v),
                _ => self.own_linear(v),
            };
        }
        if let Some(leaf) = crate::values::math_leaf(id, args.first().map(|a| a.ty)) {
            let arguments = evaluated
                .iter()
                .map(|v| self.scalar(*v, span))
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(LoweredValue::Scalar(self.leaf(leaf, &arguments, true)?));
        }
        if let Some(leaf) = crate::values::string_leaf(id) {
            let native_args = evaluated
                .iter()
                .map(|v| self.scalar(*v, span))
                .collect::<Result<Vec<_>, _>>()?;
            let value = self.leaf(leaf, &native_args, true)?;
            return match leaf {
                NativeLeaf::CharCount | NativeLeaf::StringCount => Ok(LoweredValue::Scalar(value)),
                NativeLeaf::StringIndexOf => self.own_linear(value),
                NativeLeaf::IsAlpha | NativeLeaf::IsNumeric => Ok(LoweredValue::Scalar(
                    self.builder.ins().ireduce(ir::types::I8, value),
                )),
                _ => self.own(value),
            };
        }
        match id {
            IntrinsicId::BitfieldToBytes => self.encode_bitfield(evaluated[0], args[0].ty, span),
            IntrinsicId::BitfieldFromBytes => self.decode_bitfield(evaluated[0], result_type, span),
            IntrinsicId::StringFromInt64
            | IntrinsicId::StringFromUint64
            | IntrinsicId::StringFromFloat64
            | IntrinsicId::StringFromBool => self.format_value(evaluated[0], args[0].ty, span),
            IntrinsicId::StdoutWrite => {
                let authority = self.scalar(evaluated[0], span)?;
                let text = self.scalar(evaluated[1], span)?;
                self.leaf(NativeLeaf::Stdout, &[authority, text], true)?;
                Ok(LoweredValue::Nothing)
            }
            IntrinsicId::Print | IntrinsicId::Println => {
                // Evaluate every argument before the first output effect.
                let mut output = self.literal("")?;
                for (index, (arg, value)) in args.iter().zip(evaluated).enumerate() {
                    if index != 0 {
                        let space = self.literal(" ")?;
                        output = self.concatenate(output, space, span)?;
                    }
                    let text = self.format_value(value, arg.ty, arg.span)?;
                    output = self.concatenate(output, text, span)?;
                }
                if id == IntrinsicId::Println {
                    let newline = self.literal("\n")?;
                    output = self.concatenate(output, newline, span)?;
                }
                let output = self.scalar(output, span)?;
                self.leaf(NativeLeaf::DebugPrint, &[output], true)?;
                Ok(LoweredValue::Nothing)
            }
            _ => Err(self.unsupported(span, "runtime intrinsic")),
        }
    }
}
