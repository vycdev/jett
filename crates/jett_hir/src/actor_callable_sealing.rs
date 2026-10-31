//! Initial actor snapshots composed with the sole actual callable-sealer receipt.
//! Raw predecessor and pre-attachment output tables never retain their own edge.
use super::*;
use crate::reflected_callable::CheckedCallableSealing;

#[derive(Debug)]
pub(super) struct Epoch {
    checked: Arc<jett_typecheck::CheckedResourceProgram>,
    sources: Arc<Vec<Function>>,
    after: Program,
    affected: Vec<FunctionId>,
}

/// An actor transition derived from the real shared token output, not a second
/// token or peer producer. Only the actual original Lowerer may finish it.
pub(crate) struct InitialCallableRenewal {
    checked: Arc<jett_typecheck::CheckedResourceProgram>,
    sources: Arc<Vec<Function>>,
    output: Program,
    affected: Vec<FunctionId>,
    temporary: Option<crate::temporary_local_views::PreparedOriginalFinalization>,
}

/// Complete peer checkpoint derived only inside the actual original finish:
/// exact retained actor stage plus the real initial sum producer.
pub(crate) struct InitialPeerCheckpoint {
    checked: Arc<jett_typecheck::CheckedResourceProgram>,
    sources: Arc<Vec<Function>>,
    output: Program,
}
impl InitialPeerCheckpoint {
    pub(crate) fn authenticate(
        &self,
        program: &Program,
        types: &TypeInterner,
        sources: &Arc<Vec<Function>>,
        checked: &Arc<jett_typecheck::CheckedResourceProgram>,
    ) -> Result<(), String> {
        if !Arc::ptr_eq(checked, &self.checked)
            || !std::ptr::eq(types, &self.checked.checked().interner)
            || !Arc::ptr_eq(sources, &self.sources)
            || !same_program(program, &self.output)
        {
            return Err(
                "temporary original finalization changed its complete actual actor/sum checkpoint"
                    .into(),
            );
        }
        Ok(())
    }
}

fn metadata_equal(left: &Program, right: &Program) -> bool {
    left.resource_source == right.resource_source
        && left.resource_manifest == right.resource_manifest
        && left.equality_methods == right.equality_methods
}
fn same_program(left: &Program, right: &Program) -> bool {
    metadata_equal(left, right)
        && crate::resource_materialization::functions_equal(&left.functions, &right.functions)
}
fn epoch(function: &Function) -> Option<&Arc<Epoch>> {
    function
        .actor_handler
        .as_ref()
        .and_then(|entry| entry.callable_epoch.as_ref())
        .or_else(|| {
            function
                .actor_state_views
                .iter()
                .find_map(|view| view.callable_epoch.as_ref())
        })
}
fn affected(before: &Program, output: &Program) -> Result<Vec<FunctionId>, String> {
    if before.functions.len() != output.functions.len() {
        return Err("actor callable sealing changed function inventory".into());
    }
    let mut changed = Vec::new();
    for (original, produced) in before.functions.iter().zip(&output.functions) {
        if original.id != produced.id
            || original.actor_handler != produced.actor_handler
            || original.actor_state_views != produced.actor_state_views
        {
            return Err(
                "actor callable sealing changed original actor entry/view inventory".into(),
            );
        }
        if Snapshot::of(original) != Snapshot::of(produced)
            && (original.actor_handler.is_some() || !original.actor_state_views.is_empty())
        {
            changed.push(original.id);
        }
    }
    Ok(changed)
}
fn renew_snapshots(protected: &mut Program, changed: &[FunctionId]) -> Result<(), String> {
    for id in changed {
        let function = protected
            .functions
            .get_mut(id.index() as usize)
            .filter(|f| f.id == *id)
            .ok_or("actor callable replay lost the protected original function")?;
        let snapshot = Snapshot::of(function);
        if let Some(entry) = &mut function.actor_handler {
            entry.snapshot = Some(Box::new(snapshot.clone()));
        }
        for view in &mut function.actor_state_views {
            view.constructor = Some(Box::new(snapshot.clone()));
        }
    }
    super::validate_program(protected)
        .map_err(|errors| format!("actor callable produced snapshots: {errors:?}"))
}

