//! Checks of the `regen_requirements` owner (R39): a representative subset of the saved original cases replayed through the real programs, and the
//! contract points that the saved cases do not reach.
//!
//! * `selected_saved_cli_cases_through_the_normal_program`: each case's input tree is REBUILT from its saved `before.json` snapshot (directories,
//!   files with their saved bytes, symbolic links) inside an owned scratch directory, the executable is run as the legacy script with the saved argv and
//!   environment (paths rebased to the scratch tree), and the exit status, both raw streams, the declared output file and the temporary-directory
//!   cleanup are compared with the saved original. The saved script path is rebased to the copied executable's. The cohort is a subset, one case per
//!   kind of outcome (default and explicit output, installed, overwrite, symlink, non-archive, missing file, directory/missing parent output, closed
//!   stdout, ASCII after write, self-test, empty URL, identity failure, plain HTTP); the full cohort belongs to the independent normal run.
//! * `selected_saved_api_cases_through_the_service`: pure API cases (`parse_requires`) through the typed service against the saved records (value DTO,
//!   or the exception's public members; interpreter frames are auxiliary and not compared).
//! * the JSON-lines program as a process: one request, one record, nothing else on either protocol stream.
//!
//! The saved reference is read through `R39_REQUIREMENTS_REFERENCE`. Every socket is on `127.0.0.1` and owned by the test; no Python, original,
//! external endpoint or package index is used. Production code holds no case id, saved value or fixture lookup.

use agent_runtime_support::pyjson::loads;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use upstream_runtime::fetch_https::FetchEnvironment;
use upstream_runtime::http_direct::UreqDialer;
use upstream_runtime::regen_api::{answer_with, ApiEnvironment, SCHEMA};

fn reference() -> PathBuf {
    PathBuf::from(
        std::env::var("R39_REQUIREMENTS_REFERENCE")
            .expect("R39_REQUIREMENTS_REFERENCE must name the saved capture directory"),
    )
}

fn load(path: PathBuf) -> PyValue {
    loads(
        &std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display())),
    )
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

fn items(value: Option<&PyValue>) -> Vec<&PyValue> {
    match value {
        Some(PyValue::List(items)) => items.iter().collect(),
        _ => Vec::new(),
    }
}

fn hex_decode(hex: &str) -> Vec<u8> {
    (0..hex.len() / 2)
        .map(|at| u8::from_str_radix(&hex[2 * at..2 * at + 2], 16).expect("hex"))
        .collect()
}

fn replace_bytes(haystack: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(haystack.len());
    let mut at = 0;
    while at < haystack.len() {
        if haystack[at..].starts_with(from) {
            out.extend_from_slice(to);
            at += from.len();
        } else {
            out.push(haystack[at]);
            at += 1;
        }
    }
    out
}

fn scratch(tag: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "regen-checks-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

/// Rebuilds the saved pre-run tree below `business`: directories, files with their saved bytes, symbolic links. The carrier and the private temporary
/// directory are made by the replay itself.
fn rebuild_before(snapshot: &PyValue, business: &Path) {
    for entry in items(Some(snapshot)) {
        let relative = text_of(member(entry, "path").expect("path"));
        if relative == "." || relative.starts_with("node") || relative.starts_with("private-tmp") {
            continue;
        }
        let target = business.join(&relative);
        match text_of(member(entry, "kind").expect("kind")).as_str() {
            "dir" => std::fs::create_dir_all(&target).unwrap(),
            "file" => {
                std::fs::create_dir_all(target.parent().unwrap()).unwrap();
                std::fs::write(
                    &target,
                    hex_decode(&text_of(member(entry, "hex").expect("hex"))),
                )
                .unwrap();
            }
            "symlink" => {
                symlink(text_of(member(entry, "target").expect("target")), &target).unwrap()
            }
            other => panic!("unknown snapshot kind {other}"),
        }
    }
}

/// A plain HTTP tape answering one connection with `response`.
fn serve_once(response: Vec<u8>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut head = Vec::new();
            let mut byte = [0u8; 1];
            while !head.ends_with(b"\r\n\r\n")
                && stream.read(&mut byte).map(|n| n == 1).unwrap_or(false)
            {
                head.push(byte[0]);
            }
            let _ = stream.write_all(&response);
        }
    });
    port
}

