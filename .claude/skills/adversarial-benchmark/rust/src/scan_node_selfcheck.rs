//! The native P1 self-check of `scan_node`: the 107 saved-raw rows of the accepted Original107 run (embedded golden, derived from the saved
//! raw by a pure format conversion) run through the private bridge exactly as the Python adapter calls it, over fixtures laid out in a
//! private temporary directory. It checks the native P1 only: it is **not** the legacy `scan_node.py --self-test` (that runs the original
//! Python), says nothing about P2/P3/CLI, and a clock row's hour is a literal from that row's recorded bracket, never the current time.
//! `scan_node --p1-self-check` runs it; the cargo check `scan_node_p1_original_cases` runs the same function.

use crate::scan_node_boundary::{FixedClock, StdFiles};
use crate::scan_node_bridge::{serve, Environment, PROTOCOL};
use agent_runtime_support::pyjson::loads_text;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

pub const GOLDEN: &str = include_str!("../checks/data/scan_node/p1_golden.json");
pub const TOKEN: &str = "$R50_FIXTURES";

pub fn parse(text: &str) -> PyValue {
    loads_text(&PyStr::new(text)).expect("JSON")
}

pub fn get<'a>(value: &'a PyValue, key: &str) -> &'a PyValue {
    match value.peel() {
        PyValue::Dict(map) => map.get(key).unwrap_or_else(|| panic!("no member {key}")),
        _ => panic!("not an object while reading {key}"),
    }
}

pub fn text_of(value: &PyValue) -> String {
    match value.peel() {
        PyValue::Str(own) => own.to_utf8().expect("utf-8"),
        other => panic!("not a string: {}", other.type_name()),
    }
}

pub fn items(value: &PyValue) -> &Vec<PyValue> {
    match value.peel() {
        PyValue::List(items) => items,
        _ => panic!("not a list"),
    }
}

pub fn canon(value: &PyValue) -> String {
    value.dumps_ascii().expect("dump")
}

fn fixtures_root() -> PathBuf {
    // one root per run: the checks of this file run in parallel threads
    static RUNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let run = RUNS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let root = std::env::temp_dir().join(format!("scan-node-p1-{}-{run}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("fixtures root");
    root
}

fn lay_out(root: &Path, row: &PyValue) {
    let fixture = get(row, "fixture");
    let case_dir = root.join(text_of(get(fixture, "case_dir")));
    std::fs::create_dir_all(&case_dir).expect("case dir");
    for entry in items(get(fixture, "files")) {
        let path = case_dir.join(text_of(get(entry, "path")));
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("parent");
        }
        let mode = u32::from_str_radix(&text_of(get(entry, "mode")), 8).expect("mode");
        if text_of(get(entry, "kind")) == "dir" {
            std::fs::create_dir_all(&path).expect("dir");
        } else {
            let hex = text_of(get(entry, "content_hex"));
            let bytes: Vec<u8> = (0..hex.len())
                .step_by(2)
                .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).expect("hex"))
                .collect();
            std::fs::write(&path, bytes).expect("write");
        }
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).expect("mode");
        if let PyValue::Str(after) = get(entry, "opaque_mode_after_receipt").peel() {
            let opaque = u32::from_str_radix(&after.to_utf8().expect("mode"), 8).expect("mode");
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(opaque)).expect("mode");
        }
    }
}

fn errno_class(errno: &str) -> String {
    match errno {
        "+d" => "PermissionError".to_string(),
        "+15" => "IsADirectoryError".to_string(),
        "+2" => "FileNotFoundError".to_string(),
        "+14" => "NotADirectoryError".to_string(),
        other => format!("unmapped errno {other}"),
    }
}

/// The expected exception of a row against the answer's `exc` (class, constructor and cause; frames are not carried).
fn compare_raised(expected: &PyValue, answer: &PyValue, problems: &mut Vec<String>, id: &str) {
    let exc = get(answer, "exc");
    let expected_cls = text_of(get(expected, "cls"));
    if text_of(get(exc, "cls")) != expected_cls {
        problems.push(format!(
            "{id}: class {} != {expected_cls}",
            text_of(get(exc, "cls"))
        ));
    }
    if canon(get(exc, "ctor")) != canon(get(expected, "ctor")) {
        problems.push(format!("{id}: constructor arguments differ"));
    }
    match (get(expected, "cause").peel(), get(exc, "cause").peel()) {
        (PyValue::None, PyValue::None) => {}
        (PyValue::Dict(want), PyValue::Dict(have)) => {
            let want = PyValue::Dict(want.clone());
            let have = PyValue::Dict(have.clone());
            // the original's OSError carries `args == (errno, strerror)` and `filename` as an attribute; the importer rebuilds
            // `OSError(errno, strerror, filename)`, which Python maps to the same subclass
            let want_ctor = items(get(&want, "ctor"));
            let have_ctor = items(get(&have, "ctor"));
            let attrs = get(&want, "attrs");
            let rebuilt = [&want_ctor[0], &want_ctor[1], get(attrs, "filename")];
            let have_all: Vec<&PyValue> = have_ctor.iter().collect();
            if rebuilt.len() != have_all.len()
                || rebuilt
                    .iter()
                    .zip(&have_all)
                    .any(|(a, b)| canon(a) != canon(b))
            {
                problems.push(format!("{id}: cause constructor differs"));
            }
            let errno = text_of(get(&want_ctor[0], "hex"));
            if errno_class(&errno) != text_of(get(&want, "cls")) {
                problems.push(format!(
                    "{id}: errno {errno} does not map to the original cause class"
                ));
            }
            if text_of(get(&have, "cls")) != "OSError" {
                problems.push(format!("{id}: the cause is not carried as OSError"));
            }
        }
        _ => problems.push(format!("{id}: cause presence differs")),
    }
}

