//! Checks of the `resolve_build_track` owner (R33). Nothing here runs Python or reaches a network; every program runs over an owned root with
//! the working directory on that root (the manifests are relative paths).
//!
//! * `reference_cases`: the 280 saved API calls and 34 saved command lines, whole (value, exception chain with constructors, ordered file trace,
//!   streams, status, globals, argv, the file tree before and after), through the fixture programs, the production service, the production event
//!   service with a parent that performs the writes and the import, and the production command.
//! * `contract_cases`: constants, `globals`, refusals, binding errors, source scan.
//! * `new_cases`: behaviour the saved cases do not pin (chunked reads and decoding errors, the typed value DTO, the import timing and its
//!   exceptions, the event protocol's refusals, stdout failures), expectations derived from the original source and CPython 3.12.
//! * `bench_cases`: the frozen benchmark.

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
        .join("upstream_reference")
        .join(relative);
    let bytes = std::fs::read(path).expect("saved data");
    loads_text(&PyStr::from_surrogateescape(&bytes)).expect("saved JSON")
}

fn j(text: &str) -> PyValue {
    loads_text(&PyStr::new(text)).expect("JSON")
}

fn jstr(text: &str) -> String {
    PyValue::str(text).dumps_ascii().expect("a JSON string")
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
    let held = PrivateDirectory::create(&format!("upstream-{name}-")).expect("a private directory");
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
fn command(program: &str, area: &Area, flat: bool) -> Command {
    let mut command = Command::new(program);
    command.env_clear().current_dir(&area.root);
    if flat {
        command.env("UPSTREAM_RUNTIME_YAML_PROFILE", "flat");
    }
    command
}

fn finish(mut command: Command, stdin: Option<&[u8]>) -> std::process::Output {
    command
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("start the program");
    if let Some(bytes) = stdin {
        let mut pipe = child.stdin.take().expect("stdin pipe");
        pipe.write_all(bytes).expect("stdin bytes");
    }
    child.wait_with_output().expect("wait for the program")
}

/// Starts `program args...` the way a caller passes a protocol descriptor (`pass_fds`): the pipe the caller writes to is descriptor 3 of the
/// program and descriptor 0 is the file `stdin` of the area.
fn over_descriptor(
    program: &str,
    args: &[&str],
    area: &Area,
    stdin: &[u8],
    flat: bool,
) -> (Command, std::io::PipeWriter) {
    let (reader, writer) = std::io::pipe().expect("a pipe");
    let zero = area.dir.join("stdin");
    std::fs::write(&zero, stdin).expect("a stdin file");
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
    if flat {
        command.env("UPSTREAM_RUNTIME_YAML_PROFILE", "flat");
    }
    (command, writer)
}

fn serve_lines(area: &Area, lines: &[String], flat: bool) -> (std::process::Output, Vec<PyValue>) {
    let (mut command, mut writer) = over_descriptor(
        env!("CARGO_BIN_EXE_upstream_api"),
        &["evaluate-jsonl", "--input-fd", "3"],
        area,
        b"",
        flat,
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

fn request(id: i64, operation: &str, args: &str, kwargs: &str, globals: &str) -> String {
    format!(
        "{{\"id\": {id}, \"operation\": \"resolve_build_track.{operation}\", \"args\": {args}, \"kwargs\": {kwargs}, \"globals\": {globals}}}"
    )
}

fn call1(area: &Area, flat: bool, operation: &str, args: &str) -> PyValue {
    let (output, mut records) = serve_lines(
        area,
        &[request(
            1,
            operation,
            args,
            "{}",
            "{\"argv\": [\"resolve_build_track.sh\"]}",
        )],
        flat,
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

// ── the event parent: the caller that owns the streams and the import ────────────────────────────

fn ok_ack(sequence: i64) -> String {
    format!("{{\"event\": \"write_ack\", \"sequence\": {sequence}, \"kind\": \"ok\"}}")
}

/// The acknowledgement of an `import yaml` against an installation with or without PyYAML 6.0.3.
fn import_ack(sequence: i64, flat: bool) -> String {
    if flat {
        format!(
            "{{\"event\": \"yaml_import_ack\", \"sequence\": {sequence}, \"kind\": \"exception\", \"exception_chain\": [{{\"type\": \"ModuleNotFoundError\", \"module\": \"builtins\", \"message\": \"No module named 'yaml'\", \"suppress_context\": false, \"relation\": null, \"builtin_bases\": [\"ModuleNotFoundError\", \"ImportError\", \"Exception\", \"BaseException\"]}}]}}"
        )
    } else {
        format!("{{\"event\": \"yaml_import_ack\", \"sequence\": {sequence}, \"kind\": \"available\", \"version\": \"6.0.3\"}}")
    }
}

/// What the parent saw and did.
struct Session {
    /// `(sequence, stream, text)` of every write event, in order.
    writes: Vec<(i64, String, PyStr)>,
    /// The sequences of the `yaml_import` events, and the write count before each.
    imports: Vec<(i64, usize)>,
    result: Option<PyValue>,
    status: Option<i32>,
    stderr: String,
    stray: Vec<String>,
}

/// Runs one request through `evaluate-events`; `ack_write(sequence, stream, text)` and `ack_import(sequence)` are the parent's answer lines (`None`: it goes away).
fn session(
    area: &Area,
    request_line: &str,
    mut ack_write: impl FnMut(i64, &str, &PyStr) -> Option<String>,
    mut ack_import: impl FnMut(i64) -> Option<String>,
) -> Session {
    let (mut command, mut writer) = over_descriptor(
        env!("CARGO_BIN_EXE_upstream_api"),
        &["evaluate-events", "--input-fd", "3"],
        area,
        b"",
        false,
    );
    let mut child = command.spawn().expect("start the program");
    drop(command);
    writeln!(writer, "{request_line}").expect("the request");
    writer.flush().expect("flush");
    let mut writer = Some(writer);
    let mut lines = BufReader::new(child.stdout.take().expect("events"));
    let mut seen = Session {
        writes: Vec::new(),
        imports: Vec::new(),
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
        let answer = match text_of(member(&event, "event")).as_str() {
            "write" => {
                let sequence = int_of(member(&event, "sequence"));
                let stream = text_of(member(&event, "stream"));
                let PyValue::Str(written) = member(&event, "text") else {
                    panic!("text")
                };
                seen.writes
                    .push((sequence, stream.clone(), written.clone()));
                Some(ack_write(sequence, &stream, written))
            }
            "yaml_import" => {
                let sequence = int_of(member(&event, "sequence"));
                seen.imports.push((sequence, seen.writes.len()));
                Some(ack_import(sequence))
            }
            "result" => {
                seen.result = Some(member(&event, "record").clone());
                None
            }
            other => {
                seen.stray.push(other.to_string());
                None
            }
        };
        match answer {
            Some(Some(line)) => {
                if let Some(pipe) = writer.as_mut() {
                    let _ = writeln!(pipe, "{line}").and_then(|()| pipe.flush());
                }
            }
            Some(None) => drop(writer.take()),
            None => {}
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
    flat: bool,
    before: Tree,
}

fn prepare(case: &PyValue, prefix: &str) -> Prepared {
    let request = member(case, "request");
    let id = text_of(member(request, "id"));
    let work = area(&format!("{prefix}-{id}"));
    build_files(member(request, "files"), &work.root);
    let before = snapshot(&work.root);
    let flat = text_of(member(request, "profile")) == "flat";
    Prepared {
        root: root_text(&work),
        area: work,
        flat,
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
        vec!["resolve_build_track.sh".to_string()]
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
                env!("CARGO_BIN_EXE_upstream_fixture_cli")
            } else {
                env!("CARGO_BIN_EXE_upstream_fixture_api")
            },
            &work.area,
            false,
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
        let output = finish(run, None);
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
            let want_err = replace_bytes(
                &hex_decode(&text_of(member(case, "stderr_hex"))),
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
            if output.stderr != want_err {
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
    fn the_280_saved_calls_are_reproduced_whole_with_values_exceptions_the_trace_the_streams_and_the_globals(
    ) {
        let bundle = load("bundle.json");
        let cases = items(member(&bundle, "api"));
        assert_eq!(cases.len(), 280);
        all_or_fail(
            cases
                .iter()
                .flat_map(|case| fixture_problems(case, false))
                .collect(),
        );
    }

    #[test]
    fn the_34_saved_command_lines_end_as_the_original_did_with_their_trace_streams_status_and_files(
    ) {
        let bundle = load("bundle.json");
        let cases = items(member(&bundle, "cli"));
        assert_eq!(cases.len(), 34);
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
                "resolve_build_track.{}",
                text_of(member(request_row, "function"))
            )),
        );
        line.set("args", rebase_all(member(request_row, "args"), &work.root));
        line.set(
            "kwargs",
            rebase_all(member(request_row, "kwargs"), &work.root),
        );
        line.set("globals", dict(vec![("argv", PyValue::List(argv))]));
        PyValue::Dict(line).dumps_ascii().expect("a request")
    }

    fn service_problems(case: &PyValue) -> Vec<String> {
        let id = text_of(member(member(case, "request"), "id"));
        let work = prepare(case, "service");
        let (output, records) = serve_lines(&work.area, &[request_line(case, &work)], work.flat);
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
    fn the_280_saved_calls_agree_through_the_production_service() {
        let bundle = load("bundle.json");
        all_or_fail(
            items(member(&bundle, "api"))
                .iter()
                .flat_map(service_problems)
                .collect(),
        );
    }

    /// Whether the original reached its `import yaml` in this case: `load_manifest` always does, `main` when its manifest path is a file.
    fn imports_expected(case: &PyValue) -> usize {
        let request = member(case, "request");
        match text_of(member(request, "function")).as_str() {
            "load_manifest" => 1,
            "main" => usize::from(items(member(member(case, "original"), "trace")).iter().any(
                |step| {
                    text_of(member(step, "event")) == "isfile"
                        && matches!(member(step, "result"), PyValue::Bool(true))
                },
            )),
            _ => 0,
        }
    }

    fn events_problems(case: &PyValue) -> Vec<String> {
        let id = text_of(member(member(case, "request"), "id"));
        let work = prepare(case, "events");
        let flat = work.flat;
        let seen = session(
            &work.area,
            &request_line(case, &work),
            accept_all,
            |sequence| Some(import_ack(sequence, flat)),
        );
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
        // what the parent was told to write is what the original printed, per stream, in the exact write boundaries (`print` is two writes)
        for (stream, key) in [("stdout", "stdout"), ("stderr", "stderr")] {
            let joined: Vec<u32> = seen
                .writes
                .iter()
                .filter(|(_, name, _)| name == stream)
                .flat_map(|(_, _, text)| text.code_points().to_vec())
                .collect();
            let PyValue::Str(want) = member(&original, key) else {
                panic!("a stream")
            };
            if joined != want.code_points() {
                problems.push(format!("{stream} writes differ"));
            }
        }
        for (at, (sequence, _, _)) in seen.writes.iter().enumerate() {
            let _ = at;
            if *sequence < 0 {
                problems.push("a negative sequence".to_string());
            }
        }
        // the import was asked exactly where the original reached it
        if seen.imports.len() != imports_expected(case) {
            problems.push(format!(
                "{} import event(s), the original imported {} time(s)",
                seen.imports.len(),
                imports_expected(case)
            ));
        }
        problems.extend(tree_problems(&work, &id));
        problems
            .into_iter()
            .map(|problem| format!("{id}: {problem}"))
            .collect()
    }

    #[test]
    fn the_280_saved_calls_agree_through_the_event_service_with_a_parent_that_performs_the_writes_and_the_import(
    ) {
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
        // the binary is started under another name, from anywhere: the program name in every text is the original's
        let mut run = command(
            env!("CARGO_BIN_EXE_resolve_build_track"),
            &work.area,
            work.flat,
        );
        run.args(&args);
        let output = finish(run, None);
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
        if output.stderr != want_err {
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

    #[test]
    fn the_34_saved_command_lines_give_the_same_streams_and_status_through_the_production_program_without_python_or_a_path(
    ) {
        let bundle = load("bundle.json");
        all_or_fail(
            items(member(&bundle, "cli"))
                .iter()
                .flat_map(program_problems)
                .collect(),
        );
    }

    /// The three originals collected after the source review (N1 UTC-equivalent timestamp keys, N1 the same in a `!!set`, N2 the nested
    /// zero-second datetime through `main`): their saved records, never a made answer.
    #[test]
    fn the_3_saved_supplement_calls_agree_through_the_fixture_the_production_service_and_the_event_service(
    ) {
        let bundle = load("supplement.json");
        let cases = items(member(&bundle, "api"));
        let ids: Vec<String> = cases
            .iter()
            .map(|case| text_of(member(member(case, "request"), "id")))
            .collect();
        assert_eq!(ids, ["api-yaml-000", "api-yaml-001", "api-yaml-002"]);
        assert_eq!(items(member(&bundle, "cli")).len(), 0);
        let mut problems = Vec::new();
        for case in cases {
            problems.extend(fixture_problems(case, false));
            problems.extend(service_problems(case));
            problems.extend(events_problems(case));
        }
        all_or_fail(problems);
    }

    /// Whether a file of a saved case holds a `dddd-dd-dd` run (a date or timestamp text): the cases the identity and `repr` repair can touch.
    fn holds_date_text(case: &PyValue) -> bool {
        let PyValue::Dict(files) = member(member(case, "request"), "files") else {
            return false;
        };
        files.iter().any(|(_, file)| {
            let PyValue::Dict(spec) = file else {
                return false;
            };
            let Some(PyValue::Str(hex)) = spec.get("hex") else {
                return false;
            };
            let bytes = hex_decode(&hex.to_utf8().unwrap_or_default());
            bytes.windows(10).any(|w| {
                w.iter().enumerate().all(|(at, byte)| match at {
                    4 | 7 => *byte == b'-',
                    _ => byte.is_ascii_digit(),
                })
            })
        })
    }

    /// The affected selection of the 280 saved calls (their files hold date text): the other saved calls hold no timestamp, so the repair
    /// cannot reach them; the whole 280 and 34 are not repeated for this change.
    #[test]
    fn the_saved_calls_whose_files_hold_date_text_still_agree_through_all_three_routes() {
        let bundle = load("bundle.json");
        let selected: Vec<&PyValue> = items(member(&bundle, "api"))
            .iter()
            .filter(|case| holds_date_text(case))
            .collect();
        // derived from the saved files, not listed by hand
        assert!(!selected.is_empty());
        assert!(selected.len() < 280);
        let mut problems = Vec::new();
        for case in &selected {
            problems.extend(fixture_problems(case, false));
            problems.extend(service_problems(case));
            problems.extend(events_problems(case));
        }
        all_or_fail(problems);
        let cli = items(member(&bundle, "cli"));
        assert!(!cli.iter().any(holds_date_text));
    }

    /// The original collected after the second source review (N2b: a nested datetime with the signed zero offset `+00:00` through `main`):
    /// its saved record, never a made answer.
    #[test]
    fn the_saved_signed_zero_offset_call_agrees_through_the_fixture_the_production_service_and_the_event_service(
    ) {
        let bundle = load("zero_offset.json");
        let cases = items(member(&bundle, "api"));
        let ids: Vec<String> = cases
            .iter()
            .map(|case| text_of(member(member(case, "request"), "id")))
            .collect();
        assert_eq!(ids, ["api-yamlzero-000"]);
        assert_eq!(items(member(&bundle, "cli")).len(), 0);
        let mut problems = Vec::new();
        for case in cases {
            problems.extend(fixture_problems(case, false));
            problems.extend(service_problems(case));
            problems.extend(events_problems(case));
        }
        all_or_fail(problems);
    }
}

// ── a crafted case through the fixture program ───────────────────────────────────────────────────

/// Runs a made case (`function`, JSON `args`) through the API fixture over `area` and returns the control record.
fn fixture_api(area: &Area, function: &str, args: &str, profile: &str) -> PyValue {
    let spec = area.dir.join("made-spec.json");
    static MADE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let number = MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let control = area.dir.join(format!("made-control-{number}.json"));
    std::fs::write(
        &spec,
        format!("{{\"id\": \"made\", \"owner\": \"resolve_build_track\", \"kind\": \"api\", \"function\": \"{function}\", \"args\": {args}, \"kwargs\": {{}}, \"profile\": \"{profile}\", \"files\": {{}}}}"),
    )
    .expect("a spec");
    let mut run = command(env!("CARGO_BIN_EXE_upstream_fixture_api"), area, false);
    run.arg("case")
        .arg("--spec")
        .arg(&spec)
        .arg("--root")
        .arg(&area.root)
        .arg("--control")
        .arg(&control);
    let output = finish(run, None);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    read_control(&control)
}

fn trace_of(record: &PyValue) -> Vec<(String, PyValue)> {
    items(member(record, "trace"))
        .iter()
        .map(|step| (text_of(member(step, "event")), step.clone()))
        .collect()
}

fn dto(text: &str) -> PyValue {
    j(text)
}

mod contract_cases {
    use super::*;

    #[test]
    fn the_constants_are_the_source_constants_as_typed_values() {
        let work = area("constants");
        let mut run = command(env!("CARGO_BIN_EXE_upstream_api"), &work, false);
        run.arg("constants");
        let output = finish(run, None);
        assert_eq!(output.status.code(), Some(0));
        let got = loads_text(&PyStr::from_surrogateescape(&output.stdout)).expect("constants");
        assert!(same_value(
            &got,
            &dto(
                r#"{"resolve_build_track": {"SOURCE_BUILD_MIN": ["tuple", [["int", "2"], ["int", "11"]]], "_MANIFEST_SM_KEYS": ["tuple", [["str", "compute_capability"], ["str", "sm_arch"], ["str", "gpu_compute_capability"], ["str", "cuda_arch"]]]}}"#
            )
        ));
        // the same constants are the saved records' globals
        let bundle = load("bundle.json");
        let saved = member(
            member(&items(member(&bundle, "api"))[0], "original"),
            "globals_before",
        );
        let module = member(&got, "resolve_build_track");
        assert!(same_value(
            member(saved, "SOURCE_BUILD_MIN"),
            member(module, "SOURCE_BUILD_MIN")
        ));
        assert!(same_value(
            member(saved, "_MANIFEST_SM_KEYS"),
            member(module, "_MANIFEST_SM_KEYS")
        ));
    }

    #[test]
    fn the_globals_member_the_binding_and_the_refusals_are_as_documented() {
        let work = area("contract");
        let globals = r#"{"argv": ["resolve_build_track.sh"]}"#;
        let lines = vec![
            r#"{"id": 1, "operation": "resolve_build_track.norm_release", "args": [1], "context": {}}"#.to_string(),
            request(2, "norm_release", "[1]", "{}", r#"{"other": 1}"#),
            request(3, "other", "[]", "{}", globals),
            r#"{"id": 4, "operation": "other.main", "args": []}"#.to_string(),
            request(5, "norm_release", "[]", "{}", globals),
            request(6, "_self_test", "[1]", "{}", globals),
            request(7, "main", "[]", r#"{"x": 1}"#, globals),
            request(8, "resolve", "[1, 2, 3]", "{}", globals),
            request(9, "resolve", "[]", r#"{"torch_pin": "2.11"}"#, globals),
            request(10, "norm_release", "[]", "{}", r#"{"argv": "x"}"#),
            request(11, "norm_release", "[]", "{}", r#"{"argv": ["a", 1]}"#),
            request(12, "norm_release", "[]", "{}", "[]"),
            request(13, "norm_release", "[]", "{}", r#"{"SOURCE_BUILD_MIN": ["tuple", [["int", "2"], ["int", "12"]]]}"#),
            request(14, "norm_release", "[\"2.11\"]", "{}", r#"{"SOURCE_BUILD_MIN": ["tuple", [["int", "2"], ["int", "11"]]], "_MANIFEST_SM_KEYS": ["tuple", [["str", "compute_capability"], ["str", "sm_arch"], ["str", "gpu_compute_capability"], ["str", "cuda_arch"]]], "argv": ["resolve_build_track.sh"]}"#),
            r#"{"id": 15, "operation": "resolve_build_track", "args": []}"#.to_string(),
            request(16, "norm_release", "[\"2.11\"]", r#"{"v": "2.12"}"#, globals),
        ];
        let (output, got) = serve_lines(&work, &lines, false);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let kind = |row: &PyValue| {
            (
                text_of(member(row, "type")),
                text_of(member(row, "message")),
            )
        };
        assert!(kind(&got[0])
            .1
            .starts_with("the request has a member this service does not take: "));
        for index in [1, 9, 10, 11, 12] {
            assert_eq!(kind(&got[index]).0, "RequestError", "{index}");
        }
        assert_eq!(
            kind(&got[2]),
            (
                "AttributeError".to_string(),
                "module 'resolve_build_track' has no attribute 'other'".to_string()
            )
        );
        assert_eq!(
            kind(&got[3]),
            (
                "ModuleNotFoundError".to_string(),
                "No module named 'other'".to_string()
            )
        );
        for (index, message) in [
            (
                4,
                "norm_release() missing 1 required positional argument: 'v'",
            ),
            (
                5,
                "_self_test() takes 0 positional arguments but 1 was given",
            ),
            (6, "main() got an unexpected keyword argument 'x'"),
            (7, "resolve() takes 2 positional arguments but 3 were given"),
            (
                8,
                "resolve() missing 1 required positional argument: 'sm_arch'",
            ),
            (15, "norm_release() got multiple values for argument 'v'"),
        ] {
            let (class, text) = kind(&got[index]);
            assert_eq!(
                (class.as_str(), text.as_str()),
                ("TypeError", message),
                "{index}"
            );
        }
        assert_eq!(text_of(member(&got[13], "kind")), "value");
        assert_eq!(
            kind(&got[14]),
            (
                "AttributeError".to_string(),
                "module 'resolve_build_track' has no attribute ''".to_string()
            )
        );
    }

    /// Reading the source: no provider or case lookup, no fixture module, no operational path, no Python, no process, the environment read
    /// only for the terminal width and the declared YAML installation, the import asked in one place.
    #[test]
    fn the_sources_have_no_selector_no_fixture_module_no_operational_path_no_spawn_and_one_import_site(
    ) {
        let sources = [
            ("api.rs", include_str!("../src/api.rs")),
            ("bench.rs", include_str!("../src/bench.rs")),
            ("business.rs", include_str!("../src/business.rs")),
            ("cli.rs", include_str!("../src/cli.rs")),
            ("dto.rs", include_str!("../src/dto.rs")),
            ("events.rs", include_str!("../src/events.rs")),
            ("host.rs", include_str!("../src/host.rs")),
            ("live.rs", include_str!("../src/live.rs")),
            ("main_cmd.rs", include_str!("../src/main_cmd.rs")),
            ("manifest.rs", include_str!("../src/manifest.rs")),
            ("pytext.rs", include_str!("../src/pytext.rs")),
            ("textfile.rs", include_str!("../src/textfile.rs")),
            ("thrown.rs", include_str!("../src/thrown.rs")),
            (
                "bin/resolve_build_track.rs",
                include_str!("../src/bin/resolve_build_track.rs"),
            ),
            (
                "bin/upstream_api.rs",
                include_str!("../src/bin/upstream_api.rs"),
            ),
            (
                "bin/upstream_bench.rs",
                include_str!("../src/bin/upstream_bench.rs"),
            ),
        ];
        let mut found = Vec::new();
        for (name, text) in sources {
            for forbidden in [
                "getenv",
                "set_var",
                "remove_var",
                "upstream_runtime::fixture",
                "mod fixture",
                "real-worker",
                "val_agent",
                "CARGO_MANIFEST_DIR",
                "BoundaryViolation",
                "/usr/bin/python",
                "python3",
                "find_spec",
                "Command::new",
                "process::Command",
                "\"api-",
                "\"cli-",
                "@CASE@",
                "expected_lookup",
                "pyyaml-6.0.3\"",
            ] {
                // the declared profile names live in `host.rs` only
                if text.contains(forbidden) && !(name == "host.rs" && forbidden == "pyyaml-6.0.3\"")
                {
                    found.push(format!("{name}: {forbidden}"));
                }
            }
            // the environment: the terminal width and the YAML installation, in the bins only
            let reads = text.matches("env::var(").count();
            let allowed = name.starts_with("bin/")
                && reads == 2
                && text.contains("\"COLUMNS\"")
                && text.contains("\"UPSTREAM_RUNTIME_YAML_PROFILE\"");
            if reads > 0 && !allowed && !(name == "bin/upstream_api.rs" && reads == 2) {
                found.push(format!("{name}: {reads} environment read(s)"));
            }
            if name.starts_with("bin/") && reads != 2 && name != "bin/upstream_api.rs" {
                found.push(format!(
                    "{name}: expected the two declared environment reads, found {reads}"
                ));
            }
        }
        assert!(found.is_empty(), "{}", found.join("\n"));
        // the import is asked of the host at exactly one place of the business code, and only there
        let manifest = include_str!("../src/manifest.rs");
        assert_eq!(manifest.matches("host.import_yaml()").count(), 1);
        for (name, text) in [
            ("main_cmd.rs", include_str!("../src/main_cmd.rs")),
            ("business.rs", include_str!("../src/business.rs")),
            ("api.rs", include_str!("../src/api.rs")),
            ("cli.rs", include_str!("../src/cli.rs")),
        ] {
            assert!(
                !text.contains("host.import_yaml()"),
                "{name} must not import"
            );
        }
    }
}

mod new_cases {
    use super::*;

    fn path_arg(work: &Area, name: &str) -> String {
        format!("[{}]", jstr(&format!("{}/{name}", root_text(work))))
    }

    fn value_of(record: &PyValue) -> &PyValue {
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

    fn rows(record: &PyValue) -> &[PyValue] {
        items(member(record, "exception_chain"))
    }

    // ── the typed value ──────────────────────────────────────────────────────────────────────

    #[test]
    fn yaml_dates_datetimes_sets_binary_and_typed_keys_come_back_typed_in_file_order() {
        let work = area("typed");
        put(
            &work.root,
            "manifest.yaml",
            b"d: 2026-01-01\ndt: 2026-01-01 01:02:03.5-03:30\nnaive: 2001-12-14t21:59:43.10\nutc: 2001-12-14T21:59:43Z\ns: !!set {b, a, 3, 1}\n2026-02-03: dkey\nb: !!binary SGk=\n",
        );
        let record = call1(
            &work,
            false,
            "load_manifest",
            &path_arg(&work, "manifest.yaml"),
        );
        let want = dto(r#"["dict", [
              [["str", "d"], ["date", "2026-01-01"]],
              [["str", "dt"], ["datetime", "2026-01-01T01:02:03.500000-03:30"]],
              [["str", "naive"], ["datetime", "2001-12-14T21:59:43.100000"]],
              [["str", "utc"], ["datetime", "2001-12-14T21:59:43+00:00"]],
              [["str", "s"], ["set", [["int", "1"], ["int", "3"], ["str", "a"], ["str", "b"]]]],
              [["date", "2026-02-03"], ["str", "dkey"]],
              [["str", "b"], ["bytes", "4869"]]
            ]]"#);
        assert!(
            same_value(&want, value_of(&record)),
            "{}",
            value_of(&record).repr()
        );
    }

    #[test]
    fn str_of_a_manifest_value_is_pythons_str_a_date_prints_its_iso_form_a_datetime_with_a_space_a_set_its_repr(
    ) {
        let cases = [
            (
                "compute_capability: 2026-11-07T01:02:03+09:00\n",
                "2026-11-07 01:02:03+09:00",
            ),
            ("compute_capability: 2026-11-07\n", "2026-11-07"),
            ("compute_capability: !!set {a}\n", "{'a'}"),
            ("compute_capability: [1, 2]\n", "[1, 2]"),
            ("compute_capability: 12.0\n", "12.0"),
        ];
        for (at, (manifest, shown)) in cases.iter().enumerate() {
            let work = area(&format!("str-{at}"));
            put(&work.root, "manifest.yaml", manifest.as_bytes());
            let line = request(
                1,
                "main",
                "[]",
                "{}",
                r#"{"argv": ["resolve_build_track.sh", "--torch-pin", "2.11"]}"#,
            );
            let (output, records) = serve_lines(&work, &[line], false);
            assert_eq!(output.status.code(), Some(0));
            let stdout = text_of(member(&records[0], "stdout"));
            assert!(
                stdout.contains(&format!("\"sm_arch_input\": {}", jstr(shown))),
                "{manifest}: {stdout}"
            );
            assert!(stdout.contains("\"sm_arch_source\": \"manifest[compute_capability]\""));
        }
    }

    // ── the manifest loader ──────────────────────────────────────────────────────────────────

    #[test]
    fn the_flat_parser_reads_quotes_comments_duplicates_and_skips_as_the_original_does() {
        let work = area("flat");
        put(
            &work.root,
            "manifest.yaml",
            "# c\na: 1\na: 2 # dup\nb: 'x'\nc: \"y z\" # tail\nd: 'it's'\ne:\nf: []\ng: ' x '\nh: x#y\ni: x #y\nj: '\nk: ''\n lead: no\n-item: no\n\ttab: no\nno colon\n\u{3000}\nkey with space: v\nq: \"a # b\" # c\n"
                .as_bytes(),
        );
        let record = call1(
            &work,
            true,
            "load_manifest",
            &path_arg(&work, "manifest.yaml"),
        );
        let want = dto(r#"["dict", [
              [["str", "a"], ["str", "2"]], [["str", "b"], ["str", "x"]], [["str", "c"], ["str", "y z"]],
              [["str", "d"], ["str", "it's"]], [["str", "e"], ["str", ""]], [["str", "f"], ["list", []]],
              [["str", "g"], ["str", " x "]], [["str", "h"], ["str", "x#y"]], [["str", "i"], ["str", "x"]],
              [["str", "j"], ["str", ""]], [["str", "k"], ["str", ""]], [["str", "key with space"], ["str", "v"]],
              [["str", "q"], ["str", "a # b"]]
            ]]"#);
        assert!(
            same_value(&want, value_of(&record)),
            "{}",
            value_of(&record).repr()
        );
        // universal newlines: a bare CR and CR-LF end lines, the last line needs no newline
        put(&work.root, "cr.yaml", b"a: 1\rb: 2\r\nc: 3");
        let record = call1(&work, true, "_load_yaml_flat", &path_arg(&work, "cr.yaml"));
        assert!(same_value(
            &dto(
                r#"["dict", [[["str", "a"], ["str", "1"]], [["str", "b"], ["str", "2"]], [["str", "c"], ["str", "3"]]]]"#
            ),
            value_of(&record)
        ));
    }

    /// A file longer than the reader's pull: `read(4096)` calls return the file in order, then an empty text, then the file is closed.
    #[test]
    fn the_yaml_reader_pulls_four_thousand_ninety_six_characters_at_a_time_until_the_end() {
        let work = area("pulls");
        let mut text = String::new();
        let mut at = 0;
        while text.len() < 10_000 {
            text.push_str(&format!("key{at}: v{at}\n"));
            at += 1;
        }
        put(&work.root, "manifest.yaml", text.as_bytes());
        let record = fixture_api(
            &work,
            "load_manifest",
            &path_arg(&work, "manifest.yaml"),
            "pyyaml-6.0.3",
        );
        let trace = trace_of(&record);
        let mut returned = String::new();
        let mut reads = 0;
        for (name, step) in &trace {
            match name.as_str() {
                "file_read" => {
                    reads += 1;
                    assert!(same_value(member(step, "args"), &j("[4096]")));
                }
                "file_read_return" => returned.push_str(&text_of(member(step, "text"))),
                _ => {}
            }
        }
        assert_eq!(returned, text);
        assert!(reads >= 4, "{reads} pulls");
        let names: Vec<&str> = trace.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(names.first(), Some(&"file_open"));
        assert_eq!(names.last(), Some(&"file_close"));
        assert_eq!(
            text_of(member(&trace[trace.len() - 2].1, "text")),
            "",
            "the last pull is the end of the file"
        );
        assert_eq!(text_of(member(&record, "kind")), "value");
    }

    /// Where a decoding error surfaces is the 8192-byte chunk boundary's: the characters before it are delivered, the error names the chunk.
    #[test]
    fn a_decoding_error_surfaces_at_its_chunk_with_the_chunk_as_the_object_and_the_earlier_pulls_delivered(
    ) {
        let work = area("late-decode");
        let mut bytes = b"x: ".to_vec();
        bytes.extend(std::iter::repeat_n(b'a', 8998));
        bytes.extend_from_slice(b"\xff\n");
        assert_eq!(bytes.len(), 9003);
        put(&work.root, "manifest.yaml", &bytes);
        let message = "'utf-8' codec can't decode byte 0xff in position 809: invalid start byte";
        for profile in ["pyyaml-6.0.3", "flat"] {
            let function = if profile == "flat" {
                "_load_yaml_flat"
            } else {
                "load_manifest"
            };
            let record = fixture_api(&work, function, &path_arg(&work, "manifest.yaml"), profile);
            assert_eq!(
                exception_of(&record),
                ("UnicodeDecodeError".to_string(), message.to_string()),
                "{profile}"
            );
            let row = &rows(&record)[0];
            let ctor = member(row, "constructor");
            assert_eq!(int_of(member(ctor, "start")), 809);
            assert_eq!(int_of(member(ctor, "end")), 810);
            assert_eq!(text_of(member(ctor, "reason")), "invalid start byte");
            assert_eq!(text_of(member(ctor, "encoding")), "utf-8");
            // the object is the second chunk: the bytes from offset 8192
            let want_hex: String = bytes[8192..]
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            assert_eq!(text_of(member(ctor, "object_hex")), want_hex);
            let trace = trace_of(&record);
            let last = trace.last().expect("a trace");
            assert_eq!(last.0, "file_close");
            if profile == "flat" {
                // the one long line never completes: the error is the first thing the iteration meets
                let names: Vec<&str> = trace.iter().map(|(name, _)| name.as_str()).collect();
                assert_eq!(names, ["file_open", "file_iter_error", "file_close"]);
            } else {
                assert_eq!(trace[trace.len() - 2].0, "file_read_error");
                let returns = trace
                    .iter()
                    .filter(|(name, _)| name == "file_read_return")
                    .count();
                assert!(returns >= 2, "the pulls of the first chunk were delivered");
            }
        }
        // many short lines: the complete lines of the first chunk are delivered before the error
        let mut lines = String::new();
        let mut count = 0;
        while lines.len() < 8_300 {
            lines.push_str(&format!("k{count}: v\n"));
            count += 1;
        }
        let mut many = lines.into_bytes();
        many.extend_from_slice(b"\xff\n");
        put(&work.root, "many.yaml", &many);
        let record = fixture_api(
            &work,
            "_load_yaml_flat",
            &path_arg(&work, "many.yaml"),
            "flat",
        );
        let delivered = trace_of(&record)
            .iter()
            .filter(|(name, _)| name == "file_iter_line")
            .count();
        let complete = many[..8192].iter().filter(|&&byte| byte == b'\n').count();
        assert_eq!(delivered, complete);
        assert_eq!(rows(&record).len(), 1);
    }

    #[test]
    fn a_character_split_by_the_chunk_boundary_is_decoded_whole() {
        let work = area("split");
        let mut bytes = b"x: ".to_vec();
        bytes.extend(std::iter::repeat_n(b'a', 8192 - 3 - 1));
        bytes.extend_from_slice("한".as_bytes());
        bytes.extend_from_slice(b"\n");
        put(&work.root, "manifest.yaml", &bytes);
        for (function, profile) in [
            ("load_manifest", "pyyaml-6.0.3"),
            ("_load_yaml_flat", "flat"),
        ] {
            let record = fixture_api(&work, function, &path_arg(&work, "manifest.yaml"), profile);
            let value = value_of(&record);
            let PyValue::List(parts) = value else {
                panic!("a dto")
            };
            let PyValue::List(pairs) = &parts[1] else {
                panic!("pairs")
            };
            let PyValue::List(pair) = &pairs[0] else {
                panic!("a pair")
            };
            let PyValue::List(shown) = &pair[1] else {
                panic!("a str")
            };
            assert!(text_of(&shown[1]).ends_with('한'), "{function}");
        }
    }

    // ── the import timing and its exceptions (the event service with a parent) ───────────────

    fn main_line(argv: &[&str]) -> String {
        let mut all = vec![jstr("resolve_build_track.sh")];
        all.extend(argv.iter().map(|each| jstr(each)));
        request(
            1,
            "main",
            "[]",
            "{}",
            &format!("{{\"argv\": [{}]}}", all.join(", ")),
        )
    }

    fn import_failure(sequence: i64, rows: &str) -> String {
        format!("{{\"event\": \"yaml_import_ack\", \"sequence\": {sequence}, \"kind\": \"exception\", \"exception_chain\": {rows}}}")
    }

    fn row(class: &str, module: &str, message: &str, bases: &str, relation: &str) -> String {
        format!(
            "{{\"type\": {}, \"module\": {}, \"message\": {}, \"suppress_context\": false, \"relation\": {relation}, \"builtin_bases\": {bases}}}",
            jstr(class),
            jstr(module),
            jstr(message)
        )
    }

    #[test]
    fn the_import_is_asked_exactly_where_the_original_reaches_it_and_never_before() {
        let work = area("timing");
        put(&work.root, "manifest.yaml", b"compute_capability: 12.1\n");
        std::fs::create_dir(work.root.join("dir")).expect("a directory");
        let cases: Vec<(Vec<&str>, usize)> = vec![
            (vec!["--self-test"], 0),
            (vec![], 0),
            (vec!["--torch-pin", "2.11", "--sm-arch", "89"], 0),
            (vec!["--torch-pin", "2.11", "--manifest", "absent.yaml"], 0),
            (vec!["--torch-pin", "2.11", "--manifest", "dir"], 0),
            (vec!["--help"], 0),
            (vec!["--bogus"], 0),
            (vec!["--torch-pin", "2.11"], 1),
            (vec!["--torch-pin", "2.11", "--sm-arch", ""], 1),
        ];
        for (argv, imports) in cases {
            let seen = session(&work, &main_line(&argv), accept_all, |sequence| {
                Some(import_ack(sequence, false))
            });
            assert_eq!(seen.status, Some(0), "{argv:?}: {}", seen.stderr);
            assert_eq!(seen.imports.len(), imports, "{argv:?}");
            assert!(seen.result.is_some());
            if imports == 1 {
                // the import comes before every write (the proposal is printed last), and the writes' sequence goes on from it
                assert_eq!(seen.imports[0], (0, 0));
                assert_eq!(seen.writes[0].0, 1);
            }
        }
    }

    #[test]
    fn an_import_error_takes_the_flat_path_with_the_import_error_as_context_and_any_other_failure_is_the_originals(
    ) {
        let work = area("import-kinds");
        put(
            &work.root,
            "manifest.yaml",
            b"compute_capability: 12.1\nsm_arch: old\n",
        );
        std::fs::create_dir(work.root.join("dir")).expect("a directory");
        let line = |path: &str| {
            request(
                1,
                "load_manifest",
                &format!("[{}]", jstr(&format!("{}/{path}", root_text(&work)))),
                "{}",
                "{\"argv\": [\"resolve_build_track.sh\"]}",
            )
        };
        // an ImportError of another module, by its real ancestry: the original's `except ImportError`
        let custom = import_failure(
            0,
            &format!(
                "[{}]",
                row(
                    "MyImportError",
                    "mine",
                    "no yaml here",
                    r#"["ImportError", "Exception", "BaseException"]"#,
                    "null"
                )
            ),
        );
        let seen = session(&work, &line("manifest.yaml"), accept_all, |_| {
            Some(custom.clone())
        });
        assert_eq!(seen.status, Some(0), "{}", seen.stderr);
        let record = seen.result.expect("a result");
        assert!(same_value(
            &dto(
                r#"["dict", [[["str", "compute_capability"], ["str", "12.1"]], [["str", "sm_arch"], ["str", "old"]]]]"#
            ),
            value_of(&record)
        ));
        // an exception raised by the flat path has the import error as its implicit context, whatever class it is
        let seen = session(&work, &line("dir"), accept_all, |_| Some(custom.clone()));
        let record = seen.result.expect("a result");
        assert_eq!(exception_of(&record).0, "IsADirectoryError");
        let chain = rows(&record);
        assert_eq!(chain.len(), 2);
        assert_eq!(text_of(member(&chain[0], "relation")), "context");
        assert_eq!(text_of(member(&chain[1], "type")), "MyImportError");
        assert_eq!(text_of(member(&chain[1], "module")), "mine");
        assert!(!has(&chain[1], "builtin_bases"));
        // a failure that is neither an ImportError nor a FileNotFoundError propagates whole, with its chain
        let other = import_failure(
            0,
            &format!(
                "[{}, {}]",
                row("RuntimeError", "builtins", "broken install", r#"["RuntimeError", "Exception", "BaseException"]"#, "\"cause\""),
                "{\"type\": \"OSError\", \"module\": \"builtins\", \"message\": \"disk\", \"suppress_context\": false, \"relation\": null}"
            ),
        );
        let seen = session(&work, &line("manifest.yaml"), accept_all, |_| {
            Some(other.clone())
        });
        let record = seen.result.expect("a result");
        assert_eq!(
            exception_of(&record),
            ("RuntimeError".to_string(), "broken install".to_string())
        );
        assert_eq!(rows(&record).len(), 2);
        assert_eq!(text_of(member(&rows(&record)[0], "relation")), "cause");
        // a FileNotFoundError is the original's second `except`: an empty manifest
        let missing = import_failure(
            0,
            &format!(
                "[{}]",
                row(
                    "FileNotFoundError",
                    "builtins",
                    "[Errno 2] gone",
                    r#"["FileNotFoundError", "OSError", "Exception", "BaseException"]"#,
                    "null"
                )
            ),
        );
        let seen = session(&work, &line("manifest.yaml"), accept_all, |_| {
            Some(missing.clone())
        });
        assert!(same_value(
            &dto(r#"["dict", []]"#),
            value_of(&seen.result.expect("a result"))
        ));
        // the declared absent installation: `ModuleNotFoundError`, then the flat path
        let seen = session(&work, &line("manifest.yaml"), accept_all, |sequence| {
            Some(import_ack(sequence, true))
        });
        assert!(same_value(
            &dto(
                r#"["dict", [[["str", "compute_capability"], ["str", "12.1"]], [["str", "sm_arch"], ["str", "old"]]]]"#
            ),
            value_of(&seen.result.expect("a result"))
        ));
    }

    #[test]
    fn a_missing_or_invalid_import_acknowledgement_abandons_the_run_with_no_result() {
        let work = area("import-abort");
        put(&work.root, "manifest.yaml", b"compute_capability: 12.1\n");
        let line = main_line(&["--torch-pin", "2.11"]);
        let good_row = row(
            "ModuleNotFoundError",
            "builtins",
            "No module named 'yaml'",
            r#"["ModuleNotFoundError", "ImportError", "Exception", "BaseException"]"#,
            "null",
        );
        let cases: Vec<(&str, Option<String>)> = vec![
            ("eof", None),
            ("version", Some("{\"event\": \"yaml_import_ack\", \"sequence\": 0, \"kind\": \"available\", \"version\": \"6.0.2\"}".to_string())),
            ("no-version", Some("{\"event\": \"yaml_import_ack\", \"sequence\": 0, \"kind\": \"available\"}".to_string())),
            ("sequence", Some("{\"event\": \"yaml_import_ack\", \"sequence\": 5, \"kind\": \"available\", \"version\": \"6.0.3\"}".to_string())),
            ("kind", Some("{\"event\": \"yaml_import_ack\", \"sequence\": 0, \"kind\": \"maybe\"}".to_string())),
            ("event", Some("{\"event\": \"write_ack\", \"sequence\": 0, \"kind\": \"ok\"}".to_string())),
            ("json", Some("not json".to_string())),
            ("empty-chain", Some(import_failure(0, "[]"))),
            ("no-bases", Some(import_failure(0, "[{\"type\": \"ImportError\", \"module\": \"builtins\", \"message\": \"m\", \"suppress_context\": false, \"relation\": null}]"))),
            ("bad-bases", Some(import_failure(0, &format!("[{}]", row("ImportError", "builtins", "m", r#"["ImportError", "Exception"]"#, "null"))))),
            ("not-module-not-found", Some(import_failure(0, &format!("[{}]", row("ModuleNotFoundError", "builtins", "m", r#"["ModuleNotFoundError", "Exception", "BaseException"]"#, "null"))))),
            ("constructor", Some(import_failure(0, &good_row.replace("\"builtin_bases\"", "\"constructor\": {\"encoding\": \"u\"}, \"builtin_bases\"")))),
            ("extra-member", Some(import_failure(0, &good_row.replace("\"relation\": null", "\"relation\": null, \"x\": 1")))),
        ];
        for (name, answer) in cases {
            let seen = session(&work, &line, accept_all, |_| answer.clone());
            assert_eq!(seen.status, Some(3), "{name}: {}", seen.stderr);
            assert!(seen.result.is_none(), "{name}");
            assert!(
                seen.writes.is_empty(),
                "{name}: no write after the failed import"
            );
            assert_eq!(seen.imports.len(), 1, "{name}");
            assert!(!seen.stderr.is_empty(), "{name}");
        }
    }

    // ── the writes (a parent whose streams can fail) ─────────────────────────────────────────

    fn failure(sequence: i64, class: &str, module: &str, message: &str, bases: &str) -> String {
        format!(
            "{{\"event\": \"write_ack\", \"sequence\": {sequence}, \"kind\": \"exception\", \"type\": {}, \"module\": {}, \"message\": {}, \"builtin_bases\": {bases}}}",
            jstr(class),
            jstr(module),
            jstr(message)
        )
    }

    const OS: &str =
        r#"["BrokenPipeError", "ConnectionError", "OSError", "Exception", "BaseException"]"#;
    const VALUE: &str = r#"["ValueError", "Exception", "BaseException"]"#;

    #[test]
    fn a_failing_print_stops_the_command_and_argparses_swallow_rules_follow_the_acknowledged_ancestry(
    ) {
        let work = area("writes");
        put(&work.root, "manifest.yaml", b"compute_capability: 12.1\n");
        // the proposal's print: the text write fails, the newline is never written
        let seen = session(
            &work,
            &main_line(&["--torch-pin", "2.11"]),
            |sequence, _s, _t| {
                Some(failure(
                    sequence,
                    "BrokenPipeError",
                    "builtins",
                    "[Errno 32] Broken pipe",
                    OS,
                ))
            },
            |s| Some(import_ack(s, false)),
        );
        assert_eq!(seen.writes.len(), 1);
        let record = seen.result.expect("a result");
        assert_eq!(
            exception_of(&record),
            (
                "BrokenPipeError".to_string(),
                "[Errno 32] Broken pipe".to_string()
            )
        );
        // an OSError of the parent's stream while argparse writes the help is swallowed: the SystemExit(0) follows
        let seen = session(
            &work,
            &main_line(&["--help"]),
            |sequence, _s, _t| Some(failure(sequence, "BrokenPipeError", "builtins", "x", OS)),
            |s| Some(import_ack(s, false)),
        );
        assert_eq!(
            exception_of(&seen.result.expect("a result")),
            ("SystemExit".to_string(), "0".to_string())
        );
        // any other class stops the parser at that write: that exception, no SystemExit, no second write
        let seen = session(
            &work,
            &main_line(&["--bogus"]),
            |sequence, _s, _t| {
                Some(failure(
                    sequence,
                    "ValueError",
                    "builtins",
                    "refused",
                    VALUE,
                ))
            },
            |s| Some(import_ack(s, false)),
        );
        assert_eq!(seen.writes.len(), 1);
        assert_eq!(
            exception_of(&seen.result.expect("a result")),
            ("ValueError".to_string(), "refused".to_string())
        );
        // inside the handler of an error about a named argument the failure has that error as context
        let seen = session(
            &work,
            &main_line(&["--torch-pin"]),
            |sequence, _s, _t| {
                Some(failure(
                    sequence,
                    "ValueError",
                    "builtins",
                    "refused",
                    VALUE,
                ))
            },
            |s| Some(import_ack(s, false)),
        );
        let record = seen.result.expect("a result");
        let chain = rows(&record);
        assert_eq!(chain.len(), 2);
        assert_eq!(text_of(member(&chain[0], "relation")), "context");
        assert_eq!(text_of(member(&chain[1], "type")), "ArgumentError");
        assert_eq!(seen.writes.len(), 1);
        // the usage write fails with an OSError: the error message is still written, then the SystemExit(2)
        let seen = session(
            &work,
            &main_line(&["--bogus"]),
            |sequence, _s, _t| Some(failure(sequence, "BrokenPipeError", "builtins", "x", OS)),
            |s| Some(import_ack(s, false)),
        );
        assert_eq!(seen.writes.len(), 2);
        assert!(text_of(&PyValue::Str(seen.writes[1].2.clone()))
            .starts_with("resolve_build_track.sh: error: unrecognized arguments"));
        assert_eq!(
            exception_of(&seen.result.expect("a result")),
            ("SystemExit".to_string(), "2".to_string())
        );
        // the missing pin's line goes to standard error as two writes, then SystemExit(2)
        let seen = session(&work, &main_line(&[]), accept_all, |s| {
            Some(import_ack(s, false))
        });
        let streams: Vec<&str> = seen
            .writes
            .iter()
            .map(|(_, stream, _)| stream.as_str())
            .collect();
        assert_eq!(streams, ["stderr", "stderr"]);
        assert_eq!(text_of(&PyValue::Str(seen.writes[1].2.clone())), "\n");
    }

    #[test]
    fn a_missing_or_invalid_write_acknowledgement_abandons_the_run_whatever_the_write() {
        let work = area("write-abort");
        put(&work.root, "manifest.yaml", b"compute_capability: 12.1\n");
        for argv in [
            vec!["--help"],
            vec!["--bogus"],
            vec![],
            vec!["--torch-pin", "2.11", "--sm-arch", "89"],
        ] {
            let bad: Vec<Box<dyn Fn(i64) -> Option<String>>> = vec![
                Box::new(|_| None),
                Box::new(|n| Some(ok_ack(n + 1))),
                Box::new(|_| Some("not json".to_string())),
                Box::new(|n| Some(failure(n, "ValueError", "builtins", "m", "[]"))),
                Box::new(|n| {
                    Some(failure(
                        n,
                        "ValueError",
                        "builtins",
                        "m",
                        r#"["ValueError", "Exception"]"#,
                    ))
                }),
                Box::new(|n| {
                    Some(failure(
                        n,
                        "UnicodeEncodeError",
                        "builtins",
                        "m",
                        r#"["UnicodeEncodeError", "UnicodeError", "ValueError", "Exception", "BaseException"]"#,
                    ))
                }),
            ];
            for (at, make) in bad.iter().enumerate() {
                let seen = session(
                    &work,
                    &main_line(&argv),
                    |sequence, _s, _t| make(sequence),
                    |s| Some(import_ack(s, false)),
                );
                assert_eq!(seen.status, Some(3), "{argv:?}/{at}: {}", seen.stderr);
                assert!(seen.result.is_none(), "{argv:?}/{at}");
                assert_eq!(seen.writes.len(), 1, "{argv:?}/{at}");
            }
        }
    }

    // ── the command line's streams ───────────────────────────────────────────────────────────

    fn run_with_stdout_pipe(run: Command, reader_open: bool) -> std::process::Output {
        let mut run = run;
        let (reader, writer) = std::io::pipe().expect("a pipe");
        if !reader_open {
            drop(reader);
        }
        run.stdout(Stdio::from(writer))
            .stderr(Stdio::piped())
            .stdin(Stdio::null());
        let child = run.spawn().expect("start the program");
        let output = child.wait_with_output().expect("wait for the program");
        drop(run);
        output
    }

    #[test]
    fn a_short_output_on_a_broken_pipe_fails_at_the_final_flush_with_status_120_and_a_long_one_fails_in_its_print(
    ) {
        let work = area("broken");
        let mut help = command(env!("CARGO_BIN_EXE_resolve_build_track"), &work, false);
        help.arg("--help");
        let output = run_with_stdout_pipe(help, false);
        assert_eq!(output.status.code(), Some(120));
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            "Exception ignored in: <_io.TextIOWrapper name='<stdout>' mode='w' encoding='utf-8'>\nBrokenPipeError: [Errno 32] Broken pipe\n"
        );
        // a proposal longer than the text layer's chunk: the print itself fails, the exception is reported, status 1
        put(
            &work.root,
            "manifest.yaml",
            format!("compute_capability: {}\n", "a".repeat(9000)).as_bytes(),
        );
        let mut long = command(env!("CARGO_BIN_EXE_resolve_build_track"), &work, false);
        long.args(["--torch-pin", "2.11"]);
        let output = run_with_stdout_pipe(long, false);
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            "BrokenPipeError: [Errno 32] Broken pipe\n"
        );
        // the same command with a reader writes the whole proposal
        let mut again = command(env!("CARGO_BIN_EXE_resolve_build_track"), &work, false);
        again.args(["--torch-pin", "2.11"]);
        let output = finish(again, None);
        assert_eq!(output.status.code(), Some(0));
        assert!(String::from_utf8_lossy(&output.stdout).contains(&"a".repeat(9000)));
    }

    #[test]
    fn a_stdout_closed_at_start_makes_print_a_no_op_and_a_stderr_closed_at_start_sends_the_usage_to_stdout(
    ) {
        let work = area("closed");
        let wrapped = |redirect: &str, args: &[&str]| {
            let mut run = command("/bin/sh", &work, false);
            run.args([
                "-c",
                &format!("exec \"$0\" \"$@\" {redirect}"),
                env!("CARGO_BIN_EXE_resolve_build_track"),
            ]);
            run.args(args);
            finish(run, None)
        };
        let output = wrapped(">&-", &["--self-test"]);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        // no standard error: argparse's usage goes to standard output (its message has nowhere to go), the exit is still 2
        let output = wrapped("2>&-", &["--bogus"]);
        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&output.stdout)
            .starts_with("usage: resolve_build_track.sh [-h]"));
        assert!(!String::from_utf8_lossy(&output.stdout).contains("error:"));
    }

    #[test]
    fn the_program_name_in_every_text_is_the_originals_whatever_the_executable_is_called_or_where_it_runs(
    ) {
        use std::os::unix::process::CommandExt;
        let work = area("name");
        for name in ["/opt/other/anything", "resolve", "./deep/er/tool"] {
            let mut help = command(env!("CARGO_BIN_EXE_resolve_build_track"), &work, false);
            help.arg0(name).arg("--help");
            let output = finish(help, None);
            assert_eq!(output.status.code(), Some(0), "{name}");
            let text = String::from_utf8_lossy(&output.stdout).into_owned();
            assert!(
                text.starts_with("usage: resolve_build_track.sh [-h]"),
                "{name}: {text}"
            );
            let mut bad = command(env!("CARGO_BIN_EXE_resolve_build_track"), &work, false);
            bad.arg0(name).arg("--bogus");
            let output = finish(bad, None);
            assert_eq!(output.status.code(), Some(2));
            assert!(String::from_utf8_lossy(&output.stderr)
                .contains("\nresolve_build_track.sh: error: unrecognized arguments: --bogus\n"));
        }
    }

    #[test]
    fn an_exception_nothing_handles_is_reported_with_its_chain_and_the_status_is_1() {
        let work = area("uncaught");
        put(&work.root, "manifest.yaml", b"a: [\n");
        let mut run = command(env!("CARGO_BIN_EXE_resolve_build_track"), &work, false);
        run.args(["--torch-pin", "2.11"]);
        let output = finish(run, None);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            "yaml.parser.ParserError: while parsing a flow node\nexpected the node content, but found '<stream end>'\n  in \"manifest.yaml\", line 2, column 1\n"
        );
        // without PyYAML the flat parser's failure has the import error as its context
        put(&work.root, "manifest.yaml", b"a\xff\n");
        let mut flat = command(env!("CARGO_BIN_EXE_resolve_build_track"), &work, true);
        flat.args(["--torch-pin", "2.11"]);
        let output = finish(flat, None);
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            "ModuleNotFoundError: No module named 'yaml'\n\nDuring handling of the above exception, another exception occurred:\n\nUnicodeDecodeError: 'utf-8' codec can't decode byte 0xff in position 1: invalid start byte\n"
        );
    }

    #[test]
    fn an_invalid_event_request_is_answered_by_the_legacy_record_with_no_write_and_no_import() {
        let work = area("event-request");
        for line in [
            "not json".to_string(),
            r#"{"id": 1, "operation": "resolve_build_track.main", "args": [], "globals": {"other": 1}}"#.to_string(),
            r#"{"id": 2, "operation": "other.main"}"#.to_string(),
            r#"{"id": 3, "operation": "resolve_build_track.main", "args": [], "context": {}}"#.to_string(),
        ] {
            let seen = session(&work, &line, accept_all, |sequence| Some(import_ack(sequence, false)));
            assert_eq!(seen.status, Some(0), "{line}: {}", seen.stderr);
            assert!(seen.writes.is_empty() && seen.imports.is_empty());
            assert_eq!(text_of(member(&seen.result.expect("a result"), "kind")), "exception");
        }
        // the event service needs its descriptor
        for args in [
            vec!["evaluate-events"],
            vec!["evaluate-events", "--input-fd", "1"],
            vec!["evaluate-events", "extra"],
        ] {
            let mut run = command(env!("CARGO_BIN_EXE_upstream_api"), &work, false);
            run.args(&args);
            let output = finish(run, None);
            assert_eq!(output.status.code(), Some(2), "{args:?}");
            assert!(output.stdout.is_empty());
        }
    }

    // ── the small functions beyond the saved calls ───────────────────────────────────────────

    #[test]
    fn the_small_functions_follow_the_original_for_inputs_the_saved_calls_do_not_hold() {
        let work = area("small");
        let record = |operation: &str, args: &str| call1(&work, false, operation, args);
        let value = |operation: &str, args: &str| value_of(&record(operation, args)).clone();
        assert!(same_value(
            &value("norm_release", "[2.5]"),
            &dto(r#"["tuple", [["int", "2"], ["int", "5"]]]"#)
        ));
        assert!(same_value(
            &value("norm_release", r#"["٣.١٤"]"#),
            &dto(r#"["tuple", [["int", "3"], ["int", "14"]]]"#)
        ));
        assert!(same_value(
            &value("norm_release", "[[2, 11]]"),
            &dto(r#"["none"]"#)
        ));
        assert!(same_value(
            &value("norm_release", r#"["2.11.0.5"]"#),
            &dto(r#"["tuple", [["int", "2"], ["int", "11"]]]"#)
        ));
        assert!(same_value(
            &value("map_torch_cuda_arch", "[121]"),
            &dto(r#"["str", "12.1"]"#)
        ));
        assert!(same_value(
            &value("map_torch_cuda_arch", r#"[" 9 "]"#),
            &dto(r#"["str", "9.0"]"#)
        ));
        assert!(same_value(
            &value("map_torch_cuda_arch", r#"["sm_٩٠a"]"#),
            &dto(r#"["str", "9.٠a"]"#)
        ));
        assert!(same_value(
            &value("map_torch_cuda_arch", r#"["SM_90"]"#),
            &dto(r#"["str", "9.0"]"#)
        ));
        assert!(same_value(
            &value("_strip_inline_comment", r##"[["a", " ", "#", "b"]]"##),
            &dto(r#"["list", [["str", "a"], ["str", " "]]]"#)
        ));
        assert!(same_value(
            &value("sm_from_manifest", r#"[{"cuda_arch": [1], "sm_arch": ""}]"#),
            &dto(r#"["tuple", [["str", "[1]"], ["str", "cuda_arch"]]]"#)
        ));
        // the proposal's text names the pin as `str()` shows it
        let proposal = value("propose_track", "[2.5]");
        let PyValue::List(parts) = &proposal else {
            panic!("a dto")
        };
        assert!(text_of(&parts[0]) == "dict");
        let (class, message) = exception_of(&record("_strip_inline_comment", "[5]"));
        assert_eq!(
            (class.as_str(), message.as_str()),
            ("TypeError", "'int' object is not iterable")
        );
        let (class, message) = exception_of(&record("norm_release", r#"["²"]"#));
        assert_eq!(class, "ValueError");
        assert_eq!(message, "invalid literal for int() with base 10: '²'");
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

    fn row(metric: &str, case: &PyValue, root: &str, flat: bool, expected: &PyValue) -> PyValue {
        let request = member(case, "request");
        dict(vec![
            ("metric", PyValue::str(metric)),
            (
                "operation",
                PyValue::str(&format!(
                    "resolve_build_track.{}",
                    text_of(member(request, "function"))
                )),
            ),
            ("args", rebase_all(member(request, "args"), root)),
            ("kwargs", rebase_all(member(request, "kwargs"), root)),
            (
                "profile",
                PyValue::str(if flat { "flat" } else { "pyyaml-6.0.3" }),
            ),
            ("expected", expected.clone()),
        ])
    }

    fn run_bench(work: &Area, fixture: &Path, extra: &[&str]) -> std::process::Output {
        let mut run = command(env!("CARGO_BIN_EXE_upstream_bench"), work, false);
        run.arg("--fixture").arg(fixture).args(extra);
        finish(run, None)
    }

    fn write_rows(work: &Area, name: &str, rows: Vec<PyValue>) -> PathBuf {
        let path = work.dir.join(name);
        std::fs::write(&path, PyValue::List(rows).dumps_ascii().expect("JSON")).expect("a fixture");
        path
    }

    #[test]
    fn the_benchmark_calls_each_real_function_once_to_check_the_whole_dto_then_measures_whole_calls_with_real_files(
    ) {
        let bundle = load("bundle.json");
        let (resolve, manifest) = (saved(&bundle, "api-147"), saved(&bundle, "api-159"));
        assert_eq!(
            text_of(member(member(resolve, "request"), "function")),
            "resolve"
        );
        assert_eq!(
            text_of(member(member(manifest, "request"), "function")),
            "load_manifest"
        );
        let work = area("bench");
        build_files(member(member(manifest, "request"), "files"), &work.root);
        let root = root_text(&work);
        let good = write_rows(
            &work,
            "good.json",
            vec![
                row(
                    "resolve_api",
                    resolve,
                    &root,
                    false,
                    member(member(resolve, "original"), "value"),
                ),
                row(
                    "load_manifest_api",
                    manifest,
                    &root,
                    false,
                    member(member(manifest, "original"), "value"),
                ),
            ],
        );
        let output = run_bench(&work, &good, &["--rounds", "2", "--iterations", "3"]);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let text = String::from_utf8(output.stdout).expect("text");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 3, "{text}");
        assert_eq!(lines[0], "{\"rounds\": 2, \"iterations\": 3}");
        for (line, metric) in lines[1..].iter().zip(["resolve_api", "load_manifest_api"]) {
            let report = loads_text(&PyStr::new(line)).expect("a row");
            assert_eq!(text_of(member(&report, "metric")), metric);
            assert_eq!(text_of(member(&report, "unit")), "ns_per_call");
            assert_eq!(items(member(&report, "rounds")).len(), 2);
            assert_eq!(int_of(member(&report, "iterations")), 3);
        }
        // a wrong value ends the run before any timer: no report
        let wrong = write_rows(
            &work,
            "wrong.json",
            vec![row(
                "resolve_api",
                resolve,
                &root,
                false,
                &dto(r#"["none"]"#),
            )],
        );
        let output = run_bench(&work, &wrong, &["--rounds", "2", "--iterations", "3"]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        // the manifest is read for real in the preflight: a changed file is a difference
        std::fs::write(work.root.join("manifest.yaml"), "compute_capability: 9.9\n")
            .expect("a file");
        let output = run_bench(&work, &good, &["--rounds", "1", "--iterations", "1"]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        // a failing call is an error, not a sample (a directory is no manifest; the declared absent installation reads it flat)
        std::fs::remove_file(work.root.join("manifest.yaml")).expect("remove");
        std::fs::create_dir(work.root.join("manifest.yaml")).expect("a directory");
        let failing = write_rows(
            &work,
            "failing.json",
            vec![row(
                "load_manifest_api",
                manifest,
                &root,
                true,
                member(member(manifest, "original"), "value"),
            )],
        );
        let output = run_bench(&work, &failing, &[]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
    }

    fn counted(
        log: &std::rc::Rc<std::cell::RefCell<Vec<usize>>>,
        id: usize,
        answer: PyValue,
        fail_from: Option<usize>,
    ) -> recipe_runtime::bench::Case {
        let log = log.clone();
        recipe_runtime::bench::Case {
            metric: format!("m{id}"),
            call: Box::new(move || {
                let mut seen = log.borrow_mut();
                seen.push(id);
                let own = seen.iter().filter(|row| **row == id).count();
                if fail_from.is_some_and(|from| own >= from) {
                    return Err(agent_runtime_support::failure::Failure::new(
                        "RuntimeError",
                        "controlled",
                    ));
                }
                Ok(answer.clone())
            }),
            expected: dto(r#"["str", "ok"]"#),
        }
    }

    fn ok_answer() -> PyValue {
        dto(r#"["str", "ok"]"#)
    }

    /// Owned callbacks, no clock assertion (timings are not asserted here): the calls of each row, in order, are the counts of the
    /// frozen contract (one preflight per row, all before any loop; then rounds x the row's own iterations).
    #[test]
    fn mixed_row_iteration_counts_preflight_every_row_once_before_any_timer_and_report_only_when_all_series_pass(
    ) {
        use recipe_runtime::bench::Case;
        use std::cell::RefCell;
        use std::rc::Rc;
        use upstream_runtime::bench::{run_series, Series};
        let log = Rc::new(RefCell::new(Vec::new()));
        let series = vec![
            Series {
                case: counted(&log, 0, ok_answer(), None),
                iterations: Some(3),
            },
            Series {
                case: counted(&log, 1, ok_answer(), None),
                iterations: None,
            },
            Series {
                case: counted(&log, 2, ok_answer(), None),
                iterations: Some(1),
            },
        ];
        let report = run_series(&series, 2, 4).expect("a report");
        let mut expected = vec![0, 1, 2];
        expected.extend([0; 6]);
        expected.extend([1; 8]);
        expected.extend([2; 2]);
        assert_eq!(
            *log.borrow(),
            expected,
            "19 calls: 3 preflight + 16 measured"
        );
        let lines: Vec<&str> = report.lines().collect();
        assert_eq!(lines.len(), 4, "{report}");
        assert_eq!(lines[0], "{\"rounds\": 2, \"iterations\": 4}");
        for (line, (metric, count)) in lines[1..].iter().zip([("m0", 3), ("m1", 4), ("m2", 1)]) {
            let value = loads_text(&PyStr::new(line)).expect("a row");
            assert_eq!(text_of(member(&value, "metric")), metric);
            assert_eq!(text_of(member(&value, "unit")), "ns_per_call");
            assert_eq!(items(member(&value, "rounds")).len(), 2);
            assert_eq!(int_of(member(&value, "round_count")), 2);
            assert_eq!(int_of(member(&value, "iterations")), count);
        }
        // a wrong value of a LATER row: earlier rows were preflighted once, no loop ran, no report
        log.borrow_mut().clear();
        let wrong = Case {
            expected: dto(r#"["none"]"#),
            ..counted(&log, 1, ok_answer(), None)
        };
        let series = vec![
            Series {
                case: counted(&log, 0, ok_answer(), None),
                iterations: Some(3),
            },
            Series {
                case: wrong,
                iterations: Some(2),
            },
            Series {
                case: counted(&log, 2, ok_answer(), None),
                iterations: None,
            },
        ];
        assert!(run_series(&series, 2, 4).is_err());
        assert_eq!(*log.borrow(), vec![0, 1], "no timer, row 2 never reached");
        // a failing preflight of the last row: same
        log.borrow_mut().clear();
        let series = vec![
            Series {
                case: counted(&log, 0, ok_answer(), None),
                iterations: Some(3),
            },
            Series {
                case: counted(&log, 1, ok_answer(), Some(1)),
                iterations: None,
            },
        ];
        assert!(run_series(&series, 2, 4).is_err());
        assert_eq!(*log.borrow(), vec![0, 1]);
        // a failure inside a measured loop of the second series: an error, no report, the first series ran its own count only
        log.borrow_mut().clear();
        let series = vec![
            Series {
                case: counted(&log, 0, ok_answer(), None),
                iterations: Some(3),
            },
            Series {
                case: counted(&log, 1, ok_answer(), Some(3)),
                iterations: Some(5),
            },
        ];
        let error = run_series(&series, 2, 4).expect_err("no report");
        assert!(error
            .line()
            .to_utf8()
            .unwrap_or_default()
            .contains("failed inside a measured loop"));
        let mut expected = vec![0, 1];
        expected.extend([0; 6]);
        expected.extend([1, 1]);
        assert_eq!(*log.borrow(), expected);
    }

    #[test]
    fn the_program_runs_rows_with_their_own_iteration_counts_and_refuses_an_invalid_count_without_a_report(
    ) {
        let bundle = load("bundle.json");
        let (resolve, manifest) = (saved(&bundle, "api-147"), saved(&bundle, "api-159"));
        let work = area("bench-mixed");
        build_files(member(member(manifest, "request"), "files"), &work.root);
        let root = root_text(&work);
        let with_count = |metric: &str, case: &PyValue, count: Option<&str>| -> PyValue {
            let mut value = row(
                metric,
                case,
                &root,
                false,
                member(member(case, "original"), "value"),
            );
            if let (Some(count), PyValue::Dict(members)) = (count, &mut value) {
                members.set("iterations", dto(count));
            }
            value
        };
        let good = write_rows(
            &work,
            "mixed.json",
            vec![
                with_count("resolve_api", resolve, Some("200")),
                with_count("load_manifest_api", manifest, Some("5")),
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
            .zip([("resolve_api", 200), ("load_manifest_api", 5)])
        {
            let value = loads_text(&PyStr::new(line)).expect("a row");
            assert_eq!(text_of(member(&value, "metric")), metric);
            assert_eq!(items(member(&value, "rounds")).len(), 11);
            assert_eq!(int_of(member(&value, "round_count")), 11);
            assert_eq!(int_of(member(&value, "iterations")), count);
        }
        // a row without a count uses the process's
        let absent = write_rows(
            &work,
            "absent-count.json",
            vec![with_count("resolve_api", resolve, None)],
        );
        let output = run_bench(&work, &absent, &["--rounds", "2", "--iterations", "6"]);
        let text = String::from_utf8(output.stdout).expect("text");
        let value = loads_text(&PyStr::new(text.lines().nth(1).expect("a row"))).expect("a row");
        assert_eq!(int_of(member(&value, "iterations")), 6);
        // an invalid count ends the run: no report
        for bad in ["0", "-1", "1.5", "true", "null", "\"3\"", "[3]"] {
            let path = write_rows(
                &work,
                "bad-count.json",
                vec![with_count("resolve_api", resolve, Some(bad))],
            );
            let output = run_bench(&work, &path, &[]);
            assert_eq!(output.status.code(), Some(1), "{bad}");
            assert!(output.stdout.is_empty(), "{bad}");
            std::fs::remove_file(&path).expect("remove");
        }
    }

    #[test]
    fn an_unknown_operation_a_missing_expected_value_a_bad_profile_and_a_bad_option_end_the_run_without_a_report(
    ) {
        let work = area("bench-refusals");
        let rows = [
            r#"[{"metric": "m", "operation": "resolve_build_track.nothing", "args": [], "kwargs": {}, "expected": ["none"]}]"#,
            r#"[{"metric": "m", "operation": "other.resolve", "args": [], "kwargs": {}, "expected": ["none"]}]"#,
            r#"[{"metric": "m", "operation": "resolve_build_track.norm_release", "args": ["2.11"], "kwargs": {}}]"#,
            r#"[{"metric": "m", "operation": "resolve_build_track.norm_release", "args": ["2.11"], "kwargs": {}, "profile": "other", "expected": ["none"]}]"#,
            r#"[{"metric": "m", "operation": "resolve_build_track.norm_release", "args": ["2.11"], "kwargs": {}, "globals": {"other": 1}, "expected": ["none"]}]"#,
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
