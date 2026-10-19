//! Original ordinary function, declaration and producer identity. No runtime ownership.
use super::*;
use jett_parser::ast::{CallArg, StringPart, VarDecl};
use jett_typecheck::{
    CheckedBindingFact, CheckedBindingMode, CheckedInvocationShape, CheckedInvocationTarget,
    CheckedViewSource,
};
use jett_types::{Type, TypeId};

#[derive(Clone)]
pub(in crate::interpreter) struct FunctionReceipt {
    pub(super) program: Arc<jett_typecheck::CheckedResourceProgram>,
    pub(super) item: usize,
    pub(super) definition: DefId,
    /// Exact retained concrete body, never selected by span or runtime payload.
    pub(super) instance: Option<usize>,
}

impl FunctionReceipt {
    pub(super) fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.program, &other.program)
            && self.item == other.item
            && self.definition == other.definition
            && self.instance == other.instance
    }

    pub(super) fn fact(&self, name: &Ident) -> Result<CheckedBindingFact, String> {
        let fact = self
            .facts()?
            .binding_facts
            .get(&name.span)
            .copied()
            .ok_or("temporary local view has no checked binding fact")?;
        let info = self.program.resolved().scope_table.def(fact.definition);
        if info.span != name.span
            || info.name != name.name
            || fact.declaration_span != name.span
            || !matches!(
                info.kind,
                jett_resolve::DefKind::Variable | jett_resolve::DefKind::Param
            )
        {
            return Err("temporary local view changed its binding declaration".into());
        }
        Ok(fact)
    }

    pub(super) fn has_temporary_views(&self) -> Result<bool, String> {
        Ok(storage::temporary_facts(
            &self.program,
            self.facts()?.binding_facts,
            Some(self.original()?.body.span),
        ))
    }
    pub(in crate::interpreter) fn original(&self) -> Result<&FunctionDef, String> {
        let Some(Item::Function(function)) = self.program.module().items.get(self.item) else {
            return Err("temporary local view lost its original function".into());
        };
        let info = self
            .program
            .resolved()
            .scope_table
            .definitions
            .iter()
            .find(|info| info.id == self.definition)
            .ok_or("temporary local view definition is absent from its program")?;
        let qualified = info.namespace.as_ref().map_or_else(
            || function.name.name.clone(),
            |namespace| format!("{namespace}.{}", function.name.name),
        );
        if info.kind != jett_resolve::DefKind::Function
            || !is_root_definition(&self.program, self.item, self.definition)
            || info.name != qualified
        {
            return Err("temporary local view changed its retained declaration identity".into());
        }
        let checked = self.program.checked();
        match self.instance {
            Some(index) => {
                let instance = checked
                    .generic_function_instantiations
                    .get(index)
                    .ok_or("temporary local view lost its concrete generic body")?;
                if instance.definition != self.definition
                    || instance.concrete_args.len() != function.type_params.len()
                    || instance.parameter_types.len() != function.params.len()
                    || function.type_params.is_empty()
                {
                    return Err("temporary local view changed its exact concrete body".into());
                }
            }
            None => {
                let signature = checked
                    .definition_types
                    .get(&self.definition)
                    .ok_or("temporary local view lost its checked function signature")?;
                let Type::Function {
                    params,
                    view_params,
                    ..
                } = checked.interner.resolve(*signature)
                else {
                    return Err("temporary local view has an invalid function signature".into());
                };
                if !function.type_params.is_empty()
                    || params.len() != function.params.len()
                    || view_params.len() != function.params.len()
                    || function
                        .params
                        .iter()
                        .zip(view_params)
                        .any(|(source, view)| source.view != *view)
                {
                    return Err(
                        "temporary local view changed its original parameter contract".into(),
                    );
                }
            }
        }
        Ok(function)
    }

    pub(super) fn expression(&self, expression: &Expr) -> Result<usize, String> {
        let expressions = direct_expressions(&self.original()?.body);
        let indices: Vec<_> = expressions
            .iter()
            .enumerate()
            .filter_map(|(index, original)| std::ptr::eq(*original, expression).then_some(index))
            .collect();
        match indices.as_slice() {
            [index] => Ok(*index),
            _ => Err("temporary local view needs its exact original root-body occurrence".into()),
        }
    }

    pub(super) fn source_call(
        &self,
        callee: &Expr,
        arguments: &[CallArg],
        span: Span,
    ) -> Result<&jett_typecheck::CheckedCallOwnership, String> {
        let found: Vec<_> = direct_expressions(&self.original()?.body)
            .into_iter()
            .filter(|source| {
                matches!(source, Expr::Call(original, actuals, original_span)
                | Expr::GenericCall(original, _, actuals, original_span)
                if *original_span == span && std::ptr::eq(original.as_ref(), callee)
                && actuals.len() == arguments.len()
                && (actuals.is_empty() || std::ptr::eq(actuals.as_ptr(), arguments.as_ptr())))
            })
            .collect();
        if found.len() != 1 {
            return Err("temporary local view call has no original checked occurrence".into());
        }
        self.facts()?
            .call_ownership
            .get(&span)
            .ok_or_else(|| "temporary local view call has no checked caller packet".into())
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct Projection {
    pub(super) owner: TypeId,
    pub(super) span: Span,
    pub(super) name: String,
    pub(super) fields: Vec<String>,
    pub(super) index: usize,
    pub(super) endpoint: TypeId,
}

pub(super) struct ProducerReceipt {
    pub(super) function: FunctionReceipt,
    pub(super) declaration: usize,
    pub(super) fact: CheckedBindingFact,
    pub(super) whole: TypeId,
    pub(super) path: Vec<Projection>,
}

impl ProducerReceipt {
    pub(super) fn producer(&self) -> Result<&Expr, String> {
        let Some(Stmt::VarDecl(declaration)) =
            self.function.original()?.body.stmts.get(self.declaration)
        else {
            return Err("temporary local view lost its declaration".into());
        };
        let Expr::View(inner, _) = unparen(&declaration.value) else {
            return Err("temporary local view lost its written view".into());
        };
        Ok(projection_root(inner))
    }
}

pub(super) fn function_receipt(
    program: &Arc<jett_typecheck::CheckedResourceProgram>,
    name: &str,
    namespace: &Option<String>,
    current: &FunctionDef,
) -> Result<Option<FunctionReceipt>, String> {
    if !current.type_params.is_empty() {
        return Ok(None);
    }
    // This receipt family covers ordinary root declarations only. A method or
    // generated body receives no activation; its caller still rejects required
    // temporary body/frame transport. Names can only select a refusal here.
    let items: Vec<_> = program
        .module()
        .items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            matches!(item, Item::Function(function) if function.name.span == current.name.span)
                .then_some(index)
        })
        .collect();
    if items.is_empty() {
        let claims_root = program
            .resolved()
            .scope_table
            .definitions
            .iter()
            .any(|info| {
                info.kind == jett_resolve::DefKind::Function
                    && info.name == name
                    && program
                        .module()
                        .items
                        .iter()
                        .enumerate()
                        .any(|(item, _)| is_root_definition(program, item, info.id))
            });
        return if claims_root {
            Err("temporary local view function differs from its retained checked Source".into())
        } else {
            Ok(None)
        };
    }
    let [item] = items.as_slice() else {
        return Err("temporary local view has ambiguous original function storage".into());
    };
    if crate::value::closure_capture::OriginalClosureOwner::registered(
        program, name, current, namespace,
    )
    .is_none()
    {
        return Err(
            "temporary local view function differs from its retained checked Source".into(),
        );
    }
    let definitions: Vec<_> = program
        .resolved()
        .scope_table
        .definitions
        .iter()
        .filter(|info| {
            info.kind == jett_resolve::DefKind::Function
                && info.name == name
                && is_root_definition(program, *item, info.id)
                && &info.namespace == namespace
        })
        .map(|info| info.id)
        .collect();
    let [definition] = definitions.as_slice() else {
        return Err("temporary local view function is not an exact checked declaration".into());
    };
    let receipt = FunctionReceipt {
        program: program.clone(),
        item: *item,
        definition: *definition,
        instance: None,
    };
    receipt.original()?;
    Ok(Some(receipt))
}

