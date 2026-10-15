//! A field-write place is minted only from the exact checked Source body.
//! The sealed shape is independent of MIR live-owner/loan validation.

use crate::{BodyLowerer, Expression, ExpressionKind, Local, LocalId};
use jett_parser::ast::{AssignStmt, Expr};
use jett_typecheck::{
    CheckedAssignmentPlace, CheckedFieldUpdateResourceRequirement, validate_assignment_place_shape,
};
use jett_types::{Type, TypeInterner};

/// Constructor-only identity of one actually lowered checked Source occurrence.
/// A repeated span, outer root and path do not identify a reflected body arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FieldUpdateOccurrence(u64);

#[derive(Debug, Clone, PartialEq)]
pub struct FieldUpdatePlace {
    occurrence: FieldUpdateOccurrence,
    required_scopes: Vec<crate::RequiredScopeIdentity>,
    scoped_type_bindings: Vec<crate::ScopedTypeBinding>,
    checked: CheckedAssignmentPlace,
    original_span: jett_common::Span,
    target: Expression,
    root: LocalId,
}

impl FieldUpdatePlace {
    pub fn occurrence(&self) -> FieldUpdateOccurrence {
        self.occurrence
    }
    pub fn required_scopes(&self) -> &[crate::RequiredScopeIdentity] {
        &self.required_scopes
    }
    pub fn scoped_type_bindings(&self) -> &[crate::ScopedTypeBinding] {
        &self.scoped_type_bindings
    }
    pub fn root(&self) -> LocalId {
        self.root
    }
    pub fn target(&self) -> &Expression {
        &self.target
    }
    pub fn span(&self) -> jett_common::Span {
        self.checked.assignment_span
    }
    pub fn root_type(&self) -> jett_types::TypeId {
        self.checked.root_type
    }
    pub fn projections(&self) -> &[crate::FieldUpdateProjection] {
        &self.checked.projections
    }

    /// Canonical shape validation grants no live-owner or loan authority.
    pub fn validate(
        &self,
        target: &Expression,
        locals: &[Local],
        types: &TypeInterner,
    ) -> Result<(), String> {
        validate_assignment_place_shape(&self.checked, types)
            .map_err(|error| format!("invalid checked field update shape: {error:?}"))?;
        if self.checked.resource != CheckedFieldUpdateResourceRequirement::Ordinary {
            return Err("pending Resource projected assignment transport".into());
        }
        if self.original_span != self.span() || target != &self.target {
            return Err("field update changed its exact Source target".into());
        }
        let root = locals
            .get(self.root.index() as usize)
            .filter(|local| local.id == self.root)
            .ok_or("field update root is absent from its local table")?;
        let declared = self.checked.root.declaration_span;
        if !root.mutable
            || root.view_source.is_some()
            || root.ty != self.checked.root.ty
            || root.span.file != declared.file
            || root.span.start > declared.start
            || root.span.end < declared.end
        {
            return Err("field update root changed its checked binding".into());
        }
        let mut expression = target;
        for projection in self.checked.projections.iter().rev() {
            let ExpressionKind::Field {
                base,
                owner_type,
                field,
            } = &expression.kind
            else {
                return Err("field update path is not its exact canonical projection".into());
            };
            let owner = match types.resolve(projection.owner_type) {
                Type::Secret(inner) => *inner,
                _ => projection.owner_type,
            };
            if base.ty != projection.owner_type
                || *owner_type != owner
                || field.index() as usize != projection.field_index
                || expression.ty != projection.field_type
            {
                return Err("field update changed owner, field ordinal or qualification".into());
            }
            expression = base;
        }
        if !matches!(expression.kind, ExpressionKind::Local(root) if root == self.root)
            || expression.ty != self.root_type()
        {
            return Err("field update changed its checked root use".into());
        }
        Ok(())
    }
}

fn source_root(expression: &Expr) -> Option<&jett_parser::ast::Ident> {
    match expression {
        Expr::Ident(ident) => Some(ident),
        Expr::FieldAccess(object, _, _) => source_root(object),
        Expr::Paren(inner, _) => source_root(inner),
        _ => None,
    }
}

pub(super) fn authenticate(
    body: &mut BodyLowerer<'_, '_>,
    original: &AssignStmt,
    target: &Expression,
) -> Option<FieldUpdatePlace> {
    let checked = body.assignment_places.get(&original.span).cloned();
    let result = (|| {
        let checked = checked.ok_or("field update has no exact checked-body place")?;
        if !checked.ownership_validated() {
            return Err("field update lacks completed Source ownership validation");
        }
        let root =
            source_root(&original.target).ok_or("field update Source root is not a binding")?;
        if root.span != checked.root_use_span
            || body.parent.resolve.resolutions.get(&root.span) != Some(&checked.root.definition)
            || body.binding_facts.get(&checked.root.declaration_span) != Some(&checked.root)
        {
            return Err("field update Source binding association changed");
        }
        let local = body
            .local_ids
            .get(&checked.root.definition)
            .copied()
            .ok_or("field update binding has no owning HIR local")?;
        let occurrence = FieldUpdateOccurrence(body.next_field_update_occurrence);
        body.next_field_update_occurrence = body
            .next_field_update_occurrence
            .checked_add(1)
            .ok_or("field update occurrence inventory exhausted")?;
        let place = FieldUpdatePlace {
            occurrence,
            required_scopes: body.required_scopes.clone(),
            scoped_type_bindings: body.scoped_type_bindings.clone(),
            checked,
            original_span: original.span,
            target: target.clone(),
            root: local,
        };
        place
            .validate(target, &body.locals, &body.parent.check.interner)
            .map_err(|_| "field update failed canonical checked place validation")?;
        if body.view_parameter_locals.contains(&local) {
            return Err("field update cannot target a view parameter");
        }
        Some(place).ok_or("field update place is absent")
    })();
    match result {
        Ok(place) => Some(place),
        Err(message) => {
            body.parent.error(original.span, message);
            None
        }
    }
}

