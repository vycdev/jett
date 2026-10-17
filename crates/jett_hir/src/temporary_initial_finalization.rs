//! Original-only finalization of unchanged retained temporary producers.
//! The callable receipt proves table edits; this producer proves cell states.
use super::*;
use crate::reflected_callable::CheckedCallableSealing;

/// Mint-time inventory of one unpublished Lowerer construction. Never cloned,
/// reconstructed from a Program, or returned by a public lowering entry point.
pub(crate) struct Construction {
    module_address: usize,
    check_address: usize,
    records: Vec<TemporaryLocalBacking>,
}
impl Construction {
    pub(crate) fn new(module: &Module, check: &CheckResult) -> Self {
        Self {
            module_address: std::ptr::from_ref(module).addr(),
            check_address: std::ptr::from_ref(check).addr(),
            records: Vec::new(),
        }
    }
    pub(super) fn register(&mut self, proof: TemporaryLocalBacking) {
        self.records.push(proof);
    }
    fn context(
        &self,
        checked: &Arc<jett_typecheck::CheckedResourceProgram>,
        types: &TypeInterner,
    ) -> Result<(), String> {
        if self.module_address != std::ptr::from_ref(checked.module()).addr()
            || self.check_address != std::ptr::from_ref(checked.checked()).addr()
            || !std::ptr::eq(types, &checked.checked().interner)
        {
            return Err("temporary original finalization has another fresh construction, checked context or arena".into());
        }
        Ok(())
    }
}

#[derive(Clone)]
struct Row {
    function: FunctionId,
    proof: TemporaryLocalBacking,
    canonical: Expression,
}
pub(crate) struct Prepared {
    construction: Construction,
    checked: Arc<jett_typecheck::CheckedResourceProgram>,
    sources: Arc<Vec<Function>>,
    manifest: ResourceManifest,
    equality: HashMap<TypeId, FunctionId>,
    rows: Vec<Row>,
}
pub(crate) struct Completed {
    construction: Construction,
    checked: Arc<jett_typecheck::CheckedResourceProgram>,
    rows: Vec<Row>,
}

