//! Contract checks of the bench pipeline bins of this crate (plan 26111805 「보존 계약」, cell c21c): what their consumers
//! read (runtime-support/contract/consumers.tsv L1·L2 rows of adversarial-benchmark; the campaign, manifest, scan,
//! turn-budget and topology bins are in `contract_checks_campaign.rs`) —
//! - every consumer starts a bin through its launcher `scripts/<bin>.sh` with the `<NAME>_BIN` override (absent
//!   executable = exit 2, nothing on stdout) and the exported `*_REPO_ROOT`·`*_SCRIPTS_DIR`·`BENCH_VERIFY_REPO_ROOT`…;
//! - policy-distribution `checks.rs` and judge_bench.sh run the self-tests and need exit 0;
//! - broad_search.sh: `bench_preset default` (`VALUE\x1fNAME`), `repeat_axis resolve` (`N\x1fSRC\x1fKIND`),
//!   `bench_shell broad-init`, `sweep_stop --state --now-utc` (exit 0 continue / 3 stop, the stop JSON),
//!   `sweep_stop report …` (`REPEATS\x1fSOURCE`), `verdict_rule authority-check`, `render_sweep_map`, `classify_cell`;
//! - sweep_bench.sh·run_bench.sh·judge_bench.sh·lite_bench.sh·max_envelope.sh: `bench_preset resolve` (`KEY\x1fVALUE`
//!   lines and the record), the `bench_shell` positional operations, `parse_bench`·`parse_guidellm` (stdout = the
//!   measured JSON, `measurement_ok`), `verdict_rule spec-row` (4 `\x1f` fields) and the verdict JSON, `roofline --json`,
//!   `lite_metrics --judge` (writes `lite_verdict`), `render_report`·`publish_benchmark_record`·`render_max_report`
//!   (stdout = the written path, or the document with `--stdout`), `repeat_axis record-run|aggregate-level`,
//!   `assemble_sweep_meta` (env in, `sweep_index.json` out), `classify_cell --sweep-index --write-bench-mode`,
//!   `dataset_prep synthetic-sidecar`, `resolve_topology --repo` (stdout `single|multi`, exit 3 = no manifest),
//!   `resolve_bench_tool --json` (10 keys, `status` resolved = 0 / absent = 3);
//! - policy-selftest `hint_suite.rs`·`suite.rs`, hint-publisher `render_bench_section.rs`·`e1.rs`·`e4.rs`, policy-gate
//!   `doc_naming.rs`·`completion_gate.rs`, policy-publisher `classify.rs`·`publisher.rs`, recipe `escalation_bridge.rs`:
//!   the lite and full report sections, the flat certificate lines, the `evaluate-jsonl` transport of
//!   `bench_reporting_api`·`classify_cell_api`·`parse_guidellm` and the `constants` of `classify_cell_api`.
//!
//! The bins run for real through their launchers in scratch directories (never the repository: the writers default to
//! `<repo>/docs/benchmark` and the campaign writer, so every run here names `--out-dir`/`--stdout` and a scratch cwd).
//! Values (L2: keys, JSON types, values) and exit codes, option and operation names, `\x1f` lines, markers and section
//! heads (L1) are fixed; message texts, help layout and key order are not (L3).
use std::ffi::OsStr;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use json::Json;

/// `.claude/skills/adversarial-benchmark` (this crate is its `rust/`).
fn skill() -> PathBuf {
    std::fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("..")).unwrap()
}

fn scripts(name: &str) -> PathBuf {
    skill().join("scripts").join(name)
}

fn fixture(name: &str) -> PathBuf {
    skill().join("fixtures").join(name)
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
    let p = std::env::temp_dir().join(format!("ab-contract-{tag}-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&p).unwrap();
    Scratch(std::fs::canonicalize(&p).unwrap())
}

fn text(b: &[u8]) -> String {
    String::from_utf8(b.to_vec()).unwrap()
}

fn write(path: &Path, body: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

fn ps(p: &Path) -> &str {
    p.to_str().unwrap()
}

/// The launchers (`scripts/<file>`), the override variable the consumers may set and this build's executable.
const LAUNCHERS: [(&str, &str, &str); 38] = [
    (
        "assemble_sweep_meta.sh",
        "ASSEMBLE_SWEEP_META_BIN",
        env!("CARGO_BIN_EXE_assemble_sweep_meta"),
    ),
    (
        "bench_doc_naming.sh",
        "BENCH_DOC_NAMING_BIN",
        env!("CARGO_BIN_EXE_bench_doc_naming"),
    ),
    (
        "bench_preset.sh",
        "BENCH_PRESET_BIN",
        env!("CARGO_BIN_EXE_bench_preset"),
    ),
    (
        "bench_preset_api.sh",
        "BENCH_PRESET_API_BIN",
        env!("CARGO_BIN_EXE_bench_preset_api"),
    ),
    (
        "bench_reporting_api.sh",
        "BENCH_REPORTING_API_BIN",
        env!("CARGO_BIN_EXE_bench_reporting_api"),
    ),
    (
        "bench_shell.sh",
        "BENCH_SHELL_BIN",
        env!("CARGO_BIN_EXE_bench_shell"),
    ),
    (
        "classify_cell.sh",
        "CLASSIFY_CELL_BIN",
        env!("CARGO_BIN_EXE_classify_cell"),
    ),
    (
        "classify_cell_api.sh",
        "CLASSIFY_CELL_API_BIN",
        env!("CARGO_BIN_EXE_classify_cell_api"),
    ),
    (
        "conformance_full_superset_lite.sh",
        "CONFORMANCE_BIN",
        env!("CARGO_BIN_EXE_conformance_full_superset_lite"),
    ),
    (
        "conformance_full_superset_lite_api.sh",
        "CONFORMANCE_API_BIN",
        env!("CARGO_BIN_EXE_conformance_full_superset_lite_api"),
    ),
    (
        "dataset_prep.sh",
        "DATASET_PREP_BIN",
        env!("CARGO_BIN_EXE_dataset_prep"),
    ),
    (
        "dataset_prep_api.sh",
        "DATASET_PREP_API_BIN",
        env!("CARGO_BIN_EXE_dataset_prep_api"),
    ),
    (
        "lite_metrics.sh",
        "LITE_METRICS_BIN",
        env!("CARGO_BIN_EXE_lite_metrics"),
    ),
    (
        "lite_metrics_api.sh",
        "LITE_METRICS_API_BIN",
        env!("CARGO_BIN_EXE_lite_metrics_api"),
    ),
    (
        "parse_bench.sh",
        "PARSE_BENCH_BIN",
        env!("CARGO_BIN_EXE_parse_bench"),
    ),
    (
        "parse_guidellm.sh",
        "PARSE_GUIDELLM_BIN",
        env!("CARGO_BIN_EXE_parse_guidellm"),
    ),
    (
        "publish_benchmark_record.sh",
        "PUBLISH_BENCHMARK_RECORD_BIN",
        env!("CARGO_BIN_EXE_publish_benchmark_record"),
    ),
    (
        "render_max_report.sh",
        "RENDER_MAX_REPORT_BIN",
        env!("CARGO_BIN_EXE_render_max_report"),
    ),
    (
        "render_report.sh",
        "RENDER_REPORT_BIN",
        env!("CARGO_BIN_EXE_render_report"),
    ),
    (
        "render_sweep_map.sh",
        "RENDER_SWEEP_MAP_BIN",
        env!("CARGO_BIN_EXE_render_sweep_map"),
    ),
    (
        "repeat_axis.sh",
        "REPEAT_AXIS_BIN",
        env!("CARGO_BIN_EXE_repeat_axis"),
    ),
    (
        "repeat_axis_api.sh",
        "REPEAT_AXIS_API_BIN",
        env!("CARGO_BIN_EXE_repeat_axis_api"),
    ),
    (
        "resolve_bench_tool.sh",
        "RESOLVE_BENCH_TOOL_BIN",
        env!("CARGO_BIN_EXE_resolve_bench_tool"),
    ),
    (
        "resolve_bench_tool_api.sh",
        "RESOLVE_BENCH_TOOL_API_BIN",
        env!("CARGO_BIN_EXE_resolve_bench_tool_api"),
    ),
    (
        "resolve_guidellm.sh",
        "RESOLVE_GUIDELLM_BIN",
        env!("CARGO_BIN_EXE_resolve_guidellm"),
    ),
    (
        "resolve_guidellm_api.sh",
        "RESOLVE_GUIDELLM_API_BIN",
        env!("CARGO_BIN_EXE_resolve_guidellm_api"),
    ),
    (
        "resolve_topology.sh",
        "RESOLVE_TOPOLOGY_BIN",
        env!("CARGO_BIN_EXE_resolve_topology"),
    ),
    (
        "resolve_topology_api.sh",
        "RESOLVE_TOPOLOGY_API_BIN",
        env!("CARGO_BIN_EXE_resolve_topology_api"),
    ),
    (
        "roofline.sh",
        "ROOFLINE_BIN",
        env!("CARGO_BIN_EXE_roofline"),
    ),
    (
        "selftest_broad_search_cell_args.sh",
        "SELFTEST_BROAD_CELL_BIN",
        env!("CARGO_BIN_EXE_selftest_broad_search_cell_args"),
    ),
    (
        "selftest_broad_search_precheck.sh",
        "SELFTEST_BROAD_PRECHECK_BIN",
        env!("CARGO_BIN_EXE_selftest_broad_search_precheck"),
    ),
    (
        "selftest_judge_bench.sh",
        "SELFTEST_JUDGE_BENCH_BIN",
        env!("CARGO_BIN_EXE_selftest_judge_bench"),
    ),
    (
        "selftest_lite_gate.sh",
        "SELFTEST_LITE_GATE_BIN",
        env!("CARGO_BIN_EXE_selftest_lite_gate"),
    ),
    (
        "selftest_lite_report.sh",
        "SELFTEST_LITE_REPORT_BIN",
        env!("CARGO_BIN_EXE_selftest_lite_report"),
    ),
    (
        "selftest_sweep_meta.sh",
        "SELFTEST_SWEEP_META_BIN",
        env!("CARGO_BIN_EXE_selftest_sweep_meta"),
    ),
    (
        "selftest_sweep_repeats.sh",
        "SELFTEST_SWEEP_REPEATS_BIN",
        env!("CARGO_BIN_EXE_selftest_sweep_repeats"),
    ),
    (
        "sweep_stop.sh",
        "SWEEP_STOP_BIN",
        env!("CARGO_BIN_EXE_sweep_stop"),
    ),
    (
        "verdict_rule.sh",
        "VERDICT_RULE_BIN",
        env!("CARGO_BIN_EXE_verdict_rule"),
    ),
];

fn launcher(file: &str) -> (&'static str, &'static str) {
    LAUNCHERS
        .iter()
        .find(|(f, _, _)| *f == file)
        .map(|(_, var, exe)| (*var, *exe))
        .unwrap_or_else(|| panic!("{file} is not a launcher of this crate"))
}

/// One run of `bash scripts/<file> <args>`: the override variable names this build, `EASY_VLLM_VERSION` is removed
/// (hint_suite.rs runs the writers that way) and the cwd is a scratch directory.
struct Run<'a> {
    file: &'a str,
    args: Vec<std::ffi::OsString>,
    env: Vec<(String, std::ffi::OsString)>,
    stdin: Option<Vec<u8>>,
    cwd: &'a Path,
}

impl<'a> Run<'a> {
    fn new(cwd: &'a Path, file: &'a str, args: &[&str]) -> Self {
        Run {
            file,
            args: args.iter().map(|a| a.into()).collect(),
            env: Vec::new(),
            stdin: None,
            cwd,
        }
    }
    fn arg(mut self, a: impl AsRef<OsStr>) -> Self {
        self.args.push(a.as_ref().to_os_string());
        self
    }
    fn env(mut self, k: &str, v: impl AsRef<OsStr>) -> Self {
        self.env.push((k.to_string(), v.as_ref().to_os_string()));
        self
    }
    fn stdin(mut self, s: &str) -> Self {
        self.stdin = Some(s.as_bytes().to_vec());
        self
    }
    fn out(self) -> Output {
        let mut c = Command::new("bash");
        c.arg(scripts(self.file))
            .args(&self.args)
            .current_dir(self.cwd)
            .env_remove("EASY_VLLM_VERSION");
        if let Some((var, exe)) = LAUNCHERS
            .iter()
            .find(|(f, _, _)| *f == self.file)
            .map(|(_, v, e)| (*v, *e))
        {
            c.env(var, exe);
        }
        for (k, v) in &self.env {
            c.env(k, v);
        }
        c.stdin(if self.stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        });
        c.stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = c.spawn().unwrap();
        if let Some(input) = &self.stdin {
            child.stdin.take().unwrap().write_all(input).unwrap();
        }
        child.wait_with_output().unwrap()
    }
}

fn run(cwd: &Path, file: &str, args: &[&str]) -> Output {
    Run::new(cwd, file, args).out()
}

/// Exit 0 and the stdout, failing the check with both streams otherwise.
fn ok(out: &Output, what: &str) -> String {
    assert_eq!(
        out.status.code(),
        Some(0),
        "{what}: stdout {}\nstderr {}",
        text(&out.stdout),
        text(&out.stderr)
    );
    text(&out.stdout)
}

fn parse(doc: &str, what: &str) -> Json {
    Json::parse(doc).unwrap_or_else(|e| panic!("{what}: not one JSON document ({e}):\n{doc}"))
}

fn read_json(path: &Path) -> Json {
    parse(
        &std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())),
        ps(path),
    )
}

/// The `\x1f`-separated fields of a one-line stdout (the shells `IFS=$'\x1f' read -r …`).
fn us_fields(stdout: &str) -> Vec<String> {
    let line = stdout.strip_suffix('\n').unwrap_or(stdout);
    assert!(!line.contains('\n'), "one line expected: {stdout:?}");
    line.split('\u{1f}').map(str::to_string).collect()
}

fn assert_keys(doc: &Json, types: &[(&str, &[&str])], what: &str) {
    for (k, kinds) in types {
        let v = doc
            .get(k)
            .unwrap_or_else(|| panic!("{what}: key {k:?} missing in {doc:?}"));
        assert!(
            kinds.contains(&v.kind()),
            "{what}: {k} = {v:?} not in {kinds:?}"
        );
    }
}

/// `name` appears in `listing` as a whole option or subcommand token: the bytes around it are not part of a name, so
/// `--out` is not found inside `--output-len` and `--record` not inside `--record-path`. This reads a help listing only
/// (the documents' names); the options the consumers pass are probed by running the parser with them.
fn names_option(listing: &str, name: &str) -> bool {
    let part = |c: u8| c.is_ascii_alphanumeric() || c == b'-' || c == b'_';
    let b = listing.as_bytes();
    listing.match_indices(name).any(|(at, _)| {
        let end = at + name.len();
        (at == 0 || !part(b[at - 1])) && (end == b.len() || !part(b[end]))
    })
}

/// A usage error of the parser: exit 2 with a usage block on stderr (argparse `usage: …`, clap `Usage: …`).
fn usage_error(out: &Output) -> bool {
    out.status.code() == Some(2)
        && String::from_utf8_lossy(&out.stderr)
            .to_ascii_lowercase()
            .contains("usage")
}

