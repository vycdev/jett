//! Typed source acquisitions, separate from physical invocation operands.
//! Every original block is checked before reachability or local compaction.
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use jett_common::Span;
use jett_hir::{self as hir, Expression, ExpressionKind as E, LocalId};
use jett_typecheck::{CheckedCallerEffect, CheckedOwnershipContext};
use jett_types::{Type, TypeId, TypeInterner};

use crate::{BlockId, Function, Program, StatementKind as S, TerminatorKind as T};

#[cfg(test)]
mod tests;

/// A validated source transfer. `binding` is the original caller's binding,
/// not the generated physical slot or a backing parent of a field copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceAcquisition {
    pub binding: Option<LocalId>,
    pub effect: CheckedCallerEffect,
    pub actual_type: TypeId,
    pub storage_type: TypeId,
    pub source_span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgumentAcquisition {
    pub parameter_index: usize,
    pub source_index: usize,
    pub source: SourceAcquisition,
}

/// References identify already-validated occurrences in this immutable MIR.
/// Pointer equality is only a cache lookup, never ownership authority.
#[derive(Debug)]
pub struct CallerAcquisitions<'a> {
    pub(crate) resource_pending: bool,
    owners: BTreeMap<u32, SourceAcquisition>,
    calls: Vec<(&'a Expression, Vec<ArgumentAcquisition>)>,
    operands: Vec<(&'a Expression, LocalId)>,
    // Exact input reads authenticated by a complete converted Source Handle.
    handled_conversions: Vec<(&'a Expression, &'a Expression)>,
    pub(super) absent_successes: Vec<AbsentSuccessPlan>,
    pub(super) present_successes: Vec<PresentSuccessPlan>,
    generation_storage: crate::CallGenerationStoragePlan,
}

impl Default for CallerAcquisitions<'_> {
    fn default() -> Self {
        Self {
            resource_pending: false,
            owners: BTreeMap::new(),
            calls: Vec::new(),
            operands: Vec::new(),
            handled_conversions: Vec::new(),
            absent_successes: Vec::new(),
            present_successes: Vec::new(),
            generation_storage: crate::CallGenerationStoragePlan::empty(),
        }
    }
}
impl CallerAcquisitions<'_> {
    pub(crate) fn generation_storage(&self) -> &crate::CallGenerationStoragePlan {
        &self.generation_storage
    }
    pub fn owner_initializer(&self, owner: LocalId) -> Option<&SourceAcquisition> {
        self.owners.get(&owner.index())
    }

    pub fn arguments(&self, expression: &Expression) -> Option<&[ArgumentAcquisition]> {
        self.calls.iter().find_map(|(call, arguments)| {
            std::ptr::eq(*call, expression).then_some(arguments.as_slice())
        })
    }

    pub fn owner_initializers(&self) -> impl Iterator<Item = (LocalId, &SourceAcquisition)> {
        self.owners
            .iter()
            .map(|(&local, source)| (LocalId::new(local), source))
    }

    /// Readonly occurrence lookup after full Source/CFG/Begin validation.
    /// This does not make an unassociated View initializer owning or copyable.
    pub(crate) fn handled_conversion_input(&self, conversion: &Expression) -> Option<&Expression> {
        self.handled_conversions
            .iter()
            .find_map(|(proved, input)| std::ptr::eq(*proved, conversion).then_some(*input))
    }

    pub fn argument_binding(&self, expression: &Expression) -> Option<LocalId> {
        self.operands
            .iter()
            .find_map(|(operand, binding)| std::ptr::eq(*operand, expression).then_some(*binding))
    }
}

/// Internal original proof; only canonical sum preparation turns it into a
/// retained certificate. It is not part of the public caller-acquisition API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AbsentSuccessPlan {
    pub(super) function: hir::FunctionId,
    pub(super) identity: hir::FunctionIdentity,
    pub(super) source: LocalId,
    pub(super) tag: LocalId,
    pub(super) output: LocalId,
    pub(super) source_type: TypeId,
    pub(super) output_type: TypeId,
    pub(super) span: Span,
    pub(super) selecting: BlockId,
    pub(super) failure: BlockId,
}

/// Only a complete original success-edge proof can be minted by canonical
/// Result(T,Never) preparation. No public packet or Goto creates this authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PresentSuccessPlan {
    pub(super) function: hir::FunctionId,
    pub(super) identity: hir::FunctionIdentity,
    pub(super) source: LocalId,
    pub(super) tag: LocalId,
    pub(super) output: LocalId,
    pub(super) source_type: TypeId,
    pub(super) output_type: TypeId,
    pub(super) span: Span,
    pub(super) selecting: BlockId,
    pub(super) success: BlockId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Site {
    block: BlockId,
    statement: usize,
}

#[derive(Clone, Copy)]
enum Definition<'a> {
    Let(&'a Expression),
    Begin(&'a Expression),
    SumTake { source: LocalId, success: bool },
    Other,
}

#[derive(Clone, Copy)]
struct Defined<'a> {
    site: Site,
    span: Span,
    kind: Definition<'a>,
}

fn restore_converted_observation_snapshot(
    initializer: &Expression,
    argument: &hir::ArgumentOwnership,
    types: &TypeInterner,
) -> Result<Expression, String> {
    if !matches!(initializer.kind, E::InterfaceCoerce { .. })
        || !hir::observation_data_type(types, argument.actual_type)
        || !crate::handlers::can_snapshot_view(types, argument.actual_type)
    {
        return Err(
            "call ownership observation requires its exact ordinary raw snapshot initializer"
                .into(),
        );
    }
    fn restore(
        value: &Expression,
        argument: &hir::ArgumentOwnership,
    ) -> Result<Expression, String> {
        match &value.kind {
            E::InterfaceCoerce {
                value: raw,
                adapters,
            } => Ok(Expression {
                kind: E::InterfaceCoerce {
                    value: Box::new(restore(raw, argument)?),
                    adapters: adapters.clone(),
                },
                ty: value.ty,
                span: value.span,
            }),
            E::Clone(raw)
                if value.ty == argument.actual_type
                    && raw.ty == argument.actual_type
                    && value.span == argument.source_span
                    && raw.span == argument.source_span =>
            {
                Ok(raw.as_ref().clone())
            }
            _ => Err("call ownership observation lost its exact raw snapshot occurrence".into()),
        }
    }
    restore(initializer, argument)
}

/// The integration gate runs this for every original function, including
/// descriptor-only bodies and disconnected blocks. HIR signature projections
/// retain exact IDs/identities/modes; no source names supply target authority.
pub fn validate_function<'a>(
    program: &Program,
    function: &'a Function,
    types: &TypeInterner,
) -> Result<CallerAcquisitions<'a>, String> {
    program.resource_manifest.validate(types)?;
    ProgramValidation::new(program).validate_function(function, types)
}

/// One immutable signature projection for one ownership validation pass.
/// The program borrow prevents reuse across mutation or canonical preparation.
pub(crate) struct ProgramValidation<'p> {
    _program: &'p Program,
    signatures: Vec<hir::Function>,
}

impl<'p> ProgramValidation<'p> {
    pub(crate) fn new(program: &'p Program) -> Self {
        let signatures = program
            .functions
            .iter()
            .map(|function| hir::Function {
                id: function.id,
                identity: function.identity.clone(),
                debug_kind: function.debug_kind.clone(),
                // MIR has already replaced resolver-session IDs with declaration identity.
                // The source certificate and exact dispatcher shape remain authoritative.
                source_definition: None,
                params: function.params.clone(),
                capture_count: function.capture_count,
                return_type: function.return_type,
                locals: function.locals.clone(),
                body: hir::Block {
                    statements: Vec::new(),
                    span: function.span,
                },
                span: function.span,
            })
            .collect::<Vec<_>>();
        Self {
            _program: program,
            signatures,
        }
    }

    pub(crate) fn validate_function<'a>(
        &self,
        function: &'a Function,
        types: &TypeInterner,
    ) -> Result<CallerAcquisitions<'a>, String> {
        validate_function_with_signatures(
            function,
            types,
            &self.signatures,
            &self._program.resource_manifest,
        )
    }
}

fn validate_function_with_signatures<'a>(
    function: &'a Function,
    types: &TypeInterner,
    signatures: &[hir::Function],
    manifest: &hir::ResourceManifest,
) -> Result<CallerAcquisitions<'a>, String> {
    // Fresh exact site proof precedes lexical context selection.
    let breakpoint_sites = crate::breakpoint_regions::validate(function, types)?;
    let mut validator = Validator::new(function, types, signatures, manifest)?;
    let context = if function.debug_kind == hir::FunctionDebugKind::Inline {
        CheckedOwnershipContext::Ordinary
    } else {
        match function.identity.declaration.kind {
            hir::DeclarationKind::Verify => CheckedOwnershipContext::Verify,
            hir::DeclarationKind::Property => CheckedOwnershipContext::Property,
            _ => CheckedOwnershipContext::Ordinary,
        }
    };
    for block in &function.blocks {
        for (statement, value) in block.statements.iter().enumerate() {
            let site = Site {
                block: block.id,
                statement,
            };
            let context = if breakpoint_sites.statement(block.id, statement) {
                CheckedOwnershipContext::BreakpointExpression
            } else {
                context
            };
            validator.statement(&value.kind, site, context)?;
        }
        let site = Site {
            block: block.id,
            statement: block.statements.len(),
        };
        let context = if breakpoint_sites.terminator(block.id) {
            CheckedOwnershipContext::BreakpointExpression
        } else {
            context
        };
        validator.terminator(&block.terminator.kind, site, context)?;
    }
    validator.validate_prepared_absences()?;
    validator.validate_prepared_presences()?;
    validator.acquisitions.absent_successes = validator.absent_successes.into_inner();
    validator.acquisitions.present_successes = validator.present_successes.into_inner();
    validator.acquisitions.generation_storage =
        crate::call_owner_generations::validate(function, types)?;
    Ok(validator.acquisitions)
}

struct Validator<'a, 't, 's> {
    function: &'a Function,
    types: &'t TypeInterner,
    signatures: &'s [hir::Function],
    manifest: &'s hir::ResourceManifest,
    locals: Vec<hir::OwnershipLocalInfo>,
    definitions: BTreeMap<u32, Vec<Defined<'a>>>,
    predecessors: Vec<Vec<BlockId>>,
    stages: BTreeMap<usize, usize>,
    claimed_slots: BTreeSet<u32>,
    acquisitions: CallerAcquisitions<'a>,
    absent_successes: RefCell<Vec<AbsentSuccessPlan>>,
    present_successes: RefCell<Vec<PresentSuccessPlan>>,
    iteration_scopes: Vec<crate::iteration_views::IterationScope>,
    iteration_scope_enabled: bool,
}

