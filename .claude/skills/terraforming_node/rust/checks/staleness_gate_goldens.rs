use agent_runtime_support::pyjson;
use agent_runtime_support::pyvalue::PyValue;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use terraforming_runtime::py;

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("checks/data/staleness_gate")
}

fn strs(v: Option<&PyValue>) -> Vec<String> {
    match v {
        Some(PyValue::List(l)) => l.iter().map(py::display).collect(),
        _ => vec![],
    }
}

fn pairs(v: Option<&PyValue>) -> Vec<(String, String)> {
    match v {
        Some(PyValue::Dict(d)) => d.iter().map(|(k, x)| (py::text(k), py::display(x))).collect(),
        _ => vec![],
    }
}

#[test]
fn staleness_gate_command_lines_match_goldens() {
    let data = data_dir();
    let cases = pyjson::loads(&std::fs::read_to_string(data.join("cases.json")).unwrap()).unwrap();
    let PyValue::List(cases) = cases else { panic!("cases.json is not a list") };
    assert!(!cases.is_empty());
    let scratch = std::env::temp_dir().join(format!("tf-staleness-golden-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    let mut bad = vec![];
    for c in &cases {
        let id = py::get_str(c, "id").unwrap();
        let root = scratch.join(&id);
        std::fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        for (rel, content) in pairs(py::get(c, "setup")) {
            let p = root.join(&rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, content).unwrap();
        }
        let o = Command::new(env!("CARGO_BIN_EXE_staleness_gate"))
            .args(strs(py::get(c, "args")))
            .current_dir(&root)
            .env_remove("COLUMNS")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let root_text = root.display().to_string();
        let golden = data.join(&id);
        for (channel, raw) in [("stdout", &o.stdout), ("stderr", &o.stderr)] {
            let want = std::fs::read_to_string(golden.join(channel)).unwrap();
            let got = String::from_utf8_lossy(raw).replace(&root_text, "<R>");
            if want != got {
                bad.push(format!("{id}: {channel}\n--- golden\n{want}\n--- actual\n{got}"));
            }
        }
        let want_exit: i32 = std::fs::read_to_string(golden.join("exit_code")).unwrap().trim().parse().unwrap();
        if Some(want_exit) != o.status.code() {
            bad.push(format!("{id}: exit golden={want_exit} actual={:?}", o.status.code()));
        }
    }
    let _ = std::fs::remove_dir_all(&scratch);
    assert!(bad.is_empty(), "staleness_gate golden mismatches:\n{}", bad.join("\n"));
}

#[test]
fn retired_age_options_are_refused_by_name() {
    for args in [&["--now", "2026-07-25"][..], &["--max-age-days", "30"][..], &["--now=2026-07-25"][..]] {
        let o = Command::new(env!("CARGO_BIN_EXE_staleness_gate"))
            .args(["--topology", "single"])
            .args(args)
            .env_remove("COLUMNS")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&o.stderr);
        assert_eq!(o.status.code(), Some(2), "{args:?}: {stderr}");
        assert!(stderr.ends_with(&format!("staleness_gate.sh: error: unrecognized arguments: {}\n", args.join(" "))), "{args:?}: {stderr}");
        assert!(o.stdout.is_empty(), "{args:?}");
    }
}