/// The parser takes the option `args[at]`: this command line is not a usage error, while the same line with that option
/// renamed to one no parser has is one. A domain answer to the arguments (exit 2 or another code without a usage block)
/// passes; the control keeps the probe from passing on a bin that never reports usage errors this way.
fn assert_parser_takes(cwd: &Path, file: &str, args: &[&str], at: usize, env: &[(&str, &Path)]) {
    let go = |args: &[&str]| {
        let mut r = Run::new(cwd, file, args);
        for (k, v) in env {
            r = r.env(k, v);
        }
        r.out()
    };
    let out = go(args);
    assert!(
        !usage_error(&out),
        "{file} {args:?}: {} is not taken\n{}",
        args[at],
        String::from_utf8_lossy(&out.stderr)
    );
    let mut control = args.to_vec();
    control[at] = "--contract-no-such-option";
    let out = go(&control);
    assert!(
        usage_error(&out),
        "{file} {control:?}: an unknown option must be a usage error: {:?}\n{}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
}

// ───────────────────────────────────────────── launchers

/// Every launcher exists; an absent `<NAME>_BIN` executable is exit 2 with nothing on stdout (the consumers read an
/// empty stdout as "no answer"); a present one is started with the arguments as given and its exit code and stdout
/// pass through unchanged.
#[test]
fn launchers_override_and_absent_executable() {
    let s = scratch("launchers");
    let stub = s.0.join("stub");
    write(&stub, "#!/bin/sh\nprintf 'ARGS'; for a in \"$@\"; do printf '|%s' \"$a\"; done; printf '\\n'\nexit 7\n");
    std::fs::set_permissions(&stub, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    for (file, var, _) in LAUNCHERS {
        assert!(scripts(file).is_file(), "{file} missing");
        let out = Command::new("bash")
            .arg(scripts(file))
            .arg("--help")
            .env(var, s.0.join("absent"))
            .current_dir(&s.0)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(2), "{file}: absent {var}");
        assert_eq!(
            text(&out.stdout),
            "",
            "{file}: absent {var} prints nothing on stdout"
        );
        let out = Command::new("bash")
            .arg(scripts(file))
            .args(["a b", "--x"])
            .env(var, &stub)
            .current_dir(&s.0)
            .output()
            .unwrap();
        assert_eq!(
            out.status.code(),
            Some(7),
            "{file}: {var} exit code passes through"
        );
        assert_eq!(
            text(&out.stdout),
            "ARGS|a b|--x\n",
            "{file}: {var} receives the arguments"
        );
    }
}

/// The variables the launchers export to the bin (the bins and their self-tests read them): set when the caller left
/// them unset, kept when the caller set them.
#[test]
fn launcher_exported_environment() {
    let s = scratch("exports");
    let stub = s.0.join("env-stub");
    write(&stub, "#!/bin/sh\nenv\n");
    std::fs::set_permissions(&stub, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let rows: [(&str, &[&str]); 19] = [
        (
            "bench_preset.sh",
            &["BENCH_PRESET_REPO_ROOT", "BENCH_PRESET_SKILL_DIR"],
        ),
        (
            "bench_preset_api.sh",
            &["BENCH_PRESET_REPO_ROOT", "BENCH_PRESET_SKILL_DIR"],
        ),
        ("classify_cell.sh", &["CLASSIFY_CELL_SCRIPTS_DIR"]),
        ("classify_cell_api.sh", &["CLASSIFY_CELL_SCRIPTS_DIR"]),
        (
            "conformance_full_superset_lite.sh",
            &["CONFORMANCE_SCRIPTS_DIR"],
        ),
        (
            "conformance_full_superset_lite_api.sh",
            &["CONFORMANCE_SCRIPTS_DIR"],
        ),
        ("parse_guidellm.sh", &["PARSE_GUIDELLM_FIXTURE"]),
        ("repeat_axis.sh", &["REPEAT_AXIS_REPO_ROOT"]),
        ("repeat_axis_api.sh", &["REPEAT_AXIS_REPO_ROOT"]),
        ("resolve_bench_tool.sh", &["BENCH_TOOL_PIN_PATH"]),
        ("resolve_bench_tool_api.sh", &["BENCH_TOOL_PIN_PATH"]),
        ("resolve_guidellm.sh", &["GUIDELLM_RECORD_PATH"]),
        ("resolve_guidellm_api.sh", &["GUIDELLM_RECORD_PATH"]),
        (
            "resolve_topology.sh",
            &[
                "RESOLVE_TOPOLOGY_REPO_ROOT",
                "RESOLVE_TOPOLOGY_SCRIPTS_DIR",
                "RESOLVE_TOPOLOGY_RUNTIME_DIR",
            ],
        ),
        (
            "resolve_topology_api.sh",
            &[
                "RESOLVE_TOPOLOGY_REPO_ROOT",
                "RESOLVE_TOPOLOGY_SCRIPTS_DIR",
                "RESOLVE_TOPOLOGY_RUNTIME_DIR",
            ],
        ),
        (
            "selftest_broad_search_cell_args.sh",
            &["BENCH_VERIFY_REPO_ROOT"],
        ),
        (
            "selftest_broad_search_precheck.sh",
            &["BENCH_VERIFY_REPO_ROOT"],
        ),
        ("selftest_lite_gate.sh", &["BENCH_VERIFY_REPO_ROOT"]),
        ("selftest_sweep_repeats.sh", &["BENCH_VERIFY_REPO_ROOT"]),
    ];
    let env_of = |out: &Output| -> Vec<(String, String)> {
        text(&out.stdout)
            .lines()
            .filter_map(|l| l.split_once('='))
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    };
    for (file, names) in rows {
        let (var, _) = launcher(file);
        let mut c = Command::new("bash");
        c.arg(scripts(file)).env(var, &stub).current_dir(&s.0);
        for n in names {
            c.env_remove(n);
        }
        let got = env_of(&c.output().unwrap());
        for n in names {
            let v = got
                .iter()
                .find(|(k, _)| k == n)
                .map(|(_, v)| v.as_str())
                .unwrap_or_else(|| panic!("{file}: {n} not exported"));
            assert!(!v.is_empty(), "{file}: {n} is empty");
        }
        let mut c = Command::new("bash");
        c.arg(scripts(file)).env(var, &stub).current_dir(&s.0);
        for n in names {
            c.env(n, "/caller/value");
        }
        let got = env_of(&c.output().unwrap());
        for n in names {
            assert!(
                got.iter().any(|(k, v)| k == n && v == "/caller/value"),
                "{file}: the caller's {n} is kept"
            );
        }
    }
    // the other selftest launchers export the same root
    for file in [
        "selftest_judge_bench.sh",
        "selftest_lite_report.sh",
        "selftest_sweep_meta.sh",
    ] {
        let (var, _) = launcher(file);
        let got = env_of(
            &Command::new("bash")
                .arg(scripts(file))
                .env(var, &stub)
                .env_remove("BENCH_VERIFY_REPO_ROOT")
                .current_dir(&s.0)
                .output()
                .unwrap(),
        );
        assert!(
            got.iter()
                .any(|(k, v)| k == "BENCH_VERIFY_REPO_ROOT" && !v.is_empty()),
            "{file}: BENCH_VERIFY_REPO_ROOT"
        );
    }
}

// ───────────────────────────────────────────── consumer-run self-tests (exit 0)

fn self_test(file: &str, args: &[&str]) {
    let s = scratch("selftest");
    let out = run(&s.0, file, args);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{file} {args:?}: {}{}",
        text(&out.stdout),
        text(&out.stderr)
    );
}

#[test]
fn self_test_bench_doc_naming() {
    self_test("bench_doc_naming.sh", &[]);
}
#[test]
fn self_test_bench_preset() {
    self_test("bench_preset.sh", &["--self-test"]);
    // selftest_bench_preset.sh execs the same self-test with the repository and skill roots exported
    let s = scratch("selftest");
    let out = Command::new("bash")
        .arg(scripts("selftest_bench_preset.sh"))
        .current_dir(&s.0)
        .env("BENCH_PRESET_BIN", env!("CARGO_BIN_EXE_bench_preset"))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
}
#[test]
fn self_test_bench_shell() {
    self_test("bench_shell.sh", &["--self-test"]);
}
#[test]
fn self_test_classify_cell() {
    self_test("classify_cell.sh", &["--self-test"]);
}
#[test]
fn self_test_conformance_full_superset_lite() {
    self_test("conformance_full_superset_lite.sh", &[]);
}
#[test]
fn self_test_dataset_prep() {
    self_test("dataset_prep.sh", &["--self-test"]);
}
#[test]
fn self_test_parse_bench_subcommand() {
    // `self-test` is a subcommand of parse_bench, not a flag
    self_test("parse_bench.sh", &["self-test"]);
}
#[test]
fn self_test_parse_guidellm() {
    self_test("parse_guidellm.sh", &["--self-test"]);
}
#[test]
fn self_test_render_sweep_map() {
    self_test("render_sweep_map.sh", &["--self-test"]);
}
#[test]
fn self_test_repeat_axis() {
    self_test("repeat_axis.sh", &["--self-test"]);
}
#[test]
fn self_test_resolve_bench_tool() {
    self_test("resolve_bench_tool.sh", &["--self-test"]);
}
#[test]
fn self_test_resolve_guidellm() {
    self_test("resolve_guidellm.sh", &["--self-test"]);
}
#[test]
fn self_test_resolve_topology() {
    self_test("resolve_topology.sh", &["--self-test"]);
}
#[test]
fn self_test_roofline_subcommand() {
    // `self-test` is a subcommand of roofline, not a flag
    self_test("roofline.sh", &["self-test"]);
}
#[test]
fn self_test_sweep_stop() {
    self_test("sweep_stop.sh", &["--self-test"]);
}
#[test]
fn self_test_verdict_rule() {
    self_test("verdict_rule.sh", &["--self-test"]);
}
#[test]
fn self_test_selftest_broad_search_cell_args() {
    self_test("selftest_broad_search_cell_args.sh", &[]);
}
#[test]
fn self_test_selftest_broad_search_precheck() {
    self_test("selftest_broad_search_precheck.sh", &[]);
}
#[test]
fn self_test_selftest_lite_gate() {
    self_test("selftest_lite_gate.sh", &[]);
}
#[test]
fn self_test_selftest_lite_report() {
    self_test("selftest_lite_report.sh", &[]);
}
#[test]
fn self_test_selftest_sweep_meta() {
    self_test("selftest_sweep_meta.sh", &[]);
}
#[test]
fn self_test_selftest_sweep_repeats() {
    self_test("selftest_sweep_repeats.sh", &[]);
}
/// judge_bench.sh `--self-test` execs `selftest_judge_bench.sh`; its exit code and output pass through.
#[test]
fn self_test_selftest_judge_bench_through_judge_bench() {
    let s = scratch("selftest");
    let out = Command::new("bash")
        .arg(scripts("judge_bench.sh"))
        .arg("--self-test")
        .env(
            "SELFTEST_JUDGE_BENCH_BIN",
            env!("CARGO_BIN_EXE_selftest_judge_bench"),
        )
        .current_dir(&s.0)
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}{}",
        text(&out.stdout),
        text(&out.stderr)
    );
    self_test("selftest_judge_bench.sh", &[]);
}

// ───────────────────────────────────────────── broad_search.sh: the sweep state chain

/// A sweep state in the shape `bench_shell broad-init` writes, with `cells` and `cells_remaining` as given.
fn state_json(cells: &str, remaining: &str) -> String {
    format!(
        "{{\"schema_version\": 1, \"sweep_id\": \"s1\", \"topology\": \"single\", \"control_variable\": \"a\", \
         \"rubric_authority\": \"explore\", \"started_utc\": \"2026-09-04T09:00:00Z\", \"declared_budget\": {{\"max_cells\": 6, \
         \"wall_clock_budget_s\": 3600, \"consecutive_failure_limit\": 3, \"repeats\": 3, \"repeats_source\": \"declared(x)\", \
         \"declared_by\": \"tester\", \"basis\": \"fixture\"}}, \"cells_planned\": 6, \"cells_remaining\": {remaining}, \"cells\": {cells}}}"
    )
}

/// broad_search.sh init: `bench_preset default --knob repeats` prints `VALUE\x1fNAME`; `repeat_axis resolve
/// --preset-default --preset-source` prints `N\x1fSRC\x1fKIND`; `bench_shell broad-init <state>` (env SWEEP_ID, CELLS,
/// CTRL, AUTHORITY, MAX_CELLS, WALL, FAILLIMIT, DECLARED_BY, BASIS, NOW, TOPO, REP_N, REP_SRC) writes the state.
#[test]
fn broad_search_init_chain() {
    let s = scratch("init");
    let repo = s.0.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let pd = ok(
        &Run::new(&s.0, "bench_preset.sh", &["default", "--knob", "repeats"])
            .env("BENCH_PRESET_REPO_ROOT", &repo)
            .out(),
        "bench_preset default",
    );
    let f = us_fields(&pd);
    assert_eq!(f.len(), 2, "{pd:?}");
    assert!(f[0].parse::<u32>().is_ok(), "VALUE is an integer: {pd:?}");
    assert!(!f[1].is_empty(), "NAME: {pd:?}");
    let rr = ok(
        &Run::new(
            &s.0,
            "repeat_axis.sh",
            &[
                "resolve",
                "--preset-default",
                &f[0],
                "--preset-source",
                &f[1],
            ],
        )
        .env("REPEAT_AXIS_REPO_ROOT", &repo)
        .out(),
        "repeat_axis resolve",
    );
    let r = us_fields(&rr);
    assert_eq!(r.len(), 3, "{rr:?}");
    assert_eq!(
        r[0], f[0],
        "the preset default is the repeat count when nothing else is declared"
    );
    assert!(!r[1].is_empty());
    assert!(
        ["warm-rerun", "cold-restart"].contains(&r[2].as_str()),
        "{rr:?}"
    );
    // an explicit --repeats wins and is a declaration
    let rr = ok(
        &Run::new(
            &s.0,
            "repeat_axis.sh",
            &[
                "resolve",
                "--repeats",
                "5",
                "--preset-default",
                &f[0],
                "--preset-source",
                &f[1],
            ],
        )
        .env("REPEAT_AXIS_REPO_ROOT", &repo)
        .out(),
        "repeat_axis resolve --repeats",
    );
    assert_eq!(us_fields(&rr)[0], "5");
    let state = s.0.join("bs/state.json");
    std::fs::create_dir_all(state.parent().unwrap()).unwrap();
    let mut b = Run::new(&s.0, "bench_shell.sh", &["broad-init", ps(&state)]);
    for (k, v) in [
        ("SWEEP_ID", "s1"),
        ("CELLS", "a b"),
        ("CTRL", "a"),
        ("AUTHORITY", "explore"),
        ("MAX_CELLS", "4"),
        ("WALL", "3600"),
        ("FAILLIMIT", "3"),
        ("DECLARED_BY", "tester"),
        ("BASIS", "fixture"),
        ("NOW", "2026-09-04T09:00:00Z"),
        ("TOPO", "single"),
        ("REP_N", &r[0]),
        ("REP_SRC", &r[1]),
    ] {
        b = b.env(k, v);
    }
    ok(&b.out(), "bench_shell broad-init");
    let doc = read_json(&state);
    assert_keys(
        &doc,
        &[
            ("sweep_id", &["string"]),
            ("topology", &["string"]),
            ("rubric_authority", &["string"]),
            ("started_utc", &["string"]),
            ("declared_budget", &["object"]),
            ("cells_planned", &["number"]),
            ("cells_remaining", &["array"]),
            ("cells", &["array"]),
        ],
        "broad-init state",
    );
    assert_eq!(doc.at("rubric_authority").str(), Some("explore"));
    assert_eq!(
        doc.at("declared_budget").at("repeats"),
        &Json::Num(r[0].clone())
    );
    assert_eq!(
        doc.at("declared_budget").at("repeats_source").str(),
        Some(r[1].as_str())
    );
    // the state it wrote is what sweep_stop, its report and verdict_rule authority-check read
    let dr = ok(
        &run(
            &s.0,
            "sweep_stop.sh",
            &["report", "declared-repeats", "--state", ps(&state)],
        ),
        "declared-repeats",
    );
    assert_eq!(us_fields(&dr), [r[0].clone(), r[1].clone()]);
    let au = ok(
        &run(
            &s.0,
            "verdict_rule.sh",
            &["authority-check", "--state", ps(&state)],
        ),
        "authority-check",
    );
    assert_eq!(au, "explore\n");
}

