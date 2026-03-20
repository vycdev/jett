use std::collections::HashMap;

use jett_parser::ast::{BinOp, Block, Expr, FunctionDef, Stmt, UnaryOp};

use crate::value::Value;

// ---------------------------------------------------------------------------
// Environment
// ---------------------------------------------------------------------------

/// A single lexical scope mapping variable names to values.
pub type Environment = HashMap<String, Value>;

// ---------------------------------------------------------------------------
// Control-flow signals
// ---------------------------------------------------------------------------

/// Internal signal used to propagate `return` and loop control flow through
/// the recursive interpreter.
#[derive(Debug)]
enum Signal {
    Return(Value),
    Break,
    Continue,
}

// ---------------------------------------------------------------------------
// Interpreter
// ---------------------------------------------------------------------------

/// A tree-walking interpreter that evaluates Jett AST nodes at compile time.
///
/// The interpreter maintains a stack of environments (scopes) and a registry
/// of user-defined functions.  It is intentionally simple: no heap, no GC,
/// no closures — just enough to execute `verify` blocks and `comptime`
/// expressions.
pub struct Interpreter {
    /// Stack of lexical scopes. The last element is the innermost scope.
    scopes: Vec<Environment>,
    /// User-defined functions available for calling.
    functions: HashMap<String, FunctionDef>,
}

