//! Row shapes of a pin record that the frozen criteria do not reach: how `dict(row)` reads a non-object row (pairs,
//! characters, scalars), keys that are not strings, an own `version` key, and the digest set of `accepted_digests`.
//! The expected texts are what CPython raises or returns for the same `dict(...)` call.

#[path = "bench_tool_common.rs"]
mod common;

use common::*;
use sweep_stop::bench_tool_api::evaluate_operation;
use sweep_stop::bench_tool_pin::accepted_digests;
use sweep_stop::dataset_prep::Recorder;
use sweep_stop::pyjson::loads;
use sweep_stop::pyvalue::{PyDict, PyValue};

fn load(row_json: &str) -> Result<String, (String, String)> {
    let scratch = Scratch::new("row");
    let record =
        format!(r#"{{"tools":{{"t":{{"default_version":"1","versions":{{"1":{row_json}}}}}}}}}"#);
    std::fs::write(scratch.root.join("pin.json"), record).expect("record");
    let mut arguments = PyDict::new();
    arguments.set(
        "path",
        PyValue::str(&format!("{}/pin.json", scratch.text())),
    );
    arguments.set("tool", PyValue::str("t"));
    let mut streams = Recorder::default();
    evaluate_operation("load_pin", &arguments, &mut streams)
        .map(|value| key(&value))
        .map_err(|failure| {
            (
                failure.class.to_string(),
                failure.message.to_utf8().unwrap_or_default(),
            )
        })
}

fn raised(class: &str, message: &str) -> Result<String, (String, String)> {
    Err((class.to_string(), message.to_string()))
}

#[test]
fn a_row_that_is_not_an_object_is_read_as_dict_reads_it() {
    assert_eq!(
        load(r#"[["image_ref","a"],["index_digest",null]]"#),
        Ok(r#"{"image_ref": "a", "index_digest": null, "version": "1"}"#.to_string())
    );
    assert_eq!(
        load(r#"[["image_ref","a"],[1,"b"]]"#),
        Ok(r#"{"image_ref": "a", "1": "b", "version": "1"}"#.to_string())
    );
    assert_eq!(
        load(r#"{"image_ref":"a","version":"old","z":1}"#),
        Ok(r#"{"image_ref": "a", "version": "1", "z": 1}"#.to_string())
    );
    assert_eq!(
        load(r#""""#),
        raised("PinError", "기록 t.1.image_ref 가 비었다")
    );
    assert_eq!(
        load(r#"[{"a":1,"b":2}]"#),
        raised("PinError", "기록 t.1.image_ref 가 비었다")
    );
    assert_eq!(
        load(r#""ab""#),
        raised(
            "ValueError",
            "dictionary update sequence element #0 has length 1; 2 is required"
        )
    );
    assert_eq!(
        load(r#"["abc"]"#),
        raised(
            "ValueError",
            "dictionary update sequence element #0 has length 3; 2 is required"
        )
    );
    assert_eq!(
        load(r#"[[]]"#),
        raised(
            "ValueError",
            "dictionary update sequence element #0 has length 0; 2 is required"
        )
    );
    assert_eq!(
        load("null"),
        raised("TypeError", "'NoneType' object is not iterable")
    );
    assert_eq!(
        load("5"),
        raised("TypeError", "'int' object is not iterable")
    );
    assert_eq!(
        load("[1]"),
        raised(
            "TypeError",
            "cannot convert dictionary update sequence element #0 to a sequence"
        )
    );
    assert_eq!(
        load(r#"[[["x"],1]]"#),
        raised("TypeError", "unhashable type: 'list'")
    );
}

#[test]
fn the_digest_set_holds_the_index_and_the_non_empty_platform_strings_only() {
    let entry = loads(
        r#"{"index_digest":"sha256:a","platform_digests":{"a":"","b":null,"c":3,"d":"sha256:b"}}"#,
    )
    .expect("json");
    let known = accepted_digests(&entry).expect("set");
    let texts: Vec<String> = known.texts().filter_map(|t| t.to_utf8()).collect();
    assert_eq!(texts, ["sha256:a", "sha256:b"]);
    assert!(!known.is_empty());
    let numeric = accepted_digests(&loads(r#"{"index_digest":123}"#).expect("json")).expect("set");
    assert!(!numeric.is_empty() && numeric.texts().next().is_none());
    let falsy = accepted_digests(&loads(r#"{"index_digest":0}"#).expect("json")).expect("set");
    assert!(falsy.is_empty());
}