// Metadata lookup alone grants no activation. Every receipt still authenticates
// the complete registered Source definition, exact namespace, and checked signature.
fn is_root_definition(
    program: &jett_typecheck::CheckedResourceProgram,
    item: usize,
    definition: DefId,
) -> bool {
    let Some(Item::Function(function)) = program.module().items.get(item) else {
        return false;
    };
    crate::value::closure_capture::original_named_function(program, definition)
        .is_some_and(|original| std::ptr::eq(original, function))
}

pub(super) fn producer_receipt(
    function: &FunctionReceipt,
    declaration: &VarDecl,
) -> Result<Option<ProducerReceipt>, String> {
    let checked = function.program.checked();
    let Some(fact) = function
        .facts()?
        .binding_facts
        .get(&declaration.name.span)
        .copied()
    else {
        return Ok(None);
    };
    if !matches!(
        fact.mode,
        CheckedBindingMode::View {
            source: CheckedViewSource::Other
        }
    ) {
        return Ok(None);
    }
    let indices: Vec<_> = function
        .original()?
        .body
        .stmts
        .iter()
        .enumerate()
        .filter_map(|(index, source)| {
            matches!(source, Stmt::VarDecl(original) if std::ptr::eq(original, declaration))
                .then_some(index)
        })
        .collect();
    let [index] = indices.as_slice() else {
        return Err("temporary local view nested declaration transport remains unsupported".into());
    };
    let info = function.program.resolved().scope_table.def(fact.definition);
    if declaration.mutable
        || fact.mutable
        || fact.declaration_span != declaration.name.span
        || info.id != fact.definition
        || info.span != declaration.name.span
        || info.kind != jett_resolve::DefKind::Variable
    {
        return Err("temporary local view changed its exact immutable binding".into());
    }
    let Expr::View(inner, _) = unparen(&declaration.value) else {
        return Err("temporary local view needs its original written view".into());
    };
    let producer = projection_root(inner);
    let Expr::Call(callee, arguments, span) = producer else {
        return Err("temporary local view producer transport remains unsupported".into());
    };
    let call = checked_call(function, callee, arguments, *span)?;
    let packet = &call.packet;
    named_target(&call)?
        .ok_or("temporary local view needs an ordinary non-generic named producer")?;
    let CheckedInvocationTarget::Resolved(target) = packet.target else {
        return Err("temporary local view needs a direct ordinary named producer".into());
    };
    let CheckedInvocationShape::Function { signature_type } = packet.shape else {
        return Err("temporary local view producer is not a checked Source function".into());
    };
    let target_info = function.program.resolved().scope_table.def(target);
    let Type::Function { return_type, .. } = checked.interner.resolve(signature_type) else {
        return Err("temporary local view producer has no checked return contract".into());
    };
    if target_info.kind != jett_resolve::DefKind::Function
        || checked.definition_types.get(&target) != Some(&signature_type)
        || function
            .program
            .resolved()
            .resolutions
            .get(&unparen(callee).span())
            != Some(&target)
        || function.facts()?.type_map.get(span) != Some(return_type)
        || !ordinary_type(&checked.interner, *return_type, &mut Vec::new())
        || !matches!(checked.interner.resolve(*return_type), Type::Struct(_))
    {
        return Err("temporary local view producer lacks its exact ordinary owned return".into());
    }
    let mut path = Vec::new();
    let endpoint = checked_path(function, inner, &mut path)?;
    if path.is_empty() || endpoint != fact.ty {
        return Err("temporary local view changed its checked projection endpoint".into());
    }
    Ok(Some(ProducerReceipt {
        function: function.clone(),
        declaration: *index,
        fact,
        whole: *return_type,
        path,
    }))
}

