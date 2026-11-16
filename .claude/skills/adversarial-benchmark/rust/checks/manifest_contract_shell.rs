//! Shell-level checks of the `manifest_contract` launcher and of its two callers (`run_bench.sh`, `lite_bench.sh`), run against
//! owned fixture repositories under the system temporary directory. Every run stops before any workload side effect (before
//! the environment file is read), no Docker/serving/download is reachable, and the canonical Python owner is never started:
//! the owner file of a fixture is either a marker script that records a start (and must stay unrecorded) or the reviewed
//! bytes (identical bytes route natively, so Python is never entered). `git` is replaced by a shim that always fails.
//!
//! Checked: a missing or non-executable program is a visible exit 2 with build guidance before any legacy route, even when an
//! explicit legacy interpreter is named; `MANIFEST_CONTRACT_VALIDATE_ONLY=1` is an availability check only (no program start,
//! no observer line); the launcher exports the owner and treats an empty interpreter variable as unset; the callers show the
//! launcher's guidance and propagate its status (outside the old discarded-status exit 4); a native run with an unissued Flag
//! is still 4, an issued Flag passes the gate (the caller then stops at its own missing environment file with 2), and a
//! caller whose owner file is absent keeps the old exit 4 without any availability check.

use role_contract::source_identity::REVIEWED_MANIFEST_CONTRACT;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

const NATIVE: &str = env!("CARGO_BIN_EXE_manifest_contract");
const SCRIPTS: &str = "../scripts";
const LAUNCHER_REL: &str = ".claude/skills/terraforming_node/scripts/manifest_contract.sh";
/// The reviewed original lives beside the launcher as the compat owner (`manifest_contract.py` is the thin import adapter).
const OWNER_REL: &str =
    ".claude/skills/terraforming_node/scripts/manifest_contract_compat_owner.py";
/// The callers (`run_bench.sh`, `lite_bench.sh`) take the presence of `manifest_contract.py` (now the thin import adapter) as the gate's owner presence.
const ADAPTER_REL: &str = ".claude/skills/terraforming_node/scripts/manifest_contract.py";

fn installed(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)
}

struct Fixture {
    root: PathBuf,
    marker: PathBuf,
    shims: PathBuf,
}

fn write(path: &Path, text: &[u8], mode: u32) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}

/// A repository with the launcher beside an owner file. `owner`: the bytes of `manifest_contract.py` (`None`: no owner file).
fn fixture(owner: Option<&[u8]>) -> Fixture {
    static COUNT: AtomicUsize = AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "manifest-contract-shell-{}-{}",
        std::process::id(),
        COUNT.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&root);
    let launcher = std::fs::read(installed(
        "../../terraforming_node/scripts/manifest_contract.sh",
    ))
    .unwrap();
    write(&root.join(LAUNCHER_REL), &launcher, 0o755);
    if let Some(owner) = owner {
        write(&root.join(OWNER_REL), owner, 0o755);
        write(
            &root.join(ADAPTER_REL),
            b"# stand-in for the thin import adapter: the callers only test that it exists\n",
            0o644,
        );
    }
    let shims = root.join("shims");
    write(&shims.join("git"), b"#!/bin/sh\nexit 1\n", 0o755);
    Fixture {
        marker: root.join("marker.txt"),
        root,
        shims,
    }
}

impl Fixture {
    fn command(&self, program: &Path) -> Command {
        let mut command = Command::new("bash");
        command
            .arg(program)
            .current_dir(&self.root)
            .env_clear()
            .env("PATH", format!("{}:/usr/bin:/bin", self.shims.display()))
            .env("HOME", &self.root)
            .env("GIT_CEILING_DIRECTORIES", self.root.parent().unwrap())
            .env("MC_TEST_MARKER", &self.marker)
            .stdin(Stdio::null());
        command
    }
    fn launcher(&self) -> Command {
        self.command(&self.root.join(LAUNCHER_REL))
    }
    fn caller(&self, name: &str) -> Command {
        self.command(&installed(SCRIPTS).join(name))
    }
    /// A recording stand-in for the native program (the launcher only checks that the file is executable).
    fn recording_program(&self) -> PathBuf {
        let path = self.root.join("recording-program");
        write(
            &path,
            b"#!/bin/bash\nprintf '%s\\n' \"args:$*\" \"owner:${MANIFEST_CONTRACT_COMPAT_OWNER-unset}\" \"python:${MANIFEST_CONTRACT_PYTHON-unset}\" \"validate:${MANIFEST_CONTRACT_VALIDATE_ONLY-unset}\" >> \"$MC_TEST_MARKER\"\n",
            0o755,
        );
        path
    }
    fn marker_text(&self) -> String {
        std::fs::read_to_string(&self.marker).unwrap_or_default()
    }
    fn manifest(&self, yaml: &str) {
        write(
            &self.root.join("output/single/manifest.yaml"),
            yaml.as_bytes(),
            0o644,
        );
    }
}

