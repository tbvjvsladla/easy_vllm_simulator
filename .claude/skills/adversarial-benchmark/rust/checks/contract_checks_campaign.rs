//! Contract checks of the campaign, Flag-gate, scan, turn-budget and topology bins this crate builds for terraforming_node
//! and the policies (plan 26111805 「보존 계약」, cell c21c-contract-tests): what their consumers read
//! (runtime-support/contract/consumers.tsv L1·L2 rows of campaign_init, campaign_template_validator, manifest_contract,
//! scan_node, turn_budget, topology_parity and node_identity_resolve) —
//! - every consumer starts a bin through its launcher (`terraforming_node/scripts/*.sh`,
//!   `.claude/policies/runtime/topology_parity.sh`); the launchers honour the override variables (`CAMPAIGN_READ_BIN`,
//!   `CAMPAIGN_VALIDATOR_BIN`, `MANIFEST_CONTRACT_BIN`, `SCAN_NODE_BIN`, `TURN_BUDGET_BIN`, `TOPOLOGY_PARITY_BIN`) and the
//!   availability checks (`*_VALIDATE_ONLY=1`), export the owner path the programs derive the checkout root from, and
//!   refuse an absent executable with exit 2 and nothing on stdout;
//! - broad_search.sh·sweep_bench.sh·lite_bench.sh·fetch_sub_docs.sh·single_serve_up.sh·multinode_serve_smoke.sh,
//!   hint-publisher `evidence/e4.rs`, terraforming `relay/core.rs` and its lifecycle self-test, recipe
//!   `run_trial_session.rs`: the `campaign_init` read shapes (`--active`, `--derive`, `--assigned-cells`,
//!   `--lockset-precheck` JSON with exit 0/2, `--grounding-check`), the writers (`--phase-set`, `--cell-set`,
//!   `--write-brief`, `--escalation-add`, `--assign-set`), the relay shapes (`--emit-slice`, `--lifecycle` JSON), the
//!   `--resume-brief` first line, the `--verify-purge-gate` reason lines, usage exit 2;
//! - policy-distribution `checks.rs`: the self-tests (`--selftest`, `--self-test`, `--p1-self-check`) need exit 0;
//! - run_bench.sh·lite_bench.sh, policy-predicates `claims_owner.rs`·`claims_tf.rs`: the Flag gate `--topology --repo
//!   --require-flag|--json`, exit 0 = Flag, 3 = no manifest, 4 = Flag not issued; native_health_probes.sh: the private
//!   bridge (`"protocol": "manifest-contract-bridge/1"`, the typed wire `{"t": "int", "sign": "+", "hex": "1"}`) and the
//!   `turn_budget --self-test` `[PASS]` lines;
//! - closing_sequence.sh·sync_to_sub.sh (the launcher run directly, without `bash`): `topology_parity evaluate --repo
//!   --expect --format json|value`, JSON `verdict`·`topology`·`reason_codes`·`legs_judged`·`legs_checked`·`reasons`,
//!   exit 0 / 2 / 5.
//!
//! The bins run for real in scratch directories: a scratch checkout holds copies of the launchers (their location names
//! the checkout root) and of the tracked campaign skeleton; topology_parity reads throwaway local git repositories.
//! Values (L2) and exit codes, option names, line shapes and codes (L1) are fixed; message texts are not (L3).
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

use json::Json;

/// The terraforming_node launchers (repository-relative).
const TS: &str = ".claude/skills/terraforming_node/scripts";
const TOPOLOGY_PARITY_SH: &str = ".claude/policies/runtime/topology_parity.sh";
const UTC: &str = "2026-11-18T00:00:00Z";

/// The plumbing variables a launcher reads or exports: never inherited from the environment of the check run.
const PLUMBING: &[&str] = &[
    "CAMPAIGN_READ_BIN",
    "CAMPAIGN_READ_VALIDATE_ONLY",
    "CAMPAIGN_READ_COMPAT_OWNER",
    "CAMPAIGN_VALIDATOR_BIN",
    "CAMPAIGN_VALIDATOR_VALIDATE_ONLY",
    "CAMPAIGN_VALIDATOR_COMPAT_OWNER",
    "MANIFEST_CONTRACT_BIN",
    "MANIFEST_CONTRACT_VALIDATE_ONLY",
    "MANIFEST_CONTRACT_COMPAT_OWNER",
    "MANIFEST_CONTRACT_CALLER_ENV",
    "MANIFEST_CONTRACT_BRIDGE",
    "MANIFEST_CONTRACT_BRIDGE_REQUEST",
    "MANIFEST_CONTRACT_PUBLIC_ARGV0",
    "SCAN_NODE_BIN",
    "SCAN_NODE_VALIDATE_ONLY",
    "SCAN_NODE_OWNER_PATH",
    "SCAN_NODE_CALLER_ENV",
    "SCAN_NODE_BRIDGE",
    "TURN_BUDGET_BIN",
    "TURN_BUDGET_VALIDATE_ONLY",
    "TURN_BUDGET_OWNER_PATH",
    "TOPOLOGY_PARITY_BIN",
    "EASY_VLLM_SOURCE_ROOT",
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
];

/// The repository root (this crate is `.claude/skills/adversarial-benchmark/rust/`).
fn root() -> PathBuf {
    std::fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../..")).unwrap()
}

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn scratch(tag: &str) -> Scratch {
    static N: AtomicUsize = AtomicUsize::new(0);
    let n = N.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("adversarial-contract-campaign-{tag}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    Scratch(std::fs::canonicalize(&p).unwrap())
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn write(path: &Path, body: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

fn write_exe(path: &Path, body: &str) {
    write(path, body);
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn copy_file(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
    std::fs::copy(src, dst).unwrap();
}

fn copy_tree(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for e in std::fs::read_dir(src).unwrap() {
        let e = e.unwrap();
        let d = dst.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_tree(&e.path(), &d);
        } else {
            std::fs::copy(e.path(), &d).unwrap();
        }
    }
}

/// A command with stdin closed and no inherited launcher plumbing.
fn clean(mut cmd: Command) -> Command {
    cmd.stdin(Stdio::null());
    for name in PLUMBING {
        cmd.env_remove(name);
    }
    cmd
}

/// `bash <launcher> <args>` in `cwd` with the given environment.
fn bash(launcher: &Path, args: &[&OsStr], cwd: &Path, env: &[(&str, &OsStr)]) -> Output {
    let mut cmd = Command::new("bash");
    cmd.arg(launcher).args(args).current_dir(cwd);
    let mut cmd = clean(cmd);
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd.output().unwrap()
}

fn os<'a>(args: &[&'a str]) -> Vec<&'a OsStr> {
    args.iter().map(|a| OsStr::new(*a)).collect()
}

/// The whole stdout as one JSON document.
fn stdout_json(out: &Output) -> Json {
    let so = text(&out.stdout);
    Json::parse(&so).unwrap_or_else(|e| panic!("stdout is not one JSON document ({e}):\n{so}\nstderr:\n{}", text(&out.stderr)))
}

fn num(v: &Json) -> &str {
    match v {
        Json::Num(n) => n,
        other => panic!("not a number: {other:?}"),
    }
}

fn assert_rc(out: &Output, rc: i32, what: &str) {
    assert_eq!(out.status.code(), Some(rc), "{what}\nstdout:\n{}\nstderr:\n{}", text(&out.stdout), text(&out.stderr));
}

/// The first executable ELF of that name on PATH (a stand-in for a launcher that only starts an ELF).
fn elf_on_path(name: &str) -> PathBuf {
    let path = std::env::var_os("PATH").expect("PATH");
    for dir in std::env::split_paths(&path) {
        let p = dir.join(name);
        if let Ok(bytes) = std::fs::read(&p) {
            if bytes.starts_with(b"\x7fELF") {
                return p;
            }
        }
    }
    panic!("no ELF {name} on PATH");
}

/// `KEY=VALUE` lines of an `env` dump.
fn env_value<'a>(dump: &'a str, key: &str) -> Option<&'a str> {
    dump.lines().find_map(|l| l.strip_prefix(key).and_then(|r| r.strip_prefix('=')))
}

struct Launcher {
    rel: &'static str,
    var: &'static str,
    bin: &'static str,
    validate_only: Option<&'static str>,
    /// The launcher refuses a non-ELF executable (exit 2).
    elf_only: bool,
}

