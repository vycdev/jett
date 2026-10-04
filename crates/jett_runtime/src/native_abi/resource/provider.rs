use super::*;
#[cfg(test)]
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct NativeProviderIdentity(NonZeroU64);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct NativeRestrictionIdentity(NonZeroU64);
impl NativeProviderIdentity {
    pub(super) fn new(raw: u64) -> ResourceResult<Self> {
        NonZeroU64::new(raw)
            .map(Self)
            .ok_or(NativeResourceError::WrongGrant)
    }
    pub(super) fn raw(self) -> u64 {
        self.0.get()
    }
}
impl NativeRestrictionIdentity {
    pub(super) fn new(raw: u64) -> ResourceResult<Self> {
        NonZeroU64::new(raw)
            .map(Self)
            .ok_or(NativeResourceError::WrongGrant)
    }
    pub(super) fn raw(self) -> u64 {
        self.0.get()
    }
}

pub(super) struct NativePayload {
    label: i64,
}
pub(super) struct ConstructedPayload {
    pub(super) payload: NativePayload,
    pub(super) finalizer: Box<dyn FnOnce(NativePayload) + Send>,
}
pub(super) enum InstalledProvider {
    Disabled,
    #[cfg(test)]
    Scripted(ScriptedProvider),
}
impl InstalledProvider {
    pub(super) fn enabled(&self) -> bool {
        match self {
            Self::Disabled => false,
            #[cfg(test)]
            Self::Scripted(_) => true,
        }
    }
    pub(super) fn bind(
        &mut self,
        identity: NativeProviderIdentity,
        grant: Option<ResourceHandleId>,
    ) -> ResourceResult<()> {
        match self {
            Self::Disabled => {
                let _ = (identity, grant);
                Ok(())
            }
            #[cfg(test)]
            Self::Scripted(provider) => {
                provider.identity = Some(identity);
                provider.grant = grant;
                Ok(())
            }
        }
    }
    pub(super) fn matches(
        &self,
        identity: NativeProviderIdentity,
        grant: ResourceHandleId,
    ) -> bool {
        match self {
            Self::Disabled => {
                let _ = (identity, grant);
                false
            }
            #[cfg(test)]
            Self::Scripted(provider) => {
                provider.identity == Some(identity) && provider.grant == Some(grant)
            }
        }
    }
    pub(super) fn construct(
        &mut self,
        label: i64,
    ) -> ResourceResult<Result<ConstructedPayload, String>> {
        match self {
            Self::Disabled => {
                let _ = label;
                Err(NativeResourceError::WrongGrant)
            }
            #[cfg(test)]
            Self::Scripted(provider) => provider.construct(label),
        }
    }
    pub(super) fn borrow(
        &mut self,
        payload: &mut NativePayload,
    ) -> ResourceResult<Result<i64, String>> {
        match self {
            Self::Disabled => {
                let _ = payload;
                Err(NativeResourceError::WrongGrant)
            }
            #[cfg(test)]
            Self::Scripted(provider) => provider.borrow(payload.label),
        }
    }
    #[cfg(test)]
    pub(super) fn scripted(script: DecodedScript) -> ResourceResult<Self> {
        let mut events = Vec::new();
        events
            .try_reserve_exact(
                script
                    .operations
                    .len()
                    .checked_mul(2)
                    .ok_or(NativeResourceError::Capacity)?,
            )
            .map_err(|_| NativeResourceError::Capacity)?;
        Ok(Self::Scripted(ScriptedProvider {
            identity: None,
            grant: None,
            operations: script.operations,
            telemetry: Arc::new(Mutex::new(Telemetry {
                events,
                next_sequence: 1,
            })),
            attempts: script.attempts,
        }))
    }
    #[cfg(test)]
    pub(super) fn attempts(&self) -> usize {
        match self {
            Self::Disabled => MAX_ATTEMPTS,
            Self::Scripted(provider) => provider.attempts,
        }
    }
    #[cfg(test)]
    pub(super) fn remaining(&self) -> usize {
        match self {
            Self::Disabled => 0,
            Self::Scripted(provider) => provider.operations.len(),
        }
    }
    #[cfg(test)]
    pub(super) fn observe_events<T>(&self, operation: impl FnOnce(&[NativeTestEvent]) -> T) -> T {
        match self {
            Self::Disabled => operation(&[]),
            Self::Scripted(provider) => operation(&lock_unpoisoned(&provider.telemetry).events),
        }
    }
}

