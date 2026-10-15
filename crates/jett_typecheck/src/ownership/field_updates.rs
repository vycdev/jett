//! Actual checked storage and reservations for Source field replacements.
//! Local alias expiration retains the existing bounded backend policy; this
//! does not introduce general last-use alias support or runtime loan tokens.

use super::*;

impl OwnershipChecker<'_> {
    pub(super) fn field_loan_root(&self, expression: &Expr) -> Option<BindingKey> {
        match expression {
            Expr::View(inner, _)
            | Expr::Paren(inner, _)
            | Expr::Coarsen(inner, _)
            | Expr::Declassify(inner, _)
            | Expr::FieldAccess(inner, _, _) => self.field_loan_root(inner),
            Expr::Ident(root) => {
                let definition = self.resolve?.resolutions.get(&root.span)?;
                Some(BindingKey::Definition(*definition))
            }
            // Clones, calls and other owning results start independent storage.
            _ => None,
        }
    }

    pub(super) fn field_assignment_root(target: &Expr) -> Option<&ast::Ident> {
        match target {
            Expr::Ident(ident) => Some(ident),
            Expr::FieldAccess(inner, _, _) | Expr::Paren(inner, _) => {
                Self::field_assignment_root(inner)
            }
            _ => None,
        }
    }

    pub(super) fn binding_borrow_source(&self, name: &ast::Ident) -> Option<BindingKey> {
        match self.binding_fact(name)?.mode {
            CheckedBindingMode::View {
                source: CheckedViewSource::Binding(definition),
            } => Some(BindingKey::Definition(definition)),
            CheckedBindingMode::Owned
            | CheckedBindingMode::View {
                source: CheckedViewSource::Other,
            } => None,
        }
    }

    pub(super) fn reject_reserved_field_update_change(
        &mut self,
        key: &BindingKey,
        span: Span,
    ) -> bool {
        if !self.field_update_reservations.contains(key) {
            return false;
        }
        if self
            .field_update_reservation_errors
            .insert((key.clone(), span))
        {
            let name = self.binding_label(key).unwrap_or_else(|| "owner".into());
            self.diagnostics.push(Diagnostic::error(
                404,
                format!("cannot consume or rebind `{name}` while evaluating its field replacement"),
                span,
            ));
        }
        true
    }

    fn alias_depends_on(&self, alias: &BindingKey, root: &BindingKey) -> bool {
        let mut current = alias;
        for _ in 0..self.states.len() {
            let Some(source) = self
                .states
                .get(current)
                .and_then(|info| info.borrowed_from.as_ref())
            else {
                return false;
            };
            if source == root {
                return true;
            }
            current = source;
        }
        false
    }

    pub(super) fn has_dependent_field_alias(&self, root: &BindingKey) -> bool {
        self.states.iter().any(|(key, alias)| {
            alias.state == OwnershipState::Viewed && self.alias_depends_on(key, root)
        })
    }

    fn check_field_update_live_root(&mut self, place: &CheckedAssignmentPlace) -> bool {
        let root = BindingKey::Definition(place.root.definition);
        let Some(info) = self.states.get(&root) else {
            self.diagnostics.push(Diagnostic::error(
                404,
                "field replacement requires an initialized live owned binding",
                place.root_use_span,
            ));
            return false;
        };
        let name = self.binding_label(&root).unwrap_or_else(|| "owner".into());
        if info.state == OwnershipState::Consumed || info.debug_consumed {
            self.diagnostics.push(use_after_move(
                &name,
                place.root_use_span,
                info.consumed_span.unwrap_or(place.root.declaration_span),
            ));
            return false;
        }
        if info.state == OwnershipState::Viewed {
            self.diagnostics
                .push(cannot_assign_through_view(place.root_use_span));
            return false;
        }
        if !info.mutable
            || info.type_id != place.root.ty
            || info.state == OwnershipState::Uninitialized
        {
            self.diagnostics.push(Diagnostic::error(
                404,
                "field replacement requires the exact initialized mutable owned binding",
                place.root_use_span,
            ));
            return false;
        }
        // Pending is runtime inspection state. It does not manufacture payload
        // readiness here and is deliberately not a blanket Source rejection.
        if self.has_dependent_field_alias(&root) {
            self.diagnostics.push(Diagnostic::error(
                401,
                format!(
                    "cannot replace a field of `{name}` while a dependent local view is retained"
                ),
                place.root_use_span,
            ));
            return false;
        }
        true
    }

    pub(super) fn check_field_update_assignment(
        &mut self,
        assign: &ast::AssignStmt,
        place: &CheckedAssignmentPlace,
    ) {
        let diagnostics_before = self.diagnostics.len();
        let declaration_matches = self
            .body_facts
            .and_then(|facts| facts.binding_facts.get(&place.root.declaration_span))
            == Some(&place.root);
        let source_matches = self.resolve.is_some_and(|resolve| {
            resolve.resolutions.get(&place.root_use_span) == Some(&place.root.definition)
        }) && place.assignment_span == assign.span
            && declaration_matches;
        if !source_matches || crate::validate_assignment_place_shape(place, self.interner).is_err()
        {
            self.diagnostics.push(Diagnostic::error(
                404,
                "field replacement lacks its exact checked Source place",
                assign.span,
            ));
        }
        let root = BindingKey::Definition(place.root.definition);
        self.reject_reserved_field_update_change(&root, place.root_use_span);
        let root_was_live = self.check_field_update_live_root(place);
        self.field_update_reservations.push(root);
        // This is a transfer into a staged owning field, never an implicit
        // Source clone. Written field reads retain their existing copy rules.
        self.consume_expr(&assign.value, assign.value.span());
        self.field_update_reservations.pop();
        if !self.actor_reachable() {
            return;
        }
        if root_was_live {
            self.check_field_update_live_root(place);
        }
        if self.diagnostics.len() == diagnostics_before {
            self.validated_field_updates.insert(AssignmentSite {
                context: self.assignment_context.clone(),
                span: assign.span,
            });
        }
    }
}
