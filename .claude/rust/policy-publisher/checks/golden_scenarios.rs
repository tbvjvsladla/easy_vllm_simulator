//! Golden comparison: `checks/scenarios.sh` drives the Rust `evidence_publisher` through every scenario and each step's
//! stdout / stderr / exit code / docs tree must equal the capture of the original `evidence_publisher.py`
//! (`checks/data/golden/`, produced by `checks/capture_goldens.sh` [retired with the originals · frozen · plan_26111717] — re-capture needs the original .py). No Python runs
//! here: the program under test is the release binary started as `evidence_publisher.py` (what the launcher does).

use std::path::PathBuf;
use std::process::Command;

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn scratch(tag: &str) -> PathBuf {
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    std::env::temp_dir().join(format!("policy-publisher-{tag}-{}-{n}", std::process::id()))
}

#[test]
fn scenarios_match_python_goldens() {
    let dir = crate_dir();
    let repo = dir.parent().and_then(|p| p.parent()).and_then(|p| p.parent()).unwrap().to_path_buf();
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_evidence_publisher"));
    let out = scratch("out");
    let root = scratch("root");
    // the launcher's contract: argv[0] = <repo>/.claude/policies/runtime/evidence_publisher.sh
    let argv0 = repo.join(".claude/policies/runtime/evidence_publisher.sh");
    let status = Command::new("bash")
        .arg(dir.join("checks/scenarios.sh"))
        .arg(&out)
        .arg(&root)
        .args(["bash", "-c", "exec -a \"$0\" \"$1\" \"${@:2}\""])
        .arg(&argv0)
        .arg(&bin)
        .current_dir(&repo)
        .env("COMPLETION_GATE_BIN", repo.join(".claude/rust/policy-gate/target/release/completion_gate"))
        .status()
        .expect("run scenarios.sh");
    assert!(status.success(), "scenarios.sh failed");
    let golden = dir.join("checks/data/golden");
    let mut names: Vec<String> = std::fs::read_dir(&golden).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    assert!(names.len() > 600, "golden set looks truncated: {}", names.len());
    let mut bad = Vec::new();
    for n in &names {
        let want = std::fs::read(golden.join(n)).unwrap();
        let got = std::fs::read(out.join(n)).unwrap_or_else(|_| b"<missing>".to_vec());
        if want != got {
            bad.push(n.clone());
        }
    }
    let mut extra: Vec<String> = std::fs::read_dir(&out).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| !names.contains(n)).collect();
    extra.sort();
    let _ = std::fs::remove_dir_all(&root);
    if bad.is_empty() && extra.is_empty() {
        let _ = std::fs::remove_dir_all(&out);
    }
    assert!(bad.is_empty() && extra.is_empty(), "{} golden mismatches (kept {}): {:?} extra={:?}", bad.len(), out.display(), &bad[..bad.len().min(40)], extra);
}

/// `--self-test` (the original's built-in roundtrip, ported probe for probe) must pass against the real gate.
#[test]
fn builtin_self_test_passes() {
    let dir = crate_dir();
    let repo = dir.parent().and_then(|p| p.parent()).and_then(|p| p.parent()).unwrap().to_path_buf();
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_evidence_publisher"));
    let argv0 = repo.join(".claude/policies/runtime/evidence_publisher.sh");
    let out = Command::new("bash")
        .args(["-c", "exec -a \"$0\" \"$1\" --self-test"])
        .arg(&argv0)
        .arg(&bin)
        .current_dir(&repo)
        .output()
        .expect("run self-test");
    assert_eq!(out.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout), "[evidence_publisher] self-test PASS\n");
}

/// The launcher refuses to start without its native executable (exit 2 + build guidance, never an interpreter).
#[test]
fn launcher_without_binary_is_exit_2() {
    let dir = crate_dir();
    let repo = dir.parent().and_then(|p| p.parent()).and_then(|p| p.parent()).unwrap().to_path_buf();
    let out = Command::new("bash")
        .arg(repo.join(".claude/policies/runtime/evidence_publisher.sh"))
        .arg("--help")
        .env("EVIDENCE_PUBLISHER_BIN", "/nonexistent/evidence_publisher")
        .output()
        .expect("run launcher");
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("[evidence_publisher] ERROR Rust 실행파일이 없다/실행 불가") && err.contains("build_rust_packages.sh"), "{err}");
}
