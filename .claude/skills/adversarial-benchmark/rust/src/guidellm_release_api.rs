//! `resolve_guidellm_api`: the Rust-only surface of the release resolver, a separate program so that the
//! `resolve_guidellm` command line keeps answering exactly what the original did.
//!
//! * `evaluate-jsonl` - the functions of the original module as a transport. Each stdin line is
//!   `{id, operation, arguments[, fetch_fixture]}` where `operation` is a Python function name: `version_from_tag`,
//!   `image_ref`, `resolution`, `fetch_latest`, `merge_record`, `main`, `_self_test`. A line yields
//!   `{id, kind: value, value}` or `{id, kind: exception, type, message}`, plus `stderr`/`stdout` when the call wrote to
//!   those streams, and `request` (`{url, method, headers, timeout}`) when the call asked a transport for a release.
//!   Arguments follow the Python signatures (names, defaults and the `TypeError` a wrong call raises). `merge_record`
//!   takes the record path as text (the wire form of a path); an integer file descriptor is not supported. Opt-in:
//!   `arguments_json` (each argument as raw JSON text, decoded separately) and `result_json: true` (the result as
//!   compact ASCII JSON text in `value_json`).
//! * `fetch_fixture` is the injected transport of this program only - it never touches the network: `{response_hex}` is
//!   the body of the answer (hex), `{failure: {type, message}}` the exception the opener raises (an `OSError` class).
//!   The same production `fetch_latest` runs over it. Without a fixture, `fetch_latest` and a `main` that needs the
//!   latest release raise `RuntimeError` - the API never reaches the network. The `opener` argument of the original
//!   (a Python callable) has no wire form and is refused.
//! * `constants` - the module constants as one JSON object, read from this crate.
//! * `native-help` - this list.

use crate::cli::{on_large_stack, write_stderr, Streams};
use crate::dataset_prep::Recorder;
use crate::guidellm::Failure;
use crate::guidellm_release::{
    default_record_path, fetch_latest, image_ref, merge_record, os_error_class_named, resolution,
    version_from_tag, ReleaseRequest, ReleaseTransport, API_ORIGIN, DIGEST_SOURCE, IMAGE_HOST,
    RC_ARGS, RC_OK, RC_UNRESOLVED, RECORD_PATH_ENV, REPO_DEFAULT, REQUEST_ACCEPT,
    REQUEST_USER_AGENT,
};
use crate::guidellm_release_cli::main_with;
use crate::guidellm_release_selftest::self_test;
use crate::pyjson::{decode_file_text, loads_objects};
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use crate::repeat_api_cli::{
    exception_record, load_failure, print_failed, row_arguments, Signature, Wire,
};
use std::cell::RefCell;
use std::io::{Cursor, Read};

const NATIVE_HELP: &str = "resolve_guidellm_api Rust-only commands:\n  resolve_guidellm_api evaluate-jsonl   JSON lines on stdin: {id, operation, arguments[, fetch_fixture]} (API differential transport)\n  resolve_guidellm_api constants        the module constants as one JSON object\n  resolve_guidellm_api native-help      this text\n";

/// Runs the program and returns its exit status. `args` excludes the program name.
pub fn run(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    let status = match args.first().and_then(PyStr::to_utf8).as_deref() {
        Some("evaluate-jsonl") if args.len() == 1 => run_jsonl(io),
        Some("constants") if args.len() == 1 => {
            crate::repeat_api_cli::print_text(io, &constants().dumps_ascii().unwrap_or_default())
        }
        Some("native-help") if args.len() == 1 => match io.print_lines(&PyStr::new(NATIVE_HELP)) {
            Ok(()) => 0,
            Err(failure) => print_failed(io, &failure),
        },
        _ => {
            write_stderr(io, &PyStr::new(NATIVE_HELP));
            2
        }
    };
    io.finish(status)
}

