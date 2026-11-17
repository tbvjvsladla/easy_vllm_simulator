//! Private bridge of the in-process importers of `manifest_contract.py` (`render_sub_env`, `scan_node`, `render_dockerfile`,
//! `harness_permission_probe`, `publish_install_request`, the claim predicates, the hint evidence loader): one request on standard input,
//! one answer on standard output, selected by `MANIFEST_CONTRACT_BRIDGE` in the `manifest_contract` executable (the public command line gains
//! no argument and the bin count does not change).
//!
//! **Wire (`manifest-contract-bridge/1`).** Values are the typed graph of [`crate::typed_wire`]. Answers: `returned` (a plan, a value, a
//! reference or the main answer), `raised` (the exception class and the constructor facts that rebuild the exact object; the traceback frames are
//! NOT carried: U2 stays open), `unsupported` (a shape outside the supported typed set, reported before any business, never answered by guessing).
//! The process exits 0 for all three: the transport succeeded, the API outcome is data.
//!
//! The Python side marshals types and assembles results from its ORIGINAL input objects; every decision (flag, reason, exit code, tp, warnings,
//! which role names which entry, truthiness, topology comparison, git branch, grammar classification of `main` argv) is made here.

use crate::manifest_contract::Environment;
use crate::typed_wire::{
    field, hex_encode, int_of, items_of, str_value, tagged, text_of, Decoder, Encoder,
};
use agent_runtime_support::pyjson::loads_text;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use agent_runtime_support::pyyaml::{YamlDetail, YamlError};
use role_contract::load::LoadFailure;
use role_contract::manifest_gate::api::{
    effective_model_source_which, evaluate_contract_plan, load_manifest_answer, manifest_tp_answer,
    resolve_topology_answer, PlanEntry, RoleSource, TopologyAnswer, ValueSource,
};
use role_contract::manifest_gate::{GateFailure, Raised, Step};
use role_contract::manifest_gate_cli::{execute, parse_exact, GitBranch};
use role_contract::pyprofile;
use std::io::Write;

pub const BRIDGE_ENV: &str = "MANIFEST_CONTRACT_BRIDGE";
/// The file the REQUEST arrives in (a private file the adapter wrote with mode 0600 and removes after the answer): no descriptor other than
/// the caller's own is handed to this process, so neither this process nor the `git` child of `resolve_topology` inherits an extra one, and
/// the caller's standard input stays the caller's. Absent: the request is read from standard input (direct use of the bridge).
pub const BRIDGE_REQUEST_ENV: &str = "MANIFEST_CONTRACT_BRIDGE_REQUEST";
pub const PROTOCOL: &str = "manifest-contract-bridge/1";

/// The request bytes: from the file named by [`BRIDGE_REQUEST_ENV`] or, without the variable, from standard input.
pub fn read_request() -> Result<Vec<u8>, String> {
    use std::io::Read;
    match std::env::var_os(BRIDGE_REQUEST_ENV) {
        Some(path) if !path.is_empty() => std::fs::read(&path).map_err(|error| {
            format!(
                "the request file {} cannot be read: {error}",
                path.to_string_lossy()
            )
        }),
        _ => {
            let mut request = Vec::new();
            std::io::stdin()
                .lock()
                .read_to_end(&mut request)
                .map_err(|error| error.to_string())?;
            Ok(request)
        }
    }
}

fn object(entries: Vec<(&str, PyValue)>) -> PyValue {
    let mut dict = PyDict::new();
    for (key, value) in entries {
        dict.set(key, value);
    }
    PyValue::Dict(dict)
}

fn opt(value: Option<PyValue>) -> PyValue {
    value.unwrap_or(PyValue::None)
}

fn text_value(text: &PyStr) -> PyValue {
    let mut encoder = Encoder::new();
    encoder
        .encode(&PyValue::Str(text.clone()))
        .expect("text always encodes")
}

