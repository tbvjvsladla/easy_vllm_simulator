//! The classifier dependency of the escalation owner: the canonical classifier asset reached the way the original's
//! `_load_classifier()` reaches it, from Rust, with no Python.
//!
//! The production programs locate the Rust classifier owner's launcher `classify_cell_api.sh` two skills over and refuse when it is
//! not a readable file (no `.py` is required — [`owner_path`]). The reference fixture keeps the original's layout
//! ([`Bridge::original_layout`]): the original finds `classify_cell_native.py` there, refuses (`SystemExit`) when it is
//! not a file, and imports it; importing runs the module's last statement, one call of its launcher `classify_cell_api.sh constants`
//! (the existing Rust classifier owner), whose JSON becomes the module's globals (`KILL_EVENT_KINDS` among them). This bridge locates
//! the same asset from the installed layout, checks it is a file and readable at the same point, and makes the same launcher call.
//! `_utc` and `kill_events_in_window` are the module's two thin wrappers of one launcher call each (`evaluate-jsonl`); they are
//! reproduced as such. The classifier owner stays the existing one: no vocabulary, time or kill algorithm is here, and the asset's
//! Python is never run (only the fixed, approved launcher protocol is). The launcher's own environment choices
//! (`CLASSIFY_CELL_API_BIN` and the others) are inherited from the process, as they are in the original.

use crate::escalation::{Dependency, Loaded, Raised, CLASSIFIER_MODULE};
use crate::feedback::normpath;
use crate::model_files::{
    is_file, join, load_failure, loads_str, open_binary, os_failure, splitlines,
};
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pyjson::decode_file_text;
use agent_runtime_support::pystr::{is_python_space, PyStr};
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use num_bigint::BigInt;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::process::{Command, Stdio};

const ASSET_NAME: &str = "classify_cell_native.py";
const LAUNCHER_NAME: &str = "classify_cell_api.sh";
/// The request id the original's transport uses, and the encoding it demands back.
const REQUEST_ID: &str = "python-consumer";
const ENCODING: &str = "json-values-v1";

/// Exception classes a classifier record may name besides `ClassifyError` (the builtin ones the original's `_raise` can build).
const BUILTIN_ERRORS: [&str; 22] = [
    "Exception",
    "ArithmeticError",
    "AssertionError",
    "AttributeError",
    "ImportError",
    "IndexError",
    "KeyError",
    "LookupError",
    "NotImplementedError",
    "OSError",
    "FileNotFoundError",
    "PermissionError",
    "IsADirectoryError",
    "NotADirectoryError",
    "OverflowError",
    "RecursionError",
    "RuntimeError",
    "SystemExit",
    "TypeError",
    "UnicodeDecodeError",
    "ValueError",
    "ZeroDivisionError",
];

/// Where the original's asset is, from the root of the skill this program belongs to (`<skills>/adversarial-benchmark/scripts/...`):
/// `classify_cell_native.py`, the file the saved reference cases vary (absent, a directory, unreadable). Only the reference fixture
/// ([`Bridge::original_layout`]) reads it; the production programs locate the Rust owner's launcher instead ([`owner_path`]).
pub fn asset_path(skill_root: &PyStr) -> PyStr {
    let relative = format!("../adversarial-benchmark/scripts/{ASSET_NAME}");
    normpath(&join(skill_root, &PyStr::new(&relative)))
}

/// The canonical classifier of a production run: the Rust classifier owner's launcher `classify_cell_api.sh`, the program every
/// call goes through (intentional change, plan_26111615: the original located the `.py` adapter beside it only to reach this
/// launcher, so a checkout without harness `.py` would have refused although its owner is present). Its absence or an unreadable
/// file is the same refusal the original gave for the adapter, naming this path.
pub fn owner_path(skill_root: &PyStr) -> PyStr {
    let relative = format!("../adversarial-benchmark/scripts/{LAUNCHER_NAME}");
    normpath(&join(skill_root, &PyStr::new(&relative)))
}

/// The launcher that belongs to an asset: its neighbour, with symbolic links resolved as `Path.resolve()` resolves them.
fn launcher_of(asset: &PyStr) -> PyStr {
    let bytes = asset.to_surrogateescape_bytes().unwrap_or_default();
    let path = std::path::Path::new(std::ffi::OsStr::from_bytes(&bytes));
    let resolved = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let neighbour = resolved.with_file_name(LAUNCHER_NAME);
    PyStr::from_surrogateescape(neighbour.as_os_str().as_bytes())
}

/// The outcome of one launcher run, as `subprocess.run(..., capture_output=True, text=True)` returns it.
struct Finished {
    code: i64,
    stdout: PyStr,
    stderr: PyStr,
}