pub(super) fn checked_path(
    function: &FunctionReceipt,
    expression: &Expr,
    path: &mut Vec<Projection>,
) -> Result<TypeId, String> {
    let checked = function.program.checked();
    let expression = unparen(expression);
    if let Expr::FieldAccess(base, field, span) = expression {
        let owner = checked_path(function, base, path)?;
        let Type::Struct(id) = checked.interner.resolve(owner) else {
            return Err("temporary local view needs finite ordinary struct projections".into());
        };
        let structure = checked.interner.resolve_struct(*id);
        if !checked.interner.nominal_type_arguments(owner).is_empty() {
            return Err(
                "temporary local view generic projection transport remains unsupported".into(),
            );
        }
        let index = structure
            .fields
            .iter()
            .position(|(name, _)| name == &field.name)
            .ok_or("temporary local view field is absent from its checked owner")?;
        let endpoint = structure.fields[index].1;
        if function.facts()?.type_map.get(span) != Some(&endpoint) {
            return Err("temporary local view changed its field type".into());
        }
        path.push(Projection {
            owner,
            span: *span,
            name: structure.name.clone(),
            fields: structure
                .fields
                .iter()
                .map(|(name, _)| name.clone())
                .collect(),
            index,
            endpoint,
        });
        Ok(endpoint)
    } else {
        function
            .facts()?
            .type_map
            .get(&expression.span())
            .copied()
            .ok_or_else(|| "temporary local view expression has no original checked type".into())
    }
}

