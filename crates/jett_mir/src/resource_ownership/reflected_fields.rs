//! Exact ordinal guards derived only from the original checked field-loop archive.
use super::*;
use hir::{BinaryOp, ExpressionKind as E};

#[derive(Debug, Clone, PartialEq)]
pub(super) struct FieldDispatch {
    original: hir::ReflectedFieldDispatchProof,
    key: Local,
    type_info: Expression,
    guards: Vec<BlockId>,
    dispatches: Vec<BlockId>,
    targets: Vec<BlockId>,
    paths: Vec<Vec<usize>>,
    otherwise: BlockId,
}

pub(crate) fn ordinal_guard(
    proof: &hir::ReflectedFieldDispatchProof,
    type_info: &Expression,
    index: usize,
) -> Result<Expression, String> {
    let E::Field {
        base,
        owner_type,
        field,
    } = &type_info.kind
    else {
        return Err("Resource field dispatch lost its checked field selector".into());
    };
    if *field != proof.type_info_field() {
        return Err("Resource field dispatch changed its TypeInfo member".into());
    }
    let index = i128::try_from(index)
        .map_err(|_| "Resource field ordinal exceeds the checked scalar representation")?;
    Ok(Expression {
        kind: E::Binary {
            left: Box::new(Expression {
                kind: E::Field {
                    base: base.clone(),
                    owner_type: *owner_type,
                    field: proof.field_index(),
                },
                ty: TypeInterner::INT64,
                span: type_info.span,
            }),
            op: BinaryOp::Equal,
            right: Box::new(Expression {
                kind: E::Int(index),
                ty: TypeInterner::INT64,
                span: type_info.span,
            }),
        },
        ty: TypeInterner::BOOL,
        span: type_info.span,
    })
}

impl Capture {
    pub(crate) fn reflected_field_dispatch(
        &self,
        type_info: &Expression,
        arms: &[hir::ReflectedTypeArm],
    ) -> Option<hir::ReflectedFieldDispatchProof> {
        let witness = self.witness.as_ref()?;
        witness
            .source
            .get_reflected_field_dispatch(witness.original.id, type_info, arms)
            .cloned()
    }

    pub(crate) fn capture_reflected_field_dispatch(
        &mut self,
        original: hir::ReflectedFieldDispatchProof,
        key: Local,
        guards: Vec<BlockId>,
        dispatches: Vec<BlockId>,
        targets: Vec<BlockId>,
        otherwise: BlockId,
    ) -> Result<(), String> {
        let Some(witness) = &mut self.witness else {
            return Err("Resource field dispatch has no original Source constructor".into());
        };
        let paths = (0..original.arms().len())
            .map(|index| {
                original
                    .arm_container_path(index)
                    .map(<[_]>::to_vec)
                    .ok_or("Resource field dispatch lost its ordinal Scope path")
            })
            .collect::<Result<Vec<_>, _>>()?;
        witness.reflected_fields.push(FieldDispatch {
            type_info: original.type_info().clone(),
            original,
            key,
            guards,
            dispatches,
            targets,
            paths,
            otherwise,
        });
        Ok(())
    }
}

impl FieldDispatch {
    fn current(
        &self,
        witness: &ResourceLoweringWitness,
        function: &Function,
    ) -> Result<(), String> {
        let proof = witness
            .source
            .get_reflected_field_dispatch(
                function.id,
                self.original.type_info(),
                self.original.arms(),
            )
            .ok_or("Resource field dispatch lost its exact checked archive membership")?;
        if proof != &self.original
            || proof.function() != function.id
            || self.guards.len() != proof.arms().len()
            || self.dispatches.len() != proof.arms().len()
            || self.targets.len() != proof.arms().len()
            || self.paths.len() != proof.arms().len()
            || function.local(self.key.id) != Some(&self.key)
        {
            return Err(
                "Resource field dispatch changed its sealed ordinal/header inventory".into(),
            );
        }
        let E::Field { base, .. } = &self.type_info.kind else {
            return Err("Resource field dispatch lost its current field binder".into());
        };
        if !matches!(base.kind, E::Local(key) if key == self.key.id) || base.ty != self.key.ty {
            return Err("Resource field dispatch changed its exact current field binder".into());
        }
        let block = |id: BlockId| {
            function
                .blocks
                .get(id.index() as usize)
                .filter(|block| block.id == id)
                .ok_or("Resource field dispatch lost its current block")
        };
        let mut sites = BTreeSet::new();
        for (index, arm) in proof.arms().iter().enumerate() {
            if proof.iteration_index(index) != Some(index)
                || proof.arm_container_path(index) != Some(self.paths[index].as_slice())
                || !sites.insert(self.guards[index])
                || !sites.insert(self.dispatches[index])
                || !sites.insert(self.targets[index])
            {
                return Err(
                    "Resource field dispatch reordered ordinals or changed original Scope paths"
                        .into(),
                );
            }
            let next = self
                .guards
                .get(index + 1)
                .copied()
                .unwrap_or(self.otherwise);
            let expected = ordinal_guard(proof, &self.type_info, index)?;
            if !matches!(&block(self.guards[index])?.terminator.kind,
                TerminatorKind::Branch { condition, then_block, else_block }
                    if crate::breakpoint_regions::expressions_equal(condition, &expected)
                        && *then_block == self.dispatches[index] && *else_block == next)
            {
                return Err(
                    "Resource field dispatch changed its exact ordinal selecting guard".into(),
                );
            }
            let expected = ReflectedTypeDispatchArm {
                iteration_index: index,
                bound_type: arm.bound_type,
                reflection_identity: arm.reflection_identity.clone(),
                target: self.targets[index],
            };
            if !matches!(&block(self.dispatches[index])?.terminator.kind,
                TerminatorKind::ReflectedTypeDispatch { type_info, arms, otherwise }
                    if crate::breakpoint_regions::expressions_equal(type_info, &self.type_info)
                        && arms.as_slice() == [expected] && *otherwise == self.otherwise)
            {
                return Err(
                    "Resource field dispatch changed its exact one-arm body/reflection join".into(),
                );
            }
        }
        if !sites.insert(self.otherwise)
            || !matches!(
                block(self.otherwise)?.terminator.kind,
                TerminatorKind::Unreachable
            )
        {
            return Err("Resource field dispatch changed its refusing metadata boundary".into());
        }
        Ok(())
    }
}

