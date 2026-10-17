//! Local nonzero facts for exact bindings and finite ordinary field places.
//! These are observed guard facts, never mutation or ownership authority.

use super::*;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum NonzeroFact {
    Binding(DefId),
    Field {
        root: DefId,
        root_type: TypeId,
        projections: Vec<NonzeroProjection>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct NonzeroProjection {
    owner: NonzeroOwner,
    owner_type: TypeId,
    ordinal: usize,
    declared_type: TypeId,
    access_type: TypeId,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum NonzeroOwner {
    Struct(StructId),
    MachineState(MachineId, MachineStateId),
}

impl NonzeroFact {
    fn root(&self) -> DefId {
        match self {
            Self::Binding(root) | Self::Field { root, .. } => *root,
        }
    }
}

impl TypeChecker<'_> {
    pub(super) fn nonzero_fact(&self, expression: &Expr) -> Option<NonzeroFact> {
        match expression {
            Expr::Ident(name) => self.ident_def_id(name).map(NonzeroFact::Binding),
            Expr::Paren(inner, _) => self.nonzero_fact(inner),
            Expr::FieldAccess(_, _, _) => {
                let name = Self::assignment_root(expression)?;
                let root = self.ident_def_id(name)?;
                if !matches!(
                    self.resolve.scope_table.def(root).kind,
                    DefKind::Variable | DefKind::Param
                ) {
                    return None;
                }
                let root_type = self.checked_expression_type(name.span)?;
                let mut checked = Vec::new();
                if !self.collect_assignment_projections(expression, &mut checked)
                    || checked.is_empty()
                {
                    return None;
                }
                let projections = checked
                    .into_iter()
                    .map(|projection| NonzeroProjection {
                        owner: match projection.owner {
                            crate::CheckedFieldOwner::Struct(id) => NonzeroOwner::Struct(id),
                            crate::CheckedFieldOwner::MachineState { machine, state } => {
                                NonzeroOwner::MachineState(machine, state)
                            }
                        },
                        owner_type: projection.owner_type,
                        ordinal: projection.field_index,
                        declared_type: projection.declared_field_type,
                        access_type: projection.field_type,
                    })
                    .collect();
                Some(NonzeroFact::Field {
                    root,
                    root_type,
                    projections,
                })
            }
            _ => None,
        }
    }

    pub(super) fn invalidate_nonzero_root(&mut self, root: DefId) {
        for scope in &mut self.nonzero_fact_scopes {
            scope.retain(|fact| fact.root() != root);
        }
    }

    fn invalidate_nonzero_roots(&mut self, roots: &HashSet<DefId>) {
        for scope in &mut self.nonzero_fact_scopes {
            scope.retain(|fact| !roots.contains(&fact.root()));
        }
    }

    pub(super) fn nonzero_defs_from_conditions<'b>(
        &self,
        conditions: impl IntoIterator<Item = (&'b Expr, bool)>,
    ) -> HashSet<NonzeroFact> {
        let mut facts = HashSet::new();
        let mut modified = HashSet::new();
        for (condition, truth) in conditions {
            self.collect_nonzero_defs(condition, truth, &mut facts);
            self.nonzero_expression_writes(condition, &mut modified);
        }
        // A later operand/else-if may have changed an earlier tested place.
        // Refuse to remint that fact from the condition's original syntax.
        facts.retain(|fact| !modified.contains(&fact.root()));
        facts
    }

    fn collect_nonzero_defs(
        &self,
        condition: &Expr,
        truth: bool,
        facts: &mut HashSet<NonzeroFact>,
    ) {
        match condition {
            Expr::Paren(inner, _) => self.collect_nonzero_defs(inner, truth, facts),
            Expr::Unary(UnaryOp::Not, inner, _) => self.collect_nonzero_defs(inner, !truth, facts),
            Expr::Binary(lhs, BinOp::And, rhs, _) if truth => {
                self.collect_nonzero_defs(lhs, true, facts);
                self.collect_nonzero_defs(rhs, true, facts);
            }
            Expr::Binary(lhs, BinOp::Or, rhs, _) if !truth => {
                self.collect_nonzero_defs(lhs, false, facts);
                self.collect_nonzero_defs(rhs, false, facts);
            }
            Expr::Binary(lhs, op @ (BinOp::Eq | BinOp::NotEq), rhs, _) => {
                if matches!(op, BinOp::NotEq) != truth {
                    return;
                }
                let tested = if Self::is_zero_integer_literal(rhs) {
                    Some(lhs.as_ref())
                } else if Self::is_zero_integer_literal(lhs) {
                    Some(rhs.as_ref())
                } else {
                    None
                };
                if let Some(fact) = tested.and_then(|value| self.nonzero_fact(value)) {
                    facts.insert(fact);
                }
            }
            _ => {}
        }
    }

    fn is_zero_integer_literal(expr: &Expr) -> bool {
        match expr {
            Expr::IntLiteral(value, _) => *value == 0,
            Expr::Unary(UnaryOp::Neg, inner, _) | Expr::Paren(inner, _) => {
                Self::is_zero_integer_literal(inner)
            }
            _ => false,
        }
    }

    pub(super) fn invalidate_nonzero_loop_facts(&mut self, body: &Block, condition: Option<&Expr>) {
        let mut modified = HashSet::new();
        self.nonzero_block_writes(body, &mut modified);
        if let Some(condition) = condition {
            self.nonzero_expression_writes(condition, &mut modified);
        }
        // The checker visits a loop body once; inherited observations must also
        // be valid at a backedge. A fresh per-iteration guard may prove them anew.
        self.invalidate_nonzero_roots(&modified);
    }

    fn nonzero_block_writes(&self, body: &Block, roots: &mut HashSet<DefId>) {
        for statement in &body.stmts {
            match statement {
                Stmt::VarDecl(value) => self.nonzero_expression_writes(&value.value, roots),
                Stmt::Assign(value) => {
                    if let Some(root) = Self::assignment_root(&value.target)
                        .and_then(|name| self.ident_def_id(name))
                    {
                        roots.insert(root);
                    }
                    self.nonzero_expression_writes(&value.target, roots);
                    self.nonzero_expression_writes(&value.value, roots);
                }
                Stmt::Return(value) => {
                    if let Some(value) = &value.value {
                        self.nonzero_expression_writes(value, roots);
                    }
                }
                Stmt::Respond(value) => self.nonzero_expression_writes(&value.value, roots),
                Stmt::ComptimeTypeBind(value) => {
                    self.nonzero_expression_writes(&value.value, roots);
                    self.nonzero_block_writes(&value.body, roots);
                }
                Stmt::If(value) => {
                    self.nonzero_expression_writes(&value.condition, roots);
                    self.nonzero_block_writes(&value.then_block, roots);
                    for (condition, body) in &value.else_ifs {
                        self.nonzero_expression_writes(condition, roots);
                        self.nonzero_block_writes(body, roots);
                    }
                    if let Some(body) = &value.else_block {
                        self.nonzero_block_writes(body, roots);
                    }
                }
                Stmt::For(value) => {
                    self.nonzero_expression_writes(&value.iterable, roots);
                    self.nonzero_block_writes(&value.body, roots);
                }
                Stmt::While(value) => {
                    self.nonzero_expression_writes(&value.condition, roots);
                    self.nonzero_block_writes(&value.body, roots);
                }
                Stmt::Match(value) => {
                    self.nonzero_expression_writes(&value.expr, roots);
                    for arm in &value.arms {
                        self.nonzero_block_writes(&arm.body, roots);
                    }
                }
                Stmt::Expr(value) => self.nonzero_expression_writes(&value.expr, roots),
                Stmt::Assert(value) => {
                    self.nonzero_expression_writes(&value.condition, roots);
                    if let Some(message) = &value.message {
                        self.nonzero_expression_writes(message, roots);
                    }
                }
                Stmt::Breakpoint(value) => {
                    if let Some(condition) = &value.condition {
                        self.nonzero_expression_writes(condition, roots);
                    }
                }
                Stmt::Use(_) | Stmt::Trace(_) | Stmt::Break(_) | Stmt::Continue(_) => {}
            }
        }
    }

    fn nonzero_expression_writes(&self, value: &Expr, roots: &mut HashSet<DefId>) {
        match value {
            Expr::Handle(target, _, body, _) => {
                self.nonzero_expression_writes(target, roots);
                self.nonzero_block_writes(body, roots);
            }
            Expr::Binary(left, _, right, _) => {
                self.nonzero_expression_writes(left, roots);
                self.nonzero_expression_writes(right, roots);
            }
            Expr::Unary(_, value, _)
            | Expr::FieldAccess(value, _, _)
            | Expr::Paren(value, _)
            | Expr::View(value, _)
            | Expr::Comptime(value, _)
            | Expr::Ok(value, _)
            | Expr::Fail(value, _)
            | Expr::Some(value, _)
            | Expr::Default(value, _)
            | Expr::Declassify(value, _)
            | Expr::Coarsen(value, _)
            | Expr::At(value, _, _)
            | Expr::Spawn(value, _)
            | Expr::Send(value, _)
            | Expr::Ask(value, _)
            | Expr::Clone(value, _)
            | Expr::Run(value, _)
            | Expr::Join(value, _)
            | Expr::Cancel(value, _) => self.nonzero_expression_writes(value, roots),
            Expr::Call(callee, args, _) | Expr::GenericCall(callee, _, args, _) => {
                self.nonzero_expression_writes(callee, roots);
                for arg in args {
                    self.nonzero_expression_writes(&arg.value, roots);
                }
            }
            Expr::ListConstruct(values, _) => {
                for value in values {
                    self.nonzero_expression_writes(value, roots);
                }
            }
            Expr::MapConstruct(entries, _) => {
                for (key, value) in entries {
                    self.nonzero_expression_writes(key, roots);
                    self.nonzero_expression_writes(value, roots);
                }
            }
            Expr::StringInterpolation(parts, _) => {
                for part in parts {
                    if let StringPart::Expr(value) = part {
                        self.nonzero_expression_writes(value, roots);
                    }
                }
            }
            Expr::Pipeline(value, steps, _) => {
                self.nonzero_expression_writes(value, roots);
                for step in steps {
                    self.nonzero_expression_writes(&step.function, roots);
                    for arg in &step.extra_args {
                        self.nonzero_expression_writes(&arg.value, roots);
                    }
                    if let Some(handle) = &step.handle {
                        self.nonzero_block_writes(&handle.body, roots);
                    }
                }
            }
            // Defining a closure does not execute its body. Captures keep the
            // existing copy-only policy; this scan introduces no alias effects.
            Expr::InlineFn(_, _, _, _)
            | Expr::IntLiteral(_, _)
            | Expr::FloatLiteral(_, _)
            | Expr::StringLiteral(_, _)
            | Expr::BoolLiteral(_, _)
            | Expr::Nothing(_)
            | Expr::Ident(_)
            | Expr::None(_)
            | Expr::EnumVariant(_, _, _)
            | Expr::Error(_) => {}
        }
    }
}
