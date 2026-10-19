//! Storage kernel for an authenticated ordinary checked field place.
//!
//! Deliberately not installed in `Interpreter`: its checked-body adapter and
//! exact lexical binding association must exist before Source can reach it.
//! Runtime layout records below validate storage; they never authorize a write.
use crate::value::Value;

#[path = "field_updates/checked.rs"]
mod checked;
pub(super) use checked::FieldUpdateContext;

pub(super) fn is_projected_target(target: &jett_parser::ast::Expr) -> bool {
    match target {
        jett_parser::ast::Expr::Paren(inner, _) => is_projected_target(inner),
        jett_parser::ast::Expr::FieldAccess(..) => true,
        _ => false,
    }
}

#[cfg(test)]
#[path = "field_updates/tests.rs"]
mod tests;

/// Allocated from a checked canonical owner before evaluating the RHS.
/// No constructor from AST spelling, type names, or raw Values is provided.
#[derive(Debug)]
pub(super) struct FieldStoragePlan {
    projections: Vec<StorageProjection>,
}

#[derive(Debug)]
struct StorageProjection {
    /// Exact retained wrappers in order. Refined enclosing owners are excluded
    /// by checked-place admission, rather than peeled here to obtain permission.
    wrappers: Vec<String>,
    owner: StorageOwner,
    field_index: usize,
}

#[derive(Debug)]
enum StorageOwner {
    Struct {
        type_name: String,
        concrete_type: Option<String>,
        field_names: Vec<String>,
    },
    MachineState {
        type_name: String,
        state: String,
        field_count: usize,
    },
}

/// Only the missing checked adapter may construct this after ordinary RHS
/// normalization/conversion succeeds. This carries no Resource custody.
pub(super) struct StagedOrdinaryReplacement {
    value: Value,
}

impl std::fmt::Debug for StagedOrdinaryReplacement {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StagedOrdinaryReplacement")
            .finish_non_exhaustive()
    }
}

/// Structured internal refusal: no runtime payload or secret value is formatted.
/// The checked adapter must translate pending refusal through the established
/// typed inspection diagnostic, rather than add a second observation policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FieldStorageError {
    EmptyPath,
    ResourceCustodyRequired,
    PendingAncestor { projection: usize },
    WrapperMismatch { projection: usize },
    OwnerMismatch { projection: usize },
    FieldMismatch { projection: usize },
}

/// A rejected staged owner remains available for the existing cleanup path.
/// The kernel neither duplicates it nor replaces the target on failure.
#[derive(Debug)]
pub(super) struct RejectedReplacement {
    pub(super) error: FieldStorageError,
    pub(super) replacement: StagedOrdinaryReplacement,
}

impl FieldStoragePlan {
    /// The evaluator must call this before evaluating the RHS. A pending target
    /// child is transportable: readiness is tested only for projection owners.
    pub(super) fn preflight(&self, root: &Value) -> Result<(), FieldStorageError> {
        if self.projections.is_empty() {
            return Err(FieldStorageError::EmptyPath);
        }
        // An ordinary scalar child inside a Resource-bearing root must not
        // circumvent authenticated aggregate replacement/custody handling.
        if root.contains_live_resource_or_grant() {
            return Err(FieldStorageError::ResourceCustodyRequired);
        }
        let mut current = root;
        for (index, projection) in self.projections.iter().enumerate() {
            current = projection.selected(current, index)?;
        }
        Ok(())
    }

    /// Called only after the evaluator reauthenticates the original checked
    /// assignment and the same live lexical slot/generation after RHS completion.
    /// All conversion, allocation and fallible preparation precede this method.
    pub(super) fn replace(
        &self,
        root: &mut Value,
        replacement: StagedOrdinaryReplacement,
    ) -> Result<Value, RejectedReplacement> {
        if let Err(error) = self.preflight(root) {
            return Err(RejectedReplacement { error, replacement });
        }
        if replacement.value.contains_live_resource_or_grant() {
            return Err(RejectedReplacement {
                error: FieldStorageError::ResourceCustodyRequired,
                replacement,
            });
        }
        let selected = match self.selected_mut(root) {
            Ok(selected) => selected,
            Err(error) => return Err(RejectedReplacement { error, replacement }),
        };
        // This is the first mutation. No allocation, predicate, Source call,
        // payload inspection, or other fallible work follows the displacement.
        // The caller retires the one displaced ordinary owner through its normal
        // cleanup path; all unchanged fields remain in their original storage.
        Ok(std::mem::replace(selected, replacement.value))
    }

