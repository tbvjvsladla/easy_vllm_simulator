//! Replacements for the three one-line `python3 -c` snippets in `broad_search.sh` that print values from JSON
//! files. Each function returns the exact text the snippet printed (without the trailing newline); the files are
//! opened as the snippets opened them, with strict UTF-8.

use crate::cli::read_path;
use crate::pyjson::{decode_file_text, loads_text};
use crate::pystr::PyStr;
use crate::pyvalue::PyValue;
use std::fmt;

#[derive(Debug)]
pub enum ReportError {
    Read(String),
    Load(String),
    Shape(String),
}

impl fmt::Display for ReportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReportError::Read(message)
            | ReportError::Load(message)
            | ReportError::Shape(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for ReportError {}

fn load_json(path: &PyStr) -> Result<PyValue, ReportError> {
    let bytes = read_path(path).map_err(ReportError::Read)?;
    let text = decode_file_text(&bytes).map_err(|err| ReportError::Load(err.to_string()))?;
    loads_text(&text).map_err(|err| ReportError::Load(err.to_string()))
}

fn field<'a>(document: &'a PyValue, key: &str) -> Result<&'a PyValue, ReportError> {
    match document {
        PyValue::Dict(map) => map
            .get(key)
            .ok_or_else(|| ReportError::Shape(format!("KeyError: '{key}'"))),
        other => Err(ReportError::Shape(format!(
            "TypeError: {} indices must be strings, not str",
            other.type_name()
        ))),
    }
}

fn joined(parts: &[&PyStr]) -> PyStr {
    let mut text = PyStr::default();
    for part in parts {
        text.push_py(part);
    }
    text
}

/// `print('  stopped_by:', d['stopped_by'])` on the stop-judgement file.
pub fn stopped_by_line(stop_json_path: &PyStr) -> Result<PyStr, ReportError> {
    let document = load_json(stop_json_path)?;
    let stopped_by = field(&document, "stopped_by")?.display_text();
    Ok(joined(&[&PyStr::new("  stopped_by: "), &stopped_by]))
}

/// `print('[broad_search] stop=%s by=%s 남은셀=%d' % (d['stop'], d['stopped_by'], len(d['cells_remaining'])))`.
pub fn status_line(stop_json_path: &PyStr) -> Result<PyStr, ReportError> {
    let document = load_json(stop_json_path)?;
    let stop = field(&document, "stop")?.display_text();
    let stopped_by = field(&document, "stopped_by")?.display_text();
    let remaining = match field(&document, "cells_remaining")? {
        PyValue::List(items) => items.len(),
        PyValue::Dict(map) => map.len(),
        PyValue::Str(text) => text.len(),
        other => {
            return Err(ReportError::Shape(format!(
                "TypeError: object of type '{}' has no len()",
                other.type_name()
            )))
        }
    };
    Ok(joined(&[
        &PyStr::new("[broad_search] stop="),
        &stop,
        &PyStr::new(" by="),
        &stopped_by,
        &PyStr::new(&format!(" 남은셀={remaining}")),
    ]))
}

/// The repeat count and its source from a state file's `declared_budget`, joined by `\x1f`:
/// `print("%s\x1f%s" % (b.get("repeats", ""), b.get("repeats_source") or "출처 미기재"))`.
pub fn declared_repeats_line(state_path: &PyStr) -> Result<PyStr, ReportError> {
    let document = load_json(state_path)?;
    let PyValue::Dict(top) = &document else {
        return Err(ReportError::Shape(format!(
            "AttributeError: '{}' object has no attribute 'get'",
            document.type_name()
        )));
    };
    let empty = PyValue::Dict(Default::default());
    let budget = top
        .get("declared_budget")
        .filter(|value| value.is_truthy())
        .unwrap_or(&empty);
    let PyValue::Dict(budget) = budget else {
        return Err(ReportError::Shape(format!(
            "AttributeError: '{}' object has no attribute 'get'",
            budget.type_name()
        )));
    };
    let repeats = budget
        .get("repeats")
        .map(PyValue::display_text)
        .unwrap_or_default();
    let source = budget
        .get("repeats_source")
        .filter(|value| value.is_truthy())
        .map_or_else(|| PyStr::new("출처 미기재"), PyValue::display_text);
    Ok(joined(&[&repeats, &PyStr::new("\u{1f}"), &source]))
}
