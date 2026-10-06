//! Exact returned hook metadata, retained independently of the current CFG.
//! A callable type never selects a hook; only its authenticated producer does.
use super::*;
use hir::ExpressionKind as E;

#[derive(Debug, Clone, PartialEq)]
pub(super) struct DescriptorValue {
    original: Expression,
    current: Expression,
    hook: hir::ResourceHookRef,
}
#[derive(Debug, Clone, PartialEq)]
pub(super) struct DescriptorBinding {
    original: Local,
    current: Local,
    value: DescriptorValue,
}
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct DescriptorWitness {
    graph: Option<DescriptorGraph>,
    returned: Option<hir::ResourceHookRef>,
    returns: Vec<DescriptorValue>,
    bindings: Vec<DescriptorBinding>,
    values: Vec<DescriptorValue>,
}

#[derive(Debug, Clone, PartialEq)]
struct DescriptorGraph {
    parameters: Vec<Param>,
    locals: Vec<Local>,
    blocks: Vec<BasicBlock>,
    entry: BlockId,
}
fn effects_only(
    block: &hir::Block,
    function: &hir::Function,
    manifest: &hir::ResourceManifest,
) -> bool {
    let metadata_type = |ty| manifest.hooks().any(|hook| hook.function_type() == ty);
    let metadata_local = |local: LocalId| {
        function
            .locals
            .get(local.index() as usize)
            .is_some_and(|header| header.id == local && metadata_type(header.ty))
    };
    for statement in &block.statements {
        match &statement.kind {
            hir::StatementKind::Return(_)
            | hir::StatementKind::Respond(_)
            | hir::StatementKind::Break
            | hir::StatementKind::Continue => return false,
            hir::StatementKind::If {
                then_block,
                else_block,
                ..
            } => {
                if !effects_only(then_block, function, manifest)
                    || else_block
                        .as_ref()
                        .is_some_and(|block| !effects_only(block, function, manifest))
                {
                    return false;
                }
            }
            hir::StatementKind::Scope(block) => {
                if !effects_only(block, function, manifest) {
                    return false;
                }
            }
            hir::StatementKind::While { .. }
            | hir::StatementKind::For { .. }
            | hir::StatementKind::Match { .. }
            | hir::StatementKind::ReflectedTypeDispatch { .. } => return false,
            hir::StatementKind::Trace(local) if metadata_local(*local) => return false,
            hir::StatementKind::Breakpoint { bindings, .. }
                if bindings.iter().any(|local| metadata_local(*local)) =>
            {
                return false;
            }
            _ => {}
        }
    }
    let mut valid = true;
    walk::hir_block(block, &mut |value| {
        valid &= !metadata_type(value.ty)
            && !matches!(
                value.kind,
                E::ResourceHookValue { .. } | E::InlineFunction { .. } | E::ClosureRef { .. }
            );
        if let E::Handle { failure, .. } = &value.kind {
            valid &= effects_only(failure, function, manifest);
        }
    });
    valid
}
fn original_value(
    value: &Expression,
    locals: &BTreeMap<u32, hir::ResourceHookRef>,
    summaries: &[Option<hir::ResourceHookRef>],
    manifest: &hir::ResourceManifest,
) -> Option<hir::ResourceHookRef> {
    let hook = match &value.kind {
        E::ResourceHookValue { hook } if manifest.contains_hook(hook) => hook.clone(),
        E::Local(local) => locals.get(&local.index())?.clone(),
        E::Clone(inner) if inner.ty == value.ty && inner.span == value.span => {
            original_value(inner, locals, summaries, manifest)?
        }
        E::Call {
            function,
            ownership: hir::CallOwnership::Source(source),
            ..
        } if (matches!(&source.target, hir::CallTarget::Function(id) if id == function)
            || matches!(&source.target, hir::CallTarget::Declaration { .. }))
            && source.bridge == hir::CallBridge::Direct
            && source.generated_operands.is_empty()
            && source
                .arguments
                .iter()
                .all(|argument| argument.staging == hir::ArgumentStaging::Original) =>
        {
            summaries.get(function.index() as usize)?.clone()?
        }
        _ => return None,
    };
    (hook.function_type() == value.ty).then_some(hook)
}
fn immutable_local<'a>(
    function: &'a hir::Function,
    local: LocalId,
    value: &Expression,
) -> Option<&'a Local> {
    function
        .locals
        .get(local.index() as usize)
        .filter(|header| {
            header.id == local
                && !header.mutable
                && header.view_source.is_none()
                && header.ty == value.ty
        })
}
fn root_summary(
    function: &hir::Function,
    summaries: &[Option<hir::ResourceHookRef>],
    manifest: &hir::ResourceManifest,
) -> Option<hir::ResourceHookRef> {
    if function.capture_count != 0
        || function.source_definition.is_none()
        || !function.identity.type_arguments.is_empty()
        || !function.identity.scoped_type_bindings.is_empty()
        || function.identity.specialization != Default::default()
    {
        return None;
    }
    let mut locals = BTreeMap::new();
    let mut returned = None;
    let mut returns = 0;
    for statement in &function.body.statements {
        match &statement.kind {
            hir::StatementKind::Let { local, value }
                if immutable_local(function, *local, value).is_some() =>
            {
                if let Some(hook) = original_value(value, &locals, summaries, manifest) {
                    locals.insert(local.index(), hook);
                }
            }
            hir::StatementKind::Return(Some(value)) => {
                let hook = original_value(value, &locals, summaries, manifest)?;
                if function.return_type != hook.function_type()
                    || returned.as_ref().is_some_and(|old| old != &hook)
                {
                    return None;
                }
                returned = Some(hook);
                returns += 1;
            }
            hir::StatementKind::If {
                then_block,
                else_block,
                ..
            } => {
                if !effects_only(then_block, function, manifest)
                    || else_block
                        .as_ref()
                        .is_some_and(|block| !effects_only(block, function, manifest))
                {
                    return None;
                }
            }
            // Nested descriptor exits still need their own retained return transport.
            hir::StatementKind::While { .. }
            | hir::StatementKind::For { .. }
            | hir::StatementKind::Scope(_)
            | hir::StatementKind::Match { .. }
            | hir::StatementKind::ReflectedTypeDispatch { .. }
            | hir::StatementKind::Return(None) => return None,
            _ => {}
        }
    }
    (returns == 1).then_some(returned).flatten()
}
fn summaries(archive: &hir::ResourceSourceArchive) -> Vec<Option<hir::ResourceHookRef>> {
    let mut summaries = vec![None; archive.functions().len()];
    let Some(manifest) = archive.manifest() else {
        return summaries;
    };
    // Relay chains are finite. Cycles cannot fabricate a descriptor base producer.
    for _ in 0..archive.functions().len() {
        let next: Vec<_> = archive
            .functions()
            .iter()
            .map(|function| root_summary(function, &summaries, manifest))
            .collect();
        if next == summaries {
            break;
        }
        summaries = next;
    }
    summaries
}
fn remember(
    values: &mut Vec<DescriptorValue>,
    expression: &Expression,
    hook: &hir::ResourceHookRef,
) {
    if !values
        .iter()
        .any(|value| crate::breakpoint_regions::expressions_equal(&value.original, expression))
    {
        values.push(DescriptorValue {
            original: expression.clone(),
            current: expression.clone(),
            hook: hook.clone(),
        });
    }
    if let E::Clone(inner) = &expression.kind {
        remember(values, inner, hook);
    }
}

