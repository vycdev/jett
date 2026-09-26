use std::collections::HashMap;
use std::sync::Arc;

use jett_common::{FileId, Span};
use jett_diagnostics::Diagnostic;
use jett_parser::ast::{Block, Expr, Item, Module, Stmt, StringPart};
use jett_types::ReflectionMetadata;

use crate::{Interpreter, Value};

type CollectedExpression<'a> = (Option<String>, HashMap<String, String>, &'a Expr, Span);

/// Evaluate every explicit `comptime` expression in a checked module.
///
/// The expressions are evaluated without runtime locals or parameters. Visible
/// `use` aliases are lexical name bindings and are carried into evaluation.
/// An explicit comptime value remains closed and reproducible at build time.
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
    for (namespace, aliases, expression, span) in expressions {
        match interpreter.eval_expr_in_namespace_with_aliases(
            namespace.as_deref(),
            &aliases,
            expression,
        ) {
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
    expressions: &mut Vec<CollectedExpression<'a>>,
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
        Item::Resource(item) => item.span.file,
        Item::TypeAlias(item) => item.span.file,
    }
}

fn collect_item<'a>(
    item: &'a Item,
    namespace: Option<&str>,
    expressions: &mut Vec<CollectedExpression<'a>>,
) {
    let aliases = &HashMap::new();
    match item {
        Item::Function(function) => collect_block(&function.body, namespace, aliases, expressions),
        Item::Implement(implementation) => {
            for method in &implementation.methods {
                collect_block(&method.body, namespace, aliases, expressions);
            }
        }
        Item::Struct(structure) => {
            for method in &structure.methods {
                collect_block(&method.body, namespace, aliases, expressions);
            }
        }
        Item::Actor(actor) => {
            for field in &actor.state_fields {
                collect_expr(&field.value, namespace, aliases, expressions);
            }
            for handler in &actor.handlers {
                collect_block(&handler.body, namespace, aliases, expressions);
            }
        }
        Item::VarDecl(declaration) => {
            collect_expr(&declaration.value, namespace, aliases, expressions)
        }
        Item::Verify(verify) => collect_block(&verify.body, namespace, aliases, expressions),
        Item::Property(property) => collect_block(&property.body, namespace, aliases, expressions),
        Item::TypeAlias(alias) => {
            if let Some(constraint) = &alias.constraint {
                collect_expr(constraint, namespace, aliases, expressions);
            }
        }
        Item::Namespace(_)
        | Item::Mutual(_)
        | Item::Interface(_)
        | Item::Bitfield(_)
        | Item::Enum(_)
        | Item::Resource(_)
        | Item::Machine(_) => {}
    }
}

fn collect_block<'a>(
    block: &'a Block,
    namespace: Option<&str>,
    aliases: &HashMap<String, String>,
    expressions: &mut Vec<CollectedExpression<'a>>,
) {
    let mut visible_aliases = aliases.clone();
    for statement in &block.stmts {
        collect_stmt(statement, namespace, &mut visible_aliases, expressions);
    }
}

