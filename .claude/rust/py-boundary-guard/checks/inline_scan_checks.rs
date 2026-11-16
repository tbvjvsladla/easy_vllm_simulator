//! inline_python_scan: 임시 git 저장소에서 실제 바이너리를 돌려 판정(0/1/2)과 분류를 확인한다.
use std::path::{Path, PathBuf};
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_inline_python_scan")
}

struct Repo(PathBuf);
impl Drop for Repo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn repo(name: &str) -> Repo {
    let dir = std::env::temp_dir().join(format!("ipscan-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q"]);
    Repo(dir)
}

fn git(dir: &Path, args: &[&str]) {
    let s = Command::new("git").arg("-C").arg(dir).args(args).status().unwrap();
    assert!(s.success(), "git {args:?}");
}

fn write(dir: &Path, rel: &str, body: &str) {
    let p = dir.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, body).unwrap();
}

const ALLOW_REL: &str = ".claude/rust/py-boundary-guard/inline_python_allowlist.tsv";

fn run(dir: &Path) -> (i32, String, String) {
    let out = Command::new(bin()).arg("check").arg("--repo").arg(dir).output().unwrap();
    (out.status.code().unwrap(), String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned())
}

#[test]
fn host_inline_python_is_a_violation() {
    let r = repo("host");
    write(&r.0, ALLOW_REL, "# none\n");
    write(&r.0, "a/heredoc.sh", "#!/bin/bash\nX=\"$(python3 - \"$F\" <<'PY'\nprint(1)\nPY\n)\"\n");
    write(&r.0, "a/dash_c.sh", "v=$(python -c 'print(1)')\n");
    write(&r.0, "a/script.sh", "exec python3 \"$SDIR/tool.py\" \"$@\"\n");
    write(&r.0, "a/remote.sh", "ssh \"$H\" \"bash -lc 'python3 $SUB_PY --x'\"\n");
    write(&r.0, "a/versioned.sh", "/usr/bin/python3.12 -m pip freeze\n");
    write(&r.0, "bin/tool", "#!/usr/bin/env python3\nprint(1)\n");
    git(&r.0, &["add", "-A"]);
    let (code, out, _) = run(&r.0);
    assert_eq!(code, 1, "{out}");
    for v in ["a/heredoc.sh:2:", "a/dash_c.sh:1:", "a/script.sh:1:", "a/remote.sh:1:", "a/versioned.sh:1:", "bin/tool:1:"] {
        assert!(out.contains(&format!("VIOLATION {v}")), "{v} missing in {out}");
    }
}

#[test]
fn prose_comments_and_non_commands_pass() {
    let r = repo("prose");
    write(&r.0, ALLOW_REL, "# none\n");
    write(
        &r.0,
        "a/ok.sh",
        "# python3 - <<PY was here\nx=1   # was: python3 -c 'x'\necho \"python3 대체\"\nPYTHON_BIN=python3x\nNAME=pythonic\n",
    );
    write(&r.0, "docs/a.md", "python3 -c 'print(1)'\n");
    write(&r.0, "src/x.rs", "let a = \"python3 -c\";\n");
    write(&r.0, "a/tool.py", "import os\n");
    write(&r.0, ".claude/rust/parity/data/case/stdout", "python3 - <<PY\n");
    git(&r.0, &["add", "-A"]);
    let (code, out, err) = run(&r.0);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("PASS scanned 1 files"), "{out}");
}

#[test]
fn container_lines_pass_only_by_allowlisted_literal() {
    let r = repo("container");
    write(
        &r.0,
        ALLOW_REL,
        "a/c.sh\tdocker exec \"$C\" python3 -c\tcontainer\tinside the serving container\n",
    );
    write(&r.0, "a/c.sh", "v=$(docker exec \"$C\" python3 -c 'import torch')\n");
    write(&r.0, "Dockerfile.template", "RUN python3 - <<'PY'\nprint(1)\nPY\n");
    git(&r.0, &["add", "-A"]);
    let (code, out, _) = run(&r.0);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("1 Dockerfile lines (in-image) · 1 lines within allowlist"), "{out}");
    // a second, host-side line in the same file is not covered by the container entry
    write(&r.0, "a/c.sh", "v=$(docker exec \"$C\" python3 -c 'import torch')\npython3 -c 'print(2)'\n");
    git(&r.0, &["add", "-A"]);
    let (code, out, _) = run(&r.0);
    assert_eq!(code, 1);
    assert!(out.contains("VIOLATION a/c.sh:2:"), "{out}");
}

#[test]
fn stale_entries_warn_and_bad_allowlists_fail_closed() {
    let r = repo("stale");
    write(&r.0, ALLOW_REL, "gone.sh\tpython3 -c\tcontainer\tremoved\n");
    git(&r.0, &["add", "-A"]);
    let (code, _, err) = run(&r.0);
    assert_eq!(code, 0);
    assert!(err.contains("STALE allowlist entry"), "{err}");
    for (name, body) in [
        ("fields", "a.sh\tpython3\tcontainer\n"),
        ("class", "a.sh\tpython3\thost\twhy\n"),
        ("reason", "a.sh\tpython3\tcontainer\t \n"),
        ("abs", "/a.sh\tpython3\tcontainer\twhy\n"),
        ("dup", "a.sh\tpython3\tcontainer\twhy\na.sh\tpython3\tcontainer\twhy\n"),
    ] {
        let r = repo(name);
        write(&r.0, ALLOW_REL, body);
        assert_eq!(run(&r.0).0, 2, "{name}");
    }
    let r = repo("missing");
    assert_eq!(run(&r.0).0, 2);
}

#[test]
fn usage_errors_exit_2() {
    assert_eq!(Command::new(bin()).output().unwrap().status.code(), Some(2));
    assert_eq!(Command::new(bin()).arg("bogus").output().unwrap().status.code(), Some(2));
    assert_eq!(Command::new(bin()).arg("--help").output().unwrap().status.code(), Some(0));
}

/// The real tree: every host/sub inline Python is ported; only container / golden-capture lines remain (allow-listed).
#[test]
fn this_repository_passes() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let out = Command::new(bin()).arg("check").arg("--repo").arg(&repo).output().unwrap();
    assert_eq!(out.status.code(), Some(0), "{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
}