impl<'a, 't, 's> Validator<'a, 't, 's> {
    fn new(
        function: &'a Function,
        types: &'t TypeInterner,
        signatures: &'s [hir::Function],
        manifest: &'s hir::ResourceManifest,
    ) -> Result<Self, String> {
        manifest.validate_type(types, function.return_type)?;
        for parameter in &function.params {
            manifest.validate_type(types, parameter.ty)?;
        }
        for local in &function.locals {
            manifest.validate_type(types, local.ty)?;
            manifest.validate_type(types, local.debug_ty)?;
        }
        let iteration_scopes = crate::iteration_views::scopes(function, types)?;
        let locals = function
            .locals
            .iter()
            .map(|local| hir::OwnershipLocalInfo {
                ty: local.ty,
                mutable: local.mutable,
                span: local.span,
                view_source: local.view_source,
                view_iteration: None,
                is_view_parameter: function
                    .parameter_for_local(local.id)
                    .is_some_and(|parameter| parameter.mode == hir::ParamMode::View),
            })
            .collect::<Vec<_>>();
        let stages = crate::call_views::validate(function, types)?;
        let mut definitions = BTreeMap::<u32, Vec<Defined<'a>>>::new();
        let mut predecessors = vec![Vec::new(); function.blocks.len()];
        let mut define = |local: LocalId, site, span, kind| {
            definitions
                .entry(local.index())
                .or_default()
                .push(Defined { site, span, kind });
        };
        for block in &function.blocks {
            if block.id.index() as usize >= function.blocks.len()
                || function.blocks[block.id.index() as usize].id != block.id
            {
                return Err("call ownership block IDs are not dense".into());
            }
            for (index, statement) in block.statements.iter().enumerate() {
                let site = Site {
                    block: block.id,
                    statement: index,
                };
                match &statement.kind {
                    S::Let { local, value } => {
                        define(*local, site, statement.span, Definition::Let(value))
                    }
                    S::BeginCallView { local, value } => {
                        define(*local, site, statement.span, Definition::Begin(value))
                    }
                    S::SumTake {
                        source,
                        target,
                        success,
                    } => define(
                        *target,
                        site,
                        statement.span,
                        Definition::SumTake {
                            source: *source,
                            success: *success,
                        },
                    ),
                    S::CheckRefinement { local, .. }
                    | S::SequenceLength { target: local, .. }
                    | S::SequenceGet { target: local, .. }
                    | S::SumTag { target: local, .. } => {
                        define(*local, site, statement.span, Definition::Other)
                    }
                    S::ReplaceCallOwnerGeneration { root, .. } => {
                        define(*root, site, statement.span, Definition::Other)
                    }
                    S::Assign { target, .. } => {
                        if let Some(local) = address_root(target) {
                            define(local, site, statement.span, Definition::Other);
                        }
                    }
                    _ => {}
                }
            }
            let site = Site {
                block: block.id,
                statement: block.statements.len(),
            };
            match &block.terminator.kind {
                T::Switch { variants, .. } => {
                    for (_, target, bindings) in variants {
                        for &local in bindings {
                            define(
                                local,
                                Site {
                                    block: *target,
                                    statement: 0,
                                },
                                block.terminator.span,
                                Definition::Other,
                            );
                        }
                    }
                }
                T::ForEach {
                    key, value, body, ..
                } => {
                    define(
                        *key,
                        Site {
                            block: *body,
                            statement: 0,
                        },
                        block.terminator.span,
                        Definition::Other,
                    );
                    if let Some(value) = value {
                        define(
                            *value,
                            Site {
                                block: *body,
                                statement: 0,
                            },
                            block.terminator.span,
                            Definition::Other,
                        );
                    }
                }
                _ => {
                    let _ = site;
                }
            }
            for target in successors(&block.terminator.kind) {
                let incoming = predecessors
                    .get_mut(target.index() as usize)
                    .ok_or("call ownership CFG target is outside its function")?;
                incoming.push(block.id);
            }
        }
        for (id, local) in function.locals.iter().enumerate() {
            if local.id.index() as usize != id
                || !valid_type(types, local.ty)
                || !valid_type(types, local.debug_ty)
            {
                return Err(
                    "call ownership local metadata is outside its function or interner".into(),
                );
            }
        }
        for &local in definitions.keys() {
            if function.local(LocalId::new(local)).is_none() {
                return Err("call ownership definition is outside its local table".into());
            }
        }
        Ok(Self {
            function,
            types,
            signatures,
            manifest,
            locals,
            definitions,
            predecessors,
            stages,
            claimed_slots: BTreeSet::new(),
            acquisitions: CallerAcquisitions::default(),
            absent_successes: RefCell::new(Vec::new()),
            present_successes: RefCell::new(Vec::new()),
            iteration_scopes,
            iteration_scope_enabled: true,
        })
    }

    fn invocation(
        &mut self,
        expression: &'a Expression,
        site: Site,
        context: CheckedOwnershipContext,
    ) -> Result<(), String> {
        if let E::ResourceInvoke {
            hook,
            ownership,
            args,
            evaluation_order,
            ..
        } = &expression.kind
        {
            if !self.manifest.contains_hook(hook) {
                return Err(
                    "Resource invocation belongs to another original checked manifest".into(),
                );
            }
            let locals = self.locals_at(site);
            hir::validate_hir_invocation(
                self.signatures,
                &locals,
                expression,
                self.types,
                context,
            )?;
            let hir::CallOwnership::Source(source) = ownership else {
                return Err("Resource invocation cannot use Generated authority".into());
            };
            if source.arguments.len() != args.len() || evaluation_order.len() != args.len() {
                return Err("Resource invocation source order or operand count changed".into());
            }
            self.acquisitions.resource_pending = true;
            return Ok(());
        }
        let (ownership, args, order) = match &expression.kind {
            E::Call {
                ownership,
                args,
                evaluation_order,
                ..
            }
            | E::Intrinsic {
                ownership,
                args,
                evaluation_order,
                ..
            }
            | E::IndirectCall {
                ownership,
                args,
                evaluation_order,
                ..
            } => (ownership, args, evaluation_order),
            _ => return Ok(()),
        };
        if self.iteration_scope_enabled {
            let ensure = |source: hir::CallerViewSource| {
                if let hir::CallerViewSource::Local(local) = source {
                    crate::iteration_views::ensure_scope(
                        self.function,
                        &self.iteration_scopes,
                        local,
                        site.block,
                    )?;
                }
                Ok::<_, String>(())
            };
            match ownership {
                hir::CallOwnership::Source(source) => {
                    for argument in &source.arguments {
                        match argument.origin {
                            hir::CallerOrigin::Binding(ref fact) => {
                                ensure(hir::CallerViewSource::Local(fact.local))?
                            }
                            hir::CallerOrigin::BorrowedProjection { source }
                            | hir::CallerOrigin::OwnedFieldCopy { parent: source } => {
                                ensure(source)?
                            }
                            hir::CallerOrigin::OwnedExpression => {}
                        }
                    }
                    for argument in &source.generated_operands {
                        ensure(argument.original_witness().origin())?;
                    }
                }
                hir::CallOwnership::Generated(generated) => {
                    for argument in &generated.arguments {
                        ensure(argument.original_witness().origin())?;
                    }
                }
            }
        }
        let local_info = self.locals_at(site);
        hir::validate_operand_ownership(
            ownership,
            &args.iter().map(|value| value.ty).collect::<Vec<_>>(),
            order,
            self.types,
            |local| local_info.get(local.index() as usize).copied(),
            false,
        )?;
        hir::validate_invocation_target(self.signatures, expression, self.types, context)?;
        let resource_original = crate::resource_ownership::original_call_at(
            self.function,
            self.manifest,
            self.types,
            expression,
            site.block,
            site.statement,
        )?;
        if resource_original {
            self.acquisitions.resource_pending = true;
        }
        match ownership {
            hir::CallOwnership::Source(source) => {
                let mut original = Vec::new();
                for (index, argument) in source.arguments.iter().enumerate() {
                    let value = &args[index];
                    let acquisition = self.source_acquisition(argument, value.ty);
                    match argument.staging {
                        hir::ArgumentStaging::Original => {
                            let resource_actual = resource_original
                                && crate::resource_ownership::original_resource_actual(
                                    self.manifest,
                                    self.types,
                                    argument.actual_type,
                                );
                            if (argument.effect == CheckedCallerEffect::RelinquishOwned
                                && !resource_actual)
                                || (argument.effect == CheckedCallerEffect::ObserveData
                                    && argument.physical_access
                                        == jett_typecheck::CheckedCalleeAccess::Owned)
                            {
                                return Err("call ownership requires explicit owning staging at this physical boundary".into());
                            }
                            self.original(value, argument, &source.bridge, site)?;
                            if consumes(argument.effect) && !resource_actual {
                                if let Some(binding) = acquisition.binding {
                                    self.acquisitions.operands.push((value, binding));
                                }
                                original.push(ArgumentAcquisition {
                                    parameter_index: index,
                                    source_index: argument.source_index,
                                    source: acquisition,
                                });
                            }
                        }
                        hir::ArgumentStaging::Transferred { owner } => {
                            self.slot_operand(value, owner, false)?;
                            let (initializer, defined) = self.owning_let(owner, value.ty, site)?;
                            self.original(initializer, argument, &source.bridge, defined.site)?;
                            self.claim_owner(owner, acquisition)?;
                            if !jett_typecheck::ownership::is_implicitly_copyable(
                                self.types,
                                argument.actual_type,
                            ) {
                                self.acquisitions.operands.push((value, owner));
                            }
                        }
                        hir::ArgumentStaging::Copied { value: copied } => {
                            self.slot_operand(value, copied, matches!(value.kind, E::View(_)))?;
                            let (initializer, defined) =
                                self.storage_let(copied, value.ty, site, true)?;
                            if let Err(error) =
                                self.original(initializer, argument, &source.bridge, defined.site)
                            {
                                let E::Clone(inner) = &initializer.kind else {
                                    return Err(error);
                                };
                                if inner.ty != initializer.ty
                                    || !jett_typecheck::ownership::is_implicitly_copyable(
                                        self.types,
                                        argument.actual_type,
                                    )
                                {
                                    return Err(error);
                                }
                                self.original(inner, argument, &source.bridge, defined.site)?;
                            }
                            self.claim_slot(copied)?;
                        }
                        hir::ArgumentStaging::Observed { owner } => {
                            self.slot_operand(value, owner, false)?;
                            let (initializer, defined) = self.owning_let(owner, value.ty, site)?;
                            if argument.observation_proof().is_none() {
                                return Err("call ownership observation lost its checked data/capture proof".into());
                            }
                            if let E::Clone(observed) = &initializer.kind {
                                if observed.ty != initializer.ty
                                    || !crate::handlers::can_snapshot_view(
                                        self.types,
                                        initializer.ty,
                                    )
                                {
                                    return Err("call ownership observation cannot snapshot an erased or unsupported value".into());
                                }
                                self.original(observed, argument, &source.bridge, defined.site)?;
                            } else {
                                let restored = restore_converted_observation_snapshot(
                                    initializer,
                                    argument,
                                    self.types,
                                )?;
                                self.original(&restored, argument, &source.bridge, defined.site)?;
                            }
                            self.claim_slot(owner)?;
                        }
                        hir::ArgumentStaging::Relinquished { owner, loan } => {
                            self.slot_operand(value, loan, true)?;
                            let (projection, begun) = self.loan(loan, value.ty, site)?;
                            self.slot_operand(projection, owner, true)?;
                            if self
                                .function
                                .local(loan)
                                .and_then(|local| local.view_source)
                                != Some(owner)
                            {
                                return Err(
                                    "call ownership loan does not borrow the acquired owner".into(),
                                );
                            }
                            let (initializer, defined) =
                                self.owning_let(owner, value.ty, begun.site)?;
                            // The scoped loan proves physical View access separately.
                            // Rejoin the complete owning initializer without inventing
                            // a wrapper around its original typed conversion spine.
                            self.original(initializer, argument, &source.bridge, defined.site)?;
                            self.claim_slot(loan)?;
                            self.claim_owner(owner, acquisition)?;
                        }
                        hir::ArgumentStaging::RetainedSnapshot { owner, loan } => {
                            self.slot_operand(value, loan, true)?;
                            let (projection, begun) = self.loan(loan, value.ty, site)?;
                            self.slot_operand(projection, owner, true)?;
                            if self
                                .function
                                .local(loan)
                                .and_then(|local| local.view_source)
                                != Some(owner)
                            {
                                return Err(
                                    "retained snapshot loan does not borrow its exact owner".into(),
                                );
                            }
                            let (initializer, defined) =
                                self.owning_let(owner, value.ty, begun.site)?;
                            let E::Clone(endpoint) = &initializer.kind else {
                                return Err(
                                    "retained snapshot requires a full-endpoint Clone initializer"
                                        .into(),
                                );
                            };
                            let physical_span = argument
                                .retained_snapshot_span()
                                .ok_or("retained snapshot has no checked physical occurrence")?;
                            // The initial physical View may have a pipeline-step span.
                            // Its child still rejoins the unchanged exact raw Source span.
                            let E::View(physical_loan) = &value.kind else {
                                unreachable!("slot_operand proved View");
                            };
                            let E::View(physical_owner) = &projection.kind else {
                                unreachable!("slot_operand proved View");
                            };
                            if argument.retained_snapshot_type() != Some(initializer.ty)
                                || endpoint.ty != initializer.ty
                                || initializer.span != physical_span
                                || endpoint.span != physical_span
                                || value.span != physical_span
                                || physical_loan.span != physical_span
                                || projection.span != physical_span
                                || physical_owner.span != physical_span
                                || !crate::handlers::can_snapshot_view(self.types, initializer.ty)
                            {
                                return Err("retained snapshot changes its checked endpoint type or occurrence".into());
                            }
                            self.original(endpoint, argument, &source.bridge, defined.site)?;
                            self.retained_snapshot_order(source, argument, defined.site)?;
                            self.claim_slot(owner)?;
                            self.claim_slot(loan)?;
                        }
                        hir::ArgumentStaging::Borrowed { loan } => {
                            self.slot_operand(value, loan, true)?;
                            let (projection, begun) = self.loan(loan, value.ty, site)?;
                            let restored = self
                                .restore_temporary_projection(projection, argument, begun.site)?;
                            if let Err(error) =
                                self.original(&restored, argument, &source.bridge, begun.site)
                            {
                                if !self.original_handled_view_initializer(
                                    value,
                                    projection,
                                    argument,
                                    &source.bridge,
                                    begun.site,
                                )? {
                                    return Err(error);
                                }
                            } else if hir::validate_source_handled_operand(
                                &restored,
                                argument,
                                &source.bridge,
                                self.types,
                            )?
                            .is_some()
                            {
                                self.handled_view_storage_headers(
                                    value, projection, argument, begun.site,
                                )?;
                            }
                            self.claim_slot(loan)?;
                        }
                    }
                }
                self.acquisitions.calls.push((expression, original));
                for (tail, argument) in source.generated_operands.iter().enumerate() {
                    let value = &args[source.arguments.len() + tail];
                    let (metadata, at) = match value.kind {
                        E::Local(local) => {
                            let (initializer, defined) = self.owning_let(local, value.ty, site)?;
                            self.claim_slot(local)?;
                            (initializer, defined.site)
                        }
                        _ => (value, site),
                    };
                    self.generated_original(metadata, argument, at)?;
                    hir::validate_compiler_metadata_operand(metadata, source, self.types)?;
                }
            }
            hir::CallOwnership::Generated(generated) => {
                for (value, argument) in args.iter().zip(&generated.arguments) {
                    self.generated_operand(value, argument, site)?;
                }
            }
        }
        Ok(())
    }

