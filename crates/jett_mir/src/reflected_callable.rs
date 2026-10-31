//! Independent replay and sealed graphs for checked lazy reflected adapters.
//! These records grant no Resource family, entry scope or runtime capability.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::*;

#[derive(Debug)]
struct Original {
    hir: hir::Program,
    modes: HashMap<FunctionId, Vec<ParamMode>>,
    execution: resource_ownership::ResourceExecutionClosure,
    required: HashSet<FunctionId>,
}

#[derive(Debug, Clone)]
struct Graph {
    id: FunctionId,
    identity: FunctionIdentity,
    debug_kind: hir::FunctionDebugKind,
    params: Vec<Param>,
    capture_count: usize,
    return_type: TypeId,
    locals: Vec<Local>,
    entry: BlockId,
    blocks: Vec<BasicBlock>,
    span: Span,
}

impl PartialEq for Graph {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.identity == other.identity
            && self.debug_kind == other.debug_kind
            && self.params == other.params
            && self.capture_count == other.capture_count
            && self.return_type == other.return_type
            && self.locals == other.locals
            && self.entry == other.entry
            && self.span == other.span
            && breakpoint_regions::blocks_equal(&self.blocks, &other.blocks)
    }
}

impl Graph {
    fn capture(function: &Function) -> Self {
        Self {
            id: function.id,
            identity: function.identity.clone(),
            debug_kind: function.debug_kind.clone(),
            params: function.params.clone(),
            capture_count: function.capture_count,
            return_type: function.return_type,
            locals: function.locals.clone(),
            entry: function.entry,
            blocks: function.blocks.clone(),
            span: function.span,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct Witness {
    original: Arc<Original>,
    source: Arc<Graph>,
    current: Arc<Graph>,
}

impl PartialEq for Witness {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.original, &other.original)
            && Arc::ptr_eq(&self.source, &other.source)
            && Arc::ptr_eq(&self.current, &other.current)
    }
}

pub(super) fn capture(
    program: &mut Program,
    original: &hir::Program,
    modes: &HashMap<FunctionId, Vec<ParamMode>>,
    execution: &resource_ownership::ResourceExecutionClosure,
) -> Result<(), String> {
    let mut required = HashSet::new();
    let mut has_adapters = false;
    for function in &original.functions {
        for plan in hir::reflected_callable_plans(function) {
            if !plan.validate_original() {
                return Err("reflected callable lacks original HIR constructor evidence".into());
            }
            has_adapters = true;
            required.insert(function.id);
            required.insert(plan.function());
            required.insert(
                plan.original_owner()
                    .ok_or("reflected callable original owner is missing")?,
            );
            for predicate in plan.original_predicates() {
                required.insert(predicate.id);
            }
        }
    }
    if !has_adapters {
        return Ok(());
    }
    // Original descriptor targets and capture producers remain authenticated,
    // including helpers whose signatures alone cannot identify their bodies.
    required.extend(original.functions.iter().map(|function| function.id));
    let original = Arc::new(Original {
        hir: original.clone(),
        modes: modes.clone(),
        execution: execution.clone(),
        required,
    });
    for function in &mut program.functions {
        if original.required.contains(&function.id) {
            let graph = Arc::new(Graph::capture(function));
            function.reflected_callable = Some(Witness {
                original: original.clone(),
                source: graph.clone(),
                current: graph,
            });
        }
    }
    Ok(())
}

/// Called only beside canonical preparation after the complete input gate.
/// A changed public graph cannot become new authority through this function.
pub(super) fn prepared(function: &mut Function, before: &Function) -> Result<(), String> {
    let Some(previous) = &before.reflected_callable else {
        if function.reflected_callable.is_some() {
            return Err("reflected callable witness appeared during preparation".into());
        }
        return Ok(());
    };
    if previous.current.as_ref() != &Graph::capture(before)
        || function.reflected_callable.as_ref() != Some(previous)
    {
        return Err("reflected callable preparation has no exact original current graph".into());
    }
    let current = Arc::new(Graph::capture(function));
    let mut witness = previous.clone();
    witness.current = current;
    function.reflected_callable = Some(witness);
    Ok(())
}

fn private_expression(value: &Expression) -> bool {
    use hir::ExpressionKind as E;
    matches!(
        &value.kind,
        E::Intrinsic {
            field_validation: Some(hir::ReflectedFieldValidation::ReadCallable(_)),
            ..
        }
    ) || matches!(&value.kind, E::IndirectCall { ownership: hir::CallOwnership::Generated(generated), .. }
            if matches!(generated.operation, hir::GeneratedOperation::ReflectedCallableAdapter { .. }))
}

fn has_private_operations(function: &Function) -> bool {
    function.blocks.iter().any(|block| {
        let mut found = block
            .statements
            .iter()
            .any(|statement| matches!(statement.kind, StatementKind::ReflectedValueReady { .. }));
        resource_ownership::walk::mir_block(block, &mut |expression| {
            found |= private_expression(expression)
        });
        found
    })
}

pub fn validate_reflected_callable_plans(
    program: &Program,
    types: &TypeInterner,
) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();
    let mut originals = Vec::<Arc<Original>>::new();
    for function in &program.functions {
        let Some(witness) = &function.reflected_callable else {
            let mut copied_adapter = false;
            for block in &function.blocks {
                resource_ownership::walk::mir_block(block, &mut |value| {
                    if let hir::ExpressionKind::FunctionAdapter {
                        function: target, ..
                    } = &value.kind
                    {
                        copied_adapter |= program
                            .functions
                            .get(target.index() as usize)
                            .is_some_and(has_private_operations);
                    }
                });
            }
            if has_private_operations(function) || copied_adapter {
                errors.push(ValidationError {
                    span: function.span,
                    message: "reflected callable MIR operation has no original private witness"
                        .into(),
                });
            }
            continue;
        };
        if !originals
            .iter()
            .any(|original| Arc::ptr_eq(original, &witness.original))
        {
            originals.push(witness.original.clone());
        }
        if witness.current.as_ref() != &Graph::capture(function) || witness.source.id != function.id
        {
            errors.push(ValidationError {
                span: function.span,
                message: "reflected callable current body, tuple, readiness or context changed"
                    .into(),
            });
            continue;
        }
        let Some(original) = witness
            .original
            .hir
            .functions
            .get(function.id.index() as usize)
        else {
            errors.push(ValidationError {
                span: function.span,
                message: "reflected callable MIR lost its original function".into(),
            });
            continue;
        };
        match super::lower_function_body(original, types, &witness.original.modes, &witness.original.hir.resource_manifest, &witness.original.hir.resource_source, &witness.original.execution) {
            Ok(reconstructed) if Graph::capture(&reconstructed) == *witness.source => {}
            _ => errors.push(ValidationError { span: function.span, message: "reflected callable original adapter/predicate/tuple CFG cannot be reconstructed".into() }),
        }
    }
    for original in originals {
        if let Err(hir_errors) = hir::validate(&original.hir)
            .and_then(|()| hir::validate_backend_types(&original.hir, types))
        {
            errors.extend(hir_errors.into_iter().map(|error| ValidationError {
                span: error.span,
                message: error.message,
            }));
        }
        for required in &original.required {
            if !program
                .functions
                .get(required.index() as usize)
                .is_some_and(|function| {
                    function
                        .reflected_callable
                        .as_ref()
                        .is_some_and(|witness| Arc::ptr_eq(&witness.original, &original))
                })
            {
                errors.push(ValidationError { span: original.hir.functions[required.index() as usize].span, message: "reflected callable original owner, descriptor or predicate witness was removed".into() });
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests;
