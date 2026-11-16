//! Scenario checks of the four hint-publisher tests of the full suite (`policy_selftest::hint_suite`).
//!
//! GREEN: each test passes on this checkout (the deployed hint launcher + native binary, the owner libraries).
//! RED (fail closed): the same tests against a shadow checkout whose subject is broken — a hint executable that does
//! nothing (exit 0, no output) for the three CLI E2E tests, a completion-gate contract without its public API for the
//! binding-source judge — must return an error, never Ok. The docker-call predicate is checked on its own vacuity cases.
use policy_selftest::hint_suite as hs;
use policy_selftest::tripwires::Fail;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

// The tests mutate the process environment (git isolation, HINT_SKILL_DIR): run them one at a time.
static SERIAL: Mutex<()> = Mutex::new(());

fn repo() -> PathBuf {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    std::fs::canonicalize(crate_dir.join("../../..")).unwrap()
}

fn scratch(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let p = std::env::temp_dir().join(format!("hs-check-{tag}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// A checkout that is the real one through symlinks, except `rel`, which is a regular file with `content`.
fn shadow(real: &Path, fake: &Path, rel: &str, content: &str, exec: bool) {
    let parts: Vec<&str> = rel.split('/').collect();
    let (mut r, mut f) = (real.to_path_buf(), fake.to_path_buf());
    for (i, part) in parts.iter().enumerate() {
        std::fs::create_dir_all(&f).unwrap();
        for e in std::fs::read_dir(&r).unwrap().flatten() {
            if e.file_name().to_string_lossy() != *part {
                std::os::unix::fs::symlink(e.path(), f.join(e.file_name())).unwrap();
            }
        }
        r = r.join(part);
        f = f.join(part);
        if i + 1 == parts.len() {
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(&f, content).unwrap();
            if exec {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o755)).unwrap();
            }
        }
    }
}

fn is_failure(r: Result<(), Fail>) -> bool {
    r.is_err()
}

#[test]
fn green_on_this_checkout() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let root = repo();
    let mut bad = Vec::new();
    for (name, r) in [
        ("_test_hint_binding_source", hs::hint_binding_source(&root)),
        ("_test_hint_cli_publication", hs::hint_cli_publication(&root)),
        ("_test_hint_cli_refute_paths", hs::hint_cli_refute_paths(&root)),
        ("_test_hint_map_only_publication", hs::hint_map_only_publication(&root)),
    ] {
        if let Err(Fail::Selftest(m) | Fail::Crash(m)) = r {
            bad.push(format!("{name}: {m}"));
        }
    }
    assert!(bad.is_empty(), "failed on the real checkout:\n{}", bad.join("\n"));
}

#[test]
fn red_inert_hint_executable() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let root = repo();
    let fake = scratch("inert-hint");
    shadow(&root, &fake, ".claude/skills/hint-publisher/rust/target/release/hint", "#!/bin/bash\nexit 0\n", true);
    for (name, r) in [
        ("_test_hint_cli_publication", hs::hint_cli_publication(&fake)),
        ("_test_hint_cli_refute_paths", hs::hint_cli_refute_paths(&fake)),
        ("_test_hint_map_only_publication", hs::hint_map_only_publication(&fake)),
    ] {
        assert!(is_failure(r), "{name} passed against a hint executable that does nothing");
    }
    let _ = std::fs::remove_dir_all(&fake);
}

#[test]
fn red_missing_hint_executable() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let root = repo();
    let fake = scratch("absent-hint");
    // the release dir without the binary
    let rel = ".claude/skills/hint-publisher/rust/target/release/hint";
    shadow(&root, &fake, rel, "", false);
    std::fs::remove_file(fake.join(rel)).unwrap();
    match hs::hint_cli_publication(&fake) {
        Err(Fail::Selftest(m)) => assert!(m.contains("hint native executable is absent"), "{m}"),
        other => panic!("an absent hint executable must fail closed with the build hint: {:?}", other.err().map(|e| match e {
            Fail::Selftest(m) | Fail::Crash(m) => m,
        })),
    }
    let _ = std::fs::remove_dir_all(&fake);
}

#[test]
fn red_binding_source_without_gate_contract() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let root = repo();
    let fake = scratch("no-gate-api");
    // the gate the judge requires (owner `hintlib::evidence::cgate`): its launcher and the `.py` original both absent
    // (plan_26111615 pass 3: the gate is the launcher; its API is judged on the built policy-gate source).
    // one shadow of the runtime directory (siblings are symlinks), then both gate entries go
    shadow(&root, &fake, ".claude/policies/runtime/completion_gate.py", "", false);
    for rel in [".claude/policies/runtime/completion_gate.py", ".claude/policies/runtime/completion_gate.sh"] {
        let _ = std::fs::remove_file(fake.join(rel));
        assert!(fake.join(rel).symlink_metadata().is_err(), "{rel} must be absent in the fixture");
    }
    assert!(is_failure(hs::hint_binding_source(&fake)), "the binding-source judge passed without the gate");
    let _ = std::fs::remove_dir_all(&fake);
}

#[test]
fn docker_predicate_vacuity() {
    let v = |x: &[&str]| x.iter().map(|a| a.to_string()).collect::<Vec<String>>();
    let c1 = format!("c0ffee{:058x}", 1);
    let ok = vec![
        v(&["image", "inspect", "fixture"]),
        v(&["create", "--pull", "never", "--network", "none", "--entrypoint", "/bin/true", "fixture"]),
        v(&["cp", "-L", &format!("{c1}:/opt/easy-vllm/build_ledger.json"), "/tmp/x"]),
        v(&["rm", &c1]),
    ];
    assert!(hs::docker_calls_non_starting(&ok).is_empty());
    assert!(!hs::docker_calls_non_starting(&[v(&["image", "rm", "x"])]).is_empty());
    assert!(!hs::docker_calls_non_starting(&ok[..3]).is_empty(), "a container left behind must be a violation");
    assert!(!hs::docker_calls_non_starting(&[v(&["start", &c1])]).is_empty());
}
