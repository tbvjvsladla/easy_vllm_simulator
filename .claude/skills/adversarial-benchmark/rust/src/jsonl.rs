//! Differential-test transport: JSON lines in, JSON lines out.
//!
//! Each non-blank input line is `{"id": ..., "state": ..., "now_utc": ...}`. `now_utc` may be any JSON value
//! because the original accepts any Python object there and rejects non-strings with its own message. Every
//! input line yields exactly one output line, so a missing result can never be mistaken for a pass.

use crate::evaluate::evaluate;
use crate::pyjson::loads;
use crate::pyvalue::{DumpError, PyDict, PyValue};
use crate::utc::parse_utc;

/// Output lines for all records in `text`, each terminated by a newline.
pub fn evaluate_lines(text: &str) -> String {
    let mut output = String::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let answer = evaluate_line(line);
        match answer.dumps_ascii() {
            Ok(text) => output.push_str(&text),
            Err(error) => output.push_str(&unprintable(&answer, &error)),
        }
        output.push('\n');
    }
    output
}

fn evaluate_line(line: &str) -> PyValue {
    let record = match loads(line) {
        Ok(PyValue::Dict(record)) => record,
        Ok(other) => {
            return invalid_input(
                PyValue::None,
                format!("record must be a JSON object, got {}", other.type_name()),
            )
        }
        Err(error) => return invalid_input(PyValue::None, error.to_string()),
    };
    let id = record.get("id").cloned().unwrap_or(PyValue::None);
    let (Some(state), Some(now_utc)) = (record.get("state"), record.get("now_utc")) else {
        return invalid_input(
            id,
            "record needs both \"state\" and \"now_utc\"".to_string(),
        );
    };

    // The original evaluates the timestamp argument first, so its errors win over state errors.
    let verdict = parse_utc(now_utc, "--now-utc").and_then(|now| evaluate(state, &now));
    let mut result = PyDict::new();
    result.set("id", id);
    match verdict {
        Ok(evaluation) => {
            result.set("kind", PyValue::str("value"));
            result.set("value", evaluation.to_value());
            result.set("exit_code", PyValue::int(evaluation.exit_code()));
        }
        Err(error) => {
            result.set("kind", PyValue::str("exception"));
            result.set("type", PyValue::str(error.python_type()));
            result.set("message", PyValue::str(&error.message()));
        }
    }
    PyValue::Dict(result)
}

fn invalid_input(id: PyValue, message: String) -> PyValue {
    let mut result = PyDict::new();
    result.set("id", id);
    result.set("kind", PyValue::str("invalid_input"));
    result.set("message", PyValue::str(&message));
    PyValue::Dict(result)
}

/// A verdict Python could compute but not print (for example an integer beyond 4300 digits) still gets its line.
fn unprintable(answer: &PyValue, error: &DumpError) -> String {
    let id = match answer {
        PyValue::Dict(map) => map.get("id").cloned().unwrap_or(PyValue::None),
        _ => PyValue::None,
    };
    let mut result = PyDict::new();
    result.set("id", id);
    result.set("kind", PyValue::str("exception"));
    result.set("type", PyValue::str(error.python_type()));
    result.set("message", PyValue::str(&error.to_string()));
    PyValue::Dict(result)
        .dumps_ascii()
        .unwrap_or_else(|_| "{\"kind\": \"invalid_input\"}".to_string())
}