/// One saved CLI case replayed against the copied executable; the differences found.
fn replay_cli_case(case: &str, scratch_root: &Path) -> Vec<String> {
    let saved = reference().join(case);
    let case_json = load(saved.join("case.json"));
    let execution = load(saved.join("execution.json"));
    let saved_business = format!("{}/business", saved.display());
    let business = scratch_root.join(case).join("business");
    let private_tmp = business.join("private-tmp");
    std::fs::create_dir_all(&private_tmp).unwrap();
    rebuild_before(&load(saved.join("before.json")), &business);
    let script =
        business.join("node/.claude/skills/upstream-version-watch/scripts/regen_requirements.py");
    std::fs::create_dir_all(script.parent().unwrap()).unwrap();
    std::fs::copy(env!("CARGO_BIN_EXE_regen_requirements"), &script).unwrap();

    let tape = items(member(
        &load(saved.join("peer-description.json")),
        "responses",
    ))
    .first()
    .and_then(|response| member(response, "raw_hex"))
    .map(|hex| hex_decode(&text_of(hex)));
    let peer_port = match (&tape, case) {
        (Some(response), "cli-018") => Some(serve_once(response.clone())),
        _ => None,
    };
    let rebase = |text: &str| -> String {
        let mut shown = text
            .replace("@ROOT@", &business.to_string_lossy())
            .replace(&saved_business, &business.to_string_lossy());
        if let Some(port) = peer_port {
            shown = shown.replace("@HTTP_PORT@", &port.to_string());
        }
        shown
    };
    let mut command = Command::new(&script);
    command
        .env_clear()
        .current_dir(&business)
        .stdin(Stdio::null());
    for argument in items(member(&case_json, "argv")) {
        command.arg(rebase(&text_of(argument)));
    }
    let Some(PyValue::Dict(environment)) = member(&execution, "env") else {
        panic!("saved environment")
    };
    for (name, value) in environment.iter() {
        let name = name.to_utf8().unwrap();
        let value = if name == "TMPDIR" {
            private_tmp.to_string_lossy().into_owned()
        } else {
            rebase(&text_of(value))
        };
        command.env(name, value);
    }
    let closed_stdout = member(&case_json, "stdout").is_some();
    if closed_stdout {
        let (reader, writer) = std::io::pipe().unwrap();
        drop(reader);
        command.stdout(writer);
    }
    // a freshly copied executable can be 'busy' (ETXTBSY) while another test thread forks; retry only that error
    let output = (0..50)
        .find_map(|_| match command.output() {
            Err(error) if error.raw_os_error() == Some(26) => {
                std::thread::sleep(std::time::Duration::from_millis(20));
                None
            }
            other => Some(other.expect("the copied executable ran")),
        })
        .expect("the copied executable started");
    let mut differences = Vec::new();
    let saved_peer_port =
        member(&load(saved.join("peer-observed.json")), "port").map(|port| match port {
            PyValue::Int(port) => port.to_string(),
            other => panic!("saved port {other:?}"),
        });
    let rebased_saved = |bytes: &[u8]| {
        let bytes = replace_bytes(
            bytes,
            saved_business.as_bytes(),
            business.to_string_lossy().as_bytes(),
        );
        match (&saved_peer_port, peer_port) {
            (Some(saved_port), Some(port)) => replace_bytes(
                &bytes,
                format!("127.0.0.1:{saved_port}").as_bytes(),
                format!("127.0.0.1:{port}").as_bytes(),
            ),
            _ => bytes,
        }
    };
    let saved_stdout = rebased_saved(&std::fs::read(saved.join("stdout.bin")).unwrap());
    let saved_stderr = rebased_saved(&std::fs::read(saved.join("stderr.bin")).unwrap());
    if !closed_stdout && output.stdout != saved_stdout {
        differences.push(format!(
            "{case}: stdout differs\nactual:\n{}\nsaved:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&saved_stdout)
        ));
    }
    if output.stderr != saved_stderr {
        differences.push(format!(
            "{case}: stderr differs\nactual:\n{}\nsaved:\n{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&saved_stderr)
        ));
    }
    let saved_exit = match member(&execution, "exit_code") {
        Some(PyValue::Int(code)) => i64::try_from(code.clone()).unwrap(),
        other => panic!("saved exit status {other:?}"),
    };
    if i64::from(output.status.code().unwrap_or(-1)) != saved_exit {
        differences.push(format!(
            "{case}: exit status {:?}, saved {saved_exit}",
            output.status.code()
        ));
    }
    // the declared output file (a link is followed to its target, as the original did)
    if let Some(declared) = member(&case_json, "declared_output") {
        let relative = text_of(declared).replacen(&format!("{saved_business}/"), "", 1);
        let after = load(saved.join("after.json"));
        let saved_entry = items(Some(&after))
            .into_iter()
            .find(|entry| member(entry, "path").is_some_and(|path| text_of(path) == relative));
        if let Some(entry) = saved_entry
            .filter(|entry| member(entry, "kind").is_some_and(|kind| text_of(kind) == "file"))
        {
            let wanted = rebased_saved(&hex_decode(&text_of(member(entry, "hex").unwrap())));
            let actual = std::fs::read(business.join(&relative)).unwrap_or_default();
            if actual != wanted {
                differences.push(format!(
                    "{case}: output file {relative} differs\nactual:\n{}\nsaved:\n{}",
                    String::from_utf8_lossy(&actual),
                    String::from_utf8_lossy(&wanted)
                ));
            }
        }
    }
    let leftover = std::fs::read_dir(&private_tmp)
        .map(|entries| entries.count())
        .unwrap_or(0);
    if leftover != 0 {
        differences.push(format!(
            "{case}: {leftover} temporary entries were left in the owned TMPDIR"
        ));
    }
    differences
}

