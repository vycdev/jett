//! Original checked mutable owners for canonical borrowed Handle declarations.
//! This proof supplies no loan expiry, Resource custody or alternate Default.
use super::*;
use std::sync::Arc;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone)]
pub struct MutableSumBacking {
    pub(crate) alias: LocalId,
    backing: LocalId,
    alias_fact: CheckedBindingFact,
    backing_fact: CheckedBindingFact,
    source_initializer: Expression,
    initializer: Expression,
    required_scopes: Vec<RequiredScopeIdentity>,
    scoped_types: Vec<ScopedTypeBinding>,
    producer: Arc<()>,
    original: Option<Arc<Function>>,
}
impl PartialEq for MutableSumBacking {
    fn eq(&self, other: &Self) -> bool {
        self.alias == other.alias
            && self.backing == other.backing
            && self.alias_fact == other.alias_fact
            && self.backing_fact == other.backing_fact
            && self.required_scopes == other.required_scopes
            && self.scoped_types == other.scoped_types
            && Arc::ptr_eq(&self.producer, &other.producer)
            && resource_materialization::expression_equal(
                &self.source_initializer,
                &other.source_initializer,
            )
            && resource_materialization::expression_equal(&self.initializer, &other.initializer)
            && match (&self.original, &other.original) {
                (None, None) => true,
                (Some(a), Some(b)) => resource_materialization::functions_equal(
                    std::slice::from_ref(a.as_ref()),
                    std::slice::from_ref(b.as_ref()),
                ),
                _ => false,
            }
    }
}
fn snapshot(function: &Function) -> Function {
    let mut result = function.clone();
    result.mutable_sum_backings.clear();
    // These independent receipts renew at their own authenticated transitions.
    // This snapshot pins the real function identity/header/locals/operations.
    result.actor_handler = None;
    result.actor_state_views.clear();
    result
}
impl MutableSumBacking {
    fn current(&self, function: &Function) -> Result<(), String> {
        let original = self
            .original
            .as_ref()
            .ok_or("mutable sum backing has no original function")?;
        if !resource_materialization::functions_equal(
            std::slice::from_ref(original.as_ref()),
            &[snapshot(function)],
        ) {
            return Err("mutable sum backing changed its original function context or body".into());
        }
        let alias = function
            .locals
            .get(self.alias.index() as usize)
            .filter(|local| local.id == self.alias)
            .ok_or("mutable sum alias is missing")?;
        let backing = function
            .locals
            .get(self.backing.index() as usize)
            .filter(|local| local.id == self.backing)
            .ok_or("mutable sum owner is missing")?;
        if !backing.mutable
            || backing.view_source.is_some()
            || self.backing_fact.mode != CheckedBindingMode::Owned
            || !self.backing_fact.mutable
            || backing.ty != self.backing_fact.ty
            || backing.span.file != self.backing_fact.declaration_span.file
            || backing.span.start > self.backing_fact.declaration_span.start
            || backing.span.end < self.backing_fact.declaration_span.end
            || function
                .params
                .iter()
                .any(|param| param.local == self.backing && param.mode != ParamMode::Owned)
            || alias.ty != self.alias_fact.ty
            || alias.mutable != self.alias_fact.mutable
            || alias.span.file != self.alias_fact.declaration_span.file
            || alias.span.start > self.alias_fact.declaration_span.start
            || alias.span.end < self.alias_fact.declaration_span.end
        {
            return Err("mutable sum backing lost its exact checked owned binding".into());
        }
        let expected_source = match self.alias_fact.mode {
            CheckedBindingMode::Owned => None,
            CheckedBindingMode::View {
                source: CheckedViewSource::Binding(source),
            } if source == self.backing_fact.definition && !alias.mutable => Some(self.backing),
            _ => return Err("mutable sum alias changed its checked acquisition".into()),
        };
        if alias.view_source != expected_source {
            return Err("mutable sum alias changed its checked acquisition".into());
        }
        Ok(())
    }
}

