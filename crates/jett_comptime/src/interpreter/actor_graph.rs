//! Immutable checked actor snapshots. No Source or provider runs during import.
use super::{ActorInstance, Interpreter, Value};
use crate::resource_execution::PreparedRequiredExpression;
use jett_parser::ast::TypeExpr;
use jett_typecheck::CheckedResourceProgram;
use jett_types::{Type, TypeId, TypeInterner};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

#[derive(Debug, Clone)]
pub struct FrozenActorField {
    name: String,
    ty: TypeId,
    value: Value,
}

impl FrozenActorField {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn ty(&self) -> TypeId {
        self.ty
    }
    pub fn value(&self) -> &Value {
        &self.value
    }
}

#[derive(Debug, Clone)]
pub struct FrozenActorNode {
    seed: u64,
    ty: TypeId,
    captures: Vec<FrozenActorField>,
    state: Vec<FrozenActorField>,
}

impl FrozenActorNode {
    pub fn seed(&self) -> u64 {
        self.seed
    }
    pub fn ty(&self) -> TypeId {
        self.ty
    }
    pub fn captures(&self) -> &[FrozenActorField] {
        &self.captures
    }
    pub fn state(&self) -> &[FrozenActorField] {
        &self.state
    }
}

/// Private construction and exact checked-program binding. The IDs in this
/// packet name seed nodes only; they are never runtime identities or grants.
#[derive(Debug, Clone)]
pub struct ActorGraphTemplate {
    program: Arc<CheckedResourceProgram>,
    root: Value,
    root_type: TypeId,
    nodes: Vec<FrozenActorNode>,
}

impl ActorGraphTemplate {
    pub fn belongs_to(&self, program: &Arc<CheckedResourceProgram>) -> bool {
        Arc::ptr_eq(&self.program, program)
    }
    pub fn root(&self) -> &Value {
        &self.root
    }
    pub fn root_type(&self) -> TypeId {
        self.root_type
    }
    pub fn nodes(&self) -> &[FrozenActorNode] {
        &self.nodes
    }
}

impl Interpreter {
    pub(crate) fn freeze_actor_graph(
        &self,
        root: &Value,
        prepared: Option<&PreparedRequiredExpression>,
    ) -> Result<Option<Arc<ActorGraphTemplate>>, String> {
        if !contains_actor(root) {
            return Ok(None);
        }
        let prepared = prepared.ok_or("actor snapshot has no exact checked required occurrence")?;
        let program = prepared.checked_program();
        if !self
            .retained_resource_program
            .as_ref()
            .is_some_and(|(current, _)| Arc::ptr_eq(current, program))
        {
            return Err("actor snapshot belongs to a foreign checked program".into());
        }
        let types = &program.checked().interner;
        let mut pending = vec![(root, prepared.result_type())];
        let mut seen: HashMap<u64, usize> = HashMap::new();
        let mut nodes: Vec<FrozenActorNode> = Vec::new();
        while let Some((value, ty)) = pending.pop() {
            if let (Type::Actor(owner), Value::Actor(seed)) = (types.resolve(ty), value) {
                let actor = self
                    .actor_instances
                    .get(seed)
                    .ok_or("actor snapshot has no live seed instance")?;
                let definition = types.resolve_actor(*owner);
                if actor.type_name != definition.name {
                    return Err("actor snapshot changed its exact checked nominal owner".into());
                }
                if let Some(index) = seen.get(seed) {
                    if nodes[*index].ty != ty {
                        return Err(
                            "actor graph seed was reused under another checked nominal actor type"
                                .into(),
                        );
                    }
                    continue;
                }
                seen.insert(*seed, nodes.len());
                let captures = frozen_fields(&actor.capabilities, &definition.capability_params)?;
                let state = frozen_fields(&actor.state, &definition.state_fields)?;
                nodes.push(FrozenActorNode {
                    seed: *seed,
                    ty,
                    captures,
                    state,
                });
                // Reverse pushes preserve checked declaration order at visitation.
                for (name, field_type) in definition.state_fields.iter().rev() {
                    pending.push((
                        actor
                            .state
                            .get(name)
                            .ok_or("snapshot state field is missing")?,
                        *field_type,
                    ));
                }
                for (name, field_type) in definition.capability_params.iter().rev() {
                    pending.push((
                        actor
                            .capabilities
                            .get(name)
                            .ok_or("snapshot capture is missing")?,
                        *field_type,
                    ));
                }
            } else {
                let children = self.actor_graph_children(value, ty, types, program)?;
                pending.extend(children.into_iter().rev());
            }
        }
        // Authenticated evaluator IDs encode seed creation order. Preserve
        // relative debug ordinals without replaying constructors or importing IDs.
        nodes.sort_by_key(|node| node.seed);
        Ok(Some(Arc::new(ActorGraphTemplate {
            program: program.clone(),
            root: root.clone(),
            root_type: prepared.result_type(),
            nodes,
        })))
    }