fn collect_stmt<'a>(
    statement: &'a Stmt,
    namespace: Option<&str>,
    aliases: &mut HashMap<String, String>,
    expressions: &mut Vec<CollectedExpression<'a>>,
) {
    match statement {
        Stmt::VarDecl(declaration) => {
            collect_expr(&declaration.value, namespace, aliases, expressions)
        }
        Stmt::Assign(assignment) => {
            collect_expr(&assignment.target, namespace, aliases, expressions);
            collect_expr(&assignment.value, namespace, aliases, expressions);
        }
        Stmt::Return(statement) => {
            if let Some(value) = &statement.value {
                collect_expr(value, namespace, aliases, expressions);
            }
        }
        Stmt::Respond(statement) => collect_expr(&statement.value, namespace, aliases, expressions),
        Stmt::ComptimeTypeBind(binding) => {
            collect_expr(&binding.value, namespace, aliases, expressions);
            collect_block(&binding.body, namespace, aliases, expressions);
        }
        Stmt::If(statement) => {
            collect_expr(&statement.condition, namespace, aliases, expressions);
            collect_block(&statement.then_block, namespace, aliases, expressions);
            for (condition, block) in &statement.else_ifs {
                collect_expr(condition, namespace, aliases, expressions);
                collect_block(block, namespace, aliases, expressions);
            }
            if let Some(block) = &statement.else_block {
                collect_block(block, namespace, aliases, expressions);
            }
        }
        Stmt::For(statement) => {
            collect_expr(&statement.iterable, namespace, aliases, expressions);
            collect_block(&statement.body, namespace, aliases, expressions);
        }
        Stmt::While(statement) => {
            collect_expr(&statement.condition, namespace, aliases, expressions);
            collect_block(&statement.body, namespace, aliases, expressions);
        }
        Stmt::Match(statement) => {
            collect_expr(&statement.expr, namespace, aliases, expressions);
            for arm in &statement.arms {
                collect_block(&arm.body, namespace, aliases, expressions);
            }
        }
        Stmt::Expr(statement) => collect_expr(&statement.expr, namespace, aliases, expressions),
        Stmt::Assert(statement) => {
            collect_expr(&statement.condition, namespace, aliases, expressions);
            if let Some(message) = &statement.message {
                collect_expr(message, namespace, aliases, expressions);
            }
        }
        Stmt::Breakpoint(statement) => {
            if let Some(condition) = &statement.condition {
                collect_expr(condition, namespace, aliases, expressions);
            }
        }
        Stmt::Use(declaration) => {
            let bound_name =
                Interpreter::use_bound_name(&declaration.path.name, declaration.alias.as_ref());
            aliases.insert(bound_name, declaration.path.name.clone());
        }
        Stmt::Trace(_) | Stmt::Break(_) | Stmt::Continue(_) => {}
    }
}

fn collect_expr<'a>(
    expression: &'a Expr,
    namespace: Option<&str>,
    aliases: &HashMap<String, String>,
    expressions: &mut Vec<CollectedExpression<'a>>,
) {
    match expression {
        Expr::Comptime(inner, span) => {
            expressions.push((namespace.map(str::to_string), aliases.clone(), inner, *span));
        }
        Expr::Binary(left, _, right, _) => {
            collect_expr(left, namespace, aliases, expressions);
            collect_expr(right, namespace, aliases, expressions);
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
        | Expr::Cancel(inner, _) => collect_expr(inner, namespace, aliases, expressions),
        Expr::Call(callee, arguments, _) => {
            collect_expr(callee, namespace, aliases, expressions);
            for argument in arguments {
                collect_expr(&argument.value, namespace, aliases, expressions);
            }
        }
        Expr::GenericCall(callee, _, arguments, _) => {
            collect_expr(callee, namespace, aliases, expressions);
            for argument in arguments {
                collect_expr(&argument.value, namespace, aliases, expressions);
            }
        }
        Expr::ListConstruct(items, _) => {
            for item in items {
                collect_expr(item, namespace, aliases, expressions);
            }
        }
        Expr::MapConstruct(entries, _) => {
            for (key, value) in entries {
                collect_expr(key, namespace, aliases, expressions);
                collect_expr(value, namespace, aliases, expressions);
            }
        }
        Expr::Handle(target, _, block, _) => {
            collect_expr(target, namespace, aliases, expressions);
            collect_block(block, namespace, aliases, expressions);
        }
        Expr::StringInterpolation(parts, _) => {
            for part in parts {
                if let StringPart::Expr(expression) = part {
                    collect_expr(expression, namespace, aliases, expressions);
                }
            }
        }
        Expr::Pipeline(initial, steps, _) => {
            collect_expr(initial, namespace, aliases, expressions);
            for step in steps {
                collect_expr(&step.function, namespace, aliases, expressions);
                for argument in &step.extra_args {
                    collect_expr(&argument.value, namespace, aliases, expressions);
                }
                if let Some(handle) = &step.handle {
                    collect_block(&handle.body, namespace, aliases, expressions);
                }
            }
        }
        Expr::InlineFn(_, _, block, _) => collect_block(block, namespace, aliases, expressions),
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
