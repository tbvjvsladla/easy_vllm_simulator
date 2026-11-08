//! The five public functions of `regen_requirements.py` as a JSON-lines service, in the ABI `regen_requirements-api/1` of the frozen Root proposal
//! (`abi-proposal.json`, `producer-slots.json`). `ABI.md` of the R39 work states the final contract.
//!
//! **Request** (one JSON object per line, exactly these members): `schema` (the schema string), `id` (a string), `operation`
//! (`regen_requirements.parse_requires | wheel_metadata | from_wheel_url | installed_metadata | from_installed`), `args_dto` (list DTO),
//! `kwargs_dto` (dict DTO), `globals` (`{BASE_PROVIDED: set DTO, KNOWN_INCOMPAT: ordered dict DTO}`, which must equal the source's own constants: they are
//! never replaced). Arguments are bound as CPython binds them. Any other member, or a malformed value, is a `request_error`.
//!
//! **Record**: `schema`, `id`, `kind`, `phase`, `owner_call_attempted`, `stdout`, `stderr` (the BUSINESS streams: empty, none of these functions prints)
//! and `provenance`, plus per `kind`: `value` (`args_after`, `kwargs_after`, `globals_after`, `value`, `return_aliases` always empty),
//! `exception` (`args_after`, `kwargs_after`, `globals_after`, `exception`: the typed graph of [`crate::wheel_api::exception_row`], now with the
//! observer's `path` and `content` attributes), `out_of_scope` (`boundary`, `detail`), `transport_failure` (`detail`; `phase` carries the transport phase)
//! and `request_error` (`message`).
//!
//! **Provenance** is the product's own declaration, never a kernel or interpreter audit: `source` is `native-declared`, `temp_producers` lists the
//! temporary directories this call really made (`kind` `temporary_directory`, the absolute `path`, `created`, `cleanup_complete`), and
//! `metadata_read_attempts` the metadata files it really tried to read.

use crate::dto::encode;
use crate::dto_decode::{decode_args, decode_kwargs};
use crate::fetch_https::FetchEnvironment;
use crate::http_direct::{Dial, PyException, UreqDialer};
use crate::installed_dist::{InstalledError, InstalledProfile};
use crate::regen_flow::{
    from_installed, from_wheel_url, installed_metadata, wheel_metadata, FlowError,
};
use crate::requirements_rules::{parse_requires, BASE_PROVIDED, KNOWN_INCOMPAT};
use crate::tempdir::{Provenance, TempBase};
use crate::wheel_api::exception_row;
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pycall::{required, Param, Signature};
use agent_runtime_support::pyjson::loads_text_objects;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{Opaque, PyDict, PyValue, Typed};
use std::cell::RefCell;

pub const SCHEMA: &str = "regen_requirements-api/1";
pub const MODULE: &str = "regen_requirements";
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
    ParseRequires,
    WheelMetadata,
    FromWheelUrl,
    InstalledMetadata,
    FromInstalled,
}

fn none_default() -> PyValue {
    PyValue::None
}

impl Operation {
    fn named(name: &str) -> Option<Operation> {
        Some(match name {
            "parse_requires" => Operation::ParseRequires,
            "wheel_metadata" => Operation::WheelMetadata,
            "from_wheel_url" => Operation::FromWheelUrl,
            "installed_metadata" => Operation::InstalledMetadata,
            "from_installed" => Operation::FromInstalled,
            _ => return None,
        })
    }

    fn signature(self) -> Signature {
        const PARSE: &[Param] = &[
            required("reqs"),
            Param {
                name: "version",
                default: Some(none_default),
            },
        ];
        const URL: &[Param] = &[required("url")];
        const NOTHING: &[Param] = &[];
        let (name, params) = match self {
            Operation::ParseRequires => ("parse_requires", PARSE),
            Operation::WheelMetadata => ("wheel_metadata", URL),
            Operation::FromWheelUrl => ("from_wheel_url", URL),
            Operation::InstalledMetadata => ("installed_metadata", NOTHING),
            Operation::FromInstalled => ("from_installed", NOTHING),
        };
        Signature {
            name,
            params,
            rest: false,
        }
    }
}

