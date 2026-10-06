//! Ordinary ownership remains independent of the fresh Resource custody proof.
use super::*;
use crate::copy_values::CopyValuePlan;
use std::collections::HashSet;
#[cfg(test)]
mod tests;

/// Holds the fresh plan for the entire lifetime of its ordinary storage proof.
#[derive(Debug)]
pub struct ResourceCompanionPlan<'a, 'p> {
    ownership: &'a ResourceOwnershipPlan<'p>,
    function: FunctionId,
    storage: CopyValuePlan,
    caller_acquisitions: crate::CallerAcquisitions<'p>,
}
impl<'a, 'p> ResourceCompanionPlan<'a, 'p> {
    pub fn analyze(
        ownership: &'a ResourceOwnershipPlan<'p>,
        function: FunctionId,
    ) -> Result<Self, String> {
        let context = CompanionContext::new(ownership, function)?;
        let storage = crate::move_values::MoveValuePlan::analyze_companion(&context)?;
        let caller_acquisitions = crate::call_ownership::validate_function(
            context.program,
            context.function,
            context.types,
        )?;
        Ok(Self {
            ownership,
            function,
            storage,
            caller_acquisitions,
        })
    }
    pub fn ownership(&self) -> &'a ResourceOwnershipPlan<'p> {
        self.ownership
    }
    pub fn function(&self) -> FunctionId {
        self.function
    }
    pub fn storage(&self) -> &CopyValuePlan {
        &self.storage
    }
    /// Complete Source acquisitions borrowed from this exact immutable program.
    pub fn caller_acquisitions(&self) -> &crate::CallerAcquisitions<'p> {
        &self.caller_acquisitions
    }
}

pub(crate) struct CompanionContext<'p> {
    pub(crate) program: &'p Program,
    pub(crate) function: &'p Function,
    pub(crate) types: &'p TypeInterner,
    expressions: BTreeSet<usize>,
    named_values: BTreeSet<usize>,
    descriptor_values: BTreeSet<usize>,
    named_locals: HashSet<LocalId>,
}
impl<'p> CompanionContext<'p> {
    fn new(ownership: &ResourceOwnershipPlan<'p>, function: FunctionId) -> Result<Self, String> {
        let plan = ownership
            .function(function)
            .ok_or("Resource companion has no exact fresh function plan")?;
        let current = ownership
            .program
            .functions
            .get(function.index() as usize)
            .filter(|current| current.id == function)
            .ok_or("Resource companion lost its current function")?;
        let mut expressions = BTreeSet::new();
        let mut remember = |value: &Expression| {
            walk::expression(value, &mut |value| {
                expressions.insert(std::ptr::from_ref(value).addr());
            })
        };
        for block in &current.blocks {
            for statement in &block.statements {
                match &statement.kind {
                    StatementKind::ResourceCall(ResourceCallNode::Stage { value, .. })
                    | StatementKind::Let { value, .. }
                    | StatementKind::BeginCallView { value, .. }
                    | StatementKind::Evaluate(value)
                    | StatementKind::HandleDefault(value) => remember(value),
                    StatementKind::Assign { target, value } => {
                        remember(target);
                        remember(value);
                    }
                    StatementKind::Assert { condition, message } => {
                        remember(condition);
                        if let Some(message) = message {
                            remember(message);
                        }
                    }
                    StatementKind::CheckRefinement { call, .. } => remember(call),
                    StatementKind::Breakpoint { condition, .. } => {
                        if let Some(condition) = condition {
                            remember(condition);
                        }
                    }
                    _ => {} // These exact typed statements are checked by both CFG analyses.
                }
            }
            match &block.terminator.kind {
                TerminatorKind::Return(Some(value))
                | TerminatorKind::Respond(value)
                | TerminatorKind::Branch {
                    condition: value, ..
                }
                | TerminatorKind::Switch {
                    scrutinee: value, ..
                }
                | TerminatorKind::ReflectedTypeDispatch {
                    type_info: value, ..
                } => remember(value),
                _ => {}
            }
        }
        Ok(Self {
            program: ownership.program,
            function: current,
            types: ownership.types,
            expressions,
            descriptor_values: plan
                .descriptor_values
                .iter()
                .map(|(address, _)| *address)
                .collect(),
            named_values: plan
                .named_callable_values
                .iter()
                .map(|(address, _)| *address)
                .collect(),
            named_locals: current
                .resource_lowering
                .as_ref()
                .map_or_else(HashSet::new, |witness| {
                    witness
                        .named_callables
                        .iter()
                        .map(named_callables::NamedCallableBinding::local)
                        .collect()
                }),
        })
    }
    pub(crate) fn contains(&self, value: &Expression) -> bool {
        self.expressions.contains(&std::ptr::from_ref(value).addr())
    }
    pub(crate) fn resource_type(&self, ty: TypeId) -> bool {
        resource_type_pending(self.types, ty)
    }
    pub(crate) fn descriptor_expression(&self, value: &Expression) -> bool {
        self.descriptor_values
            .contains(&std::ptr::from_ref(value).addr())
    }
    pub(crate) fn resource_expression(&self, value: &Expression) -> bool {
        self.resource_type(value.ty)
            && !self
                .named_values
                .contains(&std::ptr::from_ref(value).addr())
    }
    pub(crate) fn resource_local(&self, local: &Local) -> bool {
        self.resource_type(local.ty) && !self.named_locals.contains(&local.id)
    }
}
