//! Private bridge of the in-process importers of `scan_node.py` (P1 only): one request on standard input, one answer on standard
//! output, selected by `SCAN_NODE_BRIDGE` in the binary `scan_node` (so no command line is added). The thin Python adapter marshals
//! the bound arguments of the original's public names into typed JSON; this side owns every decision.
//!
//! **Wire (`scan-node-bridge/1`).** A Python value is `{"t":"none"}`, `{"t":"bool","v":b}`, `{"t":"int","hex":"+1f"}` (sign and
//! lowercase hexadecimal: no decimal conversion, so no 4300-digit limit in transport), `{"t":"float","repr":"1.5"}` (`nan`, `inf`
//! travel as their repr), `{"t":"str","v":text}` (JSON escapes carry lone surrogates), `{"t":"bytes","hex":..}`, `{"t":"list"|"tuple",
//! "v":[..]}` and `{"t":"dict","v":[[key,value],..]}` (string keys, insertion order). The request is `{protocol, op, params}` with
//! `params` the bound arguments by the original's parameter names.
//!
//! **Shared identity (the result plan).** Python objects other than the singletons `None`/`bool` travel with an `"id"` the importer assigned in traversal order (pre-order, parameters in the order sent); a repeat visit
//! is `{"t":"ref","id":n}`. They are decoded as shared values, so a business function that hands an argument object back (the `scanned`
//! list of `cross_validate`, a NaN threshold) hands back the *same* object. The answer is a plan: such an object is
//! `{"t":"argref","id":n}` - "the caller's own object number n" - which the importer resolves to the actual caller object; every other
//! value is built fresh. Nothing about the caller's objects is copied or chosen on the Python side.
//!
//! **Answers.** `returned` with the value; `raised` with the exception (`cls`, `ctor` that rebuilds it, `cause`); `unsupported` with the
//! reason for a shape the port does not carry (reported before any answer is built; nothing is guessed). A transport problem is an
//! error of the process, never a Python outcome.

use agent_runtime_support::pyjson::loads_text;
use agent_runtime_support::pystr::PyStr as Text;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use num_bigint::{BigInt, Sign};
use role_contract::scan_node::{self as scan, Failure, GateArgs, Raised, Res};
use role_contract::scan_node_files::{self as files, Files};
use role_contract::scan_node_manifest::{self as manifest, Clock};
use std::collections::HashMap;
use std::ffi::OsString;
use std::io::Write;
use std::os::unix::ffi::OsStringExt;
use std::sync::Arc;

pub const BRIDGE_ENV: &str = "SCAN_NODE_BRIDGE";
pub const OWNER_ENV: &str = "SCAN_NODE_OWNER_PATH";
pub const OBSERVER_ENV: &str = "SCAN_NODE_ROUTE_OBSERVER";
pub const PROTOCOL: &str = "scan-node-bridge/1";
/// Exit status of a control failure of the program itself (the importer reports it; there is no Python route).
pub const EXIT_CONTROL_FAILURE: i32 = 3;

/// The process environment this program reads (explicit, so a check can hand one in).
#[derive(Clone, Debug, Default)]
pub struct Environment {
    pub owner: Option<Vec<u8>>,
    pub observer: Option<OsString>,
}

impl Environment {
    pub fn from_process() -> Self {
        let non_empty = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty());
        Environment {
            owner: non_empty(OWNER_ENV).map(OsString::into_vec),
            observer: non_empty(OBSERVER_ENV),
        }
    }
}

fn tagged(tag: &str) -> PyDict {
    let mut dict = PyDict::new();
    dict.set("t", PyValue::str(tag));
    dict
}

fn unhex(text: &str) -> Result<Vec<u8>, String> {
    if !text.len().is_multiple_of(2) {
        return Err("odd hexadecimal length".into());
    }
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).map_err(|e| e.to_string()))
        .collect()
}

/// The caller's objects that carry identity: id -> shared value, and shared value (by allocation) -> id.
#[derive(Default)]
struct Graph {
    by_id: HashMap<u64, PyValue>,
    ids: HashMap<usize, u64>,
}

impl Graph {
    fn register(&mut self, id: u64, inner: PyValue) -> Result<PyValue, String> {
        if self.by_id.contains_key(&id) {
            return Err(format!("the object id {id} is defined twice"));
        }
        let shared = Arc::new(inner);
        self.ids.insert(Arc::as_ptr(&shared) as usize, id);
        let value = PyValue::Shared(shared);
        self.by_id.insert(id, value.clone());
        Ok(value)
    }

