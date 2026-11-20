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
//! - closing_sequence.sh·sync_branches.sh: `layer_ledger` subcommands, JSON on stdout, plain lines, exit codes;
//! - closing_sequence.sh: `push_branches --repo --remote --branch…` exit 0 / non-zero.
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

/// policy-distribution runs these self-tests through the launchers and needs exit 0; smoke_clone.sh also greps the
/// merged output of `render_dockerfile.sh --self-test` for `self-test OK|render` (case-insensitive).
#[test]
fn consumer_self_tests_pass() {
    let s = scratch("selftest");
    let rt = runtime_root(&s);
    for launcher in [
        scripts("budget_preflight.sh"),
        scripts("layer_ledger.sh"),
        scripts("push_branches.sh"),
        scripts("render_dockerfile.sh"),
        scripts("slave_forward.sh"),
        ".claude/skills/terraforming_node/scripts/node_role_contract.sh".to_string(),
    ] {
        let out = launch(&rt, &s.0, &launcher, &["--self-test"]);
        let merged = format!("{}{}", text(&out.stdout), text(&out.stderr));
        assert_eq!(
            out.status.code(),
            Some(0),
            "{launcher} --self-test:\n{merged}"
        );
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
/// `bootstrap_canary.rs`, sync_to_sub.sh): exit 0; `value` prints the bare value; `json` prints `{"<field>": {"value",
/// "source"}}` (or `{"<field>": null}` when the field does not apply — the canary falls back to the top level); a single
/// sub is `delivery_plane` = `dormant` (a2a-agent), a multi sub `active`.
#[test]
fn node_role_contract_evaluate_value_and_json() {
    let s = scratch("role");
    let rt = runtime_root(&s);
    let launcher = ".claude/skills/terraforming_node/scripts/node_role_contract.sh";
    let none = s.0.join("none.yaml");
    write(&none, "topology: single\ngpus_per_node: 1\nterraforming:\n  complete: true\n  branch_verified: true\nnodes: []\n");
    let sub = s.0.join("sub.yaml");
    write(&sub, SINGLE_WITH_SUB);
    let eval = |m: &Path, field: &str, format: &str| {
        let out = launch(
            &rt,
            &s.0,
            launcher,
            &[
                "evaluate",
                "--manifest",
                m.to_str().unwrap(),
                "--topology",
                "single",
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
    for m in [&none, &sub] {
        for field in ["delivery_plane", "tool_plane", "sub_mode"] {
            let doc = Json::parse(&eval(m, field, "json")).unwrap();
            assert_eq!(doc.keys(), [field]);
            let v = doc.at(field);
            if v != &Json::Null {
                assert_eq!(v.at("source").kind(), "string", "{field}");
                v.at("value");
            }
            let value = eval(m, field, "value");
            if let Some(Json::Str(want)) = v.get("value") {
                assert_eq!(
                    value.trim_end(),
                    want,
                    "{field}: value prints the json value"
                );
            }
        }
    }
    let doc = Json::parse(&eval(&sub, "sub_mode", "json")).unwrap();
    assert_eq!(
        doc.at("sub_mode").at("value").str(),
        Some("a2a-agent"),
        "{doc:?}"
    );
    // sync_to_sub.sh compares the value with `active` and reads the source through `inline_json delivery-source`: a
    // single sub (a2a-agent) is dormant, a multi sub (ray-worker) is active
    assert_eq!(eval(&sub, "delivery_plane", "value"), "dormant\n");
    let multi = s.0.join("multi.yaml");
    write(
        &multi,
        &SINGLE_WITH_SUB.replace("topology: single", "topology: multi"),
    );
    let out = launch(
        &rt,
        &s.0,
        launcher,
        &[
            "evaluate",
            "--manifest",
            multi.to_str().unwrap(),
            "--topology",
            "multi",
            "--field",
            "delivery_plane",
            "--format",
            "value",
        ],
    );
    assert_eq!(
        (out.status.code(), text(&out.stdout).as_str()),
        (Some(0), "active\n"),
        "{}",
        text(&out.stderr)
    );
    let json = eval(&sub, "delivery_plane", "json");
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
/// `--env-tier KEY` prints `{"tier", "detail"}`; a manifest without the Flag is exit 4 for `--canonical-kind compose`.
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
    for key in ["NCCL_IB_HCA", "MAX_JOBS", "NO_SUCH_KEY_XYZ"] {
        let out = rd(&["--env-tier", key]);
        if out.status.code() == Some(0) {
            let doc = Json::parse(&text(&out.stdout)).unwrap();
            assert_eq!(doc.at("tier").kind(), "string", "{key}");
            assert!(
                matches!(doc.at("detail").kind(), "string" | "null"),
                "{key}"
            );
            if key != "NO_SUCH_KEY_XYZ" {
                assert_eq!(doc.at("detail").kind(), "string", "{key}");
            }
        } else {
            assert_ne!(key, "MAX_JOBS");
            assert!(
                text(&out.stderr).contains(": "),
                "{key}: the reason after the first ': '"
            );
        }
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

/// `layer_ledger` (closing_sequence.sh, sync_branches.sh): `sync-point` and `divergence` print one JSON object on stdout
/// (divergence: `status`, `basis`, `base`, `sync_point`, `absorbed` when there is one, read through `inline_json
/// divergence-summary`; messages stay on stderr); `emit-skeleton` prints an
/// entry, `validate --entry-file` accepts it, `entry-id` and `utc-to-kst` print one plain line; exit 0.
#[test]
fn layer_ledger_closing_sequence_subcommands() {
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
    ]);
    let doc = Json::parse(&text(&out.stdout))
        .unwrap_or_else(|e| panic!("{e}: {}{}", text(&out.stdout), text(&out.stderr)));
    assert!(
        matches!(out.status.code(), Some(0) | Some(5)),
        "{doc:?} {}",
        text(&out.stderr)
    );
    for key in ["status", "basis", "base", "sync_point"] {
        doc.at(key);
    }
    if let Some(a) = doc.get("absorbed") {
        assert!(matches!(a, Json::Arr(_) | Json::Obj(_)), "{a:?}");
    }
    // sync_branches.sh reads it through `inline_json divergence-summary`: status, basis, base12, sync_point.status, absorbed n
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
    assert_eq!(Some(fields[0]), doc.at("status").str());
    assert_eq!(Some(fields[3]), doc.at("sync_point").at("status").str());
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
    let entry = s.0.join("entry.json");
    Json::parse(&text(&out.stdout)).unwrap();
    // the skeleton leaves the injected time to the entry's author (`<FILL: …>`); closing_sequence.sh prints the skeleton
    // and reads the authored entry file with entry-id and utc-to-kst
    let skeleton = text(&out.stdout);
    let filled: String = skeleton
        .lines()
        .map(|l| match (l.find("\"<FILL"), l.rfind('"')) {
            (Some(a), Some(b)) if b > a && l[a..].contains("시각") => {
                format!("{}\"2026-11-18T03:00:00Z\"{}", &l[..a], &l[b + 1..])
            }
            _ => l.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n");
    write(&entry, &filled);
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
    let out = ll(&["validate", "--entry-file", entry.to_str().unwrap()]);
    assert!(
        matches!(out.status.code(), Some(0) | Some(5)),
        "{}{}",
        text(&out.stdout),
        text(&out.stderr)
    );
    Json::parse(&text(&out.stdout)).unwrap();
    assert_eq!(ll(&["no-such-subcommand"]).status.code(), Some(2));
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
/// payload) and `bundle --root R --out F` (the last stdout line is the bundle's sha256).
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
