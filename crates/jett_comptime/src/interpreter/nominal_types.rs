//! Concrete type metadata that survives storing a nominal value as an interface.
use super::{HashSet, Interpreter, TypeExpr, type_expr_display};

impl Interpreter {
    pub(crate) fn concrete_type_display(&self, ty: &TypeExpr) -> String {
        type_expr_display(&self.concrete_type_expr(ty, &mut HashSet::new()))
    }

    pub(super) fn concrete_type_expr(
        &self,
        ty: &TypeExpr,
        aliases: &mut HashSet<String>,
    ) -> TypeExpr {
        let ty = self.substitute_type_expr(ty);
        match ty {
            TypeExpr::Named(ref ident)
                if matches!(self.type_aliases.get(&ident.name), Some(None)) =>
            {
                // Transparent aliases normalize to their checked target, while
                // refinements retain their nominal identity. Scope the cycle
                // guard to this path so repeated arguments expand independently.
                if !aliases.insert(ident.name.clone()) {
                    return ty;
                }
                let result = self.type_alias_bases.get(&ident.name).map(|base| {
                    let namespace = Self::type_name_namespace(&ident.name)
                        .or(self.current_namespace.as_deref());
                    let base = self.substitute_type_expr_in_namespace(base, namespace);
                    self.concrete_type_expr(&base, aliases)
                });
                aliases.remove(&ident.name);
                result.unwrap_or(ty)
            }
            TypeExpr::Generic(ident, args, span) => TypeExpr::Generic(
                ident,
                args.iter()
                    .map(|arg| self.concrete_type_expr(arg, aliases))
                    .collect(),
                span,
            ),
            TypeExpr::View(inner, span) => {
                TypeExpr::View(Box::new(self.concrete_type_expr(&inner, aliases)), span)
            }
            TypeExpr::StateQualified(inner, state, span) => TypeExpr::StateQualified(
                Box::new(self.concrete_type_expr(&inner, aliases)),
                state,
                span,
            ),
            TypeExpr::Function(params, result, span) => TypeExpr::Function(
                params
                    .iter()
                    .map(|param| self.concrete_type_expr(param, aliases))
                    .collect(),
                Box::new(self.concrete_type_expr(&result, aliases)),
                span,
            ),
            _ => ty,
        }
    }
}
