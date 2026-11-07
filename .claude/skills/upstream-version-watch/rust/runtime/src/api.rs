//! The eleven functions as a JSON-lines service, wire-compatible with the other owners' services (`ABI.md` of the R33 round).
//!
//! * `constants`: `{"resolve_build_track": {"SOURCE_BUILD_MIN": <DTO>, "_MANIFEST_SM_KEYS": <DTO>}}`.
//! * `evaluate-jsonl`: one request per line `{id, operation, args, kwargs, globals?}` with `operation` `resolve_build_track.<function>`; one record
//!   per request: `{id, kind: "value", value, stdout, stderr, args_after, kwargs_after}` or `{id, kind: "exception", type, message,
//!   exception_chain, stdout, stderr, args_after, kwargs_after}`. The value is a typed DTO. `globals` is a member of its own: `argv`, and the
//!   two source constants only when they equal the source's.

use crate::business::{
    manifest_sm_keys, map_torch_cuda_arch, norm_release, propose_track, require, resolve,
    sm_from_manifest, source_build_min, strip_inline_comment,
};
use crate::dto::{dict_of, encode};
use crate::host::{Host, Sink, YamlProfile};
use crate::main_cmd::{main, self_test, PROGRAM};
use crate::manifest::{load_manifest, load_yaml_flat, World};
use crate::thrown::{Done, Thrown};
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pycall::{required, Param, Signature};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use recipe_runtime::escalation::Raised;
use recipe_runtime::memory_api::{parse_request, Request};
use recipe_runtime::observe::same_value;

pub const MODULE: &str = "resolve_build_track";

const V: &[Param] = &[required("v")];
const TORCH_PIN: &[Param] = &[required("torch_pin")];
const SM: &[Param] = &[required("sm")];
const PATH: &[Param] = &[required("path")];
const VAL: &[Param] = &[required("val")];
const MANIFEST: &[Param] = &[required("manifest")];
const RESOLVE: &[Param] = &[required("torch_pin"), required("sm_arch")];
const REQUIRE: &[Param] = &[required("condition"), required("message")];
const NOTHING: &[Param] = &[];

/// The eleven functions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Function {
    NormRelease,
    ProposeTrack,
    MapTorchCudaArch,
    LoadManifest,
    LoadYamlFlat,
    StripInlineComment,
    SmFromManifest,
    Resolve,
    Require,
    SelfTest,
    Main,
}

impl Function {
    pub fn named(name: &str) -> Option<Function> {
        Some(match name {
            "norm_release" => Function::NormRelease,
            "propose_track" => Function::ProposeTrack,
            "map_torch_cuda_arch" => Function::MapTorchCudaArch,
            "load_manifest" => Function::LoadManifest,
            "_load_yaml_flat" => Function::LoadYamlFlat,
            "_strip_inline_comment" => Function::StripInlineComment,
            "sm_from_manifest" => Function::SmFromManifest,
            "resolve" => Function::Resolve,
            "_require" => Function::Require,
            "_self_test" => Function::SelfTest,
            "main" => Function::Main,
            _ => return None,
        })
    }

    fn signature(self) -> Signature {
        let (name, params) = match self {
            Function::NormRelease => ("norm_release", V),
            Function::ProposeTrack => ("propose_track", TORCH_PIN),
            Function::MapTorchCudaArch => ("map_torch_cuda_arch", SM),
            Function::LoadManifest => ("load_manifest", PATH),
            Function::LoadYamlFlat => ("_load_yaml_flat", PATH),
            Function::StripInlineComment => ("_strip_inline_comment", VAL),
            Function::SmFromManifest => ("sm_from_manifest", MANIFEST),
            Function::Resolve => ("resolve", RESOLVE),
            Function::Require => ("_require", REQUIRE),
            Function::SelfTest => ("_self_test", NOTHING),
            Function::Main => ("main", NOTHING),
        };
        Signature {
            name,
            params,
            rest: false,
        }
    }
}

/// The constants, keyed by owner.
pub fn constants() -> Result<PyValue, String> {
    Ok(dict_of(vec![(
        MODULE,
        dict_of(vec![
            ("SOURCE_BUILD_MIN", encode(&source_build_min())?),
            ("_MANIFEST_SM_KEYS", encode(&manifest_sm_keys())?),
        ]),
    )]))
}

/// The module values a call can read: `sys.argv`.
#[derive(Clone)]
pub struct Globals {
    pub argv: Vec<PyStr>,
}

impl Globals {
    pub fn defaults() -> Globals {
        Globals {
            argv: vec![PyStr::new(PROGRAM)],
        }
    }

