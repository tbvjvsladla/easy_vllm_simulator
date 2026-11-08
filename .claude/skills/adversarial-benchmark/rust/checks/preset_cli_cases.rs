//! The 45 invocations of the ORIGINAL `bench_preset.py` in standard temporary repositories: whole before/after file
//! trees (kind and bytes, the scratch literal normalised to `@ROOT@` after decoding), exit status and stdout exact,
//! stderr exact except that an uncaught Python exception is compared in the text before `Traceback` and in its final
//! `Class: message` line. The original code that sits in the before-tree is data, never executed.

#[path = "preset_common.rs"]
mod preset_common;

use preset_common::*;
use std::collections::BTreeMap;
use sweep_stop::pyvalue::PyValue;

const CLI: &str = env!("CARGO_BIN_EXE_bench_preset");
const TRACEBACK: &str = "Traceback (most recent call last):\n";

fn last_line(text: &str) -> String {
    text.lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .to_string()
}

#[test]
fn frozen_45_original_invocations_match_exit_streams_and_trees() {
    real_repo();
    let pack = load_json(&data_path("cli_cases.json"));
    let base = field(&pack, "base");
    let mut failures = Vec::new();
    let cases = items(field(&pack, "cases"));
    assert_eq!(cases.len(), 45);
    for case in cases {
        let id = text(field(case, "id"));
        let root = scratch(&id);
        stage(&root, base);
        stage(&root, field(case, "before_delta"));
        let before = snapshot(&root, true);
        let mut expected: BTreeMap<String, PyValue> = BTreeMap::new();
        for source in [base, field(case, "before_delta")] {
            if let PyValue::Dict(map) = source.peel() {
                for (rel, entry) in map.iter() {
                    expected.insert(rel.to_utf8().unwrap(), entry.clone());
                }
            }
        }
        let expected_before = expected.clone();
        if let PyValue::Dict(after) = field(case, "after_delta").peel() {
            for (rel, entry) in after.iter() {
                if matches!(entry.peel(), PyValue::None) {
                    expected.remove(&rel.to_utf8().unwrap());
                } else {
                    expected.insert(rel.to_utf8().unwrap(), entry.clone());
                }
            }
        }
        assert_eq!(
            before,
            expected_tree(&expected_before),
            "the packed before-tree rebuilds the original's"
        );
        let argv: Vec<String> = items(field(case, "argv"))
            .iter()
            .map(|a| text(a).replace("@ROOT@", &root.to_string_lossy()))
            .collect();
        let mut cmd = command(CLI, &root);
        cmd.current_dir(&root).args(&argv);
        let out = run(cmd, b"");
        let norm = |bytes: &[u8]| {
            String::from_utf8_lossy(&replace_bytes(
                bytes,
                root.to_string_lossy().as_bytes(),
                b"@ROOT@",
            ))
            .to_string()
        };
        let (stdout, stderr) = (norm(&out.stdout), norm(&out.stderr));
        let want_err = text(field(case, "stderr"));
        let stderr_ok = stderr == want_err
            || (matches!(field(case, "exit_code").peel(), PyValue::Int(n) if n == &1.into())
                && want_err.contains(TRACEBACK)
                && stderr.contains(TRACEBACK)
                && stderr.split(TRACEBACK).next() == want_err.split(TRACEBACK).next()
                && last_line(&stderr) == last_line(&want_err));
        let code_ok = PyValue::int(out.status.code().unwrap_or(-1)) == *field(case, "exit_code");
        let tree_ok = snapshot(&root, true) == expected_tree(&expected);
        if !(code_ok && stdout == text(field(case, "stdout")) && stderr_ok && tree_ok) {
            failures.push(id);
        }
        let _ = std::fs::remove_dir_all(&root);
    }
    assert!(failures.is_empty(), "differ: {failures:?}");
}