/// What the service runs on: the transport environment, the dialer, `PYTHONPATH` and the temporary-directory base.
pub struct ApiEnvironment<'a> {
    pub fetch: &'a FetchEnvironment,
    pub dialer: &'a dyn Dial,
    pub pythonpath: Option<std::ffi::OsString>,
    pub base: Option<TempBase>,
    resolved_base: RefCell<Option<TempBase>>,
}

impl<'a> ApiEnvironment<'a> {
    pub fn new(
        fetch: &'a FetchEnvironment,
        dialer: &'a dyn Dial,
        pythonpath: Option<std::ffi::OsString>,
        base: Option<TempBase>,
    ) -> ApiEnvironment<'a> {
        ApiEnvironment {
            fetch,
            dialer,
            pythonpath,
            base,
            resolved_base: RefCell::new(None),
        }
    }

    /// `tempfile.gettempdir()` of this process (probed on first use), or the fixed base.
    fn temp_base(&self) -> Result<TempBase, Box<PyException>> {
        if self.resolved_base.borrow().is_none() {
            let base = match &self.base {
                Some(base) => base.clone(),
                None => TempBase::of_process()?,
            };
            *self.resolved_base.borrow_mut() = Some(base);
        }
        Ok(self.resolved_base.borrow().clone().expect("resolved above"))
    }
}

/// The source's module constants as the DTO the saved originals write.
fn canonical_globals() -> PyValue {
    let members = BASE_PROVIDED
        .iter()
        .map(|name| PyValue::str(name))
        .collect();
    let set = PyValue::Opaque(Opaque {
        type_name: "set",
        repr: "{...}".to_string(),
        truthy: true,
        typed: Typed::Set(members),
        identity: None,
    });
    let mut incompat = PyDict::new();
    for (name, ceiling) in KNOWN_INCOMPAT {
        incompat.set(name, PyValue::str(ceiling));
    }
    let mut globals = PyDict::new();
    globals.set("BASE_PROVIDED", encode(&set).unwrap_or(PyValue::None));
    globals.set(
        "KNOWN_INCOMPAT",
        encode(&PyValue::Dict(incompat)).unwrap_or(PyValue::None),
    );
    PyValue::Dict(globals)
}

fn string(text: &str) -> PyValue {
    PyValue::str(text)
}

fn provenance_value(provenance: &Provenance) -> PyValue {
    let mut row = PyDict::new();
    row.set("source", string("native-declared"));
    let producers = provenance
        .temp_producers
        .iter()
        .map(|producer| {
            let mut item = PyDict::new();
            item.set("kind", string("temporary_directory"));
            item.set("path", string(&producer.path));
            item.set("created", PyValue::Bool(producer.created));
            item.set("cleanup_complete", PyValue::Bool(producer.cleanup_complete));
            PyValue::Dict(item)
        })
        .collect();
    row.set("temp_producers", PyValue::List(producers));
    let attempts = provenance
        .metadata_read_attempts
        .iter()
        .map(|path| {
            let mut item = PyDict::new();
            item.set("path", string(path));
            PyValue::Dict(item)
        })
        .collect();
    row.set("metadata_read_attempts", PyValue::List(attempts));
    PyValue::Dict(row)
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
    row.set("provenance", provenance_value(&Provenance::default()));
    row.set("message", string(message));
    PyValue::Dict(row)
}

/// A request line that passed the shape checks.
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
    if let Some(globals) = members.get("globals") {
        if *globals != canonical_globals() {
            return Err(request_error(id, "globals must equal the source's BASE_PROVIDED set and KNOWN_INCOMPAT dict (output DTO form): the constants are never replaced"));
        }
    }
    let args = match members.get("args_dto") {
        None => Vec::new(),
        Some(dto) => decode_args(dto)
            .map_err(|problem| request_error(id.clone(), &format!("args_dto: {problem}")))?,
    };
    let kwargs = match members.get("kwargs_dto") {
        None => PyDict::new(),
        Some(dto) => decode_kwargs(dto)
            .map_err(|problem| request_error(id.clone(), &format!("kwargs_dto: {problem}")))?,
    };
    Ok(Request {
        id,
        operation,
        args,
        kwargs,
    })
}