    fn retained_snapshot_order(
        &self,
        source: &hir::SourceCallOwnership,
        argument: &hir::ArgumentOwnership,
        defined: Site,
    ) -> Result<(), String> {
        for later in &source.arguments {
            if later.source_index <= argument.source_index {
                continue;
            }
            let within = |span: Span| {
                span.file == later.source_span.file
                    && span.start < span.end
                    && later.source_span.start <= span.start
                    && span.end <= later.source_span.end
            };
            for block in &self.function.blocks {
                for (statement, value) in block.statements.iter().enumerate() {
                    if within(value.span) {
                        self.dominates(
                            defined,
                            Site {
                                block: block.id,
                                statement,
                            },
                        )?;
                    }
                }
                if within(block.terminator.span) {
                    self.dominates(
                        defined,
                        Site {
                            block: block.id,
                            statement: block.statements.len(),
                        },
                    )?;
                }
            }
        }
        Ok(())
    }

    fn locals_at(&self, site: Site) -> Vec<hir::OwnershipLocalInfo> {
        let mut locals = self.locals.clone();
        if self.iteration_scope_enabled {
            crate::iteration_views::apply(
                &self.iteration_scopes,
                site.block,
                &mut locals,
                self.types,
            );
        }
        locals
    }

    fn source_acquisition(
        &self,
        argument: &hir::ArgumentOwnership,
        storage_type: TypeId,
    ) -> SourceAcquisition {
        let binding = match &argument.origin {
            hir::CallerOrigin::Binding(fact)
                if matches!(fact.mode, hir::CallerBindingMode::Owned)
                    && !jett_typecheck::ownership::is_implicitly_copyable(self.types, fact.ty) =>
            {
                Some(fact.local)
            }
            _ => None,
        };
        SourceAcquisition {
            binding,
            effect: argument.effect,
            actual_type: argument.actual_type,
            storage_type,
            source_span: argument.source_span,
        }
    }

    fn claim_slot(&mut self, local: LocalId) -> Result<(), String> {
        if !self.claimed_slots.insert(local.index()) {
            return Err(
                "call ownership staging slot belongs to more than one source acquisition".into(),
            );
        }
        Ok(())
    }

    fn claim_owner(&mut self, owner: LocalId, source: SourceAcquisition) -> Result<(), String> {
        self.claim_slot(owner)?;
        if source.binding == Some(owner) {
            return Err(
                "call ownership acquired owner cannot replace its original binding in place".into(),
            );
        }
        self.acquisitions.owners.insert(owner.index(), source);
        Ok(())
    }

    fn slot_operand(
        &self,
        expression: &Expression,
        local: LocalId,
        borrowed: bool,
    ) -> Result<(), String> {
        let value = if borrowed {
            let E::View(value) = &expression.kind else {
                return Err("call ownership staged loan requires physical View(Local)".into());
            };
            if value.ty != expression.ty {
                return Err("call ownership staged view changes its slot type".into());
            }
            value.as_ref()
        } else {
            expression
        };
        if !matches!(value.kind, E::Local(id) if id == local)
            || self
                .function
                .local(local)
                .is_none_or(|metadata| metadata.ty != value.ty)
        {
            return Err(
                "call ownership staged operand does not identify its exact storage slot".into(),
            );
        }
        Ok(())
    }

    fn owning_let(
        &self,
        local: LocalId,
        ty: TypeId,
        used: Site,
    ) -> Result<(&'a Expression, Defined<'a>), String> {
        self.storage_let(local, ty, used, false)
    }

    fn storage_let(
        &self,
        local: LocalId,
        ty: TypeId,
        used: Site,
        copy_view: bool,
    ) -> Result<(&'a Expression, Defined<'a>), String> {
        let metadata = self
            .function
            .local(local)
            .ok_or("call ownership owner slot is absent")?;
        if metadata.ty != ty
            || metadata.debug_ty != ty
            || metadata.view_source.is_some()
            || self.function.parameter_for_local(local).is_some()
        {
            return Err("call ownership acquisition requires an exact new owning slot".into());
        }
        let [defined] = self
            .definitions
            .get(&local.index())
            .map(Vec::as_slice)
            .unwrap_or(&[])
        else {
            return Err("call ownership acquired slot requires one unique initializer".into());
        };
        let Definition::Let(value) = defined.kind else {
            return Err("call ownership acquired slot is not an owning Let".into());
        };
        if value.ty != ty
            || metadata.span != value.span
            || (!copy_view && matches!(value.kind, E::View(_)))
        {
            return Err(
                "call ownership owner initializer changes type, occurrence or ownership".into(),
            );
        }
        self.dominates(defined.site, used)?;
        Ok((value, *defined))
    }

    fn loan(
        &self,
        loan: LocalId,
        ty: TypeId,
        used: Site,
    ) -> Result<(&'a Expression, Defined<'a>), String> {
        if !self.stages.contains_key(&(loan.index() as usize)) {
            return Err("call ownership staged borrow has no typed Begin/End authority".into());
        }
        let [defined] = self
            .definitions
            .get(&loan.index())
            .map(Vec::as_slice)
            .unwrap_or(&[])
        else {
            return Err("call ownership loan requires one unique Begin".into());
        };
        let Definition::Begin(value) = defined.kind else {
            return Err("call ownership loan initializer is not BeginCallView".into());
        };
        if value.ty != ty {
            return Err("call ownership loan initializer changes its endpoint type".into());
        }
        self.dominates(defined.site, used)?;
        Ok((value, *defined))
    }

    fn original(
        &self,
        value: &Expression,
        argument: &hir::ArgumentOwnership,
        bridge: &hir::CallBridge,
        site: Site,
    ) -> Result<(), String> {
        let local_info = self.locals_at(site);
        match hir::validate_source_operand(value, argument, bridge, &local_info, self.types) {
            Ok(_) => Ok(()),
            Err(error) => {
                // A CFG result is not a source binding and never gets a fabricated
                // Binding witness. Only a closed original producer may use this path.
                if argument.origin != hir::CallerOrigin::OwnedExpression {
                    return Err(error);
                }
                let original = match hir::validate_source_handled_operand(
                    value, argument, bridge, self.types,
                )? {
                    Some(backing) => backing,
                    None => {
                        let mut original = value;
                        while let E::View(inner) = &original.kind {
                            original = inner;
                        }
                        let occurrence_type = argument.source_witness().occurrence_type();
                        if original.ty != occurrence_type || original.span != argument.source_span {
                            return Err(error);
                        }
                        original
                    }
                };
                let E::Local(local) = original.kind else {
                    return Err(error);
                };
                // The sealed physical occurrence may carry an expected Secret
                // layer. Raw actual_type still controls caller acquisition.
                self.produced_result(local, original.ty, original.span, site)
            }
        }
    }

    fn produced_result(
        &self,
        local: LocalId,
        ty: TypeId,
        span: Span,
        used: Site,
    ) -> Result<(), String> {
        let metadata = self
            .function
            .local(local)
            .ok_or("call ownership produced result is absent")?;
        if metadata.ty != ty
            || metadata.debug_ty != ty
            || metadata.span != span
            || metadata.view_source.is_some()
            || self.function.parameter_for_local(local).is_some()
        {
            return Err("call ownership producer result has no exact owned metadata".into());
        }
        let definitions = self
            .definitions
            .get(&local.index())
            .ok_or("call ownership producer result has no definitions")?;
        let sum_anchors = definitions
            .iter()
            .filter_map(|defined| match defined.kind {
                Definition::SumTake {
                    source,
                    success: true,
                } if defined.span == span => Some(source),
                _ => None,
            })
            .collect::<Vec<_>>();
        let prepared = self
            .function
            .prepared_absent_successes
            .iter()
            .find(|record| record.plan().output == local);
        if let Some(record) = prepared {
            if record.plan().span != span || record.plan().output_type != ty {
                return Err(
                    "call ownership prepared absence differs from its exact output occurrence"
                        .into(),
                );
            }
            self.prepared_absence(record.plan())?;
        }
        let default_source = match sum_anchors.as_slice() {
            [source] => Some(*source),
            [] => prepared.map(|record| record.plan().source),
            _ => None,
        };
        let present = self
            .function
            .prepared_present_successes
            .iter()
            .find(|record| record.plan().output == local);
        if let Some(record) = present {
            if record.plan().span != span || record.plan().output_type != ty {
                return Err(
                    "call ownership prepared success differs from its exact output occurrence"
                        .into(),
                );
            }
            self.prepared_presence(record.plan())?;
        }
        let mut absent_plan = None;
        let mut present_plan = None;
        let mut producer = prepared.is_some();
        for defined in definitions {
            match defined.kind {
                Definition::SumTake {
                    source,
                    success: true,
                } => {
                    if defined.span != span {
                        return Err("call ownership producer result has definitions from another source occurrence".into());
                    }
                    let source_ty = self
                        .function
                        .local(source)
                        .ok_or("call ownership sum source is absent")?
                        .ty;
                    let payload = match self.types.resolve(source_ty) {
                        Type::Optional(inner) | Type::Result(inner, _) => *inner,
                        _ => {
                            return Err(
                                "call ownership produced result is not a typed sum payload".into(),
                            );
                        }
                    };
                    if payload == TypeInterner::NEVER {
                        absent_plan = Some(self.original_absence(
                            source,
                            local,
                            source_ty,
                            ty,
                            span,
                            defined.site.block,
                        )?);
                    } else if payload != ty
                        && !matches!(self.types.resolve(ty), Type::Secret(inner) if *inner == payload)
                    {
                        return Err(
                            "call ownership produced result differs from its sum payload".into(),
                        );
                    }
                    self.producer_source(source, source_ty, span, defined.site)?;
                    self.sum_success_edge(source, defined.site.block)?;
                    if present.is_none() && self.present_source_payload(source_ty).is_some() {
                        present_plan = Some(self.original_presence(
                            source,
                            local,
                            source_ty,
                            ty,
                            span,
                            defined.site.block,
                        )?);
                    }
                    producer = true;
                }
                Definition::Let(value) if value.ty == ty && !matches!(value.kind, E::View(_)) => {
                    if let Some(source) = default_source {
                        if !contains(span, defined.span) || !contains(defined.span, value.span) {
                            return Err("call ownership handled default is outside its exact source occurrence".into());
                        }
                        self.sum_failure_region(source, defined.site.block, span)?;
                        if self.handled_default_is_borrowed(value, defined.site) {
                            return Err(
                                "call ownership handled default cannot acquire a borrowed owner"
                                    .into(),
                            );
                        }
                    } else if defined.span != span {
                        return Err("call ownership producer result has definitions from another source occurrence".into());
                    }
                    if let E::RefinementValidated(inner) = &value.kind {
                        let E::Local(source) = inner.kind else {
                            return Err(
                                "call ownership validated result has no owned candidate".into()
                            );
                        };
                        self.producer_source(source, inner.ty, span, defined.site)?;
                        if !matches!(self.types.resolve(ty), Type::Refinement { .. }) {
                            return Err(
                                "call ownership validated result has no nominal refinement target"
                                    .into(),
                            );
                        }
                        producer = true;
                    } else if !matches!(value.kind, E::Local(_) | E::Field { .. }) {
                        // Typed default/constructor definitions do not take a
                        // borrowed owner. The normal value/CFG verifier still
                        // checks their complete body and initialization edges.
                        producer = true;
                    } else if let E::Local(source) = value.kind {
                        if self.function.is_view_local(source) {
                            return Err(
                                "call ownership producer default cannot acquire a borrowed local"
                                    .into(),
                            );
                        }
                    }
                }
                _ => {
                    return Err(
                        "call ownership producer result contains an unsupported definition".into(),
                    );
                }
            }
        }
        if !producer {
            return Err(
                "call ownership result is only a renamed binding, not a proved producer".into(),
            );
        }
        self.definitions_reach(definitions, used)?;
        if let Some(plan) = absent_plan {
            let mut plans = self.absent_successes.borrow_mut();
            if let Some(existing) = plans.iter().find(|existing| existing.output == plan.output) {
                if existing != &plan {
                    return Err(
                        "call ownership absent output has contradictory original proofs".into(),
                    );
                }
            } else {
                plans.push(plan);
            }
        }
        if let Some(plan) = present_plan {
            let mut plans = self.present_successes.borrow_mut();
            if let Some(existing) = plans.iter().find(|existing| existing.output == plan.output) {
                if existing != &plan {
                    return Err(
                        "call ownership present output has contradictory original proofs".into(),
                    );
                }
            } else {
                plans.push(plan);
            }
        }
        Ok(())
    }

