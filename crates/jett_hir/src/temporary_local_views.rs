//! Ordinary temporary owners retained by their original function-body scope.
//! SourceCall carries an opaque marker; the immutable archive is the inventory.
use super::*;
use std::sync::{Arc, OnceLock};

#[cfg(test)]
mod tests;

struct Original {
    checked: Arc<jett_typecheck::CheckedResourceProgram>,
    declaration: CheckedBindingFact,
    declaration_address: usize,
    owner: Local,
    alias: Local,
    scope: TemporaryLocalScope,
    initializer: Expression,
    producer: Expression,
    canonical_producer: OnceLock<Expression>,
    rewritten: Expression,
    required_scopes: Vec<RequiredScopeIdentity>,
    scoped_types: Vec<ScopedTypeBinding>,
}

/// No lexical scope, checked occurrence or owner can be supplied by a caller.
#[derive(Clone)]
pub struct TemporaryLocalBacking(Arc<Original>);
impl std::fmt::Debug for TemporaryLocalBacking {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TemporaryLocalBacking")
            .field("owner", &self.owner())
            .field("alias", &self.alias())
            .field("scope", &self.0.scope)
            .finish()
    }
}
impl PartialEq for TemporaryLocalBacking {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for TemporaryLocalBacking {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TemporaryLocalArgument {
    proof: TemporaryLocalBacking,
    alias: LocalId,
    owner: LocalId,
}
impl TemporaryLocalArgument {
    pub(crate) fn metadata_locals(&self, mut visit: impl FnMut(LocalId)) {
        visit(self.alias);
        visit(self.owner);
    }
    pub(crate) fn remap<E>(
        &mut self,
        map: &mut impl FnMut(LocalId) -> Result<LocalId, E>,
    ) -> Result<(), E> {
        self.alias = map(self.alias)?;
        self.owner = map(self.owner)?;
        Ok(())
    }
    pub(crate) fn matches(&self, fact: &CallerBindingFact, value: OwnershipLocalInfo) -> bool {
        let original = &self.proof.0;
        self.alias == fact.local
            && fact.ty == original.alias.ty
            && fact.declaration_span == original.declaration.declaration_span
            && fact.mode
                == (CallerBindingMode::View {
                    source: CallerViewSource::Other,
                })
            && !fact.mutable
            && value.view_source == Some(self.owner)
            && !value.mutable
            && !value.is_view_parameter
            && value.view_iteration.is_none()
            && value.match_binding.is_none()
    }
}

#[derive(Debug)]
pub struct TemporaryLocalScope {
    body: Span,
    statement: usize,
}
impl TemporaryLocalScope {
    pub fn body_span(&self) -> Span {
        self.body
    }
    pub fn statement_index(&self) -> usize {
        self.statement
    }
}
impl TemporaryLocalBacking {
    pub fn owner(&self) -> LocalId {
        self.0.owner.id
    }
    pub fn alias(&self) -> LocalId {
        self.0.alias.id
    }
    pub fn original_scope(&self) -> &TemporaryLocalScope {
        &self.0.scope
    }
    pub fn original_initializer(&self) -> &Expression {
        &self.0.initializer
    }
    pub fn owner_initializer(&self) -> &Expression {
        self.0.canonical_producer.get().unwrap_or(&self.0.producer)
    }
    pub fn rewritten_initializer(&self) -> &Expression {
        &self.0.rewritten
    }

