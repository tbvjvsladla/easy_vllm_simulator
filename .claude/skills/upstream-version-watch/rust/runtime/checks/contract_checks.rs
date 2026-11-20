//! Contract checks of the upstream-runtime bins (plan 26111805 「보존 계약」, cell c21b): what their consumers read
//! (runtime-support/contract/consumers.tsv L1·L2 rows of this crate) —
//! - every consumer starts a bin through its launcher (`scripts/*.sh`, `terraforming_node/scripts/node_role_contract.sh`);
//!   the launchers honour `EASY_VLLM_UPSTREAM_RUNTIME_ROOT` (sync_to_sub.sh, sync_branches.sh) and refuse an absent
//!   executable with exit 2; the documents and consumers name subcommands and long options;
//! - policy-distribution `checks.rs`: the `--help` loop and the self-tests need exit 0; smoke_clone.sh greps
//!   `render_dockerfile.sh --self-test`;
//! - sync_branches.sh·sync_to_sub.sh·multinode_serve_smoke.sh·single_serve_down.sh·container_inventory.sh: the
//!   `inline_json` lines (TAB fields, one word, integers, gate-shaped JSON, the inventory and serve-proof JSON);
//! - multinode_serve_smoke.sh·single_serve_up.sh·run_bench.sh: `budget_preflight --json` keys read by `sed`, exit 4,
//!   `--render`, `--floor-mib --bench-budget-mib`;
//! - terraforming `bootstrap_canary.rs`·sync_to_sub.sh: `node_role_contract evaluate --field --format value|json`;
//! - policy-predicates `claims_owner.rs`·`claims_tf.rs`, hint-publisher `a1.rs`·`a2.rs`, sync_to_sub.sh:
//!   `render_dockerfile` envfiles (KEY=VALUE lines), `--env-tier` JSON, the Flag gate exit 4, `--out` files;
//! - hint-publisher `a2.rs`·`a3/collect.rs`, policy-predicates `claims_owner.rs`, multinode_serve_smoke.sh:
//!   `slave_forward prefix|json`, `forwards[]`, `sub_recipe`, `[slave-forward] FAIL(<code>):` on stderr;
//! - policy-predicates `claims_text.rs`·`runtime_patch.rs`, sync_to_sub.sh: `validate_runtime_patch stamp|verify`;
//! - closing_sequence.sh·sync_branches.sh: `layer_ledger` subcommands, JSON on stdout, plain lines, exit codes
//!   (`validate` 0 `valid` / 5 `rejected`, `divergence` 0 `clean`), and `append`·`merge` keep the ledger's unknown keys;
//! - closing_sequence.sh: `push_branches --repo --remote --branch…` exit 0 / non-zero;
//! - sync_to_sub.sh: the main's ELFs it delivers to the sub (`SUB_ELF_REL`, `SUB_ELF_NAMES`) and runs there — `sub_runner`
//!   `prune-empty-dirs`·`validate-names`·`retirement-scan` (argv, stdin, stdout lines, exit codes; policy-distribution's
//!   source tokens), `regen_build_patches_src unbundle|verify --root`; the main's `native_multinode_serve`: `native_remote -`
//!   (base64 JSON request on stdin, the eleven operations, result keys, refusals);
//! - text arguments that are not UTF-8: exit 2 where base refuses them (closed choices).
//!
//! The bins run for real in scratch directories and throwaway local git repositories (a "remote" is a bare repository
//! on disk). Values (L2) and exit codes, option names, line shapes and codes (L1) are fixed; message texts are not (L3).
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use json::Json;

/// `.claude/skills/upstream-version-watch` (this crate is its `rust/runtime/`).
fn skill() -> PathBuf {
    std::fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap()
}

/// The repository root (`.claude/skills/upstream-version-watch` → three levels up).
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
        "upstream-contract-{tag}-{}-{n}",
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

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args([
            "-c",
            "user.email=contract@example.invalid",
            "-c",
            "user.name=contract",
            "-c",
            "commit.gpgSign=false",
        ])
        .arg("-C")
        .arg(dir)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {}", text(&out.stderr));
    text(&out.stdout)
}

/// The launchers (repository-relative) and the bin each one starts (`target/release/<bin>` of the runtime root).
const LAUNCHERS: [(&str, &str); 22] = [
    (
        ".claude/skills/upstream-version-watch/scripts/budget_preflight.sh",
        "budget_preflight",
    ),
    (
        ".claude/skills/upstream-version-watch/scripts/check_smoke_model.sh",
        "check_smoke_model",
    ),
    (
        ".claude/skills/upstream-version-watch/scripts/check_spec_layout.sh",
        "check_spec_layout",
    ),
    (
        ".claude/skills/upstream-version-watch/scripts/classify_failure.sh",
        "classify_failure",
    ),
    (
        ".claude/skills/upstream-version-watch/scripts/inline_json.sh",
        "inline_json",
    ),
    (
        ".claude/skills/upstream-version-watch/scripts/judge_version_delta.sh",
        "judge_version_delta",
    ),
    (
        ".claude/skills/upstream-version-watch/scripts/layer_ledger.sh",
        "layer_ledger",
    ),
    (
        ".claude/skills/upstream-version-watch/scripts/native_multinode_serve.sh",
        "native_multinode_serve",
    ),
    (
        ".claude/skills/upstream-version-watch/scripts/native_wheelhouse.sh",
        "native_wheelhouse",
    ),
    (
        ".claude/skills/upstream-version-watch/scripts/push_branches.sh",
        "push_branches",
    ),
    (
        ".claude/skills/upstream-version-watch/scripts/regen_build_patches_src.sh",
        "regen_build_patches_src",
    ),
    (
        ".claude/skills/upstream-version-watch/scripts/regen_requirements.sh",
        "regen_requirements",
    ),
    (
        ".claude/skills/upstream-version-watch/scripts/render_dockerfile.sh",
        "render_dockerfile",
    ),
    (
        ".claude/skills/upstream-version-watch/scripts/render_native_triplet.sh",
        "render_native_triplet",
    ),
    (
        ".claude/skills/upstream-version-watch/scripts/resolve_build_track.sh",
        "resolve_build_track",
    ),
    (
        ".claude/skills/upstream-version-watch/scripts/resolve_ngc_tag.sh",
        "resolve_ngc_tag",
    ),
    (
        ".claude/skills/upstream-version-watch/scripts/resolve_torch_pin.sh",
        "resolve_torch_pin",
    ),
    (
        ".claude/skills/upstream-version-watch/scripts/resolve_wheel.sh",
        "resolve_wheel",
    ),
    (
        ".claude/skills/upstream-version-watch/scripts/selftest_branch_sync.sh",
        "selftest_branch_sync",
    ),
    (
        ".claude/skills/upstream-version-watch/scripts/slave_forward.sh",
        "slave_forward",
    ),
    (
        ".claude/skills/upstream-version-watch/scripts/validate_runtime_patch.sh",
        "validate_runtime_patch",
    ),
    (
        ".claude/skills/terraforming_node/scripts/node_role_contract.sh",
        "node_role_contract",
    ),
];

/// This build's executable of a launcher's bin.
fn exe(bin: &str) -> &'static str {
    match bin {
        "budget_preflight" => env!("CARGO_BIN_EXE_budget_preflight"),
        "check_smoke_model" => env!("CARGO_BIN_EXE_check_smoke_model"),
        "check_spec_layout" => env!("CARGO_BIN_EXE_check_spec_layout"),
        "classify_failure" => env!("CARGO_BIN_EXE_classify_failure"),
        "inline_json" => env!("CARGO_BIN_EXE_inline_json"),
        "judge_version_delta" => env!("CARGO_BIN_EXE_judge_version_delta"),
        "layer_ledger" => env!("CARGO_BIN_EXE_layer_ledger"),
        "native_multinode_serve" => env!("CARGO_BIN_EXE_native_multinode_serve"),
        "native_wheelhouse" => env!("CARGO_BIN_EXE_native_wheelhouse"),
        "push_branches" => env!("CARGO_BIN_EXE_push_branches"),
        "regen_build_patches_src" => env!("CARGO_BIN_EXE_regen_build_patches_src"),
        "regen_requirements" => env!("CARGO_BIN_EXE_regen_requirements"),
        "render_dockerfile" => env!("CARGO_BIN_EXE_render_dockerfile"),
        "render_native_triplet" => env!("CARGO_BIN_EXE_render_native_triplet"),
        "resolve_build_track" => env!("CARGO_BIN_EXE_resolve_build_track"),
        "resolve_ngc_tag" => env!("CARGO_BIN_EXE_resolve_ngc_tag"),
        "resolve_torch_pin" => env!("CARGO_BIN_EXE_resolve_torch_pin"),
        "resolve_wheel" => env!("CARGO_BIN_EXE_resolve_wheel"),
        "selftest_branch_sync" => env!("CARGO_BIN_EXE_selftest_branch_sync"),
        "slave_forward" => env!("CARGO_BIN_EXE_slave_forward"),
        "validate_runtime_patch" => env!("CARGO_BIN_EXE_validate_runtime_patch"),
        "node_role_contract" => env!("CARGO_BIN_EXE_node_role_contract"),
        other => panic!("no bin {other}"),
    }
}

/// A runtime root (`EASY_VLLM_UPSTREAM_RUNTIME_ROOT`) whose `target/release/<bin>` are this build's executables.
fn runtime_root(s: &Scratch) -> PathBuf {
    let rt = s.0.join("runtime");
    let rel = rt.join("target/release");
    std::fs::create_dir_all(&rel).unwrap();
    for (_, bin) in LAUNCHERS {
        std::os::unix::fs::symlink(exe(bin), rel.join(bin)).unwrap();
    }
    rt
}

/// `bash <launcher> <args>` with the runtime root override and the given stdin.
fn launch_in(rt: &Path, cwd: &Path, launcher: &str, args: &[&str], stdin: Option<&str>) -> Output {
    use std::io::Write;
    let mut c = Command::new("bash");
    c.arg(root().join(launcher))
        .args(args)
        .env("EASY_VLLM_UPSTREAM_RUNTIME_ROOT", rt)
        .current_dir(cwd);
    c.stdin(if stdin.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    })
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    let mut child = c.spawn().unwrap();
    if let Some(input) = stdin {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
    }
    child.wait_with_output().unwrap()
}

fn launch(rt: &Path, cwd: &Path, launcher: &str, args: &[&str]) -> Output {
    launch_in(rt, cwd, launcher, args, None)
}

fn scripts(name: &str) -> String {
    format!(".claude/skills/upstream-version-watch/scripts/{name}")
}

/// Every launcher exists; an empty `EASY_VLLM_UPSTREAM_RUNTIME_ROOT` and a root without the executable are exit 2 with
/// nothing on stdout; a root holding the build starts the bin. policy-distribution's `--help` loop needs exit 0.
#[test]
fn launchers_and_the_runtime_root_override() {
    let s = scratch("launch");
    let rt = runtime_root(&s);
    let empty_root = s.0.join("empty-root");
    std::fs::create_dir_all(&empty_root).unwrap();
    for (launcher, _) in LAUNCHERS {
        assert!(root().join(launcher).is_file(), "{launcher}");
        for bad in [Path::new(""), empty_root.as_path()] {
            let out = launch(bad, &s.0, launcher, &["--help"]);
            assert_eq!(
                out.status.code(),
                Some(2),
                "{launcher} with runtime root {bad:?}"
            );
            assert_eq!(text(&out.stdout), "", "{launcher}");
        }
        if launcher.ends_with("/inline_json.sh") || launcher.ends_with("/selftest_branch_sync.sh") {
            continue; // no help: a subcommand reader, a self-test driver
        }
        let out = launch(&rt, &s.0, launcher, &["--help"]);
        assert_eq!(
            out.status.code(),
            Some(0),
            "{launcher} --help: {}",
            text(&out.stderr)
        );
    }
    let out = launch_in(
        &rt,
        &s.0,
        &scripts("inline_json.sh"),
        &["allowed"],
        Some("{\"allowed\": true}"),
    );
    assert_eq!(
        (out.status.code(), text(&out.stdout).as_str()),
        (Some(0), "true\n")
    );
}

