//! Private canonical suite calls activate checked bodies without inventing a
//! Source call certificate. Original helper calls keep their Source authority.
use super::*;

fn canonical(witness: &ResourceLoweringWitness) -> Result<Option<&hir::Function>, String> {
    let Some(function) = witness.source.native_suite_function(witness.original.id) else {
        return Ok(None);
    };
    if !original_function_equal(function, &witness.original) {
        return Err("native suite differs from its private complete HIR table".into());
    }
    Ok(Some(function))
}

pub(super) fn is_suite(function: &Function) -> bool {
    function
        .resource_lowering
        .as_ref()
        .is_some_and(|witness| witness.source.native_suite_function(function.id).is_some())
}

/// Preserve the canonical expression for later positional replay. This grants
/// no invocation authority; analyze independently joins each exact occurrence.
pub(super) fn preserve(
    witness: &ResourceLoweringWitness,
    expression: &Expression,
) -> Result<bool, String> {
    if canonical(witness)?.is_none() {
        return Ok(false);
    }
    Ok(matches!(&expression.kind,
        hir::ExpressionKind::Call {
            function,
            ownership: hir::CallOwnership::Generated(generated),
            ..
        } if matches!(&generated.operation,
            hir::GeneratedOperation::NativeSuite { function: expected } if expected == function)))
}

fn call_metadata(
    program: &Program,
    function: &Function,
    expression: &Expression,
    proof: &hir::CheckedNativeSuiteCall,
    types: &TypeInterner,
) -> Result<ResourceNativeSuiteInvocation, String> {
    let hir::ExpressionKind::Call {
        function: target,
        args,
        evaluation_order,
        ownership: hir::CallOwnership::Generated(generated),
    } = &expression.kind
    else {
        return Err("native suite receipt lost its exact generated Call".into());
    };
    let callee = program
        .functions
        .get(target.index() as usize)
        .filter(|callee| callee.id == *target)
        .ok_or("native suite call lost its exact checked declaration")?;
    if proof.owner() != function.id
        || proof.target() != *target
        || proof.evaluation_order() != evaluation_order
        || !proof.matches(function.id, expression, proof.expression_ordinal())
        || !matches!(&generated.operation,
            hir::GeneratedOperation::NativeSuite { function } if function == target)
        || expression.ty != TypeInterner::NOTHING
        || callee.return_type != TypeInterner::NOTHING
        || callee.capture_count != 0
        || !matches!(
            callee.identity.declaration.kind,
            hir::DeclarationKind::Verify | hir::DeclarationKind::Property
        )
        || args.len() != callee.params.len()
        || args.len() != generated.arguments.len()
        || args.len() != evaluation_order.len()
    {
        return Err("native suite call changed its sealed target or full tuple".into());
    }
    let mut seen = vec![false; args.len()];
    for &parameter in evaluation_order {
        let visited = seen
            .get_mut(parameter)
            .ok_or("native suite argument order is outside its exact tuple")?;
        if *visited {
            return Err("native suite argument order repeats a parameter".into());
        }
        *visited = true;
    }
    let mut formals = Vec::with_capacity(args.len());
    let mut operands = Vec::with_capacity(args.len());
    for (parameter, (argument, formal)) in args.iter().zip(&callee.params).enumerate() {
        let source_index = evaluation_order
            .iter()
            .position(|actual| *actual == parameter)
            .ok_or("native suite argument order omitted a parameter")?;
        let fact = &generated.arguments[parameter];
        if argument.ty != formal.ty
            || fact.actual_type != argument.ty
            || fact.parameter_type != formal.ty
            || fact.callee_access != jett_typecheck::CheckedCalleeAccess::Owned
            || formal.mode != ParamMode::Owned
            || fact.acquisition != hir::GeneratedAcquisition::Copy
            || resource_type_pending(types, argument.ty)
            || !jett_typecheck::ownership::is_implicitly_copyable(types, argument.ty)
        {
            return Err("pending native Resource suite: generated inputs require exact ordinary copy formals".into());
        }
        formals.push(ResourceCallFormal {
            parameter,
            source_index,
            actual_type: argument.ty,
            parameter_type: formal.ty,
            syntax: ResourceArgumentSyntax::Bare,
            effect: ResourceArgumentEffect::Copy,
            access: ParamMode::Owned,
        });
        operands.push(ResourceCallOperand::Ordinary {
            parameter,
            ty: argument.ty,
        });
    }
    Ok(ResourceNativeSuiteInvocation {
        function: *target,
        proof: proof.clone(),
        formals,
        evaluation_order: evaluation_order.clone(),
        operands,
        result: ResourceCallResult::Ordinary {
            ty: TypeInterner::NOTHING,
        },
    })
}

fn operation(
    plan: &mut ResourceFunctionPlan,
    frame: ResourceFrameId,
    site: ResourceSite,
    expression: Option<&Expression>,
    role: ResourceOperationRole,
) {
    plan.operations.push(ResourceOperation {
        id: ResourceOperationId(plan.operations.len()),
        frame,
        site,
        ordinal: usize::from(expression.is_some()),
        role,
        expression: expression.map(|value| std::ptr::from_ref(value).addr()),
        named_indirect: None,
        indirect_hook: None,
    });
}

