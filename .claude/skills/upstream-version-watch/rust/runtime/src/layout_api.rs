//! The five functions of `check_spec_layout.py` as a JSON-lines service, wire-compatible with the resolver's service except that the arguments
//! arrive as typed DTOs (`ABI.md` of the R34 round).
//!
//! * `constants`: `{"check_spec_layout": {"DSPARK_ONLY_MARKERS": <DTO>, "PLAIN_MTP_COMPONENTS": <DTO>, "SPEC_KEY_RE": {"pattern", "flags"}}}`.
//! * `evaluate-jsonl`: one request per line `{id, operation, args_dto?, kwargs_dto?, globals?}` with `operation` `check_spec_layout.<function>`;
//!   one record per request, the resolver's (`id`, `kind`, `value` | exception members, `stdout`, `stderr`, `args_after`, `kwargs_after`).

use crate::api::{record, Capture};
use crate::dto::{dict_of, encode, encode_owned};
use crate::dto_decode::{decode_args, decode_kwargs};
use crate::host::{Sink, YamlProfile};
use crate::layout_files::{load_index_keys, read_method_from_config, read_num_nextn, LayoutWorld};
use crate::layout_main::{analyze, main, PROGRAM};
use crate::layout_rules::{
    DSPARK_ONLY_MARKERS, PLAIN_MTP_COMPONENTS, SPEC_KEY_FLAGS, SPEC_KEY_PATTERN,
};
use crate::thrown::{Done, Thrown};
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pycall::{required, Param, Signature};
use agent_runtime_support::pyjson::loads_text_objects;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use recipe_runtime::escalation::Raised;
use recipe_runtime::observe::same_value;

pub const MODULE: &str = "check_spec_layout";

const CONFIG_YAML: &[Param] = &[required("config_yaml")];
const MODEL_DIR: &[Param] = &[required("model_dir")];
const KEYS: &[Param] = &[required("keys")];
const NOTHING: &[Param] = &[];

/// The five functions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Function {
    ReadMethodFromConfig,
    LoadIndexKeys,
    Analyze,
    ReadNumNextn,
    Main,
}

impl Function {
    pub fn named(name: &str) -> Option<Function> {
        Some(match name {
            "read_method_from_config" => Function::ReadMethodFromConfig,
            "load_index_keys" => Function::LoadIndexKeys,
            "analyze" => Function::Analyze,
            "read_num_nextn" => Function::ReadNumNextn,
            "main" => Function::Main,
            _ => return None,
        })
    }

    fn signature(self) -> Signature {
        let (name, params) = match self {
            Function::ReadMethodFromConfig => ("read_method_from_config", CONFIG_YAML),
            Function::LoadIndexKeys => ("load_index_keys", MODEL_DIR),
            Function::Analyze => ("analyze", KEYS),
            Function::ReadNumNextn => ("read_num_nextn", MODEL_DIR),
            Function::Main => ("main", NOTHING),
        };
        Signature {
            name,
            params,
            rest: false,
        }
    }
}

fn tuple_of(names: &[&str]) -> PyValue {
    PyValue::Tuple(names.iter().map(|name| PyValue::str(name)).collect())
}

/// The three source constants as `constants` prints them (the two tuples as DTOs, the pattern as the original's collector records it).
pub fn source_constants() -> Result<PyValue, String> {
    Ok(dict_of(vec![
        (
            "DSPARK_ONLY_MARKERS",
            encode(&tuple_of(&DSPARK_ONLY_MARKERS))?,
        ),
        (
            "PLAIN_MTP_COMPONENTS",
            encode(&tuple_of(&PLAIN_MTP_COMPONENTS))?,
        ),
        (
            "SPEC_KEY_RE",
            dict_of(vec![
                ("pattern", PyValue::str(SPEC_KEY_PATTERN)),
                ("flags", PyValue::int(SPEC_KEY_FLAGS)),
            ]),
        ),
    ]))
}

