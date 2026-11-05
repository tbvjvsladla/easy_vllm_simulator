//! `pylower::python_lower` against what CPython 3.12.3 (Unicode 15.0.0) answered for `str.lower()`: every scalar (as one
//! digest of all 1,112,064 answers) and 600 strings built to stress the Final_Sigma context (cased and case-ignorable
//! neighbours, the one-to-many U+0130, scalars Unicode 15 does not know but newer Unicode does). The data was produced
//! once by the interpreter itself, not by this crate, so it is an oracle and not an echo of the table.

use ring::digest::{Context, SHA256};
use std::path::Path;
use sweep_stop::pyjson::loads;
use sweep_stop::pylower::python_lower;
use sweep_stop::pyvalue::PyValue;

fn oracle() -> PyValue {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("checks/data/lowercase/oracle.json");
    loads(&std::fs::read_to_string(path).expect("oracle")).expect("JSON")
}

fn member(value: &PyValue, key: &str) -> PyValue {
    match value.peel() {
        PyValue::Dict(map) => map.get(key).expect("member").clone(),
        other => panic!("not an object: {}", other.repr()),
    }
}

fn text(value: &PyValue) -> String {
    match value.peel() {
        PyValue::Str(own) => own.to_utf8().expect("scalar text"),
        other => panic!("not a string: {}", other.repr()),
    }
}

#[test]
fn every_scalar_lowers_as_cpython_3_12_3_does() {
    let mut context = Context::new(&SHA256);
    for point in 0..0x11_0000_u32 {
        let Some(scalar) = char::from_u32(point) else {
            continue;
        };
        context.update(python_lower(&scalar.to_string()).as_bytes());
        context.update(&[0]);
    }
    let hex: String = context
        .finish()
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(hex, text(&member(&oracle(), "scalars_sha256")));
}

#[test]
fn the_final_sigma_and_new_scalar_contexts_lower_as_cpython_does() {
    let document = oracle();
    let PyValue::List(rows) = member(&document, "contexts").peel().clone() else {
        panic!("contexts")
    };
    assert_eq!(rows.len(), 600);
    let mut final_forms = 0;
    for row in &rows {
        let PyValue::List(pair) = row.peel() else {
            panic!("row")
        };
        let (given, want) = (text(&pair[0]), text(&pair[1]));
        assert_eq!(python_lower(&given), want, "{given:?}");
        final_forms += usize::from(want.contains('\u{3c2}'));
    }
    assert!(
        final_forms > 20,
        "the strings reach the final form often enough to mean something: {final_forms}"
    );
}

#[test]
fn rusts_own_lower_case_is_not_cpythons_for_a_scalar_unicode_15_does_not_know() {
    // the reason the helper exists: Rust's newer Unicode maps U+1C89, CPython 3.12.3 leaves it
    assert_ne!("\u{1c89}".to_lowercase(), "\u{1c89}");
    assert_eq!(python_lower("\u{1c89}"), "\u{1c89}");
}
