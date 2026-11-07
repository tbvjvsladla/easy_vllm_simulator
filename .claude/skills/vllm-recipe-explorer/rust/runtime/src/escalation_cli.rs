//! The command line of `escalation_predicate.py` (its `main` and `_self_test`).
//!
//! `escalation_predicate --symptom KIND... [--events PATH]... [--started-utc T --ended-utc T] [--tolerance-s N] [--json]` judges predicate
//! one; `--self-test` runs the nine checks. Exit status 0 = predicate one holds, 1 = it does not (or an exception nothing handles), 2 =
//! argument error. The order is the original's: parse, self-test, the events-need-an-interval guard, load the classifier, read the
//! event files (a file that cannot be opened is a warning and the run goes on; a file that is not UTF-8 is an uncaught exception), then
//! `_utc(start)`, `_utc(end)`, the kill-window call and the judgment (which loads the classifier again), then the report. The classifier
//! is the existing Rust owner reached through its launcher ([`crate::escalation_bridge`]); no Python runs.
//!
//! Standard error and output follow the stdio rules of the memory owner: a `print(file=sys.stderr)` falls back to standard output when
//! standard error was closed at start (and a write failure stops the run), the parser's own writes and the uncaught report do not.

use crate::cli::{emit, Run};
use crate::cliparse::{
    format_usage, parse as parse_arguments, terminal_width, Kind, Opt, Outcome, Spec, Value,
};
use crate::escalation::{judge, Dependency, Raised, Vocabulary};
use crate::escalation_bridge::Bridge;
use crate::memory::open_text_file;
use crate::memory_cli::{print_to_stream, split_usage, write_real_stderr};
use crate::memory_system::{startup_stdio, StdioAtStart};
use crate::model_cli::Streams;
use crate::pyexpr::Res;
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pyjson::decode_file_text;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use num_bigint::BigInt;
use std::io::Read;

/// The places the command line writes, and the classifier it reaches.
pub trait CliHost: Dependency {
    /// `print(text, file=sys.stderr)`: the text ends with its newline.
    fn print_stderr(&mut self, text: &PyStr) -> Res<()>;
    /// `sys.stderr.write(text)` of a transport record's own standard error.
    fn write_stderr(&mut self, text: &PyStr) -> Res<()>;
    /// `sys.stdout.write(text)`.
    fn write_stdout(&mut self, text: &PyStr);
    /// `print(text)`: the text and then its newline, as two writes.
    fn print_stdout(&mut self, text: &PyStr) {
        self.write_stdout(text);
        self.write_stdout(&PyStr::new("\n"));
    }
    /// `argparse.error()`: usage to `sys.stderr` (to `sys.stdout` when that is `None`), then the message to `sys.stderr`
    /// (dropped when `None`); write failures are suppressed.
    fn parser_error(&mut self, usage: &PyStr, message: &PyStr);
    /// `print_help()`: standard output, else standard error, else nowhere; failures suppressed.
    fn parser_help(&mut self, help: &PyStr);
    /// The interpreter's report of an exception nothing handled: standard error only, a failure to write it ignored.
    fn write_uncaught(&mut self, text: &PyStr);
}

fn opt(
    flags: &'static [&'static str],
    dest: &'static str,
    kind: Kind,
    help: Option<&'static str>,
) -> Opt {
    Opt {
        flags,
        dest,
        kind,
        help,
        required: false,
        group: None,
    }
}

/// The parser of the original `main`.
pub fn spec() -> Spec {
    Spec {
        prog: "escalation_predicate.py",
        description: "escalation 술어 ① 판정 (§5.5 · plan_26091108 R7)",
        options: vec![
            opt(&["-h", "--help"], "help", Kind::Help, None),
            opt(
                &["--symptom"],
                "symptom",
                Kind::TextAppend,
                Some("관측된 증상 kind (반복 가능)"),
            ),
            opt(
                &["--events"],
                "events",
                Kind::TextAppend,
                Some("노드 블랙박스 events jsonl"),
            ),
            opt(&["--started-utc"], "started_utc", Kind::Text, None),
            opt(&["--ended-utc"], "ended_utc", Kind::Text, None),
            opt(&["--tolerance-s"], "tolerance_s", Kind::Int, None),
            opt(&["--json"], "json", Kind::Flag, None),
            opt(&["--self-test"], "self_test", Kind::Flag, None),
        ],
        group_required: false,
        positional: None,
    }
}

