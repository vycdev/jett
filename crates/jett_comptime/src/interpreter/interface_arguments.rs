//! Read-only owner preflight before erased dispatch changes argument identities.
use super::{FunctionDef, HashMap, HashSet, Interpreter, TypeExpr, Value, runtime_type_name};

const OWNER_MISMATCH: &str = "interface dispatch concrete argument owner mismatch";

impl Interpreter {
    pub(super) fn validate_interface_argument_owners(
        &self,
        _interface: &str,
        function: &FunctionDef,
        namespace: Option<&str>,
        type_scope: &HashMap<String, TypeExpr>,
        arguments: &[Value],
    ) -> Result<(), String> {
        // Dispatch already established the ready receiver's concrete identity.
        let owner = arguments
            .first()
            .and_then(runtime_type_name)
            .ok_or(OWNER_MISMATCH)?;
        for (parameter, argument) in function.params.iter().zip(arguments) {
            let written = self.interface_parameter_type(&parameter.ty, type_scope, namespace);
            let expected = self.concrete_resolved_type_expr(&written, &mut HashSet::new());
            self.validate_interface_owner_occurrence(&expected, &owner, argument)?;
        }
        Ok(())
    }

    fn interface_parameter_type(
        &self,
        written: &TypeExpr,
        type_scope: &HashMap<String, TypeExpr>,
        namespace: Option<&str>,
    ) -> TypeExpr {
        match written {
            TypeExpr::Named(name) => type_scope.get(&name.name).cloned().unwrap_or_else(|| {
                TypeExpr::Named(self.qualify_declared_type_ident(name, namespace))
            }),
            TypeExpr::Generic(name, children, span) => TypeExpr::Generic(
                self.qualify_declared_type_ident(name, namespace),
                children
                    .iter()
                    .map(|child| self.interface_parameter_type(child, type_scope, namespace))
                    .collect(),
                *span,
            ),
            TypeExpr::View(inner, span) => TypeExpr::View(
                Box::new(self.interface_parameter_type(inner, type_scope, namespace)),
                *span,
            ),
            TypeExpr::StateQualified(inner, state, span) => TypeExpr::StateQualified(
                Box::new(self.interface_parameter_type(inner, type_scope, namespace)),
                state.clone(),
                *span,
            ),
            TypeExpr::Function(parameters, output, span) => TypeExpr::Function(
                parameters
                    .iter()
                    .map(|parameter| {
                        self.interface_parameter_type(parameter, type_scope, namespace)
                    })
                    .collect(),
                Box::new(self.interface_parameter_type(output, type_scope, namespace)),
                *span,
            ),
        }
    }

    fn interface_type_contains_owner(&self, expected: &TypeExpr, owner: &str) -> bool {
        if self.resolved_concrete_type_display(expected) == owner {
            return true;
        }
        match expected {
            TypeExpr::Named(_) => false,
            TypeExpr::Generic(_, children, _) => children
                .iter()
                .any(|child| self.interface_type_contains_owner(child, owner)),
            TypeExpr::View(inner, _) | TypeExpr::StateQualified(inner, _, _) => {
                self.interface_type_contains_owner(inner, owner)
            }
            // A checked callable adapter validates its arguments when invoked;
            // the callable itself is not an occurrence of its argument values.
            TypeExpr::Function(_, _, _) => false,
        }
    }

    fn validate_interface_owner_occurrence(
        &self,
        expected: &TypeExpr,
        owner: &str,
        argument: &Value,
    ) -> Result<(), String> {
        if let TypeExpr::View(inner, _) = expected {
            return self.validate_interface_owner_occurrence(inner, owner, argument);
        }
        let expected_name = self.resolved_concrete_type_display(expected);
        if expected_name == owner {
            // Reading retained identity does not join a task or inspect fields.
            return if retained_argument_owner(argument).as_deref() == Some(owner) {
                Ok(())
            } else {
                Err(OWNER_MISMATCH.into())
            };
        }
        if !self.interface_type_contains_owner(expected, owner) {
            return Ok(());
        }
        if let Value::Pending(_) = argument {
            // An unavailable container cannot supply new child-owner facts.
            return if retained_argument_owner(argument).as_deref() == Some(expected_name.as_str()) {
                Ok(())
            } else {
                Err(OWNER_MISMATCH.into())
            };
        }
        match (expected, argument.payload()) {
            (TypeExpr::Generic(name, children, _), value)
                if matches!(
                    name.name.as_str(),
                    "secret" | "list" | "set" | "optional" | "result" | "map"
                ) =>
            {
                self.validate_interface_owner_children(&name.name, children, owner, value)
            }
            (TypeExpr::StateQualified(inner, _, _), value) => {
                self.validate_interface_owner_occurrence(inner, owner, value)
            }
            // Nominal generic records are invariant. Their retained identity
            // proves all arguments without reinterpreting or inspecting fields.
            _ if retained_argument_owner(argument).as_deref() == Some(expected_name.as_str()) => {
                Ok(())
            }
            _ => Err(OWNER_MISMATCH.into()),
        }
    }

    fn validate_interface_owner_children(
        &self,
        constructor: &str,
        children: &[TypeExpr],
        owner: &str,
        value: &Value,
    ) -> Result<(), String> {
        match (constructor, children, value) {
            ("secret", [inner], value) => {
                self.validate_interface_owner_occurrence(inner, owner, value)
            }
            ("list", [element], Value::List(elements))
            | ("set", [element], Value::Set(elements)) => {
                for value in elements {
                    self.validate_interface_owner_occurrence(element, owner, value)?;
                }
                Ok(())
            }
            ("optional", [element], Value::OptionalSome(value))
            | ("result", [element, _], Value::ResultOk(value))
            | ("result", [_, element], Value::ResultFail(value)) => {
                self.validate_interface_owner_occurrence(element, owner, value)
            }
            ("optional", [_], Value::OptionalNone) => Ok(()),
            ("map", [key_type, value_type], Value::Map(entries)) => {
                for (key, value) in entries {
                    self.validate_interface_owner_occurrence(key_type, owner, key)?;
                    self.validate_interface_owner_occurrence(value_type, owner, value)?;
                }
                Ok(())
            }
            _ => {
                let expected = format!(
                    "{}[{}]",
                    constructor,
                    children
                        .iter()
                        .map(|child| self.resolved_concrete_type_display(child))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                if retained_argument_owner(value).as_deref() == Some(expected.as_str()) {
                    Ok(())
                } else {
                    Err(OWNER_MISMATCH.into())
                }
            }
        }
    }
}

fn retained_argument_owner(value: &Value) -> Option<String> {
    match value {
        Value::Pending(inner) => retained_argument_owner(inner),
        value => runtime_type_name(value),
    }
}

#[cfg(test)]
mod tests;