#[repr(C)]
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::native_abi) struct NativeTestEvent {
    pub(in crate::native_abi) sequence: u64,
    pub(in crate::native_abi) label: i64,
    pub(in crate::native_abi) kind: u32,
    pub(in crate::native_abi) reserved: u32,
}
#[cfg(test)]
struct Telemetry {
    events: Vec<NativeTestEvent>,
    next_sequence: u64,
}
#[cfg(test)]
impl Telemetry {
    fn event(&mut self, label: i64, kind: u32) {
        // Full event capacity and the small script bound were checked at installation.
        self.events.push(NativeTestEvent {
            sequence: self.next_sequence,
            label,
            kind,
            reserved: 0,
        });
        self.next_sequence += 1;
    }
}
#[cfg(test)]
pub(super) struct ScriptedProvider {
    identity: Option<NativeProviderIdentity>,
    grant: Option<ResourceHandleId>,
    operations: VecDeque<ScriptOperation>,
    telemetry: Arc<Mutex<Telemetry>>,
    attempts: usize,
}
#[cfg(test)]
enum ScriptOperation {
    ConstructOk(i64),
    ConstructFail(i64, String),
    ConstructFinalizerPanic(i64),
    BorrowOk(i64, i64),
    BorrowFail(i64, String),
    BorrowPanic(i64),
}
#[cfg(test)]
impl ScriptedProvider {
    fn construct(&mut self, label: i64) -> ResourceResult<Result<ConstructedPayload, String>> {
        match self.operations.front() {
            Some(
                ScriptOperation::ConstructOk(expected)
                | ScriptOperation::ConstructFail(expected, _)
                | ScriptOperation::ConstructFinalizerPanic(expected),
            ) if *expected == label => {}
            _ => return Err(NativeResourceError::WrongOperation),
        }
        let row = self
            .operations
            .pop_front()
            .ok_or(NativeResourceError::WrongOperation)?;
        let panic_on_drop = matches!(&row, ScriptOperation::ConstructFinalizerPanic(_));
        match row {
            ScriptOperation::ConstructFail(_, error) => {
                lock_unpoisoned(&self.telemetry).event(label, 2);
                Ok(Err(error))
            }
            ScriptOperation::ConstructOk(_) | ScriptOperation::ConstructFinalizerPanic(_) => {
                lock_unpoisoned(&self.telemetry).event(label, 1);
                let telemetry = self.telemetry.clone();
                Ok(Ok(ConstructedPayload {
                    payload: NativePayload { label },
                    finalizer: Box::new(move |payload| {
                        lock_unpoisoned(&telemetry).event(payload.label, 5);
                        if panic_on_drop {
                            panic!("scripted native Resource finalizer panic");
                        }
                    }),
                }))
            }
            _ => Err(NativeResourceError::WrongOperation),
        }
    }
    fn borrow(&mut self, label: i64) -> ResourceResult<Result<i64, String>> {
        match self.operations.front() {
            Some(
                ScriptOperation::BorrowOk(expected, _)
                | ScriptOperation::BorrowFail(expected, _)
                | ScriptOperation::BorrowPanic(expected),
            ) if *expected == label => {}
            _ => return Err(NativeResourceError::WrongOperation),
        }
        let kind = if matches!(
            self.operations.front(),
            Some(ScriptOperation::BorrowFail(..))
        ) {
            4
        } else {
            3
        };
        // The reference records Borrowed before popping and panicking.
        lock_unpoisoned(&self.telemetry).event(label, kind);
        match self
            .operations
            .pop_front()
            .ok_or(NativeResourceError::WrongOperation)?
        {
            ScriptOperation::BorrowOk(_, result) => Ok(Ok(result)),
            ScriptOperation::BorrowFail(_, error) => Ok(Err(error)),
            ScriptOperation::BorrowPanic(_) => panic!("scripted native Resource borrow panic"),
            _ => Err(NativeResourceError::WrongOperation),
        }
    }
}

