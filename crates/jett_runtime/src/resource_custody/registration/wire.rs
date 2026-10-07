//! Bounded owned decoding; wire bytes contain no Source or registry-key authority.

use super::ResourceLayoutError;
use super::schema::*;

pub(super) const MAX_BYTES: usize = 1024 * 1024;
pub(super) const MAX_ROWS: usize = 4096;
const HEADER_BYTES: usize = 56;
const MAGIC: &[u8; 8] = b"JTRSC001";

struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}
impl<'a> Reader<'a> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N], ResourceLayoutError> {
        let end = self
            .position
            .checked_add(N)
            .ok_or(ResourceLayoutError::WireLimit)?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or(ResourceLayoutError::Truncated)?;
        self.position = end;
        Ok(value.try_into().expect("fixed checked slice"))
    }
    fn word(&mut self) -> Result<u32, ResourceLayoutError> {
        Ok(u32::from_le_bytes(self.take()?))
    }
    fn count(&mut self) -> Result<usize, ResourceLayoutError> {
        let count = self.word()? as usize;
        if count > MAX_ROWS {
            return Err(ResourceLayoutError::WireLimit);
        }
        Ok(count)
    }
    fn ordinal(&mut self, index: usize) -> Result<(), ResourceLayoutError> {
        if self.word()? as usize != index {
            return Err(ResourceLayoutError::DenseOrdinal);
        }
        Ok(())
    }
    fn site(&mut self) -> Result<NativeSite, ResourceLayoutError> {
        let function = self.word()?;
        let block = self.word()?;
        let tag = self.word()?;
        let index = self.word()?;
        let position = match (tag, index) {
            (0, index) => NativePosition::Statement(index),
            (1, 0) => NativePosition::Terminator,
            (1, _) => return Err(ResourceLayoutError::Reserved),
            _ => return Err(ResourceLayoutError::UnknownTag),
        };
        Ok(NativeSite {
            function,
            block,
            position,
        })
    }
    fn loan(&mut self, version: u32) -> Result<NativeLoanSource, ResourceLayoutError> {
        Ok(match self.word()? {
            1 => NativeLoanSource::ExistingBorrow {
                operation: self.word()?,
            },
            2 => NativeLoanSource::IncomingViewFormal {
                scope: self.word()?,
                parameter: self.word()?,
            },
            3 if version == 2 => NativeLoanSource::ProjectedSumPayload {
                operation: self.word()?,
            },
            _ => return Err(ResourceLayoutError::UnknownTag),
        })
    }
    fn sum_loan(&mut self) -> Result<NativeSumLoanSource, ResourceLayoutError> {
        Ok(match self.word()? {
            1 => NativeSumLoanSource::ExistingBorrow {
                operation: self.word()?,
            },
            2 => NativeSumLoanSource::IncomingViewFormal {
                scope: self.word()?,
                parameter: self.word()?,
            },
            _ => return Err(ResourceLayoutError::UnknownTag),
        })
    }
    fn shape(&mut self) -> Result<NativeShape, ResourceLayoutError> {
        Ok(match self.word()? {
            1 => {
                let bits = self.word()?;
                let signed = match self.word()? {
                    0 => false,
                    1 => true,
                    _ => return Err(ResourceLayoutError::UnknownTag),
                };
                NativeShape::Integer { bits, signed }
            }
            2 => NativeShape::Float { bits: self.word()? },
            3 => NativeShape::Bool,
            4 => NativeShape::String,
            5 => NativeShape::Nothing,
            6 => NativeShape::Network,
            7 => NativeShape::Resource { kind: self.word()? },
            8 => NativeShape::Optional {
                child: self.word()?,
            },
            9 => NativeShape::Result {
                ok: self.word()?,
                fail: self.word()?,
            },
            10 => NativeShape::HookDescriptor { hook: self.word()? },
            _ => return Err(ResourceLayoutError::UnknownTag),
        })
    }
    fn operation(&mut self, version: u32) -> Result<NativeOperation, ResourceLayoutError> {
        Ok(match self.word()? {
            1 => NativeOperation::Acquire {
                frame: self.word()?,
                hook: self.word()?,
                destination: self.word()?,
            },
            2 => NativeOperation::Transfer {
                frame: self.word()?,
                source: self.word()?,
                destination: self.word()?,
            },
            3 => NativeOperation::Borrow {
                frame: self.word()?,
                source: self.word()?,
                lease_frame: self.word()?,
            },
            4 => NativeOperation::BoundedBorrowUse {
                frame: self.word()?,
                source: self.loan(version)?,
                callee_frame: self.word()?,
            },
            5 => NativeOperation::EndBorrow {
                frame: self.word()?,
                borrow: self.word()?,
            },
            6 => NativeOperation::InvokeBorrow {
                frame: self.word()?,
                hook: self.word()?,
                source: self.loan(version)?,
            },
            7 => NativeOperation::Close {
                frame: self.word()?,
                hook: self.word()?,
                source: self.word()?,
            },
            8 => NativeOperation::Drop {
                frame: self.word()?,
                source: self.word()?,
            },
            9 => NativeOperation::SumAdopt {
                frame: self.word()?,
                source: self.word()?,
                destination: self.word()?,
            },
            10 => NativeOperation::SumTake {
                frame: self.word()?,
                source: self.word()?,
                destination: self.word()?,
            },
            11 => NativeOperation::SumDrop {
                frame: self.word()?,
                source: self.word()?,
            },
            12 => NativeOperation::Replace {
                frame: self.word()?,
                old: self.word()?,
                replacement: self.word()?,
            },
            13 => NativeOperation::Complete {
                frame: self.word()?,
            },
            14 => NativeOperation::Descriptor { hook: self.word()? },
            15 => NativeOperation::InvokeDescriptor {
                hook: self.word()?,
                signature: self.word()?,
                target: self.word()?,
            },
            16 => NativeOperation::InvokeSourceFunction {
                frame: self.word()?,
                callee: self.word()?,
                signature: self.word()?,
                callee_scope: self.word()?,
                source: if version == 2 {
                    Some(self.source_invocation()?)
                } else {
                    None
                },
            },
            17 if version == 2 => NativeOperation::TakeFailureCompanion {
                frame: self.word()?,
                source_sum_slot: self.word()?,
                failure_shape: self.word()?,
            },
            18 if version == 2 => NativeOperation::PublishReturn {
                frame: self.word()?,
                source_return_slot: self.word()?,
            },
            19 if version == 2 => NativeOperation::CreateAbsentSum {
                frame: self.word()?,
                destination_slot: self.word()?,
            },
            20 if version == 2 => NativeOperation::CreateFailureSum {
                frame: self.word()?,
                destination_slot: self.word()?,
                failure_shape: self.word()?,
            },
            21 if version == 2 => NativeOperation::BorrowSum {
                frame: self.word()?,
                source_sum_slot: self.word()?,
                lease_frame: self.word()?,
            },
            22 if version == 2 => NativeOperation::ObserveSumView {
                frame: self.word()?,
                source: self.sum_loan()?,
            },
            23 if version == 2 => NativeOperation::ProjectSumView {
                frame: self.word()?,
                source: self.sum_loan()?,
                path: match self.word()? {
                    1 => NativePayloadStep::Some,
                    2 => NativePayloadStep::Ok,
                    _ => return Err(ResourceLayoutError::UnknownTag),
                },
                lease_frame: self.word()?,
            },
            24 if version == 2 => NativeOperation::ReadFailureCompanion {
                frame: self.word()?,
                source: self.sum_loan()?,
                failure_shape: self.word()?,
            },
            25 if version == 2 => NativeOperation::EndSumBorrow {
                frame: self.word()?,
                borrow: self.word()?,
            },
            _ => return Err(ResourceLayoutError::UnknownTag),
        })
    }
    fn source_invocation(&mut self) -> Result<NativeSourceInvocation, ResourceLayoutError> {
        let callee_return = match self.word()? {
            0 => None,
            1 => Some(self.word()?),
            _ => return Err(ResourceLayoutError::UnknownTag),
        };
        let count = self.count()?;
        let mut evaluation_order = storage(count)?;
        for _ in 0..count {
            evaluation_order.push(self.word()?);
        }
        let count = self.count()?;
        let mut formals = storage(count)?;
        for _ in 0..count {
            let parameter = self.word()?;
            let source_index = self.word()?;
            let actual_shape = self.word()?;
            let callee_shape = self.word()?;
            let syntax = match self.word()? {
                1 => NativeSourceSyntax::Bare,
                2 => NativeSourceSyntax::WrittenView,
                _ => return Err(ResourceLayoutError::UnknownTag),
            };
            let effect = match self.word()? {
                1 => NativeSourceEffect::Copy,
                2 => NativeSourceEffect::TransferOwned,
                3 => NativeSourceEffect::RelinquishOwned,
                4 => NativeSourceEffect::RetainBorrow,
                5 => NativeSourceEffect::ObserveData,
                _ => return Err(ResourceLayoutError::UnknownTag),
            };
            let access = match self.word()? {
                1 => NativeAccess::Owned,
                2 => NativeAccess::View,
                _ => return Err(ResourceLayoutError::UnknownTag),
            };
            let value = match self.word()? {
                1 => NativeSourceValue::Ordinary,
                2 => NativeSourceValue::Owned {
                    caller_argument_slot: self.word()?,
                    callee_parameter_slot: self.word()?,
                },
                3 => NativeSourceValue::ResidentView {
                    source: self.loan(2)?,
                },
                4 => NativeSourceValue::ResidentSumView {
                    source: self.sum_loan()?,
                },
                _ => return Err(ResourceLayoutError::UnknownTag),
            };
            formals.push(NativeSourceFormal {
                parameter,
                source_index,
                actual_shape,
                callee_shape,
                syntax,
                effect,
                access,
                value,
            });
        }
        let result = match self.word()? {
            1 => NativeSourceResult::Ordinary {
                shape: self.word()?,
            },
            2 => {
                let shape = self.word()?;
                let caller_destination_frame = self.word()?;
                let caller_destination_slot = self.word()?;
                let callee_return_frame = self.word()?;
                let count = self.count()?;
                let mut permitted_return_slots = storage(count)?;
                for _ in 0..count {
                    permitted_return_slots.push(self.word()?);
                }
                NativeSourceResult::Owned {
                    shape,
                    caller_destination_frame,
                    caller_destination_slot,
                    callee_return_frame,
                    permitted_return_slots,
                }
            }
            _ => return Err(ResourceLayoutError::UnknownTag),
        };
        Ok(NativeSourceInvocation {
            callee_return,
            evaluation_order,
            formals,
            result,
        })
    }
}