    fn restore_temporary_projection(
        &self,
        projection: &Expression,
        argument: &hir::ArgumentOwnership,
        used: Site,
    ) -> Result<Expression, String> {
        let temporary = matches!(
            argument.origin,
            hir::CallerOrigin::BorrowedProjection {
                source: hir::CallerViewSource::Other
            }
        ) || (argument.origin == hir::CallerOrigin::OwnedExpression
            && argument.source_witness().syntax()
                == jett_typecheck::CheckedCallerSyntax::WrittenView);
        if !temporary {
            return Ok(projection.clone());
        }
        let root = address_root(projection)
            .ok_or("call ownership temporary projection has no materialized root")?;
        let ty = self
            .function
            .local(root)
            .ok_or("call ownership temporary projection root is absent")?
            .ty;
        let (initializer, _) = self.owning_let(root, ty, used)?;
        crate::call_views::replace_borrowed_root(projection, initializer.clone())
            .ok_or_else(|| "call ownership temporary projection lost its exact root type".into())
    }

    /// Rejoin only the exact converted original Handle after its owning storage
    /// and Begin have been proved. The added physical View is not a source cast.
    fn original_handled_view_initializer(
        &mut self,
        value: &Expression,
        projection: &Expression,
        argument: &hir::ArgumentOwnership,
        bridge: &hir::CallBridge,
        used: Site,
    ) -> Result<bool, String> {
        use jett_typecheck::{CheckedCalleeAccess as Access, CheckedCallerSyntax as Syntax};
        if argument.origin != hir::CallerOrigin::OwnedExpression
            || argument.syntax != Syntax::WrittenView
            || argument.effect != CheckedCallerEffect::RetainBorrow
            || argument.physical_access != Access::View
        {
            return Ok(false);
        }
        let E::View(_physical_loan) = &value.kind else {
            return Ok(false);
        };
        let E::View(physical_owner) = &projection.kind else {
            return Ok(false);
        };
        let E::Local(owner) = physical_owner.kind else {
            return Ok(false);
        };
        let (initializer, defined) = self.owning_let(owner, projection.ty, used)?;
        if !matches!(
            initializer.kind,
            E::InterfaceCoerce { .. } | E::FunctionAdapter { .. }
        ) {
            return Ok(false);
        }
        let Some(backing) =
            hir::validate_source_handled_operand(initializer, argument, bridge, self.types)?
        else {
            return Ok(false);
        };
        if !matches!(backing.kind, E::Local(_)) {
            return Ok(false);
        }
        self.handled_view_storage_headers(value, projection, argument, used)?;
        self.original(initializer, argument, bridge, defined.site)?;
        // The existing boxing operation reads its concrete payload borrowed and
        // returns an independent owned box. Preserve the original written View;
        // only this exact authenticated conversion/input pair models that read.
        if let E::InterfaceCoerce { value: input, .. } = &initializer.kind
            && matches!(self.types.resolve(initializer.ty), Type::Interface(_))
            && !crate::move_values::is_erased_interface(self.types, input.ty)
            && matches!(&input.kind, E::View(raw) if std::ptr::eq(raw.as_ref(), backing))
        {
            self.acquisitions
                .handled_conversions
                .push((initializer, input.as_ref()));
        }
        Ok(true)
    }

    fn handled_view_storage_headers(
        &self,
        value: &Expression,
        projection: &Expression,
        argument: &hir::ArgumentOwnership,
        used: Site,
    ) -> Result<(), String> {
        use jett_typecheck::{CheckedCalleeAccess as Access, CheckedCallerSyntax as Syntax};
        if argument.origin != hir::CallerOrigin::OwnedExpression
            || argument.syntax != Syntax::WrittenView
            || argument.effect != CheckedCallerEffect::RetainBorrow
            || argument.physical_access != Access::View
        {
            return Ok(());
        }
        let E::View(physical_loan) = &value.kind else {
            return Err("call ownership handled view lost its physical loan".into());
        };
        let E::View(physical_owner) = &projection.kind else {
            return Err("call ownership handled view lost its physical owner".into());
        };
        let E::Local(owner) = physical_owner.kind else {
            return Err("call ownership handled view has no materialized owner".into());
        };
        let (initializer, _) = self.owning_let(owner, projection.ty, used)?;
        if value.span != argument.source_span
            || physical_loan.span != argument.source_span
            || projection.span != argument.source_span
            || physical_owner.span != initializer.span
        {
            return Err("call ownership handled view changes its exact physical occurrence".into());
        }
        Ok(())
    }

    fn producer_source(
        &self,
        source: LocalId,
        ty: TypeId,
        span: Span,
        used: Site,
    ) -> Result<(), String> {
        let metadata = self
            .function
            .local(source)
            .ok_or("call ownership producer source is absent")?;
        if metadata.ty != ty
            || metadata.debug_ty != ty
            || metadata.span != span
            || metadata.view_source.is_some()
            || self.function.parameter_for_local(source).is_some()
        {
            return Err("call ownership producer source has no exact new owned slot".into());
        }
        let [defined] = self
            .definitions
            .get(&source.index())
            .map(Vec::as_slice)
            .unwrap_or(&[])
        else {
            return Err("call ownership producer source requires one owning initializer".into());
        };
        let Definition::Let(value) = defined.kind else {
            return Err("call ownership producer source is not an owning Let".into());
        };
        if value.ty != ty
            || defined.span != span
            || !contains(span, value.span)
            || matches!(value.kind, E::View(_))
        {
            return Err(
                "call ownership producer source changes its type, source occurrence or ownership"
                    .into(),
            );
        }
        self.dominates(defined.site, used)
    }

    fn sum_success_edge(&self, source: LocalId, success: BlockId) -> Result<(), String> {
        let incoming = self
            .predecessors
            .get(success.index() as usize)
            .ok_or("call ownership sum success block is absent")?;
        if incoming.is_empty() {
            return Err("call ownership sum payload has no selecting branch".into());
        }
        for &previous in incoming {
            let block = self
                .function
                .blocks
                .get(previous.index() as usize)
                .ok_or("call ownership sum selecting block is absent")?;
            if let Some(record) = self
                .function
                .prepared_present_successes
                .iter()
                .find(|record| {
                    let plan = record.plan();
                    plan.source == source && plan.success == success && plan.selecting == previous
                })
            {
                self.prepared_presence(record.plan())?;
                continue;
            }
            let T::Branch {
                condition,
                then_block,
                else_block,
            } = &block.terminator.kind
            else {
                return Err("call ownership sum payload is not selected by its tag".into());
            };
            let E::Local(tag) = condition.kind else {
                return Err("call ownership sum selector is not a typed tag local".into());
            };
            if *then_block != success
                || *else_block == success
                || condition.ty != TypeInterner::BOOL
            {
                return Err(
                    "call ownership sum success edge does not preserve tag ordering".into(),
                );
            }
            let [defined] = self
                .definitions
                .get(&tag.index())
                .map(Vec::as_slice)
                .unwrap_or(&[])
            else {
                return Err("call ownership sum selector requires a unique tag definition".into());
            };
            let statement = self
                .function
                .blocks
                .get(defined.site.block.index() as usize)
                .and_then(|block| block.statements.get(defined.site.statement))
                .ok_or("call ownership sum tag definition is absent")?;
            if !matches!(statement.kind, S::SumTag { source: owner, target } if owner == source && target == tag)
            {
                return Err("call ownership sum selector belongs to another owner".into());
            }
            self.dominates(
                defined.site,
                Site {
                    block: previous,
                    statement: block.statements.len(),
                },
            )?;
        }
        Ok(())
    }

    fn present_source_payload(&self, ty: TypeId) -> Option<TypeId> {
        if !valid_type(self.types, ty) {
            return None;
        }
        match self.types.resolve(ty) {
            Type::Result(ok, error)
                if *ok != TypeInterner::NEVER
                    && *ok != TypeInterner::ERROR
                    && *error == TypeInterner::NEVER
                    && valid_type(self.types, *ok) =>
            {
                Some(*ok)
            }
            _ => None,
        }
    }

    fn original_presence(
        &self,
        source: LocalId,
        output: LocalId,
        source_type: TypeId,
        output_type: TypeId,
        span: Span,
        success: BlockId,
    ) -> Result<PresentSuccessPlan, String> {
        if self.present_source_payload(source_type).is_none() {
            return Err(
                "call ownership present success requires an exact Result(T,Never) source".into(),
            );
        }
        let definitions = self
            .definitions
            .get(&output.index())
            .ok_or("call ownership present output is undefined")?;
        if definitions
            .iter()
            .filter(|defined| {
                matches!(defined.kind,
            Definition::SumTake { source: owner, success: true } if owner == source)
                    && defined.span == span
                    && defined.site.block == success
            })
            .count()
            != 1
        {
            return Err("call ownership present output requires one exact success anchor".into());
        }
        let [selecting] = self
            .predecessors
            .get(success.index() as usize)
            .map(Vec::as_slice)
            .unwrap_or(&[])
        else {
            return Err(
                "call ownership present success requires one original selecting branch".into(),
            );
        };
        let block = &self.function.blocks[selecting.index() as usize];
        let T::Branch {
            condition,
            then_block,
            else_block,
        } = &block.terminator.kind
        else {
            return Err("call ownership present success has no original branch".into());
        };
        let E::Local(tag) = condition.kind else {
            return Err("call ownership present selector is not a tag local".into());
        };
        if *then_block != success
            || *else_block == success
            || condition.ty != TypeInterner::BOOL
            || condition.span != span
            || block.terminator.span != span
        {
            return Err("call ownership present selector differs from its exact Handle".into());
        }
        let plan = PresentSuccessPlan {
            function: self.function.id,
            identity: self.function.identity.clone(),
            source,
            tag,
            output,
            source_type,
            output_type,
            span,
            selecting: *selecting,
            success,
        };
        self.presence_tag(&plan)?;
        Ok(plan)
    }

