//! Private required-value graph plans. Raw actor IDs never enter execution HIR.
use crate::evaluated_values::{ValueContext, function_value_candidates, value_expression};
use crate::resource_source::{Materialized, RequiredMaterialization};
use crate::{
    Block, Expression, ExpressionKind as E, Function, FunctionId, FunctionIdentity, Local, LocalId,
    LowerError, Program, Statement, StatementKind as S,
};
use jett_common::Span;
use jett_comptime::{CheckedActorGraphHook, CheckedRequiredValue, ExplicitComptimeValues, Value};
use jett_typecheck::CheckedResourceProgram;
use jett_types::{ReflectionMetadata, Type, TypeId, TypeInterner};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, OnceLock, Weak},
};

#[derive(Debug, Clone)]
pub struct ActorGraphSeeds {
    ready: HashMap<u64, (TypeId, LocalId)>,
    pending: HashMap<(u64, u64), LocalId>,
    hooks: HashMap<usize, Expression>,
    origin: Arc<Origin>,
}
impl ActorGraphSeeds {
    pub(crate) fn bind_reflected_source(
        &self,
        admitted: &jett_comptime::value::ReflectedCallableValue,
        expression: &mut Expression,
    ) -> Result<(), String> {
        let leaf = std::ptr::from_ref(admitted).addr();
        if !self.origin.callback_leaves.contains(&leaf) {
            return Err("graph callback has no exact retained checked leaf".into());
        }
        let E::InterfaceCoerce { value, adapters } = &mut expression.kind else {
            return Err("graph callback lost its staged checked conversion".into());
        };
        let [adapter] = adapters.as_mut_slice() else {
            return Err("graph callback has no unique checked conversion".into());
        };
        let plan = adapter
            .reflected
            .as_mut()
            .ok_or("graph callback lost checked admission")?;
        plan.bind_graph_source(CallbackSource {
            origin: Arc::downgrade(&self.origin),
            leaf,
            source: value.as_ref().clone(),
        });
        Ok(())
    }
    pub(crate) fn hook(&self, value: &Value, ty: TypeId) -> Result<Expression, String> {
        self.hooks
            .get(&std::ptr::from_ref(value).addr())
            .filter(|expression| expression.ty == ty)
            .cloned()
            .ok_or_else(|| "graph hook lost its exact protected leaf/signature".into())
    }
    pub fn actor_type(&self, seed: u64) -> Option<TypeId> {
        self.ready.get(&seed).map(|entry| entry.0)
    }
    pub(crate) fn ready(&self, seed: u64, ty: TypeId) -> Result<LocalId, String> {
        let (expected, local) = self.ready.get(&seed).ok_or("actor graph seed is absent")?;
        if *expected != ty {
            return Err("actor graph reference changed its exact nominal actor type".into());
        }
        Ok(*local)
    }
    pub(crate) fn pending(&self, value: &Value, ty: TypeId) -> Result<LocalId, String> {
        let (seed, depth) =
            pending_actor(value).ok_or("pending actor graph carrier has no exact base seed")?;
        self.ready(seed, ty)?;
        self.pending
            .get(&(seed, depth))
            .copied()
            .ok_or_else(|| "pending actor graph reference was not reserved".into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActorGraphOperation {
    Begin,
    ReserveBase {
        ty: TypeId,
        fields: usize,
    },
    ReservePending {
        ty: TypeId,
        depth: u64,
    },
    Fill {
        ty: TypeId,
        field: usize,
        node: usize,
    },
    HookBox {
        hook: crate::ResourceHookRef,
        projection: usize,
    },
    RootFill {
        ty: TypeId,
    },
    RootPublish {
        ty: TypeId,
    },
    Commit,
}
#[derive(Debug)]
struct Origin {
    checked: Arc<CheckedResourceProgram>,
    proof: CheckedRequiredValue,
    function: FunctionId,
    identity: FunctionIdentity,
    original: Expression,
    hooks: Vec<(CheckedActorGraphHook, crate::ResourceHookRef)>,
    // All declarations/types are retained independently of caller-owned HIR.
    functions: OnceLock<Vec<Function>>,
    archive: crate::ResourceSourceArchive,
    callback_leaves: HashSet<usize>,
    callbacks: OnceLock<Vec<(usize, crate::ReflectedCallablePlan)>>,
}
#[derive(Debug)]
struct Instruction {
    origin: Arc<Origin>,
    index: usize,
    operation: ActorGraphOperation,
    result: TypeId,
    operands: Vec<(LocalId, TypeId)>,
}
#[derive(Debug, Clone)]
pub struct ActorGraphInstruction(Arc<Instruction>);
impl PartialEq for ActorGraphInstruction {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for ActorGraphInstruction {}
impl ActorGraphInstruction {
    /// Select only the sealed adapter associated with this exact frozen Graph leaf.
    pub fn reflected_callable(
        &self,
        value: &jett_comptime::value::ReflectedCallableValue,
    ) -> Option<&crate::ReflectedCallablePlan> {
        if !Arc::ptr_eq(value.checked_program(), &self.0.origin.checked) {
            return None;
        }
        let leaf = std::ptr::from_ref(value).addr();
        self.0
            .origin
            .callbacks
            .get()?
            .iter()
            .find_map(|(expected, plan)| {
                (*expected == leaf
                    && plan.is_sealed()
                    && plan.validate_original()
                    && plan.checked_plan() == value.checked_plan())
                .then_some(plan)
            })
    }
    pub fn projected_hooks(&self) -> &[(CheckedActorGraphHook, crate::ResourceHookRef)] {
        &self.0.origin.hooks
    }
    pub fn required_proof(&self) -> &CheckedRequiredValue {
        &self.0.origin.proof
    }
    pub fn projected_hook(&self) -> Option<&CheckedActorGraphHook> {
        let ActorGraphOperation::HookBox { projection, .. } = self.operation() else {
            return None;
        };
        self.0.origin.hooks.get(*projection).map(|(proof, _)| proof)
    }
    pub fn operation(&self) -> &ActorGraphOperation {
        &self.0.operation
    }
    pub fn index(&self) -> usize {
        self.0.index
    }
    pub fn same_import(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0.origin, &other.0.origin)
    }
    pub fn function(&self) -> FunctionId {
        self.0.origin.function
    }
    pub fn identity(&self) -> &FunctionIdentity {
        &self.0.origin.identity
    }
    pub fn original(&self) -> &Expression {
        &self.0.origin.original
    }
    pub fn functions(&self) -> &[Function] {
        self.0
            .origin
            .functions
            .get()
            .expect("private graph declarations were sealed before publication")
    }
    pub fn checked(&self) -> &Arc<CheckedResourceProgram> {
        &self.0.origin.checked
    }
    pub fn actor_seed_type(&self, seed: u64) -> Option<TypeId> {
        self.0
            .origin
            .proof
            .actor_graph()?
            .nodes()
            .iter()
            .find(|node| node.seed() == seed)
            .map(|node| node.ty())
    }
    pub fn field_value(&self) -> Result<&Value, String> {
        let graph = self
            .0
            .origin
            .proof
            .actor_graph()
            .ok_or("actor graph proof lost its template")?;
        if matches!(self.operation(), ActorGraphOperation::RootFill { .. }) {
            return Ok(graph.root());
        }
        let ActorGraphOperation::Fill { field, node, .. } = self.operation() else {
            return Err("actor graph instruction has no field value".into());
        };
        let graph = self
            .0
            .origin
            .proof
            .actor_graph()
            .ok_or("actor graph proof lost its template")?;
        graph
            .nodes()
            .get(*node)
            .and_then(|node| node.captures().iter().chain(node.state()).nth(*field))
            .map(|field| field.value())
            .ok_or_else(|| "actor graph field proof is absent".into())
    }
    pub fn validate(
        &self,
        function: FunctionId,
        identity: &FunctionIdentity,
        args: &[Expression],
        result: TypeId,
        types: &TypeInterner,
    ) -> Result<(), String> {
        if self.function() != function
            || self.identity() != identity
            || result != self.0.result
            || args.len() != self.0.operands.len()
        {
            return Err("actor graph instruction changed its checked owner or signature".into());
        }
        let origin = &self.0.origin;
        origin.archive.validate_types(types)?;
        if !origin.proof.belongs_to(&origin.checked)
            || origin
                .proof
                .actor_graph()
                .is_none_or(|graph| !graph.belongs_to(&origin.checked))
        {
            return Err("actor graph instruction has foreign required proof".into());
        }
        if let ActorGraphOperation::HookBox { hook, projection } = self.operation() {
            let (proof, original_hook) = origin
                .hooks
                .get(*projection)
                .ok_or("graph hook projection is absent")?;
            let checked = proof.checked_hook(&origin.checked)?;
            if hook != original_hook
                || hook.function_type() != result
                || hook.definition() != checked.definition
            {
                return Err(
                    "graph hook changed its protected required path or original producer".into(),
                );
            }
        }
        for (arg, (local, ty)) in args.iter().zip(&self.0.operands) {
            if arg.ty != *ty
                || !matches!(arg.kind, E::Local(actual) if actual == *local)
                || ty.index() as usize >= types.len()
                || types.resolve(*ty) != origin.checked.checked().interner.resolve(*ty)
            {
                return Err(
                    "actor graph instruction changed its exact original local operand or type"
                        .into(),
                );
            }
        }
        Ok(())
    }
}
#[derive(Debug)]
struct Plan {
    origin: Arc<Origin>,
    body: Block,
    output: LocalId,
    output_type: TypeId,
    instructions: Vec<ActorGraphInstruction>,
    locals: Vec<Local>,
    original_local_count: usize,
}
#[derive(Debug, Clone)]
pub struct ActorGraphPlan(Arc<Plan>);
impl PartialEq for ActorGraphPlan {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for ActorGraphPlan {}
impl ActorGraphPlan {
    pub fn required_proof(&self) -> &CheckedRequiredValue {
        &self.0.origin.proof
    }
    pub fn factory_metadata_local(&self, local: &Local, _types: &TypeInterner) -> bool {
        local.id.index() as usize >= self.0.original_local_count
            && self.0.locals.get(local.id.index() as usize) == Some(local)
    }
    pub fn body(&self) -> &Block {
        &self.0.body
    }
    pub fn function(&self) -> FunctionId {
        self.0.origin.function
    }
    pub fn output(&self) -> LocalId {
        self.0.output
    }
    pub fn output_type(&self) -> TypeId {
        self.0.output_type
    }
    pub fn instructions(&self) -> &[ActorGraphInstruction] {
        &self.0.instructions
    }
    pub fn validate(&self, function: &Function, types: &TypeInterner) -> Result<(), String> {
        if function.id != self.0.origin.function || function.identity != self.0.origin.identity {
            return Err("actor graph import changed required owner".into());
        }
        self.0.origin.archive.validate_types(types)?;
        let graph = self
            .0
            .origin
            .proof
            .actor_graph()
            .ok_or("actor graph plan lost template")?;
        if !graph.belongs_to(&self.0.origin.checked) || graph.root_type() != self.output_type() {
            return Err("actor graph plan changed checked root type".into());
        }
        // Preserve the exact independently captured local headers. A checked
        // machine-state value may initialize its widened machine local; value
        // type compatibility remains the common backend validator's obligation.
        if function.locals.get(..self.0.locals.len()) != Some(self.0.locals.as_slice()) {
            return Err("actor graph plan changed its original or generated local headers".into());
        }
        for instruction in &self.0.instructions {
            if instruction.function() != function.id || instruction.identity() != &function.identity
            {
                return Err("actor graph instruction escaped its owning occurrence".into());
            }
        }
        if graph.root_type().index() as usize >= types.len()
            || types.resolve(graph.root_type())
                != self
                    .0
                    .origin
                    .checked
                    .checked()
                    .interner
                    .resolve(graph.root_type())
        {
            return Err("actor graph root has foreign native type meaning".into());
        }
        Ok(())
    }
}

/// The common producer requires original immutable HIR and private cache proof.
/// Its only inputs are checked values and retained compiler metadata; no caller
/// can supply a graph body, positive instruction proof or actor seed map.
pub fn materialize_actor_graphs(
    program: &mut Program,
    values: &ExplicitComptimeValues,
    types: &TypeInterner,
    reflection: &ReflectionMetadata,
    methods: &HashSet<Span>,
) -> Result<(), Vec<LowerError>> {
    let archive = program.resource_source.clone();
    let result = (|| -> Result<(), String> {
        let Some(checked) = archive.checked_program() else {
            if values.has_actor_graphs() {
                return Err(
                    "actor graph materialization requires the original checked program".into(),
                );
            }
            return Ok(());
        };
        let proofs = values.checked_required_values(checked)?;
        if !proofs.iter().any(|proof| proof.actor_graph().is_some()) {
            return Ok(());
        }
        archive.validate_current(program, types)?;
        let original_functions = program.functions.clone();
        let candidates = function_value_candidates(&original_functions, methods);
        let mut functions = original_functions.clone();
        let mut rows = archive.required_materializations().to_vec();
        for function_index in 0..original_functions.len() {
            let owner = functions[function_index].clone();
            let mut occurrences = Vec::new();
            crate::resource_materialization::walk_block(
                &mut functions[function_index].body,
                &mut |expression| {
                    if !matches!(expression.kind, E::Comptime { .. }) {
                        return Ok(());
                    }
                    let selected = proofs
                        .iter()
                        .filter(|proof| {
                            crate::resource_materialization::proof_matches(
                                proof, &owner, expression, checked,
                            )
                        })
                        .collect::<Vec<_>>();
                    if let [proof] = selected.as_slice()
                        && proof.actor_graph().is_some()
                    {
                        occurrences.push((expression.clone(), (*proof).clone()));
                    }
                    Ok(())
                },
            )?;
            for (original, proof) in occurrences {
                let mut locals = functions[function_index].locals.clone();
                let plan = derive(
                    &owner,
                    &original,
                    &proof,
                    checked,
                    &archive,
                    types,
                    reflection,
                    &candidates,
                    &mut functions,
                    &mut locals,
                )?;
                functions[function_index].locals = locals;
                let mut protected_body = plan.body().clone();
                crate::resource_materialization::walk_block(
                    &mut protected_body,
                    &mut |expression| {
                        if let E::ActorGraphStep { instruction, .. } = &expression.kind
                            && let ActorGraphOperation::HookBox { hook, .. } =
                                instruction.operation()
                        {
                            rows.push(RequiredMaterialization {
                                function: owner.id,
                                identity: owner.identity.clone(),
                                original: original.clone(),
                                current: expression.clone(),
                                hook: Some(hook.clone()),
                                proof: proof.clone(),
                                projection: instruction.projected_hook().cloned(),
                            });
                        }
                        Ok(())
                    },
                )?;
                let mut installed = 0usize;
                crate::resource_materialization::walk_block(
                    &mut functions[function_index].body,
                    &mut |expression| {
                        if crate::resource_materialization::expression_equal(expression, &original)
                        {
                            expression.kind = E::ActorGraphImport(plan.clone());
                            rows.push(RequiredMaterialization {
                                function: owner.id,
                                identity: owner.identity.clone(),
                                original: original.clone(),
                                current: expression.clone(),
                                hook: None,
                                proof: proof.clone(),
                                projection: None,
                            });
                            installed += 1;
                        }
                        Ok(())
                    },
                )?;
                if installed != 1 {
                    return Err(
                        "actor graph required occurrence is not unique in its checked function"
                            .into(),
                    );
                }
            }
        }
        let required_only =
            crate::resource_materialization::required_only_functions(&archive, &functions, &rows)?;
        let view = Materialized {
            functions,
            values: rows,
            required_only,
        };
        program.functions = view.functions.clone();
        program.resource_source.graph_values = Some(Arc::new(Transition {
            checked: checked.clone(),
            original: original_functions,
            current: program.functions.clone(),
            proofs: rows_for_transition(&view.values),
        }));
        program.resource_source.install_materialized(view);
        Ok(())
    })();
    result.map_err(|message| {
        vec![LowerError {
            span: program
                .functions
                .first()
                .map_or(Span::new(jett_common::FileId::new(0), 0, 0), |f| f.span),
            message,
        }]
    })
}

#[allow(clippy::too_many_arguments)]
fn derive(
    owner: &Function,
    original: &Expression,
    proof: &CheckedRequiredValue,
    checked: &Arc<CheckedResourceProgram>,
    archive: &crate::ResourceSourceArchive,
    types: &TypeInterner,
    reflection: &ReflectionMetadata,
    candidates: &[crate::evaluated_values::FunctionValueCandidate],
    functions: &mut Vec<Function>,
    locals: &mut Vec<Local>,
) -> Result<ActorGraphPlan, String> {
    let graph = proof
        .actor_graph()
        .ok_or("required actor graph has no opaque template")?;
    if !proof.belongs_to(checked) || !graph.belongs_to(checked) || graph.root_type() != original.ty
    {
        return Err("required actor graph changed checked nominal root".into());
    }
    let span = original.span;
    let mut hooks = Vec::new();
    let manifest = archive
        .manifest()
        .ok_or("actor graph has no original hook manifest")?;
    for projection in proof.actor_graph_hooks()? {
        let original_hook = projection.checked_hook(checked)?;
        let hook = manifest
            .hook_for_definition(original_hook.definition)
            .ok_or("projected graph hook is absent from the original Resource manifest")?;
        if hook.function_type() != projection.signature()
            || hook.function_type() != original_hook.function_type
            || hook.resource_type() != original_hook.resource_type
            || hook.resource_kind().definition() != original_hook.resource_definition
        {
            return Err(
                "projected graph hook changed exact nominal signature or Resource family".into(),
            );
        }
        hook.validate(types)?;
        hooks.push((projection, hook));
    }
    let origin = Arc::new(Origin {
        checked: checked.clone(),
        proof: proof.clone(),
        function: owner.id,
        identity: owner.identity.clone(),
        original: original.clone(),
        hooks,
        functions: OnceLock::new(),
        archive: archive.clone(),
        callback_leaves: graph_callback_leaves(proof)?,
        callbacks: OnceLock::new(),
    });
    let mut seeds = ActorGraphSeeds {
        ready: HashMap::new(),
        pending: HashMap::new(),
        hooks: HashMap::new(),
        origin: origin.clone(),
    };
    let mut body = Block {
        statements: Vec::new(),
        span,
    };
    let mut instructions = Vec::new();
    let ticket = local(locals, TypeInterner::UINT64, span)?;
    push_step(
        &origin,
        ActorGraphOperation::Begin,
        TypeInterner::UINT64,
        &[],
        Some(ticket),
        span,
        &mut body,
        &mut instructions,
    );
    for node in graph.nodes() {
        let Type::Actor(id) = types.resolve(node.ty()) else {
            return Err("actor graph seed has non-actor type".into());
        };
        let declaration = types.resolve_actor(*id);
        let fields = declaration
            .capability_params
            .iter()
            .chain(&declaration.state_fields)
            .collect::<Vec<_>>();
        let frozen = node
            .captures()
            .iter()
            .chain(node.state())
            .collect::<Vec<_>>();
        if fields.len() != frozen.len()
            || fields
                .iter()
                .zip(&frozen)
                .any(|((name, ty), field)| *name != field.name() || *ty != field.ty())
        {
            return Err("actor graph changed exact constructor-owned field layout".into());
        }
        let actor = local(locals, node.ty(), span)?;
        if seeds
            .ready
            .insert(node.seed(), (node.ty(), actor))
            .is_some()
        {
            return Err("actor graph repeats a seed node".into());
        }
        push_step(
            &origin,
            ActorGraphOperation::ReserveBase {
                ty: node.ty(),
                fields: frozen.len(),
            },
            node.ty(),
            &[(ticket, TypeInterner::UINT64)],
            Some(actor),
            span,
            &mut body,
            &mut instructions,
        );
    }
    let mut pending = Vec::new();
    for value in std::iter::once(graph.root()).chain(graph.nodes().iter().flat_map(|node| {
        node.captures()
            .iter()
            .chain(node.state())
            .map(|field| field.value())
    })) {
        collect_pending(value, &mut pending);
    }
    pending.sort_unstable();
    pending.dedup();
    for (seed, depth) in pending {
        let (ty, base) = *seeds
            .ready
            .get(&seed)
            .ok_or("pending actor graph seed is absent")?;
        let alias = local(locals, ty, span)?;
        seeds.pending.insert((seed, depth), alias);
        push_step(
            &origin,
            ActorGraphOperation::ReservePending { ty, depth },
            ty,
            &[(ticket, TypeInterner::UINT64), (base, ty)],
            Some(alias),
            span,
            &mut body,
            &mut instructions,
        );
    }
    for (index, (projection, hook)) in origin.hooks.iter().enumerate() {
        let instruction = ActorGraphInstruction(Arc::new(Instruction {
            origin: origin.clone(),
            index: instructions.len(),
            operation: ActorGraphOperation::HookBox {
                hook: hook.clone(),
                projection: index,
            },
            result: projection.signature(),
            operands: vec![(ticket, TypeInterner::UINT64)],
        }));
        let expression = Expression {
            kind: E::ActorGraphStep {
                instruction: instruction.clone(),
                args: vec![Expression {
                    kind: E::Local(ticket),
                    ty: TypeInterner::UINT64,
                    span,
                }],
            },
            ty: projection.signature(),
            span,
        };
        if seeds
            .hooks
            .insert(std::ptr::from_ref(projection.value()?).addr(), expression)
            .is_some()
        {
            return Err("graph hook repeats an authenticated leaf projection".into());
        }
        instructions.push(instruction);
    }
    for (node_index, node) in graph.nodes().iter().enumerate() {
        let base = seeds.ready(node.seed(), node.ty())?;
        for (index, field) in node.captures().iter().chain(node.state()).enumerate() {
            let mut bindings = Vec::new();
            let value = value_expression(
                field.value(),
                field.ty(),
                span,
                &mut ValueContext {
                    actor_seeds: Some(&seeds),
                    types,
                    reflection,
                    functions: candidates,
                    refinement_functions: None,
                    locals,
                    bindings: &mut bindings,
                },
            )?;
            body.statements.extend(bindings);
            let stored = local(locals, field.ty(), span)?;
            body.statements.push(Statement {
                kind: S::Let {
                    local: stored,
                    value,
                },
                span,
            });
            push_step(
                &origin,
                ActorGraphOperation::Fill {
                    ty: field.ty(),
                    field: index,
                    node: node_index,
                },
                TypeInterner::NOTHING,
                &[
                    (ticket, TypeInterner::UINT64),
                    (base, node.ty()),
                    (stored, field.ty()),
                ],
                None,
                span,
                &mut body,
                &mut instructions,
            );
        }
    }
    let mut bindings = Vec::new();
    let root = value_expression(
        graph.root(),
        graph.root_type(),
        span,
        &mut ValueContext {
            actor_seeds: Some(&seeds),
            types,
            reflection,
            functions: candidates,
            refinement_functions: None,
            locals,
            bindings: &mut bindings,
        },
    )?;
    body.statements.extend(bindings);
    let output = local(locals, graph.root_type(), span)?;
    body.statements.push(Statement {
        kind: S::Let {
            local: output,
            value: root,
        },
        span,
    });
    let root_bits = local(locals, TypeInterner::UINT64, span)?;
    push_step(
        &origin,
        ActorGraphOperation::RootFill {
            ty: graph.root_type(),
        },
        TypeInterner::UINT64,
        &[(ticket, TypeInterner::UINT64), (output, graph.root_type())],
        Some(root_bits),
        span,
        &mut body,
        &mut instructions,
    );
    push_step(
        &origin,
        ActorGraphOperation::Commit,
        TypeInterner::NOTHING,
        &[(ticket, TypeInterner::UINT64)],
        None,
        span,
        &mut body,
        &mut instructions,
    );
    let published_output = local(locals, graph.root_type(), span)?;
    push_step(
        &origin,
        ActorGraphOperation::RootPublish {
            ty: graph.root_type(),
        },
        graph.root_type(),
        &[
            (ticket, TypeInterner::UINT64),
            (root_bits, TypeInterner::UINT64),
        ],
        Some(published_output),
        span,
        &mut body,
        &mut instructions,
    );
    let output = published_output;
    // Complete exact interface/callback conversions inside this private body before
    // freezing it; the later public program visitor cannot rewrite sealed data.
    let mut converted = functions[owner.id.index() as usize].clone();
    converted.locals = locals.clone();
    // Keep previous Graph and ordinary reflected construction sites visible
    // to the full seal validator while converting this private recipe.
    converted.body.statements.push(Statement {
        kind: S::Scope(body),
        span,
    });
    let mut temporary = Program {
        resource_manifest: manifest.clone(),
        resource_source: archive.clone(),
        functions: functions.to_vec(),
        equality_methods: archive
            .equality_methods()
            .cloned()
            .ok_or("graph factory lost original equality metadata")?,
    };
    temporary.functions[owner.id.index() as usize] = converted;
    temporary.resource_source.graph_values = Some(Arc::new(Transition {
        checked: checked.clone(),
        original: conversion_sources_from_archive(archive),
        current: temporary.functions.clone(),
        proofs: vec![proof.clone()],
    }));
    crate::interface_values::complete_graph_value_conversions(&mut temporary, types).map_err(
        |errors| {
            errors
                .into_iter()
                .map(|e| e.message)
                .collect::<Vec<_>>()
                .join("; ")
        },
    )?;
    let mut converted = temporary.functions[owner.id.index() as usize].clone();
    let Some(Statement {
        kind: S::Scope(converted_body),
        ..
    }) = converted.body.statements.pop()
    else {
        return Err("graph conversion lost its exact private recipe scope".into());
    };
    let mut callbacks = Vec::new();
    for function in &temporary.functions {
        for plan in crate::reflected_callable_plans(function) {
            if let Some(source) = plan.graph_source() {
                if source
                    .origin
                    .upgrade()
                    .is_some_and(|source| Arc::ptr_eq(&source, &origin))
                {
                    if !plan.is_sealed() || !plan.validate_original() {
                        return Err(
                            "graph callback adapter was not authenticated and sealed".into()
                        );
                    }
                    callbacks.push((source.leaf, plan));
                }
            }
        }
    }
    if callbacks.len() != origin.callback_leaves.len()
        || origin.callback_leaves.iter().any(|leaf| {
            callbacks
                .iter()
                .filter(|(expected, _)| expected == leaf)
                .count()
                != 1
        })
    {
        return Err("graph callback leaf has no unique sealed checked adapter".into());
    }
    origin
        .callbacks
        .set(callbacks)
        .map_err(|_| "graph callbacks were sealed twice")?;
    // Conversion can introduce callable adapters. Retain their dense IDs and
    // checked generated bodies in the common program before freezing this plan.
    let original_count = functions.len();
    functions.extend(temporary.functions.into_iter().skip(original_count));
    origin
        .functions
        .set(functions.clone())
        .map_err(|_| "actor graph declarations were sealed twice")?;
    *locals = converted.locals;
    Ok(ActorGraphPlan(Arc::new(Plan {
        origin,
        body: converted_body,
        output,
        output_type: graph.root_type(),
        instructions,
        locals: locals.clone(),
        original_local_count: owner.locals.len(),
    })))
}
fn local(locals: &mut Vec<Local>, ty: TypeId, span: Span) -> Result<LocalId, String> {
    let id = LocalId::new(
        u32::try_from(locals.len()).map_err(|_| "actor graph native local table exhausted")?,
    );
    locals.push(Local {
        id,
        name: format!("__checked_actor_graph_{}", id.index()),
        ty,
        debug_ty: ty,
        debug_type_name: None,
        mutable: false,
        view_source: None,
        span,
    });
    Ok(id)
}
#[allow(clippy::too_many_arguments)]
fn push_step(
    origin: &Arc<Origin>,
    operation: ActorGraphOperation,
    result: TypeId,
    operands: &[(LocalId, TypeId)],
    output: Option<LocalId>,
    span: Span,
    body: &mut Block,
    instructions: &mut Vec<ActorGraphInstruction>,
) {
    let instruction = ActorGraphInstruction(Arc::new(Instruction {
        origin: origin.clone(),
        index: instructions.len(),
        operation,
        result,
        operands: operands.to_vec(),
    }));
    let args = operands
        .iter()
        .map(|(local, ty)| Expression {
            kind: E::Local(*local),
            ty: *ty,
            span,
        })
        .collect();
    let value = Expression {
        kind: E::ActorGraphStep {
            instruction: instruction.clone(),
            args,
        },
        ty: result,
        span,
    };
    body.statements.push(Statement {
        kind: match output {
            Some(local) => S::Let { local, value },
            None => S::Expression(value),
        },
        span,
    });
    instructions.push(instruction);
}
fn pending_actor(mut value: &Value) -> Option<(u64, u64)> {
    let mut depth = 0u64;
    loop {
        match value {
            Value::Pending(inner) => {
                depth = depth.checked_add(1)?;
                value = inner;
            }
            Value::Typed { value: inner, .. } => value = inner,
            Value::Actor(seed) => return Some((*seed, depth)),
            _ => return None,
        }
    }
}
fn collect_pending(value: &Value, output: &mut Vec<(u64, u64)>) {
    let mut work = vec![value];
    while let Some(value) = work.pop() {
        if let Some((seed, depth)) = pending_actor(value)
            && depth != 0
        {
            output.push((seed, depth));
            continue;
        }
        match value {
            Value::Pending(inner)
            | Value::Typed { value: inner, .. }
            | Value::OptionalSome(inner)
            | Value::ResultOk(inner)
            | Value::ResultFail(inner) => work.push(inner),
            Value::List(values)
            | Value::Set(values)
            | Value::Enum { fields: values, .. }
            | Value::Machine { fields: values, .. } => work.extend(values),
            Value::Struct { fields, .. } => work.extend(fields.iter().map(|(_, value)| value)),
            Value::Map(entries) => {
                work.extend(entries.iter().flat_map(|(key, value)| [key, value]))
            }
            Value::Function { captures, .. } => work.extend(captures.values()),
            Value::TypeConstruction { fields, .. } => {
                work.extend(fields.iter().map(|(_, _, _, value)| value))
            }
            _ => {}
        }
    }
}

pub fn visit_actor_graph_factory_values(
    body: &mut Block,
    mut visit: impl FnMut(&Expression),
) -> Result<(), String> {
    crate::resource_materialization::walk_block(body, &mut |value| {
        visit(value);
        Ok(())
    })
}

/// Readonly occurrence traversal; a caller cannot mutate a sealed body.
pub fn visit_actor_graph_plans(
    body: &mut Block,
    mut visit: impl FnMut(&ActorGraphPlan),
) -> Result<(), String> {
    crate::resource_materialization::walk_block(body, &mut |expression| {
        if let E::ActorGraphImport(plan) = &expression.kind {
            visit(plan);
        }
        Ok(())
    })
}

pub fn validate_actor_graph_execution(
    program: &Program,
    types: &TypeInterner,
) -> Result<(), String> {
    let mut found = false;
    for function in &program.functions {
        let mut body = function.body.clone();
        visit_actor_graph_plans(&mut body, |plan| {
            found = true;
            let _ = plan;
        })?;
    }
    if found {
        program.resource_source.validate_current(program, types)?;
    }
    Ok(())
}

mod capture_bindings;
pub use capture_bindings::ActorGraphCaptureBinding;
#[cfg(test)]
mod tests;

/// Exact checked Graph transformations; only the private producer constructs rows.
#[derive(Debug)]
pub(crate) struct Transition {
    checked: Arc<CheckedResourceProgram>,
    original: Vec<Function>,
    current: Vec<Function>,
    proofs: Vec<CheckedRequiredValue>,
}

#[derive(Debug, Clone)]
pub(crate) struct CallbackSource {
    // The origin retains sealed plans; a weak back-reference avoids a proof cycle.
    origin: Weak<Origin>,
    leaf: usize,
    source: Expression,
}

impl CallbackSource {
    pub(crate) fn validate_site(
        &self,
        reference: &jett_comptime::value::ReflectedCallableValue,
        site: &Expression,
        owner: &Function,
        archive: &crate::ResourceSourceArchive,
    ) -> Result<(), String> {
        let E::FunctionAdapter { value, .. } = &site.kind else {
            return Err("graph callback lost its canonical adapter site".into());
        };
        let origin = self
            .origin
            .upgrade()
            .ok_or("graph callback lost its retained origin")?;
        let transition = archive
            .graph_values
            .as_ref()
            .ok_or("graph callback lacks protected owner transition")?;
        if !archive.belongs_to(&origin.checked)
            || !Arc::ptr_eq(reference.checked_program(), &origin.checked)
            || !Arc::ptr_eq(&transition.checked, &origin.checked)
            || !origin.proof.belongs_to(&origin.checked)
            || !origin.callback_leaves.contains(&self.leaf)
            || owner.id != origin.function
            || owner.identity != origin.identity
            || !transition
                .current
                .get(owner.id.index() as usize)
                .is_some_and(|current| {
                    crate::reflected_callable::source_functions_equal(
                        std::slice::from_ref(owner),
                        std::slice::from_ref(current),
                    )
                })
            || !crate::resource_materialization::expression_equal(value, &self.source)
            || site.ty != reference.requested_type()
            || value.ty != reference.source_type()
        {
            return Err(
                "graph callback changed its exact retained leaf, descriptor or protected owner"
                    .into(),
            );
        }
        Ok(())
    }
}

fn rows_for_transition(rows: &[RequiredMaterialization]) -> Vec<CheckedRequiredValue> {
    rows.iter()
        .filter(|row| row.proof.actor_graph().is_some())
        .map(|row| row.proof.clone())
        .collect()
}

fn conversion_sources_from_archive(archive: &crate::ResourceSourceArchive) -> Vec<Function> {
    archive.graph_values.as_ref().map_or_else(
        || archive.functions().to_vec(),
        |transition| transition.original.clone(),
    )
}

pub(crate) fn conversion_sources(program: &Program) -> Result<Vec<Function>, String> {
    validate_conversion_input(program).map_err(|errors| {
        errors
            .into_iter()
            .map(|e| e.message)
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    let transition = program
        .resource_source
        .graph_values
        .as_ref()
        .ok_or("graph callback has no protected Source transition")?;
    let mut source = transition.original.clone();
    let original_count = source.len();
    source.extend(program.functions.iter().skip(original_count).cloned());
    Ok(source)
}

pub(crate) fn original_function(
    archive: &crate::ResourceSourceArchive,
    function: &Function,
    checked: &Arc<CheckedResourceProgram>,
) -> Result<Option<Function>, String> {
    let Some(transition) = &archive.graph_values else {
        return Ok(None);
    };
    if !archive.belongs_to(checked)
        || !Arc::ptr_eq(&transition.checked, checked)
        || transition.proofs.is_empty()
        || transition
            .proofs
            .iter()
            .any(|proof| !proof.belongs_to(checked) || proof.actor_graph().is_none())
    {
        return Err("Graph Source restoration belongs to another checked proof".into());
    }
    let Some(current) = transition.current.get(function.id.index() as usize) else {
        return Ok(None);
    };
    if !crate::reflected_callable::source_functions_equal(
        std::slice::from_ref(function),
        std::slice::from_ref(current),
    ) {
        return Ok(None);
    }
    Ok(transition
        .original
        .get(function.id.index() as usize)
        .cloned())
}

pub(crate) fn validate_conversion_input(program: &Program) -> Result<(), Vec<LowerError>> {
    let Some(transition) = &program.resource_source.graph_values else {
        return Ok(());
    };
    if !program.resource_source.belongs_to(&transition.checked)
        || program.resource_source.manifest() != Some(&program.resource_manifest)
        || program.resource_source.equality_methods() != Some(&program.equality_methods)
        || transition.proofs.is_empty()
        || transition
            .proofs
            .iter()
            .any(|proof| !proof.belongs_to(&transition.checked) || proof.actor_graph().is_none())
        || !crate::reflected_callable::source_functions_equal(
            &program.functions,
            &transition.current,
        )
        || program
            .functions
            .iter()
            .enumerate()
            .any(|(index, function)| {
                function.id.index() as usize != index
                    || function
                        .locals
                        .iter()
                        .enumerate()
                        .any(|(index, local)| local.id.index() as usize != index)
            })
    {
        return Err(vec![LowerError {
            span: program
                .functions
                .first()
                .map_or(Span::new(jett_common::FileId::new(0), 0, 0), |f| f.span),
            message: "Graph conversion changed its protected checked whole-program input".into(),
        }]);
    }
    // The complete table, including appended adapters and every recipe site,
    // remains authenticated; callers cannot hide edited generated declarations
    // behind an original-Source prefix comparison.
    let checked_types = &transition.checked.checked().interner;
    let result = (|| -> Result<(), String> {
        program.resource_source.validate_types(checked_types)?;
        for function in &program.functions {
            let mut body = function.body.clone();
            crate::resource_materialization::walk_block(&mut body, &mut |expression| {
                match &expression.kind {
                    E::ActorGraphImport(plan) => plan.validate(function, checked_types)?,
                    E::ActorGraphStep { instruction, args } => {
                        instruction.validate(
                            function.id,
                            &function.identity,
                            args,
                            expression.ty,
                            checked_types,
                        )?;
                    }
                    _ => {}
                }
                Ok(())
            })?;
        }
        Ok(())
    })();
    result.map_err(|message| {
        vec![LowerError {
            span: program
                .functions
                .first()
                .map_or(Span::new(jett_common::FileId::new(0), 0, 0), |f| f.span),
            message,
        }]
    })?;
    Ok(())
}

pub(crate) fn complete_conversion(
    program: &mut Program,
    before: &Program,
) -> Result<(), Vec<LowerError>> {
    validate_conversion_input(before)?;
    let Some(transition) = &before.resource_source.graph_values else {
        return Ok(());
    };
    if program.resource_source != before.resource_source {
        return Err(vec![LowerError {
            span: before.functions[0].span,
            message: "Graph conversion replaced its protected original archive".into(),
        }]);
    }
    program.resource_source.graph_values = Some(Arc::new(Transition {
        checked: transition.checked.clone(),
        original: transition.original.clone(),
        current: program.functions.clone(),
        proofs: transition.proofs.clone(),
    }));
    Ok(())
}

fn graph_callback_leaves(proof: &CheckedRequiredValue) -> Result<HashSet<usize>, String> {
    fn walk(value: &Value, output: &mut HashSet<usize>) {
        match value {
            Value::ReflectedCallable(reference) => {
                output.insert(std::ptr::from_ref(reference.as_ref()).addr());
                walk(reference.source(), output);
            }
            Value::Typed { value, .. }
            | Value::Pending(value)
            | Value::OptionalSome(value)
            | Value::ResultOk(value)
            | Value::ResultFail(value) => walk(value, output),
            Value::List(values) | Value::Set(values) => {
                for value in values {
                    walk(value, output);
                }
            }
            Value::Map(entries) => {
                for (key, value) in entries {
                    walk(key, output);
                    walk(value, output);
                }
            }
            Value::Struct { fields, .. } => {
                for (_, value) in fields {
                    walk(value, output);
                }
            }
            Value::Enum { fields, .. } | Value::Machine { fields, .. } => {
                for value in fields {
                    walk(value, output);
                }
            }
            Value::Function { captures, .. } => {
                for value in captures.values() {
                    walk(value, output);
                }
            }
            Value::TypeConstruction { fields, .. } => {
                for (_, _, _, value) in fields {
                    walk(value, output);
                }
            }
            _ => {}
        }
    }
    let graph = proof
        .actor_graph()
        .ok_or("graph callback association lacks checked template")?;
    let mut output = HashSet::new();
    walk(graph.root(), &mut output);
    for node in graph.nodes() {
        for field in node.captures().iter().chain(node.state()) {
            walk(field.value(), &mut output);
        }
    }
    Ok(output)
}
