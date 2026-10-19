//! Constructor-only Source transport and independent current graph seal.
use super::*;
mod constructor_transport;
pub(super) use constructor_transport::ConstructorOutput;
use constructor_transport::{Transport, outputs};

#[derive(Debug, Clone)]
pub(super) struct Snapshot {
    id: FunctionId,
    identity: FunctionIdentity,
    debug: hir::FunctionDebugKind,
    capture_count: usize,
    span: jett_common::Span,
    return_type: TypeId,
    parameters: Vec<Param>,
    locals: Vec<Local>,
    blocks: Vec<BasicBlock>,
    entry: BlockId,
}
impl PartialEq for Snapshot {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.identity == other.identity
            && self.debug == other.debug
            && self.capture_count == other.capture_count
            && self.span == other.span
            && self.return_type == other.return_type
            && self.parameters == other.parameters
            && self.locals == other.locals
            && self.entry == other.entry
            && crate::breakpoint_regions::blocks_equal(&self.blocks, &other.blocks)
    }
}
impl Snapshot {
    fn new(function: &Function) -> Self {
        Self {
            id: function.id,
            identity: function.identity.clone(),
            debug: function.debug_kind.clone(),
            capture_count: function.capture_count,
            span: function.span,
            return_type: function.return_type,
            parameters: function.params.clone(),
            locals: function.locals.clone(),
            blocks: function.blocks.clone(),
            entry: function.entry,
        }
    }
    fn current(&self, function: &Function) -> Result<(), String> {
        if self.id != function.id
            || self.identity != function.identity
            || self.debug != function.debug_kind
            || self.capture_count != function.capture_count
            || self.span != function.span
            || self.return_type != function.return_type
            || self.parameters != function.params
            || self.locals != function.locals
            || self.entry != function.entry
            || !crate::breakpoint_regions::blocks_equal(&self.blocks, &function.blocks)
        {
            return Err("graph metadata changed its independent current Source/CFG seal".into());
        }
        Ok(())
    }
}
pub(super) fn contains(function: &Function, expression: &Expression) -> bool {
    let mut found = false;
    for block in &function.blocks {
        walk::mir_block(block, &mut |value| found |= std::ptr::eq(value, expression));
    }
    found
}
fn count(function: &Function, expression: &Expression) -> usize {
    let mut count = 0;
    for block in &function.blocks {
        walk::mir_block(block, &mut |value| {
            count += usize::from(equal(value, expression))
        });
    }
    count
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
    if let StatementKind::Evaluate(value_remapped) = wrapper.statements.remove(0).kind {
        *value = value_remapped;
    }
}
fn transported(
    value: &Expression,
    witness: &Witness,
    constructors: &[ResourceConstructorRegion],
    function: &Function,
) -> Result<Expression, String> {
    Transport {
        witness,
        constructors,
        function,
    }
    .expression(value)
}
fn header_same(original: &Local, current: &Local, witness: &Witness) -> bool {
    let mut restored = current.clone();
    restored.id = original.id;
    if let Some(source) = &mut restored.view_source {
        let Some((original, _)) = witness
            .local_map
            .iter()
            .find(|(_, current)| current.id == *source)
        else {
            return false;
        };
        *source = original.id;
    }
    restored == *original
}