/// broad_search.sh `_stop`: `sweep_stop --state S --now-utc T > S.stop.json` is exit 0 (continue) or 3 (stop) with the
/// stop JSON on stdout (other codes = cannot judge); `report stopped-by --stop-json`, `report status-line --stop-json`
/// and `report declared-repeats --state` (`REPEATS\x1fSOURCE`) are exit 0; `render_sweep_map --state --stop-json
/// --out-md [--out-json]` writes the map.
#[test]
fn sweep_stop_continue_stop_reports_and_map() {
    let s = scratch("stop");
    let state = s.0.join("state.json");
    write(
        &state,
        &state_json(
            "[{\"cell_key\": \"a\", \"cell_outcome\": \"measured\"}]",
            "[\"b\"]",
        ),
    );
    let out = run(
        &s.0,
        "sweep_stop.sh",
        &["--state", ps(&state), "--now-utc", "2026-09-04T09:10:00Z"],
    );
    let doc = parse(&ok(&out, "sweep_stop continue"), "stop json");
    assert_keys(
        &doc,
        &[
            ("stop", &["bool"]),
            ("stopped_by", &["array"]),
            ("sweep_status", &["string"]),
            ("cells_planned", &["number"]),
            ("cells_attempted", &["number"]),
            ("cells_remaining", &["array"]),
            ("consecutive_failures", &["number"]),
            ("elapsed_s", &["number"]),
            ("declared_budget", &["object"]),
        ],
        "stop json",
    );
    assert_eq!(doc.at("stop"), &Json::Bool(false));
    let stop_json = s.0.join("state.stop.json");
    std::fs::write(&stop_json, &out.stdout).unwrap();
    // six failed cells: every stop reason at once, exit 3
    let cells: Vec<String> = (0..6)
        .map(|i| format!("{{\"cell_key\": \"c{i}\", \"cell_outcome\": \"serve_failed\"}}"))
        .collect();
    let stopped = s.0.join("stopped.json");
    write(
        &stopped,
        &state_json(&format!("[{}]", cells.join(",")), "[]"),
    );
    let out = run(
        &s.0,
        "sweep_stop.sh",
        &["--state", ps(&stopped), "--now-utc", "2026-09-04T10:00:00Z"],
    );
    assert_eq!(
        out.status.code(),
        Some(3),
        "stop is exit 3: {}",
        text(&out.stderr)
    );
    let doc = parse(&text(&out.stdout), "stop json (stopped)");
    assert_eq!(doc.at("stop"), &Json::Bool(true));
    assert_eq!(
        doc.at("stopped_by").strs(),
        [
            "cells_exhausted",
            "cell_budget_exhausted",
            "wall_clock_exhausted",
            "consecutive_failures"
        ]
    );
    let stopped_json = s.0.join("stopped.stop.json");
    std::fs::write(&stopped_json, &out.stdout).unwrap();
    // an unreadable state cannot be judged: neither 0 nor 3
    write(&s.0.join("bad.json"), "{");
    let out = run(
        &s.0,
        "sweep_stop.sh",
        &[
            "--state",
            ps(&s.0.join("bad.json")),
            "--now-utc",
            "2026-09-04T10:00:00Z",
        ],
    );
    assert!(
        !matches!(out.status.code(), Some(0) | Some(3)),
        "unreadable state: {:?}",
        out.status
    );
    // the state read from stdin (`--state -`) is the same evaluation
    let out = Run::new(
        &s.0,
        "sweep_stop.sh",
        &["--state", "-", "--now-utc", "2026-09-04T09:10:00Z"],
    )
    .stdin(&std::fs::read_to_string(&state).unwrap())
    .out();
    assert_eq!(out.status.code(), Some(0));
    for args in [
        ["report", "stopped-by", "--stop-json", ps(&stopped_json)],
        ["report", "status-line", "--stop-json", ps(&stopped_json)],
    ] {
        let out = run(&s.0, "sweep_stop.sh", &args);
        assert!(!ok(&out, "report").is_empty(), "{args:?}");
    }
    let dr = ok(
        &run(
            &s.0,
            "sweep_stop.sh",
            &["report", "declared-repeats", "--state", ps(&state)],
        ),
        "declared-repeats",
    );
    assert_eq!(us_fields(&dr), ["3", "declared(x)"]);
    // a state from before the repeat axis: an empty REPEATS field (broad_search refuses a new run on it)
    write(
        &s.0.join("legacy.json"),
        "{\"declared_budget\": {\"max_cells\": 1}}",
    );
    let dr = ok(
        &run(
            &s.0,
            "sweep_stop.sh",
            &[
                "report",
                "declared-repeats",
                "--state",
                ps(&s.0.join("legacy.json")),
            ],
        ),
        "legacy",
    );
    assert_eq!(us_fields(&dr)[0], "");
    assert_eq!(
        run(&s.0, "sweep_stop.sh", &["report", "nonsense"])
            .status
            .code(),
        Some(2)
    );
    // render_sweep_map
    let md = s.0.join("map/map.md");
    let js = s.0.join("map/map.json");
    std::fs::create_dir_all(md.parent().unwrap()).unwrap();
    ok(
        &run(
            &s.0,
            "render_sweep_map.sh",
            &[
                "--state",
                ps(&state),
                "--stop-json",
                ps(&stop_json),
                "--out-md",
                ps(&md),
                "--out-json",
                ps(&js),
            ],
        ),
        "render_sweep_map",
    );
    assert!(md.is_file());
    let map = read_json(&js);
    assert_eq!(map.at("sweep_id").str(), Some("s1"));
    assert_keys(
        &map,
        &[
            ("sweep_status", &["string"]),
            ("stopped_by", &["array"]),
            ("cells_planned", &["number"]),
        ],
        "map json",
    );
    let md2 = s.0.join("map/only.md");
    ok(
        &run(
            &s.0,
            "render_sweep_map.sh",
            &[
                "--state",
                ps(&state),
                "--stop-json",
                ps(&stop_json),
                "--out-md",
                ps(&md2),
            ],
        ),
        "map without json",
    );
    assert!(md2.is_file());
}

/// verdict_rule `authority-check --state` prints the state's rubric authority (judge_bench `--authority`), and is
/// non-zero on a state that holds none of `weak|explicit|explore`.
#[test]
fn verdict_rule_authority_check() {
    let s = scratch("authority");
    for a in ["weak", "explicit", "explore"] {
        let st = s.0.join(format!("{a}.json"));
        write(
            &st,
            &state_json("[]", "[\"a\"]").replace(
                "\"rubric_authority\": \"explore\"",
                &format!("\"rubric_authority\": \"{a}\""),
            ),
        );
        assert_eq!(
            ok(
                &run(
                    &s.0,
                    "verdict_rule.sh",
                    &["authority-check", "--state", ps(&st)]
                ),
                a
            ),
            format!("{a}\n")
        );
    }
    let st = s.0.join("bogus.json");
    write(
        &st,
        &state_json("[]", "[\"a\"]").replace(
            "\"rubric_authority\": \"explore\"",
            "\"rubric_authority\": \"bogus\"",
        ),
    );
    let out = run(
        &s.0,
        "verdict_rule.sh",
        &["authority-check", "--state", ps(&st)],
    );
    assert_ne!(out.status.code(), Some(0));
}

// ───────────────────────────────────────────── bench_shell positional operations