pub(super) struct CallReceipt {
    pub(super) caller: FunctionReceipt,
    pub(super) span: Span,
    pub(super) packet: jett_typecheck::CheckedCallOwnership,
    /// Parameter order -> lexical source index, checked before any argument effects.
    pub(super) order: Vec<usize>,
}

pub(super) fn checked_call(
    function: &FunctionReceipt,
    callee: &Expr,
    arguments: &[CallArg],
    span: Span,
) -> Result<CallReceipt, String> {
    use jett_typecheck::{CheckedCalleeAccess, CheckedCallerSyntax, CheckedOwnershipContext};
    let packet = function.source_call(callee, arguments, span)?;
    let checked = function.program.checked();
    if packet.context != CheckedOwnershipContext::Ordinary
        || packet.arguments.len() != arguments.len()
    {
        return Err("temporary local view call changed its original argument frame".into());
    }
    let (parameters, views, result) = match &packet.shape {
        CheckedInvocationShape::Function { signature_type } => {
            let Type::Function {
                params,
                view_params,
                return_type,
                ..
            } = checked.interner.resolve(*signature_type)
            else {
                return Err("temporary local view call lost its checked signature".into());
            };
            if let CheckedInvocationTarget::Resolved(target) = &packet.target {
                if checked.definition_types.get(target) != Some(signature_type)
                    || function
                        .program
                        .resolved()
                        .resolutions
                        .get(&unparen(callee).span())
                        != Some(target)
                {
                    return Err("temporary local view call changed its resolved target".into());
                }
            }
            if let CheckedInvocationTarget::Generic(target) = &packet.target {
                if function.facts()?.generic_calls.get(&span) != Some(target)
                    || function
                        .program
                        .resolved()
                        .resolutions
                        .get(&unparen(callee).span())
                        != Some(&target.definition)
                {
                    return Err(
                        "temporary local view generic call changed its checked target".into(),
                    );
                }
                let instance = exact_instance(&function.program, target)?;
                let original = &checked.generic_function_instantiations[instance];
                let source = retained_function(&function.program, target.definition)?.1;
                if params != &original.parameter_types
                    || *return_type != original.return_type
                    || source.params.len() != view_params.len()
                    || source
                        .params
                        .iter()
                        .zip(view_params)
                        .any(|(parameter, view)| parameter.view != *view)
                {
                    return Err(
                        "temporary local view generic call changed its concrete signature".into(),
                    );
                }
            }
            (params.clone(), view_params.clone(), *return_type)
        }
        CheckedInvocationShape::Intrinsic {
            intrinsic,
            result_type,
            operands,
        } => {
            if packet.target != CheckedInvocationTarget::Intrinsic(*intrinsic)
                || function.facts()?.intrinsic_ids.get(&span) != Some(intrinsic)
            {
                return Err("temporary local view observer changed its closed intrinsic".into());
            }
            (
                operands.iter().map(|role| role.ty()).collect(),
                operands
                    .iter()
                    .enumerate()
                    .map(|(parameter, role)| match role {
                        jett_typecheck::CheckedIntrinsicOperandRole::View { .. } => true,
                        jett_typecheck::CheckedIntrinsicOperandRole::PrintArgument { .. }
                            if matches!(intrinsic, IntrinsicId::Print | IntrinsicId::Println) =>
                        {
                            packet
                                .arguments
                                .iter()
                                .find(|fact| fact.parameter_index == parameter)
                                .is_some_and(|fact| fact.syntax == CheckedCallerSyntax::WrittenView)
                        }
                        _ => false,
                    })
                    .collect(),
                *result_type,
            )
        }
    };
    if parameters.len() != arguments.len()
        || views.len() != arguments.len()
        || function.facts()?.type_map.get(&span) != Some(&result)
    {
        return Err("temporary local view call changed its checked arity or result".into());
    }
    let order = function
        .facts()?
        .call_argument_orders
        .get(&span)
        .map(|order| order.source_indices.clone())
        .unwrap_or_else(|| (0..arguments.len()).collect());
    if order.len() != arguments.len() {
        return Err("temporary local view call has an incomplete permutation".into());
    }
    let mut seen = vec![false; arguments.len()];
    for (source, (argument, fact)) in arguments.iter().zip(&packet.arguments).enumerate() {
        let parameter = fact.parameter_index;
        if fact.source_index != source
            || fact.source_span != argument.value.span()
            || parameter >= parameters.len()
            || seen[parameter]
            || order.get(parameter) != Some(&source)
            || function
                .facts()?
                .source_type_map
                .get(&argument.value.span())
                != Some(&fact.actual_type)
            || fact.parameter_type != parameters[parameter]
            || (fact.callee_access == CheckedCalleeAccess::View) != views[parameter]
            || (fact.syntax == CheckedCallerSyntax::WrittenView)
                != matches!(unparen(&argument.value), Expr::View(_, _))
        {
            return Err("temporary local view call changed an original lexical/formal join".into());
        }
        seen[parameter] = true;
    }
    Ok(CallReceipt {
        caller: function.clone(),
        span,
        packet: packet.clone(),
        order,
    })
}

