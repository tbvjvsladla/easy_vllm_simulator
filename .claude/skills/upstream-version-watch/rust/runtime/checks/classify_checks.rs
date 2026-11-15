//! Checks of the `classify_failure` owner (R40): a representative subset of the saved original cases replayed through the real programs, plus the
//! contract points the saved cases do not reach (own fixtures).
//!
//! * `selected_saved_cli_cases_through_the_normal_program`: each case's input (argv, standard-input bytes, files) is REBUILT from the saved case
//!   description inside an owned scratch directory; the executable is copied to `business/node/.claude/skills/upstream-version-watch/scripts/classify_failure.py`
//!   next to a copy of the pinned registry and run as the legacy script; exit status and both raw streams are compared with the saved original (the saved
//!   business root is rebased to the scratch one). The subset: help, usage error, empty stdin, a registry match, a missing-class record, a pattern
//!   error, the open-error priority and directory cases, and ASCII output after a file read. The full cohort belongs to the independent normal run.
//! * `selected_saved_api_cases_through_the_service`: a subset of `load_patterns`/`matched_lines` cases through the JSON-lines program against the
//!   saved records (value DTO, or the exception's public members; interpreter frames are auxiliary and not compared; `input_ref` is non-null exactly
//!   where the saved record has a return alias).
//!
//! The saved reference is read from the committed `checks/data/classify_reference` (override: `R40_CLASSIFY_REFERENCE`); the pinned registry from the
//! tracked `failure_patterns.yaml` (override: `R40_CLASSIFY_REGISTRY`). Nothing here runs Python, the
//! original or the network. Production code holds no case id, saved value or lookup.

use agent_runtime_support::pyjson::loads;
use agent_runtime_support::pyvalue::PyValue;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn reference() -> PathBuf {
    match std::env::var("R40_CLASSIFY_REFERENCE") {
        Ok(path) => PathBuf::from(path),
        Err(_) => Path::new(env!("CARGO_MANIFEST_DIR")).join("checks/data/classify_reference"),
    }
}