/// The `bench_shell` operations the bench scripts read as text: one line on stdout, exit 0; `measurement-ok` and
/// `broad-measurement-ok` answer with the exit code; an unknown or missing operation is exit 2 with nothing on stdout.
#[test]
fn bench_shell_operations_lines_and_exit_codes() {
    let s = scratch("bench-shell");
    let d = &s.0;
    let bs = |args: &[&str]| run(d, "bench_shell.sh", args);
    let bs_in = |args: &[&str], input: &str| Run::new(d, "bench_shell.sh", args).stdin(input).out();
    // run_bench.sh
    assert_eq!(
        ok(&bs(&["warm-fraction", "1", "3"]), "warm-fraction"),
        "0.3333\n"
    );
    let out = bs(&["warm-fraction", "x"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(text(&out.stdout), "");
    assert_eq!(
        ok(&bs_in(&["floor-mib"], "{\"floor_mib\": 1234}"), "floor-mib"),
        "1234\n"
    );
    assert_eq!(
        ok(
            &bs_in(&["image-ref"], "{\"image_ref\": \"ghcr.io/x/y:v1\"}"),
            "image-ref"
        ),
        "ghcr.io/x/y:v1\n"
    );
    let dk = d.join("ds.json");
    write(&dk, "{\"dataset_kind\": \"synthetic\"}");
    assert_eq!(
        ok(&bs(&["dataset-kind", ps(&dk)]), "dataset-kind"),
        "synthetic\n"
    );
    let da = parse(
        &ok(
            &bs(&["data-arg", "json_file", "/out/dataset_rows.jsonl"]),
            "data-arg",
        ),
        "data-arg",
    );
    assert_eq!(da.at("path").str(), Some("/out/dataset_rows.jsonl"));
    let side = d.join("bench_tool_cfg.json");
    let out = Run::new(d, "bench_shell.sh", &["tool-sidecar"])
        .stdin("{\"tool\": \"guidellm\", \"image_ref\": \"ghcr.io/x/y:v1\"}")
        .env("GL_ENDPOINT", "/v1/completions")
        .env("GL_BACKEND_SOURCE", "declared(x)")
        .env("GL_OUT", &side)
        .out();
    ok(&out, "tool-sidecar");
    let sc = read_json(&side);
    assert_eq!(sc.at("endpoint").str(), Some("/v1/completions"));
    assert_eq!(sc.at("backend_source").str(), Some("declared(x)"));
    assert_eq!(sc.at("tool").str(), Some("guidellm"));
    // lite_bench.sh: json-escape-stdin is one JSON string literal of the input
    let raw = "tail \"quoted\"\nline\twith é\n";
    let lit = ok(&bs_in(&["json-escape-stdin"], raw), "json-escape-stdin");
    assert_eq!(
        parse(lit.trim_end_matches('\n'), "escaped"),
        Json::Str(raw.to_string())
    );
    let lv = d.join("lite_raw.json");
    for (stored, word, vocab) in [
        ("pass", "pass", "pass"),
        ("server_failed", "server_failed", "server_failed"),
        (
            "measurement_path_failed",
            "measurement_path_failed",
            "measurement_path_failed",
        ),
        ("bogus", "bogus", ""),
    ] {
        write(&lv, &format!("{{\"lite_verdict\": \"{stored}\"}}"));
        assert_eq!(
            ok(&bs(&["lite-verdict", ps(&lv)]), "lite-verdict"),
            format!("{word}\n")
        );
        // sweep_bench.sh read-lite-verdict: only the vocabulary, else an empty line
        assert_eq!(
            ok(&bs(&["read-lite-verdict", ps(&lv)]), "read-lite-verdict"),
            format!("{vocab}\n")
        );
    }
    assert_eq!(
        ok(
            &bs(&["read-lite-verdict", ps(&d.join("absent.json"))]),
            "read-lite-verdict absent"
        ),
        "\n"
    );
    // judge_bench.sh
    let idx = d.join("idx.json");
    write(
        &idx,
        "{\"meta\": {\"model_path\": \"/app/models/Org/M\", \"tensor_parallel_size\": \"2\"}}",
    );
    assert_eq!(
        ok(&bs(&["judge-model-tp", ps(&idx)]), "judge-model-tp"),
        "/app/models/Org/M 2\n"
    );
    write(&idx, "{\"meta\": {}}");
    assert_eq!(
        ok(&bs(&["judge-model-tp", ps(&idx)]), "judge-model-tp absent"),
        "NA -\n"
    );
    let vj = d.join("verdict.json");
    write(&vj, "{\"verdict\": \"PASS\", \"reason_code\": null, \"rubric\": {\"authority\": \"explicit\", \"source\": \"c(user_target)\", \"floor\": 10.0}}");
    assert_eq!(ok(&bs(&["verdict", ps(&vj)]), "verdict"), "PASS\n");
    assert_eq!(ok(&bs(&["authority", ps(&vj)]), "authority"), "explicit\n");
    assert!(ok(&bs(&["judge-log", ps(&vj)]), "judge-log").starts_with("[judge_bench] "));
    // sweep_bench.sh·broad_search.sh: measurement-ok answers with the exit code
    let m = d.join("measured.json");
    for (body, rc) in [
        ("{\"measurement_ok\": true}", 0),
        ("{\"measurement_ok\": false}", 1),
    ] {
        write(&m, body);
        for op in ["measurement-ok", "broad-measurement-ok"] {
            let out = bs(&[op, ps(&m)]);
            assert_eq!(out.status.code(), Some(rc), "{op} {body}");
            assert_eq!(text(&out.stdout), "");
        }
    }
    write(
        &idx,
        "{\"levels\": [{\"level\": 1}, {\"level\": 4}, {\"level\": \"x\"}]}",
    );
    assert_eq!(
        ok(&bs(&["prior-levels", ps(&idx)]), "prior-levels"),
        "1 4\n"
    );
    // broad_search.sh: the cell classification on stdin
    let cls = "{\"void_reason\": \"blackbox_kill\", \"void_reason_source\": \"events(main)\", \"lite_raw_status\": \"present\", \"events_scanned\": [\"a.jsonl\", \"b.jsonl\"]}";
    assert_eq!(
        ok(&bs_in(&["event-reason"], cls), "event-reason"),
        "blackbox_kill\n"
    );
    assert_eq!(
        ok(&bs_in(&["lite-raw-status"], cls), "lite-raw-status"),
        "present\n"
    );
    assert_eq!(
        ok(&bs_in(&["void-reason-source"], cls), "void-reason-source"),
        "events(main)\n"
    );
    assert_eq!(
        ok(&bs_in(&["events-scanned"], cls), "events-scanned"),
        "a.jsonl\nb.jsonl\n"
    );
    let other = "{\"void_reason\": \"x\", \"void_reason_source\": \"declared(y)\"}";
    assert_eq!(
        ok(
            &bs_in(&["event-reason"], other),
            "event-reason other source"
        ),
        "\n"
    );
    let nid = d.join("node.json");
    write(&nid, "{\"node_id\": \"sub1\"}");
    assert_eq!(ok(&bs(&["node-id", ps(&nid)]), "node-id"), "sub1\n");
    let st = d.join("state.json");
    write(&st, "{\"cells\": [{\"cell_key\": \"k\", \"cell_outcome\": \"serve_failed\"}, {\"cell_key\": \"k\", \"cell_outcome\": \"measured\"}]}");
    assert_eq!(
        ok(&bs(&["cell-outcome", ps(&st), "k"]), "cell-outcome"),
        "measured\n"
    );
    assert_eq!(
        ok(&bs(&["cell-outcome", ps(&st), "z"]), "cell-outcome none"),
        "\n"
    );
    assert_eq!(
        ok(
            &bs_in(&["provenance-status"], "{\"status\": \"verified\"}"),
            "provenance-status"
        ),
        "verified\n"
    );
    // max_envelope.sh: max-level prints the level record (redirected to result.json)
    let ml = Run::new(d, "bench_shell.sh", &["max-level"])
        .env("L", "4096")
        .env("ML", "4096")
        .env("SE", "0")
        .env("ST", "safe")
        .out();
    let ml = parse(&ok(&ml, "max-level"), "max-level");
    assert_eq!(ml.at("status").str(), Some("safe"));
    assert_eq!(ml.at("max_model_len"), &Json::Num("4096".into()));
    // unknown or missing operation
    for args in [&["frob"][..], &[]] {
        let out = bs(args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert_eq!(text(&out.stdout), "", "{args:?}");
    }
}

/// max_envelope.sh: `L ML SE ST bench_shell max-level > $MAXDIR/level_<L>/result.json` per level, then `bench_shell
/// max-index` (env CONFIG, TOPO, CFGYAML, EF, MANIFEST, MAXDIR, SAFE_CEIL, SAFE_LEVELS, ORIG_LEN, SDIR) writes
/// `$MAXDIR/max_index.json`, the input `render_max_report --max-index` reads.
#[test]
fn max_envelope_index_for_the_max_report() {
    let s = scratch("max-index");
    let max_dir = s.0.join("max");
    let cfg = s.0.join("cfg.yaml");
    write(
        &cfg,
        "model: /app/models/acme/Acme-7B\ntensor-parallel-size: 1\nmax-model-len: 8192\n",
    );
    let manifest = s.0.join("manifest.yaml");
    write(&manifest, "gpu_model: \"NVIDIA GB10\"\n");
    write(&s.0.join(".env.x"), "CONFIG_FILE=x\n");
    for (level, status) in [("4096", "safe"), ("8192", "unsafe")] {
        let out = Run::new(&s.0, "bench_shell.sh", &["max-level"])
            .env("L", level)
            .env("ML", level)
            .env("SE", if status == "safe" { "0" } else { "1" })
            .env("ST", status)
            .out();
        write(
            &max_dir.join(format!("level_{level}/result.json")),
            &ok(&out, "max-level"),
        );
    }
    let mut r = Run::new(&s.0, "bench_shell.sh", &["max-index"]);
    for (k, v) in [
        ("CFGYAML", cfg.clone()),
        ("EF", s.0.join(".env.x")),
        ("MANIFEST", manifest.clone()),
        ("MAXDIR", max_dir.clone()),
        ("SDIR", skill().join("scripts")),
    ] {
        r = r.env(k, v);
    }
    for (k, v) in [
        ("CONFIG", "x"),
        ("TOPO", "single"),
        ("SAFE_CEIL", "4096"),
        ("SAFE_LEVELS", "4096"),
        ("ORIG_LEN", "8192"),
    ] {
        r = r.env(k, v);
    }
    ok(&r.out(), "max-index");
    let index = max_dir.join("max_index.json");
    let doc = read_json(&index);
    assert_eq!(doc.kind(), "object");
    assert_eq!(doc.at("meta").at("topology").str(), Some("single"));
    assert_eq!(doc.at("meta").at("gpu_model").str(), Some("NVIDIA GB10"));
    assert!(!ok(
        &run(
            &s.0,
            "render_max_report.sh",
            &["--max-index", ps(&index), "--stdout"]
        ),
        "render_max_report"
    )
    .is_empty());
}

// ───────────────────────────────────────────── sweep_bench.sh: knob resolution, measurement, assembly

/// sweep_bench.sh: `bench_preset resolve --tool --serving-yaml --record-out [--backend --bench-budget-mib --knob-reason
/// …]` prints `KEY\x1fVALUE` lines (LEVELS ILEN OLEN NPROMPTS WARMUPS REPEATS REPEATS_SOURCE REPEAT_KIND BACKEND
/// BENCH_BUDGET_MIB PRESET_PATH) and writes the knob record; `bench_shell extract-dataset <record> <spec>` and
/// `dataset-kind <spec>` read it; a refusal is non-zero.
#[test]
fn bench_preset_resolve_lines_and_record() {
    let s = scratch("preset");
    let repo = s.0.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let serving = s.0.join("serve.yaml");
    write(
        &serving,
        "model: /app/models/x\ntensor-parallel-size: 1\nmax-model-len: 4096\n",
    );
    let record = s.0.join("knobs.json");
    let args = [
        "resolve",
        "--tool",
        "guidellm",
        "--serving-yaml",
        ps(&serving),
        "--record-out",
        ps(&record),
        "--backend",
        "openai",
        "--knob-reason",
        "backend=prior_cell_shape",
        "--bench-budget-mib",
        "4096",
        "--knob-reason",
        "bench_budget_mib=host_budget",
    ];
    let out = ok(
        &Run::new(&s.0, "bench_preset.sh", &args)
            .env("BENCH_PRESET_REPO_ROOT", &repo)
            .out(),
        "bench_preset resolve",
    );
    let mut keys = Vec::new();
    for line in out.lines() {
        let (k, v) = line
            .split_once('\u{1f}')
            .unwrap_or_else(|| panic!("not KEY\\x1fVALUE: {line:?}"));
        keys.push(k.to_string());
        match k {
            "BACKEND" => assert_eq!(v, "openai"),
            "BENCH_BUDGET_MIB" => assert_eq!(v, "4096"),
            "LEVELS" => assert!(v.split(',').all(|l| l.parse::<u32>().is_ok()), "{v}"),
            "ILEN" | "OLEN" | "NPROMPTS" | "WARMUPS" | "REPEATS" => {
                assert!(v.parse::<u32>().is_ok(), "{k}={v}")
            }
            "PRESET_PATH" => assert!(Path::new(v).is_file(), "{v}"),
            _ => {}
        }
    }
    for k in [
        "LEVELS",
        "ILEN",
        "OLEN",
        "NPROMPTS",
        "WARMUPS",
        "REPEATS",
        "REPEATS_SOURCE",
        "REPEAT_KIND",
        "BACKEND",
        "BENCH_BUDGET_MIB",
        "PRESET_PATH",
    ] {
        assert!(keys.iter().any(|x| x == k), "{k} missing in {out:?}");
    }
    let rec = read_json(&record);
    assert_keys(
        &rec,
        &[
            ("bench", &["string"]),
            ("preset", &["string"]),
            ("knobs", &["object"]),
            ("dataset", &["object"]),
        ],
        "knob record",
    );
    let spec = s.0.join("dataset_spec.json");
    ok(
        &run(
            &s.0,
            "bench_shell.sh",
            &["extract-dataset", ps(&record), ps(&spec)],
        ),
        "extract-dataset",
    );
    let kind = read_json(&spec)
        .at("dataset_kind")
        .str()
        .unwrap()
        .to_string();
    assert_eq!(
        ok(
            &run(&s.0, "bench_shell.sh", &["dataset-kind", ps(&spec)]),
            "dataset-kind"
        ),
        format!("{kind}\n")
    );
    assert!(!ok(
        &run(&s.0, "bench_shell.sh", &["dry-knobs", ps(&record)]),
        "dry-knobs"
    )
    .is_empty());
    // an Agent choice without its reason code is refused before anything is written
    let refused = s.0.join("refused.json");
    let out = Run::new(
        &s.0,
        "bench_preset.sh",
        &[
            "resolve",
            "--tool",
            "guidellm",
            "--serving-yaml",
            ps(&serving),
            "--record-out",
            ps(&refused),
            "--backend",
            "openai",
        ],
    )
    .env("BENCH_PRESET_REPO_ROOT", &repo)
    .out();
    assert_ne!(out.status.code(), Some(0));
    assert!(!refused.exists());
    // validate (the preset README): the default preset is valid
    ok(
        &Run::new(&s.0, "bench_preset.sh", &["validate"])
            .env("BENCH_PRESET_REPO_ROOT", &repo)
            .out(),
        "bench_preset validate",
    );
}

/// A vllm `bench serve --save-result` JSON as parse_bench reads it.
const VLLM_BENCH: &str = "{\"completed\": 10, \"failed\": 0, \"max_concurrency\": 1, \"num_prompts\": 10, \"median_tpot_ms\": 20.0, \
    \"mean_tpot_ms\": 21.0, \"output_throughput\": 48.0, \"total_token_throughput\": 100.0, \"median_ttft_ms\": 100.0, \"mean_ttft_ms\": 110.0, \
    \"median_itl_ms\": 20.0}";

/// sweep_bench.sh: `parse_bench --bench-json B --engine-log L > measured.json` (stdout is the measured JSON whose
/// `measurement_ok` `bench_shell measurement-ok` reads); `--json` prints the same document; an unreadable input is
/// non-zero.
#[test]
fn parse_bench_measured_json() {
    let s = scratch("parse-bench");
    let b = s.0.join("bench.json");
    write(&b, VLLM_BENCH);
    let log = s.0.join("engine.log");
    write(&log, "INFO Starting vLLM API server\n");
    for extra in [&["--engine-log", ps(&log)][..], &["--json"]] {
        let mut args = vec!["--bench-json", ps(&b)];
        args.extend_from_slice(extra);
        let doc = parse(
            &ok(&run(&s.0, "parse_bench.sh", &args), "parse_bench"),
            "measured",
        );
        assert_keys(
            &doc,
            &[
                ("decode_tps", &["number"]),
                ("output_throughput", &["number"]),
                ("ttft_ms_median", &["number"]),
                ("itl_ms_median", &["number"]),
                ("completed", &["number"]),
                ("failed", &["number"]),
                ("spec_on", &["bool"]),
                ("accept_len", &["number", "null"]),
                ("measurement_ok", &["bool"]),
            ],
            "measured",
        );
        assert_eq!(doc.at("measurement_ok"), &Json::Bool(true));
        let m = s.0.join("measured.json");
        std::fs::write(&m, doc_text(&doc)).unwrap();
        assert_eq!(
            run(&s.0, "bench_shell.sh", &["measurement-ok", ps(&m)])
                .status
                .code(),
            Some(0)
        );
    }
    assert_ne!(
        run(
            &s.0,
            "parse_bench.sh",
            &["--bench-json", ps(&s.0.join("absent.json"))]
        )
        .status
        .code(),
        Some(0)
    );
}

fn doc_text(doc: &Json) -> String {
    doc.dump()
}

/// sweep_bench.sh: `parse_guidellm --benchmarks-json B (--spec-axis-absent | --accept-len-src P) [--engine-log
/// --max-error-rate --post-health-json --prefix-cache-json]` prints the measured JSON (the parse_bench contract,
/// `measurement_ok`); hint-publisher `e4.rs`: `evaluate-jsonl` answers one `build` request with one line (`id` echoed,
/// `encoding` `json-values-v1`, `kind` value, `value_json`) and nothing on stderr.
#[test]
fn parse_guidellm_measured_json_and_build_transport() {
    let s = scratch("guidellm");
    let fx = fixture("guidellm_benchmarks_sample.json");
    let doc = parse(
        &ok(
            &run(
                &s.0,
                "parse_guidellm.sh",
                &["--benchmarks-json", ps(&fx), "--spec-axis-absent"],
            ),
            "parse_guidellm",
        ),
        "measured",
    );
    assert_keys(
        &doc,
        &[
            ("decode_tps", &["number"]),
            ("completed", &["number"]),
            ("failed", &["number"]),
            ("error_rate", &["number"]),
            ("spec_on", &["bool"]),
            ("measurement_ok", &["bool"]),
        ],
        "guidellm measured",
    );
    assert_eq!(doc.at("measurement_ok"), &Json::Bool(true));
    let log = s.0.join("engine.log");
    write(&log, "INFO x\n");
    let out = run(
        &s.0,
        "parse_guidellm.sh",
        &[
            "--benchmarks-json",
            ps(&fx),
            "--engine-log",
            ps(&log),
            "--spec-axis-absent",
            "--max-error-rate",
            "0.0",
        ],
    );
    assert_eq!(
        parse(&ok(&out, "parse_guidellm with options"), "m").at("measurement_ok"),
        &Json::Bool(true)
    );
    let src = s.0.join("lite_warm.json");
    write(&src, VLLM_BENCH);
    ok(
        &run(
            &s.0,
            "parse_guidellm.sh",
            &["--benchmarks-json", ps(&fx), "--accept-len-src", ps(&src)],
        ),
        "--accept-len-src",
    );
    // the e4.rs request
    let doc_arg = std::fs::read_to_string(&fx).unwrap();
    let req = format!(
        "{{\"id\": \"python-consumer\", \"operation\": \"build\", \"arguments_json\": {{\"doc\": {}, \"benchmark_index\": \"0\", \"engine_max\": \"null\", \
         \"spec\": \"null\", \"max_error_rate\": \"0.0\", \"server_alive_at_bench_end\": \"null\"}}, \"result_json\": true}}\n",
        Json::Str(doc_arg).dump()
    );
    let out = Run::new(&s.0, "parse_guidellm.sh", &["evaluate-jsonl"])
        .stdin(&req)
        .out();
    let rec = one_record(&out, "parse_guidellm evaluate-jsonl");
    assert_eq!(rec.at("kind").str(), Some("value"), "{rec:?}");
    let value = parse(rec.at("value_json").str().unwrap(), "value_json");
    assert_eq!(value.at("measurement_ok"), &Json::Bool(true));
}

/// The `evaluate-jsonl` answer of one request: exit 0, nothing on stderr, one line with `id` echoed and `encoding`
/// `json-values-v1`.
fn one_record(out: &Output, what: &str) -> Json {
    let so = ok(out, what);
    assert_eq!(text(&out.stderr), "", "{what}: stderr must stay empty");
    let lines: Vec<&str> = so.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(lines.len(), 1, "{what}: one record: {so}");
    let rec = parse(lines[0], what);
    assert_eq!(rec.at("id").str(), Some("python-consumer"), "{what}");
    assert_eq!(rec.at("encoding").str(), Some("json-values-v1"), "{what}");
    rec
}

/// sweep_bench.sh: `repeat_axis record-run --run-dir --level --run --started-utc --ended-utc --run-bench-rc --parse-rc`
/// writes `repeat_run.json`; `aggregate-level --level-dir --requested --requested-source --kind` adds `runs` to the
/// level's representative `measured.json`.
#[test]
fn repeat_axis_record_run_and_aggregate_level() {
    let s = scratch("repeat");
    let level = s.0.join("level_01");
    std::fs::create_dir_all(&level).unwrap();
    std::fs::copy(fixture("measured_pass.json"), level.join("measured.json")).unwrap();
    let out = run(
        &s.0,
        "repeat_axis.sh",
        &[
            "record-run",
            "--run-dir",
            ps(&level),
            "--level",
            "1",
            "--run",
            "1",
            "--started-utc",
            "2026-09-04T09:00:00Z",
            "--ended-utc",
            "2026-09-04T09:01:00Z",
            "--run-bench-rc",
            "0",
            "--parse-rc",
            "0",
        ],
    );
    ok(&out, "record-run");
    let rr = read_json(&level.join("repeat_run.json"));
    assert_keys(
        &rr,
        &[
            ("level", &["number"]),
            ("run", &["number"]),
            ("started_utc", &["string"]),
            ("ended_utc", &["string"]),
            ("run_bench_rc", &["number"]),
            ("parse_rc", &["number"]),
        ],
        "repeat_run.json",
    );
    assert_eq!(rr.at("started_utc").str(), Some("2026-09-04T09:00:00Z"));
    ok(
        &run(
            &s.0,
            "repeat_axis.sh",
            &[
                "aggregate-level",
                "--level-dir",
                ps(&level),
                "--requested",
                "3",
                "--requested-source",
                "declared(x)",
                "--kind",
                "warm-rerun",
            ],
        ),
        "aggregate-level",
    );
    let m = read_json(&level.join("measured.json"));
    assert_eq!(m.at("runs").kind(), "array");
    // a level without its run record cannot be aggregated
    let bare = s.0.join("level_02");
    std::fs::create_dir_all(&bare).unwrap();
    std::fs::copy(fixture("measured_pass.json"), bare.join("measured.json")).unwrap();
    assert_ne!(
        run(
            &s.0,
            "repeat_axis.sh",
            &[
                "aggregate-level",
                "--level-dir",
                ps(&bare),
                "--requested",
                "3",
                "--requested-source",
                "x",
                "--kind",
                "warm-rerun"
            ]
        )
        .status
        .code(),
        Some(0)
    );
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

/// sweep_bench.sh: `assemble_sweep_meta` reads everything from the environment (CONFIG, TOPO, CFGYAML, EF, MANIFEST,
/// SWEEPDIR, VLLM_VER, COMPLETED, ILEN, KNOB_RECORD, REASSEMBLE, LITE_RAW, SDIR, …) and writes
/// `$SWEEPDIR/sweep_index.json`, which judge_bench (`bench_shell judge-model-tp`), `prior-levels` and the classifier
/// read; `classify_cell --sweep-index --events-from-repo --manifest --write-bench-mode` prints the bench-mode record
/// and writes `bench_mode.json` beside the index; `bench_shell bench-mode-log` reads that stdout.
#[test]
fn assemble_sweep_meta_index_and_bench_mode_record() {
    let s = scratch("assemble");
    let r = s.0.join("in");
    copy_tree(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("checks/data/sweep_meta/c3-measured-over-declared/input"),
        &r,
    );
    let sweep = r.join("sweep_x");
    let mut a = Run::new(&s.0, "assemble_sweep_meta.sh", &[]);
    for (k, v) in [
        ("CFGYAML", r.join("cfg.yaml")),
        ("EF", r.join(".env.x")),
        ("KNOB_RECORD", r.join("bench_knobs.fixture.json")),
        ("MANIFEST", r.join("manifest.yaml")),
        ("SWEEPDIR", sweep.clone()),
        ("SDIR", skill().join("scripts")),
    ] {
        a = a.env(k, v);
    }
    for (k, v) in [
        ("COMPLETED", "1"),
        ("CONFIG", "x"),
        ("ILEN", "1024"),
        ("IMAGE_TAG_ACTUAL", "NA"),
        ("LITE_RAW", ""),
        ("REASSEMBLE", "0"),
        ("TOPO", "single"),
        ("VLLM_VER", ""),
    ] {
        a = a.env(k, v);
    }
    ok(&a.out(), "assemble_sweep_meta");
    let index = sweep.join("sweep_index.json");
    let doc = read_json(&index);
    assert_keys(
        &doc,
        &[
            ("config", &["string"]),
            ("topology", &["string"]),
            ("verdict_point_level", &["number"]),
            ("meta", &["object"]),
            ("levels", &["array"]),
            ("repetition", &["object"]),
            ("bench_knobs", &["object"]),
        ],
        "sweep_index",
    );
    assert_keys(
        doc.at("meta"),
        &[
            ("model_path", &["string"]),
            ("tensor_parallel_size", &["string"]),
            ("spec_declared", &["string"]),
            ("topology", &["string"]),
            ("measured_node", &["string"]),
        ],
        "sweep_index.meta",
    );
    let level = &doc.at("levels").arr().unwrap()[0];
    assert_eq!(level.at("level"), &Json::Num("1".into()));
    assert_eq!(
        ok(
            &run(&s.0, "bench_shell.sh", &["judge-model-tp", ps(&index)]),
            "judge-model-tp"
        ),
        "/models/acme/Acme-7B 1\n"
    );
    assert_eq!(
        ok(
            &run(&s.0, "bench_shell.sh", &["prior-levels", ps(&index)]),
            "prior-levels"
        ),
        "1\n"
    );
    let events_repo = s.0.join("events-repo");
    std::fs::create_dir_all(&events_repo).unwrap();
    let out = run(
        &s.0,
        "classify_cell.sh",
        &[
            "--sweep-index",
            ps(&index),
            "--events-from-repo",
            ps(&events_repo),
            "--manifest",
            ps(&r.join("manifest.yaml")),
            "--write-bench-mode",
        ],
    );
    let bm = parse(&ok(&out, "classify_cell bench-mode"), "bench-mode");
    assert_keys(
        &bm,
        &[
            ("bench_mode", &["string", "null"]),
            ("bench_mode_source", &["string"]),
            ("downgrade_reason", &["string", "null"]),
            ("downgrade_correlation", &["string"]),
        ],
        "bench-mode record",
    );
    let written = read_json(&sweep.join("bench_mode.json"));
    assert_eq!(written.at("bench_mode_source"), bm.at("bench_mode_source"));
    ok(
        &Run::new(&s.0, "bench_shell.sh", &["bench-mode-log"])
            .stdin(&text(&out.stdout))
            .out(),
        "bench-mode-log",
    );
}

/// broad_search.sh·sweep_bench.sh: `classify_cell --serve-rc --measure-rc --started-utc --ended-utc --events-from-repo
/// --topology --manifest` prints the cell classification JSON on stdout (warnings stay on stderr) with `cell_outcome`,
/// `void_reason`, `void_reason_source`, `lite_raw_status`, `events_scanned` that `bench_shell` reads.
#[test]
fn classify_cell_cell_mode_json() {
    let s = scratch("classify");
    let repo = s.0.join("repo");
    let manifest = repo.join("output/multi/manifest.yaml");
    write(&manifest, "self_role: main\ntopology: multi\n");
    for (serve, measure, outcome) in [("0", "0", "measured"), ("1", "absent", "serve_failed")] {
        let out = run(
            &s.0,
            "classify_cell.sh",
            &[
                "--serve-rc",
                serve,
                "--measure-rc",
                measure,
                "--started-utc",
                "2026-09-04T09:00:00Z",
                "--ended-utc",
                "2026-09-04T09:10:00Z",
                "--events-from-repo",
                ps(&repo),
                "--topology",
                "multi",
                "--manifest",
                ps(&manifest),
            ],
        );
        let cls = parse(&ok(&out, "classify_cell"), "classification");
        assert_keys(
            &cls,
            &[
                ("cell_outcome", &["string"]),
                ("void_reason", &["string", "null"]),
                ("void_reason_source", &["string", "null"]),
                ("lite_raw_status", &["string"]),
                ("events_scanned", &["array"]),
            ],
            "classification",
        );
        assert_eq!(
            cls.at("cell_outcome").str(),
            Some(outcome),
            "{serve}/{measure}"
        );
        for op in [
            "event-reason",
            "lite-raw-status",
            "void-reason-source",
            "events-scanned",
        ] {
            ok(
                &Run::new(&s.0, "bench_shell.sh", &[op])
                    .stdin(&text(&out.stdout))
                    .out(),
                op,
            );
        }
    }
}

/// policy-publisher `classify.rs`, hint-publisher `selftest.rs`, recipe `escalation_bridge.rs`: `classify_cell_api
/// constants` is one JSON object with `DOWNGRADE_REASONS`, `BENCH_MODE_LITE`, `EVENTS_RECOVERED_PARTS`,
/// `KILL_EVENT_KINDS`; `evaluate-jsonl` answers `bench_mode_kind`, `_utc`, `kill_events_in_window` one line each.
#[test]
fn classify_cell_api_constants_and_transport() {
    let s = scratch("classify-api");
    let c = parse(
        &ok(
            &run(&s.0, "classify_cell_api.sh", &["constants"]),
            "constants",
        ),
        "constants",
    );
    assert_eq!(c.at("DOWNGRADE_REASONS").kind(), "array");
    c.at("DOWNGRADE_REASONS").strs();
    assert_eq!(c.at("BENCH_MODE_LITE").str(), Some("lite"));
    assert_eq!(
        c.at("EVENTS_RECOVERED_PARTS").strs(),
        ["sync_staging", "sub_docs", "logs"]
    );
    assert!(!c.at("KILL_EVENT_KINDS").strs().is_empty());
    let call = |op: &str, args: &str| {
        let req = format!("{{\"id\": \"python-consumer\", \"operation\": \"{op}\", \"arguments_json\": {args}, \"result_json\": true}}\n");
        one_record(
            &Run::new(&s.0, "classify_cell_api.sh", &["evaluate-jsonl"])
                .stdin(&req)
                .out(),
            op,
        )
    };
    let rec = call(
        "bench_mode_kind",
        "{\"record\": \"{\\\"bench_mode\\\": \\\"lite\\\"}\"}",
    );
    assert_eq!(rec.at("kind").str(), Some("value"));
    assert_eq!(rec.at("value_json").kind(), "string");
    let started = call(
        "_utc",
        "{\"value\": \"\\\"2026-09-04T09:00:00Z\\\"\", \"field\": \"\\\"started\\\"\"}",
    );
    assert_eq!(started.at("kind").str(), Some("value"));
    let ended = call(
        "_utc",
        "{\"value\": \"\\\"2026-09-04T09:10:00Z\\\"\", \"field\": \"\\\"ended\\\"\"}",
    );
    let (sv, ev) = (
        started.at("value_json").str().unwrap().to_string(),
        ended.at("value_json").str().unwrap().to_string(),
    );
    let kill = format!(
        "{{\"event_lines\": {}, \"started\": {}, \"ended\": {}, \"tolerance_s\": \"0\"}}",
        Json::Str("[]".into()).dump(),
        Json::Str(sv).dump(),
        Json::Str(ev).dump()
    );
    let rec = call("kill_events_in_window", &kill);
    assert_eq!(rec.at("kind").str(), Some("value"), "{rec:?}");
    assert_eq!(
        parse(rec.at("value_json").str().unwrap(), "kill events").kind(),
        "array"
    );
    // an unknown operation is an exception record, not a crash
    let rec = call("no_such_operation", "{}");
    assert_eq!(rec.at("kind").str(), Some("exception"));
    assert_eq!(rec.at("type").kind(), "string");
}

// ───────────────────────────────────────────── judge_bench.sh: roofline and verdict

/// A model directory roofline reads: `config.json` and one safetensors header (no tensor data).
fn model_dir(at: &Path) {
    write(
        &at.join("config.json"),
        "{\"architectures\": [\"LlamaForCausalLM\"], \"hidden_size\": 4096, \"num_hidden_layers\": 32, \"num_attention_heads\": 32, \
         \"num_key_value_heads\": 8, \"intermediate_size\": 14336, \"vocab_size\": 128256, \"torch_dtype\": \"bfloat16\"}",
    );
    let header = b"{\"w\": {\"dtype\": \"BF16\", \"shape\": [8000000000], \"data_offsets\": [0, 16000000000]}}";
    let mut bytes = (header.len() as u64).to_le_bytes().to_vec();
    bytes.extend_from_slice(header);
    std::fs::write(at.join("model.safetensors"), bytes).unwrap();
}

/// judge_bench.sh: `roofline --model-path --json [--tp --bandwidth-gbps] --accept-len-source --accept-len-evidence`
/// prints the roofline JSON (redirected to roofline.json, read by `verdict_rule --roofline`); an invalid
/// `--accept-len-source` pairing is exit 2.
#[test]
fn roofline_json_for_verdict_rule() {
    let s = scratch("roofline");
    let model = s.0.join("model");
    model_dir(&model);
    let out = run(
        &s.0,
        "roofline.sh",
        &[
            "--model-path",
            ps(&model),
            "--json",
            "--tp",
            "1",
            "--bandwidth-gbps",
            "273",
            "--accept-len-source",
            "absent",
            "--accept-len-evidence",
            "fixture",
        ],
    );
    let roof = parse(&ok(&out, "roofline"), "roofline");
    assert_keys(
        &roof,
        &[
            ("tp", &["number"]),
            ("R_fp", &["number"]),
            ("R_token", &["number"]),
            ("expected_achievable", &["number"]),
            ("accept_len", &["number"]),
            ("accept_len_source", &["string"]),
            ("accept_len_evidence", &["string", "null"]),
        ],
        "roofline",
    );
    assert_eq!(roof.at("accept_len_source").str(), Some("absent"));
    let rf = s.0.join("roofline.json");
    std::fs::write(&rf, &out.stdout).unwrap();
    let v = run(
        &s.0,
        "verdict_rule.sh",
        &[
            "--measured",
            ps(&fixture("measured_pass.json")),
            "--roofline",
            ps(&rf),
            "--authority",
            "weak",
            "--spec-declared",
            "off",
        ],
    );
    parse(&ok(&v, "verdict on the roofline"), "verdict")
        .at("verdict")
        .str()
        .unwrap();
    let out = run(
        &s.0,
        "roofline.sh",
        &[
            "--model-path",
            ps(&model),
            "--json",
            "--bandwidth-gbps",
            "273",
            "--accept-len-source",
            "measured",
        ],
    );
    assert_eq!(out.status.code(), Some(2), "measured without --accept-len");
    let out = run(
        &s.0,
        "roofline.sh",
        &[
            "--model-path",
            ps(&model),
            "--json",
            "--bandwidth-gbps",
            "273",
            "--accept-len",
            "1.5",
            "--accept-len-source",
            "declared",
            "--accept-len-evidence",
            "x",
        ],
    );
    assert_eq!(
        parse(&ok(&out, "declared accept_len"), "roofline")
            .at("accept_len_source")
            .str(),
        Some("declared")
    );
}

/// judge_bench.sh: `verdict_rule spec-row --index --measured --human-value --level-rel` prints one line of four `\x1f`
/// fields (spec_declared, accept_len or `-`, source, evidence); `verdict_rule --measured --roofline --authority
/// --spec-declared [...]` prints the verdict JSON (verdict.json) whose `verdict`, `reason_code` and `rubric.{authority,
/// floor, ratio_M_over_primary, source, primary}` the publisher, hint and `bench_shell verdict|authority|judge-log`
/// read; the policy-distribution fixture run is exit 0; an authority outside `weak|explicit|explore` is exit 2.
#[test]
fn verdict_rule_spec_row_and_verdict_json() {
    let s = scratch("verdict");
    let row = ok(
        &run(
            &s.0,
            "verdict_rule.sh",
            &[
                "spec-row",
                "--index",
                ps(&fixture("sweep_index_sample.json")),
                "--measured",
                ps(&fixture("measured_pass.json")),
                "--human-value",
                "",
                "--level-rel",
                "level_01/measured.json",
            ],
        ),
        "spec-row",
    );
    let f = us_fields(&row);
    assert_eq!(f.len(), 4, "{row:?}");
    assert!(["on", "off", "unknown"].contains(&f[0].as_str()), "{row:?}");
    assert!(f[1] == "-" || f[1].parse::<f64>().is_ok(), "{row:?}");
    assert!(
        ["measured", "declared", "declared-absent", "absent"].contains(&f[2].as_str()),
        "{row:?}"
    );
    let (measured, roofline) = (
        fixture("measured_pass.json"),
        fixture("roofline_sample.json"),
    );
    let base = ["--measured", ps(&measured), "--roofline", ps(&roofline)];
    let out = run(&s.0, "verdict_rule.sh", &base);
    let v = parse(&ok(&out, "verdict fixture"), "verdict");
    assert_eq!(v.at("verdict").str(), Some("PASS"));
    assert_keys(
        &v,
        &[
            ("verdict", &["string"]),
            ("reason_code", &["string", "null"]),
            ("rubric", &["object"]),
        ],
        "verdict",
    );
    assert_keys(
        v.at("rubric"),
        &[
            ("authority", &["string"]),
            ("floor", &["number", "null"]),
            ("ratio_M_over_primary", &["number", "null"]),
            ("source", &["string", "null"]),
            ("primary", &["number", "null"]),
        ],
        "verdict.rubric",
    );
    let vj = s.0.join("verdict.json");
    std::fs::write(&vj, &out.stdout).unwrap();
    assert_eq!(
        ok(
            &run(&s.0, "bench_shell.sh", &["verdict", ps(&vj)]),
            "verdict"
        ),
        "PASS\n"
    );
    assert_eq!(
        ok(
            &run(&s.0, "bench_shell.sh", &["authority", ps(&vj)]),
            "authority"
        ),
        "weak\n"
    );
    // explicit takes the user target (c) as the canon and needs --target-tps (fail-closed without it)
    for (a, extra) in [
        ("weak", &[][..]),
        ("explicit", &["--target-tps", "20"]),
        ("explore", &[]),
    ] {
        let mut args = base.to_vec();
        args.extend(["--authority", a, "--spec-declared", "off"]);
        args.extend_from_slice(extra);
        let v = parse(&ok(&run(&s.0, "verdict_rule.sh", &args), a), "verdict");
        assert_eq!(v.at("rubric").at("authority").str(), Some(a));
    }
    let mut args = base.to_vec();
    args.extend([
        "--json",
        "--e-search",
        "no",
        "--tolerance",
        "0.15",
        "--target-tps",
        "20",
        "--authority",
        "explicit",
    ]);
    ok(
        &run(&s.0, "verdict_rule.sh", &args),
        "verdict with the judge_bench options",
    );
    for bad in [&["--authority", "bogus"][..], &["--authority", "explicit"]] {
        let mut args = base.to_vec();
        args.extend_from_slice(bad);
        let out = run(&s.0, "verdict_rule.sh", &args);
        assert_eq!(out.status.code(), Some(2), "{bad:?}");
        assert_eq!(text(&out.stdout), "", "{bad:?}");
    }
}

// ───────────────────────────────────────────── lite_bench.sh: judge, report, certificate

/// lite_bench.sh: `lite_metrics --raw-json R --judge` records `lite_verdict` and `lite_exit` in R (exit 0); the codes are
/// the routing ones (pass 0, measurement_path_failed 6, server_failed 7) and `bench_shell lite-verdict` reads the word.
#[test]
fn lite_metrics_judge_records_the_verdict() {
    let s = scratch("lite-judge");
    let raw = s.0.join("raw.json");
    std::fs::copy(fixture("lite_raw_d1_connection_refused.json"), &raw).unwrap();
    ok(
        &run(
            &s.0,
            "lite_metrics.sh",
            &["--raw-json", ps(&raw), "--judge"],
        ),
        "lite_metrics --judge",
    );
    let doc = read_json(&raw);
    assert_eq!(
        doc.at("lite_verdict").str(),
        Some("measurement_path_failed")
    );
    assert_eq!(doc.at("lite_exit"), &Json::Num("6".into()));
    assert_eq!(
        ok(
            &run(&s.0, "bench_shell.sh", &["lite-verdict", ps(&raw)]),
            "lite-verdict"
        ),
        "measurement_path_failed\n"
    );
    let json = parse(
        &ok(
            &run(&s.0, "lite_metrics.sh", &["--raw-json", ps(&raw), "--json"]),
            "--json",
        ),
        "lite json",
    );
    assert_eq!(json.kind(), "object");
    let api = parse(
        &ok(
            &run(&s.0, "lite_metrics_api.sh", &["constants"]),
            "lite constants",
        ),
        "constants",
    );
    assert_eq!(
        api.at("LITE_VERDICTS").strs(),
        ["pass", "measurement_path_failed", "server_failed"]
    );
    let exits = api.at("LITE_EXIT");
    assert_eq!(exits.at("pass"), &Json::Num("0".into()));
    assert_eq!(exits.at("measurement_path_failed"), &Json::Num("6".into()));
    assert_eq!(exits.at("server_failed"), &Json::Num("7".into()));
}

/// A lite raw whose verdict is `pass` (the shape policy-selftest `hint_suite.rs` builds).
fn lite_pass_raw(d: &Path) -> PathBuf {
    write(&d.join("serve.yaml"), "model: /app/quant_models/Org/Fixture-Model-NVFP4\ntensor-parallel-size: 2\nmax-model-len: 2048\n");
    write(
        &d.join("envx"),
        "CONFIG_FILE=cellx\nIMAGE_TAG=fixture:0.9.0-source\n",
    );
    write(&d.join("manifest.yaml"), "gpu_model: \"NVIDIA GB10\"\n");
    write(
        &d.join("bl/w.json"),
        "{\"completed\": 3, \"failed\": 0, \"median_tpot_ms\": 81.3, \"output_throughput\": 12.3}",
    );
    write(
        &d.join("bl/c.json"),
        "{\"completed\": 1, \"failed\": 0, \"median_ttft_ms\": 140.0}",
    );
    write(&d.join("bl/e.log"), "INFO Starting vLLM API server v0.9.0\nINFO GPU KV cache size: 54,192 tokens\nINFO Available KV cache memory: 8.5 GiB\n");
    let p = |n: &str| Json::Str(ps(&d.join(n)).to_string()).dump();
    let raw = d.join("lite_raw_cellx.json");
    write(
        &raw,
        &format!(
            "{{\"topology\": \"multi\", \"burst_n\": 3, \"config_name\": \"cellx\", \"measured_utc\": \"2026-09-04T09:00:00Z\", \"backend\": \"openai\", \
             \"endpoint\": \"/v1/completions\", \"config_yaml\": {}, \"env_file\": {}, \"manifest\": {}, \"bench_warm_json\": {}, \"bench_cold_json\": {}, \
             \"engine_log\": {}, \"lite_verdict\": \"pass\", \"lite_exit\": 0, \"lite_verdict_source\": \"fixture\", \"nodes\": [{{\"role\": \"main\", \
             \"gpu_smi_used_mib\": null, \"gpu_smi_total_mib\": null, \"ram_total_kib\": 128000000, \"ram_avail_kib\": 64000000}}]}}",
            p("serve.yaml"),
            p("envx"),
            p("manifest.yaml"),
            p("bl/w.json"),
            p("bl/c.json"),
            p("bl/e.log")
        ),
    );
    raw
}

/// The flat certificate (`key: value` lines, no indentation, no duplicate key) as completion_gate, the publisher and hint
/// read it.
fn flat_certificate(doc: &str) -> Vec<(String, String)> {
    let mut seen: Vec<(String, String)> = Vec::new();
    for line in doc.lines() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        assert!(
            !line.starts_with(' ') && !line.starts_with('\t'),
            "indented line in a flat certificate: {line:?}"
        );
        let (k, v) = line
            .split_once(": ")
            .unwrap_or_else(|| panic!("not `key: value`: {line:?}"));
        assert!(!seen.iter().any(|(x, _)| x == k), "duplicate key {k}");
        seen.push((k.to_string(), v.to_string()));
    }
    seen
}

fn flat(cert: &[(String, String)], key: &str) -> String {
    cert.iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| panic!("{key} missing in the certificate"))
}

/// lite_bench.sh and policy-selftest `hint_suite.rs`: `render_report --lite-only --lite-raw-json R` prints the written
/// report path (`--stdout` prints the report: `mode: lite`, `| bench_mode | lite |`, `생성일 <measured_utc>`, `## lite
/// 지표`, `## 측정 구성 — bench_mode · 도구 · 반복`); `publish_benchmark_record --lite-raw-json R` prints the written
/// certificate path (`--stdout`: the flat certificate with `benchmark_mode: lite`); a raw whose verdict is not `pass`
/// gets no certificate (non-zero, nothing written).
#[test]
fn lite_report_and_certificate() {
    let s = scratch("lite-report");
    let raw = lite_pass_raw(&s.0);
    let md = ok(
        &run(
            &s.0,
            "render_report.sh",
            &["--lite-only", "--lite-raw-json", ps(&raw), "--stdout"],
        ),
        "render_report --lite-only --stdout",
    );
    for needle in [
        "mode: lite",
        "| bench_mode | lite |",
        "생성일 2026-09-04T09:00:00Z",
        "## lite 지표",
        "## 측정 구성 — bench_mode · 도구 · 반복",
    ] {
        assert!(
            md.contains(needle),
            "{needle:?} missing in the lite report:\n{md}"
        );
    }
    let out_dir = s.0.join("out");
    let printed = ok(
        &run(
            &s.0,
            "render_report.sh",
            &[
                "--lite-only",
                "--lite-raw-json",
                ps(&raw),
                "--out-dir",
                ps(&out_dir),
            ],
        ),
        "render_report --out-dir",
    );
    let path = printed.lines().last().unwrap().trim();
    assert!(
        Path::new(path).is_file() && Path::new(path).starts_with(&out_dir),
        "stdout names the written report: {printed:?}"
    );
    assert!(
        Path::new(path)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("bench_report_"),
        "{path}"
    );
    let cert = ok(
        &run(
            &s.0,
            "publish_benchmark_record.sh",
            &["--lite-raw-json", ps(&raw), "--stdout"],
        ),
        "publish --lite-raw-json --stdout",
    );
    let c = flat_certificate(&cert);
    assert_eq!(flat(&c, "benchmark_mode"), "lite");
    assert_eq!(flat(&c, "lite_verdict"), "pass");
    assert_eq!(flat(&c, "record_type"), "benchmark_certificate");
    let printed = ok(
        &run(
            &s.0,
            "publish_benchmark_record.sh",
            &["--lite-raw-json", ps(&raw), "--out-dir", ps(&out_dir)],
        ),
        "publish --out-dir",
    );
    let path = printed.lines().last().unwrap().trim();
    assert!(
        Path::new(path).is_file() && Path::new(path).starts_with(&out_dir),
        "stdout names the certificate: {printed:?}"
    );
    assert!(
        Path::new(path)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("benchmark_"),
        "{path}"
    );
    // not pass: no certificate
    let failed = s.0.join("failed");
    std::fs::create_dir_all(&failed).unwrap();
    let fraw = failed.join("raw.json");
    std::fs::write(
        &fraw,
        std::fs::read_to_string(&raw).unwrap().replace(
            "\"lite_verdict\": \"pass\"",
            "\"lite_verdict\": \"server_failed\"",
        ),
    )
    .unwrap();
    let fout = failed.join("out");
    let out = run(
        &s.0,
        "publish_benchmark_record.sh",
        &["--lite-raw-json", ps(&fraw), "--out-dir", ps(&fout)],
    );
    assert_ne!(out.status.code(), Some(0));
    assert!(
        !fout.exists() || std::fs::read_dir(&fout).unwrap().next().is_none(),
        "nothing written"
    );
}

/// The full report (render_report `--sweep-index --verdict-json`) carries what hint-publisher `render_bench_section.rs`,
/// the publisher's measurement-config check and policy-selftest read: the sweep table head, `★판정점`, `## 측정 구성 —
/// bench_mode · 도구 · 반복` and the `## 판정 (표시만` section with the `verdict` and `루브릭 권한` rows; the full
/// certificate is flat `key: value` with `benchmark_mode: full` and `verdict: PASS` (only on PASS).
#[test]
fn full_report_sections_and_certificate() {
    let s = scratch("full-report");
    let idx = s.0.join("sweep_index.json");
    std::fs::copy(fixture("sweep_index_sample.json"), &idx).unwrap();
    let vj = s.0.join("verdict.json");
    let out = run(
        &s.0,
        "verdict_rule.sh",
        &[
            "--measured",
            ps(&fixture("measured_pass.json")),
            "--roofline",
            ps(&fixture("roofline_sample.json")),
        ],
    );
    std::fs::write(&vj, ok(&out, "verdict")).unwrap();
    let md = ok(
        &run(
            &s.0,
            "render_report.sh",
            &[
                "--sweep-index",
                ps(&idx),
                "--verdict-json",
                ps(&vj),
                "--stdout",
            ],
        ),
        "render_report full",
    );
    for needle in [
        "| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |",
        "★판정점",
        "## 측정 구성 — bench_mode · 도구 · 반복",
        "## 판정 (표시만",
        "| verdict | **PASS** |",
        "| 루브릭 권한 | weak |",
    ] {
        assert!(
            md.contains(needle),
            "{needle:?} missing in the full report:\n{md}"
        );
    }
    let rf = s.0.join("roofline.json");
    std::fs::copy(fixture("roofline_sample.json"), &rf).unwrap();
    ok(
        &run(
            &s.0,
            "render_report.sh",
            &[
                "--sweep-index",
                ps(&idx),
                "--verdict-json",
                ps(&vj),
                "--roofline-json",
                ps(&rf),
                "--stdout",
            ],
        ),
        "with --roofline-json",
    );
    let out_dir = s.0.join("out");
    let printed = ok(
        &run(
            &s.0,
            "render_report.sh",
            &[
                "--sweep-index",
                ps(&idx),
                "--verdict-json",
                ps(&vj),
                "--out-dir",
                ps(&out_dir),
            ],
        ),
        "render_report --out-dir",
    );
    assert!(
        Path::new(printed.lines().last().unwrap().trim()).is_file(),
        "{printed:?}"
    );
    let cert = ok(
        &run(
            &s.0,
            "publish_benchmark_record.sh",
            &[
                "--sweep-index",
                ps(&idx),
                "--verdict-json",
                ps(&vj),
                "--stdout",
            ],
        ),
        "publish full",
    );
    let c = flat_certificate(&cert);
    assert_eq!(flat(&c, "benchmark_mode"), "full");
    assert_eq!(flat(&c, "verdict"), "PASS");
    for k in [
        "model",
        "gpu_model",
        "vllm_version",
        "quantization",
        "topology",
        "tensor_parallel_size",
        "measured_utc",
        "rubric_authority",
    ] {
        flat(&c, k);
    }
    // judge_bench.sh auto-publishes with --sweep-index --verdict-json: a REFUTE verdict is no certificate
    let refute = s.0.join("refute.json");
    std::fs::write(
        &refute,
        std::fs::read_to_string(&vj).unwrap().replacen(
            "\"verdict\": \"PASS\"",
            "\"verdict\": \"REFUTE\"",
            1,
        ),
    )
    .unwrap();
    let rout = s.0.join("rout");
    let out = run(
        &s.0,
        "publish_benchmark_record.sh",
        &[
            "--sweep-index",
            ps(&idx),
            "--verdict-json",
            ps(&refute),
            "--out-dir",
            ps(&rout),
        ],
    );
    assert!(
        !rout.exists() || std::fs::read_dir(&rout).unwrap().next().is_none(),
        "no certificate for REFUTE: {}",
        text(&out.stdout)
    );
}

/// policy-selftest `suite.rs` and policy-gate `doc_naming.rs`: `bench_reporting_api evaluate-jsonl` answers a request
/// with one line `{id, kind: value|exception, value | type, message}` and nothing on stderr: `render_report.build_md`
/// returns the report text with the verdict table; `doc_naming.kst_tokens` a triple; an exception is a record.
#[test]
fn bench_reporting_api_transport() {
    let s = scratch("reporting-api");
    let call = |line: &str| -> Json {
        let out = Run::new(&s.0, "bench_reporting_api.sh", &["evaluate-jsonl"])
            .stdin(&format!("{line}\n"))
            .out();
        let so = ok(&out, "bench_reporting_api evaluate-jsonl");
        assert_eq!(text(&out.stderr), "");
        let lines: Vec<&str> = so.lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(lines.len(), 1, "{so}");
        parse(lines[0], "record")
    };
    let rec = call(
        "{\"id\": \"writer-contract\", \"operation\": \"render_report.build_md\", \"arguments\": {\"index\": {\"meta\": {}, \"levels\": []}, \
         \"verdict\": {\"verdict\": \"PASS\", \"rubric\": {\"authority\": \"explore\"}}, \"roofline\": null}}",
    );
    assert_eq!(rec.at("id").str(), Some("writer-contract"));
    assert_eq!(rec.at("kind").str(), Some("value"));
    let md = rec.at("value").str().unwrap();
    assert!(
        md.contains("## 판정 (표시만")
            && md.contains("| verdict | **PASS** |")
            && md.contains("| 루브릭 권한 | explore |"),
        "{md}"
    );
    let rec = call("{\"id\": \"call\", \"operation\": \"doc_naming.kst_tokens\", \"arguments\": {\"generated_utc\": \"2026-09-04T09:00:00Z\"}}");
    assert_eq!(rec.at("id").str(), Some("call"));
    assert_eq!(rec.at("kind").str(), Some("value"), "{rec:?}");
    assert_eq!(rec.at("value").strs().len(), 3);
    let rec = call("{\"id\": \"call\", \"operation\": \"doc_naming.gpu_key\", \"arguments\": {\"gpu_model\": \"NVIDIA GB10\"}}");
    assert_eq!(rec.at("kind").str(), Some("value"), "{rec:?}");
    let rec = call("{\"id\": \"call\", \"operation\": \"doc_naming.kst_tokens\", \"arguments\": {\"bogus\": 1}}");
    assert_eq!(rec.at("kind").str(), Some("exception"));
    assert_eq!(rec.at("type").kind(), "string");
    assert_eq!(rec.at("message").kind(), "string");
}

/// max_envelope.sh names `render_max_report --max-index <max_index.json>`: it prints the written report path (exit 0);
/// `--stdout` prints the report.
#[test]
fn render_max_report_path_and_stdout() {
    let s = scratch("max-report");
    let mi = fixture("max_index_sample.json");
    assert!(!ok(
        &run(
            &s.0,
            "render_max_report.sh",
            &["--max-index", ps(&mi), "--stdout"]
        ),
        "--stdout"
    )
    .is_empty());
    let out_dir = s.0.join("out");
    let printed = ok(
        &run(
            &s.0,
            "render_max_report.sh",
            &["--max-index", ps(&mi), "--out-dir", ps(&out_dir)],
        ),
        "--out-dir",
    );
    let path = printed.lines().last().unwrap().trim();
    assert!(
        Path::new(path).is_file() && Path::new(path).starts_with(&out_dir),
        "{printed:?}"
    );
}

/// run_bench.sh: `dataset_prep synthetic-sidecar --input-len --output-len --sidecar F` writes the dataset sidecar
/// (`dataset_kind` synthetic, `input_len`, `output_len`); exit 0. `<subcommand> --help` lists the options as whole
/// tokens; `prepare` and `slice` are probed by running them (`dataset_prep_prepare_and_slice_by_argument_probe`).
#[test]
fn dataset_prep_synthetic_sidecar_and_subcommands() {
    let s = scratch("dataset");
    let side = s.0.join("dataset_cfg.json");
    ok(
        &run(
            &s.0,
            "dataset_prep.sh",
            &[
                "synthetic-sidecar",
                "--input-len",
                "128",
                "--output-len",
                "64",
                "--sidecar",
                ps(&side),
            ],
        ),
        "synthetic-sidecar",
    );
    let d = read_json(&side);
    assert_eq!(d.at("dataset_kind").str(), Some("synthetic"));
    assert_eq!(d.at("input_len"), &Json::Num("128".into()));
    assert_eq!(d.at("output_len"), &Json::Num("64".into()));
    for (sub, opts) in [
        (
            "prepare",
            &[
                "--spec",
                "--tokstage",
                "--cache",
                "--image",
                "--budget-mib",
                "--input-len",
                "--output-len",
            ][..],
        ),
        (
            "slice",
            &[
                "--spec",
                "--cache",
                "--run-index",
                "--sweep-salt",
                "--count",
                "--input-len",
                "--output-len",
                "--out",
                "--sidecar",
                "--tokenizer-label",
            ],
        ),
        (
            "synthetic-sidecar",
            &["--input-len", "--output-len", "--sidecar"],
        ),
    ] {
        let help = ok(&run(&s.0, "dataset_prep.sh", &[sub, "--help"]), sub);
        for o in opts {
            assert!(names_option(&help, o), "dataset_prep {sub}: {o} not listed");
        }
    }
}

// ───────────────────────────────────────────── resolvers

/// A checkout with branch `branch`, a specialised constitution header and both manifests: what topology_parity's
/// four-way predicate reads.
fn topology_repo(at: &Path, branch: &str, header: &str) {
    std::fs::create_dir_all(at).unwrap();
    let git = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(at)
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {}", text(&out.stderr));
    };
    git(&["init", "-q"]);
    git(&["symbolic-ref", "HEAD", &format!("refs/heads/{branch}")]);
    write(
        &at.join(".claude/rules/strategy.topology.md"),
        &format!("# t\n\n**topology: {header}** · layer: topology\n"),
    );
    git(&["add", "-A"]);
    write(
        &at.join("output/multi/manifest.yaml"),
        "self_role: main\ntopology: multi\n",
    );
    write(
        &at.join("output/single/manifest.yaml"),
        "self_role: main\ntopology: single\n",
    );
}

/// The bench scripts: `resolve_topology --repo R` prints `single` or `multi` (exit 0); exit 3 = no manifest at all (an
/// unterraformed tree, info-only for run_bench·lite_bench), another non-zero (5) = not resolvable; nothing on stdout
/// then. A repository path that is not UTF-8 is carried as a path.
#[test]
fn resolve_topology_stdout_and_exit_codes() {
    let s = scratch("topology");
    for (name, branch, header, want) in [
        ("m", "multi-node", "multi", "multi"),
        ("s", "single-node", "single", "single"),
    ] {
        let r = s.0.join(name);
        topology_repo(&r, branch, header);
        assert_eq!(
            ok(&run(&s.0, "resolve_topology.sh", &["--repo", ps(&r)]), name),
            format!("{want}\n")
        );
    }
    let bare = s.0.join("bare");
    std::fs::create_dir_all(&bare).unwrap();
    let out = run(&s.0, "resolve_topology.sh", &["--repo", ps(&bare)]);
    assert_eq!(out.status.code(), Some(3));
    assert_eq!(text(&out.stdout), "");
    let x = s.0.join("x");
    topology_repo(&x, "feature-x", "multi");
    let out = run(&s.0, "resolve_topology.sh", &["--repo", ps(&x)]);
    assert!(
        !matches!(out.status.code(), Some(0) | Some(3)),
        "an unresolvable branch is neither resolved nor unterraformed"
    );
    assert_eq!(text(&out.stdout), "");
    let odd = s.0.join(OsStr::from_bytes(b"r\xff"));
    topology_repo(&odd, "multi-node", "multi");
    let out = Run::new(&s.0, "resolve_topology.sh", &["--repo"])
        .arg(&odd)
        .out();
    assert_eq!(ok(&out, "non-UTF-8 repository path"), "multi\n");
}

/// run_bench.sh and policy-predicates `claims_text.rs`: `resolve_bench_tool --tool --pin [--platform] --json` prints a
/// JSON object with exactly the keys tool, version, version_source, image_ref, recorded_digest, observed_digest,
/// digest_source, matches_record, platform_note, status (in any order); `status` `resolved` is exit 0, `absent` is exit
/// 3 with `ABSENT` on stderr. The engine is a stand-in program on PATH (no image is pulled or inspected for real).
#[test]
fn resolve_bench_tool_json_and_status() {
    let s = scratch("bench-tool");
    let pin = s.0.join("bench_tool_pin.json");
    std::fs::copy(skill().join("bench_tool_pin.json"), &pin).unwrap();
    let bin = s.0.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let engine = bin.join("docker");
    let path = format!("{}:{}", ps(&bin), std::env::var("PATH").unwrap_or_default());
    let ask = |extra: &[&str]| {
        let mut args = vec!["--tool", "guidellm", "--pin", ps(&pin), "--json"];
        args.extend_from_slice(extra);
        Run::new(&s.0, "resolve_bench_tool.sh", &args)
            .env("PATH", &path)
            .out()
    };
    // The key set is fixed (L2); the key order is not.
    let key_set = |doc: &Json| {
        let mut keys = doc.keys();
        keys.sort_unstable();
        keys.iter().map(|k| k.to_string()).collect::<Vec<_>>()
    };
    let mut keys = [
        "tool",
        "version",
        "version_source",
        "image_ref",
        "recorded_digest",
        "observed_digest",
        "digest_source",
        "matches_record",
        "platform_note",
        "status",
    ];
    keys.sort_unstable();
    write(
        &engine,
        "#!/bin/sh\necho 'Error: No such image' >&2\nexit 1\n",
    );
    std::fs::set_permissions(&engine, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let out = ask(&[]);
    assert_eq!(out.status.code(), Some(3), "absent image");
    let doc = parse(&text(&out.stdout), "absent");
    assert_eq!(key_set(&doc), keys);
    assert_eq!(
        doc.at("tool").str(),
        Some("guidellm"),
        "--tool is its own option, not --tool-version"
    );
    assert_eq!(doc.at("status").str(), Some("absent"));
    assert_eq!(doc.at("matches_record"), &Json::Null);
    assert!(
        text(&out.stderr).contains("ABSENT"),
        "{}",
        text(&out.stderr)
    );
    let image = doc.at("image_ref").str().unwrap().to_string();
    let digest = doc.at("recorded_digest").str().unwrap().to_string();
    let repo = image
        .rsplit_once(':')
        .map(|(r, _)| r)
        .unwrap_or(&image)
        .to_string();
    write(
        &engine,
        &format!("#!/bin/sh\necho '[\"{repo}@{digest}\"]'\n"),
    );
    let out = ask(&["--platform", "linux/amd64"]);
    let doc = parse(&ok(&out, "resolved"), "resolved");
    assert_eq!(key_set(&doc), keys);
    assert_eq!(doc.at("status").str(), Some("resolved"));
    assert_eq!(doc.at("matches_record"), &Json::Bool(true));
    assert_eq!(doc.at("observed_digest").str(), Some(digest.as_str()));
    assert_eq!(
        doc.at("digest_source").str(),
        Some("measured(docker image inspect .RepoDigests)")
    );
    // without --json run_bench reruns to show the reason on stderr: same exit code
    write(&engine, "#!/bin/sh\nexit 1\n");
    let out = Run::new(
        &s.0,
        "resolve_bench_tool.sh",
        &["--tool", "guidellm", "--pin", ps(&pin)],
    )
    .env("PATH", &path)
    .out();
    assert_eq!(out.status.code(), Some(3));
}

// ───────────────────────────────────────────── the *_api launchers and option names

/// The `*_api` bins (code consumers and the conformance transport): `native-help` and `constants` are exit 0, and
/// `constants` is one JSON object.
#[test]
fn api_bins_native_help_and_constants() {
    let s = scratch("api");
    for file in [
        "bench_preset_api.sh",
        "bench_reporting_api.sh",
        "classify_cell_api.sh",
        "conformance_full_superset_lite_api.sh",
        "dataset_prep_api.sh",
        "lite_metrics_api.sh",
        "repeat_axis_api.sh",
        "resolve_bench_tool_api.sh",
        "resolve_guidellm_api.sh",
        "resolve_topology_api.sh",
    ] {
        let help = ok(&run(&s.0, file, &["native-help"]), file);
        assert!(
            names_option(&help, "evaluate-jsonl") && names_option(&help, "constants"),
            "{file}: {help}"
        );
        let c = parse(&ok(&run(&s.0, file, &["constants"]), file), file);
        assert_eq!(c.kind(), "object", "{file}");
    }
}

/// The long options and subcommands the documents name are listed by each `--help` (exit 0) as whole tokens (the help
/// layout itself is free). The help is a captured copy, not the parser: the options the consumers pass are probed by
/// running the parser with them (the `*_by_argument_probe` checks and the domain checks above).
#[test]
fn documented_options_and_subcommands() {
    let s = scratch("options");
    let rows: [(&str, &[&str], &[&str]); 18] = [
        (
            "bench_preset.sh",
            &[],
            &["validate", "default", "resolve", "--self-test"],
        ),
        (
            "bench_preset.sh",
            &["resolve"],
            &[
                "--tool",
                "--serving-yaml",
                "--record-out",
                "--dry-run",
                "--preset",
                "--levels",
                "--input-len",
                "--output-len",
                "--num-prompts",
                "--warmups",
                "--backend",
                "--bench-budget-mib",
                "--repeats",
                "--repeats-source",
                "--campaign-id",
                "--pinned-by",
                "--approved-by",
                "--pin",
                "--knob-reason",
            ],
        ),
        ("bench_preset.sh", &["default"], &["--knob", "--preset"]),
        (
            "repeat_axis.sh",
            &[],
            &["resolve", "record-run", "aggregate-level", "--self-test"],
        ),
        (
            "repeat_axis.sh",
            &["resolve"],
            &[
                "--repeats",
                "--preset-default",
                "--preset-source",
                "--campaign-id",
            ],
        ),
        (
            "repeat_axis.sh",
            &["record-run"],
            &[
                "--run-dir",
                "--level",
                "--run",
                "--started-utc",
                "--ended-utc",
                "--run-bench-rc",
                "--parse-rc",
            ],
        ),
        (
            "repeat_axis.sh",
            &["aggregate-level"],
            &["--level-dir", "--requested", "--requested-source", "--kind"],
        ),
        (
            "verdict_rule.sh",
            &[],
            &[
                "--measured",
                "--roofline",
                "--reference-tps",
                "--target-tps",
                "--authority",
                "--tolerance",
                "--spec-supported",
                "--spec-declared",
                "--e-search",
                "--node-vram-gib",
                "--balance-tol",
                "--self-test",
                "--json",
            ],
        ),
        (
            "roofline.sh",
            &[],
            &[
                "--model-path",
                "--nas-root",
                "--quant-root",
                "--manifest",
                "--tp",
                "--accept-len",
                "--accept-len-source",
                "--accept-len-evidence",
                "--bandwidth-gbps",
                "--realistic-fraction",
                "--json",
            ],
        ),
        (
            "parse_bench.sh",
            &[],
            &["--bench-json", "--engine-log", "--json"],
        ),
        (
            "parse_guidellm.sh",
            &[],
            &[
                "--benchmarks-json",
                "--engine-log",
                "--benchmark-index",
                "--accept-len-src",
                "--spec-axis-absent",
                "--post-health-json",
                "--prefix-cache-json",
                "--max-error-rate",
                "--json",
                "--self-test",
            ],
        ),
        ("lite_metrics.sh", &[], &["--raw-json", "--json", "--judge"]),
        (
            "render_report.sh",
            &[],
            &[
                "--sweep-index",
                "--verdict-json",
                "--lite-only",
                "--lite-raw-json",
                "--roofline-json",
                "--bench-mode-json",
                "--out-dir",
                "--stdout",
            ],
        ),
        (
            "publish_benchmark_record.sh",
            &[],
            &[
                "--sweep-index",
                "--verdict-json",
                "--lite-raw-json",
                "--out-dir",
                "--stdout",
            ],
        ),
        (
            "render_max_report.sh",
            &[],
            &["--max-index", "--out-dir", "--stdout"],
        ),
        (
            "render_sweep_map.sh",
            &[],
            &[
                "--state",
                "--stop-json",
                "--out-md",
                "--out-json",
                "--self-test",
            ],
        ),
        (
            "classify_cell.sh",
            &[],
            &[
                "--serve-rc",
                "--measure-rc",
                "--started-utc",
                "--ended-utc",
                "--events",
                "--events-from-repo",
                "--topology",
                "--manifest",
                "--tolerance-s",
                "--cross-node-tolerance-s",
                "--cross-node-tolerance-source",
                "--sweep-index",
                "--write-bench-mode",
                "--lite-raw",
                "--json",
                "--self-test",
            ],
        ),
        (
            "resolve_bench_tool.sh",
            &[],
            &[
                "--tool",
                "--tool-version",
                "--pin",
                "--platform",
                "--json",
                "--self-test",
            ],
        ),
    ];
    for (file, sub, names) in rows {
        let mut args = sub.to_vec();
        args.push("--help");
        let help = ok(&run(&s.0, file, &args), file);
        for n in names {
            assert!(names_option(&help, n), "{file} {sub:?}: {n} not listed");
        }
    }
    for (file, names) in [
        (
            "sweep_stop.sh",
            &["--state", "--now-utc", "--self-test"][..],
        ),
        ("resolve_topology.sh", &["--repo", "--self-test"]),
        (
            "resolve_guidellm.sh",
            &[
                "--version",
                "--repo",
                "--record",
                "--record-path",
                "--json",
                "--self-test",
            ],
        ),
    ] {
        let help = ok(&run(&s.0, file, &["--help"]), file);
        for n in names {
            assert!(names_option(&help, n), "{file}: {n} not listed");
        }
    }
    // parse_guidellm spec-source --lite-raw (sweep_bench.sh): a raw without its warm JSON is not a source
    let raw = s.0.join("raw.json");
    std::fs::copy(fixture("lite_raw_d1_connection_refused.json"), &raw).unwrap();
    assert_ne!(
        run(
            &s.0,
            "parse_guidellm.sh",
            &["spec-source", "--lite-raw", ps(&raw)]
        )
        .status
        .code(),
        Some(0)
    );
}

// ───────────────────────────────────────────── the options the consumers pass, probed through the parser

/// `KEY\x1fVALUE` lines as a map.
fn us_map(stdout: &str) -> Vec<(String, String)> {
    stdout
        .lines()
        .map(|l| {
            let (k, v) = l
                .split_once('\u{1f}')
                .unwrap_or_else(|| panic!("not KEY\\x1fVALUE: {l:?}"));
            (k.to_string(), v.to_string())
        })
        .collect()
}

fn us_get<'a>(lines: &'a [(String, String)], key: &str) -> &'a str {
    lines
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
        .unwrap_or_else(|| panic!("{key} missing in {lines:?}"))
}

