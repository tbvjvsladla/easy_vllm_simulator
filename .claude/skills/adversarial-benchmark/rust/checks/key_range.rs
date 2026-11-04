//! Dict keys made of YAML numbers (findings R3-K2 and R3-K3), against what the ORIGINAL printed.
//!
//! * K2: a float key with an integral value is the same Python key as the integer of that exact value, however large
//!   (`1e40` and `2e40` are two keys; `True`, `1` and `1.0` are one). A saturating cast made the big ones collide.
//! * K3: a NaN key is equal only to the same *object*. PyYAML's `SafeConstructor` returns one shared object for the
//!   `.nan` scalar and builds a fresh one for every other NaN scalar node (`!!float nan`), and an alias reuses its
//!   node's object - so `{.nan: 1, .nan: 2}` has one entry and `{!!float nan: 1, !!float nan: 2}` has two.
//!
//! `key_range_cases.json` keeps the original's stdout as raw text: where it has two entries with the same JSON spelling
//! (`"NaN": 1, "NaN": 2`) a parsed comparison would hide the difference. 21 cases: the four origin observations, the root's
//! and the independent verifier's reproductions, and fourteen more captured from the original.

#[path = "common.rs"]
mod common;

use common::{command, data_path, dict, hex_decode, list, load_json_file, text_of};
use std::process::Stdio;
use sweep_stop::pyvalue::PyValue;

fn int_of(value: Option<&PyValue>) -> i32 {
    match value {
        Some(PyValue::Int(number)) => number.to_string().parse().expect("small integer"),
        other => panic!("expected an integer, got {other:?}"),
    }
}

#[test]
fn number_and_nan_keys_are_the_originals() {
    let fixture = load_json_file(&data_path("roofline_verdict/key_range_cases.json"));
    let fixture = dict(&fixture);
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("key_range_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("model")).expect("scratch");
    let PyValue::Dict(files) = fixture.get("files").expect("files") else {
        panic!("files is not an object");
    };
    for (name, content) in files.iter() {
        let name = name.to_utf8().expect("name");
        std::fs::write(dir.join(&name), hex_decode(&text_of(Some(content)))).expect("model file");
    }
    let PyValue::Dict(manifests) = fixture.get("manifests").expect("manifests") else {
        panic!("manifests is not an object");
    };
    for (name, content) in manifests.iter() {
        std::fs::write(
            dir.join(name.to_utf8().expect("name")),
            text_of(Some(content)),
        )
        .expect("manifest");
    }
    let model = dir.join("model");
    let cases = list(fixture.get("cases").expect("cases").clone());
    assert!(cases.len() >= 21, "only {} key cases", cases.len());
    let mut failures = Vec::new();
    for case in &cases {
        let case = dict(case);
        let id = text_of(case.get("id"));
        let manifest = dir.join(text_of(case.get("manifest")));
        let output = command(
            env!("CARGO_BIN_EXE_roofline"),
            None,
            &[
                "--model-path".to_string(),
                model.to_str().unwrap().to_string(),
                "--manifest".to_string(),
                manifest.to_str().unwrap().to_string(),
                "--tp".to_string(),
                "1".to_string(),
                "--bandwidth-gbps".to_string(),
                "273".to_string(),
            ],
        )
        .stdin(Stdio::null())
        .output()
        .expect("run");
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let want = text_of(case.get("stdout")).replace("{MODEL}", model.to_str().unwrap());
        let exit = output.status.code().unwrap_or(-1);
        if exit != int_of(case.get("exit"))
            || stdout != want
            || String::from_utf8_lossy(&output.stderr) != text_of(case.get("stderr"))
        {
            failures.push(format!(
                "{id}: exit {exit} (original {}), stdout differs: {}",
                int_of(case.get("exit")),
                stdout != want
            ));
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        failures.is_empty(),
        "{} of {} key cases differ:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}
