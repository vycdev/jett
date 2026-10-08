//! Constructor-only correspondence for direct original nominal struct field loops.
//! Metadata is runtime data; ordinal-specific checked bodies remain Source authority.
use crate::{
    Block, DeclarationKind, Expression, ExpressionKind as E, FieldId, Function, FunctionId,
    LocalId, Program, ReflectedTypeArm, Statement, StatementKind as S,
};
use jett_common::Span;
use jett_intrinsics::IntrinsicId;
use jett_parser::ast::{self, Expr, Item, Stmt, TypeExpr};
use jett_resolve::{DefId, DefKind, ResolveResult};
use jett_typecheck::{
    CheckResult, CheckedBodyFacts, CheckedComptimeTypeSelection, CheckedGenericSpecialization,
    CheckedResourceProgram,
};
use jett_types::{ReflectionFieldInfo, ReflectionTypeInfo, Type, TypeId, TypeInterner};
use std::sync::Arc;

#[derive(Debug)]
struct SelectedBody {
    checked_index: usize,
    address: usize,
    bound_type: TypeId,
    reflection: ReflectionTypeInfo,
    // The constructor retains the full original checked expansion. No public
    // constructor, setter, or caller-supplied fact map can substitute this body.
    _facts: CheckedBodyFacts,
}

#[derive(Debug)]
struct Dispatch {
    checked: Arc<CheckedResourceProgram>,
    function: FunctionId,
    declaration: DefId,
    function_span: Span,
    loop_address: usize,
    binding_address: usize,
    loop_span: Span,
    binding_span: Span,
    binder: DefId,
    owner_definition: DefId,
    owner: TypeId,
    owner_reflection: ReflectionTypeInfo,
    fields: Vec<(TypeId, ReflectionFieldInfo)>,
    selected: Vec<SelectedBody>,
    key: LocalId,
    field_index: FieldId,
    type_info_field: FieldId,
    loop_path: Vec<usize>,
    dispatch_path: Vec<usize>,
    containers: Vec<Vec<usize>>,
    scopes: Vec<Vec<usize>>,
    type_info: Expression,
    arms: Vec<ReflectedTypeArm>,
}

/// Immutable original Source authority; only successful checked HIR lowering
/// can create it. Clones retain the same checked session and exact body record.
#[derive(Debug, Clone)]
pub struct ReflectedFieldDispatchProof {
    data: Arc<Dispatch>,
}
impl PartialEq for ReflectedFieldDispatchProof {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data)
    }
}
impl Eq for ReflectedFieldDispatchProof {}

