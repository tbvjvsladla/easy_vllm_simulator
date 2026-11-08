//! The Python exceptions of the node-role owner as the shared exception graph ([`PyException`]) holds them: `ContractViolation(code, message, field)`,
//! the `yaml.*` marked errors with their typed six-field marks, `UnicodeDecodeError`, `OSError` subclasses and the `TypeError`/`ValueError` of the
//! argument types. Built from the library's data ([`role_contract`]); nothing is recovered from message text.

use crate::classify_source::unicode_decode_exception;
use crate::http_direct::{ErrorValue, PyException};
use crate::py_errors::{builtin, os_error_with_filename};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;
use agent_runtime_support::pyyaml::{Mark6, MarkedDetail, YamlDetail, YamlError};
use role_contract::load::LoadFailure;
use role_contract::{ContractViolation, PathJoinError, SourceFrame};

/// The module name a native owner reports for its own classes.
pub const OWNER_MODULE: &str = "node_role_contract";

/// What a library call ended with besides a value: a Python exception (and whether it was raised `from None`), or a named boundary.
pub enum Raised {
    Exception {
        error: Box<PyException>,
        from_none: bool,
        /// The logical source-compatibility traceback (outermost first); `None` when the owner/loader has no mapped chain for this failure.
        frames: Option<Vec<SourceFrame>>,
    },
    OutOfScope {
        boundary: &'static str,
        detail: String,
    },
}

fn exception(error: PyException) -> Raised {
    Raised::Exception {
        error: Box::new(error),
        from_none: false,
        frames: None,
    }
}

fn with_frames(raised: Raised, frames: Option<Vec<SourceFrame>>) -> Raised {
    match raised {
        Raised::Exception {
            error, from_none, ..
        } => Raised::Exception {
            error,
            from_none,
            frames,
        },
        other => other,
    }
}

/// `ContractViolation(code, message, field)`: `args == (message,)`, public attributes `code`, `message`, `field`.
pub fn contract_violation(violation: &ContractViolation) -> PyException {
    PyException {
        class: "ContractViolation",
        module: OWNER_MODULE,
        message: violation.message.clone(),
        args: vec![ErrorValue::Plain(PyValue::Str(violation.message.clone()))],
        attributes: vec![
            ("code", PyValue::str(violation.code).into()),
            ("message", PyValue::Str(violation.message.clone()).into()),
            (
                "field",
                violation.field.map_or(PyValue::None, PyValue::str).into(),
            ),
        ],
        http_headers: None,
        link: None,
    }
}

/// The `TypeError` of `os.path.join`, raised by `_check_arg_types` `from None`.
pub fn path_join_error(error: &PathJoinError) -> Raised {
    Raised::Exception {
        error: Box::new(builtin("TypeError", &error.message())),
        from_none: true,
        frames: Some(error.frames()),
    }
}

fn mark_value(mark: &Option<Mark6>) -> ErrorValue {
    match mark {
        Some(mark) => ErrorValue::YamlMark(Box::new(mark.clone())),
        None => PyValue::None.into(),
    }
}

fn text_value(text: &Option<PyStr>) -> ErrorValue {
    match text {
        Some(text) => PyValue::Str(text.clone()).into(),
        None => PyValue::None.into(),
    }
}

fn yaml_module(class: &str) -> Option<&'static str> {
    Some(match class {
        "ScannerError" => "yaml.scanner",
        "ParserError" => "yaml.parser",
        "ComposerError" => "yaml.composer",
        "ConstructorError" => "yaml.constructor",
        _ => return None,
    })
}

/// A failure of `load_yaml`.
pub fn load_failure(failure: LoadFailure) -> Raised {
    let frames = failure.frames();
    with_frames(load_failure_graph(failure), frames)
}

fn load_failure_graph(failure: LoadFailure) -> Raised {
    match failure {
        LoadFailure::NotPathLike(name) => exception(builtin(
            "TypeError",
            &format!("expected str, bytes or os.PathLike object, not {name}"),
        )),
        LoadFailure::EmbeddedNull => exception(builtin("ValueError", "embedded null byte")),
        LoadFailure::Os { error, filename } => match os_error_with_filename(&error, &filename) {
            Some(raised) => exception(raised),
            None => Raised::OutOfScope {
                boundary: "open_failure",
                detail: error.to_string(),
            },
        },
        LoadFailure::Unsupported { boundary, detail } => Raised::OutOfScope { boundary, detail },
        LoadFailure::Yaml(error) => yaml_failure(*error),
    }
}

fn yaml_failure(error: YamlError) -> Raised {
    let YamlError {
        class,
        message,
        detail,
    } = error;
    match detail {
        YamlDetail::Marked(detail) => {
            let MarkedDetail {
                context,
                context_mark,
                problem,
                problem_mark,
                note,
                frames: _,
            } = *detail;
            let Some(module) = yaml_module(class) else {
                return Raised::OutOfScope {
                    boundary: "yaml_error_class",
                    detail: format!("the marked error class {class} has no module in this profile"),
                };
            };
            exception(PyException {
                class,
                module,
                message,
                args: vec![
                    text_value(&context),
                    mark_value(&context_mark),
                    text_value(&problem),
                    mark_value(&problem_mark),
                ],
                attributes: vec![
                    ("context", text_value(&context)),
                    ("context_mark", mark_value(&context_mark)),
                    ("problem", text_value(&problem)),
                    ("problem_mark", mark_value(&problem_mark)),
                    ("note", text_value(&note)),
                ],
                http_headers: None,
                link: None,
            })
        }
        YamlDetail::Decode {
            object,
            start,
            end,
            reason,
            ..
        } => exception(unicode_decode_exception(
            "utf-8", &object, start, end, reason,
        )),
        YamlDetail::Reader { .. } => Raised::OutOfScope {
            boundary: "reader_error_graph",
            detail:
                "a yaml ReaderError (an unacceptable character) is outside the selected profile"
                    .to_string(),
        },
        YamlDetail::None => match class {
            "ValueError" => exception(builtin(
                "ValueError",
                &message.to_utf8().unwrap_or_default(),
            )),
            other => Raised::OutOfScope {
                boundary: "yaml_plain_exception",
                detail: format!("{other} raised by the loader has no modelled exception graph"),
            },
        },
    }
}
