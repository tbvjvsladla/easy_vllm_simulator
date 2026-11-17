//! Golden checks: the Rust `completion_gate` (through its launcher, as harness callers run it) must reproduce, byte
//! for byte, the stdout / stderr / exit code the ORIGINAL Python program produced on the same fixture
//! (`checks/data/cg/golden/*`, captured by `checks/capture_goldens.sh` [retired with the originals · frozen · plan_26111717]). No Python is executed here.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo() -> PathBuf {
    fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")).unwrap()
}

fn copy_tree(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for e in fs::read_dir(src).unwrap() {
        let e = e.unwrap();
        let p = e.path();
        let d = dst.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_tree(&p, &d);
        } else {
            fs::copy(&p, &d).unwrap();
        }
    }
}

fn scratch(tag: &str) -> PathBuf {
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let p = std::env::temp_dir().join(format!("policy-gate-{tag}-{}-{n}", std::process::id()));
    fs::create_dir_all(&p).unwrap();
    fs::canonicalize(&p).unwrap()
}

fn normalize(text: &str, tmp: &Path, repo: &Path) -> String {
    text.replace(&tmp.to_string_lossy().into_owned(), "<TMP>").replace(&repo.to_string_lossy().into_owned(), "<REPO>")
}

/// A stand-in repository holding the real schemas and a `runtime_selftest.sh` beside the program's script path: the
/// golden cases run with `argv[0]` = `<fake>/.claude/policies/runtime/completion_gate.py` (what the launcher does
/// for the real repository), so the authorize tripwire pre-check sees its executor present, as in the Python capture.
fn fake_repo() -> PathBuf {
    static ONCE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    ONCE.get_or_init(|| {
        let f = scratch("fake-repo");
        copy_tree(&repo().join(".claude/schemas"), &f.join(".claude/schemas"));
        let rt = f.join(".claude/policies/runtime");
        fs::create_dir_all(&rt).unwrap();
        fs::write(rt.join("runtime_selftest.sh"), "exit 0\n").unwrap();
        f
    })
    .clone()
}

fn run_case(id: &str, args: &str) -> Option<String> {
    let repo = fake_repo();
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("checks/data/cg");
    let tmp = scratch(id);
    copy_tree(&data.join("fx"), &tmp.join("fx"));
    for line in fs::read_to_string(data.join("symlinks.tsv")).unwrap().lines() {
        if let Some((link, target)) = line.split_once('\t') {
            std::os::unix::fs::symlink(target, tmp.join(link)).unwrap();
        }
    }
    let argv: Vec<&str> = args.split_whitespace().collect();
    let out = Command::new("bash")
        .arg("-c")
        .arg("exec -a \"$0\" \"$GATE_BIN\" \"$@\"")
        .arg(repo.join(".claude/policies/runtime/completion_gate.sh"))
        .args(&argv)
        .current_dir(&tmp)
        .env("GATE_BIN", env!("CARGO_BIN_EXE_completion_gate"))
        .output()
        .unwrap();
    let got_out = normalize(&String::from_utf8_lossy(&out.stdout), &tmp, &repo);
    let got_err = normalize(&String::from_utf8_lossy(&out.stderr), &tmp, &repo);
    let got_rc = out.status.code().unwrap_or(-1);
    let _ = fs::remove_dir_all(&tmp);
    let g = data.join("golden");
    let want_out = fs::read_to_string(g.join(format!("{id}.out"))).unwrap();
    let want_err = fs::read_to_string(g.join(format!("{id}.err"))).unwrap();
    let want_rc: i32 = fs::read_to_string(g.join(format!("{id}.rc"))).unwrap().trim().parse().unwrap();
    let mut bad = Vec::new();
    if got_rc != want_rc {
        bad.push(format!("rc {got_rc} != {want_rc}"));
    }
    if got_out != want_out {
        bad.push(format!("stdout differs:\n--- want\n{want_out}\n--- got\n{got_out}"));
    }
    if got_err != want_err {
        bad.push(format!("stderr differs:\n--- want\n{want_err}\n--- got\n{got_err}"));
    }
    if bad.is_empty() {
        None
    } else {
        Some(format!("[{id}] {}", bad.join("\n")))
    }
}

