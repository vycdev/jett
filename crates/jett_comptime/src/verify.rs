use jett_common::Span;
use jett_diagnostics::Diagnostic;
use jett_parser::ast::{FunctionDef, Item, Module};

use crate::interpreter::Interpreter;
use crate::value::Value;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// An error produced during compile-time evaluation.
#[derive(Debug, Clone)]
pub struct ComptimeError {
    pub message: String,
    pub span: Span,
}

impl ComptimeError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Run all verify blocks / assert-bearing functions in the module and return
/// any diagnostics produced by assertion failures.
///
/// Since `verify` blocks are not yet in the AST, this implementation scans
/// for top-level functions whose bodies contain at least one `assert`
/// statement and executes them as zero-argument verify functions.
pub fn run_verify_blocks(module: &Module) -> Vec<Diagnostic> {
    let mut interp = Interpreter::new();
    let mut diagnostics = Vec::new();

    // First pass: register all functions so they can call each other.
    let mut verify_functions: Vec<FunctionDef> = Vec::new();
    for item in &module.items {
        if let Item::Function(func) = item {
            interp.register_function(func);
            if has_assert_stmts(func) && func.params.is_empty() {
                verify_functions.push(func.clone());
            }
        }
    }

    // Second pass: execute the verify functions.
    for func in &verify_functions {
        match interp.call_function(&func.name.name, vec![]) {
            Ok(_) => {} // all assertions passed
            Err(msg) => {
                diagnostics.push(Diagnostic::error(
                    9000,
                    format!("comptime verify failed in '{}': {}", func.name.name, msg),
                    func.span,
                ));
            }
        }
    }

    diagnostics
}

/// Evaluate a single pure function at compile time with the given arguments.
pub fn eval_function(
    func: &FunctionDef,
    args: Vec<Value>,
) -> Result<Value, ComptimeError> {
    let mut interp = Interpreter::new();
    interp.register_function(func);
    interp
        .call_function(&func.name.name, args)
        .map_err(|msg| ComptimeError::new(msg, func.span))
}