    fn presence_tag(&self, plan: &PresentSuccessPlan) -> Result<(), String> {
        let payload = self
            .present_source_payload(plan.source_type)
            .ok_or("call ownership prepared success has no exact Result(T,Never) type")?;
        if plan.function != self.function.id
            || plan.identity != self.function.identity
            || !valid_type(self.types, plan.output_type)
            || (plan.output_type != payload
                && !matches!(self.types.resolve(plan.output_type), Type::Secret(inner) if *inner == payload))
        {
            return Err(
                "call ownership prepared success has foreign function or payload type authority"
                    .into(),
            );
        }
        let source = self
            .function
            .local(plan.source)
            .ok_or("call ownership present source is outside its function")?;
        let output = self
            .function
            .local(plan.output)
            .ok_or("call ownership present output is outside its function")?;
        let tag = self
            .function
            .local(plan.tag)
            .ok_or("call ownership present tag is outside its function")?;
        if source.ty != plan.source_type
            || source.debug_ty != plan.source_type
            || output.ty != plan.output_type
            || output.debug_ty != plan.output_type
            || output.span != plan.span
            || output.view_source.is_some()
            || self.function.parameter_for_local(plan.output).is_some()
            || tag.ty != TypeInterner::BOOL
            || tag.debug_ty != TypeInterner::BOOL
            || tag.span != plan.span
            || tag.view_source.is_some()
            || self.function.parameter_for_local(plan.tag).is_some()
        {
            return Err(
                "call ownership present certificate changed its exact local metadata".into(),
            );
        }
        let block = self
            .function
            .blocks
            .get(plan.selecting.index() as usize)
            .ok_or("call ownership present selecting block is outside its function")?;
        if self
            .function
            .blocks
            .get(plan.success.index() as usize)
            .is_none()
            || plan.selecting == plan.success
            || block.terminator.span != plan.span
        {
            return Err("call ownership present success edge is outside its exact Handle".into());
        }
        let Some(statement) = block.statements.last() else {
            return Err("call ownership present edge lost its runtime tag".into());
        };
        if statement.span != plan.span
            || !matches!(statement.kind,
            S::SumTag { source, target } if source == plan.source && target == plan.tag)
        {
            return Err("call ownership present edge lost its exact runtime SumTag".into());
        }
        let [definition] = self
            .definitions
            .get(&plan.tag.index())
            .map(Vec::as_slice)
            .unwrap_or(&[])
        else {
            return Err("call ownership present edge has an ambiguous tag definition".into());
        };
        if definition.site
            != (Site {
                block: plan.selecting,
                statement: block.statements.len() - 1,
            })
        {
            return Err("call ownership present tag belongs to another selecting block".into());
        }
        self.producer_source(plan.source, plan.source_type, plan.span, definition.site)
    }

    fn prepared_presence(&self, plan: &PresentSuccessPlan) -> Result<(), String> {
        self.presence_tag(plan)?;
        if !matches!(self.function.blocks[plan.selecting.index() as usize].terminator.kind,
            T::Goto(target) if target == plan.success)
        {
            return Err(
                "call ownership prepared success changed its canonical selected edge".into(),
            );
        }
        let [selecting] = self
            .predecessors
            .get(plan.success.index() as usize)
            .map(Vec::as_slice)
            .unwrap_or(&[])
        else {
            return Err("call ownership prepared success has a foreign incoming edge".into());
        };
        if *selecting != plan.selecting {
            return Err("call ownership prepared success lost its selecting predecessor".into());
        }
        let [defined] = self
            .definitions
            .get(&plan.output.index())
            .map(Vec::as_slice)
            .unwrap_or(&[])
        else {
            return Err(
                "call ownership prepared success output requires one retained extraction".into(),
            );
        };
        if defined.span != plan.span
            || defined.site.block != plan.success
            || !matches!(defined.kind, Definition::SumTake { source, success: true } if source == plan.source)
        {
            return Err("call ownership prepared success lost its exact successful SumTake".into());
        }
        self.producer_source(plan.source, plan.source_type, plan.span, defined.site)
    }

    fn validate_prepared_presences(&self) -> Result<(), String> {
        let mut outputs = BTreeSet::new();
        for record in &self.function.prepared_present_successes {
            if !outputs.insert(record.plan().output.index()) {
                return Err(
                    "call ownership prepared success has duplicate output authority".into(),
                );
            }
            self.prepared_presence(record.plan())?;
        }
        Ok(())
    }

    fn absent_source_type(&self, ty: TypeId) -> bool {
        if !valid_type(self.types, ty) {
            return false;
        }
        match self.types.resolve(ty) {
            Type::Optional(inner) => *inner == TypeInterner::NEVER,
            Type::Result(ok, error) => {
                *ok == TypeInterner::NEVER
                    && *error != TypeInterner::NEVER
                    && valid_type(self.types, *error)
            }
            _ => false,
        }
    }

    fn original_absence(
        &self,
        source: LocalId,
        output: LocalId,
        source_type: TypeId,
        output_type: TypeId,
        span: Span,
        success: BlockId,
    ) -> Result<AbsentSuccessPlan, String> {
        if !self.absent_source_type(source_type) {
            return Err(
                "call ownership absent success requires an exact uninhabited sum source".into(),
            );
        }
        let definitions = self
            .definitions
            .get(&output.index())
            .ok_or("call ownership absent output is undefined")?;
        if definitions
            .iter()
            .filter(|defined| {
                matches!(defined.kind,
            Definition::SumTake { source: owner, success: true } if owner == source)
                    && defined.span == span
            })
            .count()
            != 1
        {
            return Err("call ownership absent output requires one exact success anchor".into());
        }
        let [selecting] = self
            .predecessors
            .get(success.index() as usize)
            .map(Vec::as_slice)
            .unwrap_or(&[])
        else {
            return Err("call ownership absent success requires one selecting branch".into());
        };
        let block = &self.function.blocks[selecting.index() as usize];
        let T::Branch {
            condition,
            then_block,
            else_block,
        } = &block.terminator.kind
        else {
            return Err("call ownership absent success has no original branch".into());
        };
        let E::Local(tag) = condition.kind else {
            return Err("call ownership absent selector is not a tag local".into());
        };
        if *then_block != success
            || *else_block == success
            || condition.ty != TypeInterner::BOOL
            || condition.span != span
            || block.terminator.span != span
        {
            return Err("call ownership absent selector differs from its exact Handle".into());
        }
        let plan = AbsentSuccessPlan {
            function: self.function.id,
            identity: self.function.identity.clone(),
            source,
            tag,
            output,
            source_type,
            output_type,
            span,
            selecting: *selecting,
            failure: *else_block,
        };
        self.absence_tag(&plan)?;
        Ok(plan)
    }

    fn absence_tag(&self, plan: &AbsentSuccessPlan) -> Result<(), String> {
        if plan.function != self.function.id
            || plan.identity != self.function.identity
            || !self.absent_source_type(plan.source_type)
            || !valid_type(self.types, plan.output_type)
        {
            return Err(
                "call ownership prepared absence has foreign function or type authority".into(),
            );
        }
        let source = self
            .function
            .local(plan.source)
            .ok_or("call ownership absent source is outside its function")?;
        let output = self
            .function
            .local(plan.output)
            .ok_or("call ownership absent output is outside its function")?;
        let tag = self
            .function
            .local(plan.tag)
            .ok_or("call ownership absent tag is outside its function")?;
        if source.ty != plan.source_type
            || source.debug_ty != plan.source_type
            || output.ty != plan.output_type
            || output.debug_ty != plan.output_type
            || output.span != plan.span
            || output.view_source.is_some()
            || self.function.parameter_for_local(plan.output).is_some()
            || tag.ty != TypeInterner::BOOL
            || tag.debug_ty != TypeInterner::BOOL
            || tag.span != plan.span
        {
            return Err(
                "call ownership absent certificate changed its exact local metadata".into(),
            );
        }
        let block = self
            .function
            .blocks
            .get(plan.selecting.index() as usize)
            .ok_or("call ownership absent selecting block is outside its function")?;
        if self
            .function
            .blocks
            .get(plan.failure.index() as usize)
            .is_none()
            || plan.selecting == plan.failure
            || block.terminator.span != plan.span
        {
            return Err("call ownership absent failure edge is outside its exact Handle".into());
        }
        let Some(statement) = block.statements.last() else {
            return Err("call ownership absent edge lost its runtime tag".into());
        };
        if statement.span != plan.span
            || !matches!(statement.kind,
            S::SumTag { source, target } if source == plan.source && target == plan.tag)
        {
            return Err("call ownership absent edge lost its exact runtime SumTag".into());
        }
        let [definition] = self
            .definitions
            .get(&plan.tag.index())
            .map(Vec::as_slice)
            .unwrap_or(&[])
        else {
            return Err("call ownership absent edge has an ambiguous tag definition".into());
        };
        if definition.site
            != (Site {
                block: plan.selecting,
                statement: block.statements.len() - 1,
            })
        {
            return Err("call ownership absent tag belongs to another selecting block".into());
        }
        self.producer_source(plan.source, plan.source_type, plan.span, definition.site)
    }

    fn prepared_absence(&self, plan: &AbsentSuccessPlan) -> Result<(), String> {
        self.absence_tag(plan)?;
        if !matches!(self.function.blocks[plan.selecting.index() as usize].terminator.kind,
            T::Goto(target) if target == plan.failure)
        {
            return Err(
                "call ownership prepared absence changed its canonical selected edge".into(),
            );
        }
        let definitions = self
            .definitions
            .get(&plan.output.index())
            .ok_or("call ownership prepared absent output has no defaults")?;
        for defined in definitions {
            let Definition::Let(value) = defined.kind else {
                return Err(
                    "call ownership prepared absent output has a foreign definition".into(),
                );
            };
            if value.ty != plan.output_type
                || matches!(value.kind, E::View(_))
                || !contains(plan.span, defined.span)
                || !contains(defined.span, value.span)
            {
                return Err(
                    "call ownership prepared absent default changes type or occurrence".into(),
                );
            }
            if let E::Local(local) = value.kind {
                if self.function.is_view_local(local) {
                    return Err(
                        "call ownership prepared absent default acquires a borrowed owner".into(),
                    );
                }
            }
            self.sum_failure_region(plan.source, defined.site.block, plan.span)?;
        }
        Ok(())
    }

    fn validate_prepared_absences(&self) -> Result<(), String> {
        let mut outputs = BTreeSet::new();
        for record in &self.function.prepared_absent_successes {
            if !outputs.insert(record.plan().output.index()) {
                return Err(
                    "call ownership prepared absence has duplicate output authority".into(),
                );
            }
            self.prepared_absence(record.plan())?;
        }
        Ok(())
    }

    /// A default definition belongs to the failed arm of this exact Handle,
    /// not merely to a source span somewhere inside the same function.
    fn sum_failure_region(
        &self,
        source: LocalId,
        start: BlockId,
        span: Span,
    ) -> Result<(), String> {
        let mut queue = vec![start];
        let mut seen = BTreeSet::new();
        let mut anchored = false;
        while let Some(current) = queue.pop() {
            if !seen.insert(current.index()) {
                continue;
            }
            let incoming = self
                .predecessors
                .get(current.index() as usize)
                .ok_or("call ownership handled default block is absent")?;
            if current == self.function.entry || incoming.is_empty() {
                return Err(
                    "call ownership handled default has no exact failure-edge anchor".into(),
                );
            }
            for &previous in incoming {
                let block = self
                    .function
                    .blocks
                    .get(previous.index() as usize)
                    .ok_or("call ownership handled default predecessor is absent")?;
                let mut selected = false;
                if let Some(record) =
                    self.function
                        .prepared_absent_successes
                        .iter()
                        .find(|record| {
                            let plan = record.plan();
                            plan.source == source
                                && plan.span == span
                                && plan.selecting == previous
                                && plan.failure == current
                        })
                {
                    self.absence_tag(record.plan())?;
                    if !matches!(block.terminator.kind, T::Goto(target) if target == current) {
                        return Err(
                            "call ownership prepared default changed its authenticated false edge"
                                .into(),
                        );
                    }
                    selected = true;
                    anchored = true;
                }
                if let T::Branch {
                    condition,
                    then_block,
                    else_block,
                } = &block.terminator.kind
                {
                    if let E::Local(tag) = condition.kind {
                        if let [defined] = self
                            .definitions
                            .get(&tag.index())
                            .map(Vec::as_slice)
                            .unwrap_or(&[])
                        {
                            let statement = self
                                .function
                                .blocks
                                .get(defined.site.block.index() as usize)
                                .and_then(|block| block.statements.get(defined.site.statement));
                            if statement.is_some_and(|statement| matches!(statement.kind,
                                S::SumTag { source: owner, target } if owner == source && target == tag)) {
                                if condition.ty != TypeInterner::BOOL
                                    || self.function.local(tag).is_none_or(|local| local.ty != TypeInterner::BOOL)
                                    || block.terminator.span != span || defined.span != span
                                    || *else_block != current || *then_block == current {
                                    return Err("call ownership handled default is not selected by its exact false-tag edge".into());
                                }
                                self.dominates(defined.site, Site { block: previous, statement: block.statements.len() })?;
                                selected = true;
                                anchored = true;
                            }
                        }
                    }
                }
                if !selected {
                    queue.push(previous);
                }
            }
        }
        if !anchored {
            return Err(
                "call ownership handled default cycle has no exact failure-edge anchor".into(),
            );
        }
        Ok(())
    }