#[test]
fn completion_gate_matches_python_goldens() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("checks/data/cg");
    let cases: Vec<(String, String)> = fs::read_to_string(data.join("cases.tsv"))
        .unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| {
            let (a, b) = l.split_once('\t').unwrap_or((l, ""));
            (a.to_string(), b.to_string())
        })
        .collect();
    assert!(cases.len() >= 100, "case table shrank: {}", cases.len());
    let handles: Vec<_> = cases
        .chunks(cases.len().div_ceil(8))
        .map(|chunk| {
            let chunk = chunk.to_vec();
            std::thread::spawn(move || chunk.iter().filter_map(|(id, args)| run_case(id, args)).collect::<Vec<_>>())
        })
        .collect();
    let failures: Vec<String> = handles.into_iter().flat_map(|h| h.join().unwrap()).collect();
    assert!(failures.is_empty(), "{} of {} cases differ:\n{}", failures.len(), cases.len(), failures.join("\n\n"));
}

#[test]
fn doc_naming_matches_python_goldens() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("checks/data/dn");
    let launcher = repo().join(".claude/policies/runtime/doc_naming.sh");
    let mut bad = Vec::new();
    for line in fs::read_to_string(data.join("cases.tsv")).unwrap().lines().filter(|l| !l.is_empty()) {
        let (id, args) = line.split_once('\t').unwrap_or((line, ""));
        let out = Command::new("bash")
            .arg(&launcher)
            .args(args.split_whitespace())
            .env("DOC_NAMING_BIN", env!("CARGO_BIN_EXE_doc_naming"))
            .output()
            .unwrap();
        let read = |ext: &str| fs::read_to_string(data.join(format!("{id}.{ext}"))).unwrap();
        let rc: i32 = read("rc").trim().parse().unwrap();
        if out.status.code() != Some(rc) || String::from_utf8_lossy(&out.stdout) != read("out") || String::from_utf8_lossy(&out.stderr) != read("err") {
            bad.push(format!("[{id}] rc={:?} out={:?} err={:?}", out.status.code(), String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn launcher_runs_the_native_program() {
    let launcher = repo().join(".claude/policies/runtime/completion_gate.sh");
    let out = Command::new("bash")
        .arg(&launcher)
        .arg("--help")
        .env("COMPLETION_GATE_BIN", env!("CARGO_BIN_EXE_completion_gate"))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let want = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("checks/data/cg/golden/cli-help.out")).unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), want);
    let missing = Command::new("bash").arg(&launcher).env("COMPLETION_GATE_BIN", "/nonexistent/completion_gate").output().unwrap();
    assert_eq!(missing.status.code(), Some(2), "absent native program = exit 2, never a Python start");
    assert!(String::from_utf8_lossy(&missing.stderr).contains("build_rust_packages.sh"));
}

#[test]
fn flat_certificate_parser_contract() {
    use policy_gate::completion_gate::*;
    let base = "schema_version: 1\nrecord_type: benchmark_certificate\nverdict: PASS\nmodel: m\ngpu_model: \"NVIDIA GB10\"\nvllm_version: 0.18.0\nquantization: mxfp4\ntopology: single\ntensor_parallel_size: 1\n";
    let (f, ok) = parse_flat_certificate(&format!("{base}measured_utc: \"2026-09-03T12:00:00Z\"\n"));
    assert!(ok);
    assert_eq!(f.get("gpu_model"), Some("NVIDIA GB10"));
    assert_eq!(certificate_run_key(&f).unwrap().last().unwrap(), "2026-09-03T12:00:00Z");
    let (f2, ok2) = parse_flat_certificate(base);
    assert!(ok2 && certificate_run_key(&f2).is_none());
    assert!(!parse_flat_certificate("a:\n  nested: 1\n").1);
    assert!(!parse_flat_certificate("a: 1\na: 2\n").1);
    assert!(!parse_flat_certificate("a: 1 \n").1, "trailing whitespace is not flat");
    assert_eq!(CERTIFICATE_RUN_KEY_FIELDS[6], "measured_utc");
    assert!(!CERTIFICATE_RUN_KEY_FIELDS.contains(&"image_digest"));
    assert_eq!(certificate_grade(&parse_flat_certificate("benchmark_mode: lite\n").0), Some("lite"));
    assert_eq!(certificate_grade(&parse_flat_certificate("x: 1\n").0), Some("full"));
    assert_eq!(certificate_grade(&parse_flat_certificate("benchmark_mode: odd\n").0), None);
}
