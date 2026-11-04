//! Descriptor arguments of `_load_json` / `parse_engine_log`: a nonzero `int` or `bool` path is `open(fd)` in the
//! original. It reads from the **current offset** of the descriptor's shared open file description to its end,
//! then closes that descriptor number (also when the read or the decode fails, and never when the number is not
//! open or names a directory, where `open` itself raises). `0` and `False` are falsey: the functions return before
//! opening anything, so stdin stays open.
//!
//! The 22 scenarios in `checks/data/lite_metrics/descriptor_cases.json` were captured from the original
//! (`lite_metrics.py` at head `831068cb5777798cc8209374ecb42e0657d671a4`) in an isolated child process each: results, the
//! shared offset observed through a duplicate descriptor, whether each number is still open afterwards, whether a
//! newly opened file reuses the freed number, and for the standard-stream cases the offset and bytes of the
//! file the child's stdout was.
//!
//! This is a `harness = false` target. The same executable is both the checker and, re-executed with
//! `LITE_FD_CHILD_CASE`, the isolated child that runs one scenario against the native functions: closing
//! descriptors (fd 1 included) therefore never touches the test runner's own, and a libtest banner cannot disturb
//! the redirected stdout. Only the child's result file carries its observations.

#[path = "common.rs"]
mod common;

use common::{data_path, load_json_file, same};
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::os::fd::{AsRawFd, IntoRawFd};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use sweep_stop::lite_metrics::{load_json, parse_engine_log};
use sweep_stop::pyjson::loads;
use sweep_stop::pyvalue::{PyDict, PyValue};

const CHILD_CASE: &str = "LITE_FD_CHILD_CASE";
const CHILD_SCRATCH: &str = "LITE_FD_CHILD_SCRATCH";
const CHILD_RESULT: &str = "LITE_FD_CHILD_RESULT";

fn text(value: &PyValue) -> String {
    match value.peel() {
        PyValue::Str(text) => text.to_utf8().expect("UTF-8 text"),
        other => panic!("not a string: {}", other.repr()),
    }
}

fn dict(value: &PyValue) -> &PyDict {
    match value.peel() {
        PyValue::Dict(map) => map,
        other => panic!("not an object: {}", other.repr()),
    }
}

fn list(value: &PyValue) -> &[PyValue] {
    match value.peel() {
        PyValue::List(items) => items,
        other => panic!("not a list: {}", other.repr()),
    }
}

fn field<'a>(map: &'a PyDict, key: &str) -> &'a PyValue {
    map.get(key).unwrap_or_else(|| panic!("no field {key}"))
}

fn integer(value: &PyValue) -> u64 {
    match value.peel() {
        PyValue::Int(number) => u64::try_from(number.clone()).expect("small integer"),
        other => panic!("not an integer: {}", other.repr()),
    }
}

/// File content of a corpus entry: text, or `{hex: ...}` for bytes.
fn content_bytes(value: &PyValue) -> Vec<u8> {
    match value.peel() {
        PyValue::Dict(map) => {
            let hex = text(field(map, "hex"));
            (0..hex.len())
                .step_by(2)
                .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).expect("hex"))
                .collect()
        }
        other => text(other).into_bytes(),
    }
}

fn open_with(path: &Path, access: &str) -> File {
    let mut options = OpenOptions::new();
    match access {
        "r" => options.read(true),
        "w" => options.write(true),
        "rw" => options.read(true).write(true),
        other => panic!("unknown access {other}"),
    };
    options.open(path).expect("open scenario file")
}

fn is_open(number: i32) -> bool {
    std::fs::metadata(format!("/proc/self/fd/{number}")).is_ok()
}

fn find_case(corpus: &PyValue, id: &str) -> PyValue {
    list(corpus)
        .iter()
        .find(|case| text(field(dict(case), "id")) == id)
        .expect("case")
        .clone()
}

fn exception_record(class: &str, message: &str) -> PyValue {
    let mut map = PyDict::new();
    map.set("kind", PyValue::str("exception"));
    map.set("type", PyValue::str(class));
    map.set("message", PyValue::str(message));
    PyValue::Dict(map)
}

// ── the isolated child ───────────────────────────────────────────────────────────

