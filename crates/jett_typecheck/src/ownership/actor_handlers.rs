//! Accepting actor flow. Exact checked captures remain obligations at normal
//! handler exits; observation-only breakpoint traversal never grants a receipt.
use super::*;
use crate::{
    CheckedActorBindingRole, CheckedActorHandlerBinding, CheckedActorHandlerExit,
    CheckedActorHandlerExitKind,
};

#[derive(Debug)]
pub(crate) struct ActorHandlerReport {
    pub exits: Vec<CheckedActorHandlerExit>,
    pub validated: bool,
}

#[derive(Default)]
struct ActorLoopFlow {
    breaks: Vec<HashMap<BindingKey, VarInfo>>,
    continues: Vec<HashMap<BindingKey, VarInfo>>,
}

pub(super) struct ActorFlow {
    reachable: bool,
    response_type: TypeId,
    responding: bool,
    persistent: Vec<CheckedActorHandlerBinding>,
    exits: Vec<CheckedActorHandlerExit>,
    supported: bool,
    loops: Vec<ActorLoopFlow>,
}

impl<'a> OwnershipChecker<'a> {
    pub(super) fn reject_actor_loan_change(&mut self, key: &BindingKey, span: Span) -> bool {
        if self.actor_flow.is_none()
            || !self.has_dependent_field_alias(key)
            || self
                .states
                .get(key)
                .is_none_or(|info| self.is_copyable(info.type_id))
        {
            return false;
        }
        self.diagnostics.push(Diagnostic::error(
            401,
            "cannot move or replace an actor owner while a dependent view is retained",
            span,
        ));
        true
    }

    pub(super) fn actor_reachable(&self) -> bool {
        self.actor_flow.as_ref().is_none_or(|flow| flow.reachable)
    }

    pub(super) fn set_actor_reachable(&mut self, reachable: bool) {
        if let Some(flow) = &mut self.actor_flow {
            flow.reachable = reachable;
        }
    }

    fn actor_header_matches(
        &self,
        actor: &ast::ActorDef,
        handler_index: usize,
        checked: &CheckedActorHandler,
    ) -> bool {
        let target = checked.target();
        let Some(resolve) = self.resolve else {
            return false;
        };
        let Some(handler) = actor.handlers.get(handler_index) else {
            return false;
        };
        let canonical = self.interner.resolve_actor(target.actor);
        if resolve.scope_table.def(target.actor_definition).span != actor.name.span
            || target.handler_index != handler_index
            || checked.body_span() != handler.body.span
            || canonical.capability_params.len() != actor.capability_params.len()
            || canonical.state_fields.len() != actor.state_fields.len()
            || canonical.messages.len() != actor.handlers.len()
            || checked.response_type() != canonical.messages[handler_index].responds
            || checked.bindings().len()
                != actor.capability_params.len() + actor.state_fields.len() + handler.params.len()
        {
            return false;
        }
        let mut expected = Vec::new();
        for (parameter, (param, (_, ty))) in actor
            .capability_params
            .iter()
            .zip(&canonical.capability_params)
            .enumerate()
        {
            expected.push((
                CheckedActorBindingRole::Capability { parameter },
                &param.name,
                *ty,
            ));
        }
        for (field, (decl, (_, ty))) in actor
            .state_fields
            .iter()
            .zip(&canonical.state_fields)
            .enumerate()
        {
            expected.push((CheckedActorBindingRole::State { field }, &decl.name, *ty));
        }
        for (parameter, (param, (_, ty))) in handler
            .params
            .iter()
            .zip(&canonical.messages[handler_index].params)
            .enumerate()
        {
            expected.push((
                CheckedActorBindingRole::Message { parameter },
                &param.name,
                *ty,
            ));
        }
        checked
            .bindings()
            .iter()
            .zip(expected)
            .all(|(binding, (role, name, ty))| {
                binding.role == role
                    && binding.fact.ty == ty
                    && binding.fact.ty != TypeInterner::ERROR
                    && binding.fact.declaration_span == name.span
                    && self.binding_key(&name.name, name.span)
                        == BindingKey::Definition(binding.fact.definition)
                    && checked.body().binding_facts.get(&name.span) == Some(&binding.fact)
            })
    }