    fn id_of(&self, value: &PyValue) -> Option<u64> {
        match value {
            PyValue::Shared(shared) => self.ids.get(&(Arc::as_ptr(shared) as usize)).copied(),
            _ => None,
        }
    }
}

/// A Python object as its wire form (a caller's own object is a reference into the plan).
fn encode(value: &PyValue, graph: &Graph) -> Result<PyValue, String> {
    if let Some(id) = graph.id_of(value) {
        let mut out = tagged("argref");
        out.set("id", PyValue::int(id));
        return Ok(PyValue::Dict(out));
    }
    let mut out;
    match value.peel() {
        PyValue::None => out = tagged("none"),
        PyValue::Bool(flag) => {
            out = tagged("bool");
            out.set("v", PyValue::Bool(*flag));
        }
        PyValue::Int(number) => {
            out = tagged("int");
            let sign = if number.sign() == Sign::Minus {
                "-"
            } else {
                "+"
            };
            out.set(
                "hex",
                PyValue::str(&format!("{sign}{:x}", number.magnitude())),
            );
        }
        PyValue::Float(number) => {
            out = tagged("float");
            out.set("repr", PyValue::str(&PyValue::Float(*number).repr()));
        }
        PyValue::Str(text) => {
            out = tagged("str");
            out.set("v", PyValue::Str(text.clone()));
        }
        PyValue::Bytes(bytes) => {
            out = tagged("bytes");
            let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
            out.set("hex", PyValue::str(&hex));
        }
        PyValue::List(items) | PyValue::Tuple(items) => {
            out = tagged(if matches!(value.peel(), PyValue::List(_)) {
                "list"
            } else {
                "tuple"
            });
            let encoded: Result<Vec<PyValue>, String> =
                items.iter().map(|item| encode(item, graph)).collect();
            out.set("v", PyValue::List(encoded?));
        }
        PyValue::Dict(dict) => {
            out = tagged("dict");
            let mut pairs = Vec::new();
            for (key, item) in dict.iter() {
                pairs.push(PyValue::List(vec![
                    encode(&PyValue::Str(key.clone()), graph)?,
                    encode(item, graph)?,
                ]));
            }
            out.set("v", PyValue::List(pairs));
        }
        other => {
            return Err(format!(
                "a value of type {} has no wire form",
                other.type_name()
            ))
        }
    }
    Ok(PyValue::Dict(out))
}

fn member<'a>(dict: &'a PyDict, name: &str) -> Result<&'a PyValue, String> {
    dict.get(name)
        .ok_or_else(|| format!("the wire object has no \"{name}\""))
}

fn string_of(value: &PyValue) -> Result<String, String> {
    match value.peel() {
        PyValue::Str(text) => text
            .to_utf8()
            .ok_or_else(|| "a wire word holds a lone surrogate".to_string()),
        _ => Err("a wire word is not a string".into()),
    }
}

/// The `"id"` a wire object carries (containers and NaN floats), if any.
fn object_id(dict: &PyDict) -> Result<Option<u64>, String> {
    match dict.get("id").map(PyValue::peel) {
        None => Ok(None),
        Some(PyValue::Int(number)) => u64::try_from(number.clone())
            .map(Some)
            .map_err(|_| "an object id is out of range".to_string()),
        Some(_) => Err("an object id is not an integer".into()),
    }
}

