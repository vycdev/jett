//! Exact v3 operation projection. The fresh private plan, not a wire type/name,
//! selects every carrier slot, path, constructor child, and lifetime edge.
use super::*;
use custody::{
    ResourceCarrierConstructor as Constructor, ResourceCarrierLoanSource as LoanSource,
    ResourceCarrierObservation as Observation, ResourceCarrierOperationRole as CarrierRole,
    ResourceCarrierProjectionPath as Path, ResourceCarrierValue as CarrierValue,
};

pub(in crate::emit) fn is_carrier_type(
    plan: &custody::ResourceOwnershipPlan<'_>,
    ty: TypeId,
) -> bool {
    if !plan
        .carrier_shapes()
        .iter()
        .any(|shape| shape.ty() == ty && shape.contains_resource())
    {
        return false;
    }
    match plan.types().resolve(ty) {
        Type::Resource(_) | Type::Function { .. } => false,
        Type::Optional(inner) if matches!(plan.types().resolve(*inner), Type::Resource(_)) => false,
        Type::Result(ok, fail)
            if matches!(plan.types().resolve(*ok), Type::Resource(_))
                && !plan.type_requires_custody(*fail) =>
        {
            false
        }
        _ => true,
    }
}

/// Child/commit and the explicit borrowed-sum activation use the one row owned by
/// their constructor. A dead frame or matching signature cannot create an alias.
pub(super) fn alias<'a>(
    function: &'a custody::ResourceFunctionPlan,
    operation: &custody::ResourceOperation,
) -> Result<Option<&'a custody::ResourceOperation>, CodegenError> {
    let Some(carriers) = function.carriers() else {
        return Ok(None);
    };
    let authority = match operation.role() {
        Role::Carrier { operation: id } => {
            let current = carriers
                .operations()
                .get(id.index())
                .filter(|row| row.id() == *id)
                .ok_or_else(|| pending("carrier wrapper lost its exact private operation"))?;
            let destination = match current.role() {
                CarrierRole::ConstructorChild { destination, .. }
                | CarrierRole::CommitConstructor { destination } => *destination,
                _ => return Ok(None),
            };
            let mut starts = carriers.operations().iter().filter(|row|
                matches!(row.role(), CarrierRole::BeginConstructor { destination: slot, .. } if *slot == destination));
            let start = starts
                .next()
                .ok_or_else(|| pending("carrier activation has no constructor begin"))?;
            if starts.next().is_some()
                || start.site() != current.site()
                || start.frame() != current.frame()
                || start.id().index() >= current.id().index()
            {
                return Err(pending(
                    "carrier activation changes its exact constructor/site/order",
                ));
            }
            start.id()
        }
        Role::BorrowSum { loan } => {
            let record = function
                .loans()
                .get(loan.index())
                .filter(|row| row.id() == *loan)
                .ok_or_else(|| pending("carrier sum activation lost its exact loan"))?;
            let custody::ResourceLoanSource::CarrierSumProjection { operation: id } =
                record.source()
            else {
                return Ok(None);
            };
            let adapter = carriers
                .operations()
                .get(id.index())
                .filter(|row| row.id() == id)
                .ok_or_else(|| pending("carrier sum loan lost its exact adapter"))?;
            if adapter.frame() != operation.frame()
                || !matches!(adapter.role(), CarrierRole::AdaptSum { loan: Some(expected), .. } if *expected == *loan)
            {
                return Err(pending(
                    "carrier sum activation changes its parent/lease/loan",
                ));
            }
            id
        }
        _ => return Ok(None),
    };
    let mut wrappers = function.operations().iter().filter(|candidate|
        matches!(candidate.role(), Role::Carrier { operation } if *operation == authority));
    let selected = wrappers
        .next()
        .ok_or_else(|| pending("carrier authority has no exact wrapper"))?;
    if wrappers.next().is_some() {
        return Err(pending("carrier authority has duplicate wrappers"));
    }
    Ok(Some(selected))
}

