//! Checked proof plans for reflected reads. Selector admission remains in the
//! original intrinsic; these plans execute only after that read succeeds.
use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReflectedFieldValidation {
    Validate(Vec<ReflectedFieldPlan>),
    /// Internal MIR stage: execute only the original selector checks and read.
    Read,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReflectedFieldPlan {
    pub source_type: TypeId,
    pub requested_type: TypeId,
    pub action: ReflectedFieldAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReflectedFieldAction {
    Exact,
    Predicates(Vec<RefinementPredicate>),
    Unsupported(ReflectedFieldUnsupported),
    Refine {
        base: Box<ReflectedFieldPlan>,
        predicates: Vec<RefinementPredicate>,
    },
    List(Box<ReflectedFieldPlan>),
    Set(Box<ReflectedFieldPlan>),
    Map {
        key: Box<ReflectedFieldPlan>,
        value: Box<ReflectedFieldPlan>,
    },
    Optional(Box<ReflectedFieldPlan>),
    Result {
        ok: Box<ReflectedFieldPlan>,
        error: Box<ReflectedFieldPlan>,
    },
}

impl ReflectedFieldPlan {
    /// Canonical execution order, also used by function reachability visitors.
    pub fn predicates(&self) -> Vec<&RefinementPredicate> {
        let mut result = Vec::new();
        self.collect_predicates(&mut result);
        result
    }

    fn collect_predicates<'a>(&'a self, result: &mut Vec<&'a RefinementPredicate>) {
        match &self.action {
            ReflectedFieldAction::Exact | ReflectedFieldAction::Unsupported(_) => {}
            ReflectedFieldAction::Predicates(predicates) => result.extend(predicates),
            ReflectedFieldAction::Refine { base, predicates } => {
                base.collect_predicates(result);
                result.extend(predicates);
            }
            ReflectedFieldAction::List(child)
            | ReflectedFieldAction::Set(child)
            | ReflectedFieldAction::Optional(child) => child.collect_predicates(result),
            ReflectedFieldAction::Map { key, value } => {
                key.collect_predicates(result);
                value.collect_predicates(result);
            }
            ReflectedFieldAction::Result { ok, error } => {
                ok.collect_predicates(result);
                error.collect_predicates(result);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReflectedFieldUnsupported {
    Nested,
    Secret,
    CallableOrNominal,
}

impl ReflectedFieldUnsupported {
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
pub enum ReflectedFieldRequirement {
    Exact,
    Predicates(Vec<TypeId>),
    Unsupported(ReflectedFieldUnsupported),
    Refine {
        base: Box<ReflectedFieldRequirementPlan>,
        predicates: Vec<TypeId>,
    },
    List(Box<ReflectedFieldRequirementPlan>),
    Set(Box<ReflectedFieldRequirementPlan>),
    Map {
        key: Box<ReflectedFieldRequirementPlan>,
        value: Box<ReflectedFieldRequirementPlan>,
    },
    Optional(Box<ReflectedFieldRequirementPlan>),
    Result {
        ok: Box<ReflectedFieldRequirementPlan>,
        error: Box<ReflectedFieldRequirementPlan>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReflectedFieldRequirementPlan {
    pub source_type: TypeId,
    pub requested_type: TypeId,
    pub requirement: ReflectedFieldRequirement,
}

pub fn is_reflected_field_intrinsic(id: IntrinsicId) -> bool {
    matches!(
        id,
        IntrinsicId::TypeFieldValue
            | IntrinsicId::TypeVariantFieldValue
            | IntrinsicId::TypeMachineFieldValue
    )
}

/// A proof must call the declaration of that exact nominal refinement, not a
/// different predicate with the same carrier and bool signature.
pub fn refinement_predicate_declaration_matches(
    declaration: &DeclarationId,
    type_name: &str,
) -> bool {
    let (namespace, name) = type_name.rsplit_once('.').unwrap_or(("", type_name));
    declaration.kind == DeclarationKind::RefinementPredicate
        && declaration.namespace == namespace
        && declaration.name == name
}

pub fn valid_reflected_field_metadata_type(types: &TypeInterner, ty: TypeId) -> bool {
    if ty.index() as usize >= types.len() {
        return false;
    }
    let Type::Struct(id) = types.resolve(ty) else {
        return false;
    };
    let fields = &types.resolve_struct(*id).fields;
    let names = [
        "index",
        "owner_type",
        "owner_member",
        "name",
        "type_name",
        "kind",
        "kind_tag",
        "serialize_name",
        "has_secret",
        "type_info",
    ];
    types.resolve_struct(*id).name == "TypeField"
        && fields.iter().map(|(name, _)| name.as_str()).eq(names)
        && fields[0].1 == TypeInterner::INT64
        && [1, 3, 4, 5, 7]
            .into_iter()
            .all(|index| fields[index].1 == TypeInterner::STRING)
        && (fields[2].1.index() as usize) < types.len()
        && matches!(types.resolve(fields[2].1), Type::Optional(inner) if *inner == TypeInterner::STRING)
        && fields[8].1 == TypeInterner::BOOL
}

/// Flattened in the same order as checked hidden TypeField operands. Member
/// names are validated against the active enum/machine tag by the raw getter.
pub fn reflected_field_layout(
    types: &TypeInterner,
    id: IntrinsicId,
    owner: TypeId,
) -> Option<Vec<(Option<String>, usize, TypeId)>> {
    if owner.index() as usize >= types.len() {
        return None;
    }
    let fields = match (id, types.resolve(owner)) {
        (IntrinsicId::TypeFieldValue, Type::Struct(id)) => types
            .resolve_struct(*id)
            .fields
            .iter()
            .enumerate()
            .map(|(index, (_, ty))| (None, index, *ty))
            .collect(),
        (IntrinsicId::TypeFieldValue, Type::Bitfield(id)) => types
            .resolve_bitfield(*id)
            .fields
            .iter()
            .enumerate()
            .map(|(index, field)| (None, index, field.ty))
            .collect(),
        (IntrinsicId::TypeVariantFieldValue, Type::Enum(id)) => types
            .resolve_enum(*id)
            .variants
            .iter()
            .flat_map(|variant| {
                variant
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(index, (_, ty))| (Some(variant.name.clone()), index, *ty))
            })
            .collect(),
        (
            IntrinsicId::TypeMachineFieldValue,
            Type::Machine(id) | Type::MachineState { machine: id, .. },
        ) => types
            .resolve_machine(*id)
            .states
            .iter()
            .flat_map(|state| {
                state
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(index, (_, ty))| (Some(state.name.clone()), index, *ty))
            })
            .collect(),
        _ => return None,
    };
    Some(fields)
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
        // Named payload fields never make a requested nominal type contain a
        // new producer invariant. Only its declared generic arguments do.
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

fn refinement_chain(types: &TypeInterner, mut ty: TypeId) -> Option<(TypeId, Vec<TypeId>)> {
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

pub fn reflected_field_requirement(
    types: &TypeInterner,
    actual: TypeId,
    requested: TypeId,
) -> Option<ReflectedFieldRequirement> {
    requirement_node(types, actual, requested, types.len()).map(|node| node.requirement)
}

fn requirement_node(
    types: &TypeInterner,
    actual: TypeId,
    requested: TypeId,
    budget: usize,
) -> Option<ReflectedFieldRequirementPlan> {
    if budget == 0
        || actual.index() as usize >= types.len()
        || requested.index() as usize >= types.len()
    {
        return None;
    }
    let requirement = requirement_action(types, actual, requested, budget - 1)?;
    Some(ReflectedFieldRequirementPlan {
        source_type: actual,
        requested_type: requested,
        requirement,
    })
}

fn requirement_action(
    types: &TypeInterner,
    actual: TypeId,
    requested: TypeId,
    budget: usize,
) -> Option<ReflectedFieldRequirement> {
    use ReflectedFieldRequirement as R;
    if actual == requested || !contains_refinement(types, requested, &mut HashSet::new()) {
        return Some(R::Exact);
    }
    let (actual_base, actual_chain) = refinement_chain(types, actual)?;
    // Only the declared root chain establishes an ancestor proof. A changed
    // wrapper must still be ready even if every occupied child is already exact.
    if actual_chain.contains(&requested) {
        return Some(R::Exact);
    }
    // Keep the established non-builtin root-base contracts separate. A bare
    // ready builtin request with refined children has a structural preflight
    // even when the actual root declaration proves every child exactly.
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
        // Reuse the nearest declared requested ancestor, including a common
        // ancestor of sibling root branches. Its proof already establishes the
        // complete base, so a remaining root suffix needs no wrapper preflight.
        if let Some(index) = chain.iter().rposition(|ty| actual_chain.contains(ty)) {
            chain.drain(..=index);
            return Some(R::Predicates(chain));
        }
        // A new root over a generic base must keep the original declared source
        // spelling. Erasing its root here would turn changed-wrapper readiness
        // into a false whole-schema Exact before any outer predicate executes.
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
            (_, Type::Secret(_)) => R::Unsupported(ReflectedFieldUnsupported::Secret),
            _ => R::Unsupported(ReflectedFieldUnsupported::CallableOrNominal),
        },
    )
}

pub fn validate_reflected_field_plans(
    types: &TypeInterner,
    intrinsic: IntrinsicId,
    owner: TypeId,
    requested: TypeId,
    plans: &[ReflectedFieldPlan],
) -> Result<(), &'static str> {
    let layout = reflected_field_layout(types, intrinsic, owner)
        .ok_or("invalid reflected validation owner")?;
    if plans.len() != layout.len() {
        return Err("reflected validation plan count disagrees with field layout");
    }
    for ((_, _, actual), plan) in layout.iter().zip(plans) {
        if plan.source_type != *actual || plan.requested_type != requested {
            return Err("reflected validation types disagree with declared field and request");
        }
        let requirement = requirement_node(types, *actual, requested, types.len())
            .ok_or("invalid reflected validation type")?;
        validate_requirement_node(types, &requirement, plan)?;
    }
    Ok(())
}

fn validate_requirement_node(
    types: &TypeInterner,
    expected: &ReflectedFieldRequirementPlan,
    plan: &ReflectedFieldPlan,
) -> Result<(), &'static str> {
    use ReflectedFieldAction as A;
    use ReflectedFieldRequirement as R;
    if plan.source_type != expected.source_type || plan.requested_type != expected.requested_type {
        return Err("reflected validation child types disagree with checked schema");
    }
    match (&expected.requirement, &plan.action) {
        (R::Exact, A::Exact) => Ok(()),
        (R::Unsupported(a), A::Unsupported(b)) if a == b => Ok(()),
        (R::Predicates(expected), A::Predicates(predicates)) => {
            validate_plan_predicates(types, plan.requested_type, expected, predicates)
        }
        (
            R::Refine {
                base: expected,
                predicates: suffix,
            },
            A::Refine { base, predicates },
        ) => {
            validate_requirement_node(types, expected, base)?;
            validate_plan_predicates(types, plan.requested_type, suffix, predicates)
        }
        (R::List(a), A::List(b)) | (R::Set(a), A::Set(b)) | (R::Optional(a), A::Optional(b)) => {
            validate_requirement_node(types, a, b)
        }
        (R::Map { key: a, value: b }, A::Map { key: c, value: d })
        | (R::Result { ok: a, error: b }, A::Result { ok: c, error: d }) => {
            validate_requirement_node(types, a, c)?;
            validate_requirement_node(types, b, d)
        }
        _ => Err("reflected validation action disagrees with checked source"),
    }
}

fn validate_plan_predicates(
    types: &TypeInterner,
    requested: TypeId,
    expected: &[TypeId],
    predicates: &[RefinementPredicate],
) -> Result<(), &'static str> {
    if predicates
        .iter()
        .map(|p| p.refined_type)
        .ne(expected.iter().copied())
    {
        return Err("reflected validation predicate suffix disagrees with checked source");
    }
    let (base_type, _) =
        refinement_chain(types, requested).ok_or("invalid reflected refinement chain")?;
    let input_type = match types.resolve(base_type) {
        Type::Secret(inner) => *inner,
        _ => base_type,
    };
    if input_type.index() as usize >= types.len() {
        return Err("invalid reflected predicate input type");
    }
    for predicate in predicates {
        // Expected ids came from the checked chain, so a forged id can never
        // reach resolve without first failing the exact suffix comparison.
        let Type::Refinement { name, .. } = types.resolve(predicate.refined_type) else {
            return Err("reflected predicate is not a refinement");
        };
        if predicate.type_name != *name
            || predicate.base_type != base_type
            || predicate.input_type != input_type
        {
            return Err("reflected predicate metadata disagrees with declaration");
        }
    }
    Ok(())
}

impl BodyLowerer<'_, '_> {
    /// Reflected value observers use compiler metadata after their one source
    /// operand. A pipeline result type comes from its checked step signature;
    /// the step span itself describes the input, so never infer this from it.
    pub(super) fn append_reflected_value_arguments(
        &mut self,
        intrinsic: IntrinsicId,
        type_arguments: &[TypeId],
        reflection_arguments: &[ReflectionTypeInfo],
        args: &mut Vec<Expression>,
        evaluation_order: &mut Vec<usize>,
        result_type: Option<TypeId>,
        span: Span,
    ) -> Option<()> {
        if type_arguments.len() != 1 || args.len() != 1 {
            return Some(());
        }
        let Some(info) = reflection_arguments.first() else {
            return Some(());
        };
        let selected = match intrinsic {
            IntrinsicId::TypeVariantValue => info.kind == "enum",
            IntrinsicId::TypeMachineStateValue => {
                matches!(info.kind.as_str(), "machine" | "machine_state")
            }
            IntrinsicId::TypeArg => reflection_arguments.len() == 1,
            _ => false,
        };
        if !selected {
            return Some(());
        }
        let result_type = result_type?;
        let values: Vec<ExpressionKind> = match intrinsic {
            IntrinsicId::TypeVariantValue => {
                let Some(variants) = self
                    .parent
                    .check
                    .reflection_metadata
                    .get_type_variants_for_id(type_arguments[0])
                    .map(<[_]>::to_vec)
                else {
                    self.parent
                        .error(span, "type.variant_value has no checked variant metadata");
                    return None;
                };
                variants
                    .iter()
                    .map(|variant| {
                        self.lower_reflection_type_variant(
                            variant,
                            &info.type_name,
                            result_type,
                            span,
                        )
                    })
                    .collect::<Option<_>>()?
            }
            IntrinsicId::TypeMachineStateValue => {
                let machine = self.checked_reflection_machine(type_arguments[0], span)?;
                let owner_name = info
                    .type_name
                    .split_once(" at ")
                    .map_or(info.type_name.as_str(), |(base, _)| base);
                machine
                    .states
                    .iter()
                    .map(|state| {
                        self.lower_reflection_machine_state(state, owner_name, result_type, span)
                    })
                    .collect::<Option<_>>()?
            }
            IntrinsicId::TypeArg => info
                .args
                .iter()
                .map(|arg| self.lower_reflection_type_info(arg, result_type, span))
                .collect::<Option<_>>()?,
            _ => return Some(()),
        };
        for kind in values {
            evaluation_order.push(args.len());
            args.push(Expression {
                kind,
                ty: result_type,
                span,
            });
        }
        Some(())
    }

    /// Direct and piped getters share the same compiler-owned metadata tail.
    /// User operands keep their checked source order and run before this tail.
    pub(super) fn append_reflected_read_arguments(
        &mut self,
        intrinsic: IntrinsicId,
        type_arguments: &[TypeId],
        reflection_arguments: &[ReflectionTypeInfo],
        args: &mut Vec<Expression>,
        evaluation_order: &mut Vec<usize>,
        span: Span,
    ) -> Option<()> {
        if type_arguments.len() != 2 || args.len() != 2 {
            return Some(());
        }
        let Some(info) = reflection_arguments.first() else {
            return Some(());
        };
        let fields: Vec<(Option<String>, ReflectionFieldInfo)> = match intrinsic {
            IntrinsicId::TypeFieldValue if matches!(info.kind.as_str(), "struct" | "bitfield") => {
                let Some(fields) = self
                    .parent
                    .check
                    .reflection_metadata
                    .get_type_fields_for_id(type_arguments[0])
                else {
                    self.parent
                        .error(span, "type.field_value has no checked field metadata");
                    return None;
                };
                fields.iter().map(|field| (None, field.clone())).collect()
            }
            IntrinsicId::TypeVariantFieldValue if info.kind == "enum" => {
                let Some(variants) = self
                    .parent
                    .check
                    .reflection_metadata
                    .get_type_variants_for_id(type_arguments[0])
                else {
                    self.parent.error(
                        span,
                        "type.variant_field_value has no checked variant metadata",
                    );
                    return None;
                };
                variants
                    .iter()
                    .flat_map(|variant| {
                        variant
                            .fields
                            .iter()
                            .map(|field| (Some(variant.name.clone()), field.clone()))
                    })
                    .collect()
            }
            IntrinsicId::TypeMachineFieldValue
                if matches!(info.kind.as_str(), "machine" | "machine_state") =>
            {
                let machine = self.checked_reflection_machine(type_arguments[0], span)?;
                machine
                    .states
                    .iter()
                    .flat_map(|state| {
                        state
                            .fields
                            .iter()
                            .map(|field| (Some(state.name.clone()), field.clone()))
                    })
                    .collect()
            }
            _ => return Some(()),
        };
        let owner_name = if intrinsic == IntrinsicId::TypeMachineFieldValue {
            info.type_name
                .split_once(" at ")
                .map_or(info.type_name.as_str(), |(base, _)| base)
        } else {
            &info.type_name
        };
        let field_type = args[1].ty;
        for (member, field) in fields {
            let kind = self.lower_reflection_type_field(
                &field,
                owner_name,
                member.as_deref(),
                field_type,
                span,
            )?;
            evaluation_order.push(args.len());
            args.push(Expression {
                kind,
                ty: field_type,
                span,
            });
        }
        Some(())
    }

    pub(super) fn reflected_read_validation(
        &mut self,
        intrinsic: IntrinsicId,
        type_arguments: &[TypeId],
        span: Span,
    ) -> Option<Option<ReflectedFieldValidation>> {
        if !is_reflected_field_intrinsic(intrinsic) {
            return Some(None);
        }
        let [owner, requested] = type_arguments else {
            self.parent
                .error(span, "reflected read has no exact checked type operands");
            return None;
        };
        let Some(layout) = reflected_field_layout(&self.parent.check.interner, intrinsic, *owner)
        else {
            self.parent
                .error(span, "reflected read has no checked field layout");
            return None;
        };
        let mut plans = Vec::with_capacity(layout.len());
        for (_, _, source_type) in layout {
            let Some(requirement) = requirement_node(
                &self.parent.check.interner,
                source_type,
                *requested,
                self.parent.check.interner.len(),
            ) else {
                self.parent
                    .error(span, "reflected read has invalid proof types");
                return None;
            };
            plans.push(self.lower_reflected_requirement(requirement, span)?);
        }
        Some(Some(ReflectedFieldValidation::Validate(plans)))
    }

    fn lower_reflected_requirement(
        &mut self,
        node: ReflectedFieldRequirementPlan,
        span: Span,
    ) -> Option<ReflectedFieldPlan> {
        use ReflectedFieldAction as A;
        use ReflectedFieldRequirement as R;
        let action = match node.requirement {
            R::Exact => A::Exact,
            R::Unsupported(reason) => A::Unsupported(reason),
            R::Predicates(expected) => A::Predicates(self.reflected_predicate_suffix(
                node.requested_type,
                &expected,
                span,
            )?),
            R::Refine { base, predicates } => A::Refine {
                base: Box::new(self.lower_reflected_requirement(*base, span)?),
                predicates: self.reflected_predicate_suffix(
                    node.requested_type,
                    &predicates,
                    span,
                )?,
            },
            R::List(child) => A::List(Box::new(self.lower_reflected_requirement(*child, span)?)),
            R::Set(child) => A::Set(Box::new(self.lower_reflected_requirement(*child, span)?)),
            R::Optional(child) => {
                A::Optional(Box::new(self.lower_reflected_requirement(*child, span)?))
            }
            R::Map { key, value } => A::Map {
                key: Box::new(self.lower_reflected_requirement(*key, span)?),
                value: Box::new(self.lower_reflected_requirement(*value, span)?),
            },
            R::Result { ok, error } => A::Result {
                ok: Box::new(self.lower_reflected_requirement(*ok, span)?),
                error: Box::new(self.lower_reflected_requirement(*error, span)?),
            },
        };
        Some(ReflectedFieldPlan {
            source_type: node.source_type,
            requested_type: node.requested_type,
            action,
        })
    }

    fn reflected_predicate_suffix(
        &mut self,
        requested: TypeId,
        expected: &[TypeId],
        span: Span,
    ) -> Option<Vec<RefinementPredicate>> {
        let mut predicates = self.checked_refinement_predicates(requested, span)?;
        predicates.retain(|predicate| expected.contains(&predicate.refined_type));
        if predicates
            .iter()
            .map(|p| p.refined_type)
            .ne(expected.iter().copied())
        {
            self.parent
                .error(span, "reflected read has no canonical predicate suffix");
            return None;
        }
        Some(predicates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jett_diagnostics::Severity;

    const SOURCE: &str = r#"namespace app
type Positive = int64 where value > 0
type Higher = Positive where value > 10
type Sibling = int64 where value >= 0
struct Record:
    raw: int64
    positive: Positive
    higher: Higher
    sibling: Sibling
enum Event:
    first(raw: int64, positive: Positive, higher: Higher, sibling: Sibling)
    second(higher: Higher, raw: int64)
machine Session:
    states:
        first(raw: int64, positive: Positive, higher: Higher, sibling: Sibling)
        second(higher: Higher, raw: int64)
    transitions:
        first to second
function record_read(view value: Record, view field: TypeField) returns Higher:
    return type.field_value[Record, Higher](view value, view field)
function event_read(view value: Event, view field: TypeField) returns Higher:
    return type.variant_field_value[Event, Higher](view value, view field)
function session_read(view value: Session, view field: TypeField) returns Higher:
    return type.machine_field_value[Session, Higher](view value, view field)
"#;

    fn lower_source() -> (Program, TypeInterner) {
        lower_source_text(SOURCE)
    }

    fn lower_source_text(source: &str) -> (Program, TypeInterner) {
        let file = FileId::new(0);
        let parsed = jett_parser::parse(source, file);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let resolved = jett_resolve::resolve(&parsed.module);
        let checked = jett_typecheck::check(&parsed.module, &resolved);
        assert!(
            checked
                .diagnostics
                .iter()
                .all(|d| d.severity != Severity::Error),
            "{:?}",
            checked.diagnostics
        );
        let origins = HashMap::from([(file, SourceOrigin::Project)]);
        let program = lower(&parsed.module, &resolved, &checked, &origins).expect("reflected HIR");
        (program, checked.interner)
    }

    fn read_mut(program: &mut Program) -> &mut Expression {
        let function = program
            .functions
            .iter_mut()
            .find(|f| f.identity.declaration.name == "record_read")
            .unwrap();
        let StatementKind::Return(Some(value)) = &mut function.body.statements[0].kind else {
            panic!("reflected return");
        };
        value
    }

    #[test]
    fn native_reflected_root_plans_use_declared_slots_and_only_new_suffixes() {
        let (program, types) = lower_source();
        validate(&program).unwrap();
        validate_backend_types(&program, &types).unwrap();
        for name in ["record_read", "event_read", "session_read"] {
            let function = program
                .functions
                .iter()
                .find(|f| f.identity.declaration.name == name)
                .unwrap();
            let StatementKind::Return(Some(Expression {
                kind:
                    ExpressionKind::Intrinsic {
                        intrinsic,
                        type_arguments,
                        field_validation: Some(ReflectedFieldValidation::Validate(plans)),
                        ..
                    },
                ..
            })) = &function.body.statements[0].kind
            else {
                panic!("checked reflected plans");
            };
            let layout = reflected_field_layout(&types, *intrinsic, type_arguments[0]).unwrap();
            assert_eq!(
                plans.iter().map(|p| p.source_type).collect::<Vec<_>>(),
                layout.iter().map(|(_, _, ty)| *ty).collect::<Vec<_>>()
            );
            let expected = if name == "record_read" {
                vec![
                    vec!["app.Positive", "app.Higher"],
                    vec!["app.Higher"],
                    vec![],
                    vec!["app.Positive", "app.Higher"],
                ]
            } else {
                vec![
                    vec!["app.Positive", "app.Higher"],
                    vec!["app.Higher"],
                    vec![],
                    vec!["app.Positive", "app.Higher"],
                    vec![],
                    vec!["app.Positive", "app.Higher"],
                ]
            };
            let actual = plans
                .iter()
                .map(|p| match &p.action {
                    ReflectedFieldAction::Exact => Vec::new(),
                    ReflectedFieldAction::Predicates(chain) => {
                        chain.iter().map(|p| p.type_name.as_str()).collect()
                    }
                    _ => panic!("root scalar is supported"),
                })
                .collect::<Vec<Vec<_>>>();
            assert_eq!(actual, expected, "{name}");
        }
    }

    #[test]
    fn native_reflected_root_hir_rejects_incomplete_or_forged_proof_plans() {
        let (original, types) = lower_source();
        let sibling = original
            .functions
            .iter()
            .find(|f| {
                f.identity.declaration.kind == DeclarationKind::RefinementPredicate
                    && f.identity.declaration.name == "Sibling"
            })
            .unwrap()
            .id;
        for mutation in 0..12 {
            let mut program = original.clone();
            let value = read_mut(&mut program);
            let ExpressionKind::Intrinsic {
                type_arguments,
                field_validation,
                ..
            } = &mut value.kind
            else {
                panic!("reflected read");
            };
            let Some(ReflectedFieldValidation::Validate(plans)) = field_validation else {
                panic!("proof plans");
            };
            match mutation {
                0 => {
                    plans.pop();
                }
                1 => plans[0].source_type = TypeInterner::BOOL,
                2 => plans[0].action = ReflectedFieldAction::Exact,
                3 => {
                    let ReflectedFieldAction::Predicates(p) = &mut plans[0].action else {
                        unreachable!();
                    };
                    p[0].type_name = "Positive".into();
                }
                4 => {
                    let ReflectedFieldAction::Predicates(p) = &mut plans[0].action else {
                        unreachable!();
                    };
                    p[0].input_type = TypeInterner::STRING;
                }
                5 => {
                    let ReflectedFieldAction::Predicates(p) = &mut plans[0].action else {
                        unreachable!();
                    };
                    p[0].function = sibling;
                }
                6 => {
                    let ReflectedFieldAction::Predicates(p) = &mut plans[0].action else {
                        unreachable!();
                    };
                    p[0].function = FunctionId::new(u32::MAX);
                }
                7 => *field_validation = None,
                8 => type_arguments[1] = TypeInterner::INT64,
                9 => *field_validation = Some(ReflectedFieldValidation::Read),
                10 => {
                    let ReflectedFieldAction::Predicates(p) = &mut plans[0].action else {
                        unreachable!();
                    };
                    p[0].base_type = TypeInterner::BOOL;
                }
                11 => {
                    let ReflectedFieldAction::Predicates(p) = &mut plans[0].action else {
                        unreachable!();
                    };
                    p.pop();
                }
                _ => unreachable!(),
            }
            let structural = validate(&program).is_err();
            let typed = validate_backend_types(&program, &types).is_err();
            assert!(
                structural || typed,
                "forged plan {mutation} escaped validation"
            );
            if mutation == 5 || mutation == 6 {
                assert!(structural, "wrong predicate identity is structural");
            }
        }
    }

    #[test]
    fn native_reflected_root_metadata_requires_canonical_names_and_used_slot_types() {
        for mutation in 0..6 {
            let (program, mut types) = lower_source();
            let function = program
                .functions
                .iter()
                .find(|f| f.identity.declaration.name == "record_read")
                .unwrap();
            let metadata_type = function.params[1].ty;
            assert!(valid_reflected_field_metadata_type(&types, metadata_type));
            let Type::Struct(id) = types.resolve(metadata_type) else {
                panic!("TypeField descriptor");
            };
            let id = *id;
            let mut definition = types.resolve_struct(id).clone();
            match mutation {
                0 => definition.fields[1].0 = "owner".into(),
                1 => definition.fields[1].1 = TypeInterner::INT64,
                2 => definition.fields[2].1 = types.intern(Type::Optional(TypeInterner::INT64)),
                3 => definition.fields[3].1 = TypeInterner::BOOL,
                4 => {
                    definition.fields.pop();
                }
                5 => definition.fields[0].1 = TypeInterner::BOOL,
                _ => unreachable!(),
            }
            types.update_struct(id, definition);
            assert!(
                !valid_reflected_field_metadata_type(&types, metadata_type),
                "metadata mutation {mutation}"
            );
        }
    }

    #[test]
    fn producer_nominal_argument_detection_never_infers_arguments_from_payload_fields() {
        let mut types = TypeInterner::new();
        let positive = types.intern(Type::Refinement {
            name: "models.Positive".into(),
            base: TypeInterner::INT64,
        });
        let element = types.add_struct(jett_types::StructDef {
            name: "models.Element".into(),
            fields: vec![("value".into(), positive)],
            methods: Vec::new(),
        });
        let element = types.intern(Type::Struct(element));
        let event = types.add_enum(jett_types::EnumDef {
            name: "models.Event".into(),
            variants: vec![jett_types::VariantDef {
                name: "content".into(),
                fields: vec![("value".into(), positive)],
                discriminant: 0,
            }],
        });
        let event = types.intern(Type::Enum(event));
        let machine = types.add_machine(jett_types::MachineDef {
            name: "models.Session".into(),
            states: vec![jett_types::MachineStateDef {
                name: "content".into(),
                fields: vec![("value".into(), positive)],
            }],
            transitions: Vec::new(),
        });
        let state = types.intern(Type::MachineState {
            machine,
            state: jett_types::MachineStateId::new(0),
        });
        let machine = types.intern(Type::Machine(machine));
        let bitfield = types.add_bitfield(jett_types::BitfieldDef {
            name: "models.Packet".into(),
            network_order: false,
            fields: vec![jett_types::BitfieldFieldDef {
                name: "value".into(),
                ty: positive,
                kind: jett_types::BitfieldFieldKind::Payload,
            }],
        });
        let bitfield = types.intern(Type::Bitfield(bitfield));
        for leaf in [element, event, machine, state, bitfield] {
            assert!(types.nominal_type_arguments(leaf).is_empty());
            assert!(!contains_refinement(&types, leaf, &mut HashSet::new()));
        }
        let marker = types.add_struct(jett_types::StructDef {
            name: "models.Marker[models.Positive]".into(),
            fields: vec![("value".into(), TypeInterner::INT64)],
            methods: Vec::new(),
        });
        let marker = types.intern(Type::Struct(marker));
        types
            .register_nominal_type_arguments(marker, vec![positive])
            .unwrap();
        let plain_marker = types.add_struct(jett_types::StructDef {
            name: "models.Marker[int64]".into(),
            fields: vec![("value".into(), TypeInterner::INT64)],
            methods: Vec::new(),
        });
        let plain_marker = types.intern(Type::Struct(plain_marker));
        types
            .register_nominal_type_arguments(plain_marker, vec![TypeInterner::INT64])
            .unwrap();
        for (leaf, requires_ready) in [(element, false), (marker, true), (plain_marker, false)] {
            let list = types.intern(Type::List(leaf));
            let root = types.intern(Type::Refinement {
                name: format!("models.Values{}", leaf.index()),
                base: list,
            });
            let requirement = reflected_field_requirement(&types, root, list).unwrap();
            if requires_ready {
                assert_eq!(
                    requirement,
                    ReflectedFieldRequirement::List(Box::new(ReflectedFieldRequirementPlan {
                        source_type: leaf,
                        requested_type: leaf,
                        requirement: ReflectedFieldRequirement::Exact,
                    }))
                );
            } else {
                assert_eq!(requirement, ReflectedFieldRequirement::Exact);
            }
        }
        assert_eq!(
            reflected_field_requirement(&types, plain_marker, marker),
            Some(ReflectedFieldRequirement::Unsupported(
                ReflectedFieldUnsupported::CallableOrNominal
            ))
        );
    }

    fn nominal_leaf_source(leaf: &str) -> String {
        format!(
            r#"
namespace app
type Positive = int64 where value > 0
type Alias = Positive
struct Element:
    value: Positive
struct Holder[T]:
    value: T
struct Unused[T]:
    value: int64
type Values = list[{leaf}] where true
struct Record:
    values: Values
enum Event:
    content(values: Values)
machine Session:
    states:
        content(values: Values)
        empty
    transitions:
        content to empty
function record_read(view source: Record, view field: TypeField) returns list[{leaf}]:
    return type.field_value[Record, list[{leaf}]](view source, view field)
function record_pipe(view source: Record, view field: TypeField) returns list[{leaf}]:
    return source into view type.field_value[Record, list[{leaf}]](view field)
function event_read(view source: Event, view field: TypeField) returns list[{leaf}]:
    return type.variant_field_value[Event, list[{leaf}]](view source, view field)
function event_pipe(view source: Event, view field: TypeField) returns list[{leaf}]:
    return source into view type.variant_field_value[Event, list[{leaf}]](view field)
function machine_read(view source: Session, view field: TypeField) returns list[{leaf}]:
    return type.machine_field_value[Session, list[{leaf}]](view source, view field)
function machine_pipe(view source: Session, view field: TypeField) returns list[{leaf}]:
    return source into view type.machine_field_value[Session, list[{leaf}]](view field)
"#
        )
    }

    #[test]
    fn checked_nominal_leaf_plans_preserve_ordinary_proof_and_unused_generic_invariants() {
        for (leaf, requires_ready) in [
            ("Element", false),
            ("Holder[Positive]", true),
            ("Unused[Alias]", true),
            ("Unused[int64]", false),
            ("Holder[Element]", false),
        ] {
            let (mut program, types) = lower_source_text(&nominal_leaf_source(leaf));
            validate_backend_types(&program, &types).unwrap();
            for name in [
                "record_read",
                "record_pipe",
                "event_read",
                "event_pipe",
                "machine_read",
                "machine_pipe",
            ] {
                let function = program
                    .functions
                    .iter()
                    .find(|f| f.identity.declaration.name == name)
                    .unwrap();
                let StatementKind::Return(Some(Expression {
                    kind:
                        ExpressionKind::Intrinsic {
                            field_validation: Some(ReflectedFieldValidation::Validate(plans)),
                            ..
                        },
                    ..
                })) = &function.body.statements[0].kind
                else {
                    panic!("{name} nominal leaf plan");
                };
                assert_eq!(plans.len(), 1);
                assert!(
                    plans[0].predicates().is_empty(),
                    "no proven predicates rerun"
                );
                if requires_ready {
                    let ReflectedFieldAction::List(child) = &plans[0].action else {
                        panic!("{name}: {leaf} requires readiness");
                    };
                    assert_eq!(child.action, ReflectedFieldAction::Exact);
                    assert_eq!(child.source_type, child.requested_type);
                } else {
                    assert_eq!(
                        plans[0].action,
                        ReflectedFieldAction::Exact,
                        "{name}: {leaf} stays exact"
                    );
                }
            }
            // Closed IR must reject both adding readiness to a plain nominal
            // leaf and deleting it when a declared generic argument is refined.
            let read = read_mut(&mut program);
            let ExpressionKind::Intrinsic {
                field_validation: Some(ReflectedFieldValidation::Validate(plans)),
                ..
            } = &mut read.kind
            else {
                panic!("record plan");
            };
            if requires_ready {
                plans[0].action = ReflectedFieldAction::Exact;
            } else {
                let Type::List(child) = types.resolve(plans[0].requested_type) else {
                    panic!("requested list");
                };
                plans[0].action = ReflectedFieldAction::List(Box::new(ReflectedFieldPlan {
                    source_type: *child,
                    requested_type: *child,
                    action: ReflectedFieldAction::Exact,
                }));
            }
            let errors =
                validate_backend_types(&program, &types).expect_err("forged readiness plan");
            assert!(
                errors.iter().any(|error| error
                    .message
                    .contains("reflected validation action disagrees")),
                "{leaf}: {errors:?}"
            );
        }
    }

    #[test]
    fn native_reflected_root_schema_reuses_ancestors_and_keeps_nested_base_proofs() {
        let mut types = TypeInterner::new();
        let positive = types.intern(Type::Refinement {
            name: "app.Positive".into(),
            base: TypeInterner::INT64,
        });
        let higher = types.intern(Type::Refinement {
            name: "app.Higher".into(),
            base: positive,
        });
        assert_eq!(
            reflected_field_requirement(&types, higher, positive),
            Some(ReflectedFieldRequirement::Exact)
        );
        assert_eq!(
            reflected_field_requirement(&types, higher, TypeInterner::INT64),
            Some(ReflectedFieldRequirement::Exact)
        );
        let sibling_high = types.intern(Type::Refinement {
            name: "app.SiblingHigh".into(),
            base: positive,
        });
        assert_eq!(
            reflected_field_requirement(&types, higher, sibling_high),
            Some(ReflectedFieldRequirement::Predicates(vec![sibling_high]))
        );
        let list = types.intern(Type::List(positive));
        let nonempty = types.intern(Type::Refinement {
            name: "app.Nonempty".into(),
            base: list,
        });
        assert_eq!(
            reflected_field_requirement(&types, list, nonempty),
            Some(ReflectedFieldRequirement::Predicates(vec![nonempty]))
        );
        assert_eq!(
            reflected_field_requirement(&types, nonempty, list),
            Some(ReflectedFieldRequirement::List(Box::new(
                ReflectedFieldRequirementPlan {
                    source_type: positive,
                    requested_type: positive,
                    requirement: ReflectedFieldRequirement::Exact,
                }
            )))
        );
        let plain_list = types.intern(Type::List(TypeInterner::INT64));
        assert_eq!(
            reflected_field_requirement(&types, plain_list, nonempty),
            Some(ReflectedFieldRequirement::Refine {
                base: Box::new(ReflectedFieldRequirementPlan {
                    source_type: plain_list,
                    requested_type: list,
                    requirement: ReflectedFieldRequirement::List(Box::new(
                        ReflectedFieldRequirementPlan {
                            source_type: TypeInterner::INT64,
                            requested_type: positive,
                            requirement: ReflectedFieldRequirement::Predicates(vec![positive]),
                        }
                    )),
                }),
                predicates: vec![nonempty],
            })
        );
        let secret = types.intern(Type::Secret(positive));
        let hidden = types.intern(Type::Refinement {
            name: "app.Hidden".into(),
            base: secret,
        });
        assert_eq!(
            reflected_field_requirement(&types, secret, hidden),
            Some(ReflectedFieldRequirement::Predicates(vec![hidden]))
        );
        assert_eq!(
            reflected_field_requirement(&types, hidden, secret),
            Some(ReflectedFieldRequirement::Exact)
        );
        let plain_secret = types.intern(Type::Secret(TypeInterner::INT64));
        assert_eq!(
            reflected_field_requirement(&types, plain_secret, hidden),
            Some(ReflectedFieldRequirement::Unsupported(
                ReflectedFieldUnsupported::Secret
            ))
        );
        let callback = types.intern(Type::Function {
            params: vec![],
            view_params: vec![],
            return_type: positive,
        });
        let plain_callback = types.intern(Type::Function {
            params: vec![],
            view_params: vec![],
            return_type: TypeInterner::INT64,
        });
        assert_eq!(
            reflected_field_requirement(&types, plain_callback, callback),
            Some(ReflectedFieldRequirement::Unsupported(
                ReflectedFieldUnsupported::CallableOrNominal
            ))
        );
    }

    fn requirement_children(
        requirement: &ReflectedFieldRequirement,
    ) -> Vec<&ReflectedFieldRequirementPlan> {
        match requirement {
            ReflectedFieldRequirement::List(child)
            | ReflectedFieldRequirement::Set(child)
            | ReflectedFieldRequirement::Optional(child) => vec![child],
            ReflectedFieldRequirement::Map { key, value } => vec![key, value],
            ReflectedFieldRequirement::Result { ok, error } => vec![ok, error],
            _ => panic!("changed builtin wrapper"),
        }
    }

    fn requirement_wrapper(types: &mut TypeInterner, kind: usize, child: TypeId) -> TypeId {
        types.intern(match kind {
            0 => Type::List(child),
            1 => Type::Set(child),
            2 => Type::Map(child, child),
            3 => Type::Optional(child),
            4 => Type::Result(child, child),
            _ => panic!("test wrapper kind"),
        })
    }

    #[test]
    fn native_reflected_recursive_requirements_keep_wrapper_readiness_and_leaf_provenance() {
        let mut types = TypeInterner::new();
        let positive = types.intern(Type::Refinement {
            name: "app.Positive".into(),
            base: TypeInterner::INT64,
        });
        let higher = types.intern(Type::Refinement {
            name: "app.Higher".into(),
            base: positive,
        });
        let sibling = types.intern(Type::Refinement {
            name: "app.Sibling".into(),
            base: TypeInterner::INT64,
        });
        for kind in 0..5 {
            let requested = requirement_wrapper(&mut types, kind, positive);
            assert_eq!(
                reflected_field_requirement(&types, requested, requested),
                Some(ReflectedFieldRequirement::Exact)
            );
            for (source, expected) in [
                (higher, ReflectedFieldRequirement::Exact),
                (
                    sibling,
                    ReflectedFieldRequirement::Predicates(vec![positive]),
                ),
                (
                    TypeInterner::INT64,
                    ReflectedFieldRequirement::Predicates(vec![positive]),
                ),
            ] {
                let actual = requirement_wrapper(&mut types, kind, source);
                let requirement = reflected_field_requirement(&types, actual, requested).unwrap();
                for child in requirement_children(&requirement) {
                    assert_eq!(child.source_type, source);
                    assert_eq!(child.requested_type, positive);
                    assert_eq!(child.requirement, expected);
                }
            }
        }
        let plain_secret = types.intern(Type::Secret(TypeInterner::INT64));
        let refined_secret = types.intern(Type::Secret(positive));
        let actual = types.intern(Type::Optional(plain_secret));
        let requested = types.intern(Type::Optional(refined_secret));
        let requirement = reflected_field_requirement(&types, actual, requested).unwrap();
        assert_eq!(
            requirement_children(&requirement)[0].requirement,
            ReflectedFieldRequirement::Unsupported(ReflectedFieldUnsupported::Secret)
        );
        let plain_callable = types.intern(Type::Function {
            params: vec![],
            view_params: vec![],
            return_type: TypeInterner::INT64,
        });
        let refined_callable = types.intern(Type::Function {
            params: vec![],
            view_params: vec![],
            return_type: positive,
        });
        let actual = types.intern(Type::List(plain_callable));
        let requested = types.intern(Type::List(refined_callable));
        let requirement = reflected_field_requirement(&types, actual, requested).unwrap();
        assert_eq!(
            requirement_children(&requirement)[0].requirement,
            ReflectedFieldRequirement::Unsupported(ReflectedFieldUnsupported::CallableOrNominal)
        );
    }

    #[test]
    fn native_reflected_root_to_builtin_base_checks_readiness_without_repeating_child_proofs() {
        let mut types = TypeInterner::new();
        let positive = types.intern(Type::Refinement {
            name: "app.Positive".into(),
            base: TypeInterner::INT64,
        });
        for kind in 0..5 {
            let base = requirement_wrapper(&mut types, kind, positive);
            let named = types.intern(Type::Refinement {
                name: format!("app.Wrapper{kind}"),
                base,
            });
            let higher = types.intern(Type::Refinement {
                name: format!("app.HigherWrapper{kind}"),
                base: named,
            });
            let sibling = types.intern(Type::Refinement {
                name: format!("app.SiblingWrapper{kind}"),
                base: named,
            });
            let independent = types.intern(Type::Refinement {
                name: format!("app.IndependentWrapper{kind}"),
                base,
            });
            let plain = requirement_wrapper(&mut types, kind, TypeInterner::INT64);
            // Plan shape is independent of runtime pending depth. Exact named
            // and generic requests and no-refinement requests skip preflight,
            // retaining the original pending owner rather than joining it.
            for (actual, requested) in [
                (named, named),
                (higher, named),
                (base, base),
                (named, plain),
            ] {
                assert_eq!(
                    reflected_field_requirement(&types, actual, requested),
                    Some(ReflectedFieldRequirement::Exact)
                );
            }
            assert_eq!(
                reflected_field_requirement(&types, named, higher),
                Some(ReflectedFieldRequirement::Predicates(vec![higher]))
            );
            assert_eq!(
                reflected_field_requirement(&types, higher, sibling),
                Some(ReflectedFieldRequirement::Predicates(vec![sibling]))
            );
            for actual in [named, higher] {
                let ReflectedFieldRequirement::Refine {
                    base: ready,
                    predicates,
                } = reflected_field_requirement(&types, actual, independent).unwrap()
                else {
                    panic!("new root must preflight generic base");
                };
                assert_eq!(ready.source_type, actual);
                assert_eq!(ready.requested_type, base);
                assert_eq!(predicates, [independent]);
                for child in requirement_children(&ready.requirement) {
                    assert_eq!(child.requirement, ReflectedFieldRequirement::Exact);
                }
                let requirement = reflected_field_requirement(&types, actual, base).unwrap();
                for child in requirement_children(&requirement) {
                    assert_eq!(child.source_type, positive);
                    assert_eq!(child.requested_type, positive);
                    assert_eq!(child.requirement, ReflectedFieldRequirement::Exact);
                }
            }
        }
    }

    const RECURSIVE_SOURCE: &str = r#"namespace app
 type Positive = int64 where value > 0
 type Higher = Positive where value > 10
 type Sibling = int64 where value >= 0
 type Text = string where value != ""
 function is_nonempty(view values: list[Positive]) returns bool:
     for item in view values:
         return true
     return false
 type Nonempty = list[Positive] where is_nonempty(view value)
 type Larger = Nonempty where true
 type AlternateLarger = Nonempty where true
 type OtherValues = list[Positive] where true
 struct RecursiveRecord:
     raw: list[optional[result[map[int64, list[int64]], set[int64]]]]
     known: list[optional[result[map[Higher, list[Higher]], set[Higher]]]]
 enum RecursiveEvent:
     content(raw: list[optional[result[map[int64, list[int64]], set[int64]]]])
     known(value: list[optional[result[map[Higher, list[Higher]], set[Higher]]]])
 machine RecursiveSession:
     states:
         content(raw: list[optional[result[map[int64, list[int64]], set[int64]]]])
         known(value: list[optional[result[map[Higher, list[Higher]], set[Higher]]]])
     transitions:
         content to known
 struct TextRecord:
     raw: optional[string]
 struct RootRecord:
     raw: list[int64]
 struct ProvenRootRecord:
     established: Nonempty
 struct HigherRootRecord:
     established: Larger
 function record_read(view value: RecursiveRecord, view field: TypeField) returns list[optional[result[map[Positive, list[Positive]], set[Positive]]]]:
     return type.field_value[RecursiveRecord, list[optional[result[map[Positive, list[Positive]], set[Positive]]]]](view value, view field)
 function record_pipe(view value: RecursiveRecord, view field: TypeField) returns list[optional[result[map[Positive, list[Positive]], set[Positive]]]]:
     return value into view type.field_value[RecursiveRecord, list[optional[result[map[Positive, list[Positive]], set[Positive]]]]](view field)
 function event_read(view value: RecursiveEvent, view field: TypeField) returns list[optional[result[map[Positive, list[Positive]], set[Positive]]]]:
     return type.variant_field_value[RecursiveEvent, list[optional[result[map[Positive, list[Positive]], set[Positive]]]]](view value, view field)
 function event_pipe(view value: RecursiveEvent, view field: TypeField) returns list[optional[result[map[Positive, list[Positive]], set[Positive]]]]:
     return value into view type.variant_field_value[RecursiveEvent, list[optional[result[map[Positive, list[Positive]], set[Positive]]]]](view field)
 function session_read(view value: RecursiveSession, view field: TypeField) returns list[optional[result[map[Positive, list[Positive]], set[Positive]]]]:
     return type.machine_field_value[RecursiveSession, list[optional[result[map[Positive, list[Positive]], set[Positive]]]]](view value, view field)
 function session_pipe(view value: RecursiveSession, view field: TypeField) returns list[optional[result[map[Positive, list[Positive]], set[Positive]]]]:
     return value into view type.machine_field_value[RecursiveSession, list[optional[result[map[Positive, list[Positive]], set[Positive]]]]](view field)
 function text_read(view value: TextRecord, view field: TypeField) returns optional[Text]:
     return type.field_value[TextRecord, optional[Text]](view value, view field)
 function root_read(view value: RootRecord, view field: TypeField) returns Nonempty:
     return type.field_value[RootRecord, Nonempty](view value, view field)
 function named_root_read(view value: ProvenRootRecord, view field: TypeField) returns Nonempty:
     return type.field_value[ProvenRootRecord, Nonempty](view value, view field)
 function generic_root_base_read(view value: ProvenRootRecord, view field: TypeField) returns list[Positive]:
     return type.field_value[ProvenRootRecord, list[Positive]](view value, view field)
 function plain_root_base_read(view value: ProvenRootRecord, view field: TypeField) returns list[int64]:
     return type.field_value[ProvenRootRecord, list[int64]](view value, view field)
 function promoted_root_read(view value: ProvenRootRecord, view field: TypeField) returns Larger:
     return type.field_value[ProvenRootRecord, Larger](view value, view field)
 function independent_root_read(view value: ProvenRootRecord, view field: TypeField) returns OtherValues:
     return type.field_value[ProvenRootRecord, OtherValues](view value, view field)
 function shared_root_read(view value: HigherRootRecord, view field: TypeField) returns AlternateLarger:
     return type.field_value[HigherRootRecord, AlternateLarger](view value, view field)
"#;

    fn recursive_source() -> String {
        // Keep this long type matrix readable without changing Jett indentation.
        RECURSIVE_SOURCE
            .lines()
            .map(|line| line.strip_prefix(' ').unwrap_or(line))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn plans(value: &Expression) -> &[ReflectedFieldPlan] {
        let ExpressionKind::Intrinsic {
            field_validation: Some(ReflectedFieldValidation::Validate(plans)),
            ..
        } = &value.kind
        else {
            panic!("reflected plans");
        };
        plans
    }

    #[test]
    fn native_reflected_recursive_plans_preserve_builtin_shapes_declared_proofs_and_call_forms() {
        let (program, types) = lower_source_text(&recursive_source());
        validate(&program).unwrap();
        validate_backend_types(&program, &types).unwrap();
        for prefix in ["record", "event", "session"] {
            let direct = plans(returned_read(&program, &format!("{prefix}_read")));
            let piped = plans(returned_read(&program, &format!("{prefix}_pipe")));
            assert_eq!(direct, piped);
            let ReflectedFieldAction::List(optional) = &direct[0].action else {
                panic!("list");
            };
            let ReflectedFieldAction::Optional(result) = &optional.action else {
                panic!("optional");
            };
            let ReflectedFieldAction::Result { ok, error } = &result.action else {
                panic!("result");
            };
            let ReflectedFieldAction::Map { key, value } = &ok.action else {
                panic!("map");
            };
            assert!(matches!(key.action, ReflectedFieldAction::Predicates(_)));
            assert!(matches!(value.action, ReflectedFieldAction::List(_)));
            assert!(matches!(error.action, ReflectedFieldAction::Set(_)));
            assert_eq!(
                direct[0]
                    .predicates()
                    .iter()
                    .map(|p| p.type_name.as_str())
                    .collect::<Vec<_>>(),
                ["app.Positive", "app.Positive", "app.Positive"]
            );
            // A changed whole wrapper retains readiness even when all leaves
            // are declared Higher values read through established Positive.
            assert!(matches!(direct[1].action, ReflectedFieldAction::List(_)));
            assert!(direct[1].predicates().is_empty());
        }
        let named = &plans(returned_read(&program, "named_root_read"))[0];
        let generic = &plans(returned_read(&program, "generic_root_base_read"))[0];
        let plain = &plans(returned_read(&program, "plain_root_base_read"))[0];
        assert!(matches!(named.action, ReflectedFieldAction::Exact));
        assert!(matches!(plain.action, ReflectedFieldAction::Exact));
        let ReflectedFieldAction::List(child) = &generic.action else {
            panic!("generic base readiness");
        };
        assert!(matches!(child.action, ReflectedFieldAction::Exact));
        assert!(generic.predicates().is_empty());
        let promoted = &plans(returned_read(&program, "promoted_root_read"))[0];
        let shared = &plans(returned_read(&program, "shared_root_read"))[0];
        assert!(matches!(
            promoted.action,
            ReflectedFieldAction::Predicates(_)
        ));
        assert_eq!(
            promoted
                .predicates()
                .iter()
                .map(|p| p.type_name.as_str())
                .collect::<Vec<_>>(),
            ["app.Larger"]
        );
        assert!(matches!(shared.action, ReflectedFieldAction::Predicates(_)));
        assert_eq!(
            shared
                .predicates()
                .iter()
                .map(|p| p.type_name.as_str())
                .collect::<Vec<_>>(),
            ["app.AlternateLarger"]
        );
        let independent = &plans(returned_read(&program, "independent_root_read"))[0];
        let ReflectedFieldAction::Refine { base, predicates } = &independent.action else {
            panic!("sibling base readiness");
        };
        assert_eq!(base.source_type, named.source_type);
        let ReflectedFieldAction::List(child) = &base.action else {
            panic!("base list");
        };
        assert!(matches!(child.action, ReflectedFieldAction::Exact));
        assert_eq!(predicates[0].type_name, "app.OtherValues");
        assert_eq!(independent.predicates().len(), 1);
        let text = &plans(returned_read(&program, "text_read"))[0];
        assert!(matches!(text.action, ReflectedFieldAction::Optional(_)));
        assert_eq!(text.predicates()[0].input_type, TypeInterner::STRING);
        let root = &plans(returned_read(&program, "root_read"))[0];
        let ReflectedFieldAction::Refine { base, predicates } = &root.action else {
            panic!("base-first root");
        };
        assert!(matches!(base.action, ReflectedFieldAction::List(_)));
        assert_eq!(predicates[0].type_name, "app.Nonempty");
        assert_eq!(
            root.predicates()
                .iter()
                .map(|p| p.type_name.as_str())
                .collect::<Vec<_>>(),
            ["app.Positive", "app.Nonempty"]
        );
    }

    fn recursive_key_mut(plan: &mut ReflectedFieldPlan) -> &mut ReflectedFieldPlan {
        let ReflectedFieldAction::List(optional) = &mut plan.action else {
            panic!("list");
        };
        let ReflectedFieldAction::Optional(result) = &mut optional.action else {
            panic!("optional");
        };
        let ReflectedFieldAction::Result { ok, .. } = &mut result.action else {
            panic!("result");
        };
        let ReflectedFieldAction::Map { key, .. } = &mut ok.action else {
            panic!("map");
        };
        key
    }

    #[test]
    fn native_reflected_recursive_hir_rejects_forged_child_schemas_and_canonical_predicates() {
        let (original, types) = lower_source_text(&recursive_source());
        let sibling = original
            .functions
            .iter()
            .find(|f| {
                f.identity.declaration.kind == DeclarationKind::RefinementPredicate
                    && f.identity.declaration.name == "Sibling"
            })
            .unwrap()
            .id;
        let mut foreign = TypeInterner::new();
        let mut invalid = TypeInterner::INT64;
        for index in 0..=types.len() {
            invalid = foreign.intern(Type::Refinement {
                name: format!("app.UncheckedForeignType{index}"),
                base: TypeInterner::INT64,
            });
        }
        assert!(invalid.index() as usize >= types.len());
        for mutation in 0..12 {
            let mut program = original.clone();
            let value = read_mut(&mut program);
            let ExpressionKind::Intrinsic {
                field_validation: Some(ReflectedFieldValidation::Validate(plans)),
                ..
            } = &mut value.kind
            else {
                panic!("plans");
            };
            if mutation == 0 {
                plans[0].requested_type = invalid;
            } else if mutation == 1 {
                let ReflectedFieldAction::List(child) = &plans[0].action else {
                    panic!("list");
                };
                plans[0].action = ReflectedFieldAction::Optional(child.clone());
            } else {
                let key = recursive_key_mut(&mut plans[0]);
                match mutation {
                    2 => key.source_type = TypeInterner::STRING,
                    3 => key.requested_type = invalid,
                    4 => key.action = ReflectedFieldAction::Exact,
                    _ => {
                        let ReflectedFieldAction::Predicates(predicates) = &mut key.action else {
                            panic!("leaf");
                        };
                        match mutation {
                            5 => predicates[0].function = sibling,
                            6 => predicates[0].function = FunctionId::new(u32::MAX),
                            7 => predicates[0].refined_type = invalid,
                            8 => predicates[0].base_type = TypeInterner::STRING,
                            9 => predicates[0].input_type = TypeInterner::STRING,
                            10 => predicates.clear(),
                            11 => predicates.push(predicates[0].clone()),
                            _ => unreachable!(),
                        }
                    }
                }
            }
            let structural = validate(&program).is_err();
            let typed = validate_backend_types(&program, &types).is_err();
            assert!(structural || typed, "accepted child mutation {mutation}");
            if matches!(mutation, 5 | 6) {
                assert!(structural, "recursive function identity is checked");
            } else {
                assert!(typed, "recursive checked schema is authoritative");
            }
        }
    }

    #[test]
    fn native_reflected_refine_base_plan_retains_declared_source_identity() {
        let (mut program, types) = lower_source_text(&recursive_source());
        let function = program
            .functions
            .iter_mut()
            .find(|f| f.identity.declaration.name == "independent_root_read")
            .unwrap();
        let StatementKind::Return(Some(value)) = &mut function.body.statements[0].kind else {
            panic!("root return");
        };
        let ExpressionKind::Intrinsic {
            field_validation: Some(ReflectedFieldValidation::Validate(plans)),
            ..
        } = &mut value.kind
        else {
            panic!("reflected root plan");
        };
        let ReflectedFieldAction::Refine { base, .. } = &mut plans[0].action else {
            panic!("independent root base");
        };
        assert_ne!(base.source_type, base.requested_type);
        // Forged erasure must not transform structural readiness into exact
        // whole-schema proof or discard the original declared producer owner.
        base.source_type = base.requested_type;
        assert!(validate_backend_types(&program, &types).is_err());
    }

    fn returned_read<'a>(program: &'a Program, name: &str) -> &'a Expression {
        let function = program
            .functions
            .iter()
            .find(|f| f.identity.declaration.name == name)
            .unwrap();
        let StatementKind::Return(Some(value)) = &function.body.statements[0].kind else {
            panic!("reflected return for {name}");
        };
        value
    }

    fn metadata_summary(
        args: &[Expression],
    ) -> Vec<(i128, String, Option<String>, String, String)> {
        args[2..]
            .iter()
            .map(|value| {
                let ExpressionKind::StructConstruct {
                    struct_type,
                    fields,
                    evaluation_order,
                    validates_refinements: false,
                    refinement_predicates,
                } = &value.kind
                else {
                    panic!("canonical hidden TypeField");
                };
                assert_eq!(*struct_type, args[1].ty);
                assert_eq!(value.ty, args[1].ty);
                assert_eq!(fields.len(), 10);
                assert_eq!(*evaluation_order, (0..10).collect::<Vec<_>>());
                assert!(refinement_predicates.is_empty());
                let ExpressionKind::Int(index) = fields[0].kind else {
                    panic!("checked field index");
                };
                let string = |field: &Expression| {
                    let ExpressionKind::String(value) = &field.kind else {
                        panic!("checked metadata string");
                    };
                    value.clone()
                };
                let member = match &fields[2].kind {
                    ExpressionKind::OptionalNone => None,
                    ExpressionKind::OptionalSome(value) => Some(string(value)),
                    _ => panic!("checked owner member"),
                };
                (
                    index,
                    string(&fields[1]),
                    member,
                    string(&fields[3]),
                    string(&fields[4]),
                )
            })
            .collect()
    }

    #[test]
    fn native_reflected_getter_pipelines_preserve_metadata_and_root_plans() {
        let source = format!(
            "{SOURCE}{}",
            r#"
function record_pipe(view value: Record, view field: TypeField) returns Higher:
    return value into view type.field_value[Record, Higher](view field)
function event_pipe(view value: Event, view field: TypeField) returns Higher:
    return value into view type.variant_field_value[Event, Higher](view field)
function session_pipe(view value: Session, view field: TypeField) returns Higher:
    return value into view type.machine_field_value[Session, Higher](view field)
"#
        );
        let (program, types) = lower_source_text(&source);
        validate(&program).unwrap();
        validate_backend_types(&program, &types).unwrap();
        for (direct, pipe, owner, members) in [
            ("record_read", "record_pipe", "app.Record", vec![None]),
            (
                "event_read",
                "event_pipe",
                "app.Event",
                vec![Some("first"), Some("second")],
            ),
            (
                "session_read",
                "session_pipe",
                "app.Session",
                vec![Some("first"), Some("second")],
            ),
        ] {
            let ExpressionKind::Intrinsic {
                intrinsic: direct_id,
                type_arguments: direct_types,
                reflection_arguments: direct_reflection,
                field_validation: direct_validation,
                args: direct_args,
                ..
            } = &returned_read(&program, direct).kind
            else {
                panic!("direct read");
            };
            let ExpressionKind::Intrinsic {
                intrinsic,
                type_arguments,
                reflection_arguments,
                field_validation,
                args,
                evaluation_order,
                ..
            } = &returned_read(&program, pipe).kind
            else {
                panic!("piped read");
            };
            assert_eq!(intrinsic, direct_id);
            assert_eq!(type_arguments, direct_types);
            assert_eq!(reflection_arguments, direct_reflection);
            assert_eq!(field_validation, direct_validation);
            assert_eq!(*evaluation_order, (0..args.len()).collect::<Vec<_>>());
            for (index, arg) in args[..2].iter().enumerate() {
                let ExpressionKind::View(value) = &arg.kind else {
                    panic!("view operand {index}");
                };
                assert!(
                    matches!(value.kind, ExpressionKind::Local(local) if local == LocalId::new(index as u32))
                );
            }
            let expected = members
                .into_iter()
                .flat_map(|member| {
                    let fields = if member == Some("second") {
                        vec![(0, "higher", "app.Higher"), (1, "raw", "int64")]
                    } else {
                        vec![
                            (0, "raw", "int64"),
                            (1, "positive", "app.Positive"),
                            (2, "higher", "app.Higher"),
                            (3, "sibling", "app.Sibling"),
                        ]
                    };
                    fields.into_iter().map(move |(index, name, ty)| {
                        (
                            index,
                            owner.to_string(),
                            member.map(str::to_string),
                            name.to_string(),
                            ty.to_string(),
                        )
                    })
                })
                .collect::<Vec<_>>();
            assert_eq!(metadata_summary(args), expected, "{pipe}");
            assert_eq!(metadata_summary(direct_args), expected, "{direct}");
        }
    }

    #[test]
    fn native_reflected_getter_pipelines_keep_bitfield_and_narrowed_machine_owners() {
        let source = format!(
            "{SOURCE}{}",
            r#"
bitfield Header:
    first: 4 bits
    second: 8 bits
function header_read(view value: Header, view field: TypeField) returns int64:
    return type.field_value[Header, int64](view value, view field)
function header_pipe(view value: Header, view field: TypeField) returns int64:
    return value into view type.field_value[Header, int64](view field)
function narrowed_read(view value: Session at first, view field: TypeField) returns Higher:
    return type.machine_field_value[Session at first, Higher](view value, view field)
function narrowed_pipe(view value: Session at first, view field: TypeField) returns Higher:
    return value into view type.machine_field_value[Session at first, Higher](view field)
"#
        );
        let (program, types) = lower_source_text(&source);
        validate(&program).unwrap();
        validate_backend_types(&program, &types).unwrap();
        for (direct, pipe, owner, count) in [
            ("header_read", "header_pipe", "app.Header", 2),
            ("narrowed_read", "narrowed_pipe", "app.Session", 6),
        ] {
            let ExpressionKind::Intrinsic {
                type_arguments: direct_types,
                field_validation: direct_validation,
                args: direct_args,
                ..
            } = &returned_read(&program, direct).kind
            else {
                panic!("direct read");
            };
            let ExpressionKind::Intrinsic {
                type_arguments,
                field_validation,
                args,
                evaluation_order,
                ..
            } = &returned_read(&program, pipe).kind
            else {
                panic!("piped read");
            };
            assert_eq!(type_arguments, direct_types);
            assert_eq!(field_validation, direct_validation);
            assert_eq!(*evaluation_order, (0..args.len()).collect::<Vec<_>>());
            let summary = metadata_summary(args);
            assert_eq!(summary.len(), count);
            assert!(summary.iter().all(|(_, actual, _, _, _)| actual == owner));
            assert_eq!(summary, metadata_summary(direct_args));
            if pipe == "header_pipe" {
                assert!(
                    summary
                        .iter()
                        .all(|(_, _, member, _, ty)| member.is_none() && ty == "int64")
                );
                let Some(ReflectedFieldValidation::Validate(plans)) = field_validation else {
                    panic!("primitive plan");
                };
                assert!(
                    plans
                        .iter()
                        .all(|plan| plan.action == ReflectedFieldAction::Exact)
                );
            } else {
                assert_eq!(summary[0].2.as_deref(), Some("first"));
                assert_eq!(summary[4].2.as_deref(), Some("second"));
            }
        }
    }

    #[test]
    fn native_reflected_getter_pipeline_stages_source_then_selector_and_feeds_next_call() {
        let source = format!(
            "{SOURCE}{}",
            r#"
function source_operand(view value: Record) returns Record:
    return clone value
function field_operand(view field: TypeField) returns TypeField:
    return clone field
function observe(value: Higher) returns int64:
    return coarsen value
function pipeline_operands(view value: Record, view field: TypeField) returns int64:
    return source_operand(view value) into view type.field_value[Record, Higher](view field_operand(view field)) into observe
"#
        );
        let (program, types) = lower_source_text(&source);
        validate(&program).unwrap();
        validate_backend_types(&program, &types).unwrap();
        let ExpressionKind::Call {
            function,
            args,
            evaluation_order,
            ..
        } = &returned_read(&program, "pipeline_operands").kind
        else {
            panic!("next source call");
        };
        assert_eq!(
            program
                .functions
                .iter()
                .find(|f| f.id == *function)
                .unwrap()
                .identity
                .declaration
                .name,
            "observe"
        );
        assert_eq!(evaluation_order, &[0]);
        let ExpressionKind::Intrinsic {
            type_arguments,
            args,
            evaluation_order,
            field_validation: Some(ReflectedFieldValidation::Validate(_)),
            ..
        } = &args[0].kind
        else {
            panic!("piped getter input");
        };
        assert_eq!(args.len(), 6);
        assert_eq!(*evaluation_order, (0..6).collect::<Vec<_>>());
        for (arg, expected) in args[..2].iter().zip(["source_operand", "field_operand"]) {
            let ExpressionKind::View(inner) = &arg.kind else {
                panic!("borrowed temporary");
            };
            let ExpressionKind::Call { function, .. } = &inner.kind else {
                panic!("operand expression");
            };
            assert_eq!(
                program
                    .functions
                    .iter()
                    .find(|f| f.id == *function)
                    .unwrap()
                    .identity
                    .declaration
                    .name,
                expected
            );
        }
        let Type::Refinement { name, .. } = types.resolve(type_arguments[1]) else {
            panic!("requested checked result");
        };
        assert_eq!(name, "app.Higher");
    }

    const VALUE_OBSERVERS: &str = r#"namespace app
struct Holder[T]:
    value: T
type Positive = int64 where value > 0
enum Event:
    empty = 17
    content(text: string, values: list[optional[int64]])
machine Session:
    states:
        empty
        content(text: string, values: list[optional[int64]])
    transitions:
        empty to content
function variant_direct(view source: Event) returns TypeVariant:
    return type.variant_value[Event](view source)
function variant_pipe(view source: Event) returns TypeVariant:
    return source into view type.variant_value[Event]()
function state_direct(view source: Session) returns TypeMachineState:
    return type.machine_state_value[Session](view source)
function state_pipe(view source: Session) returns TypeMachineState:
    return source into view type.machine_state_value[Session]()
function narrowed_direct(view source: Session at content) returns TypeMachineState:
    return type.machine_state_value[Session at content](view source)
function narrowed_pipe(view source: Session at content) returns TypeMachineState:
    return source into view type.machine_state_value[Session at content]()
function map_direct(index: int64) returns TypeInfo:
    return type.arg[map[string, list[optional[int64]]]](index)
function map_pipe(index: int64) returns TypeInfo:
    return index into type.arg[map[string, list[optional[int64]]]]()
function holder_direct(index: int64) returns TypeInfo:
    return type.arg[Holder[list[optional[int64]]]](index)
function holder_pipe(index: int64) returns TypeInfo:
    return index into type.arg[Holder[list[optional[int64]]]]()
function refinement_direct(index: int64) returns TypeInfo:
    return type.arg[Positive](index)
function refinement_pipe(index: int64) returns TypeInfo:
    return index into type.arg[Positive]()
function primitive_direct(index: int64) returns TypeInfo:
    return type.arg[int64](index)
function primitive_pipe(index: int64) returns TypeInfo:
    return index into type.arg[int64]()
"#;

    #[derive(Debug, PartialEq, Eq)]
    enum MetadataKind {
        Int(i128),
        String(String),
        Bool(bool),
        Struct(TypeId, Vec<MetadataValue>),
        Enum(TypeId, VariantId, Vec<MetadataValue>),
        List(Vec<MetadataValue>),
        Some(Box<MetadataValue>),
        None,
    }

    #[derive(Debug, PartialEq, Eq)]
    struct MetadataValue {
        ty: TypeId,
        kind: MetadataKind,
    }

    // Compare complete recursive metadata without source spans. In particular,
    // this catches an input type being substituted for a pipeline result type.
    fn metadata_value(value: &Expression) -> MetadataValue {
        let kind = match &value.kind {
            ExpressionKind::Int(value) => MetadataKind::Int(*value),
            ExpressionKind::String(value) => MetadataKind::String(value.clone()),
            ExpressionKind::Bool(value) => MetadataKind::Bool(*value),
            ExpressionKind::StructConstruct {
                struct_type,
                fields,
                evaluation_order,
                validates_refinements,
                refinement_predicates,
            } => {
                assert!(!validates_refinements);
                assert!(refinement_predicates.is_empty());
                assert_eq!(*evaluation_order, (0..fields.len()).collect::<Vec<_>>());
                MetadataKind::Struct(*struct_type, fields.iter().map(metadata_value).collect())
            }
            ExpressionKind::EnumConstruct {
                enum_type,
                variant,
                payloads,
                evaluation_order,
            } => {
                assert_eq!(*evaluation_order, (0..payloads.len()).collect::<Vec<_>>());
                MetadataKind::Enum(
                    *enum_type,
                    *variant,
                    payloads.iter().map(metadata_value).collect(),
                )
            }
            ExpressionKind::ListConstruct { elements } => {
                MetadataKind::List(elements.iter().map(metadata_value).collect())
            }
            ExpressionKind::OptionalSome(value) => {
                MetadataKind::Some(Box::new(metadata_value(value)))
            }
            ExpressionKind::OptionalNone => MetadataKind::None,
            other => panic!("non-metadata expression: {other:?}"),
        };
        MetadataValue { ty: value.ty, kind }
    }

    fn metadata_fields(value: &Expression) -> &[Expression] {
        let ExpressionKind::StructConstruct { fields, .. } = &value.kind else {
            panic!("checked metadata struct");
        };
        fields
    }

    fn metadata_text(value: &Expression) -> &str {
        let ExpressionKind::String(value) = &value.kind else {
            panic!("checked metadata text");
        };
        value
    }

    fn assert_value_metadata_parity(program: &Program, direct: &str, pipe: &str, count: usize) {
        let direct = returned_read(program, direct);
        let pipe = returned_read(program, pipe);
        let ExpressionKind::Intrinsic {
            intrinsic: direct_id,
            type_arguments: direct_types,
            reflection_arguments: direct_reflection,
            args: direct_args,
            field_validation: None,
            ..
        } = &direct.kind
        else {
            panic!("direct value observer");
        };
        let ExpressionKind::Intrinsic {
            intrinsic,
            type_arguments,
            reflection_arguments,
            args,
            evaluation_order,
            field_validation: None,
            ..
        } = &pipe.kind
        else {
            panic!("pipeline value observer");
        };
        assert_eq!(pipe.ty, direct.ty);
        assert_eq!(intrinsic, direct_id);
        assert_eq!(type_arguments, direct_types);
        assert_eq!(reflection_arguments, direct_reflection);
        assert_eq!(args[0].ty, direct_args[0].ty);
        assert_eq!(args.len(), count + 1);
        assert_eq!(*evaluation_order, (0..args.len()).collect::<Vec<_>>());
        assert!(args[1..].iter().all(|arg| arg.ty == pipe.ty));
        assert_eq!(
            args[1..].iter().map(metadata_value).collect::<Vec<_>>(),
            direct_args[1..]
                .iter()
                .map(metadata_value)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn native_reflected_value_pipelines_match_complete_direct_metadata() {
        let (program, types) = lower_source_text(VALUE_OBSERVERS);
        validate(&program).unwrap();
        validate_backend_types(&program, &types).unwrap();
        for (direct, pipe, count) in [
            ("variant_direct", "variant_pipe", 2),
            ("state_direct", "state_pipe", 2),
            ("narrowed_direct", "narrowed_pipe", 2),
            ("map_direct", "map_pipe", 2),
            ("holder_direct", "holder_pipe", 1),
            ("refinement_direct", "refinement_pipe", 1),
            ("primitive_direct", "primitive_pipe", 0),
        ] {
            assert_value_metadata_parity(&program, direct, pipe, count);
        }
        for (name, expected) in [
            ("map_pipe", vec!["string", "list[optional[int64]]"]),
            ("holder_pipe", vec!["list[optional[int64]]"]),
            ("refinement_pipe", vec!["int64"]),
        ] {
            let ExpressionKind::Intrinsic { args, .. } = &returned_read(&program, name).kind else {
                panic!("type.arg");
            };
            assert_eq!(
                args[1..]
                    .iter()
                    .map(|arg| metadata_text(&metadata_fields(arg)[0]))
                    .collect::<Vec<_>>(),
                expected
            );
        }
    }

    #[test]
    fn native_reflected_value_pipeline_metadata_keeps_owner_members_and_all_narrowed_states() {
        let (program, types) = lower_source_text(VALUE_OBSERVERS);
        for (name, owner, fields_index) in [
            ("variant_pipe", "app.Event", 5),
            ("state_pipe", "app.Session", 4),
            ("narrowed_pipe", "app.Session", 4),
        ] {
            let ExpressionKind::Intrinsic {
                type_arguments,
                args,
                ..
            } = &returned_read(&program, name).kind
            else {
                panic!("value observer");
            };
            if name == "narrowed_pipe" {
                assert!(matches!(
                    types.resolve(type_arguments[0]),
                    Type::MachineState { .. }
                ));
                assert_eq!(args[0].ty, type_arguments[0]);
            }
            for (index, (metadata, member)) in
                args[1..].iter().zip(["empty", "content"]).enumerate()
            {
                let fields = metadata_fields(metadata);
                assert!(
                    matches!(fields[0].kind, ExpressionKind::Int(actual) if actual == index as i128)
                );
                assert_eq!(metadata_text(&fields[1]), owner);
                assert_eq!(metadata_text(&fields[2]), member);
                if name == "variant_pipe" {
                    assert!(
                        matches!(fields[3].kind, ExpressionKind::Int(actual) if actual == 17 + index as i128)
                    );
                }
                let ExpressionKind::ListConstruct { elements } = &fields[fields_index].kind else {
                    panic!("payload TypeField list");
                };
                assert_eq!(elements.len(), index * 2);
                for (field_index, (field, field_name)) in
                    elements.iter().zip(["text", "values"]).enumerate()
                {
                    let slots = metadata_fields(field);
                    assert!(
                        matches!(slots[0].kind, ExpressionKind::Int(actual) if actual == field_index as i128)
                    );
                    assert_eq!(metadata_text(&slots[1]), owner);
                    let ExpressionKind::OptionalSome(actual_member) = &slots[2].kind else {
                        panic!("declared owner member");
                    };
                    assert_eq!(metadata_text(actual_member), member);
                    assert_eq!(metadata_text(&slots[3]), field_name);
                }
            }
        }
    }

    #[test]
    fn native_reflected_value_pipeline_stages_one_operand_and_feeds_the_next_call() {
        let source = format!(
            "{VALUE_OBSERVERS}{}",
            r#"
function source_operand(view source: Event) returns Event:
    return clone source
function state_operand(view source: Session at content) returns Session at content:
    return clone source
function index_operand(index: int64) returns int64:
    return index
function consume_variant(value: TypeVariant) returns string:
    return value.name
function consume_state(value: TypeMachineState) returns string:
    return value.name
function consume_info(value: TypeInfo) returns string:
    return value.type_name
function variant_order(view source: Event) returns string:
    return source_operand(view source) into view type.variant_value[Event]() into consume_variant
function state_order(view source: Session at content) returns string:
    return state_operand(view source) into view type.machine_state_value[Session at content]() into consume_state
function arg_order(index: int64) returns string:
    return index_operand(index) into type.arg[map[string, list[optional[int64]]]]() into consume_info
"#
        );
        let (program, types) = lower_source_text(&source);
        validate(&program).unwrap();
        validate_backend_types(&program, &types).unwrap();
        for (name, producer, consumer, borrowed) in [
            ("variant_order", "source_operand", "consume_variant", true),
            ("state_order", "state_operand", "consume_state", true),
            ("arg_order", "index_operand", "consume_info", false),
        ] {
            let ExpressionKind::Call {
                function,
                args,
                evaluation_order,
                ..
            } = &returned_read(&program, name).kind
            else {
                panic!("following call");
            };
            assert_eq!(
                program.functions[function.index() as usize]
                    .identity
                    .declaration
                    .name,
                consumer
            );
            assert_eq!(evaluation_order, &[0]);
            let ExpressionKind::Intrinsic {
                args,
                evaluation_order,
                ..
            } = &args[0].kind
            else {
                panic!("observed pipeline input");
            };
            assert_eq!(*evaluation_order, (0..args.len()).collect::<Vec<_>>());
            let input = if borrowed {
                let ExpressionKind::View(value) = &args[0].kind else {
                    panic!("borrowed source temporary");
                };
                value.as_ref()
            } else {
                &args[0]
            };
            let ExpressionKind::Call { function, .. } = &input.kind else {
                panic!("one source operand call");
            };
            assert_eq!(
                program.functions[function.index() as usize]
                    .identity
                    .declaration
                    .name,
                producer
            );
            for metadata in &args[1..] {
                metadata_value(metadata);
            }
        }
    }
}