    /// Ignore only this exact original marker, never checked argument/call data.
    pub fn matches_owner_initializer(&self, value: &Expression) -> bool {
        let mut value = value.clone();
        let ExpressionKind::Call {
            ownership: CallOwnership::Source(source),
            ..
        } = &mut value.kind
        else {
            return false;
        };
        if source.temporary_local_backing() != Some(self) {
            return false;
        }
        source.clear_temporary_local_backing();
        self.0
            .canonical_producer
            .get()
            .is_some_and(|original| resource_materialization::expression_equal(&value, original))
    }
}

fn marker(value: &Expression) -> Option<&TemporaryLocalBacking> {
    let ExpressionKind::Call {
        ownership: CallOwnership::Source(source),
        ..
    } = &value.kind
    else {
        return None;
    };
    source.temporary_local_backing()
}
fn markers(function: &Function) -> Vec<TemporaryLocalBacking> {
    let mut body = function.body.clone();
    let mut result = Vec::new();
    let _ = resource_materialization::walk_block(&mut body, &mut |value| {
        if let Some(proof) = marker(value) {
            result.push(proof.clone());
        }
        Ok(())
    });
    result
}
pub(super) fn has_markers(program: &Program) -> bool {
    program
        .functions
        .iter()
        .any(|function| !markers(function).is_empty())
}
fn expected(program: &Program) -> bool {
    program
        .resource_source
        .functions()
        .iter()
        .any(|function| !markers(function).is_empty())
}

/// The immutable original, rather than an optional current marker, determines
/// which declarations owe storage. Whole-archive erasure remains the preexisting
/// public Program boundary; this API claims no protection beyond that boundary.
pub fn temporary_local_backings(
    program: &Program,
    function: &Function,
    types: &TypeInterner,
) -> Result<Vec<TemporaryLocalBacking>, String> {
    if !expected(program) && !has_markers(program) {
        return Ok(Vec::new());
    }
    program.resource_source.validate_current(program, types)?;
    let current = program
        .functions
        .get(function.id.index() as usize)
        .filter(|current| current.id == function.id)
        .ok_or("temporary backing function is absent")?;
    if !resource_materialization::functions_equal(
        std::slice::from_ref(current),
        std::slice::from_ref(function),
    ) {
        return Err("temporary backing consumer supplied a different function".into());
    }
    let Some(original) = program
        .resource_source
        .functions()
        .iter()
        .find(|original| original.id == function.id)
    else {
        // Canonical conversion may append adapters. The complete current table
        // was authenticated above; an appended function cannot import a Source
        // temporary occurrence of its own.
        return if markers(current).is_empty() {
            Ok(Vec::new())
        } else {
            Err("temporary backing has no original checked function".into())
        };
    };
    let originals = markers(original);
    let actual = markers(current);
    if originals != actual {
        return Err("temporary backing lost its exact original inventory".into());
    }
    for (index, proof) in originals.iter().enumerate() {
        if originals[..index]
            .iter()
            .any(|previous| previous.alias() == proof.alias() || previous.owner() == proof.owner())
        {
            return Err("temporary backing duplicated an original declaration".into());
        }
        validate_record(program, current, proof, types)?;
    }
    Ok(originals)
}

pub(super) fn validate_program(
    program: &Program,
    types: &TypeInterner,
) -> Result<(), Vec<ValidationError>> {
    if expected(program) || has_markers(program) {
        let span = program
            .resource_source
            .functions()
            .first()
            .map_or(Span::new(FileId::new(0), 0, 0), |function| function.span);
        program
            .resource_source
            .validate_current(program, types)
            .map_err(|message| vec![ValidationError { span, message }])?;
    }
    for function in &program.functions {
        temporary_local_backings(program, function, types).map_err(|message| {
            vec![ValidationError {
                span: function.span,
                message,
            }]
        })?;
    }
    Ok(())
}

fn validate_record(
    program: &Program,
    function: &Function,
    proof: &TemporaryLocalBacking,
    types: &TypeInterner,
) -> Result<(), String> {
    let checked = program
        .resource_source
        .checked_program()
        .ok_or("temporary backing has no checked Source")?;
    if !Arc::ptr_eq(checked, &proof.0.checked) {
        return Err("temporary backing changed its checked Source session".into());
    }
    let owner = validate_source_record(function, proof, types)?;
    if !proof.matches_owner_initializer(owner) {
        return Err("temporary backing changed its canonical original production".into());
    }
    Ok(())
}

fn validate_source_record<'a>(
    function: &'a Function,
    proof: &TemporaryLocalBacking,
    types: &TypeInterner,
) -> Result<&'a Expression, String> {
    let data = &proof.0;
    if function.identity.declaration.kind != DeclarationKind::Function
        || !function.identity.type_arguments.is_empty()
        || !function.identity.scoped_type_bindings.is_empty()
        || function.body.span != data.scope.body
        || !data.required_scopes.is_empty()
        || !data.scoped_types.is_empty()
        || function.locals.get(data.owner.id.index() as usize) != Some(&data.owner)
        || function.locals.get(data.alias.id.index() as usize) != Some(&data.alias)
        || function
            .params
            .iter()
            .any(|param| param.local == data.owner.id || param.local == data.alias.id)
        || !matches!(
            data.declaration.mode,
            CheckedBindingMode::View {
                source: CheckedViewSource::Other
            }
        )
    {
        return Err("temporary backing changed its checked occurrence, scope or headers".into());
    }
    let source = original_source_body(&data.checked, function)
        .ok_or("temporary backing lost its original named function")?;
    let Some(Stmt::VarDecl(declaration)) = source.stmts.get(data.scope.statement) else {
        return Err("temporary backing original declaration is absent".into());
    };
    if std::ptr::from_ref(declaration).addr() != data.declaration_address
        || declaration.name.span != data.declaration.declaration_span
        || declaration.mutable
        || !matches!(declaration.value, Expr::View(..))
    {
        return Err("temporary backing original declaration changed".into());
    }
    let mut owner_sites = Vec::new();
    let mut alias_sites = Vec::new();
    // Only original root-body declarations are supported in this first slice.
    for (index, statement) in function.body.statements.iter().enumerate() {
        if let StatementKind::Let { local, value } = &statement.kind {
            if *local == data.owner.id {
                owner_sites.push((index, value));
            }
            if *local == data.alias.id {
                alias_sites.push((index, value));
            }
        }
    }
    let ([(owner_index, owner)], [(alias_index, alias)]) =
        (owner_sites.as_slice(), alias_sites.as_slice())
    else {
        return Err("temporary backing lost unique original owner and alias initialization".into());
    };
    if *alias_index != *owner_index + 1
        || !resource_materialization::expression_equal(alias, &data.rewritten)
    {
        return Err(
            "temporary backing changed production, projection or initialization order".into(),
        );
    }
    local_views::validate_local_view_initializer(
        alias,
        data.owner.id,
        data.owner.ty,
        data.alias.ty,
        types,
    )?;
    Ok(owner)
}