impl Rows<'_, '_> {
    pub(super) fn carrier_node(&mut self, ty: TypeId) -> Result<u32, CodegenError> {
        if let Some(node) = self.carrier_graph.node_for_type(ty) {
            return Ok(node);
        }
        let plan = self.plan;
        let carriers = plan
            .functions()
            .iter()
            .find_map(|function| function.carriers())
            .ok_or_else(|| pending("carrier shape has no constructor-owned graph"))?;
        let shape = carriers
            .shape_for_type(ty)
            .ok_or_else(|| pending("carrier shape is outside its exact checked graph"))?;
        let mut graph = std::mem::take(&mut self.carrier_graph);
        let result = graph.project(
            carriers,
            plan.types(),
            shape.id(),
            &mut |ty| self.shape(ty),
            &mut |ty| {
                let predicate = carriers
                    .shape_for_type(ty)
                    .and_then(|shape| shape.refinement_predicate())
                    .ok_or_else(|| {
                        pending("refinement graph lost its original checked predicate/body")
                    })?;
                // The private getter authenticates the unique retained header and full
                // body against the original checked archive/current witness. These
                // bytes remain consistency metadata; runtime never grants a predicate
                // or nominal proof because a type spelling happens to match.
                use sha2::{Digest, Sha256};
                let mut bytes = predicate.id.index().to_le_bytes().to_vec();
                bytes.extend_from_slice(&Sha256::digest(format!("{:?}", predicate).as_bytes()));
                Ok(bytes)
            },
        );
        self.carrier_graph = graph;
        result
    }

    pub(super) fn carrier_slot(
        &self,
        function: FunctionId,
        slot: custody::ResourceCarrierSlotId,
    ) -> Result<u32, CodegenError> {
        self.carrier_slot_ids
            .get(&(function.index(), slot.index()))
            .copied()
            .ok_or_else(|| pending("carrier slot is outside its exact function plan"))
    }

    pub(super) fn carrier_loan(
        &self,
        function: &custody::ResourceFunctionPlan,
        loan: custody::ResourceCarrierLoanId,
    ) -> Result<Vec<u32>, CodegenError> {
        let carriers = function
            .carriers()
            .ok_or_else(|| pending("carrier loan has no current plan"))?;
        let row = carriers
            .loans()
            .get(loan.index())
            .filter(|row| row.id() == loan)
            .ok_or_else(|| pending("carrier loan is outside its exact plan"))?;
        if let Some(operation) = self
            .carrier_borrow_ids
            .get(&(function.function().index(), loan.index()))
        {
            return Ok(vec![1, *operation]);
        }
        match row.source() {
            LoanSource::IncomingViewFormal { scope, parameter } if row.path().is_empty() => {
                Ok(vec![
                    2,
                    self.frame(function.function(), scope)?,
                    count(parameter)?,
                ])
            }
            _ => Err(pending(
                "carrier loan has no unique Borrow or exact incoming formal",
            )),
        }
    }

    fn carrier_source(
        &self,
        function: &custody::ResourceFunctionPlan,
        source: LoanSource,
    ) -> Result<Vec<u32>, CodegenError> {
        let mut words = Vec::new();
        match source {
            LoanSource::Slot(slot) => {
                words.extend([1, self.carrier_slot(function.function(), slot)?])
            }
            LoanSource::Loan(loan) => {
                words.push(2);
                words.extend(self.carrier_loan(function, loan)?);
            }
            LoanSource::IncomingViewFormal { scope, parameter } => words.extend([
                2,
                2,
                self.frame(function.function(), scope)?,
                count(parameter)?,
            ]),
        }
        Ok(words)
    }

    fn borrowed_source(
        &self,
        function: &custody::ResourceFunctionPlan,
        loan: custody::ResourceCarrierLoanId,
    ) -> Result<Vec<u32>, CodegenError> {
        let mut row = vec![2];
        row.extend(self.carrier_loan(function, loan)?);
        Ok(row)
    }

    /// Owning extraction must resolve to the exact root, rather than converting
    /// an incoming or sibling loan into ownership. Paths stay in original order.
    fn owned_root(
        &self,
        function: &custody::ResourceFunctionPlan,
        loan: custody::ResourceCarrierLoanId,
    ) -> Result<(custody::ResourceCarrierSlotId, Vec<Path>), CodegenError> {
        let carriers = function
            .carriers()
            .ok_or_else(|| pending("carrier extraction has no plan"))?;
        let mut visited = std::collections::BTreeSet::new();
        let mut cursor = loan;
        let mut paths = Vec::new();
        loop {
            if !visited.insert(cursor.index()) {
                return Err(pending("carrier extraction has a cyclic loan origin"));
            }
            let row = carriers
                .loans()
                .get(cursor.index())
                .filter(|row| row.id() == cursor)
                .ok_or_else(|| pending("carrier extraction lost its parent loan"))?;
            paths.push(row.path().to_vec());
            match row.source() {
                LoanSource::Slot(slot) => {
                    let mut path = Vec::new();
                    for segment in paths.iter().rev() {
                        path.extend_from_slice(segment);
                    }
                    return Ok((slot, path));
                }
                LoanSource::Loan(parent) => cursor = parent,
                LoanSource::IncomingViewFormal { .. } => {
                    return Err(pending("owning extraction cannot consume an incoming view"));
                }
            }
        }
    }