#[cfg(test)]
pub(in crate::native_abi) struct DecodedScript {
    attempts: usize,
    operations: VecDeque<ScriptOperation>,
}
#[cfg(test)]
impl DecodedScript {
    pub(in crate::native_abi) fn decode(bytes: &[u8]) -> ResourceResult<Self> {
        if bytes.len() > 1024 * 1024 {
            return Err(NativeResourceError::Capacity);
        }
        let mut reader = ScriptReader { bytes, offset: 0 };
        if reader.bytes(8)? != b"JTRST001"
            || reader.u32()? != 1
            || reader.u32()? != 0
            || reader.u64()? != bytes.len() as u64
        {
            return Err(NativeResourceError::WrongOperation);
        }
        let attempts = reader.u32()? as usize;
        let count = reader.u32()? as usize;
        if !(1..=MAX_ATTEMPTS).contains(&attempts) || count > 4096 {
            return Err(NativeResourceError::Capacity);
        }
        let mut operations = VecDeque::new();
        operations
            .try_reserve(count)
            .map_err(|_| NativeResourceError::Capacity)?;
        for _ in 0..count {
            let tag = reader.u32()?;
            let label = reader.u64()? as i64;
            operations.push_back(match tag {
                1 => ScriptOperation::ConstructOk(label),
                2 => ScriptOperation::ConstructFail(label, reader.string()?),
                3 => ScriptOperation::ConstructFinalizerPanic(label),
                4 => ScriptOperation::BorrowOk(label, reader.u64()? as i64),
                5 => ScriptOperation::BorrowFail(label, reader.string()?),
                6 => ScriptOperation::BorrowPanic(label),
                _ => return Err(NativeResourceError::WrongOperation),
            });
        }
        if reader.offset != bytes.len() {
            return Err(NativeResourceError::WrongOperation);
        }
        Ok(Self {
            attempts,
            operations,
        })
    }
}
#[cfg(test)]
struct ScriptReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
#[cfg(test)]
impl<'a> ScriptReader<'a> {
    fn bytes(&mut self, count: usize) -> ResourceResult<&'a [u8]> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(NativeResourceError::Capacity)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(NativeResourceError::WrongOperation)?;
        self.offset = end;
        Ok(bytes)
    }
    fn u32(&mut self) -> ResourceResult<u32> {
        let bytes: [u8; 4] = self
            .bytes(4)?
            .try_into()
            .map_err(|_| NativeResourceError::WrongOperation)?;
        Ok(u32::from_le_bytes(bytes))
    }
    fn u64(&mut self) -> ResourceResult<u64> {
        let bytes: [u8; 8] = self
            .bytes(8)?
            .try_into()
            .map_err(|_| NativeResourceError::WrongOperation)?;
        Ok(u64::from_le_bytes(bytes))
    }
    fn string(&mut self) -> ResourceResult<String> {
        let length = self.u32()? as usize;
        if length > 64 * 1024 {
            return Err(NativeResourceError::Capacity);
        }
        let text = std::str::from_utf8(self.bytes(length)?)
            .map_err(|_| NativeResourceError::WrongOperation)?;
        let mut owned = String::new();
        owned
            .try_reserve_exact(text.len())
            .map_err(|_| NativeResourceError::Capacity)?;
        owned.push_str(text);
        Ok(owned)
    }
}