/// The constants of the original module, from this crate. `RECORD_PATH` is the default record as this process
/// resolves it (the environment variable [`RECORD_PATH_ENV`], else the record beside the crate's folder).
pub fn constants() -> PyValue {
    let mut map = PyDict::new();
    map.set("REPO_DEFAULT", PyValue::str(REPO_DEFAULT));
    map.set("IMAGE_HOST", PyValue::str(IMAGE_HOST));
    map.set(
        "LATEST_API",
        PyValue::str(&format!("{API_ORIGIN}/repos/%s/releases/latest")),
    );
    map.set("RECORD_PATH", PyValue::Str(default_record_path()));
    map.set("RC_OK", PyValue::int(RC_OK));
    map.set("RC_ARGS", PyValue::int(RC_ARGS));
    map.set("RC_UNRESOLVED", PyValue::int(RC_UNRESOLVED));
    map.set("RECORD_PATH_ENV", PyValue::str(RECORD_PATH_ENV));
    map.set("DIGEST_SOURCE", PyValue::str(DIGEST_SOURCE));
    map.set("REQUEST_ACCEPT", PyValue::str(REQUEST_ACCEPT));
    map.set("REQUEST_USER_AGENT", PyValue::str(REQUEST_USER_AGENT));
    PyValue::Dict(map)
}

const VERSION_FROM_TAG: Signature = Signature {
    function: "version_from_tag",
    positional: &["tag"],
    keyword_only: &[],
    optional: &[],
};
const IMAGE_REF: Signature = Signature {
    function: "image_ref",
    positional: &["version", "repo"],
    keyword_only: &[],
    optional: &["repo"],
};
const RESOLUTION: Signature = Signature {
    function: "resolution",
    positional: &["version", "source", "repo", "published_utc"],
    keyword_only: &[],
    optional: &["repo", "published_utc"],
};
const FETCH_LATEST: Signature = Signature {
    function: "fetch_latest",
    positional: &["repo", "opener"],
    keyword_only: &[],
    optional: &["repo", "opener"],
};
const MERGE_RECORD: Signature = Signature {
    function: "merge_record",
    positional: &["path", "res", "digest"],
    keyword_only: &[],
    optional: &["digest"],
};
const MAIN: Signature = Signature {
    function: "main",
    positional: &["argv"],
    keyword_only: &[],
    optional: &["argv"],
};
const SELF_TEST: Signature = Signature {
    function: "_self_test",
    positional: &[],
    keyword_only: &[],
    optional: &[],
};
const CONSTANTS: Signature = Signature {
    function: "constants",
    positional: &[],
    keyword_only: &[],
    optional: &[],
};

/// The transport of a program that must not reach the network: every request is refused with a `RuntimeError`.
pub struct RefusedTransport;

impl ReleaseTransport for RefusedTransport {
    fn open(&self, _request: &ReleaseRequest) -> Result<Box<dyn Read>, Failure> {
        Err(Failure::new(
            "RuntimeError",
            "the API transport has no network: give the row a fetch_fixture",
        ))
    }
}

/// What an injected release answer does when it is asked: a body, or the exception of the opener.
pub struct FixtureTransport {
    answer: Result<Vec<u8>, Failure>,
    asked: RefCell<Vec<ReleaseRequest>>,
}

impl FixtureTransport {
    pub fn new(answer: Result<Vec<u8>, Failure>) -> FixtureTransport {
        FixtureTransport {
            answer,
            asked: RefCell::new(Vec::new()),
        }
    }

    /// The requests the transport was asked for, in order.
    pub fn requests(&self) -> Vec<ReleaseRequest> {
        self.asked.borrow().clone()
    }
}

impl ReleaseTransport for FixtureTransport {
    fn open(&self, request: &ReleaseRequest) -> Result<Box<dyn Read>, Failure> {
        self.asked.borrow_mut().push(request.clone());
        match &self.answer {
            Ok(body) => Ok(Box::new(Cursor::new(body.clone()))),
            Err(failure) => Err(failure.clone()),
        }
    }
}

fn fixture_error(message: &str) -> Failure {
    Failure::new("TypeError", &format!("fetch_fixture: {message}"))
}

fn hex_bytes(text: &PyStr) -> Result<Vec<u8>, Failure> {
    let text = text
        .to_utf8()
        .filter(|text| text.len() % 2 == 0 && text.is_ascii())
        .ok_or_else(|| fixture_error("response_hex is not an even-length hex string"))?;
    (0..text.len())
        .step_by(2)
        .map(|at| {
            u8::from_str_radix(&text[at..at + 2], 16)
                .map_err(|_| fixture_error("response_hex is not an even-length hex string"))
        })
        .collect()
}

