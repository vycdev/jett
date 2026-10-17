//! Exact metadata projection from the immutable, privately cached graph.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorGraphHookRoot {
    Root,
    Field { node: usize, field: usize },
}

/// No public constructor. A caller value/name/type cannot become a producer.
#[derive(Debug, Clone)]
pub struct CheckedActorGraphHook {
    required: CheckedRequiredValue,
    root: ActorGraphHookRoot,
    path: Vec<usize>,
    signature: TypeId,
}

fn children(value: &Value) -> Vec<&Value> {
    match value {
        Value::Typed { value, .. }
        | Value::Pending(value)
        | Value::OptionalSome(value)
        | Value::ResultOk(value)
        | Value::ResultFail(value) => vec![value],
        Value::List(values)
        | Value::Set(values)
        | Value::Enum { fields: values, .. }
        | Value::Machine { fields: values, .. } => values.iter().collect(),
        Value::Map(values) => values
            .iter()
            .flat_map(|(key, value)| [key, value])
            .collect(),
        Value::Struct { fields, .. } => fields.iter().map(|(_, value)| value).collect(),
        Value::TypeConstruction { fields, .. } => {
            fields.iter().map(|(_, _, _, value)| value).collect()
        }
        Value::Function { captures, .. } => {
            let mut ordered = captures.iter().collect::<Vec<_>>();
            ordered.sort_by(|a, b| a.0.cmp(b.0));
            ordered.into_iter().map(|(_, value)| value).collect()
        }
        Value::ReflectedCallable(adapter) => vec![adapter.source()],
        _ => Vec::new(),
    }
}
impl CheckedRequiredValue {
    /// Enumerate only authenticated metadata leaves, retaining complete root paths.
    pub fn actor_graph_hooks(&self) -> Result<Vec<CheckedActorGraphHook>, String> {
        let Some(graph) = self.actor_graph() else {
            return Ok(Vec::new());
        };
        let roots = std::iter::once(graph.root()).chain(graph.nodes().iter().flat_map(|node| {
            node.captures()
                .iter()
                .chain(node.state())
                .map(|field| field.value())
        }));
        let mut result = Vec::new();
        for root in roots {
            let mut work = vec![root];
            while let Some(value) = work.pop() {
                if let Value::ResourceHook(descriptor) = value {
                    let hook = self
                        .key
                        .program
                        .checked()
                        .resource_hooks
                        .get(&descriptor.definition)
                        .ok_or("graph metadata leaf has no exact checked hook")?;
                    result.push(
                        self.project_actor_graph_hook(value, hook.function_type)?
                            .ok_or("graph metadata leaf lost its protected path")?,
                    );
                }
                let descendants = children(value);
                work.extend(descendants.into_iter().rev());
            }
        }
        Ok(result)
    }

    /// Associate only the actual retained leaf address with this protected row.
    /// The graph exporter has already checked every intermediate typed carrier.
    pub fn project_actor_graph_hook(
        &self,
        value: &Value,
        signature: TypeId,
    ) -> Result<Option<CheckedActorGraphHook>, String> {
        if !matches!(value, Value::ResourceHook(_)) {
            return Ok(None);
        }
        let graph = self
            .actor_graph()
            .ok_or("hook projection has no original checked graph")?;
        if !graph.belongs_to(&self.key.program) {
            return Err(ResourceExecutionError::ForeignProgram.to_string());
        }
        let mut roots = vec![(ActorGraphHookRoot::Root, graph.root())];
        for (node_index, node) in graph.nodes().iter().enumerate() {
            for (field_index, field) in node.captures().iter().chain(node.state()).enumerate() {
                roots.push((
                    ActorGraphHookRoot::Field {
                        node: node_index,
                        field: field_index,
                    },
                    field.value(),
                ));
            }
        }
        let mut found = None;
        for (root, original) in roots {
            let mut pending = vec![(original, Vec::new())];
            while let Some((candidate, path)) = pending.pop() {
                if std::ptr::eq(candidate, value) {
                    if found.is_some() {
                        return Err("hook projection has ambiguous retained leaf identity".into());
                    }
                    let projection = CheckedActorGraphHook {
                        required: self.clone(),
                        root,
                        path: path.clone(),
                        signature,
                    };
                    projection.checked_hook(&self.key.program)?;
                    found = Some(projection);
                }
                for (index, child) in children(candidate).into_iter().enumerate() {
                    let mut child_path = path.clone();
                    child_path.push(index);
                    pending.push((child, child_path));
                }
            }
        }
        found.map(Some).ok_or_else(|| {
            "hook projection is not an original leaf of this private required graph".into()
        })
    }
}
impl CheckedActorGraphHook {
    pub fn required(&self) -> &CheckedRequiredValue {
        &self.required
    }
    pub fn root(&self) -> ActorGraphHookRoot {
        self.root
    }
    pub fn path(&self) -> &[usize] {
        &self.path
    }
    pub fn signature(&self) -> TypeId {
        self.signature
    }
    pub fn belongs_to(&self, program: &Arc<CheckedResourceProgram>) -> bool {
        self.required.belongs_to(program)
    }
    pub fn value(&self) -> Result<&Value, String> {
        let graph = self
            .required
            .actor_graph()
            .ok_or("projected hook lost its private graph")?;
        let mut value = match self.root {
            ActorGraphHookRoot::Root => graph.root(),
            ActorGraphHookRoot::Field { node, field } => graph
                .nodes()
                .get(node)
                .and_then(|node| node.captures().iter().chain(node.state()).nth(field))
                .map(|field| field.value())
                .ok_or("projected hook lost its exact constructor-owned field")?,
        };
        for index in &self.path {
            value = children(value)
                .get(*index)
                .copied()
                .ok_or("projected hook changed its retained carrier/capture path")?;
        }
        Ok(value)
    }
    pub fn checked_hook<'a>(
        &'a self,
        program: &Arc<CheckedResourceProgram>,
    ) -> Result<&'a CheckedResourceHook, String> {
        if !self.belongs_to(program) {
            return Err(ResourceExecutionError::ForeignProgram.to_string());
        }
        let graph = self
            .required
            .actor_graph()
            .ok_or("projected hook lost its private graph")?;
        if !graph.belongs_to(program) {
            return Err(ResourceExecutionError::ForeignProgram.to_string());
        }
        let mut value = match self.root {
            ActorGraphHookRoot::Root => graph.root(),
            ActorGraphHookRoot::Field { node, field } => graph
                .nodes()
                .get(node)
                .and_then(|node| node.captures().iter().chain(node.state()).nth(field))
                .map(|field| field.value())
                .ok_or("projected hook lost its exact constructor-owned field")?,
        };
        for index in &self.path {
            value = children(value)
                .get(*index)
                .copied()
                .ok_or("projected hook changed its retained carrier/capture path")?;
        }
        let Value::ResourceHook(descriptor) = value else {
            return Err("projected hook leaf changed value kind".into());
        };
        if !descriptor.matches_type(program, self.signature) {
            return Err("projected hook changed exact same-program nominal signature".into());
        }
        self.required
            .key
            .program
            .checked()
            .resource_hooks
            .get(&descriptor.definition)
            .ok_or_else(|| ResourceExecutionError::InvalidInvocation.to_string())
    }
}