    /// Prepare every owner off-registry, then publish only the complete graph.
    /// Numeric reservations are retired on failure rather than reused.
    pub(crate) fn instantiate_actor_graph(
        &mut self,
        template: &ActorGraphTemplate,
    ) -> Result<Value, String> {
        let (program, _) = self
            .retained_resource_program
            .as_ref()
            .ok_or("actor import has no checked program")?;
        if !template.belongs_to(program) {
            return Err("actor import belongs to a foreign checked program".into());
        }
        let count = u64::try_from(template.nodes.len())
            .map_err(|_| "actor graph exceeds identity space")?;
        let limit = self
            .next_actor_id
            .checked_add(count)
            .ok_or("actor identity space exhausted")?;
        let first = self.next_actor_id;
        let mut identities = HashMap::new();
        identities
            .try_reserve(template.nodes.len())
            .map_err(|_| "cannot reserve actor import identities")?;
        for (offset, node) in template.nodes.iter().enumerate() {
            let offset =
                u64::try_from(offset).map_err(|_| "actor graph index exceeds identity space")?;
            let fresh = first
                .checked_add(offset)
                .ok_or("actor identity space exhausted")?;
            if self.actor_instances.contains_key(&fresh)
                || identities.insert(node.seed, fresh).is_some()
            {
                return Err("actor graph import has a duplicate or occupied identity".into());
            }
        }
        self.actor_instances
            .try_reserve(template.nodes.len())
            .map_err(|_| "cannot reserve actor registry")?;
        self.next_actor_id = limit;
        let mut unpublished = Vec::new();
        unpublished
            .try_reserve_exact(template.nodes.len())
            .map_err(|_| "cannot reserve unpublished actor graph")?;
        for node in &template.nodes {
            let Type::Actor(owner) = program.checked().interner.resolve(node.ty) else {
                return Err("actor graph node has no checked actor type".into());
            };
            let definition = program.checked().interner.resolve_actor(*owner);
            let state = import_fields(&node.state, &identities)?;
            let capabilities = import_fields(&node.captures, &identities)?;
            unpublished.push((
                *identities
                    .get(&node.seed)
                    .ok_or("actor seed is not reserved")?,
                ActorInstance {
                    type_name: definition.name.clone(),
                    state,
                    capabilities,
                },
            ));
        }
        let root = remap_value(template.root.clone(), &identities)?;
        // No fallible operation or Source execution occurs after publication starts.
        for (identity, actor) in unpublished {
            self.actor_instances.insert(identity, actor);
        }
        Ok(root)
    }

