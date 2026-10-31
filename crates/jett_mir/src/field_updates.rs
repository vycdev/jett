//! Original checked field-write places and their preflight/commit pairing.
use crate::{ControlFlowGraph, Function, Program, StatementKind};
use jett_hir::{self as hir, ExpressionKind, FieldUpdatePlace};
use jett_types::TypeInterner;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Witness {
    identity: hir::FunctionIdentity,
    entry: crate::BlockId,
    locals: Vec<hir::Local>,
    params: Vec<hir::Param>,
    blocks: Vec<crate::BasicBlock>,
    sites: Vec<Site>,
}
#[derive(Debug, Clone, PartialEq)]
struct Site {
    place: FieldUpdatePlace,
    target: hir::Expression,
    value: hir::Expression,
}

pub(crate) fn seal(
    original: &hir::Function,
    function: &Function,
    types: &TypeInterner,
) -> Result<Option<Witness>, String> {
    let mut sources = hir::field_update_places(&original.body)
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let mut sites = Vec::new();
    for statement in function.blocks.iter().flat_map(|block| &block.statements) {
        if let StatementKind::UpdateField {
            target,
            value,
            place,
        } = &statement.kind
        {
            let index = sources
                .iter()
                .position(|source| source == place)
                .ok_or("field update has no exact original checked HIR place")?;
            sources.remove(index);
            place.validate(target, &function.locals, types)?;
            sites.push(Site {
                place: place.clone(),
                target: target.clone(),
                value: value.clone(),
            });
        }
    }
    if !sources.is_empty() {
        return Err("checked HIR field update was lost during MIR lowering".into());
    }
    if sites.is_empty() {
        return Ok(None);
    }
    let witness = Witness {
        identity: original.identity.clone(),
        entry: function.entry,
        locals: function.locals.clone(),
        params: function.params.clone(),
        blocks: function.blocks.clone(),
        sites,
    };
    validate(function, types, Some(&witness))?;
    Ok(Some(witness))
}

fn key(place: &FieldUpdatePlace) -> (u32, u32, u32, hir::FieldUpdateOccurrence) {
    let span = place.span();
    (span.file.index(), span.start, span.end, place.occurrence())
}

