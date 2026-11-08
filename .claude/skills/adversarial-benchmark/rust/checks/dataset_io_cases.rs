//! The frozen original file criteria of `_load_spec` and `_dir_digest`: real reads, hashes and tree listings, each on
//! a scratch tree built from the recorded one, with the whole tree compared afterwards.

#[path = "dataset_common.rs"]
mod common;

use common::*;
use sweep_stop::dataset_prep::Recorder;
use sweep_stop::dataset_prep_api::evaluate_operation;

#[test]
fn readers_and_digests_match_the_original_with_whole_trees() {
    let cases = list(read_json("io_cases.json"));
    assert_eq!(cases.len(), 19);
    let mut failures = Vec::new();
    for case in &cases {
        let case = dict(case);
        let id = text(field(case, "id"));
        let scratch = Scratch::new("io");
        scratch.populate(field(case, "before"));
        let arguments = dict(&rebase(field(case, "arguments"), "@ROOT@", &scratch.text())).clone();
        let want = dict(field(case, "expected"));
        let want_stderr = want
            .get("stderr")
            .map(|t| text(&rebase(t, "@ROOT@", &scratch.text())))
            .unwrap_or_default();
        let mut streams = Recorder::default();
        let got = evaluate_operation(&text(field(case, "operation")), &arguments, &mut streams);
        let agrees = match (got, text(field(want, "kind")).as_str()) {
            (Ok(value), "value") => {
                key(&value) == key(&rebase(field(want, "value"), "@ROOT@", &scratch.text()))
            }
            (Err(failure), "exception") => {
                failure.class == text(field(want, "type"))
                    && failure.message.to_utf8()
                        == Some(text(&rebase(
                            field(want, "message"),
                            "@ROOT@",
                            &scratch.text(),
                        )))
            }
            _ => false,
        };
        let stderr_ok = streams.stderr.to_utf8().unwrap_or_default() == want_stderr;
        if !(agrees && stderr_ok) || scratch.snapshot() != after_tree(case) {
            failures.push(id);
        }
    }
    assert!(failures.is_empty(), "differing cases {failures:?}");
}
