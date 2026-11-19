use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

struct Tree(PathBuf);
impl Drop for Tree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn tree(tag: &str) -> Tree {
    let root = std::env::temp_dir().join(format!("policy-audit-hv-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for d in [".claude/policies/runtime", ".claude/policies/predicates"] {
        std::fs::create_dir_all(root.join(d)).unwrap();
    }
    let stage = |rel: &str, name: &str| {
        let body = format!(
            "#!/bin/bash\necho \"{name} $*\" >>\"$HV_LOG\"\necho \"{name} out\"\nexit \"${{RC_{}:-0}}\"\n",
            name.to_uppercase()
        );
        std::fs::write(root.join(rel), body).unwrap();
    };
    stage(".claude/policies/runtime/policy_registry.sh", "registry");
    stage(".claude/policies/runtime/runtime_selftest.sh", "runtime");
    stage(".claude/policies/predicates/claim_predicates.sh", "claim");
    stage(".claude/policies/predicates/companion_predicate.sh", "companion");
    stage(".claude/policies/predicates/runtime_patch_predicate.sh", "patch");
    Tree(root)
}

fn run(t: &Tree, args: &[&str], env: &[(&str, &str)]) -> (i32, String, String, String) {
    let log = t.0.join("log");
    let _ = std::fs::remove_file(&log);
    let mut c = Command::new(env!("CARGO_BIN_EXE_harness_verify"));
    c.arg0(t.0.join(".claude/policies/runtime/harness_verify.sh")).args(args).env("HV_LOG", &log);
    for (k, v) in env {
        c.env(k, v);
    }
    let o = c.output().unwrap();
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
        std::fs::read_to_string(&log).unwrap_or_default(),
    )
}

#[test]
fn all_green_runs_every_stage_in_order() {
    let t = tree("green");
    let (rc, out, err, log) = run(&t, &["--registry", "r.json", "--repo-root", "x"], &[]);
    assert_eq!(rc, 0, "{err}");
    assert_eq!(log, "registry verify --registry r.json --repo-root x\nruntime \nclaim \ncompanion \npatch \n");
    assert_eq!(
        out,
        "registry out\nruntime out\nclaim out\ncompanion out\npatch out\n\
[harness_verify] stage 1/3: policy_registry.sh verify\n\
[harness_verify] stage 2/3: constitution-owned runtime regression probe\n\
[harness_verify] stage 3/3: constitution-owned production predicates\n\
[harness_verify] PASS: policy audit, runtime regression, and production predicates green.\n"
    );
    assert!(err.is_empty());
}

#[test]
fn stage_failures_stop_and_map_exit_codes() {
    let t = tree("fail");
    let (rc, _, err, log) = run(&t, &[], &[("RC_REGISTRY", "1")]);
    assert_eq!((rc, log.lines().count()), (1, 1));
    assert!(err.contains("BLOCKED: policy audit exited 1"));
    let (rc, _, err, log) = run(&t, &[], &[("RC_RUNTIME", "5")]);
    assert_eq!((rc, log.lines().count()), (4, 2));
    assert!(err.contains("BLOCKED: runtime regression probe exited 5."));
    let (rc, _, err, log) = run(&t, &[], &[("RC_CLAIM", "1")]);
    assert_eq!((rc, log.lines().count()), (3, 3));
    assert!(err.contains("BLOCKED: production predicate suite exited 1."));
    let (rc, _, _, log) = run(&t, &[], &[("RC_COMPANION", "1")]);
    assert_eq!(rc, 3);
    assert!(log.ends_with("companion \npatch \n"), "every companion runs: {log}");
}

#[test]
fn a_missing_stage_launcher_fails_closed() {
    let t = tree("missing");
    std::fs::remove_file(t.0.join(".claude/policies/predicates/runtime_patch_predicate.sh")).unwrap();
    let (rc, _, err, _) = run(&t, &[], &[]);
    assert_eq!(rc, 3);
    assert!(err.contains("stage launcher is missing (no Python fallback)") && err.contains("runtime_patch_predicate.sh"), "{err}");
    std::fs::remove_file(t.0.join(".claude/policies/runtime/policy_registry.sh")).unwrap();
    let (rc, _, _, log) = run(&t, &[], &[]);
    assert_eq!((rc, log.as_str()), (2, ""));
}

#[test]
fn the_retired_as_of_option_is_a_usage_error() {
    let t = tree("asof");
    for args in [&["--as-of", "2026-01-15"][..], &["--as-of=2026-01-15"][..]] {
        let (rc, out, err, log) = run(&t, args, &[]);
        assert_eq!((rc, out.as_str(), log.as_str()), (2, "", ""), "{args:?}: {err}");
        assert!(err.contains("error: unrecognized arguments: --as-of"), "{args:?}: {err}");
    }
}
