//! Private ordinary local-view transport. Public Value remains an owned carrier.
//! Exact checked activations retain whole owners through lexical cleanup.
use super::*;
use jett_resolve::DefId;
use jett_typecheck::{CheckedBindingFact, CheckedBindingMode, CheckedViewSource};
use jett_types::{Type, TypeId};

mod calls;
mod receipts;
mod storage;
pub(super) use calls::PreparedNamedFrame;
use receipts::{FunctionReceipt, ProducerReceipt, Projection};
pub(super) use storage::{ActivationId, Context};

/// An owned successful output is carried through cleanup, rather than an
/// ambient flag that could suppress another invocation's return boundary.
pub(in crate::interpreter) enum BlockOutput {
    Ordinary(Option<Signal>),
    Acquired(AcquiredOutput),
}
pub(in crate::interpreter) struct AcquiredOutput {
    activation: ActivationId,
    function: FunctionReceipt,
    value: Value,
    source: Option<String>,
}
impl BlockOutput {
    pub(in crate::interpreter) fn into_signal(self) -> Option<Signal> {
        match self {
            Self::Ordinary(signal) => signal,
            Self::Acquired(output) => Some(Signal::Return(output.value, output.source)),
        }
    }
}
impl AcquiredOutput {
    pub(in crate::interpreter) fn into_registered_value(
        self,
        interpreter: &Interpreter,
        activation: Option<ActivationId>,
        function: Option<&FunctionReceipt>,
    ) -> Result<Value, String> {
        let expected = function.ok_or("temporary output lost its checked function receipt")?;
        let current = interpreter
            .temporary_local_views
            .as_ref()
            .and_then(Context::current)
            .ok_or("temporary output lost its completed activation")?;
        interpreter.temporary_program()?;
        if activation != Some(self.activation)
            || current.id != self.activation
            || current.root_active
            || !storage::same_function(expected, &self.function)?
            || !storage::same_function(&current.function, &self.function)?
        {
            return Err("temporary output belongs to a different function activation".into());
        }
        Ok(self.value)
    }
}

impl Interpreter {
    pub(in crate::interpreter) fn set_temporary_entry_allowed(&mut self, allowed: bool) -> bool {
        self.temporary_local_views
            .as_mut()
            .is_some_and(|context| std::mem::replace(&mut context.entry_allowed, allowed))
    }
    fn temporary_program(
        &self,
    ) -> Result<Option<Arc<jett_typecheck::CheckedResourceProgram>>, String> {
        let Some(context) = &self.temporary_local_views else {
            return Ok(None);
        };
        let (retained, _) = self
            .retained_resource_program
            .as_ref()
            .ok_or("temporary local views lost the installed checked program")?;
        if !Arc::ptr_eq(retained, &context.program) {
            return Err("temporary local views reject a foreign installed program context".into());
        }
        let Some(types) = &self.checked_expression_types else {
            return Err("temporary local views need the canonical checked type projection".into());
        };
        if !types.matches_original_program(&context.program) {
            return Err(
                "temporary local views reject a foreign or edited checked type projection".into(),
            );
        }
        Ok(Some(context.program.clone()))
    }

    /// Raw entries never gain a checked local-view activation from names or spans.
    pub(in crate::interpreter) fn prepare_temporary_function(
        &self,
        name: &str,
        namespace: &Option<String>,
        current: &FunctionDef,
        source: bool,
        frame: Option<&PreparedNamedFrame>,
    ) -> Result<Option<FunctionReceipt>, String> {
        let Some(context) = &self.temporary_local_views else {
            return Ok(None);
        };
        let Some(receipt) = receipts::function_receipt(&context.program, name, namespace, current)?
        else {
            if frame.is_some() || Context::body_needed(&context.program, Some(current.body.span)) {
                return Err(
                    "temporary local view generic call transport remains unsupported".into(),
                );
            }
            return Ok(None);
        };
        if Context::scoped_body_needed(&context.program, current.body.span) {
            return Err(
                "temporary local view reflected/comptime body transport remains unsupported".into(),
            );
        }
        let admitted = source
            && self.allow_checked_refinement_proofs
            && (frame.is_some() || context.entry_allowed);
        if !admitted {
            return if receipt.has_temporary_views()? {
                Err("temporary local view body needs its original checked entry or caller".into())
            } else {
                Ok(None)
            };
        }
        self.temporary_program()?;
        if let Some(frame) = frame {
            frame.validate_target(self, &receipt)?;
        }
        Ok(Some(receipt))
    }