/// Exhaustive dependency walk of physically owned current function bodies.
/// Extracted closures and sealed graph factories retain their own identity.
pub fn field_update_places(block: &crate::Block) -> Vec<&FieldUpdatePlace> {
    fn expression<'a>(value: &'a Expression, output: &mut Vec<&'a FieldUpdatePlace>) {
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
    fn walk<'a>(block: &'a crate::Block, output: &mut Vec<&'a FieldUpdatePlace>) {
        use crate::StatementKind as S;
        for statement in &block.statements {
            match &statement.kind {
                S::UpdateField {
                    target,
                    value,
                    place,
                } => {
                    output.push(place);
                    expression(target, output);
                    expression(value, output);
                }
                S::Assign { target, value } => {
                    expression(target, output);
                    expression(value, output);
                }
                S::Let { value, .. }
                | S::HandleDefault(value)
                | S::Expression(value)
                | S::Respond(value) => expression(value, output),
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

#[cfg(test)]
mod tests {
    use super::*;
    use jett_common::{FileId, SourceOrigin};
    use std::collections::HashMap;

    const SOURCE: &str = "struct Record:\n    value: int64\n    sibling: int64\nfunction next_value(view record: Record) returns int64:\n    return record.value + 1\nfunction update(mutable record: Record) returns Record:\n    record.value = next_value(view record)\n    return record\n";
    fn checked(
        source: &str,
    ) -> (
        jett_parser::ast::Module,
        jett_resolve::ResolveResult,
        jett_typecheck::CheckResult,
    ) {
        let parsed = jett_parser::parse(source, FileId::new(0));
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let resolved = jett_resolve::resolve(&parsed.module);
        assert!(
            resolved
                .diagnostics
                .iter()
                .all(|d| d.severity != jett_diagnostics::Severity::Error),
            "{:?}",
            resolved.diagnostics
        );
        let checked = jett_typecheck::check(&parsed.module, &resolved);
        assert!(
            checked
                .diagnostics
                .iter()
                .all(|d| d.severity != jett_diagnostics::Severity::Error),
            "{:?}",
            checked.diagnostics
        );
        (parsed.module, resolved, checked)
    }
    #[test]
    fn source_field_place_retains_exact_same_type_sibling_ordinal() {
        let (module, resolved, mut checked) = checked(SOURCE);
        let origins = HashMap::from([(FileId::new(0), SourceOrigin::Project)]);
        let high =
            crate::lower(&module, &resolved, &checked, &origins).expect("checked field update HIR");
        let update = high
            .functions
            .iter()
            .find(|f| f.identity.declaration.name == "update")
            .unwrap();
        let places = field_update_places(&update.body);
        assert_eq!(places.len(), 1);
        assert_eq!(places[0].projections()[0].field_index, 0);
        let fact = checked.assignment_places.values_mut().next().unwrap();
        fact.projections[0].field_index = 1;
        assert!(!fact.ownership_validated());
        assert!(crate::lower(&module, &resolved, &checked, &origins).is_err());
    }
    #[test]
    fn source_field_place_walk_includes_nested_handler_bodies() {
        let source = "struct Record:\n    value: int64\n    sibling: int64\nfunction update(mutable record: Record, incoming: optional[int64]) returns Record:\n    int64 selected = incoming handle:\n        mutable Record independent = Record(value: 0, sibling: 0)\n        independent.value = 7\n        default independent.value\n    record.sibling = selected\n    return record\n";
        let (module, resolved, checked) = checked(source);
        let high = crate::lower(
            &module,
            &resolved,
            &checked,
            &HashMap::from([(FileId::new(0), SourceOrigin::Project)]),
        )
        .expect("nested handler field update HIR");
        let update = high
            .functions
            .iter()
            .find(|f| f.identity.declaration.name == "update")
            .unwrap();
        assert_eq!(field_update_places(&update.body).len(), 2);
    }
}

pub(super) fn checked_function_has_updates(
    check: &jett_typecheck::CheckResult,
    function: jett_common::Span,
) -> bool {
    fn inside(child: jett_common::Span, owner: jett_common::Span) -> bool {
        child.file == owner.file && child.start >= owner.start && child.end <= owner.end
    }
    fn body_has_updates(body: &jett_typecheck::CheckedBodyFacts) -> bool {
        body.assignment_places
            .values()
            .any(|place| place.ownership_validated())
            || body
                .comptime_type_bindings
                .values()
                .flatten()
                .any(|row| body_has_updates(&row.body))
    }
    check
        .assignment_places
        .iter()
        .any(|(span, place)| inside(*span, function) && place.ownership_validated())
        || check.comptime_type_bindings.iter().any(|(span, rows)| {
            inside(*span, function) && rows.iter().any(|row| body_has_updates(&row.body))
        })
}

pub(super) fn requires_original_reflected_archive(program: &crate::Program) -> bool {
    program.functions.iter().any(|function| {
        function.identity.declaration.kind == crate::DeclarationKind::Function
            && function.source_definition.is_some()
            && function.capture_count == 0
            && function.identity.type_arguments.is_empty()
            && function.identity.scoped_type_bindings.is_empty()
            && function.identity.specialization
                == jett_typecheck::CheckedGenericSpecialization::default()
            && field_update_places(&function.body).iter().any(|place| {
                place.required_scopes().iter().any(|scope| {
                    matches!(
                        scope.selection,
                        jett_typecheck::CheckedComptimeTypeSelection::ReflectedIteration(_)
                    )
                })
            })
    })
}