fn returned(entries: Vec<(&str, PyValue)>) -> PyValue {
    let mut all = vec![("status", PyValue::str("returned"))];
    all.extend(entries);
    object(all)
}

fn unsupported(boundary: &str, detail: &str) -> PyValue {
    object(vec![
        ("status", PyValue::str("unsupported")),
        ("boundary", PyValue::str(boundary)),
        ("detail", PyValue::str(detail)),
    ])
}

fn step_value(step: &Step) -> PyValue {
    match step {
        Step::Arg(index) => object(vec![("arg", PyValue::int(*index as i64))]),
        Step::Key(key) => object(vec![("key", PyValue::str(key))]),
        Step::Index(index) => object(vec![("index", PyValue::int(*index as i64))]),
    }
}

fn raised_gate(raised: &Raised) -> PyValue {
    object(vec![
        ("status", PyValue::str("raised")),
        ("cls", PyValue::str(raised.class)),
        ("kind", PyValue::str("plain")),
        ("message", text_value(&raised.message)),
        ("name", opt(raised.name.as_deref().map(PyValue::str))),
        (
            "obj",
            opt(raised
                .obj
                .as_ref()
                .map(|path| PyValue::List(path.iter().map(step_value).collect()))),
        ),
    ])
}

fn gate_failure(failure: &GateFailure) -> PyValue {
    match failure {
        GateFailure::Raised(raised) => raised_gate(raised),
        GateFailure::Unsupported { boundary, detail } => unsupported(boundary, detail),
    }
}

fn plain(cls: &str, message: &str) -> PyValue {
    object(vec![
        ("status", PyValue::str("raised")),
        ("cls", PyValue::str(cls)),
        ("kind", PyValue::str("plain")),
        ("message", str_value(message)),
        ("name", PyValue::None),
        ("obj", PyValue::None),
    ])
}

fn mark_value(mark: &agent_runtime_support::pyyaml::Mark6) -> PyValue {
    object(vec![
        ("name", text_value(&mark.name)),
        ("index", PyValue::int(mark.index as i64)),
        ("line", PyValue::int(mark.line as i64)),
        ("column", PyValue::int(mark.column as i64)),
        ("buffer", opt(mark.buffer.as_ref().map(text_value))),
        ("pointer", opt(mark.pointer.map(|p| PyValue::int(p as i64)))),
    ])
}

fn yaml_failure(error: &YamlError) -> PyValue {
    let mut entries = vec![
        ("status", PyValue::str("raised")),
        ("cls", PyValue::str(error.class)),
    ];
    match &error.detail {
        YamlDetail::Marked(marked) => {
            entries.push(("kind", PyValue::str("yaml_marked")));
            entries.push(("marks_shared", PyValue::Bool(marked.shared_marks)));
            entries.push(("context", opt(marked.context.as_ref().map(text_value))));
            entries.push((
                "context_mark",
                opt(marked.context_mark.as_ref().map(mark_value)),
            ));
            entries.push(("problem", opt(marked.problem.as_ref().map(text_value))));
            entries.push((
                "problem_mark",
                opt(marked.problem_mark.as_ref().map(mark_value)),
            ));
            entries.push(("note", opt(marked.note.as_ref().map(text_value))));
            // the number of positional arguments of the original construction: PyYAML's raise sites pass four and never a note
            entries.push((
                "args_arity",
                PyValue::int(if marked.note.is_some() { 5 } else { 4 }),
            ));
        }
        YamlDetail::Reader {
            name,
            position,
            character,
            encoding,
            reason,
        } => {
            entries.push(("kind", PyValue::str("yaml_reader")));
            entries.push(("source_name", text_value(name)));
            entries.push(("position", PyValue::int(*position as i64)));
            entries.push(("character", PyValue::int(i64::from(*character))));
            entries.push(("encoding", PyValue::str(encoding)));
            entries.push(("reason", PyValue::str(reason)));
        }
        YamlDetail::Decode {
            encoding,
            object: chunk,
            start,
            end,
            reason,
            ..
        } => {
            entries.push(("kind", PyValue::str("decode")));
            entries.push(("encoding", PyValue::str(encoding)));
            entries.push(("object", PyValue::str(&hex_encode(chunk))));
            entries.push(("start", PyValue::int(*start as i64)));
            entries.push(("end", PyValue::int(*end as i64)));
            entries.push(("reason", PyValue::str(reason)));
        }
        _ => {
            entries.push(("kind", PyValue::str("plain")));
        }
    }
    entries.push(("message", text_value(&error.message)));
    entries.push(("name", PyValue::None));
    entries.push(("obj", PyValue::None));
    object(entries)
}