/// How a call ended.
enum Ended {
    Value(PyValue),
    Exception(Box<PyException>),
    OutOfScope { boundary: String, detail: String },
    Transport { phase: String, detail: String },
}

impl From<FlowError> for Ended {
    fn from(error: FlowError) -> Ended {
        match error {
            FlowError::Raised { error, .. } => Ended::Exception(error),
            FlowError::OutOfScope { boundary, detail } => Ended::OutOfScope { boundary, detail },
            FlowError::Transport(transport) => Ended::Transport {
                phase: transport.phase.to_string(),
                detail: transport.detail,
            },
        }
    }
}

fn binding_error(failure: Failure) -> PyException {
    crate::py_errors::builtin(
        failure.class,
        &failure.message.to_utf8().unwrap_or_default(),
    )
}

fn texts(items: Vec<PyStr>) -> PyValue {
    PyValue::List(items.into_iter().map(PyValue::Str).collect())
}

fn installed_profile(environment: &ApiEnvironment<'_>) -> Result<InstalledProfile, Ended> {
    InstalledProfile::from_pythonpath(environment.pythonpath.as_deref()).map_err(
        |error| match error {
            InstalledError::OutOfScope { boundary, detail } => Ended::OutOfScope {
                boundary: boundary.to_string(),
                detail,
            },
            InstalledError::Raised(error) => Ended::Exception(error),
        },
    )
}

fn run(
    operation: Operation,
    bound: &[PyValue],
    environment: &ApiEnvironment<'_>,
    provenance: &mut Provenance,
) -> Ended {
    let flow = |result: Result<Vec<PyStr>, FlowError>| match result {
        Ok(requires) => Ended::Value(texts(requires)),
        Err(error) => error.into(),
    };
    match operation {
        Operation::ParseRequires => match parse_requires(&bound[0], &bound[1]) {
            Ok(parsed) => Ended::Value(PyValue::Tuple(vec![
                texts(parsed.lines),
                texts(parsed.applied),
            ])),
            Err(error) => Ended::Exception(error),
        },
        Operation::WheelMetadata | Operation::FromWheelUrl => {
            let base = match environment.temp_base() {
                Ok(base) => base,
                Err(error) => return Ended::Exception(error),
            };
            if operation == Operation::WheelMetadata {
                match wheel_metadata(
                    &bound[0],
                    environment.fetch,
                    environment.dialer,
                    &base,
                    provenance,
                ) {
                    Ok((requires, version)) => {
                        Ended::Value(PyValue::Tuple(vec![texts(requires), PyValue::Str(version)]))
                    }
                    Err(error) => error.into(),
                }
            } else {
                flow(from_wheel_url(
                    &bound[0],
                    environment.fetch,
                    environment.dialer,
                    &base,
                    provenance,
                ))
            }
        }
        Operation::InstalledMetadata | Operation::FromInstalled => {
            let profile = match installed_profile(environment) {
                Ok(profile) => profile,
                Err(ended) => return ended,
            };
            if operation == Operation::InstalledMetadata {
                match installed_metadata(&profile, provenance) {
                    Ok((requires, version)) => {
                        Ended::Value(PyValue::Tuple(vec![texts(requires), PyValue::Str(version)]))
                    }
                    Err(error) => error.into(),
                }
            } else {
                flow(from_installed(&profile, provenance))
            }
        }
    }
}

