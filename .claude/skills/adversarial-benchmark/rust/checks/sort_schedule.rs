//! The comparison schedule of `list.sort` (CPython 3.12.3), proven against the interpreter itself.
//!
//! * `repeat_axis/sort_schedule_goldens.jsonl` - 439 bounded, structured key sets (ascending/descending/duplicate/
//!   sawtooth/block/organ-pipe runs that drive run discovery, binary insertion, `minrun`, both galloping modes and the
//!   powersort merge policy; floats with and without NaN; mixed numbers; strings; list keys; mixed types that make a
//!   comparison fail at assorted positions; lengths 0..300 around the 63/64, 127/128/129 and 255/256/257
//!   boundaries). Each row holds the comparisons CPython made - a logging key class records `(left, right)` at every
//!   `__lt__` - and either the resulting permutation or the exception. The port must make exactly the same
//!   comparisons in the same order, and end the same way.
//! * `repeat_axis/sort_boundary_api.jsonl` - the 8 original `summarize` observations root froze for lengths 63..128
//!   with NaN levels, fed to `evaluate-jsonl` and compared as whole ordered records.

use std::io::Write;
use std::process::{Command, Stdio};
use sweep_stop::pyjson::loads;
use sweep_stop::pysort::comparison_schedule;
use sweep_stop::pyvalue::{PyDict, PyValue};

fn data(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("checks/data/repeat_axis")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn as_dict(value: &PyValue) -> &PyDict {
    match value {
        PyValue::Dict(map) => map,
        other => panic!("expected object, got {}", other.repr()),
    }
}

fn text(value: Option<&PyValue>) -> String {
    match value {
        Some(PyValue::Str(text)) => text.to_utf8().unwrap(),
        other => panic!("expected string, got {other:?}"),
    }
}

fn indices(value: &PyValue) -> Vec<usize> {
    match value {
        PyValue::List(items) => items
            .iter()
            .map(|item| match item {
                PyValue::Int(number) => number.to_string().parse().unwrap(),
                other => panic!("expected int, got {}", other.repr()),
            })
            .collect(),
        other => panic!("expected list, got {}", other.repr()),
    }
}

#[test]
fn comparison_schedule_equals_the_interpreters() {
    let mut count = 0;
    let (mut longest, mut with_error, mut galloping_sized) = (0, 0, 0);
    for line in data("sort_schedule_goldens.jsonl").lines() {
        let row = loads(line).unwrap();
        let row = as_dict(&row);
        let id = text(row.get("id"));
        let keys = match row.get("keys").unwrap() {
            PyValue::List(keys) => keys.clone(),
            other => panic!("{id}: keys {}", other.repr()),
        };
        let expected_pairs = indices(row.get("pairs").unwrap());
        let (pairs, outcome) = comparison_schedule(&keys);
        let flat: Vec<usize> = pairs
            .iter()
            .flat_map(|(left, right)| [*left, *right])
            .collect();
        assert_eq!(
            flat, expected_pairs,
            "{id}: comparisons (flat left,right key indices)"
        );
        match (row.get("order"), row.get("exception"), outcome) {
            (Some(order), None, Ok(own)) => assert_eq!(own, indices(order), "{id}: order"),
            (None, Some(exception), Err(failure)) => {
                let exception = as_dict(exception);
                assert_eq!(failure.class, text(exception.get("type")), "{id}");
                assert_eq!(
                    failure.message.to_utf8().unwrap(),
                    text(exception.get("message")),
                    "{id}"
                );
                with_error += 1;
            }
            (_, _, own) => panic!("{id}: outcome kinds differ: {:?}", own.map(|o| o.len())),
        }
        longest = longest.max(keys.len());
        galloping_sized += usize::from(keys.len() >= 64);
        count += 1;
    }
    assert_eq!(count, 439);
    assert!(
        longest >= 300 && with_error >= 70 && galloping_sized >= 250,
        "the golden set must keep its reach"
    );
}

#[test]
fn nan_levels_beyond_the_old_limit_summarize_as_the_original_does() {
    let raw = data("sort_boundary_api.jsonl");
    let records: Vec<PyDict> = raw
        .lines()
        .map(|line| as_dict(&loads(line).unwrap()).clone())
        .collect();
    assert_eq!(records.len(), 8);
    let mut input = String::new();
    for record in &records {
        let mut row = PyDict::new();
        for key in ["id", "operation", "arguments"] {
            row.set(key, record.get(key).unwrap().clone());
        }
        input.push_str(&PyValue::Dict(row).dumps_ascii().unwrap());
        input.push('\n');
    }
    let mut child = Command::new(env!("CARGO_BIN_EXE_repeat_axis_api"))
        .arg("evaluate-jsonl")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    let lines: Vec<String> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(lines.len(), 8);
    for (record, line) in records.iter().zip(lines) {
        let answer = loads(&line).unwrap();
        let answer = as_dict(&answer);
        let expected = as_dict(record.get("expected").unwrap());
        let id = text(record.get("id"));
        assert_eq!(text(answer.get("kind")), text(expected.get("kind")), "{id}");
        assert_eq!(
            answer.get("value").unwrap().dumps_ascii().unwrap(),
            expected.get("value").unwrap().dumps_ascii().unwrap(),
            "{id}: whole ordered summary"
        );
    }
}
