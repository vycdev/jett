use super::*;
use jett_hir::{InterfaceFunctionAdapter, IntrinsicId, MapEntry, StringSegment};
use jett_mir::move_values::is_erased_interface;
use jett_runtime::native_abi::values::interface_conversion::NativeInterfaceConversion;
use jett_types::BitfieldFieldKind;
use std::collections::BTreeSet;

fn append_builder_debug_layout(
    layout: &mut Vec<u8>,
    types: &TypeInterner,
    ty: TypeId,
) -> Result<(), CodegenError> {
    let debug = super::debug::debug_layout(types, ty).unwrap_or_default();
    let length = u32::try_from(debug.len())
        .map_err(|_| CodegenError::Backend("reflected field debug layout is too long".into()))?;
    layout.extend_from_slice(&length.to_le_bytes());
    layout.extend_from_slice(&debug);
    Ok(())
}

fn interface_conversion(
    types: &TypeInterner,
    source: TypeId,
    target: TypeId,
    adapters: &[InterfaceFunctionAdapter],
) -> Option<jett_runtime::native_abi::values::interface_conversion::NativeInterfaceConversion> {
    use jett_runtime::native_abi::values::interface_conversion::NativeInterfaceConversion as C;
    let source_rep = representation_type(types, source);
    let target_rep = representation_type(types, target);
    let source_erased = is_erased_interface(types, source);
    let target_erased = is_erased_interface(types, target);
    let boxed_owner = interface_box_owner(types, source, target);
    let owned = |ty| is_copy_owned(types, ty) || is_linear(types, ty);
    if (source_rep == target_rep && source_erased == target_erased) || source == TypeInterner::NEVER
    {
        return Some(C::Copy {
            owned: owned(target),
        });
    }
    Some(
        match (types.resolve(source_rep), types.resolve(target_rep)) {
            (_, _) if target_erased => C::Box {
                concrete: boxed_owner.index() as u64,
                owned: owned(source),
                nothing: source_rep == TypeInterner::NOTHING,
                layout: debug::debug_layout(types, boxed_owner)?,
            },
            (_, _) if source_erased => C::Unbox {
                concrete: target.index() as u64,
                owned: owned(target),
                nothing: target_rep == TypeInterner::NOTHING,
            },
            (Type::List(a), Type::List(b)) => {
                C::List(Box::new(interface_conversion(types, *a, *b, adapters)?))
            }
            (Type::Optional(a), Type::Optional(b)) => {
                C::Optional(Box::new(interface_conversion(types, *a, *b, adapters)?))
            }
            (Type::Result(a, b), Type::Result(c, d)) => C::Result(
                Box::new(interface_conversion(types, *a, *c, adapters)?),
                Box::new(interface_conversion(types, *b, *d, adapters)?),
            ),
            (Type::Map(ak, av), Type::Map(bk, bv)) if ak == bk || *ak == TypeInterner::NEVER => {
                C::Map(
                    Box::new(interface_conversion(types, *av, *bv, adapters)?),
                    is_string(types, *bk),
                )
            }
            (Type::Function { .. }, Type::Function { .. }) => C::FunctionAdapter {
                code: adapters
                    .iter()
                    .find(|entry| entry.source == source_rep && entry.target == target_rep)?
                    .function
                    .index() as u64,
            },
            _ => return None,
        },
    )
}

fn interface_box_owner(types: &TypeInterner, mut source: TypeId, mut target: TypeId) -> TypeId {
    // Shared outer secret qualifiers protect the destination rather than
    // naming its concrete implementation. An unqualified interface destination
    // can instead select an explicit implementation for the secret source.
    while let Type::Secret(inner) = types.resolve(target) {
        target = *inner;
        if let Type::Secret(inner) = types.resolve(source) {
            source = *inner;
        }
    }
    source
}

fn reflected_field_types_compatible(
    types: &TypeInterner,
    actual: TypeId,
    requested: TypeId,
) -> bool {
    reflected_field_carriers_compatible(types, actual, requested, true, types.len())
}

fn reflected_field_carriers_compatible(
    types: &TypeInterner,
    actual: TypeId,
    requested: TypeId,
    root: bool,
    budget: usize,
) -> bool {
    if budget == 0
        || actual.index() as usize >= types.len()
        || requested.index() as usize >= types.len()
    {
        return false;
    }
    if actual == requested {
        return true;
    }
    // Only the original root read knows how to box an interface refinement.
    // A collection clone cannot manufacture nested interface conversions.
    if !root && is_erased_interface(types, actual) != is_erased_interface(types, requested) {
        return false;
    }
    let Some(actual) = reflected_field_carrier(types, actual) else {
        return false;
    };
    let Some(requested) = reflected_field_carrier(types, requested) else {
        return false;
    };
    let child =
        |a, b, at_root| reflected_field_carriers_compatible(types, a, b, at_root, budget - 1);
    match (types.resolve(actual), types.resolve(requested)) {
        // Match every secrecy layer rather than comparing only an outer flag.
        (Type::Secret(a), Type::Secret(b)) => child(*a, *b, root),
        (Type::Secret(_), _) | (_, Type::Secret(_)) => false,
        (Type::List(a), Type::List(b))
        | (Type::Set(a), Type::Set(b))
        | (Type::Optional(a), Type::Optional(b)) => child(*a, *b, false),
        (Type::Map(a, b), Type::Map(c, d)) | (Type::Result(a, b), Type::Result(c, d)) => {
            child(*a, *c, false) && child(*b, *d, false)
        }
        // Canonical nominal identity, callable signatures and primitive widths
        // remain exact. Equal machine words or field layouts are not casts.
        _ => actual == requested,
    }
}

fn reflected_field_carrier(types: &TypeInterner, mut ty: TypeId) -> Option<TypeId> {
    for _ in 0..types.len() {
        if ty.index() as usize >= types.len() {
            return None;
        }
        match types.resolve(ty) {
            Type::Refinement { base, .. } => ty = *base,
            _ => return Some(ty),
        }
    }
    None
}

fn reflected_field_needs_interface_box(
    types: &TypeInterner,
    actual: TypeId,
    requested: TypeId,
) -> bool {
    reflected_field_types_compatible(types, actual, requested)
        && is_erased_interface(types, requested)
        && !is_erased_interface(types, actual)
}

#[derive(Clone, Copy)]
struct ReflectedFieldConversion {
    layout: Value,
    length: Value,
}

