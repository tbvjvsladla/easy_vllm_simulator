//! Contract checks of `runtime_selftest`, `antipattern_scan` and `constitution_lint` (plan 26111805 「보존 계약」, cell
//! c21c-contract-tests): the surface their consumers read (runtime-support/contract/consumers.tsv rows of these bins) —
//! - the pre-commit hook and policy-gate `completion_gate.rs` run `runtime_selftest.sh --tripwires-only` in the
//!   repository root: exit 0 passes, non-zero refuses; the last non-empty stderr line is the refusal reason
//!   (`[tripwire] FAIL …`); harness_verify (stage 2) and policy-distribution run `runtime_selftest.sh` without arguments
//!   (the regression suite) and read exit 0 / non-zero;
//! - policy-selftest `tripwires.rs` (tripwire ⑥) runs `antipattern_scan --root <root> --tripwire` and reads the exit code
//!   (0 / 3) and the stderr lines (`[antipattern_scan] SURFACE …`, `[antipattern_scan] REVIVED <shape> — <file>:<line>`);
//!   the input is terraforming_node SKILL.md `## 4. 공개 표면` (heading and table rows L1); policy-distribution runs
//!   `antipattern_scan.sh --self-test` and `constitution_lint.sh --self-test` (exit 0);
//! - closing_sequence.sh runs `constitution_lint.sh --repo <repo>` (exit ignored, stdout shown); `--format json|text`;
//! - the ledger, harness_verify.sh, pure_goldens and the documents name the launchers; `RUNTIME_SELFTEST_BIN`,
//!   `ANTIPATTERN_SCAN_BIN`, `CONSTITUTION_LINT_BIN` override the executables.
//!
//! The bins run for real through their launchers. The tripwires run in scratch git repositories through a copy of the
//! launcher (argv[0] locates the repository it judges), never on the real checkout, whose refs differ between M and W.
//! Exit codes, line prefixes, shape ids and option names are L1; the JSON report of constitution_lint is L2; message
//! texts and help are L3.
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
        "policy-selftest-contract-{tag}-{}-{n}",
        std::process::id()
    ));
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

/// `git <args>` in `dir` with hooks and global configuration out of the way.
fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "user.name=contract",
            "-c",
            "user.email=contract",
        ])
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {}", text(&out.stderr));
}

const RUNTIME_REL: &str = ".claude/policies/runtime/runtime_selftest.sh";
const ANTIPATTERN_REL: &str = ".claude/policies/runtime/antipattern_scan.sh";
const LINT_REL: &str = ".claude/policies/runtime/constitution_lint.sh";

fn bin_of(rel: &str) -> (&'static str, &'static str) {
    match rel {
        RUNTIME_REL => (
            "RUNTIME_SELFTEST_BIN",
            env!("CARGO_BIN_EXE_runtime_selftest"),
        ),
        ANTIPATTERN_REL => (
            "ANTIPATTERN_SCAN_BIN",
            env!("CARGO_BIN_EXE_antipattern_scan"),
        ),
        _ => (
            "CONSTITUTION_LINT_BIN",
            env!("CARGO_BIN_EXE_constitution_lint"),
        ),
    }
}

/// `bash <launcher> <args>` (the launcher at `launcher`, a repository's or a scratch copy) with the override = this build.
fn launch(launcher: &Path, rel: &str, cwd: &Path, args: &[&std::ffi::OsStr]) -> Output {
    let (var, bin) = bin_of(rel);
    Command::new("bash")
        .arg(launcher)
        .args(args)
        .env(var, bin)
        .current_dir(cwd)
        .output()
        .unwrap()
}

fn os(args: &[&str]) -> Vec<std::ffi::OsString> {
    args.iter().map(std::ffi::OsString::from).collect()
}

fn run(rel: &str, cwd: &Path, args: &[&str]) -> Output {
    let a = os(args);
    let refs: Vec<&std::ffi::OsStr> = a.iter().map(|x| x.as_os_str()).collect();
    launch(&root().join(rel), rel, cwd, &refs)
}

