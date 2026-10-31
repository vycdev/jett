//! Publication admits checked inactive data, never occupied aggregate custody.
use super::super::checked::PreparedAbsentBindings;
use super::*;
use jett_parser::ast::{ForStmt, Ident, MatchStmt};

impl ResourceTransport {
    fn validate_absent_parent(
        &self,
        proof: &PreparedAbsentBindings<'_>,
        value: &Value,
    ) -> Result<(), ResourceExecutionError> {
        if !self.checked_source_active || value.contains_live_resource_or_grant() {
            return Err(ResourceExecutionError::InvalidPayloadPath);
        }
        self.checked.revalidate_absent_bindings(proof)?;
        self.validate_value_type(
            &EvaluatedValue::ordinary(value.clone()),
            proof.parent_type(),
        )?;
        proof.validate_parent(value)
    }

    pub(crate) fn prepare_absent_for<'source>(
        &self,
        source: &'source ForStmt,
        value: &Value,
    ) -> Result<Option<PreparedAbsentBindings<'source>>, ResourceExecutionError> {
        if !self.checked_source_active {
            return Ok(None);
        }
        let proof = self.checked.prepare_absent_for(source)?;
        if let Some(proof) = &proof {
            self.validate_absent_parent(proof, value)?;
        }
        Ok(proof)
    }

    pub(crate) fn prepare_absent_match<'source>(
        &self,
        source: &'source MatchStmt,
        arm: usize,
        value: &Value,
    ) -> Result<Option<PreparedAbsentBindings<'source>>, ResourceExecutionError> {
        if !self.checked_source_active {
            return Ok(None);
        }
        let proof = self.checked.prepare_absent_match(source, arm)?;
        if let Some(proof) = &proof {
            self.validate_absent_parent(proof, value)?;
        }
        Ok(proof)
    }

    pub(crate) fn validate_absent_binding(
        &self,
        proof: &PreparedAbsentBindings<'_>,
        binder: &Ident,
        ordinal: usize,
        value: &Value,
    ) -> Result<CheckedBindingFact, ResourceExecutionError> {
        if !self.checked_source_active || value.contains_live_resource_or_grant() {
            return Err(ResourceExecutionError::InvalidPayloadPath);
        }
        self.checked.revalidate_absent_bindings(proof)?;
        let fact = proof.validate_binding(binder, ordinal)?;
        absent_shape(value, fact.ty, &self.checked)?;
        let ordinary = EvaluatedValue::ordinary(value.clone());
        self.validate_value_type(&ordinary, fact.ty)?;
        Ok(fact)
    }

    pub(crate) fn publish_absent_binding(
        &mut self,
        proof: &PreparedAbsentBindings<'_>,
        binder: &Ident,
        ordinal: usize,
        value: Value,
    ) -> Result<(), ResourceExecutionError> {
        let fact = self.validate_absent_binding(proof, binder, ordinal, &value)?;
        if self.checked.type_contains_resource(fact.ty)? {
            self.install_binding(fact.declaration_span, EvaluatedValue::ordinary(value))?;
        }
        Ok(())
    }
}