/// Called once by the authenticated lowering constructor, after the complete
/// body is emitted. Public edits cannot call this or refresh this snapshot.
pub(in crate::resource_ownership) fn seal(
    witness: &mut Witness,
    function: &Function,
    calls: &[OriginalCallAssociation],
    constructors: &[ResourceConstructorRegion],
    types: &TypeInterner,
) -> Result<(), String> {
    if !witness.needed() {
        return Ok(());
    }
    if witness.graph.is_some() {
        return Err("graph metadata cannot reseal an existing constructor witness".into());
    }
    let original = witness
        .owner
        .as_ref()
        .ok_or("graph metadata lost original owner")?;
    if original.id != function.id
        || original.identity != function.identity
        || original.return_type != function.return_type
        || original.span != function.span
        || original.capture_count != function.capture_count
        || original.debug_kind != function.debug_kind
        || original.params != function.params
        || function.locals.len() < original.locals.len()
        || original
            .locals
            .iter()
            .zip(&function.locals)
            .any(|(left, right)| left != right)
    {
        return Err("graph metadata constructor changed its original Source/header prefix".into());
    }
    witness.constructor_outputs = outputs(&witness.values, constructors, function)?;
    let currents = witness
        .values
        .iter()
        .chain(&witness.returns)
        .map(|row| transported(&row.original, witness, constructors, function))
        .collect::<Result<Vec<_>, _>>()?;
    for (row, current) in witness
        .values
        .iter_mut()
        .chain(&mut witness.returns)
        .zip(currents)
    {
        row.current = current;
    }
    let edge_currents = witness
        .edges
        .iter()
        .map(|edge| transported(&edge.original_call, witness, constructors, function))
        .collect::<Result<Vec<_>, _>>()?;
    for (edge, expected) in witness.edges.iter_mut().zip(edge_currents) {
        let matching = calls
            .iter()
            .filter(|call| equal(&call.original, &edge.original_call))
            .collect::<Vec<_>>();
        let [call] = matching.as_slice() else {
            return Err(
                "graph Source transport lost unique original/current call association".into(),
            );
        };
        // The initial packet preserves whole metadata Source calls. Canonical
        // staging that changes the packet requires its own producer transcript.
        if !equal(&expected, &call.current) {
            return Err(
                "pending graph metadata: staged Source actual needs its exact producer transcript"
                    .into(),
            );
        }
        edge.current_call = call.current.clone();
    }
    for original in &witness.calls {
        let expected = transported(original, witness, constructors, function)?;
        if calls
            .iter()
            .filter(|call| equal(&call.original, original) && equal(&call.current, &expected))
            .count()
            != 1
            || count(function, &expected) != 1
        {
            return Err("graph metadata call changed original tuple or current occurrence".into());
        }
    }
    for row in &witness.headers {
        if function.local(row.current.id) != Some(&row.current)
            || row.proof.ty != row.current.ty
            || !storage_safe(types, row.proof.ty)
        {
            return Err("graph storage changed exact constructor local".into());
        }
        if !function
            .params
            .iter()
            .any(|parameter| parameter.local == row.current.id)
        {
            let initializers = function
                .blocks
                .iter()
                .flat_map(|block| &block.statements)
                .filter_map(|statement| match &statement.kind {
                    StatementKind::Let { local, value } if *local == row.current.id => Some(value),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if let [initializer]=initializers.as_slice() {
                if !witness.values.iter().any(|value|equal(&value.current,initializer) && value.proof==row.proof) {
                    return Err("graph storage local differs from protected original producer".into());
                }
            } else if initializers.is_empty() && original_iteration(original,row.original.id).is_some_and(|iterable|
                function.blocks.iter().filter(|block|matches!(&block.terminator.kind,TerminatorKind::ForEach {key,value,iterable:current,..}
                    if (*key==row.current.id || *value==Some(row.current.id)) && transported(iterable,witness,constructors,function).is_ok_and(|expected|equal(current,&expected)))).count()==1) {
                // Exact raw ForEach binder. A later prepared sequence requires
                // the independently reconstructed control-flow transition.
            } else {
                return Err("graph storage has no unique constructor local or iteration producer".into());
            }
        }
    }
    for row in &witness.returns {
        if !function
            .blocks
            .iter()
            .any(|block| match &block.terminator.kind {
                TerminatorKind::Return(Some(value)) | TerminatorKind::Respond(value) => {
                    equal(value, &row.current)
                }
                _ => false,
            })
        {
            return Err("graph return lost original/current exit expression".into());
        }
    }
    witness.graph = Some(Snapshot::new(function));
    Ok(())
}

pub(in crate::resource_ownership) fn validate(
    parent: &ResourceLoweringWitness,
    function: &Function,
    types: &TypeInterner,
) -> Result<(), String> {
    // The parent runs its full-body/header gate before this typed reconstruction.
    // This function must not recurse through the parent's current seal.
    let witness = &parent.graph_metadata;
    let expected = parent
        .execution
        .original_graph(&parent.original, &parent.source, types)?;
    if !constructors::records_equal(&parent.constructors, &parent.constructor_seal) {
        return Err("graph transport lost its independent constructor occurrence seal".into());
    }
    if witness.needed() != expected.needed() {
        return Err("graph metadata lost original checked root membership".into());
    }
    if !witness.needed() {
        return Ok(());
    }
    witness
        .graph
        .as_ref()
        .ok_or("graph metadata lost independent current graph")?
        .current(function)?;
    let owner = witness
        .owner
        .as_ref()
        .ok_or("graph metadata lost original Source owner")?;
    if !original_function_equal(owner, &parent.original)
        || witness.returned != expected.returned
        || !expression_rows_equal(&witness.calls, &expected.calls)
        || witness.values.len() != expected.values.len()
        || witness.headers.len() != expected.headers.len()
        || witness.formals.len() != expected.formals.len()
        || witness.returns.len() != expected.returns.len()
        || witness.edges.len() != expected.edges.len()
    {
        return Err(
            "graph metadata differs from reconstructed original whole-root Source rows".into(),
        );
    }
    if witness.constructor_outputs != outputs(&expected.values, &parent.constructor_seal, function)?
    {
        return Err("graph transport changed its exact generated constructor output rows".into());
    }
    for (row, original) in witness
        .values
        .iter()
        .chain(&witness.returns)
        .zip(expected.values.iter().chain(&expected.returns))
    {
        if !equal(&row.original, &original.original)
            || row.proof != original.proof
            || !storage_safe(types, row.proof.ty)
            || !equal(
                &row.current,
                &transported(&row.original, witness, &parent.constructor_seal, function)?,
            )
        {
            return Err(
                "graph metadata changed original/current expression producer or rooted shape"
                    .into(),
            );
        }
    }
    for row in &witness.sequence_outputs {
        if function.local(row.current.id) != Some(&row.current)
            || row.current.ty != row.proof.ty
            || !storage_safe(types, row.proof.ty)
            || !expected.values.iter().any(|value| {
                equal(&value.original, &row.original_iterable) && value.proof == row.proof
            })
        {
            return Err("graph iterator changed its exact generated source/proof".into());
        }
        let producer = transported(
            &row.original_iterable,
            witness,
            &parent.constructor_seal,
            function,
        )?;
        if function.blocks.iter().flat_map(|block| &block.statements)
            .filter(|statement| matches!(&statement.kind,
                StatementKind::Let { local, value } if *local == row.current.id && equal(value, &producer)))
            .count() != 1
        {
            return Err("graph iterator lost its unique protected source initializer".into());
        }
    }
    for (index, (original, current)) in witness.local_map.iter().enumerate() {
        if owner.locals.get(original.id.index() as usize) != Some(original)
            || !header_same(original, current, witness)
            || function.local(current.id) != Some(current)
            || witness.local_map[..index]
                .iter()
                .any(|(old, now)| old.id == original.id || now.id == current.id)
        {
            return Err("graph metadata changed its canonical original/current local map".into());
        }
    }
    for (row, original) in witness.headers.iter().zip(&expected.headers) {
        if row.original != original.original
            || row.proof != original.proof
            || !header_same(&row.original, &row.current, witness)
            || function.local(row.current.id) != Some(&row.current)
        {
            return Err("graph metadata changed exact original/current local header".into());
        }
    }
    for (row, original) in witness.formals.iter().zip(&expected.formals) {
        let mut restored = row.current.clone();
        restored.local = row.original.local;
        if row.original != original.original
            || row.proof != original.proof
            || restored != row.original
            || !function.params.contains(&row.current)
        {
            return Err(
                "graph metadata changed exact original/current formal Source header".into(),
            );
        }
    }
    for (edge, original) in witness.edges.iter().zip(&expected.edges) {
        if !equal(&edge.original_call, &original.original_call)
            || edge.formal != original.formal
            || edge.parameter != original.parameter
            || !equal(&edge.actual, &original.actual)
            || edge.proof != original.proof
            || !parent.calls.iter().any(|call| {
                equal(&call.original, &edge.original_call)
                    && equal(&call.current, &edge.current_call)
            })
            || !equal(
                &edge.current_call,
                &transported(
                    &edge.original_call,
                    witness,
                    &parent.constructor_seal,
                    function,
                )?,
            )
            || count(function, &edge.current_call) != 1
        {
            return Err(
                "graph metadata lost full original/current Source actual/formal tuple".into(),
            );
        }
    }
    Ok(())
}
pub(super) fn selection(
    parent: &ResourceLoweringWitness,
    function: &Function,
    types: &TypeInterner,
    expression: &Expression,
) -> Option<GraphStorageProof> {
    parent.current(function).ok()?;
    validate(parent, function, types).ok()?;
    contains(function, expression).then_some(())?;
    parent
        .graph_metadata
        .values
        .iter()
        .find(|row| equal(&row.current, expression))
        .map(|row| row.proof.clone())
}
pub(in crate::resource_ownership) fn current(
    witness: &Witness,
    function: &Function,
) -> Result<(), String> {
    if !witness.needed() {
        return Ok(());
    }
    witness
        .graph
        .as_ref()
        .ok_or("graph metadata lost independent current graph")?
        .current(function)
}

pub(in crate::resource_ownership) fn remap_locals(
    witness: &mut Witness,
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
    witness.local_map.retain(|(_, current)| {
        map.get(current.id.index() as usize)
            .copied()
            .flatten()
            .is_some()
    });
    for (_, current) in &mut witness.local_map {
        remap(&mut current.id);
        if let Some(source) = &mut current.view_source {
            remap(source);
        }
    }
    for row in &mut witness.headers {
        remap(&mut row.current.id);
        if let Some(source) = &mut row.current.view_source {
            remap(source);
        }
    }
    for row in &mut witness.sequence_outputs {
        remap(&mut row.current.id);
        if let Some(source) = &mut row.current.view_source {
            remap(source);
        }
    }
    for row in &mut witness.constructor_outputs {
        remap(&mut row.current.id);
        if let Some(source) = &mut row.current.view_source {
            remap(source);
        }
    }
    for row in &mut witness.formals {
        remap(&mut row.current.local);
    }
    for row in witness.values.iter_mut().chain(&mut witness.returns) {
        remap_expression(&mut row.current, remap);
    }
    for edge in &mut witness.edges {
        remap_expression(&mut edge.current_call, remap);
    }
}
fn original_iteration(function: &hir::Function, local: LocalId) -> Option<&Expression> {
    fn find(block: &hir::Block, local: LocalId) -> Option<&Expression> {
        for statement in &block.statements {
            let found = match &statement.kind {
                hir::StatementKind::For {
                    key,
                    value,
                    iterable,
                    ..
                } if *key == local || *value == Some(local) => Some(iterable),
                hir::StatementKind::For { body, .. }
                | hir::StatementKind::While { body, .. }
                | hir::StatementKind::Scope(body) => find(body, local),
                hir::StatementKind::If {
                    then_block,
                    else_block,
                    ..
                } => find(then_block, local)
                    .or_else(|| else_block.as_ref().and_then(|block| find(block, local))),
                hir::StatementKind::Match { arms, .. } => {
                    arms.iter().find_map(|arm| find(&arm.body, local))
                }
                hir::StatementKind::ReflectedTypeDispatch { arms, .. } => {
                    arms.iter().find_map(|arm| find(&arm.body, local))
                }
                _ => None,
            };
            if found.is_some() {
                return found;
            }
        }
        None
    }
    find(&function.body, local)
}
pub(in crate::resource_ownership) fn remap_blocks(
    witness: &mut Witness,
    map: &[Option<BlockId>],
) -> Result<(), String> {
    let Some(graph) = &mut witness.graph else {
        return Ok(());
    };
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
        return Err("graph metadata canonical block map removes retained entry or edge".into());
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

/// Advances only the canonical iterator body independently reconstructed by
/// control_flow. Original Source headers remain untouched; this records the
/// one generated owner initialized from the protected current iterable.
pub(in crate::resource_ownership) fn sequence_transition(
    witness: &mut Witness,
    before: &Function,
    expected: &Function,
    edit: &crate::breakpoint_regions::SequenceEdit,
) -> Result<(), String> {
    if !witness.needed() {
        return Ok(());
    }
    current(witness, before)?;
    let TerminatorKind::ForEach { iterable, .. } = &edit.before.kind else {
        return Err("graph sequence transport has no exact original iterator".into());
    };
    if let Some(row) = witness
        .values
        .iter()
        .find(|row| equal(&row.current, iterable))
    {
        let proof = row.proof.clone();
        let original_iterable = row.original.clone();
        let [(_, initialization)] = edit.append.as_slice() else {
            return Err("graph sequence transport lost its unique preheader".into());
        };
        let Some(Statement {
            kind: StatementKind::Let { local, value },
            ..
        }) = initialization.first()
        else {
            return Err("graph sequence transport lost its canonical source initializer".into());
        };
        if !equal(value, iterable) {
            return Err("graph sequence transport substituted its protected iterable".into());
        }
        let current = expected
            .local(*local)
            .filter(|header| header.ty == proof.ty)
            .ok_or("graph sequence transport changed its generated source header")?
            .clone();
        if witness
            .sequence_outputs
            .iter()
            .any(|row| row.current.id == current.id)
        {
            return Err("graph sequence transport repeated its generated source owner".into());
        }
        witness.sequence_outputs.push(SequenceOutput {
            current,
            original_iterable,
            proof,
        });
    }
    witness.graph = Some(Snapshot::new(expected));
    Ok(())
}