pub(super) fn storage<T>(count: usize) -> Result<Vec<T>, ResourceLayoutError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| ResourceLayoutError::CapacityExhausted)?;
    Ok(values)
}

pub(super) fn decode(bytes: &[u8]) -> Result<WireLayout, ResourceLayoutError> {
    if bytes.len() > MAX_BYTES {
        return Err(ResourceLayoutError::WireLimit);
    }
    if bytes.len() < HEADER_BYTES {
        return Err(ResourceLayoutError::Truncated);
    }
    let mut input = Reader { bytes, position: 0 };
    if &input.take::<8>()? != MAGIC {
        return Err(ResourceLayoutError::Header);
    }
    let version = input.word()?;
    if ![1, super::NATIVE_RESOURCE_LAYOUT_WIRE_VERSION].contains(&version) {
        return Err(ResourceLayoutError::Header);
    }
    if input.word()? != 0 {
        return Err(ResourceLayoutError::Reserved);
    }
    if u64::from_le_bytes(input.take()?) != bytes.len() as u64 {
        return Err(ResourceLayoutError::Length);
    }
    let counts = [
        input.count()?,
        input.count()?,
        input.count()?,
        input.count()?,
        input.count()?,
        input.count()?,
        input.count()?,
    ];
    if input.word()? != 0 {
        return Err(ResourceLayoutError::Reserved);
    }
    let mut kinds = storage(counts[0])?;
    for i in 0..counts[0] {
        input.ordinal(i)?;
        kinds.push(i as u32);
    }
    let mut hooks = storage(counts[1])?;
    for i in 0..counts[1] {
        input.ordinal(i)?;
        let kind = input.word()?;
        let recipe = match input.word()? {
            1 => NativeRecipe::NetworkFactory,
            2 => NativeRecipe::NetworkBorrow,
            3 => NativeRecipe::Finalize,
            _ => return Err(ResourceLayoutError::UnknownTag),
        };
        hooks.push(NativeHook {
            ordinal: i as u32,
            kind,
            recipe,
            signature: input.word()?,
        });
    }
    let mut signatures = storage(counts[2])?;
    for i in 0..counts[2] {
        input.ordinal(i)?;
        let count = input.count()?;
        let result = input.word()?;
        let mut parameters = storage(count)?;
        for _ in 0..count {
            let shape = input.word()?;
            let access = match input.word()? {
                1 => NativeAccess::Owned,
                2 => NativeAccess::View,
                _ => return Err(ResourceLayoutError::UnknownTag),
            };
            parameters.push(NativeFormal { shape, access });
        }
        signatures.push(NativeSignature {
            ordinal: i as u32,
            parameters,
            result,
        });
    }
    let mut shapes = storage(counts[3])?;
    for i in 0..counts[3] {
        input.ordinal(i)?;
        shapes.push(input.shape()?);
    }
    let mut frames = storage(counts[4])?;
    for i in 0..counts[4] {
        input.ordinal(i)?;
        let role = match input.word()? {
            1 => NativeFrameRole::Scope,
            2 => NativeFrameRole::Operation,
            3 => NativeFrameRole::Return,
            _ => return Err(ResourceLayoutError::UnknownTag),
        };
        let site = input.site()?;
        let signature = input.word()?;
        let count = input.count()?;
        let mut parents = storage(count)?;
        for _ in 0..count {
            let tag = input.word()?;
            let index = input.word()?;
            parents.push(match (tag, index) {
                (0, 0) => NativeParent::Root,
                (0, _) => return Err(ResourceLayoutError::Reserved),
                (1, index) => NativeParent::Frame(index),
                _ => return Err(ResourceLayoutError::UnknownTag),
            });
        }
        frames.push(NativeFrame {
            ordinal: i as u32,
            role,
            site,
            signature,
            parents,
        });
    }
    let mut slots = storage(counts[5])?;
    for i in 0..counts[5] {
        input.ordinal(i)?;
        let frame = input.word()?;
        let shape = input.word()?;
        let count = input.count()?;
        let mut path = storage(count)?;
        for _ in 0..count {
            path.push(match input.word()? {
                1 => NativePayloadStep::Some,
                2 => NativePayloadStep::Ok,
                3 => NativePayloadStep::Fail,
                _ => return Err(ResourceLayoutError::UnknownTag),
            });
        }
        slots.push(NativeSlot {
            ordinal: i as u32,
            frame,
            shape,
            path,
        });
    }
    let mut operations = storage(counts[6])?;
    for i in 0..counts[6] {
        input.ordinal(i)?;
        let site = input.site()?;
        operations.push(NativeOperationRecord {
            ordinal: i as u32,
            site,
            operation: input.operation(version)?,
        });
    }
    if input.position != bytes.len() {
        return Err(ResourceLayoutError::Trailing);
    }
    Ok(WireLayout {
        version,
        kinds,
        hooks,
        signatures,
        shapes,
        frames,
        slots,
        operations,
    })
}