fn launchers() -> [Launcher; 6] {
    [
        Launcher {
            rel: ".claude/skills/terraforming_node/scripts/campaign_init.sh",
            var: "CAMPAIGN_READ_BIN",
            bin: env!("CARGO_BIN_EXE_campaign_init"),
            validate_only: Some("CAMPAIGN_READ_VALIDATE_ONLY"),
            elf_only: false,
        },
        Launcher {
            rel: ".claude/skills/terraforming_node/scripts/campaign_template_validator.sh",
            var: "CAMPAIGN_VALIDATOR_BIN",
            bin: env!("CARGO_BIN_EXE_campaign_template_validator"),
            validate_only: Some("CAMPAIGN_VALIDATOR_VALIDATE_ONLY"),
            elf_only: false,
        },
        Launcher {
            rel: ".claude/skills/terraforming_node/scripts/manifest_contract.sh",
            var: "MANIFEST_CONTRACT_BIN",
            bin: env!("CARGO_BIN_EXE_manifest_contract"),
            validate_only: Some("MANIFEST_CONTRACT_VALIDATE_ONLY"),
            elf_only: false,
        },
        Launcher {
            rel: ".claude/skills/terraforming_node/scripts/scan_node.sh",
            var: "SCAN_NODE_BIN",
            bin: env!("CARGO_BIN_EXE_scan_node"),
            validate_only: Some("SCAN_NODE_VALIDATE_ONLY"),
            elf_only: true,
        },
        Launcher {
            rel: ".claude/skills/terraforming_node/scripts/turn_budget.sh",
            var: "TURN_BUDGET_BIN",
            bin: env!("CARGO_BIN_EXE_turn_budget"),
            validate_only: Some("TURN_BUDGET_VALIDATE_ONLY"),
            elf_only: false,
        },
        Launcher {
            rel: TOPOLOGY_PARITY_SH,
            var: "TOPOLOGY_PARITY_BIN",
            bin: env!("CARGO_BIN_EXE_topology_parity"),
            validate_only: None,
            elf_only: true,
        },
    ]
}

/// The launchers the consumers and documents name exist; an absent override executable is exit 2 with nothing on stdout;
/// `<NAME>_VALIDATE_ONLY=1` (fetch_sub_docs.sh, broad_search.sh, run_bench.sh, lite_bench.sh) is exit 0 with no output
/// when the executable is there and exit 2 when it is not; the ELF-only launchers refuse a non-ELF executable with 2;
/// closing_sequence.sh and sync_to_sub.sh run `topology_parity.sh` directly (its executable bit).
#[test]
fn launchers_overrides_validate_only_and_absent_executable() {
    let s = scratch("launchers");
    let not_elf = s.0.join("not_elf");
    write_exe(&not_elf, "#!/bin/sh\necho stub\n");
    for l in launchers() {
        let path = root().join(l.rel);
        assert!(path.is_file(), "{}", l.rel);
        let out = bash(&path, &os(&["--help"]), &s.0, &[(l.var, OsStr::new("/nonexistent/contract-check/bin"))]);
        assert_rc(&out, 2, l.rel);
        assert!(out.stdout.is_empty(), "{}: stdout of an absent executable", l.rel);
        if let Some(vo) = l.validate_only {
            let out = bash(&path, &[], &s.0, &[(l.var, OsStr::new(l.bin)), (vo, OsStr::new("1"))]);
            assert_rc(&out, 0, vo);
            assert!(out.stdout.is_empty() && out.stderr.is_empty(), "{vo}: availability check has no output");
            let out = bash(&path, &[], &s.0, &[(l.var, OsStr::new("/nonexistent/contract-check/bin")), (vo, OsStr::new("1"))]);
            assert_rc(&out, 2, vo);
        }
        if l.elf_only {
            let out = bash(&path, &os(&["--help"]), &s.0, &[(l.var, not_elf.as_os_str())]);
            assert_rc(&out, 2, l.rel);
            assert!(out.stdout.is_empty(), "{}: a non-ELF executable is not started", l.rel);
        }
    }
    let tp = root().join(TOPOLOGY_PARITY_SH);
    assert_ne!(std::fs::metadata(&tp).unwrap().permissions().mode() & 0o111, 0, "topology_parity.sh is run without bash");
    let mut cmd = Command::new(&tp);
    cmd.arg("--self-test").current_dir(&s.0);
    let out = clean(cmd).env("TOPOLOGY_PARITY_BIN", env!("CARGO_BIN_EXE_topology_parity")).output().unwrap();
    assert_rc(&out, 0, "topology_parity.sh --self-test run directly");
}

/// The launchers hand the arguments through unchanged and export what the programs read: `CAMPAIGN_READ_COMPAT_OWNER`,
/// `CAMPAIGN_VALIDATOR_COMPAT_OWNER`, `TURN_BUDGET_OWNER_PATH`, `SCAN_NODE_OWNER_PATH` (= the launcher's own path unless
/// the caller set it), `MANIFEST_CONTRACT_CALLER_ENV=1` with the `MANIFEST_CONTRACT_CALLER_<suffix>` carriers and
/// `MANIFEST_CONTRACT_COMPAT_OWNER`, `SCAN_NODE_CALLER_ENV=1`; a validate-only value other than 1 is not handed on; the
/// manifest bridge (`MANIFEST_CONTRACT_BRIDGE`) reaches the program untouched, an inherited validate-only included.
#[test]
fn launchers_export_owner_paths_and_pass_arguments() {
    let s = scratch("exports");
    let stub = s.0.join("stub.sh");
    write_exe(&stub, "#!/bin/sh\nenv\nfor a in \"$@\"; do printf 'ARG:%s\\n' \"$a\"; done\n");
    let ts = root().join(TS);
    let run = |launcher: &str, env: &[(&str, &OsStr)]| -> String {
        let out = bash(&ts.join(launcher), &os(&["--x", "a b"]), &s.0, env);
        assert_rc(&out, 0, launcher);
        text(&out.stdout)
    };
    let owner = |name: &str| ts.join(name).to_str().unwrap().to_string();
    for (launcher, var, owner_var, vo) in [
        ("campaign_init.sh", "CAMPAIGN_READ_BIN", "CAMPAIGN_READ_COMPAT_OWNER", "CAMPAIGN_READ_VALIDATE_ONLY"),
        ("campaign_template_validator.sh", "CAMPAIGN_VALIDATOR_BIN", "CAMPAIGN_VALIDATOR_COMPAT_OWNER", "CAMPAIGN_VALIDATOR_VALIDATE_ONLY"),
        ("turn_budget.sh", "TURN_BUDGET_BIN", "TURN_BUDGET_OWNER_PATH", "TURN_BUDGET_VALIDATE_ONLY"),
        ("manifest_contract.sh", "MANIFEST_CONTRACT_BIN", "MANIFEST_CONTRACT_COMPAT_OWNER", "MANIFEST_CONTRACT_VALIDATE_ONLY"),
    ] {
        let dump = run(launcher, &[(var, stub.as_os_str()), (vo, OsStr::new("0"))]);
        assert_eq!(env_value(&dump, owner_var), Some(owner(launcher).as_str()), "{launcher}: {owner_var}");
        assert_eq!(env_value(&dump, vo), None, "{launcher}: {vo} other than 1 is not handed on");
        let args: Vec<&str> = dump.lines().filter_map(|l| l.strip_prefix("ARG:")).collect();
        assert_eq!(args, ["--x", "a b"], "{launcher}: arguments");
        let dump = run(launcher, &[(var, stub.as_os_str()), (owner_var, OsStr::new("/caller/owner"))]);
        assert_eq!(env_value(&dump, owner_var), Some("/caller/owner"), "{launcher}: a caller's {owner_var} is kept");
        if launcher == "manifest_contract.sh" {
            assert_eq!(env_value(&dump, "MANIFEST_CONTRACT_CALLER_ENV"), Some("1"));
            assert_eq!(env_value(&dump, "MANIFEST_CONTRACT_CALLER_COMPAT_OWNER"), Some("/caller/owner"));
            let dump = run(launcher, &[(var, stub.as_os_str())]);
            assert_eq!(env_value(&dump, "MANIFEST_CONTRACT_CALLER_ENV"), Some("1"));
            assert_eq!(env_value(&dump, "MANIFEST_CONTRACT_CALLER_COMPAT_OWNER"), None, "carrier absent = caller had it unset");
            let dump = run(
                launcher,
                &[(var, stub.as_os_str()), ("MANIFEST_CONTRACT_BRIDGE", OsStr::new("1")), (vo, OsStr::new("1"))],
            );
            assert_eq!(env_value(&dump, "MANIFEST_CONTRACT_BRIDGE"), Some("1"), "the bridge reaches the program");
            assert_eq!(env_value(&dump, vo), Some("1"), "the bridge keeps an inherited {vo}");
            assert_eq!(env_value(&dump, "MANIFEST_CONTRACT_CALLER_ENV"), None);
        }
    }
    // scan_node.sh starts only an ELF: a copy of `env` stands for the program.
    let fake = s.0.join("scan_node");
    copy_file(&elf_on_path("env"), &fake);
    let out = bash(&ts.join("scan_node.sh"), &[], &s.0, &[("SCAN_NODE_BIN", fake.as_os_str())]);
    assert_rc(&out, 0, "scan_node.sh with an env stand-in");
    let dump = text(&out.stdout);
    assert_eq!(env_value(&dump, "SCAN_NODE_OWNER_PATH"), Some(owner("scan_node.sh").as_str()));
    assert_eq!(env_value(&dump, "SCAN_NODE_CALLER_ENV"), Some("1"));
}