pub(crate) fn renew(
    program: &mut Program,
    before: &Program,
    types: &TypeInterner,
    receipt: &CheckedCallableSealing<'_>,
    original: Option<&Arc<jett_typecheck::CheckedResourceProgram>>,
    construction: &mut Option<crate::temporary_local_views::Construction>,
) -> Result<Option<InitialCallableRenewal>, String> {
    receipt.authenticate_output(before, program, types)?;
    super::validate_program(before)
        .map_err(|errors| format!("actor callable original snapshots: {errors:?}"))?;
    let mut protected = receipt.replay(before, types)?;
    let changed = affected(before, &protected)?;
    if changed.is_empty() {
        return Ok(None);
    }
    let checked = original.ok_or(
        "actor-changing callable sealing has no represented original-Lowerer epoch; later actor sealing remains unsupported"
    )?;
    if !receipt
        .checked()
        .is_some_and(|owner| Arc::ptr_eq(owner, checked))
        || !std::ptr::eq(types, &checked.checked().interner)
        || before.resource_source != ResourceSourceArchive::empty()
        || before
            .functions
            .iter()
            .any(|function| epoch(function).is_some())
    {
        return Err(
            "actor callable sealing lost its exact original checked context, arena or archive"
                .into(),
        );
    }
    let temporary = if crate::temporary_local_views::has_markers(before) {
        let fresh = construction.take().ok_or(
            "initial actor callable sealing with temporary-owner finalization remains unsupported without its fresh original construction"
        )?;
        Some(crate::temporary_local_views::prepare_original_finalization(
            before, program, types, receipt, checked, fresh,
        )?)
    } else {
        None
    };
    renew_snapshots(&mut protected, &changed)?;
    *program = protected.clone();
    Ok(Some(InitialCallableRenewal {
        checked: checked.clone(),
        sources: receipt.source_epoch().clone(),
        output: protected,
        affected: changed,
        temporary,
    }))
}

fn preflight_attachment(program: &Program, affected: &[FunctionId]) -> Result<(), String> {
    for (index, id) in affected.iter().enumerate() {
        let function = program
            .functions
            .get(id.index() as usize)
            .filter(|f| f.id == *id)
            .ok_or("actor callable epoch lost an affected function")?;
        if affected[..index].contains(id)
            || (function.actor_handler.is_none() && function.actor_state_views.is_empty())
            || function
                .actor_handler
                .as_ref()
                .is_some_and(|entry| entry.callable_epoch.is_some())
            || function
                .actor_state_views
                .iter()
                .any(|view| view.callable_epoch.is_some())
        {
            return Err(
                "actor callable attachment has repeated or foreign original epoch inventory".into(),
            );
        }
    }
    Ok(())
}

fn attach(program: &mut Program, edge: &Arc<Epoch>) -> Result<(), String> {
    preflight_attachment(program, &edge.affected)?;
    for id in &edge.affected {
        let function = program
            .functions
            .get_mut(id.index() as usize)
            .filter(|f| f.id == *id)
            .ok_or("actor callable epoch lost an affected function")?;
        if let Some(entry) = &mut function.actor_handler {
            if entry.callable_epoch.is_some() {
                return Err("repeated actor callable epochs are not represented".into());
            }
            entry.callable_epoch = Some(edge.clone());
        }
        for view in &mut function.actor_state_views {
            if view.callable_epoch.is_some() {
                return Err(
                    "repeated actor constructor callable epochs are not represented".into(),
                );
            }
            view.callable_epoch = Some(edge.clone());
        }
    }
    Ok(())
}

/// Fixed initial finalization, after existing peers checked the same before
/// archive. Authenticate the complete retained actor stage, then run the genuine
/// initial sum producer once on it. No caller supplies an expected final table.
pub(crate) fn finish_original(
    program: &mut Program,
    types: &TypeInterner,
    renewal: InitialCallableRenewal,
) -> Result<Option<crate::temporary_local_views::CompletedOriginalFinalization>, String> {
    if !std::ptr::eq(types, &renewal.checked.checked().interner)
        || renewal.output.resource_source != ResourceSourceArchive::empty()
        || !same_program(program, &renewal.output)
    {
        return Err(
            "initial actor callable peers changed their protected complete checkpoint".into(),
        );
    }
    let mut completed = renewal.output.clone();
    crate::mutable_sum_backings::seal_program(&mut completed, types)
        .map_err(|errors| format!("initial actor callable sum sealing: {errors:?}"))?;
    super::validate_program(&completed)
        .map_err(|errors| format!("initial actor callable snapshots: {errors:?}"))?;
    crate::mutable_sum_backings::validate_program(&completed)
        .map_err(|errors| format!("initial actor callable sum receipts: {errors:?}"))?;
    preflight_attachment(&completed, &renewal.affected)?;
    // Finalize only immutable BEFORE payloads, after complete peer preflight
    // and before the retained after table exists. Ordinary APIs have no capability.
    let checkpoint = InitialPeerCheckpoint {
        checked: renewal.checked.clone(),
        sources: renewal.sources.clone(),
        output: completed.clone(),
    };
    let temporary = match renewal.temporary {
        Some(prepared) => {
            Some(prepared.commit(&completed, types, &renewal.sources, &checkpoint)?)
        }
        None => None,
    };
    let edge = Arc::new(Epoch {
        checked: renewal.checked.clone(),
        sources: renewal.sources.clone(),
        after: completed.clone(),
        affected: renewal.affected.clone(),
    });
    attach(&mut completed, &edge)?;
    super::validate_program(&completed)
        .map_err(|errors| format!("initial actor callable attached snapshots: {errors:?}"))?;
    *program = completed;
    Ok(temporary)
}

