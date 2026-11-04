//! The failures the original program can raise, kept as distinct kinds so callers can map them to exit codes.

use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SweepError {
    /// Python `StopEvalError`: the state or declaration cannot be judged, so no stop decision is made.
    StopEval(String),
    /// Python `TypeError` from iterating a truthy non-iterable `cells_remaining` (`int`, `float` or `bool`).
    NotIterable { type_name: &'static str },
    /// Python `OverflowError` from converting a timestamp near the calendar limits to UTC.
    Overflow(String),
}

impl SweepError {
    pub(crate) fn stop_eval(message: impl Into<String>) -> Self {
        SweepError::StopEval(message.into())
    }

    /// The Python exception class name.
    pub fn python_type(&self) -> &'static str {
        match self {
            SweepError::StopEval(_) => "StopEvalError",
            SweepError::NotIterable { .. } => "TypeError",
            SweepError::Overflow(_) => "OverflowError",
        }
    }

    /// `str(exception)` as Python renders it.
    pub fn message(&self) -> String {
        match self {
            SweepError::StopEval(message) | SweepError::Overflow(message) => message.clone(),
            SweepError::NotIterable { type_name } => {
                format!("'{type_name}' object is not iterable")
            }
        }
    }

    /// Only `StopEvalError` is handled by the original CLI (exit 2); the others escape as tracebacks (exit 1).
    pub fn is_declaration_error(&self) -> bool {
        matches!(self, SweepError::StopEval(_))
    }
}

impl fmt::Display for SweepError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message())
    }
}

impl std::error::Error for SweepError {}
