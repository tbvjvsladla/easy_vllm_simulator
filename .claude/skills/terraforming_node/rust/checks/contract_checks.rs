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
//! - build_rust_packages.sh: `rust_package_check METADATA_JSON CRATE_DIR` exit 0 / 1 and its summary line;
//! - install_node_blackbox.sh·verify_node_blackbox.sh: the installed daemon copies (`/usr/local/sbin/easy-vllm-bb-*`) run
//!   without a launcher — the units' argv, cwd `/`, no owner path — and their `--self-test`;
//! - wiki-desk `accessor.ts`·`gateway.ts`: `library_access --revoke` rewrites the manifest's `library` block lines and drops
//!   the `wiki-desk-gateway:<id>` line of authorized_keys;
//! - relay's ledgers keep the keys relay does not own when it rewrites them (`--supervise-step`), and relay keeps refusing
//!   a sub report with a key outside `task-report.schema.json`;
//! - thermal_watchdog.sh·install_node_blackbox.sh·verify_node_blackbox.sh: the collector's sample CSV (header, columns,
//!   integer `ts`, the UTC-date file name, one flushed row per sample);
//! - node_identity.sh·role-contract `identity.rs`: the sub manifest render_sub_env delivers keeps its top-level
//!   `self_role: sub` line;
//! - policy-predicates `claims_text.rs` and upstream `branch_sync_selftest.rs`: the source tokens they read in this crate;
//! - text arguments that are not UTF-8: exit 2 where base refuses them (closed choices); the consumers' options no other
//!   check runs are probed against a renamed control.
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
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let p = std::env::temp_dir().join(format!(
        "terraforming-contract-{tag}-{}-{n}",
        std::process::id()
    ));
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
    Path::new(env!("CARGO_BIN_EXE_relay"))
        .parent()
        .unwrap()
        .to_path_buf()
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
    std::env::split_paths(&path)
        .map(|d| d.join("env"))
        .find(|p| p.is_file())
        .expect("env on PATH")
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
        assert_eq!(
            out.status.code(),
            Some(0),
            "{launcher}: {}",
            text(&out.stderr)
        );
        let env = text(&out.stdout);
        let owner = env
            .lines()
            .find_map(|l| l.strip_prefix("TERRAFORMING_OWNER_PATH="))
            .unwrap_or_else(|| panic!("{launcher}: {env}"));
        assert_eq!(
            std::fs::canonicalize(owner).unwrap(),
            path.canonicalize().unwrap(),
            "{launcher}"
        );
        if stem(launcher).starts_with("selftest_") {
            continue; // self-test drivers without options (run in consumer_self_tests_pass)
        }
        let out = launch(&s.0, launcher, &["--help"]);
        assert_eq!(
            out.status.code(),
            Some(0),
            "{launcher} --help: {}",
            text(&out.stderr)
        );
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
        (
            "agent_control.sh",
            &[],
            &["invoke", "runners", "--self-test"],
        ),
        (
            "agent_control.sh",
            &["invoke"],
            &["--dispatch-id", "--request", "--result-file"],
        ),
        ("agent_control.sh", &["runners"], &["--provider"]),
        (
            "audit_session_ssh.sh",
            &[],
            &["--transcript", "--self-test"],
        ),
        (
            "bootstrap_canary.sh",
            &[],
            &[
                "--phase",
                "--target",
                "--topology",
                "--backend",
                "--self-test",
                "discovery",
                "permissions",
                "caller",
            ],
        ),
        (
            "check_sub_hint_equip.sh",
            &[],
            &[
                "--clone",
                "--remote-url",
                "--write-push-authority",
                "--utc",
                "--self-test",
                "--push-transport",
                "--narrow-env-mode",
                "--sub-workspace-shared",
            ],
        ),
        (
            "library_access.sh",
            &[],
            &[
                "--plan",
                "--apply",
                "--revoke",
                "--approved-by",
                "--node",
                "--topology",
                "--self-test",
            ],
        ),
        (
            "relay.sh",
            &[],
            &[
                "--apply",
                "--budget-source",
                "--continue",
                "--max-turns",
                "--resume",
                "--timeout-seconds",
                "--context-id",
                "--task",
                "--task-file",
                "--emit-only",
                "--runners",
                "--supervise-step",
                "--backend",
                "--runner-effort",
                "--campaign-control",
                "--campaign-id",
                "--node",
                "--orphan-sweep",
                "--reason",
                "--self-test",
                "suspend",
                "resume",
                "close",
                "abandon",
                "reslice",
            ],
        ),
        (
            "render_sub_env.sh",
            &[],
            &[
                "--topology",
                "--manifest",
                "--out",
                "--tracked-list",
                "--sub-manifest",
                "--hint-write-scopes",
                "--self-test",
            ],
        ),
        (
            "staleness_gate.sh",
            &[],
            &["--topology", "--repo", "--observed", "--self-test"],
        ),
        (
            "node_blackbox/blackbox_session.sh",
            &[],
            &[
                "--node-dir",
                "--self-test",
                "start",
                "stop",
                "declare-budget",
                "budget-status",
                "renew-budget",
                "budget-defaults",
                "clear-budget",
                "budget-skip",
                "budget-block",
            ],
        ),
        (
            "node_blackbox/blackbox_session.sh",
            &["declare-budget"],
            &[
                "--mem-total-mib",
                "--weights-mib",
                "--kv-mib",
                "--overhead-mib",
                "--ttl-s",
                "--expected-load-s",
                "--label",
                "--now",
            ],
        ),
        (
            "node_blackbox/blackbox_session.sh",
            &["renew-budget"],
            &["--ttl-s", "--expect-label", "--now"],
        ),
        (
            "node_blackbox/blackbox_session.sh",
            &["budget-defaults"],
            &["--field"],
        ),
        (
            "node_blackbox/blackbox_session.sh",
            &["budget-block"],
            &["--stage", "--reason", "--label", "--detail", "--now"],
        ),
        (
            "node_blackbox/blackbox_session.sh",
            &["budget-skip"],
            &["--reason", "--label", "--now"],
        ),
        (
            "node_blackbox/blackbox_session.sh",
            &["start"],
            &[
                "--session-id",
                "--topology",
                "--tp",
                "--config-file",
                "--model",
                "--model-path",
                "--now",
                "--image",
            ],
        ),
        (
            "node_blackbox/blackbox_session.sh",
            &["stop"],
            &["--session-id", "--now"],
        ),
        (
            "node_blackbox/blackbox_session.sh",
            &["clear-budget"],
            &["--now"],
        ),
        (
            "node_blackbox/blackbox_session.sh",
            &["budget-status"],
            &["--now"],
        ),
        (
            "node_blackbox/blackbox_eta.sh",
            &[],
            &[
                "--node-dir",
                "--emit-params",
                "--set",
                "--explain",
                "--mem-total-mib",
                "--self-test",
            ],
        ),
        (
            "node_blackbox/blackbox_thermal.sh",
            &[],
            &[
                "--emit-params",
                "--set",
                "--explain",
                "--gpu-model",
                "--self-test",
            ],
        ),
        (
            "node_blackbox/regen_envelope.sh",
            &[],
            &[
                "regen",
                "check-keys",
                "staleness",
                "--node-dir",
                "--self-test",
            ],
        ),
        (
            "node_blackbox/regen_envelope.sh",
            &["staleness"],
            &["--now", "--max-age-days"],
        ),
    ];
    for (launcher, sub, options) in table {
        let mut args: Vec<&str> = sub.to_vec();
        args.push("--help");
        let out = launch(&s.0, launcher, &args);
        let help = text(&out.stdout);
        assert_eq!(
            out.status.code(),
            Some(0),
            "{launcher} {sub:?}: {}",
            text(&out.stderr)
        );
        for o in options {
            assert!(
                help.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
                    .any(|w| w == *o),
                "{launcher} {sub:?} lacks {o}:\n{help}"
            );
        }
    }
    let out = launch(
        &s.0,
        "node_blackbox/blackbox_session.sh",
        &["--bogus-option"],
    );
    assert_eq!(
        (out.status.code(), text(&out.stdout).as_str()),
        (Some(2), "")
    );
}

/// The check lines of a self-test's merged output: (`[PASS] …` or `PASS …` lines, `[FAIL] …` or `FAIL …` lines).
fn check_lines(out: &str) -> (usize, usize) {
    let starts = |l: &str, word: &str| {
        l.starts_with(&format!("[{word}]")) || l.starts_with(&format!("{word} "))
    };
    let lines: Vec<&str> = out.lines().map(str::trim_start).collect();
    (
        lines.iter().filter(|l| starts(l, "PASS")).count(),
        lines.iter().filter(|l| starts(l, "FAIL")).count(),
    )
}