/// The injected answer of a row's `fetch_fixture`.
pub fn parse_fixture(fixture: &PyValue) -> Result<FixtureTransport, Failure> {
    let PyValue::Dict(map) = fixture.peel() else {
        return Err(fixture_error("must be an object"));
    };
    match (map.get("response_hex"), map.get("failure")) {
        (Some(PyValue::Str(hex)), None) => Ok(FixtureTransport::new(Ok(hex_bytes(hex)?))),
        (None, Some(PyValue::Dict(failure))) => {
            let (Some(PyValue::Str(class)), Some(PyValue::Str(message))) =
                (failure.get("type"), failure.get("message"))
            else {
                return Err(fixture_error("failure needs string type and message"));
            };
            let class = class
                .to_utf8()
                .and_then(|name| os_error_class_named(&name))
                .ok_or_else(|| {
                    fixture_error(
                        "failure.type must be an OSError class (URLError, TimeoutError, ...)",
                    )
                })?;
            Ok(FixtureTransport::new(Err(Failure::with_message(
                class,
                message.clone(),
            ))))
        }
        _ => Err(fixture_error(
            "needs exactly one of response_hex (string) and failure (object)",
        )),
    }
}

/// `main(argv)`'s argument list (absent: the process's own arguments, which a transport has none of).
fn argv_of(value: &PyValue) -> Result<Vec<PyStr>, Failure> {
    match value.peel() {
        PyValue::None => Ok(Vec::new()),
        PyValue::List(items) | PyValue::Tuple(items) => items
            .iter()
            .map(|item| match item.peel() {
                PyValue::Str(text) => Ok(text.clone()),
                other => Err(Failure::new(
                    "TypeError",
                    &format!("argument of type '{}' is not iterable", other.type_name()),
                )),
            })
            .collect(),
        other => Err(Failure::new(
            "TypeError",
            &format!("'{}' object is not iterable", other.type_name()),
        )),
    }
}

/// One operation, with what it wrote to the two standard streams kept in `streams`. The transport never forwards the
/// operation's own exit: a `SystemExit` is reported as the exception it is. Releases are asked of `transport`.
pub fn evaluate_operation(
    operation: &str,
    arguments: &PyDict,
    streams: &mut Recorder,
    transport: &dyn ReleaseTransport,
) -> Result<PyValue, Failure> {
    let default_repo = PyValue::str(REPO_DEFAULT);
    match operation {
        "version_from_tag" => {
            let call = VERSION_FROM_TAG.bind(arguments)?;
            Ok(PyValue::Str(version_from_tag(call.get_ref("tag"))?))
        }
        "image_ref" => {
            let call = IMAGE_REF.bind(arguments)?;
            Ok(PyValue::Str(image_ref(
                call.get_ref("version"),
                call.get_or_ref("repo", &default_repo),
            )))
        }
        "resolution" => {
            let call = RESOLUTION.bind(arguments)?;
            Ok(PyValue::Dict(resolution(
                call.get_ref("version"),
                call.get_ref("source"),
                call.get_or_ref("repo", &default_repo),
                call.get_ref("published_utc"),
            )))
        }
        "fetch_latest" => {
            let call = FETCH_LATEST.bind(arguments)?;
            if !matches!(call.get_ref("opener").peel(), PyValue::None) {
                return Err(Failure::new(
                    "TypeError",
                    "an opener callable has no wire form: give the row a fetch_fixture",
                ));
            }
            fetch_latest(call.get_or_ref("repo", &default_repo), transport)
        }
        "merge_record" => {
            let call = MERGE_RECORD.bind(arguments)?;
            merge_record(
                call.get_ref("path"),
                call.get_ref("res"),
                call.get_ref("digest"),
            )
        }
        "main" => {
            let call = MAIN.bind(arguments)?;
            let args = argv_of(call.get_ref("argv"))?;
            Ok(PyValue::int(main_with(&args, streams, transport)?))
        }
        "_self_test" => {
            SELF_TEST.bind(arguments)?;
            Ok(PyValue::int(self_test(streams)?))
        }
        "constants" => {
            CONSTANTS.bind(arguments)?;
            Ok(constants())
        }
        other => Err(Failure::new(
            "ValueError",
            &format!("unknown operation {}", PyStr::new(other).repr()),
        )),
    }
}

