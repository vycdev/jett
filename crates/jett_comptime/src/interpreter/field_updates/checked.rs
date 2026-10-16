//! Program-bound ordinary storage adapter. No Resource provider or custody.
use super::*;
use crate::interpreter::{ExprFlow, Interpreter, Signal};
use crate::resource_execution::{
    CheckedBodyCursor, CheckedBodyReference, CheckedExecution, ExecutionPurpose,
    PreparedFieldAssignment, PreparedFieldBinding,
};
use crate::value::ClosureTypeArgument;
use jett_common::Span;
use jett_parser::ast::{AssignStmt, FunctionDef, Ident};
use jett_resolve::DefId;
use jett_typecheck::assignment_places::{
    CheckedAssignmentPlace, CheckedFieldOwner, CheckedFieldUpdateResourceRequirement,
};
use jett_typecheck::{CheckedBindingFact, CheckedResourceProgram};
use jett_types::Type;
use std::collections::HashMap;
use std::sync::Arc;

struct InstalledFieldBinding {
    checked: PreparedFieldBinding,
    name: String,
    generation: u64,
}

pub(in crate::interpreter) struct FieldUpdateContext {
    pub(in crate::interpreter) checked: CheckedExecution,
    pub(in crate::interpreter) active: bool,
    slots: Vec<HashMap<DefId, InstalledFieldBinding>>,
    next_generation: Option<u64>,
}

pub(in crate::interpreter) struct SavedFieldCursor {
    cursor: CheckedBodyCursor,
    active: bool,
}

struct FieldSlotTicket {
    scope: usize,
    fact: CheckedBindingFact,
    name: String,
    generation: u64,
    commit_generation: u64,
}

impl FieldUpdateContext {
    pub(in crate::interpreter) fn new(
        program: Arc<CheckedResourceProgram>,
        purpose: ExecutionPurpose,
        scope_depth: usize,
    ) -> Result<Option<Self>, String> {
        let checked = program.checked();
        let any = |_: &CheckedAssignmentPlace| true;
        if checked
            .intrinsic_ids
            .values()
            .all(|id| !field_reflection_intrinsic(*id))
            && !scoped_reflection(&checked.comptime_type_bindings)
            && checked
                .generic_function_instantiations
                .iter()
                .all(|instance| {
                    instance
                        .intrinsic_ids
                        .values()
                        .all(|id| !field_reflection_intrinsic(*id))
                        && !scoped_reflection(&instance.comptime_type_bindings)
                })
            && checked.actor_handlers.is_empty()
            && checked.assignment_places.is_empty()
            && !scoped_place_matches(&checked.comptime_type_bindings, &any)
            && !checked
                .generic_function_instantiations
                .iter()
                .any(|instance| {
                    !instance.assignment_places.is_empty()
                        || scoped_place_matches(&instance.comptime_type_bindings, &any)
                })
        {
            return Ok(None);
        }
        Ok(Some(Self {
            checked: CheckedExecution::new(program, purpose).map_err(|error| error.to_string())?,
            active: false,
            slots: (0..scope_depth).map(|_| HashMap::new()).collect(),
            next_generation: Some(0),
        }))
    }

    pub(in crate::interpreter) fn push_scope(&mut self) {
        self.slots.push(HashMap::new());
    }

    pub(in crate::interpreter) fn pop_scope(&mut self) {
        self.slots.pop();
    }

    fn fresh_generation(&mut self) -> Result<u64, String> {
        let generation = self
            .next_generation
            .ok_or("ordinary field storage identity exhausted")?;
        self.next_generation = generation.checked_add(1);
        Ok(generation)
    }

