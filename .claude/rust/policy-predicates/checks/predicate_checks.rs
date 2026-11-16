//! Goldens of the original Python predicates (captured by `checks/capture_goldens.sh`; nothing here runs Python):
//!   * the real tree: the original's verdict is PASS for all 75 clauses -> every ported clause passes here too, the
//!     JSON payload of the ported set is byte-identical to the original's PASS payload, the mapping equals the real
//!     registry's clause ids, and every clause that is not ported is reported as a failure (never a silent pass);
//!   * negative controls: each mutation of `checks/data/mutations.json`, applied to a copy of the tracked tree, makes
//!     the targeted clause fail with exactly the original's `<Class>: <message>` (or, for the unittest suites, fails
//!     the targeted test case with exit code 1).
//! One sequential test: the predicates read the repository root from `POLICY_PREDICATES_REPO`.
use agent_runtime_support::pyjson;
use agent_runtime_support::pyvalue::PyValue;
use policy_predicates::claims::{self, Impl};
use std::path::{Path, PathBuf};
use std::process::Command;

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}
fn repo() -> PathBuf {
    crate_dir().join("../../..").canonicalize().unwrap()
}
fn load(name: &str) -> PyValue {
    let text = std::fs::read_to_string(crate_dir().join("checks/data").join(name)).unwrap();
    pyjson::loads(&text).unwrap()
}
fn get<'a>(v: &'a PyValue, k: &str) -> &'a PyValue {
    match v.peel() {
        PyValue::Dict(d) => d.get(k).unwrap_or_else(|| panic!("key {k}")).peel(),
        _ => panic!("not a dict"),
    }
}
fn s(v: &PyValue) -> String {
    match v.peel() {
        PyValue::Str(x) => x.to_utf8().unwrap(),
        other => panic!("not a str: {other:?}"),
    }
}

/// A copy of the tracked tree (working-tree bytes) with the ignored Rust build outputs linked in, so the launchers
/// the ported predicates call find their binaries.
fn tracked_copy(dst: &Path) {
    let out = Command::new("git").arg("-C").arg(repo()).args(["ls-files", "-z"]).output().unwrap();
    for rel in String::from_utf8(out.stdout).unwrap().split('\0').filter(|r| !r.is_empty()) {
        let src = repo().join(rel);
        let meta = match std::fs::symlink_metadata(&src) { Ok(m) => m, Err(_) => continue };
        if !meta.is_file() {
            continue;
        }
        let d = dst.join(rel);
        std::fs::create_dir_all(d.parent().unwrap()).unwrap();
        std::fs::copy(&src, &d).unwrap();
    }
    for target in [".claude/skills/upstream-version-watch/rust/runtime/target", ".claude/skills/adversarial-benchmark/rust/target",
                   ".claude/skills/vllm-recipe-explorer/rust/runtime/target"] {
        let src = repo().join(target);
        if src.is_dir() {
            std::fs::create_dir_all(dst.join(target).parent().unwrap()).unwrap();
            std::os::unix::fs::symlink(&src, dst.join(target)).unwrap();
        }
    }
}

