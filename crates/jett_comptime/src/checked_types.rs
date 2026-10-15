//! Checked expression types retained separately for each generic body.
use std::collections::HashMap;
use std::sync::Arc;

use jett_common::Span;
use jett_types::ReflectionTypeInfo;

use crate::value::{ClosureTypeArgument, Value};

#[derive(Debug, Clone, Default)]
pub struct CheckedExpressionTypes {
    /// Exact compiler session. Ordinary type-name projections confer no hook authority.
    pub resource_program: Option<Arc<jett_typecheck::CheckedResourceProgram>>,
    pub bindings: CheckedScopedBindings,
    pub expressions: HashMap<Span, String>,
    pub functions: HashMap<Span, Vec<Arc<CheckedFunctionTypes>>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedFunctionTypes {
    pub bindings: CheckedScopedBindings,
    pub type_arguments: Vec<String>,
    pub type_argument_reflections: Vec<ReflectionTypeInfo>,
    pub type_info_kinds: Vec<(usize, String)>,
    pub type_info_primitives: Vec<(usize, Option<String>)>,
    pub type_kind_values: Vec<(usize, String)>,
    pub type_primitive_values: Vec<(usize, String)>,
    pub expressions: Arc<HashMap<Span, String>>,
}

/// Recursive checked facts for one concrete lexical type binding.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedScopedTypes {
    pub bound_type: String,
    pub reflection: Option<ReflectionTypeInfo>,
    pub expressions: HashMap<Span, String>,
    pub bindings: CheckedScopedBindings,
}

pub type CheckedScopedBindings = HashMap<Span, Vec<Arc<CheckedScopedTypes>>>;

pub(crate) fn select_scoped_types(
    bindings: &CheckedScopedBindings,
    span: Span,
    bound_type: &str,
    reflection: Option<&ReflectionTypeInfo>,
) -> Result<Option<Arc<CheckedScopedTypes>>, String> {
    let Some(candidates) = bindings.get(&span) else {
        return Ok(None);
    };
    let mut matching = candidates.iter().filter(|types| {
        types.bound_type == bound_type
            && types
                .reflection
                .as_ref()
                .is_none_or(|expected| Some(expected) == reflection)
    });
    let Some(selected) = matching.next() else {
        return Ok(None);
    };
    if matching.any(|other| other != selected) {
        return Err(format!(
            "scoped type `{bound_type}` has conflicting checked expression types"
        ));
    }
    Ok(Some(selected.clone()))
}

impl CheckedExpressionTypes {
    /// Canonical mutable projections confer no execution or cache provenance.
    pub fn from_checked_program(program: &Arc<jett_typecheck::CheckedResourceProgram>) -> Self {
        Self::from_original_program(program)
    }

    /// Internal canonical projection for a worker that owns this checked Source.
    pub(crate) fn from_original_program(
        program: &Arc<jett_typecheck::CheckedResourceProgram>,
    ) -> Self {
        let checked = program.checked();
        let (mut expressions, bindings) = original_body_types(
            &checked.type_map,
            &checked.comptime_type_bindings,
            &checked.interner,
        );
        let mut functions: HashMap<Span, Vec<Arc<CheckedFunctionTypes>>> = HashMap::new();
        for instance in &checked.generic_function_instantiations {
            for span in instance.type_map.keys() {
                expressions.remove(span);
            }
            remove_original_scoped_types(&mut expressions, &instance.comptime_type_bindings);
            let (names, scopes) = original_body_types(
                &instance.type_map,
                &instance.comptime_type_bindings,
                &checked.interner,
            );
            let facts = &instance.specialization;
            functions
                .entry(program.resolved().scope_table.def(instance.definition).span)
                .or_default()
                .push(Arc::new(CheckedFunctionTypes {
                    bindings: scopes,
                    type_arguments: instance
                        .concrete_args
                        .iter()
                        .map(|ty| checked.interner.type_name(*ty))
                        .collect(),
                    type_argument_reflections: facts.type_argument_reflections.clone(),
                    type_info_kinds: facts.type_info_kinds.clone(),
                    type_info_primitives: facts.type_info_primitives.clone(),
                    type_kind_values: facts.type_kind_values.clone(),
                    type_primitive_values: facts.type_primitive_values.clone(),
                    expressions: Arc::new(names),
                }));
        }
        Self {
            resource_program: Some(program.clone()),
            bindings,
            expressions,
            functions,
        }
    }