    fn actor_graph_children<'a>(
        &'a self,
        value: &'a Value,
        ty: TypeId,
        types: &TypeInterner,
        program: &Arc<CheckedResourceProgram>,
    ) -> Result<Vec<(&'a Value, TypeId)>, String> {
        if let Value::Pending(inner) = value {
            return Ok(vec![(inner, ty)]);
        }
        if let Value::Typed {
            type_name,
            value: inner,
        } = value
        {
            let concrete = checked_type_name(types, type_name)?;
            if let Type::Interface(_) = types.resolve(ty) {
                return Ok(vec![(inner, concrete)]);
            }
            if let Type::Secret(base) = types.resolve(ty) {
                if concrete != ty {
                    return Ok(vec![(value, *base)]);
                }
            }
            if concrete != ty {
                return Err("actor graph typed carrier changed its checked owner".into());
            }
            let inner_type = match types.resolve(ty) {
                Type::Refinement { base, .. } | Type::Secret(base) => *base,
                _ => ty,
            };
            return Ok(vec![(inner, inner_type)]);
        }
        if let Type::Secret(inner) = types.resolve(ty) {
            return Ok(vec![(value, *inner)]);
        }
        if let Type::Refinement { base, .. } = types.resolve(ty) {
            // The value is already provider-free checked data; import never reruns a predicate.
            return Ok(vec![(value, *base)]);
        }
        if let Type::Interface(_) = types.resolve(ty) {
            return Ok(vec![(value, self.actor_graph_concrete_type(value, types)?)]);
        }
        match (types.resolve(ty), value) {
            (_, Value::Capability(_) | Value::Resource(_) | Value::GrantedNetwork(_)) => Err(
                "actor graph cannot retain runtime capability authority or Resource custody".into(),
            ),
            (Type::Function { .. }, Value::ResourceHook(descriptor))
                if descriptor.matches_type(program, ty) =>
            {
                Ok(Vec::new())
            }
            (_, Value::ResourceHook(_)) => {
                Err("actor graph contains a foreign or mistyped checked hook descriptor".into())
            }
            (Type::List(element), Value::List(values))
            | (Type::Set(element), Value::Set(values)) => {
                Ok(values.iter().map(|value| (value, *element)).collect())
            }
            (Type::Map(key, mapped), Value::Map(entries)) => Ok(entries
                .iter()
                .flat_map(|(left, right)| [(left, *key), (right, *mapped)])
                .collect()),
            (Type::Optional(inner), Value::OptionalSome(value)) => Ok(vec![(value, *inner)]),
            (Type::Optional(_), Value::OptionalNone) => Ok(Vec::new()),
            (Type::Result(ok, _), Value::ResultOk(value)) => Ok(vec![(value, *ok)]),
            (Type::Result(_, fail), Value::ResultFail(value)) => Ok(vec![(value, *fail)]),
            (
                Type::Struct(owner),
                Value::Struct {
                    type_name,
                    concrete_type,
                    fields,
                },
            ) => {
                let definition = types.resolve_struct(*owner);
                if definition.name != concrete_type.as_deref().unwrap_or(type_name) {
                    return Err("actor graph record owner mismatch".into());
                }
                named_children(fields, &definition.fields)
            }
            (
                Type::Bitfield(owner),
                Value::Struct {
                    type_name, fields, ..
                },
            ) => {
                let definition = types.resolve_bitfield(*owner);
                if definition.name != *type_name {
                    return Err("actor graph bitfield owner mismatch".into());
                }
                let fields_by_name = definition
                    .fields
                    .iter()
                    .map(|field| (field.name.clone(), field.ty))
                    .collect::<Vec<_>>();
                named_children(fields, &fields_by_name)
            }
            (
                Type::Enum(owner),
                Value::Enum {
                    type_name,
                    variant,
                    fields,
                },
            ) => {
                let definition = types.resolve_enum(*owner);
                if definition.name != *type_name {
                    return Err("actor graph enum owner mismatch".into());
                }
                let variant = definition
                    .variants
                    .iter()
                    .find(|candidate| candidate.name == *variant)
                    .ok_or("actor graph variant is absent")?;
                positional_children(fields, &variant.fields)
            }
            (
                Type::Machine(owner) | Type::MachineState { machine: owner, .. },
                Value::Machine {
                    type_name,
                    state,
                    fields,
                },
            ) => {
                let definition = types.resolve_machine(*owner);
                if definition.name != *type_name {
                    return Err("actor graph machine owner mismatch".into());
                }
                let index = definition
                    .state_id(state)
                    .ok_or("actor graph machine state is absent")?;
                if let Type::MachineState { state: checked, .. } = types.resolve(ty) {
                    if *checked != index {
                        return Err("actor graph machine state mismatch".into());
                    }
                }
                positional_children(
                    fields,
                    &definition
                        .state(index)
                        .ok_or("actor graph machine state is absent")?
                        .fields,
                )
            }
            (
                Type::Function { .. },
                Value::Function {
                    captures,
                    capture_types,
                    type_context,
                    namespace_aliases,
                    namespace,
                    ..
                },
            ) => {
                let mut names = captures.keys().collect::<Vec<_>>();
                names.sort();
                names
                    .into_iter()
                    .map(|name| {
                        let written = capture_types
                            .get(name)
                            .ok_or("actor graph callback capture has no declared type")?;
                        let concrete = self.actor_graph_capture_type(
                            written,
                            &type_context.bindings,
                            namespace_aliases,
                            namespace.as_deref(),
                            &mut HashSet::new(),
                        )?;
                        let name_of_type = self.resolved_concrete_type_display(&concrete);
                        Ok((&captures[name], checked_type_name(types, &name_of_type)?))
                    })
                    .collect()
            }
            (Type::TypeConstruction, Value::TypeConstruction { fields, .. }) => fields
                .iter()
                .map(|(_, _, field_type, value)| Ok((value, checked_type_name(types, field_type)?)))
                .collect(),
            (Type::Function { .. }, Value::ReflectedCallable(adapter)) => {
                if !Arc::ptr_eq(adapter.checked_program(), program)
                    || adapter.requested_type() != ty
                    || !adapter.checked_plan().validate(program.checked())
                {
                    return Err("actor graph callback changed its checked adapter plan".into());
                }
                crate::value::ReflectedCallableValue::authenticate_source(
                    adapter.source(),
                    program,
                    adapter.source_type(),
                )?;
                Ok(vec![(adapter.source(), adapter.source_type())])
            }
            (Type::Function { .. }, Value::NamedFunction(_)) => Ok(Vec::new()),
            (
                Type::Int8
                | Type::Int16
                | Type::Int32
                | Type::Int64
                | Type::Uint8
                | Type::Uint16
                | Type::Uint32
                | Type::Uint64,
                Value::Int64(_),
            )
            | (Type::Uint8 | Type::Uint16 | Type::Uint32 | Type::Uint64, Value::Uint64(_))
            | (Type::Float32 | Type::Float64, Value::Float64(_))
            | (Type::String, Value::String(_) | Value::Error(_))
            | (Type::Bool, Value::Bool(_))
            | (Type::Bytes, Value::Bytes(_))
            | (Type::Nothing, Value::Nothing) => Ok(Vec::new()),
            _ => Err("actor graph carrier disagrees with its checked shape".into()),
        }
    }

    // Resolve written capture contracts in the closure's retained lexical
    // context. Import never borrows aliases or generic bindings from its caller.
    fn actor_graph_capture_type(
        &self,
        written: &TypeExpr,
        bindings: &HashMap<String, TypeExpr>,
        aliases: &HashMap<String, String>,
        namespace: Option<&str>,
        seen: &mut HashSet<String>,
    ) -> Result<TypeExpr, String> {
        let name = |ident: &jett_parser::ast::Ident| {
            let mut ident = self.qualify_declared_type_ident(ident, namespace);
            if let Some((prefix, suffix)) = ident.name.split_once('.') {
                if let Some(target) = aliases.get(prefix) {
                    ident.name = format!("{target}.{suffix}");
                }
            }
            ident
        };
        Ok(match written {
            TypeExpr::Named(ident) => {
                if let Some(bound) = bindings.get(&ident.name) {
                    if !seen.insert(ident.name.clone()) {
                        return Err("actor graph capture type binding is cyclic".into());
                    }
                    // Installed bindings already carry their canonical identity.
                    let resolved = bound.clone();
                    seen.remove(&ident.name);
                    resolved
                } else {
                    TypeExpr::Named(name(ident))
                }
            }
            TypeExpr::Generic(ident, args, span) => TypeExpr::Generic(
                name(ident),
                args.iter()
                    .map(|arg| {
                        self.actor_graph_capture_type(arg, bindings, aliases, namespace, seen)
                    })
                    .collect::<Result<_, _>>()?,
                *span,
            ),
            TypeExpr::View(inner, span) => TypeExpr::View(
                Box::new(self.actor_graph_capture_type(inner, bindings, aliases, namespace, seen)?),
                *span,
            ),
            TypeExpr::StateQualified(inner, state, span) => TypeExpr::StateQualified(
                Box::new(self.actor_graph_capture_type(inner, bindings, aliases, namespace, seen)?),
                state.clone(),
                *span,
            ),
            TypeExpr::Function(params, result, span) => TypeExpr::Function(
                params
                    .iter()
                    .map(|arg| {
                        self.actor_graph_capture_type(arg, bindings, aliases, namespace, seen)
                    })
                    .collect::<Result<_, _>>()?,
                Box::new(
                    self.actor_graph_capture_type(result, bindings, aliases, namespace, seen)?,
                ),
                *span,
            ),
        })
    }

    fn actor_graph_concrete_type(
        &self,
        value: &Value,
        types: &TypeInterner,
    ) -> Result<TypeId, String> {
        let primitive = match value {
            Value::Int64(_) => Some(TypeInterner::INT64),
            Value::Uint64(_) => Some(TypeInterner::UINT64),
            Value::Float64(_) => Some(TypeInterner::FLOAT64),
            Value::Bool(_) => Some(TypeInterner::BOOL),
            Value::String(_) => Some(TypeInterner::STRING),
            Value::Bytes(_) => Some(TypeInterner::BYTES),
            Value::Nothing => Some(TypeInterner::NOTHING),
            Value::TypeConstruction { .. } => Some(TypeInterner::TYPE_CONSTRUCTION),
            _ => None,
        };
        if let Some(primitive) = primitive {
            return Ok(primitive);
        }
        let name = match value {
            Value::Actor(seed) => self
                .actor_instances
                .get(seed)
                .ok_or("erased actor seed is not live")?
                .type_name
                .as_str(),
            Value::Struct {
                type_name,
                concrete_type,
                ..
            } => concrete_type.as_deref().unwrap_or(type_name),
            Value::Enum { type_name, .. } | Value::Machine { type_name, .. } => type_name,
            _ => {
                return Err(
                    "actor graph interface payload has no retained exact concrete owner".into(),
                );
            }
        };
        checked_type_name(types, name)
    }
}

