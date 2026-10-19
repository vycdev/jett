//! Nonconsuming ordinary sum payloads require their original checked Handle proof.
use super::*;
use jett_mir::{BlockId, OrdinaryBorrowedSumProjection, OrdinarySumPayloadPath};

#[cfg(test)]
mod tests;

/// Public admission checks the private current graph before any ordinary
/// initializer or consuming sum path can reinterpret a stale projection.
pub(super) fn validate_public_program(
    program: &Program,
    types: &TypeInterner,
) -> Result<(), CodegenError> {
    let mut errors = Vec::new();
    for function in &program.functions {
        for block in &function.blocks {
            for (index, statement) in block.statements.iter().enumerate() {
                let result = validate_statement(function, types, block.id, index, statement)
                    .and_then(|()| match statement.kind {
                        StatementKind::SumTag { source, .. }
                        | StatementKind::SumTake { source, .. }
                            if function.is_view_local(source) =>
                        {
                            function
                                .ordinary_borrowed_sum_projection(block.id, index)?
                                .ok_or_else(|| {
                                    "borrowed ordinary sum has no original projection proof".into()
                                })
                                .map(|_| ())
                        }
                        _ => Ok(()),
                    });
                if let Err(message) = result {
                    errors.push(jett_mir::ValidationError {
                        span: statement.span,
                        message,
                    });
                }
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(CodegenError::InvalidMir(errors))
    }
}

pub(crate) fn validate_statement(
    function: &Function,
    types: &TypeInterner,
    block: BlockId,
    index: usize,
    statement: &Statement,
) -> Result<(), String> {
    match &statement.kind {
        StatementKind::SumTag { .. } | StatementKind::SumTake { .. } => {
            if let Some(row) = function.ordinary_borrowed_sum_projection(block, index)? {
                validate_types(function, types, row)?;
            }
        }
        StatementKind::Let { local, value } | StatementKind::BeginCallView { local, value } => {
            if function.ordinary_borrowed_sum_alias(*local, value)?
                && function.local(*local).is_none()
            {
                return Err("ordinary sum alias lost its exact local header".into());
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_types(
    function: &Function,
    types: &TypeInterner,
    row: &OrdinaryBorrowedSumProjection,
) -> Result<(), String> {
    let source = function
        .local(row.source())
        .ok_or("ordinary sum source is absent")?;
    let backing = function
        .local(row.backing())
        .ok_or("ordinary sum backing is absent")?;
    let output = function
        .local(row.output())
        .ok_or("ordinary sum output is absent")?;
    let tag = function
        .local(row.tag())
        .ok_or("ordinary sum tag is absent")?;
    if source.ty != row.sum_type()
        || backing.ty != row.sum_type()
        || output.ty != row.payload_type()
        || tag.ty != TypeInterner::BOOL
        || !function.is_view_local(row.source())
        || !function.is_view_local(row.output())
        || source.view_source != Some(row.backing())
        || output.view_source != Some(row.backing())
    {
        return Err("ordinary sum projection changed its exact types or backing origin".into());
    }
    match (types.resolve(row.sum_type()), row.path()) {
        (Type::Optional(payload), OrdinarySumPayloadPath::OptionalSome)
            if *payload == row.payload_type() && row.error().is_none() => {}
        (Type::Result(payload, error), OrdinarySumPayloadPath::ResultOk)
            if *payload == row.payload_type() && *error == TypeInterner::STRING =>
        {
            let error = row
                .error()
                .and_then(|local| function.local(local))
                .ok_or("ordinary Result projection lost its String companion")?;
            if error.ty != TypeInterner::STRING || function.is_view_local(error.id) {
                return Err("ordinary Result failure must own one String copy".into());
            }
        }
        _ => return Err("ordinary sum projection changed its selected nominal payload".into()),
    }
    Ok(())
}

impl Translator<'_, '_> {
    pub(super) fn ordinary_borrowed_sum_statement(
        &mut self,
        block: BlockId,
        index: usize,
        statement: &Statement,
    ) -> Result<bool, CodegenError> {
        let symbol = self.symbol;
        let fail = |message| contract_error(symbol, statement.span, message);
        match &statement.kind {
            StatementKind::Let { local, value } | StatementKind::BeginCallView { local, value } => {
                if self
                    .function
                    .ordinary_borrowed_sum_alias(*local, value)
                    .map_err(fail)?
                {
                    // The original header selects either a nonowning alias or
                    // an ordinary copyable final binding. Projected bits stay
                    // borrowed until that exact destination is initialized.
                    if self.local_types[local.index() as usize]
                        .view_source
                        .is_some()
                        && self.local_slots[local.index() as usize].is_some()
                    {
                        return Err(fail(
                            "ordinary borrowed payload gained owning storage".into(),
                        ));
                    }
                    let value = self.argument(value, true)?;
                    self.define_local(*local, value, statement.span)?;
                    return Ok(true);
                }
                return Ok(false);
            }
            StatementKind::SumTag { .. } | StatementKind::SumTake { .. } => {}
            _ => return Ok(false),
        }
        let Some(row) = self
            .function
            .ordinary_borrowed_sum_projection(block, index)
            .map_err(fail)?
        else {
            return Ok(false);
        };
        validate_types(self.function, self.types, row).map_err(fail)?;
        let sum_type = row.sum_type();
        let source = Expression {
            kind: ExpressionKind::Local(row.source()),
            ty: sum_type,
            span: statement.span,
        };
        let value = self.argument(&source, true)?;
        let value = self.scalar(value, statement.span)?;
        match statement.kind {
            StatementKind::SumTag { target, .. } => {
                let layout = debug::debug_layout(self.types, sum_type)
                    .ok_or_else(|| self.unsupported(statement.span, "ordinary sum debug layout"))?;
                let (pointer, length) = self.static_data(&layout)?;
                let tag = self.leaf(NativeLeaf::SumHandleTag, &[value, pointer, length], true)?;
                let tag = self.builder.ins().ireduce(ir::types::I8, tag);
                self.define_local(target, LoweredValue::Scalar(tag), statement.span)?;
            }
            StatementKind::SumTake {
                target, success, ..
            } => {
                if success && self.local_slots[target.index() as usize].is_some() {
                    return Err(fail(
                        "ordinary success payload gained owning storage".into(),
                    ));
                }
                if !success && self.local_slots[target.index() as usize].is_none() {
                    return Err(fail("ordinary failure String lost owning storage".into()));
                }
                let tag = self
                    .builder
                    .ins()
                    .iconst(ir::types::I32, i64::from(success));
                let depth = self.leaf(NativeLeaf::SumPayloadPendingDepth, &[value], true)?;
                let bits = self.leaf(NativeLeaf::SumPayloadBorrow, &[value, tag], true)?;
                let ty = self.local_types[target.index() as usize].ty;
                let value = self.ordinary_borrowed_payload(bits, ty, depth, statement.span)?;
                self.define_local(target, value, statement.span)?;
            }
            _ => unreachable!("projection query admits only tag and payload rows"),
        }
        Ok(true)
    }

    /// Keep opaque carriers borrowed; only primitive payload bits need a native
    /// bitcast or width reduction. No temporary owner is created here.
    fn ordinary_borrowed_payload(
        &mut self,
        bits: Value,
        ty: TypeId,
        depth: Value,
        span: Span,
    ) -> Result<LoweredValue, CodegenError> {
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
        if is_task_scalar(self.types, ty)? {
            Ok(LoweredValue::ScalarTask(value, depth))
        } else {
            Ok(LoweredValue::Scalar(value))
        }
    }
}
