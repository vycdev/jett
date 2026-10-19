//! Private projection of a genuine HIR actor entry through canonical MIR edits.
use super::*;
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Witness {
    source: Option<hir::ActorHandlerEntry>,
    constructor_views: Vec<hir::ActorStateView>,
    id: FunctionId,
    identity: FunctionIdentity,
    params: Vec<Param>,
    captures: usize,
    result: TypeId,
    locals: Vec<Local>,
    entry: BlockId,
    blocks: Vec<BasicBlock>,
    span: Span,
}
impl Witness {
    fn current(&self, function: &Function) -> Result<(), String> {
        if self.id != function.id
            || self.identity != function.identity
            || self.params != function.params
            || self.captures != function.capture_count
            || self.result != function.return_type
            || self.locals != function.locals
            || self.entry != function.entry
            || self.blocks != function.blocks
            || self.span != function.span
            || self.constructor_views != function.actor_state_views
        {
            return Err(
                "actor handler changed its original lowered body or role projection".into(),
            );
        }
        Ok(())
    }
    fn refresh(&mut self, function: &Function) {
        self.locals = function.locals.clone();
        self.entry = function.entry;
        self.blocks = function.blocks.clone();
    }
}
pub(super) fn seal(source: &hir::Function, function: &Function) -> Result<Option<Witness>, String> {
    if let Some(entry) = &source.actor_handler {
        entry.validate(source)?;
    } else if function.identity.declaration.kind == hir::DeclarationKind::ActorHandler {
        return Err("actor handler has no original HIR entry".into());
    } else if source.actor_state_views.is_empty() {
        return Ok(None);
    } else if source
        .actor_state_views
        .iter()
        .any(|view| !view.matches_constructor(source))
    {
        return Err("actor constructor views lack their original Source function".into());
    }
    if source.params != function.params
        || source.capture_count != function.capture_count
        || !function.locals.starts_with(&source.locals)
        || source.id != function.id
        || source.identity != function.identity
    {
        return Err("actor lowering changed its original checked role header".into());
    }
    Ok(Some(Witness {
        source: source.actor_handler.clone(),
        constructor_views: source.actor_state_views.clone(),
        id: function.id,
        identity: function.identity.clone(),
        params: function.params.clone(),
        captures: function.capture_count,
        result: function.return_type,
        locals: function.locals.clone(),
        entry: function.entry,
        blocks: function.blocks.clone(),
        span: function.span,
    }))
}
pub(crate) fn has_records(function: &Function) -> bool {
    function.actor_handler.is_some() || !function.actor_state_views.is_empty()
}
pub(super) fn validate(function: &Function) -> Result<(), String> {
    match (&function.actor_handler, function.identity.declaration.kind) {
        (Some(witness), hir::DeclarationKind::ActorHandler) if witness.source.is_some() => {
            witness.current(function)
        }
        (Some(witness), hir::DeclarationKind::ActorConstructor)
            if witness.source.is_none() && !witness.constructor_views.is_empty() =>
        {
            witness.current(function)
        }
        (None, hir::DeclarationKind::ActorHandler) => {
            Err("actor handler lacks original native entry receipt".into())
        }
        (Some(_), _) => Err("actor handler receipt attached to foreign function".into()),
        (None, _) if !function.actor_state_views.is_empty() => {
            Err("actor state views lack original constructor witness".into())
        }
        (None, _) => Ok(()),
    }
}
impl Function {
    pub fn actor_handler_entry(&self) -> Result<Option<&hir::ActorHandlerEntry>, String> {
        validate(self)?;
        Ok(self
            .actor_handler
            .as_ref()
            .and_then(|witness| witness.source.as_ref()))
    }
    pub fn actor_state_view(&self, local: LocalId) -> Result<bool, String> {
        validate(self)?;
        if self
            .actor_state_views
            .iter()
            .any(|view| view.local() == local && view.matches_metadata(&self.locals))
        {
            return Ok(true);
        }
        let Some(entry) = self.actor_handler_entry()? else {
            return Ok(false);
        };
        Ok(entry.roles().iter().any(|role| {
            role.local() == local
                && matches!(
                    role.role(),
                    jett_typecheck::CheckedActorBindingRole::State { .. }
                )
                && matches!(
                    role.fact().mode,
                    jett_typecheck::CheckedBindingMode::View {
                        source: jett_typecheck::CheckedViewSource::Binding(_)
                    }
                )
        }))
    }
}
impl Function {
    /// The final generated allocation transfers one closed set of ready locals.
    /// A private original constructor receipt authenticates both owner and view
    /// destinations; this never expires an ordinary local or call loan.
    pub(crate) fn actor_constructor_view_transfers(
        &self,
        expression: &Expression,
        types: &TypeInterner,
    ) -> Result<Option<Vec<LocalId>>, String> {
        if self.actor_state_views.is_empty() {
            return Ok(None);
        }
        validate(self)?;
        if self.identity.declaration.kind != hir::DeclarationKind::ActorConstructor {
            return Err("actor view transport is outside its original constructor".into());
        }
        let hir::ExpressionKind::ActorSpawn {
            actor_type,
            args,
            evaluation_order,
            constructor: None,
        } = &expression.kind
        else {
            return Ok(None);
        };
        if !self.blocks.iter().any(|block|matches!(&block.terminator.kind,TerminatorKind::Return(Some(value)) if std::ptr::eq(value,expression))) {return Err("actor view transport is not its terminal original allocation".into());}
        let jett_types::Type::Actor(actor) = types.resolve(self.return_type) else {
            return Err("actor constructor lost canonical actor result".into());
        };
        let actor = types.resolve_actor(*actor);
        let count = actor.capability_params.len() + actor.state_fields.len();
        if actor_type != &actor.name
            || expression.ty != self.return_type
            || args.len() != count
            || evaluation_order.iter().copied().ne(0..count)
        {
            return Err("actor view allocation changed original complete field order".into());
        }
        let mut owners = std::collections::BTreeSet::new();
        let mut aliases = Vec::new();
        for (index, (argument, (_, expected))) in args
            .iter()
            .zip(actor.capability_params.iter().chain(&actor.state_fields))
            .enumerate()
        {
            if argument.ty != *expected {
                return Err("actor view allocation changed canonical field type".into());
            }
            if let hir::ExpressionKind::Local(local) = argument.kind {
                if self.is_view_local(local) || !owners.insert(local.index() as usize) {
                    return Err(
                        "actor allocation requires unique original owned field destinations".into(),
                    );
                }
                continue;
            }
            let hir::ExpressionKind::View(value) = &argument.kind else {
                return Err("actor allocation view fields must be frozen original locals".into());
            };
            let hir::ExpressionKind::Local(local) = value.kind else {
                return Err("actor allocation view endpoint is not original local".into());
            };
            let view = self
                .actor_state_views
                .iter()
                .find(|view| {
                    view.local() == local
                        && view.matches_metadata(&self.locals)
                        && view
                            .field()
                            .is_some_and(|field| actor.capability_params.len() + field == index)
                })
                .ok_or("actor allocation view has no exact original state ordinal")?;
            if value.ty != argument.ty
                || value.ty != self.local(local).ok_or("actor alias local absent")?.ty
                || aliases.contains(&view.local())
            {
                return Err("actor allocation alias endpoint changed or repeated".into());
            }
            aliases.push(view.local());
        }
        if aliases.len() != self.actor_state_views.len() {
            return Err("actor allocation lost a persistent dependent view".into());
        }
        for alias in &aliases {
            let root = self
                .view_root(*alias)
                .ok_or("actor allocation alias has invalid original backing")?;
            if !owners.contains(&(root.index() as usize)) {
                return Err(
                    "actor allocation alias backing is not among original owned destinations"
                        .into(),
                );
            }
        }
        Ok(Some(aliases))
    }
}

