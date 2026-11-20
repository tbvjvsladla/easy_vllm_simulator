//! Contract checks of `evidence_publisher` (plan 26111805 「보존 계약」, cell c21c-contract-tests): the surface its
//! consumers read (runtime-support/contract/consumers.tsv rows of this bin) —
//! - policy-selftest `suite.rs` runs `bash .claude/policies/runtime/evidence_publisher.sh --self-test` and needs exit 0;
//! - the documents (docs.md, adversarial-benchmark `lite-and-publication.md`, terraforming `sub_agent_env.md`, the
//!   document skeletons, wiki-desk SKILL.md) name the subcommands `init`, `append-raw`, `set-narrative`,
//!   `record-capacity-rejection`, `publish-benchmark`, `publish-lite-report`, `set-promotion-target`, `finalize` and
//!   their long options (`--topic`, `--task-class`, `--generated-utc`, `--identity-json`, `--bench-report-src`,
//!   `--verdict-json-src`, …), and `-h`;
//! - `finalize` hands the work manifest to `completion_gate.sh verify` and relays the gate's exit code and stdout
//!   unchanged (consumers.md 「고정 표면」 completion_gate row);
//! - the hint e2e fixture and the sync scripts name the launcher path and its override `EVIDENCE_PUBLISHER_BIN`;
//!   hint-publisher `core.rs:32`·`:667-669` runs the `.sh` launcher beside the `.py` name it keeps;
//! - hint-publisher `evidence/e4.rs` drives `init`·`append-raw`·`set-narrative`·`publish-benchmark`·`publish-lite-report`·
//!   `set-promotion-target`·`finalize` as `<subcommand> --repo-root <repo> …` and reads `reason_codes`·`messages`; the
//!   options it passes beyond the documents (`--benchmark-mode`, `--benchmark-verdict`, `--dest-name`,
//!   `--downgrade-from`, `--downgrade-reason`, `promotion_target`) are run through to the record;
//! - `append-raw` re-writes the publication record and never drops a key it does not know (types.md 4절).
//!
//! The program runs for real through its launcher (argv[0] = the launcher, so the program finds the repository's
//! schemas) with `--repo-root` set to a scratch directory, so nothing is written into the repository. stdout is one JSON
//! object (L2: keys, types, values); exit codes and reason code strings are L1; message texts and stderr notes are L3.
//! A non-UTF-8 argument panics today (`src/lib.rs:197` `std::env::args()`); that is recorded as a defect of the base and
//! not pinned here.
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use json::Json;

fn root() -> PathBuf {
    std::fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")).unwrap()
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
        "policy-publisher-contract-{tag}-{}-{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&p).unwrap();
    Scratch(std::fs::canonicalize(&p).unwrap())
}

fn text(b: &[u8]) -> String {
    String::from_utf8(b.to_vec()).unwrap()
}

fn launcher() -> PathBuf {
    root().join(".claude/policies/runtime/evidence_publisher.sh")
}

/// The completion gate executable `finalize` delegates to (the built repository's; the publisher never re-implements it).
fn gate_bin() -> PathBuf {
    let p = root().join(".claude/rust/policy-gate/target/release/completion_gate");
    assert!(
        p.is_file(),
        "the built repository's completion_gate is required: {}",
        p.display()
    );
    p
}

/// `bash evidence_publisher.sh <args>` with `EVIDENCE_PUBLISHER_BIN` = this build, cwd = `cwd`.
fn publish(cwd: &Path, args: &[&str]) -> Output {
    Command::new("bash")
        .arg(launcher())
        .args(args)
        .env(
            "EVIDENCE_PUBLISHER_BIN",
            env!("CARGO_BIN_EXE_evidence_publisher"),
        )
        .env("COMPLETION_GATE_BIN", gate_bin())
        .current_dir(cwd)
        .output()
        .unwrap()
}

/// stdout is one JSON object carrying `ok` (bool), `reason_codes` (strings), `messages` (object), `schema_version`.
fn envelope(out: &Output) -> Json {
    let so = text(&out.stdout);
    let doc = Json::parse(&so).unwrap_or_else(|e| {
        panic!(
            "stdout is not one JSON document ({e}):\n{so}\nstderr:\n{}",
            text(&out.stderr)
        )
    });
    assert_eq!(doc.kind(), "object", "{so}");
    assert_eq!(doc.at("ok").kind(), "bool", "{so}");
    assert_eq!(doc.at("messages").kind(), "object", "{so}");
    assert_eq!(doc.at("schema_version").kind(), "number", "{so}");
    doc.at("reason_codes").strs();
    doc
}