    pub(in crate::interpreter) fn notice_write(&mut self, scope: usize, name: &str) {
        let Some(bindings) = self.slots.get(scope) else {
            return;
        };
        if !bindings.values().any(|binding| binding.name == name) {
            return;
        }
        let Ok(generation) = self.fresh_generation() else {
            // Setters predate fallible slot metadata. Exhaustion invalidates all
            // prepared identities and must never wrap or admit an ABA match.
            self.slots.iter_mut().for_each(HashMap::clear);
            return;
        };
        if let Some(bindings) = self.slots.get_mut(scope) {
            for binding in bindings.values_mut().filter(|binding| binding.name == name) {
                binding.generation = generation;
            }
        }
    }

    pub(in crate::interpreter) fn replace_purpose(&mut self, purpose: ExecutionPurpose) {
        self.checked.replace_purpose(purpose);
    }

    fn enter(
        &mut self,
        reference: Option<&CheckedBodyReference>,
    ) -> Result<SavedFieldCursor, String> {
        let saved = SavedFieldCursor {
            cursor: self.checked.cursor(),
            active: self.active,
        };
        if let Some(reference) = reference {
            self.checked
                .install_field_function_body(reference)
                .map_err(|error| error.to_string())?;
            self.active = true;
        } else {
            self.checked.enter_ordinary();
            self.active = false;
        }
        Ok(saved)
    }

    fn restore(&mut self, saved: SavedFieldCursor) -> Result<(), String> {
        self.checked
            .restore_field_cursor(saved.cursor)
            .map_err(|error| error.to_string())?;
        self.active = saved.active;
        Ok(())
    }

    fn slot(
        &self,
        prepared: &PreparedFieldAssignment<'_>,
        floor: usize,
    ) -> Result<(usize, &InstalledFieldBinding), String> {
        let fact = prepared.place().root;
        self.slots
            .iter()
            .enumerate()
            .rev()
            .filter(|(index, _)| *index >= floor)
            .find_map(|(index, bindings)| bindings.get(&fact.definition).map(|slot| (index, slot)))
            .filter(|(_, slot)| prepared.accepts_binding(&slot.checked))
            .ok_or_else(|| "field assignment has no exact live declared storage slot".to_string())
    }
}

fn inside(span: Span, parent: Span) -> bool {
    span.file == parent.file && span.start >= parent.start && span.end <= parent.end
}

fn scoped_place_matches(
    scopes: &HashMap<Span, Vec<jett_typecheck::CheckedComptimeTypeBinding>>,
    predicate: &impl Fn(&CheckedAssignmentPlace) -> bool,
) -> bool {
    scopes.values().flatten().any(|scope| {
        scope.body.assignment_places.values().any(predicate)
            || scoped_place_matches(&scope.body.comptime_type_bindings, predicate)
    })
}

fn field_reflection_intrinsic(id: jett_intrinsics::IntrinsicId) -> bool {
    matches!(
        id,
        jett_intrinsics::IntrinsicId::TypeFields
            | jett_intrinsics::IntrinsicId::TypeFieldValue
            | jett_intrinsics::IntrinsicId::TypeConstructStart
            | jett_intrinsics::IntrinsicId::TypeConstructPut
            | jett_intrinsics::IntrinsicId::TypeConstructFinish
    )
}

fn scoped_reflection(
    scopes: &HashMap<Span, Vec<jett_typecheck::CheckedComptimeTypeBinding>>,
) -> bool {
    scopes.values().flatten().any(|scope| {
        scope
            .body
            .intrinsic_ids
            .values()
            .any(|id| field_reflection_intrinsic(*id))
            || scoped_reflection(&scope.body.comptime_type_bindings)
    })
}

impl Interpreter {
    pub(in crate::interpreter) fn checked_field_intrinsic_owner(
        &self,
        callee: &jett_parser::ast::Expr,
        types: &[jett_parser::ast::TypeExpr],
        arguments: &[jett_parser::ast::CallArg],
        span: Span,
    ) -> Result<Option<jett_parser::ast::TypeExpr>, String> {
        let Some(context) = self
            .field_update_context
            .as_ref()
            .filter(|context| context.active)
        else {
            return Ok(None);
        };
        let owner = context
            .checked
            .field_intrinsic_owner(callee, types, arguments, span)
            .map_err(|error| error.to_string())?;
        self.canonical_field_intrinsic_owner(context, owner)
    }

