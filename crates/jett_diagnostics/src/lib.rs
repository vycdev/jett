pub mod render;
pub mod toon;

use jett_common::Span;

/// Severity level of a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// A stable, greppable error code (e.g., E0001).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiagnosticCode(u16);

impl DiagnosticCode {
    pub fn new(code: u16) -> Self {
        Self(code)
    }

    pub fn code(self) -> u16 {
        self.0
    }
}

impl std::fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "E{:04}", self.0)
    }
}

/// A secondary label pointing to a related source location.
#[derive(Debug, Clone)]
pub struct Label {
    pub span: Span,
    pub message: String,
}

/// A concrete, apply-ready fix suggestion.
#[derive(Debug, Clone)]
pub struct SuggestedFix {
    pub span: Span,
    pub old_text: String,
    pub new_text: String,
    pub explanation: String,
}

/// A compiler diagnostic (error, warning, or info).
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: DiagnosticCode,
    pub message: String,
    pub span: Span,
    pub labels: Vec<Label>,
    pub suggested_fix: Option<SuggestedFix>,
}

impl Diagnostic {
    pub fn error(code: u16, message: impl Into<String>, span: Span) -> Self {
        Self {
            severity: Severity::Error,
            code: DiagnosticCode::new(code),
            message: message.into(),
            span,
            labels: Vec::new(),
            suggested_fix: None,
        }
    }

    pub fn warning(code: u16, message: impl Into<String>, span: Span) -> Self {
        Self {
            severity: Severity::Warning,
            code: DiagnosticCode::new(code),
            message: message.into(),
            span,
            labels: Vec::new(),
            suggested_fix: None,
        }
    }

    pub fn with_label(mut self, span: Span, message: impl Into<String>) -> Self {
        self.labels.push(Label {
            span,
            message: message.into(),
        });
        self
    }

    pub fn with_fix(
        mut self,
        span: Span,
        old_text: impl Into<String>,
        new_text: impl Into<String>,
        explanation: impl Into<String>,
    ) -> Self {
        self.suggested_fix = Some(SuggestedFix {
            span,
            old_text: old_text.into(),
            new_text: new_text.into(),
            explanation: explanation.into(),
        });
        self
    }
}

/// Collects diagnostics during compilation.
#[derive(Debug, Default)]
pub struct DiagnosticSink {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticSink {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn emit(&mut self, diag: Diagnostic) {
        self.diagnostics.push(diag);
    }

    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
