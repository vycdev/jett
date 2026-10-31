//! Preflight the selected ancestors before RHS execution; commit never Takes.
use super::*;

impl Translator<'_, '_> {
    pub(super) fn field_update_parent(
        &mut self,
        place: &jett_hir::FieldUpdatePlace,
    ) -> Result<(Value, Value), CodegenError> {
        let index = place.root().index() as usize;
        let slot = self
            .local_slots
            .get(index)
            .copied()
            .flatten()
            .ok_or_else(|| {
                self.unsupported(place.span(), "field update root has no owning storage")
            })?;
        let mut handle = self.builder.ins().stack_load(ir::types::I64, slot, 0);
        for (position, projection) in place.projections().iter().enumerate() {
            self.check_struct_pending_access(
                handle,
                projection.owner_type,
                "field access is not supported on ",
                true,
            )?;
            let offset = match projection.owner {
                jett_hir::FieldUpdateOwner::Struct(_) => 0,
                jett_hir::FieldUpdateOwner::MachineState { state, .. } => {
                    let expected = self
                        .builder
                        .ins()
                        .iconst(ir::types::I64, i64::from(state.index()));
                    self.leaf(NativeLeaf::MachineExpectState, &[handle, expected], true)?;
                    1
                }
            };
            let ordinal = projection
                .field_index
                .checked_add(offset)
                .and_then(|index| i64::try_from(index).ok())
                .ok_or_else(|| {
                    self.unsupported(place.span(), "field update ordinal is not representable")
                })?;
            let field = self.builder.ins().iconst(ir::types::I64, ordinal);
            if position + 1 == place.projections().len() {
                return Ok((handle, field));
            }
            handle = self.leaf(NativeLeaf::StructField, &[handle, field], true)?;
        }
        Err(self.unsupported(place.span(), "field update path is empty"))
    }

    pub(super) fn commit_field_update(
        &mut self,
        place: &jett_hir::FieldUpdatePlace,
        expression: &Expression,
        span: Span,
    ) -> Result<(), CodegenError> {
        // Revalidation can still fail without transferring the staged RHS.
        let (parent, field) = self.field_update_parent(place)?;
        let value = self.expression(expression)?;
        let owned =
            is_linear(self.types, expression.ty) || is_copy_owned(self.types, expression.ty);
        let value = if is_copy_owned(self.types, expression.ty) {
            self.own_copy_value(value, expression.ty, span)?
        } else {
            value
        };
        if owned && !matches!(value, LoweredValue::Owned(..)) {
            return Err(
                self.unsupported(span, "field update replacement has no acquired ownership")
            );
        }
        let (bits, actual_owned) = self.payload_bits(value);
        if actual_owned != owned {
            return Err(self.unsupported(
                span,
                "field update replacement ownership differs from checked field",
            ));
        }
        let depth = match value {
            LoweredValue::ScalarTask(_, depth) => depth,
            _ => self.builder.ins().iconst(ir::types::I64, 0),
        };
        let owns = self.builder.ins().iconst(ir::types::I32, i64::from(owned));
        // The leaf validates every fallible condition first and performs one
        // occupied swap. It returns the old bits and performs no cleanup.
        let displaced = self.leaf(
            NativeLeaf::StructReplaceOccupied,
            &[parent, field, bits, owns, depth],
            true,
        )?;
        if let LoweredValue::Owned(_, slot) = value {
            self.clear_slot(slot);
        }
        if owned {
            let LoweredValue::Owned(_, slot) = self.own_linear(displaced)? else {
                return Err(self.unsupported(span, "displaced field has no retirement slot"));
            };
            // Existing terminal cleanup precedence; the new field stays owned
            // by its root even if destruction reports a failure.
            self.drop_slot(slot)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checked_source_field_update_emits_atomic_occupied_leaf() {
        let source = "struct Record:\n    value: int64\n    label: string\nfunction change(mutable record: Record) returns Record:\n    record.value = record.value + 1\n    record.label = \"after\"\n    return record\n";
        let file = jett_common::FileId::new(0);
        let parsed = jett_parser::parse(source, file);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let resolved = jett_resolve::resolve(&parsed.module);
        let checked = jett_typecheck::check(&parsed.module, &resolved);
        assert!(
            checked
                .diagnostics
                .iter()
                .all(|d| d.severity != jett_diagnostics::Severity::Error),
            "{:?}",
            checked.diagnostics
        );
        let high = jett_hir::lower(
            &parsed.module,
            &resolved,
            &checked,
            &std::collections::HashMap::from([(file, jett_common::SourceOrigin::Project)]),
        )
        .expect("normally checked HIR");
        let program = jett_mir::lower(&high, &checked.interner).expect("checked-place MIR");
        let object = emit_host_object(&program, &checked.interner)
            .expect("native occupied field replacement");
        let leaf = b"jett_rt_v1_struct_replace_occupied";
        assert!(object.bytes.windows(leaf.len()).any(|bytes| bytes == leaf));
    }
}