fn checked_type_name(types: &TypeInterner, name: &str) -> Result<TypeId, String> {
    let name = name.strip_prefix("view ").unwrap_or(name);
    let mut matches = types.type_ids().filter(|ty| types.type_name(*ty) == name);
    let ty = matches
        .next()
        .ok_or("actor graph type is absent from its checked program")?;
    if matches.next().is_some() {
        return Err("actor graph type identity is ambiguous".into());
    }
    Ok(ty)
}

fn frozen_fields(
    values: &HashMap<String, Value>,
    expected: &[(String, TypeId)],
) -> Result<Vec<FrozenActorField>, String> {
    if values.len() != expected.len() {
        return Err("actor snapshot field arity mismatch".into());
    }
    expected
        .iter()
        .map(|(name, ty)| {
            Ok(FrozenActorField {
                name: name.clone(),
                ty: *ty,
                value: values
                    .get(name)
                    .ok_or("actor snapshot field is absent")?
                    .clone(),
            })
        })
        .collect()
}

fn named_children<'a>(
    values: &'a [(String, Value)],
    expected: &[(String, TypeId)],
) -> Result<Vec<(&'a Value, TypeId)>, String> {
    if values.len() != expected.len() {
        return Err("actor graph record field arity mismatch".into());
    }
    expected
        .iter()
        .map(|(name, ty)| {
            let mut matching = values.iter().filter(|(actual, _)| actual == name);
            let (_, value) = matching
                .next()
                .ok_or("actor graph record field is absent")?;
            if matching.next().is_some() {
                return Err("actor graph record field is duplicated".into());
            }
            Ok((value, *ty))
        })
        .collect()
}

