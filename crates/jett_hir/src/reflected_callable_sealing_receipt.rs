//! Receipt of the sole actual callable sealer, including optional adapter slots.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Slot {
    Getter { field: usize },
    Adapter,
}

#[derive(Clone)]
pub(super) struct Edit {
    pub(super) owner: FunctionId,
    pub(super) expression: usize,
    pub(super) slot: Slot,
    pub(super) previous: Option<ReflectedCallablePlan>,
    pub(super) current: ReflectedCallablePlan,
}

struct Sealing<'types> {
    types: &'types TypeInterner,
    before: Program,
    output: Program,
    checked: Option<Arc<jett_typecheck::CheckedResourceProgram>>,
    source_epoch: Arc<Vec<Function>>,
    edits: Vec<Edit>,
}

/// No public constructor or stored archive back-reference. This receipt lives
/// only across the real producer's exact peer-renewal boundary.
pub(crate) struct CheckedCallableSealing<'types>(Box<Sealing<'types>>);

fn metadata_equal(left: &Program, right: &Program) -> bool {
    let Program {
        resource_manifest,
        resource_source,
        equality_methods,
        functions: _,
    } = left;
    let Program {
        resource_manifest: other_manifest,
        resource_source: other_source,
        equality_methods: other_equality,
        functions: _,
    } = right;
    resource_manifest == other_manifest
        && resource_source == other_source
        && equality_methods == other_equality
}
fn same_program(left: &Program, right: &Program) -> bool {
    metadata_equal(left, right)
        && crate::resource_materialization::functions_equal(&left.functions, &right.functions)
}

fn replacement(
    edits: &[Edit],
    used: &mut [bool],
    owner: FunctionId,
    expression: usize,
    slot: Slot,
    previous: Option<&ReflectedCallablePlan>,
) -> Result<Option<ReflectedCallablePlan>, String> {
    let selected = edits
        .iter()
        .enumerate()
        .filter(|(_, edit)| {
            edit.owner == owner && edit.expression == expression && edit.slot == slot
        })
        .collect::<Vec<_>>();
    let [(index, edit)] = selected.as_slice() else {
        return if selected.is_empty() {
            Ok(None)
        } else {
            Err("callable sealing repeated an exact original token slot".into())
        };
    };
    if used[*index] || edit.previous.as_ref() != previous {
        return Err("callable sealing changed an exact previous token".into());
    }
    used[*index] = true;
    Ok(Some(edit.current.clone()))
}

fn replay_edits(before: &Program, edits: &[Edit]) -> Result<Program, String> {
    let mut replay = before.clone();
    let mut used = vec![false; edits.len()];
    for function in &mut replay.functions {
        let owner = function.id;
        let mut ordinal = 0usize;
        crate::resource_materialization::walk_block(&mut function.body, &mut |value| {
            let expression = ordinal;
            ordinal += 1;
            match &mut value.kind {
                ExpressionKind::Intrinsic {
                    field_validation: Some(ReflectedFieldValidation::Validate(fields)),
                    ..
                } => {
                    for (field, plan) in fields.iter_mut().enumerate() {
                        if let ReflectedFieldAction::CallableReturn(callable) = &mut plan.action {
                            if let Some(current) = replacement(
                                edits,
                                &mut used,
                                owner,
                                expression,
                                Slot::Getter { field },
                                Some(callable),
                            )? {
                                *callable = current;
                            }
                        }
                    }
                }
                ExpressionKind::Handle {
                    kind: HandleKind::ReflectedReturn { callable, .. },
                    ..
                } => {
                    if let Some(current) = replacement(
                        edits,
                        &mut used,
                        owner,
                        expression,
                        Slot::Adapter,
                        callable.as_ref(),
                    )? {
                        *callable = Some(current);
                    }
                }
                _ => {}
            }
            Ok(())
        })?;
    }
    if used.iter().any(|used| !used) {
        return Err("callable sealing lost its exact original getter or adapter occurrence".into());
    }
    Ok(replay)
}

