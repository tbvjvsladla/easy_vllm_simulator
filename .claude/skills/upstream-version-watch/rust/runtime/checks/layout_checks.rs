//! Checks of the `check_spec_layout` owner (R34). Nothing here runs Python or reaches a network; every program runs over an owned root with the
//! working directory on that root.
//!
//! * `reference_cases`: the 153 saved API calls and 73 saved command lines, whole (value, exception chain with constructors, ordered file trace,
//!   streams, status, globals, argv, the file tree before and after), through the fixture programs, the production service, the production event
//!   service with a parent that performs the writes, and the production command. The 17 command lines that end in an uncaught exception are
//!   compared by value, chain, status and standard output; their standard-error presentation is OPEN_S1 and is only compared by its last line.
//! * `contract_cases`: constants, requests and their refusals, the source scan.
//! * `new_cases`: behaviour the saved cases do not pin (Unicode, line and end semantics of the key pattern, iteration of every kind of `keys`,
//!   the config line and its quoting, the chunked config reads), expectations derived from the original source and CPython 3.12.
//! * `bench_cases`: the frozen measurement.

use agent_runtime_support::pyjson::loads_text;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use recipe_runtime::model_files::PrivateDirectory;
use recipe_runtime::observe::same_value;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// ── data ─────────────────────────────────────────────────────────────────────────────────────────

fn load(relative: &str) -> PyValue {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("checks")
        .join("data")
        .join("layout_reference")
        .join(relative);
    let bytes = std::fs::read(path).expect("saved data");
    loads_text(&PyStr::from_surrogateescape(&bytes)).expect("saved JSON")
}

fn j(text: &str) -> PyValue {
    loads_text(&PyStr::new(text)).expect("JSON")
}

fn member<'a>(value: &'a PyValue, key: &str) -> &'a PyValue {
    match value {
        PyValue::Dict(map) => map.get(key).unwrap_or_else(|| panic!("missing {key}")),
        other => panic!("not an object: {}", other.repr()),
    }
}

fn has(value: &PyValue, key: &str) -> bool {
    matches!(value, PyValue::Dict(map) if map.contains_key(key))
}

fn items(value: &PyValue) -> &[PyValue] {
    match value {
        PyValue::List(items) => items,
        other => panic!("not a list: {}", other.repr()),
    }
}

fn text_of(value: &PyValue) -> String {
    match value {
        PyValue::Str(text) => text.to_utf8().expect("text"),
        other => panic!("not text: {}", other.repr()),
    }
}

fn int_of(value: &PyValue) -> i64 {
    match value {
        PyValue::Int(number) => i64::try_from(number.clone()).expect("an integer"),
        other => panic!("not an integer: {}", other.repr()),
    }
}

fn hex_decode(text: &str) -> Vec<u8> {
    (0..text.len() / 2)
        .map(|at| u8::from_str_radix(&text[2 * at..2 * at + 2], 16).expect("hex"))
        .collect()
}

fn replace_bytes(bytes: &[u8], token: &str, with: &[u8]) -> Vec<u8> {
    let token = token.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at..].starts_with(token) {
            out.extend_from_slice(with);
            at += token.len();
        } else {
            out.push(bytes[at]);
            at += 1;
        }
    }
    out
}

/// Every text of the value with `@CASE@` replaced (by code point: a saved text may hold a lone surrogate).
fn rebase_all(value: &PyValue, root: &str) -> PyValue {
    match value {
        PyValue::Str(text) => {
            let marker: Vec<u32> = "@CASE@".chars().map(u32::from).collect();
            let replacement: Vec<u32> = root.chars().map(u32::from).collect();
            let points = text.code_points();
            let mut out = Vec::with_capacity(points.len());
            let mut at = 0;
            while at < points.len() {
                if points[at..].starts_with(&marker) {
                    out.extend_from_slice(&replacement);
                    at += marker.len();
                } else {
                    out.push(points[at]);
                    at += 1;
                }
            }
            PyValue::Str(PyStr::from_code_points(out))
        }
        PyValue::List(entries) => {
            PyValue::List(entries.iter().map(|each| rebase_all(each, root)).collect())
        }
        PyValue::Dict(map) => {
            let mut copy = PyDict::new();
            for (key, each) in map.iter() {
                copy.insert(key.clone(), rebase_all(each, root));
            }
            PyValue::Dict(copy)
        }
        other => other.clone(),
    }
}

fn dict(entries: Vec<(&str, PyValue)>) -> PyValue {
    let mut map = PyDict::new();
    for (key, value) in entries {
        map.set(key, value);
    }
    PyValue::Dict(map)
}

// ── the case area and its file tree ──────────────────────────────────────────────────────────────

struct Area {
    _held: PrivateDirectory,
    dir: PathBuf,
    root: PathBuf,
}

/// A new private area with an empty case root.
fn area(name: &str) -> Area {
    let held = PrivateDirectory::create(&format!("layout-{name}-")).expect("a private directory");
    let dir = held.path().to_path_buf();
    let root = dir.join("case");
    std::fs::create_dir(&root).expect("the case root");
    Area {
        _held: held,
        dir,
        root,
    }
}

fn root_text(area: &Area) -> String {
    area.root.to_str().expect("a path").to_string()
}

fn put(root: &Path, relative: &str, bytes: &[u8]) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("directories");
    std::fs::write(path, bytes).expect("a file");
}

/// Writes the files a saved case prepares: `{path: {kind: file, hex} | {kind: dir} | {kind: link, target}}`.
fn build_files(spec: &PyValue, root: &Path) {
    let PyValue::Dict(files) = spec else {
        panic!("a file spec")
    };
    for (path, entry) in files.iter() {
        let target = root.join(path.to_utf8().expect("a path"));
        std::fs::create_dir_all(target.parent().expect("a parent")).expect("directories");
        match text_of(member(entry, "kind")).as_str() {
            "dir" => std::fs::create_dir_all(&target).expect("a directory"),
            "link" => symlink(text_of(member(entry, "target")), &target).expect("a link"),
            _ => {
                std::fs::write(&target, hex_decode(&text_of(member(entry, "hex")))).expect("a file")
            }
        }
    }
}

type Tree = BTreeMap<String, (String, Vec<u8>)>;

/// The whole tree under `root`: path, kind (`dir`, `file`, `link`), bytes (file content, link target).
fn snapshot(root: &Path) -> Tree {
    fn walk(root: &Path, path: &Path, rows: &mut Tree) {
        let info = std::fs::symlink_metadata(path).expect("an entry");
        let name = path
            .strip_prefix(root)
            .expect("below the root")
            .to_string_lossy()
            .into_owned();
        if info.is_symlink() {
            let target = std::fs::read_link(path).expect("a link");
            rows.insert(
                name,
                (
                    "link".to_string(),
                    target.to_string_lossy().into_owned().into_bytes(),
                ),
            );
        } else if info.is_dir() {
            rows.insert(name, ("dir".to_string(), Vec::new()));
            let mut children: Vec<PathBuf> = std::fs::read_dir(path)
                .expect("a directory")
                .map(|entry| entry.expect("entry").path())
                .collect();
            children.sort();
            for child in children {
                walk(root, &child, rows);
            }
        } else {
            rows.insert(
                name,
                ("file".to_string(), std::fs::read(path).expect("bytes")),
            );
        }
    }
    let mut rows = Tree::new();
    walk(root, root, &mut rows);
    rows
}

// ── running programs ─────────────────────────────────────────────────────────────────────────────

/// A program over the case root: the working directory is the root, the environment is empty (no `PATH`, no Python).
fn command(program: &str, area: &Area) -> Command {
    let mut command = Command::new(program);
    command.env_clear().current_dir(&area.root);
    command
}

fn finish(mut command: Command) -> std::process::Output {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
        .spawn()
        .expect("start the program")
        .wait_with_output()
        .expect("wait for the program")
}

/// Starts `program args...` the way a caller passes a protocol descriptor (`pass_fds`): the pipe the caller writes to is descriptor 3 of the
/// program and descriptor 0 is the file `stdin` of the area.
fn over_descriptor(program: &str, args: &[&str], area: &Area) -> (Command, std::io::PipeWriter) {
    let (reader, writer) = std::io::pipe().expect("a pipe");
    let zero = area.dir.join("stdin");
    std::fs::write(&zero, b"").expect("a stdin file");
    let mut command = Command::new("/bin/sh");
    command
        .args(["-c", "exec \"$@\" 3<&0 0<\"$F0\"", "sh", program])
        .args(args)
        .env_clear()
        .env("F0", &zero)
        .current_dir(&area.root)
        .stdin(Stdio::from(reader))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    (command, writer)
}

fn serve_lines(area: &Area, lines: &[String]) -> (std::process::Output, Vec<PyValue>) {
    let (mut command, mut writer) = over_descriptor(
        env!("CARGO_BIN_EXE_upstream_layout_api"),
        &["evaluate-jsonl", "--input-fd", "3"],
        area,
    );
    let running = command.spawn().expect("start the service");
    writer
        .write_all(format!("{}\n", lines.join("\n")).as_bytes())
        .expect("the protocol bytes");
    drop(writer);
    let output = running.wait_with_output().expect("wait for the service");
    let records = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| loads_text(&PyStr::new(line)).expect("a record"))
        .collect();
    (output, records)
}

fn request(id: i64, operation: &str, args_dto: &str, kwargs_dto: &str, globals: &str) -> String {
    format!(
        "{{\"id\": {id}, \"operation\": \"check_spec_layout.{operation}\", \"args_dto\": {args_dto}, \"kwargs_dto\": {kwargs_dto}, \"globals\": {globals}}}"
    )
}

const DEFAULT_GLOBALS: &str = "{\"argv\": [\"check_spec_layout.py\"]}";