    fn selected_mut<'value>(
        &self,
        root: &'value mut Value,
    ) -> Result<&'value mut Value, FieldStorageError> {
        let mut current = root;
        for (index, projection) in self.projections.iter().enumerate() {
            current = projection.selected_mut(current, index)?;
        }
        Ok(current)
    }
}

impl StorageProjection {
    fn selected<'value>(
        &self,
        value: &'value Value,
        projection: usize,
    ) -> Result<&'value Value, FieldStorageError> {
        let owner = self.checked_wrappers(value, projection)?;
        match (&self.owner, owner) {
            (
                StorageOwner::Struct {
                    type_name,
                    concrete_type,
                    field_names,
                },
                Value::Struct {
                    type_name: actual_name,
                    concrete_type: actual_concrete,
                    fields,
                },
            ) if type_name == actual_name && concrete_type == actual_concrete => {
                if fields.len() != field_names.len()
                    || fields
                        .iter()
                        .zip(field_names)
                        .any(|((actual, _), expected)| actual != expected)
                {
                    return Err(FieldStorageError::FieldMismatch { projection });
                }
                fields
                    .get(self.field_index)
                    .map(|(_, value)| value)
                    .ok_or(FieldStorageError::FieldMismatch { projection })
            }
            (
                StorageOwner::MachineState {
                    type_name,
                    state,
                    field_count,
                },
                Value::Machine {
                    type_name: actual_name,
                    state: actual_state,
                    fields,
                },
            ) if type_name == actual_name && state == actual_state => {
                if fields.len() != *field_count {
                    return Err(FieldStorageError::FieldMismatch { projection });
                }
                fields
                    .get(self.field_index)
                    .ok_or(FieldStorageError::FieldMismatch { projection })
            }
            _ => Err(FieldStorageError::OwnerMismatch { projection }),
        }
    }

    fn checked_wrappers<'value>(
        &self,
        mut value: &'value Value,
        projection: usize,
    ) -> Result<&'value Value, FieldStorageError> {
        for expected in &self.wrappers {
            match value {
                Value::Pending(_) => {
                    return Err(FieldStorageError::PendingAncestor { projection });
                }
                Value::Typed {
                    type_name,
                    value: inner,
                } if type_name == expected => value = inner,
                _ => return Err(FieldStorageError::WrapperMismatch { projection }),
            }
        }
        match value {
            Value::Pending(_) => Err(FieldStorageError::PendingAncestor { projection }),
            Value::Typed { .. } => Err(FieldStorageError::WrapperMismatch { projection }),
            _ => Ok(value),
        }
    }

    fn selected_mut<'value>(
        &self,
        value: &'value mut Value,
        projection: usize,
    ) -> Result<&'value mut Value, FieldStorageError> {
        // Reuse exact shape validation before taking mutable descendant access.
        self.selected(value, projection)?;
        let mut owner = value;
        for expected in &self.wrappers {
            match owner {
                Value::Typed { type_name, value } if type_name == expected => owner = value,
                _ => return Err(FieldStorageError::WrapperMismatch { projection }),
            }
        }
        match (&self.owner, owner) {
            (StorageOwner::Struct { .. }, Value::Struct { fields, .. }) => fields
                .get_mut(self.field_index)
                .map(|(_, value)| value)
                .ok_or(FieldStorageError::FieldMismatch { projection }),
            (StorageOwner::MachineState { .. }, Value::Machine { fields, .. }) => fields
                .get_mut(self.field_index)
                .ok_or(FieldStorageError::FieldMismatch { projection }),
            _ => Err(FieldStorageError::OwnerMismatch { projection }),
        }
    }
}
