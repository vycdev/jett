use super::*;
use jett_hir::{IntrinsicId, StringSegment};
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
        Ok(LoweredValue::Scalar(value))
    }
    pub(super) fn drop_slot(&mut self, slot: ir::StackSlot) -> Result<(), CodegenError> {
        let value = self.builder.ins().stack_load(ir::types::I64, slot, 0);
        self.leaf(NativeLeaf::Release, &[value], false)?;
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
            if !live.contains(&i) {
                if let Some(slot) = slot {
                    self.drop_slot(*slot)?;
                }
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
        span: Span,
    ) -> Result<LoweredValue, CodegenError> {
        let evaluated = reordered_map(
            args,
            order,
            |a| self.expression(a),
            || CodegenError::Backend("invalid intrinsic argument order".into()),
        )?;
        if let Some(leaf) = crate::values::string_leaf(id) {
            let native_args = evaluated
                .iter()
                .map(|v| self.scalar(*v, span))
                .collect::<Result<Vec<_>, _>>()?;
            let value = self.leaf(leaf, &native_args, true)?;
            return match leaf {
                NativeLeaf::CharCount => Ok(LoweredValue::Scalar(value)),
                NativeLeaf::IsAlpha | NativeLeaf::IsNumeric => Ok(LoweredValue::Scalar(
                    self.builder.ins().ireduce(ir::types::I8, value),
                )),
                _ => self.own(value),
            };
        }
        match id {
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