    pub(super) fn allocate_carrier_operation(
        &mut self,
        function: &custody::ResourceFunctionPlan,
        id: custody::ResourceCarrierOperationId,
        wrapper: u32,
    ) -> Result<(), CodegenError> {
        let carriers = function
            .carriers()
            .ok_or_else(|| pending("carrier wrapper has no current plan"))?;
        let row = carriers
            .operations()
            .get(id.index())
            .filter(|row| row.id() == id)
            .ok_or_else(|| pending("carrier wrapper changed its exact operation"))?;
        let record = ordinal(self.carrier_operations.len())?;
        if self
            .carrier_operation_ids
            .insert((function.function().index(), id.index()), wrapper)
            .is_some()
            || self
                .carrier_record_ids
                .insert((function.function().index(), id.index()), record)
                .is_some()
        {
            return Err(pending("carrier operation already has a wire row"));
        }
        match row.role() {
            CarrierRole::Borrow { loan }
            | CarrierRole::Project {
                destination: loan, ..
            } => {
                if self
                    .carrier_borrow_ids
                    .insert((function.function().index(), loan.index()), wrapper)
                    .is_some()
                {
                    return Err(pending("carrier loan has duplicate exact Borrow rows"));
                }
            }
            CarrierRole::AdaptSum {
                loan: Some(loan), ..
            } => {
                if self
                    .borrow_ids
                    .insert((function.function().index(), loan.index()), wrapper)
                    .is_some()
                {
                    return Err(pending("carrier sum loan has duplicate adapter rows"));
                }
            }
            _ => {}
        }
        self.carrier_operations.push(Vec::new());
        Ok(())
    }

