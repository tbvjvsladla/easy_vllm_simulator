//! The ten public functions of `node_role_contract.py` as a JSON-lines service, schema `node_role_contract-api/1`. `docs/ABI.md` states the contract
//! as implemented.
//!
//! **Request** (one JSON object per line; all six members are required, any other member is refused): `schema`, `id` (a string), `operation`
//! (`node_role_contract.<function>`), `args_dto` (list DTO), `kwargs_dto` (dict DTO), `globals` (the module's 14 constants as an object of DTOs; it must
//! equal the owner's own, which are never replaced). Arguments are bound as CPython binds them (`Signature`), so a missing, extra or duplicated argument
//! is the interpreter's `TypeError`.
//!
//! **Record**: `schema`, `id`, `kind`, `phase`, `owner_call_attempted`, `stdout`, `stderr` (the business streams, always empty: no function prints), plus
//! per `kind`: `value` (`args_after`, `kwargs_after`, `globals_after`, `value`, `input_ref`), `exception` (`args_after`, `kwargs_after`, `globals_after`,
//! `exception`), `out_of_scope` (`boundary`, `detail`, with the three `*_after`) and `request_error` (`message`).
//!
//! **`input_ref`**: `null` for a value that is a new object; the path of the caller argument the value IS for `assert_topology` (it returns its own
//! argument), `[{"kind":"args"},{"kind":"list_item","index":0}]` or `[{"kind":"kwargs"},{"kind":"dict_value","index":N}]`. No other function returns a
//! caller object as its whole value.

use crate::dto::encode;
use crate::dto_decode::{decode_args, decode_kwargs};
use crate::role_errors::{contract_violation, load_failure, path_join_error, Raised};
use crate::wheel_api::exception_row;
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pycall::{required, Param, Signature};
use agent_runtime_support::pyjson::loads_text_objects;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use role_contract::load::load_yaml;
use role_contract::{
    assert_topology, delivery_plane, evaluate_manifest, globals, identity_authority,
    manifest_path_for, resolve_rank, resolve_sub_mode, sub_self_publishes, tool_plane,
    ContractViolation, Verdict,
};
use role_contract::{FrameRoot, SourceFrame};

pub const SCHEMA: &str = "node_role_contract-api/1";
pub const MODULE: &str = "node_role_contract";
const REQUEST_MEMBERS: [&str; 6] = [
    "schema",
    "id",
    "operation",
    "args_dto",
    "kwargs_dto",
    "globals",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    LoadYaml,
    ManifestPathFor,
    AssertTopology,
    ResolveSubMode,
    ResolveRank,
    IdentityAuthority,
    DeliveryPlane,
    ToolPlane,
    SubSelfPublishes,
    EvaluateManifest,
}

fn none_default() -> PyValue {
    PyValue::None
}
fn sub_default() -> PyValue {
    PyValue::str("sub")
}

impl Operation {
    fn named(name: &str) -> Option<Operation> {
        Some(match name {
            "load_yaml" => Operation::LoadYaml,
            "manifest_path_for" => Operation::ManifestPathFor,
            "assert_topology" => Operation::AssertTopology,
            "resolve_sub_mode" => Operation::ResolveSubMode,
            "resolve_rank" => Operation::ResolveRank,
            "identity_authority" => Operation::IdentityAuthority,
            "delivery_plane" => Operation::DeliveryPlane,
            "tool_plane" => Operation::ToolPlane,
            "sub_self_publishes" => Operation::SubSelfPublishes,
            "evaluate_manifest" => Operation::EvaluateManifest,
            _ => return None,
        })
    }

    fn signature(self) -> Signature {
        const PATH: &[Param] = &[required("path")];
        const REPO: &[Param] = &[required("repo"), required("topology")];
        const TOPOLOGY: &[Param] = &[required("topology")];
        const DECLARED: &[Param] = &[
            required("topology"),
            Param {
                name: "declared",
                default: Some(none_default),
            },
        ];
        const RANK: &[Param] = &[required("topology"), required("nodes"), required("role")];
        const MODE: &[Param] = &[required("topology"), required("sub_mode")];
        const MODE_OPTIONAL: &[Param] = &[
            required("topology"),
            Param {
                name: "sub_mode",
                default: Some(none_default),
            },
        ];
        const EVALUATE: &[Param] = &[
            required("doc"),
            Param {
                name: "topology",
                default: Some(none_default),
            },
            Param {
                name: "role",
                default: Some(sub_default),
            },
        ];
        let (name, params) = match self {
            Operation::LoadYaml => ("load_yaml", PATH),
            Operation::ManifestPathFor => ("manifest_path_for", REPO),
            Operation::AssertTopology => ("assert_topology", TOPOLOGY),
            Operation::ResolveSubMode => ("resolve_sub_mode", DECLARED),
            Operation::ResolveRank => ("resolve_rank", RANK),
            Operation::IdentityAuthority => ("identity_authority", TOPOLOGY),
            Operation::DeliveryPlane => ("delivery_plane", MODE),
            Operation::ToolPlane => ("tool_plane", MODE),
            Operation::SubSelfPublishes => ("sub_self_publishes", MODE_OPTIONAL),
            Operation::EvaluateManifest => ("evaluate_manifest", EVALUATE),
        };
        Signature {
            name,
            params,
            rest: false,
        }
    }
}

