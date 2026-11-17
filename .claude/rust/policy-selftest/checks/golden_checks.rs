//! CLI goldens: every case of `checks/fixtures.sh` is built as a fixture repository and the native program runs in it
//! with argv[0] = `<fixture>/.claude/policies/runtime/<name>.py` (the launcher's repository-root rule). stdout, stderr,
//! exit code (and the inventory file) must equal what the ORIGINAL Python wrote on the same fixture
//! (`checks/data/cli/<case>/`, captured by `checks/fixtures.sh capture`). No Python is started here.
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

fn checks() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/checks"))
}

fn bin(name: &str) -> &'static str {
    match name {
        "runtime_selftest" => env!("CARGO_BIN_EXE_runtime_selftest"),
        "antipattern_scan" => env!("CARGO_BIN_EXE_antipattern_scan"),
        "constitution_lint" => env!("CARGO_BIN_EXE_constitution_lint"),
        other => panic!("unknown program {other}"),
    }
}

fn fixtures(args: &[&str]) -> String {
    let out = Command::new("bash").arg(checks().join("fixtures.sh")).args(args).output().expect("bash");
    assert!(out.status.success(), "fixtures.sh {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

fn scratch(case: &str) -> PathBuf {
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    std::env::temp_dir().join(format!("policy-selftest-golden-{case}-{}-{n}", std::process::id()))
}

fn run_case(case: &str) -> Result<(), String> {
    let base = scratch(case);
    std::fs::create_dir_all(&base).unwrap();
    let root = base.join("r");
    fixtures(&["build", case, root.to_str().unwrap()]);
    let argv = fixtures(&["argv", case]);
    let mut words = argv.split_whitespace();
    let prog = words.next().unwrap();
    let rest: Vec<&str> = words.collect();
    let real = std::fs::canonicalize(&root).unwrap();
    let out = Command::new(bin(prog))
        .arg0(real.join(".claude/policies/runtime").join(format!("{prog}.sh")))
        .args(&rest)
        .current_dir(&real)
        .env_remove("GIT_DIR")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_WORK_TREE")
        .output()
        .unwrap();
    let norm = |b: &[u8]| String::from_utf8_lossy(b).replace(real.to_str().unwrap(), "<ROOT>").replace(root.to_str().unwrap(), "<ROOT>");
    let g = checks().join("data/cli").join(case);
    let want = |n: &str| std::fs::read_to_string(g.join(n)).unwrap_or_else(|e| panic!("{case}/{n}: {e}"));
    let mut bad = Vec::new();
    if norm(&out.stdout) != want("stdout") {
        bad.push(format!("stdout:\n--- got\n{}\n--- want\n{}", norm(&out.stdout), want("stdout")));
    }
    if norm(&out.stderr) != want("stderr") {
        bad.push(format!("stderr:\n--- got\n{}\n--- want\n{}", norm(&out.stderr), want("stderr")));
    }
    let code = out.status.code().unwrap_or(-1).to_string();
    if code != want("exit_code").trim() {
        bad.push(format!("exit: got {code} want {}", want("exit_code").trim()));
    }
    if g.join("inv.jsonl").is_file() {
        let got = std::fs::read_to_string(real.join("inv.jsonl")).unwrap_or_default();
        if got != want("inv.jsonl") {
            bad.push(format!("inv.jsonl:\n--- got\n{got}\n--- want\n{}", want("inv.jsonl")));
        }
    }
    let _ = std::fs::remove_dir_all(&base);
    if bad.is_empty() {
        Ok(())
    } else {
        Err(format!("[{case}] {}", bad.join("\n")))
    }
}

#[test]
fn cli_goldens() {
    let cases = fixtures(&["list"]);
    let cases: Vec<&str> = cases.lines().filter(|l| !l.is_empty()).collect();
    assert!(cases.len() >= 25, "case list shrank: {}", cases.len());
    let mut fails = Vec::new();
    for c in &cases {
        assert!(Path::new(&checks().join("data/cli").join(c)).is_dir(), "golden missing for {c}");
        if let Err(e) = run_case(c) {
            fails.push(e);
        }
    }
    assert!(fails.is_empty(), "{} of {} cases differ:\n{}", fails.len(), cases.len(), fails.join("\n\n"));
}
