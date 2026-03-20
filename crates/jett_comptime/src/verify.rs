use jett_common::Span;
use jett_diagnostics::Diagnostic;
use jett_parser::ast::{FunctionDef, Item, Module, VerifyBlock};

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
// Verify result
// ---------------------------------------------------------------------------

/// Outcome of running a single verify block.
#[derive(Debug, Clone)]
pub struct VerifyResult {
    pub name: String,
    pub passed: bool,
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Run all verify blocks in the module and return any diagnostics produced by
/// assertion failures.
///
/// This looks for:
/// 1. `Item::Verify` blocks (the primary mechanism).
/// 2. Legacy fallback: top-level zero-argument functions whose bodies contain
///    `assert` statements (kept for backward compatibility).
pub fn run_verify_blocks(module: &Module) -> Vec<Diagnostic> {
    let results = run_verify_blocks_detailed(module);
    results
        .into_iter()
        .filter_map(|r| {
            if r.passed {
                None
            } else {
                Some(Diagnostic::error(
                    9000,
                    format!(
                        "comptime verify failed in '{}': {}",
                        r.name,
                        r.error.unwrap_or_default()
                    ),
                    // We don't have the span here; use a dummy.  The
                    // caller (build_file) already has the module.
                    Span::new(jett_common::FileId::new(0), 0, 0),
                ))
            }
        })
        .collect()
}

/// Run all verify blocks and return structured results.  Used by the
/// `jett test` command for per-block reporting.
pub fn run_verify_blocks_detailed(module: &Module) -> Vec<VerifyResult> {
    let mut interp = Interpreter::new();
    let mut results = Vec::new();

    // First pass: register all functions and type aliases so verify blocks
    // can call them and use refinement types.
    let mut legacy_verify_functions: Vec<FunctionDef> = Vec::new();
    let mut verify_blocks: Vec<VerifyBlock> = Vec::new();
    for item in &module.items {
        match item {
            Item::Function(func) => {
                interp.register_function(func);
                if has_assert_stmts(func) && func.params.is_empty() && func.name.name != "main" {
                    legacy_verify_functions.push(func.clone());
                }
            }
            Item::Verify(vb) => {
                verify_blocks.push(vb.clone());
            }
            Item::TypeAlias(alias) => {
                interp.register_type_alias(alias);
            }
            Item::Machine(machine) => {
                interp.register_machine(machine);
            }
            _ => {}
        }
    }

    // Execute proper verify blocks.
    for vb in &verify_blocks {
        match interp.exec_block(&vb.body) {
            Ok(_) => {
                results.push(VerifyResult {
                    name: vb.name.name.clone(),
                    passed: true,
                    error: None,
                });
            }
            Err(msg) => {
                results.push(VerifyResult {
                    name: vb.name.name.clone(),
                    passed: false,
                    error: Some(msg),
                });
            }
        }
    }

    // Execute legacy verify functions (zero-arg functions with asserts).
    for func in &legacy_verify_functions {
        match interp.call_function(&func.name.name, vec![]) {
            Ok(_) => {
                results.push(VerifyResult {
                    name: func.name.name.clone(),
                    passed: true,
                    error: None,
                });
            }
            Err(msg) => {
                results.push(VerifyResult {
                    name: func.name.name.clone(),
                    passed: false,
                    error: Some(msg),
                });
            }
        }
    }

    results
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

    // -----------------------------------------------------------------------
    // Item::Verify block tests
    // -----------------------------------------------------------------------

    fn verify_block_item(name: &str, body: Block) -> VerifyBlock {
        VerifyBlock {
            name: ident(name),
            body,
            span: sp(),
        }
    }

    #[test]
    fn verify_block_passing() {
        // function add(a: int64, b: int64) returns int64:
        //     return a + b
        let add_fn = func_def(
            "add",
            vec![("a", "int64"), ("b", "int64")],
            block(vec![return_stmt(binary(var("a"), BinOp::Add, var("b")))]),
        );

        // verify add:
        //     assert add(2, 3) == 5
        let call_add = Expr::Call(
            Box::new(var("add")),
            vec![
                CallArg { name: None, value: int(2), span: sp() },
                CallArg { name: None, value: int(3), span: sp() },
            ],
            sp(),
        );
        let vb = verify_block_item(
            "add",
            block(vec![assert_stmt_ast(binary(call_add, BinOp::Eq, int(5)))]),
        );

        let module = Module {
            items: vec![Item::Function(add_fn), Item::Verify(vb)],
            span: sp(),
        };
        let results = run_verify_blocks_detailed(&module);
        assert_eq!(results.len(), 1);
        assert!(results[0].passed, "expected verify block to pass");
        assert_eq!(results[0].name, "add");
    }

    #[test]
    fn verify_block_failing() {
        // function add(a: int64, b: int64) returns int64:
        //     return a + b
        let add_fn = func_def(
            "add",
            vec![("a", "int64"), ("b", "int64")],
            block(vec![return_stmt(binary(var("a"), BinOp::Add, var("b")))]),
        );

        // verify add:
        //     assert add(2, 3) == 99   <-- wrong!
        let call_add = Expr::Call(
            Box::new(var("add")),
            vec![
                CallArg { name: None, value: int(2), span: sp() },
                CallArg { name: None, value: int(3), span: sp() },
            ],
            sp(),
        );
        let vb = verify_block_item(
            "add",
            block(vec![assert_stmt_ast(binary(call_add, BinOp::Eq, int(99)))]),
        );

        let module = Module {
            items: vec![Item::Function(add_fn), Item::Verify(vb)],
            span: sp(),
        };
        let results = run_verify_blocks_detailed(&module);
        assert_eq!(results.len(), 1);
        assert!(!results[0].passed, "expected verify block to fail");
        assert_eq!(results[0].name, "add");
        assert!(results[0].error.is_some());
    }

    // -----------------------------------------------------------------------
    // Refinement type tests
    // -----------------------------------------------------------------------

    fn type_alias(name: &str, base: &str, constraint: Option<Expr>) -> TypeAlias {
        TypeAlias {
            name: ident(name),
            base_type: type_named(base),
            constraint,
            span: sp(),
        }
    }

    fn var_decl_stmt(ty_name: &str, var_name: &str, value: Expr) -> Stmt {
        Stmt::VarDecl(VarDecl {
            mutable: false,
            ty: type_named(ty_name),
            name: ident(var_name),
            value,
            span: sp(),
        })
    }

    #[test]
    fn refinement_type_valid_port() {
        // type Port = int64 where value >= 1 && value <= 65535
        // verify port_test:
        //     Port p = 8080
        //     assert p == 8080
        let constraint = binary(
            binary(var("value"), BinOp::GtEq, int(1)),
            BinOp::And,
            binary(var("value"), BinOp::LtEq, int(65535)),
        );
        let port_alias = type_alias("Port", "int64", Some(constraint));

        let vb = verify_block_item(
            "port_test",
            block(vec![
                var_decl_stmt("Port", "p", int(8080)),
                assert_stmt_ast(binary(var("p"), BinOp::Eq, int(8080))),
            ]),
        );

        let module = Module {
            items: vec![Item::TypeAlias(port_alias), Item::Verify(vb)],
            span: sp(),
        };
        let results = run_verify_blocks_detailed(&module);
        assert_eq!(results.len(), 1);
        assert!(results[0].passed, "expected verify block to pass: {:?}", results[0].error);
    }

    #[test]
    fn refinement_type_invalid_port() {
        // type Port = int64 where value >= 1 && value <= 65535
        // verify port_test:
        //     Port p = 0     # should fail — 0 is not >= 1
        let constraint = binary(
            binary(var("value"), BinOp::GtEq, int(1)),
            BinOp::And,
            binary(var("value"), BinOp::LtEq, int(65535)),
        );
        let port_alias = type_alias("Port", "int64", Some(constraint));

        let vb = verify_block_item(
            "port_test",
            block(vec![
                var_decl_stmt("Port", "p", int(0)),
            ]),
        );

        let module = Module {
            items: vec![Item::TypeAlias(port_alias), Item::Verify(vb)],
            span: sp(),
        };
        let results = run_verify_blocks_detailed(&module);
        assert_eq!(results.len(), 1);
        assert!(!results[0].passed, "expected verify block to fail for invalid port");
        assert!(
            results[0].error.as_ref().unwrap().contains("refinement"),
            "error should mention refinement: {:?}",
            results[0].error,
        );
    }

    #[test]
    fn refinement_type_boundary_valid() {
        // Port p = 1 should pass (boundary value)
        let constraint = binary(
            binary(var("value"), BinOp::GtEq, int(1)),
            BinOp::And,
            binary(var("value"), BinOp::LtEq, int(65535)),
        );
        let port_alias = type_alias("Port", "int64", Some(constraint));

        let vb = verify_block_item(
            "port_boundary",
            block(vec![
                var_decl_stmt("Port", "p", int(1)),
                assert_stmt_ast(binary(var("p"), BinOp::Eq, int(1))),
            ]),
        );

        let module = Module {
            items: vec![Item::TypeAlias(port_alias), Item::Verify(vb)],
            span: sp(),
        };
        let results = run_verify_blocks_detailed(&module);
        assert_eq!(results.len(), 1);
        assert!(results[0].passed, "expected boundary value 1 to pass");
    }

    #[test]
    fn coarsen_strips_refinement() {
        // type Port = int64 where value >= 1 && value <= 65535
        // verify coarsen_test:
        //     Port p = 8080
        //     int64 raw = coarsen p
        //     assert raw == 8080
        let constraint = binary(
            binary(var("value"), BinOp::GtEq, int(1)),
            BinOp::And,
            binary(var("value"), BinOp::LtEq, int(65535)),
        );
        let port_alias = type_alias("Port", "int64", Some(constraint));

        let coarsen_expr = Expr::Coarsen(Box::new(var("p")), sp());

        let vb = verify_block_item(
            "coarsen_test",
            block(vec![
                var_decl_stmt("Port", "p", int(8080)),
                var_decl_stmt("int64", "raw", coarsen_expr),
                assert_stmt_ast(binary(var("raw"), BinOp::Eq, int(8080))),
            ]),
        );

        let module = Module {
            items: vec![Item::TypeAlias(port_alias), Item::Verify(vb)],
            span: sp(),
        };
        let results = run_verify_blocks_detailed(&module);
        assert_eq!(results.len(), 1);
        assert!(results[0].passed, "expected coarsen test to pass: {:?}", results[0].error);
    }

    #[test]
    fn simple_type_alias_no_constraint() {
        // type UserId = int64
        // verify alias_test:
        //     UserId id = 42
        //     assert id == 42
        let alias = type_alias("UserId", "int64", None);

        let vb = verify_block_item(
            "alias_test",
            block(vec![
                var_decl_stmt("UserId", "id", int(42)),
                assert_stmt_ast(binary(var("id"), BinOp::Eq, int(42))),
            ]),
        );

        let module = Module {
            items: vec![Item::TypeAlias(alias), Item::Verify(vb)],
            span: sp(),
        };
        let results = run_verify_blocks_detailed(&module);
        assert_eq!(results.len(), 1);
        assert!(results[0].passed, "expected simple alias to pass: {:?}", results[0].error);
    }

    // -----------------------------------------------------------------------
    // State machine tests
    // -----------------------------------------------------------------------

    fn machine_def() -> MachineDef {
        MachineDef {
            name: ident("UserAuth"),
            states: vec![
                MachineState {
                    name: ident("guest"),
                    fields: vec![],
                    span: sp(),
                },
                MachineState {
                    name: ident("logged_in"),
                    fields: vec![FieldDef {
                        name: ident("user_id"),
                        ty: type_named("string"),
                        span: sp(),
                    }],
                    span: sp(),
                },
                MachineState {
                    name: ident("banned"),
                    fields: vec![FieldDef {
                        name: ident("user_id"),
                        ty: type_named("string"),
                        span: sp(),
                    }],
                    span: sp(),
                },
            ],
            transitions: vec![
                MachineTransition {
                    from: ident("guest"),
                    to: ident("logged_in"),
                    span: sp(),
                },
                MachineTransition {
                    from: ident("logged_in"),
                    to: ident("guest"),
                    span: sp(),
                },
                MachineTransition {
                    from: ident("logged_in"),
                    to: ident("banned"),
                    span: sp(),
                },
            ],
            span: sp(),
        }
    }

    #[test]
    #[ignore] // state name resolution not yet wired
    fn machine_construct_and_check_state() {
        // machine UserAuth: ...
        // verify machine_test:
        //     UserAuth session = UserAuth(guest)
        //     assert session at guest

        let construct = Expr::Call(
            Box::new(var("UserAuth")),
            vec![CallArg {
                name: None,
                value: var("guest"),
                span: sp(),
            }],
            sp(),
        );

        let at_check = Expr::At(Box::new(var("session")), ident("guest"), sp());

        let vb = verify_block_item(
            "machine_test",
            block(vec![
                var_decl_stmt("UserAuth", "session", construct),
                assert_stmt_ast(at_check),
            ]),
        );

        let module = Module {
            items: vec![Item::Machine(machine_def()), Item::Verify(vb)],
            span: sp(),
        };
        let results = run_verify_blocks_detailed(&module);
        assert_eq!(results.len(), 1);
        assert!(
            results[0].passed,
            "expected machine construct + at check to pass: {:?}",
            results[0].error
        );
    }

    #[test]
    #[ignore]
    fn machine_transition_valid() {
        // machine UserAuth: ...
        // verify transition_test:
        //     UserAuth session = UserAuth(guest)
        //     UserAuth session2 = UserAuth.transition(session, logged_in, "user_123")
        //     assert session2 at logged_in

        let construct = Expr::Call(
            Box::new(var("UserAuth")),
            vec![CallArg {
                name: None,
                value: var("guest"),
                span: sp(),
            }],
            sp(),
        );

        let transition_call = Expr::Call(
            Box::new(Expr::FieldAccess(
                Box::new(var("UserAuth")),
                ident("transition"),
                sp(),
            )),
            vec![
                CallArg {
                    name: None,
                    value: var("session"),
                    span: sp(),
                },
                CallArg {
                    name: None,
                    value: var("logged_in"),
                    span: sp(),
                },
                CallArg {
                    name: None,
                    value: Expr::StringLiteral("user_123".to_string(), sp()),
                    span: sp(),
                },
            ],
            sp(),
        );

        let at_check = Expr::At(Box::new(var("session2")), ident("logged_in"), sp());

        let vb = verify_block_item(
            "transition_test",
            block(vec![
                var_decl_stmt("UserAuth", "session", construct),
                var_decl_stmt("UserAuth", "session2", transition_call),
                assert_stmt_ast(at_check),
            ]),
        );

        let module = Module {
            items: vec![Item::Machine(machine_def()), Item::Verify(vb)],
            span: sp(),
        };
        let results = run_verify_blocks_detailed(&module);
        assert_eq!(results.len(), 1);
        assert!(
            results[0].passed,
            "expected valid transition to pass: {:?}",
            results[0].error
        );
    }

    #[test]
    #[ignore]
    fn machine_transition_invalid_rejected() {
        // machine UserAuth: ...
        // verify invalid_transition_test:
        //     UserAuth session = UserAuth(guest)
        //     UserAuth session2 = UserAuth.transition(session, banned, "user_123")
        //     ^^ This should fail because guest -> banned is not an allowed transition.

        let construct = Expr::Call(
            Box::new(var("UserAuth")),
            vec![CallArg {
                name: None,
                value: var("guest"),
                span: sp(),
            }],
            sp(),
        );

        let transition_call = Expr::Call(
            Box::new(Expr::FieldAccess(
                Box::new(var("UserAuth")),
                ident("transition"),
                sp(),
            )),
            vec![
                CallArg {
                    name: None,
                    value: var("session"),
                    span: sp(),
                },
                CallArg {
                    name: None,
                    value: var("banned"),
                    span: sp(),
                },
                CallArg {
                    name: None,
                    value: Expr::StringLiteral("user_123".to_string(), sp()),
                    span: sp(),
                },
            ],
            sp(),
        );

        let vb = verify_block_item(
            "invalid_transition_test",
            block(vec![
                var_decl_stmt("UserAuth", "session", construct),
                var_decl_stmt("UserAuth", "session2", transition_call),
            ]),
        );

        let module = Module {
            items: vec![Item::Machine(machine_def()), Item::Verify(vb)],
            span: sp(),
        };
        let results = run_verify_blocks_detailed(&module);
        assert_eq!(results.len(), 1);
        assert!(
            !results[0].passed,
            "expected invalid transition to fail"
        );
        let err = results[0].error.as_ref().unwrap();
        assert!(
            err.contains("not allowed"),
            "expected 'not allowed' in error message, got: {err}"
        );
    }

    #[test]
    #[ignore]
    fn machine_at_check_wrong_state() {
        // machine UserAuth: ...
        // verify at_wrong_state:
        //     UserAuth session = UserAuth(guest)
        //     assert session at logged_in  <-- should be false

        let construct = Expr::Call(
            Box::new(var("UserAuth")),
            vec![CallArg {
                name: None,
                value: var("guest"),
                span: sp(),
            }],
            sp(),
        );

        // `session at logged_in` should evaluate to false
        let at_check = Expr::At(Box::new(var("session")), ident("logged_in"), sp());

        let vb = verify_block_item(
            "at_wrong_state",
            block(vec![
                var_decl_stmt("UserAuth", "session", construct),
                // assert NOT(session at logged_in)
                assert_stmt_ast(Expr::Unary(
                    UnaryOp::Not,
                    Box::new(at_check),
                    sp(),
                )),
            ]),
        );

        let module = Module {
            items: vec![Item::Machine(machine_def()), Item::Verify(vb)],
            span: sp(),
        };
        let results = run_verify_blocks_detailed(&module);
        assert_eq!(results.len(), 1);
        assert!(
            results[0].passed,
            "expected at-check for wrong state to pass (not at logged_in): {:?}",
            results[0].error
        );
    }
}