/// Exit 0 and, for a self-test that prints one line per check, at least the checks it has at base and no failed one
/// (a shrunken self-test still exits 0, so its exit code alone would not show it; 0 = no check lines are printed).
fn assert_self_test(what: &str, out: &Output, min_checks: usize) {
    let merged = format!("{}{}", text(&out.stdout), text(&out.stderr));
    assert_eq!(out.status.code(), Some(0), "{what}:\n{merged}");
    let (passed, failed) = check_lines(&merged);
    assert!(
        passed >= min_checks && failed == 0,
        "{what}: {passed} passed (base {min_checks}), {failed} failed:\n{merged}"
    );
}

/// policy-distribution, policy-selftest and verify_node_blackbox.sh run these through the launchers and need exit 0.
/// (`blackbox_collect --self-test` runs in `blackbox_daemons_run_as_installed`, away from the host's container and GPU
/// tools.)
#[test]
fn consumer_self_tests_pass() {
    let s = scratch("selftest");
    for (launcher, min_checks) in [
        ("agent_control.sh", 0),
        ("bootstrap_canary.sh", 10),
        ("check_sub_hint_equip.sh", 22),
        ("library_access.sh", 17),
        ("relay.sh", 147),
        ("render_sub_env.sh", 24),
        ("staleness_gate.sh", 24),
        ("node_blackbox/agent_guard.sh", 45),
        ("node_blackbox/blackbox_eta.sh", 79),
        ("node_blackbox/blackbox_events.sh", 42),
        ("node_blackbox/blackbox_session.sh", 69),
        ("node_blackbox/blackbox_thermal.sh", 46),
        ("node_blackbox/logs_lifecycle.sh", 29),
        ("node_blackbox/regen_envelope.sh", 37),
        ("node_blackbox/seed_from_journal.sh", 13),
    ] {
        assert_self_test(
            &format!("{launcher} --self-test"),
            &launch(&s.0, launcher, &["--self-test"]),
            min_checks,
        );
    }
    for (driver, min_checks) in [
        ("selftest_bootstrap_permissions.sh", 85),
        ("selftest_campaign_lifecycle.sh", 103),
        ("selftest_claude_native_events.sh", 49),
        ("selftest_harness_switch.sh", 147),
        ("selftest_sub_hint_write_scopes.sh", 62),
    ] {
        assert_self_test(driver, &launch(&s.0, driver, &[]), min_checks);
    }
}

