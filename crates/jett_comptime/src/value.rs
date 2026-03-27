use std::collections::HashMap;
use std::fmt;

use jett_parser::ast::{Block, Param};

/// Runtime value for the compile-time interpreter.
#[derive(Debug, Clone)]
pub enum Value {
    /// 64-bit signed integer.
    Int64(i64),
    /// 64-bit floating-point number.
    Float64(f64),
    /// UTF-8 string.
    String(String),
    /// Boolean.
    Bool(bool),
    /// Ordered list of values.
    List(Vec<Value>),
    /// Raw bytes.
    Bytes(Vec<u8>),
    /// `ok(value)`
    ResultOk(Box<Value>),
    /// `fail(error)`
    ResultFail(Box<Value>),
    /// `some(value)`
    OptionalSome(Box<Value>),
    /// `none`
    OptionalNone,
    /// The `nothing` value (Jett's unit type).
    Nothing,
    /// A user-defined struct instance: `Point(x: 1, y: 2)`.
    Struct {
        type_name: String,
        fields: Vec<(String, Value)>,
    },
    /// An enum instance: `Color.red` or `Shape.circle(5.0)`.
    Enum {
        type_name: String,
        variant: String,
        fields: Vec<Value>,
    },
    /// A state machine instance: `UserAuth` in state `guest` with fields.
    Machine {
        type_name: String,
        state: String,
        fields: Vec<Value>,
    },
    /// A handle to a spawned actor instance (holds the instance ID).
    Actor(u64),
    /// A runtime error value.
    Error(String),
    /// A pending concurrent task (sequential simulation: already evaluated).
    Pending(Box<Value>),
    /// A key-value map: `map(key1: val1, key2: val2)`.
    Map(Vec<(Value, Value)>),
    /// A captured inline function expression (closure).
    Function {
        params: Vec<Param>,
        body: Block,
        captures: HashMap<String, Value>,
    },
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Int64(a), Value::Int64(b)) => a == b,
            (Value::Float64(a), Value::Float64(b)) => a == b,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::List(a), Value::List(b)) => a == b,
            (Value::Bytes(a), Value::Bytes(b)) => a == b,
            (Value::ResultOk(a), Value::ResultOk(b)) => a == b,
            (Value::ResultFail(a), Value::ResultFail(b)) => a == b,
            (Value::OptionalSome(a), Value::OptionalSome(b)) => a == b,
            (Value::OptionalNone, Value::OptionalNone) => true,
            (Value::Nothing, Value::Nothing) => true,
            (Value::Struct { type_name: t1, fields: f1 }, Value::Struct { type_name: t2, fields: f2 }) => t1 == t2 && f1 == f2,
            (Value::Enum { type_name: t1, variant: v1, fields: f1 }, Value::Enum { type_name: t2, variant: v2, fields: f2 }) => t1 == t2 && v1 == v2 && f1 == f2,
            (Value::Machine { type_name: t1, state: s1, fields: f1 }, Value::Machine { type_name: t2, state: s2, fields: f2 }) => t1 == t2 && s1 == s2 && f1 == f2,
            (Value::Actor(a), Value::Actor(b)) => a == b,
            (Value::Error(a), Value::Error(b)) => a == b,
            (Value::Pending(a), Value::Pending(b)) => a == b,
            (Value::Map(a), Value::Map(b)) => a == b,
            // Functions are never considered equal.
            (Value::Function { .. }, Value::Function { .. }) => false,
            _ => false,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int64(n) => write!(f, "{n}"),
            Value::Float64(n) => write!(f, "{n}"),
            Value::String(s) => write!(f, "{s}"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::List(items) => {
                write!(f, "list(")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, ")")
            }
            Value::Bytes(bytes) => {
                write!(f, "bytes(")?;
                for (i, byte) in bytes.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{byte}")?;
                }
                write!(f, ")")
            }
            Value::ResultOk(value) => write!(f, "ok({value})"),
            Value::ResultFail(value) => write!(f, "fail({value})"),
            Value::OptionalSome(value) => write!(f, "some({value})"),
            Value::OptionalNone => write!(f, "none"),
            Value::Nothing => write!(f, "nothing"),
            Value::Struct { type_name, fields } => {
                write!(f, "{type_name}(")?;
                for (i, (name, value)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{name}: {value}")?;
                }
                write!(f, ")")
            }
            Value::Enum {
                type_name,
                variant,
                fields,
            } => {
                write!(f, "{type_name}.{variant}")?;
                if !fields.is_empty() {
                    write!(f, "(")?;
                    for (i, field) in fields.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{field}")?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
            Value::Machine {
                type_name,
                state,
                fields,
            } => {
                write!(f, "{type_name}@{state}")?;
                if !fields.is_empty() {
                    write!(f, "(")?;
                    for (i, field) in fields.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{field}")?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
            Value::Function { params, .. } => {
                write!(f, "function(")?;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", p.name.name)?;
                }
                write!(f, ")")
            }
            Value::Actor(id) => write!(f, "actor#{id}"),
            Value::Error(msg) => write!(f, "error: {msg}"),
            Value::Pending(inner) => write!(f, "pending({inner})"),
            Value::Map(entries) => {
                write!(f, "map(")?;
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{k}: {v}")?;
                }
                write!(f, ")")
            }
        }
    }
}
