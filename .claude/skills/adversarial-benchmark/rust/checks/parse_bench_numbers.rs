//! Python's `float()`, `int()`, `float(int)` and `round(x, 2)` against values captured from the interpreter that
//! runs the original (`data/parse_bench/numbers.jsonl`): the string grammars (underscores, signs, Unicode digits
//! and whitespace, NaN/Infinity, the 4300-digit limit), the rounding of integers beyond 2^53 and of the double
//! range's end, and the half-even rounding of exact decimal expansions (ties, the classic 2.675/1.005, 1000/x
//! quotients, subnormals).

#[path = "common.rs"]
mod common;

use common::{load_jsonl, text_of};
use num_bigint::BigInt;
use sweep_stop::pynumber::{float_from_int, float_from_str, int_from_str, round_to_two_places};
use sweep_stop::pystr::PyStr;
use sweep_stop::pyvalue::{float_repr, PyValue};

fn python_text(record: &sweep_stop::pyvalue::PyDict) -> PyStr {
    match record.get("input") {
        Some(PyValue::Str(text)) => text.clone(),
        other => panic!("input must be a string, got {other:?}"),
    }
}

#[test]
fn conversions_match_the_interpreter() {
    let mut failures = Vec::new();
    let records = load_jsonl("parse_bench/numbers.jsonl");
    assert!(
        records.len() >= 2000,
        "golden set shrank: {}",
        records.len()
    );
    for record in &records {
        let kind = text_of(record.get("kind"));
        let want = text_of(record.get("want"));
        let input = python_text(record);
        let got = match kind.as_str() {
            "float_str" => match float_from_str(&input) {
                Ok(number) => float_repr(number),
                Err(error) => error.class.to_string(),
            },
            "int_str" => match int_from_str(&input) {
                Ok(number) => number.to_string(),
                Err(error) => format!(
                    "{}:{}",
                    error.class,
                    error.message.chars().take(60).collect::<String>()
                ),
            },
            "float_int" => {
                let number: BigInt = input.to_utf8().unwrap().parse().unwrap();
                match float_from_int(&number) {
                    Ok(value) => float_repr(value),
                    Err(error) => error.class.to_string(),
                }
            }
            "round2" => {
                let text = input.to_utf8().unwrap();
                let number: f64 = match text.as_str() {
                    "inf" => f64::INFINITY,
                    "-inf" => f64::NEG_INFINITY,
                    "nan" => f64::NAN,
                    other => other.parse().unwrap(),
                };
                float_repr(round_to_two_places(number))
            }
            other => panic!("unknown kind {other}"),
        };
        if got != want {
            failures.push(format!("{kind} {:?}: got {got}, want {want}", input.repr()));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} differ:\n{}",
        failures.len(),
        records.len(),
        failures.join("\n")
    );
}