impl CheckedCallableSealing<'_> {
    pub(crate) fn authenticate_output(
        &self,
        before: &Program,
        output: &Program,
        types: &TypeInterner,
    ) -> Result<(), String> {
        if !std::ptr::eq(types, self.0.types)
            || !same_program(before, &self.0.before)
            || !same_program(output, &self.0.output)
            || !crate::resource_materialization::functions_equal(
                &self.0.source_epoch,
                &before.functions,
            )
            || self.0.checked.as_ref().is_some_and(|checked| {
                before
                    .resource_source
                    .checked_program()
                    .is_some_and(|original| !Arc::ptr_eq(original, checked))
            })
        {
            return Err("callable sealing receipt has another complete input, output, archive or type arena".into());
        }
        Ok(())
    }
    pub(crate) fn replay(&self, before: &Program, types: &TypeInterner) -> Result<Program, String> {
        self.authenticate_output(before, &self.0.output, types)?;
        Ok(self.0.output.clone())
    }
    pub(crate) fn before(&self) -> &Program {
        &self.0.before
    }
    pub(crate) fn output(&self) -> &Program {
        &self.0.output
    }
    pub(crate) fn source_epoch(&self) -> &Arc<Vec<Function>> {
        &self.0.source_epoch
    }
    pub(crate) fn checked(&self) -> Option<&Arc<jett_typecheck::CheckedResourceProgram>> {
        self.0.checked.as_ref()
    }
    pub(super) fn token_pairs(&self) -> Vec<(ReflectedCallablePlan, ReflectedCallablePlan)> {
        self.0
            .edits
            .iter()
            .filter_map(|edit| {
                edit.previous
                    .as_ref()
                    .map(|previous| (previous.clone(), edit.current.clone()))
            })
            .collect()
    }
}

fn bound_plan_relation(previous: &ReflectedCallablePlan, current: &ReflectedCallablePlan) -> bool {
    let Plan {
        checked,
        admission,
        returned,
        span,
        function,
        context,
        seal,
        materialized,
        ordinal,
        reference,
        graph_source,
    } = previous.data.as_ref();
    let Plan {
        checked: other_checked,
        admission: other_admission,
        returned: other_returned,
        span: other_span,
        function: other_function,
        context: other_context,
        seal: other_seal,
        materialized: other_materialized,
        ordinal: other_ordinal,
        reference: other_reference,
        graph_source: other_graph_source,
    } = current.data.as_ref();
    let same_reference = match (reference, other_reference) {
        (Some(left), Some(right)) => Arc::ptr_eq(left, right),
        (None, None) => true,
        _ => false,
    };
    let same_graph_source = match (graph_source, other_graph_source) {
        (Some(left), Some(right)) => left.same_source(right),
        (None, None) => true,
        _ => false,
    };
    seal.is_none()
        && other_seal.is_some()
        && Arc::ptr_eq(checked, other_checked)
        && admission == other_admission
        && returned == other_returned
        && span == other_span
        && function == other_function
        && context == other_context
        && materialized == other_materialized
        && ordinal == other_ordinal
        && same_reference
        && same_graph_source
}

/// Called only inside the real sealer after its actual token insertions and
/// before any archive peer changes. A consumer cannot nominate a replacement table.
pub(super) fn produce<'types>(
    before: &Program,
    output: &Program,
    types: &'types TypeInterner,
    source_epoch: Arc<Vec<Function>>,
    edits: Vec<Edit>,
) -> Result<CheckedCallableSealing<'types>, String> {
    if !metadata_equal(before, output)
        || !crate::resource_materialization::functions_equal(&source_epoch, &before.functions)
        || !same_program(&replay_edits(before, &edits)?, output)
    {
        return Err(
            "actual callable sealing changed outside its exact recorded token edits".into(),
        );
    }
    let mut checked = before.resource_source.checked_program().cloned();
    for edit in &edits {
        let current = &edit.current;
        if !current.is_sealed()
            || !current.validate_original()
            || current.validate_types(types).is_err()
        {
            return Err("actual callable sealing produced an invalid current token".into());
        }
        let selected = checked.get_or_insert_with(|| current.data.checked.clone());
        if !Arc::ptr_eq(selected, &current.data.checked) {
            return Err("actual callable sealing mixed checked Source sessions".into());
        }
        if let Some(previous) = &edit.previous {
            if !bound_plan_relation(previous, current) {
                return Err("actual callable sealing changed its bound token relation".into());
            }
        } else if edit.slot != Slot::Adapter
            || current.function() != edit.owner
            || edits
                .iter()
                .filter(|getter| {
                    matches!(getter.slot, Slot::Getter { .. }) && getter.current == *current
                })
                .count()
                != 1
        {
            return Err(
                "initial callable adapter insertion lacks its actual paired getter token".into(),
            );
        }
    }
    if let Some(checked) = &checked {
        if before.resource_source.checked_program().is_some() {
            // The execution view can still be a protected pre-seal peer (for
            // example Graph). Its renewal remains owned by that producer. This
            // receipt binds the exact real input/output tables and type meanings;
            // each consuming peer authenticates its own current view.
            before.resource_source.validate_types(types)?;
        }
        if edits
            .iter()
            .any(|edit| !Arc::ptr_eq(checked, &edit.current.data.checked))
        {
            return Err("actual callable sealing lost its exact checked program".into());
        }
    }
    Ok(CheckedCallableSealing(Box::new(Sealing {
        types,
        before: before.clone(),
        output: output.clone(),
        checked,
        source_epoch,
        edits,
    })))
}

#[cfg(test)]
#[path = "callable_sealing_execution_tests.rs"]
mod tests;
