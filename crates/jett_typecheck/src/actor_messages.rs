//! Checked actor messages and persistent handler roles from one compiler session.
//!
//! These facts do not grant Resource custody or live instance authority. A
//! consumer must join them to the retained original actor and handler body.
use std::collections::HashMap;
use std::sync::Arc;

use jett_common::Span;
use jett_parser::ast;
use jett_resolve::scope::DefId;
use jett_types::{ActorId, TypeId};

use crate::{
    CheckedArgumentOwnership, CheckedBindingFact, CheckedBodyFacts, CheckedComptimeTypeBinding,
    CheckedOwnershipContext,
};

#[derive(Debug, Clone)]
pub(crate) struct ActorCheckSession(Arc<()>);

impl ActorCheckSession {
    pub(crate) fn new() -> Self {
        Self(Arc::new(()))
    }
}

impl PartialEq for ActorCheckSession {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for ActorCheckSession {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CheckedActorHandlerTarget {
    pub actor_definition: DefId,
    pub actor: ActorId,
    pub handler_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedActorBindingRole {
    Capability { parameter: usize },
    State { field: usize },
    Message { parameter: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedActorHandlerBinding {
    pub role: CheckedActorBindingRole,
    pub fact: CheckedBindingFact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedActorMessageKind {
    Send,
    Ask,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedActorMessageCall {
    pub target: CheckedActorHandlerTarget,
    pub kind: CheckedActorMessageKind,
    pub context: CheckedOwnershipContext,
    pub source_span: Span,
    pub receiver_span: Span,
    pub receiver_type: TypeId,
    pub response_type: TypeId,
    /// Original argument occurrences in lexical source order.
    pub arguments: Vec<CheckedArgumentOwnership>,
    origin: ActorMessageOrigin,
}

impl CheckedActorMessageCall {
    pub(crate) fn new(
        session: ActorCheckSession,
        target: CheckedActorHandlerTarget,
        kind: CheckedActorMessageKind,
        context: CheckedOwnershipContext,
        source_span: Span,
        receiver_span: Span,
        receiver_type: TypeId,
        response_type: TypeId,
        arguments: Vec<CheckedArgumentOwnership>,
    ) -> Self {
        let origin = ActorMessageOrigin {
            session,
            occurrence: Arc::new(()),
            target,
            kind,
            context,
            source_span,
            receiver_span,
            receiver_type,
            response_type,
            arguments: arguments.clone(),
        };
        Self {
            target,
            kind,
            context,
            source_span,
            receiver_span,
            receiver_type,
            response_type,
            arguments,
            origin,
        }
    }
}

#[derive(Debug, Clone)]
struct ActorMessageOrigin {
    session: ActorCheckSession,
    occurrence: Arc<()>,
    target: CheckedActorHandlerTarget,
    kind: CheckedActorMessageKind,
    context: CheckedOwnershipContext,
    source_span: Span,
    receiver_span: Span,
    receiver_type: TypeId,
    response_type: TypeId,
    arguments: Vec<CheckedArgumentOwnership>,
}

impl ActorMessageOrigin {
    fn matches(&self, call: &CheckedActorMessageCall) -> bool {
        self.target == call.target
            && self.kind == call.kind
            && self.context == call.context
            && self.source_span == call.source_span
            && self.receiver_span == call.receiver_span
            && self.receiver_type == call.receiver_type
            && self.response_type == call.response_type
            && self.arguments == call.arguments
    }
}

impl PartialEq for ActorMessageOrigin {
    fn eq(&self, other: &Self) -> bool {
        self.session == other.session
            && Arc::ptr_eq(&self.occurrence, &other.occurrence)
            && self.target == other.target
            && self.kind == other.kind
            && self.context == other.context
            && self.source_span == other.source_span
            && self.receiver_span == other.receiver_span
            && self.receiver_type == other.receiver_type
            && self.response_type == other.response_type
            && self.arguments == other.arguments
    }
}

impl Eq for ActorMessageOrigin {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CheckedActorMessageRoot {
    Ordinary,
    Generic(usize),
    ActorHandler(CheckedActorHandlerTarget),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ActorMessageSite {
    root: CheckedActorMessageRoot,
    scopes: Vec<(Span, usize)>,
    source_span: Span,
}

#[derive(Debug, Default)]
pub(crate) struct ActorMessageSites(HashMap<ActorMessageSite, ActorMessageOrigin>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedActorHandlerExitKind {
    Fallthrough,
    Respond,
    Return,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedActorHandlerExit {
    pub span: Span,
    pub kind: CheckedActorHandlerExitKind,
}

/// An exact original handler header and its selected body facts.
///
/// The private body cannot be replaced or edited through public observations.
/// Its ownership receipt is granted only by accepting ownership traversal and
/// successful checking of the complete program, never by breakpoint observation.
#[derive(Debug, Clone)]
pub struct CheckedActorHandler {
    target: CheckedActorHandlerTarget,
    body_span: Span,
    bindings: Vec<CheckedActorHandlerBinding>,
    response_type: TypeId,
    body: CheckedBodyFacts,
    exits: Vec<CheckedActorHandlerExit>,
    session: ActorCheckSession,
    source_actor: String,
    ownership_seal: Option<ActorHandlerOwnershipSeal>,
}

impl CheckedActorHandler {
    pub(crate) fn new(
        target: CheckedActorHandlerTarget,
        body_span: Span,
        bindings: Vec<CheckedActorHandlerBinding>,
        response_type: TypeId,
        body: CheckedBodyFacts,
        session: ActorCheckSession,
        source_actor: &ast::ActorDef,
    ) -> Self {
        Self {
            target,
            body_span,
            bindings,
            response_type,
            body,
            exits: Vec::new(),
            session,
            source_actor: format!("{source_actor:?}"),
            ownership_seal: None,
        }
    }

    pub fn target(&self) -> CheckedActorHandlerTarget {
        self.target
    }

    pub fn matches_source_actor(&self, actor: &ast::ActorDef) -> bool {
        self.source_actor == format!("{actor:?}")
    }

    pub fn body_span(&self) -> Span {
        self.body_span
    }

    pub fn bindings(&self) -> &[CheckedActorHandlerBinding] {
        &self.bindings
    }

    pub fn response_type(&self) -> TypeId {
        self.response_type
    }

    pub fn body(&self) -> &CheckedBodyFacts {
        &self.body
    }

    pub fn exits(&self) -> &[CheckedActorHandlerExit] {
        &self.exits
    }

    pub fn ownership_validated(&self) -> bool {
        self.ownership_seal
            .as_ref()
            .is_some_and(|seal| seal.matches(self))
    }

    pub(crate) fn body_mut(&mut self) -> &mut CheckedBodyFacts {
        self.ownership_seal = None;
        &mut self.body
    }

    pub(crate) fn record_exits(&mut self, exits: Vec<CheckedActorHandlerExit>) {
        self.ownership_seal = None;
        self.exits = exits;
    }

    pub(crate) fn authorize_ownership(&mut self) {
        self.ownership_seal = Some(ActorHandlerOwnershipSeal {
            target: self.target,
            body_span: self.body_span,
            bindings: self.bindings.clone(),
            response_type: self.response_type,
            exits: self.exits.clone(),
        });
    }
}

#[derive(Debug, Clone)]
struct ActorHandlerOwnershipSeal {
    target: CheckedActorHandlerTarget,
    body_span: Span,
    bindings: Vec<CheckedActorHandlerBinding>,
    response_type: TypeId,
    exits: Vec<CheckedActorHandlerExit>,
}

impl ActorHandlerOwnershipSeal {
    fn matches(&self, handler: &CheckedActorHandler) -> bool {
        self.target == handler.target
            && self.body_span == handler.body_span
            && self.bindings == handler.bindings
            && self.response_type == handler.response_type
            && self.exits == handler.exits
    }
}

impl crate::CheckResult {
    pub fn actor_handler(&self, target: CheckedActorHandlerTarget) -> Option<&CheckedActorHandler> {
        self.actor_handlers.get(&target).filter(|handler| {
            handler.target == target
                && handler.session == self.actor_check_session
                && handler.ownership_validated()
        })
    }

    /// Snapshot/session sanity only; executable consumers also select the exact
    /// original occurrence and call `validates_actor_message_at` for its body.
    pub fn validates_actor_message(&self, call: &CheckedActorMessageCall) -> bool {
        self.diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != jett_diagnostics::Severity::Error)
            && call.origin.session == self.actor_check_session
            && call.origin.matches(call)
    }

    pub fn validates_actor_message_at(
        &self,
        call: &CheckedActorMessageCall,
        root: CheckedActorMessageRoot,
        scopes: &[(Span, usize)],
        source_span: Span,
    ) -> bool {
        source_span == call.source_span
            && self.validates_actor_message(call)
            && self.actor_message_sites.0.get(&ActorMessageSite {
                root,
                scopes: scopes.to_vec(),
                source_span,
            }) == Some(&call.origin)
    }

    pub(crate) fn freeze_actor_message_sites(&mut self) {
        let mut sites = ActorMessageSites::default();
        capture_message_sites(
            self,
            CheckedActorMessageRoot::Ordinary,
            &[],
            &self.actor_message_calls,
            &self.comptime_type_bindings,
            &mut sites,
        );
        for (index, body) in self.generic_function_instantiations.iter().enumerate() {
            capture_message_sites(
                self,
                CheckedActorMessageRoot::Generic(index),
                &[],
                &body.actor_message_calls,
                &body.comptime_type_bindings,
                &mut sites,
            );
        }
        for (target, handler) in &self.actor_handlers {
            capture_message_sites(
                self,
                CheckedActorMessageRoot::ActorHandler(*target),
                &[],
                &handler.body.actor_message_calls,
                &handler.body.comptime_type_bindings,
                &mut sites,
            );
        }
        self.actor_message_sites = sites;
    }
}

fn capture_message_sites(
    checked: &crate::CheckResult,
    root: CheckedActorMessageRoot,
    scopes: &[(Span, usize)],
    calls: &HashMap<Span, CheckedActorMessageCall>,
    nested: &HashMap<Span, Vec<CheckedComptimeTypeBinding>>,
    sites: &mut ActorMessageSites,
) {
    for (source_span, call) in calls {
        if *source_span == call.source_span && checked.validates_actor_message(call) {
            sites.0.insert(
                ActorMessageSite {
                    root,
                    scopes: scopes.to_vec(),
                    source_span: *source_span,
                },
                call.origin.clone(),
            );
        }
    }
    for (owner, bodies) in nested {
        for (index, binding) in bodies.iter().enumerate() {
            let mut selected = scopes.to_vec();
            selected.push((*owner, index));
            capture_message_sites(
                checked,
                root,
                &selected,
                &binding.body.actor_message_calls,
                &binding.body.comptime_type_bindings,
                sites,
            );
        }
    }
}
