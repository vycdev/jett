//! Dependency-only traversal; visits neither mint custody nor change context.
use super::*;
use hir::{ExpressionKind as E, StatementKind as H};

pub(super) fn expression(value: &Expression, visit: &mut impl FnMut(&Expression)) {
    visit(value);
    match &value.kind {
        E::Binary { left, right, .. } => {
            expression(left, visit);
            expression(right, visit);
        }
        E::Unary { value, .. }
        | E::ResultOk(value)
        | E::ResultFail(value)
        | E::OptionalSome(value)
        | E::DisplayResult(value)
        | E::EquatableResult(value)
        | E::Declassify(value)
        | E::Coarsen(value)
        | E::RefinementValidated(value)
        | E::InterfaceType(value)
        | E::RuntimeFailureMessage(value)
        | E::Run(value)
        | E::Join(value)
        | E::Cancel(value)
        | E::View(value)
        | E::Clone(value)
        | E::Comptime { value, .. }
        | E::InterfaceCoerce { value, .. }
        | E::FunctionAdapter { value, .. }
        | E::StateIs { value, .. } => expression(value, visit),
        E::Call { args, .. }
        | E::ResourceInvoke { args, .. }
        | E::Intrinsic { args, .. }
        | E::ActorSpawn { args, .. } => {
            for value in args {
                expression(value, visit);
            }
        }
        E::IndirectCall { callee, args, .. } => {
            expression(callee, visit);
            for value in args {
                expression(value, visit);
            }
        }
        E::ActorMessage { actor, args, .. } => {
            expression(actor, visit);
            for value in args {
                expression(value, visit);
            }
        }
        E::StructConstruct { fields, .. } | E::BitfieldConstruct { fields, .. } => {
            for value in fields {
                expression(value, visit);
            }
        }
        E::MachineConstruct { payloads, .. } | E::EnumConstruct { payloads, .. } => {
            for value in payloads {
                expression(value, visit);
            }
        }
        E::MachineTransition {
            source, payloads, ..
        } => {
            expression(source, visit);
            for value in payloads {
                expression(value, visit);
            }
        }
        E::ListConstruct { elements } => {
            for value in elements {
                expression(value, visit);
            }
        }
        E::MapConstruct { entries } => {
            for entry in entries {
                expression(&entry.key, visit);
                expression(&entry.value, visit);
            }
        }
        E::Handle {
            target, failure, ..
        } => {
            expression(target, visit);
            hir_block(failure, visit);
        }
        E::StringInterpolation(parts) => {
            for part in parts {
                if let hir::StringSegment::Value(value) = part {
                    expression(value, visit);
                }
            }
        }
        E::InlineFunction { body, .. } => hir_block(body, visit),
        E::Field { base, .. } => expression(base, visit),
        E::Int(_)
        | E::Float(_)
        | E::String(_)
        | E::Bool(_)
        | E::Nothing
        | E::Local(_)
        | E::Constant { .. }
        | E::FunctionRef(_)
        | E::ResourceHookValue { .. }
        | E::ClosureRef { .. }
        | E::OptionalNone
        | E::RuntimeFailure(_)
        | E::PropertyCaseContext(_) => {}
    }
}
pub(super) fn hir_block(block: &hir::Block, visit: &mut impl FnMut(&Expression)) {
    for statement in &block.statements {
        match &statement.kind {
            H::Let { value, .. }
            | H::HandleDefault(value)
            | H::Expression(value)
            | H::Respond(value) => expression(value, visit),
            H::Assign { target, value } => {
                expression(target, visit);
                expression(value, visit);
            }
            H::Return(value) => {
                if let Some(value) = value {
                    expression(value, visit);
                }
            }
            H::If {
                condition,
                then_block,
                else_block,
            } => {
                expression(condition, visit);
                hir_block(then_block, visit);
                if let Some(block) = else_block {
                    hir_block(block, visit);
                }
            }
            H::While { condition, body } => {
                expression(condition, visit);
                hir_block(body, visit);
            }
            H::For { iterable, body, .. } => {
                expression(iterable, visit);
                hir_block(body, visit);
            }
            H::Match { scrutinee, arms } => {
                expression(scrutinee, visit);
                for arm in arms {
                    hir_block(&arm.body, visit);
                }
            }
            H::Assert { condition, message } => {
                expression(condition, visit);
                if let Some(value) = message {
                    expression(value, visit);
                }
            }
            H::Breakpoint { condition, .. } => {
                if let Some(value) = condition {
                    expression(value, visit);
                }
            }
            H::Scope(block) => hir_block(block, visit),
            H::ReflectedTypeDispatch { type_info, arms } => {
                expression(type_info, visit);
                for arm in arms {
                    hir_block(&arm.body, visit);
                }
            }
            H::Break | H::Continue | H::Trace(_) => {}
        }
    }
}
pub(super) fn mir_block(block: &BasicBlock, visit: &mut impl FnMut(&Expression)) {
    for statement in &block.statements {
        match &statement.kind {
            StatementKind::ResourceCall(ResourceCallNode::Stage { value, .. })
            | StatementKind::Let { value, .. }
            | StatementKind::BeginCallView { value, .. }
            | StatementKind::CheckRefinement { call: value, .. }
            | StatementKind::Evaluate(value)
            | StatementKind::HandleDefault(value) => expression(value, visit),
            StatementKind::Assign { target, value } => {
                expression(target, visit);
                expression(value, visit);
            }
            StatementKind::Assert { condition, message } => {
                expression(condition, visit);
                if let Some(value) = message {
                    expression(value, visit);
                }
            }
            StatementKind::Breakpoint { condition, .. } => {
                if let Some(value) = condition {
                    expression(value, visit);
                }
            }
            StatementKind::ResourceCall(
                ResourceCallNode::Begin { .. }
                | ResourceCallNode::Invoke { .. }
                | ResourceCallNode::End { .. },
            )
            | StatementKind::ResourceLexicalExit(_)
            | StatementKind::OpenCallOwnerGeneration { .. }
            | StatementKind::ReplaceCallOwnerGeneration { .. }
            | StatementKind::CloseCallOwnerGeneration { .. }
            | StatementKind::EndCallView { .. }
            | StatementKind::ReflectedContainerReady { .. }
            | StatementKind::SequenceLength { .. }
            | StatementKind::SequenceGet { .. }
            | StatementKind::IterationBorrow { .. }
            | StatementKind::SumTag { .. }
            | StatementKind::SumTake { .. }
            | StatementKind::Trace(_) => {}
        }
    }
    match &block.terminator.kind {
        TerminatorKind::Return(value) => {
            if let Some(value) = value {
                expression(value, visit);
            }
        }
        TerminatorKind::Respond(value)
        | TerminatorKind::Branch {
            condition: value, ..
        }
        | TerminatorKind::Switch {
            scrutinee: value, ..
        }
        | TerminatorKind::ForEach {
            iterable: value, ..
        }
        | TerminatorKind::ReflectedTypeDispatch {
            type_info: value, ..
        } => expression(value, visit),
        TerminatorKind::Goto(_) | TerminatorKind::Unreachable => {}
    }
}
pub(super) fn mir_block_has_custody(
    block: &BasicBlock,
    function: &Function,
    types: &TypeInterner,
) -> bool {
    let mut found = false;
    mir_block(block, &mut |value| {
        found |= resource_type_pending(types, value.ty)
            && !matches!(types.resolve(value.ty), Type::Function { .. });
    });
    // SumTag/SumTake hold typed local headers rather than expression nodes.
    // Normalized calls also own an exact operation prefix at header-only sites.
    let occupied = |id| {
        function
            .local(id)
            .is_some_and(|local| custody_type(types, local.ty))
    };
    found
        || block
            .statements
            .iter()
            .any(|statement| match &statement.kind {
                StatementKind::SumTag { source, target }
                | StatementKind::SumTake { source, target, .. } => {
                    occupied(*source) || occupied(*target)
                }
                StatementKind::ResourceCall(_) => true,
                _ => false,
            })
}
pub(super) fn mir_block_has_resource(block: &BasicBlock) -> bool {
    let mut found = false;
    mir_block(block, &mut |value| {
        found |= matches!(
            value.kind,
            E::ResourceInvoke { .. } | E::ResourceHookValue { .. }
        );
    });
    found
}