fn write(path: &Path, body: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

const IDENTITY: &str = r#"{"model": "m", "gpu": "GB10", "vllm": "0.0.0", "quant": null, "topology": "single", "tp": 1}"#;

/// A scratch `--repo-root` holding the identity, raw evidence and gate inputs of a minor_patch publication.
fn publication_root(tag: &str) -> Scratch {
    let s = scratch(tag);
    write(&s.0.join("identity.json"), IDENTITY);
    write(&s.0.join("verify.txt"), "ok\n");
    write(&s.0.join("commit.txt"), "deadbeef\n");
    write(
        &s.0.join("pii.json"),
        r#"{"passed": true, "scanned_paths": ["../../verify.txt", "../../commit.txt"]}"#,
    );
    write(
        &s.0.join("runtime.json"),
        &format!(
            r#"{{"health_ok": true, "functional_smoke_passed": true, "identity": {IDENTITY}, "containers": [{{"name": "svc", "restart_count": 0, "oom_killed": false}}]}}"#
        ),
    );
    s
}

/// The launcher: it is the file the documents and fixtures name; `EVIDENCE_PUBLISHER_BIN` selects the executable; an
/// absent or non-ELF executable is exit 2 with nothing on stdout.
#[test]
fn launcher_override_and_absent_executable() {
    assert!(launcher().is_file());
    let s = scratch("launcher");
    let fake = s.0.join("not-elf");
    write(&fake, "#!/bin/bash\necho '{}'\n");
    std::fs::set_permissions(&fake, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    for bin in [PathBuf::from("/nonexistent/evidence_publisher"), fake] {
        let out = Command::new("bash")
            .arg(launcher())
            .arg("--help")
            .env("EVIDENCE_PUBLISHER_BIN", &bin)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(2), "{}", bin.display());
        assert_eq!(text(&out.stdout), "", "{}", bin.display());
    }
    let out = publish(&s.0, &["--help"]);
    assert_eq!(out.status.code(), Some(0));
}

/// policy-selftest `suite.rs`: `bash evidence_publisher.sh --self-test` is exit 0.
#[test]
fn self_test_through_the_launcher_passes() {
    let s = scratch("selftest");
    let out = publish(&s.0, &["--self-test"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&out.stdout),
        text(&out.stderr)
    );
}

/// `init`, `append-raw` and `finalize` on a scratch `--repo-root`: exit codes, the JSON keys and types the agents and
/// the hint path read, and `finalize` = `completion_gate.sh verify --manifest <work manifest> --repo-root <root>`
/// (same exit code, same stdout; the gate's `exit_code` field equals the process exit code).
#[test]
fn init_append_raw_and_finalize_relay_the_gate() {
    let s = publication_root("roundtrip");
    let r = s.0.to_str().unwrap();
    let out = publish(
        &s.0,
        &[
            "init",
            "--repo-root",
            r,
            "--topic",
            "t1",
            "--task-class",
            "minor_patch",
            "--generated-utc",
            "2026-01-01T00:00:00Z",
            "--identity-json",
            "identity.json",
        ],
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stdout));
    let doc = envelope(&out);
    assert_eq!(doc.at("ok"), &Json::Bool(true));
    assert_eq!(doc.at("publication_id").str(), Some("t1"));
    assert_eq!(doc.at("task_class").str(), Some("minor_patch"));
    assert_eq!(doc.at("record_path").str(), Some("docs/_evidence/t1.json"));
    assert!(s.0.join("docs/_evidence/t1.json").is_file());
    let mut required = doc.at("required_evidence").strs();
    required.sort();
    assert_eq!(required, ["commit", "verification"]);
    doc.at("pending_evidence").strs();
    assert_eq!(doc.at("scaffolded").kind(), "object");

    for (kind, src) in [("verification", "verify.txt"), ("commit", "commit.txt")] {
        let out = publish(
            &s.0,
            &[
                "append-raw",
                "--repo-root",
                r,
                "--topic",
                "t1",
                "--kind",
                kind,
                "--src",
                src,
                "--recorded-utc",
                "2026-01-01T00:01:00Z",
            ],
        );
        assert_eq!(out.status.code(), Some(0), "{}", text(&out.stdout));
        let doc = envelope(&out);
        assert_eq!(doc.at("kind").str(), Some(kind));
        assert_eq!(doc.at("seq"), &Json::Num("1".into()));
        assert_eq!(doc.at("sha256").str().map(str::len), Some(64));
        assert_eq!(
            doc.at("recorded_path").str(),
            Some(format!("docs/_evidence/t1.{kind}.raw.jsonl").as_str())
        );
    }
    let out = publish(
        &s.0,
        &[
            "append-raw",
            "--repo-root",
            r,
            "--topic",
            "t1",
            "--kind",
            "commit",
            "--src",
            "missing.txt",
            "--recorded-utc",
            "2026-01-01T00:02:00Z",
        ],
    );
    assert_eq!(out.status.code(), Some(2));
    let doc = envelope(&out);
    assert_eq!(doc.at("ok"), &Json::Bool(false));
    assert_eq!(
        doc.at("reason_codes").strs(),
        ["RAW_EVIDENCE_SRC_NOT_FOUND"]
    );

    let fin = publish(
        &s.0,
        &[
            "finalize",
            "--repo-root",
            r,
            "--topic",
            "t1",
            "--pii-scan-json",
            "pii.json",
            "--runtime-json",
            "runtime.json",
        ],
    );
    let so = text(&fin.stdout);
    let doc = Json::parse(&so)
        .unwrap_or_else(|e| panic!("finalize stdout is not one JSON document ({e}):\n{so}"));
    let rc = fin.status.code().expect("exit code");
    assert_eq!(doc.at("exit_code"), &Json::Num(rc.to_string()), "{so}");
    assert_eq!(
        rc, 1,
        "a minor_patch publication is not promotion-ready: {so}"
    );
    assert_eq!(doc.at("state").str(), Some("evidence-complete"));
    assert_eq!(doc.at("eligible_for_promotion"), &Json::Bool(false));
    assert_eq!(
        doc.at("reason_codes").strs(),
        ["TASK_CLASS_CAPS_BELOW_PROMOTION:minor_patch"]
    );
    let manifest = s.0.join("docs/_evidence/t1.work-manifest.json");
    assert!(manifest.is_file());
    let gate = Command::new("bash")
        .arg(root().join(".claude/policies/runtime/completion_gate.sh"))
        .args(["verify", "--manifest"])
        .arg(&manifest)
        .arg("--repo-root")
        .arg(&s.0)
        .env("COMPLETION_GATE_BIN", gate_bin())
        .current_dir(&s.0)
        .output()
        .unwrap();
    assert_eq!(
        gate.status.code(),
        fin.status.code(),
        "finalize relays the gate's exit code"
    );
    assert_eq!(
        text(&gate.stdout),
        so,
        "finalize relays the gate's stdout unchanged"
    );
}

