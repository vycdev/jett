use std::collections::HashMap;

use jett_parser::ast::{
    BinOp, Block, CallArg, Expr, FunctionDef, Ident, ImplementBlock, InterfaceDecl, MachineDef,
    Pattern, PipelineStep, Stmt, StringPart, StructDef, TypeAlias, TypeExpr, UnaryOp,
};

use crate::value::Value;

// ---------------------------------------------------------------------------
// Built-in argument checking (must be defined before call_builtin uses it)
// ---------------------------------------------------------------------------

/// Check that a built-in function received the expected number of arguments.
/// Returns `Some(Err(...))` on mismatch (suitable for early-return from
/// `call_builtin` via `if let`), or `None` if the count is correct.
fn check_args(name: &str, expected: usize, args: &[Value]) -> Option<Result<Value, String>> {
    if args.len() != expected {
        Some(Err(format!(
            "{name} expects {expected} argument(s), got {}",
            args.len()
        )))
    } else {
        None
    }
}

/// Convenience macro: invoke `check_args` and, if the count is wrong,
/// immediately return the error wrapped in `Some`.
macro_rules! require_args {
    ($name:expr, $expected:expr, $args:expr) => {
        if let Some(err) = check_args($name, $expected, $args) {
            return Some(err);
        }
    };
}

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
    Default(Value),
    Break,
    Continue,
}

#[derive(Debug)]
enum ExprFlow {
    Value(Value),
    Signal(Signal),
}

macro_rules! value_or_signal {
    ($self:expr, $expr:expr) => {
        match $self.eval_expr_flow($expr)? {
            ExprFlow::Value(value) => value,
            ExprFlow::Signal(signal) => return Ok(ExprFlow::Signal(signal)),
        }
    };
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
/// A registered refinement type alias with its base type name and constraint.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct RefinementDef {
    base_type_name: String,
    constraint: Expr,
}

pub struct Interpreter {
    /// Stack of lexical scopes. The last element is the innermost scope.
    scopes: Vec<Environment>,
    /// User-defined functions available for calling.
    functions: HashMap<String, FunctionDef>,
    /// Registered user-defined structs available for construction and field access.
    structs: HashMap<String, StructDef>,
    /// Interface dotted name -> concrete runtime type -> concrete dotted function name.
    interface_methods: HashMap<String, HashMap<String, String>>,
    /// Registered type alias base names.
    type_alias_bases: HashMap<String, String>,
    /// Registered type aliases: name -> (base_type_name, optional constraint).
    type_aliases: HashMap<String, Option<RefinementDef>>,
    /// Registered state machine definitions: name -> MachineDef.
    machines: HashMap<String, MachineDef>,
}

impl Interpreter {
    /// Create a new interpreter with an empty global scope.
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            functions: HashMap::new(),
            structs: HashMap::new(),
            interface_methods: HashMap::new(),
            type_alias_bases: HashMap::new(),
            type_aliases: HashMap::new(),
            machines: HashMap::new(),
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

    // -- Public scope management (for property-based testing) ---------------

    /// Push a new scope (public wrapper for use by verify/property runners).
    pub fn push_scope_public(&mut self) {
        self.push_scope();
    }

    /// Pop the current scope (public wrapper for use by verify/property runners).
    pub fn pop_scope_public(&mut self) {
        self.pop_scope();
    }

    /// Set a variable in the current scope (public wrapper for use by
    /// verify/property runners).
    pub fn set_variable_public(&mut self, name: &str, value: Value) {
        self.set_variable(name, value);
    }

    // -- Public helpers -----------------------------------------------------

    /// Register a function definition so it can be called later.
    pub fn register_function(&mut self, func: &FunctionDef) {
        self.functions.insert(func.name.name.clone(), func.clone());
    }

    /// Register a user-defined struct so it can be constructed and its methods
    /// called with dotted syntax like `Point.total(view p)`.
    pub fn register_struct(&mut self, strukt: &StructDef) {
        self.structs
            .insert(strukt.name.name.clone(), strukt.clone());

        for method in &strukt.methods {
            self.functions.insert(
                format!("{}.{}", strukt.name.name, method.name.name),
                method.clone(),
            );
        }
    }

    /// Register an interface declaration. Interfaces carry no runtime state,
    /// but keeping the entry point makes module registration symmetric.
    pub fn register_interface(&mut self, _interface: &InterfaceDecl) {}

    /// Register an `implement Interface for Type` block so interface-qualified
    /// calls can dispatch to the concrete method body at runtime.
    pub fn register_implement_block(&mut self, block: &ImplementBlock) {
        let interface_name = block.interface_name.name.clone();
        let owner_name = type_expr_name(&block.for_type);

        for method in &block.methods {
            let concrete_name = format!("{}.{}", owner_name, method.name.name);
            let interface_method_name = format!("{}.{}", interface_name, method.name.name);

            self.functions.insert(concrete_name.clone(), method.clone());
            self.interface_methods
                .entry(interface_method_name)
                .or_default()
                .insert(owner_name.clone(), concrete_name);
        }
    }

    /// Register a state machine definition so it can be used for construction
    /// and transitions.
    pub fn register_machine(&mut self, machine: &MachineDef) {
        self.machines
            .insert(machine.name.name.clone(), machine.clone());
    }

    /// Register a type alias so the interpreter can validate refinement
    /// constraints when values are assigned to the type.
    pub fn register_type_alias(&mut self, alias: &TypeAlias) {
        let base_name = type_expr_name(&alias.base_type);
        self.type_alias_bases
            .insert(alias.name.name.clone(), base_name.clone());
        let def = alias.constraint.as_ref().map(|c| RefinementDef {
            base_type_name: base_name,
            constraint: c.clone(),
        });
        self.type_aliases.insert(alias.name.name.clone(), def);
    }

    /// Check a value against a refinement type's constraint.
    /// Returns `Ok(())` if valid, or `Err(message)` if the constraint fails.
    fn check_refinement(&mut self, type_name: &str, value: &Value) -> Result<(), String> {
        if let Some(base_type_name) = self.type_alias_bases.get(type_name).cloned() {
            self.check_refinement(&base_type_name, value)?;
        }

        let def = match self.type_aliases.get(type_name) {
            Some(Some(def)) => def.clone(),
            Some(None) => return Ok(()), // simple alias, no constraint
            None => return Ok(()),       // not a known type alias
        };

        self.push_scope();
        self.set_variable("value", value.clone());
        let result = self.eval_expr(&def.constraint);
        self.pop_scope();

        match result {
            Ok(Value::Bool(true)) => Ok(()),
            Ok(Value::Bool(false)) => Err(format!(
                "refinement type constraint failed for '{type_name}'"
            )),
            Ok(other) => Err(format!(
                "refinement constraint for '{type_name}' must return bool, got {other}"
            )),
            Err(e) => Err(format!(
                "error evaluating refinement constraint for '{type_name}': {e}"
            )),
        }
    }

    fn eval_refinement_boundary(
        &mut self,
        type_name: &str,
        target: &Expr,
        bind_name: Option<&Ident>,
        body: &Block,
    ) -> Result<ExprFlow, String> {
        let value = value_or_signal!(self, target);
        match self.check_refinement(type_name, &value) {
            Ok(()) => Ok(ExprFlow::Value(value)),
            Err(message) => self.exec_handle_block(bind_name, Some(Value::String(message)), body),
        }
    }

    fn type_name_has_refinement(&self, type_name: &str) -> bool {
        match self.type_aliases.get(type_name) {
            Some(Some(_)) => true,
            Some(None) => self
                .type_alias_bases
                .get(type_name)
                .is_some_and(|base| self.type_name_has_refinement(base)),
            None => false,
        }
    }

    /// Return an immutable reference to the current (flat) environment.
    /// Useful for `eval_assert` which needs to inspect the environment.
    pub fn current_env(&self) -> &Environment {
        self.scopes.last().unwrap()
    }

    // -- Expression evaluation ----------------------------------------------

    /// Evaluate an expression, returning its [`Value`].
    pub fn eval_expr(&mut self, expr: &Expr) -> Result<Value, String> {
        match self.eval_expr_flow(expr)? {
            ExprFlow::Value(value) => Ok(value),
            ExprFlow::Signal(Signal::Default(_)) => {
                Err("`default` can only be used inside a `handle` block".to_string())
            }
            ExprFlow::Signal(Signal::Return(_)) => {
                Err("`return` cannot escape expression evaluation".to_string())
            }
            ExprFlow::Signal(Signal::Break) => {
                Err("`break` cannot escape expression evaluation".to_string())
            }
            ExprFlow::Signal(Signal::Continue) => {
                Err("`continue` cannot escape expression evaluation".to_string())
            }
        }
    }