fn inventory(
    program: &Program,
    construction: &Construction,
    checked: &Arc<jett_typecheck::CheckedResourceProgram>,
    types: &TypeInterner,
) -> Result<Vec<Row>, String> {
    construction.context(checked, types)?;
    let mut result: Vec<Row> = Vec::new();
    for function in &program.functions {
        for proof in markers(function) {
            if !Arc::ptr_eq(&proof.0.checked, checked)
                || construction
                    .records
                    .iter()
                    .filter(|record| *record == &proof)
                    .count()
                    != 1
                || result.iter().any(|row| {
                    row.proof == proof
                        || (row.function == function.id
                            && [row.proof.owner(), row.proof.alias()]
                                .iter()
                                .any(|id| *id == proof.owner() || *id == proof.alias()))
                })
            {
                return Err(
                    "temporary original finalization changed its exact minted marker inventory"
                        .into(),
                );
            }
            let canonical = canonical_production(function, &proof, types)?;
            result.push(Row {
                function: function.id,
                proof,
                canonical,
            });
        }
    }
    if result.len() != construction.records.len()
        || construction
            .records
            .iter()
            .any(|proof| result.iter().filter(|row| &row.proof == proof).count() != 1)
    {
        return Err("temporary original finalization lost a minted declaration".into());
    }
    Ok(result)
}
fn same_rows(left: &[Row], right: &[Row]) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(a, b)| {
            a.function == b.function
                && a.proof == b.proof
                && resource_materialization::expression_equal(&a.canonical, &b.canonical)
        })
}
fn open(rows: &[Row]) -> Result<(), String> {
    if rows
        .iter()
        .any(|row| row.proof.0.canonical_producer.get().is_some())
    {
        return Err("temporary original finalization requires every exact cell to be open".into());
    }
    Ok(())
}
fn bound_callable(plan: &ReflectedCallablePlan) -> Result<(), String> {
    if plan.is_sealed() || plan.is_materialized() || plan.graph_source().is_some() {
        return Err(
            "temporary original canonical payload contains an unrepresented callable epoch".into(),
        );
    }
    bound_field(plan.return_plan())
}
fn bound_field(plan: &ReflectedFieldPlan) -> Result<(), String> {
    match &plan.action {
        ReflectedFieldAction::Exact
        | ReflectedFieldAction::Predicates(_)
        | ReflectedFieldAction::Unsupported(_) => Ok(()),
        ReflectedFieldAction::CallableReturn(plan) => bound_callable(plan),
        ReflectedFieldAction::Refine { base, .. }
        | ReflectedFieldAction::List(base)
        | ReflectedFieldAction::Set(base)
        | ReflectedFieldAction::Optional(base) => bound_field(base),
        ReflectedFieldAction::Map { key, value } => {
            bound_field(key)?;
            bound_field(value)
        }
        ReflectedFieldAction::Result { ok, error } => {
            bound_field(ok)?;
            bound_field(error)
        }
    }
}
fn argument_proof(argument: &TemporaryLocalArgument) -> Result<TemporaryLocalBacking, String> {
    if argument.owner != argument.proof.owner() || argument.alias != argument.proof.alias() {
        return Err("temporary original argument proof changed owner or alias identity".into());
    }
    Ok(argument.proof.clone())
}
fn payload_dependencies(value: &Expression) -> Result<Vec<TemporaryLocalBacking>, String> {
    let mut dependencies = Vec::new();
    let mut block = Block {
        statements: vec![Statement {
            kind: StatementKind::Expression(value.clone()),
            span: value.span,
        }],
        span: value.span,
    };
    resource_materialization::walk_block(&mut block, &mut |value| {
        if let Some(proof) = marker(value) {
            dependencies.push(proof.clone());
        }
        let ownership = match &value.kind {
            ExpressionKind::Call { ownership, .. }
            | ExpressionKind::ResourceInvoke { ownership, .. }
            | ExpressionKind::Intrinsic { ownership, .. }
            | ExpressionKind::IndirectCall { ownership, .. } => Some(ownership),
            _ => None,
        };
        if let Some(CallOwnership::Source(source)) = ownership {
            for argument in &source.arguments {
                if let Some(argument) = argument.source_witness().temporary_local_origin() {
                    dependencies.push(argument_proof(argument)?);
                }
            }
        }
        match &value.kind {
            ExpressionKind::ActorGraphImport(_) | ExpressionKind::ActorGraphStep { .. } => {
                return Err(
                    "temporary original canonical payload contains an unrepresented Graph epoch"
                        .into(),
                );
            }
            ExpressionKind::Intrinsic {
                field_validation: Some(validation),
                ..
            } => match validation {
                ReflectedFieldValidation::Validate(plans) => {
                    for plan in plans {
                        bound_field(plan)?;
                    }
                }
                _ => {
                    return Err(
                        "temporary original canonical payload acquired a MIR selector".into(),
                    );
                }
            },
            ExpressionKind::Handle {
                kind: HandleKind::ReflectedReturn { plan, callable },
                ..
            } => {
                bound_field(plan)?;
                if let Some(callable) = callable {
                    bound_callable(callable)?;
                }
            }
            ExpressionKind::InterfaceCoerce { adapters, .. } => {
                for adapter in adapters {
                    if let Some(plan) = &adapter.reflected {
                        bound_callable(plan)?;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    })?;
    let mut unique = Vec::new();
    for proof in dependencies {
        if !unique.contains(&proof) {
            unique.push(proof);
        }
    }
    Ok(unique)
}
fn dependency(row: &Row, predecessor: &TemporaryLocalBacking, rows: &[Row]) -> Result<(), String> {
    let selected = rows
        .iter()
        .filter(|candidate| candidate.proof == *predecessor)
        .collect::<Vec<_>>();
    let [earlier] = selected.as_slice() else {
        return Err(
            "temporary original proof depends on an absent or foreign minted declaration".into(),
        );
    };
    if earlier.function != row.function
        || !Arc::ptr_eq(&earlier.proof.0.checked, &row.proof.0.checked)
        || earlier.proof.0.scope.body != row.proof.0.scope.body
        || earlier.proof.0.scope.statement >= row.proof.0.scope.statement
    {
        return Err(
            "temporary original proof is not a strictly earlier same-body declaration".into(),
        );
    }
    Ok(())
}
fn closed(rows: &[Row]) -> Result<(), String> {
    for row in rows {
        for payload in [
            &row.proof.0.initializer,
            &row.proof.0.producer,
            &row.proof.0.rewritten,
            &row.canonical,
        ] {
            for predecessor in payload_dependencies(payload)? {
                dependency(row, &predecessor, rows)?;
            }
        }
    }
    Ok(())
}

/// No writes: retain only the genuine receipt's BEFORE canonical values.
pub(crate) fn prepare(
    before: &Program,
    output: &Program,
    types: &TypeInterner,
    receipt: &CheckedCallableSealing<'_>,
    checked: &Arc<jett_typecheck::CheckedResourceProgram>,
    construction: Construction,
) -> Result<Prepared, String> {
    receipt.authenticate_output(before, output, types)?;
    if before.resource_source != ResourceSourceArchive::empty()
        || !receipt
            .checked()
            .is_some_and(|owner| Arc::ptr_eq(owner, checked))
    {
        return Err("temporary original finalization has no exact initial callable receipt".into());
    }
    let rows = inventory(before, &construction, checked, types)?;
    let produced = inventory(output, &construction, checked, types)?;
    open(&rows)?;
    if rows.is_empty() || !same_rows(&rows, &produced) {
        return Err("temporary original producer changed during callable sealing; current-canonical renewal remains unsupported".into());
    }
    closed(&rows)?;
    let prepared = Prepared {
        construction,
        checked: checked.clone(),
        sources: receipt.source_epoch().clone(),
        manifest: before.resource_manifest.clone(),
        equality: before.equality_methods.clone(),
        rows,
    };
    #[cfg(test)]
    tests::observe(&prepared, before, output, types, receipt);
    Ok(prepared)
}
impl Prepared {
    /// The caller is the actual initial actor finish, after its exact full peer
    /// checkpoint and real sum producer have passed. No current body is stored.
    fn preflight_commit(
        &self,
        program: &Program,
        types: &TypeInterner,
        sources: &Arc<Vec<Function>>,
        checkpoint: &crate::actor_handlers::InitialPeerCheckpoint,
    ) -> Result<(), String> {
        checkpoint.authenticate(program, types, sources, &self.checked)?;
        if !Arc::ptr_eq(sources, &self.sources)
            || program.resource_source != ResourceSourceArchive::empty()
            || program.resource_manifest != self.manifest
            || program.equality_methods != self.equality
        {
            return Err("temporary original finalization lost its exact initial peer epoch".into());
        }
        let current = inventory(program, &self.construction, &self.checked, types)?;
        if !same_rows(&self.rows, &current) {
            return Err(
                "temporary original finalization changed its retained canonical production".into(),
            );
        }
        open(&self.rows)?;
        closed(&self.rows)?;
        Ok(())
    }
    pub(crate) fn commit(
        self,
        program: &Program,
        types: &TypeInterner,
        sources: &Arc<Vec<Function>>,
        checkpoint: &crate::actor_handlers::InitialPeerCheckpoint,
    ) -> Result<Completed, String> {
        self.preflight_commit(program, types, sources, checkpoint)?;
        #[cfg(test)]
        tests::observe_commit(&self, program, types, sources, checkpoint);
        // The owned mint-time capability has no public escape or other setter.
        // All cells and payloads were preflighted before this first write.
        for row in &self.rows {
            row.proof
                .0
                .canonical_producer
                .set(row.canonical.clone())
                .map_err(|_| {
                    "unpublished temporary original finalization lost its sole setter".to_owned()
                })?;
        }
        Ok(Completed {
            construction: self.construction,
            checked: self.checked,
            rows: self.rows,
        })
    }
}

/// Consume the original-only capability exactly once. Ordinary/no-delta paths
/// retain the existing late canonical producer; only a genuine completed cell
/// transition can skip it. No supplied bool or prefilled-cell fallback exists.
pub(crate) fn finish_fresh(
    program: &Program,
    types: &TypeInterner,
    construction: Option<Construction>,
    completed: Option<Completed>,
) -> Result<(), String> {
    match (construction, completed) {
        (Some(construction), None) => {
            if let Some(proof) = construction.records.first() {
                let rows = inventory(program, &construction, &proof.0.checked, types)?;
                open(&rows)?;
            } else if has_markers(program) {
                return Err(
                    "temporary original finalization imported an unregistered marker".into(),
                );
            }
            seal_original_program(program, types)
                .map_err(|errors| format!("temporary original finalization: {errors:?}"))
        }
        (None, Some(completed)) => {
            let current = inventory(program, &completed.construction, &completed.checked, types)?;
            if !same_rows(&completed.rows, &current)
                || completed.rows.iter().any(|row| {
                    !row.proof.0.canonical_producer.get().is_some_and(|value| {
                        resource_materialization::expression_equal(value, &row.canonical)
                    })
                })
            {
                return Err("temporary original completion lost its exact filled cells or canonical payloads".into());
            }
            Ok(())
        }
        _ => Err(
            "temporary original finalization has no single consumed construction capability".into(),
        ),
    }
}

#[cfg(test)]
#[path = "temporary_initial_finalization_tests.rs"]
mod tests;
