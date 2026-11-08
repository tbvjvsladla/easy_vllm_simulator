//! The two public functions of `classify_failure.py` as a JSON-lines service, schema `classify_failure-api/1`, implementing Root's immutable proposal
//! (`docs/ROOT_ABI_PROPOSAL.json`). `docs/ABI.md` states the contract as implemented.
//!
//! **Request** (one JSON object per line; all six members are required, any other member is refused): `schema`, `id` (a string), `operation`
//! (`classify_failure.load_patterns | classify_failure.matched_lines`), `args_dto` (list DTO), `kwargs_dto` (dict DTO), `globals`
//! (`{"PATTERNS_FILE": ["str", text]}`, echoed verbatim in `globals_after`; the service never reads it: `load_patterns` takes its path as an argument).
//! Arguments are bound as CPython binds them.
//!
//! **Record**: `schema`, `id`, `kind`, `phase`, `owner_call_attempted`, `stdout`, `stderr` (the business streams, always empty: neither function
//! prints), plus per `kind`: `value` (`args_after`, `kwargs_after`, `globals_after`, `value`, `input_ref`), `exception` (`args_after`, `kwargs_after`,
//! `globals_after`, `exception`: the typed graph of [`crate::wheel_api::exception_row`]), `out_of_scope` (`boundary`, `detail`, with the three `*_after`),
//! `transport_failure` (never produced here) and `request_error` (`message`).
//!
//! **`input_ref`** is `null` for a value that is a new object, or the path of the caller object the value *is* (`[{"kind":"args"},{"kind":"list_item","index":0}]`
//! or `[{"kind":"kwargs"},{"kind":"dict_value","index":N}]`), computed from the string operations of the source ([`crate::classify_source::Origin`]).
//! The shared empty-string and one-character-Latin-1 objects of the interpreter are NOT claimed as aliases of an equal argument: such a value is `null`.

use crate::classify_source::{load_patterns, matched_lines, Origin, SourceFailure};
use crate::dto::encode;
use crate::dto_decode::{decode_args, decode_kwargs};
use crate::http_direct::PyException;
use crate::wheel_api::exception_row;
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pycall::{required, Param, Signature};
use agent_runtime_support::pyjson::loads_text_objects;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};

pub const SCHEMA: &str = "classify_failure-api/1";
pub const MODULE: &str = "classify_failure";
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
    LoadPatterns,
    MatchedLines,
}

fn default_max_lines() -> PyValue {
    PyValue::int(5)
}
fn default_max_chars() -> PyValue {
    PyValue::int(1000)
}

impl Operation {
    fn named(name: &str) -> Option<Operation> {
        match name {
            "load_patterns" => Some(Operation::LoadPatterns),
            "matched_lines" => Some(Operation::MatchedLines),
            _ => None,
        }
    }