    pub(super) fn check_actor_handlers(&mut self, actor: &ast::ActorDef) {
        let Some(handlers) = self.actor_handlers else {
            return;
        };
        let saved_states = std::mem::take(&mut self.states);
        let saved_context = self.assignment_context.clone();
        let saved_facts = self.body_facts;
        // Check actual initializer acquisition in resolver order, preserving a
        // persistent view's immediate source instead of inventing owned seeds.
        for param in &actor.capability_params {
            self.register_checked_binding(&param.name);
        }
        for field in &actor.state_fields {
            self.check_var_decl(field);
        }
        let initial_state = self.states.clone();
        for (handler_index, handler) in actor.handlers.iter().enumerate() {
            let Some(checked) = handlers.values().find(|checked| {
                checked.target().handler_index == handler_index
                    && checked.body_span() == handler.body.span
                    && self.resolve.is_some_and(|resolve| {
                        resolve
                            .scope_table
                            .def(checked.target().actor_definition)
                            .span
                            == actor.name.span
                    })
            }) else {
                continue;
            };
            if !self.actor_header_matches(actor, handler_index, checked) {
                continue;
            }
            let target = checked.target();
            self.body_facts = Some(OwnershipBodyFacts::scoped(checked.body()));
            self.assignment_context = AssignmentContext {
                root: AssignmentContextRoot::ActorHandler(target),
                scopes: Vec::new(),
            };
            let persistent = checked
                .bindings()
                .iter()
                .copied()
                .filter(|binding| !matches!(binding.role, CheckedActorBindingRole::Message { .. }))
                .collect::<Vec<_>>();
            self.states = initial_state
                .iter()
                .filter(|(key, _)| {
                    persistent
                        .iter()
                        .any(|binding| **key == BindingKey::Definition(binding.fact.definition))
                })
                .map(|(key, info)| (key.clone(), info.clone()))
                .collect();
            for param in &handler.params {
                self.register_checked_binding(&param.name);
            }
            let diagnostics_before = self.diagnostics.len();
            self.actor_flow = Some(ActorFlow {
                reachable: true,
                response_type: checked.response_type(),
                responding: handler.responds.is_some(),
                persistent,
                exits: Vec::new(),
                supported: true,
                loops: Vec::new(),
            });
            // The initial state must itself be available. This also verifies
            // persistent aliases against exact previously installed captures.
            self.validate_actor_captures(handler.body.span);
            self.check_block(&handler.body);
            if self.actor_reachable() {
                self.record_actor_exit(handler.body.span, CheckedActorHandlerExitKind::Fallthrough);
                if handler.responds.is_some() || checked.response_type() != TypeInterner::NOTHING {
                    self.actor_flow.as_mut().unwrap().supported = false;
                }
            }
            let mut flow = self.actor_flow.take().unwrap();
            flow.exits.sort_by_key(|exit| {
                (
                    exit.span.file.index(),
                    exit.span.start,
                    exit.span.end,
                    match exit.kind {
                        CheckedActorHandlerExitKind::Fallthrough => 0,
                        CheckedActorHandlerExitKind::Respond => 1,
                        CheckedActorHandlerExitKind::Return => 2,
                    },
                )
            });
            flow.exits.dedup();
            self.actor_reports.insert(
                target,
                ActorHandlerReport {
                    exits: flow.exits,
                    validated: flow.supported && self.diagnostics.len() == diagnostics_before,
                },
            );
        }
        self.states = saved_states;
        self.assignment_context = saved_context;
        self.body_facts = saved_facts;
    }

