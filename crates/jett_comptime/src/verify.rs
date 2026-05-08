use jett_common::Span;
use jett_diagnostics::Diagnostic;
use jett_parser::ast::{
    FunctionDef, GivenDecl, Item, Module, PropertyBlock, TypeExpr, VerifyBlock,
};

use crate::interpreter::Interpreter;
use crate::value::Value;

// ---------------------------------------------------------------------------
// Default iteration count for property-based testing
// ---------------------------------------------------------------------------

const PROPERTY_DEFAULT_ITERATIONS: usize = 100;
const SHRINK_MAX_STEPS: usize = 50;

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

/// Outcome of running a single verify or property block.
#[derive(Debug, Clone)]
pub struct VerifyResult {
    pub name: String,
    pub passed: bool,
    pub error: Option<String>,
    /// If this was a property block, how many iterations were run.
    pub iterations: Option<usize>,
    /// If this was a property block, whether it is a property (true) or verify (false).
    pub is_property: bool,
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
    interp.register_module(module);
    let mut legacy_verify_functions: Vec<FunctionDef> = Vec::new();
    let mut verify_blocks: Vec<VerifyBlock> = Vec::new();
    let mut property_blocks: Vec<PropertyBlock> = Vec::new();
    for item in &module.items {
        match item {
            Item::Function(func) => {
                if has_assert_stmts(func) && func.params.is_empty() && func.name.name != "main" {
                    legacy_verify_functions.push(func.clone());
                }
            }
            Item::Verify(vb) => {
                verify_blocks.push(vb.clone());
            }
            Item::Property(pb) => {
                property_blocks.push(pb.clone());
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
                    iterations: None,
                    is_property: false,
                });
            }
            Err(msg) => {
                results.push(VerifyResult {
                    name: vb.name.name.clone(),
                    passed: false,
                    error: Some(msg),
                    iterations: None,
                    is_property: false,
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
                    iterations: None,
                    is_property: false,
                });
            }
            Err(msg) => {
                results.push(VerifyResult {
                    name: func.name.name.clone(),
                    passed: false,
                    error: Some(msg),
                    iterations: None,
                    is_property: false,
                });
            }
        }
    }

    // Execute property blocks.
    for pb in &property_blocks {
        let result = run_property_block(&mut interp, pb);
        results.push(result);
    }

    results
}

/// Evaluate a single pure function at compile time with the given arguments.
pub fn eval_function(func: &FunctionDef, args: Vec<Value>) -> Result<Value, ComptimeError> {
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
// Property-based testing
// ---------------------------------------------------------------------------

/// Run a single property block for `PROPERTY_DEFAULT_ITERATIONS` iterations.
/// Each iteration generates random values for each `given` parameter, binds
/// them into the interpreter, and executes the body.
/// Generate candidate shrunk versions of a value (ordered from simplest to most complex).
fn shrink_value(value: &Value) -> Vec<Value> {
    match value {
        Value::Int64(n) => {
            let mut candidates = Vec::new();
            if *n != 0 {
                candidates.push(Value::Int64(0));
            }
            if *n > 1 {
                candidates.push(Value::Int64(n / 2));
                candidates.push(Value::Int64(n - 1));
            }
            if *n < -1 {
                candidates.push(Value::Int64(n / 2));
                candidates.push(Value::Int64(n + 1));
                candidates.push(Value::Int64(-n)); // try positive version
            }
            if *n == -1 {
                candidates.push(Value::Int64(1));
            }
            candidates
        }
        Value::Float64(f) => {
            let mut candidates = Vec::new();
            if *f != 0.0 {
                candidates.push(Value::Float64(0.0));
            }
            if f.abs() > 1.0 {
                candidates.push(Value::Float64(f / 2.0));
                candidates.push(Value::Float64(f.floor()));
            }
            if *f < 0.0 {
                candidates.push(Value::Float64(-f)); // try positive version
            }
            candidates
        }
        Value::String(s) if !s.is_empty() => {
            let mut candidates = Vec::new();
            candidates.push(Value::String(String::new()));
            let chars: Vec<char> = s.chars().collect();
            if chars.len() > 1 {
                // Remove first half
                candidates.push(Value::String(chars[chars.len() / 2..].iter().collect()));
                // Remove second half
                candidates.push(Value::String(chars[..chars.len() / 2].iter().collect()));
                // Remove last character
                candidates.push(Value::String(chars[..chars.len() - 1].iter().collect()));
            }
            candidates
        }
        Value::List(items) if !items.is_empty() => {
            let mut candidates = Vec::new();
            candidates.push(Value::List(vec![]));
            if items.len() > 1 {
                // First half
                candidates.push(Value::List(items[..items.len() / 2].to_vec()));
                // Second half
                candidates.push(Value::List(items[items.len() / 2..].to_vec()));
                // Remove last element
                candidates.push(Value::List(items[..items.len() - 1].to_vec()));
            }
            // Try shrinking individual elements
            for (i, item) in items.iter().enumerate() {
                for shrunk_item in shrink_value(item) {
                    let mut new_list = items.clone();
                    new_list[i] = shrunk_item;
                    candidates.push(Value::List(new_list));
                }
            }
            candidates
        }
        _ => vec![], // Bool, Nothing, etc. cannot be shrunk further
    }
}

/// Try to find simpler inputs that still cause the property to fail.
/// Returns the shrunk inputs as a Vec<Value> in the same order as `failing`.
fn shrink_inputs(interp: &mut Interpreter, pb: &PropertyBlock, failing: Vec<Value>) -> Vec<Value> {
    let mut current = failing;

    'outer: for _ in 0..SHRINK_MAX_STEPS {
        // Try to shrink each input one at a time.
        for i in 0..current.len() {
            let candidates = shrink_value(&current[i]);
            for candidate in candidates {
                let mut attempt = current.clone();
                attempt[i] = candidate;

                // Run the property with the candidate inputs.
                interp.push_scope_public();
                for (given, value) in pb.givens.iter().zip(attempt.iter()) {
                    interp.set_variable_public(&given.name.name, value.clone());
                }
                let result = interp.exec_block(&pb.body);
                interp.pop_scope_public();

                if result.is_err() {
                    // Still fails — use the simpler version.
                    current = attempt;
                    continue 'outer;
                }
            }
        }
        // No further shrinking possible.
        break;
    }

    current
}

