//! Contract checks of the terraforming bins (plan 26111805 「보존 계약」, cell c21b): what their consumers read
//! (runtime-support/contract/consumers.tsv L1·L2 rows of this crate) —
//! - every consumer starts a bin through its launcher (`scripts/*.sh`, `scripts/node_blackbox/*.sh`); the launchers honour
//!   `TERRAFORMING_BIN_DIR`, export `TERRAFORMING_OWNER_PATH` (their own path) and refuse an absent executable with exit 2;
//!   the skill documents name subcommands and long options;
//! - policy-distribution `checks.rs`, policy-selftest `suite.rs` and `verify_node_blackbox.sh` run the self-tests and the
//!   self-test drivers and need exit 0;
//! - run_bench.sh·budget_renew_loop.sh·single_serve_up.sh·single_serve_down.sh·multinode_serve_smoke.sh·
//!   native_multinode.rs·recipe `run_trial_session.rs`·mem_watchdog_eta.sh: the `blackbox_session` budget subcommands,
//!   `budget-defaults --field ttl_s` digits, `budget-status` JSON (`label` read by `sed`), renew exit 3, `serve_budget.env`;
//! - install_node_blackbox.sh·mem_watchdog_eta.sh·thermal_watchdog.sh·verify_node_blackbox.sh·upstream
//!   `budget_preflight.rs`·broad_search.sh: `blackbox_eta`/`blackbox_thermal` `--emit-params` files (sourced `BB_*` keys),
//!   `--explain`, `--print-uncalibrated`;
//! - upstream `branch_sync_selftest.rs` (S6): `render_sub_env` stops with exit 5 and `TOPOLOGY_RULES_MISMATCH` on stderr
//!   before any write, and reaches the manifest check (exit 3) when the rules match;
//! - multinode_serve_smoke.sh: `regen_envelope --node-dir staleness --now --max-age-days` (merged output, warning only);
//! - verify_node_blackbox.sh: `bb_capture_proven <OUT>` prints the carried proof or `null`;
//! - build_rust_packages.sh: `rust_package_check METADATA_JSON CRATE_DIR` exit 0 / 1 and its summary line.
//!
//! The bins run for real (through the launchers where the consumers do) in scratch directories. Values (L2) and exit
//! codes, option names, line shapes and keys (L1) are fixed; message texts are not (L3).
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use json::Json;

/// `.claude/skills/terraforming_node` (this crate is its `rust/`).
fn skill() -> PathBuf {
    std::fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("..")).unwrap()
}

/// The repository root (`.claude/skills/terraforming_node` → three levels up).
fn root() -> PathBuf {
    skill().join("../../..").canonicalize().unwrap()
}

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn scratch(tag: &str) -> Scratch {
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let p = std::env::temp_dir().join(format!("terraforming-contract-{tag}-{}-{n}", std::process::id()));
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

/// The launchers (relative to the skill's `scripts/`); each starts `$TERRAFORMING_BIN_DIR/<file stem>`.
const LAUNCHERS: [&str; 26] = [
    "agent_card_contract.sh",
    "agent_control.sh",
    "audit_session_ssh.sh",
    "bootstrap_canary.sh",
    "check_sub_hint_equip.sh",
    "harness_permission_probe.sh",
    "library_access.sh",
    "relay.sh",
    "render_sub_env.sh",
    "selftest_bootstrap_permissions.sh",
    "selftest_campaign_lifecycle.sh",
    "selftest_claude_native_events.sh",
    "selftest_harness_switch.sh",
    "selftest_sub_hint_write_scopes.sh",
    "staleness_gate.sh",
    "node_blackbox/adversarial_stress.sh",
    "node_blackbox/agent_guard.sh",
    "node_blackbox/blackbox_collect.sh",
    "node_blackbox/blackbox_eta.sh",
    "node_blackbox/blackbox_events.sh",
    "node_blackbox/blackbox_session.sh",
    "node_blackbox/blackbox_thermal.sh",
    "node_blackbox/logs_lifecycle.sh",
    "node_blackbox/publish_install_request.sh",
    "node_blackbox/regen_envelope.sh",
    "node_blackbox/seed_from_journal.sh",
];

/// The directory of this build's executables (`TERRAFORMING_BIN_DIR`).
fn bin_dir() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_relay")).parent().unwrap().to_path_buf()
}