/// Non-executable actor metadata at one exact private Source checkpoint.
/// Existing protected restoration has already selected the represented epoch;
/// this function never searches arbitrary ancestors or copies historical bodies.
fn source_view(
    sources: &Arc<Vec<Function>>,
    program: &Program,
    restored: &[Function],
    checked: &Arc<jett_typecheck::CheckedResourceProgram>,
) -> Result<Vec<Function>, &'static str> {
    super::validate_program(program).map_err(|_| "actor callable current snapshots changed")?;
    crate::mutable_sum_backings::validate_program(program)
        .map_err(|_| "actor callable current sum receipts changed")?;
    if program.resource_source.checked_program().is_some() {
        if !program.resource_source.belongs_to(checked) {
            return Err("actor callable Source epoch belongs to another checked program");
        }
        program
            .resource_source
            .validate_current(program, &checked.checked().interner)
            .map_err(
                |_| "actor callable current table has no exact authenticated peer checkpoint",
            )?;
    } else if program.resource_source != ResourceSourceArchive::empty() {
        return Err("actor callable initial Source epoch acquired foreign archive views");
    }
    let mut matching = Vec::<Arc<Epoch>>::new();
    for function in restored {
        if let Some(edge) = epoch(function) {
            if Arc::ptr_eq(&edge.sources, sources)
                && !matching.iter().any(|old| Arc::ptr_eq(old, edge))
            {
                matching.push(edge.clone());
            }
        }
    }
    let [edge] = matching.as_slice() else {
        return Err("actor callable Source comparison has no exact represented epoch");
    };
    if !Arc::ptr_eq(&edge.checked, checked)
        || edge.after.resource_manifest != program.resource_manifest
        || edge.after.equality_methods != program.equality_methods
    {
        return Err("actor callable epoch changed original session, manifest or equality table");
    }
    let mut expected = edge.after.clone();
    attach(&mut expected, edge)
        .map_err(|_| "actor callable epoch contains its own edge or repeated history")?;
    if !crate::resource_materialization::functions_equal(&expected.functions, restored) {
        return Err("actor callable Source epoch has no exact canonical/bake restoration path");
    }
    let mut view = restored.to_vec();
    for id in &edge.affected {
        let historical = edge
            .sources
            .get(id.index() as usize)
            .filter(|f| f.id == *id)
            .ok_or("actor callable Source epoch lost historical actor metadata")?;
        let current = view
            .get_mut(id.index() as usize)
            .filter(|f| f.id == *id)
            .ok_or("actor callable Source epoch lost restored actor metadata")?;
        current.actor_handler = historical.actor_handler.clone();
        current.actor_state_views = historical.actor_state_views.clone();
    }
    Ok(view)
}

/// The historical table never leaves this private actor module as executable HIR.
pub(crate) fn source_matches(
    sources: &Arc<Vec<Function>>,
    program: &Program,
    restored: &[Function],
    original: &[Function],
    checked: &Arc<jett_typecheck::CheckedResourceProgram>,
) -> Result<(), &'static str> {
    let historical = source_view(sources, program, restored, checked)?;
    if historical.len() != original.len() {
        return Err("actor callable Source epoch changed the complete function inventory");
    }
    if !crate::reflected_callable::source_functions_equal(&historical, original) {
        return Err(
            "actor callable Source comparison changed outside exact historical metadata replay",
        );
    }
    Ok(())
}

#[cfg(test)]
#[path = "actor_callable_sealing_edge_tests.rs"]
mod tests;