fn registry() -> PathBuf {
    match std::env::var("R40_CLASSIFY_REGISTRY") {
        Ok(path) => PathBuf::from(path),
        Err(_) => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../failure_patterns.yaml"),
    }
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

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

fn scratch(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "classify-checks-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn accepted_case(id: &str) -> PyValue {
    let all = load(&reference().join("accepted-cases.json"));
    match member(&all, "cases") {
        Some(PyValue::List(cases)) => cases
            .iter()
            .find(|c| text_of(member(c, "id").unwrap()) == id)
            .unwrap_or_else(|| panic!("no case {id}"))
            .clone(),
        _ => panic!("cases"),
    }
}

/// Builds `business/` of a case: the carrier script, the pinned registry beside it, and the case's own files; returns (business, carrier).
// A sibling test thread forking while our copy's write fd is open makes exec fail with ETXTBSY (the fd lives in that
// forked child until its exec). Retry only that error, briefly; any other spawn error still fails at once.
fn spawn_retrying_etxtbsy(command: &mut Command) -> std::process::Child {
    let mut attempt = 0;
    loop {
        match command.spawn() {
            Ok(c) => return c,
            Err(e) if e.raw_os_error() == Some(26) && attempt < 50 => {
                attempt += 1;
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            Err(e) => panic!("spawn: {e}"),
        }
    }
}

fn build_business(root: &Path, case: &PyValue, executable: &str) -> (PathBuf, PathBuf) {
    let business = root.join("business");
    let scripts = business.join("node/.claude/skills/upstream-version-watch/scripts");
    std::fs::create_dir_all(&scripts).unwrap();
    let carrier = scripts.join("classify_failure.py");
    std::fs::copy(executable, &carrier).unwrap();
    std::fs::copy(registry(), scripts.join("../failure_patterns.yaml")).unwrap();
    if let Some(PyValue::List(files)) = member(case, "files") {
        for file in files {
            let rel = text_of(member(file, "path").unwrap());
            if rel.starts_with('@') {
                continue; // the registry placeholder: the pinned copy above
            }
            let path = business.join(&rel);
            let mode = match member(file, "mode") {
                Some(PyValue::Int(m)) => m.to_string().parse::<u32>().unwrap(),
                _ => 0o644,
            };
            match text_of(member(file, "kind").unwrap()).as_str() {
                "dir" => {
                    std::fs::create_dir_all(&path).unwrap();
                    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
                }
                _ => {
                    std::fs::write(&path, hex(&text_of(member(file, "hex").unwrap()))).unwrap();
                    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
                }
            }
        }
    }
    (business, carrier)
}

fn saved_business(id: &str) -> String {
    let execution = load(&reference().join(id).join("execution.json"));
    text_of(member(&execution, "cwd").unwrap())
}

fn rebase(saved: &[u8], from: &str, to: &str) -> Vec<u8> {
    String::from_utf8_lossy(saved)
        .replace(from, to)
        .into_bytes()
}

#[test]
fn selected_saved_cli_cases_through_the_normal_program() {
    let subset = [
        "cli-001", "cli-002", "cli-003", "cli-004", "cli-006", "cli-013", "cli-014", "cli-015",
        "cli-016", "cli-017", "cli-018", "cli-019", "cli-020", "cli-022",
    ];
    for id in subset {
        let case = accepted_case(id);
        let root = scratch(id);
        let (business, carrier) =
            build_business(&root, &case, env!("CARGO_BIN_EXE_classify_failure"));
        let args: Vec<String> = match member(&case, "argv") {
            Some(PyValue::List(items)) => items
                .iter()
                .map(|a| text_of(a).replace("@ROOT@", &business.to_string_lossy()))
                .collect(),
            _ => Vec::new(),
        };
        let stdin_bytes = hex(&text_of(member(&case, "stdin_hex").unwrap()));
        let mut command = Command::new(&carrier);
        command
            .args(&args)
            .current_dir(&business)
            .env_clear()
            .env("LANG", "C.UTF-8")
            .env("LC_ALL", "C.UTF-8")
            .env("COLUMNS", "80")
            .env("PATH", "");
        if let Some(PyValue::Dict(over)) = member(&case, "child_env_override") {
            for (k, v) in over.iter() {
                command.env(k.to_utf8().unwrap(), text_of(v));
            }
        }
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = spawn_retrying_etxtbsy(&mut command);
        child.stdin.take().unwrap().write_all(&stdin_bytes).unwrap();
        let output = child.wait_with_output().unwrap();
        let saved_root = saved_business(id);
        let mine = business.to_string_lossy().into_owned();
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
        let execution = load(&reference().join(id).join("execution.json"));
        let expected_code = match member(&execution, "exit_code") {
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

fn api_request(id: &str, case: &PyValue, business: &Path) -> String {
    let function = text_of(member(case, "function").unwrap());
    let substitute = |value: &PyValue| {
        let text = loads_to_string(value);
        text.replace("@ROOT@", &business.to_string_lossy()).replace(
            "@PATTERNS_FILE@",
            &business
                .join("node/.claude/skills/upstream-version-watch/failure_patterns.yaml")
                .to_string_lossy(),
        )
    };
    let patterns_file = business
        .join("node/.claude/skills/upstream-version-watch/scripts/../failure_patterns.yaml");
    format!(
        r#"{{"schema":"classify_failure-api/1","id":"{id}","operation":"classify_failure.{function}","args_dto":{},"kwargs_dto":{},"globals":{{"PATTERNS_FILE":["str","{}"]}}}}"#,
        substitute(member(case, "args_dto").unwrap()),
        substitute(member(case, "kwargs_dto").unwrap()),
        patterns_file.to_string_lossy()
    )
}

fn loads_to_string(value: &PyValue) -> String {
    upstream_runtime::api::render(value).expect("renderable DTO")
}

#[test]
fn selected_saved_api_cases_through_the_service() {
    let subset = [
        "load-001",
        "load-004",
        "load-007",
        "load-009",
        "load-010",
        "load-013",
        "load-015",
        "load-016",
        "load-017",
        "load-018",
        "load-019",
        "load-020",
        "matched-001",
        "matched-005",
        "matched-007",
        "matched-008",
        "matched-009",
        "matched-010",
        "matched-011",
        "matched-013",
        "matched-014",
        "matched-019",
        "matched-020",
        "matched-021",
        "matched-022",
        "matched-023",
        "matched-024",
    ];
    for id in subset {
        let case = accepted_case(id);
        let root = scratch(id);
        let (business, _) = build_business(&root, &case, env!("CARGO_BIN_EXE_classify_failure"));
        let request = api_request(id, &case, &business);
        let mut child = Command::new(env!("CARGO_BIN_EXE_upstream_classify_api"))
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
            .write_all(format!("{request}\n").as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.stderr.is_empty(),
            "{id}: protocol stderr {:?}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.status.code(), Some(0), "{id}");
        let record = loads(std::str::from_utf8(&output.stdout).unwrap().trim_end())
            .expect("one JSON record");
        let saved_text = std::fs::read_to_string(reference().join(id).join("api-record.json"))
            .unwrap()
            .replace(&saved_business(id), &business.to_string_lossy());
        let saved = loads(&saved_text).expect("saved record");
        let kind = text_of(member(&saved, "kind").unwrap());
        assert_eq!(
            text_of(member(&record, "kind").unwrap()),
            kind,
            "{id}: kind {record:?}"
        );
        assert_eq!(member(&record, "stdout"), Some(&PyValue::str("")), "{id}");
        assert_eq!(
            member(&record, "args_after"),
            member(&saved, "args_after"),
            "{id}: args_after"
        );
        assert_eq!(
            member(&record, "kwargs_after"),
            member(&saved, "kwargs_after"),
            "{id}: kwargs_after"
        );
        if kind == "value" {
            assert_eq!(
                member(&record, "value"),
                member(&saved, "value"),
                "{id}: value"
            );
            let aliased =
                matches!(member(&saved, "return_aliases"), Some(PyValue::List(a)) if !a.is_empty());
            let reference = !matches!(member(&record, "input_ref"), Some(PyValue::None));
            assert_eq!(
                reference, aliased,
                "{id}: input_ref against the saved return alias"
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
            let (mine, theirs) = (
                member(&record, "exception").unwrap(),
                member(&saved, "exception").unwrap(),
            );
            for key in [
                "type",
                "module",
                "message",
                "args",
                "suppress_context",
                "relation",
                "attributes",
                "constructor",
            ] {
                assert_eq!(
                    member(mine, key),
                    member(theirs, key),
                    "{id}: exception.{key}"
                );
            }
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn the_service_refuses_what_it_does_not_decide_by_name_and_keeps_one_record_per_line() {
    let root = scratch("own");
    let request = |args: &str| {
        format!(
            r#"{{"schema":"classify_failure-api/1","id":"o","operation":"classify_failure.matched_lines","args_dto":{args},"kwargs_dto":["dict",[]],"globals":{{"PATTERNS_FILE":["str","/x/scripts/../failure_patterns.yaml"]}}}}"#
        )
    };
    let mut child = Command::new(env!("CARGO_BIN_EXE_upstream_classify_api"))
        .arg("evaluate-jsonl")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let input = format!(
        "{}\n\n{}\n",
        request(r#"["list",[["str","İıſK x"],["str","(?i)[a-z]+"]]]"#),
        request(r#"["list",[["str","a"],["str","\\N{LATIN SMALL LETTER A}"]]]"#)
    );
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    let text = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2, "{text}");
    let first = loads(lines[0]).unwrap();
    assert_eq!(member(&first, "kind"), Some(&PyValue::str("value")));
    assert_eq!(
        member(&first, "value"),
        Some(&PyValue::List(vec![
            PyValue::str("str"),
            PyValue::str("İıſK x")
        ]))
    );
    let second = loads(lines[1]).unwrap();
    assert_eq!(member(&second, "kind"), Some(&PyValue::str("out_of_scope")));
    assert_eq!(
        member(&second, "boundary"),
        Some(&PyValue::str("unicode_character_name"))
    );
    std::fs::remove_dir_all(root).unwrap();
}
