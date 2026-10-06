//! Immutable named callable provenance, derived from the whole checked archive.
//! Descriptor storage stays ordinary; this proof only selects a Source body.
use super::*;
use hir::ExpressionKind as E;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceNamedCallableProof {
    function: FunctionId,
    signature_type: TypeId,
    identity: FunctionIdentity,
}
impl ResourceNamedCallableProof {
    pub fn function(&self) -> FunctionId {
        self.function
    }
    pub fn signature_type(&self) -> TypeId {
        self.signature_type
    }
    pub fn identity(&self) -> &FunctionIdentity {
        &self.identity
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct NamedCallableBinding {
    original_header: Local,
    original_value: Expression,
    current_header: Local,
    current_value: Expression,
    parent: Option<usize>,
    root: usize,
    proof: ResourceNamedCallableProof,
}

fn alias(value: &Expression) -> Option<LocalId> {
    match &value.kind {
        E::Local(local) => Some(*local),
        E::Clone(inner) if inner.ty == value.ty && inner.span == value.span => alias(inner),
        _ => None,
    }
}

fn target(
    archive: &hir::ResourceSourceArchive,
    function: FunctionId,
    signature_type: TypeId,
    types: &TypeInterner,
) -> Option<ResourceNamedCallableProof> {
    let declaration = archive.functions().get(function.index() as usize)?;
    let Type::Function {
        params,
        view_params,
        return_type,
    } = types.resolve(signature_type)
    else {
        return None;
    };
    if declaration.id != function
        || declaration.source_definition.is_none()
        || !matches!(declaration.debug_kind, hir::FunctionDebugKind::Named(_))
        || declaration.identity.declaration.kind != hir::DeclarationKind::Function
        || !declaration.identity.type_arguments.is_empty()
        || !declaration.identity.scoped_type_bindings.is_empty()
        || declaration.identity.specialization != Default::default()
        || declaration.capture_count != 0
        || declaration.params.len() != params.len()
        || view_params.len() != params.len()
        || declaration.return_type != *return_type
        || declaration
            .params
            .iter()
            .zip(params)
            .zip(view_params)
            .any(|((parameter, ty), view)| {
                parameter.ty != *ty || (parameter.mode == ParamMode::View) != *view
            })
    {
        return None;
    }
    Some(ResourceNamedCallableProof {
        function,
        signature_type,
        identity: declaration.identity.clone(),
    })
}

fn original_callee(value: &Expression) -> Option<&Expression> {
    let E::IndirectCall {
        callee,
        ownership: hir::CallOwnership::Source(source),
        ..
    } = &value.kind
    else {
        return None;
    };
    let hir::CallTarget::Indirect { signature_type } = source.target else {
        return None;
    };
    (callee.ty == signature_type
        && source.bridge == hir::CallBridge::Direct
        && source.generated_operands.is_empty()
        && source
            .arguments
            .iter()
            .all(|argument| argument.staging == hir::ArgumentStaging::Original))
    .then_some(callee)
}

/// The first supported producer is an original root-scope immutable Let.
/// Unsupported producers/escapes retain the existing pending ABI refusal.
pub(super) fn capture(
    original: &hir::Function,
    archive: &hir::ResourceSourceArchive,
    types: &TypeInterner,
    execution: &ResourceExecutionClosure,
) -> Vec<NamedCallableBinding> {
    let mut bindings: Vec<NamedCallableBinding> = Vec::new();
    let mut permitted = BTreeSet::new();
    for statement in &original.body.statements {
        let hir::StatementKind::Let { local, value } = &statement.kind else {
            continue;
        };
        let Some(header) = original
            .locals
            .get(local.index() as usize)
            .filter(|header| {
                header.id == *local
                    && !header.mutable
                    && header.view_source.is_none()
                    && header.ty == value.ty
            })
        else {
            continue;
        };
        let (proof, parent, root) = if let E::FunctionRef(function) = value.kind {
            let Some(proof) = target(archive, function, value.ty, types) else {
                continue;
            };
            (proof, None, bindings.len())
        } else if let Some(source) = alias(value) {
            let Some((index, parent)) = bindings
                .iter()
                .enumerate()
                .find(|(_, binding)| binding.original_header.id == source)
            else {
                continue;
            };
            if parent.proof.signature_type != value.ty {
                continue;
            }
            walk::expression(value, &mut |leaf| {
                if matches!(leaf.kind, E::Local(id) if id == source) {
                    permitted.insert(std::ptr::from_ref(leaf).addr());
                }
            });
            (parent.proof.clone(), Some(index), parent.root)
        } else {
            continue;
        };
        bindings.push(NamedCallableBinding {
            original_header: header.clone(),
            original_value: value.clone(),
            current_header: header.clone(),
            current_value: value.clone(),
            parent,
            root,
            proof,
        });
    }
    let mut reached = BTreeSet::new();
    walk::hir_block(&original.body, &mut |value| {
        if let Some(callee) = original_callee(value)
            && let E::Local(local) = callee.kind
            && let Some(binding) = bindings
                .iter()
                .find(|binding| binding.original_header.id == local)
            && binding.proof.signature_type == callee.ty
            && source_custody_call(value, types, execution)
        {
            permitted.insert(std::ptr::from_ref(callee).addr());
            reached.insert(binding.root);
        }
    });
    let mut escaped = BTreeSet::new();
    metadata_escapes(&original.body, &bindings, &mut escaped);
    walk::hir_block(&original.body, &mut |value| {
        for binding in &bindings {
            let escapes = match &value.kind {
                E::Local(local) => {
                    *local == binding.original_header.id
                        && !permitted.contains(&std::ptr::from_ref(value).addr())
                }
                E::ClosureRef { captures, .. } => captures.contains(&binding.original_header.id),
                _ => false,
            };
            if escapes {
                escaped.insert(binding.root);
            }
        }
    });
    // Remap only private binding indexes. Archival Local/Function IDs stay exact.
    let mut map = vec![None; bindings.len()];
    let mut next = 0;
    for (index, binding) in bindings.iter().enumerate() {
        if reached.contains(&binding.root) && !escaped.contains(&binding.root) {
            map[index] = Some(next);
            next += 1;
        }
    }
    bindings
        .into_iter()
        .filter_map(|mut binding| {
            binding.root = map[binding.root]?;
            binding.parent = binding.parent.and_then(|parent| map[parent]);
            Some(binding)
        })
        .collect()
}

fn header_same(original: &Local, current: &Local) -> bool {
    let mut restored = current.clone();
    restored.id = original.id;
    restored == *original
}

pub(super) fn validate(
    witness: &ResourceLoweringWitness,
    function: &Function,
    types: &TypeInterner,
) -> Result<(), String> {
    let expected = capture(
        &witness.original,
        &witness.source,
        types,
        &witness.execution,
    );
    if expected.len() != witness.named_callables.len() {
        return Err("Resource named callable lost its original producer/use proof".into());
    }
    for (binding, expected) in witness.named_callables.iter().zip(&expected) {
        if binding.original_header != expected.original_header
            || !crate::breakpoint_regions::expressions_equal(
                &binding.original_value,
                &expected.original_value,
            )
            || binding.parent != expected.parent
            || binding.root != expected.root
            || binding.proof != expected.proof
            || !header_same(&binding.original_header, &binding.current_header)
            || function.local(binding.current_header.id) != Some(&binding.current_header)
        {
            return Err("Resource named callable differs from its exact archived declaration or immutable binding".into());
        }
        let mut restored = binding.current_value.clone();
        let mut valid = true;
        remap_expression(&mut restored, &mut |local| {
            if let Some(original) = witness
                .named_callables
                .iter()
                .find(|candidate| candidate.current_header.id == *local)
            {
                *local = original.original_header.id;
            } else {
                valid = false;
            }
        });
        if !valid
            || !crate::breakpoint_regions::expressions_equal(&binding.original_value, &restored)
        {
            return Err("Resource named callable changed its exact original initializer".into());
        }
        let count = function
            .blocks
            .iter()
            .flat_map(|block| &block.statements)
            .filter(|statement| {
                matches!(&statement.kind, StatementKind::Let { local, value }
                if *local == binding.current_header.id
                    && crate::breakpoint_regions::expressions_equal(value, &binding.current_value))
            })
            .count();
        if count != 1 {
            return Err("Resource named callable has no unique current immutable producer".into());
        }
    }
    current_values(witness, function)?;
    Ok(())
}

fn metadata_escapes(
    block: &hir::Block,
    bindings: &[NamedCallableBinding],
    escaped: &mut BTreeSet<usize>,
) {
    for statement in &block.statements {
        let locals: &[LocalId] = match &statement.kind {
            hir::StatementKind::Trace(local) => std::slice::from_ref(local),
            hir::StatementKind::Breakpoint { bindings, .. } => bindings,
            _ => &[],
        };
        for binding in bindings {
            if locals.contains(&binding.original_header.id) {
                escaped.insert(binding.root);
            }
        }
        match &statement.kind {
            hir::StatementKind::If {
                then_block,
                else_block,
                ..
            } => {
                metadata_escapes(then_block, bindings, escaped);
                if let Some(block) = else_block {
                    metadata_escapes(block, bindings, escaped);
                }
            }
            hir::StatementKind::While { body, .. }
            | hir::StatementKind::For { body, .. }
            | hir::StatementKind::Scope(body) => metadata_escapes(body, bindings, escaped),
            hir::StatementKind::Match { arms, .. } => {
                for arm in arms {
                    metadata_escapes(&arm.body, bindings, escaped);
                }
            }
            hir::StatementKind::ReflectedTypeDispatch { arms, .. } => {
                for arm in arms {
                    metadata_escapes(&arm.body, bindings, escaped);
                }
            }
            _ => {}
        }
    }
}

/// Original aliases/callees are rejoined to exact current occurrences, independently
/// of the constructor graph equality check. A copied/resealed graph adds no use role.
pub(super) fn current_values(
    witness: &ResourceLoweringWitness,
    function: &Function,
) -> Result<Vec<(usize, ResourceNamedCallableProof)>, String> {
    let mut values = BTreeMap::new();
    for binding in &witness.named_callables {
        for statement in function.blocks.iter().flat_map(|block| &block.statements) {
            if let StatementKind::Let { local, value } = &statement.kind
                && *local == binding.current_header.id
                && crate::breakpoint_regions::expressions_equal(value, &binding.current_value)
            {
                walk::expression(value, &mut |value| {
                    values.insert(std::ptr::from_ref(value).addr(), binding.proof.clone());
                });
            }
        }
    }
    for block in &function.blocks {
        walk::mir_block(block, &mut |value| {
            if let E::IndirectCall { callee, .. } = &value.kind
                && let E::Local(local) = callee.kind
                && let Some(binding) = witness.named_callables.iter().find(|binding| binding.current_header.id == local)
                && witness.calls.iter().any(|record| {
                    crate::breakpoint_regions::expressions_equal(&record.current, value)
                        && matches!(&record.original.kind, E::IndirectCall { callee, .. } if binding.original_callee_matches(callee))
                })
            {
                values.insert(std::ptr::from_ref(callee.as_ref()).addr(), binding.proof.clone());
            }
        });
    }
    let mut escaped = false;
    for block in &function.blocks {
        walk::mir_block(block, &mut |value| {
            for binding in &witness.named_callables {
                escaped |= match &value.kind {
                    E::Local(local) => {
                        *local == binding.current_header.id
                            && !values.contains_key(&std::ptr::from_ref(value).addr())
                    }
                    E::ClosureRef { captures, .. } => captures.contains(&binding.current_header.id),
                    _ => false,
                };
            }
        });
        for statement in &block.statements {
            let locals: &[LocalId] = match &statement.kind {
                StatementKind::Trace(local) => std::slice::from_ref(local),
                StatementKind::Breakpoint { bindings, .. } => bindings,
                _ => &[],
            };
            escaped |= witness
                .named_callables
                .iter()
                .any(|binding| locals.contains(&binding.current_header.id));
        }
    }
    if escaped {
        return Err(
            "Resource named callable has an unproved current escape or callee occurrence".into(),
        );
    }
    Ok(values.into_iter().collect())
}

pub(super) fn remap_locals(
    bindings: &mut [NamedCallableBinding],
    remap: &mut impl FnMut(&mut LocalId),
) {
    for binding in bindings {
        remap(&mut binding.current_header.id);
        remap_expression(&mut binding.current_value, remap);
    }
}

fn remap_expression(value: &mut Expression, remap: &mut impl FnMut(&mut LocalId)) {
    let mut wrapper = BasicBlock {
        id: BlockId(0),
        statements: vec![Statement {
            kind: StatementKind::Evaluate(value.clone()),
            span: value.span,
        }],
        terminator: Terminator {
            kind: TerminatorKind::Unreachable,
            span: value.span,
        },
    };
    crate::sequences::prune::block_locals(&mut wrapper, remap, &mut |_| {});
    if let StatementKind::Evaluate(remapped) = wrapper.statements.remove(0).kind {
        *value = remapped;
    }
}

impl NamedCallableBinding {
    pub(super) fn original_callee_matches(&self, callee: &Expression) -> bool {
        callee.ty == self.proof.signature_type
            && matches!(callee.kind, E::Local(local) if local == self.original_header.id)
    }
    pub(super) fn local(&self) -> LocalId {
        self.current_header.id
    }
    pub(super) fn parent(&self) -> Option<usize> {
        self.parent
    }
    pub(super) fn initializer(&self) -> &Expression {
        &self.current_value
    }
    pub(super) fn proof(&self) -> &ResourceNamedCallableProof {
        &self.proof
    }
}

#[cfg(test)]
#[path = "named_callables_tests.rs"]
mod tests;