// Absence is a value/layout proof, not an exemption based on the lack
// of a carrier. Recurse through every occupied slot of the exact type.
pub(super) fn absent_shape(
    value: &Value,
    ty: jett_types::TypeId,
    checked: &CheckedExecution,
) -> Result<(), ResourceExecutionError> {
    use jett_types::Type;
    let types = &checked.program().checked().interner;
    if ty.index() as usize >= types.len() {
        return Err(ResourceExecutionError::InvalidCheckedProgram);
    }
    if value.contains_live_resource_or_grant() {
        return Err(ResourceExecutionError::InvalidPayloadPath);
    }
    let kind = types.resolve(ty);
    // A layout match cannot replace the runtime predicate boundary.
    // Resource-bearing return normalization does not prove it here.
    if matches!(kind, Type::Refinement { .. }) {
        return Err(ResourceExecutionError::InvalidPayloadPath);
    }
    if let Value::Typed { type_name, value } = value {
        if type_name != &types.type_name(ty) {
            return Err(ResourceExecutionError::InvalidPayloadPath);
        }
        let inner = match kind {
            Type::Secret(inner) => *inner,
            _ => ty,
        };
        return absent_shape(value, inner, checked);
    }
    let fields = |values: &[(String, Value)], expected: &[(String, jett_types::TypeId)]| {
        if values.len() != expected.len() {
            return Err(ResourceExecutionError::InvalidPayloadPath);
        }
        for ((name, value), (expected_name, field_type)) in values.iter().zip(expected) {
            if name != expected_name {
                return Err(ResourceExecutionError::InvalidPayloadPath);
            }
            absent_shape(value, *field_type, checked)?;
        }
        Ok(())
    };
    let payloads = |values: &[Value], expected: &[(String, jett_types::TypeId)]| {
        if values.len() != expected.len() {
            return Err(ResourceExecutionError::InvalidPayloadPath);
        }
        for (value, (_, field_type)) in values.iter().zip(expected) {
            absent_shape(value, *field_type, checked)?;
        }
        Ok(())
    };
    match (value, kind) {
        (value, Type::Secret(inner)) => absent_shape(value, *inner, checked),
        (Value::Int64(number), Type::Int8) if i8::try_from(*number).is_ok() => Ok(()),
        (Value::Int64(number), Type::Int16) if i16::try_from(*number).is_ok() => Ok(()),
        (Value::Int64(number), Type::Int32) if i32::try_from(*number).is_ok() => Ok(()),
        (Value::Int64(_), Type::Int64) => Ok(()),
        (Value::Int64(number), Type::Uint8) if u8::try_from(*number).is_ok() => Ok(()),
        (Value::Int64(number), Type::Uint16) if u16::try_from(*number).is_ok() => Ok(()),
        (Value::Int64(number), Type::Uint32) if u32::try_from(*number).is_ok() => Ok(()),
        (Value::Uint64(_), Type::Uint64)
        | (Value::Float64(_), Type::Float32 | Type::Float64)
        | (Value::String(_), Type::String)
        | (Value::Bool(_), Type::Bool)
        | (Value::Bytes(_), Type::Bytes)
        | (Value::Nothing, Type::Nothing) => Ok(()),
        (Value::List(values), Type::List(inner)) | (Value::Set(values), Type::Set(inner)) => {
            for value in values {
                absent_shape(value, *inner, checked)?;
            }
            Ok(())
        }
        (Value::Map(values), Type::Map(key, item)) => {
            for (key_value, item_value) in values {
                absent_shape(key_value, *key, checked)?;
                absent_shape(item_value, *item, checked)?;
            }
            Ok(())
        }
        (Value::OptionalNone, Type::Optional(_)) => Ok(()),
        (Value::OptionalSome(value), Type::Optional(inner)) => absent_shape(value, *inner, checked),
        (Value::ResultOk(value), Type::Result(success, _)) => {
            absent_shape(value, *success, checked)
        }
        (Value::ResultFail(value), Type::Result(_, error)) => absent_shape(value, *error, checked),
        (
            Value::Struct {
                type_name,
                concrete_type,
                fields: values,
            },
            Type::Struct(owner),
        ) => {
            let definition = types.resolve_struct(*owner);
            let arguments = types.nominal_type_arguments(ty);
            let header_matches = if arguments.is_empty() {
                type_name == &definition.name
                    && concrete_type
                        .as_ref()
                        .is_none_or(|name| name == &definition.name)
            } else {
                let physical_instance = format!(
                    "{}[{}]",
                    type_name,
                    arguments
                        .iter()
                        .map(|argument| types.type_name(*argument))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                physical_instance == definition.name
                    && concrete_type.as_deref() == Some(definition.name.as_str())
            };
            if !header_matches {
                return Err(ResourceExecutionError::InvalidPayloadPath);
            }
            fields(values, &definition.fields)
        }
        (
            Value::Enum {
                type_name,
                variant,
                fields: values,
            },
            Type::Enum(owner),
        ) => {
            let definition = types.resolve_enum(*owner);
            if type_name != &definition.name {
                return Err(ResourceExecutionError::InvalidPayloadPath);
            }
            let mut variants = definition
                .variants
                .iter()
                .filter(|candidate| candidate.name == *variant);
            let selected = variants
                .next()
                .ok_or(ResourceExecutionError::InvalidPayloadPath)?;
            if variants.next().is_some() {
                return Err(ResourceExecutionError::InvalidCheckedProgram);
            }
            payloads(values, &selected.fields)
        }
        (
            Value::Machine {
                type_name,
                state,
                fields: values,
            },
            Type::Machine(owner),
        ) => {
            let definition = types.resolve_machine(*owner);
            if type_name != &definition.name {
                return Err(ResourceExecutionError::InvalidPayloadPath);
            }
            let mut states = definition
                .states
                .iter()
                .filter(|candidate| candidate.name == *state);
            let selected = states
                .next()
                .ok_or(ResourceExecutionError::InvalidPayloadPath)?;
            if states.next().is_some() {
                return Err(ResourceExecutionError::InvalidCheckedProgram);
            }
            payloads(values, &selected.fields)
        }
        (
            Value::Machine {
                type_name,
                state,
                fields: values,
            },
            Type::MachineState {
                machine,
                state: expected,
            },
        ) => {
            let definition = types.resolve_machine(*machine);
            let selected = definition
                .state(*expected)
                .ok_or(ResourceExecutionError::InvalidCheckedProgram)?;
            if type_name != &definition.name || state != &selected.name {
                return Err(ResourceExecutionError::InvalidPayloadPath);
            }
            payloads(values, &selected.fields)
        }
        (Value::ResourceHook(descriptor), Type::Function { .. })
            if Arc::ptr_eq(&descriptor.program, checked.program())
                && checked
                    .program()
                    .checked()
                    .resource_hooks
                    .get(&descriptor.definition)
                    .is_some_and(|hook| hook.function_type == ty) =>
        {
            Ok(())
        }
        _ => Err(ResourceExecutionError::InvalidPayloadPath),
    }
}

#[cfg(test)]
#[path = "absent_generated/tests.rs"]
mod tests;
