//! The two command lines of the transform owners: `rope_scaling_translate.py` (`main`, `_self_test`) and `parse_vllm_log.py` (`_main`).
//!
//! * `rope_scaling_translate --model-dir DIR --target-len N [--factor F] [--json]` / `--self-test`: the whole command line is parsed
//!   first (so a bad option beats `--self-test`), then the self-test, then the guard (a missing or zero argument is status 2), then
//!   `DIR/config.json` is opened as UTF-8 and read as JSON (an `OSError` or `ValueError`, a decoding failure included, is `FAIL` and
//!   status 2, raised from the failure that caused it), then the translation is printed as JSON or as text. Status 0 translates or needs
//!   no extension, 3 cannot translate.
//! * `parse_vllm_log [--log FILE]`: the log is read from FILE when that is a non-empty text (UTF-8, strict, universal newlines), else from
//!   standard input (UTF-8 with surrogate escapes, line ends left as they are); the twelve fields are printed as JSON.
//!
//! Files and standard input go through the [`Console`], so a check can observe every open and read; the stdio rules of the other command
//! lines apply (a `print(file=sys.stderr)` falls back to standard output when standard error was closed at start, the parser's own
//! writes and the uncaught report do not).

use crate::cli::{emit, Run};
use crate::cliparse::{
    format_usage, parse as parse_arguments, terminal_width, Kind, Opt, Outcome, Spec, Value,
};
use crate::escalation::Raised;
use crate::memory::open_text_file;
use crate::memory_cli::{print_to_stream, split_usage, write_real_stderr};
use crate::memory_system::{startup_stdin_open, startup_stdio, StdioAtStart};
use crate::model_cli::Streams;
use crate::model_files::{join, load_failure, os_failure};
use crate::pyexpr::Res;
use crate::rope_translate::{translate, Profile};
use crate::vllm_log::parse_log;
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pyjson::{decode_file_text, loads_text, loads_text_objects};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use num_bigint::BigInt;
use std::io::Read;

/// The places a transform command line reads and writes.
pub trait Console {
    /// `print(text, file=sys.stderr)`: the text ends with its newline.
    fn print_stderr(&mut self, text: &PyStr) -> Res<()>;
    /// `sys.stdout.write(text)`.
    fn write_stdout(&mut self, text: &PyStr);
    /// `print(text)`: the text and then its newline, as two writes.
    fn print_stdout(&mut self, text: &PyStr) {
        self.write_stdout(text);
        self.write_stdout(&PyStr::new("\n"));
    }
    /// `argparse.error()`: usage to `sys.stderr` (to `sys.stdout` when that is `None`), then the message to `sys.stderr`; failures suppressed.
    fn parser_error(&mut self, usage: &PyStr, message: &PyStr);
    /// `print_help()`: standard output, else standard error, else nowhere; failures suppressed.
    fn parser_help(&mut self, help: &PyStr);
    /// The interpreter's report of an exception nothing handled: standard error only, a failure to write it ignored.
    fn write_uncaught(&mut self, text: &PyStr);
    /// Records an ordered observation (a check's control record); nothing in a production run.
    fn observe(&mut self, _name: &str, _fields: Vec<(&str, PyValue)>) {}
    /// `sys.stdin.read()`: everything, as text (bytes that are not UTF-8 are surrogate escapes, line ends untouched).
    fn read_stdin(&mut self) -> Result<PyStr, Raised>;
}

fn event_row(name: &str, fields: Vec<(&str, PyValue)>) -> (String, Vec<(String, PyValue)>) {
    (
        name.to_string(),
        fields
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
    )
}

