//! Caller authority retained separately from physical invocation operands.
use jett_common::Span;
use jett_typecheck::{
    CheckedCalleeAccess, CheckedCallerEffect, CheckedCallerSyntax, CheckedInvocationShape,
    CheckedOwnershipContext,
};
use jett_types::{Type, TypeId, TypeInterner};

use crate::{Expression, ExpressionKind, FunctionId, FunctionIdentity, IntrinsicId, LocalId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallerViewSource {
    Local(LocalId),
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallerBindingMode {
    Owned,
    View { source: CallerViewSource },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallerBindingFact {
    pub local: LocalId,
    pub declaration_span: Span,
    pub ty: TypeId,
    pub mode: CallerBindingMode,
    pub mutable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallerOrigin {
    Binding(CallerBindingFact),
    BorrowedProjection { source: CallerViewSource },
    OwnedFieldCopy { parent: CallerViewSource },
    OwnedExpression,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgumentOwnership {
    pub source_span: Span,
    pub source_index: usize,
    pub parameter_index: usize,
    pub actual_type: TypeId,
    pub parameter_type: TypeId,
    pub syntax: CheckedCallerSyntax,
    pub origin: CallerOrigin,
    pub callee_access: CheckedCalleeAccess,
    /// Actual implementation access; a validated helper bridge may differ.
    pub physical_access: CheckedCalleeAccess,
    pub effect: CheckedCallerEffect,
    pub staging: ArgumentStaging,
    /// Immutable source authority. MIR may stage an operand, never rewrite this witness.
    witness: SourceArgumentWitness,
    retained_snapshot: Option<RetainedSnapshotProof>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceArgumentWitness {
    source_span: Span,
    source_index: usize,
    parameter_index: usize,
    actual_type: TypeId,
    occurrence_type: TypeId,
    parameter_type: TypeId,
    syntax: CheckedCallerSyntax,
    origin: CallerOrigin,
    callee_access: CheckedCalleeAccess,
    effect: CheckedCallerEffect,
    context: CheckedOwnershipContext,
    observation: Option<ObservationProof>,
    handled_result: Option<SourceHandledResult>,
    projection_root: Option<SourceProjectionRoot>,
}

/// Original checked flow occurrence, separate from the owner's stored type.
/// Only the Source lowerer can mint this; it grants no alias or loan authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SourceProjectionRoot {
    local: LocalId,
    span: Span,
    occurrence_type: TypeId,
    storage_type: TypeId,
}

fn retained_snapshot_origin(origin: &CallerOrigin) -> bool {
    matches!(
        origin,
        CallerOrigin::Binding(_)
            | CallerOrigin::BorrowedProjection {
                source: CallerViewSource::Local(_)
            }
    )
}

/// Checked ordinary-data endpoint permission, independent of caller consumption.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RetainedSnapshotProof {
    actual_type: TypeId,
    occurrence_type: TypeId,
    /// Exact initial HIR header; pipeline View and raw source spans differ.
    physical_span: Span,
}

/// Exact original Handle backing; it grants no CFG acquisition authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SourceHandledResult {
    ty: TypeId,
    span: Span,
}

/// Checked capture ownership, separate from a function's parameter/result types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationProof {
    actual_type: TypeId,
    captures: Vec<(LocalId, TypeId)>,
    descriptor_checked: bool,
}

impl ArgumentOwnership {
    pub fn source_witness(&self) -> &SourceArgumentWitness {
        &self.witness
    }
    pub fn observation_proof(&self) -> Option<&ObservationProof> {
        self.witness.observation.as_ref()
    }
    pub fn retained_snapshot_type(&self) -> Option<TypeId> {
        self.retained_snapshot.map(|proof| proof.occurrence_type)
    }
    pub fn retained_snapshot_span(&self) -> Option<Span> {
        self.retained_snapshot.map(|proof| proof.physical_span)
    }
}

impl SourceArgumentWitness {
    pub fn projection_root(&self) -> Option<(LocalId, Span, TypeId, TypeId)> {
        self.projection_root.map(|root| {
            (
                root.local,
                root.span,
                root.occurrence_type,
                root.storage_type,
            )
        })
    }
    pub fn occurrence_type(&self) -> TypeId {
        self.occurrence_type
    }
    pub fn syntax(&self) -> CheckedCallerSyntax {
        self.syntax
    }
    pub fn context(&self) -> CheckedOwnershipContext {
        self.context
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgumentStaging {
    Original,
    Transferred { owner: LocalId },
    Copied { value: LocalId },
    Borrowed { loan: LocalId },
    Relinquished { owner: LocalId, loan: LocalId },
    RetainedSnapshot { owner: LocalId, loan: LocalId },
    Observed { owner: LocalId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallTarget {
    ResourceHook(crate::ResourceHookRef),
    Function(FunctionId),
    /// Proven source facade identity; its implementation body need not survive pruning.
    Declaration {
        identity: FunctionIdentity,
        signature_type: TypeId,
    },
    Interface {
        function: FunctionId,
        interface_type: TypeId,
        method_index: usize,
    },
    Indirect {
        signature_type: TypeId,
    },
    Intrinsic(IntrinsicId),
}

/// An implementation helper does not change the original caller disposition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallBridge {
    ResourceHook {
        hook: crate::ResourceHookRef,
    },
    Direct,
    TrustedIntrinsic {
        intrinsic: IntrinsicId,
    },
    JsonSource {
        intrinsic: IntrinsicId,
        function: FunctionId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceCallOwnership {
    pub target: CallTarget,
    pub shape: CheckedInvocationShape,
    pub context: CheckedOwnershipContext,
    /// Formal order; source_index retains original lexical order.
    pub arguments: Vec<ArgumentOwnership>,
    pub bridge: CallBridge,
    /// HIR-added metadata tail. It is never counted as a source actual.
    pub generated_operands: Vec<GeneratedArgumentOwnership>,
    certificate: SourceCallCertificate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceCallCertificate {
    result_type: TypeId,
    generated_operands: Vec<GeneratedArgumentOwnership>,
    target: CallTarget,
    shape: CheckedInvocationShape,
    context: CheckedOwnershipContext,
    bridge: CallBridge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeneratedAcquisition {
    Copy,
    OwnedExpression,
    Borrow { source: CallerViewSource },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedArgumentOwnership {
    pub actual_type: TypeId,
    pub parameter_type: TypeId,
    pub callee_access: CheckedCalleeAccess,
    pub acquisition: GeneratedAcquisition,
    pub staging: GeneratedArgumentStaging,
    staging_certificate: GeneratedArgumentStaging,
    witness: GeneratedArgumentWitness,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneratedArgumentStaging {
    Existing { loan: Option<LocalId> },
    OwnedProducer { owner: LocalId, loan: LocalId },
    OrdinarySnapshot { owner: LocalId, loan: LocalId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedSnapshotProof {
    actual_type: TypeId,
}

impl GeneratedSnapshotProof {
    pub fn actual_type(&self) -> TypeId {
        self.actual_type
    }
}

/// Private original node identity. The exact discriminant is recorded even for
/// ordinary data leaves; a typed leaf is never interchangeable with a Clone.
#[derive(Debug, Clone, PartialEq, Eq)]
struct GeneratedProducerShape {
    kind: std::mem::Discriminant<ExpressionKind>,
    ty: TypeId,
    span: Span,
    identity: GeneratedProducerIdentity,
    backing: Vec<GeneratedProducerShape>,
    ordinary_handle_source_snapshot: bool,
    ordinary_local_snapshot: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum GeneratedProducerIdentity {
    ResourceHook(crate::ResourceHookRef),
    ResourceInvoke {
        hook: crate::ResourceHookRef,
        operands: Vec<(TypeId, Span)>,
        order: Vec<usize>,
    },
    Data,
    Transparent(GeneratedTransparentShape),
    Local(LocalId),
    Constant(Span),
    Function(FunctionId),
    Closure {
        function: FunctionId,
        captures: Vec<LocalId>,
    },
    Call {
        function: FunctionId,
        operands: Vec<(TypeId, Span)>,
        order: Vec<usize>,
    },
    Intrinsic {
        intrinsic: IntrinsicId,
        type_arguments: Vec<TypeId>,
        operands: Vec<(TypeId, Span)>,
        order: Vec<usize>,
    },
    Indirect {
        operands: Vec<(TypeId, Span)>,
        order: Vec<usize>,
    },
    Struct {
        owner: TypeId,
        order: Vec<usize>,
        validates_refinements: bool,
    },
    Bitfield {
        owner: TypeId,
        order: Vec<usize>,
        validates_widths: bool,
    },
    Enum {
        owner: TypeId,
        variant: crate::VariantId,
        order: Vec<usize>,
    },
    Machine {
        owner: TypeId,
        state: crate::StateId,
    },
    Transition {
        owner: TypeId,
        state: crate::StateId,
    },
    Field {
        owner: TypeId,
        field: crate::FieldId,
    },
    InterfaceCoerce(Vec<crate::InterfaceFunctionAdapter>),
    FunctionAdapter(FunctionId),
    Handle {
        kind: crate::HandleKind,
        error_local: Option<LocalId>,
        failure_span: Span,
    },
    Inline {
        params: Vec<LocalId>,
        view_params: Vec<LocalId>,
        local_floor: u32,
        body_span: Span,
    },
    ActorSpawn {
        constructor: Option<FunctionId>,
        actor_type: String,
        order: Vec<usize>,
    },
    ActorMessage {
        handler: FunctionId,
        message: String,
        order: Vec<usize>,
    },
    State(crate::StateId),
}

/// Created only at the canonical extractor's checked Inline-to-descriptor join.
#[derive(Debug, Clone)]
pub(crate) struct GeneratedInlineExtraction {
    original: GeneratedProducerShape,
    replacement: GeneratedProducerShape,
}

pub(crate) fn generated_inline_extraction(
    original: &Expression,
    replacement: &Expression,
) -> Option<GeneratedInlineExtraction> {
    if !matches!(original.kind, ExpressionKind::InlineFunction { .. })
        || !matches!(
            replacement.kind,
            ExpressionKind::FunctionRef(_) | ExpressionKind::ClosureRef { .. }
        )
        || original.ty != replacement.ty
        || original.span != replacement.span
    {
        return None;
    }
    Some(GeneratedInlineExtraction {
        original: GeneratedProducerShape::from_expression(original),
        replacement: GeneratedProducerShape::from_expression(replacement),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GeneratedTransparentShape {
    View,
    Coarsen,
    Declassify,
    RefinementValidated,
}

/// Immutable original node, usable only after the consumer authenticates a
/// canonical owning storage definition. It cannot mint or replace a witness.
#[derive(Debug, Clone, Copy)]
pub struct GeneratedOriginalShape<'a> {
    shape: &'a GeneratedProducerShape,
}

impl<'a> GeneratedOriginalShape<'a> {
    pub fn ty(&self) -> TypeId {
        self.shape.ty
    }
    pub fn span(&self) -> Span {
        self.shape.span
    }
    pub fn handle(&self) -> Option<GeneratedHandleShape<'a>> {
        matches!(
            self.shape.identity,
            GeneratedProducerIdentity::Handle { .. }
        )
        .then_some(GeneratedHandleShape { shape: self.shape })
    }
    pub fn validate_reconstructed(
        &self,
        value: &Expression,
        mut validate_storage: impl FnMut(&Expression, GeneratedOriginalShape<'a>) -> Result<(), String>,
    ) -> Result<(), String> {
        self.shape.validate_lowered(value, &mut validate_storage)
    }
    pub fn validate_existing_local_snapshot(
        &self,
        value: &Expression,
        mut validate_storage: impl FnMut(&Expression, GeneratedOriginalShape<'a>) -> Result<(), String>,
    ) -> Result<(), String> {
        if !self.shape.ordinary_local_snapshot {
            return Err(
                "call ownership generated node has no sealed existing Local snapshot permission"
                    .into(),
            );
        }
        let restored = self
            .shape
            .restore_local_snapshot(value)
            .ok_or("call ownership generated node is not its exact existing Local snapshot")?;
        self.shape
            .validate_lowered(&restored, &mut validate_storage)
    }
}

/// Read-only original Handle node supplied only during independently
/// authenticated MIR reconstruction. It cannot mint or replace a certificate.
#[derive(Debug, Clone, Copy)]
pub struct GeneratedHandleShape<'a> {
    shape: &'a GeneratedProducerShape,
}

impl<'a> GeneratedHandleShape<'a> {
    pub fn kind(&self) -> &'a crate::HandleKind {
        let GeneratedProducerIdentity::Handle { kind, .. } = &self.shape.identity else {
            unreachable!("private Handle shape");
        };
        kind
    }
    pub fn result_type(&self) -> TypeId {
        self.shape.ty
    }
    pub fn source_type(&self) -> TypeId {
        self.shape.backing[0].ty
    }
    pub fn span(&self) -> Span {
        self.shape.span
    }
    pub fn source_span(&self) -> Span {
        self.shape.backing[0].span
    }
    pub fn error_local(&self) -> Option<LocalId> {
        let GeneratedProducerIdentity::Handle { error_local, .. } = &self.shape.identity else {
            unreachable!("private Handle shape");
        };
        *error_local
    }
    pub fn failure_span(&self) -> Span {
        let GeneratedProducerIdentity::Handle { failure_span, .. } = &self.shape.identity else {
            unreachable!("private Handle shape");
        };
        *failure_span
    }
    pub fn validate_source_shape(&self, value: &Expression) -> Result<(), String> {
        let mut reject = |_: &Expression, _: GeneratedOriginalShape<'a>| {
            Err("call ownership generated Handle source requires its original tree".into())
        };
        self.validate_lowered_source_shape(value, &mut reject)
    }
    pub fn validate_lowered_source_shape(
        &self,
        value: &Expression,
        mut validate_handle: impl FnMut(&Expression, GeneratedOriginalShape<'a>) -> Result<(), String>,
    ) -> Result<(), String> {
        let original = &self.shape.backing[0];
        match original.validate_lowered(value, &mut validate_handle) {
            Ok(()) => Ok(()),
            Err(error) => {
                if !self.shape.ordinary_handle_source_snapshot {
                    return Err(error);
                }
                let Some(restored) = original.restore_local_snapshot(value) else {
                    return Err(error);
                };
                original.validate_lowered(&restored, &mut validate_handle)
            }
        }
    }
}

fn generated_backing_values(value: &Expression) -> Vec<&Expression> {
    use ExpressionKind as E;
    match &value.kind {
        E::IndirectCall { callee, .. } => vec![callee],
        E::Field { base, .. } => vec![base],
        E::Handle { target, .. } => vec![target],
        E::StructConstruct { fields, .. } | E::BitfieldConstruct { fields, .. } => {
            fields.iter().collect()
        }
        E::EnumConstruct { payloads, .. } | E::MachineConstruct { payloads, .. } => {
            payloads.iter().collect()
        }
        E::MachineTransition {
            source, payloads, ..
        } => std::iter::once(source.as_ref()).chain(payloads).collect(),
        E::Binary { left, right, .. } => vec![left, right],
        E::Unary { value, .. }
        | E::View(value)
        | E::Clone(value)
        | E::Coarsen(value)
        | E::Declassify(value)
        | E::RefinementValidated(value)
        | E::Run(value)
        | E::Join(value)
        | E::Cancel(value)
        | E::InterfaceType(value)
        | E::DisplayResult(value)
        | E::EquatableResult(value)
        | E::ResultOk(value)
        | E::ResultFail(value)
        | E::OptionalSome(value)
        | E::RuntimeFailureMessage(value)
        | E::Comptime { value, .. }
        | E::InterfaceCoerce { value, .. }
        | E::FunctionAdapter { value, .. }
        | E::StateIs { value, .. } => vec![value],
        E::ListConstruct { elements } => elements.iter().collect(),
        E::MapConstruct { entries } => entries
            .iter()
            .flat_map(|entry| [&entry.key, &entry.value])
            .collect(),
        E::StringInterpolation(parts) => parts
            .iter()
            .filter_map(|part| match part {
                crate::StringSegment::Value(value) => Some(value),
                _ => None,
            })
            .collect(),
        E::ActorSpawn { args, .. } => args.iter().collect(),
        E::ActorMessage { actor, args, .. } => {
            std::iter::once(actor.as_ref()).chain(args).collect()
        }
        E::Int(_)
        | E::Float(_)
        | E::String(_)
        | E::Bool(_)
        | E::Nothing
        | E::Local(_)
        | E::Constant { .. }
        | E::FunctionRef(_)
        | E::ResourceHookValue { .. }
        | E::ResourceInvoke { .. }
        | E::ClosureRef { .. }
        | E::Call { .. }
        | E::Intrinsic { .. }
        | E::OptionalNone
        | E::RuntimeFailure(_)
        | E::PropertyCaseContext(_)
        | E::InlineFunction { .. } => Vec::new(),
    }
}

impl GeneratedProducerShape {
    fn from_expression(value: &Expression) -> Self {
        use ExpressionKind as E;
        let occurrence =
            |values: &[Expression]| values.iter().map(|value| (value.ty, value.span)).collect();
        let mut backing = Vec::new();
        let mut add = |value: &Expression| backing.push(Self::from_expression(value));
        let identity = match &value.kind {
            E::Local(local) => GeneratedProducerIdentity::Local(*local),
            E::Constant { declaration } => GeneratedProducerIdentity::Constant(*declaration),
            E::FunctionRef(function) => GeneratedProducerIdentity::Function(*function),
            E::ResourceHookValue { hook } => GeneratedProducerIdentity::ResourceHook(hook.clone()),
            E::ResourceInvoke {
                hook,
                args,
                evaluation_order,
                ..
            } => GeneratedProducerIdentity::ResourceInvoke {
                hook: hook.clone(),
                operands: occurrence(args),
                order: evaluation_order.clone(),
            },
            E::ClosureRef { function, captures } => GeneratedProducerIdentity::Closure {
                function: *function,
                captures: captures.clone(),
            },
            // Invocation arguments have their own independently sealed packets.
            // Retain their exact typed occurrences, not rewritten storage trees.
            E::Call {
                function,
                args,
                evaluation_order,
                ..
            } => GeneratedProducerIdentity::Call {
                function: *function,
                operands: occurrence(args),
                order: evaluation_order.clone(),
            },
            E::Intrinsic {
                intrinsic,
                type_arguments,
                args,
                evaluation_order,
                ..
            } => GeneratedProducerIdentity::Intrinsic {
                intrinsic: *intrinsic,
                type_arguments: type_arguments.clone(),
                operands: occurrence(args),
                order: evaluation_order.clone(),
            },
            E::IndirectCall {
                callee,
                args,
                evaluation_order,
                ..
            } => {
                add(callee);
                GeneratedProducerIdentity::Indirect {
                    operands: occurrence(args),
                    order: evaluation_order.clone(),
                }
            }
            E::StructConstruct {
                struct_type,
                fields,
                evaluation_order,
                validates_refinements,
                ..
            } => {
                for value in fields {
                    add(value);
                }
                GeneratedProducerIdentity::Struct {
                    owner: *struct_type,
                    order: evaluation_order.clone(),
                    validates_refinements: *validates_refinements,
                }
            }
            E::BitfieldConstruct {
                bitfield_type,
                fields,
                evaluation_order,
                validates_widths,
            } => {
                for value in fields {
                    add(value);
                }
                GeneratedProducerIdentity::Bitfield {
                    owner: *bitfield_type,
                    order: evaluation_order.clone(),
                    validates_widths: *validates_widths,
                }
            }
            E::EnumConstruct {
                enum_type,
                variant,
                payloads,
                evaluation_order,
            } => {
                for value in payloads {
                    add(value);
                }
                GeneratedProducerIdentity::Enum {
                    owner: *enum_type,
                    variant: *variant,
                    order: evaluation_order.clone(),
                }
            }
            E::MachineConstruct {
                state_type,
                state,
                payloads,
            } => {
                for value in payloads {
                    add(value);
                }
                GeneratedProducerIdentity::Machine {
                    owner: *state_type,
                    state: *state,
                }
            }
            E::MachineTransition {
                source,
                state_type,
                target,
                payloads,
            } => {
                add(source);
                for value in payloads {
                    add(value);
                }
                GeneratedProducerIdentity::Transition {
                    owner: *state_type,
                    state: *target,
                }
            }
            E::Field {
                base,
                owner_type,
                field,
            } => {
                add(base);
                GeneratedProducerIdentity::Field {
                    owner: *owner_type,
                    field: *field,
                }
            }
            E::InterfaceCoerce { value, adapters } => {
                add(value);
                GeneratedProducerIdentity::InterfaceCoerce(adapters.clone())
            }
            E::FunctionAdapter { value, function } => {
                add(value);
                GeneratedProducerIdentity::FunctionAdapter(*function)
            }
            E::Handle {
                target,
                kind,
                error_local,
                failure,
            } => {
                add(target);
                GeneratedProducerIdentity::Handle {
                    kind: kind.clone(),
                    error_local: *error_local,
                    failure_span: failure.span,
                }
            }
            E::InlineFunction {
                params,
                view_params,
                local_floor,
                body,
                ..
            } => GeneratedProducerIdentity::Inline {
                params: params.clone(),
                view_params: view_params.clone(),
                local_floor: *local_floor,
                body_span: body.span,
            },
            E::ActorSpawn {
                actor_type,
                args,
                evaluation_order,
                constructor,
            } => {
                for value in args {
                    add(value);
                }
                GeneratedProducerIdentity::ActorSpawn {
                    constructor: *constructor,
                    actor_type: actor_type.clone(),
                    order: evaluation_order.clone(),
                }
            }
            E::ActorMessage {
                actor,
                message,
                handler,
                args,
                evaluation_order,
                ..
            } => {
                add(actor);
                for value in args {
                    add(value);
                }
                GeneratedProducerIdentity::ActorMessage {
                    handler: *handler,
                    message: message.clone(),
                    order: evaluation_order.clone(),
                }
            }
            E::StateIs { value, state } => {
                add(value);
                GeneratedProducerIdentity::State(*state)
            }
            E::Binary { left, right, .. } => {
                add(left);
                add(right);
                GeneratedProducerIdentity::Data
            }
            E::View(value) => {
                add(value);
                GeneratedProducerIdentity::Transparent(GeneratedTransparentShape::View)
            }
            E::Coarsen(value) => {
                add(value);
                GeneratedProducerIdentity::Transparent(GeneratedTransparentShape::Coarsen)
            }
            E::Declassify(value) => {
                add(value);
                GeneratedProducerIdentity::Transparent(GeneratedTransparentShape::Declassify)
            }
            E::RefinementValidated(value) => {
                add(value);
                GeneratedProducerIdentity::Transparent(
                    GeneratedTransparentShape::RefinementValidated,
                )
            }
            E::Unary { value, .. }
            | E::Clone(value)
            | E::Run(value)
            | E::Join(value)
            | E::Cancel(value)
            | E::InterfaceType(value)
            | E::DisplayResult(value)
            | E::EquatableResult(value)
            | E::ResultOk(value)
            | E::ResultFail(value)
            | E::OptionalSome(value)
            | E::RuntimeFailureMessage(value)
            | E::Comptime { value, .. } => {
                add(value);
                GeneratedProducerIdentity::Data
            }
            E::ListConstruct { elements } => {
                for value in elements {
                    add(value);
                }
                GeneratedProducerIdentity::Data
            }
            E::MapConstruct { entries } => {
                for entry in entries {
                    add(&entry.key);
                    add(&entry.value);
                }
                GeneratedProducerIdentity::Data
            }
            E::StringInterpolation(parts) => {
                for part in parts {
                    if let crate::StringSegment::Value(value) = part {
                        add(value);
                    }
                }
                GeneratedProducerIdentity::Data
            }
            E::Int(_)
            | E::Float(_)
            | E::String(_)
            | E::Bool(_)
            | E::Nothing
            | E::OptionalNone
            | E::RuntimeFailure(_)
            | E::PropertyCaseContext(_) => GeneratedProducerIdentity::Data,
        };
        Self {
            kind: std::mem::discriminant(&value.kind),
            ty: value.ty,
            span: value.span,
            identity,
            backing,
            ordinary_handle_source_snapshot: false,
            ordinary_local_snapshot: false,
        }
    }

    fn apply_inline_extractions(&mut self, records: &[GeneratedInlineExtraction]) {
        if matches!(self.identity, GeneratedProducerIdentity::Inline { .. })
            && let Some(record) = records.iter().find(|record| record.original == *self)
        {
            *self = record.replacement.clone();
            return;
        }
        for value in &mut self.backing {
            value.apply_inline_extractions(records);
        }
    }

    fn certify_handle_sources(&mut self, types: &TypeInterner) {
        self.ordinary_local_snapshot =
            self.local_snapshot_backing() && observation_data_type(types, self.ty);
        if matches!(self.identity, GeneratedProducerIdentity::Handle { .. }) {
            let target = &self.backing[0];
            self.ordinary_handle_source_snapshot =
                target.local_snapshot_backing() && observation_data_type(types, target.ty);
        }
        for value in &mut self.backing {
            value.certify_handle_sources(types);
        }
    }

    fn local_snapshot_backing(&self) -> bool {
        match self.identity {
            GeneratedProducerIdentity::Local(_) => true,
            GeneratedProducerIdentity::InterfaceCoerce(_) => {
                self.backing[0].local_snapshot_backing()
            }
            _ => false,
        }
    }

    fn restore_local_snapshot(&self, value: &Expression) -> Option<Expression> {
        match (&self.identity, &value.kind) {
            (GeneratedProducerIdentity::Local(_), ExpressionKind::Clone(inner))
                if value.ty == self.ty
                    && value.span == self.span
                    && inner.ty == self.ty
                    && inner.span == self.span =>
            {
                Some(inner.as_ref().clone())
            }
            (
                GeneratedProducerIdentity::InterfaceCoerce(expected),
                ExpressionKind::InterfaceCoerce {
                    value: inner,
                    adapters,
                },
            ) if adapters == expected && value.ty == self.ty && value.span == self.span => {
                let mut restored = value.clone();
                let ExpressionKind::InterfaceCoerce { value, .. } = &mut restored.kind else {
                    unreachable!()
                };
                *value = Box::new(self.backing[0].restore_local_snapshot(inner)?);
                Some(restored)
            }
            _ => None,
        }
    }

    fn validate_lowered<'a>(
        &'a self,
        value: &Expression,
        validate_handle: &mut impl FnMut(&Expression, GeneratedOriginalShape<'a>) -> Result<(), String>,
    ) -> Result<(), String> {
        let actual = Self::from_expression(value);
        if self.kind != actual.kind
            || self.ty != actual.ty
            || self.span != actual.span
            || self.identity != actual.identity
            || self.backing.len() != actual.backing.len()
        {
            if matches!(value.kind, ExpressionKind::Local(_))
                && value.ty == self.ty
                && value.span == self.span
            {
                return validate_handle(value, GeneratedOriginalShape { shape: self });
            }
            return Err(
                "call ownership generated operand changes its sealed original producer shape"
                    .into(),
            );
        }
        for (expected, value) in self.backing.iter().zip(generated_backing_values(value)) {
            expected.validate_lowered(value, validate_handle)?;
        }
        Ok(())
    }

    fn metadata_types(&self, visit: &mut impl FnMut(TypeId)) {
        visit(self.ty);
        match &self.identity {
            GeneratedProducerIdentity::Call { operands, .. }
            | GeneratedProducerIdentity::Indirect { operands, .. }
            | GeneratedProducerIdentity::Intrinsic { operands, .. }
            | GeneratedProducerIdentity::ResourceInvoke { operands, .. } => {
                for &(ty, _) in operands {
                    visit(ty);
                }
            }
            _ => {}
        }
        match &self.identity {
            GeneratedProducerIdentity::ResourceHook(hook)
            | GeneratedProducerIdentity::ResourceInvoke { hook, .. } => {
                hook.metadata_types(&mut *visit)
            }
            GeneratedProducerIdentity::Intrinsic { type_arguments, .. } => {
                for &ty in type_arguments {
                    visit(ty);
                }
            }
            GeneratedProducerIdentity::Struct { owner, .. }
            | GeneratedProducerIdentity::Bitfield { owner, .. }
            | GeneratedProducerIdentity::Enum { owner, .. }
            | GeneratedProducerIdentity::Machine { owner, .. }
            | GeneratedProducerIdentity::Transition { owner, .. }
            | GeneratedProducerIdentity::Field { owner, .. } => visit(*owner),
            GeneratedProducerIdentity::InterfaceCoerce(adapters) => {
                for adapter in adapters {
                    visit(adapter.source);
                    visit(adapter.target);
                }
            }
            GeneratedProducerIdentity::Handle {
                kind:
                    crate::HandleKind::Refinement {
                        refined_type,
                        predicates,
                    },
                ..
            } => {
                visit(*refined_type);
                for predicate in predicates {
                    visit(predicate.refined_type);
                    visit(predicate.base_type);
                    visit(predicate.input_type);
                }
            }
            _ => {}
        }
        for value in &self.backing {
            value.metadata_types(visit);
        }
    }

    fn metadata_locals(&self, visit: &mut impl FnMut(LocalId)) {
        match &self.identity {
            GeneratedProducerIdentity::Local(local) => visit(*local),
            GeneratedProducerIdentity::Closure { captures, .. } => {
                for &local in captures {
                    visit(local);
                }
            }
            GeneratedProducerIdentity::Handle {
                error_local: Some(local),
                ..
            } => visit(*local),
            GeneratedProducerIdentity::Inline {
                params,
                view_params,
                ..
            } => {
                for &local in params.iter().chain(view_params) {
                    visit(local);
                }
            }
            _ => {}
        }
        for value in &self.backing {
            value.metadata_locals(visit);
        }
    }

    fn remap_locals<E>(
        &mut self,
        map: &mut impl FnMut(LocalId) -> Result<LocalId, E>,
    ) -> Result<(), E> {
        match &mut self.identity {
            GeneratedProducerIdentity::Local(local) => *local = map(*local)?,
            GeneratedProducerIdentity::Closure { captures, .. } => {
                for local in captures {
                    *local = map(*local)?;
                }
            }
            GeneratedProducerIdentity::Handle {
                error_local: Some(local),
                ..
            } => *local = map(*local)?,
            GeneratedProducerIdentity::Inline {
                params,
                view_params,
                ..
            } => {
                for local in params.iter_mut().chain(view_params) {
                    *local = map(*local)?;
                }
            }
            _ => {}
        }
        for value in &mut self.backing {
            value.remap_locals(map)?;
        }
        Ok(())
    }

    fn metadata_functions(&self, visit: &mut impl FnMut(FunctionId)) {
        match &self.identity {
            GeneratedProducerIdentity::Function(function)
            | GeneratedProducerIdentity::FunctionAdapter(function)
            | GeneratedProducerIdentity::Closure { function, .. }
            | GeneratedProducerIdentity::Call { function, .. }
            | GeneratedProducerIdentity::ActorMessage {
                handler: function, ..
            } => visit(*function),
            GeneratedProducerIdentity::InterfaceCoerce(adapters) => {
                for adapter in adapters {
                    visit(adapter.function);
                }
            }
            GeneratedProducerIdentity::ActorSpawn {
                constructor: Some(function),
                ..
            } => visit(*function),
            GeneratedProducerIdentity::Handle {
                kind: crate::HandleKind::Refinement { predicates, .. },
                ..
            } => {
                for predicate in predicates {
                    visit(predicate.function);
                }
            }
            _ => {}
        }
        for value in &self.backing {
            value.metadata_functions(visit);
        }
    }

    fn remap_functions<E>(
        &mut self,
        map: &mut impl FnMut(FunctionId) -> Result<FunctionId, E>,
    ) -> Result<(), E> {
        match &mut self.identity {
            GeneratedProducerIdentity::Function(function)
            | GeneratedProducerIdentity::FunctionAdapter(function)
            | GeneratedProducerIdentity::Closure { function, .. }
            | GeneratedProducerIdentity::Call { function, .. }
            | GeneratedProducerIdentity::ActorMessage {
                handler: function, ..
            } => *function = map(*function)?,
            GeneratedProducerIdentity::InterfaceCoerce(adapters) => {
                for adapter in adapters {
                    adapter.function = map(adapter.function)?;
                }
            }
            GeneratedProducerIdentity::ActorSpawn {
                constructor: Some(function),
                ..
            } => *function = map(*function)?,
            GeneratedProducerIdentity::Handle {
                kind: crate::HandleKind::Refinement { predicates, .. },
                ..
            } => {
                for predicate in predicates {
                    predicate.function = map(predicate.function)?;
                }
            }
            _ => {}
        }
        for value in &mut self.backing {
            value.remap_functions(map)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedArgumentWitness {
    actual_type: TypeId,
    parameter_type: TypeId,
    callee_access: CheckedCalleeAccess,
    acquisition: GeneratedAcquisition,
    origin: CallerViewSource,
    source_span: Span,
    written_view: bool,
    owned_producer: bool,
    producer_shape: GeneratedProducerShape,
    ordinary_snapshot: Option<GeneratedSnapshotProof>,
}

impl GeneratedArgumentWitness {
    pub fn actual_type(&self) -> TypeId {
        self.actual_type
    }
    pub fn parameter_type(&self) -> TypeId {
        self.parameter_type
    }
    pub fn callee_access(&self) -> CheckedCalleeAccess {
        self.callee_access
    }
    pub fn acquisition(&self) -> &GeneratedAcquisition {
        &self.acquisition
    }
    pub fn origin(&self) -> CallerViewSource {
        self.origin
    }
    pub fn source_span(&self) -> Span {
        self.source_span
    }
    pub fn written_view(&self) -> bool {
        self.written_view
    }
    pub fn owned_producer(&self) -> bool {
        self.owned_producer
    }
    /// Compare an authenticated restored operand with the immutable original
    /// operation/backing shape; matching a type or producer flag is insufficient.
    pub fn validate_producer_shape(&self, value: &Expression) -> Result<(), String> {
        self.producer_shape.validate_lowered(value, &mut |_, _| {
            Err("call ownership original generated operand cannot be a lowered storage slot".into())
        })
    }
    /// A storage substitution needs an independently authenticated unique
    /// owning Let; original Handles additionally require their exact CFG proof.
    pub fn validate_lowered_producer_shape<'a>(
        &'a self,
        value: &Expression,
        mut validate_handle: impl FnMut(&Expression, GeneratedOriginalShape<'a>) -> Result<(), String>,
    ) -> Result<(), String> {
        self.producer_shape
            .validate_lowered(value, &mut validate_handle)
    }
    pub fn ordinary_snapshot_proof(&self) -> Option<&GeneratedSnapshotProof> {
        self.ordinary_snapshot.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GeneratedCallCertificate {
    operation: GeneratedOperation,
    result_type: TypeId,
    evaluation_order: Vec<usize>,
    arguments: Vec<GeneratedArgumentWitness>,
}

fn generated_owned_producer(mut value: &Expression) -> bool {
    loop {
        match &value.kind {
            ExpressionKind::View(inner)
            | ExpressionKind::Coarsen(inner)
            | ExpressionKind::Declassify(inner)
            | ExpressionKind::RefinementValidated(inner) => value = inner,
            ExpressionKind::Call { .. }
            | ExpressionKind::IndirectCall { .. }
            | ExpressionKind::Intrinsic { .. }
            | ExpressionKind::StructConstruct { .. }
            | ExpressionKind::BitfieldConstruct { .. }
            | ExpressionKind::MachineConstruct { .. }
            | ExpressionKind::MachineTransition { .. }
            | ExpressionKind::Clone(_)
            | ExpressionKind::Run(_)
            | ExpressionKind::Join(_) => return true,
            _ => return false,
        }
    }
}

fn generated_witness(
    value: &Expression,
    parameter_type: TypeId,
    callee_access: CheckedCalleeAccess,
    acquisition: GeneratedAcquisition,
    types: &TypeInterner,
) -> GeneratedArgumentWitness {
    let mut producer_shape = GeneratedProducerShape::from_expression(value);
    producer_shape.certify_handle_sources(types);
    GeneratedArgumentWitness {
        actual_type: value.ty,
        parameter_type,
        callee_access,
        acquisition,
        origin: immediate_source(value),
        source_span: value.span,
        written_view: original_view(value),
        owned_producer: generated_owned_producer(value),
        producer_shape,
        ordinary_snapshot: observation_data_type(types, value.ty).then_some(
            GeneratedSnapshotProof {
                actual_type: value.ty,
            },
        ),
    }
}

fn generated_tuple_matches(argument: &GeneratedArgumentOwnership) -> bool {
    let witness = &argument.witness;
    argument.actual_type == witness.actual_type
        && argument.parameter_type == witness.parameter_type
        && argument.callee_access == witness.callee_access
        && argument.acquisition == witness.acquisition
}

fn generated_operation_function(operation: &GeneratedOperation) -> Option<FunctionId> {
    match operation {
        GeneratedOperation::Display { method } | GeneratedOperation::Equality { method } => {
            Some(*method)
        }
        GeneratedOperation::InterfaceDispatch { function, .. }
        | GeneratedOperation::NativeSuite { function }
        | GeneratedOperation::RefinementPredicate { function } => Some(*function),
        _ => None,
    }
}

fn remap_generated_operation<E>(
    operation: &mut GeneratedOperation,
    map: &mut impl FnMut(FunctionId) -> Result<FunctionId, E>,
) -> Result<(), E> {
    match operation {
        GeneratedOperation::Display { method } | GeneratedOperation::Equality { method } => {
            *method = map(*method)?
        }
        GeneratedOperation::InterfaceDispatch { function, .. }
        | GeneratedOperation::NativeSuite { function }
        | GeneratedOperation::RefinementPredicate { function } => *function = map(*function)?,
        _ => {}
    }
    Ok(())
}

fn remap_generated_operation_locals<E>(
    operation: &mut GeneratedOperation,
    map: &mut impl FnMut(LocalId) -> Result<LocalId, E>,
) -> Result<(), E> {
    if let GeneratedOperation::FunctionAdapter { callee, .. } = operation {
        *callee = map(*callee)?;
    }
    Ok(())
}

fn validate_generated_stage(
    argument: &GeneratedArgumentOwnership,
    argument_type: TypeId,
    local: &impl Fn(LocalId) -> Option<OwnershipLocalInfo>,
    initial_hir: bool,
) -> Result<(), String> {
    if !generated_tuple_matches(argument) {
        return Err("generated ownership differs from its sealed original acquisition".into());
    }
    if argument.staging != argument.staging_certificate {
        return Err("generated stage differs from its sealed storage association".into());
    }
    if initial_hir && argument.staging != (GeneratedArgumentStaging::Existing { loan: None }) {
        return Err("initial HIR generated ownership cannot carry MIR staging authority".into());
    }
    let (owner, loan) = match argument.staging {
        GeneratedArgumentStaging::Existing { loan: None } => return Ok(()),
        GeneratedArgumentStaging::Existing { loan: Some(loan) } => (None, loan),
        GeneratedArgumentStaging::OwnedProducer { owner, loan } => {
            if !argument.witness.owned_producer {
                return Err("generated stage has no sealed owning producer proof".into());
            }
            (Some(owner), loan)
        }
        GeneratedArgumentStaging::OrdinarySnapshot { owner, loan } => {
            if argument
                .witness
                .ordinary_snapshot
                .as_ref()
                .map(|proof| proof.actual_type)
                != Some(argument.actual_type)
            {
                return Err("generated stage has no sealed ordinary snapshot proof".into());
            }
            (Some(owner), loan)
        }
    };
    if argument.callee_access != CheckedCalleeAccess::View
        || !matches!(
            argument.acquisition,
            GeneratedAcquisition::Borrow { .. } | GeneratedAcquisition::Copy
        )
    {
        return Err("generated loan stage has no original physically borrowed acquisition".into());
    }
    if require_local(loan, local)?.ty != argument_type || argument_type != argument.actual_type {
        return Err("generated loan stage changed its sealed operand type".into());
    }
    if let Some(owner) = owner {
        let owner = require_local(owner, local)?;
        if owner.ty != argument_type
            || owner.view_source.is_some()
            || owner.is_view_parameter
            || owner.view_iteration.is_some()
        {
            return Err("generated backing stage cannot acquire a borrowed slot".into());
        }
    }
    Ok(())
}

fn generated_stage_local_ids(staging: GeneratedArgumentStaging, visit: &mut impl FnMut(LocalId)) {
    match staging {
        GeneratedArgumentStaging::Existing { loan: Some(loan) } => visit(loan),
        GeneratedArgumentStaging::OwnedProducer { owner, loan }
        | GeneratedArgumentStaging::OrdinarySnapshot { owner, loan } => {
            visit(owner);
            visit(loan);
        }
        GeneratedArgumentStaging::Existing { loan: None } => {}
    }
}

fn remap_generated_staging<E>(
    staging: &mut GeneratedArgumentStaging,
    map: &mut impl FnMut(LocalId) -> Result<LocalId, E>,
) -> Result<(), E> {
    match staging {
        GeneratedArgumentStaging::Existing { loan: Some(loan) } => *loan = map(*loan)?,
        GeneratedArgumentStaging::OwnedProducer { owner, loan }
        | GeneratedArgumentStaging::OrdinarySnapshot { owner, loan } => {
            *owner = map(*owner)?;
            *loan = map(*loan)?;
        }
        GeneratedArgumentStaging::Existing { loan: None } => {}
    }
    Ok(())
}

/// Each operation is validated against its own existing compiler contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeneratedOperation {
    Display {
        method: FunctionId,
    },
    Equality {
        method: FunctionId,
    },
    InterfaceDispatch {
        function: FunctionId,
        interface_type: TypeId,
        method_index: usize,
    },
    FunctionAdapter {
        callee: LocalId,
        source_type: TypeId,
        target_type: TypeId,
    },
    JsonBridge {
        intrinsic: IntrinsicId,
    },
    NativeSuite {
        function: FunctionId,
    },
    RefinementPredicate {
        function: FunctionId,
    },
    FailureFormatting,
    PropertyContext,
    ReflectionMetadata {
        intrinsic: IntrinsicId,
    },
    EvaluatedValue {
        intrinsic: IntrinsicId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedCallOwnership {
    pub operation: GeneratedOperation,
    pub arguments: Vec<GeneratedArgumentOwnership>,
    certificate: GeneratedCallCertificate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallOwnership {
    Source(SourceCallOwnership),
    Generated(GeneratedCallOwnership),
}

impl CallOwnership {
    pub fn metadata_types(&self, mut visit: impl FnMut(TypeId)) {
        match self {
            Self::Source(source) => {
                visit(source.certificate.result_type);
                match &source.shape {
                    CheckedInvocationShape::Function { signature_type } => visit(*signature_type),
                    CheckedInvocationShape::Intrinsic {
                        result_type,
                        operands,
                        ..
                    } => {
                        visit(*result_type);
                        for role in operands {
                            visit(role.ty());
                        }
                    }
                }
                match &source.target {
                    CallTarget::Declaration {
                        identity,
                        signature_type,
                    } => {
                        visit(*signature_type);
                        for &ty in &identity.type_arguments {
                            visit(ty);
                        }
                        for binding in &identity.scoped_type_bindings {
                            visit(binding.ty);
                        }
                    }
                    CallTarget::ResourceHook(hook) => hook.metadata_types(&mut visit),
                    CallTarget::Interface { interface_type, .. } => visit(*interface_type),
                    CallTarget::Indirect { signature_type } => visit(*signature_type),
                    _ => {}
                }
                for argument in &source.arguments {
                    visit(argument.actual_type);
                    visit(argument.witness.occurrence_type);
                    if let Some(result) = argument.witness.handled_result {
                        visit(result.ty);
                    }
                    if let Some(root) = argument.witness.projection_root {
                        visit(root.occurrence_type);
                        visit(root.storage_type);
                    }
                    if let Some(proof) = argument.retained_snapshot {
                        visit(proof.actual_type);
                        visit(proof.occurrence_type);
                    }
                    visit(argument.parameter_type);
                    if let CallerOrigin::Binding(fact) = &argument.origin {
                        visit(fact.ty);
                    }
                    if let Some(proof) = argument.observation_proof() {
                        visit(proof.actual_type);
                        for &(_, ty) in &proof.captures {
                            visit(ty);
                        }
                    }
                }
                for argument in &source.generated_operands {
                    visit(argument.actual_type);
                    visit(argument.parameter_type);
                    argument.witness.producer_shape.metadata_types(&mut visit);
                }
            }
            Self::Generated(generated) => {
                visit(generated.certificate.result_type);
                match generated.operation {
                    GeneratedOperation::InterfaceDispatch { interface_type, .. } => {
                        visit(interface_type)
                    }
                    GeneratedOperation::FunctionAdapter {
                        source_type,
                        target_type,
                        ..
                    } => {
                        visit(source_type);
                        visit(target_type);
                    }
                    _ => {}
                }
                for argument in &generated.arguments {
                    visit(argument.actual_type);
                    visit(argument.parameter_type);
                    argument.witness.producer_shape.metadata_types(&mut visit);
                }
            }
        }
    }
    pub fn source_parameter_effect(&self, index: usize) -> Option<CheckedCallerEffect> {
        match self {
            Self::Source(source) => source.arguments.get(index).map(|arg| arg.effect),
            Self::Generated(_) => None,
        }
    }
    pub fn source_parameter_access(&self, index: usize) -> Option<CheckedCalleeAccess> {
        match self {
            Self::Source(source) => source.arguments.get(index).map(|arg| arg.callee_access),
            Self::Generated(_) => None,
        }
    }
    pub fn source_parameter_syntax(&self, index: usize) -> Option<CheckedCallerSyntax> {
        match self {
            Self::Source(source) => source.arguments.get(index).map(|arg| arg.syntax),
            Self::Generated(_) => None,
        }
    }
    pub fn parameter_count(&self) -> usize {
        match self {
            Self::Source(source) => source.arguments.len() + source.generated_operands.len(),
            Self::Generated(generated) => generated.arguments.len(),
        }
    }

    pub fn parameter_physical_access(&self, index: usize) -> Option<CheckedCalleeAccess> {
        match self {
            Self::Source(source) => source
                .arguments
                .get(index)
                .map(|arg| arg.physical_access)
                .or_else(|| {
                    index.checked_sub(source.arguments.len()).and_then(|tail| {
                        source
                            .generated_operands
                            .get(tail)
                            .map(|arg| arg.callee_access)
                    })
                }),
            Self::Generated(generated) => {
                generated.arguments.get(index).map(|arg| arg.callee_access)
            }
        }
    }

    pub fn stage_parameter(
        &mut self,
        index: usize,
        owner: Option<LocalId>,
        loan: LocalId,
    ) -> Result<(), String> {
        let Self::Source(source) = self else {
            return Err("generated operand cannot acquire source staging authority".into());
        };
        let argument = source
            .arguments
            .get_mut(index)
            .ok_or_else(|| "source staging parameter is out of bounds".to_string())?;
        if argument.staging != ArgumentStaging::Original
            || argument.physical_access != CheckedCalleeAccess::View
        {
            return Err("source staging requires an original physically borrowed operand".into());
        }
        argument.staging = match owner {
            Some(owner)
                if argument.effect == CheckedCallerEffect::RelinquishOwned
                    || (argument.effect == CheckedCallerEffect::TransferOwned
                        && source.bridge != CallBridge::Direct) =>
            {
                ArgumentStaging::Relinquished { owner, loan }
            }
            None if matches!(
                argument.effect,
                CheckedCallerEffect::RetainBorrow | CheckedCallerEffect::ObserveData
            ) =>
            {
                ArgumentStaging::Borrowed { loan }
            }
            _ => {
                return Err(
                    "source staging does not match the checked caller effect and bridge".into(),
                );
            }
        };
        Ok(())
    }

    pub fn stage_retained_snapshot(
        &mut self,
        index: usize,
        owner: LocalId,
        loan: LocalId,
    ) -> Result<(), String> {
        let Self::Source(source) = self else {
            return Err(
                "generated operand cannot acquire retained source snapshot authority".into(),
            );
        };
        let argument = source
            .arguments
            .get_mut(index)
            .ok_or("retained snapshot parameter is out of bounds")?;
        if argument.staging != ArgumentStaging::Original
            || argument.effect != CheckedCallerEffect::RetainBorrow
            || argument.physical_access != CheckedCalleeAccess::View
            || argument.retained_snapshot.is_none()
            || owner == loan
        {
            return Err("retained snapshot requires an original checked ordinary-data view".into());
        }
        argument.staging = ArgumentStaging::RetainedSnapshot { owner, loan };
        Ok(())
    }

    pub fn stage_observation(&mut self, index: usize, owner: LocalId) -> Result<(), String> {
        let Self::Source(source) = self else {
            return Err("generated operand cannot acquire source observation authority".into());
        };
        let argument = source
            .arguments
            .get_mut(index)
            .ok_or_else(|| "source observation parameter is out of bounds".to_string())?;
        if argument.staging != ArgumentStaging::Original
            || argument.effect != CheckedCallerEffect::ObserveData
            || argument.physical_access != CheckedCalleeAccess::Owned
        {
            return Err(
                "observation staging requires original ObserveData at a physical owned boundary"
                    .into(),
            );
        }
        argument.staging = ArgumentStaging::Observed { owner };
        Ok(())
    }

    pub fn stage_acquisition(&mut self, index: usize, local: LocalId) -> Result<(), String> {
        let Self::Source(source) = self else {
            return Err("generated operand cannot acquire source staging authority".into());
        };
        let argument = source
            .arguments
            .get_mut(index)
            .ok_or("source acquisition parameter is out of bounds")?;
        if argument.staging != ArgumentStaging::Original {
            return Err("source acquisition is already staged".into());
        }
        argument.staging = match (argument.effect, argument.physical_access) {
            (CheckedCallerEffect::TransferOwned, CheckedCalleeAccess::Owned) => {
                ArgumentStaging::Transferred { owner: local }
            }
            (CheckedCallerEffect::Copy, _) => ArgumentStaging::Copied { value: local },
            _ => {
                return Err(
                    "source acquisition does not match checked copy or physical ownership".into(),
                );
            }
        };
        Ok(())
    }

    pub fn stage_generated_parameter(
        &mut self,
        index: usize,
        staging: GeneratedArgumentStaging,
    ) -> Result<(), String> {
        let Self::Generated(generated) = self else {
            return Err("source packet cannot acquire generated staging authority".into());
        };
        if generated.operation != generated.certificate.operation {
            return Err("generated stage operation differs from its certificate".into());
        }
        if generated.arguments.len() != generated.certificate.arguments.len() {
            return Err("generated stage argument count differs from its certificate".into());
        }
        let witness = generated
            .certificate
            .arguments
            .get(index)
            .ok_or("generated stage certificate parameter is out of bounds")?;
        let argument = generated
            .arguments
            .get_mut(index)
            .ok_or("generated stage parameter is out of bounds")?;
        if !generated_tuple_matches(argument)
            || argument.witness != *witness
            || argument.staging != argument.staging_certificate
            || argument.staging != (GeneratedArgumentStaging::Existing { loan: None })
        {
            return Err(
                "generated stage cannot rewrite its original acquisition certificate".into(),
            );
        }
        if argument.callee_access != CheckedCalleeAccess::View
            || !matches!(
                argument.acquisition,
                GeneratedAcquisition::Borrow { .. } | GeneratedAcquisition::Copy
            )
        {
            return Err("generated stage requires a physically borrowed original operand".into());
        }
        match staging {
            GeneratedArgumentStaging::Existing { loan: Some(_) } => {}
            GeneratedArgumentStaging::OwnedProducer { .. } if argument.witness.owned_producer => {}
            GeneratedArgumentStaging::OrdinarySnapshot { .. }
                if argument.witness.ordinary_snapshot.is_some() => {}
            _ => return Err("generated stage has no sealed original backing permission".into()),
        }
        argument.staging = staging;
        argument.staging_certificate = staging;
        Ok(())
    }

    /// Structural metadata only: this must not feed execution liveness or loans.
    pub fn metadata_local_ids(&self, mut visit: impl FnMut(LocalId)) {
        match self {
            Self::Source(source) => {
                for argument in &source.arguments {
                    if let Some(root) = argument.witness.projection_root {
                        visit(root.local);
                    }
                    if let Some(proof) = argument.observation_proof() {
                        for &(capture, _) in &proof.captures {
                            visit(capture);
                        }
                    }
                    match argument.staging {
                        ArgumentStaging::Original => {}
                        ArgumentStaging::Transferred { owner } => visit(owner),
                        ArgumentStaging::Copied { value } => visit(value),
                        ArgumentStaging::Borrowed { loan } => visit(loan),
                        ArgumentStaging::Relinquished { owner, loan }
                        | ArgumentStaging::RetainedSnapshot { owner, loan } => {
                            visit(owner);
                            visit(loan);
                        }
                        ArgumentStaging::Observed { owner } => visit(owner),
                    }
                    match argument.origin {
                        CallerOrigin::Binding(fact) => {
                            visit(fact.local);
                            if let CallerBindingMode::View {
                                source: CallerViewSource::Local(id),
                            } = fact.mode
                            {
                                visit(id);
                            }
                        }
                        CallerOrigin::BorrowedProjection {
                            source: CallerViewSource::Local(id),
                        }
                        | CallerOrigin::OwnedFieldCopy {
                            parent: CallerViewSource::Local(id),
                        } => visit(id),
                        _ => {}
                    }
                }
                for operand in &source.generated_operands {
                    operand.metadata_local_ids(&mut visit);
                }
            }
            Self::Generated(generated) => {
                if let GeneratedOperation::FunctionAdapter { callee, .. } = generated.operation {
                    visit(callee);
                }
                for operand in &generated.arguments {
                    operand.metadata_local_ids(&mut visit);
                }
            }
        }
    }

    pub fn remap_metadata_locals<E>(
        &mut self,
        mut map: impl FnMut(LocalId) -> Result<LocalId, E>,
    ) -> Result<(), E> {
        match self {
            Self::Source(source) => {
                for argument in &mut source.arguments {
                    if let Some(root) = &mut argument.witness.projection_root {
                        root.local = map(root.local)?;
                    }
                    if let Some(proof) = &mut argument.witness.observation {
                        for (capture, _) in &mut proof.captures {
                            *capture = map(*capture)?;
                        }
                    }
                    remap_origin(&mut argument.witness.origin, &mut map)?;
                    match &mut argument.staging {
                        ArgumentStaging::Original => {}
                        ArgumentStaging::Transferred { owner } => *owner = map(*owner)?,
                        ArgumentStaging::Copied { value } => *value = map(*value)?,
                        ArgumentStaging::Borrowed { loan } => *loan = map(*loan)?,
                        ArgumentStaging::Relinquished { owner, loan }
                        | ArgumentStaging::RetainedSnapshot { owner, loan } => {
                            *owner = map(*owner)?;
                            *loan = map(*loan)?;
                        }
                        ArgumentStaging::Observed { owner } => *owner = map(*owner)?,
                    }
                    match &mut argument.origin {
                        CallerOrigin::Binding(fact) => {
                            fact.local = map(fact.local)?;
                            if let CallerBindingMode::View { source } = &mut fact.mode {
                                remap_source(source, &mut map)?;
                            }
                        }
                        CallerOrigin::BorrowedProjection { source }
                        | CallerOrigin::OwnedFieldCopy { parent: source } => {
                            remap_source(source, &mut map)?
                        }
                        CallerOrigin::OwnedExpression => {}
                    }
                }
                for operand in &mut source.generated_operands {
                    operand.remap_metadata_locals(&mut map)?;
                }
                for operand in &mut source.certificate.generated_operands {
                    operand.remap_metadata_locals(&mut map)?;
                }
            }
            Self::Generated(generated) => {
                for operand in &mut generated.arguments {
                    operand.remap_metadata_locals(&mut map)?;
                }
                remap_generated_operation_locals(&mut generated.operation, &mut map)?;
                remap_generated_operation_locals(&mut generated.certificate.operation, &mut map)?;
                for witness in &mut generated.certificate.arguments {
                    if let GeneratedAcquisition::Borrow { source } = &mut witness.acquisition {
                        remap_source(source, &mut map)?;
                    }
                    remap_source(&mut witness.origin, &mut map)?;
                    witness.producer_shape.remap_locals(&mut map)?;
                }
            }
        }
        Ok(())
    }

    /// Structural identities only; declaration provenance is not an executable root.
    pub fn metadata_function_ids(&self, mut visit: impl FnMut(FunctionId)) {
        match self {
            Self::Source(source) => {
                match source.target {
                    CallTarget::Function(function) | CallTarget::Interface { function, .. } => {
                        visit(function)
                    }
                    _ => {}
                }
                if let CallBridge::JsonSource { function, .. } = source.bridge {
                    visit(function);
                }
                for argument in &source.generated_operands {
                    argument
                        .witness
                        .producer_shape
                        .metadata_functions(&mut visit);
                }
            }
            Self::Generated(generated) => {
                if let Some(function) = generated_operation_function(&generated.operation) {
                    visit(function);
                }
                for argument in &generated.arguments {
                    argument
                        .witness
                        .producer_shape
                        .metadata_functions(&mut visit);
                }
            }
        }
    }

    pub fn remap_metadata_functions<E>(
        &mut self,
        mut map: impl FnMut(FunctionId) -> Result<FunctionId, E>,
    ) -> Result<(), E> {
        match self {
            Self::Source(source) => {
                match &mut source.target {
                    CallTarget::Function(function) | CallTarget::Interface { function, .. } => {
                        *function = map(*function)?
                    }
                    _ => {}
                }
                if let CallBridge::JsonSource { function, .. } = &mut source.bridge {
                    *function = map(*function)?;
                }
                match &mut source.certificate.target {
                    CallTarget::Function(function) | CallTarget::Interface { function, .. } => {
                        *function = map(*function)?
                    }
                    _ => {}
                }
                if let CallBridge::JsonSource { function, .. } = &mut source.certificate.bridge {
                    *function = map(*function)?;
                }
                for argument in &mut source.generated_operands {
                    argument.witness.producer_shape.remap_functions(&mut map)?;
                }
                for argument in &mut source.certificate.generated_operands {
                    argument.witness.producer_shape.remap_functions(&mut map)?;
                }
            }
            Self::Generated(generated) => {
                remap_generated_operation(&mut generated.operation, &mut map)?;
                remap_generated_operation(&mut generated.certificate.operation, &mut map)?;
                for argument in &mut generated.arguments {
                    argument.witness.producer_shape.remap_functions(&mut map)?;
                }
                for witness in &mut generated.certificate.arguments {
                    witness.producer_shape.remap_functions(&mut map)?;
                }
            }
        }
        Ok(())
    }

    pub(crate) fn apply_generated_inline_extractions(
        &mut self,
        records: &[GeneratedInlineExtraction],
    ) {
        match self {
            Self::Source(source) => {
                for argument in &mut source.generated_operands {
                    argument
                        .witness
                        .producer_shape
                        .apply_inline_extractions(records);
                }
                for argument in &mut source.certificate.generated_operands {
                    argument
                        .witness
                        .producer_shape
                        .apply_inline_extractions(records);
                }
            }
            Self::Generated(generated) => {
                for argument in &mut generated.arguments {
                    argument
                        .witness
                        .producer_shape
                        .apply_inline_extractions(records);
                }
                for witness in &mut generated.certificate.arguments {
                    witness.producer_shape.apply_inline_extractions(records);
                }
            }
        }
    }

    pub fn generated(
        operation: GeneratedOperation,
        args: &[Expression],
        parameter_types: &[TypeId],
        access: &[CheckedCalleeAccess],
        result_type: TypeId,
        evaluation_order: &[usize],
        types: &TypeInterner,
    ) -> Result<Self, String> {
        if !valid_type(types, result_type) {
            return Err("generated result type is outside its interner".into());
        }
        if evaluation_order.len() != args.len() {
            return Err("generated evaluation order has the wrong operand count".into());
        }
        let mut seen = vec![false; args.len()];
        for &index in evaluation_order {
            let slot = seen
                .get_mut(index)
                .ok_or("generated evaluation order is out of bounds")?;
            if *slot {
                return Err("generated evaluation order repeats an operand".into());
            }
            *slot = true;
        }
        if args.len() != parameter_types.len() || args.len() != access.len() {
            return Err("generated ownership parameter count is invalid".into());
        }
        let mut arguments = Vec::with_capacity(args.len());
        for ((argument, &parameter_type), &callee_access) in
            args.iter().zip(parameter_types).zip(access)
        {
            if !valid_type(types, argument.ty) || !valid_type(types, parameter_type) {
                return Err("generated ownership type is outside its interner".into());
            }
            let acquisition =
                if jett_typecheck::ownership::is_implicitly_copyable(types, argument.ty) {
                    GeneratedAcquisition::Copy
                } else if callee_access == CheckedCalleeAccess::View {
                    GeneratedAcquisition::Borrow {
                        source: immediate_source(argument),
                    }
                } else {
                    GeneratedAcquisition::OwnedExpression
                };
            let witness = generated_witness(
                argument,
                parameter_type,
                callee_access,
                acquisition.clone(),
                types,
            );
            arguments.push(GeneratedArgumentOwnership {
                actual_type: argument.ty,
                parameter_type,
                callee_access,
                acquisition,
                staging: GeneratedArgumentStaging::Existing { loan: None },
                staging_certificate: GeneratedArgumentStaging::Existing { loan: None },
                witness,
            });
        }
        let certificate = GeneratedCallCertificate {
            operation: operation.clone(),
            result_type,
            evaluation_order: evaluation_order.to_vec(),
            arguments: arguments
                .iter()
                .map(|argument| argument.witness.clone())
                .collect(),
        };
        Ok(Self::Generated(GeneratedCallOwnership {
            operation,
            arguments,
            certificate,
        }))
    }
}

fn valid_type(types: &TypeInterner, ty: TypeId) -> bool {
    (ty.index() as usize) < types.len()
}

pub(crate) fn immediate_source(mut value: &Expression) -> CallerViewSource {
    loop {
        match &value.kind {
            ExpressionKind::Local(local) => return CallerViewSource::Local(*local),
            ExpressionKind::Field { base, .. }
            | ExpressionKind::View(base)
            | ExpressionKind::Coarsen(base)
            | ExpressionKind::Declassify(base)
            | ExpressionKind::RefinementValidated(base) => value = base,
            ExpressionKind::InterfaceCoerce {
                value: inner,
                adapters,
            } if adapters.is_empty() => value = inner,
            _ => return CallerViewSource::Other,
        }
    }
}

fn remap_source<E>(
    source: &mut CallerViewSource,
    map: &mut impl FnMut(LocalId) -> Result<LocalId, E>,
) -> Result<(), E> {
    if let CallerViewSource::Local(id) = source {
        *id = map(*id)?;
    }
    Ok(())
}

fn remap_origin<E>(
    origin: &mut CallerOrigin,
    map: &mut impl FnMut(LocalId) -> Result<LocalId, E>,
) -> Result<(), E> {
    match origin {
        CallerOrigin::Binding(fact) => {
            fact.local = map(fact.local)?;
            if let CallerBindingMode::View { source } = &mut fact.mode {
                remap_source(source, map)?;
            }
        }
        CallerOrigin::BorrowedProjection { source }
        | CallerOrigin::OwnedFieldCopy { parent: source } => remap_source(source, map)?,
        CallerOrigin::OwnedExpression => {}
    }
    Ok(())
}

impl GeneratedArgumentOwnership {
    pub fn original_witness(&self) -> &GeneratedArgumentWitness {
        &self.witness
    }
    fn metadata_local_ids(&self, visit: &mut impl FnMut(LocalId)) {
        if let GeneratedAcquisition::Borrow {
            source: CallerViewSource::Local(id),
        } = self.acquisition
        {
            visit(id);
        }
        if let CallerViewSource::Local(id) = self.witness.origin {
            visit(id);
        }
        self.witness.producer_shape.metadata_locals(visit);
        generated_stage_local_ids(self.staging, visit);
        generated_stage_local_ids(self.staging_certificate, visit);
    }
    fn remap_metadata_locals<E>(
        &mut self,
        map: &mut impl FnMut(LocalId) -> Result<LocalId, E>,
    ) -> Result<(), E> {
        if let GeneratedAcquisition::Borrow { source } = &mut self.acquisition {
            remap_source(source, map)?;
        }
        if let GeneratedAcquisition::Borrow { source } = &mut self.witness.acquisition {
            remap_source(source, map)?;
        }
        remap_source(&mut self.witness.origin, map)?;
        self.witness.producer_shape.remap_locals(map)?;
        remap_generated_staging(&mut self.staging, map)?;
        remap_generated_staging(&mut self.staging_certificate, map)?;
        Ok(())
    }
}

/// Primitive local facts usable by HIR and MIR without sharing their bodies.
#[derive(Debug, Clone, Copy)]
pub struct OwnershipLocalInfo {
    pub ty: TypeId,
    pub mutable: bool,
    pub span: Span,
    pub view_source: Option<LocalId>,
    pub is_view_parameter: bool,
    /// Present only while an exact original/prepared viewed loop proves this binding.
    pub view_iteration: Option<crate::ViewIterationBinding>,
}

/// Validate structural authority before considering the current operand tree.
/// The caller separately proves its typed conversion/staging/target relation.
pub fn validate_operand_ownership(
    ownership: &CallOwnership,
    argument_types: &[TypeId],
    evaluation_order: &[usize],
    types: &TypeInterner,
    local: impl Fn(LocalId) -> Option<OwnershipLocalInfo>,
    initial_hir: bool,
) -> Result<(), String> {
    if ownership.parameter_count() != argument_types.len()
        || evaluation_order.len() != argument_types.len()
    {
        return Err("call ownership operand count is invalid".into());
    }
    let mut seen = vec![false; argument_types.len()];
    for &parameter in evaluation_order {
        let Some(slot) = seen.get_mut(parameter) else {
            return Err("call ownership evaluation permutation is out of bounds".into());
        };
        if *slot {
            return Err("call ownership evaluation permutation repeats an operand".into());
        }
        *slot = true;
    }
    for &ty in argument_types {
        if !valid_type(types, ty) {
            return Err("call ownership operand type is outside its interner".into());
        }
    }
    let mut valid_metadata = true;
    ownership.metadata_types(|ty| valid_metadata &= valid_type(types, ty));
    if !valid_metadata {
        return Err("call ownership metadata type is outside its interner".into());
    }
    match ownership {
        CallOwnership::Source(source) => {
            if source.target != source.certificate.target
                || source.shape != source.certificate.shape
                || source.context != source.certificate.context
                || source.bridge != source.certificate.bridge
                || source.generated_operands != source.certificate.generated_operands
            {
                return Err(
                    "call ownership invocation differs from its checked source certificate".into(),
                );
            }
            let source_count = source.arguments.len();
            let mut source_seen = vec![false; source_count];
            match &source.shape {
                CheckedInvocationShape::Function { signature_type } => {
                    if !valid_type(types, *signature_type) {
                        return Err("call ownership signature type is outside its interner".into());
                    }
                    let Type::Function {
                        params,
                        view_params,
                        ..
                    } = types.resolve(*signature_type)
                    else {
                        return Err("call ownership signature is not a function".into());
                    };
                    if params.len() != source_count || view_params.len() != source_count {
                        return Err("call ownership signature arity is invalid".into());
                    }
                    for (index, argument) in source.arguments.iter().enumerate() {
                        let access = if view_params[index] {
                            CheckedCalleeAccess::View
                        } else {
                            CheckedCalleeAccess::Owned
                        };
                        if argument.parameter_type != params[index]
                            || argument.callee_access != access
                        {
                            return Err(
                                "call ownership signature parameter type or access is invalid"
                                    .into(),
                            );
                        }
                    }
                }
                CheckedInvocationShape::Intrinsic {
                    intrinsic,
                    result_type,
                    operands,
                } => {
                    if !valid_type(types, *result_type) || operands.len() != source_count {
                        return Err("call ownership intrinsic source shape is invalid".into());
                    }
                    for (index, (argument, role)) in
                        source.arguments.iter().zip(operands).enumerate()
                    {
                        if !valid_type(types, role.ty()) || role.ty() != argument.parameter_type {
                            return Err("call ownership intrinsic operand type is invalid".into());
                        }
                        let permitted = jett_typecheck::intrinsic_operand_access(
                            *intrinsic,
                            index,
                            source_count,
                        )
                        .ok_or_else(|| {
                            "call ownership intrinsic source operand has no closed role".to_string()
                        })?;
                        let print = matches!(
                            role,
                            jett_typecheck::CheckedIntrinsicOperandRole::PrintArgument { .. }
                        );
                        if print {
                            if !matches!(intrinsic, IntrinsicId::Print | IntrinsicId::Println) {
                                return Err(
                                    "call ownership print role belongs to another intrinsic".into(),
                                );
                            }
                            let expected = if argument.syntax == CheckedCallerSyntax::WrittenView {
                                CheckedCalleeAccess::View
                            } else {
                                CheckedCalleeAccess::Owned
                            };
                            if argument.callee_access != expected {
                                return Err("call ownership print access is invalid".into());
                            }
                        } else if permitted != argument.callee_access {
                            return Err(
                                "call ownership intrinsic access differs from its closed role"
                                    .into(),
                            );
                        }
                        if !print {
                            let copyable =
                                jett_typecheck::ownership::is_implicitly_copyable(types, role.ty());
                            let valid = match role {
                                jett_typecheck::CheckedIntrinsicOperandRole::Copy { .. } => {
                                    copyable
                                }
                                jett_typecheck::CheckedIntrinsicOperandRole::Owned { .. } => {
                                    !copyable && permitted == CheckedCalleeAccess::Owned
                                }
                                jett_typecheck::CheckedIntrinsicOperandRole::View { .. } => {
                                    !copyable && permitted == CheckedCalleeAccess::View
                                }
                                jett_typecheck::CheckedIntrinsicOperandRole::PrintArgument {
                                    ..
                                } => false,
                            };
                            if !valid {
                                return Err("call ownership intrinsic role does not match its checked type and access".into());
                            }
                        }
                    }
                }
            }
            for (parameter_index, argument) in source.arguments.iter().enumerate() {
                let witness = &argument.witness;
                if argument.source_span != witness.source_span
                    || argument.source_index != witness.source_index
                    || argument.parameter_index != witness.parameter_index
                    || argument.actual_type != witness.actual_type
                    || argument.parameter_type != witness.parameter_type
                    || argument.syntax != witness.syntax
                    || argument.origin != witness.origin
                    || argument.callee_access != witness.callee_access
                    || argument.effect != witness.effect
                    || source.context != witness.context
                {
                    return Err("call ownership disagrees with its original source witness".into());
                }
                if argument.parameter_index != parameter_index
                    || argument.source_index >= source_count
                    || source_seen[argument.source_index]
                {
                    return Err("call ownership source/formal permutation is invalid".into());
                }
                source_seen[argument.source_index] = true;
                if evaluation_order.get(argument.source_index) != Some(&parameter_index) {
                    return Err("call ownership lexical evaluation order disagrees with its source permutation".into());
                }
                if !valid_type(types, argument.actual_type)
                    || !valid_type(types, witness.occurrence_type)
                    || witness
                        .handled_result
                        .is_some_and(|result| !valid_type(types, result.ty))
                    || !valid_type(types, argument.parameter_type)
                {
                    return Err("call ownership source type is outside its interner".into());
                }
                if witness.occurrence_type != argument.actual_type
                    && !(has_outer_secret(types, argument.parameter_type)
                        && secret_taint_matches(
                            types,
                            witness.occurrence_type,
                            argument.actual_type,
                        ))
                {
                    return Err("call ownership sealed occurrence has no exact expected secret qualification".into());
                }
                validate_origin(&argument.origin, &local, types)?;
                validate_effect(argument, source.context, types)?;
                if argument.effect == CheckedCallerEffect::ObserveData {
                    let proof = argument
                        .observation_proof()
                        .ok_or("observing call has no checked data acquisition proof")?;
                    if proof.actual_type != argument.actual_type
                        || !(observation_data_type(types, proof.actual_type)
                            || (proof.descriptor_checked
                                && matches!(
                                    types.resolve(proof.actual_type),
                                    Type::Function { .. }
                                )))
                    {
                        return Err(
                            "observing call acquisition contains unsupported authority".into()
                        );
                    }
                    for &(capture, ty) in &proof.captures {
                        let value = require_local(capture, &local)?;
                        if value.ty != ty
                            || value.view_source.is_some()
                            || value.is_view_parameter
                            || value.view_iteration.is_some()
                            || !observation_data_type(types, ty)
                        {
                            return Err(
                                "observing descriptor capture has no owning ordinary-data proof"
                                    .into(),
                            );
                        }
                    }
                }
                if let Some(proof) = argument.retained_snapshot {
                    if argument.effect != CheckedCallerEffect::RetainBorrow
                        || !retained_snapshot_origin(&argument.origin)
                        || proof.actual_type != argument.actual_type
                        || proof.occurrence_type != witness.occurrence_type
                        || !observation_data_type(types, proof.actual_type)
                        || !observation_data_type(types, proof.occurrence_type)
                    {
                        return Err(
                            "retained snapshot lost its checked ordinary-data endpoint proof"
                                .into(),
                        );
                    }
                }
                if let Some(root) = witness.projection_root {
                    let stored = require_local(root.local, &local)?;
                    if stored.ty != root.storage_type
                        || !flow_projection_root_matches(
                            types,
                            root.occurrence_type,
                            root.storage_type,
                        )
                        || !matches!(argument.origin,
                            CallerOrigin::BorrowedProjection { source: CallerViewSource::Local(id) }
                            | CallerOrigin::OwnedFieldCopy { parent: CallerViewSource::Local(id) }
                            if id == root.local)
                        || root.span.file != argument.source_span.file
                        || root.span.start < argument.source_span.start
                        || root.span.end > argument.source_span.end
                        || root.span.start >= root.span.end
                    {
                        return Err(
                            "call ownership flow projection lost its exact checked root".into()
                        );
                    }
                }
                if initial_hir && argument.staging != ArgumentStaging::Original {
                    return Err(
                        "initial HIR call ownership cannot carry MIR staging authority".into(),
                    );
                }
                match argument.staging {
                    ArgumentStaging::Original => {}
                    ArgumentStaging::Transferred { owner } => {
                        let owner = require_local(owner, &local)?;
                        if argument.effect != CheckedCallerEffect::TransferOwned
                            || argument.physical_access != CheckedCalleeAccess::Owned
                            || owner.ty != argument_types[parameter_index]
                            || owner.view_source.is_some()
                            || owner.is_view_parameter
                            || owner.view_iteration.is_some()
                        {
                            return Err(
                                "transferred call acquisition disagrees with checked ownership"
                                    .into(),
                            );
                        }
                    }
                    ArgumentStaging::Copied { value } => {
                        if argument.effect != CheckedCallerEffect::Copy
                            || require_local(value, &local)?.ty != argument_types[parameter_index]
                        {
                            return Err(
                                "copied call staging disagrees with checked copy or operand type"
                                    .into(),
                            );
                        }
                    }
                    ArgumentStaging::Borrowed { loan } => {
                        if argument.physical_access != CheckedCalleeAccess::View
                            || !matches!(
                                argument.effect,
                                CheckedCallerEffect::RetainBorrow
                                    | CheckedCallerEffect::ObserveData
                            )
                        {
                            return Err(
                                "borrowed call staging disagrees with the caller effect".into()
                            );
                        }
                        if require_local(loan, &local)?.ty != argument_types[parameter_index] {
                            return Err("borrowed call staging has the wrong operand type".into());
                        }
                    }
                    ArgumentStaging::Relinquished { owner, loan } => {
                        if argument.physical_access != CheckedCalleeAccess::View
                            || !(argument.effect == CheckedCallerEffect::RelinquishOwned
                                || (argument.effect == CheckedCallerEffect::TransferOwned
                                    && source.bridge != CallBridge::Direct))
                        {
                            return Err(
                                "owning call staging disagrees with the caller effect or bridge"
                                    .into(),
                            );
                        }
                        let owner = require_local(owner, &local)?;
                        if owner.ty != argument_types[parameter_index]
                            || owner.view_source.is_some()
                            || owner.is_view_parameter
                            || owner.view_iteration.is_some()
                        {
                            return Err("relinquished call owner cannot be a borrowed slot".into());
                        }
                        if require_local(loan, &local)?.ty != argument_types[parameter_index] {
                            return Err("relinquished call loan has the wrong operand type".into());
                        }
                    }
                    ArgumentStaging::RetainedSnapshot { owner, loan } => {
                        if argument.effect != CheckedCallerEffect::RetainBorrow
                            || argument.physical_access != CheckedCalleeAccess::View
                            || argument.retained_snapshot_type()
                                != Some(argument_types[parameter_index])
                            || owner == loan
                        {
                            return Err("retained snapshot disagrees with checked endpoint or caller retention".into());
                        }
                        let owner = require_local(owner, &local)?;
                        if owner.ty != argument_types[parameter_index]
                            || owner.view_source.is_some()
                            || owner.is_view_parameter
                            || owner.view_iteration.is_some()
                            || require_local(loan, &local)?.ty != owner.ty
                        {
                            return Err(
                                "retained snapshot requires an owning endpoint and exact loan type"
                                    .into(),
                            );
                        }
                    }
                    ArgumentStaging::Observed { owner } => {
                        if argument.effect != CheckedCallerEffect::ObserveData
                            || argument.physical_access != CheckedCalleeAccess::Owned
                        {
                            return Err("observation owner disagrees with the caller effect".into());
                        }
                        let owner = require_local(owner, &local)?;
                        if owner.ty != argument_types[parameter_index]
                            || owner.view_source.is_some()
                            || owner.is_view_parameter
                            || owner.view_iteration.is_some()
                        {
                            return Err("observation snapshot cannot be a borrowed slot or change the operand type".into());
                        }
                    }
                }
                if source.bridge == CallBridge::Direct
                    && argument.physical_access != argument.callee_access
                {
                    return Err("direct source call changed its physical parameter access".into());
                }
            }
            for (tail, argument) in source.generated_operands.iter().enumerate() {
                if !metadata_root_matches(source, argument.actual_type, types)
                    || argument.actual_type != argument.parameter_type
                    || argument.callee_access != CheckedCalleeAccess::Owned
                    || argument.acquisition != GeneratedAcquisition::OwnedExpression
                {
                    return Err(
                        "call ownership metadata tail has no closed compiler-owned schema".into(),
                    );
                }
                if evaluation_order.get(source_count + tail) != Some(&(source_count + tail)) {
                    return Err(
                        "generated metadata must follow all original source operands".into(),
                    );
                }
                validate_generated_operand(argument, &local, types)?;
            }
        }
        CallOwnership::Generated(generated) => {
            if generated.operation != generated.certificate.operation
                || evaluation_order != generated.certificate.evaluation_order
                || generated.arguments.len() != generated.certificate.arguments.len()
            {
                return Err(
                    "generated invocation differs from its original operation/order certificate"
                        .into(),
                );
            }
            for (index, argument) in generated.arguments.iter().enumerate() {
                if argument.witness != generated.certificate.arguments[index] {
                    return Err(
                        "generated argument witness differs from its invocation certificate".into(),
                    );
                }
                validate_generated_operand(argument, &local, types)?;
                validate_generated_stage(argument, argument_types[index], &local, initial_hir)?;
            }
            if let GeneratedOperation::FunctionAdapter {
                callee,
                source_type,
                ..
            } = generated.operation
            {
                if require_local(callee, &local)?.ty != source_type {
                    return Err("generated callable adapter capture differs from its sealed source signature".into());
                }
            }
        }
    }
    Ok(())
}

fn require_local(
    id: LocalId,
    local: &impl Fn(LocalId) -> Option<OwnershipLocalInfo>,
) -> Result<OwnershipLocalInfo, String> {
    local(id).ok_or_else(|| "call ownership origin is outside its function".into())
}

fn validate_source(
    source: CallerViewSource,
    local: &impl Fn(LocalId) -> Option<OwnershipLocalInfo>,
) -> Result<(), String> {
    if let CallerViewSource::Local(id) = source {
        require_local(id, local)?;
    }
    Ok(())
}

fn validate_origin(
    origin: &CallerOrigin,
    local: &impl Fn(LocalId) -> Option<OwnershipLocalInfo>,
    types: &TypeInterner,
) -> Result<(), String> {
    match origin {
        CallerOrigin::Binding(fact) => {
            let value = require_local(fact.local, local)?;
            if !valid_type(types, fact.ty)
                || value.ty != fact.ty
                || value.mutable != fact.mutable
                || value.span.file != fact.declaration_span.file
                || fact.declaration_span.start < value.span.start
                || fact.declaration_span.end > value.span.end
            {
                return Err(
                    "call ownership binding metadata differs from its declared local".into(),
                );
            }
            match fact.mode {
                CallerBindingMode::Owned
                    if !jett_typecheck::ownership::is_implicitly_copyable(types, fact.ty)
                        && (value.view_source.is_some()
                            || value.is_view_parameter
                            || value.view_iteration.is_some()) =>
                {
                    return Err("call ownership falsely claims an owned borrowed binding".into());
                }
                CallerBindingMode::View { source } => {
                    validate_source(source, local)?;
                    let expected = match source {
                        CallerViewSource::Local(id) => Some(id),
                        CallerViewSource::Other => None,
                    };
                    let iteration = value
                        .view_iteration
                        .is_some_and(|proof| proof.matches_binding(fact.ty, fact.declaration_span));
                    if value.view_source != expected
                        || (expected.is_none() && !value.is_view_parameter && !iteration)
                    {
                        return Err(
                            "call ownership borrowed binding lost its immediate source".into()
                        );
                    }
                }
                CallerBindingMode::Owned => {}
            }
        }
        CallerOrigin::BorrowedProjection { source }
        | CallerOrigin::OwnedFieldCopy { parent: source } => validate_source(*source, local)?,
        CallerOrigin::OwnedExpression => {}
    }
    Ok(())
}

fn validate_effect(
    argument: &ArgumentOwnership,
    context: CheckedOwnershipContext,
    types: &TypeInterner,
) -> Result<(), String> {
    if argument.syntax == CheckedCallerSyntax::WrittenView
        && argument.callee_access == CheckedCalleeAccess::Owned
    {
        return Err("call ownership written view cannot satisfy a source-owned parameter".into());
    }
    let copyable = jett_typecheck::ownership::is_implicitly_copyable(types, argument.actual_type);
    if argument.effect == CheckedCallerEffect::ObserveData {
        if context == CheckedOwnershipContext::Ordinary {
            return Err("ordinary call has no observation retention authority".into());
        }
        return Ok(()); // Snapshot/capture acquisition is proved independently by its consumer.
    }
    let expected = if copyable {
        CheckedCallerEffect::Copy
    } else if argument.syntax == CheckedCallerSyntax::WrittenView {
        CheckedCallerEffect::RetainBorrow
    } else {
        if matches!(
            argument.origin,
            CallerOrigin::Binding(CallerBindingFact {
                mode: CallerBindingMode::View { .. },
                ..
            }) | CallerOrigin::BorrowedProjection { .. }
        ) {
            return Err("bare move-only borrowed call input cannot acquire ownership".into());
        }
        match argument.callee_access {
            CheckedCalleeAccess::Owned => CheckedCallerEffect::TransferOwned,
            CheckedCalleeAccess::View => CheckedCallerEffect::RelinquishOwned,
        }
    };
    if argument.effect != expected {
        return Err("call ownership effect does not follow its checked tuple".into());
    }
    Ok(())
}

fn validate_generated_operand(
    argument: &GeneratedArgumentOwnership,
    local: &impl Fn(LocalId) -> Option<OwnershipLocalInfo>,
    types: &TypeInterner,
) -> Result<(), String> {
    if !valid_type(types, argument.actual_type) || !valid_type(types, argument.parameter_type) {
        return Err("generated call ownership type is outside its interner".into());
    }
    match argument.acquisition {
        GeneratedAcquisition::Copy
            if !jett_typecheck::ownership::is_implicitly_copyable(types, argument.actual_type) =>
        {
            return Err("generated operand cannot copy a move-only value".into());
        }
        GeneratedAcquisition::Borrow { source } => {
            if argument.callee_access != CheckedCalleeAccess::View {
                return Err("generated borrow cannot supply physical ownership".into());
            }
            validate_source(source, local)?;
        }
        GeneratedAcquisition::OwnedExpression
            if argument.callee_access != CheckedCalleeAccess::Owned =>
        {
            return Err("generated owner has no physical ownership destination".into());
        }
        _ => {}
    }
    Ok(())
}

/// Ordinary snapshot domain. Function capture authority is proved from the
/// descriptor's checked construction separately, never from its signature.
pub fn observation_data_type(types: &TypeInterner, ty: TypeId) -> bool {
    fn visit(
        types: &TypeInterner,
        ty: TypeId,
        seen: &mut std::collections::HashSet<TypeId>,
    ) -> bool {
        if !valid_type(types, ty) {
            return false;
        }
        if !seen.insert(ty) {
            return true;
        }
        if !types
            .nominal_type_arguments(ty)
            .iter()
            .all(|&argument| visit(types, argument, seen))
        {
            return false;
        }
        match types.resolve(ty) {
            Type::Resource(_)
            | Type::Capability(_)
            | Type::Actor(_)
            | Type::Interface(_)
            | Type::Function { .. }
            | Type::Error
            | Type::TypeConstruction => false,
            Type::List(inner)
            | Type::Set(inner)
            | Type::Optional(inner)
            | Type::Secret(inner)
            | Type::Refinement { base: inner, .. } => visit(types, *inner, seen),
            Type::Map(key, value) | Type::Result(key, value) => {
                visit(types, *key, seen) && visit(types, *value, seen)
            }
            Type::Struct(id) => types
                .resolve_struct(*id)
                .fields
                .iter()
                .all(|(_, field)| visit(types, *field, seen)),
            Type::Bitfield(id) => types
                .resolve_bitfield(*id)
                .fields
                .iter()
                .all(|field| visit(types, field.ty, seen)),
            Type::Enum(id) => types.resolve_enum(*id).variants.iter().all(|variant| {
                variant
                    .fields
                    .iter()
                    .all(|(_, field)| visit(types, *field, seen))
            }),
            Type::Machine(id) => types.resolve_machine(*id).states.iter().all(|state| {
                state
                    .fields
                    .iter()
                    .all(|(_, field)| visit(types, *field, seen))
            }),
            Type::MachineState { machine, state } => types
                .resolve_machine(*machine)
                .state(*state)
                .is_some_and(|state| {
                    state
                        .fields
                        .iter()
                        .all(|(_, field)| visit(types, *field, seen))
                }),
            Type::Int8
            | Type::Int16
            | Type::Int32
            | Type::Int64
            | Type::Uint8
            | Type::Uint16
            | Type::Uint32
            | Type::Uint64
            | Type::Float32
            | Type::Float64
            | Type::String
            | Type::Bool
            | Type::Bytes
            | Type::Nothing
            | Type::Never => true,
        }
    }
    visit(types, ty, &mut std::collections::HashSet::new())
}

impl crate::BodyLowerer<'_, '_> {
    pub(crate) fn generated_ownership(
        &mut self,
        operation: GeneratedOperation,
        args: &[Expression],
        parameter_types: &[TypeId],
        access: &[CheckedCalleeAccess],
        result_type: TypeId,
        evaluation_order: &[usize],
        span: Span,
    ) -> Option<CallOwnership> {
        match CallOwnership::generated(
            operation,
            args,
            parameter_types,
            access,
            result_type,
            evaluation_order,
            &self.parent.check.interner,
        ) {
            Ok(ownership) => Some(ownership),
            Err(message) => {
                self.parent.error(span, message);
                None
            }
        }
    }
    pub(crate) fn finish_call_ownership(
        &mut self,
        mut ownership: CallOwnership,
        function: Option<FunctionId>,
        intrinsic: Option<IntrinsicId>,
        args: &[Expression],
        span: Span,
    ) -> Option<CallOwnership> {
        let result = self.join_physical_ownership(&mut ownership, function, intrinsic, args);
        if let Err(message) = result {
            self.parent.error(span, message);
            return None;
        }
        Some(ownership)
    }

    pub(crate) fn finish_resource_call_ownership(
        &mut self,
        mut ownership: CallOwnership,
        hook: &crate::ResourceHookRef,
        args: &[Expression],
        span: Span,
    ) -> Option<CallOwnership> {
        let proof = (|| -> Result<(), String> {
            hook.validate(&self.parent.check.interner)?;
            if !self.parent.resource_manifest.contains_hook(hook) {
                return Err("Resource hook is from another checked program".into());
            }
            let CallOwnership::Source(source) = &mut ownership else {
                return Err("Resource invocation requires an original Source certificate".into());
            };
            if source.target != CallTarget::ResourceHook(hook.clone())
                || source.shape
                    != (CheckedInvocationShape::Function {
                        signature_type: hook.function_type(),
                    })
                || source.arguments.len() != args.len()
                || !source.generated_operands.is_empty()
            {
                return Err(
                    "Resource invocation differs from its exact checked hook target".into(),
                );
            }
            source.bridge = CallBridge::ResourceHook { hook: hook.clone() };
            source.certificate.bridge = source.bridge.clone();
            Ok(())
        })();
        if let Err(message) = proof {
            self.parent.error(span, message);
            return None;
        }
        Some(ownership)
    }

    fn join_physical_ownership(
        &self,
        ownership: &mut CallOwnership,
        function: Option<FunctionId>,
        intrinsic: Option<IntrinsicId>,
        args: &[Expression],
    ) -> Result<(), String> {
        let CallOwnership::Source(source) = ownership else {
            return Err("source handoff requires a source packet".into());
        };
        let source_count = source.arguments.len();
        if source_count > args.len() {
            return Err("source handoff removed an original operand".into());
        }
        if let Some(function) = function {
            let (params, access) = self.ownership_function_parameters(function)?;
            if params.len() != args.len() {
                return Err("source handoff physical function arity changed".into());
            }
            let direct = matches!(source.target, CallTarget::Function(target) | CallTarget::Interface { function: target, .. } if target == function);
            if !direct {
                let CallTarget::Intrinsic(intrinsic) = source.target else {
                    return Err(
                        "source call cannot redirect to an unrelated implementation function"
                            .into(),
                    );
                };
                if !json_intrinsic(intrinsic) || !self.json_ownership_helper(function, intrinsic) {
                    return Err("source JSON handoff has no closed trusted helper identity".into());
                }
                source.bridge = CallBridge::JsonSource {
                    intrinsic,
                    function,
                };
            }
            for (index, argument) in source.arguments.iter_mut().enumerate() {
                argument.physical_access = access[index];
            }
        } else if let Some(intrinsic) = intrinsic {
            match source.target.clone() {
                CallTarget::Intrinsic(original) if original == intrinsic => {}
                CallTarget::Function(function) => {
                    let key = self
                        .function_ids
                        .iter()
                        .find_map(|(key, &id)| (id == function).then_some(key))
                        .ok_or("intrinsic facade has no exact source declaration key")?;
                    let identity = self.ownership_declaration(key)?;
                    let CheckedInvocationShape::Function { signature_type } = source.shape else {
                        return Err("intrinsic facade has no checked signature".into());
                    };
                    if !trusted_intrinsic_declaration(&identity, intrinsic) {
                        return Err("intrinsic handoff is not a closed compiler facade".into());
                    }
                    source.target = CallTarget::Declaration {
                        identity,
                        signature_type,
                    };
                    source.bridge = CallBridge::TrustedIntrinsic { intrinsic };
                }
                CallTarget::Declaration { ref identity, .. }
                    if trusted_intrinsic_declaration(identity, intrinsic) =>
                {
                    source.bridge = CallBridge::TrustedIntrinsic { intrinsic };
                }
                _ => return Err("source intrinsic handoff changed its checked target".into()),
            }
            for (index, argument) in source.arguments.iter_mut().enumerate() {
                argument.physical_access =
                    if matches!(intrinsic, IntrinsicId::Print | IntrinsicId::Println) {
                        argument.callee_access
                    } else {
                        jett_typecheck::intrinsic_operand_access(intrinsic, index, source_count)
                            .ok_or("source intrinsic physical operand has no closed access")?
                    };
            }
        }
        for argument in &args[source_count..] {
            validate_compiler_metadata_operand(argument, source, &self.parent.check.interner)?;
            source.generated_operands.push(GeneratedArgumentOwnership {
                actual_type: argument.ty,
                parameter_type: argument.ty,
                callee_access: CheckedCalleeAccess::Owned,
                acquisition: GeneratedAcquisition::OwnedExpression,
                staging: GeneratedArgumentStaging::Existing { loan: None },
                staging_certificate: GeneratedArgumentStaging::Existing { loan: None },
                witness: generated_witness(
                    argument,
                    argument.ty,
                    CheckedCalleeAccess::Owned,
                    GeneratedAcquisition::OwnedExpression,
                    &self.parent.check.interner,
                ),
            });
        }
        source.certificate.generated_operands = source.generated_operands.clone();
        source.certificate.target = source.target.clone();
        source.certificate.bridge = source.bridge.clone();
        Ok(())
    }

    pub(crate) fn ownership_function_parameters(
        &self,
        function: FunctionId,
    ) -> Result<(Vec<TypeId>, Vec<CheckedCalleeAccess>), String> {
        let key = self
            .function_ids
            .iter()
            .find_map(|(key, &id)| (id == function).then_some(key))
            .ok_or("physical source helper has no exact HIR key")?;
        let (params, views) = match key {
            crate::FunctionKey::Definition {
                definition,
                concrete_args,
                specialization,
            } if !concrete_args.is_empty() => {
                let instance = self
                    .parent
                    .check
                    .generic_function_instantiations
                    .iter()
                    .find(|instance| {
                        instance.definition == *definition
                            && instance.concrete_args == *concrete_args
                            && instance.specialization == *specialization
                    })
                    .ok_or("physical source helper has no exact checked generic body")?;
                let source = self
                    .parent
                    .module
                    .items
                    .iter()
                    .find_map(|item| match item {
                        jett_parser::ast::Item::Function(source)
                            if self.parent.definition_at(
                                source.name.span,
                                jett_resolve::DefKind::Function,
                            ) == Some(*definition) =>
                        {
                            Some(source)
                        }
                        _ => None,
                    })
                    .ok_or("physical helper has no source parameter modes")?;
                (
                    instance.parameter_types.clone(),
                    source.params.iter().map(|param| param.view).collect(),
                )
            }
            crate::FunctionKey::Definition { definition, .. } => {
                let ty = *self
                    .parent
                    .check
                    .definition_types
                    .get(definition)
                    .ok_or("physical source helper has no checked function type")?;
                let Type::Function {
                    params,
                    view_params,
                    ..
                } = self.parent.check.interner.resolve(ty)
                else {
                    return Err("physical source helper type is not a function".into());
                };
                (params.clone(), view_params.clone())
            }
            crate::FunctionKey::Method { source_span } => {
                let method = self
                    .parent
                    .check
                    .method_definitions
                    .iter()
                    .find(|method| method.source_span == *source_span)
                    .ok_or("physical method has no checked declaration")?;
                let source = self
                    .parent
                    .method_at(*source_span)
                    .ok_or("physical method has no source parameter modes")?;
                (
                    method.parameter_types.clone(),
                    source.params.iter().map(|param| param.view).collect(),
                )
            }
            crate::FunctionKey::Interface { owner, method } => {
                let Type::Interface(id) = self.parent.check.interner.resolve(*owner) else {
                    return Err("physical dispatcher is not an interface".into());
                };
                let signature = self
                    .parent
                    .check
                    .interner
                    .resolve_interface(*id)
                    .methods
                    .get(*method)
                    .ok_or("physical interface slot is out of bounds")?;
                (
                    signature.params.iter().map(|(_, ty, _)| *ty).collect(),
                    signature.params.iter().map(|(_, _, view)| *view).collect(),
                )
            }
        };
        Ok((
            params,
            views
                .into_iter()
                .map(|view| {
                    if view {
                        CheckedCalleeAccess::View
                    } else {
                        CheckedCalleeAccess::Owned
                    }
                })
                .collect(),
        ))
    }

    fn json_ownership_helper(&self, function: FunctionId, intrinsic: IntrinsicId) -> bool {
        let Some(key) = self
            .function_ids
            .iter()
            .find_map(|(key, &id)| (id == function).then_some(key))
        else {
            return false;
        };
        self.ownership_declaration(key)
            .is_ok_and(|identity| json_helper_declaration(&identity, intrinsic))
    }

    pub(crate) fn source_ownership(
        &mut self,
        span: Span,
        args: &[Expression],
        evaluation_order: &[usize],
    ) -> Option<CallOwnership> {
        let result = self.build_source_ownership(span, args, evaluation_order);
        match result {
            Ok(value) => Some(value),
            Err(message) => {
                self.parent.error(span, message);
                None
            }
        }
    }

    fn build_source_ownership(
        &self,
        span: Span,
        args: &[Expression],
        evaluation_order: &[usize],
    ) -> Result<CallOwnership, String> {
        let checked = self
            .call_ownership
            .get(&span)
            .ok_or("source invocation has no checked caller ownership")?;
        if checked.context != self.ownership_context {
            return Err("source invocation ownership context is not its lexical context".into());
        }
        if checked.arguments.len() != args.len() || evaluation_order.len() != args.len() {
            return Err("checked caller ownership source operand count changed".into());
        }
        let target = self.checked_ownership_target(&checked.target, &checked.shape)?;
        let mut arguments = Vec::with_capacity(args.len());
        for parameter_index in 0..args.len() {
            let fact = checked
                .arguments
                .iter()
                .find(|fact| fact.parameter_index == parameter_index)
                .ok_or("checked caller ownership has no exact formal occurrence")?;
            if checked.arguments.get(fact.source_index) != Some(fact)
                || evaluation_order.get(fact.source_index) != Some(&parameter_index)
            {
                return Err("checked caller ownership source/formal join changed".into());
            }
            if self.source_expression_types.get(&fact.source_span) != Some(&fact.actual_type) {
                return Err(
                    "call ownership raw source occurrence differs from its exact checked body"
                        .into(),
                );
            }
            // A pipeline input and its field/callable target can share a span.
            // The dedicated ledger retains input authority without overwriting
            // either the target expression or the prior raw source output.
            let occurrence_type =
                if fact.source_index == 0 && self.pipeline_step_call_types.contains_key(&span) {
                    *self
                        .pipeline_step_input_types
                        .get(&span)
                        .ok_or("pipeline ownership input has no exact checked physical type")?
                } else {
                    *self
                        .expression_types
                        .get(&fact.source_span)
                        .ok_or("call ownership occurrence has no exact checked physical type")?
                };
            if occurrence_type != fact.actual_type
                && !(has_outer_secret(&self.parent.check.interner, fact.parameter_type)
                    && secret_taint_matches(
                        &self.parent.check.interner,
                        occurrence_type,
                        fact.actual_type,
                    ))
            {
                return Err(
                    "call ownership occurrence has no checked outer-secret qualification".into(),
                );
            }
            let origin = self.checked_caller_origin(&fact.origin)?;
            let observation = if fact.effect == CheckedCallerEffect::ObserveData {
                Some(self.observation_proof(&args[parameter_index], fact.actual_type)?)
            } else {
                None
            };
            let witness = SourceArgumentWitness {
                source_span: fact.source_span,
                source_index: fact.source_index,
                parameter_index,
                actual_type: fact.actual_type,
                occurrence_type,
                parameter_type: fact.parameter_type,
                syntax: fact.syntax,
                origin: origin.clone(),
                callee_access: fact.callee_access,
                effect: fact.effect,
                context: checked.context,
                observation,
                handled_result: None,
                projection_root: self.checked_flow_projection_root(fact.source_span, &origin)?,
            };
            let mut argument = ArgumentOwnership {
                source_span: fact.source_span,
                source_index: fact.source_index,
                parameter_index,
                actual_type: fact.actual_type,
                parameter_type: fact.parameter_type,
                syntax: fact.syntax,
                origin: origin.clone(),
                callee_access: fact.callee_access,
                physical_access: fact.callee_access,
                effect: fact.effect,
                staging: ArgumentStaging::Original,
                witness,
                retained_snapshot: (fact.effect == CheckedCallerEffect::RetainBorrow
                    && retained_snapshot_origin(&origin)
                    && observation_data_type(&self.parent.check.interner, fact.actual_type)
                    && observation_data_type(&self.parent.check.interner, occurrence_type))
                .then_some(RetainedSnapshotProof {
                    actual_type: fact.actual_type,
                    occurrence_type,
                    physical_span: args[parameter_index].span,
                }),
            };
            if argument.origin == CallerOrigin::OwnedExpression {
                let original = source_operand(
                    &args[parameter_index],
                    &argument,
                    &CallBridge::Direct,
                    &self.parent.check.interner,
                )?;
                let backing = source_view_backing(original);
                if matches!(backing.kind, ExpressionKind::Handle { .. }) {
                    argument.witness.handled_result = Some(SourceHandledResult {
                        ty: backing.ty,
                        span: backing.span,
                    });
                }
            }
            arguments.push(argument);
        }
        let shape = checked.shape.clone();
        let context = checked.context;
        let result_type = self
            .pipeline_step_call_types
            .get(&span)
            .or_else(|| self.expression_types.get(&span))
            .copied()
            .ok_or("source invocation has no exact checked result type")?;
        let ownership = CallOwnership::Source(SourceCallOwnership {
            target: target.clone(),
            shape: shape.clone(),
            context,
            arguments,
            bridge: CallBridge::Direct,
            generated_operands: Vec::new(),
            certificate: SourceCallCertificate {
                target,
                shape,
                context,
                bridge: CallBridge::Direct,
                result_type,
                generated_operands: Vec::new(),
            },
        });
        validate_operand_ownership(
            &ownership,
            &args.iter().map(|arg| arg.ty).collect::<Vec<_>>(),
            evaluation_order,
            &self.parent.check.interner,
            |id| self.ownership_local_info(id),
            true,
        )?;
        Ok(ownership)
    }

    fn checked_flow_projection_root(
        &self,
        source_span: Span,
        origin: &CallerOrigin,
    ) -> Result<Option<SourceProjectionRoot>, String> {
        let local = match origin {
            CallerOrigin::BorrowedProjection {
                source: CallerViewSource::Local(id),
            }
            | CallerOrigin::OwnedFieldCopy {
                parent: CallerViewSource::Local(id),
            } => *id,
            _ => return Ok(None),
        };
        let stored = self
            .locals
            .get(local.index() as usize)
            .filter(|stored| stored.id == local)
            .ok_or("call ownership projection root has no exact stored local")?;
        if !valid_type(&self.parent.check.interner, stored.ty) {
            return Err("call ownership projection root has invalid stored metadata".into());
        }
        if !matches!(
            self.parent.check.interner.resolve(stored.ty),
            Type::Machine(_)
        ) {
            return Ok(None);
        }
        let root = self
            .source_projection_roots
            .get(&source_span)
            .ok_or("call ownership projection has no original source AST root")?;
        let occurrence_type = *self
            .source_expression_types
            .get(&root.span)
            .ok_or("call ownership projection root has no exact checked body type")?;
        if !flow_projection_root_matches(&self.parent.check.interner, occurrence_type, stored.ty) {
            return Ok(None);
        }
        let definition = self
            .parent
            .resolve
            .resolutions
            .get(&root.span)
            .ok_or("call ownership projection root has no exact source definition")?;
        if self.local_ids.get(definition) != Some(&local)
            || self.expression_types.get(&root.span) != Some(&occurrence_type)
            || root.span.file != source_span.file
            || root.span.start < source_span.start
            || root.span.end > source_span.end
            || root.span.start >= root.span.end
        {
            return Err(
                "call ownership flow projection differs from its original source root".into(),
            );
        }
        Ok(Some(SourceProjectionRoot {
            local,
            span: root.span,
            occurrence_type,
            storage_type: stored.ty,
        }))
    }

    fn checked_ownership_target(
        &self,
        target: &jett_typecheck::CheckedInvocationTarget,
        shape: &CheckedInvocationShape,
    ) -> Result<CallTarget, String> {
        use jett_typecheck::CheckedInvocationTarget as Target;
        if let Target::Resolved(definition) = target
            && let Some(hook) = self
                .parent
                .resource_manifest
                .hook_for_definition(*definition)
        {
            return Ok(CallTarget::ResourceHook(hook));
        }
        // A resolver kernel without the original envelope cannot become an ordinary function.
        if let Target::Resolved(definition) = target
            && self
                .parent
                .resolve
                .resource_kernels
                .contains_definition(*definition)
        {
            return Err(
                "Resource hook requires lower_checked_resource_program and its original manifest"
                    .into(),
            );
        }
        let key = match target {
            Target::Resolved(definition) => crate::FunctionKey::Definition {
                definition: *definition,
                concrete_args: Vec::new(),
                specialization: jett_typecheck::CheckedGenericSpecialization::default(),
            },
            Target::Generic(generic) => crate::FunctionKey::Definition {
                definition: generic.definition,
                concrete_args: generic.concrete_args.clone(),
                specialization: generic.specialization.clone(),
            },
            Target::Method(method) => crate::FunctionKey::Method {
                source_span: method.source_span,
            },
            Target::Interface(interface) => {
                let key = crate::FunctionKey::Interface {
                    owner: interface.interface_type,
                    method: interface.method_index,
                };
                let function = *self
                    .function_ids
                    .get(&key)
                    .ok_or("source interface ownership target has no HIR dispatcher")?;
                return Ok(CallTarget::Interface {
                    function,
                    interface_type: interface.interface_type,
                    method_index: interface.method_index,
                });
            }
            Target::Indirect(signature_type) => {
                return Ok(CallTarget::Indirect {
                    signature_type: *signature_type,
                });
            }
            Target::Intrinsic(intrinsic) => return Ok(CallTarget::Intrinsic(*intrinsic)),
        };
        if let Some(&function) = self.function_ids.get(&key) {
            return Ok(CallTarget::Function(function));
        }
        let CheckedInvocationShape::Function { signature_type } = shape else {
            return Err("source declaration ownership target has no function signature".into());
        };
        let identity = self.ownership_declaration(&key)?;
        if identity.declaration.origin != jett_common::SourceOrigin::Stdlib {
            return Err("source ownership declaration has no executable HIR target".into());
        }
        Ok(CallTarget::Declaration {
            identity,
            signature_type: *signature_type,
        })
    }

    fn ownership_declaration(&self, key: &crate::FunctionKey) -> Result<FunctionIdentity, String> {
        let crate::FunctionKey::Definition {
            definition,
            concrete_args,
            specialization,
        } = key
        else {
            return Err("ownership declaration is not an ordinary concrete source function".into());
        };
        let info = self
            .parent
            .resolve
            .scope_table
            .definitions
            .get(definition.index() as usize)
            .filter(|info| info.id == *definition)
            .ok_or("ownership declaration definition is outside its resolution session")?;
        if info.kind != jett_resolve::DefKind::Function {
            return Err("ownership declaration is not a function definition".into());
        }
        let origin = self
            .parent
            .origins
            .get(&info.span.file)
            .cloned()
            .ok_or("ownership declaration has no compiler source origin")?;
        Ok(FunctionIdentity {
            declaration: crate::DeclarationId {
                origin,
                namespace: info.namespace.clone().unwrap_or_default(),
                name: info
                    .name
                    .rsplit('.')
                    .next()
                    .unwrap_or(&info.name)
                    .to_string(),
                kind: crate::DeclarationKind::Function,
            },
            scoped_type_bindings: Vec::new(),
            type_arguments: concrete_args.clone(),
            specialization: specialization.clone(),
        })
    }

    fn checked_view_source(
        &self,
        source: jett_typecheck::CheckedViewSource,
    ) -> Result<CallerViewSource, String> {
        match source {
            jett_typecheck::CheckedViewSource::Other => Ok(CallerViewSource::Other),
            jett_typecheck::CheckedViewSource::Binding(definition) => self
                .local_ids
                .get(&definition)
                .copied()
                .map(CallerViewSource::Local)
                .ok_or_else(|| "caller origin has no local in its exact checked body".into()),
        }
    }

    fn ownership_local_info(&self, id: LocalId) -> Option<OwnershipLocalInfo> {
        let local = self
            .locals
            .get(id.index() as usize)
            .filter(|local| local.id == id)?;
        let is_view_parameter = self.view_parameter_locals.contains(&id);
        Some(OwnershipLocalInfo {
            ty: local.ty,
            mutable: local.mutable,
            span: local.span,
            view_source: local.view_source,
            is_view_parameter,
            view_iteration: self.view_iteration_bindings.get(&id).copied(),
        })
    }

    pub(crate) fn check_pipeline_actuals(
        &mut self,
        span: Span,
        piped: &Expression,
        written_view: bool,
        extras: &[jett_parser::ast::CallArg],
    ) -> Option<()> {
        let valid = self.call_ownership.get(&span).is_some_and(|checked| {
            checked.arguments.len() == extras.len() + 1
                && checked.arguments.first().is_some_and(|first| {
                    self.source_expression_types.get(&first.source_span) == Some(&first.actual_type)
                        && self.pipeline_step_input_types.get(&span) == Some(&piped.ty)
                        && first.syntax
                            == if written_view || original_view(piped) {
                                CheckedCallerSyntax::WrittenView
                            } else {
                                CheckedCallerSyntax::Bare
                            }
                })
                && checked.arguments[1..]
                    .iter()
                    .zip(extras)
                    .all(|(fact, extra)| {
                        fact.source_span == extra.value.span()
                            && fact.syntax == source_syntax(&extra.value)
                            && self.source_expression_types.get(&extra.value.span())
                                == Some(&fact.actual_type)
                    })
        });
        if !valid {
            self.parent.error(
                span,
                "pipeline ownership differs from its original virtual source occurrences",
            );
            return None;
        }
        Some(())
    }

    fn checked_caller_origin(
        &self,
        origin: &jett_typecheck::CheckedCallerOrigin,
    ) -> Result<CallerOrigin, String> {
        use jett_typecheck::CheckedCallerOrigin as Origin;
        match origin {
            Origin::Binding(fact) => {
                if self.binding_facts.get(&fact.declaration_span) != Some(fact) {
                    return Err("caller binding fact is not owned by its exact checked body".into());
                }
                let local = *self
                    .local_ids
                    .get(&fact.definition)
                    .ok_or("caller binding has no exact HIR local")?;
                let mode = match fact.mode {
                    jett_typecheck::CheckedBindingMode::Owned => CallerBindingMode::Owned,
                    jett_typecheck::CheckedBindingMode::View { source } => {
                        CallerBindingMode::View {
                            source: self.checked_view_source(source)?,
                        }
                    }
                };
                Ok(CallerOrigin::Binding(CallerBindingFact {
                    local,
                    declaration_span: fact.declaration_span,
                    ty: fact.ty,
                    mode,
                    mutable: fact.mutable,
                }))
            }
            Origin::BorrowedProjection { source } => Ok(CallerOrigin::BorrowedProjection {
                source: self.checked_view_source(*source)?,
            }),
            Origin::OwnedFieldCopy { parent } => Ok(CallerOrigin::OwnedFieldCopy {
                parent: self.checked_view_source(*parent)?,
            }),
            Origin::OwnedExpression => Ok(CallerOrigin::OwnedExpression),
        }
    }

    pub(crate) fn check_source_actuals(
        &mut self,
        span: Span,
        actuals: &[jett_parser::ast::CallArg],
    ) -> Option<()> {
        let valid = self.call_ownership.get(&span).is_some_and(|checked| {
            checked.arguments.len() == actuals.len()
                && checked.arguments.iter().zip(actuals).all(|(fact, actual)| {
                    fact.source_span == actual.value.span()
                        && fact.syntax == source_syntax(&actual.value)
                        && self.source_expression_types.get(&actual.value.span())
                            == Some(&fact.actual_type)
                })
        });
        if !valid {
            self.parent.error(
                span,
                "caller ownership differs from its original source argument syntax or type",
            );
            return None;
        }
        Some(())
    }

    fn observation_proof(
        &self,
        expression: &Expression,
        actual_type: TypeId,
    ) -> Result<ObservationProof, String> {
        let mut value = expression;
        while let ExpressionKind::View(inner) = &value.kind {
            value = inner;
        }
        let types = &self.parent.check.interner;
        if observation_data_type(types, actual_type) {
            return Ok(ObservationProof {
                actual_type,
                captures: Vec::new(),
                descriptor_checked: false,
            });
        }
        if !valid_type(types, actual_type)
            || !matches!(types.resolve(actual_type), Type::Function { .. })
        {
            return Err(
                "observing caller input has no supported ordinary-data snapshot proof".into(),
            );
        }
        let captures = match &value.kind {
            ExpressionKind::FunctionRef(_) => Vec::new(),
            ExpressionKind::InlineFunction {
                local_floor, body, ..
            } => self
                .locals
                .iter()
                .filter(|local| {
                    local.id.index() < *local_floor
                        && crate::inline_functions::body_uses_local(body, local.id)
                })
                .map(|local| (local.id, local.ty))
                .collect(),
            _ => {
                return Err(
                    "observing function descriptor has no checked capture construction proof"
                        .into(),
                );
            }
        };
        for &(id, ty) in &captures {
            let local = self
                .ownership_local_info(id)
                .ok_or("observing descriptor capture is outside its function")?;
            if local.view_source.is_some()
                || local.is_view_parameter
                || local.view_iteration.is_some()
                || !observation_data_type(types, ty)
            {
                return Err(
                    "observing descriptor capture cannot acquire authority or a borrowed owner"
                        .into(),
                );
            }
        }
        Ok(ObservationProof {
            actual_type,
            captures,
            descriptor_checked: true,
        })
    }
}

fn source_syntax(mut expression: &jett_parser::ast::Expr) -> CheckedCallerSyntax {
    loop {
        match expression {
            jett_parser::ast::Expr::Paren(inner, _)
            | jett_parser::ast::Expr::Coarsen(inner, _)
            | jett_parser::ast::Expr::Declassify(inner, _) => expression = inner,
            jett_parser::ast::Expr::View(_, _) => return CheckedCallerSyntax::WrittenView,
            _ => return CheckedCallerSyntax::Bare,
        }
    }
}

/// Syntactic place provenance only, recorded while lowering the original AST.
pub(super) fn source_projection_ast_root(
    mut expression: &jett_parser::ast::Expr,
) -> Option<&jett_parser::ast::Ident> {
    use jett_parser::ast::Expr;
    let mut projected = false;
    loop {
        expression = match expression {
            Expr::View(inner, _)
            | Expr::Paren(inner, _)
            | Expr::Coarsen(inner, _)
            | Expr::Declassify(inner, _) => inner,
            Expr::FieldAccess(base, _, _) => {
                projected = true;
                base
            }
            Expr::Ident(root) if projected => return Some(root),
            _ => return None,
        };
    }
}

fn original_view(mut expression: &Expression) -> bool {
    loop {
        match &expression.kind {
            ExpressionKind::Coarsen(inner) | ExpressionKind::Declassify(inner) => {
                expression = inner
            }
            ExpressionKind::View(_) => return true,
            _ => return false,
        }
    }
}

fn json_intrinsic(intrinsic: IntrinsicId) -> bool {
    matches!(
        intrinsic,
        IntrinsicId::JsonParse
            | IntrinsicId::JsonParseExact
            | IntrinsicId::JsonSerialize
            | IntrinsicId::JsonSerializePublic
    )
}

fn trusted_intrinsic_declaration(identity: &FunctionIdentity, intrinsic: IntrinsicId) -> bool {
    if identity.declaration.origin != jett_common::SourceOrigin::Stdlib
        || identity.declaration.kind != crate::DeclarationKind::Function
    {
        return false;
    }
    match (
        identity.declaration.namespace.as_str(),
        identity.declaration.name.as_str(),
        intrinsic,
    ) {
        ("math", "average", IntrinsicId::MathAverage)
        | ("math", "median", IntrinsicId::MathMedian) => true,
        ("json", "serialize", IntrinsicId::JsonSerialize)
        | ("json", "serialize_public", IntrinsicId::JsonSerializePublic)
        | ("json", "parse", IntrinsicId::JsonParse)
        | ("json", "parse_exact", IntrinsicId::JsonParseExact) => true,
        _ => false,
    }
}

fn json_helper_declaration(identity: &FunctionIdentity, intrinsic: IntrinsicId) -> bool {
    if identity.declaration.origin != jett_common::SourceOrigin::Stdlib
        || identity.declaration.namespace != "json"
        || identity.declaration.kind != crate::DeclarationKind::Function
    {
        return false;
    }
    match intrinsic {
        IntrinsicId::JsonSerialize | IntrinsicId::JsonSerializePublic => matches!(
            identity.declaration.name.as_str(),
            "serialize"
                | "serialize_public"
                | "serialize_raw"
                | "json_serialize_native_enum"
                | "json_serialize_native_bitfield"
                | "json_serialize_native_bytes"
                | "json_serialize_native_nothing"
        ),
        IntrinsicId::JsonParse => matches!(
            identity.declaration.name.as_str(),
            "parse"
                | "parse_raw"
                | "json_parse_native_enum"
                | "json_parse_native_bitfield"
                | "json_parse_native_secret_tree"
                | "json_parse_native_string"
                | "json_parse_native_bool"
                | "json_parse_native_int64"
                | "json_parse_native_uint8"
                | "json_parse_native_uint64"
                | "json_parse_native_float64"
                | "json_parse_native_bytes"
                | "json_parse_native_nothing"
        ),
        IntrinsicId::JsonParseExact => matches!(
            identity.declaration.name.as_str(),
            "parse_exact"
                | "parse_raw"
                | "json_parse_exact_native_enum"
                | "json_parse_exact_native_bitfield"
                | "json_parse_native_secret_tree"
                | "json_parse_native_string"
                | "json_parse_native_bool"
                | "json_parse_native_int64"
                | "json_parse_native_uint8"
                | "json_parse_native_uint64"
                | "json_parse_native_float64"
                | "json_parse_native_bytes"
                | "json_parse_native_nothing"
        ),
        _ => false,
    }
}

/// Full typed HIR gate, run on every retained or unused invocation before MIR.
pub fn validate_program_call_ownership(
    program: &crate::Program,
    types: &TypeInterner,
) -> Result<(), Vec<crate::ValidationError>> {
    crate::validate_backend_types(program, types)
}

pub fn validate_hir_invocation(
    functions: &[crate::Function],
    locals: &[OwnershipLocalInfo],
    expression: &Expression,
    types: &TypeInterner,
    context: CheckedOwnershipContext,
) -> Result<(), String> {
    let (ownership, args, order) = match &expression.kind {
        ExpressionKind::Call {
            ownership,
            args,
            evaluation_order,
            ..
        }
        | ExpressionKind::Intrinsic {
            ownership,
            args,
            evaluation_order,
            ..
        }
        | ExpressionKind::IndirectCall {
            ownership,
            args,
            evaluation_order,
            ..
        }
        | ExpressionKind::ResourceInvoke {
            ownership,
            args,
            evaluation_order,
            ..
        } => (ownership, args, evaluation_order),
        _ => return Ok(()),
    };
    validate_operand_ownership(
        ownership,
        &args.iter().map(|arg| arg.ty).collect::<Vec<_>>(),
        order,
        types,
        |id| locals.get(id.index() as usize).copied(),
        true,
    )?;
    validate_invocation_target(functions, expression, types, context)?;
    match ownership {
        CallOwnership::Source(source) => {
            for (index, argument) in source.arguments.iter().enumerate() {
                if argument
                    .retained_snapshot_span()
                    .is_some_and(|span| span != args[index].span)
                {
                    return Err(
                        "call ownership retained snapshot physical occurrence changed".into(),
                    );
                }
                validate_source_operand(&args[index], argument, &source.bridge, locals, types)?;
            }
            for (tail, argument) in source.generated_operands.iter().enumerate() {
                validate_generated_operand_tree(
                    &args[source.arguments.len() + tail],
                    argument,
                    locals,
                )?;
                validate_compiler_metadata_operand(
                    &args[source.arguments.len() + tail],
                    source,
                    types,
                )?;
            }
        }
        CallOwnership::Generated(generated) => {
            for (value, argument) in args.iter().zip(&generated.arguments) {
                validate_generated_operand_tree(value, argument, locals)?;
            }
        }
    }
    Ok(())
}

/// Validate invocation identity without granting authority to a rewritten operand.
/// MIR separately proves each generated result, owning Let and BeginCallView.
pub fn validate_invocation_target(
    functions: &[crate::Function],
    expression: &Expression,
    types: &TypeInterner,
    context: CheckedOwnershipContext,
) -> Result<(), String> {
    let (ownership, args, _order) = match &expression.kind {
        ExpressionKind::Call {
            ownership,
            args,
            evaluation_order,
            ..
        }
        | ExpressionKind::Intrinsic {
            ownership,
            args,
            evaluation_order,
            ..
        }
        | ExpressionKind::IndirectCall {
            ownership,
            args,
            evaluation_order,
            ..
        }
        | ExpressionKind::ResourceInvoke {
            ownership,
            args,
            evaluation_order,
            ..
        } => (ownership, args, evaluation_order),
        _ => return Ok(()),
    };
    if ownership.parameter_count() != args.len() {
        return Err("call ownership target operand count is invalid".into());
    }
    let mut valid_metadata = true;
    ownership.metadata_types(|ty| valid_metadata &= valid_type(types, ty));
    if !valid_metadata {
        return Err("call ownership target metadata type is outside its interner".into());
    }
    if let CallOwnership::Source(source) = ownership {
        if source.target != source.certificate.target
            || source.shape != source.certificate.shape
            || source.context != source.certificate.context
            || source.bridge != source.certificate.bridge
            || source.generated_operands != source.certificate.generated_operands
        {
            return Err(
                "call ownership invocation differs from its checked source certificate".into(),
            );
        }
    }
    if args.iter().any(|argument| !valid_type(types, argument.ty)) {
        return Err("call ownership physical operand type is outside its interner".into());
    }
    let target = match &expression.kind {
        ExpressionKind::Call { function, .. } => Some(function_lookup(functions, *function)?),
        _ => None,
    };
    match ownership {
        CallOwnership::Source(source) => {
            if expression.ty != source.certificate.result_type {
                return Err(
                    "call ownership result differs from its checked source certificate".into(),
                );
            }
            if source.context != context {
                return Err("call ownership source context is not its lexical owner".into());
            }
            match (&source.target, &source.bridge, &expression.kind) {
                (
                    CallTarget::ResourceHook(target_hook),
                    CallBridge::ResourceHook { hook: bridge_hook },
                    ExpressionKind::ResourceInvoke { hook, .. },
                ) if target_hook == hook && bridge_hook == hook => {
                    hook.validate(types)?;
                    if source.shape
                        != (CheckedInvocationShape::Function {
                            signature_type: hook.function_type(),
                        })
                        || !source.generated_operands.is_empty()
                    {
                        return Err("Resource invocation changed its original hook signature or added Generated operands".into());
                    }
                    let Type::Function {
                        params,
                        view_params,
                        ..
                    } = types.resolve(hook.function_type())
                    else {
                        return Err("Resource hook has no exact function signature".into());
                    };
                    if params.len() != args.len() || view_params.len() != args.len() {
                        return Err("Resource invocation hook arity changed".into());
                    }
                    for (index, argument) in source.arguments.iter().enumerate() {
                        let expected_access = if view_params[index] {
                            CheckedCalleeAccess::View
                        } else {
                            CheckedCalleeAccess::Owned
                        };
                        if argument.physical_access != expected_access
                            || argument.callee_access != expected_access
                            || argument.parameter_type != params[index]
                            || !source_parameter_matches(
                                argument,
                                args[index].ty,
                                params[index],
                                types,
                            )
                        {
                            return Err("Resource invocation differs from its exact source formal type or mode".into());
                        }
                    }
                }

                (
                    CallTarget::Function(source_target),
                    CallBridge::Direct,
                    ExpressionKind::Call { function, .. },
                ) if source_target == function => {
                    validate_function_shape(
                        &source.shape,
                        target.ok_or("source call has no physical function")?,
                        types,
                    )?;
                }
                (
                    CallTarget::Interface {
                        function: source_target,
                        interface_type,
                        method_index,
                    },
                    CallBridge::Direct,
                    ExpressionKind::Call { function, .. },
                ) if source_target == function => {
                    let target =
                        target.ok_or("source interface call has no physical dispatcher")?;
                    validate_function_shape(&source.shape, target, types)?;
                    let Type::Interface(id) = types.resolve(*interface_type) else {
                        return Err("source interface ownership owner is not an interface".into());
                    };
                    let method = types
                        .resolve_interface(*id)
                        .methods
                        .get(*method_index)
                        .ok_or("source interface ownership slot is out of bounds")?;
                    if target.source_definition.is_some()
                        || target.params.len() != method.params.len()
                        || target
                            .params
                            .iter()
                            .zip(&method.params)
                            .any(|(param, (_, ty, view))| {
                                param.ty != *ty || (param.mode == crate::ParamMode::View) != *view
                            })
                        || target.return_type != method.return_type
                    {
                        return Err(
                            "source interface ownership dispatcher differs from the checked slot"
                                .into(),
                        );
                    }
                }
                (
                    CallTarget::Indirect { signature_type },
                    CallBridge::Direct,
                    ExpressionKind::IndirectCall { callee, .. },
                ) => {
                    if callee.ty != *signature_type
                        || source.shape
                            != (CheckedInvocationShape::Function {
                                signature_type: *signature_type,
                            })
                    {
                        return Err(
                            "indirect call ownership signature differs from its actual callee"
                                .into(),
                        );
                    }
                }
                (
                    CallTarget::Intrinsic(source_id),
                    CallBridge::Direct,
                    ExpressionKind::Intrinsic { intrinsic, .. },
                ) if source_id == intrinsic => {
                    if !matches!(source.shape, CheckedInvocationShape::Intrinsic { intrinsic: id, .. } if id == *intrinsic)
                    {
                        return Err("source intrinsic ownership shape has another operation".into());
                    }
                }
                (
                    CallTarget::Declaration {
                        identity,
                        signature_type,
                    },
                    CallBridge::TrustedIntrinsic { intrinsic: bridge },
                    ExpressionKind::Intrinsic { intrinsic, .. },
                ) if bridge == intrinsic
                    && trusted_intrinsic_declaration(identity, *intrinsic)
                    && source.shape
                        == (CheckedInvocationShape::Function {
                            signature_type: *signature_type,
                        }) => {}
                (
                    CallTarget::Intrinsic(source_id),
                    CallBridge::JsonSource {
                        intrinsic: bridge,
                        function: helper,
                    },
                    ExpressionKind::Call { function, .. },
                ) if source_id == bridge && helper == function && json_intrinsic(*bridge) => {
                    let target = target.ok_or("source JSON ownership helper is absent")?;
                    if !json_helper_declaration(&target.identity, *bridge)
                        || target.capture_count != 0
                    {
                        return Err(
                            "source JSON ownership helper has no trusted closed identity".into(),
                        );
                    }
                    let CheckedInvocationShape::Intrinsic { result_type, .. } = source.shape else {
                        return Err("source JSON helper has no original intrinsic shape".into());
                    };
                    if target.return_type != result_type {
                        return Err("source JSON helper changed the checked result type".into());
                    }
                }
                _ => {
                    return Err(
                        "call ownership physical target has no exact checked source join".into(),
                    );
                }
            }
            for (index, argument) in source.arguments.iter().enumerate() {
                if let Some(target) = target {
                    let parameter = target
                        .params
                        .get(index)
                        .ok_or("ownership physical function parameter is missing")?;
                    if argument.physical_access != access(parameter.mode) {
                        return Err("call ownership physical function access changed".into());
                    }
                    if !source_parameter_matches(argument, args[index].ty, parameter.ty, types) {
                        return Err(
                            "call ownership converted operand differs from its physical parameter"
                                .into(),
                        );
                    }
                } else if let ExpressionKind::IndirectCall { callee, .. } = &expression.kind {
                    let Type::Function { params, .. } = types.resolve(callee.ty) else {
                        return Err("call ownership indirect callee is not a function".into());
                    };
                    let parameter_type = params
                        .get(index)
                        .copied()
                        .ok_or("call ownership indirect parameter is missing")?;
                    if !source_parameter_matches(argument, args[index].ty, parameter_type, types) {
                        return Err("call ownership indirect converted operand differs from its physical parameter".into());
                    }
                }
            }
        }
        CallOwnership::Generated(generated) => {
            if generated.operation != generated.certificate.operation
                || _order != &generated.certificate.evaluation_order
                || expression.ty != generated.certificate.result_type
            {
                return Err(
                    "generated invocation differs from its sealed operation/result/order".into(),
                );
            }
            if target.is_some_and(|target| target.params.len() != args.len()) {
                return Err(
                    "generated invocation differs from its exact physical target arity".into(),
                );
            }
            match (&generated.operation, &expression.kind) {
                (GeneratedOperation::Display { method }, ExpressionKind::Call { function, .. })
                    if method == function =>
                {
                    let target = target.ok_or("display ownership method is absent")?;
                    if target.params.len() != 1
                        || target.params[0].mode != crate::ParamMode::View
                        || target.return_type != TypeInterner::STRING
                        || target.identity.declaration.kind != crate::DeclarationKind::Method
                    {
                        return Err(
                            "generated display ownership has an invalid method shape".into()
                        );
                    }
                }
                (
                    GeneratedOperation::Equality { method },
                    ExpressionKind::Call { function, .. },
                ) if method == function => {
                    let target = target.ok_or("equality ownership method is absent")?;
                    if target.params.len() != 2
                        || target.params[0].ty != target.params[1].ty
                        || target
                            .params
                            .iter()
                            .any(|param| param.mode != crate::ParamMode::View)
                        || target.return_type != TypeInterner::BOOL
                        || target.identity.declaration.kind != crate::DeclarationKind::Method
                    {
                        return Err(
                            "generated equality ownership has an invalid method shape".into()
                        );
                    }
                }
                (
                    GeneratedOperation::InterfaceDispatch {
                        function: expected,
                        interface_type,
                        method_index,
                    },
                    ExpressionKind::Call { function, .. },
                ) if expected == function => {
                    if !valid_type(types, *interface_type) {
                        return Err("generated dispatch owner is outside its interner".into());
                    }
                    let Type::Interface(id) = types.resolve(*interface_type) else {
                        return Err("generated dispatch owner is not an interface".into());
                    };
                    let slot = types
                        .resolve_interface(*id)
                        .methods
                        .get(*method_index)
                        .ok_or("generated dispatch slot is out of bounds")?;
                    let target = target.ok_or("generated dispatch implementation is absent")?;
                    if target.identity.declaration.kind != crate::DeclarationKind::Method
                        || slot.params.len() != target.params.len()
                        || target.return_type != slot.return_type
                        || slot
                            .params
                            .iter()
                            .zip(&target.params)
                            .any(|((_, _, view), param)| {
                                *view != (param.mode == crate::ParamMode::View)
                            })
                    {
                        return Err(
                            "generated dispatch implementation differs from its interface slot"
                                .into(),
                        );
                    }
                }
                (
                    GeneratedOperation::FunctionAdapter {
                        callee: expected,
                        source_type,
                        target_type,
                    },
                    ExpressionKind::IndirectCall { callee, .. },
                ) => {
                    if !matches!(callee.kind, ExpressionKind::Local(local) if local == *expected)
                        || callee.ty != *source_type
                        || !valid_type(types, *source_type)
                        || !valid_type(types, *target_type)
                        || !matches!(types.resolve(*target_type), Type::Function { .. })
                    {
                        return Err("generated callable adapter ownership types are invalid".into());
                    }
                }
                (
                    GeneratedOperation::RefinementPredicate {
                        function: predicate,
                    },
                    ExpressionKind::Call { function, .. },
                ) if predicate == function => {
                    let target = target.ok_or("generated refinement predicate is absent")?;
                    if target.params.len() != 1
                        || target.params[0].mode != crate::ParamMode::Owned
                        || target.capture_count != 0
                        || target.return_type != TypeInterner::BOOL
                        || target.identity.declaration.kind
                            != crate::DeclarationKind::RefinementPredicate
                    {
                        return Err(
                            "generated refinement ownership has an invalid predicate shape".into(),
                        );
                    }
                }
                (
                    GeneratedOperation::NativeSuite { function: expected },
                    ExpressionKind::Call { function, .. },
                ) if expected == function => {
                    let target = target.ok_or("generated native suite target is absent")?;
                    if !matches!(
                        target.identity.declaration.kind,
                        crate::DeclarationKind::Verify | crate::DeclarationKind::Property
                    ) || target.return_type != TypeInterner::NOTHING
                        || target.capture_count != 0
                    {
                        return Err(
                            "generated native suite has no exact checked verify/property target"
                                .into(),
                        );
                    }
                }
                (
                    GeneratedOperation::EvaluatedValue {
                        intrinsic: expected,
                    },
                    ExpressionKind::Intrinsic {
                        intrinsic,
                        type_arguments,
                        ..
                    },
                ) if expected == intrinsic => {
                    validate_evaluated_value_shape(
                        *intrinsic,
                        type_arguments,
                        args,
                        expression.ty,
                        &generated.arguments,
                        types,
                    )?;
                }
                (
                    GeneratedOperation::ReflectionMetadata {
                        intrinsic: expected,
                    },
                    ExpressionKind::Intrinsic {
                        intrinsic,
                        type_arguments,
                        args,
                        ..
                    },
                ) if *expected == IntrinsicId::TypeName
                    && expected == intrinsic
                    && args.is_empty()
                    && type_arguments.len() == 1
                    && valid_type(types, type_arguments[0])
                    && expression.ty == TypeInterner::STRING => {}
                _ => {
                    return Err(
                        "generated call ownership has no closed operation/target contract".into(),
                    );
                }
            }
            for (index, argument) in generated.arguments.iter().enumerate() {
                if let Some(target) = target {
                    let parameter = target
                        .params
                        .get(index)
                        .ok_or("generated physical function parameter is missing")?;
                    if argument.parameter_type != parameter.ty
                        || argument.callee_access != access(parameter.mode)
                    {
                        return Err(
                            "generated ownership differs from its physical function signature"
                                .into(),
                        );
                    }
                    if args[index].ty != parameter.ty
                        && !(matches!(
                            generated.operation,
                            GeneratedOperation::Display { .. }
                                | GeneratedOperation::Equality { .. }
                        ) && has_outer_secret(types, argument.actual_type)
                            && secret_taint_matches(types, args[index].ty, parameter.ty))
                    {
                        return Err(
                            "generated converted operand differs from its physical parameter"
                                .into(),
                        );
                    }
                } else if let ExpressionKind::IndirectCall { callee, .. } = &expression.kind {
                    let Type::Function {
                        params,
                        view_params,
                        ..
                    } = types.resolve(callee.ty)
                    else {
                        return Err("generated indirect ownership callee is not a function".into());
                    };
                    if params.get(index) != Some(&argument.parameter_type)
                        || params.get(index) != Some(&args[index].ty)
                        || view_params.get(index).copied().map(|view| {
                            if view {
                                CheckedCalleeAccess::View
                            } else {
                                CheckedCalleeAccess::Owned
                            }
                        }) != Some(argument.callee_access)
                    {
                        return Err(
                            "generated indirect ownership differs from its descriptor signature"
                                .into(),
                        );
                    }
                }
            }
        }
    }
    let declared_result = if let ExpressionKind::ResourceInvoke { hook, .. } = &expression.kind {
        match types.resolve(hook.function_type()) {
            Type::Function { return_type, .. } => Some(*return_type),
            _ => None,
        }
    } else if let Some(target) = target {
        Some(target.return_type)
    } else if let ExpressionKind::IndirectCall { callee, .. } = &expression.kind {
        match types.resolve(callee.ty) {
            Type::Function { return_type, .. } => Some(*return_type),
            _ => None,
        }
    } else {
        None
    };
    if let Some(declared) = declared_result {
        let taint = match ownership {
            CallOwnership::Source(source) => expression.ty == source.certificate.result_type,
            CallOwnership::Generated(generated) => {
                matches!(
                    generated.operation,
                    GeneratedOperation::Display { .. } | GeneratedOperation::Equality { .. }
                ) && generated
                    .arguments
                    .iter()
                    .any(|argument| has_outer_secret(types, argument.actual_type))
            }
        };
        if expression.ty != declared
            && !(taint && secret_taint_matches(types, expression.ty, declared))
        {
            return Err(
                "call ownership physical result has no exact checked return/secret join".into(),
            );
        }
    }
    Ok(())
}

fn access(mode: crate::ParamMode) -> CheckedCalleeAccess {
    if mode == crate::ParamMode::View {
        CheckedCalleeAccess::View
    } else {
        CheckedCalleeAccess::Owned
    }
}

fn function_lookup(
    functions: &[crate::Function],
    id: FunctionId,
) -> Result<&crate::Function, String> {
    functions
        .get(id.index() as usize)
        .filter(|function| function.id == id)
        .ok_or_else(|| "ownership physical function is outside its program".into())
}

fn validate_function_shape(
    shape: &CheckedInvocationShape,
    target: &crate::Function,
    types: &TypeInterner,
) -> Result<(), String> {
    let CheckedInvocationShape::Function { signature_type } = shape else {
        return Err("source function ownership has an intrinsic shape".into());
    };
    let Type::Function {
        params,
        view_params,
        return_type,
    } = types.resolve(*signature_type)
    else {
        return Err("source function ownership signature is invalid".into());
    };
    if target.capture_count != 0
        || params.len() != target.params.len()
        || *return_type != target.return_type
        || target
            .params
            .iter()
            .zip(params)
            .zip(view_params)
            .any(|((parameter, ty), view)| {
                parameter.ty != *ty || (parameter.mode == crate::ParamMode::View) != *view
            })
    {
        return Err(
            "source function ownership signature differs from the exact HIR declaration".into(),
        );
    }
    Ok(())
}

fn source_operand<'a>(
    expression: &'a Expression,
    argument: &ArgumentOwnership,
    bridge: &CallBridge,
    types: &TypeInterner,
) -> Result<&'a Expression, String> {
    if expression.ty != argument.actual_type
        && matches!(
            bridge,
            CallBridge::JsonSource {
                intrinsic: IntrinsicId::JsonSerialize | IntrinsicId::JsonSerializePublic,
                ..
            }
        )
        && jett_typecheck::ownership::is_implicitly_copyable(types, argument.actual_type)
    {
        return json_scalar_original(expression, argument, types);
    }
    let mut value = expression;
    while value.span != argument.source_span {
        value = match &value.kind {
            ExpressionKind::View(inner) => inner,
            _ => return Err("call ownership actual lost its original typed occurrence".into()),
        };
    }
    while value.ty != argument.actual_type {
        if value.ty == argument.witness.occurrence_type
            && has_outer_secret(types, argument.parameter_type)
            && secret_taint_matches(types, value.ty, argument.actual_type)
        {
            break;
        }
        value = match &value.kind {
            ExpressionKind::InterfaceCoerce { value, .. }
            | ExpressionKind::FunctionAdapter { value, .. } => value,
            _ => {
                return Err("call ownership current operand has no typed conversion from the checked actual".into());
            }
        };
    }
    if value.span != argument.source_span {
        return Err("call ownership actual lost its original typed occurrence".into());
    }
    Ok(value)
}

fn json_scalar_original<'a>(
    expression: &'a Expression,
    argument: &ArgumentOwnership,
    types: &TypeInterner,
) -> Result<&'a Expression, String> {
    let ExpressionKind::View(tree) = &expression.kind else {
        return Err("source JSON scalar bridge requires a borrowed raw tree".into());
    };
    let ExpressionKind::EnumConstruct {
        enum_type,
        variant,
        payloads,
        evaluation_order,
    } = &tree.kind
    else {
        return Err("source JSON scalar bridge lost its raw tree construction".into());
    };
    let Type::Enum(id) = types.resolve(*enum_type) else {
        return Err("source JSON scalar bridge tree is not an enum".into());
    };
    let definition = types.resolve_enum(*id);
    let variant = definition
        .variants
        .get(variant.index() as usize)
        .ok_or("source JSON scalar bridge variant is invalid")?;
    if definition.name != "json.JsonTree"
        || tree.ty != *enum_type
        || expression.ty != *enum_type
        || payloads.len() != 1
        || evaluation_order != &[0]
        || payloads[0].ty != TypeInterner::STRING
    {
        return Err("source JSON scalar bridge raw shape is invalid".into());
    }
    let original = match (&payloads[0].kind, types.resolve(argument.actual_type)) {
        (ExpressionKind::Clone(original), Type::String) if variant.name == "string_value" => {
            original.as_ref()
        }
        (
            ExpressionKind::StringInterpolation(parts),
            Type::Int8
            | Type::Int16
            | Type::Int32
            | Type::Int64
            | Type::Uint8
            | Type::Uint16
            | Type::Uint32
            | Type::Uint64
            | Type::Float32
            | Type::Float64,
        ) if variant.name == "number_value" => {
            let [crate::StringSegment::Value(original)] = parts.as_slice() else {
                return Err(
                    "source JSON numeric bridge must evaluate exactly one source value".into(),
                );
            };
            original
        }
        _ => return Err("source JSON scalar bridge does not match the original primitive".into()),
    };
    if original.ty != argument.actual_type || original.span != argument.source_span {
        return Err("source JSON scalar bridge lost its exact original occurrence".into());
    }
    Ok(original)
}

/// Prove the original checked expression, its finite conversion and source place.
/// This accepts an original expression, never a lowered result slot by itself.
pub fn validate_source_operand<'a>(
    expression: &'a Expression,
    argument: &ArgumentOwnership,
    bridge: &CallBridge,
    locals: &[OwnershipLocalInfo],
    types: &TypeInterner,
) -> Result<&'a Expression, String> {
    let original = source_operand(expression, argument, bridge, types)?;
    if let Some(backing) = validate_source_handled_operand(expression, argument, bridge, types)?
        && !matches!(backing.kind, ExpressionKind::Handle { .. })
    {
        return Err("call ownership original handled occurrence is no longer a Handle".into());
    }
    validate_operand_origin(original, argument, locals, types)?;
    Ok(original)
}

fn source_view_backing(mut value: &Expression) -> &Expression {
    while let ExpressionKind::View(inner) = &value.kind {
        value = inner;
    }
    value
}

/// Rejoin a privately retained Handle occurrence through its original finite
/// conversion. MIR must independently prove a lowered Local's complete CFG;
/// this occurrence check never grants an owner or manufactures a Binding.
pub fn validate_source_handled_operand<'a>(
    expression: &'a Expression,
    argument: &ArgumentOwnership,
    bridge: &CallBridge,
    types: &TypeInterner,
) -> Result<Option<&'a Expression>, String> {
    let Some(result) = argument.witness.handled_result else {
        return Ok(None);
    };
    if argument.origin != CallerOrigin::OwnedExpression {
        return Err("call ownership handled occurrence is not an original source producer".into());
    }
    let original = source_operand(expression, argument, bridge, types)?;
    let backing = source_view_backing(original);
    if backing.ty != result.ty || backing.span != result.span {
        return Err(
            "call ownership handled backing differs from its original typed occurrence".into(),
        );
    }
    Ok(Some(backing))
}

fn validate_operand_origin(
    value: &Expression,
    argument: &ArgumentOwnership,
    locals: &[OwnershipLocalInfo],
    types: &TypeInterner,
) -> Result<(), String> {
    // This is only the exact checked SOURCE projection proof. The runtime tree
    // and any nested call's sealed promoted result certificate stay unchanged.
    let raw_occurrence = if value.ty != argument.actual_type
        && value.ty == argument.witness.occurrence_type
        && has_outer_secret(types, argument.parameter_type)
        && secret_taint_matches(types, value.ty, argument.actual_type)
    {
        let mut raw = value.clone();
        raw.ty = argument.actual_type;
        let mut place = &mut raw;
        while let ExpressionKind::View(inner) = &mut place.kind {
            if inner.ty == argument.witness.occurrence_type {
                inner.ty = argument.actual_type;
            }
            place = inner;
        }
        Some(raw)
    } else {
        None
    };
    let value = raw_occurrence.as_ref().unwrap_or(value);
    let mut leaf = value;
    loop {
        leaf = match &leaf.kind {
            ExpressionKind::View(inner)
            | ExpressionKind::Coarsen(inner)
            | ExpressionKind::Declassify(inner)
            | ExpressionKind::RefinementValidated(inner)
            | ExpressionKind::InterfaceCoerce { value: inner, .. }
            | ExpressionKind::FunctionAdapter { value: inner, .. } => inner,
            _ => break,
        };
    }
    match &argument.origin {
        CallerOrigin::Binding(fact) => {
            let checked_state_narrowing = value.ty == leaf.ty
                && leaf.ty == argument.actual_type
                && leaf.ty == argument.witness.occurrence_type
                && machine_state_parent_matches(types, leaf.ty, fact.ty);
            if !matches!(leaf.kind, ExpressionKind::Local(local) if local == fact.local)
                || (leaf.ty != fact.ty && !checked_state_narrowing)
            {
                return Err(
                    "call ownership binding does not match its actual typed operand".into(),
                );
            }
        }
        CallerOrigin::BorrowedProjection { source }
        | CallerOrigin::OwnedFieldCopy { parent: source } => {
            if !matches!(leaf.kind, ExpressionKind::Field { .. })
                || immediate_source(leaf) != *source
            {
                return Err(
                    "call ownership projection does not match its immediate checked source".into(),
                );
            }
            if let CallerViewSource::Local(id) = source {
                let parent = locals
                    .get(id.index() as usize)
                    .ok_or("call projection source is outside its function")?;
                if let Some(root) = argument.witness.projection_root {
                    validate_flow_projection_operand(value, *id, parent.ty, root, types)?;
                } else {
                    crate::validate_local_view_initializer(value, *id, parent.ty, value.ty, types)
                        .map_err(str::to_string)?;
                }
            } else {
                let mut field = leaf;
                while let ExpressionKind::Field { base, .. } = &field.kind {
                    crate::local_views::validate_field_projection(field, types)
                        .map_err(str::to_string)?;
                    field = base;
                }
            }
        }
        CallerOrigin::OwnedExpression => {
            if matches!(
                leaf.kind,
                ExpressionKind::Local(_) | ExpressionKind::Field { .. }
            ) {
                return Err("call ownership cannot manufacture a producer acquisition from a binding or field".into());
            }
        }
    }
    Ok(())
}

fn validate_flow_projection_operand(
    value: &Expression,
    source: LocalId,
    storage_type: TypeId,
    root: SourceProjectionRoot,
    types: &TypeInterner,
) -> Result<(), String> {
    if root.local != source
        || root.storage_type != storage_type
        || !flow_projection_root_matches(types, root.occurrence_type, storage_type)
    {
        return Err("call ownership flow projection changed its declared root storage".into());
    }
    // Every existing field/wrapper/endpoint check remains strict. Only this
    // privately certified source occurrence supplies its exact flowed root type.
    crate::validate_local_view_initializer(value, source, root.occurrence_type, value.ty, types)
        .map_err(str::to_string)?;
    let mut terminal = value;
    loop {
        terminal = match &terminal.kind {
            ExpressionKind::Field { base, .. } => base,
            ExpressionKind::View(inner)
            | ExpressionKind::Coarsen(inner)
            | ExpressionKind::Declassify(inner) => inner,
            ExpressionKind::InterfaceCoerce { value, adapters } if adapters.is_empty() => value,
            ExpressionKind::Local(id)
                if *id == root.local
                    && terminal.span == root.span
                    && terminal.ty == root.occurrence_type =>
            {
                return Ok(());
            }
            _ => {
                return Err(
                    "call ownership flow projection changed its exact source occurrence".into(),
                );
            }
        };
    }
}

fn flow_projection_root_matches(types: &TypeInterner, occurrence: TypeId, storage: TypeId) -> bool {
    if !valid_type(types, occurrence) || !valid_type(types, storage) {
        return false;
    }
    let (Type::MachineState { machine, state }, Type::Machine(parent)) =
        (types.resolve(occurrence), types.resolve(storage))
    else {
        return false;
    };
    machine == parent && types.resolve_machine(*machine).state(*state).is_some()
}

pub fn validate_generated_operand_tree(
    value: &Expression,
    argument: &GeneratedArgumentOwnership,
    locals: &[OwnershipLocalInfo],
) -> Result<(), String> {
    if !generated_tuple_matches(argument) {
        return Err("generated operand differs from its sealed acquisition witness".into());
    }
    let mut original = value;
    // Physical View wrapping is separate from the original generated operand.
    if !argument.witness.written_view
        && argument.callee_access == CheckedCalleeAccess::View
        && let ExpressionKind::View(inner) = &original.kind
        && original.span == inner.span
        && original.ty == inner.ty
    {
        original = inner;
    }
    // A canonical conversion keeps the physical occurrence header, but may
    // allocate an adapter around the sealed raw parameter. Rejoin its backing
    // only after the existing exact-actual-type conversion walk below.
    if original.span != argument.witness.source_span {
        return Err(
            "generated operand differs from its original typed occurrence or backing".into(),
        );
    }
    while original.ty != argument.actual_type {
        original = match &original.kind {
            ExpressionKind::InterfaceCoerce { value, .. }
            | ExpressionKind::FunctionAdapter { value, .. } => value,
            _ => return Err("generated operand has no checked actual/conversion type".into()),
        };
    }
    if original.span != argument.witness.source_span
        || original_view(original) != argument.witness.written_view
        || immediate_source(original) != argument.witness.origin
    {
        return Err(
            "generated operand differs from its original typed occurrence or backing".into(),
        );
    }
    argument.witness.validate_producer_shape(original)?;
    match argument.acquisition {
        GeneratedAcquisition::Borrow { source } if immediate_source(original) != source => {
            return Err("generated borrow origin differs from its operand".into());
        }
        GeneratedAcquisition::OwnedExpression => {
            if original_view(original) {
                return Err("generated owner cannot acquire an explicit borrowed operand".into());
            }
            if let CallerViewSource::Local(id) = immediate_source(original) {
                let local = locals
                    .get(id.index() as usize)
                    .ok_or("generated operand source is outside its function")?;
                if local.view_source.is_some()
                    || local.is_view_parameter
                    || local.view_iteration.is_some()
                {
                    return Err("generated owner cannot acquire a borrowed binding".into());
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn metadata_operation(source: &SourceCallOwnership) -> Option<IntrinsicId> {
    match (&source.target, &source.bridge) {
        (CallTarget::Intrinsic(id), CallBridge::Direct)
            if matches!(
                id,
                IntrinsicId::TypeFieldValue
                    | IntrinsicId::TypeVariantFieldValue
                    | IntrinsicId::TypeMachineFieldValue
                    | IntrinsicId::TypeVariantValue
                    | IntrinsicId::TypeMachineStateValue
                    | IntrinsicId::TypeArg
            ) =>
        {
            Some(*id)
        }
        _ => None,
    }
}

fn canonical_metadata_enum(types: &TypeInterner, ty: TypeId) -> bool {
    if !valid_type(types, ty) {
        return false;
    }
    let Type::Enum(id) = types.resolve(ty) else {
        return false;
    };
    let definition = types.resolve_enum(*id);
    let names: &[&str] = match definition.name.as_str() {
        "TypeKind" => &[
            "primitive_type",
            "alias_type",
            "refinement_type",
            "struct_type",
            "bitfield_type",
            "enum_type",
            "list_type",
            "set_type",
            "map_type",
            "optional_type",
            "result_type",
            "secret_type",
            "function_type",
            "machine_type",
            "machine_state_type",
            "unknown_type",
            "resource_type",
        ],
        "TypePrimitive" => &[
            "int8_type",
            "int16_type",
            "int32_type",
            "int64_type",
            "uint8_type",
            "uint16_type",
            "uint32_type",
            "uint64_type",
            "float32_type",
            "float64_type",
            "string_type",
            "bool_type",
            "bytes_type",
            "nothing_type",
            "type_construction_type",
            "unknown_type",
        ],
        _ => return false,
    };
    definition.variants.len() == names.len()
        && definition
            .variants
            .iter()
            .zip(names)
            .enumerate()
            .all(|(index, (variant, name))| {
                variant.name == *name
                    && variant.fields.is_empty()
                    && variant.discriminant == index as i64
            })
}

fn canonical_metadata_type(types: &TypeInterner, ty: TypeId) -> bool {
    fn visit(
        types: &TypeInterner,
        ty: TypeId,
        seen: &mut std::collections::HashSet<TypeId>,
    ) -> bool {
        if !valid_type(types, ty) || !types.nominal_type_arguments(ty).is_empty() {
            return false;
        }
        if !seen.insert(ty) {
            return true;
        }
        match types.resolve(ty) {
            Type::Int64 | Type::String | Type::Bool => true,
            Type::Optional(inner) | Type::List(inner) => visit(types, *inner, seen),
            Type::Enum(_) => canonical_metadata_enum(types, ty),
            Type::Struct(id) => {
                let definition = types.resolve_struct(*id);
                if !definition.methods.is_empty() {
                    return false;
                }
                let fields = &definition.fields;
                let names: &[&str] = match definition.name.as_str() {
                    "TypeInfo" => &[
                        "type_name",
                        "kind",
                        "kind_tag",
                        "primitive_tag",
                        "has_secret",
                        "args",
                    ],
                    "TypeField" => &[
                        "index",
                        "owner_type",
                        "owner_member",
                        "name",
                        "type_name",
                        "kind",
                        "kind_tag",
                        "serialize_name",
                        "has_secret",
                        "type_info",
                    ],
                    "TypeVariant" => &[
                        "index",
                        "owner_type",
                        "name",
                        "discriminant",
                        "has_secret",
                        "fields",
                    ],
                    "TypeMachineState" => &["index", "owner_type", "name", "has_secret", "fields"],
                    _ => return false,
                };
                if !fields
                    .iter()
                    .map(|(name, _)| name.as_str())
                    .eq(names.iter().copied())
                {
                    return false;
                }
                let field = |index: usize| fields[index].1;
                let named = |candidate: TypeId, name: &str| {
                    valid_type(types, candidate)
                        && match types.resolve(candidate) {
                            Type::Struct(id) => types.resolve_struct(*id).name == name,
                            Type::Enum(id) => types.resolve_enum(*id).name == name,
                            _ => false,
                        }
                };
                let optional = |candidate: TypeId, payload: TypeId| {
                    valid_type(types, candidate)
                        && matches!(types.resolve(candidate), Type::Optional(inner) if *inner == payload)
                };
                let list_named = |candidate: TypeId, name: &str| {
                    valid_type(types, candidate)
                        && matches!(types.resolve(candidate), Type::List(inner) if named(*inner, name))
                };
                let shape = match definition.name.as_str() {
                    "TypeInfo" => {
                        field(0) == TypeInterner::STRING
                            && field(1) == TypeInterner::STRING
                            && named(field(2), "TypeKind")
                            && valid_type(types, field(3))
                            && matches!(types.resolve(field(3)), Type::Optional(inner) if named(*inner, "TypePrimitive"))
                            && field(4) == TypeInterner::BOOL
                            && valid_type(types, field(5))
                            && matches!(types.resolve(field(5)), Type::List(inner) if *inner == ty)
                    }
                    "TypeField" => {
                        field(0) == TypeInterner::INT64
                            && [1, 3, 4, 5, 7]
                                .into_iter()
                                .all(|index| field(index) == TypeInterner::STRING)
                            && optional(field(2), TypeInterner::STRING)
                            && named(field(6), "TypeKind")
                            && field(8) == TypeInterner::BOOL
                            && named(field(9), "TypeInfo")
                    }
                    "TypeVariant" => {
                        field(0) == TypeInterner::INT64
                            && field(1) == TypeInterner::STRING
                            && field(2) == TypeInterner::STRING
                            && field(3) == TypeInterner::INT64
                            && field(4) == TypeInterner::BOOL
                            && list_named(field(5), "TypeField")
                    }
                    "TypeMachineState" => {
                        field(0) == TypeInterner::INT64
                            && field(1) == TypeInterner::STRING
                            && field(2) == TypeInterner::STRING
                            && field(3) == TypeInterner::BOOL
                            && list_named(field(4), "TypeField")
                    }
                    _ => false,
                };
                shape && fields.iter().all(|(_, child)| visit(types, *child, seen))
            }
            _ => false,
        }
    }
    visit(types, ty, &mut std::collections::HashSet::new())
}

fn metadata_root_matches(source: &SourceCallOwnership, ty: TypeId, types: &TypeInterner) -> bool {
    let Some(operation) = metadata_operation(source) else {
        return false;
    };
    if !canonical_metadata_type(types, ty) {
        return false;
    }
    let Type::Struct(id) = types.resolve(ty) else {
        return false;
    };
    let expected = match operation {
        IntrinsicId::TypeFieldValue
        | IntrinsicId::TypeVariantFieldValue
        | IntrinsicId::TypeMachineFieldValue => "TypeField",
        IntrinsicId::TypeVariantValue => "TypeVariant",
        IntrinsicId::TypeMachineStateValue => "TypeMachineState",
        IntrinsicId::TypeArg => "TypeInfo",
        _ => return false,
    };
    types.resolve_struct(*id).name == expected
        && match (&source.shape, operation) {
            (
                CheckedInvocationShape::Intrinsic { operands, .. },
                IntrinsicId::TypeFieldValue
                | IntrinsicId::TypeVariantFieldValue
                | IntrinsicId::TypeMachineFieldValue,
            ) => operands.get(1).is_some_and(|role| role.ty() == ty),
            (CheckedInvocationShape::Intrinsic { result_type, .. }, _) => *result_type == ty,
            _ => false,
        }
}

/// Compiler metadata is a closed constant construction, never a borrowed or
/// cloned source payload. MIR must reconstruct a lowered producer before use.
pub fn validate_compiler_metadata_operand(
    value: &Expression,
    source: &SourceCallOwnership,
    types: &TypeInterner,
) -> Result<(), String> {
    fn constant(value: &Expression, types: &TypeInterner) -> bool {
        if !canonical_metadata_type(types, value.ty) {
            return false;
        }
        match (&value.kind, types.resolve(value.ty)) {
            (ExpressionKind::Int(_), Type::Int64)
            | (ExpressionKind::String(_), Type::String)
            | (ExpressionKind::Bool(_), Type::Bool) => true,
            (ExpressionKind::OptionalNone, Type::Optional(_)) => true,
            (ExpressionKind::OptionalSome(value), Type::Optional(inner)) => {
                value.ty == *inner && constant(value, types)
            }
            (ExpressionKind::ListConstruct { elements }, Type::List(inner)) => elements
                .iter()
                .all(|value| value.ty == *inner && constant(value, types)),
            (
                ExpressionKind::EnumConstruct {
                    enum_type,
                    variant,
                    payloads,
                    evaluation_order,
                },
                Type::Enum(id),
            ) => {
                *enum_type == value.ty
                    && payloads.is_empty()
                    && evaluation_order.is_empty()
                    && types
                        .resolve_enum(*id)
                        .variants
                        .get(variant.index() as usize)
                        .is_some_and(|variant| variant.fields.is_empty())
            }
            (
                ExpressionKind::StructConstruct {
                    struct_type,
                    fields,
                    evaluation_order,
                    validates_refinements,
                    refinement_predicates,
                },
                Type::Struct(id),
            ) => {
                let definition = types.resolve_struct(*id);
                *struct_type == value.ty
                    && !*validates_refinements
                    && refinement_predicates.is_empty()
                    && evaluation_order.iter().copied().eq(0..fields.len())
                    && fields.len() == definition.fields.len()
                    && fields
                        .iter()
                        .zip(&definition.fields)
                        .all(|(value, (_, ty))| value.ty == *ty && constant(value, types))
            }
            _ => false,
        }
    }
    if metadata_root_matches(source, value.ty, types) && constant(value, types) {
        Ok(())
    } else {
        Err(
            "call ownership compiler metadata tail has no canonical owned construction proof"
                .into(),
        )
    }
}

fn secret_taint_matches(types: &TypeInterner, mut actual: TypeId, declared: TypeId) -> bool {
    if !valid_type(types, actual) || !valid_type(types, declared) {
        return false;
    }
    for _ in 0..types.len() {
        if actual == declared {
            return true;
        }
        let Type::Secret(inner) = types.resolve(actual) else {
            return false;
        };
        actual = *inner;
    }
    false
}

fn has_outer_secret(types: &TypeInterner, ty: TypeId) -> bool {
    valid_type(types, ty) && matches!(types.resolve(ty), Type::Secret(_))
}

/// Locate only the first Secret boundary behind nominal refinement bases.
/// This is metadata for an exact checked actual, not refinement coarsening.
fn nominal_secret_payload(types: &TypeInterner, mut actual: TypeId) -> Option<TypeId> {
    if !valid_type(types, actual) || !matches!(types.resolve(actual), Type::Refinement { .. }) {
        return None;
    }
    for _ in 0..types.len() {
        if !valid_type(types, actual) {
            return None;
        }
        match types.resolve(actual) {
            Type::Refinement { base, .. } => actual = *base,
            Type::Secret(inner) => return valid_type(types, *inner).then_some(*inner),
            _ => return None,
        }
    }
    None
}

// The checker admits only this one-way nominal state erasure. No state is
// recovered or substituted, and the checked actual occurrence stays exact.
fn machine_state_parent_matches(types: &TypeInterner, actual: TypeId, declared: TypeId) -> bool {
    if !valid_type(types, actual) || !valid_type(types, declared) {
        return false;
    }
    matches!(
        (types.resolve(actual), types.resolve(declared)),
        (Type::MachineState { machine, .. }, Type::Machine(parent)) if machine == parent
    )
}

fn source_parameter_matches(
    argument: &ArgumentOwnership,
    physical: TypeId,
    declared: TypeId,
    types: &TypeInterner,
) -> bool {
    physical == declared
        || (physical == argument.actual_type
            && physical == argument.witness.occurrence_type
            && declared == argument.parameter_type
            && machine_state_parent_matches(types, physical, declared))
        || (has_outer_secret(types, argument.actual_type)
            && secret_taint_matches(types, physical, declared))
        || (physical == argument.actual_type
            && physical == argument.witness.occurrence_type
            && declared == argument.parameter_type
            && nominal_secret_payload(types, physical) == Some(declared))
        || (physical == argument.actual_type
            && physical == argument.witness.occurrence_type
            && declared == argument.parameter_type
            && has_outer_secret(types, declared)
            && secret_taint_matches(types, declared, physical))
}

fn evaluated_key_type(types: &TypeInterner, mut ty: TypeId) -> bool {
    let mut seen = std::collections::HashSet::new();
    while valid_type(types, ty) && seen.insert(ty) {
        match types.resolve(ty) {
            Type::Refinement { base, .. } => ty = *base,
            Type::Int8
            | Type::Int16
            | Type::Int32
            | Type::Int64
            | Type::Uint8
            | Type::Uint16
            | Type::Uint32
            | Type::Uint64
            | Type::String
            | Type::Bool => return true,
            _ => return false,
        }
    }
    false
}

fn validate_evaluated_value_shape(
    intrinsic: IntrinsicId,
    type_arguments: &[TypeId],
    args: &[Expression],
    result: TypeId,
    arguments: &[GeneratedArgumentOwnership],
    types: &TypeInterner,
) -> Result<(), String> {
    if type_arguments.iter().any(|&ty| !valid_type(types, ty)) || !valid_type(types, result) {
        return Err("generated evaluated value has an unknown type argument or result".into());
    }
    let construction_result = || {
        matches!(types.resolve(result), Type::Result(ok, error)
        if *ok == TypeInterner::TYPE_CONSTRUCTION && *error == TypeInterner::STRING)
    };
    let metadata = |ty: TypeId, name: &str| {
        valid_type(types, ty)
            && canonical_metadata_type(types, ty)
            && matches!(types.resolve(ty), Type::Struct(id) if types.resolve_struct(*id).name == name)
    };
    let construction_owner = |ty: TypeId| {
        matches!(
            types.resolve(ty),
            Type::Struct(_)
                | Type::Bitfield(_)
                | Type::Enum(_)
                | Type::Machine(_)
                | Type::MachineState { .. }
        )
    };
    let shape = match intrinsic {
        IntrinsicId::SetNew => {
            type_arguments.len() == 1
                && args.is_empty()
                && evaluated_key_type(types, type_arguments[0])
                && matches!(types.resolve(result), Type::Set(inner) if *inner == type_arguments[0])
        }
        IntrinsicId::SetAdd => {
            type_arguments.len() == 1
                && args.len() == 2
                && evaluated_key_type(types, type_arguments[0])
                && args[0].ty == result
                && args[1].ty == type_arguments[0]
                && matches!(types.resolve(result), Type::Set(inner) if *inner == type_arguments[0])
        }
        IntrinsicId::BytesNew => {
            type_arguments.is_empty() && args.is_empty() && result == TypeInterner::BYTES
        }
        IntrinsicId::BytesFromHex => {
            type_arguments.is_empty()
                && args.len() == 1
                && args[0].ty == TypeInterner::STRING
                && matches!(types.resolve(result), Type::Result(ok, error) if *ok == TypeInterner::BYTES && *error == TypeInterner::STRING)
        }
        IntrinsicId::TypeConstructStart => {
            type_arguments.len() == 1
                && args.is_empty()
                && construction_owner(type_arguments[0])
                && result == TypeInterner::TYPE_CONSTRUCTION
        }
        IntrinsicId::TypeConstructVariantStart => {
            type_arguments.len() == 1
                && args.len() == 1
                && matches!(types.resolve(type_arguments[0]), Type::Enum(_))
                && metadata(args[0].ty, "TypeVariant")
                && construction_result()
        }
        IntrinsicId::TypeConstructMachineStart => {
            type_arguments.len() == 1
                && args.len() == 1
                && matches!(
                    types.resolve(type_arguments[0]),
                    Type::Machine(_) | Type::MachineState { .. }
                )
                && metadata(args[0].ty, "TypeMachineState")
                && construction_result()
        }
        IntrinsicId::TypeConstructPut => {
            type_arguments.len() == 2
                && args.len() == 3
                && construction_owner(type_arguments[0])
                && args[0].ty == TypeInterner::TYPE_CONSTRUCTION
                && metadata(args[1].ty, "TypeField")
                && args[2].ty == type_arguments[1]
                && construction_result()
        }
        _ => false,
    };
    if !shape {
        return Err("generated evaluated value has no closed intrinsic/type/result shape".into());
    }
    for (index, argument) in arguments.iter().enumerate() {
        let access = jett_typecheck::intrinsic_operand_access(intrinsic, index, args.len())
            .ok_or("generated evaluated value operand has no closed access")?;
        if argument.callee_access != access || argument.parameter_type != args[index].ty {
            return Err(
                "generated evaluated value ownership differs from its closed physical operand"
                    .into(),
            );
        }
    }
    Ok(())
}