    pub(super) fn encode_carrier_operation(
        &mut self,
        function: &custody::ResourceFunctionPlan,
        id: custody::ResourceCarrierOperationId,
    ) -> Result<u32, CodegenError> {
        let carriers = function
            .carriers()
            .ok_or_else(|| pending("carrier wrapper lost its private plan"))?;
        let operation = carriers
            .operations()
            .get(id.index())
            .filter(|row| row.id() == id)
            .ok_or_else(|| pending("carrier wrapper lost its exact occurrence"))?;
        let record = *self
            .carrier_record_ids
            .get(&(function.function().index(), id.index()))
            .ok_or_else(|| pending("carrier occurrence has no allocated row"))?;
        let frame = self.frame(function.function(), operation.frame())?;
        let mut row = Vec::new();
        site(&mut row, operation.site())?;
        let slot = |this: &Self, slot| this.carrier_slot(function.function(), slot);
        match operation.role() {
            CarrierRole::BeginConstructor {
                destination,
                constructor,
            } => {
                words(
                    &mut row,
                    &[
                        1,
                        frame,
                        slot(self, *destination)?,
                        constructor_selector(*constructor)?,
                    ],
                );
                let mut children = Vec::new();
                let mut committed = false;
                for candidate in carriers.operations().iter().skip(id.index() + 1) {
                    match candidate.role() {
                        CarrierRole::ConstructorChild {
                            destination: target,
                            child,
                        } if target == destination => {
                            if candidate.site() != operation.site()
                                || candidate.frame() != operation.frame()
                                || committed
                            {
                                return Err(pending(
                                    "constructor child changes exact frame/site/commit order",
                                ));
                            }
                            children.push(child);
                        }
                        CarrierRole::CommitConstructor {
                            destination: target,
                        } if target == destination => {
                            if candidate.site() != operation.site()
                                || candidate.frame() != operation.frame()
                                || committed
                            {
                                return Err(pending(
                                    "constructor commit is duplicated or disconnected",
                                ));
                            }
                            committed = true;
                        }
                        _ => {}
                    }
                }
                if !committed {
                    return Err(pending("carrier constructor has no exact commit"));
                }
                word(&mut row, count(children.len())?);
                for child in children {
                    let ty = carriers
                        .shape(child.shape)
                        .ok_or_else(|| pending("constructor child lost its exact node"))?
                        .ty();
                    words(&mut row, &[count(child.index)?, self.carrier_node(ty)?]);
                    match child.value {
                        CarrierValue::Ordinary { ty } => words(&mut row, &[1, self.shape(ty)?]),
                        CarrierValue::Owned { slot: source } => {
                            words(&mut row, &[2, slot(self, source)?])
                        }
                        CarrierValue::LeafOwned { slot: source } => {
                            words(&mut row, &[3, self.slot(function.function(), source)?])
                        }
                        CarrierValue::Borrowed { .. } | CarrierValue::LeafBorrowed { .. } => {
                            return Err(pending(
                                "owning carrier constructor cannot adopt a borrowed child",
                            ));
                        }
                    }
                }
            }
            CarrierRole::Transfer {
                source,
                destination,
            } => words(
                &mut row,
                &[2, frame, slot(self, *source)?, slot(self, *destination)?],
            ),
            CarrierRole::QualifyMachine {
                source,
                destination,
                machine,
                state,
            } => {
                let ty = carriers
                    .shape(*machine)
                    .ok_or_else(|| pending("carrier qualification lost its exact machine"))?
                    .ty();
                words(
                    &mut row,
                    &[
                        11,
                        frame,
                        slot(self, *source)?,
                        slot(self, *destination)?,
                        self.carrier_node(ty)?,
                        count(*state)?,
                    ],
                );
            }
            CarrierRole::Borrow { loan }
            | CarrierRole::Project {
                destination: loan, ..
            } => {
                let loan = carriers
                    .loans()
                    .get(loan.index())
                    .ok_or_else(|| pending("carrier Borrow lost its exact loan"))?;
                words(&mut row, &[3, frame]);
                row.extend(words_bytes(&self.carrier_source(function, loan.source())?));
                path(&mut row, loan.path())?;
                let ty = carriers
                    .shape(loan.shape())
                    .ok_or_else(|| pending("carrier Borrow lost its exact shape"))?
                    .ty();
                words(
                    &mut row,
                    &[
                        self.carrier_node(ty)?,
                        self.frame(function.function(), loan.frame())?,
                    ],
                );
            }
            CarrierRole::EndBorrow { loan } => words(
                &mut row,
                &[
                    4,
                    frame,
                    *self
                        .carrier_borrow_ids
                        .get(&(function.function().index(), loan.index()))
                        .ok_or_else(|| pending("carrier EndBorrow has no exact activation row"))?,
                ],
            ),
            CarrierRole::Observe {
                source,
                observation,
                ..
            } => {
                words(&mut row, &[5, frame]);
                row.extend(words_bytes(&self.borrowed_source(function, *source)?));
                match observation {
                    Observation::Length => {
                        path(&mut row, &[])?;
                        word(&mut row, 1);
                    }
                    Observation::Tag => {
                        path(&mut row, &[])?;
                        word(&mut row, 2);
                    }
                    Observation::State { .. } => {
                        path(&mut row, &[])?;
                        word(&mut row, 3);
                    }
                    Observation::Ordinary { path: selected, ty } => {
                        path(&mut row, selected)?;
                        let clone = match self.plan.types().resolve(*ty) {
                            Type::String => 1,
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
                            | Type::Bool
                            | Type::Nothing => 0,
                            _ => {
                                return Err(pending(
                                    "carrier ordinary observation needs its exact owning clone transport",
                                ));
                            }
                        };
                        words(&mut row, &[4, self.shape(*ty)?, clone]);
                    }
                }
            }
            CarrierRole::Extract {
                source,
                path: selected,
                destination,
            } => {
                let (root, mut full) = self.owned_root(function, *source)?;
                full.extend_from_slice(selected);
                words(&mut row, &[6, frame, slot(self, root)?]);
                path(&mut row, &full)?;
                words(&mut row, &[1, slot(self, *destination)?]);
            }
            CarrierRole::ExtractOrdinary {
                source,
                path: selected,
                ty,
                ..
            } => {
                let (root, mut full) = self.owned_root(function, *source)?;
                full.extend_from_slice(selected);
                words(&mut row, &[6, frame, slot(self, root)?]);
                path(&mut row, &full)?;
                words(&mut row, &[2, self.shape(*ty)?]);
            }
            CarrierRole::AdaptSum {
                source,
                path: selected,
                destination,
                loan,
                lease_frame,
            } => {
                words(&mut row, &[7, frame]);
                if loan.is_some() {
                    row.extend(words_bytes(&self.borrowed_source(function, *source)?));
                    path(&mut row, selected)?;
                } else {
                    let (root, mut full) = self.owned_root(function, *source)?;
                    full.extend_from_slice(selected);
                    words(&mut row, &[1, slot(self, root)?]);
                    path(&mut row, &full)?;
                }
                words(
                    &mut row,
                    &[
                        self.slot(function.function(), *destination)?,
                        self.frame(function.function(), *lease_frame)?,
                        u32::from(loan.is_some()),
                    ],
                );
            }
            CarrierRole::BeginIteration {
                source,
                destination,
                ..
            } => {
                let CarrierValue::Owned { slot: source } = source else {
                    return Err(pending(
                        "consuming carrier loop requires its exact owned source",
                    ));
                };
                words(
                    &mut row,
                    &[2, frame, slot(self, *source)?, slot(self, *destination)?],
                );
            }
            CarrierRole::RetireIteration { source, binders } => {
                let mut carrier_binders = Vec::new();
                let mut leaf_binders = Vec::new();
                for binder in binders {
                    let mut carrier = carriers.slots().iter().filter(|slot| matches!(slot.storage(), custody::ResourceSlotStorage::Local { header } if header.id == *binder));
                    if let Some(selected) = carrier.next() {
                        if carrier.next().is_some() {
                            return Err(pending("iteration binder has ambiguous carrier storage"));
                        }
                        carrier_binders.push(slot(self, selected.id())?);
                        continue;
                    }
                    let mut leaf = function.owner_slots().iter().filter(|slot| matches!(slot.storage(), custody::ResourceSlotStorage::Local { header } if header.id == *binder));
                    if let Some(selected) = leaf.next() {
                        if leaf.next().is_some() {
                            return Err(pending("iteration binder has ambiguous sum storage"));
                        }
                        leaf_binders.push(self.slot(function.function(), selected.id())?);
                    }
                }
                words(
                    &mut row,
                    &[
                        10,
                        frame,
                        slot(self, *source)?,
                        count(carrier_binders.len())?,
                    ],
                );
                words(&mut row, &carrier_binders);
                word(&mut row, count(leaf_binders.len())?);
                words(&mut row, &leaf_binders);
            }
            CarrierRole::PublishReturn { source } => {
                words(&mut row, &[9, frame, slot(self, *source)?])
            }
            CarrierRole::Retire { source } => words(&mut row, &[8, frame, slot(self, *source)?]),
            CarrierRole::ConstructorChild { .. } | CarrierRole::CommitConstructor { .. } => {
                return Err(pending(
                    "constructor activation must alias its one Begin row",
                ));
            }
        }
        self.carrier_operations[record as usize] = row;
        Ok(record)
    }
}

