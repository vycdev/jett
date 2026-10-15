//! Original checked actor roots, separate from callable function declarations.
use crate::*;
use jett_typecheck::{CheckedActorBindingRole, CheckedActorHandler, CheckedActorHandlerTarget};

#[derive(Debug, Clone, PartialEq)]
pub struct ActorHandlerRole {
    role: CheckedActorBindingRole,
    fact: CheckedBindingFact,
    local: LocalId,
    view: Option<ActorStateView>,
}
impl ActorHandlerRole {
    pub(super) fn new(
        role: CheckedActorBindingRole,
        fact: CheckedBindingFact,
        local: LocalId,
    ) -> Self {
        Self {
            role,
            fact,
            local,
            view: None,
        }
    }
    pub fn role(&self) -> CheckedActorBindingRole {
        self.role
    }
    pub fn fact(&self) -> CheckedBindingFact {
        self.fact
    }
    pub fn local(&self) -> LocalId {
        self.local
    }
    pub(super) fn set_view(&mut self, view: ActorStateView) {
        self.view = Some(view);
    }
    pub fn state_view(&self) -> Option<&ActorStateView> {
        self.view.as_ref()
    }
}

#[derive(Debug, Clone)]
pub struct ActorStateView {
    receipt: Arc<CheckedActorHandler>,
    fact: CheckedBindingFact,
    local: LocalId,
    root: LocalId,
    initializer: Box<Expression>,
    constructor: Option<Box<Snapshot>>,
}
impl PartialEq for ActorStateView {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.receipt, &other.receipt)
            && self.fact == other.fact
            && self.local == other.local
            && self.root == other.root
            && self.initializer == other.initializer
            && self.constructor == other.constructor
    }
}
impl ActorStateView {
    pub(super) fn new(
        receipt: CheckedActorHandler,
        fact: CheckedBindingFact,
        local: LocalId,
        root: LocalId,
        initializer: Expression,
    ) -> Self {
        Self {
            receipt: Arc::new(receipt),
            fact,
            local,
            root,
            initializer: Box::new(initializer),
            constructor: None,
        }
    }
    pub fn local(&self) -> LocalId {
        self.local
    }
    pub fn root(&self) -> LocalId {
        self.root
    }
    pub fn field(&self) -> Option<usize> {
        self.receipt
            .bindings()
            .iter()
            .find_map(|binding| match binding.role {
                CheckedActorBindingRole::State { field } if binding.fact == self.fact => {
                    Some(field)
                }
                _ => None,
            })
    }