fn positional_children<'a>(
    values: &'a [Value],
    expected: &[(String, TypeId)],
) -> Result<Vec<(&'a Value, TypeId)>, String> {
    if values.len() != expected.len() {
        return Err("actor graph payload arity mismatch".into());
    }
    Ok(values
        .iter()
        .zip(expected)
        .map(|(value, (_, ty))| (value, *ty))
        .collect())
}

fn import_fields(
    fields: &[FrozenActorField],
    identities: &HashMap<u64, u64>,
) -> Result<HashMap<String, Value>, String> {
    let mut values = HashMap::new();
    values
        .try_reserve(fields.len())
        .map_err(|_| "cannot reserve actor imported fields")?;
    for field in fields {
        values.insert(
            field.name.clone(),
            remap_value(field.value.clone(), identities)?,
        );
    }
    Ok(values)
}

pub(super) fn contains_actor(value: &Value) -> bool {
    let mut pending = vec![value];
    while let Some(value) = pending.pop() {
        if matches!(value, Value::Actor(_)) {
            return true;
        }
        pending.extend(carrier_children(value));
    }
    false
}

fn carrier_children(value: &Value) -> Vec<&Value> {
    match value {
        Value::Typed { value, .. }
        | Value::Pending(value)
        | Value::ResultOk(value)
        | Value::ResultFail(value)
        | Value::OptionalSome(value) => vec![value],
        Value::List(values)
        | Value::Set(values)
        | Value::Enum { fields: values, .. }
        | Value::Machine { fields: values, .. } => values.iter().collect(),
        Value::Struct { fields, .. } => fields.iter().map(|(_, value)| value).collect(),
        Value::Map(entries) => entries
            .iter()
            .flat_map(|(left, right)| [left, right])
            .collect(),
        Value::TypeConstruction { fields, .. } => {
            fields.iter().map(|(_, _, _, value)| value).collect()
        }
        Value::Function { captures, .. } => captures.values().collect(),
        Value::ReflectedCallable(adapter) => vec![adapter.source()],
        Value::Actor(_)
        | Value::Int64(_)
        | Value::Uint64(_)
        | Value::Float64(_)
        | Value::String(_)
        | Value::Bool(_)
        | Value::Bytes(_)
        | Value::OptionalNone
        | Value::Nothing
        | Value::Capability(_)
        | Value::Resource(_)
        | Value::ResourceHook(_)
        | Value::GrantedNetwork(_)
        | Value::Error(_)
        | Value::NamedFunction(_) => Vec::new(),
    }
}