/// A scratch repository with the campaign writer's launcher (`CAMPAIGN_READ_BIN` = this build) and one campaign `cX`
/// declaring `budgets.repeats` = 5: what bench_preset and repeat_axis read for `--campaign-id`.
fn campaign_repo(at: &Path) -> std::ffi::OsString {
    let launcher = ".claude/skills/terraforming_node/scripts/campaign_init.sh";
    let real = skill().join("../../..").join(launcher);
    std::fs::create_dir_all(at.join(launcher).parent().unwrap()).unwrap();
    std::fs::copy(real, at.join(launcher)).unwrap();
    write(
        &at.join("campaigns/cX/campaign.yaml"),
        "{\"id\": \"cX\", \"budgets\": {\"repeats\": 5}}\n",
    );
    env!("CARGO_BIN_EXE_campaign_init").into()
}

/// sweep_bench.sh `bench_preset resolve` options reach their knobs: `--levels` with a confirm-grade value opens only with
/// `--approved-by` (recorded); `--input-len`, `--num-prompts`, `--warmups` with their `--knob-reason` are agent choices;
/// `--pin KNOB=VALUE --pinned-by` opens an escalate-grade value (`--pin` is not `--pinned-by`); `--repeats
/// --repeats-source` is a declaration (`--repeats` is not `--repeats-source`); `--campaign-id` reads that campaign's
/// `budgets.repeats` through the campaign launcher; `--dry-run` accepts an absent serving yaml; `--preset` names the
/// preset (broad_search's `default --knob --preset` too).
#[test]
fn bench_preset_resolve_consumer_options_by_argument_probe() {
    let s = scratch("preset-probe");
    let repo = s.0.join("repo");
    let ci_bin = campaign_repo(&repo);
    let serving = s.0.join("serve.yaml");
    write(
        &serving,
        "model: /app/models/x\ntensor-parallel-size: 1\nmax-model-len: 8192\n",
    );
    let record = s.0.join("knobs.json");
    let base = [
        "resolve",
        "--tool",
        "guidellm",
        "--serving-yaml",
        ps(&serving),
        "--record-out",
        ps(&record),
        "--backend",
        "openai",
        "--knob-reason",
        "backend=prior_cell_shape",
        "--bench-budget-mib",
        "4096",
        "--knob-reason",
        "bench_budget_mib=host_budget",
    ];
    let resolve = |extra: &[&str]| {
        let mut args = base.to_vec();
        args.extend_from_slice(extra);
        Run::new(&s.0, "bench_preset.sh", &args)
            .env("BENCH_PRESET_REPO_ROOT", &repo)
            .env("CAMPAIGN_READ_BIN", &ci_bin)
            .out()
    };
    let confirm = [
        "--levels",
        "1,2,32",
        "--knob-reason",
        "levels=prior_cell_shape",
    ];
    let mut all = confirm.to_vec();
    all.extend_from_slice(&[
        "--approved-by",
        "tester",
        "--input-len",
        "2048",
        "--knob-reason",
        "input_len=prior_cell_shape",
        "--pin",
        "output_len=512",
        "--pinned-by",
        "tester",
        "--num-prompts",
        "16",
        "--knob-reason",
        "num_prompts=preset_default",
        "--warmups",
        "2",
        "--knob-reason",
        "warmups=preset_default",
        "--repeats",
        "3",
        "--repeats-source",
        "declared(t)",
        "--preset",
        "default_preset",
    ]);
    let lines = us_map(&ok(&resolve(&all), "resolve with the sweep options"));
    assert_eq!(us_get(&lines, "LEVELS"), "1,2,32");
    assert_eq!(us_get(&lines, "ILEN"), "2048");
    assert_eq!(us_get(&lines, "OLEN"), "512");
    assert_eq!(us_get(&lines, "NPROMPTS"), "16");
    assert_eq!(us_get(&lines, "WARMUPS"), "2");
    assert_eq!(us_get(&lines, "REPEATS"), "3");
    assert!(
        us_get(&lines, "REPEATS_SOURCE").contains("declared(t)"),
        "{lines:?}"
    );
    assert!(
        us_get(&lines, "PRESET_PATH").ends_with("default_preset.yaml"),
        "{lines:?}"
    );
    let rec = read_json(&record);
    assert_eq!(rec.at("preset").str(), Some("default_preset"));
    assert_eq!(rec.at("approved_by").str(), Some("tester"));
    assert_eq!(rec.at("pinned_by").str(), Some("tester"));
    let knobs = rec.at("knobs");
    assert_eq!(knobs.at("levels").at("approved_by").str(), Some("tester"));
    assert_eq!(knobs.at("input_len").at("source").str(), Some("agent"));
    assert_eq!(
        knobs.at("input_len").at("reason").str(),
        Some("prior_cell_shape")
    );
    assert_eq!(knobs.at("output_len").at("source").str(), Some("cli_pin"));
    for k in ["num_prompts", "warmups"] {
        assert_eq!(knobs.at(k).at("source").str(), Some("agent"), "{k}");
        assert_eq!(
            knobs.at(k).at("reason").str(),
            Some("preset_default"),
            "{k}"
        );
    }
    assert_eq!(knobs.at("repeats").at("source").str(), Some("cli_pin"));
    // without --approved-by the confirm-grade level is refused (a domain refusal, nothing recorded)
    std::fs::remove_file(&record).unwrap();
    let out = resolve(&confirm);
    assert_ne!(
        out.status.code(),
        Some(0),
        "confirm grade without --approved-by"
    );
    assert!(
        !usage_error(&out),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!record.exists());
    // --campaign-id: the campaign's budgets.repeats
    let lines = us_map(&ok(&resolve(&["--campaign-id", "cX"]), "--campaign-id"));
    assert_eq!(us_get(&lines, "REPEATS"), "5");
    assert!(
        us_get(&lines, "REPEATS_SOURCE").contains("campaigns/cX/campaign.yaml"),
        "{lines:?}"
    );
    assert_eq!(read_json(&record).at("campaign").str(), Some("cX"));
    // --dry-run: an absent serving yaml is allowed (otherwise a refusal)
    let absent = s.0.join("absent.yaml");
    let dry = |extra: &[&str]| {
        let mut args = base.to_vec();
        args[4] = ps(&absent);
        args.extend_from_slice(extra);
        Run::new(&s.0, "bench_preset.sh", &args)
            .env("BENCH_PRESET_REPO_ROOT", &repo)
            .env("CAMPAIGN_READ_BIN", &ci_bin)
            .out()
    };
    ok(
        &dry(&["--dry-run"]),
        "--dry-run with an absent serving yaml",
    );
    let out = dry(&[]);
    assert_ne!(
        out.status.code(),
        Some(0),
        "an absent serving yaml without --dry-run"
    );
    assert!(!usage_error(&out));
    // broad_search.sh: default --knob repeats --preset NAME
    let pd = ok(
        &Run::new(
            &s.0,
            "bench_preset.sh",
            &["default", "--knob", "repeats", "--preset", "default_preset"],
        )
        .env("BENCH_PRESET_REPO_ROOT", &repo)
        .out(),
        "default --preset",
    );
    assert_eq!(us_fields(&pd), ["3", "default_preset"]);
    let out = Run::new(
        &s.0,
        "bench_preset.sh",
        &[
            "default",
            "--knob",
            "repeats",
            "--preset",
            "contract-absent",
        ],
    )
    .env("BENCH_PRESET_REPO_ROOT", &repo)
    .out();
    assert_ne!(out.status.code(), Some(0), "an unknown preset name");
    assert!(!usage_error(&out));
}