fn load_failure(failure: &LoadFailure) -> PyValue {
    match failure {
        LoadFailure::NotPathLike(name) => plain(
            "TypeError",
            &format!("expected str, bytes or os.PathLike object, not {name}"),
        ),
        LoadFailure::EmbeddedNull => plain("ValueError", "embedded null byte"),
        LoadFailure::Os { error, filename } => {
            let number = error.raw_os_error().unwrap_or(0);
            let text = error.to_string();
            let strerror = text
                .rsplit_once(" (os error ")
                .map_or(text.clone(), |(head, _)| head.to_string());
            object(vec![
                ("status", PyValue::str("raised")),
                ("cls", PyValue::str("OSError")),
                ("kind", PyValue::str("os")),
                ("errno", PyValue::int(i64::from(number))),
                ("strerror", str_value(&strerror)),
                ("filename", text_value(filename)),
                ("message", str_value(&text)),
                ("name", PyValue::None),
                ("obj", PyValue::None),
            ])
        }
        LoadFailure::Yaml(error) => yaml_failure(error),
        LoadFailure::Unsupported { boundary, detail } => unsupported(boundary, detail),
    }
}

fn args_of(request: &PyValue) -> Result<Vec<PyValue>, String> {
    Ok(items_of(field(request, "args")?)?.to_vec())
}

fn plan_value(plan: &[PlanEntry]) -> Result<PyValue, String> {
    let mut entries = Vec::new();
    for entry in plan {
        entries.push(match entry {
            PlanEntry::Input { key, name } => object(vec![
                ("key", PyValue::str(key)),
                ("from", PyValue::str("input")),
                ("name", PyValue::str(name)),
            ]),
            PlanEntry::Value { key, value } => object(vec![
                ("key", PyValue::str(key)),
                ("from", PyValue::str("value")),
                ("v", Encoder::new().encode(value)?),
            ]),
            PlanEntry::EffectiveMap { key, entries: map } => object(vec![
                ("key", PyValue::str(key)),
                ("from", PyValue::str("effective_map")),
                (
                    "entries",
                    PyValue::List(
                        map.iter()
                            .map(|entry| {
                                object(vec![
                                    ("node", PyValue::int(entry.node as i64)),
                                    (
                                        "role",
                                        match &entry.role {
                                            RoleSource::NodeRole => PyValue::str("node_role"),
                                            RoleSource::Fallback(text) => {
                                                object(vec![("text", PyValue::str(text))])
                                            }
                                        },
                                    ),
                                    (
                                        "value",
                                        PyValue::str(match entry.value {
                                            ValueSource::NodeModelSource => "node_model_source",
                                            ValueSource::TopModelSource => "top_model_source",
                                        }),
                                    ),
                                ])
                            })
                            .collect(),
                    ),
                ),
            ]),
        });
    }
    Ok(PyValue::List(entries))
}

/// Text of a wire string as raw bytes the way Python's `os.fsencode` makes them (surrogateescape).
fn fs_bytes(value: &PyValue) -> Result<Vec<u8>, String> {
    match value.peel() {
        PyValue::Str(text) => text.to_surrogateescape_bytes().map_err(|e| e.message()),
        _ => Err("text expected".into()),
    }
}