pub(super) fn capture(
    function: &hir::Function,
    archive: &hir::ResourceSourceArchive,
) -> DescriptorWitness {
    let summaries = summaries(archive);
    let mut witness = DescriptorWitness {
        returned: summaries
            .get(function.id.index() as usize)
            .cloned()
            .flatten(),
        ..Default::default()
    };
    let Some(manifest) = archive.manifest() else {
        return witness;
    };
    let mut locals = BTreeMap::new();
    for statement in &function.body.statements {
        if let hir::StatementKind::Let { local, value } = &statement.kind
            && let Some(header) = immutable_local(function, *local, value)
            && let Some(hook) = original_value(value, &locals, &summaries, manifest)
        {
            remember(&mut witness.values, value, &hook);
            locals.insert(local.index(), hook.clone());
            witness.bindings.push(DescriptorBinding {
                original: header.clone(),
                current: header.clone(),
                value: DescriptorValue {
                    original: value.clone(),
                    current: value.clone(),
                    hook,
                },
            });
        }
        if let hir::StatementKind::Return(Some(value)) = &statement.kind
            && let Some(hook) = original_value(value, &locals, &summaries, manifest)
            && witness.returned.as_ref() == Some(&hook)
        {
            remember(&mut witness.values, value, &hook);
            witness.returns.push(DescriptorValue {
                original: value.clone(),
                current: value.clone(),
                hook,
            });
        }
    }
    walk::hir_block(&function.body, &mut |value| {
        if let E::IndirectCall {
            callee,
            ownership: hir::CallOwnership::Source(source),
            ..
        } = &value.kind
            && source.target
                == (hir::CallTarget::Indirect {
                    signature_type: callee.ty,
                })
            && source.bridge == hir::CallBridge::Direct
            && source.generated_operands.is_empty()
            && let Some(hook) = original_value(callee, &locals, &summaries, manifest)
        {
            remember(&mut witness.values, callee, &hook);
        }
    });
    witness
}