fn record(request: &Request, ended: Ended, provenance: &Provenance) -> PyValue {
    let dto = |value: &PyValue| encode(value).unwrap_or(PyValue::None);
    let mut row = PyDict::new();
    row.set("schema", string(SCHEMA));
    row.set("id", request.id.clone());
    let (kind, phase) = match &ended {
        Ended::Value(_) => ("value", "direct_call_attempt".to_string()),
        Ended::Exception(_) => ("exception", "direct_call_attempt".to_string()),
        Ended::OutOfScope { .. } => ("out_of_scope", "direct_call_attempt".to_string()),
        Ended::Transport { phase, .. } => ("transport_failure", phase.clone()),
    };
    row.set("kind", string(kind));
    row.set("phase", string(&phase));
    row.set("owner_call_attempted", PyValue::Bool(true));
    row.set("stdout", string(""));
    row.set("stderr", string(""));
    row.set("provenance", provenance_value(provenance));
    row.set("args_after", dto(&PyValue::List(request.args.clone())));
    row.set("kwargs_after", dto(&PyValue::Dict(request.kwargs.clone())));
    row.set("globals_after", canonical_globals());
    match ended {
        Ended::Value(value) => {
            row.set("value", dto(&value));
            row.set("return_aliases", PyValue::List(Vec::new()));
        }
        Ended::Exception(error) => {
            row.set("exception", PyValue::Dict(exception_row(&error, false)))
        }
        Ended::OutOfScope { boundary, detail } => {
            row.set("boundary", string(&boundary));
            row.set("detail", string(&detail));
        }
        Ended::Transport { detail, .. } => row.set("detail", string(&detail)),
    }
    PyValue::Dict(row)
}

/// Answers one request line.
pub fn answer_with(line: &PyStr, environment: &ApiEnvironment<'_>) -> PyValue {
    let request = match parse_line(line) {
        Ok(request) => request,
        Err(row) => return row,
    };
    let mut provenance = Provenance::default();
    let signature = request.operation.signature();
    let ended = match signature.bind(&request.args, &request.kwargs) {
        Err(failure) => Ended::Exception(Box::new(binding_error(failure))),
        Ok(bound) => {
            let values: Vec<PyValue> = (0..signature.params.len())
                .map(|index| bound[index].clone())
                .collect();
            run(request.operation, &values, environment, &mut provenance)
        }
    };
    record(&request, ended, &provenance)
}

/// `evaluate-jsonl` over this process's environment: every non-blank line answered in order, one JSON line each.
pub fn evaluate_jsonl(input: &[u8]) -> Result<Vec<u8>, Failure> {
    let fetch = FetchEnvironment::from_process();
    let dialer = UreqDialer::unbounded();
    let environment = ApiEnvironment::new(&fetch, &dialer, std::env::var_os("PYTHONPATH"), None);
    evaluate_with(input, &environment)
}