/// This is called only from the original Lowerer, after its canonical passes
/// and before its immutable archive is captured. It is deliberately single use;
/// public conversion entry points can never fill or replace this record.
pub(super) fn seal_original_program(
    program: &Program,
    types: &TypeInterner,
) -> Result<(), Vec<LowerError>> {
    let mut pending = Vec::new();
    for function in &program.functions {
        for proof in markers(function) {
            let fail = |message: String| {
                vec![LowerError {
                    span: function.span,
                    message,
                }]
            };
            if proof.0.canonical_producer.get().is_some()
                || pending
                    .iter()
                    .any(|(previous, _): &(TemporaryLocalBacking, Expression)| previous == &proof)
            {
                return Err(fail(
                    "temporary original production was already captured".into(),
                ));
            }
            let owner = validate_source_record(function, &proof, types).map_err(fail)?;
            let mut canonical = owner.clone();
            let ExpressionKind::Call {
                function: current_target,
                args: current_args,
                evaluation_order: current_order,
                ownership: CallOwnership::Source(current_ownership),
            } = &mut canonical.kind
            else {
                return Err(fail(
                    "temporary original owner lost its checked direct invocation".into(),
                ));
            };
            let ExpressionKind::Call {
                function: original_target,
                args: original_args,
                evaluation_order: original_order,
                ownership: CallOwnership::Source(original_ownership),
            } = &proof.0.producer.kind
            else {
                return Err(fail(
                    "temporary original producer is not its checked direct invocation".into(),
                ));
            };
            if current_ownership.temporary_local_backing() != Some(&proof) {
                return Err(fail(
                    "temporary original owner changed its occurrence marker".into(),
                ));
            }
            current_ownership.clear_temporary_local_backing();
            // Canonical coercion changes operands, never the retained Source
            // call certificate, target, source order, result or source span.
            if *current_target != *original_target
                || current_args.len() != original_args.len()
                || current_order.as_slice() != original_order.as_slice()
                || &*current_ownership != original_ownership
                || canonical.ty != proof.0.producer.ty
                || canonical.span != proof.0.producer.span
            {
                return Err(fail(
                    "temporary canonical production changed its checked invocation headers".into(),
                ));
            }
            pending.push((proof, canonical));
        }
    }
    for (proof, canonical) in pending {
        proof.0.canonical_producer.set(canonical).map_err(|_| {
            vec![LowerError {
                span: proof.0.declaration.declaration_span,
                message: "temporary original production was already captured".into(),
            }]
        })?;
    }
    Ok(())
}