    pub fn to_value(&self) -> PyValue {
        PyValue::List(self.argv.iter().cloned().map(PyValue::Str).collect())
    }

    /// Takes the members a request's `globals` names; anything else is refused with the reason. The two source constants are accepted
    /// only when they are the source's own (any other value would need the dynamic semantics of arbitrary Python objects).
    pub fn replace_from(&mut self, given: &PyValue) -> Result<(), String> {
        let PyValue::Dict(members) = given else {
            return Err("globals must be an object".to_string());
        };
        for (key, _) in members.iter() {
            if !key.equals("argv")
                && !key.equals("SOURCE_BUILD_MIN")
                && !key.equals("_MANIFEST_SM_KEYS")
            {
                return Err(format!(
                    "globals holds argv and the two source constants only: {}",
                    key.repr()
                ));
            }
        }
        for (name, canonical) in [
            ("SOURCE_BUILD_MIN", source_build_min()),
            ("_MANIFEST_SM_KEYS", manifest_sm_keys()),
        ] {
            if let Some(value) = members.get(name) {
                let want = encode(&canonical)?;
                if !same_value(&want, value) {
                    return Err(format!(
                        "{name} is the source's constant (a DTO): no other value is modelled"
                    ));
                }
            }
        }
        if let Some(argv) = members.get("argv") {
            let PyValue::List(items) = argv else {
                return Err("argv must be an array of strings".to_string());
            };
            let mut strings = Vec::new();
            for item in items {
                let PyValue::Str(text) = item else {
                    return Err("argv must be an array of strings".to_string());
                };
                strings.push(text.clone());
            }
            self.argv = strings;
        }
        Ok(())
    }
}

/// Resolves `resolve_build_track.function`.
pub fn resolve_operation(operation: &str) -> Result<Function, Raised> {
    let (module, name) = operation.split_once('.').unwrap_or((operation, ""));
    if module != MODULE {
        return Err(Raised::plain(
            "ModuleNotFoundError",
            &format!("No module named '{module}'"),
        ));
    }
    Function::named(name).ok_or_else(|| {
        Raised::plain(
            "AttributeError",
            &format!("module '{module}' has no attribute '{name}'"),
        )
    })
}

/// What an API call printed (and, in a check, the ordered record of its file steps).
pub struct Capture {
    pub stdout: PyStr,
    pub stderr: PyStr,
    pub tracing: bool,
    pub trace: Vec<PyValue>,
    profile: YamlProfile,
}

impl Capture {
    pub fn new(profile: YamlProfile, tracing: bool) -> Capture {
        Capture {
            stdout: PyStr::default(),
            stderr: PyStr::default(),
            tracing,
            trace: Vec::new(),
            profile,
        }
    }
}

fn event(name: &str, fields: Vec<(&str, PyValue)>) -> PyValue {
    let mut row = PyDict::new();
    row.set("event", PyValue::str(name));
    for (key, value) in fields {
        row.set(key, value);
    }
    PyValue::Dict(row)
}

impl Sink for Capture {
    fn out(&mut self, text: &PyStr) -> Done<()> {
        self.stdout.push_py(text);
        Ok(())
    }

    fn err(&mut self, text: &PyStr) -> Done<()> {
        self.stderr.push_py(text);
        Ok(())
    }

    fn parser_help(&mut self, help: &PyStr) {
        self.stdout.push_py(help);
    }

    fn parser_error(&mut self, usage: &PyStr, message: &PyStr) {
        self.stderr.push_py(usage);
        self.stderr.push_py(message);
    }

    fn tracing(&self) -> bool {
        self.tracing
    }

    fn trace(&mut self, name: &str, fields: Vec<(&str, PyValue)>) {
        self.trace.push(event(name, fields));
    }
}

impl Host for Capture {
    fn import_yaml(&mut self) -> Done<()> {
        self.profile.import_yaml()
    }
}

/// Binds the arguments as Python does and runs the function; the value is the Python value (the DTO is made by the caller).
pub fn call(
    function: Function,
    world: &mut World<'_>,
    args: &[PyValue],
    kwargs: &PyDict,
) -> Done<PyValue> {
    let signature = function.signature();
    let bound = signature
        .bind(args, kwargs)
        .map_err(|failure: Failure| Thrown::plain(Raised::from(failure)))?;
    match function {
        Function::NormRelease => norm_release(&bound[0]),
        Function::ProposeTrack => propose_track(&bound[0]),
        Function::MapTorchCudaArch => map_torch_cuda_arch(&bound[0]),
        Function::LoadManifest => load_manifest(world, &bound[0]),
        Function::LoadYamlFlat => load_yaml_flat(world, &bound[0]),
        Function::StripInlineComment => strip_inline_comment(&bound[0]),
        Function::SmFromManifest => sm_from_manifest(&bound[0]),
        Function::Resolve => resolve(&bound[0], &bound[1]),
        Function::Require => require(&bound[0], &bound[1]),
        Function::SelfTest => self_test(world),
        Function::Main => main(world),
    }
}

