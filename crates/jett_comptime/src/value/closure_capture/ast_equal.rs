//! Typed source comparison for producer custody; no display/hash authority.
use jett_parser::ast::*;

pub(super) fn list<T>(a: &[T], b: &[T], mut same: impl FnMut(&T, &T) -> bool) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same(a, b))
}
pub(super) fn optional<T>(a: &Option<T>, b: &Option<T>, same: impl FnOnce(&T, &T) -> bool) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => same(a, b),
        (None, None) => true,
        _ => false,
    }
}
fn ident(a: &Ident, b: &Ident) -> bool {
    a.name == b.name && a.span == b.span
}
pub(super) fn type_expr(a: &TypeExpr, b: &TypeExpr) -> bool {
    if a.span() != b.span() {
        return false;
    }
    match (a, b) {
        (TypeExpr::Named(a), TypeExpr::Named(b)) => ident(a, b),
        (TypeExpr::Generic(an, aa, _), TypeExpr::Generic(bn, ba, _)) => {
            ident(an, bn) && list(aa, ba, type_expr)
        }
        (TypeExpr::View(a, _), TypeExpr::View(b, _)) => type_expr(a, b),
        (TypeExpr::Function(ap, ar, _), TypeExpr::Function(bp, br, _)) => {
            list(ap, bp, type_expr) && type_expr(ar, br)
        }
        (TypeExpr::StateQualified(a, an, _), TypeExpr::StateQualified(b, bn, _)) => {
            type_expr(a, b) && ident(an, bn)
        }
        _ => false,
    }
}
pub(super) fn params(a: &[Param], b: &[Param]) -> bool {
    list(a, b, |a, b| {
        a.view == b.view
            && a.mutable == b.mutable
            && a.span == b.span
            && ident(&a.name, &b.name)
            && type_expr(&a.ty, &b.ty)
    })
}
pub(super) fn function(a: &FunctionDef, b: &FunctionDef) -> bool {
    a.span == b.span
        && a.exported == b.exported
        && ident(&a.name, &b.name)
        && list(&a.type_params, &b.type_params, ident)
        && params(&a.params, &b.params)
        && optional(&a.return_type, &b.return_type, type_expr)
        && block(&a.body, &b.body)
}
pub(super) fn block(a: &Block, b: &Block) -> bool {
    a.span == b.span && list(&a.stmts, &b.stmts, stmt)
}
fn pattern(a: &Pattern, b: &Pattern) -> bool {
    match (a, b) {
        (Pattern::Ident(a), Pattern::Ident(b)) => ident(a, b),
        (Pattern::Variant(an, aa), Pattern::Variant(bn, ba)) => {
            ident(an, bn) && list(aa, ba, ident)
        }
        (Pattern::Other(a), Pattern::Other(b)) => a == b,
        _ => false,
    }
}
fn stmt(a: &Stmt, b: &Stmt) -> bool {
    match (a, b) {
        (Stmt::VarDecl(a), Stmt::VarDecl(b)) => {
            a.span == b.span
                && a.mutable == b.mutable
                && type_expr(&a.ty, &b.ty)
                && ident(&a.name, &b.name)
                && expr(&a.value, &b.value)
        }
        (Stmt::Assign(a), Stmt::Assign(b)) => {
            a.span == b.span && expr(&a.target, &b.target) && expr(&a.value, &b.value)
        }
        (Stmt::Return(a), Stmt::Return(b)) => {
            a.span == b.span && optional(&a.value, &b.value, expr)
        }
        (Stmt::Respond(a), Stmt::Respond(b)) => a.span == b.span && expr(&a.value, &b.value),
        (Stmt::ComptimeTypeBind(a), Stmt::ComptimeTypeBind(b)) => {
            a.span == b.span
                && ident(&a.name, &b.name)
                && expr(&a.value, &b.value)
                && block(&a.body, &b.body)
        }
        (Stmt::If(a), Stmt::If(b)) => {
            a.span == b.span
                && expr(&a.condition, &b.condition)
                && block(&a.then_block, &b.then_block)
                && list(&a.else_ifs, &b.else_ifs, |(ac, ab), (bc, bb)| {
                    expr(ac, bc) && block(ab, bb)
                })
                && optional(&a.else_block, &b.else_block, block)
        }
        (Stmt::For(a), Stmt::For(b)) => {
            a.span == b.span
                && a.view == b.view
                && ident(&a.variable, &b.variable)
                && optional(&a.value_variable, &b.value_variable, ident)
                && expr(&a.iterable, &b.iterable)
                && block(&a.body, &b.body)
        }
        (Stmt::While(a), Stmt::While(b)) => {
            a.span == b.span && expr(&a.condition, &b.condition) && block(&a.body, &b.body)
        }
        (Stmt::Match(a), Stmt::Match(b)) => {
            a.span == b.span
                && expr(&a.expr, &b.expr)
                && list(&a.arms, &b.arms, |a, b| {
                    a.span == b.span && pattern(&a.pattern, &b.pattern) && block(&a.body, &b.body)
                })
        }
        (Stmt::Expr(a), Stmt::Expr(b)) => a.span == b.span && expr(&a.expr, &b.expr),
        (Stmt::Use(a), Stmt::Use(b)) => {
            a.span == b.span && ident(&a.path, &b.path) && optional(&a.alias, &b.alias, ident)
        }
        (Stmt::Assert(a), Stmt::Assert(b)) => {
            a.span == b.span
                && expr(&a.condition, &b.condition)
                && optional(&a.message, &b.message, expr)
        }
        (Stmt::Trace(a), Stmt::Trace(b)) => a.span == b.span && ident(&a.name, &b.name),
        (Stmt::Breakpoint(a), Stmt::Breakpoint(b)) => {
            a.span == b.span && optional(&a.condition, &b.condition, expr)
        }
        (Stmt::Break(a), Stmt::Break(b)) | (Stmt::Continue(a), Stmt::Continue(b)) => a == b,
        _ => false,
    }
}
fn args(a: &[CallArg], b: &[CallArg]) -> bool {
    list(a, b, |a, b| {
        a.span == b.span && optional(&a.name, &b.name, ident) && expr(&a.value, &b.value)
    })
}
pub(super) fn expr(a: &Expr, b: &Expr) -> bool {
    if a.span() != b.span() {
        return false;
    }
    match (a, b) {
        (Expr::IntLiteral(a, _), Expr::IntLiteral(b, _)) => a == b,
        (Expr::FloatLiteral(a, _), Expr::FloatLiteral(b, _)) => a.to_bits() == b.to_bits(),
        (Expr::StringLiteral(a, _), Expr::StringLiteral(b, _)) => a == b,
        (Expr::BoolLiteral(a, _), Expr::BoolLiteral(b, _)) => a == b,
        (Expr::Nothing(_), Expr::Nothing(_))
        | (Expr::None(_), Expr::None(_))
        | (Expr::Error(_), Expr::Error(_)) => true,
        (Expr::Ident(a), Expr::Ident(b)) => ident(a, b),
        (Expr::Binary(al, ao, ar, _), Expr::Binary(bl, bo, br, _)) => {
            ao == bo && expr(al, bl) && expr(ar, br)
        }
        (Expr::Unary(ao, a, _), Expr::Unary(bo, b, _)) => ao == bo && expr(a, b),
        (Expr::FieldAccess(a, an, _), Expr::FieldAccess(b, bn, _))
        | (Expr::At(a, an, _), Expr::At(b, bn, _)) => expr(a, b) && ident(an, bn),
        (Expr::Call(a, aa, _), Expr::Call(b, ba, _)) => expr(a, b) && args(aa, ba),
        (Expr::GenericCall(a, at, aa, _), Expr::GenericCall(b, bt, ba, _)) => {
            expr(a, b) && list(at, bt, type_expr) && args(aa, ba)
        }
        (Expr::Paren(a, _), Expr::Paren(b, _))
        | (Expr::View(a, _), Expr::View(b, _))
        | (Expr::Comptime(a, _), Expr::Comptime(b, _))
        | (Expr::Ok(a, _), Expr::Ok(b, _))
        | (Expr::Fail(a, _), Expr::Fail(b, _))
        | (Expr::Some(a, _), Expr::Some(b, _))
        | (Expr::Default(a, _), Expr::Default(b, _))
        | (Expr::Declassify(a, _), Expr::Declassify(b, _))
        | (Expr::Coarsen(a, _), Expr::Coarsen(b, _))
        | (Expr::Spawn(a, _), Expr::Spawn(b, _))
        | (Expr::Send(a, _), Expr::Send(b, _))
        | (Expr::Ask(a, _), Expr::Ask(b, _))
        | (Expr::Clone(a, _), Expr::Clone(b, _))
        | (Expr::Run(a, _), Expr::Run(b, _))
        | (Expr::Join(a, _), Expr::Join(b, _))
        | (Expr::Cancel(a, _), Expr::Cancel(b, _)) => expr(a, b),
        (Expr::ListConstruct(a, _), Expr::ListConstruct(b, _)) => list(a, b, expr),
        (Expr::MapConstruct(a, _), Expr::MapConstruct(b, _)) => {
            list(a, b, |(ak, av), (bk, bv)| expr(ak, bk) && expr(av, bv))
        }
        (Expr::Handle(a, an, ab, _), Expr::Handle(b, bn, bb, _)) => {
            expr(a, b) && optional(an, bn, ident) && block(ab, bb)
        }
        (Expr::EnumVariant(at, av, _), Expr::EnumVariant(bt, bv, _)) => {
            ident(at, bt) && ident(av, bv)
        }
        (Expr::StringInterpolation(a, _), Expr::StringInterpolation(b, _)) => {
            list(a, b, |a, b| match (a, b) {
                (StringPart::Literal(a), StringPart::Literal(b)) => a == b,
                (StringPart::Expr(a), StringPart::Expr(b)) => expr(a, b),
                _ => false,
            })
        }
        (Expr::Pipeline(a, aa, _), Expr::Pipeline(b, ba, _)) => {
            expr(a, b)
                && list(aa, ba, |a, b| {
                    a.span == b.span
                        && expr(&a.function, &b.function)
                        && args(&a.extra_args, &b.extra_args)
                        && optional(&a.handle, &b.handle, |a, b| {
                            a.span == b.span
                                && optional(&a.error_name, &b.error_name, ident)
                                && block(&a.body, &b.body)
                        })
                })
        }
        (Expr::InlineFn(ap, ar, ab, _), Expr::InlineFn(bp, br, bb, _)) => {
            params(ap, bp) && optional(ar, br, type_expr) && block(ab, bb)
        }
        _ => false,
    }
}