fn run_property_block(interp: &mut Interpreter, pb: &PropertyBlock) -> VerifyResult {
    let iterations = PROPERTY_DEFAULT_ITERATIONS;

    // Pre-compute the value pools for each given declaration.
    let pools: Vec<Vec<Value>> = pb
        .givens
        .iter()
        .map(|g| generate_values_for_type(&g.ty))
        .collect();

    // If any pool is empty, we cannot test.
    for (i, pool) in pools.iter().enumerate() {
        if pool.is_empty() {
            return VerifyResult {
                name: pb.name.name.clone(),
                passed: false,
                error: Some(format!(
                    "unsupported type for property given '{}': cannot generate values",
                    pb.givens[i].name.name,
                )),
                iterations: Some(0),
                is_property: true,
            };
        }
    }

    for iteration in 0..iterations {
        // Pick values for this iteration: cycle through the pool.
        let chosen: Vec<(&GivenDecl, Value)> = pb
            .givens
            .iter()
            .zip(pools.iter())
            .map(|(given, pool)| {
                let idx = iteration % pool.len();
                (given, pool[idx].clone())
            })
            .collect();

        // Push a scope, bind the given values, execute the body.
        interp.push_scope_public();
        for (given, value) in &chosen {
            interp.set_variable_public(&given.name.name, value.clone());
        }

        let exec_result = interp.exec_block(&pb.body);
        interp.pop_scope_public();

        if let Err(msg) = exec_result {
            // Shrink the failing inputs to find a simpler counterexample.
            let failing_values: Vec<Value> = chosen.iter().map(|(_, v)| v.clone()).collect();
            let shrunk = shrink_inputs(interp, pb, failing_values);

            let input_desc: Vec<String> = pb
                .givens
                .iter()
                .zip(shrunk.iter())
                .map(|(given, value)| format!("{} = {}", given.name.name, value))
                .collect();
            return VerifyResult {
                name: pb.name.name.clone(),
                passed: false,
                error: Some(format!(
                    "{} (counterexample: {})",
                    msg,
                    input_desc.join(", ")
                )),
                iterations: Some(iteration + 1),
                is_property: true,
            };
        }
    }

    VerifyResult {
        name: pb.name.name.clone(),
        passed: true,
        error: None,
        iterations: Some(iterations),
        is_property: true,
    }
}

