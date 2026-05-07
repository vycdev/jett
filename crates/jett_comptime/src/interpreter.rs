use std::collections::{BTreeMap, HashMap, HashSet};

use rand::Rng;
use serde_json::Value as JsonValue;

use jett_parser::ast::{
    ActorDef, BinOp, BitfieldDef, BitfieldFieldKind, Block, CallArg, EnumDef, Expr, FunctionDef,
    Ident, ImplementBlock, InterfaceDecl, MachineDef, Pattern, PipelineStep, Stmt, StringPart,
    StructDef, TypeAlias, TypeExpr, UnaryOp,
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
    Respond(Value),
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

#[derive(Debug, Clone)]
struct ReflectionField {
    name: String,
    ty: TypeExpr,
    serialize_name: String,
}

#[derive(Debug, Clone)]
struct ReflectionVariant {
    name: String,
    fields: Vec<ReflectionField>,
}

pub struct Interpreter {
    /// Stack of lexical scopes. The last element is the innermost scope.
    scopes: Vec<Environment>,
    /// User-defined functions available for calling.
    functions: HashMap<String, FunctionDef>,
    /// Registered user-defined structs available for construction and field access.
    structs: HashMap<String, StructDef>,
    /// Registered user-defined bitfields available for construction and field access.
    bitfields: HashMap<String, BitfieldDef>,
    /// Registered enums for bitfield enum annotations and runtime mapping.
    enums: HashMap<String, EnumDef>,
    /// Interface dotted name -> concrete runtime type -> concrete dotted function name.
    interface_methods: HashMap<String, HashMap<String, String>>,
    /// Registered type alias base expressions.
    type_alias_bases: HashMap<String, TypeExpr>,
    /// Registered type aliases: name -> (base_type_name, optional constraint).
    type_aliases: HashMap<String, Option<RefinementDef>>,
    /// Registered state machine definitions: name -> MachineDef.
    machines: HashMap<String, MachineDef>,
    /// Registered actor definitions: type_name -> ActorDef (AST).
    actor_defs: HashMap<String, ActorDef>,
    /// Active generic type argument substitutions for interpreted generic functions.
    type_arg_scopes: Vec<HashMap<String, TypeExpr>>,
    /// Live actor instances keyed by unique ID.
    actor_instances: HashMap<u64, ActorInstance>,
    /// Next actor instance ID.
    next_actor_id: u64,
    /// Recorded debug output lines (`trace`, `breakpoint`).
    debug_output: Vec<String>,
    /// Whether debug output should print as the program runs.
    emit_runtime_debug: bool,
}

/// Runtime state of a spawned actor instance.
struct ActorInstance {
    /// Name of the actor type (e.g. `"Counter"`).
    type_name: String,
    /// Current values of the actor's mutable state fields.
    state: HashMap<String, Value>,
    /// Capability values passed at spawn time.
    capabilities: HashMap<String, Value>,
}

impl Interpreter {
    /// Create a new interpreter with an empty global scope.
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            functions: HashMap::new(),
            structs: HashMap::new(),
            bitfields: HashMap::new(),
            enums: HashMap::new(),
            interface_methods: HashMap::new(),
            type_alias_bases: HashMap::new(),
            type_aliases: HashMap::new(),
            machines: HashMap::new(),
            actor_defs: HashMap::new(),
            type_arg_scopes: Vec::new(),
            actor_instances: HashMap::new(),
            next_actor_id: 0,
            debug_output: Vec::new(),
            emit_runtime_debug: false,
        }
    }

    /// Create an interpreter that emits debug output during execution.
    pub fn new_runtime() -> Self {
        let mut interp = Self::new();
        interp.emit_runtime_debug = true;
        interp
    }

    /// Drain any debug lines recorded so far.
    pub fn take_debug_output(&mut self) -> Vec<String> {
        std::mem::take(&mut self.debug_output)
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

    fn emit_debug_line(&mut self, line: String) {
        self.debug_output.push(line.clone());
        if self.emit_runtime_debug {
            println!("{line}");
        }
    }

    fn trace_variable(&mut self, name: &str) -> Result<(), String> {
        let value = self
            .get_variable(name)
            .cloned()
            .ok_or_else(|| format!("undefined variable '{name}'"))?;
        self.emit_debug_line(format!("trace {name} = {value}"));
        Ok(())
    }

    fn hit_breakpoint(&mut self) {
        let mut bindings = BTreeMap::new();
        for scope in &self.scopes {
            for (name, value) in scope {
                bindings.insert(name.clone(), value.clone());
            }
        }

        if bindings.is_empty() {
            self.emit_debug_line("breakpoint hit".to_string());
            return;
        }

        let fields: Vec<String> = bindings
            .into_iter()
            .map(|(name, value)| format!("{name} = {value}"))
            .collect();
        self.emit_debug_line(format!("breakpoint hit: {}", fields.join(", ")));
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

    /// Register a user-defined actor so it can be spawned and messaged.
    pub fn register_actor(&mut self, actor: &ActorDef) {
        self.actor_defs
            .insert(actor.name.name.clone(), actor.clone());
    }

    /// Register a user-defined bitfield so it can be constructed and its
    /// fields can be read like a struct value.
    pub fn register_bitfield(&mut self, bitfield: &BitfieldDef) {
        self.bitfields
            .insert(bitfield.name.name.clone(), bitfield.clone());
    }

    /// Register an enum definition so runtime bitfield conversions can map
    /// between stored integers and named variants.
    pub fn register_enum(&mut self, enm: &EnumDef) {
        self.enums.insert(enm.name.name.clone(), enm.clone());
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
        let base_ty = alias.base_type.clone();
        let base_name = type_expr_name(&base_ty);
        self.type_alias_bases
            .insert(alias.name.name.clone(), base_ty);
        let def = alias.constraint.as_ref().map(|c| RefinementDef {
            base_type_name: base_name,
            constraint: c.clone(),
        });
        self.type_aliases.insert(alias.name.name.clone(), def);
    }

    /// Check a value against a refinement type's constraint.
    /// Returns `Ok(())` if valid, or `Err(message)` if the constraint fails.
    fn check_refinement(&mut self, type_name: &str, value: &Value) -> Result<(), String> {
        if let Some(base_ty) = self.type_alias_bases.get(type_name).cloned() {
            let base_type_name = type_expr_name(&base_ty);
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

    fn finish_refinement_boundary(
        &mut self,
        type_name: &str,
        value: Value,
        bind_name: Option<&Ident>,
        body: &Block,
    ) -> Result<ExprFlow, String> {
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
                .is_some_and(|base| self.type_name_has_refinement(&type_expr_name(base))),
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
            ExprFlow::Signal(Signal::Respond(_)) => {
                Err("`respond` cannot escape expression evaluation".to_string())
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
            Expr::Ident(ident) => {
                if let Some(val) = self.get_variable(&ident.name).cloned() {
                    Ok(ExprFlow::Value(val))
                } else if let Some(func) = self.functions.get(&ident.name).cloned() {
                    // Named function reference — wrap as a function value.
                    Ok(ExprFlow::Value(Value::Function {
                        params: func.params.clone(),
                        body: func.body.clone(),
                        captures: HashMap::new(),
                    }))
                } else {
                    Err(format!("undefined variable '{}'", ident.name))
                }
            }

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
            Expr::Call(callee, args, _) => self.eval_call_flow(callee, &[], args),
            Expr::GenericCall(callee, type_args, args, _) => {
                self.eval_call_flow(callee, type_args, args)
            }

            // List construction
            Expr::ListConstruct(elems, _) => {
                let mut vals = Vec::with_capacity(elems.len());
                for elem in elems {
                    vals.push(value_or_signal!(self, elem));
                }
                Ok(ExprFlow::Value(Value::List(vals)))
            }

            Expr::MapConstruct(entries, _) => {
                let mut pairs = Vec::with_capacity(entries.len());
                for (key_expr, val_expr) in entries {
                    let k = value_or_signal!(self, key_expr);
                    let v = value_or_signal!(self, val_expr);
                    pairs.push((k, v));
                }
                Ok(ExprFlow::Value(Value::Map(pairs)))
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

            Expr::Clone(inner, _) => {
                // `clone expr` — deep clone (all Values are Clone so this is a no-op copy).
                self.eval_expr_flow(inner)
            }

            Expr::Spawn(inner, _) => {
                // `spawn ActorType(cap1: val1, ...)` — create a new actor instance.
                let (actor_name, args) = match inner.as_ref() {
                    Expr::Call(callee, args, _) => match callee.as_ref() {
                        Expr::Ident(ident) => (ident.name.clone(), args),
                        _ => return Err("spawn: expected actor type name".to_string()),
                    },
                    _ => return Err("spawn: expected call expression".to_string()),
                };

                let actor_def = self
                    .actor_defs
                    .get(&actor_name)
                    .ok_or_else(|| format!("unknown actor type '{actor_name}'"))?
                    .clone();

                // Evaluate capability args.
                let mut capabilities = HashMap::new();
                for (arg, param) in args.iter().zip(actor_def.capability_params.iter()) {
                    let val = value_or_signal!(self, &arg.value);
                    let name = arg
                        .name
                        .as_ref()
                        .map(|n| n.name.clone())
                        .unwrap_or_else(|| param.name.name.clone());
                    capabilities.insert(name, val);
                }

                // Evaluate state field initializers in a temp scope with capabilities in scope.
                self.push_scope();
                for (name, val) in &capabilities {
                    self.set_variable(name, val.clone());
                }
                let mut state = HashMap::new();
                for field in &actor_def.state_fields {
                    let val = value_or_signal!(self, &field.value);
                    state.insert(field.name.name.clone(), val);
                }
                self.pop_scope();

                let id = self.next_actor_id;
                self.next_actor_id += 1;
                self.actor_instances.insert(
                    id,
                    ActorInstance {
                        type_name: actor_name.clone(),
                        state,
                        capabilities,
                    },
                );

                Ok(ExprFlow::Value(Value::Actor(id)))
            }

            Expr::Send(inner, _) => {
                self.eval_actor_message(inner, false)?;
                Ok(ExprFlow::Value(Value::Nothing))
            }

            Expr::Ask(inner, _) => {
                let val = self.eval_actor_message(inner, true)?;
                Ok(ExprFlow::Value(val))
            }

            // Structured concurrency — sequential simulation:
            // `run call` evaluates immediately and wraps in Pending.
            Expr::Run(inner, _) => {
                let val = value_or_signal!(self, inner);
                Ok(ExprFlow::Value(Value::Pending(Box::new(val))))
            }

            // `join pending` unwraps the Pending, returning result[T, error]
            // so that a `handle error:` block can handle failures.
            Expr::Join(inner, _) => {
                let val = value_or_signal!(self, inner);
                let result = match val {
                    Value::Pending(inner_val) => match *inner_val {
                        Value::ResultOk(_) | Value::ResultFail(_) => *inner_val,
                        other => Value::ResultOk(Box::new(other)),
                    },
                    Value::Nothing => {
                        Value::ResultFail(Box::new(Value::String("task was cancelled".to_string())))
                    }
                    other => Value::ResultOk(Box::new(other)),
                };
                Ok(ExprFlow::Value(result))
            }

            // `cancel task` — in the sequential simulation this is a no-op;
            // the task has already completed.
            Expr::Cancel(inner, _) => {
                value_or_signal!(self, inner);
                Ok(ExprFlow::Value(Value::Nothing))
            }

            Expr::InlineFn(params, _return_type, body, _) => {
                // Capture the current environment (all visible variables) for closure semantics.
                let mut captures = HashMap::new();
                for scope in &self.scopes {
                    for (name, value) in scope {
                        captures.insert(name.clone(), value.clone());
                    }
                }
                Ok(ExprFlow::Value(Value::Function {
                    params: params.clone(),
                    body: body.clone(),
                    captures,
                }))
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
    fn eval_call_flow(
        &mut self,
        callee: &Expr,
        type_args: &[TypeExpr],
        args: &[CallArg],
    ) -> Result<ExprFlow, String> {
        // Check for machine construction/transition BEFORE evaluating args,
        // since state-name arguments are bare identifiers (not variables) and
        // would fail evaluation.
        match callee {
            Expr::Ident(ident) if self.structs.contains_key(&ident.name) => {}
            Expr::Ident(ident) if self.machines.contains_key(&ident.name) => {
                return Ok(ExprFlow::Value(self.construct_machine(&ident.name, args)?));
            }
            Expr::FieldAccess(obj, field, _) => {
                if let Expr::Ident(ident) = obj.as_ref() {
                    if field.name == "transition" && self.machines.contains_key(&ident.name) {
                        return Ok(ExprFlow::Value(self.machine_transition(&ident.name, args)?));
                    }
                }
            }
            _ => {}
        }

        let mut arg_values = Vec::with_capacity(args.len());
        for arg in args {
            arg_values.push(value_or_signal!(self, &arg.value));
        }

        match callee {
            Expr::Ident(ident) if self.structs.contains_key(&ident.name) => Ok(ExprFlow::Value(
                self.construct_struct(&ident.name, args, arg_values)?,
            )),
            Expr::Ident(ident) if self.bitfields.contains_key(&ident.name) => Ok(ExprFlow::Value(
                self.construct_bitfield(&ident.name, args, arg_values)?,
            )),
            Expr::Ident(ident) => Ok(ExprFlow::Value(self.call_function_with_type_args(
                &ident.name,
                type_args,
                arg_values,
            )?)),
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
                    // Try higher-order built-ins first (require &mut self).
                    if let Some(result) = self.call_higher_order_builtin(name, arg_values.clone()) {
                        return Ok(ExprFlow::Value(result?));
                    }
                    // Try type-reflection built-ins before ordinary built-ins.
                    if let Some(result) =
                        self.call_builtin_with_type_args(name, type_args, &arg_values)
                    {
                        return Ok(ExprFlow::Value(result?));
                    }
                    if let Some(result) = self.call_builtin(name, &arg_values) {
                        return Ok(ExprFlow::Value(result?));
                    }
                    // Try user-defined dotted functions.
                    if self.functions.contains_key(name.as_str())
                        || self.resolve_interface_dispatch(name, &arg_values).is_some()
                    {
                        return Ok(ExprFlow::Value(
                            self.call_function_with_type_args(name, type_args, arg_values)?,
                        ));
                    }
                }
                // Fall through to enum variant construction if no built-in or
                // user function matched.
                if let Expr::Ident(ident) = obj.as_ref() {
                    return Ok(ExprFlow::Value(Value::Enum {
                        type_name: ident.name.clone(),
                        variant: field.name.clone(),
                        fields: arg_values,
                    }));
                }
                match dotted {
                    Some(name) => Ok(ExprFlow::Value(
                        self.call_function_with_type_args(&name, type_args, arg_values)?,
                    )),
                    None => Err("only named function calls are supported in comptime".to_string()),
                }
            }
            _ => Err("only named function calls are supported in comptime".to_string()),
        }
    }

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
                    // Check higher-order builtins first (need &mut self).
                    if let Some(result) = self.call_higher_order_builtin(name, arg_values.clone()) {
                        return Ok(ExprFlow::Value(result?));
                    }
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
                            let target_value = match self.eval_expr_flow(target)? {
                                ExprFlow::Value(value) => value,
                                ExprFlow::Signal(signal) => return Ok(Some(signal)),
                            };
                            let flow = match target_value {
                                Value::ResultOk(value) | Value::OptionalSome(value) => self
                                    .finish_refinement_boundary(
                                        &type_name,
                                        *value,
                                        bind_name.as_ref(),
                                        body,
                                    )?,
                                Value::ResultFail(error) => {
                                    self.exec_handle_block(bind_name.as_ref(), Some(*error), body)?
                                }
                                Value::OptionalNone => self.exec_handle_block(None, None, body)?,
                                value => self.finish_refinement_boundary(
                                    &type_name,
                                    value,
                                    bind_name.as_ref(),
                                    body,
                                )?,
                            };
                            match flow {
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
                    Value::String(s) => {
                        for ch in s.chars() {
                            self.push_scope();
                            self.set_variable(
                                &for_stmt.variable.name,
                                Value::String(ch.to_string()),
                            );
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
                    Value::Map(entries) => {
                        for (key, val) in entries {
                            self.push_scope();
                            self.set_variable(&for_stmt.variable.name, key);
                            if let Some(ref val_var) = for_stmt.value_variable {
                                self.set_variable(&val_var.name, val);
                            }
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
                    Value::Set(items) => {
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
                    _ => {
                        return Err(
                            "for loop requires a list, string, map, or set value".to_string()
                        );
                    }
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

            Stmt::Trace(trace_stmt) => {
                self.trace_variable(&trace_stmt.name.name)?;
                Ok(None)
            }

            Stmt::Breakpoint(breakpoint_stmt) => {
                let should_break = if let Some(condition) = &breakpoint_stmt.condition {
                    match self.eval_expr_flow(condition)? {
                        ExprFlow::Value(Value::Bool(value)) => value,
                        ExprFlow::Value(other) => {
                            return Err(format!("breakpoint condition must be bool, got {other}"));
                        }
                        ExprFlow::Signal(signal) => return Ok(Some(signal)),
                    }
                } else {
                    true
                };

                if should_break {
                    self.hit_breakpoint();
                }
                Ok(None)
            }

            Stmt::Respond(resp) => {
                let val = self.eval_expr(&resp.value)?;
                Ok(Some(Signal::Respond(val)))
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
            Some(Signal::Respond(_)) => {
                Err("`respond` can only be used inside a `receive` handler".to_string())
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

    /// Execute an actor message (send or ask).
    ///
    /// `inner` is the expression after the `send`/`ask` keyword:
    ///   - `actor_expr.handler_name`   (no args)
    ///   - `actor_expr.handler_name(args...)` (with args)
    ///
    /// When `is_ask` is true, a `respond value` inside the handler returns
    /// `value` as the result; for `send` the result is always `nothing`.
    fn eval_actor_message(&mut self, inner: &Expr, is_ask: bool) -> Result<Value, String> {
        // Decompose `actor_expr.handler_name` or `actor_expr.handler_name(args)`.
        let (actor_expr, handler_name, call_args) = match inner {
            Expr::Call(callee, args, _) => match callee.as_ref() {
                Expr::FieldAccess(base, field, _) => {
                    (base.as_ref(), field.name.clone(), Some(args.as_slice()))
                }
                _ => {
                    return Err(
                        "send/ask: expected actor.handler or actor.handler(args)".to_string()
                    );
                }
            },
            Expr::FieldAccess(base, field, _) => (base.as_ref(), field.name.clone(), None),
            _ => return Err("send/ask: expected actor.handler expression".to_string()),
        };

        // Evaluate actor handle.
        let actor_val = match self.eval_expr_flow(actor_expr)? {
            ExprFlow::Value(v) => v,
            ExprFlow::Signal(s) => {
                return Err(format!("send/ask: actor expression returned signal: {s:?}"));
            }
        };
        let actor_id = match actor_val {
            Value::Actor(id) => id,
            _ => return Err(format!("send/ask: expected actor value, got {actor_val}")),
        };

        // Evaluate message arguments.
        let mut arg_values = Vec::new();
        if let Some(args) = call_args {
            for arg in args {
                let val = match self.eval_expr_flow(&arg.value)? {
                    ExprFlow::Value(v) => v,
                    ExprFlow::Signal(s) => {
                        return Err(format!("send/ask: arg expression returned signal: {s:?}"));
                    }
                };
                arg_values.push(val);
            }
        }

        // Clone the actor def and instance state to avoid borrow conflicts.
        let instance = self
            .actor_instances
            .get(&actor_id)
            .ok_or_else(|| format!("unknown actor instance #{actor_id}"))?;
        let type_name = instance.type_name.clone();
        let state_snapshot = instance.state.clone();
        let caps_snapshot = instance.capabilities.clone();

        let actor_def = self
            .actor_defs
            .get(&type_name)
            .ok_or_else(|| format!("unknown actor type '{type_name}'"))?
            .clone();

        let handler = actor_def
            .handlers
            .iter()
            .find(|h| h.name.name == handler_name)
            .ok_or_else(|| format!("actor '{type_name}' has no handler '{handler_name}'"))?
            .clone();

        // Execute handler body in a new scope with state + caps + params.
        self.push_scope();
        for (name, val) in &state_snapshot {
            self.set_variable(name, val.clone());
        }
        for (name, val) in &caps_snapshot {
            self.set_variable(name, val.clone());
        }
        for (param, val) in handler.params.iter().zip(arg_values.iter()) {
            self.set_variable(&param.name.name, val.clone());
        }

        // Execute the handler body, collecting signals.
        let mut respond_value = Value::Nothing;
        for stmt in &handler.body.stmts {
            match self.exec_stmt_inner(stmt)? {
                Some(Signal::Respond(val)) => {
                    respond_value = val;
                    break;
                }
                Some(Signal::Return(_)) => break,
                Some(Signal::Break) | Some(Signal::Continue) => break,
                Some(Signal::Default(_)) => break,
                None => {}
            }
        }

        // Collect updated state field values before popping scope.
        let state_field_names: Vec<String> = state_snapshot.keys().cloned().collect();
        let mut updated_state = state_snapshot;
        for name in &state_field_names {
            // Check innermost scope(s) for the updated value.
            if let Some(val) = self.scopes.last().and_then(|s| s.get(name)).cloned() {
                updated_state.insert(name.clone(), val);
            }
        }

        self.pop_scope();

        // Write updated state back to the actor instance.
        if let Some(instance) = self.actor_instances.get_mut(&actor_id) {
            instance.state = updated_state;
        }

        if is_ask {
            Ok(respond_value)
        } else {
            Ok(Value::Nothing)
        }
    }

    fn call_bitfield_builtin(&self, name: &str, args: &[Value]) -> Option<Result<Value, String>> {
        let (bitfield_name, method_name) = name.rsplit_once('.')?;
        if !self.bitfields.contains_key(bitfield_name) {
            return None;
        }

        match method_name {
            "to_bytes" => Some(self.bitfield_to_bytes(bitfield_name, args)),
            "from_bytes" => Some(self.bitfield_from_bytes(bitfield_name, args)),
            _ => None,
        }
    }

    fn bitfield_to_bytes(&self, bitfield_name: &str, args: &[Value]) -> Result<Value, String> {
        if args.len() != 1 {
            return Err(format!(
                "{}.to_bytes expects 1 argument(s), got {}",
                bitfield_name,
                args.len()
            ));
        }
        let bitfield = self
            .bitfields
            .get(bitfield_name)
            .ok_or_else(|| format!("undefined bitfield '{bitfield_name}'"))?;
        let bytes = self.encode_bitfield_value(bitfield, &args[0])?;
        Ok(Value::Bytes(bytes))
    }

    fn bitfield_from_bytes(&self, bitfield_name: &str, args: &[Value]) -> Result<Value, String> {
        if args.len() != 1 {
            return Err(format!(
                "{}.from_bytes expects 1 argument(s), got {}",
                bitfield_name,
                args.len()
            ));
        }
        let bitfield = self
            .bitfields
            .get(bitfield_name)
            .ok_or_else(|| format!("undefined bitfield '{bitfield_name}'"))?;

        let Value::Bytes(bytes) = &args[0] else {
            return Err(format!(
                "{bitfield_name}.from_bytes expects a bytes argument"
            ));
        };

        match self.decode_bitfield_value(bitfield, bytes) {
            Ok(value) => Ok(Value::ResultOk(Box::new(value))),
            Err(message) => Ok(Value::ResultFail(Box::new(Value::String(message)))),
        }
    }

    fn encode_bitfield_value(
        &self,
        bitfield: &BitfieldDef,
        value: &Value,
    ) -> Result<Vec<u8>, String> {
        let Value::Struct { type_name, fields } = value else {
            return Err(format!(
                "{}.to_bytes expects a {} value",
                bitfield.name.name, bitfield.name.name
            ));
        };
        if type_name != &bitfield.name.name {
            return Err(format!(
                "{}.to_bytes expects a {} value, got {}",
                bitfield.name.name, bitfield.name.name, type_name
            ));
        }

        let mut bits = Vec::new();
        for field in &bitfield.fields {
            let (_, field_value) = fields
                .iter()
                .find(|(name, _)| name == &field.name.name)
                .ok_or_else(|| {
                    format!(
                        "bitfield '{}' is missing field '{}'",
                        bitfield.name.name, field.name.name
                    )
                })?;

            match &field.kind {
                BitfieldFieldKind::Bits { width, as_type } => {
                    let numeric = self.bitfield_field_numeric_value(
                        bitfield,
                        &field.name.name,
                        *width,
                        as_type.as_ref(),
                        field_value,
                    )?;
                    let byte_aligned = bits.len() % 8 == 0;
                    self.push_encoded_bits(
                        &mut bits,
                        numeric,
                        *width,
                        bitfield.network_order,
                        byte_aligned,
                    );
                }
                BitfieldFieldKind::Payload(_) => {
                    if bits.len() % 8 != 0 {
                        return Err(format!(
                            "bitfield '{}' payload field '{}' must begin on a byte boundary",
                            bitfield.name.name, field.name.name
                        ));
                    }
                    let payload = self.value_to_byte_list(field_value).map_err(|message| {
                        format!(
                            "bitfield '{}' field '{}': {}",
                            bitfield.name.name, field.name.name, message
                        )
                    })?;
                    let mut bytes = Self::bits_to_bytes(&bits);
                    bytes.extend(payload);
                    return Ok(bytes);
                }
            }
        }

        Ok(Self::bits_to_bytes(&bits))
    }

    fn decode_bitfield_value(&self, bitfield: &BitfieldDef, bytes: &[u8]) -> Result<Value, String> {
        let mut bit_index = 0usize;
        let mut fields = Vec::with_capacity(bitfield.fields.len());

        for field in &bitfield.fields {
            match &field.kind {
                BitfieldFieldKind::Bits { width, as_type } => {
                    let numeric = if *width > 8
                        && width % 8 == 0
                        && !bitfield.network_order
                        && bit_index % 8 == 0
                    {
                        let byte_count = (*width as usize) / 8;
                        let start = bit_index / 8;
                        let end = start + byte_count;
                        if end > bytes.len() {
                            return Err(format!(
                                "bitfield '{}.from_bytes' expected at least {} byte(s), got {}",
                                bitfield.name.name,
                                end,
                                bytes.len()
                            ));
                        }
                        bit_index += *width as usize;
                        let mut value = 0u64;
                        for (shift, byte) in bytes[start..end].iter().enumerate() {
                            value |= (*byte as u64) << (shift * 8);
                        }
                        value
                    } else {
                        Self::read_bits(bytes, &mut bit_index, *width).ok_or_else(|| {
                            format!(
                                "bitfield '{}.from_bytes' expected {} bit(s), got {} byte(s)",
                                bitfield.name.name,
                                bit_index + (*width as usize),
                                bytes.len()
                            )
                        })?
                    };

                    let value = if let Some(enum_ty) = as_type {
                        let enum_name = type_expr_name(enum_ty);
                        self.enum_value_from_numeric(&enum_name, numeric)
                            .map_err(|message| {
                                format!(
                                    "bitfield '{}' field '{}': {}",
                                    bitfield.name.name, field.name.name, message
                                )
                            })?
                    } else {
                        Value::Int64(numeric as i64)
                    };
                    fields.push((field.name.name.clone(), value));
                }
                BitfieldFieldKind::Payload(_) => {
                    if bit_index % 8 != 0 {
                        return Err(format!(
                            "bitfield '{}' payload field '{}' must begin on a byte boundary",
                            bitfield.name.name, field.name.name
                        ));
                    }
                    let start = bit_index / 8;
                    let payload = bytes[start..]
                        .iter()
                        .map(|byte| Value::Int64(*byte as i64))
                        .collect();
                    bit_index = bytes.len() * 8;
                    fields.push((field.name.name.clone(), Value::List(payload)));
                }
            }
        }

        let consumed_bytes = bit_index.div_ceil(8);
        if consumed_bytes != bytes.len() {
            return Err(format!(
                "bitfield '{}.from_bytes' expected {} byte(s), got {}",
                bitfield.name.name,
                consumed_bytes,
                bytes.len()
            ));
        }

        Ok(Value::Struct {
            type_name: bitfield.name.name.clone(),
            fields,
        })
    }

    fn bitfield_field_numeric_value(
        &self,
        bitfield: &BitfieldDef,
        field_name: &str,
        width: u16,
        as_type: Option<&TypeExpr>,
        value: &Value,
    ) -> Result<u64, String> {
        if let Some(enum_ty) = as_type {
            let enum_name = type_expr_name(enum_ty);
            let Value::Enum {
                type_name,
                variant,
                fields,
            } = value
            else {
                return Err(format!(
                    "field '{}' expects enum '{}'",
                    field_name, enum_name
                ));
            };
            if !fields.is_empty() {
                return Err(format!(
                    "field '{}' enum '{}' must use unit variants",
                    field_name, enum_name
                ));
            }
            if type_name != &enum_name {
                return Err(format!(
                    "field '{}' expects enum '{}', got '{}'",
                    field_name, enum_name, type_name
                ));
            }
            let numeric = self.enum_numeric_value(type_name, variant)?;
            if !Self::fits_in_bits(numeric, width) {
                return Err(format!(
                    "bitfield '{}' field '{}' is {} bit(s) wide and cannot hold enum variant '{}.{}'",
                    bitfield.name.name, field_name, width, type_name, variant
                ));
            }
            return Ok(numeric);
        }

        let Value::Int64(int_value) = value else {
            return Err(format!("field '{}' expects int64", field_name));
        };
        if *int_value < 0 || !Self::fits_in_bits(*int_value as u64, width) {
            return Err(format!(
                "bitfield '{}' field '{}' is {} bit(s) wide and cannot hold '{}'",
                bitfield.name.name, field_name, width, int_value
            ));
        }
        Ok(*int_value as u64)
    }

    fn enum_numeric_value(&self, enum_name: &str, variant_name: &str) -> Result<u64, String> {
        let enm = self
            .enums
            .get(enum_name)
            .ok_or_else(|| format!("unknown enum '{}'", enum_name))?;
        let mut next_discriminant = 0_i64;
        for variant in &enm.variants {
            let discriminant = variant.discriminant.unwrap_or(next_discriminant);
            next_discriminant = discriminant.saturating_add(1);
            if variant.name.name == variant_name {
                if discriminant < 0 {
                    return Err(format!(
                        "enum '{}.{}' has negative discriminant {}",
                        enum_name, variant_name, discriminant
                    ));
                }
                return Ok(discriminant as u64);
            }
        }
        Err(format!(
            "enum '{}' has no variant '{}'",
            enum_name, variant_name
        ))
    }

    fn enum_value_from_numeric(&self, enum_name: &str, numeric: u64) -> Result<Value, String> {
        let enm = self
            .enums
            .get(enum_name)
            .ok_or_else(|| format!("unknown enum '{}'", enum_name))?;
        let mut next_discriminant = 0_i64;
        for variant in &enm.variants {
            let discriminant = variant.discriminant.unwrap_or(next_discriminant);
            next_discriminant = discriminant.saturating_add(1);
            if discriminant >= 0 && discriminant as u64 == numeric {
                return Ok(Value::Enum {
                    type_name: enum_name.to_string(),
                    variant: variant.name.name.clone(),
                    fields: vec![],
                });
            }
        }
        Err(format!(
            "enum '{}' has no variant for value {}",
            enum_name, numeric
        ))
    }

    fn value_to_byte_list(&self, value: &Value) -> Result<Vec<u8>, String> {
        match value {
            Value::Bytes(bytes) => Ok(bytes.clone()),
            Value::List(items) => items
                .iter()
                .map(|item| match item {
                    Value::Int64(value) if (0..=255).contains(value) => Ok(*value as u8),
                    Value::Int64(value) => Err(format!("byte value out of range: {}", value)),
                    other => Err(format!("payload expects list[uint8], found {}", other)),
                })
                .collect(),
            other => Err(format!(
                "payload expects list[uint8] or bytes, found {}",
                other
            )),
        }
    }

    fn fits_in_bits(value: u64, width: u16) -> bool {
        if width >= 64 {
            true
        } else {
            value < (1_u64 << width)
        }
    }

    fn push_encoded_bits(
        &self,
        bits: &mut Vec<bool>,
        value: u64,
        width: u16,
        network_order: bool,
        byte_aligned: bool,
    ) {
        if width > 8 && width % 8 == 0 && !network_order && byte_aligned {
            let byte_count = (width / 8) as usize;
            for byte_index in 0..byte_count {
                let byte = ((value >> (byte_index * 8)) & 0xFF) as u8;
                for bit_shift in (0..8).rev() {
                    bits.push(((byte >> bit_shift) & 1) == 1);
                }
            }
            return;
        }

        for bit_shift in (0..width).rev() {
            bits.push(((value >> bit_shift) & 1) == 1);
        }
    }

    fn bits_to_bytes(bits: &[bool]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(bits.len().div_ceil(8));
        for chunk in bits.chunks(8) {
            let mut byte = 0u8;
            for (index, bit) in chunk.iter().enumerate() {
                if *bit {
                    byte |= 1 << (7 - index);
                }
            }
            bytes.push(byte);
        }
        bytes
    }

    fn read_bits(bytes: &[u8], bit_index: &mut usize, width: u16) -> Option<u64> {
        let mut value = 0u64;
        for _ in 0..width {
            let byte_index = *bit_index / 8;
            if byte_index >= bytes.len() {
                return None;
            }
            let bit_in_byte = 7 - (*bit_index % 8);
            let bit = (bytes[byte_index] >> bit_in_byte) & 1;
            value = (value << 1) | (bit as u64);
            *bit_index += 1;
        }
        Some(value)
    }

    fn call_builtin_with_type_args(
        &mut self,
        name: &str,
        type_args: &[TypeExpr],
        args: &[Value],
    ) -> Option<Result<Value, String>> {
        let is_typed_builtin = matches!(
            name,
            "type.name"
                | "type.kind"
                | "type.has_secret"
                | "type.info"
                | "type.fields"
                | "type.variants"
                | "type.field_value"
                | "json.parse"
                | "json.serialize"
                | "json.serialize_public"
        );
        if !is_typed_builtin {
            return None;
        }
        let expected_type_arg_count = if name == "type.field_value" { 2 } else { 1 };
        if type_args.len() != expected_type_arg_count {
            return Some(Err(format!(
                "{name} expects {expected_type_arg_count} type argument(s), got {}",
                type_args.len()
            )));
        }

        let ty = self.substitute_type_expr(&type_args[0]);
        Some(match name {
            "type.name" => {
                if let Some(err) = check_args(name, 0, args) {
                    return Some(err);
                }
                Ok(Value::String(type_expr_display(&ty)))
            }
            "type.kind" => {
                if let Some(err) = check_args(name, 0, args) {
                    return Some(err);
                }
                Ok(Value::String(self.type_expr_kind(&ty).to_string()))
            }
            "type.has_secret" => {
                if let Some(err) = check_args(name, 0, args) {
                    return Some(err);
                }
                Ok(Value::Bool(self.type_expr_has_secret(&ty)))
            }
            "type.info" => {
                if let Some(err) = check_args(name, 0, args) {
                    return Some(err);
                }
                Ok(self.type_info_value(&ty))
            }
            "type.fields" => {
                if let Some(err) = check_args(name, 0, args) {
                    return Some(err);
                }
                Ok(Value::List(
                    self.type_expr_fields(&ty)
                        .into_iter()
                        .enumerate()
                        .map(|(index, field)| self.type_field_value(index, field))
                        .collect(),
                ))
            }
            "type.variants" => {
                if let Some(err) = check_args(name, 0, args) {
                    return Some(err);
                }
                Ok(Value::List(
                    self.type_expr_variants(&ty)
                        .into_iter()
                        .enumerate()
                        .map(|(index, variant)| self.type_variant_value(index, variant))
                        .collect(),
                ))
            }
            "type.field_value" => {
                if let Some(err) = check_args(name, 2, args) {
                    return Some(err);
                }
                let expected_field_ty = self.substitute_type_expr(&type_args[1]);
                self.reflected_field_value(&args[0], &ty, &args[1], &expected_field_ty)
            }
            "json.parse" => {
                if let Some(err) = check_args(name, 1, args) {
                    return Some(err);
                }
                match &args[0] {
                    Value::String(raw) => match serde_json::from_str::<JsonValue>(raw) {
                        Ok(json) => match self.json_to_value_typed(&json, &ty) {
                            Ok(value) => Ok(Value::ResultOk(Box::new(value))),
                            Err(message) => Ok(Value::ResultFail(Box::new(Value::String(message)))),
                        },
                        Err(err) => Ok(Value::ResultFail(Box::new(Value::String(err.to_string())))),
                    },
                    other => Err(format!("json.parse expects a string, got {other}")),
                }
            }
            "json.serialize" | "json.serialize_public" => {
                if let Some(err) = check_args(name, 1, args) {
                    return Some(err);
                }
                Ok(Value::String(self.value_to_json_typed(
                    &args[0],
                    &ty,
                    name == "json.serialize_public",
                )))
            }
            _ => return None,
        })
    }

    fn current_type_binding(&self, name: &str) -> Option<TypeExpr> {
        self.type_arg_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }

    fn substitute_type_expr(&self, ty: &TypeExpr) -> TypeExpr {
        match ty {
            TypeExpr::Named(ident) => self
                .current_type_binding(&ident.name)
                .unwrap_or_else(|| ty.clone()),
            TypeExpr::Generic(ident, args, span) => TypeExpr::Generic(
                ident.clone(),
                args.iter()
                    .map(|arg| self.substitute_type_expr(arg))
                    .collect(),
                *span,
            ),
            TypeExpr::View(inner, span) => {
                TypeExpr::View(Box::new(self.substitute_type_expr(inner)), *span)
            }
            TypeExpr::Function(params, return_type, span) => TypeExpr::Function(
                params
                    .iter()
                    .map(|param| self.substitute_type_expr(param))
                    .collect(),
                Box::new(self.substitute_type_expr(return_type)),
                *span,
            ),
        }
    }

    fn substitute_type_expr_with_map(
        &self,
        ty: &TypeExpr,
        substitutions: &HashMap<String, TypeExpr>,
    ) -> TypeExpr {
        match ty {
            TypeExpr::Named(ident) => substitutions
                .get(&ident.name)
                .cloned()
                .or_else(|| self.current_type_binding(&ident.name))
                .unwrap_or_else(|| ty.clone()),
            TypeExpr::Generic(ident, args, span) => TypeExpr::Generic(
                ident.clone(),
                args.iter()
                    .map(|arg| self.substitute_type_expr_with_map(arg, substitutions))
                    .collect(),
                *span,
            ),
            TypeExpr::View(inner, span) => TypeExpr::View(
                Box::new(self.substitute_type_expr_with_map(inner, substitutions)),
                *span,
            ),
            TypeExpr::Function(params, return_type, span) => TypeExpr::Function(
                params
                    .iter()
                    .map(|param| self.substitute_type_expr_with_map(param, substitutions))
                    .collect(),
                Box::new(self.substitute_type_expr_with_map(return_type, substitutions)),
                *span,
            ),
        }
    }

    fn type_expr_kind(&self, ty: &TypeExpr) -> &'static str {
        match ty {
            TypeExpr::Named(ident) => {
                if let Some(bound) = self.current_type_binding(&ident.name) {
                    return self.type_expr_kind(&bound);
                }
                if matches!(
                    ident.name.as_str(),
                    "int64" | "float64" | "string" | "bool" | "bytes" | "nothing"
                ) {
                    "primitive"
                } else if self.type_aliases.contains_key(&ident.name) {
                    if self
                        .type_aliases
                        .get(&ident.name)
                        .is_some_and(|def| def.is_some())
                    {
                        "refinement"
                    } else {
                        "alias"
                    }
                } else if self.structs.contains_key(&ident.name) {
                    "struct"
                } else if self.enums.contains_key(&ident.name) {
                    "enum"
                } else if self.bitfields.contains_key(&ident.name) {
                    "bitfield"
                } else {
                    "named"
                }
            }
            TypeExpr::Generic(ident, _, _) => {
                if self.structs.contains_key(&ident.name) {
                    "struct"
                } else {
                    match ident.name.as_str() {
                        "list" => "list",
                        "map" => "map",
                        "set" => "set",
                        "optional" => "optional",
                        "result" => "result",
                        "secret" => "secret",
                        other if self.structs.contains_key(other) => "struct",
                        _ => "generic",
                    }
                }
            }
            TypeExpr::View(inner, _) => self.type_expr_kind(inner),
            TypeExpr::Function(_, _, _) => "function",
        }
    }

    fn type_expr_has_secret(&self, ty: &TypeExpr) -> bool {
        let ty = self.substitute_type_expr(ty);
        self.type_expr_has_secret_inner(&ty, &mut HashSet::new())
    }

    fn type_expr_has_secret_inner(&self, ty: &TypeExpr, visited: &mut HashSet<String>) -> bool {
        match ty {
            TypeExpr::Named(ident) => {
                if let Some(bound) = self.current_type_binding(&ident.name) {
                    return self.type_expr_has_secret_inner(&bound, visited);
                }
                if let Some(base_ty) = self.type_alias_bases.get(&ident.name).cloned() {
                    return self
                        .type_expr_has_secret_inner(&self.substitute_type_expr(&base_ty), visited);
                }
                if !visited.insert(type_expr_display(ty)) {
                    return false;
                }
                if let Some(strukt) = self.structs.get(&ident.name) {
                    return strukt.fields.iter().any(|field| {
                        self.type_expr_has_secret_inner(
                            &self.substitute_type_expr(&field.ty),
                            visited,
                        )
                    });
                }
                if let Some(enum_def) = self.enums.get(&ident.name) {
                    return enum_def
                        .variants
                        .iter()
                        .flat_map(|variant| variant.fields.iter())
                        .any(|field| {
                            self.type_expr_has_secret_inner(
                                &self.substitute_type_expr(&field.ty),
                                visited,
                            )
                        });
                }
                if let Some(bitfield) = self.bitfields.get(&ident.name) {
                    return bitfield.fields.iter().any(|field| match &field.kind {
                        BitfieldFieldKind::Bits { as_type, .. } => as_type
                            .as_ref()
                            .is_some_and(|ty| self.type_expr_has_secret_inner(ty, visited)),
                        BitfieldFieldKind::Payload(ty) => {
                            self.type_expr_has_secret_inner(ty, visited)
                        }
                    });
                }
                false
            }
            TypeExpr::Generic(ident, args, _) => {
                if ident.name == "secret" {
                    return true;
                }
                if let Some(strukt) = self.structs.get(&ident.name) {
                    if !visited.insert(type_expr_display(ty)) {
                        return false;
                    }
                    let substitutions = self.generic_type_substitutions(strukt, args);
                    return strukt.fields.iter().any(|field| {
                        let field_ty =
                            self.substitute_type_expr_with_map(&field.ty, &substitutions);
                        self.type_expr_has_secret_inner(&field_ty, visited)
                    });
                }
                args.iter()
                    .any(|arg| self.type_expr_has_secret_inner(arg, visited))
            }
            TypeExpr::View(inner, _) => self.type_expr_has_secret_inner(inner, visited),
            TypeExpr::Function(params, return_type, _) => {
                params
                    .iter()
                    .any(|param| self.type_expr_has_secret_inner(param, visited))
                    || self.type_expr_has_secret_inner(return_type, visited)
            }
        }
    }

    fn type_expr_fields(&self, ty: &TypeExpr) -> Vec<ReflectionField> {
        match ty {
            TypeExpr::Named(ident) => {
                if let Some(strukt) = self.structs.get(&ident.name) {
                    return strukt
                        .fields
                        .iter()
                        .map(|field| ReflectionField {
                            name: field.name.name.clone(),
                            ty: self.substitute_type_expr(&field.ty),
                            serialize_name: field
                                .serialize_name
                                .clone()
                                .unwrap_or_else(|| field.name.name.clone()),
                        })
                        .collect();
                }
                if let Some(bitfield) = self.bitfields.get(&ident.name) {
                    return bitfield
                        .fields
                        .iter()
                        .map(|field| ReflectionField {
                            name: field.name.name.clone(),
                            ty: match &field.kind {
                                BitfieldFieldKind::Bits { as_type, .. } => {
                                    as_type.clone().unwrap_or_else(|| {
                                        TypeExpr::Named(Ident {
                                            name: "int64".to_string(),
                                            span: field.span,
                                        })
                                    })
                                }
                                BitfieldFieldKind::Payload(ty) => self.substitute_type_expr(ty),
                            },
                            serialize_name: field.name.name.clone(),
                        })
                        .collect();
                }
                Vec::new()
            }
            TypeExpr::Generic(ident, args, _) => self
                .structs
                .get(&ident.name)
                .map(|strukt| {
                    let substitutions = self.generic_type_substitutions(strukt, args);
                    strukt
                        .fields
                        .iter()
                        .map(|field| ReflectionField {
                            name: field.name.name.clone(),
                            ty: self.substitute_type_expr_with_map(&field.ty, &substitutions),
                            serialize_name: field
                                .serialize_name
                                .clone()
                                .unwrap_or_else(|| field.name.name.clone()),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            TypeExpr::View(inner, _) => self.type_expr_fields(inner),
            TypeExpr::Function(_, _, _) => Vec::new(),
        }
    }

    fn type_expr_variants(&self, ty: &TypeExpr) -> Vec<ReflectionVariant> {
        match ty {
            TypeExpr::Named(ident) => self
                .enums
                .get(&ident.name)
                .map(|enum_def| {
                    enum_def
                        .variants
                        .iter()
                        .map(|variant| ReflectionVariant {
                            name: variant.name.name.clone(),
                            fields: variant
                                .fields
                                .iter()
                                .map(|field| ReflectionField {
                                    name: field.name.name.clone(),
                                    ty: self.substitute_type_expr(&field.ty),
                                    serialize_name: field
                                        .serialize_name
                                        .clone()
                                        .unwrap_or_else(|| field.name.name.clone()),
                                })
                                .collect(),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            TypeExpr::View(inner, _) => self.type_expr_variants(inner),
            TypeExpr::Generic(_, _, _) | TypeExpr::Function(_, _, _) => Vec::new(),
        }
    }

    fn generic_type_substitutions(
        &self,
        strukt: &StructDef,
        args: &[TypeExpr],
    ) -> HashMap<String, TypeExpr> {
        strukt
            .type_params
            .iter()
            .zip(args.iter())
            .map(|(param, arg)| (param.name.clone(), self.substitute_type_expr(arg)))
            .collect()
    }

    fn type_field_value(&self, index: usize, field: ReflectionField) -> Value {
        let kind = self.type_expr_kind(&field.ty).to_string();
        let has_secret = self.type_expr_has_secret(&field.ty);
        let type_info = self.type_info_value(&field.ty);
        Value::Struct {
            type_name: "TypeField".to_string(),
            fields: vec![
                ("index".to_string(), Value::Int64(index as i64)),
                ("name".to_string(), Value::String(field.name)),
                (
                    "type_name".to_string(),
                    Value::String(type_expr_display(&field.ty)),
                ),
                ("kind".to_string(), Value::String(kind)),
                (
                    "serialize_name".to_string(),
                    Value::String(field.serialize_name),
                ),
                ("has_secret".to_string(), Value::Bool(has_secret)),
                ("type_info".to_string(), type_info),
            ],
        }
    }

    fn type_variant_value(&self, index: usize, variant: ReflectionVariant) -> Value {
        let has_secret = variant
            .fields
            .iter()
            .any(|field| self.type_expr_has_secret(&field.ty));
        let fields = variant
            .fields
            .into_iter()
            .enumerate()
            .map(|(index, field)| self.type_field_value(index, field))
            .collect::<Vec<_>>();

        Value::Struct {
            type_name: "TypeVariant".to_string(),
            fields: vec![
                ("index".to_string(), Value::Int64(index as i64)),
                ("name".to_string(), Value::String(variant.name)),
                ("has_secret".to_string(), Value::Bool(has_secret)),
                ("fields".to_string(), Value::List(fields)),
            ],
        }
    }

    fn type_info_value(&self, ty: &TypeExpr) -> Value {
        let ty = self.substitute_type_expr(ty);
        if let TypeExpr::View(inner, _) = &ty {
            return self.type_info_value(inner);
        }

        let arg_values = match &ty {
            TypeExpr::Named(ident) if self.type_aliases.contains_key(&ident.name) => self
                .type_alias_bases
                .get(&ident.name)
                .map(|base_ty| vec![self.type_info_value(base_ty)])
                .unwrap_or_default(),
            TypeExpr::Generic(_, args, _) => args
                .iter()
                .map(|arg| self.type_info_value(arg))
                .collect::<Vec<_>>(),
            TypeExpr::Function(params, return_type, _) => params
                .iter()
                .chain(std::iter::once(return_type.as_ref()))
                .map(|arg| self.type_info_value(arg))
                .collect(),
            _ => Vec::new(),
        };

        Value::Struct {
            type_name: "TypeInfo".to_string(),
            fields: vec![
                (
                    "type_name".to_string(),
                    Value::String(type_expr_display(&ty)),
                ),
                (
                    "kind".to_string(),
                    Value::String(self.type_expr_kind(&ty).to_string()),
                ),
                (
                    "has_secret".to_string(),
                    Value::Bool(self.type_expr_has_secret(&ty)),
                ),
                ("args".to_string(), Value::List(arg_values)),
            ],
        }
    }

    fn reflected_field_value(
        &self,
        value: &Value,
        owner_ty: &TypeExpr,
        field_metadata: &Value,
        expected_field_ty: &TypeExpr,
    ) -> Result<Value, String> {
        let (field_index, metadata_name, metadata_type_name) =
            Self::type_field_metadata(field_metadata)?;
        let owner_fields = self.type_expr_fields(owner_ty);
        let field = owner_fields.get(field_index).ok_or_else(|| {
            format!(
                "type.field_value: type '{}' has no field at index {}",
                type_expr_display(owner_ty),
                field_index
            )
        })?;

        if field.name != metadata_name {
            return Err(format!(
                "type.field_value: field metadata '{}' does not match field '{}' on type '{}'",
                metadata_name,
                field.name,
                type_expr_display(owner_ty)
            ));
        }

        let actual_type_name = type_expr_display(&field.ty);
        if actual_type_name != metadata_type_name {
            return Err(format!(
                "type.field_value: field metadata for '{}' has type '{}', but type '{}' reports '{}'",
                metadata_name,
                metadata_type_name,
                type_expr_display(owner_ty),
                actual_type_name
            ));
        }

        let expected_type_name = type_expr_display(expected_field_ty);
        if actual_type_name != expected_type_name {
            return Err(format!(
                "type.field_value: field '{}' has type '{}', requested '{}'",
                metadata_name, actual_type_name, expected_type_name
            ));
        }

        match value {
            Value::Struct { fields, .. } => fields
                .iter()
                .find(|(name, _)| name == &field.name)
                .map(|(_, field_value)| field_value.clone())
                .ok_or_else(|| {
                    format!("type.field_value: value is missing field '{}'", field.name)
                }),
            other => Err(format!(
                "type.field_value: expected struct value for '{}', got {other}",
                type_expr_display(owner_ty)
            )),
        }
    }

    fn type_field_metadata(value: &Value) -> Result<(usize, String, String), String> {
        let Value::Struct { type_name, fields } = value else {
            return Err(format!(
                "type.field_value: second argument must be TypeField, got {value}"
            ));
        };
        if type_name != "TypeField" {
            return Err(format!(
                "type.field_value: second argument must be TypeField, got {type_name}"
            ));
        }

        let field_value = |name: &str| {
            fields
                .iter()
                .find(|(field_name, _)| field_name == name)
                .map(|(_, field_value)| field_value)
                .ok_or_else(|| format!("type.field_value: TypeField is missing '{name}'"))
        };

        let index = match field_value("index")? {
            Value::Int64(index) if *index >= 0 => *index as usize,
            other => {
                return Err(format!(
                    "type.field_value: TypeField.index must be a non-negative int64, got {other}"
                ));
            }
        };
        let name = match field_value("name")? {
            Value::String(name) => name.clone(),
            other => {
                return Err(format!(
                    "type.field_value: TypeField.name must be string, got {other}"
                ));
            }
        };
        let type_name = match field_value("type_name")? {
            Value::String(type_name) => type_name.clone(),
            other => {
                return Err(format!(
                    "type.field_value: TypeField.type_name must be string, got {other}"
                ));
            }
        };

        Ok((index, name, type_name))
    }

    fn json_to_value_typed(&mut self, json: &JsonValue, ty: &TypeExpr) -> Result<Value, String> {
        let ty = self.substitute_type_expr(ty);
        match &ty {
            TypeExpr::View(inner, _) => self.json_to_value_typed(json, inner),
            TypeExpr::Named(ident) => {
                if let Some(base_ty) = self.type_alias_bases.get(&ident.name).cloned() {
                    let value = self.json_to_value_typed(json, &base_ty)?;
                    self.check_refinement(&ident.name, &value)?;
                    return Ok(value);
                }

                match ident.name.as_str() {
                    "int64" => json
                        .as_i64()
                        .map(Value::Int64)
                        .ok_or_else(|| format!("expected int64, got {}", json_type_name(json))),
                    "float64" => json
                        .as_f64()
                        .map(Value::Float64)
                        .ok_or_else(|| format!("expected float64, got {}", json_type_name(json))),
                    "string" => json
                        .as_str()
                        .map(|value| Value::String(value.to_string()))
                        .ok_or_else(|| format!("expected string, got {}", json_type_name(json))),
                    "bool" => json
                        .as_bool()
                        .map(Value::Bool)
                        .ok_or_else(|| format!("expected bool, got {}", json_type_name(json))),
                    "nothing" => {
                        if json.is_null() {
                            Ok(Value::Nothing)
                        } else {
                            Err(format!("expected null, got {}", json_type_name(json)))
                        }
                    }
                    "bytes" => self.json_to_bytes_value(json),
                    name if self.structs.contains_key(name) => {
                        self.json_to_struct_value(json, name, &HashMap::new())
                    }
                    name if self.enums.contains_key(name) => self.json_to_enum_value(json, name),
                    name if self.bitfields.contains_key(name) => {
                        self.json_to_bitfield_value(json, name)
                    }
                    other => Err(format!("json.parse does not support type '{other}' yet")),
                }
            }
            TypeExpr::Generic(ident, args, _) if self.structs.contains_key(&ident.name) => {
                let Some(strukt) = self.structs.get(&ident.name).cloned() else {
                    return Err(format!("unknown struct '{}'", ident.name));
                };
                let substitutions = self.generic_type_substitutions(&strukt, args);
                self.json_to_struct_value(json, &ident.name, &substitutions)
            }
            TypeExpr::Generic(ident, args, _) => match ident.name.as_str() {
                "list" if args.len() == 1 => {
                    let items = json
                        .as_array()
                        .ok_or_else(|| format!("expected array, got {}", json_type_name(json)))?;
                    let mut values = Vec::with_capacity(items.len());
                    for item in items {
                        values.push(self.json_to_value_typed(item, &args[0])?);
                    }
                    Ok(Value::List(values))
                }
                "set" if args.len() == 1 => {
                    let items = json
                        .as_array()
                        .ok_or_else(|| format!("expected array, got {}", json_type_name(json)))?;
                    let mut values = Vec::with_capacity(items.len());
                    for item in items {
                        let value = self.json_to_value_typed(item, &args[0])?;
                        if !values.contains(&value) {
                            values.push(value);
                        }
                    }
                    Ok(Value::Set(values))
                }
                "map" if args.len() == 2 => {
                    if type_expr_display(&args[0]) != "string" {
                        return Err("json.parse supports only map[string, V]".to_string());
                    }
                    let object = json
                        .as_object()
                        .ok_or_else(|| format!("expected object, got {}", json_type_name(json)))?;
                    let mut entries = Vec::with_capacity(object.len());
                    for (key, value) in object {
                        entries.push((
                            Value::String(key.clone()),
                            self.json_to_value_typed(value, &args[1])?,
                        ));
                    }
                    Ok(Value::Map(entries))
                }
                "optional" if args.len() == 1 => {
                    if json.is_null() {
                        Ok(Value::OptionalNone)
                    } else {
                        Ok(Value::OptionalSome(Box::new(
                            self.json_to_value_typed(json, &args[0])?,
                        )))
                    }
                }
                "result" if args.len() == 2 => {
                    let object = json
                        .as_object()
                        .ok_or_else(|| format!("expected object, got {}", json_type_name(json)))?;
                    if object.len() != 1 {
                        return Err("expected result object with exactly one key".to_string());
                    }
                    if let Some(value) = object.get("ok") {
                        Ok(Value::ResultOk(Box::new(
                            self.json_to_value_typed(value, &args[0])?,
                        )))
                    } else if let Some(value) = object.get("fail") {
                        Ok(Value::ResultFail(Box::new(
                            self.json_to_value_typed(value, &args[1])?,
                        )))
                    } else {
                        Err("expected result object with key 'ok' or 'fail'".to_string())
                    }
                }
                "secret" if args.len() == 1 => self.json_to_value_typed(json, &args[0]),
                _ => Err(format!(
                    "json.parse does not support type '{}' yet",
                    type_expr_display(&ty)
                )),
            },
            TypeExpr::Function(_, _, _) => {
                Err("json.parse does not support function types".to_string())
            }
        }
    }

    fn json_to_struct_value(
        &mut self,
        json: &JsonValue,
        struct_name: &str,
        substitutions: &HashMap<String, TypeExpr>,
    ) -> Result<Value, String> {
        let object = json
            .as_object()
            .ok_or_else(|| format!("expected object, got {}", json_type_name(json)))?;
        let strukt = self
            .structs
            .get(struct_name)
            .cloned()
            .ok_or_else(|| format!("unknown struct '{struct_name}'"))?;
        let mut fields = Vec::with_capacity(strukt.fields.len());

        for field in &strukt.fields {
            let field_ty = self.substitute_type_expr_with_map(&field.ty, substitutions);
            let json_name = field.serialize_name.as_ref().unwrap_or(&field.name.name);
            let value = match object.get(json_name) {
                Some(raw) => self.json_to_value_typed(raw, &field_ty)?,
                None if type_expr_is_optional(&field_ty) => Value::OptionalNone,
                None => {
                    return Err(format!(
                        "missing required field '{}' for {}",
                        json_name, struct_name
                    ));
                }
            };
            fields.push((field.name.name.clone(), value));
        }

        Ok(Value::Struct {
            type_name: struct_name.to_string(),
            fields,
        })
    }

    fn json_to_enum_value(&mut self, json: &JsonValue, enum_name: &str) -> Result<Value, String> {
        let enum_def = self
            .enums
            .get(enum_name)
            .cloned()
            .ok_or_else(|| format!("unknown enum '{enum_name}'"))?;

        if let Some(variant_name) = json.as_str() {
            let variant = enum_def
                .variants
                .iter()
                .find(|variant| variant.name.name == variant_name)
                .ok_or_else(|| format!("unknown enum variant '{enum_name}.{variant_name}'"))?;
            if !variant.fields.is_empty() {
                return Err(format!(
                    "enum variant '{}.{}' expects {} payload field(s)",
                    enum_name,
                    variant_name,
                    variant.fields.len()
                ));
            }
            return Ok(Value::Enum {
                type_name: enum_name.to_string(),
                variant: variant_name.to_string(),
                fields: Vec::new(),
            });
        }

        let object = json.as_object().ok_or_else(|| {
            format!(
                "expected enum string or object, got {}",
                json_type_name(json)
            )
        })?;
        if object.len() != 1 {
            return Err("expected enum object with exactly one variant key".to_string());
        }

        let (variant_name, payload) = object.iter().next().expect("object has one entry");
        let variant = enum_def
            .variants
            .iter()
            .find(|variant| variant.name.name == *variant_name)
            .ok_or_else(|| format!("unknown enum variant '{enum_name}.{variant_name}'"))?;
        let items = payload.as_array().ok_or_else(|| {
            format!(
                "expected array payload for enum variant '{}.{}', got {}",
                enum_name,
                variant_name,
                json_type_name(payload)
            )
        })?;
        if items.len() != variant.fields.len() {
            return Err(format!(
                "enum variant '{}.{}' expects {} payload field(s), got {}",
                enum_name,
                variant_name,
                variant.fields.len(),
                items.len()
            ));
        }

        let mut fields = Vec::with_capacity(items.len());
        for (field, raw) in variant.fields.iter().zip(items.iter()) {
            fields.push(self.json_to_value_typed(raw, &field.ty)?);
        }

        Ok(Value::Enum {
            type_name: enum_name.to_string(),
            variant: variant_name.to_string(),
            fields,
        })
    }

    fn json_to_bitfield_value(
        &mut self,
        json: &JsonValue,
        bitfield_name: &str,
    ) -> Result<Value, String> {
        let object = json
            .as_object()
            .ok_or_else(|| format!("expected object, got {}", json_type_name(json)))?;
        let bitfield = self
            .bitfields
            .get(bitfield_name)
            .cloned()
            .ok_or_else(|| format!("unknown bitfield '{bitfield_name}'"))?;
        let mut fields = Vec::with_capacity(bitfield.fields.len());

        for field in &bitfield.fields {
            let raw = object.get(&field.name.name).ok_or_else(|| {
                format!(
                    "missing required field '{}' for {}",
                    field.name.name, bitfield_name
                )
            })?;
            let value = match &field.kind {
                BitfieldFieldKind::Bits { width, as_type } => {
                    let ty = as_type.as_ref().cloned().unwrap_or_else(|| {
                        TypeExpr::Named(Ident {
                            name: "int64".to_string(),
                            span: field.span,
                        })
                    });
                    let value = self.json_to_value_typed(raw, &ty)?;
                    self.bitfield_field_numeric_value(
                        &bitfield,
                        &field.name.name,
                        *width,
                        as_type.as_ref(),
                        &value,
                    )?;
                    value
                }
                BitfieldFieldKind::Payload(ty) => self.json_to_value_typed(raw, ty)?,
            };
            fields.push((field.name.name.clone(), value));
        }

        Ok(Value::Struct {
            type_name: bitfield_name.to_string(),
            fields,
        })
    }

    fn json_to_bytes_value(&self, json: &JsonValue) -> Result<Value, String> {
        let raw = json
            .as_str()
            .ok_or_else(|| format!("expected bytes string, got {}", json_type_name(json)))?;
        let hex = raw.strip_prefix("0x").unwrap_or(raw);
        if hex.len() % 2 != 0 {
            return Err("expected even-length hex bytes string".to_string());
        }
        let mut bytes = Vec::with_capacity(hex.len() / 2);
        for index in (0..hex.len()).step_by(2) {
            let byte = u8::from_str_radix(&hex[index..index + 2], 16)
                .map_err(|_| "expected hex bytes string".to_string())?;
            bytes.push(byte);
        }
        Ok(Value::Bytes(bytes))
    }

    fn value_to_json_typed(&self, value: &Value, ty: &TypeExpr, public_only: bool) -> String {
        let ty = self.substitute_type_expr(ty);
        match &ty {
            TypeExpr::View(inner, _) => {
                return self.value_to_json_typed(value, inner, public_only);
            }
            TypeExpr::Generic(ident, args, _) if ident.name == "list" && args.len() == 1 => {
                if let Value::List(items) = value {
                    let elems: Vec<String> = items
                        .iter()
                        .map(|item| self.value_to_json_typed(item, &args[0], public_only))
                        .collect();
                    return format!("[{}]", elems.join(","));
                }
            }
            TypeExpr::Generic(ident, args, _) if ident.name == "set" && args.len() == 1 => {
                if let Value::Set(items) = value {
                    let elems: Vec<String> = items
                        .iter()
                        .map(|item| self.value_to_json_typed(item, &args[0], public_only))
                        .collect();
                    return format!("[{}]", elems.join(","));
                }
            }
            TypeExpr::Generic(ident, args, _) if ident.name == "map" && args.len() == 2 => {
                if let Value::Map(entries) = value {
                    let pairs: Vec<String> = entries
                        .iter()
                        .map(|(key, val)| {
                            format!(
                                "{}:{}",
                                self.value_to_json_typed(key, &args[0], public_only),
                                self.value_to_json_typed(val, &args[1], public_only)
                            )
                        })
                        .collect();
                    return format!("{{{}}}", pairs.join(","));
                }
            }
            TypeExpr::Generic(ident, args, _) if ident.name == "optional" && args.len() == 1 => {
                return match value {
                    Value::OptionalNone => "null".to_string(),
                    Value::OptionalSome(inner) => {
                        self.value_to_json_typed(inner, &args[0], public_only)
                    }
                    _ => self.value_to_json_reflected(value, public_only),
                };
            }
            TypeExpr::Generic(ident, args, _) if ident.name == "result" && args.len() == 2 => {
                return match value {
                    Value::ResultOk(inner) => format!(
                        "{{\"ok\":{}}}",
                        self.value_to_json_typed(inner, &args[0], public_only)
                    ),
                    Value::ResultFail(inner) => format!(
                        "{{\"fail\":{}}}",
                        self.value_to_json_typed(inner, &args[1], public_only)
                    ),
                    _ => self.value_to_json_reflected(value, public_only),
                };
            }
            TypeExpr::Generic(ident, args, _) if ident.name == "secret" && args.len() == 1 => {
                return self.value_to_json_typed(value, &args[0], public_only);
            }
            _ => {}
        }

        if self.type_expr_kind(&ty) == "struct" {
            if let Value::Struct { fields, .. } = value {
                let reflected_fields = self.type_expr_fields(&ty);
                let pairs: Vec<String> = reflected_fields
                    .iter()
                    .filter(|field| !public_only || !self.type_expr_has_secret(&field.ty))
                    .filter_map(|field| {
                        fields.iter().find(|(name, _)| name == &field.name).map(
                            |(_, field_value)| {
                                format!(
                                    "{}:{}",
                                    json_string(&field.serialize_name),
                                    self.value_to_json_typed(field_value, &field.ty, public_only)
                                )
                            },
                        )
                    })
                    .collect();
                return format!("{{{}}}", pairs.join(","));
            }
        }

        self.value_to_json_reflected(value, public_only)
    }

    fn value_to_json_reflected(&self, value: &Value, public_only: bool) -> String {
        match value {
            Value::Int64(n) => n.to_string(),
            Value::Float64(f) => {
                if f.fract() == 0.0 && f.is_finite() {
                    format!("{f:.1}")
                } else {
                    f.to_string()
                }
            }
            Value::String(s) => json_string(s),
            Value::Bool(b) => b.to_string(),
            Value::Nothing | Value::OptionalNone => "null".to_string(),
            Value::OptionalSome(inner) => self.value_to_json_reflected(inner, public_only),
            Value::ResultOk(inner) => {
                format!(
                    "{{\"ok\":{}}}",
                    self.value_to_json_reflected(inner, public_only)
                )
            }
            Value::ResultFail(inner) => {
                format!(
                    "{{\"fail\":{}}}",
                    self.value_to_json_reflected(inner, public_only)
                )
            }
            Value::List(items) => {
                let elems: Vec<String> = items
                    .iter()
                    .map(|item| self.value_to_json_reflected(item, public_only))
                    .collect();
                format!("[{}]", elems.join(","))
            }
            Value::Map(entries) => {
                let pairs: Vec<String> = entries
                    .iter()
                    .map(|(k, v)| {
                        format!(
                            "{}:{}",
                            self.value_to_json_reflected(k, public_only),
                            self.value_to_json_reflected(v, public_only)
                        )
                    })
                    .collect();
                format!("{{{}}}", pairs.join(","))
            }
            Value::Set(items) => {
                let elems: Vec<String> = items
                    .iter()
                    .map(|item| self.value_to_json_reflected(item, public_only))
                    .collect();
                format!("[{}]", elems.join(","))
            }
            Value::Struct { type_name, fields } => {
                let pairs: Vec<String> = fields
                    .iter()
                    .filter(|(name, _)| {
                        !public_only || !self.struct_field_has_secret(type_name, name)
                    })
                    .map(|(name, value)| {
                        format!(
                            "{}:{}",
                            json_string(&self.struct_field_serialize_name(type_name, name)),
                            self.value_to_json_reflected(value, public_only)
                        )
                    })
                    .collect();
                format!("{{{}}}", pairs.join(","))
            }
            Value::Enum {
                type_name: _,
                variant,
                fields,
            } => {
                if fields.is_empty() {
                    format!("\"{}\"", variant)
                } else {
                    let elems: Vec<String> = fields
                        .iter()
                        .map(|field| self.value_to_json_reflected(field, public_only))
                        .collect();
                    format!("{{\"{}\":[{}]}}", variant, elems.join(","))
                }
            }
            Value::Bytes(bytes) => {
                let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
                format!("\"0x{hex}\"")
            }
            Value::Error(msg) => format!(
                "{{\"error\":{}}}",
                self.value_to_json_reflected(&Value::String(msg.clone()), public_only)
            ),
            _ => "null".to_string(),
        }
    }

    fn struct_field_has_secret(&self, type_name: &str, field_name: &str) -> bool {
        self.structs
            .get(type_name)
            .and_then(|strukt| {
                strukt
                    .fields
                    .iter()
                    .find(|field| field.name.name == field_name)
            })
            .map(|field| self.type_expr_has_secret(&field.ty))
            .unwrap_or(false)
    }

    fn struct_field_serialize_name(&self, type_name: &str, field_name: &str) -> String {
        self.structs
            .get(type_name)
            .and_then(|strukt| {
                strukt
                    .fields
                    .iter()
                    .find(|field| field.name.name == field_name)
            })
            .and_then(|field| field.serialize_name.clone())
            .unwrap_or_else(|| field_name.to_string())
    }

    // =========================================================================
    // Built-in implementations
    //
    // This function handles two distinct categories:
    //
    //   COMPILER PRIMITIVES — operations that are semantically special and
    //   will remain as interpreter/compiler built-ins permanently (I/O
    //   capabilities, JSON serialization, secret-taint ops, random).
    //
    //   STANDARD LIBRARY STUBS — functions that belong to the Jett standard
    //   library (stdlib/*.jett) but are implemented here as interpreter
    //   builtins until Phase D code generation is complete.  Once codegen
    //   exists these should migrate to actual Jett source files.
    // =========================================================================

    /// Try to call a built-in function.  Returns `None` if the name does not
    /// match any built-in, allowing the caller to fall through to
    /// user-defined function lookup.
    fn call_builtin(&self, name: &str, args: &[Value]) -> Option<Result<Value, String>> {
        if let Some(result) = self.call_bitfield_builtin(name, args) {
            return Some(result);
        }

        match name {
            // =================================================================
            // COMPILER PRIMITIVES
            // =================================================================

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
                Some(Ok(Value::String("***".to_string())))
            }

            "secret.compare" => {
                require_args!(name, 2, args);
                Some(Ok(Value::Bool(args[0] == args[1])))
            }

            // -- JSON operations ---------------------------------------------
            "json.serialize" | "json.serialize_public" => {
                require_args!(name, 1, args);
                Some(Ok(Value::String(self.value_to_json_reflected(
                    &args[0],
                    name == "json.serialize_public",
                ))))
            }

            // -- Random operations (stdlib/random.jett) -----------------------
            "random.int64" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::Int64(lo), Value::Int64(hi)) => {
                        if lo >= hi {
                            Some(Err(format!(
                                "random.int64: lo ({lo}) must be less than hi ({hi})"
                            )))
                        } else {
                            let n = rand::thread_rng().gen_range(*lo..*hi);
                            Some(Ok(Value::Int64(n)))
                        }
                    }
                    _ => Some(Err(format!("{name} expects two int64 arguments"))),
                }
            }

            "random.float64" => {
                require_args!(name, 0, args);
                let f: f64 = rand::thread_rng().gen_range(0.0f64..1.0f64);
                Some(Ok(Value::Float64(f)))
            }

            "random.bool" => {
                require_args!(name, 0, args);
                Some(Ok(Value::Bool(rand::thread_rng().gen_bool(0.5))))
            }

            "random.choice" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::List(items) => {
                        if items.is_empty() {
                            Some(Ok(Value::OptionalNone))
                        } else {
                            let idx = rand::thread_rng().gen_range(0..items.len());
                            Some(Ok(Value::OptionalSome(Box::new(items[idx].clone()))))
                        }
                    }
                    _ => Some(Err(format!("{name} expects a list argument"))),
                }
            }

            "random.shuffle" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::List(items) => {
                        use rand::seq::SliceRandom;
                        let mut shuffled = items.clone();
                        shuffled.shuffle(&mut rand::thread_rng());
                        Some(Ok(Value::List(shuffled)))
                    }
                    _ => Some(Err(format!("{name} expects a list argument"))),
                }
            }

            // =================================================================
            // STANDARD LIBRARY STUBS
            // (will migrate to stdlib/*.jett once codegen is available)
            // =================================================================

            // -- String operations (stdlib/string.jett) -----------------------
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

            "string.is_empty" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::String(s) => Some(Ok(Value::Bool(s.is_empty()))),
                    _ => Some(Err(format!("{name} expects a string argument"))),
                }
            }

            "string.slice" => {
                require_args!(name, 3, args);
                match (&args[0], &args[1], &args[2]) {
                    (Value::String(s), Value::Int64(start), Value::Int64(end)) => {
                        let chars: Vec<char> = s.chars().collect();
                        let len = chars.len() as i64;
                        let start = (*start).clamp(0, len) as usize;
                        let end = (*end).clamp(0, len) as usize;
                        let result: String = chars[start.min(end)..end].iter().collect();
                        Some(Ok(Value::String(result)))
                    }
                    _ => Some(Err(format!(
                        "{name} expects a string and two int64 indices"
                    ))),
                }
            }

            "string.repeat" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::String(s), Value::Int64(n)) => {
                        let n = (*n).max(0) as usize;
                        Some(Ok(Value::String(s.repeat(n))))
                    }
                    _ => Some(Err(format!("{name} expects a string and an int64"))),
                }
            }

            // string.pad_start is an alias for string.pad_left
            "string.pad_start" => self.call_builtin("string.pad_left", args),

            "string.pad_end" => {
                require_args!(name, 3, args);
                match (&args[0], &args[1], &args[2]) {
                    (Value::String(s), Value::Int64(width), Value::String(pad)) => {
                        let pad_char = pad.chars().next().unwrap_or(' ');
                        let current_len = s.chars().count();
                        let width = (*width).max(0) as usize;
                        if current_len >= width {
                            Some(Ok(Value::String(s.clone())))
                        } else {
                            let padding: String = std::iter::repeat(pad_char)
                                .take(width - current_len)
                                .collect();
                            Some(Ok(Value::String(format!("{s}{padding}"))))
                        }
                    }
                    _ => Some(Err(format!(
                        "{name} expects a string, int64 width, and string pad char"
                    ))),
                }
            }

            // -- Type conversions (stdlib/string.jett, stdlib/int64.jett) -----
            "string.from_int64" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::Int64(n) => Some(Ok(Value::String(n.to_string()))),
                    _ => Some(Err(format!("{name} expects an int64 argument"))),
                }
            }

            "string.is_not_empty" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::String(s) => Some(Ok(Value::Bool(!s.is_empty()))),
                    _ => Some(Err(format!("{name} expects a string argument"))),
                }
            }

            "string.slugify" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::String(s) => {
                        let slug: String = s
                            .to_lowercase()
                            .chars()
                            .map(|c| if c.is_alphanumeric() { c } else { '-' })
                            .collect::<String>()
                            .split('-')
                            .filter(|part| !part.is_empty())
                            .collect::<Vec<_>>()
                            .join("-");
                        Some(Ok(Value::String(slug)))
                    }
                    _ => Some(Err(format!("{name} expects a string argument"))),
                }
            }

            "string.truncate" => {
                if args.len() != 3 {
                    return Some(Err(format!("{name} expects 3 arguments")));
                }
                match (&args[0], &args[1], &args[2]) {
                    (Value::String(s), Value::Int64(max_len), Value::String(suffix)) => {
                        let max = (*max_len).max(0) as usize;
                        let chars: Vec<char> = s.chars().collect();
                        let result = if chars.len() <= max {
                            s.clone()
                        } else {
                            // Keep first `max` characters, then append suffix
                            let kept: String = chars[..max].iter().collect();
                            format!("{kept}{suffix}")
                        };
                        Some(Ok(Value::String(result)))
                    }
                    _ => Some(Err(format!("{name} expects (string, int64, string)"))),
                }
            }

            "string.between" => {
                if args.len() != 3 {
                    return Some(Err(format!("{name} expects 3 arguments")));
                }
                match (&args[0], &args[1], &args[2]) {
                    (Value::String(s), Value::String(start), Value::String(end)) => {
                        // Returns "" when the markers are not found (design doc shows plain string)
                        let result = if let Some(after_start) =
                            s.find(start.as_str()).map(|i| &s[i + start.len()..])
                        {
                            if let Some(end_pos) = after_start.find(end.as_str()) {
                                after_start[..end_pos].to_string()
                            } else {
                                String::new()
                            }
                        } else {
                            String::new()
                        };
                        Some(Ok(Value::String(result)))
                    }
                    _ => Some(Err(format!("{name} expects (string, string, string)"))),
                }
            }

            // string.pad_left is the canonical name (design doc); pad_start is an alias
            "string.pad_left" => {
                require_args!(name, 3, args);
                match (&args[0], &args[1], &args[2]) {
                    (Value::String(s), Value::Int64(width), Value::String(pad)) => {
                        let pad_char = pad.chars().next().unwrap_or(' ');
                        let current_len = s.chars().count();
                        let width = (*width).max(0) as usize;
                        if current_len >= width {
                            Some(Ok(Value::String(s.clone())))
                        } else {
                            let padding: String = std::iter::repeat(pad_char)
                                .take(width - current_len)
                                .collect();
                            Some(Ok(Value::String(format!("{padding}{s}"))))
                        }
                    }
                    _ => Some(Err(format!("{name} expects (string, int64, string)"))),
                }
            }

            // -- int64 / float64 conversions ----------------------------------
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

            // -- float64 conversions ------------------------------------------
            "float64.from_int64" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::Int64(n) => Some(Ok(Value::Float64(*n as f64))),
                    _ => Some(Err(format!("{name} expects an int64 argument"))),
                }
            }
            "float64.from_string" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::String(s) => match s.parse::<f64>() {
                        Ok(n) => Some(Ok(Value::ResultOk(Box::new(Value::Float64(n))))),
                        Err(_) => Some(Ok(Value::ResultFail(Box::new(Value::String(format!(
                            "float64.from_string: cannot parse '{s}' as float64"
                        )))))),
                    },
                    _ => Some(Err(format!("{name} expects a string argument"))),
                }
            }

            // -- Additional string conversions --------------------------------
            "string.from_float64" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::Float64(n) => Some(Ok(Value::String(format!("{n}")))),
                    _ => Some(Err(format!("{name} expects a float64 argument"))),
                }
            }
            "string.from_bool" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::Bool(b) => Some(Ok(Value::String(format!("{b}")))),
                    _ => Some(Err(format!("{name} expects a bool argument"))),
                }
            }

            // -- print (debugging helper) -------------------------------------
            "print" => {
                let output: Vec<String> = args.iter().map(|v| format!("{v}")).collect();
                print!("{}", output.join(" "));
                Some(Ok(Value::Nothing))
            }
            "println" => {
                let output: Vec<String> = args.iter().map(|v| format!("{v}")).collect();
                println!("{}", output.join(" "));
                Some(Ok(Value::Nothing))
            }

            // -- List operations (stdlib/list.jett) ---------------------------
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

            "list.skip" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::List(items), Value::Int64(n)) => {
                        let n = (*n).max(0) as usize;
                        Some(Ok(Value::List(items[n.min(items.len())..].to_vec())))
                    }
                    _ => Some(Err(format!("{name} expects a list and an int64"))),
                }
            }

            "list.take" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::List(items), Value::Int64(n)) => {
                        let n = (*n).max(0) as usize;
                        Some(Ok(Value::List(items[..n.min(items.len())].to_vec())))
                    }
                    _ => Some(Err(format!("{name} expects a list and an int64"))),
                }
            }

            "list.reverse" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::List(items) => {
                        let mut reversed = items.clone();
                        reversed.reverse();
                        Some(Ok(Value::List(reversed)))
                    }
                    _ => Some(Err(format!("{name} expects a list argument"))),
                }
            }

            "list.sort" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::List(items) => {
                        let mut sorted = items.clone();
                        sorted.sort_by(|a, b| match (a, b) {
                            (Value::Int64(x), Value::Int64(y)) => x.cmp(y),
                            (Value::Float64(x), Value::Float64(y)) => {
                                x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal)
                            }
                            (Value::String(x), Value::String(y)) => x.cmp(y),
                            (Value::Bool(x), Value::Bool(y)) => x.cmp(y),
                            _ => std::cmp::Ordering::Equal,
                        });
                        Some(Ok(Value::List(sorted)))
                    }
                    _ => Some(Err(format!("{name} expects a list argument"))),
                }
            }

            "list.contains" => {
                require_args!(name, 2, args);
                match &args[0] {
                    Value::List(items) => Some(Ok(Value::Bool(items.contains(&args[1])))),
                    _ => Some(Err(format!("{name} expects a list as first argument"))),
                }
            }

            "list.index_of" => {
                require_args!(name, 2, args);
                match &args[0] {
                    Value::List(items) => {
                        let idx = items.iter().position(|v| v == &args[1]);
                        Some(Ok(match idx {
                            Some(i) => Value::OptionalSome(Box::new(Value::Int64(i as i64))),
                            None => Value::OptionalNone,
                        }))
                    }
                    _ => Some(Err(format!("{name} expects a list as first argument"))),
                }
            }

            "list.remove" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::List(items), Value::Int64(index)) => {
                        let idx = *index as usize;
                        if idx < items.len() {
                            let mut new_list = items.clone();
                            new_list.remove(idx);
                            Some(Ok(Value::List(new_list)))
                        } else {
                            Some(Err(format!("{name}: index {index} out of bounds")))
                        }
                    }
                    _ => Some(Err(format!("{name} expects a list and an int64 index"))),
                }
            }

            "list.concat" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::List(a), Value::List(b)) => {
                        let mut result = a.clone();
                        result.extend(b.iter().cloned());
                        Some(Ok(Value::List(result)))
                    }
                    _ => Some(Err(format!("{name} expects two list arguments"))),
                }
            }

            "list.flatten" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::List(items) => {
                        let mut flat = Vec::new();
                        for item in items {
                            match item {
                                Value::List(inner) => flat.extend(inner.iter().cloned()),
                                other => flat.push(other.clone()),
                            }
                        }
                        Some(Ok(Value::List(flat)))
                    }
                    _ => Some(Err(format!("{name} expects a list argument"))),
                }
            }

            "list.unique" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::List(items) => {
                        let mut seen = Vec::new();
                        for item in items {
                            if !seen.contains(item) {
                                seen.push(item.clone());
                            }
                        }
                        Some(Ok(Value::List(seen)))
                    }
                    _ => Some(Err(format!("{name} expects a list argument"))),
                }
            }

            "list.zip" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::List(a), Value::List(b)) => {
                        let pairs: Vec<Value> = a
                            .iter()
                            .zip(b.iter())
                            .map(|(x, y)| Value::List(vec![x.clone(), y.clone()]))
                            .collect();
                        Some(Ok(Value::List(pairs)))
                    }
                    _ => Some(Err(format!("{name} expects two list arguments"))),
                }
            }

            // -- Map operations (stdlib/map.jett) -----------------------------
            "map.new" => Some(Ok(Value::Map(Vec::new()))),
            "map.length" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::Map(entries) => Some(Ok(Value::Int64(entries.len() as i64))),
                    _ => Some(Err(format!("{name} expects a map argument"))),
                }
            }
            "map.has" => {
                require_args!(name, 2, args);
                match &args[0] {
                    Value::Map(entries) => {
                        let found = entries.iter().any(|(k, _)| k == &args[1]);
                        Some(Ok(Value::Bool(found)))
                    }
                    _ => Some(Err(format!("{name} expects a map as first argument"))),
                }
            }
            "map.get" => {
                require_args!(name, 2, args);
                match &args[0] {
                    Value::Map(entries) => {
                        let val = entries
                            .iter()
                            .find(|(k, _)| k == &args[1])
                            .map(|(_, v)| v.clone());
                        Some(Ok(match val {
                            Some(v) => Value::OptionalSome(Box::new(v)),
                            None => Value::OptionalNone,
                        }))
                    }
                    _ => Some(Err(format!("{name} expects a map as first argument"))),
                }
            }
            "map.insert" => {
                require_args!(name, 3, args);
                match args[0].clone() {
                    Value::Map(mut entries) => {
                        let key = args[1].clone();
                        let val = args[2].clone();
                        if let Some(entry) = entries.iter_mut().find(|(k, _)| k == &key) {
                            entry.1 = val;
                        } else {
                            entries.push((key, val));
                        }
                        Some(Ok(Value::Map(entries)))
                    }
                    _ => Some(Err(format!("{name} expects a map as first argument"))),
                }
            }
            "map.remove" => {
                require_args!(name, 2, args);
                match args[0].clone() {
                    Value::Map(mut entries) => {
                        entries.retain(|(k, _)| k != &args[1]);
                        Some(Ok(Value::Map(entries)))
                    }
                    _ => Some(Err(format!("{name} expects a map as first argument"))),
                }
            }
            "map.keys" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::Map(entries) => {
                        let keys = entries.iter().map(|(k, _)| k.clone()).collect();
                        Some(Ok(Value::List(keys)))
                    }
                    _ => Some(Err(format!("{name} expects a map argument"))),
                }
            }
            "map.values" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::Map(entries) => {
                        let vals = entries.iter().map(|(_, v)| v.clone()).collect();
                        Some(Ok(Value::List(vals)))
                    }
                    _ => Some(Err(format!("{name} expects a map argument"))),
                }
            }
            "map.is_empty" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::Map(entries) => Some(Ok(Value::Bool(entries.is_empty()))),
                    _ => Some(Err(format!("{name} expects a map argument"))),
                }
            }

            // map.set is the canonical name (design doc); insert is an alias
            "map.set" => self.call_builtin("map.insert", args),

            "map.get_or" => {
                if args.len() != 3 {
                    return Some(Err(format!("{name} expects 3 arguments")));
                }
                match &args[0] {
                    Value::Map(entries) => {
                        let key = &args[1];
                        let default = args[2].clone();
                        let found = entries
                            .iter()
                            .find(|(k, _)| k == key)
                            .map(|(_, v)| v.clone())
                            .unwrap_or(default);
                        Some(Ok(found))
                    }
                    _ => Some(Err(format!("{name} expects a map argument"))),
                }
            }

            "map.merge" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::Map(a), Value::Map(b)) => {
                        let mut merged = a.clone();
                        for (k, v) in b {
                            if let Some(pos) = merged.iter().position(|(mk, _)| mk == k) {
                                merged[pos].1 = v.clone();
                            } else {
                                merged.push((k.clone(), v.clone()));
                            }
                        }
                        Some(Ok(Value::Map(merged)))
                    }
                    _ => Some(Err(format!("{name} expects two map arguments"))),
                }
            }

            "map.contains_key" => self.call_builtin("map.has", args),

            "map.from_lists" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::List(keys), Value::List(values)) => {
                        let entries: Vec<(Value, Value)> = keys
                            .iter()
                            .zip(values.iter())
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect();
                        Some(Ok(Value::Map(entries)))
                    }
                    _ => Some(Err(format!("{name} expects two list arguments"))),
                }
            }

            "map.entries" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::Map(entries) => {
                        let pairs = entries
                            .iter()
                            .map(|(k, v)| Value::List(vec![k.clone(), v.clone()]))
                            .collect();
                        Some(Ok(Value::List(pairs)))
                    }
                    _ => Some(Err(format!("{name} expects a map argument"))),
                }
            }

            // -- Set operations (stdlib/set.jett) -----------------------------
            "set.new" => Some(Ok(Value::Set(Vec::new()))),
            "set.add" => {
                require_args!(name, 2, args);
                match args[0].clone() {
                    Value::Set(mut items) => {
                        let val = args[1].clone();
                        if !items.contains(&val) {
                            items.push(val);
                        }
                        Some(Ok(Value::Set(items)))
                    }
                    _ => Some(Err(format!("{name} expects a set as first argument"))),
                }
            }
            "set.remove" => {
                require_args!(name, 2, args);
                match args[0].clone() {
                    Value::Set(mut items) => {
                        items.retain(|v| v != &args[1]);
                        Some(Ok(Value::Set(items)))
                    }
                    _ => Some(Err(format!("{name} expects a set as first argument"))),
                }
            }
            "set.contains" => {
                require_args!(name, 2, args);
                match &args[0] {
                    Value::Set(items) => Some(Ok(Value::Bool(items.contains(&args[1])))),
                    _ => Some(Err(format!("{name} expects a set as first argument"))),
                }
            }
            "set.length" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::Set(items) => Some(Ok(Value::Int64(items.len() as i64))),
                    _ => Some(Err(format!("{name} expects a set argument"))),
                }
            }
            "set.is_empty" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::Set(items) => Some(Ok(Value::Bool(items.is_empty()))),
                    _ => Some(Err(format!("{name} expects a set argument"))),
                }
            }
            "set.to_list" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::Set(items) => Some(Ok(Value::List(items.clone()))),
                    _ => Some(Err(format!("{name} expects a set argument"))),
                }
            }
            "set.union" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::Set(a), Value::Set(b)) => {
                        let mut result = a.clone();
                        for item in b {
                            if !result.contains(item) {
                                result.push(item.clone());
                            }
                        }
                        Some(Ok(Value::Set(result)))
                    }
                    _ => Some(Err(format!("{name} expects two set arguments"))),
                }
            }
            "set.intersection" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::Set(a), Value::Set(b)) => {
                        let result: Vec<Value> =
                            a.iter().filter(|v| b.contains(v)).cloned().collect();
                        Some(Ok(Value::Set(result)))
                    }
                    _ => Some(Err(format!("{name} expects two set arguments"))),
                }
            }
            "set.difference" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::Set(a), Value::Set(b)) => {
                        let result: Vec<Value> =
                            a.iter().filter(|v| !b.contains(v)).cloned().collect();
                        Some(Ok(Value::Set(result)))
                    }
                    _ => Some(Err(format!("{name} expects two set arguments"))),
                }
            }

            // -- Additional list operations ------------------------------------
            "list.chunk" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::List(items), Value::Int64(size)) => {
                        let size = (*size).max(1) as usize;
                        let chunks: Vec<Value> = items
                            .chunks(size)
                            .map(|c| Value::List(c.to_vec()))
                            .collect();
                        Some(Ok(Value::List(chunks)))
                    }
                    _ => Some(Err(format!(
                        "{name} expects a list and an int64 chunk size"
                    ))),
                }
            }

            "list.sort_by_index" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::List(items), Value::Int64(idx)) => {
                        let idx = *idx as usize;
                        let mut sorted = items.clone();
                        sorted.sort_by(|a, b| {
                            let va = match a {
                                Value::List(l) => l.get(idx).cloned(),
                                _ => None,
                            };
                            let vb = match b {
                                Value::List(l) => l.get(idx).cloned(),
                                _ => None,
                            };
                            match (va, vb) {
                                (Some(Value::String(sa)), Some(Value::String(sb))) => sa.cmp(&sb),
                                (Some(Value::Int64(ia)), Some(Value::Int64(ib))) => ia.cmp(&ib),
                                _ => std::cmp::Ordering::Equal,
                            }
                        });
                        Some(Ok(Value::List(sorted)))
                    }
                    _ => Some(Err(format!(
                        "{name} expects a list of lists and an int64 index"
                    ))),
                }
            }

            "list.is_sorted" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::List(items) => {
                        let sorted = items.windows(2).all(|w| match (&w[0], &w[1]) {
                            (Value::Int64(a), Value::Int64(b)) => a <= b,
                            (Value::Float64(a), Value::Float64(b)) => a <= b,
                            (Value::String(a), Value::String(b)) => a <= b,
                            _ => true,
                        });
                        Some(Ok(Value::Bool(sorted)))
                    }
                    _ => Some(Err(format!("{name} expects a list argument"))),
                }
            }

            "list.all_elements_in" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::List(items), Value::List(pool)) => {
                        let all_in = items.iter().all(|item| pool.contains(item));
                        Some(Ok(Value::Bool(all_in)))
                    }
                    _ => Some(Err(format!("{name} expects two list arguments"))),
                }
            }

            // -- Math operations (stdlib/math.jett) ---------------------------
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

            "math.sqrt" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::Float64(n) => Some(Ok(Value::Float64(n.sqrt()))),
                    Value::Int64(n) => Some(Ok(Value::Float64((*n as f64).sqrt()))),
                    _ => Some(Err(format!("{name} expects a numeric argument"))),
                }
            }

            "math.pow" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::Float64(base), Value::Float64(exp)) => {
                        Some(Ok(Value::Float64(base.powf(*exp))))
                    }
                    (Value::Int64(base), Value::Int64(exp)) => {
                        let exp_u = (*exp).max(0) as u32;
                        Some(Ok(Value::Int64(base.pow(exp_u))))
                    }
                    (Value::Float64(base), Value::Int64(exp)) => {
                        Some(Ok(Value::Float64(base.powi(*exp as i32))))
                    }
                    _ => Some(Err(format!("{name} expects numeric arguments"))),
                }
            }

            "math.floor" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::Float64(n) => Some(Ok(Value::Float64(n.floor()))),
                    Value::Int64(n) => Some(Ok(Value::Int64(*n))),
                    _ => Some(Err(format!("{name} expects a numeric argument"))),
                }
            }

            "math.ceil" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::Float64(n) => Some(Ok(Value::Float64(n.ceil()))),
                    Value::Int64(n) => Some(Ok(Value::Int64(*n))),
                    _ => Some(Err(format!("{name} expects a numeric argument"))),
                }
            }

            "math.round" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::Float64(n) => Some(Ok(Value::Float64(n.round()))),
                    Value::Int64(n) => Some(Ok(Value::Int64(*n))),
                    _ => Some(Err(format!("{name} expects a numeric argument"))),
                }
            }

            "math.clamp" => {
                require_args!(name, 3, args);
                match (&args[0], &args[1], &args[2]) {
                    (Value::Int64(v), Value::Int64(lo), Value::Int64(hi)) => {
                        Some(Ok(Value::Int64((*v).clamp(*lo, *hi))))
                    }
                    (Value::Float64(v), Value::Float64(lo), Value::Float64(hi)) => {
                        Some(Ok(Value::Float64(v.clamp(*lo, *hi))))
                    }
                    _ => Some(Err(format!(
                        "{name} expects three arguments of the same numeric type"
                    ))),
                }
            }

            "math.log" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::Float64(n) => Some(Ok(Value::Float64(n.ln()))),
                    Value::Int64(n) => Some(Ok(Value::Float64((*n as f64).ln()))),
                    _ => Some(Err(format!("{name} expects a numeric argument"))),
                }
            }

            "math.log2" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::Float64(n) => Some(Ok(Value::Float64(n.log2()))),
                    Value::Int64(n) => Some(Ok(Value::Float64((*n as f64).log2()))),
                    _ => Some(Err(format!("{name} expects a numeric argument"))),
                }
            }

            "math.log10" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::Float64(n) => Some(Ok(Value::Float64(n.log10()))),
                    Value::Int64(n) => Some(Ok(Value::Float64((*n as f64).log10()))),
                    _ => Some(Err(format!("{name} expects a numeric argument"))),
                }
            }

            "math.average" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::List(items) if !items.is_empty() => {
                        let sum: f64 = items
                            .iter()
                            .map(|v| match v {
                                Value::Int64(n) => *n as f64,
                                Value::Float64(n) => *n,
                                _ => 0.0,
                            })
                            .sum();
                        Some(Ok(Value::Float64(sum / items.len() as f64)))
                    }
                    Value::List(_) => Some(Err("math.average: list is empty".to_string())),
                    _ => Some(Err(format!("{name} expects a list of numbers"))),
                }
            }

            "math.median" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::List(items) if !items.is_empty() => {
                        let mut nums: Vec<f64> = items
                            .iter()
                            .map(|v| match v {
                                Value::Int64(n) => *n as f64,
                                Value::Float64(n) => *n,
                                _ => 0.0,
                            })
                            .collect();
                        nums.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                        let mid = nums.len() / 2;
                        let median = if nums.len() % 2 == 0 {
                            (nums[mid - 1] + nums[mid]) / 2.0
                        } else {
                            nums[mid]
                        };
                        Some(Ok(Value::Float64(median)))
                    }
                    Value::List(_) => Some(Err("math.median: list is empty".to_string())),
                    _ => Some(Err(format!("{name} expects a list of numbers"))),
                }
            }

            // -- Math constants and extras -----------------------------------------
            "math.pi" => {
                require_args!(name, 0, args);
                Some(Ok(Value::Float64(std::f64::consts::PI)))
            }
            "math.e" => {
                require_args!(name, 0, args);
                Some(Ok(Value::Float64(std::f64::consts::E)))
            }
            "math.sin" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::Float64(n) => Some(Ok(Value::Float64(n.sin()))),
                    Value::Int64(n) => Some(Ok(Value::Float64((*n as f64).sin()))),
                    _ => Some(Err(format!("{name} expects a numeric argument"))),
                }
            }
            "math.cos" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::Float64(n) => Some(Ok(Value::Float64(n.cos()))),
                    Value::Int64(n) => Some(Ok(Value::Float64((*n as f64).cos()))),
                    _ => Some(Err(format!("{name} expects a numeric argument"))),
                }
            }
            "math.tan" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::Float64(n) => Some(Ok(Value::Float64(n.tan()))),
                    Value::Int64(n) => Some(Ok(Value::Float64((*n as f64).tan()))),
                    _ => Some(Err(format!("{name} expects a numeric argument"))),
                }
            }
            "math.mod" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::Int64(a), Value::Int64(b)) => {
                        if *b == 0 {
                            Some(Err("math.mod: division by zero".to_string()))
                        } else {
                            Some(Ok(Value::Int64(a % b)))
                        }
                    }
                    _ => Some(Err(format!("{name} expects two int64 arguments"))),
                }
            }
            "math.is_even" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::Int64(n) => Some(Ok(Value::Bool(n % 2 == 0))),
                    _ => Some(Err(format!("{name} expects an int64 argument"))),
                }
            }
            "math.is_odd" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::Int64(n) => Some(Ok(Value::Bool(n % 2 != 0))),
                    _ => Some(Err(format!("{name} expects an int64 argument"))),
                }
            }
            "math.sum" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::List(items) => {
                        let mut total: i64 = 0;
                        for item in items {
                            match item {
                                Value::Int64(n) => total += n,
                                _ => {
                                    return Some(Err(
                                        "math.sum: list must contain int64 values".to_string()
                                    ));
                                }
                            }
                        }
                        Some(Ok(Value::Int64(total)))
                    }
                    _ => Some(Err(format!("{name} expects a list argument"))),
                }
            }

            "math.gcd" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::Int64(a), Value::Int64(b)) => {
                        let (mut x, mut y) = (a.abs(), b.abs());
                        while y != 0 {
                            let t = y;
                            y = x % y;
                            x = t;
                        }
                        Some(Ok(Value::Int64(x)))
                    }
                    _ => Some(Err(format!("{name} expects two int64 arguments"))),
                }
            }
            "math.lcm" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::Int64(a), Value::Int64(b)) => {
                        if *a == 0 && *b == 0 {
                            Some(Ok(Value::Int64(0)))
                        } else {
                            let (mut x, mut y) = (a.abs(), b.abs());
                            let product = x * y;
                            while y != 0 {
                                let t = y;
                                y = x % y;
                                x = t;
                            }
                            Some(Ok(Value::Int64(product / x)))
                        }
                    }
                    _ => Some(Err(format!("{name} expects two int64 arguments"))),
                }
            }
            "math.factorial" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::Int64(n) => {
                        if *n < 0 {
                            Some(Err(
                                "math.factorial: argument must be non-negative".to_string()
                            ))
                        } else {
                            let mut result: i64 = 1;
                            for i in 2..=*n {
                                result = result.saturating_mul(i);
                            }
                            Some(Ok(Value::Int64(result)))
                        }
                    }
                    _ => Some(Err(format!("{name} expects an int64 argument"))),
                }
            }
            "math.sign" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::Int64(n) => {
                        let s = if *n < 0 {
                            -1
                        } else if *n > 0 {
                            1
                        } else {
                            0
                        };
                        Some(Ok(Value::Int64(s)))
                    }
                    _ => Some(Err(format!("{name} expects an int64 argument"))),
                }
            }
            "math.to_radians" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::Float64(deg) => Some(Ok(Value::Float64(deg.to_radians()))),
                    _ => Some(Err(format!("{name} expects a float64 argument"))),
                }
            }
            "math.to_degrees" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::Float64(rad) => Some(Ok(Value::Float64(rad.to_degrees()))),
                    _ => Some(Err(format!("{name} expects a float64 argument"))),
                }
            }

            // -- list.enumerate (returns list of [index, value] pairs) ----------
            "list.enumerate" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::List(items) => {
                        let enumerated: Vec<Value> = items
                            .iter()
                            .enumerate()
                            .map(|(i, v)| Value::List(vec![Value::Int64(i as i64), v.clone()]))
                            .collect();
                        Some(Ok(Value::List(enumerated)))
                    }
                    _ => Some(Err(format!("{name} expects a list argument"))),
                }
            }

            // -- list.from_set (convert set to list) ----------------------------
            "list.from_set" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::Set(items) => Some(Ok(Value::List(items.clone()))),
                    _ => Some(Err(format!("{name} expects a set argument"))),
                }
            }

            // -- list.repeat, list.range, list.last_index_of, list.insert_at, list.remove_at, list.swap
            "list.repeat" => {
                require_args!(name, 2, args);
                match &args[1] {
                    Value::Int64(count) => {
                        let n = (*count).max(0) as usize;
                        let items: Vec<Value> =
                            std::iter::repeat(args[0].clone()).take(n).collect();
                        Some(Ok(Value::List(items)))
                    }
                    _ => Some(Err(format!("{name} expects a value and an int64 count"))),
                }
            }
            "list.range" => self.call_builtin("range", args),
            "list.last_index_of" => {
                require_args!(name, 2, args);
                match &args[0] {
                    Value::List(items) => {
                        let idx = items.iter().rposition(|v| v == &args[1]);
                        Some(Ok(match idx {
                            Some(i) => Value::OptionalSome(Box::new(Value::Int64(i as i64))),
                            None => Value::OptionalNone,
                        }))
                    }
                    _ => Some(Err(format!("{name} expects a list as first argument"))),
                }
            }
            "list.insert_at" => {
                require_args!(name, 3, args);
                match (&args[0], &args[1]) {
                    (Value::List(items), Value::Int64(index)) => {
                        let idx = *index as usize;
                        if idx <= items.len() {
                            let mut new_list = items.clone();
                            new_list.insert(idx, args[2].clone());
                            Some(Ok(Value::List(new_list)))
                        } else {
                            Some(Err(format!("{name}: index {index} out of bounds")))
                        }
                    }
                    _ => Some(Err(format!(
                        "{name} expects a list, an int64 index, and a value"
                    ))),
                }
            }
            "list.remove_at" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::List(items), Value::Int64(index)) => {
                        let idx = *index as usize;
                        if idx < items.len() {
                            let mut new_list = items.clone();
                            new_list.remove(idx);
                            Some(Ok(Value::List(new_list)))
                        } else {
                            Some(Err(format!("{name}: index {index} out of bounds")))
                        }
                    }
                    _ => Some(Err(format!("{name} expects a list and an int64 index"))),
                }
            }
            "list.swap" => {
                require_args!(name, 3, args);
                match (&args[0], &args[1], &args[2]) {
                    (Value::List(items), Value::Int64(i), Value::Int64(j)) => {
                        let a = *i as usize;
                        let b = *j as usize;
                        if a >= items.len() || b >= items.len() {
                            Some(Err(format!("{name}: index out of bounds")))
                        } else {
                            let mut new_list = items.clone();
                            new_list.swap(a, b);
                            Some(Ok(Value::List(new_list)))
                        }
                    }
                    _ => Some(Err(format!("{name} expects a list and two int64 indices"))),
                }
            }

            // -- Additional string operations (stdlib/string.jett) ------------
            "string.reverse" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::String(s) => {
                        let reversed: String = s.chars().rev().collect();
                        Some(Ok(Value::String(reversed)))
                    }
                    _ => Some(Err(format!("{name} expects a string argument"))),
                }
            }

            "string.after" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::String(s), Value::String(marker)) => {
                        let result = if let Some(pos) = s.find(marker.as_str()) {
                            s[pos + marker.len()..].to_string()
                        } else {
                            String::new()
                        };
                        Some(Ok(Value::String(result)))
                    }
                    _ => Some(Err(format!("{name} expects two string arguments"))),
                }
            }

            "string.before" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::String(s), Value::String(marker)) => {
                        let result = if let Some(pos) = s.find(marker.as_str()) {
                            s[..pos].to_string()
                        } else {
                            s.clone()
                        };
                        Some(Ok(Value::String(result)))
                    }
                    _ => Some(Err(format!("{name} expects two string arguments"))),
                }
            }

            "string.trim_start" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::String(s) => Some(Ok(Value::String(s.trim_start().to_string()))),
                    _ => Some(Err(format!("{name} expects a string argument"))),
                }
            }

            "string.trim_end" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::String(s) => Some(Ok(Value::String(s.trim_end().to_string()))),
                    _ => Some(Err(format!("{name} expects a string argument"))),
                }
            }

            // string.chars / string.words / string.lines — yield list[string]
            "string.chars" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::String(s) => {
                        // Yields grapheme clusters; approximate with chars for now
                        let chars: Vec<Value> =
                            s.chars().map(|c| Value::String(c.to_string())).collect();
                        Some(Ok(Value::List(chars)))
                    }
                    _ => Some(Err(format!("{name} expects a string argument"))),
                }
            }

            "string.words" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::String(s) => {
                        let words: Vec<Value> = s
                            .split_whitespace()
                            .map(|w| Value::String(w.to_string()))
                            .collect();
                        Some(Ok(Value::List(words)))
                    }
                    _ => Some(Err(format!("{name} expects a string argument"))),
                }
            }

            "string.lines" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::String(s) => {
                        let lines: Vec<Value> =
                            s.lines().map(|l| Value::String(l.to_string())).collect();
                        Some(Ok(Value::List(lines)))
                    }
                    _ => Some(Err(format!("{name} expects a string argument"))),
                }
            }

            // -- UUID operations (stdlib/uuid.jett) ---------------------------
            "uuid.new" => {
                require_args!(name, 0, args);
                // Generate a UUID v4 using rand
                let mut rng = rand::thread_rng();
                let mut b = [0u8; 16];
                for byte in b.iter_mut() {
                    *byte = rand::Rng::r#gen(&mut rng);
                }
                // Set version 4 bits
                b[6] = (b[6] & 0x0F) | 0x40;
                // Set variant bits
                b[8] = (b[8] & 0x3F) | 0x80;
                let uuid = format!(
                    "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
                    b[0],
                    b[1],
                    b[2],
                    b[3],
                    b[4],
                    b[5],
                    b[6],
                    b[7],
                    b[8],
                    b[9],
                    b[10],
                    b[11],
                    b[12],
                    b[13],
                    b[14],
                    b[15]
                );
                Some(Ok(Value::String(uuid)))
            }

            // -- Additional char-level string operations -----------------------
            "string.take_chars" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::String(s), Value::Int64(n)) => {
                        let n = (*n).max(0) as usize;
                        let result: String = s.chars().take(n).collect();
                        Some(Ok(Value::String(result)))
                    }
                    _ => Some(Err(format!("{name} expects a string and an int64"))),
                }
            }

            "string.take_last_chars" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::String(s), Value::Int64(n)) => {
                        let n = (*n).max(0) as usize;
                        let chars: Vec<char> = s.chars().collect();
                        let start = chars.len().saturating_sub(n);
                        let result: String = chars[start..].iter().collect();
                        Some(Ok(Value::String(result)))
                    }
                    _ => Some(Err(format!("{name} expects a string and an int64"))),
                }
            }

            "string.drop_chars" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::String(s), Value::Int64(n)) => {
                        let n = (*n).max(0) as usize;
                        let result: String = s.chars().skip(n).collect();
                        Some(Ok(Value::String(result)))
                    }
                    _ => Some(Err(format!("{name} expects a string and an int64"))),
                }
            }

            "string.char_at" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::String(s), Value::Int64(i)) => {
                        let result = if *i < 0 {
                            Value::OptionalNone
                        } else {
                            match s.chars().nth(*i as usize) {
                                Some(c) => {
                                    Value::OptionalSome(Box::new(Value::String(c.to_string())))
                                }
                                None => Value::OptionalNone,
                            }
                        };
                        Some(Ok(result))
                    }
                    _ => Some(Err(format!("{name} expects a string and an int64 index"))),
                }
            }

            "string.index_of" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::String(haystack), Value::String(needle)) => {
                        let result = match haystack.find(needle.as_str()) {
                            Some(pos) => {
                                // Convert byte offset to char index.
                                let char_idx = haystack[..pos].chars().count() as i64;
                                Value::OptionalSome(Box::new(Value::Int64(char_idx)))
                            }
                            None => Value::OptionalNone,
                        };
                        Some(Ok(result))
                    }
                    _ => Some(Err(format!("{name} expects two string arguments"))),
                }
            }
            "string.count" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::String(haystack), Value::String(needle)) => {
                        let count = if needle.is_empty() {
                            0
                        } else {
                            haystack.matches(needle.as_str()).count() as i64
                        };
                        Some(Ok(Value::Int64(count)))
                    }
                    _ => Some(Err(format!("{name} expects two string arguments"))),
                }
            }
            "string.to_upper_first" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::String(s) => {
                        let mut chars = s.chars();
                        let result = match chars.next() {
                            Some(c) => {
                                let upper: String = c.to_uppercase().collect();
                                format!("{upper}{}", chars.as_str())
                            }
                            None => String::new(),
                        };
                        Some(Ok(Value::String(result)))
                    }
                    _ => Some(Err(format!("{name} expects a string argument"))),
                }
            }
            "string.to_lower_first" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::String(s) => {
                        let mut chars = s.chars();
                        let result = match chars.next() {
                            Some(c) => {
                                let lower: String = c.to_lowercase().collect();
                                format!("{lower}{}", chars.as_str())
                            }
                            None => String::new(),
                        };
                        Some(Ok(Value::String(result)))
                    }
                    _ => Some(Err(format!("{name} expects a string argument"))),
                }
            }

            // -- String formatting operations ----------------------------------
            "string.center" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::String(s), Value::Int64(width)) => {
                        let width = (*width).max(0) as usize;
                        let char_len = s.chars().count();
                        if char_len >= width {
                            Some(Ok(Value::String(s.clone())))
                        } else {
                            let total_pad = width - char_len;
                            let left_pad = total_pad / 2;
                            let right_pad = total_pad - left_pad;
                            let left: String = std::iter::repeat(' ').take(left_pad).collect();
                            let right: String = std::iter::repeat(' ').take(right_pad).collect();
                            Some(Ok(Value::String(format!("{left}{s}{right}"))))
                        }
                    }
                    _ => Some(Err(format!("{name} expects a string and an int64 width"))),
                }
            }

            "string.ljust" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::String(s), Value::Int64(width)) => {
                        let width = (*width).max(0) as usize;
                        let char_len = s.chars().count();
                        if char_len >= width {
                            Some(Ok(Value::String(s.clone())))
                        } else {
                            let padding: String =
                                std::iter::repeat(' ').take(width - char_len).collect();
                            Some(Ok(Value::String(format!("{s}{padding}"))))
                        }
                    }
                    _ => Some(Err(format!("{name} expects a string and an int64 width"))),
                }
            }

            "string.rjust" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::String(s), Value::Int64(width)) => {
                        let width = (*width).max(0) as usize;
                        let char_len = s.chars().count();
                        if char_len >= width {
                            Some(Ok(Value::String(s.clone())))
                        } else {
                            let padding: String =
                                std::iter::repeat(' ').take(width - char_len).collect();
                            Some(Ok(Value::String(format!("{padding}{s}"))))
                        }
                    }
                    _ => Some(Err(format!("{name} expects a string and an int64 width"))),
                }
            }

            "string.zfill" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::String(s), Value::Int64(width)) => {
                        let width = (*width).max(0) as usize;
                        let char_len = s.chars().count();
                        if char_len >= width {
                            Some(Ok(Value::String(s.clone())))
                        } else {
                            // Handle optional leading sign
                            let (sign, digits) = if s.starts_with('-') || s.starts_with('+') {
                                (&s[..1], &s[1..])
                            } else {
                                ("", s.as_str())
                            };
                            let zeros: String =
                                std::iter::repeat('0').take(width - char_len).collect();
                            Some(Ok(Value::String(format!("{sign}{zeros}{digits}"))))
                        }
                    }
                    _ => Some(Err(format!("{name} expects a string and an int64 width"))),
                }
            }

            "string.remove_prefix" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::String(s), Value::String(prefix)) => {
                        let result = s.strip_prefix(prefix.as_str()).unwrap_or(s).to_string();
                        Some(Ok(Value::String(result)))
                    }
                    _ => Some(Err(format!("{name} expects two string arguments"))),
                }
            }

            "string.remove_suffix" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::String(s), Value::String(suffix)) => {
                        let result = s.strip_suffix(suffix.as_str()).unwrap_or(s).to_string();
                        Some(Ok(Value::String(result)))
                    }
                    _ => Some(Err(format!("{name} expects two string arguments"))),
                }
            }

            "string.is_numeric" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::String(s) => Some(Ok(Value::Bool(
                        !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()),
                    ))),
                    _ => Some(Err(format!("{name} expects a string argument"))),
                }
            }

            "string.is_alpha" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::String(s) => Some(Ok(Value::Bool(
                        !s.is_empty() && s.chars().all(|c| c.is_alphabetic()),
                    ))),
                    _ => Some(Err(format!("{name} expects a string argument"))),
                }
            }

            // -- Encoding operations (stdlib/encoding.jett) -------------------
            "encoding.base64_encode" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::String(s) => {
                        let encoded = base64_encode(s.as_bytes());
                        Some(Ok(Value::String(encoded)))
                    }
                    _ => Some(Err(format!("{name} expects a string argument"))),
                }
            }

            "encoding.base64_decode" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::String(s) => match base64_decode(s) {
                        Ok(bytes) => match String::from_utf8(bytes) {
                            Ok(decoded) => Some(Ok(Value::String(decoded))),
                            Err(_) => Some(Err(
                                "encoding.base64_decode: decoded bytes are not valid UTF-8"
                                    .to_string(),
                            )),
                        },
                        Err(e) => Some(Err(format!("encoding.base64_decode: {e}"))),
                    },
                    _ => Some(Err(format!("{name} expects a string argument"))),
                }
            }

            "encoding.hex_encode" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::String(s) => {
                        let hex: String = s.bytes().map(|b| format!("{b:02x}")).collect();
                        Some(Ok(Value::String(hex)))
                    }
                    _ => Some(Err(format!("{name} expects a string argument"))),
                }
            }

            "encoding.hex_decode" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::String(s) => {
                        if s.len() % 2 != 0 {
                            return Some(Err(
                                "encoding.hex_decode: odd-length hex string".to_string()
                            ));
                        }
                        let bytes: Result<Vec<u8>, _> = (0..s.len() / 2)
                            .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16))
                            .collect();
                        match bytes {
                            Ok(b) => match String::from_utf8(b) {
                                Ok(decoded) => Some(Ok(Value::String(decoded))),
                                Err(_) => {
                                    Some(Err("encoding.hex_decode: bytes are not valid UTF-8"
                                        .to_string()))
                                }
                            },
                            Err(_) => Some(Err(
                                "encoding.hex_decode: invalid hex characters".to_string()
                            )),
                        }
                    }
                    _ => Some(Err(format!("{name} expects a string argument"))),
                }
            }

            "encoding.url_encode" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::String(s) => {
                        let encoded: String = s
                            .bytes()
                            .flat_map(|b| {
                                if b.is_ascii_alphanumeric()
                                    || b == b'-'
                                    || b == b'_'
                                    || b == b'.'
                                    || b == b'~'
                                {
                                    vec![b as char]
                                } else {
                                    format!("%{b:02X}").chars().collect()
                                }
                            })
                            .collect();
                        Some(Ok(Value::String(encoded)))
                    }
                    _ => Some(Err(format!("{name} expects a string argument"))),
                }
            }

            "encoding.url_decode" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::String(s) => {
                        let bytes = s.as_bytes();
                        let mut result = Vec::new();
                        let mut i = 0;
                        while i < bytes.len() {
                            if bytes[i] == b'%' && i + 2 < bytes.len() {
                                if let Ok(b) = u8::from_str_radix(
                                    std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""),
                                    16,
                                ) {
                                    result.push(b);
                                    i += 3;
                                    continue;
                                }
                            } else if bytes[i] == b'+' {
                                result.push(b' ');
                                i += 1;
                                continue;
                            }
                            result.push(bytes[i]);
                            i += 1;
                        }
                        match String::from_utf8(result) {
                            Ok(decoded) => Some(Ok(Value::String(decoded))),
                            Err(_) => Some(Err(
                                "encoding.url_decode: result is not valid UTF-8".to_string()
                            )),
                        }
                    }
                    _ => Some(Err(format!("{name} expects a string argument"))),
                }
            }

            // -- Crypto operations (stdlib/crypto.jett) -----------------------
            "crypto.sha256" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::String(s) => Some(Ok(Value::String(sha256_hash(s.as_bytes())))),
                    _ => Some(Err(format!("{name} expects a string argument"))),
                }
            }

            "crypto.md5" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::String(s) => Some(Ok(Value::String(md5_hash(s.as_bytes())))),
                    _ => Some(Err(format!("{name} expects a string argument"))),
                }
            }

            // -- Time operations (stdlib/time.jett) -------------------------------
            "time.now_ms" => {
                require_args!(name, 0, args);
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64;
                Some(Ok(Value::Int64(now)))
            }
            "time.now_s" => {
                require_args!(name, 0, args);
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;
                Some(Ok(Value::Int64(now)))
            }

            // -- OS operations (stdlib/os.jett) ---------------------------------
            "os.env" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::String(key) => {
                        let result = match std::env::var(key) {
                            Ok(val) => Value::OptionalSome(Box::new(Value::String(val))),
                            Err(_) => Value::OptionalNone,
                        };
                        Some(Ok(result))
                    }
                    _ => Some(Err(format!("{name} expects a string argument"))),
                }
            }
            "os.args" => {
                require_args!(name, 0, args);
                let args_list: Vec<Value> = std::env::args().map(Value::String).collect();
                Some(Ok(Value::List(args_list)))
            }

            // -- CSV operations (stdlib/csv.jett) ---------------------------------
            "csv.parse" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::String(s) => {
                        let rows: Vec<Value> = s
                            .lines()
                            .filter(|line| !line.is_empty())
                            .map(|line| {
                                let cols: Vec<Value> = parse_csv_line(line)
                                    .into_iter()
                                    .map(Value::String)
                                    .collect();
                                Value::List(cols)
                            })
                            .collect();
                        Some(Ok(Value::List(rows)))
                    }
                    _ => Some(Err(format!("{name} expects a string argument"))),
                }
            }

            "csv.stringify" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::List(rows) => {
                        let mut lines = Vec::new();
                        for row in rows {
                            match row {
                                Value::List(cols) => {
                                    let fields: Vec<String> = cols
                                        .iter()
                                        .map(|v| match v {
                                            Value::String(s) => csv_quote_field(s),
                                            other => csv_quote_field(&format!("{other}")),
                                        })
                                        .collect();
                                    lines.push(fields.join(","));
                                }
                                _ => {
                                    return Some(Err(format!("{name} expects list[list[string]]")));
                                }
                            }
                        }
                        Some(Ok(Value::String(lines.join("\n"))))
                    }
                    _ => Some(Err(format!("{name} expects a list argument"))),
                }
            }

            "csv.parse_with_header" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::String(s) => {
                        let mut line_iter = s.lines().filter(|line| !line.is_empty());
                        let headers: Vec<String> = match line_iter.next() {
                            Some(header_line) => parse_csv_line(header_line),
                            None => return Some(Ok(Value::List(Vec::new()))),
                        };
                        let rows: Vec<Value> = line_iter
                            .map(|line| {
                                let cols = parse_csv_line(line);
                                let entries: Vec<(Value, Value)> = headers
                                    .iter()
                                    .zip(cols.into_iter())
                                    .map(|(h, c)| (Value::String(h.clone()), Value::String(c)))
                                    .collect();
                                Value::Map(entries)
                            })
                            .collect();
                        Some(Ok(Value::List(rows)))
                    }
                    _ => Some(Err(format!("{name} expects a string argument"))),
                }
            }

            // -- Range generation ---------------------------------------------------
            "range" => {
                match args.len() {
                    // range(end) — 0 to end exclusive
                    1 => match &args[0] {
                        Value::Int64(end) => {
                            let items: Vec<Value> = (0..*end).map(Value::Int64).collect();
                            Some(Ok(Value::List(items)))
                        }
                        _ => Some(Err(format!("{name} expects int64 arguments"))),
                    },
                    // range(start, end) — start to end exclusive
                    2 => match (&args[0], &args[1]) {
                        (Value::Int64(start), Value::Int64(end)) => {
                            let items: Vec<Value> = (*start..*end).map(Value::Int64).collect();
                            Some(Ok(Value::List(items)))
                        }
                        _ => Some(Err(format!("{name} expects int64 arguments"))),
                    },
                    // range(start, end, step)
                    3 => match (&args[0], &args[1], &args[2]) {
                        (Value::Int64(start), Value::Int64(end), Value::Int64(step)) => {
                            if *step == 0 {
                                return Some(Err("range step cannot be zero".to_string()));
                            }
                            let mut items = Vec::new();
                            let mut i = *start;
                            if *step > 0 {
                                while i < *end {
                                    items.push(Value::Int64(i));
                                    i += step;
                                }
                            } else {
                                while i > *end {
                                    items.push(Value::Int64(i));
                                    i += step;
                                }
                            }
                            Some(Ok(Value::List(items)))
                        }
                        _ => Some(Err(format!("{name} expects int64 arguments"))),
                    },
                    _ => Some(Err(format!("{name} expects 1, 2, or 3 arguments"))),
                }
            }

            // -- Bytes operations (stdlib/bytes.jett) ---------------------------
            "bytes.new" => {
                require_args!(name, 0, args);
                Some(Ok(Value::Bytes(Vec::new())))
            }

            "bytes.length" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::Bytes(b) => Some(Ok(Value::Int64(b.len() as i64))),
                    _ => Some(Err(format!("{name} expects a bytes argument"))),
                }
            }

            "bytes.slice" => {
                require_args!(name, 3, args);
                match (&args[0], &args[1], &args[2]) {
                    (Value::Bytes(b), Value::Int64(start), Value::Int64(end)) => {
                        let len = b.len() as i64;
                        let start = (*start).clamp(0, len) as usize;
                        let end = (*end).clamp(0, len) as usize;
                        let result = b[start.min(end)..end].to_vec();
                        Some(Ok(Value::Bytes(result)))
                    }
                    _ => Some(Err(format!(
                        "{name} expects a bytes value and two int64 indices"
                    ))),
                }
            }

            "bytes.concat" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::Bytes(a), Value::Bytes(b)) => {
                        let mut result = a.clone();
                        result.extend(b.iter());
                        Some(Ok(Value::Bytes(result)))
                    }
                    _ => Some(Err(format!("{name} expects two bytes arguments"))),
                }
            }

            "bytes.from_string" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::String(s) => Some(Ok(Value::Bytes(s.as_bytes().to_vec()))),
                    _ => Some(Err(format!("{name} expects a string argument"))),
                }
            }

            "bytes.to_string" => {
                require_args!(name, 1, args);
                match &args[0] {
                    Value::Bytes(b) => match String::from_utf8(b.clone()) {
                        Ok(s) => Some(Ok(Value::ResultOk(Box::new(Value::String(s))))),
                        Err(e) => Some(Ok(Value::ResultFail(Box::new(Value::String(format!(
                            "invalid UTF-8: {e}"
                        )))))),
                    },
                    _ => Some(Err(format!("{name} expects a bytes argument"))),
                }
            }

            "bytes.get" => {
                require_args!(name, 2, args);
                match (&args[0], &args[1]) {
                    (Value::Bytes(b), Value::Int64(index)) => {
                        let idx = *index as usize;
                        if idx < b.len() {
                            Some(Ok(Value::OptionalSome(Box::new(Value::Int64(
                                b[idx] as i64,
                            )))))
                        } else {
                            Some(Ok(Value::OptionalNone))
                        }
                    }
                    _ => Some(Err(format!(
                        "{name} expects a bytes value and an int64 index"
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
        self.call_function_with_type_args(name, &[], args)
    }

    fn call_function_with_type_args(
        &mut self,
        name: &str,
        type_args: &[TypeExpr],
        args: Vec<Value>,
    ) -> Result<Value, String> {
        // Check higher-order built-ins first (require &mut self).
        if let Some(result) = self.call_higher_order_builtin(name, args.clone()) {
            return result;
        }
        if let Some(result) = self.call_builtin_with_type_args(name, type_args, &args) {
            return result;
        }
        // Check built-in functions first.
        if let Some(result) = self.call_builtin(name, &args) {
            return result;
        }

        // Check if the name refers to a variable holding a function value (closure).
        if let Some(fn_val) = self.get_variable(name).cloned() {
            if matches!(fn_val, Value::Function { .. }) {
                return self.call_fn_value(fn_val, args);
            }
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

        let type_scope = self.type_scope_for_function(&func, type_args)?;
        self.type_arg_scopes.push(type_scope);

        // Create a new scope and bind parameters.
        self.push_scope();
        let mut bind_error = None;
        for (param, arg) in func.params.iter().zip(args) {
            let type_name = type_expr_name(&param.ty);
            if let Err(message) = self.check_refinement(&type_name, &arg) {
                bind_error = Some(message);
                break;
            }
            self.set_variable(&param.name.name, arg);
        }
        if let Some(message) = bind_error {
            self.pop_scope();
            self.type_arg_scopes.pop();
            return Err(message);
        }

        let result = self.exec_block_inner(&func.body);
        self.pop_scope();
        self.type_arg_scopes.pop();
        let result = result?;

        let value = match result {
            Some(Signal::Return(v)) => v,
            Some(Signal::Default(_)) => {
                return Err("`default` can only be used inside a `handle` block".to_string());
            }
            _ => Value::Nothing,
        };

        if let Some(return_type) = &func.return_type {
            let type_name = type_expr_name(return_type);
            self.check_refinement(&type_name, &value)?;
        }

        Ok(value)
    }

    fn type_scope_for_function(
        &self,
        func: &FunctionDef,
        type_args: &[TypeExpr],
    ) -> Result<HashMap<String, TypeExpr>, String> {
        if func.type_params.is_empty() && type_args.is_empty() {
            return Ok(HashMap::new());
        }
        if func.type_params.len() != type_args.len() {
            return Err(format!(
                "function '{}' expects {} type argument(s), got {}",
                func.name.name,
                func.type_params.len(),
                type_args.len()
            ));
        }

        let raw: HashMap<String, TypeExpr> = func
            .type_params
            .iter()
            .zip(type_args.iter())
            .map(|(param, arg)| (param.name.clone(), self.substitute_type_expr(arg)))
            .collect();

        Ok(raw
            .iter()
            .map(|(name, ty)| (name.clone(), self.substitute_type_expr_with_map(ty, &raw)))
            .collect())
    }

    /// Try to call a higher-order built-in that requires `&mut self` (because
    /// it needs to invoke a user-supplied function value).  Returns `None` if
    /// the name is not a higher-order built-in.
    fn call_higher_order_builtin(
        &mut self,
        name: &str,
        args: Vec<Value>,
    ) -> Option<Result<Value, String>> {
        match name {
            "list.filter" => {
                if args.len() != 2 {
                    return Some(Err(format!(
                        "list.filter expects 2 arguments, got {}",
                        args.len()
                    )));
                }
                let items = match &args[0] {
                    Value::List(v) => v.clone(),
                    _ => return Some(Err("list.filter: first argument must be a list".into())),
                };
                let fn_val = args[1].clone();
                let mut result = Vec::new();
                for item in items {
                    match self.call_fn_value(fn_val.clone(), vec![item.clone()]) {
                        Ok(Value::Bool(true)) => result.push(item),
                        Ok(Value::Bool(false)) => {}
                        Ok(other) => {
                            return Some(Err(format!(
                                "list.filter: predicate returned {other}, expected bool"
                            )));
                        }
                        Err(e) => return Some(Err(e)),
                    }
                }
                Some(Ok(Value::List(result)))
            }
            "list.map" => {
                if args.len() != 2 {
                    return Some(Err(format!(
                        "list.map expects 2 arguments, got {}",
                        args.len()
                    )));
                }
                let items = match &args[0] {
                    Value::List(v) => v.clone(),
                    _ => return Some(Err("list.map: first argument must be a list".into())),
                };
                let fn_val = args[1].clone();
                let mut result = Vec::new();
                for item in items {
                    match self.call_fn_value(fn_val.clone(), vec![item]) {
                        Ok(v) => result.push(v),
                        Err(e) => return Some(Err(e)),
                    }
                }
                Some(Ok(Value::List(result)))
            }
            "list.find" => {
                if args.len() != 2 {
                    return Some(Err(format!(
                        "list.find expects 2 arguments, got {}",
                        args.len()
                    )));
                }
                let items = match &args[0] {
                    Value::List(v) => v.clone(),
                    _ => return Some(Err("list.find: first argument must be a list".into())),
                };
                let fn_val = args[1].clone();
                for item in items {
                    match self.call_fn_value(fn_val.clone(), vec![item.clone()]) {
                        Ok(Value::Bool(true)) => {
                            return Some(Ok(Value::OptionalSome(Box::new(item))));
                        }
                        Ok(Value::Bool(false)) => {}
                        Ok(other) => {
                            return Some(Err(format!(
                                "list.find: predicate returned {other}, expected bool"
                            )));
                        }
                        Err(e) => return Some(Err(e)),
                    }
                }
                Some(Ok(Value::OptionalNone))
            }
            "list.sort_by" => {
                if args.len() != 2 {
                    return Some(Err(format!(
                        "list.sort_by expects 2 arguments, got {}",
                        args.len()
                    )));
                }
                let items = match &args[0] {
                    Value::List(v) => v.clone(),
                    _ => return Some(Err("list.sort_by: first argument must be a list".into())),
                };
                let fn_val = args[1].clone();
                // Compute keys for each item.
                let mut keyed: Vec<(i64, Value)> = Vec::new();
                for item in items {
                    match self.call_fn_value(fn_val.clone(), vec![item.clone()]) {
                        Ok(Value::Int64(k)) => keyed.push((k, item)),
                        Ok(other) => {
                            return Some(Err(format!(
                                "list.sort_by: key function returned {other}, expected int64"
                            )));
                        }
                        Err(e) => return Some(Err(e)),
                    }
                }
                keyed.sort_by_key(|(k, _)| *k);
                Some(Ok(Value::List(keyed.into_iter().map(|(_, v)| v).collect())))
            }
            "list.all" => {
                if args.len() != 2 {
                    return Some(Err(format!(
                        "list.all expects 2 arguments, got {}",
                        args.len()
                    )));
                }
                let items = match &args[0] {
                    Value::List(v) => v.clone(),
                    _ => return Some(Err("list.all: first argument must be a list".into())),
                };
                let fn_val = args[1].clone();
                for item in items {
                    match self.call_fn_value(fn_val.clone(), vec![item]) {
                        Ok(Value::Bool(false)) => return Some(Ok(Value::Bool(false))),
                        Ok(Value::Bool(true)) => {}
                        Ok(other) => {
                            return Some(Err(format!(
                                "list.all: predicate returned {other}, expected bool"
                            )));
                        }
                        Err(e) => return Some(Err(e)),
                    }
                }
                Some(Ok(Value::Bool(true)))
            }
            "list.any" => {
                if args.len() != 2 {
                    return Some(Err(format!(
                        "list.any expects 2 arguments, got {}",
                        args.len()
                    )));
                }
                let items = match &args[0] {
                    Value::List(v) => v.clone(),
                    _ => return Some(Err("list.any: first argument must be a list".into())),
                };
                let fn_val = args[1].clone();
                for item in items {
                    match self.call_fn_value(fn_val.clone(), vec![item]) {
                        Ok(Value::Bool(true)) => return Some(Ok(Value::Bool(true))),
                        Ok(Value::Bool(false)) => {}
                        Ok(other) => {
                            return Some(Err(format!(
                                "list.any: predicate returned {other}, expected bool"
                            )));
                        }
                        Err(e) => return Some(Err(e)),
                    }
                }
                Some(Ok(Value::Bool(false)))
            }
            "list.count" => {
                if args.len() != 2 {
                    return Some(Err(format!(
                        "list.count expects 2 arguments, got {}",
                        args.len()
                    )));
                }
                let items = match &args[0] {
                    Value::List(v) => v.clone(),
                    _ => return Some(Err("list.count: first argument must be a list".into())),
                };
                let fn_val = args[1].clone();
                let mut count = 0i64;
                for item in items {
                    match self.call_fn_value(fn_val.clone(), vec![item]) {
                        Ok(Value::Bool(true)) => count += 1,
                        Ok(Value::Bool(false)) => {}
                        Ok(other) => {
                            return Some(Err(format!(
                                "list.count: predicate returned {other}, expected bool"
                            )));
                        }
                        Err(e) => return Some(Err(e)),
                    }
                }
                Some(Ok(Value::Int64(count)))
            }
            "list.sum" => {
                if args.len() != 1 {
                    return Some(Err(format!(
                        "list.sum expects 1 argument, got {}",
                        args.len()
                    )));
                }
                match &args[0] {
                    Value::List(items) => {
                        if items.is_empty() {
                            return Some(Ok(Value::Int64(0)));
                        }
                        // Detect int64 vs float64 from first element.
                        match &items[0] {
                            Value::Int64(_) => {
                                let mut total = 0i64;
                                for item in items {
                                    match item {
                                        Value::Int64(n) => total += n,
                                        _ => return Some(Err("list.sum: mixed types".into())),
                                    }
                                }
                                Some(Ok(Value::Int64(total)))
                            }
                            Value::Float64(_) => {
                                let mut total = 0.0f64;
                                for item in items {
                                    match item {
                                        Value::Float64(n) => total += n,
                                        _ => return Some(Err("list.sum: mixed types".into())),
                                    }
                                }
                                Some(Ok(Value::Float64(total)))
                            }
                            _ => Some(Err(
                                "list.sum: list elements must be int64 or float64".into()
                            )),
                        }
                    }
                    _ => Some(Err("list.sum: argument must be a list".into())),
                }
            }
            "list.group_by" => {
                if args.len() != 2 {
                    return Some(Err(format!(
                        "list.group_by expects 2 arguments, got {}",
                        args.len()
                    )));
                }
                let items = match &args[0] {
                    Value::List(v) => v.clone(),
                    _ => return Some(Err("list.group_by: first argument must be a list".into())),
                };
                let fn_val = args[1].clone();
                let mut groups: Vec<(Value, Value)> = Vec::new();
                for item in items {
                    let key = match self.call_fn_value(fn_val.clone(), vec![item.clone()]) {
                        Ok(k) => k,
                        Err(e) => return Some(Err(e)),
                    };
                    if let Some((_, group)) = groups.iter_mut().find(|(k, _)| k == &key) {
                        if let Value::List(v) = group {
                            v.push(item);
                        }
                    } else {
                        groups.push((key, Value::List(vec![item])));
                    }
                }
                Some(Ok(Value::Map(groups)))
            }

            // list.reduce[T, U](list, initial, fn(acc, item) -> acc)
            "list.reduce" => {
                if args.len() != 3 {
                    return Some(Err(format!(
                        "list.reduce expects 3 arguments (list, initial, fn), got {}",
                        args.len()
                    )));
                }
                let items = match &args[0] {
                    Value::List(v) => v.clone(),
                    _ => return Some(Err("list.reduce: first argument must be a list".into())),
                };
                let mut acc = args[1].clone();
                let fn_val = args[2].clone();
                for item in items {
                    acc = match self.call_fn_value(fn_val.clone(), vec![acc, item]) {
                        Ok(v) => v,
                        Err(e) => return Some(Err(e)),
                    };
                }
                Some(Ok(acc))
            }

            "list.flat_map" => {
                if args.len() != 2 {
                    return Some(Err(format!(
                        "list.flat_map expects 2 arguments, got {}",
                        args.len()
                    )));
                }
                let items = match &args[0] {
                    Value::List(v) => v.clone(),
                    _ => return Some(Err("list.flat_map: first argument must be a list".into())),
                };
                let fn_val = args[1].clone();
                let mut result = Vec::new();
                for item in items {
                    match self.call_fn_value(fn_val.clone(), vec![item]) {
                        Ok(Value::List(inner)) => result.extend(inner),
                        Ok(other) => {
                            return Some(Err(format!(
                                "list.flat_map: function returned {other}, expected a list"
                            )));
                        }
                        Err(e) => return Some(Err(e)),
                    }
                }
                Some(Ok(Value::List(result)))
            }

            // -- Map higher-order builtins ------------------------------------
            "map.filter" => {
                if args.len() != 2 {
                    return Some(Err(format!(
                        "map.filter expects 2 arguments, got {}",
                        args.len()
                    )));
                }
                let entries = match &args[0] {
                    Value::Map(v) => v.clone(),
                    _ => return Some(Err("map.filter: first argument must be a map".into())),
                };
                let fn_val = args[1].clone();
                let mut result = Vec::new();
                for (k, v) in entries {
                    match self.call_fn_value(fn_val.clone(), vec![k.clone(), v.clone()]) {
                        Ok(Value::Bool(true)) => result.push((k, v)),
                        Ok(Value::Bool(false)) => {}
                        Ok(other) => {
                            return Some(Err(format!(
                                "map.filter: predicate returned {other}, expected bool"
                            )));
                        }
                        Err(e) => return Some(Err(e)),
                    }
                }
                Some(Ok(Value::Map(result)))
            }
            "map.map_values" => {
                if args.len() != 2 {
                    return Some(Err(format!(
                        "map.map_values expects 2 arguments, got {}",
                        args.len()
                    )));
                }
                let entries = match &args[0] {
                    Value::Map(v) => v.clone(),
                    _ => return Some(Err("map.map_values: first argument must be a map".into())),
                };
                let fn_val = args[1].clone();
                let mut result = Vec::new();
                for (k, v) in entries {
                    match self.call_fn_value(fn_val.clone(), vec![v]) {
                        Ok(new_v) => result.push((k, new_v)),
                        Err(e) => return Some(Err(e)),
                    }
                }
                Some(Ok(Value::Map(result)))
            }
            "map.for_each" => {
                if args.len() != 2 {
                    return Some(Err(format!(
                        "map.for_each expects 2 arguments, got {}",
                        args.len()
                    )));
                }
                let entries = match &args[0] {
                    Value::Map(v) => v.clone(),
                    _ => return Some(Err("map.for_each: first argument must be a map".into())),
                };
                let fn_val = args[1].clone();
                for (k, v) in entries {
                    match self.call_fn_value(fn_val.clone(), vec![k, v]) {
                        Ok(_) => {}
                        Err(e) => return Some(Err(e)),
                    }
                }
                Some(Ok(Value::Nothing))
            }

            _ => None,
        }
    }

    /// Call a `Value::Function` (inline function) with the given arguments.
    fn call_fn_value(&mut self, fn_val: Value, args: Vec<Value>) -> Result<Value, String> {
        match fn_val {
            Value::Function {
                params,
                body,
                captures,
            } => {
                if args.len() != params.len() {
                    return Err(format!(
                        "inline function expects {} argument(s), got {}",
                        params.len(),
                        args.len()
                    ));
                }
                // Push the captured environment as a scope, then the parameter scope on top.
                self.push_scope();
                for (name, value) in &captures {
                    self.set_variable(name, value.clone());
                }
                self.push_scope();
                for (param, arg) in params.iter().zip(args) {
                    self.set_variable(&param.name.name, arg);
                }
                let result = self.exec_block_inner(&body)?;
                self.pop_scope(); // params
                self.pop_scope(); // captures
                Ok(match result {
                    Some(Signal::Return(v)) => v,
                    _ => Value::Nothing,
                })
            }
            other => Err(format!("expected function value, got {other}")),
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

    fn construct_bitfield(
        &mut self,
        bitfield_name: &str,
        args: &[CallArg],
        arg_values: Vec<Value>,
    ) -> Result<Value, String> {
        let bitfield = self
            .bitfields
            .get(bitfield_name)
            .ok_or_else(|| format!("undefined bitfield '{bitfield_name}'"))?
            .clone();

        if args.len() > bitfield.fields.len() {
            return Err(format!(
                "bitfield '{}' expects {} field argument(s), got {}",
                bitfield_name,
                bitfield.fields.len(),
                args.len()
            ));
        }

        let mut fields: Vec<Option<Value>> = vec![None; bitfield.fields.len()];
        let mut requires_runtime_validation = false;
        for (arg, value) in args.iter().zip(arg_values) {
            let field_index = if let Some(name) = &arg.name {
                bitfield
                    .fields
                    .iter()
                    .position(|field| field.name.name == name.name)
                    .ok_or_else(|| {
                        format!("bitfield '{bitfield_name}' has no field '{}'", name.name)
                    })?
            } else {
                let Some(index) = fields.iter().position(|value| value.is_none()) else {
                    return Err(format!(
                        "bitfield '{}' expects {} field argument(s), got {}",
                        bitfield_name,
                        bitfield.fields.len(),
                        args.len()
                    ));
                };
                index
            };

            if fields[field_index].is_some() {
                return Err(format!(
                    "bitfield '{}' received field '{}' more than once",
                    bitfield_name, bitfield.fields[field_index].name.name
                ));
            }

            if let BitfieldFieldKind::Bits { width, as_type } = &bitfield.fields[field_index].kind {
                if as_type.is_none() {
                    let Value::Int64(int_value) = value else {
                        return Err(format!(
                            "bitfield '{}' field '{}' expects int64",
                            bitfield_name, bitfield.fields[field_index].name.name
                        ));
                    };

                    let max_value = if *width >= 63 {
                        i64::MAX
                    } else {
                        (1_i64 << width) - 1
                    };
                    let in_range = int_value >= 0 && int_value <= max_value;
                    let literal_input = matches!(arg.value, Expr::IntLiteral(_, _));

                    if literal_input {
                        if !in_range {
                            return Err(format!(
                                "bitfield '{bitfield_name}' field '{}' is {} bit(s) wide and cannot hold '{int_value}'",
                                bitfield.fields[field_index].name.name, width,
                            ));
                        }
                    } else {
                        requires_runtime_validation = true;
                        if !in_range {
                            return Ok(Value::ResultFail(Box::new(Value::String(format!(
                                "bitfield '{bitfield_name}' field '{}' is {} bit(s) wide and cannot hold '{int_value}'",
                                bitfield.fields[field_index].name.name, width,
                            )))));
                        }
                    }

                    fields[field_index] = Some(Value::Int64(int_value));
                    continue;
                }
            }

            fields[field_index] = Some(value);
        }

        for (index, field) in bitfield.fields.iter().enumerate() {
            if fields[index].is_none() {
                return Err(format!(
                    "bitfield '{}' is missing required field '{}'",
                    bitfield_name, field.name.name
                ));
            }
        }

        let value = Value::Struct {
            type_name: bitfield_name.to_string(),
            fields: bitfield
                .fields
                .iter()
                .zip(fields.into_iter())
                .map(|(field, value)| (field.name.name.clone(), value.unwrap()))
                .collect(),
        };

        if requires_runtime_validation {
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

        // -- Enum equality ---------------------------------------------------
        (Value::Enum { .. }, BinOp::Eq, Value::Enum { .. }) => Ok(Value::Bool(left == right)),
        (Value::Enum { .. }, BinOp::NotEq, Value::Enum { .. }) => Ok(Value::Bool(left != right)),

        // -- Nothing equality ------------------------------------------------
        (Value::Nothing, BinOp::Eq, Value::Nothing) => Ok(Value::Bool(true)),
        (Value::Nothing, BinOp::NotEq, Value::Nothing) => Ok(Value::Bool(false)),

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
        TypeExpr::Function(_, _, _) => "function".to_string(),
    }
}

fn type_expr_is_optional(ty: &TypeExpr) -> bool {
    match ty {
        TypeExpr::Generic(ident, _, _) => ident.name == "optional",
        TypeExpr::View(inner, _) => type_expr_is_optional(inner),
        _ => false,
    }
}

fn json_type_name(value: &JsonValue) -> &'static str {
    match value {
        JsonValue::Null => "null",
        JsonValue::Bool(_) => "bool",
        JsonValue::Number(_) => "number",
        JsonValue::String(_) => "string",
        JsonValue::Array(_) => "array",
        JsonValue::Object(_) => "object",
    }
}

fn json_string(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            '\u{08}' => escaped.push_str("\\b"),
            '\u{0c}' => escaped.push_str("\\f"),
            ch if ch <= '\u{1f}' => escaped.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => escaped.push(ch),
        }
    }
    format!("\"{escaped}\"")
}

fn type_expr_display(ty: &TypeExpr) -> String {
    match ty {
        TypeExpr::Named(ident) => ident.name.clone(),
        TypeExpr::Generic(ident, args, _) => {
            let args = args.iter().map(type_expr_display).collect::<Vec<_>>();
            format!("{}[{}]", ident.name, args.join(", "))
        }
        TypeExpr::View(inner, _) => format!("view {}", type_expr_display(inner)),
        TypeExpr::Function(params, return_type, _) => {
            let params = params.iter().map(type_expr_display).collect::<Vec<_>>();
            format!(
                "function({}) returns {}",
                params.join(", "),
                type_expr_display(return_type)
            )
        }
    }
}

fn runtime_type_name(value: &Value) -> Option<String> {
    match value {
        Value::Int64(_) => Some("int64".to_string()),
        Value::Float64(_) => Some("float64".to_string()),
        Value::String(_) => Some("string".to_string()),
        Value::Bool(_) => Some("bool".to_string()),
        Value::List(_) => Some("list".to_string()),
        Value::Bytes(_) => Some("bytes".to_string()),
        Value::ResultOk(_) | Value::ResultFail(_) => Some("result".to_string()),
        Value::OptionalSome(_) | Value::OptionalNone => Some("optional".to_string()),
        Value::Nothing => Some("nothing".to_string()),
        Value::Struct { type_name, .. }
        | Value::Enum { type_name, .. }
        | Value::Machine { type_name, .. } => Some(type_name.clone()),
        Value::Error(_) => None,
        Value::Actor(_) => Some("actor".to_string()),
        Value::Pending(_) => Some("pending".to_string()),
        Value::Map(_) => Some("map".to_string()),
        Value::Set(_) => Some("set".to_string()),
        Value::Function { .. } => Some("function".to_string()),
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

    #[test]
    fn json_string_escapes_control_characters() {
        assert_eq!(
            json_string(
                "quote \" slash \\ newline\n tab\t backspace\u{08} formfeed\u{0c} nul\0 unit\u{1f}"
            ),
            "\"quote \\\" slash \\\\ newline\\n tab\\t backspace\\b formfeed\\f nul\\u0000 unit\\u001f\""
        );
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

    fn enum_def_with_values(name: &str, variants: Vec<(&str, i64)>) -> EnumDef {
        EnumDef {
            name: ident(name),
            variants: variants
                .into_iter()
                .map(|(variant_name, discriminant)| jett_parser::ast::Variant {
                    name: ident(variant_name),
                    fields: vec![],
                    discriminant: Some(discriminant),
                    span: sp(),
                })
                .collect(),
            span: sp(),
        }
    }

    /// Helper: create a simple bitfield definition.
    fn bitfield_def(
        name: &str,
        fields: Vec<(&str, BitfieldFieldKind)>,
        network_order: bool,
    ) -> BitfieldDef {
        BitfieldDef {
            name: ident(name),
            network_order,
            fields: fields
                .into_iter()
                .map(|(field_name, kind)| jett_parser::ast::BitfieldFieldDef {
                    name: ident(field_name),
                    kind,
                    span: sp(),
                })
                .collect(),
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
    fn trace_stmt_records_current_value() {
        let mut interp = Interpreter::new();
        interp.exec_stmt(&var_decl("total", int(42))).unwrap();

        interp
            .exec_stmt(&Stmt::Trace(TraceStmt {
                name: ident("total"),
                span: sp(),
            }))
            .unwrap();

        assert_eq!(interp.take_debug_output(), vec!["trace total = 42"]);
    }

    #[test]
    fn breakpoint_stmt_records_visible_bindings() {
        let mut interp = Interpreter::new();
        interp.exec_stmt(&var_decl("total", int(42))).unwrap();

        interp
            .exec_stmt(&Stmt::Breakpoint(BreakpointStmt {
                condition: Some(bool_expr(true)),
                span: sp(),
            }))
            .unwrap();

        assert_eq!(
            interp.take_debug_output(),
            vec!["breakpoint hit: total = 42"]
        );
    }

    #[test]
    fn breakpoint_stmt_skips_when_condition_is_false() {
        let mut interp = Interpreter::new();
        interp.exec_stmt(&var_decl("total", int(42))).unwrap();

        interp
            .exec_stmt(&Stmt::Breakpoint(BreakpointStmt {
                condition: Some(bool_expr(false)),
                span: sp(),
            }))
            .unwrap();

        assert!(interp.take_debug_output().is_empty());
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
            value_variable: None,
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
            value_variable: None,
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
    fn bitfield_constructor_with_literals_returns_value() {
        let mut interp = Interpreter::new();
        interp.register_bitfield(&bitfield_def(
            "TcpFlags",
            vec![
                (
                    "syn",
                    BitfieldFieldKind::Bits {
                        width: 1,
                        as_type: None,
                    },
                ),
                (
                    "ack",
                    BitfieldFieldKind::Bits {
                        width: 1,
                        as_type: None,
                    },
                ),
            ],
            false,
        ));

        let expr = Expr::Call(
            Box::new(var("TcpFlags")),
            vec![named_arg("syn", int(0)), named_arg("ack", int(1))],
            sp(),
        );

        assert_eq!(
            interp.eval_expr(&expr).unwrap(),
            Value::Struct {
                type_name: "TcpFlags".to_string(),
                fields: vec![
                    ("syn".to_string(), Value::Int64(0)),
                    ("ack".to_string(), Value::Int64(1)),
                ],
            }
        );
    }

    #[test]
    fn bitfield_constructor_with_dynamic_field_returns_result_ok() {
        let mut interp = Interpreter::new();
        interp.register_bitfield(&bitfield_def(
            "TcpFlags",
            vec![
                (
                    "syn",
                    BitfieldFieldKind::Bits {
                        width: 1,
                        as_type: None,
                    },
                ),
                (
                    "ack",
                    BitfieldFieldKind::Bits {
                        width: 1,
                        as_type: None,
                    },
                ),
            ],
            false,
        ));
        interp.set_variable("bit", Value::Int64(1));

        let expr = Expr::Call(
            Box::new(var("TcpFlags")),
            vec![named_arg("syn", var("bit")), named_arg("ack", int(0))],
            sp(),
        );

        assert_eq!(
            interp.eval_expr(&expr).unwrap(),
            Value::ResultOk(Box::new(Value::Struct {
                type_name: "TcpFlags".to_string(),
                fields: vec![
                    ("syn".to_string(), Value::Int64(1)),
                    ("ack".to_string(), Value::Int64(0)),
                ],
            }))
        );
    }

    #[test]
    fn bitfield_constructor_with_dynamic_field_returns_result_fail() {
        let mut interp = Interpreter::new();
        interp.register_bitfield(&bitfield_def(
            "TcpFlags",
            vec![
                (
                    "syn",
                    BitfieldFieldKind::Bits {
                        width: 1,
                        as_type: None,
                    },
                ),
                (
                    "ack",
                    BitfieldFieldKind::Bits {
                        width: 1,
                        as_type: None,
                    },
                ),
            ],
            false,
        ));
        interp.set_variable("bit", Value::Int64(2));

        let expr = Expr::Call(
            Box::new(var("TcpFlags")),
            vec![named_arg("syn", var("bit")), named_arg("ack", int(0))],
            sp(),
        );

        assert_eq!(
            interp.eval_expr(&expr).unwrap(),
            Value::ResultFail(Box::new(Value::String(
                "bitfield 'TcpFlags' field 'syn' is 1 bit(s) wide and cannot hold '2'".to_string(),
            )))
        );
    }

    #[test]
    fn bitfield_field_access_reads_registered_field() {
        let mut interp = Interpreter::new();
        interp.register_bitfield(&bitfield_def(
            "TcpFlags",
            vec![
                (
                    "syn",
                    BitfieldFieldKind::Bits {
                        width: 1,
                        as_type: None,
                    },
                ),
                (
                    "ack",
                    BitfieldFieldKind::Bits {
                        width: 1,
                        as_type: None,
                    },
                ),
                (
                    "payload",
                    BitfieldFieldKind::Payload(TypeExpr::Generic(
                        ident("list"),
                        vec![type_named("uint8")],
                        sp(),
                    )),
                ),
            ],
            false,
        ));

        let expr = field_access(
            Expr::Call(
                Box::new(var("TcpFlags")),
                vec![
                    named_arg("syn", int(1)),
                    named_arg("ack", int(0)),
                    named_arg("payload", Expr::ListConstruct(vec![int(1), int(2)], sp())),
                ],
                sp(),
            ),
            "ack",
        );

        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Int64(0));
    }

    #[test]
    fn bitfield_to_bytes_packs_network_order_fields() {
        let mut interp = Interpreter::new();
        interp.register_bitfield(&bitfield_def(
            "IpHeader",
            vec![
                (
                    "version",
                    BitfieldFieldKind::Bits {
                        width: 4,
                        as_type: None,
                    },
                ),
                (
                    "header_length",
                    BitfieldFieldKind::Bits {
                        width: 4,
                        as_type: None,
                    },
                ),
                (
                    "total_length",
                    BitfieldFieldKind::Bits {
                        width: 16,
                        as_type: None,
                    },
                ),
            ],
            true,
        ));

        let expr = dotted_call(
            "IpHeader",
            "to_bytes",
            vec![Expr::Call(
                Box::new(var("IpHeader")),
                vec![
                    named_arg("version", int(4)),
                    named_arg("header_length", int(5)),
                    named_arg("total_length", int(500)),
                ],
                sp(),
            )],
        );

        assert_eq!(
            interp.eval_expr(&expr).unwrap(),
            Value::Bytes(vec![0x45, 0x01, 0xF4])
        );
    }

    #[test]
    fn bitfield_from_bytes_unpacks_network_order_fields() {
        let mut interp = Interpreter::new();
        interp.register_bitfield(&bitfield_def(
            "IpHeader",
            vec![
                (
                    "version",
                    BitfieldFieldKind::Bits {
                        width: 4,
                        as_type: None,
                    },
                ),
                (
                    "header_length",
                    BitfieldFieldKind::Bits {
                        width: 4,
                        as_type: None,
                    },
                ),
                (
                    "total_length",
                    BitfieldFieldKind::Bits {
                        width: 16,
                        as_type: None,
                    },
                ),
            ],
            true,
        ));

        let expr = dotted_call("IpHeader", "from_bytes", vec![var("raw")]);
        interp.set_variable("raw", Value::Bytes(vec![0x45, 0x01, 0xF4]));

        assert_eq!(
            interp.eval_expr(&expr).unwrap(),
            Value::ResultOk(Box::new(Value::Struct {
                type_name: "IpHeader".to_string(),
                fields: vec![
                    ("version".to_string(), Value::Int64(4)),
                    ("header_length".to_string(), Value::Int64(5)),
                    ("total_length".to_string(), Value::Int64(500)),
                ],
            }))
        );
    }

    #[test]
    fn bitfield_to_bytes_uses_enum_annotations() {
        let mut interp = Interpreter::new();
        interp.register_enum(&enum_def_with_values(
            "IpProtocol",
            vec![("icmp", 1), ("tcp", 6), ("udp", 17)],
        ));
        interp.register_bitfield(&bitfield_def(
            "Header",
            vec![(
                "protocol",
                BitfieldFieldKind::Bits {
                    width: 8,
                    as_type: Some(type_named("IpProtocol")),
                },
            )],
            true,
        ));

        let expr = dotted_call(
            "Header",
            "to_bytes",
            vec![Expr::Call(
                Box::new(var("Header")),
                vec![named_arg(
                    "protocol",
                    Expr::EnumVariant(ident("IpProtocol"), ident("tcp"), sp()),
                )],
                sp(),
            )],
        );

        assert_eq!(interp.eval_expr(&expr).unwrap(), Value::Bytes(vec![6]));
    }

    #[test]
    fn bitfield_from_bytes_decodes_enum_annotations() {
        let mut interp = Interpreter::new();
        interp.register_enum(&enum_def_with_values(
            "IpProtocol",
            vec![("icmp", 1), ("tcp", 6), ("udp", 17)],
        ));
        interp.register_bitfield(&bitfield_def(
            "Header",
            vec![(
                "protocol",
                BitfieldFieldKind::Bits {
                    width: 8,
                    as_type: Some(type_named("IpProtocol")),
                },
            )],
            true,
        ));

        let expr = dotted_call("Header", "from_bytes", vec![var("raw")]);
        interp.set_variable("raw", Value::Bytes(vec![17]));

        assert_eq!(
            interp.eval_expr(&expr).unwrap(),
            Value::ResultOk(Box::new(Value::Struct {
                type_name: "Header".to_string(),
                fields: vec![(
                    "protocol".to_string(),
                    Value::Enum {
                        type_name: "IpProtocol".to_string(),
                        variant: "udp".to_string(),
                        fields: vec![],
                    },
                )],
            }))
        );
    }

    #[test]
    fn function_parameter_refinement_rejects_invalid_argument() {
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

        let mut accept_age = func_def(
            "accept_age",
            vec![("age", "Age")],
            block(vec![return_stmt(var("age"))]),
        );
        accept_age.return_type = Some(type_named("int64"));
        interp.register_function(&accept_age);

        assert_eq!(
            interp
                .call_function("accept_age", vec![Value::Int64(200)])
                .unwrap_err(),
            "refinement type constraint failed for 'Age'".to_string()
        );
    }

    #[test]
    fn function_with_refinement_return_accepts_valid_value() {
        let mut interp = Interpreter::new();
        interp.register_type_alias(&type_alias(
            "Port",
            "int64",
            Some(binary(
                binary(var("value"), BinOp::GtEq, int(1)),
                BinOp::And,
                binary(var("value"), BinOp::LtEq, int(65535)),
            )),
        ));

        let mut default_port =
            func_def("default_port", vec![], block(vec![return_stmt(int(8080))]));
        default_port.return_type = Some(type_named("Port"));
        interp.register_function(&default_port);

        assert_eq!(
            interp.call_function("default_port", vec![]).unwrap(),
            Value::Int64(8080)
        );
    }

    #[test]
    fn function_with_refinement_return_rejects_invalid_value() {
        let mut interp = Interpreter::new();
        interp.register_type_alias(&type_alias(
            "Port",
            "int64",
            Some(binary(
                binary(var("value"), BinOp::GtEq, int(1)),
                BinOp::And,
                binary(var("value"), BinOp::LtEq, int(65535)),
            )),
        ));

        let mut invalid_port =
            func_def("invalid_port", vec![], block(vec![return_stmt(int(70000))]));
        invalid_port.return_type = Some(type_named("Port"));
        interp.register_function(&invalid_port);

        assert_eq!(
            interp.call_function("invalid_port", vec![]).unwrap_err(),
            "refinement type constraint failed for 'Port'".to_string()
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

// ---------------------------------------------------------------------------
// Base64 helpers (no external crate dependency)
// ---------------------------------------------------------------------------

fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut i = 0;
    while i < data.len() {
        let b0 = data[i] as u32;
        let b1 = if i + 1 < data.len() {
            data[i + 1] as u32
        } else {
            0
        };
        let b2 = if i + 2 < data.len() {
            data[i + 2] as u32
        } else {
            0
        };
        let combined = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[((combined >> 18) & 63) as usize] as char);
        out.push(ALPHABET[((combined >> 12) & 63) as usize] as char);
        if i + 1 < data.len() {
            out.push(ALPHABET[((combined >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }
        if i + 2 < data.len() {
            out.push(ALPHABET[(combined & 63) as usize] as char);
        } else {
            out.push('=');
        }
        i += 3;
    }
    out
}

// ---------------------------------------------------------------------------
// SHA-256 helper (no external crate dependency)
// ---------------------------------------------------------------------------

fn sha256_hash(data: &[u8]) -> String {
    #[rustfmt::skip]
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5,
        0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
        0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc,
        0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
        0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
        0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5,
        0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
        0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0x00);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    format!(
        "{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}",
        h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]
    )
}

// ---------------------------------------------------------------------------
// MD5 helper (no external crate dependency)
// ---------------------------------------------------------------------------

fn md5_hash(data: &[u8]) -> String {
    #[rustfmt::skip]
    const S: [u32; 64] = [
        7, 12, 17, 22,  7, 12, 17, 22,  7, 12, 17, 22,  7, 12, 17, 22,
        5,  9, 14, 20,  5,  9, 14, 20,  5,  9, 14, 20,  5,  9, 14, 20,
        4, 11, 16, 23,  4, 11, 16, 23,  4, 11, 16, 23,  4, 11, 16, 23,
        6, 10, 15, 21,  6, 10, 15, 21,  6, 10, 15, 21,  6, 10, 15, 21,
    ];
    #[rustfmt::skip]
    const K: [u32; 64] = [
        0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee,
        0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
        0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be,
        0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
        0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa,
        0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
        0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed,
        0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
        0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c,
        0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
        0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05,
        0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
        0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039,
        0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
        0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1,
        0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
    ];
    let mut a0: u32 = 0x67452301;
    let mut b0: u32 = 0xefcdab89;
    let mut c0: u32 = 0x98badcfe;
    let mut d0: u32 = 0x10325476;
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0x00);
    }
    msg.extend_from_slice(&bit_len.to_le_bytes());
    for chunk in msg.chunks(64) {
        let mut m = [0u32; 16];
        for i in 0..16 {
            m[i] = u32::from_le_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        let mut a = a0;
        let mut b = b0;
        let mut c = c0;
        let mut d = d0;
        for i in 0..64usize {
            let (f, g) = if i < 16 {
                ((b & c) | ((!b) & d), i)
            } else if i < 32 {
                ((d & b) | ((!d) & c), (5 * i + 1) % 16)
            } else if i < 48 {
                (b ^ c ^ d, (3 * i + 5) % 16)
            } else {
                (c ^ (b | (!d)), (7 * i) % 16)
            };
            let temp = d;
            d = c;
            c = b;
            b = b.wrapping_add(
                a.wrapping_add(f)
                    .wrapping_add(K[i])
                    .wrapping_add(m[g])
                    .rotate_left(S[i]),
            );
            a = temp;
        }
        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }
    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(&a0.to_le_bytes());
    out[4..8].copy_from_slice(&b0.to_le_bytes());
    out[8..12].copy_from_slice(&c0.to_le_bytes());
    out[12..16].copy_from_slice(&d0.to_le_bytes());
    out.iter().map(|b| format!("{b:02x}")).collect()
}

fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    fn char_val(c: u8) -> Result<u32, String> {
        match c {
            b'A'..=b'Z' => Ok((c - b'A') as u32),
            b'a'..=b'z' => Ok((c - b'a' + 26) as u32),
            b'0'..=b'9' => Ok((c - b'0' + 52) as u32),
            b'+' => Ok(62),
            b'/' => Ok(63),
            b'=' => Ok(0),
            _ => Err(format!("invalid base64 character: {c:?}")),
        }
    }
    let bytes = s.as_bytes();
    if bytes.len() % 4 != 0 {
        return Err("base64 string length must be a multiple of 4".to_string());
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let v0 = char_val(bytes[i])?;
        let v1 = char_val(bytes[i + 1])?;
        let v2 = char_val(bytes[i + 2])?;
        let v3 = char_val(bytes[i + 3])?;
        let combined = (v0 << 18) | (v1 << 12) | (v2 << 6) | v3;
        out.push(((combined >> 16) & 0xFF) as u8);
        if bytes[i + 2] != b'=' {
            out.push(((combined >> 8) & 0xFF) as u8);
        }
        if bytes[i + 3] != b'=' {
            out.push((combined & 0xFF) as u8);
        }
        i += 4;
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// CSV helpers
// ---------------------------------------------------------------------------

/// Parse a single CSV line, handling quoted fields with embedded commas,
/// quotes (escaped as `""`), and newlines.
fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if in_quotes {
            if ch == '"' {
                // Check for escaped quote ""
                if i + 1 < chars.len() && chars[i + 1] == '"' {
                    current.push('"');
                    i += 2;
                    continue;
                } else {
                    in_quotes = false;
                    i += 1;
                    continue;
                }
            } else {
                current.push(ch);
            }
        } else if ch == '"' {
            in_quotes = true;
        } else if ch == ',' {
            fields.push(current.clone());
            current.clear();
        } else {
            current.push(ch);
        }
        i += 1;
    }
    fields.push(current);
    fields
}

/// Quote a CSV field if it contains commas, quotes, or newlines.
fn csv_quote_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        let escaped = s.replace('"', "\"\"");
        format!("\"{escaped}\"")
    } else {
        s.to_string()
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
            type_params: vec![],
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
            Value::String("***".to_string())
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

    #[test]
    fn test_sha256_known_vectors() {
        // NIST test vectors
        assert_eq!(
            sha256_hash(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "sha256 of empty string"
        );
        assert_eq!(
            sha256_hash(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            "sha256 of 'abc'"
        );
    }

    #[test]
    fn test_md5_known_vectors() {
        assert_eq!(md5_hash(b""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(md5_hash(b"abc"), "900150983cd24fb0d6963f7d28e17f72");
    }
}
