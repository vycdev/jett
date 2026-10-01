//! Fill inferred bottom slots without discarding source-visible type aliases.
use super::{Expr, Interpreter, Param, TypeExpr};

impl Interpreter {
    pub(super) fn type_witness_source(mut expression: &Expr) -> &Expr {
        while let Expr::View(inner, _) | Expr::Paren(inner, _) = expression {
            expression = inner;
        }
        expression
    }

    pub(super) fn type_witness_parameter(param: &Param, ty: TypeExpr) -> TypeExpr {
        if param.view && !matches!(ty, TypeExpr::View(_, _)) {
            TypeExpr::View(Box::new(ty), param.span)
        } else {
            ty
        }
    }

    pub(super) fn merge_inferred_never(
        &self,
        first: &TypeExpr,
        next: &TypeExpr,
        contravariant: bool,
    ) -> Option<TypeExpr> {
        // Equal canonical types retain the first parameter's source spelling.
        // Root alias peeling happens before inference, never recursively here.
        if self.concrete_type_display(first) == self.concrete_type_display(next) {
            return Some(first.clone());
        }
        let first_never = is_never(first);
        let next_never = is_never(next);
        if first_never || next_never {
            return Some(if first_never != contravariant {
                next.clone()
            } else {
                first.clone()
            });
        }
        match (first, next) {
            (
                TypeExpr::Generic(first_owner, first_args, span),
                TypeExpr::Generic(next_owner, next_args, _),
            ) if first_owner.name == next_owner.name
                && first_args.len() == next_args.len()
                && matches!(
                    first_owner.name.as_str(),
                    "list" | "set" | "optional" | "result" | "map" | "secret"
                ) =>
            {
                Some(TypeExpr::Generic(
                    first_owner.clone(),
                    first_args
                        .iter()
                        .zip(next_args)
                        .map(|(first, next)| self.merge_inferred_never(first, next, contravariant))
                        .collect::<Option<_>>()?,
                    *span,
                ))
            }
            (
                TypeExpr::Function(first_params, first_return, span),
                TypeExpr::Function(next_params, next_return, _),
            ) if first_params.len() == next_params.len() => Some(TypeExpr::Function(
                first_params
                    .iter()
                    .zip(next_params)
                    .map(|(first, next)| self.merge_inferred_never(first, next, !contravariant))
                    .collect::<Option<_>>()?,
                Box::new(self.merge_inferred_never(first_return, next_return, contravariant)?),
                *span,
            )),
            (TypeExpr::View(first, span), TypeExpr::View(next, _)) => Some(TypeExpr::View(
                Box::new(self.merge_inferred_never(first, next, contravariant)?),
                *span,
            )),
            _ => None,
        }
    }
}

fn is_never(ty: &TypeExpr) -> bool {
    matches!(ty, TypeExpr::Named(name) if name.name == "<never>")
}

#[cfg(test)]
mod tests;
