//! How deep a manifest may nest, and what a shared alias costs - against what the real original did.
//!
//! * The original's composer recurses once per nesting level, so `RecursionError` comes when PyYAML's stack of Python
//!   frames reaches the interpreter's limit, and the frames a level costs depend on what is being scanned at the
//!   bottom. `roofline yaml-depth` reports the port's own count; the expectations come from a profile hook on the
//!   original (`yaml_depth_expect.jsonl`: deepest stack, and for small documents the number of calls and an FNV-1a 64
//!   of every `depth name` line), and `yaml_boundary_cases.jsonl` holds manifests on both sides of the depth where
//!   the original CLI stops reading them (its exit code and handled stderr).
//! * Python shares the object an alias points at. `alias_sharing_case.json` is a 628-byte manifest whose 26 unused
//!   fields would expand to tens of millions of nodes if copied; the original answers it normally, and so must the
//!   port inside the same child-only limits (1.5 GiB of address space, 3 CPU seconds, no core file).

#[path = "common.rs"]
mod common;

use common::{
    command, data_path, dict, hex_decode, load_json_file, load_jsonl, roofline_inputs, text_of,
};
use std::collections::HashMap;
use std::process::{Command, Output, Stdio};
use sweep_stop::pyjson::loads_text;
use sweep_stop::pyvalue::PyValue;

fn scratch(tag: &str) -> std::path::PathBuf {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("yaml_depth_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch");
    dir
}

