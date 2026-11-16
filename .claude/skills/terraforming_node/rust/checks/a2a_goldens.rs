//! The A2A control-plane parity stages (`ta` agent_control/providers · `tb` render_sub_env · `tc` canary, probe,
//! staleness gate, hint equip · `td` relay) run against THIS crate's executables and compared with the committed goldens of the original
//! Python (`.claude/rust/parity/data/<id>/`). Each case runs in a parity-style sandbox: a copy of the working tree (tracked +
//! untracked, not ignored) with cwd = the copy, the case `setup` files written into it and the declared `outputs` compared as a
//! file delta. No Python is executed — the goldens were captured once from the original programs.
//! Normalizations are the parity tool's N01/N02 (the sandbox root), N03 (timestamps), N04 (`pid=` in stderr) and N06 (a
//! traceback compared by its last line). `A2A_GOLDEN_CASE=<id>[,<id>]` narrows a run.

use agent_runtime_support::pyjson;
use agent_runtime_support::pyvalue::PyValue;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use terraforming_runtime::py;

/// The repository the sandboxes copy: this crate's checkout, or `A2A_REPO` (a crate copy developed outside the tree).
fn repo() -> PathBuf {
    if let Some(r) = std::env::var_os("A2A_REPO") {
        return PathBuf::from(r).canonicalize().unwrap();
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../..").canonicalize().unwrap()
}

fn bin_dir() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_relay")).parent().unwrap().to_path_buf()
}

fn scrub_ts(s: &str) -> String {
    let c: Vec<char> = s.chars().collect();
    let d = |i: usize| c.get(i).map_or(false, |x| x.is_ascii_digit());
    let is = |i: usize, ch: char| c.get(i) == Some(&ch);
    let mut o = String::new();
    let mut i = 0;
    while i < c.len() {
        if d(i) && (0..4).all(|k| d(i + k)) && is(i + 4, '-') && d(i + 5) && d(i + 6) && is(i + 7, '-') && d(i + 8)
            && d(i + 9) && (is(i + 10, 'T') || is(i + 10, ' ')) && d(i + 11) && d(i + 12) && is(i + 13, ':')
            && d(i + 14) && d(i + 15) && is(i + 16, ':') && d(i + 17) && d(i + 18)
        {
            let mut j = i + 19;
            if is(j, '.') && d(j + 1) {
                j += 1;
                while d(j) {
                    j += 1;
                }
            }
            if is(j, 'Z') {
                j += 1;
            } else if is(j, '+') && is(j + 3, ':') && d(j + 1) && d(j + 2) && d(j + 4) && d(j + 5) {
                j += 6;
            }
            o.push_str("<TS>");
            i = j;
            continue;
        }
        if d(i) && (i == 0 || !d(i - 1)) && (0..10).all(|k| d(i + k)) && !d(i + 10) {
            o.push_str("<TS>");
            i += 10;
            continue;
        }
        o.push(c[i]);
        i += 1;
    }
    o
}

fn scrub_pid(s: &str) -> String {
    let mut o = String::new();
    let mut rest = s;
    while let Some(i) = rest.find("pid=") {
        o.push_str(&rest[..i + 4]);
        let after = &rest[i + 4..];
        let n = after.bytes().take_while(|b| b.is_ascii_digit()).count();
        if n > 0 {
            o.push_str("<PID>");
        }
        rest = &after[n..];
    }
    o.push_str(rest);
    o
}

fn reduce_traceback(s: &str) -> String {
    if !s.contains("Traceback (most recent call last):") {
        return s.to_string();
    }
    format!("{}\n", s.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or(""))
}

fn norm(raw: &[u8], root: &str, stderr: bool) -> String {
    let s = scrub_ts(&String::from_utf8_lossy(raw).replace(root, "<R>"));
    if stderr {
        reduce_traceback(&scrub_pid(&s))
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
        Some(PyValue::Dict(d)) => d.iter().map(|(k, x)| (py::text(k), py::display(x))).collect(),
        _ => vec![],
    }
}