/// A canonical scratch repository (the three markers, a git work tree on `single-node`) holding a copy of the
/// runtime_selftest launcher, so the program judges this tree.
fn canonical_repo(tag: &str) -> Scratch {
    let s = scratch(tag);
    let l = s.0.join(RUNTIME_REL);
    std::fs::create_dir_all(l.parent().unwrap()).unwrap();
    std::fs::copy(root().join(RUNTIME_REL), &l).unwrap();
    write(&s.0.join("CLAUDE.md"), "# c\n");
    write(&s.0.join(".claude/rules/workflow.md"), "# w\n");
    write(
        &s.0.join(".claude/policies/registry.yaml"),
        "{\"schema_version\": 2, \"policies\": []}\n",
    );
    git(&s.0, &["init", "-q", "-b", "single-node"]);
    s
}

fn tripwires(s: &Scratch) -> Output {
    let a = os(&["--tripwires-only"]);
    launch(
        &s.0.join(RUNTIME_REL),
        RUNTIME_REL,
        &s.0,
        &[a[0].as_os_str()],
    )
}

fn last_line(s: &str) -> &str {
    s.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("")
}

/// The three launchers are the files the documents, the ledger and the harness name; the override variable selects
/// the executable; an absent one is exit 2 with nothing on stdout; `--help` is exit 0.
#[test]
fn launchers_override_and_absent_executable() {
    let s = scratch("launchers");
    for rel in [RUNTIME_REL, ANTIPATTERN_REL, LINT_REL] {
        let launcher = root().join(rel);
        assert!(launcher.is_file(), "{rel}");
        let (var, _) = bin_of(rel);
        let out = Command::new("bash")
            .arg(&launcher)
            .arg("--help")
            .env(var, "/nonexistent/bin")
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(2), "{rel}");
        assert_eq!(text(&out.stdout), "", "{rel}");
        let out = run(rel, &s.0, &["--help"]);
        assert_eq!(out.status.code(), Some(0), "{rel}");
    }
    let out = run(RUNTIME_REL, &s.0, &["--tripwires-only", "-h"]);
    assert_eq!(out.status.code(), Some(0));
}

/// pre-commit / completion_gate: a failing tripwire is a non-zero exit (1) with nothing on stdout, and the last
/// non-empty stderr line is the `[tripwire] FAIL …` reason — a backup artifact in the work tree, a stray branch.
#[test]
fn tripwire_failure_is_nonzero_with_the_reason_last_on_stderr() {
    let s = canonical_repo("bak");
    write(&s.0.join("notes.bak"), "x\n");
    let out = tripwires(&s);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), "");
    assert!(
        last_line(&text(&out.stderr)).starts_with("[tripwire] FAIL "),
        "{}",
        text(&out.stderr)
    );

    let s = canonical_repo("branch");
    git(&s.0, &["add", "-A"]);
    git(&s.0, &["commit", "-q", "-m", "init"]);
    git(&s.0, &["branch", "stray"]);
    let out = tripwires(&s);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), "");
    let err = text(&out.stderr);
    assert!(
        last_line(&err).starts_with("[tripwire] FAIL ") && last_line(&err).contains("stray"),
        "{err}"
    );
}

/// harness_verify stage 2 / policy-distribution: the regression suite (no arguments) answers with a non-zero exit when
/// the tree it judges is not a full checkout (fail closed, never a silent pass).
#[test]
fn regression_suite_fails_closed_outside_a_checkout() {
    let s = scratch("suite");
    let l = s.0.join(RUNTIME_REL);
    std::fs::create_dir_all(l.parent().unwrap()).unwrap();
    std::fs::copy(root().join(RUNTIME_REL), &l).unwrap();
    let out = launch(&l, RUNTIME_REL, &s.0, &[]);
    assert_eq!(out.status.code(), Some(1));
}

/// Usage errors are exit 2 with nothing on stdout, including a non-UTF-8 argument (types.md 2절).
#[test]
fn usage_errors_and_non_utf8_arguments_are_exit_2() {
    use std::os::unix::ffi::OsStrExt;
    let s = scratch("usage");
    let bad = std::ffi::OsStr::from_bytes(b"--tripwires-only\xff");
    for rel in [RUNTIME_REL, ANTIPATTERN_REL, LINT_REL] {
        for args in [vec![std::ffi::OsStr::new("--bogus")], vec![bad]] {
            let out = launch(&root().join(rel), rel, &s.0, &args);
            assert_eq!(out.status.code(), Some(2), "{rel} {args:?}");
            assert_eq!(text(&out.stdout), "", "{rel} {args:?}");
        }
    }
    let out = run(LINT_REL, &s.0, &["--format", "yaml"]);
    assert_eq!(out.status.code(), Some(2));
    let out = run(ANTIPATTERN_REL, &s.0, &[]);
    assert_eq!(out.status.code(), Some(2), "no mode is a usage answer");
}