#[test]
fn selected_saved_cli_cases_through_the_normal_program() {
    let root = scratch("cli");
    let mut differences = Vec::new();
    // default output, installed, overwrite, symlink target, non-archive, missing file, output is a directory, closed stdout, ASCII after write,
    // self-test, empty URL, identity failure before the output is opened, plain HTTP
    for case in [
        "cli-005", "cli-006", "cli-007", "cli-009", "cli-010", "cli-011", "cli-012", "cli-013",
        "cli-014", "cli-016", "cli-017", "cli-020", "cli-021", "cli-022", "cli-023", "cli-018",
    ] {
        differences.extend(replay_cli_case(case, &root));
    }
    let _ = std::fs::remove_dir_all(&root);
    assert!(differences.is_empty(), "{}", differences.join("\n---\n"));
}

fn api_environment<'a>(fetch: &'a FetchEnvironment, dialer: &'a UreqDialer) -> ApiEnvironment<'a> {
    ApiEnvironment::new(fetch, dialer, None, None)
}

#[test]
fn selected_saved_api_cases_through_the_service() {
    let (fetch, dialer) = (FetchEnvironment::default(), UreqDialer::unbounded());
    let environment = api_environment(&fetch, &dialer);
    let mut differences = Vec::new();
    for case in [
        "parse-001",
        "parse-005",
        "parse-010",
        "parse-015",
        "parse-020",
        "parse-021",
        "parse-022",
        "parse-025",
        "parse-026",
    ] {
        let saved = reference().join(case);
        let case_json = load(saved.join("case.json"));
        let original = load(saved.join("original.json"));
        let globals = member(&load(saved.join("api-before-call.json")), "globals")
            .cloned()
            .expect("saved globals");
        let mut request = PyDict::new();
        request.set("schema", PyValue::str(SCHEMA));
        request.set("id", member(&case_json, "id").cloned().unwrap());
        request.set(
            "operation",
            PyValue::str(&format!(
                "regen_requirements.{}",
                text_of(member(&case_json, "function").unwrap())
            )),
        );
        request.set("args_dto", member(&case_json, "args_dto").cloned().unwrap());
        request.set(
            "kwargs_dto",
            member(&case_json, "kwargs_dto").cloned().unwrap(),
        );
        request.set("globals", globals);
        let line = upstream_runtime::api::render(&PyValue::Dict(request)).unwrap();
        let PyValue::Dict(record) = answer_with(&PyStr::new(&line), &environment) else {
            panic!("record")
        };
        let record = PyValue::Dict(record);
        let kind = text_of(member(&original, "kind").unwrap());
        if text_of(member(&record, "kind").unwrap()) != kind {
            differences.push(format!(
                "{case}: kind {:?}, saved {kind}",
                member(&record, "kind")
            ));
            continue;
        }
        if kind == "value" {
            if member(&record, "value") != member(&original, "value") {
                differences.push(format!(
                    "{case}: value differs: {:?} versus {:?}",
                    member(&record, "value"),
                    member(&original, "value")
                ));
            }
        } else {
            let (mine, saved_exception) = (
                member(&record, "exception").unwrap(),
                member(&original, "exception").unwrap(),
            );
            for key in [
                "type",
                "module",
                "message",
                "args",
                "suppress_context",
                "relation",
                "attributes",
            ] {
                if member(mine, key) != member(saved_exception, key) {
                    differences.push(format!(
                        "{case}: exception member {key} differs: {:?} versus {:?}",
                        member(mine, key),
                        member(saved_exception, key)
                    ));
                }
            }
        }
        for key in ["args_after", "kwargs_after"] {
            if member(&record, key) != member(&original, key) {
                differences.push(format!("{case}: {key} differs"));
            }
        }
        if member(&record, "globals_after") != member(&original, "globals_after") {
            differences.push(format!("{case}: globals_after differs"));
        }
    }
    assert!(differences.is_empty(), "{}", differences.join("\n---\n"));
}

