//! The frozen original-only file criteria: real readers (`load`, `_read_text`, the manifest path labels, the
//! naming scan, the repository root walk, the lite identity precedence), each on a scratch tree built from the
//! recorded one, with the whole tree compared afterwards.

#[path = "reporting_common.rs"]
mod common;

use common::*;
use sweep_stop::bench_reporting_api::evaluate_operation;
use sweep_stop::pyvalue::PyValue;
use sweep_stop::report_support::Diagnostics;

#[test]
fn readers_identity_and_scan_match_the_original_with_whole_trees() {
    let PyValue::List(cases) = read_json("io_cases.json") else {
        panic!("cases are a list")
    };
    assert_eq!(cases.len(), 39);
    let mut failures = Vec::new();
    for case in &cases {
        let case = dict(case);
        let id = text(field(case, "id"));
        let scratch = Scratch::new("io");
        scratch.populate(field(case, "before"));
        let arguments = dict(&rebase(field(case, "arguments"), "@ROOT@", &scratch.text())).clone();
        let want = dict(field(case, "expected"));
        let operation = format!(
            "{}.{}",
            text(field(case, "owner")),
            text(field(case, "operation"))
        );
        let mut diagnostics = Diagnostics::default();
        let got = evaluate_operation(&operation, &arguments, &mut diagnostics);
        let agrees = match (got, text(field(want, "kind")).as_str()) {
            (Ok(value), "value") => {
                key(&value) == key(&rebase(field(want, "value"), "@ROOT@", &scratch.text()))
            }
            (Err(failure), "exception") => {
                let expected_stderr = want
                    .get("stderr")
                    .map(|t| text(&rebase(t, "@ROOT@", &scratch.text())));
                failure.class == text(field(want, "type"))
                    && failure.message.to_utf8()
                        == Some(text(&rebase(
                            field(want, "message"),
                            "@ROOT@",
                            &scratch.text(),
                        )))
                    && diagnostics.text().to_utf8().unwrap_or_default()
                        == expected_stderr.unwrap_or_default()
            }
            _ => false,
        };
        if !agrees || scratch.snapshot() != expected_tree(field(case, "after")) {
            failures.push(id);
        }
    }
    assert!(failures.is_empty(), "differing cases {failures:?}");
}
