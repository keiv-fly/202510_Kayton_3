use crate::span::Span;
use smol_str::SmolStr;

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub message: SmolStr,
    pub span: Span,
    pub severity: Severity,
    pub notes: Vec<SmolStr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

impl Diagnostic {
    pub fn error<M: Into<SmolStr>>(message: M, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
            severity: Severity::Error,
            notes: Vec::new(),
        }
    }

    pub fn with_note(mut self, note: impl Into<SmolStr>) -> Self {
        self.notes.push(note.into());
        self
    }
}