    fn validate_actor_captures(&mut self, span: Span) {
        let persistent = self.actor_flow.as_ref().unwrap().persistent.clone();
        for binding in persistent {
            let key = BindingKey::Definition(binding.fact.definition);
            let expected_source = match binding.fact.mode {
                CheckedBindingMode::View {
                    source: CheckedViewSource::Binding(source),
                } => Some(BindingKey::Definition(source)),
                _ => None,
            };
            let valid = self.states.get(&key).is_some_and(|info| {
                info.type_id == binding.fact.ty
                    && info.mutable == binding.fact.mutable
                    && !info.debug_consumed
                    && match binding.fact.mode {
                        CheckedBindingMode::Owned => {
                            matches!(info.state, OwnershipState::Owned | OwnershipState::Pending)
                                && info.borrowed_from.is_none()
                        }
                        CheckedBindingMode::View { .. } => {
                            info.state == OwnershipState::Viewed
                                && info.borrowed_from == expected_source
                        }
                    }
            });
            if !valid {
                let name = self.binding_label(&key).unwrap_or_else(|| "capture".into());
                self.diagnostics.push(
                    Diagnostic::error(
                        400,
                        format!(
                            "actor capture `{name}` must retain its owned state at handler exit"
                        ),
                        span,
                    )
                    .with_label(
                        binding.fact.declaration_span,
                        "persistent actor capture declared here",
                    ),
                );
            }
            if matches!(binding.role, CheckedActorBindingRole::State { .. })
                && matches!(binding.fact.mode, CheckedBindingMode::View { .. })
                && expected_source.is_none()
            {
                // Existing temporary-view validity is not redefined here.
                // Without an exact persistent dependency there is no receipt.
                self.actor_flow.as_mut().unwrap().supported = false;
            }
            if let Some(source) = expected_source {
                if !self.states.get(&source).is_some_and(|info| {
                    !info.debug_consumed
                        && matches!(
                            info.state,
                            OwnershipState::Owned
                                | OwnershipState::Pending
                                | OwnershipState::Viewed
                        )
                }) {
                    self.diagnostics.push(Diagnostic::error(
                        401,
                        "persistent actor view lost its exact backing capture",
                        span,
                    ));
                }
            }
        }
    }

    fn record_actor_exit(&mut self, span: Span, kind: CheckedActorHandlerExitKind) {
        self.validate_actor_captures(span);
        self.actor_flow
            .as_mut()
            .unwrap()
            .exits
            .push(CheckedActorHandlerExit { span, kind });
    }

    pub(super) fn check_actor_respond(&mut self, response: &ast::RespondStmt) {
        // This terminal path abandons any enclosing staged field replacement.
        let reservations = std::mem::take(&mut self.field_update_reservations);
        let result_type = self
            .body_facts
            .and_then(|facts| facts.type_map.get(&response.value.span()))
            .copied();
        if result_type.is_some_and(|ty| self.is_copyable(ty)) {
            self.check_expr_ownership(&response.value);
        } else {
            self.consume_expr(&response.value, response.value.span());
        }
        self.field_update_reservations = reservations;
        if !self.actor_reachable() {
            return;
        }
        self.record_actor_exit(response.span, CheckedActorHandlerExitKind::Respond);
        self.set_actor_reachable(false);
    }

    fn actor_return_has_explicit_view(value: &Expr) -> bool {
        match value {
            Expr::Paren(inner, _) | Expr::Coarsen(inner, _) | Expr::Declassify(inner, _) => {
                Self::actor_return_has_explicit_view(inner)
            }
            Expr::View(_, _) => true,
            _ => false,
        }
    }