/// The importer's numbering of the caller's objects (pre-order, parameters in order): containers and NaN floats get an `"id"`.
/// Returns the wire with ids and, per id, the path of the caller object (parameter name, then list index or dict key).
fn number_ids(params: &PyValue) -> (PyValue, Vec<(u64, PyValue)>) {
    fn walk(
        value: &PyValue,
        path: &mut Vec<PyValue>,
        next: &mut u64,
        paths: &mut Vec<(u64, PyValue)>,
    ) -> PyValue {
        let PyValue::Dict(wire) = value.peel() else {
            panic!("a wire value is an object")
        };
        let tag = text_of(wire.get("t").expect("tag"));
        let is_nan = tag == "float" && text_of(wire.get("repr").expect("repr")) == "nan";
        if !(is_nan || matches!(tag.as_str(), "list" | "tuple" | "dict")) {
            return value.clone();
        }
        let id = *next;
        *next += 1;
        paths.push((id, PyValue::List(path.clone())));
        let mut out = PyDict::new();
        for (key, member) in wire.iter() {
            let key_text = key.to_utf8().expect("key");
            let rebuilt = match (tag.as_str(), key_text.as_str()) {
                ("list" | "tuple", "v") => PyValue::List(
                    items(member)
                        .iter()
                        .enumerate()
                        .map(|(index, item)| {
                            path.push(PyValue::int(index));
                            let done = walk(item, path, next, paths);
                            path.pop();
                            done
                        })
                        .collect(),
                ),
                ("dict", "v") => PyValue::List(
                    items(member)
                        .iter()
                        .map(|pair| {
                            let pair = items(pair);
                            path.push(PyValue::Str(PyStr::new(&text_of(get(&pair[0], "v")))));
                            let done = walk(&pair[1], path, next, paths);
                            path.pop();
                            PyValue::List(vec![pair[0].clone(), done])
                        })
                        .collect(),
                ),
                _ => member.clone(),
            };
            out.set(&key_text, rebuilt);
            if key_text == "t" {
                out.set("id", PyValue::int(id));
            }
        }
        PyValue::Dict(out)
    }
    let mut numbered = PyDict::new();
    let mut paths = Vec::new();
    let mut next = 0;
    let PyValue::Dict(params) = params.peel() else {
        panic!("params is an object")
    };
    for (name, wire) in params.iter() {
        let mut path = vec![PyValue::Str(name.clone())];
        numbered.insert(name.clone(), walk(wire, &mut path, &mut next, &mut paths));
    }
    (PyValue::Dict(numbered), paths)
}

/// The answer value with every `argref` id replaced by the path of the caller object it names (the form the golden records).
fn argrefs_by_path(value: &PyValue, paths: &[(u64, PyValue)]) -> PyValue {
    match value.peel() {
        PyValue::Dict(map) => {
            if map.get("t").is_some_and(|t| text_of(t) == "argref") {
                let id = match map.get("id").expect("id").peel() {
                    PyValue::Int(n) => u64::try_from(n.clone()).expect("id"),
                    _ => panic!("argref id"),
                };
                let path = paths
                    .iter()
                    .find(|(known, _)| *known == id)
                    .expect("argref names a numbered object");
                let mut out = PyDict::new();
                out.set("t", PyValue::str("argref"));
                out.set("path", path.1.clone());
                return PyValue::Dict(out);
            }
            let mut out = PyDict::new();
            for (key, member) in map.iter() {
                out.insert(key.clone(), argrefs_by_path(member, paths));
            }
            PyValue::Dict(out)
        }
        PyValue::List(entries) => {
            PyValue::List(entries.iter().map(|e| argrefs_by_path(e, paths)).collect())
        }
        other => other.clone(),
    }
}

fn contains_argref(value: &PyValue) -> bool {
    match value.peel() {
        PyValue::Dict(map) => {
            map.get("t").is_some_and(|t| text_of(t) == "argref")
                || map.iter().any(|(_, m)| contains_argref(m))
        }
        PyValue::List(entries) => entries.iter().any(contains_argref),
        _ => false,
    }
}