/// The answer of a call: the outcome with the streams and the arguments after it.
pub fn record(
    id: PyValue,
    outcome: Done<PyValue>,
    stdout: &PyStr,
    stderr: &PyStr,
    args: Vec<PyValue>,
    kwargs: PyDict,
) -> PyValue {
    let mut row = PyDict::new();
    row.set("id", id);
    let shown = outcome
        .and_then(|value| encode(&value).map_err(|refusal| Thrown::builtin("TypeError", &refusal)));
    match shown {
        Ok(value) => {
            row.set("kind", PyValue::str("value"));
            row.set("value", value);
        }
        Err(thrown) => {
            row.set("kind", PyValue::str("exception"));
            row.set("type", PyValue::str(&thrown.raised.class));
            row.set("message", PyValue::Str(thrown.raised.message.clone()));
            row.set("exception_chain", thrown.chain());
        }
    }
    row.set("stdout", PyValue::Str(stdout.clone()));
    row.set("stderr", PyValue::Str(stderr.clone()));
    row.set(
        "args_after",
        encode(&PyValue::List(args)).unwrap_or(PyValue::None),
    );
    row.set(
        "kwargs_after",
        encode(&PyValue::Dict(kwargs)).unwrap_or(PyValue::None),
    );
    PyValue::Dict(row)
}

fn request_record(id: PyValue, message: &str) -> PyValue {
    record(
        id,
        Err(Thrown::plain(Raised::from(Failure::new(
            "RequestError",
            message,
        )))),
        &PyStr::default(),
        &PyStr::default(),
        Vec::new(),
        PyDict::new(),
    )
}

fn operation_of(request: &Request) -> String {
    match request.members.get("operation") {
        Some(PyValue::Str(text)) => text.to_utf8().unwrap_or_default(),
        _ => String::new(),
    }
}

/// What a service run is configured with: the terminal width and the YAML installation.
pub struct Defaults {
    pub columns: Option<String>,
    pub profile: YamlProfile,
}

/// Answers one request line on `host` (the files are read and every real write goes through `host`). The streams of the record are empty:
/// what the call wrote is whatever `host` did with it.
pub fn answer_on(line: &PyStr, defaults: &Defaults, host: &mut dyn Host) -> PyValue {
    let request = match parse_request(line, &["globals"]) {
        Ok(request) => request,
        Err(row) => return row,
    };
    let mut globals = Globals::defaults();
    if let Some(given) = request.members.get("globals") {
        if let Err(problem) = globals.replace_from(given) {
            return request_record(request.id.clone(), &problem);
        }
    }
    let outcome = match resolve_operation(&operation_of(&request)) {
        Err(raised) => Err(Thrown::plain(raised)),
        Ok(function) => call(
            function,
            &mut World {
                host,
                argv: &globals.argv,
                columns: defaults.columns.as_deref(),
            },
            &request.args,
            &request.kwargs,
        ),
    };
    record(
        request.id,
        outcome,
        &PyStr::default(),
        &PyStr::default(),
        request.args,
        request.kwargs,
    )
}

/// Answers one request line; what the call printed is in the record.
pub fn answer(line: &PyStr, defaults: &Defaults) -> PyValue {
    let mut capture = Capture::new(defaults.profile, false);
    let mut row = answer_on(line, defaults, &mut capture);
    if let PyValue::Dict(map) = &mut row {
        if map.contains_key("stdout") {
            map.set("stdout", PyValue::Str(capture.stdout.clone()));
            map.set("stderr", PyValue::Str(capture.stderr.clone()));
        }
    }
    row
}

/// One output line of a record.
pub fn render(row: &PyValue) -> Result<String, Failure> {
    recipe_runtime::api::render(row)
}

/// `evaluate-jsonl`: every non-blank line of `input` answered in order, one JSON line each.
pub fn evaluate_jsonl(input: &[u8], defaults: &Defaults) -> Result<Vec<u8>, Failure> {
    let text = PyStr::from_surrogateescape(input);
    let newline = u32::from('\n');
    let mut out = Vec::new();
    for piece in text.code_points().split(|&point| point == newline) {
        let line = PyStr::from_code_points(piece.to_vec());
        if line.strip().is_empty() {
            continue;
        }
        out.extend_from_slice(render(&answer(&line, defaults))?.as_bytes());
        out.push(b'\n');
    }
    Ok(out)
}