    fn generated_operand(
        &mut self,
        value: &Expression,
        argument: &hir::GeneratedArgumentOwnership,
        used: Site,
    ) -> Result<(), String> {
        let local_info = self.locals_at(used);
        match argument.staging {
            hir::GeneratedArgumentStaging::Existing { loan: None } => {
                if hir::validate_generated_operand_tree(value, argument, &local_info).is_ok() {
                    return Ok(());
                }
                // A physical owned operand can have been materialized by the
                // ordinary ordered-value path. Its unique Let must restore the
                // sealed original tree, including primitive Copy acquisitions.
                if argument.callee_access != jett_typecheck::CheckedCalleeAccess::Owned {
                    return Err(
                        "generated borrowed operand lost its explicit storage association".into(),
                    );
                }
                let E::Local(local) = value.kind else {
                    return self.generated_original(value, argument, used);
                };
                self.slot_operand(value, local, false)?;
                let (initializer, defined) = self.owning_let(local, value.ty, used)?;
                if let Err(error) = self.generated_original(initializer, argument, defined.site) {
                    let E::Clone(original) = &initializer.kind else {
                        return Err(error);
                    };
                    if argument.acquisition != hir::GeneratedAcquisition::Copy
                        || original.ty != initializer.ty
                        || !jett_typecheck::ownership::is_implicitly_copyable(
                            self.types,
                            argument.actual_type,
                        )
                    {
                        return Err(error);
                    }
                    self.generated_original(original, argument, defined.site)?;
                }
                self.claim_slot(local)
            }
            hir::GeneratedArgumentStaging::Existing { loan: Some(loan) } => {
                self.slot_operand(value, loan, true)?;
                let (projection, begun) = self.loan(loan, value.ty, used)?;
                self.generated_original(projection, argument, begun.site)?;
                self.claim_slot(loan)
            }
            hir::GeneratedArgumentStaging::OwnedProducer { owner, loan }
            | hir::GeneratedArgumentStaging::OrdinarySnapshot { owner, loan } => {
                self.slot_operand(value, loan, true)?;
                let (projection, begun) = self.loan(loan, value.ty, used)?;
                self.slot_operand(projection, owner, true)?;
                if self
                    .function
                    .local(loan)
                    .and_then(|local| local.view_source)
                    != Some(owner)
                {
                    return Err("generated loan does not borrow its exact acquired owner".into());
                }
                let (initializer, defined) = self.owning_let(owner, value.ty, begun.site)?;
                match argument.staging {
                    hir::GeneratedArgumentStaging::OwnedProducer { .. } => {
                        if !argument.original_witness().owned_producer()
                            || matches!(initializer.kind, E::View(_))
                        {
                            return Err("generated owner lost its sealed full producer".into());
                        }
                        let restored = crate::call_views::replace_borrowed_root(
                            projection,
                            initializer.clone(),
                        )
                        .ok_or("generated owner changed its original endpoint type")?;
                        self.generated_original(&restored, argument, defined.site)?;
                    }
                    hir::GeneratedArgumentStaging::OrdinarySnapshot { .. } => {
                        if argument
                            .original_witness()
                            .ordinary_snapshot_proof()
                            .map(|proof| proof.actual_type())
                            != Some(argument.actual_type)
                        {
                            return Err(
                                "generated snapshot lost its closed ordinary-data proof".into()
                            );
                        }
                        let E::Clone(observed) = &initializer.kind else {
                            return Err(
                                "generated snapshot requires its exact explicit Clone initializer"
                                    .into(),
                            );
                        };
                        if observed.ty != initializer.ty || initializer.ty != argument.actual_type {
                            return Err(
                                "generated snapshot changed its sealed endpoint type".into()
                            );
                        }
                        self.generated_original(observed, argument, defined.site)?;
                    }
                    _ => unreachable!("matched generated owner stage"),
                }
                self.claim_slot(owner)?;
                self.claim_slot(loan)
            }
        }
    }

    fn generated_original(
        &self,
        value: &Expression,
        argument: &hir::GeneratedArgumentOwnership,
        used: Site,
    ) -> Result<(), String> {
        let restored = self.restore_generated_internal_copy(value, argument);
        let value = restored.as_ref().unwrap_or(value);
        let local_info = self.locals_at(used);
        let error = match hir::validate_generated_operand_tree(value, argument, &local_info) {
            Ok(()) => return Ok(()),
            Err(error) => error,
        };
        let witness = argument.original_witness();
        if witness.origin() != hir::CallerViewSource::Other
            || value.ty != witness.actual_type()
            || value.span != witness.source_span()
        {
            return Err(error);
        }
        let mut original = value;
        if !witness.written_view()
            && let E::View(inner) = &original.kind
            && original.ty == inner.ty
            && original.span == inner.span
        {
            original = inner;
        }
        // A private original node can be restored through unique owning storage;
        // only its sealed Handle variant authorizes the selected CFG result proof.
        // Every reconstructed root/target/wrapper/backing remains exact, including
        // original explicit Clone versus an introduced Clone.
        witness.validate_lowered_producer_shape(original, |value, shape| {
            self.generated_storage(value, shape, used)
        })
    }

    fn generated_storage(
        &self,
        value: &Expression,
        shape: hir::GeneratedOriginalShape<'_>,
        used: Site,
    ) -> Result<(), String> {
        self.generated_storage_inner(value, shape, used, &mut BTreeSet::new())
    }

    fn generated_storage_inner(
        &self,
        value: &Expression,
        shape: hir::GeneratedOriginalShape<'_>,
        used: Site,
        aliases: &mut BTreeSet<u32>,
    ) -> Result<(), String> {
        if value.ty != shape.ty() || value.span != shape.span() {
            return Err(
                "call ownership generated storage changes its sealed original occurrence".into(),
            );
        }
        let E::Local(local) = value.kind else {
            return Err("call ownership generated storage has no exact materialized slot".into());
        };
        if !aliases.insert(local.index()) {
            return Err("call ownership generated storage contains a cycle".into());
        }
        let result = (|| {
            if let Some(handle) = shape.handle() {
                return self.generated_handled_result(value, handle, shape, used, aliases);
            }
            let (initializer, defined) = self.owning_let(local, shape.ty(), used)?;
            match shape.validate_reconstructed(initializer, |value, nested| {
                self.generated_storage_inner(value, nested, defined.site, aliases)
            }) {
                Ok(()) => Ok(()),
                Err(error) => shape
                    .validate_existing_local_snapshot(initializer, |value, nested| {
                        self.generated_storage_inner(value, nested, defined.site, aliases)
                    })
                    .map_err(|_| error),
            }
        })();
        aliases.remove(&local.index());
        result
    }

    fn generated_handled_result(
        &self,
        value: &Expression,
        shape: hir::GeneratedHandleShape<'_>,
        original_node: hir::GeneratedOriginalShape<'_>,
        used: Site,
        aliases: &mut BTreeSet<u32>,
    ) -> Result<(), String> {
        if matches!(shape.kind(), hir::HandleKind::Refinement { .. }) {
            return self.generated_refinement_result(value, shape, original_node, used, aliases);
        }
        if !matches!(
            shape.kind(),
            hir::HandleKind::Optional | hir::HandleKind::Result
        ) || value.ty != shape.result_type()
            || value.span != shape.span()
        {
            return Err(
                "call ownership generated Handle has no exact selected sum occurrence".into(),
            );
        }
        let E::Local(local) = value.kind else {
            return Err(
                "call ownership generated Handle is not an authenticated CFG result".into(),
            );
        };
        let definitions = self
            .definitions
            .get(&local.index())
            .ok_or("call ownership generated Handle result is undefined")?;
        let prepared = self
            .function
            .prepared_absent_successes
            .iter()
            .find(|record| record.plan().output == local);
        if prepared.is_none()
            && let [defined] = definitions.as_slice()
            && let Definition::Let(initializer) = defined.kind
            && matches!(initializer.kind, E::Local(_))
        {
            // Ordered constructor fields can add a unique owning slot around
            // an already lowered result. Rejoin only the same exact occurrence;
            // no binding, View, Clone or arbitrary producer becomes a Handle.
            let (initializer, defined) = self.owning_let(local, shape.result_type(), used)?;
            if initializer.span != shape.span() {
                return Err(
                    "call ownership generated Handle storage changes its occurrence".into(),
                );
            }
            return self.generated_storage_inner(initializer, original_node, defined.site, aliases);
        }
        self.produced_result(local, shape.result_type(), shape.span(), used)?;
        let sources = definitions
            .iter()
            .filter_map(|defined| match defined.kind {
                Definition::SumTake {
                    source,
                    success: true,
                } if defined.span == shape.span() => Some(source),
                _ => None,
            })
            .collect::<Vec<_>>();
        let source = match (sources.as_slice(), prepared) {
            ([source], None) => *source,
            ([], Some(record)) => record.plan().source,
            _ => return Err("call ownership generated Handle lost its exact source anchor".into()),
        };
        let source_type = self
            .function
            .local(source)
            .ok_or("call ownership generated Handle source is absent")?
            .ty;
        if source_type != shape.source_type()
            || !valid_type(self.types, source_type)
            || !matches!(
                (shape.kind(), self.types.resolve(source_type)),
                (hir::HandleKind::Optional, Type::Optional(_))
                    | (hir::HandleKind::Result, Type::Result(_, _))
            )
        {
            return Err(
                "call ownership generated Handle source differs from its sealed sum type".into(),
            );
        }
        self.producer_source(source, source_type, shape.span(), used)?;
        let [defined] = self
            .definitions
            .get(&source.index())
            .map(Vec::as_slice)
            .unwrap_or(&[])
        else {
            return Err(
                "call ownership generated Handle requires one exact source initializer".into(),
            );
        };
        let Definition::Let(initializer) = defined.kind else {
            return Err(
                "call ownership generated Handle source is not its owning initializer".into(),
            );
        };
        shape.validate_lowered_source_shape(initializer, |value, nested| {
            self.generated_storage_inner(value, nested, defined.site, aliases)
        })
    }

