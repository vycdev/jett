//! Private association of canonical prepared iterators with their original For.
//! These rows describe current sites; they issue no owner, lease or Source permit.
use super::*;
use hir::{BinaryOp, StatementKind as H};

struct SourceFor<'a> {
    key: LocalId,
    value: Option<LocalId>,
    by_view: bool,
    iterable: &'a Expression,
    span: Span,
}

#[derive(Default)]
struct Inventory<'a> {
    loops: Vec<SourceFor<'a>>,
    binders: BTreeSet<u32>,
}

impl<'a> Inventory<'a> {
    fn block(&mut self, block: &'a hir::Block) -> Result<(), String> {
        for statement in &block.statements {
            match &statement.kind {
                H::For {
                    key,
                    value,
                    by_view,
                    iterable,
                    body,
                } => {
                    for binder in std::iter::once(*key).chain(value.iter().copied()) {
                        if !self.binders.insert(binder.index()) {
                            return Err(
                                "carrier iterator duplicated an original Source binder".into()
                            );
                        }
                    }
                    self.loops.push(SourceFor {
                        key: *key,
                        value: *value,
                        by_view: *by_view,
                        iterable,
                        span: statement.span,
                    });
                    self.expression(iterable)?;
                    self.block(body)?;
                }
                H::If {
                    condition,
                    then_block,
                    else_block,
                } => {
                    self.expression(condition)?;
                    self.block(then_block)?;
                    if let Some(block) = else_block {
                        self.block(block)?;
                    }
                }
                H::While { condition, body } => {
                    self.expression(condition)?;
                    self.block(body)?;
                }
                H::Match { scrutinee, arms } => {
                    self.expression(scrutinee)?;
                    for arm in arms {
                        self.block(&arm.body)?;
                    }
                }
                H::Scope(block) => self.block(block)?,
                H::ReflectedTypeDispatch { type_info, arms } => {
                    self.expression(type_info)?;
                    for arm in arms {
                        self.block(&arm.body)?;
                    }
                }
                H::Let { value, .. }
                | H::Expression(value)
                | H::HandleDefault(value)
                | H::Respond(value) => self.expression(value)?,
                H::Assign { target, value } => {
                    self.expression(target)?;
                    self.expression(value)?;
                }
                H::Return(value) => {
                    if let Some(value) = value {
                        self.expression(value)?;
                    }
                }
                H::Assert { condition, message } => {
                    self.expression(condition)?;
                    if let Some(message) = message {
                        self.expression(message)?;
                    }
                }
                H::Breakpoint { condition, .. } => {
                    if let Some(condition) = condition {
                        self.expression(condition)?;
                    }
                }
                H::Break | H::Continue | H::Trace(_) => {}
            }
        }
        Ok(())
    }