/// The `"label": *"…"` sed of budget_renew_loop.sh:145 (label charset `[A-Za-z0-9._-]{1,64}`).
fn sed_label(out: &str) -> Option<String> {
    out.lines().find_map(|l| {
        let rest = l[l.find("\"label\":")? + 8..].trim_start_matches(' ');
        let v: String = rest
            .strip_prefix('"')?
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
            .collect();
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
    assert!(
        !ttl.trim_end().is_empty() && ttl.trim_end().bytes().all(|b| b.is_ascii_digit()),
        "{ttl:?}"
    );
    // budget-status: rc 3 = not declared
    let out = bs(&["budget-status", "--now", now]);
    assert_eq!(out.status.code(), Some(3));
    assert_eq!(
        Json::parse(&text(&out.stdout)).unwrap().at("declared"),
        &Json::Bool(false)
    );
    // declare-budget writes serve_budget.env (`floor_mib=`, `expires_epoch=`, `label=` lines read by sed)
    let out = bs(&[
        "declare-budget",
        "--mem-total-mib",
        "1000000",
        "--weights-mib",
        "1000",
        "--kv-mib",
        "1000",
        "--overhead-mib",
        "1000",
        "--ttl-s",
        "600",
        "--expected-load-s",
        "60",
        "--label",
        "model-a",
        "--now",
        now,
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}{}",
        text(&out.stdout),
        text(&out.stderr)
    );
    let env = std::fs::read_to_string(node.join("serve_budget.env")).unwrap();
    let value = |k: &str| {
        env.lines()
            .find_map(|l| l.strip_prefix(&format!("{k}=")).map(str::to_string))
    };
    assert_eq!(value("floor_mib").as_deref(), Some("997000"), "{env}");
    assert!(
        value("expires_epoch").is_some_and(|v| v.parse::<u64>().is_ok()),
        "{env}"
    );
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
    let out = bs(&[
        "renew-budget",
        "--ttl-s",
        "600",
        "--expect-label",
        "model-a",
        "--now",
        "2026-11-18T03:05:00Z",
    ]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let out = bs(&[
        "renew-budget",
        "--ttl-s",
        "600",
        "--expect-label",
        "model-b",
        "--now",
        "2026-11-18T03:06:00Z",
    ]);
    assert_eq!(out.status.code(), Some(3));
    // budget-block / budget-skip are recorded (exit 0); start / stop a session
    assert_eq!(
        bs(&[
            "budget-block",
            "--stage",
            "preflight_ceiling",
            "--reason",
            "arm_ceiling_below_min",
            "--label",
            "model-a",
            "--now",
            now
        ])
        .status
        .code(),
        Some(0)
    );
    assert_eq!(
        bs(&[
            "budget-skip",
            "--reason=no-budget",
            "--label",
            "model-a",
            "--now",
            now
        ])
        .status
        .code(),
        Some(0)
    );
    let out = bs(&[
        "start",
        "--session-id",
        "sess-1",
        "--topology",
        "single",
        "--tp",
        "1",
        "--config-file",
        "cfg-1",
        "--model",
        "org/model",
        "--model-path",
        "/app/models/org/model",
        "--now",
        now,
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}{}",
        text(&out.stdout),
        text(&out.stderr)
    );
    let out = bs(&[
        "stop",
        "--session-id",
        "sess-1",
        "--now",
        "2026-11-18T03:10:00Z",
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}{}",
        text(&out.stdout),
        text(&out.stderr)
    );
    // the data hint-publisher `e2.rs` reads: the session file (`session_id`, `started_utc`, `stopped_utc`,
    // `config_file`, `topology`) and the node's events (`kind` serve_start / serve_stop with `session_id`, `source` =
    // blackbox_session and, on the start, `topology`; budget_declare with the same `source`)
    let session =
        Json::parse(&std::fs::read_to_string(node.join("sessions/sess-1.json")).unwrap()).unwrap();
    for (key, value) in [
        ("session_id", "sess-1"),
        ("started_utc", now),
        ("stopped_utc", "2026-11-18T03:10:00Z"),
        ("config_file", "cfg-1"),
        ("topology", "single"),
    ] {
        assert_eq!(session.at(key).str(), Some(value), "{key}: {session:?}");
    }
    let mut events = Vec::new();
    for e in std::fs::read_dir(node.join("events")).unwrap().flatten() {
        if e.path().extension().is_some_and(|x| x == "jsonl") {
            for line in std::fs::read_to_string(e.path())
                .unwrap()
                .lines()
                .filter(|l| !l.trim().is_empty())
            {
                events.push(Json::parse(line).unwrap());
            }
        }
    }
    for kind in ["serve_start", "serve_stop"] {
        let ev = events
            .iter()
            .find(|e| e.at("kind").str() == Some(kind))
            .unwrap_or_else(|| panic!("no {kind}: {events:?}"));
        assert_eq!(ev.at("session_id").str(), Some("sess-1"), "{kind}");
        assert_eq!(ev.at("source").str(), Some("blackbox_session"), "{kind}");
    }
    let start = events
        .iter()
        .find(|e| e.at("kind").str() == Some("serve_start"))
        .unwrap();
    assert_eq!(start.at("topology").str(), Some("single"));
    let declared = events
        .iter()
        .find(|e| e.at("kind").str() == Some("budget_declare"))
        .unwrap_or_else(|| panic!("no budget_declare: {events:?}"));
    assert_eq!(declared.at("source").str(), Some("blackbox_session"));
    // the declaration's numbers (e2.rs `_EVENT_DETAIL_FIELDS`; `mem_total_mib` is the runner's measurement, :529-538)
    assert_eq!(declared.at("label").str(), Some("model-a"));
    for (key, value) in [
        ("floor_mib", "997000"),
        ("weights_mib", "1000"),
        ("kv_mib", "1000"),
        ("overhead_mib", "1000"),
        ("mem_total_mib", "1000000"),
    ] {
        assert_eq!(
            declared.at(key),
            &Json::Num(value.into()),
            "{key}: {declared:?}"
        );
    }
    // clear-budget removes the declaration
    assert_eq!(bs(&["clear-budget", "--now", now]).status.code(), Some(0));
    assert_eq!(bs(&["budget-status", "--now", now]).status.code(), Some(3));
}

/// Keys of a `KEY=VALUE` params file as bash sees them after `source` (the watchdogs source it).
fn sourced(file: &Path, keys: &[&str]) -> Vec<String> {
    let script = keys
        .iter()
        .map(|k| format!("printf '%s\\n' \"${{{k}-UNSET}}\""))
        .collect::<Vec<_>>()
        .join("; ");
    let out = Command::new("bash")
        .arg("-c")
        .arg(format!(". \"$0\"; {script}"))
        .arg(file)
        .stdin(Stdio::null())
        .output()
        .unwrap();
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
    let out = launch(
        &s.0,
        "node_blackbox/blackbox_eta.sh",
        &[
            "--node-dir",
            node.to_str().unwrap(),
            "--emit-params",
            eta.to_str().unwrap(),
            "--mem-total-mib",
            "1",
        ],
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let keys = [
        "BB_HARD_FLOOR_MIB",
        "BB_RUNWAY_MS",
        "BB_DEBOUNCE_N",
        "BB_MIN_RATE_MIB_S",
        "BB_MAX_RATE_MIB_S",
        "BB_ABS_BAND_MIB",
        "BB_DECL_MARGIN_MIB",
        "BB_DECL_MIN_CEILING_MIB",
        "BB_KILL_LATENCY_S",
        "BB_DETECT_MARGIN_S",
        "BB_DEBOUNCE_POLLS",
    ];
    for (k, v) in keys.iter().zip(sourced(&eta, &keys)) {
        assert!(
            v != "UNSET" && !v.is_empty() && v.parse::<f64>().is_ok(),
            "{k}={v:?}"
        );
    }
    // budget_preflight.rs parses these lines as `KEY=<int>`
    let body = std::fs::read_to_string(&eta).unwrap();
    for k in [
        "BB_DECL_MARGIN_MIB",
        "BB_DECL_MIN_CEILING_MIB",
        "BB_ABS_BAND_MIB",
    ] {
        assert!(
            body.lines().any(|l| l
                .strip_prefix(&format!("{k}="))
                .is_some_and(|v| v.parse::<i64>().is_ok())),
            "{k}: {body}"
        );
    }
    let out = launch(
        &s.0,
        "node_blackbox/blackbox_eta.sh",
        &[
            "--set",
            "kill_latency_s=5",
            "--set",
            "detect_margin_s=5",
            "--set",
            "debounce_polls=3",
            "--explain",
            "10000",
            "100",
        ],
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));

    let keys = [
        "BB_TP_GPU_PWR_W",
        "BB_TP_GPU_PWR_DW",
        "BB_TP_GPU_SUSTAIN_S",
        "BB_TP_SOC_WARN_C",
        "BB_TP_SOC_HARD_C",
        "BB_TP_GPU_AXIS",
        "BB_TP_UNCALIBRATED",
    ];
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
        assert!(
            matches!(got[5].as_str(), "0" | "1"),
            "BB_TP_GPU_AXIS={}",
            got[5]
        );
    }
    let out = launch(
        &s.0,
        "node_blackbox/blackbox_thermal.sh",
        &[
            "--set",
            "gpu_pwr_w=100",
            "--set",
            "gpu_sustain_s=60",
            "--set",
            "soc_warn_c=80",
            "--set",
            "soc_hard_c=95",
            "--explain",
            "90",
            "47",
        ],
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let out = launch(
        &s.0,
        "node_blackbox/blackbox_thermal.sh",
        &["--print-uncalibrated"],
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let line = text(&out.stdout);
    let line = line.trim_end();
    assert!(
        line.is_empty() || line.split(',').all(|k| !k.is_empty() && !k.contains(' ')),
        "{line:?}"
    );
}

/// upstream `branch_sync_selftest.rs` S6: in a tree whose topology rules declare `multi`, `render_sub_env --topology single`
/// is exit 5 with `TOPOLOGY_RULES_MISMATCH` on stderr and writes nothing; `--topology multi` passes the guard and reaches
/// the manifest check (exit 3). The tree is a copy (the launcher, the rules file); the executables are this build's. The
/// guard reads the rules file in-process: neither path runs the 4-way predicate (`topology_parity`).
#[test]
fn render_sub_env_topology_rules_guard() {
    let s = scratch("render");
    let tree = s.0.join("tree");
    let rel = ".claude/skills/terraforming_node/scripts/render_sub_env.sh";
    let to = tree.join(rel);
    std::fs::create_dir_all(to.parent().unwrap()).unwrap();
    std::fs::copy(root().join(rel), &to).unwrap();
    write(
        &tree.join(".claude/rules/strategy.topology.md"),
        "# 특화헌법 픽스처\n\n**topology: multi** · layer: topology\n",
    );
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
            .args([
                "--topology",
                topology,
                "--manifest",
                absent.to_str().unwrap(),
            ])
            .env("TERRAFORMING_BIN_DIR", bin_dir())
            .env("GIT_CEILING_DIRECTORIES", std::env::temp_dir())
            .env_remove("TERRAFORMING_OWNER_PATH")
            .current_dir(&s.0)
            .stdin(Stdio::null())
            .output()
            .unwrap()
    };
    let out = render("single");
    assert_eq!(out.status.code(), Some(5), "{}", text(&out.stderr));
    assert!(
        text(&out.stderr).contains("TOPOLOGY_RULES_MISMATCH"),
        "{}",
        text(&out.stderr)
    );
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
    let out = launch(
        &s.0,
        "node_blackbox/regen_envelope.sh",
        &[
            "--node-dir",
            node.to_str().unwrap(),
            "staleness",
            "--now",
            "2026-11-18T03:00:00Z",
            "--max-age-days",
            "14",
        ],
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(!format!("{}{}", text(&out.stdout), text(&out.stderr))
        .trim()
        .is_empty());
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
        let out = Command::new(env!("CARGO_BIN_EXE_bb_capture_proven"))
            .arg(&f)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(0));
        Json::parse(&text(&out.stdout)).unwrap()
    };
    let doc = run(Some(
        r#"{"schema_version": 2, "capture_proven": {"proven_utc": "2026-11-01T00:00:00Z", "pass": 28, "fail": 0, "pending": 0}}"#,
    ));
    assert_eq!(doc.at("pass"), &Json::Num("28".into()));
    assert_eq!(doc.at("proven_utc").str(), Some("2026-11-01T00:00:00Z"));
    let doc = run(Some(
        r#"{"mode": "post", "verified_utc": "2026-10-01T00:00:00Z", "pass": 2, "fail": 0, "pending": 1}"#,
    ));
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
    std::os::unix::fs::symlink(
        env!("CARGO_BIN_EXE_bb_capture_proven"),
        krate.join("target/release/demo_tool"),
    )
    .unwrap();
    write(&krate.join("target/release/libdemo.rlib"), "rlib");
    let meta = s.0.join("metadata.json");
    let manifest = krate.join("Cargo.toml");
    let body = |bins: &[&str]| {
        let targets: Vec<String> = bins
            .iter()
            .map(|b| format!(r#"{{"kind": ["bin"], "name": "{b}", "required-features": []}}"#))
            .chain([
                r#"{"kind": ["lib"], "name": "demo"}"#.to_string(),
                r#"{"kind": ["bin"], "name": "gated", "required-features": ["fixture"]}"#
                    .to_string(),
            ])
            .collect();
        format!(
            r#"{{"packages": [{{"name": "demo", "manifest_path": "{}", "targets": [{}]}}]}}"#,
            manifest.display(),
            targets.join(", ")
        )
    };
    let check = || {
        Command::new(env!("CARGO_BIN_EXE_rust_package_check"))
            .arg(&meta)
            .arg(&krate)
            .stdin(Stdio::null())
            .output()
            .unwrap()
    };
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

/// The first `name` on this process's `PATH`.
fn on_path(name: &str) -> PathBuf {
    let path = std::env::var_os("PATH").unwrap();
    std::env::split_paths(&path)
        .map(|d| d.join(name))
        .find(|p| p.is_file())
        .unwrap_or_else(|| panic!("{name} on PATH"))
}

/// The effective user id of this process (`/proc/self/status`).
fn euid() -> u32 {
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    let line = status.lines().find(|l| l.starts_with("Uid:")).unwrap();
    line.split_whitespace().nth(2).unwrap().parse().unwrap()
}

/// Lines with their runs of blanks made one space.
fn squeezed(body: &str) -> Vec<String> {
    body.lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect()
}

/// install_node_blackbox.sh copies the daemons' ELFs into its `$BIN` (`install -m 0755 … $BIN/easy-vllm-bb-<name>`,
/// :255-263) and its systemd units start the copies with no launcher (:332, :383, :406-407); verify_node_blackbox.sh
/// :232-245 runs each installed copy's `--self-test` from `/` (exit 0 = deployed). The installer's lines are fixed as it
/// writes them, then each is run the way the unit runs it: the ELF copied (not linked) into a scratch `sbin/`, cwd `/`,
/// argv[0] the installed path, neither `TERRAFORMING_OWNER_PATH` nor `TERRAFORMING_BIN_DIR` — the copy's place does not
/// lead to the repository — and a scratch node directory as the installer prepares it (`samples`·`events`·`rollup`).
/// `PATH` holds only `date` (the lifecycle units' `$(date -u …)`): the host's container, GPU and journal tools are not
/// reached, and the samplers record those columns as absent. The collector (`--interval 1`, a `Restart=always` service)
/// records samples until it is interrupted (SIGINT ends it with exit 0); lifecycle then regen run through `sh -c` with
/// the units' command text, and regen writes the node's `envelope.json`.
#[test]
fn blackbox_daemons_run_as_installed() {
    let s = scratch("installed");
    let installer = squeezed(
        &std::fs::read_to_string(skill().join("scripts/node_blackbox/install_node_blackbox.sh"))
            .unwrap(),
    );
    let has = |line: &str| installer.iter().any(|l| l == line);
    for line in [
        r#"LIFECYCLE_BIN="$RUST_REL/logs_lifecycle""#,
        r#"run install -m 0755 "$RUST_REL/blackbox_collect" "$BIN/easy-vllm-bb-collect""#,
        r#"run install -m 0755 "$RUST_REL/blackbox_eta" "$BIN/easy-vllm-bb-eta""#,
        r#"run install -m 0755 "$RUST_REL/blackbox_events" "$BIN/easy-vllm-bb-events""#,
        r#"run install -m 0755 "$LIFECYCLE_BIN" "$BIN/easy-vllm-bb-lifecycle""#,
        r#"run install -m 0755 "$RUST_REL/regen_envelope" "$BIN/easy-vllm-bb-regen-envelope""#,
        "ExecStart=$BIN/easy-vllm-bb-collect --node-dir $NODE_DIR --interval 1",
        "ExecStart=$BIN/easy-vllm-bb-events --node-dir $NODE_DIR",
        r#"ExecStart=/bin/sh -c '$BIN/easy-vllm-bb-lifecycle --node-dir $NODE_DIR --now "\$(date -u +%%Y-%%m-%%dT%%H:%%M:%%SZ)" --apply'"#,
        r#"ExecStart=/bin/sh -c '$BIN/easy-vllm-bb-regen-envelope --node-dir $NODE_DIR regen --now "\$(date -u +%%Y-%%m-%%dT%%H:%%M:%%SZ)"'"#,
    ] {
        assert!(has(line), "install_node_blackbox.sh lacks: {line}");
    }
    let verifier = squeezed(
        &std::fs::read_to_string(skill().join("scripts/node_blackbox/verify_node_blackbox.sh"))
            .unwrap(),
    )
    .join("\n");
    assert!(verifier.contains("for _b in easy-vllm-bb-eta easy-vllm-bb-collect easy-vllm-bb-events \\\neasy-vllm-bb-lifecycle easy-vllm-bb-regen-envelope; do"), "verify_node_blackbox.sh: the installed copies");
    assert!(
        verifier.contains(r#"( cd / && "$_d" --self-test >/dev/null 2>&1 )"#),
        "verify_node_blackbox.sh: the self-test from /"
    );

    let sbin = s.0.join("sbin");
    let tools = s.0.join("tools");
    std::fs::create_dir_all(&sbin).unwrap();
    std::fs::create_dir_all(&tools).unwrap();
    std::os::unix::fs::symlink(on_path("date"), tools.join("date")).unwrap();
    for (name, exe) in [
        ("collect", env!("CARGO_BIN_EXE_blackbox_collect")),
        ("eta", env!("CARGO_BIN_EXE_blackbox_eta")),
        ("events", env!("CARGO_BIN_EXE_blackbox_events")),
        ("lifecycle", env!("CARGO_BIN_EXE_logs_lifecycle")),
        ("regen-envelope", env!("CARGO_BIN_EXE_regen_envelope")),
    ] {
        // `install -m 0755`, as the installer copies them; a copy written by this process could still be open for
        // writing in a child another check thread is forking, and executing it would fail with ETXTBSY
        let to = sbin.join(format!("easy-vllm-bb-{name}"));
        let st = Command::new(on_path("install"))
            .arg("-m")
            .arg("0755")
            .arg(exe)
            .arg(&to)
            .stdin(Stdio::null())
            .status()
            .unwrap();
        assert!(st.success(), "install {exe} {to:?}");
    }
    // (the installed eta copy has one check fewer than through its launcher: the legacy-watchdog cross-check needs the
    // repository and reports itself skipped)
    let installed = |cmd: &mut Command| {
        cmd.current_dir("/")
            .env_remove("TERRAFORMING_OWNER_PATH")
            .env_remove("TERRAFORMING_BIN_DIR")
            .env("PATH", &tools)
            .stdin(Stdio::null());
    };
    for (name, min_checks) in [
        ("eta", 78),
        ("collect", 47),
        ("events", 42),
        ("lifecycle", 29),
        ("regen-envelope", 37),
    ] {
        let mut c = Command::new(sbin.join(format!("easy-vllm-bb-{name}")));
        installed(c.arg("--self-test"));
        assert_self_test(
            &format!("installed easy-vllm-bb-{name} --self-test"),
            &c.output().unwrap(),
            min_checks,
        );
    }

    let node = s.0.join("node");
    for d in ["samples", "events", "rollup"] {
        std::fs::create_dir_all(node.join(d)).unwrap();
    }
    let n = node.to_str().unwrap();
    // the collector unit: samples every second until stopped
    if euid() == 0 {
        eprintln!("blackbox_daemons_run_as_installed: as root the collector would also write the kernel log (/dev/kmsg); its unit command is not run");
    } else {
        let log = s.0.join("collect.log");
        let mut c = Command::new(sbin.join("easy-vllm-bb-collect"));
        installed(c.args(["--node-dir", n, "--interval", "1"]));
        let mut child = c
            .stdout(std::fs::File::create(&log).unwrap())
            .stderr(std::fs::File::create(s.0.join("collect.err")).unwrap())
            .spawn()
            .unwrap();
        let sampled = || {
            std::fs::read_dir(node.join("samples"))
                .unwrap()
                .flatten()
                .any(|e| std::fs::read_to_string(e.path()).is_ok_and(|t| t.lines().count() >= 2))
        };
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        let mut exited = None;
        while !sampled() && std::time::Instant::now() < deadline {
            // a collector that ended by itself has been reaped: it is not signalled (its pid may be another process's now)
            if let Some(st) = child.try_wait().unwrap() {
                exited = Some(st);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        let rows_seen = sampled();
        let errors = || std::fs::read_to_string(s.0.join("collect.err")).unwrap_or_default();
        let status = match exited {
            Some(st) => st,
            None => {
                let kill = Command::new(on_path("kill"))
                    .args(["-INT", &child.id().to_string()])
                    .status()
                    .unwrap();
                assert!(kill.success(), "{}", errors());
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
                loop {
                    if let Some(st) = child.try_wait().unwrap() {
                        break st;
                    }
                    assert!(
                        std::time::Instant::now() < deadline,
                        "the collector did not stop on SIGINT: {}",
                        errors()
                    );
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            }
        };
        assert!(rows_seen, "no sample row ({status:?}): {}", errors());
        assert_eq!(status.code(), Some(0), "{}", errors());
        let day = std::fs::read_dir(node.join("samples"))
            .unwrap()
            .flatten()
            .next()
            .unwrap()
            .path();
        assert_eq!(
            std::fs::read_to_string(&day).unwrap().lines().next(),
            Some(SAMPLES_HEADER),
            "{day:?}"
        );
    }
    // the events unit (oneshot)
    let mut c = Command::new(sbin.join("easy-vllm-bb-events"));
    installed(c.args(["--node-dir", n]));
    let out = c.output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}{}",
        text(&out.stdout),
        text(&out.stderr)
    );
    // the lifecycle unit's two ExecStart lines, in order, through sh -c with the unit's command text
    for unit_command in [
        format!(
            r#"{}/easy-vllm-bb-lifecycle --node-dir {n} --now "$(date -u +%Y-%m-%dT%H:%M:%SZ)" --apply"#,
            sbin.display()
        ),
        format!(
            r#"{}/easy-vllm-bb-regen-envelope --node-dir {n} regen --now "$(date -u +%Y-%m-%dT%H:%M:%SZ)""#,
            sbin.display()
        ),
    ] {
        let mut c = Command::new(on_path("sh"));
        installed(c.arg("-c").arg(&unit_command));
        let out = c.output().unwrap();
        assert_eq!(
            out.status.code(),
            Some(0),
            "{unit_command}: {}{}",
            text(&out.stdout),
            text(&out.stderr)
        );
    }
    let envelope =
        Json::parse(&std::fs::read_to_string(node.join("envelope.json")).unwrap()).unwrap();
    assert_eq!(envelope.kind(), "object");
}

/// `library_access --revoke --approved-by WHO --node ID --topology T` (the main withdraws a sub's library access point,
/// SKILL.md §2.7.8): drops the line of the main's `~/.ssh/authorized_keys` that ends with the marker
/// `wiki-desk-gateway:<id>` (other lines stay) and rewrites the manifest's top-level `library` block — the lines wiki-desk
/// `accessor.ts` reads with its own YAML subset parser: two-space keys, path values as JSON strings, `access_points.<id>`
/// with `identity_file`·`known_hosts` — after the rest of the manifest, which stays; stdout `{"mode": "revoke",
/// "node_id", "approved_by"}`. Without `--approved-by` it stops (exit 3) and changes nothing. The tree is a copy of the
/// launcher (the repository is found from it); `HOME` is a scratch directory.
#[test]
fn library_access_revoke_rewrites_the_block_and_the_marked_line() {
    let s = scratch("library");
    let tree = s.0.join("tree");
    let launcher = tree.join(".claude/skills/terraforming_node/scripts/library_access.sh");
    std::fs::create_dir_all(launcher.parent().unwrap()).unwrap();
    std::fs::copy(skill().join("scripts/library_access.sh"), &launcher).unwrap();
    let home = s.0.join("home");
    let keys = home.join(".ssh/authorized_keys");
    let kept_key = "ssh-ed25519 AAAAkeep me@main";
    let other_gateway = r#"command="node /w/repo/.claude/skills/wiki-desk/src/gateway.ts --node sub2",restrict ssh-ed25519 AAAAsub2 wiki-desk-gateway:sub2"#;
    let gateway = r#"command="node /w/repo/.claude/skills/wiki-desk/src/gateway.ts --node sub",restrict ssh-ed25519 AAAAsub wiki-desk-gateway:sub"#;
    write(&keys, &format!("{kept_key}\n{gateway}\n{other_gateway}\n"));
    let rest = "# manifest fixture\ntopology: single\ngpus_per_node: 1\nnodes:\n  - role: main\n    host: main.invalid\n    ssh_user: u\n    work_dir: /w/repo\n  - role: sub\n    host: sub.invalid\n    ssh_user: u\n    work_dir: /w/sub\n    node_id: sub\n";
    let block = "library:\n  transport: local\n  wiki_dir: __llm-wiki\n  repo_root: \"/w/repo\"\n  access_points:\n    sub:\n      identity_file: \"/h/.ssh/k\"\n      known_hosts: \"/h/.ssh/kh\"\n    sub2:\n      identity_file: \"/h/.ssh/k2\"\n      known_hosts: \"/h/.ssh/kh2\"\n";
    let manifest = tree.join("output/single/manifest.yaml");
    write(
        &manifest,
        &format!("{rest}{block}terraforming:\n  complete: true\n"),
    );
    let la = |args: &[&str]| {
        Command::new("bash")
            .arg(&launcher)
            .args(args)
            .env("TERRAFORMING_BIN_DIR", bin_dir())
            .env_remove("TERRAFORMING_OWNER_PATH")
            .env("HOME", &home)
            .env("GIT_CEILING_DIRECTORIES", std::env::temp_dir())
            .current_dir(&s.0)
            .stdin(Stdio::null())
            .output()
            .unwrap()
    };
    let before = (
        std::fs::read_to_string(&manifest).unwrap(),
        std::fs::read_to_string(&keys).unwrap(),
    );
    let out = la(&["--revoke", "--node", "sub", "--topology", "single"]);
    assert_eq!(
        out.status.code(),
        Some(3),
        "no approval: {}{}",
        text(&out.stdout),
        text(&out.stderr)
    );
    assert_eq!(
        (
            std::fs::read_to_string(&manifest).unwrap(),
            std::fs::read_to_string(&keys).unwrap()
        ),
        before
    );
    assert_eq!(
        la(&["--revoke", "--node", "sub"]).status.code(),
        Some(2),
        "no --topology"
    );
    let out = la(&[
        "--revoke",
        "--approved-by",
        "reviewer",
        "--node",
        "sub",
        "--topology",
        "single",
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}{}",
        text(&out.stdout),
        text(&out.stderr)
    );
    let doc = Json::parse(&text(&out.stdout)).unwrap();
    assert!(
        doc.same(
            &Json::parse(r#"{"mode": "revoke", "node_id": "sub", "approved_by": "reviewer"}"#)
                .unwrap()
        ),
        "{doc:?}"
    );
    assert_eq!(
        std::fs::read_to_string(&keys).unwrap(),
        format!("{kept_key}\n{other_gateway}\n")
    );
    let after = std::fs::read_to_string(&manifest).unwrap();
    let lines: Vec<&str> = after.lines().collect();
    let at = lines
        .iter()
        .position(|l| l.starts_with("library:"))
        .unwrap_or_else(|| panic!("{after}"));
    let outside: Vec<&str> = lines[..at]
        .iter()
        .copied()
        .filter(|l| !l.is_empty())
        .collect();
    let mut want_outside: Vec<&str> = rest.lines().collect();
    want_outside.extend(["terraforming:", "  complete: true"]);
    assert_eq!(
        outside, want_outside,
        "the rest of the manifest stays, before the block"
    );
    assert_eq!(
        &lines[at + 1..],
        [
            "  transport: local",
            "  wiki_dir: __llm-wiki",
            "  repo_root: \"/w/repo\"",
            "  access_points:",
            "    sub2:",
            "      identity_file: \"/h/.ssh/k2\"",
            "      known_hosts: \"/h/.ssh/kh2\""
        ],
        "{after}"
    );
}

/// relay's ledgers `campaigns/<id>/relay/<ctx>.json` are rewritten whole by relay (types.md 4절 「영속 문서」 4), and keys
/// that are not relay's own — at the top, in an attempt — stay as read. `relay --supervise-step <camp> --utc T` takes one
/// supervisor step (without `--apply` it dispatches nothing): it appends to `supervisor_steps` an entry with `action`,
/// `reason` and `utc`, and the ledger's `revision` goes up by one. The tree holds copies of what relay reads (its launcher,
/// agent_control.sh for the runner table, the request schema); with no campaign_init.sh in it the campaign's lifecycle
/// is unknown and the step is a `hold`.
#[test]
fn relay_supervise_step_keeps_unknown_ledger_keys() {
    let s = scratch("relay");
    let tree = s.0.join("tree");
    for rel in [
        ".claude/skills/terraforming_node/scripts/relay.sh",
        ".claude/skills/terraforming_node/scripts/agent_control.sh",
        ".claude/schemas/agent-control-request.schema.json",
    ] {
        let to = tree.join(rel);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(root().join(rel), &to).unwrap();
    }
    write(&tree.join("campaigns/ACTIVE"), "c1\n");
    let ledger = tree.join("campaigns/c1/relay/ctx-1.json");
    let original = r#"{"schema_version": 1, "context_id": "ctx-1", "campaign_id": "c1", "campaign_node": "sub", "campaign_context_kind": "cell", "campaign_cell": "cell-a", "revision": 3, "attempts": [{"attempt": 1, "status": "completed", "phase": "serve", "_attempt_note": {"k": [1, 2]}}], "_note": "kept", "_x": {"nested": true}}"#;
    write(&ledger, original);
    let out = Command::new("bash")
        .arg(tree.join(".claude/skills/terraforming_node/scripts/relay.sh"))
        .args(["--supervise-step", "c1", "--utc", "2026-11-19T03:00:00Z"])
        .env("TERRAFORMING_BIN_DIR", bin_dir())
        .env_remove("TERRAFORMING_OWNER_PATH")
        .env("GIT_CEILING_DIRECTORIES", std::env::temp_dir())
        .current_dir(&s.0)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}{}",
        text(&out.stdout),
        text(&out.stderr)
    );
    let doc = Json::parse(&std::fs::read_to_string(&ledger).unwrap()).unwrap();
    let was = Json::parse(original).unwrap();
    for (key, value) in match &was {
        Json::Obj(m) => m.clone(),
        _ => unreachable!(),
    } {
        if key != "revision" {
            assert!(doc.at(&key).same(&value), "{key}: {doc:?}");
        }
    }
    assert_eq!(doc.at("revision"), &Json::Num("4".into()));
    let steps = doc.at("supervisor_steps").arr().unwrap();
    assert_eq!(steps.len(), 1, "{doc:?}");
    assert_eq!(steps[0].at("action").str(), Some("hold"), "{doc:?}");
    assert_eq!(steps[0].at("reason").kind(), "string");
    assert_eq!(steps[0].at("utc").str(), Some("2026-11-19T03:00:00Z"));
}

/// A text argument that is not UTF-8 (types.md 2절): where base already refuses it — an option with closed choices — the
/// refusal is a usage error, exit 2 with nothing on stdout; the same command line with a valid choice is not exit 2.
#[test]
fn non_utf8_choice_arguments_are_usage_errors() {
    use std::ffi::{OsStr, OsString};
    use std::os::unix::ffi::OsStrExt;
    let s = scratch("nonutf8");
    let node = s.0.join("node");
    std::fs::create_dir_all(&node).unwrap();
    let (n, r) = (node.to_str().unwrap(), s.0.to_str().unwrap());
    for (launcher, args, option, valid) in [
        (
            "node_blackbox/blackbox_session.sh",
            vec!["--node-dir", n, "budget-defaults"],
            "--field",
            "ttl_s",
        ),
        (
            "staleness_gate.sh",
            vec!["--repo", r],
            "--topology",
            "single",
        ),
    ] {
        let run = |value: &OsStr| {
            let mut all: Vec<OsString> = args.iter().map(OsString::from).collect();
            all.extend([OsString::from(option), value.to_os_string()]);
            Command::new("bash")
                .arg(skill().join("scripts").join(launcher))
                .args(&all)
                .env("TERRAFORMING_BIN_DIR", bin_dir())
                .env_remove("TERRAFORMING_OWNER_PATH")
                .current_dir(&s.0)
                .stdin(Stdio::null())
                .output()
                .unwrap()
        };
        let out = run(OsStr::from_bytes(b"\xff"));
        assert_eq!(
            (out.status.code(), text(&out.stdout).as_str()),
            (Some(2), ""),
            "{launcher} {option}: {}",
            text(&out.stderr)
        );
        assert_ne!(
            run(OsStr::new(valid)).status.code(),
            Some(2),
            "{launcher} {option} {valid}"
        );
    }
}

/// The header line of the collector's sample CSV (`blackbox_collect.rs:22-23` `CSV_HEADER`): the column order the readers
/// count on.
const SAMPLES_HEADER: &str =
    "ts,mem_avail,mem_rate,gpu_temp,gpu_pwr,gpu_sm,gpu_util,gpu_mem,load1,ctr_n,soc_temp,soc_zone";

/// The UTC calendar date `YYYY-MM-DD` of an epoch second (days to a proleptic Gregorian date).
fn utc_date(epoch: i64) -> String {
    let z = epoch.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// An executable script at `to`, made by `install -m 0755` from a source file beside it: a file this process wrote and
/// executes at once could still be open for writing in a child another check thread is forking (ETXTBSY).
fn install_script(to: &Path, body: &str) {
    let source = to.with_file_name(format!(
        ".{}.source",
        to.file_name().unwrap().to_str().unwrap()
    ));
    write(&source, body);
    let st = Command::new(on_path("install"))
        .arg("-m")
        .arg("0755")
        .arg(&source)
        .arg(to)
        .stdin(Stdio::null())
        .status()
        .unwrap();
    assert!(st.success(), "install {to:?}");
}

/// The collector's sample CSV (consumers.md 「영속 데이터」 블랙박스 표본 CSV) `<node_dir>/samples/<UTC date>.csv`: the header
/// line once, then one row per sample, written and flushed at once. Its readers: thermal_watchdog.sh:127-143 (`tail -c
/// 8192 | awk -F,` keeps the last row whose first field is an integer and that has at least five fields, and takes the
/// fifth, `gpu_pwr`, for its kill rule), :503 (today's file by its UTC date), install_node_blackbox.sh:659-664 (today's
/// file is not empty), verify_node_blackbox.sh:315-320 (the file grows between two looks). The collector runs as its unit
/// runs it (the ELF, cwd `/`, no owner path, `--node-dir D --interval 1`) with `--max-samples 3 --no-kmsg`; `PATH` holds
/// `date`, `sleep` and a stand-in `nvidia-smi` script in the scratch directory that prints one query row a second (so
/// `gpu_pwr` has a value) and no container tool (`ctr_n` stays empty). While it runs the file is looked at: every look
/// ends with a whole line and the rows come one by one. The watchdog's own `tp_read_gpu` function, taken from the
/// script, reads the file.
#[test]
fn blackbox_samples_csv_rows_the_watchdog_reads() {
    let s = scratch("samples");
    let tools = s.0.join("tools");
    std::fs::create_dir_all(&tools).unwrap();
    for name in ["date", "sleep"] {
        std::os::unix::fs::symlink(on_path(name), tools.join(name)).unwrap();
    }
    // the query stream the collector reads (`--query-gpu=… --format=csv,noheader,nounits -l 1`), one row a second
    install_script(
        &tools.join("nvidia-smi"),
        "#!/bin/sh\nwhile :; do echo '45, 87.25, 2400, 50, 1234'; sleep 1; done\n",
    );
    let node = s.0.join("node");
    std::fs::create_dir_all(&node).unwrap();
    let samples = node.join("samples");
    let err = s.0.join("collect.err");
    let mut child = Command::new(env!("CARGO_BIN_EXE_blackbox_collect"))
        .args([
            "--node-dir",
            node.to_str().unwrap(),
            "--interval",
            "1",
            "--max-samples",
            "3",
            "--no-kmsg",
        ])
        .current_dir("/")
        .env_remove("TERRAFORMING_OWNER_PATH")
        .env_remove("TERRAFORMING_BIN_DIR")
        .env("PATH", &tools)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(std::fs::File::create(&err).unwrap())
        .spawn()
        .unwrap();
    let files = || -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = std::fs::read_dir(&samples)
            .map(|rd| rd.flatten().map(|e| e.path()).collect())
            .unwrap_or_default();
        v.sort();
        v
    };
    let errors = || std::fs::read_to_string(&err).unwrap_or_default();
    // the looks while it runs: (lines of the sample files, every file ends with a whole line)
    let mut looks: Vec<(usize, bool)> = Vec::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let status = loop {
        if let Some(st) = child.try_wait().unwrap() {
            break st;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the collector did not end after three samples: {}",
            errors()
        );
        let texts: Vec<String> = files()
            .iter()
            .filter_map(|p| std::fs::read_to_string(p).ok())
            .collect();
        if !texts.is_empty() {
            looks.push((
                texts.iter().map(|t| t.lines().count()).sum(),
                texts.iter().all(|t| t.is_empty() || t.ends_with('\n')),
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    assert_eq!(status.code(), Some(0), "{}", errors());
    let files = files();
    assert!(!files.is_empty() && files.len() <= 2, "{files:?}");
    let mut rows: Vec<Vec<String>> = Vec::new();
    for file in &files {
        let body = std::fs::read_to_string(file).unwrap();
        let mut lines = body.lines();
        assert_eq!(lines.next(), Some(SAMPLES_HEADER), "{file:?}");
        let name = file.file_name().unwrap().to_str().unwrap();
        for line in lines {
            let fields: Vec<String> = line.split(',').map(str::to_string).collect();
            assert_eq!(fields.len(), 12, "{line:?}");
            assert!(
                !fields[0].is_empty() && fields[0].bytes().all(|b| b.is_ascii_digit()),
                "ts, an integer: {line:?}"
            );
            assert_eq!(
                format!("{}.csv", utc_date(fields[0].parse().unwrap())),
                name,
                "a row's file is the UTC date of its ts"
            );
            rows.push(fields);
        }
    }
    assert_eq!(rows.len(), 3, "{rows:?}");
    let last = rows.last().unwrap();
    assert_eq!(last[4], "87.25", "gpu_pwr is the fifth field: {last:?}");
    assert!(
        looks.iter().all(|(_, whole)| *whole),
        "a look saw a partial row: {looks:?}"
    );
    assert!(looks.windows(2).all(|w| w[0].0 <= w[1].0), "{looks:?}");
    let total = files.len() + rows.len();
    assert!(
        looks.iter().any(|(n, _)| *n > 0 && *n < total),
        "the rows came one at a time, each flushed: {looks:?}"
    );
    // thermal_watchdog.sh's reader, the function itself: GPU_TS = the last row's ts, GPU_PWR_DW = round(gpu_pwr × 10)
    let script =
        std::fs::read_to_string(skill().join("scripts/node_blackbox/thermal_watchdog.sh")).unwrap();
    let lines: Vec<&str> = script.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.starts_with("tp_read_gpu(){"))
        .expect("tp_read_gpu in thermal_watchdog.sh");
    let end = (start..lines.len()).find(|&i| lines[i] == "}").unwrap();
    let watchdog = Command::new("bash")
        .arg("-c")
        .arg(format!(
            "ABSENT=-1\n{}\ntp_read_gpu \"$1\"\nprintf '%s %s\\n' \"$GPU_TS\" \"$GPU_PWR_DW\"\n",
            lines[start..=end].join("\n")
        ))
        .arg("thermal-watchdog")
        .arg(files.last().unwrap())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(
        text(&watchdog.stdout),
        format!("{} 873\n", last[0]),
        "{}",
        text(&watchdog.stderr)
    );
}

/// The topology this checkout's constitution declares (`.claude/rules/strategy.topology.md`, `**topology: <t>**`) — the
/// one render_sub_env's guard lets through in this tree.
fn rules_topology() -> String {
    let rules = std::fs::read_to_string(root().join(".claude/rules/strategy.topology.md")).unwrap();
    let head = "**topology: ";
    let at = rules.find(head).expect("the rules' topology header") + head.len();
    rules[at..].split("**").next().unwrap().trim().to_string()
}

/// A main manifest the renderer accepts (the fields its placeholders need), for `topology`.
fn render_main_manifest(topology: &str) -> String {
    format!("topology: {topology}\ncpu_arch: \"aarch64\"\ncuda_version: \"132\"\ngpus_per_node: 1\ngpu_model: \"TEST-GPU\"\nnas_model_path: /srv/test-models\nmodel_source: nas\ninterconnect:\n  type: RoCE v2\n  hca_devices: [testhca0, testhca1]\n  gid_index: 3\n  socket_iface: testif0\n  mtu: 9000\n  bandwidth_gbps: 200\n  platform_preset: test-preset\nnodes:\n  - role: main\n    host: main.invalid\n    ssh_user: tester\n    work_dir: /srv/tester/ws\n  - role: sub\n    host: sub.invalid\n    ssh_user: tester\n    work_dir: /srv/tester/ws\n")
}

/// The sub's manifest reaches the sub through render_sub_env (consumers.md 「영속 데이터」 manifest 줄 모양): `--topology T
/// --manifest M --sub-manifest S --out O --no-runtime-block` (the overlay only) refuses a sub manifest whose `self_role`
/// is not `sub` and writes S unchanged to `O/output/T/manifest.yaml`. Its top-level `self_role: sub` line is what
/// node_identity.sh:66 greps (`^self_role:`) and role-contract `identity.rs` reads the same way: `node_identity.sh
/// --resolve --repo O` prints `sub` — without the line its `- role: main` item (:92) would make the sub `main`. The tree
/// is this repository (the launcher finds it; T is its rules' topology); the manifests and O are in a scratch directory.
#[test]
fn render_sub_env_delivers_the_sub_manifest_self_role_line() {
    let s = scratch("submanifest");
    let t = rules_topology();
    let main = s.0.join("manifest.yaml");
    write(&main, &render_main_manifest(&t));
    let sub_text = format!("self_role: sub\ntopology: {t}\ncpu_arch: aarch64\ngpus_per_node: 1\ngpu_model: SUB-GPU\nmodel_source: managed\nnas_model_path: /srv/test-models\nterraforming:\n  complete: true\n  branch_verified: true\n  issued_by: main\nnodes:\n  - role: main\n    host: main.invalid\n  - role: sub\n    host: sub.invalid\n");
    let render = |sub: &Path, out: &Path| {
        launch(
            &s.0,
            "render_sub_env.sh",
            &[
                "--topology",
                &t,
                "--manifest",
                main.to_str().unwrap(),
                "--sub-manifest",
                sub.to_str().unwrap(),
                "--out",
                out.to_str().unwrap(),
                "--no-runtime-block",
            ],
        )
    };
    let delivered = |out: &Path| out.join("output").join(&t).join("manifest.yaml");
    let bad = s.0.join("sub-main.yaml");
    write(&bad, &sub_text.replace("self_role: sub", "self_role: main"));
    let out_bad = s.0.join("out-bad");
    let out = render(&bad, &out_bad);
    assert!(
        !matches!(out.status.code(), Some(0) | Some(5)),
        "{:?}: {}",
        out.status,
        text(&out.stderr)
    );
    assert!(
        !delivered(&out_bad).exists(),
        "a refused sub manifest is not delivered"
    );
    let sub = s.0.join("sub_manifest.yaml");
    write(&sub, &sub_text);
    let o = s.0.join("out");
    let out = render(&sub, &o);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let copy = std::fs::read_to_string(delivered(&o)).unwrap();
    assert_eq!(copy, sub_text, "the sub manifest is delivered as written");
    assert_eq!(
        copy.lines().find(|l| l.starts_with("self_role:")),
        Some("self_role: sub")
    );
    let resolved = Command::new("bash")
        .arg(skill().join("scripts/node_blackbox/node_identity.sh"))
        .args(["--resolve", "--repo", o.to_str().unwrap()])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(
        (resolved.status.code(), text(&resolved.stdout).trim()),
        (Some(0), "sub"),
        "{}",
        text(&resolved.stderr)
    );
}

/// relay keeps its report check's refusal (types.md 4절 2, 거부 보존): before `relay --continue --apply --context-id <ctx>`
/// reuses a completed attempt's report (`campaigns/<id>/relay/<ctx>.reports/attempt-NN.json`, `{"report": …}`,
/// dispatch.rs:241-330) it checks the report against `sub_node/task-report.schema.json` (`additionalProperties: false` at
/// the top, `:7`, and in its objects). A key outside the schema — at the top or in `self_verification` — is a refusal:
/// exit 1, `HANDOFF_RESULT_UNAVAILABLE` on stderr, nothing on stdout, the ledger's bytes unchanged. The same report
/// without the key is reused: exit 0, stdout `action` `reuse_completed_result` with the report as stored, and the ledger
/// records `handoff_reuse`. The tree holds copies of what relay reads.
#[test]
fn relay_refuses_a_report_outside_the_task_report_schema() {
    let s = scratch("relay-report");
    let tree = s.0.join("tree");
    for rel in [
        ".claude/skills/terraforming_node/scripts/relay.sh",
        ".claude/skills/terraforming_node/scripts/agent_control.sh",
        ".claude/schemas/agent-control-request.schema.json",
        ".claude/skills/terraforming_node/sub_node/task-report.schema.json",
    ] {
        let to = tree.join(rel);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(root().join(rel), &to).unwrap();
    }
    write(&tree.join("campaigns/ACTIVE"), "c1\n");
    let ledger = tree.join("campaigns/c1/relay/ctx-1.json");
    write(
        &ledger,
        r#"{"schema_version": 1, "context_id": "ctx-1", "campaign_id": "c1", "campaign_node": "sub", "topology": "single", "revision": 2, "attempts": [{"attempt": 1, "status": "completed", "control_status": "completed", "sub_reported": true, "context_id_reported": "ctx-1", "campaign_id_reported": "c1", "report_path": "campaigns/c1/relay/ctx-1.reports/attempt-01.json", "running_services": [], "provider": "claude_code", "session_id": "s-1", "execution_started": true, "reason_codes": [], "budget_outcome": "within_budget"}]}"#,
    );
    let report = r#"{"task_id": "task-1", "context_id": "ctx-1", "turn": 1, "phase": "publish", "status": "completed", "node_id": "sub", "campaign_id": "c1", "running_services": [], "self_verification": {"checks_run": ["identity", "schema_valid"], "identity_ok": true}, "artifacts": [{"kind": "report", "path": "docs/testlog/result.md"}]}"#;
    let stored = tree.join("campaigns/c1/relay/ctx-1.reports/attempt-01.json");
    let relay = || {
        Command::new("bash")
            .arg(tree.join(".claude/skills/terraforming_node/scripts/relay.sh"))
            .args(["--continue", "--apply", "--context-id", "ctx-1"])
            .env("TERRAFORMING_BIN_DIR", bin_dir())
            .env_remove("TERRAFORMING_OWNER_PATH")
            .env("GIT_CEILING_DIRECTORIES", std::env::temp_dir())
            .current_dir(&s.0)
            .stdin(Stdio::null())
            .output()
            .unwrap()
    };
    for (what, outside) in [
        (
            "a top-level key",
            report.replace(r#""artifacts""#, r#""extra_key": 1, "artifacts""#),
        ),
        (
            "a key in self_verification",
            report.replace(
                r#""identity_ok": true"#,
                r#""identity_ok": true, "extra_key": 1"#,
            ),
        ),
    ] {
        assert_ne!(outside, report);
        write(&stored, &format!("{{\"report\": {outside}}}\n"));
        let before = std::fs::read(&ledger).unwrap();
        let out = relay();
        assert_eq!(out.status.code(), Some(1), "{what}: {}", text(&out.stderr));
        assert!(
            text(&out.stderr).contains("HANDOFF_RESULT_UNAVAILABLE"),
            "{what}: {}",
            text(&out.stderr)
        );
        assert_eq!(text(&out.stdout), "", "{what}");
        assert_eq!(
            std::fs::read(&ledger).unwrap(),
            before,
            "{what}: the ledger is not rewritten"
        );
    }
    write(&stored, &format!("{{\"report\": {report}}}\n"));
    let out = relay();
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let doc = Json::parse(&text(&out.stdout)).unwrap();
    assert_eq!(doc.at("action").str(), Some("reuse_completed_result"));
    assert!(
        doc.at("report").same(&Json::parse(report).unwrap()),
        "{doc:?}"
    );
    let led = Json::parse(&std::fs::read_to_string(&ledger).unwrap()).unwrap();
    assert_eq!(
        led.at("handoff_reuse").at("action").str(),
        Some("reuse_completed_result"),
        "{led:?}"
    );
}

/// The source text policy-predicates reads in this crate (consumers.md 「소스 본문·경로 결합」; its `claims_text` goldens are in
/// the M baseline failure set, so its gate does not show a break here): the column-0 `pub fn render_tree(` item of
/// `render_sub_env.rs` (head to the first column-0 `}`, support.rs:493-501 `rust_top_fn`) holds the exact call that renders
/// `CLAUDE.template.md` as `CLAUDE.md` (SUB_GIT_LOCAL_ONLY C2); the file names `Agent_Card.template.json` and none of
/// `signing_key`·`a2a_signing`·`trusted_keys.json`·`runtime/a2a` (A2A_IDENTITY_PROOF_FAIL_CLOSED C4); `agent_control.rs`
/// holds `role/transport mismatch` (C3); `relay.rs` and `relay/*.rs` other than `selftest.rs` hold neither
/// `serve_library_requests` nor `library_request` (LIBRARY_GROUNDING_FAIL_CLOSED C5). And upstream
/// `branch_sync_selftest.rs:490-493` finds its guard-call anchor in `render_sub_env.rs` exactly once.
#[test]
fn policy_predicate_source_tokens_of_this_crate() {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let read = |p: &Path| std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{p:?}: {e}"));
    let render = read(&src_dir.join("render_sub_env.rs"));
    let lines: Vec<&str> = render.split('\n').collect();
    let start = lines
        .iter()
        .position(|l| l.starts_with("pub fn render_tree("))
        .expect("a column-0 `pub fn render_tree(` in render_sub_env.rs");
    let end = (start..lines.len())
        .find(|&i| lines[i] == "}" || (i == start && lines[i].ends_with('}')))
        .expect("the column-0 end of render_tree");
    let tree = lines[start..=end].join("\n");
    assert!(
        tree.contains(r#"render_file("CLAUDE.template.md", "CLAUDE.md", "md", &mut produced)"#),
        "render_tree renders the sub's CLAUDE.md"
    );
    assert!(render.contains("Agent_Card.template.json"));
    for token in [
        "signing_key",
        "a2a_signing",
        "trusted_keys.json",
        "runtime/a2a",
    ] {
        assert!(!render.contains(token), "render_sub_env.rs names {token}");
    }
    assert_eq!(
        render
            .matches("    let mut mismatch = topology_rules_mismatch(ctx, &topology, None);\n")
            .count(),
        1
    );
    assert!(read(&src_dir.join("agent_control.rs")).contains("role/transport mismatch"));
    let mut relay_files = vec![src_dir.join("relay.rs")];
    let mut module: Vec<PathBuf> = std::fs::read_dir(src_dir.join("relay"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|e| e == "rs")
                && p.file_name().is_some_and(|n| n != "selftest.rs")
        })
        .collect();
    module.sort();
    assert!(!module.is_empty());
    relay_files.extend(module);
    for file in &relay_files {
        let body = read(file);
        for token in ["serve_library_requests", "library_request"] {
            assert!(!body.contains(token), "{file:?} names {token}");
        }
    }
}

/// A usage error of the parsers: exit 2 with a usage block on stderr (`usage: …`).
fn usage_error(out: &Output) -> bool {
    out.status.code() == Some(2) && text(&out.stderr).to_ascii_lowercase().contains("usage")
}

/// The consumers' options no other check runs are taken by the parser: adversarial-benchmark `campaign_write` prints
/// `relay.sh --campaign-control reslice --campaign-id <c> --node <n>` for the agent (campaign_write_cases.rs:677), and
/// sync_to_sub.sh:239·:1376-1380 runs `render_sub_env.sh --topology <t> --tracked-list <f> [--hint-write-scopes <f>]`.
/// Each line is not a usage error (a domain answer passes: relay asks for `--context-id`, exit 1; render_sub_env finds no
/// manifest, exit 3), while the same line with one of its options renamed is one, with nothing on stdout (review r2 C-N7:
/// an argument probe, not the `--help` listing). relay runs in a tree of copies; render_sub_env in this tree (its guard
/// reads the rules) with the files in a scratch directory.
#[test]
fn consumer_options_are_taken_by_the_parser() {
    let s = scratch("probe");
    let tree = s.0.join("tree");
    for rel in [
        ".claude/skills/terraforming_node/scripts/relay.sh",
        ".claude/skills/terraforming_node/scripts/agent_control.sh",
        ".claude/schemas/agent-control-request.schema.json",
    ] {
        let to = tree.join(rel);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(root().join(rel), &to).unwrap();
    }
    let relay = tree.join(".claude/skills/terraforming_node/scripts/relay.sh");
    let render = skill().join("scripts/render_sub_env.sh");
    let t = rules_topology();
    let (absent, list, scopes) = (
        s.0.join("absent.yaml"),
        s.0.join("tracked.list"),
        s.0.join("scopes.json"),
    );
    write(&list, "CLAUDE.md\n");
    write(&scopes, "{}\n");
    let run = |launcher: &Path, args: &[&str]| {
        Command::new("bash")
            .arg(launcher)
            .args(args)
            .env("TERRAFORMING_BIN_DIR", bin_dir())
            .env_remove("TERRAFORMING_OWNER_PATH")
            .env("GIT_CEILING_DIRECTORIES", std::env::temp_dir())
            .current_dir(&s.0)
            .stdin(Stdio::null())
            .output()
            .unwrap()
    };
    let probes: [(&Path, Vec<&str>, &[&str]); 2] = [
        (
            &relay,
            vec![
                "--campaign-control",
                "reslice",
                "--campaign-id",
                "c1",
                "--node",
                "sub",
            ],
            &["--campaign-control", "--campaign-id", "--node"],
        ),
        (
            &render,
            vec![
                "--topology",
                &t,
                "--manifest",
                absent.to_str().unwrap(),
                "--tracked-list",
                list.to_str().unwrap(),
                "--hint-write-scopes",
                scopes.to_str().unwrap(),
            ],
            &["--tracked-list", "--hint-write-scopes"],
        ),
    ];
    for (launcher, line, words) in probes {
        let out = run(launcher, &line);
        assert!(
            !usage_error(&out),
            "{line:?} is a usage error:\n{}",
            text(&out.stderr)
        );
        for word in words {
            let mut control = line.clone();
            let at = control.iter().position(|w| w == word).unwrap();
            control[at] = "--contract-no-such-option";
            let out = run(launcher, &control);
            assert!(
                usage_error(&out),
                "{control:?}: an unknown option must be a usage error: {:?}\n{}",
                out.status,
                text(&out.stderr)
            );
            assert_eq!(text(&out.stdout), "", "{control:?}");
        }
    }
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
            let mut r = Reader {
                b: text.as_bytes(),
                i: 0,
            };
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
            self.get(key)
                .unwrap_or_else(|| panic!("key {key:?} missing in {self:?}"))
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
        /// The same parsed value, object keys in any order (key order is not part of an L2 value).
        pub fn same(&self, other: &Json) -> bool {
            match (self, other) {
                (Json::Obj(a), Json::Obj(b)) => {
                    a.len() == b.len()
                        && a.iter()
                            .all(|(k, v)| b.iter().any(|(k2, v2)| k == k2 && v.same(v2)))
                }
                (Json::Arr(a), Json::Arr(b)) => {
                    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.same(y))
                }
                _ => self == other,
            }
        }
        /// A list of strings (e.g. `reason_codes`), failing the check on any other shape.
        pub fn strs(&self) -> Vec<String> {
            let items = self
                .arr()
                .unwrap_or_else(|| panic!("not an array: {self:?}"));
            items
                .iter()
                .map(|v| {
                    v.str()
                        .unwrap_or_else(|| panic!("not a string: {v:?}"))
                        .to_string()
                })
                .collect()
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
                    while self.i < self.b.len()
                        && matches!(
                            self.b[self.i],
                            b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9'
                        )
                    {
                        self.i += 1;
                    }
                    let t = std::str::from_utf8(&self.b[start..self.i])
                        .unwrap()
                        .to_string();
                    t.parse::<f64>()
                        .map_err(|e| format!("bad number {t:?}: {e}"))?;
                    Ok(Json::Num(t))
                }
                Some(c) => Err(format!("unexpected byte {c:#x} at {}", self.i)),
            }
        }
        fn hex4(&mut self) -> Result<u32, String> {
            let h = self.b.get(self.i..self.i + 4).ok_or("short \\u escape")?;
            let v = u32::from_str_radix(std::str::from_utf8(h).map_err(|e| e.to_string())?, 16)
                .map_err(|e| e.to_string())?;
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
