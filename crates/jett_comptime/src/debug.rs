//! Typed compiler-owned debug observations, separate from program stdout.

/// The source operation that produced a debug observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DebugEventKind {
    Trace,
    Breakpoint,
    Print,
    Println,
}

impl DebugEventKind {
    /// Canonical kind for structured tool output, independent of event text.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Breakpoint => "breakpoint",
            Self::Print => "print",
            Self::Println => "println",
        }
    }
}

/// One complete observation in execution order.
///
/// `text` contains the exact emitted bytes as UTF-8, including only the newline
/// deliberately supplied by the operation. A `Print` can be empty, partial, or
/// contain embedded newlines. Its text never determines its kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugEvent {
    pub kind: DebugEventKind,
    pub text: String,
}

/// Render a diagnostic stream without adding separators or splitting events.
pub fn render_debug_events(events: &[DebugEvent]) -> String {
    let mut output = String::new();
    for event in events {
        output.push_str(&event.text);
    }
    output
}