    pub(super) fn check_actor_return(&mut self, ret: &ast::ReturnStmt) {
        let flow = self.actor_flow.as_ref().unwrap();
        let void_exit = !flow.responding
            && flow.response_type == TypeInterner::NOTHING
            && ret.value.as_ref().is_none_or(|value| {
                !Self::actor_return_has_explicit_view(value)
                    && self
                        .body_facts
                        .and_then(|facts| facts.type_map.get(&value.span()))
                        == Some(&TypeInterner::NOTHING)
            });
        // Return exits the handler, so its real operand is evaluated without
        // committing or reserving any interrupted outer field replacement.
        let reservations = std::mem::take(&mut self.field_update_reservations);
        if let Some(value) = &ret.value {
            // An admitted nothing result is Copy. Its expression still applies
            // every selected call acquisition before persistent exit checks.
            self.check_expr_ownership(value);
        }
        self.field_update_reservations = reservations;
        if !self.actor_reachable() {
            return;
        }
        self.record_actor_exit(ret.span, CheckedActorHandlerExitKind::Return);
        self.actor_flow.as_mut().unwrap().supported &= void_exit;
        self.set_actor_reachable(false);
    }

    pub(super) fn check_actor_loop_jump(&mut self, _span: Span, continuing: bool) {
        let state = self.states.clone();
        let Some(flow) = &mut self.actor_flow else {
            return;
        };
        let Some(loop_flow) = flow.loops.last_mut() else {
            flow.supported = false;
            return;
        };
        if continuing {
            loop_flow.continues.push(state);
        } else {
            loop_flow.breaks.push(state);
        }
        flow.reachable = false;
    }

    fn actor_branch(
        &mut self,
        state: &HashMap<BindingKey, VarInfo>,
        body: &Block,
    ) -> Option<HashMap<BindingKey, VarInfo>> {
        self.states = state.clone();
        self.set_actor_reachable(true);
        self.check_block(body);
        self.actor_reachable().then(|| self.states.clone())
    }

    fn join_actor_branches(
        &mut self,
        baseline: &HashMap<BindingKey, VarInfo>,
        outcomes: &[HashMap<BindingKey, VarInfo>],
    ) {
        self.states = self.merge_fallthrough_states(baseline, outcomes);
        self.set_actor_reachable(!outcomes.is_empty());
    }

    pub(super) fn check_actor_if(&mut self, statement: &ast::IfStmt) {
        self.check_expr_ownership(&statement.condition);
        if !self.actor_reachable() {
            return;
        }
        let selection = self
            .body_facts
            .and_then(|facts| facts.static_selections)
            .and_then(|choices| choices.get(&statement.span))
            .copied();
        if let Some(selection) = selection {
            match selection {
                CheckedStaticSelection::IfThen => self.check_block(&statement.then_block),
                CheckedStaticSelection::IfElseIf(index) => {
                    if let Some((condition, body)) = statement.else_ifs.get(index) {
                        self.check_expr_ownership(condition);
                        if self.actor_reachable() {
                            self.check_block(body);
                        }
                    }
                }
                CheckedStaticSelection::IfElse => {
                    if let Some(body) = &statement.else_block {
                        self.check_block(body);
                    }
                }
                CheckedStaticSelection::IfNoBranch => {}
                CheckedStaticSelection::MatchArm(_) => {
                    self.actor_flow.as_mut().unwrap().supported = false
                }
            }
            return;
        }
        let mut prefix = self.states.clone();
        let mut outcomes = Vec::new();
        if let Some(state) = self.actor_branch(&prefix, &statement.then_block) {
            outcomes.push(state);
        }
        let mut prefix_reachable = true;
        for (condition, body) in &statement.else_ifs {
            self.states = prefix.clone();
            self.set_actor_reachable(true);
            self.check_expr_ownership(condition);
            if !self.actor_reachable() {
                prefix_reachable = false;
                break;
            }
            prefix = self.states.clone();
            if let Some(state) = self.actor_branch(&prefix, body) {
                outcomes.push(state);
            }
        }
        if prefix_reachable {
            if let Some(body) = &statement.else_block {
                if let Some(state) = self.actor_branch(&prefix, body) {
                    outcomes.push(state);
                }
            } else {
                outcomes.push(prefix.clone());
            }
        }
        self.join_actor_branches(&prefix, &outcomes);
    }