    fn eval_expr_flow(&mut self, expr: &Expr) -> Result<ExprFlow, String> {
        match expr {
            // Literals
            Expr::IntLiteral(n, _) => Ok(ExprFlow::Value(Value::Int64(*n))),
            Expr::FloatLiteral(n, _) => Ok(ExprFlow::Value(Value::Float64(*n))),
            Expr::StringLiteral(s, _) => Ok(ExprFlow::Value(Value::String(s.clone()))),
            Expr::StringInterpolation(parts, _) => {
                let mut result = String::new();
                for part in parts {
                    match part {
                        StringPart::Literal(s) => result.push_str(s),
                        StringPart::Expr(expr) => {
                            let val = value_or_signal!(self, expr);
                            result.push_str(&val.to_string());
                        }
                    }
                }
                Ok(ExprFlow::Value(Value::String(result)))
            }
            Expr::BoolLiteral(b, _) => Ok(ExprFlow::Value(Value::Bool(*b))),
            Expr::Nothing(_) => Ok(ExprFlow::Value(Value::Nothing)),
            Expr::Ok(inner, _) => {
                let value = value_or_signal!(self, inner);
                Ok(ExprFlow::Value(Value::ResultOk(Box::new(value))))
            }
            Expr::Fail(inner, _) => {
                let value = value_or_signal!(self, inner);
                Ok(ExprFlow::Value(Value::ResultFail(Box::new(value))))
            }
            Expr::Some(inner, _) => {
                let value = value_or_signal!(self, inner);
                Ok(ExprFlow::Value(Value::OptionalSome(Box::new(value))))
            }
            Expr::None(_) => Ok(ExprFlow::Value(Value::OptionalNone)),
            Expr::Default(inner, _) => {
                let value = value_or_signal!(self, inner);
                Ok(ExprFlow::Signal(Signal::Default(value)))
            }

            // Variables
            Expr::Ident(ident) => self
                .get_variable(&ident.name)
                .cloned()
                .map(ExprFlow::Value)
                .ok_or_else(|| format!("undefined variable '{}'", ident.name)),

            // Parenthesized
            Expr::Paren(inner, _) => self.eval_expr_flow(inner),
            Expr::View(inner, _) => self.eval_expr_flow(inner),
            Expr::Declassify(inner, _) => self.eval_expr_flow(inner),

            // Binary operations
            Expr::Binary(lhs, op, rhs, _) => {
                let left = value_or_signal!(self, lhs);
                // Short-circuit for logical operators
                match op {
                    BinOp::And => {
                        if let Value::Bool(false) = left {
                            return Ok(ExprFlow::Value(Value::Bool(false)));
                        }
                        let right = value_or_signal!(self, rhs);
                        return Ok(ExprFlow::Value(eval_binary_op(&left, *op, &right)?));
                    }
                    BinOp::Or => {
                        if let Value::Bool(true) = left {
                            return Ok(ExprFlow::Value(Value::Bool(true)));
                        }
                        let right = value_or_signal!(self, rhs);
                        return Ok(ExprFlow::Value(eval_binary_op(&left, *op, &right)?));
                    }
                    _ => {}
                }
                let right = value_or_signal!(self, rhs);
                Ok(ExprFlow::Value(eval_binary_op(&left, *op, &right)?))
            }

            // Unary operations
            Expr::Unary(op, operand, _) => {
                let val = value_or_signal!(self, operand);
                match op {
                    UnaryOp::Not => match val {
                        Value::Bool(b) => Ok(ExprFlow::Value(Value::Bool(!b))),
                        _ => Err("'not' requires a boolean operand".to_string()),
                    },
                    UnaryOp::Neg => match val {
                        Value::Int64(n) => Ok(ExprFlow::Value(Value::Int64(-n))),
                        Value::Float64(n) => Ok(ExprFlow::Value(Value::Float64(-n))),
                        _ => Err("unary '-' requires a numeric operand".to_string()),
                    },
                }
            }

            // Function / method calls
            Expr::Call(callee, args, _) | Expr::GenericCall(callee, _, args, _) => {
                // Check for machine construction/transition BEFORE evaluating
                // args, since state-name arguments are bare identifiers (not
                // variables) and would fail evaluation.
                match callee.as_ref() {
                    Expr::Ident(ident) if self.structs.contains_key(&ident.name) => {}
                    Expr::Ident(ident) if self.machines.contains_key(&ident.name) => {
                        return Ok(ExprFlow::Value(self.construct_machine(&ident.name, args)?));
                    }
                    Expr::FieldAccess(obj, field, _) => {
                        if let Expr::Ident(ident) = obj.as_ref() {
                            if field.name == "transition" && self.machines.contains_key(&ident.name)
                            {
                                return Ok(ExprFlow::Value(
                                    self.machine_transition(&ident.name, args)?,
                                ));
                            }
                        }
                    }
                    _ => {}
                }

                let mut arg_values = Vec::with_capacity(args.len());
                for arg in args {
                    arg_values.push(value_or_signal!(self, &arg.value));
                }

                match callee.as_ref() {
                    Expr::Ident(ident) if self.structs.contains_key(&ident.name) => Ok(
                        ExprFlow::Value(self.construct_struct(&ident.name, args, arg_values)?),
                    ),
                    Expr::Ident(ident) => Ok(ExprFlow::Value(
                        self.call_function(&ident.name, arg_values)?,
                    )),
                    // Handle enum variant construction: Type.variant(args)
                    Expr::EnumVariant(type_name, variant, _) => Ok(ExprFlow::Value(Value::Enum {
                        type_name: type_name.name.clone(),
                        variant: variant.name.clone(),
                        fields: arg_values,
                    })),
                    // Handle dotted names: string.trim(...), Stdout.write(...), etc.
                    // Also handles enum variant construction: Shape.circle(5.0)
                    Expr::FieldAccess(obj, field, _) => {
                        let dotted = Self::extract_dotted_name(obj, &field.name);
                        if let Some(ref name) = dotted {
                            // Try built-in functions first.
                            if let Some(result) = self.call_builtin(name, &arg_values) {
                                return Ok(ExprFlow::Value(result?));
                            }
                            // Try user-defined dotted functions.
                            if self.functions.contains_key(name.as_str())
                                || self.resolve_interface_dispatch(name, &arg_values).is_some()
                            {
                                return Ok(ExprFlow::Value(self.call_function(name, arg_values)?));
                            }
                        }
                        // Fall through to enum variant construction if no
                        // built-in or user function matched.
                        if let Expr::Ident(ident) = obj.as_ref() {
                            return Ok(ExprFlow::Value(Value::Enum {
                                type_name: ident.name.clone(),
                                variant: field.name.clone(),
                                fields: arg_values,
                            }));
                        }
                        match dotted {
                            Some(name) => {
                                Ok(ExprFlow::Value(self.call_function(&name, arg_values)?))
                            }
                            None => {
                                Err("only named function calls are supported in comptime"
                                    .to_string())
                            }
                        }
                    }
                    _ => Err("only named function calls are supported in comptime".to_string()),
                }
            }

            // List construction
            Expr::ListConstruct(elems, _) => {
                let mut vals = Vec::with_capacity(elems.len());
                for elem in elems {
                    vals.push(value_or_signal!(self, elem));
                }
                Ok(ExprFlow::Value(Value::List(vals)))
            }

            Expr::Handle(target, bind_name, body, _) => {
                let target_value = value_or_signal!(self, target);
                match target_value {
                    Value::ResultOk(value) => Ok(ExprFlow::Value(*value)),
                    Value::ResultFail(error) => {
                        self.exec_handle_block(bind_name.as_ref(), Some(*error), body)
                    }
                    Value::OptionalSome(value) => Ok(ExprFlow::Value(*value)),
                    Value::OptionalNone => self.exec_handle_block(None, None, body),
                    other => Err(format!(
                        "handle block requires a result or optional value, got {other}"
                    )),
                }
            }

            // Enum variant reference: `Color.red`
            Expr::EnumVariant(type_name, variant, _) => Ok(ExprFlow::Value(Value::Enum {
                type_name: type_name.name.clone(),
                variant: variant.name.clone(),
                fields: vec![],
            })),

            // Field access: struct field access, or enum variant like `Color.red`
            Expr::FieldAccess(obj, field, _) => {
                match obj.as_ref() {
                    Expr::Ident(ident) => {
                        if let Some(value) = self.get_variable(&ident.name).cloned() {
                            self.eval_value_field_access(value, &field.name)
                        } else {
                            // Treat as enum variant: Type.variant
                            Ok(ExprFlow::Value(Value::Enum {
                                type_name: ident.name.clone(),
                                variant: field.name.clone(),
                                fields: vec![],
                            }))
                        }
                    }
                    _ => {
                        let base = value_or_signal!(self, obj);
                        self.eval_value_field_access(base, &field.name)
                    }
                }
            }

            // Coarsen: strip refinement type, returning the underlying value.
            // In the interpreter, the value is already the base type at
            // runtime, so coarsen is a no-op.
            Expr::Coarsen(inner, _) => self.eval_expr_flow(inner),

            // Pipeline: `expr into f into g(extra)`
            // Evaluate the initial expression, then for each step, call the
            // function with the accumulated value as the first argument plus
            // any extra args.
            Expr::Pipeline(initial, steps, _) => {
                let mut value = value_or_signal!(self, initial);
                for step in steps {
                    value = match self.eval_pipeline_step(&step, value)? {
                        ExprFlow::Value(next) => next,
                        ExprFlow::Signal(signal) => return Ok(ExprFlow::Signal(signal)),
                    };
                }
                Ok(ExprFlow::Value(value))
            }

            // State check: `expr at state_name`
            Expr::At(expr, state_name, _) => {
                let val = value_or_signal!(self, expr);
                match val {
                    Value::Machine { state, .. } => {
                        Ok(ExprFlow::Value(Value::Bool(state == state_name.name)))
                    }
                    _ => Err(format!("'at' requires a machine value, got {val}")),
                }
            }

            // Unsupported expressions produce a clear error.
            _ => Err(format!(
                "unsupported expression in comptime: {:?}",
                std::mem::discriminant(expr)
            )),
        }
    }