/// A parity-style working-tree copy (tracked + untracked, not ignored); ignored `*/target` build dirs are symlinked.
fn sandbox(repo: &Path, id: &str) -> PathBuf {
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let sb = std::env::temp_dir().join(format!("parity-a2a-{id}-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&sb).unwrap();
    let script = "git -C \"$1\" ls-files -z --cached --others --exclude-standard | tar -C \"$1\" --null -T - --ignore-failed-read -cf - 2>/dev/null | tar -x -C \"$2\"";
    Command::new("sh").arg("-c").arg(script).arg("sh").arg(repo).arg(&sb).stderr(Stdio::null()).status().unwrap();
    let o = Command::new("git").arg("-C").arg(repo)
        .args(["ls-files", "-z", "--others", "--ignored", "--exclude-standard", "--directory"])
        .output().unwrap();
    for d in String::from_utf8_lossy(&o.stdout).split('\0') {
        let d = d.trim_end_matches('/');
        if d.ends_with("/target") && !d.starts_with(".claude/runtime/") {
            let dst = sb.join(d);
            std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
            let _ = std::os::unix::fs::symlink(repo.join(d), &dst);
        }
    }
    sb.canonicalize().unwrap()
}

fn run_stage(stage: &str) {
    let repo = repo();
    let parity = repo.join(".claude/rust/parity");
    let cases = pyjson::loads(&std::fs::read_to_string(parity.join(format!("cases/{stage}.json"))).unwrap()).unwrap();
    let PyValue::List(cases) = cases else { panic!("cases") };
    let only: Vec<String> = std::env::var("A2A_GOLDEN_CASE")
        .map(|v| v.split(',').map(String::from).collect())
        .unwrap_or_default();
    let mut bad = vec![];
    for c in &cases {
        let id = py::get_str(c, "id").unwrap();
        if !only.is_empty() && !only.contains(&id) {
            continue;
        }
        let entry = py::get_str(c, "entry").unwrap();
        let orig = py::get_str(c, "orig").unwrap();
        let prog = entry.rsplit('/').next().unwrap().trim_end_matches(".sh").to_string();
        let root = sandbox(&repo, &id);
        for (rel, content) in pairs(py::get(c, "setup")) {
            let p = root.join(&rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, content).unwrap();
        }
        let outputs = strs(py::get(c, "outputs"));
        let before = snapshot(&root, &outputs);
        let mut cmd = Command::new(bin_dir().join(&prog));
        cmd.args(strs(py::get(c, "args")))
            .current_dir(&root)
            .env_remove("EASY_VLLM_SOURCE_ROOT")
            .env("PARITY_GIT_DIR", repo.join(".git"))
            .env("TERRAFORMING_OWNER_PATH", root.join(&orig))
            .env("TERRAFORMING_BIN_DIR", bin_dir())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (k, v) in pairs(py::get(c, "env")) {
            cmd.env(k, v);
        }
        let mut child = cmd.spawn().unwrap_or_else(|e| panic!("{id}: spawn {prog}: {e}"));
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
        let (gs, as_) = (norm(&rd("stdout"), &groot, false), norm(&o.stdout, &rs, false));
        if gs != as_ {
            bad.push(format!("{id}: stdout\n--- golden\n{gs}\n--- actual\n{as_}"));
        }
        let (ge, ae) = (norm(&rd("stderr"), &groot, true), norm(&o.stderr, &rs, true));
        if ge != ae {
            bad.push(format!("{id}: stderr\n--- golden\n{ge}\n--- actual\n{ae}"));
        }
        let gexit: i32 = String::from_utf8_lossy(&rd("exit_code")).trim().parse().unwrap();
        if Some(gexit) != o.status.code() {
            bad.push(format!("{id}: exit golden={gexit} actual={:?}", o.status.code()));
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
        } else {
            for k in &want {
                let gb = std::fs::read(g.join("files").join(k)).unwrap();
                let (x, y) = (norm(&gb, &groot, false), norm(&delta[k], &rs, false));
                if x != y {
                    bad.push(format!("{id}: files/{k}\n--- golden\n{x}\n--- actual\n{y}"));
                }
            }
        }
        let _ = std::fs::remove_dir_all(&root);
    }
    assert!(bad.is_empty(), "golden mismatches ({stage}):\n{}", bad.join("\n"));
}

#[test]
fn ta_agent_control_providers_goldens() {
    run_stage("ta");
}

#[test]
fn td_relay_goldens() {
    run_stage("td");
}

#[test]
fn tb_render_sub_env_goldens() {
    run_stage("tb");
}

#[test]
fn tc_canary_probe_gates_goldens() {
    run_stage("tc");
}