fn remap_expression(value: &mut Expression, remap: &mut impl FnMut(&mut LocalId)) {
    let mut wrapper = BasicBlock {
        id: BlockId(0),
        statements: vec![Statement {
            kind: StatementKind::Evaluate(value.clone()),
            span: value.span,
        }],
        terminator: Terminator {
            kind: TerminatorKind::Unreachable,
            span: value.span,
        },
    };
    crate::sequences::prune::block_locals(&mut wrapper, remap, &mut |_| {});
    if let StatementKind::Evaluate(remapped) = wrapper.statements.remove(0).kind {
        *value = remapped;
    }
}

pub(super) fn remap_locals(
    witness: &mut DescriptorWitness,
    map: &[Option<LocalId>],
    remap: &mut impl FnMut(&mut LocalId),
) {
    if let Some(graph) = &mut witness.graph {
        for parameter in &mut graph.parameters {
            remap(&mut parameter.local);
        }
        graph.locals.retain(|local| {
            map.get(local.id.index() as usize)
                .copied()
                .flatten()
                .is_some()
        });
        for local in &mut graph.locals {
            remap(&mut local.id);
            if let Some(source) = &mut local.view_source {
                remap(source);
            }
        }
        for block in &mut graph.blocks {
            crate::sequences::prune::block_locals(block, remap, &mut |_| {});
        }
    }
    for binding in &mut witness.bindings {
        remap(&mut binding.current.id);
        remap_expression(&mut binding.value.current, remap);
    }
    for value in witness.values.iter_mut().chain(&mut witness.returns) {
        remap_expression(&mut value.current, remap);
    }
}
pub(super) fn remap_blocks(
    witness: &mut DescriptorWitness,
    map: &[Option<BlockId>],
) -> Result<(), String> {
    let Some(graph) = &mut witness.graph else {
        return Ok(());
    };
    // Preflight every retained edge before deleting or renumbering any evidence.
    // An unmapped original target must never survive as a coincidentally equal new ID.
    let missing = std::cell::Cell::new(false);
    let check = |id: &mut BlockId| {
        if map.get(id.index() as usize).copied().flatten().is_none() {
            missing.set(true);
        }
    };
    let mut entry = graph.entry;
    check(&mut entry);
    for block in &graph.blocks {
        if map
            .get(block.id.index() as usize)
            .copied()
            .flatten()
            .is_some()
        {
            let mut terminator = block.terminator.kind.clone();
            crate::sequences::prune::block_targets(&mut terminator, &check);
        }
    }
    if missing.get() {
        return Err(
            "Resource descriptor canonical block map removes a retained entry or edge target"
                .into(),
        );
    }
    let remap = |id: &mut BlockId| {
        if let Some(mapped) = map.get(id.index() as usize).copied().flatten() {
            *id = mapped;
        }
    };
    graph.blocks.retain(|block| {
        map.get(block.id.index() as usize)
            .copied()
            .flatten()
            .is_some()
    });
    remap(&mut graph.entry);
    for block in &mut graph.blocks {
        remap(&mut block.id);
        crate::sequences::prune::block_targets(&mut block.terminator.kind, &remap);
    }
    Ok(())
}
fn mapped_value_valid(value: &DescriptorValue, witness: &ResourceLoweringWitness) -> bool {
    if matches!(value.original.kind, E::Call { .. }) {
        return witness.calls.iter().any(|call| {
            crate::breakpoint_regions::expressions_equal(&call.original, &value.original)
                && crate::breakpoint_regions::expressions_equal(&call.current, &value.current)
        });
    }
    let mut restored = value.current.clone();
    let mut valid = true;
    remap_expression(&mut restored, &mut |local| {
        if let Some(binding) = witness
            .descriptors
            .bindings
            .iter()
            .find(|binding| binding.current.id == *local)
        {
            *local = binding.original.id;
        } else {
            valid = false;
        }
    });
    valid && crate::breakpoint_regions::expressions_equal(&restored, &value.original)
}
fn header_same(original: &Local, current: &Local) -> bool {
    let mut restored = current.clone();
    restored.id = original.id;
    restored == *original
}