/// The constants, keyed by owner.
pub fn constants() -> Result<PyValue, String> {
    Ok(dict_of(vec![(MODULE, source_constants()?)]))
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

    /// Takes the members a request's `globals` names; anything else is refused with the reason. The three source constants are accepted only
    /// when they are the source's own.
    pub fn replace_from(&mut self, given: &PyValue) -> Result<(), String> {
        let PyValue::Dict(members) = given else {
            return Err("globals must be an object".to_string());
        };
        let canonical = source_constants()?;
        let PyValue::Dict(canonical) = &canonical else {
            unreachable!("a dict")
        };
        for (key, value) in members.iter() {
            if key.equals("argv") {
                continue;
            }
            match canonical.iter().find(|(name, _)| name == key) {
                Some((_, want)) if same_value(want, value) => {}
                Some(_) => {
                    return Err(format!(
                        "{} is the source's constant: no other value is modelled",
                        key.repr()
                    ))
                }
                None => {
                    return Err(format!(
                        "globals holds argv and the three source constants only: {}",
                        key.repr()
                    ))
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

/// Resolves `check_spec_layout.function`.
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

/// Binds the arguments as Python does and runs the function; the value is the Python value (the DTO is made by the caller).
pub fn call(
    function: Function,
    world: &mut LayoutWorld<'_>,
    args: &[PyValue],
    kwargs: &PyDict,
) -> Done<PyValue> {
    let signature = function.signature();
    let bound = signature
        .bind(args, kwargs)
        .map_err(|failure: Failure| Thrown::plain(Raised::from(failure)))?;
    match function {
        Function::ReadMethodFromConfig => read_method_from_config(world, &bound[0]),
        Function::LoadIndexKeys => load_index_keys(world, &bound[0]),
        Function::Analyze => analyze(&bound[0]),
        Function::ReadNumNextn => read_num_nextn(world, &bound[0]),
        Function::Main => main(world),
    }
}

/// A request that passed the shape checks.
pub struct Request {
    pub id: PyValue,
    pub operation: String,
    pub args: Vec<PyValue>,
    pub kwargs: PyDict,
    pub globals: Option<PyValue>,
}

/// The answer of a call (`crate::api::record`, member for member: `id`, `kind`, the value or the exception, the streams and the arguments after the call),
/// except that the call's RETURNED value, which the call owns and nothing reads afterwards, is turned into its DTO by [`encode_owned`]: fresh text, lists
/// and tuples are moved into the DTO instead of copied. The record, its member order, the refusal-as-`TypeError` and the exception chain are the same as
/// `record`'s; the arguments after the call are still encoded by the borrowed encoder.
fn record_returned(
    id: PyValue,
    outcome: Done<PyValue>,
    stdout: &PyStr,
    stderr: &PyStr,
    args: Vec<PyValue>,
    kwargs: PyDict,
) -> PyValue {
    let mut row = PyDict::new();
    row.set("id", id);
    let shown = outcome.and_then(|value| {
        encode_owned(value).map_err(|refusal| Thrown::builtin("TypeError", &refusal))
    });
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

fn invalid(id: PyValue, message: &str) -> PyValue {
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

/// Reads one request line. `Err` is the finished record to answer with.
pub fn parse_request(line: &PyStr) -> Result<Request, PyValue> {
    let request = match loads_text_objects(line) {
        Ok(request) => request,
        Err(error) => return Err(invalid(PyValue::None, &format!("not JSON: {error}"))),
    };
    let PyValue::Dict(members) = request else {
        return Err(invalid(PyValue::None, "the request is not an object"));
    };
    let Some(id) = members.get("id").cloned() else {
        return Err(invalid(PyValue::None, "the request has no id"));
    };
    for (key, _) in members.iter() {
        let known = ["id", "operation", "args_dto", "kwargs_dto", "globals"]
            .iter()
            .any(|name| key.equals(name));
        if !known {
            return Err(invalid(
                id,
                &format!(
                    "the request has a member this service does not take: {}",
                    key.repr()
                ),
            ));
        }
    }
    let Some(PyValue::Str(operation)) = members.get("operation") else {
        return Err(invalid(id, "operation must be a string 'module.function'"));
    };
    let args = match members.get("args_dto") {
        None => Vec::new(),
        Some(dto) => decode_args(dto)
            .map_err(|problem| invalid(id.clone(), &format!("args_dto: {problem}")))?,
    };
    let kwargs = match members.get("kwargs_dto") {
        None => PyDict::new(),
        Some(dto) => decode_kwargs(dto)
            .map_err(|problem| invalid(id.clone(), &format!("kwargs_dto: {problem}")))?,
    };
    Ok(Request {
        id,
        operation: operation.to_utf8().unwrap_or_default(),
        args,
        kwargs,
        globals: members.get("globals").cloned(),
    })
}

/// What a service run is configured with: the terminal width.
pub struct Defaults {
    pub columns: Option<String>,
}

/// Answers one request line on `host` (the files are read and every real write goes through `host`). The streams of the record are empty:
/// what the call wrote is whatever `host` did with it.
pub fn answer_on(line: &PyStr, defaults: &Defaults, host: &mut dyn Sink) -> PyValue {
    let request = match parse_request(line) {
        Ok(request) => request,
        Err(row) => return row,
    };
    let mut globals = Globals::defaults();
    if let Some(given) = &request.globals {
        if let Err(problem) = globals.replace_from(given) {
            return invalid(request.id.clone(), &problem);
        }
    }
    let outcome = match resolve_operation(&request.operation) {
        Err(raised) => Err(Thrown::plain(raised)),
        Ok(function) => call(
            function,
            &mut LayoutWorld::new(host, &globals.argv, defaults.columns.as_deref()),
            &request.args,
            &request.kwargs,
        ),
    };
    record_returned(
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
    // the resolver's capture serves here: it only collects the two streams and never asks the (absent) import
    let mut capture = Capture::new(YamlProfile::Available, false);
    let mut row = answer_on(line, defaults, &mut capture);
    if let PyValue::Dict(map) = &mut row {
        if map.contains_key("stdout") {
            map.set("stdout", PyValue::Str(capture.stdout.clone()));
            map.set("stderr", PyValue::Str(capture.stderr.clone()));
        }
    }
    row
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
        out.extend_from_slice(crate::api::render(&answer(&line, defaults))?.as_bytes());
        out.push(b'\n');
    }
    Ok(out)
}
