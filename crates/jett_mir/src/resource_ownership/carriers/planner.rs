//! Fresh projection of the complete authenticated current graph. The original
//! checker owns Source move legality; runtime generations own physical custody.
use super::*;
use jett_typecheck::{
    CheckedCalleeAccess as Access, CheckedCallerEffect as Effect, CheckedCallerSyntax as Syntax,
};

struct Planner<'p> {
    program: &'p Program,
    function: &'p Function,
    types: &'p TypeInterner,
    plan: ResourceFunctionPlan,
    carriers: ResourceCarrierFunctionPlan,
    carrier_locals: Vec<Option<ResourceCarrierValue>>,
    leaf_locals: Vec<Option<ResourceCarrierValue>>,
    site: ResourceSite,
    expression: Option<usize>,
    ordinal: usize,
    frame: ResourceFrameId,
    call_frames: BTreeMap<ResourceCallRegionId, ResourceFrameId>,
    iteration_slots: BTreeMap<u32, ResourceCarrierSlotId>,
    edge: Option<ResourceCarrierEdge>,
    ended_loans: BTreeSet<ResourceCarrierLoanId>,
}
impl Planner<'_> {
    fn shape_id(&self, ty: TypeId) -> Result<ResourceCarrierShapeId, String> {
        self.carriers
            .shape_for_type(ty)
            .map(ResourceCarrierShape::id)
            .ok_or_else(|| "carrier operation lost its exact checked type graph row".into())
    }
    fn slot(
        &mut self,
        ty: TypeId,
        storage: ResourceSlotStorage,
        frame: ResourceFrameId,
    ) -> Result<ResourceCarrierSlotId, String> {
        let shape = self.shape_id(ty)?;
        if !self
            .carriers
            .shape(shape)
            .is_some_and(ResourceCarrierShape::contains_resource)
        {
            return Err(
                "carrier slot cannot grant custody to ordinary data or a descriptor".into(),
            );
        }
        let id = ResourceCarrierSlotId(self.carriers.slots.len());
        self.carriers.slots.push(ResourceCarrierSlot {
            id,
            frame,
            shape,
            storage,
        });
        Ok(id)
    }
    fn temporary(&mut self, ty: TypeId) -> Result<ResourceCarrierSlotId, String> {
        self.slot(
            ty,
            ResourceSlotStorage::Expression {
                site: self.site,
                ordinal: self.ordinal,
            },
            self.frame,
        )
    }
    fn leaf_slot(
        &mut self,
        ty: TypeId,
        storage: ResourceSlotStorage,
        frame: ResourceFrameId,
    ) -> Result<ResourceOwnerSlotId, String> {
        let shape = shape(&self.program.resource_manifest, self.types, ty)?
            .ok_or("carrier leaf bridge has no exact existing leaf shape")?;
        let id = ResourceOwnerSlotId(self.plan.slots.len());
        self.plan.slots.push(ResourceOwnerSlot {
            id,
            frame,
            shape,
            storage,
        });
        Ok(id)
    }
    fn operation(&mut self, role: ResourceOperationRole) {
        let id = ResourceOperationId(self.plan.operations.len());
        self.plan.operations.push(ResourceOperation {
            id,
            frame: self.frame,
            site: self.site,
            ordinal: self.ordinal,
            role,
            expression: self.expression,
            named_indirect: None,
            indirect_hook: None,
        });
    }
    fn carrier_operation(
        &mut self,
        role: ResourceCarrierOperationRole,
    ) -> ResourceCarrierOperationId {
        let id = ResourceCarrierOperationId(self.carriers.operations.len());
        self.carriers.operations.push(ResourceCarrierOperation {
            id,
            site: self.site,
            frame: self.frame,
            role,
            expression: self.expression,
            edge: self.edge,
        });
        self.operation(ResourceOperationRole::Carrier { operation: id });
        id
    }
    fn frame(&mut self, role: ResourceFrameRole) -> ResourceFrameId {
        let id = ResourceFrameId(self.plan.frames.len());
        self.plan.frames.push(ResourceFrame {
            id,
            role,
            site: self.site,
            parent: Some(self.frame),
            ordinal: self.ordinal,
        });
        id
    }
    fn end_loan(&mut self, loan: ResourceCarrierLoanId) {
        if self.ended_loans.insert(loan) {
            self.carrier_operation(ResourceCarrierOperationRole::EndBorrow { loan });
        }
    }
    fn loan(
        &mut self,
        value: ResourceCarrierValue,
        ty: TypeId,
        path: Vec<ResourceCarrierProjectionPath>,
    ) -> Result<ResourceCarrierLoanId, String> {
        let source = match value {
            ResourceCarrierValue::Owned { slot } => ResourceCarrierLoanSource::Slot(slot),
            ResourceCarrierValue::Borrowed { loan } => ResourceCarrierLoanSource::Loan(loan),
            _ => {
                return Err(
                    "carrier projection cannot borrow an ordinary or single-leaf value".into(),
                );
            }
        };
        let id = ResourceCarrierLoanId(self.carriers.loans.len());
        let shape = self.shape_id(ty)?;
        self.carriers.loans.push(ResourceCarrierLoan {
            id,
            frame: self.frame,
            shape,
            source,
            path,
        });
        self.carrier_operation(ResourceCarrierOperationRole::Borrow { loan: id });
        Ok(id)
    }
    fn value_type(&self, value: ResourceCarrierValue) -> Result<TypeId, String> {
        match value {
            ResourceCarrierValue::Ordinary { ty } => Ok(ty),
            ResourceCarrierValue::Owned { slot } => self
                .carriers
                .slots
                .get(slot.index())
                .and_then(|slot| self.carriers.shape(slot.shape))
                .map(ResourceCarrierShape::ty)
                .ok_or_else(|| "carrier input lost its installed slot shape".into()),
            ResourceCarrierValue::Borrowed { loan } => self
                .carriers
                .loans
                .get(loan.index())
                .and_then(|loan| self.carriers.shape(loan.shape))
                .map(ResourceCarrierShape::ty)
                .ok_or_else(|| "carrier input lost its installed loan shape".into()),
            ResourceCarrierValue::LeafOwned { slot } => match &self
                .plan
                .slots
                .get(slot.index())
                .ok_or("carrier leaf input slot missing")?
                .storage
            {
                ResourceSlotStorage::Local { header } => Ok(header.ty),
                _ => {
                    let wanted = &self.plan.slots[slot.index()].shape;
                    self.types
                        .type_ids()
                        .find(|ty| {
                            shape(&self.program.resource_manifest, self.types, *ty)
                                .ok()
                                .flatten()
                                .as_ref()
                                == Some(wanted)
                        })
                        .ok_or_else(|| "carrier leaf input lost its exact shape type".into())
                }
            },
            ResourceCarrierValue::LeafBorrowed { .. } => {
                Err("carrier constructor cannot adopt a borrowed leaf".into())
            }
        }
    }
    fn begin_constructor(
        &mut self,
        expression: &Expression,
        constructor: ResourceCarrierConstructor,
    ) -> Result<ResourceCarrierSlotId, String> {
        let destination = self.temporary(expression.ty)?;
        self.carrier_operation(ResourceCarrierOperationRole::BeginConstructor {
            destination,
            constructor,
        });
        Ok(destination)
    }
    fn child(
        &mut self,
        destination: ResourceCarrierSlotId,
        index: usize,
        expected: TypeId,
        value: ResourceCarrierValue,
    ) -> Result<(), String> {
        if self.value_type(value)? != expected {
            return Err("carrier constructor child changes its exact checked type".into());
        }
        if matches!(
            value,
            ResourceCarrierValue::Borrowed { .. } | ResourceCarrierValue::LeafBorrowed { .. }
        ) {
            return Err("carrier owning constructor cannot adopt a resident loan".into());
        }
        let shape = self.shape_id(expected)?;
        self.carrier_operation(ResourceCarrierOperationRole::ConstructorChild {
            destination,
            child: ResourceCarrierChild {
                index,
                shape,
                value,
            },
        });
        Ok(())
    }
    fn commit(&mut self, destination: ResourceCarrierSlotId) -> ResourceCarrierValue {
        self.carrier_operation(ResourceCarrierOperationRole::CommitConstructor { destination });
        ResourceCarrierValue::Owned { slot: destination }
    }
    fn exact_order(&self, order: &[usize], count: usize) -> Result<(), String> {
        let mut seen = BTreeSet::new();
        if order.len() != count
            || order
                .iter()
                .any(|index| *index >= count || !seen.insert(*index))
        {
            return Err("carrier constructor changed its checked lexical evaluation order".into());
        }
        Ok(())
    }
    fn expression(
        &mut self,
        expression: &Expression,
        owned: bool,
    ) -> Result<ResourceCarrierValue, String> {
        let previous = self
            .expression
            .replace(std::ptr::from_ref(expression).addr());
        self.ordinal = self
            .ordinal
            .checked_add(1)
            .ok_or("carrier expression ordinal overflow")?;
        let result = self.expression_inner(expression, owned);
        self.expression = previous;
        result
    }
    // A checked branch can narrow a machine Local without changing its
    // physical storage. Only observational places may read that same owner;
    // owning Local reads still use expression_inner's exact header check.
    fn observational_place(
        &mut self,
        expression: &Expression,
    ) -> Result<ResourceCarrierValue, String> {
        let previous = self
            .expression
            .replace(std::ptr::from_ref(expression).addr());
        self.ordinal = self
            .ordinal
            .checked_add(1)
            .ok_or("carrier expression ordinal overflow")?;
        let result = match &expression.kind {
            E::Local(local) => {
                let header = self
                    .function
                    .local(*local)
                    .ok_or("carrier observation lost its exact dense local header")?;
                match (
                    self.types.resolve(header.ty),
                    self.types.resolve(expression.ty),
                ) {
                    (Type::Machine(owner), Type::MachineState { machine, .. })
                        if owner == machine =>
                    {
                        let index = usize::try_from(local.index())
                            .map_err(|_| "carrier local index is not representable")?;
                        let value = self.carrier_locals.get(index).copied().flatten().ok_or(
                            "carrier observation has no dedicated physical slot or formal loan",
                        )?;
                        if !matches!(
                            value,
                            ResourceCarrierValue::Owned { .. }
                                | ResourceCarrierValue::Borrowed { .. }
                        ) || self.value_type(value)? != header.ty
                        {
                            Err(
                                "carrier observation changed its physical local custody type"
                                    .into(),
                            )
                        } else {
                            Ok(value)
                        }
                    }
                    _ => self.expression_inner(expression, false),
                }
            }
            E::View(inner) => {
                if expression.ty != inner.ty {
                    Err("carrier observational View changed its exact checked type".into())
                } else {
                    self.observational_place(inner)
                }
            }
            _ => self.expression_inner(expression, false),
        };
        self.expression = previous;
        result
    }
    fn expression_inner(
        &mut self,
        expression: &Expression,
        owned: bool,
    ) -> Result<ResourceCarrierValue, String> {
        match &expression.kind {
            E::Local(local) => {
                let header = self.function.local(*local).ok_or("carrier read lost its exact dense local header")?;
                if header.ty != expression.ty { return Err("carrier local read changed its checked type".into()); }
                let index = usize::try_from(local.index()).map_err(|_| "carrier local index is not representable")?;
                if let Some(value) = self.carrier_locals[index] {
                    if owned && matches!(value, ResourceCarrierValue::Borrowed { .. }) { return Err("carrier owning read cannot consume an incoming or resident view".into()); }
                    return Ok(value);
                }
                if let Some(value) = self.leaf_locals[index] {
                    if owned && matches!(value, ResourceCarrierValue::LeafBorrowed { .. }) { return Err("carrier leaf adapter cannot adopt a borrowed view".into()); }
                    return Ok(value);
                }
                if resource_type_pending(self.types, expression.ty) && !matches!(self.types.resolve(expression.ty), Type::Function { .. }) {
                    return Err("carrier read has no dedicated slot or formal loan".into());
                }
                Ok(ResourceCarrierValue::Ordinary { ty: expression.ty })
            }
            E::View(inner) => {
                if owned && resource_type_pending(self.types, expression.ty) { return Err("carrier View cannot be adopted by owning storage".into()); }
                self.expression(inner, false)
            }
            E::Call { function, args, evaluation_order, ownership } => self.call(expression, *function, args, evaluation_order, ownership),
            E::InterfaceCoerce { value, adapters } if carrier_type(self.types, expression.ty) && adapters.is_empty() => {
                if !self.compatible(value.ty, expression.ty) { return Err("carrier coercion changes its exact checked qualification".into()); }
                let input = self.expression(value, owned)?;
                if value.ty == expression.ty { return Ok(input); }
                let ResourceCarrierValue::Owned { slot: source } = input else { return Err("pending Resource carrier: borrowed qualification needs its exact readonly graph projection".into()); };
                let destination = self.temporary(expression.ty)?;
                self.transfer(source, destination)?;
                Ok(ResourceCarrierValue::Owned { slot: destination })
            }
            E::Field { base, owner_type, field } if carrier_type(self.types, base.ty) => self.field(expression, base, *owner_type, *field, owned),
            E::StateIs { value, state } if carrier_type(self.types, value.ty) => {
                let input = self.observational_place(value)?;
                let physical_type = self.value_type(input)?;
                let source = self.loan(input, physical_type, Vec::new())?;
                let state = usize::try_from(state.index()).map_err(|_| "carrier state selector is not representable")?;
                self.carrier_operation(ResourceCarrierOperationRole::Observe { source, observation: ResourceCarrierObservation::State { state }, target: None });
                self.end_loan(source);
                Ok(ResourceCarrierValue::Ordinary { ty: expression.ty })
            }
            E::Intrinsic { intrinsic, args, evaluation_order, .. } => {
                self.exact_order(evaluation_order, args.len())?;
                if matches!(intrinsic, hir::IntrinsicId::ListLength | hir::IntrinsicId::MapLength) && args.len() == 1 && carrier_type(self.types, args[0].ty) {
                    let input = self.expression(&args[0], false)?;
                    let source = self.loan(input, args[0].ty, Vec::new())?;
                    self.carrier_operation(ResourceCarrierOperationRole::Observe { source, observation: ResourceCarrierObservation::Length, target: None });
                    self.end_loan(source);
                    return Ok(ResourceCarrierValue::Ordinary { ty: expression.ty });
                }
                if args.iter().any(|arg| resource_type_pending(self.types, arg.ty)) || carrier_type(self.types, expression.ty) {
                    return Err("pending Resource carrier: this intrinsic requires its dedicated custody operation".into());
                }
                for index in evaluation_order { self.expression(&args[*index], true)?; }
                Ok(ResourceCarrierValue::Ordinary { ty: expression.ty })
            }
            E::ListConstruct { elements } if carrier_type(self.types, expression.ty) => {
                let Type::List(element) = self.types.resolve(expression.ty) else { return Err("carrier list constructor lost its exact list type".into()); };
                let element = *element;
                let destination = self.begin_constructor(expression, ResourceCarrierConstructor::List)?;
                for (index, child) in elements.iter().enumerate() {
                    if child.ty != element { return Err("carrier list child changes its exact element type".into()); }
                    let value = self.expression(child, true)?;
                    self.child(destination, index, element, value)?;
                }
                Ok(self.commit(destination))
            }
            E::MapConstruct { entries } if carrier_type(self.types, expression.ty) => {
                let Type::Map(key, value) = self.types.resolve(expression.ty) else { return Err("carrier map constructor lost its exact map type".into()); };
                let (key, value) = (*key, *value);
                if resource_type_pending(self.types, key) { return Err("carrier map key must preserve the checked primitive key contract".into()); }
                let destination = self.begin_constructor(expression, ResourceCarrierConstructor::Map)?;
                for (index, entry) in entries.iter().enumerate() {
                    if entry.key.ty != key || entry.value.ty != value { return Err("carrier map entry changes its exact key/value types".into()); }
                    let key_value = self.expression(&entry.key, true)?;
                    self.child(destination, index.checked_mul(2).ok_or("carrier map ordinal overflow")?, key, key_value)?;
                    let payload = self.expression(&entry.value, true)?;
                    self.child(destination, index.checked_mul(2).and_then(|n| n.checked_add(1)).ok_or("carrier map ordinal overflow")?, value, payload)?;
                }
                Ok(self.commit(destination))
            }
            E::StructConstruct { struct_type, fields, evaluation_order, .. } if carrier_type(self.types, expression.ty) => {
                if *struct_type != expression.ty { return Err("carrier record changed its exact nominal type".into()); }
                let Type::Struct(owner) = self.types.resolve(*struct_type) else { return Err("carrier record lost its nominal definition".into()); };
                let definition = self.types.resolve_struct(*owner);
                let expected = definition.fields.iter().map(|(_, ty)| *ty).collect::<Vec<_>>();
                if expected.len() != fields.len() { return Err("carrier record changed its declared field count".into()); }
                self.exact_order(evaluation_order, fields.len())?;
                let destination = self.begin_constructor(expression, ResourceCarrierConstructor::Struct)?;
                for index in evaluation_order {
                    let value = self.expression(&fields[*index], true)?;
                    self.child(destination, *index, expected[*index], value)?;
                }
                Ok(self.commit(destination))
            }
            E::EnumConstruct { enum_type, variant, payloads, evaluation_order } if carrier_type(self.types, expression.ty) => {
                if *enum_type != expression.ty { return Err("carrier enum changed its exact nominal owner".into()); }
                let Type::Enum(owner) = self.types.resolve(*enum_type) else { return Err("carrier enum lost its exact definition".into()); };
                let expected = self.types.resolve_enum(*owner).variants.get(usize::try_from(variant.index()).map_err(|_| "carrier variant index is not representable")?)
                    .ok_or("carrier enum selected a foreign variant")?.fields.iter().map(|(_, ty)| *ty).collect::<Vec<_>>();
                if expected.len() != payloads.len() { return Err("carrier enum changed its selected payload count".into()); }
                self.exact_order(evaluation_order, payloads.len())?;
                let destination = self.begin_constructor(expression, ResourceCarrierConstructor::Enum { variant: *variant })?;
                for index in evaluation_order {
                    let value = self.expression(&payloads[*index], true)?;
                    self.child(destination, *index, expected[*index], value)?;
                }
                Ok(self.commit(destination))
            }
            E::MachineConstruct { state_type, state, payloads } if carrier_type(self.types, expression.ty) => self.machine(expression, *state_type, *state, payloads),
            E::OptionalNone if !carrier_type(self.types, expression.ty) && shape(&self.program.resource_manifest, self.types, expression.ty)?.is_some() => {
                let destination = self.leaf_slot(expression.ty, ResourceSlotStorage::Expression { site: self.site, ordinal: self.ordinal }, self.frame)?;
                self.operation(ResourceOperationRole::CreateAbsentSum { destination });
                Ok(ResourceCarrierValue::LeafOwned { slot: destination })
            }
            E::OptionalNone if carrier_type(self.types, expression.ty) => {
                let destination = self.begin_constructor(expression, ResourceCarrierConstructor::OptionalNone)?;
                Ok(self.commit(destination))
            }
            E::OptionalSome(inner) | E::ResultOk(inner) | E::ResultFail(inner) if carrier_type(self.types, expression.ty) => {
                let (constructor, expected) = match (&expression.kind, self.types.resolve(expression.ty)) {
                    (E::OptionalSome(_), Type::Optional(payload)) => (ResourceCarrierConstructor::OptionalSome, *payload),
                    (E::ResultOk(_), Type::Result(ok, _)) => (ResourceCarrierConstructor::ResultOk, *ok),
                    (E::ResultFail(_), Type::Result(_, fail)) => (ResourceCarrierConstructor::ResultFail, *fail),
                    _ => return Err("carrier sum changed its exact selected arm type".into()),
                };
                let destination = self.begin_constructor(expression, constructor)?;
                let value = self.expression(inner, true)?;
                self.child(destination, 0, expected, value)?;
                Ok(self.commit(destination))
            }
            E::RefinementValidated(_) if carrier_type(self.types, expression.ty) => Err("pending Resource carrier: occupied or refined construction needs its original checked predicate transition".into()),
            E::ResourceInvoke { .. } | E::IndirectCall { .. } if resource_type_pending(self.types, expression.ty) => Err("pending Resource carrier: live leaf producers remain under their dedicated custody plan".into()),
            E::Clone(_) if resource_type_pending(self.types, expression.ty) => Err("carrier physical copies cannot create an owning root".into()),
            _ if carrier_type(self.types, expression.ty) => Err("pending Resource carrier: this expression needs its exact constructor or projection transport".into()),
            _ => {
                let mut has_resource_child = false;
                walk::expression(expression, &mut |child| {
                    if !std::ptr::eq(child, expression) && resource_type_pending(self.types, child.ty) && !matches!(self.types.resolve(child.ty), Type::Function { .. }) {
                        has_resource_child = true;
                    }
                });
                if has_resource_child {
                    // Common ordinary bool/scalar operators contain an exact typed
                    // observation. Recurse through their direct operands only.
                    match &expression.kind {
                        E::Binary { left, right, .. } => { self.expression(left, false)?; self.expression(right, false)?; }
                        E::Unary { value, .. } => { self.expression(value, false)?; }
                        _ => return Err("pending Resource carrier: nested ordinary observation needs its dedicated operation".into()),
                    }
                }
                Ok(ResourceCarrierValue::Ordinary { ty: expression.ty })
            }
        }
    }
    fn machine(
        &mut self,
        expression: &Expression,
        state_type: TypeId,
        state: hir::StateId,
        payloads: &[Expression],
    ) -> Result<ResourceCarrierValue, String> {
        let Type::MachineState {
            machine,
            state: selected,
        } = self.types.resolve(state_type)
        else {
            return Err("carrier machine constructor lost its exact checked state type".into());
        };
        if selected.index() != state.index() {
            return Err("carrier machine constructor changed its checked state selector".into());
        }
        match self.types.resolve(expression.ty) {
            Type::Machine(owner) if owner == machine => {}
            Type::MachineState {
                machine: owner,
                state: actual,
            } if owner == machine && actual == selected => {}
            _ => {
                return Err(
                    "carrier machine constructor changed its checked owner or qualification".into(),
                );
            }
        }
        let fields = self
            .types
            .resolve_machine(*machine)
            .state(*selected)
            .ok_or("carrier machine lost its selected state definition")?
            .fields
            .iter()
            .map(|(_, ty)| *ty)
            .collect::<Vec<_>>();
        if fields.len() != payloads.len() {
            return Err("carrier machine changed its selected payload count".into());
        }
        let state = usize::try_from(state.index())
            .map_err(|_| "carrier state selector is not representable")?;
        let destination =
            self.begin_constructor(expression, ResourceCarrierConstructor::Machine { state })?;
        for (index, (expected, child)) in fields.into_iter().zip(payloads).enumerate() {
            let value = self.expression(child, true)?;
            self.child(destination, index, expected, value)?;
        }
        Ok(self.commit(destination))
    }
    fn field(
        &mut self,
        expression: &Expression,
        base: &Expression,
        owner_type: TypeId,
        field: FieldId,
        owned: bool,
    ) -> Result<ResourceCarrierValue, String> {
        let expected = match self.types.resolve(owner_type) {
            Type::Struct(owner) => self
                .types
                .resolve_struct(*owner)
                .fields
                .get(
                    usize::try_from(field.index())
                        .map_err(|_| "carrier field selector is not representable")?,
                )
                .map(|(_, ty)| *ty),
            Type::MachineState { machine, state } => self
                .types
                .resolve_machine(*machine)
                .state(*state)
                .and_then(|state| state.fields.get(usize::try_from(field.index()).ok()?))
                .map(|(_, ty)| *ty),
            _ => None,
        }
        .ok_or("carrier field lost its exact checked nominal owner/ordinal")?;
        if expected != expression.ty {
            return Err("carrier field changes its declared exact payload type".into());
        }
        let input = match self.types.resolve(owner_type) {
            Type::MachineState { .. } => self.observational_place(base)?,
            _ => self.expression(base, false)?,
        };
        let physical_type = self.value_type(input)?;
        let source = self.loan(input, physical_type, Vec::new())?;
        let path = match self.types.resolve(owner_type) {
            Type::Struct(_) => ResourceCarrierProjectionPath::Field {
                owner: owner_type,
                field,
            },
            Type::MachineState { state, .. } => ResourceCarrierProjectionPath::State {
                owner: owner_type,
                state: usize::try_from(state.index())
                    .map_err(|_| "carrier state selector is not representable")?,
                field: usize::try_from(field.index())
                    .map_err(|_| "carrier field selector is not representable")?,
            },
            _ => return Err("carrier field lost its exact checked projection family".into()),
        };
        let result = self.project(expression.ty, source, vec![path], owned, None)?;
        if owned {
            self.end_loan(source);
        }
        Ok(result)
    }
    fn project(
        &mut self,
        ty: TypeId,
        source: ResourceCarrierLoanId,
        path: Vec<ResourceCarrierProjectionPath>,
        owned: bool,
        target: Option<LocalId>,
    ) -> Result<ResourceCarrierValue, String> {
        if carrier_type(self.types, ty) {
            if owned {
                let destination = match target {
                    Some(local) => match self.carrier_locals[usize::try_from(local.index())
                        .map_err(|_| "carrier binder index is not representable")?]
                    {
                        Some(ResourceCarrierValue::Owned { slot }) => slot,
                        _ => {
                            return Err(
                                "carrier extracted binder lacks its exact destination slot".into(),
                            );
                        }
                    },
                    None => self.temporary(ty)?,
                };
                self.end_loan(source);
                self.carrier_operation(ResourceCarrierOperationRole::Extract {
                    source,
                    path,
                    destination,
                });
                Ok(ResourceCarrierValue::Owned { slot: destination })
            } else {
                let destination =
                    self.loan(ResourceCarrierValue::Borrowed { loan: source }, ty, path)?;
                // The single Borrow row already authenticates the exact parent
                // and path; no second Project row may mint the same loan again.
                Ok(ResourceCarrierValue::Borrowed { loan: destination })
            }
        } else if let Some(shape) = shape(&self.program.resource_manifest, self.types, ty)? {
            if !matches!(
                shape,
                ResourceShape::Optional { .. } | ResourceShape::Result { .. }
            ) {
                return Err("pending Resource carrier: projected live Resource leaves need their exact child custody transport".into());
            }
            let destination = match target {
                Some(local) if owned => match self.leaf_locals[usize::try_from(local.index())
                    .map_err(|_| "leaf binder index is not representable")?]
                {
                    Some(ResourceCarrierValue::LeafOwned { slot }) => slot,
                    _ => {
                        return Err(
                            "carrier leaf extraction lacks its dedicated destination slot".into(),
                        );
                    }
                },
                _ => self.leaf_slot(
                    ty,
                    ResourceSlotStorage::Expression {
                        site: self.site,
                        ordinal: self.ordinal,
                    },
                    self.frame,
                )?,
            };
            let loan = if owned {
                None
            } else {
                Some(ResourceLoanId(self.plan.loans.len()))
            };
            if owned {
                self.end_loan(source);
            }
            let operation = self.carrier_operation(ResourceCarrierOperationRole::AdaptSum {
                source,
                path,
                destination,
                loan,
                lease_frame: self.frame,
            });
            if let Some(id) = loan {
                self.plan.loans.push(ResourceLoan {
                    id,
                    frame: self.frame,
                    shape,
                    source: ResourceLoanSource::CarrierSumProjection { operation },
                    parameter: None,
                });
                self.operation(ResourceOperationRole::BorrowSum { loan: id });
                Ok(ResourceCarrierValue::LeafBorrowed { loan: id })
            } else {
                Ok(ResourceCarrierValue::LeafOwned { slot: destination })
            }
        } else {
            if owned && let Some(target) = target {
                self.end_loan(source);
                self.carrier_operation(ResourceCarrierOperationRole::ExtractOrdinary {
                    source,
                    path,
                    target,
                    ty,
                });
            } else {
                self.carrier_operation(ResourceCarrierOperationRole::Observe {
                    source,
                    observation: ResourceCarrierObservation::Ordinary { path, ty },
                    target,
                });
            }
            self.end_loan(source);
            Ok(ResourceCarrierValue::Ordinary { ty })
        }
    }
    fn call(
        &mut self,
        expression: &Expression,
        function: FunctionId,
        args: &[Expression],
        order: &[usize],
        ownership: &hir::CallOwnership,
    ) -> Result<ResourceCarrierValue, String> {
        let callee = self
            .program
            .functions
            .get(
                usize::try_from(function.index())
                    .map_err(|_| "carrier callee index is not representable")?,
            )
            .filter(|callee| callee.id == function)
            .ok_or("carrier Source call lost its exact callee")?;
        self.exact_order(order, args.len())?;
        let relevant = has_execution_records(callee, self.types)
            || args
                .iter()
                .any(|arg| resource_type_pending(self.types, arg.ty))
            || resource_type_pending(self.types, expression.ty);
        if !relevant {
            for index in order {
                self.expression(&args[*index], true)?;
            }
            return Ok(ResourceCarrierValue::Ordinary { ty: expression.ty });
        }
        let hir::CallOwnership::Source(source) = ownership else {
            return Err("carrier call cannot use Generated authority".into());
        };
        if source.arguments.len() != args.len() || callee.params.len() != args.len() {
            return Err("carrier call changed its original complete Source tuple".into());
        }
        let parent = self.frame;
        let call_ordinal = self.ordinal;
        let frame = self.frame(ResourceFrameRole::Operation);
        self.frame = frame;
        let loan_start = self.carriers.loans.len();
        let leaf_loan_start = self.plan.loans.len();
        let mut operands = Vec::new();
        for parameter in order {
            let fact = source
                .arguments
                .iter()
                .find(|fact| fact.parameter_index == *parameter)
                .ok_or("carrier actual lost its original Source formal tuple")?;
            let arg = &args[*parameter];
            if !resource_type_pending(self.types, arg.ty)
                || matches!(self.types.resolve(arg.ty), Type::Function { .. })
            {
                self.expression(arg, true)?;
                operands.push(ResourceCallOperand::Ordinary {
                    parameter: *parameter,
                    ty: arg.ty,
                });
                continue;
            }
            if fact.staging != hir::ArgumentStaging::Original
                || matches!(fact.effect, Effect::Copy | Effect::ObserveData)
            {
                return Err(
                    "carrier actual cannot use ordinary staging or copying authority".into(),
                );
            }
            let retaining = fact.effect == Effect::RetainBorrow;
            if retaining
                && (fact.syntax != Syntax::WrittenView || fact.callee_access != Access::View)
            {
                return Err(
                    "carrier retained actual changed its written View/formal association".into(),
                );
            }
            if !retaining && fact.syntax != Syntax::Bare {
                return Err("carrier owned actual changed its checked bare spelling".into());
            }
            let input = if !retaining && fact.physical_access == Access::View {
                if let E::View(inner) = &arg.kind {
                    inner.as_ref()
                } else {
                    arg
                }
            } else {
                arg
            };
            let value = self.expression(input, !retaining)?;
            match value {
                ResourceCarrierValue::Owned { .. }
                    if retaining && fact.callee_access == Access::View =>
                {
                    let loan = self.loan(value, arg.ty, Vec::new())?;
                    operands.push(ResourceCallOperand::CarrierBorrowed {
                        parameter: *parameter,
                        loan,
                    });
                }
                ResourceCarrierValue::Owned { slot: source } => {
                    let slot = self.slot(
                        arg.ty,
                        ResourceSlotStorage::Argument {
                            site: self.site,
                            call_ordinal,
                            parameter: *parameter,
                        },
                        frame,
                    )?;
                    self.transfer(source, slot)?;
                    if fact.callee_access == Access::Owned {
                        operands.push(ResourceCallOperand::CarrierOwned {
                            parameter: *parameter,
                            slot,
                        });
                    } else {
                        let loan =
                            self.loan(ResourceCarrierValue::Owned { slot }, arg.ty, Vec::new())?;
                        operands.push(ResourceCallOperand::CarrierBorrowed {
                            parameter: *parameter,
                            loan,
                        });
                    }
                }
                ResourceCarrierValue::Borrowed { .. } if fact.callee_access == Access::View => {
                    let loan = self.loan(value, arg.ty, Vec::new())?;
                    operands.push(ResourceCallOperand::CarrierBorrowed {
                        parameter: *parameter,
                        loan,
                    });
                }
                ResourceCarrierValue::LeafOwned { slot: source }
                    if fact.callee_access == Access::Owned =>
                {
                    let slot = self.leaf_slot(
                        arg.ty,
                        ResourceSlotStorage::Argument {
                            site: self.site,
                            call_ordinal,
                            parameter: *parameter,
                        },
                        frame,
                    )?;
                    self.operation(ResourceOperationRole::Transfer {
                        source,
                        destination: slot,
                    });
                    operands.push(ResourceCallOperand::Owned {
                        parameter: *parameter,
                        slot,
                    });
                }
                ResourceCarrierValue::LeafOwned { slot: source }
                    if fact.callee_access == Access::View =>
                {
                    let slot = if retaining {
                        source
                    } else {
                        let destination = self.leaf_slot(
                            arg.ty,
                            ResourceSlotStorage::Argument {
                                site: self.site,
                                call_ordinal,
                                parameter: *parameter,
                            },
                            frame,
                        )?;
                        self.operation(ResourceOperationRole::Transfer {
                            source,
                            destination,
                        });
                        destination
                    };
                    let loan = ResourceLoanId(self.plan.loans.len());
                    let shape = self.plan.slots[slot.index()].shape.clone();
                    self.plan.loans.push(ResourceLoan {
                        id: loan,
                        frame,
                        shape,
                        source: ResourceLoanSource::Owner(slot),
                        parameter: Some(*parameter),
                    });
                    self.operation(ResourceOperationRole::BorrowSum { loan });
                    operands.push(ResourceCallOperand::Borrowed {
                        parameter: *parameter,
                        loan,
                    });
                }
                ResourceCarrierValue::LeafBorrowed { loan }
                    if fact.callee_access == Access::View =>
                {
                    operands.push(ResourceCallOperand::Borrowed {
                        parameter: *parameter,
                        loan,
                    })
                }
                _ => {
                    return Err(
                        "carrier actual changes its checked owning/view endpoint role".into(),
                    );
                }
            }
        }
        let (result, value) = if carrier_type(self.types, expression.ty) {
            let slot = self.slot(
                expression.ty,
                ResourceSlotStorage::Expression {
                    site: self.site,
                    ordinal: self.ordinal,
                },
                parent,
            )?;
            (
                ResourceCallResult::Carrier { slot },
                ResourceCarrierValue::Owned { slot },
            )
        } else if shape(&self.program.resource_manifest, self.types, expression.ty)?.is_some() {
            let slot = self.leaf_slot(
                expression.ty,
                ResourceSlotStorage::Expression {
                    site: self.site,
                    ordinal: self.ordinal,
                },
                parent,
            )?;
            (
                ResourceCallResult::Owned { slot },
                ResourceCarrierValue::LeafOwned { slot },
            )
        } else {
            (
                ResourceCallResult::Ordinary { ty: expression.ty },
                ResourceCarrierValue::Ordinary { ty: expression.ty },
            )
        };
        let completed_ordinal = self.ordinal;
        self.ordinal = call_ordinal;
        self.operation(ResourceOperationRole::InvokeSourceFunction {
            function,
            source: source.clone(),
            formals: source
                .arguments
                .iter()
                .map(ResourceCallFormal::original)
                .collect(),
            evaluation_order: order.to_vec(),
            operands,
            result,
        });
        self.ordinal = completed_ordinal;
        for index in (leaf_loan_start..self.plan.loans.len()).rev() {
            let loan = ResourceLoanId(index);
            if self.plan.loans[index].frame == frame {
                self.operation(ResourceOperationRole::EndSumBorrow { loan });
            }
        }
        for index in (loan_start..self.carriers.loans.len()).rev() {
            let loan = ResourceCarrierLoanId(index);
            if self.carriers.loans[index].frame == frame {
                self.end_loan(loan);
            }
        }
        self.operation(ResourceOperationRole::Complete {
            outcome: ResourceCompletion::Normal,
        });
        self.frame = parent;
        Ok(value)
    }
    fn compatible(&self, actual: TypeId, expected: TypeId) -> bool {
        actual == expected
            || matches!((self.types.resolve(actual), self.types.resolve(expected)),
            (Type::MachineState { machine, .. }, Type::Machine(owner)) if machine == owner)
    }
    fn transfer(
        &mut self,
        source: ResourceCarrierSlotId,
        destination: ResourceCarrierSlotId,
    ) -> Result<(), String> {
        let actual = self
            .carriers
            .shape(self.carriers.slots[source.index()].shape)
            .ok_or("carrier transfer source shape missing")?
            .ty;
        let expected = self
            .carriers
            .shape(self.carriers.slots[destination.index()].shape)
            .ok_or("carrier transfer destination shape missing")?
            .ty;
        if actual == expected {
            self.carrier_operation(ResourceCarrierOperationRole::Transfer {
                source,
                destination,
            });
        } else if let (Type::MachineState { machine, state }, Type::Machine(owner)) =
            (self.types.resolve(actual), self.types.resolve(expected))
        {
            if machine != owner {
                return Err("carrier qualification changes its exact machine owner".into());
            }
            let machine = self.shape_id(expected)?;
            let state = usize::try_from(state.index())
                .map_err(|_| "carrier state selector is not representable")?;
            self.carrier_operation(ResourceCarrierOperationRole::QualifyMachine {
                source,
                destination,
                machine,
                state,
            });
        } else {
            return Err(
                "carrier transfer changes exact graph node without a checked qualification".into(),
            );
        }
        Ok(())
    }
    fn store(
        &mut self,
        local: LocalId,
        input: ResourceCarrierValue,
        ty: TypeId,
    ) -> Result<(), String> {
        let header = self
            .function
            .local(local)
            .ok_or("carrier storage lost its exact current local header")?;
        if !self.compatible(ty, header.ty) {
            return Err("carrier storage changes its exact nominal/generic/state type".into());
        }
        let index = usize::try_from(local.index())
            .map_err(|_| "carrier destination index is not representable")?;
        if carrier_type(self.types, header.ty) {
            if self.function.is_view_local(local) {
                let loan = self.loan(input, header.ty, Vec::new())?;
                self.carrier_locals[index] = Some(ResourceCarrierValue::Borrowed { loan });
            } else {
                let Some(ResourceCarrierValue::Owned { slot: destination }) =
                    self.carrier_locals[index]
                else {
                    return Err("carrier destination lacks its dedicated owning slot".into());
                };
                let ResourceCarrierValue::Owned { slot: source } = input else {
                    return Err(
                        "carrier owned storage cannot adopt a borrowed or ordinary value".into(),
                    );
                };
                if source != destination {
                    self.transfer(source, destination)?;
                }
            }
        } else if shape(&self.program.resource_manifest, self.types, header.ty)?.is_some() {
            if self.function.is_view_local(local) {
                match input {
                    ResourceCarrierValue::LeafBorrowed { .. } => { self.leaf_locals[index] = Some(input); }
                    _ => return Err("pending Resource carrier: this resident leaf alias needs its lexical constructor transport".into()),
                }
            } else {
                let Some(ResourceCarrierValue::LeafOwned { slot: destination }) =
                    self.leaf_locals[index]
                else {
                    return Err("carrier leaf storage lacks its dedicated owning slot".into());
                };
                let ResourceCarrierValue::LeafOwned { slot: source } = input else {
                    return Err(
                        "carrier leaf storage cannot adopt a borrowed or aggregate value".into(),
                    );
                };
                if source != destination {
                    self.operation(ResourceOperationRole::Transfer {
                        source,
                        destination,
                    });
                }
            }
        } else if !matches!(input, ResourceCarrierValue::Ordinary { .. }) {
            return Err("carrier custody cannot enter ordinary companion storage".into());
        }
        Ok(())
    }
    fn source_loan(
        &mut self,
        source: &SequenceSource,
    ) -> Result<(ResourceCarrierLoanId, TypeId), String> {
        let ty = source
            .ty(self.function)
            .ok_or("carrier sequence lost its exact current source type")?;
        let root = usize::try_from(source.root().index())
            .map_err(|_| "carrier sequence root is not representable")?;
        let input = self
            .carrier_locals
            .get(root)
            .and_then(|value| *value)
            .ok_or("carrier sequence source lacks a dedicated carrier slot or loan")?;
        let path = match source {
            SequenceSource::Local(_) => Vec::new(),
            SequenceSource::Projected { path, .. } => path
                .iter()
                .map(|field| ResourceCarrierProjectionPath::Field {
                    owner: field.owner_type,
                    field: field.field,
                })
                .collect(),
        };
        let loan = self.loan(input, ty, path)?;
        Ok((loan, ty))
    }
    fn sequence(
        &mut self,
        consume: bool,
        source: &SequenceSource,
        index: LocalId,
        target: LocalId,
        part: SequencePart,
    ) -> Result<(), String> {
        if self.function.local(index).map(|header| header.ty) != Some(TypeInterner::INT64) {
            return Err("carrier sequence index lost its exact Int64 header".into());
        }
        let (source, ty) = self.source_loan(source)?;
        let (expected, path) = match (self.types.resolve(ty), part) {
            (Type::List(element), SequencePart::Element) => (
                *element,
                ResourceCarrierProjectionPath::Element {
                    index: ResourceCarrierIndex::Local(index),
                },
            ),
            (Type::Map(key, _), SequencePart::Key) => (
                *key,
                ResourceCarrierProjectionPath::MapKey {
                    index: ResourceCarrierIndex::Local(index),
                },
            ),
            (Type::Map(_, value), SequencePart::Value) => (
                *value,
                ResourceCarrierProjectionPath::MapValue {
                    index: ResourceCarrierIndex::Local(index),
                },
            ),
            _ => {
                return Err(
                    "carrier sequence extraction changed its checked list/map component".into(),
                );
            }
        };
        if self.function.local(target).map(|header| header.ty) != Some(expected) {
            return Err("carrier sequence extraction changes its exact current binder type".into());
        }
        self.project(expected, source, vec![path], consume, Some(target))?;
        self.end_loan(source);
        Ok(())
    }
    fn finish(&mut self, outcome: ResourceCompletion) {
        // Dynamic activation tracks acquisition order. These rows specify exact
        // eligible endpoints; cleanup does not invent local-number drop order.
        self.operation(ResourceOperationRole::Complete { outcome });
    }
    fn block(&mut self, block: &BasicBlock) -> Result<(), String> {
        self.frame = ResourceFrameId(0);
        for (index, statement) in block.statements.iter().enumerate() {
            self.site = ResourceSite {
                function: self.function.id,
                block: block.id,
                position: ResourcePosition::Statement(index),
            };
            self.ordinal = 0;
            self.expression = None;
            self.plan.execution_frames.push((self.site, self.frame));
            match &statement.kind {
                StatementKind::Let { local, value } => {
                    let input = self.expression(value, !self.function.is_view_local(*local))?;
                    self.store(*local, input, value.ty)?;
                }
                StatementKind::Assign { target, value } => {
                    if resource_type_pending(self.types, target.ty) { return Err("pending Resource carrier: replacement needs its established acquisition/destruction transition".into()); }
                    self.expression(target, false)?;
                    self.expression(value, true)?;
                }
                StatementKind::Evaluate(value) | StatementKind::HandleDefault(value) => {
                    let input = self.expression(value, true)?;
                    match input {
                        ResourceCarrierValue::Owned { slot } => { self.carrier_operation(ResourceCarrierOperationRole::Retire { source: slot }); }
                        ResourceCarrierValue::LeafOwned { slot } => self.operation(ResourceOperationRole::Drop { source: slot, occupancy: ResourceOccupancy::Empty }),
                        ResourceCarrierValue::Borrowed { .. } | ResourceCarrierValue::LeafBorrowed { .. } => return Err("carrier borrowed result cannot be discarded as an owning endpoint".into()),
                        ResourceCarrierValue::Ordinary { .. } => {}
                    }
                }
                StatementKind::SequenceLength { source, target } if source.ty(self.function).is_some_and(|ty| carrier_type(self.types, ty)) => {
                    if self.function.local(*target).map(|header| header.ty) != Some(TypeInterner::INT64) { return Err("carrier sequence length lost its exact Int64 destination".into()); }
                    let (source, _) = self.source_loan(source)?;
                    self.carrier_operation(ResourceCarrierOperationRole::Observe { source, observation: ResourceCarrierObservation::Length, target: Some(*target) });
                    self.end_loan(source);
                }
                StatementKind::SequenceGet { consume, source, index, target, part } if source.ty(self.function).is_some_and(|ty| carrier_type(self.types, ty)) => self.sequence(*consume, source, *index, *target, *part)?,
                StatementKind::SumTag { source, target } => {
                    let ty = self.function.local(*source).ok_or("carrier SumTag source header missing")?.ty;
                    if carrier_type(self.types, ty) {
                        let input = self.carrier_locals[usize::try_from(source.index()).map_err(|_| "carrier sum source index is not representable")?].ok_or("carrier SumTag source slot missing")?;
                        let loan = self.loan(input, ty, Vec::new())?;
                        self.carrier_operation(ResourceCarrierOperationRole::Observe { source: loan, observation: ResourceCarrierObservation::Tag, target: Some(*target) });
                        self.end_loan(loan);
                    } else if let Some(ResourceCarrierValue::LeafBorrowed { loan }) = self.leaf_locals[usize::try_from(source.index()).map_err(|_| "carrier leaf source index is not representable")?] {
                        self.operation(ResourceOperationRole::ObserveSumView { source: loan, target: *target });
                    }
                }
                StatementKind::SumTake { source, target, success } => {
                    let ty = self.function.local(*source).ok_or("carrier SumTake source header missing")?.ty;
                    let output = self.function.local(*target).ok_or("carrier SumTake destination header missing")?.ty;
                    if carrier_type(self.types, ty) {
                        let (expected, path) = match (self.types.resolve(ty), success) {
                            (Type::Optional(inner), true) => (*inner, ResourceCarrierProjectionPath::OptionalSome),
                            (Type::Result(ok, _), true) => (*ok, ResourceCarrierProjectionPath::ResultOk),
                            (Type::Result(_, fail), false) => (*fail, ResourceCarrierProjectionPath::ResultFail),
                            _ => return Err("carrier SumTake selected a non-payload arm".into()),
                        };
                        if output != expected { return Err("carrier SumTake changed its exact selected payload type".into()); }
                        let input = self.carrier_locals[usize::try_from(source.index()).map_err(|_| "carrier sum source index is not representable")?].ok_or("carrier SumTake source slot missing")?;
                        let loan = self.loan(input, ty, Vec::new())?;
                        self.project(output, loan, vec![path], !self.function.is_view_local(*target), Some(*target))?;
                        self.end_loan(loan);
                        if !self.function.is_view_local(*target) && let ResourceCarrierValue::Owned { slot } = input {
                            self.carrier_operation(ResourceCarrierOperationRole::Retire { source: slot });
                        }
                    } else if resource_type_pending(self.types, ty) {
                        return Err("pending Resource carrier: nested leaf Handle needs its original borrowed-sum selecting edge".into());
                    }
                }
                StatementKind::Assert { condition, message } => { self.expression(condition, false)?; if let Some(message) = message { self.expression(message, false)?; } }
                StatementKind::CheckRefinement { call, .. } => { self.expression(call, false)?; }
                StatementKind::Breakpoint { condition, bindings } => {
                    if bindings.iter().any(|local| self.function.local(*local).is_some_and(|local| resource_type_pending(self.types, local.ty))) { return Err("pending Resource carrier: debugger custody observation transport is unproved".into()); }
                    if let Some(condition) = condition { self.expression(condition, false)?; }
                }
                StatementKind::BeginCallView { local, value } => {
                    if resource_type_pending(self.types, value.ty) || self.function.local(*local).is_some_and(|header| resource_type_pending(self.types, header.ty)) { return Err("ordinary BeginCallView cannot mint a carrier loan".into()); }
                    self.expression(value, false)?;
                }
                StatementKind::ResourceCall(_) => return Err("pending Resource carrier: cross-block call requires its complete prepared carrier prefix".into()),
                StatementKind::ResourceLexicalExit(_) => return Err("pending Resource carrier: lexical carrier aliases require their exact private exit rows".into()),
                StatementKind::IterationBorrow { source, .. } if source.ty(self.function).is_some_and(|ty| carrier_type(self.types, ty)) => return Err("pending Resource carrier: borrowed iteration needs its exact bounded parent lease".into()),
                _ => {}
            }
        }
        self.site = ResourceSite {
            function: self.function.id,
            block: block.id,
            position: ResourcePosition::Terminator,
        };
        self.ordinal = 0;
        self.expression = None;
        self.plan.execution_frames.push((self.site, self.frame));
        match &block.terminator.kind {
            TerminatorKind::Return(value) => {
                if let Some(value) = value {
                    let input = self.expression(value, true)?;
                    if !self.compatible(value.ty, self.function.return_type) { return Err("carrier Return changes its declared exact nominal/state type".into()); }
                    match input {
                        ResourceCarrierValue::Owned { slot: source } => {
                            let frame = self.plan.provisional_return().ok_or("carrier Return has no exact provisional frame")?.id();
                            let destination = self.slot(self.function.return_type, ResourceSlotStorage::Return { site: self.site }, frame)?;
                            self.transfer(source, destination)?;
                            self.frame = ResourceFrameId(0);
                            self.finish(ResourceCompletion::Return);
                            self.carrier_operation(ResourceCarrierOperationRole::PublishReturn { source: destination });
                        }
                        ResourceCarrierValue::LeafOwned { slot: source } => {
                            let frame = self.plan.provisional_return().ok_or("carrier leaf Return has no exact provisional frame")?.id();
                            let destination = self.leaf_slot(self.function.return_type, ResourceSlotStorage::Return { site: self.site }, frame)?;
                            self.operation(ResourceOperationRole::Transfer { source, destination });
                            self.frame = ResourceFrameId(0);
                            self.finish(ResourceCompletion::Return);
                            self.operation(ResourceOperationRole::CompleteReturnAfterCleanup { source: destination });
                        }
                        ResourceCarrierValue::Ordinary { .. } => self.finish(ResourceCompletion::Return),
                        _ => return Err("carrier Return cannot publish a borrowed value".into()),
                    }
                } else { self.finish(ResourceCompletion::Return); }
            }
            TerminatorKind::Branch { condition, .. } => { self.expression(condition, false)?; }
            TerminatorKind::ForEach { key, value, by_view, iterable, body, exit } if carrier_type(self.types, iterable.ty) => {
                control_flow::carrier_sequence(self.function, self.types, *key, *value, *by_view, iterable)?;
                let input = self.expression(iterable, true)?;
                let destination = self.temporary(iterable.ty)?;
                self.iteration_slots.insert(block.id.index(), destination);
                self.carrier_operation(ResourceCarrierOperationRole::BeginIteration { source: input, destination, body: *body, exit: *exit });
                let mut binders = vec![*key];
                if let Some(value) = value { binders.push(*value); }
                self.carriers.iterations.push(ResourceCarrierIteration { header: block.id, body: *body, exit: *exit, source: destination, binders: binders.clone(), cursor: None });
                self.site = ResourceSite { function: self.function.id, block: *body, position: ResourcePosition::Statement(0) };
                self.edge = Some(ResourceCarrierEdge::IterationBody { source: block.id, target: *body });
                for (ordinal, binder) in binders.iter().enumerate() {
                    let ty = self.function.local(*binder).ok_or("carrier For lost its exact original binder")?.ty;
                    let source = self.loan(ResourceCarrierValue::Owned { slot: destination }, iterable.ty, Vec::new())?;
                    let index = ResourceCarrierIndex::CurrentIteration { header: block.id };
                    let path = match self.types.resolve(iterable.ty) {
                        Type::List(_) => ResourceCarrierProjectionPath::Element { index },
                        Type::Map(..) if ordinal == 0 => ResourceCarrierProjectionPath::MapKey { index },
                        Type::Map(..) => ResourceCarrierProjectionPath::MapValue { index },
                        _ => return Err("carrier For lost its checked list/map component".into()),
                    };
                    self.project(ty, source, vec![path], true, Some(*binder))?;
                    self.end_loan(source);
                }
                self.edge = None;
                self.site = ResourceSite { function: self.function.id, block: block.id, position: ResourcePosition::Terminator };
            }
            TerminatorKind::Switch { scrutinee, variants, otherwise } if carrier_type(self.types, scrutinee.ty) => {
                let input = self.expression(scrutinee, true)?;
                let tag_loan = self.loan(input, scrutinee.ty, Vec::new())?;
                self.carrier_operation(ResourceCarrierOperationRole::Observe { source: tag_loan, observation: ResourceCarrierObservation::Tag, target: None });
                self.end_loan(tag_loan);
                let Type::Enum(owner) = self.types.resolve(scrutinee.ty) else { return Err("carrier Switch lost its checked enum owner".into()); };
                let mut seen = BTreeSet::new();
                for (variant, target, bindings) in variants {
                    let index = usize::try_from(variant.index()).map_err(|_| "carrier Switch selector is not representable")?;
                    let fields = self.types.resolve_enum(*owner).variants.get(index).ok_or("carrier Switch selected a foreign variant")?.fields.iter().map(|(_, ty)| *ty).collect::<Vec<_>>();
                    if !seen.insert(index) || fields.len() != bindings.len() { return Err("carrier Switch changed its exact selected binder count".into()); }
                    self.site = ResourceSite { function: self.function.id, block: *target, position: ResourcePosition::Statement(0) };
                    self.edge = Some(ResourceCarrierEdge::SwitchVariant { source: block.id, variant: *variant, target: *target });
                    for (ordinal, (ty, binding)) in fields.into_iter().zip(bindings).enumerate() {
                        if self.function.local(*binding).map(|header| header.ty) != Some(ty) { return Err("carrier Switch changed its exact selected binder header".into()); }
                        let path = vec![ResourceCarrierProjectionPath::Variant { owner: scrutinee.ty, variant: *variant, field: ordinal }];
                        // Runtime extraction is selected by the exact Switch edge.
                        self.site = ResourceSite { function: self.function.id, block: *target, position: ResourcePosition::Statement(0) };
                        let source = self.loan(input, scrutinee.ty, Vec::new())?;
                        self.project(ty, source, path, !self.function.is_view_local(*binding), Some(*binding))?;
                        self.end_loan(source);
                    }
                    if let ResourceCarrierValue::Owned { slot } = input { self.carrier_operation(ResourceCarrierOperationRole::Retire { source: slot }); }
                    self.edge = None;
                    self.site = ResourceSite { function: self.function.id, block: block.id, position: ResourcePosition::Terminator };
                }
                if let Some(target) = otherwise {
                    self.site = ResourceSite { function: self.function.id, block: *target, position: ResourcePosition::Statement(0) };
                    self.edge = Some(ResourceCarrierEdge::SwitchOtherwise { source: block.id, target: *target });
                    if let ResourceCarrierValue::Owned { slot } = input { self.carrier_operation(ResourceCarrierOperationRole::Retire { source: slot }); }
                    self.edge = None;
                    self.site = ResourceSite { function: self.function.id, block: block.id, position: ResourcePosition::Terminator };
                }
            }
            TerminatorKind::Unreachable => self.finish(ResourceCompletion::Abort),
            TerminatorKind::Respond(_) | TerminatorKind::ReflectedTypeDispatch { .. } => return Err("pending Resource carrier: this control family needs its independent current-node proof".into()),
            _ => {}
        }
        Ok(())
    }
}