    pub(crate) fn matches_original_program(
        &self,
        program: &Arc<jett_typecheck::CheckedResourceProgram>,
    ) -> bool {
        let expected = Self::from_original_program(program);
        self.resource_program
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, program))
            && self.expressions == expected.expressions
            && self.bindings == expected.bindings
            && self.functions == expected.functions
    }

    pub fn get(&self, span: &Span) -> Option<&String> {
        self.expressions.get(span)
    }

    pub(crate) fn select(
        &self,
        declaration: Span,
        types: &[ClosureTypeArgument],
        args: &[Value],
    ) -> Result<Option<Arc<CheckedFunctionTypes>>, String> {
        let Some(candidates) = self.functions.get(&declaration) else {
            return Ok(None);
        };
        let matching = candidates
            .iter()
            .filter(|candidate| candidate.matches(types, args));
        let Some(specificity) = matching
            .clone()
            .map(|candidate| candidate.specificity())
            .max()
        else {
            // Compiler-owned facades can execute interpreter source bodies
            // whose checked call uses a builtin or a different native helper.
            // Their source specialization is absent, just as for an unchecked
            // standalone interpreter. Never borrow another instance's map.
            return Ok(None);
        };
        let mut matching = matching.filter(|candidate| candidate.specificity() == specificity);
        let selected = matching
            .next()
            .expect("matching candidate has maximum specificity");
        if matching.any(|other| {
            other.expressions != selected.expressions || other.bindings != selected.bindings
        }) {
            return Err("generic invocation has conflicting checked expression types".into());
        }
        Ok(Some(selected.clone()))
    }
}

fn original_body_types(
    types: &HashMap<Span, jett_types::TypeId>,
    bindings: &HashMap<Span, Vec<jett_typecheck::CheckedComptimeTypeBinding>>,
    interner: &jett_types::TypeInterner,
) -> (HashMap<Span, String>, CheckedScopedBindings) {
    let mut expressions = types
        .iter()
        .map(|(span, ty)| (*span, interner.type_name(*ty)))
        .collect();
    remove_original_scoped_types(&mut expressions, bindings);
    let nested = bindings
        .values()
        .flatten()
        .flat_map(|binding| binding.body.comptime_type_bindings.keys().copied())
        .collect::<std::collections::HashSet<_>>();
    let scopes = bindings
        .iter()
        .filter(|(span, _)| !nested.contains(span))
        .map(|(span, candidates)| {
            (
                *span,
                candidates
                    .iter()
                    .map(|binding| {
                        let (expressions, bindings) = original_body_types(
                            &binding.body.type_map,
                            &binding.body.comptime_type_bindings,
                            interner,
                        );
                        Arc::new(CheckedScopedTypes {
                            bound_type: interner.type_name(binding.bound_type),
                            reflection: Some(binding.reflection.clone()),
                            expressions,
                            bindings,
                        })
                    })
                    .collect(),
            )
        })
        .collect();
    (expressions, scopes)
}

fn remove_original_scoped_types(
    expressions: &mut HashMap<Span, String>,
    bindings: &HashMap<Span, Vec<jett_typecheck::CheckedComptimeTypeBinding>>,
) {
    for binding in bindings.values().flatten() {
        for span in binding.body.type_map.keys() {
            expressions.remove(span);
        }
        remove_original_scoped_types(expressions, &binding.body.comptime_type_bindings);
    }
}

impl CheckedFunctionTypes {
    fn specificity(&self) -> usize {
        self.type_info_kinds.len()
            + self.type_info_primitives.len()
            + self.type_kind_values.len()
            + self.type_primitive_values.len()
    }

    fn matches(&self, types: &[ClosureTypeArgument], args: &[Value]) -> bool {
        self.type_arguments.len() == types.len()
            && self
                .type_arguments
                .iter()
                .zip(types)
                .all(|(expected, actual)| expected == &actual.canonical_name)
            && (self.type_argument_reflections.is_empty()
                || (self.type_argument_reflections.len() == types.len()
                    && self
                        .type_argument_reflections
                        .iter()
                        .zip(types)
                        .all(|(expected, actual)| actual.reflection.as_ref() == Some(expected))))
            && self.type_info_kinds.iter().all(|(index, expected)| {
                args.get(*index)
                    .and_then(|value| field(value, "kind_tag"))
                    .and_then(|value| variant(value, "TypeKind"))
                    == Some(expected.as_str())
            })
            && self.type_info_primitives.iter().all(|(index, expected)| {
                let actual = args
                    .get(*index)
                    .and_then(|value| field(value, "primitive_tag"));
                match (actual, expected) {
                    (Some(Value::OptionalNone), None) => true,
                    (Some(Value::OptionalSome(value)), Some(expected)) => {
                        variant(value, "TypePrimitive") == Some(expected.as_str())
                    }
                    _ => false,
                }
            })
            && self.type_kind_values.iter().all(|(index, expected)| {
                args.get(*index)
                    .and_then(|value| variant(value, "TypeKind"))
                    == Some(expected.as_str())
            })
            && self.type_primitive_values.iter().all(|(index, expected)| {
                args.get(*index)
                    .and_then(|value| variant(value, "TypePrimitive"))
                    == Some(expected.as_str())
            })
    }
}

