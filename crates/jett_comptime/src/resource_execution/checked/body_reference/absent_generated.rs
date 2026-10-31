//! Exact generated control binders; the proof carries no runtime custody.
use super::*;
use crate::Value;
use jett_parser::ast::{ForStmt, Ident, MatchStmt, Pattern};
use jett_typecheck::{CheckedBindingMode, CheckedViewSource};

enum Control<'source> {
    For(&'source ForStmt),
    Match(&'source MatchStmt, usize, String),
}

pub(crate) struct PreparedAbsentBindings<'source> {
    key: CheckedAttemptKey,
    control: Control<'source>,
    parent: TypeId,
    payload_count: Option<usize>,
    bindings: Vec<(&'source Ident, CheckedBindingFact)>,
}

impl PreparedAbsentBindings<'_> {
    pub(crate) fn parent_type(&self) -> TypeId {
        self.parent
    }
    pub(crate) fn binding_count(&self) -> usize {
        self.bindings.len()
    }

    pub(crate) fn validate_binding(
        &self,
        original: &Ident,
        ordinal: usize,
    ) -> Result<CheckedBindingFact, ResourceExecutionError> {
        let (binder, fact) = self
            .bindings
            .get(ordinal)
            .ok_or(ResourceExecutionError::InvalidInvocation)?;
        if !std::ptr::eq(*binder, original) {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        Ok(*fact)
    }

    pub(crate) fn validate_parent(&self, value: &Value) -> Result<(), ResourceExecutionError> {
        match (&self.control, value.payload()) {
            (Control::For(source), Value::List(_)) if source.value_variable.is_none() => Ok(()),
            (Control::For(_), Value::Map(_)) => Ok(()),
            (
                Control::Match(source, arm, variant),
                Value::Enum {
                    variant: actual,
                    fields,
                    ..
                },
            ) if (variant.is_empty() || variant == actual)
                && self.payload_count.is_none_or(|count| count == fields.len())
                && source
                    .arms
                    .iter()
                    .position(|candidate| match &candidate.pattern {
                        Pattern::Ident(name) | Pattern::Variant(name, _) => name.name == *actual,
                        Pattern::Other(_) => true,
                    })
                    == Some(*arm) =>
            {
                Ok(())
            }
            _ => Err(ResourceExecutionError::InvalidPayloadPath),
        }
    }
}

impl CheckedExecution {
    fn original_absent_control(
        &self,
        source: impl Fn(&Stmt) -> bool,
    ) -> Result<(), ResourceExecutionError> {
        self.validate_executable_cursor(&self.body)?;
        let reference = CheckedBodyReference {
            cursor: self.body.clone(),
        };
        let mut count = 0;
        walk_region(
            reference.region()?,
            &mut |_| {},
            &mut |_| {},
            &mut |statement| {
                if source(statement) {
                    count += 1;
                }
            },
        );
        if count == 1 {
            Ok(())
        } else {
            Err(ResourceExecutionError::MissingCheckedBody)
        }
    }

    fn absent_binder(
        &self,
        original: &Ident,
        ty: TypeId,
        mode: CheckedBindingMode,
    ) -> Result<CheckedBindingFact, ResourceExecutionError> {
        let fact = self.binding(original.span)?;
        let info = self
            .program
            .resolved()
            .scope_table
            .definitions
            .get(fact.definition.index() as usize)
            .ok_or(ResourceExecutionError::InvalidCheckedProgram)?;
        if info.id != fact.definition
            || info.span != original.span
            || info.name != original.name
            || info.kind != jett_resolve::DefKind::Variable
            || fact.declaration_span != original.span
            || fact.ty != ty
            || fact.mode != mode
            || fact.mutable
            || self.expression_type(original.span)? != ty
        {
            return Err(ResourceExecutionError::InvalidInvocation);
        }
        Ok(fact)
    }

    pub(crate) fn prepare_absent_for<'source>(
        &self,
        source: &'source ForStmt,
    ) -> Result<Option<PreparedAbsentBindings<'source>>, ResourceExecutionError> {
        self.original_absent_control(
            |statement| matches!(statement, Stmt::For(original) if std::ptr::eq(original, source)),
        )?;
        let parent = self.expression_type(source.iterable.span())?;
        if !self.type_contains_resource(parent)? {
            return Ok(None);
        }
        let types = &self.program.checked().interner;
        let mode = |ty| {
            if source.view && !jett_typecheck::ownership::is_implicitly_copyable(types, ty) {
                CheckedBindingMode::View {
                    source: CheckedViewSource::Other,
                }
            } else {
                CheckedBindingMode::Owned
            }
        };
        let mut bindings = Vec::new();
        match types.resolve(parent) {
            Type::List(item) if source.value_variable.is_none() => {
                bindings.push((
                    &source.variable,
                    self.absent_binder(&source.variable, *item, mode(*item))?,
                ));
            }
            Type::Map(key, item) => {
                bindings.push((
                    &source.variable,
                    self.absent_binder(&source.variable, *key, mode(*key))?,
                ));
                if let Some(value) = &source.value_variable {
                    bindings.push((value, self.absent_binder(value, *item, mode(*item))?));
                }
            }
            _ => return Err(ResourceExecutionError::InvalidPayloadPath),
        }
        Ok(Some(PreparedAbsentBindings {
            key: self.attempt_key(&self.body)?,
            control: Control::For(source),
            parent,
            payload_count: None,
            bindings,
        }))
    }

    pub(crate) fn prepare_absent_match<'source>(
        &self,
        source: &'source MatchStmt,
        arm: usize,
    ) -> Result<Option<PreparedAbsentBindings<'source>>, ResourceExecutionError> {
        self.original_absent_control(|statement| matches!(statement, Stmt::Match(original) if std::ptr::eq(original, source)))?;
        let parent = self.expression_type(source.expr.span())?;
        if !self.type_contains_resource(parent)? {
            return Ok(None);
        }
        let Some(selected) = source.arms.get(arm) else {
            return Err(ResourceExecutionError::InvalidInvocation);
        };
        if let Some(selections) = self.facts(&self.body)?.selections {
            if let Some(selection) = selections.get(&source.span) {
                if *selection != jett_typecheck::CheckedStaticSelection::MatchArm(arm) {
                    return Err(ResourceExecutionError::MissingCheckedBody);
                }
            }
        }
        let (variant, names) = match &selected.pattern {
            Pattern::Variant(name, names) => (Some(name), names.as_slice()),
            Pattern::Ident(name) => (Some(name), &[][..]),
            Pattern::Other(_) => (None, &[][..]),
        };
        let types = &self.program.checked().interner;
        let Type::Enum(owner) = types.resolve(parent) else {
            return Err(ResourceExecutionError::InvalidPayloadPath);
        };
        let schema = types.resolve_enum(*owner);
        let (bindings, payload_count) = if let Some(variant) = variant {
            let mut matching = schema
                .variants
                .iter()
                .filter(|candidate| candidate.name == variant.name);
            let selected = matching
                .next()
                .ok_or(ResourceExecutionError::InvalidInvocation)?;
            if matching.next().is_some()
                || (matches!(source.arms[arm].pattern, Pattern::Variant(..))
                    && names.len() != selected.fields.len())
            {
                return Err(ResourceExecutionError::InvalidInvocation);
            }
            let bindings = names
                .iter()
                .zip(&selected.fields)
                .map(|(name, (_, ty))| {
                    self.absent_binder(name, *ty, CheckedBindingMode::Owned)
                        .map(|fact| (name, fact))
                })
                .collect::<Result<Vec<_>, _>>()?;
            (bindings, Some(selected.fields.len()))
        } else {
            (Vec::new(), None)
        };
        Ok(Some(PreparedAbsentBindings {
            key: self.attempt_key(&self.body)?,
            control: Control::Match(
                source,
                arm,
                variant.map(|name| name.name.clone()).unwrap_or_default(),
            ),
            parent,
            payload_count,
            bindings,
        }))
    }

    pub(crate) fn revalidate_absent_bindings(
        &self,
        proof: &PreparedAbsentBindings<'_>,
    ) -> Result<(), ResourceExecutionError> {
        if self.attempt_key(&self.body)? != proof.key {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        let current = match &proof.control {
            Control::For(source) => self.prepare_absent_for(source)?,
            Control::Match(source, arm, _) => self.prepare_absent_match(source, *arm)?,
        }
        .ok_or(ResourceExecutionError::MissingCheckedBody)?;
        let same_control = match (&current.control, &proof.control) {
            (Control::For(left), Control::For(right)) => std::ptr::eq(*left, *right),
            (
                Control::Match(left, arm, variant),
                Control::Match(right, expected_arm, expected_variant),
            ) => std::ptr::eq(*left, *right) && arm == expected_arm && variant == expected_variant,
            _ => false,
        };
        if !same_control
            || current.parent != proof.parent
            || current.payload_count != proof.payload_count
            || current.bindings.len() != proof.bindings.len()
            || !current.bindings.iter().zip(&proof.bindings).all(
                |((left, fact), (right, expected))| std::ptr::eq(*left, *right) && fact == expected,
            )
        {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "absent_generated/tests.rs"]
mod tests;
