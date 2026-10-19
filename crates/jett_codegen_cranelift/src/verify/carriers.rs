//! The generic native value visitor does not grant an aggregate Resource ABI.
//! Only exact occurrences in the freshly reconstructed carrier plan reach the
//! dedicated emitter; complete current-body/type seals are checked before here.
use super::*;
use jett_hir::IntrinsicId;
use jett_mir::ResourceCarrierOperationRole as Role;

impl Verifier<'_> {
    pub(super) fn carrier_expression(
        &self,
        function: &Function,
        expression: &Expression,
    ) -> Result<Option<()>, CodegenError> {
        let Some(ownership) = self.resource else {
            return Ok(None);
        };
        let Some(plan) = ownership
            .function(function.id)
            .and_then(|plan| plan.carriers())
        else {
            return Ok(None);
        };
        let operations = plan
            .operations_for_expression(expression)
            .collect::<Vec<_>>();
        if operations.is_empty() {
            return Ok(None);
        }
        // Source invocation continues through the existing exact original tuple,
        // callee/signature checks. A carrier row cannot replace that authority.
        if ownership.function(function.id).is_some_and(|plan| {
            plan.operations_for_expression(expression).any(|operation| {
                matches!(
                    operation.role(),
                    jett_mir::ResourceOperationRole::InvokeSourceFunction { .. }
                        | jett_mir::ResourceOperationRole::InvokeHook { .. }
                )
            })
        }) {
            return Ok(None);
        }
        match &expression.kind {
            ExpressionKind::ListConstruct { elements } => {
                self.require_carrier_constructor(function, expression, plan, &operations)?;
                for child in elements {
                    self.expression(function, child)?;
                }
            }
            ExpressionKind::MapConstruct { entries } => {
                self.require_carrier_constructor(function, expression, plan, &operations)?;
                for child in entries {
                    self.expression(function, &child.key)?;
                    self.expression(function, &child.value)?;
                }
            }
            ExpressionKind::StructConstruct { fields, .. }
            | ExpressionKind::EnumConstruct {
                payloads: fields, ..
            }
            | ExpressionKind::MachineConstruct {
                payloads: fields, ..
            } => {
                self.require_carrier_constructor(function, expression, plan, &operations)?;
                for child in fields {
                    self.expression(function, child)?;
                }
            }
            ExpressionKind::OptionalNone => {
                self.require_carrier_constructor(function, expression, plan, &operations)?;
            }
            ExpressionKind::OptionalSome(inner)
            | ExpressionKind::ResultOk(inner)
            | ExpressionKind::ResultFail(inner) => {
                self.require_carrier_constructor(function, expression, plan, &operations)?;
                self.expression(function, inner)?;
            }
            ExpressionKind::InterfaceCoerce { value, adapters } if adapters.is_empty() => {
                let transfers = operations
                    .iter()
                    .filter_map(|operation| match operation.role() {
                        Role::Transfer {
                            source,
                            destination,
                        }
                        | Role::QualifyMachine {
                            source,
                            destination,
                            ..
                        } => Some((*source, *destination)),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let [(source, destination)] = transfers.as_slice() else {
                    return Err(self.contract_error(
                        function,
                        expression.span,
                        "carrier qualification lacks its single exact private transfer",
                    ));
                };
                let source_type = plan
                    .slots()
                    .get(source.index())
                    .and_then(|slot| plan.shape(slot.shape()))
                    .map(|shape| shape.ty());
                let destination_type = plan
                    .slots()
                    .get(destination.index())
                    .and_then(|slot| plan.shape(slot.shape()))
                    .map(|shape| shape.ty());
                if source_type != Some(value.ty) || destination_type != Some(expression.ty) {
                    return Err(self.contract_error(
                        function,
                        expression.span,
                        "carrier qualification changes its exact source/destination type",
                    ));
                }
                self.expression(function, value)?;
            }
            ExpressionKind::Field { base, .. } => {
                if !operations.iter().any(|operation| {
                    matches!(
                        operation.role(),
                        Role::Extract { .. }
                            | Role::AdaptSum { .. }
                            | Role::Observe { .. }
                            | Role::Borrow { .. }
                    )
                }) {
                    return Err(self.contract_error(
                        function,
                        expression.span,
                        "carrier field lacks its exact projection/parent lease",
                    ));
                }
                self.expression(function, base)?;
            }
            ExpressionKind::StateIs { value, state } => {
                if !operations.iter().any(|operation| {
                    matches!(operation.role(), Role::Observe {
                    observation: jett_mir::ResourceCarrierObservation::State { state: expected }, ..
                } if u32::try_from(*expected).ok() == Some(state.index()))
                }) {
                    return Err(self.contract_error(
                        function,
                        expression.span,
                        "carrier state observation changes its checked selected state",
                    ));
                }
                self.expression(function, value)?;
            }
            ExpressionKind::Intrinsic {
                intrinsic, args, ..
            } if matches!(intrinsic, IntrinsicId::ListLength | IntrinsicId::MapLength)
                && args.len() == 1 =>
            {
                if !operations.iter().any(|operation| {
                    matches!(
                        operation.role(),
                        Role::Observe {
                            observation: jett_mir::ResourceCarrierObservation::Length,
                            ..
                        }
                    )
                }) {
                    return Err(self.contract_error(
                        function,
                        expression.span,
                        "carrier length lacks its exact observation row",
                    ));
                }
                self.expression(function, &args[0])?;
            }
            _ => {
                return Err(self.contract_error(
                    function,
                    expression.span,
                    "carrier occurrence does not have its dedicated exact native value transport",
                ));
            }
        }
        Ok(Some(()))
    }
    fn require_carrier_constructor(
        &self,
        function: &Function,
        expression: &Expression,
        plan: &jett_mir::ResourceCarrierFunctionPlan,
        operations: &[&jett_mir::ResourceCarrierOperation],
    ) -> Result<(), CodegenError> {
        let mut begins = operations
            .iter()
            .filter_map(|operation| match operation.role() {
                Role::BeginConstructor { destination, .. } => Some(*destination),
                _ => None,
            });
        let destination = begins.next().ok_or_else(|| {
            self.contract_error(
                function,
                expression.span,
                "carrier constructor lost its exact private Begin",
            )
        })?;
        if begins.next().is_some()
            || plan.slots().get(destination.index()).and_then(|slot| plan.shape(slot.shape())).map(|shape| shape.ty()) != Some(expression.ty)
            || operations.iter().filter(|operation| matches!(operation.role(), Role::CommitConstructor { destination: selected } if *selected == destination)).count() != 1
        {
            return Err(self.contract_error(function, expression.span, "carrier constructor changes its exact node/destination/commit"));
        }
        Ok(())
    }
}
