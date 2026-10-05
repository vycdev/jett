//! Derived only after the whole original checked HIR archive is authenticated.
//! A current body or signature alone can never add a function to this family.
use super::*;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ResourceExecutionClosure {
    functions: BTreeSet<u32>,
}
impl ResourceExecutionClosure {
    pub(super) fn contains(&self, function: FunctionId) -> bool {
        self.functions.contains(&function.index())
    }
    pub(super) fn direct_call(&self, value: &Expression) -> bool {
        matches!(&value.kind, hir::ExpressionKind::Call { function, .. } if self.contains(*function))
    }
    pub(super) fn from_original(
        functions: &[hir::Function],
        types: &TypeInterner,
    ) -> Result<Self, String> {
        let mut family = Self::default();
        for (index, function) in functions.iter().enumerate() {
            if function.id.index() as usize != index {
                return Err(
                    "Resource original call closure has no exact dense function identity".into(),
                );
            }
            let mut needed = custody_type(types, function.return_type)
                || function
                    .locals
                    .iter()
                    .any(|local| custody_type(types, local.ty));
            walk::hir_block(&function.body, &mut |value| {
                needed |= custody_type(types, value.ty)
                    || matches!(
                        value.kind,
                        hir::ExpressionKind::ResourceHookValue { .. }
                            | hir::ExpressionKind::ResourceInvoke { .. }
                    );
            });
            if needed {
                family.functions.insert(function.id.index());
            }
        }
        loop {
            let mut predecessors = Vec::new();
            for function in functions {
                if family.contains(function.id) {
                    continue;
                }
                let mut calls = false;
                let mut malformed = false;
                walk::hir_block(&function.body, &mut |value| {
                    if let hir::ExpressionKind::Call {
                        function: target,
                        ownership,
                        ..
                    } = &value.kind
                        && family.contains(*target)
                    {
                        calls = true;
                        malformed |= !matches!(ownership, hir::CallOwnership::Source(_));
                    }
                });
                if malformed {
                    return Err(
                        "Resource execution predecessor has no original Source call certificate"
                            .into(),
                    );
                }
                if calls {
                    predecessors.push(function.id);
                }
            }
            if predecessors.is_empty() {
                break;
            }
            family
                .functions
                .extend(predecessors.into_iter().map(|function| function.index()));
        }
        Ok(family)
    }
}