pub(super) fn named_target(call: &CallReceipt) -> Result<Option<FunctionReceipt>, String> {
    let (definition, instance) = match &call.packet.target {
        CheckedInvocationTarget::Resolved(definition) => (*definition, None),
        CheckedInvocationTarget::Generic(target) => (
            target.definition,
            Some(exact_instance(&call.caller.program, target)?),
        ),
        _ => return Ok(None),
    };
    let info = call.caller.program.resolved().scope_table.def(definition);
    if info.kind != jett_resolve::DefKind::Function {
        return Ok(None);
    }
    let (item, source) = retained_function(&call.caller.program, definition)?;
    if source.type_params.is_empty() != instance.is_none() {
        return Err("temporary local view target changed its generic declaration contract".into());
    }
    let result = FunctionReceipt {
        program: call.caller.program.clone(),
        item,
        definition,
        instance,
    };
    result.original()?;
    Ok(Some(result))
}

fn retained_function(
    program: &jett_typecheck::CheckedResourceProgram,
    definition: DefId,
) -> Result<(usize, &FunctionDef), String> {
    let candidates: Vec<_> = program
        .module()
        .items
        .iter()
        .enumerate()
        .filter_map(|(item, source)| match source {
            Item::Function(function) if is_root_definition(program, item, definition) => {
                Some((item, function))
            }
            _ => None,
        })
        .collect();
    match candidates.as_slice() {
        [selected] => Ok(*selected),
        _ => Err("temporary local view call lost its retained named function".into()),
    }
}

