use jett_diagnostics::Diagnostic;
use jett_parser::ast::{self, BinOp, Block, Expr, FunctionDef, Item, Module, Stmt, StringPart};

use crate::errors;

const MAX_STATEMENTS: usize = 100;
const MAX_NESTING_DEPTH: usize = 4;
const MAX_CYCLOMATIC_COMPLEXITY: usize = 10;

#[derive(Debug, Default)]
struct FunctionMetrics {
    statements: usize,
    max_nesting_depth: usize,
    cyclomatic_complexity: usize,
}

pub fn check_complexity(module: &Module) -> Vec<Diagnostic> {
    let mut checker = ComplexityChecker {
        diagnostics: Vec::new(),
    };
    checker.check_module(module);
    checker.diagnostics
}

struct ComplexityChecker {
    diagnostics: Vec<Diagnostic>,
}

impl ComplexityChecker {
    fn check_module(&mut self, module: &Module) {
        for item in &module.items {
            self.check_item(item);
        }
    }

    fn check_item(&mut self, item: &Item) {
        match item {
            Item::Function(func) => self.check_function(func, &func.name.name),
            Item::Struct(def) => {
                for method in &def.methods {
                    self.check_function(method, &format!("{}.{}", def.name.name, method.name.name));
                }
            }
            Item::Implement(block) => {
                for method in &block.methods {
                    self.check_function(method, &method.name.name);
                }
            }
            Item::Actor(actor) => {
                for handler in &actor.handlers {
                    self.check_block_like_function(
                        &format!("{}.{}", actor.name.name, handler.name.name),
                        &handler.body,
                        handler.span,
                    );
                }
            }
            _ => {}
        }
    }

    fn check_function(&mut self, func: &FunctionDef, name: &str) {
        self.check_block_like_function(name, &func.body, func.span);
    }

    fn check_block_like_function(&mut self, name: &str, body: &Block, span: jett_common::Span) {
        let mut metrics = FunctionMetrics {
            cyclomatic_complexity: 1,
            ..FunctionMetrics::default()
        };
        self.collect_block(body, 0, &mut metrics);

        if metrics.statements > MAX_STATEMENTS {
            self.diagnostics.push(errors::function_statement_limit(
                name,
                metrics.statements,
                MAX_STATEMENTS,
                span,
            ));
        }
        if metrics.max_nesting_depth > MAX_NESTING_DEPTH {
            self.diagnostics.push(errors::function_nesting_depth_limit(
                name,
                metrics.max_nesting_depth,
                MAX_NESTING_DEPTH,
                span,
            ));
        }
        if metrics.cyclomatic_complexity > MAX_CYCLOMATIC_COMPLEXITY {
            self.diagnostics
                .push(errors::function_cyclomatic_complexity_limit(
                    name,
                    metrics.cyclomatic_complexity,
                    MAX_CYCLOMATIC_COMPLEXITY,
                    span,
                ));
        }
    }

    fn collect_block(&mut self, block: &Block, depth: usize, metrics: &mut FunctionMetrics) {
        for stmt in &block.stmts {
            self.collect_stmt(stmt, depth, metrics);
        }
    }

    fn collect_nested_block(
        &mut self,
        block: &Block,
        parent_depth: usize,
        metrics: &mut FunctionMetrics,
    ) {
        let depth = parent_depth + 1;
        metrics.max_nesting_depth = metrics.max_nesting_depth.max(depth);
        self.collect_block(block, depth, metrics);
    }

    fn count_stmt(&self, stmt: &Stmt, metrics: &mut FunctionMetrics) {
        if !matches!(stmt, Stmt::Use(_)) {
            metrics.statements += 1;
        }
    }

