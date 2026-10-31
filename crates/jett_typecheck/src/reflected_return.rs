//! Type-only admission for a value produced by a reflected callable invocation.
//! No value, callback or predicate is inspected while deriving this plan.

use std::collections::HashSet;

use jett_types::{Type, TypeId, TypeInterner};

use crate::CheckResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProducedRefinementUnsupported {
    Nested,
    Secret,
    CallableOrNominal,
}

impl ProducedRefinementUnsupported {
    pub fn message(self) -> &'static str {
        match self {
            Self::Nested => {
                "reflected field refinement: native nested refinement validation is not implemented"
            }
            Self::Secret => {
                "reflected field refinement: cannot establish new invariants beneath secret"
            }
            Self::CallableOrNominal => {
                "reflected field refinement: unsupported callable or nominal refinement conversion"
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProducedRefinementRequirement {
    Exact,
    Predicates(Vec<TypeId>),
    Unsupported(ProducedRefinementUnsupported),
    Refine {
        base: Box<ProducedRefinementRequirementPlan>,
        predicates: Vec<TypeId>,
    },
    List(Box<ProducedRefinementRequirementPlan>),
    Set(Box<ProducedRefinementRequirementPlan>),
    Map {
        key: Box<ProducedRefinementRequirementPlan>,
        value: Box<ProducedRefinementRequirementPlan>,
    },
    Optional(Box<ProducedRefinementRequirementPlan>),
    Result {
        ok: Box<ProducedRefinementRequirementPlan>,
        error: Box<ProducedRefinementRequirementPlan>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProducedRefinementRequirementPlan {
    pub source_type: TypeId,
    pub requested_type: TypeId,
    pub requirement: ProducedRefinementRequirement,
}

/// Constructor-only evidence from one checked type relation. Its caller must
/// additionally retain that exact immutable checked session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedReflectedCallablePlan {
    source_type: TypeId,
    requested_type: TypeId,
    source_return: TypeId,
    requested_return: TypeId,
    return_requirement: ProducedRefinementRequirement,
    source_signature: Type,
    requested_signature: Type,
}

impl CheckedReflectedCallablePlan {
    pub fn source_type(&self) -> TypeId {
        self.source_type
    }
    pub fn requested_type(&self) -> TypeId {
        self.requested_type
    }
    pub fn source_return(&self) -> TypeId {
        self.source_return
    }
    pub fn requested_return(&self) -> TypeId {
        self.requested_return
    }
    pub fn return_requirement(&self) -> &ProducedRefinementRequirement {
        &self.return_requirement
    }

    pub fn validate(&self, checked: &CheckResult) -> bool {
        checked
            .reflected_callable_plan(self.source_type, self.requested_type)
            .as_ref()
            == Some(self)
    }

    pub fn validate_types(&self, types: &TypeInterner) -> bool {
        (self.source_type.index() as usize) < types.len()
            && (self.requested_type.index() as usize) < types.len()
            && types.resolve(self.source_type) == &self.source_signature
            && types.resolve(self.requested_type) == &self.requested_signature
            && produced_refinement_requirement(types, self.source_return, self.requested_return)
                .as_ref()
                == Some(&self.return_requirement)
    }
}

impl CheckResult {
    pub fn reflected_callable_plan(
        &self,
        actual: TypeId,
        requested: TypeId,
    ) -> Option<CheckedReflectedCallablePlan> {
        if !self.ordinary_callable_parameters_compatible(actual, requested) {
            return None;
        }
        let Type::Function {
            return_type: source_return,
            ..
        } = self.interner.resolve(actual)
        else {
            return None;
        };
        let Type::Function {
            return_type: requested_return,
            ..
        } = self.interner.resolve(requested)
        else {
            return None;
        };
        let return_requirement =
            produced_refinement_requirement(&self.interner, *source_return, *requested_return)?;
        let node = ProducedRefinementRequirementPlan {
            source_type: *source_return,
            requested_type: *requested_return,
            requirement: return_requirement.clone(),
        };
        if !supported_node(self, &node) {
            return None;
        }
        Some(CheckedReflectedCallablePlan {
            source_type: actual,
            requested_type: requested,
            source_return: *source_return,
            requested_return: *requested_return,
            return_requirement,
            source_signature: self.interner.resolve(actual).clone(),
            requested_signature: self.interner.resolve(requested).clone(),
        })
    }
}

fn supported_node(checked: &CheckResult, node: &ProducedRefinementRequirementPlan) -> bool {
    use ProducedRefinementRequirement as R;
    let Some((source_base, source_chain)) = refinement_chain(&checked.interner, node.source_type)
    else {
        return false;
    };
    let Some((requested_base, _)) = refinement_chain(&checked.interner, node.requested_type) else {
        return false;
    };
    match &node.requirement {
        R::Exact => {
            node.source_type == node.requested_type
                || source_chain.contains(&node.requested_type)
                || checked.ordinary_types_compatible(node.requested_type, node.source_type)
                || checked.ordinary_types_compatible(node.requested_type, source_base)
        }
        R::Predicates(_) => checked.ordinary_types_compatible(requested_base, source_base),
        R::Unsupported(_) => false,
        R::Refine { base, .. } => supported_node(checked, base),
        R::List(child) | R::Set(child) | R::Optional(child) => supported_node(checked, child),
        R::Map { key, value } => supported_node(checked, key) && supported_node(checked, value),
        R::Result { ok, error } => supported_node(checked, ok) && supported_node(checked, error),
    }
}

fn contains_refinement(types: &TypeInterner, ty: TypeId, seen: &mut HashSet<TypeId>) -> bool {
    if ty.index() as usize >= types.len() {
        return true;
    }
    if !seen.insert(ty) {
        return false;
    }
    let children = match types.resolve(ty) {
        Type::Refinement { .. } => return true,
        Type::Secret(inner) | Type::List(inner) | Type::Set(inner) | Type::Optional(inner) => {
            vec![*inner]
        }
        Type::Result(a, b) | Type::Map(a, b) => vec![*a, *b],
        Type::Function {
            params,
            return_type,
            ..
        } => params
            .iter()
            .copied()
            .chain(std::iter::once(*return_type))
            .collect(),
        Type::Struct(_)
        | Type::Enum(_)
        | Type::Machine(_)
        | Type::MachineState { .. }
        | Type::Bitfield(_)
        | Type::Interface(_)
        | Type::Actor(_) => types.nominal_type_arguments(ty).to_vec(),
        _ => Vec::new(),
    };
    children
        .into_iter()
        .any(|child| contains_refinement(types, child, seen))
}

pub(crate) fn refinement_chain(
    types: &TypeInterner,
    mut ty: TypeId,
) -> Option<(TypeId, Vec<TypeId>)> {
    let mut chain = Vec::new();
    for _ in 0..types.len() {
        if ty.index() as usize >= types.len() {
            return None;
        }
        match types.resolve(ty) {
            Type::Refinement { base, .. } => {
                chain.push(ty);
                ty = *base;
            }
            _ => {
                chain.reverse();
                return Some((ty, chain));
            }
        }
    }
    None
}

pub fn produced_refinement_requirement(
    types: &TypeInterner,
    actual: TypeId,
    requested: TypeId,
) -> Option<ProducedRefinementRequirement> {
    requirement_node(types, actual, requested, types.len()).map(|node| node.requirement)
}

fn requirement_node(
    types: &TypeInterner,
    actual: TypeId,
    requested: TypeId,
    budget: usize,
) -> Option<ProducedRefinementRequirementPlan> {
    if budget == 0
        || actual.index() as usize >= types.len()
        || requested.index() as usize >= types.len()
    {
        return None;
    }
    Some(ProducedRefinementRequirementPlan {
        source_type: actual,
        requested_type: requested,
        requirement: requirement_action(types, actual, requested, budget - 1)?,
    })
}

fn requirement_action(
    types: &TypeInterner,
    actual: TypeId,
    requested: TypeId,
    budget: usize,
) -> Option<ProducedRefinementRequirement> {
    use ProducedRefinementRequirement as R;
    if actual == requested || !contains_refinement(types, requested, &mut HashSet::new()) {
        return Some(R::Exact);
    }
    let (actual_base, actual_chain) = refinement_chain(types, actual)?;
    if actual_chain.contains(&requested) {
        return Some(R::Exact);
    }
    if actual_base == requested
        && !matches!(
            types.resolve(requested),
            Type::List(_) | Type::Set(_) | Type::Map(..) | Type::Optional(_) | Type::Result(..)
        )
    {
        return Some(R::Exact);
    }
    let (base, mut chain) = refinement_chain(types, requested)?;
    if !chain.is_empty() {
        if let Some(index) = chain.iter().rposition(|ty| actual_chain.contains(ty)) {
            chain.drain(..=index);
            return Some(R::Predicates(chain));
        }
        let child = requirement_node(types, actual, base, budget)?;
        return Some(match child.requirement {
            R::Exact => R::Predicates(chain),
            R::Unsupported(reason) => R::Unsupported(reason),
            _ => R::Refine {
                base: Box::new(child),
                predicates: chain,
            },
        });
    }
    let child = |source, target| requirement_node(types, source, target, budget).map(Box::new);
    Some(
        match (types.resolve(actual_base), types.resolve(requested)) {
            (Type::List(a), Type::List(b)) => R::List(child(*a, *b)?),
            (Type::Set(a), Type::Set(b)) => R::Set(child(*a, *b)?),
            (Type::Map(ak, av), Type::Map(bk, bv)) => R::Map {
                key: child(*ak, *bk)?,
                value: child(*av, *bv)?,
            },
            (Type::Optional(a), Type::Optional(b)) => R::Optional(child(*a, *b)?),
            (Type::Result(a, b), Type::Result(c, d)) => R::Result {
                ok: child(*a, *c)?,
                error: child(*b, *d)?,
            },
            (_, Type::Secret(_)) => R::Unsupported(ProducedRefinementUnsupported::Secret),
            _ => R::Unsupported(ProducedRefinementUnsupported::CallableOrNominal),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn return_requirements_keep_exact_prefix_and_new_suffix_separate() {
        let mut types = TypeInterner::new();
        let positive = types.intern(Type::Refinement {
            name: "app.Positive".into(),
            base: TypeInterner::INT64,
        });
        let bounded = types.intern(Type::Refinement {
            name: "app.Bounded".into(),
            base: positive,
        });
        assert_eq!(
            produced_refinement_requirement(&types, TypeInterner::INT64, positive),
            Some(ProducedRefinementRequirement::Predicates(vec![positive]))
        );
        assert_eq!(
            produced_refinement_requirement(&types, positive, bounded),
            Some(ProducedRefinementRequirement::Predicates(vec![bounded]))
        );
        assert_eq!(
            produced_refinement_requirement(&types, bounded, positive),
            Some(ProducedRefinementRequirement::Exact)
        );
        let secret = types.intern(Type::Secret(positive));
        assert_eq!(
            produced_refinement_requirement(&types, TypeInterner::INT64, secret),
            Some(ProducedRefinementRequirement::Unsupported(
                ProducedRefinementUnsupported::Secret
            ))
        );
    }
}
