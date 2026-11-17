//! The three command lines (`quant_table.py`, `estimate_vram.py`, `rank_recipes.py`) as functions from arguments to
//! status and stream text. Program names in usage and help are the original `.py` names. An exception nothing handles
//! ends the run with `ClassName: message` on standard error and status 1 (no traceback frames are invented).

use crate::cliparse::{parse, Kind, Opt, Outcome, Spec, Value};
use crate::estimate_vram::{estimate, self_test};
use crate::pyexpr::Res;
use crate::quant_table::{dtype_bpw, vllm_quant_bpw};
use crate::rank_recipes::{auto_candidates, rank, render_report};
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pyjson::{decode_file_text, loads_text, LoadError};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;

/// What a command line produced.
pub struct Run {
    pub status: i32,
    pub stdout: PyStr,
    pub stderr: PyStr,
    /// The `write` calls that produced `stdout`, in order; empty means one `print` of `stdout`. Python's help output and
    /// the self-test report are single writes, and `quant_table` without `--json` is one `print` per line; the buffering
    /// of a terminal depends on these boundaries.
    pub writes: Vec<PyStr>,
}

impl Run {
    fn done(stdout: PyStr) -> Run {
        Run {
            status: 0,
            stdout,
            stderr: PyStr::default(),
            writes: Vec::new(),
        }
    }

    /// A run whose output is the given `print` lines (each the text and then its own newline).
    fn done_lines(lines: Vec<PyStr>) -> Run {
        let mut stdout = PyStr::default();
        let mut writes = Vec::new();
        for line in lines {
            stdout.push_py(&line);
            stdout.push_str("\n");
            writes.push(line);
            writes.push(PyStr::new("\n"));
        }
        Run {
            status: 0,
            stdout,
            stderr: PyStr::default(),
            writes,
        }
    }

    /// An exit whose standard output is one `write` of the given text (the help message, the self-test report).
    fn single_write(status: i32, stdout: PyStr, stderr: PyStr) -> Run {
        let writes = if stdout.is_empty() {
            Vec::new()
        } else {
            vec![stdout.clone()]
        };
        Run {
            status,
            stdout,
            stderr,
            writes,
        }
    }

    fn from_failure(failure: &Failure) -> Run {
        let class = match failure.class {
            "JSONDecodeError" => "json.decoder.JSONDecodeError",
            other => other,
        };
        let mut line = PyStr::new(class);
        if !failure.message.is_empty() {
            line.push_str(": ");
            line.push_py(&failure.message);
        }
        line.push_str("\n");
        Run {
            status: 1,
            stdout: PyStr::default(),
            stderr: line,
            writes: Vec::new(),
        }
    }
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

fn help_option() -> Opt {
    opt(&["-h", "--help"], "help", Kind::Help, None)
}

fn exit_of(outcome: Outcome) -> Result<BTreeMap<&'static str, Value>, Run> {
    match outcome {
        Outcome::Values(values) => Ok(values),
        Outcome::Exit {
            status,
            stdout,
            stderr,
        } => Err(Run::single_write(status, stdout, stderr)),
    }
}

/// `open(path, "r", encoding="utf-8").read()`.
fn read_text(path: &PyStr) -> Res<PyStr> {
    let bytes = path
        .to_surrogateescape_bytes()
        .map_err(|failure| Failure::new("UnicodeEncodeError", &failure.message()))?;
    let content = std::fs::read(OsString::from_vec(bytes)).map_err(|error| {
        let class = match error.raw_os_error() {
            Some(1 | 13) => "PermissionError",
            Some(2) => "FileNotFoundError",
            Some(20) => "NotADirectoryError",
            Some(21) => "IsADirectoryError",
            _ => "OSError",
        };
        let detail = error.to_string();
        let strerror = detail
            .split(" (os error")
            .next()
            .unwrap_or(&detail)
            .to_string();
        let message = match error.raw_os_error() {
            Some(errno) => format!("[Errno {errno}] {strerror}: {}", path.repr()),
            None => format!("{strerror}: {}", path.repr()),
        };
        Failure::new(class, &message)
    })?;
    decode_file_text(&content).map_err(load_failure)
}