    /// Evaluate a single pipeline step: call the step's function with the
    /// accumulated `piped_value` as the first argument, followed by any
    /// extra arguments.
    fn eval_pipeline_step(
        &mut self,
        step: &PipelineStep,
        piped_value: Value,
    ) -> Result<ExprFlow, String> {
        // Build argument list: piped value first, then extra args.
        let mut arg_values = vec![piped_value];
        for arg in &step.extra_args {
            arg_values.push(value_or_signal!(self, &arg.value));
        }

        // Resolve the function name from the expression.
        match &step.function {
            Expr::Ident(ident) => Ok(ExprFlow::Value(
                self.call_function(&ident.name, arg_values)?,
            )),
            Expr::FieldAccess(obj, field, _) => {
                let dotted = Self::extract_dotted_name(obj, &field.name);
                if let Some(ref name) = dotted {
                    if let Some(result) = self.call_builtin(name, &arg_values) {
                        return Ok(ExprFlow::Value(result?));
                    }
                    if self.functions.contains_key(name.as_str()) {
                        return Ok(ExprFlow::Value(self.call_function(name, arg_values)?));
                    }
                }
                match dotted {
                    Some(name) => Ok(ExprFlow::Value(self.call_function(&name, arg_values)?)),
                    None => {
                        Err("only named function calls are supported in pipeline steps".to_string())
                    }
                }
            }
            _ => Err("only named function calls are supported in pipeline steps".to_string()),
        }
    }

    fn exec_handle_block(
        &mut self,
        bind_name: Option<&Ident>,
        bind_value: Option<Value>,
        body: &Block,
    ) -> Result<ExprFlow, String> {
        self.push_scope();
        if let (Some(name), Some(value)) = (bind_name, bind_value) {
            self.set_variable(&name.name, value);
        }

        let mut signal = None;
        for stmt in &body.stmts {
            if let Some(next) = self.exec_stmt_inner(stmt)? {
                signal = Some(next);
                break;
            }
        }
        self.pop_scope();

        match signal {
            Some(Signal::Default(value)) => Ok(ExprFlow::Value(value)),
            Some(other) => Ok(ExprFlow::Signal(other)),
            None => Err("handle block must end with return or default".to_string()),
        }
    }

    // -- Statement execution ------------------------------------------------

    /// Execute a single statement.  Returns `Ok(None)` for normal flow, or
    /// a [`Signal`] if control flow must be altered.
    fn exec_stmt_inner(&mut self, stmt: &Stmt) -> Result<Option<Signal>, String> {
        match stmt {
            Stmt::VarDecl(decl) => {
                let type_name = type_expr_name(&decl.ty);
                let val = if self.type_aliases.contains_key(&type_name) {
                    match &decl.value {
                        Expr::Handle(target, bind_name, body, _) => {
                            match self.eval_refinement_boundary(
                                &type_name,
                                target,
                                bind_name.as_ref(),
                                body,
                            )? {
                                ExprFlow::Value(value) => value,
                                ExprFlow::Signal(signal) => return Ok(Some(signal)),
                            }
                        }
                        _ => {
                            let val = match self.eval_expr_flow(&decl.value)? {
                                ExprFlow::Value(value) => value,
                                ExprFlow::Signal(signal) => return Ok(Some(signal)),
                            };
                            self.check_refinement(&type_name, &val)?;
                            val
                        }
                    }
                } else {
                    match self.eval_expr_flow(&decl.value)? {
                        ExprFlow::Value(value) => value,
                        ExprFlow::Signal(signal) => return Ok(Some(signal)),
                    }
                };
                self.set_variable(&decl.name.name, val);
                Ok(None)
            }

            Stmt::Assign(assign) => {
                let val = match self.eval_expr_flow(&assign.value)? {
                    ExprFlow::Value(value) => value,
                    ExprFlow::Signal(signal) => return Ok(Some(signal)),
                };
                match &assign.target {
                    Expr::Ident(ident) => {
                        // If variable doesn't exist yet, create it (handles parser producing
                        // AssignStmt instead of VarDecl for `Type name = expr` patterns)
                        if self.get_variable(&ident.name).is_none() {
                            self.set_variable(&ident.name, val);
                        } else {
                            self.assign_variable(&ident.name, val)?;
                        }
                    }
                    _ => {
                        return Err(
                            "only simple variable assignment is supported in comptime".to_string()
                        );
                    }
                }
                Ok(None)
            }

            Stmt::Return(ret) => {
                let val = match &ret.value {
                    Some(expr) => match self.eval_expr_flow(expr)? {
                        ExprFlow::Value(value) => value,
                        ExprFlow::Signal(signal) => return Ok(Some(signal)),
                    },
                    None => Value::Nothing,
                };
                Ok(Some(Signal::Return(val)))
            }

            Stmt::If(if_stmt) => {
                let cond = match self.eval_expr_flow(&if_stmt.condition)? {
                    ExprFlow::Value(value) => value,
                    ExprFlow::Signal(signal) => return Ok(Some(signal)),
                };
                if is_truthy(&cond)? {
                    return self.exec_block_inner(&if_stmt.then_block);
                }
                for (else_if_cond, else_if_block) in &if_stmt.else_ifs {
                    let val = match self.eval_expr_flow(else_if_cond)? {
                        ExprFlow::Value(value) => value,
                        ExprFlow::Signal(signal) => return Ok(Some(signal)),
                    };
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
                let iterable = match self.eval_expr_flow(&for_stmt.iterable)? {
                    ExprFlow::Value(value) => value,
                    ExprFlow::Signal(signal) => return Ok(Some(signal)),
                };
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
                                Some(other) => return Ok(Some(other)),
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
                    let cond = match self.eval_expr_flow(&while_stmt.condition)? {
                        ExprFlow::Value(value) => value,
                        ExprFlow::Signal(signal) => return Ok(Some(signal)),
                    };
                    if !is_truthy(&cond)? {
                        break;
                    }
                    self.push_scope();
                    let signal = self.exec_block_inner(&while_stmt.body)?;
                    self.pop_scope();
                    match signal {
                        Some(Signal::Break) => break,
                        Some(Signal::Continue) => continue,
                        Some(other) => return Ok(Some(other)),
                        None => {}
                    }
                }
                Ok(None)
            }

            Stmt::Match(match_stmt) => {
                let val = match self.eval_expr_flow(&match_stmt.expr)? {
                    ExprFlow::Value(value) => value,
                    ExprFlow::Signal(signal) => return Ok(Some(signal)),
                };
                let (variant_name, fields) = match &val {
                    Value::Enum {
                        variant, fields, ..
                    } => (variant.clone(), fields.clone()),
                    _ => return Err(format!("match requires an enum value, got {val}")),
                };

                for arm in &match_stmt.arms {
                    match &arm.pattern {
                        Pattern::Ident(ident) => {
                            if ident.name == variant_name {
                                return self.exec_block_inner(&arm.body);
                            }
                        }
                        Pattern::Variant(name, bindings) => {
                            if name.name == variant_name {
                                self.push_scope();
                                for (binding, field_val) in bindings.iter().zip(fields.iter()) {
                                    self.set_variable(&binding.name, field_val.clone());
                                }
                                let result = self.exec_block_inner(&arm.body);
                                self.pop_scope();
                                return result;
                            }
                        }
                        Pattern::Other(_) => {
                            return self.exec_block_inner(&arm.body);
                        }
                    }
                }
                Ok(None)
            }

            Stmt::Expr(expr_stmt) => {
                // Type names appearing as bare ExprStmt (from parser producing ExprStmt
                // instead of VarDecl for `Type name = expr`) are harmless — ignore errors.
                match self.eval_expr_flow(&expr_stmt.expr) {
                    Ok(ExprFlow::Value(_)) => Ok(None),
                    Ok(ExprFlow::Signal(signal)) => Ok(Some(signal)),
                    Err(_) => Ok(None),
                }
            }

            Stmt::Assert(assert_stmt) => {
                let cond = match self.eval_expr_flow(&assert_stmt.condition)? {
                    ExprFlow::Value(value) => value,
                    ExprFlow::Signal(signal) => return Ok(Some(signal)),
                };
                match cond {
                    Value::Bool(true) => Ok(None),
                    Value::Bool(false) => {
                        let msg = if let Some(msg_expr) = &assert_stmt.message {
                            match self.eval_expr_flow(msg_expr)? {
                                ExprFlow::Value(Value::String(s)) => s,
                                ExprFlow::Value(other) => other.to_string(),
                                ExprFlow::Signal(signal) => return Ok(Some(signal)),
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
            None | Some(Signal::Break) | Some(Signal::Continue) | Some(Signal::Return(_)) => Ok(()),
            Some(Signal::Default(_)) => {
                Err("`default` can only be used inside a `handle` block".to_string())
            }
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
            Some(Signal::Default(_)) => {
                Err("`default` can only be used inside a `handle` block".to_string())
            }
            _ => Ok(None),
        }
    }

    // -- Dotted name extraction -----------------------------------------------

    /// Recursively extract a dotted name from nested FieldAccess nodes.
    /// e.g. `Expr::FieldAccess(Expr::Ident("Stdout"), "write")` → `"Stdout.write"`
    fn extract_dotted_name(expr: &Expr, suffix: &str) -> Option<String> {
        match expr {
            Expr::Ident(ident) => Some(format!("{}.{}", ident.name, suffix)),
            Expr::FieldAccess(inner, field, _) => {
                let inner_name = Self::extract_dotted_name(inner, &field.name)?;
                Some(format!("{inner_name}.{suffix}"))
            }
            _ => None,
        }
    }

    // -- Built-in standard library functions ----------------------------------

    /// Try to call a built-in standard library function.  Returns `None` if
    /// the name does not match any built-in, allowing the caller to fall
    /// through to user-defined function lookup.
    fn call_builtin(&self, name: &str, args: &[Value]) -> Option<Result<Value, String>> {
        match name {
            // -- I/O (capability-simulated) -----------------------------------
            "Stdout.write" => {
                // Stdout.write(stdout, message) — ignore capability, print message
                if args.len() < 2 {
                    return Some(Err(format!(
                        "Stdout.write expects 2 arguments, got {}",
                        args.len()
                    )));
                }
                // The first arg is the capability (ignored), second is the message.
                print!("{}", args[1]);
                Some(Ok(Value::Nothing))
            }

            // -- Secret-safe operations --------------------------------------
            "secret.redact" => {
                require_args!(name, 1, args);
                Some(Ok(Value::String("[redacted]".to_string())))
            }

            "secret.compare" => {
                require_args!(name, 2, args);
                Some(Ok(Value::Bool(args[0] == args[1])))
            }

            // -- String operations --------------------------------------------
            "string.length" | "string.char_count" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::String(s) => Some(Ok(Value::Int64(s.chars().count() as i64))),
                    _ => Some(Err(format!("{name} expects a string argument"))),
                }
            }

            "string.contains" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::String(s), Value::String(substr)) => {
                        Some(Ok(Value::Bool(s.contains(substr.as_str()))))
                    }
                    _ => Some(Err(format!("{name} expects two string arguments"))),
                }
            }

            "string.trim" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::String(s) => Some(Ok(Value::String(s.trim().to_string()))),
                    _ => Some(Err(format!("{name} expects a string argument"))),
                }
            }