fn original_source_body<'a>(
    checked: &'a jett_typecheck::CheckedResourceProgram,
    function: &Function,
) -> Option<&'a ast::Block> {
    let definition = function.source_definition?;
    checked.module().items.iter().find_map(|item| match item {
        Item::Function(source)
            if source.span == function.span
                && reflected_field_sources::original_root_function(
                    checked.module(),
                    checked.resolved(),
                    source,
                    definition,
                ) =>
        {
            Some(&source.body)
        }
        _ => None,
    })
}

fn finite_ordinary(types: &TypeInterner, ty: TypeId, visiting: &mut HashSet<TypeId>) -> bool {
    if ty.index() as usize >= types.len() || !visiting.insert(ty) {
        return false;
    }
    let valid = match types.resolve(ty) {
        Type::Int8
        | Type::Int16
        | Type::Int32
        | Type::Int64
        | Type::Uint8
        | Type::Uint16
        | Type::Uint32
        | Type::Uint64
        | Type::Float32
        | Type::Float64
        | Type::String
        | Type::Bool
        | Type::Bytes
        | Type::Nothing => true,
        Type::List(inner) => finite_ordinary(types, *inner, visiting),
        Type::Struct(id) => types
            .resolve_struct(*id)
            .fields
            .iter()
            .all(|(_, field)| finite_ordinary(types, *field, visiting)),
        _ => false,
    };
    visiting.remove(&ty);
    valid
}
fn producer<'a>(value: &'a Expression, types: &TypeInterner) -> Option<&'a Expression> {
    let ExpressionKind::View(current) = &value.kind else {
        return None;
    };
    let mut current = current.as_ref();
    let mut fields = 0;
    while let ExpressionKind::Field {
        base, owner_type, ..
    } = &current.kind
    {
        if !matches!(types.resolve(*owner_type), Type::Struct(_)) {
            return None;
        }
        local_views::validate_field_projection(current, types).ok()?;
        fields += 1;
        current = base;
    }
    if fields == 0
        || !matches!(types.resolve(value.ty), Type::Bytes | Type::List(_))
        || !matches!(types.resolve(current.ty), Type::Struct(_))
        || !finite_ordinary(types, current.ty, &mut HashSet::new())
    {
        return None;
    }
    let ExpressionKind::Call {
        ownership: CallOwnership::Source(source),
        ..
    } = &current.kind
    else {
        return None;
    };
    (source.bridge == CallBridge::Direct
        && source.context == jett_typecheck::CheckedOwnershipContext::Ordinary
        && source.temporary_local_backing().is_none())
    .then_some(current)
}
fn replace_producer(value: &mut Expression, owner: LocalId) {
    match &mut value.kind {
        ExpressionKind::View(inner) => replace_producer(inner, owner),
        ExpressionKind::Field { base, .. } => replace_producer(base, owner),
        _ => value.kind = ExpressionKind::Local(owner),
    }
}