    pub fn initializer(&self) -> &Expression {
        &self.initializer
    }
    pub fn matches_constructor(&self, function: &Function) -> bool {
        function.identity.declaration.kind == DeclarationKind::ActorConstructor
            && function.source_definition.is_none()
            && self.constructor.as_deref() == Some(&Snapshot::of(function))
            && self.matches_metadata(&function.locals)
    }
    pub fn matches_metadata(&self, locals: &[Local]) -> bool {
        let Some(alias) = locals
            .get(self.local.index() as usize)
            .filter(|local| local.id == self.local)
        else {
            return false;
        };
        let Some(root) = locals
            .get(self.root.index() as usize)
            .filter(|local| local.id == self.root)
        else {
            return false;
        };
        self.receipt.ownership_validated()
            && self.receipt.bindings().iter().any(|role| {
                matches!(role.role, CheckedActorBindingRole::State { .. }) && role.fact == self.fact
            })
            && matches!(
                self.fact.mode,
                CheckedBindingMode::View {
                    source: CheckedViewSource::Binding(_)
                }
            )
            && alias.view_source == Some(root.id)
            && !alias.mutable
            && alias.ty == self.fact.ty
            && alias.span.file == self.fact.declaration_span.file
            && alias.span.start <= self.fact.declaration_span.start
            && alias.span.end >= self.fact.declaration_span.end
    }
    pub fn validate(&self, locals: &[Local], types: &TypeInterner) -> Result<(), String> {
        if !self.matches_metadata(locals) {
            return Err("actor persistent view changed its original checked state binding".into());
        }
        super::local_views::validate_local_view_initializer(
            &self.initializer,
            self.root,
            locals[self.root.index() as usize].ty,
            self.fact.ty,
            types,
        )
        .map_err(str::to_owned)
    }
    pub(super) fn remap(&self, map: &impl Fn(LocalId) -> Option<LocalId>) -> Result<Self, String> {
        fn expression(
            value: &mut Expression,
            map: &impl Fn(LocalId) -> Option<LocalId>,
        ) -> Result<(), String> {
            match &mut value.kind {
                ExpressionKind::Local(local) => {
                    *local = map(*local)
                        .ok_or("actor view initializer lost original declaration/local join")?
                }
                ExpressionKind::Field { base, .. }
                | ExpressionKind::View(base)
                | ExpressionKind::Coarsen(base)
                | ExpressionKind::Declassify(base) => expression(base, map)?,
                ExpressionKind::InterfaceCoerce { value, adapters } if adapters.is_empty() => {
                    expression(value, map)?
                }
                _ => {
                    return Err(
                        "actor state view initializer is not a finite stable typed projection"
                            .into(),
                    );
                }
            }
            Ok(())
        }
        let mut result = self.clone();
        result.local = map(self.local).ok_or("actor view lost alias declaration")?;
        result.root = map(self.root).ok_or("actor view lost root declaration")?;
        expression(&mut result.initializer, map)?;
        Ok(result)
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Snapshot {
    id: FunctionId,
    identity: FunctionIdentity,
    params: Vec<Param>,
    captures: usize,
    result: TypeId,
    locals: Vec<Local>,
    body: Block,
    span: Span,
}
impl Snapshot {
    fn of(function: &Function) -> Self {
        Self {
            id: function.id,
            identity: function.identity.clone(),
            params: function.params.clone(),
            captures: function.capture_count,
            result: function.return_type,
            locals: function.locals.clone(),
            body: function.body.clone(),
            span: function.span,
        }
    }
}

/// Minted only while lowering the actual original ActorDef handler ordinal.
/// Source ownership remains in the checker receipt; this is its native projection.
#[derive(Debug, Clone)]
pub struct ActorHandlerEntry {
    target: CheckedActorHandlerTarget,
    receipt: Arc<CheckedActorHandler>,
    roles: Vec<ActorHandlerRole>,
    snapshot: Option<Box<Snapshot>>,
}
impl PartialEq for ActorHandlerEntry {
    fn eq(&self, other: &Self) -> bool {
        self.target == other.target
            && Arc::ptr_eq(&self.receipt, &other.receipt)
            && self.roles == other.roles
            && self.snapshot == other.snapshot
    }
}
impl ActorHandlerEntry {
    pub(super) fn new(
        target: CheckedActorHandlerTarget,
        receipt: CheckedActorHandler,
        roles: Vec<ActorHandlerRole>,
    ) -> Self {
        Self {
            target,
            receipt: Arc::new(receipt),
            roles,
            snapshot: None,
        }
    }
    pub fn target(&self) -> CheckedActorHandlerTarget {
        self.target
    }
    pub fn roles(&self) -> &[ActorHandlerRole] {
        &self.roles
    }
    pub fn allows_state_view(&self, function: &Function, local: LocalId) -> bool {
        if self.validate(function).is_err() {
            return false;
        }
        self.roles.iter().any(|role| {
            role.local == local
                && matches!(role.role, CheckedActorBindingRole::State { .. })
                && matches!(
                    role.fact.mode,
                    CheckedBindingMode::View {
                        source: CheckedViewSource::Binding(_)
                    }
                )
        })
    }
    pub fn validate(&self, function: &Function) -> Result<(), String> {
        if !self.receipt.ownership_validated()
            || self.receipt.target() != self.target
            || self.receipt.body_span() != function.body.span
            || self.receipt.response_type() != function.return_type
            || function.source_definition.is_some()
            || function.identity.declaration.kind != DeclarationKind::ActorHandler
        {
            return Err("actor entry changed its original checked handler root".into());
        }
        if self.roles.len() != self.receipt.bindings().len()
            || self.roles.len() != function.params.len()
        {
            return Err("actor entry role cardinality changed".into());
        }
        let mut states = 0;
        let mut capabilities = 0;
        let mut messages = 0;
        for (index, (role, checked)) in self.roles.iter().zip(self.receipt.bindings()).enumerate() {
            let parameter = &function.params[index];
            let local = function
                .locals
                .get(role.local.index() as usize)
                .filter(|local| local.id == role.local)
                .ok_or("actor role local is absent")?;
            if role.local.index() as usize != index
                || role.role != checked.role
                || role.fact != checked.fact
                || parameter.local != role.local
                || parameter.ty != role.fact.ty
                || parameter.mutable != role.fact.mutable
                || local.ty != role.fact.ty
                || local.mutable != role.fact.mutable
                || local.span.file != role.fact.declaration_span.file
                || local.span.start > role.fact.declaration_span.start
                || local.span.end < role.fact.declaration_span.end
            {
                return Err("actor role changed its exact checked declaration".into());
            }
            let mode = match role.fact.mode {
                CheckedBindingMode::Owned => ParamMode::Owned,
                CheckedBindingMode::View { .. } => ParamMode::View,
            };
            if parameter.mode != mode {
                return Err("actor role changed checked ownership mode".into());
            }
            match role.role {
                CheckedActorBindingRole::Capability { parameter }
                    if parameter == capabilities && states == 0 && messages == 0 =>
                {
                    capabilities += 1
                }
                CheckedActorBindingRole::State { field } if field == states && messages == 0 => {
                    states += 1
                }
                CheckedActorBindingRole::Message { parameter } if parameter == messages => {
                    messages += 1
                }
                _ => return Err("actor role ordinal/order changed".into()),
            }
            if let CheckedBindingMode::View {
                source: CheckedViewSource::Binding(definition),
            } = role.fact.mode
            {
                let owner = self.roles[..index]
                    .iter()
                    .find(|owner| owner.fact.definition == definition)
                    .ok_or("actor state view has no earlier original backing role")?;
                if role.view.as_ref().is_none_or(|view| {
                    !view.matches_metadata(&function.locals)
                        || view.root() != owner.local
                        || view.local() != local.id
                }) {
                    return Err(
                        "actor state view lost original typed initializer projection".into(),
                    );
                }
                if local.view_source != Some(owner.local) {
                    return Err("actor state view changed original dependency".into());
                }
            } else if local.view_source.is_some() || role.view.is_some() {
                return Err("actor role acquired an invented alias origin".into());
            }
        }
        if function.capture_count != capabilities + states {
            return Err("actor capture projection changed".into());
        }
        if self.snapshot.as_deref() != Some(&Snapshot::of(function)) {
            return Err("actor handler changed its sealed original native body".into());
        }
        Ok(())
    }
}

pub(super) fn seal_program(program: &mut Program) -> Result<(), Vec<LowerError>> {
    for function in &mut program.functions {
        let snapshot = Snapshot::of(function);
        for view in &mut function.actor_state_views {
            if view.constructor.is_none() {
                view.constructor = Some(Box::new(snapshot.clone()));
            }
        }
        if let Some(mut entry) = function.actor_handler.take() {
            if entry.snapshot.is_some() {
                function.actor_handler = Some(entry);
                continue;
            }
            entry.snapshot = Some(Box::new(Snapshot::of(function)));
            entry.validate(function).map_err(|message| {
                vec![LowerError {
                    span: function.span,
                    message,
                }]
            })?;
            function.actor_handler = Some(entry);
        }
    }
    Ok(())
}
pub(super) fn validate_program(program: &Program) -> Result<(), Vec<ValidationError>> {
    for function in &program.functions {
        for view in &function.actor_state_views {
            if !view.matches_constructor(function) {
                return Err(vec![ValidationError {
                    span: function.span,
                    message: "actor state view is outside its original constructor body".into(),
                }]);
            }
        }
        if function.identity.declaration.kind == DeclarationKind::ActorHandler {
            let result = function
                .actor_handler
                .as_ref()
                .ok_or_else(|| "actor handler lacks original entry receipt".to_owned())
                .and_then(|entry| entry.validate(function));
            result.map_err(|message| {
                vec![ValidationError {
                    span: function.span,
                    message,
                }]
            })?;
        } else if function.actor_handler.is_some() {
            return Err(vec![ValidationError {
                span: function.span,
                message: "actor entry attached to foreign function kind".into(),
            }]);
        }
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
    super::interface_values::coerce_program(&mut expected, types);
    for function in &mut program.functions {
        if !function.actor_state_views.is_empty() {
            let original = before
                .functions
                .iter()
                .find(|old| old.id == function.id)
                .ok_or_else(|| {
                    vec![LowerError {
                        span: function.span,
                        message: "actor constructor conversion lost original".into(),
                    }]
                })?;
            let replay = expected
                .functions
                .iter()
                .find(|old| old.id == function.id)
                .ok_or_else(|| {
                    vec![LowerError {
                        span: function.span,
                        message: "actor constructor conversion lost replay".into(),
                    }]
                })?;
            if function.actor_state_views != original.actor_state_views
                || Snapshot::of(function) != Snapshot::of(replay)
            {
                return Err(vec![LowerError {
                    span: function.span,
                    message:
                        "actor constructor conversion changed original view proof or operations"
                            .into(),
                }]);
            }
            let snapshot = Snapshot::of(replay);
            for view in &mut function.actor_state_views {
                view.constructor = Some(Box::new(snapshot.clone()));
            }
        }
        let Some(entry) = function.actor_handler.as_mut() else {
            continue;
        };
        let original = before
            .functions
            .iter()
            .find(|old| old.id == function.id)
            .and_then(|old| old.actor_handler.as_ref())
            .ok_or_else(|| {
                vec![LowerError {
                    span: function.span,
                    message: "actor conversion lost original receipt".into(),
                }]
            })?;
        let replay = expected
            .functions
            .iter()
            .find(|old| old.id == function.id)
            .ok_or_else(|| {
                vec![LowerError {
                    span: function.span,
                    message: "actor conversion lost replay function".into(),
                }]
            })?;
        if entry != original {
            return Err(vec![LowerError {
                span: function.span,
                message: "actor conversion replaced original receipt".into(),
            }]);
        }
        let snapshot = Snapshot::of(replay);
        // Borrow the full function only after releasing its mutable receipt.
        entry.snapshot = Some(Box::new(snapshot));
    }
    for function in &program.functions {
        if let Some(entry) = &function.actor_handler {
            entry.validate(function).map_err(|message| {
                vec![LowerError {
                    span: function.span,
                    message,
                }]
            })?;
        }
    }
    Ok(())
}

/// One exact original Send/Ask occurrence with its checked incoming acquisitions.
#[derive(Debug, Clone, PartialEq)]
pub struct ActorMessageProof {
    checked: jett_typecheck::CheckedActorMessageCall,
    handler: FunctionId,
    receiver: Box<Expression>,
    receiver_shape: ActorReceiverProof,
    arguments: Vec<ArgumentOwnership>,
}
impl ActorMessageProof {
    pub fn target(&self) -> CheckedActorHandlerTarget {
        self.checked.target
    }
    pub fn original_receiver(&self) -> &Expression {
        &self.receiver
    }
    pub fn receiver_proof(&self) -> &ActorReceiverProof {
        &self.receiver_shape
    }
    pub fn response_type(&self) -> TypeId {
        self.checked.response_type
    }
    pub fn argument(&self, index: usize) -> Option<&ArgumentOwnership> {
        self.arguments.get(index)
    }
    pub fn validate(
        &self,
        functions: &[Function],
        expression: &Expression,
        locals: &[OwnershipLocalInfo],
        types: &TypeInterner,
        context: jett_typecheck::CheckedOwnershipContext,
        initial: bool,
    ) -> Result<(), String> {
        let ExpressionKind::ActorMessage {
            actor,
            message,
            handler,
            args,
            evaluation_order,
            kind,
            ..
        } = &expression.kind
        else {
            return Err("actor packet attached to non-message".into());
        };
        if initial && actor.as_ref() != self.receiver.as_ref() {
            return Err("actor message receiver changed its original expression".into());
        }
        let expected_kind = match kind {
            ActorMessageKind::Send => jett_typecheck::CheckedActorMessageKind::Send,
            ActorMessageKind::Ask => jett_typecheck::CheckedActorMessageKind::Ask,
        };
        let result = if *kind == ActorMessageKind::Send {
            TypeInterner::NOTHING
        } else {
            self.checked.response_type
        };
        if *handler != self.handler
            || self.checked.kind != expected_kind
            || self.checked.context != context
            || actor.ty != self.checked.receiver_type
            || actor.span != self.checked.receiver_span
            || expression.ty != result
            || expression.span.file != self.checked.source_span.file
            || expression.span.start > self.checked.source_span.start
            || expression.span.end < self.checked.source_span.end
            || !matches!(types.resolve(actor.ty),Type::Actor(id) if *id==self.checked.target.actor)
        {
            return Err(
                "actor message changed its original checked target, kind or source occurrence"
                    .into(),
            );
        }
        let definition = types.resolve_actor(self.checked.target.actor);
        let original = definition
            .messages
            .get(self.checked.target.handler_index)
            .ok_or("actor message ordinal is absent")?;
        if original.name != *message
            || original.responds != self.checked.response_type
            || original.params.len() != args.len()
        {
            return Err("actor message changed original canonical message shape".into());
        }
        let function = functions
            .get(handler.index() as usize)
            .filter(|function| function.id == *handler)
            .ok_or("actor message handler function absent")?;
        let entry = function
            .actor_handler
            .as_ref()
            .ok_or("actor message target has no original handler receipt")?;
        if entry.target() != self.checked.target
            || function.return_type != self.checked.response_type
            || function.params.len() != function.capture_count + args.len()
        {
            return Err("actor message selected a foreign original handler".into());
        }
        for (index, argument) in self.arguments.iter().enumerate() {
            let param = &function.params[function.capture_count + index];
            let access = if param.mode == ParamMode::View {
                jett_typecheck::CheckedCalleeAccess::View
            } else {
                jett_typecheck::CheckedCalleeAccess::Owned
            };
            if original.params[index].1 != argument.parameter_type
                || param.ty != argument.parameter_type
                || access != argument.callee_access
            {
                return Err("actor message changed checked formal type or mode".into());
            }
        }
        super::call_ownership::validate_actor_arguments(
            &self.arguments,
            args,
            evaluation_order,
            context,
            locals,
            types,
            initial,
        )
    }
}
pub(super) fn message_proof(
    body: &BodyLowerer<'_, '_>,
    inner: &Expr,
    span: Span,
    actor: &Expression,
    message: &str,
    handler: FunctionId,
    args: &[Expression],
    order: &[usize],
    kind: ActorMessageKind,
) -> Result<ActorMessageProof, String> {
    let checked = body
        .actor_message_calls
        .get(&span)
        .ok_or("actor message has no exact checked Source packet")?;
    let scopes = body
        .required_scopes
        .iter()
        .map(|scope| (scope.owner, scope.index))
        .collect::<Vec<_>>();
    if !body.parent.check.validates_actor_message_at(
        checked,
        body.actor_message_root,
        &scopes,
        span,
    ) {
        return Err("actor message lacks its original checked session/body occurrence".into());
    }
    let original = match inner {
        Expr::Call(callee, _, _) => callee.as_ref(),
        _ => inner,
    };
    let Expr::FieldAccess(receiver, name, _) = original else {
        return Err("actor Source message receiver changed".into());
    };
    let expected = match kind {
        ActorMessageKind::Send => jett_typecheck::CheckedActorMessageKind::Send,
        ActorMessageKind::Ask => jett_typecheck::CheckedActorMessageKind::Ask,
    };
    if checked.source_span != span
        || checked.receiver_span != receiver.span()
        || actor.span != receiver.span()
        || actor.ty != checked.receiver_type
        || checked.kind != expected
        || checked.context != body.ownership_context
        || name.name != message
        || !matches!(body.parent.check.interner.resolve(actor.ty),Type::Actor(id) if *id==checked.target.actor)
    {
        return Err("actor message does not join its full original Send/Ask source".into());
    }
    let receipt = body
        .parent
        .check
        .actor_handler(checked.target)
        .ok_or("actor message original handler is not authenticated")?;
    if !receipt.ownership_validated() {
        return Err("actor message original handler ownership is incomplete".into());
    }
    let definition = body
        .parent
        .check
        .interner
        .resolve_actor(checked.target.actor);
    if definition
        .messages
        .get(checked.target.handler_index)
        .is_none_or(|original| {
            original.name != message || original.responds != checked.response_type
        })
        || body
            .parent
            .actor_handler_ids
            .get(&(actor.ty, message.to_owned()))
            != Some(&handler)
    {
        return Err("actor message changed original handler ordinal association".into());
    }
    let arguments =
        body.build_source_arguments(span, args, order, &checked.arguments, checked.context)?;
    Ok(ActorMessageProof {
        checked: checked.clone(),
        handler,
        receiver: Box::new(actor.clone()),
        receiver_shape: ActorReceiverProof::original(actor, &body.parent.check.interner),
        arguments,
    })
}

/// Dependency inventory only; this grants no actor or Source authority.
pub fn has_actor_messages(block: &Block) -> bool {
    let mut body = block.clone();
    let mut found = false;
    let result = super::resource_materialization::walk_block(&mut body, &mut |value| {
        found |= matches!(value.kind, ExpressionKind::ActorMessage { .. });
        Ok(())
    });
    result.is_ok() && found
}

#[cfg(test)]
mod tests;