fn stem(launcher: &str) -> &str {
    launcher.rsplit('/').next().unwrap().trim_end_matches(".sh")
}

/// `bash <scripts/launcher> <args>` with `TERRAFORMING_BIN_DIR` = `dir`, stdin closed.
fn launch_with(dir: &Path, cwd: &Path, launcher: &str, args: &[&str]) -> Output {
    Command::new("bash")
        .arg(skill().join("scripts").join(launcher))
        .args(args)
        .env("TERRAFORMING_BIN_DIR", dir)
        .env_remove("TERRAFORMING_OWNER_PATH")
        .current_dir(cwd)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn launch(cwd: &Path, launcher: &str, args: &[&str]) -> Output {
    launch_with(&bin_dir(), cwd, launcher, args)
}

/// `env` from `PATH`, an ELF that prints the environment it is started with (the launchers start only ELF files).
fn env_program() -> PathBuf {
    let path = std::env::var_os("PATH").unwrap();
    std::env::split_paths(&path).map(|d| d.join("env")).find(|p| p.is_file()).expect("env on PATH")
}

/// Every launcher exists; a `TERRAFORMING_BIN_DIR` without the executable is exit 2 with nothing on stdout; the launcher
/// exports `TERRAFORMING_OWNER_PATH` = its own path to the program it starts; with this build the bin starts (`--help`).
#[test]
fn launchers_bin_dir_override_and_owner_path() {
    let s = scratch("launch");
    let empty = s.0.join("empty");
    let probe = s.0.join("probe");
    std::fs::create_dir_all(&empty).unwrap();
    std::fs::create_dir_all(&probe).unwrap();
    for launcher in LAUNCHERS {
        let path = skill().join("scripts").join(launcher);
        assert!(path.is_file(), "{launcher}");
        let out = launch_with(&empty, &s.0, launcher, &["--help"]);
        assert_eq!(out.status.code(), Some(2), "{launcher}: no executable");
        assert_eq!(text(&out.stdout), "", "{launcher}");
        std::os::unix::fs::symlink(env_program(), probe.join(stem(launcher))).unwrap();
        let out = launch_with(&probe, &s.0, launcher, &[]);
        assert_eq!(out.status.code(), Some(0), "{launcher}: {}", text(&out.stderr));
        let env = text(&out.stdout);
        let owner = env.lines().find_map(|l| l.strip_prefix("TERRAFORMING_OWNER_PATH=")).unwrap_or_else(|| panic!("{launcher}: {env}"));
        assert_eq!(std::fs::canonicalize(owner).unwrap(), path.canonicalize().unwrap(), "{launcher}");
        if stem(launcher).starts_with("selftest_") {
            continue; // self-test drivers without options (run in consumer_self_tests_pass)
        }
        let out = launch(&s.0, launcher, &["--help"]);
        assert_eq!(out.status.code(), Some(0), "{launcher} --help: {}", text(&out.stderr));
    }
}

/// The subcommands and long options the documents and consumers name are part of the parser: `--help` (of the launcher
/// or the subcommand) is exit 0 and lists them.
#[test]
fn documented_subcommands_and_long_options() {
    let s = scratch("argv");
    let table: [(&str, &[&str], &[&str]); 26] = [
        ("agent_card_contract.sh", &[], &["validate", "--self-test"]),
        ("agent_card_contract.sh", &["validate"], &["--card"]),
        ("agent_control.sh", &[], &["invoke", "runners", "--self-test"]),
        ("agent_control.sh", &["invoke"], &["--dispatch-id", "--request", "--result-file"]),
        ("agent_control.sh", &["runners"], &["--provider"]),
        ("audit_session_ssh.sh", &[], &["--transcript", "--self-test"]),
        ("bootstrap_canary.sh", &[], &["--phase", "--target", "--topology", "--backend", "--self-test", "discovery", "permissions", "caller"]),
        ("check_sub_hint_equip.sh", &[], &["--clone", "--remote-url", "--write-push-authority", "--utc", "--self-test"]),
        ("library_access.sh", &[], &["--plan", "--apply", "--revoke", "--approved-by", "--node", "--topology", "--self-test"]),
        (
            "relay.sh",
            &[],
            &[
                "--apply", "--budget-source", "--continue", "--max-turns", "--resume", "--timeout-seconds", "--context-id", "--task", "--task-file",
                "--emit-only", "--runners", "--supervise-step", "--backend", "--runner-effort", "--campaign-control", "--campaign-id", "--node",
                "--orphan-sweep", "--reason", "--self-test", "suspend", "resume", "close", "abandon", "reslice",
            ],
        ),
        ("render_sub_env.sh", &[], &["--topology", "--manifest", "--out", "--tracked-list", "--sub-manifest", "--hint-write-scopes", "--self-test"]),
        ("staleness_gate.sh", &[], &["--topology", "--repo", "--observed", "--now", "--max-age-days", "--self-test"]),
        (
            "node_blackbox/blackbox_session.sh",
            &[],
            &[
                "--node-dir", "--self-test", "start", "stop", "declare-budget", "budget-status", "renew-budget", "budget-defaults", "clear-budget",
                "budget-skip", "budget-block",
            ],
        ),
        (
            "node_blackbox/blackbox_session.sh",
            &["declare-budget"],
            &["--mem-total-mib", "--weights-mib", "--kv-mib", "--overhead-mib", "--ttl-s", "--expected-load-s", "--label", "--now"],
        ),
        ("node_blackbox/blackbox_session.sh", &["renew-budget"], &["--ttl-s", "--expect-label", "--now"]),
        ("node_blackbox/blackbox_session.sh", &["budget-defaults"], &["--field"]),
        ("node_blackbox/blackbox_session.sh", &["budget-block"], &["--stage", "--reason", "--label", "--detail", "--now"]),
        ("node_blackbox/blackbox_session.sh", &["budget-skip"], &["--reason", "--label", "--now"]),
        (
            "node_blackbox/blackbox_session.sh",
            &["start"],
            &["--session-id", "--topology", "--tp", "--config-file", "--model", "--model-path", "--now", "--image"],
        ),
        ("node_blackbox/blackbox_session.sh", &["stop"], &["--session-id", "--now"]),
        ("node_blackbox/blackbox_session.sh", &["clear-budget"], &["--now"]),
        ("node_blackbox/blackbox_session.sh", &["budget-status"], &["--now"]),
        ("node_blackbox/blackbox_eta.sh", &[], &["--node-dir", "--emit-params", "--set", "--explain", "--mem-total-mib", "--self-test"]),
        ("node_blackbox/blackbox_thermal.sh", &[], &["--emit-params", "--set", "--explain", "--gpu-model", "--self-test"]),
        ("node_blackbox/regen_envelope.sh", &[], &["regen", "check-keys", "staleness", "--node-dir", "--self-test"]),
        ("node_blackbox/regen_envelope.sh", &["staleness"], &["--now", "--max-age-days"]),
    ];
    for (launcher, sub, options) in table {
        let mut args: Vec<&str> = sub.to_vec();
        args.push("--help");
        let out = launch(&s.0, launcher, &args);
        let help = text(&out.stdout);
        assert_eq!(out.status.code(), Some(0), "{launcher} {sub:?}: {}", text(&out.stderr));
        for o in options {
            assert!(help.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-')).any(|w| w == *o), "{launcher} {sub:?} lacks {o}:\n{help}");
        }
    }
    let out = launch(&s.0, "node_blackbox/blackbox_session.sh", &["--bogus-option"]);
    assert_eq!((out.status.code(), text(&out.stdout).as_str()), (Some(2), ""));
}

/// policy-distribution, policy-selftest and verify_node_blackbox.sh run these through the launchers and need exit 0.
/// (`blackbox_collect --self-test` reads the live container and GPU state of the host and is not run here.)
#[test]
fn consumer_self_tests_pass() {
    let s = scratch("selftest");
    for launcher in [
        "agent_control.sh",
        "bootstrap_canary.sh",
        "check_sub_hint_equip.sh",
        "library_access.sh",
        "relay.sh",
        "render_sub_env.sh",
        "staleness_gate.sh",
        "node_blackbox/agent_guard.sh",
        "node_blackbox/blackbox_eta.sh",
        "node_blackbox/blackbox_events.sh",
        "node_blackbox/blackbox_session.sh",
        "node_blackbox/blackbox_thermal.sh",
        "node_blackbox/logs_lifecycle.sh",
        "node_blackbox/regen_envelope.sh",
        "node_blackbox/seed_from_journal.sh",
    ] {
        let out = launch(&s.0, launcher, &["--self-test"]);
        assert_eq!(out.status.code(), Some(0), "{launcher} --self-test:\n{}{}", text(&out.stdout), text(&out.stderr));
    }
    for driver in [
        "selftest_bootstrap_permissions.sh",
        "selftest_campaign_lifecycle.sh",
        "selftest_claude_native_events.sh",
        "selftest_harness_switch.sh",
        "selftest_sub_hint_write_scopes.sh",
    ] {
        let out = launch(&s.0, driver, &[]);
        assert_eq!(out.status.code(), Some(0), "{driver}:\n{}{}", text(&out.stdout), text(&out.stderr));
    }
}

/// The `"label": *"…"` sed of budget_renew_loop.sh:145 (label charset `[A-Za-z0-9._-]{1,64}`).
fn sed_label(out: &str) -> Option<String> {
    out.lines().find_map(|l| {
        let rest = l[l.find("\"label\":")? + 8..].trim_start_matches(' ');
        let v: String = rest.strip_prefix('"')?.chars().take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')).collect();
        (rest[1 + v.len()..].starts_with('"') && (1..=64).contains(&v.len())).then_some(v)
    })
}

/// `blackbox_session --node-dir D …`, the serving-budget declaration of the serve and bench scripts.
#[test]
fn blackbox_session_budget_declaration() {
    let s = scratch("session");
    let node = s.0.join("node");
    std::fs::create_dir_all(&node).unwrap();
    let n = node.to_str().unwrap();
    let bs = |args: &[&str]| {
        let mut a = vec!["--node-dir", n];
        a.extend_from_slice(args);
        launch(&s.0, "node_blackbox/blackbox_session.sh", &a)
    };
    let now = "2026-11-18T03:00:00Z";
    // budget-defaults --field ttl_s: ASCII digits only (run_trial_session.rs, budget_renew_loop.sh)
    let out = bs(&["budget-defaults", "--field", "ttl_s"]);
    let ttl = text(&out.stdout);
    assert_eq!(out.status.code(), Some(0));
    assert!(!ttl.trim_end().is_empty() && ttl.trim_end().bytes().all(|b| b.is_ascii_digit()), "{ttl:?}");
    // budget-status: rc 3 = not declared
    let out = bs(&["budget-status", "--now", now]);
    assert_eq!(out.status.code(), Some(3));
    assert_eq!(Json::parse(&text(&out.stdout)).unwrap().at("declared"), &Json::Bool(false));
    // declare-budget writes serve_budget.env (`floor_mib=`, `expires_epoch=`, `label=` lines read by sed)
    let out = bs(&[
        "declare-budget", "--mem-total-mib", "1000000", "--weights-mib", "1000", "--kv-mib", "1000", "--overhead-mib", "1000", "--ttl-s", "600",
        "--expected-load-s", "60", "--label", "model-a", "--now", now,
    ]);
    assert_eq!(out.status.code(), Some(0), "{}{}", text(&out.stdout), text(&out.stderr));
    let env = std::fs::read_to_string(node.join("serve_budget.env")).unwrap();
    let value = |k: &str| env.lines().find_map(|l| l.strip_prefix(&format!("{k}=")).map(str::to_string));
    assert_eq!(value("floor_mib").as_deref(), Some("997000"), "{env}");
    assert!(value("expires_epoch").is_some_and(|v| v.parse::<u64>().is_ok()), "{env}");
    assert_eq!(value("label").as_deref(), Some("model-a"), "{env}");
    // budget-status: JSON with `floor_mib` (int) and `label`, which the shell reads with its sed
    let out = bs(&["budget-status", "--now", now]);
    assert_eq!(out.status.code(), Some(0));
    let status = text(&out.stdout);
    let doc = Json::parse(&status).unwrap();
    assert_eq!(doc.at("floor_mib"), &Json::Num("997000".into()));
    assert_eq!(doc.at("label").str(), Some("model-a"));
    assert_eq!(sed_label(&status).as_deref(), Some("model-a"), "{status}");
    // renew-budget: 0 for the expected label, 3 when another declaration replaced it
    let out = bs(&["renew-budget", "--ttl-s", "600", "--expect-label", "model-a", "--now", "2026-11-18T03:05:00Z"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let out = bs(&["renew-budget", "--ttl-s", "600", "--expect-label", "model-b", "--now", "2026-11-18T03:06:00Z"]);
    assert_eq!(out.status.code(), Some(3));
    // budget-block / budget-skip are recorded (exit 0); start / stop a session
    assert_eq!(bs(&["budget-block", "--stage", "preflight_ceiling", "--reason", "arm_ceiling_below_min", "--label", "model-a", "--now", now]).status.code(), Some(0));
    assert_eq!(bs(&["budget-skip", "--reason=no-budget", "--label", "model-a", "--now", now]).status.code(), Some(0));
    let out = bs(&[
        "start", "--session-id", "sess-1", "--topology", "single", "--tp", "1", "--config-file", "cfg-1", "--model", "org/model", "--model-path",
        "/app/models/org/model", "--now", now,
    ]);
    assert_eq!(out.status.code(), Some(0), "{}{}", text(&out.stdout), text(&out.stderr));
    let out = bs(&["stop", "--session-id", "sess-1", "--now", "2026-11-18T03:10:00Z"]);
    assert_eq!(out.status.code(), Some(0), "{}{}", text(&out.stdout), text(&out.stderr));
    // clear-budget removes the declaration
    assert_eq!(bs(&["clear-budget", "--now", now]).status.code(), Some(0));
    assert_eq!(bs(&["budget-status", "--now", now]).status.code(), Some(3));
}

/// Keys of a `KEY=VALUE` params file as bash sees them after `source` (the watchdogs source it).
fn sourced(file: &Path, keys: &[&str]) -> Vec<String> {
    let script = keys.iter().map(|k| format!("printf '%s\\n' \"${{{k}-UNSET}}\"")).collect::<Vec<_>>().join("; ");
    let out = Command::new("bash").arg("-c").arg(format!(". \"$0\"; {script}")).arg(file).stdin(Stdio::null()).output().unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    text(&out.stdout).lines().map(str::to_string).collect()
}

/// `blackbox_eta --emit-params F` and `blackbox_thermal --emit-params F [--gpu-model]` write the shell constants the
/// watchdogs source and budget_preflight parses; `--explain` (with `--set`) is exit 0; `--print-uncalibrated` prints the
/// comma-joined uncalibrated keys (empty = none) with exit 0.
#[test]
fn eta_and_thermal_params_files() {
    let s = scratch("params");
    let node = s.0.join("node");
    std::fs::create_dir_all(&node).unwrap();
    let eta = s.0.join("eta_params.env");
    let out = launch(&s.0, "node_blackbox/blackbox_eta.sh", &["--node-dir", node.to_str().unwrap(), "--emit-params", eta.to_str().unwrap(), "--mem-total-mib", "1"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let keys = [
        "BB_HARD_FLOOR_MIB", "BB_RUNWAY_MS", "BB_DEBOUNCE_N", "BB_MIN_RATE_MIB_S", "BB_MAX_RATE_MIB_S", "BB_ABS_BAND_MIB", "BB_DECL_MARGIN_MIB",
        "BB_DECL_MIN_CEILING_MIB", "BB_KILL_LATENCY_S", "BB_DETECT_MARGIN_S", "BB_DEBOUNCE_POLLS",
    ];
    for (k, v) in keys.iter().zip(sourced(&eta, &keys)) {
        assert!(v != "UNSET" && !v.is_empty() && v.parse::<f64>().is_ok(), "{k}={v:?}");
    }
    // budget_preflight.rs parses these lines as `KEY=<int>`
    let body = std::fs::read_to_string(&eta).unwrap();
    for k in ["BB_DECL_MARGIN_MIB", "BB_DECL_MIN_CEILING_MIB", "BB_ABS_BAND_MIB"] {
        assert!(body.lines().any(|l| l.strip_prefix(&format!("{k}=")).is_some_and(|v| v.parse::<i64>().is_ok())), "{k}: {body}");
    }
    let out = launch(&s.0, "node_blackbox/blackbox_eta.sh", &["--set", "kill_latency_s=5", "--set", "detect_margin_s=5", "--set", "debounce_polls=3", "--explain", "10000", "100"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));

    let keys = ["BB_TP_GPU_PWR_W", "BB_TP_GPU_PWR_DW", "BB_TP_GPU_SUSTAIN_S", "BB_TP_SOC_WARN_C", "BB_TP_SOC_HARD_C", "BB_TP_GPU_AXIS", "BB_TP_UNCALIBRATED"];
    for gpu in [None, Some("NVIDIA GB10")] {
        let th = s.0.join("thermal_params.env");
        let mut args = vec!["--emit-params", th.to_str().unwrap()];
        if let Some(g) = gpu {
            args.extend(["--gpu-model", g]);
        }
        let out = launch(&s.0, "node_blackbox/blackbox_thermal.sh", &args);
        assert_eq!(out.status.code(), Some(0), "{gpu:?}: {}", text(&out.stderr));
        let got = sourced(&th, &keys);
        for (k, v) in keys.iter().zip(&got) {
            assert_ne!(v, "UNSET", "{k}");
        }
        assert!(matches!(got[5].as_str(), "0" | "1"), "BB_TP_GPU_AXIS={}", got[5]);
    }
    let out = launch(&s.0, "node_blackbox/blackbox_thermal.sh", &["--set", "gpu_pwr_w=100", "--set", "gpu_sustain_s=60", "--set", "soc_warn_c=80", "--set", "soc_hard_c=95", "--explain", "90", "47"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let out = launch(&s.0, "node_blackbox/blackbox_thermal.sh", &["--print-uncalibrated"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let line = text(&out.stdout);
    let line = line.trim_end();
    assert!(line.is_empty() || line.split(',').all(|k| !k.is_empty() && !k.contains(' ')), "{line:?}");
}

/// upstream `branch_sync_selftest.rs` S6: in a tree whose topology rules declare `multi`, `render_sub_env --topology single`
/// is exit 5 with `TOPOLOGY_RULES_MISMATCH` on stderr and writes nothing; `--topology multi` passes the guard and reaches
/// the manifest check (exit 3). The tree is a copy (the launcher, the 4-way predicate's launcher, the rules file); the
/// executables are this build and the policy-gate build of the repository (`TOPOLOGY_PARITY_BIN`).
#[test]
fn render_sub_env_topology_rules_guard() {
    let s = scratch("render");
    let tree = s.0.join("tree");
    for rel in [".claude/skills/terraforming_node/scripts/render_sub_env.sh", ".claude/policies/runtime/topology_parity.sh"] {
        let to = tree.join(rel);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(root().join(rel), &to).unwrap();
    }
    write(&tree.join(".claude/rules/strategy.topology.md"), "# 특화헌법 픽스처\n\n**topology: multi** · layer: topology\n");
    let listing = |dir: &Path| {
        let mut v: Vec<PathBuf> = Vec::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                if e.path().is_dir() {
                    stack.push(e.path());
                }
                v.push(e.path());
            }
        }
        v.sort();
        v
    };
    let before = listing(&tree);
    let absent = s.0.join("no-such-manifest.yaml");
    let render = |topology: &str| {
        Command::new("bash")
            .arg(tree.join(".claude/skills/terraforming_node/scripts/render_sub_env.sh"))
            .args(["--topology", topology, "--manifest", absent.to_str().unwrap()])
            .env("TERRAFORMING_BIN_DIR", bin_dir())
            .env("TOPOLOGY_PARITY_BIN", root().join(".claude/rust/policy-gate/target/release/topology_parity"))
            .env("GIT_CEILING_DIRECTORIES", std::env::temp_dir())
            .env_remove("TERRAFORMING_OWNER_PATH")
            .current_dir(&s.0)
            .stdin(Stdio::null())
            .output()
            .unwrap()
    };
    let out = render("single");
    assert_eq!(out.status.code(), Some(5), "{}", text(&out.stderr));
    assert!(text(&out.stderr).contains("TOPOLOGY_RULES_MISMATCH"), "{}", text(&out.stderr));
    assert_eq!(listing(&tree), before, "nothing staged");
    let out = render("multi");
    assert_eq!(out.status.code(), Some(3), "{}", text(&out.stderr));
    assert!(!text(&out.stderr).contains("TOPOLOGY_RULES_MISMATCH"));
}

/// multinode_serve_smoke.sh: `regen_envelope --node-dir D staleness --now T --max-age-days N` (2>&1 shown as a warning,
/// exit ignored) runs and says something about an absent or old envelope.
#[test]
fn regen_envelope_staleness_runs() {
    let s = scratch("stale");
    let node = s.0.join("node");
    std::fs::create_dir_all(&node).unwrap();
    let out = launch(&s.0, "node_blackbox/regen_envelope.sh", &["--node-dir", node.to_str().unwrap(), "staleness", "--now", "2026-11-18T03:00:00Z", "--max-age-days", "14"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(!format!("{}{}", text(&out.stdout), text(&out.stderr)).trim().is_empty());
}

/// verify_node_blackbox.sh: `bb_capture_proven <OUT>` (no launcher) prints the carried `capture_proven` object, converts a
/// schema-1 post record, and prints `null` for anything else; exit 0.
#[test]
fn bb_capture_proven_carry_forward() {
    let s = scratch("proven");
    let run = |body: Option<&str>| {
        let f = s.0.join("capture_verified.json");
        let _ = std::fs::remove_file(&f);
        if let Some(b) = body {
            write(&f, b);
        }
        let out = Command::new(env!("CARGO_BIN_EXE_bb_capture_proven")).arg(&f).stdin(Stdio::null()).output().unwrap();
        assert_eq!(out.status.code(), Some(0));
        Json::parse(&text(&out.stdout)).unwrap()
    };
    let doc = run(Some(r#"{"schema_version": 2, "capture_proven": {"proven_utc": "2026-11-01T00:00:00Z", "pass": 28, "fail": 0, "pending": 0}}"#));
    assert_eq!(doc.at("pass"), &Json::Num("28".into()));
    assert_eq!(doc.at("proven_utc").str(), Some("2026-11-01T00:00:00Z"));
    let doc = run(Some(r#"{"mode": "post", "verified_utc": "2026-10-01T00:00:00Z", "pass": 2, "fail": 0, "pending": 1}"#));
    assert_eq!(doc.keys(), ["proven_utc", "pass", "fail", "pending"]);
    assert_eq!(doc.at("proven_utc").str(), Some("2026-10-01T00:00:00Z"));
    assert_eq!(run(Some(r#"{"mode": "check"}"#)), Json::Null);
    assert_eq!(run(Some("not json")), Json::Null);
    assert_eq!(run(None), Json::Null);
}

/// build_rust_packages.sh: `rust_package_check METADATA_JSON CRATE_DIR` — exit 0 and one JSON line (`package`,
/// `normal_bins`, `normal_bin_count`, `libraries`) when every normal bin is an executable ELF under `target/release` and
/// every library has its `.rlib`; exit 1 otherwise.
#[test]
fn rust_package_check_summary_and_failure() {
    let s = scratch("pkgcheck");
    let krate = s.0.join("crate");
    write(&krate.join("Cargo.toml"), "[package]\nname = \"demo\"\n");
    std::fs::create_dir_all(krate.join("target/release")).unwrap();
    std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_bb_capture_proven"), krate.join("target/release/demo_tool")).unwrap();
    write(&krate.join("target/release/libdemo.rlib"), "rlib");
    let meta = s.0.join("metadata.json");
    let manifest = krate.join("Cargo.toml");
    let body = |bins: &[&str]| {
        let targets: Vec<String> = bins
            .iter()
            .map(|b| format!(r#"{{"kind": ["bin"], "name": "{b}", "required-features": []}}"#))
            .chain([r#"{"kind": ["lib"], "name": "demo"}"#.to_string(), r#"{"kind": ["bin"], "name": "gated", "required-features": ["fixture"]}"#.to_string()])
            .collect();
        format!(r#"{{"packages": [{{"name": "demo", "manifest_path": "{}", "targets": [{}]}}]}}"#, manifest.display(), targets.join(", "))
    };
    let check = || Command::new(env!("CARGO_BIN_EXE_rust_package_check")).arg(&meta).arg(&krate).stdin(Stdio::null()).output().unwrap();
    write(&meta, &body(&["demo_tool"]));
    let out = check();
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let doc = Json::parse(&text(&out.stdout)).unwrap();
    assert_eq!(doc.at("package").str(), Some("demo"));
    assert_eq!(doc.at("normal_bins").strs(), ["demo_tool"]);
    assert_eq!(doc.at("normal_bin_count"), &Json::Num("1".into()));
    assert_eq!(doc.at("libraries").strs(), ["demo"]);
    write(&meta, &body(&["demo_tool", "missing_tool"]));
    let out = check();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(text(&out.stdout), "");
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