fn run(command: &mut Command) -> (i32, String, String) {
    let output = command.output().expect("bash starts");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

const EXACT: [&str; 6] = [
    "--topology",
    "single",
    "--repo",
    ".",
    "--require-flag",
    "--json",
];

#[test]
fn a_missing_or_non_executable_program_is_a_visible_exit_2_before_any_legacy_route() {
    let f = fixture(Some(b"#!/bin/sh\necho started >> \"$MC_TEST_MARKER\"\n"));
    let absent = f.root.join("no-such-program");
    let (code, out, err) = run(f
        .launcher()
        .env("MANIFEST_CONTRACT_BIN", &absent)
        .args(EXACT));
    assert_eq!(code, 2);
    assert!(out.is_empty());
    assert!(
        err.contains("[manifest_contract] ERROR Rust 실행파일이 없다/실행 불가"),
        "{err}"
    );
    assert!(err.contains("build_rust.sh"), "{err}");
    let plain_file = f.root.join("not-executable");
    write(&plain_file, b"#!/bin/sh\n", 0o644);
    let (code, _, err) = run(f
        .launcher()
        .env("MANIFEST_CONTRACT_BIN", &plain_file)
        .args(EXACT));
    assert_eq!(code, 2, "{err}");
    // An explicit legacy interpreter does not rescue a missing program, and a legacy shape does not either.
    for args in [&EXACT[..], &["-h"][..], &["--self-test"][..]] {
        let (code, _, err) = run(f
            .launcher()
            .env("MANIFEST_CONTRACT_BIN", &absent)
            .env("MANIFEST_CONTRACT_PYTHON", "/bin/sh")
            .args(args));
        assert_eq!(code, 2, "{err}");
        assert!(err.contains("실행파일이 없다"), "{err}");
    }
    assert_eq!(
        f.marker_text(),
        "",
        "no program, interpreter or owner was started"
    );
}

#[test]
fn validate_only_is_an_availability_check_and_nothing_else() {
    let f = fixture(Some(b"stub owner\n"));
    let program = f.recording_program();
    let observer = f.root.join("observer.jsonl");
    let (code, out, err) = run(f
        .launcher()
        .env("MANIFEST_CONTRACT_BIN", &program)
        .env("MANIFEST_CONTRACT_VALIDATE_ONLY", "1")
        .env("MANIFEST_CONTRACT_ROUTE_OBSERVER", &observer)
        .args(EXACT));
    assert_eq!((code, out.as_str(), err.as_str()), (0, "", ""));
    assert_eq!(f.marker_text(), "", "the program was not started");
    assert!(!observer.exists(), "no observer line");
    let (code, _, err) = run(f
        .launcher()
        .env("MANIFEST_CONTRACT_BIN", f.root.join("missing"))
        .env("MANIFEST_CONTRACT_VALIDATE_ONLY", "1"));
    assert_eq!(
        code, 2,
        "an unavailable program fails the availability check: {err}"
    );
    // Any other value is not an availability check: the program starts and does not see the variable.
    let (code, _, _) = run(f
        .launcher()
        .env("MANIFEST_CONTRACT_BIN", &program)
        .env("MANIFEST_CONTRACT_VALIDATE_ONLY", "2")
        .args(["--json"]));
    assert_eq!(code, 0);
    assert!(
        f.marker_text().contains("args:--json") && f.marker_text().contains("validate:unset"),
        "{}",
        f.marker_text()
    );
}

#[test]
fn the_launcher_exports_the_owner_and_never_passes_an_interpreter() {
    let f = fixture(Some(b"stub owner\n"));
    let program = f.recording_program();
    run(f
        .launcher()
        .env("MANIFEST_CONTRACT_BIN", &program)
        .env("MANIFEST_CONTRACT_PYTHON", "")
        .args(EXACT));
    run(f
        .launcher()
        .env("MANIFEST_CONTRACT_BIN", &program)
        .env("MANIFEST_CONTRACT_PYTHON", "/opt/py")
        .args(EXACT));
    let text = f.marker_text();
    let owner = f.root.join(OWNER_REL);
    assert_eq!(
        text.matches(&format!("owner:{}", owner.display())).count(),
        2,
        "{text}"
    );
    assert_eq!(text.matches("python:unset").count(), 2, "no interpreter variable reaches the program: {text}");
}

const RUN_BENCH: [&str; 11] = [
    "cfg",
    "--topology",
    "single",
    "--backend",
    "openai",
    "--input-len",
    "8",
    "--output-len",
    "8",
    "--num-prompts",
    "1",
];

#[test]
fn the_callers_show_the_launcher_guidance_and_propagate_its_status() {
    for (script, tag) in [
        ("run_bench.sh", "[run_bench]"),
        ("lite_bench.sh", "[lite_bench]"),
    ] {
        let f = fixture(Some(b"#!/bin/sh\necho started >> \"$MC_TEST_MARKER\"\n"));
        let args: Vec<&str> = if script == "run_bench.sh" {
            RUN_BENCH.to_vec()
        } else {
            vec!["cfg", "--topology", "single", "--backend", "openai"]
        };
        let (code, _, err) = run(f
            .caller(script)
            .env("MANIFEST_CONTRACT_BIN", f.root.join("missing"))
            .args(&args));
        assert_eq!(code, 2, "{script}: {err}");
        assert!(
            err.contains("[manifest_contract] ERROR Rust 실행파일이 없다/실행 불가"),
            "{script}: {err}"
        );
        assert!(
            err.contains(&format!(
                "{tag} ERROR 테라포밍-완수 Flag 게이트 런타임(manifest_contract)을 쓸 수 없다"
            )),
            "{script}: {err}"
        );
        assert!(
            !err.contains("info-only(fail-closed"),
            "{script}: not the old redirect: {err}"
        );
        assert_eq!(f.marker_text(), "", "{script}: the owner was not started");
    }
}

#[test]
fn a_native_gate_keeps_the_old_flag_statuses_and_stops_before_the_workload() {
    let issued = "terraforming: {complete: true, branch_verified: true}\ntopology: single\ngpus_per_node: 1\nmodel_source: managed\n";
    let unissued = issued.replace("complete: true", "complete: false");
    for (script, tag) in [
        ("run_bench.sh", "[run_bench]"),
        ("lite_bench.sh", "[lite_bench]"),
    ] {
        let args: Vec<&str> = if script == "run_bench.sh" {
            RUN_BENCH.to_vec()
        } else {
            vec!["cfg", "--topology", "single", "--backend", "openai"]
        };
        // Reviewed owner bytes route natively: the Python program is never entered.
        let f = fixture(Some(REVIEWED_MANIFEST_CONTRACT));
        f.manifest(&unissued);
        let (code, _, err) = run(f
            .caller(script)
            .env("MANIFEST_CONTRACT_BIN", NATIVE)
            .args(&args));
        assert_eq!(
            code, 4,
            "{script}: an unissued Flag is the old exit 4: {err}"
        );
        assert!(
            err.contains(tag) && err.contains("info-only"),
            "{script}: {err}"
        );
        f.manifest(issued);
        let (code, _, err) = run(f
            .caller(script)
            .env("MANIFEST_CONTRACT_BIN", NATIVE)
            .args(&args));
        assert_eq!(code, 2, "{script}: the gate passed and the caller stops at its own missing environment file: {err}");
        assert!(err.contains("envfile 없음"), "{script}: {err}");
        // No gate program (its launcher is the program; the retired .py is no longer the presence test): the old branch
        // (exit 4), with no availability check and no native start.
        let g = fixture(None);
        std::fs::remove_file(g.root.join(LAUNCHER_REL)).unwrap();
        g.manifest(issued);
        let (code, _, err) = run(g
            .caller(script)
            .env("MANIFEST_CONTRACT_BIN", g.root.join("missing"))
            .args(&args));
        assert_eq!(code, 4, "{script}: {err}");
        assert!(
            !err.contains("실행파일이 없다"),
            "{script}: no availability check without an owner: {err}"
        );
    }
}