fn empty_plan(function: &Function, terminal: ResourceSite) -> ResourceFunctionPlan {
    ResourceFunctionPlan {
        function: function.id,
        identity: function.identity.clone(),
        parameters: function.params.clone(),
        return_type: function.return_type,
        frames: vec![ResourceFrame {
            id: ResourceFrameId(0),
            role: ResourceFrameRole::Scope,
            site: terminal,
            parent: None,
            ordinal: 0,
        }],
        carriers: None,
        slots: Vec::new(),
        loans: Vec::new(),
        operations: Vec::new(),
        execution_frames: Vec::new(),
        lexical_exits: Vec::new(),
        named_callable_producers: Vec::new(),
        named_callable_values: Vec::new(),
        descriptor_return: None,
        descriptor_values: Vec::new(),
        descriptor_locals: Vec::new(),
        graph_descriptors: Vec::new(),
    }
}

/// A factory wrapper is currently a flat, closed sequence of case contexts and
/// calls with ordinary copy inputs. Exact statement positions distinguish even
/// identical trial values; neither spans nor structural first-match selects one.
pub(super) fn analyze(
    program: &Program,
    function: &Function,
    types: &TypeInterner,
) -> Result<ResourceFunctionPlan, String> {
    let witness = function
        .resource_lowering
        .as_ref()
        .ok_or("native suite has no constructor witness")?;
    witness.current(function)?;
    let original = canonical(witness)?.ok_or("native suite has no private factory origin")?;
    if !function.params.is_empty()
        || function.capture_count != 0
        || function.return_type != TypeInterner::NOTHING
        || !function.locals.is_empty()
        || function.blocks.len() != 1
    {
        return Err(
            "pending native Resource suite: wrapper requires its closed flat original scope".into(),
        );
    }
    let block = &function.blocks[0];
    if block.id != function.entry
        || !matches!(block.terminator.kind, TerminatorKind::Return(None))
        || block.statements.len() != original.body.statements.len()
    {
        return Err("native suite lost its exact flat statement/return association".into());
    }
    ControlFlowGraph::analyze(function)
        .map_err(|errors| format!("native suite CFG is malformed: {errors:?}"))?;
    let terminal = ResourceSite {
        function: function.id,
        block: block.id,
        position: ResourcePosition::Terminator,
    };
    let mut plan = empty_plan(function, terminal);
    let mut global_ordinal = 0;
    for (index, (archived, current)) in original
        .body
        .statements
        .iter()
        .zip(&block.statements)
        .enumerate()
    {
        let (hir::StatementKind::Expression(original), StatementKind::Evaluate(expression)) =
            (&archived.kind, &current.kind)
        else {
            return Err("pending native Resource suite: wrapper statement needs its exact expression transport".into());
        };
        if archived.span != current.span
            || !crate::breakpoint_regions::expressions_equal(original, expression)
        {
            return Err(
                "native suite changed an exact original/current statement occurrence".into(),
            );
        }
        let ordinal = global_ordinal + 1;
        walk::expression(original, &mut |_| global_ordinal += 1);
        let site = ResourceSite {
            function: function.id,
            block: block.id,
            position: ResourcePosition::Statement(index),
        };
        plan.execution_frames.push((site, ResourceFrameId(0)));
        match &expression.kind {
            hir::ExpressionKind::PropertyCaseContext(_) => {}
            hir::ExpressionKind::Call { .. } => {
                let proof = witness
                    .source
                    .native_suite_call(function.id, original, ordinal)
                    .ok_or("native suite occurrence has no exact private call receipt")?;
                if !proof.belongs_to_archive(&witness.source)
                    || !proof.matches(function.id, expression, ordinal)
                {
                    return Err(
                        "native suite occurrence belongs to a different checked plan".into(),
                    );
                }
                let invocation = call_metadata(program, function, expression, proof, types)?;
                let callee = &program.functions[invocation.function.index() as usize];
                if !has_execution_records(callee, types) {
                    // Ordinary targets keep their existing generated-call ABI.
                    continue;
                }
                let frame = ResourceFrameId(plan.frames.len());
                plan.frames.push(ResourceFrame {
                    id: frame,
                    role: ResourceFrameRole::Operation,
                    site,
                    parent: Some(ResourceFrameId(0)),
                    ordinal: 1,
                });
                operation(
                    &mut plan,
                    frame,
                    site,
                    Some(expression),
                    ResourceOperationRole::InvokeNativeSuite {
                        invocation: Box::new(invocation),
                    },
                );
                operation(
                    &mut plan,
                    frame,
                    site,
                    Some(expression),
                    ResourceOperationRole::Complete {
                        outcome: ResourceCompletion::Normal,
                    },
                );
            }
            _ => return Err("native suite contains a noncanonical factory expression".into()),
        }
    }
    plan.execution_frames.push((terminal, ResourceFrameId(0)));
    operation(
        &mut plan,
        ResourceFrameId(0),
        terminal,
        None,
        ResourceOperationRole::Complete {
            outcome: ResourceCompletion::Return,
        },
    );
    Ok(plan)
}

#[cfg(test)]
mod tests;
