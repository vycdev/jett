//! Exact original Source body lookup; names choose a resolved declaration only.
use jett_parser::ast::{
    Block, ComptimeTypeBindStmt, Expr, FunctionDef, Item, Stmt, StringPart, VarDecl,
};
use jett_resolve::{DefId, DefKind};
use jett_typecheck::CheckedResourceProgram;

pub(super) fn named_body(program: &CheckedResourceProgram, definition: DefId) -> Option<&Block> {
    named_function(program, definition).map(|function| &function.body)
}

pub(crate) fn named_function(
    program: &CheckedResourceProgram,
    definition: DefId,
) -> Option<&FunctionDef> {
    let info = program
        .resolved()
        .scope_table
        .definitions
        .get(definition.index() as usize)?;
    if info.id != definition || info.kind != DefKind::Function {
        return None;
    }
    let mut matches = Vec::new();
    for item in &program.module().items {
        match item {
            // A mutual declaration owns the DefId and declaration span. The
            // resolver joins its later, immutable root definition to that same
            // DefId; the body span must not be confused with the declaration.
            Item::Function(function)
                if function.name.span == info.span
                    || program.resolved().resolutions.get(&function.name.span)
                        == Some(&definition) =>
            {
                matches.push(function);
            }
            Item::Implement(block) => {
                for method in &block.methods {
                    if method.name.span == info.span {
                        matches.push(method);
                    }
                }
            }
            Item::Struct(structure) => {
                for method in &structure.methods {
                    if method.name.span == info.span {
                        matches.push(method);
                    }
                }
            }
            _ => {}
        }
    }
    match matches.as_slice() {
        [function] => Some(*function),
        _ => None,
    }
}

/// Locate actual declaration storage, excluding pattern and iteration binders.
/// The immutable module remains the authority; this lookup grants no execution.
pub(crate) fn variable_declaration<'a>(
    program: &'a CheckedResourceProgram,
    span: jett_common::Span,
) -> Option<&'a VarDecl> {
    let contains = |outer: jett_common::Span| {
        outer.file == span.file && outer.start <= span.start && outer.end >= span.end
    };
    let mut matches = Vec::new();
    let mut blocks = Vec::new();
    let mut expressions = Vec::new();
    for item in &program.module().items {
        match item {
            Item::Function(function) => blocks.push(&function.body),
            Item::Implement(block) => {
                blocks.extend(block.methods.iter().map(|method| &method.body));
            }
            Item::Struct(structure) => {
                blocks.extend(structure.methods.iter().map(|method| &method.body));
            }
            Item::Verify(block) => blocks.push(&block.body),
            Item::Property(block) => blocks.push(&block.body),
            Item::VarDecl(declaration) => {
                if declaration.name.span == span {
                    matches.push(declaration);
                }
                expressions.push(&declaration.value);
            }
            Item::Actor(actor) => {
                for declaration in &actor.state_fields {
                    if declaration.name.span == span {
                        matches.push(declaration);
                    }
                    expressions.push(&declaration.value);
                }
                blocks.extend(actor.handlers.iter().map(|handler| &handler.body));
            }
            Item::TypeAlias(alias) => expressions.extend(alias.constraint.iter()),
            Item::Namespace(_)
            | Item::Mutual(_)
            | Item::Interface(_)
            | Item::Bitfield(_)
            | Item::Enum(_)
            | Item::Machine(_)
            | Item::Resource(_) => {}
        }
    }
    let mut declaration = |statement: &'a Stmt| {
        if let Stmt::VarDecl(value) = statement {
            if value.name.span == span {
                matches.push(value);
            }
        }
    };
    for block in blocks.into_iter().filter(|block| contains(block.span)) {
        walk_block_nodes(block, &mut |_| {}, &mut |_| {}, &mut declaration);
    }
    for expression in expressions
        .into_iter()
        .filter(|expression| contains(expression.span()))
    {
        walk_expr_nodes(expression, &mut |_| {}, &mut |_| {}, &mut declaration);
    }
    match matches.as_slice() {
        [declaration] => Some(*declaration),
        _ => None,
    }
}