/// A wire form as the Python object it stands for. An object with an `"id"` becomes a shared value registered in `graph`; a `ref` is
/// the same shared value.
fn decode(value: &PyValue, graph: &mut Graph) -> Result<PyValue, String> {
    let PyValue::Dict(dict) = value.peel() else {
        return Err("a wire value is not an object".into());
    };
    let tag = string_of(member(dict, "t")?)?;
    let built = match tag.as_str() {
        "none" => PyValue::None,
        "bool" => match member(dict, "v")?.peel() {
            PyValue::Bool(flag) => PyValue::Bool(*flag),
            _ => return Err("a bool value is not a boolean".into()),
        },
        "int" => {
            let word = string_of(member(dict, "hex")?)?;
            let (negative, digits) = match word.strip_prefix('-') {
                Some(rest) => (true, rest),
                None => (false, word.strip_prefix('+').unwrap_or(&word)),
            };
            let magnitude = BigInt::parse_bytes(digits.as_bytes(), 16)
                .ok_or_else(|| "an int value is not hexadecimal".to_string())?;
            PyValue::Int(if negative { -magnitude } else { magnitude })
        }
        "float" => {
            let word = string_of(member(dict, "repr")?)?;
            word.parse::<f64>()
                .map(PyValue::Float)
                .map_err(|_| format!("a float value {word:?} is not a number"))?
        }
        "str" => match member(dict, "v")?.peel() {
            PyValue::Str(text) => PyValue::Str(text.clone()),
            _ => return Err("a str value is not a string".into()),
        },
        "bytes" => PyValue::Bytes(unhex(&string_of(member(dict, "hex")?)?)?),
        "ref" => {
            let Some(id) = object_id(dict)? else {
                return Err("a ref has no id".into());
            };
            return graph.by_id.get(&id).cloned().ok_or_else(|| {
                format!("a ref names the object {id}, which is not defined before it")
            });
        }
        "list" | "tuple" => {
            let PyValue::List(items) = member(dict, "v")?.peel() else {
                return Err("a sequence value is not a list".into());
            };
            let mut decoded = Vec::with_capacity(items.len());
            for item in items {
                decoded.push(decode(item, graph)?);
            }
            if tag == "list" {
                PyValue::List(decoded)
            } else {
                PyValue::Tuple(decoded)
            }
        }
        "dict" => {
            let PyValue::List(pairs) = member(dict, "v")?.peel() else {
                return Err("a dict value is not a list of pairs".into());
            };
            let mut out = PyDict::new();
            for pair in pairs {
                let PyValue::List(pair) = pair.peel() else {
                    return Err("a dict pair is not a list".into());
                };
                let [key, item] = pair.as_slice() else {
                    return Err("a dict pair has not two members".into());
                };
                match decode(key, graph)? {
                    PyValue::Str(key) => out.insert(key, decode(item, graph)?),
                    _ => return Err("a dict key is not a string".into()),
                }
            }
            PyValue::Dict(out)
        }
        other => return Err(format!("unknown wire type {other:?}")),
    };
    // Every numbered object is shared (the importer numbers each non-singleton object it passes, so that a failed attribute read can name the
    // caller's own object); `None` and `bool` are singletons and carry no id.
    let carries_identity = !matches!(&built, PyValue::None | PyValue::Bool(_));
    match (object_id(dict)?, carries_identity) {
        (Some(id), true) => graph.register(id, built),
        (Some(_), false) => Err("an id on a singleton (none or bool)".into()),
        (None, _) => Ok(built),
    }
}

fn encode_raised(raised: &Raised, graph: &Graph) -> Result<PyValue, String> {
    let mut out = PyDict::new();
    out.set("cls", PyValue::str(raised.class));
    // `str(exception)` as the owner shows it (diagnostic: the importer rebuilds the object from `ctor`)
    out.set("message", PyValue::Str(raised.message.clone()));
    let ctor: Result<Vec<PyValue>, String> =
        raised.ctor.iter().map(|item| encode(item, graph)).collect();
    out.set("ctor", PyValue::List(ctor?));
    let mut kwargs = PyDict::new();
    for (name, value) in &raised.ctor_kwargs {
        kwargs.set(name, encode(value, graph)?);
    }
    out.set("ctor_kwargs", PyValue::Dict(kwargs));
    out.set(
        "cause",
        match &raised.cause {
            Some(cause) => encode_raised(cause, graph)?,
            None => PyValue::None,
        },
    );
    Ok(PyValue::Dict(out))
}

fn answer(status: &str, key: &str, value: PyValue) -> PyValue {
    let mut out = PyDict::new();
    out.set("protocol", PyValue::str(PROTOCOL));
    out.set("status", PyValue::str(status));
    out.set(key, value);
    PyValue::Dict(out)
}

fn finish(graph: &Graph, result: Res<PyValue>) -> Result<PyValue, String> {
    match result {
        Ok(value) => Ok(answer("returned", "value", encode(&value, graph)?)),
        Err(Failure::Raised(raised)) => Ok(answer("raised", "exc", encode_raised(&raised, graph)?)),
        Err(Failure::Unsupported(why)) => Ok(answer("unsupported", "reason", PyValue::str(&why))),
    }
}