    pub(super) fn check_actor_match(&mut self, statement: &ast::MatchStmt) {
        self.check_expr_ownership(&statement.expr);
        if !self.actor_reachable() {
            return;
        }
        self.record_debug_move(&statement.expr);
        let baseline = self.states.clone();
        let source = self.field_loan_root(&statement.expr);
        let selected = self
            .body_facts
            .and_then(|facts| facts.static_selections)
            .and_then(|choices| choices.get(&statement.span))
            .and_then(|choice| {
                if let CheckedStaticSelection::MatchArm(index) = choice {
                    Some(*index)
                } else {
                    None
                }
            });
        let mut outcomes = Vec::new();
        for (index, arm) in statement.arms.iter().enumerate() {
            if selected.is_some_and(|selected| selected != index) {
                continue;
            }
            self.states = baseline.clone();
            self.set_actor_reachable(true);
            if let ast::Pattern::Variant(_, bindings) = &arm.pattern {
                for name in bindings {
                    self.register_checked_binding(name);
                    let key = self.binding_key(&name.name, name.span);
                    if let Some(info) = self.states.get_mut(&key)
                        && info.state == OwnershipState::Viewed
                        && info.borrowed_from.is_none()
                    {
                        info.borrowed_from = source.clone();
                    }
                }
            }
            self.check_block(&arm.body);
            if self.actor_reachable() {
                outcomes.push(self.states.clone());
            }
        }
        self.join_actor_branches(&baseline, &outcomes);
    }

    pub(super) fn check_actor_scoped_type_body(&mut self, statement: &ast::ComptimeTypeBindStmt) {
        let Some(bodies) = self
            .body_facts
            .and_then(|facts| facts.comptime_type_bindings.get(&statement.span))
        else {
            self.actor_flow.as_mut().unwrap().supported = false;
            return;
        };
        let baseline = self.states.clone();
        let mut outcomes = Vec::new();
        for (index, body) in bodies.iter().enumerate() {
            let saved_context = self.assignment_context.clone();
            self.assignment_context = saved_context.scoped(statement.span, index);
            let saved_facts = self
                .body_facts
                .replace(OwnershipBodyFacts::scoped(&body.body));
            if let Some(state) = self.actor_branch(&baseline, &statement.body) {
                outcomes.push(state);
            }
            self.body_facts = saved_facts;
            self.assignment_context = saved_context;
        }
        self.join_actor_branches(&baseline, &outcomes);
    }

    pub(super) fn check_actor_handle(
        &mut self,
        target: &Expr,
        error: Option<&ast::Ident>,
        body: &Block,
    ) {
        self.check_expr_ownership(target);
        if !self.actor_reachable() {
            return;
        }
        self.record_debug_move(target);
        self.check_actor_handle_body(error, body);
    }

    pub(super) fn check_actor_handle_body(&mut self, error: Option<&ast::Ident>, body: &Block) {
        if !self.actor_reachable() {
            return;
        }
        let success = self.states.clone();
        if let Some(name) = error {
            self.register_checked_binding(name);
        }
        self.check_block(body);
        let mut outcomes = vec![success.clone()];
        if self.actor_reachable() {
            outcomes.push(self.states.clone());
        }
        self.join_actor_branches(&success, &outcomes);
    }