fn child(case_id: &str) {
    let corpus = load_json_file(&data_path("lite_metrics/descriptor_cases.json"));
    let case = find_case(&corpus, case_id);
    let case = dict(&case);
    let scratch = PathBuf::from(std::env::var_os(CHILD_SCRATCH).expect("scratch"));
    let result_path = PathBuf::from(std::env::var_os(CHILD_RESULT).expect("result path"));
    for (name, content) in dict(field(case, "files")).iter() {
        std::fs::write(
            scratch.join(name.to_utf8().expect("name")),
            content_bytes(content),
        )
        .expect("write scenario file");
    }
    // Numbers that name a descriptor handed to the function under test (it takes ownership and closes it) and the
    // duplicates this child keeps to observe the shared offset.
    let mut numbers: BTreeMap<String, i32> = BTreeMap::new();
    let mut duplicates: BTreeMap<String, File> = BTreeMap::new();
    for open in list(field(case, "opens")) {
        let open = dict(open);
        let mut file = open_with(
            &scratch.join(text(field(open, "file"))),
            &text(field(open, "access")),
        );
        let offset = integer(field(open, "offset"));
        if offset != 0 {
            file.seek(SeekFrom::Start(offset)).expect("seek");
        }
        if let Some(PyValue::Str(name)) = open.get("dup") {
            duplicates.insert(
                name.to_utf8().expect("name"),
                file.try_clone().expect("duplicate"),
            );
        }
        let sym = text(field(open, "sym"));
        let number = file.as_raw_fd();
        numbers.insert(sym, number);
        if matches!(open.get("close_now"), Some(PyValue::Bool(true))) {
            drop(file);
        } else {
            // Ownership of the number passes to the function under test, as the original's `open(fd)` takes it.
            let _ = file.into_raw_fd();
        }
    }
    let mut results = Vec::new();
    for call in list(field(case, "calls")) {
        let call = dict(call);
        let path = match field(call, "path").peel() {
            PyValue::Dict(sym) => PyValue::int(numbers[&text(field(sym, "sym"))]),
            other => other.clone(),
        };
        let record = match text(field(call, "op")).as_str() {
            "_load_json" => match load_json(&path) {
                Ok(value) => value_record(value.unwrap_or(PyValue::None)),
                Err(failure) => exception_record(
                    failure.class,
                    &failure.message.to_utf8().unwrap_or_default(),
                ),
            },
            "parse_engine_log" => match parse_engine_log(&path) {
                Ok(engine) => value_record(engine.to_value()),
                Err(failure) => exception_record(
                    failure.class,
                    &failure.message.to_utf8().unwrap_or_default(),
                ),
            },
            other => panic!("unknown operation {other}"),
        };
        results.push(record);
    }
    let mut out = PyDict::new();
    out.set("results", PyValue::List(results));
    let mut offsets = PyDict::new();
    for name in list(field(case, "observe_offsets")) {
        let name = text(name);
        let position = duplicates
            .get_mut(&name)
            .expect("duplicate")
            .stream_position()
            .expect("position");
        offsets.set(&name, PyValue::int(position));
    }
    out.set("offsets", PyValue::Dict(offsets));
    let mut open_after = PyDict::new();
    for name in list(field(case, "observe_closed")) {
        let name = text(name);
        let open = duplicates.contains_key(&name) || is_open(numbers[&name]);
        open_after.set(&name, PyValue::Bool(open));
    }
    out.set("open_after", PyValue::Dict(open_after));
    let mut stdio = PyDict::new();
    for number in list(field(case, "observe_stdio")) {
        let number = i32::try_from(integer(number)).expect("descriptor");
        stdio.set(&number.to_string(), PyValue::Bool(is_open(number)));
    }
    out.set("stdio_open_after", PyValue::Dict(stdio));
    if let PyValue::Dict(reuse) = field(case, "reuse").peel() {
        let probe = File::open(scratch.join(text(field(reuse, "file")))).expect("reuse probe");
        let same_number = probe.as_raw_fd() == numbers[&text(field(reuse, "same_as"))];
        out.set("reuse_same_number", PyValue::Bool(same_number));
    }
    let json = PyValue::Dict(out).dumps_ascii().expect("serialisable");
    File::create(result_path)
        .and_then(|mut file| file.write_all(json.as_bytes()))
        .expect("write result");
}