fn row_operation(row: &PyDict) -> Result<String, Failure> {
    match row.get("operation") {
        Some(PyValue::Str(name)) => Ok(name.to_utf8().unwrap_or_default()),
        Some(other) => Ok(other.repr()),
        None => Err(Failure::new("KeyError", "'operation'")),
    }
}

/// The record with the streams the call wrote to and the release request it made, when there were any.
fn with_extras(record: PyValue, streams: &Recorder, requests: &[ReleaseRequest]) -> PyValue {
    let PyValue::Dict(mut map) = record else {
        return record;
    };
    if let Some(first) = requests.first() {
        map.set("request", first.to_value());
    }
    if !streams.stderr.is_empty() {
        map.set("stderr", PyValue::Str(streams.stderr.clone()));
    }
    if !streams.stdout.is_empty() {
        map.set("stdout", PyValue::Str(streams.stdout.clone()));
    }
    PyValue::Dict(map)
}

fn evaluate_row(row: &PyValue) -> PyValue {
    let PyValue::Dict(map) = row else {
        return exception_record(
            PyValue::None,
            &Failure::new("TypeError", "operation row is not an object"),
            Wire::Values,
        );
    };
    let id = map.get("id").cloned().unwrap_or(PyValue::None);
    let wire = match map.get("result_json") {
        Some(PyValue::Bool(true)) => Wire::JsonText,
        _ => Wire::Values,
    };
    let mut streams = Recorder::default();
    let fixture = map.get("fetch_fixture").map(parse_fixture).transpose();
    let mut asked: Vec<ReleaseRequest> = Vec::new();
    let outcome = (|| {
        let fixture = fixture?;
        let operation = row_operation(map)?;
        let arguments = row_arguments(map, true)?;
        let result = match &fixture {
            Some(injected) => evaluate_operation(&operation, &arguments, &mut streams, injected),
            None => evaluate_operation(&operation, &arguments, &mut streams, &RefusedTransport),
        };
        if let Some(injected) = &fixture {
            asked = injected.requests();
        }
        result
    })();
    let record = match outcome {
        Ok(value) => crate::repeat_api_cli::value_record(id, value, wire),
        Err(failure) => exception_record(id, &failure, wire),
    };
    with_extras(record, &streams, &asked)
}

fn run_jsonl(io: &mut Streams<'_>) -> i32 {
    let mut bytes = Vec::new();
    if let Err(error) = io.stdin.read_to_end(&mut bytes) {
        write_stderr(
            io,
            &PyStr::new(&format!("[resolve_guidellm_api] ERROR stdin: {error}\n")),
        );
        return 2;
    }
    // The transport is deliberately strict UTF-8 in and ASCII out; it is not the legacy stream behaviour.
    let text = match decode_file_text(&bytes) {
        Ok(text) => text
            .to_utf8()
            .expect("strict decoding yields no surrogates"),
        Err(error) => {
            write_stderr(
                io,
                &PyStr::new(&format!("[resolve_guidellm_api] ERROR stdin: {error}\n")),
            );
            return 2;
        }
    };
    let output = on_large_stack(|| {
        let mut out = String::new();
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            let record = match loads_objects(line) {
                Ok(row) => evaluate_row(&row),
                Err(error) => exception_record(PyValue::None, &load_failure(&error), Wire::Values),
            };
            match record.dumps_ascii() {
                Ok(json) => out.push_str(&json),
                Err(error) => {
                    let id = match &record {
                        PyValue::Dict(map) => map.get("id").cloned().unwrap_or(PyValue::None),
                        _ => PyValue::None,
                    };
                    let text = error.to_string();
                    let message = text
                        .strip_prefix(&format!("{}: ", error.python_type()))
                        .unwrap_or(&text);
                    out.push_str(
                        &exception_record(
                            id,
                            &Failure::new(error.python_type(), message),
                            Wire::Values,
                        )
                        .dumps_ascii()
                        .unwrap_or_default(),
                    );
                }
            }
            out.push('\n');
        }
        out
    });
    match io.print_lines(&PyStr::new(&output)) {
        Ok(()) => 0,
        Err(failure) => print_failed(io, &failure),
    }
}