/// The canonical classifier, reached through its launcher.
pub struct Bridge {
    asset: PyStr,
    recording: bool,
    trace: Vec<PyValue>,
    rpc: Vec<PyValue>,
    runs: u64,
    succeeded: u64,
    stderr: Vec<PyStr>,
}

fn event(name: &str, fields: Vec<(&str, PyValue)>) -> PyValue {
    let mut row = PyDict::new();
    row.set("event", PyValue::str(name));
    for (key, value) in fields {
        row.set(key, value);
    }
    PyValue::Dict(row)
}

impl Bridge {
    /// The bridge of the installed layout of the program whose skill root is `skill_root`: the Rust owner's launcher
    /// ([`owner_path`]); no `.py` is required.
    pub fn installed(skill_root: &PyStr) -> Bridge {
        Bridge::for_asset(owner_path(skill_root))
    }

    /// The bridge of the original's layout (`classify_cell_native.py` beside the launcher, [`asset_path`]): only the reference
    /// fixture uses it, over trees built from the committed reference bundle, so the saved cases that vary that file keep
    /// their meaning. Production programs use [`Bridge::installed`].
    pub fn original_layout(skill_root: &PyStr) -> Bridge {
        Bridge {
            asset: asset_path(skill_root),
            recording: false,
            trace: Vec::new(),
            rpc: Vec::new(),
            runs: 0,
            succeeded: 0,
            stderr: Vec::new(),
        }
    }

    /// The bridge of an explicitly named asset (a validation input of a check or a benchmark row; the programs use [`Bridge::installed`]).
    pub fn for_asset(asset: PyStr) -> Bridge {
        Bridge {
            asset,
            recording: false,
            trace: Vec::new(),
            rpc: Vec::new(),
            runs: 0,
            succeeded: 0,
            stderr: Vec::new(),
        }
    }

    /// Keep the ordered trace and every launcher run (the control record of a check; a production run records nothing).
    pub fn recording(mut self) -> Bridge {
        self.recording = true;
        self
    }

    pub fn asset(&self) -> &PyStr {
        &self.asset
    }

    pub fn trace(&self) -> &[PyValue] {
        &self.trace
    }

    pub fn rpc(&self) -> &[PyValue] {
        &self.rpc
    }

    /// How many launcher runs were attempted and how many of them succeeded (exit 0, nothing on standard error): counters only,
    /// kept in every run and never growing a trace.
    pub fn counters(&self) -> (u64, u64) {
        (self.runs, self.succeeded)
    }

    /// Records a trace event of the caller (an event file open, ...), when recording.
    pub fn note(&mut self, name: &str, fields: Vec<(&str, PyValue)>) {
        if self.recording {
            self.trace.push(event(name, fields));
        }
    }

    /// `bash <launcher> <argv>` with the payload as standard input (standard input is inherited without one).
    fn run(
        &mut self,
        launcher: &PyStr,
        argv: &[&str],
        payload: Option<&str>,
    ) -> Result<Finished, Raised> {
        let operation = payload
            .and_then(|text| loads_str(&PyStr::new(text.lines().next().unwrap_or(""))).ok())
            .and_then(|request| match request {
                PyValue::Dict(map) => map.get("operation").cloned(),
                _ => None,
            })
            .and_then(|value| match value {
                PyValue::Str(text) => text.to_utf8(),
                _ => None,
            })
            .unwrap_or_else(|| argv.first().copied().unwrap_or("").to_string());
        let index = usize::try_from(self.runs).unwrap_or(usize::MAX);
        self.runs += 1;
        self.note(
            "rpc",
            vec![
                ("index", PyValue::int(index)),
                ("operation", PyValue::str(&operation)),
            ],
        );
        let launcher_bytes = launcher.to_surrogateescape_bytes().unwrap_or_default();
        let mut command = Command::new("bash");
        command
            .arg(std::ffi::OsStr::from_bytes(&launcher_bytes))
            .args(argv)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command.stdin(if payload.is_some() {
            Stdio::piped()
        } else {
            Stdio::inherit()
        });
        let mut child = command
            .spawn()
            .map_err(|error| Raised::from(os_failure(&error, Some(&PyStr::new("bash")))))?;
        let feeder = payload.map(|text| {
            let mut input = child.stdin.take().expect("a piped standard input");
            let bytes = text.as_bytes().to_vec();
            std::thread::spawn(move || {
                let _ = input.write_all(&bytes);
            })
        });
        let output = child
            .wait_with_output()
            .map_err(|error| Raised::from(os_failure(&error, None)))?;
        if let Some(thread) = feeder {
            let _ = thread.join();
        }
        let decode = |bytes: &[u8]| {
            decode_file_text(bytes).map_err(|error| Raised::from(load_failure(error)))
        };
        let code = {
            use std::os::unix::process::ExitStatusExt;
            match (output.status.code(), output.status.signal()) {
                (Some(code), _) => i64::from(code),
                (None, Some(signal)) => -i64::from(signal),
                (None, None) => -1,
            }
        };
        let (stdout, stderr) = (decode(&output.stdout)?, decode(&output.stderr)?);
        if self.recording {
            let mut argv_row = vec![PyValue::str("bash"), PyValue::Str(launcher.clone())];
            argv_row.extend(argv.iter().map(|part| PyValue::str(part)));
            let mut row = PyDict::new();
            row.set("index", PyValue::int(index));
            row.set("argv", PyValue::List(argv_row));
            row.set("operation", PyValue::str(&operation));
            row.set("input", payload.map_or(PyValue::None, PyValue::str));
            row.set("exit_code", PyValue::int(code));
            row.set("stdout", PyValue::Str(stdout.clone()));
            row.set("stderr", PyValue::Str(stderr.clone()));
            self.rpc.push(PyValue::Dict(row));
            self.trace.push(event(
                "rpc_return",
                vec![
                    ("index", PyValue::int(index)),
                    ("exit_code", PyValue::int(code)),
                ],
            ));
        }
        if code == 0 && stderr.is_empty() {
            self.succeeded += 1;
        }
        Ok(Finished {
            code,
            stdout,
            stderr,
        })
    }