    fn collect_stmt(&mut self, stmt: &Stmt, depth: usize, metrics: &mut FunctionMetrics) {
        self.count_stmt(stmt, metrics);
        match stmt {
            Stmt::VarDecl(decl) => self.collect_expr(&decl.value, depth, metrics),
            Stmt::Assign(assign) => {
                self.collect_expr(&assign.target, depth, metrics);
                self.collect_expr(&assign.value, depth, metrics);
            }
            Stmt::Return(ret) => {
                if let Some(value) = &ret.value {
                    self.collect_expr(value, depth, metrics);
                }
            }
            Stmt::Respond(resp) => self.collect_expr(&resp.value, depth, metrics),
            Stmt::ComptimeTypeBind(bind) => {
                self.collect_expr(&bind.value, depth, metrics);
                self.collect_nested_block(&bind.body, depth, metrics);
            }
            Stmt::If(if_stmt) => {
                metrics.cyclomatic_complexity += 1 + if_stmt.else_ifs.len();
                self.collect_expr(&if_stmt.condition, depth, metrics);
                self.collect_nested_block(&if_stmt.then_block, depth, metrics);
                for (condition, block) in &if_stmt.else_ifs {
                    self.collect_expr(condition, depth, metrics);
                    self.collect_nested_block(block, depth, metrics);
                }
                if let Some(block) = &if_stmt.else_block {
                    self.collect_nested_block(block, depth, metrics);
                }
            }
            Stmt::For(for_stmt) => {
                metrics.cyclomatic_complexity += 1;
                self.collect_expr(&for_stmt.iterable, depth, metrics);
                self.collect_nested_block(&for_stmt.body, depth, metrics);
            }
            Stmt::While(while_stmt) => {
                metrics.cyclomatic_complexity += 1;
                self.collect_expr(&while_stmt.condition, depth, metrics);
                self.collect_nested_block(&while_stmt.body, depth, metrics);
            }
            Stmt::Match(match_stmt) => {
                metrics.cyclomatic_complexity += match_stmt.arms.len().max(1);
                self.collect_expr(&match_stmt.expr, depth, metrics);
                for arm in &match_stmt.arms {
                    self.collect_nested_block(&arm.body, depth, metrics);
                }
            }
            Stmt::Expr(expr_stmt) => self.collect_expr(&expr_stmt.expr, depth, metrics),
            Stmt::Assert(assert_stmt) => {
                self.collect_expr(&assert_stmt.condition, depth, metrics);
                if let Some(message) = &assert_stmt.message {
                    self.collect_expr(message, depth, metrics);
                }
            }
            Stmt::Breakpoint(breakpoint) => {
                if let Some(condition) = &breakpoint.condition {
                    self.collect_expr(condition, depth, metrics);
                }
            }
            Stmt::Use(_) | Stmt::Trace(_) | Stmt::Break(_) | Stmt::Continue(_) => {}
        }
    }

    fn collect_expr(&mut self, expr: &Expr, depth: usize, metrics: &mut FunctionMetrics) {
        match expr {
            Expr::Binary(lhs, op, rhs, _) => {
                if matches!(op, BinOp::And | BinOp::Or) {
                    metrics.cyclomatic_complexity += 1;
                }
                self.collect_expr(lhs, depth, metrics);
                self.collect_expr(rhs, depth, metrics);
            }
            Expr::Unary(_, inner, _)
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
            | Expr::Cancel(inner, _) => self.collect_expr(inner, depth, metrics),
            Expr::FieldAccess(base, _, _) => self.collect_expr(base, depth, metrics),
            Expr::Call(callee, args, _) => {
                self.collect_expr(callee, depth, metrics);
                self.collect_call_args(args, depth, metrics);
            }
            Expr::GenericCall(callee, _, args, _) => {
                self.collect_expr(callee, depth, metrics);
                self.collect_call_args(args, depth, metrics);
            }
            Expr::ListConstruct(items, _) => {
                for item in items {
                    self.collect_expr(item, depth, metrics);
                }
            }
            Expr::MapConstruct(entries, _) => {
                for (key, value) in entries {
                    self.collect_expr(key, depth, metrics);
                    self.collect_expr(value, depth, metrics);
                }
            }
            Expr::Handle(base, _, block, _) => {
                metrics.cyclomatic_complexity += 1;
                self.collect_expr(base, depth, metrics);
                self.collect_nested_block(block, depth, metrics);
            }
            Expr::StringInterpolation(parts, _) => {
                for part in parts {
                    if let StringPart::Expr(inner) = part {
                        self.collect_expr(inner, depth, metrics);
                    }
                }
            }
            Expr::Pipeline(base, steps, _) => {
                self.collect_expr(base, depth, metrics);
                for step in steps {
                    self.collect_expr(&step.function, depth, metrics);
                    self.collect_call_args(&step.extra_args, depth, metrics);
                }
            }
            Expr::InlineFn(_, _, body, span) => {
                self.check_block_like_function("<inline function>", body, *span);
            }
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

    fn collect_call_args(
        &mut self,
        args: &[ast::CallArg],
        depth: usize,
        metrics: &mut FunctionMetrics,
    ) {
        for arg in args {
            self.collect_expr(&arg.value, depth, metrics);
        }
    }
}
