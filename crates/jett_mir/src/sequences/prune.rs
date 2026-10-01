//! Compact functions selected by bounded native preparation passes.
//! The input program is validated before each preparation; all references
//! below therefore name dense local/block tables, including unreachable input.
use super::*;

pub(crate) fn unreachable(function: &mut Function) {
    let Ok(cfg) = ControlFlowGraph::analyze(function) else {
        return;
    };
    let mut reachable = vec![false; function.blocks.len()];
    let mut pending = vec![function.entry];
    while let Some(block) = pending.pop() {
        if reachable[block.index() as usize] {
            continue;
        }
        reachable[block.index() as usize] = true;
        pending.extend_from_slice(cfg.successors(block));
    }
    let mut blocks = vec![None; function.blocks.len()];
    function
        .blocks
        .retain(|block| reachable[block.id.index() as usize]);
    for (index, block) in function.blocks.iter_mut().enumerate() {
        let new = BlockId(index as u32);
        blocks[block.id.index() as usize] = Some(new);
        block.id = new;
    }
    let remap_block = |id: &mut BlockId| {
        *id = blocks[id.index() as usize].expect("edge from reachable block has reachable target");
    };
    remap_block(&mut function.entry);
    for block in &mut function.blocks {
        block_targets(&mut block.terminator.kind, &remap_block);
    }

    let mut used = vec![false; function.locals.len()];
    for parameter in &function.params {
        used[parameter.local.index() as usize] = true;
    }
    for block in &mut function.blocks {
        block_locals(
            block,
            &mut |id| used[id.index() as usize] = true,
            &mut |_| {},
        );
    }
    loop {
        let mut changed = false;
        for local in &function.locals {
            if used[local.id.index() as usize]
                && let Some(source) = local.view_source
                && !used[source.index() as usize]
            {
                used[source.index() as usize] = true;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let mut locals = vec![None; function.locals.len()];
    function
        .locals
        .retain(|local| used[local.id.index() as usize]);
    for (index, local) in function.locals.iter_mut().enumerate() {
        let new = LocalId::new(index as u32);
        locals[local.id.index() as usize] = Some(new);
        local.id = new;
    }
    let mut remap_local = |id: &mut LocalId| {
        *id = locals[id.index() as usize].expect("retained local reference was marked used");
    };
    let mut remap_floor = |floor: &mut u32| {
        if (*floor as usize) <= locals.len() {
            *floor = locals[..*floor as usize]
                .iter()
                .filter(|id| id.is_some())
                .count() as u32;
        }
    };
    for parameter in &mut function.params {
        remap_local(&mut parameter.local);
    }
    for local in &mut function.locals {
        if let Some(source) = &mut local.view_source {
            remap_local(source);
        }
    }
    for block in &mut function.blocks {
        block_locals(block, &mut remap_local, &mut remap_floor);
    }
}

fn block_targets(kind: &mut TerminatorKind, visit: &impl Fn(&mut BlockId)) {
    match kind {
        TerminatorKind::Goto(target) => visit(target),
        TerminatorKind::Branch {
            then_block,
            else_block,
            ..
        } => {
            visit(then_block);
            visit(else_block);
        }
        TerminatorKind::Switch {
            variants,
            otherwise,
            ..
        } => {
            for (_, target, _) in variants {
                visit(target);
            }
            if let Some(target) = otherwise {
                visit(target);
            }
        }
        TerminatorKind::ForEach { body, exit, .. } => {
            visit(body);
            visit(exit);
        }
        TerminatorKind::ReflectedTypeDispatch {
            arms, otherwise, ..
        } => {
            for arm in arms {
                visit(&mut arm.target);
            }
            visit(otherwise);
        }
        TerminatorKind::Return(_) | TerminatorKind::Respond(_) | TerminatorKind::Unreachable => {}
    }
}

fn source_local(source: &mut SequenceSource, visit: &mut impl FnMut(&mut LocalId)) {
    match source {
        SequenceSource::Local(local) | SequenceSource::Projected { owner: local, .. } => {
            visit(local)
        }
    }
}

fn block_locals(
    block: &mut BasicBlock,
    visit: &mut impl FnMut(&mut LocalId),
    floor: &mut impl FnMut(&mut u32),
) {
    for statement in &mut block.statements {
        match &mut statement.kind {
            StatementKind::SequenceLength { source, target } => {
                source_local(source, visit);
                visit(target);
            }
            StatementKind::SequenceGet {
                source,
                target,
                index,
                ..
            } => {
                source_local(source, visit);
                visit(target);
                visit(index);
            }
            StatementKind::IterationBorrow { source, token, .. } => {
                source_local(source, visit);
                visit(token);
            }
            StatementKind::SumTag { source, target }
            | StatementKind::SumTake { source, target, .. } => {
                visit(source);
                visit(target);
            }
            StatementKind::Let { local, value }
            | StatementKind::CheckRefinement {
                local, call: value, ..
            } => {
                visit(local);
                expression(value, visit, floor);
            }
            StatementKind::Assign { target, value } => {
                expression(target, visit, floor);
                expression(value, visit, floor);
            }
            StatementKind::Evaluate(value) | StatementKind::HandleDefault(value) => {
                expression(value, visit, floor)
            }
            StatementKind::Assert { condition, message } => {
                expression(condition, visit, floor);
                if let Some(message) = message {
                    expression(message, visit, floor);
                }
            }
            StatementKind::Trace(local) => visit(local),
            StatementKind::Breakpoint {
                condition,
                bindings,
            } => {
                if let Some(condition) = condition {
                    expression(condition, visit, floor);
                }
                for binding in bindings {
                    visit(binding);
                }
            }
        }
    }
    match &mut block.terminator.kind {
        TerminatorKind::Return(value) => {
            if let Some(value) = value {
                expression(value, visit, floor);
            }
        }
        TerminatorKind::Respond(value)
        | TerminatorKind::Branch {
            condition: value, ..
        } => expression(value, visit, floor),
        TerminatorKind::Switch {
            scrutinee,
            variants,
            ..
        } => {
            expression(scrutinee, visit, floor);
            for (_, _, bindings) in variants {
                for binding in bindings {
                    visit(binding);
                }
            }
        }
        TerminatorKind::ForEach {
            key,
            value,
            iterable,
            ..
        } => {
            visit(key);
            if let Some(value) = value {
                visit(value);
            }
            expression(iterable, visit, floor);
        }
        TerminatorKind::ReflectedTypeDispatch { type_info, .. } => {
            expression(type_info, visit, floor)
        }
        TerminatorKind::Goto(_) | TerminatorKind::Unreachable => {}
    }
}

fn expression(
    value: &mut Expression,
    visit: &mut impl FnMut(&mut LocalId),
    floor: &mut impl FnMut(&mut u32),
) {
    match &mut value.kind {
        ExpressionKind::Local(local) => visit(local),
        ExpressionKind::ClosureRef { captures, .. } => {
            for capture in captures {
                visit(capture);
            }
        }
        ExpressionKind::Binary { left, right, .. } => {
            expression(left, visit, floor);
            expression(right, visit, floor);
        }
        ExpressionKind::Unary { value, .. }
        | ExpressionKind::ResultOk(value)
        | ExpressionKind::ResultFail(value)
        | ExpressionKind::OptionalSome(value)
        | ExpressionKind::Declassify(value)
        | ExpressionKind::Coarsen(value)
        | ExpressionKind::RefinementValidated(value)
        | ExpressionKind::DisplayResult(value)
        | ExpressionKind::EquatableResult(value)
        | ExpressionKind::InterfaceCoerce { value, .. }
        | ExpressionKind::FunctionAdapter { value, .. }
        | ExpressionKind::InterfaceType(value)
        | ExpressionKind::Comptime { value, .. }
        | ExpressionKind::StateIs { value, .. }
        | ExpressionKind::Run(value)
        | ExpressionKind::Join(value)
        | ExpressionKind::Cancel(value)
        | ExpressionKind::Field { base: value, .. }
        | ExpressionKind::View(value)
        | ExpressionKind::Clone(value) => expression(value, visit, floor),
        ExpressionKind::Call { args, .. }
        | ExpressionKind::Intrinsic { args, .. }
        | ExpressionKind::ActorSpawn { args, .. }
        | ExpressionKind::StructConstruct { fields: args, .. }
        | ExpressionKind::BitfieldConstruct { fields: args, .. }
        | ExpressionKind::MachineConstruct { payloads: args, .. }
        | ExpressionKind::EnumConstruct { payloads: args, .. }
        | ExpressionKind::ListConstruct { elements: args } => {
            for arg in args {
                expression(arg, visit, floor);
            }
        }
        ExpressionKind::IndirectCall { callee, args, .. }
        | ExpressionKind::ActorMessage {
            actor: callee,
            args,
            ..
        }
        | ExpressionKind::MachineTransition {
            source: callee,
            payloads: args,
            ..
        } => {
            expression(callee, visit, floor);
            for arg in args {
                expression(arg, visit, floor);
            }
        }
        ExpressionKind::MapConstruct { entries } => {
            for entry in entries {
                expression(&mut entry.key, visit, floor);
                expression(&mut entry.value, visit, floor);
            }
        }
        ExpressionKind::Handle {
            target,
            error_local,
            failure,
            ..
        } => {
            expression(target, visit, floor);
            if let Some(local) = error_local {
                visit(local);
            }
            hir_block(failure, visit, floor);
        }
        ExpressionKind::StringInterpolation(parts) => {
            for part in parts {
                if let hir::StringSegment::Value(value) = part {
                    expression(value, visit, floor);
                }
            }
        }
        ExpressionKind::InlineFunction {
            params,
            view_params,
            local_floor,
            body,
            ..
        } => {
            for local in params.iter_mut().chain(view_params) {
                visit(local);
            }
            floor(local_floor);
            hir_block(body, visit, floor);
        }
        ExpressionKind::Int(_)
        | ExpressionKind::Float(_)
        | ExpressionKind::String(_)
        | ExpressionKind::Bool(_)
        | ExpressionKind::Nothing
        | ExpressionKind::Constant { .. }
        | ExpressionKind::FunctionRef(_)
        | ExpressionKind::OptionalNone
        | ExpressionKind::RuntimeFailure(_)
        | ExpressionKind::PropertyCaseContext(_) => {}
    }
}

fn hir_block(
    block: &mut hir::Block,
    visit: &mut impl FnMut(&mut LocalId),
    floor: &mut impl FnMut(&mut u32),
) {
    for statement in &mut block.statements {
        match &mut statement.kind {
            hir::StatementKind::Let { local, value } => {
                visit(local);
                expression(value, visit, floor);
            }
            hir::StatementKind::Assign { target, value } => {
                expression(target, visit, floor);
                expression(value, visit, floor);
            }
            hir::StatementKind::Return(value) => {
                if let Some(value) = value {
                    expression(value, visit, floor);
                }
            }
            hir::StatementKind::HandleDefault(value)
            | hir::StatementKind::Expression(value)
            | hir::StatementKind::Respond(value) => expression(value, visit, floor),
            hir::StatementKind::If {
                condition,
                then_block,
                else_block,
            } => {
                expression(condition, visit, floor);
                hir_block(then_block, visit, floor);
                if let Some(block) = else_block {
                    hir_block(block, visit, floor);
                }
            }
            hir::StatementKind::While { condition, body } => {
                expression(condition, visit, floor);
                hir_block(body, visit, floor);
            }
            hir::StatementKind::For {
                key,
                value,
                iterable,
                body,
                ..
            } => {
                visit(key);
                if let Some(value) = value {
                    visit(value);
                }
                expression(iterable, visit, floor);
                hir_block(body, visit, floor);
            }
            hir::StatementKind::Match { scrutinee, arms } => {
                expression(scrutinee, visit, floor);
                for arm in arms {
                    for binding in &mut arm.bindings {
                        visit(binding);
                    }
                    hir_block(&mut arm.body, visit, floor);
                }
            }
            hir::StatementKind::Assert { condition, message } => {
                expression(condition, visit, floor);
                if let Some(value) = message {
                    expression(value, visit, floor);
                }
            }
            hir::StatementKind::Trace(local) => visit(local),
            hir::StatementKind::Breakpoint {
                condition,
                bindings,
            } => {
                if let Some(value) = condition {
                    expression(value, visit, floor);
                }
                for binding in bindings {
                    visit(binding);
                }
            }
            hir::StatementKind::Scope(block) => hir_block(block, visit, floor),
            hir::StatementKind::ReflectedTypeDispatch { type_info, arms } => {
                expression(type_info, visit, floor);
                for arm in arms {
                    hir_block(&mut arm.body, visit, floor);
                }
            }
            hir::StatementKind::Break | hir::StatementKind::Continue => {}
        }
    }
}