pub(super) fn closure_body<'a>(
    program: &'a CheckedResourceProgram,
    span: jett_common::Span,
) -> Option<&'a Block> {
    match closure_node(program, span)? {
        Expr::InlineFn(_, _, body, _) => Some(body),
        _ => None,
    }
}

pub(crate) fn closure_node<'a>(
    program: &'a CheckedResourceProgram,
    span: jett_common::Span,
) -> Option<&'a Expr> {
    let mut matches = Vec::new();
    let mut expression = |value: &'a Expr| {
        if let Expr::InlineFn(_, _, body, _) = value {
            if body.span == span {
                matches.push(value);
            }
        }
    };
    for item in &program.module().items {
        match item {
            Item::Function(function) => {
                walk_block_nodes(&function.body, &mut |_| {}, &mut expression, &mut |_| {})
            }
            Item::Implement(block) => {
                for method in &block.methods {
                    walk_block_nodes(&method.body, &mut |_| {}, &mut expression, &mut |_| {});
                }
            }
            Item::Struct(structure) => {
                for method in &structure.methods {
                    walk_block_nodes(&method.body, &mut |_| {}, &mut expression, &mut |_| {});
                }
            }
            Item::Verify(block) => {
                walk_block_nodes(&block.body, &mut |_| {}, &mut expression, &mut |_| {})
            }
            Item::Property(block) => {
                walk_block_nodes(&block.body, &mut |_| {}, &mut expression, &mut |_| {})
            }
            Item::VarDecl(declaration) => walk_expr_nodes(
                &declaration.value,
                &mut |_| {},
                &mut expression,
                &mut |_| {},
            ),
            Item::Actor(actor) => {
                for state in &actor.state_fields {
                    walk_expr_nodes(&state.value, &mut |_| {}, &mut expression, &mut |_| {});
                }
                for handler in &actor.handlers {
                    walk_block_nodes(&handler.body, &mut |_| {}, &mut expression, &mut |_| {});
                }
            }
            _ => {}
        }
    }
    match matches.as_slice() {
        [node] => Some(*node),
        _ => None,
    }
}

pub(crate) fn closure_node_in_body<'a>(
    body: &'a Block,
    span: jett_common::Span,
) -> Option<&'a Expr> {
    let mut matches = Vec::new();
    walk_block_nodes(
        body,
        &mut |_| {},
        &mut |value| {
            if let Expr::InlineFn(_, _, body, _) = value {
                if body.span == span {
                    matches.push(value);
                }
            }
        },
        &mut |_| {},
    );
    match matches.as_slice() {
        [node] => Some(*node),
        _ => None,
    }
}

/// A negative result proves this body cannot directly construct an inline value.
/// Required materialization and scoped type binding remain conservative positives;
/// a dynamic factory call installs its own origin at its separate call boundary.
pub(crate) fn body_may_mint_inline_function(body: &Block) -> bool {
    let mut found_expression = false;
    let mut found_binding = false;
    walk_block_nodes(
        body,
        &mut |_| {},
        &mut |expression| {
            found_expression |= matches!(
                expression,
                Expr::InlineFn(..) | Expr::Comptime(..) | Expr::Error(..)
            );
        },
        &mut |statement| {
            found_binding |= matches!(statement, Stmt::ComptimeTypeBind(_));
        },
    );
    found_expression || found_binding
}