fn load_failure(error: LoadError) -> Failure {
    match error {
        LoadError::Decode { .. } => Failure::new("JSONDecodeError", &error.to_string()),
        LoadError::IntegerTooLong { .. } => Failure::new("ValueError", &error.to_string()),
        LoadError::Encoding(message) => Failure::new("UnicodeDecodeError", &message),
        LoadError::TooDeep { .. } => Failure::new(
            "RecursionError",
            "maximum recursion depth exceeded while decoding a JSON document",
        ),
    }
}

/// `json.load(open(path, encoding="utf-8"))`.
pub(crate) fn load_json(path: &PyStr) -> Res<PyValue> {
    loads_text(&read_text(path)?).map_err(load_failure)
}

pub(crate) fn dumps(value: &PyValue) -> Res<PyStr> {
    value.dumps_pretty().map_err(|error| {
        let text = error.to_string();
        let prefix = format!("{}: ", error.python_type());
        Failure::new(
            error.python_type(),
            text.strip_prefix(&prefix).unwrap_or(&text),
        )
    })
}

pub(crate) fn println_text(mut text: PyStr) -> PyStr {
    text.push_str("\n");
    text
}

fn as_value(value: &Value) -> PyValue {
    match value {
        Value::Text(text) => PyValue::Str(text.clone()),
        Value::Texts(list) => PyValue::List(list.iter().cloned().map(PyValue::Str).collect()),
        Value::Int(number) => PyValue::Int(number.clone()),
        Value::Float(number) => PyValue::Float(*number),
        Value::Flag => PyValue::Bool(true),
    }
}

fn arg(values: &BTreeMap<&'static str, Value>, dest: &str) -> PyValue {
    values.get(dest).map_or(PyValue::None, as_value)
}

fn arg_or(values: &BTreeMap<&'static str, Value>, dest: &str, default: PyValue) -> PyValue {
    values.get(dest).map_or(default, as_value)
}

// ── quant_table ──────────────────────────────────────────────────────────────

fn quant_table_spec() -> Spec {
    Spec {
        prog: "quant_table.sh",
        description: "quant 바이트 테이블 해소(벤더링) — quant/dtype → bpw.",
        options: vec![
            help_option(),
            opt(
                &["--quant"],
                "quant",
                Kind::Text,
                Some("vLLM quantization 플래그(none/fp8/awq/...). 미지정 시 serve_bpw 반환."),
            ),
            opt(
                &["--serve-bpw"],
                "serve_bpw",
                Kind::Float,
                Some("native dtype bytes-per-weight (none/null quant 주입용, 기본 2.0)."),
            ),
            opt(
                &["--dtype"],
                "dtype",
                Kind::Text,
                Some("dtype 문자열(bfloat16/float16/float32/...) → bpw 해소."),
            ),
            opt(&["--json"], "json", Kind::Flag, Some("JSON 출력.")),
        ],
        group_required: false,
        positional: None,
    }
}

fn quant_table_main(args: &[PyStr], columns: Option<&str>) -> Res<Run> {
    let values = match exit_of(parse(&quant_table_spec(), args, columns)) {
        Ok(values) => values,
        Err(run) => return Ok(run),
    };
    let (quant, dtype) = (arg(&values, "quant"), arg(&values, "dtype"));
    let serve_bpw = arg_or(&values, "serve_bpw", PyValue::Float(2.0));
    let mut out = PyDict::new();
    if !matches!(dtype, PyValue::None) {
        let PyValue::Tuple(pair) = dtype_bpw(&dtype)? else {
            unreachable!("a pair")
        };
        out.set("dtype", dtype.clone());
        out.set("dtype_bpw", pair[0].clone());
        out.set("dtype_warning", pair[1].clone());
    }
    if !matches!(quant, PyValue::None) || matches!(dtype, PyValue::None) {
        out.set("quant", quant.clone());
        out.set("serve_bpw", serve_bpw.clone());
        out.set("vllm_quant_bpw", vllm_quant_bpw(&quant, &serve_bpw)?);
    }
    let out = PyValue::Dict(out);
    if matches!(values.get("json"), Some(Value::Flag)) {
        return Ok(Run::done(println_text(dumps(&out)?)));
    }
    let mut lines = Vec::new();
    if let PyValue::Dict(map) = &out {
        for (key, value) in map.iter() {
            let mut line = PyStr::default();
            line.push_py(key);
            line.push_str(": ");
            line.push_py(&value.display_text());
            lines.push(line);
        }
    }
    Ok(Run::done_lines(lines))
}

