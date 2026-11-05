//! The frozen original pure-function criteria: `classify` on 96 recorded inputs - values, exceptions with their exact
//! messages, and the arguments unchanged afterwards.

#[path = "bench_tool_common.rs"]
mod common;

use common::*;
use sweep_stop::bench_tool_api::evaluate_operation;
use sweep_stop::dataset_prep::Recorder;
use sweep_stop::pyvalue::PyValue;

#[test]
fn every_recorded_pure_call_returns_or_raises_what_the_original_did() {
    let rows = read_jsonl("pure_api.jsonl");
    assert_eq!(rows.len(), 96);
    let mut failures = Vec::new();
    for row in &rows {
        let id = text(field(row, "id"));
        let arguments = dict(field(row, "arguments")).clone();
        let want = dict(field(row, "expected"));
        let mut streams = Recorder::default();
        let got = evaluate_operation(&text(field(row, "operation")), &arguments, &mut streams);
        let agrees = match (got, text(field(want, "kind")).as_str()) {
            (Ok(value), "value") => key(&value) == key(field(want, "value")),
            (Err(failure), "exception") => {
                failure.class == text(field(want, "type"))
                    && failure.message.to_utf8() == Some(text(field(want, "message")))
            }
            _ => false,
        };
        let untouched = key(&PyValue::Dict(arguments)) == key(field(row, "arguments_after"));
        if !(agrees && untouched && streams.stderr.is_empty() && streams.stdout.is_empty()) {
            failures.push(id);
        }
    }
    assert!(failures.is_empty(), "differing calls {failures:?}");
}