fn scratch(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("pp-check-{tag}-{}-{}", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn run_clause(clause: &str) -> Option<String> {
    let e = claims::predicates().into_iter().find(|e| e.clause_id == clause).unwrap();
    match e.imp {
        Impl::Ported(f) => f().err().map(|x| format!("{}: {}", x.kind, x.message)),
        Impl::NotPorted(_) => panic!("{clause} is not ported"),
    }
}

#[test]
fn goldens() {
    std::env::set_var("POLICY_PREDICATES_REPO", repo());

    // ---- real tree
    let orig = load("orig_verdict.json");
    let orig_stdout = s(get(&orig, "stdout"));
    assert!(orig_stdout.contains("\"verdict\": \"PASS\""), "the golden is the original's PASS verdict");
    let entries = claims::predicates();
    assert_eq!(entries.len(), claims::EXPECTED_PREDICATES);
    let ids: std::collections::BTreeSet<String> = entries.iter().map(|e| e.clause_id.to_string()).collect();
    assert_eq!(ids, claims::registry_clause_ids().unwrap(), "mapping must equal the real registry clause ids");
    let ported: Vec<String> = entries.iter().filter(|e| matches!(e.imp, Impl::Ported(_))).map(|e| e.clause_id.to_string()).collect();
    let (payload, code) = claims::run_all(&ported);
    assert_eq!(code, 0, "every ported clause passes on the real tree, as in the original: {payload}");
    assert_eq!(format!("{payload}\n"), orig_stdout, "the PASS payload is byte-identical to the original's");
    let (full, full_code) = claims::run_all(&[]);
    let not_ported = entries.len() - ported.len();
    if not_ported == 0 {
        // Every clause is native: the full run is the original's full PASS verdict, byte for byte.
        assert_eq!(full_code, 0, "{full}");
        assert_eq!(format!("{full}\n"), orig_stdout, "the full PASS payload is byte-identical to the original's");
    } else {
        assert_eq!(full_code, 1, "not-ported clauses must fail closed");
        assert!(full.contains(&format!("\"failed_count\": {not_ported},")), "{full}");
    }
    assert_eq!(full.matches("NotPorted: ").count(), not_ported);
    for e in &entries {
        if matches!(e.imp, Impl::Ported(_)) {
            assert!(claims::direct_requires(e.name).unwrap_or(0) >= 1, "{} has no direct req", e.name);
        }
    }
    let units = load("orig_unit.json");
    for (module, cases) in [("companion_predicate", policy_predicates::companion::cases()), ("runtime_patch_predicate", policy_predicates::runtime_patch::cases())] {
        let want = match get(&units, module) { PyValue::Int(i) => i.to_string(), _ => panic!() };
        let bad: Vec<&str> = cases.iter().filter(|c| (c.f)().is_err()).map(|c| c.name).collect();
        assert_eq!(want, if bad.is_empty() { "0" } else { "1" }, "{module}: {bad:?}");
    }

    // ---- negative controls
    let muts = load("mutations.json");
    let goldens = load("negative_goldens.json");
    let list = match muts.peel() { PyValue::List(l) => l.clone(), _ => panic!() };
    for m in list {
        let id = s(get(&m, "id"));
        let g = get(&goldens, &id);
        let root = scratch(&id);
        tracked_copy(&root);
        let file = root.join(s(get(&m, "file")));
        let text = std::fs::read_to_string(&file).unwrap();
        let old = s(get(&m, "old"));
        assert!(text.contains(&old), "{id}: mutation anchor missing (re-capture the goldens)");
        std::fs::write(&file, text.replacen(&old, &s(get(&m, "new")), 1)).unwrap();
        std::env::set_var("POLICY_PREDICATES_REPO", &root);
        let has = |k: &str| matches!(m.peel(), PyValue::Dict(d) if d.contains_key(k));
        if has("clause") {
            let got = run_clause(&s(get(&m, "clause")));
            assert_eq!(got.as_deref(), Some(s(get(g, "error")).as_str()), "{id}");
        } else {
            let unit = s(get(&m, "unit"));
            let test = s(get(&m, "test"));
            let cases = if unit == "companion_predicate" { policy_predicates::companion::cases() } else { policy_predicates::runtime_patch::cases() };
            let failed: Vec<String> = cases.iter().filter(|c| (c.f)().is_err()).map(|c| c.name.to_string()).collect();
            let golden_failed: Vec<String> = match get(g, "failed") { PyValue::List(l) => l.iter().map(s).collect(), _ => panic!() };
            assert!(golden_failed.contains(&test), "{id}: golden must fail the targeted test");
            assert!(failed.contains(&test), "{id}: the targeted test must fail: {failed:?}");
            assert_eq!(match get(g, "exit_code") { PyValue::Int(i) => i.to_string(), _ => panic!() }, "1");
        }
        std::env::set_var("POLICY_PREDICATES_REPO", repo());
        let _ = std::fs::remove_dir_all(&root);
    }
}
