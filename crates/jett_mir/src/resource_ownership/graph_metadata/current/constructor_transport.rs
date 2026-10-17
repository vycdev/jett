//! Forward transport through the existing independently sealed constructor lifecycle.
use super::*;

#[derive(Debug, Clone)]
pub(in crate::resource_ownership::graph_metadata) struct ConstructorOutput {
    pub(in crate::resource_ownership::graph_metadata) constructor: ResourceConstructorId,
    pub(super) original: Expression,
    pub(in crate::resource_ownership::graph_metadata) current: Local,
    pub(in crate::resource_ownership::graph_metadata) proof: GraphStorageProof,
}
impl PartialEq for ConstructorOutput {
    fn eq(&self, other: &Self) -> bool {
        self.constructor == other.constructor
            && equal(&self.original, &other.original)
            && self.current == other.current
            && self.proof == other.proof
    }
}

pub(super) fn outputs(
    values: &[ValueRow],
    constructors: &[ResourceConstructorRegion],
    function: &Function,
) -> Result<Vec<ConstructorOutput>, String> {
    let mut outputs = Vec::new();
    for constructor in constructors {
        let Some(value) = values
            .iter()
            .find(|value| equal(&value.original, constructor.original()))
        else {
            continue;
        };
        constructor.current(function)?;
        let (_, output) = constructor
            .commit()
            .ok_or("graph constructor lost its sealed Commit")?;
        if value.proof.ty != output.ty {
            return Err("graph constructor output changed its rooted producer type".into());
        }
        outputs.push(ConstructorOutput {
            constructor: constructor.id(),
            original: constructor.original().clone(),
            current: output.clone(),
            proof: value.proof.clone(),
        });
    }
    Ok(outputs)
}

pub(super) struct Transport<'a> {
    pub(super) witness: &'a Witness,
    pub(super) constructors: &'a [ResourceConstructorRegion],
    pub(super) function: &'a Function,
}
impl Transport<'_> {
    fn local(&self, original: LocalId) -> Result<LocalId, String> {
        self.witness
            .local_map
            .iter()
            .find(|(header, _)| header.id == original)
            .map(|(_, current)| current.id)
            .ok_or_else(|| "graph transport lost its exact original/current local".into())
    }

    fn constructor(&self, output: &ConstructorOutput) -> Result<Expression, String> {
        let constructor = self
            .constructors
            .get(output.constructor.index())
            .filter(|constructor| constructor.id() == output.constructor)
            .ok_or("graph transport lost its independently sealed constructor")?;
        constructor.current(self.function)?;
        if !equal(constructor.original(), &output.original)
            || constructor
                .commit()
                .is_none_or(|(_, current)| current != &output.current)
            || output.proof.ty != output.current.ty
        {
            return Err("graph transport changed its exact constructor producer/output".into());
        }
        for field in constructor.fields() {
            if !equal(&self.expression(field.original())?, field.endpoint()) {
                return Err("graph transport changed its exact constructor field endpoint".into());
            }
        }
        Ok(Expression {
            kind: E::Local(output.current.id),
            ty: output.original.ty,
            span: output.original.span,
        })
    }

    pub(super) fn expression(&self, value: &Expression) -> Result<Expression, String> {
        let mut matches = self
            .witness
            .constructor_outputs
            .iter()
            .filter(|output| equal(&output.original, value));
        if let Some(output) = matches.next() {
            if matches.next().is_some() {
                return Err("graph transport has duplicate original constructor producers".into());
            }
            return self.constructor(output);
        }
        let mut result = value.clone();
        let ownership = match &mut result.kind {
            E::Call { ownership, .. }
            | E::IndirectCall { ownership, .. }
            | E::Intrinsic { ownership, .. }
            | E::ResourceInvoke { ownership, .. } => Some(ownership),
            _ => None,
        };
        if let Some(ownership) = ownership {
            ownership.remap_metadata_locals(|local| self.local(local))?;
        }
        match &mut result.kind {
            E::ActorGraphImport(plan) => result.kind = E::Local(self.local(plan.output())?),
            E::Local(local) => *local = self.local(*local)?,
            E::Clone(inner)
            | E::View(inner)
            | E::OptionalSome(inner)
            | E::ResultOk(inner)
            | E::ResultFail(inner) => **inner = self.expression(inner)?,
            E::Field { base, .. } => **base = self.expression(base)?,
            E::StructConstruct { fields, .. } | E::BitfieldConstruct { fields, .. } => {
                for field in fields {
                    *field = self.expression(field)?;
                }
            }
            E::ListConstruct { elements } => {
                for element in elements {
                    *element = self.expression(element)?;
                }
            }
            E::MapConstruct { entries } => {
                for entry in entries {
                    entry.key = self.expression(&entry.key)?;
                    entry.value = self.expression(&entry.value)?;
                }
            }
            E::Call { args, .. } | E::Intrinsic { args, .. } | E::ActorGraphStep { args, .. } => {
                for argument in args {
                    *argument = self.expression(argument)?;
                }
            }
            E::IndirectCall { callee, args, .. } => {
                **callee = self.expression(callee)?;
                for argument in args {
                    *argument = self.expression(argument)?;
                }
            }
            E::ActorMessage { actor, args, .. } => {
                **actor = self.expression(actor)?;
                for argument in args {
                    *argument = self.expression(argument)?;
                }
            }
            _ => {}
        }
        Ok(result)
    }
}
