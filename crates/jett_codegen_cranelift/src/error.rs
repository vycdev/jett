use std::fmt;

use jett_common::Span;
use jett_mir::ValidationError;

/// A failure at the MIR-to-object boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodegenError {
    InvalidTarget {
        target: String,
        message: String,
    },
    UnsupportedTarget {
        requested: String,
        supported: String,
    },
    InvalidMir(Vec<ValidationError>),
    InvalidMirContract {
        function: String,
        span: Span,
        message: String,
    },
    UnsupportedType {
        type_name: String,
        context: String,
    },
    UnsupportedMir {
        function: String,
        span: Span,
        construct: String,
    },
    MissingProgramEntry {
        function_id: u32,
    },
    DuplicateProgramEntry {
        function_id: u32,
    },
    UnreachableProgramEntry {
        function_id: u32,
    },
    IncompatibleProgramEntry {
        function_id: u32,
        message: String,
    },
    DuplicateSymbol(String),
    Backend(String),
}

impl fmt::Display for CodegenError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTarget { target, message } => {
                write!(formatter, "invalid target `{target}`: {message}")
            }
            Self::UnsupportedTarget {
                requested,
                supported,
            } => write!(
                formatter,
                "target `{requested}` is unsupported; this compiler supports only `{supported}`"
            ),
            Self::InvalidMir(errors) => {
                write!(
                    formatter,
                    "MIR validation failed with {} error(s)",
                    errors.len()
                )
            }
            Self::InvalidMirContract {
                function,
                span,
                message,
            } => write!(
                formatter,
                "invalid MIR contract in `{function}` at {span:?}: {message}"
            ),
            Self::UnsupportedType { type_name, context } => {
                write!(formatter, "type `{type_name}` is unsupported in {context}")
            }
            Self::UnsupportedMir {
                function,
                span,
                construct,
            } => write!(
                formatter,
                "unsupported MIR construct `{construct}` in `{function}` at {span:?}"
            ),
            Self::MissingProgramEntry { function_id } => {
                write!(
                    formatter,
                    "program entry function {function_id} is absent from the MIR program"
                )
            }
            Self::DuplicateProgramEntry { function_id } => write!(
                formatter,
                "program entry function ID {function_id} occurs more than once in the MIR program"
            ),
            Self::UnreachableProgramEntry { function_id } => write!(
                formatter,
                "program entry function {function_id} is not reachable from a project function"
            ),
            Self::IncompatibleProgramEntry {
                function_id,
                message,
            } => write!(
                formatter,
                "program entry function {function_id} has an incompatible contract: {message}"
            ),
            Self::DuplicateSymbol(symbol) => {
                write!(
                    formatter,
                    "multiple MIR functions map to native symbol `{symbol}`"
                )
            }
            Self::Backend(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for CodegenError {}