fn constructor_selector(constructor: Constructor) -> Result<u32, CodegenError> {
    Ok(match constructor {
        Constructor::List
        | Constructor::Map
        | Constructor::Struct
        | Constructor::OptionalNone
        | Constructor::ResultFail => 0,
        Constructor::OptionalSome | Constructor::ResultOk => 1,
        Constructor::Enum { variant } => variant.index(),
        Constructor::Machine { state } => count(state)?,
    })
}
fn words_bytes(input: &[u32]) -> Vec<u8> {
    let mut bytes = Vec::new();
    words(&mut bytes, input);
    bytes
}
fn path(output: &mut Vec<u8>, input: &[Path]) -> Result<(), CodegenError> {
    word(output, count(input.len())?);
    for step in input {
        match step {
            Path::Field { field, .. } => words(output, &[1, field.index()]),
            Path::Variant { variant, field, .. } => {
                words(output, &[2, variant.index(), count(*field)?])
            }
            Path::State { state, field, .. } => words(output, &[3, count(*state)?, count(*field)?]),
            Path::Element { index } => {
                word(output, 4);
                selector(output, *index)?;
            }
            Path::MapKey { index } => {
                word(output, 5);
                selector(output, *index)?;
            }
            Path::MapValue { index } => {
                word(output, 6);
                selector(output, *index)?;
            }
            Path::OptionalSome => word(output, 7),
            Path::ResultOk => word(output, 8),
            Path::ResultFail => word(output, 9),
        }
    }
    Ok(())
}
fn selector(
    output: &mut Vec<u8>,
    index: custody::ResourceCarrierIndex,
) -> Result<(), CodegenError> {
    match index {
        custody::ResourceCarrierIndex::Constant(index) => words(output, &[0, count(index)?]),
        custody::ResourceCarrierIndex::Local(_)
        | custody::ResourceCarrierIndex::CurrentIteration { .. } => word(output, 1),
    }
    Ok(())
}
