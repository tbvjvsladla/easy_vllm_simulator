//! 임시 git 저장소에서 실제 바이너리를 돌려 판정(0/1/2)을 확인한다.
use std::path::{Path, PathBuf};
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_py_boundary_guard")
}

struct Repo(PathBuf);
impl Drop for Repo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn repo(name: &str) -> Repo {
    let dir = std::env::temp_dir().join(format!("pybg-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(".claude/policies")).unwrap();
    git(&dir, &["init", "-q"]);
    git(&dir, &["config", "user.email", "t@example.invalid"]);
    git(&dir, &["config", "user.name", "t"]);
    Repo(dir)
}

fn git(dir: &Path, args: &[&str]) {
    let s = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .status()
        .unwrap();
    assert!(s.success(), "git {args:?}");
}

fn write(dir: &Path, rel: &str, body: &str) {
    let p = dir.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, body).unwrap();
}

fn run(dir: &Path, extra: &[&str]) -> (i32, String, String) {
    let out = Command::new(bin())
        .arg("check")
        .arg("--repo")
        .arg(dir)
        .args(extra)
        .output()
        .unwrap();
    (
        out.status.code().unwrap(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

const ALLOW: &str = "# c\nconfigs/check_reqs.py\tvLLM deps\n";

#[test]
fn pass_when_only_allowlisted_py_tracked() {
    let r = repo("pass");
    write(&r.0, ".claude/policies/py_boundary_allowlist.tsv", ALLOW);
    write(&r.0, "configs/check_reqs.py", "x=1\n");
    write(&r.0, "src/main.rs", "fn main(){}\n");
    git(&r.0, &["add", "-A"]);
    let (code, out, _) = run(&r.0, &[]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("PASS tracked *.py = 1"));
}

#[test]
fn fail_on_harness_py() {
    let r = repo("fail");
    write(&r.0, ".claude/policies/py_boundary_allowlist.tsv", ALLOW);
    write(&r.0, ".claude/skills/x/scripts/tool.py", "x=1\n");
    git(&r.0, &["add", "-A"]);
    let (code, out, err) = run(&r.0, &[]);
    assert_eq!(code, 1);
    assert!(out.contains("VIOLATION .claude/skills/x/scripts/tool.py"));
    assert!(err.contains("STALE allowlist entry (not tracked here): configs/check_reqs.py"));
}

#[test]
fn staged_additions_only_sees_new_py() {
    let r = repo("staged");
    write(&r.0, ".claude/policies/py_boundary_allowlist.tsv", ALLOW);
    write(&r.0, "old/legacy.py", "x=1\n");
    git(&r.0, &["add", "-A"]);
    git(&r.0, &["commit", "-qm", "base"]);
    // 기존 py 만 있으면 staged 모드는 통과한다(삭제 전 과도기).
    write(&r.0, "README.md", "hi\n");
    git(&r.0, &["add", "README.md"]);
    assert_eq!(run(&r.0, &["--staged-additions"]).0, 0);
    // 새 하네스 py 추가는 거부한다.
    write(&r.0, "new/tool.py", "x=1\n");
    git(&r.0, &["add", "new/tool.py"]);
    let (code, out, _) = run(&r.0, &["--staged-additions"]);
    assert_eq!(code, 1);
    assert!(out.contains("VIOLATION new/tool.py"));
    assert!(!out.contains("old/legacy.py"));
}

#[test]
fn malformed_allowlist_is_fail_closed() {
    for (name, body) in [
        ("noreason", "configs/check_reqs.py\t \n"),
        ("notab", "configs/check_reqs.py\n"),
        ("abs", "/etc/x.py\treason\n"),
        ("dup", "a.py\tr\na.py\tr\n"),
        ("notpy", "a.sh\tr\n"),
    ] {
        let r = repo(name);
        write(&r.0, ".claude/policies/py_boundary_allowlist.tsv", body);
        let (code, _, err) = run(&r.0, &[]);
        assert_eq!(code, 2, "{name}: {err}");
    }
}

#[test]
fn missing_allowlist_is_fail_closed() {
    let r = repo("missing");
    let (code, _, err) = run(&r.0, &[]);
    assert_eq!(code, 2);
    assert!(err.contains("cannot read allowlist"));
}

#[test]
fn usage_errors_exit_2() {
    let out = Command::new(bin()).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    let out = Command::new(bin()).arg("bogus").output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    let out = Command::new(bin()).arg("--help").output().unwrap();
    assert_eq!(out.status.code(), Some(0));
}