/// The lines of a report that carry the PASS marker: the word `PASS` with no ASCII letter, digit or `_` on either side.
fn pass_lines(report: &str) -> usize {
    let part = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    report
        .lines()
        .filter(|line| {
            let b = line.as_bytes();
            line.match_indices("PASS").any(|(at, _)| {
                let end = at + 4;
                (at == 0 || !part(b[at - 1])) && (end == b.len() || !part(b[end]))
            })
        })
        .count()
}

/// policy-distribution: `antipattern_scan.sh --self-test` and `constitution_lint.sh --self-test` are exit 0, with a
/// lower bound on their reports (review r2 P-T5: an exit 0 of a self-test that ran no check fails): antipattern_scan
/// prints one `[PASS]` line per check on stdout (16 in the base, 2026-11-19); constitution_lint reports no count, only
/// its `[constitution_lint] self-test PASS` line on stderr, which is the bound. The PASS marker is fixed, the wording
/// around it is L3.
#[test]
fn self_tests_pass() {
    let r = root();
    for (rel, min_stdout, min_stderr) in [(ANTIPATTERN_REL, 16, 0), (LINT_REL, 0, 1)] {
        let out = run(rel, &r, &["--self-test"]);
        let (so, se) = (text(&out.stdout), text(&out.stderr));
        assert_eq!(
            out.status.code(),
            Some(0),
            "{rel}\nstdout:\n{so}\nstderr:\n{se}"
        );
        assert!(
            pass_lines(&so) >= min_stdout && pass_lines(&se) >= min_stderr,
            "{rel}: PASS lines (stdout {}, stderr {}) below ({min_stdout}, {min_stderr})\nstdout:\n{so}\nstderr:\n{se}",
            pass_lines(&so),
            pass_lines(&se)
        );
    }
}

/// The owner skill directory whose SKILL.md §4 lists the public surface. Written in two pieces so that this tracked
/// file does not itself carry a `<skill>/scripts/<x>` reference the real tripwire ⑥ would judge.
const SURFACE_SKILL: &str = concat!(".claude/skills/terraforming", "_node");

/// `bash <owner skill>/scripts/<name>` — a caller line as tripwire ⑥ reads it.
fn caller_line(name: &str) -> String {
    format!("bash {SURFACE_SKILL}/{}/{name}\n", "scripts")
}

/// A scratch git repository whose owner SKILL.md carries the `## 4. 공개 표면` table listing `scripts/a.sh`.
fn surface_repo(tag: &str) -> Scratch {
    let s = scratch(tag);
    write(
        &s.0.join(SURFACE_SKILL).join("SKILL.md"),
        "# t\n## 4. 공개 표면 — 전수\n\n| 진입점 | 소유 |\n|---|---|\n| `scripts/a.sh` | x |\n\n## 5. 금지\n",
    );
    write(&s.0.join("ok.sh"), &caller_line("a.sh"));
    git(&s.0, &["init", "-q", "-b", "single-node"]);
    git(&s.0, &["add", "-A"]);
    s
}

fn scan(s: &Scratch, extra: &[&str]) -> Output {
    let mut args = vec!["--root", s.0.to_str().unwrap(), "--tripwire"];
    args.extend_from_slice(extra);
    run(ANTIPATTERN_REL, &s.0, &args)
}