    fn generated_refinement_result(
        &self,
        value: &Expression,
        shape: hir::GeneratedHandleShape<'_>,
        original_node: hir::GeneratedOriginalShape<'_>,
        used: Site,
        aliases: &mut BTreeSet<u32>,
    ) -> Result<(), String> {
        let hir::HandleKind::Refinement {
            refined_type,
            predicates,
        } = shape.kind()
        else {
            return Err("call ownership generated refinement lost its immutable kind".into());
        };
        if value.ty != shape.result_type()
            || value.span != shape.span()
            || *refined_type != shape.result_type()
            || predicates.is_empty()
            || predicates
                .last()
                .is_none_or(|predicate| predicate.refined_type != *refined_type)
            || !valid_type(self.types, *refined_type)
            || !matches!(self.types.resolve(*refined_type), Type::Refinement { .. })
        {
            return Err(
                "call ownership generated refinement differs from its exact nominal occurrence"
                    .into(),
            );
        }
        let E::Local(output) = value.kind else {
            return Err(
                "call ownership generated refinement has no authenticated output slot".into(),
            );
        };
        let definitions = self
            .definitions
            .get(&output.index())
            .ok_or("call ownership generated refinement output is undefined")?;
        if let [defined] = definitions.as_slice()
            && let Definition::Let(initializer) = defined.kind
            && matches!(initializer.kind, E::Local(_))
        {
            let (initializer, defined) = self.owning_let(output, value.ty, used)?;
            if initializer.span != shape.span() {
                return Err(
                    "call ownership generated refinement storage changes its occurrence".into(),
                );
            }
            return self.generated_storage_inner(initializer, original_node, defined.site, aliases);
        }
        let metadata = self
            .function
            .local(output)
            .ok_or("call ownership generated refinement output is absent")?;
        if metadata.ty != value.ty
            || metadata.debug_ty != value.ty
            || metadata.span != shape.span()
            || metadata.view_source.is_some()
            || self.function.parameter_for_local(output).is_some()
        {
            return Err(
                "call ownership generated refinement output has no exact owning metadata".into(),
            );
        }
        let successes = definitions
            .iter()
            .filter_map(|defined| match defined.kind {
                Definition::Let(Expression {
                    kind: E::RefinementValidated(inner),
                    ty,
                    span,
                }) if *ty == value.ty && *span == shape.span() && defined.span == shape.span() => {
                    Some((*defined, inner.as_ref()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let [(success, candidate)] = successes.as_slice() else {
            return Err(
                "call ownership generated refinement requires one exact validated output".into(),
            );
        };
        let E::Local(source) = candidate.kind else {
            return Err(
                "call ownership generated refinement validation has no exact candidate".into(),
            );
        };
        if candidate.ty != shape.source_type() || candidate.span != shape.span() {
            return Err(
                "call ownership generated refinement candidate differs from its immutable source"
                    .into(),
            );
        }
        self.producer_source(source, candidate.ty, shape.span(), success.site)?;
        let success_block = &self.function.blocks[success.site.block.index() as usize];
        if success.site.statement != 0
            || success_block.statements.len() != 1
            || success_block.terminator.span != shape.span()
            || !matches!(success_block.terminator.kind, T::Goto(_))
        {
            return Err(
                "call ownership generated refinement changed its canonical success block".into(),
            );
        }
        let mut next = success.site.block;
        let mut rejected = BTreeSet::new();
        let mut selecting = BTreeSet::new();
        let mut failed = None;
        for predicate in predicates.iter().rev() {
            let (previous, refusal, failure) =
                self.refinement_selection(next, source, shape, predicate)?;
            if !selecting.insert(previous.index())
                || !rejected.insert(refusal.index())
                || failed.is_some_and(|expected| expected != failure)
            {
                return Err("call ownership generated refinement changed its ordered checks or shared failure".into());
            }
            failed = Some(failure);
            next = previous;
        }
        let failed = failed.ok_or("call ownership generated refinement has no selected failure")?;
        let failure_block = self
            .function
            .blocks
            .get(failed.index() as usize)
            .ok_or("call ownership generated refinement failure block is absent")?;
        let incoming = self
            .predecessors
            .get(failed.index() as usize)
            .ok_or("call ownership generated refinement failure predecessors are absent")?;
        if incoming
            .iter()
            .map(|block| block.index())
            .collect::<BTreeSet<_>>()
            != rejected
            || incoming.len() != rejected.len()
            || failed == success.site.block
            || failure_block
                .statements
                .iter()
                .any(|statement| !contains(shape.failure_span(), statement.span))
            || (!contains(shape.failure_span(), failure_block.terminator.span)
                && failure_block.terminator.span != shape.span())
        {
            return Err(
                "call ownership generated refinement failure has a foreign edge or occurrence"
                    .into(),
            );
        }
        for defined in definitions {
            if defined.site == success.site {
                continue;
            }
            let Definition::Let(default) = defined.kind else {
                return Err(
                    "call ownership generated refinement output has a foreign definition".into(),
                );
            };
            if default.ty != value.ty
                || !contains(shape.failure_span(), defined.span)
                || !contains(defined.span, default.span)
                || self.handled_default_is_borrowed(default, defined.site)
            {
                return Err("call ownership generated refinement default lost its exact owned failure occurrence".into());
            }
            self.refinement_failure_region(failed, defined.site.block)?;
        }
        self.definitions_reach(definitions, used)?;
        let [defined] = self
            .definitions
            .get(&source.index())
            .map(Vec::as_slice)
            .unwrap_or(&[])
        else {
            return Err(
                "call ownership generated refinement requires one owning source initializer".into(),
            );
        };
        let Definition::Let(initializer) = defined.kind else {
            return Err(
                "call ownership generated refinement source is not its owning initializer".into(),
            );
        };
        shape.validate_lowered_source_shape(initializer, |value, nested| {
            self.generated_storage_inner(value, nested, defined.site, aliases)
        })
    }

    fn refinement_selection(
        &self,
        success: BlockId,
        source: LocalId,
        shape: hir::GeneratedHandleShape<'_>,
        predicate: &hir::RefinementPredicate,
    ) -> Result<(BlockId, BlockId, BlockId), String> {
        let [previous] = self
            .predecessors
            .get(success.index() as usize)
            .map(Vec::as_slice)
            .unwrap_or(&[])
        else {
            return Err(
                "call ownership generated refinement success has a bypass or foreign predecessor"
                    .into(),
            );
        };
        let block = &self.function.blocks[previous.index() as usize];
        let T::Branch {
            condition,
            then_block,
            else_block,
        } = &block.terminator.kind
        else {
            return Err(
                "call ownership generated refinement requires its original predicate Branch".into(),
            );
        };
        if *then_block != success
            || *else_block == success
            || block.terminator.span != shape.span()
            || condition.ty != TypeInterner::BOOL
            || condition.span != shape.span()
        {
            return Err("call ownership generated refinement changed its selecting edge".into());
        }
        let E::Local(passed) = condition.kind else {
            return Err(
                "call ownership generated refinement requires its exact passed local".into(),
            );
        };
        let (test, defined) = self.owning_let(
            passed,
            TypeInterner::BOOL,
            Site {
                block: *previous,
                statement: block.statements.len(),
            },
        )?;
        if defined.site.block != *previous
            || defined.site.statement + 1 != block.statements.len()
            || defined.site.statement == 0
        {
            return Err(
                "call ownership generated refinement passed test is not after its exact check"
                    .into(),
            );
        }
        let S::CheckRefinement {
            local: error,
            call,
            type_name,
        } = &block.statements[defined.site.statement - 1].kind
        else {
            return Err(
                "call ownership generated refinement lost its exact predicate check".into(),
            );
        };
        let checked_site = Site {
            block: *previous,
            statement: defined.site.statement - 1,
        };
        let checked = &block.statements[checked_site.statement];
        let [error_definition] = self
            .definitions
            .get(&error.index())
            .map(Vec::as_slice)
            .unwrap_or(&[])
        else {
            return Err(
                "call ownership generated refinement error has a foreign definition".into(),
            );
        };
        let error_metadata = self
            .function
            .local(*error)
            .ok_or("call ownership generated refinement error is absent")?;
        if error_definition.site != checked_site
            || error_definition.span != shape.span()
            || error_metadata.ty != TypeInterner::STRING
            || error_metadata.debug_ty != TypeInterner::STRING
            || error_metadata.span != shape.span()
            || error_metadata.view_source.is_some()
            || self.function.parameter_for_local(*error).is_some()
            || checked.span != shape.span()
            || type_name != &predicate.type_name
            || call.ty != TypeInterner::BOOL
            || call.span != shape.span()
        {
            return Err(
                "call ownership generated refinement changed its exact predicate occurrence".into(),
            );
        }
        let E::Call {
            function,
            args,
            evaluation_order,
            ownership: hir::CallOwnership::Generated(packet),
        } = &call.kind
        else {
            return Err(
                "call ownership generated refinement check has no exact generated target".into(),
            );
        };
        if *function != predicate.function
            || args.len() != 1
            || evaluation_order != &[0]
            || packet.operation
                != (hir::GeneratedOperation::RefinementPredicate {
                    function: predicate.function,
                })
        {
            return Err(
                "call ownership generated refinement check differs from its immutable predicate"
                    .into(),
            );
        }
        let span = shape.span();
        let mut input = Expression {
            kind: E::Clone(Box::new(Expression {
                kind: E::Local(source),
                ty: shape.source_type(),
                span,
            })),
            ty: shape.source_type(),
            span,
        };
        if input.ty != predicate.base_type && input.ty != predicate.input_type {
            input = Expression {
                kind: E::Coarsen(Box::new(input)),
                ty: predicate.base_type,
                span,
            };
        }
        if input.ty != predicate.input_type {
            input = Expression {
                kind: E::Declassify(Box::new(input)),
                ty: predicate.input_type,
                span,
            };
        }
        if args[0] != input
            || test.ty != TypeInterner::BOOL
            || test.span != span
            || defined.span != span
        {
            return Err(
                "call ownership generated refinement changed its exact candidate input".into(),
            );
        }
        let expected = Expression {
            kind: E::Binary {
                left: Box::new(Expression {
                    kind: E::View(Box::new(Expression {
                        kind: E::Local(*error),
                        ty: TypeInterner::STRING,
                        span,
                    })),
                    ty: TypeInterner::STRING,
                    span,
                }),
                op: hir::BinaryOp::Equal,
                right: Box::new(Expression {
                    kind: E::String(String::new()),
                    ty: TypeInterner::STRING,
                    span,
                }),
            },
            ty: TypeInterner::BOOL,
            span,
        };
        if *test != expected {
            return Err(
                "call ownership generated refinement changed its exact empty-error test".into(),
            );
        }
        self.producer_source(source, shape.source_type(), span, checked_site)?;
        let refusal = &self.function.blocks[else_block.index() as usize];
        let T::Goto(failed) = refusal.terminator.kind else {
            return Err(
                "call ownership generated refinement rejection does not enter its failure body"
                    .into(),
            );
        };
        if refusal.terminator.span != span
            || self.predecessors[else_block.index() as usize].as_slice() != [*previous]
        {
            return Err("call ownership generated refinement rejection has a foreign edge".into());
        }
        if let Some(local) = shape.error_local() {
            let metadata = self
                .function
                .local(local)
                .ok_or("call ownership generated refinement error binding is absent")?;
            if metadata.ty != TypeInterner::STRING
                || metadata.debug_ty != TypeInterner::STRING
                || metadata.view_source.is_some()
                || self.function.parameter_for_local(local).is_some()
            {
                return Err(
                    "call ownership generated refinement error binding has invalid owning metadata"
                        .into(),
                );
            }
        }
        match (shape.error_local(), refusal.statements.as_slice()) {
            (None, []) => {}
            (Some(local), [statement])
                if statement.span == span
                    && matches!(&statement.kind, S::Let { local: target, value } if *target == local
                    && value.ty == TypeInterner::STRING && value.span == span
                    && matches!(value.kind, E::Local(source) if source == *error)) => {}
            _ => {
                return Err(
                    "call ownership generated refinement lost its exact error binding".into(),
                );
            }
        }
        Ok((*previous, *else_block, failed))
    }

    fn handled_default_is_borrowed(&self, value: &Expression, site: Site) -> bool {
        let endpoint = value.ty;
        let mut backing = value;
        loop {
            backing = match &backing.kind {
                E::Coarsen(inner) | E::Declassify(inner) => inner,
                E::View(_) | E::RefinementValidated(_) => return true,
                E::Local(local) => {
                    if jett_typecheck::ownership::is_implicitly_copyable(self.types, endpoint) {
                        return false;
                    }
                    return self.locals_at(site).get(local.index() as usize).is_none_or(
                        |metadata| {
                            metadata.view_source.is_some()
                                || metadata.is_view_parameter
                                || metadata.view_iteration.is_some()
                        },
                    );
                }
                _ => return false,
            };
        }
    }

    fn refinement_failure_region(&self, failure: BlockId, start: BlockId) -> Result<(), String> {
        let mut queue = vec![start];
        let mut seen = BTreeSet::new();
        let mut anchored = false;
        while let Some(current) = queue.pop() {
            if current == failure {
                anchored = true;
                continue;
            }
            if !seen.insert(current.index()) {
                continue;
            }
            let incoming = self
                .predecessors
                .get(current.index() as usize)
                .ok_or("call ownership generated refinement default block is absent")?;
            if current == self.function.entry || incoming.is_empty() {
                return Err(
                    "call ownership generated refinement default has a failure-region bypass"
                        .into(),
                );
            }
            queue.extend(incoming.iter().copied());
        }
        if !anchored {
            return Err(
                "call ownership generated refinement default has no exact failure anchor".into(),
            );
        }
        Ok(())
    }

    fn restore_generated_internal_copy(
        &self,
        value: &Expression,
        argument: &hir::GeneratedArgumentOwnership,
    ) -> Option<Expression> {
        let hir::CallerViewSource::Local(source) = argument.original_witness().origin() else {
            return None;
        };
        argument.original_witness().ordinary_snapshot_proof()?;
        // Existing Coarsen/Declassify lowering can snapshot a source Local.
        // Restore only that exact transparent path for metadata validation;
        // the emitted Clone and qualified runtime tree remain unchanged.
        fn restore(value: &mut Expression, source: LocalId) -> bool {
            let replacement = match &value.kind {
                E::Clone(inner)
                    if value.ty == inner.ty
                        && value.span == inner.span
                        && matches!(inner.kind, E::Local(local) if local == source) =>
                {
                    Some(inner.as_ref().clone())
                }
                _ => None,
            };
            if let Some(replacement) = replacement {
                *value = replacement;
                return true;
            }
            match &mut value.kind {
                E::View(inner) | E::Coarsen(inner) | E::Declassify(inner) => restore(inner, source),
                _ => false,
            }
        }
        let mut original = value.clone();
        if !restore(&mut original, source) {
            return None;
        }
        let root = self.function.local(source)?;
        let proof = Expression {
            kind: E::View(Box::new(original.clone())),
            ty: original.ty,
            span: original.span,
        };
        hir::validate_local_view_initializer(&proof, source, root.ty, original.ty, self.types)
            .ok()?;
        Some(original)
    }
    fn dominates(&self, definition: Site, used: Site) -> Result<(), String> {
        self.definitions_reach(
            &[Defined {
                site: definition,
                span: self.function.span,
                kind: Definition::Other,
            }],
            used,
        )
    }

    /// Check every backward path, retaining disconnected-block obligations.
    /// A cycle with no defining predecessor cannot manufacture initialization.
    fn definitions_reach(&self, definitions: &[Defined<'_>], used: Site) -> Result<(), String> {
        let mut queue = vec![used];
        let mut seen = BTreeSet::new();
        let mut reached = false;
        while let Some(site) = queue.pop() {
            if definitions.iter().any(|definition| {
                definition.site.block == site.block && definition.site.statement < site.statement
            }) {
                reached = true;
                continue;
            }
            if !seen.insert(site.block.index()) {
                continue;
            }
            let incoming = self
                .predecessors
                .get(site.block.index() as usize)
                .ok_or("call ownership use block is absent")?;
            if site.block == self.function.entry || incoming.is_empty() {
                return Err(
                    "call ownership storage use is not preceded by its initializer on every path"
                        .into(),
                );
            }
            for &previous in incoming {
                let block = self
                    .function
                    .blocks
                    .get(previous.index() as usize)
                    .ok_or("call ownership predecessor is absent")?;
                queue.push(Site {
                    block: previous,
                    statement: block.statements.len(),
                });
            }
        }
        if !reached {
            return Err("call ownership storage cycle has no initialized producer".into());
        }
        Ok(())
    }

    fn statement(
        &mut self,
        statement: &'a S,
        site: Site,
        context: CheckedOwnershipContext,
    ) -> Result<(), String> {
        match statement {
            S::Let { value, .. }
            | S::BeginCallView { value, .. }
            | S::Evaluate(value)
            | S::HandleDefault(value) => self.expression(value, site, context)?,
            S::CheckRefinement { call, .. } => self.expression(call, site, context)?,
            S::Assign { target, value } => {
                self.expression(target, site, context)?;
                self.expression(value, site, context)?;
            }
            S::Assert { condition, message } => {
                self.expression(condition, site, context)?;
                if let Some(message) = message {
                    self.expression(message, site, context)?;
                }
            }
            S::Breakpoint { condition, .. } => {
                if let Some(condition) = condition {
                    self.expression(
                        condition,
                        site,
                        CheckedOwnershipContext::BreakpointExpression,
                    )?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn terminator(
        &mut self,
        terminator: &'a T,
        site: Site,
        context: CheckedOwnershipContext,
    ) -> Result<(), String> {
        match terminator {
            T::Return(Some(value)) | T::Respond(value) => self.expression(value, site, context)?,
            T::Branch { condition, .. } => self.expression(condition, site, context)?,
            T::Switch { scrutinee, .. } => self.expression(scrutinee, site, context)?,
            T::ForEach { iterable, .. } => self.expression(iterable, site, context)?,
            T::ReflectedTypeDispatch { type_info, .. } => {
                self.expression(type_info, site, context)?
            }
            _ => {}
        }
        Ok(())
    }

    fn expression(
        &mut self,
        expression: &'a Expression,
        site: Site,
        context: CheckedOwnershipContext,
    ) -> Result<(), String> {
        self.manifest.validate_type(self.types, expression.ty)?;
        if let E::ResourceHookValue { hook } = &expression.kind {
            if !self.manifest.contains_hook(hook) || expression.ty != hook.function_type() {
                return Err(
                    "Resource descriptor differs from its original checked manifest or signature"
                        .into(),
                );
            }
        }
        self.invocation(expression, site, context)?;
        match &expression.kind {
            E::Call { args, .. }
            | E::ResourceInvoke { args, .. }
            | E::Intrinsic { args, .. }
            | E::ActorSpawn { args, .. } => {
                for value in args {
                    self.expression(value, site, context)?;
                }
            }
            E::IndirectCall { callee, args, .. } => {
                self.expression(callee, site, context)?;
                for value in args {
                    self.expression(value, site, context)?;
                }
            }
            E::Binary { left, right, .. } => {
                self.expression(left, site, context)?;
                self.expression(right, site, context)?;
            }
            E::Unary { value, .. }
            | E::View(value)
            | E::Clone(value)
            | E::Run(value)
            | E::Join(value)
            | E::Cancel(value)
            | E::Coarsen(value)
            | E::Declassify(value)
            | E::RefinementValidated(value)
            | E::InterfaceType(value)
            | E::DisplayResult(value)
            | E::EquatableResult(value)
            | E::ResultOk(value)
            | E::ResultFail(value)
            | E::OptionalSome(value)
            | E::RuntimeFailureMessage(value)
            | E::StateIs { value, .. }
            | E::Comptime { value, .. }
            | E::FunctionAdapter { value, .. }
            | E::InterfaceCoerce { value, .. } => self.expression(value, site, context)?,
            E::Field { base, .. } => self.expression(base, site, context)?,
            E::StructConstruct { fields, .. } | E::BitfieldConstruct { fields, .. } => {
                for value in fields {
                    self.expression(value, site, context)?;
                }
            }
            E::EnumConstruct { payloads, .. } | E::MachineConstruct { payloads, .. } => {
                for value in payloads {
                    self.expression(value, site, context)?;
                }
            }
            E::MachineTransition {
                source, payloads, ..
            } => {
                self.expression(source, site, context)?;
                for value in payloads {
                    self.expression(value, site, context)?;
                }
            }
            E::ListConstruct { elements } => {
                for value in elements {
                    self.expression(value, site, context)?;
                }
            }
            E::MapConstruct { entries } => {
                for value in entries {
                    self.expression(&value.key, site, context)?;
                    self.expression(&value.value, site, context)?;
                }
            }
            E::StringInterpolation(parts) => {
                for part in parts {
                    if let hir::StringSegment::Value(value) = part {
                        self.expression(value, site, context)?;
                    }
                }
            }
            E::Handle {
                target, failure, ..
            } => {
                self.expression(target, site, context)?;
                self.hir_block(failure, site, context)?;
            }
            E::InlineFunction { body, .. } => {
                let saved = std::mem::replace(&mut self.iteration_scope_enabled, false);
                let result = self.hir_block(body, site, CheckedOwnershipContext::Ordinary);
                self.iteration_scope_enabled = saved;
                result?;
            }
            E::ActorMessage { actor, args, .. } => {
                self.expression(actor, site, context)?;
                for value in args {
                    self.expression(value, site, context)?;
                }
            }
            E::Int(_)
            | E::Float(_)
            | E::String(_)
            | E::Bool(_)
            | E::Nothing
            | E::Local(_)
            | E::Constant { .. }
            | E::ResourceHookValue { .. }
            | E::FunctionRef(_)
            | E::ClosureRef { .. }
            | E::OptionalNone
            | E::RuntimeFailure(_)
            | E::PropertyCaseContext(_) => {}
        }
        Ok(())
    }

    fn hir_block(
        &mut self,
        block: &'a hir::Block,
        site: Site,
        context: CheckedOwnershipContext,
    ) -> Result<(), String> {
        use hir::StatementKind as H;
        for statement in &block.statements {
            match &statement.kind {
                H::Let { value, .. }
                | H::Expression(value)
                | H::HandleDefault(value)
                | H::Respond(value) => self.expression(value, site, context)?,
                H::Assign { target, value } => {
                    self.expression(target, site, context)?;
                    self.expression(value, site, context)?;
                }
                H::Return(Some(value)) => self.expression(value, site, context)?,
                H::If {
                    condition,
                    then_block,
                    else_block,
                } => {
                    self.expression(condition, site, context)?;
                    self.hir_block(then_block, site, context)?;
                    if let Some(block) = else_block {
                        self.hir_block(block, site, context)?;
                    }
                }
                H::While { condition, body } => {
                    self.expression(condition, site, context)?;
                    self.hir_block(body, site, context)?;
                }
                H::For { iterable, body, .. } => {
                    self.expression(iterable, site, context)?;
                    self.hir_block(body, site, context)?;
                }
                H::Match { scrutinee, arms } => {
                    self.expression(scrutinee, site, context)?;
                    for arm in arms {
                        self.hir_block(&arm.body, site, context)?;
                    }
                }
                H::Assert { condition, message } => {
                    self.expression(condition, site, context)?;
                    if let Some(value) = message {
                        self.expression(value, site, context)?;
                    }
                }
                H::Breakpoint { condition, .. } => {
                    if let Some(value) = condition {
                        self.expression(
                            value,
                            site,
                            CheckedOwnershipContext::BreakpointExpression,
                        )?;
                    }
                }
                H::Scope(block) => self.hir_block(block, site, context)?,
                H::ReflectedTypeDispatch { type_info, arms } => {
                    self.expression(type_info, site, context)?;
                    for arm in arms {
                        self.hir_block(&arm.body, site, context)?;
                    }
                }
                H::Return(None) | H::Break | H::Continue | H::Trace(_) => {}
            }
        }
        Ok(())
    }
}

fn consumes(effect: CheckedCallerEffect) -> bool {
    matches!(
        effect,
        CheckedCallerEffect::TransferOwned | CheckedCallerEffect::RelinquishOwned
    )
}

fn valid_type(types: &TypeInterner, ty: TypeId) -> bool {
    (ty.index() as usize) < types.len() && ty != TypeInterner::ERROR
}

fn contains(outer: Span, inner: Span) -> bool {
    outer.file == inner.file && outer.start <= inner.start && inner.end <= outer.end
}

fn address_root(mut expression: &Expression) -> Option<LocalId> {
    loop {
        expression = match &expression.kind {
            E::Local(local) => return Some(*local),
            E::Field { base, .. } => base,
            E::View(value)
            | E::Coarsen(value)
            | E::Declassify(value)
            | E::RefinementValidated(value) => value,
            E::InterfaceCoerce { value, adapters } if adapters.is_empty() => value,
            _ => return None,
        };
    }
}

fn successors(terminator: &T) -> Vec<BlockId> {
    match terminator {
        T::Goto(target) => vec![*target],
        T::Branch {
            then_block,
            else_block,
            ..
        } => vec![*then_block, *else_block],
        T::Switch {
            variants,
            otherwise,
            ..
        } => variants
            .iter()
            .map(|(_, target, _)| *target)
            .chain(otherwise.iter().copied())
            .collect(),
        T::ForEach { body, exit, .. } => vec![*body, *exit],
        T::ReflectedTypeDispatch {
            arms, otherwise, ..
        } => arms
            .iter()
            .map(|arm| arm.target)
            .chain(std::iter::once(*otherwise))
            .collect(),
        T::Return(_) | T::Respond(_) | T::Unreachable => Vec::new(),
    }
}