fn fnv(calls: &[(i64, String)]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for (depth, name) in calls {
        for byte in format!("{depth} {name}\n").bytes() {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    format!("{hash:016x}")
}

fn int_of(value: Option<&PyValue>) -> i64 {
    match value {
        Some(PyValue::Int(number)) => number.to_string().parse().expect("small integer"),
        other => panic!("expected an integer, got {other:?}"),
    }
}

#[test]
fn stack_depth_and_calls_match_the_original() {
    let expectations = load_jsonl("roofline_verdict/yaml_depth_expect.jsonl");
    let mut documents: HashMap<String, Vec<u8>> = HashMap::new();
    for file in [
        "roofline_verdict/yaml_reader_cases.jsonl",
        "roofline_verdict/yaml_reader_cases2.jsonl",
    ] {
        for row in load_jsonl(file) {
            documents.insert(
                text_of(row.get("name")),
                hex_decode(&text_of(row.get("hex"))),
            );
        }
    }
    assert!(
        expectations.len() >= 3200,
        "only {} expectations",
        expectations.len()
    );
    let dir = scratch("calls");
    let mut failures = Vec::new();
    let mut compared_calls = 0;
    for row in &expectations {
        let name = text_of(row.get("name"));
        let path = dir.join(format!("{name}.yaml"));
        std::fs::write(&path, &documents[&name]).expect("document");
        let output = command(
            env!("CARGO_BIN_EXE_roofline"),
            None,
            &["yaml-depth".to_string(), path.to_str().unwrap().to_string()],
        )
        .stdin(Stdio::null())
        .output()
        .expect("run");
        let text = String::from_utf8_lossy(&output.stdout).to_string();
        let PyValue::Dict(report) =
            loads_text(&sweep_stop::pystr::PyStr::new(&text)).expect("yaml-depth prints JSON")
        else {
            panic!("yaml-depth did not print an object for {name}");
        };
        let max = int_of(report.get("max"));
        if max != int_of(row.get("max")) {
            failures.push(format!(
                "{name}: deepest stack {max}, the original's {}",
                int_of(row.get("max"))
            ));
            continue;
        }
        if let Some(expected_len) = row.get("calls") {
            let PyValue::List(calls) = report.get("calls").expect("calls") else {
                panic!("calls is not a list");
            };
            let calls: Vec<(i64, String)> = calls
                .iter()
                .map(|call| {
                    let PyValue::List(pair) = call else {
                        panic!("call is not a pair")
                    };
                    (int_of(pair.first()), text_of(pair.get(1)))
                })
                .collect();
            compared_calls += 1;
            if calls.len() as i64 != int_of(Some(expected_len))
                || fnv(&calls) != text_of(row.get("fnv"))
            {
                failures.push(format!(
                    "{name}: {} calls (fnv {}), the original's {} (fnv {})",
                    calls.len(),
                    fnv(&calls),
                    int_of(Some(expected_len)),
                    text_of(row.get("fnv"))
                ));
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(compared_calls >= 3000, "only {compared_calls} call lists");
    assert!(
        failures.is_empty(),
        "{} of {} documents differ:\n{}",
        failures.len(),
        expectations.len(),
        failures[..failures.len().min(8)].join("\n")
    );
}

fn run_roofline(manifest: &std::path::Path) -> Output {
    let model = roofline_inputs().join("root/dense");
    command(
        env!("CARGO_BIN_EXE_roofline"),
        None,
        &[
            "--model-path".to_string(),
            model.to_str().unwrap().to_string(),
            "--manifest".to_string(),
            manifest.to_str().unwrap().to_string(),
        ],
    )
    .stdin(Stdio::null())
    .output()
    .expect("run")
}

#[test]
fn the_recursion_boundary_of_the_cli_is_the_originals() {
    let rows = load_jsonl("roofline_verdict/yaml_boundary_cases.jsonl");
    assert!(rows.len() >= 150, "only {} boundary documents", rows.len());
    let dir = scratch("boundary");
    // an exit-0 run does not use the nested field: its stdout is that of the same manifest without it
    let head = dir.join("head.yaml");
    std::fs::write(
        &head,
        "gpu_model: GB10\ngpus_per_node: 1\ntopology: single\nk: &k kk\n",
    )
    .expect("head");
    let baseline = String::from_utf8_lossy(&run_roofline(&head).stdout).to_string();
    assert!(baseline.contains("\"gpu_model\": \"GB10\""), "{baseline}");
    let (mut zero, mut two) = (0, 0);
    let mut failures = Vec::new();
    for row in &rows {
        let name = text_of(row.get("name"));
        let path = dir.join(format!("{name}.yaml"));
        std::fs::write(&path, text_of(row.get("text"))).expect("document");
        let output = run_roofline(&path);
        let exit = int_of(row.get("exit"));
        let stderr =
            String::from_utf8_lossy(&output.stderr).replace(path.to_str().unwrap(), "{MANIFEST}");
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let want_stderr = text_of(row.get("stderr"));
        let good = i64::from(output.status.code().unwrap_or(-1)) == exit
            && stderr == want_stderr
            && (exit != 0 || stdout == baseline);
        if exit == 0 {
            zero += 1;
        } else {
            two += 1;
        }
        if !good {
            failures.push(format!(
                "{name}: exit {:?} (original {exit}); stderr {:?} (original {:?})",
                output.status.code(),
                &stderr[..stderr.len().min(300)],
                &want_stderr[..want_stderr.len().min(300)]
            ));
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        zero > 30 && two > 30,
        "the boundary must have documents on both sides ({zero} read, {two} refused)"
    );
    assert!(
        failures.is_empty(),
        "{} of {} documents differ:\n{}",
        failures.len(),
        rows.len(),
        failures[..failures.len().min(8)].join("\n")
    );
}

#[test]
fn shared_aliases_are_not_copied() {
    let case = load_json_file(&data_path("roofline_verdict/alias_sharing_case.json"));
    let case = dict(&case);
    let dir = scratch("alias");
    let manifest = dir.join("shared.yaml");
    std::fs::write(&manifest, hex_decode(&text_of(case.get("hex")))).expect("manifest");
    let model = roofline_inputs().join("root/dense");
    let output: Output = Command::new("sh")
        .arg("-c")
        // the limits of the root's probe, applied to the child only
        .arg("ulimit -v 1572864 && ulimit -t 3 && ulimit -c 0 && exec \"$@\"")
        .arg("sh")
        .arg(env!("CARGO_BIN_EXE_roofline"))
        .args([
            "--model-path",
            model.to_str().unwrap(),
            "--manifest",
            manifest.to_str().unwrap(),
            "--tp",
            "1",
        ])
        .env_remove("COLUMNS")
        .env_remove("LINES")
        .stdin(Stdio::null())
        .output()
        .expect("run");
    let want_stdout = text_of(case.get("stdout")).replace("{MODEL}", model.to_str().unwrap());
    assert_eq!(output.status.code(), Some(0), "{:?}", output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), want_stdout);
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        text_of(case.get("stderr"))
    );
    let _ = std::fs::remove_dir_all(&dir);
}
