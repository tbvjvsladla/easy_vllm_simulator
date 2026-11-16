//! Every `S5-*` parity case of `.claude/rust/parity/cases/s5.json` run against THIS crate's executables and compared with the
//! committed goldens of the original Python (`.claude/rust/parity/data/<id>/`): stdout, stderr, exit code and the files the
//! case declares as outputs. No Python is executed here — the goldens were captured once from the original programs.
//! Normalizations are the parity tool's N01/N02 (the per-case root path) and N06 (a traceback compared by its last line).

use agent_runtime_support::pyjson;
use agent_runtime_support::pyvalue::PyValue;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use terraforming_runtime::py;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../..")
        .canonicalize()
        .unwrap()
}

fn exe(prog: &str) -> &'static str {
    match prog {
        "audit_session_ssh" => env!("CARGO_BIN_EXE_audit_session_ssh"),
        "agent_card_contract" => env!("CARGO_BIN_EXE_agent_card_contract"),
        "library_access" => env!("CARGO_BIN_EXE_library_access"),
        "logs_lifecycle" => env!("CARGO_BIN_EXE_logs_lifecycle"),
        "adversarial_stress" => env!("CARGO_BIN_EXE_adversarial_stress"),
        "seed_from_journal" => env!("CARGO_BIN_EXE_seed_from_journal"),
        "blackbox_eta" => env!("CARGO_BIN_EXE_blackbox_eta"),
        "blackbox_session" => env!("CARGO_BIN_EXE_blackbox_session"),
        "blackbox_collect" => env!("CARGO_BIN_EXE_blackbox_collect"),
        "blackbox_thermal" => env!("CARGO_BIN_EXE_blackbox_thermal"),
        "blackbox_events" => env!("CARGO_BIN_EXE_blackbox_events"),
        "regen_envelope" => env!("CARGO_BIN_EXE_regen_envelope"),
        "agent_guard" => env!("CARGO_BIN_EXE_agent_guard"),
        "publish_install_request" => env!("CARGO_BIN_EXE_publish_install_request"),
        other => panic!("no executable for {other}"),
    }
}

fn reduce_traceback(s: &str) -> String {
    if !s.contains("Traceback (most recent call last):") {
        return s.to_string();
    }
    format!(
        "{}\n",
        s.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("")
    )
}

fn norm(raw: &[u8], root: &str, stderr: bool) -> String {
    let s = String::from_utf8_lossy(raw).replace(root, "<R>");
    if stderr {
        reduce_traceback(&s)
    } else {
        s
    }
}

fn collect(dir: &Path, base: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                collect(&p, base, out);
            } else if let Ok(b) = std::fs::read(&p) {
                out.insert(p.strip_prefix(base).unwrap().display().to_string(), b);
            }
        }
    }
}

fn snapshot(base: &Path, outputs: &[String]) -> BTreeMap<String, Vec<u8>> {
    let mut m = BTreeMap::new();
    for rel in outputs {
        let p = base.join(rel);
        if p.is_dir() {
            collect(&p, base, &mut m);
        } else if let Ok(b) = std::fs::read(&p) {
            m.insert(rel.clone(), b);
        }
    }
    m
}

fn strs(v: Option<&PyValue>) -> Vec<String> {
    match v {
        Some(PyValue::List(l)) => l.iter().map(py::display).collect(),
        _ => vec![],
    }
}
fn pairs(v: Option<&PyValue>) -> Vec<(String, String)> {
    match v {
        Some(PyValue::Dict(d)) => d
            .iter()
            .map(|(k, x)| (py::text(k), py::display(x)))
            .collect(),
        _ => vec![],
    }
}

#[test]
fn s5_goldens() {
    let repo = repo();
    let parity = repo.join(".claude/rust/parity");
    let cases =
        pyjson::loads(&std::fs::read_to_string(parity.join("cases/s5.json")).unwrap()).unwrap();
    let PyValue::List(cases) = cases else {
        panic!("cases")
    };
    let mut bad = vec![];
    let scratch = std::env::temp_dir().join(format!("tf-golden-{}", std::process::id()));
    for c in &cases {
        let id = py::get_str(c, "id").unwrap();
        let entry = py::get_str(c, "entry").unwrap();
        let prog = entry
            .rsplit('/')
            .next()
            .unwrap()
            .trim_end_matches(".sh")
            .to_string();
        let sb = scratch.join(&id);
        std::fs::create_dir_all(&sb).unwrap();
        let root = sb.canonicalize().unwrap();
        for (rel, content) in pairs(py::get(c, "setup")) {
            let p = root.join(&rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, content).unwrap();
        }
        let outputs = strs(py::get(c, "outputs"));
        let before = snapshot(&root, &outputs);
        let owner = root.join(entry.trim_end_matches(".sh").to_string() + ".py");
        let mut cmd = Command::new(exe(&prog));
        cmd.args(strs(py::get(c, "args")))
            .current_dir(&root)
            .env("TERRAFORMING_OWNER_PATH", &owner)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (k, v) in pairs(py::get(c, "env")) {
            cmd.env(k, v);
        }
        let mut child = cmd.spawn().unwrap();
        let input = py::get(c, "stdin").map(py::display).unwrap_or_default();
        {
            use std::io::Write;
            let mut si = child.stdin.take().unwrap();
            let _ = si.write_all(input.as_bytes());
        }
        let o = child.wait_with_output().unwrap();
        let after = snapshot(&root, &outputs);
        let g = parity.join("data").join(&id);
        let meta = pyjson::loads(&std::fs::read_to_string(g.join("meta.json")).unwrap()).unwrap();
        let groot = py::get_str(&meta, "tmp_root").unwrap();
        let rd = |n: &str| std::fs::read(g.join(n)).unwrap();
        let rs = root.display().to_string();
        if norm(&rd("stdout"), &groot, false) != norm(&o.stdout, &rs, false) {
            bad.push(format!("{id}: stdout"));
        }
        if norm(&rd("stderr"), &groot, true) != norm(&o.stderr, &rs, true) {
            bad.push(format!("{id}: stderr"));
        }
        let gexit: i32 = String::from_utf8_lossy(&rd("exit_code"))
            .trim()
            .parse()
            .unwrap();
        if Some(gexit) != o.status.code() {
            bad.push(format!(
                "{id}: exit golden={gexit} actual={:?}",
                o.status.code()
            ));
        }
        let mut delta = BTreeMap::new();
        for (k, v) in &after {
            if before.get(k) != Some(v) {
                delta.insert(k.clone(), v.clone());
            }
        }
        for k in before.keys() {
            if !after.contains_key(k) {
                delta.insert(format!("{k}.parity-deleted"), vec![]);
            }
        }
        let idx = pyjson::loads(&String::from_utf8_lossy(&rd("files.index.json"))).unwrap();
        let want: Vec<String> = strs(Some(&idx));
        let have: Vec<String> = delta.keys().cloned().collect();
        if want != have {
            bad.push(format!("{id}: files {want:?} != {have:?}"));
            continue;
        }
        for k in &want {
            let gb = std::fs::read(g.join("files").join(k)).unwrap();
            if norm(&gb, &groot, false) != norm(&delta[k], &rs, false) {
                bad.push(format!("{id}: files/{k}"));
            }
        }
    }
    let _ = std::fs::remove_dir_all(&scratch);
    assert!(bad.is_empty(), "golden mismatches:\n{}", bad.join("\n"));
}