fn field<'a>(value: &'a Value, name: &str) -> Option<&'a Value> {
    let Value::Struct {
        type_name, fields, ..
    } = value
    else {
        return None;
    };
    (type_name == "TypeInfo")
        .then(|| fields.iter().find(|(field, _)| field == name))?
        .map(|(_, value)| value)
}

fn variant<'a>(value: &'a Value, expected_type: &str) -> Option<&'a str> {
    let Value::Enum {
        type_name, variant, ..
    } = value
    else {
        return None;
    };
    (type_name == expected_type).then_some(variant.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use jett_common::FileId;

    #[test]
    fn scoped_alias_and_base_select_distinct_facts() {
        let span = Span::new(FileId::new(0), 1, 2);
        let base_info = ReflectionTypeInfo::new(
            "string",
            "primitive",
            Some("string_type".into()),
            false,
            vec![],
        );
        let alias_info =
            ReflectionTypeInfo::new("Label", "alias", None, false, vec![base_info.clone()]);
        let base = Arc::new(CheckedScopedTypes {
            bound_type: "string".into(),
            reflection: Some(base_info.clone()),
            ..Default::default()
        });
        let alias = Arc::new(CheckedScopedTypes {
            bound_type: "string".into(),
            reflection: Some(alias_info.clone()),
            ..Default::default()
        });
        for candidates in [
            vec![base.clone(), alias.clone()],
            vec![alias.clone(), base.clone()],
        ] {
            let bindings = HashMap::from([(span, candidates)]);
            assert!(Arc::ptr_eq(
                &select_scoped_types(&bindings, span, "string", Some(&base_info))
                    .unwrap()
                    .unwrap(),
                &base
            ));
            assert!(Arc::ptr_eq(
                &select_scoped_types(&bindings, span, "string", Some(&alias_info))
                    .unwrap()
                    .unwrap(),
                &alias
            ));
        }
    }

    #[test]
    fn conflicting_scoped_widths_are_rejected_in_either_registration_order() {
        let span = Span::new(FileId::new(0), 1, 2);
        let narrow = Arc::new(CheckedScopedTypes {
            bound_type: "Record".into(),
            expressions: HashMap::from([(span, "int8".into())]),
            ..Default::default()
        });
        let wide = Arc::new(CheckedScopedTypes {
            bound_type: "Record".into(),
            expressions: HashMap::from([(span, "int64".into())]),
            ..Default::default()
        });
        for candidates in [vec![narrow.clone(), wide.clone()], vec![wide, narrow]] {
            let types = HashMap::from([(span, candidates)]);
            assert!(
                select_scoped_types(&types, span, "Record", None)
                    .unwrap_err()
                    .contains("conflicting")
            );
            assert!(
                select_scoped_types(&types, span, "Other", None)
                    .unwrap()
                    .is_none()
            );
        }
    }

    #[test]
    fn conflicting_checked_widths_are_rejected_in_either_registration_order() {
        let span = Span::new(FileId::new(0), 1, 2);
        let narrow = Arc::new(CheckedFunctionTypes {
            expressions: Arc::new(HashMap::from([(span, "int8".into())])),
            ..Default::default()
        });
        let wide = Arc::new(CheckedFunctionTypes {
            expressions: Arc::new(HashMap::from([(span, "int64".into())])),
            ..Default::default()
        });
        for candidates in [vec![narrow.clone(), wide.clone()], vec![wide, narrow]] {
            let types = CheckedExpressionTypes {
                functions: HashMap::from([(span, candidates)]),
                ..Default::default()
            };
            assert!(
                types
                    .select(span, &[], &[])
                    .unwrap_err()
                    .contains("conflicting")
            );
        }
    }
}
