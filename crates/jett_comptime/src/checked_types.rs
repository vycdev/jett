//! Checked expression types retained separately for each generic body.
use std::collections::HashMap;
use std::sync::Arc;

use jett_common::Span;
use jett_types::ReflectionTypeInfo;

use crate::value::{ClosureTypeArgument, Value};

#[derive(Debug, Clone, Default)]
pub struct CheckedExpressionTypes {
    pub expressions: HashMap<Span, String>,
    pub functions: HashMap<Span, Vec<CheckedFunctionTypes>>,
}

#[derive(Debug, Clone, Default)]
pub struct CheckedFunctionTypes {
    pub type_arguments: Vec<String>,
    pub type_argument_reflections: Vec<ReflectionTypeInfo>,
    pub type_info_kinds: Vec<(usize, String)>,
    pub type_info_primitives: Vec<(usize, Option<String>)>,
    pub type_kind_values: Vec<(usize, String)>,
    pub type_primitive_values: Vec<(usize, String)>,
    pub expressions: Arc<HashMap<Span, String>>,
}

impl CheckedExpressionTypes {
    pub fn get(&self, span: &Span) -> Option<&String> {
        self.expressions.get(span)
    }

    pub(crate) fn select(
        &self,
        declaration: Span,
        types: &[ClosureTypeArgument],
        args: &[Value],
    ) -> Result<Option<Arc<HashMap<Span, String>>>, String> {
        let Some(candidates) = self.functions.get(&declaration) else {
            return Ok(None);
        };
        let matching = candidates
            .iter()
            .filter(|candidate| candidate.matches(types, args));
        let Some(specificity) = matching
            .clone()
            .map(CheckedFunctionTypes::specificity)
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
        if matching.any(|other| other.expressions != selected.expressions) {
            return Err("generic invocation has conflicting checked expression types".into());
        }
        Ok(Some(selected.expressions.clone()))
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
    fn conflicting_checked_widths_are_rejected_in_either_registration_order() {
        let span = Span::new(FileId::new(0), 1, 2);
        let narrow = CheckedFunctionTypes {
            expressions: Arc::new(HashMap::from([(span, "int8".into())])),
            ..Default::default()
        };
        let wide = CheckedFunctionTypes {
            expressions: Arc::new(HashMap::from([(span, "int64".into())])),
            ..Default::default()
        };
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