    fn expression(&mut self, expression: &'a Expression) -> Result<(), String> {
        match &expression.kind {
            E::Handle {
                target, failure, ..
            } => {
                self.expression(target)?;
                self.block(failure)?;
            }
            E::Binary { left, right, .. } => {
                self.expression(left)?;
                self.expression(right)?;
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
            | E::StateIs { value, .. } => self.expression(value)?,
            E::Call { args, .. }
            | E::ResourceInvoke { args, .. }
            | E::Intrinsic { args, .. }
            | E::ActorSpawn { args, .. } => {
                for value in args {
                    self.expression(value)?;
                }
            }
            E::IndirectCall { callee, args, .. } => {
                self.expression(callee)?;
                for value in args {
                    self.expression(value)?;
                }
            }
            E::ActorMessage { actor, args, .. } => {
                self.expression(actor)?;
                for value in args {
                    self.expression(value)?;
                }
            }
            E::StructConstruct { fields, .. } | E::BitfieldConstruct { fields, .. } => {
                for value in fields {
                    self.expression(value)?;
                }
            }
            E::EnumConstruct { payloads, .. } | E::MachineConstruct { payloads, .. } => {
                for value in payloads {
                    self.expression(value)?;
                }
            }
            E::MachineTransition {
                source, payloads, ..
            } => {
                self.expression(source)?;
                for value in payloads {
                    self.expression(value)?;
                }
            }
            E::ListConstruct { elements } => {
                for value in elements {
                    self.expression(value)?;
                }
            }
            E::MapConstruct { entries } => {
                for entry in entries {
                    self.expression(&entry.key)?;
                    self.expression(&entry.value)?;
                }
            }
            E::StringInterpolation(parts) => {
                for part in parts {
                    if let hir::StringSegment::Value(value) = part {
                        self.expression(value)?;
                    }
                }
            }
            E::Field { base, .. } => self.expression(base)?,
            // A separate callable body belongs to its own function/local archive.
            E::InlineFunction { .. }
            | E::Int(_)
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
        Ok(())
    }
}

fn dense_block(function: &Function, id: BlockId) -> Result<&BasicBlock, String> {
    let index =
        usize::try_from(id.index()).map_err(|_| "carrier iterator block is not representable")?;
    function
        .blocks
        .get(index)
        .filter(|block| block.id == id)
        .ok_or_else(|| "carrier iterator lost its dense current block".into())
}

fn local_expression(value: &Expression, local: LocalId, ty: TypeId, span: Span) -> bool {
    value.ty == ty
        && value.span == span
        && matches!(&value.kind, E::Local(current) if *current == local)
}

fn temporary(function: &Function, id: LocalId, ty: TypeId, span: Span) -> Result<(), String> {
    let local = function
        .local(id)
        .ok_or("carrier iterator temporary lost its dense header")?;
    if local.ty != ty
        || local.debug_ty != ty
        || local.span != span
        || !local.mutable
        || local.view_source.is_some()
        || function.is_view_local(id)
        || function.parameter_for_local(id).is_some()
    {
        return Err("carrier iterator changed its exact temporary declaration".into());
    }
    Ok(())
}

fn source_expression(
    witness: &ResourceLoweringWitness,
    original: &Expression,
    current: &Expression,
) -> bool {
    if crate::breakpoint_regions::expressions_equal(original, current) {
        return true;
    }
    let direct = witness
        .calls
        .iter()
        .filter(|row| {
            crate::breakpoint_regions::expressions_equal(&row.original, original)
                && crate::breakpoint_regions::expressions_equal(&row.current, current)
        })
        .count();
    let normalized = if let E::Local(local) = &current.kind {
        witness
            .regions
            .iter()
            .filter(|row| {
                row.output() == Some(*local)
                    && crate::breakpoint_regions::expressions_equal(row.original(), original)
                    && original.ty == current.ty
            })
            .count()
    } else {
        0
    };
    direct + normalized == 1
}

fn binding(
    function: &Function,
    original: &hir::Function,
    id: LocalId,
    ty: TypeId,
) -> Result<(), String> {
    let index =
        usize::try_from(id.index()).map_err(|_| "carrier iterator binder is not representable")?;
    let archived = original
        .locals
        .get(index)
        .filter(|local| local.id == id)
        .ok_or("carrier iterator binder has no exact original Source header")?;
    let current = function
        .local(id)
        .ok_or("carrier iterator binder lost its current header")?;
    if archived.ty != ty
        || current.ty != ty
        || current.debug_ty != ty
        || archived.mutable
        || current.mutable
        || archived.view_source.is_some()
        || current.view_source.is_some()
        || function.is_view_local(id)
        || function.parameter_for_local(id).is_some()
    {
        return Err("carrier iterator changed its exact original owning binder".into());
    }
    Ok(())
}

fn source_slot(
    carriers: &ResourceCarrierFunctionPlan,
    header: &Local,
) -> Result<ResourceCarrierSlotId, String> {
    let matches = carriers.slots().iter().filter(|slot| {
        matches!(slot.storage(), ResourceSlotStorage::Local { header: current } if current == header)
            && carriers.shape(slot.shape()).is_some_and(|shape| {
                shape.ty() == header.ty && shape.contains_resource()
            })
    }).collect::<Vec<_>>();
    let [slot] = matches.as_slice() else {
        return Err("carrier iterator has no unique exact local source slot".into());
    };
    Ok(slot.id())
}

fn extraction(
    statement: &Statement,
    source: LocalId,
    cursor: LocalId,
    target: LocalId,
    part: SequencePart,
    span: Span,
) -> bool {
    statement.span == span
        && matches!(&statement.kind, StatementKind::SequenceGet {
        consume: true,
        source: SequenceSource::Local(current_source),
        index,
        target: current_target,
        part: current_part,
    } if *current_source == source && *index == cursor && *current_target == target && *current_part == part)
}

fn advance(statement: &Statement, cursor: LocalId, span: Span) -> bool {
    let StatementKind::Assign { target, value } = &statement.kind else {
        return false;
    };
    let E::Binary {
        left,
        op: BinaryOp::Add,
        right,
    } = &value.kind
    else {
        return false;
    };
    statement.span == span
        && value.ty == TypeInterner::INT64
        && value.span == span
        && local_expression(target, cursor, TypeInterner::INT64, span)
        && local_expression(left, cursor, TypeInterner::INT64, span)
        && right.ty == TypeInterner::INT64
        && right.span == span
        && matches!(&right.kind, E::Int(1))
}

fn preheader(
    function: &Function,
    cfg: &ControlFlowGraph,
    header: BlockId,
) -> Result<BlockId, String> {
    let mut outside = BTreeSet::new();
    let mut pending = vec![function.entry];
    while let Some(block) = pending.pop() {
        if block == header || !outside.insert(block.index()) {
            continue;
        }
        pending.extend_from_slice(cfg.successors(block));
    }
    let candidates = cfg
        .predecessors(header)
        .iter()
        .copied()
        .filter(|block| outside.contains(&block.index()))
        .collect::<Vec<_>>();
    let [preheader] = candidates.as_slice() else {
        return Err("carrier iterator lost its unique exact preheader".into());
    };
    if !matches!(&dense_block(function, *preheader)?.terminator.kind, TerminatorKind::Goto(target) if *target == header)
    {
        return Err("carrier iterator preheader bypasses its exact header".into());
    }
    Ok(*preheader)
}

struct Prepared {
    row: ResourceCarrierIteration,
    preheader: BlockId,
    length_site: usize,
}

fn prepared(
    function: &Function,
    types: &TypeInterner,
    carriers: &ResourceCarrierFunctionPlan,
    witness: &ResourceLoweringWitness,
    originals: &[SourceFor<'_>],
    cfg: &ControlFlowGraph,
    header: &BasicBlock,
) -> Result<Option<Prepared>, String> {
    let TerminatorKind::Branch {
        condition,
        then_block: body,
        else_block: exit,
    } = &header.terminator.kind
    else {
        return Ok(None);
    };
    let body_block = dense_block(function, *body)?;
    let Some(first) = body_block.statements.first() else {
        return Ok(None);
    };
    let StatementKind::SequenceGet {
        source,
        index: cursor,
        target: key,
        ..
    } = &first.kind
    else {
        return Ok(None);
    };
    let Some(ty) = source.ty(function) else {
        return Err("carrier iterator lost its source type".into());
    };
    if ty.index() as usize >= types.len() {
        return Err("carrier iterator source type is out of range".into());
    }
    if !carrier_type(types, ty) {
        return Ok(None);
    }
    let SequenceSource::Local(source) = source else {
        return Err(
            "pending Resource carrier: projected iteration needs its bounded parent lease".into(),
        );
    };
    let source_header = function
        .local(*source)
        .ok_or("carrier iterator source has no dense local header")?;
    let (key_type, value_type, key_part) = match types.resolve(ty) {
        Type::List(element) => (*element, None, SequencePart::Element),
        Type::Map(key, value) => (*key, Some(*value), SequencePart::Key),
        _ => return Err("carrier iterator source is not its exact list or map".into()),
    };
    let value = if value_type.is_some() {
        let second = body_block
            .statements
            .get(1)
            .ok_or("carrier map iterator lost its value extraction")?;
        let StatementKind::SequenceGet { target, .. } = &second.kind else {
            return Err("carrier map iterator reordered its exact value extraction".into());
        };
        Some(*target)
    } else {
        None
    };
    let candidates = originals
        .iter()
        .filter(|row| row.key == *key && row.value == value && row.iterable.ty == ty)
        .collect::<Vec<_>>();
    let [original] = candidates.as_slice() else {
        return Err("carrier iterator has no unique exact original Source For tuple".into());
    };
    if original.by_view
        || matches!(&original.iterable.kind, E::View(_) | E::Field { .. })
        || matches!(&original.iterable.kind, E::Local(local) if function.is_view_local(*local))
    {
        return Err(
            "pending Resource carrier: borrowed iteration needs its exact Source lease".into(),
        );
    }
    binding(function, &witness.original, *key, key_type)?;
    if let Some((binding_id, binding_type)) = value.zip(value_type) {
        binding(function, &witness.original, binding_id, binding_type)?;
    }
    let span = original.iterable.span;
    if key_type == TypeInterner::NEVER || value_type == Some(TypeInterner::NEVER) {
        return Err(
            "pending Resource carrier: uninhabited iteration needs its exact transport".into(),
        );
    }
    let E::Binary {
        left,
        op: BinaryOp::Less,
        right,
    } = &condition.kind
    else {
        return Err("carrier iterator changed its exact selecting comparison".into());
    };
    let E::Local(length) = &right.kind else {
        return Err("carrier iterator lost its exact length local".into());
    };
    if condition.ty != TypeInterner::BOOL
        || condition.span != span
        || header.terminator.span != original.span
        || !local_expression(left, *cursor, TypeInterner::INT64, span)
        || !local_expression(right, *length, TypeInterner::INT64, span)
        || *body == header.id
        || *exit == header.id
        || body == exit
        || *cursor == *length
        || *cursor == *source
        || *length == *source
    {
        return Err("carrier iterator changed its exact selecting endpoints".into());
    }
    temporary(function, *cursor, TypeInterner::INT64, span)?;
    temporary(function, *length, TypeInterner::INT64, span)?;
    temporary(function, *source, ty, span)?;
    if !extraction(first, *source, *cursor, *key, key_part, span) {
        return Err(
            "carrier iterator changed its exact consuming key or element extraction".into(),
        );
    }
    let mut binders = vec![*key];
    if let Some(value) = value {
        if value == *key {
            return Err("carrier map iterator duplicated its original binder".into());
        }
        let second = body_block
            .statements
            .get(1)
            .ok_or("carrier map extraction disappeared")?;
        if !extraction(second, *source, *cursor, value, SequencePart::Value, span) {
            return Err("carrier map iterator changed Key then Value extraction".into());
        }
        binders.push(value);
    }
    if binders
        .iter()
        .any(|binder| [*source, *cursor, *length].contains(binder))
    {
        return Err("carrier iterator aliased its source or cursor with a Source binder".into());
    }
    let increment = body_block
        .statements
        .get(binders.len())
        .ok_or("carrier iterator advance is absent")?;
    if !advance(increment, *cursor, span) {
        return Err("carrier iterator changed its exact cursor advance".into());
    }
    if cfg
        .predecessors(*body)
        .iter()
        .any(|predecessor| *predecessor != header.id)
    {
        return Err("carrier iterator body has an entry outside its exact selecting header".into());
    }
    let preheader_id = preheader(function, cfg, header.id)?;
    let preheader = dense_block(function, preheader_id)?;
    let start = preheader
        .statements
        .len()
        .checked_sub(3)
        .ok_or("carrier iterator initialization is incomplete")?;
    let [source_init, zero, length_init] = &preheader.statements[start..] else {
        return Err("carrier iterator initialization changed its exact arity".into());
    };
    let StatementKind::Let {
        local: source_local,
        value: source_value,
    } = &source_init.kind
    else {
        return Err("carrier iterator lost its owning source initializer".into());
    };
    if source_init.span != span
        || *source_local != *source
        || source_value.ty != ty
        || !source_expression(witness, original.iterable, source_value)
        || zero.span != span
        || !matches!(&zero.kind, StatementKind::Let { local, value } if *local == *cursor
            && value.ty == TypeInterner::INT64 && value.span == span && matches!(&value.kind, E::Int(0)))
        || length_init.span != span
        || !matches!(&length_init.kind, StatementKind::SequenceLength {
            source: SequenceSource::Local(current_source), target
        } if *current_source == *source && *target == *length)
    {
        return Err(
            "carrier iterator changed its exact Source, zero or length initialization".into(),
        );
    }
    Ok(Some(Prepared {
        row: ResourceCarrierIteration {
            header: header.id,
            body: *body,
            exit: *exit,
            source: source_slot(carriers, source_header)?,
            binders,
            cursor: Some(*cursor),
        },
        preheader: preheader_id,
        length_site: start + 2,
    }))
}

pub(super) fn capture(
    function: &Function,
    types: &TypeInterner,
    carriers: &ResourceCarrierFunctionPlan,
) -> Result<Vec<ResourceCarrierIteration>, String> {
    let witness = function
        .resource_lowering
        .as_ref()
        .ok_or("carrier iterator has no exact Source witness")?;
    witness.source.validate_types(types)?;
    witness.current(function)?;
    let mut inventory = Inventory::default();
    inventory.block(&witness.original.body)?;
    let cfg = ControlFlowGraph::analyze(function)
        .map_err(|errors| format!("carrier iterator CFG is malformed: {errors:?}"))?;
    let mut rows = Vec::new();
    let mut binders = BTreeSet::new();
    let mut sources = BTreeSet::new();
    let mut cursors = BTreeSet::new();
    let mut lengths = BTreeSet::new();
    let mut gets = BTreeSet::new();
    // Raw For rows remain the planner's operation metadata. They cannot share
    // their original binders with a second prepared loop.
    for block in &function.blocks {
        if let TerminatorKind::ForEach {
            key,
            value,
            iterable,
            ..
        } = &block.terminator.kind
            && (iterable.ty.index() as usize) < types.len()
            && carrier_type(types, iterable.ty)
        {
            for binder in std::iter::once(*key).chain(value.iter().copied()) {
                if !binders.insert(binder.index()) {
                    return Err("carrier raw iterator reused its Source binder".into());
                }
            }
        }
    }
    for block in &function.blocks {
        let Some(prepared) = prepared(
            function,
            types,
            carriers,
            witness,
            &inventory.loops,
            &cfg,
            block,
        )?
        else {
            continue;
        };
        if !sources.insert(prepared.row.source.index())
            || !cursors.insert(
                prepared
                    .row
                    .cursor
                    .ok_or("prepared carrier iterator has no cursor")?
                    .index(),
            )
            || !lengths.insert((prepared.preheader.index(), prepared.length_site))
        {
            return Err(
                "carrier prepared iterator duplicated its source, cursor or initialization".into(),
            );
        }
        for (ordinal, binder) in prepared.row.binders.iter().enumerate() {
            if !binders.insert(binder.index()) || !gets.insert((prepared.row.body.index(), ordinal))
            {
                return Err(
                    "carrier prepared iterator reused its Source binder or extraction".into(),
                );
            }
        }
        rows.push(prepared.row);
    }
    for block in &function.blocks {
        for (position, statement) in block.statements.iter().enumerate() {
            let source = match &statement.kind {
                StatementKind::SequenceLength { source, .. }
                | StatementKind::SequenceGet { source, .. }
                | StatementKind::IterationBorrow { source, .. } => source,
                _ => continue,
            };
            let ty = source
                .ty(function)
                .ok_or("carrier sequence statement lost its exact source type")?;
            if ty.index() as usize >= types.len() {
                return Err("carrier sequence source type is out of range".into());
            }
            if !carrier_type(types, ty) {
                continue;
            }
            let site = (block.id.index(), position);
            match &statement.kind {
                StatementKind::SequenceLength { .. } if lengths.contains(&site) => {}
                StatementKind::SequenceGet { .. } if gets.contains(&site) => {}
                StatementKind::IterationBorrow { .. } => {
                    return Err("pending Resource carrier: borrowed iteration needs its exact current lease".into());
                }
                _ => {
                    return Err(
                        "carrier sequence statement is outside its exact prepared Source iterator"
                            .into(),
                    );
                }
            }
        }
    }
    Ok(rows)
}