/// broad_search.sh `repeat_axis resolve --preset-default --preset-source --campaign-id` reads that campaign's
/// `budgets.repeats` through the campaign launcher (`N\x1fSRC\x1fKIND` names the declaration); sweep_bench.sh
/// `record-run` writes the values it was given (`--run` is neither `--run-dir` nor `--run-bench-rc`) and
/// `aggregate-level --requested --requested-source --kind` records the request (`--requested` is not
/// `--requested-source`; `--kind cold-restart`, declared but not implemented, is a domain refusal).
#[test]
fn repeat_axis_consumer_options_by_argument_probe() {
    let s = scratch("repeat-probe");
    let repo = s.0.join("repo");
    let ci_bin = campaign_repo(&repo);
    let rr = ok(
        &Run::new(
            &s.0,
            "repeat_axis.sh",
            &[
                "resolve",
                "--preset-default",
                "3",
                "--preset-source",
                "p",
                "--campaign-id",
                "cX",
            ],
        )
        .env("REPEAT_AXIS_REPO_ROOT", &repo)
        .env("CAMPAIGN_READ_BIN", &ci_bin)
        .out(),
        "resolve --campaign-id",
    );
    let f = us_fields(&rr);
    assert_eq!(f[0], "5");
    assert!(f[1].contains("campaigns/cX/campaign.yaml"), "{rr:?}");
    let level = s.0.join("level_01");
    std::fs::create_dir_all(&level).unwrap();
    std::fs::copy(fixture("measured_pass.json"), level.join("measured.json")).unwrap();
    ok(
        &run(
            &s.0,
            "repeat_axis.sh",
            &[
                "record-run",
                "--run-dir",
                ps(&level),
                "--level",
                "1",
                "--run",
                "2",
                "--started-utc",
                "2026-09-04T09:00:00Z",
                "--ended-utc",
                "2026-09-04T09:01:00Z",
                "--run-bench-rc",
                "0",
                "--parse-rc",
                "0",
            ],
        ),
        "record-run",
    );
    let rec = read_json(&level.join("repeat_run.json"));
    assert_eq!(rec.at("level"), &Json::Num("1".into()));
    assert_eq!(rec.at("run"), &Json::Num("2".into()));
    assert_eq!(rec.at("run_bench_rc"), &Json::Num("0".into()));
    assert_eq!(rec.at("ended_utc").str(), Some("2026-09-04T09:01:00Z"));
    let aggregate = |kind: &str| {
        run(
            &s.0,
            "repeat_axis.sh",
            &[
                "aggregate-level",
                "--level-dir",
                ps(&level),
                "--requested",
                "4",
                "--requested-source",
                "declared(x)",
                "--kind",
                kind,
            ],
        )
    };
    // `--kind` is read: a declared but unimplemented kind is a domain refusal
    let out = aggregate("cold-restart");
    assert_ne!(
        out.status.code(),
        Some(0),
        "cold-restart is not implemented"
    );
    assert!(
        !usage_error(&out),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    ok(&aggregate("warm-rerun"), "aggregate-level");
    let m = read_json(&level.join("measured.json"));
    assert_eq!(m.at("repeats_requested"), &Json::Num("4".into()));
    assert_eq!(m.at("repeats_requested_source").str(), Some("declared(x)"));
    assert_eq!(m.at("repeat_kind").str(), Some("warm-rerun"));
}

/// broad_search.sh·sweep_bench.sh `classify_cell … --cross-node-tolerance-s N --cross-node-tolerance-source TEXT
/// [--lite-raw RAW]`: the declared tolerance and its source reach the events scan (`--cross-node-tolerance-s` is not
/// `--cross-node-tolerance-source`), and a given lite raw is judged (`lite_raw_status` is not `absent`).
#[test]
fn classify_cell_consumer_options_by_argument_probe() {
    let s = scratch("classify-probe");
    let repo = s.0.join("repo");
    let manifest = repo.join("output/multi/manifest.yaml");
    write(&manifest, "self_role: main\ntopology: multi\n");
    let cell = [
        "--serve-rc",
        "0",
        "--measure-rc",
        "0",
        "--started-utc",
        "2026-09-04T09:00:00Z",
        "--ended-utc",
        "2026-09-04T09:10:00Z",
        "--events-from-repo",
        ps(&repo),
        "--topology",
        "multi",
        "--manifest",
        ps(&manifest),
    ];
    let mut args = cell.to_vec();
    args.extend_from_slice(&[
        "--cross-node-tolerance-s",
        "7",
        "--cross-node-tolerance-source",
        "declared(contract)",
    ]);
    let cls = parse(
        &ok(
            &run(&s.0, "classify_cell.sh", &args),
            "cross-node tolerance",
        ),
        "classification",
    );
    let scan = cls.at("events_scan");
    assert_eq!(scan.at("cross_node_tolerance_s"), &Json::Num("7".into()));
    assert_eq!(
        scan.at("cross_node_tolerance_source").str(),
        Some("declared(contract)")
    );
    assert_eq!(
        parse(
            &ok(&run(&s.0, "classify_cell.sh", &cell), "no lite raw"),
            "c"
        )
        .at("lite_raw_status")
        .str(),
        Some("absent")
    );
    let raw = s.0.join("raw.json");
    std::fs::copy(fixture("lite_raw_d1_connection_refused.json"), &raw).unwrap();
    let mut args = cell.to_vec();
    args.extend_from_slice(&["--lite-raw", ps(&raw)]);
    let cls = parse(
        &ok(&run(&s.0, "classify_cell.sh", &args), "--lite-raw"),
        "classification",
    );
    assert_ne!(cls.at("lite_raw_status").str(), Some("absent"), "{cls:?}");
}

/// sweep_bench.sh `parse_guidellm --benchmarks-json … --max-error-rate R --post-health-json F --prefix-cache-json F`:
/// the declared error rate, the server liveness at the end of the bench and the prefix-cache observation reach the
/// measured JSON (`max_error_rate_declared`, `server_alive_at_bench_end`, `prefix_cache_run`).
#[test]
fn parse_guidellm_consumer_options_by_argument_probe() {
    let s = scratch("guidellm-probe");
    let health = s.0.join("post_health.json");
    write(&health, "{\"server_alive_at_bench_end\": false}");
    let cache = s.0.join("prefix_cache.json");
    write(&cache, "{\"provenance\": \"measured\", \"window_start_utc\": \"t\", \"queries_delta\": 10, \"hits_delta\": 1, \"requests_delta\": 5, \"hit_rate_run\": 0.1, \"reason\": null, \"source\": \"x\"}");
    let fx = fixture("guidellm_benchmarks_sample.json");
    let args = [
        "--benchmarks-json",
        ps(&fx),
        "--spec-axis-absent",
        "--max-error-rate",
        "0.25",
        "--post-health-json",
        ps(&health),
        "--prefix-cache-json",
        ps(&cache),
    ];
    let doc = parse(
        &ok(
            &run(&s.0, "parse_guidellm.sh", &args),
            "parse_guidellm with the sweep inputs",
        ),
        "measured",
    );
    assert_eq!(doc.at("max_error_rate_declared"), &Json::Num("0.25".into()));
    assert_eq!(doc.at("server_alive_at_bench_end"), &Json::Bool(false));
    let pc = doc.at("prefix_cache_run");
    assert_eq!(pc.at("hits_delta"), &Json::Num("1".into()));
    assert_eq!(pc.at("provenance").str(), Some("measured"));
}

/// judge_bench.sh `roofline --model-path --json [--tp --manifest --nas-root --quant-root --realistic-fraction
/// --accept-len] --accept-len-source --accept-len-evidence`: `--realistic-fraction` is the fraction used, `--manifest`
/// gives the TP when `--tp` is absent, `--nas-root` resolves a container model path, `--accept-len` (not
/// `--accept-len-source`) is the length used; the parser takes `--quant-root`.
#[test]
fn roofline_consumer_options_by_argument_probe() {
    let s = scratch("roofline-probe");
    let model = s.0.join("nas/contract-m1");
    model_dir(&model);
    let tail = [
        "--json",
        "--bandwidth-gbps",
        "273",
        "--accept-len-source",
        "absent",
        "--accept-len-evidence",
        "fixture",
    ];
    let roof = |args: &[&str]| {
        let mut all = args.to_vec();
        all.extend_from_slice(&tail);
        parse(
            &ok(
                &run(&s.0, "roofline.sh", &all),
                &format!("roofline {args:?}"),
            ),
            "roofline",
        )
    };
    let doc = roof(&[
        "--model-path",
        ps(&model),
        "--tp",
        "1",
        "--realistic-fraction",
        "0.5",
    ]);
    assert_eq!(doc.at("realistic_fraction"), &Json::Num("0.5".into()));
    let manifest = s.0.join("manifest.yaml");
    write(&manifest, "topology: single\ngpus_per_node: 2\n");
    let doc = roof(&["--model-path", ps(&model), "--manifest", ps(&manifest)]);
    assert_eq!(doc.at("tp"), &Json::Num("2".into()));
    assert!(
        doc.at("tp_source").str().unwrap().starts_with("manifest("),
        "{doc:?}"
    );
    let doc = roof(&[
        "--model-path",
        "/app/models/contract-m1",
        "--nas-root",
        ps(&s.0.join("nas")),
        "--tp",
        "1",
    ]);
    assert_eq!(doc.at("model_path").str(), Some("/app/models/contract-m1"));
    let declared = [
        "--model-path",
        ps(&model),
        "--json",
        "--bandwidth-gbps",
        "273",
        "--tp",
        "1",
        "--accept-len",
        "1.5",
        "--accept-len-source",
        "declared",
        "--accept-len-evidence",
        "x",
    ];
    assert_eq!(
        parse(
            &ok(&run(&s.0, "roofline.sh", &declared), "--accept-len"),
            "r"
        )
        .at("accept_len"),
        &Json::Num("1.5".into())
    );
    let quant_root = s.0.join("quant");
    let quant = [
        "--model-path",
        ps(&model),
        "--tp",
        "1",
        "--quant-root",
        ps(&quant_root),
        "--json",
        "--bandwidth-gbps",
        "273",
        "--accept-len-source",
        "absent",
        "--accept-len-evidence",
        "fixture",
    ];
    assert_parser_takes(&s.0, "roofline.sh", &quant, 4, &[]);
}

/// judge_bench.sh `verdict_rule --measured --roofline --authority --spec-declared [--target-tps --reference-tps --e-search
/// --tolerance --node-vram-gib --spec-supported]`: `--reference-tps`, `--target-tps` and `--tolerance` reach the rubric
/// (`reference_E`, `target_c`, `tolerance`); the parser takes `--node-vram-gib` and `--spec-supported`.
#[test]
fn verdict_rule_consumer_options_by_argument_probe() {
    let s = scratch("verdict-probe");
    let m = fixture("measured_pass.json");
    let r = fixture("roofline_sample.json");
    let base = [
        "--measured",
        ps(&m),
        "--roofline",
        ps(&r),
        "--spec-declared",
        "off",
    ];
    let verdict = |extra: &[&str]| {
        let mut args = base.to_vec();
        args.extend_from_slice(extra);
        parse(
            &ok(
                &run(&s.0, "verdict_rule.sh", &args),
                &format!("verdict {extra:?}"),
            ),
            "verdict",
        )
    };
    let doc = verdict(&[
        "--authority",
        "weak",
        "--reference-tps",
        "50",
        "--tolerance",
        "0.2",
    ]);
    assert_eq!(
        doc.at("rubric").at("reference_E"),
        &Json::Num("50.0".into())
    );
    assert_eq!(doc.at("rubric").at("tolerance"), &Json::Num("0.2".into()));
    let doc = verdict(&["--authority", "explicit", "--target-tps", "20"]);
    assert_eq!(doc.at("rubric").at("target_c"), &Json::Num("20.0".into()));
    let mut args = base.to_vec();
    args.extend_from_slice(&["--authority", "weak", "--node-vram-gib", "100,100"]);
    let at = args.len() - 2;
    assert_parser_takes(&s.0, "verdict_rule.sh", &args, at, &[]);
    let mut args = base.to_vec();
    args.extend_from_slice(&["--authority", "weak", "--spec-supported"]);
    let at = args.len() - 1;
    assert_parser_takes(&s.0, "verdict_rule.sh", &args, at, &[]);
}

/// run_bench.sh `resolve_bench_tool --tool guidellm --tool-version V --json`: a recorded version is the answer's
/// `version` (status absent = exit 3 here: the engine stand-in has no image); a version the record lacks is a domain
/// refusal, not a usage error.
#[test]
fn resolve_bench_tool_consumer_options_by_argument_probe() {
    let s = scratch("bench-tool-probe");
    let pin = s.0.join("bench_tool_pin.json");
    std::fs::copy(skill().join("bench_tool_pin.json"), &pin).unwrap();
    let bin = s.0.join("bin");
    let engine = bin.join("docker");
    write(
        &engine,
        "#!/bin/sh\necho 'Error: No such image' >&2\nexit 1\n",
    );
    std::fs::set_permissions(&engine, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let path = std::ffi::OsString::from(format!(
        "{}:{}",
        ps(&bin),
        std::env::var("PATH").unwrap_or_default()
    ));
    let version = read_json(&pin)
        .at("tools")
        .at("guidellm")
        .at("default_version")
        .str()
        .unwrap()
        .to_string();
    let out = Run::new(
        &s.0,
        "resolve_bench_tool.sh",
        &[
            "--tool",
            "guidellm",
            "--tool-version",
            &version,
            "--pin",
            ps(&pin),
            "--json",
        ],
    )
    .env("PATH", &path)
    .out();
    assert_eq!(out.status.code(), Some(3), "{}", text(&out.stderr));
    assert_eq!(
        parse(&text(&out.stdout), "absent").at("version").str(),
        Some(version.as_str())
    );
    let args = [
        "--tool",
        "guidellm",
        "--tool-version",
        "0.0.0-contract-absent",
        "--pin",
        ps(&pin),
        "--json",
    ];
    let out = Run::new(&s.0, "resolve_bench_tool.sh", &args)
        .env("PATH", &path)
        .out();
    assert_ne!(out.status.code(), Some(0), "an unrecorded version");
    let path_dir = std::path::PathBuf::from(&path);
    assert_parser_takes(
        &s.0,
        "resolve_bench_tool.sh",
        &args,
        2,
        &[("PATH", &path_dir)],
    );
}

/// run_bench.sh `dataset_prep slice --spec --cache --run-index --sweep-salt --count --input-len --output-len --out
/// --sidecar --tokenizer-label` on a prepared cache writes the rows to `--out` (not `--output-len`) and the sidecar
/// with the values it was given; `prepare --spec --tokstage --cache --image --budget-mib --input-len --output-len` takes
/// every option (an absent dataset file is its domain refusal; the tokenizer image is never started).
#[test]
fn dataset_prep_prepare_and_slice_by_argument_probe() {
    let s = scratch("dataset-probe");
    let spec = s.0.join("spec.json");
    write(
        &spec,
        "{\"dataset_kind\": \"json_file\", \"seed\": 7, \"dataset_path\": \"absent.jsonl\"}",
    );
    let cache = s.0.join("cache");
    write(
        &cache.join("prepared_meta.json"),
        "{\"rows\": 3, \"key\": {\"tokenizer_digest\": \"d0\"}}",
    );
    let rows: String = (1..=3).map(|i| format!("{{\"prompt\": \"p{i}\", \"prompt_tokens_count\": 128, \"output_tokens_count\": 64}}\n")).collect();
    write(&cache.join("prepared.jsonl"), &rows);
    let (out_rows, side) = (s.0.join("rows.jsonl"), s.0.join("side.json"));
    let args = [
        "slice",
        "--spec",
        ps(&spec),
        "--cache",
        ps(&cache),
        "--run-index",
        "0",
        "--sweep-salt",
        "s1",
        "--count",
        "2",
        "--input-len",
        "128",
        "--output-len",
        "64",
        "--out",
        ps(&out_rows),
        "--sidecar",
        ps(&side),
        "--tokenizer-label",
        "tok-x",
    ];
    ok(&run(&s.0, "dataset_prep.sh", &args), "slice");
    assert_eq!(
        std::fs::read_to_string(&out_rows).unwrap().lines().count(),
        2
    );
    let d = read_json(&side);
    for (k, v) in [
        ("input_len", "128"),
        ("output_len", "64"),
        ("run_index", "0"),
        ("rows_used", "2"),
    ] {
        assert_eq!(d.at(k), &Json::Num(v.into()), "{k}");
    }
    assert_eq!(d.at("sweep_salt").str(), Some("s1"));
    assert_eq!(d.at("tokenizer").str(), Some("tok-x"));
    let tok = s.0.join("tokstage");
    std::fs::create_dir_all(&tok).unwrap();
    let cache2 = s.0.join("cache2");
    let prepare = [
        "prepare",
        "--spec",
        ps(&spec),
        "--tokstage",
        ps(&tok),
        "--cache",
        ps(&cache2),
        "--image",
        "contract-image:1",
        "--budget-mib",
        "1024",
        "--input-len",
        "128",
        "--output-len",
        "64",
    ];
    for at in (1..prepare.len()).step_by(2) {
        assert_parser_takes(&s.0, "dataset_prep.sh", &prepare, at, &[]);
    }
}

/// A text argument that is not UTF-8 is a usage error (exit 2, nothing on stdout), never a crash (types.md 2절).
#[test]
fn non_utf8_text_argument_is_exit_2() {
    let s = scratch("nonutf8");
    let out = Run::new(
        &s.0,
        "verdict_rule.sh",
        &[
            "--measured",
            ps(&fixture("measured_pass.json")),
            "--roofline",
            ps(&fixture("roofline_sample.json")),
            "--authority",
        ],
    )
    .arg(OsStr::from_bytes(b"we\xffak"))
    .out();
    assert_eq!(out.status.code(), Some(2));
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
        /// Compact JSON text of the value (strings escaped as RFC 8259 requires, numbers as read).
        pub fn dump(&self) -> String {
            match self {
                Json::Null => "null".into(),
                Json::Bool(b) => b.to_string(),
                Json::Num(n) => n.clone(),
                Json::Str(s) => {
                    let mut o = String::from("\"");
                    for c in s.chars() {
                        match c {
                            '"' => o.push_str("\\\""),
                            '\\' => o.push_str("\\\\"),
                            '\n' => o.push_str("\\n"),
                            '\r' => o.push_str("\\r"),
                            '\t' => o.push_str("\\t"),
                            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
                            c => o.push(c),
                        }
                    }
                    o.push('"');
                    o
                }
                Json::Arr(a) => format!(
                    "[{}]",
                    a.iter().map(Json::dump).collect::<Vec<_>>().join(", ")
                ),
                Json::Obj(m) => format!(
                    "{{{}}}",
                    m.iter()
                        .map(|(k, v)| format!("{}: {}", Json::Str(k.clone()).dump(), v.dump()))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            }
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