fn string(text: &str) -> PyValue {
    PyValue::str(text)
}

/// The 14 constants as an object of DTOs (the saved originals' `globals_after`).
pub fn canonical_globals() -> PyValue {
    let mut dict = PyDict::new();
    for (name, value) in globals() {
        dict.set(name, encode(&value).unwrap_or(PyValue::None));
    }
    PyValue::Dict(dict)
}

fn request_error(id: PyValue, message: &str) -> PyValue {
    let mut row = PyDict::new();
    row.set("schema", string(SCHEMA));
    row.set("id", id);
    row.set("kind", string("request_error"));
    row.set("phase", string("request"));
    row.set("owner_call_attempted", PyValue::Bool(false));
    row.set("stdout", string(""));
    row.set("stderr", string(""));
    row.set("message", string(message));
    PyValue::Dict(row)
}

struct Request {
    id: PyValue,
    operation: Operation,
    args: Vec<PyValue>,
    kwargs: PyDict,
}

fn parse_line(line: &PyStr) -> Result<Request, PyValue> {
    let parsed = loads_text_objects(line)
        .map_err(|error| request_error(PyValue::None, &format!("not JSON: {error}")))?;
    let PyValue::Dict(members) = parsed else {
        return Err(request_error(PyValue::None, "the request is not an object"));
    };
    let id = members.get("id").cloned().unwrap_or(PyValue::None);
    if let Some((key, _)) = members
        .iter()
        .find(|(key, _)| !REQUEST_MEMBERS.iter().any(|known| key.equals(known)))
    {
        return Err(request_error(
            id,
            &format!(
                "the request has a member this service does not take: {}",
                key.repr()
            ),
        ));
    }
    if !matches!(id, PyValue::Str(_)) {
        return Err(request_error(PyValue::None, "id must be a string"));
    }
    for member in REQUEST_MEMBERS {
        if members.get(member).is_none() {
            return Err(request_error(
                id,
                &format!("the request lacks the member '{member}'"),
            ));
        }
    }
    if members.get("schema") != Some(&string(SCHEMA)) {
        return Err(request_error(id, &format!("schema must be '{SCHEMA}'")));
    }
    let Some(PyValue::Str(operation)) = members.get("operation") else {
        return Err(request_error(
            id,
            "operation must be a string 'module.function'",
        ));
    };
    let operation = operation.to_utf8().unwrap_or_default();
    let (module, name) = operation
        .split_once('.')
        .unwrap_or((operation.as_str(), ""));
    let known = if module == MODULE {
        Operation::named(name)
    } else {
        None
    };
    let Some(operation) = known else {
        return Err(request_error(
            id,
            &format!("unknown operation {}", PyStr::new(&operation).repr()),
        ));
    };
    if members.get("globals") != Some(&canonical_globals()) {
        return Err(request_error(
            id,
            "globals must equal the owner's 14 constants (output DTO form): the constants are never replaced",
        ));
    }
    let args = decode_args(members.get("args_dto").unwrap_or(&PyValue::None))
        .map_err(|problem| request_error(id.clone(), &format!("args_dto: {problem}")))?;
    let kwargs = decode_kwargs(members.get("kwargs_dto").unwrap_or(&PyValue::None))
        .map_err(|problem| request_error(id.clone(), &format!("kwargs_dto: {problem}")))?;
    Ok(Request {
        id,
        operation,
        args,
        kwargs,
    })
}

enum Ended {
    Value {
        value: PyValue,
        input_ref: PyValue,
    },
    Exception {
        error: Box<crate::http_direct::PyException>,
        from_none: bool,
        frames: Option<Vec<SourceFrame>>,
    },
    OutOfScope {
        boundary: String,
        detail: String,
    },
}

impl From<Raised> for Ended {
    fn from(raised: Raised) -> Ended {
        match raised {
            Raised::Exception {
                error,
                from_none,
                frames,
            } => Ended::Exception {
                error,
                from_none,
                frames,
            },
            Raised::OutOfScope { boundary, detail } => Ended::OutOfScope {
                boundary: boundary.to_string(),
                detail,
            },
        }
    }
}

fn violation(violation: &ContractViolation) -> Ended {
    Ended::Exception {
        error: Box::new(contract_violation(violation)),
        from_none: false,
        frames: Some(violation.frames()),
    }
}

