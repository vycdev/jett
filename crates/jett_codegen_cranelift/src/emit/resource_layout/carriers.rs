//! V3 typed graph projection from a fresh constructor-owned carrier plan.
//! Serialized identities are consistency metadata, never Source/Scope authority.
use super::{BYTE_LIMIT, LIMIT, count, pending, word, words};
use crate::CodegenError;
use jett_mir::{ResourceCarrierFunctionPlan, ResourceCarrierNode, ResourceCarrierShapeId};
use jett_types::{TypeId, TypeInterner};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Default)]
pub(super) struct CarrierGraphRows {
    rows: Vec<Vec<u8>>,
    /// Canonical TypeIds are only keys within the already authenticated session.
    ids: BTreeMap<u32, u32>,
    building: BTreeSet<u32>,
}

impl CarrierGraphRows {
    pub(super) fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
    pub(super) fn len(&self) -> usize {
        self.rows.len()
    }
    pub(super) fn rows(&self) -> &[Vec<u8>] {
        &self.rows
    }
    pub(super) fn node_for_type(&self, ty: TypeId) -> Option<u32> {
        self.ids.get(&ty.index()).copied()
    }

    /// Project only nodes reachable from original carrier slots/formals/results.
    /// Unrelated ordinary signature/never metadata does not acquire a carrier ABI.
    pub(super) fn project(
        &mut self,
        plan: &ResourceCarrierFunctionPlan,
        types: &TypeInterner,
        id: ResourceCarrierShapeId,
        ordinary_shape: &mut impl FnMut(TypeId) -> Result<u32, CodegenError>,
        predicate: &mut impl FnMut(TypeId) -> Result<Vec<u8>, CodegenError>,
    ) -> Result<u32, CodegenError> {
        let shape = plan
            .shape(id)
            .ok_or_else(|| pending("carrier graph lost its exact node"))?;
        let ty = shape.ty();
        if let Some(row) = self.ids.get(&ty.index()).copied() {
            return Ok(row);
        }
        if self.rows.len() >= LIMIT || !self.building.insert(ty.index()) {
            return Err(pending(
                "carrier graph needs its exact finite recursive transport",
            ));
        }
        let node = shape.node().clone();
        let mut data = Vec::new();
        let result = (|| {
            match node {
                ResourceCarrierNode::Ordinary => words(&mut data, &[1, ordinary_shape(ty)?]),
                ResourceCarrierNode::Resource { kind } => words(&mut data, &[2, kind.id().index()]),
                ResourceCarrierNode::Optional { payload } => {
                    let child = self.project(plan, types, payload, ordinary_shape, predicate)?;
                    words(&mut data, &[3, child]);
                }
                ResourceCarrierNode::Result { success, failure } => {
                    let ok = self.project(plan, types, success, ordinary_shape, predicate)?;
                    let fail = self.project(plan, types, failure, ordinary_shape, predicate)?;
                    words(&mut data, &[4, ok, fail]);
                }
                ResourceCarrierNode::List { element } => {
                    let child = self.project(plan, types, element, ordinary_shape, predicate)?;
                    words(&mut data, &[5, child]);
                }
                ResourceCarrierNode::Map { key, value } => {
                    let key = self.project(plan, types, key, ordinary_shape, predicate)?;
                    let value = self.project(plan, types, value, ordinary_shape, predicate)?;
                    words(&mut data, &[6, key, value]);
                }
                ResourceCarrierNode::Struct {
                    arguments, fields, ..
                } => {
                    word(&mut data, 7);
                    identity(&mut data, ty, types)?;
                    self.arguments(
                        &mut data,
                        plan,
                        types,
                        &arguments,
                        ordinary_shape,
                        predicate,
                    )?;
                    self.fields(&mut data, plan, types, &fields, ordinary_shape, predicate)?;
                }
                ResourceCarrierNode::Enum {
                    arguments,
                    variants,
                    ..
                } => {
                    word(&mut data, 8);
                    identity(&mut data, ty, types)?;
                    self.arguments(
                        &mut data,
                        plan,
                        types,
                        &arguments,
                        ordinary_shape,
                        predicate,
                    )?;
                    word(&mut data, count(variants.len())?);
                    for (index, variant) in variants.iter().enumerate() {
                        if variant.ordinal != index {
                            return Err(pending(
                                "carrier enum changed its original ordered variant",
                            ));
                        }
                        blob(&mut data, variant.name.as_bytes())?;
                        data.extend_from_slice(&variant.discriminant.to_le_bytes());
                        self.fields(
                            &mut data,
                            plan,
                            types,
                            &variant.fields,
                            ordinary_shape,
                            predicate,
                        )?;
                    }
                }
                ResourceCarrierNode::Machine {
                    states,
                    transitions,
                    ..
                } => {
                    word(&mut data, 9);
                    identity(&mut data, ty, types)?;
                    word(&mut data, count(states.len())?);
                    for (index, state) in states.iter().enumerate() {
                        if state.ordinal != index {
                            return Err(pending(
                                "carrier machine changed its original ordered state",
                            ));
                        }
                        blob(&mut data, state.name.as_bytes())?;
                        data.extend_from_slice(
                            &i64::try_from(index)
                                .map_err(|_| {
                                    pending("carrier state ordinal exceeds checked tag width")
                                })?
                                .to_le_bytes(),
                        );
                        self.fields(
                            &mut data,
                            plan,
                            types,
                            &state.fields,
                            ordinary_shape,
                            predicate,
                        )?;
                    }
                    word(&mut data, count(transitions.len())?);
                    for (source, target) in transitions {
                        words(&mut data, &[count(source)?, count(target)?]);
                    }
                }
                ResourceCarrierNode::MachineState { machine, state } => {
                    let owner = self.project(plan, types, machine, ordinary_shape, predicate)?;
                    words(&mut data, &[10, owner, count(state)?]);
                }
                ResourceCarrierNode::Refinement { base, .. } => {
                    let base = self.project(plan, types, base, ordinary_shape, predicate)?;
                    word(&mut data, 11);
                    identity(&mut data, ty, types)?;
                    word(&mut data, base);
                    // This callback selects the exact authenticated predicate
                    // association. A type name or numerically valid base is not proof.
                    let original = predicate(ty)?;
                    if original.is_empty() {
                        return Err(pending(
                            "carrier refinement lost its original predicate association",
                        ));
                    }
                    blob(&mut data, &original)?;
                }
            }
            if data.len() > BYTE_LIMIT {
                return Err(pending("carrier graph row exceeds wire capacity"));
            }
            Ok(())
        })();
        self.building.remove(&ty.index());
        result?;
        let ordinal = count(self.rows.len())?;
        self.rows.push(data);
        self.ids.insert(ty.index(), ordinal);
        Ok(ordinal)
    }

