//! `-h` and the usage of an argument error for `roofline` and `verdict_rule`, at the first and last terminal width
//! of every distinct layout the ORIGINAL produced for COLUMNS 1..700 (`layouts.jsonl`). Widths above 700 share the
//! last layout (the longest help line is shorter than that).

#[path = "common.rs"]
mod common;

use common::{command, load_jsonl, text_of};
use std::process::Stdio;
use sweep_stop::pyvalue::PyValue;

fn bin(tool: &str) -> &'static str {
    match tool {
        "roofline" => env!("CARGO_BIN_EXE_roofline"),
        "verdict_rule" => env!("CARGO_BIN_EXE_verdict_rule"),
        other => panic!("unknown tool {other}"),
    }
}

fn number(value: Option<&PyValue>) -> u32 {
    value
        .and_then(PyValue::as_plain_int)
        .unwrap()
        .to_string()
        .parse()
        .unwrap()
}

#[test]
fn every_layout_matches_at_both_ends_of_its_width_range() {
    let layouts = load_jsonl("roofline_verdict/layouts.jsonl");
    assert!(layouts.len() >= 200, "layout set shrank: {}", layouts.len());
    let mut failures = Vec::new();
    for layout in &layouts {
        let tool = text_of(layout.get("tool"));
        let (from, to) = (number(layout.get("from")), number(layout.get("to")));
        let (help, err) = (text_of(layout.get("help")), text_of(layout.get("err")));
        for columns in [from, to] {
            let value = columns.to_string();
            let helped = command(bin(&tool), Some(&value), &["-h".to_string()])
                .stdin(Stdio::null())
                .output()
                .expect("run");
            if helped.status.code() != Some(0) || String::from_utf8_lossy(&helped.stdout) != help {
                failures.push(format!("{tool} -h COLUMNS={columns}"));
            }
            let bogus = command(bin(&tool), Some(&value), &["--bogus".to_string()])
                .stdin(Stdio::null())
                .output()
                .expect("run");
            if bogus.status.code() != Some(2) || String::from_utf8_lossy(&bogus.stderr) != err {
                failures.push(format!("{tool} --bogus COLUMNS={columns}"));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} widths differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