fn verdict(result: Result<Verdict, ContractViolation>) -> Ended {
    match result {
        Ok(verdict) => Ended::Value {
            value: verdict.to_value(),
            input_ref: PyValue::None,
        },
        Err(failure) => violation(&failure),
    }
}

fn path_item(kind: &str, index: Option<usize>) -> PyValue {
    let mut item = PyDict::new();
    item.set("kind", string(kind));
    if let Some(index) = index {
        item.set("index", PyValue::int(index));
    }
    PyValue::Dict(item)
}

/// The observation-graph path of the argument bound to the first parameter, named `first`.
fn first_argument_path(request: &Request, first: &str) -> PyValue {
    if !request.args.is_empty() {
        return PyValue::List(vec![
            path_item("args", None),
            path_item("list_item", Some(0)),
        ]);
    }
    let index = request
        .kwargs
        .iter()
        .position(|(key, _)| key.equals(first))
        .unwrap_or(0);
    PyValue::List(vec![
        path_item("kwargs", None),
        path_item("dict_value", Some(index)),
    ])
}

fn run(request: &Request, b: &[PyValue]) -> Ended {
    match request.operation {
        Operation::LoadYaml => match load_yaml(&b[0]) {
            Ok(value) => Ended::Value {
                value,
                input_ref: PyValue::None,
            },
            Err(failure) => load_failure(failure).into(),
        },
        Operation::ManifestPathFor => match manifest_path_for(&b[0], &b[1]) {
            Ok(path) => Ended::Value {
                value: PyValue::Str(path),
                input_ref: PyValue::None,
            },
            Err(error) => path_join_error(&error).into(),
        },
        Operation::AssertTopology => match assert_topology(&b[0]) {
            Ok(()) => Ended::Value {
                value: b[0].clone(),
                input_ref: first_argument_path(request, "topology"),
            },
            Err(failure) => violation(&failure),
        },
        Operation::ResolveSubMode => verdict(resolve_sub_mode(&b[0], &b[1])),
        Operation::ResolveRank => verdict(resolve_rank(&b[0], &b[1], &b[2])),
        Operation::IdentityAuthority => verdict(identity_authority(&b[0])),
        Operation::DeliveryPlane => verdict(delivery_plane(&b[0], &b[1])),
        Operation::ToolPlane => verdict(tool_plane(&b[0], &b[1])),
        Operation::SubSelfPublishes => verdict(sub_self_publishes(&b[0], &b[1])),
        Operation::EvaluateManifest => Ended::Value {
            value: evaluate_manifest(&b[0], &b[1], &b[2]).to_value(),
            input_ref: PyValue::None,
        },
    }
}

fn binding_error(failure: Failure) -> crate::http_direct::PyException {
    crate::py_errors::builtin(
        failure.class,
        &failure.message.to_utf8().unwrap_or_default(),
    )
}

fn root_name(root: FrameRoot) -> &'static str {
    match root {
        FrameRoot::Owner => "owner",
        FrameRoot::Yaml => "yaml_package",
        FrameRoot::Frozen => "frozen",
    }
}

/// The logical traceback, outermost first: `{file, line, function, root}` per entry, the file relative to its root.
fn frames_value(frames: &[SourceFrame]) -> PyValue {
    PyValue::List(
        frames
            .iter()
            .map(|frame| {
                let mut item = PyDict::new();
                item.set("file", string(frame.file));
                item.set("line", PyValue::int(frame.line));
                item.set("function", string(frame.function));
                item.set("root", string(root_name(frame.root)));
                PyValue::Dict(item)
            })
            .collect(),
    )
}

/// The typed logical import binding of the owner module: the native module is `node_role_contract`; the parent maps it to the import alias
/// of the harness that loaded the original.
fn module_binding() -> PyValue {
    let mut binding = PyDict::new();
    binding.set("logical_module", string(MODULE));
    binding.set("kind", string("owner_module"));
    binding.set("import_alias", string("parent_supplied"));
    PyValue::Dict(binding)
}

