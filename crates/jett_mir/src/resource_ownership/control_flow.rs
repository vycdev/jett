//! Resource-free control-flow transport preserves the independently proven custody state.
//! Sequence preparation may only replace an exact original ForEach by its canonical
//! consuming-list edit. It never creates a Resource owner, resident lease or join rule.
use super::*;
use hir::{BinaryOp, ExpressionKind as E};

// Erased storage is not evidence of absent custody, even if its nominal
// signature mentions no Resource. This is a bounded compiler proof, not a
// restriction on valid Source programs.
fn resource_free(types: &TypeInterner, ty: TypeId, seen: &mut BTreeSet<u32>) -> bool {
    if ty.index() as usize >= types.len() {
        return false;
    }
    if !seen.insert(ty.index()) {
        return true;
    }
    match types.resolve(ty) {
        Type::Resource(_)
        | Type::Interface(_)
        | Type::Actor(_)
        | Type::Function { .. }
        | Type::Capability(_)
        | Type::TypeConstruction
        | Type::Error => false,
        Type::List(inner)
        | Type::Set(inner)
        | Type::Optional(inner)
        | Type::Secret(inner)
        | Type::Refinement { base: inner, .. } => resource_free(types, *inner, seen),
        Type::Map(key, value) | Type::Result(key, value) => {
            resource_free(types, *key, seen) && resource_free(types, *value, seen)
        }
        Type::Struct(id) => types
            .resolve_struct(*id)
            .fields
            .iter()
            .all(|(_, ty)| resource_free(types, *ty, seen)),
        Type::Bitfield(id) => types
            .resolve_bitfield(*id)
            .fields
            .iter()
            .all(|field| resource_free(types, field.ty, seen)),
        Type::Enum(id) => types.resolve_enum(*id).variants.iter().all(|variant| {
            variant
                .fields
                .iter()
                .all(|(_, ty)| resource_free(types, *ty, seen))
        }),
        Type::Machine(id) | Type::MachineState { machine: id, .. } => {
            types.resolve_machine(*id).states.iter().all(|state| {
                state
                    .fields
                    .iter()
                    .all(|(_, ty)| resource_free(types, *ty, seen))
            })
        }
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
        | Type::String
        | Type::Bool
        | Type::Bytes
        | Type::Nothing
        | Type::Never => true,
    }
}
fn ordinary_type(types: &TypeInterner, ty: TypeId) -> Result<(), String> {
    if !resource_free(types, ty, &mut BTreeSet::new()) {
        return Err("pending ResourceOwnershipPlan: control-flow value needs its dedicated Resource carrier transport".into());
    }
    Ok(())
}
fn header<'a>(
    function: &'a Function,
    types: &TypeInterner,
    local: LocalId,
) -> Result<&'a Local, String> {
    let header = function
        .local(local)
        .ok_or("Resource control-flow binder lost its dense local header")?;
    ordinary_type(types, header.ty)?;
    Ok(header)
}
pub(super) fn consuming_list(
    function: &Function,
    types: &TypeInterner,
    key: LocalId,
    value: Option<LocalId>,
    by_view: bool,
    iterable: &Expression,
) -> Result<TypeId, String> {
    ordinary_type(types, iterable.ty)?;
    let Type::List(element) = types.resolve(iterable.ty) else {
        return Err("pending ResourceOwnershipPlan: this iterator needs its dedicated ordinary sequence normalization".into());
    };
    ordinary_type(types, *element)?;
    if by_view || value.is_some() || *element == TypeInterner::NEVER {
        return Err("pending ResourceOwnershipPlan: borrowed or uninhabited iteration needs its exact sequence transport".into());
    }
    let binding = header(function, types, key)?;
    if binding.ty != *element || binding.view_source.is_some() {
        return Err("Resource consuming iterator changed its exact ordinary element binder".into());
    }
    Ok(*element)
}
pub(super) fn ordinary_switch(
    function: &Function,
    types: &TypeInterner,
    scrutinee: &Expression,
    variants: &[(VariantId, BlockId, Vec<LocalId>)],
    otherwise: Option<BlockId>,
) -> Result<(), String> {
    ordinary_type(types, scrutinee.ty)?;
    let Type::Enum(id) = types.resolve(scrutinee.ty) else {
        return Err("Resource switch requires its exact ordinary enum discriminant".into());
    };
    let definition = types.resolve_enum(*id);
    let mut seen = BTreeSet::new();
    for (variant, _, bindings) in variants {
        let index = variant.index() as usize;
        let fields = &definition
            .variants
            .get(index)
            .ok_or("Resource switch selected a foreign enum variant")?
            .fields;
        if !seen.insert(index) || fields.len() != bindings.len() {
            return Err("Resource switch changed its distinct ordinary variant bindings".into());
        }
        for ((_, expected), binding) in fields.iter().zip(bindings) {
            ordinary_type(types, *expected)?;
            if header(function, types, *binding)?.ty != *expected {
                return Err("Resource switch changed its exact ordinary payload type".into());
            }
        }
    }
    if otherwise.is_none() && seen.len() != definition.variants.len() {
        return Err("Resource switch lost its checked exhaustive variant edges".into());
    }
    Ok(())
}
fn sequence_element(
    function: &Function,
    types: &TypeInterner,
    source: &SequenceSource,
) -> Result<TypeId, String> {
    let SequenceSource::Local(local) = source else {
        return Err(
            "pending ResourceOwnershipPlan: projected sequence needs its exact aggregate transport"
                .into(),
        );
    };
    let source = header(function, types, *local)?;
    let Type::List(element) = types.resolve(source.ty) else {
        return Err("Resource sequence statement changed its exact ordinary list source".into());
    };
    ordinary_type(types, *element)?;
    Ok(*element)
}
pub(super) fn sequence_length(
    function: &Function,
    types: &TypeInterner,
    source: &SequenceSource,
    target: LocalId,
) -> Result<(), String> {
    sequence_element(function, types, source)?;
    if header(function, types, target)?.ty != TypeInterner::INT64 {
        return Err("Resource sequence length changed its ordinary integer destination".into());
    }
    Ok(())
}
pub(super) fn sequence_get(
    function: &Function,
    types: &TypeInterner,
    consume: bool,
    source: &SequenceSource,
    index: LocalId,
    target: LocalId,
    part: SequencePart,
) -> Result<(), String> {
    let element = sequence_element(function, types, source)?;
    if !consume
        || part != SequencePart::Element
        || header(function, types, index)?.ty != TypeInterner::INT64
        || header(function, types, target)?.ty != element
    {
        return Err(
            "Resource consuming sequence changed its exact ordinary element extraction".into(),
        );
    }
    Ok(())
}
fn local(id: LocalId, ty: TypeId, span: Span) -> Expression {
    Expression {
        kind: E::Local(id),
        ty,
        span,
    }
}
fn temporary(function: &mut Function, ty: TypeId, span: Span) -> LocalId {
    let id = LocalId::new(function.locals.len() as u32);
    function.locals.push(Local {
        id,
        name: format!("$iterator{}", id.index()),
        ty,
        debug_ty: ty,
        debug_type_name: None,
        mutable: true,
        view_source: None,
        span,
    });
    id
}
/// Reconstruct rather than trusting the edit transcript to choose statements,
/// headers, guards or edges. The before graph must still match all independent seals.
fn canonical_sequence(
    before: &Function,
    edit: &crate::breakpoint_regions::SequenceEdit,
    types: &TypeInterner,
) -> Result<(Function, BlockId), String> {
    let block = before
        .blocks
        .get(edit.header.index() as usize)
        .filter(|block| block.id == edit.header)
        .ok_or("Resource sequence transition lost its exact original header")?;
    let TerminatorKind::ForEach {
        key,
        value,
        by_view,
        iterable,
        body,
        exit,
    } = &block.terminator.kind
    else {
        return Err("Resource sequence transition has no exact original ForEach".into());
    };
    consuming_list(before, types, *key, *value, *by_view, iterable)?;
    let cfg = ControlFlowGraph::analyze(before)
        .map_err(|errors| format!("Resource sequence CFG is malformed: {errors:?}"))?;
    let mut outside = vec![false; before.blocks.len()];
    let mut pending = vec![before.entry];
    while let Some(current) = pending.pop() {
        if current == edit.header || outside[current.index() as usize] {
            continue;
        }
        outside[current.index() as usize] = true;
        pending.extend_from_slice(cfg.successors(current));
    }
    let preheaders = cfg
        .predecessors(edit.header)
        .iter()
        .copied()
        .filter(|id| outside[id.index() as usize])
        .collect::<Vec<_>>();
    let [preheader] = preheaders.as_slice() else {
        return Err("Resource sequence transition has no unique original preheader".into());
    };
    if !matches!(before.blocks[preheader.index() as usize].terminator.kind, TerminatorKind::Goto(target) if target == edit.header)
    {
        return Err("Resource sequence transition changed its exact preheader edge".into());
    }
    let mut expected = before.clone();
    let span = iterable.span;
    let cursor = temporary(&mut expected, TypeInterner::INT64, span);
    let length = temporary(&mut expected, TypeInterner::INT64, span);
    let source = temporary(&mut expected, iterable.ty, span);
    let value = if let E::View(inner) = &iterable.kind {
        *inner.clone()
    } else {
        iterable.clone()
    };
    let initialization = vec![
        Statement {
            kind: StatementKind::Let {
                local: source,
                value,
            },
            span,
        },
        Statement {
            kind: StatementKind::Let {
                local: cursor,
                value: Expression {
                    kind: E::Int(0),
                    ty: TypeInterner::INT64,
                    span,
                },
            },
            span,
        },
        Statement {
            kind: StatementKind::SequenceLength {
                source: SequenceSource::Local(source),
                target: length,
            },
            span,
        },
    ];
    let prefix = vec![
        Statement {
            kind: StatementKind::SequenceGet {
                consume: true,
                source: SequenceSource::Local(source),
                index: cursor,
                target: *key,
                part: SequencePart::Element,
            },
            span,
        },
        Statement {
            kind: StatementKind::Assign {
                target: local(cursor, TypeInterner::INT64, span),
                value: Expression {
                    kind: E::Binary {
                        left: Box::new(local(cursor, TypeInterner::INT64, span)),
                        op: BinaryOp::Add,
                        right: Box::new(Expression {
                            kind: E::Int(1),
                            ty: TypeInterner::INT64,
                            span,
                        }),
                    },
                    ty: TypeInterner::INT64,
                    span,
                },
            },
            span,
        },
    ];
    let terminator = Terminator {
        kind: TerminatorKind::Branch {
            condition: Expression {
                kind: E::Binary {
                    left: Box::new(local(cursor, TypeInterner::INT64, span)),
                    op: BinaryOp::Less,
                    right: Box::new(local(length, TypeInterner::INT64, span)),
                },
                ty: TypeInterner::BOOL,
                span,
            },
            then_block: *body,
            else_block: *exit,
        },
        span: block.terminator.span,
    };
    if !crate::breakpoint_regions::terminator_equal(&edit.before, &block.terminator)
        || !crate::breakpoint_regions::terminator_equal(&edit.after, &terminator)
        || !edit.redirects.is_empty()
        || !edit.blocks.is_empty()
        || edit.append.len() != 1
        || edit.append[0].0 != *preheader
        || edit.prefix.as_ref().map(|(id, _)| *id) != Some(*body)
    {
        return Err(
            "Resource sequence transcript differs from the canonical consuming-list edit".into(),
        );
    }
    let same_statements = |left: &[Statement], right: &[Statement]| {
        left.len() == right.len()
            && left
                .iter()
                .zip(right)
                .all(|(left, right)| crate::breakpoint_regions::statement_equal(left, right))
    };
    if !same_statements(&edit.append[0].1, &initialization)
        || !same_statements(
            &edit
                .prefix
                .as_ref()
                .ok_or("Resource sequence prefix disappeared")?
                .1,
            &prefix,
        )
    {
        return Err(
            "Resource sequence transcript changed its exact source, cursor or extraction order"
                .into(),
        );
    }
    expected.blocks[preheader.index() as usize]
        .statements
        .extend(initialization);
    let statements = &mut expected.blocks[body.index() as usize].statements;
    let mut all = prefix;
    all.append(statements);
    *statements = all;
    expected.blocks[edit.header.index() as usize].terminator = terminator;
    Ok((expected, *body))
}
pub(crate) fn sequence_transition(
    function: &mut Function,
    before: &Function,
    edit: &crate::breakpoint_regions::SequenceEdit,
    types: &TypeInterner,
) -> Result<(), String> {
    let Some(original) = before.resource_lowering.as_ref() else {
        return Ok(());
    };
    original.current(before)?;
    if !function
        .resource_lowering
        .as_ref()
        .is_some_and(|current| current.same(original))
    {
        return Err("Resource sequence transition changed its exact before-witness".into());
    }
    let (expected, body) = canonical_sequence(before, edit, types)?;
    if function.id != expected.id
        || function.identity != expected.identity
        || function.params != expected.params
        || function.locals != expected.locals
        || function.capture_count != expected.capture_count
        || function.return_type != expected.return_type
        || function.entry != expected.entry
        || function.span != expected.span
        || function.debug_kind != expected.debug_kind
        || !crate::breakpoint_regions::blocks_equal(&function.blocks, &expected.blocks)
    {
        return Err(
            "Resource sequence transition changed more than its exact reconstructed canonical edit"
                .into(),
        );
    }
    let mut witness = original.clone();
    witness.locals = expected.locals.clone();
    witness.blocks = expected.blocks.clone();
    borrowed_sums::shift_prefix(&mut witness, body, 2);
    lexical_borrows::shift_prefix(&mut witness, body, 2);
    for region in &mut witness.regions {
        region.shift_prefix(body, 2);
    }
    // The independent body advances only after reconstructing the same finite edit.
    if original.descriptors.has_body() {
        witness.descriptors.seal_body(&expected);
    }
    witness.current(function)?;
    function.resource_lowering = Some(witness);
    Ok(())
}