fn value_record(value: PyValue) -> PyValue {
    let mut map = PyDict::new();
    map.set("kind", PyValue::str("value"));
    map.set("value", value);
    PyValue::Dict(map)
}

// ── the checker ──────────────────────────────────────────────────────────────────

fn run_case(case: &PyValue, exe: &Path) -> Vec<String> {
    let case = dict(case);
    let id = text(field(case, "id"));
    let root = std::env::temp_dir().join(format!("lite_metrics_fd_{}_{id}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("scratch");
    let result_path = root.join("result.json");
    let mut command = Command::new(exe);
    command
        .env(CHILD_CASE, &id)
        .env(CHILD_SCRATCH, &root)
        .env(CHILD_RESULT, &result_path)
        .stdin(Stdio::null())
        .stderr(Stdio::piped());
    let mut stdout_file: Option<(File, PathBuf)> = None;
    if let PyValue::Dict(config) = field(case, "stdout").peel() {
        let path = root.join(text(field(config, "file")));
        std::fs::write(&path, content_bytes(field(config, "content"))).expect("stdout file");
        let mut file = open_with(&path, &text(field(config, "access")));
        file.seek(SeekFrom::Start(integer(field(config, "offset"))))
            .expect("seek");
        command.stdout(Stdio::from(file.try_clone().expect("duplicate")));
        stdout_file = Some((file, path));
    } else {
        command.stdout(Stdio::null());
    }
    let output = command.output().expect("run child");
    let mut problems = Vec::new();
    let expected = dict(field(case, "expected"));
    if output.status.code() != Some(i32::try_from(integer(field(expected, "exit"))).unwrap()) {
        problems.push(format!(
            "{id}: child status {:?}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    match std::fs::read_to_string(&result_path) {
        Ok(json) => {
            let actual = loads(&json).expect("child result is JSON");
            let want = field(expected, "result");
            // Only the observations the scenario asked for are in the corpus; compare the same keys.
            let (actual, want) = (dict(&actual), dict(want));
            for (key, wanted) in want.iter() {
                let key = key.to_utf8().expect("key");
                match actual.get(&key) {
                    Some(found) if same(found, wanted) => {}
                    found => problems.push(format!(
                        "{id}: {key}: native {} != original {}",
                        found.map_or("<absent>".to_string(), PyValue::repr),
                        wanted.repr()
                    )),
                }
            }
        }
        Err(error) => problems.push(format!("{id}: no result file: {error}")),
    }
    if let Some((mut file, path)) = stdout_file {
        let offset = file.stream_position().expect("position");
        if Some(offset) != expected.get("stdout_offset_after").map(integer) {
            problems.push(format!(
                "{id}: stdout offset after = {offset}, original {}",
                expected
                    .get("stdout_offset_after")
                    .map_or(String::new(), PyValue::repr)
            ));
        }
        let bytes = std::fs::read(path).expect("stdout file after");
        let wanted = text(field(expected, "stdout_file_bytes_after"));
        if base64_encode(&bytes) != wanted {
            problems.push(format!(
                "{id}: stdout file bytes differ from the original's"
            ));
        }
    }
    let _ = std::fs::remove_dir_all(&root);
    problems
}

fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, &b)| n | (u32::from(b) << (16 - 8 * i)));
        for i in 0..=chunk.len() {
            out.push(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize] as char);
        }
        for _ in chunk.len()..3 {
            out.push('=');
        }
    }
    out
}

fn main() {
    if let Some(case_id) = std::env::var_os(CHILD_CASE) {
        child(&case_id.to_string_lossy());
        return;
    }
    let corpus = load_json_file(&data_path("lite_metrics/descriptor_cases.json"));
    let cases = list(&corpus);
    assert_eq!(cases.len(), 22, "the captured corpus has 22 scenarios");
    let exe = std::env::current_exe().expect("own path");
    let mut problems = Vec::new();
    for case in cases {
        let found = run_case(case, &exe);
        println!(
            "descriptor case {} ... {}",
            text(field(dict(case), "id")),
            if found.is_empty() { "ok" } else { "FAILED" }
        );
        problems.extend(found);
    }
    if !problems.is_empty() {
        eprintln!("{}", problems.join("\n"));
        std::process::exit(1);
    }
    println!(
        "descriptor_cases: {} scenarios equal the original",
        cases.len()
    );
}