/// What the logical frames mean and what the parent composes around them (see `docs/ABI.md`).
fn frame_context() -> PyValue {
    let mut roots = PyDict::new();
    let mut owner = PyDict::new();
    owner.set("module", string(MODULE));
    owner.set("file", string(role_contract::OWNER_FILE));
    owner.set(
        "resolution",
        string("the parent joins the directory of the owner file it imported"),
    );
    roots.set("owner", PyValue::Dict(owner));
    let mut yaml = PyDict::new();
    yaml.set("package", string("yaml"));
    yaml.set("source_version", string("6.0.1"));
    yaml.set(
        "resolution",
        string("the parent joins the site-packages directory of the reference PyYAML"),
    );
    roots.set("yaml_package", PyValue::Dict(yaml));
    let mut frozen = PyDict::new();
    frozen.set(
        "resolution",
        string("the file text is the interpreter's own frozen-module name"),
    );
    roots.set("frozen", PyValue::Dict(frozen));
    let mut transport = PyDict::new();
    transport.set("composed_by", string("parent"));
    transport.set("native_contribution", string("none"));
    transport.set("meaning", string("the outermost caller frame of the transport that invoked the owner is added by the parent; it is not an owner or library frame"));
    let mut context = PyDict::new();
    context.set("schema", string("source-compat-frames/1"));
    context.set("order", string("outermost first"));
    context.set(
        "provenance",
        string("logical source-compatibility graph computed from the source mapping of the original (owner file, PyYAML 6.0.1, frozen stdlib 3.12.3) and the call state of the port; not a native Rust or kernel stack and not a Python traceback observed natively"),
    );
    context.set("roots", PyValue::Dict(roots));
    context.set("transport_context", PyValue::Dict(transport));
    PyValue::Dict(context)
}

fn record(request: &Request, ended: Ended) -> PyValue {
    // a value the typed DTO cannot carry (a self-containing structure) is a named boundary, never a null
    let ended = match ended {
        Ended::Value { value, input_ref } => match encode(&value) {
            Ok(_) => Ended::Value { value, input_ref },
            Err(problem) => Ended::OutOfScope {
                boundary: "value_dto".to_string(),
                detail: format!("the returned value has no typed DTO: {problem}"),
            },
        },
        // an exception whose logical traceback the owner/loader cannot place from the source map is a named boundary, never frames: []
        Ended::Exception { frames: None, error, .. } => Ended::OutOfScope {
            boundary: "exception_frames_unmapped".to_string(),
            detail: format!(
                "{} raised where the source map has no traceback chain (scanner, constructor or a late reader chunk of the YAML loader)",
                error.class
            ),
        },
        other => other,
    };
    let dto = |value: &PyValue| encode(value).unwrap_or(PyValue::None);
    let mut row = PyDict::new();
    row.set("schema", string(SCHEMA));
    row.set("id", request.id.clone());
    let kind = match &ended {
        Ended::Value { .. } => "value",
        Ended::Exception { .. } => "exception",
        Ended::OutOfScope { .. } => "out_of_scope",
    };
    row.set("kind", string(kind));
    row.set("phase", string("direct_call_attempt"));
    row.set("owner_call_attempted", PyValue::Bool(true));
    row.set("stdout", string(""));
    row.set("stderr", string(""));
    row.set("args_after", dto(&PyValue::List(request.args.clone())));
    row.set("kwargs_after", dto(&PyValue::Dict(request.kwargs.clone())));
    row.set("globals_after", canonical_globals());
    match ended {
        Ended::Value { value, input_ref } => {
            row.set("value", dto(&value));
            row.set("input_ref", input_ref);
        }
        Ended::Exception {
            error,
            from_none,
            frames,
        } => {
            let mut graph = exception_row(&error, from_none);
            graph.set("frames", frames_value(&frames.unwrap_or_default()));
            if error.module == crate::role_errors::OWNER_MODULE {
                graph.set("module_binding", module_binding());
            }
            row.set("exception", PyValue::Dict(graph));
            row.set("frame_context", frame_context());
        }
        Ended::OutOfScope { boundary, detail } => {
            row.set("boundary", string(&boundary));
            row.set("detail", string(&detail));
        }
    }
    PyValue::Dict(row)
}

/// Answers one request line.
pub fn answer(line: &PyStr) -> PyValue {
    let request = match parse_line(line) {
        Ok(request) => request,
        Err(row) => return row,
    };
    let signature = request.operation.signature();
    let ended = match signature.bind(&request.args, &request.kwargs) {
        // the interpreter raises a binding `TypeError` in the caller's frame: the owner contributes no frame
        Err(failure) => Ended::Exception {
            error: Box::new(binding_error(failure)),
            from_none: false,
            frames: Some(Vec::new()),
        },
        Ok(bound) => {
            let values: Vec<PyValue> = (0..signature.params.len())
                .map(|i| bound[i].clone())
                .collect();
            run(&request, &values)
        }
    };
    record(&request, ended)
}

/// `evaluate-jsonl`: every non-blank line answered in order, one JSON line each.
pub fn evaluate_jsonl(input: &[u8]) -> Result<Vec<u8>, Failure> {
    let text = PyStr::from_surrogateescape(input);
    let newline = u32::from('\n');
    let mut out = Vec::new();
    for piece in text.code_points().split(|&point| point == newline) {
        let line = PyStr::from_code_points(piece.to_vec());
        if line.strip().is_empty() {
            continue;
        }
        out.extend_from_slice(crate::api::render(&answer(&line))?.as_bytes());
        out.push(b'\n');
    }
    Ok(out)
}