impl Interpreter {
    /// Create a new interpreter with an empty global scope.
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            functions: HashMap::new(),
        }
    }

    // -- Scope management ---------------------------------------------------

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn set_variable(&mut self, name: &str, value: Value) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), value);
        }
    }

    fn get_variable(&self, name: &str) -> Option<&Value> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(name) {
                return Some(v);
            }
        }
        None
    }

    /// Reassign an existing variable in the nearest enclosing scope that
    /// contains it.  Returns `Err` if the variable was never declared.
    fn assign_variable(&mut self, name: &str, value: Value) -> Result<(), String> {
        for scope in self.scopes.iter_mut().rev() {
            if scope.contains_key(name) {
                scope.insert(name.to_string(), value);
                return Ok(());
            }
        }
        Err(format!("undefined variable '{name}'"))
    }

    // -- Public helpers -----------------------------------------------------

    /// Register a function definition so it can be called later.
    pub fn register_function(&mut self, func: &FunctionDef) {
        self.functions
            .insert(func.name.name.clone(), func.clone());
    }

    /// Return an immutable reference to the current (flat) environment.
    /// Useful for `eval_assert` which needs to inspect the environment.
    pub fn current_env(&self) -> &Environment {
        self.scopes.last().unwrap()
    }

    // -- Expression evaluation ----------------------------------------------

    /// Evaluate an expression, returning its [`Value`].
    pub fn eval_expr(&mut self, expr: &Expr) -> Result<Value, String> {
        match expr {
            // Literals
            Expr::IntLiteral(n, _) => Ok(Value::Int64(*n)),
            Expr::FloatLiteral(n, _) => Ok(Value::Float64(*n)),
            Expr::StringLiteral(s, _) => Ok(Value::String(s.clone())),
            Expr::BoolLiteral(b, _) => Ok(Value::Bool(*b)),
            Expr::Nothing(_) => Ok(Value::Nothing),

            // Variables
            Expr::Ident(ident) => self
                .get_variable(&ident.name)
                .cloned()
                .ok_or_else(|| format!("undefined variable '{}'", ident.name)),

            // Parenthesized
            Expr::Paren(inner, _) => self.eval_expr(inner),

            // Binary operations
            Expr::Binary(lhs, op, rhs, _) => {
                let left = self.eval_expr(lhs)?;
                // Short-circuit for logical operators
                match op {
                    BinOp::And => {
                        if let Value::Bool(false) = left {
                            return Ok(Value::Bool(false));
                        }
                        let right = self.eval_expr(rhs)?;
                        return eval_binary_op(&left, *op, &right);
                    }
                    BinOp::Or => {
                        if let Value::Bool(true) = left {
                            return Ok(Value::Bool(true));
                        }
                        let right = self.eval_expr(rhs)?;
                        return eval_binary_op(&left, *op, &right);
                    }
                    _ => {}
                }
                let right = self.eval_expr(rhs)?;
                eval_binary_op(&left, *op, &right)
            }

            // Unary operations
            Expr::Unary(op, operand, _) => {
                let val = self.eval_expr(operand)?;
                match op {
                    UnaryOp::Not => match val {
                        Value::Bool(b) => Ok(Value::Bool(!b)),
                        _ => Err("'not' requires a boolean operand".to_string()),
                    },
                    UnaryOp::Neg => match val {
                        Value::Int64(n) => Ok(Value::Int64(-n)),
                        Value::Float64(n) => Ok(Value::Float64(-n)),
                        _ => Err("unary '-' requires a numeric operand".to_string()),
                    },
                }
            }

            // Function / method calls
            Expr::Call(callee, args, _) => {
                let arg_values: Vec<Value> = args
                    .iter()
                    .map(|a| self.eval_expr(&a.value))
                    .collect::<Result<_, _>>()?;

                match callee.as_ref() {
                    Expr::Ident(ident) => self.call_function(&ident.name, arg_values),
                    _ => Err("only named function calls are supported in comptime".to_string()),
                }
            }

            // List construction
            Expr::ListConstruct(elems, _) => {
                let vals: Vec<Value> = elems
                    .iter()
                    .map(|e| self.eval_expr(e))
                    .collect::<Result<_, _>>()?;
                Ok(Value::List(vals))
            }

            // Unsupported expressions produce a clear error.
            _ => Err(format!(
                "unsupported expression in comptime: {:?}",
                std::mem::discriminant(expr)
            )),
        }
    }

    // -- Statement execution ------------------------------------------------

    /// Execute a single statement.  Returns `Ok(None)` for normal flow, or
    /// a [`Signal`] if control flow must be altered.
    fn exec_stmt_inner(&mut self, stmt: &Stmt) -> Result<Option<Signal>, String> {
        match stmt {
            Stmt::VarDecl(decl) => {
                let val = self.eval_expr(&decl.value)?;
                self.set_variable(&decl.name.name, val);
                Ok(None)
            }

            Stmt::Assign(assign) => {
                let val = self.eval_expr(&assign.value)?;
                match &assign.target {
                    Expr::Ident(ident) => {
                        self.assign_variable(&ident.name, val)?;
                    }
                    _ => return Err("only simple variable assignment is supported in comptime".to_string()),
                }
                Ok(None)
            }

            Stmt::Return(ret) => {
                let val = match &ret.value {
                    Some(expr) => self.eval_expr(expr)?,
                    None => Value::Nothing,
                };
                Ok(Some(Signal::Return(val)))
            }

            Stmt::If(if_stmt) => {
                let cond = self.eval_expr(&if_stmt.condition)?;
                if is_truthy(&cond)? {
                    return self.exec_block_inner(&if_stmt.then_block);
                }
                for (else_if_cond, else_if_block) in &if_stmt.else_ifs {
                    let val = self.eval_expr(else_if_cond)?;
                    if is_truthy(&val)? {
                        return self.exec_block_inner(else_if_block);
                    }
                }
                if let Some(else_block) = &if_stmt.else_block {
                    return self.exec_block_inner(else_block);
                }
                Ok(None)
            }

            Stmt::For(for_stmt) => {
                let iterable = self.eval_expr(&for_stmt.iterable)?;
                match iterable {
                    Value::List(items) => {
                        for item in items {
                            self.push_scope();
                            self.set_variable(&for_stmt.variable.name, item);
                            let signal = self.exec_block_inner(&for_stmt.body)?;
                            self.pop_scope();
                            match signal {
                                Some(Signal::Break) => break,
                                Some(Signal::Continue) => continue,
                                Some(Signal::Return(v)) => return Ok(Some(Signal::Return(v))),
                                None => {}
                            }
                        }
                    }
                    _ => return Err("for loop requires a list value in comptime".to_string()),
                }
                Ok(None)
            }

            Stmt::While(while_stmt) => {
                loop {
                    let cond = self.eval_expr(&while_stmt.condition)?;
                    if !is_truthy(&cond)? {
                        break;
                    }
                    self.push_scope();
                    let signal = self.exec_block_inner(&while_stmt.body)?;
                    self.pop_scope();
                    match signal {
                        Some(Signal::Break) => break,
                        Some(Signal::Continue) => continue,
                        Some(Signal::Return(v)) => return Ok(Some(Signal::Return(v))),
                        None => {}
                    }
                }
                Ok(None)
            }

            Stmt::Expr(expr_stmt) => {
                self.eval_expr(&expr_stmt.expr)?;
                Ok(None)
            }

            Stmt::Assert(assert_stmt) => {
                let cond = self.eval_expr(&assert_stmt.condition)?;
                match cond {
                    Value::Bool(true) => Ok(None),
                    Value::Bool(false) => {
                        let msg = if let Some(msg_expr) = &assert_stmt.message {
                            match self.eval_expr(msg_expr)? {
                                Value::String(s) => s,
                                other => other.to_string(),
                            }
                        } else {
                            "assertion failed".to_string()
                        };
                        Err(msg)
                    }
                    _ => Err("assert condition must be a boolean".to_string()),
                }
            }

            Stmt::Break(_) => Ok(Some(Signal::Break)),
            Stmt::Continue(_) => Ok(Some(Signal::Continue)),

            Stmt::Use(_) => {
                // use declarations are a no-op during comptime evaluation
                Ok(None)
            }
        }
    }

    /// Execute a single statement.  Converts the internal signal into a
    /// public-facing result.
    pub fn exec_stmt(&mut self, stmt: &Stmt) -> Result<(), String> {
        match self.exec_stmt_inner(stmt)? {
            None | Some(Signal::Break) | Some(Signal::Continue) => Ok(()),
            Some(Signal::Return(_)) => Ok(()),
        }
    }

    // -- Block execution ----------------------------------------------------

    /// Execute a block (list of statements), propagating control-flow signals.
    fn exec_block_inner(&mut self, block: &Block) -> Result<Option<Signal>, String> {
        self.push_scope();
        let mut result = None;
        for stmt in &block.stmts {
            if let Some(signal) = self.exec_stmt_inner(stmt)? {
                result = Some(signal);
                break;
            }
        }
        self.pop_scope();
        Ok(result)
    }

    /// Execute a block, returning the value produced by a `return` statement
    /// (if any).
    pub fn exec_block(&mut self, block: &Block) -> Result<Option<Value>, String> {
        match self.exec_block_inner(block)? {
            Some(Signal::Return(v)) => Ok(Some(v)),
            _ => Ok(None),
        }
    }

    // -- Function calls -----------------------------------------------------

    /// Call a registered function by name with the given arguments.
    pub fn call_function(&mut self, name: &str, args: Vec<Value>) -> Result<Value, String> {
        // Look up the function definition.
        let func = self
            .functions
            .get(name)
            .ok_or_else(|| format!("undefined function '{name}'"))?
            .clone();

        if args.len() != func.params.len() {
            return Err(format!(
                "function '{}' expects {} argument(s), got {}",
                name,
                func.params.len(),
                args.len()
            ));
        }

        // Create a new scope and bind parameters.
        self.push_scope();
        for (param, arg) in func.params.iter().zip(args) {
            self.set_variable(&param.name.name, arg);
        }

        let result = self.exec_block_inner(&func.body)?;
        self.pop_scope();

        match result {
            Some(Signal::Return(v)) => Ok(v),
            _ => Ok(Value::Nothing),
        }
    }
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn is_truthy(val: &Value) -> Result<bool, String> {
    match val {
        Value::Bool(b) => Ok(*b),
        _ => Err(format!("expected boolean, got {val}")),
    }
}