impl ReflectedFieldDispatchProof {
    pub fn function(&self) -> FunctionId {
        self.data.function
    }
    pub fn key(&self) -> LocalId {
        self.data.key
    }
    pub fn owner(&self) -> TypeId {
        self.data.owner
    }
    pub fn owner_reflection(&self) -> &ReflectionTypeInfo {
        &self.data.owner_reflection
    }
    pub fn fields(&self) -> &[(TypeId, ReflectionFieldInfo)] {
        &self.data.fields
    }
    pub fn type_info(&self) -> &Expression {
        &self.data.type_info
    }
    pub fn arms(&self) -> &[ReflectedTypeArm] {
        &self.data.arms
    }
    pub fn field_index(&self) -> FieldId {
        self.data.field_index
    }
    pub fn type_info_field(&self) -> FieldId {
        self.data.type_info_field
    }
    pub fn loop_path(&self) -> &[usize] {
        &self.data.loop_path
    }
    pub fn dispatch_path(&self) -> &[usize] {
        &self.data.dispatch_path
    }
    pub fn arm_container_path(&self, index: usize) -> Option<&[usize]> {
        self.data.containers.get(index).map(Vec::as_slice)
    }
    /// The explicit Scope body path in the original lexical inventory.
    pub fn arm_path(&self, index: usize) -> Option<&[usize]> {
        self.data.scopes.get(index).map(Vec::as_slice)
    }
    pub fn iteration_index(&self, index: usize) -> Option<usize> {
        self.data.arms.get(index).map(|arm| arm.iteration_index)
    }
    pub(crate) fn belongs_to(&self, checked: &Arc<CheckedResourceProgram>) -> bool {
        Arc::ptr_eq(&self.data.checked, checked)
    }
    pub(crate) fn matches(
        &self,
        functions: &[Function],
        type_info: &Expression,
        arms: &[ReflectedTypeArm],
    ) -> bool {
        if !self.validate_original() {
            return false;
        }
        let Some(original) = functions
            .iter()
            .find(|function| function.id == self.function())
        else {
            return false;
        };
        let mut candidate = original.clone();
        let Some(statement) = root_dispatch_mut(&mut candidate.body, &self.data.dispatch_path)
        else {
            return false;
        };
        statement.kind = S::ReflectedTypeDispatch {
            type_info: type_info.clone(),
            arms: arms.to_vec(),
        };
        // The existing archive comparison preserves float bits, including NaN.
        crate::resource_materialization::functions_equal(
            std::slice::from_ref(original),
            std::slice::from_ref(&candidate),
        )
    }
    pub(crate) fn validate_original(&self) -> bool {
        let data = &self.data;
        let Some(source) = original_function(&data.checked, data.declaration, data.function_span)
        else {
            return false;
        };
        let loops = source
            .body
            .stmts
            .iter()
            .filter_map(|statement| match statement {
                Stmt::For(source) if source.span == data.loop_span => Some(source),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [source_loop] = loops.as_slice() else {
            return false;
        };
        if std::ptr::from_ref(*source_loop).addr() != data.loop_address {
            return false;
        }
        let bindings = source_loop
            .body
            .stmts
            .iter()
            .filter_map(|statement| match statement {
                Stmt::ComptimeTypeBind(source) if source.span == data.binding_span => Some(source),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [binding] = bindings.as_slice() else {
            return false;
        };
        if std::ptr::from_ref(*binding).addr() != data.binding_address
            || direct_owner(&data.checked, source_loop)
                != Some((
                    data.owner_definition,
                    data.owner,
                    data.owner_reflection.clone(),
                ))
            || binder(&data.checked, source_loop) != Some(data.binder)
            || checked_fields(&data.checked, data.owner).as_ref() != Some(&data.fields)
            || !binding_selects_binder(&data.checked, binding, data.binder)
        {
            return false;
        }
        let Some(rows) = data
            .checked
            .checked()
            .comptime_type_bindings
            .get(&binding.span)
        else {
            return false;
        };
        if rows.len() != data.selected.len() {
            return false;
        }
        data.selected.iter().enumerate().all(|(ordinal, selected)| {
            let Some(row) = rows.get(selected.checked_index) else {
                return false;
            };
            std::ptr::from_ref(&row.body).addr() == selected.address
                && row.selection == CheckedComptimeTypeSelection::ReflectedIteration(ordinal)
                && row.bound_type == selected.bound_type
                && row.reflection == selected.reflection
        })
    }
}

pub(crate) fn capture(
    program: &Program,
    checked: &Arc<CheckedResourceProgram>,
) -> Vec<ReflectedFieldDispatchProof> {
    let mut proofs = Vec::new();
    for function in &program.functions {
        if function.identity.declaration.kind != DeclarationKind::Function
            || !function.identity.type_arguments.is_empty()
            || !function.identity.scoped_type_bindings.is_empty()
            || function.identity.specialization != CheckedGenericSpecialization::default()
            || function.capture_count != 0
        {
            continue;
        }
        let Some(declaration) = function.source_definition else {
            continue;
        };
        let Some(source) = original_function(checked, declaration, function.span) else {
            continue;
        };
        let Some(definition) = checked
            .resolved()
            .scope_table
            .definitions
            .get(declaration.index() as usize)
        else {
            continue;
        };
        let namespace = definition.namespace.as_deref().unwrap_or("");
        let canonical_name = if namespace.is_empty() {
            source.name.name.clone()
        } else {
            format!("{namespace}.{}", source.name.name)
        };
        if function.identity.declaration.name != source.name.name
            || definition.name != canonical_name
            || function.identity.declaration.namespace
                != definition.namespace.as_deref().unwrap_or("")
            || checked.source_origins().get(&source.span.file)
                != Some(&function.identity.declaration.origin)
        {
            continue;
        }
        // Root body facts are the only admitted producer context. Nested or
        // generic reflected expansions need their own checked scope cursor.
        for statement in &source.body.stmts {
            let Stmt::For(source_loop) = statement else {
                continue;
            };
            let Some((owner_definition, owner, owner_reflection)) =
                direct_owner(checked, source_loop)
            else {
                continue;
            };
            let Some(fields) = checked_fields(checked, owner) else {
                continue;
            };
            let Some(binder) = binder(checked, source_loop) else {
                continue;
            };
            let loops = function
                .body
                .statements
                .iter()
                .enumerate()
                .filter(|(_, statement)| {
                    statement.span == source_loop.span && matches!(statement.kind, S::For { .. })
                })
                .collect::<Vec<_>>();
            let [(loop_index, loop_statement)] = loops.as_slice() else {
                continue;
            };
            let S::For {
                key,
                value: None,
                by_view: false,
                iterable,
                body,
            } = &loop_statement.kind
            else {
                continue;
            };
            let Some(local) = function.locals.get(key.index() as usize) else {
                continue;
            };
            if local.id != *key
                || local.span != source_loop.variable.span
                || local.mutable
                || local.name != source_loop.variable.name
                || local.view_source.is_some()
                || checked.checked().definition_types.get(&binder) != Some(&local.ty)
                || !metadata_list_matches(
                    iterable,
                    local.ty,
                    owner_reflection.type_name.as_str(),
                    &fields,
                    checked,
                )
            {
                continue;
            }
            for statement in &source_loop.body.stmts {
                let Stmt::ComptimeTypeBind(binding) = statement else {
                    continue;
                };
                if !binding_selects_binder(checked, binding, binder) {
                    continue;
                }
                if let Some(proof) = capture_dispatch(
                    checked,
                    function,
                    declaration,
                    source_loop,
                    binding,
                    *loop_index,
                    body,
                    *key,
                    binder,
                    owner_definition,
                    owner,
                    &owner_reflection,
                    &fields,
                ) {
                    proofs.push(proof);
                }
            }
        }
    }
    proofs
}

#[allow(clippy::too_many_arguments)]
fn capture_dispatch(
    checked: &Arc<CheckedResourceProgram>,
    function: &Function,
    declaration: DefId,
    source_loop: &ast::ForStmt,
    binding: &ast::ComptimeTypeBindStmt,
    loop_index: usize,
    body: &Block,
    key: LocalId,
    binder: DefId,
    owner_definition: DefId,
    owner: TypeId,
    owner_reflection: &ReflectionTypeInfo,
    fields: &[(TypeId, ReflectionFieldInfo)],
) -> Option<ReflectedFieldDispatchProof> {
    let dispatches = body
        .statements
        .iter()
        .enumerate()
        .filter(|(_, statement)| {
            statement.span == binding.span
                && matches!(statement.kind, S::ReflectedTypeDispatch { .. })
        })
        .collect::<Vec<_>>();
    let [(dispatch_index, statement)] = dispatches.as_slice() else {
        return None;
    };
    let S::ReflectedTypeDispatch { type_info, arms } = &statement.kind else {
        return None;
    };
    let local = function.locals.get(key.index() as usize)?;
    let E::Field {
        base,
        owner_type,
        field,
    } = &type_info.kind
    else {
        return None;
    };
    let Type::Struct(field_struct) = checked.checked().interner.resolve(local.ty) else {
        return None;
    };
    let field_header = checked.checked().interner.resolve_struct(*field_struct);
    if *field != FieldId::new(9)
        || *owner_type != local.ty
        || field_header.fields.get(9).map(|(_, ty)| *ty) != Some(type_info.ty)
        || !matches!(base.kind, E::Local(found) if found == key)
        || base.ty != local.ty
        || type_info.span != binding.value.span()
    {
        return None;
    }
    let rows = checked
        .checked()
        .comptime_type_bindings
        .get(&binding.span)?;
    if arms.len() != fields.len() || rows.len() != fields.len() || fields.is_empty() {
        return None;
    }
    let mut selected = Vec::with_capacity(fields.len());
    let mut containers = Vec::with_capacity(fields.len());
    let mut scopes = Vec::with_capacity(fields.len());
    let loop_path = vec![0, loop_index];
    let dispatch_path = vec![0, loop_index, 2, 0, 0, *dispatch_index];
    for (ordinal, ((bound_type, metadata), arm)) in fields.iter().zip(arms).enumerate() {
        let candidates = rows
            .iter()
            .enumerate()
            .filter(|(_, row)| {
                row.selection == CheckedComptimeTypeSelection::ReflectedIteration(ordinal)
            })
            .collect::<Vec<_>>();
        let [(checked_index, row)] = candidates.as_slice() else {
            return None;
        };
        let [wrapper] = arm.body.statements.as_slice() else {
            return None;
        };
        let S::Scope(original_body) = &wrapper.kind else {
            return None;
        };
        if row.bound_type != *bound_type
            || row.reflection != metadata.type_info
            || arm.iteration_index != ordinal
            || arm.bound_type != *bound_type
            || arm.reflection_identity != metadata.type_info.reflection_identity()
            || wrapper.span != binding.span
            || original_body.span != binding.body.span
            || arm.body.span != binding.body.span
        {
            return None;
        }
        selected.push(SelectedBody {
            checked_index: *checked_index,
            address: std::ptr::from_ref(&row.body).addr(),
            bound_type: row.bound_type,
            reflection: row.reflection.clone(),
            _facts: row.body.clone(),
        });
        let mut container = dispatch_path.clone();
        container.extend([2, ordinal]);
        let mut scope = container.clone();
        scope.extend([0, 0, 2, 0]);
        containers.push(container);
        scopes.push(scope);
    }
    Some(ReflectedFieldDispatchProof {
        data: Arc::new(Dispatch {
            checked: checked.clone(),
            function: function.id,
            declaration,
            function_span: function.span,
            loop_address: std::ptr::from_ref(source_loop).addr(),
            binding_address: std::ptr::from_ref(binding).addr(),
            loop_span: source_loop.span,
            binding_span: binding.span,
            binder,
            owner_definition,
            owner,
            owner_reflection: owner_reflection.clone(),
            fields: fields.to_vec(),
            selected,
            key,
            field_index: FieldId::new(0),
            type_info_field: FieldId::new(9),
            loop_path,
            dispatch_path,
            containers,
            scopes,
            type_info: type_info.clone(),
            arms: arms.clone(),
        }),
    })
}

fn original_function(
    checked: &CheckedResourceProgram,
    declaration: DefId,
    span: Span,
) -> Option<&ast::FunctionDef> {
    original_function_parts(checked.module(), checked.resolved(), declaration, span)
}

fn original_function_parts<'a>(
    module: &'a ast::Module,
    resolve: &ResolveResult,
    declaration: DefId,
    span: Span,
) -> Option<&'a ast::FunctionDef> {
    let definition = resolve
        .scope_table
        .definitions
        .get(declaration.index() as usize)?;
    if definition.id != declaration || definition.kind != DefKind::Function {
        return None;
    }
    let candidates = module
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Function(source)
                if source.name.span == definition.span
                    && source.span == span
                    && source.type_params.is_empty() =>
            {
                Some(source)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [source] = candidates.as_slice() else {
        return None;
    };
    Some(*source)
}

pub(crate) fn original_root_function(
    module: &ast::Module,
    resolve: &ResolveResult,
    source: &ast::FunctionDef,
    declaration: DefId,
) -> bool {
    original_function_parts(module, resolve, declaration, source.span)
        .is_some_and(|original| std::ptr::eq(original, source))
}

fn direct_owner(
    checked: &CheckedResourceProgram,
    source: &ast::ForStmt,
) -> Option<(DefId, TypeId, ReflectionTypeInfo)> {
    direct_owner_parts(checked.resolved(), checked.checked(), source)
}

fn direct_owner_parts(
    resolve: &ResolveResult,
    check: &CheckResult,
    source: &ast::ForStmt,
) -> Option<(DefId, TypeId, ReflectionTypeInfo)> {
    let Expr::GenericCall(_, operands, arguments, occurrence) = &source.iterable else {
        return None;
    };
    if source.view
        || source.value_variable.is_some()
        || !arguments.is_empty()
        || check.intrinsic_ids.get(occurrence) != Some(&IntrinsicId::TypeFields)
    {
        return None;
    }
    let [TypeExpr::Named(operand)] = operands.as_slice() else {
        return None;
    };
    let definition = *resolve.resolutions.get(&operand.span)?;
    let header = resolve
        .scope_table
        .definitions
        .get(definition.index() as usize)?;
    if header.id != definition || header.kind != DefKind::Struct {
        return None;
    }
    let [owner] = check.intrinsic_type_arguments.get(occurrence)?.as_slice() else {
        return None;
    };
    let [reflection] = check
        .intrinsic_reflection_arguments
        .get(occurrence)?
        .as_slice()
    else {
        return None;
    };
    let types = &check.interner;
    if check.definition_types.get(&definition) != Some(owner)
        || !matches!(types.resolve(*owner), Type::Struct(_))
        || !types.nominal_type_arguments(*owner).is_empty()
        || reflection.kind != "struct"
        || !reflection.args.is_empty()
        || check.reflection_metadata.get_type_info_for_id(*owner) != Some(reflection)
    {
        return None;
    }
    Some((definition, *owner, reflection.clone()))
}

fn binder(checked: &CheckedResourceProgram, source: &ast::ForStmt) -> Option<DefId> {
    binder_parts(checked.resolved(), source)
}

fn binder_parts(resolve: &ResolveResult, source: &ast::ForStmt) -> Option<DefId> {
    let candidates = resolve
        .scope_table
        .definitions
        .iter()
        .filter(|definition| {
            definition.kind == DefKind::Variable
                && definition.span == source.variable.span
                && definition.name == source.variable.name
        })
        .collect::<Vec<_>>();
    let [definition] = candidates.as_slice() else {
        return None;
    };
    Some(definition.id)
}

fn binding_selects_binder(
    checked: &CheckedResourceProgram,
    binding: &ast::ComptimeTypeBindStmt,
    binder: DefId,
) -> bool {
    binding_selects_binder_parts(checked.resolved(), binding, binder)
}

fn binding_selects_binder_parts(
    resolve: &ResolveResult,
    binding: &ast::ComptimeTypeBindStmt,
    binder: DefId,
) -> bool {
    let Expr::FieldAccess(base, member, _) = &binding.value else {
        return false;
    };
    let Expr::Ident(variable) = base.as_ref() else {
        return false;
    };
    member.name == "type_info" && resolve.resolutions.get(&variable.span) == Some(&binder)
}

fn checked_fields(
    checked: &CheckedResourceProgram,
    owner: TypeId,
) -> Option<Vec<(TypeId, ReflectionFieldInfo)>> {
    checked_fields_parts(checked.checked(), owner)
}

fn checked_fields_parts(
    check: &CheckResult,
    owner: TypeId,
) -> Option<Vec<(TypeId, ReflectionFieldInfo)>> {
    let types = &check.interner;
    let Type::Struct(id) = types.resolve(owner) else {
        return None;
    };
    let fields = &types.resolve_struct(*id).fields;
    let metadata = check.reflection_metadata.get_type_fields_for_id(owner)?;
    if metadata.len() != fields.len()
        || metadata
            .iter()
            .zip(fields)
            .enumerate()
            .any(|(index, (metadata, (name, ty)))| {
                metadata.index != index
                    || metadata.name != *name
                    || ty.index() as usize >= types.len()
            })
    {
        return None;
    }
    Some(
        fields
            .iter()
            .map(|(_, ty)| *ty)
            .zip(metadata.iter().cloned())
            .collect(),
    )
}

/// Called while lowering the exact original root For AST occurrence. This is
/// only a lowering choice; Source authority is minted later in archive capture.
pub(crate) fn original_loop_bindings(
    resolve: &ResolveResult,
    check: &CheckResult,
    source: &ast::ForStmt,
) -> Vec<Span> {
    if source.view || source.value_variable.is_some() {
        return Vec::new();
    }
    let Some((_, owner, _)) = direct_owner_parts(resolve, check, source) else {
        return Vec::new();
    };
    let Some(fields) = checked_fields_parts(check, owner) else {
        return Vec::new();
    };
    let Some(binder) = binder_parts(resolve, source) else {
        return Vec::new();
    };
    source
        .body
        .stmts
        .iter()
        .filter_map(|statement| {
            let Stmt::ComptimeTypeBind(binding) = statement else {
                return None;
            };
            if !binding_selects_binder_parts(resolve, binding, binder) {
                return None;
            }
            let rows = check.comptime_type_bindings.get(&binding.span)?;
            if rows.len() != fields.len() || fields.is_empty() {
                return None;
            }
            fields
                .iter()
                .enumerate()
                .all(|(ordinal, (ty, field))| {
                    let candidates = rows
                        .iter()
                        .filter(|row| {
                            row.selection
                                == CheckedComptimeTypeSelection::ReflectedIteration(ordinal)
                        })
                        .collect::<Vec<_>>();
                    let [row] = candidates.as_slice() else {
                        return false;
                    };
                    row.bound_type == *ty && row.reflection == field.type_info
                })
                .then_some(binding.span)
        })
        .collect()
}

fn metadata_list_matches(
    iterable: &Expression,
    element_type: TypeId,
    owner_name: &str,
    fields: &[(TypeId, ReflectionFieldInfo)],
    checked: &CheckedResourceProgram,
) -> bool {
    let types = &checked.checked().interner;
    let Type::Struct(id) = types.resolve(element_type) else {
        return false;
    };
    let definition = types.resolve_struct(*id);
    let expected = [
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
    if definition.name != "TypeField"
        || !definition
            .fields
            .iter()
            .map(|(name, _)| name.as_str())
            .eq(expected)
        || definition.fields[0].1 != TypeInterner::INT64
        || !matches!(types.resolve(iterable.ty), Type::List(element) if *element == element_type)
    {
        return false;
    }
    let E::ListConstruct { elements } = &iterable.kind else {
        return false;
    };
    elements.len() == fields.len()
        && elements.iter().zip(fields).all(|(element, (_, metadata))| {
            let E::StructConstruct {
                struct_type,
                fields,
                evaluation_order,
                validates_refinements: false,
                refinement_predicates,
            } = &element.kind
            else {
                return false;
            };
            *struct_type == element_type
                && element.ty == element_type
                && fields.len() == expected.len()
                && evaluation_order.iter().copied().eq(0..expected.len())
                && refinement_predicates.is_empty()
                && fields
                    .iter()
                    .zip(&definition.fields)
                    .all(|(field, (_, ty))| field.ty == *ty)
                && i128::try_from(metadata.index)
                    .ok()
                    .is_some_and(|index| matches!(fields[0].kind, E::Int(value) if value == index))
                && matches!(&fields[1].kind, E::String(value) if value == owner_name)
                && matches!(fields[2].kind, E::OptionalNone)
                && matches!(&fields[3].kind, E::String(value) if value == &metadata.name)
                && matches!(&fields[4].kind, E::String(value) if value == &metadata.type_name)
                && matches!(&fields[5].kind, E::String(value) if value == &metadata.kind)
                && metadata_tag_matches(
                    &fields[6],
                    ReflectionTypeInfo::kind_tag_variant(&metadata.kind),
                    types,
                )
                && matches!(&fields[7].kind, E::String(value) if value == &metadata.serialize_name)
                && matches!(fields[8].kind, E::Bool(value) if value == metadata.has_secret)
                && metadata_type_info_matches(&fields[9], &metadata.type_info, types)
        })
}

fn metadata_tag_matches(value: &Expression, expected: &str, types: &TypeInterner) -> bool {
    let E::EnumConstruct {
        enum_type,
        variant,
        payloads,
        evaluation_order,
    } = &value.kind
    else {
        return false;
    };
    let Type::Enum(id) = types.resolve(*enum_type) else {
        return false;
    };
    *enum_type == value.ty
        && payloads.is_empty()
        && evaluation_order.is_empty()
        && types
            .resolve_enum(*id)
            .variants
            .get(variant.index() as usize)
            .is_some_and(|variant| variant.name == expected && variant.fields.is_empty())
}

fn metadata_type_info_matches(
    value: &Expression,
    expected: &ReflectionTypeInfo,
    types: &TypeInterner,
) -> bool {
    let E::StructConstruct {
        struct_type,
        fields,
        evaluation_order,
        validates_refinements: false,
        refinement_predicates,
    } = &value.kind
    else {
        return false;
    };
    let Type::Struct(id) = types.resolve(*struct_type) else {
        return false;
    };
    let header = types.resolve_struct(*id);
    let members = [
        "type_name",
        "kind",
        "kind_tag",
        "primitive_tag",
        "has_secret",
        "args",
    ];
    if *struct_type != value.ty
        || header.name != "TypeInfo"
        || !header
            .fields
            .iter()
            .map(|(name, _)| name.as_str())
            .eq(members)
        || fields.len() != members.len()
        || !evaluation_order.iter().copied().eq(0..members.len())
        || !refinement_predicates.is_empty()
        || !fields
            .iter()
            .zip(&header.fields)
            .all(|(field, (_, ty))| field.ty == *ty)
        || !matches!(&fields[0].kind, E::String(name) if name == &expected.type_name)
        || !matches!(&fields[1].kind, E::String(kind) if kind == &expected.kind)
        || !metadata_tag_matches(
            &fields[2],
            ReflectionTypeInfo::kind_tag_variant(&expected.kind),
            types,
        )
        || !matches!(fields[4].kind, E::Bool(secret) if secret == expected.has_secret)
    {
        return false;
    }
    let primitive_matches = match (&fields[3].kind, expected.primitive_tag.as_deref()) {
        (E::OptionalNone, None) => true,
        (E::OptionalSome(tag), Some(expected)) => {
            matches!(types.resolve(fields[3].ty), Type::Optional(inner) if *inner == tag.ty)
                && metadata_tag_matches(tag, expected, types)
        }
        _ => false,
    };
    let E::ListConstruct { elements } = &fields[5].kind else {
        return false;
    };
    primitive_matches
        && matches!(types.resolve(fields[5].ty), Type::List(inner) if *inner == value.ty)
        && elements.len() == expected.args.len()
        && elements
            .iter()
            .zip(&expected.args)
            .all(|(argument, expected)| {
                argument.ty == value.ty && metadata_type_info_matches(argument, expected, types)
            })
}

fn root_dispatch_mut<'a>(body: &'a mut Block, path: &[usize]) -> Option<&'a mut Statement> {
    let [0, loop_index, 2, 0, 0, dispatch_index] = path else {
        return None;
    };
    let S::For { body, .. } = &mut body.statements.get_mut(*loop_index)?.kind else {
        return None;
    };
    let statement = body.statements.get_mut(*dispatch_index)?;
    matches!(statement.kind, S::ReflectedTypeDispatch { .. }).then_some(statement)
}