pub(super) fn seal(witness: &mut ResourceLoweringWitness) {
    witness.reflected_seal = witness.reflected_fields.clone();
}
pub(super) fn current(
    witness: &ResourceLoweringWitness,
    function: &Function,
) -> Result<(), String> {
    if witness.reflected_fields != witness.reflected_seal {
        return Err(
            "Resource field dispatch changed its independent constructor membership seal".into(),
        );
    }
    for (index, row) in witness.reflected_fields.iter().enumerate() {
        if witness.reflected_fields[..index]
            .iter()
            .any(|previous| previous.original == row.original)
        {
            return Err("Resource field dispatch duplicated an original checked occurrence".into());
        }
        row.current(witness, function)?;
    }
    Ok(())
}
pub(super) fn admits(witness: &ResourceLoweringWitness, block: BlockId) -> bool {
    witness
        .reflected_fields
        .iter()
        .any(|row| row.dispatches.contains(&block))
}

/// Ordinary proof-only helpers retain the established ordinary sequence edit.
/// The finite edit cannot introduce Resource data or a runtime execution role.
pub(super) fn ordinary_sequence_transition(
    function: &mut Function,
    before: &Function,
    edit: &crate::breakpoint_regions::SequenceEdit,
    types: &TypeInterner,
) -> Result<(), String> {
    let original = before
        .resource_lowering
        .as_ref()
        .ok_or("Ordinary reflected sequence lost its original Source proof")?;
    original.current(before)?;
    if has_execution_records(before, types)
        || original.reflected_fields.is_empty()
        || !original.calls.is_empty()
        || !original.regions.is_empty()
        || !original.borrowed_sums.is_empty()
        || !original.lexical_exits.is_empty()
        || !function
            .resource_lowering
            .as_ref()
            .is_some_and(|current| current.same(original))
    {
        return Err("Ordinary reflected sequence cannot change a Resource custody witness".into());
    }
    // This exact finite ordinary edit validator is also used by ordinary
    // borrowed-sum and call-generation proof transitions. The shadow cannot
    // install a new breakpoint, Scope, owner, provider or reflected proof.
    let mut expected = function.clone();
    crate::breakpoint_regions::sequence_transition(&mut expected, before, edit.clone(), types)?;
    if function.id != before.id
        || function.identity != before.identity
        || function.params != before.params
        || function.capture_count != before.capture_count
        || function.return_type != before.return_type
        || function.entry != before.entry
        || function.span != before.span
        || function.debug_kind != before.debug_kind
        || function.locals.len() < before.locals.len()
        || function.locals[..before.locals.len()] != before.locals
        || function.locals[before.locals.len()..].iter().any(|local| {
            local.ty.index() as usize >= types.len() || resource_type_pending(types, local.ty)
        })
        || function.blocks.iter().any(|block| {
            walk::mir_block_has_resource(block)
                || walk::mir_block_has_custody(block, function, types)
        })
    {
        return Err(
            "Ordinary reflected sequence changed original headers or introduced custody".into(),
        );
    }
    let mut witness = original.clone();
    witness.locals = function.locals.clone();
    witness.blocks = function.blocks.clone();
    if let Some((body, prefix)) = &edit.prefix {
        lexical_borrows::shift_prefix(&mut witness, *body, prefix.len());
    }
    if original.descriptors.has_body() {
        witness.descriptors.seal_body(function);
    }
    witness.current(function)?;
    function.resource_lowering = Some(witness);
    Ok(())
}

pub(super) fn remap_blocks(
    witness: &mut ResourceLoweringWitness,
    map: &[Option<BlockId>],
) -> Result<(), String> {
    let remap = |id: &mut BlockId| -> Result<(), String> {
        *id = map
            .get(id.index() as usize)
            .copied()
            .flatten()
            .ok_or("Resource field dispatch lost a live canonical ordinal block")?;
        Ok(())
    };
    for row in &mut witness.reflected_fields {
        for id in row
            .guards
            .iter_mut()
            .chain(&mut row.dispatches)
            .chain(&mut row.targets)
        {
            remap(id)?;
        }
        remap(&mut row.otherwise)?;
    }
    seal(witness);
    Ok(())
}
pub(super) fn remap_locals(
    witness: &mut ResourceLoweringWitness,
    remap: &mut impl FnMut(&mut LocalId),
) {
    for row in &mut witness.reflected_fields {
        remap(&mut row.key.id);
        if let Some(id) = &mut row.key.view_source {
            remap(id);
        }
        let mut wrapper = BasicBlock {
            id: BlockId(0),
            statements: vec![Statement {
                kind: StatementKind::Evaluate(row.type_info.clone()),
                span: row.type_info.span,
            }],
            terminator: Terminator {
                kind: TerminatorKind::Unreachable,
                span: row.type_info.span,
            },
        };
        crate::sequences::prune::block_locals(&mut wrapper, remap, &mut |_| {});
        if let StatementKind::Evaluate(value) = wrapper.statements.remove(0).kind {
            row.type_info = value;
        }
    }
    seal(witness);
}

#[cfg(test)]
#[path = "reflected_fields_tests.rs"]
mod tests;