    pub(in crate::interpreter) fn enter_temporary_function(
        &mut self,
        receipt: Option<FunctionReceipt>,
    ) -> Result<Option<ActivationId>, String> {
        match receipt {
            Some(receipt) => self
                .temporary_local_views
                .as_mut()
                .ok_or("temporary local view context disappeared")?
                .enter(receipt)
                .map(Some),
            None => Ok(None),
        }
    }

    pub(in crate::interpreter) fn leave_temporary_function(
        &mut self,
        activation: Option<ActivationId>,
    ) -> Result<(), String> {
        if let Some(activation) = activation {
            self.temporary_local_views
                .as_mut()
                .ok_or("temporary local view context disappeared")?
                .leave(activation)?;
        }
        Ok(())
    }

    fn temporary_alias(&self, expression: &Expr) -> Result<Option<storage::AliasTicket>, String> {
        let Some(context) = &self.temporary_local_views else {
            return Ok(None);
        };
        let Some(current) = context.current() else {
            return Ok(None);
        };
        // A raw/nested unsupported body cannot borrow a caller's placeholder.
        if !current.root_active {
            return Ok(None);
        }
        self.temporary_program()?;
        context.alias_for(&current.function, expression, self.lexical_scope_floor)
    }

    pub(in crate::interpreter) fn eval_temporary_read(
        &mut self,
        expression: &Expr,
    ) -> Result<Option<ExprFlow>, String> {
        let (operand, explicit_clone) = match expression {
            Expr::Clone(inner, _) => (inner.as_ref(), true),
            Expr::Ident(_) | Expr::FieldAccess(_, _, _) | Expr::View(_, _) => (expression, false),
            _ => return Ok(None),
        };
        let Some(ticket) = self.temporary_alias(operand)? else {
            return Ok(None);
        };
        let context = self
            .temporary_local_views
            .as_mut()
            .ok_or("temporary local view context disappeared")?;
        if explicit_clone {
            return Ok(Some(ExprFlow::Value(context.explicit_clone(&ticket)?)));
        }
        if !receipts::copyable(&context.program.checked().interner, ticket.endpoint) {
            return Err("temporary local view needs a checked observer, forwarding binding, or explicit clone".into());
        }
        // Only implicitly copyable endpoints reach an owned Value result here.
        let value = context.observe(&ticket)?.clone();
        Ok(Some(ExprFlow::Value(value)))
    }

