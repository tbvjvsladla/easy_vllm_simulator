//! Checks of the `node_role_contract` owner (R41): a representative subset of the saved original cases replayed through the real programs.
//!
//! * `selected_saved_cli_cases_through_the_normal_program`: each case's input files are copied from the saved case's `business/` (the fixture bytes the
//!   original read; its `node/` copy of the Python owner is NOT copied) into an owned scratch directory, the executable is copied to
//!   `business/node/.claude/skills/terraforming_node/scripts/node_role_contract.py` and run as the legacy script with the saved argv (paths rebased),
//!   environment and a null stdin; exit status and both raw streams are compared with the saved original (the saved business root is rebased).
//! * `selected_saved_api_cases_through_the_service`: the saved typed arguments (`input_before`) and the saved 14 globals are sent to the JSON-lines
//!   program; the answer's kind, value, exception graph (class, message, args, attributes, constructor, suppression; `module` of `ContractViolation`
//!   is the owner's import name and is not compared, its `module_binding` is asserted instead), `*_after` members and `input_ref` (non-null exactly
//!   where the saved record has a return alias) are compared. The logical frames ARE compared: the native `frames` equal the saved frames minus
//!   the collector's transport call site (the parent composes that one), entry by entry (file resolved by its root, line, function, count), and
//!   `frame_context` is asserted; an unmapped YAML chain must be the named boundary `exception_frames_unmapped`.
//!
//! The saved reference is read through `R41_ROLE_REFERENCE`. No Python, original or network is used; production code holds no case id or saved value.

use agent_runtime_support::pyjson::loads;
use agent_runtime_support::pyvalue::PyValue;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn reference() -> PathBuf {
    PathBuf::from(
        std::env::var("R41_ROLE_REFERENCE")
            .expect("R41_ROLE_REFERENCE must name the saved capture directory"),
    )
}

fn load(path: &Path) -> PyValue {
    loads(&std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())))
        .expect("saved JSON")
}

fn member<'a>(value: &'a PyValue, key: &str) -> Option<&'a PyValue> {
    match value {
        PyValue::Dict(members) => members.get(key),
        _ => None,
    }
}

fn text_of(value: &PyValue) -> String {
    match value {
        PyValue::Str(text) => text.to_utf8().expect("text"),
        other => panic!("a string was expected: {other:?}"),
    }
}

fn scratch(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "role-checks-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if from.ends_with("business") && entry.file_name() == "node" {
            continue; // the saved copy of the Python owner is not part of the input
        }
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
            let mode = std::fs::metadata(entry.path()).unwrap().permissions();
            std::fs::set_permissions(&target, mode).unwrap();
        }
    }
}

fn saved_business(id: &str) -> String {
    let execution = load(&reference().join(id).join("execution.json"));
    text_of(member(&execution, "cwd").unwrap())
}

/// The owned business root of a case with the fixtures copied and the executable in place; returns (business, carrier).
fn build_business(root: &Path, id: &str, executable: &str) -> (PathBuf, PathBuf) {
    let business = root.join("business");
    copy_tree(&reference().join(id).join("business"), &business);
    let scripts = business.join("node/.claude/skills/terraforming_node/scripts");
    std::fs::create_dir_all(&scripts).unwrap();
    let carrier = scripts.join("node_role_contract.py");
    std::fs::copy(executable, &carrier).unwrap();
    (business, carrier)
}

fn rebase(saved: &[u8], from: &str, to: &str) -> Vec<u8> {
    String::from_utf8_lossy(saved)
        .replace(from, to)
        .into_bytes()
}