    /// `_run(argv, payload)` of the module: a failed or noisy run is `RuntimeError`.
    fn transport(
        &mut self,
        launcher: &PyStr,
        argv: &[&str],
        payload: Option<&str>,
    ) -> Result<PyStr, Raised> {
        let finished = self.run(launcher, argv, payload)?;
        if finished.code != 0 || !finished.stderr.is_empty() {
            let mut message = PyStr::new(&format!(
                "classifier native transport failed ({}): ",
                finished.code
            ));
            let shown = finished.stderr.code_points();
            let end = shown
                .iter()
                .rposition(|&point| !is_python_space(point))
                .map_or(0, |at| at + 1);
            message.push_py(&PyStr::from_code_points(shown[..end].to_vec()));
            return Err(Raised::plain("RuntimeError", "").with_message(message));
        }
        Ok(finished.stdout)
    }

    /// One `evaluate-jsonl` call of the module's `_call(operation, arguments)`.
    fn call(
        &mut self,
        operation: &str,
        arguments: Vec<(&str, PyValue)>,
    ) -> Result<PyValue, Raised> {
        let mut encoded = PyDict::new();
        for (name, value) in arguments {
            let text = value.dumps_ascii().map_err(|error| {
                Raised::from(Failure::new(error.python_type(), &error.to_string()))
            })?;
            encoded.set(name, PyValue::str(&text));
        }
        let mut request = PyDict::new();
        request.set("id", PyValue::str(REQUEST_ID));
        request.set("operation", PyValue::str(operation));
        request.set("arguments_json", PyValue::Dict(encoded));
        request.set("result_json", PyValue::Bool(true));
        let payload = PyValue::Dict(request)
            .dumps_ascii()
            .map_err(|error| Raised::from(Failure::new(error.python_type(), &error.to_string())))?
            + "\n";
        let launcher = launcher_of(&self.asset);
        let stdout = self.transport(&launcher, &["evaluate-jsonl"], Some(&payload))?;
        let lines = splitlines(&stdout);
        if lines.len() != 1 {
            return Err(Raised::plain(
                "RuntimeError",
                "classifier native transport did not return one record",
            ));
        }
        let record = loads_str(&lines[0]).map_err(Raised::from)?;
        let PyValue::Dict(record) = record else {
            return Err(Raised::plain(
                "AttributeError",
                &format!("'{}' object has no attribute 'get'", record.type_name()),
            ));
        };
        let is_text = |key: &str, want: &str| matches!(record.get(key), Some(PyValue::Str(text)) if text.equals(want));
        if !is_text("id", REQUEST_ID) || !is_text("encoding", ENCODING) {
            return Err(Raised::plain(
                "RuntimeError",
                "classifier native binary lacks the required JSON transport; rebuild it",
            ));
        }
        if let Some(PyValue::Str(text)) = record.get("stderr") {
            if !text.is_empty() {
                self.stderr.push(text.clone());
            }
        }
        match record.get("kind") {
            Some(PyValue::Str(kind)) if kind.equals("value") => match record.get("value_json") {
                Some(PyValue::Str(text)) => loads_str(text).map_err(Raised::from),
                _ => Err(Raised::plain("KeyError", "'value_json'")),
            },
            Some(PyValue::Str(kind)) if kind.equals("exception") => Err(reported(&record)),
            _ => Err(Raised::plain(
                "RuntimeError",
                "classifier native transport returned an unknown record kind",
            )),
        }
    }
}