fn argv_bytes(value: &PyValue) -> Result<Vec<Vec<u8>>, String> {
    match value.peel() {
        PyValue::List(items) => items.iter().map(fs_bytes).collect(),
        _ => Err("argv must be a list of text".into()),
    }
}

fn answer_main(args: &[PyValue]) -> PyValue {
    let argv = match args.first().map(argv_bytes) {
        Some(Ok(argv)) => argv,
        Some(Err(why)) => return unsupported("main_argv", &why),
        None => return unsupported("main_argv", "argv is required"),
    };
    let Some(request) = parse_exact(&argv) else {
        return unsupported("main_legacy_shape", "the adapter classifies this argv as a legacy shape before any business; it must not be sent to the native main");
    };
    match execute(
        &request,
        b"manifest_contract.sh",
        &mut GitBranch::from_process(),
    ) {
        Err(error) => unsupported(error.boundary, &error.detail),
        Ok(answer) => {
            if answer.crashed {
                return match &answer.exception {
                    Some((class, message)) => object(vec![
                        ("status", PyValue::str("raised")),
                        ("cls", PyValue::str(class)),
                        ("kind", PyValue::str("plain")),
                        ("message", text_value(message)),
                        ("name", PyValue::None),
                        ("obj", PyValue::None),
                    ]),
                    None => unsupported(
                        "main_crash_class",
                        "the native answer crashed without a modelled exception class",
                    ),
                };
            }
            returned(vec![
                ("rc", PyValue::int(i64::from(answer.status))),
                (
                    "stdout",
                    opt(answer
                        .stdout
                        .as_ref()
                        .map(|printed| text_value(&printed.text))),
                ),
                ("stderr", opt(answer.stderr.as_ref().map(text_value))),
            ])
        }
    }
}

fn serve_request(request: &PyValue) -> Result<PyValue, String> {
    let op = text_of(field(request, "op")?)?;
    let wires = args_of(request)?;
    let documents: Vec<&PyValue> = wires.iter().collect();
    let mut decoder = Decoder::new(&documents);
    let mut decode_all = |wires: &[PyValue]| -> Result<Vec<PyValue>, String> {
        wires.iter().map(|wire| decoder.decode(wire)).collect()
    };
    let need = |args: &[PyValue], count: usize| -> Result<(), String> {
        if args.len() == count {
            Ok(())
        } else {
            Err(format!("{op} takes {count} argument(s)"))
        }
    };
    Ok(match op.as_str() {
        "evaluate_contract" => {
            need(&wires, 2)?;
            let values = decode_all(&wires)?;
            match evaluate_contract_plan(&values[0], &values[1]) {
                Ok(plan) => returned(vec![("plan", plan_value(&plan)?)]),
                Err(failure) => gate_failure(&failure),
            }
        }
        "manifest_tp" => {
            need(&wires, 2)?;
            let values = decode_all(&wires)?;
            match manifest_tp_answer(&values[0], &values[1]) {
                Ok(value) => returned(vec![("value", Encoder::new().encode(&value)?)]),
                Err(failure) => gate_failure(&failure),
            }
        }
        "effective_model_source" => {
            need(&wires, 2)?;
            let values = decode_all(&wires)?;
            let which = match effective_model_source_which(&values[0], &values[1]) {
                ValueSource::NodeModelSource => "node_model_source",
                ValueSource::TopModelSource => "top_model_source",
            };
            returned(vec![("which", PyValue::str(which))])
        }
        "resolve_topology" => {
            need(&wires, 2)?;
            let values = decode_all(&wires)?;
            let repo = fs_bytes(&values[0]).map_err(|why| format!("repo: {why}"))?;
            match resolve_topology_answer(&mut GitBranch::from_process(), &repo, &values[1]) {
                TopologyAnswer::Explicit => returned(vec![("which", PyValue::str("explicit"))]),
                TopologyAnswer::Branch(branch) => returned(vec![
                    ("which", PyValue::str("value")),
                    (
                        "value",
                        branch.map_or(object(vec![("t", PyValue::str("none"))]), |text| {
                            str_value(&text)
                        }),
                    ),
                ]),
            }
        }
        "load_manifest" => {
            need(&wires, 1)?;
            let values = decode_all(&wires)?;
            match load_manifest_answer(&values[0]) {
                Ok(value) => returned(vec![("value", Encoder::new().encode(&value)?)]),
                Err(failure) => load_failure(&failure),
            }
        }
        "classify_main" => {
            need(&wires, 1)?;
            let values = decode_all(&wires)?;
            match argv_bytes(&values[0]) {
                Ok(argv) => returned(vec![(
                    "which",
                    PyValue::str(if parse_exact(&argv).is_some() {
                        "native"
                    } else {
                        "legacy"
                    }),
                )]),
                Err(why) => unsupported("main_argv", &why),
            }
        }
        "main" => {
            need(&wires, 1)?;
            let values = decode_all(&wires)?;
            answer_main(&values)
        }
        other => unsupported("operation", &format!("{other} is not a bridge operation")),
    })
}