/// Evaluate an assert condition in the given environment.
///
/// Returns `Ok(())` if the assertion passes, or a `ComptimeError` if it
/// fails or evaluates to a non-boolean value.
pub fn eval_assert(
    condition: &jett_parser::ast::Expr,
    interp: &mut Interpreter,
) -> Result<(), ComptimeError> {
    let span = condition.span();
    match interp.eval_expr(condition) {
        Ok(Value::Bool(true)) => Ok(()),
        Ok(Value::Bool(false)) => Err(ComptimeError::new("assertion failed", span)),
        Ok(other) => Err(ComptimeError::new(
            format!("assert condition must be boolean, got {other}"),
            span,
        )),
        Err(msg) => Err(ComptimeError::new(msg, span)),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns `true` if the function body contains at least one `assert` statement
/// (at the top level of the body).
fn has_assert_stmts(func: &FunctionDef) -> bool {
    func.body
        .stmts
        .iter()
        .any(|s| matches!(s, jett_parser::ast::Stmt::Assert(_)))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use jett_common::{FileId, Span};
    use jett_parser::ast::*;

    use super::*;

    fn sp() -> Span {
        Span::new(FileId::new(0), 0, 0)
    }

    fn ident(name: &str) -> Ident {
        Ident {
            name: name.to_string(),
            span: sp(),
        }
    }

    fn int(n: i64) -> Expr {
        Expr::IntLiteral(n, sp())
    }

    fn var(name: &str) -> Expr {
        Expr::Ident(ident(name))
    }

    fn binary(lhs: Expr, op: BinOp, rhs: Expr) -> Expr {
        Expr::Binary(Box::new(lhs), op, Box::new(rhs), sp())
    }

    fn type_named(name: &str) -> TypeExpr {
        TypeExpr::Named(ident(name))
    }

    fn block(stmts: Vec<Stmt>) -> Block {
        Block {
            stmts,
            span: sp(),
        }
    }

    fn return_stmt(value: Expr) -> Stmt {
        Stmt::Return(ReturnStmt {
            value: Some(value),
            span: sp(),
        })
    }

    fn assert_stmt_ast(condition: Expr) -> Stmt {
        Stmt::Assert(AssertStmt {
            condition,
            message: None,
            span: sp(),
        })
    }

    fn func_def(name: &str, params: Vec<(&str, &str)>, body: Block) -> FunctionDef {
        FunctionDef {
            name: ident(name),
            params: params
                .into_iter()
                .map(|(pname, ptype)| Param {
                    view: false,
                    mutable: false,
                    name: ident(pname),
                    ty: type_named(ptype),
                    span: sp(),
                })
                .collect(),
            return_type: None,
            body,
            span: sp(),
        }
    }

    #[test]
    fn eval_function_simple() {
        let add = func_def(
            "add",
            vec![("a", "int64"), ("b", "int64")],
            block(vec![return_stmt(binary(var("a"), BinOp::Add, var("b")))]),
        );
        let result = eval_function(&add, vec![Value::Int64(10), Value::Int64(20)]).unwrap();
        assert_eq!(result, Value::Int64(30));
    }

    #[test]
    fn eval_function_error() {
        let bad = func_def(
            "bad",
            vec![("a", "int64")],
            block(vec![return_stmt(binary(var("a"), BinOp::Div, int(0)))]),
        );
        let result = eval_function(&bad, vec![Value::Int64(1)]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("division by zero"));
    }

    #[test]
    fn eval_assert_passes() {
        let mut interp = crate::interpreter::Interpreter::new();
        let cond = binary(int(1), BinOp::Eq, int(1));
        eval_assert(&cond, &mut interp).unwrap();
    }

    #[test]
    fn eval_assert_fails() {
        let mut interp = crate::interpreter::Interpreter::new();
        let cond = binary(int(1), BinOp::Eq, int(2));
        assert!(eval_assert(&cond, &mut interp).is_err());
    }

    #[test]
    fn run_verify_blocks_passing() {
        let verify_fn = func_def(
            "test_verify",
            vec![],
            block(vec![assert_stmt_ast(binary(int(2), BinOp::Add, int(3)).pipe_eq(int(5)))]),
        );
        let module = Module {
            items: vec![Item::Function(verify_fn)],
            span: sp(),
        };
        let diags = run_verify_blocks(&module);
        assert!(diags.is_empty());
    }

    #[test]
    fn run_verify_blocks_failing() {
        // assert 1 == 2  -> should produce a diagnostic
        let verify_fn = func_def(
            "test_verify",
            vec![],
            block(vec![assert_stmt_ast(binary(int(1), BinOp::Eq, int(2)))]),
        );
        let module = Module {
            items: vec![Item::Function(verify_fn)],
            span: sp(),
        };
        let diags = run_verify_blocks(&module);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("comptime verify failed"));
    }

    #[test]
    fn run_verify_blocks_calls_helper_function() {
        // function double(x: int64) returns int64:
        //     return x * 2
        let double_fn = func_def(
            "double",
            vec![("x", "int64")],
            block(vec![return_stmt(binary(var("x"), BinOp::Mul, int(2)))]),
        );

        // function test_verify():
        //     assert double(5) == 10
        let call_double = Expr::Call(
            Box::new(var("double")),
            vec![CallArg {
                name: None,
                value: int(5),
                span: sp(),
            }],
            sp(),
        );
        let verify_fn = func_def(
            "test_verify",
            vec![],
            block(vec![assert_stmt_ast(binary(call_double, BinOp::Eq, int(10)))]),
        );

        let module = Module {
            items: vec![
                Item::Function(double_fn),
                Item::Function(verify_fn),
            ],
            span: sp(),
        };
        let diags = run_verify_blocks(&module);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    /// Helper trait to build `a == b` more concisely in tests.
    trait PipeEq {
        fn pipe_eq(self, other: Expr) -> Expr;
    }

    impl PipeEq for Expr {
        fn pipe_eq(self, other: Expr) -> Expr {
            Expr::Binary(Box::new(self), BinOp::Eq, Box::new(other), sp())
        }
    }
}