/// `python quant_table.py ...`.
pub fn quant_table(args: &[PyStr], columns: Option<&str>) -> Run {
    quant_table_main(args, columns).unwrap_or_else(|failure| Run::from_failure(&failure))
}

// ── estimate_vram ────────────────────────────────────────────────────────────

fn estimate_spec() -> Spec {
    Spec {
        prog: "estimate_vram.sh",
        description: "결정론 VRAM 추정기 (vllm-recipe-explorer).",
        options: vec![
            help_option(),
            opt(
                &["--parsed"],
                "parsed",
                Kind::Text,
                Some("parse_model_config 출력 JSON 경로"),
            ),
            opt(
                &["--self-test"],
                "self_test",
                Kind::Flag,
                Some("하이브리드 KV 층수 인지 회귀(모델/하드웨어 불요)"),
            ),
            opt(
                &["--quant"],
                "quant",
                Kind::Text,
                Some("후보 quantization 플래그(none/fp8/awq/...)"),
            ),
            opt(&["--max-model-len"], "max_model_len", Kind::Int, None),
            opt(
                &["--gmu"],
                "gmu",
                Kind::Float,
                Some("gpu_memory_utilization (0~1)"),
            ),
            opt(&["--tp"], "tp", Kind::Int, Some("tensor_parallel_size")),
            opt(
                &["--budget"],
                "budget",
                Kind::Float,
                Some("vram_budget_gib"),
            ),
            opt(
                &["--margin"],
                "margin",
                Kind::Float,
                Some("safety_margin (하드게이트 임계)"),
            ),
            opt(
                &["--kv-bytes"],
                "kv_bytes",
                Kind::Int,
                Some("KV dtype 바이트(기본 2=fp16)"),
            ),
            opt(&["--id"], "id", Kind::Text, Some("후보 id (선택)")),
        ],
        group_required: false,
        positional: None,
    }
}

fn estimate_main(args: &[PyStr], columns: Option<&str>) -> Res<Run> {
    let spec = estimate_spec();
    let values = match exit_of(parse(&spec, args, columns)) {
        Ok(values) => values,
        Err(run) => return Ok(run),
    };
    if matches!(values.get("self_test"), Some(Value::Flag)) {
        let outcome = self_test();
        return Ok(Run::single_write(
            outcome.status,
            PyStr::new(&outcome.stdout),
            PyStr::new(&outcome.stderr),
        ));
    }
    let missing: Vec<&str> = [
        ("--parsed", "parsed"),
        ("--max-model-len", "max_model_len"),
        ("--gmu", "gmu"),
        ("--tp", "tp"),
        ("--budget", "budget"),
        ("--margin", "margin"),
    ]
    .iter()
    .filter(|(_, dest)| !values.contains_key(dest))
    .map(|(flag, _)| *flag)
    .collect();
    if !missing.is_empty() {
        let width = crate::cliparse::terminal_width(columns);
        let usage = crate::cliparse::format_usage(&spec, width);
        return Ok(Run {
            status: 2,
            writes: Vec::new(),
            stdout: PyStr::default(),
            stderr: PyStr::new(&format!(
                "{usage}{}: error: {} 는 필수다(--self-test 는 예외)\n",
                spec.prog,
                missing.join(", ")
            )),
        });
    }
    let parsed = load_json(&match values.get("parsed") {
        Some(Value::Text(path)) => path.clone(),
        _ => unreachable!("required above"),
    })?;
    let candidate = PyValue::Dict({
        let mut dict = PyDict::new();
        dict.set("id", arg(&values, "id"));
        dict.set("quantization", arg(&values, "quant"));
        dict.set("max_model_len", arg(&values, "max_model_len"));
        dict.set("gpu_memory_utilization", arg(&values, "gmu"));
        dict
    });
    let result = estimate(
        &parsed,
        &candidate,
        &arg(&values, "tp"),
        &arg(&values, "budget"),
        &arg(&values, "margin"),
        &arg_or(&values, "kv_bytes", PyValue::int(2)),
    )?;
    Ok(Run::done(println_text(dumps(&result)?)))
}