pub(super) fn validate(
    witness: &ResourceLoweringWitness,
    function: &Function,
) -> Result<(), String> {
    let expected = capture(&witness.original, &witness.source);
    let current = &witness.descriptors;
    current.current(function)?;
    if current.graph.is_none() && (!current.values.is_empty() || current.returned.is_some()) {
        return Err("Resource descriptor lost its authenticated current body transport".into());
    }
    if current.returned != expected.returned
        || current.bindings.len() != expected.bindings.len()
        || current.values.len() != expected.values.len()
        || current.returns.len() != expected.returns.len()
    {
        return Err("Resource descriptor lost its exact archived return or producer family".into());
    }
    for (binding, original) in current.bindings.iter().zip(&expected.bindings) {
        if binding.original != original.original
            || binding.value.original != original.value.original
            || binding.value.hook != original.value.hook
            || !header_same(&binding.original, &binding.current)
            || function.local(binding.current.id) != Some(&binding.current)
            || !mapped_value_valid(&binding.value, witness)
        {
            return Err(
                "Resource descriptor changed its exact immutable producer header or Source tuple"
                    .into(),
            );
        }
        let count = function.blocks.iter().flat_map(|block| &block.statements).filter(|statement| {
            matches!(&statement.kind, StatementKind::Let { local, value }
                if *local == binding.current.id && crate::breakpoint_regions::expressions_equal(value, &binding.value.current))
        }).count();
        if count != 1 {
            return Err("Resource descriptor has no unique current immutable producer".into());
        }
    }
    for (value, original) in current.values.iter().zip(&expected.values) {
        if value.original != original.original
            || value.hook != original.hook
            || !mapped_value_valid(value, witness)
        {
            return Err(
                "Resource descriptor differs from its archived hook, producer or alias occurrence"
                    .into(),
            );
        }
    }
    for (value, original) in current.returns.iter().zip(&expected.returns) {
        if value.original != original.original
            || value.hook != original.hook
            || !mapped_value_valid(value, witness)
        {
            return Err("Resource descriptor differs from its exact retained Return".into());
        }
        let count = function
            .blocks
            .iter()
            .filter(|block| {
                matches!(&block.terminator.kind, TerminatorKind::Return(Some(actual))
                if crate::breakpoint_regions::expressions_equal(actual, &value.current))
            })
            .count();
        if count != 1 {
            return Err("Resource descriptor return has no unique current Return transport".into());
        }
    }
    current_values(witness, function)?;
    Ok(())
}