    pub(in crate::interpreter) fn exec_temporary_binding(
        &mut self,
        declaration: &jett_parser::ast::VarDecl,
    ) -> Result<Option<Option<Signal>>, String> {
        let Some(context) = &self.temporary_local_views else {
            return Ok(None);
        };
        let required = Context::body_needed(&context.program, Some(declaration.name.span));
        let unsupported = || {
            "temporary local view declaration needs its checked ordinary root-body adapter"
                .to_string()
        };
        let Some(current) = context.current() else {
            return if required {
                Err(unsupported())
            } else {
                Ok(None)
            };
        };
        if !current.root_active {
            return if required {
                Err(unsupported())
            } else {
                Ok(None)
            };
        }
        let function = current.function.clone();
        self.temporary_program()?;
        let Some(fact) = function
            .program
            .checked()
            .binding_facts
            .get(&declaration.name.span)
            .copied()
        else {
            return if required {
                Err(unsupported())
            } else {
                Ok(None)
            };
        };
        if !matches!(fact.mode, CheckedBindingMode::View { .. }) {
            return Ok(None);
        }
        if let Some(ticket) = self.temporary_alias(&declaration.value)? {
            let original = function.original()?.body.stmts.iter().any(|statement| matches!(statement, Stmt::VarDecl(original) if std::ptr::eq(original, declaration)));
            let source = match fact.mode {
                CheckedBindingMode::View {
                    source: CheckedViewSource::Binding(source),
                } => source,
                _ => {
                    return Err(
                        "temporary local view forwarding lost its checked binding source".into(),
                    );
                }
            };
            let root = alias_root(&declaration.value)
                .ok_or("temporary local view forwarding needs its original alias root")?;
            if !original
                || declaration.mutable
                || fact.mutable
                || fact.ty != ticket.endpoint
                || function.fact(&declaration.name)? != fact
                || function.program.resolved().resolutions.get(&root.span) != Some(&source)
            {
                return Err(
                    "temporary local view forwarding changed its exact immutable declaration"
                        .into(),
                );
            }
            return self
                .publish_temporary_alias(fact, &declaration.name.name, ticket, &declaration.ty)
                .map(|()| Some(None));
        }
        let Some(receipt) = receipts::producer_receipt(&function, declaration)? else {
            return Ok(None);
        };
        if !matches!(
            function.program.checked().interner.resolve(fact.ty),
            Type::Bytes
        ) {
            return Err(
                "temporary local view endpoint transport remains unsupported beyond bytes".into(),
            );
        }
        // Reserve only metadata. This slot cannot be published or observed yet.
        let slot = self
            .temporary_local_views
            .as_mut()
            .ok_or("temporary local view context disappeared")?
            .reserve(receipt)?;
        // Borrow original producer via a separate Arc; nested calls may mutate context.
        let original = function.original()?;
        let Stmt::VarDecl(original_declaration) = original.body.stmts.iter()
            .find(|statement| matches!(statement, Stmt::VarDecl(value) if std::ptr::eq(value, declaration)))
            .ok_or("temporary local view lost its original declaration before evaluation")?
        else { return Err("temporary local view lost its original declaration before evaluation".into()); };
        let Expr::View(projected, _) = receipts::unparen(&original_declaration.value) else {
            return Err("temporary local view lost its written initializer".into());
        };
        let producer = projection_root(projected);
        let value = match self.eval_expr_flow(producer)? {
            ExprFlow::Value(value) => value,
            ExprFlow::Signal(signal) => return Ok(Some(Some(signal))),
            ExprFlow::Resource(_) => {
                return Err("temporary local views cannot adopt Resource custody".into());
            }
        };
        // The authentic named producer already normalizes its complete declared
        // return. acquire validates that exact whole type without taking a field.
        let ticket = self
            .temporary_local_views
            .as_mut()
            .ok_or("temporary local view context disappeared")?
            .acquire(slot, value)?;
        self.publish_temporary_alias(fact, &declaration.name.name, ticket, &declaration.ty)?;
        Ok(Some(None))
    }

    fn publish_temporary_alias(
        &mut self,
        fact: CheckedBindingFact,
        name: &str,
        ticket: storage::AliasTicket,
        ty: &TypeExpr,
    ) -> Result<(), String> {
        let scope = self
            .scopes
            .len()
            .checked_sub(1)
            .ok_or("temporary local view has no lexical storage")?;
        let ty = self.substitute_type_expr(ty);
        // Placeholder has no bytes/aggregate payload; only the private ticket
        // can observe the owner. get_variable shields it from raw Value APIs.
        self.temporary_local_views
            .as_mut()
            .ok_or("temporary local view context disappeared")?
            .publish(fact, name, ticket, scope)?;
        self.set_variable_with_type(name, Value::Nothing, ty);
        Ok(())
    }

