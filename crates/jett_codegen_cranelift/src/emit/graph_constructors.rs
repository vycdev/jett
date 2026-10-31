//! Ordinary owned construction selected by exact protected Graph output proofs.
use super::*;

#[derive(Clone, Copy)]
pub(super) struct Storage {
    constructor: jett_mir::ResourceConstructorId,
    pub(super) slot: ir::StackSlot,
}

pub(super) fn storage(
    function: &Function,
    types: &TypeInterner,
    builder: &mut FunctionBuilder<'_>,
) -> Result<Vec<Storage>, CodegenError> {
    let mut slots = Vec::new();
    for statement in function.blocks.iter().flat_map(|block| &block.statements) {
        let StatementKind::ResourceConstructor(
            node @ jett_mir::ResourceConstructorNode::Begin { .. },
        ) = &statement.kind
        else {
            continue;
        };
        if jett_mir::native_graph_storage_for_constructor(function, types, node).is_none() {
            continue;
        }
        function
            .resource_constructor(node.constructor())
            .map_err(CodegenError::Backend)?;
        if slots
            .iter()
            .any(|storage: &Storage| storage.constructor == node.constructor())
        {
            return Err(CodegenError::Backend(
                "Graph constructor has copied Begin storage".into(),
            ));
        }
        slots.push(Storage {
            constructor: node.constructor(),
            slot: builder.create_sized_stack_slot(ir::StackSlotData::new(
                ir::StackSlotKind::ExplicitSlot,
                8,
                3,
            )),
        });
    }
    Ok(slots)
}

impl Translator<'_, '_> {
    pub(super) fn graph_constructor_statement(
        &mut self,
        statement: &Statement,
    ) -> Result<bool, CodegenError> {
        let StatementKind::ResourceConstructor(node) = &statement.kind else {
            return Ok(false);
        };
        let Some(proof) =
            jett_mir::native_graph_storage_for_constructor(self.function, self.types, node)
        else {
            return Ok(false);
        };
        let original = self
            .function
            .resource_constructor(node.constructor())
            .map_err(|message| contract_error(self.symbol, statement.span, message))?;
        if proof.ty() != original.original().ty {
            return Err(contract_error(
                self.symbol,
                statement.span,
                "Graph constructor changed its exact output type",
            ));
        }
        let storage = self
            .graph_constructor_slots
            .iter()
            .find(|storage| storage.constructor == node.constructor())
            .copied()
            .ok_or_else(|| {
                contract_error(
                    self.symbol,
                    statement.span,
                    "Graph constructor has no protected native builder storage",
                )
            })?;
        match node {
            jett_mir::ResourceConstructorNode::Begin { .. } => {
                let count = i64::try_from(original.field_types().len()).map_err(|_| {
                    contract_error(
                        self.symbol,
                        statement.span,
                        "Graph constructor field count exceeds native storage",
                    )
                })?;
                let count = self.builder.ins().iconst(ir::types::I64, count);
                let handle = self.leaf(NativeLeaf::StructNew, &[count], true)?;
                // This private owner spans physical statements; ordinary temporary
                // and local liveness do not describe an incomplete constructor.
                self.builder.ins().stack_store(handle, storage.slot, 0);
            }
            jett_mir::ResourceConstructorNode::Stage { field, value, .. } => {
                let handle = self
                    .builder
                    .ins()
                    .stack_load(ir::types::I64, storage.slot, 0);
                self.initialize_struct_field(handle, *field, value)?;
            }
            jett_mir::ResourceConstructorNode::Commit { output, .. } => {
                let (_, header) = original.commit().ok_or_else(|| {
                    contract_error(
                        self.symbol,
                        statement.span,
                        "Graph constructor has no exact Commit output",
                    )
                })?;
                if header.id != *output
                    || self
                        .local_slots
                        .get(output.index() as usize)
                        .copied()
                        .flatten()
                        .is_none()
                {
                    return Err(contract_error(
                        self.symbol,
                        statement.span,
                        "Graph constructor Commit has no ordinary owning output",
                    ));
                }
                let handle = self
                    .builder
                    .ins()
                    .stack_load(ir::types::I64, storage.slot, 0);
                self.define_local(
                    *output,
                    LoweredValue::Owned(handle, storage.slot),
                    statement.span,
                )?;
            }
            jett_mir::ResourceConstructorNode::End { .. } => {
                // Commit cleared this slot on success. An interrupted field prefix
                // retains its complete ordinary ownership until this exact End.
                self.drop_slot(storage.slot)?;
            }
        }
        Ok(true)
    }
}
