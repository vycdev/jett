//! Execute a sealed required actor graph without Source or predicate replay.
use super::*;
use debug::{DebugGraph, DebugNode};
use jett_comptime::Value as FrozenValue;
use jett_hir::evaluated_values::{function_value_candidates, selected_function};
use jett_hir::{ActorGraphInstruction, ActorGraphOperation};
use std::collections::HashMap;

enum Supplement {
    Hidden(u32),
    Function(FunctionId, Option<u32>),
    Interface(TypeId, u32),
    Builder(String, u32, Vec<u32>),
    Resource,
    List(Vec<u32>),
    Map(Vec<(u32, u32)>),
    CheckedHook(u32, u32, u32),
}
struct ImportGraph<'a> {
    debug: DebugGraph<'a>,
    instruction: &'a ActorGraphInstruction,
    supplements: Vec<(u32, Supplement)>,
    hooks: HashMap<usize, (u32, u32, u32)>,
}
impl ImportGraph<'_> {
    fn add(&mut self, node: DebugNode) -> Result<u32, String> {
        let index = u32::try_from(self.debug.nodes.len())
            .map_err(|_| "native actor import schema exhausted")?;
        self.debug.nodes.push(Some(node));
        Ok(index)
    }
    fn typed(&mut self, ty: TypeId) -> Result<u32, String> {
        if let Some(index) = self.debug.indexes.get(&ty) {
            return Ok(*index);
        }
        let index =
            u32::try_from(self.debug.nodes.len()).map_err(|_| "native import schema exhausted")?;
        self.debug.indexes.insert(ty, index);
        self.debug.nodes.push(None);
        let node = if let Some(kind) = crate::values::list_sort_kind(self.debug.types, ty) {
            DebugNode::Primitive(kind as u32)
        } else {
            match self.debug.types.resolve(ty).clone() {
                Type::Nothing => DebugNode::Nothing,
                Type::Bytes => DebugNode::Bytes,
                Type::List(c) => DebugNode::List(self.typed(c)?),
                Type::Set(c) => DebugNode::Set(self.typed(c)?),
                Type::Map(a, b) => DebugNode::Map(self.typed(a)?, self.typed(b)?),
                Type::Optional(c) => DebugNode::Optional(self.typed(c)?),
                Type::Result(a, b) => DebugNode::Result(self.typed(a)?, self.typed(b)?),
                Type::Struct(id) => {
                    let definition = self.debug.types.resolve_struct(id).clone();
                    let fields = definition
                        .fields
                        .into_iter()
                        .map(|(name, ty)| Ok((name, self.typed(ty)?)))
                        .collect::<Result<_, String>>()?;
                    DebugNode::Record(definition.name, fields)
                }
                Type::Bitfield(id) => {
                    let definition = self.debug.types.resolve_bitfield(id).clone();
                    let fields = definition
                        .fields
                        .into_iter()
                        .map(|field| Ok((field.name, self.typed(field.ty)?)))
                        .collect::<Result<_, String>>()?;
                    DebugNode::Record(definition.name, fields)
                }
                Type::Enum(id) => {
                    let definition = self.debug.types.resolve_enum(id).clone();
                    let variants = definition
                        .variants
                        .into_iter()
                        .map(|variant| {
                            Ok((
                                variant.name,
                                variant
                                    .fields
                                    .into_iter()
                                    .map(|(_, ty)| self.typed(ty))
                                    .collect::<Result<_, _>>()?,
                            ))
                        })
                        .collect::<Result<_, String>>()?;
                    DebugNode::Enum(definition.name, variants)
                }
                Type::Machine(id) | Type::MachineState { machine: id, .. } => {
                    let definition = self.debug.types.resolve_machine(id).clone();
                    let states = definition
                        .states
                        .into_iter()
                        .map(|state| {
                            Ok((
                                state.name,
                                state
                                    .fields
                                    .into_iter()
                                    .map(|(_, ty)| self.typed(ty))
                                    .collect::<Result<_, _>>()?,
                            ))
                        })
                        .collect::<Result<_, String>>()?;
                    DebugNode::Machine(definition.name, states)
                }
                Type::Refinement { base, .. } => DebugNode::Alias(self.typed(base)?),
                Type::Secret(base) => {
                    let child = self.typed(base)?;
                    self.supplements.push((index, Supplement::Hidden(child)));
                    DebugNode::Redacted
                }
                Type::Resource(_) => {
                    self.supplements.push((index, Supplement::Resource));
                    DebugNode::Redacted
                }
                Type::Capability(_) => DebugNode::Capability(self.debug.types.type_name(ty)),
                Type::Actor(_) => DebugNode::Actor,
                Type::Function { .. } => DebugNode::Function,
                Type::Interface(_) => DebugNode::Interface,
                Type::TypeConstruction => DebugNode::TypeConstruction,
                Type::Never => DebugNode::Uninhabited,
                _ => return Err("native graph type lacks its checked value carrier layout".into()),
            }
        };
        self.debug.nodes[index as usize] = Some(node);
        Ok(index)
    }
    fn value(&mut self, ty: TypeId, mut value: &FrozenValue) -> Result<u32, String> {
        // Pending depth belongs to the runtime carrier. It never changes the
        // checked value/owner schema or imports a saved native wrapper ID.
        while let FrozenValue::Pending(inner) = value {
            value = inner;
        }
        let types = self.debug.types;
        if let Type::Secret(inner) = types.resolve(ty) {
            let child = self.value(*inner, value)?;
            let node = self.add(DebugNode::Redacted)?;
            self.supplements.push((node, Supplement::Hidden(child)));
            return Ok(node);
        }
        if let Type::Refinement { base, .. } = types.resolve(ty) {
            if let FrozenValue::Typed {
                type_name,
                value: inner,
            } = value
            {
                if *type_name != types.type_name(ty) {
                    return Err("frozen refinement lost its exact nominal owner".into());
                }
                value = inner;
            }
            let child = self.value(*base, value)?;
            return self.add(DebugNode::Alias(child));
        }
        if let Type::Interface(_) = types.resolve(ty) {
            let concrete = self.concrete(value)?;
            let child = self.value(concrete, value)?;
            let node = self.add(DebugNode::Interface)?;
            self.supplements
                .push((node, Supplement::Interface(concrete, child)));
            return Ok(node);
        }
        if let FrozenValue::Typed { value: inner, .. } = value {
            return self.value(ty, inner);
        }
        match (types.resolve(ty), value) {
            (Type::Function { .. }, FrozenValue::ResourceHook(_)) => self.function(ty, value),
            (Type::Function { .. }, FrozenValue::ReflectedCallable(_)) => self.function(ty, value),
            (
                Type::Function { .. },
                FrozenValue::Function { .. } | FrozenValue::NamedFunction(_),
            ) => self.function(ty, value),
            (Type::List(inner), FrozenValue::List(values)) => {
                let default = self.typed(*inner)?;
                let mut children = Vec::new();
                for value in values {
                    children.push(self.value(*inner, value)?);
                }
                let node = self.add(DebugNode::List(default))?;
                self.supplements.push((node, Supplement::List(children)));
                Ok(node)
            }
            (Type::Map(key, mapped), FrozenValue::Map(values)) => {
                let default_key = self.typed(*key)?;
                let default_value = self.typed(*mapped)?;
                let mut children = Vec::new();
                for (left, right) in values {
                    children.push((self.value(*key, left)?, self.value(*mapped, right)?));
                }
                let node = self.add(DebugNode::Map(default_key, default_value))?;
                self.supplements.push((node, Supplement::Map(children)));
                Ok(node)
            }
            (Type::Set(inner), FrozenValue::Set(_)) => {
                let child = self.typed(*inner)?;
                self.add(DebugNode::Set(child))
            }
            (Type::Optional(inner), FrozenValue::OptionalSome(value)) => {
                let child = self.value(*inner, value)?;
                self.add(DebugNode::Optional(child))
            }
            (Type::Optional(inner), FrozenValue::OptionalNone) => {
                let child = self.typed(*inner)?;
                self.add(DebugNode::Optional(child))
            }
            (Type::Result(ok, error), FrozenValue::ResultOk(value)) => {
                let a = self.value(*ok, value)?;
                let b = self.typed(*error)?;
                self.add(DebugNode::Result(a, b))
            }
            (Type::Result(ok, error), FrozenValue::ResultFail(value)) => {
                let a = self.typed(*ok)?;
                let b = self.value(*error, value)?;
                self.add(DebugNode::Result(a, b))
            }
            (Type::Struct(id), FrozenValue::Struct { fields, .. }) => {
                let definition = types.resolve_struct(*id);
                let mut children = Vec::new();
                for (name, ty) in &definition.fields {
                    let value = fields
                        .iter()
                        .find(|(n, _)| n == name)
                        .ok_or("frozen record field missing")?;
                    children.push((name.clone(), self.value(*ty, &value.1)?));
                }
                self.add(DebugNode::Record(definition.name.clone(), children))
            }
            (Type::Bitfield(id), FrozenValue::Struct { fields, .. }) => {
                let definition = types.resolve_bitfield(*id);
                let mut children = Vec::new();
                for field in &definition.fields {
                    let value = fields
                        .iter()
                        .find(|(n, _)| n == &field.name)
                        .ok_or("frozen bitfield field missing")?;
                    children.push((field.name.clone(), self.value(field.ty, &value.1)?));
                }
                self.add(DebugNode::Record(definition.name.clone(), children))
            }
            (
                Type::Enum(id),
                FrozenValue::Enum {
                    variant, fields, ..
                },
            ) => {
                let definition = types.resolve_enum(*id);
                let mut variants = Vec::new();
                for candidate in &definition.variants {
                    let mut children = Vec::new();
                    for (index, (_, ty)) in candidate.fields.iter().enumerate() {
                        children.push(if &candidate.name == variant {
                            self.value(
                                *ty,
                                fields.get(index).ok_or("frozen enum payload missing")?,
                            )?
                        } else {
                            self.typed(*ty)?
                        });
                    }
                    variants.push((candidate.name.clone(), children));
                }
                self.add(DebugNode::Enum(definition.name.clone(), variants))
            }
            (
                Type::Machine(id) | Type::MachineState { machine: id, .. },
                FrozenValue::Machine { state, fields, .. },
            ) => {
                let definition = types.resolve_machine(*id);
                let mut states = Vec::new();
                for candidate in &definition.states {
                    let mut children = Vec::new();
                    for (index, (_, ty)) in candidate.fields.iter().enumerate() {
                        children.push(if &candidate.name == state {
                            self.value(
                                *ty,
                                fields.get(index).ok_or("frozen machine payload missing")?,
                            )?
                        } else {
                            self.typed(*ty)?
                        });
                    }
                    states.push((candidate.name.clone(), children));
                }
                self.add(DebugNode::Machine(definition.name.clone(), states))
            }
            (Type::TypeConstruction, FrozenValue::TypeConstruction { .. }) => self.builder(value),
            (Type::Actor(_), FrozenValue::Actor(seed)) => {
                if self.instruction.actor_seed_type(*seed) != Some(ty) {
                    return Err("native graph field actor has foreign nominal seed".into());
                }
                self.add(DebugNode::Actor)
            }
            (Type::Capability(_) | Type::Resource(_), _) => {
                Err("native graph import cannot clone runtime authority or Resource custody".into())
            }
            _ => self.typed(ty),
        }
    }
    fn concrete(&self, value: &FrozenValue) -> Result<TypeId, String> {
        let types = self.debug.types;
        if let FrozenValue::Typed { type_name, .. } = value {
            return checked_type(types, type_name);
        }
        Ok(match value {
            FrozenValue::Actor(seed) => self
                .instruction
                .actor_seed_type(*seed)
                .ok_or("frozen interface actor lost its exact seed type")?,
            FrozenValue::Int64(_) => TypeInterner::INT64,
            FrozenValue::Uint64(_) => TypeInterner::UINT64,
            FrozenValue::Float64(_) => TypeInterner::FLOAT64,
            FrozenValue::Bool(_) => TypeInterner::BOOL,
            FrozenValue::String(_) => TypeInterner::STRING,
            FrozenValue::Bytes(_) => TypeInterner::BYTES,
            FrozenValue::Nothing => TypeInterner::NOTHING,
            FrozenValue::TypeConstruction { .. } => TypeInterner::TYPE_CONSTRUCTION,
            FrozenValue::Struct {
                type_name,
                concrete_type,
                ..
            } => checked_type(types, concrete_type.as_deref().unwrap_or(type_name))?,
            FrozenValue::Enum { type_name, .. } | FrozenValue::Machine { type_name, .. } => {
                checked_type(types, type_name)?
            }
            _ => return Err("frozen erased graph field has no exact concrete identity".into()),
        })
    }
    fn function(&mut self, ty: TypeId, value: &FrozenValue) -> Result<u32, String> {
        if matches!(value, FrozenValue::ResourceHook(_)) {
            let (operation, hook, signature) = self
                .hooks
                .get(&std::ptr::from_ref(value).addr())
                .copied()
                .ok_or("graph hook field lost its exact emitted Descriptor projection")?;
            let projection = self
                .instruction
                .required_proof()
                .project_actor_graph_hook(value, ty)?
                .ok_or("graph hook field lost its required path/signature")?;
            projection.checked_hook(self.instruction.checked())?;
            let node = self.add(DebugNode::Function)?;
            self.supplements
                .push((node, Supplement::CheckedHook(operation, hook, signature)));
            return Ok(node);
        }

        if let FrozenValue::ReflectedCallable(admitted) = value {
            let plan = self
                .instruction
                .reflected_callable(admitted)
                .ok_or("frozen graph reflected callback has no exact sealed adapter occurrence")?;
            plan.validate_types(self.debug.types)
                .map_err(|message| message.to_string())?;
            if !plan.is_materialized()
                || !plan.is_sealed()
                || !plan.validate_original()
                || plan.checked_plan() != admitted.checked_plan()
                || plan.source_type() != admitted.source_type()
                || plan.requested_type() != admitted.requested_type()
                || admitted.requested_type() != ty
            {
                return Err(
                    "frozen graph reflected callback changed its checked adapter contract".into(),
                );
            }
            // The private HIR selector joins this exact retained leaf to its
            // already sealed adapter and original source snapshot. Import the
            // genuine source descriptor recursively; never select by label or
            // substitute it for the requested callable return contract.
            let adapter = plan.function();
            let canonical = plan
                .canonical_adapter()
                .ok_or("frozen graph reflected callback lost its canonical adapter")?;
            let declaration = self
                .instruction
                .functions()
                .iter()
                .find(|function| function.id == adapter)
                .ok_or("frozen graph reflected callback adapter declaration is absent")?;
            if declaration.id != canonical.id
                || declaration.identity != canonical.identity
                || declaration.debug_kind != canonical.debug_kind
                || declaration.source_definition != canonical.source_definition
                || declaration.params != canonical.params
                || declaration.capture_count != canonical.capture_count
                || declaration.return_type != canonical.return_type
                || declaration.locals != canonical.locals
                || declaration.span != canonical.span
                || declaration.capture_count != 1
                || declaration.params.first().is_none_or(|param| {
                    param.ty != admitted.source_type() || param.mode != jett_hir::ParamMode::Owned
                })
            {
                return Err(
                    "frozen graph reflected callback changed its canonical adapter declaration"
                        .into(),
                );
            }
            let signature = Type::Function {
                params: declaration.params[1..]
                    .iter()
                    .map(|param| param.ty)
                    .collect(),
                view_params: declaration.params[1..]
                    .iter()
                    .map(|param| param.mode == jett_hir::ParamMode::View)
                    .collect(),
                return_type: declaration.return_type,
            };
            if self.debug.types.resolve(ty) != &signature {
                return Err("frozen graph reflected callback adapter signature changed".into());
            }
            // Mandatory HIR/MIR reflected callable validation authenticates the
            // canonical body, return conversion and predicates before emission.
            // Its retained sealed metadata is preserved rather than erased here.
            let source = self.value(admitted.source_type(), admitted.source())?;
            let depth = self.typed(TypeInterner::UINT64)?;
            let environment = self.add(DebugNode::Record(
                "$checked_adapter_environment".into(),
                vec![("$source".into(), source), ("$pending0".into(), depth)],
            ))?;
            let node = self.add(DebugNode::Function)?;
            self.supplements
                .push((node, Supplement::Function(adapter, Some(environment))));
            return Ok(node);
        }

        let checked = self.instruction.checked();
        let candidates = function_value_candidates(
            self.instruction.functions(),
            &checked.checked().method_value_definitions,
        );
        let selected = selected_function(value, ty, self.debug.types, &candidates)?;
        let mut children = Vec::new();
        for (name, ty) in selected.captures() {
            let FrozenValue::Function { captures, .. } = value else {
                return Err("frozen named callback has unexpected environment".into());
            };
            let value = captures
                .get(name)
                .ok_or("frozen callback capture is absent")?;
            children.push((name.clone(), self.value(*ty, value)?));
        }
        let environment = if children.is_empty() {
            None
        } else {
            for index in 0..selected.captures().len() {
                children.push((
                    format!("$pending{index}"),
                    self.typed(TypeInterner::UINT64)?,
                ));
            }
            Some(self.add(DebugNode::Record(
                "$checked_capture_environment".into(),
                children,
            ))?)
        };
        let source = self.add(DebugNode::Function)?;
        self.supplements
            .push((source, Supplement::Function(selected.id(), environment)));
        let declaration = self
            .instruction
            .functions()
            .iter()
            .find(|function| function.id == selected.id())
            .ok_or("frozen callback declaration absent")?;
        let signature = Type::Function {
            params: declaration.params[declaration.capture_count..]
                .iter()
                .map(|param| param.ty)
                .collect(),
            view_params: declaration.params[declaration.capture_count..]
                .iter()
                .map(|param| param.mode == jett_hir::ParamMode::View)
                .collect(),
            return_type: declaration.return_type,
        };
        let source_type = self
            .debug
            .types
            .type_ids()
            .find(|candidate| self.debug.types.resolve(*candidate) == &signature)
            .ok_or("frozen callback source signature absent")?;
        if source_type == ty {
            return Ok(source);
        }
        let mut adapters = self.instruction.functions().iter().filter(|function| {
            function
                .identity
                .declaration
                .name
                .starts_with("$interface.adapter.")
                && function.capture_count == 1
                && function
                    .params
                    .first()
                    .is_some_and(|param| param.ty == source_type)
                && self.debug.types.resolve(ty)
                    == &Type::Function {
                        params: function.params[1..].iter().map(|param| param.ty).collect(),
                        view_params: function.params[1..]
                            .iter()
                            .map(|param| param.mode == jett_hir::ParamMode::View)
                            .collect(),
                        return_type: function.return_type,
                    }
        });
        let adapter = adapters
            .next()
            .ok_or("frozen callback conversion lost its checked generated adapter")?
            .id;
        if adapters.next().is_some() {
            return Err("frozen callback conversion is ambiguous".into());
        }
        let depth = self.typed(TypeInterner::UINT64)?;
        let environment = self.add(DebugNode::Record(
            "$checked_adapter_environment".into(),
            vec![("$source".into(), source), ("$pending0".into(), depth)],
        ))?;
        let node = self.add(DebugNode::Function)?;
        self.supplements
            .push((node, Supplement::Function(adapter, Some(environment))));
        Ok(node)
    }
    fn builder(&mut self, value: &FrozenValue) -> Result<u32, String> {
        let FrozenValue::TypeConstruction {
            type_name,
            variant,
            state,
            fields,
        } = value
        else {
            unreachable!()
        };
        let checked = self.instruction.checked();
        let reflection = &checked.checked().reflection_metadata;
        let mut selected = if let Some(name) = variant {
            reflection
                .get_type_variants(type_name)
                .ok_or("frozen builder variant metadata absent")?
                .iter()
                .find(|v| &v.name == name)
                .ok_or("frozen builder variant absent")?
                .fields
                .clone()
        } else if let Some(name) = state {
            reflection
                .get_machine(type_name)
                .ok_or("frozen builder machine metadata absent")?
                .states
                .iter()
                .find(|v| &v.name == name)
                .ok_or("frozen builder state absent")?
                .fields
                .clone()
        } else {
            reflection
                .get_type_fields(type_name)
                .unwrap_or(&[])
                .to_vec()
        };
        let mut children = Vec::new();
        for field in selected.drain(..) {
            let mut ty = checked_type(self.debug.types, &field.type_info.type_name)?;
            if !matches!(
                self.debug
                    .types
                    .resolve(checked_type(self.debug.types, type_name)?),
                Type::Bitfield(_)
            ) {
                while let Type::Refinement { base, .. } = self.debug.types.resolve(ty) {
                    ty = *base;
                }
            }
            let actual = fields.iter().find(|(_, name, _, _)| name == &field.name);
            children.push(if let Some((_, _, _, value)) = actual {
                self.value(ty, value)?
            } else {
                self.typed(ty)?
            });
        }
        let node = self.add(DebugNode::TypeConstruction)?;
        self.supplements.push((
            node,
            Supplement::Builder(
                type_name.clone(),
                u32::from(variant.is_some() || state.is_some()),
                children,
            ),
        ));
        Ok(node)
    }
    fn encode(self, root: u32) -> Result<(Vec<u8>, Vec<(usize, u64)>), String> {
        let mut debug = b"JD\x01".to_vec();
        put(&mut debug, self.debug.nodes.len())?;
        debug.extend(root.to_le_bytes());
        for node in self.debug.nodes {
            let mut bytes = Vec::new();
            debug::encode_node(&mut bytes, node.ok_or("incomplete private graph schema")?)
                .ok_or("cannot encode private graph node")?;
            put(&mut debug, bytes.len())?;
            debug.extend(bytes);
        }
        let mut bytes = b"JA\x01".to_vec();
        put(&mut bytes, debug.len())?;
        bytes.extend(debug);
        put(&mut bytes, self.supplements.len())?;
        let mut functions = Vec::new();
        for (node, supplement) in self.supplements {
            bytes.extend(node.to_le_bytes());
            match supplement {
                Supplement::Hidden(child) => {
                    bytes.push(0);
                    bytes.extend(child.to_le_bytes());
                }
                Supplement::Function(function, env) => {
                    bytes.push(1);
                    functions.push((bytes.len(), u64::from(function.index())));
                    bytes.extend(0u64.to_le_bytes());
                    bytes.extend(env.unwrap_or(u32::MAX).to_le_bytes());
                }
                Supplement::Interface(ty, child) => {
                    bytes.push(2);
                    bytes.extend(u64::from(ty.index()).to_le_bytes());
                    bytes.extend(child.to_le_bytes());
                }
                Supplement::Builder(owner, offset, fields) => {
                    bytes.push(3);
                    put(&mut bytes, owner.len())?;
                    bytes.extend(owner.as_bytes());
                    bytes.extend(offset.to_le_bytes());
                    put(&mut bytes, fields.len())?;
                    for child in fields {
                        bytes.extend(child.to_le_bytes());
                    }
                }
                Supplement::CheckedHook(operation, hook, signature) => {
                    bytes.push(7);
                    bytes.extend(operation.to_le_bytes());
                    bytes.extend(hook.to_le_bytes());
                    bytes.extend(signature.to_le_bytes());
                }
                Supplement::Resource => bytes.push(4),
                Supplement::List(children) => {
                    bytes.push(5);
                    put(&mut bytes, children.len())?;
                    for child in children {
                        bytes.extend(child.to_le_bytes());
                    }
                }
                Supplement::Map(children) => {
                    bytes.push(6);
                    put(&mut bytes, children.len())?;
                    for (a, b) in children {
                        bytes.extend(a.to_le_bytes());
                        bytes.extend(b.to_le_bytes());
                    }
                }
            }
        }
        Ok((bytes, functions))
    }
}
fn put(bytes: &mut Vec<u8>, count: usize) -> Result<(), String> {
    bytes.extend(
        u32::try_from(count)
            .map_err(|_| "native import schema exhausted")?
            .to_le_bytes(),
    );
    Ok(())
}
fn checked_type(types: &TypeInterner, name: &str) -> Result<TypeId, String> {
    let mut candidates = types.type_ids().filter(|ty| types.type_name(*ty) == name);
    let selected = candidates.next().ok_or("frozen import type absent")?;
    if candidates.next().is_some() {
        return Err("frozen import type ambiguous".into());
    }
    Ok(selected)
}
fn import_layout(
    instruction: &ActorGraphInstruction,
    types: &TypeInterner,
    ty: TypeId,
    hooks: HashMap<usize, (u32, u32, u32)>,
) -> Result<(Vec<u8>, Vec<(usize, u64)>), String> {
    let mut graph = ImportGraph {
        debug: DebugGraph {
            types,
            equality_methods: None,
            custom: Vec::new(),
            indexes: HashMap::new(),
            nodes: Vec::new(),
        },
        instruction,
        supplements: Vec::new(),
        hooks,
    };
    let root = graph.value(ty, instruction.field_value()?)?;
    graph.encode(root)
}