            "string.upper" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::String(s) => Some(Ok(Value::String(s.to_uppercase()))),
                    _ => Some(Err(format!("{name} expects a string argument"))),
                }
            }

            "string.lower" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::String(s) => Some(Ok(Value::String(s.to_lowercase()))),
                    _ => Some(Err(format!("{name} expects a string argument"))),
                }
            }

            "string.replace" => {
                require_args!(name, 3, args);
                match (&args[0], &args[1], &args[2]) {
                    (Value::String(s), Value::String(from), Value::String(to)) => {
                        Some(Ok(Value::String(s.replace(from.as_str(), to.as_str()))))
                    }
                    _ => Some(Err(format!("{name} expects three string arguments"))),
                }
            }

            "string.split" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::String(s), Value::String(delim)) => {
                        let parts: Vec<Value> = s
                            .split(delim.as_str())
                            .map(|p| Value::String(p.to_string()))
                            .collect();
                        Some(Ok(Value::List(parts)))
                    }
                    _ => Some(Err(format!("{name} expects two string arguments"))),
                }
            }

            "string.join" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::List(items), Value::String(sep)) => {
                        let strs: Result<Vec<String>, String> = items
                            .iter()
                            .map(|v| match v {
                                Value::String(s) => Ok(s.clone()),
                                _ => Err(format!("{name} requires a list of strings, found {v}")),
                            })
                            .collect();
                        match strs {
                            Ok(parts) => Some(Ok(Value::String(parts.join(sep)))),
                            Err(e) => Some(Err(e)),
                        }
                    }
                    _ => Some(Err(format!("{name} expects a list and a string separator"))),
                }
            }

            "string.starts_with" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::String(s), Value::String(prefix)) => {
                        Some(Ok(Value::Bool(s.starts_with(prefix.as_str()))))
                    }
                    _ => Some(Err(format!("{name} expects two string arguments"))),
                }
            }

            "string.ends_with" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::String(s), Value::String(suffix)) => {
                        Some(Ok(Value::Bool(s.ends_with(suffix.as_str()))))
                    }
                    _ => Some(Err(format!("{name} expects two string arguments"))),
                }
            }

            // -- String conversions -------------------------------------------
            "string.from_int64" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::Int64(n) => Some(Ok(Value::String(n.to_string()))),
                    _ => Some(Err(format!("{name} expects an int64 argument"))),
                }
            }

            // -- Int64 conversions --------------------------------------------
            "int64.from_string" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::String(s) => match s.parse::<i64>() {
                        Ok(n) => Some(Ok(Value::ResultOk(Box::new(Value::Int64(n))))),
                        Err(_) => Some(Ok(Value::ResultFail(Box::new(Value::String(format!(
                            "int64.from_string: cannot parse '{s}' as int64"
                        )))))),
                    },
                    _ => Some(Err(format!("{name} expects a string argument"))),
                }
            }

            // -- Float64 conversions ------------------------------------------
            "float64.from_int64" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::Int64(n) => Some(Ok(Value::Float64(*n as f64))),
                    _ => Some(Err(format!("{name} expects an int64 argument"))),
                }
            }

            // -- List operations ----------------------------------------------
            "list.new" => {
                require_args!(name, 0, args);
                Some(Ok(Value::List(vec![])))
            }

            "list.length" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::List(items) => Some(Ok(Value::Int64(items.len() as i64))),
                    _ => Some(Err(format!("{name} expects a list argument"))),
                }
            }

            "list.append" => {
                require_args!(name, 2, args);
                match &args[0] {
                    Value::List(items) => {
                        let mut new_list = items.clone();
                        new_list.push(args[1].clone());
                        Some(Ok(Value::List(new_list)))
                    }
                    _ => Some(Err(format!("{name} expects a list as first argument"))),
                }
            }

            "list.get" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::List(items), Value::Int64(index)) => {
                        let idx = *index as usize;
                        if idx < items.len() {
                            Some(Ok(Value::OptionalSome(Box::new(items[idx].clone()))))
                        } else {
                            Some(Ok(Value::OptionalNone))
                        }
                    }
                    _ => Some(Err(format!("{name} expects a list and an int64 index"))),
                }
            }

            "list.first" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::List(items) => {
                        if items.is_empty() {
                            Some(Ok(Value::OptionalNone))
                        } else {
                            Some(Ok(Value::OptionalSome(Box::new(items[0].clone()))))
                        }
                    }
                    _ => Some(Err(format!("{name} expects a list argument"))),
                }
            }

            "list.last" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::List(items) => {
                        if items.is_empty() {
                            Some(Ok(Value::OptionalNone))
                        } else {
                            Some(Ok(Value::OptionalSome(Box::new(
                                items[items.len() - 1].clone(),
                            ))))
                        }
                    }
                    _ => Some(Err(format!("{name} expects a list argument"))),
                }
            }

            "list.is_empty" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::List(items) => Some(Ok(Value::Bool(items.is_empty()))),
                    _ => Some(Err(format!("{name} expects a list argument"))),
                }
            }

            // -- Math operations ----------------------------------------------
            "math.abs" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::Int64(n) => Some(Ok(Value::Int64(n.abs()))),
                    Value::Float64(n) => Some(Ok(Value::Float64(n.abs()))),
                    _ => Some(Err(format!("{name} expects a numeric argument"))),
                }
            }

            "math.min" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::Int64(a), Value::Int64(b)) => Some(Ok(Value::Int64(*a.min(b)))),
                    (Value::Float64(a), Value::Float64(b)) => Some(Ok(Value::Float64(a.min(*b)))),
                    _ => Some(Err(format!(
                        "{name} expects two arguments of the same numeric type"
                    ))),
                }
            }

            "math.max" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::Int64(a), Value::Int64(b)) => Some(Ok(Value::Int64(*a.max(b)))),
                    (Value::Float64(a), Value::Float64(b)) => Some(Ok(Value::Float64(a.max(*b)))),
                    _ => Some(Err(format!(
                        "{name} expects two arguments of the same numeric type"
                    ))),
                }
            }

            // Not a built-in
            _ => None,
        }
    }

    // -- Function calls -----------------------------------------------------

    /// Call a registered function by name with the given arguments.
    /// Built-in standard library functions are checked first; if the name
    /// does not match a built-in, user-defined functions are consulted.
    pub fn call_function(&mut self, name: &str, args: Vec<Value>) -> Result<Value, String> {
        // Check built-in functions first.
        if let Some(result) = self.call_builtin(name, &args) {
            return result;
        }

        let resolved_name = self
            .resolve_interface_dispatch(name, &args)
            .unwrap_or_else(|| name.to_string());

        // Look up the function definition.
        let func = self
            .functions
            .get(&resolved_name)
            .ok_or_else(|| format!("undefined function '{name}'"))?
            .clone();

        if args.len() != func.params.len() {
            return Err(format!(
                "function '{}' expects {} argument(s), got {}",
                resolved_name,
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
            Some(Signal::Default(_)) => {
                Err("`default` can only be used inside a `handle` block".to_string())
            }
            _ => Ok(Value::Nothing),
        }
    }

    fn resolve_interface_dispatch(&self, name: &str, args: &[Value]) -> Option<String> {
        if self.functions.contains_key(name) {
            return Some(name.to_string());
        }

        let receiver_type = runtime_type_name(args.first()?)?;
        self.interface_methods
            .get(name)
            .and_then(|methods| methods.get(&receiver_type))
            .cloned()
    }

    fn construct_struct(
        &mut self,
        struct_name: &str,
        args: &[CallArg],
        arg_values: Vec<Value>,
    ) -> Result<Value, String> {
        let strukt = self
            .structs
            .get(struct_name)
            .ok_or_else(|| format!("undefined struct '{struct_name}'"))?
            .clone();
        let validates_refinements = strukt
            .fields
            .iter()
            .any(|field| self.type_name_has_refinement(&type_expr_name(&field.ty)));

        if args.len() > strukt.fields.len() {
            return Err(format!(
                "struct '{}' expects {} field argument(s), got {}",
                struct_name,
                strukt.fields.len(),
                args.len()
            ));
        }

        let mut fields: Vec<Option<Value>> = vec![None; strukt.fields.len()];
        for (arg, value) in args.iter().zip(arg_values) {
            let field_index = if let Some(name) = &arg.name {
                strukt
                    .fields
                    .iter()
                    .position(|field| field.name.name == name.name)
                    .ok_or_else(|| format!("struct '{struct_name}' has no field '{}'", name.name))?
            } else {
                let Some(index) = fields.iter().position(|value| value.is_none()) else {
                    return Err(format!(
                        "struct '{}' expects {} field argument(s), got {}",
                        struct_name,
                        strukt.fields.len(),
                        args.len()
                    ));
                };
                index
            };

            if fields[field_index].is_some() {
                return Err(format!(
                    "struct '{}' received field '{}' more than once",
                    struct_name, strukt.fields[field_index].name.name
                ));
            }

            let type_name = type_expr_name(&strukt.fields[field_index].ty);
            if let Err(message) = self.check_refinement(&type_name, &value) {
                if validates_refinements {
                    return Ok(Value::ResultFail(Box::new(Value::String(message))));
                }
                return Err(message);
            }
            fields[field_index] = Some(value);
        }

        for (index, field) in strukt.fields.iter().enumerate() {
            if fields[index].is_none() {
                return Err(format!(
                    "struct '{}' is missing required field '{}'",
                    struct_name, field.name.name
                ));
            }
        }

        let fields = strukt
            .fields
            .iter()
            .zip(fields.into_iter())
            .map(|(field, value)| (field.name.name.clone(), value.unwrap()))
            .collect();

        let value = Value::Struct {
            type_name: struct_name.to_string(),
            fields,
        };

        if validates_refinements {
            Ok(Value::ResultOk(Box::new(value)))
        } else {
            Ok(value)
        }
    }

    fn eval_value_field_access(&self, value: Value, field_name: &str) -> Result<ExprFlow, String> {
        match value {
            Value::Struct { type_name, fields } => fields
                .into_iter()
                .find(|(name, _)| name == field_name)
                .map(|(_, value)| ExprFlow::Value(value))
                .ok_or_else(|| format!("struct '{type_name}' has no field '{field_name}'")),
            other => Err(format!("field access is not supported on {other}")),
        }
    }

    // -- Machine operations -------------------------------------------------

    /// Construct a machine value: `MachineName(state_name, fields...)`
    /// The first argument is a bare identifier naming the initial state (NOT
    /// evaluated).  Remaining arguments are evaluated as field values.
    fn construct_machine(&mut self, machine_name: &str, args: &[CallArg]) -> Result<Value, String> {
        if args.is_empty() {
            return Err(format!(
                "machine '{machine_name}' construction requires at least a state name"
            ));
        }

        // The first argument should be an identifier naming the state.
        let state_name = match &args[0].value {
            Expr::Ident(ident) => ident.name.clone(),
            _ => {
                return Err(format!(
                    "machine '{machine_name}' construction: first argument must be a state name"
                ));
            }
        };

        let machine_def = self.machines.get(machine_name).unwrap().clone();

        // Validate that the state exists.
        if !machine_def.states.iter().any(|s| s.name.name == state_name) {
            return Err(format!(
                "machine '{machine_name}' has no state '{state_name}'"
            ));
        }

        // Evaluate field arguments (args[1..]).
        let mut fields = Vec::new();
        for arg in &args[1..] {
            fields.push(self.eval_expr(&arg.value)?);
        }

        Ok(Value::Machine {
            type_name: machine_name.to_string(),
            state: state_name,
            fields,
        })
    }

    /// Perform a machine transition: `MachineName.transition(value, target_state, fields...)`
    /// - arg 0: the current machine value (evaluated)
    /// - arg 1: a bare identifier naming the target state (NOT evaluated)
    /// - args 2..: fields for the target state (evaluated)
    fn machine_transition(
        &mut self,
        machine_name: &str,
        args: &[CallArg],
    ) -> Result<Value, String> {
        if args.len() < 2 {
            return Err(format!(
                "{machine_name}.transition requires at least 2 arguments (value, target_state)"
            ));
        }

        // arg 0 is evaluated — it should be the current machine value
        let current_value = self.eval_expr(&args[0].value)?;
        let current_state = match &current_value {
            Value::Machine {
                type_name, state, ..
            } => {
                if type_name != machine_name {
                    return Err(format!(
                        "{machine_name}.transition: expected a {machine_name} value, got {type_name}"
                    ));
                }
                state.clone()
            }
            _ => {
                return Err(format!(
                    "{machine_name}.transition: first argument must be a machine value"
                ));
            }
        };

        // arg 1 should be a bare identifier naming the target state
        let target_state = match &args[1].value {
            Expr::Ident(ident) => ident.name.clone(),
            _ => {
                return Err(format!(
                    "{machine_name}.transition: second argument must be a state name"
                ));
            }
        };

        let machine_def = self.machines.get(machine_name).unwrap().clone();

        // Validate that the target state exists.
        if !machine_def
            .states
            .iter()
            .any(|s| s.name.name == target_state)
        {
            return Err(format!(
                "machine '{machine_name}' has no state '{target_state}'"
            ));
        }

        // Validate that the transition is allowed.
        let transition_allowed = machine_def
            .transitions
            .iter()
            .any(|t| t.from.name == current_state && t.to.name == target_state);
        if !transition_allowed {
            return Err(format!(
                "machine '{machine_name}': transition from '{current_state}' to '{target_state}' is not allowed"
            ));
        }

        // Evaluate field arguments (args[2..]).
        let mut fields = Vec::new();
        for arg in &args[2..] {
            fields.push(self.eval_expr(&arg.value)?);
        }

        Ok(Value::Machine {
            type_name: machine_name.to_string(),
            state: target_state,
            fields,
        })
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
        (Value::Int64(a), BinOp::Add, Value::Int64(b)) => a
            .checked_add(*b)
            .map(Value::Int64)
            .ok_or_else(|| format!("integer overflow: {a} + {b}")),
        (Value::Int64(a), BinOp::Sub, Value::Int64(b)) => a
            .checked_sub(*b)
            .map(Value::Int64)
            .ok_or_else(|| format!("integer overflow: {a} - {b}")),
        (Value::Int64(a), BinOp::Mul, Value::Int64(b)) => a
            .checked_mul(*b)
            .map(Value::Int64)
            .ok_or_else(|| format!("integer overflow: {a} * {b}")),
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
        (Value::String(a), BinOp::Add, Value::String(b)) => Ok(Value::String(format!("{a}{b}"))),

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

/// Extract the simple name from a `TypeExpr` (e.g. `"int64"`, `"Port"`, `"list"`).
fn type_expr_name(ty: &TypeExpr) -> String {
    match ty {
        TypeExpr::Named(ident) => ident.name.clone(),
        TypeExpr::Generic(ident, _, _) => ident.name.clone(),
        TypeExpr::View(inner, _) => type_expr_name(inner),
    }
}

fn runtime_type_name(value: &Value) -> Option<String> {
    match value {
        Value::Int64(_) => Some("int64".to_string()),
        Value::Float64(_) => Some("float64".to_string()),
        Value::String(_) => Some("string".to_string()),
        Value::Bool(_) => Some("bool".to_string()),
        Value::List(_) => Some("list".to_string()),
        Value::ResultOk(_) | Value::ResultFail(_) => Some("result".to_string()),
        Value::OptionalSome(_) | Value::OptionalNone => Some("optional".to_string()),
        Value::Nothing => Some("nothing".to_string()),
        Value::Struct { type_name, .. }
        | Value::Enum { type_name, .. }
        | Value::Machine { type_name, .. } => Some(type_name.clone()),
        Value::Error(_) => None,
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

    fn type_alias(name: &str, base: &str, constraint: Option<Expr>) -> TypeAlias {
        TypeAlias {
            name: ident(name),
            base_type: type_named(base),
            constraint,
            span: sp(),
        }
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
        Block { stmts, span: sp() }
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

    /// Helper: create a field access expression.
    fn field_access(base: Expr, field: &str) -> Expr {
        Expr::FieldAccess(Box::new(base), ident(field), sp())
    }

    /// Helper: create a named call argument.
    fn named_arg(name: &str, value: Expr) -> CallArg {
        CallArg {
            name: Some(ident(name)),
            value,
            span: sp(),
        }
    }

    /// Helper: create a dotted call expression like `Point.total(view point)`.
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

    /// Helper: create a simple struct definition.
    fn struct_def(name: &str, fields: Vec<(&str, &str)>, methods: Vec<FunctionDef>) -> StructDef {
        StructDef {
            name: ident(name),
            fields: fields
                .into_iter()
                .map(|(field_name, field_ty)| FieldDef {
                    name: ident(field_name),
                    ty: type_named(field_ty),
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
    // String interpolation
    // -----------------------------------------------------------------------

    #[test]
    fn string_interpolation_simple() {
        // "hello {name}" with name = "world"
        let mut interp = Interpreter::new();
        interp
            .exec_stmt(&var_decl("name", string("world")))
            .unwrap();
        let expr = Expr::StringInterpolation(
            vec![
                StringPart::Literal("hello ".to_string()),
                StringPart::Expr(Box::new(var("name"))),
            ],
            sp(),
        );
        assert_eq!(
            interp.eval_expr(&expr).unwrap(),
            Value::String("hello world".to_string())
        );
    }

    #[test]
    fn string_interpolation_multiple() {
        // "{a} + {b} = {c}" with a=2, b=3, c=5
        let mut interp = Interpreter::new();
        interp.exec_stmt(&var_decl("a", int(2))).unwrap();
        interp.exec_stmt(&var_decl("b", int(3))).unwrap();
        interp.exec_stmt(&var_decl("c", int(5))).unwrap();
        let expr = Expr::StringInterpolation(
            vec![
                StringPart::Expr(Box::new(var("a"))),
                StringPart::Literal(" + ".to_string()),
                StringPart::Expr(Box::new(var("b"))),
                StringPart::Literal(" = ".to_string()),
                StringPart::Expr(Box::new(var("c"))),
            ],
            sp(),
        );
        assert_eq!(
            interp.eval_expr(&expr).unwrap(),
            Value::String("2 + 3 = 5".to_string())
        );
    }

    #[test]
    fn string_interpolation_with_function_call() {
        // "result: {add(2, 3)}" with a function add(a, b) returns a + b
        let mut interp = Interpreter::new();
        let add_fn = func_def(
            "add",
            vec![("a", "int64"), ("b", "int64")],
            Block {
                stmts: vec![Stmt::Return(ReturnStmt {
                    value: Some(binary(var("a"), BinOp::Add, var("b"))),
                    span: sp(),
                })],
                span: sp(),
            },
        );
        interp.register_function(&add_fn);
        let expr = Expr::StringInterpolation(
            vec![
                StringPart::Literal("result: ".to_string()),
                StringPart::Expr(Box::new(call("add", vec![int(2), int(3)]))),
            ],
            sp(),
        );
        assert_eq!(
            interp.eval_expr(&expr).unwrap(),
            Value::String("result: 5".to_string())
        );
    }

    #[test]
    fn string_interpolation_plain_string_still_works() {
        // "plain string" with no interpolation should still work as StringLiteral
        let mut interp = Interpreter::new();
        let expr = string("plain string");
        assert_eq!(
            interp.eval_expr(&expr).unwrap(),
            Value::String("plain string".to_string())
        );
    }

    #[test]
    fn declassify_is_a_runtime_no_op() {
        let mut interp = Interpreter::new();
        interp.set_variable_public("api_key", Value::String("abc".to_string()));

        let expr = Expr::Declassify(Box::new(var("api_key")), sp());
        assert_eq!(
            interp.eval_expr(&expr).unwrap(),
            Value::String("abc".to_string())
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

        let result = interp
            .eval_expr(&call("add", vec![int(3), int(4)]))
            .unwrap();
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
    // User-defined structs
    // -----------------------------------------------------------------------

    #[test]
    fn struct_constructor_returns_struct_value() {
        let mut interp = Interpreter::new();
        interp.register_struct(&struct_def(
            "Point",
            vec![("x", "int64"), ("y", "int64")],
            vec![],
        ));

        let expr = Expr::Call(
            Box::new(var("Point")),
            vec![named_arg("x", int(3)), named_arg("y", int(4))],
            sp(),
        );

        assert_eq!(
            interp.eval_expr(&expr).unwrap(),
            Value::Struct {
                type_name: "Point".to_string(),
                fields: vec![
                    ("x".to_string(), Value::Int64(3)),
                    ("y".to_string(), Value::Int64(4)),
                ],
            }
        );
    }

    #[test]
    fn struct_constructor_with_refinement_field_returns_result_ok() {
        let mut interp = Interpreter::new();
        interp.register_type_alias(&type_alias(
            "Age",
            "int64",
            Some(binary(
                binary(var("value"), BinOp::GtEq, int(0)),
                BinOp::And,
                binary(var("value"), BinOp::Lt, int(150)),
            )),
        ));
        interp.register_struct(&struct_def("User", vec![("age", "Age")], vec![]));

        let expr = Expr::Call(Box::new(var("User")), vec![named_arg("age", int(42))], sp());

        assert_eq!(
            interp.eval_expr(&expr).unwrap(),
            Value::ResultOk(Box::new(Value::Struct {
                type_name: "User".to_string(),
                fields: vec![("age".to_string(), Value::Int64(42))],
            }))
        );
    }

    #[test]
    fn struct_constructor_with_refinement_field_returns_result_fail() {
        let mut interp = Interpreter::new();
        interp.register_type_alias(&type_alias(
            "Age",
            "int64",
            Some(binary(
                binary(var("value"), BinOp::GtEq, int(0)),
                BinOp::And,
                binary(var("value"), BinOp::Lt, int(150)),
            )),
        ));
        interp.register_struct(&struct_def("User", vec![("age", "Age")], vec![]));

        let expr = Expr::Call(
            Box::new(var("User")),
            vec![named_arg("age", int(200))],
            sp(),
        );

        assert_eq!(
            interp.eval_expr(&expr).unwrap(),
            Value::ResultFail(Box::new(Value::String(
                "refinement type constraint failed for 'Age'".to_string(),
            )))
        );
    }

    #[test]
    fn struct_field_access_reads_registered_field() {
        let mut interp = Interpreter::new();
        interp.register_struct(&struct_def(
            "Point",
            vec![("x", "int64"), ("y", "int64")],
            vec![],
        ));

        let expr = field_access(
            Expr::Call(
                Box::new(var("Point")),
                vec![named_arg("x", int(8)), named_arg("y", int(13))],
                sp(),
            ),
            "x",
        );

        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Int64(8));
    }

    #[test]
    fn struct_method_call_uses_struct_fields() {
        let mut interp = Interpreter::new();
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

        interp.register_struct(&struct_def(
            "Point",
            vec![("x", "int64"), ("y", "int64")],
            vec![total_method],
        ));
        interp
            .exec_stmt(&Stmt::VarDecl(VarDecl {
                mutable: false,
                ty: type_named("Point"),
                name: ident("point"),
                value: Expr::Call(
                    Box::new(var("Point")),
                    vec![named_arg("x", int(10)), named_arg("y", int(20))],
                    sp(),
                ),
                span: sp(),
            }))
            .unwrap();

        let expr = dotted_call(
            "Point",
            "total",
            vec![Expr::View(Box::new(var("point")), sp())],
        );
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Int64(30));
    }

    #[test]
    fn interface_call_dispatches_to_struct_implementation() {
        let mut interp = Interpreter::new();
        interp.register_interface(&interface_decl(
            "Speaker",
            vec![("speak", vec![("self", "Speaker", true)], "string")],
        ));
        interp.register_struct(&struct_def("Dog", vec![("name", "string")], vec![]));

        let mut speak_method = func_def(
            "speak",
            vec![("self", "Dog")],
            block(vec![return_stmt(field_access(var("self"), "name"))]),
        );
        speak_method.params[0].view = true;
        interp.register_implement_block(&implement_block("Speaker", "Dog", vec![speak_method]));

        interp
            .exec_stmt(&Stmt::VarDecl(VarDecl {
                mutable: false,
                ty: type_named("Dog"),
                name: ident("dog"),
                value: Expr::Call(
                    Box::new(var("Dog")),
                    vec![named_arg("name", string("woof"))],
                    sp(),
                ),
                span: sp(),
            }))
            .unwrap();

        let expr = dotted_call(
            "Speaker",
            "speak",
            vec![Expr::View(Box::new(var("dog")), sp())],
        );
        assert_eq!(
            interp.eval_expr(&expr).unwrap(),
            Value::String("woof".to_string())
        );
    }

    #[test]
    fn interface_call_dispatches_to_primitive_implementation() {
        let mut interp = Interpreter::new();
        interp.register_interface(&interface_decl(
            "Displayable",
            vec![("display", vec![("self", "Displayable", true)], "string")],
        ));

        let mut display_method = func_def(
            "display",
            vec![("self", "int64")],
            block(vec![return_stmt(string("forty-two"))]),
        );
        display_method.params[0].view = true;
        interp.register_implement_block(&implement_block(
            "Displayable",
            "int64",
            vec![display_method],
        ));

        let expr = dotted_call(
            "Displayable",
            "display",
            vec![Expr::View(Box::new(int(42)), sp())],
        );
        assert_eq!(
            interp.eval_expr(&expr).unwrap(),
            Value::String("forty-two".to_string())
        );
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
        let expr = call(
            "add",
            vec![call("double", vec![int(3)]), call("double", vec![int(5)])],
        );
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

        let result = interp.eval_expr(&call("factorial", vec![int(5)])).unwrap();
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
        let stmt = assert_stmt(binary(int(2), BinOp::Add, int(2)), None);
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

    // -----------------------------------------------------------------------
    // Match statement
    // -----------------------------------------------------------------------

    /// Helper: create an enum value expression (via EnumVariant AST node).
    fn enum_variant(type_name: &str, variant_name: &str) -> Expr {
        Expr::EnumVariant(ident(type_name), ident(variant_name), sp())
    }

    /// Helper: create a match statement.
    fn match_stmt(expr: Expr, arms: Vec<(Pattern, Vec<Stmt>)>) -> Stmt {
        Stmt::Match(MatchStmt {
            expr,
            arms: arms
                .into_iter()
                .map(|(pattern, stmts)| MatchArm {
                    pattern,
                    body: block(stmts),
                    span: sp(),
                })
                .collect(),
            span: sp(),
        })
    }

    #[test]
    fn match_simple_enum_variant() {
        // match Color.green:
        //     red:  result = "red"
        //     green: result = "green"
        //     blue:  result = "blue"
        let mut interp = Interpreter::new();
        interp
            .exec_stmt(&var_decl("result", string("none")))
            .unwrap();

        let m = match_stmt(
            enum_variant("Color", "green"),
            vec![
                (
                    Pattern::Ident(ident("red")),
                    vec![assign("result", string("red"))],
                ),
                (
                    Pattern::Ident(ident("green")),
                    vec![assign("result", string("green"))],
                ),
                (
                    Pattern::Ident(ident("blue")),
                    vec![assign("result", string("blue"))],
                ),
            ],
        );
        interp.exec_stmt(&m).unwrap();
        assert_eq!(
            interp.eval_expr(&var("result")).unwrap(),
            Value::String("green".to_string())
        );
    }

    #[test]
    fn match_destructuring_binds_variables() {
        // shape = Shape.circle(5)
        // match shape:
        //     circle(r):
        //         result = r
        //     rect(w, h):
        //         result = w + h
        let mut interp = Interpreter::new();
        interp.exec_stmt(&var_decl("result", int(0))).unwrap();
        interp.set_variable(
            "shape",
            Value::Enum {
                type_name: "Shape".to_string(),
                variant: "circle".to_string(),
                fields: vec![Value::Int64(5)],
            },
        );

        let m = match_stmt(
            var("shape"),
            vec![
                (
                    Pattern::Variant(ident("circle"), vec![ident("r")]),
                    vec![assign("result", var("r"))],
                ),
                (
                    Pattern::Variant(ident("rect"), vec![ident("w"), ident("h")]),
                    vec![assign("result", binary(var("w"), BinOp::Add, var("h")))],
                ),
            ],
        );
        interp.exec_stmt(&m).unwrap();
        assert_eq!(interp.eval_expr(&var("result")).unwrap(), Value::Int64(5));
    }

    #[test]
    fn match_destructuring_rect_variant() {
        // shape = Shape.rect(3, 4)
        // match shape:
        //     circle(r):
        //         result = r
        //     rect(w, h):
        //         result = w + h
        let mut interp = Interpreter::new();
        interp.exec_stmt(&var_decl("result", int(0))).unwrap();
        interp.set_variable(
            "shape",
            Value::Enum {
                type_name: "Shape".to_string(),
                variant: "rect".to_string(),
                fields: vec![Value::Int64(3), Value::Int64(4)],
            },
        );

        let m = match_stmt(
            var("shape"),
            vec![
                (
                    Pattern::Variant(ident("circle"), vec![ident("r")]),
                    vec![assign("result", var("r"))],
                ),
                (
                    Pattern::Variant(ident("rect"), vec![ident("w"), ident("h")]),
                    vec![assign("result", binary(var("w"), BinOp::Add, var("h")))],
                ),
            ],
        );
        interp.exec_stmt(&m).unwrap();
        assert_eq!(interp.eval_expr(&var("result")).unwrap(), Value::Int64(7));
    }

    #[test]
    fn match_other_catch_all() {
        // match Color.blue:
        //     red: result = "red"
        //     other: result = "other"
        let mut interp = Interpreter::new();
        interp
            .exec_stmt(&var_decl("result", string("none")))
            .unwrap();

        let m = match_stmt(
            enum_variant("Color", "blue"),
            vec![
                (
                    Pattern::Ident(ident("red")),
                    vec![assign("result", string("red"))],
                ),
                (
                    Pattern::Other(sp()),
                    vec![assign("result", string("other"))],
                ),
            ],
        );
        interp.exec_stmt(&m).unwrap();
        assert_eq!(
            interp.eval_expr(&var("result")).unwrap(),
            Value::String("other".to_string())
        );
    }

    #[test]
    fn match_with_return_signal() {
        // Test that return inside a match arm propagates correctly.
        let mut interp = Interpreter::new();

        // function pick(color: ?) returns string:
        //     match color:
        //         red: return "red"
        //         other: return "unknown"
        let b = block(vec![match_stmt(
            var("color"),
            vec![
                (
                    Pattern::Ident(ident("red")),
                    vec![return_stmt(string("red"))],
                ),
                (Pattern::Other(sp()), vec![return_stmt(string("unknown"))]),
            ],
        )]);

        // Execute as a block with a variable set up.
        interp.set_variable(
            "color",
            Value::Enum {
                type_name: "Color".to_string(),
                variant: "red".to_string(),
                fields: vec![],
            },
        );
        let result = interp.exec_block(&b).unwrap();
        assert_eq!(result, Some(Value::String("red".to_string())));
    }
}

#[cfg(test)]
mod builtin_tests {
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

    fn float(n: f64) -> Expr {
        Expr::FloatLiteral(n, sp())
    }

    fn string(s: &str) -> Expr {
        Expr::StringLiteral(s.to_string(), sp())
    }

    fn var(name: &str) -> Expr {
        Expr::Ident(ident(name))
    }

    fn dotted_call(module: &str, func_name: &str, args: Vec<Expr>) -> Expr {
        let callee = Expr::FieldAccess(Box::new(var(module)), ident(func_name), sp());
        Expr::Call(
            Box::new(callee),
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

    fn default_stmt(value: Expr) -> Stmt {
        Stmt::Expr(ExprStmt {
            expr: Expr::Default(Box::new(value), sp()),
            span: sp(),
        })
    }

    fn return_stmt(value: Expr) -> Stmt {
        Stmt::Return(ReturnStmt {
            value: Some(value),
            span: sp(),
        })
    }

    fn block(stmts: Vec<Stmt>) -> Block {
        Block { stmts, span: sp() }
    }

    fn func_def(name: &str, body: Block) -> FunctionDef {
        FunctionDef {
            name: ident(name),
            params: vec![],
            return_type: None,
            body,
            span: sp(),
        }
    }

    #[test]
    fn builtin_stdout_write() {
        let mut interp = Interpreter::new();
        let expr = dotted_call("Stdout", "write", vec![string("fake_cap"), string("hello")]);
        let result = interp.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Nothing);
    }

    #[test]
    fn builtin_string_char_count() {
        let mut interp = Interpreter::new();
        let expr = dotted_call("string", "char_count", vec![string("hello")]);
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Int64(5));
    }

    #[test]
    fn builtin_string_char_count_unicode() {
        let mut interp = Interpreter::new();
        let expr = dotted_call(
            "string",
            "char_count",
            vec![string("\u{00e9}\u{00e9}\u{00e9}")],
        );
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Int64(3));
    }

    #[test]
    fn builtin_string_length() {
        let mut interp = Interpreter::new();
        let expr = dotted_call("string", "length", vec![string("test")]);
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Int64(4));
    }

    #[test]
    fn builtin_string_contains_true() {
        let mut interp = Interpreter::new();
        let expr = dotted_call(
            "string",
            "contains",
            vec![string("hello world"), string("world")],
        );
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Bool(true));
    }

    #[test]
    fn builtin_string_contains_false() {
        let mut interp = Interpreter::new();
        let expr = dotted_call(
            "string",
            "contains",
            vec![string("hello world"), string("xyz")],
        );
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Bool(false));
    }

    #[test]
    fn builtin_string_trim() {
        let mut interp = Interpreter::new();
        let expr = dotted_call("string", "trim", vec![string("  hello  ")]);
        assert_eq!(
            interp.eval_expr(&expr).unwrap(),
            Value::String("hello".to_string())
        );
    }

    #[test]
    fn builtin_string_upper() {
        let mut interp = Interpreter::new();
        let expr = dotted_call("string", "upper", vec![string("hello")]);
        assert_eq!(
            interp.eval_expr(&expr).unwrap(),
            Value::String("HELLO".to_string())
        );
    }

    #[test]
    fn builtin_string_lower() {
        let mut interp = Interpreter::new();
        let expr = dotted_call("string", "lower", vec![string("HELLO")]);
        assert_eq!(
            interp.eval_expr(&expr).unwrap(),
            Value::String("hello".to_string())
        );
    }

    #[test]
    fn builtin_string_split() {
        let mut interp = Interpreter::new();
        let expr = dotted_call("string", "split", vec![string("a,b,c"), string(",")]);
        assert_eq!(
            interp.eval_expr(&expr).unwrap(),
            Value::List(vec![
                Value::String("a".to_string()),
                Value::String("b".to_string()),
                Value::String("c".to_string()),
            ])
        );
    }

    #[test]
    fn builtin_string_join() {
        let mut interp = Interpreter::new();
        let list_expr = Expr::ListConstruct(vec![string("a"), string("b"), string("c")], sp());
        let expr = dotted_call("string", "join", vec![list_expr, string(", ")]);
        assert_eq!(
            interp.eval_expr(&expr).unwrap(),
            Value::String("a, b, c".to_string())
        );
    }

    #[test]
    fn builtin_string_starts_with() {
        let mut interp = Interpreter::new();
        let expr = dotted_call(
            "string",
            "starts_with",
            vec![string("hello world"), string("hello")],
        );
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Bool(true));
    }

    #[test]
    fn builtin_string_ends_with() {
        let mut interp = Interpreter::new();
        let expr = dotted_call(
            "string",
            "ends_with",
            vec![string("hello world"), string("world")],
        );
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Bool(true));
    }

    #[test]
    fn builtin_string_from_int64() {
        let mut interp = Interpreter::new();
        let expr = dotted_call("string", "from_int64", vec![int(42)]);
        assert_eq!(
            interp.eval_expr(&expr).unwrap(),
            Value::String("42".to_string())
        );
    }

    #[test]
    fn builtin_secret_redact() {
        let mut interp = Interpreter::new();
        let expr = dotted_call("secret", "redact", vec![string("top-secret")]);
        assert_eq!(
            interp.eval_expr(&expr).unwrap(),
            Value::String("[redacted]".to_string())
        );
    }

    #[test]
    fn builtin_secret_compare() {
        let mut interp = Interpreter::new();
        let expr = dotted_call("secret", "compare", vec![string("a"), string("a")]);
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Bool(true));
    }

    #[test]
    fn builtin_list_length() {
        let mut interp = Interpreter::new();
        let list_expr = Expr::ListConstruct(vec![int(1), int(2), int(3)], sp());
        let expr = dotted_call("list", "length", vec![list_expr]);
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Int64(3));
    }

    #[test]
    fn builtin_list_append() {
        let mut interp = Interpreter::new();
        let list_expr = Expr::ListConstruct(vec![int(1), int(2)], sp());
        let expr = dotted_call("list", "append", vec![list_expr, int(3)]);
        assert_eq!(
            interp.eval_expr(&expr).unwrap(),
            Value::List(vec![Value::Int64(1), Value::Int64(2), Value::Int64(3)])
        );
    }

    #[test]
    fn builtin_list_get() {
        let mut interp = Interpreter::new();
        let list_expr = Expr::ListConstruct(vec![int(10), int(20), int(30)], sp());
        let expr = dotted_call("list", "get", vec![list_expr, int(1)]);
        assert_eq!(
            interp.eval_expr(&expr).unwrap(),
            Value::OptionalSome(Box::new(Value::Int64(20)))
        );
    }

    #[test]
    fn builtin_list_get_out_of_bounds() {
        let mut interp = Interpreter::new();
        let list_expr = Expr::ListConstruct(vec![int(10)], sp());
        let expr = dotted_call("list", "get", vec![list_expr, int(5)]);
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::OptionalNone);
    }

    #[test]
    fn builtin_list_first() {
        let mut interp = Interpreter::new();
        let list_expr = Expr::ListConstruct(vec![int(10), int(20)], sp());
        let expr = dotted_call("list", "first", vec![list_expr]);
        assert_eq!(
            interp.eval_expr(&expr).unwrap(),
            Value::OptionalSome(Box::new(Value::Int64(10)))
        );
    }

    #[test]
    fn builtin_list_last() {
        let mut interp = Interpreter::new();
        let list_expr = Expr::ListConstruct(vec![int(10), int(20)], sp());
        let expr = dotted_call("list", "last", vec![list_expr]);
        assert_eq!(
            interp.eval_expr(&expr).unwrap(),
            Value::OptionalSome(Box::new(Value::Int64(20)))
        );
    }

    #[test]
    fn builtin_list_new() {
        let mut interp = Interpreter::new();
        let expr = Expr::GenericCall(
            Box::new(Expr::FieldAccess(Box::new(var("list")), ident("new"), sp())),
            vec![TypeExpr::Named(ident("int64"))],
            vec![],
            sp(),
        );
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::List(vec![]));
    }

    #[test]
    fn builtin_list_is_empty_true() {
        let mut interp = Interpreter::new();
        let list_expr = Expr::ListConstruct(vec![], sp());
        let expr = dotted_call("list", "is_empty", vec![list_expr]);
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Bool(true));
    }

    #[test]
    fn builtin_list_is_empty_false() {
        let mut interp = Interpreter::new();
        let list_expr = Expr::ListConstruct(vec![int(1)], sp());
        let expr = dotted_call("list", "is_empty", vec![list_expr]);
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Bool(false));
    }

    #[test]
    fn builtin_math_abs_positive() {
        let mut interp = Interpreter::new();
        let expr = dotted_call("math", "abs", vec![int(5)]);
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Int64(5));
    }

    #[test]
    fn builtin_math_abs_negative() {
        let mut interp = Interpreter::new();
        let expr = dotted_call("math", "abs", vec![int(-7)]);
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Int64(7));
    }

    #[test]
    fn builtin_math_abs_float() {
        let mut interp = Interpreter::new();
        let expr = dotted_call("math", "abs", vec![float(-3.5)]);
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Float64(3.5));
    }

    #[test]
    fn builtin_math_min_int() {
        let mut interp = Interpreter::new();
        let expr = dotted_call("math", "min", vec![int(3), int(7)]);
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Int64(3));
    }

    #[test]
    fn builtin_math_max_int() {
        let mut interp = Interpreter::new();
        let expr = dotted_call("math", "max", vec![int(3), int(7)]);
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Int64(7));
    }

    #[test]
    fn builtin_math_min_float() {
        let mut interp = Interpreter::new();
        let expr = dotted_call("math", "min", vec![float(1.5), float(2.5)]);
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Float64(1.5));
    }

    #[test]
    fn builtin_math_max_float() {
        let mut interp = Interpreter::new();
        let expr = dotted_call("math", "max", vec![float(1.5), float(2.5)]);
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Float64(2.5));
    }

    #[test]
    fn builtin_int64_from_string() {
        let mut interp = Interpreter::new();
        let expr = dotted_call("int64", "from_string", vec![string("123")]);
        assert_eq!(
            interp.eval_expr(&expr).unwrap(),
            Value::ResultOk(Box::new(Value::Int64(123)))
        );
    }

    #[test]
    fn builtin_int64_from_string_error() {
        let mut interp = Interpreter::new();
        let expr = dotted_call("int64", "from_string", vec![string("abc")]);
        assert_eq!(
            interp.eval_expr(&expr).unwrap(),
            Value::ResultFail(Box::new(Value::String(
                "int64.from_string: cannot parse 'abc' as int64".to_string(),
            )))
        );
    }

    #[test]
    fn handle_result_uses_default_value() {
        let mut interp = Interpreter::new();
        let expr = Expr::Handle(
            Box::new(dotted_call("int64", "from_string", vec![string("abc")])),
            Some(ident("error")),
            block(vec![default_stmt(int(7))]),
            sp(),
        );
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Int64(7));
    }

    #[test]
    fn handle_optional_uses_default_value() {
        let mut interp = Interpreter::new();
        let expr = Expr::Handle(
            Box::new(Expr::None(sp())),
            None,
            block(vec![default_stmt(int(5))]),
            sp(),
        );
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Int64(5));
    }

    #[test]
    fn handle_return_exits_enclosing_function() {
        let mut interp = Interpreter::new();
        let parse_fn = func_def(
            "parse_or_default",
            block(vec![
                Stmt::VarDecl(VarDecl {
                    mutable: false,
                    ty: TypeExpr::Named(ident("int64")),
                    name: ident("parsed"),
                    value: Expr::Handle(
                        Box::new(dotted_call("int64", "from_string", vec![string("abc")])),
                        Some(ident("error")),
                        block(vec![return_stmt(int(9))]),
                        sp(),
                    ),
                    span: sp(),
                }),
                return_stmt(var("parsed")),
            ]),
        );
        interp.register_function(&parse_fn);

        let result = interp.call_function("parse_or_default", vec![]).unwrap();
        assert_eq!(result, Value::Int64(9));
    }

    #[test]
    fn builtin_float64_from_int64() {
        let mut interp = Interpreter::new();
        let expr = dotted_call("float64", "from_int64", vec![int(42)]);
        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Float64(42.0));
    }

    #[test]
    fn builtin_wrong_arg_count() {
        let mut interp = Interpreter::new();
        let expr = dotted_call("string", "trim", vec![string("a"), string("b")]);
        assert!(interp.eval_expr(&expr).is_err());
    }

    #[test]
    fn pipeline_string_trim_and_upper() {
        // "  hello  " into string.trim into string.upper => "HELLO"
        let mut interp = Interpreter::new();
        let initial = string("  hello  ");
        let steps = vec![
            PipelineStep {
                function: Expr::FieldAccess(Box::new(var("string")), ident("trim"), sp()),
                extra_args: vec![],
                span: sp(),
            },
            PipelineStep {
                function: Expr::FieldAccess(Box::new(var("string")), ident("upper"), sp()),
                extra_args: vec![],
                span: sp(),
            },
        ];
        let expr = Expr::Pipeline(Box::new(initial), steps, sp());
        let result = interp.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::String("HELLO".to_string()));
    }

    #[test]
    fn pipeline_string_replace_with_extra_args() {
        // "hello world" into string.replace("world", "jett") => "hello jett"
        let mut interp = Interpreter::new();
        let initial = string("hello world");
        let steps = vec![PipelineStep {
            function: Expr::FieldAccess(Box::new(var("string")), ident("replace"), sp()),
            extra_args: vec![
                CallArg {
                    name: None,
                    value: string("world"),
                    span: sp(),
                },
                CallArg {
                    name: None,
                    value: string("jett"),
                    span: sp(),
                },
            ],
            span: sp(),
        }];
        let expr = Expr::Pipeline(Box::new(initial), steps, sp());
        let result = interp.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::String("hello jett".to_string()));
    }
}