/// Generate a pool of test values for a given type expression.
fn generate_values_for_type(ty: &TypeExpr) -> Vec<Value> {
    match ty {
        TypeExpr::Named(ident) => match ident.name.as_str() {
            "int64" => vec![
                Value::Int64(0),
                Value::Int64(1),
                Value::Int64(-1),
                Value::Int64(42),
                Value::Int64(-42),
                Value::Int64(100),
                Value::Int64(i64::MAX),
                Value::Int64(i64::MIN),
            ],
            "string" => vec![
                Value::String(String::new()),
                Value::String("a".to_string()),
                Value::String("hello".to_string()),
                Value::String("hello world".to_string()),
                Value::String("123".to_string()),
            ],
            "bool" => vec![Value::Bool(true), Value::Bool(false)],
            "float64" => vec![
                Value::Float64(0.0),
                Value::Float64(1.0),
                Value::Float64(-1.0),
                Value::Float64(3.14),
                Value::Float64(-0.0),
            ],
            _ => vec![], // unsupported type
        },
        TypeExpr::Generic(ident, args, _) => {
            match ident.name.as_str() {
                "list" if args.len() == 1 => {
                    match &args[0] {
                        TypeExpr::Named(inner) if inner.name == "int64" => vec![
                            Value::List(vec![]),
                            Value::List(vec![Value::Int64(1)]),
                            Value::List(vec![Value::Int64(3), Value::Int64(1), Value::Int64(2)]),
                            Value::List(vec![Value::Int64(1), Value::Int64(1), Value::Int64(1)]),
                            Value::List(vec![Value::Int64(-5), Value::Int64(0), Value::Int64(5)]),
                        ],
                        _ => vec![], // unsupported inner type
                    }
                }
                _ => vec![], // unsupported generic type
            }
        }
        TypeExpr::View(inner, _) => generate_values_for_type(inner),
        TypeExpr::Function(_, _, _) => vec![], // cannot generate function values
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

    fn string(s: &str) -> Expr {
        Expr::StringLiteral(s.to_string(), sp())
    }

    fn type_named(name: &str) -> TypeExpr {
        TypeExpr::Named(ident(name))
    }

    fn block(stmts: Vec<Stmt>) -> Block {
        Block { stmts, span: sp() }
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
            type_params: vec![],
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

    fn field_access(base: Expr, field: &str) -> Expr {
        Expr::FieldAccess(Box::new(base), ident(field), sp())
    }

    fn named_arg(name: &str, value: Expr) -> CallArg {
        CallArg {
            name: Some(ident(name)),
            value,
            span: sp(),
        }
    }

    fn dotted_call(module: &str, func_name: &str, args: Vec<Expr>) -> Expr {
        Expr::Call(
            Box::new(field_access(var(module), func_name)),
            args.into_iter()
                .map(|value| CallArg {
                    name: None,
                    value,
                    span: sp(),
                })
                .collect(),
            sp(),
        )
    }

    fn struct_def(name: &str, fields: Vec<(&str, &str)>, methods: Vec<FunctionDef>) -> StructDef {
        StructDef {
            name: ident(name),
            type_params: vec![],
            fields: fields
                .into_iter()
                .map(|(field_name, field_ty)| FieldDef {
                    name: ident(field_name),
                    ty: type_named(field_ty),
                    serialize_name: None,
                    span: sp(),
                })
                .collect(),
            methods,
            span: sp(),
        }
    }

    fn interface_decl(
        name: &str,
        methods: Vec<(&str, Vec<(&str, &str, bool)>, &str)>,
    ) -> InterfaceDecl {
        InterfaceDecl {
            name: ident(name),
            methods: methods
                .into_iter()
                .map(|(method_name, params, return_type)| FunctionDecl {
                    name: ident(method_name),
                    type_params: vec![],
                    params: params
                        .into_iter()
                        .map(|(param_name, param_ty, view)| Param {
                            view,
                            mutable: false,
                            name: ident(param_name),
                            ty: type_named(param_ty),
                            span: sp(),
                        })
                        .collect(),
                    return_type: Some(type_named(return_type)),
                    span: sp(),
                })
                .collect(),
            span: sp(),
        }
    }

    fn implement_block(
        interface_name: &str,
        for_type: &str,
        methods: Vec<FunctionDef>,
    ) -> ImplementBlock {
        ImplementBlock {
            interface_name: ident(interface_name),
            for_type: type_named(for_type),
            methods,
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
            block(vec![assert_stmt_ast(
                binary(int(2), BinOp::Add, int(3)).pipe_eq(int(5)),
            )]),
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
            block(vec![assert_stmt_ast(binary(
                call_double,
                BinOp::Eq,
                int(10),
            ))]),
        );

        let module = Module {
            items: vec![Item::Function(double_fn), Item::Function(verify_fn)],
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
                CallArg {
                    name: None,
                    value: int(2),
                    span: sp(),
                },
                CallArg {
                    name: None,
                    value: int(3),
                    span: sp(),
                },
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
                CallArg {
                    name: None,
                    value: int(2),
                    span: sp(),
                },
                CallArg {
                    name: None,
                    value: int(3),
                    span: sp(),
                },
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

    #[test]
    fn verify_block_can_construct_and_call_struct_methods() {
        let mut total_method = func_def(
            "total",
            vec![("self", "Point")],
            block(vec![return_stmt(binary(
                field_access(var("self"), "x"),
                BinOp::Add,
                field_access(var("self"), "y"),
            ))]),
        );
        total_method.params[0].view = true;

        let point_struct = struct_def(
            "Point",
            vec![("x", "int64"), ("y", "int64")],
            vec![total_method],
        );

        let point_ctor = Expr::Call(
            Box::new(var("Point")),
            vec![named_arg("x", int(2)), named_arg("y", int(3))],
            sp(),
        );
        let point_total = dotted_call(
            "Point",
            "total",
            vec![Expr::View(Box::new(var("point")), sp())],
        );
        let vb = verify_block_item(
            "point_total",
            block(vec![
                var_decl_stmt("Point", "point", point_ctor),
                assert_stmt_ast(binary(field_access(var("point"), "x"), BinOp::Eq, int(2))),
                assert_stmt_ast(binary(point_total, BinOp::Eq, int(5))),
            ]),
        );

        let module = Module {
            items: vec![Item::Struct(point_struct), Item::Verify(vb)],
            span: sp(),
        };
        let results = run_verify_blocks_detailed(&module);
        assert_eq!(results.len(), 1);
        assert!(
            results[0].passed,
            "expected struct verify block to pass: {:?}",
            results[0].error
        );
        assert_eq!(results[0].name, "point_total");
    }

    #[test]
    fn verify_block_can_call_interface_methods() {
        let speaker = interface_decl(
            "Speaker",
            vec![("speak", vec![("self", "Speaker", true)], "string")],
        );
        let dog = struct_def("Dog", vec![("name", "string")], vec![]);

        let mut speak = func_def(
            "speak",
            vec![("self", "Dog")],
            block(vec![return_stmt(field_access(var("self"), "name"))]),
        );
        speak.params[0].view = true;
        let dog_speaker = implement_block("Speaker", "Dog", vec![speak]);

        let dog_ctor = Expr::Call(
            Box::new(var("Dog")),
            vec![named_arg("name", string("bark"))],
            sp(),
        );
        let speaker_call = dotted_call(
            "Speaker",
            "speak",
            vec![Expr::View(Box::new(var("dog")), sp())],
        );
        let vb = verify_block_item(
            "speaker_dispatch",
            block(vec![
                var_decl_stmt("Dog", "dog", dog_ctor),
                assert_stmt_ast(binary(speaker_call, BinOp::Eq, string("bark"))),
            ]),
        );

        let module = Module {
            items: vec![
                Item::Interface(speaker),
                Item::Struct(dog),
                Item::Implement(dog_speaker),
                Item::Verify(vb),
            ],
            span: sp(),
        };
        let results = run_verify_blocks_detailed(&module);
        assert_eq!(results.len(), 1);
        assert!(
            results[0].passed,
            "expected interface verify block to pass: {:?}",
            results[0].error
        );
        assert_eq!(results[0].name, "speaker_dispatch");
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
        assert!(
            results[0].passed,
            "expected verify block to pass: {:?}",
            results[0].error
        );
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

        let vb = verify_block_item("port_test", block(vec![var_decl_stmt("Port", "p", int(0))]));

        let module = Module {
            items: vec![Item::TypeAlias(port_alias), Item::Verify(vb)],
            span: sp(),
        };
        let results = run_verify_blocks_detailed(&module);
        assert_eq!(results.len(), 1);
        assert!(
            !results[0].passed,
            "expected verify block to fail for invalid port"
        );
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
        assert!(
            results[0].passed,
            "expected coarsen test to pass: {:?}",
            results[0].error
        );
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
        assert!(
            results[0].passed,
            "expected simple alias to pass: {:?}",
            results[0].error
        );
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
                        serialize_name: None,
                        span: sp(),
                    }],
                    span: sp(),
                },
                MachineState {
                    name: ident("banned"),
                    fields: vec![FieldDef {
                        name: ident("user_id"),
                        ty: type_named("string"),
                        serialize_name: None,
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
        assert!(!results[0].passed, "expected invalid transition to fail");
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
                assert_stmt_ast(Expr::Unary(UnaryOp::Not, Box::new(at_check), sp())),
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

    // -----------------------------------------------------------------------
    // Property block tests
    // -----------------------------------------------------------------------

    fn type_generic(name: &str, args: Vec<TypeExpr>) -> TypeExpr {
        TypeExpr::Generic(ident(name), args, sp())
    }

    fn property_block_item(name: &str, givens: Vec<GivenDecl>, body: Block) -> PropertyBlock {
        PropertyBlock {
            name: ident(name),
            givens,
            body,
            span: sp(),
        }
    }

    fn given_decl(name: &str, ty: TypeExpr) -> GivenDecl {
        GivenDecl {
            name: ident(name),
            ty,
            span: sp(),
        }
    }

    #[test]
    fn property_block_passing_int64() {
        // property int_identity:
        //     given x: int64
        //     assert x == x
        let pb = property_block_item(
            "int_identity",
            vec![given_decl("x", type_named("int64"))],
            block(vec![assert_stmt_ast(binary(var("x"), BinOp::Eq, var("x")))]),
        );

        let module = Module {
            items: vec![Item::Property(pb)],
            span: sp(),
        };
        let results = run_verify_blocks_detailed(&module);
        assert_eq!(results.len(), 1);
        assert!(
            results[0].passed,
            "expected property to pass: {:?}",
            results[0].error
        );
        assert!(results[0].is_property);
        assert_eq!(results[0].iterations, Some(100));
    }

    #[test]
    fn property_block_list_length_non_negative() {
        // function list_length(view items: list[int64]) returns int64:
        //     return list.length(items)
        //
        // property list_length_non_negative:
        //     given items: list[int64]
        //     int64 len = list_length(items)
        //     assert len >= 0

        let length_fn = func_def(
            "list_length",
            vec![("items", "list")],
            block(vec![return_stmt(Expr::Call(
                Box::new(Expr::FieldAccess(
                    Box::new(var("list")),
                    ident("length"),
                    sp(),
                )),
                vec![CallArg {
                    name: None,
                    value: var("items"),
                    span: sp(),
                }],
                sp(),
            ))]),
        );

        let pb = property_block_item(
            "list_length_non_negative",
            vec![given_decl(
                "items",
                type_generic("list", vec![type_named("int64")]),
            )],
            block(vec![
                var_decl_stmt(
                    "int64",
                    "len",
                    Expr::Call(
                        Box::new(var("list_length")),
                        vec![CallArg {
                            name: None,
                            value: var("items"),
                            span: sp(),
                        }],
                        sp(),
                    ),
                ),
                assert_stmt_ast(binary(var("len"), BinOp::GtEq, int(0))),
            ]),
        );

        let module = Module {
            items: vec![Item::Function(length_fn), Item::Property(pb)],
            span: sp(),
        };
        let results = run_verify_blocks_detailed(&module);
        assert_eq!(results.len(), 1);
        assert!(
            results[0].passed,
            "expected property to pass: {:?}",
            results[0].error
        );
        assert!(results[0].is_property);
    }

    #[test]
    fn property_block_failing_detects_bug() {
        // property all_ints_positive:
        //     given x: int64
        //     assert x > 0
        //
        // This should fail because the int64 pool includes 0, -1, -42, i64::MIN.
        let pb = property_block_item(
            "all_ints_positive",
            vec![given_decl("x", type_named("int64"))],
            block(vec![assert_stmt_ast(binary(var("x"), BinOp::Gt, int(0)))]),
        );

        let module = Module {
            items: vec![Item::Property(pb)],
            span: sp(),
        };
        let results = run_verify_blocks_detailed(&module);
        assert_eq!(results.len(), 1);
        assert!(!results[0].passed, "expected property to fail");
        assert!(results[0].is_property);
        let err = results[0].error.as_ref().unwrap();
        assert!(
            err.contains("counterexample:") || err.contains("input:"),
            "error should contain input values: {err}"
        );
    }

    #[test]
    #[ignore]
    fn property_block_with_function_call() {
        // function negate(x: int64) returns int64:
        //     return 0 - x
        //
        // property negate_inverts_sign:
        //     given x: int64
        //     int64 neg = negate(x)
        //     int64 neg_neg = negate(neg)
        //     assert neg_neg == x
        //
        // Note: negate(i64::MIN) overflows, which the property correctly catches.
        // We use a function that returns the same value to avoid overflow.
        let identity_fn = func_def(
            "identity",
            vec![("x", "int64")],
            block(vec![return_stmt(var("x"))]),
        );

        let pb = property_block_item(
            "identity_round_trip",
            vec![given_decl("x", type_named("int64"))],
            block(vec![
                var_decl_stmt(
                    "int64",
                    "y",
                    Expr::Call(
                        Box::new(var("identity")),
                        vec![CallArg {
                            name: None,
                            value: var("x"),
                            span: sp(),
                        }],
                        sp(),
                    ),
                ),
                assert_stmt_ast(binary(var("y"), BinOp::Eq, var("x"))),
            ]),
        );

        let module = Module {
            items: vec![Item::Function(identity_fn), Item::Property(pb)],
            span: sp(),
        };
        let results = run_verify_blocks_detailed(&module);
        assert_eq!(results.len(), 1);
        assert!(
            results[0].passed,
            "expected property to pass: {:?}",
            results[0].error
        );
        assert!(results[0].is_property);
        assert_eq!(results[0].iterations, Some(100));
    }

    #[test]
    fn property_and_verify_blocks_coexist() {
        // function is_non_negative(x: int64) returns bool:
        //     return x >= 0
        //
        // verify is_non_negative:
        //     assert is_non_negative(5) == true
        //
        // property bool_result:
        //     given x: int64
        //     bool result = is_non_negative(x)
        //     # result is always either true or false — trivially true
        //     assert result == true || result == false
        let is_nn_fn = FunctionDef {
            name: ident("is_non_negative"),
            type_params: vec![],
            params: vec![Param {
                view: false,
                mutable: false,
                name: ident("x"),
                ty: type_named("int64"),
                span: sp(),
            }],
            return_type: Some(type_named("bool")),
            body: block(vec![return_stmt(binary(var("x"), BinOp::GtEq, int(0)))]),
            span: sp(),
        };

        let call_nn = |arg: Expr| -> Expr {
            Expr::Call(
                Box::new(var("is_non_negative")),
                vec![CallArg {
                    name: None,
                    value: arg,
                    span: sp(),
                }],
                sp(),
            )
        };

        let vb = verify_block_item(
            "is_non_negative",
            block(vec![assert_stmt_ast(binary(
                call_nn(int(5)),
                BinOp::Eq,
                Expr::BoolLiteral(true, sp()),
            ))]),
        );

        let pb = property_block_item(
            "bool_result",
            vec![given_decl("x", type_named("int64"))],
            block(vec![
                Stmt::VarDecl(VarDecl {
                    mutable: false,
                    ty: type_named("bool"),
                    name: ident("result"),
                    value: call_nn(var("x")),
                    span: sp(),
                }),
                assert_stmt_ast(binary(
                    binary(var("result"), BinOp::Eq, Expr::BoolLiteral(true, sp())),
                    BinOp::Or,
                    binary(var("result"), BinOp::Eq, Expr::BoolLiteral(false, sp())),
                )),
            ]),
        );

        let module = Module {
            items: vec![
                Item::Function(is_nn_fn),
                Item::Verify(vb),
                Item::Property(pb),
            ],
            span: sp(),
        };
        let results = run_verify_blocks_detailed(&module);
        assert_eq!(results.len(), 2);

        // First result is the verify block
        assert!(!results[0].is_property);
        assert!(results[0].passed, "verify block should pass");

        // Second result is the property block
        assert!(results[1].is_property);
        assert!(
            results[1].passed,
            "property block should pass: {:?}",
            results[1].error
        );
        assert_eq!(results[1].iterations, Some(100));
    }
}
