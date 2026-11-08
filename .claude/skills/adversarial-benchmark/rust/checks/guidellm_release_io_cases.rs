//! The frozen original file criteria of `merge_record`: 19 real calls (11 values, 8 exceptions), each on a scratch tree
//! built from the recorded one, with the whole tree compared afterwards - including the partial file a failing dump
//! leaves behind.

#[path = "guidellm_release_common.rs"]
mod common;

use common::*;
use sweep_stop::dataset_prep::Recorder;
use sweep_stop::guidellm_release_api::{evaluate_operation, RefusedTransport};

#[test]
fn merges_match_the_original_with_whole_trees() {
    let cases = list(read_json("io_cases.json"));
    assert_eq!(cases.len(), 19);
    let (mut values, mut exceptions) = (0, 0);
    let mut failures = Vec::new();
    for case in &cases {
        let case = dict(case);
        let id = text(field(case, "id"));
        let scratch = Scratch::new("io");
        scratch.populate(field(case, "before"));
        let arguments = dict(&rebase(field(case, "arguments"), "@ROOT@", &scratch.text())).clone();
        let want = dict(field(case, "expected"));
        let mut streams = Recorder::default();
        let got = evaluate_operation(
            &text(field(case, "operation")),
            &arguments,
            &mut streams,
            &RefusedTransport,
        );
        let agrees = match (got, text(field(want, "kind")).as_str()) {
            (Ok(value), "value") => {
                values += 1;
                key(&value) == key(&rebase(field(want, "value"), "@ROOT@", &scratch.text()))
            }
            (Err(failure), "exception") => {
                exceptions += 1;
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
        if !agrees
            || !streams.stderr.is_empty()
            || !streams.stdout.is_empty()
            || scratch.snapshot() != after_tree(case)
        {
            failures.push(id);
        }
    }
    assert!(failures.is_empty(), "differing cases {failures:?}");
    assert_eq!((values, exceptions), (11, 8));
}

/// `open(path, "w")` truncates the file in place: a second name of the same inode (a hard link) sees the partial
/// document a failing dump leaves, and the document of a successful merge.
#[test]
fn the_record_is_rewritten_in_place_so_a_hard_link_follows_it() {
    use sweep_stop::pyvalue::PyValue;
    let cases = list(read_json("io_cases.json"));
    let base = cases
        .iter()
        .map(dict)
        .find(|case| text(field(case, "id")) == "026-merge-digest-26")
        .expect("a merge case");
    let scratch = Scratch::new("alias");
    scratch.populate(field(base, "before"));
    std::fs::hard_link(
        scratch.root.join("record.json"),
        scratch.root.join("alias.json"),
    )
    .expect("link");
    let arguments = dict(&rebase(field(base, "arguments"), "@ROOT@", &scratch.text())).clone();
    let mut streams = Recorder::default();
    let got = evaluate_operation("merge_record", &arguments, &mut streams, &RefusedTransport);
    assert!(matches!(got, Ok(PyValue::Str(_))));
    assert_eq!(
        std::fs::read(scratch.root.join("alias.json")).expect("alias"),
        std::fs::read(scratch.root.join("record.json")).expect("record")
    );
}