fn refresh(function: &mut Function, before: &Function) -> Result<(), String> {
    let Some(original) = &before.actor_handler else {
        return Ok(());
    };
    if function.id != before.id
        || function.identity != before.identity
        || function.params != before.params
        || function.capture_count != before.capture_count
        || function.return_type != before.return_type
        || !function.locals.starts_with(&before.locals)
    {
        return Err("actor preparation changed original checked role identity".into());
    }
    let mut next = original.clone();
    next.refresh(function);
    function.actor_handler = Some(next);
    validate(function)
}
pub(super) fn sequence_transition(
    function: &mut Function,
    before: &Function,
    edit: &crate::breakpoint_regions::SequenceEdit,
    types: &TypeInterner,
) -> Result<(), String> {
    if !has_records(before) {
        return Ok(());
    }
    validate(before)?;
    let mut replay = function.clone();
    crate::breakpoint_regions::sequence_transition(&mut replay, before, edit.clone(), types)?;
    refresh(function, before)
}
pub(super) fn sum_transition(
    function: &mut Function,
    before: &Function,
    types: &TypeInterner,
) -> Result<(), String> {
    if !has_records(before) {
        return Ok(());
    }
    validate(before)?;
    let mut replay = function.clone();
    crate::breakpoint_regions::sum_transition(&mut replay, before, types)?;
    refresh(function, before)
}
pub(super) fn remap_blocks(
    function: &mut Function,
    before: &Function,
    map: &[Option<BlockId>],
) -> Result<(), String> {
    if !has_records(before) {
        return Ok(());
    }
    validate(before)?;
    let cfg = ControlFlowGraph::analyze(before).map_err(|errors| format!("{errors:?}"))?;
    let reachable = cfg
        .reverse_postorder()
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    if map.len() != before.blocks.len()
        || before
            .blocks
            .iter()
            .any(|block| map[block.id.index() as usize].is_some() != reachable.contains(&block.id))
    {
        return Err("actor block map is not exact reachable compaction".into());
    }
    let get = |id: BlockId| {
        map.get(id.index() as usize)
            .copied()
            .flatten()
            .ok_or("actor live block has no exact compaction map")
    };
    let mut expected = before
        .blocks
        .iter()
        .filter(|block| reachable.contains(&block.id))
        .cloned()
        .collect::<Vec<_>>();
    for block in &mut expected {
        block.id = get(block.id)?;
        crate::sequences::prune::block_targets(&mut block.terminator.kind, &|target| {
            *target = map[target.index() as usize].expect("reachable target has map")
        });
    }
    if expected != function.blocks
        || function.entry != get(before.entry)?
        || function.locals != before.locals
    {
        return Err("actor block compaction changed surviving operations".into());
    }
    refresh(function, before)
}

#[cfg(test)]
mod tests;