    pub(in crate::interpreter) fn checked_field_pipeline_intrinsic_owner(
        &self,
        step: &jett_parser::ast::PipelineStep,
    ) -> Result<Option<jett_parser::ast::TypeExpr>, String> {
        let Some(context) = self
            .field_update_context
            .as_ref()
            .filter(|context| context.active)
        else {
            return Ok(None);
        };
        let owner = context
            .checked
            .field_pipeline_intrinsic_owner(step)
            .map_err(|error| error.to_string())?;
        self.canonical_field_intrinsic_owner(context, owner)
    }

    fn canonical_field_intrinsic_owner(
        &self,
        context: &FieldUpdateContext,
        owner: Option<jett_types::TypeId>,
    ) -> Result<Option<jett_parser::ast::TypeExpr>, String> {
        let Some(owner) = owner else {
            return Ok(None);
        };
        let checked = context.checked.program().checked();
        let name = checked.interner.type_name(owner);
        let fields = checked
            .reflection_metadata
            .get_type_fields_for_id(owner)
            .ok_or("checked field intrinsic owner has no canonical field metadata")?;
        let expected_kind = match checked.interner.resolve(owner) {
            Type::Struct(_) => "struct",
            Type::Bitfield(_) => "bitfield",
            _ => return Err("checked field intrinsic owner is not a struct or bitfield".into()),
        };
        let metadata = self
            .reflection_metadata
            .as_ref()
            .ok_or("checked field intrinsic owner has no installed reflection metadata")?;
        if metadata.get_type_fields(&name) != Some(fields)
            || metadata
                .get_type_info(&name)
                .is_none_or(|info| info.kind != expected_kind)
        {
            return Err(
                "checked field intrinsic owner has conflicting canonical reflection metadata"
                    .into(),
            );
        }
        Self::debug_type(&name)
            .map(Some)
            .ok_or("checked field intrinsic owner has no canonical interpreter type".into())
    }

    pub(in crate::interpreter) fn prepare_checked_field_inline(
        &self,
        source: &Value,
    ) -> Result<Option<CheckedBodyReference>, String> {
        let Some(context) = &self.field_update_context else {
            return Ok(None);
        };
        let Value::Function {
            body, type_context, ..
        } = source
        else {
            return Ok(None);
        };
        let program = context.checked.program();
        let checked = program.checked();
        let relevant = |place: &CheckedAssignmentPlace| inside(place.assignment_span, body.span);
        let needed = checked.assignment_places.values().any(relevant)
            || scoped_place_matches(&checked.comptime_type_bindings, &relevant)
            || checked
                .generic_function_instantiations
                .iter()
                .any(|instance| {
                    instance.assignment_places.values().any(relevant)
                        || scoped_place_matches(&instance.comptime_type_bindings, &relevant)
                });
        if !context.active {
            return if needed {
                Err("inline field update requires its checked Source caller".to_string())
            } else {
                Ok(None)
            };
        }
        let proof = crate::value::closure_capture::field_invocation_proof(source, program);
        let Some(proof) = proof else {
            return if needed {
                Err("inline field update has no original capture witness".to_string())
            } else {
                Ok(None)
            };
        };
        match context
            .checked
            .prepare_field_inline_body(&proof, type_context)
        {
            Ok(reference) => Ok(Some(reference)),
            Err(error) if needed => Err(error.to_string()),
            Err(_) => Ok(None),
        }
    }
    pub(in crate::interpreter) fn prepare_checked_field_reflection_loop(
        &self,
        source: &jett_parser::ast::ForStmt,
    ) -> Result<Option<Arc<crate::resource_execution::PreparedReflectedFieldLoop>>, String> {
        let Some(context) = self
            .field_update_context
            .as_ref()
            .filter(|context| context.active)
        else {
            return Ok(None);
        };
        context
            .checked
            .prepare_field_reflected_loop(source)
            .map_err(|error| error.to_string())
    }