/// `open(path, [mode,] encoding="utf-8")` and `.read()`: the whole text of a file, strict UTF-8, universal newlines. The open and the read
/// are observed as the original's collector saw them (`file_open`, `file_open_error`, `file_read`, `file_read_error`, `file_read_return`).
pub fn read_text_observed(
    console: &mut dyn Console,
    path: &PyStr,
    explicit_mode: bool,
) -> Result<PyStr, Raised> {
    let mut keywords = PyDict::new();
    keywords.set("encoding", PyValue::str("utf-8"));
    let modes = if explicit_mode {
        vec![PyValue::str("r")]
    } else {
        Vec::new()
    };
    let (name, fields) = event_row(
        "file_open",
        vec![
            ("path", PyValue::Str(path.clone())),
            ("args", PyValue::List(modes)),
            ("kwargs", PyValue::Dict(keywords)),
        ],
    );
    console.observe(
        &name,
        fields
            .iter()
            .map(|(k, v)| (k.as_str(), v.clone()))
            .collect(),
    );
    let report = |console: &mut dyn Console, event: &str, raised: &Raised| {
        console.observe(
            event,
            vec![
                ("type", PyValue::str(&raised.class)),
                ("message", PyValue::Str(raised.message.clone())),
            ],
        );
    };
    let mut file = match open_text_file(path) {
        Ok(file) => file,
        Err(failure) => {
            let raised = Raised::from(failure);
            report(console, "file_open_error", &raised);
            return Err(raised);
        }
    };
    console.observe(
        "file_read",
        vec![
            ("args", PyValue::List(Vec::new())),
            ("kwargs", PyValue::Dict(PyDict::new())),
            ("path", PyValue::Str(path.clone())),
        ],
    );
    let mut bytes = Vec::new();
    let decoded = file
        .read_to_end(&mut bytes)
        .map_err(|error| os_failure(&error, None))
        .and_then(|_| decode_file_text(&bytes).map_err(load_failure));
    match decoded {
        Ok(text) => {
            console.observe(
                "file_read_return",
                vec![("text", PyValue::Str(text.clone()))],
            );
            Ok(text)
        }
        Err(failure) => {
            let raised = Raised::from(failure);
            report(console, "file_read_error", &raised);
            Err(raised)
        }
    }
}

/// How a command line ended without an exception: the status and, for an exit raised while another exception was being handled (or an
/// argument `argparse` could not convert), that exception, which the `SystemExit` then has as its implicit context.
pub struct Exit {
    pub status: i32,
    pub context: Option<Raised>,
}

fn done(status: i32) -> Exit {
    Exit {
        status,
        context: None,
    }
}