/// The subcommands and long options the documents name are part of the parser (no `CLI_USAGE_ERROR`; the answer is a
/// domain reason code), `<subcommand> -h` is exit 0, and usage errors are exit 2 with `CLI_USAGE_ERROR` on stdout.
#[test]
fn subcommands_long_options_and_usage_errors() {
    let s = publication_root("cli");
    let r = s.0.to_str().unwrap();
    let out = publish(
        &s.0,
        &[
            "init",
            "--repo-root",
            r,
            "--topic",
            "t1",
            "--task-class",
            "minor_patch",
            "--generated-utc",
            "2026-01-01T00:00:00Z",
            "--identity-json",
            "identity.json",
        ],
    );
    assert_eq!(out.status.code(), Some(0));
    type Row = (&'static [&'static str], &'static str);
    let rows: [Row; 5] = [
        (
            &[
                "publish-benchmark",
                "--topic",
                "t1",
                "--verdict",
                "PASS",
                "--generated-utc",
                "2026-01-01T00:02:00Z",
                "--bench-report-src",
                "x.md",
                "--verdict-json-src",
                "v.json",
                "--certificate-src",
                "c.yaml",
            ],
            "PUBLISH_BENCHMARK_WRONG_TASK_CLASS",
        ),
        (
            &[
                "publish-lite-report",
                "--topic",
                "t1",
                "--generated-utc",
                "2026-01-01T00:02:00Z",
                "--bench-report-src",
                "x.md",
                "--certificate-src",
                "c.yaml",
            ],
            "PUBLISH_LITE_REPORT_WRONG_TASK_CLASS",
        ),
        (
            &[
                "record-capacity-rejection",
                "--topic",
                "t1",
                "--gate-evidence-src",
                "commit.txt",
                "--reason",
                "r",
                "--recorded-utc",
                "2026-01-01T00:02:00Z",
            ],
            "CAPACITY_REJECTION_WRONG_TASK_CLASS",
        ),
        (
            &[
                "set-narrative",
                "--topic",
                "t1",
                "--kind",
                "plan",
                "--narrative-file",
                "commit.txt",
                "--author",
                "a",
                "--generated-utc",
                "2026-01-01T00:02:00Z",
            ],
            "NARRATIVE_TARGET_NOT_SCAFFOLDED",
        ),
        (
            &[
                "set-promotion-target",
                "--topic",
                "t1",
                "--tag",
                "hint/x",
                "--topology",
                "single",
                "--anchor",
                "abc",
                "--generated-utc",
                "2026-01-01T00:02:00Z",
            ],
            "PROMOTION_TARGET_INVALID",
        ),
    ];
    for (args, code) in rows {
        let mut argv: Vec<&str> = vec![args[0], "--repo-root", r];
        argv.extend_from_slice(&args[1..]);
        let out = publish(&s.0, &argv);
        let doc = envelope(&out);
        assert_eq!(doc.at("ok"), &Json::Bool(false), "{argv:?}");
        assert_eq!(doc.at("reason_codes").strs(), [code], "{argv:?}");
    }
    for sub in [
        "init",
        "append-raw",
        "set-narrative",
        "record-capacity-rejection",
        "publish-benchmark",
        "publish-lite-report",
        "set-promotion-target",
        "finalize",
    ] {
        let out = publish(&s.0, &[sub, "-h"]);
        assert_eq!(out.status.code(), Some(0), "{sub} -h");
        assert!(!text(&out.stdout).is_empty(), "{sub} -h");
    }
    let bad: [&[&str]; 4] = [
        &[],
        &["bogus"],
        &[
            "init",
            "--repo-root",
            r,
            "--topic",
            "t2",
            "--task-class",
            "minor_patch",
            "--generated-utc",
            "2026-01-01T00:00:00Z",
            "--identity-json",
            "identity.json",
            "--extra",
        ],
        &[
            "init",
            "--repo-root",
            r,
            "--topic",
            "t2",
            "--task-class",
            "nope",
            "--generated-utc",
            "2026-01-01T00:00:00Z",
            "--identity-json",
            "identity.json",
        ],
    ];
    for args in bad {
        let out = publish(&s.0, args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        let doc = envelope(&out);
        assert_eq!(doc.at("ok"), &Json::Bool(false), "{args:?}");
        assert_eq!(
            doc.at("reason_codes").strs(),
            ["CLI_USAGE_ERROR"],
            "{args:?}"
        );
    }
}

fn read_json(path: &Path) -> Json {
    let body = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    Json::parse(&body).unwrap_or_else(|e| panic!("{} is not JSON ({e}): {body}", path.display()))
}

/// The options hint-publisher's driver (`evidence/e4.rs` `_publisher`, `drive_publisher`, `_append_simlog`,
/// `set_promotion_target`, `_finalize`) passes after `<subcommand> --repo-root <repo>` reach the publication record:
/// - `init --task-class full_benchmark --benchmark-mode full --benchmark-verdict PASS` records `benchmark.{mode,verdict}`;
/// - `append-raw --kind simlog --src --recorded-utc --dest-name NAME` copies the file into the scaffolded simlog folder
///   under NAME (`recorded_path`);
/// - `publish-benchmark --verdict --generated-utc --bench-report-src` binds the report; the downgrade `init --task-class
///   hint_map_only --benchmark-mode lite --benchmark-verdict --downgrade-from full_benchmark --downgrade-reason R`
///   reclassifies the record (`reclassification.{from,reason}`, `task_class`, `benchmark.mode`), with the reason
///   matched against the report's measurement table; `--downgrade-from` without a reason and `--downgrade-reason`
///   without `--downgrade-from` are their own reason codes (each option is parsed as itself);
/// - `set-promotion-target --tag --topology --anchor --generated-utc` records `promotion_target` (and returns it), and
///   `finalize --pii-scan-json --runtime-json` (accepted exit codes 0 and 1, as the driver accepts them) emits it into
///   the work manifest unchanged.
#[test]
fn hint_driver_options_reach_the_record() {
    let s = scratch("driver");
    let r = s.0.to_str().unwrap();
    write(&s.0.join("identity.json"), IDENTITY);
    let call = |sub: &str, args: &[&str]| {
        let mut argv = vec![sub, "--repo-root", r];
        argv.extend_from_slice(args);
        let out = publish(&s.0, &argv);
        (out.status.code(), envelope(&out))
    };
    let ok = |sub: &str, args: &[&str]| {
        let (rc, doc) = call(sub, args);
        assert_eq!(rc, Some(0), "{sub} {args:?}: {doc:?}");
        assert_eq!(doc.at("ok"), &Json::Bool(true), "{sub} {args:?}");
        doc
    };
    let init = [
        "--topic",
        "b1",
        "--generated-utc",
        "2026-01-01T00:00:00Z",
        "--identity-json",
        "identity.json",
    ];
    let mut full = init.to_vec();
    full.extend_from_slice(&[
        "--task-class",
        "full_benchmark",
        "--benchmark-mode",
        "full",
        "--benchmark-verdict",
        "PASS",
    ]);
    ok("init", &full);
    let record = s.0.join("docs/_evidence/b1.json");
    let rec = read_json(&record);
    assert_eq!(rec.at("benchmark").at("mode").str(), Some("full"));
    assert_eq!(rec.at("benchmark").at("verdict").str(), Some("PASS"));

    write(&s.0.join("measured.json"), "{\"measurement_ok\": true}\n");
    let doc = ok(
        "append-raw",
        &[
            "--topic",
            "b1",
            "--kind",
            "simlog",
            "--src",
            "measured.json",
            "--recorded-utc",
            "2026-01-01T00:01:00Z",
            "--dest-name",
            "level_01_measured.json",
        ],
    );
    let simlog = rec.at("scaffolded").at("simlog").str().unwrap().to_string();
    let copied = format!("{simlog}/level_01_measured.json");
    assert_eq!(doc.at("recorded_path").str(), Some(copied.as_str()));
    assert_eq!(
        std::fs::read_to_string(s.0.join(&copied)).unwrap(),
        "{\"measurement_ok\": true}\n"
    );

    let report = "docs/benchmark/bench_report_26010103_m_GB10_0.0.0.md";
    write(
        &s.0.join(report),
        "# report\n생성일 2026-01-01T03:00:00Z.\n\n## 측정 구성 — bench_mode · 도구 · 반복 (기재 · 게이트 아님)\n\n| 키 | 값 |\n|---|---|\n| bench_mode | lite |\n| bench_mode_kind | downgraded-lite |\n| bench_mode_source | contract |\n| downgrade_reason | run_failed |\n",
    );
    ok(
        "publish-benchmark",
        &[
            "--topic",
            "b1",
            "--verdict",
            "PASS",
            "--generated-utc",
            "2026-01-01T03:05:00Z",
            "--bench-report-src",
            report,
        ],
    );
    let mut lite = init.to_vec();
    lite.extend_from_slice(&["--task-class", "hint_map_only"]);
    for (extra, code) in [
        (
            &["--downgrade-from", "full_benchmark"][..],
            "INIT_DOWNGRADE_REASON_REQUIRED",
        ),
        (
            &["--downgrade-reason", "run_failed"],
            "INIT_DOWNGRADE_REASON_WITHOUT_FROM",
        ),
    ] {
        let mut args = lite.clone();
        args.extend_from_slice(extra);
        let (rc, doc) = call("init", &args);
        assert_ne!(rc, Some(0), "{extra:?}");
        assert_eq!(doc.at("reason_codes").strs(), [code], "{extra:?}");
    }
    let mut down = lite.clone();
    down.extend_from_slice(&[
        "--benchmark-mode",
        "lite",
        "--benchmark-verdict",
        "PASS",
        "--downgrade-from",
        "full_benchmark",
        "--downgrade-reason",
        "run_failed",
    ]);
    let doc = ok("init", &down);
    assert_eq!(
        doc.at("reclassification").at("from").str(),
        Some("full_benchmark")
    );
    let rec = read_json(&record);
    assert_eq!(rec.at("task_class").str(), Some("hint_map_only"));
    assert_eq!(rec.at("benchmark").at("mode").str(), Some("lite"));
    assert_eq!(
        rec.at("reclassification").at("reason").str(),
        Some("run_failed")
    );

    let anchor = "0123456789abcdef0123456789abcdef01234567";
    let doc = ok(
        "set-promotion-target",
        &[
            "--topic",
            "b1",
            "--tag",
            "hint/contract-1",
            "--topology",
            "single TP=1",
            "--anchor",
            anchor,
            "--generated-utc",
            "2026-01-01T05:00:00Z",
        ],
    );
    let target = doc.at("promotion_target").clone();
    assert_eq!(target.at("kind").str(), Some("hint"));
    assert_eq!(target.at("tag").str(), Some("hint/contract-1"));
    assert_eq!(target.at("topology").str(), Some("single TP=1"));
    assert_eq!(target.at("anchor").str(), Some(anchor));
    assert_eq!(read_json(&record).at("promotion_target"), &target);
    write(
        &s.0.join("pii.json"),
        r#"{"passed": true, "scanned_paths": []}"#,
    );
    write(
        &s.0.join("runtime.json"),
        &format!(
            r#"{{"health_ok": true, "functional_smoke_passed": true, "identity": {IDENTITY}, "containers": []}}"#
        ),
    );
    let fin = publish(
        &s.0,
        &[
            "finalize",
            "--repo-root",
            r,
            "--topic",
            "b1",
            "--pii-scan-json",
            "pii.json",
            "--runtime-json",
            "runtime.json",
        ],
    );
    assert!(
        matches!(fin.status.code(), Some(0) | Some(1)),
        "finalize: {:?}\n{}",
        fin.status,
        text(&fin.stdout)
    );
    assert_eq!(
        read_json(&s.0.join("docs/_evidence/b1.work-manifest.json")).at("promotion_target"),
        &target
    );
}

/// `append-raw` never drops a key of the publication record it does not know (types.md 4절 「영속 문서」 4): after the
/// call the record still holds the unknown key and its value. Whether the call then succeeds is not fixed here — the base
/// treats the record as a closed schema and refuses it (see the cell record), which types.md's "preserve" wording does
/// not describe.
#[test]
fn append_raw_never_drops_unknown_record_keys() {
    let s = publication_root("unknown-keys");
    let r = s.0.to_str().unwrap();
    let out = publish(
        &s.0,
        &[
            "init",
            "--repo-root",
            r,
            "--topic",
            "t1",
            "--task-class",
            "minor_patch",
            "--generated-utc",
            "2026-01-01T00:00:00Z",
            "--identity-json",
            "identity.json",
        ],
    );
    assert_eq!(out.status.code(), Some(0));
    let record = s.0.join("docs/_evidence/t1.json");
    let body = std::fs::read_to_string(&record).unwrap();
    assert!(body.contains("\"identity\": {"), "{body}");
    let body = body
        .replacen('{', "{\"_contract_top\": \"keep-top\", ", 1)
        .replacen(
            "\"identity\": {",
            "\"identity\": {\"_contract_item\": \"keep-item\", ",
            1,
        );
    std::fs::write(&record, body).unwrap();
    let out = publish(
        &s.0,
        &[
            "append-raw",
            "--repo-root",
            r,
            "--topic",
            "t1",
            "--kind",
            "verification",
            "--src",
            "verify.txt",
            "--recorded-utc",
            "2026-01-01T00:01:00Z",
        ],
    );
    envelope(&out);
    let rec = read_json(&record);
    assert_eq!(rec.at("_contract_top").str(), Some("keep-top"));
    assert_eq!(
        rec.at("identity").at("_contract_item").str(),
        Some("keep-item")
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