/// `python estimate_vram.py ...`.
pub fn estimate_vram(args: &[PyStr], columns: Option<&str>) -> Run {
    estimate_main(args, columns).unwrap_or_else(|failure| Run::from_failure(&failure))
}

// ── rank_recipes ─────────────────────────────────────────────────────────────

fn rank_spec() -> Spec {
    let mut parsed = opt(
        &["--parsed"],
        "parsed",
        Kind::Text,
        Some("parse_model_config.sh 의 JSON 출력 파일 경로"),
    );
    parsed.required = true;
    let mut candidates = opt(
        &["--candidates"],
        "candidates",
        Kind::Text,
        Some("후보 리스트 JSON 파일 (LLM 생성 candidates.json)"),
    );
    candidates.group = Some(0);
    let mut auto = opt(
        &["--auto"],
        "auto",
        Kind::Flag,
        Some("결정론 기본 그리드(auto_candidates) 사용"),
    );
    auto.group = Some(0);
    let mut budget = opt(
        &["--budget"],
        "budget",
        Kind::Float,
        Some("VRAM 예산 (GiB)"),
    );
    budget.required = true;
    Spec {
        prog: "rank_recipes.sh",
        description: "Rank vLLM serving recipes against a VRAM budget hard-gate.",
        options: vec![
            help_option(),
            parsed,
            candidates,
            auto,
            opt(&["--tp"], "tp", Kind::Int, Some("tensor_parallel_size")),
            budget,
            opt(
                &["--margin"],
                "margin",
                Kind::Float,
                Some("안전마진 하드게이트 임계 (기본 0.90)"),
            ),
            opt(
                &["--kv-bytes"],
                "kv_bytes",
                Kind::Int,
                Some("KV cache dtype 바이트 (기본 2=fp16)"),
            ),
            opt(
                &["--json"],
                "json",
                Kind::Flag,
                Some("리포트 표 대신 랭킹 결과 JSON 출력"),
            ),
        ],
        group_required: true,
        positional: None,
    }
}

fn rank_main(args: &[PyStr], columns: Option<&str>) -> Res<Run> {
    let values = match exit_of(parse(&rank_spec(), args, columns)) {
        Ok(values) => values,
        Err(run) => return Ok(run),
    };
    let path = |dest: &str| match values.get(dest) {
        Some(Value::Text(path)) => path.clone(),
        _ => unreachable!("required by the parser"),
    };
    let parsed = load_json(&path("parsed"))?;
    let tp = arg_or(&values, "tp", PyValue::int(1));
    let (budget, margin) = (
        arg(&values, "budget"),
        arg_or(&values, "margin", PyValue::Float(0.90)),
    );
    let kv_bytes = arg_or(&values, "kv_bytes", PyValue::int(2));
    let candidates = if matches!(values.get("auto"), Some(Value::Flag)) {
        auto_candidates(&parsed, &tp)?
    } else {
        let loaded = load_json(&path("candidates"))?;
        let mut numbered = Vec::new();
        for (index, item) in crate::pyexpr::items_of(&loaded)?.into_iter().enumerate() {
            match item {
                PyValue::Dict(mut dict) => {
                    if !dict.contains_key("id") {
                        dict.set("id", PyValue::str(&format!("r{}", index + 1)));
                    }
                    numbered.push(PyValue::Dict(dict));
                }
                other => {
                    return Err(Failure::new(
                        "AttributeError",
                        &format!(
                            "'{}' object has no attribute 'setdefault'",
                            other.type_name()
                        ),
                    ));
                }
            }
        }
        PyValue::List(numbered)
    };
    let result = rank(&parsed, &candidates, &tp, &budget, &margin, &kv_bytes)?;
    if matches!(values.get("json"), Some(Value::Flag)) {
        return Ok(Run::done(println_text(dumps(&result)?)));
    }
    Ok(Run::done(println_text(render_report(
        &result, &parsed, &budget, &margin, &tp,
    )?)))
}