    pub(in crate::interpreter) fn stage_temporary_parameters(
        &mut self,
        function: &FunctionDef,
        arguments: Vec<Value>,
        source_types: Option<&[Option<String>]>,
        frame: Option<&PreparedNamedFrame>,
    ) -> Result<Vec<(Value, TypeExpr)>, String> {
        if let Some(frame) = frame {
            frame.validate_values(self, function, &arguments, source_types)?;
        }
        let mut staged = Vec::with_capacity(arguments.len());
        for (index, (parameter, value)) in function.params.iter().zip(arguments).enumerate() {
            let ty = self.substitute_type_expr(&parameter.ty);
            let value = if frame.is_some_and(|frame| frame.alias(index).is_some()) {
                value // private Nothing placeholder; checked above, no Value clone
            } else {
                let source = source_types
                    .and_then(|types| types.get(index))
                    .and_then(Option::as_deref);
                self.normalize_and_validate_value_from_source(&ty, value, source)?
            };
            staged.push((value, ty));
        }
        Ok(staged)
    }

    pub(in crate::interpreter) fn install_temporary_parameter(
        &mut self,
        index: usize,
        parameter: &Param,
        frame: Option<&PreparedNamedFrame>,
    ) -> Result<(), String> {
        let Some(frame) = frame else {
            return Ok(());
        };
        let Some(ticket) = frame.alias(index) else {
            return Ok(());
        };
        let fact = frame.callee.fact(&parameter.name)?;
        let scope = self
            .scopes
            .len()
            .checked_sub(1)
            .ok_or("temporary local view parameter has no storage")?;
        self.temporary_local_views
            .as_mut()
            .ok_or("temporary local view context disappeared")?
            .publish(fact, &parameter.name.name, ticket.clone(), scope)
    }

    pub(in crate::interpreter) fn begin_temporary_block(
        &mut self,
        block: &Block,
    ) -> Result<Option<ActivationId>, String> {
        let scope = self
            .scopes
            .len()
            .checked_sub(1)
            .ok_or("temporary local view body has no storage")?;
        self.temporary_local_views
            .as_mut()
            .map_or(Ok(None), |context| context.begin(block, scope))
    }

    pub(in crate::interpreter) fn acquire_temporary_output(
        &mut self,
        activation: Option<ActivationId>,
        result: Result<Option<Signal>, String>,
    ) -> Result<BlockOutput, String> {
        let Some(activation) = activation else {
            return result.map(BlockOutput::Ordinary);
        };
        let function = self
            .temporary_local_views
            .as_ref()
            .and_then(Context::current)
            .filter(|current| current.id == activation)
            .ok_or("temporary local view output has stale activation")?
            .function
            .clone();
        let result = result?;
        let (value, source) = match result {
            Some(Signal::Return(value, source)) => (value, source),
            None => (Value::Nothing, None),
            other => return Ok(BlockOutput::Ordinary(other)),
        };
        let value = match &function.original()?.return_type {
            Some(ty) => {
                let ty = self.substitute_type_expr(ty);
                self.normalize_and_validate_value_from_source(&ty, value, source.as_deref())?
            }
            None => value,
        };
        self.temporary_local_views
            .as_mut()
            .ok_or("temporary local view context disappeared")?
            .audit
            .push(storage::AuditEvent::Output { activation });
        Ok(BlockOutput::Acquired(AcquiredOutput {
            activation,
            function,
            value,
            source,
        }))
    }

    pub(in crate::interpreter) fn complete_temporary_block(
        &mut self,
        activation: Option<ActivationId>,
    ) -> Result<(), String> {
        if let Some(activation) = activation {
            self.temporary_local_views
                .as_mut()
                .ok_or("temporary local view context disappeared")?
                .complete(activation)?;
        }
        Ok(())
    }
}

fn alias_root(mut expression: &Expr) -> Option<&Ident> {
    loop {
        match receipts::unparen(expression) {
            Expr::View(inner, _) | Expr::FieldAccess(inner, _, _) => expression = inner,
            Expr::Ident(root) => return Some(root),
            _ => return None,
        }
    }
}

fn projection_root(mut expression: &Expr) -> &Expr {
    loop {
        match receipts::unparen(expression) {
            Expr::FieldAccess(base, _, _) => expression = base,
            root => return root,
        }
    }
}