fn remap_value(mut value: Value, identities: &HashMap<u64, u64>) -> Result<Value, String> {
    let mut pending = vec![&mut value];
    while let Some(value) = pending.pop() {
        match value {
            Value::Actor(seed) => {
                *seed = *identities
                    .get(seed)
                    .ok_or("actor reference is absent from the frozen graph")?
            }
            Value::Typed { value, .. }
            | Value::Pending(value)
            | Value::ResultOk(value)
            | Value::ResultFail(value)
            | Value::OptionalSome(value) => pending.push(value.as_mut()),
            Value::List(values)
            | Value::Set(values)
            | Value::Enum { fields: values, .. }
            | Value::Machine { fields: values, .. } => pending.extend(values.iter_mut()),
            Value::Struct { fields, .. } => {
                pending.extend(fields.iter_mut().map(|(_, value)| value))
            }
            Value::Map(entries) => {
                pending.extend(entries.iter_mut().flat_map(|(left, right)| [left, right]))
            }
            Value::TypeConstruction { fields, .. } => {
                pending.extend(fields.iter_mut().map(|(_, _, _, value)| value))
            }
            Value::Function { captures, .. } => pending.extend(captures.values_mut()),
            Value::ReflectedCallable(adapter) => {
                // Retain the opaque source and its original sealed capture witness.
                // Current Source rejects move-only Actor closure captures (E0402).
                if contains_actor(adapter.source()) {
                    return Err(
                        "actor import cannot rewrite an original reflected callback capture".into(),
                    );
                }
            }
            Value::Capability(_) | Value::Resource(_) | Value::GrantedNetwork(_) => {
                return Err(
                    "actor import cannot clone runtime authority or Resource custody".into(),
                );
            }
            Value::Int64(_)
            | Value::Uint64(_)
            | Value::Float64(_)
            | Value::String(_)
            | Value::Bool(_)
            | Value::Bytes(_)
            | Value::OptionalNone
            | Value::Nothing
            | Value::ResourceHook(_)
            | Value::Error(_)
            | Value::NamedFunction(_) => {}
        }
    }
    drop(pending);
    Ok(value)
}

#[cfg(test)]
mod tests;
