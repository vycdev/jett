//! Concrete type metadata that survives storing a nominal value as an interface.
use super::{HashSet, Interpreter, TypeExpr, type_expr_display};

impl Interpreter {
    pub(crate) fn concrete_type_display(&self, ty: &TypeExpr) -> String {
        type_expr_display(&self.concrete_type_expr(ty, &mut HashSet::new()))
    }

    pub(super) fn resolved_concrete_type_display(&self, ty: &TypeExpr) -> String {
        type_expr_display(&self.concrete_resolved_type_expr(ty, &mut HashSet::new()))
    }

    pub(super) fn concrete_type_expr(
        &self,
        ty: &TypeExpr,
        aliases: &mut HashSet<String>,
    ) -> TypeExpr {
        let ty = self.substitute_type_expr(ty);
        self.concrete_resolved_type_expr(&ty, aliases)
    }

    pub(super) fn concrete_resolved_type_expr(
        &self,
        ty: &TypeExpr,
        aliases: &mut HashSet<String>,
    ) -> TypeExpr {
        match ty {
            TypeExpr::Named(ident) if matches!(self.type_aliases.get(&ident.name), Some(None)) => {
                // Transparent aliases normalize to their checked target, while
                // refinements retain their nominal identity. Scope the cycle
                // guard to this path so repeated arguments expand independently.
                if !aliases.insert(ident.name.clone()) {
                    return ty.clone();
                }
                let result = self.type_alias_bases.get(&ident.name).map(|base| {
                    let namespace = Self::type_name_namespace(&ident.name)
                        .or(self.current_namespace.as_deref());
                    let base = self.qualify_declared_type_expr(base, namespace);
                    self.concrete_resolved_type_expr(&base, aliases)
                });
                aliases.remove(&ident.name);
                result.unwrap_or_else(|| ty.clone())
            }
            TypeExpr::Generic(ident, args, span) => TypeExpr::Generic(
                ident.clone(),
                args.iter()
                    .map(|arg| self.concrete_resolved_type_expr(arg, aliases))
                    .collect(),
                *span,
            ),
            TypeExpr::View(inner, span) => TypeExpr::View(
                Box::new(self.concrete_resolved_type_expr(inner, aliases)),
                *span,
            ),
            TypeExpr::StateQualified(inner, state, span) => TypeExpr::StateQualified(
                Box::new(self.concrete_resolved_type_expr(inner, aliases)),
                state.clone(),
                *span,
            ),
            TypeExpr::Function(params, result, span) => TypeExpr::Function(
                params
                    .iter()
                    .map(|param| self.concrete_resolved_type_expr(param, aliases))
                    .collect(),
                Box::new(self.concrete_resolved_type_expr(result, aliases)),
                *span,
            ),
            _ => ty.clone(),
        }
    }
}