const VALID_MANIFEST: &str =
    "topology: single\ngpus_per_node: 1\nmodel_source: managed\nnodes: []\nterraforming:\n  complete: true\n  branch_verified: true\n";

fn manifest_contract(cwd: &Path, args: &[&OsStr]) -> Output {
    bash(
        &root().join(TS).join("manifest_contract.sh"),
        args,
        cwd,
        &[("MANIFEST_CONTRACT_BIN", OsStr::new(env!("CARGO_BIN_EXE_manifest_contract")))],
    )
}

/// The Flag gate (run_bench.sh·lite_bench.sh `--topology --repo --require-flag`, claims_owner.rs·claims_tf.rs
/// `--json`/`--require-flag`, policy-distribution `checks.rs` `--self-test`): exit 0 = Flag issued, 3 = no manifest,
/// 4 = a manifest without the Flag, 2 = usage (a non-UTF-8 choice included); `--json` prints one object (`flag`,
/// `reason`, `topology`, `redirect` when absent; `exit_code`, `gpus_per_node`, `manifest_tp`, `model_source`,
/// `manifest_path` when issued) and nothing on stderr.
#[test]
fn manifest_contract_flag_gate_exit_codes_and_json() {
    let s = scratch("flag");
    let absent = s.0.join("absent");
    std::fs::create_dir_all(&absent).unwrap();
    let present = s.0.join("present");
    write(&present.join("output/single/manifest.yaml"), VALID_MANIFEST);
    let unissued = s.0.join("unissued");
    write(
        &unissued.join("output/single/manifest.yaml"),
        &VALID_MANIFEST.replace("complete: true", "complete: false"),
    );
    let gate = |repo: &Path, topology: &str, extra: &[&str]| {
        let mut args = vec![OsStr::new("--topology"), OsStr::new(topology), OsStr::new("--repo"), repo.as_os_str()];
        args.extend(extra.iter().map(OsStr::new));
        manifest_contract(&s.0, &args)
    };

    let out = gate(&absent, "single", &["--json"]);
    assert_rc(&out, 3, "absent --json");
    assert!(out.stderr.is_empty(), "--json keeps stderr empty");
    let doc = stdout_json(&out);
    assert_eq!(doc.at("flag"), &Json::Bool(false));
    assert_eq!(doc.at("topology").str(), Some("single"));
    assert_eq!(doc.at("reason").kind(), "string");
    assert_eq!(doc.at("redirect").kind(), "string");
    for extra in [&["--require-flag"][..], &[]] {
        let out = gate(&absent, "single", extra);
        assert_rc(&out, 3, "absent");
        assert!(out.stdout.is_empty(), "the reason goes to stderr");
        assert!(!out.stderr.is_empty());
    }
    assert_rc(&gate(&present, "multi", &["--require-flag"]), 3, "no output/multi manifest");

    assert_rc(&gate(&present, "single", &["--require-flag"]), 0, "Flag issued");
    let out = gate(&present, "single", &["--require-flag", "--json"]);
    assert_rc(&out, 0, "Flag issued --json");
    assert!(out.stderr.is_empty());
    let doc = stdout_json(&out);
    assert_eq!(doc.at("flag"), &Json::Bool(true));
    assert_eq!(num(doc.at("exit_code")), "0");
    assert_eq!(doc.at("topology").str(), Some("single"));
    assert_eq!(num(doc.at("gpus_per_node")), "1");
    assert_eq!(num(doc.at("manifest_tp")), "1");
    assert_eq!(doc.at("model_source").str(), Some("managed"));
    assert_eq!(doc.at("manifest_path").kind(), "string");
    assert_eq!(doc.at("nodes").kind(), "array");

    assert_rc(&gate(&unissued, "single", &["--require-flag"]), 4, "a manifest without the Flag");

    let out = gate(&present, "triple", &[]);
    assert_rc(&out, 2, "--topology outside the choices");
    assert!(out.stdout.is_empty());
    let out = manifest_contract(
        &s.0,
        &[OsStr::new("--topology"), OsStr::from_bytes(b"sin\xffgle"), OsStr::new("--repo"), present.as_os_str()],
    );
    assert_rc(&out, 2, "non-UTF-8 --topology");
    assert!(out.stdout.is_empty());

    let out = manifest_contract(&s.0, &os(&["--self-test"]));
    assert_rc(&out, 0, "--self-test");
    let out = manifest_contract(&s.0, &os(&["--help"]));
    assert_rc(&out, 0, "--help");
    let help = text(&out.stdout);
    for opt in ["--topology", "--repo", "--require-flag", "--json", "--self-test"] {
        assert!(help.contains(opt), "manifest_contract option {opt}");
    }
}