/// The `argparse` exceptions behind an `invalid int value` / `invalid float value` usage error.
pub(crate) fn conversion_context(message: &PyStr) -> Option<Raised> {
    let text = message.to_utf8()?;
    let line = text.trim_end_matches('\n');
    let detail = line.rsplit_once(": error: ")?.1;
    let (kind, shown) = ["int", "float"].iter().find_map(|kind| {
        detail
            .split_once(&format!(": invalid {kind} value: "))
            .map(|(_, shown)| (*kind, shown))
    })?;
    let conversion = if kind == "int" {
        Raised::plain(
            "ValueError",
            &format!("invalid literal for int() with base 10: {shown}"),
        )
    } else {
        Raised::plain(
            "ValueError",
            &format!("could not convert string to float: {shown}"),
        )
    };
    let mut argument = Raised::new("ArgumentError", "argparse", PyStr::new(detail));
    argument.cause = Some(Box::new(conversion));
    argument.via_context = true;
    Some(argument)
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

/// The parser of `rope_scaling_translate.main`.
pub fn rope_spec() -> Spec {
    Spec {
        prog: "rope_scaling_translate.sh",
        description: "컨텍스트 확장 선언 → serve 인자 번역 (R8)",
        options: vec![
            opt(&["-h", "--help"], "help", Kind::Help, None),
            opt(
                &["--model-dir"],
                "model_dir",
                Kind::Text,
                Some("모델 디렉터리(config.json 을 읽는다)"),
            ),
            opt(
                &["--target-len"],
                "target_len",
                Kind::Int,
                Some("목표 max-model-len"),
            ),
            opt(
                &["--factor"],
                "factor",
                Kind::Float,
                Some("선언된 YaRN factor(생략 시 목표에서 파생)"),
            ),
            opt(&["--json"], "json", Kind::Flag, None),
            opt(&["--self-test"], "self_test", Kind::Flag, None),
        ],
        group_required: false,
        positional: None,
    }
}

/// The parser of `parse_vllm_log._main`.
pub fn log_spec() -> Spec {
    Spec {
        prog: "parse_vllm_log.sh",
        description: "vLLM 서빙 로그 파서 (vllm-recipe-explorer Phase 2).",
        options: vec![
            opt(&["-h", "--help"], "help", Kind::Help, None),
            opt(
                &["--log"],
                "log",
                Kind::Text,
                Some("vLLM 로그 파일 경로(미지정 시 stdin 에서 읽음)"),
            ),
        ],
        group_required: false,
        positional: None,
    }
}

pub(crate) type Values = std::collections::BTreeMap<&'static str, Value>;

/// Parses; `Err` is the early exit (help, usage error) already written.
pub(crate) fn parse_command(
    console: &mut dyn Console,
    spec: &Spec,
    args: &[PyStr],
    columns: Option<&str>,
) -> Result<Values, Exit> {
    match parse_arguments(spec, args, columns) {
        Outcome::Values(values) => Ok(values),
        Outcome::Exit {
            status,
            stdout,
            stderr,
        } => {
            if !stdout.is_empty() {
                console.parser_help(&stdout);
            }
            if !stderr.is_empty() {
                let usage = PyStr::new(&format_usage(spec, terminal_width(columns)));
                let (usage, message) = split_usage(&usage, &stderr);
                console.parser_error(&usage, &message);
            }
            Err(Exit {
                status,
                context: conversion_context(&stderr),
            })
        }
    }
}

fn json_text(value: &PyValue, pretty: bool) -> Result<PyStr, Raised> {
    let shown = if pretty {
        value.dumps_pretty()
    } else {
        value.dumps_ascii_tight().map(|text| PyStr::new(&text))
    };
    shown.map_err(|error| Raised::from(Failure::new(error.python_type(), &error.to_string())))
}

// ── rope_scaling_translate ───────────────────────────────────────────────────────────────────────

/// `rope_scaling_translate.main()`.
pub fn rope_execute(
    console: &mut dyn Console,
    profile: &Profile,
    args: &[PyStr],
    columns: Option<&str>,
) -> Result<Exit, Raised> {
    let values = match parse_command(console, &rope_spec(), args, columns) {
        Ok(values) => values,
        Err(exit) => return Ok(exit),
    };
    if values.contains_key("self_test") {
        return rope_self_test(console, profile).map(done);
    }
    let model_dir = match values.get("model_dir") {
        Some(Value::Text(text)) => text.clone(),
        _ => PyStr::default(),
    };
    let target = match values.get("target_len") {
        Some(Value::Int(number)) => number.clone(),
        _ => BigInt::from(0),
    };
    if model_dir.is_empty() || target == BigInt::from(0) {
        console.print_stderr(&PyStr::new(
            "[rope-translate] FAIL: --model-dir 과 --target-len 이 필요하다\n",
        ))?;
        return Ok(done(2));
    }
    let path = join(&model_dir, &PyStr::new("config.json"));
    let loaded = read_text_observed(console, &path, false)
        .and_then(|text| loads_text(&text).map_err(|error| Raised::from(load_failure(error))));
    let config = match loaded {
        Ok(config) => config,
        Err(raised) if is_caught(&raised) => {
            let mut line = PyStr::new("[rope-translate] FAIL: config.json 을 읽지 못했다 — ");
            line.push_py(&raised.message);
            line.push_str("\n");
            console.print_stderr(&line)?;
            return Ok(Exit {
                status: 2,
                context: Some(with_context_flag(raised)),
            });
        }
        Err(raised) => return Err(raised),
    };
    let factor = match values.get("factor") {
        Some(Value::Float(number)) => PyValue::Float(*number),
        _ => PyValue::None,
    };
    let out = translate(profile, &config, &PyValue::Int(target), &factor)?;
    let get = |key: &str| out.get(key).cloned().unwrap_or(PyValue::None);
    if values.contains_key("json") {
        console.print_stdout(&json_text(&PyValue::Dict(out.clone()), true)?);
    } else {
        if let PyValue::List(reasons) = get("reasons") {
            for reason in &reasons {
                let mut line = PyStr::new("[rope-translate] ");
                line.push_py(&reason.display_text());
                console.print_stdout(&line);
            }
        }
        if let PyValue::List(unknowns) = get("unknowns") {
            for unknown in &unknowns {
                let mut line = PyStr::new("[rope-translate] ⚠ 미검증: ");
                line.push_py(&unknown.display_text());
                console.print_stdout(&line);
            }
        }
        let overrides = get("hf_overrides");
        if overrides.is_truthy() {
            console.print_stdout(&PyStr::new("[rope-translate] serve 트리플렛에 넣을 줄:"));
            let mut line = PyStr::new("hf-overrides: '");
            line.push_py(&json_text(&overrides, false)?);
            line.push_str("'");
            console.print_stdout(&line);
        }
    }
    let failed = get("needed").is_truthy() && !get("hf_overrides").is_truthy();
    Ok(done(if failed { 3 } else { 0 }))
}

/// `except (OSError, ValueError)`.
fn is_caught(raised: &Raised) -> bool {
    crate::model_dyn::is_os_error(&raised.class) || crate::model_dyn::is_value_error(&raised.class)
}

/// The caught exception as the `SystemExit` raised in its handler has it: `JSONDecodeError("Expecting value")` is raised `from None`
/// by the decoder (its context is suppressed); every other exception here is raised plain.
fn with_context_flag(mut raised: Raised) -> Raised {
    if raised.class == "JSONDecodeError"
        && raised
            .message
            .to_utf8()
            .is_some_and(|text| text.starts_with("Expecting value"))
    {
        raised.suppress_context = true;
    }
    raised
}

fn field(value: &PyValue, key: &str) -> PyValue {
    match value {
        PyValue::Dict(map) => map.get(key).cloned().unwrap_or(PyValue::None),
        _ => PyValue::None,
    }
}

/// `value[key]`: a missing item or a value that is not a dictionary is the interpreter's failure.
fn item(value: &PyValue, key: &str) -> Res<PyValue> {
    match value {
        PyValue::Dict(map) => map
            .get(key)
            .cloned()
            .ok_or_else(|| Failure::new("KeyError", &PyStr::new(key).repr())),
        other => Err(Failure::new(
            "TypeError",
            &format!("'{}' object is not subscriptable", other.type_name()),
        )),
    }
}

fn is_int(value: &PyValue, expected: i64) -> bool {
    match value.peel() {
        PyValue::Int(number) => *number == BigInt::from(expected),
        PyValue::Bool(flag) => i64::from(*flag) == expected,
        PyValue::Float(number) => *number == expected as f64,
        _ => false,
    }
}

fn is_text(value: &PyValue, expected: &str) -> bool {
    matches!(value, PyValue::Str(text) if text.equals(expected))
}

fn any_contains(list: &PyValue, needle: &str) -> bool {
    matches!(list, PyValue::List(items) if items.iter().any(|entry| {
        matches!(entry, PyValue::Str(text) if text.to_utf8().is_some_and(|text| text.contains(needle)))
    }))
}

struct Checks<'a> {
    console: &'a mut dyn Console,
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
        self.console.print_stdout(&PyStr::new(&line));
    }
}