fn walk_block_nodes<'a>(
    block: &'a Block,
    binding: &mut impl FnMut(&'a ComptimeTypeBindStmt),
    expression: &mut impl FnMut(&'a Expr),
    statement: &mut impl FnMut(&'a Stmt),
) {
    for node in &block.stmts {
        statement(node);
        match node {
            Stmt::VarDecl(value) => walk_expr_nodes(&value.value, binding, expression, statement),
            Stmt::Assign(value) => {
                walk_expr_nodes(&value.target, binding, expression, statement);
                walk_expr_nodes(&value.value, binding, expression, statement);
            }
            Stmt::Return(value) => {
                if let Some(value) = &value.value {
                    walk_expr_nodes(value, binding, expression, statement);
                }
            }
            Stmt::Respond(value) => walk_expr_nodes(&value.value, binding, expression, statement),
            Stmt::ComptimeTypeBind(value) => {
                binding(value);
                walk_expr_nodes(&value.value, binding, expression, statement);
                walk_block_nodes(&value.body, binding, expression, statement);
            }
            Stmt::If(value) => {
                walk_expr_nodes(&value.condition, binding, expression, statement);
                walk_block_nodes(&value.then_block, binding, expression, statement);
                for (condition, block) in &value.else_ifs {
                    walk_expr_nodes(condition, binding, expression, statement);
                    walk_block_nodes(block, binding, expression, statement);
                }
                if let Some(block) = &value.else_block {
                    walk_block_nodes(block, binding, expression, statement);
                }
            }
            Stmt::For(value) => {
                walk_expr_nodes(&value.iterable, binding, expression, statement);
                walk_block_nodes(&value.body, binding, expression, statement);
            }
            Stmt::While(value) => {
                walk_expr_nodes(&value.condition, binding, expression, statement);
                walk_block_nodes(&value.body, binding, expression, statement);
            }
            Stmt::Match(value) => {
                walk_expr_nodes(&value.expr, binding, expression, statement);
                for arm in &value.arms {
                    walk_block_nodes(&arm.body, binding, expression, statement);
                }
            }
            Stmt::Expr(value) => walk_expr_nodes(&value.expr, binding, expression, statement),
            Stmt::Assert(value) => {
                walk_expr_nodes(&value.condition, binding, expression, statement);
                if let Some(message) = &value.message {
                    walk_expr_nodes(message, binding, expression, statement);
                }
            }
            Stmt::Breakpoint(value) => {
                if let Some(condition) = &value.condition {
                    walk_expr_nodes(condition, binding, expression, statement);
                }
            }
            Stmt::Use(_) | Stmt::Trace(_) | Stmt::Break(_) | Stmt::Continue(_) => {}
        }
    }
}

fn walk_expr_nodes<'a>(
    value: &'a Expr,
    binding: &mut impl FnMut(&'a ComptimeTypeBindStmt),
    expression: &mut impl FnMut(&'a Expr),
    statement: &mut impl FnMut(&'a Stmt),
) {
    expression(value);
    match value {
        Expr::Binary(left, _, right, _) => {
            walk_expr_nodes(left, binding, expression, statement);
            walk_expr_nodes(right, binding, expression, statement);
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
        | Expr::Cancel(value, _) => walk_expr_nodes(value, binding, expression, statement),
        Expr::Call(callee, arguments, _) | Expr::GenericCall(callee, _, arguments, _) => {
            walk_expr_nodes(callee, binding, expression, statement);
            for argument in arguments {
                walk_expr_nodes(&argument.value, binding, expression, statement);
            }
        }
        Expr::ListConstruct(values, _) => {
            for value in values {
                walk_expr_nodes(value, binding, expression, statement);
            }
        }
        Expr::MapConstruct(values, _) => {
            for (key, value) in values {
                walk_expr_nodes(key, binding, expression, statement);
                walk_expr_nodes(value, binding, expression, statement);
            }
        }
        Expr::Handle(value, _, block, _) => {
            walk_expr_nodes(value, binding, expression, statement);
            walk_block_nodes(block, binding, expression, statement);
        }
        Expr::StringInterpolation(parts, _) => {
            for part in parts {
                match part {
                    StringPart::Expr(value) => {
                        walk_expr_nodes(value, binding, expression, statement);
                    }
                    StringPart::Literal(_) => {}
                }
            }
        }
        Expr::Pipeline(value, steps, _) => {
            walk_expr_nodes(value, binding, expression, statement);
            for step in steps {
                walk_expr_nodes(&step.function, binding, expression, statement);
                for argument in &step.extra_args {
                    walk_expr_nodes(&argument.value, binding, expression, statement);
                }
                if let Some(handle) = &step.handle {
                    walk_block_nodes(&handle.body, binding, expression, statement);
                }
            }
        }
        Expr::InlineFn(_, _, body, _) => walk_block_nodes(body, binding, expression, statement),
        Expr::IntLiteral(_, _)
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
