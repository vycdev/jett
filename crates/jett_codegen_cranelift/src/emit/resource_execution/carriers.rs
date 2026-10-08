//! Native execution of constructor-owned typed carrier operations. Ordinary
//! container leaves never receive a carrier root, loan, or Resource sum shell.
use super::*;
use jett_mir::{
    ResourceCarrierEdge as Edge, ResourceCarrierIndex as Index,
    ResourceCarrierLoanSource as LoanSource, ResourceCarrierObservation as Observation,
    ResourceCarrierOperation as Operation, ResourceCarrierOperationRole as CarrierRole,
    ResourceCarrierProjectionPath as Path, ResourceCarrierSlotId as SlotId,
    ResourceCarrierValue as CarrierValue,
};

impl<'a> ResourceEmission<'a> {
    fn carrier_plan(self) -> Result<&'a jett_mir::ResourceCarrierFunctionPlan, CodegenError> {
        self.function_plan()?
            .carriers()
            .ok_or_else(|| pending("carrier execution has no fresh private plan"))
    }
    fn carrier_at_site(self) -> Result<Vec<&'a Operation>, CodegenError> {
        Ok(self
            .carrier_plan()?
            .operations()
            .iter()
            .filter(|row| {
                row.site().block() == self.block
                    && row.site().position() == self.position
                    && row.edge().is_none()
                    && !row.is_expression_operation()
            })
            .collect())
    }
    pub(super) fn carrier_local_slot(
        self,
        local: jett_mir::LocalId,
    ) -> Result<Option<SlotId>, CodegenError> {
        let Some(plan) = self.function_plan()?.carriers() else {
            return Ok(None);
        };
        let mut slots = plan.slots().iter().filter(
            |slot| matches!(slot.storage(), Storage::Local { header } if header.id == local),
        );
        let selected = slots.next().map(|slot| slot.id());
        if slots.next().is_some() {
            return Err(pending("carrier local has ambiguous current storage"));
        }
        Ok(selected)
    }
}

