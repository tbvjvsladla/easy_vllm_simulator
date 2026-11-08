//! What `tomllib.loads` raises, and what this port refuses to answer.

/// The exception classes `loads` can raise on a `str`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExceptionClass {
    /// `tomllib.TOMLDecodeError` (a `ValueError` subclass; its `__module__` is `tomllib`).
    TomlDecodeError,
    /// `ValueError` raised by the interpreter: the date constructors and the integer-digit limit.
    ValueError,
    /// `IndexError` of `cont[-1]` on an empty array: raised by the original only when a traversal meets an empty array, which the frozen-flag
    /// guards keep unreachable (see `nested.rs`).
    IndexError,
}

impl ExceptionClass {
    pub fn name(self) -> &'static str {
        match self {
            ExceptionClass::TomlDecodeError => "TOMLDecodeError",
            ExceptionClass::ValueError => "ValueError",
            ExceptionClass::IndexError => "IndexError",
        }
    }

    pub fn module(self) -> &'static str {
        match self {
            ExceptionClass::TomlDecodeError => "tomllib",
            ExceptionClass::ValueError | ExceptionClass::IndexError => "builtins",
        }
    }
}

/// One raised exception: its single string argument, the `raise ... from` state and the exception it was raised from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Raised {
    pub class: ExceptionClass,
    /// `str(exception)`, which is also `exception.args[0]`.
    pub message: String,
    /// `__suppress_context__`: true after `raise ... from None` and after `raise ... from error`.
    pub suppress_context: bool,
    /// `__cause__` (`raise ... from error`).
    pub cause: Option<Box<Raised>>,
}

impl Raised {
    /// An exception raised outside every `except` block: no cause, nothing suppressed.
    pub fn plain(class: ExceptionClass, message: String) -> Raised {
        Raised {
            class,
            message,
            suppress_context: false,
            cause: None,
        }
    }

    /// `raise ... from None` inside an `except` block: the implicit context is suppressed and so not observable.
    pub fn from_none(class: ExceptionClass, message: String) -> Raised {
        Raised {
            suppress_context: true,
            ..Raised::plain(class, message)
        }
    }

    /// `raise ... from cause`.
    pub fn from_cause(class: ExceptionClass, message: String, cause: Raised) -> Raised {
        Raised {
            suppress_context: true,
            cause: Some(Box::new(cause)),
            ..Raised::plain(class, message)
        }
    }
}

/// A domain this port does not model; the answer is a refusal, never a parse result or a parser exception.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsupportedDomain {
    /// Arrays and inline tables nested deeper than the modelled share of the interpreter's recursion limit: whether the original raises
    /// `RecursionError` there depends on the stack of its caller, which is not modelled.
    NestingBeyondRecursionLimit,
    /// A number the regular-expression grammar accepted but the float conversion refused: not expected, and never answered with a guess.
    FloatTextNotConvertible,
}

impl UnsupportedDomain {
    /// The stable name used in refusal records.
    pub fn name(self) -> &'static str {
        match self {
            UnsupportedDomain::NestingBeyondRecursionLimit => "nesting_beyond_recursion_limit",
            UnsupportedDomain::FloatTextNotConvertible => "float_text_not_convertible",
        }
    }

    pub fn detail(self) -> &'static str {
        match self {
            UnsupportedDomain::NestingBeyondRecursionLimit => {
                "arrays and inline tables are nested deeper than the modelled share of the interpreter's recursion limit (500 of 1000 frames); whether the original raises RecursionError there depends on the caller's stack, which this component does not model"
            }
            UnsupportedDomain::FloatTextNotConvertible => {
                "a number accepted by the grammar was refused by the float conversion; the original's result is not known"
            }
        }
    }
}

/// Why `loads` did not return a table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadsFailure {
    /// The original raises this exception.
    Raised(Raised),
    /// A domain outside this port.
    Unsupported(UnsupportedDomain),
}

impl From<Raised> for LoadsFailure {
    fn from(raised: Raised) -> Self {
        LoadsFailure::Raised(raised)
    }
}

impl From<UnsupportedDomain> for LoadsFailure {
    fn from(domain: UnsupportedDomain) -> Self {
        LoadsFailure::Unsupported(domain)
    }
}