fn exact_instance(
    program: &jett_typecheck::CheckedResourceProgram,
    target: &jett_typecheck::CheckedGenericCall,
) -> Result<usize, String> {
    let matching: Vec<_> = program
        .checked()
        .generic_function_instantiations
        .iter()
        .enumerate()
        .filter(|(_, instance)| {
            instance.definition == target.definition
                && instance.concrete_args == target.concrete_args
                && instance.specialization == target.specialization
        })
        .map(|(index, _)| index)
        .collect();
    match matching.as_slice() {
        [index] => Ok(*index),
        _ => Err("temporary local view generic target has no unique exact checked body".into()),
    }
}

pub(super) struct Facts<'a> {
    pub(super) type_map: &'a HashMap<Span, TypeId>,
    pub(super) source_type_map: &'a HashMap<Span, TypeId>,
    pub(super) binding_facts: &'a HashMap<Span, CheckedBindingFact>,
    pub(super) call_ownership: &'a HashMap<Span, jett_typecheck::CheckedCallOwnership>,
    pub(super) generic_calls: &'a HashMap<Span, jett_typecheck::CheckedGenericCall>,
    pub(super) intrinsic_ids: &'a HashMap<Span, IntrinsicId>,
    pub(super) intrinsic_type_arguments: &'a HashMap<Span, Vec<TypeId>>,
    pub(super) call_argument_orders: &'a HashMap<Span, jett_typecheck::CheckedCallArgumentOrder>,
    pub(super) scopes: &'a HashMap<Span, Vec<jett_typecheck::CheckedComptimeTypeBinding>>,
}

impl FunctionReceipt {
    pub(super) fn facts(&self) -> Result<Facts<'_>, String> {
        let checked = self.program.checked();
        if let Some(index) = self.instance {
            let body = checked
                .generic_function_instantiations
                .get(index)
                .filter(|body| body.definition == self.definition)
                .ok_or("temporary local view lost its exact generic body facts")?;
            Ok(Facts {
                type_map: &body.type_map,
                source_type_map: &body.source_type_map,
                binding_facts: &body.binding_facts,
                call_ownership: &body.call_ownership,
                generic_calls: &body.generic_calls,
                intrinsic_ids: &body.intrinsic_ids,
                intrinsic_type_arguments: &body.intrinsic_type_arguments,
                call_argument_orders: &body.call_argument_orders,
                scopes: &body.comptime_type_bindings,
            })
        } else {
            Ok(Facts {
                type_map: &checked.type_map,
                source_type_map: &checked.source_type_map,
                binding_facts: &checked.binding_facts,
                call_ownership: &checked.call_ownership,
                generic_calls: &checked.generic_calls,
                intrinsic_ids: &checked.intrinsic_ids,
                intrinsic_type_arguments: &checked.intrinsic_type_arguments,
                call_argument_orders: &checked.call_argument_orders,
                scopes: &checked.comptime_type_bindings,
            })
        }
    }

    pub(super) fn validate_declaration(
        &self,
        name: &str,
        namespace: &Option<String>,
        current: &FunctionDef,
    ) -> Result<(), String> {
        let info = self.program.resolved().scope_table.def(self.definition);
        if name != info.name
            || namespace != &info.namespace
            || crate::value::closure_capture::OriginalClosureOwner::registered(
                &self.program,
                name,
                current,
                namespace,
            )
            .is_none()
            || current.name.span != self.original()?.name.span
        {
            return Err(
                "temporary local view function differs from its retained checked Source".into(),
            );
        }
        Ok(())
    }

    pub(super) fn validate_registered(
        &self,
        name: &str,
        namespace: &Option<String>,
        current: &FunctionDef,
        projection: Option<&crate::checked_types::CheckedFunctionTypes>,
        arguments: &[crate::value::ClosureTypeArgument],
    ) -> Result<(), String> {
        self.validate_declaration(name, namespace, current)?;
        match self.instance {
            Some(index) => {
                let body = &self.program.checked().generic_function_instantiations[index];
                if projection.is_none_or(|projection| {
                    !crate::value::closure_capture::matches_field_function_projection(
                        projection,
                        arguments,
                        body,
                        &self.program.checked().interner,
                    )
                }) {
                    return Err(
                        "temporary local view generic frame changed its exact concrete projection"
                            .into(),
                    );
                }
            }
            None if projection.is_none() && arguments.is_empty() => {}
            None => {
                return Err("temporary local view ordinary frame acquired generic metadata".into());
            }
        }
        Ok(())
    }
}

