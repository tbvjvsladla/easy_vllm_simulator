//! `publish_install_request` without `--topology` asks the topology owner. The owner is the Rust launcher
//! `.claude/skills/terraforming_node/scripts/manifest_contract.sh`; until plan_26111717 the program looked for the deleted
//! `manifest_contract.py`, so every run without `--topology` was refused even in a complete repository.

use std::path::Path;
use std::process::Command;

const OWNER_REFUSAL: &str = "manifest_contract 소유자를 찾지 못했다";

fn fixture(with_owner: bool) -> tempfile_dir::TempDir {
    let dir = tempfile_dir::TempDir::new();
    let root = dir.path();
    let git = |args: &[&str]| {
        let ok = Command::new("git")
            .args(args)
            .current_dir(root)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_AUTHOR_NAME", "fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@invalid")
            .env("GIT_COMMITTER_NAME", "fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@invalid")
            .output()
            .expect("git")
            .status
            .success();
        assert!(ok, "git {args:?}");
    };
    git(&["init", "-q", "-b", "single-node"]);
    let scripts = root.join(".claude/skills/terraforming_node/scripts");
    std::fs::create_dir_all(&scripts).unwrap();
    if with_owner {
        std::fs::write(scripts.join("manifest_contract.sh"), "#!/bin/bash\n").unwrap();
    }
    // the program resolves its doc-naming owner before the topology (the real file; it is only called after the topology)
    let naming = root.join(".claude/skills/wiki-desk/src");
    std::fs::create_dir_all(&naming).unwrap();
    let real = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../wiki-desk/src/doc_naming.ts");
    std::fs::copy(&real, naming.join("doc_naming.ts")).expect("doc_naming.ts");
    std::fs::write(root.join("README.md"), "fixture\n").unwrap();
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "fixture"]);
    dir
}

fn run(repo: &Path) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_publish_install_request"))
        .args(["--generated-utc", "2026-08-18T05:00:00Z", "--repo"])
        .arg(repo)
        .args(["--out-dir", "out"])
        .current_dir(repo)
        .output()
        .expect("publish_install_request");
    (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stderr).into_owned())
}

#[test]
fn the_rust_owner_launcher_satisfies_the_topology_owner_check() {
    let dir = fixture(true);
    let (code, stderr) = run(dir.path());
    assert_eq!(code, 0, "owner present: the request must be published without --topology: {stderr}");
    assert!(!stderr.contains(OWNER_REFUSAL), "owner present but refused: {stderr}");
    assert!(!stderr.contains("No module named"), "Python import wording must be gone: {stderr}");
}

#[test]
fn a_repository_without_the_owner_is_still_refused_explicitly() {
    let dir = fixture(false);
    let (code, stderr) = run(dir.path());
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains(OWNER_REFUSAL), "{stderr}");
    assert!(stderr.contains("manifest_contract.sh"), "{stderr}");
}

/// Minimal self-cleaning temporary directory (no extra crate).
mod tempfile_dir {
    use std::path::{Path, PathBuf};
    pub struct TempDir(PathBuf);
    impl TempDir {
        pub fn new() -> Self {
            let base = std::env::temp_dir();
            let pid = std::process::id();
            for n in 0u32.. {
                let p = base.join(format!("install-request-owner-{pid}-{n}"));
                if std::fs::create_dir(&p).is_ok() {
                    return TempDir(p);
                }
            }
            unreachable!()
        }
        pub fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}
