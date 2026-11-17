//! `main()` of `node_role_contract.py`: the argument parser (`--self-test`, `evaluate ...`), `cmd_evaluate` with its `[node-role]` lines and exit codes
//! (0 consistent, 2 usage or parse failure, 5 contract violation), the JSON/value output and the translated `--self-test`.
//!
//! The policy lives in [`role_contract`]; this module is the shell only. The manifest file is read with `isfile` before the read, parsed by the library's
//! loader and handed to `evaluate_manifest` as the loaded structure (never serialised first).
//!
//! Top-level command lines decided here: no arguments (help, status 2), `-h`/`--help`, `--self-test` alone, and `evaluate ...` (parsed with the common
//! argument parser). Any other top-level form is a named refusal (status 70), not a guess at argparse's text for it.

use crate::host::Sink;
use crate::main_cmd::{argument_error, split_usage};
use crate::torch_main::Refusal;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use recipe_runtime::cliparse::{
    format_usage, parse, terminal_width, Kind, Opt, Outcome, Spec, Value,
};
use role_contract::load::{load_yaml, LoadFailure};
use role_contract::{evaluate_manifest, manifest_path_for, FIELDS};
use std::os::unix::ffi::OsStringExt;

pub const PROGRAM: &str = "node_role_contract.sh";

const HELP: &str = "usage: node_role_contract.sh [-h] [--self-test] {evaluate} ...\n\n토폴로지 축 노드 계약(불변식 A)의 결정론 판정기\n\npositional arguments:\n  {evaluate}\n    evaluate   manifest 의 노드 계약을 평가\n\noptions:\n  -h, --help   show this help message and exit\n  --self-test\n";

/// What a run is on.
pub struct RoleWorld<'a> {
    pub host: &'a mut dyn Sink,
    pub argv: &'a [PyStr],
    pub columns: Option<&'a str>,
}

fn refusal(boundary: &'static str, detail: impl Into<String>) -> Refusal {
    Refusal {
        boundary,
        detail: detail.into(),
    }
}

fn spec() -> Spec {
    let option = |flags: &'static [&'static str], dest, kind, help| Opt {
        flags,
        dest,
        kind,
        help,
        required: false,
        group: None,
    };
    Spec {
        prog: "node_role_contract.sh evaluate",
        description: "",
        options: vec![
            option(&["-h", "--help"], "help", Kind::Help, None),
            option(&["--manifest"], "manifest", Kind::Text, None),
            option(&["--repo"], "repo", Kind::Text, None),
            option(
                &["--topology"],
                "topology",
                Kind::Choice(&["single", "multi"]),
                Some("manifest 의 topology 를 덮어쓴다(통로 선택 겸용)."),
            ),
            option(
                &["--role"],
                "role",
                Kind::Text,
                Some("rank 를 물어볼 role 슬러그(기본 sub)"),
            ),
            option(&["--field"], "field", Kind::Choice(&FIELDS), None),
            option(
                &["--format"],
                "format",
                Kind::Choice(&["json", "value"]),
                None,
            ),
        ],
        group_required: false,
        positional: None,
    }
}

fn say(host: &mut dyn Sink, text: &str) {
    let _ = host.out(&PyStr::new(text));
}

fn print_to(host: &mut dyn Sink, text: &PyStr) {
    let _ = host.out(text);
    let _ = host.out(&PyStr::new("\n"));
}

fn eprint(host: &mut dyn Sink, text: &PyStr) {
    let _ = host.err(text);
    let _ = host.err(&PyStr::new("\n"));
}

fn sorted_keys(value: &PyValue) -> PyValue {
    match value.peel() {
        PyValue::Dict(dict) => {
            let mut entries: Vec<&(PyStr, PyValue)> = dict.iter().collect();
            entries.sort_by(|a, b| a.0.code_points().cmp(b.0.code_points()));
            let mut sorted = PyDict::new();
            for (key, item) in entries {
                sorted.insert(key.clone(), sorted_keys(item));
            }
            PyValue::Dict(sorted)
        }
        PyValue::List(items) => PyValue::List(items.iter().map(sorted_keys).collect()),
        PyValue::Tuple(items) => PyValue::Tuple(items.iter().map(sorted_keys).collect()),
        other => other.clone(),
    }
}