// Test encoder pins the same explicit tags/order. It intentionally admits mutated
// records so negative tests exercise production decoding/validation, not a second issuer.
#[cfg(test)]
pub(super) fn encode_records(layout: &WireLayout) -> Vec<u8> {
    struct Writer(Vec<u8>);
    impl Writer {
        fn word(&mut self, value: u32) {
            self.0.extend_from_slice(&value.to_le_bytes());
        }
        fn site(&mut self, site: NativeSite) {
            self.word(site.function);
            self.word(site.block);
            match site.position {
                NativePosition::Statement(i) => {
                    self.word(0);
                    self.word(i);
                }
                NativePosition::Terminator => {
                    self.word(1);
                    self.word(0);
                }
            }
        }
        fn loan(&mut self, source: NativeLoanSource) {
            match source {
                NativeLoanSource::ProjectedSumPayload { operation } => {
                    self.word(3);
                    self.word(operation);
                }
                NativeLoanSource::ExistingBorrow { operation } => {
                    self.word(1);
                    self.word(operation);
                }
                NativeLoanSource::IncomingViewFormal { scope, parameter } => {
                    self.word(2);
                    self.word(scope);
                    self.word(parameter);
                }
            }
        }
        fn sum_loan(&mut self, source: NativeSumLoanSource) {
            match source {
                NativeSumLoanSource::ExistingBorrow { operation } => {
                    self.word(1);
                    self.word(operation);
                }
                NativeSumLoanSource::IncomingViewFormal { scope, parameter } => {
                    self.word(2);
                    self.word(scope);
                    self.word(parameter);
                }
            }
        }
        fn operation(&mut self, operation: &NativeOperation) {
            use NativeOperation::*;
            match *operation {
                Acquire {
                    frame,
                    hook,
                    destination,
                } => {
                    self.word(1);
                    self.word(frame);
                    self.word(hook);
                    self.word(destination);
                }
                Transfer {
                    frame,
                    source,
                    destination,
                } => {
                    self.word(2);
                    self.word(frame);
                    self.word(source);
                    self.word(destination);
                }
                Borrow {
                    frame,
                    source,
                    lease_frame,
                } => {
                    self.word(3);
                    self.word(frame);
                    self.word(source);
                    self.word(lease_frame);
                }
                BoundedBorrowUse {
                    frame,
                    source,
                    callee_frame,
                } => {
                    self.word(4);
                    self.word(frame);
                    self.loan(source);
                    self.word(callee_frame);
                }
                EndBorrow { frame, borrow } => {
                    self.word(5);
                    self.word(frame);
                    self.word(borrow);
                }
                InvokeBorrow {
                    frame,
                    hook,
                    source,
                } => {
                    self.word(6);
                    self.word(frame);
                    self.word(hook);
                    self.loan(source);
                }
                Close {
                    frame,
                    hook,
                    source,
                } => {
                    self.word(7);
                    self.word(frame);
                    self.word(hook);
                    self.word(source);
                }
                Drop { frame, source } => {
                    self.word(8);
                    self.word(frame);
                    self.word(source);
                }
                SumAdopt {
                    frame,
                    source,
                    destination,
                } => {
                    self.word(9);
                    self.word(frame);
                    self.word(source);
                    self.word(destination);
                }
                SumTake {
                    frame,
                    source,
                    destination,
                } => {
                    self.word(10);
                    self.word(frame);
                    self.word(source);
                    self.word(destination);
                }
                SumDrop { frame, source } => {
                    self.word(11);
                    self.word(frame);
                    self.word(source);
                }
                Replace {
                    frame,
                    old,
                    replacement,
                } => {
                    self.word(12);
                    self.word(frame);
                    self.word(old);
                    self.word(replacement);
                }
                Complete { frame } => {
                    self.word(13);
                    self.word(frame);
                }
                Descriptor { hook } => {
                    self.word(14);
                    self.word(hook);
                }
                InvokeDescriptor {
                    hook,
                    signature,
                    target,
                } => {
                    self.word(15);
                    self.word(hook);
                    self.word(signature);
                    self.word(target);
                }
                InvokeSourceFunction {
                    frame,
                    callee,
                    signature,
                    callee_scope,
                    ref source,
                } => {
                    self.word(16);
                    self.word(frame);
                    self.word(callee);
                    self.word(signature);
                    self.word(callee_scope);
                    if let Some(source) = source {
                        self.source_invocation(source);
                    }
                }
                TakeFailureCompanion {
                    frame,
                    source_sum_slot,
                    failure_shape,
                } => {
                    self.word(17);
                    self.word(frame);
                    self.word(source_sum_slot);
                    self.word(failure_shape);
                }
                PublishReturn {
                    frame,
                    source_return_slot,
                } => {
                    self.word(18);
                    self.word(frame);
                    self.word(source_return_slot);
                }
                CreateAbsentSum {
                    frame,
                    destination_slot,
                } => {
                    self.word(19);
                    self.word(frame);
                    self.word(destination_slot);
                }
                CreateFailureSum {
                    frame,
                    destination_slot,
                    failure_shape,
                } => {
                    self.word(20);
                    self.word(frame);
                    self.word(destination_slot);
                    self.word(failure_shape);
                }
                BorrowSum {
                    frame,
                    source_sum_slot,
                    lease_frame,
                } => {
                    self.word(21);
                    self.word(frame);
                    self.word(source_sum_slot);
                    self.word(lease_frame);
                }
                ObserveSumView { frame, source } => {
                    self.word(22);
                    self.word(frame);
                    self.sum_loan(source);
                }
                ProjectSumView {
                    frame,
                    source,
                    path,
                    lease_frame,
                } => {
                    self.word(23);
                    self.word(frame);
                    self.sum_loan(source);
                    self.word(match path {
                        NativePayloadStep::Some => 1,
                        NativePayloadStep::Ok => 2,
                        NativePayloadStep::Fail => 3,
                    });
                    self.word(lease_frame);
                }
                ReadFailureCompanion {
                    frame,
                    source,
                    failure_shape,
                } => {
                    self.word(24);
                    self.word(frame);
                    self.sum_loan(source);
                    self.word(failure_shape);
                }
                EndSumBorrow { frame, borrow } => {
                    self.word(25);
                    self.word(frame);
                    self.word(borrow);
                }
            }
        }
        fn source_invocation(&mut self, source: &NativeSourceInvocation) {
            match source.callee_return {
                None => self.word(0),
                Some(frame) => {
                    self.word(1);
                    self.word(frame);
                }
            }
            self.word(source.evaluation_order.len() as u32);
            for parameter in &source.evaluation_order {
                self.word(*parameter);
            }
            self.word(source.formals.len() as u32);
            for formal in &source.formals {
                self.word(formal.parameter);
                self.word(formal.source_index);
                self.word(formal.actual_shape);
                self.word(formal.callee_shape);
                self.word(match formal.syntax {
                    NativeSourceSyntax::Bare => 1,
                    NativeSourceSyntax::WrittenView => 2,
                });
                self.word(match formal.effect {
                    NativeSourceEffect::Copy => 1,
                    NativeSourceEffect::TransferOwned => 2,
                    NativeSourceEffect::RelinquishOwned => 3,
                    NativeSourceEffect::RetainBorrow => 4,
                    NativeSourceEffect::ObserveData => 5,
                });
                self.word(match formal.access {
                    NativeAccess::Owned => 1,
                    NativeAccess::View => 2,
                });
                match formal.value {
                    NativeSourceValue::Ordinary => self.word(1),
                    NativeSourceValue::Owned {
                        caller_argument_slot,
                        callee_parameter_slot,
                    } => {
                        self.word(2);
                        self.word(caller_argument_slot);
                        self.word(callee_parameter_slot);
                    }
                    NativeSourceValue::ResidentView { source } => {
                        self.word(3);
                        self.loan(source);
                    }
                    NativeSourceValue::ResidentSumView { source } => {
                        self.word(4);
                        self.sum_loan(source);
                    }
                }
            }
            match &source.result {
                NativeSourceResult::Ordinary { shape } => {
                    self.word(1);
                    self.word(*shape);
                }
                NativeSourceResult::Owned {
                    shape,
                    caller_destination_frame,
                    caller_destination_slot,
                    callee_return_frame,
                    permitted_return_slots,
                } => {
                    self.word(2);
                    self.word(*shape);
                    self.word(*caller_destination_frame);
                    self.word(*caller_destination_slot);
                    self.word(*callee_return_frame);
                    self.word(permitted_return_slots.len() as u32);
                    for slot in permitted_return_slots {
                        self.word(*slot);
                    }
                }
            }
        }
    }
    let mut out = Writer(Vec::new());
    out.0.extend_from_slice(MAGIC);
    out.word(layout.version);
    out.word(0);
    out.0.extend_from_slice(&0u64.to_le_bytes());
    for count in [
        layout.kinds.len(),
        layout.hooks.len(),
        layout.signatures.len(),
        layout.shapes.len(),
        layout.frames.len(),
        layout.slots.len(),
        layout.operations.len(),
    ] {
        out.word(count as u32);
    }
    out.word(0);
    for kind in &layout.kinds {
        out.word(*kind);
    }
    for hook in &layout.hooks {
        out.word(hook.ordinal);
        out.word(hook.kind);
        out.word(match hook.recipe {
            NativeRecipe::NetworkFactory => 1,
            NativeRecipe::NetworkBorrow => 2,
            NativeRecipe::Finalize => 3,
        });
        out.word(hook.signature);
    }
    for signature in &layout.signatures {
        out.word(signature.ordinal);
        out.word(signature.parameters.len() as u32);
        out.word(signature.result);
        for parameter in &signature.parameters {
            out.word(parameter.shape);
            out.word(match parameter.access {
                NativeAccess::Owned => 1,
                NativeAccess::View => 2,
            });
        }
    }
    for (i, shape) in layout.shapes.iter().enumerate() {
        out.word(i as u32);
        match *shape {
            NativeShape::Integer { bits, signed } => {
                out.word(1);
                out.word(bits);
                out.word(u32::from(signed));
            }
            NativeShape::Float { bits } => {
                out.word(2);
                out.word(bits);
            }
            NativeShape::Bool => out.word(3),
            NativeShape::String => out.word(4),
            NativeShape::Nothing => out.word(5),
            NativeShape::Network => out.word(6),
            NativeShape::Resource { kind } => {
                out.word(7);
                out.word(kind);
            }
            NativeShape::Optional { child } => {
                out.word(8);
                out.word(child);
            }
            NativeShape::Result { ok, fail } => {
                out.word(9);
                out.word(ok);
                out.word(fail);
            }
            NativeShape::HookDescriptor { hook } => {
                out.word(10);
                out.word(hook);
            }
        }
    }
    for frame in &layout.frames {
        out.word(frame.ordinal);
        out.word(match frame.role {
            NativeFrameRole::Scope => 1,
            NativeFrameRole::Operation => 2,
            NativeFrameRole::Return => 3,
        });
        out.site(frame.site);
        out.word(frame.signature);
        out.word(frame.parents.len() as u32);
        for parent in &frame.parents {
            match *parent {
                NativeParent::Root => {
                    out.word(0);
                    out.word(0);
                }
                NativeParent::Frame(index) => {
                    out.word(1);
                    out.word(index);
                }
            }
        }
    }
    for slot in &layout.slots {
        out.word(slot.ordinal);
        out.word(slot.frame);
        out.word(slot.shape);
        out.word(slot.path.len() as u32);
        for step in &slot.path {
            out.word(match step {
                NativePayloadStep::Some => 1,
                NativePayloadStep::Ok => 2,
                NativePayloadStep::Fail => 3,
            });
        }
    }
    for operation in &layout.operations {
        out.word(operation.ordinal);
        out.site(operation.site);
        out.operation(&operation.operation);
    }
    let length = out.0.len() as u64;
    out.0[16..24].copy_from_slice(&length.to_le_bytes());
    out.0
}