    pub(in crate::interpreter) fn checked_field_scope_body(
        &self,
        original: &jett_parser::ast::ComptimeTypeBindStmt,
    ) -> Result<
        Option<(
            CheckedBodyReference,
            Arc<crate::checked_types::CheckedScopedTypes>,
            jett_types::ReflectionTypeInfo,
        )>,
        String,
    > {
        let Some(context) = self
            .field_update_context
            .as_ref()
            .filter(|context| context.active)
        else {
            return Ok(None);
        };
        let needed = context
            .checked
            .field_scope_has_updates(original.span)
            .map_err(|error| error.to_string())?;
        // A genuine checked scope remains a checked caller even if its writes
        // occur in a separately declared callee or a returned inline function.
        let attempt = (|| {
            let prepared = match &original.value {
                jett_parser::ast::Expr::FieldAccess(base, member, _)
                    if member.name == "type_info" =>
                {
                    let jett_parser::ast::Expr::Ident(variable) = base.as_ref() else {
                        return Err(
                            "field update scope has no original reflected variable".to_string()
                        );
                    };
                    let definition = context
                        .checked
                        .program()
                        .resolved()
                        .resolutions
                        .get(&variable.span)
                        .ok_or("field update scope has no resolved reflected variable")?;
                    let iteration = self
                        .checked_reflected_fields
                        .iter()
                        .rev()
                        .find(|proof| proof.variable() == *definition)
                        .ok_or("field update scope has no original reflected iteration")?;
                    let current = self
                        .get_variable(&variable.name)
                        .ok_or("field update scope lost its live reflected variable")?;
                    iteration
                        .validate_value(current)
                        .map_err(|error| error.to_string())?;
                    context
                        .checked
                        .prepare_field_reflected_scope(original, iteration)
                }
                _ => context.checked.prepare_field_direct_scope(original),
            }
            .map_err(|error| error.to_string())?;
            Ok::<_, String>((
                prepared.field_reference(),
                prepared.projection(),
                prepared.reflection().clone(),
            ))
        })();
        match attempt {
            Ok(prepared) => Ok(Some(prepared)),
            Err(error) if needed => Err(error),
            Err(_) => Ok(None),
        }
    }
    pub(in crate::interpreter) fn checked_field_test_body(
        &self,
        original: &jett_parser::ast::Block,
        purpose: ExecutionPurpose,
    ) -> Result<Option<CheckedBodyReference>, String> {
        self.field_update_context
            .as_ref()
            .map(|context| {
                context
                    .checked
                    .prepare_field_test_body(original, purpose)
                    .map_err(|error| error.to_string())
            })
            .transpose()
    }