fn config(text: &str) -> PyValue {
    loads_text_objects(&PyStr::new(text)).expect("a fixed fixture is JSON")
}

/// `_self_test()`: the eleven checks of the original.
fn rope_self_test(console: &mut dyn Console, profile: &Profile) -> Result<i32, Raised> {
    let cfg = config(
        r#"{"text_config": {"max_position_embeddings": 262144, "rope_parameters": {"rope_type": "default", "rope_theta": 10000000, "mrope_interleaved": true, "mrope_section": [11, 11, 10], "partial_rotary_factor": 0.25}}}"#,
    );
    let (none, two) = (PyValue::None, PyValue::int(2));
    let run = |cfg: &PyValue, target: i64, factor: &PyValue| -> Res<PyValue> {
        translate(profile, cfg, &PyValue::int(target), factor).map(PyValue::Dict)
    };
    let rope_of = |result: &PyValue| -> Res<PyValue> {
        item(
            &item(
                &item(&field_of(result, "hf_overrides")?, "text_config")?,
                "rope_parameters",
            )?,
            "rope_parameters",
        )
        .or_else(|_| {
            item(
                &item(
                    &item(&field_of(result, "hf_overrides")?, "text_config")?,
                    "rope_parameters",
                )?,
                "rope_type",
            )
            .map(|_| PyValue::None)
        })
    };
    let _ = rope_of;
    let rope_parameters = |result: &PyValue| -> Res<PyValue> {
        item(
            &item(&field_of(result, "hf_overrides")?, "text_config")?,
            "rope_parameters",
        )
    };
    let mut checks = Checks {
        console,
        failures: 0,
    };

    let r = run(&cfg, 262144, &none)?;
    checks.check(
        "Y1 목표가 네이티브 이하면 확장하지 않는다(불필요한 인자를 넣지 않는다)",
        matches!(field(&r, "needed"), PyValue::Bool(false))
            && matches!(field(&r, "hf_overrides"), PyValue::None),
    );
    let r = run(&cfg, 524288, &two)?;
    let rp = rope_parameters(&r)?;
    checks.check(
        "Y2 factor 2 로 512k 를 연다",
        field(&r, "needed").is_truthy()
            && is_text(&field(&rp, "rope_type"), "yarn")
            && is_int(&field(&rp, "factor"), 2)
            && is_int(&field(&rp, "original_max_position_embeddings"), 262144),
    );
    checks.check(
        "★Y3 병합: 기존 rope 키가 **전부 살아남는다**(갈아끼우면 다른 모델이 된다)",
        matches!(field(&rp, "mrope_interleaved"), PyValue::Bool(true))
            && matches!(field(&rp, "mrope_section"), PyValue::List(ref items) if items.len() == 3 && is_int(&items[0], 11) && is_int(&items[1], 11) && is_int(&items[2], 10))
            && matches!(field(&rp, "partial_rotary_factor"), PyValue::Float(number) if number == 0.25)
            && is_int(&field(&rp, "rope_theta"), 10_000_000),
    );
    let unknowns = field(&r, "unknowns");
    checks.check(
        "★Y4 미검증 상호작용을 산출물이 스스로 밝힌다(가린 채 통과 ✗)",
        matches!(&unknowns, PyValue::List(items) if items.len() == 2)
            && any_contains(&unknowns, "mrope")
            && any_contains(&unknowns, "partial_rotary_factor"),
    );
    let r = run(&cfg, 1048576, &none)?;
    checks.check(
        "Y5 factor 미지정이면 목표에서 올림으로 파생한다",
        is_int(&field(&r, "factor"), 4),
    );
    let r = run(&cfg, 1048576, &two)?;
    checks.check(
        "★Y6 factor 가 목표에 못 미치면 거부한다(조용히 모자란 확장 ✗)",
        matches!(field(&r, "hf_overrides"), PyValue::None)
            && any_contains(&field(&r, "reasons"), "닿지 않는다"),
    );
    let already = config(
        r#"{"text_config": {"max_position_embeddings": 262144, "rope_parameters": {"rope_type": "yarn", "factor": 2}}}"#,
    );
    let r = run(&already, 524288, &none)?;
    checks.check(
        "★Y7 이미 확장된 모델에 다시 얹지 않는다(이중 적용 방지)",
        matches!(field(&r, "hf_overrides"), PyValue::None)
            && any_contains(&field(&r, "reasons"), "이중 적용"),
    );
    let bare = config(r#"{"text_config": {"max_position_embeddings": 262144}}"#);
    let r = run(&bare, 524288, &none)?;
    checks.check(
        "★Y8 rope 딕셔너리를 못 찾으면 override 를 지어내지 않는다(조용히 무시될 인자 금지)",
        matches!(field(&r, "hf_overrides"), PyValue::None)
            && any_contains(&field(&r, "reasons"), "찾지 못했다"),
    );
    let r = run(&config("{}"), 524288, &none)?;
    checks.check(
        "Y9 네이티브를 모르면 판정하지 않는다",
        matches!(field(&r, "needed"), PyValue::None),
    );
    let legacy =
        config(r#"{"rope_scaling": {"rope_type": "default"}, "max_position_embeddings": 8192}"#);
    let r = run(&legacy, 16384, &two)?;
    let legacy_type = item(
        &item(&field_of(&r, "hf_overrides")?, "rope_scaling")?,
        "rope_type",
    )?;
    checks.check(
        "Y10 구 레이아웃(최상위 rope_scaling)도 같은 규칙으로 번역한다",
        is_text(&legacy_type, "yarn") && is_text(&field(&r, "container_path"), "rope_scaling"),
    );
    let merged = rope_parameters(&run(&cfg, 524288, &two)?)?;
    let naive = ["rope_type", "factor", "original_max_position_embeddings"];
    let all_in_merged = naive
        .iter()
        .all(|key| matches!(&merged, PyValue::Dict(map) if map.contains_key(key)));
    let proper_subset = matches!(&merged, PyValue::Dict(map) if map.len() > naive.len());
    checks.check(
        "★Y11 음성대조: 순진한 override 는 mrope·partial rotary 를 잃는다(병합이 그것을 막는다)",
        all_in_merged && proper_subset,
    );
    let verdict = if checks.failures == 0 {
        String::from("PASS")
    } else {
        format!("FAIL ({})", checks.failures)
    };
    checks
        .console
        .print_stdout(&PyStr::new(&format!("[rope_scaling_translate] {verdict}")));
    Ok(i32::from(checks.failures != 0))
}

/// `result["name"]` of a value that must be a dictionary holding it.
fn field_of(value: &PyValue, key: &str) -> Res<PyValue> {
    item(value, key)
}

// ── parse_vllm_log ───────────────────────────────────────────────────────────────────────────────

/// `parse_vllm_log._main(argv)`.
pub fn log_execute(
    console: &mut dyn Console,
    args: &[PyStr],
    columns: Option<&str>,
) -> Result<Exit, Raised> {
    let values = match parse_command(console, &log_spec(), args, columns) {
        Ok(values) => values,
        Err(exit) => return Ok(exit),
    };
    let text = match values.get("log") {
        Some(Value::Text(path)) if !path.is_empty() => read_text_observed(console, path, true)?,
        _ => {
            console.observe(
                "stdin_read",
                vec![
                    ("args", PyValue::List(Vec::new())),
                    ("kwargs", PyValue::Dict(PyDict::new())),
                ],
            );
            match console.read_stdin() {
                Ok(text) => {
                    console.observe(
                        "stdin_read_return",
                        vec![("text", PyValue::Str(text.clone()))],
                    );
                    text
                }
                Err(raised) => {
                    console.observe(
                        "stdin_read_error",
                        vec![
                            ("type", PyValue::str(&raised.class)),
                            ("message", PyValue::Str(raised.message.clone())),
                        ],
                    );
                    return Err(raised);
                }
            }
        }
    };
    let result = parse_log(&PyValue::Str(text))?;
    console.print_stdout(&json_text(&PyValue::Dict(result), true)?);
    Ok(done(0))
}

// ── running ──────────────────────────────────────────────────────────────────────────────────────

/// How a process ends: an exception nothing handles is reported and ends with status 1.
pub fn finish(console: &mut dyn Console, outcome: Result<Exit, Raised>) -> i32 {
    match outcome {
        Ok(exit) => exit.status,
        Err(raised) => {
            console.write_uncaught(&raised.report());
            1
        }
    }
}

/// The program's own console: standard error live, standard output kept for the end.
pub struct ProcessConsole {
    streams: Streams,
    start: StdioAtStart,
}

impl ProcessConsole {
    pub fn new() -> ProcessConsole {
        ProcessConsole {
            streams: Streams::default(),
            start: startup_stdio(),
        }
    }

    /// Hands what the program printed to standard output; the final exit status.
    pub fn conclude(self, status: i32) -> i32 {
        let run: Run = self.streams.finish(status);
        emit(&run)
    }
}

impl Default for ProcessConsole {
    fn default() -> Self {
        Self::new()
    }
}

impl Console for ProcessConsole {
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

    fn read_stdin(&mut self) -> Result<PyStr, Raised> {
        read_process_stdin()
    }
}

/// `sys.stdin.read()` of the process: when descriptor 0 was closed at start `sys.stdin` is `None` (the Rust runtime reopens it on the
/// null device, so the snapshot taken before `main` decides) and the read is the `AttributeError` of `None.read`, before anything is
/// read; otherwise everything, undecodable bytes kept as surrogate escapes and line ends untouched.
pub fn read_process_stdin() -> Result<PyStr, Raised> {
    if !startup_stdin_open() {
        return Err(Raised::plain(
            "AttributeError",
            "'NoneType' object has no attribute 'read'",
        ));
    }
    let mut bytes = Vec::new();
    std::io::stdin()
        .lock()
        .read_to_end(&mut bytes)
        .map_err(|error| Raised::from(os_failure(&error, None)))?;
    Ok(PyStr::from_surrogateescape(&bytes))
}

/// `rope_scaling_translate` as a process.
pub fn run_rope_process(args: &[PyStr], columns: Option<&str>) -> i32 {
    let mut console = ProcessConsole::new();
    let outcome = rope_execute(&mut console, &Profile::canonical(), args, columns);
    let status = finish(&mut console, outcome);
    console.conclude(status)
}

/// `parse_vllm_log` as a process.
pub fn run_log_process(args: &[PyStr], columns: Option<&str>) -> i32 {
    let mut console = ProcessConsole::new();
    let outcome = log_execute(&mut console, args, columns);
    let status = finish(&mut console, outcome);
    console.conclude(status)
}
