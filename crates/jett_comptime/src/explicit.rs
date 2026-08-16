use std::collections::HashMap;
use std::sync::Arc;

use jett_common::{FileId, Span};
use jett_diagnostics::Diagnostic;
use jett_parser::ast::{Block, Expr, Item, Module, Stmt, StringPart};
use jett_types::ReflectionMetadata;

use crate::{Interpreter, Value};

/// Evaluate every explicit `comptime` expression in a checked module.
///
/// The expressions are evaluated with an empty lexical environment. This is
/// deliberate: an explicit comptime value must be closed and reproducible at
/// build time, rather than depending on a runtime parameter or local.
pub fn evaluate_explicit_comptime_expressions(
    module: &Module,
    reflection_metadata: Arc<ReflectionMetadata>,
    checked_expression_types: Arc<HashMap<Span, String>>,
) -> (HashMap<Span, Value>, Vec<Diagnostic>) {
    let mut expressions = Vec::new();
    collect_module_expressions(module, &mut expressions);

    let mut interpreter = Interpreter::new();
    interpreter.set_reflection_metadata(reflection_metadata);
    interpreter.set_checked_expression_types(checked_expression_types);
    interpreter.register_module(module);

    let mut values = HashMap::new();
    let mut diagnostics = Vec::new();
    for (namespace, expression, span) in expressions {
        match interpreter.eval_expr_in_namespace(namespace.as_deref(), expression) {
            Ok(value) => {
                values.insert(span, value);
            }
            Err(error) => diagnostics.push(Diagnostic::error(
                9001,
                format!(
                    "`comptime` expression must be closed and evaluable during compilation: {error}"
                ),
                span,
            )),
        }
    }

    (values, diagnostics)
}

fn collect_module_expressions<'a>(
    module: &'a Module,
    expressions: &mut Vec<(Option<String>, &'a Expr, Span)>,
) {
    let mut current_file = None;
    let mut current_namespace = None;
    for item in &module.items {
        let file = item_file(item);
        if current_file.is_some_and(|current| current != file) {
            current_namespace = None;
        }
        current_file = Some(file);
        if let Item::Namespace(namespace) = item {
            current_namespace = Some(namespace.name.name.clone());
        }
        collect_item(item, current_namespace.as_deref(), expressions);
    }
}

fn item_file(item: &Item) -> FileId {
    match item {
        Item::Namespace(item) => item.span.file,
        Item::Function(item) => item.span.file,
        Item::Mutual(item) => item.span.file,
        Item::Interface(item) => item.span.file,
        Item::Implement(item) => item.span.file,
        Item::Struct(item) => item.span.file,
        Item::Bitfield(item) => item.span.file,
        Item::Enum(item) => item.span.file,
        Item::Machine(item) => item.span.file,
        Item::Actor(item) => item.span.file,
        Item::VarDecl(item) => item.span.file,
        Item::Verify(item) => item.span.file,
        Item::Property(item) => item.span.file,
        Item::TypeAlias(item) => item.span.file,
    }
}

fn collect_item<'a>(
    item: &'a Item,
    namespace: Option<&str>,
    expressions: &mut Vec<(Option<String>, &'a Expr, Span)>,
) {
    match item {
        Item::Function(function) => collect_block(&function.body, namespace, expressions),
        Item::Implement(implementation) => {
            for method in &implementation.methods {
                collect_block(&method.body, namespace, expressions);
            }
        }
        Item::Struct(structure) => {
            for method in &structure.methods {
                collect_block(&method.body, namespace, expressions);
            }
        }
        Item::Actor(actor) => {
            for field in &actor.state_fields {
                collect_expr(&field.value, namespace, expressions);
            }
            for handler in &actor.handlers {
                collect_block(&handler.body, namespace, expressions);
            }
        }
        Item::VarDecl(declaration) => collect_expr(&declaration.value, namespace, expressions),
        Item::Verify(verify) => collect_block(&verify.body, namespace, expressions),
        Item::Property(property) => collect_block(&property.body, namespace, expressions),
        Item::TypeAlias(alias) => {
            if let Some(constraint) = &alias.constraint {
                collect_expr(constraint, namespace, expressions);
            }
        }
        Item::Namespace(_)
        | Item::Mutual(_)
        | Item::Interface(_)
        | Item::Bitfield(_)
        | Item::Enum(_)
        | Item::Machine(_) => {}
    }
}

fn collect_block<'a>(
    block: &'a Block,
    namespace: Option<&str>,
    expressions: &mut Vec<(Option<String>, &'a Expr, Span)>,
) {
    for statement in &block.stmts {
        collect_stmt(statement, namespace, expressions);
    }
}

