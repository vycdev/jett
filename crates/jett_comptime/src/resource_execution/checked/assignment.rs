//! A replacement joins the original statement and its complete checked region.
use super::*;
use jett_parser::ast::{AssignStmt, Expr};
use jett_resolve::DefKind;
use jett_typecheck::CheckedBindingMode;

pub(crate) struct CheckedAssignment<'source> {
    source: &'source AssignStmt,
    fact: CheckedBindingFact,
    key: CheckedAttemptKey,
}

impl CheckedAssignment<'_> {
    pub(crate) fn fact(&self) -> CheckedBindingFact {
        self.fact
    }
}

impl CheckedExecution {
    pub(crate) fn prepare_assignment<'source>(
        &self,
        source: &'source AssignStmt,
    ) -> Result<CheckedAssignment<'source>, ResourceExecutionError> {
        let key = self.original_assignment_key(source)?;
        let Expr::Ident(target) = &source.target else {
            return Err(ResourceExecutionError::InvalidInvocation);
        };
        let definition = self.resolved_definition(target.span)?;
        let facts = self.facts(&self.body)?;
        let mut candidates = facts
            .bindings
            .iter()
            .filter(|(_, fact)| fact.definition == definition);
        let (declaration, fact) = candidates
            .next()
            .ok_or(ResourceExecutionError::MissingCheckedBody)?;
        let definition_info = self
            .program
            .resolved()
            .scope_table
            .definitions
            .get(definition.index() as usize)
            .ok_or(ResourceExecutionError::InvalidCheckedProgram)?;
        if candidates.next().is_some()
            || *declaration != fact.declaration_span
            || definition_info.id != definition
            || definition_info.span != fact.declaration_span
            || definition_info.name != target.name
            || !matches!(definition_info.kind, DefKind::Variable | DefKind::Param)
            || !fact.mutable
            || fact.mode != CheckedBindingMode::Owned
            || self.expression_type(target.span)? != fact.ty
            || self.expression_type(source.value.span())? != fact.ty
            || !self.type_contains_resource(fact.ty)?
        {
            return Err(ResourceExecutionError::InvalidOwner);
        }
        Ok(CheckedAssignment {
            source,
            fact: *fact,
            key,
        })
    }

    pub(crate) fn revalidate_assignment(
        &self,
        assignment: &CheckedAssignment<'_>,
    ) -> Result<(), ResourceExecutionError> {
        let current = self.prepare_assignment(assignment.source)?;
        if current.fact != assignment.fact || current.key != assignment.key {
            return Err(ResourceExecutionError::MissingCheckedBody);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jett_common::FileId;
    use jett_parser::ast::{FunctionDef, Item, Stmt};

    const SOURCE: &str = include_str!("../fixtures/24_mutable_assignment.jett");

    fn function<'a>(program: &'a CheckedResourceProgram, name: &str) -> &'a FunctionDef {
        program
            .module()
            .items
            .iter()
            .find_map(|item| match item {
                Item::Function(function)
                    if function.name.name == name && function.name.span.file == FileId::new(0) =>
                {
                    Some(function)
                }
                _ => None,
            })
            .unwrap()
    }

    fn assignment(function: &FunctionDef) -> &AssignStmt {
        function
            .body
            .stmts
            .iter()
            .find_map(|statement| match statement {
                Stmt::Assign(assignment) => Some(assignment),
                _ => None,
            })
            .unwrap()
    }

    fn install(checked: &mut CheckedExecution, function: &FunctionDef) {
        let definition = checked.declaration_definition(function.name.span).unwrap();
        let invocation = checked.entry(definition).unwrap();
        let reference = checked.prepare_function_body(&invocation).unwrap();
        checked.install_function_body(&reference).unwrap();
    }

    #[test]
    fn checked_resource_assignment_refuses_clones_foreign_programs_and_other_bodies() {
        for release in [false, true] {
            let program = crate::resource_execution::tests::program(SOURCE, release);
            let original = function(&program, "replace_live");
            let original_assignment = assignment(original);
            let mut checked =
                CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
            install(&mut checked, original);
            checked.prepare_assignment(original_assignment).unwrap();
            let clone = original_assignment.clone();
            assert!(checked.prepare_assignment(&clone).is_err());
            let foreign = crate::resource_execution::tests::program(SOURCE, release);
            assert!(
                checked
                    .prepare_assignment(assignment(function(&foreign, "replace_live")))
                    .is_err()
            );
            install(&mut checked, function(&program, "rebind_after_close"));
            assert!(checked.prepare_assignment(original_assignment).is_err());
        }
    }

    #[test]
    fn checked_resource_assignment_rejoins_prepared_facts_and_body_after_rhs() {
        for release in [false, true] {
            let program = crate::resource_execution::tests::program(SOURCE, release);
            let original = function(&program, "replace_live");
            let mut checked =
                CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
            install(&mut checked, original);
            let mut prepared = checked.prepare_assignment(assignment(original)).unwrap();
            checked.revalidate_assignment(&prepared).unwrap();
            prepared.fact.mutable = false;
            assert!(checked.revalidate_assignment(&prepared).is_err());
            prepared.fact.mutable = true;
            install(&mut checked, function(&program, "rebind_after_close"));
            assert!(checked.revalidate_assignment(&prepared).is_err());
            checked.enter_ordinary();
            assert!(checked.revalidate_assignment(&prepared).is_err());
        }
    }
}