fn texts(values: &std::collections::BTreeMap<&'static str, Value>, dest: &str) -> Vec<PyStr> {
    match values.get(dest) {
        Some(Value::Texts(list)) => list.clone(),
        _ => Vec::new(),
    }
}

fn text(values: &std::collections::BTreeMap<&'static str, Value>, dest: &str) -> PyStr {
    match values.get(dest) {
        Some(Value::Text(text)) => text.clone(),
        _ => PyStr::default(),
    }
}

/// `open(path, encoding="utf-8").readlines()`: strict UTF-8, universal newlines, each line with its newline.
fn read_lines(path: &PyStr) -> Result<Vec<PyStr>, Raised> {
    let mut file = open_text_file(path).map_err(Raised::from)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|error| Raised::from(crate::model_files::os_failure(&error, None)))?;
    let text = decode_file_text(&bytes)
        .map_err(|error| Raised::from(crate::model_files::load_failure(error)))?;
    let newline = u32::from('\n');
    let mut lines = Vec::new();
    let mut start = 0;
    let points = text.code_points();
    for (at, point) in points.iter().enumerate() {
        if *point == newline {
            lines.push(PyStr::from_code_points(points[start..=at].to_vec()));
            start = at + 1;
        }
    }
    if start < points.len() {
        lines.push(PyStr::from_code_points(points[start..].to_vec()));
    }
    Ok(lines)
}

/// What the dependency wrote to standard error is written to it as it came.
fn forward_stderr(host: &mut dyn CliHost) -> Result<(), Raised> {
    for piece in host.take_stderr() {
        host.write_stderr(&piece)?;
    }
    Ok(())
}

fn failure(text: &str) -> PyStr {
    PyStr::new(text)
}

/// How the program ended without an exception: the exit status, and (for an argument that `argparse` could not convert) the exceptions
/// `argparse` was handling, which a `SystemExit` raised then has as its implicit context.
pub struct Exit {
    pub status: i32,
    pub context: Option<Raised>,
}

/// The `argparse` exceptions behind an `invalid int value` usage error: `ArgumentError` raised for the `ValueError` of `int()`.
fn conversion_context(message: &PyStr) -> Option<Raised> {
    let text = message.to_utf8()?;
    let line = text.trim_end_matches('\n');
    let detail = line.rsplit_once(": error: ")?.1;
    let (_, shown) = detail.split_once(": invalid int value: ")?;
    let conversion = Raised::plain(
        "ValueError",
        &format!("invalid literal for int() with base 10: {shown}"),
    );
    let mut argument = Raised::new("ArgumentError", "argparse", PyStr::new(detail));
    argument.cause = Some(Box::new(conversion));
    argument.via_context = true;
    Some(argument)
}