fn collect_stmt<'a>(
    statement: &'a Stmt,
    namespace: Option<&str>,
    expressions: &mut Vec<(Option<String>, &'a Expr, Span)>,
) {
    match statement {
        Stmt::VarDecl(declaration) => collect_expr(&declaration.value, namespace, expressions),
        Stmt::Assign(assignment) => {
            collect_expr(&assignment.target, namespace, expressions);
            collect_expr(&assignment.value, namespace, expressions);
        }
        Stmt::Return(statement) => {
            if let Some(value) = &statement.value {
                collect_expr(value, namespace, expressions);
            }
        }
        Stmt::Respond(statement) => collect_expr(&statement.value, namespace, expressions),
        Stmt::ComptimeTypeBind(binding) => {
            collect_expr(&binding.value, namespace, expressions);
            collect_block(&binding.body, namespace, expressions);
        }
        Stmt::If(statement) => {
            collect_expr(&statement.condition, namespace, expressions);
            collect_block(&statement.then_block, namespace, expressions);
            for (condition, block) in &statement.else_ifs {
                collect_expr(condition, namespace, expressions);
                collect_block(block, namespace, expressions);
            }
            if let Some(block) = &statement.else_block {
                collect_block(block, namespace, expressions);
            }
        }
        Stmt::For(statement) => {
            collect_expr(&statement.iterable, namespace, expressions);
            collect_block(&statement.body, namespace, expressions);
        }
        Stmt::While(statement) => {
            collect_expr(&statement.condition, namespace, expressions);
            collect_block(&statement.body, namespace, expressions);
        }
        Stmt::Match(statement) => {
            collect_expr(&statement.expr, namespace, expressions);
            for arm in &statement.arms {
                collect_block(&arm.body, namespace, expressions);
            }
        }
        Stmt::Expr(statement) => collect_expr(&statement.expr, namespace, expressions),
        Stmt::Assert(statement) => {
            collect_expr(&statement.condition, namespace, expressions);
            if let Some(message) = &statement.message {
                collect_expr(message, namespace, expressions);
            }
        }
        Stmt::Breakpoint(statement) => {
            if let Some(condition) = &statement.condition {
                collect_expr(condition, namespace, expressions);
            }
        }
        Stmt::Use(_) | Stmt::Trace(_) | Stmt::Break(_) | Stmt::Continue(_) => {}
    }
}

fn collect_expr<'a>(
    expression: &'a Expr,
    namespace: Option<&str>,
    expressions: &mut Vec<(Option<String>, &'a Expr, Span)>,
) {
    match expression {
        Expr::Comptime(inner, span) => {
            expressions.push((namespace.map(str::to_string), inner, *span));
        }
        Expr::Binary(left, _, right, _) => {
            collect_expr(left, namespace, expressions);
            collect_expr(right, namespace, expressions);
        }
        Expr::Unary(_, inner, _)
        | Expr::FieldAccess(inner, _, _)
        | Expr::Paren(inner, _)
        | Expr::View(inner, _)
        | Expr::Ok(inner, _)
        | Expr::Fail(inner, _)
        | Expr::Some(inner, _)
        | Expr::Default(inner, _)
        | Expr::Declassify(inner, _)
        | Expr::Coarsen(inner, _)
        | Expr::At(inner, _, _)
        | Expr::Spawn(inner, _)
        | Expr::Send(inner, _)
        | Expr::Ask(inner, _)
        | Expr::Clone(inner, _)
        | Expr::Run(inner, _)
        | Expr::Join(inner, _)
        | Expr::Cancel(inner, _) => collect_expr(inner, namespace, expressions),
        Expr::Call(callee, arguments, _) => {
            collect_expr(callee, namespace, expressions);
            for argument in arguments {
                collect_expr(&argument.value, namespace, expressions);
            }
        }
        Expr::GenericCall(callee, _, arguments, _) => {
            collect_expr(callee, namespace, expressions);
            for argument in arguments {
                collect_expr(&argument.value, namespace, expressions);
            }
        }
        Expr::ListConstruct(items, _) => {
            for item in items {
                collect_expr(item, namespace, expressions);
            }
        }
        Expr::MapConstruct(entries, _) => {
            for (key, value) in entries {
                collect_expr(key, namespace, expressions);
                collect_expr(value, namespace, expressions);
            }
        }
        Expr::Handle(target, _, block, _) => {
            collect_expr(target, namespace, expressions);
            collect_block(block, namespace, expressions);
        }
        Expr::StringInterpolation(parts, _) => {
            for part in parts {
                if let StringPart::Expr(expression) = part {
                    collect_expr(expression, namespace, expressions);
                }
            }
        }
        Expr::Pipeline(initial, steps, _) => {
            collect_expr(initial, namespace, expressions);
            for step in steps {
                collect_expr(&step.function, namespace, expressions);
                for argument in &step.extra_args {
                    collect_expr(&argument.value, namespace, expressions);
                }
                if let Some(handle) = &step.handle {
                    collect_block(&handle.body, namespace, expressions);
                }
            }
        }
        Expr::InlineFn(_, _, block, _) => collect_block(block, namespace, expressions),
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
