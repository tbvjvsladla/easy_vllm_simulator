//! Differential checks of the Python behaviours the port re-implements, against data captured from
//! CPython 3.12.3 by `r1-worker-checks/gen_oracle_data.py`:
//!
//! * `_parse_utc` (strip, `fromisoformat`, `astimezone`, `strftime`) over about 27 000 inputs;
//! * `json.loads` results and `JSONDecodeError` text over about 5 000 documents.

#[path = "common.rs"]
mod common;

use common::{load_jsonl, text_of};
use sweep_stop::pyjson::{loads, LoadError};
use sweep_stop::pystr::PyStr;
use sweep_stop::pyvalue::PyValue;
use sweep_stop::{parse_utc, SweepError};

#[test]
fn parse_utc_matches_cpython() {
    let records = load_jsonl("parse_utc_oracle.jsonl");
    let mut failures = Vec::new();
    for record in &records {
        let value = record.get("value").expect("record has value");
        let field = record
            .get("field")
            .map_or_else(|| "--now-utc".to_string(), |f| text_of(Some(f)));
        let got = parse_utc(value, &field);
        let ok = match (&got, record.get("utc")) {
            (Ok(instant), Some(utc)) => {
                let evaluated_ok = record
                    .get("evaluated")
                    .is_none_or(|e| text_of(Some(e)) == instant.evaluated_text());
                text_of(Some(utc)) == instant.iso_text() && evaluated_ok
            }
            (Err(error), None) => {
                text_of(record.get("type")) == error.python_type()
                    && text_of(record.get("message")) == error.message()
            }
            _ => false,
        };
        if !ok {
            let want = record.get("utc").map_or_else(
                || {
                    format!(
                        "{}: {}",
                        text_of(record.get("type")),
                        text_of(record.get("message"))
                    )
                },
                |utc| text_of(Some(utc)),
            );
            let have = match got {
                Ok(instant) => instant.iso_text(),
                Err(error) => format!("{}: {}", error.python_type(), error.message()),
            };
            failures.push(format!(
                "  input {}\n    got  {have}\n    want {want}",
                value.repr()
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} timestamps differ (first 25):\n{}",
        failures.len(),
        records.len(),
        failures
            .iter()
            .take(25)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn json_loads_matches_cpython() {
    let records = load_jsonl("json_decode_oracle.jsonl");
    let mut failures = Vec::new();
    for record in &records {
        let text = text_of(record.get("text"));
        let got = match loads(&text) {
            Ok(value) => format!("OK {}", value.repr()),
            Err(error @ LoadError::Decode { .. }) => format!("ERR JSONDecodeError: {error}"),
            Err(error @ LoadError::IntegerTooLong { .. }) => format!("ERR ValueError: {error}"),
            Err(error) => format!("ERR {error}"),
        };
        let want = match record.get("repr") {
            Some(repr) => format!("OK {}", text_of(Some(repr))),
            None => format!("ERR {}", text_of(record.get("error"))),
        };
        if got != want {
            let shown: String = text.chars().take(60).collect();
            failures.push(format!(
                "  input {}\n    got  {got}\n    want {want}",
                PyValue::str(&shown).repr()
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} documents differ (first 25):\n{}",
        failures.len(),
        records.len(),
        failures
            .iter()
            .take(25)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn str_repr_and_whitespace_match_cpython_at_every_boundary() {
    let records = load_jsonl("str_repr_oracle.jsonl");
    let mut failures = Vec::new();
    for record in &records {
        let code_point = u32::try_from(
            record
                .get("cp")
                .and_then(PyValue::as_plain_int)
                .unwrap()
                .to_string()
                .parse::<u64>()
                .unwrap(),
        )
        .unwrap();
        let text = PyStr::from_code_points(vec![code_point]);
        let space = matches!(record.get("space"), Some(PyValue::Bool(true)));
        if text.repr() != text_of(record.get("repr")) || text.strip().is_empty() != space {
            failures.push(format!(
                "U+{code_point:04X}: got {} want {}",
                text.repr(),
                text_of(record.get("repr"))
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} code points differ:\n{}",
        failures.len(),
        records.len(),
        failures.join("\n")
    );
}

#[test]
fn error_kinds_map_to_python_exception_names() {
    assert_eq!(
        SweepError::StopEval(String::new()).python_type(),
        "StopEvalError"
    );
    assert_eq!(
        SweepError::NotIterable { type_name: "int" }.python_type(),
        "TypeError"
    );
    assert_eq!(
        SweepError::Overflow(String::new()).python_type(),
        "OverflowError"
    );
}