impl Function {
    /// Consumed only with the actual original canonical Let initializer.
    pub fn mutable_sum_backing(
        &self,
        alias: LocalId,
        initializer: &Expression,
        backing: LocalId,
    ) -> Result<Option<&MutableSumBacking>, String> {
        validate_function(self)?;
        let mut matches = self
            .mutable_sum_backings
            .iter()
            .filter(|proof| proof.alias == alias);
        let Some(proof) = matches.next() else {
            return Ok(None);
        };
        if matches.next().is_some()
            || proof.backing != backing
            || !resource_materialization::expression_equal(&proof.initializer, initializer)
        {
            return Err("mutable sum backing differs from its exact original declaration".into());
        }
        Ok(Some(proof))
    }
}

impl BodyLowerer<'_, '_> {
    /// Run for both final Owned copies and true View aliases. Neither the
    /// absence of view_source nor a mutable header establishes ownership.
    pub(super) fn record_mutable_sum_backing(
        &mut self,
        declaration: &ast::VarDecl,
        definition: DefId,
        alias: LocalId,
        value: &Expression,
    ) -> Result<(), String> {
        let ExpressionKind::View(handle) = &value.kind else {
            return Ok(());
        };
        let ExpressionKind::Handle { target, .. } = &handle.kind else {
            return Ok(());
        };
        let ExpressionKind::View(source) = &target.kind else {
            return Ok(());
        };
        let ExpressionKind::Local(backing) = source.kind else {
            return Ok(());
        };
        let header = self
            .locals
            .get(backing.index() as usize)
            .filter(|local| local.id == backing)
            .ok_or("mutable sum backing is missing")?;
        if !header.mutable {
            return Ok(());
        }
        if !matches!(
            self.parent.check.interner.resolve(header.ty),
            Type::Optional(_) | Type::Result(_, _)
        ) {
            return Ok(());
        }
        local_views::borrowed_sum_view_initializer(
            value,
            backing,
            header.ty,
            value.ty,
            &self.parent.check.interner,
        )?
        .ok_or("mutable sum backing lost its canonical Handle")?;
        if !self.mutable_alias_backing_is_owned(backing) {
            return Err("mutable borrowed Handle lacks its exact selected owned backing".into());
        }
        let root_definition = self
            .local_ids
            .iter()
            .find_map(|(definition, local)| (*local == backing).then_some(*definition))
            .ok_or("mutable sum root has no original definition")?;
        let root_span = self.parent.resolve.scope_table.def(root_definition).span;
        let backing_fact = *self
            .binding_facts
            .get(&root_span)
            .ok_or("mutable sum root has no selected fact")?;
        let alias_fact = *self
            .binding_facts
            .get(&declaration.name.span)
            .filter(|fact| {
                fact.definition == definition
                    && fact.declaration_span == declaration.name.span
                    && fact.ty == value.ty
                    && fact.mutable == declaration.mutable
            })
            .ok_or("mutable sum alias has no exact selected binding fact")?;
        let copyable = jett_typecheck::ownership::is_implicitly_copyable(
            &self.parent.check.interner,
            value.ty,
        );
        let mode = if copyable {
            CheckedBindingMode::Owned
        } else {
            CheckedBindingMode::View {
                source: CheckedViewSource::Binding(root_definition),
            }
        };
        if alias_fact.mode != mode || (!copyable && declaration.mutable) {
            return Err("mutable sum alias lost its checked acquisition mode".into());
        }
        self.mutable_sum_backings.push(MutableSumBacking {
            alias,
            backing,
            alias_fact,
            backing_fact,
            source_initializer: value.clone(),
            initializer: value.clone(),
            required_scopes: self.required_scopes.clone(),
            scoped_types: self.scoped_type_bindings.clone(),
            producer: Arc::new(()),
            original: None,
        });
        Ok(())
    }
}