/// `main()`: how it ends, or the exception it ends with.
pub fn execute(
    host: &mut dyn CliHost,
    vocabulary: &Vocabulary,
    args: &[PyStr],
    columns: Option<&str>,
) -> Result<Exit, Raised> {
    let parser = spec();
    let values = match parse_arguments(&parser, args, columns) {
        Outcome::Values(values) => values,
        Outcome::Exit {
            status,
            stdout,
            stderr,
        } => {
            if !stdout.is_empty() {
                host.parser_help(&stdout);
            }
            if !stderr.is_empty() {
                let usage = PyStr::new(&format_usage(&parser, terminal_width(columns)));
                let (usage, message) = split_usage(&usage, &stderr);
                host.parser_error(&usage, &message);
            }
            let context = conversion_context(&stderr);
            return Ok(Exit { status, context });
        }
    };
    let done = |status: i32| Exit {
        status,
        context: None,
    };
    if values.contains_key("self_test") {
        return self_test(host, vocabulary).map(done);
    }
    let symptoms = PyValue::List(
        texts(&values, "symptom")
            .into_iter()
            .map(PyValue::Str)
            .collect(),
    );
    let events = texts(&values, "events");
    let (started, ended) = (text(&values, "started_utc"), text(&values, "ended_utc"));
    let tolerance = match values.get("tolerance_s") {
        Some(Value::Int(number)) => number.clone(),
        _ => BigInt::from(0),
    };
    let mut hits = PyValue::List(Vec::new());
    let mut scanned = PyValue::str("none");
    if !events.is_empty() {
        if started.is_empty() || ended.is_empty() {
            host.print_stderr(&failure(
                "[escalation] FAIL: --events 를 쓰려면 --started-utc/--ended-utc 가 필요하다 (구간 없는 대조는 전 이력을 이 셀의 사인으로 읽는다)\n",
            ))?;
            return Ok(done(2));
        }
        host.load()?;
        let mut lines: Vec<PyStr> = Vec::new();
        for path in &events {
            host.record_event("event_open", vec![("path", PyValue::Str(path.clone()))]);
            match read_lines(path) {
                Ok(read) => lines.extend(read),
                Err(raised) if is_os_error(&raised.class) => {
                    host.record_event(
                        "event_open_error",
                        vec![
                            ("type", PyValue::str(&raised.class)),
                            ("message", PyValue::Str(raised.message.clone())),
                        ],
                    );
                    let mut line = PyStr::new("[escalation] ⚠ events 읽기 실패 ");
                    line.push_py(path);
                    line.push_str(": ");
                    line.push_py(&raised.message);
                    line.push_str("\n");
                    host.print_stderr(&line)?;
                }
                Err(raised) => return Err(raised),
            }
        }
        let started_wire = host.utc(&started, "--started-utc");
        forward_stderr(host)?;
        let started_wire = started_wire?;
        let ended_wire = host.utc(&ended, "--ended-utc");
        forward_stderr(host)?;
        let ended_wire = ended_wire?;
        let found = host.kill_events(&lines, &started_wire, &ended_wire, &tolerance);
        forward_stderr(host)?;
        hits = found?;
        let mut names = PyStr::default();
        for (position, path) in events.iter().enumerate() {
            if position > 0 {
                names.push_str(",");
            }
            names.push_py(path);
        }
        scanned = PyValue::Str(names);
    }
    let out = judge(host, vocabulary, &symptoms, &hits, &scanned)?;
    let holds = matches!(out.get("predicate_1"), Some(PyValue::Bool(true)));
    if values.contains_key("json") {
        let shown = PyValue::Dict(out)
            .dumps_pretty()
            .map_err(|error| Raised::from(Failure::new(error.python_type(), &error.to_string())))?;
        host.print_stdout(&shown);
    } else {
        host.print_stdout(&PyStr::new(&format!(
            "[escalation] 술어 ① = {}",
            if holds { "충족" } else { "미충족" }
        )));
        if let Some(PyValue::List(reasons)) = out.get("reasons") {
            for reason in reasons {
                let mut line = PyStr::new("  - ");
                line.push_py(&reason.display_text());
                host.print_stdout(&line);
            }
        }
    }
    Ok(done(i32::from(!holds)))
}

fn is_os_error(class: &str) -> bool {
    crate::model_dyn::is_os_error(class)
}

// ── the self-test ────────────────────────────────────────────────────────────────────────────────

struct Checks<'a> {
    host: &'a mut dyn CliHost,
    failures: usize,
}

impl Checks<'_> {
    fn check(&mut self, name: &str, passed: bool) {
        let mut line = String::from(if passed { "  [PASS] " } else { "  [FAIL] " });
        line.push_str(name);
        if !passed {
            line.push(' ');
            self.failures += 1;
        }
        self.host.print_stdout(&PyStr::new(&line));
    }
}

fn hit(kind: &str, stamp: &str) -> PyValue {
    let mut row = PyDict::new();
    row.set("kind", PyValue::str(kind));
    row.set("ts", PyValue::str(stamp));
    row.set("mode", PyValue::str("armed"));
    PyValue::Dict(row)
}

fn names(items: &[&str]) -> PyValue {
    PyValue::List(items.iter().map(|name| PyValue::str(name)).collect())
}

fn field<'a>(row: &'a PyDict, key: &str) -> &'a PyValue {
    row.get(key).expect("the verdict has every field")
}

fn is_false(row: &PyDict) -> bool {
    matches!(field(row, "predicate_1"), PyValue::Bool(false))
}

