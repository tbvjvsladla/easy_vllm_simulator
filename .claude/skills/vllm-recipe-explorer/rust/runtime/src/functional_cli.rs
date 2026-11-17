//! The command line of `functional_smoke.py` (`_main`):
//!
//! `functional_smoke [--base-url URL] --model NAME [--candidate FILE] [--timeout SECONDS] [--mock FILE]`
//!
//! The arguments are parsed as `argparse` does (abbreviations, the last of a repeated option, `--help`, the required `--model`, the float
//! `--timeout`), `--candidate` (when it is a non-empty text) is read as UTF-8 and loaded as JSON (any failure is uncaught: the traceback
//! of the exception, status 1), then `--mock` the same way. The JSON of the mock file is the original's public mock path: a file holding
//! `null` is live HTTP, anything else is a mock. The result is printed as `json.dumps(result, ensure_ascii=False, indent=2)` and the
//! status is 0 whatever `passed` says.

use crate::cliparse::{Kind, Opt, Spec, Value};
use crate::escalation::Raised;
use crate::functional_smoke::{smoke, Constants, Observe};
use crate::memory::open_text_file;
use crate::model_files::{load_failure, os_failure};
use crate::transform_cli::{finish, parse_command, Console, Exit, ProcessConsole};
use agent_runtime_support::pyjson::{decode_file_text, loads_text};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::io::Read;

fn opt(
    flags: &'static [&'static str],
    dest: &'static str,
    kind: Kind,
    help: Option<&'static str>,
    required: bool,
) -> Opt {
    Opt {
        flags,
        dest,
        kind,
        help,
        required,
        group: None,
    }
}

/// The parser of `_main`.
pub fn spec() -> Spec {
    Spec {
        prog: "functional_smoke.sh",
        description: "기능 스모크 검사기 (vllm-recipe-explorer Phase 2).",
        options: vec![
            opt(&["-h", "--help"], "help", Kind::Help, None, false),
            opt(
                &["--base-url"],
                "base_url",
                Kind::Text,
                Some("서빙 base_url (예: http://127.0.0.1:8903)"),
                false,
            ),
            opt(
                &["--model"],
                "model",
                Kind::Text,
                Some("served_model_name"),
                true,
            ),
            opt(
                &["--candidate"],
                "candidate",
                Kind::Text,
                Some("candidate(lock-set) JSON 경로 — model_capabilities 게이팅에 사용"),
                false,
            ),
            opt(
                &["--timeout"],
                "timeout",
                Kind::Float,
                Some("HTTP 타임아웃(초)"),
                false,
            ),
            opt(
                &["--mock"],
                "mock",
                Kind::Text,
                Some("실제 HTTP 대신 사용할 mock 응답 JSON 경로 ({completion,reasoning,tool_call: <resp>})."),
                false,
            ),
        ],
        group_required: false,
        positional: None,
    }
}

/// Tells the console about the HTTP steps (its control record, in a check; nothing in production).
struct ConsoleObserver<'a> {
    console: &'a mut dyn Console,
}

impl Observe for ConsoleObserver<'_> {
    fn enabled(&self) -> bool {
        true
    }

    fn record(&mut self, name: &str, fields: Vec<(&str, PyValue)>) {
        self.console.observe(name, fields);
    }
}

/// `open(path, "r", encoding="utf-8")` and `.read()` as the collector observes them: the open (`file_open`, and `file_open_error` with the path), the read
/// (`file_read`, then `file_read_return` with the text or `file_read_error`).
fn read_file(console: &mut dyn Console, path: &PyStr) -> Result<PyStr, Raised> {
    let mut keywords = PyDict::new();
    keywords.set("encoding", PyValue::str("utf-8"));
    console.observe(
        "file_open",
        vec![
            ("path", PyValue::Str(path.clone())),
            ("args", PyValue::List(vec![PyValue::str("r")])),
            ("kwargs", PyValue::Dict(keywords)),
        ],
    );
    let mut file = match open_text_file(path) {
        Ok(file) => file,
        Err(failure) => {
            let raised = Raised::from(failure);
            console.observe(
                "file_open_error",
                vec![
                    ("path", PyValue::Str(path.clone())),
                    ("type", PyValue::str(&raised.class)),
                    ("message", PyValue::Str(raised.message.clone())),
                ],
            );
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
            console.observe(
                "file_read_error",
                vec![
                    ("type", PyValue::str(&raised.class)),
                    ("message", PyValue::Str(raised.message.clone())),
                ],
            );
            Err(raised)
        }
    }
}

/// `json.load(open(path, "r", encoding="utf-8"))`: a decoding failure is the exception of the load (`Expecting value` is raised `from None`).
fn load_json(console: &mut dyn Console, path: &PyStr) -> Result<PyValue, Raised> {
    let text = read_file(console, path)?;
    loads_text(&text).map_err(|error| {
        let mut raised = Raised::from(load_failure(error));
        if raised.class == "JSONDecodeError"
            && raised
                .message
                .to_utf8()
                .is_some_and(|message| message.starts_with("Expecting value"))
        {
            raised.suppress_context = true;
        }
        raised
    })
}

fn text_option(values: &crate::transform_cli::Values, dest: &str) -> Option<PyStr> {
    match values.get(dest) {
        Some(Value::Text(text)) => Some(text.clone()),
        _ => None,
    }
}

/// `_main(argv)`: how it ends, or the exception it ends with.
pub fn execute(
    console: &mut dyn Console,
    args: &[PyStr],
    columns: Option<&str>,
) -> Result<Exit, Raised> {
    let values = match parse_command(console, &spec(), args, columns) {
        Ok(values) => values,
        Err(exit) => return Ok(exit),
    };
    let base_url =
        text_option(&values, "base_url").unwrap_or_else(|| PyStr::new("http://127.0.0.1:8903"));
    let model = text_option(&values, "model").unwrap_or_default();
    let timeout = match values.get("timeout") {
        Some(Value::Float(seconds)) => *seconds,
        _ => 60.0,
    };
    let candidate = match text_option(&values, "candidate").filter(|path| !path.is_empty()) {
        Some(path) => load_json(console, &path)?,
        None => PyValue::Dict(PyDict::new()),
    };
    let mock = match text_option(&values, "mock").filter(|path| !path.is_empty()) {
        Some(path) => load_json(console, &path)?,
        None => PyValue::None,
    };
    let result = smoke(
        &mut ConsoleObserver {
            console: &mut *console,
        },
        &Constants::canonical(),
        &PyValue::Str(base_url),
        &PyValue::Str(model),
        &candidate,
        &PyValue::Float(timeout),
        &mock,
    )?;
    let shown = result.dumps_pretty().map_err(|error| {
        let class = error.python_type();
        let text = error.to_string();
        Raised::plain(
            class,
            text.strip_prefix(&format!("{class}: ")).unwrap_or(&text),
        )
    })?;
    console.print_stdout(&shown);
    Ok(Exit {
        status: 0,
        context: None,
    })
}

/// The program as a process.
pub fn run_process(args: &[PyStr], columns: Option<&str>) -> i32 {
    let mut console = ProcessConsole::new();
    let outcome = execute(&mut console, args, columns);
    let status = finish(&mut console, outcome);
    console.conclude(status)
}