fn is_file(path: &PyStr) -> bool {
    let Ok(bytes) = path.to_surrogateescape_bytes() else {
        return false;
    };
    if bytes.contains(&0) {
        return false;
    }
    std::fs::metadata(std::path::PathBuf::from(std::ffi::OsString::from_vec(
        bytes,
    )))
    .map(|m| m.is_file())
    .unwrap_or(false)
}

/// `str(exc)` of a failure of `load_yaml`.
fn failure_text(failure: LoadFailure) -> Result<PyStr, Refusal> {
    match crate::role_errors::load_failure(failure) {
        crate::role_errors::Raised::Exception { error, .. } => Ok(error.message.clone()),
        crate::role_errors::Raised::OutOfScope { boundary, detail } => {
            Err(refusal(boundary, detail))
        }
    }
}

fn parse_evaluate(world: &mut RoleWorld<'_>) -> Result<Result<Evaluate, i32>, Refusal> {
    let spec = spec();
    let usage = PyStr::new(&format_usage(&spec, terminal_width(world.columns)));
    let (status, stdout, stderr, head, rest) = match parse(&spec, &world.argv[2..], world.columns) {
        Outcome::Values(values) => {
            let text = |dest: &str| match values.get(dest) {
                Some(Value::Text(text)) => Some(text.clone()),
                _ => None,
            };
            return Ok(Ok(Evaluate {
                manifest: text("manifest"),
                repo: text("repo").unwrap_or_else(|| PyStr::new(".")),
                topology: text("topology"),
                role: text("role").unwrap_or_else(|| PyStr::new("sub")),
                field: text("field"),
                format: text("format").unwrap_or_else(|| PyStr::new("json")),
            }));
        }
        Outcome::Exit {
            status,
            stdout,
            stderr,
        } => {
            let (head, rest) = split_usage(&usage, &stderr);
            (status, stdout, stderr, head, rest)
        }
    };
    if !stdout.is_empty() {
        world.host.parser_help(&stdout);
    }
    let mut message = None;
    if !stderr.is_empty() || !rest.is_empty() {
        world.host.parser_error(&head, &rest);
        message = Some(rest);
    }
    if world.host.take_parser_failure().is_some() {
        let _ = message.as_ref().and_then(argument_error);
        return Err(refusal(
            "argparse_output_failure",
            "an argument-parser output write failed; its traceback was not captured",
        ));
    }
    Ok(Err(status))
}

struct Evaluate {
    manifest: Option<PyStr>,
    repo: PyStr,
    topology: Option<PyStr>,
    role: PyStr,
    field: Option<PyStr>,
    format: PyStr,
}

fn violation_line(host: &mut dyn Sink, violation: &role_contract::ContractViolation) {
    let mut text = PyStr::new(&format!("[node-role] VIOLATION {}: ", violation.code));
    text.push_py(&violation.message);
    eprint(host, &text);
}

fn field_value(result: &role_contract::Evaluation, field: &str) -> PyValue {
    if field == "topology" {
        return result.topology().clone();
    }
    let verdict = match field {
        "sub_mode" => result.sub_mode().map(|v| v.to_value()),
        "rank" => result.rank().map(|v| v.to_value()),
        "identity_authority" => result.identity_authority().map(|v| v.to_value()),
        "delivery_plane" => Some(result.delivery_plane().to_value()),
        _ => Some(result.tool_plane().to_value()),
    };
    match verdict {
        None => PyValue::str(""),
        Some(PyValue::Dict(dict)) => match dict.get("value") {
            None | Some(PyValue::None) => PyValue::str(""),
            Some(value) => value.clone(),
        },
        Some(other) => other,
    }
}