pub(super) fn current_values(
    witness: &ResourceLoweringWitness,
    function: &Function,
) -> Result<Vec<(usize, hir::ResourceHookRef)>, String> {
    let mut values = BTreeMap::new();
    for proof in &witness.descriptors.values {
        let mut count = 0;
        for block in &function.blocks {
            walk::mir_block(block, &mut |value| {
                if crate::breakpoint_regions::expressions_equal(value, &proof.current) {
                    values.insert(std::ptr::from_ref(value).addr(), proof.hook.clone());
                    count += 1;
                }
            });
        }
        if count != 1 {
            return Err(
                "Resource descriptor lost its unique original/current value occurrence".into(),
            );
        }
    }
    let mut escaped = false;
    for block in &function.blocks {
        walk::mir_block(block, &mut |value| {
            for binding in &witness.descriptors.bindings {
                escaped |= match &value.kind {
                    E::Local(local) => {
                        *local == binding.current.id
                            && !values.contains_key(&std::ptr::from_ref(value).addr())
                    }
                    E::ClosureRef { captures, .. } => captures.contains(&binding.current.id),
                    _ => false,
                };
            }
            escaped |= matches!(value.kind, E::ResourceHookValue { .. })
                && !values.contains_key(&std::ptr::from_ref(value).addr());
        });
        for statement in &block.statements {
            let locals: &[LocalId] = match &statement.kind {
                StatementKind::Trace(local) => std::slice::from_ref(local),
                StatementKind::Breakpoint { bindings, .. } => bindings,
                StatementKind::Assign { target, .. } if matches!(target.kind, E::Local(_)) => {
                    if let E::Local(local) = &target.kind {
                        std::slice::from_ref(local)
                    } else {
                        &[]
                    }
                }
                _ => &[],
            };
            escaped |= witness
                .descriptors
                .bindings
                .iter()
                .any(|binding| locals.contains(&binding.current.id));
        }
    }
    if escaped {
        return Err(
            "Resource descriptor has an unproved current escape, mutation or producer".into(),
        );
    }
    Ok(values.into_iter().collect())
}
impl DescriptorWitness {
    pub(super) fn current(&self, function: &Function) -> Result<(), String> {
        if let Some(graph) = &self.graph
            && (graph.parameters != function.params
                || graph.locals != function.locals
                || graph.entry != function.entry
                || !crate::breakpoint_regions::blocks_equal(&graph.blocks, &function.blocks))
        {
            return Err("Resource descriptor body or headers differ from their authenticated current Source transport".into());
        }
        Ok(())
    }

    pub(super) fn seal(&mut self, function: &Function) {
        if self.returned.is_some() || !self.values.is_empty() {
            self.graph = Some(DescriptorGraph {
                parameters: function.params.clone(),
                locals: function.locals.clone(),
                blocks: function.blocks.clone(),
                entry: function.entry,
            });
        }
    }
    pub(super) fn returned(&self) -> Option<&hir::ResourceHookRef> {
        self.returned.as_ref()
    }
    pub(super) fn locals(&self) -> impl Iterator<Item = (LocalId, &hir::ResourceHookRef)> {
        self.bindings
            .iter()
            .map(|binding| (binding.current.id, &binding.value.hook))
    }
    pub(super) fn binding(
        &self,
        local: LocalId,
        value: &Expression,
    ) -> Option<&hir::ResourceHookRef> {
        self.bindings.iter().find_map(|binding| {
            (binding.current.id == local
                && crate::breakpoint_regions::expressions_equal(&binding.value.current, value))
            .then_some(&binding.value.hook)
        })
    }
    pub(super) fn return_matches(&self, value: &Expression) -> bool {
        self.returns
            .iter()
            .any(|proof| crate::breakpoint_regions::expressions_equal(&proof.current, value))
    }
}

#[cfg(test)]
#[path = "returned_descriptors_tests.rs"]
mod tests;