impl BodyLowerer<'_, '_> {
    /// Called only for genuine Other facts; unsuccessful recognition preserves
    /// the existing conservative backend refusal without changing Source rules.
    pub(super) fn lower_temporary_local_declaration(
        &mut self,
        declaration: &ast::VarDecl,
        definition: DefId,
        value: &Expression,
        ty: TypeId,
    ) -> Option<(LocalId, Expression)> {
        let root = self.temporary_root?;
        let (scope, index) = self.temporary_scope?;
        let checked = self.parent.reflected_checked_program.clone()?;
        let fact = *self.binding_facts.get(&declaration.name.span)?;
        if scope != root.span
            || self.required_scopes.len() != 0
            || self.scoped_type_bindings.len() != 0
            || !matches!(root.stmts.get(index), Some(Stmt::VarDecl(original)) if std::ptr::eq(original, declaration))
            || declaration.mutable
            || !matches!(declaration.value, Expr::View(..))
            || fact.definition != definition
            || fact.declaration_span != declaration.name.span
            || fact.ty != ty
            || fact.mutable
            || fact.mode
                != (CheckedBindingMode::View {
                    source: CheckedViewSource::Other,
                })
            || self.binding_modes.get(&declaration.name.span) != Some(&fact.mode)
        {
            return None;
        }
        let production = producer(value, &self.parent.check.interner)?.clone();
        let ExpressionKind::Call {
            function: callee,
            ownership,
            args,
            evaluation_order,
        } = &production.kind
        else {
            return None;
        };
        let CallOwnership::Source(source) = ownership else {
            return None;
        };
        if source.target != CallTarget::Function(*callee)
            || call_ownership::validate_operand_ownership(
                ownership,
                &args.iter().map(|argument| argument.ty).collect::<Vec<_>>(),
                evaluation_order,
                &self.parent.check.interner,
                |local| self.ownership_local_info(local),
                true,
            )
            .is_err()
        {
            return None;
        };
        // Lowerer::lower drains its scheduling list before lowering bodies.
        // Join the immutable original declaration to the retained exact ID map,
        // rather than looking for authority in that now-empty scheduling list.
        let ordinary_target = checked.module().items.iter().any(|item| {
            let Item::Function(source) = item else {
                return false;
            };
            let Some(definition) = self
                .parent
                .definition_at(source.name.span, DefKind::Function)
            else {
                return false;
            };
            source.type_params.is_empty()
                && reflected_field_sources::original_root_function(
                    self.parent.module,
                    self.parent.resolve,
                    source,
                    definition,
                )
                && self.function_ids.get(&FunctionKey::Definition {
                    definition,
                    concrete_args: Vec::new(),
                    specialization: CheckedGenericSpecialization::default(),
                }) == Some(callee)
        });
        if !ordinary_target {
            return None;
        }
        let owner = LocalId(u32::try_from(self.locals.len()).ok()?);
        let owner_header = Local {
            id: owner,
            name: String::new(),
            ty: production.ty,
            debug_ty: production.ty,
            debug_type_name: None,
            mutable: false,
            view_source: None,
            span: declaration.span,
        };
        self.locals.push(owner_header.clone());
        let alias = self.allocate_local(
            definition,
            &declaration.name.name,
            ty,
            false,
            declaration.span,
        );
        self.locals[alias.index() as usize].view_source = Some(owner);
        let mut rewritten = value.clone();
        replace_producer(&mut rewritten, owner);
        let proof = TemporaryLocalBacking(Arc::new(Original {
            checked,
            declaration: fact,
            declaration_address: std::ptr::from_ref(declaration).addr(),
            owner: owner_header,
            alias: self.locals[alias.index() as usize].clone(),
            scope: TemporaryLocalScope {
                body: scope,
                statement: index,
            },
            initializer: value.clone(),
            producer: production.clone(),
            canonical_producer: OnceLock::new(),
            rewritten: rewritten.clone(),
            required_scopes: self.required_scopes.clone(),
            scoped_types: self.scoped_type_bindings.clone(),
        }));
        let mut production = production;
        let ExpressionKind::Call {
            ownership: CallOwnership::Source(source),
            ..
        } = &mut production.kind
        else {
            return None;
        };
        source.set_temporary_local_backing(proof.clone());
        self.temporary_origins.insert(alias, proof);
        self.temporary_prefix.push(Statement {
            kind: StatementKind::Let {
                local: owner,
                value: production,
            },
            span: declaration.span,
        });
        Some((alias, rewritten))
    }
    pub(super) fn temporary_argument_origin(
        &self,
        origin: &CallerOrigin,
    ) -> Option<Box<TemporaryLocalArgument>> {
        let CallerOrigin::Binding(fact) = origin else {
            return None;
        };
        let proof = self.temporary_origins.get(&fact.local)?.clone();
        Some(Box::new(TemporaryLocalArgument {
            owner: proof.owner(),
            alias: proof.alias(),
            proof,
        }))
    }
}