fn event(
    env: &Environment,
    kind: &str,
    shape: Option<&str>,
    outcome: Option<&str>,
) -> Result<(), String> {
    let Some(path) = &env.observer else {
        return Ok(());
    };
    let line = object(vec![
        ("event", PyValue::str(kind)),
        ("route", PyValue::str("NativeBridge")),
        ("shape", shape.map_or(PyValue::None, PyValue::str)),
        ("pid", PyValue::int(i64::from(std::process::id()))),
        (
            "ppid",
            PyValue::int(i64::from(std::os::unix::process::parent_id())),
        ),
        ("outcome", outcome.map_or(PyValue::None, PyValue::str)),
    ]);
    let text = line.dumps_ascii().map_err(|error| error.to_string())?;
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| writeln!(file, "{text}"))
        .map_err(|error| format!("the route observation cannot be written: {error}"))
}

fn outcome_of(answer: &PyValue) -> String {
    match answer.peel() {
        PyValue::Dict(map) => match map.get("status").map(PyValue::peel) {
            Some(PyValue::Str(status)) => {
                let status = status.to_utf8().unwrap_or_default();
                match (status.as_str(), map.get("cls")) {
                    ("raised", Some(cls)) => format!("raised:{}", text_of(cls).unwrap_or_default()),
                    _ => status,
                }
            }
            _ => "unknown".into(),
        },
        _ => "unknown".into(),
    }
}

/// Answers one request. `Err` is a TRANSPORT failure (the request is not a valid bridge request).
pub fn serve(input: &[u8], env: &Environment) -> Result<String, String> {
    let profile = env
        .text_profile()
        .map_err(|reason| format!("the text profile is not modelled: {reason}"))?;
    let text = PyStr::from_surrogateescape(input);
    let request = loads_text(&text).map_err(|error| format!("the request is not JSON: {error}"))?;
    let shape = field(&request, "op").ok().and_then(|op| text_of(op).ok());
    let version = field(&request, "v").ok().map(int_of);
    if !matches!(version, Some(Ok(1))) {
        return Err(format!(
            "the request must declare protocol version 1 ({PROTOCOL})"
        ));
    }
    event(env, "start", shape.as_deref(), None)?;
    let answer = pyprofile::with_profile(&profile, || serve_request(&request))?;
    event(env, "end", shape.as_deref(), Some(&outcome_of(&answer)))?;
    let mut framed = tagged("answer");
    framed.set("protocol", PyValue::str(PROTOCOL));
    framed.set("answer", answer);
    PyValue::Dict(framed)
        .dumps_ascii()
        .map_err(|error| error.to_string())
}
