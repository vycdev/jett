//! Ownership planning for the copyable scalar/string MIR subset.
//! This deliberately rejects unextracted handlers rather than walking hidden
//! control flow. Resources and other move-only values need a distinct
//! move/borrow/drop analysis before their backend support can be enabled.
use crate::{ControlFlowGraph, Function, ResourceCallNode, StatementKind, TerminatorKind};
use jett_hir::{Expression, ExpressionKind, IntrinsicId, StringSegment};
use jett_types::{Type, TypeId, TypeInterner};
use std::collections::BTreeSet;
type Set = BTreeSet<usize>;

#[derive(Clone, Copy)]
struct PlanningContext<'a> {
    program: Option<&'a crate::Program>,
    companion: Option<&'a crate::resource_ownership::CompanionContext<'a>>,
}
impl<'a> PlanningContext<'a> {
    fn is_some(self) -> bool {
        self.program.is_some()
    }
    fn is_none(self) -> bool {
        self.program.is_none()
    }
    fn is_some_and(self, f: impl FnOnce(&'a crate::Program) -> bool) -> bool {
        self.program.is_some_and(f)
    }
    fn and_then<T>(self, f: impl FnOnce(&'a crate::Program) -> Option<T>) -> Option<T> {
        self.program.and_then(f)
    }
    fn resource_type(self, ty: TypeId) -> bool {
        self.companion
            .is_some_and(|context| context.resource_type(ty))
    }
    fn resource_expression(self, value: &Expression) -> bool {
        self.companion
            .is_some_and(|context| context.resource_expression(value))
    }
    fn resource_local(self, local: &jett_hir::Local) -> bool {
        self.companion
            .is_some_and(|context| context.resource_local(local))
    }
}

#[derive(Debug)]
pub struct CopyValuePlan {
    pub owned_locals: Vec<usize>,
    pub live_in: Vec<Set>,
    pub live_out: Vec<Set>,
    pub live_after_statement: Vec<Vec<Set>>,
    /// Bound on owned intermediates in one full expression, including formatting.
    pub temporary_slots: usize,
    generation_storage: crate::CallGenerationStoragePlan,
}
impl CopyValuePlan {
    pub fn generation_storage(&self) -> &crate::CallGenerationStoragePlan {
        &self.generation_storage
    }
    pub fn analyze(function: &Function, types: &TypeInterner) -> Result<Self, String> {
        Self::analyze_storage(function, types, None)
    }
    pub(crate) fn analyze_storage(
        function: &Function,
        types: &TypeInterner,
        program: Option<&crate::Program>,
    ) -> Result<Self, String> {
        Self::analyze_with_context(
            function,
            types,
            PlanningContext {
                program,
                companion: None,
            },
        )
    }
    pub(crate) fn analyze_companion(
        context: &crate::resource_ownership::CompanionContext<'_>,
    ) -> Result<Self, String> {
        Self::analyze_with_context(
            context.function,
            context.types,
            PlanningContext {
                program: Some(context.program),
                companion: Some(context),
            },
        )
    }
    fn analyze_with_context(
        function: &Function,
        types: &TypeInterner,
        program: PlanningContext<'_>,
    ) -> Result<Self, String> {
        crate::ordinary_borrowed_sums::validate(function, types)?;
        if program.companion.is_none()
            && (crate::resource_type_pending(types, function.return_type)
                || function
                    .locals
                    .iter()
                    .any(|local| crate::resource_type_pending(types, local.ty)))
        {
            return Err("pending ResourceOwnershipPlan: ordinary CopyValuePlan cannot plan Resource carriers or descriptors".into());
        }
        let generation_storage = crate::call_owner_generations::validate(function, types)?;
        if program.is_none() && !generation_storage.slots().is_empty() {
            return Err("generation storage requires complete program ownership validation".into());
        }
        plan_type(types, function.return_type, program)?;
        for local in &function.locals {
            plan_type(types, local.ty, program)?;
            if function.view_root(local.id).is_none() {
                return Err("borrowed local origin is invalid or cyclic".into());
            }
        }
        let cfg = ControlFlowGraph::analyze(function).map_err(|e| format!("{e:?}"))?;
        let n = function.blocks.len();
        let mut facts = Vec::new();
        let mut max_temporaries = 0;
        for block in &function.blocks {
            let mut statements = Vec::new();
            for (index, statement) in block.statements.iter().enumerate() {
                let mut reads = Set::new();
                let mut temporaries = 0;
                let mut killed = None;
                let definition = match &statement.kind {
                    StatementKind::OpenCallOwnerGeneration { .. }
                    | StatementKind::CloseCallOwnerGeneration { .. } => None,
                    StatementKind::ReplaceCallOwnerGeneration {
                        root, rhs_owner, ..
                    } => {
                        reads.insert(root.index() as usize);
                        reads.insert(rhs_owner.index() as usize);
                        killed = Some(rhs_owner.index() as usize);
                        Some(root.index() as usize)
                    }
                    StatementKind::ReflectedContainerReady { source, .. } => {
                        reads.insert(source.index() as usize);
                        None
                    }
                    StatementKind::IterationBorrow { source, .. } => {
                        reads.insert(source.root().index() as usize);
                        None
                    }
                    StatementKind::SequenceLength { source, target }
                    | StatementKind::SequenceGet { source, target, .. } => {
                        reads.insert(source.root().index() as usize);
                        if let StatementKind::SequenceGet { index, .. } = statement.kind {
                            reads.insert(index.index() as usize);
                            temporaries += usize::from(
                                crate::move_values::is_copy_owned(
                                    types,
                                    function.locals[target.index() as usize].ty,
                                ) || crate::move_values::is_linear(
                                    types,
                                    function.locals[target.index() as usize].ty,
                                ),
                            );
                        }
                        Some(target.index() as usize)
                    }
                    StatementKind::SumTag { source, target }
                    | StatementKind::SumTake { source, target, .. }
                        if program.is_some() =>
                    {
                        reads.insert(source.index() as usize);
                        if matches!(statement.kind, StatementKind::SumTake { .. }) {
                            let ty = function.locals[target.index() as usize].ty;
                            let borrowed = matches!(
                                statement.kind,
                                StatementKind::SumTake { success: true, .. }
                            ) && function
                                .ordinary_borrowed_sum_projection(block.id, index)?
                                .is_some();
                            if !borrowed {
                                temporaries += usize::from(
                                    crate::move_values::is_copy_owned(types, ty)
                                        || crate::move_values::is_linear(types, ty),
                                );
                            }
                        }
                        Some(target.index() as usize)
                    }
                    StatementKind::ResourceCall(ResourceCallNode::Stage {
                        value,
                        ordinary: Some(local),
                        ..
                    })
                    | StatementKind::Let { local, value }
                    | StatementKind::BeginCallView { local, value } => {
                        let borrowed = function
                            .local(*local)
                            .ok_or("local definition is outside its function")?
                            .view_source
                            .is_some();
                        visit(
                            value,
                            &mut reads,
                            &mut temporaries,
                            types,
                            program,
                            borrowed,
                        )?;
                        Some(local.index() as usize)
                    }
                    StatementKind::ResourceCall(ResourceCallNode::Stage {
                        value,
                        ordinary: None,
                        ..
                    }) => {
                        if program.is_none() {
                            return Err(
                                "Resource call stage requires its fresh companion plan".into()
                            );
                        }
                        visit(value, &mut reads, &mut temporaries, types, program, false)?;
                        None
                    }
                    StatementKind::ResourceCall(ResourceCallNode::Invoke { region, output }) => {
                        let record = function
                            .resource_call_region(*region)
                            .ok_or("Resource call output lacks its sealed region")?;
                        for actual in record.actuals() {
                            if let Some(local) = actual.ordinary() {
                                reads.insert(local.index() as usize);
                            }
                        }
                        let ty = function.locals[output.index() as usize].ty;
                        temporaries += usize::from(
                            crate::move_values::is_copy_owned(types, ty)
                                || crate::move_values::is_linear(types, ty),
                        );
                        Some(output.index() as usize)
                    }
                    StatementKind::ResourceCall(
                        ResourceCallNode::Begin { .. } | ResourceCallNode::End { .. },
                    ) => {
                        if program.is_none() {
                            return Err(
                                "Resource call region requires its fresh companion plan".into()
                            );
                        }
                        None
                    }
                    StatementKind::ResourceLexicalExit(_) => {
                        if program.is_none() {
                            return Err(
                                "Resource lexical exit requires its fresh companion plan".into()
                            );
                        }
                        None
                    }
                    StatementKind::EndCallView { local } => {
                        reads.insert(local.index() as usize);
                        killed = Some(local.index() as usize);
                        None
                    }
                    StatementKind::CheckRefinement { local, call, .. } => {
                        visit(call, &mut reads, &mut temporaries, types, program, false)?;
                        Some(local.index() as usize)
                    }
                    StatementKind::Assign { target, value } => {
                        let ExpressionKind::Local(local) = target.kind else {
                            return Err("nonlocal assignment needs place ownership".into());
                        };
                        visit(value, &mut reads, &mut temporaries, types, program, false)?;
                        Some(local.index() as usize)
                    }
                    StatementKind::Evaluate(value) => {
                        visit(value, &mut reads, &mut temporaries, types, program, false)?;
                        None
                    }
                    StatementKind::Assert { condition, message } => {
                        visit(
                            condition,
                            &mut reads,
                            &mut temporaries,
                            types,
                            program,
                            false,
                        )?;
                        if let Some(message) = message {
                            visit(message, &mut reads, &mut temporaries, types, program, false)?;
                        }
                        None
                    }
                    StatementKind::Trace(local) => {
                        reads.insert(local.index() as usize);
                        temporaries += 1;
                        None
                    }
                    StatementKind::Breakpoint {
                        condition,
                        bindings,
                    } => {
                        if let Some(condition) = condition {
                            visit(
                                condition,
                                &mut reads,
                                &mut temporaries,
                                types,
                                program,
                                false,
                            )?;
                        }
                        reads.extend(bindings.iter().map(|local| local.index() as usize));
                        temporaries += usize::from(!bindings.is_empty());
                        None
                    }
                    _ => return Err("statement needs explicit ownership lowering".into()),
                };
                expand_view_reads(function, &mut reads)?;
                max_temporaries = max_temporaries.max(temporaries);
                statements.push((reads, definition, killed));
            }
            let mut reads = Set::new();
            let mut temporaries = 0;
            match &block.terminator.kind {
                TerminatorKind::Return(Some(v))
                | TerminatorKind::Respond(v)
                | TerminatorKind::Branch { condition: v, .. } => {
                    visit(v, &mut reads, &mut temporaries, types, program, false)?
                }
                TerminatorKind::ReflectedTypeDispatch { type_info, .. } if program.is_some() => {
                    visit(
                        type_info,
                        &mut reads,
                        &mut temporaries,
                        types,
                        program,
                        true,
                    )?;
                }
                TerminatorKind::Switch {
                    scrutinee,
                    variants,
                    ..
                } if program.is_some() => {
                    visit(
                        scrutinee,
                        &mut reads,
                        &mut temporaries,
                        types,
                        program,
                        false,
                    )?;
                    temporaries += variants
                        .iter()
                        .map(|(_, _, bindings)| {
                            bindings
                                .iter()
                                .filter(|binding| {
                                    let ty = function.locals[binding.index() as usize].ty;
                                    crate::move_values::is_copy_owned(types, ty)
                                        || crate::move_values::is_linear(types, ty)
                                })
                                .count()
                        })
                        .max()
                        .unwrap_or(0);
                }
                TerminatorKind::Return(None)
                | TerminatorKind::Goto(_)
                | TerminatorKind::Unreachable => {}
                _ => return Err("terminator needs explicit ownership lowering".into()),
            }
            expand_view_reads(function, &mut reads)?;
            max_temporaries = max_temporaries.max(temporaries);
            facts.push((statements, reads));
        }
        // Definite initialization is an intersection fixed point, including
        // backedges. Reading a zero-initialized native slot is not a substitute.
        let all: Set = (0..function.locals.len()).collect();
        let params: Set = function
            .params
            .iter()
            .map(|p| p.local.index() as usize)
            .collect();
        let mut initialized_in = vec![all.clone(); n];
        let mut initialized_out = vec![all; n];
        loop {
            let mut changed = false;
            for &id in cfg.reverse_postorder() {
                let i = id.index() as usize;
                let incoming = if id == function.entry {
                    params.clone()
                } else {
                    let mut incoming = (0..function.locals.len()).collect::<Set>();
                    for pred in cfg
                        .predecessors(id)
                        .iter()
                        .filter(|p| cfg.reverse_postorder().contains(p))
                    {
                        let edge = switch_bindings_on_edge(function, *pred, id);
                        let produced = initialized_out[pred.index() as usize]
                            .union(&edge)
                            .copied()
                            .collect::<Set>();
                        incoming = incoming.intersection(&produced).copied().collect();
                    }
                    incoming
                };
                let mut outgoing = incoming.clone();
                for (_, definition, killed) in &facts[i].0 {
                    if let Some(local) = killed {
                        outgoing.remove(local);
                    }
                    if let Some(d) = definition {
                        outgoing.insert(*d);
                    }
                }
                changed |= incoming != initialized_in[i] || outgoing != initialized_out[i];
                initialized_in[i] = incoming;
                initialized_out[i] = outgoing;
            }
            if !changed {
                break;
            }
        }
        for &id in cfg.reverse_postorder() {
            let i = id.index() as usize;
            let mut initialized = initialized_in[i].clone();
            for (reads, definition, killed) in &facts[i].0 {
                if !reads.is_subset(&initialized) {
                    return Err(format!("read before definite initialization in block {i}"));
                }
                if let Some(local) = killed {
                    initialized.remove(local);
                }
                if let Some(d) = definition {
                    initialized.insert(*d);
                }
            }
            if !facts[i].1.is_subset(&initialized) {
                return Err(format!(
                    "terminator read before definite initialization in block {i}"
                ));
            }
        }
        let mut live_in = vec![Set::new(); n];
        let mut live_out = live_in.clone();
        let mut after = facts
            .iter()
            .map(|(ss, _)| vec![Set::new(); ss.len()])
            .collect::<Vec<_>>();
        loop {
            let mut changed = false;
            for &id in cfg.reverse_postorder().iter().rev() {
                let i = id.index() as usize;
                let out: Set = cfg
                    .successors(id)
                    .iter()
                    .flat_map(|s| {
                        let edge = switch_bindings_on_edge(function, id, *s);
                        live_in[s.index() as usize]
                            .difference(&edge)
                            .copied()
                            .collect::<Set>()
                    })
                    .collect();
                let mut live = out.union(&facts[i].1).copied().collect::<Set>();
                for (j, (reads, definition, killed)) in facts[i].0.iter().enumerate().rev() {
                    after[i][j] = live.clone();
                    if let Some(local) = killed {
                        live.remove(local);
                    }
                    if let Some(d) = definition {
                        live.remove(d);
                    }
                    live.extend(reads);
                }
                changed |= live != live_in[i] || out != live_out[i];
                live_in[i] = live;
                live_out[i] = out;
            }
            if !changed {
                break;
            }
        }
        Ok(Self {
            owned_locals: function
                .locals
                .iter()
                .filter(|l| {
                    l.view_source.is_none()
                        && !program.resource_local(l)
                        && (crate::move_values::is_copy_owned(types, l.ty)
                            || (program.is_some()
                                && crate::move_values::is_linear(types, l.ty)
                                && !function.is_view_local(l.id)))
                })
                .map(|l| l.id.index() as usize)
                .collect(),
            live_in,
            live_out,
            live_after_statement: after,
            temporary_slots: max_temporaries,
            generation_storage,
        })
    }
}

/// Reading a view also observes every alias and owner backing its value. Expand
/// before both initialization and liveness so cleanup cannot drop that owner.
fn expand_view_reads(function: &Function, reads: &mut Set) -> Result<(), String> {
    for index in reads.clone() {
        let mut local = function
            .locals
            .get(index)
            .ok_or("native read is outside its local table")?;
        if function.view_root(local.id).is_none() {
            return Err("borrowed local origin is invalid or cyclic".into());
        }
        while let Some(source) = local.view_source {
            reads.insert(source.index() as usize);
            local = function
                .local(source)
                .ok_or("borrowed local origin is outside its function")?;
        }
    }
    Ok(())
}

pub(crate) fn switch_bindings_on_edge(
    function: &Function,
    source: crate::BlockId,
    target: crate::BlockId,
) -> Set {
    let TerminatorKind::Switch {
        variants,
        otherwise,
        ..
    } = &function.blocks[source.index() as usize].terminator.kind
    else {
        return Set::new();
    };
    let mut candidates = variants
        .iter()
        .filter(|(_, block, _)| *block == target)
        .map(|(_, _, bindings)| {
            bindings
                .iter()
                .map(|local| local.index() as usize)
                .collect::<Set>()
        })
        .collect::<Vec<_>>();
    if *otherwise == Some(target) {
        candidates.push(Set::new());
    }
    let Some(mut common) = candidates.pop() else {
        return Set::new();
    };
    for candidate in candidates {
        common = common.intersection(&candidate).copied().collect();
    }
    common
}
fn visit(
    value: &Expression,
    reads: &mut Set,
    temporaries: &mut usize,
    types: &TypeInterner,
    program: PlanningContext<'_>,
    borrowed: bool,
) -> Result<(), String> {
    if let Some(context) = program.companion {
        if !context.contains(value) {
            return Err(
                "Resource companion expression is outside its exact current function".into(),
            );
        }
    }
    plan_type(types, value.ty, program)?;
    if program.is_some()
        && !program.resource_expression(value)
        && crate::move_values::is_linear(types, value.ty)
    {
        *temporaries += usize::from(match &value.kind {
            ExpressionKind::Local(_) => !borrowed,
            ExpressionKind::Call { .. }
            | ExpressionKind::IndirectCall { .. }
            | ExpressionKind::Intrinsic { .. }
            | ExpressionKind::Clone(_)
            | ExpressionKind::ResultOk(_)
            | ExpressionKind::ResultFail(_)
            | ExpressionKind::OptionalSome(_)
            | ExpressionKind::OptionalNone
            | ExpressionKind::Join(_)
            | ExpressionKind::ListConstruct { .. }
            | ExpressionKind::MapConstruct { .. }
            | ExpressionKind::StructConstruct { .. }
            | ExpressionKind::EnumConstruct { .. } => true,
            ExpressionKind::Field { .. } => !borrowed,
            ExpressionKind::MachineConstruct { .. } | ExpressionKind::MachineTransition { .. } => {
                true
            }
            ExpressionKind::BitfieldConstruct { .. } => true,
            ExpressionKind::Run(inner) => {
                matches!(
                    types.resolve(crate::move_values::representation_type(types, inner.ty)),
                    Type::Bytes
                        | Type::List(_)
                        | Type::Set(_)
                        | Type::Map(..)
                        | Type::Optional(_)
                        | Type::Result(..)
                        | Type::Struct(_)
                        | Type::Enum(_)
                        | Type::Bitfield(_)
                        | Type::Machine(_)
                        | Type::MachineState { .. }
                        | Type::Function { .. }
                        | Type::Interface(_)
                        | Type::TypeConstruction
                )
            }
            _ => false,
        });
    }
    // Count owning emitter operations, not copy-owned AST nodes. Children
    // accumulate until full-expression cleanup; even short-circuit alternatives
    // receive distinct slots during emission. View/clone add no ownership.
    match &value.kind {
        ExpressionKind::FunctionAdapter { .. } | ExpressionKind::InterfaceCoerce { .. } => {
            *temporaries += 3;
        }
        ExpressionKind::RuntimeFailure(_) | ExpressionKind::PropertyCaseContext(Some(_)) => {
            *temporaries += 1;
        }
        ExpressionKind::String(_)
        | ExpressionKind::Local(_)
        | ExpressionKind::Call { .. }
        | ExpressionKind::IndirectCall { .. }
        | ExpressionKind::Field { .. }
            if !program.resource_expression(value)
                && crate::move_values::is_copy_owned(types, value.ty) =>
        {
            *temporaries += 1
        }
        ExpressionKind::Intrinsic {
            intrinsic: id,
            args,
            ..
        } => {
            if *id == IntrinsicId::GraphicsRun {
                // The generated session loop stages callback results, scene and
                // domain-result sums across its branches and backedge.
                *temporaries += 16;
            } else if matches!(id, IntrinsicId::Print | IntrinsicId::Println) {
                // Empty output, argument concatenations, inter-argument spaces
                // (literal + concat), and the optional newline (literal + concat).
                *temporaries += 1 + args.len() + 2 * args.len().saturating_sub(1);
                *temporaries += usize::from(*id == IntrinsicId::Println) * 2;
                *temporaries += args.len();
            } else if crate::move_values::is_copy_owned(types, value.ty) {
                // Copy-owned leaves and scalar conversion each own once.
                *temporaries += 1;
            }
        }
        ExpressionKind::StringInterpolation(segments) => {
            // Initial empty literal plus one concatenation per segment.
            *temporaries += 1 + segments.len();
        }
        ExpressionKind::Run(value)
            if matches!(
                types.resolve(crate::move_values::representation_type(types, value.ty)),
                Type::String
            ) =>
        {
            *temporaries += 1;
        }
        ExpressionKind::Join(value)
            if matches!(
                types.resolve(crate::move_values::representation_type(types, value.ty)),
                Type::String
                    | Type::Bytes
                    | Type::List(_)
                    | Type::Set(_)
                    | Type::Map(..)
                    | Type::Optional(_)
                    | Type::Result(..)
                    | Type::Struct(_)
                    | Type::Enum(_)
                    | Type::Bitfield(_)
                    | Type::Machine(_)
                    | Type::MachineState { .. }
                    | Type::Function { .. }
                    | Type::Interface(_)
                    | Type::TypeConstruction
            ) =>
        {
            // Joining a pending owned value owns its extracted value before the
            // result sum takes it.
            *temporaries += 1;
        }
        ExpressionKind::FunctionRef(_) => {
            // The descriptor and its source display label are owned temporaries.
            *temporaries += 2;
        }
        ExpressionKind::ClosureRef { function, captures } => {
            // The descriptor, environment, and source label are owned temporaries.
            *temporaries += 3;
            let closure = program
                .and_then(|program| program.functions.get(function.index() as usize))
                .ok_or("closure capture needs its checked function")?;
            if closure.capture_count != captures.len() {
                return Err("closure capture count disagrees with its function".into());
            }
            for param in closure.params.iter().take(closure.capture_count) {
                *temporaries += usize::from(crate::move_values::is_copy_owned(types, param.ty));
            }
        }
        _ => {}
    }
    match &value.kind {
        ExpressionKind::ResourceHookValue { .. } if program.companion.is_some() => {},
        ExpressionKind::ResourceInvoke { hook, args, evaluation_order, .. } if program.companion.is_some() => {
            let Type::Function { view_params, .. } = types.resolve(hook.function_type()) else { return Err("Resource hook has no exact function signature".into()); };
            for &parameter in evaluation_order {
                visit(&args[parameter], reads, temporaries, types, program, view_params[parameter])?;
            }
            // Ordinary hook results still own normal runtime temporaries.
            *temporaries += usize::from(!program.resource_expression(value) && (crate::move_values::is_copy_owned(types, value.ty) || crate::move_values::is_linear(types, value.ty)));
        }
        ExpressionKind::ResourceInvoke { .. } | ExpressionKind::ResourceHookValue { .. } => return Err("pending ResourceOwnershipPlan: ordinary expression liveness cannot own Resource operation or descriptor temporaries".into()),
        ExpressionKind::Local(l) => {
            reads.insert(l.index() as usize);
            if program.is_some() && !program.resource_expression(value) && !borrowed && crate::move_values::is_linear(types, value.ty) {
                // Test predicates clone local owners on each ordinary read;
                // reserving this slot for all native functions is conservative.
                *temporaries += 1;
            }
        }
        ExpressionKind::ClosureRef { captures, .. } => {
            reads.extend(captures.iter().map(|local| local.index() as usize));
        }
        ExpressionKind::Int(_)
        | ExpressionKind::Float(_)
        | ExpressionKind::Bool(_)
        | ExpressionKind::String(_)
        | ExpressionKind::FunctionRef(_)
        | ExpressionKind::Nothing
        | ExpressionKind::PropertyCaseContext(_)
        | ExpressionKind::RuntimeFailure(_) => {}
        ExpressionKind::Binary { left, op, right } => {
            let enum_equality =
                matches!(op, jett_hir::BinaryOp::Equal | jett_hir::BinaryOp::NotEqual)
                    && matches!(
                        types.resolve(crate::move_values::representation_type(types, left.ty)),
                        Type::Enum(_)
                    );
            visit(left, reads, temporaries, types, program, enum_equality)?;
            visit(right, reads, temporaries, types, program, enum_equality)?;
            // A custom payload method needs one owned traversal cursor. This
            // is a conservative bound; pure primitive graphs need no cursor.
            *temporaries += usize::from(
                enum_equality
                    && program.is_some_and(|program| !program.equality_methods.is_empty()),
            );
        }
        ExpressionKind::OptionalNone if program.is_some() => {}
        ExpressionKind::StructConstruct {
            fields,
            validates_refinements,
            ..
        } if program.is_some() => {
            // A validated construction owns the record before wrapping it in
            // a separately owned success sum. The expression type counts the
            // sum above; reserve one more slot for the intermediate record.
            *temporaries += usize::from(*validates_refinements);
            for field in fields {
                visit(field, reads, temporaries, types, program, false)?;
            }
        }
        ExpressionKind::EnumConstruct {
            payloads,
            evaluation_order,
            ..
        } if program.is_some() => {
            for &index in evaluation_order {
                visit(&payloads[index], reads, temporaries, types, program, false)?;
            }
        }
        ExpressionKind::MachineConstruct { payloads, .. } if program.is_some() => {
            for payload in payloads {
                visit(payload, reads, temporaries, types, program, false)?;
            }
        }
        ExpressionKind::MachineTransition {
            source, payloads, ..
        } if program.is_some() => {
            visit(source, reads, temporaries, types, program, false)?;
            for payload in payloads {
                visit(payload, reads, temporaries, types, program, false)?;
            }
        }
        ExpressionKind::StateIs { value, .. } if program.is_some() => {
            visit(value, reads, temporaries, types, program, true)?;
        }
        ExpressionKind::BitfieldConstruct { fields, .. } if program.is_some() => {
            for field in fields {
                visit(field, reads, temporaries, types, program, false)?;
            }
        }
        ExpressionKind::Field { base, .. } if program.is_some() => {
            visit(base, reads, temporaries, types, program, true)?;
        }
        ExpressionKind::ListConstruct { elements } if program.is_some() => {
            for element in elements {
                visit(element, reads, temporaries, types, program, false)?;
            }
        }
        ExpressionKind::MapConstruct { entries } if program.is_some() => {
            for entry in entries {
                visit(&entry.key, reads, temporaries, types, program, false)?;
                visit(&entry.value, reads, temporaries, types, program, false)?;
            }
        }
        ExpressionKind::ResultOk(value)
        | ExpressionKind::ResultFail(value)
        | ExpressionKind::OptionalSome(value)
            if program.is_some() =>
        {
            visit(value, reads, temporaries, types, program, false)?
        }
        ExpressionKind::Declassify(value)
        | ExpressionKind::Coarsen(value)
        | ExpressionKind::RefinementValidated(value) => {
            visit(value, reads, temporaries, types, program, borrowed)?
        }
        ExpressionKind::FunctionAdapter { value, .. }
        | ExpressionKind::DisplayResult(value)
        | ExpressionKind::EquatableResult(value)
        | ExpressionKind::InterfaceCoerce { value, .. }
        | ExpressionKind::InterfaceType(value)
        | ExpressionKind::Run(value)
        | ExpressionKind::Join(value)
        | ExpressionKind::Cancel(value)
        | ExpressionKind::Unary { value, .. } => {
            visit(value, reads, temporaries, types, program, false)?
        }
        ExpressionKind::View(value)
        | ExpressionKind::Clone(value)
        | ExpressionKind::RuntimeFailureMessage(value) => {
            visit(value, reads, temporaries, types, program, true)?
        }
        ExpressionKind::Call { function, args, .. } => {
            for (index, v) in args.iter().enumerate() {
                let borrowed = program.is_some_and(|p| {
                    p.functions[function.index() as usize].params[index].mode
                        == crate::ParamMode::View
                });
                visit(v, reads, temporaries, types, program, borrowed)?;
                if !borrowed
                    && matches!(v.kind, ExpressionKind::View(_))
                    && crate::move_values::is_linear(types, v.ty)
                {
                    *temporaries += 1;
                }
            }
        }
        ExpressionKind::ActorSpawn {
            args,
            evaluation_order,
            constructor,
            ..
        } => {
            if constructor.is_none() && program.is_some() {
                // Allocation owns a record before transferring it to the actor registry.
                *temporaries += 1;
            }
            for &index in evaluation_order {
                let argument = &args[index];
                let borrowed = constructor.is_some_and(|id| {
                    program.is_some_and(|program| {
                        program.functions[id.index() as usize].params[index].mode
                            == crate::ParamMode::View
                    })
                });
                visit(argument, reads, temporaries, types, program, borrowed)?;
                if !borrowed
                    && matches!(argument.kind, ExpressionKind::View(_))
                    && crate::move_values::is_linear(types, argument.ty)
                {
                    *temporaries += 1;
                }
            }
        }
        ExpressionKind::ActorMessage {
            actor,
            handler,
            args,
            evaluation_order,
            ..
        } => {
            visit(actor, reads, temporaries, types, program, false)?;
            let target = program
                .and_then(|program| program.functions.get(handler.index() as usize))
                .ok_or("actor message needs its checked handler")?;
            if crate::move_values::is_copy_owned(types, target.return_type)
                || crate::move_values::is_linear(types, target.return_type)
            {
                // A send discards a response only after the call has owned it.
                *temporaries += 1;
            }
            for &index in evaluation_order {
                let argument = &args[index];
                let parameter = target
                    .params
                    .get(target.capture_count + index)
                    .ok_or("actor message argument is absent from its handler")?;
                let borrowed = parameter.mode == crate::ParamMode::View;
                visit(argument, reads, temporaries, types, program, borrowed)?;
                if !borrowed
                    && matches!(argument.kind, ExpressionKind::View(_))
                    && crate::move_values::is_linear(types, argument.ty)
                {
                    *temporaries += 1;
                }
            }
        }
        ExpressionKind::IndirectCall { callee, args, .. } => {
            visit(callee, reads, temporaries, types, program, true)?;
            let view_params = match types
                .resolve(crate::move_values::representation_type(types, callee.ty))
            {
                Type::Function { view_params, .. } if view_params.len() == args.len() => {
                    view_params
                }
                _ => return Err("indirect call has no matching function parameter modes".into()),
            };
            for (argument, borrowed) in args.iter().zip(view_params) {
                visit(argument, reads, temporaries, types, program, *borrowed)?;
                if !*borrowed
                    && matches!(argument.kind, ExpressionKind::View(_))
                    && crate::move_values::is_linear(types, argument.ty)
                {
                    *temporaries += 1;
                }
            }
        }
        ExpressionKind::Intrinsic {
            intrinsic, args, ..
        } => {
            for (index, v) in args.iter().enumerate() {
                visit(
                    v,
                    reads,
                    temporaries,
                    types,
                    program,
                    crate::move_values::intrinsic_borrows(*intrinsic, index, v),
                )?;
            }
        }
        ExpressionKind::StringInterpolation(segments) => {
            for s in segments {
                match s {
                    StringSegment::Value(v) => {
                        visit(v, reads, temporaries, types, program, false)?;
                        // Formatting owns a string even when it retains an
                        // ordinary string or renders a pending one.
                        *temporaries += 1;
                    }
                    StringSegment::Text(_) => *temporaries += 1,
                }
            }
        }
        _ => return Err("expression needs explicit ownership lowering".into()),
    }
    Ok(())
}

fn copy_plan_type(types: &TypeInterner, ty: TypeId) -> Result<(), String> {
    if ty.index() as usize >= types.len() {
        return Err("invalid type in native ownership plan".into());
    }
    if ty == TypeInterner::NEVER {
        // An uninhabited sum arm or empty collection contributes no value to
        // move, borrow, or drop.
        return Ok(());
    }
    if let Type::Secret(inner) | Type::Refinement { base: inner, .. } = types.resolve(ty) {
        return copy_plan_type(types, *inner);
    }
    if let Type::Function {
        params,
        return_type,
        ..
    } = types.resolve(ty)
    {
        for param in params {
            copy_plan_type(types, *param)?;
        }
        return copy_plan_type(types, *return_type);
    }
    if matches!(
        types.resolve(ty),
        Type::Int8
            | Type::Int16
            | Type::Int32
            | Type::Int64
            | Type::Uint8
            | Type::Uint16
            | Type::Uint32
            | Type::Uint64
            | Type::Float32
            | Type::Float64
            | Type::Bool
            | Type::Nothing
            | Type::String
            | Type::Capability(_)
            | Type::Actor(_)
            | Type::Interface(_)
    ) {
        return Ok(());
    }
    Err(format!(
        "type {} requires separate move/borrow/drop analysis",
        types.type_name(ty)
    ))
}

fn plan_type(types: &TypeInterner, ty: TypeId, program: PlanningContext<'_>) -> Result<(), String> {
    plan_type_inner(types, ty, program, &mut BTreeSet::new())
}
fn plan_type_inner(
    types: &TypeInterner,
    ty: TypeId,
    program: PlanningContext<'_>,
    seen: &mut BTreeSet<u32>,
) -> Result<(), String> {
    if ty.index() as usize >= types.len() {
        return Err("invalid native type".into());
    }
    if !seen.insert(ty.index()) {
        return Ok(());
    }
    if program.resource_type(ty) {
        // The fresh custody proof owns only the carrier. Its ordinary companion
        // and signature children keep their existing backend type checks.
        match types.resolve(ty) {
            Type::Resource(_) => return Ok(()),
            Type::Optional(inner) => return plan_type_inner(types, *inner, program, seen),
            Type::Result(ok, failure) => {
                plan_type_inner(types, *ok, program, seen)?;
                return plan_type_inner(types, *failure, program, seen);
            }
            Type::Function {
                params,
                return_type,
                ..
            } => {
                for parameter in params {
                    plan_type_inner(types, *parameter, program, seen)?;
                }
                return plan_type_inner(types, *return_type, program, seen);
            }
            _ => return Err(
                "pending Resource companion: qualified or aggregate custody type is unsupported"
                    .into(),
            ),
        }
    }
    if program.is_some() {
        match types.resolve(ty) {
            Type::Secret(inner) | Type::Refinement { base: inner, .. } => {
                return plan_type_inner(types, *inner, program, seen);
            }
            Type::Function {
                params,
                return_type,
                ..
            } => {
                for param in params {
                    plan_type_inner(types, *param, program, seen)?;
                }
                return plan_type_inner(types, *return_type, program, seen);
            }
            Type::Bytes | Type::TypeConstruction => return Ok(()),
            Type::Struct(id) => {
                for (_, field) in &types.resolve_struct(*id).fields {
                    plan_type_inner(types, *field, program, seen)?;
                }
                return Ok(());
            }
            Type::Enum(id) => {
                for variant in &types.resolve_enum(*id).variants {
                    for (_, field) in &variant.fields {
                        plan_type_inner(types, *field, program, seen)?;
                    }
                }
                return Ok(());
            }
            Type::Machine(id) | Type::MachineState { machine: id, .. } => {
                for state in &types.resolve_machine(*id).states {
                    for (_, field) in &state.fields {
                        plan_type_inner(types, *field, program, seen)?;
                    }
                }
                return Ok(());
            }
            Type::Bitfield(id) => {
                for field in &types.resolve_bitfield(*id).fields {
                    plan_type_inner(types, field.ty, program, seen)?;
                }
                return Ok(());
            }
            Type::List(inner) if *inner == TypeInterner::NEVER => return Ok(()),
            Type::Optional(inner) | Type::List(inner) | Type::Set(inner) => {
                return plan_type_inner(types, *inner, program, seen);
            }
            Type::Map(key, value) => {
                plan_type_inner(types, *key, program, seen)?;
                return plan_type_inner(types, *value, program, seen);
            }
            Type::Result(ok, error) => {
                plan_type_inner(types, *ok, program, seen)?;
                return plan_type_inner(types, *error, program, seen);
            }
            _ => {}
        }
    }
    copy_plan_type(types, ty)
}