    pub(in crate::interpreter) fn with_checked_field_region<T>(
        &mut self,
        reference: &CheckedBodyReference,
        body: impl FnOnce(&mut Self) -> Result<T, String>,
    ) -> Result<T, String> {
        let saved = self.enter_checked_field_body(Some(reference))?;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(self)));
        let restore = self.restore_checked_field_body(saved);
        match result {
            Err(panic) => std::panic::resume_unwind(panic),
            Ok(Err(error)) => Err(error),
            Ok(Ok(value)) => restore.map(|()| value),
        }
    }
    pub(in crate::interpreter) fn prepare_checked_field_function(
        &self,
        name: &str,
        namespace: &Option<String>,
        current: &FunctionDef,
        projection: Option<&crate::checked_types::CheckedFunctionTypes>,
        arguments: &[ClosureTypeArgument],
        checked_call: bool,
    ) -> Result<Option<CheckedBodyReference>, String> {
        let Some(context) = &self.field_update_context else {
            return Ok(None);
        };
        let program = context.checked.program();
        let checked = program.checked();
        let relevant =
            |place: &CheckedAssignmentPlace| inside(place.assignment_span, current.body.span);
        let needed = checked.assignment_places.values().any(relevant)
            || scoped_place_matches(&checked.comptime_type_bindings, &relevant)
            || checked
                .generic_function_instantiations
                .iter()
                .any(|instance| {
                    instance.assignment_places.values().any(relevant)
                        || scoped_place_matches(&instance.comptime_type_bindings, &relevant)
                });
        if !checked_call
            || !(context.active || self.field_update_entry_allowed)
            || !self.checked_expression_types.as_ref().is_some_and(|types| {
                types
                    .resource_program
                    .as_ref()
                    .is_some_and(|installed| Arc::ptr_eq(installed, program))
                    && crate::value::closure_capture::original_root_types(program, types)
            })
        {
            return if needed {
                Err("field assignment requires its retained checked Source invocation".to_string())
            } else {
                Ok(None)
            };
        }
        match context
            .checked
            .prepare_field_function_body(name, namespace, current, projection, arguments)
        {
            Ok(reference) => Ok(Some(reference)),
            Err(error) if needed => Err(error.to_string()),
            Err(_) => Ok(None), // Legacy bodies cannot pass field invocation authority onward.
        }
    }

    pub(in crate::interpreter) fn enter_checked_field_body(
        &mut self,
        reference: Option<&CheckedBodyReference>,
    ) -> Result<Option<SavedFieldCursor>, String> {
        self.field_update_entry_allowed = false;
        self.field_update_context
            .as_mut()
            .map(|context| context.enter(reference))
            .transpose()
    }

    pub(in crate::interpreter) fn restore_checked_field_body(
        &mut self,
        saved: Option<SavedFieldCursor>,
    ) -> Result<(), String> {
        match (self.field_update_context.as_mut(), saved) {
            (Some(context), Some(saved)) => context.restore(saved),
            (None, None) => Ok(()),
            _ => Err("ordinary field context lost its checked program".to_string()),
        }
    }

    pub(in crate::interpreter) fn install_checked_field_binding(
        &mut self,
        name: &Ident,
    ) -> Result<(), String> {
        let Some(context) = &mut self.field_update_context else {
            return Ok(());
        };
        if !context.active
            || !context
                .checked
                .field_binding_needed(name.span)
                .map_err(|error| error.to_string())?
        {
            return Ok(());
        }
        let checked = context
            .checked
            .prepare_field_binding(name)
            .map_err(|error| error.to_string())?;
        let generation = context.fresh_generation()?;
        let fact = checked.fact();
        let scope = context
            .slots
            .last_mut()
            .ok_or("ordinary field storage has no lexical scope")?;
        scope.retain(|_, binding| binding.name != name.name);
        scope.insert(
            fact.definition,
            InstalledFieldBinding {
                checked,
                name: name.name.clone(),
                generation,
            },
        );
        Ok(())
    }

    fn checked_field_plan(
        &self,
        place: &CheckedAssignmentPlace,
        root: &Value,
    ) -> Result<FieldStoragePlan, String> {
        let context = self
            .field_update_context
            .as_ref()
            .ok_or("field assignment has no checked context")?;
        let types = &context.checked.program().checked().interner;
        let mut current = root;
        let mut projections = Vec::new();
        projections
            .try_reserve_exact(place.projections.len())
            .map_err(|_| "ordinary field path allocation failed")?;
        for (index, checked) in place.projections.iter().enumerate() {
            let original_owner = current;
            let nominal = match types.resolve(checked.owner_type) {
                Type::Secret(inner) => *inner,
                _ => checked.owner_type,
            };
            let canonical = types.type_name(nominal);
            let mut wrappers = Vec::new();
            while let Value::Typed { type_name, value } = current {
                let base = self.primitive_base_type_name(type_name);
                if self.type_name_has_refinement(type_name)
                    || base
                        .split_once(" at ")
                        .map_or(base.as_ref(), |(owner, _)| owner)
                        != canonical
                            .split_once(" at ")
                            .map_or(canonical.as_str(), |(owner, _)| owner)
                {
                    return Err(
                        "field assignment retained an incompatible owner wrapper".to_string()
                    );
                }
                wrappers.push(type_name.clone());
                current = value;
            }
            if matches!(current, Value::Pending(_)) {
                let ty = Self::simple_type_expr_from_name(
                    &types.type_name(checked.owner_type),
                    checked.source_span,
                )
                .ok_or("checked field owner has no canonical interpreter type")?;
                return Err(format!(
                    "field access is not supported on {}",
                    self.format_debug_value(original_owner, Some(&ty))
                ));
            }
            let owner = match checked.owner {
                CheckedFieldOwner::Struct(id) => {
                    let definition = types.resolve_struct(id);
                    let name = definition
                        .name
                        .split('[')
                        .next()
                        .unwrap_or(&definition.name)
                        .to_string();
                    StorageOwner::Struct {
                        type_name: name,
                        concrete_type: (!types.nominal_type_arguments(nominal).is_empty())
                            .then_some(canonical),
                        field_names: definition
                            .fields
                            .iter()
                            .map(|(name, _)| name.clone())
                            .collect(),
                    }
                }
                CheckedFieldOwner::MachineState { machine, state } => {
                    let definition = types.resolve_machine(machine);
                    let state = definition
                        .state(state)
                        .ok_or("field assignment lost its exact machine state")?;
                    StorageOwner::MachineState {
                        type_name: definition.name.clone(),
                        state: state.name.clone(),
                        field_count: state.fields.len(),
                    }
                }
            };
            let projection = StorageProjection {
                wrappers,
                owner,
                field_index: checked.field_index,
            };
            // Select from the original wrapper-bearing ancestor, not the
            // temporary inner reference used to validate retained wrapper labels.
            current = projection
                .selected(original_owner, index)
                .map_err(|_| "field assignment has incompatible ordinary storage")?;
            projections.push(projection);
        }
        let plan = FieldStoragePlan { projections };
        plan.preflight(root)
            .map_err(|_| "field assignment has incompatible ordinary storage")?;
        Ok(plan)
    }

    pub(in crate::interpreter) fn exec_checked_field_assignment(
        &mut self,
        source: &AssignStmt,
    ) -> Result<Option<Signal>, String> {
        if let Some(transport) = self
            .resource_transport
            .as_ref()
            .filter(|transport| transport.checked_source_active)
        {
            let resource = transport
                .checked
                .prepare_field_assignment(source)
                .map_err(|error| error.to_string())?;
            if resource.place().resource == CheckedFieldUpdateResourceRequirement::RequiresCustody {
                return self.exec_resource_assignment(source);
            }
        }
        let context = self
            .field_update_context
            .as_ref()
            .filter(|context| context.active)
            .ok_or("field assignment has no exact checked executable body")?;
        let prepared = context
            .checked
            .prepare_field_assignment(source)
            .map_err(|error| error.to_string())?;
        if prepared.place().resource == CheckedFieldUpdateResourceRequirement::RequiresCustody {
            // Keep the existing authenticated projected-Resource refusal. Do
            // not classify a scalar leaf as permission to bypass root custody.
            return self.exec_resource_assignment(source);
        }
        let (scope, slot) = context.slot(&prepared, self.lexical_scope_floor)?;
        let mut ticket = FieldSlotTicket {
            scope,
            fact: slot.checked.fact(),
            name: slot.name.clone(),
            generation: slot.generation,
            commit_generation: 0,
        };
        let root = self
            .scopes
            .get(scope)
            .and_then(|scope| scope.get(&ticket.name))
            .ok_or("field assignment lost its declared physical root")?;
        let plan = self.checked_field_plan(prepared.place(), root)?;
        ticket.commit_generation = self
            .field_update_context
            .as_mut()
            .ok_or("field assignment lost its checked context")?
            .fresh_generation()?;
        let value = match self.eval_expr_flow(&source.value)? {
            ExprFlow::Value(value) => value,
            ExprFlow::Resource(_) => {
                return Err(
                    "Resource operand has no checked ownership transport for this operation"
                        .to_string(),
                );
            }
            // An early handler return abandons the assignment. The returned
            // root is not committed or retired a second time by this adapter.
            ExprFlow::Signal(signal) => return Ok(Some(signal)),
        };
        let field = prepared
            .place()
            .projections
            .last()
            .ok_or("checked field assignment has no projection")?;
        let context = self
            .field_update_context
            .as_ref()
            .ok_or("field assignment lost its checked context")?;
        let name = context
            .checked
            .program()
            .checked()
            .interner
            .type_name(field.field_type);
        let ty = Self::simple_type_expr_from_name(&name, field.source_span)
            .ok_or("checked field destination has no canonical interpreter type")?;
        let source_type = self
            .checked_refinement_source_type(source.value.span())
            .cloned();
        let value =
            self.normalize_and_validate_value_from_source(&ty, value, source_type.as_deref())?;
        let context = self
            .field_update_context
            .as_ref()
            .ok_or("field assignment lost its checked context")?;
        context
            .checked
            .revalidate_field_assignment(&prepared)
            .map_err(|error| error.to_string())?;
        let (scope, slot) = context.slot(&prepared, self.lexical_scope_floor)?;
        if scope != ticket.scope
            || slot.generation != ticket.generation
            || slot.checked.fact() != ticket.fact
        {
            return Err("field assignment root changed while evaluating its RHS".to_string());
        }
        let root = self
            .scopes
            .get(scope)
            .and_then(|scope| scope.get(&ticket.name))
            .ok_or("field assignment lost its declared physical root")?;
        // This repeated typed preflight is still before displacement. It also
        // shares the established typed direct-field pending error policy.
        self.checked_field_plan(prepared.place(), root)?;
        let context = self
            .field_update_context
            .as_mut()
            .ok_or("field assignment lost its checked context")?;
        let slot = context
            .slots
            .get_mut(ticket.scope)
            .and_then(|scope| scope.get_mut(&ticket.fact.definition))
            .ok_or("field assignment lost its declared storage installation")?;
        let root = self
            .scopes
            .get_mut(ticket.scope)
            .and_then(|scope| scope.get_mut(&ticket.name))
            .ok_or("field assignment lost its declared physical root")?;
        let displaced = plan
            .replace(root, StagedOrdinaryReplacement { value })
            .map_err(|_| "field assignment lost its prepared ordinary storage")?;
        slot.generation = ticket.commit_generation;
        drop(displaced);
        Ok(None)
    }
}

impl Interpreter {
    pub(in crate::interpreter) fn install_checked_actor_binding(
        &mut self,
        handler: &crate::resource_execution::PreparedActorHandler,
        role: jett_typecheck::CheckedActorBindingRole,
    ) -> Result<(), String> {
        let context = self
            .field_update_context
            .as_mut()
            .filter(|context| context.active)
            .ok_or("actor binding has no checked handler context")?;
        let checked = context
            .checked
            .prepare_actor_binding(handler, role)
            .map_err(|error| error.to_string())?;
        let name = handler.role_name(role).map_err(|error| error.to_string())?;
        let generation = context.fresh_generation()?;
        let fact = checked.fact();
        let scope = context
            .slots
            .last_mut()
            .ok_or("actor binding has no physical scope")?;
        if scope.values().any(|binding| binding.name == name.name) {
            return Err("actor captured binding repeated a declaration in one scope".to_string());
        }
        scope.insert(
            fact.definition,
            InstalledFieldBinding {
                checked,
                name: name.name.clone(),
                generation,
            },
        );
        Ok(())
    }
}