/// The bound arguments of a request, each by the original's parameter name, decoded in the order sent (a `ref` may name an object of an
/// earlier parameter).
struct Params(Vec<(String, PyValue)>);

impl Params {
    fn decode(params: &PyDict, graph: &mut Graph) -> Result<Params, String> {
        let mut values = Vec::new();
        for (name, wire) in params.iter() {
            let name = name
                .to_utf8()
                .ok_or("a parameter name holds a lone surrogate")?;
            values.push((name, decode(wire, graph)?));
        }
        Ok(Params(values))
    }

    fn get(&self, name: &str) -> Result<PyValue, String> {
        self.0
            .iter()
            .find(|(own, _)| own == name)
            .map(|(_, value)| value.clone())
            .ok_or_else(|| format!("the request has no parameter \"{name}\""))
    }
}

fn serve_request(
    request: &PyValue,
    files: &dyn Files,
    clock: &dyn Clock,
) -> Result<PyValue, String> {
    let PyValue::Dict(request) = request.peel() else {
        return Err("the request is not an object".into());
    };
    if string_of(member(request, "protocol")?)? != PROTOCOL {
        return Err(format!("the request does not speak {PROTOCOL}"));
    }
    let PyValue::Dict(params) = member(request, "params")?.peel() else {
        return Err("the request's params is not an object".into());
    };
    let mut graph = Graph::default();
    let p = Params::decode(params, &mut graph)?;
    let graph = &graph;
    let ok = |value: PyValue| -> Res<PyValue> { Ok(value) };
    match string_of(member(request, "op")?)?.as_str() {
        "valid_host" => finish(graph, ok(PyValue::Bool(scan::valid_host(&p.get("value")?)))),
        "_ver_tuple" => finish(graph, scan::ver_tuple(&p.get("s")?)),
        "_yaml_str" => finish(graph, ok(PyValue::Str(scan::yaml_str(&p.get("value")?)))),
        "parse_gpu_memory_line" => finish(graph, scan::parse_gpu_memory_line(&p.get("line")?)),
        "check_model_env" => finish(
            graph,
            ok(scan::check_model_env(
                &p.get("model_source")?,
                &p.get("egress")?,
                &p.get("nas_reachable")?,
                &p.get("hf_token_present")?,
            )),
        ),
        "derive_platform_preset" => {
            finish(graph, scan::derive_platform_preset(&p.get("gpu_model")?))
        }
        "assert_homogeneity" => finish(
            graph,
            scan::assert_homogeneity(&p.get("local")?, &p.get("peer")?),
        ),
        "default_env_interconnect_path" => finish(
            graph,
            scan::default_env_interconnect_path(&p.get("compose_path")?),
        ),
        "emit_gate" => finish(
            graph,
            ok(PyValue::int(scan::emit_gate(
                &p.get("emit_manifest")?,
                &p.get("topology")?,
            ))),
        ),
        "evaluate_gate" => {
            let args = GateArgs {
                declared: p.get("declared")?,
                ic_present: p.get("ic_present")?,
                peer_given: p.get("peer_given")?,
                peer_reachable: p.get("peer_reachable")?,
                bandwidth: p.get("bandwidth")?,
                bw_floor: p.get("bw_floor")?,
                per_port_gbps: p.get("per_port_gbps")?,
                per_port_floor: p.get("per_port_floor")?,
                branch: p.get("branch")?,
                branch_topo: p.get("branch_topo")?,
                mani_topo: p.get("mani_topo")?,
                mani_path: p.get("mani_path")?,
                peer_ip: p.get("peer_ip")?,
                egress_self: p.get("egress_self")?,
                egress_peer: p.get("egress_peer")?,
                model_env_ok: p.get("model_env_ok")?,
                model_env_reason: p.get("model_env_reason")?,
                manifest_read_error: p.get("manifest_read_error")?,
            };
            finish(graph, scan::evaluate_gate(&args))
        }
        "emission_blockers" => finish(graph, scan::emission_blockers(&p.get("result")?)),
        "build_sub_manifest" => finish(
            graph,
            manifest::build_sub_manifest(
                &p.get("result")?,
                &p.get("peer_hw")?,
                &p.get("main_manifest")?,
                &p.get("scanned_at")?,
                &p.get("egress_peer")?,
                clock,
            ),
        ),
        "emit_manifest_block" => finish(
            graph,
            manifest::emit_manifest_block(&p.get("result")?, clock),
        ),
        "cross_validate" => finish(
            graph,
            files::cross_validate(
                &p.get("ic")?,
                &p.get("compose_path")?,
                &p.get("env_interconnect_path")?,
                files,
            ),
        ),
        "read_manifest_field" => finish(
            graph,
            files::read_manifest_field(files, &p.get("path")?, &p.get("key")?),
        ),
        "read_manifest_topology" => {
            finish(graph, files::read_manifest_topology(files, &p.get("path")?))
        }
        "_read_manifest_line" => finish(
            graph,
            files::read_manifest_line_of(files, &p.get("path")?, &p.get("prefix")?),
        ),
        "_parse_compose_nccl" => finish(
            graph,
            files::parse_compose_nccl(files, &p.get("path")?).map(PyValue::Dict),
        ),
        "_parse_env_file_nccl" => finish(
            graph,
            files::parse_env_file_nccl(files, &p.get("path")?).map(PyValue::Dict),
        ),
        "_parse_nccl_lines" => finish(
            graph,
            files::parse_nccl_lines(files, &p.get("path")?, &p.get("sep")?).map(PyValue::Dict),
        ),
        other => Err(format!("unknown operation {other:?}")),
    }
}