impl Raised {
    fn with_message(mut self, message: PyStr) -> Raised {
        self.message = message;
        self
    }
}

/// `_raise(record)`: the exception the classifier owner reported, as the module raises it.
fn reported(record: &PyDict) -> Raised {
    let text = |key: &str| match record.get(key) {
        Some(PyValue::Str(text)) => text.clone(),
        _ => PyStr::default(),
    };
    let (name, message) = (text("type").to_utf8().unwrap_or_default(), text("message"));
    if name == "ClassifyError" {
        return Raised::new("ClassifyError", CLASSIFIER_MODULE, message);
    }
    match BUILTIN_ERRORS.iter().find(|known| **known == name) {
        Some(known) => {
            let mut raised = Raised::new(known, "builtins", message.clone());
            if *known == "SystemExit" {
                raised.code = Some(Box::new(PyValue::Str(message)));
            }
            raised
        }
        None => {
            let mut line = PyStr::new(&format!("classifier native owner reported {name}: "));
            line.push_py(&message);
            Raised::new("RuntimeError", "builtins", line)
        }
    }
}

impl Dependency for Bridge {
    fn load(&mut self) -> Result<Loaded, Raised> {
        self.note(
            "classifier_load",
            vec![("path", PyValue::Str(self.asset.clone()))],
        );
        let outcome = self.load_inner();
        match &outcome {
            Ok(_) => self.note(
                "classifier_loaded",
                vec![
                    ("name", PyValue::str(CLASSIFIER_MODULE)),
                    ("file", PyValue::Str(self.asset.clone())),
                ],
            ),
            Err(raised) => self.note(
                "classifier_load_error",
                vec![
                    ("type", PyValue::str(&raised.class)),
                    ("message", PyValue::Str(raised.message.clone())),
                ],
            ),
        }
        outcome
    }

    fn utc(&mut self, value: &PyStr, field: &str) -> Result<PyValue, Raised> {
        self.call(
            "_utc",
            vec![
                ("value", PyValue::Str(value.clone())),
                ("field", PyValue::str(field)),
            ],
        )
    }

    fn kill_events(
        &mut self,
        lines: &[PyStr],
        started: &PyValue,
        ended: &PyValue,
        tolerance_s: &BigInt,
    ) -> Result<PyValue, Raised> {
        self.call(
            "kill_events_in_window",
            vec![
                (
                    "event_lines",
                    PyValue::List(lines.iter().cloned().map(PyValue::Str).collect()),
                ),
                ("started", started.clone()),
                ("ended", ended.clone()),
                ("tolerance_s", PyValue::Int(tolerance_s.clone())),
            ],
        )
    }

    fn take_stderr(&mut self) -> Vec<PyStr> {
        std::mem::take(&mut self.stderr)
    }

    fn record_event(&mut self, name: &str, fields: Vec<(&str, PyValue)>) {
        self.note(name, fields);
    }
}

impl Bridge {
    /// `_load_classifier()` without the trace.
    fn load_inner(&mut self) -> Result<Loaded, Raised> {
        if !is_file(&self.asset) {
            let mut message = PyStr::new("[escalation] FAIL: 사살 어휘의 정본을 찾지 못했다 — ");
            message.push_py(&self.asset);
            message.push_str(
                "\n  여기에 사본을 두지 않는다(거울 금지). adversarial-benchmark 배달을 확인하라.",
            );
            return Err(Raised::system_exit(message));
        }
        // Importing opens the source for reading: a file nobody may read is the `OSError` of that open.
        drop(open_binary(&self.asset).map_err(Raised::from)?);
        let launcher = launcher_of(&self.asset);
        let wrapped = |cause: Raised| {
            Raised::new("ImportError", "builtins", cause.message.clone()).from_cause(cause)
        };
        let stdout = match self.transport(&launcher, &["constants"], None) {
            Ok(stdout) => stdout,
            // `except (RuntimeError, ValueError, KeyError, TypeError)`: the import fails with the cause kept.
            Err(raised)
                if matches!(raised.class.as_str(), "RuntimeError" | "UnicodeDecodeError") =>
            {
                return Err(wrapped(raised))
            }
            Err(raised) => return Err(raised),
        };
        let bindings = match loads_str(&stdout) {
            Ok(value) => value,
            Err(failure) => return Err(wrapped(Raised::from(failure))),
        };
        let PyValue::Dict(bindings) = bindings else {
            return Err(Raised::plain(
                "AttributeError",
                &format!("'{}' object has no attribute 'items'", bindings.type_name()),
            ));
        };
        let kinds = match bindings.get("KILL_EVENT_KINDS") {
            Some(PyValue::List(items)) | Some(PyValue::Tuple(items)) => Some(items.clone()),
            _ => None,
        };
        Ok(Loaded {
            kill_event_kinds: kinds,
        })
    }
}