/// One call through the production service over `area`.
fn call1(area: &Area, operation: &str, args_dto: &str) -> PyValue {
    let (output, mut records) = serve_lines(
        area,
        &[request(
            1,
            operation,
            args_dto,
            "[\"dict\", []]",
            DEFAULT_GLOBALS,
        )],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(records.len(), 1);
    records.remove(0)
}

fn str_dto(text: &str) -> String {
    format!(
        "[\"str\", {}]",
        PyValue::str(text).dumps_ascii().expect("a JSON string")
    )
}

fn list_dto(members: &[String]) -> String {
    format!("[\"list\", [{}]]", members.join(", "))
}

fn args_dto(arguments: &[String]) -> String {
    list_dto(arguments)
}

// ── the event parent: the caller that owns the streams ───────────────────────────────────────────

fn ok_ack(sequence: i64) -> String {
    format!("{{\"event\": \"write_ack\", \"sequence\": {sequence}, \"kind\": \"ok\"}}")
}

/// What the parent saw and did.
struct Session {
    /// `(sequence, stream, text)` of every write event, in order.
    writes: Vec<(i64, String, PyStr)>,
    result: Option<PyValue>,
    status: Option<i32>,
    stderr: String,
    stray: Vec<String>,
}

/// Runs one request through `evaluate-events`; `ack_write(sequence, stream, text)` is the parent's answer line (`None`: it goes away).
fn session(
    area: &Area,
    request_line: &str,
    mut ack_write: impl FnMut(i64, &str, &PyStr) -> Option<String>,
) -> Session {
    let (mut command, mut writer) = over_descriptor(
        env!("CARGO_BIN_EXE_upstream_layout_api"),
        &["evaluate-events", "--input-fd", "3"],
        area,
    );
    let mut child = command.spawn().expect("start the program");
    drop(command);
    writeln!(writer, "{request_line}").expect("the request");
    writer.flush().expect("flush");
    let mut writer = Some(writer);
    let mut lines = BufReader::new(child.stdout.take().expect("events"));
    let mut seen = Session {
        writes: Vec::new(),
        result: None,
        status: None,
        stderr: String::new(),
        stray: Vec::new(),
    };
    loop {
        let mut raw = Vec::new();
        if lines.read_until(b'\n', &mut raw).expect("an event line") == 0 {
            break;
        }
        let text = PyStr::from_surrogateescape(&raw);
        let trimmed = PyStr::from_code_points(
            text.code_points()
                .iter()
                .copied()
                .filter(|&point| point != u32::from('\n'))
                .collect(),
        );
        let Ok(event) = loads_text(&trimmed) else {
            seen.stray.push(String::from_utf8_lossy(&raw).into_owned());
            continue;
        };
        match text_of(member(&event, "event")).as_str() {
            "write" => {
                let sequence = int_of(member(&event, "sequence"));
                let stream = text_of(member(&event, "stream"));
                let PyValue::Str(written) = member(&event, "text") else {
                    panic!("text")
                };
                seen.writes
                    .push((sequence, stream.clone(), written.clone()));
                match ack_write(sequence, &stream, written) {
                    Some(line) => {
                        if let Some(pipe) = writer.as_mut() {
                            let _ = writeln!(pipe, "{line}").and_then(|()| pipe.flush());
                        }
                    }
                    None => drop(writer.take()),
                }
            }
            "result" => seen.result = Some(member(&event, "record").clone()),
            other => seen.stray.push(other.to_string()),
        }
    }
    drop(writer);
    let mut stderr = child.stderr.take().expect("stderr");
    seen.status = child.wait().expect("wait").code();
    let mut bytes = Vec::new();
    stderr.read_to_end(&mut bytes).expect("stderr bytes");
    seen.stderr = String::from_utf8_lossy(&bytes).into_owned();
    seen
}

fn accept_all(sequence: i64, _stream: &str, _text: &PyStr) -> Option<String> {
    Some(ok_ack(sequence))
}

// ── the saved cases ──────────────────────────────────────────────────────────────────────────────

/// A saved case set up in a new area: the files under a root, the root marker.
struct Prepared {
    area: Area,
    root: String,
    before: Tree,
}

fn prepare(case: &PyValue, prefix: &str) -> Prepared {
    let request = member(case, "request");
    let id = text_of(member(request, "id"));
    let work = area(&format!("{prefix}-{id}"));
    build_files(member(request, "files"), &work.root);
    let before = snapshot(&work.root);
    Prepared {
        root: root_text(&work),
        area: work,
        before,
    }
}

fn argv_of(request: &PyValue, root: &str) -> Vec<String> {
    if has(request, "argv") {
        items(member(request, "argv"))
            .iter()
            .map(|each| text_of(&rebase_all(each, root)))
            .collect()
    } else {
        vec!["check_spec_layout.py".to_string()]
    }
}

fn all_or_fail(failures: Vec<String>) {
    assert!(
        failures.is_empty(),
        "{} problems:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

fn tree_problems(work: &Prepared, id: &str) -> Vec<String> {
    let after = snapshot(&work.area.root);
    if after == work.before {
        Vec::new()
    } else {
        vec![format!("{id}: the file tree changed")]
    }
}

fn read_control(path: &Path) -> PyValue {
    let bytes = std::fs::read(path).expect("the control record");
    loads_text(&PyStr::from_surrogateescape(&bytes)).expect("control JSON")
}

/// The last non-empty line of a stream: all that is compared of an uncaught exception's standard error (the interpreter's frames are OPEN_S1).
fn last_line(bytes: &[u8]) -> Vec<u8> {
    bytes
        .split(|&byte| byte == b'\n')
        .rfind(|line| !line.is_empty())
        .map(<[u8]>::to_vec)
        .unwrap_or_default()
}

/// Whether a saved command line ended in an exception nothing handled (its standard error is a Python traceback).
fn is_uncaught(case: &PyValue) -> bool {
    text_of(member(member(case, "original"), "kind")) == "exception"
}

mod reference_cases {
    use super::*;

    fn fixture_problems(case: &PyValue, cli: bool) -> Vec<String> {
        let request = member(case, "request");
        let id = text_of(member(request, "id"));
        let work = prepare(case, "fixture");
        let (spec, control) = (
            work.area.dir.join("spec.json"),
            work.area.dir.join("control.json"),
        );
        std::fs::write(&spec, request.dumps_ascii().expect("JSON")).expect("a spec");
        let mut run = command(
            if cli {
                env!("CARGO_BIN_EXE_upstream_layout_fixture_cli")
            } else {
                env!("CARGO_BIN_EXE_upstream_layout_fixture_api")
            },
            &work.area,
        );
        if !cli {
            run.arg("case");
        }
        run.arg("--spec")
            .arg(&spec)
            .arg("--root")
            .arg(&work.area.root)
            .arg("--control")
            .arg(&control);
        let output = finish(run);
        let mut problems = Vec::new();
        let want_status = if cli {
            int_of(member(case, "exit_code")).to_string()
        } else {
            "0".to_string()
        };
        if output.status.code().map(|code| code.to_string()).as_deref()
            != Some(want_status.as_str())
        {
            problems.push(format!(
                "status {:?} want {want_status}: {}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        if cli {
            let want_out = replace_bytes(
                &hex_decode(&text_of(member(case, "stdout_hex"))),
                "@CASE@",
                work.root.as_bytes(),
            );
            if output.stdout != want_out {
                problems.push(format!(
                    "stdout differs:\n--- want\n{}\n--- got\n{}",
                    String::from_utf8_lossy(&want_out),
                    String::from_utf8_lossy(&output.stdout)
                ));
            }
            let want_err = replace_bytes(
                &hex_decode(&text_of(member(case, "stderr_hex"))),
                "@CASE@",
                work.root.as_bytes(),
            );
            if is_uncaught(case) {
                if last_line(&output.stderr) != last_line(&want_err) {
                    problems.push(format!(
                        "the last stderr line of an uncaught exception differs: {:?}",
                        String::from_utf8_lossy(&output.stderr)
                    ));
                }
            } else if output.stderr != want_err {
                problems.push(format!(
                    "stderr differs:\n--- want\n{}\n--- got\n{}",
                    String::from_utf8_lossy(&want_err),
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
        } else if !output.stdout.is_empty() || !output.stderr.is_empty() {
            problems.push("the API fixture wrote to its own streams".to_string());
        }
        if control.exists() {
            let got = read_control(&control);
            let want = rebase_all(member(case, "original"), &work.root);
            if !same_value(&want, &got) {
                problems.push(format!(
                    "record differs:\n  want {}\n  got  {}",
                    want.repr(),
                    got.repr()
                ));
            }
        } else {
            problems.push("no control record".to_string());
        }
        problems.extend(tree_problems(&work, &id));
        problems
            .into_iter()
            .map(|problem| format!("{id}: {problem}"))
            .collect()
    }

    #[test]
    fn the_153_saved_calls_are_reproduced_whole_with_values_exceptions_the_trace_the_streams_and_the_globals(
    ) {
        let bundle = load("bundle.json");
        let cases = items(member(&bundle, "api"));
        assert_eq!(cases.len(), 153);
        all_or_fail(
            cases
                .iter()
                .flat_map(|case| fixture_problems(case, false))
                .collect(),
        );
    }

    #[test]
    fn the_73_saved_command_lines_end_as_the_original_did_with_their_trace_streams_status_and_files(
    ) {
        let bundle = load("bundle.json");
        let cases = items(member(&bundle, "cli"));
        assert_eq!(cases.len(), 73);
        assert_eq!(cases.iter().filter(|case| is_uncaught(case)).count(), 17);
        all_or_fail(
            cases
                .iter()
                .flat_map(|case| fixture_problems(case, true))
                .collect(),
        );
    }

    /// The original record as the services give it: no owner, module, code, trace, globals, argv; streams before the arguments.
    fn service_view(original: &PyValue) -> PyValue {
        let PyValue::Dict(map) = original else {
            panic!("a record")
        };
        let mut out = PyDict::new();
        for key in ["id", "kind", "value", "type", "message", "exception_chain"] {
            if let Some(value) = map.get(key) {
                out.set(key, value.clone());
            }
        }
        for key in ["stdout", "stderr", "args_after", "kwargs_after"] {
            out.set(key, member(original, key).clone());
        }
        PyValue::Dict(out)
    }

    fn request_line(case: &PyValue, work: &Prepared) -> String {
        let request_row = member(case, "request");
        let argv: Vec<PyValue> = argv_of(request_row, &work.root)
            .iter()
            .map(|each| PyValue::str(each))
            .collect();
        let mut line = PyDict::new();
        line.set("id", member(request_row, "id").clone());
        line.set(
            "operation",
            PyValue::str(&format!(
                "check_spec_layout.{}",
                text_of(member(request_row, "function"))
            )),
        );
        line.set(
            "args_dto",
            rebase_all(member(request_row, "args_dto"), &work.root),
        );
        line.set(
            "kwargs_dto",
            rebase_all(member(request_row, "kwargs_dto"), &work.root),
        );
        line.set("globals", dict(vec![("argv", PyValue::List(argv))]));
        PyValue::Dict(line).dumps_ascii().expect("a request")
    }

    fn service_problems(case: &PyValue) -> Vec<String> {
        let id = text_of(member(member(case, "request"), "id"));
        let work = prepare(case, "service");
        let (output, records) = serve_lines(&work.area, &[request_line(case, &work)]);
        let mut problems = Vec::new();
        if output.status.code() != Some(0) || !output.stderr.is_empty() || records.len() != 1 {
            problems.push(format!(
                "service ended {:?}: {}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr)
            ));
        } else {
            let want = service_view(&rebase_all(member(case, "original"), &work.root));
            if !same_value(&want, &records[0]) {
                problems.push(format!(
                    "record differs:\n  want {}\n  got  {}",
                    want.repr(),
                    records[0].repr()
                ));
            }
        }
        problems.extend(tree_problems(&work, &id));
        problems
            .into_iter()
            .map(|problem| format!("{id}: {problem}"))
            .collect()
    }

    #[test]
    fn the_153_saved_calls_agree_through_the_production_service() {
        let bundle = load("bundle.json");
        all_or_fail(
            items(member(&bundle, "api"))
                .iter()
                .flat_map(service_problems)
                .collect(),
        );
    }

    fn events_problems(case: &PyValue) -> Vec<String> {
        let id = text_of(member(member(case, "request"), "id"));
        let work = prepare(case, "events");
        let seen = session(&work.area, &request_line(case, &work), accept_all);
        let mut problems = Vec::new();
        if seen.status != Some(0) || !seen.stray.is_empty() {
            problems.push(format!(
                "events ended {:?}: {} {:?}",
                seen.status, seen.stderr, seen.stray
            ));
            return problems.into_iter().map(|p| format!("{id}: {p}")).collect();
        }
        let original = rebase_all(member(case, "original"), &work.root);
        let mut want_record = service_view(&original);
        if let PyValue::Dict(map) = &mut want_record {
            map.set("stdout", PyValue::str(""));
            map.set("stderr", PyValue::str(""));
        }
        let Some(got) = &seen.result else {
            problems.push("no result event".to_string());
            return problems.into_iter().map(|p| format!("{id}: {p}")).collect();
        };
        if !same_value(&want_record, got) {
            problems.push(format!(
                "record differs:\n  want {}\n  got  {}",
                want_record.repr(),
                got.repr()
            ));
        }
        // what the parent was told to write is what the original printed, per stream (`print` is two writes)
        for stream in ["stdout", "stderr"] {
            let joined: Vec<u32> = seen
                .writes
                .iter()
                .filter(|(_, name, _)| name == stream)
                .flat_map(|(_, _, text)| text.code_points().to_vec())
                .collect();
            let PyValue::Str(want) = member(&original, stream) else {
                panic!("a stream")
            };
            if joined != want.code_points() {
                problems.push(format!("{stream} writes differ"));
            }
        }
        problems.extend(tree_problems(&work, &id));
        problems
            .into_iter()
            .map(|problem| format!("{id}: {problem}"))
            .collect()
    }

    #[test]
    fn the_153_saved_calls_agree_through_the_event_service_with_a_parent_that_performs_the_writes()
    {
        let bundle = load("bundle.json");
        all_or_fail(
            items(member(&bundle, "api"))
                .iter()
                .flat_map(events_problems)
                .collect(),
        );
    }

    fn program_problems(case: &PyValue) -> Vec<String> {
        let request = member(case, "request");
        let id = text_of(member(request, "id"));
        let work = prepare(case, "program");
        let args: Vec<String> = argv_of(request, &work.root).into_iter().skip(1).collect();
        let mut run = command(env!("CARGO_BIN_EXE_check_spec_layout"), &work.area);
        run.args(&args);
        let output = finish(run);
        let mut problems = Vec::new();
        let want_status = int_of(member(case, "exit_code"));
        if output.status.code() != Some(i32::try_from(want_status).expect("a status")) {
            problems.push(format!(
                "status {:?} want {want_status}",
                output.status.code()
            ));
        }
        let want_out = replace_bytes(
            &hex_decode(&text_of(member(case, "stdout_hex"))),
            "@CASE@",
            work.root.as_bytes(),
        );
        let want_err = replace_bytes(
            &hex_decode(&text_of(member(case, "stderr_hex"))),
            "@CASE@",
            work.root.as_bytes(),
        );
        if output.stdout != want_out {
            problems.push("stdout differs".to_string());
        }
        if is_uncaught(case) {
            if last_line(&output.stderr) != last_line(&want_err) {
                problems.push(format!(
                    "the last stderr line differs: {:?}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
        } else if output.stderr != want_err {
            problems.push(format!(
                "stderr differs: {:?}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        problems.extend(tree_problems(&work, &id));
        problems
            .into_iter()
            .map(|problem| format!("{id}: {problem}"))
            .collect()
    }

    /// The 11 first originals of the set round: the fixture is fed the OBSERVED argument wire the collector saved before the call (the canonical authored
    /// spec is kept beside it), the whole control record is compared strictly under that order, never an answer taken from `expected`.
    fn set_rows() -> Vec<PyValue> {
        let rows = load("sets.json");
        let rows = items(&rows).to_vec();
        assert_eq!(rows.len(), 11);
        for row in &rows {
            assert!(matches!(
                member(member(row, "wire"), "captured_before_public_call"),
                PyValue::Bool(true)
            ));
            // the observed wire is the authored spec's canonical DTO plus the iteration orders, never different members
            assert!(has(member(row, "request"), "args_dto"));
        }
        rows
    }

    #[test]
    fn the_11_saved_set_calls_are_reproduced_whole_under_their_observed_order_through_the_fixture_the_service_and_the_event_service(
    ) {
        let mut problems = Vec::new();
        for row in set_rows() {
            problems.extend(fixture_problems(&row, false));
            problems.extend(service_problems(&row));
            problems.extend(events_problems(&row));
        }
        all_or_fail(problems);
    }

    /// The actual standalone entry of the original script (`python3 -I -S -B <script> ...`, no collector frames): the raw bytes and the status are the
    /// criterion. The program is started with the script's path as `argv[0]` (the owned path of the capture, rebased); that file does not exist
    /// in the test area, so nothing can have been read from it.
    fn standalone_problems(case: &PyValue, fixture: bool) -> Vec<String> {
        let request = member(case, "request");
        let id = text_of(member(request, "id"));
        let work = prepare(
            case,
            if fixture {
                "standalone-fixture"
            } else {
                "standalone"
            },
        );
        let script = text_of(&rebase_all(member(case, "argv0"), &work.root));
        let args: Vec<String> = items(member(case, "args"))
            .iter()
            .map(|each| text_of(&rebase_all(each, &work.root)))
            .collect();
        let output = if fixture {
            let mut argv = vec![PyValue::str(&script)];
            argv.extend(args.iter().map(|arg| PyValue::str(arg)));
            let mut spec = PyDict::new();
            for key in ["id", "owner", "function", "args_dto", "kwargs_dto"] {
                spec.set(key, member(request, key).clone());
            }
            spec.set("kind", PyValue::str("cli"));
            spec.set("argv", PyValue::List(argv));
            let (spec_file, control) = (
                work.area.dir.join("spec.json"),
                work.area.dir.join("control.json"),
            );
            std::fs::write(&spec_file, PyValue::Dict(spec).dumps_ascii().expect("JSON"))
                .expect("a spec");
            let mut run = command(
                env!("CARGO_BIN_EXE_upstream_layout_fixture_cli"),
                &work.area,
            );
            run.arg("--spec")
                .arg(&spec_file)
                .arg("--root")
                .arg(&work.area.root)
                .arg("--control")
                .arg(&control);
            finish(run)
        } else {
            use std::os::unix::process::CommandExt;
            let mut run = command(env!("CARGO_BIN_EXE_check_spec_layout"), &work.area);
            run.arg0(&script).args(&args);
            finish(run)
        };
        let mut problems = Vec::new();
        let want_status = int_of(member(case, "exit_code"));
        if output.status.code() != Some(i32::try_from(want_status).expect("a status")) {
            problems.push(format!(
                "status {:?} want {want_status}",
                output.status.code()
            ));
        }
        for (name, got, key) in [
            ("stdout", &output.stdout, "stdout_hex"),
            ("stderr", &output.stderr, "stderr_hex"),
        ] {
            let want = replace_bytes(
                &hex_decode(&text_of(member(case, key))),
                "@CASE@",
                work.root.as_bytes(),
            );
            if *got != want {
                problems.push(format!(
                    "{name} differs:\n--- want\n{}\n--- got\n{}",
                    String::from_utf8_lossy(&want),
                    String::from_utf8_lossy(got)
                ));
            }
        }
        problems.extend(tree_problems(&work, &id));
        problems
            .into_iter()
            .map(|problem| format!("{id}: {problem}"))
            .collect()
    }

    fn standalone_rows() -> Vec<PyValue> {
        let rows = load("standalone.json");
        let rows = items(&rows).to_vec();
        assert_eq!(rows.len(), 73);
        assert_eq!(
            rows.iter()
                .filter(|row| int_of(member(row, "exit_code")) == 1)
                .count(),
            17
        );
        rows
    }

    #[test]
    fn the_73_standalone_originals_end_with_exactly_the_raw_stdout_stderr_and_status_through_the_production_program(
    ) {
        all_or_fail(
            standalone_rows()
                .iter()
                .flat_map(|row| standalone_problems(row, false))
                .collect(),
        );
    }

    #[test]
    fn the_73_standalone_originals_end_with_exactly_the_raw_stdout_stderr_and_status_through_the_fixture_command(
    ) {
        all_or_fail(
            standalone_rows()
                .iter()
                .flat_map(|row| standalone_problems(row, true))
                .collect(),
        );
    }

    #[test]
    fn the_73_saved_command_lines_give_the_same_streams_and_status_through_the_production_program_without_python_or_a_path(
    ) {
        let bundle = load("bundle.json");
        all_or_fail(
            items(member(&bundle, "cli"))
                .iter()
                .flat_map(program_problems)
                .collect(),
        );
    }
}

// ── helpers of the crafted cases ─────────────────────────────────────────────────────────────────

fn record_value(record: &PyValue) -> &PyValue {
    assert_eq!(
        text_of(member(record, "kind")),
        "value",
        "{}",
        record.repr()
    );
    member(record, "value")
}

fn exception_of(record: &PyValue) -> (String, String) {
    assert_eq!(
        text_of(member(record, "kind")),
        "exception",
        "{}",
        record.repr()
    );
    (
        text_of(member(record, "type")),
        text_of(member(record, "message")),
    )
}

/// The DTO text of `(dict of index -> set of parts, total)`.
fn layout_dto(per_index: &[(&str, &[&str])], total: usize) -> String {
    let entries: Vec<String> = per_index
        .iter()
        .map(|(index, parts)| {
            let mut sorted: Vec<&str> = parts.to_vec();
            sorted.sort_by_key(|part| PyValue::str(part).dumps_ascii_tight().expect("JSON"));
            let members: Vec<String> = sorted.iter().map(|part| str_dto(part)).collect();
            format!("[{}, [\"set\", [{}]]]", str_dto(index), members.join(", "))
        })
        .collect();
    format!(
        "[\"tuple\", [[\"dict\", [{}]], [\"int\", \"{total}\"]]]",
        entries.join(", ")
    )
}

fn analyze_keys_call(area: &Area, keys: &[&str]) -> PyValue {
    let members: Vec<String> = keys.iter().map(|key| str_dto(key)).collect();
    call1(area, "analyze", &args_dto(&[list_dto(&members)]))
}

fn assert_layout(record: &PyValue, per_index: &[(&str, &[&str])], total: usize) {
    let want = j(&layout_dto(per_index, total));
    assert!(
        same_value(&want, record_value(record)),
        "want {}\n got {}",
        want.repr(),
        record_value(record).repr()
    );
}

fn config_call(area: &Area, text: &[u8]) -> PyValue {
    put(&area.root, "config.yaml", text);
    call1(
        area,
        "read_method_from_config",
        &args_dto(&[str_dto(&format!("{}/config.yaml", root_text(area)))]),
    )
}

fn model_call(area: &Area, operation: &str) -> PyValue {
    call1(
        area,
        operation,
        &args_dto(&[str_dto(&format!("{}/model", root_text(area)))]),
    )
}

mod contract_cases {
    use super::*;

    #[test]
    fn the_constants_are_the_sources_markers_components_and_pattern() {
        let bundle = load("bundle.json");
        let first = &items(member(&bundle, "api"))[0];
        let saved = member(member(first, "original"), "globals_before");
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_upstream_layout_api"))
            .arg("constants")
            .env_clear()
            .output()
            .expect("run");
        assert_eq!(output.status.code(), Some(0));
        let printed = loads_text(&PyStr::from_surrogateescape(&output.stdout)).expect("JSON");
        assert!(same_value(
            &dict(vec![("check_spec_layout", saved.clone())]),
            &printed
        ));
    }

    #[test]
    fn requests_the_service_cannot_model_end_in_a_request_record_and_the_binding_errors_are_pythons(
    ) {
        let work = area("requests");
        let lines = vec![
            // a set input has no deterministic original
            request(
                1,
                "analyze",
                "[\"list\", [[\"set\", [[\"str\", \"mtp.0.a.b\"]]]]]",
                "[\"dict\", []]",
                DEFAULT_GLOBALS,
            ),
            // a malformed DTO, an unhashable key
            request(
                2,
                "analyze",
                "[\"list\", [[\"int\", \"1.5\"]]]",
                "[\"dict\", []]",
                DEFAULT_GLOBALS,
            ),
            request(
                3,
                "analyze",
                "[\"list\", [[\"dict\", [[[\"list\", []], [\"none\"]]]]]]",
                "[\"dict\", []]",
                DEFAULT_GLOBALS,
            ),
            // plain JSON arguments (the resolver's wire) are not this service's
            "{\"id\": 4, \"operation\": \"check_spec_layout.analyze\", \"args\": [[]]}".to_string(),
            request(
                5,
                "analyze",
                "[\"list\", []]",
                "[\"dict\", []]",
                "{\"other\": 1}",
            ),
            request(
                6,
                "analyze",
                "[\"list\", []]",
                "[\"dict\", []]",
                "{\"DSPARK_ONLY_MARKERS\": [\"tuple\", []]}",
            ),
            "{\"id\": 7, \"operation\": \"check_spec_layout.analyze\", \"args_dto\": \"x\"}"
                .to_string(),
            "not json".to_string(),
            // the binding errors of the originals
            request(
                9,
                "analyze",
                "[\"list\", []]",
                "[\"dict\", []]",
                DEFAULT_GLOBALS,
            ),
            request(
                10,
                "analyze",
                "[\"list\", [[\"list\", []], [\"none\"]]]",
                "[\"dict\", []]",
                DEFAULT_GLOBALS,
            ),
            request(
                11,
                "analyze",
                "[\"list\", [[\"list\", []]]]",
                "[\"dict\", [[[\"str\", \"keys\"], [\"list\", []]]]]",
                DEFAULT_GLOBALS,
            ),
            request(
                12,
                "main",
                "[\"list\", [[\"none\"]]]",
                "[\"dict\", []]",
                DEFAULT_GLOBALS,
            ),
            request(
                13,
                "nothing",
                "[\"list\", []]",
                "[\"dict\", []]",
                DEFAULT_GLOBALS,
            ),
        ];
        let (output, records) = serve_lines(&work, &lines);
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(records.len(), lines.len());
        for at in [0, 1, 2, 3, 4, 5, 6, 7] {
            assert_eq!(
                exception_of(&records[at]).0,
                "RequestError",
                "{at}: {}",
                records[at].repr()
            );
        }
        assert_eq!(
            exception_of(&records[8]),
            (
                "TypeError".to_string(),
                "analyze() missing 1 required positional argument: 'keys'".to_string()
            )
        );
        assert_eq!(
            exception_of(&records[9]),
            (
                "TypeError".to_string(),
                "analyze() takes 1 positional argument but 2 were given".to_string()
            )
        );
        assert_eq!(
            exception_of(&records[10]),
            (
                "TypeError".to_string(),
                "analyze() got multiple values for argument 'keys'".to_string()
            )
        );
        assert_eq!(
            exception_of(&records[11]),
            (
                "TypeError".to_string(),
                "main() takes 0 positional arguments but 1 was given".to_string()
            )
        );
        assert_eq!(
            exception_of(&records[12]),
            (
                "AttributeError".to_string(),
                "module 'check_spec_layout' has no attribute 'nothing'".to_string()
            )
        );
    }

    /// The frame metadata is compiled-in data: the traceback modules read no file, and the table was generated from exactly the frozen source of the
    /// original script (the digest of the dataset's source), so a regeneration from another source is noticed here.
    #[test]
    fn the_traceback_metadata_is_generated_from_the_frozen_source_and_the_modules_read_no_file() {
        for (name, text) in [
            (
                "layout_traceback.rs",
                include_str!("../src/layout_traceback.rs"),
            ),
            ("layout_sites.rs", include_str!("../src/layout_sites.rs")),
        ] {
            for forbidden in [
                "std::fs",
                "File::open",
                "read_to_string",
                "std::env",
                "Command",
            ] {
                assert!(!text.contains(forbidden), "{name}: {forbidden}");
            }
        }
        let frozen = include_str!("data/layout_reference/owner_source.sha256").trim();
        assert_eq!(upstream_runtime::layout_sites::OWNER_SOURCE_SHA256, frozen);
        assert_eq!(
            upstream_runtime::layout_sites::REFERENCE_INTERPRETER,
            "CPython 3.12.3"
        );
        assert_eq!(upstream_runtime::layout_sites::SITES.len(), 14);
    }

    /// Reading the source: no provider or case lookup, no fixture module, no operational path, no Python, no process, no import of a YAML
    /// library, the environment read only for the terminal width.
    #[test]
    fn the_layout_sources_have_no_selector_no_fixture_module_no_operational_path_no_spawn_and_no_import(
    ) {
        let sources = [
            ("layout_api.rs", include_str!("../src/layout_api.rs")),
            ("layout_bench.rs", include_str!("../src/layout_bench.rs")),
            ("layout_cli.rs", include_str!("../src/layout_cli.rs")),
            ("layout_events.rs", include_str!("../src/layout_events.rs")),
            ("layout_files.rs", include_str!("../src/layout_files.rs")),
            ("layout_main.rs", include_str!("../src/layout_main.rs")),
            ("layout_rules.rs", include_str!("../src/layout_rules.rs")),
            (
                "layout_traceback.rs",
                include_str!("../src/layout_traceback.rs"),
            ),
            ("dto_decode.rs", include_str!("../src/dto_decode.rs")),
            (
                "bin/check_spec_layout.rs",
                include_str!("../src/bin/check_spec_layout.rs"),
            ),
            (
                "bin/upstream_layout_api.rs",
                include_str!("../src/bin/upstream_layout_api.rs"),
            ),
            (
                "bin/upstream_layout_bench.rs",
                include_str!("../src/bin/upstream_layout_bench.rs"),
            ),
        ];
        let mut found = Vec::new();
        for (name, text) in sources {
            for forbidden in [
                "getenv",
                "set_var",
                "remove_var",
                "layout_fixture",
                "real-worker",
                "val_agent",
                "CARGO_MANIFEST_DIR",
                "/usr/bin/python",
                "python3",
                "Command::new",
                "process::Command",
                "\"api-",
                "\"cli-",
                "@CASE@",
                "expected_lookup",
                "import_yaml",
                "YamlProfile::from",
                "safe_load",
            ] {
                if text.contains(forbidden) {
                    found.push(format!("{name}: {forbidden}"));
                }
            }
        }
        assert!(found.is_empty(), "{found:?}");
        // the measured call builds the whole DTO of the result inside the timed closure, and the loop reads no expected value
        let bench = include_str!("../src/layout_bench.rs");
        let closure = &bench[bench
            .find("call: Box::new(move ||")
            .expect("the measured closure")..];
        assert!(closure.contains("encode_owned(value)"));
    }
}

mod new_cases {
    use super::*;

    #[test]
    fn spec_keys_follow_the_pattern_on_unicode_digits_empty_parts_dots_line_ends_and_duplicates() {
        let work = area("analyze");
        // Unicode decimal digits and their leading zeros are kept; `model.layers.<n>.mtp_block` is the `block` index
        let record = analyze_keys_call(
            &work,
            &[
                "mtp.٠٣.ffn.w",
                "mtp.007.ffn.w",
                "model.layers.٥.mtp_block.attn.w",
                "mtp.0.attn.a",
            ],
        );
        assert_layout(
            &record,
            &[
                ("٠٣", &["ffn"]),
                ("007", &["ffn"]),
                ("block", &["attn"]),
                ("0", &["attn"]),
            ],
            4,
        );
        // a superscript is no decimal digit; the part after the dot needs one character; a first character dot is an empty component
        let record = analyze_keys_call(
            &work,
            &["mtp.²π.ffn", "mtp.0.", "mtp.0..x", "mtp.1.", "xmtp.0.attn"],
        );
        assert_layout(&record, &[("0", &[""])], 1);
        // `$` takes one final line feed, nothing else; a carriage return and the Unicode separators are ordinary characters
        let record = analyze_keys_call(
            &work,
            &[
                "mtp.0.attn\n",
                "mtp.0.attn\n\n",
                "mtp.0.at\ntn",
                "mtp.0.at\rtn",
                "mtp.0.a\u{2028}b",
                "mtp.0.\n",
            ],
        );
        assert_layout(&record, &[("0", &["attn", "at\rtn", "a\u{2028}b"])], 3);
        // the first insertion order of the indexes, every matching key counted (a duplicate too), a part once
        let record = analyze_keys_call(
            &work,
            &[
                "mtp.2.x.1",
                "mtp.10.y.1",
                "mtp.2.z.1",
                "mtp.2.x.2",
                "mtp.2.x.1",
            ],
        );
        assert_layout(&record, &[("2", &["x", "z"]), ("10", &["y"])], 5);
    }

    #[test]
    fn every_kind_of_keys_is_iterated_as_python_does_and_a_bad_key_stops_with_the_input_unchanged()
    {
        let work = area("iterate");
        let call = |dto: &str| call1(&work, "analyze", &args_dto(&[dto.to_string()]));
        // a tuple, a str (its characters), a dict (its keys, of any type), bytes (their integers)
        assert_layout(
            &call("[\"tuple\", [[\"str\", \"mtp.0.attn.a\"]]]"),
            &[("0", &["attn"])],
            1,
        );
        assert_layout(&call("[\"str\", \"mtp.0.attn.a\"]"), &[], 0);
        assert_layout(
            &call("[\"dict\", [[[\"str\", \"mtp.1.ffn.a\"], [\"int\", \"1\"]], [[\"str\", \"mtp.1.ffn.b\"], [\"none\"]]]]"),
            &[("1", &["ffn"])],
            2,
        );
        let (class, message) = exception_of(&call("[\"dict\", [[[\"int\", \"7\"], [\"none\"]]]]"));
        assert_eq!(
            (class.as_str(), message.as_str()),
            (
                "TypeError",
                "expected string or bytes-like object, got 'int'"
            )
        );
        let (_, message) = exception_of(&call("[\"bytes\", \"6162\"]"));
        assert_eq!(message, "expected string or bytes-like object, got 'int'");
        for (dto, shown) in [
            ("[\"none\"]", "'NoneType'"),
            ("[\"int\", \"5\"]", "'int'"),
            ("[\"bool\", true]", "'bool'"),
            ("[\"float\", \"1.5\"]", "'float'"),
        ] {
            let (class, message) = exception_of(&call(dto));
            assert_eq!(class, "TypeError");
            assert_eq!(message, format!("{shown} object is not iterable"));
        }
        // a key of the wrong type after good ones: a TypeError of the regular expression, and the argument comes back unchanged
        let input = "[\"list\", [[\"str\", \"mtp.0.attn.a\"], [\"bytes\", \"ff\"], [\"str\", \"mtp.1.ffn.b\"]]]";
        let record = call(input);
        assert_eq!(
            exception_of(&record),
            (
                "TypeError".to_string(),
                "cannot use a string pattern on a bytes-like object".to_string()
            )
        );
        assert!(same_value(
            member(&record, "args_after"),
            &j(&format!("[\"list\", [{input}]]"))
        ));
        for (element, shown) in [
            ("[\"none\"]", "NoneType"),
            ("[\"bool\", true]", "bool"),
            ("[\"tuple\", []]", "tuple"),
        ] {
            let record = call(&format!("[\"list\", [{element}]]"));
            assert_eq!(
                exception_of(&record).1,
                format!("expected string or bytes-like object, got '{shown}'")
            );
        }
    }

    #[test]
    fn the_config_line_is_matched_by_the_source_pattern_stripped_and_decoded_as_the_original_does()
    {
        let work = area("config");
        let method = |text: &[u8]| {
            let record = config_call(&work, text);
            record_value(&record).clone()
        };
        let none = j("[\"none\"]");
        let text = |value: &str| {
            j(&format!(
                "[\"str\", {}]",
                PyValue::str(value).dumps_ascii().unwrap()
            ))
        };
        // the first speculative-config line decides, even when it holds no method
        assert!(same_value(
            &method(b"speculative_config: {}\nspeculative-config: {\"method\":\"mtp\"}\n"),
            &none
        ));
        // quotes around the value, any run of them, and the Unicode whitespace of `\s`
        assert!(same_value(
            &method(b"speculative-config: '\"{\\\"method\\\":\\\"a\\\"}\"'\n"),
            &none
        ));
        assert!(same_value(
            &method("\u{a0}speculative-config\u{2003}:\u{3000}'{\"method\":\"u\"}'\n".as_bytes()),
            &text("u")
        ));
        assert!(same_value(
            &method(b"  speculative_config :''{\"method\":\"d\"}''"),
            &text("d")
        ));
        // a line that is only whitespace after the colon gives back one space (the group needs a character): the empty value is no JSON
        assert!(same_value(&method(b"speculative-config:   \n"), &none));
        // not JSON: the first `\"method\": \"...\"` of the source pattern, none when it needs another quote style
        assert!(same_value(
            &method(b"speculative-config: {\"method\": \"mtp\",}\n"),
            &text("mtp")
        ));
        assert!(same_value(
            &method(b"speculative-config: {'method': 'mtp'}\n"),
            &none
        ));
        // a falsy JSON value is `{}`, a method that is not text is returned as it is (no coercion)
        assert!(same_value(&method(b"speculative-config: 0\n"), &none));
        assert!(same_value(
            &method(b"speculative-config: {\"method\": 7}\n"),
            &j("[\"int\", \"7\"]")
        ));
        assert!(same_value(
            &method(b"speculative-config: {\"method\": [1, \"a\"]}\n"),
            &j("[\"list\", [[\"int\", \"1\"], [\"str\", \"a\"]]]")
        ));
        // carriage returns are line ends (universal newlines)
        assert!(same_value(
            &method(b"a: 1\rspeculative-config: {\"method\":\"cr\"}\r\n"),
            &text("cr")
        ));
        // a missing file is `None`, a directory too (an OSError)
        let record = call1(
            &work,
            "read_method_from_config",
            &args_dto(&[str_dto(&format!("{}/missing.yaml", root_text(&work)))]),
        );
        assert!(same_value(record_value(&record), &none));
        std::fs::create_dir(work.root.join("dir.yaml")).expect("a directory");
        let record = call1(
            &work,
            "read_method_from_config",
            &args_dto(&[str_dto(&format!("{}/dir.yaml", root_text(&work)))]),
        );
        assert!(same_value(record_value(&record), &none));
    }

    #[test]
    fn the_index_files_are_tried_in_order_and_the_first_one_that_exists_decides() {
        let work = area("index");
        let keys = |record: &PyValue| record_value(record).clone();
        // a directory named like the first index is no file: the second is used
        std::fs::create_dir_all(work.root.join("model/model.safetensors.index.json"))
            .expect("a directory");
        put(
            &work.root,
            "model/pytorch_model.bin.index.json",
            b"{\"weight_map\": {\"a\": 1, \"b\": 2}}",
        );
        let record = model_call(&work, "load_index_keys");
        assert!(same_value(
            &keys(&record),
            &j("[\"tuple\", [[\"list\", [[\"str\", \"a\"], [\"str\", \"b\"]]], [\"str\", \"pytorch_model.bin.index.json\"]]]")
        ));
        // the first file that exists is the only one read: a bad first is a reason, the valid second is NOT tried
        let work = area("index-bad-first");
        put(
            &work.root,
            "model/model.safetensors.index.json",
            b"{\"weight_map\": ",
        );
        put(
            &work.root,
            "model/pytorch_model.bin.index.json",
            b"{\"weight_map\": {\"a\": 1}}",
        );
        let record = model_call(&work, "load_index_keys");
        let want = "[\"tuple\", [[\"none\"], [\"str\", \"model.safetensors.index.json 파싱 실패: JSONDecodeError('Expecting value: line 1 column 16 (char 15)')\"]]]";
        assert!(
            same_value(&keys(&record), &j(want)),
            "{}",
            keys(&record).repr()
        );
        // a missing weight_map or a falsy one is no keys; a truthy non-dict is the AttributeError of `.keys()`
        let work = area("index-shapes");
        put(
            &work.root,
            "model/model.safetensors.index.json",
            b"{\"weight_map\": 0}",
        );
        let record = model_call(&work, "load_index_keys");
        assert!(same_value(
            &keys(&record),
            &j("[\"tuple\", [[\"list\", []], [\"str\", \"model.safetensors.index.json\"]]]")
        ));
        put(
            &work.root,
            "model/model.safetensors.index.json",
            b"{\"weight_map\": [1]}",
        );
        let record = model_call(&work, "load_index_keys");
        assert_eq!(
            exception_of(&record),
            (
                "AttributeError".to_string(),
                "'list' object has no attribute 'keys'".to_string()
            )
        );
        // the keys keep the order of the document; a number is no coercion of `num_nextn_predict_layers`
        put(
            &work.root,
            "model/model.safetensors.index.json",
            b"{\"weight_map\": {\"z\": 1, \"a\": 2, \"m\": 3}}",
        );
        put(
            &work.root,
            "model/config.json",
            b"{\"num_nextn_predict_layers\": 1.0}",
        );
        let record = model_call(&work, "load_index_keys");
        assert!(same_value(
            &keys(&record),
            &j("[\"tuple\", [[\"list\", [[\"str\", \"z\"], [\"str\", \"a\"], [\"str\", \"m\"]]], [\"str\", \"model.safetensors.index.json\"]]]")
        ));
        let record = model_call(&work, "read_num_nextn");
        assert!(same_value(
            record_value(&record),
            &j("[\"float\", \"1.0\"]")
        ));
    }

    #[test]
    fn a_config_longer_than_one_decoder_chunk_is_decoded_chunk_by_chunk() {
        let work = area("chunks");
        // the first line spans two chunks of the decoder (8192 bytes each); the config line is in the second and the invalid byte in the third:
        // the first match returns before that chunk is ever decoded
        let mut text = "#".repeat(8999).into_bytes();
        text.push(b'\n');
        text.extend_from_slice(b"speculative-config: {\"method\":\"late\"}\n");
        text.extend_from_slice("#".repeat(99).repeat(120).as_bytes());
        text.push(b'\n');
        text.push(0xff);
        assert!(text.len() > 3 * 4096 + 8192 - 1);
        let record = config_call(&work, &text);
        assert!(same_value(record_value(&record), &j("[\"str\", \"late\"]")));
        // no config line: the invalid byte is decoded in its chunk and the error names that chunk (the bytes the decoder was handed, the
        // position counted inside them: 9000 - 8192)
        let mut text = "#".repeat(8999).into_bytes();
        text.push(b'\n');
        text.push(0xff);
        text.extend_from_slice(b"\nspeculative-config: {}\n");
        let record = config_call(&work, &text);
        let (class, message) = exception_of(&record);
        assert_eq!(class, "UnicodeDecodeError");
        assert_eq!(
            message,
            "'utf-8' codec can't decode byte 0xff in position 808: invalid start byte"
        );
        let chain = items(member(&record, "exception_chain"));
        let constructor = member(&chain[0], "constructor");
        assert_eq!(int_of(member(constructor, "start")), 808);
        assert_eq!(int_of(member(constructor, "end")), 809);
        assert_eq!(text_of(member(constructor, "reason")), "invalid start byte");
        assert_eq!(
            text_of(member(constructor, "object_hex")).len(),
            2 * (text.len() - 8192)
        );
    }

    #[test]
    fn the_command_line_runs_through_the_event_service_with_the_writes_announced_and_a_dying_parent_stops_it(
    ) {
        let work = area("events-main");
        put(
            &work.root,
            "model/model.safetensors.index.json",
            b"{\"weight_map\": {\"mtp.0.attn.w\": \"s\"}}",
        );
        let line = request(
            1,
            "main",
            "[\"list\", []]",
            "[\"dict\", []]",
            &format!(
                "{{\"argv\": [\"/x/check_spec_layout.py\", \"--model-dir\", {}, \"--method\", \"mtp\"]}}",
                PyValue::str(&format!("{}/model", root_text(&work))).dumps_ascii().unwrap()
            ),
        );
        let seen = session(&work, &line, accept_all);
        assert_eq!(seen.status, Some(0), "{}", seen.stderr);
        assert!(seen.stray.is_empty());
        let result = seen.result.expect("a result");
        assert_eq!(
            exception_of(&result),
            ("SystemExit".to_string(), "0".to_string())
        );
        assert_eq!(text_of(member(&result, "stdout")), "");
        // `print` is two writes; the sequence has no gap
        let streams: Vec<(&str, String)> = seen
            .writes
            .iter()
            .map(|(_, stream, text)| (stream.as_str(), text.to_utf8().unwrap()))
            .collect();
        assert_eq!(streams.len(), 6);
        assert!(streams.iter().all(|(stream, _)| *stream == "stdout"));
        assert!(streams[0].1.starts_with(
            "[spec-layout] provenance=measured src=model.safetensors.index.json 전체텐서=1"
        ));
        assert_eq!(streams[1].1, "\n");
        assert_eq!(
            seen.writes
                .iter()
                .map(|(sequence, _, _)| *sequence)
                .collect::<Vec<_>>(),
            [0, 1, 2, 3, 4, 5]
        );
        // the caller's stream failed at the first write: that exception is the outcome and nothing after it ran
        let failing = |sequence: i64, _: &str, _: &PyStr| {
            Some(format!("{{\"event\": \"write_ack\", \"sequence\": {sequence}, \"kind\": \"exception\", \"type\": \"BrokenPipeError\", \"module\": \"builtins\", \"message\": \"[Errno 32] Broken pipe\", \"builtin_bases\": [\"BrokenPipeError\", \"ConnectionError\", \"OSError\", \"Exception\", \"BaseException\"]}}"))
        };
        let seen = session(&work, &line, failing);
        assert_eq!(seen.status, Some(0), "{}", seen.stderr);
        assert_eq!(seen.writes.len(), 1);
        assert_eq!(
            exception_of(&seen.result.expect("a result")),
            (
                "BrokenPipeError".to_string(),
                "[Errno 32] Broken pipe".to_string()
            )
        );
        // a parent that goes away abandons the run: no result, status 3
        let seen = session(&work, &line, |_, _, _| None);
        assert_eq!(seen.status, Some(3));
        assert!(seen.result.is_none());
    }
}

mod oracle_cases {
    use super::*;
    use upstream_runtime::layout_rules::{
        config_line_value, first_component, match_spec_key, search_quoted_method,
        text_of as text_from, unquote_value,
    };

    fn points(value: &PyValue) -> Vec<u32> {
        match value {
            PyValue::Str(text) => text.code_points().to_vec(),
            other => panic!("not text: {}", other.repr()),
        }
    }

    fn optional(value: &PyValue) -> Option<Vec<u32>> {
        match value {
            PyValue::None => None,
            other => Some(points(other)),
        }
    }

    /// The saved results of CPython 3.12.3's `re` for the three fixed expressions of the source (patterns read from the source text, the owner
    /// never run: `W/dev/layout_regex_oracle.py`) over template corpora of Unicode digits, separators, whitespace, quotes and line ends.
    #[test]
    fn the_hand_matched_expressions_agree_with_cpythons_re_over_the_saved_corpora() {
        let oracle = load("regex_oracle.json");
        assert_eq!(text_of(member(&oracle, "python")), "3.12.3");
        let mut problems = Vec::new();
        let spec_keys = items(member(&oracle, "spec_keys"));
        let mut matched = 0;
        for row in spec_keys {
            let key = points(&items(row)[0]);
            let ours =
                match_spec_key(&key).map(|found| (found.index, first_component(&found.rest)));
            let want = match &items(row)[1] {
                PyValue::None => None,
                pair => Some((optional(&items(pair)[0]), points(&items(pair)[1]))),
            };
            matched += usize::from(want.is_some());
            if ours != want {
                problems.push(format!(
                    "spec key {:?}: want {want:?} got {ours:?}",
                    text_from(&key).repr()
                ));
            }
        }
        assert_eq!((spec_keys.len(), matched), (588, 185));
        let lines = items(member(&oracle, "config_lines"));
        let mut hits = 0;
        for row in lines {
            let line = points(&items(row)[0]);
            let want = optional(&items(row)[1]);
            hits += usize::from(want.is_some());
            let ours = config_line_value(&line);
            if ours != want {
                problems.push(format!(
                    "config line {:?}: want {want:?} got {ours:?}",
                    text_from(&line).repr()
                ));
            }
        }
        assert_eq!((lines.len(), hits), (5184, 1206));
        for row in items(member(&oracle, "unquote")) {
            let group = points(&items(row)[0]);
            let want = points(&items(row)[1]);
            if unquote_value(&group) != want {
                problems.push(format!("unquote {:?}", text_from(&group).repr()));
            }
        }
        for row in items(member(&oracle, "search")) {
            let raw = points(&items(row)[0]);
            let want = optional(&items(row)[1]);
            if search_quoted_method(&raw) != want {
                problems.push(format!("search {:?}", text_from(&raw).repr()));
            }
        }
        all_or_fail(problems);
    }
}

mod bench_cases {
    use super::*;

    fn saved<'a>(bundle: &'a PyValue, id: &str) -> &'a PyValue {
        items(member(bundle, "api"))
            .iter()
            .find(|case| text_of(member(member(case, "request"), "id")) == id)
            .unwrap_or_else(|| panic!("no saved case {id}"))
    }

    /// A benchmark row of a saved call: the arguments as DTO (the case root rebased), the whole saved DTO as the expected value.
    fn row(
        metric: &str,
        case: &PyValue,
        root: &str,
        iterations: Option<&str>,
        expected: &PyValue,
    ) -> PyValue {
        let request = member(case, "request");
        let mut members = vec![
            ("metric", PyValue::str(metric)),
            (
                "operation",
                PyValue::str(&format!(
                    "check_spec_layout.{}",
                    text_of(member(request, "function"))
                )),
            ),
            ("args_dto", rebase_all(member(request, "args_dto"), root)),
            (
                "kwargs_dto",
                rebase_all(member(request, "kwargs_dto"), root),
            ),
            ("expected", expected.clone()),
        ];
        if let Some(count) = iterations {
            members.push(("iterations", j(count)));
        }
        dict(members)
    }

    fn run_bench(work: &Area, fixture: &Path, extra: &[&str]) -> std::process::Output {
        let mut run = command(env!("CARGO_BIN_EXE_upstream_layout_bench"), work);
        run.arg("--fixture").arg(fixture).args(extra);
        finish(run)
    }

    fn write_rows(work: &Area, name: &str, rows: Vec<PyValue>) -> PathBuf {
        let path = work.dir.join(name);
        std::fs::write(&path, PyValue::List(rows).dumps_ascii().expect("JSON")).expect("a fixture");
        path
    }

    #[test]
    fn the_benchmark_checks_the_whole_dto_of_each_saved_call_once_then_measures_rows_with_their_own_iteration_counts(
    ) {
        let bundle = load("bundle.json");
        let (analyze, index) = (saved(&bundle, "api-086"), saved(&bundle, "api-120"));
        assert_eq!(
            text_of(member(member(analyze, "request"), "function")),
            "analyze"
        );
        assert_eq!(
            text_of(member(member(index, "request"), "function")),
            "load_index_keys"
        );
        let work = area("bench");
        build_files(member(member(index, "request"), "files"), &work.root);
        let root = root_text(&work);
        let good = write_rows(
            &work,
            "good.json",
            vec![
                row(
                    "analyze_2048",
                    analyze,
                    &root,
                    Some("200"),
                    member(member(analyze, "original"), "value"),
                ),
                row(
                    "load_index_keys_2048",
                    index,
                    &root,
                    Some("5"),
                    member(member(index, "original"), "value"),
                ),
            ],
        );
        let output = run_bench(&work, &good, &["--rounds", "11", "--iterations", "7"]);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let text = String::from_utf8(output.stdout).expect("text");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 3, "{text}");
        assert_eq!(lines[0], "{\"rounds\": 11, \"iterations\": 7}");
        for (line, (metric, count)) in lines[1..]
            .iter()
            .zip([("analyze_2048", 200), ("load_index_keys_2048", 5)])
        {
            let report = loads_text(&PyStr::new(line)).expect("a row");
            assert_eq!(text_of(member(&report, "metric")), metric);
            assert_eq!(text_of(member(&report, "unit")), "ns_per_call");
            assert_eq!(items(member(&report, "rounds")).len(), 11);
            assert_eq!(int_of(member(&report, "round_count")), 11);
            assert_eq!(int_of(member(&report, "iterations")), count);
        }
        // a different whole value ends the run before any timer: no report (one set member of the expected result is changed)
        let mut wrong = member(member(analyze, "original"), "value")
            .dumps_ascii()
            .expect("JSON");
        let at = wrong.find("\"set\", [").expect("a set") + 8;
        wrong.insert_str(at, "[\"str\", \"zz-extra\"], ");
        let path = write_rows(
            &work,
            "wrong.json",
            vec![row("analyze_2048", analyze, &root, None, &j(&wrong))],
        );
        let output = run_bench(&work, &path, &["--rounds", "2", "--iterations", "3"]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        // the index file is read for real in the preflight: a changed file is a difference
        std::fs::write(
            work.root.join("model/model.safetensors.index.json"),
            b"{\"weight_map\": {}}",
        )
        .expect("a file");
        let output = run_bench(&work, &good, &["--rounds", "1", "--iterations", "1"]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
    }

    #[test]
    fn an_unknown_operation_a_missing_expected_value_a_bad_count_and_a_bad_option_end_the_run_without_a_report(
    ) {
        let work = area("bench-refusals");
        let rows = [
            r#"[{"metric": "m", "operation": "check_spec_layout.nothing", "args_dto": ["list", []], "kwargs_dto": ["dict", []], "expected": ["none"]}]"#,
            r#"[{"metric": "m", "operation": "other.analyze", "args_dto": ["list", [["list", []]]], "kwargs_dto": ["dict", []], "expected": ["none"]}]"#,
            r#"[{"metric": "m", "operation": "check_spec_layout.analyze", "args_dto": ["list", [["list", []]]], "kwargs_dto": ["dict", []]}]"#,
            r#"[{"metric": "m", "operation": "check_spec_layout.analyze", "args_dto": ["list", [["list", []]]], "kwargs_dto": ["dict", []], "expected": ["none"], "iterations": 0}]"#,
            r#"[{"metric": "m", "operation": "check_spec_layout.analyze", "args_dto": ["list", [["set", []]]], "kwargs_dto": ["dict", []], "expected": ["none"]}]"#,
            r#"[{"metric": "m", "operation": "check_spec_layout.analyze", "args": [[]], "expected": ["none"]}]"#,
            r#"{"not": "an array"}"#,
        ];
        for (at, text) in rows.iter().enumerate() {
            let path = work.dir.join(format!("refusal-{at}.json"));
            std::fs::write(&path, text).expect("a fixture");
            let output = run_bench(&work, &path, &[]);
            assert_eq!(output.status.code(), Some(1), "{text}");
            assert!(output.stdout.is_empty(), "{text}");
        }
        let fixture = work.dir.join("refusal-0.json");
        for extra in [
            vec!["--rounds", "0"],
            vec!["--iterations", "x"],
            vec!["--bogus", "1"],
        ] {
            assert_eq!(
                run_bench(&work, &fixture, &extra).status.code(),
                Some(2),
                "{extra:?}"
            );
        }
        assert_eq!(
            run_bench(&work, &work.dir.join("absent.json"), &[])
                .status
                .code(),
            Some(1)
        );
    }
}

mod set_cases {
    use super::*;

    fn set_dto(members: &[&str], order: &str) -> String {
        format!("[\"set\", [{}], {order}]", members.join(", "))
    }

    fn analyze_set(work: &Area, dto: &str) -> PyValue {
        call1(work, "analyze", &args_dto(&[dto.to_string()]))
    }

    fn error_class(record: &PyValue) -> String {
        exception_of(record).0
    }

    /// The members of two spec keys, sorted by their compact JSON as the wire requires.
    const ONE: &str = "[\"str\", \"mtp.1.a.x\"]";
    const TWO: &str = "[\"str\", \"mtp.2.b.x\"]";

    #[test]
    fn the_observed_order_decides_the_key_order_of_the_result_and_never_its_membership_or_the_canonical_arguments_back(
    ) {
        let work = area("set-order");
        for (order, keys) in [
            ("[0, 1]", ["1", "2"]),
            ("[1, 0]", ["2", "1"]),
            ("\"sorted\"", ["1", "2"]),
        ] {
            let record = analyze_set(&work, &set_dto(&[ONE, TWO], order));
            assert_layout(
                &record,
                &[
                    (keys[0], &[if keys[0] == "1" { "a" } else { "b" }]),
                    (keys[1], &[if keys[1] == "1" { "a" } else { "b" }]),
                ],
                2,
            );
            // the arguments come back in the canonical two-element form whatever order was supplied
            assert!(same_value(
                member(&record, "args_after"),
                &j(&format!("[\"list\", [[\"set\", [{ONE}, {TWO}]]]]"))
            ));
        }
    }

    #[test]
    fn the_first_failing_member_in_the_observed_order_is_the_one_whose_type_error_is_raised() {
        let work = area("set-first-error");
        // members sorted by compact JSON: the int, the None, the string
        let members = ["[\"int\", \"5\"]", "[\"none\"]", "[\"str\", \"mtp.0.a.b\"]"];
        let message = |order: &str| exception_of(&analyze_set(&work, &set_dto(&members, order))).1;
        assert_eq!(
            message("[2, 1, 0]"),
            "expected string or bytes-like object, got 'NoneType'"
        );
        assert_eq!(
            message("[2, 0, 1]"),
            "expected string or bytes-like object, got 'int'"
        );
        assert_eq!(
            message("[0, 1, 2]"),
            "expected string or bytes-like object, got 'int'"
        );
        assert_eq!(
            message("[1, 0, 2]"),
            "expected string or bytes-like object, got 'NoneType'"
        );
        // a bytes member among strings: the pattern is a string pattern, and only when it is reached
        let record = analyze_set(
            &work,
            &set_dto(
                &["[\"bytes\", \"6d\"]", "[\"str\", \"mtp.1.ffn\"]"],
                "[1, 0]",
            ),
        );
        assert_eq!(
            exception_of(&record).1,
            "cannot use a string pattern on a bytes-like object"
        );
        // a tuple member is hashable and is no string
        let record = analyze_set(&work, &set_dto(&["[\"tuple\", [[\"int\", \"1\"]]]"], "[0]"));
        assert_eq!(
            exception_of(&record).1,
            "expected string or bytes-like object, got 'tuple'"
        );
    }

    #[test]
    fn an_empty_set_a_keyword_set_and_a_set_inside_a_list_are_valid_inputs() {
        let work = area("set-shapes");
        for order in ["[]", "\"sorted\""] {
            assert_layout(&analyze_set(&work, &set_dto(&[], order)), &[], 0);
        }
        // as the keyword `keys=`
        let line = request(
            1,
            "analyze",
            "[\"list\", []]",
            &format!(
                "[\"dict\", [[[\"str\", \"keys\"], {}]]]",
                set_dto(&[ONE, TWO], "[1, 0]")
            ),
            DEFAULT_GLOBALS,
        );
        let (output, records) = serve_lines(&work, &[line]);
        assert_eq!(output.status.code(), Some(0));
        assert_layout(&records[0], &[("2", &["b"]), ("1", &["a"])], 2);
        assert!(same_value(
            member(&records[0], "kwargs_after"),
            &j(&format!(
                "[\"dict\", [[[\"str\", \"keys\"], [\"set\", [{ONE}, {TWO}]]]]]"
            ))
        ));
        // a set as one key of a list is a key of the wrong type, not a request error
        let record = call1(
            &work,
            "analyze",
            &args_dto(&[format!("[\"list\", [{}]]", set_dto(&[ONE], "[0]"))]),
        );
        assert_eq!(
            exception_of(&record),
            (
                "TypeError".to_string(),
                "expected string or bytes-like object, got 'set'".to_string()
            )
        );
        // a set given as a path is the TypeError of the source call
        let record = call1(
            &work,
            "read_num_nextn",
            &args_dto(&[set_dto(&[ONE], "[0]")]),
        );
        assert_eq!(
            exception_of(&record),
            (
                "TypeError".to_string(),
                "expected str, bytes or os.PathLike object, not set".to_string()
            )
        );
    }

    #[test]
    fn a_wrong_order_wire_an_unsorted_or_equal_or_unhashable_member_and_a_missing_order_are_request_errors(
    ) {
        let work = area("set-wire");
        let bad = [
            // the order: duplicate, out of range, wrong length, not integers, unknown word, missing
            set_dto(&[ONE, TWO], "[0, 0]"),
            set_dto(&[ONE, TWO], "[0, 2]"),
            set_dto(&[ONE, TWO], "[0]"),
            set_dto(&[ONE, TWO], "[0, 1, 1]"),
            set_dto(&[ONE, TWO], "[\"0\", \"1\"]"),
            set_dto(&[ONE, TWO], "[-1, 0]"),
            set_dto(&[ONE, TWO], "\"unsorted\""),
            set_dto(&[ONE, TWO], "null"),
            format!("[\"set\", [{ONE}, {TWO}]]"),
            // the members: not ascending, repeated, equal under Python's == (True and 1), unhashable, a set inside a set
            set_dto(&[TWO, ONE], "[0, 1]"),
            set_dto(&[ONE, ONE], "[0, 1]"),
            set_dto(&["[\"bool\", true]", "[\"int\", \"1\"]"], "[0, 1]"),
            set_dto(&["[\"list\", []]"], "[0]"),
            set_dto(&["[\"set\", [], \"sorted\"]"], "[0]"),
            "[\"set\", [], [], []]".to_string(),
            "[\"set\", \"x\", []]".to_string(),
        ];
        for (at, dto) in bad.iter().enumerate() {
            let record = analyze_set(&work, dto);
            assert_eq!(error_class(&record), "RequestError", "{at}: {dto}");
        }
    }
}

mod public_command_cases {
    use super::*;
    use std::os::unix::process::CommandExt;

    fn failing_case(work: &Area) -> Vec<String> {
        put(
            &work.root,
            "model/model.safetensors.index.json",
            b"{\"weight_map\": {\"mtp.0.main_norm.w\": \"s\"}}",
        );
        put(
            &work.root,
            "model/config.json",
            b"{\"num_nextn_predict_layers\": \"2\"}",
        );
        vec![
            "--model-dir".to_string(),
            format!("{}/model", root_text(work)),
            "--method".to_string(),
            "mtp".to_string(),
        ]
    }

    fn run(work: &Area, arg0: &str, args: &[String]) -> std::process::Output {
        let mut run = command(env!("CARGO_BIN_EXE_check_spec_layout"), work);
        run.arg0(arg0).args(args);
        finish(run)
    }

    #[test]
    fn the_script_frames_show_the_callers_argv0_and_a_binary_called_by_its_own_name_prints_the_logical_name(
    ) {
        let work = area("public-argv0");
        let args = failing_case(&work);
        let tail = "TypeError: unsupported operand type(s) for -: 'str' and 'int'\n";
        // the script's path is the caller's fact; the file need not exist (nothing reads it)
        let script = format!("{}/node/scripts/check_spec_layout.py", root_text(&work));
        let output = run(&work, &script, &args);
        assert_eq!(output.status.code(), Some(1));
        let stderr = String::from_utf8(output.stderr).expect("text");
        assert!(stderr.starts_with(&format!(
            "Traceback (most recent call last):\n  File \"{script}\", line 196, in <module>\n    main()\n  File \"{script}\", line 163, in main\n"
        )), "{stderr}");
        assert!(stderr.ends_with(tail));
        assert!(!stderr.contains("r34_original") && !stderr.contains("collector"));
        // another name is the virtual compatibility form: the logical name, never a path of this program
        for arg0 in ["check_spec_layout", "/somewhere/else.py", ""] {
            let output = run(&work, arg0, &args);
            let stderr = String::from_utf8(output.stderr).expect("text");
            assert!(stderr.starts_with("Traceback (most recent call last):\n  File \"check_spec_layout.py\", line 196, in <module>\n"), "{arg0}: {stderr}");
        }
        // the partial standard output before the exception is kept
        let output = run(&work, &script, &args);
        assert!(String::from_utf8_lossy(&output.stdout)
            .starts_with("[spec-layout] provenance=measured src=model.safetensors.index.json"));
    }

    #[test]
    fn an_exception_through_a_site_the_table_does_not_hold_keeps_the_plain_report_and_is_not_dressed_with_guessed_frames(
    ) {
        let work = area("public-open");
        put(
            &work.root,
            "model/model.safetensors.index.json",
            format!("{{\"weight_map\": {}}}", "9".repeat(5000)).as_bytes(),
        );
        let args = vec![
            "--model-dir".to_string(),
            format!("{}/model", root_text(&work)),
            "--method".to_string(),
            "mtp".to_string(),
        ];
        let output = run(&work, "check_spec_layout.py", &args);
        assert_eq!(output.status.code(), Some(1));
        let stderr = String::from_utf8(output.stderr).expect("text");
        assert!(
            stderr.starts_with("ValueError: Exceeds the limit (4300 digits)"),
            "{stderr}"
        );
        assert!(!stderr.contains("Traceback"));
    }
}

mod stream_cases {
    use super::*;
    use std::time::{Duration, Instant};

    const PAYLOAD: &[u8] = b"speculative-config: {\"method\":\"mtp\"}\n";

    /// Starts the program under test while a real writer holds the FIFO open after the first valid line: the program must return before the writer
    /// is released. The writer is released only afterwards (or at the deadline, which fails the check).
    fn with_open_fifo(
        label: &str,
        start: impl FnOnce(&Area, &Path) -> std::process::Child,
    ) -> (Area, std::process::Output) {
        let work = area(label);
        let fifo = work.root.join("config.fifo");
        assert!(std::process::Command::new("/usr/bin/mkfifo")
            .arg(&fifo)
            .status()
            .expect("mkfifo")
            .success());
        let (release, released) = std::sync::mpsc::channel::<()>();
        let writer_path = fifo.clone();
        let writer = std::thread::spawn(move || {
            // blocks until the program opens the read end, as a writer to a FIFO does
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .open(&writer_path)
                .expect("the writer opens");
            file.write_all(PAYLOAD).expect("the payload");
            file.flush().expect("flush");
            let _ = released.recv();
            drop(file);
        });
        let mut child = start(&work, &fifo);
        let deadline = Instant::now() + Duration::from_secs(10);
        let returned = loop {
            if child.try_wait().expect("poll").is_some() {
                break true;
            }
            if Instant::now() > deadline {
                break false;
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        if !returned {
            let _ = child.kill();
        }
        release.send(()).expect("release the writer");
        writer.join().expect("the writer ends");
        let output = child.wait_with_output().expect("collect");
        assert!(
            returned,
            "the program waited for the end of the stream instead of returning at the first match"
        );
        (work, output)
    }

    #[test]
    fn the_production_service_returns_at_the_first_match_while_the_writer_still_holds_the_stream_open(
    ) {
        let (_, output) = with_open_fifo("fifo-service", |work, fifo| {
            let (mut command, mut writer) = over_descriptor(
                env!("CARGO_BIN_EXE_upstream_layout_api"),
                &["evaluate-jsonl", "--input-fd", "3"],
                work,
            );
            let child = command.spawn().expect("start the service");
            let line = request(
                1,
                "read_method_from_config",
                &args_dto(&[str_dto(&fifo.to_string_lossy())]),
                "[\"dict\", []]",
                DEFAULT_GLOBALS,
            );
            writeln!(writer, "{line}").expect("the request");
            drop(writer);
            child
        });
        assert_eq!(output.status.code(), Some(0));
        let record = loads_text(&PyStr::from_surrogateescape(&output.stdout)).expect("a record");
        assert!(same_value(record_value(&record), &j("[\"str\", \"mtp\"]")));
    }

    #[test]
    fn the_fixture_api_returns_at_the_first_match_and_records_open_iterate_close_without_an_end() {
        let (work, output) = with_open_fifo("fifo-fixture", |work, fifo| {
            let spec = format!(
                "{{\"id\": \"fifo\", \"owner\": \"check_spec_layout\", \"kind\": \"api\", \"function\": \"read_method_from_config\", \"args_dto\": {}, \"kwargs_dto\": [\"dict\", []]}}",
                args_dto(&[str_dto(&fifo.to_string_lossy())])
            );
            std::fs::write(work.dir.join("spec.json"), spec).expect("a spec");
            let mut run = command(env!("CARGO_BIN_EXE_upstream_layout_fixture_api"), work);
            run.arg("case")
                .arg("--spec")
                .arg(work.dir.join("spec.json"))
                .arg("--root")
                .arg(&work.root)
                .arg("--control")
                .arg(work.dir.join("control.json"))
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            run.spawn().expect("start the fixture")
        });
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let control = read_control(&work.dir.join("control.json"));
        assert!(same_value(
            member(&control, "value"),
            &j("[\"str\", \"mtp\"]")
        ));
        let events: Vec<String> = items(member(&control, "trace"))
            .iter()
            .map(|row| text_of(member(row, "event")))
            .collect();
        assert_eq!(events, ["file_open", "file_iter_line", "file_close"]);
    }

    #[test]
    fn the_public_command_returns_at_the_first_match_while_the_writer_still_holds_the_stream_open()
    {
        let (_, output) = with_open_fifo("fifo-cli", |work, fifo| {
            let mut run = command(env!("CARGO_BIN_EXE_check_spec_layout"), work);
            run.arg("--model-dir")
                .arg(&work.root)
                .arg("--config")
                .arg(fifo)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            run.spawn().expect("start the command")
        });
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        // the method came from the FIFO's first line; the directory has no index file, so the command skips with its reason
        assert!(String::from_utf8_lossy(&output.stdout)
            .starts_with("[spec-layout] SKIP: index.json 부재"));
    }

    #[test]
    fn a_directory_is_refused_at_open_and_a_regular_file_is_still_read_chunk_by_chunk() {
        let work = area("open-phase");
        std::fs::create_dir(work.root.join("config.yaml")).expect("a directory");
        // CPython's `open()` of a directory is the IsADirectoryError of the open phase (an OSError: the method is None) and no read step follows
        let spec = |name: &str| {
            format!(
                "{{\"id\": \"d\", \"owner\": \"check_spec_layout\", \"kind\": \"api\", \"function\": \"read_method_from_config\", \"args_dto\": [\"list\", [[\"str\", \"{}/{name}\"]]], \"kwargs_dto\": [\"dict\", []]}}",
                root_text(&work)
            )
        };
        std::fs::write(work.dir.join("spec.json"), spec("config.yaml")).expect("a spec");
        let mut run = command(env!("CARGO_BIN_EXE_upstream_layout_fixture_api"), &work);
        run.arg("case")
            .arg("--spec")
            .arg(work.dir.join("spec.json"))
            .arg("--root")
            .arg(&work.root)
            .arg("--control")
            .arg(work.dir.join("control.json"));
        let output = finish(run);
        assert_eq!(output.status.code(), Some(0));
        let control = read_control(&work.dir.join("control.json"));
        assert!(same_value(member(&control, "value"), &j("[\"none\"]")));
        let events: Vec<String> = items(member(&control, "trace"))
            .iter()
            .map(|row| text_of(member(row, "event")))
            .collect();
        assert_eq!(events, ["file_open", "file_open_error"]);
    }
}

mod stage2_cases {
    use super::*;
    use std::cell::Cell;
    use upstream_runtime::host::Sink;
    use upstream_runtime::layout_files::{load_index_keys, read_num_nextn, LayoutWorld};
    use upstream_runtime::thrown::Done;

    /// A host that counts the questions "do you trace?" and keeps the events it is told with their field values.
    struct Counting {
        traces: bool,
        asked: Cell<usize>,
        told: Vec<(String, Vec<(String, PyValue)>)>,
    }

    impl Counting {
        fn new(traces: bool) -> Counting {
            Counting {
                traces,
                asked: Cell::new(0),
                told: Vec::new(),
            }
        }

        fn names(&self) -> Vec<&str> {
            self.told.iter().map(|(name, _)| name.as_str()).collect()
        }
    }

    impl Sink for Counting {
        fn out(&mut self, _: &PyStr) -> Done<()> {
            Ok(())
        }
        fn err(&mut self, _: &PyStr) -> Done<()> {
            Ok(())
        }
        fn parser_help(&mut self, _: &PyStr) {}
        fn parser_error(&mut self, _: &PyStr, _: &PyStr) {}
        fn tracing(&self) -> bool {
            self.asked.set(self.asked.get() + 1);
            self.traces
        }
        fn trace(&mut self, name: &str, fields: Vec<(&str, PyValue)>) {
            self.told.push((
                name.to_string(),
                fields
                    .into_iter()
                    .map(|(key, value)| (key.to_string(), value))
                    .collect(),
            ));
        }
    }

    fn run_index(work: &Area, host: &mut Counting) -> Done<PyValue> {
        let argv = [PyStr::new("check_spec_layout.py")];
        let mut world = LayoutWorld::new(host, &argv, None);
        load_index_keys(
            &mut world,
            &PyValue::str(&format!("{}/model", root_text(work))),
        )
    }

    fn run_nextn(work: &Area, host: &mut Counting) -> Done<PyValue> {
        let argv = [PyStr::new("check_spec_layout.py")];
        let mut world = LayoutWorld::new(host, &argv, None);
        read_num_nextn(
            &mut world,
            &PyValue::str(&format!("{}/model", root_text(work))),
        )
    }

    #[test]
    fn the_host_is_asked_once_per_event_site_and_the_traced_document_text_is_the_file_content() {
        let work = area("stage2-trace");
        let content = "{\"weight_map\": {\"a.b\": \"s\", \"c.d\": \"t\"}}";
        put(
            &work.root,
            "model/model.safetensors.index.json",
            content.as_bytes(),
        );
        let (mut silent, mut traced) = (Counting::new(false), Counting::new(true));
        let silent_value = run_index(&work, &mut silent).expect("a value");
        let traced_value = run_index(&work, &mut traced).expect("a value");
        assert!(same_value(&silent_value, &traced_value));
        // five event sites are reached (`isfile`, `file_open`, `file_read`, `file_read_return`, `file_close`): five questions, traced or not
        assert_eq!(silent.asked.get(), 5);
        assert_eq!(traced.asked.get(), 5);
        assert!(silent.told.is_empty());
        assert_eq!(
            traced.names(),
            [
                "isfile",
                "file_open",
                "file_read",
                "file_read_return",
                "file_close"
            ]
        );
        let (_, fields) = &traced.told[3];
        let keys: Vec<&str> = fields.iter().map(|(key, _)| key.as_str()).collect();
        assert_eq!(keys, ["path", "text"]);
        assert!(matches!(&fields[1].1, PyValue::Str(text) if text.equals(content)));
        assert!(
            matches!(&fields[0].1, PyValue::Str(path) if path.equals(&format!("{}/model/model.safetensors.index.json", root_text(&work))))
        );
    }

    #[test]
    fn the_events_and_their_order_after_a_wrong_type_member_are_the_same_with_the_member_moved_out()
    {
        let work = area("stage2-order");
        put(&work.root, "model/config.json", b"[1]");
        let mut host = Counting::new(true);
        let error = run_nextn(&work, &mut host).expect_err("a list has no `get`");
        assert_eq!(error.raised.class, "AttributeError");
        assert_eq!(
            error.raised.message.to_utf8().unwrap(),
            "'list' object has no attribute 'get'"
        );
        // the member step fails after the read, and the file is closed after it, as before
        assert_eq!(
            host.names(),
            ["file_open", "file_read", "file_read_return", "file_close"]
        );
        assert_eq!(host.asked.get(), 4);
    }

    #[test]
    fn the_member_is_the_documents_value_for_a_repeated_missing_nested_or_non_finite_member() {
        let work = area("stage2-members");
        let nextn = |text: &str| {
            put(&work.root, "model/config.json", text.as_bytes());
            record_value(&model_call(&work, "read_num_nextn")).clone()
        };
        // a repeated member keeps its last value; other members do not matter; a missing member is None
        assert!(same_value(&nextn("{\"a\": 1, \"num_nextn_predict_layers\": 1, \"b\": 2, \"num_nextn_predict_layers\": 3}"), &j("[\"int\", \"3\"]")));
        assert!(same_value(&nextn("{\"a\": 1}"), &j("[\"none\"]")));
        assert!(same_value(&nextn("{}"), &j("[\"none\"]")));
        // non-finite and nested values come back whole
        assert!(same_value(
            &nextn("{\"num_nextn_predict_layers\": NaN}"),
            &j("[\"float\", \"nan\"]")
        ));
        assert!(same_value(
            &nextn("{\"num_nextn_predict_layers\": -Infinity}"),
            &j("[\"float\", \"-inf\"]")
        ));
        assert!(same_value(
            &nextn("{\"num_nextn_predict_layers\": {\"x\": [1, {\"y\": 2}], \"x\": 5}}"),
            &j("[\"dict\", [[[\"str\", \"x\"], [\"int\", \"5\"]]]]")
        ));
        // the index: a repeated weight_map is the last one, extra members are ignored, a missing or falsy one is an empty key list
        let index = |text: &str| {
            put(
                &work.root,
                "model/model.safetensors.index.json",
                text.as_bytes(),
            );
            record_value(&model_call(&work, "load_index_keys")).clone()
        };
        let name = "[\"str\", \"model.safetensors.index.json\"]";
        assert!(same_value(
            &index("{\"meta\": {\"k\": 1}, \"weight_map\": {\"a\": 1}, \"weight_map\": {\"z\": 1, \"y\": 2}}"),
            &j(&format!("[\"tuple\", [[\"list\", [[\"str\", \"z\"], [\"str\", \"y\"]]], {name}]]"))
        ));
        for text in [
            "{}",
            "{\"weight_map\": null}",
            "{\"weight_map\": {}}",
            "{\"weight_map\": 0}",
            "{\"weight_map\": \"\"}",
        ] {
            assert!(
                same_value(
                    &index(text),
                    &j(&format!("[\"tuple\", [[\"list\", []], {name}]]"))
                ),
                "{text}"
            );
        }
        // a truthy weight_map that is no dict, and a document that is no dict, are the attribute errors of the source
        put(
            &work.root,
            "model/model.safetensors.index.json",
            b"{\"weight_map\": [1]}",
        );
        assert_eq!(
            exception_of(&model_call(&work, "load_index_keys")),
            (
                "AttributeError".to_string(),
                "'list' object has no attribute 'keys'".to_string()
            )
        );
        for (text, shown) in [
            ("null", "NoneType"),
            ("[]", "list"),
            ("5", "int"),
            ("true", "bool"),
            ("1.5", "float"),
            ("\"s\"", "str"),
        ] {
            put(
                &work.root,
                "model/model.safetensors.index.json",
                text.as_bytes(),
            );
            assert_eq!(
                exception_of(&model_call(&work, "load_index_keys")),
                (
                    "AttributeError".to_string(),
                    format!("'{shown}' object has no attribute 'get'")
                ),
                "{text}"
            );
        }
    }

    const INDEX_NAME: &str = "model.safetensors.index.json";

    fn index_text(work: &Area, text: &str) {
        put(
            &work.root,
            "model/model.safetensors.index.json",
            text.as_bytes(),
        );
    }

    fn expected_keys(keys: Vec<PyStr>) -> PyValue {
        PyValue::Tuple(vec![
            PyValue::List(keys.into_iter().map(PyValue::Str).collect()),
            PyValue::str(INDEX_NAME),
        ])
    }

    /// The `(keys, name)` the index must give for `weight_map` text and the keys that text spells (document order, first position).
    fn assert_index_keys(work: &Area, weight_map: &str, keys: Vec<PyStr>, what: &str) {
        index_text(work, &format!("{{\"weight_map\": {weight_map}}}"));
        let mut host = Counting::new(false);
        let got = run_index(work, &mut host).expect("a value");
        let want = expected_keys(keys);
        assert!(
            same_value(&got, &want),
            "{what}: want {}\n got {}",
            want.repr(),
            got.repr()
        );
    }

    #[test]
    fn owned_index_keys_keep_document_order_sizes_and_escaped_empty_long_and_lone_surrogate_keys() {
        let work = area("stage3-keys");
        for size in [1usize, 16, 17, 64, 2048] {
            let names: Vec<String> = (0..size).map(|n| format!("layers.{n}.weight")).collect();
            let body: Vec<String> = names
                .iter()
                .map(|name| format!("\"{name}\": \"s\""))
                .collect();
            assert_index_keys(
                &work,
                &format!("{{{}}}", body.join(", ")),
                names.iter().map(|name| PyStr::new(name)).collect(),
                &format!("size {size}"),
            );
        }
        let long = "L".repeat(4096);
        assert_index_keys(
            &work,
            &format!(
                "{{\"a\\n\\t\\\"\\u00e9\": 1, \"\": 2, \"\\ud800\": 3, \"{long}\": 4, \"\\ud83d\\ude00\": 5, \"z\": 6}}"
            ),
            vec![
                PyStr::from_code_points(vec![0x61, 0x0a, 0x09, 0x22, 0xe9]),
                PyStr::from_code_points(vec![]),
                PyStr::from_code_points(vec![0xd800]),
                PyStr::new(&long),
                PyStr::from_code_points(vec![0x1f600]),
                PyStr::new("z"),
            ],
            "escaped, empty, lone surrogate, long",
        );
        // duplicates: the first position, once (the last value is dropped with the values)
        assert_index_keys(
            &work,
            "{\"b\": 1, \"a\": 2, \"b\": 3, \"c\": 4, \"a\": 5}",
            vec![PyStr::new("b"), PyStr::new("a"), PyStr::new("c")],
            "duplicates",
        );
        // values of any shape are dropped without effect on the keys
        assert_index_keys(
            &work,
            "{\"x\": {\"n\": [1, {\"m\": null}]}, \"y\": [NaN, 1.5], \"w\": \"s\"}",
            vec![PyStr::new("x"), PyStr::new("y"), PyStr::new("w")],
            "nested values",
        );
    }

    #[test]
    fn owned_index_keys_empty_falsy_and_missing_members_give_an_empty_list() {
        let work = area("stage3-falsy");
        for text in [
            "{}",
            "{\"weight_map\": {}}",
            "{\"weight_map\": null}",
            "{\"weight_map\": 0}",
            "{\"weight_map\": false}",
            "{\"weight_map\": \"\"}",
            "{\"weight_map\": []}",
            "{\"weight_map\": 0.0}",
        ] {
            index_text(&work, text);
            let mut host = Counting::new(false);
            let got = run_index(&work, &mut host).expect("a value");
            assert!(same_value(&got, &expected_keys(vec![])), "{text}");
        }
    }

    #[test]
    fn owned_index_keys_wrong_type_members_raise_the_same_error_stamp_and_events() {
        let work = area("stage3-wrong");
        for (text, shown) in [
            ("[1]", "list"),
            ("5", "int"),
            ("\"s\"", "str"),
            ("true", "bool"),
            ("1.5", "float"),
        ] {
            index_text(&work, &format!("{{\"weight_map\": {text}}}"));
            let mut host = Counting::new(true);
            let error = run_index(&work, &mut host).expect_err("no `keys`");
            assert_eq!(error.raised.class, "AttributeError", "{text}");
            assert_eq!(
                error.raised.message.to_utf8().unwrap(),
                format!("'{shown}' object has no attribute 'keys'"),
                "{text}"
            );
            // the file is read and closed before the member is used; the order is the former one
            assert_eq!(
                host.names(),
                [
                    "isfile",
                    "file_open",
                    "file_read",
                    "file_read_return",
                    "file_close"
                ],
                "{text}"
            );
            assert_eq!(host.asked.get(), 5, "{text}");
        }
        // the same errors through the public path carry the `IndexKeys` stamp the former path put
        index_text(&work, "{\"weight_map\": [1]}");
        assert_eq!(
            exception_of(&model_call(&work, "load_index_keys")),
            (
                "AttributeError".to_string(),
                "'list' object has no attribute 'keys'".to_string()
            )
        );
        index_text(&work, "{\"weight_map\": \"s\"}");
        assert_eq!(
            exception_of(&model_call(&work, "load_index_keys")),
            (
                "AttributeError".to_string(),
                "'str' object has no attribute 'keys'".to_string()
            )
        );
    }

    #[test]
    fn owned_index_keys_events_and_the_host_questions_are_those_of_a_success() {
        let work = area("stage3-events");
        index_text(&work, "{\"weight_map\": {\"a\": 1, \"b\": 2, \"c\": 3}}");
        let (mut silent, mut traced) = (Counting::new(false), Counting::new(true));
        let silent_value = run_index(&work, &mut silent).expect("a value");
        let traced_value = run_index(&work, &mut traced).expect("a value");
        assert!(same_value(&silent_value, &traced_value));
        assert!(same_value(
            &traced_value,
            &expected_keys(vec![PyStr::new("a"), PyStr::new("b"), PyStr::new("c")])
        ));
        assert_eq!(silent.asked.get(), 5);
        assert_eq!(traced.asked.get(), 5);
        assert!(silent.told.is_empty());
        assert_eq!(
            traced.names(),
            [
                "isfile",
                "file_open",
                "file_read",
                "file_read_return",
                "file_close"
            ]
        );
    }

    #[test]
    fn a_rejected_index_file_reports_the_decoder_fields_once_and_a_valid_one_is_decoded_with_universal_newlines(
    ) {
        use upstream_runtime::thrown::Ctor;
        let work = area("a1-decode");
        // the three classes of rejection `bytes.decode("utf-8")` reports: (bytes, message, start, end, reason)
        let rejected: [(&[u8], &str, usize, usize, &str); 3] = [
            (
                b"{\"weight_map\": {\"a\": 1}}\xff",
                "'utf-8' codec can't decode byte 0xff in position 24: invalid start byte",
                24,
                25,
                "invalid start byte",
            ),
            (
                b"{\"w\": \"\xc3(\"}",
                "'utf-8' codec can't decode byte 0xc3 in position 7: invalid continuation byte",
                7,
                8,
                "invalid continuation byte",
            ),
            (
                b"{\"w\": \"\xe2\x82",
                "'utf-8' codec can't decode bytes in position 7-8: unexpected end of data",
                7,
                9,
                "unexpected end of data",
            ),
        ];
        for (bytes, message, start, end, reason) in rejected {
            put(&work.root, "model/model.safetensors.index.json", bytes);
            let mut host = Counting::new(true);
            let error = run_index(&work, &mut host).expect_err("the file is not UTF-8");
            assert_eq!(error.raised.class, "UnicodeDecodeError");
            assert_eq!(error.raised.message.to_utf8().unwrap(), message);
            match &error.ctors[0] {
                Some(Ctor::Decode {
                    encoding,
                    object,
                    start: from,
                    end: to,
                    reason: why,
                }) => {
                    assert_eq!(encoding, "utf-8");
                    assert_eq!(
                        object.as_slice(),
                        bytes,
                        "the whole file is the decoder's object"
                    );
                    assert_eq!((*from, *to, why.as_str()), (start, end, reason));
                }
                other => panic!("no decode constructor: {other:?}"),
            }
            // the event of the failed read replaces `file_read_return`, and the file is still closed
            assert_eq!(
                host.names(),
                [
                    "isfile",
                    "file_open",
                    "file_read",
                    "file_read_error",
                    "file_close"
                ]
            );
            let (_, fields) = &host.told[3];
            let keys: Vec<&str> = fields.iter().map(|(key, _)| key.as_str()).collect();
            assert_eq!(keys, ["path", "type", "message"]);
            assert!(
                matches!(&fields[1].1, PyValue::Str(kind) if kind.equals("UnicodeDecodeError"))
            );
            assert!(matches!(&fields[2].1, PyValue::Str(text) if text.equals(message)));
            assert_eq!(host.asked.get(), 5);
        }
        // valid UTF-8 with CRLF line ends and a non-ASCII key: one decode, universal newlines, whole text traced
        let crlf = "{\"weight_map\":\r\n{\"a\": 1,\r\n\"\u{e9}\": 2}}\r\n";
        put(
            &work.root,
            "model/model.safetensors.index.json",
            crlf.as_bytes(),
        );
        let mut host = Counting::new(true);
        let value = run_index(&work, &mut host).expect("a value");
        assert!(same_value(
            &value,
            &expected_keys(vec![PyStr::new("a"), PyStr::new("\u{e9}")])
        ));
        let (name, fields) = &host.told[3];
        assert_eq!(name, "file_read_return");
        assert!(
            matches!(&fields[1].1, PyValue::Str(text) if text.equals("{\"weight_map\":\n{\"a\": 1,\n\"\u{e9}\": 2}}\n"))
        );
    }
}

mod owned_dto_cases {
    use super::*;
    use agent_runtime_support::pyvalue::{DictKey, NonStringKey, Opaque, Typed};
    use std::sync::Arc;
    use upstream_runtime::dto::{dict_of, encode, encode_owned};

    fn text(value: &str) -> PyValue {
        PyValue::str(value)
    }

    fn opaque(type_name: &'static str, repr: &str, typed: Typed) -> PyValue {
        PyValue::Opaque(Opaque {
            type_name,
            repr: repr.to_string(),
            truthy: true,
            typed,
            identity: None,
        })
    }

    /// Values chosen from the type system's own cases (independent of how the owned encoder is written).
    fn values() -> Vec<(&'static str, PyValue)> {
        let surrogate = PyValue::Str(PyStr::from_code_points(vec![
            0xD800, 0x61, 0xDFFF, 0x1F600, 0xE9,
        ]));
        let mut keyless = PyDict::new();
        keyless.insert_key(
            DictKey::Other(NonStringKey {
                identity: "num:1".to_string(),
                repr: "1".to_string(),
                value: None,
            }),
            text("v"),
        );
        vec![
            ("empty list", PyValue::List(vec![])),
            ("empty tuple", PyValue::Tuple(vec![])),
            ("plain str", text("é")),
            ("surrogates and astral", surrogate.clone()),
            (
                "nested str/list/tuple",
                PyValue::Tuple(vec![
                    PyValue::List(vec![text("a"), text(""), surrogate.clone()]),
                    PyValue::List(vec![PyValue::List(vec![PyValue::Tuple(vec![text(
                        "deep",
                    )])])]),
                    text("name"),
                ]),
            ),
            (
                "scalars among fresh containers",
                PyValue::List(vec![
                    PyValue::int(10).clone(),
                    PyValue::Float(f64::NAN),
                    PyValue::Float(-0.0),
                    PyValue::Float(f64::INFINITY),
                    PyValue::Bool(true),
                    PyValue::None,
                    PyValue::Bytes(vec![0, 255, 16]),
                    text("tail"),
                ]),
            ),
            (
                "dict inside a list",
                PyValue::List(vec![
                    text("k"),
                    dict_of(vec![
                        ("a", PyValue::List(vec![text("x")])),
                        ("b", PyValue::Tuple(vec![])),
                    ]),
                ]),
            ),
            (
                "shared carrier",
                PyValue::Shared(Arc::new(PyValue::List(vec![text("s")]))),
            ),
            (
                "shared member",
                PyValue::List(vec![
                    PyValue::Shared(Arc::new(text("é"))),
                    PyValue::Shared(Arc::new(PyValue::Tuple(vec![text("t")]))),
                ]),
            ),
            (
                "set (canonical order) in a tuple",
                PyValue::Tuple(vec![
                    opaque(
                        "set",
                        "{...}",
                        Typed::Set(vec![text("b"), text("a"), PyValue::int(2)]),
                    ),
                    text("z"),
                ]),
            ),
            (
                "date and datetime",
                PyValue::List(vec![
                    opaque("datetime.date", "d", Typed::Date("2026-01-02".to_string())),
                    opaque(
                        "datetime.datetime",
                        "dt",
                        Typed::DateTime("2026-01-02T03:04:05".to_string()),
                    ),
                ]),
            ),
            (
                "cyclic marker refused",
                opaque("list", "[...]", Typed::Plain),
            ),
            // R48 Q: the reader's marker of a self-containing value is `Typed::Cycle`; it must give the SAME refusal as the cyclic `Plain` repr
            (
                "reader cycle marker (list) refused",
                opaque("list", "[...]", Typed::Cycle(1)),
            ),
            (
                "reader cycle marker (dict) refused",
                opaque("dict", "{...}", Typed::Cycle(2)),
            ),
            (
                "reader cycle marker (set) refused",
                opaque("set", "set(...)", Typed::Cycle(1)),
            ),
            (
                "first refusal in member order: undeclared before a reader cycle marker",
                PyValue::List(vec![
                    opaque("object", "<x>", Typed::Plain),
                    opaque("list", "[...]", Typed::Cycle(1)),
                ]),
            ),
            (
                "first refusal in member order: reader cycle marker before an undeclared type",
                PyValue::List(vec![
                    opaque("list", "[...]", Typed::Cycle(1)),
                    opaque("object", "<x>", Typed::Plain),
                ]),
            ),
            (
                "undeclared type refused",
                opaque("object", "<x>", Typed::Plain),
            ),
            (
                "first refusal in member order",
                PyValue::List(vec![
                    text("ok"),
                    opaque("object", "<x>", Typed::Plain),
                    opaque("list", "[...]", Typed::Plain),
                ]),
            ),
            (
                "refusal under a tuple under a list",
                PyValue::List(vec![PyValue::Tuple(vec![
                    text("ok"),
                    opaque("dict", "{...}", Typed::Plain),
                ])]),
            ),
            (
                "dict key without its typed value",
                PyValue::List(vec![PyValue::Dict(keyless)]),
            ),
        ]
    }

    #[test]
    fn the_owned_encoder_gives_the_borrowed_encoders_dto_or_refusal_for_every_kind_of_value() {
        for (name, value) in values() {
            let borrowed = encode(&value);
            let owned = encode_owned(value.clone());
            match (&borrowed, &owned) {
                (Ok(want), Ok(got)) => {
                    assert!(
                        same_value(want, got),
                        "{name}: {} vs {}",
                        want.repr(),
                        got.repr()
                    );
                    // the compact ASCII JSON is the DTO's canonical text: types, order, escapes and surrogates must agree character by character
                    assert_eq!(
                        want.dumps_ascii_tight().unwrap(),
                        got.dumps_ascii_tight().unwrap(),
                        "{name}"
                    );
                }
                (Err(want), Err(got)) => assert_eq!(want, got, "{name}"),
                _ => panic!(
                    "{name}: one encoder refused and the other did not: {borrowed:?} / {owned:?}"
                ),
            }
            // the borrowed encoder is unchanged and still works on the retained value
            assert_eq!(encode(&value), borrowed, "{name}");
        }
    }

    /// Through the ACTUAL loader (`safe_load_objects`, no Original cohort replay): a document whose anchor contains itself loads with the R48 Q
    /// cycle marker, and the DTO refuses it with the exact R33 wording (borrowed and owned encoders alike), also inside a container.
    #[test]
    fn a_self_containing_yaml_alias_loaded_by_the_actual_loader_gets_the_cyclic_refusal() {
        use agent_runtime_support::pyyaml::safe_load_objects;
        for (document, marker_kind) in [("&a [*a]\n", "list"), ("&a {k: *a}\n", "dict")] {
            let value = safe_load_objects(document.as_bytes(), &PyStr::new("cycle.yaml"))
                .unwrap_or_else(|e| {
                    panic!(
                        "{document:?} did not load: {}",
                        e.message.to_utf8().unwrap_or_default()
                    )
                });
            fn first_cycle(value: &PyValue) -> Option<&'static str> {
                match value.peel() {
                    PyValue::Opaque(o) if o.cycle_up().is_some() => Some(o.type_name),
                    PyValue::List(items) | PyValue::Tuple(items) => {
                        items.iter().find_map(first_cycle)
                    }
                    PyValue::Dict(map) => map.iter().find_map(|(_, v)| first_cycle(v)),
                    _ => None,
                }
            }
            assert_eq!(
                first_cycle(&value),
                Some(marker_kind),
                "{document:?}: the loader must hand over the reader's cycle marker"
            );
            assert_eq!(
                encode(&value).unwrap_err(),
                "cyclic value outside R33 DTO profile",
                "{document:?} borrowed"
            );
            assert_eq!(
                encode_owned(value.clone()).unwrap_err(),
                "cyclic value outside R33 DTO profile",
                "{document:?} owned"
            );
            let wrapped = PyValue::List(vec![text("ok"), value]);
            assert_eq!(
                encode(&wrapped).unwrap_err(),
                "cyclic value outside R33 DTO profile",
                "{document:?} inside a list"
            );
        }
        // a plain alias (not self-containing) is its value and stays inside the profile
        let plain = safe_load_objects(b"a: &x [1, 2]\nb: *x\n", &PyStr::new("alias.yaml")).unwrap();
        assert!(encode(&plain).is_ok());
    }

    #[test]
    fn the_owned_dto_has_the_declared_tags_text_and_refusals() {
        let value = PyValue::Tuple(vec![
            PyValue::List(vec![text("a"), text("\u{e9}")]),
            text("name"),
        ]);
        // compact ASCII JSON escapes the non-ASCII character as a backslash, `u00e9`
        let want = format!(
            r#"["tuple",[["list",[["str","a"],["str","{slash}u00e9"]]],["str","name"]]]"#,
            slash = '\\'
        );
        assert_eq!(
            encode_owned(value).unwrap().dumps_ascii_tight().unwrap(),
            want
        );
        assert_eq!(
            encode_owned(PyValue::List(vec![
                text("x"),
                opaque("object", "<x>", Typed::Plain)
            ]))
            .unwrap_err(),
            "undeclared DTO type"
        );
        assert_eq!(
            encode_owned(opaque("list", "[...]", Typed::Plain)).unwrap_err(),
            "cyclic value outside R33 DTO profile"
        );
        for (kind, marker) in [("list", "[...]"), ("dict", "{...}"), ("set", "set(...)")] {
            assert_eq!(
                encode_owned(opaque(kind, marker, Typed::Cycle(1))).unwrap_err(),
                "cyclic value outside R33 DTO profile",
                "{kind}"
            );
        }
        // member order is kept: the FIRST refusal wins, whichever kind of refusal it is
        assert_eq!(
            encode_owned(PyValue::List(vec![
                opaque("object", "<x>", Typed::Plain),
                opaque("list", "[...]", Typed::Cycle(1)),
            ]))
            .unwrap_err(),
            "undeclared DTO type"
        );
        assert_eq!(
            encode_owned(PyValue::List(vec![
                opaque("list", "[...]", Typed::Cycle(1)),
                opaque("object", "<x>", Typed::Plain),
            ]))
            .unwrap_err(),
            "cyclic value outside R33 DTO profile"
        );
        let empty = encode_owned(PyValue::List(vec![])).unwrap();
        assert_eq!(empty.dumps_ascii_tight().unwrap(), r#"["list",[]]"#);
    }

    fn inner_text_pointer(dto: &PyValue) -> *const u32 {
        // ["list", [["str", <text>]]]
        let PyValue::List(parts) = dto else {
            panic!("tag")
        };
        let PyValue::List(members) = &parts[1] else {
            panic!("members")
        };
        let PyValue::List(tag) = &members[0] else {
            panic!("member tag")
        };
        let PyValue::Str(shown) = &tag[1] else {
            panic!("text")
        };
        shown.code_points().as_ptr()
    }

    #[test]
    fn a_fresh_text_is_moved_into_the_owned_dto_and_copied_by_the_borrowed_one() {
        let make = || {
            PyValue::List(vec![PyValue::Str(PyStr::from_code_points(
                "a long enough key for its own buffer"
                    .chars()
                    .map(u32::from)
                    .collect(),
            ))])
        };
        let value = make();
        let PyValue::List(items) = &value else {
            unreachable!()
        };
        let PyValue::Str(source) = &items[0] else {
            unreachable!()
        };
        let original = source.code_points().as_ptr();
        let borrowed = encode(&value).unwrap();
        assert_ne!(
            inner_text_pointer(&borrowed),
            original,
            "the borrowed encoder copies the text"
        );
        let moved_from = make();
        let PyValue::List(items) = &moved_from else {
            unreachable!()
        };
        let PyValue::Str(source) = &items[0] else {
            unreachable!()
        };
        let before = source.code_points().as_ptr();
        let owned = encode_owned(moved_from).unwrap();
        assert_eq!(
            inner_text_pointer(&owned),
            before,
            "the owned encoder moves the buffer into its tag"
        );
        assert!(same_value(&borrowed, &owned));
    }
}