pub(super) fn copyable(types: &jett_types::TypeInterner, ty: TypeId) -> bool {
    matches!(
        types.resolve(ty),
        Type::Int8
            | Type::Int16
            | Type::Int32
            | Type::Int64
            | Type::Uint8
            | Type::Uint16
            | Type::Uint32
            | Type::Uint64
            | Type::Float32
            | Type::Float64
            | Type::String
            | Type::Bool
            | Type::Nothing
    )
}

fn ordinary_type(
    types: &jett_types::TypeInterner,
    ty: TypeId,
    ancestors: &mut Vec<TypeId>,
) -> bool {
    if ancestors.contains(&ty) {
        return false;
    }
    ancestors.push(ty);
    let ordinary = if copyable(types, ty) || matches!(types.resolve(ty), Type::Bytes) {
        true
    } else {
        match types.resolve(ty) {
            Type::Struct(id) if types.nominal_type_arguments(ty).is_empty() => types
                .resolve_struct(*id)
                .fields
                .iter()
                .all(|(_, field)| ordinary_type(types, *field, ancestors)),
            Type::List(inner) | Type::Set(inner) | Type::Optional(inner) => {
                ordinary_type(types, *inner, ancestors)
            }
            Type::Map(first, second) | Type::Result(first, second) => {
                ordinary_type(types, *first, ancestors) && ordinary_type(types, *second, ancestors)
            }
            _ => false,
        }
    };
    ancestors.pop();
    ordinary
}

pub(super) fn unparen(mut expression: &Expr) -> &Expr {
    while let Expr::Paren(inner, _) = expression {
        expression = inner;
    }
    expression
}

fn projection_root(mut expression: &Expr) -> &Expr {
    loop {
        match unparen(expression) {
            Expr::FieldAccess(base, _, _) => expression = base,
            root => return root,
        }
    }
}

pub(super) fn direct_expressions(block: &Block) -> Vec<&Expr> {
    let mut expressions = Vec::new();
    for statement in &block.stmts {
        let expression = match statement {
            Stmt::VarDecl(value) => Some(&value.value),
            Stmt::Return(value) => value.value.as_ref(),
            Stmt::Expr(value) => Some(&value.expr),
            _ => None,
        };
        if let Some(expression) = expression {
            visit_expression(expression, &mut expressions);
        }
    }
    expressions
}

fn visit_expression<'a>(expression: &'a Expr, output: &mut Vec<&'a Expr>) {
    output.push(expression);
    match expression {
        Expr::Paren(inner, _)
        | Expr::View(inner, _)
        | Expr::Clone(inner, _)
        | Expr::Unary(_, inner, _)
        | Expr::Ok(inner, _)
        | Expr::Fail(inner, _)
        | Expr::Some(inner, _)
        | Expr::Coarsen(inner, _)
        | Expr::Declassify(inner, _)
        | Expr::Run(inner, _)
        | Expr::Join(inner, _)
        | Expr::Cancel(inner, _) => visit_expression(inner, output),
        Expr::FieldAccess(base, _, _) => visit_expression(base, output),
        Expr::Binary(left, _, right, _) => {
            visit_expression(left, output);
            visit_expression(right, output);
        }
        Expr::Call(callee, arguments, _) | Expr::GenericCall(callee, _, arguments, _) => {
            visit_expression(callee, output);
            for argument in arguments {
                visit_expression(&argument.value, output);
            }
        }
        Expr::StringInterpolation(parts, _) => {
            for part in parts {
                if let StringPart::Expr(inner) = part {
                    visit_expression(inner, output);
                }
            }
        }
        // Handle/inline/control-flow regions need distinct original scope receipts.
        _ => {}
    }
}
