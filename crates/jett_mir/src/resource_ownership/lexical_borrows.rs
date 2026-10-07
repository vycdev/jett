//! Private lexical declaration and retirement custody. Source paths never remap.
use super::*;
use hir::{ExpressionKind as E, StatementKind as H};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceLexicalExitId(usize);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceLexicalExitKind {
    Fallthrough,
    Return,
    Break,
    Continue,
}
#[derive(Debug, Clone, PartialEq)]
pub struct ResourceLexicalExit {
    id: ResourceLexicalExitId,
    kind: ResourceLexicalExitKind,
    site: ResourceSite,
    pub(super) scopes: Vec<LexicalScope>,
    original_exit: Option<hir::Statement>,
    target: Option<BlockId>,
}
impl ResourceLexicalExit {
    pub fn kind(&self) -> ResourceLexicalExitKind {
        self.kind
    }
    pub(super) fn declarations(&self) -> impl Iterator<Item = LocalId> + '_ {
        self.scopes
            .iter()
            .rev()
            .flat_map(|scope| scope.active.iter().rev().copied())
    }
    pub(crate) fn current_locals(&self, function: &Function) -> Result<Vec<LocalId>, String> {
        let witness = function
            .resource_lowering
            .as_ref()
            .ok_or("Resource lexical exit has no exact function witness")?;
        let mut locals = Vec::new();
        for original in self.declarations() {
            let row = witness
                .borrowed_sums
                .iter()
                .find(|row| row.original_local() == original)
                .ok_or("Resource lexical exit lost its current declaration headers")?;
            for local in row.retirement_locals() {
                if !locals.contains(&local) {
                    locals.push(local);
                }
            }
        }
        Ok(locals)
    }
}
#[derive(Debug, Clone)]
pub struct ResourceLexicalExitPlan {
    site: ResourceSite,
    kind: ResourceLexicalExitKind,
    operation_ids: Vec<ResourceOperationId>,
    clear_locals: Vec<LocalId>,
}
impl ResourceLexicalExitPlan {
    pub fn kind(&self) -> ResourceLexicalExitKind {
        self.kind
    }
    pub fn operation_ids(&self) -> &[ResourceOperationId] {
        &self.operation_ids
    }
    pub fn clear_locals(&self) -> &[LocalId] {
        &self.clear_locals
    }
    pub(super) fn new(
        site: ResourceSite,
        kind: ResourceLexicalExitKind,
        operation_ids: Vec<ResourceOperationId>,
        clear_locals: Vec<LocalId>,
    ) -> Self {
        Self {
            site,
            kind,
            operation_ids,
            clear_locals,
        }
    }
}
impl ResourceFunctionPlan {
    pub fn lexical_exit(
        &self,
        block: BlockId,
        position: ResourcePosition,
    ) -> Option<&ResourceLexicalExitPlan> {
        self.lexical_exits.iter().find(|exit| {
            exit.site
                == ResourceSite {
                    function: self.function,
                    block,
                    position,
                }
        })
    }
}
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LexicalScope {
    path: Vec<usize>,
    pub(crate) active: Vec<LocalId>,
}
#[derive(Clone)]
pub(super) struct Declaration {
    pub(super) path: Vec<usize>,
    pub(super) scope: Vec<usize>,
    pub(super) local: LocalId,
    pub(super) value: Expression,
}
struct OriginalScope {
    path: Vec<usize>,
    body: hir::Block,
}
#[derive(Default)]
struct Inventory {
    declarations: Vec<Declaration>,
    scopes: Vec<OriginalScope>,
}
fn child(path: &[usize], kind: usize, index: usize) -> Vec<usize> {
    let mut next = path.to_vec();
    next.extend([kind, index]);
    next
}
impl Inventory {
    fn block(&mut self, block: &hir::Block, path: Vec<usize>, manifest: &hir::ResourceManifest) {
        self.scopes.push(OriginalScope {
            path: path.clone(),
            body: block.clone(),
        });
        for (index, statement) in block.statements.iter().enumerate() {
            let at = child(&path, 0, index);
            if let H::Let { local, value } = &statement.kind
                && manifest.kind_for_type(value.ty).is_some()
                && matches!(&value.kind, E::View(handle) if matches!(handle.kind, E::Handle { .. }))
            {
                self.declarations.push(Declaration {
                    path: at.clone(),
                    scope: path.clone(),
                    local: *local,
                    value: value.clone(),
                });
            }
            let mut values = Vec::new();
            match &statement.kind {
                H::Let { value, .. }
                | H::HandleDefault(value)
                | H::Expression(value)
                | H::Respond(value) => values.push(value),
                H::Assign { target, value } => values.extend([target, value]),
                H::Return(value) => values.extend(value.iter()),
                H::If {
                    condition,
                    then_block,
                    else_block,
                } => {
                    values.push(condition);
                    self.block(then_block, child(&at, 2, 0), manifest);
                    if let Some(block) = else_block {
                        self.block(block, child(&at, 2, 1), manifest);
                    }
                }
                H::While { condition, body } => {
                    values.push(condition);
                    self.block(body, child(&at, 2, 0), manifest);
                }
                H::For { iterable, body, .. } => {
                    values.push(iterable);
                    self.block(body, child(&at, 2, 0), manifest);
                }
                H::Match { scrutinee, arms } => {
                    values.push(scrutinee);
                    for (arm, value) in arms.iter().enumerate() {
                        self.block(&value.body, child(&at, 2, arm), manifest);
                    }
                }
                H::Assert { condition, message } => {
                    values.push(condition);
                    values.extend(message.iter());
                }
                H::Breakpoint { condition, .. } => values.extend(condition.iter()),
                H::Scope(block) => self.block(block, child(&at, 2, 0), manifest),
                H::ReflectedTypeDispatch { type_info, arms } => {
                    values.push(type_info);
                    for (arm, value) in arms.iter().enumerate() {
                        self.block(&value.body, child(&at, 2, arm), manifest);
                    }
                }
                H::Break | H::Continue | H::Trace(_) => {}
            }
            for (index, value) in values.into_iter().enumerate() {
                self.expression(value, child(&at, 1, index), manifest);
            }
        }
    }
    fn expression(
        &mut self,
        value: &Expression,
        path: Vec<usize>,
        manifest: &hir::ResourceManifest,
    ) {
        let mut children: Vec<&Expression> = Vec::new();
        match &value.kind {
            E::Binary { left, right, .. } => children.extend([left.as_ref(), right.as_ref()]),
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
            | E::StateIs { value, .. } => children.push(value),
            E::Call { args, .. }
            | E::ResourceInvoke { args, .. }
            | E::Intrinsic { args, .. }
            | E::ActorSpawn { args, .. } => children.extend(args),
            E::IndirectCall { callee, args, .. } => {
                children.push(callee);
                children.extend(args);
            }
            E::ActorMessage { actor, args, .. } => {
                children.push(actor);
                children.extend(args);
            }
            E::StructConstruct { fields, .. } | E::BitfieldConstruct { fields, .. } => {
                children.extend(fields)
            }
            E::MachineConstruct { payloads, .. } | E::EnumConstruct { payloads, .. } => {
                children.extend(payloads)
            }
            E::MachineTransition {
                source, payloads, ..
            } => {
                children.push(source);
                children.extend(payloads);
            }
            E::ListConstruct { elements } => children.extend(elements),
            E::MapConstruct { entries } => {
                for entry in entries {
                    children.extend([&entry.key, &entry.value]);
                }
            }
            E::Handle {
                target, failure, ..
            } => {
                children.push(target);
                self.block(failure, child(&path, 3, 0), manifest);
            }
            E::StringInterpolation(parts) => {
                for part in parts {
                    if let hir::StringSegment::Value(value) = part {
                        children.push(value);
                    }
                }
            }
            E::Field { base, .. } => children.push(base),
            // A separate callable body has its own checked function/local archive.
            E::InlineFunction { .. }
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
        for (index, value) in children.into_iter().enumerate() {
            self.expression(value, child(&path, 1, index), manifest);
        }
    }
}
fn inventory(witness: &ResourceLoweringWitness) -> Inventory {
    let mut inventory = Inventory::default();
    inventory.block(&witness.original.body, Vec::new(), &witness.manifest);
    inventory
}
pub(super) fn declarations(witness: &ResourceLoweringWitness) -> Vec<Declaration> {
    inventory(witness).declarations
}
impl Capture {
    pub(crate) fn lexical_scope(&self, block: &hir::Block) -> Result<Option<LexicalScope>, String> {
        let Some(witness) = &self.witness else {
            return Ok(None);
        };
        let inventory = inventory(witness);
        if inventory.declarations.is_empty() {
            return Ok(None);
        }
        let mut scopes = inventory
            .scopes
            .iter()
            .filter(|scope| crate::breakpoint_regions::hir_blocks_equal(&scope.body, block));
        let scope = scopes
            .next()
            .ok_or("Resource lexical block is absent from its exact original archive")?;
        if scopes.next().is_some() {
            return Err(
                "pending Resource lexical block has ambiguous original specialization identity"
                    .into(),
            );
        }
        Ok(Some(LexicalScope {
            path: scope.path.clone(),
            active: Vec::new(),
        }))
    }
    pub(crate) fn lexical_exit(
        &mut self,
        kind: ResourceLexicalExitKind,
        block: BlockId,
        position: ResourcePosition,
        scopes: &[LexicalScope],
        original_exit: Option<&hir::Statement>,
        target: Option<BlockId>,
    ) -> Option<ResourceLexicalExitId> {
        if scopes.iter().all(|scope| scope.active.is_empty()) {
            return None;
        }
        let witness = self.witness.as_mut()?;
        let site = ResourceSite {
            function: witness.original.id,
            block,
            position,
        };
        let id = ResourceLexicalExitId(witness.lexical_exits.len());
        witness.lexical_exits.push(ResourceLexicalExit {
            id,
            kind,
            site,
            scopes: scopes.to_vec(),
            original_exit: original_exit.cloned(),
            target,
        });
        Some(id)
    }
}
pub(super) fn seal(witness: &mut ResourceLoweringWitness) {
    witness.lexical_seal = witness.lexical_exits.clone();
}
pub(super) fn current(
    witness: &ResourceLoweringWitness,
    function: &Function,
) -> Result<(), String> {
    if witness.lexical_exits != witness.lexical_seal {
        return Err(
            "Resource lexical exits changed their independent constructor membership seal".into(),
        );
    }
    let inventory = inventory(witness);
    for declaration in &inventory.declarations {
        if !witness
            .lexical_exits
            .iter()
            .any(|exit| exit.declarations().any(|local| local == declaration.local))
        {
            return Err("Resource lexical exits lost the complete original declaration retirement inventory".into());
        }
    }
    for (index, exit) in witness.lexical_exits.iter().enumerate() {
        if exit.id.0 != index || exit.site.function != function.id {
            return Err(
                "Resource lexical exit has a foreign or duplicated constructor identity".into(),
            );
        }
        let mut keys = BTreeSet::new();
        for scope in &exit.scopes {
            if !inventory
                .scopes
                .iter()
                .any(|original| original.path == scope.path)
            {
                return Err("Resource lexical exit lost its original lexical block".into());
            }
            for key in &scope.active {
                if !keys.insert(key.index())
                    || !inventory
                        .declarations
                        .iter()
                        .any(|row| row.local == *key && row.scope == scope.path)
                {
                    return Err(
                        "Resource lexical exit changed its exact original declaration membership"
                            .into(),
                    );
                }
            }
        }
        if let Some(original_exit) = &exit.original_exit {
            let scope = exit
                .scopes
                .last()
                .and_then(|scope| {
                    inventory
                        .scopes
                        .iter()
                        .find(|original| original.path == scope.path)
                })
                .ok_or("Resource lexical exit lost its exact original containing block")?;
            let matches = scope
                .body
                .statements
                .iter()
                .filter(|statement| {
                    let left = hir::Block {
                        statements: vec![(*statement).clone()],
                        span: original_exit.span,
                    };
                    let right = hir::Block {
                        statements: vec![original_exit.clone()],
                        span: original_exit.span,
                    };
                    crate::breakpoint_regions::hir_blocks_equal(&left, &right)
                })
                .count();
            let kind = matches!(
                (exit.kind, &original_exit.kind),
                (ResourceLexicalExitKind::Return, H::Return(_))
                    | (ResourceLexicalExitKind::Break, H::Break)
                    | (ResourceLexicalExitKind::Continue, H::Continue)
            );
            if matches != 1 || !kind {
                return Err(
                    "Resource lexical exit changed its exact archived control statement".into(),
                );
            }
        } else if exit.kind != ResourceLexicalExitKind::Fallthrough {
            return Err("Resource lexical control exit lost its archived statement".into());
        }
        let block = function
            .blocks
            .get(exit.site.block.index() as usize)
            .filter(|block| block.id == exit.site.block)
            .ok_or("Resource lexical exit lost its current block")?;
        match (exit.kind, exit.site.position) {
            (ResourceLexicalExitKind::Return, ResourcePosition::Terminator)
                if matches!(block.terminator.kind, TerminatorKind::Return(_))
                    && matches!(
                        exit.original_exit.as_ref().map(|statement| &statement.kind),
                        Some(H::Return(_))
                    ) => {}
            (
                ResourceLexicalExitKind::Fallthrough
                | ResourceLexicalExitKind::Break
                | ResourceLexicalExitKind::Continue,
                ResourcePosition::Statement(index),
            ) if matches!(block.statements.get(index).map(|statement| &statement.kind), Some(StatementKind::ResourceLexicalExit(id)) if *id == exit.id) => {
                if let Some(target) = exit.target {
                    if index + 1 != block.statements.len()
                        || !matches!(block.terminator.kind, TerminatorKind::Goto(current) if current == target)
                    {
                        return Err(
                            "Resource lexical loop exit changed its exact boundary target".into(),
                        );
                    }
                }
            }
            _ => {
                return Err(
                    "Resource lexical exit lost its exact constructor node or Return boundary"
                        .into(),
                );
            }
        }
        let copies = function.blocks.iter().flat_map(|block| &block.statements).filter(|statement| matches!(statement.kind, StatementKind::ResourceLexicalExit(id) if id == exit.id)).count();
        if copies != usize::from(exit.kind != ResourceLexicalExitKind::Return) {
            return Err("Resource lexical exit has copied or missing current nodes".into());
        }
    }
    for block in &function.blocks {
        for (index, statement) in block.statements.iter().enumerate() {
            if let StatementKind::ResourceLexicalExit(id) = statement.kind
                && !witness.lexical_exits.iter().any(|exit| {
                    exit.id == id
                        && exit.site
                            == ResourceSite {
                                function: function.id,
                                block: block.id,
                                position: ResourcePosition::Statement(index),
                            }
                })
            {
                return Err("Resource lexical exit node has no exact constructor authority".into());
            }
        }
    }
    Ok(())
}
pub(super) fn remap_blocks(
    witness: &mut ResourceLoweringWitness,
    map: &[Option<BlockId>],
) -> Result<(), String> {
    for exit in &mut witness.lexical_exits {
        exit.site.block = map
            .get(exit.site.block.index() as usize)
            .copied()
            .flatten()
            .ok_or("Resource canonical map removed an authenticated lexical exit")?;
        if let Some(target) = &mut exit.target {
            *target = map
                .get(target.index() as usize)
                .copied()
                .flatten()
                .ok_or("Resource canonical map removed an exact lexical loop target")?;
        }
    }
    seal(witness);
    Ok(())
}
impl Function {
    pub fn resource_lexical_exit(
        &self,
        block: BlockId,
        position: ResourcePosition,
    ) -> Result<Option<&ResourceLexicalExit>, String> {
        let Some(witness) = &self.resource_lowering else {
            return Ok(None);
        };
        witness.current(self)?;
        Ok(witness.lexical_exits.iter().find(|exit| {
            exit.site
                == ResourceSite {
                    function: self.id,
                    block,
                    position,
                }
        }))
    }
}