fn validate(
    function: &Function,
    types: &TypeInterner,
    witness: Option<&Witness>,
) -> Result<(), String> {
    let mut preparations = BTreeMap::new();
    let mut commits = Vec::new();
    for statement in function.blocks.iter().flat_map(|block| &block.statements) {
        match &statement.kind {
            StatementKind::PrepareFieldUpdate { place, begin: true } => {
                place.validate(place.target(), &function.locals, types)?;
                if place.required_scopes().len() != place.scoped_type_bindings().len() {
                    return Err(
                        "field update changed its checked scope/type-binding inventory".into(),
                    );
                }
                for (scope, binding) in place
                    .required_scopes()
                    .iter()
                    .zip(place.scoped_type_bindings())
                {
                    if matches!(
                        scope.selection,
                        hir::FieldUpdateScopeSelection::ReflectedIteration(_)
                    ) {
                        crate::resource_ownership::validate_field_update_scope(
                            function, scope, binding,
                        )?;
                    }
                }
                if statement.span != place.span()
                    || preparations.insert(key(place), place).is_some()
                {
                    return Err(
                        "field update preflight is duplicated or has changed Source association"
                            .into(),
                    );
                }
            }
            StatementKind::UpdateField {
                target,
                value,
                place,
            } => {
                place.validate(target, &function.locals, types)?;
                if function.is_view_local(place.root())
                    || statement.span != place.span()
                    || value.ty != target.ty
                {
                    return Err(
                        "field update commit changed ownership, Source association or field type"
                            .into(),
                    );
                }
                commits.push(Site {
                    place: place.clone(),
                    target: target.clone(),
                    value: value.clone(),
                });
            }
            _ => {}
        }
    }
    if commits.is_empty() && preparations.is_empty() {
        return if witness.is_none() {
            Ok(())
        } else {
            Err("field update witness lost its current operations".into())
        };
    }
    let witness =
        witness.ok_or("field update has no constructor-authenticated original/current witness")?;
    if witness.identity != function.identity
        || witness.entry != function.entry
        || witness.locals != function.locals
        || witness.params != function.params
        || !crate::breakpoint_regions::blocks_equal(&witness.blocks, &function.blocks)
        || witness.sites != commits
        || preparations.len() != commits.len()
        || commits
            .iter()
            .any(|site| preparations.get(&key(&site.place)).copied() != Some(&site.place))
    {
        return Err("field update changed its exact original/current operations".into());
    }
    let cfg = ControlFlowGraph::analyze(function).map_err(|error| format!("{error:?}"))?;
    let mut outgoing = vec![BTreeSet::new(); function.blocks.len()];
    loop {
        let mut changed = false;
        for block in cfg.reverse_postorder() {
            let mut active = BTreeSet::new();
            for predecessor in cfg.predecessors(*block) {
                active.extend(outgoing[predecessor.index() as usize].iter().copied());
            }
            for statement in &function.blocks[block.index() as usize].statements {
                match &statement.kind {
                    StatementKind::PrepareFieldUpdate { place, begin } => {
                        if *begin {
                            active.insert(key(place));
                        } else {
                            active.remove(&key(place));
                        }
                    }
                    StatementKind::UpdateField { place, .. } => {
                        active.remove(&key(place));
                    }
                    _ => {}
                }
            }
            if outgoing[block.index() as usize] != active {
                outgoing[block.index() as usize] = active;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let reachable = cfg
        .reverse_postorder()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    for block in cfg.reverse_postorder() {
        let predecessors = cfg
            .predecessors(*block)
            .iter()
            .filter(|predecessor| reachable.contains(*predecessor))
            .collect::<Vec<_>>();
        let mut active = if *block == function.entry {
            BTreeSet::new()
        } else {
            predecessors
                .first()
                .map_or_else(BTreeSet::new, |predecessor| {
                    outgoing[predecessor.index() as usize].clone()
                })
        };
        if predecessors
            .iter()
            .any(|predecessor| outgoing[predecessor.index() as usize] != active)
        {
            return Err("field update reservations disagree across a reachable CFG merge".into());
        }
        for statement in &function.blocks[block.index() as usize].statements {
            match &statement.kind {
                StatementKind::PrepareFieldUpdate { place, begin: true } => {
                    if !active.insert(key(place))
                        || active
                            .iter()
                            .filter_map(|key| preparations.get(key))
                            .filter(|other| other.root() == place.root())
                            .count()
                            != 1
                    {
                        return Err(
                            "field update root is already reserved by an enclosing RHS".into()
                        );
                    }
                }
                StatementKind::PrepareFieldUpdate {
                    place,
                    begin: false,
                } => {
                    if !active.remove(&key(place)) {
                        return Err("field update abort has no active preflight".into());
                    }
                }
                StatementKind::UpdateField { place, .. } => {
                    if !active.remove(&key(place)) {
                        return Err(
                            "field update commit has no dominating ancestor preflight".into()
                        );
                    }
                }
                StatementKind::Assign { target, .. }
                    if matches!(target.kind, ExpressionKind::Local(_)) =>
                {
                    let ExpressionKind::Local(local) = target.kind else {
                        unreachable!()
                    };
                    if active
                        .iter()
                        .filter_map(|key| preparations.get(key))
                        .any(|place| place.root() == local)
                    {
                        return Err("field update RHS overwrites its reserved root".into());
                    }
                }
                StatementKind::Let { local, .. }
                | StatementKind::ReplaceCallOwnerGeneration { root: local, .. } => {
                    if active
                        .iter()
                        .filter_map(|key| preparations.get(key))
                        .any(|place| place.root() == *local)
                    {
                        return Err("field update RHS changes its reserved root generation".into());
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

pub(crate) fn has_records(function: &Function) -> bool {
    function.field_updates.is_some()
}

fn refresh_canonical(
    function: &mut Function,
    before: &Function,
    types: &TypeInterner,
) -> Result<(), String> {
    let Some(original) = &before.field_updates else {
        return Ok(());
    };
    if function.identity != before.identity
        || function.entry != before.entry
        || function.params != before.params
        || !function.locals.starts_with(&before.locals)
    {
        return Err(
            "field update canonical preparation changed original binding identities".into(),
        );
    }
    let sites = function
        .blocks
        .iter()
        .flat_map(|block| &block.statements)
        .filter_map(|statement| {
            if let StatementKind::UpdateField {
                target,
                value,
                place,
            } = &statement.kind
            {
                Some(Site {
                    place: place.clone(),
                    target: target.clone(),
                    value: value.clone(),
                })
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    if sites != original.sites {
        return Err("field update canonical preparation changed its original commits".into());
    }
    let mut next = original.clone();
    next.locals = function.locals.clone();
    next.blocks = function.blocks.clone();
    next.entry = function.entry;
    function.field_updates = Some(next);
    validate(function, types, function.field_updates.as_ref())
}

pub(super) fn sequence_transition(
    function: &mut Function,
    before: &Function,
    edit: &crate::breakpoint_regions::SequenceEdit,
    types: &TypeInterner,
) -> Result<(), String> {
    if !has_records(before) {
        return Ok(());
    }
    validate(before, types, before.field_updates.as_ref())?;
    let mut shadow = function.clone();
    crate::breakpoint_regions::sequence_transition(&mut shadow, before, edit.clone(), types)?;
    refresh_canonical(function, before, types)
}

pub(super) fn sum_transition(
    function: &mut Function,
    before: &Function,
    types: &TypeInterner,
) -> Result<(), String> {
    if !has_records(before) {
        return Ok(());
    }
    validate(before, types, before.field_updates.as_ref())?;
    let mut shadow = function.clone();
    crate::breakpoint_regions::sum_transition(&mut shadow, before, types)?;
    refresh_canonical(function, before, types)
}

pub(super) fn remap_blocks(
    function: &mut Function,
    before: &Function,
    map: &[Option<crate::BlockId>],
) -> Result<(), String> {
    if !has_records(before) {
        return Ok(());
    }
    let witness = before
        .field_updates
        .as_ref()
        .ok_or("field update compaction lost its witness")?;
    if !crate::breakpoint_regions::blocks_equal(&before.blocks, &witness.blocks)
        || before.entry != witness.entry
        || before.locals != witness.locals
        || before.params != witness.params
    {
        return Err("field update compaction started from changed current Source".into());
    }
    let cfg = ControlFlowGraph::analyze(before).map_err(|errors| format!("{errors:?}"))?;
    let reachable = cfg
        .reverse_postorder()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if map.len() != before.blocks.len()
        || before
            .blocks
            .iter()
            .any(|block| map[block.id.index() as usize].is_some() != reachable.contains(&block.id))
    {
        return Err("field update block map is not exact reachable compaction".into());
    }
    let get = |id: crate::BlockId| {
        map.get(id.index() as usize)
            .copied()
            .flatten()
            .ok_or("field update surviving block has no exact map")
    };
    let mut expected = before
        .blocks
        .iter()
        .filter(|block| reachable.contains(&block.id))
        .cloned()
        .collect::<Vec<_>>();
    for block in &mut expected {
        block.id = get(block.id)?;
        crate::sequences::prune::block_targets(&mut block.terminator.kind, &|target| {
            *target = map[target.index() as usize].expect("reachable target has map");
        });
    }
    if !crate::breakpoint_regions::blocks_equal(&expected, &function.blocks)
        || function.entry != get(before.entry)?
        || function.locals != before.locals
        || function.params != before.params
    {
        return Err("field update block compaction changed surviving operations".into());
    }
    let mut next = witness.clone();
    next.blocks = function.blocks.clone();
    next.entry = function.entry;
    // A removed complete update requires its own absent-commit certificate;
    // this bounded transition retains every original update until then.
    if next.sites.iter().any(|site| !function.blocks.iter().flat_map(|block| &block.statements).any(|statement|
        matches!(&statement.kind, StatementKind::UpdateField { target, value, place } if target == &site.target && value == &site.value && place == &site.place))) {
        return Err("field update compaction removed a commit without an abort certificate".into());
    }
    function.field_updates = Some(next);
    Ok(())
}

pub(crate) fn validate_function(function: &Function, types: &TypeInterner) -> Result<(), String> {
    validate(function, types, function.field_updates.as_ref())
}

pub fn validate_field_updates(program: &Program, types: &TypeInterner) -> Result<(), String> {
    for function in &program.functions {
        validate(function, types, function.field_updates.as_ref())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use jett_common::{FileId, SourceOrigin};
    use std::collections::HashMap;

    const SOURCE: &str = "struct Record:\n    value: int64\n    sibling: int64\nfunction next_value(view record: Record) returns int64:\n    return record.value + 1\nfunction update(mutable record: Record) returns Record:\n    record.value = next_value(view record)\n    return record\n";
    const SEQUENCE: &str = "struct Record:\n    value: int64\n    sibling: int64\nfunction update(mutable record: Record) returns Record:\n    list[int64] steps = list(2, 3)\n    for step in steps:\n        record.value = record.value + step\n    return record\n";
    fn source_program(source: &str) -> (Program, TypeInterner) {
        let file = FileId::new(0);
        let parsed = jett_parser::parse(source, file);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let resolved = jett_resolve::resolve(&parsed.module);
        assert!(
            resolved
                .diagnostics
                .iter()
                .all(|d| d.severity != jett_diagnostics::Severity::Error),
            "{:?}",
            resolved.diagnostics
        );
        let checked = jett_typecheck::check(&parsed.module, &resolved);
        assert!(
            checked
                .diagnostics
                .iter()
                .all(|d| d.severity != jett_diagnostics::Severity::Error),
            "{:?}",
            checked.diagnostics
        );
        let high = hir::lower(
            &parsed.module,
            &resolved,
            &checked,
            &HashMap::from([(file, SourceOrigin::Project)]),
        )
        .expect("normally checked HIR");
        let program = crate::lower(&high, &checked.interner).expect("checked-place MIR");
        (program, checked.interner)
    }
    fn update(program: &mut Program) -> &mut Function {
        program
            .functions
            .iter_mut()
            .find(|function| function.identity.declaration.name == "update")
            .unwrap()
    }
    #[test]
    fn source_field_update_keeps_rhs_temporary_view_and_rejects_proof_corruption() {
        let (program, types) = source_program(SOURCE);
        validate_field_updates(&program, &types).unwrap();
        let function = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "update")
            .unwrap();
        crate::move_values::MoveValuePlan::analyze(&program, function, &types)
            .expect("RHS call view ends before commit");
        for corruption in 0..4 {
            let mut changed = program.clone();
            let function = update(&mut changed);
            match corruption {
                0 => function.field_updates = None,
                1 => {
                    for block in &mut function.blocks {
                        block.statements.retain(|statement| {
                            !matches!(
                                statement.kind,
                                StatementKind::PrepareFieldUpdate { begin: true, .. }
                            )
                        });
                    }
                }
                2 => {
                    for block in &mut function.blocks {
                        for statement in &mut block.statements {
                            if let StatementKind::UpdateField { target, .. } = &mut statement.kind {
                                let ExpressionKind::Field { field, .. } = &mut target.kind else {
                                    panic!("checked field target")
                                };
                                *field = hir::FieldId::new(1);
                            }
                        }
                    }
                }
                3 => {
                    for block in &mut function.blocks {
                        if let Some(index) = block.statements.iter().position(|statement| {
                            matches!(
                                statement.kind,
                                StatementKind::PrepareFieldUpdate { begin: true, .. }
                            )
                        }) {
                            let preparation = block.statements.remove(index);
                            let commit = block
                                .statements
                                .iter()
                                .position(|statement| {
                                    matches!(statement.kind, StatementKind::UpdateField { .. })
                                })
                                .unwrap();
                            block.statements.insert(commit, preparation);
                            break;
                        }
                    }
                }
                _ => unreachable!(),
            }
            assert!(
                validate_field_updates(&changed, &types).is_err(),
                "corruption {corruption}"
            );
        }
    }
    #[test]
    fn source_field_update_survives_canonical_sequence_preparation_and_refuses_foreign_remap() {
        let (mut program, types) = source_program(SEQUENCE);
        crate::prepare_native_sequences(&mut program, &types);
        crate::prepare_native_uninhabited_sums(&mut program, &types);
        crate::prepare_native_generated_functions(&mut program, &types);
        validate_field_updates(&program, &types)
            .expect("canonical native preparation retains exact Source places");
        let before = update(&mut program).clone();
        assert!(
            !before.blocks.iter().any(|block| matches!(
                block.terminator.kind,
                crate::TerminatorKind::ForEach { .. }
            ))
        );
        let mut function = before.clone();
        let map = vec![None; before.blocks.len()];
        assert!(remap_blocks(&mut function, &before, &map).is_err());
        function.blocks[0].statements.clear();
        assert!(validate(&function, &types, function.field_updates.as_ref()).is_err());
    }
}

#[cfg(test)]
mod merge_tests {
    use super::*;
    #[test]
    fn fresh_seal_rejects_preflight_missing_on_one_diamond_edge() {
        let source = "struct Record:\n    value: int64\nfunction update(mutable record: Record) returns Record:\n    record.value = record.value + 1\n    return record\n";
        let file = jett_common::FileId::new(0);
        let parsed = jett_parser::parse(source, file);
        assert!(parsed.errors.is_empty());
        let resolved = jett_resolve::resolve(&parsed.module);
        let checked = jett_typecheck::check(&parsed.module, &resolved);
        assert!(
            checked
                .diagnostics
                .iter()
                .all(|d| d.severity != jett_diagnostics::Severity::Error),
            "{:?}",
            checked.diagnostics
        );
        let high = hir::lower(
            &parsed.module,
            &resolved,
            &checked,
            &std::collections::HashMap::from([(file, jett_common::SourceOrigin::Project)]),
        )
        .unwrap();
        let program = crate::lower(&high, &checked.interner).unwrap();
        let original = &high.functions[0];
        let mut function = program.functions[0].clone();
        assert_eq!(function.blocks.len(), 1);
        let mut continuation = function.blocks.remove(0);
        let begin = continuation.statements.remove(0);
        assert!(matches!(
            begin.kind,
            StatementKind::PrepareFieldUpdate { begin: true, .. }
        ));
        let span = begin.span;
        continuation.id = crate::BlockId(3);
        function.blocks = vec![
            crate::BasicBlock {
                id: crate::BlockId(0),
                statements: Vec::new(),
                terminator: crate::Terminator {
                    kind: crate::TerminatorKind::Branch {
                        condition: hir::Expression {
                            kind: ExpressionKind::Bool(true),
                            ty: TypeInterner::BOOL,
                            span,
                        },
                        then_block: crate::BlockId(1),
                        else_block: crate::BlockId(2),
                    },
                    span,
                },
            },
            crate::BasicBlock {
                id: crate::BlockId(1),
                statements: vec![begin],
                terminator: crate::Terminator {
                    kind: crate::TerminatorKind::Goto(crate::BlockId(3)),
                    span,
                },
            },
            crate::BasicBlock {
                id: crate::BlockId(2),
                statements: Vec::new(),
                terminator: crate::Terminator {
                    kind: crate::TerminatorKind::Goto(crate::BlockId(3)),
                    span,
                },
            },
            continuation,
        ];
        assert!(seal(original, &function, &checked.interner).is_err());
    }
}

#[cfg(test)]
mod reflected_tests;