pub struct Outcome {
    pub argref_rows: usize,
    pub executed: usize,
    pub returned: usize,
    pub raised: usize,
    pub by_plane: std::collections::BTreeMap<String, usize>,
    pub problems: Vec<String>,
}

pub fn run_all() -> Outcome {
    run_golden(GOLDEN)
}

pub fn run_golden(source: &str) -> Outcome {
    let root = fixtures_root();
    let golden_text = source.replace(TOKEN, root.to_str().expect("utf-8 root"));
    let golden = parse(&golden_text);
    let mut outcome = Outcome {
        argref_rows: 0,
        executed: 0,
        returned: 0,
        raised: 0,
        by_plane: Default::default(),
        problems: Vec::new(),
    };
    for row in items(get(&golden, "rows")) {
        let id = text_of(get(row, "id"));
        lay_out(&root, row);
        let mut request = PyDict::new();
        request.set("protocol", PyValue::str(PROTOCOL));
        request.set("op", get(row, "operation").clone());
        let (numbered, paths) = number_ids(get(row, "params"));
        request.set("params", numbered);
        let request = PyValue::Dict(request).dumps_ascii().expect("request");
        // only the clock rows carry a bracket; for every other row the hour is never read (and would fail loudly if it were)
        let hour = match row.peel() {
            PyValue::Dict(map) => map.get("clock").map_or_else(
                || String::from("not-a-clock-row"),
                |clock| text_of(get(clock, "hour")),
            ),
            _ => unreachable!("a row is an object"),
        };
        let clock = FixedClock {
            hour,
            provenance: "the local hour of the row's own recorded before/after bracket",
        };
        let environment = Environment::default();
        let answer = match serve(request.as_bytes(), &environment, &StdFiles, &clock) {
            Ok(text) => parse(&text),
            Err(message) => {
                outcome
                    .problems
                    .push(format!("{id}: the bridge refused: {message}"));
                continue;
            }
        };
        outcome.executed += 1;
        *outcome
            .by_plane
            .entry(text_of(get(row, "plane")))
            .or_default() += 1;
        let status = text_of(get(&answer, "status"));
        match text_of(get(row, "status")).as_str() {
            "RETURNED" => {
                outcome.returned += 1;
                if status != "returned" {
                    outcome
                        .problems
                        .push(format!("{id}: answered {status}, the original returned"));
                } else {
                    let planned = argrefs_by_path(get(&answer, "value"), &paths);
                    if contains_argref(&planned) {
                        outcome.argref_rows += 1;
                    }
                    if canon(&planned) != canon(get(row, "returned")) {
                        outcome.problems.push(format!(
                            "{id}: value or identity plan differs\n  have {}\n  want {}",
                            canon(&planned),
                            canon(get(row, "returned"))
                        ));
                    }
                }
            }
            "RAISED" => {
                outcome.raised += 1;
                if status != "raised" {
                    outcome
                        .problems
                        .push(format!("{id}: answered {status}, the original raised"));
                } else {
                    compare_raised(get(row, "raised"), &answer, &mut outcome.problems, &id);
                }
            }
            other => outcome
                .problems
                .push(format!("{id}: golden status {other}")),
        }
    }
    let _ = std::fs::remove_dir_all(&root);
    outcome
}

pub trait OwnerUid {
    fn uid_of_owner(&self) -> u32;
}

impl OwnerUid for std::fs::Metadata {
    fn uid_of_owner(&self) -> u32 {
        std::os::unix::fs::MetadataExt::uid(self)
    }
}

/// The self-check as the program runs it: the report lines and whether the native P1 passed.
pub fn report() -> (Vec<String>, bool) {
    let mut lines = vec![
        "scan_node P1 native self-check (embedded saved-raw rows; native P1 only - not the legacy scan_node.sh --self-test, no P2/P3/CLI)".to_string(),
    ];
    let uid = OwnerUid::uid_of_owner(&std::fs::metadata("/proc/self").expect("proc"));
    if uid == 0 {
        lines.push("FAIL: run as an ordinary user (a mode-000 fixture is readable by root)".into());
        return (lines, false);
    }
    let outcome = run_all();
    let expected = (107, 105, 2, 6);
    let shape = (
        outcome.executed,
        outcome.returned,
        outcome.raised,
        outcome.argref_rows,
    );
    lines.push(format!(
        "rows executed={} returned={} raised={} argref-plans={} planes={:?}",
        outcome.executed, outcome.returned, outcome.raised, outcome.argref_rows, outcome.by_plane
    ));
    for problem in &outcome.problems {
        lines.push(format!("FAIL {problem}"));
    }
    let ok = outcome.problems.is_empty() && shape == expected;
    if !ok && outcome.problems.is_empty() {
        lines.push(format!(
            "FAIL the row counts {shape:?} are not the golden's {expected:?}"
        ));
    }
    lines.push(if ok {
        "OK native P1 rows match the saved raw".into()
    } else {
        "FAILED".into()
    });
    (lines, ok)
}