/// native_health_probes.sh: the private bridge (`MANIFEST_CONTRACT_BRIDGE=1`, `MANIFEST_CONTRACT_BRIDGE_REQUEST=<0600
/// file>`, stdin closed) answers `manifest_tp` with the typed wire the probe matches by regex — `"protocol":
/// "manifest-contract-bridge/1"`, `"status": "raised"` + `"cls": "ValueError"` for `{}`, `"status": "returned"` +
/// `"value": {"t": "int", "sign": "+", "hex": "1"}` (spacing included) for `{gpus_per_node: 1}` on `single`; an
/// unreadable request is exit 2; the probe script itself passes against this build.
#[test]
fn manifest_contract_private_bridge_and_health_probe() {
    let s = scratch("bridge");
    let single = r#"{"t":"str","utf8_surrogatepass_hex":"73696e676c65","codepoints":6}"#;
    let gpus = r#"{"t":"str","utf8_surrogatepass_hex":"677075735f7065725f6e6f6465","codepoints":13}"#;
    let one = r#"{"t":"int","sign":"+","hex":"1"}"#;
    let ask = |request: &str, extra: &[(&str, &OsStr)]| {
        let req = s.0.join("request");
        std::fs::write(&req, request).unwrap();
        std::fs::set_permissions(&req, std::fs::Permissions::from_mode(0o600)).unwrap();
        let mut env = vec![
            ("MANIFEST_CONTRACT_BRIDGE", OsStr::new("1")),
            ("MANIFEST_CONTRACT_BRIDGE_REQUEST", req.as_os_str()),
            ("MANIFEST_CONTRACT_BIN", OsStr::new(env!("CARGO_BIN_EXE_manifest_contract"))),
        ];
        env.extend_from_slice(extra);
        bash(&root().join(TS).join("manifest_contract.sh"), &[], &s.0, &env)
    };
    let out = ask(&format!(r#"{{"v":1,"op":"manifest_tp","args":[{{"t":"dict","id":1,"pairs":[]}},{single}]}}"#), &[]);
    assert_rc(&out, 0, "bridge manifest_tp({})");
    let raw = text(&out.stdout);
    for needle in [r#""protocol": "manifest-contract-bridge/1""#, r#""status": "raised""#, r#""cls": "ValueError""#] {
        assert!(raw.contains(needle), "{needle} in {raw}");
    }
    let doc = stdout_json(&out);
    assert_eq!(doc.at("protocol").str(), Some("manifest-contract-bridge/1"));
    assert_eq!(doc.at("answer").at("status").str(), Some("raised"));

    let request = format!(r#"{{"v":1,"op":"manifest_tp","args":[{{"t":"dict","id":1,"pairs":[[{gpus},{one}]]}},{single}]}}"#);
    let out = ask(&request, &[("MANIFEST_CONTRACT_VALIDATE_ONLY", OsStr::new("1"))]);
    assert_rc(&out, 0, "bridge manifest_tp({gpus_per_node: 1}, single)");
    let raw = text(&out.stdout);
    assert!(raw.contains(r#""status": "returned""#), "{raw}");
    assert!(raw.contains(r#""value": {"t": "int", "sign": "+", "hex": "1"}"#), "typed int wire: {raw}");
    let doc = stdout_json(&out);
    assert_eq!(doc.at("answer").at("value").at("hex").str(), Some("1"));

    let out = ask("not json", &[]);
    assert_rc(&out, 2, "unreadable bridge request");
    assert!(out.stdout.is_empty());

    // The probe script as build_rust.sh runs it, pointed at this build (`<crate>/target/release/<bin>`).
    let crate_dir = s.0.join("crate");
    let release = crate_dir.join("target/release");
    std::fs::create_dir_all(&release).unwrap();
    std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_manifest_contract"), release.join("manifest_contract")).unwrap();
    std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_turn_budget"), release.join("turn_budget")).unwrap();
    let probe = root().join(".claude/skills/adversarial-benchmark/scripts/native_health_probes.sh");
    let mut cmd = Command::new("bash");
    cmd.arg(&probe).arg(&crate_dir).current_dir(&s.0);
    let out = clean(cmd).output().unwrap();
    assert_rc(&out, 0, "native_health_probes.sh");
}

fn turn_budget(cwd: &Path, args: &[&str]) -> Output {
    bash(
        &root().join(TS).join("turn_budget.sh"),
        &os(args),
        cwd,
        &[("TURN_BUDGET_BIN", OsStr::new(env!("CARGO_BIN_EXE_turn_budget")))],
    )
}

/// turn_budget (native_health_probes.sh, policy-distribution `checks.rs`, sub_agent_env.md): `--self-test` is exit 0 with
/// `[PASS]` lines and no `[FAIL]` line; `--caps` prints `{max_turns, timeout_seconds, source}`; a declaration
/// `--max-turns --timeout-seconds --source` prints the declared values; a refused declaration is exit 1 with nothing on
/// stdout; a usage error is exit 2.
#[test]
fn turn_budget_self_test_caps_and_declaration() {
    let s = scratch("turn-budget");
    let out = turn_budget(&s.0, &["--self-test"]);
    assert_rc(&out, 0, "--self-test");
    let so = text(&out.stdout);
    assert!(so.contains("[PASS]") && !so.contains("[FAIL]"), "{so}");

    let out = turn_budget(&s.0, &["--caps"]);
    assert_rc(&out, 0, "--caps");
    let doc = stdout_json(&out);
    assert_eq!(doc.at("max_turns").kind(), "number");
    assert_eq!(doc.at("timeout_seconds").kind(), "number");
    assert_eq!(doc.at("source").kind(), "string");

    let out = turn_budget(&s.0, &["--max-turns", "10", "--timeout-seconds", "600", "--source", "contract"]);
    assert_rc(&out, 0, "declaration");
    let doc = stdout_json(&out);
    assert_eq!(num(doc.at("max_turns")), "10");
    assert_eq!(num(doc.at("timeout_seconds")), "600");
    assert_eq!(doc.at("source").str(), Some("contract"));

    for args in [&["--max-turns", "10", "--timeout-seconds", "600"][..], &["--max-turns", "0", "--timeout-seconds", "600", "--source", "x"]] {
        let out = turn_budget(&s.0, args);
        assert_rc(&out, 1, "refused declaration");
        assert!(out.stdout.is_empty(), "{args:?}");
    }
    let out = turn_budget(&s.0, &["--max-turns", "abc", "--source", "x"]);
    assert_rc(&out, 2, "usage");
    assert!(out.stdout.is_empty());
    let out = turn_budget(&s.0, &["--help"]);
    assert_rc(&out, 0, "--help");
    let help = text(&out.stdout);
    for opt in ["--max-turns", "--timeout-seconds", "--source", "--caps", "--self-test"] {
        assert!(help.contains(opt), "turn_budget option {opt}");
    }
}

/// scan_node (policy-distribution `checks.rs`): `--self-test` and `--p1-self-check` are exit 0 (no hardware, no host
/// scan); the long options the documents and the sub-manifest path name are part of the parser. The scanning shapes
/// (host probes, `--peer-ssh`, `--check-egress`) touch the hardware or the network and are not started here.
#[test]
fn scan_node_self_tests_and_options() {
    let s = scratch("scan-node");
    let scan = |args: &[&str]| {
        bash(
            &root().join(TS).join("scan_node.sh"),
            &os(args),
            &s.0,
            &[("SCAN_NODE_BIN", OsStr::new(env!("CARGO_BIN_EXE_scan_node")))],
        )
    };
    assert_rc(&scan(&["--self-test"]), 0, "--self-test");
    assert_rc(&scan(&["--p1-self-check"]), 0, "--p1-self-check");
    let out = scan(&["--help"]);
    assert_rc(&out, 0, "--help");
    let help = text(&out.stdout);
    for opt in [
        "--topology",
        "--peer-ip",
        "--peer-port",
        "--peer-ssh",
        "--compose",
        "--env-interconnect",
        "--bandwidth-gbps",
        "--per-port-gbps",
        "--per-port-floor",
        "--bw-floor",
        "--emit-manifest",
        "--manifest",
        "--check-egress",
        "--sub-work-dir",
        "--model-source",
        "--emit-sub-manifest",
        "--self-test",
    ] {
        assert!(help.contains(opt), "scan_node option {opt}");
    }
}

/// A throwaway git repository on `branch` with one tracked `*.topology.md` layer file declaring `layer`.
fn parity_repo(s: &Scratch, branch: &str, layer: &str) -> PathBuf {
    let repo = s.0.join("repo");
    write(&repo.join(".claude/rules/strategy.topology.md"), &format!("# strategy\n\n**topology: {layer}** · layer: topology\n"));
    let git = |args: &[&str]| {
        let mut cmd = Command::new("git");
        cmd.args(["-c", "user.name=contract", "-c", "user.email=contract@example.invalid", "-c", "commit.gpgsign=false", "-c", "core.hooksPath=/dev/null"])
            .args(args)
            .current_dir(&repo);
        let out = clean(cmd).output().unwrap();
        assert!(out.status.success(), "git {args:?}: {}", text(&out.stderr));
    };
    git(&["init", "-q"]);
    git(&["symbolic-ref", "HEAD", &format!("refs/heads/{branch}")]);
    git(&["add", ".claude/rules/strategy.topology.md"]);
    git(&["commit", "-q", "-m", "layer"]);
    repo
}

/// `topology_parity.sh evaluate …` run directly (no `bash`), as closing_sequence.sh and sync_to_sub.sh do.
fn parity(cwd: &Path, args: &[&OsStr]) -> Output {
    let mut cmd = Command::new(root().join(TOPOLOGY_PARITY_SH));
    cmd.args(args).current_dir(cwd);
    clean(cmd).env("TOPOLOGY_PARITY_BIN", env!("CARGO_BIN_EXE_topology_parity")).output().unwrap()
}

fn parity_args<'a>(repo: &'a Path, extra: &[&'a str]) -> Vec<&'a OsStr> {
    let mut args = vec![OsStr::new("evaluate"), OsStr::new("--repo"), repo.as_os_str()];
    args.extend(extra.iter().map(|a| OsStr::new(*a)));
    args
}

/// The JSON verdict: keys and types sync_to_sub.sh (`inline_json parity-summary`) reads.
fn parity_verdict(out: &Output) -> Json {
    let doc = stdout_json(out);
    assert!(["PASS", "RED"].contains(&doc.at("verdict").str().unwrap()), "{doc:?}");
    assert!(["string", "null"].contains(&doc.at("topology").kind()));
    doc.at("reason_codes").strs();
    doc.at("reasons").strs();
    doc.at("legs_judged").strs();
    assert_eq!(doc.at("legs_checked").kind(), "number");
    let rc = out.status.code().unwrap();
    assert_eq!(doc.at("verdict").str() == Some("PASS"), rc == 0, "verdict PASS iff exit 0");
    doc
}

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

/// `evaluate --repo --expect --format json|value` (sync_to_sub.sh, closing_sequence.sh): PASS is exit 0 with `verdict`
/// PASS, the topology, `legs_checked`/`legs_judged` naming the legs read, empty `reason_codes`; a contract violation is
/// exit 5 with `verdict` RED and the reason codes (`EXPECT_MISMATCH`, `MANIFEST_MISMATCH`, `BRANCH_UNKNOWN`); `--format
/// value` prints the bare topology line on PASS and nothing on stdout on RED (the reason on stderr); usage is exit 2.
#[test]
fn topology_parity_evaluate_json_value_and_exit_codes() {
    let s = scratch("parity");
    let repo = parity_repo(&s, "single-node", "single");

    let out = parity(&s.0, &parity_args(&repo, &["--format", "json"]));
    assert_rc(&out, 0, "two legs");
    assert!(out.stderr.is_empty(), "diagnostics only on stderr; none on PASS");
    let doc = parity_verdict(&out);
    assert_eq!(doc.at("verdict").str(), Some("PASS"));
    assert_eq!(doc.at("topology").str(), Some("single"));
    assert_eq!(num(doc.at("legs_checked")), "2");
    assert_eq!(sorted(doc.at("legs_judged").strs()), ["branch", "layer"]);
    assert!(doc.at("reason_codes").strs().is_empty());
    assert_eq!(stdout_json(&parity(&s.0, &parity_args(&repo, &[]))), doc, "json is the default format");

    let out = parity(&s.0, &parity_args(&repo, &["--format", "value"]));
    assert_rc(&out, 0, "value");
    assert_eq!(text(&out.stdout), "single\n");

    let out = parity(&s.0, &parity_args(&repo, &["--expect", "multi", "--format", "json"]));
    assert_rc(&out, 5, "--expect multi on single-node");
    let doc = parity_verdict(&out);
    assert_eq!(doc.at("verdict").str(), Some("RED"));
    assert!(doc.at("reason_codes").strs().contains(&"EXPECT_MISMATCH".to_string()), "{doc:?}");

    write(&repo.join("output/single/manifest.yaml"), "topology: single\ngpus_per_node: 1\n");
    let out = parity(&s.0, &parity_args(&repo, &["--expect", "single", "--format", "json"]));
    assert_rc(&out, 0, "three legs");
    let doc = parity_verdict(&out);
    assert_eq!(num(doc.at("legs_checked")), "3");
    assert_eq!(sorted(doc.at("legs_judged").strs()), ["branch", "layer", "manifest"]);

    write(&repo.join("output/single/manifest.yaml"), "topology: multi\n");
    let out = parity(&s.0, &parity_args(&repo, &["--format", "json"]));
    assert_rc(&out, 5, "manifest mismatch");
    let doc = parity_verdict(&out);
    assert!(doc.at("reason_codes").strs().contains(&"MANIFEST_MISMATCH".to_string()), "{doc:?}");
    assert!(!doc.at("reasons").strs().is_empty());
    let out = parity(&s.0, &parity_args(&repo, &["--format", "value"]));
    assert_rc(&out, 5, "value on RED");
    assert!(out.stdout.is_empty(), "no topology line on RED");
    assert!(!out.stderr.is_empty());
    std::fs::remove_dir_all(repo.join("output")).unwrap();

    let mut cmd = Command::new("git");
    cmd.args(["checkout", "-q", "-b", "elsewhere"]).current_dir(&repo);
    assert!(clean(cmd).output().unwrap().status.success());
    let out = parity(&s.0, &parity_args(&repo, &["--format", "json"]));
    assert_rc(&out, 5, "unknown branch");
    let doc = parity_verdict(&out);
    assert_eq!(doc.at("reason_codes").strs(), ["BRANCH_UNKNOWN"]);
    assert_eq!(doc.at("topology"), &Json::Null);

    for bad in [&b"triple"[..], &b"sin\xffgle"[..]] {
        let mut args = parity_args(&repo, &["--expect"]);
        args.push(OsStr::from_bytes(bad));
        let out = parity(&s.0, &args);
        assert_rc(&out, 2, "--expect outside the choices");
        assert!(out.stdout.is_empty());
    }
    let out = parity(&s.0, &parity_args(&repo, &["--format", "yaml"]));
    assert_rc(&out, 2, "--format outside the choices");
    assert!(out.stdout.is_empty());
}

/// The fourth leg (the active campaign's `control_variables.topology`, read when the campaign writer's launcher is in the
/// checkout): a multi-node checkout agreeing on all four legs is PASS with `legs_checked` 4 and `--format value` = `multi`;
/// a campaign declared for the other topology is exit 5 with `CAMPAIGN_MISMATCH`.
#[test]
fn topology_parity_four_legs_and_campaign_mismatch() {
    let s = scratch("parity4");
    let repo = parity_repo(&s, "multi-node", "multi");
    write(&repo.join("output/multi/manifest.yaml"), "topology: multi\n");
    copy_file(&root().join(TS).join("campaign_init.sh"), &repo.join(TS).join("campaign_init.sh"));
    write(&repo.join("campaigns/ACTIVE"), "c1\n");
    let decl = |t: &str| {
        format!(r#"{{"id": "c1", "control_variables": {{"topology": "{t}"}}, "nodes": [{{"node_id": "main", "topology": "{t}"}}]}}"#)
    };
    write(&repo.join("campaigns/c1/campaign.yaml"), &decl("multi"));
    let out = parity(&s.0, &parity_args(&repo, &["--expect", "multi", "--format", "json"]));
    assert_rc(&out, 0, "four legs");
    let doc = parity_verdict(&out);
    assert_eq!(doc.at("topology").str(), Some("multi"));
    assert_eq!(num(doc.at("legs_checked")), "4");
    assert_eq!(sorted(doc.at("legs_judged").strs()), ["branch", "campaign", "layer", "manifest"]);
    let out = parity(&s.0, &parity_args(&repo, &["--format", "value"]));
    assert_rc(&out, 0, "value");
    assert_eq!(text(&out.stdout), "multi\n");

    write(&repo.join("campaigns/c1/campaign.yaml"), &decl("single"));
    let out = parity(&s.0, &parity_args(&repo, &["--format", "json"]));
    assert_rc(&out, 5, "campaign mismatch");
    let doc = parity_verdict(&out);
    assert!(doc.at("reason_codes").strs().contains(&"CAMPAIGN_MISMATCH".to_string()), "{doc:?}");
}

/// A scratch checkout for the campaign writer and the template validator: the two launchers (their location names the
/// checkout root), the tracked campaign skeleton, the schema engine's launcher the declaration validator asks for, and a
/// single-node manifest.
struct Checkout {
    _s: Scratch,
    repo: PathBuf,
}

const DECL: &str = r#"{"schema_version": 1, "id": "c-test", "plan_ref": "docs/plan/p.md", "declared_utc": "2026-11-18T00:00:00Z",
 "nodes": [{"node_id": "main", "role": "main", "topology": "single", "hw": "gpu"}, {"node_id": "sub1", "role": "sub", "topology": "single", "hw": "gpu"}],
 "matrix": {"versions": ["0.31.0"], "models": ["m"]},
 "assignments": {"main": [{"cell": "c1", "mode": "AUTO"}], "sub1": [{"cell": "c2", "mode": "AUTO"}, {"cell": "c3", "mode": "AUTO"}]},
 "budgets": {"smoke_budget_overhead_mib": 1, "ready_max_seconds": 1, "repeats": 3},
 "control_variables": {"model": "m", "vllm_version": "0.31.0", "topology": "single", "target_gpu": "g"},
 "revisions": [], "hint_targets": [], "topology_sections": {"single": {}, "multi": {}}}
"#;

impl Checkout {
    fn new(tag: &str) -> Checkout {
        let s = scratch(tag);
        let repo = s.0.join("repo");
        let real = root();
        for name in ["campaign_init.sh", "campaign_template_validator.sh"] {
            copy_file(&real.join(TS).join(name), &repo.join(TS).join(name));
        }
        copy_tree(&real.join("campaigns/_template"), &repo.join("campaigns/_template"));
        copy_file(&real.join("campaigns/README.md"), &repo.join("campaigns/README.md"));
        let gate = ".claude/policies/runtime/completion_gate.sh";
        copy_file(&real.join(gate), &repo.join(gate));
        write(&repo.join("output/single/manifest.yaml"), "topology: single\n");
        Checkout { _s: s, repo }
    }
    fn ci_os(&self, args: &[&OsStr], env: &[(&str, &OsStr)]) -> Output {
        let mut all = vec![("CAMPAIGN_READ_BIN", OsStr::new(env!("CARGO_BIN_EXE_campaign_init")))];
        all.extend_from_slice(env);
        bash(&self.repo.join(TS).join("campaign_init.sh"), args, &self.repo, &all)
    }
    /// `bash campaign_init.sh <args>` in the checkout.
    fn ci(&self, args: &[&str]) -> Output {
        self.ci_os(&os(args), &[])
    }
    fn ok(&self, args: &[&str]) -> Output {
        let out = self.ci(args);
        assert_rc(&out, 0, &format!("campaign_init {args:?}"));
        out
    }
    fn validator(&self, args: &[&str]) -> Output {
        bash(
            &self.repo.join(TS).join("campaign_template_validator.sh"),
            &os(args),
            &self.repo,
            &[("CAMPAIGN_VALIDATOR_BIN", OsStr::new(env!("CARGO_BIN_EXE_campaign_template_validator")))],
        )
    }
    /// Opens `c-test` (`--init --apply` from the skeleton) and fills its declaration: main → c1, sub1 → c2, c3.
    fn open(&self) {
        self.ok(&["--init", "c-test", "--plan-ref", "docs/plan/p.md", "--utc", UTC, "--approved-by", "contract", "--apply"]);
        write(&self.repo.join("campaigns/c-test/campaign.yaml"), DECL);
    }
    fn path(&self, rel: &str) -> String {
        self.repo.join(rel).to_str().unwrap().to_string()
    }
    fn json_file(&self, rel: &str) -> Json {
        let body = std::fs::read_to_string(self.repo.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"));
        Json::parse(&body).unwrap_or_else(|e| panic!("{rel} is not JSON ({e}): {body}"))
    }
}

/// One stdout line (the shell consumers capture it with `$(…)`), nothing on stderr.
fn one_line(out: &Output) -> String {
    let so = text(&out.stdout);
    assert!(out.stderr.is_empty(), "stderr: {}", text(&out.stderr));
    let line = so.strip_suffix('\n').unwrap_or_else(|| panic!("no line: {so:?}"));
    assert!(!line.contains('\n'), "{so:?}");
    line.to_string()
}

/// The read shapes the scripts capture (broad_search.sh, sweep_bench.sh, fetch_sub_docs.sh, single_serve_up.sh,
/// run_trial_session.rs): `--active` prints the active id (`_bootstrap` when none); `--assigned-cells <node>` the
/// comma-separated cells; `--derive sweep|cell-status|config|lockset` one absolute path; `--lockset-precheck --cell`
/// one JSON object (`status` not_applicable|passed = exit 0, refused = exit 2, `campaign_id`, `cell`, `lockset`,
/// `provenance`, `reason`); `--grounding-check` exit 0 without an active campaign and non-zero (the callers' STOP) when
/// the active campaign has no grounding record; `--lifecycle` JSON `line` + `campaigns`.
#[test]
fn campaign_init_read_shapes_and_lockset_precheck() {
    let c = Checkout::new("ci-read");
    assert_eq!(one_line(&c.ok(&["--active"])), "_bootstrap");
    let out = c.ok(&["--lockset-precheck", "--cell", "c1"]);
    let doc = stdout_json(&out);
    assert_eq!(doc.at("status").str(), Some("not_applicable"));
    assert_eq!(doc.at("campaign_id").str(), Some("_bootstrap"));
    assert_eq!(doc.at("cell").str(), Some("c1"));
    assert_eq!(doc.at("lockset"), &Json::Null);
    assert_eq!(doc.at("reason").kind(), "string");
    c.ok(&["--grounding-check"]);
    let doc = stdout_json(&c.ok(&["--lifecycle"]));
    assert_eq!(doc.at("line").kind(), "string");
    assert_eq!(doc.at("campaigns").kind(), "object");

    c.open();
    assert_eq!(one_line(&c.ok(&["--active"])), "c-test");
    assert_eq!(one_line(&c.ok(&["--assigned-cells", "main"])), "c1");
    assert_eq!(one_line(&c.ok(&["--assigned-cells", "sub1"])), "c2,c3");
    let sweep = c.path("campaigns/c-test/sweeps/s1.json");
    assert_eq!(one_line(&c.ok(&["--derive", "sweep", "--sweep", "s1"])), sweep);
    assert_eq!(one_line(&c.ok(&["--derive", "sweep", "--sweep", "s1", "--campaign-id", "c-test"])), sweep);
    for (kind, file) in [("cell-status", "cell.status.json"), ("config", "config.yaml"), ("lockset", "lockset.json")] {
        let line = one_line(&c.ok(&["--derive", kind, "--cell", "c1"]));
        assert_eq!(line, c.path(&format!("campaigns/c-test/cells/c1/{file}")), "--derive {kind}");
    }

    for extra in [&[][..], &["--campaign-id", "c-test"]] {
        let mut args = vec!["--lockset-precheck", "--cell", "c1"];
        args.extend_from_slice(extra);
        let out = c.ci(&args);
        assert_rc(&out, 2, "lockset absent");
        assert!(!out.stderr.is_empty(), "the refusal reason is on stderr too");
        let doc = stdout_json(&out);
        assert_eq!(doc.at("status").str(), Some("refused"));
        assert_eq!(doc.at("campaign_id").str(), Some("c-test"));
        assert_eq!(doc.at("lockset").str(), Some("campaigns/c-test/cells/c1/lockset.json"));
        assert_eq!(doc.at("provenance"), &Json::Null);
        assert_eq!(doc.at("reason").kind(), "string");
    }
    write(&c.repo.join("campaigns/c-test/cells/c1/lockset.json"), r#"{"provenance": "hand-authored"}"#);
    let doc = stdout_json(&c.ok(&["--lockset-precheck", "--cell", "c1"]));
    assert_eq!(doc.at("status").str(), Some("passed"));
    assert_eq!(doc.at("provenance").str(), Some("hand-authored"));
    assert_eq!(doc.at("reason"), &Json::Null);

    let out = c.ci(&["--grounding-check"]);
    assert_ne!(out.status.code(), Some(0), "an active campaign without a grounding record stops the caller");
}

/// The writers the phase scripts and hint-publisher call (`--phase-set build|serve|bench|publish`, `--cell-set`,
/// `--escalation-add`, `--write-brief`) are exit 0 and write the status, cell, candidate and brief files other readers
/// parse (L2 keys); a refused write is non-zero. The relay shapes (terraforming `relay/core.rs`): `--emit-slice <node>
/// --campaign-id --utc` prints the node's slice (`id`, `plan_ref`, only that node's `assignments`, `self_role`,
/// `_assignment_revision` null or `{rev}` after `--assign-set`); `--lifecycle <id>` prints `campaign_id`, `state`,
/// `close_reason`. The lifecycle self-test reads `--resume-brief` (first line `상태: `) and `--verify-purge-gate`
/// (non-zero, reason lines `  - `).
#[test]
fn campaign_init_writers_relay_and_brief_shapes() {
    let c = Checkout::new("ci-write");
    c.open();
    c.ok(&[
        "--phase-set", "bench", "--node", "main", "--cell", "c1", "--state", "running", "--proof-predicate", "p",
        "--proof-source", "s", "--started-utc", UTC, "--authored-by", "main",
    ]);
    let doc = c.json_file("campaigns/c-test/phases/main/bench.status.json");
    assert_eq!(doc.at("phase").str(), Some("bench"));
    assert_eq!(doc.at("state").str(), Some("running"));
    assert_eq!(doc.at("node_id").str(), Some("main"));
    assert_eq!(doc.at("cell_id").str(), Some("c1"));
    assert_eq!(doc.at("proof").at("ok"), &Json::Bool(false));
    c.ok(&[
        "--campaign-id", "c-test", "--phase-set", "publish", "--node", "main", "--cell", "c1", "--state", "done",
        "--proof-predicate", "p", "--proof-ok", "--proof-source", "s", "--ended-utc", UTC, "--authored-by", "main",
    ]);
    let doc = c.json_file("campaigns/c-test/phases/main/publish.status.json");
    assert_eq!(doc.at("state").str(), Some("done"));
    assert_eq!(doc.at("proof").at("ok"), &Json::Bool(true));
    for (phase, state, proof) in [("serve", "done", &["--proof-ok"][..]), ("build", "failed", &[])] {
        let mut args = vec![
            "--phase-set", phase, "--node", "main", "--state", state, "--cell", "c1", "--proof-predicate", "p",
            "--proof-source", "s", "--started-utc", UTC, "--ended-utc", UTC, "--authored-by", "main",
        ];
        args.extend_from_slice(proof);
        c.ok(&args);
        assert_eq!(c.json_file(&format!("campaigns/c-test/phases/main/{phase}.status.json")).at("state").str(), Some(state));
    }
    let out = c.ci(&[
        "--phase-set", "bench", "--node", "main", "--cell", "c1", "--state", "unknown-state", "--proof-predicate", "p",
        "--proof-source", "s", "--started-utc", UTC,
    ]);
    assert_ne!(out.status.code(), Some(0), "a state outside the enum is refused");

    c.ok(&["--cell-set", "c1", "--outcome", "measurement_void", "--utc", UTC]);
    let doc = c.json_file("campaigns/c-test/cells/c1/cell.status.json");
    assert_eq!(doc.at("cell_id").str(), Some("c1"));
    assert_eq!(doc.at("cell_outcome").str(), Some("measurement_void"));

    c.ok(&[
        "--escalation-add", "--cell", "c1", "--node", "main", "--utc", UTC, "--window-start-utc", UTC, "--window-end-utc", UTC,
        "--symptom", "contract",
    ]);
    let lines = std::fs::read_to_string(c.repo.join("campaigns/c-test/escalation_candidates.jsonl")).unwrap();
    assert_eq!(lines.lines().count(), 1);
    assert_eq!(Json::parse(lines.lines().next().unwrap()).unwrap().kind(), "object");

    c.ok(&["--write-brief", "--node", "main", "--utc", UTC]);
    let brief = c.json_file("docs/logs/main/campaign_brief.json");
    assert_eq!(brief.at("campaign_id").str(), Some("c-test"));
    assert_eq!(brief.at("generated_utc").str(), Some(UTC));
    let cells = brief.at("cells").arr().unwrap();
    assert_eq!(cells[0].at("cell_id").str(), Some("c1"));

    let slice = |c: &Checkout| {
        let out = c.ok(&["--emit-slice", "sub1", "--campaign-id", "c-test", "--utc", UTC]);
        assert!(out.stderr.is_empty());
        stdout_json(&out)
    };
    let doc = slice(&c);
    assert_eq!(doc.at("id").str(), Some("c-test"));
    assert_eq!(doc.at("plan_ref").str(), Some("docs/plan/p.md"));
    assert_eq!(doc.at("assignments").keys(), ["sub1"]);
    assert_eq!(doc.at("self_role").str(), Some("sub"));
    assert_eq!(doc.at("_assignment_revision"), &Json::Null);
    for node in doc.at("nodes").arr().unwrap() {
        assert_eq!(node.at("node_id").str(), Some("sub1"));
    }
    c.ok(&[
        "--assign-set", "sub1", "--assign-cells", "c2", "--assign-reason", "r", "--assign-evidence", "e",
        "--assign-approved-by", "contract", "--utc", UTC, "--apply",
    ]);
    let doc = slice(&c);
    assert_eq!(doc.at("_assignment_revision").at("rev").kind(), "number");

    let doc = stdout_json(&c.ok(&["--lifecycle", "c-test"]));
    assert_eq!(doc.at("campaign_id").str(), Some("c-test"));
    assert_eq!(doc.at("state").str(), Some("active"));
    assert_eq!(doc.at("close_reason"), &Json::Null);
    assert!(doc.get("sub_convergence").is_some());
    let doc = stdout_json(&c.ok(&["--lifecycle"]));
    assert!(doc.at("line").str().unwrap().starts_with("상태: "));
    assert_eq!(doc.at("campaigns").at("c-test").at("state").str(), Some("active"));

    let out = c.ok(&["--resume-brief"]);
    assert!(text(&out.stdout).lines().next().unwrap().starts_with("상태: "), "{}", text(&out.stdout));
    let out = c.ci(&["--verify-purge-gate", "c-test"]);
    assert_ne!(out.status.code(), Some(0), "purge gate closed without evidence");
    let merged = format!("{}{}", text(&out.stdout), text(&out.stderr));
    assert!(merged.lines().any(|l| l.starts_with("  - ")), "{merged}");
}

/// Usage errors are exit 2 with nothing on stdout (a non-UTF-8 argument included); policy-distribution `checks.rs`
/// needs `--selftest` exit 0; the long options the scripts, the relay, hint-publisher and the documents name are part
/// of the parser (`--help` exit 0 lists them).
#[test]
fn campaign_init_usage_options_and_selftest() {
    let c = Checkout::new("ci-usage");
    for args in [vec![OsStr::new("--bogus")], vec![OsStr::new("--active"), OsStr::from_bytes(b"\xff")]] {
        let out = c.ci_os(&args, &[]);
        assert_rc(&out, 2, "usage error");
        assert!(out.stdout.is_empty());
    }
    assert_rc(&c.ci(&["--selftest"]), 0, "--selftest");
    let out = c.ok(&["--help"]);
    let help = text(&out.stdout);
    for opt in [
        "--active", "--derive", "--cell", "--sweep", "--campaign-id", "--verify-purge-gate", "--close", "--init", "--plan-ref",
        "--from-slice", "--ground", "--grounding-check", "--lockset-precheck", "--assigned-cells", "--emit-slice", "--out",
        "--import-sub", "--write-brief", "--apply", "--selftest", "--suspend-current", "--derived-from", "--preempt",
        "--release", "--suspend", "--resume-campaign", "--accept-drift", "--abandon", "--reason", "--summary-testlog",
        "--answer-query", "--sub-echo", "--control", "--lifecycle", "--phase-set", "--node", "--state", "--proof-predicate",
        "--proof-ok", "--proof-source", "--started-utc", "--ended-utc", "--cell-set", "--cell-materialize", "--outcome",
        "--sweep-state", "--escalation-add", "--symptom", "--events", "--window-start-utc", "--window-end-utc",
        "--authored-by", "--void-reason", "--void-reason-source", "--lite-raw", "--reentry-decide", "--decision", "--route",
        "--axis-citation", "--next-intent", "--evidence-add", "--kind", "--path", "--assign-set", "--assign-cells",
        "--assign-reason", "--assign-evidence", "--assign-approved-by", "--slice-update", "--revise", "--hint-approve",
        "--cells", "--approved-by", "--source", "--utc", "--resume-brief",
    ] {
        assert!(help.contains(opt), "campaign_init option {opt}");
    }
}

/// campaign_template_validator (policy-distribution `checks.rs` `--selftest`, fetch_sub_docs.sh `--acceptance`, the
/// documents' `--campaign`·`--instance`·`--template`): exit 0 = pass, 1 = violations (a placeholder skeleton, a missing
/// declaration, an instance whose acceptance predicates fail), 2 = usage; `--acceptance` judges P4·P5·P6 only (exit
/// 0 once the main phase carries a time and the cell its provenance; fetch_sub_docs.sh only shows the result).
#[test]
fn campaign_template_validator_exit_codes() {
    let c = Checkout::new("validator");
    assert_rc(&c.validator(&["--selftest"]), 0, "--selftest");
    assert_rc(&c.validator(&["--template"]), 0, "--template");
    assert_rc(&c.validator(&["--campaign", "campaigns/_template/campaign.yaml"]), 1, "placeholders");
    assert_rc(&c.validator(&["--campaign", "campaigns/absent/campaign.yaml"]), 1, "missing declaration");
    c.open();
    // A main-only declaration: a sub's acceptance (P4) needs the relay ledger of a delegated build, out of this check.
    let main_only = DECL
        .replace(r#", {"node_id": "sub1", "role": "sub", "topology": "single", "hw": "gpu"}"#, "")
        .replace(r#", "sub1": [{"cell": "c2", "mode": "AUTO"}, {"cell": "c3", "mode": "AUTO"}]"#, "");
    assert!(!main_only.contains("sub1"));
    write(&c.repo.join("campaigns/c-test/campaign.yaml"), &main_only);
    write(&c.repo.join("campaigns/c-test/cells/c1/config.yaml"), "{}\n");
    assert_rc(&c.validator(&["--campaign", "campaigns/c-test/campaign.yaml"]), 0, "a filled declaration");
    assert_rc(&c.validator(&["--acceptance", "campaigns/c-test"]), 1, "--acceptance: no phase time, no cell provenance");
    write(&c.repo.join("campaigns/c-test/cells/c1/lockset.json"), r#"{"provenance": "hand-authored"}"#);
    c.ok(&[
        "--phase-set", "bench", "--node", "main", "--cell", "c1", "--state", "running", "--proof-predicate", "p",
        "--proof-source", "s", "--started-utc", UTC, "--authored-by", "main",
    ]);
    assert_rc(&c.validator(&["--acceptance", "campaigns/c-test"]), 0, "--acceptance");
    let out = c.validator(&["--bogus"]);
    assert_rc(&out, 2, "usage");
    assert!(out.stdout.is_empty());
    let out = c.validator(&["--help"]);
    assert_rc(&out, 0, "--help");
    let help = text(&out.stdout);
    for opt in ["--campaign", "--instance", "--template", "--acceptance", "--selftest"] {
        assert!(help.contains(opt), "campaign_template_validator option {opt}");
    }
}

/// node_identity_resolve, the private helper of the node-identity lookup (`--repo <repo>`): exit 0 with the node id as
/// one stdout line, 1 = fail-loud (nothing on stdout, the reason on stderr), 2 = its own usage error.
#[test]
fn node_identity_resolve_private_interface() {
    let s = scratch("node-identity");
    let run = |args: &[&OsStr]| {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_node_identity_resolve"));
        cmd.args(args).current_dir(&s.0);
        clean(cmd).output().unwrap()
    };
    let out = run(&[]);
    assert_rc(&out, 2, "no --repo");
    assert!(out.stdout.is_empty());
    let empty = s.0.join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    let out = run(&[OsStr::new("--repo"), empty.as_os_str()]);
    assert_rc(&out, 1, "unresolvable");
    assert!(out.stdout.is_empty() && !out.stderr.is_empty());
    let sub = s.0.join("sub");
    write(&sub.join("output/single/manifest.yaml"), "topology: single\nself_role: sub\n");
    let out = run(&[OsStr::new("--repo"), sub.as_os_str()]);
    assert_rc(&out, 0, "resolved");
    let so = text(&out.stdout);
    assert!(so.ends_with('\n') && so.trim_end().lines().count() == 1 && !so.trim().is_empty(), "{so:?}");
}

/// A small RFC 8259 reader for the contract checks: the fixed surface is the parsed value (L2), so the checks read the
/// output as JSON instead of comparing bytes. Numbers keep their source text; no dependency of the crate is used.
mod json {
    // Each crate's checks use a different part of this shared reader.
    #![allow(dead_code)]

    #[derive(Debug, Clone, PartialEq)]
    pub enum Json {
        Null,
        Bool(bool),
        Num(String),
        Str(String),
        Arr(Vec<Json>),
        Obj(Vec<(String, Json)>),
    }

    impl Json {
        pub fn parse(text: &str) -> Result<Json, String> {
            let mut r = Reader { b: text.as_bytes(), i: 0 };
            let v = r.value()?;
            r.ws();
            if r.i != r.b.len() {
                return Err(format!("text after the JSON value at byte {}", r.i));
            }
            Ok(v)
        }
        pub fn get(&self, key: &str) -> Option<&Json> {
            match self {
                Json::Obj(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
                _ => None,
            }
        }
        /// `self[key]`, failing the check when the key is absent.
        pub fn at(&self, key: &str) -> &Json {
            self.get(key).unwrap_or_else(|| panic!("key {key:?} missing in {self:?}"))
        }
        pub fn keys(&self) -> Vec<&str> {
            match self {
                Json::Obj(m) => m.iter().map(|(k, _)| k.as_str()).collect(),
                _ => Vec::new(),
            }
        }
        pub fn str(&self) -> Option<&str> {
            match self {
                Json::Str(s) => Some(s),
                _ => None,
            }
        }
        pub fn arr(&self) -> Option<&[Json]> {
            match self {
                Json::Arr(a) => Some(a),
                _ => None,
            }
        }
        /// The JSON type name: null, bool, number, string, array, object.
        pub fn kind(&self) -> &'static str {
            match self {
                Json::Null => "null",
                Json::Bool(_) => "bool",
                Json::Num(_) => "number",
                Json::Str(_) => "string",
                Json::Arr(_) => "array",
                Json::Obj(_) => "object",
            }
        }
        /// A list of strings (e.g. `reason_codes`), failing the check on any other shape.
        pub fn strs(&self) -> Vec<String> {
            let items = self.arr().unwrap_or_else(|| panic!("not an array: {self:?}"));
            items.iter().map(|v| v.str().unwrap_or_else(|| panic!("not a string: {v:?}")).to_string()).collect()
        }
    }

    struct Reader<'a> {
        b: &'a [u8],
        i: usize,
    }

    impl Reader<'_> {
        fn ws(&mut self) {
            while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\n' | b'\r') {
                self.i += 1;
            }
        }
        fn eat(&mut self, lit: &str) -> Result<(), String> {
            if self.b[self.i..].starts_with(lit.as_bytes()) {
                self.i += lit.len();
                Ok(())
            } else {
                Err(format!("expected {lit:?} at byte {}", self.i))
            }
        }
        fn value(&mut self) -> Result<Json, String> {
            self.ws();
            match self.b.get(self.i) {
                None => Err("unexpected end of input".into()),
                Some(b'{') => {
                    self.i += 1;
                    let mut m = Vec::new();
                    self.ws();
                    if self.b.get(self.i) == Some(&b'}') {
                        self.i += 1;
                        return Ok(Json::Obj(m));
                    }
                    loop {
                        self.ws();
                        let k = self.string()?;
                        self.ws();
                        self.eat(":")?;
                        let v = self.value()?;
                        m.push((k, v));
                        self.ws();
                        match self.b.get(self.i) {
                            Some(b',') => self.i += 1,
                            Some(b'}') => {
                                self.i += 1;
                                return Ok(Json::Obj(m));
                            }
                            _ => return Err(format!("expected ',' or '}}' at byte {}", self.i)),
                        }
                    }
                }
                Some(b'[') => {
                    self.i += 1;
                    let mut a = Vec::new();
                    self.ws();
                    if self.b.get(self.i) == Some(&b']') {
                        self.i += 1;
                        return Ok(Json::Arr(a));
                    }
                    loop {
                        a.push(self.value()?);
                        self.ws();
                        match self.b.get(self.i) {
                            Some(b',') => self.i += 1,
                            Some(b']') => {
                                self.i += 1;
                                return Ok(Json::Arr(a));
                            }
                            _ => return Err(format!("expected ',' or ']' at byte {}", self.i)),
                        }
                    }
                }
                Some(b'"') => Ok(Json::Str(self.string()?)),
                Some(b't') => self.eat("true").map(|_| Json::Bool(true)),
                Some(b'f') => self.eat("false").map(|_| Json::Bool(false)),
                Some(b'n') => self.eat("null").map(|_| Json::Null),
                Some(c) if *c == b'-' || c.is_ascii_digit() => {
                    let start = self.i;
                    while self.i < self.b.len() && matches!(self.b[self.i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
                        self.i += 1;
                    }
                    let t = std::str::from_utf8(&self.b[start..self.i]).unwrap().to_string();
                    t.parse::<f64>().map_err(|e| format!("bad number {t:?}: {e}"))?;
                    Ok(Json::Num(t))
                }
                Some(c) => Err(format!("unexpected byte {c:#x} at {}", self.i)),
            }
        }
        fn hex4(&mut self) -> Result<u32, String> {
            let h = self.b.get(self.i..self.i + 4).ok_or("short \\u escape")?;
            let v = u32::from_str_radix(std::str::from_utf8(h).map_err(|e| e.to_string())?, 16).map_err(|e| e.to_string())?;
            self.i += 4;
            Ok(v)
        }
        fn string(&mut self) -> Result<String, String> {
            self.eat("\"")?;
            let mut out = Vec::new();
            loop {
                let c = *self.b.get(self.i).ok_or("unterminated string")?;
                self.i += 1;
                match c {
                    b'"' => return String::from_utf8(out).map_err(|e| e.to_string()),
                    b'\\' => {
                        let e = *self.b.get(self.i).ok_or("unterminated escape")?;
                        self.i += 1;
                        let ch = match e {
                            b'"' => '"',
                            b'\\' => '\\',
                            b'/' => '/',
                            b'b' => '\u{8}',
                            b'f' => '\u{c}',
                            b'n' => '\n',
                            b'r' => '\r',
                            b't' => '\t',
                            b'u' => {
                                let hi = self.hex4()?;
                                let code = if (0xD800..0xDC00).contains(&hi) {
                                    self.eat("\\u")?;
                                    let lo = self.hex4()?;
                                    0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)
                                } else {
                                    hi
                                };
                                char::from_u32(code).ok_or("lone surrogate")?
                            }
                            _ => return Err(format!("bad escape at byte {}", self.i)),
                        };
                        let mut buf = [0u8; 4];
                        out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                    }
                    c if c < 0x20 => return Err(format!("control byte in string at {}", self.i)),
                    c => out.push(c),
                }
            }
        }
    }
}