fn is_true(row: &PyDict) -> bool {
    matches!(field(row, "predicate_1"), PyValue::Bool(true))
}

fn same_names(value: &PyValue, expected: &[&str]) -> bool {
    matches!(value, PyValue::List(items)
        if items.len() == expected.len()
            && items.iter().zip(expected).all(|(item, name)| matches!(item, PyValue::Str(text) if text.equals(name))))
}

fn self_test(host: &mut dyn CliHost, vocabulary: &Vocabulary) -> Result<i32, Raised> {
    let none = PyValue::str("none");
    let kill = PyValue::List(vec![hit("watchdog_kill_ack", "2026-09-10T05:20:54Z")]);
    let trip_only = PyValue::List(vec![hit("watchdog_trip", "2026-09-10T05:20:53Z")]);
    let empty = PyValue::List(Vec::new());
    let mut checks = Checks { host, failures: 0 };
    let r = judge(
        checks.host,
        vocabulary,
        &names(&["watchdog_kill_ack"]),
        &kill,
        &none,
    )?;
    checks.check(
        "E1 호스트 사인 단독은 ① 을 만들지 못한다",
        is_false(&r) && same_names(field(&r, "host_plane_ignored"), &["watchdog_kill_ack"]),
    );
    let r = judge(
        checks.host,
        vocabulary,
        &names(&["serve_init_immediate_death"]),
        &kill,
        &none,
    )?;
    checks.check(
        "E2 사살이 관측되면 '즉사' 는 반증된다",
        is_false(&r)
            && same_names(
                field(&r, "model_plane_refuted"),
                &["serve_init_immediate_death"],
            ),
    );
    let r = judge(
        checks.host,
        vocabulary,
        &names(&["serve_init_immediate_death"]),
        &empty,
        &none,
    )?;
    checks.check(
        "E3 사살이 없으면 '즉사' 는 ① 을 만든다(과잉차단 ✗)",
        is_true(&r),
    );
    let r = judge(
        checks.host,
        vocabulary,
        &names(&["serve_init_immediate_death"]),
        &trip_only,
        &none,
    )?;
    checks.check(
        "E4 trip 만으로는 반증하지 않는다(집행된 사살만 반증한다)",
        is_true(&r),
    );
    let r = judge(
        checks.host,
        vocabulary,
        &names(&["config_arch_unsupported"]),
        &kill,
        &none,
    )?;
    checks.check(
        "E5 사살이 있어도 arch 미지원은 독립으로 ① 을 만든다",
        is_true(&r),
    );
    let r = judge(
        checks.host,
        vocabulary,
        &names(&["fused_moe_fp8_config_absent"]),
        &empty,
        &none,
    )?;
    checks.check(
        "E6 닫힌 목록 밖 증상은 조용히 통과하지 않는다",
        is_false(&r)
            && same_names(
                field(&r, "unknown_symptoms"),
                &["fused_moe_fp8_config_absent"],
            ),
    );
    let r = judge(checks.host, vocabulary, &empty, &empty, &none)?;
    checks.check("E7 증상 부재는 미충족이다(부재 ≠ 충족)", is_false(&r));
    let r = judge(
        checks.host,
        vocabulary,
        &names(&["config_arch_unsupported", "watchdog_kill_ack"]),
        &kill,
        &none,
    )?;
    checks.check(
        "E8 호스트 사인이 섞여도 모델 평면 증상이 서면 ① 은 선다",
        is_true(&r) && same_names(field(&r, "host_plane_ignored"), &["watchdog_kill_ack"]),
    );
    // The negative control: a host sign put into the model list must still not make predicate one.
    let mut injected = vocabulary.clone();
    injected
        .model
        .set("watchdog_kill_ack", PyValue::str("(음성대조 주입)"));
    let r = judge(
        checks.host,
        &injected,
        &names(&["watchdog_kill_ack"]),
        &kill,
        &none,
    )?;
    checks.check(
        "★E9 음성대조: 호스트 사인을 모델 평면에 넣어도 ① 은 서지 않는다(호스트 우선은 코드다)",
        is_false(&r),
    );
    let flagged = matches!(field(&r, "reasons"), PyValue::List(reasons)
        if reasons.iter().any(|reason| matches!(reason, PyValue::Str(text) if text.to_utf8().is_some_and(|text| text.contains("배선 오류")))));
    checks.check(
        "★E9b 그 겹침을 배선 오류로 지목한다(조용한 통과 ✗)",
        same_names(field(&r, "vocabulary_collisions"), &["watchdog_kill_ack"]) && flagged,
    );
    let verdict = if checks.failures == 0 {
        String::from("PASS")
    } else {
        format!("FAIL ({})", checks.failures)
    };
    checks
        .host
        .print_stdout(&PyStr::new(&format!("[escalation_predicate] {verdict}")));
    Ok(i32::from(checks.failures != 0))
}