#[test]
fn selected_saved_cli_cases_through_the_normal_program() {
    let subset = [
        "cli-001", "cli-002", "cli-003", "cli-004", "cli-005", "cli-006", "cli-007", "cli-008",
        "cli-010", "cli-011", "cli-012", "cli-014", "cli-015", "cli-017", "cli-018", "cli-019",
        "cli-020", "cli-021",
    ];
    for id in subset {
        let case_json = load(&reference().join(id).join("execution.json"));
        let root = scratch(id);
        let (business, carrier) =
            build_business(&root, id, env!("CARGO_BIN_EXE_node_role_contract"));
        let saved_root = saved_business(id);
        let mine = business.to_string_lossy().into_owned();
        let argv: Vec<String> = match member(&case_json, "argv") {
            Some(PyValue::List(items)) => items[7..]
                .iter()
                .map(|a| text_of(a).replace(&saved_root, &mine))
                .collect(),
            other => panic!("{other:?}"),
        };
        let mut command = Command::new(&carrier);
        command.args(&argv).current_dir(&business).env_clear();
        if let Some(PyValue::Dict(env)) = member(&case_json, "env") {
            for (k, v) in env.iter() {
                command.env(k.to_utf8().unwrap(), text_of(v));
            }
        }
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // a freshly copied executable can be 'busy' while another test thread forks; the program itself is not started yet, so retry the spawn
        let output = (0..20)
            .find_map(|_| match command.output() {
                Err(error) if error.raw_os_error() == Some(26) => {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    None
                }
                other => Some(other.unwrap()),
            })
            .expect("the program started");
        let expected_out = rebase(
            &std::fs::read(reference().join(id).join("stdout.bin")).unwrap(),
            &saved_root,
            &mine,
        );
        let expected_err = rebase(
            &std::fs::read(reference().join(id).join("stderr.bin")).unwrap(),
            &saved_root,
            &mine,
        );
        let expected_code = match member(&case_json, "exit_code") {
            Some(PyValue::Int(code)) => code.to_string().parse::<i32>().unwrap(),
            other => panic!("{other:?}"),
        };
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&expected_out),
            "{id} stdout"
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&expected_err),
            "{id} stderr"
        );
        assert_eq!(output.status.code(), Some(expected_code), "{id} status");
        std::fs::remove_dir_all(root).unwrap();
    }
}

fn dump(value: &PyValue) -> String {
    upstream_runtime::api::render(value).expect("renderable")
}

fn run_service(requests: &[String]) -> Vec<PyValue> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_upstream_role_api"))
        .arg("evaluate-jsonl")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all((requests.join("\n") + "\n").as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.stderr.is_empty(),
        "protocol stderr {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.status.code(), Some(0));
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|l| loads(l).expect("one JSON record per line"))
        .collect()
}