    pub(super) fn check_actor_for(&mut self, statement: &ast::ForStmt) {
        if statement.view {
            self.check_expr_ownership(&statement.iterable);
        } else {
            self.consume_expr(&statement.iterable, statement.iterable.span());
        }
        if !self.actor_reachable() {
            return;
        }
        let entry = self.states.clone();
        let mut header = entry.clone();
        let mut exits = vec![entry.clone()]; // the iterable may be empty
        // The join only loses availability or a pending guarantee. At most one
        // loss per entry binding is needed to reach the finite fixed point.
        for _ in 0..entry.len().saturating_mul(2).saturating_add(3) {
            self.states = header.clone();
            self.set_actor_reachable(true);
            self.register_checked_binding(&statement.variable);
            let source = if statement.view {
                self.field_loan_root(&statement.iterable)
            } else {
                self.binding_borrow_source(&statement.variable)
            };
            let key = self.binding_key(&statement.variable.name, statement.variable.span);
            if let Some(info) = self.states.get_mut(&key) {
                info.borrowed_from = source;
            } else {
                self.actor_flow.as_mut().unwrap().supported = false;
            }
            if let Some(name) = &statement.value_variable {
                self.register_checked_binding(name);
            }
            self.actor_flow
                .as_mut()
                .unwrap()
                .loops
                .push(ActorLoopFlow::default());
            self.check_block(&statement.body);
            let loop_flow = self.actor_flow.as_mut().unwrap().loops.pop().unwrap();
            exits.extend(loop_flow.breaks);
            let mut backedges = vec![entry.clone()];
            backedges.extend(loop_flow.continues);
            if self.actor_reachable() {
                backedges.push(self.states.clone());
            }
            let next = self.merge_fallthrough_states(&entry, &backedges);
            exits.push(next.clone());
            if next == header {
                self.join_actor_branches(&entry, &exits);
                return;
            }
            header = next;
        }
        self.actor_flow.as_mut().unwrap().supported = false;
        self.join_actor_branches(&entry, &exits);
    }

    pub(super) fn check_actor_while(&mut self, statement: &ast::WhileStmt) {
        let entry = self.states.clone();
        let mut header = entry.clone();
        let mut exits = Vec::new();
        for _ in 0..entry.len().saturating_mul(2).saturating_add(3) {
            self.states = header.clone();
            self.set_actor_reachable(true);
            self.check_expr_ownership(&statement.condition);
            if !self.actor_reachable() {
                self.join_actor_branches(&entry, &exits);
                return;
            }
            exits.push(self.states.clone()); // condition may be false
            self.actor_flow
                .as_mut()
                .unwrap()
                .loops
                .push(ActorLoopFlow::default());
            self.check_block(&statement.body);
            let loop_flow = self.actor_flow.as_mut().unwrap().loops.pop().unwrap();
            exits.extend(loop_flow.breaks);
            let mut backedges = vec![entry.clone()];
            backedges.extend(loop_flow.continues);
            if self.actor_reachable() {
                backedges.push(self.states.clone());
            }
            let next = self.merge_fallthrough_states(&entry, &backedges);
            if next == header {
                self.join_actor_branches(&entry, &exits);
                return;
            }
            header = next;
        }
        self.actor_flow.as_mut().unwrap().supported = false;
        self.join_actor_branches(&entry, &exits);
    }

    pub(super) fn check_actor_message_ownership(&mut self, inner: &Expr, span: Span) {
        let Some(packet) = self
            .body_facts
            .and_then(|facts| facts.actor_message_calls.get(&span))
        else {
            // Failed checking has no actor transfer packet.
            self.check_expr_ownership(inner);
            if let Some(flow) = &mut self.actor_flow {
                flow.supported = false;
            }
            return;
        };
        let (receiver, args) = match inner {
            Expr::Call(callee, args, _) => match callee.as_ref() {
                Expr::FieldAccess(receiver, _, _) => (receiver.as_ref(), args.as_slice()),
                _ => return,
            },
            Expr::FieldAccess(receiver, _, _) => (receiver.as_ref(), &[][..]),
            _ => return,
        };
        self.check_expr_ownership(receiver);
        for argument in &packet.arguments {
            if let Some(source) = args.get(argument.source_index) {
                self.check_argument_effect(&source.value, source.span, argument.effect);
            }
        }
    }
}