/// Source scope admission is finite: no resource declaration gets a guessed root lifetime.
pub(super) fn nested_resource_declaration(
    block: &hir::Block,
    function: &hir::Function,
    types: &TypeInterner,
    nested: bool,
    admitted: &[LocalId],
) -> bool {
    for statement in &block.statements {
        if let H::Let { local, .. } = &statement.kind
            && nested
            && !admitted.contains(local)
            && function
                .locals
                .get(local.index() as usize)
                .is_some_and(|local| resource_type_pending(types, local.ty))
        {
            return true;
        }
        let found = match &statement.kind {
            H::If {
                then_block,
                else_block,
                ..
            } => {
                nested_resource_declaration(then_block, function, types, true, admitted)
                    || else_block.as_ref().is_some_and(|block| {
                        nested_resource_declaration(block, function, types, true, admitted)
                    })
            }
            H::While { body, .. } | H::For { body, .. } | H::Scope(body) => {
                nested_resource_declaration(body, function, types, true, admitted)
            }
            H::Match { arms, .. } => arms
                .iter()
                .any(|arm| nested_resource_declaration(&arm.body, function, types, true, admitted)),
            H::ReflectedTypeDispatch { arms, .. } => arms
                .iter()
                .any(|arm| nested_resource_declaration(&arm.body, function, types, true, admitted)),
            _ => false,
        };
        if found {
            return true;
        }
        let mut inside = false;
        let single = hir::Block {
            statements: vec![statement.clone()],
            span: statement.span,
        };
        hir_block(&single, &mut |value| {
            if let E::Handle { failure, .. } = &value.kind {
                inside |= nested_resource_declaration(failure, function, types, true, admitted);
            }
        });
        if inside {
            return true;
        }
    }
    false
}