fn eval_binary_op(left: &Value, op: BinOp, right: &Value) -> Result<Value, String> {
    match (left, op, right) {
        // -- Integer arithmetic -----------------------------------------------
        (Value::Int64(a), BinOp::Add, Value::Int64(b)) => Ok(Value::Int64(a + b)),
        (Value::Int64(a), BinOp::Sub, Value::Int64(b)) => Ok(Value::Int64(a - b)),
        (Value::Int64(a), BinOp::Mul, Value::Int64(b)) => Ok(Value::Int64(a * b)),
        (Value::Int64(a), BinOp::Div, Value::Int64(b)) => {
            if *b == 0 {
                Err("division by zero".to_string())
            } else {
                Ok(Value::Int64(a / b))
            }
        }
        (Value::Int64(a), BinOp::Modulo, Value::Int64(b)) => {
            if *b == 0 {
                Err("modulo by zero".to_string())
            } else {
                Ok(Value::Int64(a % b))
            }
        }

        // -- Float arithmetic ------------------------------------------------
        (Value::Float64(a), BinOp::Add, Value::Float64(b)) => Ok(Value::Float64(a + b)),
        (Value::Float64(a), BinOp::Sub, Value::Float64(b)) => Ok(Value::Float64(a - b)),
        (Value::Float64(a), BinOp::Mul, Value::Float64(b)) => Ok(Value::Float64(a * b)),
        (Value::Float64(a), BinOp::Div, Value::Float64(b)) => Ok(Value::Float64(a / b)),
        (Value::Float64(a), BinOp::Modulo, Value::Float64(b)) => Ok(Value::Float64(a % b)),

        // -- String concatenation --------------------------------------------
        (Value::String(a), BinOp::Add, Value::String(b)) => {
            Ok(Value::String(format!("{a}{b}")))
        }

        // -- Integer comparisons ---------------------------------------------
        (Value::Int64(a), BinOp::Eq, Value::Int64(b)) => Ok(Value::Bool(a == b)),
        (Value::Int64(a), BinOp::NotEq, Value::Int64(b)) => Ok(Value::Bool(a != b)),
        (Value::Int64(a), BinOp::Lt, Value::Int64(b)) => Ok(Value::Bool(a < b)),
        (Value::Int64(a), BinOp::Gt, Value::Int64(b)) => Ok(Value::Bool(a > b)),
        (Value::Int64(a), BinOp::LtEq, Value::Int64(b)) => Ok(Value::Bool(a <= b)),
        (Value::Int64(a), BinOp::GtEq, Value::Int64(b)) => Ok(Value::Bool(a >= b)),

        // -- Float comparisons -----------------------------------------------
        (Value::Float64(a), BinOp::Eq, Value::Float64(b)) => Ok(Value::Bool(a == b)),
        (Value::Float64(a), BinOp::NotEq, Value::Float64(b)) => Ok(Value::Bool(a != b)),
        (Value::Float64(a), BinOp::Lt, Value::Float64(b)) => Ok(Value::Bool(a < b)),
        (Value::Float64(a), BinOp::Gt, Value::Float64(b)) => Ok(Value::Bool(a > b)),
        (Value::Float64(a), BinOp::LtEq, Value::Float64(b)) => Ok(Value::Bool(a <= b)),
        (Value::Float64(a), BinOp::GtEq, Value::Float64(b)) => Ok(Value::Bool(a >= b)),

        // -- String comparisons ----------------------------------------------
        (Value::String(a), BinOp::Eq, Value::String(b)) => Ok(Value::Bool(a == b)),
        (Value::String(a), BinOp::NotEq, Value::String(b)) => Ok(Value::Bool(a != b)),

        // -- Boolean comparisons ---------------------------------------------
        (Value::Bool(a), BinOp::Eq, Value::Bool(b)) => Ok(Value::Bool(a == b)),
        (Value::Bool(a), BinOp::NotEq, Value::Bool(b)) => Ok(Value::Bool(a != b)),

        // -- Boolean logic ---------------------------------------------------
        (Value::Bool(a), BinOp::And, Value::Bool(b)) => Ok(Value::Bool(*a && *b)),
        (Value::Bool(a), BinOp::Or, Value::Bool(b)) => Ok(Value::Bool(*a || *b)),

        _ => Err(format!(
            "unsupported binary operation: {left} {op:?} {right}"
        )),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use jett_common::{FileId, Span};
    use jett_parser::ast::*;

    use super::*;

    /// Helper: create a dummy span for test AST nodes.
    fn sp() -> Span {
        Span::new(FileId::new(0), 0, 0)
    }

    /// Helper: create an Ident.
    fn ident(name: &str) -> Ident {
        Ident {
            name: name.to_string(),
            span: sp(),
        }
    }

    /// Helper: create an int literal expression.
    fn int(n: i64) -> Expr {
        Expr::IntLiteral(n, sp())
    }

    /// Helper: create a float literal expression.
    fn float(n: f64) -> Expr {
        Expr::FloatLiteral(n, sp())
    }

    /// Helper: create a string literal expression.
    fn string(s: &str) -> Expr {
        Expr::StringLiteral(s.to_string(), sp())
    }

    /// Helper: create a bool literal expression.
    fn bool_expr(b: bool) -> Expr {
        Expr::BoolLiteral(b, sp())
    }

    /// Helper: create an identifier expression.
    fn var(name: &str) -> Expr {
        Expr::Ident(ident(name))
    }

    /// Helper: create a binary expression.
    fn binary(lhs: Expr, op: BinOp, rhs: Expr) -> Expr {
        Expr::Binary(Box::new(lhs), op, Box::new(rhs), sp())
    }

    /// Helper: create a simple type expression.
    fn type_named(name: &str) -> TypeExpr {
        TypeExpr::Named(ident(name))
    }

    /// Helper: create a variable declaration statement.
    fn var_decl(name: &str, value: Expr) -> Stmt {
        Stmt::VarDecl(VarDecl {
            mutable: true,
            ty: type_named("int64"),
            name: ident(name),
            value,
            span: sp(),
        })
    }

    /// Helper: create an assignment statement.
    fn assign(name: &str, value: Expr) -> Stmt {
        Stmt::Assign(AssignStmt {
            target: var(name),
            value,
            span: sp(),
        })
    }

    /// Helper: create an assert statement.
    fn assert_stmt(condition: Expr, message: Option<Expr>) -> Stmt {
        Stmt::Assert(AssertStmt {
            condition,
            message,
            span: sp(),
        })
    }

    /// Helper: create a block from statements.
    fn block(stmts: Vec<Stmt>) -> Block {
        Block {
            stmts,
            span: sp(),
        }
    }

    /// Helper: create a return statement.
    fn return_stmt(value: Expr) -> Stmt {
        Stmt::Return(ReturnStmt {
            value: Some(value),
            span: sp(),
        })
    }

    /// Helper: create a function call expression.
    fn call(name: &str, args: Vec<Expr>) -> Expr {
        Expr::Call(
            Box::new(var(name)),
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

    /// Helper: create a FunctionDef.
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

    // -----------------------------------------------------------------------
    // Arithmetic
    // -----------------------------------------------------------------------

    #[test]
    fn eval_integer_addition() {
        let mut interp = Interpreter::new();
        let expr = binary(int(2), BinOp::Add, int(3));
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Int64(5));
    }

    #[test]
    fn eval_integer_subtraction() {
        let mut interp = Interpreter::new();
        let expr = binary(int(10), BinOp::Sub, int(4));
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Int64(6));
    }

    #[test]
    fn eval_integer_multiplication() {
        let mut interp = Interpreter::new();
        let expr = binary(int(3), BinOp::Mul, int(7));
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Int64(21));
    }

    #[test]
    fn eval_integer_division() {
        let mut interp = Interpreter::new();
        let expr = binary(int(15), BinOp::Div, int(3));
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Int64(5));
    }

    #[test]
    fn eval_integer_modulo() {
        let mut interp = Interpreter::new();
        let expr = binary(int(17), BinOp::Modulo, int(5));
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Int64(2));
    }

    #[test]
    fn eval_float_addition() {
        let mut interp = Interpreter::new();
        let expr = binary(float(1.5), BinOp::Add, float(2.5));
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Float64(4.0));
    }

    #[test]
    fn eval_complex_arithmetic() {
        // (2 + 3) * 4 == 20
        let mut interp = Interpreter::new();
        let sum = binary(int(2), BinOp::Add, int(3));
        let product = binary(sum, BinOp::Mul, int(4));
        assert_eq!(interp.eval_expr(&product).unwrap(), Value::Int64(20));
    }

    #[test]
    fn eval_division_by_zero() {
        let mut interp = Interpreter::new();
        let expr = binary(int(10), BinOp::Div, int(0));
        assert!(interp.eval_expr(&expr).is_err());
    }

    // -----------------------------------------------------------------------
    // Boolean logic
    // -----------------------------------------------------------------------

    #[test]
    fn eval_and_true_true() {
        let mut interp = Interpreter::new();
        let expr = binary(bool_expr(true), BinOp::And, bool_expr(true));
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Bool(true));
    }

    #[test]
    fn eval_and_true_false() {
        let mut interp = Interpreter::new();
        let expr = binary(bool_expr(true), BinOp::And, bool_expr(false));
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Bool(false));
    }

    #[test]
    fn eval_or_false_true() {
        let mut interp = Interpreter::new();
        let expr = binary(bool_expr(false), BinOp::Or, bool_expr(true));
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Bool(true));
    }

    #[test]
    fn eval_or_false_false() {
        let mut interp = Interpreter::new();
        let expr = binary(bool_expr(false), BinOp::Or, bool_expr(false));
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Bool(false));
    }

    #[test]
    fn eval_not_true() {
        let mut interp = Interpreter::new();
        let expr = Expr::Unary(UnaryOp::Not, Box::new(bool_expr(true)), sp());
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Bool(false));
    }

    #[test]
    fn eval_not_false() {
        let mut interp = Interpreter::new();
        let expr = Expr::Unary(UnaryOp::Not, Box::new(bool_expr(false)), sp());
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Bool(true));
    }

    #[test]
    fn eval_comparison_eq() {
        let mut interp = Interpreter::new();
        let expr = binary(int(5), BinOp::Eq, int(5));
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Bool(true));
    }

    #[test]
    fn eval_comparison_neq() {
        let mut interp = Interpreter::new();
        let expr = binary(int(5), BinOp::NotEq, int(3));
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Bool(true));
    }

    #[test]
    fn eval_comparison_lt() {
        let mut interp = Interpreter::new();
        let expr = binary(int(3), BinOp::Lt, int(5));
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Bool(true));
    }

    #[test]
    fn eval_comparison_gte() {
        let mut interp = Interpreter::new();
        let expr = binary(int(5), BinOp::GtEq, int(5));
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Bool(true));
    }

    // -----------------------------------------------------------------------
    // String literals
    // -----------------------------------------------------------------------

    #[test]
    fn eval_string_literal() {
        let mut interp = Interpreter::new();
        let expr = string("hello");
        assert_eq!(
            interp.eval_expr(&expr).unwrap(),
            Value::String("hello".to_string())
        );
    }

    #[test]
    fn eval_string_concatenation() {
        let mut interp = Interpreter::new();
        let expr = binary(string("hello "), BinOp::Add, string("world"));
        assert_eq!(
            interp.eval_expr(&expr).unwrap(),
            Value::String("hello world".to_string())
        );
    }

    // -----------------------------------------------------------------------
    // Variable declaration and lookup
    // -----------------------------------------------------------------------

    #[test]
    fn variable_declaration_and_lookup() {
        let mut interp = Interpreter::new();
        let decl = var_decl("x", int(42));
        interp.exec_stmt(&decl).unwrap();
        let result = interp.eval_expr(&var("x")).unwrap();
        assert_eq!(result, Value::Int64(42));
    }

    #[test]
    fn variable_assignment() {
        let mut interp = Interpreter::new();
        interp.exec_stmt(&var_decl("x", int(1))).unwrap();
        interp.exec_stmt(&assign("x", int(99))).unwrap();
        assert_eq!(interp.eval_expr(&var("x")).unwrap(), Value::Int64(99));
    }

    #[test]
    fn undefined_variable_error() {
        let mut interp = Interpreter::new();
        assert!(interp.eval_expr(&var("nonexistent")).is_err());
    }

    // -----------------------------------------------------------------------
    // If/else branching
    // -----------------------------------------------------------------------

    #[test]
    fn if_then_branch_taken() {
        let mut interp = Interpreter::new();
        interp.exec_stmt(&var_decl("result", int(0))).unwrap();
        let if_stmt = Stmt::If(IfStmt {
            condition: bool_expr(true),
            then_block: block(vec![assign("result", int(1))]),
            else_ifs: vec![],
            else_block: None,
            span: sp(),
        });
        interp.exec_stmt(&if_stmt).unwrap();
        assert_eq!(interp.eval_expr(&var("result")).unwrap(), Value::Int64(1));
    }

    #[test]
    fn if_else_branch_taken() {
        let mut interp = Interpreter::new();
        interp.exec_stmt(&var_decl("result", int(0))).unwrap();
        let if_stmt = Stmt::If(IfStmt {
            condition: bool_expr(false),
            then_block: block(vec![assign("result", int(1))]),
            else_ifs: vec![],
            else_block: Some(block(vec![assign("result", int(2))])),
            span: sp(),
        });
        interp.exec_stmt(&if_stmt).unwrap();
        assert_eq!(interp.eval_expr(&var("result")).unwrap(), Value::Int64(2));
    }

    #[test]
    fn if_else_if_branch_taken() {
        let mut interp = Interpreter::new();
        interp.exec_stmt(&var_decl("result", int(0))).unwrap();
        let if_stmt = Stmt::If(IfStmt {
            condition: bool_expr(false),
            then_block: block(vec![assign("result", int(1))]),
            else_ifs: vec![(bool_expr(true), block(vec![assign("result", int(3))]))],
            else_block: Some(block(vec![assign("result", int(2))])),
            span: sp(),
        });
        interp.exec_stmt(&if_stmt).unwrap();
        assert_eq!(interp.eval_expr(&var("result")).unwrap(), Value::Int64(3));
    }

    // -----------------------------------------------------------------------
    // For loop
    // -----------------------------------------------------------------------

    #[test]
    fn for_loop_over_list() {
        let mut interp = Interpreter::new();
        interp.exec_stmt(&var_decl("sum", int(0))).unwrap();

        let list = Expr::ListConstruct(vec![int(1), int(2), int(3)], sp());
        let for_stmt = Stmt::For(ForStmt {
            variable: ident("item"),
            view: false,
            iterable: list,
            body: block(vec![assign(
                "sum",
                binary(var("sum"), BinOp::Add, var("item")),
            )]),
            span: sp(),
        });
        interp.exec_stmt(&for_stmt).unwrap();
        assert_eq!(interp.eval_expr(&var("sum")).unwrap(), Value::Int64(6));
    }

    #[test]
    fn for_loop_with_break() {
        let mut interp = Interpreter::new();
        interp.exec_stmt(&var_decl("count", int(0))).unwrap();

        let list = Expr::ListConstruct(vec![int(1), int(2), int(3), int(4), int(5)], sp());
        let for_stmt = Stmt::For(ForStmt {
            variable: ident("item"),
            view: false,
            iterable: list,
            body: block(vec![
                // if item == 4: break
                Stmt::If(IfStmt {
                    condition: binary(var("item"), BinOp::Eq, int(4)),
                    then_block: block(vec![Stmt::Break(sp())]),
                    else_ifs: vec![],
                    else_block: None,
                    span: sp(),
                }),
                assign("count", binary(var("count"), BinOp::Add, int(1))),
            ]),
            span: sp(),
        });
        interp.exec_stmt(&for_stmt).unwrap();
        // Should have counted 1, 2, 3 (stopped before 4)
        assert_eq!(interp.eval_expr(&var("count")).unwrap(), Value::Int64(3));
    }

    // -----------------------------------------------------------------------
    // While loop
    // -----------------------------------------------------------------------

    #[test]
    fn while_loop_with_counter() {
        let mut interp = Interpreter::new();
        interp.exec_stmt(&var_decl("i", int(0))).unwrap();
        interp.exec_stmt(&var_decl("sum", int(0))).unwrap();

        // while i < 5: sum = sum + i; i = i + 1
        let while_stmt = Stmt::While(WhileStmt {
            condition: binary(var("i"), BinOp::Lt, int(5)),
            body: block(vec![
                assign("sum", binary(var("sum"), BinOp::Add, var("i"))),
                assign("i", binary(var("i"), BinOp::Add, int(1))),
            ]),
            span: sp(),
        });
        interp.exec_stmt(&while_stmt).unwrap();
        // sum = 0 + 1 + 2 + 3 + 4 = 10
        assert_eq!(interp.eval_expr(&var("sum")).unwrap(), Value::Int64(10));
    }

    // -----------------------------------------------------------------------
    // Function calls
    // -----------------------------------------------------------------------

    #[test]
    fn function_call_add() {
        let mut interp = Interpreter::new();

        // function add(a: int64, b: int64) returns int64:
        //     return a + b
        let add_fn = func_def(
            "add",
            vec![("a", "int64"), ("b", "int64")],
            block(vec![return_stmt(binary(var("a"), BinOp::Add, var("b")))]),
        );
        interp.register_function(&add_fn);

        let result = interp.eval_expr(&call("add", vec![int(3), int(4)])).unwrap();
        assert_eq!(result, Value::Int64(7));
    }

    #[test]
    fn function_call_no_return() {
        let mut interp = Interpreter::new();

        // function noop():
        //     pass (empty body)
        let noop_fn = func_def("noop", vec![], block(vec![]));
        interp.register_function(&noop_fn);

        let result = interp.eval_expr(&call("noop", vec![])).unwrap();
        assert_eq!(result, Value::Nothing);
    }

    #[test]
    fn function_wrong_arg_count() {
        let mut interp = Interpreter::new();
        let add_fn = func_def(
            "add",
            vec![("a", "int64"), ("b", "int64")],
            block(vec![return_stmt(binary(var("a"), BinOp::Add, var("b")))]),
        );
        interp.register_function(&add_fn);

        let result = interp.eval_expr(&call("add", vec![int(1)]));
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Nested function calls
    // -----------------------------------------------------------------------

    #[test]
    fn nested_function_calls() {
        let mut interp = Interpreter::new();

        // function double(x: int64) returns int64:
        //     return x * 2
        let double_fn = func_def(
            "double",
            vec![("x", "int64")],
            block(vec![return_stmt(binary(var("x"), BinOp::Mul, int(2)))]),
        );
        interp.register_function(&double_fn);

        // function add(a: int64, b: int64) returns int64:
        //     return a + b
        let add_fn = func_def(
            "add",
            vec![("a", "int64"), ("b", "int64")],
            block(vec![return_stmt(binary(var("a"), BinOp::Add, var("b")))]),
        );
        interp.register_function(&add_fn);

        // add(double(3), double(5)) == 16
        let expr = call("add", vec![call("double", vec![int(3)]), call("double", vec![int(5)])]);
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Int64(16));
    }

    #[test]
    fn recursive_function_call() {
        let mut interp = Interpreter::new();

        // function factorial(n: int64) returns int64:
        //     if n <= 1:
        //         return 1
        //     return n * factorial(n - 1)
        let factorial_fn = func_def(
            "factorial",
            vec![("n", "int64")],
            block(vec![
                Stmt::If(IfStmt {
                    condition: binary(var("n"), BinOp::LtEq, int(1)),
                    then_block: block(vec![return_stmt(int(1))]),
                    else_ifs: vec![],
                    else_block: None,
                    span: sp(),
                }),
                return_stmt(binary(
                    var("n"),
                    BinOp::Mul,
                    call("factorial", vec![binary(var("n"), BinOp::Sub, int(1))]),
                )),
            ]),
        );
        interp.register_function(&factorial_fn);

        let result = interp
            .eval_expr(&call("factorial", vec![int(5)]))
            .unwrap();
        assert_eq!(result, Value::Int64(120));
    }

    // -----------------------------------------------------------------------
    // Assert
    // -----------------------------------------------------------------------

    #[test]
    fn assert_passing() {
        let mut interp = Interpreter::new();
        // assert (2 + 3) == 5
        let stmt = assert_stmt(
            binary(binary(int(2), BinOp::Add, int(3)), BinOp::Eq, int(5)),
            None,
        );
        interp.exec_stmt(&stmt).unwrap(); // should not error
    }

    #[test]
    fn assert_failing() {
        let mut interp = Interpreter::new();
        // 2 + 2 = 4, which is not a boolean — type error
        let stmt = assert_stmt(
            binary(int(2), BinOp::Add, int(2)),
            None,
        );
        assert!(interp.exec_stmt(&stmt).is_err());
    }

    #[test]
    fn assert_failing_with_bool() {
        let mut interp = Interpreter::new();
        let stmt = assert_stmt(
            binary(int(1), BinOp::Eq, int(2)),
            Some(string("one should not equal two")),
        );
        let err = interp.exec_stmt(&stmt).unwrap_err();
        assert_eq!(err, "one should not equal two");
    }

    #[test]
    fn assert_passing_bool() {
        let mut interp = Interpreter::new();
        let stmt = assert_stmt(bool_expr(true), None);
        interp.exec_stmt(&stmt).unwrap();
    }

    // -----------------------------------------------------------------------
    // Block execution returns value from return stmt
    // -----------------------------------------------------------------------

    #[test]
    fn exec_block_with_return() {
        let mut interp = Interpreter::new();
        let b = block(vec![
            var_decl("x", int(10)),
            return_stmt(binary(var("x"), BinOp::Mul, int(2))),
        ]);
        let result = interp.exec_block(&b).unwrap();
        assert_eq!(result, Some(Value::Int64(20)));
    }

    #[test]
    fn exec_block_without_return() {
        let mut interp = Interpreter::new();
        let b = block(vec![var_decl("x", int(10))]);
        let result = interp.exec_block(&b).unwrap();
        assert_eq!(result, None);
    }
}