// These transitions update the existing execution-functions view only after
// exact authenticated replay. Reusing this archive grants no Resource custody,
// execution family, provider or manifest scope.
fn semantic_functions(program: &Program) -> Vec<Function> {
    let mut functions = program.functions.clone();
    for function in &mut functions {
        function.mutable_sum_backings.clear();
        function.actor_handler = None;
        function.actor_state_views.clear();
    }
    functions
}
fn independently_validated(program: &Program) -> Result<(), Vec<LowerError>> {
    mutable_sum_backings::validate_program(program)
        .and_then(|()| actor_handlers::validate_program(program))
        .map_err(|errors| {
            errors
                .into_iter()
                .map(|error| LowerError {
                    span: error.span,
                    message: error.message,
                })
                .collect()
        })
}
fn finish_replay(
    program: &Program,
    before: &Program,
    replay: &Program,
) -> Result<resource_source::Materialized, Vec<LowerError>> {
    independently_validated(before)?;
    independently_validated(program)?;
    let span = before
        .functions
        .first()
        .map_or(Span::new(FileId::new(0), 0, 0), |f| f.span);
    if before.resource_source != program.resource_source
        || before.resource_manifest != program.resource_manifest
        || replay.equality_methods != program.equality_methods
        || !resource_materialization::functions_equal(
            &semantic_functions(program),
            &semantic_functions(replay),
        )
    {
        return Err(vec![LowerError {
            span,
            message: "temporary backing changed outside its exact authenticated transition".into(),
        }]);
    }
    let archive = before.resource_source.clone();
    let view = resource_source::Materialized {
        functions: program.functions.clone(),
        values: archive.required_materializations().to_vec(),
        required_only: archive.required_only_function_ids().to_vec(),
    };
    let mut candidate = program.clone();
    let native_suites = native_suite_transition::converted(program, before)
        .map_err(|message| vec![LowerError { span, message }])?;
    if native_suites.is_some() {
        candidate.resource_source.native_suites = native_suites;
    }
    candidate
        .resource_source
        .install_materialized(resource_source::Materialized {
            functions: view.functions.clone(),
            values: view.values.clone(),
            required_only: view.required_only.clone(),
        });
    validate_program(
        &candidate,
        &archive
            .checked_program()
            .ok_or_else(|| {
                vec![LowerError {
                    span,
                    message: "temporary transition lost checked Source".into(),
                }]
            })?
            .checked()
            .interner,
    )
    .map_err(|errors| {
        errors
            .into_iter()
            .map(|error| LowerError {
                span: error.span,
                message: error.message,
            })
            .collect::<Vec<_>>()
    })?;
    Ok(view)
}
pub(super) fn conversion(
    program: &Program,
    before: &Program,
    types: &TypeInterner,
    conversion: &interface_values::CanonicalValueConversion<'_>,
) -> Result<Option<resource_source::Materialized>, Vec<LowerError>> {
    if !expected(before) && !has_markers(before) {
        return Ok(None);
    }
    validate_program(before, types).map_err(|errors| {
        errors
            .into_iter()
            .map(|error| LowerError {
                span: error.span,
                message: error.message,
            })
            .collect::<Vec<_>>()
    })?;
    let replay = conversion.replay(before, types)?;
    // The caller joins only this execution view after its peer Graph/baked
    // transitions authenticate the same unchanged archive independently.
    finish_replay(program, before, &replay).map(Some)
}
pub(super) fn callable_sealing(
    program: &mut Program,
    before: &Program,
    edits: &[(ReflectedCallablePlan, ReflectedCallablePlan)],
) -> Result<(), Vec<LowerError>> {
    if !expected(before) {
        return Ok(());
    } // Initial genuine sealing precedes archive capture.
    let types = &before
        .resource_source
        .checked_program()
        .ok_or_else(|| {
            vec![LowerError {
                span: before.functions[0].span,
                message: "temporary callable replay lost original Source".into(),
            }]
        })?
        .checked()
        .interner;
    validate_program(before, types).map_err(|errors| {
        errors
            .into_iter()
            .map(|error| LowerError {
                span: error.span,
                message: error.message,
            })
            .collect::<Vec<_>>()
    })?;
    let mut replay = before.clone();
    for function in &mut replay.functions {
        resource_materialization::walk_block(&mut function.body, &mut |value| {
            let replace = |plan: &mut ReflectedCallablePlan| {
                if let Some((_, current)) = edits.iter().find(|(previous, _)| previous == plan) {
                    *plan = current.clone();
                }
            };
            match &mut value.kind {
                ExpressionKind::Intrinsic {
                    field_validation: Some(ReflectedFieldValidation::Validate(plans)),
                    ..
                } => {
                    for plan in plans {
                        if let ReflectedFieldAction::CallableReturn(callable) = &mut plan.action {
                            replace(callable);
                        }
                    }
                }
                ExpressionKind::Handle {
                    kind:
                        HandleKind::ReflectedReturn {
                            callable: Some(callable),
                            ..
                        },
                    ..
                } => replace(callable),
                _ => {}
            }
            Ok(())
        })
        .map_err(|message| {
            vec![LowerError {
                span: function.span,
                message,
            }]
        })?;
    }
    let view = finish_replay(program, before, &replay)?;
    program.resource_source.install_materialized(view);
    Ok(())
}
