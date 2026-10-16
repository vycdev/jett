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
}

impl FunctionReceipt {
    pub(super) fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.program, &other.program)
            && self.item == other.item
            && self.definition == other.definition
    }

    pub(super) fn fact(&self, name: &Ident) -> Result<CheckedBindingFact, String> {
        let checked = self.program.checked();
        let fact = checked
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
            &self.program.checked().binding_facts,
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
        if !function.type_params.is_empty() {
            return Err("temporary local views need an ordinary non-generic body".into());
        }
        let checked = self.program.checked();
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
        if params.len() != function.params.len()
            || view_params.len() != params.len()
            || function
                .params
                .iter()
                .zip(view_params)
                .any(|(source, view)| source.view != *view)
        {
            return Err("temporary local view changed its original parameter contract".into());
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
                if *original_span == span && std::ptr::eq(original.as_ref(), callee)
                && actuals.len() == arguments.len()
                && (actuals.is_empty() || std::ptr::eq(actuals.as_ptr(), arguments.as_ptr())))
            })
            .collect();
        if found.len() != 1 {
            return Err("temporary local view call has no original checked occurrence".into());
        }
        self.program
            .checked()
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
    let Some(fact) = checked.binding_facts.get(&declaration.name.span).copied() else {
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
        || checked.type_map.get(span) != Some(return_type)
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
        if checked.type_map.get(span) != Some(&endpoint) {
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
        checked
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
            (params.clone(), view_params.clone(), *return_type)
        }
        CheckedInvocationShape::Intrinsic {
            intrinsic,
            result_type,
            operands,
        } => {
            if packet.target != CheckedInvocationTarget::Intrinsic(*intrinsic)
                || checked.intrinsic_ids.get(&span) != Some(intrinsic)
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
        || checked.type_map.get(&span) != Some(&result)
    {
        return Err("temporary local view call changed its checked arity or result".into());
    }
    let order = checked
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
            || checked.source_type_map.get(&argument.value.span()) != Some(&fact.actual_type)
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
    let CheckedInvocationTarget::Resolved(definition) = &call.packet.target else {
        return Ok(None);
    };
    let info = call.caller.program.resolved().scope_table.def(*definition);
    if info.kind != jett_resolve::DefKind::Function {
        return Ok(None);
    }
    let candidates: Vec<_> = call
        .caller
        .program
        .module()
        .items
        .iter()
        .enumerate()
        .filter_map(|(item, _)| {
            is_root_definition(&call.caller.program, item, *definition).then_some(item)
        })
        .collect();
    let [item] = candidates.as_slice() else {
        return Err("temporary local view call lost its retained named function".into());
    };
    let result = FunctionReceipt {
        program: call.caller.program.clone(),
        item: *item,
        definition: *definition,
    };
    if let Item::Function(function) = &result.program.module().items[*item] {
        if !function.type_params.is_empty() {
            return Ok(None);
        }
    }
    result.original()?;
    Ok(Some(result))
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