#[test]
fn the_json_lines_program_answers_one_request_with_one_record_and_nothing_else() {
    let globals = upstream_runtime::api::render(
        &member(
            &load(reference().join("parse-001/api-before-call.json")),
            "globals",
        )
        .cloned()
        .unwrap(),
    )
    .unwrap();
    let request = format!(
        r#"{{"schema":"{SCHEMA}","id":"one","operation":"regen_requirements.parse_requires","args_dto":["list",[["list",[["str","fastapi"]]],["none"]]],"kwargs_dto":["dict",[]],"globals":{globals}}}"#
    );
    let mut child = Command::new(env!("CARGO_BIN_EXE_upstream_requirements_api"))
        .arg("evaluate-jsonl")
        .env_clear()
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
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.lines().count(), 1);
    let record = loads(stdout.trim_end()).unwrap();
    assert_eq!(
        member(&record, "kind").map(text_of).as_deref(),
        Some("value")
    );
    assert_eq!(
        member(&record, "schema").map(text_of).as_deref(),
        Some(SCHEMA)
    );
    assert!(
        member(&record, "provenance").is_some_and(|provenance| member(provenance, "source")
            .map(text_of)
            .as_deref()
            == Some("native-declared"))
    );
}

/// A saved wheel or installed API case that needs no peer, replayed through the typed service on its rebuilt input tree; the differences found.
fn replay_api_case_without_peer(case: &str, scratch_root: &Path) -> Vec<String> {
    let saved = reference().join(case);
    let case_json = load(saved.join("case.json"));
    let original = load(saved.join("original.json"));
    let execution = load(saved.join("execution.json"));
    let saved_business = format!("{}/business", saved.display());
    let business = scratch_root.join(case).join("business");
    let private_tmp = business.join("private-tmp");
    std::fs::create_dir_all(&private_tmp).unwrap();
    rebuild_before(&load(saved.join("before.json")), &business);
    let rebase = |text: String| {
        text.replace("@ROOT@", &business.to_string_lossy())
            .replace(&saved_business, &business.to_string_lossy())
    };
    let pythonpath = member(&execution, "env")
        .and_then(|environment| member(environment, "PYTHONPATH"))
        .map(|value| std::ffi::OsString::from(rebase(text_of(value))));
    let globals = member(&load(saved.join("api-before-call.json")), "globals")
        .cloned()
        .expect("saved globals");
    let mut request = PyDict::new();
    request.set("schema", PyValue::str(SCHEMA));
    request.set("id", member(&case_json, "id").cloned().unwrap());
    request.set(
        "operation",
        PyValue::str(&format!(
            "regen_requirements.{}",
            text_of(member(&case_json, "function").unwrap())
        )),
    );
    request.set("args_dto", member(&case_json, "args_dto").cloned().unwrap());
    request.set(
        "kwargs_dto",
        member(&case_json, "kwargs_dto").cloned().unwrap(),
    );
    request.set("globals", globals);
    let line = rebase(upstream_runtime::api::render(&PyValue::Dict(request)).unwrap());
    let (fetch, dialer) = (FetchEnvironment::default(), UreqDialer::unbounded());
    let environment = ApiEnvironment::new(
        &fetch,
        &dialer,
        pythonpath,
        Some(upstream_runtime::tempdir::TempBase::fixed(
            private_tmp.clone(),
        )),
    );
    let record = answer_with(&PyStr::new(&line), &environment);
    let shown = |value: Option<&PyValue>| {
        rebase(upstream_runtime::api::render(value.unwrap_or(&PyValue::None)).unwrap())
    };
    let saved_shown = |value: Option<&PyValue>| {
        upstream_runtime::api::render(value.unwrap_or(&PyValue::None))
            .unwrap()
            .replace(&saved_business, &business.to_string_lossy())
    };
    let mut differences = Vec::new();
    let kind = text_of(member(&original, "kind").unwrap());
    if member(&record, "kind").map(text_of).as_deref() != Some(kind.as_str()) {
        differences.push(format!(
            "{case}: kind {:?}, saved {kind}: {:?}",
            member(&record, "kind"),
            member(&record, "boundary")
        ));
        return differences;
    }
    if kind == "value" {
        if shown(member(&record, "value")) != saved_shown(member(&original, "value")) {
            differences.push(format!("{case}: value differs"));
        }
    } else {
        let (mine, wanted) = (
            member(&record, "exception").unwrap(),
            member(&original, "exception").unwrap(),
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
            "http_headers",
        ] {
            if shown(member(mine, key)) != saved_shown(member(wanted, key)) {
                differences.push(format!(
                    "{case}: exception member {key} differs:\n{}\nversus\n{}",
                    shown(member(mine, key)),
                    saved_shown(member(wanted, key))
                ));
            }
        }
    }
    if shown(member(&record, "args_after")) != saved_shown(member(&original, "args_after")) {
        differences.push(format!("{case}: args_after differs"));
    }
    if std::fs::read_dir(&private_tmp)
        .map(|entries| entries.count())
        .unwrap_or(0)
        != 0
    {
        differences.push(format!("{case}: temporary entries were left behind"));
    }
    differences
}

#[test]
fn selected_saved_wheel_and_installed_api_cases_without_a_peer_through_the_service() {
    let root = scratch("api");
    let mut differences = Vec::new();
    let mut ran = 0;
    for case in [
        "wheel-001",
        "wheel-002",
        "wheel-003",
        "wheel-015",
        "installed-001",
        "installed-002",
        "installed-003",
        "installed-005",
        "installed-008",
    ] {
        let peer_free = items(member(
            &load(reference().join(case).join("peer-description.json")),
            "responses",
        ))
        .is_empty();
        if !peer_free {
            continue;
        }
        ran += 1;
        differences.extend(replay_api_case_without_peer(case, &root));
    }
    let _ = std::fs::remove_dir_all(&root);
    assert!(ran >= 5, "only {ran} of the selected cases need no peer");
    assert!(differences.is_empty(), "{}", differences.join("\n---\n"));
}