    fn signature(self) -> Signature {
        const LOAD: &[Param] = &[required("path")];
        const MATCHED: &[Param] = &[
            required("text"),
            required("pattern"),
            Param {
                name: "max_lines",
                default: Some(default_max_lines),
            },
            Param {
                name: "max_chars",
                default: Some(default_max_chars),
            },
        ];
        let (name, params) = match self {
            Operation::LoadPatterns => ("load_patterns", LOAD),
            Operation::MatchedLines => ("matched_lines", MATCHED),
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
    globals: PyValue,
}

fn valid_globals(globals: &PyValue) -> bool {
    let PyValue::Dict(dict) = globals else {
        return false;
    };
    if dict.iter().count() != 1 {
        return false;
    }
    matches!(
        dict.get("PATTERNS_FILE"),
        Some(PyValue::List(items))
            if items.len() == 2 && items[0] == string("str") && matches!(items[1], PyValue::Str(_))
    )
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
    let globals = members.get("globals").cloned().unwrap_or(PyValue::None);
    if !valid_globals(&globals) {
        return Err(request_error(
            id,
            "globals must be {\"PATTERNS_FILE\": [\"str\", text]} and nothing else",
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
        globals,
    })
}

enum Ended {
    Value { value: PyValue, input_ref: PyValue },
    Exception(Box<PyException>),
    OutOfScope { boundary: String, detail: String },
}

fn binding_error(failure: Failure) -> PyException {
    crate::py_errors::builtin(
        failure.class,
        &failure.message.to_utf8().unwrap_or_default(),
    )
}

fn path_item(kind: &str, index: Option<usize>) -> PyValue {
    let mut item = PyDict::new();
    item.set("kind", string(kind));
    if let Some(index) = index {
        item.set("index", PyValue::int(index));
    }
    PyValue::Dict(item)
}

/// The observation-graph path of the object bound to the first parameter (`text`).
fn text_path(request: &Request) -> PyValue {
    if !request.args.is_empty() {
        return PyValue::List(vec![
            path_item("args", None),
            path_item("list_item", Some(0)),
        ]);
    }
    let index = request
        .kwargs
        .iter()
        .position(|(key, _)| key.equals("text"))
        .unwrap_or(0);
    PyValue::List(vec![
        path_item("kwargs", None),
        path_item("dict_value", Some(index)),
    ])
}

fn run(request: &Request, bound: &[PyValue]) -> Ended {
    match request.operation {
        Operation::LoadPatterns => match load_patterns(&bound[0]) {
            Ok(loaded) => Ended::Value {
                value: PyValue::List(loaded.patterns.into_iter().map(PyValue::Dict).collect()),
                input_ref: PyValue::None,
            },
            Err(failure) => failed(failure),
        },
        Operation::MatchedLines => {
            match matched_lines(&bound[0], &bound[1], &bound[2], &bound[3]) {
                Ok(result) => Ended::Value {
                    value: result.value,
                    input_ref: if result.origin == Origin::Text {
                        text_path(request)
                    } else {
                        PyValue::None
                    },
                },
                Err(failure) => failed(failure),
            }
        }
    }
}

fn failed(failure: SourceFailure) -> Ended {
    match failure {
        SourceFailure::Raised(error) => Ended::Exception(error),
        SourceFailure::OutOfScope { boundary, detail } => Ended::OutOfScope {
            boundary: boundary.to_string(),
            detail,
        },
    }
}

fn record(request: &Request, ended: Ended) -> PyValue {
    let dto = |value: &PyValue| encode(value).unwrap_or(PyValue::None);
    let mut row = PyDict::new();
    row.set("schema", string(SCHEMA));
    row.set("id", request.id.clone());
    let kind = match &ended {
        Ended::Value { .. } => "value",
        Ended::Exception(_) => "exception",
        Ended::OutOfScope { .. } => "out_of_scope",
    };
    row.set("kind", string(kind));
    row.set("phase", string("direct_call_attempt"));
    row.set("owner_call_attempted", PyValue::Bool(true));
    row.set("stdout", string(""));
    row.set("stderr", string(""));
    row.set("args_after", dto(&PyValue::List(request.args.clone())));
    row.set("kwargs_after", dto(&PyValue::Dict(request.kwargs.clone())));
    row.set("globals_after", request.globals.clone());
    match ended {
        Ended::Value { value, input_ref } => {
            row.set("value", dto(&value));
            row.set("input_ref", input_ref);
        }
        Ended::Exception(error) => {
            row.set("exception", PyValue::Dict(exception_row(&error, false)))
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
        Err(failure) => Ended::Exception(Box::new(binding_error(failure))),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(args: &str, kwargs: &str, operation: &str) -> PyDict {
        let line = format!(
            r#"{{"schema":"{SCHEMA}","id":"t","operation":"classify_failure.{operation}","args_dto":{args},"kwargs_dto":{kwargs},"globals":{{"PATTERNS_FILE":["str","/x/scripts/../failure_patterns.yaml"]}}}}"#
        );
        match answer(&PyStr::new(&line)) {
            PyValue::Dict(row) => row,
            other => panic!("{other:?}"),
        }
    }

    fn member<'a>(row: &'a PyDict, key: &str) -> &'a PyValue {
        row.get(key).unwrap_or_else(|| panic!("no member {key}"))
    }

    #[test]
    fn a_value_that_is_the_caller_text_names_its_argument_path_and_a_new_string_is_null() {
        let row = ask(
            r#"["list",[["str","ababa"],["str","(?=aba)"]]]"#,
            r#"["dict",[]]"#,
            "matched_lines",
        );
        assert_eq!(member(&row, "kind"), &string("value"));
        let expected = PyValue::List(vec![
            path_item("args", None),
            path_item("list_item", Some(0)),
        ]);
        assert_eq!(member(&row, "input_ref"), &expected);
        let row = ask(
            r#"["list",[["str","  a FAIL  "],["str","FAIL"]]]"#,
            r#"["dict",[]]"#,
            "matched_lines",
        );
        assert_eq!(member(&row, "input_ref"), &PyValue::None);
        let row = ask(
            r#"["list",[["str","a\nb"],["str","a.b"]]]"#,
            r#"["dict",[]]"#,
            "matched_lines",
        );
        assert_eq!(member(&row, "value"), &encode(&string("")).unwrap());
        assert_eq!(member(&row, "input_ref"), &PyValue::None);
        assert_eq!(member(&row, "globals_after"), &decode_args_globals());
    }

    fn decode_args_globals() -> PyValue {
        let mut globals = PyDict::new();
        globals.set(
            "PATTERNS_FILE",
            PyValue::List(vec![
                string("str"),
                string("/x/scripts/../failure_patterns.yaml"),
            ]),
        );
        PyValue::Dict(globals)
    }

    #[test]
    fn keyword_text_is_named_by_its_position_among_the_keywords_and_exceptions_are_graphs() {
        let row = ask(
            r#"["list",[]]"#,
            r#"["dict",[[["str","pattern"],["str","(?=a)"]],[["str","text"],["str","a"]]]]"#,
            "matched_lines",
        );
        let expected = PyValue::List(vec![
            path_item("kwargs", None),
            path_item("dict_value", Some(1)),
        ]);
        assert_eq!(member(&row, "input_ref"), &expected);
        let row = ask(
            r#"["list",[["str","text"],["str","["]]]"#,
            r#"["dict",[]]"#,
            "matched_lines",
        );
        assert_eq!(member(&row, "kind"), &string("exception"));
        let PyValue::Dict(exception) = member(&row, "exception") else {
            panic!()
        };
        assert_eq!(exception.get("module"), Some(&string("re")));
        assert!(exception.get("constructor").is_some());
        let row = ask(r#"["list",[]]"#, r#"["dict",[]]"#, "matched_lines");
        assert_eq!(member(&row, "kind"), &string("exception"));
    }

    #[test]
    fn the_request_shape_is_strict() {
        let row = match answer(&PyStr::new(
            r#"{"schema":"classify_failure-api/1","id":"t","operation":"classify_failure.load_patterns"}"#,
        )) {
            PyValue::Dict(row) => row,
            other => panic!("{other:?}"),
        };
        assert_eq!(member(&row, "kind"), &string("request_error"));
        let row = ask(
            r#"["list",[["str","/definitely/not/here"]]]"#,
            r#"["dict",[]]"#,
            "load_patterns",
        );
        assert_eq!(member(&row, "kind"), &string("exception"));
    }
}