/// Tripwire ⑥: `--root R --tripwire` is exit 0 on a clean tree; a call outside the §4 table is exit 3 with a
/// `[antipattern_scan] SURFACE ` line; a revived shape among the staged files (or the `--files` given) is exit 3 with a
/// `[antipattern_scan] REVIVED <shape> — <file>:<line>` line; the diagnostics are on stderr, stdout stays empty.
#[test]
fn antipattern_tripwire_exit_codes_and_stderr_lines() {
    let s = surface_repo("clean");
    let out = scan(&s, &[]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), "");

    write(&s.0.join("bad.sh"), &caller_line("b.sh"));
    git(&s.0, &["add", "-A"]);
    let out = scan(&s, &[]);
    assert_eq!(out.status.code(), Some(3));
    assert_eq!(text(&out.stdout), "");
    let err = text(&out.stderr);
    assert!(
        err.lines()
            .any(|l| l.starts_with("[antipattern_scan] SURFACE ") && l.contains("scripts/b.sh")),
        "{err}"
    );

    let s = surface_repo("revived");
    let rel = ".claude/skills/adversarial-benchmark/rust/src/bench_tool_pin.rs";
    write(&s.0.join(rel), "pub const RC_DRIFT: i32 = 4;\n");
    let out = scan(&s, &["--files", rel]);
    assert_eq!(out.status.code(), Some(3));
    assert_eq!(text(&out.stdout), "");
    let err = text(&out.stderr);
    assert!(
        err.lines().any(|l| l.starts_with(&format!(
            "[antipattern_scan] REVIVED G-C3-bench-drift-gate — {rel}:1"
        ))),
        "{err}"
    );
    let out = scan(&s, &["--files", "ok.sh"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "only the named files are scanned"
    );
    git(&s.0, &["add", "-A"]);
    let out = scan(&s, &[]);
    assert_eq!(
        out.status.code(),
        Some(3),
        "without --files the staged files are scanned"
    );

    let s = scratch("nosurface");
    git(&s.0, &["init", "-q", "-b", "single-node"]);
    let out = scan(&s, &[]);
    assert_eq!(
        out.status.code(),
        Some(3),
        "a missing SKILL.md §4 table fails the tripwire"
    );
    assert!(text(&out.stderr).starts_with("[antipattern_scan] SURFACE "));
}

/// `--root R --inventory OUT.jsonl --files F…`: exit 0 and one JSON object per line with `file`, `line`, `cat` and a
/// `disposition` from the closed vocabulary.
#[test]
fn antipattern_inventory_rows() {
    let s = surface_repo("inventory");
    write(&s.0.join("img.sh"), "IMAGE=nvcr.io/nvidia/vllm:1.2.3\n");
    let inv = s.0.join("inv.jsonl");
    let out = run(
        ANTIPATTERN_REL,
        &s.0,
        &[
            "--root",
            s.0.to_str().unwrap(),
            "--inventory",
            inv.to_str().unwrap(),
            "--files",
            "img.sh",
        ],
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let body = std::fs::read_to_string(&inv).unwrap();
    assert!(!body.is_empty());
    for line in body.lines() {
        let row = Json::parse(line).unwrap();
        assert_eq!(row.at("file").str(), Some("img.sh"));
        assert_eq!(row.at("line").kind(), "number");
        assert_eq!(row.at("cat").kind(), "string");
        assert!(
            ["fixture", "diagnostic", "local-const", "review"]
                .contains(&row.at("disposition").str().unwrap()),
            "{line}"
        );
    }
}

/// closing_sequence.sh: `constitution_lint.sh --repo R` is exit 0 with the report on stdout; `--format json` prints one
/// JSON object (`schema_version`, `verdict` = `INFO`, `duplicate_sentences`, `always_loaded`, `ghost_policy_ids`,
/// `skill_router`); a tree without CLAUDE.md is exit 2 with nothing on stdout.
#[test]
fn constitution_lint_report_and_exit_codes() {
    let s = scratch("lint");
    write(&s.0.join("CLAUDE.md"), "# c\n");
    write(
        &s.0.join(".claude/policies/registry.yaml"),
        "{\"schema_version\": 2, \"policies\": []}\n",
    );
    let repo = s.0.to_str().unwrap();
    let out = run(LINT_REL, &s.0, &["--repo", repo]);
    assert_eq!(out.status.code(), Some(0));
    assert!(!text(&out.stdout).is_empty());
    let out = run(LINT_REL, &s.0, &["--repo", repo, "--format", "text"]);
    assert_eq!(out.status.code(), Some(0));
    let out = run(LINT_REL, &s.0, &["--repo", repo, "--format", "json"]);
    assert_eq!(out.status.code(), Some(0));
    let doc = Json::parse(&text(&out.stdout)).unwrap();
    assert_eq!(doc.at("schema_version").kind(), "number");
    assert_eq!(doc.at("verdict").str(), Some("INFO"));
    assert_eq!(doc.at("duplicate_sentences").kind(), "array");
    assert_eq!(doc.at("always_loaded").at("total_bytes").kind(), "number");
    assert_eq!(doc.at("always_loaded").at("files").kind(), "object");
    assert_eq!(doc.at("ghost_policy_ids").kind(), "array");
    assert_eq!(doc.at("skill_router").kind(), "object");
    let missing = s.0.join("nope");
    let out = run(LINT_REL, &s.0, &["--repo", missing.to_str().unwrap()]);
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