/// [`evaluate_jsonl`] on an explicit environment (the checks' owned one).
pub fn evaluate_with(input: &[u8], environment: &ApiEnvironment<'_>) -> Result<Vec<u8>, Failure> {
    let text = PyStr::from_surrogateescape(input);
    let newline = u32::from('\n');
    let mut out = Vec::new();
    for piece in text.code_points().split(|&point| point == newline) {
        let line = PyStr::from_code_points(piece.to_vec());
        if line.strip().is_empty() {
            continue;
        }
        out.extend_from_slice(crate::api::render(&answer_with(&line, environment))?.as_bytes());
        out.push(b'\n');
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    fn root() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "regen-api-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn ask(request: &str, environment: &ApiEnvironment<'_>) -> PyDict {
        match answer_with(&PyStr::new(request), environment) {
            PyValue::Dict(row) => row,
            other => panic!("a record object, not {other:?}"),
        }
    }

    fn member<'a>(row: &'a PyDict, key: &str) -> &'a PyValue {
        row.get(key).unwrap_or_else(|| panic!("no member {key}"))
    }

    fn line(id: &str, operation: &str, args: &str) -> String {
        let globals = crate::api::render(&canonical_globals()).unwrap();
        format!(
            r#"{{"schema":"{SCHEMA}","id":"{id}","operation":"regen_requirements.{operation}","args_dto":{args},"kwargs_dto":["dict",[]],"globals":{globals}}}"#
        )
    }

    fn environment<'a>(
        fetch: &'a FetchEnvironment,
        dialer: &'a UreqDialer,
        pythonpath: Option<&str>,
        base: Option<TempBase>,
    ) -> ApiEnvironment<'a> {
        ApiEnvironment::new(
            fetch,
            dialer,
            pythonpath.map(std::ffi::OsString::from),
            base,
        )
    }

    #[test]
    fn parse_requires_returns_the_two_lists_as_a_tuple_and_echoes_the_constants() {
        let (fetch, dialer) = (FetchEnvironment::default(), UreqDialer::unbounded());
        let env = environment(&fetch, &dialer, None, None);
        let row = ask(
            &line(
                "a",
                "parse_requires",
                r#"["list",[["list",[["str","fastapi>=1"],["str","numpy"]]],["none"]]]"#,
            ),
            &env,
        );
        assert_eq!(member(&row, "kind"), &string("value"));
        assert_eq!(
            member(&row, "value"),
            &encode(&PyValue::Tuple(vec![
                texts(vec![PyStr::new("fastapi>=1,<0.137.0")]),
                texts(vec![PyStr::new("fastapi<0.137.0")])
            ]))
            .unwrap()
        );
        assert_eq!(member(&row, "globals_after"), &canonical_globals());
        assert_eq!(member(&row, "return_aliases"), &PyValue::List(Vec::new()));
        let provenance = member(&row, "provenance");
        assert!(
            matches!(provenance, PyValue::Dict(p) if p.get("source") == Some(&string("native-declared")))
        );
    }

    #[test]
    fn binding_and_request_errors_follow_the_contract() {
        let (fetch, dialer) = (FetchEnvironment::default(), UreqDialer::unbounded());
        let env = environment(&fetch, &dialer, None, None);
        let row = ask(
            &line(
                "b",
                "parse_requires",
                r#"["list",[["list",[]],["none"],["none"]]]"#,
            ),
            &env,
        );
        let PyValue::Dict(exception) = member(&row, "exception") else {
            panic!("exception")
        };
        assert_eq!(
            exception.get("message"),
            Some(&string(
                "parse_requires() takes from 1 to 2 positional arguments but 3 were given"
            ))
        );
        for bad in [
            r#"{"schema":"other","id":"x","operation":"regen_requirements.from_installed"}"#.to_string(),
            r#"{"schema":"regen_requirements-api/1","id":"x","operation":"regen_requirements.main"}"#.to_string(),
            r#"{"schema":"regen_requirements-api/1","id":"x","operation":"regen_requirements.from_installed","extra":1}"#.to_string(),
            r#"{"schema":"regen_requirements-api/1","id":7,"operation":"regen_requirements.from_installed"}"#.to_string(),
            r#"{"schema":"regen_requirements-api/1","id":"x","operation":"regen_requirements.from_installed","globals":{"BASE_PROVIDED":["set",[]],"KNOWN_INCOMPAT":["dict",[]]}}"#.to_string(),
        ] {
            let row = ask(&bad, &env);
            assert_eq!(member(&row, "kind"), &string("request_error"), "{bad}");
            assert_eq!(member(&row, "owner_call_attempted"), &PyValue::Bool(false));
        }
    }

    #[test]
    fn installed_metadata_reads_the_declared_pythonpath_and_declares_its_reads() {
        let directory = root();
        let dist = directory.join("vllm-0.32.0.dist-info");
        std::fs::create_dir(&dist).unwrap();
        std::fs::write(
            dist.join("METADATA"),
            "Name: vllm\nVersion: 0.32.0\nRequires-Dist: a\n",
        )
        .unwrap();
        let (fetch, dialer) = (FetchEnvironment::default(), UreqDialer::unbounded());
        let env = environment(&fetch, &dialer, Some(&directory.to_string_lossy()), None);
        let row = ask(&line("c", "installed_metadata", r#"["list",[]]"#), &env);
        assert_eq!(
            member(&row, "value"),
            &encode(&PyValue::Tuple(vec![
                texts(vec![PyStr::new("a")]),
                string("0.32.0")
            ]))
            .unwrap()
        );
        let PyValue::Dict(provenance) = member(&row, "provenance") else {
            panic!("provenance")
        };
        let Some(PyValue::List(attempts)) = provenance.get("metadata_read_attempts") else {
            panic!("attempts")
        };
        assert_eq!(attempts.len(), 1);
        std::fs::remove_dir_all(&dist).unwrap();
        let row = ask(&line("d", "from_installed", r#"["list",[]]"#), &env);
        let PyValue::Dict(exception) = member(&row, "exception") else {
            panic!("exception")
        };
        assert_eq!(exception.get("type"), Some(&string("PackageNotFoundError")));
        assert_eq!(exception.get("relation"), Some(&string("context")));
        let Some(PyValue::Dict(attributes)) = exception.get("attributes") else {
            panic!("attributes")
        };
        let order: Vec<String> = attributes
            .iter()
            .map(|(name, _)| name.to_utf8().unwrap())
            .collect();
        assert_eq!(order, ["name", "path", "msg"]);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn a_short_wheel_body_reports_the_typed_content_bound_to_the_declared_temporary_directory() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap().to_string();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut head = Vec::new();
                let mut byte = [0u8; 1];
                while !head.ends_with(b"\r\n\r\n")
                    && stream.read(&mut byte).map(|n| n == 1).unwrap_or(false)
                {
                    head.push(byte[0]);
                }
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 455\r\nConnection: close\r\n\r\nshort",
                );
            }
        });
        let base_dir = root();
        let (fetch, dialer) = (FetchEnvironment::default(), UreqDialer::unbounded());
        let env = environment(
            &fetch,
            &dialer,
            None,
            Some(TempBase::fixed(base_dir.clone())),
        );
        let row = ask(
            &line(
                "e",
                "wheel_metadata",
                &format!(r#"["list",[["str","http://{address}/"]]]"#),
            ),
            &env,
        );
        let PyValue::Dict(exception) = member(&row, "exception") else {
            panic!("exception")
        };
        assert_eq!(exception.get("type"), Some(&string("ContentTooShortError")));
        let Some(PyValue::Dict(attributes)) = exception.get("attributes") else {
            panic!("attributes")
        };
        let Some(PyValue::List(content)) = attributes.get("content") else {
            panic!("content")
        };
        assert_eq!(content[0], string("tuple"));
        let PyValue::List(members) = &content[1] else {
            panic!("members")
        };
        let PyValue::List(path) = &members[0] else {
            panic!("path")
        };
        let PyValue::List(message) = &members[1] else {
            panic!("message")
        };
        assert_eq!(message[0], string("http_message"));
        let PyValue::Dict(node) = &message[1] else {
            panic!("node")
        };
        assert_eq!(
            (node.get("type"), node.get("module")),
            (Some(&string("HTTPMessage")), Some(&string("http.client")))
        );
        let PyValue::Dict(provenance) = member(&row, "provenance") else {
            panic!("provenance")
        };
        let Some(PyValue::List(producers)) = provenance.get("temp_producers") else {
            panic!("producers")
        };
        let PyValue::Dict(producer) = &producers[0] else {
            panic!("producer")
        };
        let Some(PyValue::Str(directory)) = producer.get("path") else {
            panic!("path")
        };
        let expected = format!("{}/pkg.whl", directory.to_utf8().unwrap());
        assert_eq!(path[1], string(&expected));
        assert_eq!(
            (producer.get("created"), producer.get("cleanup_complete")),
            (Some(&PyValue::Bool(true)), Some(&PyValue::Bool(true)))
        );
        assert_eq!(std::fs::read_dir(&base_dir).unwrap().count(), 0);
        std::fs::remove_dir_all(base_dir).unwrap();
    }
}