    fn arguments(
        &mut self,
        out: &mut Vec<u8>,
        plan: &ResourceCarrierFunctionPlan,
        types: &TypeInterner,
        arguments: &[ResourceCarrierShapeId],
        ordinary: &mut impl FnMut(TypeId) -> Result<u32, CodegenError>,
        predicate: &mut impl FnMut(TypeId) -> Result<Vec<u8>, CodegenError>,
    ) -> Result<(), CodegenError> {
        word(out, count(arguments.len())?);
        for argument in arguments {
            word(
                out,
                self.project(plan, types, *argument, ordinary, predicate)?,
            );
        }
        Ok(())
    }

    fn fields(
        &mut self,
        out: &mut Vec<u8>,
        plan: &ResourceCarrierFunctionPlan,
        types: &TypeInterner,
        fields: &[jett_mir::ResourceCarrierField],
        ordinary: &mut impl FnMut(TypeId) -> Result<u32, CodegenError>,
        predicate: &mut impl FnMut(TypeId) -> Result<Vec<u8>, CodegenError>,
    ) -> Result<(), CodegenError> {
        word(out, count(fields.len())?);
        for (index, field) in fields.iter().enumerate() {
            if field.ordinal != index {
                return Err(pending("carrier graph changed its original ordered field"));
            }
            blob(out, field.name.as_bytes())?;
            word(
                out,
                self.project(plan, types, field.shape, ordinary, predicate)?,
            );
        }
        Ok(())
    }
}

fn blob(out: &mut Vec<u8>, value: &[u8]) -> Result<(), CodegenError> {
    if value.len() > LIMIT {
        return Err(pending("carrier identity exceeds bounded wire blob domain"));
    }
    word(out, count(value.len())?);
    out.extend_from_slice(value);
    Ok(())
}

fn identity(out: &mut Vec<u8>, ty: TypeId, types: &TypeInterner) -> Result<(), CodegenError> {
    // This stamp is emitted only from the authenticated graph. The runtime binds
    // it to its installed node and issues constructor identities independently.
    let mut bytes = ty.index().to_le_bytes().to_vec();
    bytes.extend_from_slice(types.type_name(ty).as_bytes());
    blob(out, &bytes)
}

pub(super) fn append_extension(
    out: &mut Vec<u8>,
    graph: &CarrierGraphRows,
    slots: &[Vec<u32>],
    operations: &[Vec<u8>],
) -> Result<(), CodegenError> {
    out.extend_from_slice(b"JTCAR003");
    words(
        out,
        &[
            count(graph.len())?,
            count(slots.len())?,
            count(operations.len())?,
            0,
        ],
    );
    for (index, node) in graph.rows().iter().enumerate() {
        word(out, count(index)?);
        out.extend_from_slice(node);
    }
    for (index, slot) in slots.iter().enumerate() {
        word(out, count(index)?);
        words(out, slot);
    }
    for (index, operation) in operations.iter().enumerate() {
        word(out, count(index)?);
        out.extend_from_slice(operation);
    }
    if out.len() > BYTE_LIMIT {
        return Err(pending("carrier extension exceeds bounded wire capacity"));
    }
    Ok(())
}
