//! Contract checks of the hint-publisher bins (plan 26111805 「보존 계약」, cell c21): what their consumers read
//! (consumers.md 「고정 표면」 `hint` row and consumers.tsv) —
//! - policy-distribution `checks.rs`: `hint match --vllm --model [--arch | --include-other] --json` without git,
//!   `[].tag`·`[].arch_match`, the row count; `hint --self-test` and `render_bench_section --selftest` exit 0;
//! - policy-predicates `claims_hint.rs` and the skill documents: the subcommands and their long options;
//! - policy-selftest `hint_suite.rs`: `[hint] FAIL <CODE>:` on stderr, `verify`'s verdict line on stdout, exit codes;
//! - upstream `closing_sequence.sh`·`branch_sync_selftest.rs`: `catalog derive --remote --generated-kst
//!   [--record-missing]` (exit `EXIT_LOCAL_TAG_OBJECT_MISSING` read from `src/catalog.rs`, the fetch hint line on
//!   stderr, `hints/index.json` rows `object: absent-local`) and `catalog classify --index --rc` (`<rc> <absent n>`);
//! - policy-predicates `companion.rs`: the closed list of launchers in `scripts/`, their override variables and the
//!   exported `HINT_SKILL_DIR`;
//! - policy-selftest `hint_suite.rs` (gate relay): `verify`/`push` ask the completion gate first and relay its refusal
//!   JSON with its exit code; an unreadable gate output is a refusal JSON with `HINT_GATE_OUTPUT_UNREADABLE`;
//! - hint's own reading of other bins' surfaces (consumers.md 「계약 시험에 반영할 것」): `evidence_publisher` through
//!   its `.sh` launcher, the native serve proof's `pip_freeze_paths` keys, the blackbox session documents and events
//!   (through the crate's `hint_api` ELF, see the section below).
//!
//! The bins run for real (through the launchers where the consumers do), in scratch directories and throwaway local
//! git repositories (the "remote" is a bare repository on disk). Values (L2) and exit codes, codes and marker
//! prefixes (L1) are fixed; message texts are not (L3).
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use json::Json;