/// The subcommands and long options the consumers and the documents name are part of the parser: `--help` (of the
/// launcher, or of the subcommand) is exit 0 and lists them.
#[test]
fn documented_subcommands_and_long_options() {
    let s = scratch("argv");
    let rt = runtime_root(&s);
    let table: [(&str, &[&str], &[&str]); 21] = [
        (
            "budget_preflight.sh",
            &[],
            &[
                "--json",
                "--declared-gmu-yaml",
                "--mem-total-mib",
                "--weights-mib",
                "--kv-mib",
                "--overhead-mib",
                "--render",
                "--floor-mib",
                "--bench-budget-mib",
                "--self-test",
            ],
        ),
        (
            "check_smoke_model.sh",
            &[],
            &["--repo", "--topology", "--emit-gate-params"],
        ),
        ("classify_failure.sh", &[], &["--log"]),
        (
            "judge_version_delta.sh",
            &[],
            &["--check-fixture", "--self-test", "--write-resolved"],
        ),
        (
            "layer_ledger.sh",
            &[],
            &[
                "sync-point",
                "divergence",
                "validate",
                "append",
                "merge",
                "emit-skeleton",
                "entry-id",
                "evidence-rels",
                "utc-to-kst",
                "--repo",
                "--branch",
                "--counterpart",
                "--ledger-ref",
                "--entry-file",
                "--from-ref",
                "--cwd",
                "--manifest",
                "--self-test",
            ],
        ),
        (
            "native_multinode_serve.sh",
            &[],
            &["up", "down", "client", "--self-test"],
        ),
        (
            "native_multinode_serve.sh",
            &["up"],
            &["--cell", "--run-id", "--apply", "--source-cell"],
        ),
        (
            "native_multinode_serve.sh",
            &["down"],
            &["--cell", "--run-id", "--apply"],
        ),
        (
            "native_wheelhouse.sh",
            &[],
            &["build", "verify", "--self-test"],
        ),
        (
            "push_branches.sh",
            &[],
            &["--repo", "--remote", "--branch", "--dry-run", "--self-test"],
        ),
        (
            "regen_build_patches_src.sh",
            &[],
            &["verify", "bundle", "unbundle", "derive", "--repo"],
        ),
        (
            "regen_requirements.sh",
            &[],
            &["--from-wheel-url", "--use-installed", "--self-test", "-o"],
        ),
        (
            "render_dockerfile.sh",
            &[],
            &[
                "--self-test",
                "--nccl-envfile",
                "--cluster-envfile",
                "--materialize-configs",
                "--materialize-env",
                "--topology",
                "--repo",
                "--template",
                "--canonical-kind",
                "--manifest",
                "--resolved",
                "-o",
                "--out",
                "--allow-unvalidated",
                "--env-tier",
            ],
        ),
        (
            "render_native_triplet.sh",
            &[],
            &["--apply", "--check", "--self-test"],
        ),
        (
            "resolve_build_track.sh",
            &[],
            &["--torch-pin", "--topology", "--sm-arch", "--self-test"],
        ),
        ("resolve_ngc_tag.sh", &[], &["--start", "--arch"]),
        ("resolve_torch_pin.sh", &[], &[]),
        ("resolve_wheel.sh", &[], &["--arch"]),
        (
            "slave_forward.sh",
            &[],
            &["prefix", "json", "default", "non-identity", "--self-test"],
        ),
        (
            "validate_runtime_patch.sh",
            &[],
            &[
                "stamp",
                "verify",
                "--patch",
                "--topology",
                "--config-dir",
                "--env-dir",
                "--resolution",
            ],
        ),
        (
            "../../terraforming_node/scripts/node_role_contract.sh",
            &["evaluate"],
            &["--manifest", "--topology", "--field", "--format"],
        ),
    ];
    for (launcher, sub, options) in table {
        let mut args: Vec<&str> = sub.to_vec();
        args.push("--help");
        let out = launch(&rt, &s.0, &scripts(launcher), &args);
        let help = format!("{}{}", text(&out.stdout), text(&out.stderr));
        assert_eq!(out.status.code(), Some(0), "{launcher} {sub:?}: {help}");
        for o in options {
            assert!(
                help.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
                    .any(|w| w == *o),
                "{launcher} {sub:?} lacks {o}:\n{help}"
            );
        }
    }
    let out = launch(
        &rt,
        &s.0,
        &scripts("budget_preflight.sh"),
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

/// policy-distribution runs these self-tests through the launchers and needs exit 0; smoke_clone.sh also greps the
/// merged output of `render_dockerfile.sh --self-test` for `self-test OK|render` (case-insensitive). A self-test that
/// prints one line per check keeps at least the checks it has at base (the second column; 0 = it prints no check lines)
/// and prints no failed check — a shrunken self-test still exits 0, so its exit code alone would not show it.
#[test]
fn consumer_self_tests_pass() {
    let s = scratch("selftest");
    let rt = runtime_root(&s);
    for (launcher, min_checks) in [
        (scripts("budget_preflight.sh"), 47),
        (scripts("layer_ledger.sh"), 0),
        (scripts("push_branches.sh"), 0),
        (scripts("render_dockerfile.sh"), 0),
        (scripts("slave_forward.sh"), 0),
        (
            ".claude/skills/terraforming_node/scripts/node_role_contract.sh".to_string(),
            33,
        ),
    ] {
        let out = launch(&rt, &s.0, &launcher, &["--self-test"]);
        let merged = format!("{}{}", text(&out.stdout), text(&out.stderr));
        assert_eq!(
            out.status.code(),
            Some(0),
            "{launcher} --self-test:\n{merged}"
        );
        let (passed, failed) = check_lines(&merged);
        assert!(passed >= min_checks && failed == 0, "{launcher} --self-test: {passed} passed (base {min_checks}), {failed} failed:\n{merged}");
        if launcher.ends_with("render_dockerfile.sh") {
            let low = merged.to_lowercase();
            assert!(
                low.contains("self-test ok") || low.contains("render"),
                "{merged}"
            );
        }
    }
}

/// `inline_json`, the readers the entry-point shells run (sync_branches.sh 313-326, sync_to_sub.sh 121,
/// multinode_serve_smoke.sh 67·338·1118·1131·1179, single_serve_down.sh 179, container_inventory.sh 43): their stdout lines
/// and the attestation and serve-proof files.
#[test]
fn inline_json_lines_for_the_shell_consumers() {
    let s = scratch("inline");
    let rt = runtime_root(&s);
    let ij = |args: &[&str], stdin: Option<&str>| {
        let out = launch_in(&rt, &s.0, &scripts("inline_json.sh"), args, stdin);
        (out.status.code(), text(&out.stdout))
    };
    // allowed: `true` only for a gate JSON whose allowed is true
    assert_eq!(
        ij(
            &["allowed"],
            Some(r#"{"allowed": true, "reason_codes": []}"#)
        ),
        (Some(0), "true\n".into())
    );
    assert_eq!(
        ij(&["allowed"], Some(r#"{"allowed": false}"#)),
        (Some(0), "false\n".into())
    );
    assert_eq!(
        ij(&["allowed"], Some("not json")),
        (Some(0), "false\n".into())
    );
    // gate-unreadable-*: a gate-shaped deny JSON with the unreadable reason code
    for (args, code, mode) in [
        (
            &["gate-unreadable-branches", "promotion", "m.yaml", "3"][..],
            "SYNC_BRANCHES_GATE_OUTPUT_UNREADABLE",
            Json::Str("promotion".into()),
        ),
        (
            &["gate-unreadable-sub"],
            "SYNC_TO_SUB_GATE_OUTPUT_UNREADABLE",
            Json::Null,
        ),
    ] {
        let (rc, out) = ij(args, None);
        assert_eq!(rc, Some(0), "{args:?}");
        let doc = Json::parse(&out).unwrap();
        assert_eq!(doc.at("allowed"), &Json::Bool(false));
        assert_eq!(doc.at("exit_code"), &Json::Num("2".into()));
        assert_eq!(doc.at("reason_codes").strs(), [code]);
        assert_eq!(doc.at("mode"), &mode);
        assert_eq!(doc.at("action").kind(), "string");
    }
    // divergence-summary: status, basis, base (12 chars), sync_point.status, absorbed count
    let d = r#"{"status": "clean", "basis": "ledger", "base": "0123456789abcdef", "sync_point": {"status": "ok"}, "absorbed": ["a", "b"]}"#;
    assert_eq!(
        ij(&["divergence-summary"], Some(d)),
        (Some(0), "clean\tledger\t0123456789ab\tok\t2\n".into())
    );
    let (_, line) = ij(&["divergence-summary"], Some("{}"));
    assert_eq!(
        line.trim_end_matches('\n').split('\t').count(),
        5,
        "{line:?}"
    );
    // parity-summary: `verdict\ttopology\tcodes\tlegs(checked/4)` then one reason per line
    let p = r#"{"verdict": "FAIL", "topology": "single", "reason_codes": ["EXPECT_MISMATCH"], "legs_judged": ["branch", "manifest"], "legs_checked": 2, "reasons": ["r1", "r2"]}"#;
    assert_eq!(
        ij(&["parity-summary"], Some(p)),
        (
            Some(0),
            "FAIL\tsingle\tEXPECT_MISMATCH\tbranch,manifest(2/4)\nr1\nr2\n".into()
        )
    );
    // delivery-source: one word (unknown when absent)
    assert_eq!(
        ij(
            &["delivery-source"],
            Some(r#"{"delivery_plane": {"value": "active", "source": "manifest"}}"#)
        ),
        (Some(0), "manifest\n".into())
    );
    assert_eq!(
        ij(&["delivery-source"], Some("{}")),
        (Some(0), "unknown\n".into())
    );
    // propagation-fields: status, topology, node_id, ssh_host, work_dir, planes (comma-joined)
    let g = r#"{"propagation_scope": {"status": "approved", "topology": "multi", "node_id": "n1", "ssh_host": "h", "work_dir": "/w", "planes": ["a", "b"]}}"#;
    assert_eq!(
        ij(&["propagation-fields"], Some(g)),
        (Some(0), "approved\tmulti\tn1\th\t/w\ta,b\n".into())
    );
    // chat-verdict: `verdict clen rlen finish_reason`; completions-len: an integer
    let chat = s.0.join("chat.json");
    write(
        &chat,
        r#"{"choices": [{"message": {"content": " hello "}, "finish_reason": "stop"}]}"#,
    );
    assert_eq!(
        ij(&["chat-verdict", chat.to_str().unwrap()], None),
        (Some(0), "content 5 0 stop\n".into())
    );
    write(
        &chat,
        r#"{"choices": [{"message": {"content": "", "reasoning_content": "abc"}, "finish_reason": "length"}]}"#,
    );
    assert_eq!(
        ij(&["chat-verdict", chat.to_str().unwrap()], None),
        (Some(0), "reasoning-only 0 3 length\n".into())
    );
    write(
        &chat,
        r#"{"choices": [{"message": {"content": ""}, "finish_reason": "stop"}]}"#,
    );
    assert_eq!(
        ij(&["chat-verdict", chat.to_str().unwrap()], None)
            .1
            .split(' ')
            .next(),
        Some("empty")
    );
    write(&chat, "not json");
    assert_eq!(
        ij(&["chat-verdict", chat.to_str().unwrap()], None)
            .1
            .split(' ')
            .next(),
        Some("unparsable")
    );
    write(&chat, r#"{"choices": [{"text": " four "}]}"#);
    assert_eq!(
        ij(&["completions-len", chat.to_str().unwrap()], None),
        (Some(0), "4\n".into())
    );
    // campaign-budget <repo> <key>: the positive integer budgets[key] of the active campaign, nothing otherwise
    let repo = s.0.join("repo");
    write(&repo.join("campaigns/ACTIVE"), "camp-1\n");
    write(
        &repo.join("campaigns/camp-1/campaign.yaml"),
        r#"{"budgets": {"serve_turns": 7}}"#,
    );
    assert_eq!(
        ij(
            &["campaign-budget", repo.to_str().unwrap(), "serve_turns"],
            None
        ),
        (Some(0), "7\n".into())
    );
    write(&repo.join("campaigns/ACTIVE"), "_bootstrap\n");
    assert_eq!(
        ij(
            &["campaign-budget", repo.to_str().unwrap(), "serve_turns"],
            None
        ),
        (Some(0), String::new())
    );
    // open-sessions <node_dir>: one open session id per line
    let node = s.0.join("node");
    write(
        &node.join("sessions/s1.json"),
        r#"{"session_id": "s1", "stopped_utc": null}"#,
    );
    write(
        &node.join("sessions/s2.json"),
        r#"{"session_id": "s2", "stopped_utc": "2026-11-18T00:00:00Z"}"#,
    );
    write(
        &node.join("sessions/s3.json"),
        r#"{"session_id": "s3", "stopped_utc": null}"#,
    );
    assert_eq!(
        ij(&["open-sessions", node.to_str().unwrap()], None),
        (Some(0), "s1\ns3\n".into())
    );
    // inventory (env REPO TOPO FILTER FMT IMG_RAW PS_RAW): FMT=json keys
    let out = Command::new("bash")
        .arg(root().join(scripts("inline_json.sh")))
        .arg("inventory")
        .env("EASY_VLLM_UPSTREAM_RUNTIME_ROOT", &rt)
        .env("REPO", &repo)
        .env("TOPO", "single")
        .env("FILTER", "")
        .env("FMT", "json")
        .env(
            "IMG_RAW",
            "easy-vllm:x\tsha256:1\t<none>\t2026-11-18\t1GB\n",
        )
        .env("PS_RAW", "")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let doc = Json::parse(&text(&out.stdout)).unwrap();
    for key in [
        "schema_version",
        "topologies",
        "filter",
        "images",
        "running",
        "referenced_but_absent",
    ] {
        doc.at(key);
    }
    // serve-proof <out> <config> <ready> <http> <verdict> <evidence> <clen> <colen> <rlen> <fr> <image> <mc> <att>: a JSON file
    let proof = s.0.join("serve_proof_c1.json");
    let (rc, _) = ij(
        &[
            "serve-proof",
            proof.to_str().unwrap(),
            "cfg",
            "1",
            "200",
            "pass",
            "chat.content",
            "5",
            "",
            "0",
            "stop",
            "img:1",
            "mc",
            "",
        ],
        None,
    );
    assert_eq!(rc, Some(0));
    let doc = Json::parse(&std::fs::read_to_string(&proof).unwrap()).unwrap();
    assert_eq!(doc.at("kind").str(), Some("multinode_serve_proof"));
    assert_eq!(doc.at("ready"), &Json::Bool(true));
    assert_eq!(doc.at("health_http"), &Json::Num("200".into()));
    assert_eq!(doc.at("content_len"), &Json::Num("5".into()));
    // attestation-write <out> <config> <image> build "" <rows> <sf> <main_id> <sub_id>: the build-phase attestation file
    let rows = s.0.join("rows.tsv");
    write(
        &rows,
        "vllm_version\tmain\t0.1.0\t\nvllm_version\tsub\t0.1.0\t0.1.0\n",
    );
    let att = s.0.join("attest/attestation.json");
    std::fs::create_dir_all(att.parent().unwrap()).unwrap();
    let (rc, line) = ij(
        &[
            "attestation-write",
            att.to_str().unwrap(),
            "cfg",
            "img:1",
            "build",
            "",
            rows.to_str().unwrap(),
            "sf",
            "main-1",
            "sub-1",
        ],
        None,
    );
    assert_eq!(rc, Some(0));
    assert_eq!(line.lines().count(), 1, "{line:?}");
    let doc = Json::parse(&std::fs::read_to_string(&att).unwrap()).unwrap();
    assert_eq!(
        doc.at("kind").str(),
        Some("multinode_node_parity_attestation")
    );
    assert_eq!(doc.at("schema_version"), &Json::Num("2".into()));
    assert_eq!(doc.at("phase").str(), Some("build"));
    assert_eq!(doc.at("parity_blocking"), &Json::Bool(false));
    let checks = doc.at("checks").arr().unwrap();
    assert_eq!(checks.len(), 2);
    for c in checks {
        for key in ["axis", "node", "observed", "expected", "phase"] {
            assert_eq!(c.at(key).kind(), "string", "{key}");
        }
    }
    // an unknown subcommand is a usage error
    assert_eq!(ij(&["no-such-subcommand"], None).0, Some(2));
}

/// The `"key": <int>` sed of multinode_serve_smoke.sh:883 on the merged output.
fn sed_int(out: &str, key: &str) -> Option<i64> {
    let pat = format!("\"{key}\": ");
    out.lines().find_map(|l| {
        let rest = &l[l.find(&pat)? + pat.len()..];
        let n: String = rest
            .chars()
            .enumerate()
            .take_while(|(i, c)| c.is_ascii_digit() || (*i == 0 && *c == '-'))
            .map(|(_, c)| c)
            .collect();
        n.parse().ok()
    })
}

/// `budget_preflight --json --declared-gmu-yaml --mem-total-mib --weights-mib --kv-mib --overhead-mib` (2>&1): exit 0 or 4
/// (the watchdog would reject the declaration), one JSON object whose integer keys `floor_mib`, `arm_ceiling_mib`,
/// `overhead_max_mib`, `decl_min_ceiling_mib` the shells read with `sed` (`"key": <int>`); `--render gmu|reasons|summary`
/// reads that JSON on stdin; run_bench.sh: `--floor-mib --bench-budget-mib` exit 0 when the bench fits.
#[test]
fn budget_preflight_json_render_and_exit_4() {
    let s = scratch("preflight");
    let rt = runtime_root(&s);
    let serve = s.0.join("serve.yaml");
    write(&serve, "gpu-memory-utilization: 0.8\n");
    let pf = |weights: &str| {
        let out = Command::new("bash")
            .arg("-c")
            .arg("bash \"$0\" \"$@\" 2>&1")
            .arg(root().join(scripts("budget_preflight.sh")))
            .args([
                "--json",
                "--declared-gmu-yaml",
                serve.to_str().unwrap(),
                "--mem-total-mib",
                "131072",
                "--weights-mib",
                weights,
            ])
            .args(["--kv-mib", "10000", "--overhead-mib", "4000"])
            .env("EASY_VLLM_UPSTREAM_RUNTIME_ROOT", &rt)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        (out.status.code(), text(&out.stdout))
    };
    for (weights, want) in [("20000", Some(0)), ("125000", Some(4))] {
        let (rc, merged) = pf(weights);
        assert_eq!(rc, want, "{merged}");
        let doc = Json::parse(&merged).unwrap_or_else(|e| panic!("{e}: {merged}"));
        for key in [
            "floor_mib",
            "arm_ceiling_mib",
            "overhead_max_mib",
            "decl_min_ceiling_mib",
        ] {
            assert_eq!(doc.at(key).kind(), "number", "{key}");
            assert_eq!(
                sed_int(&merged, key)
                    .map(|v| Json::Num(v.to_string()))
                    .as_ref(),
                Some(doc.at(key)),
                "{key}: sed reads the same value"
            );
        }
        doc.at("reasons").strs();
        for render in ["gmu", "reasons", "summary"] {
            let out = launch_in(
                &rt,
                &s.0,
                &scripts("budget_preflight.sh"),
                &["--render", render],
                Some(&merged),
            );
            assert_eq!(
                out.status.code(),
                Some(0),
                "--render {render}: {}",
                text(&out.stderr)
            );
        }
        if want == Some(4) {
            assert!(!doc.at("reasons").strs().is_empty());
            let out = launch_in(
                &rt,
                &s.0,
                &scripts("budget_preflight.sh"),
                &["--render", "reasons"],
                Some(&merged),
            );
            assert!(!text(&out.stdout).trim().is_empty());
        }
    }
    let out = launch(
        &rt,
        &s.0,
        &scripts("budget_preflight.sh"),
        &["--floor-mib", "30000", "--bench-budget-mib", "10000"],
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
}

const SINGLE_WITH_SUB: &str = "topology: single\ngpus_per_node: 1\nterraforming:\n  complete: true\n  branch_verified: true\nnodes:\n  - role: main\n    host: main.invalid\n    ssh_user: u\n    work_dir: /w\n  - role: sub\n    host: sub.invalid\n    ssh_user: u\n    work_dir: /w\n";

/// `node_role_contract evaluate --manifest M --topology T --field F --format value|json` (terraforming
/// `bootstrap_canary.rs`, sync_to_sub.sh): exit 0; `json` prints `{"<field>": {"value", "source"}}`, or
/// `{"<field>": null}` when the field does not apply (the canary then falls back to the top level) — which one it is,
/// per manifest and field, is the table below; `value` prints the bare value (an empty line for null). sync_to_sub.sh
/// compares `delivery_plane` with `active` (a single sub, a2a-agent, is dormant; a multi sub, ray-worker, is active),
/// reads its source through `inline_json delivery-source`, and splits the `tool_plane` value on every character outside
/// `[A-Za-z0-9-]` into skill names (none = no such character) — they must be the json value's names.
#[test]
fn node_role_contract_evaluate_value_and_json() {
    let s = scratch("role");
    let rt = runtime_root(&s);
    let launcher = ".claude/skills/terraforming_node/scripts/node_role_contract.sh";
    let none = s.0.join("none.yaml");
    write(&none, "topology: single\ngpus_per_node: 1\nterraforming:\n  complete: true\n  branch_verified: true\nnodes: []\n");
    let sub = s.0.join("sub.yaml");
    write(&sub, SINGLE_WITH_SUB);
    let multi = s.0.join("multi.yaml");
    write(
        &multi,
        &SINGLE_WITH_SUB.replace("topology: single", "topology: multi"),
    );
    let eval = |m: &Path, topology: &str, field: &str, format: &str| {
        let out = launch(
            &rt,
            &s.0,
            launcher,
            &[
                "evaluate",
                "--manifest",
                m.to_str().unwrap(),
                "--topology",
                topology,
                "--field",
                field,
                "--format",
                format,
            ],
        );
        assert_eq!(
            out.status.code(),
            Some(0),
            "{field} {format}: {}",
            text(&out.stderr)
        );
        text(&out.stdout)
    };
    // (manifest, topology, field, the JSON type of `value`, or None when the field is null; `value` when it is a string)
    let table: [(&Path, &str, &str, Option<&str>, Option<&str>); 9] = [
        (
            &none,
            "single",
            "delivery_plane",
            Some("string"),
            Some("dormant"),
        ),
        (&none, "single", "tool_plane", Some("array"), None),
        (&none, "single", "sub_mode", None, None),
        (
            &sub,
            "single",
            "delivery_plane",
            Some("string"),
            Some("dormant"),
        ),
        (&sub, "single", "tool_plane", Some("array"), None),
        (
            &sub,
            "single",
            "sub_mode",
            Some("string"),
            Some("a2a-agent"),
        ),
        (
            &multi,
            "multi",
            "delivery_plane",
            Some("string"),
            Some("active"),
        ),
        (&multi, "multi", "tool_plane", Some("array"), None),
        (
            &multi,
            "multi",
            "sub_mode",
            Some("string"),
            Some("ray-worker"),
        ),
    ];
    for (m, topology, field, kind, want) in table {
        let doc = Json::parse(&eval(m, topology, field, "json")).unwrap();
        assert_eq!(doc.keys(), [field], "{m:?} {field}");
        let v = doc.at(field);
        let value = eval(m, topology, field, "value");
        let Some(kind) = kind else {
            assert_eq!(v, &Json::Null, "{m:?} {field}: null");
            assert_eq!(value.trim_end(), "", "{m:?} {field}: value of null");
            continue;
        };
        assert_eq!(v.keys().len(), 2, "{m:?} {field}: {v:?}");
        assert_eq!(v.at("source").kind(), "string", "{m:?} {field}");
        assert!(!v.at("source").str().unwrap().is_empty(), "{m:?} {field}");
        assert_eq!(v.at("value").kind(), kind, "{m:?} {field}");
        if let Some(w) = want {
            assert_eq!(v.at("value").str(), Some(w), "{m:?} {field}");
            assert_eq!(
                value,
                format!("{w}\n"),
                "{m:?} {field}: value prints the json value"
            );
        } else {
            let names: Vec<&str> = value
                .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
                .filter(|t| !t.is_empty())
                .collect();
            assert_eq!(
                names,
                v.at("value").strs(),
                "{m:?} {field}: the shell's skill names"
            );
        }
    }
    assert!(Json::parse(&eval(&none, "single", "tool_plane", "json"))
        .unwrap()
        .at("tool_plane")
        .at("value")
        .strs()
        .is_empty());
    assert!(!Json::parse(&eval(&sub, "single", "tool_plane", "json"))
        .unwrap()
        .at("tool_plane")
        .at("value")
        .strs()
        .is_empty());
    let json = eval(&sub, "single", "delivery_plane", "json");
    let source = Json::parse(&json)
        .unwrap()
        .at("delivery_plane")
        .at("source")
        .str()
        .unwrap()
        .to_string();
    let out = launch_in(
        &rt,
        &s.0,
        &scripts("inline_json.sh"),
        &["delivery-source"],
        Some(&json),
    );
    assert_eq!(text(&out.stdout), format!("{source}\n"));
}

/// A text argument that is not UTF-8 (types.md 2절): where base already refuses it — an option with closed choices — the
/// refusal is a usage error, exit 2 with nothing on stdout; the same command line with a valid choice is not exit 2.
#[test]
fn non_utf8_choice_arguments_are_usage_errors() {
    use std::ffi::{OsStr, OsString};
    use std::os::unix::ffi::OsStrExt;
    let s = scratch("nonutf8");
    let rt = runtime_root(&s);
    let manifest = s.0.join("m.yaml");
    write(&manifest, "topology: single\nnodes: []\n");
    let m = manifest.to_str().unwrap();
    let role = ".claude/skills/terraforming_node/scripts/node_role_contract.sh";
    for (launcher, args, option, valid) in [
        (
            role,
            vec!["evaluate", "--manifest", m, "--topology", "single"],
            "--field",
            "sub_mode",
        ),
        (
            role,
            vec!["evaluate", "--manifest", m, "--field", "sub_mode"],
            "--topology",
            "single",
        ),
    ] {
        let run = |value: &OsStr| {
            let mut all: Vec<OsString> = args.iter().map(OsString::from).collect();
            all.extend([OsString::from(option), value.to_os_string()]);
            Command::new("bash")
                .arg(root().join(launcher))
                .args(&all)
                .env("EASY_VLLM_UPSTREAM_RUNTIME_ROOT", &rt)
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

const CLUSTER_MANIFEST: &str = "topology: multi\ngpus_per_node: 1\ncpu_arch: aarch64\nterraforming:\n  complete: true\n  branch_verified: true\ninterconnect:\n  type: RoCE v2\n  hca_devices: [rocep1s0f1]\n  gid_index: 3\n  socket_iface: enp1s0f1np1\n  mtu: 9000\n  bandwidth_gbps: null\n  platform_preset: dgx-spark-gb10\nnodes:\n  - role: main\n    host: main.invalid\n    ssh_user: u\n    work_dir: /w\n  - role: sub\n    host: sub.invalid\n    ssh_user: u\n    work_dir: /w\n";

fn env_lines(out: &str) -> Vec<(String, String)> {
    out.lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| {
            let (k, v) = l
                .split_once('=')
                .unwrap_or_else(|| panic!("not KEY=VALUE: {l:?}"));
            assert!(
                !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
                "{l:?}"
            );
            (k.to_string(), v.to_string())
        })
        .collect()
}

/// `render_dockerfile`: `--cluster-envfile --manifest M` and `--nccl-envfile --manifest M` print an envfile on stdout
/// (`KEY=VALUE` lines and `#` comments; claims_owner asserts `MAX_JOBS=4`), `--out F` writes it to F (sync_to_sub.sh);
/// `--env-tier KEY` prints `{"tier", "detail"}` for any key; a manifest without the Flag is exit 4 for
/// `--canonical-kind compose`.
#[test]
fn render_dockerfile_envfiles_env_tier_and_flag_gate() {
    let s = scratch("render");
    let rt = runtime_root(&s);
    let m = s.0.join("manifest.yaml");
    write(&m, CLUSTER_MANIFEST);
    let rd = |args: &[&str]| launch(&rt, &s.0, &scripts("render_dockerfile.sh"), args);
    let out = rd(&["--cluster-envfile", "--manifest", m.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let cluster = env_lines(&text(&out.stdout));
    assert!(
        cluster.contains(&("MAX_JOBS".into(), "4".into())),
        "{cluster:?}"
    );
    assert!(
        cluster.iter().any(|(k, _)| k == "MASTER_HOST_IP"),
        "{cluster:?}"
    );
    let out = rd(&["--nccl-envfile", "--manifest", m.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let nccl = env_lines(&text(&out.stdout));
    assert!(
        nccl.iter().any(|(k, _)| k == "NCCL_SOCKET_IFNAME"),
        "{nccl:?}"
    );
    for (flag, name) in [
        ("--nccl-envfile", ".env.interconnect"),
        ("--cluster-envfile", ".env.cluster"),
    ] {
        let file = s.0.join("envs").join(name);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        let out = rd(&[
            flag,
            "--manifest",
            m.to_str().unwrap(),
            "--out",
            file.to_str().unwrap(),
        ]);
        assert_eq!(out.status.code(), Some(0), "{flag}: {}", text(&out.stderr));
        assert!(
            !env_lines(&std::fs::read_to_string(&file).unwrap()).is_empty(),
            "{flag}"
        );
    }
    // --env-tier KEY (hint-publisher a2.rs): exit 0 for every key, `tier` from the closed set, `detail` a string for the
    // env and preset tiers and null for an invariant or an unknown key
    for (key, tier, detail) in [
        ("NCCL_IB_HCA", "env", "string"),
        ("MAX_JOBS", "preset", "string"),
        ("NCCL_DEBUG", "invariant", "null"),
        ("NO_SUCH_KEY_XYZ", "unknown", "null"),
    ] {
        let out = rd(&["--env-tier", key]);
        assert_eq!(out.status.code(), Some(0), "{key}: {}", text(&out.stderr));
        let doc = Json::parse(&text(&out.stdout)).unwrap();
        assert_eq!(doc.keys().len(), 2, "{key}: {doc:?}");
        let got = doc
            .at("tier")
            .str()
            .unwrap_or_else(|| panic!("{key}: {doc:?}"));
        assert!(
            ["env", "preset", "invariant", "unknown"].contains(&got),
            "{key}: {got}"
        );
        assert_eq!(got, tier, "{key}");
        assert_eq!(doc.at("detail").kind(), detail, "{key}");
    }
    let flagless = s.0.join("flagless.yaml");
    write(
        &flagless,
        &CLUSTER_MANIFEST
            .replace("complete: true", "complete: false")
            .replace("topology: multi", "topology: single"),
    );
    let out = rd(&[
        "--canonical-kind",
        "compose",
        "--topology",
        "single",
        "--manifest",
        flagless.to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(4), "{}", text(&out.stderr));
}

/// `slave_forward prefix|json --cell-env --project-env --compose [--cluster-env] [--group G]` (multinode_serve_smoke.sh,
/// hint-publisher `a2.rs`·`a3/collect.rs`, policy-predicates `claims_owner.rs`): `prefix` prints space-separated KEY=VALUE
/// tokens, `json` prints `forwards[]` rows with `key`·`group`·`value` and `sub_recipe`; a refusal is non-zero with
/// `[slave-forward] FAIL(<CODE>): ` on stderr and nothing on stdout.
#[test]
fn slave_forward_prefix_json_and_refusal() {
    let s = scratch("forward");
    let rt = runtime_root(&s);
    let (cell, project, compose, cluster) = (
        s.0.join("cell.env"),
        s.0.join("project.env"),
        s.0.join("compose.yaml"),
        s.0.join("cluster.env"),
    );
    write(&cell, "CONFIG_FILE=c1\nIMAGE_TAG=easy-vllm:x\nVLLM_REF=v1\nBUILD_JOBS=8\nSLAVE_CONTAINER_NAME=c1-slave\n");
    write(&project, "NAS_MODEL_PATH=/data/models\n");
    write(
        &cluster,
        "MASTER_HOST_IP=main.invalid\nSLAVE_HOST_IP=sub.invalid\nRAY_PORT=6379\n",
    );
    write(
        &compose,
        "x-gpu-common: &gpu-common\n  image: ${IMAGE_TAG:-easy-vllm:base}\n  build:\n    context: .\n    args:\n      VLLM_REF: ${VLLM_REF:-v0}\n      BUILD_JOBS: ${BUILD_JOBS:-16}\n  volumes:\n    - ${NAS_MODEL_PATH:-/data/x}:/app/models:ro\nservices:\n  vllm-master-serve:\n    <<: *gpu-common\n    environment:\n      CONFIG_FILE: ${CONFIG_FILE:-default}\n  vllm-slave-serve:\n    <<: *gpu-common\n    container_name: ${SLAVE_CONTAINER_NAME:-vllm-slave-serve-container}\n    environment:\n      VLLM_HOST_IP: ${SLAVE_HOST_IP}\n      HEAD_NODE_IP: ${MASTER_HOST_IP}\n      RAY_PORT: ${RAY_PORT:-6379}\n      CONFIG_FILE: ${CONFIG_FILE:-default}\n",
    );
    let paths = [
        "--cell-env",
        cell.to_str().unwrap(),
        "--project-env",
        project.to_str().unwrap(),
        "--compose",
        compose.to_str().unwrap(),
        "--cluster-env",
        cluster.to_str().unwrap(),
    ];
    let sf = |args: &[&str]| launch(&rt, &s.0, &scripts("slave_forward.sh"), args);
    let mut json_args = vec!["json"];
    json_args.extend(paths);
    let out = sf(&json_args);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let doc =
        Json::parse(&text(&out.stdout)).unwrap_or_else(|e| panic!("{e}: {}", text(&out.stdout)));
    let rows = doc.at("forwards").arr().unwrap().to_vec();
    assert!(!rows.is_empty(), "{doc:?}");
    for r in &rows {
        assert_eq!(
            (
                r.at("key").kind(),
                r.at("group").kind(),
                r.at("value").kind()
            ),
            ("string", "string", "string"),
            "{r:?}"
        );
        assert_ne!(
            r.at("key").str(),
            Some("CONFIG_FILE"),
            "CONFIG_FILE is never forwarded"
        );
    }
    doc.at("sub_recipe");
    let keys: Vec<&str> = rows.iter().filter_map(|r| r.at("key").str()).collect();
    assert!(
        keys.contains(&"IMAGE_TAG") && keys.contains(&"VLLM_REF"),
        "{keys:?}"
    );
    let mut prefix_args = vec!["prefix", "--group", "image_identity,cluster"];
    prefix_args.extend(paths);
    let out = sf(&prefix_args);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let prefix = text(&out.stdout);
    let tokens: Vec<&str> = prefix.split_whitespace().collect();
    assert!(
        !tokens.is_empty() && tokens.iter().all(|t| t.contains('=')),
        "{prefix:?}"
    );
    assert!(tokens.contains(&"IMAGE_TAG=easy-vllm:x"), "{prefix:?}");
    for group in ["build_tuning", "mount", "serve_env"] {
        let mut a = vec!["prefix", "--group", group];
        a.extend(paths);
        assert_eq!(sf(&a).status.code(), Some(0), "--group {group}");
    }
    let mut bad = vec!["json"];
    bad.extend(paths);
    let absent = s.0.join("absent.yaml");
    bad[6] = absent.to_str().unwrap();
    let out = sf(&bad);
    assert_ne!(out.status.code(), Some(0));
    assert_eq!(text(&out.stdout), "");
    assert!(
        text(&out.stderr)
            .lines()
            .any(|l| l.starts_with("[slave-forward] FAIL(SF_COMPOSE_ABSENT): ")),
        "{}",
        text(&out.stderr)
    );
}

/// `validate_runtime_patch stamp|verify --patch P --topology multi --config-dir C --env-dir E --resolution R`
/// (policy-predicates `runtime_patch.rs`, sync_to_sub.sh): stamp then verify are exit 0; a changed patch, a changed config
/// and a missing sidecar make verify non-zero.
#[test]
fn validate_runtime_patch_stamp_and_verify() {
    let s = scratch("patch");
    let rt = runtime_root(&s);
    let (configs, envs) = (s.0.join("configs"), s.0.join("envs"));
    let patch = configs.join("modelA_patch.py");
    write(&patch, "# current patch\n");
    write(&configs.join("modelA.yaml"), "model: A\nversion: 1\n");
    write(&envs.join(".env.modelA"), "MODEL=A\n");
    let resolution = skill().join("assets/current-production-resolution.json");
    let call = |action: &str| {
        let out = launch(
            &rt,
            &s.0,
            &scripts("validate_runtime_patch.sh"),
            &[
                action,
                "--patch",
                patch.to_str().unwrap(),
                "--topology",
                "multi",
                "--config-dir",
                configs.to_str().unwrap(),
                "--env-dir",
                envs.to_str().unwrap(),
                "--resolution",
                resolution.to_str().unwrap(),
            ],
        );
        (out.status.code(), text(&out.stderr))
    };
    assert_eq!(
        call("verify").0.map(|c| c != 0),
        Some(true),
        "no sidecar yet"
    );
    assert_eq!(call("stamp").0, Some(0));
    assert_eq!(call("verify").0, Some(0), "{}", call("verify").1);
    assert!(configs.join("modelA_patch.provenance.json").is_file());
    write(&patch, "# stale carried-forward patch\n");
    assert_ne!(call("verify").0, Some(0));
    write(&patch, "# current patch\n");
    assert_eq!(call("stamp").0, Some(0));
    write(&configs.join("modelA.yaml"), "model: A\nversion: 2\n");
    assert_ne!(call("verify").0, Some(0));
}

/// A repository with the two branch lines, the ledger and the files the layer judge reads.
fn ledger_repo(s: &Scratch) -> PathBuf {
    let repo = s.0.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "single-node"]);
    write(&repo.join("a.txt"), "a\n");
    write(
        &repo.join(".claude/policies/branch_layer_ledger.json"),
        "{\"schema_version\": 1, \"entries\": []}\n",
    );
    let sync = repo.join(".claude/skills/upstream-version-watch/scripts/sync_branches.sh");
    std::fs::create_dir_all(sync.parent().unwrap()).unwrap();
    std::fs::copy(skill().join("scripts/sync_branches.sh"), &sync).unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "base"]);
    git(&repo, &["branch", "multi-node"]);
    repo
}

/// A closing-sequence entry an author has filled in: every required field, one classification row.
const FILLED_ENTRY: &str = r#"{"entry_id": "sync-202611180300", "utc": "2026-11-18T03:00:00Z", "departure": "single-node", "departure_commit": "0123456789abcdef0123456789abcdef01234567", "approved_by": "reviewer", "approved_utc": "2026-11-18T03:10:00Z", "_reach": "multi-node vs HEAD", "classification": [{"file": "a.txt", "verdict": "common_promote", "reason": "shared text", "provenance": "agent-judged"}]}"#;

/// `layer_ledger` (closing_sequence.sh, sync_branches.sh):
/// - `sync-point` and `divergence` print one JSON object on stdout; messages stay on stderr. sync_branches.sh:521-548 runs
///   `divergence --repo --branch --counterpart --ledger-ref`, reads it through `inline_json divergence-summary` (status,
///   basis, base, sync_point.status, absorbed) and goes on only on `clean`: a branch whose counterpart is the same commit
///   is exit 0 and `clean`.
/// - closing_sequence.sh:137 runs the launcher itself (no `bash`) as `validate --entry-file F` and stops on non-zero: the
///   skeleton `emit-skeleton` prints, its `<FILL…>` placeholders left, is exit 5 with `status` `rejected` and the problems;
///   an entry with every field filled is exit 0 with `valid`. `entry-id` and `utc-to-kst` print one plain line.
#[test]
fn layer_ledger_closing_sequence_subcommands() {
    use std::os::unix::fs::PermissionsExt;
    let s = scratch("ledger");
    let rt = runtime_root(&s);
    let repo = ledger_repo(&s);
    let r = repo.to_str().unwrap();
    let ll = |args: &[&str]| launch(&rt, &repo, &scripts("layer_ledger.sh"), args);
    let out = ll(&[
        "sync-point",
        "--repo",
        r,
        "--branch",
        "single-node",
        "--counterpart",
        "multi-node",
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}{}",
        text(&out.stdout),
        text(&out.stderr)
    );
    assert_eq!(
        Json::parse(&text(&out.stdout)).unwrap().at("status").kind(),
        "string"
    );
    let out = ll(&[
        "divergence",
        "--repo",
        r,
        "--branch",
        "single-node",
        "--counterpart",
        "multi-node",
        "--ledger-ref",
        "multi-node",
    ]);
    let doc = Json::parse(&text(&out.stdout))
        .unwrap_or_else(|e| panic!("{e}: {}{}", text(&out.stdout), text(&out.stderr)));
    assert_eq!(out.status.code(), Some(0), "{doc:?} {}", text(&out.stderr));
    assert_eq!(doc.at("status").str(), Some("clean"), "{doc:?}");
    assert_eq!(doc.at("basis").kind(), "string");
    assert!(
        matches!(doc.at("base").kind(), "string" | "null"),
        "{doc:?}"
    );
    assert_eq!(doc.at("sync_point").at("status").kind(), "string");
    if let Some(a) = doc.get("absorbed") {
        assert!(matches!(a, Json::Arr(_) | Json::Obj(_)), "{a:?}");
    }
    let line = launch_in(
        &rt,
        &repo,
        &scripts("inline_json.sh"),
        &["divergence-summary"],
        Some(&text(&out.stdout)),
    );
    let summary = text(&line.stdout);
    let fields: Vec<&str> = summary.trim_end_matches('\n').split('\t').collect();
    assert_eq!(fields.len(), 5, "{summary:?}");
    assert_eq!(fields[0], "clean");
    assert_eq!(Some(fields[1]), doc.at("basis").str());
    assert_eq!(Some(fields[3]), doc.at("sync_point").at("status").str());
    // emit-skeleton: the entry fields closing_sequence.sh's author fills, placeholders `<FILL…>` in them
    let out = ll(&[
        "emit-skeleton",
        "--repo",
        r,
        "--branch",
        "single-node",
        "--counterpart",
        "multi-node",
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}{}",
        text(&out.stdout),
        text(&out.stderr)
    );
    let skeleton = text(&out.stdout);
    let doc = Json::parse(&skeleton).unwrap();
    for key in [
        "entry_id",
        "utc",
        "departure",
        "departure_commit",
        "approved_by",
        "approved_utc",
    ] {
        assert_eq!(doc.at(key).kind(), "string", "{key}");
    }
    assert_eq!(doc.at("departure").str(), Some("single-node"));
    assert!(doc.at("utc").str().unwrap().starts_with("<FILL"), "{doc:?}");
    doc.at("classification").arr().unwrap();
    // validate, run as closing_sequence.sh runs it: the launcher is executable
    let launcher = root().join(scripts("layer_ledger.sh"));
    assert_ne!(
        std::fs::metadata(&launcher).unwrap().permissions().mode() & 0o111,
        0,
        "closing_sequence.sh executes {launcher:?}"
    );
    let validate = |entry: &Path| {
        let out = Command::new(&launcher)
            .args(["validate", "--entry-file", entry.to_str().unwrap()])
            .env("EASY_VLLM_UPSTREAM_RUNTIME_ROOT", &rt)
            .current_dir(&repo)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let doc = Json::parse(&text(&out.stdout))
            .unwrap_or_else(|e| panic!("{e}: {}{}", text(&out.stdout), text(&out.stderr)));
        (out.status.code(), doc)
    };
    let raw = s.0.join("skeleton.json");
    write(&raw, &skeleton);
    let (rc, doc) = validate(&raw);
    assert_eq!(rc, Some(5), "{doc:?}");
    assert_eq!(doc.at("status").str(), Some("rejected"));
    assert!(!doc.at("problems").strs().is_empty());
    let entry = s.0.join("entry.json");
    write(&entry, FILLED_ENTRY);
    let (rc, doc) = validate(&entry);
    assert_eq!(rc, Some(0), "{doc:?}");
    assert_eq!(doc.at("status").str(), Some("valid"));
    assert!(doc.at("problems").strs().is_empty());
    for sub in ["entry-id", "utc-to-kst"] {
        let out = ll(&[sub, "--entry-file", entry.to_str().unwrap()]);
        let so = text(&out.stdout);
        assert_eq!(
            out.status.code(),
            Some(0),
            "{sub}: {so}{}",
            text(&out.stderr)
        );
        assert_eq!(so.lines().count(), 1, "{sub}: {so:?}");
    }
    assert_eq!(ll(&["no-such-subcommand"]).status.code(), Some(2));
}

/// The ledger's one writer (types.md 4절 「영속 문서」 4, consumers.md 「영속 데이터」): `append --repo R --entry-file F`
/// (closing_sequence.sh:154, exit 0 `appended`) and `merge --repo R --from-ref REF` (closing_sequence.sh:268, exit 0
/// `merged`) rewrite the whole file, and keys outside the schema — at the top, in an entry, in a classification row, like
/// the ledger's `_reach`·`_execution_note`·`_note`·`_approval_source` — stay as they were read.
#[test]
fn layer_ledger_append_and_merge_keep_unknown_keys() {
    let s = scratch("ledger-keys");
    let rt = runtime_root(&s);
    let repo = ledger_repo(&s);
    let r = repo.to_str().unwrap();
    let ledger = repo.join(".claude/policies/branch_layer_ledger.json");
    let prior = r#"{"entry_id": "sync-202611010000", "utc": "2026-11-01T00:00:00Z", "departure": "multi-node", "departure_commit": "1111111111111111111111111111111111111111", "approved_by": "reviewer", "approved_utc": "2026-11-01T00:10:00Z", "_reach": "r", "_execution_note": "by hand", "_note": "n", "classification": [{"file": "b.txt", "verdict": "branch_only", "reason": "local", "provenance": "agent-judged", "_row_note": [1, 2]}]}"#;
    let other = r#"{"entry_id": "sync-202611050000", "utc": "2026-11-05T00:00:00Z", "departure": "multi-node", "departure_commit": "2222222222222222222222222222222222222222", "approved_by": "reviewer", "approved_utc": "2026-11-05T00:10:00Z", "_approval_source": "chat", "classification": [{"file": "c.txt", "verdict": "common_promote", "reason": "shared", "provenance": "agent-judged"}]}"#;
    // the counterpart branch carries one more entry for merge
    git(&repo, &["checkout", "-q", "multi-node"]);
    write(
        &ledger,
        &format!(
            r#"{{"schema_version": 1, "_ledger_note": {{"kept": true}}, "entries": [{prior}, {other}]}}"#
        ),
    );
    git(&repo, &["commit", "-qam", "ledger on multi-node"]);
    git(&repo, &["checkout", "-q", "single-node"]);
    write(
        &ledger,
        &format!(
            r#"{{"schema_version": 1, "_ledger_note": {{"kept": true}}, "entries": [{prior}]}}"#
        ),
    );
    let entry = s.0.join("entry.json");
    write(&entry, FILLED_ENTRY);
    let ll = |args: &[&str]| launch(&rt, &repo, &scripts("layer_ledger.sh"), args);
    let read = || Json::parse(&std::fs::read_to_string(&ledger).unwrap()).unwrap();
    let entry_by_id = |doc: &Json, id: &str| {
        doc.at("entries")
            .arr()
            .unwrap()
            .iter()
            .find(|e| e.at("entry_id").str() == Some(id))
            .cloned()
            .unwrap_or_else(|| panic!("{id}"))
    };
    let out = ll(&[
        "append",
        "--repo",
        r,
        "--entry-file",
        entry.to_str().unwrap(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}{}",
        text(&out.stdout),
        text(&out.stderr)
    );
    assert_eq!(
        Json::parse(&text(&out.stdout)).unwrap().at("status").str(),
        Some("appended")
    );
    let doc = read();
    assert!(
        doc.at("_ledger_note")
            .same(&Json::parse(r#"{"kept": true}"#).unwrap()),
        "{doc:?}"
    );
    assert!(
        entry_by_id(&doc, "sync-202611010000").same(&Json::parse(prior).unwrap()),
        "the earlier entry is unchanged: {doc:?}"
    );
    assert!(
        entry_by_id(&doc, "sync-202611180300").same(&Json::parse(FILLED_ENTRY).unwrap()),
        "the appended entry, `_reach` with it: {doc:?}"
    );
    let out = ll(&["merge", "--repo", r, "--from-ref", "multi-node"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}{}",
        text(&out.stdout),
        text(&out.stderr)
    );
    let res = Json::parse(&text(&out.stdout)).unwrap();
    assert_eq!(res.at("status").str(), Some("merged"));
    assert_eq!(res.at("added"), &Json::Num("1".into()));
    let doc = read();
    assert!(
        doc.at("_ledger_note")
            .same(&Json::parse(r#"{"kept": true}"#).unwrap()),
        "{doc:?}"
    );
    assert_eq!(doc.at("entries").arr().unwrap().len(), 3);
    assert!(
        entry_by_id(&doc, "sync-202611010000").same(&Json::parse(prior).unwrap()),
        "{doc:?}"
    );
    assert!(
        entry_by_id(&doc, "sync-202611050000").same(&Json::parse(other).unwrap()),
        "the merged entry, `_approval_source` with it: {doc:?}"
    );
    assert!(
        entry_by_id(&doc, "sync-202611180300").same(&Json::parse(FILLED_ENTRY).unwrap()),
        "{doc:?}"
    );
}

/// closing_sequence.sh: `push_branches --repo R --remote X --branch A --branch B` pushes the named branches (exit 0) and is
/// non-zero when a branch cannot be pushed; a local bare repository stands for the remote.
#[test]
fn push_branches_to_a_local_remote() {
    let s = scratch("push");
    let rt = runtime_root(&s);
    let repo = ledger_repo(&s);
    let bare = s.0.join("remote.git");
    std::fs::create_dir_all(&bare).unwrap();
    git(&bare, &["init", "-q", "--bare"]);
    git(&repo, &["remote", "add", "fx", bare.to_str().unwrap()]);
    let r = repo.to_str().unwrap();
    let out = launch(
        &rt,
        &repo,
        &scripts("push_branches.sh"),
        &[
            "--repo",
            r,
            "--remote",
            "fx",
            "--branch",
            "single-node",
            "--branch",
            "multi-node",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}{}",
        text(&out.stdout),
        text(&out.stderr)
    );
    let heads = git(
        &bare,
        &["for-each-ref", "--format=%(refname)", "refs/heads"],
    );
    assert_eq!(
        heads.lines().collect::<Vec<_>>(),
        ["refs/heads/multi-node", "refs/heads/single-node"]
    );
    let out = launch(
        &rt,
        &repo,
        &scripts("push_branches.sh"),
        &["--repo", r, "--remote", "fx", "--branch", "no-such-branch"],
    );
    assert_ne!(out.status.code(), Some(0));
}

/// multinode_serve_smoke.sh·single_serve_up.sh: `check_smoke_model <CONFIG> --repo R --topology T --emit-gate-params`
/// (2>&1) is exit 2 when the model is absent on the host root the compose file mounts (no download), and another non-zero
/// code (3) when the triplet is missing — before any RAM gate.
#[test]
fn check_smoke_model_model_absent_is_exit_2() {
    let s = scratch("smoke");
    let rt = runtime_root(&s);
    let repo = s.0.join("repo");
    let base = repo.join("output/multi");
    let nas = s.0.join("nas");
    std::fs::create_dir_all(&nas).unwrap();
    write(
        &base.join("configs/cfg1.yaml"),
        "model: /app/models/org/absent-model\nmax-model-len: 4096\n",
    );
    write(
        &base.join("docker-compose.yaml"),
        &format!(
            "services:\n  vllm-master-serve:\n    volumes:\n      - {}:/app/models:ro\n",
            nas.display()
        ),
    );
    let run = |cfg: &str| {
        let out = Command::new("bash")
            .arg(root().join(scripts("check_smoke_model.sh")))
            .args([
                cfg,
                "--repo",
                repo.to_str().unwrap(),
                "--topology",
                "multi",
                "--emit-gate-params",
            ])
            .env("EASY_VLLM_UPSTREAM_RUNTIME_ROOT", &rt)
            .env_remove("NAS_MODEL_PATH")
            .current_dir(&s.0)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        (
            out.status.code(),
            format!("{}{}", text(&out.stdout), text(&out.stderr)),
        )
    };
    let (rc, merged) = run("cfg1");
    assert_eq!(rc, Some(2), "{merged}");
    assert!(
        !merged.contains("required_mib=") && !merged.contains("BUDGET_PARAMS"),
        "{merged}"
    );
    let (rc, merged) = run("no-such-config");
    assert!(
        !matches!(rc, Some(0) | Some(2) | Some(7)),
        "{rc:?} {merged}"
    );
}

fn sha256_of(path: &Path) -> String {
    let out = Command::new("sha256sum")
        .arg(path)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    text(&out.stdout)
        .split_whitespace()
        .next()
        .unwrap()
        .to_string()
}

/// sync_to_sub.sh: `regen_build_patches_src modules-kind --manifest P` (exit 0 = script modules), `modules-files
/// --manifest P` (one file name per line, then `PROVENANCE.json`), `verify --root R` (exit 0 / non-zero on a changed
/// payload) and `bundle --root R --out F` (the last stdout line is the bundle's sha256) on the main; on the sub, the
/// delivered ELF's `unbundle --root --bundle --expect-sha256` and `verify --root`.
#[test]
fn regen_build_patches_src_delivery_subcommands() {
    let s = scratch("regen");
    let rt = runtime_root(&s);
    let rb = |args: &[&str]| launch(&rt, &s.0, &scripts("regen_build_patches_src.sh"), args);
    // script modules
    let modules = s.0.join("modules");
    write(&modules.join("10-first.sh"), "echo one\n");
    write(&modules.join("20-second.sh"), "echo two\n");
    let (a, b) = (
        sha256_of(&modules.join("10-first.sh")),
        sha256_of(&modules.join("20-second.sh")),
    );
    write(
        &modules.join("PROVENANCE.json"),
        &format!(
            r#"{{"source": {{"repo": "example/vllm", "ref": "v0"}}, "source_payload": {{"kind": "script-modules", "modules_sha256": {{"20-second.sh": "{b}", "10-first.sh": "{a}"}}}}}}"#
        ),
    );
    let prov = modules.join("PROVENANCE.json");
    assert_eq!(
        rb(&["modules-kind", "--manifest", prov.to_str().unwrap()])
            .status
            .code(),
        Some(0)
    );
    let out = rb(&["modules-files", "--manifest", prov.to_str().unwrap()]);
    assert_eq!(
        (out.status.code(), text(&out.stdout).as_str()),
        (Some(0), "10-first.sh\n20-second.sh\nPROVENANCE.json\n")
    );
    let out = rb(&["verify", "--root", modules.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    // a files-map payload
    let fm = s.0.join("filesmap");
    write(&fm.join("files/a/x.py"), "x = 1\n");
    let x = sha256_of(&fm.join("files/a/x.py"));
    write(
        &fm.join("PROVENANCE.json"),
        &format!(
            r#"{{"source": {{"repo": "example/vllm", "ref": "v0"}}, "files": {{"a/x.py": "{x}"}}, "counts": {{"files": 1}}}}"#
        ),
    );
    let fprov = fm.join("PROVENANCE.json");
    assert_ne!(
        rb(&["modules-kind", "--manifest", fprov.to_str().unwrap()])
            .status
            .code(),
        Some(0)
    );
    let out = rb(&["verify", "--root", fm.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let bundle = s.0.join("out/bundle.tar.gz");
    let out = rb(&[
        "bundle",
        "--root",
        fm.to_str().unwrap(),
        "--out",
        bundle.to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let last = text(&out.stdout)
        .lines()
        .last()
        .unwrap_or_default()
        .to_string();
    assert_eq!(
        last,
        sha256_of(&bundle),
        "the last stdout line is the bundle's sha256"
    );
    // the sub side (sync_to_sub.sh:1475-1494, the delivered ELF in the sub work tree, paths relative to it): `unbundle
    // --root R --bundle B --expect-sha256 S` lays the payload out and verifies it, `verify --root R` checks it again; a
    // bundle that is not the expected sha256 is refused and the payload stays
    let (work, rel) = sub_work_tree(&s);
    let port = "output/single/source_port";
    write(
        &work.join(port).join("PROVENANCE.json"),
        &std::fs::read_to_string(&fprov).unwrap(),
    );
    let remote_bundle = work.join(port).join("bundle.tar.gz");
    std::fs::copy(&bundle, &remote_bundle).unwrap();
    let rb_sub = |args: &[&str]| sub_call(&work, &rel, "regen_build_patches_src", args, None);
    let out = rb_sub(&[
        "unbundle",
        "--root",
        port,
        "--bundle",
        remote_bundle.to_str().unwrap(),
        "--expect-sha256",
        &last,
    ]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(
        std::fs::read_to_string(work.join(port).join("files/a/x.py")).unwrap(),
        "x = 1\n"
    );
    assert_eq!(rb_sub(&["verify", "--root", port]).status.code(), Some(0));
    let wrong = "0".repeat(64);
    assert_ne!(
        rb_sub(&[
            "unbundle",
            "--root",
            port,
            "--bundle",
            remote_bundle.to_str().unwrap(),
            "--expect-sha256",
            &wrong
        ])
        .status
        .code(),
        Some(0)
    );
    assert_eq!(
        std::fs::read_to_string(work.join(port).join("files/a/x.py")).unwrap(),
        "x = 1\n"
    );
    write(&fm.join("files/a/x.py"), "x = 2\n");
    assert_ne!(
        rb(&["verify", "--root", fm.to_str().unwrap()])
            .status
            .code(),
        Some(0)
    );
    assert_ne!(
        rb(&[
            "bundle",
            "--root",
            fm.to_str().unwrap(),
            "--out",
            bundle.to_str().unwrap()
        ])
        .status
        .code(),
        Some(0)
    );
}

/// The value of `NAME="…"` and the words of `NAME=(…)` (comment after `#` dropped) on their sync_to_sub.sh lines.
fn sync_to_sub_assignment(name: &str) -> String {
    let body = std::fs::read_to_string(skill().join("scripts/sync_to_sub.sh")).unwrap();
    let line = body
        .lines()
        .find(|l| l.starts_with(&format!("{name}=")))
        .unwrap_or_else(|| panic!("sync_to_sub.sh lacks {name}="));
    let value = &line[name.len() + 1..];
    let value = value.split(" #").next().unwrap().trim_end();
    value
        .trim_matches('"')
        .trim_start_matches('(')
        .trim_end_matches(')')
        .to_string()
}

/// This build's executable of a delivered sub-node ELF.
fn sub_elf(bin: &str) -> &'static str {
    match bin {
        "sub_runner" => env!("CARGO_BIN_EXE_sub_runner"),
        "regen_build_patches_src" => env!("CARGO_BIN_EXE_regen_build_patches_src"),
        "inline_json" => env!("CARGO_BIN_EXE_inline_json"),
        "native_remote" => env!("CARGO_BIN_EXE_native_remote"),
        other => panic!("no delivered bin {other}"),
    }
}

/// A sub work tree as sync_to_sub.sh leaves it: the main's ELFs `SUB_ELF_NAMES` under `SUB_ELF_REL` (the consumer's
/// own values, fixed here: the main's `native_multinode_serve` runs `<work_dir>/SUB_REMOTE_REL`).
fn sub_work_tree(s: &Scratch) -> (PathBuf, String) {
    let rel = sync_to_sub_assignment("SUB_ELF_REL");
    assert_eq!(
        rel,
        ".claude/skills/upstream-version-watch/rust/runtime/target/release"
    );
    let names = sync_to_sub_assignment("SUB_ELF_NAMES");
    assert_eq!(
        names.split_whitespace().collect::<Vec<_>>(),
        [
            "sub_runner",
            "regen_build_patches_src",
            "inline_json",
            "native_remote"
        ]
    );
    assert_eq!(
        upstream_runtime::native_multinode::SUB_REMOTE_REL,
        format!("{rel}/native_remote")
    );
    let work = s.0.join("sub-work");
    let dir = work.join(&rel);
    std::fs::create_dir_all(&dir).unwrap();
    for name in names.split_whitespace() {
        std::os::unix::fs::symlink(sub_elf(name), dir.join(name)).unwrap();
    }
    (work, rel)
}

/// The sub-side call of sync_to_sub.sh (`sub_runner_run`, `sub_run`): `cd '<SUB_WORK_DIR>' && '<SUB_ELF_REL>/<bin>' <args>`
/// in a shell, stdin given (`/dev/null` when none), stdout and stderr apart.
fn sub_call(work: &Path, rel: &str, bin: &str, args: &[&str], stdin: Option<&str>) -> Output {
    use std::io::Write;
    let mut c = Command::new("bash");
    c.arg("-c")
        .arg(format!("cd '{}' && '{rel}/{bin}' \"$@\"", work.display()))
        .arg("sub-call")
        .args(args);
    c.stdin(if stdin.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    })
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    let mut child = c.spawn().unwrap();
    if let Some(input) = stdin {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
    }
    child.wait_with_output().unwrap()
}

/// The non-empty lines of an output, sorted (the walks print in directory order).
fn line_set(out: &str) -> Vec<String> {
    let mut v: Vec<String> = out
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();
    v.sort();
    v
}

/// sync_to_sub.sh runs the delivered `sub_runner` on the sub (:886-891, :1090, :1111, :2001):
/// - `prune-empty-dirs ROOT SKIP POST` (SKIP·POST `:`-joined names): removes every empty directory below ROOT, deepest
///   first, except under a SKIP top entry and below a POST top directory; stdout one removed path (relative to ROOT) per
///   line, nothing when none; exit 0, 9 when a removal fails; a ROOT that is not a directory is exit 0 with nothing.
/// - `validate-names ROOT SKIP POST`: stdout one path per line whose name holds a byte ≤ 0x20 or 0x7f (outside the SKIP
///   top entries, not below the subdirectories of a POST top directory); nothing when every name is clean; exit 0.
/// - `retirement-scan STALE`, stdin the active surface (paths relative to the work tree, one per line): stdout first line
///   the number of regular files it classified, then one `<path>:<line number>:<line>` per reference to STALE that is not
///   the owner path `.claude/skills/upstream-version-watch/<STALE>` (Rust `checks/` files and the parity fixtures are
///   counted, not read); an empty list or a symbolic link in it is exit 1.
/// - a wrong argument count is exit 2. policy-distribution `checks.rs:735-739` matches three tokens of the source.
#[test]
fn sub_runner_delivered_subcommands() {
    use std::os::unix::fs::PermissionsExt;
    let s = scratch("subrunner");
    let (work, rel) = sub_work_tree(&s);
    let sr = |args: &[&str], stdin: Option<&str>| sub_call(&work, &rel, "sub_runner", args, stdin);
    let root = work.join("output/single");
    let r = root.to_str().unwrap();
    let (skip, post) = ("manifest.yaml:cache", "build_patches:build_patches_src");
    for d in [
        "a/b/c",
        "a/keep",
        "empty_top",
        "cache/tmp",
        "build_patches/sub",
        "build_patches_src/x/y",
    ] {
        std::fs::create_dir_all(root.join(d)).unwrap();
    }
    write(&root.join("a/keep/file.txt"), "x\n");
    let out = sr(&["prune-empty-dirs", r, skip, post], None);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(line_set(&text(&out.stdout)), ["a/b", "a/b/c", "empty_top"]);
    for (d, kept) in [
        ("a/b", false),
        ("empty_top", false),
        ("a/keep", true),
        ("cache/tmp", true),
        ("build_patches/sub", true),
        ("build_patches_src/x/y", true),
    ] {
        assert_eq!(root.join(d).is_dir(), kept, "{d}");
    }
    let out = sr(&["prune-empty-dirs", r, skip, post], None);
    assert_eq!(
        (out.status.code(), line_set(&text(&out.stdout))),
        (Some(0), Vec::<String>::new())
    );
    let out = sr(
        &[
            "prune-empty-dirs",
            work.join("output/absent").to_str().unwrap(),
            skip,
            post,
        ],
        None,
    );
    assert_eq!(
        (out.status.code(), text(&out.stdout).as_str()),
        (Some(0), "")
    );
    assert_eq!(
        sr(&["prune-empty-dirs", r, skip], None).status.code(),
        Some(2)
    );
    // a removal the file system refuses (a read-only parent; the check runs where this process cannot write it either)
    let locked = root.join("locked");
    std::fs::create_dir_all(locked.join("inner")).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();
    if std::fs::create_dir(locked.join("probe")).is_err() {
        let out = sr(&["prune-empty-dirs", r, skip, post], None);
        assert_eq!(out.status.code(), Some(9), "{}", text(&out.stderr));
    }
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
    let _ = std::fs::remove_dir(locked.join("probe"));

    // validate-names
    write(&root.join("ok.txt"), "x\n");
    write(&root.join("bad name.txt"), "x\n");
    write(&root.join("tab\there/inner name"), "x\n");
    write(&root.join("cache/skipped name"), "x\n");
    write(&root.join("build_patches/top level.sh"), "x\n");
    write(&root.join("build_patches/sub/below post"), "x\n");
    std::fs::create_dir_all(root.join("build_patches/dir name")).unwrap();
    let out = sr(&["validate-names", r, skip, post], None);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(
        line_set(&text(&out.stdout)),
        [
            "bad name.txt",
            "build_patches/top level.sh",
            "tab\there",
            "tab\there/inner name"
        ]
    );
    let clean = work.join("output/multi");
    write(&clean.join("a/b.txt"), "x\n");
    let out = sr(
        &["validate-names", clean.to_str().unwrap(), skip, post],
        None,
    );
    assert_eq!(
        (out.status.code(), line_set(&text(&out.stdout))),
        (Some(0), Vec::<String>::new())
    );
    let out = sr(
        &[
            "validate-names",
            work.join("output/absent").to_str().unwrap(),
            skip,
            post,
        ],
        None,
    );
    assert_eq!(
        (out.status.code(), text(&out.stdout).as_str()),
        (Some(0), "")
    );
    assert_eq!(sr(&["validate-names", r], None).status.code(), Some(2));

    // retirement-scan
    let stale = "scripts/old_tool.sh";
    write(
        &work.join("docs/a.md"),
        "intro\nsee scripts/old_tool.sh here\n",
    );
    write(
        &work.join("docs/crlf.md"),
        "x\r\nrun scripts/old_tool.sh\r\n",
    );
    write(&work.join("docs/owner.md"), "canonical .claude/skills/upstream-version-watch/scripts/old_tool.sh\nalso ./.claude/skills/upstream-version-watch/scripts/old_tool.sh\n");
    write(
        &work.join("docs/other.md"),
        "scripts/old_tool.sh.bak xscripts/old_tool.sh\n",
    );
    write(
        &work.join(".claude/rust/policy-gate/checks/t.rs"),
        "// scripts/old_tool.sh\n",
    );
    write(
        &work.join(".claude/rust/parity/cases/c.json"),
        "\"scripts/old_tool.sh\"\n",
    );
    std::fs::create_dir_all(work.join("docs/dir")).unwrap();
    let list = "docs/a.md\ndocs/crlf.md\ndocs/owner.md\ndocs/other.md\n.claude/rust/policy-gate/checks/t.rs\n.claude/rust/parity/cases/c.json\ndocs/absent.md\ndocs/dir\n";
    let out = sr(&["retirement-scan", stale], Some(list));
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let so = text(&out.stdout);
    let mut lines = so.lines();
    assert_eq!(lines.next(), Some("6"), "{so:?}");
    let mut hits: Vec<&str> = lines.filter(|l| !l.is_empty()).collect();
    hits.sort();
    assert_eq!(
        hits,
        [
            "docs/a.md:2:see scripts/old_tool.sh here",
            "docs/crlf.md:2:run scripts/old_tool.sh"
        ]
    );
    let out = sr(
        &["retirement-scan", stale],
        Some("docs/owner.md\ndocs/other.md\n"),
    );
    assert_eq!(
        (out.status.code(), line_set(&text(&out.stdout))),
        (Some(0), vec!["2".to_string()])
    );
    let out = sr(&["retirement-scan", stale], Some("\n"));
    assert_eq!(
        out.status.code(),
        Some(1),
        "an empty surface list is a derivation failure"
    );
    std::os::unix::fs::symlink(work.join("docs/a.md"), work.join("docs/link.md")).unwrap();
    let out = sr(
        &["retirement-scan", stale],
        Some("docs/a.md\ndocs/link.md\n"),
    );
    assert_eq!(out.status.code(), Some(1), "a symbolic link in the surface");
    assert_eq!(sr(&["retirement-scan"], Some(list)).status.code(), Some(2));
    assert_eq!(sr(&["no-such-subcommand"], None).status.code(), Some(2));

    let source =
        std::fs::read_to_string(skill().join("rust/runtime/src/bin/sub_runner.rs")).unwrap();
    for token in [
        "let owner = format!(",
        "while lo > 0 && is_chars(cs[lo - 1])",
        "if token != owner",
    ] {
        assert!(
            source.contains(token),
            "policy-distribution matches {token:?} in sub_runner.rs"
        );
    }
}

/// Standard base64 (with padding) of a request, as the main's `Ssh` runner sends it.
fn base64(data: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            out.push(if i <= chunk.len() {
                A[(n >> (18 - 6 * i)) as usize & 63] as char
            } else {
                '='
            });
        }
    }
    out
}

/// A JSON string literal.
fn js(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The main's `native_multinode_serve` drives its sub through the delivered `native_remote` (native_multinode.rs:739-760,
/// `native_multinode/remote.rs`): `ssh … '<SUB_WORK_DIR>/<SUB_REMOTE_REL> -'` with stdin one base64 line of a JSON object —
/// the call's own keys and `op`·`root`·`run_id` (the root's last name)·`marker`·`created_by`. One request is one
/// operation; exit 0 with the result object on stdout, or a refusal: non-zero with nothing on stdout and the reason on
/// stderr (the main carries its tail). The eleven operations of the main's allowlist all answer here, every one of them
/// on this machine and below a scratch root: `init`·`put`·`text`·`exists`·`remove_owned_tree` on the owned root (the
/// marker file guards every mutating one), `run`·`start`·`stop` on short local programs, `tree` on this process,
/// `gpu`·`ports` on stand-in `nvidia-smi`·`ss` programs on `PATH`. Result keys: `ok`; `rc`·`out`·`err`; the identity
/// `node`·`role`·`pid`·`pgid`·`starttime`·`log_rel`; `node`·`role`·`pid`·`term_status`·`starttime_match`; `text`;
/// `processes` (rows `pid`·`ppid`·`pgid`·`starttime`·`argv`, or the GPU lines); `ports`; `exists`.
#[test]
fn native_remote_stdin_request_protocol() {
    use std::os::unix::fs::PermissionsExt;
    use upstream_runtime::native_multinode::{CREATED_BY, MARKER, SUB_REMOTE_REL};
    let s = scratch("remote");
    let (work, _) = sub_work_tree(&s);
    let runs = s.0.join("runs");
    let root = runs.join("run-20261119");
    let rs = root.to_str().unwrap();
    let stand_in = s.0.join("stand-in");
    std::fs::create_dir_all(&stand_in).unwrap();
    for (name, body) in [
        ("nvidia-smi", "#!/bin/sh\nprintf '4242, vllm, 1024\\n'\n"),
        ("ss", "#!/bin/sh\nprintf 'LISTEN 0 4096 127.0.0.1:8000 0.0.0.0:*\\nLISTEN 0 128 [::]:22 [::]:*\\n'\n"),
    ] {
        write(&stand_in.join(name), body);
        std::fs::set_permissions(stand_in.join(name), std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let call_raw = |stdin: &str, path: Option<&Path>| {
        use std::io::Write;
        let mut c = Command::new(work.join(SUB_REMOTE_REL));
        c.arg("-");
        if let Some(p) = path {
            c.env("PATH", p);
        }
        c.current_dir(&s.0)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = c.spawn().unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(stdin.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    };
    let request = |root: &str, fields: &str| {
        let run_id = Path::new(root).file_name().unwrap().to_str().unwrap();
        let sep = if fields.is_empty() { "" } else { ", " };
        format!(
            r#"{{{fields}{sep}"root": {}, "run_id": {}, "marker": {}, "created_by": {}}}"#,
            js(root),
            js(run_id),
            js(MARKER),
            js(CREATED_BY)
        )
    };
    let send = |root: &str, fields: &str, path: Option<&Path>| {
        call_raw(&base64(request(root, fields).as_bytes()), path)
    };
    let ok = |fields: &str| {
        let out = send(rs, fields, None);
        assert_eq!(
            out.status.code(),
            Some(0),
            "{fields}: {}",
            text(&out.stderr)
        );
        Json::parse(&text(&out.stdout))
            .unwrap_or_else(|e| panic!("{fields}: {e}: {}", text(&out.stdout)))
    };
    let refused = |out: Output, what: &str| {
        assert!(
            matches!(out.status.code(), Some(c) if c != 0),
            "{what}: {:?}",
            out.status
        );
        assert_eq!(text(&out.stdout), "", "{what}");
        assert!(
            !text(&out.stderr).trim().is_empty(),
            "{what}: a reason on stderr"
        );
    };
    let is_ok = |doc: &Json| {
        assert!(
            doc.same(&Json::parse(r#"{"ok": true}"#).unwrap()),
            "{doc:?}"
        )
    };

    assert!(ok(r#""op": "exists""#).same(&Json::parse(r#"{"exists": false}"#).unwrap()));
    is_ok(&ok(r#""op": "init", "ensure_parent": true"#));
    assert!(root.join(MARKER).is_file());
    refused(
        send(rs, r#""op": "init", "ensure_parent": true"#, None),
        "init of an existing root",
    );
    assert!(ok(r#""op": "exists""#).same(&Json::parse(r#"{"exists": true}"#).unwrap()));
    is_ok(&ok(&format!(
        r#""op": "put", "rel": "bin/tool.sh", "data": {}, "mode": 493"#,
        js(&base64(b"hello\n"))
    )));
    assert_eq!(
        std::fs::read_to_string(root.join("bin/tool.sh")).unwrap(),
        "hello\n"
    );
    assert_eq!(
        std::fs::metadata(root.join("bin/tool.sh"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o755
    );
    refused(
        send(
            rs,
            &format!(
                r#""op": "put", "rel": "bin/tool.sh", "data": {}, "mode": 493"#,
                js(&base64(b"x"))
            ),
            None,
        ),
        "put over a file",
    );
    assert!(ok(r#""op": "text", "rel": "bin/tool.sh""#)
        .same(&Json::parse(r#"{"text": "hello\n"}"#).unwrap()));
    refused(
        send(rs, r#""op": "text", "rel": "../outside""#, None),
        "a path escaping the root",
    );
    refused(
        send(rs, r#""op": "text", "rel": "/etc/hostname""#, None),
        "an absolute path",
    );
    let doc = ok(
        r#""op": "run", "argv": ["sh", "-c", "printf out; printf err >&2; exit 3"], "timeout": 30"#,
    );
    assert!(
        doc.same(&Json::parse(r#"{"rc": 3, "out": "out", "err": "err"}"#).unwrap()),
        "{doc:?}"
    );
    let me = std::process::id().to_string();
    let doc = ok(&format!(r#""op": "tree", "pids": [{me}], "pgids": []"#));
    let rows = doc.at("processes").arr().unwrap();
    assert!(
        rows.iter().any(|r| r.at("pid") == &Json::Num(me.clone())),
        "{doc:?}"
    );
    for row in rows {
        assert_eq!(row.keys().len(), 5, "{row:?}");
        for (key, kind) in [
            ("pid", "number"),
            ("ppid", "number"),
            ("pgid", "number"),
            ("starttime", "string"),
            ("argv", "string"),
        ] {
            assert_eq!(row.at(key).kind(), kind, "{key}");
        }
    }
    // start a short-lived local program in its own session, then stop it by its identity
    let identity = ok(
        r#""op": "start", "argv": ["sleep", "30"], "log_rel": "logs/sleep.log", "role": "probe", "node": "sub""#,
    );
    assert_eq!(identity.keys().len(), 6, "{identity:?}");
    for (key, kind) in [
        ("node", "string"),
        ("role", "string"),
        ("pid", "number"),
        ("pgid", "number"),
        ("starttime", "string"),
        ("log_rel", "string"),
    ] {
        assert_eq!(identity.at(key).kind(), kind, "{key}");
    }
    assert_eq!(
        (
            identity.at("node").str(),
            identity.at("role").str(),
            identity.at("log_rel").str()
        ),
        (Some("sub"), Some("probe"), Some("logs/sleep.log"))
    );
    assert!(root.join("logs/sleep.log").is_file());
    let row = |j: &Json| match j {
        Json::Obj(m) => format!(
            "{{{}}}",
            m.iter()
                .map(|(k, v)| format!(
                    "{}: {}",
                    js(k),
                    match v {
                        Json::Str(t) => js(t),
                        Json::Num(n) => n.clone(),
                        other => panic!("{other:?}"),
                    }
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        other => panic!("{other:?}"),
    };
    let doc = ok(&format!(
        r#""op": "stop", "identity": {}, "wait": 10, "pid_only": false"#,
        row(&identity)
    ));
    assert_eq!(doc.keys().len(), 5, "{doc:?}");
    assert_eq!(doc.at("pid"), identity.at("pid"));
    assert_eq!(doc.at("term_status").str(), Some("term-exited"), "{doc:?}");
    assert_eq!(doc.at("starttime_match"), &Json::Bool(true));
    let doc = ok(
        r#""op": "stop", "identity": {"node": "sub", "role": "gone", "pid": 2147483647, "pgid": 2147483647, "starttime": "1"}, "wait": 1"#,
    );
    assert_eq!(doc.at("term_status").str(), Some("already-gone"), "{doc:?}");
    // gpu and ports read their host tools from PATH: stand-ins here
    let out = send(rs, r#""op": "gpu""#, Some(&stand_in));
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(Json::parse(&text(&out.stdout))
        .unwrap()
        .same(&Json::parse(r#"{"processes": ["4242, vllm, 1024"]}"#).unwrap()));
    let out = send(rs, r#""op": "ports""#, Some(&stand_in));
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(Json::parse(&text(&out.stdout))
        .unwrap()
        .same(&Json::parse(r#"{"ports": [22, 8000]}"#).unwrap()));
    let empty_path = s.0.join("empty-path");
    std::fs::create_dir_all(&empty_path).unwrap();
    refused(
        send(rs, r#""op": "gpu""#, Some(&empty_path)),
        "gpu without its tool",
    );
    // the owned-root guard: a wrong marker, a root whose name is not the run id
    write(&runs.join("other/").join(MARKER), "{}\n");
    refused(
        send(
            runs.join("other").to_str().unwrap(),
            &format!(
                r#""op": "put", "rel": "x", "data": {}, "mode": 420"#,
                js(&base64(b"x"))
            ),
            None,
        ),
        "a wrong marker",
    );
    let mismatched = request(rs, r#""op": "remove_owned_tree""#)
        .replace("\"run_id\": \"run-20261119\"", "\"run_id\": \"run-other\"");
    refused(
        call_raw(&base64(mismatched.as_bytes()), None),
        "a root that is not the run id",
    );
    assert!(root.is_dir());
    is_ok(&ok(r#""op": "remove_owned_tree""#));
    assert!(!root.exists());
    assert!(ok(r#""op": "exists""#).same(&Json::parse(r#"{"exists": false}"#).unwrap()));
    // requests the runner refuses: an operation outside the allowlist, a request that is not base64 or not JSON, a
    // missing request key
    refused(
        send(rs, r#""op": "rm_rf""#, None),
        "an operation outside the allowlist",
    );
    refused(call_raw("not base64 !!\n", None), "not base64");
    refused(call_raw(&base64(b"not json"), None), "not JSON");
    refused(
        call_raw(
            &base64(br#"{"op": "exists", "run_id": "x", "marker": "m", "created_by": "c"}"#),
            None,
        ),
        "no root",
    );
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