impl Translator<'_, '_> {
    pub(super) fn interface_coerce(
        &mut self,
        expression: &Expression,
        target: TypeId,
        borrowed: bool,
        adapters: &[InterfaceFunctionAdapter],
    ) -> Result<LoweredValue, CodegenError> {
        let source = representation_type(self.types, expression.ty);
        let target_representation = representation_type(self.types, target);
        let source_erased = is_erased_interface(self.types, expression.ty);
        let target_erased = is_erased_interface(self.types, target);
        if source == target_representation && source_erased == target_erased {
            return self.argument(expression, borrowed);
        }
        if target_erased {
            let value = self.argument(expression, borrowed)?;
            let depth = if source == TypeInterner::NOTHING {
                self.scalar(value, expression.span)?
            } else {
                match value {
                    LoweredValue::ScalarTask(_, depth) => depth,
                    _ => self.builder.ins().iconst(ir::types::I64, 0),
                }
            };
            let bits = if source == TypeInterner::NOTHING {
                self.builder.ins().iconst(ir::types::I64, 0)
            } else {
                self.payload_bits(value).0
            };
            let owned =
                is_copy_owned(self.types, expression.ty) || is_linear(self.types, expression.ty);
            let owned = self.builder.ins().iconst(ir::types::I32, i64::from(owned));
            let boxed_owner = interface_box_owner(self.types, expression.ty, target);
            let identity = self
                .builder
                .ins()
                .iconst(ir::types::I64, boxed_owner.index() as i64);
            let layout = debug::debug_layout(self.types, boxed_owner).ok_or_else(|| {
                self.unsupported(expression.span, "interface payload debug layout")
            })?;
            let (layout, length) = self.static_data(&layout)?;
            let result = self.leaf(
                NativeLeaf::InterfaceBox,
                &[identity, bits, owned, depth, layout, length],
                true,
            )?;
            return self.own(result);
        }
        if source_erased {
            let value = self.argument(expression, true)?;
            let value = self.scalar(value, expression.span)?;
            let identity = self
                .builder
                .ins()
                .iconst(ir::types::I64, target.index() as i64);
            let bits = self.leaf(NativeLeaf::InterfaceUnbox, &[value, identity], true)?;
            if target_representation == TypeInterner::NOTHING {
                let depth = self.leaf(NativeLeaf::InterfacePendingDepth, &[value], true)?;
                return Ok(LoweredValue::Scalar(depth));
            }
            let unpacked = self.unpack_payload(bits, target, expression.span)?;
            if is_task_scalar(self.types, target)? {
                let depth = self.leaf(NativeLeaf::InterfacePendingDepth, &[value], true)?;
                return Ok(LoweredValue::ScalarTask(
                    self.scalar(unpacked, expression.span)?,
                    depth,
                ));
            }
            return Ok(unpacked);
        }
        let conversion = interface_conversion(self.types, expression.ty, target, adapters)
            .ok_or_else(|| {
                self.unsupported(expression.span, "nested interface-compatible conversion")
            })?;
        let value = self.argument(expression, borrowed)?;
        let value = self.scalar(value, expression.span)?;
        let (mut bytes, functions) = conversion.encode_with_function_offsets();
        // Relocations supply addresses. Never leave a function ID in the object.
        for (offset, _) in &functions {
            bytes[*offset..*offset + 8].fill(0);
        }
        let (layout, length) = self.static_data_with_functions(&bytes, &functions)?;
        let output = self.leaf(NativeLeaf::InterfaceConvert, &[value, layout, length], true)?;
        self.own_linear(output)
    }

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
        if let Some(resource) = self.resource {
            self.builder
                .ins()
                .brif(status, resource.failure, &[status.into()], success, &[]);
        } else {
            self.builder
                .ins()
                .brif(status, self.failure_block, &[], success, &[]);
        }
        self.builder.switch_to_block(success);
        Ok(())
    }
    pub(super) fn own(&mut self, value: Value) -> Result<LoweredValue, CodegenError> {
        let slot = self
            .temporary_slots
            .get(self.next_temporary)
            .ok_or_else(|| {
                CodegenError::Backend(format!(
                    "MIR temporary ownership bound exceeded in `{}`: needed at least {}, planned {}",
                    self.symbol,
                    self.next_temporary + 1,
                    self.temporary_slots.len()
                ))
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
        self.initialize_struct(handle, fields, evaluation_order)?;
        let _ = span;
        Ok(record)
    }
    fn initialize_struct(
        &mut self,
        handle: Value,
        fields: &[Expression],
        evaluation_order: &[usize],
    ) -> Result<(), CodegenError> {
        for &index in evaluation_order {
            let value = self.expression(&fields[index])?;
            let (bits, owned) = self.payload_bits(value);
            let index = self.builder.ins().iconst(ir::types::I64, index as i64);
            if let LoweredValue::ScalarTask(_, depth) = value {
                self.leaf(
                    NativeLeaf::StructInitScalarTask,
                    &[handle, index, bits, depth],
                    true,
                )?;
            } else {
                let owned = self.builder.ins().iconst(ir::types::I32, i64::from(owned));
                self.leaf(NativeLeaf::StructInit, &[handle, index, bits, owned], true)?;
            }
            if let LoweredValue::Owned(_, slot) = value {
                self.clear_slot(slot);
            }
        }
        Ok(())
    }
    pub(super) fn construct_validated_bitfield(
        &mut self,
        bitfield_type: TypeId,
        fields: &[Expression],
        evaluation_order: &[usize],
        span: Span,
    ) -> Result<LoweredValue, CodegenError> {
        let Type::Bitfield(id) = self.types.resolve(bitfield_type) else {
            return Err(self.unsupported(span, "bitfield construction type"));
        };
        let layout = self.types.resolve_bitfield(*id).clone();
        let mut data = b"JC\x03".to_vec();
        fn name(data: &mut Vec<u8>, value: &str) -> Result<(), CodegenError> {
            let length = u32::try_from(value.len())
                .map_err(|_| CodegenError::Backend("bitfield layout name is too long".into()))?;
            data.extend_from_slice(&length.to_le_bytes());
            data.extend_from_slice(value.as_bytes());
            Ok(())
        }
        name(&mut data, &layout.name)?;
        let count = u32::try_from(layout.fields.len())
            .map_err(|_| CodegenError::Backend("too many bitfield fields".into()))?;
        data.extend_from_slice(&count.to_le_bytes());
        for field in &layout.fields {
            name(&mut data, &field.name)?;
            let field_type = self.types.type_name(field.ty);
            name(&mut data, &field_type)?;
            name(&mut data, &field_type)?;
        }
        for field in &layout.fields {
            match field.kind {
                BitfieldFieldKind::Payload => data.push(0),
                BitfieldFieldKind::Bits { width } => {
                    let kind = match self.types.resolve(field.ty) {
                        Type::Int64 => 1,
                        Type::Uint64 => 2,
                        Type::Enum(_) => 3,
                        _ => return Err(self.unsupported(span, "bitfield field validation")),
                    };
                    data.push(kind);
                    data.extend_from_slice(&u32::from(width).to_le_bytes());
                    if let Type::Enum(enum_id) = self.types.resolve(field.ty) {
                        let definition = self.types.resolve_enum(*enum_id);
                        name(&mut data, &definition.name)?;
                        let variants = u32::try_from(definition.variants.len())
                            .map_err(|_| self.unsupported(span, "bitfield enum variants"))?;
                        data.extend_from_slice(&variants.to_le_bytes());
                        for variant in &definition.variants {
                            name(&mut data, &variant.name)?;
                            data.extend_from_slice(&variant.discriminant.to_le_bytes());
                        }
                    }
                }
            }
        }
        let (pointer, length) = self.static_data(&data)?;
        let handle = self.leaf(NativeLeaf::BuilderNew, &[pointer, length], true)?;
        let builder = self.own_linear(handle)?;
        self.initialize_struct(handle, fields, evaluation_order)?;
        let (owner_pointer, owner_length) = self.static_bytes(&layout.name)?;
        let result = self.leaf(
            NativeLeaf::BuilderFinish,
            &[handle, owner_pointer, owner_length],
            true,
        )?;
        if let LoweredValue::Owned(_, slot) = builder {
            self.clear_slot(slot);
        }
        self.own_linear(result)
    }
    pub(super) fn construct_tagged_record(
        &mut self,
        tag: u32,
        payloads: &[Expression],
        evaluation_order: impl IntoIterator<Item = usize>,
        span: Span,
    ) -> Result<LoweredValue, CodegenError> {
        let count = i64::try_from(payloads.len() + 1)
            .map_err(|_| self.unsupported(span, "enum payload count"))?;
        let count = self.builder.ins().iconst(ir::types::I64, count);
        let handle = self.leaf(NativeLeaf::StructNew, &[count], true)?;
        let record = self.own_linear(handle)?;
        let zero = self.builder.ins().iconst(ir::types::I64, 0);
        let tag = self.builder.ins().iconst(ir::types::I64, i64::from(tag));
        let borrowed = self.builder.ins().iconst(ir::types::I32, 0);
        self.leaf(NativeLeaf::StructInit, &[handle, zero, tag, borrowed], true)?;
        for field in evaluation_order {
            let value = self.expression(&payloads[field])?;
            let (bits, owned) = self.payload_bits(value);
            let index = self.builder.ins().iconst(ir::types::I64, field as i64 + 1);
            if let LoweredValue::ScalarTask(_, depth) = value {
                self.leaf(
                    NativeLeaf::StructInitScalarTask,
                    &[handle, index, bits, depth],
                    true,
                )?;
            } else {
                let owned = self.builder.ins().iconst(ir::types::I32, i64::from(owned));
                self.leaf(NativeLeaf::StructInit, &[handle, index, bits, owned], true)?;
            }
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
        let message = format!("{}.to_bytes expects a {} value", layout.name, layout.name);
        self.reject_pending_handle(value, 4, &message, span)?;
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
                    let prefix = format!("bitfield '{}' field '{}': ", layout.name, field.name);
                    let (pointer, length) = self.static_bytes(&prefix)?;
                    self.leaf(
                        NativeLeaf::BitfieldCheckPayloadPending,
                        &[bits, pointer, length],
                        true,
                    )?;
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
        let message = format!("{}.from_bytes expects a bytes argument", layout.name);
        self.reject_pending_handle(value, 1, &message, span)?;
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
    pub(super) fn project_field(
        &mut self,
        base: &Expression,
        field: jett_hir::FieldId,
        ty: TypeId,
        span: Span,
        borrowed: bool,
    ) -> Result<LoweredValue, CodegenError> {
        let index = if matches!(
            self.types.resolve(representation_type(self.types, base.ty)),
            Type::MachineState { .. }
        ) {
            field.index() + 1
        } else {
            field.index()
        };
        self.struct_field(base, index, ty, span, borrowed)
    }
    pub(super) fn clone_linear(
        &mut self,
        borrowed: LoweredValue,
        ty: TypeId,
        span: Span,
    ) -> Result<LoweredValue, CodegenError> {
        let value = self.scalar(borrowed, span)?;
        let cloned = self.clone_linear_handle(value, ty)?;
        self.own_linear(cloned)
    }
    pub(super) fn clone_linear_handle(
        &mut self,
        value: Value,
        ty: TypeId,
    ) -> Result<Value, CodegenError> {
        let leaf = match self.types.resolve(representation_type(self.types, ty)) {
            Type::Bytes => NativeLeaf::BytesClone,
            Type::List(_) => NativeLeaf::ListClone,
            Type::Set(_) => NativeLeaf::SetClone,
            Type::Map(..) => NativeLeaf::MapClone,
            Type::Struct(_) | Type::Enum(_) | Type::Bitfield(_) | Type::TypeConstruction => {
                NativeLeaf::StructClone
            }
            Type::Machine(_) | Type::MachineState { .. } | Type::Interface(_) => {
                NativeLeaf::StructClone
            }
            _ => NativeLeaf::SumClone,
        };
        self.leaf(leaf, &[value], true)
    }
    pub(super) fn check_struct_pending_access(
        &mut self,
        value: Value,
        owner_type: TypeId,
        message: &str,
        render: bool,
    ) -> Result<(), CodegenError> {
        let layout = super::debug::debug_layout(self.types, owner_type).unwrap_or_default();
        let (layout_pointer, layout_length) = self.static_data(&layout)?;
        let (message_pointer, message_length) = self.static_bytes(message)?;
        let render = self.builder.ins().iconst(ir::types::I32, i64::from(render));
        self.leaf(
            NativeLeaf::StructPendingAccessCheck,
            &[
                value,
                layout_pointer,
                layout_length,
                message_pointer,
                message_length,
                render,
            ],
            true,
        )?;
        Ok(())
    }

    fn struct_field(
        &mut self,
        base: &Expression,
        index: u32,
        ty: TypeId,
        span: Span,
        borrowed: bool,
    ) -> Result<LoweredValue, CodegenError> {
        let parent = self.argument(base, true)?;
        let parent = self.scalar(parent, span)?;
        self.check_struct_pending_access(
            parent,
            base.ty,
            "field access is not supported on ",
            true,
        )?;
        let index = self.builder.ins().iconst(ir::types::I64, i64::from(index));
        let bits = self.leaf(NativeLeaf::StructField, &[parent, index], true)?;
        if is_linear(self.types, ty) {
            // A view keeps the owning root live; an owned read deep-clones the
            // field before full-expression cleanup releases the root.
            let value = LoweredValue::Scalar(bits);
            return if borrowed {
                Ok(value)
            } else {
                self.clone_linear(value, ty, span)
            };
        }
        if is_function(self.types, ty) {
            return if borrowed {
                Ok(LoweredValue::Scalar(bits))
            } else {
                let cloned = self.leaf(NativeLeaf::StructClone, &[bits], true)?;
                self.own(cloned)
            };
        }
        let bits = if is_string(self.types, ty) {
            self.leaf(NativeLeaf::Retain, &[bits], true)?
        } else {
            bits
        };
        let value = self.unpack_payload(bits, ty, span)?;
        if is_task_scalar(self.types, ty)? {
            let depth = self.leaf(NativeLeaf::StructFieldPendingDepth, &[parent, index], true)?;
            Ok(LoweredValue::ScalarTask(self.scalar(value, span)?, depth))
        } else {
            Ok(value)
        }
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
            self.nothing()
        };
        self.construct_sum_value(success, value, span)
    }
    pub(super) fn construct_sum_value(
        &mut self,
        success: bool,
        value: LoweredValue,
        span: Span,
    ) -> Result<LoweredValue, CodegenError> {
        if let LoweredValue::ScalarTask(bits, depth) = value {
            let (bits, _) = self.payload_bits(LoweredValue::Scalar(bits));
            let tag = self
                .builder
                .ins()
                .iconst(ir::types::I32, i64::from(success));
            let sum = self.leaf(NativeLeaf::SumNewScalarTask, &[tag, bits, depth], true)?;
            return self.own_linear(sum);
        }
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
    pub(super) fn payload_bits(&mut self, value: LoweredValue) -> (Value, bool) {
        match value {
            LoweredValue::Owned(v, _) => (v, true),
            LoweredValue::Scalar(v) | LoweredValue::ScalarTask(v, _) => {
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
        if is_copy_owned(self.types, ty) || is_linear(self.types, ty) {
            return self.own(bits);
        }
        let native = self.required_clif_type(ty, span)?;
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
        let owned = is_copy_owned(self.types, *element) || is_linear(self.types, *element);
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
        if let LoweredValue::ScalarTask(_, depth) = value {
            self.leaf(
                NativeLeaf::ListAppendScalarTask,
                &[handle, bits, depth],
                true,
            )?;
        } else {
            self.leaf(NativeLeaf::ListAppend, &[handle, bits], true)?;
        }
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
            .iconst(ir::types::I32, i64::from(is_string(self.types, *key)));
        let value_owned = self.builder.ins().iconst(
            ir::types::I32,
            i64::from(is_copy_owned(self.types, *value) || is_linear(self.types, *value)),
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
        let key_depth = if let LoweredValue::ScalarTask(_, depth) = key {
            depth
        } else {
            self.builder.ins().iconst(ir::types::I64, 0)
        };
        let value_depth = if let LoweredValue::ScalarTask(_, depth) = value {
            depth
        } else {
            self.builder.ins().iconst(ir::types::I64, 0)
        };
        self.leaf(
            if literal {
                NativeLeaf::MapAppendLiteralScalarTasks
            } else {
                NativeLeaf::MapInsertScalarTasks
            },
            &[handle, key_bits, key_depth, value_bits, value_depth],
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
                let (key, _) = self.payload_bits(values[1]);
                let found = if let LoweredValue::ScalarTask(_, depth) = values[1] {
                    self.leaf(NativeLeaf::MapHasScalarTask, &[map, key, depth], true)?
                } else {
                    self.leaf(NativeLeaf::MapHas, &[map, key], true)?
                };
                Ok(LoweredValue::Scalar(
                    self.builder.ins().ireduce(ir::types::I8, found),
                ))
            }
            IntrinsicId::MapGet => {
                let map = self.scalar(values[0], span)?;
                let (key, _) = self.payload_bits(values[1]);
                let result = if let LoweredValue::ScalarTask(_, depth) = values[1] {
                    self.leaf(NativeLeaf::MapGetScalarTask, &[map, key, depth], true)?
                } else {
                    self.leaf(NativeLeaf::MapGet, &[map, key], true)?
                };
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
                let (key, _) = self.payload_bits(values[1]);
                let result = if let LoweredValue::ScalarTask(_, depth) = values[1] {
                    self.leaf(NativeLeaf::MapRemoveScalarTask, &[map, key, depth], true)?
                } else {
                    self.leaf(NativeLeaf::MapRemove, &[map, key], true)?
                };
                self.clear_slot(slot);
                self.own_linear(result)
            }
            IntrinsicId::MapFromLists => {
                let Type::Map(key, value) = self.types.resolve(result_type) else {
                    return Err(self.unsupported(span, "invalid map result layout"));
                };
                let key_strings = is_string(self.types, *key);
                let value_owned =
                    is_copy_owned(self.types, *value) || is_linear(self.types, *value);
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
        args: &[Expression],
        values: &[LoweredValue],
        result_type: TypeId,
        span: Span,
    ) -> Result<LoweredValue, CodegenError> {
        let check_kind = match id {
            IntrinsicId::ListLength => Some(0),
            IntrinsicId::ListAppend => Some(1),
            IntrinsicId::ListGetClone => Some(2),
            IntrinsicId::ListInsertAt => Some(3),
            IntrinsicId::ListRemoveAt => Some(4),
            IntrinsicId::ListSort => Some(5),
            IntrinsicId::ListSortByIndex => Some(6),
            IntrinsicId::ListIsSorted => Some(7),
            IntrinsicId::ListSwap => Some(8),
            _ => None,
        };
        if let Some(check_kind) = check_kind {
            let list = self.scalar(values[0], span)?;
            let kind = self.builder.ins().iconst(ir::types::I32, check_kind);
            let zero = self.builder.ins().iconst(ir::types::I64, 0);
            let indexed = matches!(
                id,
                IntrinsicId::ListGetClone
                    | IntrinsicId::ListInsertAt
                    | IntrinsicId::ListRemoveAt
                    | IntrinsicId::ListSortByIndex
                    | IntrinsicId::ListSwap
            );
            let first_depth = if indexed {
                match values[1] {
                    LoweredValue::ScalarTask(_, depth) => depth,
                    _ => zero,
                }
            } else {
                zero
            };
            let second_depth = if id == IntrinsicId::ListSwap {
                match values[2] {
                    LoweredValue::ScalarTask(_, depth) => depth,
                    _ => zero,
                }
            } else {
                zero
            };
            self.leaf(
                NativeLeaf::ListCheckArguments,
                &[list, kind, first_depth, second_depth],
                true,
            )?;
        }
        match id {
            IntrinsicId::ListNew => self.list_new(result_type, span),
            IntrinsicId::ListSum => {
                let list = self.scalar(values[0], span)?;
                let kind = crate::values::list_sum_kind(self.types, result_type)
                    .ok_or_else(|| self.unsupported(span, "invalid numeric list sum type"))?;
                let kind = self.builder.ins().iconst(ir::types::I32, kind as i64);
                let bits = self.leaf(NativeLeaf::ListSumPrimitive, &[list, kind], true)?;
                self.unpack_payload(bits, result_type, span)
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
            IntrinsicId::ListInsertAt => {
                let LoweredValue::Owned(list, list_slot) = values[0] else {
                    return Err(self.unsupported(span, "insert requires owning list"));
                };
                let index = self.scalar(values[1], span)?;
                let (bits, _) = self.payload_bits(values[2]);
                let inserted = if let LoweredValue::ScalarTask(_, depth) = values[2] {
                    self.leaf(
                        NativeLeaf::ListInsertAtScalarTask,
                        &[list, index, bits, depth],
                        true,
                    )?
                } else {
                    self.leaf(NativeLeaf::ListInsertAt, &[list, index, bits], true)?
                };
                self.clear_slot(list_slot);
                if let LoweredValue::Owned(_, value_slot) = values[2] {
                    self.clear_slot(value_slot);
                }
                self.own_linear(inserted)
            }
            IntrinsicId::ListRemoveAt => {
                let LoweredValue::Owned(list, slot) = values[0] else {
                    return Err(self.unsupported(span, "remove requires owning list"));
                };
                let index = self.scalar(values[1], span)?;
                let removed = self.leaf(NativeLeaf::ListRemoveAt, &[list, index], true)?;
                self.clear_slot(slot);
                self.own_linear(removed)
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
            IntrinsicId::ListSortByIndex => {
                let Type::List(row) = self.types.resolve(result_type) else {
                    return Err(self.unsupported(span, "indexed sort requires list result"));
                };
                let Type::List(element) = self.types.resolve(*row) else {
                    return Err(self.unsupported(span, "indexed sort requires row lists"));
                };
                let kind = crate::values::list_comparison_kind(self.types, *element)
                    .map_or(-1, |kind| kind as i64);
                let LoweredValue::Owned(list, slot) = values[0] else {
                    return Err(self.unsupported(span, "indexed sort requires owning list"));
                };
                let index = self.scalar(values[1], span)?;
                let kind = self.builder.ins().iconst(ir::types::I32, kind);
                let sorted = self.leaf(NativeLeaf::ListSortByIndex, &[list, index, kind], true)?;
                self.clear_slot(slot);
                self.own_linear(sorted)
            }
            IntrinsicId::ListIsSorted => {
                let Type::List(element) = self.types.resolve(args[0].ty) else {
                    return Err(self.unsupported(span, "sortedness requires a list"));
                };
                let kind = crate::values::list_comparison_kind(self.types, *element)
                    .map_or(-1, |kind| kind as i64);
                let list = self.scalar(values[0], span)?;
                let kind = self.builder.ins().iconst(ir::types::I32, kind);
                let sorted = self.leaf(NativeLeaf::ListIsSorted, &[list, kind], true)?;
                Ok(LoweredValue::Scalar(
                    self.builder.ins().ireduce(ir::types::I8, sorted),
                ))
            }
            IntrinsicId::ListSwap => {
                let LoweredValue::Owned(list, slot) = values[0] else {
                    return Err(self.unsupported(span, "swap requires owning list"));
                };
                let first = self.scalar(values[1], span)?;
                let second = self.scalar(values[2], span)?;
                let swapped = self.leaf(NativeLeaf::ListSwap, &[list, first, second], true)?;
                self.clear_slot(slot);
                self.own_linear(swapped)
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
                    .iconst(ir::types::I32, i64::from(is_string(self.types, *element)));
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
                let (key, _) = self.payload_bits(values[1]);
                let found = if let LoweredValue::ScalarTask(_, depth) = values[1] {
                    self.leaf(
                        NativeLeaf::SetContainsScalarTask,
                        &[value, key, depth],
                        true,
                    )?
                } else {
                    self.leaf(NativeLeaf::SetContains, &[value, key], true)?
                };
                Ok(LoweredValue::Scalar(
                    self.builder.ins().ireduce(ir::types::I8, found),
                ))
            }
            IntrinsicId::SetAdd | IntrinsicId::SetRemove => {
                let LoweredValue::Owned(set, slot) = values[0] else {
                    return Err(self.unsupported(span, "set mutation requires owner"));
                };
                let (key, _) = self.payload_bits(values[1]);
                let value = if let LoweredValue::ScalarTask(_, depth) = values[1] {
                    let leaf = if id == IntrinsicId::SetAdd {
                        NativeLeaf::SetAddScalarTask
                    } else {
                        NativeLeaf::SetRemoveScalarTask
                    };
                    self.leaf(leaf, &[set, key, depth], true)?
                } else {
                    let leaf = if id == IntrinsicId::SetAdd {
                        NativeLeaf::SetAdd
                    } else {
                        NativeLeaf::SetRemove
                    };
                    self.leaf(leaf, &[set, key], true)?
                };
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
    pub(super) fn own_copy_value(
        &mut self,
        value: LoweredValue,
        ty: TypeId,
        span: Span,
    ) -> Result<LoweredValue, CodegenError> {
        if matches!(value, LoweredValue::Owned(..)) {
            return Ok(value);
        }
        let bits = self.scalar(value, span)?;
        let leaf = if is_function(self.types, ty) {
            NativeLeaf::StructClone
        } else {
            NativeLeaf::Retain
        };
        let owned = self.leaf(leaf, &[bits], true)?;
        self.own(owned)
    }
    pub(super) fn argument(
        &mut self,
        expression: &Expression,
        borrowed: bool,
    ) -> Result<LoweredValue, CodegenError> {
        if borrowed
            && let ExpressionKind::Coarsen(inner)
            | ExpressionKind::Declassify(inner)
            | ExpressionKind::RefinementValidated(inner) = &expression.kind
        {
            return self.argument(inner, true);
        }
        if borrowed
            && let ExpressionKind::InterfaceCoerce {
                value: inner,
                adapters,
            } = &expression.kind
        {
            return self.interface_coerce(inner, expression.ty, true, adapters);
        }
        if borrowed
            && (is_linear(self.types, expression.ty) || is_copy_owned(self.types, expression.ty))
        {
            match &expression.kind {
                ExpressionKind::View(inner) => return self.argument(inner, true),
                ExpressionKind::Field { base, field, .. } => {
                    return self.project_field(base, *field, expression.ty, expression.span, true);
                }
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
                    self.expect_machine_state(local.index() as usize, expression.ty, v)?;
                    return Ok(LoweredValue::Scalar(v));
                }
                _ => {}
            }
        }
        self.expression(expression)
    }
    pub(super) fn expect_machine_state(
        &mut self,
        local_index: usize,
        expression_type: TypeId,
        value: Value,
    ) -> Result<(), CodegenError> {
        if let (
            Type::Machine(machine),
            Type::MachineState {
                machine: narrowed,
                state,
            },
        ) = (
            self.types.resolve(self.local_types[local_index].ty),
            self.types.resolve(expression_type),
        ) && machine == narrowed
        {
            let expected = self
                .builder
                .ins()
                .iconst(ir::types::I64, i64::from(state.index()));
            self.leaf(NativeLeaf::MachineExpectState, &[value, expected], true)?;
        }
        Ok(())
    }
    pub(super) fn drop_slot(&mut self, slot: ir::StackSlot) -> Result<(), CodegenError> {
        let value = self.builder.ins().stack_load(ir::types::I64, slot, 0);
        self.leaf(NativeLeaf::DropValue, &[value], false)?;
        let zero = self.builder.ins().iconst(ir::types::I64, 0);
        self.builder.ins().stack_store(zero, slot, 0);
        Ok(())
    }
    pub(super) fn call_generation_storage(
        &self,
        generation: jett_mir::CallOwnerGenerationId,
        root: Option<jett_mir::LocalId>,
        span: Span,
    ) -> Result<NativeCallGenerationStorage, CodegenError> {
        let mut matches = self
            .generation_slots
            .iter()
            .filter(|storage| storage.generation == generation);
        let storage = matches.next().copied().ok_or_else(|| {
            contract_error(
                self.symbol,
                span,
                "call owner generation has no validated native storage",
            )
        })?;
        if matches.next().is_some() || root.is_some_and(|root| root != storage.root) {
            return Err(contract_error(
                self.symbol,
                span,
                "call owner generation changes its exact root or storage",
            ));
        }
        Ok(storage)
    }
    pub(super) fn replace_call_generation(
        &mut self,
        generation: jett_mir::CallOwnerGenerationId,
        root: jett_mir::LocalId,
        rhs_owner: jett_mir::LocalId,
        span: Span,
    ) -> Result<(), CodegenError> {
        let storage = self.call_generation_storage(generation, Some(root), span)?;
        let rhs = self
            .local_types
            .get(rhs_owner.index() as usize)
            .ok_or_else(|| {
                contract_error(
                    self.symbol,
                    span,
                    "call owner generation RHS local is absent",
                )
            })?;
        let rhs_slot = self
            .local_slots
            .get(rhs_owner.index() as usize)
            .copied()
            .flatten()
            .ok_or_else(|| {
                contract_error(
                    self.symbol,
                    span,
                    "call owner generation RHS has no owning storage",
                )
            })?;
        if rhs.id != rhs_owner
            || rhs.ty != storage.ty
            || rhs.view_source.is_some()
            || rhs_owner == root
            || rhs_slot == storage.root_slot
        {
            return Err(contract_error(
                self.symbol,
                span,
                "call owner generation RHS is not its independent exact owner",
            ));
        }
        // RHS acquisition already completed at its unique checked owning Let.
        // Its slot is transferred exactly once; no source expression runs here.
        let next = self.builder.ins().stack_load(ir::types::I64, rhs_slot, 0);
        self.clear_slot(rhs_slot);
        let retired = self
            .builder
            .ins()
            .stack_load(ir::types::I64, storage.retired_slot, 0);
        let has_retired = self.builder.ins().icmp_imm(IntCC::NotEqual, retired, 0);
        let attached = self.builder.create_block();
        let replaced = self.builder.create_block();
        let installed = self.builder.create_block();
        self.builder
            .ins()
            .brif(has_retired, replaced, &[], attached, &[]);

        self.builder.switch_to_block(attached);
        let captured = self
            .builder
            .ins()
            .stack_load(ir::types::I64, storage.root_slot, 0);
        self.clear_slot(storage.root_slot);
        self.builder
            .ins()
            .stack_store(captured, storage.owner_slot, 0);
        let one = self.builder.ins().iconst(ir::types::I64, 1);
        self.builder.ins().stack_store(one, storage.retired_slot, 0);
        self.builder.ins().jump(installed, &[]);

        self.builder.switch_to_block(replaced);
        // The old captured owner stays in escrow. Only the independent current
        // generation is dropped before its ordinary replacement is installed.
        self.drop_slot(storage.root_slot)?;
        self.builder.ins().jump(installed, &[]);

        self.builder.switch_to_block(installed);
        self.builder.ins().stack_store(next, storage.root_slot, 0);
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
        self.drop_dead_locals(&BTreeSet::new())?;
        for index in 0..self.generation_slots.len() {
            let storage = self.generation_slots[index];
            self.drop_slot(storage.owner_slot)?;
            self.clear_slot(storage.retired_slot);
        }
        Ok(())
    }
    pub(super) fn static_data(&mut self, bytes: &[u8]) -> Result<(Value, Value), CodegenError> {
        self.static_data_with_functions(bytes, &[])
    }
    fn static_data_with_functions(
        &mut self,
        bytes: &[u8],
        functions: &[(usize, u64)],
    ) -> Result<(Value, Value), CodegenError> {
        let data = self
            .module
            .declare_anonymous_data(false, false)
            .map_err(|e| CodegenError::Backend(e.to_string()))?;
        let mut desc = cranelift_module::DataDescription::new();
        // Object formats require storage even for an empty literal.
        desc.define(if bytes.is_empty() {
            vec![0].into_boxed_slice()
        } else {
            bytes.into()
        });
        for (offset, function) in functions {
            if self.module.target_config().pointer_type() != ir::types::I64 {
                return Err(CodegenError::Backend(
                    "callback conversion requires 64-bit pointers".into(),
                ));
            }
            let function = u32::try_from(*function).map_err(|_| {
                CodegenError::Backend("invalid callback conversion function ID".into())
            })?;
            let declared = self
                .declarations
                .get(FunctionId::new(function))
                .ok_or_else(|| {
                    CodegenError::Backend("callback conversion function is unreachable".into())
                })?;
            let reference = self
                .module
                .declare_func_in_data(declared.native_id, &mut desc);
            let offset = u32::try_from(*offset).map_err(|_| {
                CodegenError::Backend("callback conversion descriptor is too large".into())
            })?;
            desc.write_function_addr(offset, reference);
        }
        self.module
            .define_data(data, &desc)
            .map_err(|e| CodegenError::Backend(e.to_string()))?;
        let reference = self.module.declare_data_in_func(data, self.builder.func);
        let pointer = self.builder.ins().global_value(ir::types::I64, reference);
        let length = self
            .builder
            .ins()
            .iconst(ir::types::I64, bytes.len() as i64);
        Ok((pointer, length))
    }
    pub(super) fn static_bytes(&mut self, text: &str) -> Result<(Value, Value), CodegenError> {
        self.static_data(text.as_bytes())
    }
    fn construct_builder(
        &mut self,
        owner: TypeId,
        reflection_arguments: &[ReflectionTypeInfo],
        span: Span,
    ) -> Result<LoweredValue, CodegenError> {
        fn encode_name(layout: &mut Vec<u8>, name: &str) -> Result<(), CodegenError> {
            let length = u32::try_from(name.len()).map_err(|_| {
                CodegenError::Backend("reflected construction name is too long".into())
            })?;
            layout.extend_from_slice(&length.to_le_bytes());
            layout.extend_from_slice(name.as_bytes());
            Ok(())
        }
        if let [info] = reflection_arguments
            && !matches!(info.kind.as_str(), "struct" | "bitfield")
        {
            // Generic reflected code can instantiate this builder for a type
            // whose kind is not constructible. Preserve the interpreter's
            // handled put/finish error without inventing a record layout.
            let mut layout = b"JC\x05".to_vec();
            encode_name(&mut layout, &info.type_name)?;
            encode_name(&mut layout, &info.kind)?;
            let (pointer, length) = self.static_data(&layout)?;
            let builder = self.leaf(NativeLeaf::BuilderNew, &[pointer, length], true)?;
            return self.own_linear(builder);
        }
        let fields = match self.types.resolve(owner) {
            Type::Struct(id) => self
                .types
                .resolve_struct(*id)
                .fields
                .iter()
                .map(|(name, ty)| (name.clone(), *ty))
                .collect::<Vec<_>>(),
            Type::Bitfield(id) => self
                .types
                .resolve_bitfield(*id)
                .fields
                .iter()
                .map(|field| (field.name.clone(), field.ty))
                .collect::<Vec<_>>(),
            _ => return Err(self.unsupported(span, "reflected construction owner")),
        };
        if reflection_arguments.len() != fields.len() + 1 {
            return Err(self.unsupported(span, "checked construction field metadata"));
        }
        let bitfield = matches!(self.types.resolve(owner), Type::Bitfield(_));
        let mut layout = if bitfield {
            b"JC\x07".to_vec()
        } else {
            b"JC\x06".to_vec()
        };
        encode_name(&mut layout, &reflection_arguments[0].type_name)?;
        let count = u32::try_from(fields.len())
            .map_err(|_| CodegenError::Backend("too many reflected construction fields".into()))?;
        layout.extend_from_slice(&count.to_le_bytes());
        for ((name, field_ty), field_info) in fields.iter().zip(&reflection_arguments[1..]) {
            encode_name(&mut layout, name)?;
            encode_name(&mut layout, &field_info.type_name)?;
            let mut storage_type = *field_ty;
            if !bitfield {
                while let Type::Refinement { base, .. } = self.types.resolve(storage_type) {
                    storage_type = *base;
                }
            }
            encode_name(&mut layout, &self.types.type_name(storage_type))?;
        }
        if let Type::Bitfield(id) = self.types.resolve(owner) {
            for field in &self.types.resolve_bitfield(*id).fields {
                match field.kind {
                    BitfieldFieldKind::Payload => layout.push(0),
                    BitfieldFieldKind::Bits { width } => {
                        let kind = match self.types.resolve(field.ty) {
                            Type::Int64 => 1,
                            Type::Uint64 => 2,
                            Type::Enum(_) => 3,
                            _ => return Err(self.unsupported(span, "reflected bitfield field")),
                        };
                        layout.push(kind);
                        layout.extend_from_slice(&u32::from(width).to_le_bytes());
                        if let Type::Enum(enum_id) = self.types.resolve(field.ty) {
                            let definition = self.types.resolve_enum(*enum_id);
                            encode_name(&mut layout, &definition.name)?;
                            let count = u32::try_from(definition.variants.len())
                                .map_err(|_| self.unsupported(span, "bitfield enum variants"))?;
                            layout.extend_from_slice(&count.to_le_bytes());
                            for variant in &definition.variants {
                                encode_name(&mut layout, &variant.name)?;
                                layout.extend_from_slice(&variant.discriminant.to_le_bytes());
                            }
                        }
                    }
                }
            }
        }
        for (_, field_ty) in &fields {
            append_builder_debug_layout(&mut layout, self.types, *field_ty)?;
        }
        let (pointer, length) = self.static_data(&layout)?;
        let builder = self.leaf(NativeLeaf::BuilderNew, &[pointer, length], true)?;
        self.own_linear(builder)
    }
    fn construct_variant_builder(
        &mut self,
        owner: TypeId,
        reflection_arguments: &[ReflectionTypeInfo],
        metadata: LoweredValue,
        metadata_type: TypeId,
        span: Span,
    ) -> Result<LoweredValue, CodegenError> {
        let Type::Enum(id) = self.types.resolve(owner) else {
            return Err(self.unsupported(span, "reflected variant construction owner"));
        };
        let variants = &self.types.resolve_enum(*id).variants;
        let fields = variants
            .iter()
            .map(|variant| variant.fields.len())
            .sum::<usize>();
        if reflection_arguments.len() != fields + 1 {
            return Err(self.unsupported(span, "checked variant construction metadata"));
        }
        fn name(layout: &mut Vec<u8>, value: &str) -> Result<(), CodegenError> {
            let length = u32::try_from(value.len())
                .map_err(|_| CodegenError::Backend("reflected variant name is too long".into()))?;
            layout.extend_from_slice(&length.to_le_bytes());
            layout.extend_from_slice(value.as_bytes());
            Ok(())
        }
        let mut layout = b"JC\x08".to_vec();
        name(&mut layout, &reflection_arguments[0].type_name)?;
        let count = u32::try_from(variants.len())
            .map_err(|_| self.unsupported(span, "reflected variant count"))?;
        layout.extend_from_slice(&count.to_le_bytes());
        let mut field_info = reflection_arguments[1..].iter();
        for variant in variants {
            name(&mut layout, &variant.name)?;
            layout.extend_from_slice(&variant.discriminant.to_le_bytes());
            let count = u32::try_from(variant.fields.len())
                .map_err(|_| self.unsupported(span, "reflected payload field count"))?;
            layout.extend_from_slice(&count.to_le_bytes());
            for (field_name, field_ty) in &variant.fields {
                name(&mut layout, field_name)?;
                name(
                    &mut layout,
                    &field_info
                        .next()
                        .ok_or_else(|| self.unsupported(span, "checked variant payload type"))?
                        .type_name,
                )?;
                let mut storage_type = *field_ty;
                while let Type::Refinement { base, .. } = self.types.resolve(storage_type) {
                    storage_type = *base;
                }
                name(&mut layout, &self.types.type_name(storage_type))?;
                append_builder_debug_layout(&mut layout, self.types, *field_ty)?;
            }
        }
        let (pointer, length) = self.static_data(&layout)?;
        let metadata = self.scalar(metadata, span)?;
        let debug_layout = super::debug::debug_layout(self.types, metadata_type)
            .ok_or_else(|| self.unsupported(span, "variant metadata debug layout"))?;
        let (debug_pointer, debug_length) = self.static_data(&debug_layout)?;
        let result = self.leaf(
            NativeLeaf::BuilderVariantNew,
            &[pointer, length, metadata, debug_pointer, debug_length],
            true,
        )?;
        self.own_linear(result)
    }
    fn construct_machine_builder(
        &mut self,
        owner: TypeId,
        reflection_arguments: &[ReflectionTypeInfo],
        metadata: LoweredValue,
        metadata_type: TypeId,
        span: Span,
    ) -> Result<LoweredValue, CodegenError> {
        let (machine_id, state_id) = match self.types.resolve(owner) {
            Type::Machine(id) => (*id, None),
            Type::MachineState { machine, state } => (*machine, Some(*state)),
            _ => return Err(self.unsupported(span, "reflected machine construction owner")),
        };
        let machine = self.types.resolve_machine(machine_id);
        let fields = machine
            .states
            .iter()
            .map(|state| state.fields.len())
            .sum::<usize>();
        if reflection_arguments.len() != fields + 1 {
            return Err(self.unsupported(span, "checked machine construction metadata"));
        }
        fn name(layout: &mut Vec<u8>, value: &str) -> Result<(), CodegenError> {
            let length = u32::try_from(value.len())
                .map_err(|_| CodegenError::Backend("reflected machine name is too long".into()))?;
            layout.extend_from_slice(&length.to_le_bytes());
            layout.extend_from_slice(value.as_bytes());
            Ok(())
        }
        let mut layout = b"JC\x09".to_vec();
        name(&mut layout, &reflection_arguments[0].type_name)?;
        name(&mut layout, &machine.name)?;
        let target_state = state_id
            .and_then(|id| machine.state(id))
            .map(|state| state.name.as_str())
            .unwrap_or("");
        name(&mut layout, target_state)?;
        let count = u32::try_from(machine.states.len())
            .map_err(|_| self.unsupported(span, "reflected machine state count"))?;
        layout.extend_from_slice(&count.to_le_bytes());
        let mut field_info = reflection_arguments[1..].iter();
        for state in &machine.states {
            name(&mut layout, &state.name)?;
            let count = u32::try_from(state.fields.len())
                .map_err(|_| self.unsupported(span, "reflected state payload count"))?;
            layout.extend_from_slice(&count.to_le_bytes());
            for (field_name, field_ty) in &state.fields {
                name(&mut layout, field_name)?;
                name(
                    &mut layout,
                    &field_info
                        .next()
                        .ok_or_else(|| self.unsupported(span, "checked machine payload type"))?
                        .type_name,
                )?;
                let mut storage_type = *field_ty;
                while let Type::Refinement { base, .. } = self.types.resolve(storage_type) {
                    storage_type = *base;
                }
                name(&mut layout, &self.types.type_name(storage_type))?;
                append_builder_debug_layout(&mut layout, self.types, *field_ty)?;
            }
        }
        let (pointer, length) = self.static_data(&layout)?;
        let metadata = self.scalar(metadata, span)?;
        let debug_layout = super::debug::debug_layout(self.types, metadata_type)
            .ok_or_else(|| self.unsupported(span, "machine metadata debug layout"))?;
        let (debug_pointer, debug_length) = self.static_data(&debug_layout)?;
        let result = self.leaf(
            NativeLeaf::BuilderMachineNew,
            &[pointer, length, metadata, debug_pointer, debug_length],
            true,
        )?;
        self.own_linear(result)
    }
    pub(super) fn literal(&mut self, text: &str) -> Result<LoweredValue, CodegenError> {
        let (pointer, length) = self.static_bytes(text)?;
        let value = self.leaf(NativeLeaf::Literal, &[pointer, length], true)?;
        self.own(value)
    }
    fn format_debug_value(
        &mut self,
        value: LoweredValue,
        ty: TypeId,
        span: Span,
    ) -> Result<LoweredValue, CodegenError> {
        if crate::values::is_formattable(self.types, representation_type(self.types, ty)) {
            return self.format_value(value, ty, span);
        }
        let layout = super::debug::debug_layout(self.types, ty)
            .ok_or_else(|| self.unsupported(span, "debug print value layout"))?;
        let (layout_pointer, layout_length) = self.static_data(&layout)?;
        let (label_pointer, label_length) = self.static_bytes("")?;
        let text = self.literal("")?;
        let text_handle = self.scalar(text, span)?;
        let bits = self.payload_bits(value).0;
        self.leaf(
            NativeLeaf::DebugAppendAggregate,
            &[
                text_handle,
                label_pointer,
                label_length,
                bits,
                layout_pointer,
                layout_length,
            ],
            true,
        )?;
        Ok(text)
    }

    fn format_value(
        &mut self,
        value: LoweredValue,
        ty: TypeId,
        span: Span,
    ) -> Result<LoweredValue, CodegenError> {
        let kind = scalar_kind(self.types, ty, "format")?;
        if kind == ScalarKind::Nothing {
            let depth = self.scalar(value, span)?;
            let text = self.leaf(NativeLeaf::NothingFormat, &[depth], true)?;
            return self.own(text);
        }
        if kind == ScalarKind::String {
            let source = self.scalar(value, span)?;
            let text = self.leaf(NativeLeaf::StringTaskFormat, &[source], true)?;
            return self.own(text);
        }
        if let LoweredValue::ScalarTask(_, depth) = value {
            let (bits, _) = self.payload_bits(value);
            let kind = task_scalar_debug_kind(self.types, ty)?
                .ok_or_else(|| self.unsupported(span, "pending scalar formatting"))?;
            let kind = self.builder.ins().iconst(ir::types::I32, kind as i64);
            let text = self.leaf(NativeLeaf::ScalarTaskFormat, &[bits, kind, depth], true)?;
            return self.own(text);
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
    fn check_index_count_arguments(
        &mut self,
        id: IntrinsicId,
        evaluated: &[LoweredValue],
        span: Span,
    ) -> Result<(), CodegenError> {
        let (kind, offset) = match id {
            IntrinsicId::Range => (0, 0),
            IntrinsicId::BytesGet => (1, 1),
            IntrinsicId::BytesSlice => (2, 1),
            IntrinsicId::StringSlice => (3, 1),
            IntrinsicId::StringRepeat => (4, 1),
            _ => return Ok(()),
        };
        let zero = self.builder.ins().iconst(ir::types::I64, 0);
        let receiver = if offset == 0 {
            zero
        } else {
            self.scalar(evaluated[0], span)?
        };
        let mut depths = [zero; 3];
        for (slot, value) in evaluated.iter().skip(offset).enumerate() {
            if let LoweredValue::ScalarTask(_, depth) = value {
                depths[slot] = *depth;
            }
        }
        let kind = self.builder.ins().iconst(ir::types::I32, kind);
        self.leaf(
            NativeLeaf::CheckIndexCountArguments,
            &[receiver, kind, depths[0], depths[1], depths[2]],
            true,
        )?;
        Ok(())
    }
    pub(super) fn reject_pending_scalars(
        &mut self,
        evaluated: &[LoweredValue],
        message: &str,
    ) -> Result<(), CodegenError> {
        let zero = self.builder.ins().iconst(ir::types::I64, 0);
        let mut depths = [zero; 3];
        let mut has_task = false;
        for (slot, value) in evaluated.iter().enumerate() {
            if let LoweredValue::ScalarTask(_, depth) = value {
                depths[slot] = *depth;
                has_task = true;
            }
        }
        if has_task {
            let (pointer, length) = self.static_bytes(message)?;
            self.leaf(
                NativeLeaf::RejectPendingScalars,
                &[depths[0], depths[1], depths[2], pointer, length],
                true,
            )?;
        }
        Ok(())
    }
    fn reject_pending_handle(
        &mut self,
        value: LoweredValue,
        kind: i64,
        message: &str,
        span: Span,
    ) -> Result<(), CodegenError> {
        let handle = self.scalar(value, span)?;
        let kind = self.builder.ins().iconst(ir::types::I32, kind);
        let (pointer, length) = self.static_bytes(message)?;
        self.leaf(
            NativeLeaf::RejectPendingHandle,
            &[handle, kind, pointer, length],
            true,
        )?;
        Ok(())
    }
    fn check_string_arguments(
        &mut self,
        id: IntrinsicId,
        evaluated: &[LoweredValue],
        span: Span,
    ) -> Result<(), CodegenError> {
        let (expected, kinds): (&str, &[i64]) = match id {
            IntrinsicId::StringJoin => ("a list and a string separator", &[2, 0]),
            IntrinsicId::StringReplace => ("three string arguments", &[0, 0, 0]),
            IntrinsicId::StringSplit | IntrinsicId::StringIndexOf | IntrinsicId::StringCount => {
                ("two string arguments", &[0, 0])
            }
            IntrinsicId::StringCharCount
            | IntrinsicId::StringChars
            | IntrinsicId::StringWords
            | IntrinsicId::StringLines
            | IntrinsicId::StringLower
            | IntrinsicId::StringUpper
            | IntrinsicId::StringTrim
            | IntrinsicId::StringTrimStart
            | IntrinsicId::StringTrimEnd
            | IntrinsicId::StringSlugify
            | IntrinsicId::StringToUpperFirst
            | IntrinsicId::StringToLowerFirst
            | IntrinsicId::StringIsAlpha
            | IntrinsicId::StringIsNumeric => ("a string argument", &[0]),
            _ => return Ok(()),
        };
        let message = format!("{} expects {expected}", id.canonical_name());
        for (&kind, &value) in kinds.iter().zip(evaluated) {
            self.reject_pending_handle(value, kind, &message, span)?;
        }
        Ok(())
    }
    fn check_reflected_field_owner(
        &mut self,
        metadata: Value,
        metadata_type: TypeId,
        owner_name: &str,
        members: Option<(&[String], Value)>,
        kind: i64,
        span: Span,
    ) -> Result<(), CodegenError> {
        let (owner_pointer, owner_length) = self.static_bytes(owner_name)?;
        let layout = super::debug::debug_layout(self.types, metadata_type)
            .ok_or_else(|| self.unsupported(span, "reflected field metadata debug layout"))?;
        let (layout_pointer, layout_length) = self.static_data(&layout)?;
        let pointer_type = self.module.target_config().pointer_type();
        let mut member_pointer = self.builder.ins().iconst(pointer_type, 0);
        let mut member_length = self.builder.ins().iconst(ir::types::I64, 0);
        let has_member = if let Some((names, tag)) = members {
            for (index, name) in names.iter().enumerate() {
                let index = i64::try_from(index)
                    .map_err(|_| self.unsupported(span, "reflected member index"))?;
                let matches = self.builder.ins().icmp_imm(IntCC::Equal, tag, index);
                let (pointer, length) = self.static_bytes(name)?;
                member_pointer = self.builder.ins().select(matches, pointer, member_pointer);
                member_length = self.builder.ins().select(matches, length, member_length);
            }
            1
        } else {
            0
        };
        let has_member = self.builder.ins().iconst(ir::types::I32, has_member);
        let kind = self.builder.ins().iconst(ir::types::I32, kind);
        self.leaf(
            NativeLeaf::ReflectedFieldOwnerCheck,
            &[
                metadata,
                owner_pointer,
                owner_length,
                member_pointer,
                member_length,
                has_member,
                kind,
                layout_pointer,
                layout_length,
            ],
            true,
        )?;
        Ok(())
    }

    fn check_reflected_owner_pending(
        &mut self,
        value: Value,
        owner_type: TypeId,
        owner_name: &str,
        kind: i64,
    ) -> Result<(), CodegenError> {
        let (name_pointer, name_length) = self.static_bytes(owner_name)?;
        // Secret-bearing owners deliberately have no debug layout. Their
        // ordinary reflected operations still compile; a pending owner is
        // rejected by the runtime without exposing its secret payload.
        let layout = super::debug::debug_layout(self.types, owner_type).unwrap_or_default();
        let (layout_pointer, layout_length) = self.static_data(&layout)?;
        let kind = self.builder.ins().iconst(ir::types::I32, kind);
        self.leaf(
            NativeLeaf::ReflectedOwnerPendingCheck,
            &[
                value,
                name_pointer,
                name_length,
                layout_pointer,
                layout_length,
                kind,
            ],
            true,
        )?;
        Ok(())
    }

    fn check_reflected_field_metadata_pending(
        &mut self,
        metadata: Value,
        metadata_type: TypeId,
        kind: i64,
        span: Span,
    ) -> Result<(), CodegenError> {
        let layout = super::debug::debug_layout(self.types, metadata_type)
            .ok_or_else(|| self.unsupported(span, "reflected field metadata debug layout"))?;
        let (layout_pointer, layout_length) = self.static_data(&layout)?;
        let kind = self.builder.ins().iconst(ir::types::I32, kind);
        self.leaf(
            NativeLeaf::ReflectedFieldMetadataPendingCheck,
            &[metadata, layout_pointer, layout_length, kind],
            true,
        )?;
        Ok(())
    }

    fn select_reflected_field_conversion(
        &mut self,
        selected: &mut Option<ReflectedFieldConversion>,
        matches: Value,
        actual: TypeId,
        requested: TypeId,
        span: Span,
    ) -> Result<(), CodegenError> {
        if !reflected_field_needs_interface_box(self.types, actual, requested) {
            return Ok(());
        }
        let conversion = interface_conversion(self.types, actual, requested, &[])
            .ok_or_else(|| self.unsupported(span, "reflected interface field conversion"))?;
        let (layout, length) = self.static_data(&conversion.encode())?;
        let previous = match *selected {
            Some(previous) => previous,
            None => {
                // Other compatible fields already contain an erased interface.
                // The descriptor is selected before validation, but conversion
                // runs only after the original metadata and pending checks.
                let copy = NativeInterfaceConversion::Copy { owned: true };
                let (layout, length) = self.static_data(&copy.encode())?;
                ReflectedFieldConversion { layout, length }
            }
        };
        *selected = Some(ReflectedFieldConversion {
            layout: self.builder.ins().select(matches, layout, previous.layout),
            length: self.builder.ins().select(matches, length, previous.length),
        });
        Ok(())
    }

    fn reflected_field_value(
        &mut self,
        owner: Value,
        index: Value,
        result_type: TypeId,
        conversion: Option<ReflectedFieldConversion>,
        span: Span,
    ) -> Result<LoweredValue, CodegenError> {
        let bits = self.leaf(NativeLeaf::StructField, &[owner, index], true)?;
        if let Some(conversion) = conversion {
            // InterfaceConvert borrows and clones the stored owner. A nominal
            // interface-backed refinement needs its own outer identity box;
            // cloning the bits as the requested interface would lose that owner.
            let owned = self.leaf(
                NativeLeaf::InterfaceConvert,
                &[bits, conversion.layout, conversion.length],
                true,
            )?;
            return self.own_linear(owned);
        }
        if is_linear(self.types, result_type) {
            return self.clone_linear(LoweredValue::Scalar(bits), result_type, span);
        }
        if is_string(self.types, result_type) {
            let owned = self.leaf(NativeLeaf::Retain, &[bits], true)?;
            return self.own(owned);
        }
        if is_function(self.types, result_type) {
            let owned = self.leaf(NativeLeaf::StructClone, &[bits], true)?;
            return self.own(owned);
        }
        let value = self.unpack_payload(bits, result_type, span)?;
        if is_task_scalar(self.types, result_type)? {
            let depth = self.leaf(NativeLeaf::StructFieldPendingDepth, &[owner, index], true)?;
            Ok(LoweredValue::ScalarTask(self.scalar(value, span)?, depth))
        } else {
            Ok(value)
        }
    }

    pub(super) fn intrinsic(
        &mut self,
        id: IntrinsicId,
        type_arguments: &[TypeId],
        reflection_arguments: &[ReflectionTypeInfo],
        args: &[Expression],
        order: &[usize],
        result_type: TypeId,
        span: Span,
    ) -> Result<LoweredValue, CodegenError> {
        let mut evaluated = vec![None; args.len()];
        for &index in order {
            evaluated[index] = Some(self.argument(
                &args[index],
                jett_mir::move_values::intrinsic_borrows(id, index, &args[index]),
            )?);
        }
        let evaluated = evaluated
            .into_iter()
            .map(|argument| {
                argument.ok_or_else(|| {
                    contract_error(self.symbol, span, "intrinsic argument was not evaluated")
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let conversion_expected = match id {
            IntrinsicId::Float32FromFloat64 | IntrinsicId::Int64FromFloat64 => {
                Some("a float64 argument")
            }
            IntrinsicId::Float64FromInt64 | IntrinsicId::StringFromInt64 => {
                Some("an int64 argument")
            }
            IntrinsicId::StringFromUint64 => Some("a uint64 argument"),
            IntrinsicId::StringFromFloat64 => Some("a float64 argument"),
            IntrinsicId::StringFromBool => Some("a bool argument"),
            _ => None,
        };
        if let Some(expected) = conversion_expected {
            let message = format!("{} expects {expected}", id.canonical_name());
            self.reject_pending_scalars(&evaluated, &message)?;
        }
        let conversion_handle_kind = match id {
            IntrinsicId::Int64FromString
            | IntrinsicId::Uint64FromString
            | IntrinsicId::Float64FromString
            | IntrinsicId::BytesFromString
            | IntrinsicId::BytesFromHex => Some((0, "a string argument")),
            IntrinsicId::BytesToString | IntrinsicId::BytesToHex => Some((1, "a bytes argument")),
            _ => None,
        };
        if let Some((kind, expected)) = conversion_handle_kind {
            let message = format!("{} expects {expected}", id.canonical_name());
            self.reject_pending_handle(evaluated[0], kind, &message, span)?;
        }
        if id == IntrinsicId::GraphicsRun {
            let display = evaluated.first().copied().ok_or_else(|| {
                contract_error(self.symbol, span, "Graphics authority operand is absent")
            })?;
            self.reject_pending_handle(
                display,
                3,
                "graphics.__run expects a Graphics capability",
                span,
            )?;
            return self.graphics_run(type_arguments, args, &evaluated, span);
        }
        if matches!(
            id,
            IntrinsicId::TypeName | IntrinsicId::TypeKind | IntrinsicId::TypeHasSecret
        ) {
            let info = reflection_arguments
                .first()
                .ok_or_else(|| self.unsupported(span, "checked reflection operand"))?;
            return match id {
                IntrinsicId::TypeName => self.literal(&info.type_name),
                IntrinsicId::TypeKind => self.literal(&info.kind),
                IntrinsicId::TypeHasSecret => Ok(LoweredValue::Scalar(
                    self.builder
                        .ins()
                        .iconst(ir::types::I8, i64::from(info.has_secret)),
                )),
                _ => unreachable!(),
            };
        }
        if id == IntrinsicId::TypeConstructStart {
            let owner = *type_arguments
                .first()
                .ok_or_else(|| self.unsupported(span, "checked construction type"))?;
            return self.construct_builder(owner, reflection_arguments, span);
        }
        if id == IntrinsicId::TypeConstructVariantStart {
            let owner = *type_arguments
                .first()
                .ok_or_else(|| self.unsupported(span, "checked variant construction type"))?;
            return self.construct_variant_builder(
                owner,
                reflection_arguments,
                evaluated[0],
                args[0].ty,
                span,
            );
        }
        if id == IntrinsicId::TypeConstructMachineStart {
            let owner = *type_arguments
                .first()
                .ok_or_else(|| self.unsupported(span, "checked machine construction type"))?;
            return self.construct_machine_builder(
                owner,
                reflection_arguments,
                evaluated[0],
                args[0].ty,
                span,
            );
        }
        if id == IntrinsicId::TypeConstructPut {
            let builder = self.scalar(evaluated[0], span)?;
            let field = self.scalar(evaluated[1], span)?;
            let (bits, owned) = self.payload_bits(evaluated[2]);
            let pending_depth = if let LoweredValue::ScalarTask(_, depth) = evaluated[2] {
                Some(depth)
            } else {
                None
            };
            let owned = self.builder.ins().iconst(ir::types::I32, i64::from(owned));
            let owner = reflection_arguments
                .first()
                .ok_or_else(|| self.unsupported(span, "checked construction owner"))?;
            let field_type = reflection_arguments
                .get(1)
                .ok_or_else(|| self.unsupported(span, "checked construction field type"))?;
            let (owner_pointer, owner_length) = self.static_bytes(&owner.type_name)?;
            let (type_pointer, type_length) = self.static_bytes(&field_type.type_name)?;
            let mut value_type = type_arguments[1];
            if matches!(
                self.types.resolve(type_arguments[0]),
                Type::Struct(_) | Type::Enum(_) | Type::Machine(_) | Type::MachineState { .. }
            ) {
                while let Type::Refinement { base, .. } = self.types.resolve(value_type) {
                    value_type = *base;
                }
            }
            let canonical_type = self.types.type_name(value_type);
            let (canonical_pointer, canonical_length) = self.static_bytes(&canonical_type)?;
            let metadata_layout =
                super::debug::debug_layout(self.types, args[1].ty).ok_or_else(|| {
                    self.unsupported(span, "construction field metadata debug layout")
                })?;
            let (layout_pointer, layout_length) = self.static_data(&metadata_layout)?;
            let mut arguments = vec![builder, field, bits];
            if let Some(depth) = pending_depth {
                arguments.push(depth);
            }
            arguments.extend([
                owned,
                owner_pointer,
                owner_length,
                type_pointer,
                type_length,
                canonical_pointer,
                canonical_length,
                layout_pointer,
                layout_length,
            ]);
            let leaf = if pending_depth.is_some() {
                NativeLeaf::BuilderPutScalarTask
            } else {
                NativeLeaf::BuilderPut
            };
            let result = self.leaf(leaf, &arguments, true)?;
            for transferred in [evaluated[0], evaluated[2]] {
                if let LoweredValue::Owned(_, slot) = transferred {
                    self.clear_slot(slot);
                }
            }
            return self.own_linear(result);
        }
        if id == IntrinsicId::TypeConstructFinish {
            let builder = self.scalar(evaluated[0], span)?;
            let owner = reflection_arguments
                .first()
                .ok_or_else(|| self.unsupported(span, "checked construction owner"))?;
            let (owner_pointer, owner_length) = self.static_bytes(&owner.type_name)?;
            let result = self.leaf(
                NativeLeaf::BuilderFinish,
                &[builder, owner_pointer, owner_length],
                true,
            )?;
            if let LoweredValue::Owned(_, slot) = evaluated[0] {
                self.clear_slot(slot);
            }
            return self.own_linear(result);
        }
        if id == IntrinsicId::TypeArg {
            let (requested, depth) = self.scalar_task(evaluated[0], span)?;
            let count = i64::try_from(evaluated.len() - 1)
                .map_err(|_| self.unsupported(span, "reflected type argument count"))?;
            let count = self.builder.ins().iconst(ir::types::I64, count);
            let reflected = reflection_arguments
                .first()
                .ok_or_else(|| self.unsupported(span, "checked type.arg owner"))?;
            let (name_pointer, name_length) = self.static_bytes(&reflected.type_name)?;
            let checked = self.leaf(
                NativeLeaf::TypeArgCheckedIndex,
                &[requested, count, depth, name_pointer, name_length],
                true,
            )?;
            let mut selected = self.builder.ins().iconst(ir::types::I64, 0);
            for (index, candidate) in evaluated.iter().enumerate().skip(1) {
                let index = i64::try_from(index - 1)
                    .map_err(|_| self.unsupported(span, "reflected type argument index"))?;
                let matches = self.builder.ins().icmp_imm(IntCC::Equal, checked, index);
                let candidate = self.scalar(*candidate, span)?;
                selected = self.builder.ins().select(matches, candidate, selected);
            }
            let cloned = self.leaf(NativeLeaf::StructClone, &[selected], true)?;
            return self.own_linear(cloned);
        }
        if matches!(
            id,
            IntrinsicId::TypeVariantValue | IntrinsicId::TypeMachineStateValue
        ) {
            let value = self.scalar(evaluated[0], span)?;
            let owner = reflection_arguments
                .first()
                .ok_or_else(|| self.unsupported(span, "checked reflected owner"))?;
            self.check_reflected_owner_pending(
                value,
                args[0].ty,
                &owner.type_name,
                i64::from(id == IntrinsicId::TypeMachineStateValue),
            )?;
            let zero = self.builder.ins().iconst(ir::types::I64, 0);
            let tag = self.leaf(NativeLeaf::StructField, &[value, zero], true)?;
            let mut selected = self.scalar(evaluated[1], span)?;
            for (index, variant) in evaluated.iter().enumerate().skip(2) {
                let index = i64::try_from(index - 1)
                    .map_err(|_| self.unsupported(span, "reflected variant or state index"))?;
                let matches = self.builder.ins().icmp_imm(IntCC::Equal, tag, index);
                let variant = self.scalar(*variant, span)?;
                selected = self.builder.ins().select(matches, variant, selected);
            }
            let cloned = self.leaf(NativeLeaf::StructClone, &[selected], true)?;
            return self.own_linear(cloned);
        }
        if id == IntrinsicId::TypeFieldValue {
            let field_types = match self.types.resolve(args[0].ty) {
                Type::Struct(id) => self
                    .types
                    .resolve_struct(*id)
                    .fields
                    .iter()
                    .map(|(_, ty)| *ty)
                    .collect::<Vec<_>>(),
                Type::Bitfield(id) => self
                    .types
                    .resolve_bitfield(*id)
                    .fields
                    .iter()
                    .map(|field| field.ty)
                    .collect::<Vec<_>>(),
                _ => return Err(self.unsupported(span, "reflected field owner")),
            };
            let metadata = self.scalar(evaluated[1], span)?;
            let owner_name = reflection_arguments
                .first()
                .ok_or_else(|| self.unsupported(span, "checked reflected field owner"))?
                .type_name
                .clone();
            self.check_reflected_field_owner(metadata, args[1].ty, &owner_name, None, 0, span)?;
            let zero = self.builder.ins().iconst(ir::types::I64, 0);
            let requested_index = self.leaf(NativeLeaf::StructField, &[metadata, zero], true)?;
            let mut expected = zero;
            let mut compatible = zero;
            let mut conversion = None;
            for (index, field_ty) in field_types.iter().enumerate() {
                let candidate = self.scalar(evaluated[index + 2], span)?;
                let index = i64::try_from(index)
                    .map_err(|_| self.unsupported(span, "reflected field index"))?;
                let matches = self
                    .builder
                    .ins()
                    .icmp_imm(IntCC::Equal, requested_index, index);
                expected = self.builder.ins().select(matches, candidate, expected);
                if reflected_field_types_compatible(self.types, *field_ty, result_type) {
                    compatible = self.builder.ins().select(matches, candidate, compatible);
                }
                self.select_reflected_field_conversion(
                    &mut conversion,
                    matches,
                    *field_ty,
                    result_type,
                    span,
                )?;
            }
            let requested_type = reflection_arguments
                .get(1)
                .ok_or_else(|| self.unsupported(span, "checked reflected field type"))?;
            let (name_pointer, name_length) = self.static_bytes(&requested_type.type_name)?;
            let kind = self.builder.ins().iconst(ir::types::I32, 0);
            let checked_index = self.leaf(
                NativeLeaf::ReflectedCheckedFieldIndex,
                &[
                    metadata,
                    expected,
                    compatible,
                    name_pointer,
                    name_length,
                    kind,
                ],
                true,
            )?;
            let owner = self.scalar(evaluated[0], span)?;
            self.check_reflected_owner_pending(owner, args[0].ty, &owner_name, 2)?;
            return self.reflected_field_value(owner, checked_index, result_type, conversion, span);
        }
        if matches!(
            id,
            IntrinsicId::TypeVariantFieldValue | IntrinsicId::TypeMachineFieldValue
        ) {
            let (field_groups, owner_name, member_names) =
                match (id, self.types.resolve(args[0].ty)) {
                    (IntrinsicId::TypeVariantFieldValue, Type::Enum(enum_id)) => {
                        let definition = self.types.resolve_enum(*enum_id);
                        let owner_name = reflection_arguments
                            .first()
                            .ok_or_else(|| self.unsupported(span, "checked reflected enum owner"))?
                            .type_name
                            .clone();
                        (
                            definition
                                .variants
                                .iter()
                                .map(|variant| {
                                    variant.fields.iter().map(|(_, ty)| *ty).collect::<Vec<_>>()
                                })
                                .collect::<Vec<_>>(),
                            owner_name,
                            definition
                                .variants
                                .iter()
                                .map(|variant| variant.name.clone())
                                .collect::<Vec<_>>(),
                        )
                    }
                    (IntrinsicId::TypeMachineFieldValue, Type::Machine(machine_id))
                    | (
                        IntrinsicId::TypeMachineFieldValue,
                        Type::MachineState {
                            machine: machine_id,
                            ..
                        },
                    ) => {
                        let definition = self.types.resolve_machine(*machine_id);
                        (
                            definition
                                .states
                                .iter()
                                .map(|state| {
                                    state.fields.iter().map(|(_, ty)| *ty).collect::<Vec<_>>()
                                })
                                .collect::<Vec<_>>(),
                            definition.name.clone(),
                            definition
                                .states
                                .iter()
                                .map(|state| state.name.clone())
                                .collect::<Vec<_>>(),
                        )
                    }
                    _ => return Err(self.unsupported(span, "reflected payload field owner")),
                };
            let owner = self.scalar(evaluated[0], span)?;
            let metadata = self.scalar(evaluated[1], span)?;
            let zero = self.builder.ins().iconst(ir::types::I64, 0);
            let tag = self.leaf(NativeLeaf::StructField, &[owner, zero], true)?;
            let kind = if id == IntrinsicId::TypeMachineFieldValue {
                2
            } else {
                1
            };
            self.check_reflected_field_metadata_pending(metadata, args[1].ty, kind, span)?;
            let reflected_owner_name = &reflection_arguments
                .first()
                .ok_or_else(|| self.unsupported(span, "checked reflected payload owner"))?
                .type_name;
            self.check_reflected_owner_pending(
                owner,
                args[0].ty,
                reflected_owner_name,
                if id == IntrinsicId::TypeMachineFieldValue {
                    4
                } else {
                    3
                },
            )?;
            self.check_reflected_field_owner(
                metadata,
                args[1].ty,
                &owner_name,
                Some((&member_names, tag)),
                kind,
                span,
            )?;
            let requested_index = self.leaf(NativeLeaf::StructField, &[metadata, zero], true)?;
            let mut expected = zero;
            let mut compatible = zero;
            let mut conversion = None;
            let mut argument_index = 2;
            for (variant_index, fields) in field_groups.iter().enumerate() {
                for (field_index, field_ty) in fields.iter().enumerate() {
                    let candidate = self.scalar(evaluated[argument_index], span)?;
                    argument_index += 1;
                    let variant_index = i64::try_from(variant_index)
                        .map_err(|_| self.unsupported(span, "reflected variant index"))?;
                    let field_index = i64::try_from(field_index)
                        .map_err(|_| self.unsupported(span, "reflected field index"))?;
                    let variant_matches =
                        self.builder
                            .ins()
                            .icmp_imm(IntCC::Equal, tag, variant_index);
                    let field_matches =
                        self.builder
                            .ins()
                            .icmp_imm(IntCC::Equal, requested_index, field_index);
                    let matches = self.builder.ins().band(variant_matches, field_matches);
                    expected = self.builder.ins().select(matches, candidate, expected);
                    if reflected_field_types_compatible(self.types, *field_ty, result_type) {
                        compatible = self.builder.ins().select(matches, candidate, compatible);
                    }
                    self.select_reflected_field_conversion(
                        &mut conversion,
                        matches,
                        *field_ty,
                        result_type,
                        span,
                    )?;
                }
            }
            let requested_type = reflection_arguments
                .get(1)
                .ok_or_else(|| self.unsupported(span, "checked reflected payload field type"))?;
            let (name_pointer, name_length) = self.static_bytes(&requested_type.type_name)?;
            let kind = if id == IntrinsicId::TypeMachineFieldValue {
                2
            } else {
                1
            };
            let kind = self.builder.ins().iconst(ir::types::I32, kind);
            let checked_index = self.leaf(
                NativeLeaf::ReflectedCheckedFieldIndex,
                &[
                    metadata,
                    expected,
                    compatible,
                    name_pointer,
                    name_length,
                    kind,
                ],
                true,
            )?;
            let one = self.builder.ins().iconst(ir::types::I64, 1);
            let slot = self.builder.ins().iadd(checked_index, one);
            return self.reflected_field_value(owner, slot, result_type, conversion, span);
        }
        if id == IntrinsicId::Range {
            self.check_index_count_arguments(id, &evaluated, span)?;
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
            return self.list_intrinsic(id, args, &evaluated, result_type, span);
        }
        if crate::values::set_intrinsic(id) {
            return self.set_intrinsic(id, &evaluated, result_type, span);
        }
        if crate::values::map_intrinsic(id) {
            return self.map_intrinsic(id, &evaluated, result_type, span);
        }
        if let Some(leaf) = crate::values::bytes_leaf(id) {
            self.check_index_count_arguments(id, &evaluated, span)?;
            let pending_check = match id {
                IntrinsicId::BytesLength
                | IntrinsicId::EncodingBase64Encode
                | IntrinsicId::CryptoSha256
                | IntrinsicId::CryptoSha512
                | IntrinsicId::CryptoMd5 => Some(("a bytes argument", 1)),
                IntrinsicId::BytesConcat => Some(("two bytes arguments", 1)),
                IntrinsicId::CryptoHmacSha256 => Some(("bytes arguments", 1)),
                IntrinsicId::EncodingBase64Decode
                | IntrinsicId::EncodingHexDecode
                | IntrinsicId::EncodingUrlEncode
                | IntrinsicId::EncodingUrlDecode
                | IntrinsicId::EncodingFormEncode
                | IntrinsicId::EncodingFormDecode
                | IntrinsicId::CsvParse
                | IntrinsicId::CsvParseWithHeader => Some(("a string argument", 0)),
                IntrinsicId::CsvStringify => Some(("a list argument", 2)),
                _ => None,
            };
            if let Some((expected, kind)) = pending_check {
                let message = format!("{} expects {expected}", id.canonical_name());
                for &value in &evaluated {
                    self.reject_pending_handle(value, kind, &message, span)?;
                }
            }
            let arguments = evaluated
                .iter()
                .map(|v| self.scalar(*v, span))
                .collect::<Result<Vec<_>, _>>()?;
            let v = self.leaf(leaf, &arguments, true)?;
            return match leaf {
                NativeLeaf::BytesLength => Ok(LoweredValue::Scalar(v)),
                NativeLeaf::BytesToHex
                | NativeLeaf::EncodingBase64Encode
                | NativeLeaf::EncodingUrlEncode
                | NativeLeaf::EncodingFormEncode => self.own(v),
                NativeLeaf::CsvStringify => self.own(v),
                _ => self.own_linear(v),
            };
        }
        if matches!(id, IntrinsicId::MathAverage | IntrinsicId::MathMedian) {
            let list = self.scalar(evaluated[0], span)?;
            let kind = crate::values::math_aggregate_kind(self.types, args[0].ty)
                .ok_or_else(|| self.unsupported(span, "numeric list kind"))?;
            let kind = self
                .builder
                .ins()
                .iconst(ir::types::I32, i64::from(kind as u32));
            let leaf = if id == IntrinsicId::MathAverage {
                NativeLeaf::MathAverage
            } else {
                NativeLeaf::MathMedian
            };
            return Ok(LoweredValue::Scalar(self.leaf(
                leaf,
                &[list, kind],
                true,
            )?));
        }
        if let Some(leaf) = crate::values::math_leaf(id, args.first().map(|a| a.ty)) {
            let expected = match id {
                IntrinsicId::MathKernelAbs
                | IntrinsicId::MathAbs
                | IntrinsicId::MathSqrt
                | IntrinsicId::MathFloor
                | IntrinsicId::MathCeil
                | IntrinsicId::MathRound
                | IntrinsicId::MathLog
                | IntrinsicId::MathLog2
                | IntrinsicId::MathLog10
                | IntrinsicId::MathSin
                | IntrinsicId::MathCos
                | IntrinsicId::MathTan => Some("a numeric argument"),
                IntrinsicId::MathKernelMin
                | IntrinsicId::MathMin
                | IntrinsicId::MathKernelMax
                | IntrinsicId::MathMax => Some("two arguments of the same numeric type"),
                IntrinsicId::MathPow => Some("numeric arguments"),
                IntrinsicId::MathClamp => Some("three arguments of the same numeric type"),
                IntrinsicId::MathMod | IntrinsicId::MathGcd | IntrinsicId::MathLcm => {
                    Some("two int64 arguments")
                }
                IntrinsicId::MathFactorial => Some("an int64 argument"),
                _ => None,
            };
            if let Some(expected) = expected {
                let message = format!("{} expects {expected}", id.canonical_name());
                self.reject_pending_scalars(&evaluated, &message)?;
            }
            let arguments = evaluated
                .iter()
                .map(|v| self.scalar(*v, span))
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(LoweredValue::Scalar(self.leaf(leaf, &arguments, true)?));
        }
        if let Some(leaf) = crate::values::string_leaf(id) {
            self.check_index_count_arguments(id, &evaluated, span)?;
            self.check_string_arguments(id, &evaluated, span)?;
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
            IntrinsicId::UuidNew => {
                let value = self.leaf(NativeLeaf::UuidNew, &[], true)?;
                self.own(value)
            }
            IntrinsicId::ClockNow => {
                let message = format!("{} expects Clock", id.canonical_name());
                self.reject_pending_handle(evaluated[0], 3, &message, span)?;
                let authority = self.scalar(evaluated[0], span)?;
                Ok(LoweredValue::Scalar(self.leaf(
                    NativeLeaf::ClockNow,
                    &[authority],
                    true,
                )?))
            }
            IntrinsicId::RandomBounded => {
                let message = format!(
                    "{} expects Random and two int64 arguments",
                    id.canonical_name()
                );
                self.reject_pending_handle(evaluated[0], 3, &message, span)?;
                self.reject_pending_scalars(&evaluated[1..], &message)?;
                let arguments = evaluated
                    .iter()
                    .map(|value| self.scalar(*value, span))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(LoweredValue::Scalar(self.leaf(
                    NativeLeaf::RandomBounded,
                    &arguments,
                    true,
                )?))
            }
            IntrinsicId::RandomUnitFloat64 => {
                let message = format!("{} expects Random", id.canonical_name());
                self.reject_pending_handle(evaluated[0], 3, &message, span)?;
                let authority = self.scalar(evaluated[0], span)?;
                Ok(LoweredValue::Scalar(self.leaf(
                    NativeLeaf::RandomUnit53,
                    &[authority],
                    true,
                )?))
            }
            IntrinsicId::RandomBool => {
                let message = format!("{} expects Random", id.canonical_name());
                self.reject_pending_handle(evaluated[0], 3, &message, span)?;
                let authority = self.scalar(evaluated[0], span)?;
                let value = self.leaf(NativeLeaf::RandomBool, &[authority], true)?;
                Ok(LoweredValue::Scalar(
                    self.builder.ins().ireduce(ir::types::I8, value),
                ))
            }
            IntrinsicId::EnvironmentGet => {
                let authority_error = format!("{} expects Environment", id.canonical_name());
                self.reject_pending_handle(evaluated[0], 3, &authority_error, span)?;
                let key_error = format!("{} expects a string key", id.canonical_name());
                self.reject_pending_handle(evaluated[1], 0, &key_error, span)?;
                let authority = self.scalar(evaluated[0], span)?;
                let key = self.scalar(evaluated[1], span)?;
                let value = self.leaf(NativeLeaf::EnvironmentGet, &[authority, key], true)?;
                self.own_linear(value)
            }
            IntrinsicId::EnvironmentArgs => {
                let message = format!("{} expects Environment", id.canonical_name());
                self.reject_pending_handle(evaluated[0], 3, &message, span)?;
                let authority = self.scalar(evaluated[0], span)?;
                let value = self.leaf(NativeLeaf::EnvironmentArgs, &[authority], true)?;
                self.own_linear(value)
            }
            IntrinsicId::SecretCompare => {
                let message = "secret.compare expects two strings or two byte strings";
                let kind = if is_string(self.types, args[0].ty) {
                    0
                } else {
                    1
                };
                for &value in &evaluated {
                    self.reject_pending_handle(value, kind, message, span)?;
                }
                let left = self.scalar(evaluated[0], span)?;
                let right = self.scalar(evaluated[1], span)?;
                let leaf = if is_string(self.types, args[0].ty) {
                    NativeLeaf::SecretCompareString
                } else {
                    NativeLeaf::SecretCompareBytes
                };
                let result = self.leaf(leaf, &[left, right], true)?;
                Ok(LoweredValue::Scalar(
                    self.builder.ins().ireduce(ir::types::I8, result),
                ))
            }
            IntrinsicId::SecretRedact => self.literal("***"),
            IntrinsicId::BitfieldToBytes => self.encode_bitfield(evaluated[0], args[0].ty, span),
            IntrinsicId::BitfieldFromBytes => self.decode_bitfield(evaluated[0], result_type, span),
            IntrinsicId::Float32FromFloat64 => {
                let input = self.scalar(evaluated[0], span)?;
                Ok(LoweredValue::Scalar(
                    self.builder.ins().fdemote(ir::types::F32, input),
                ))
            }
            IntrinsicId::Float64FromInt64 | IntrinsicId::Int64FromFloat64 => {
                let input = self.scalar(evaluated[0], span)?;
                let leaf = if id == IntrinsicId::Float64FromInt64 {
                    NativeLeaf::FloatFromInt
                } else {
                    NativeLeaf::IntFromFloat
                };
                let result = self.leaf(leaf, &[input], true)?;
                self.own_linear(result)
            }
            IntrinsicId::StringFromInt64
            | IntrinsicId::StringFromUint64
            | IntrinsicId::StringFromFloat64
            | IntrinsicId::StringFromBool => self.format_value(evaluated[0], args[0].ty, span),
            IntrinsicId::StdoutWrite => {
                let authority = self.scalar(evaluated[0], span)?;
                let text = self.scalar(evaluated[1], span)?;
                self.leaf(NativeLeaf::Stdout, &[authority, text], true)?;
                Ok(self.nothing())
            }
            IntrinsicId::Print | IntrinsicId::Println => {
                // Evaluate every argument before the first output effect.
                let mut output = self.literal("")?;
                for (index, (arg, value)) in args.iter().zip(evaluated).enumerate() {
                    if index != 0 {
                        let space = self.literal(" ")?;
                        output = self.concatenate(output, space, span)?;
                    }
                    let text = self.format_debug_value(value, arg.ty, arg.span)?;
                    output = self.concatenate(output, text, span)?;
                }
                if id == IntrinsicId::Println {
                    let newline = self.literal("\n")?;
                    output = self.concatenate(output, newline, span)?;
                }
                let output = self.scalar(output, span)?;
                self.leaf(NativeLeaf::DebugPrint, &[output], true)?;
                Ok(self.nothing())
            }
            _ => Err(self.unsupported(span, "runtime intrinsic")),
        }
    }
}

#[cfg(test)]
mod reflected_field_tests {
    use super::*;
    use jett_types::InterfaceDef;

    fn interface(types: &mut TypeInterner, name: &str) -> TypeId {
        let id = types.add_interface(InterfaceDef {
            name: name.into(),
            methods: vec![],
        });
        types.intern(Type::Interface(id))
    }

    fn refinement(types: &mut TypeInterner, name: &str, base: TypeId) -> TypeId {
        types.intern(Type::Refinement {
            name: name.into(),
            base,
        })
    }

    #[test]
    fn reflected_interface_conversion_retains_the_declared_refinement_owner() {
        let mut types = TypeInterner::new();
        let named = interface(&mut types, "app.Named");
        let selected = refinement(&mut types, "app.Selected", named);
        let nested = refinement(&mut types, "app.Nested", selected);
        for actual in [selected, nested] {
            assert!(reflected_field_types_compatible(&types, actual, named));
            assert!(reflected_field_needs_interface_box(&types, actual, named));
            assert_eq!(
                interface_conversion(&types, actual, named, &[]),
                Some(NativeInterfaceConversion::Box {
                    concrete: u64::from(actual.index()),
                    owned: true,
                    nothing: false,
                    layout: debug::debug_layout(&types, actual).unwrap(),
                }),
            );
        }
        assert!(!reflected_field_needs_interface_box(&types, named, named));
        assert_eq!(
            interface_conversion(&types, named, named, &[]),
            Some(NativeInterfaceConversion::Copy { owned: true }),
        );
    }

    #[test]
    fn reflected_interface_conversion_does_not_add_nominal_casts_or_admission() {
        let mut types = TypeInterner::new();
        let named = interface(&mut types, "app.Named");
        let other = interface(&mut types, "app.Other");
        let selected = refinement(&mut types, "app.Selected", named);
        let sibling = refinement(&mut types, "app.Sibling", named);
        let nested = refinement(&mut types, "app.Nested", selected);
        let count = refinement(&mut types, "app.Count", TypeInterner::INT64);
        for (actual, requested) in [
            (selected, selected),
            (selected, sibling),
            (nested, selected),
            (named, selected),
            (count, TypeInterner::INT64),
        ] {
            assert!(reflected_field_types_compatible(&types, actual, requested));
            assert!(!reflected_field_needs_interface_box(
                &types, actual, requested
            ));
        }
        for (actual, requested) in [
            (selected, other),
            (TypeInterner::INT64, named),
            (TypeInterner::STRING, named),
        ] {
            assert!(!reflected_field_types_compatible(&types, actual, requested));
            assert!(!reflected_field_needs_interface_box(
                &types, actual, requested
            ));
        }
        let actual = types.intern(Type::List(selected));
        let requested = types.intern(Type::List(named));
        assert!(!reflected_field_types_compatible(&types, actual, requested));
        assert!(!reflected_field_needs_interface_box(
            &types, actual, requested
        ));
    }

    #[test]
    fn reflected_interface_conversion_preserves_secret_admission_and_layout() {
        let mut types = TypeInterner::new();
        let named = interface(&mut types, "app.Named");
        let selected = refinement(&mut types, "app.Selected", named);
        let secret_named = types.intern(Type::Secret(named));
        let secret_selected = types.intern(Type::Secret(selected));
        assert!(reflected_field_needs_interface_box(
            &types,
            secret_selected,
            secret_named,
        ));
        assert_eq!(
            interface_conversion(&types, secret_selected, secret_named, &[]),
            Some(NativeInterfaceConversion::Box {
                concrete: u64::from(selected.index()),
                owned: true,
                nothing: false,
                layout: debug::debug_layout(&types, selected).unwrap(),
            }),
        );
        for (actual, requested) in [(selected, secret_named), (secret_selected, named)] {
            assert!(!reflected_field_types_compatible(&types, actual, requested));
            assert!(!reflected_field_needs_interface_box(
                &types, actual, requested
            ));
        }
    }

    fn wrapper(types: &mut TypeInterner, kind: usize, child: TypeId) -> TypeId {
        types.intern(match kind {
            0 => Type::List(child),
            1 => Type::Set(child),
            2 => Type::Map(child, child),
            3 => Type::Optional(child),
            4 => Type::Result(child, child),
            _ => panic!("test wrapper kind"),
        })
    }

    #[test]
    fn reflected_builtin_carriers_recurse_through_all_shapes_and_repeated_children() {
        let mut types = TypeInterner::new();
        let positive = refinement(&mut types, "app.Positive", TypeInterner::INT64);
        let higher = refinement(&mut types, "app.Higher", positive);
        let sibling = refinement(&mut types, "app.Sibling", TypeInterner::INT64);
        let text = refinement(&mut types, "app.Text", TypeInterner::STRING);
        for kind in 0..5 {
            for (actual, requested) in [
                (TypeInterner::INT64, positive),
                (higher, positive),
                (sibling, higher),
                (positive, TypeInterner::INT64),
                (TypeInterner::STRING, text),
            ] {
                let actual = wrapper(&mut types, kind, actual);
                let requested = wrapper(&mut types, kind, requested);
                assert!(
                    reflected_field_types_compatible(&types, actual, requested),
                    "shape {kind}"
                );
                assert!(!reflected_field_needs_interface_box(
                    &types, actual, requested
                ));
            }
        }
        let mut actual = TypeInterner::INT64;
        let mut requested = higher;
        // Four full rounds distinguish arbitrary recursion from a one-level fix.
        for _ in 0..4 {
            for kind in 0..5 {
                actual = wrapper(&mut types, kind, actual);
                requested = wrapper(&mut types, kind, requested);
            }
        }
        assert!(reflected_field_types_compatible(&types, actual, requested));
        let actual_root = refinement(&mut types, "app.ActualTree", actual);
        let requested_root = refinement(&mut types, "app.RequestedTree", requested);
        assert!(reflected_field_types_compatible(
            &types,
            actual_root,
            requested_root
        ));
        assert!(reflected_field_types_compatible(
            &types,
            actual_root,
            requested
        ));
    }

    #[test]
    fn reflected_builtin_carriers_preserve_secrecy_and_refuse_nested_adapters_and_nominal_casts() {
        let mut types = TypeInterner::new();
        let positive = refinement(&mut types, "app.Positive", TypeInterner::INT64);
        let secret_plain = types.intern(Type::Secret(TypeInterner::INT64));
        let secret_positive = types.intern(Type::Secret(positive));
        let twice_secret = types.intern(Type::Secret(secret_plain));
        let named = interface(&mut types, "app.Named");
        let selected = refinement(&mut types, "app.Selected", named);
        let callback_plain = types.intern(Type::Function {
            params: vec![TypeInterner::INT64],
            view_params: vec![false],
            return_type: TypeInterner::INT64,
        });
        let callback_refined = types.intern(Type::Function {
            params: vec![TypeInterner::INT64],
            view_params: vec![false],
            return_type: positive,
        });
        let first = types.add_struct(jett_types::StructDef {
            name: "app.Holder[int64]".into(),
            fields: vec![("value".into(), TypeInterner::INT64)],
            methods: vec![],
        });
        let first = types.intern(Type::Struct(first));
        let second = types.add_struct(jett_types::StructDef {
            name: "app.Holder[app.Positive]".into(),
            fields: vec![("value".into(), positive)],
            methods: vec![],
        });
        let second = types.intern(Type::Struct(second));
        for kind in [0, 2, 3, 4] {
            let actual = wrapper(&mut types, kind, secret_plain);
            let requested = wrapper(&mut types, kind, secret_positive);
            // Same secrecy placement is raw-compatible; checked planning then
            // refuses to establish a new invariant underneath that secret.
            assert!(reflected_field_types_compatible(&types, actual, requested));
            for (a, b) in [
                (secret_plain, positive),
                (positive, secret_positive),
                (twice_secret, secret_positive),
                (selected, named),
                (named, selected),
                (callback_plain, callback_refined),
                (first, second),
                (TypeInterner::INT32, positive),
            ] {
                let a = wrapper(&mut types, kind, a);
                let b = wrapper(&mut types, kind, b);
                assert!(
                    !reflected_field_types_compatible(&types, a, b),
                    "shape {kind}"
                );
            }
        }
        let list = types.intern(Type::List(positive));
        let set = types.intern(Type::Set(positive));
        let optional = types.intern(Type::Optional(positive));
        assert!(!reflected_field_types_compatible(&types, list, set));
        assert!(!reflected_field_types_compatible(&types, list, optional));
        let mut foreign = TypeInterner::new();
        let mut invalid = TypeInterner::INT64;
        for index in 0..=types.len() {
            invalid = refinement(
                &mut foreign,
                &format!("app.Foreign{index}"),
                TypeInterner::INT64,
            );
        }
        assert!(invalid.index() as usize >= types.len());
        assert!(!reflected_field_types_compatible(&types, invalid, positive));
        assert!(!reflected_field_types_compatible(&types, positive, invalid));
    }

    #[test]
    fn interface_box_owner_removes_only_shared_outer_secret_qualifiers() {
        let mut types = TypeInterner::new();
        let named = interface(&mut types, "app.Named");
        let selected = refinement(&mut types, "app.Selected", named);
        let nested = refinement(&mut types, "app.Nested", selected);
        let secret_named = types.intern(Type::Secret(named));
        let secret_nested = types.intern(Type::Secret(nested));
        let twice_secret_nested = types.intern(Type::Secret(secret_nested));
        let secret_backed = refinement(&mut types, "app.Hidden", secret_named);
        for (source, target, owner) in [
            (nested, secret_named, nested),
            (secret_nested, secret_named, nested),
            (secret_nested, named, secret_nested),
            (twice_secret_nested, secret_named, secret_nested),
            (secret_backed, secret_named, secret_backed),
        ] {
            assert_eq!(interface_box_owner(&types, source, target), owner);
            assert_eq!(
                interface_conversion(&types, source, target, &[]),
                Some(NativeInterfaceConversion::Box {
                    concrete: u64::from(owner.index()),
                    owned: true,
                    nothing: false,
                    layout: debug::debug_layout(&types, owner).unwrap(),
                }),
            );
        }
    }

    #[test]
    fn qualified_interface_box_layout_keeps_payload_secrets_redacted() {
        let mut types = TypeInterner::new();
        let named = interface(&mut types, "app.Named");
        let secret_named = types.intern(Type::Secret(named));
        let secret_string = types.intern(Type::Secret(TypeInterner::STRING));
        let id = types.add_struct(jett_types::StructDef {
            name: "app.Record".into(),
            fields: vec![("token".into(), secret_string)],
            methods: vec![],
        });
        let record = types.intern(Type::Struct(id));
        let selected = refinement(&mut types, "app.Selected", record);
        let secret_selected = types.intern(Type::Secret(selected));
        let layout = debug::debug_layout(&types, selected).unwrap();
        assert_eq!(
            layout.last(),
            Some(&(jett_runtime::native_abi::values::NativeDebugTag::Redacted as u8)),
        );
        assert_eq!(
            interface_conversion(&types, secret_selected, secret_named, &[]),
            Some(NativeInterfaceConversion::Box {
                concrete: u64::from(selected.index()),
                owned: true,
                nothing: false,
                layout,
            }),
        );
        assert_eq!(
            debug::debug_layout(&types, secret_named).unwrap().last(),
            Some(&(jett_runtime::native_abi::values::NativeDebugTag::Redacted as u8)),
        );
        assert_eq!(
            interface_conversion(&types, secret_string, named, &[]),
            Some(NativeInterfaceConversion::Box {
                concrete: u64::from(secret_string.index()),
                owned: true,
                nothing: false,
                layout: debug::debug_layout(&types, secret_string).unwrap(),
            }),
        );
    }
}
