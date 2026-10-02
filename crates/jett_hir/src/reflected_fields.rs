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
    pub action: ReflectedFieldAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReflectedFieldAction {
    Exact,
    Predicates(Vec<RefinementPredicate>),
    Unsupported(ReflectedFieldUnsupported),
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
        Type::Struct(id) => types
            .resolve_struct(*id)
            .fields
            .iter()
            .map(|(_, ty)| *ty)
            .collect(),
        Type::Enum(id) => types
            .resolve_enum(*id)
            .variants
            .iter()
            .flat_map(|variant| variant.fields.iter().map(|(_, ty)| *ty))
            .collect(),
        Type::Machine(id) | Type::MachineState { machine: id, .. } => types
            .resolve_machine(*id)
            .states
            .iter()
            .flat_map(|state| state.fields.iter().map(|(_, ty)| *ty))
            .collect(),
        Type::Bitfield(id) => types
            .resolve_bitfield(*id)
            .fields
            .iter()
            .map(|field| field.ty)
            .collect(),
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
    if actual.index() as usize >= types.len() || requested.index() as usize >= types.len() {
        return None;
    }
    if actual == requested || !contains_refinement(types, requested, &mut HashSet::new()) {
        return Some(ReflectedFieldRequirement::Exact);
    }
    let (actual_base, actual_chain) = refinement_chain(types, actual)?;
    // Reading an already established refinement through its exact base does
    // not create any child invariant, even if that base is nominal or secret.
    if actual_base == requested || actual_chain.contains(&requested) {
        return Some(ReflectedFieldRequirement::Exact);
    }
    let (base, mut chain) = refinement_chain(types, requested)?;
    if !chain.is_empty() {
        // A new outer predicate may rely on existing child invariants. Never
        // claim those from equal erased carrier bits alone.
        if base != actual_base && contains_refinement(types, base, &mut HashSet::new()) {
            return Some(ReflectedFieldRequirement::Unsupported(unsupported_shape(
                types, base,
            )));
        }
        if let Some(index) = chain.iter().position(|ty| *ty == actual) {
            chain.drain(..=index);
        }
        return Some(ReflectedFieldRequirement::Predicates(chain));
    }
    Some(ReflectedFieldRequirement::Unsupported(unsupported_shape(
        types, requested,
    )))
}

fn unsupported_shape(types: &TypeInterner, requested: TypeId) -> ReflectedFieldUnsupported {
    match types.resolve(requested) {
        Type::Secret(_) => ReflectedFieldUnsupported::Secret,
        Type::List(_) | Type::Set(_) | Type::Map(..) | Type::Optional(_) | Type::Result(..) => {
            ReflectedFieldUnsupported::Nested
        }
        _ => ReflectedFieldUnsupported::CallableOrNominal,
    }
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
        if plan.source_type != *actual {
            return Err("reflected validation source type disagrees with declared field");
        }
        let requirement = reflected_field_requirement(types, *actual, requested)
            .ok_or("invalid reflected validation type")?;
        match (requirement, &plan.action) {
            (ReflectedFieldRequirement::Exact, ReflectedFieldAction::Exact) => {}
            (
                ReflectedFieldRequirement::Unsupported(expected),
                ReflectedFieldAction::Unsupported(actual),
            ) if expected == *actual => {}
            (
                ReflectedFieldRequirement::Predicates(expected),
                ReflectedFieldAction::Predicates(predicates),
            ) => {
                if predicates
                    .iter()
                    .map(|predicate| predicate.refined_type)
                    .ne(expected.iter().copied())
                {
                    return Err(
                        "reflected validation predicate suffix disagrees with checked source",
                    );
                }
                let (base_type, _) = refinement_chain(types, requested)
                    .ok_or("invalid reflected refinement chain")?;
                let input_type = match types.resolve(base_type) {
                    Type::Secret(inner) => *inner,
                    _ => base_type,
                };
                for predicate in predicates {
                    let Type::Refinement { name, .. } = types.resolve(predicate.refined_type)
                    else {
                        return Err("reflected predicate is not a refinement");
                    };
                    if predicate.type_name != *name
                        || predicate.base_type != base_type
                        || predicate.input_type != input_type
                    {
                        return Err("reflected predicate metadata disagrees with declaration");
                    }
                }
            }
            _ => return Err("reflected validation action disagrees with checked source"),
        }
    }
    Ok(())
}

impl BodyLowerer<'_, '_> {
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
            let Some(requirement) =
                reflected_field_requirement(&self.parent.check.interner, source_type, *requested)
            else {
                self.parent
                    .error(span, "reflected read has invalid proof types");
                return None;
            };
            let action = match requirement {
                ReflectedFieldRequirement::Exact => ReflectedFieldAction::Exact,
                ReflectedFieldRequirement::Unsupported(reason) => {
                    ReflectedFieldAction::Unsupported(reason)
                }
                ReflectedFieldRequirement::Predicates(expected) => {
                    let mut predicates = self.checked_refinement_predicates(*requested, span)?;
                    predicates.retain(|predicate| expected.contains(&predicate.refined_type));
                    ReflectedFieldAction::Predicates(predicates)
                }
            };
            plans.push(ReflectedFieldPlan {
                source_type,
                action,
            });
        }
        Some(Some(ReflectedFieldValidation::Validate(plans)))
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
                    ReflectedFieldAction::Unsupported(_) => panic!("root scalar is supported"),
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
            Some(ReflectedFieldRequirement::Exact)
        );
        let plain_list = types.intern(Type::List(TypeInterner::INT64));
        assert_eq!(
            reflected_field_requirement(&types, plain_list, nonempty),
            Some(ReflectedFieldRequirement::Unsupported(
                ReflectedFieldUnsupported::Nested
            ))
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
}