/// `.claude/skills/hint-publisher` (this crate is its `rust/`).
fn skill() -> PathBuf {
    std::fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("..")).unwrap()
}

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn scratch(tag: &str) -> Scratch {
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let p = std::env::temp_dir().join(format!("hint-contract-{tag}-{}-{n}", std::process::id()));
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

/// `bash scripts/hint.sh <args>` with `HINT_BIN` = this build (the consumers' form).
fn hint(cwd: &Path, args: &[&str]) -> Output {
    Command::new("bash")
        .arg(skill().join("scripts/hint.sh"))
        .args(args)
        .env("HINT_BIN", env!("CARGO_BIN_EXE_hint"))
        .current_dir(cwd)
        .output()
        .unwrap()
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(["-c", "user.email=contract@example.invalid", "-c", "user.name=contract", "-c", "commit.gpgSign=false", "-c", "tag.gpgSign=false"])
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {}", text(&out.stderr));
}

/// The closed launcher list of `scripts/` exists; every bin launcher honours its override variable, refuses an absent
/// executable with exit 2 (nothing on stdout) and exports `HINT_SKILL_DIR` (default = the skill directory, a value
/// already set is kept).
#[test]
fn launchers_overrides_and_skill_dir_export() {
    for name in ["build_rust.sh", "hint.sh", "hint_api.sh", "render_bench_section.sh"] {
        assert!(skill().join("scripts").join(name).is_file(), "{name}");
    }
    let s = scratch("launch");
    let probe = s.0.join("probe");
    write(&probe, "#!/bin/sh\nprintf '%s\\n' \"$HINT_SKILL_DIR\"\n");
    Command::new("chmod").arg("+x").arg(&probe).status().unwrap();
    for (launcher, var) in [("hint.sh", "HINT_BIN"), ("hint_api.sh", "HINT_API_BIN"), ("render_bench_section.sh", "RENDER_BENCH_SECTION_BIN")] {
        let run = |bin: &Path, skill_dir: Option<&str>| {
            let mut c = Command::new("bash");
            c.arg(skill().join("scripts").join(launcher)).env(var, bin).env_remove("HINT_SKILL_DIR").current_dir(&s.0);
            if let Some(d) = skill_dir {
                c.env("HINT_SKILL_DIR", d);
            }
            c.output().unwrap()
        };
        let out = run(Path::new("/nonexistent/evs-contract-bin"), None);
        assert_eq!(out.status.code(), Some(2), "{launcher}");
        assert_eq!(text(&out.stdout), "", "{launcher}");
        let out = run(&probe, None);
        assert_eq!(out.status.code(), Some(0), "{launcher}");
        let exported = PathBuf::from(text(&out.stdout).trim_end());
        assert_eq!(std::fs::canonicalize(&exported).unwrap(), skill(), "{launcher}: default HINT_SKILL_DIR");
        let out = run(&probe, Some("/kept/by/caller"));
        assert_eq!(text(&out.stdout).trim_end(), "/kept/by/caller", "{launcher}");
    }
}

/// The subcommands and the long options the consumers and the documents name are part of the parser (each
/// subcommand's `--help` is exit 0 and lists them); an unknown option is a usage error, exit 2, nothing on stdout.
#[test]
fn hint_subcommands_and_long_options() {
    let s = scratch("argv");
    let table: [(&[&str], &[&str]); 14] = [
        (&[], &["--repo", "--self-test"]),
        (&["publish"], &["--campaign", "--cell", "--publication", "--replay", "--node", "--generated-utc", "--lineage-add", "--out", "--remote", "--no-remote-check", "--docker-read-only", "--docker-probe"]),
        (&["continue"], &["--campaign", "--cell", "--draft", "--node", "--generated-utc", "--approved-by", "--approved-utc", "--remote", "--no-push", "--factcheck-waiver", "--hint-clone", "--guide-commit"]),
        (&["lint"], &["--draft", "--json"]),
        (&["refresh"], &["--draft"]),
        (&["refacts"], &["--draft", "--docker-read-only"]),
        (&["excerpt"], &["--draft", "--source", "--lines", "--numbered"]),
        (&["name"], &["--campaign", "--cell", "--publication", "--node", "--json"]),
        (&["verify"], &["--tag", "--draft", "--hint-clone"]),
        (&["push"], &["--tag", "--remote", "--apply", "--draft"]),
        (&["catalog", "derive"], &["--remote", "--generated-kst", "--dry-run", "--record-missing", "--allow-empty"]),
        (&["catalog", "classify"], &["--index", "--rc"]),
        (&["match"], &["--vllm", "--model", "--arch", "--include-other", "--json"]),
        (&["branch-transition"], &["--remote", "--generated-utc", "--approved-by", "--approved-utc"]),
    ];
    for (sub, options) in table {
        let mut args: Vec<&str> = sub.to_vec();
        args.push("--help");
        let out = hint(&s.0, &args);
        let help = text(&out.stdout);
        assert_eq!(out.status.code(), Some(0), "{sub:?}");
        for o in options {
            assert!(help.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-')).any(|w| w == *o), "{sub:?} lacks {o}:\n{help}");
        }
    }
    let out = hint(&s.0, &["--repo", s.0.to_str().unwrap(), "match", "--vllm", "1", "--model", "m", "--bogus"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(text(&out.stdout), "");
}

const INDEX: &str = r#"{"schema": 5, "hints": [
 {"tag": "hint/0.31.0/m-one/gb10-x/r1", "vllm": "0.31.0", "model": "m-one", "arch": "gb10-x", "recipe": "r1", "base_model": null},
 {"tag": "hint/0.30.0/other/h100/r2", "vllm": "0.30.0", "model": "other", "arch": "h100", "recipe": "r2", "base_model": null}
]}"#;

/// `match` is read-only and gitless (`PATH` cut, as policy-distribution runs it): a JSON array on stdout whose rows
/// carry `tag` and `arch_match` (`exact`·`prefix`·`unspecified`…); `--include-other` returns every index row.
#[test]
fn match_json_rows_without_git() {
    let s = scratch("match");
    write(&s.0.join("hints/index.json"), INDEX);
    let run = |extra: &[&str]| {
        let mut args = vec!["--repo", s.0.to_str().unwrap(), "match", "--vllm", "0.31.0", "--model", "m-one"];
        args.extend_from_slice(extra);
        let out = Command::new(env!("CARGO_BIN_EXE_hint")).args(&args).env("PATH", "/nonexistent").current_dir(&s.0).output().unwrap();
        assert_eq!(out.status.code(), Some(0), "{extra:?}: {}", text(&out.stderr));
        Json::parse(&text(&out.stdout)).unwrap_or_else(|e| panic!("{extra:?}: {e}"))
    };
    for (arch, want) in [(Some("gb10-x"), "exact"), (Some("gb10"), "prefix"), (None, "unspecified")] {
        let mut extra: Vec<&str> = Vec::new();
        if let Some(a) = arch {
            extra.extend(["--arch", a]);
        }
        extra.push("--json");
        let rows = run(&extra);
        let rows = rows.arr().unwrap();
        assert_eq!(rows.len(), 1, "{arch:?}");
        assert_eq!(rows[0].at("tag").str(), Some("hint/0.31.0/m-one/gb10-x/r1"));
        assert_eq!(rows[0].at("arch_match").str(), Some(want), "{arch:?}");
        assert_eq!(rows[0].at("vllm_match"), &Json::Bool(true));
        assert_eq!(rows[0].at("relation").kind(), "string");
    }
    let all = run(&["--include-other", "--json"]);
    assert_eq!(all.arr().unwrap().len(), 2, "--include-other = every index row");

    // the distribution form: no --repo and no git; the root is found from the executable's place in the tree
    let d = scratch("match-dist");
    write(&d.0.join("hints/index.json"), INDEX);
    let exe = d.0.join(".claude/skills/hint-publisher/rust/target/release/hint");
    std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
    std::fs::copy(env!("CARGO_BIN_EXE_hint"), &exe).unwrap();
    let out = Command::new(&exe)
        .args(["match", "--vllm", "0.31.0", "--model", "m-one", "--arch", "gb10-x", "--json"])
        .env("PATH", "/nonexistent")
        .env("HINT_SKILL_DIR", d.0.join(".claude/skills/hint-publisher"))
        .current_dir(&d.0)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let rows = Json::parse(&text(&out.stdout)).unwrap();
    assert_eq!(rows.arr().unwrap()[0].at("arch_match").str(), Some("exact"));
}

/// Failures: `[hint] FAIL <CODE>:` on stderr with a non-zero exit and nothing on stdout; `verify` writes its verdict
/// line on stdout (`[hint] verify FAIL <tag>` … `codes <CODE>`, exit 1).
#[test]
fn failure_codes_on_stderr_and_verify_verdict_on_stdout() {
    let s = scratch("fail");
    let repo = s.0.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["commit", "-q", "--allow-empty", "-m", "base"]);
    let r = repo.to_str().unwrap();
    let cases: [(&[&str], &str); 4] = [
        (&["--repo", r, "match", "--vllm", "1", "--model", "m"], "HINT_CATALOG_INDEX_ABSENT"),
        (&["--repo", r, "lint", "--draft", "nodraft", "--json"], "HINT_DRAFT_STATE_UNREADABLE"),
        (&["--repo", r, "push", "--tag", "hint/0.31.0/m/a/r", "--remote", "fx"], "HINT_PUSH_CENTRAL_AUTHORITY_ABSENT"),
        (&["--repo", r, "catalog", "derive", "--remote", "fx", "--generated-kst", "2026-11-18T21:00:00"], "HINT_CATALOG_CENTRAL_FLAG_ABSENT"),
    ];
    for (args, code) in cases {
        let out = hint(&repo, args);
        assert!(matches!(out.status.code(), Some(c) if c != 0), "{args:?}");
        assert_eq!(text(&out.stdout), "", "{args:?}");
        assert!(text(&out.stderr).lines().any(|l| l.starts_with(&format!("[hint] FAIL {code}:"))), "{args:?}: {}", text(&out.stderr));
    }
    let out = hint(&repo, &["--repo", r, "verify", "--tag", "hint/0.31.0/m/a/r"]);
    assert_eq!(out.status.code(), Some(1));
    let so = text(&out.stdout);
    let verdict = so.lines().find(|l| l.starts_with("[hint] verify FAIL hint/0.31.0/m/a/r")).unwrap_or_else(|| panic!("{so}"));
    assert!(verdict.contains("HINT_TAG_ABSENT"), "{verdict}");
}

/// `EXIT_LOCAL_TAG_OBJECT_MISSING` as upstream's branch-sync self-test reads it: the `pub const` line of
/// `src/catalog.rs`.
fn exit_local_tag_object_missing() -> i32 {
    let src = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/catalog.rs")).unwrap();
    src.lines()
        .find_map(|l| {
            l.trim_start()
                .strip_prefix("pub const EXIT_LOCAL_TAG_OBJECT_MISSING")
                .and_then(|r| r.split_once('='))
                .and_then(|(_, v)| v.split(';').next())
                .and_then(|v| v.trim().parse().ok())
        })
        .expect("pub const EXIT_LOCAL_TAG_OBJECT_MISSING in src/catalog.rs")
}

/// The closing sequence: a remote hint tag whose object is not local stops `catalog derive` with the constant's exit
/// code, the fetch command of the queried remote on stderr and no write; `--record-missing` writes the row as
/// `object: absent-local` and exits 0; `catalog classify` prints `<constant> <absent rows>` (rows only for `--rc 0`).
#[test]
fn catalog_derive_and_classify_for_the_closing_sequence() {
    let missing = exit_local_tag_object_missing();
    let s = scratch("catalog");
    let (src, bare, work) = (s.0.join("src"), s.0.join("remote.git"), s.0.join("work"));
    for d in [&src, &bare] {
        std::fs::create_dir_all(d).unwrap();
    }
    git(&src, &["init", "-q", "-b", "main"]);
    git(&src, &["commit", "-q", "--allow-empty", "-m", "base"]);
    git(&bare, &["init", "-q", "--bare", "-b", "main"]);
    git(&src, &["push", "-q", bare.to_str().unwrap(), "main"]);
    git(&s.0, &["clone", "-q", bare.to_str().unwrap(), work.to_str().unwrap()]);
    // the tag reaches the remote after the clone: its object is absent from `work`
    git(&src, &["tag", "-a", "-m", "payload", "hint/0.31.0/m-one/gb10-x/r1"]);
    git(&src, &["push", "-q", bare.to_str().unwrap(), "refs/tags/hint/*:refs/tags/hint/*"]);
    git(&work, &["remote", "add", "fx", bare.to_str().unwrap()]);
    write(&work.join("hints/.central_authority"), "contract\n");
    let index = work.join("hints/index.json");
    write(&index, "{\"before\": true}\n");
    let w = work.to_str().unwrap();

    let out = hint(&work, &["--repo", w, "catalog", "derive", "--remote", "fx", "--generated-kst", "2026-11-18T21:00:00"]);
    assert_eq!(out.status.code(), Some(missing), "{}", text(&out.stderr));
    assert!(text(&out.stderr).contains(&format!("git -C {w} fetch fx 'refs/tags/hint/*:refs/tags/hint/*'")), "{}", text(&out.stderr));
    assert_eq!(std::fs::read_to_string(&index).unwrap(), "{\"before\": true}\n", "no write on the stop");

    let classify = |rc: &str| {
        let out = hint(&work, &["--repo", w, "catalog", "classify", "--index", index.to_str().unwrap(), "--rc", rc]);
        assert_eq!(out.status.code(), Some(0));
        text(&out.stdout)
    };
    let line = classify(&missing.to_string());
    assert_eq!(line.split_whitespace().collect::<Vec<_>>(), [missing.to_string().as_str()], "{line:?}");

    let out = hint(&work, &["--repo", w, "catalog", "derive", "--remote", "fx", "--generated-kst", "2026-11-18T21:00:00", "--record-missing"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let doc = Json::parse(&std::fs::read_to_string(&index).unwrap()).unwrap();
    let rows = doc.at("hints").arr().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].at("tag").str(), Some("hint/0.31.0/m-one/gb10-x/r1"));
    assert_eq!(rows[0].at("object").str(), Some("absent-local"));
    assert!(work.join("HINTS.md").is_file());
    assert_eq!(classify("0").split_whitespace().collect::<Vec<_>>(), [missing.to_string().as_str(), "1"]);
}

/// `catalog classify --index I --rc R` as `closing_sequence.sh:227-236` reads it (`read -r rc n`): for `--rc 0` the
/// second field is the number of `object: absent-local` rows, `0` included; an unreadable index leaves it empty
/// ("cannot classify" is not "0 absent"); for any other rc there is no second field. Exit 0 in every case.
#[test]
fn catalog_classify_fields_for_the_closing_sequence() {
    let missing = exit_local_tag_object_missing().to_string();
    let s = scratch("classify");
    let index = s.0.join("index.json");
    let classify = |rc: &str| {
        let out = hint(&s.0, &["catalog", "classify", "--index", index.to_str().unwrap(), "--rc", rc]);
        assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
        text(&out.stdout).split_whitespace().map(str::to_string).collect::<Vec<_>>()
    };
    write(&index, r#"{"schema": 5, "hints": [{"tag": "hint/0.31.0/m-one/gb10-x/r1", "object": "present"}]}"#);
    assert_eq!(classify("0"), [missing.clone(), "0".to_string()], "no absent row is the count 0");
    write(
        &index,
        r#"{"schema": 5, "hints": [{"tag": "hint/0.31.0/m-one/gb10-x/r1", "object": "absent-local"}, {"tag": "hint/0.30.0/o/h100/r2"}]}"#,
    );
    assert_eq!(classify("0"), [missing.clone(), "1".to_string()]);
    assert_eq!(classify("1"), [missing.clone()], "a non-zero rc has no count field");
    std::fs::remove_file(&index).unwrap();
    assert_eq!(classify("0"), [missing], "an unreadable index leaves the count field empty");
}

/// A repository with one annotated hint tag whose footer binds `manifest_ref`, a central-authority flag, a bare remote
/// `fx` and a stand-in completion gate launcher (`completion_gate.sh`: records its arguments, prints `$GATE_OUT`, exits
/// `$GATE_RC`). Returns (scratch, repo, tag).
fn gate_fixture(tag: &str) -> (Scratch, PathBuf, String) {
    let s = scratch(tag);
    let (repo, bare) = (s.0.join("repo"), s.0.join("remote.git"));
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::create_dir_all(&bare).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["commit", "-q", "--allow-empty", "-m", "base"]);
    git(&bare, &["init", "-q", "--bare", "-b", "main"]);
    git(&repo, &["remote", "add", "fx", bare.to_str().unwrap()]);
    write(&repo.join("hints/.central_authority"), "contract\n");
    write(
        &repo.join(".claude/policies/runtime/completion_gate.sh"),
        "#!/bin/bash\nprintf '%s\\n' \"$@\" > \"$GATE_ARGV\"\nprintf '%s' \"$GATE_OUT\"\nexit \"$GATE_RC\"\n",
    );
    let anchor = String::from_utf8(Command::new("git").arg("-C").arg(&repo).args(["rev-parse", "HEAD"]).output().unwrap().stdout).unwrap();
    let name = "hint/0.31.0/m-one/gb10-x/r1".to_string();
    let mut fields = hintlib::value::J::dict();
    for (k, v) in [
        ("version", hintlib::tag::FOOTER_VERSION),
        ("tag", name.as_str()),
        ("topology", "single"),
        ("anchor", anchor.trim()),
        ("manifest_ref", "docs/_evidence/work_manifest.json"),
        ("bench_ref", "../benchmark/benchmark_contract.yaml"),
        ("bench_kind", "certificate"),
    ] {
        fields.set(k, v);
    }
    let message = hintlib::tag::annotation("contract fixture brief", &fields).unwrap();
    let msg = s.0.join("annotation.txt");
    write(&msg, &message);
    git(&repo, &["tag", "-a", "-F", msg.to_str().unwrap(), "--cleanup=verbatim", &name, "HEAD"]);
    (s, repo, name)
}

/// The promotion-gate relay (policy-selftest `hint_suite.rs` `gate_denied`): `verify --tag` and `push --tag --remote
/// --apply` ask the gate first — `completion_gate.sh authorize --manifest <repo>/<manifest_ref> --mode promotion --action
/// hint_verify|hint_push --repo-root <repo>`; a refusal is the gate's JSON on stdout as it was printed, with the gate's
/// exit code, and nothing is pushed. A gate output that is not JSON is `HINT_GATE_OUTPUT_UNREADABLE`: a refusal JSON
/// (`allowed: false`, that reason code, the action) on stdout, exit 2.
#[test]
fn gate_refusal_relayed_by_verify_and_push() {
    let (s, repo, name) = gate_fixture("gate");
    let r = repo.to_str().unwrap();
    let argv = s.0.join("gate_argv.txt");
    let run = |args: &[&str], out: &str, rc: &str| {
        Command::new("bash")
            .arg(skill().join("scripts/hint.sh"))
            .args(args)
            .env("HINT_BIN", env!("CARGO_BIN_EXE_hint"))
            .env("GATE_ARGV", &argv)
            .env("GATE_OUT", out)
            .env("GATE_RC", rc)
            .current_dir(&repo)
            .output()
            .unwrap()
    };
    let manifest = format!("{r}/docs/_evidence/work_manifest.json");
    for (sub, action) in [("verify", "hint_verify"), ("push", "hint_push")] {
        let deny = format!(
            "{{\"allowed\": false, \"action\": \"{action}\", \"exit_code\": 3, \"reason_codes\": [\"RUNTIME_FUNCTIONAL_SMOKE_NOT_PASSED\"]}}\n"
        );
        let args: Vec<&str> = match sub {
            "verify" => vec!["--repo", r, "verify", "--tag", &name],
            _ => vec!["--repo", r, "push", "--tag", &name, "--remote", "fx", "--apply"],
        };
        let out = run(&args, &deny, "3");
        assert_eq!(out.status.code(), Some(3), "{sub}: {}{}", text(&out.stdout), text(&out.stderr));
        assert_eq!(Json::parse(&text(&out.stdout)).unwrap(), Json::parse(&deny).unwrap(), "{sub}: the gate JSON is relayed");
        let called: Vec<String> = std::fs::read_to_string(&argv).unwrap().lines().map(str::to_string).collect();
        assert_eq!(
            called,
            ["authorize", "--manifest", &manifest, "--mode", "promotion", "--action", action, "--repo-root", r],
            "{sub}: the gate's arguments"
        );
        std::fs::remove_file(&argv).unwrap();

        let out = run(&args, "not a JSON document", "0");
        assert_eq!(out.status.code(), Some(2), "{sub}: {}{}", text(&out.stdout), text(&out.stderr));
        let doc = Json::parse(&text(&out.stdout)).unwrap_or_else(|e| panic!("{sub}: {e}: {}", text(&out.stdout)));
        assert_eq!(doc.at("allowed"), &Json::Bool(false), "{sub}");
        assert_eq!(doc.at("action").str(), Some(action), "{sub}");
        assert_eq!(doc.at("reason_codes").strs(), ["HINT_GATE_OUTPUT_UNREADABLE"], "{sub}");
        assert!(text(&out.stderr).lines().any(|l| l.starts_with("[hint] FAIL HINT_GATE_OUTPUT_UNREADABLE:")), "{sub}");
    }
    let remote_tags = Command::new("git").arg("-C").arg(s.0.join("remote.git")).args(["tag", "--list"]).output().unwrap();
    assert_eq!(text(&remote_tags.stdout), "", "a refused push pushes nothing");
}

/// policy-distribution runs `hint.sh --self-test` and needs exit 0 (the self-test's isolated E2E also drives the gate
/// relay: the gate's deny JSON on stdout with its exit code, and an unreadable gate output).
#[test]
fn hint_self_test_passes() {
    let s = scratch("selftest");
    let out = hint(&s.0, &["--self-test"]);
    assert_eq!(out.status.code(), Some(0), "{}\n{}", text(&out.stdout), text(&out.stderr));
}

fn bench_report() -> String {
    [
        "# 성능 보고서",
        "",
        "## 측정 구성 — bench_mode · 도구 · 반복 · 데이터셋",
        "",
        "| 키 | 값 |",
        "|---|---|",
        "| bench_mode | full |",
        "| bench_tool | guidellm |",
        "| repeats | 3 |",
        "| repeats_completed | N/A |",
        "",
        "## 부하 스윕 곡선 (client-load · reload 없음)",
        "",
        "| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |",
        "|---|---|---|---|---|---|---|",
        "| 1 ★판정점 | 22.26 | 22.12 | 113.17 | 750.48 | 42.13 | 16/0 |",
        "",
        "## 다음",
        "",
    ]
    .join("\n")
}

/// `render_bench_section`: `--selftest` is exit 0 (policy-distribution); `--report R --out S` then `--verify
/// --section S --report R` is exit 0 (the documents' "diff 0"); a touched number is exit 1 with nothing on stdout and the
/// reason on stderr (its wording is free); an option no document names is a usage error, exit 2. The documented
/// `--verify --all --hints-dir` is not fixed (the parser refuses it: a defect candidate, contract_tests.tsv).
#[test]
fn render_bench_section_selftest_and_verify() {
    let s = scratch("rbs");
    let rbs = |args: &[&str]| {
        Command::new("bash")
            .arg(skill().join("scripts/render_bench_section.sh"))
            .args(args)
            .env("RENDER_BENCH_SECTION_BIN", env!("CARGO_BIN_EXE_render_bench_section"))
            .current_dir(&s.0)
            .output()
            .unwrap()
    };
    let out = rbs(&["--selftest"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stdout));
    let report = s.0.join("bench_report_x.md");
    let section = s.0.join("section.md");
    write(&report, &bench_report());
    let (r, sec) = (report.to_str().unwrap(), section.to_str().unwrap());
    let out = rbs(&["--report", r, "--out", sec]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let out = rbs(&["--verify", "--section", sec, "--report", r]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let touched = std::fs::read_to_string(&section).unwrap().replace("22.26", "99.99");
    std::fs::write(&section, touched).unwrap();
    let out = rbs(&["--verify", "--section", sec, "--report", r]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(text(&out.stdout), "");
    assert!(!text(&out.stderr).trim().is_empty(), "the reason is on stderr");
    let out = rbs(&["--verify", "--no-such-option"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(text(&out.stdout), "");
}

// ── hint's side of other bins' surfaces ─────────────────────────────────────────────────────────────────────────────
// consumers.md 「계약 시험에 반영할 것」 hint-publisher: the evidence layer starts `evidence_publisher` through its `.sh`
// launcher (core.rs `REL_EVIDENCE_PUBLISHER` is the `.py` name, `owner_launcher` swaps it), checks the native serve
// proof's `pip_freeze_paths` keys and reads the blackbox session data. The `hint` CLI reaches these only inside a full
// `publish` (fake docker, every owner ELF — policy-selftest `hint_suite_cases.rs::green_on_this_checkout`); here the
// crate's `hint_api` ELF drives the same library paths one call per request line.

/// A JSON string literal.
fn q(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// `{"__path__": p}` (the bridge's path argument).
fn qpath(p: &Path) -> String {
    format!("{{\"__path__\": {}}}", q(p.to_str().unwrap()))
}

/// One `hint_api` process: each request a line on stdin, one parsed response per request, exit 0. `HINT_SKILL_DIR`
/// points into the scratch directory so no owner script of this checkout is found or run.
fn hint_api(s: &Scratch, requests: &[String]) -> Vec<Json> {
    use std::io::Write;
    let mut child = Command::new(env!("CARGO_BIN_EXE_hint_api"))
        .env("HINT_SKILL_DIR", s.0.join("no-skill/skills/hint-publisher"))
        .current_dir(&s.0)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = requests.join("\n");
    input.push('\n');
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let rows: Vec<Json> = text(&out.stdout).lines().map(|l| Json::parse(l).unwrap_or_else(|e| panic!("{e}: {l}"))).collect();
    assert_eq!(rows.len(), requests.len(), "{}", text(&out.stdout));
    rows
}

fn value(r: &Json) -> &Json {
    assert_eq!(r.at("kind").str(), Some("value"), "{r:?}");
    r.at("value")
}

/// The `HintError` code of an exception response.
fn error_code(r: &Json) -> String {
    assert_eq!(r.at("kind").str(), Some("exception"), "{r:?}");
    r.at("code").str().unwrap_or_else(|| panic!("{r:?}")).to_string()
}

const PUBLISHER_SH: &str = ".claude/policies/runtime/evidence_publisher.sh";

/// `evidence_publisher <sub> --repo-root <repo> <args…>` starts `.claude/policies/runtime/evidence_publisher.sh` (the
/// `.sh` beside the `.py` name; the `.py` is never run) with cwd = the repository: its stdout JSON comes back with the
/// exit code (an empty stdout = `{}`), a stdout that is not JSON is `HINT_PUBLISHER_OUTPUT_UNREADABLE`, an exit code
/// outside the accepted set is `HINT_PUBLISHER_FAILED` with that code, and without the launcher nothing runs:
/// `HINT_OWNER_LAUNCHER_MISSING`.
#[test]
fn evidence_publisher_is_started_through_its_sh_launcher() {
    let s = scratch("publisher");
    let repo = s.0.join("repo");
    let launcher = repo.join(PUBLISHER_SH);
    let set = |out: &str, rc: i32| {
        write(&launcher, &format!("#!/bin/bash\nprintf '%s\\n' \"$PWD\" \"$@\" > \"$PWD/../argv.txt\"\nprintf '%s' {}\nexit {rc}\n", q(out)));
    };
    let call = |id: &str, accept: &str| {
        format!(
            "{{\"id\": {}, \"fn\": \"evidence._publisher\", \"args\": [{}, \"append-raw\", \"--topic\", \"t1\", \"--kind\", \"simlog\"], \"kwargs\": {{\"accept\": {accept}}}}}",
            q(id),
            qpath(&repo)
        )
    };
    let doc = r#"{"reason_codes": ["PUB_OK"], "messages": {"PUB_OK": "m"}}"#;

    set(doc, 0);
    let rows = hint_api(&s, &[call("a", "[0]")]);
    let v = value(&rows[0]).arr().unwrap();
    assert_eq!(v[0], Json::Num("0".into()));
    assert_eq!(v[1], Json::parse(doc).unwrap(), "the publisher's stdout JSON");
    let argv: Vec<String> = std::fs::read_to_string(s.0.join("argv.txt")).unwrap().lines().map(str::to_string).collect();
    let r = repo.to_str().unwrap();
    assert_eq!(argv, [r, "append-raw", "--repo-root", r, "--topic", "t1", "--kind", "simlog"], "cwd and arguments");

    set("", 0);
    assert_eq!(value(&hint_api(&s, &[call("b", "[0]")])[0]).arr().unwrap()[1], Json::Obj(Vec::new()), "empty stdout = {{}}");

    set("not json", 0);
    assert_eq!(error_code(&hint_api(&s, &[call("c", "[0]")])[0]), "HINT_PUBLISHER_OUTPUT_UNREADABLE");

    set(doc, 3);
    let rows = hint_api(&s, &[call("d", "[0]"), call("e", "[0, 3]")]);
    assert_eq!(error_code(&rows[0]), "HINT_PUBLISHER_FAILED");
    assert_eq!(rows[0].at("exit_code"), &Json::Num("3".into()));
    assert_eq!(value(&rows[1]).arr().unwrap()[0], Json::Num("3".into()), "an accepted non-zero code is a value");

    std::fs::remove_file(&launcher).unwrap();
    std::fs::remove_file(s.0.join("argv.txt")).unwrap();
    let py = repo.join(".claude/policies/runtime/evidence_publisher.py");
    write(&py, "#!/bin/bash\ntouch \"$PWD/../py-ran\"\n");
    assert_eq!(error_code(&hint_api(&s, &[call("f", "[0]")])[0]), "HINT_OWNER_LAUNCHER_MISSING");
    assert!(!s.0.join("py-ran").exists() && !s.0.join("argv.txt").exists(), "nothing ran");
}

/// The native serve proof (upstream `native_multinode_serve`, consumers.tsv `e1.rs` row): `pip_freeze_paths` must be
/// exactly the two keys `main` and `sub`, each a preserved regular file under the repository; with them the evidence
/// carries both paths, any other key set is `HINT_NATIVE_EVIDENCE_MISSING`.
#[test]
fn native_serve_proof_needs_exactly_main_and_sub_pip_freeze_paths() {
    let s = scratch("native");
    let repo = s.0.join("repo");
    let keep = "output/multi/benchlog/native_c1-a";
    for f in ["native-install.sh", "pip-freeze-main.txt", "pip-freeze-sub.txt"] {
        write(&repo.join(keep).join(f), "x\n");
    }
    let node = r#"{"root_absent": true, "owned_processes": [], "owned_gpu_processes": [], "unknown": []}"#;
    write(
        &repo.join(keep).join("cleanup.json"),
        &format!(
            r#"{{"kind": "native_multinode_cleanup_attestation", "plane": "native", "status": "PASS", "errors": [], "evidence_preserved": true, "run_id": "r1", "nodes": {{"main": {node}, "sub": {node}}}}}"#
        ),
    );
    let proof = |freezes: &str| {
        format!(
            r#"{{"plane": "native", "kind": "native_multinode_serve_proof", "status": "PASS", "cell": "c1-a", "run_id": "r1",
            "health": {{"ok": true}}, "inference": {{"ok": true, "completion_text_len": 5}},
            "endpoints": {{"health": {{"http_status": 200}}, "inference": {{"http_status": 200}}}},
            "native_install_path": "{keep}/native-install.sh", "cleanup_attestation_path": "{keep}/cleanup.json",
            "pip_freeze_paths": {freezes}}}"#
        )
        .replace('\n', " ")
    };
    let call = |id: &str, freezes: &str| {
        format!(
            "{{\"id\": {}, \"fn\": \"evidence._native_producer_evidence\", \"args\": [{}, \"multi\", \"c1-a\", {}, \"output/multi/benchlog/serve_proof_c1-a.json\"]}}",
            q(id),
            qpath(&repo),
            proof(freezes)
        )
    };
    let both = format!(r#"{{"main": "{keep}/pip-freeze-main.txt", "sub": "{keep}/pip-freeze-sub.txt"}}"#);
    let only_main = format!(r#"{{"main": "{keep}/pip-freeze-main.txt"}}"#);
    let extra = format!(r#"{{"main": "{keep}/pip-freeze-main.txt", "sub": "{keep}/pip-freeze-sub.txt", "sub2": "{keep}/pip-freeze-sub.txt"}}"#);
    let rows = hint_api(&s, &[call("ok", &both), call("main", &only_main), call("extra", &extra), call("none", "null")]);
    let ev = value(&rows[0]);
    assert_eq!(ev.at("plane").str(), Some("native"));
    let paths = ev.at("pip_freeze_paths");
    assert_eq!(paths.keys(), ["main", "sub"]);
    assert_eq!(paths.at("main").str(), Some(format!("{keep}/pip-freeze-main.txt").as_str()));
    assert_eq!(paths.at("sub").str(), Some(format!("{keep}/pip-freeze-sub.txt").as_str()));
    for r in &rows[1..] {
        assert_eq!(error_code(r), "HINT_NATIVE_EVIDENCE_MISSING", "{r:?}");
    }
}

/// Blackbox session data (terraforming `blackbox_session`, consumers.tsv `e2.rs` row): a session document
/// `docs/logs/<node>/sessions/<session_id>.json` counts only when its `session_id` is its file name and `started_utc`
/// (and `stopped_utc`, when not null) are UTC times; an event belongs to the cell when its `kind` is `serve_start` or
/// `serve_stop`, its `session_id` names such a document whose `config_file`·`topology` are the cell's and whose
/// `started_utc`/`stopped_utc` is the event time, and its `source` (when present) is `blackbox_session`.
#[test]
fn blackbox_session_documents_and_events_hint_reads() {
    let s = scratch("session");
    let repo = s.0.join("repo");
    let sid = "serve-c1-a-20260101T000000Z";
    let sessions = repo.join("docs/logs/n1/sessions");
    write(
        &sessions.join(format!("{sid}.json")),
        &format!(
            r#"{{"session_id": "{sid}", "started_utc": "2026-01-01T00:00:00Z", "stopped_utc": "2026-01-01T01:00:00Z", "config_file": "c1-a", "topology": "multi"}}"#
        ),
    );
    write(&sessions.join("serve-c1-a-other.json"), &format!(r#"{{"session_id": "{sid}", "started_utc": "2026-01-01T00:00:00Z"}}"#));
    let docs = format!("{{\"__call__\": {{\"fn\": \"evidence._node_session_docs\", \"args\": [{}, \"n1\"]}}}}", qpath(&repo));
    let ev = r#"{"cell": "c1-a", "topology": "multi", "sweep": {"index": {"meta": {"config_name": "c1-a"}}}}"#;
    let event = |id: &str, fields: &str, ts: &str| {
        format!(
            "{{\"id\": {}, \"fn\": \"evidence._session_event_is_cell\", \"args\": [{{{fields}}}, {}, {ev}, {docs}]}}",
            q(id),
            q(ts)
        )
    };
    let start = format!(r#""kind": "serve_start", "session_id": "{sid}", "source": "blackbox_session", "topology": "multi""#);
    let stop = format!(r#""kind": "serve_stop", "session_id": "{sid}""#);
    let rows = hint_api(
        &s,
        &[
            format!("{{\"id\": \"docs\", \"fn\": \"evidence._node_session_docs\", \"args\": [{}, \"n1\"]}}", qpath(&repo)),
            event("start", &start, "2026-01-01T00:00:00Z"),
            event("stop", &stop, "2026-01-01T01:00:00Z"),
            event("start-late", &start, "2026-01-01T00:00:01Z"),
            event("other-source", &start.replace("blackbox_session", "other_tool"), "2026-01-01T00:00:00Z"),
            event("other-kind", &start.replace("serve_start", "serve_ready"), "2026-01-01T00:00:00Z"),
        ],
    );
    let found = value(&rows[0]);
    let doc = found.at(sid);
    assert_eq!(doc.at("config_file").str(), Some("c1-a"));
    assert_eq!(doc.at("_source").str(), Some(format!("docs/logs/n1/sessions/{sid}.json").as_str()));
    assert_eq!(found.at("serve-c1-a-other"), &Json::Null, "a document whose session_id is not its name does not count");
    let want = [("start", true), ("stop", true), ("start-late", false), ("other-source", false), ("other-kind", false)];
    for (r, (id, is)) in rows[1..].iter().zip(want) {
        assert_eq!(value(r), &Json::Bool(is), "{id}");
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
