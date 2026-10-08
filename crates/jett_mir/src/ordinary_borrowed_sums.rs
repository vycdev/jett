//! Constructor-certified ordinary Handle views. These records grant no Resource custody.
use super::*;
use hir::{ExpressionKind as E, StatementKind as H};
use std::sync::Arc;

#[cfg(test)]
mod tests;

// Erased interfaces/callables and actors do not expose all runtime payload
// types through a sum's TypeId. Their ordinary projection proof remains pending.
fn ordinary_type_pending(types: &TypeInterner, ty: TypeId) -> bool {
    fn visit(
        types: &TypeInterner,
        ty: TypeId,
        seen: &mut std::collections::HashSet<TypeId>,
    ) -> bool {
        if ty.index() as usize >= types.len() {
            return true;
        }
        if !seen.insert(ty) {
            return false;
        }
        match types.resolve(ty) {
            Type::Resource(_)
            | Type::Interface(_)
            | Type::Function { .. }
            | Type::Actor(_)
            | Type::Capability(_)
            | Type::TypeConstruction
            | Type::Error => true,
            Type::List(inner)
            | Type::Set(inner)
            | Type::Optional(inner)
            | Type::Secret(inner)
            | Type::Refinement { base: inner, .. } => visit(types, *inner, seen),
            Type::Map(key, value) | Type::Result(key, value) => {
                visit(types, *key, seen) || visit(types, *value, seen)
            }
            Type::Struct(id) => types
                .resolve_struct(*id)
                .fields
                .iter()
                .any(|(_, ty)| visit(types, *ty, seen)),
            Type::Bitfield(id) => types
                .resolve_bitfield(*id)
                .fields
                .iter()
                .any(|field| visit(types, field.ty, seen)),
            Type::Enum(id) => types
                .resolve_enum(*id)
                .variants
                .iter()
                .any(|variant| variant.fields.iter().any(|(_, ty)| visit(types, *ty, seen))),
            Type::Machine(id) | Type::MachineState { machine: id, .. } => types
                .resolve_machine(*id)
                .states
                .iter()
                .any(|state| state.fields.iter().any(|(_, ty)| visit(types, *ty, seen))),
            _ => false,
        }
    }
    visit(types, ty, &mut std::collections::HashSet::new())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrdinarySumPayloadPath {
    OptionalSome,
    ResultOk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrdinarySumProjectionSite {
    block: BlockId,
    statement_index: usize,
}
impl OrdinarySumProjectionSite {
    pub fn block(self) -> BlockId {
        self.block
    }
    pub fn statement_index(self) -> usize {
        self.statement_index
    }
    pub(super) fn new(block: BlockId, statement_index: usize) -> Self {
        Self {
            block,
            statement_index,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OrdinaryBorrowedSumProjection {
    original_path: Vec<usize>,
    original: Expression,
    original_alias: Local,
    original_backing: Local,
    alias: Local,
    backing: Local,
    source: Local,
    output: Local,
    tag: Local,
    error: Option<Local>,
    initialize: OrdinarySumProjectionSite,
    observe: OrdinarySumProjectionSite,
    project: OrdinarySumProjectionSite,
    failure: Option<OrdinarySumProjectionSite>,
    alias_site: OrdinarySumProjectionSite,
    failed: BlockId,
    continuation: BlockId,
    path: OrdinarySumPayloadPath,
}
impl OrdinaryBorrowedSumProjection {
    pub fn alias(&self) -> LocalId {
        self.alias.id
    }
    pub fn backing(&self) -> LocalId {
        self.backing.id
    }
    pub fn source(&self) -> LocalId {
        self.source.id
    }
    pub fn output(&self) -> LocalId {
        self.output.id
    }
    pub fn tag(&self) -> LocalId {
        self.tag.id
    }
    pub fn error(&self) -> Option<LocalId> {
        self.error.as_ref().map(|local| local.id)
    }
    pub fn sum_type(&self) -> TypeId {
        self.backing.ty
    }
    pub fn payload_type(&self) -> TypeId {
        self.alias.ty
    }
    pub fn path(&self) -> OrdinarySumPayloadPath {
        self.path
    }
    pub fn original_handle(&self) -> &Expression {
        let E::View(handle) = &self.original.kind else {
            unreachable!()
        };
        handle
    }
    pub fn initialize_site(&self) -> OrdinarySumProjectionSite {
        self.initialize
    }
    pub fn observe_site(&self) -> OrdinarySumProjectionSite {
        self.observe
    }
    pub fn success_site(&self) -> OrdinarySumProjectionSite {
        self.project
    }
    pub fn failure_site(&self) -> Option<OrdinarySumProjectionSite> {
        self.failure
    }

    fn same(&self, other: &Self) -> bool {
        self.original_path == other.original_path
            && breakpoint_regions::expressions_equal(&self.original, &other.original)
            && self.original_alias == other.original_alias
            && self.original_backing == other.original_backing
            && self.alias == other.alias
            && self.backing == other.backing
            && self.source == other.source
            && self.output == other.output
            && self.tag == other.tag
            && self.error == other.error
            && self.initialize == other.initialize
            && self.observe == other.observe
            && self.project == other.project
            && self.failure == other.failure
            && self.alias_site == other.alias_site
            && self.failed == other.failed
            && self.continuation == other.continuation
            && self.path == other.path
    }
    fn statement<'a>(
        &self,
        function: &'a Function,
        site: OrdinarySumProjectionSite,
    ) -> Result<&'a Statement, String> {
        function
            .blocks
            .get(site.block.index() as usize)
            .filter(|block| block.id == site.block)
            .and_then(|block| block.statements.get(site.statement_index))
            .ok_or_else(|| "ordinary borrowed Handle lost its exact statement site".into())
    }
    fn current(&self, function: &Function) -> Result<(), String> {
        for header in [
            &self.alias,
            &self.backing,
            &self.source,
            &self.output,
            &self.tag,
        ]
        .into_iter()
        .chain(self.error.iter())
        {
            if function.local(header.id) != Some(header) {
                return Err("ordinary borrowed Handle changed a sealed local header".into());
            }
        }
        if (self.alias.mutable && self.original_alias.view_source.is_some())
            || self.backing.mutable
            || self.source.mutable
            || self.output.mutable
            || self.alias.view_source != self.original_alias.view_source.map(|_| self.backing.id)
            || self.source.view_source != Some(self.backing.id)
            || self.output.view_source != Some(self.backing.id)
            || self.source.ty != self.backing.ty
            || self.output.ty != self.alias.ty
            || self.tag.ty != TypeInterner::BOOL
            || self.tag.view_source.is_some()
        {
            return Err("ordinary borrowed Handle lost its immutable nonowning headers".into());
        }
        if !matches!(&self.statement(function, self.initialize)?.kind, StatementKind::Let { local, value }
            if *local == self.source.id && value.ty == self.source.ty
            && matches!(&value.kind, E::View(inner) if inner.ty == self.source.ty
                && matches!(inner.kind, E::Local(backing) if backing == self.backing.id)))
            || !matches!(self.statement(function, self.observe)?.kind, StatementKind::SumTag { source, target }
                if source == self.source.id && target == self.tag.id)
            || !matches!(self.statement(function, self.project)?.kind, StatementKind::SumTake { source, target, success: true }
                if source == self.source.id && target == self.output.id)
            || !matches!(&self.statement(function, self.alias_site)?.kind, StatementKind::Let { local, value }
                if *local == self.alias.id && value.ty == self.alias.ty
                && matches!(&value.kind, E::View(inner) if inner.ty == self.output.ty
                    && matches!(inner.kind, E::Local(output) if output == self.output.id)))
        {
            return Err(
                "ordinary borrowed Handle changed its exact initializer or projection".into(),
            );
        }
        let branch = &function.blocks[self.observe.block.index() as usize];
        if self.initialize.block != self.observe.block
            || self.initialize.statement_index >= self.observe.statement_index
            || self.alias_site.block != self.continuation
            || !matches!(&branch.terminator.kind, TerminatorKind::Branch { condition, then_block, else_block }
                if condition.ty == TypeInterner::BOOL && matches!(condition.kind, E::Local(tag) if tag == self.tag.id)
                && *then_block == self.project.block && *else_block == self.failed && then_block != else_block)
            || !matches!(function.blocks[self.project.block.index() as usize].terminator.kind,
                TerminatorKind::Goto(target) if target == self.continuation)
        {
            return Err("ordinary borrowed Handle lost its exact selecting tag edge".into());
        }
        match (&self.error, self.failure) {
            (Some(error), Some(site))
                if error.ty == TypeInterner::STRING
                    && error.view_source.is_none()
                    && !error.mutable
                    && site.block == self.failed
                    && matches!(self.statement(function, site)?.kind, StatementKind::SumTake { source, target, success: false }
                    if source == self.source.id && target == error.id) => {}
            (None, None) if self.path == OrdinarySumPayloadPath::OptionalSome => {}
            _ => {
                return Err(
                    "ordinary borrowed Handle changed its owning String failure companion".into(),
                );
            }
        }
        let cfg = ControlFlowGraph::analyze(function).map_err(|errors| format!("{errors:?}"))?;
        if !cfg.reverse_postorder().contains(&self.initialize.block)
            || !cfg.reverse_postorder().contains(&self.project.block)
            || !cfg.reverse_postorder().contains(&self.alias_site.block)
            || cfg.predecessors(self.project.block) != [self.observe.block]
            || cfg.predecessors(self.failed) != [self.observe.block]
        {
            return Err(
                "ordinary borrowed Handle is disconnected from its constructor guard".into(),
            );
        }
        let definitions = function.blocks.iter().flat_map(|block| &block.statements);
        let alias_count = definitions
            .clone()
            .filter(|statement| {
                matches!(statement.kind,
            StatementKind::Let { local, .. } if local == self.alias.id)
            })
            .count();
        let output_count = definitions
            .filter(|statement| {
                matches!(statement.kind,
            StatementKind::SumTake { target, .. } if target == self.output.id)
            })
            .count();
        if alias_count != 1 || output_count != 1 {
            return Err(
                "ordinary borrowed Handle has copied or missing current declarations".into(),
            );
        }
        Ok(())
    }
    fn remap_blocks(&mut self, map: &[Option<BlockId>]) -> Result<(), String> {
        let remap = |id: &mut BlockId| -> Result<(), String> {
            *id = map
                .get(id.index() as usize)
                .copied()
                .flatten()
                .ok_or("ordinary borrowed Handle canonical map removed its guard")?;
            Ok(())
        };
        for site in [
            &mut self.initialize,
            &mut self.observe,
            &mut self.project,
            &mut self.alias_site,
        ]
        .into_iter()
        .chain(self.failure.iter_mut())
        {
            remap(&mut site.block)?;
        }
        remap(&mut self.failed)?;
        remap(&mut self.continuation)
    }
    fn remap_locals(&mut self, map: &[Option<LocalId>]) -> Result<(), String> {
        for header in [
            &mut self.alias,
            &mut self.backing,
            &mut self.source,
            &mut self.output,
            &mut self.tag,
        ]
        .into_iter()
        .chain(self.error.iter_mut())
        {
            header.id = map
                .get(header.id.index() as usize)
                .copied()
                .flatten()
                .ok_or("ordinary borrowed Handle canonical map removed a sealed header")?;
            if let Some(source) = &mut header.view_source {
                *source = map
                    .get(source.index() as usize)
                    .copied()
                    .flatten()
                    .ok_or("ordinary borrowed Handle canonical map removed its backing")?;
            }
        }
        Ok(())
    }
    fn shift(&mut self, block: BlockId, count: usize) {
        for site in [
            &mut self.initialize,
            &mut self.observe,
            &mut self.project,
            &mut self.alias_site,
        ]
        .into_iter()
        .chain(self.failure.iter_mut())
        {
            if site.block == block {
                site.statement_index += count;
            }
        }
    }
}

#[derive(Debug, Clone)]
struct Declaration {
    path: Vec<usize>,
    value: Expression,
    alias: Local,
    backing: Local,
    path_kind: OrdinarySumPayloadPath,
}
#[derive(Default)]
struct Inventory {
    declarations: Vec<Declaration>,
}
fn child(path: &[usize], kind: usize, index: usize) -> Vec<usize> {
    let mut child = path.to_vec();
    child.extend([kind, index]);
    child
}
impl Inventory {
    fn block(
        &mut self,
        block: &hir::Block,
        path: Vec<usize>,
        function: &hir::Function,
        types: &TypeInterner,
    ) -> Result<(), String> {
        for (index, statement) in block.statements.iter().enumerate() {
            let at = child(&path, 0, index);
            if let H::Let { local, value } = &statement.kind {
                let alias = function
                    .locals
                    .get(local.index() as usize)
                    .filter(|header| header.id == *local)
                    .ok_or("ordinary borrowed Handle has no original local header")?;
                let written_source = match &value.kind {
                    E::View(handle) => match &handle.kind {
                        E::Handle { target, .. } => match &target.kind {
                            E::View(backing) => match backing.kind {
                                E::Local(source) => Some(source),
                                _ => None,
                            },
                            _ => None,
                        },
                        _ => None,
                    },
                    _ => None,
                };
                if let Some(source) = written_source {
                    let backing = function
                        .locals
                        .get(source.index() as usize)
                        .filter(|header| header.id == source)
                        .ok_or("ordinary borrowed Handle has no original backing header")?;
                    if !ordinary_type_pending(types, backing.ty)
                        && let Some(handle) = hir::borrowed_sum_view_initializer(
                            value, source, backing.ty, alias.ty, types,
                        )?
                    {
                        let copyable =
                            jett_typecheck::ownership::is_implicitly_copyable(types, alias.ty);
                        if backing.mutable || (alias.mutable && !copyable) {
                            return Err(
                                "ordinary borrowed Handle requires an immutable backing and view alias".into()
                            );
                        }
                        if alias.view_source != if copyable { None } else { Some(source) } {
                            return Err(
                                "ordinary borrowed Handle changed its checked alias ownership mode"
                                    .into(),
                            );
                        }
                        let E::Handle { kind, .. } = &handle.kind else {
                            unreachable!()
                        };
                        let path_kind = match kind {
                            hir::HandleKind::Optional => OrdinarySumPayloadPath::OptionalSome,
                            hir::HandleKind::Result => {
                                if !matches!(types.resolve(backing.ty), Type::Result(_, failure) if *failure == TypeInterner::STRING)
                                {
                                    return Err("ordinary borrowed Handle failure companion requires String".into());
                                }
                                OrdinarySumPayloadPath::ResultOk
                            }
                            _ => {
                                return Err(
                                    "ordinary borrowed Handle has an unsupported sum kind".into()
                                );
                            }
                        };
                        if self.declarations.iter().any(|old| old.alias.id == *local) {
                            return Err(
                                "ordinary borrowed Handle lacks a unique original declaration"
                                    .into(),
                            );
                        }
                        self.declarations.push(Declaration {
                            path: at.clone(),
                            value: value.clone(),
                            alias: alias.clone(),
                            backing: backing.clone(),
                            path_kind,
                        });
                    }
                }
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
                    self.block(then_block, child(&at, 2, 0), function, types)?;
                    if let Some(block) = else_block {
                        self.block(block, child(&at, 2, 1), function, types)?;
                    }
                }
                H::While { condition, body } => {
                    values.push(condition);
                    self.block(body, child(&at, 2, 0), function, types)?;
                }
                H::For { iterable, body, .. } => {
                    values.push(iterable);
                    self.block(body, child(&at, 2, 0), function, types)?;
                }
                H::Match { scrutinee, arms } => {
                    values.push(scrutinee);
                    for (index, arm) in arms.iter().enumerate() {
                        self.block(&arm.body, child(&at, 2, index), function, types)?;
                    }
                }
                H::Assert { condition, message } => {
                    values.push(condition);
                    values.extend(message.iter());
                }
                H::Breakpoint { condition, .. } => values.extend(condition.iter()),
                H::Scope(block) => self.block(block, child(&at, 2, 0), function, types)?,
                H::ReflectedTypeDispatch { type_info, arms } => {
                    values.push(type_info);
                    for (index, arm) in arms.iter().enumerate() {
                        self.block(&arm.body, child(&at, 2, index), function, types)?;
                    }
                }
                H::Break | H::Continue | H::Trace(_) => {}
            }
            for (index, value) in values.into_iter().enumerate() {
                self.expression(value, child(&at, 1, index), function, types)?;
            }
        }
        Ok(())
    }
    fn expression(
        &mut self,
        value: &Expression,
        path: Vec<usize>,
        function: &hir::Function,
        types: &TypeInterner,
    ) -> Result<(), String> {
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
                self.block(failure, child(&path, 3, 0), function, types)?;
            }
            E::StringInterpolation(parts) => {
                for part in parts {
                    if let hir::StringSegment::Value(value) = part {
                        children.push(value);
                    }
                }
            }
            E::Field { base, .. } => children.push(base),
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
            self.expression(value, child(&path, 1, index), function, types)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct CurrentSeal {
    id: FunctionId,
    identity: FunctionIdentity,
    debug_kind: hir::FunctionDebugKind,
    params: Vec<Param>,
    capture_count: usize,
    return_type: TypeId,
    span: Span,
    locals: Vec<Local>,
    entry: BlockId,
    blocks: Vec<BasicBlock>,
    rows: Vec<OrdinaryBorrowedSumProjection>,
}
impl CurrentSeal {
    fn new(function: &Function, rows: &[OrdinaryBorrowedSumProjection]) -> Self {
        Self {
            id: function.id,
            identity: function.identity.clone(),
            debug_kind: function.debug_kind.clone(),
            params: function.params.clone(),
            capture_count: function.capture_count,
            return_type: function.return_type,
            span: function.span,
            locals: function.locals.clone(),
            entry: function.entry,
            blocks: function.blocks.clone(),
            rows: rows.to_vec(),
        }
    }
    fn matches(&self, function: &Function) -> bool {
        self.id == function.id
            && self.identity == function.identity
            && self.debug_kind == function.debug_kind
            && self.params == function.params
            && self.capture_count == function.capture_count
            && self.return_type == function.return_type
            && self.span == function.span
            && self.locals == function.locals
            && self.entry == function.entry
            && breakpoint_regions::blocks_equal(&self.blocks, &function.blocks)
    }
}
#[derive(Debug, Clone)]
pub(super) struct Witness {
    original: Arc<hir::Function>,
    declarations: Arc<Vec<Declaration>>,
    rows: Vec<OrdinaryBorrowedSumProjection>,
    sealed: Arc<CurrentSeal>,
}
impl PartialEq for Witness {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.original, &other.original)
            && Arc::ptr_eq(&self.declarations, &other.declarations)
            && Arc::ptr_eq(&self.sealed, &other.sealed)
            && same_rows(&self.rows, &other.rows)
    }
}
fn same_rows(
    left: &[OrdinaryBorrowedSumProjection],
    right: &[OrdinaryBorrowedSumProjection],
) -> bool {
    left.len() == right.len() && left.iter().zip(right).all(|(left, right)| left.same(right))
}
impl Witness {
    fn current(&self, function: &Function) -> Result<(), String> {
        if !self.sealed.matches(function) || !same_rows(&self.rows, &self.sealed.rows) {
            return Err(
                "ordinary borrowed Handle differs from its independently sealed constructor graph"
                    .into(),
            );
        }
        if self.original.id != function.id
            || self.original.identity != function.identity
            || self.rows.len() != self.declarations.len()
        {
            return Err("ordinary borrowed Handle lost its original function or complete declaration inventory".into());
        }
        for (index, row) in self.rows.iter().enumerate() {
            row.current(function)?;
            if self.rows[..index]
                .iter()
                .any(|old| old.original_alias.id == row.original_alias.id)
            {
                return Err("ordinary borrowed Handle duplicated an original declaration".into());
            }
            if self
                .declarations
                .iter()
                .filter(|declaration| {
                    declaration.path == row.original_path
                        && declaration.alias == row.original_alias
                        && declaration.backing == row.original_backing
                        && declaration.path_kind == row.path
                        && breakpoint_regions::expressions_equal(&declaration.value, &row.original)
                })
                .count()
                != 1
                || self
                    .original
                    .locals
                    .get(row.original_alias.id.index() as usize)
                    != Some(&row.original_alias)
                || self
                    .original
                    .locals
                    .get(row.original_backing.id.index() as usize)
                    != Some(&row.original_backing)
                || row.original_alias.ty != row.alias.ty
                || row.original_backing.ty != row.backing.ty
            {
                return Err(
                    "ordinary borrowed Handle changed its exact original declaration association"
                        .into(),
                );
            }
        }
        Ok(())
    }
}

#[derive(Default, Clone)]
pub(super) struct Capture {
    original: Option<Arc<hir::Function>>,
    declarations: Arc<Vec<Declaration>>,
    rows: Vec<OrdinaryBorrowedSumProjection>,
}
pub(super) struct Seed {
    pub original: Expression,
    pub backing: Local,
    declaration: Declaration,
}
impl Capture {
    pub(super) fn authenticated(
        function: &hir::Function,
        types: &TypeInterner,
    ) -> Result<Self, String> {
        let mut inventory = Inventory::default();
        inventory.block(&function.body, Vec::new(), function, types)?;
        if inventory.declarations.is_empty() {
            return Ok(Self::default());
        }
        Ok(Self {
            original: Some(Arc::new(function.clone())),
            declarations: Arc::new(inventory.declarations),
            rows: Vec::new(),
        })
    }
    pub(super) fn seed(&self, local: LocalId, value: &Expression) -> Result<Option<Seed>, String> {
        let Some(declaration) = self
            .declarations
            .iter()
            .find(|declaration| declaration.alias.id == local)
        else {
            return Ok(None);
        };
        if !breakpoint_regions::expressions_equal(&declaration.value, value)
            || self.rows.iter().any(|row| row.original_alias.id == local)
        {
            return Err(
                "ordinary borrowed Handle changed or copied its original initializer".into(),
            );
        }
        Ok(Some(Seed {
            original: declaration.value.clone(),
            backing: declaration.backing.clone(),
            declaration: declaration.clone(),
        }))
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn record(
        &mut self,
        seed: Seed,
        source: Local,
        output: Local,
        tag: Local,
        error: Option<Local>,
        initialize: OrdinarySumProjectionSite,
        observe: OrdinarySumProjectionSite,
        project: OrdinarySumProjectionSite,
        failure: Option<OrdinarySumProjectionSite>,
        alias_site: OrdinarySumProjectionSite,
        failed: BlockId,
        continuation: BlockId,
    ) {
        self.rows.push(OrdinaryBorrowedSumProjection {
            original_path: seed.declaration.path,
            original: seed.original,
            original_alias: seed.declaration.alias.clone(),
            original_backing: seed.declaration.backing.clone(),
            alias: seed.declaration.alias,
            backing: seed.declaration.backing,
            source,
            output,
            tag,
            error,
            initialize,
            observe,
            project,
            failure,
            alias_site,
            failed,
            continuation,
            path: seed.declaration.path_kind,
        });
    }
    pub(super) fn finish(self, function: &Function) -> Result<Option<Witness>, String> {
        let Some(original) = self.original else {
            return Ok(None);
        };
        let sealed = Arc::new(CurrentSeal::new(function, &self.rows));
        let witness = Witness {
            original,
            declarations: self.declarations,
            rows: self.rows,
            sealed,
        };
        witness.current(function)?;
        Ok(Some(witness))
    }
}

pub(super) fn has_records(function: &Function) -> bool {
    function.ordinary_borrowed_sums.is_some()
}
pub(super) fn validate_current(function: &Function) -> Result<(), String> {
    function
        .ordinary_borrowed_sums
        .as_ref()
        .map_or(Ok(()), |witness| witness.current(function))
}
pub(super) fn validate(function: &Function, types: &TypeInterner) -> Result<(), String> {
    validate_current(function)?;
    for block in &function.blocks {
        for (index, statement) in block.statements.iter().enumerate() {
            if let StatementKind::SumTag { source, .. } | StatementKind::SumTake { source, .. } =
                statement.kind
                && function.is_view_local(source)
                && let Some(header) = function.local(source)
                && !resource_type_pending(types, header.ty)
                && function
                    .ordinary_borrowed_sum_projection(block.id, index)?
                    .is_none()
            {
                return Err(
                    "ordinary borrowed sum operation has no constructor-certified projection"
                        .into(),
                );
            }
        }
    }
    let Some(witness) = &function.ordinary_borrowed_sums else {
        return Ok(());
    };
    for declaration in witness.declarations.iter() {
        if ordinary_type_pending(types, declaration.backing.ty)
            || hir::borrowed_sum_view_initializer(
                &declaration.value,
                declaration.backing.id,
                declaration.backing.ty,
                declaration.alias.ty,
                types,
            )?
            .is_none()
        {
            return Err(
                "ordinary borrowed Handle changed its Resource-free original sum type".into(),
            );
        }
        let copyable =
            jett_typecheck::ownership::is_implicitly_copyable(types, declaration.alias.ty);
        if declaration.alias.view_source
            != if copyable {
                None
            } else {
                Some(declaration.backing.id)
            }
        {
            return Err(
                "ordinary borrowed Handle changed its checked final copy or view mode".into(),
            );
        }
        if declaration.path_kind == OrdinarySumPayloadPath::ResultOk
            && !matches!(types.resolve(declaration.backing.ty), Type::Result(_, failure) if *failure == TypeInterner::STRING)
        {
            return Err("ordinary borrowed Handle changed its exact String failure type".into());
        }
    }
    Ok(())
}
impl Function {
    /// Read only after the complete constructor graph and original association are authenticated.
    pub fn ordinary_borrowed_sum_projection(
        &self,
        block: BlockId,
        statement_index: usize,
    ) -> Result<Option<&OrdinaryBorrowedSumProjection>, String> {
        let Some(witness) = &self.ordinary_borrowed_sums else {
            return Ok(None);
        };
        witness.current(self)?;
        let site = OrdinarySumProjectionSite::new(block, statement_index);
        Ok(witness
            .rows
            .iter()
            .find(|row| row.observe == site || row.project == site || row.failure == Some(site)))
    }
    pub fn ordinary_borrowed_sum_alias(
        &self,
        local: LocalId,
        value: &Expression,
    ) -> Result<bool, String> {
        let Some(witness) = &self.ordinary_borrowed_sums else {
            return Ok(false);
        };
        witness.current(self)?;
        Ok(witness.rows.iter().any(|row| row.alias.id == local
            && matches!(&self.blocks[row.alias_site.block.index() as usize].statements[row.alias_site.statement_index].kind,
                StatementKind::Let { local: defined, value: current } if *defined == local && std::ptr::eq(current, value))))
    }
    pub(crate) fn ordinary_copied_sum_intermediates(
        &self,
        local: LocalId,
        value: &Expression,
    ) -> Result<Option<[LocalId; 2]>, String> {
        if !self.ordinary_borrowed_sum_alias(local, value)? {
            return Ok(None);
        }
        let witness = self
            .ordinary_borrowed_sums
            .as_ref()
            .expect("authenticated ordinary alias");
        Ok(witness
            .rows
            .iter()
            .find(|row| row.alias.id == local && row.original_alias.view_source.is_none())
            .map(|row| [row.source.id, row.output.id]))
    }
}

fn transition(
    function: &mut Function,
    before: &Function,
    expected: &Function,
    rows: Vec<OrdinaryBorrowedSumProjection>,
) -> Result<(), String> {
    let original = before
        .ordinary_borrowed_sums
        .as_ref()
        .ok_or("ordinary borrowed Handle has no before witness")?;
    original.current(before)?;
    if function.ordinary_borrowed_sums.as_ref() != Some(original)
        || !CurrentSeal::new(expected, &rows).matches(function)
    {
        return Err(
            "ordinary borrowed Handle preparation differs from its exact authenticated edit".into(),
        );
    }
    // Identity canonical edits preserve the private proof identity as well as the graph.
    if original.sealed.matches(function) && same_rows(&rows, &original.rows) {
        original.current(function)?;
        return Ok(());
    }
    let mut witness = original.clone();
    witness.rows = rows;
    witness.sealed = Arc::new(CurrentSeal::new(expected, &witness.rows));
    witness.current(function)?;
    function.ordinary_borrowed_sums = Some(witness);
    Ok(())
}
pub(super) fn remap_blocks(
    function: &mut Function,
    before: &Function,
    map: &[Option<BlockId>],
) -> Result<(), String> {
    let Some(witness) = &before.ordinary_borrowed_sums else {
        return Ok(());
    };
    witness.current(before)?;
    let cfg = ControlFlowGraph::analyze(before).map_err(|errors| format!("{errors:?}"))?;
    let mut canonical = vec![None; before.blocks.len()];
    let mut next = 0;
    for block in &before.blocks {
        if cfg.reverse_postorder().contains(&block.id) {
            canonical[block.id.index() as usize] = Some(BlockId(next));
            next += 1;
        }
    }
    if map != canonical {
        return Err("ordinary borrowed Handle block map is not exact reachable compaction".into());
    }
    let mut expected = before.clone();
    expected
        .blocks
        .retain(|block| map[block.id.index() as usize].is_some());
    let remap = |id: &mut BlockId| {
        *id = map[id.index() as usize].expect("reachable edge remains reachable");
    };
    remap(&mut expected.entry);
    for block in &mut expected.blocks {
        remap(&mut block.id);
        sequences::prune::block_targets(&mut block.terminator.kind, &remap);
    }
    let mut rows = witness.rows.clone();
    for row in &mut rows {
        row.remap_blocks(map)?;
    }
    transition(function, before, &expected, rows)
}
pub(super) fn remap_locals(
    function: &mut Function,
    before: &Function,
    map: &[Option<LocalId>],
) -> Result<(), String> {
    let Some(witness) = &before.ordinary_borrowed_sums else {
        return Ok(());
    };
    witness.current(before)?;
    if map.len() != before.locals.len() {
        return Err("ordinary borrowed Handle local map has a foreign width".into());
    }
    let mut next = 0;
    for id in map.iter().flatten() {
        if id.index() as usize != next {
            return Err("ordinary borrowed Handle local map is not dense compaction".into());
        }
        next += 1;
    }
    let mut expected = before.clone();
    expected
        .locals
        .retain(|local| map[local.id.index() as usize].is_some());
    let mut valid = true;
    let mut remap = |id: &mut LocalId| {
        if let Some(mapped) = map.get(id.index() as usize).copied().flatten() {
            *id = mapped;
        } else {
            valid = false;
        }
    };
    for parameter in &mut expected.params {
        remap(&mut parameter.local);
    }
    for local in &mut expected.locals {
        remap(&mut local.id);
        if let Some(source) = &mut local.view_source {
            remap(source);
        }
    }
    let mut floor = |floor: &mut u32| {
        if (*floor as usize) <= map.len() {
            *floor = map[..*floor as usize]
                .iter()
                .filter(|id| id.is_some())
                .count() as u32;
        }
    };
    for block in &mut expected.blocks {
        sequences::prune::block_locals(block, &mut remap, &mut floor);
    }
    if !valid {
        return Err("ordinary borrowed Handle local map removed an execution read".into());
    }
    let mut rows = witness.rows.clone();
    for row in &mut rows {
        row.remap_locals(map)?;
    }
    transition(function, before, &expected, rows)
}
pub(super) fn sequence_transition(
    function: &mut Function,
    before: &Function,
    edit: &breakpoint_regions::SequenceEdit,
    types: &TypeInterner,
) -> Result<(), String> {
    let Some(witness) = &before.ordinary_borrowed_sums else {
        return Ok(());
    };
    validate(before, types)?;
    // The existing finite edit validator checks every block, prefix and redirected
    // edge against the exact original ForEach. The shadow grants no breakpoint proof.
    let mut shadow = function.clone();
    breakpoint_regions::sequence_transition(&mut shadow, before, edit.clone(), types)?;
    if function.locals.len() < before.locals.len()
        || function.locals[..before.locals.len()] != before.locals
        || function.params != before.params
        || function.entry != before.entry
    {
        return Err(
            "ordinary borrowed Handle sequence preparation changed existing headers".into(),
        );
    }
    let mut rows = witness.rows.clone();
    if let Some((block, prefix)) = &edit.prefix {
        for row in &mut rows {
            row.shift(*block, prefix.len());
        }
    }
    transition(function, before, &shadow, rows)
}
