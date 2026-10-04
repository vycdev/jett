//! Metadata restoration at the checked top-level entry boundary.
//! This snapshot contains no Value, Environment or cleanup ticket.
use super::*;
use crate::resource_execution::CheckedBodyCursor;

pub(super) struct SavedResourceEntryContext {
    program: Arc<jett_typecheck::CheckedResourceProgram>,
    retained_purpose: ExecutionPurpose,
    checked_purpose: ExecutionPurpose,
    source_active: bool,
    cursor: CheckedBodyCursor,
    scope_depth: usize,
    variable_depth: usize,
    alias_depth: usize,
    namespace: Option<String>,
    trusted: bool,
    floor: usize,
    proofs: bool,
    checked_function: Option<Arc<CheckedFunctionTypes>>,
    checked_scope: Option<Arc<CheckedScopedTypes>>,
    arguments: Vec<TypeExpr>,
    type_scopes: Vec<HashMap<String, TypeExpr>>,
    scoped_bindings: Vec<ClosureScopedTypeBinding>,
    fields: Vec<HashMap<String, ReflectedFieldBinding>>,
    type_infos: Vec<HashMap<String, ReflectedTypeInfoBinding>>,
    variants: Vec<HashMap<String, ReflectedVariantBinding>>,
    states: Vec<HashMap<String, ReflectedMachineStateBinding>>,
}

impl SavedResourceEntryContext {
    pub(super) fn capture(interpreter: &Interpreter) -> Result<Self, String> {
        let transport = interpreter
            .resource_transport
            .as_ref()
            .ok_or("missing checked Resource transport")?;
        let (program, retained_purpose) = interpreter
            .retained_resource_program
            .as_ref()
            .ok_or("checked entry has no retained program")?;
        if !Arc::ptr_eq(program, transport.checked.program()) {
            return Err("checked entry context belongs to another retained program".to_string());
        }
        Ok(Self {
            program: program.clone(),
            retained_purpose: *retained_purpose,
            checked_purpose: transport.checked.purpose(),
            source_active: transport.checked_source_active,
            cursor: transport.cursor(),
            scope_depth: interpreter.scopes.len(),
            variable_depth: interpreter.variable_type_scopes.len(),
            alias_depth: interpreter.namespace_alias_scopes.len(),
            namespace: interpreter.current_namespace.clone(),
            trusted: interpreter.current_function_trusted_stdlib,
            floor: interpreter.lexical_scope_floor,
            proofs: interpreter.allow_checked_refinement_proofs,
            checked_function: interpreter.active_checked_function.clone(),
            checked_scope: interpreter.active_checked_scope.clone(),
            arguments: interpreter.current_type_arguments.clone(),
            type_scopes: interpreter.type_arg_scopes.clone(),
            scoped_bindings: interpreter.scoped_type_bindings.clone(),
            fields: interpreter.reflected_field_scopes.clone(),
            type_infos: interpreter.reflected_type_info_scopes.clone(),
            variants: interpreter.reflected_variant_scopes.clone(),
            states: interpreter.reflected_machine_state_scopes.clone(),
        })
    }

    pub(super) fn scope_depth(&self) -> usize {
        self.scope_depth
    }

    pub(super) fn restore(self, interpreter: &mut Interpreter) -> Result<(), String> {
        let underflow = interpreter.scopes.len() < self.scope_depth
            || interpreter.variable_type_scopes.len() < self.variable_depth
            || interpreter.namespace_alias_scopes.len() < self.alias_depth;
        interpreter.scopes.truncate(self.scope_depth);
        interpreter
            .variable_type_scopes
            .truncate(self.variable_depth);
        interpreter
            .namespace_alias_scopes
            .truncate(self.alias_depth);
        interpreter.current_namespace = self.namespace;
        interpreter.current_function_trusted_stdlib = self.trusted;
        interpreter.lexical_scope_floor = self.floor;
        interpreter.allow_checked_refinement_proofs = self.proofs;
        interpreter.active_checked_function = self.checked_function;
        interpreter.active_checked_scope = self.checked_scope;
        interpreter.current_type_arguments = self.arguments;
        interpreter.type_arg_scopes = self.type_scopes;
        interpreter.scoped_type_bindings = self.scoped_bindings;
        interpreter.reflected_field_scopes = self.fields;
        interpreter.reflected_type_info_scopes = self.type_infos;
        interpreter.reflected_variant_scopes = self.variants;
        interpreter.reflected_machine_state_scopes = self.states;
        let retained = match interpreter.retained_resource_program.as_mut() {
            Some((program, purpose)) if Arc::ptr_eq(program, &self.program) => {
                *purpose = self.retained_purpose;
                Ok(())
            }
            _ => Err("checked entry lost its exact retained program".to_string()),
        };
        let cursor = match interpreter.resource_transport.as_mut() {
            Some(transport) if Arc::ptr_eq(transport.checked.program(), &self.program) => {
                transport.checked_source_active = self.source_active;
                transport.checked.replace_purpose(self.checked_purpose);
                transport
                    .checked
                    .restore_cursor(self.cursor)
                    .map_err(|error| error.to_string())
            }
            _ => Err("checked entry lost its exact Resource transport".to_string()),
        };
        if underflow {
            return Err("checked entry removed a preexisting interpreter scope".to_string());
        }
        retained.and(cursor)
    }
}

#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct EntryContextObservation {
    namespace: Option<String>,
    trusted: bool,
    floor: usize,
    proofs: bool,
    scope_depths: [usize; 3],
    metadata_depths: [usize; 7],
    checked_function: bool,
    checked_scope: bool,
    source_active: bool,
    purpose: ExecutionPurpose,
    original_scope_depth: Option<usize>,
}

#[cfg(test)]
impl Interpreter {
    pub(crate) fn resource_test_entry_context(&self) -> Result<EntryContextObservation, String> {
        let transport = self
            .resource_transport
            .as_ref()
            .ok_or("missing checked Resource transport")?;
        Ok(EntryContextObservation {
            namespace: self.current_namespace.clone(),
            trusted: self.current_function_trusted_stdlib,
            floor: self.lexical_scope_floor,
            proofs: self.allow_checked_refinement_proofs,
            scope_depths: [
                self.scopes.len(),
                self.variable_type_scopes.len(),
                self.namespace_alias_scopes.len(),
            ],
            metadata_depths: [
                self.current_type_arguments.len(),
                self.type_arg_scopes.len(),
                self.scoped_type_bindings.len(),
                self.reflected_field_scopes.len(),
                self.reflected_type_info_scopes.len(),
                self.reflected_variant_scopes.len(),
                self.reflected_machine_state_scopes.len(),
            ],
            checked_function: self.active_checked_function.is_some(),
            checked_scope: self.active_checked_scope.is_some(),
            source_active: transport.checked_source_active,
            purpose: transport.checked.purpose(),
            original_scope_depth: transport.checked.original_body_scope_depth(),
        })
    }
}
