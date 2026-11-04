//! The 746 pure observations of the ORIGINAL `bench_preset.py` (25 `validate_preset` problem lists and 721
//! `_parse_cli_value` calls, 627 values and 94 exceptions) fed to `bench_preset_api evaluate-jsonl`. Whole records
//! compare by their ordered JSON text, so a `bool`/`int`/`float` or key-order difference is a difference.

#[path = "preset_common.rs"]
mod preset_common;

use preset_common::{command, data_path, real_repo, run};
use sweep_stop::pyjson::loads;

const API: &str = env!("CARGO_BIN_EXE_bench_preset_api");

#[test]
fn frozen_746_original_observations_match_whole_records() {
    let rows: Vec<String> = std::fs::read_to_string(data_path("api.jsonl"))
        .expect("NOT_RUN: api.jsonl")
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(rows.len(), 746);
    let mut payload = String::new();
    let mut expected = Vec::new();
    for line in &rows {
        let row = loads(line).expect("row is JSON");
        let sweep_stop::pyvalue::PyValue::Dict(map) = &row else {
            panic!("row object")
        };
        let mut request = sweep_stop::pyvalue::PyDict::new();
        let mut record = sweep_stop::pyvalue::PyDict::new();
        for key in ["id", "operation", "arguments"] {
            request.set(key, map.get(key).expect("field").clone());
        }
        record.set("id", map.get("id").expect("id").clone());
        let sweep_stop::pyvalue::PyValue::Dict(want) = map.get("expected").expect("expected")
        else {
            panic!("expected object")
        };
        for (name, value) in want.iter() {
            record.insert(name.clone(), value.clone());
        }
        payload.push_str(
            &sweep_stop::pyvalue::PyValue::Dict(request)
                .dumps_ascii()
                .expect("dump request"),
        );
        payload.push('\n');
        expected.push(
            sweep_stop::pyvalue::PyValue::Dict(record)
                .dumps_ascii()
                .expect("dump expected"),
        );
    }
    let mut cmd = command(API, &real_repo());
    cmd.arg("evaluate-jsonl");
    let out = run(cmd, payload.as_bytes());
    assert!(
        out.status.success(),
        "evaluate-jsonl failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let actual: Vec<String> = String::from_utf8(out.stdout)
        .expect("ASCII output")
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(actual.len(), expected.len());
    let mut mismatches = Vec::new();
    for (index, (got, want)) in actual.iter().zip(&expected).enumerate() {
        let got = loads(got)
            .expect("answer is JSON")
            .dumps_ascii()
            .expect("re-dump");
        if &got != want {
            mismatches.push(index);
        }
    }
    assert!(
        mismatches.is_empty(),
        "records differ at {:?}",
        &mismatches[..mismatches.len().min(8)]
    );
}

#[test]
fn constants_and_native_help_are_reachable() {
    let mut cmd = command(API, &real_repo());
    cmd.arg("constants");
    let out = run(cmd, b"");
    assert!(out.status.success());
    let constants = loads(String::from_utf8(out.stdout).expect("ASCII").trim()).expect("JSON");
    let floor = preset_common::field(&constants, "FULL_REPEATS_MIN");
    assert!(matches!(floor.peel(), sweep_stop::pyvalue::PyValue::Int(_)));
    let mut cmd = command(API, &real_repo());
    cmd.arg("native-help");
    assert!(String::from_utf8_lossy(&run(cmd, b"").stdout).contains("evaluate-jsonl"));
}