fn cmd_evaluate(world: &mut RoleWorld<'_>, args: Evaluate) -> Result<i32, Refusal> {
    let path = match args.manifest.clone().filter(|m| !m.is_empty()) {
        Some(path) => path,
        None => {
            let Some(topology) = args.topology.clone().filter(|t| !t.is_empty()) else {
                eprint(
                    world.host,
                    &PyStr::new(
                        "[node-role] FAIL: --manifest 또는 --topology 중 하나는 있어야 한다.",
                    ),
                );
                return Ok(2);
            };
            manifest_path_for(&PyValue::Str(args.repo.clone()), &PyValue::Str(topology))
                .map_err(|e| refusal("manifest_path", e.message()))?
        }
    };
    if !is_file(&path) {
        let mut text = PyStr::new("[node-role] FAIL: manifest 부재: ");
        text.push_py(&path);
        eprint(world.host, &text);
        return Ok(2);
    }
    let doc = match load_yaml(&PyValue::Str(path.clone())) {
        Ok(doc) => doc,
        Err(failure) => {
            let message = failure_text(failure)?;
            let mut text = PyStr::new("[node-role] FAIL: manifest 파싱 실패(");
            text.push_py(&path);
            text.push_str("): ");
            text.push_py(&message);
            eprint(world.host, &text);
            return Ok(2);
        }
    };
    let topology = args.topology.clone().map_or(PyValue::None, PyValue::Str);
    let result = evaluate_manifest(&doc, &topology, &PyValue::Str(args.role.clone()));
    let mut dict = match result.to_value() {
        PyValue::Dict(dict) => dict,
        other => return Err(refusal("evaluation_shape", format!("{other:?}"))),
    };
    dict.set("manifest_path", PyValue::Str(path.clone()));
    let full = PyValue::Dict(dict);
    let json_whole = args.format.equals("json") && args.field.is_none();
    let dump_failed = |error: agent_runtime_support::pyvalue::DumpError| {
        refusal(
            "json_dump",
            format!("json.dumps would raise ({error:?}); its traceback was not captured"),
        )
    };
    if json_whole {
        let text = sorted_keys(&full).dumps_indented(2).map_err(dump_failed)?;
        print_to(world.host, &text);
    } else if let Some(field) = &args.field {
        let name = field.to_utf8().unwrap_or_default();
        if !result.violations().is_empty() {
            for violation in result.violations() {
                violation_line(world.host, violation);
            }
        } else if args.format.equals("value") {
            print_to(world.host, &field_value(&result, &name).display_text());
        } else {
            let mut single = PyDict::new();
            let node = match full.peel() {
                PyValue::Dict(d) => d.get(&name).cloned().unwrap_or(PyValue::None),
                _ => PyValue::None,
            };
            single.set(&name, node);
            let text = sorted_keys(&PyValue::Dict(single))
                .dumps_unicode()
                .map_err(dump_failed)?;
            print_to(world.host, &text);
        }
    }
    for note in result.notes() {
        let mut text = PyStr::new("[node-role] note: ");
        text.push_py(note);
        eprint(world.host, &text);
    }
    if !result.violations().is_empty() {
        if !json_whole && args.field.is_none() {
            for violation in result.violations() {
                violation_line(world.host, violation);
            }
        }
        return Ok(5);
    }
    Ok(0)
}

/// The program over `argv`: the status it ends with.
pub fn main(world: &mut RoleWorld<'_>) -> Result<i32, Refusal> {
    let args: Vec<String> = world.argv[1..]
        .iter()
        .map(|a| a.to_utf8().unwrap_or_default())
        .collect();
    match args.as_slice() {
        [] => {
            say(world.host, HELP);
            Ok(2)
        }
        [one] if one == "-h" || one == "--help" => {
            say(world.host, HELP);
            Ok(0)
        }
        [one] if one == "--self-test" => {
            let run = role_contract::self_test::run();
            for line in &run.lines {
                print_to(world.host, &PyStr::new(line));
            }
            Ok(if run.failures == 0 { 0 } else { 1 })
        }
        [first, ..] if first == "evaluate" => match parse_evaluate(world)? {
            Ok(evaluate) => cmd_evaluate(world, evaluate),
            Err(status) => Ok(status),
        },
        _ => Err(refusal(
            "argparse_top_level_form",
            "only no arguments, -h/--help, --self-test and `evaluate ...` are decided by this port; the argparse text for other top-level forms was not captured",
        )),
    }
}
