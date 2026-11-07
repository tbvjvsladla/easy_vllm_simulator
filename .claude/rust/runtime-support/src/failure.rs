//! The exception a Python-compatible operation raises, as typed data: its class name and `str(exception)`.

use crate::pynumber::PyException;
use crate::pystr::PyStr;

/// A Python exception whose message may hold characters a Rust `String` cannot (argument bytes that are not
/// UTF-8 survive as lone surrogates in the message).
#[derive(Clone, Debug, PartialEq)]
pub struct Failure {
    pub class: &'static str,
    pub message: PyStr,
}

impl Failure {
    pub fn new(class: &'static str, message: &str) -> Self {
        Failure {
            class,
            message: PyStr::new(message),
        }
    }

    pub fn with_message(class: &'static str, message: PyStr) -> Self {
        Failure { class, message }
    }

    /// `except ValueError` catches this one; everything else escapes the original's handlers.
    pub fn is_value_error(&self) -> bool {
        self.class == "ValueError"
    }

    /// `Class: message`, or just the class name when the message is empty, as Python prints it.
    pub fn line(&self) -> PyStr {
        let mut text = PyStr::new(self.class);
        if !self.message.is_empty() {
            text.push_str(": ");
            text.push_py(&self.message);
        }
        text
    }
}

impl From<PyException> for Failure {
    fn from(error: PyException) -> Self {
        Failure::new(error.class, &error.message)
    }
}