impl Translator<'_, '_> {
    fn carrier_operation(
        &mut self,
        operation: &Operation,
    ) -> Result<(Value, Value, Value), CodegenError> {
        let resource = self
            .resource
            .ok_or_else(|| pending("carrier operation has no selected family"))?;
        let context = self.resource_context();
        let frame = self.resource_frame(operation.frame())?;
        let ordinal = resource
            .layout
            .carrier_operation(resource.function, operation.id())
            .ok_or_else(|| pending("carrier occurrence lost its exact installed row"))?;
        let ordinal = self
            .builder
            .ins()
            .iconst(ir::types::I32, i64::from(ordinal));
        Ok((context, frame, ordinal))
    }
    pub(super) fn carrier_owner(&mut self, slot: SlotId) -> Result<Value, CodegenError> {
        let resource = self
            .resource
            .ok_or_else(|| pending("carrier owner has no selected family"))?;
        let storage = *resource
            .carrier_owners
            .get(slot.index())
            .ok_or_else(|| pending("carrier owner lost its exact storage"))?;
        Ok(self.builder.ins().stack_load(ir::types::I64, storage, 0))
    }
    pub(super) fn carrier_store(&mut self, slot: SlotId, value: Value) -> Result<(), CodegenError> {
        let resource = self
            .resource
            .ok_or_else(|| pending("carrier store has no selected family"))?;
        let storage = *resource
            .carrier_owners
            .get(slot.index())
            .ok_or_else(|| pending("carrier store lost its exact storage"))?;
        self.builder.ins().stack_store(value, storage, 0);
        Ok(())
    }
    pub(super) fn carrier_loan(
        &mut self,
        loan: jett_mir::ResourceCarrierLoanId,
    ) -> Result<Value, CodegenError> {
        let resource = self
            .resource
            .ok_or_else(|| pending("carrier loan has no selected family"))?;
        let record = resource
            .carrier_plan()?
            .loans()
            .get(loan.index())
            .filter(|row| row.id() == loan)
            .ok_or_else(|| pending("carrier loan changed its exact current row"))?;
        if let LoanSource::IncomingViewFormal { scope, parameter } = record.source() {
            if scope != resource.function_plan()?.root_scope().id() || !record.path().is_empty() {
                return Err(pending(
                    "incoming carrier view changes its original scope/path",
                ));
            }
            let formal = resource
                .function_plan()?
                .parameters()
                .get(parameter)
                .ok_or_else(|| pending("incoming carrier view lost its exact formal"))?;
            let variable = self.variables[formal.local.index() as usize]
                .ok_or_else(|| pending("incoming carrier view has no native parameter"))?;
            return Ok(self.builder.use_var(variable));
        }
        let storage = *resource
            .carrier_loans
            .get(loan.index())
            .ok_or_else(|| pending("carrier loan lost its native storage"))?;
        Ok(self.builder.ins().stack_load(ir::types::I64, storage, 0))
    }
    fn carrier_source(&mut self, source: LoanSource) -> Result<Value, CodegenError> {
        match source {
            LoanSource::Slot(slot) => self.carrier_owner(slot),
            LoanSource::Loan(loan) => self.carrier_loan(loan),
            LoanSource::IncomingViewFormal { scope, parameter } => {
                let resource = self
                    .resource
                    .ok_or_else(|| pending("carrier formal source has no family"))?;
                if scope != resource.function_plan()?.root_scope().id() {
                    return Err(pending("carrier formal uses a foreign scope"));
                }
                let formal = resource
                    .function_plan()?
                    .parameters()
                    .get(parameter)
                    .ok_or_else(|| pending("carrier formal is absent"))?;
                let variable = self.variables[formal.local.index() as usize]
                    .ok_or_else(|| pending("carrier formal has no native variable"))?;
                Ok(self.builder.use_var(variable))
            }
        }
    }
    fn carrier_owned_root(
        &self,
        loan: jett_mir::ResourceCarrierLoanId,
    ) -> Result<(SlotId, Vec<Path>), CodegenError> {
        let resource = self
            .resource
            .ok_or_else(|| pending("carrier extraction has no family"))?;
        let plan = resource.carrier_plan()?;
        let mut visited = std::collections::BTreeSet::new();
        let mut cursor = loan;
        let mut segments = Vec::new();
        loop {
            if !visited.insert(cursor.index()) {
                return Err(pending("carrier extraction has a cyclic parent loan"));
            }
            let row = plan
                .loans()
                .get(cursor.index())
                .filter(|row| row.id() == cursor)
                .ok_or_else(|| pending("carrier extraction lost its exact parent loan"))?;
            segments.push(row.path().to_vec());
            match row.source() {
                LoanSource::Slot(slot) => {
                    let mut path = Vec::new();
                    for segment in segments.iter().rev() {
                        path.extend_from_slice(segment);
                    }
                    return Ok((slot, path));
                }
                LoanSource::Loan(parent) => cursor = parent,
                LoanSource::IncomingViewFormal { .. } => {
                    return Err(pending(
                        "owning projection cannot consume an incoming carrier view",
                    ));
                }
            }
        }
    }
    fn carrier_selection(&mut self, path: &[Path]) -> Result<Value, CodegenError> {
        let mut selected = None;
        for step in path {
            let index = match step {
                Path::Element { index } | Path::MapKey { index } | Path::MapValue { index } => {
                    *index
                }
                _ => continue,
            };
            let local = match index {
                Index::Constant(_) => continue,
                Index::Local(local) => local,
                Index::CurrentIteration { header } => {
                    let resource = self
                        .resource
                        .ok_or_else(|| pending("iteration selection has no family"))?;
                    let mut loops = resource
                        .carrier_plan()?
                        .iterations()
                        .iter()
                        .filter(|row| row.header == header);
                    let selected = loops.next().ok_or_else(|| {
                        pending("iteration selection has no original/canonical loop")
                    })?;
                    if loops.next().is_some() {
                        return Err(pending("iteration selection is ambiguous"));
                    }
                    selected.cursor.ok_or_else(|| {
                        pending("raw carrier loop has no authenticated native cursor")
                    })?
                }
            };
            if selected.replace(local).is_some() {
                return Err(pending(
                    "carrier path needs its exact multiple-selection transport",
                ));
            }
        }
        if let Some(local) = selected {
            if self.local_types[local.index() as usize].ty != TypeInterner::INT64 {
                return Err(pending("carrier selector changed its exact Int64 local"));
            }
            let variable = self.variables[local.index() as usize]
                .ok_or_else(|| pending("carrier selector has no native variable"))?;
            Ok(self.builder.use_var(variable))
        } else {
            Ok(self.builder.ins().iconst(ir::types::I64, 0))
        }
    }
    pub(super) fn carrier_transfer(
        &mut self,
        operation: &Operation,
    ) -> Result<Value, CodegenError> {
        let resource = self
            .resource
            .ok_or_else(|| pending("carrier transfer has no family"))?;
        let (source, destination, leaf) = match operation.role() {
            CarrierRole::Transfer {
                source,
                destination,
            } => (*source, *destination, Leaf::CarrierTransfer),
            CarrierRole::QualifyMachine {
                source,
                destination,
                ..
            } => (*source, *destination, Leaf::CarrierQualify),
            CarrierRole::BeginIteration {
                source: CarrierValue::Owned { slot },
                destination,
                ..
            } => (*slot, *destination, Leaf::CarrierTransfer),
            _ => return Err(pending("carrier transfer selected another role")),
        };
        let input = self.carrier_owner(source)?;
        let (context, frame, ordinal) = self.carrier_operation(operation)?;
        let bits = leaf.output(
            self.module,
            self.builder,
            &[context, frame, ordinal, input],
            resource.failure,
            8,
        )?;
        self.clear_slot(resource.carrier_owners[source.index()]);
        self.carrier_store(destination, bits)?;
        Ok(bits)
    }
    fn carrier_begin_borrow(&mut self, operation: &Operation) -> Result<Value, CodegenError> {
        let resource = self
            .resource
            .ok_or_else(|| pending("carrier borrow has no family"))?;
        let loan = match operation.role() {
            CarrierRole::Borrow { loan }
            | CarrierRole::Project {
                destination: loan, ..
            } => *loan,
            _ => return Err(pending("carrier borrow selected another role")),
        };
        let record = resource
            .carrier_plan()?
            .loans()
            .get(loan.index())
            .ok_or_else(|| pending("carrier borrow lost its exact loan"))?;
        let input = self.carrier_source(record.source())?;
        let selection = self.carrier_selection(record.path())?;
        let (context, frame, ordinal) = self.carrier_operation(operation)?;
        let bits = Leaf::CarrierBorrow.output(
            self.module,
            self.builder,
            &[context, frame, ordinal, input, selection],
            resource.failure,
            8,
        )?;
        self.builder
            .ins()
            .stack_store(bits, resource.carrier_loans[loan.index()], 0);
        Ok(bits)
    }
    fn carrier_end_borrow(&mut self, operation: &Operation) -> Result<(), CodegenError> {
        let resource = self
            .resource
            .ok_or_else(|| pending("carrier end borrow has no family"))?;
        let CarrierRole::EndBorrow { loan } = operation.role() else {
            return Err(pending("carrier end selected another role"));
        };
        let input = self.carrier_loan(*loan)?;
        let (context, frame, ordinal) = self.carrier_operation(operation)?;
        Leaf::CarrierEndBorrow.checked(
            self.module,
            self.builder,
            &[context, frame, ordinal, input],
            resource.failure,
        )?;
        self.clear_slot(resource.carrier_loans[loan.index()]);
        Ok(())
    }
    /// Execute one exact occurrence; expression child evaluation stays with the
    /// constructor emitter and publication stays after ScopeComplete.
    fn carrier_effect(
        &mut self,
        operation: &Operation,
        span: Span,
    ) -> Result<Option<LoweredValue>, CodegenError> {
        let resource = self
            .resource
            .ok_or_else(|| pending("carrier effect has no family"))?;
        match operation.role() {
            CarrierRole::Transfer { .. }
            | CarrierRole::QualifyMachine { .. }
            | CarrierRole::BeginIteration { .. } => self
                .carrier_transfer(operation)
                .map(|bits| Some(LoweredValue::Scalar(bits))),
            CarrierRole::Borrow { .. } | CarrierRole::Project { .. } => self
                .carrier_begin_borrow(operation)
                .map(|bits| Some(LoweredValue::Scalar(bits))),
            CarrierRole::EndBorrow { .. } => {
                self.carrier_end_borrow(operation)?;
                Ok(None)
            }
            CarrierRole::Observe {
                source,
                observation,
                target,
            } => {
                let input = self.carrier_loan(*source)?;
                let selected = match observation {
                    Observation::Ordinary { path, .. } => path.as_slice(),
                    _ => &[],
                };
                let selection = self.carrier_selection(selected)?;
                let (context, frame, ordinal) = self.carrier_operation(operation)?;
                let bits = Leaf::CarrierObserve.output(
                    self.module,
                    self.builder,
                    &[context, frame, ordinal, input, selection],
                    resource.failure,
                    8,
                )?;
                let result = match observation {
                    Observation::Length | Observation::Tag => LoweredValue::Scalar(bits),
                    Observation::State { state } => {
                        let selected = i64::try_from(*state)
                            .map_err(|_| pending("checked carrier state is outside tag width"))?;
                        LoweredValue::Scalar(self.builder.ins().icmp_imm(
                            IntCC::Equal,
                            bits,
                            selected,
                        ))
                    }
                    Observation::Ordinary { ty, .. } => self.unpack_payload(bits, *ty, span)?,
                };
                if let Some(target) = target {
                    let bits = self.scalar(result, span)?;
                    let result = if self.local_types[target.index() as usize].ty
                        == TypeInterner::BOOL
                        && self.builder.func.dfg.value_type(bits) != ir::types::I8
                    {
                        LoweredValue::Scalar(self.builder.ins().ireduce(ir::types::I8, bits))
                    } else {
                        result
                    };
                    self.define_local(*target, result, span)?;
                }
                Ok(Some(result))
            }
            CarrierRole::Extract {
                source,
                path,
                destination,
            } => {
                let (root, mut full) = self.carrier_owned_root(*source)?;
                full.extend_from_slice(path);
                let input = self.carrier_owner(root)?;
                let selection = self.carrier_selection(&full)?;
                let (context, frame, ordinal) = self.carrier_operation(operation)?;
                let bits = Leaf::CarrierExtract.output(
                    self.module,
                    self.builder,
                    &[context, frame, ordinal, input, selection],
                    resource.failure,
                    8,
                )?;
                self.carrier_store(*destination, bits)?;
                if let Storage::Local { header } =
                    resource.carrier_plan()?.slots()[destination.index()].storage()
                {
                    self.define_local(header.id, LoweredValue::Scalar(bits), span)?;
                }
                Ok(Some(LoweredValue::Scalar(bits)))
            }
            CarrierRole::ExtractOrdinary {
                source,
                path,
                target,
                ty,
            } => {
                let (root, mut full) = self.carrier_owned_root(*source)?;
                full.extend_from_slice(path);
                let input = self.carrier_owner(root)?;
                let selection = self.carrier_selection(&full)?;
                let (context, frame, ordinal) = self.carrier_operation(operation)?;
                let bits = Leaf::CarrierExtract.output(
                    self.module,
                    self.builder,
                    &[context, frame, ordinal, input, selection],
                    resource.failure,
                    8,
                )?;
                let value = self.unpack_payload(bits, *ty, span)?;
                self.define_local(*target, value, span)?;
                Ok(Some(value))
            }
            CarrierRole::AdaptSum {
                source,
                path,
                destination,
                loan,
                ..
            } => {
                let (input, full) = if loan.is_some() {
                    (self.carrier_loan(*source)?, path.clone())
                } else {
                    let (root, mut full) = self.carrier_owned_root(*source)?;
                    full.extend_from_slice(path);
                    (self.carrier_owner(root)?, full)
                };
                let selection = self.carrier_selection(&full)?;
                let (context, frame, ordinal) = self.carrier_operation(operation)?;
                let bits = Leaf::CarrierAdaptSum.output(
                    self.module,
                    self.builder,
                    &[context, frame, ordinal, input, selection],
                    resource.failure,
                    8,
                )?;
                if let Some(loan) = loan {
                    // The adapter returns the exact SumLoan. Its parent-bound
                    // shell is owned solely by runtime, never an ordinary owner.
                    self.builder
                        .ins()
                        .stack_store(bits, resource.loans[loan.index()], 0);
                } else {
                    self.resource_store(*destination, bits)?;
                    if let Storage::Local { header } =
                        resource.function_plan()?.owner_slots()[destination.index()].storage()
                    {
                        self.define_local(header.id, LoweredValue::Scalar(bits), span)?;
                    }
                }
                Ok(Some(LoweredValue::Scalar(bits)))
            }
            CarrierRole::Retire { source } => {
                let input = self.carrier_owner(*source)?;
                let (context, frame, ordinal) = self.carrier_operation(operation)?;
                Leaf::CarrierRetire.checked(
                    self.module,
                    self.builder,
                    &[context, frame, ordinal, input],
                    resource.failure,
                )?;
                self.clear_slot(resource.carrier_owners[source.index()]);
                Ok(None)
            }
            CarrierRole::RetireIteration { source, binders } => {
                let input = self.carrier_owner(*source)?;
                let (context, frame, ordinal) = self.carrier_operation(operation)?;
                Leaf::CarrierResetIteration.checked(
                    self.module,
                    self.builder,
                    &[context, frame, ordinal, input],
                    resource.failure,
                )?;
                for binder in binders {
                    if let Some(slot) = resource.carrier_local_slot(*binder)? {
                        self.clear_slot(resource.carrier_owners[slot.index()]);
                    }
                    if let Some(slot) = resource.local_slot(*binder)? {
                        self.clear_slot(resource.owners[slot.index()]);
                    }
                }
                Ok(None)
            }
            CarrierRole::BeginConstructor { .. }
            | CarrierRole::ConstructorChild { .. }
            | CarrierRole::CommitConstructor { .. }
            | CarrierRole::PublishReturn { .. } => Err(pending(
                "carrier effect must use its exact constructor/publication phase",
            )),
        }
    }
    fn carrier_construct(
        &mut self,
        expression: &Expression,
        operations: &[&Operation],
    ) -> Result<LoweredValue, CodegenError> {
        let resource = self
            .resource
            .ok_or_else(|| pending("carrier constructor has no family"))?;
        let mut output = None;
        let mut destination = None;
        let mut arrival = 0usize;
        for operation in operations {
            match operation.role() {
                CarrierRole::BeginConstructor {
                    destination: slot, ..
                } => {
                    if destination.replace(*slot).is_some() {
                        return Err(pending(
                            "carrier expression contains multiple outer constructors",
                        ));
                    }
                    let (context, frame, ordinal) = self.carrier_operation(operation)?;
                    let builder = Leaf::CarrierConstructBegin.output(
                        self.module,
                        self.builder,
                        &[context, frame, ordinal],
                        resource.failure,
                        8,
                    )?;
                    self.builder.ins().stack_store(
                        builder,
                        resource.carrier_builders[slot.index()],
                        0,
                    );
                }
                CarrierRole::ConstructorChild {
                    destination: slot,
                    child,
                } => {
                    if destination != Some(*slot) {
                        return Err(pending(
                            "carrier child changed its exact constructor destination",
                        ));
                    }
                    let actual = constructor_child(expression, child.index)?;
                    let evaluated = self.expression(actual)?;
                    let value = match child.value {
                        CarrierValue::Ordinary { ty } => {
                            if ty != actual.ty {
                                return Err(pending(
                                    "carrier ordinary child changed its exact source type",
                                ));
                            }
                            if is_copy_owned(self.types, ty) || is_linear(self.types, ty) {
                                self.own_copy_value(evaluated, ty, actual.span)?
                            } else {
                                evaluated
                            }
                        }
                        _ => evaluated,
                    };
                    let bits = match child.value {
                        CarrierValue::Owned { slot } => self.carrier_owner(slot)?,
                        CarrierValue::LeafOwned { slot } => self.resource_owner(slot)?,
                        CarrierValue::Ordinary { .. } => self.payload_bits(value).0,
                        CarrierValue::Borrowed { .. } | CarrierValue::LeafBorrowed { .. } => {
                            return Err(pending(
                                "carrier constructor cannot adopt a borrowed child",
                            ));
                        }
                    };
                    let builder = self.builder.ins().stack_load(
                        ir::types::I64,
                        resource.carrier_builders[slot.index()],
                        0,
                    );
                    let context = self.resource_context();
                    let index = u32::try_from(arrival)
                        .map_err(|_| pending("carrier child arrival is outside wire width"))?;
                    arrival = arrival
                        .checked_add(1)
                        .ok_or_else(|| pending("carrier child arrival overflow"))?;
                    let index = self.builder.ins().iconst(ir::types::I32, i64::from(index));
                    Leaf::CarrierChild.checked(
                        self.module,
                        self.builder,
                        &[context, builder, index, bits],
                        resource.failure,
                    )?;
                    match child.value {
                        CarrierValue::Owned { slot } => {
                            self.clear_slot(resource.carrier_owners[slot.index()])
                        }
                        CarrierValue::LeafOwned { slot } => {
                            self.clear_slot(resource.owners[slot.index()])
                        }
                        CarrierValue::Ordinary { .. } => {
                            if let LoweredValue::Owned(_, slot) = value {
                                self.clear_slot(slot);
                            }
                        }
                        CarrierValue::Borrowed { .. } | CarrierValue::LeafBorrowed { .. } => {
                            unreachable!()
                        }
                    }
                }
                CarrierRole::CommitConstructor { destination: slot } => {
                    if destination != Some(*slot) || output.is_some() {
                        return Err(pending(
                            "carrier constructor changed its exact commit phase",
                        ));
                    }
                    let builder = self.builder.ins().stack_load(
                        ir::types::I64,
                        resource.carrier_builders[slot.index()],
                        0,
                    );
                    let context = self.resource_context();
                    let bits = Leaf::CarrierCommit.output(
                        self.module,
                        self.builder,
                        &[context, builder],
                        resource.failure,
                        8,
                    )?;
                    self.clear_slot(resource.carrier_builders[slot.index()]);
                    self.carrier_store(*slot, bits)?;
                    output = Some(LoweredValue::Scalar(bits));
                }
                _ => {
                    return Err(pending(
                        "carrier constructor selected an unrelated outer expression role",
                    ));
                }
            }
        }
        output.ok_or_else(|| pending("carrier constructor omitted its exact commit"))
    }
    pub(super) fn carrier_expression(
        &mut self,
        expression: &Expression,
    ) -> Result<Option<LoweredValue>, CodegenError> {
        let Some(resource) = self.resource else {
            return Ok(None);
        };
        let Some(plan) = resource.function_plan()?.carriers() else {
            return Ok(None);
        };
        let operations = plan
            .operations_for_expression(expression)
            .collect::<Vec<_>>();
        if operations
            .iter()
            .any(|row| matches!(row.role(), CarrierRole::BeginConstructor { .. }))
        {
            return self.carrier_construct(expression, &operations).map(Some);
        }
        if !operations.is_empty() {
            match &expression.kind {
                ExpressionKind::Field { base, .. } => {
                    self.expression(base)?;
                }
                ExpressionKind::StateIs { value, .. } => {
                    self.expression(value)?;
                }
                ExpressionKind::InterfaceCoerce { value, adapters } if adapters.is_empty() => {
                    self.expression(value)?;
                }
                ExpressionKind::Intrinsic { args, .. } => {
                    for argument in args {
                        self.expression(argument)?;
                    }
                }
                _ => {}
            }
        }
        let mut value = None;
        for operation in operations {
            if let Some(result) = self.carrier_effect(operation, expression.span)? {
                value = Some(result);
            }
        }
        if value.is_some() {
            return Ok(value);
        }
        if !super::super::resource_layout::carrier_operations::is_carrier_type(
            resource.layout.plan(),
            expression.ty,
        ) {
            return Ok(None);
        }
        match &expression.kind {
            ExpressionKind::Local(local) => {
                let bits = if let Some(slot) = resource.carrier_local_slot(*local)? {
                    self.carrier_owner(slot)?
                } else {
                    let variable = self.variables[local.index() as usize]
                        .ok_or_else(|| pending("carrier view local has no current variable"))?;
                    self.builder.use_var(variable)
                };
                Ok(Some(LoweredValue::Scalar(bits)))
            }
            ExpressionKind::View(inner) => self.expression(inner).map(Some),
            _ => Err(pending(
                "carrier expression has no exact constructor/projection/current-local occurrence",
            )),
        }
    }
    pub(super) fn carrier_stage_actual(
        &mut self,
        operations: &[&jett_mir::ResourceOperation],
        frame: jett_mir::ResourceFrameId,
        parameter: usize,
    ) -> Result<(), CodegenError> {
        let Some(resource) = self.resource else {
            return Ok(());
        };
        let Some(plan) = resource.function_plan()?.carriers() else {
            return Ok(());
        };
        for wrapper in operations
            .iter()
            .copied()
            .filter(|row| row.frame() == frame)
        {
            let Role::Carrier { operation } = wrapper.role() else {
                continue;
            };
            let row = plan
                .operations()
                .get(operation.index())
                .ok_or_else(|| pending("carrier staging lost its exact occurrence"))?;
            match row.role() {
                CarrierRole::Transfer { destination, .. }
                | CarrierRole::QualifyMachine { destination, .. }
                    if matches!(plan.slots()[destination.index()].storage(), Storage::Argument { parameter: selected, .. } if *selected == parameter) =>
                {
                    self.carrier_transfer(row)?;
                }
                CarrierRole::Borrow { loan } => {
                    let selected = operations.iter().any(|wrapper| match wrapper.role() {
                        Role::InvokeSourceFunction { operands, .. } => operands.iter().any(|operand| matches!(operand, Operand::CarrierBorrowed { parameter: actual, loan: selected } if *actual == parameter && selected == loan)),
                        _ => false,
                    });
                    if selected {
                        self.carrier_begin_borrow(row)?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
    pub(super) fn carrier_finish_operation(
        &mut self,
        wrapper: &jett_mir::ResourceOperation,
    ) -> Result<(), CodegenError> {
        let resource = self
            .resource
            .ok_or_else(|| pending("carrier finish has no family"))?;
        let Role::Carrier { operation } = wrapper.role() else {
            return Ok(());
        };
        let row = resource
            .carrier_plan()?
            .operations()
            .get(operation.index())
            .ok_or_else(|| pending("carrier finish lost its exact occurrence"))?;
        if matches!(
            row.role(),
            CarrierRole::EndBorrow { .. } | CarrierRole::Retire { .. }
        ) {
            let span = resource
                .layout
                .plan()
                .program()
                .functions
                .get(resource.function.index() as usize)
                .filter(|function| function.id == resource.function)
                .ok_or_else(|| pending("carrier completion lost its original function"))?
                .span;
            self.carrier_effect(row, span)?;
        }
        Ok(())
    }
    pub(super) fn carrier_statement(
        &mut self,
        statement: &Statement,
    ) -> Result<bool, CodegenError> {
        let Some(resource) = self.resource else {
            return Ok(false);
        };
        if resource.function_plan()?.carriers().is_none() {
            return Ok(false);
        }
        let operations = resource.carrier_at_site()?;
        let mut claimed = false;
        match &statement.kind {
            StatementKind::Let { local, value }
                if super::super::resource_layout::carrier_operations::is_carrier_type(
                    resource.layout.plan(),
                    value.ty,
                ) =>
            {
                let mut evaluated = self.expression(value)?;
                for operation in &operations {
                    if let Some(value) = self.carrier_effect(operation, statement.span)? {
                        evaluated = value;
                    }
                }
                self.define_local(*local, evaluated, statement.span)?;
                return Ok(true);
            }
            StatementKind::Evaluate(value) | StatementKind::HandleDefault(value)
                if super::super::resource_layout::carrier_operations::is_carrier_type(
                    resource.layout.plan(),
                    value.ty,
                ) =>
            {
                self.expression(value)?;
                claimed = true;
            }
            StatementKind::SequenceLength { source, .. }
            | StatementKind::SequenceGet { source, .. }
                if source
                    .ty(resource
                        .layout
                        .plan()
                        .program()
                        .functions
                        .get(resource.function.index() as usize)
                        .ok_or_else(|| pending("carrier sequence lost its current function"))?)
                    .is_some_and(|ty| {
                        super::super::resource_layout::carrier_operations::is_carrier_type(
                            resource.layout.plan(),
                            ty,
                        )
                    }) =>
            {
                claimed = true;
            }
            StatementKind::SumTag { source, .. } | StatementKind::SumTake { source, .. }
                if super::super::resource_layout::carrier_operations::is_carrier_type(
                    resource.layout.plan(),
                    self.local_types[source.index() as usize].ty,
                ) =>
            {
                claimed = true;
            }
            _ => {}
        }
        if claimed {
            for operation in operations {
                self.carrier_effect(operation, statement.span)?;
            }
        }
        Ok(claimed)
    }
    pub(in crate::emit) fn carrier_iteration_boundary(
        &mut self,
        span: Span,
    ) -> Result<(), CodegenError> {
        let Some(resource) = self.resource else {
            return Ok(());
        };
        if resource.function_plan()?.carriers().is_none() {
            return Ok(());
        }
        for operation in resource.carrier_at_site()? {
            if matches!(operation.role(), CarrierRole::RetireIteration { .. }) {
                self.carrier_effect(operation, span)?;
            }
        }
        Ok(())
    }
    pub(in crate::emit) fn carrier_switch(
        &mut self,
        scrutinee: &Expression,
        variants: &[(
            jett_hir::VariantId,
            jett_mir::BlockId,
            Vec<jett_mir::LocalId>,
        )],
        otherwise: Option<jett_mir::BlockId>,
        span: Span,
    ) -> Result<bool, CodegenError> {
        let Some(resource) = self.resource else {
            return Ok(false);
        };
        if !super::super::resource_layout::carrier_operations::is_carrier_type(
            resource.layout.plan(),
            scrutinee.ty,
        ) {
            return Ok(false);
        }
        self.expression(scrutinee)?;
        let plan = resource.carrier_plan()?;
        let mut tag = None;
        for row in resource.carrier_at_site()? {
            let output = self.carrier_effect(row, span)?;
            if matches!(
                row.role(),
                CarrierRole::Observe {
                    observation: Observation::Tag,
                    ..
                }
            ) {
                if tag.is_some() {
                    return Err(pending("carrier Switch has multiple tag selectors"));
                }
                let output = output.ok_or_else(|| pending("carrier Switch omitted tag output"))?;
                tag = Some(self.scalar(output, span)?);
            }
        }
        let tag = tag.ok_or_else(|| pending("carrier Switch has no exact tag observation"))?;
        let Type::Enum(owner) = self.types.resolve(scrutinee.ty) else {
            return Err(pending("carrier Switch lost its exact enum type"));
        };
        let definition = self.types.resolve_enum(*owner);
        for (variant, target, _) in variants {
            let discriminator = definition
                .variants
                .get(variant.index() as usize)
                .ok_or_else(|| pending("carrier Switch selected a foreign variant"))?
                .discriminant;
            let matches = self
                .builder
                .ins()
                .icmp_imm(IntCC::Equal, tag, discriminator);
            let selected = self.builder.create_block();
            let next = self.builder.create_block();
            self.builder.ins().brif(matches, selected, &[], next, &[]);
            self.builder.switch_to_block(selected);
            let edge = Edge::SwitchVariant {
                source: resource.block,
                variant: *variant,
                target: *target,
            };
            for operation in plan.operations_on_edge(edge) {
                self.carrier_effect(operation, span)?;
            }
            let target = block_for(self.blocks, target.index(), span, self.symbol)?;
            self.drop_temporaries()?;
            self.builder.ins().jump(target, &[]);
            self.builder.switch_to_block(next);
        }
        if let Some(target) = otherwise {
            for operation in plan.operations_on_edge(Edge::SwitchOtherwise {
                source: resource.block,
                target,
            }) {
                self.carrier_effect(operation, span)?;
            }
            self.drop_temporaries()?;
            let target = block_for(self.blocks, target.index(), span, self.symbol)?;
            self.builder.ins().jump(target, &[]);
        } else {
            self.builder.ins().trap(TrapCode::unwrap_user(1));
        }
        Ok(true)
    }
}

fn constructor_child(expression: &Expression, index: usize) -> Result<&Expression, CodegenError> {
    let child = match &expression.kind {
        ExpressionKind::ListConstruct { elements } => elements.get(index),
        ExpressionKind::MapConstruct { entries } => entries.get(index / 2).map(|entry| {
            if index % 2 == 0 {
                &entry.key
            } else {
                &entry.value
            }
        }),
        ExpressionKind::StructConstruct { fields, .. } => fields.get(index),
        ExpressionKind::EnumConstruct { payloads, .. }
        | ExpressionKind::MachineConstruct { payloads, .. } => payloads.get(index),
        ExpressionKind::OptionalSome(inner)
        | ExpressionKind::ResultOk(inner)
        | ExpressionKind::ResultFail(inner)
            if index == 0 =>
        {
            Some(inner.as_ref())
        }
        _ => None,
    };
    child
        .ok_or_else(|| pending("carrier child lost its exact original/current constructor operand"))
}