/// Visit only expressions physically owned by this exact current statement/terminator.
pub(super) fn mir_site(block: &BasicBlock, position: usize, visit: &mut impl FnMut(&Expression)) {
    if let Some(statement) = block.statements.get(position) {
        match &statement.kind {
            StatementKind::ResourceCall(ResourceCallNode::Stage { value, .. })
            | StatementKind::Let { value, .. }
            | StatementKind::BeginCallView { value, .. }
            | StatementKind::CheckRefinement { call: value, .. }
            | StatementKind::Evaluate(value)
            | StatementKind::HandleDefault(value) => expression(value, visit),
            StatementKind::Assign { target, value } => {
                expression(target, visit);
                expression(value, visit);
            }
            StatementKind::Assert { condition, message } => {
                expression(condition, visit);
                if let Some(value) = message {
                    expression(value, visit);
                }
            }
            StatementKind::Breakpoint { condition, .. } => {
                if let Some(value) = condition {
                    expression(value, visit);
                }
            }
            StatementKind::ResourceCall(
                ResourceCallNode::Begin { .. }
                | ResourceCallNode::Invoke { .. }
                | ResourceCallNode::End { .. },
            )
            | StatementKind::ResourceLexicalExit(_)
            | StatementKind::OpenCallOwnerGeneration { .. }
            | StatementKind::ReplaceCallOwnerGeneration { .. }
            | StatementKind::CloseCallOwnerGeneration { .. }
            | StatementKind::EndCallView { .. }
            | StatementKind::ReflectedContainerReady { .. }
            | StatementKind::SequenceLength { .. }
            | StatementKind::SequenceGet { .. }
            | StatementKind::IterationBorrow { .. }
            | StatementKind::SumTag { .. }
            | StatementKind::SumTake { .. }
            | StatementKind::Trace(_) => {}
        }
    } else if position == block.statements.len() {
        match &block.terminator.kind {
            TerminatorKind::Return(value) => {
                if let Some(value) = value {
                    expression(value, visit);
                }
            }
            TerminatorKind::Respond(value)
            | TerminatorKind::Branch {
                condition: value, ..
            }
            | TerminatorKind::Switch {
                scrutinee: value, ..
            }
            | TerminatorKind::ForEach {
                iterable: value, ..
            }
            | TerminatorKind::ReflectedTypeDispatch {
                type_info: value, ..
            } => expression(value, visit),
            TerminatorKind::Goto(_) | TerminatorKind::Unreachable => {}
        }
    }
}