pub(super) fn analyze(
    program: &Program,
    function: &Function,
    types: &TypeInterner,
) -> Result<ResourceFunctionPlan, String> {
    let witness = function
        .resource_lowering
        .as_ref()
        .ok_or("carrier plan has no original checked Source constructor witness")?;
    witness.source.validate_types(types)?;
    witness.current(function)?;
    if function.capture_count != 0 {
        return Err("pending Resource carrier: aggregate captures need their exact environment custody transport".into());
    }
    let cfg = ControlFlowGraph::analyze(function)
        .map_err(|errors| format!("carrier CFG is malformed: {errors:?}"))?;
    let site = ResourceSite {
        function: function.id,
        block: function.entry,
        position: ResourcePosition::Terminator,
    };
    let returned = resource_type_pending(types, function.return_type)
        && !matches!(types.resolve(function.return_type), Type::Function { .. });
    let return_frame = returned.then_some(ResourceFrameId(1));
    let mut frames = vec![ResourceFrame {
        id: ResourceFrameId(0),
        role: ResourceFrameRole::Scope,
        site,
        parent: return_frame,
        ordinal: 0,
    }];
    if let Some(id) = return_frame {
        frames.push(ResourceFrame {
            id,
            role: ResourceFrameRole::Return,
            site,
            parent: None,
            ordinal: 0,
        });
    }
    let plan = ResourceFunctionPlan {
        function: function.id,
        identity: function.identity.clone(),
        parameters: function.params.clone(),
        return_type: function.return_type,
        frames,
        slots: Vec::new(),
        loans: Vec::new(),
        operations: Vec::new(),
        execution_frames: Vec::new(),
        lexical_exits: Vec::new(),
        named_callable_producers: Vec::new(),
        named_callable_values: named_callables::current_values(witness, function)?,
        descriptor_return: witness.descriptors.returned().cloned(),
        descriptor_values: returned_descriptors::current_values(witness, function)?,
        descriptor_locals: witness
            .descriptors
            .locals()
            .map(|(local, hook)| (local, hook.clone()))
            .collect(),
        carriers: None,
    };
    let mut planner = Planner {
        program,
        function,
        types,
        plan,
        carriers: ResourceCarrierFunctionPlan {
            shapes: graph::capture(&witness.source, &program.resource_manifest, types)?,
            slots: Vec::new(),
            loans: Vec::new(),
            operations: Vec::new(),
            iterations: Vec::new(),
        },
        carrier_locals: vec![None; function.locals.len()],
        leaf_locals: vec![None; function.locals.len()],
        site,
        expression: None,
        ordinal: 0,
        frame: ResourceFrameId(0),
        call_frames: BTreeMap::new(),
        iteration_slots: BTreeMap::new(),
        edge: None,
        ended_loans: BTreeSet::new(),
    };
    for local in &function.locals {
        let index = usize::try_from(local.id.index())
            .map_err(|_| "carrier local index is not representable")?;
        if carrier_type(types, local.ty) && !function.is_view_local(local.id) {
            let slot = planner.slot(
                local.ty,
                ResourceSlotStorage::Local {
                    header: local.clone(),
                },
                ResourceFrameId(0),
            )?;
            planner.carrier_locals[index] = Some(ResourceCarrierValue::Owned { slot });
        } else if !carrier_type(types, local.ty)
            && shape(&program.resource_manifest, types, local.ty)?.is_some()
            && !function.is_view_local(local.id)
        {
            let slot = planner.leaf_slot(
                local.ty,
                ResourceSlotStorage::Local {
                    header: local.clone(),
                },
                ResourceFrameId(0),
            )?;
            planner.leaf_locals[index] = Some(ResourceCarrierValue::LeafOwned { slot });
        }
    }
    for (parameter, param) in function.params.iter().enumerate() {
        if param.mode != ParamMode::View {
            continue;
        }
        let index = usize::try_from(param.local.index())
            .map_err(|_| "carrier formal index is not representable")?;
        if carrier_type(types, param.ty) {
            let id = ResourceCarrierLoanId(planner.carriers.loans.len());
            let shape = planner.shape_id(param.ty)?;
            planner.carriers.loans.push(ResourceCarrierLoan {
                id,
                frame: ResourceFrameId(0),
                shape,
                source: ResourceCarrierLoanSource::IncomingViewFormal {
                    scope: ResourceFrameId(0),
                    parameter,
                },
                path: Vec::new(),
            });
            planner.carrier_locals[index] = Some(ResourceCarrierValue::Borrowed { loan: id });
        } else if let Some(shape) = shape(&program.resource_manifest, types, param.ty)? {
            let id = ResourceLoanId(planner.plan.loans.len());
            planner.plan.loans.push(ResourceLoan {
                id,
                frame: ResourceFrameId(0),
                shape,
                source: ResourceLoanSource::IncomingViewFormal {
                    scope: ResourceFrameId(0),
                    parameter,
                },
                parameter: None,
            });
            planner.leaf_locals[index] = Some(ResourceCarrierValue::LeafBorrowed { loan: id });
        }
    }
    let mut reachable = vec![false; function.blocks.len()];
    let mut pending = vec![function.entry];
    while let Some(block) = pending.pop() {
        let index = usize::try_from(block.index())
            .map_err(|_| "carrier block index is not representable")?;
        if reachable[index] {
            continue;
        }
        reachable[index] = true;
        pending.extend_from_slice(cfg.successors(block));
    }
    for block in &function.blocks {
        if reachable[usize::try_from(block.id.index())
            .map_err(|_| "carrier block index is not representable")?]
        {
            planner.block(block)?;
        } else if walk::mir_block_has_resource(block)
            || walk::mir_block_has_custody(block, function, types)
        {
            return Err(
                "carrier custody operation is disconnected from its authenticated entry CFG".into(),
            );
        }
    }
    // Prepared sequence rows are accepted only after their complete canonical
    // current loop and original Source For tuple have been joined privately.
    let prepared = iterations::capture(function, types, &planner.carriers)?;
    planner.carriers.iterations.extend(prepared);
    let resets = planner.carriers.iterations.clone();
    for iteration in resets {
        planner.site = ResourceSite {
            function: function.id,
            block: iteration.header,
            position: ResourcePosition::Terminator,
        };
        planner.frame = ResourceFrameId(0);
        planner.expression = None;
        planner.edge = None;
        planner.carrier_operation(ResourceCarrierOperationRole::RetireIteration {
            source: iteration.source,
            binders: iteration.binders,
        });
    }
    flow::validate(function, types, &planner.plan, &planner.carriers)?;
    planner.plan.carriers = Some(planner.carriers);
    Ok(planner.plan)
}