/// `main()` as a process runs it: an exception nothing handles is reported (a `SystemExit` with text prints the text) and ends
/// with status 1.
pub fn run(
    host: &mut dyn CliHost,
    vocabulary: &Vocabulary,
    args: &[PyStr],
    columns: Option<&str>,
) -> i32 {
    match execute(host, vocabulary, args, columns) {
        Ok(exit) => exit.status,
        Err(raised) => {
            let report = if raised.class == "SystemExit" {
                let mut text = raised.message.clone();
                text.push_str("\n");
                text
            } else {
                raised.report()
            };
            host.write_uncaught(&report);
            1
        }
    }
}

// ── the production host ──────────────────────────────────────────────────────────────────────────

/// The program's own host: standard error live, standard output kept for the end.
pub struct ProcessHost {
    bridge: Bridge,
    streams: Streams,
    start: StdioAtStart,
}

impl ProcessHost {
    pub fn new(bridge: Bridge) -> ProcessHost {
        ProcessHost {
            bridge,
            streams: Streams::default(),
            start: startup_stdio(),
        }
    }
}

impl Dependency for ProcessHost {
    fn load(&mut self) -> Result<crate::escalation::Loaded, Raised> {
        self.bridge.load()
    }

    fn utc(&mut self, value: &PyStr, field: &str) -> Result<PyValue, Raised> {
        self.bridge.utc(value, field)
    }

    fn kill_events(
        &mut self,
        lines: &[PyStr],
        started: &PyValue,
        ended: &PyValue,
        tolerance_s: &BigInt,
    ) -> Result<PyValue, Raised> {
        self.bridge.kill_events(lines, started, ended, tolerance_s)
    }

    fn take_stderr(&mut self) -> Vec<PyStr> {
        self.bridge.take_stderr()
    }
}

impl CliHost for ProcessHost {
    fn print_stderr(&mut self, text: &PyStr) -> Res<()> {
        if self.start.stderr_open {
            write_real_stderr(text)
        } else {
            if self.start.stdout_open {
                print_to_stream(&mut self.streams, text);
            }
            Ok(())
        }
    }

    fn write_stderr(&mut self, text: &PyStr) -> Res<()> {
        if self.start.stderr_open {
            write_real_stderr(text)
        } else {
            Err(Failure::new(
                "AttributeError",
                "'NoneType' object has no attribute 'write'",
            ))
        }
    }

    fn write_stdout(&mut self, text: &PyStr) {
        self.streams.write(text);
    }

    fn parser_error(&mut self, usage: &PyStr, message: &PyStr) {
        if self.start.stderr_open {
            for text in [usage, message].into_iter().filter(|text| !text.is_empty()) {
                let _ = write_real_stderr(text);
            }
        } else if self.start.stdout_open && !usage.is_empty() {
            self.streams.write(usage);
        }
    }

    fn parser_help(&mut self, help: &PyStr) {
        if self.start.stdout_open {
            self.streams.write(help);
        } else if self.start.stderr_open {
            let _ = write_real_stderr(help);
        }
    }

    fn write_uncaught(&mut self, text: &PyStr) {
        if self.start.stderr_open {
            let _ = write_real_stderr(text);
        }
    }
}

/// Runs the program against its installed classifier and hands what it printed to standard output; the final exit status (a
/// standard output that cannot be written changes it, as it does in Python).
pub fn run_process(skill_root: &PyStr, args: &[PyStr], columns: Option<&str>) -> i32 {
    let mut host = ProcessHost::new(Bridge::installed(skill_root));
    let status = run(&mut host, &Vocabulary::canonical(), args, columns);
    let run: Run = host.streams.finish(status);
    emit(&run)
}
