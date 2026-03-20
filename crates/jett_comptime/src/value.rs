use std::fmt;

/// Runtime value for the compile-time interpreter.
#[derive(Debug, Clone, PartialEq)]
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
    /// The `nothing` value (Jett's unit type).
    Nothing,
    /// A runtime error value.
    Error(String),
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
            Value::Nothing => write!(f, "nothing"),
            Value::Error(msg) => write!(f, "error: {msg}"),
        }
    }
}