#[test]
fn selected_saved_api_cases_through_the_service() {
    let subset = [
        "api-001", "api-002", "api-003", "api-004", "api-006", "api-007", "api-008", "api-009",
        "api-010", "api-011", "api-012", "api-013", "api-014", "api-015", "api-016", "api-018",
        "api-020", "api-021", "api-022", "api-023", "api-024", "api-025", "api-026", "api-028",
        "api-029", "api-030", "api-031", "api-032", "api-033", "api-034", "api-035", "api-036",
        "api-037", "api-038", "api-039", "api-042", "api-043", "api-044", "api-045", "api-046",
        "api-047", "api-048", "api-049", "api-050", "api-051",
    ];
    for id in subset {
        let saved_text =
            std::fs::read_to_string(reference().join(id).join("api-record.json")).unwrap();
        let root = scratch(id);
        let (business, _) = build_business(&root, id, env!("CARGO_BIN_EXE_node_role_contract"));
        let (saved_root, mine) = (saved_business(id), business.to_string_lossy().into_owned());
        let saved = loads(&saved_text.replace(&saved_root, &mine)).expect("saved record");
        let before = member(&saved, "input_before").unwrap();
        let entry = {
            let case = load(&reference().join(id).join("case.json"));
            text_of(
                member(&case, "entry")
                    .or_else(|| member(&case, "function"))
                    .unwrap_or_else(|| panic!("no entry in case.json of {id}")),
            )
        };
        let request = format!(
            r#"{{"schema":"node_role_contract-api/1","id":"{id}","operation":"node_role_contract.{entry}","args_dto":{},"kwargs_dto":{},"globals":{}}}"#,
            dump(member(before, "args").unwrap()),
            dump(member(before, "kwargs").unwrap()),
            dump(member(&saved, "globals_after").unwrap())
        );
        let record = run_service(&[request]).remove(0);
        let kind = text_of(member(&saved, "kind").unwrap());
        assert_eq!(
            text_of(member(&record, "kind").unwrap()),
            kind,
            "{id}: kind {record:?}"
        );
        for key in ["args_after", "kwargs_after", "globals_after"] {
            assert_eq!(member(&record, key), member(&saved, key), "{id}: {key}");
        }
        if kind == "value" {
            assert_eq!(
                member(&record, "value"),
                member(&saved, "value"),
                "{id}: value"
            );
            let aliased =
                matches!(member(&saved, "return_aliases"), Some(PyValue::List(a)) if !a.is_empty());
            assert_eq!(
                !matches!(member(&record, "input_ref"), Some(PyValue::None)),
                aliased,
                "{id}: input_ref"
            );
            if aliased {
                let PyValue::List(aliases) = member(&saved, "return_aliases").unwrap() else {
                    unreachable!()
                };
                assert_eq!(
                    member(&record, "input_ref"),
                    Some(&aliases[0]),
                    "{id}: alias path"
                );
            }
        } else {
            let (mine_e, theirs) = (
                member(&record, "exception").unwrap(),
                member(&saved, "exception").unwrap(),
            );
            let mut keys = vec![
                "type",
                "message",
                "args",
                "suppress_context",
                "relation",
                "attributes",
            ];
            if text_of(member(theirs, "type").unwrap()) != "ContractViolation" {
                keys.push("constructor");
            }
            if text_of(member(theirs, "type").unwrap()) != "ContractViolation" {
                keys.push("module");
            }
            for key in keys {
                assert_eq!(
                    member(mine_e, key),
                    member(theirs, key),
                    "{id}: exception.{key}"
                );
            }
            // the logical frames: the native graph is the original's traceback WITHOUT its first entry, the transport call site of the collector
            // (the parent composes that one); owner entries resolve to the owner file the original imported, `yaml_package` entries to the
            // reference site-packages, `frozen` entries are literal
            let saved_frames = match member(theirs, "frames") {
                Some(PyValue::List(items)) => items,
                other => panic!("{id}: saved frames {other:?}"),
            };
            assert_eq!(
                text_of(member(&saved_frames[0], "function").unwrap()),
                "invoke_observed",
                "{id}: the transport call site"
            );
            let native_frames = match member(mine_e, "frames") {
                Some(PyValue::List(items)) => items,
                other => panic!("{id}: native frames {other:?}"),
            };
            assert_eq!(
                native_frames.len() + 1,
                saved_frames.len(),
                "{id}: frame count"
            );
            for (native, saved_frame) in native_frames.iter().zip(&saved_frames[1..]) {
                let (file, root) = (
                    text_of(member(native, "file").unwrap()),
                    text_of(member(native, "root").unwrap()),
                );
                let saved_file = text_of(member(saved_frame, "file").unwrap());
                match root.as_str() {
                    "owner" => assert!(
                        saved_file.ends_with(&format!("/{file}")),
                        "{id}: {saved_file} vs {file}"
                    ),
                    "yaml_package" => assert_eq!(
                        saved_file,
                        format!("/usr/lib/python3/dist-packages/{file}"),
                        "{id}"
                    ),
                    "frozen" => assert_eq!(saved_file, file, "{id}"),
                    other => panic!("{id}: unknown root {other}"),
                }
                for key in ["line", "function"] {
                    assert_eq!(
                        member(native, key),
                        member(saved_frame, key),
                        "{id}: frame {key} in {file}"
                    );
                }
            }
            let context =
                member(&record, "frame_context").unwrap_or_else(|| panic!("{id}: frame_context"));
            assert_eq!(
                member(context, "schema"),
                Some(&PyValue::str("source-compat-frames/1"))
            );
            if text_of(member(theirs, "type").unwrap()) == "ContractViolation" {
                assert_eq!(
                    member(member(mine_e, "module_binding").unwrap(), "logical_module"),
                    Some(&PyValue::str("node_role_contract")),
                    "{id}"
                );
            }
            if text_of(member(theirs, "type").unwrap()) == "ContractViolation" {
                // the constructor repeats the owner's import name (module); kind and fields are the contract
                let (a, b) = (
                    member(mine_e, "constructor").unwrap(),
                    member(theirs, "constructor").unwrap(),
                );
                for key in ["kind", "fields"] {
                    assert_eq!(member(a, key), member(b, key), "{id}: constructor.{key}");
                }
            }
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn the_service_binds_like_python_refuses_unknown_requests_and_keeps_one_record_per_line() {
    let globals = dump(&upstream_runtime::role_api::canonical_globals());
    let request = |id: &str, operation: &str, args: &str, kwargs: &str| {
        format!(
            r#"{{"schema":"node_role_contract-api/1","id":"{id}","operation":"node_role_contract.{operation}","args_dto":{args},"kwargs_dto":{kwargs},"globals":{globals}}}"#
        )
    };
    let records = run_service(&[
        request(
            "a",
            "evaluate_manifest",
            r#"["list",[["dict",[[["str","topology"],["str","multi"]],[["str","nodes"],["list",[["dict",[[["str","role"],["str","sub"]]]],["dict",[[["str","role"],["str","main"]]]]]]]]]]]"#,
            r#"["dict",[]]"#,
        ),
        request(
            "b",
            "resolve_rank",
            r#"["list",[["str","multi"]]]"#,
            r#"["dict",[]]"#,
        ),
        request(
            "c",
            "assert_topology",
            r#"["list",[]]"#,
            r#"["dict",[[["str","topology"],["str","single"]]]]"#,
        ),
        r#"{"schema":"node_role_contract-api/1","id":"d"}"#.to_string(),
    ]);
    assert_eq!(records.len(), 4);
    assert_eq!(member(&records[0], "kind"), Some(&PyValue::str("value")));
    assert!(
        dump(member(&records[0], "value").unwrap()).contains("role:main"),
        "the note of a main that is not first"
    );
    // a missing positional argument is the interpreter's TypeError, an exception record with owner_call_attempted true
    assert_eq!(
        member(&records[1], "kind"),
        Some(&PyValue::str("exception"))
    );
    assert_eq!(
        member(&records[1], "owner_call_attempted"),
        Some(&PyValue::Bool(true))
    );
    // the keyword argument is named by its position among the keywords
    assert_eq!(member(&records[2], "kind"), Some(&PyValue::str("value")));
    assert_eq!(
        member(&records[3], "kind"),
        Some(&PyValue::str("request_error"))
    );
}

#[test]
fn a_yaml_failure_without_a_mapped_traceback_is_a_named_boundary_not_empty_frames() {
    let dir = scratch("unmapped");
    let path = dir.join("scanner.yaml");
    std::fs::write(&path, "a: @\n").unwrap();
    let globals = dump(&upstream_runtime::role_api::canonical_globals());
    let request = format!(
        r#"{{"schema":"node_role_contract-api/1","id":"u","operation":"node_role_contract.load_yaml","args_dto":["list",[["str","{}"]]],"kwargs_dto":["dict",[]],"globals":{globals}}}"#,
        path.to_string_lossy()
    );
    let record = run_service(&[request]).remove(0);
    assert_eq!(member(&record, "kind"), Some(&PyValue::str("out_of_scope")));
    assert_eq!(
        member(&record, "boundary"),
        Some(&PyValue::str("exception_frames_unmapped"))
    );
    std::fs::remove_dir_all(dir).unwrap();
}

/// R48 Q added `MarkedDetail::shared_marks` (are `context_mark` and `problem_mark` one Python object?). The R33 error DTO profile encodes marks
/// as VALUES, so the new fact must not change any existing graph: two marked errors that differ ONLY in `shared_marks` give the same outcome,
/// and the real loader's marked errors keep their defined graph (class, five args/attributes, both marks) as before.
#[test]
fn the_new_shared_marks_fact_does_not_change_the_defined_yaml_error_graph() {
    use agent_runtime_support::pystr::PyStr;
    use agent_runtime_support::pyyaml::{Mark6, MarkedDetail, YamlDetail, YamlError};
    use role_contract::load::LoadFailure;
    use upstream_runtime::role_errors::{load_failure, Raised};
    let mark = |index: usize| Mark6 {
        name: PyStr::new("m.yaml"),
        index,
        line: 0,
        column: index,
        buffer: None,
        pointer: None,
    };
    let make = |shared: bool| {
        LoadFailure::Yaml(Box::new(YamlError {
            class: "ParserError",
            message: PyStr::new("while parsing\nproblem\n"),
            detail: YamlDetail::Marked(Box::new(MarkedDetail {
                context: Some(PyStr::new("while parsing")),
                context_mark: Some(mark(1)),
                problem: Some(PyStr::new("problem")),
                problem_mark: Some(mark(1)),
                note: None,
                shared_marks: shared,
                frames: None,
            })),
        }))
    };
    let show = |raised: Raised| match raised {
        Raised::Exception {
            error,
            from_none,
            frames,
        } => format!(
            "exception {} {} {:?} {:?} {:?} {}",
            error.class,
            error.module,
            error.args.len(),
            error.attributes.len(),
            frames.is_some(),
            from_none
        ),
        Raised::OutOfScope { boundary, detail } => format!("out_of_scope {boundary} {detail}"),
    };
    assert_eq!(
        show(load_failure(make(true))),
        show(load_failure(make(false)))
    );
    // the real loader, through the service: a marked error with equal marks is still the same boundary/graph it was
    let dir = scratch("marked");
    let path = dir.join("flow.yaml");
    std::fs::write(&path, "a: [1, 2\n").unwrap();
    let globals = dump(&upstream_runtime::role_api::canonical_globals());
    let request = format!(
        r#"{{"schema":"node_role_contract-api/1","id":"u","operation":"node_role_contract.load_yaml","args_dto":["list",[["str","{}"]]],"kwargs_dto":["dict",[]],"globals":{globals}}}"#,
        path.to_string_lossy()
    );
    let record = run_service(&[request]).remove(0);
    let kind = member(&record, "kind").cloned();
    assert!(
        matches!(&kind, Some(PyValue::Str(k)) if k.equals("exception") || k.equals("out_of_scope")),
        "the loader's marked error is a defined exception graph or a named boundary, never a panic or an empty record: {kind:?}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}
