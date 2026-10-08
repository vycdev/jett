//! Explicit v3 suffix codec; old records are never reinterpreted.
use super::super::carriers::*;
use super::*;
const MAGIC: &[u8; 8] = b"JTCAR003";
fn blob(r: &mut Reader<'_>) -> Result<Vec<u8>, ResourceLayoutError> {
    let n = r.count()?;
    let end = r
        .position
        .checked_add(n)
        .ok_or(ResourceLayoutError::WireLimit)?;
    let bytes = r
        .bytes
        .get(r.position..end)
        .ok_or(ResourceLayoutError::Truncated)?;
    let mut out = storage(n)?;
    out.extend_from_slice(bytes);
    r.position = end;
    Ok(out)
}
fn ids(r: &mut Reader<'_>) -> Result<Vec<u32>, ResourceLayoutError> {
    let n = r.count()?;
    let mut v = storage(n)?;
    for _ in 0..n {
        v.push(r.word()?);
    }
    Ok(v)
}
fn fields(r: &mut Reader<'_>) -> Result<Vec<NativeCarrierField>, ResourceLayoutError> {
    let n = r.count()?;
    let mut v = storage(n)?;
    for _ in 0..n {
        v.push(NativeCarrierField {
            name: blob(r)?,
            node: r.word()?,
        });
    }
    Ok(v)
}
fn members(r: &mut Reader<'_>) -> Result<Vec<NativeCarrierMember>, ResourceLayoutError> {
    let n = r.count()?;
    let mut v = storage(n)?;
    for _ in 0..n {
        v.push(NativeCarrierMember {
            name: blob(r)?,
            discriminant: i64::from_le_bytes(r.take()?),
            fields: fields(r)?,
        });
    }
    Ok(v)
}
pub(super) fn loan(r: &mut Reader<'_>) -> Result<NativeCarrierLoanSource, ResourceLayoutError> {
    Ok(match r.word()? {
        1 => NativeCarrierLoanSource::ExistingBorrow {
            operation: r.word()?,
        },
        2 => NativeCarrierLoanSource::IncomingViewFormal {
            scope: r.word()?,
            parameter: r.word()?,
        },
        _ => return Err(ResourceLayoutError::UnknownTag),
    })
}
fn source(r: &mut Reader<'_>) -> Result<NativeCarrierSource, ResourceLayoutError> {
    Ok(match r.word()? {
        1 => NativeCarrierSource::Slot(r.word()?),
        2 => NativeCarrierSource::Loan(loan(r)?),
        _ => return Err(ResourceLayoutError::UnknownTag),
    })
}
fn selector(r: &mut Reader<'_>) -> Result<NativeCarrierSelector, ResourceLayoutError> {
    Ok(match r.word()? {
        0 => NativeCarrierSelector::Static(r.word()?),
        1 => NativeCarrierSelector::Dynamic,
        _ => return Err(ResourceLayoutError::UnknownTag),
    })
}
fn path(r: &mut Reader<'_>) -> Result<Vec<NativeCarrierPath>, ResourceLayoutError> {
    let n = r.count()?;
    let mut v = storage(n)?;
    for _ in 0..n {
        v.push(match r.word()? {
            1 => NativeCarrierPath::Field(r.word()?),
            2 => NativeCarrierPath::EnumPayload {
                variant: r.word()?,
                field: r.word()?,
            },
            3 => NativeCarrierPath::MachinePayload {
                state: r.word()?,
                field: r.word()?,
            },
            4 => NativeCarrierPath::List(selector(r)?),
            5 => NativeCarrierPath::MapKey(selector(r)?),
            6 => NativeCarrierPath::MapValue(selector(r)?),
            7 => NativeCarrierPath::Some,
            8 => NativeCarrierPath::Ok,
            9 => NativeCarrierPath::Fail,
            _ => return Err(ResourceLayoutError::UnknownTag),
        });
    }
    Ok(v)
}
fn flag(r: &mut Reader<'_>) -> Result<bool, ResourceLayoutError> {
    match r.word()? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(ResourceLayoutError::Reserved),
    }
}
fn operation(r: &mut Reader<'_>) -> Result<NativeCarrierOperation, ResourceLayoutError> {
    let tag = r.word()?;
    let frame = r.word()?;
    Ok(match tag {
        1 => {
            let destination = r.word()?;
            let selector = r.word()?;
            let n = r.count()?;
            let mut children = storage(n)?;
            for _ in 0..n {
                let index = r.word()?;
                let node = r.word()?;
                let source = match r.word()? {
                    1 => NativeCarrierChildSource::Ordinary { shape: r.word()? },
                    2 => NativeCarrierChildSource::Move { slot: r.word()? },
                    3 => NativeCarrierChildSource::LeafSum { slot: r.word()? },
                    _ => return Err(ResourceLayoutError::UnknownTag),
                };
                children.push(NativeCarrierChild {
                    index,
                    node,
                    source,
                });
            }
            NativeCarrierOperation::Construct {
                frame,
                destination,
                selector,
                children,
            }
        }
        2 => NativeCarrierOperation::Transfer {
            frame,
            source: r.word()?,
            destination: r.word()?,
        },
        3 => NativeCarrierOperation::Borrow {
            frame,
            source: source(r)?,
            path: path(r)?,
            node: r.word()?,
            lease_frame: r.word()?,
        },
        4 => NativeCarrierOperation::EndBorrow {
            frame,
            borrow: r.word()?,
        },
        5 => {
            let source = source(r)?;
            let path = path(r)?;
            let observation = match r.word()? {
                1 => NativeCarrierObservation::Length,
                2 => NativeCarrierObservation::Tag,
                3 => NativeCarrierObservation::State,
                4 => NativeCarrierObservation::Ordinary {
                    shape: r.word()?,
                    clone: flag(r)?,
                },
                _ => return Err(ResourceLayoutError::UnknownTag),
            };
            NativeCarrierOperation::Observe {
                frame,
                source,
                path,
                observation,
            }
        }
        6 => {
            let source = r.word()?;
            let path = path(r)?;
            let destination = match r.word()? {
                1 => NativeCarrierDestination::Slot(r.word()?),
                2 => NativeCarrierDestination::Ordinary { shape: r.word()? },
                _ => return Err(ResourceLayoutError::UnknownTag),
            };
            NativeCarrierOperation::Extract {
                frame,
                source,
                path,
                destination,
            }
        }
        7 => NativeCarrierOperation::AdaptSum {
            frame,
            source: source(r)?,
            path: path(r)?,
            sum_slot: r.word()?,
            lease_frame: r.word()?,
            borrowed: flag(r)?,
        },
        8 => NativeCarrierOperation::Retire {
            frame,
            source: r.word()?,
        },
        9 => NativeCarrierOperation::PublishReturn {
            frame,
            source: r.word()?,
        },
        10 => NativeCarrierOperation::ResetIteration {
            frame,
            source: r.word()?,
            carrier_binders: ids(r)?,
            leaf_sum_binders: ids(r)?,
        },
        11 => NativeCarrierOperation::QualifyMachine {
            frame,
            source: r.word()?,
            destination: r.word()?,
            machine: r.word()?,
            state: r.word()?,
        },
        _ => return Err(ResourceLayoutError::UnknownTag),
    })
}
pub(super) fn decode(r: &mut Reader<'_>) -> Result<NativeCarrierLayout, ResourceLayoutError> {
    if &r.take::<8>()? != MAGIC {
        return Err(ResourceLayoutError::Header);
    }
    let ns = r.count()?;
    let ss = r.count()?;
    let os = r.count()?;
    if r.word()? != 0 {
        return Err(ResourceLayoutError::Reserved);
    }
    let mut nodes = storage(ns)?;
    for i in 0..ns {
        r.ordinal(i)?;
        nodes.push(match r.word()? {
            1 => NativeCarrierNode::Ordinary { shape: r.word()? },
            2 => NativeCarrierNode::Resource { kind: r.word()? },
            3 => NativeCarrierNode::Optional { child: r.word()? },
            4 => NativeCarrierNode::Result {
                ok: r.word()?,
                fail: r.word()?,
            },
            5 => NativeCarrierNode::List { element: r.word()? },
            6 => NativeCarrierNode::Map {
                key: r.word()?,
                value: r.word()?,
            },
            7 => NativeCarrierNode::Struct {
                identity: blob(r)?,
                arguments: ids(r)?,
                fields: fields(r)?,
            },
            8 => NativeCarrierNode::Enum {
                identity: blob(r)?,
                arguments: ids(r)?,
                variants: members(r)?,
            },
            9 => {
                let identity = blob(r)?;
                let states = members(r)?;
                let n = r.count()?;
                let mut transitions = storage(n)?;
                for _ in 0..n {
                    transitions.push((r.word()?, r.word()?));
                }
                NativeCarrierNode::Machine {
                    identity,
                    states,
                    transitions,
                }
            }
            10 => NativeCarrierNode::MachineState {
                machine: r.word()?,
                state: r.word()?,
            },
            11 => NativeCarrierNode::Refinement {
                identity: blob(r)?,
                base: r.word()?,
                predicate: blob(r)?,
            },
            _ => return Err(ResourceLayoutError::UnknownTag),
        });
    }
    let mut slots = storage(ss)?;
    for i in 0..ss {
        r.ordinal(i)?;
        slots.push(NativeCarrierSlot {
            frame: r.word()?,
            node: r.word()?,
        });
    }
    let mut operations = storage(os)?;
    for i in 0..os {
        r.ordinal(i)?;
        operations.push(NativeCarrierOperationRecord {
            site: r.site()?,
            operation: operation(r)?,
        });
    }
    Ok(NativeCarrierLayout {
        nodes,
        slots,
        operations,
    })
}
#[cfg(test)]
fn word(v: &mut Vec<u8>, n: u32) {
    v.extend_from_slice(&n.to_le_bytes());
}
#[cfg(test)]
fn blob_out(v: &mut Vec<u8>, b: &[u8]) {
    word(v, b.len() as u32);
    v.extend_from_slice(b);
}
#[cfg(test)]
fn ids_out(v: &mut Vec<u8>, ids: &[u32]) {
    word(v, ids.len() as u32);
    for n in ids {
        word(v, *n);
    }
}
#[cfg(test)]
fn fields_out(v: &mut Vec<u8>, fs: &[NativeCarrierField]) {
    word(v, fs.len() as u32);
    for f in fs {
        blob_out(v, &f.name);
        word(v, f.node);
    }
}
#[cfg(test)]
fn members_out(v: &mut Vec<u8>, ms: &[NativeCarrierMember]) {
    word(v, ms.len() as u32);
    for m in ms {
        blob_out(v, &m.name);
        v.extend_from_slice(&m.discriminant.to_le_bytes());
        fields_out(v, &m.fields);
    }
}
#[cfg(test)]
pub(super) fn loan_out(v: &mut Vec<u8>, s: NativeCarrierLoanSource) {
    match s {
        NativeCarrierLoanSource::ExistingBorrow { operation } => {
            word(v, 1);
            word(v, operation);
        }
        NativeCarrierLoanSource::IncomingViewFormal { scope, parameter } => {
            word(v, 2);
            word(v, scope);
            word(v, parameter);
        }
    }
}
#[cfg(test)]
fn source_out(v: &mut Vec<u8>, s: NativeCarrierSource) {
    match s {
        NativeCarrierSource::Slot(slot) => {
            word(v, 1);
            word(v, slot);
        }
        NativeCarrierSource::Loan(s) => {
            word(v, 2);
            loan_out(v, s);
        }
    }
}
#[cfg(test)]
fn selector_out(v: &mut Vec<u8>, s: NativeCarrierSelector) {
    match s {
        NativeCarrierSelector::Static(i) => {
            word(v, 0);
            word(v, i);
        }
        NativeCarrierSelector::Dynamic => word(v, 1),
    }
}
#[cfg(test)]
fn path_out(v: &mut Vec<u8>, ps: &[NativeCarrierPath]) {
    word(v, ps.len() as u32);
    for p in ps {
        match *p {
            NativeCarrierPath::Field(i) => {
                word(v, 1);
                word(v, i);
            }
            NativeCarrierPath::EnumPayload { variant, field } => {
                word(v, 2);
                word(v, variant);
                word(v, field);
            }
            NativeCarrierPath::MachinePayload { state, field } => {
                word(v, 3);
                word(v, state);
                word(v, field);
            }
            NativeCarrierPath::List(s) => {
                word(v, 4);
                selector_out(v, s);
            }
            NativeCarrierPath::MapKey(s) => {
                word(v, 5);
                selector_out(v, s);
            }
            NativeCarrierPath::MapValue(s) => {
                word(v, 6);
                selector_out(v, s);
            }
            NativeCarrierPath::Some => word(v, 7),
            NativeCarrierPath::Ok => word(v, 8),
            NativeCarrierPath::Fail => word(v, 9),
        }
    }
}
#[cfg(test)]
pub(super) fn encode(g: &NativeCarrierLayout) -> Vec<u8> {
    let mut v = MAGIC.to_vec();
    for n in [g.nodes.len(), g.slots.len(), g.operations.len()] {
        word(&mut v, n as u32);
    }
    word(&mut v, 0);
    for (i, n) in g.nodes.iter().enumerate() {
        word(&mut v, i as u32);
        match n {
            NativeCarrierNode::Ordinary { shape } => {
                word(&mut v, 1);
                word(&mut v, *shape);
            }
            NativeCarrierNode::Resource { kind } => {
                word(&mut v, 2);
                word(&mut v, *kind);
            }
            NativeCarrierNode::Optional { child } => {
                word(&mut v, 3);
                word(&mut v, *child);
            }
            NativeCarrierNode::Result { ok, fail } => {
                word(&mut v, 4);
                word(&mut v, *ok);
                word(&mut v, *fail);
            }
            NativeCarrierNode::List { element } => {
                word(&mut v, 5);
                word(&mut v, *element);
            }
            NativeCarrierNode::Map { key, value } => {
                word(&mut v, 6);
                word(&mut v, *key);
                word(&mut v, *value);
            }
            NativeCarrierNode::Struct {
                identity,
                arguments,
                fields,
            } => {
                word(&mut v, 7);
                blob_out(&mut v, identity);
                ids_out(&mut v, arguments);
                fields_out(&mut v, fields);
            }
            NativeCarrierNode::Enum {
                identity,
                arguments,
                variants,
            } => {
                word(&mut v, 8);
                blob_out(&mut v, identity);
                ids_out(&mut v, arguments);
                members_out(&mut v, variants);
            }
            NativeCarrierNode::Machine {
                identity,
                states,
                transitions,
            } => {
                word(&mut v, 9);
                blob_out(&mut v, identity);
                members_out(&mut v, states);
                word(&mut v, transitions.len() as u32);
                for (a, b) in transitions {
                    word(&mut v, *a);
                    word(&mut v, *b);
                }
            }
            NativeCarrierNode::MachineState { machine, state } => {
                word(&mut v, 10);
                word(&mut v, *machine);
                word(&mut v, *state);
            }
            NativeCarrierNode::Refinement {
                identity,
                base,
                predicate,
            } => {
                word(&mut v, 11);
                blob_out(&mut v, identity);
                word(&mut v, *base);
                blob_out(&mut v, predicate);
            }
        }
    }
    for (i, s) in g.slots.iter().enumerate() {
        word(&mut v, i as u32);
        word(&mut v, s.frame);
        word(&mut v, s.node);
    }
    for (i, row) in g.operations.iter().enumerate() {
        word(&mut v, i as u32);
        word(&mut v, row.site.function);
        word(&mut v, row.site.block);
        match row.site.position {
            NativePosition::Statement(i) => {
                word(&mut v, 0);
                word(&mut v, i);
            }
            NativePosition::Terminator => {
                word(&mut v, 1);
                word(&mut v, 0);
            }
        }
        let op = &row.operation;
        let tag = match op {
            NativeCarrierOperation::Construct { .. } => 1,
            NativeCarrierOperation::Transfer { .. } => 2,
            NativeCarrierOperation::Borrow { .. } => 3,
            NativeCarrierOperation::EndBorrow { .. } => 4,
            NativeCarrierOperation::Observe { .. } => 5,
            NativeCarrierOperation::Extract { .. } => 6,
            NativeCarrierOperation::AdaptSum { .. } => 7,
            NativeCarrierOperation::Retire { .. } => 8,
            NativeCarrierOperation::PublishReturn { .. } => 9,
            NativeCarrierOperation::ResetIteration { .. } => 10,
            NativeCarrierOperation::QualifyMachine { .. } => 11,
        };
        word(&mut v, tag);
        word(&mut v, op.frame());
        match op {
            NativeCarrierOperation::Construct {
                destination,
                selector,
                children,
                ..
            } => {
                word(&mut v, *destination);
                word(&mut v, *selector);
                word(&mut v, children.len() as u32);
                for c in children {
                    word(&mut v, c.index);
                    word(&mut v, c.node);
                    match c.source {
                        NativeCarrierChildSource::Ordinary { shape } => {
                            word(&mut v, 1);
                            word(&mut v, shape);
                        }
                        NativeCarrierChildSource::Move { slot } => {
                            word(&mut v, 2);
                            word(&mut v, slot);
                        }
                        NativeCarrierChildSource::LeafSum { slot } => {
                            word(&mut v, 3);
                            word(&mut v, slot);
                        }
                    }
                }
            }
            NativeCarrierOperation::Transfer {
                source,
                destination,
                ..
            } => {
                word(&mut v, *source);
                word(&mut v, *destination);
            }
            NativeCarrierOperation::Borrow {
                source,
                path,
                node,
                lease_frame,
                ..
            } => {
                source_out(&mut v, *source);
                path_out(&mut v, path);
                word(&mut v, *node);
                word(&mut v, *lease_frame);
            }
            NativeCarrierOperation::EndBorrow { borrow, .. } => word(&mut v, *borrow),
            NativeCarrierOperation::Observe {
                source,
                path,
                observation,
                ..
            } => {
                source_out(&mut v, *source);
                path_out(&mut v, path);
                match observation {
                    NativeCarrierObservation::Length => word(&mut v, 1),
                    NativeCarrierObservation::Tag => word(&mut v, 2),
                    NativeCarrierObservation::State => word(&mut v, 3),
                    NativeCarrierObservation::Ordinary { shape, clone } => {
                        word(&mut v, 4);
                        word(&mut v, *shape);
                        word(&mut v, u32::from(*clone));
                    }
                }
            }
            NativeCarrierOperation::Extract {
                source,
                path,
                destination,
                ..
            } => {
                word(&mut v, *source);
                path_out(&mut v, path);
                match destination {
                    NativeCarrierDestination::Slot(s) => {
                        word(&mut v, 1);
                        word(&mut v, *s);
                    }
                    NativeCarrierDestination::Ordinary { shape } => {
                        word(&mut v, 2);
                        word(&mut v, *shape);
                    }
                }
            }
            NativeCarrierOperation::AdaptSum {
                source,
                path,
                sum_slot,
                lease_frame,
                borrowed,
                ..
            } => {
                source_out(&mut v, *source);
                path_out(&mut v, path);
                word(&mut v, *sum_slot);
                word(&mut v, *lease_frame);
                word(&mut v, u32::from(*borrowed));
            }
            NativeCarrierOperation::Retire { source, .. }
            | NativeCarrierOperation::PublishReturn { source, .. } => word(&mut v, *source),
            NativeCarrierOperation::ResetIteration {
                source,
                carrier_binders,
                leaf_sum_binders,
                ..
            } => {
                word(&mut v, *source);
                ids_out(&mut v, carrier_binders);
                ids_out(&mut v, leaf_sum_binders);
            }
            NativeCarrierOperation::QualifyMachine {
                source,
                destination,
                machine,
                state,
                ..
            } => {
                word(&mut v, *source);
                word(&mut v, *destination);
                word(&mut v, *machine);
                word(&mut v, *state);
            }
        }
    }
    v
}