/// The outcome word of an answer: `returned`, `unsupported` or `raised:<class>`. A `raised` answer travels with transport exit 0.
fn outcome_of(answer: &PyValue) -> String {
    let PyValue::Dict(answer) = answer.peel() else {
        return "malformed".into();
    };
    let status = answer
        .get("status")
        .and_then(|s| string_of(s).ok())
        .unwrap_or_default();
    if status != "raised" {
        return status;
    }
    let class = match answer.get("exc").map(PyValue::peel) {
        Some(PyValue::Dict(exc)) => exc
            .get("cls")
            .and_then(|c| string_of(c).ok())
            .unwrap_or_default(),
        _ => String::new(),
    };
    format!("raised:{class}")
}

/// One route observation: `event` (`start` before the business, `end` with the outcome), the route, the operation, the process's own
/// `pid`/`ppid`, the declared owner path and the outcome. Nothing in it is derived from the business output.
pub fn observe_event(
    env: &Environment,
    event: &str,
    route: &str,
    owner: Option<&[u8]>,
    shape: Option<&str>,
    outcome: Option<&str>,
) -> Result<(), String> {
    let Some(path) = &env.observer else {
        return Ok(());
    };
    let mut line = PyDict::new();
    line.set("event", PyValue::str(event));
    line.set("route", PyValue::str(route));
    line.set("pid", PyValue::int(std::process::id()));
    line.set("ppid", PyValue::int(std::os::unix::process::parent_id()));
    line.set(
        "owner_path",
        owner.map_or(PyValue::None, |bytes| {
            PyValue::Str(Text::from_surrogateescape(bytes))
        }),
    );
    line.set("shape", shape.map_or(PyValue::None, PyValue::str));
    line.set("outcome", outcome.map_or(PyValue::None, PyValue::str));
    let rendered = PyValue::Dict(line)
        .dumps_ascii()
        .map_err(|error| error.to_string())?;
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| writeln!(file, "{rendered}"))
        .map_err(|error| format!("the route observation cannot be written: {error}"))
}

/// Serves one request. The answer is JSON text (ASCII); a protocol error is an `Err` the binary reports on standard error.
/// The route is recorded (when `SCAN_NODE_ROUTE_OBSERVER` names a file) before the business starts and again with the outcome.
pub fn serve(
    input: &[u8],
    env: &Environment,
    files: &dyn Files,
    clock: &dyn Clock,
) -> Result<String, String> {
    let text = Text::from_surrogateescape(input);
    let request = loads_text(&text).map_err(|error| format!("the request is not JSON: {error}"))?;
    let shape = match request.peel() {
        PyValue::Dict(dict) => dict.get("op").and_then(|op| string_of(op).ok()),
        _ => None,
    };
    let owner = env.owner.as_deref();
    observe_event(env, "start", "NativeBridge", owner, shape.as_deref(), None)?;
    let answer = serve_request(&request, files, clock)?;
    observe_event(
        env,
        "end",
        "NativeBridge",
        owner,
        shape.as_deref(),
        Some(&outcome_of(&answer)),
    )?;
    answer.dumps_ascii().map_err(|error| error.to_string())
}