fn validate_function(function: &Function) -> Result<(), String> {
    for (index, proof) in function.mutable_sum_backings.iter().enumerate() {
        if function.mutable_sum_backings[..index]
            .iter()
            .any(|old| old.alias == proof.alias)
        {
            return Err("mutable sum backing duplicated an original declaration".into());
        }
        proof.current(function)?;
    }
    Ok(())
}
pub(super) fn validate_program(program: &Program) -> Result<(), Vec<ValidationError>> {
    for function in &program.functions {
        validate_function(function).map_err(|message| {
            vec![ValidationError {
                span: function.span,
                message,
            }]
        })?;
    }
    Ok(())
}
fn prepared_initializer(
    function: &Function,
    proof: &MutableSumBacking,
    types: &TypeInterner,
) -> Result<Expression, String> {
    let value = physical_initializer(function, proof)?;
    local_views::borrowed_sum_view_initializer(
        &value,
        proof.backing,
        proof.backing_fact.ty,
        proof.alias_fact.ty,
        types,
    )?
    .ok_or("mutable sum backing lost its canonical prepared initializer")?;
    Ok(value)
}
fn physical_initializer(
    function: &Function,
    proof: &MutableSumBacking,
) -> Result<Expression, String> {
    let matches = declarations(&function.body)
        .into_iter()
        .filter(|(local, _)| *local == proof.alias)
        .collect::<Vec<_>>();
    let [(.., value)] = matches.as_slice() else {
        return Err("mutable sum backing lost its unique physical declaration".into());
    };
    Ok((**value).clone())
}
pub(super) fn seal_program(
    program: &mut Program,
    types: &TypeInterner,
) -> Result<(), Vec<LowerError>> {
    for function in &mut program.functions {
        let prepared = function
            .mutable_sum_backings
            .iter()
            .map(|proof| prepared_initializer(function, proof, types))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|message| {
                vec![LowerError {
                    span: function.span,
                    message,
                }]
            })?;
        let original = Arc::new(snapshot(function));
        for (proof, value) in function.mutable_sum_backings.iter_mut().zip(prepared) {
            if proof.original.is_none() {
                proof.initializer = value;
                proof.original = Some(original.clone());
            }
        }
        validate_function(function).map_err(|message| {
            vec![LowerError {
                span: function.span,
                message,
            }]
        })?;
    }
    Ok(())
}
pub(super) fn conversion(
    program: &mut Program,
    before: &Program,
    types: &TypeInterner,
) -> Result<(), Vec<LowerError>> {
    validate_program(before).map_err(|errors| {
        errors
            .into_iter()
            .map(|error| LowerError {
                span: error.span,
                message: error.message,
            })
            .collect::<Vec<_>>()
    })?;
    let mut expected = before.clone();
    interface_values::coerce_program(&mut expected, types);
    for original in &before.functions {
        if original.mutable_sum_backings.is_empty() {
            continue;
        }
        let fail = || {
            vec![LowerError {
                span: original.span,
                message: "mutable sum backing conversion changed its original proof or operations"
                    .into(),
            }]
        };
        let replay = expected
            .functions
            .iter()
            .find(|function| function.id == original.id)
            .ok_or_else(fail)?;
        let current = program
            .functions
            .iter_mut()
            .find(|function| function.id == original.id)
            .ok_or_else(fail)?;
        if current.mutable_sum_backings != original.mutable_sum_backings
            || !resource_materialization::functions_equal(&[snapshot(current)], &[snapshot(replay)])
        {
            return Err(fail());
        }
        let updated = Arc::new(snapshot(replay));
        let prepared = current
            .mutable_sum_backings
            .iter()
            .map(|proof| prepared_initializer(replay, proof, types))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| fail())?;
        for (proof, value) in current.mutable_sum_backings.iter_mut().zip(prepared) {
            proof.initializer = value;
            proof.original = Some(updated.clone());
        }
    }
    validate_program(program).map_err(|errors| {
        errors
            .into_iter()
            .map(|error| LowerError {
                span: error.span,
                message: error.message,
            })
            .collect()
    })
}

