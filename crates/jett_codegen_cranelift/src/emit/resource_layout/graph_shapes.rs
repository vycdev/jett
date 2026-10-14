//! Ordinary native word representation selected only by protected Graph rows.
use super::*;

#[path = "source_edge_locator.rs"]
mod source_edges;

fn current_function<'p>(
    plan: &custody::ResourceOwnershipPlan<'p>,
    selected: &custody::ResourceFunctionPlan,
) -> Result<&'p Function, CodegenError> {
    plan.program()
        .functions
        .get(selected.function().index() as usize)
        .filter(|current| {
            current.id == selected.function()
                && &current.identity == selected.identity()
                && current.params.as_slice() == selected.parameters()
                && current.return_type == selected.return_type()
        })
        .ok_or_else(|| pending("Graph shape has no exact current fresh function header"))
}

impl Rows<'_, '_> {
    fn graph_storage_shape(
        &mut self,
        proof: &custody::GraphStorageProof,
        ty: TypeId,
    ) -> Result<u32, CodegenError> {
        if proof.ty() != ty {
            return Err(pending("Graph shape proof changes its exact Source type"));
        }
        let kind =
            crate::verify::native_storage_kind(self.plan.types(), ty, "Graph ordinary word")?;
        if !matches!(
            kind,
            ScalarKind::Struct
                | ScalarKind::Enum
                | ScalarKind::Bitfield
                | ScalarKind::Machine
                | ScalarKind::Function
                | ScalarKind::Actor
                | ScalarKind::List
                | ScalarKind::Set
                | ScalarKind::Map
                | ScalarKind::Sum
                | ScalarKind::Construction
        ) {
            return self.shape(ty);
        }
        // This ordinary word grants no custody. Do not cache it by Source TypeId:
        // another value of that type must select its own protected Graph proof.
        let words = vec![1, 64, 0];
        if let Some(index) = self.shapes.iter().position(|shape| shape == &words) {
            return ordinal(index);
        }
        let id = ordinal(self.shapes.len())?;
        self.shapes.push(words);
        Ok(id)
    }

    pub(super) fn graph_function_result(
        &mut self,
        function: &custody::ResourceFunctionPlan,
    ) -> Result<Option<u32>, CodegenError> {
        let current = current_function(self.plan, function)?;
        custody::native_graph_storage_for_return(current, self.plan.types())
            .map(|proof| self.graph_storage_shape(&proof, function.return_type()))
            .transpose()
    }

    pub(super) fn function_signature_result(
        &mut self,
        function: &custody::ResourceFunctionPlan,
        result: u32,
    ) -> Result<u32, CodegenError> {
        let current = current_function(self.plan, function)?;
        let mut parameters = Vec::new();
        for param in &current.params {
            let shape =
                match custody::native_graph_storage_for_formal(current, self.plan.types(), param) {
                    Some(proof) => self.graph_storage_shape(&proof, param.ty)?,
                    None => self.shape(param.ty)?,
                };
            parameters.push((
                shape,
                if param.mode == jett_mir::ParamMode::View {
                    2
                } else {
                    1
                },
            ));
        }
        self.shaped_signature(parameters, result)
    }

    pub(super) fn source_shapes(
        &mut self,
        caller: &custody::ResourceFunctionPlan,
        operation: &custody::ResourceOperation,
        callee: &custody::ResourceFunctionPlan,
        param: &jett_mir::Param,
        fact: &custody::ResourceCallFormal,
        operand: &Operand,
        order: &[usize],
    ) -> Result<(u32, u32), CodegenError> {
        let current = current_function(self.plan, callee)?;
        let physical = current
            .params
            .get(fact.parameter())
            .filter(|physical| *physical == param)
            .ok_or_else(|| pending("Graph Source formal is not the exact fresh header"))?;
        let Some(formal) =
            custody::native_graph_storage_for_formal(current, self.plan.types(), physical)
        else {
            return Ok((self.shape(fact.actual_type())?, self.shape(param.ty)?));
        };
        let edge = source_edges::source_edge(
            current_function(self.plan, caller)?,
            self.plan.types(),
            caller,
            operation,
            callee.function(),
            order,
            fact.parameter(),
        )?
        .ok_or_else(|| pending("Graph Source shape lacks its exact whole-call edge proof"))?;
        if !matches!(operand, Operand::Ordinary { parameter, ty }
                    if *parameter == fact.parameter() && *ty == fact.actual_type())
            || edge.parameter_index() != fact.parameter()
            || edge.original_formal().ty != param.ty
            || edge.original_formal().mode != param.mode
            || edge.original_actual().ty != fact.actual_type()
        {
            return Err(pending(
                "Graph Source edge changes its exact ordinary formal/actual tuple",
            ));
        }
        Ok((
            self.graph_storage_shape(edge.storage(), fact.actual_type())?,
            self.graph_storage_shape(&formal, param.ty)?,
        ))
    }
}
