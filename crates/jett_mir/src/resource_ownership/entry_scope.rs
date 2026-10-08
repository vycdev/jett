//! A selected native wrapper Scope does not make its ordinary body a Source family.
use super::*;

/// Wrapper-only metadata borrowed from a fully authenticated current Program.
#[derive(Debug)]
pub struct ResourceEntryScopePlan {
    function: FunctionId,
    identity: hir::FunctionIdentity,
    parameters: Vec<Param>,
    return_type: TypeId,
    root_scope: ResourceFrame,
    completion: ResourceOperation,
}
impl ResourceEntryScopePlan {
    pub fn function(&self) -> FunctionId {
        self.function
    }
    pub fn identity(&self) -> &hir::FunctionIdentity {
        &self.identity
    }
    pub fn parameters(&self) -> &[Param] {
        &self.parameters
    }
    pub fn return_type(&self) -> TypeId {
        self.return_type
    }
    pub fn root_scope(&self) -> &ResourceFrame {
        &self.root_scope
    }
    pub fn completion(&self) -> &ResourceOperation {
        &self.completion
    }
}

pub(super) fn capture_needed(
    function: &hir::Function,
    source: &hir::ResourceSourceArchive,
) -> bool {
    source.manifest().is_some()
        && function.capture_count == 0
        && function.return_type == TypeInterner::NOTHING
        && function
            .params
            .iter()
            .all(|param| param.ty == TypeInterner::NETWORK)
}

/// Selects an authenticated wrapper Scope without changing execution-family membership.
pub fn validate_resource_ownership_for_entry<'p>(
    program: &'p Program,
    types: &'p TypeInterner,
    entry: FunctionId,
) -> Result<ResourceOwnershipPlan<'p>, Vec<ValidationError>> {
    crate::validate(program)?;
    let mut plan = validate_resource_ownership(program, types)?;
    let span = program
        .functions
        .first()
        .map_or(Span::new(jett_common::FileId::new(0), 0, 0), |function| {
            function.span
        });
    let selected = program
        .functions
        .get(entry.index() as usize)
        .filter(|function| function.id == entry)
        .ok_or_else(|| {
            vec![ValidationError {
                span,
                message: "native Resource entry has no exact current function".into(),
            }]
        })?;
    let refusal = |message: &str| {
        vec![ValidationError {
            span: selected.span,
            message: message.into(),
        }]
    };
    if plan.required_only_function_ids().contains(&entry) {
        return Err(refusal(
            "required-only Resource helper cannot be a native entry",
        ));
    }
    let witness = selected.resource_lowering.as_ref().ok_or_else(|| {
        refusal("native Resource entry lacks its initially authenticated Source witness")
    })?;
    let original = plan
        .original_source(entry)
        .ok_or_else(|| refusal("native Resource entry has no exact original Source function"))?;
    if !capture_needed(original, &witness.source)
        || !capture_needed(&witness.original, &witness.source)
        || selected.identity != original.identity
        || selected.capture_count != 0
        || selected.return_type != original.return_type
        || selected.params.len() != original.params.len()
        || !selected
            .params
            .iter()
            .zip(&original.params)
            .all(|(current, archived)| {
                current.name == archived.name
                    && current.ty == archived.ty
                    && current.mode == archived.mode
                    && current.mutable == archived.mutable
                    && current.span == archived.span
            })
    {
        return Err(refusal(
            "native Resource entry changed its closed Network/Nothing Source header",
        ));
    }
    if plan.function(entry).is_some() {
        return Ok(plan);
    }
    if has_execution_records(selected, types) {
        return Err(refusal(
            "native entry cannot replace a missing Source execution-family plan",
        ));
    }
    ControlFlowGraph::analyze(selected).map_err(|errors| {
        errors
            .into_iter()
            .map(|error| ValidationError {
                span: selected.span,
                message: format!("native Resource entry CFG: {}", error.message),
            })
            .collect::<Vec<_>>()
    })?;
    let site = ResourceSite {
        function: entry,
        block: selected.entry,
        position: ResourcePosition::Terminator,
    };
    let frame = ResourceFrameId(0);
    plan.entry_scope = Some(ResourceEntryScopePlan {
        function: entry,
        identity: selected.identity.clone(),
        parameters: selected.params.clone(),
        return_type: selected.return_type,
        root_scope: ResourceFrame {
            id: frame,
            role: ResourceFrameRole::Scope,
            site,
            parent: None,
            ordinal: 0,
        },
        completion: ResourceOperation {
            id: ResourceOperationId(0),
            frame,
            site,
            ordinal: 0,
            role: ResourceOperationRole::Complete {
                outcome: ResourceCompletion::Return,
            },
            expression: None,
            named_indirect: None,
            indirect_hook: None,
        },
    });
    Ok(plan)
}

#[cfg(test)]
mod tests;