/// Only reflected_callable::seal_program supplies these exact old/new tokens.
/// Replay the successful private sealing operation; public current metadata is
/// never used as replacement authority, and callable identity remains exact.
pub(super) fn callable_sealing(
    program: &mut Program,
    before: &Program,
    edits: &[(ReflectedCallablePlan, ReflectedCallablePlan)],
) -> Result<(), Vec<LowerError>> {
    fn replace(value: &mut Expression, edits: &[(ReflectedCallablePlan, ReflectedCallablePlan)]) {
        let replace_plan = |plan: &mut ReflectedCallablePlan| {
            if let Some((_, sealed)) = edits.iter().find(|(previous, _)| previous == &*plan) {
                *plan = sealed.clone();
            }
        };
        match &mut value.kind {
            ExpressionKind::Intrinsic {
                field_validation: Some(ReflectedFieldValidation::Validate(plans)),
                ..
            } => {
                for plan in plans {
                    if let ReflectedFieldAction::CallableReturn(callable) = &mut plan.action {
                        replace_plan(callable);
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
            } => replace_plan(callable),
            _ => {}
        }
    }
    for original in &before.functions {
        if original.mutable_sum_backings.is_empty()
            || original
                .mutable_sum_backings
                .iter()
                .all(|proof| proof.original.is_none())
        {
            // The initial genuine lowering is sealed after this pass finishes.
            continue;
        }
        let fail = || {
            vec![LowerError {
                span: original.span,
                message: "mutable sum backing changed outside original callable sealing".into(),
            }]
        };
        validate_function(original).map_err(|_| fail())?;
        let current = program
            .functions
            .iter_mut()
            .find(|function| function.id == original.id)
            .ok_or_else(fail)?;
        if current.mutable_sum_backings != original.mutable_sum_backings {
            return Err(fail());
        }
        let mut expected = snapshot(original);
        resource_materialization::walk_block(&mut expected.body, &mut |value| {
            replace(value, edits);
            Ok(())
        })
        .map_err(|_| fail())?;
        if !resource_materialization::functions_equal(&[expected], &[snapshot(current)]) {
            return Err(fail());
        }
        let renewed = Arc::new(snapshot(current));
        let prepared = current
            .mutable_sum_backings
            .iter()
            .map(|proof| physical_initializer(current, proof))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| fail())?;
        for (proof, value) in current.mutable_sum_backings.iter_mut().zip(prepared) {
            proof.initializer = value;
            proof.original = Some(renewed.clone());
        }
        validate_function(current).map_err(|_| fail())?;
    }
    Ok(())
}

fn declarations(block: &crate::Block) -> Vec<(LocalId, &Expression)> {
    fn expression<'a>(value: &'a Expression, output: &mut Vec<(LocalId, &'a Expression)>) {
        use crate::ExpressionKind as E;
        match &value.kind {
            E::Binary { left, right, .. } => {
                expression(left, output);
                expression(right, output);
            }
            E::Unary { value, .. }
            | E::ResultOk(value)
            | E::ResultFail(value)
            | E::OptionalSome(value)
            | E::DisplayResult(value)
            | E::EquatableResult(value)
            | E::Declassify(value)
            | E::Coarsen(value)
            | E::RefinementValidated(value)
            | E::InterfaceType(value)
            | E::RuntimeFailureMessage(value)
            | E::Run(value)
            | E::Join(value)
            | E::Cancel(value)
            | E::View(value)
            | E::Clone(value)
            | E::Comptime { value, .. }
            | E::InterfaceCoerce { value, .. }
            | E::FunctionAdapter { value, .. }
            | E::StateIs { value, .. }
            | E::Field { base: value, .. } => expression(value, output),
            E::Call { args, .. }
            | E::ResourceInvoke { args, .. }
            | E::Intrinsic { args, .. }
            | E::ActorSpawn { args, .. }
            | E::ActorGraphStep { args, .. } => {
                for value in args {
                    expression(value, output);
                }
            }
            E::IndirectCall { callee, args, .. } => {
                expression(callee, output);
                for value in args {
                    expression(value, output);
                }
            }
            E::ActorMessage { actor, args, .. } => {
                expression(actor, output);
                for value in args {
                    expression(value, output);
                }
            }
            E::StructConstruct { fields, .. } | E::BitfieldConstruct { fields, .. } => {
                for value in fields {
                    expression(value, output);
                }
            }
            E::MachineConstruct { payloads, .. } | E::EnumConstruct { payloads, .. } => {
                for value in payloads {
                    expression(value, output);
                }
            }
            E::MachineTransition {
                source, payloads, ..
            } => {
                expression(source, output);
                for value in payloads {
                    expression(value, output);
                }
            }
            E::ListConstruct { elements } => {
                for value in elements {
                    expression(value, output);
                }
            }
            E::MapConstruct { entries } => {
                for entry in entries {
                    expression(&entry.key, output);
                    expression(&entry.value, output);
                }
            }
            E::Handle {
                target, failure, ..
            } => {
                expression(target, output);
                walk(failure, output);
            }
            E::StringInterpolation(parts) => {
                for part in parts {
                    if let crate::StringSegment::Value(value) = part {
                        expression(value, output);
                    }
                }
            }
            E::ActorGraphImport(_)
            | E::InlineFunction { .. }
            | E::Int(_)
            | E::Float(_)
            | E::String(_)
            | E::Bool(_)
            | E::Nothing
            | E::Local(_)
            | E::Constant { .. }
            | E::FunctionRef(_)
            | E::ResourceHookValue { .. }
            | E::ClosureRef { .. }
            | E::OptionalNone
            | E::RuntimeFailure(_)
            | E::PropertyCaseContext(_) => {}
        }
    }
    fn walk<'a>(block: &'a crate::Block, output: &mut Vec<(LocalId, &'a Expression)>) {
        use crate::StatementKind as S;
        for statement in &block.statements {
            match &statement.kind {
                S::UpdateField {
                    target,
                    value,
                    place: _,
                } => {
                    expression(target, output);
                    expression(value, output);
                }
                S::Assign { target, value } => {
                    expression(target, output);
                    expression(value, output);
                }
                S::Let { local, value } => {
                    output.push((*local, value));
                    expression(value, output);
                }
                S::HandleDefault(value) | S::Expression(value) | S::Respond(value) => {
                    expression(value, output)
                }
                S::Return(value) => {
                    if let Some(value) = value {
                        expression(value, output);
                    }
                }
                S::If {
                    condition,
                    then_block,
                    else_block,
                } => {
                    expression(condition, output);
                    walk(then_block, output);
                    if let Some(block) = else_block {
                        walk(block, output);
                    }
                }
                S::While { condition, body } => {
                    expression(condition, output);
                    walk(body, output);
                }
                S::For { iterable, body, .. } => {
                    expression(iterable, output);
                    walk(body, output);
                }
                S::Match { scrutinee, arms } => {
                    expression(scrutinee, output);
                    for arm in arms {
                        walk(&arm.body, output);
                    }
                }
                S::Assert { condition, message } => {
                    expression(condition, output);
                    if let Some(value) = message {
                        expression(value, output);
                    }
                }
                S::Breakpoint { condition, .. } => {
                    if let Some(value) = condition {
                        expression(value, output);
                    }
                }
                S::Scope(body) => walk(body, output),
                S::ReflectedTypeDispatch { type_info, arms } => {
                    expression(type_info, output);
                    for arm in arms {
                        walk(&arm.body, output);
                    }
                }
                S::Break | S::Continue | S::Trace(_) => {}
            }
        }
    }
    let mut output = Vec::new();
    walk(block, &mut output);
    output
}