impl Translator<'_, '_> {
    fn actor_graph_hook_rows(
        &self,
        instruction: &ActorGraphInstruction,
    ) -> Result<HashMap<usize, (u32, u32, u32)>, CodegenError> {
        let mut rows = HashMap::new();
        if instruction.projected_hooks().is_empty() {
            return Ok(rows);
        }
        let resource = self.resource.ok_or_else(|| {
            contract_error(
                self.symbol,
                instruction.original().span,
                "graph hook import has no Resource family",
            )
        })?;
        let plan = resource.function_plan()?;
        for (projection, hook) in instruction.projected_hooks() {
            let mut selected = plan.graph_descriptor_operations().filter(|(producer, _)| {
                producer.same_import(instruction)
                    && producer.projected_hook().is_some_and(|current| {
                        current.root() == projection.root()
                            && current.path() == projection.path()
                            && current.signature() == projection.signature()
                    })
            });
            let (producer, operation) = selected.next().ok_or_else(|| {
                contract_error(
                    self.symbol,
                    instruction.original().span,
                    "graph hook import lost its current Descriptor occurrence",
                )
            })?;
            if selected.next().is_some()
                || !matches!(producer.operation(),ActorGraphOperation::HookBox {hook:current,..} if current==hook)
            {
                return Err(contract_error(
                    self.symbol,
                    instruction.original().span,
                    "graph hook Descriptor projection is not unique",
                ));
            }
            let (hook_id, signature) = resource.layout.graph_hook(hook).ok_or_else(|| {
                contract_error(
                    self.symbol,
                    instruction.original().span,
                    "graph hook has no exact installed nominal signature",
                )
            })?;
            rows.insert(
                std::ptr::from_ref(projection.value().map_err(|message| {
                    contract_error(self.symbol, instruction.original().span, message)
                })?)
                .addr(),
                (resource.ordinal(operation)?, hook_id, signature),
            );
        }
        Ok(rows)
    }
    pub(super) fn actor_graph_step(
        &mut self,
        instruction: &ActorGraphInstruction,
        args: &[Expression],
        expression: &Expression,
    ) -> Result<LoweredValue, CodegenError> {
        let ticket = if let Some(arg) = args.first() {
            let value = self.expression(arg)?;
            Some(self.scalar(value, arg.span)?)
        } else {
            None
        };
        match instruction.operation() {
            ActorGraphOperation::Begin => Ok(LoweredValue::Scalar(self.leaf(
                NativeLeaf::ActorGraphBegin,
                &[],
                true,
            )?)),
            ActorGraphOperation::ReserveBase { fields, .. } => {
                let count = self.builder.ins().iconst(ir::types::I64, *fields as i64);
                Ok(LoweredValue::Scalar(self.leaf(
                    NativeLeaf::ActorGraphReserveBase,
                    &[ticket.expect("sealed ticket"), count],
                    true,
                )?))
            }
            ActorGraphOperation::ReservePending { depth, .. } => {
                let base = self.expression(&args[1])?;
                let base = self.scalar(base, args[1].span)?;
                let depth = self.builder.ins().iconst(ir::types::I64, *depth as i64);
                Ok(LoweredValue::Scalar(self.leaf(
                    NativeLeaf::ActorGraphReservePending,
                    &[ticket.expect("sealed ticket"), base, depth],
                    true,
                )?))
            }
            ActorGraphOperation::Fill { ty, field, .. } => {
                let base = self.expression(&args[1])?;
                let base = self.scalar(base, args[1].span)?;
                // Earlier completed values now live in local slots or the
                // unpublished batch. Retire expression snapshots before transfer.
                self.drop_temporaries()?;
                let ExpressionKind::Local(local) = args[2].kind else {
                    return Err(self.unsupported(expression.span, "unsealed graph field place"));
                };
                let value = if let Some(slot) = self.local_slots[local.index() as usize] {
                    let bits = self.builder.ins().stack_load(ir::types::I64, slot, 0);
                    LoweredValue::Owned(bits, slot)
                } else {
                    self.expression(&args[2])?
                };
                let depth = if let LoweredValue::ScalarTask(_, depth) = value {
                    depth
                } else {
                    self.builder.ins().iconst(ir::types::I64, 0)
                };
                let (bits, owned) = self.payload_bits(value);
                let owned = self.builder.ins().iconst(ir::types::I32, i64::from(owned));
                let field = self.builder.ins().iconst(ir::types::I64, *field as i64);
                let (bytes, functions) = import_layout(
                    instruction,
                    self.types,
                    *ty,
                    self.actor_graph_hook_rows(instruction)?,
                )
                .map_err(|message| contract_error(self.symbol, expression.span, message))?;
                let (layout, length) = self.static_data_with_functions(&bytes, &functions)?;
                self.leaf(
                    NativeLeaf::ActorGraphFill,
                    &[
                        ticket.expect("sealed ticket"),
                        base,
                        field,
                        bits,
                        owned,
                        depth,
                        layout,
                        length,
                    ],
                    true,
                )?;
                if let LoweredValue::Owned(_, slot) = value {
                    self.clear_slot(slot);
                }
                Ok(self.nothing())
            }
            ActorGraphOperation::RootFill { ty } => {
                self.drop_temporaries()?;
                let ExpressionKind::Local(local) = args[1].kind else {
                    return Err(self.unsupported(expression.span, "unsealed graph root place"));
                };
                let value = if let Some(slot) = self.local_slots[local.index() as usize] {
                    LoweredValue::Owned(
                        self.builder.ins().stack_load(ir::types::I64, slot, 0),
                        slot,
                    )
                } else {
                    self.expression(&args[1])?
                };
                let depth = if let LoweredValue::ScalarTask(_, depth) = value {
                    depth
                } else {
                    self.builder.ins().iconst(ir::types::I64, 0)
                };
                let (bits, owned) = self.payload_bits(value);
                let owned = self.builder.ins().iconst(ir::types::I32, i64::from(owned));
                let (bytes, functions) = import_layout(
                    instruction,
                    self.types,
                    *ty,
                    self.actor_graph_hook_rows(instruction)?,
                )
                .map_err(|message| contract_error(self.symbol, expression.span, message))?;
                let (layout, length) = self.static_data_with_functions(&bytes, &functions)?;
                self.leaf(
                    NativeLeaf::ActorGraphRootFill,
                    &[
                        ticket.expect("sealed ticket"),
                        bits,
                        owned,
                        depth,
                        layout,
                        length,
                    ],
                    true,
                )?;
                if let LoweredValue::Owned(_, slot) = value {
                    self.clear_slot(slot);
                }
                Ok(LoweredValue::Scalar(bits))
            }
            ActorGraphOperation::RootPublish { ty } => {
                let bits = self.expression(&args[1])?;
                let bits = self.scalar(bits, args[1].span)?;
                if is_copy_owned(self.types, *ty) || is_linear(self.types, *ty) {
                    self.own(bits)
                } else {
                    Ok(LoweredValue::Scalar(bits))
                }
            }
            ActorGraphOperation::HookBox { .. } => Err(contract_error(
                self.symbol,
                expression.span,
                "graph hook producer lost its exact Resource Descriptor operation",
            )),
            ActorGraphOperation::Commit => {
                self.leaf(
                    NativeLeaf::ActorGraphCommit,
                    &[ticket.expect("sealed ticket")],
                    true,
                )?;
                Ok(self.nothing())
            }
        }
    }
}

#[cfg(test)]
mod tests;