/// `python rank_recipes.py ...`.
pub fn rank_recipes(args: &[PyStr], columns: Option<&str>) -> Run {
    rank_main(args, columns).unwrap_or_else(|failure| Run::from_failure(&failure))
}

/// One `print` of text that already ends with its newline: the text, then the newline, as two writes.
fn print_writes(stdout: &PyStr) -> Vec<PyStr> {
    let points = stdout.code_points();
    if points.last() == Some(&u32::from('\n')) {
        vec![
            PyStr::from_code_points(points[..points.len() - 1].to_vec()),
            PyStr::new("\n"),
        ]
    } else {
        vec![stdout.clone()]
    }
}

/// Writes a finished [`Run`] to the inherited streams the way the interpreter ends a program and returns the exit status.
///
/// Standard output goes through [`TextStdout`](crate::stdio::TextStdout), so a failing descriptor reports at the same point
/// CPython does: inside the `print` for a result of 8192 bytes or more (the uncaught `OSError` line, status 1), or at exit for
/// a shorter one (the "Exception ignored" report and status 120). Standard error is written last-resort: its failures are
/// ignored, as the interpreter ignores them. Text that cannot be encoded is the `UnicodeEncodeError` of the print.
pub fn emit(run: &Run) -> i32 {
    use crate::stdio::{inherited_stdout, os_failure, TextStdout};
    use std::io::{IsTerminal, Write};
    let mut status = run.status;
    let mut stderr_text = run.stderr.clone();
    if !run.stdout.is_empty() {
        let (file, capacity) = match inherited_stdout() {
            Ok(pair) => pair,
            Err(error) => {
                stderr_text.push_py(&os_failure(&error).line());
                stderr_text.push_str("\n");
                let _ = std::io::stderr().write_all(&stderr_text.to_backslashreplace_bytes());
                return 1;
            }
        };
        // A terminal makes the text layer line-buffered, as `sys.stdout` is when `isatty()` is true.
        let mut out =
            TextStdout::new(file, capacity).line_buffered(std::io::stdout().is_terminal());
        let writes = if run.writes.is_empty() {
            print_writes(&run.stdout)
        } else {
            run.writes.clone()
        };
        let mut printing = Ok(());
        for text in &writes {
            let bytes = match text.to_surrogateescape_bytes() {
                Ok(bytes) => bytes,
                Err(failure) => {
                    printing = Err(Failure::new("UnicodeEncodeError", &failure.message()));
                    break;
                }
            };
            if let Err(error) = out.write(&bytes) {
                printing = Err(os_failure(&error));
                break;
            }
        }
        if let Err(failure) = printing {
            status = 1;
            stderr_text.push_py(&failure.line());
            stderr_text.push_str("\n");
        }
        if let Err(error) = out.finish() {
            stderr_text.push_str(
                "Exception ignored in: <_io.TextIOWrapper name='<stdout>' mode='w' encoding='utf-8'>\n",
            );
            stderr_text.push_py(&os_failure(&error).line());
            stderr_text.push_str("\n");
            status = 120;
        }
    }
    let _ = std::io::stderr().write_all(&stderr_text.to_backslashreplace_bytes());
    status
}
