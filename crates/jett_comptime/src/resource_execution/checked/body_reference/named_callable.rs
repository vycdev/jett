//! Exact named Source descriptors contain no physical Resource or capture environment.
//! Their function signature remains Resource-bearing; only original occurrence
//! provenance permits ordinary storage and the dedicated checked indirect bridge.

use super::*;

pub(crate) struct PreparedNamedCallable {
    key: CheckedAttemptKey,
    expression: usize,
    definition: DefId,
    signature: TypeId,
    aliases: Vec<(Span, DefId)>,
}

impl PreparedNamedCallable {
    pub(crate) fn definition(&self) -> DefId {
        self.definition
    }
}

impl CheckedExecution {
    pub(crate) fn prepare_named_callable(
        &self,
        expression: &Expr,
    ) -> Result<Option<PreparedNamedCallable>, ResourceExecutionError> {
        self.validate_executable_cursor(&self.body)?;
        // Scoped/generic callable adaptation needs its own complete body proof.
        if !matches!(self.body.root, BodyRoot::Ordinary) || !self.body.scopes.is_empty() {
            return Ok(None);
        }
        let reference = CheckedBodyReference {
            cursor: self.body.clone(),
        };
        let region = reference.region()?;
        let index = expression_index(region, expression)?;
        let mut aliases = Vec::new();
        let Some((definition, signature)) =
            self.named_callable_in(region, expression, &mut aliases)?
        else {
            return Ok(None);
        };
        Ok(Some(PreparedNamedCallable {
            key: self.attempt_key(&self.body)?,
            expression: index,
            definition,
            signature,
            aliases,
        }))
    }

    fn named_callable_in(
        &self,
        region: OriginalRegion<'_>,
        expression: &Expr,
        aliases: &mut Vec<(Span, DefId)>,
    ) -> Result<Option<(DefId, TypeId)>, ResourceExecutionError> {
        if let Expr::Paren(inner, _) = expression {
            return self.named_callable_in(region, inner, aliases);
        }
        let Expr::Ident(identifier) = expression else {
            return Ok(None);
        };
        let definition = self.resolved_definition(identifier.span)?;
        let signature = self.expression_type(expression.span())?;
        let info = self
            .program
            .resolved()
            .scope_table
            .definitions
            .get(definition.index() as usize)
            .ok_or(ResourceExecutionError::InvalidInvocation)?;
        if info.id != definition
            || !matches!(
                self.program.checked().interner.resolve(signature),
                Type::Function { .. }
            )
        {
            return Ok(None);
        }
        match info.kind {
            jett_resolve::DefKind::Function if !self.has_hook(definition) => {
                let function = self.retained_function(definition)?;
                if !function.type_params.is_empty()
                    || self.program.checked().definition_types.get(&definition) != Some(&signature)
                {
                    return Ok(None);
                }
                Ok(Some((definition, signature)))
            }
            jett_resolve::DefKind::Variable => {
                if aliases.iter().any(|(_, seen)| *seen == definition) {
                    return Err(ResourceExecutionError::InvalidInvocation);
                }
                let mut declarations = Vec::new();
                walk_region(region, &mut |_| {}, &mut |_| {}, &mut |statement| {
                    if let Stmt::VarDecl(declaration) = statement {
                        if self
                            .facts(&self.body)
                            .ok()
                            .and_then(|facts| facts.bindings.get(&declaration.name.span))
                            .is_some_and(|fact| fact.definition == definition)
                        {
                            declarations.push(declaration);
                        }
                    }
                });
                let [declaration] = declarations.as_slice() else {
                    return Ok(None);
                };
                let fact = self.binding(declaration.name.span)?;
                if declaration.mutable
                    || fact.mutable
                    || fact.ty != signature
                    || declaration.span.file != identifier.span.file
                    || declaration.span.end > identifier.span.start
                {
                    return Ok(None);
                }
                aliases.push((declaration.name.span, definition));
                let target = self.named_callable_in(region, &declaration.value, aliases)?;
                Ok(target.filter(|(_, target_signature)| *target_signature == signature))
            }
            _ => Ok(None),
        }
    }

    pub(crate) fn prepare_named_indirect(
        &self,
        source: &CheckedInvocation,
        callee: &Expr,
    ) -> Result<PreparedNamedCallable, ResourceExecutionError> {
        let target = self
            .prepare_named_callable(callee)?
            .ok_or(ResourceExecutionError::InvalidInvocation)?;
        self.validate_named_indirect(source, &target)?;
        Ok(target)
    }

    pub(super) fn validate_named_indirect(
        &self,
        source: &CheckedInvocation,
        target: &PreparedNamedCallable,
    ) -> Result<(), ResourceExecutionError> {
        self.validate_executable_cursor(&source.body)?;
        if target.key != self.attempt_key(&source.body)?
            || self.facts(&source.body)?.calls.get(&source.span) != Some(&source.packet)
            || !matches!(source.target(), CheckedInvocationTarget::Indirect(signature) if *signature == target.signature)
            || !matches!(&source.packet.shape, CheckedInvocationShape::Function { signature_type } if *signature_type == target.signature)
        {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        let reference = CheckedBodyReference {
            cursor: source.body.clone(),
        };
        let region = reference.region()?;
        let expression = region_expression(region, target.expression)?;
        let mut calls = 0;
        walk_region(
            region,
            &mut |_| {},
            &mut |original| {
                if let Expr::Call(callee, _, span) = original {
                    let mut bare = callee.as_ref();
                    while let Expr::Paren(inner, _) = bare {
                        bare = inner;
                    }
                    if *span == source.span && std::ptr::eq(bare, expression) {
                        calls += 1;
                    }
                }
            },
            &mut |_| {},
        );
        let mut aliases = Vec::new();
        if calls != 1
            || self.named_callable_in(region, expression, &mut aliases)?
                != Some((target.definition, target.signature))
            || aliases != target.aliases
        {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "named_callable_tests.rs"]
mod tests;
