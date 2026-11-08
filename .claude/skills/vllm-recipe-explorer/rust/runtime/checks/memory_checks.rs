//! The registered checks of the RAM-gate owner (`preload_ram_gate.py`): the saved original observations, observed whole, and the
//! parts of the programs that touch the operating system, driven only with controlled machines.
//!
//! These checks need the `fixture` feature (`required-features`): the controlled machine exists only in that build, never in the
//! production programs. Nothing here reads the real `/proc/meminfo`, tests for or runs the real cache-drop helper, or starts `sudo`.
//!
//! * `reference_cases` runs the 178 saved function calls through `recipe_memory_fixture_api case` and the 25 saved command lines
//!   through `preload_ram_gate_fixture`, each in its own private directory rebuilt from the saved before-tree, and compares the whole
//!   control record (values, types, key order, exceptions, ordered provider/log/stderr trace, counters, streams, arguments after),
//!   the exit status, the raw streams and the complete tree after with the saved observation. The only rebasing is the token
//!   `@CASE@` (the capture directory); the Python host routing belongs to the Python layer and is not part of the saved record
//!   compared here. A command line's standard error is compared after the original's own traceback frames (its script and the
//!   collector's) were removed and nothing else. The original is never run: expected values are the saved observations.
//! * `production_cases` runs what the production service can run without touching the machine (the constants, the model-size
//!   functions, argument binding) and shows it refuses every request member that could substitute the machine.
//! * `session_cases` drives the gate session with a scripted caller against a controlled machine: ordered events, acknowledgements,
//!   abort, end of input and malformed acknowledgements, and what the machine was asked after each stop.
//! * `system_cases` runs the file parser at chunk edges (the saved expectations come from one stdlib probe of the language
//!   primitive, never of the original module) and the real timeout, kill and reap of the helper runner with harmless commands.
//! * `bench_cases` runs the benchmark program on a tiny schedule (diagnostic children, never the official measurement).

use agent_runtime_support::pyjson::loads_text;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use recipe_runtime::model_files::PrivateDirectory;
use recipe_runtime::observe::same_value;
use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// ── data ─────────────────────────────────────────────────────────────────────────────────────────

fn data(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("checks")
        .join("data")
        .join("memory_reference")
        .join(relative)
}

fn load(relative: &str) -> PyValue {
    let bytes = std::fs::read(data(relative)).expect("saved data");
    loads_text(&PyStr::from_surrogateescape(&bytes)).expect("saved JSON")
}

fn member<'a>(value: &'a PyValue, key: &str) -> &'a PyValue {
    match value {
        PyValue::Dict(map) => map.get(key).unwrap_or_else(|| panic!("missing {key}")),
        other => panic!("not an object: {}", other.repr()),
    }
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

/// Every text of the value with `@CASE@` replaced by `root`.
fn rebase(value: &PyValue, root: &str) -> PyValue {
    match value {
        PyValue::Str(text) => PyValue::str(&text.to_utf8().expect("text").replace("@CASE@", root)),
        PyValue::List(entries) => {
            PyValue::List(entries.iter().map(|each| rebase(each, root)).collect())
        }
        PyValue::Dict(map) => {
            let mut copy = PyDict::new();
            for (key, each) in map.iter() {
                copy.insert(key.clone(), rebase(each, root));
            }
            PyValue::Dict(copy)
        }
        other => other.clone(),
    }
}

fn without(value: &PyValue, names: &[&str]) -> PyValue {
    let PyValue::Dict(map) = value else {
        panic!("not an object")
    };
    let mut copy = PyDict::new();
    for (key, each) in map.iter() {
        if !names.iter().any(|name| key.equals(name)) {
            copy.insert(key.clone(), each.clone());
        }
    }
    PyValue::Dict(copy)
}

// ── trees ────────────────────────────────────────────────────────────────────────────────────────

type Tree = BTreeMap<String, (String, u32, Vec<u8>)>;

fn entry_bytes(bundle: &PyValue, entry: &PyValue) -> Vec<u8> {
    match entry {
        PyValue::Dict(map) if map.contains_key("source") => hex_decode(&text_of(member(
            member(bundle, "sources"),
            &text_of(member(entry, "source")),
        ))),
        PyValue::Dict(map) if map.contains_key("bytes_hex") => {
            hex_decode(&text_of(member(entry, "bytes_hex")))
        }
        _ => Vec::new(),
    }
}

/// The saved tree `index` as expected observations: path, kind, mode, bytes.
fn expected_tree(bundle: &PyValue, index: &PyValue) -> Tree {
    let PyValue::Int(number) = index else {
        panic!("a tree index")
    };
    let at = usize::try_from(number.clone()).expect("an index");
    items(&items(member(bundle, "trees"))[at])
        .iter()
        .map(|entry| {
            let mode = match member(entry, "mode") {
                PyValue::Int(mode) => u32::try_from(mode.clone()).expect("mode"),
                other => panic!("{}", other.repr()),
            };
            (
                text_of(member(entry, "path")),
                (
                    text_of(member(entry, "kind")),
                    mode,
                    entry_bytes(bundle, entry),
                ),
            )
        })
        .collect()
}

/// Writes the saved tree `index` into the (new, empty) directory `root`.
fn build_tree(bundle: &PyValue, index: &PyValue, root: &Path) {
    let expected = expected_tree(bundle, index);
    let mut directories: Vec<(PathBuf, u32)> = Vec::new();
    for (path, (kind, mode, bytes)) in &expected {
        let target = if path == "." {
            root.to_path_buf()
        } else {
            root.join(path)
        };
        match kind.as_str() {
            "dir" => {
                if path != "." {
                    std::fs::create_dir(&target).expect("a directory");
                }
                directories.push((target, *mode));
            }
            _ => {
                std::fs::write(&target, bytes).expect("a file");
                std::fs::set_permissions(&target, std::fs::Permissions::from_mode(*mode))
                    .expect("a mode");
            }
        }
    }
    for (target, mode) in directories.into_iter().rev() {
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(mode)).expect("a mode");
    }
}

fn walk(root: &Path, path: &Path, rows: &mut Tree) {
    let info = std::fs::symlink_metadata(path).expect("an entry");
    let name = path.strip_prefix(root).map_or_else(
        |_| ".".to_string(),
        |rest| rest.to_string_lossy().into_owned(),
    );
    let name = if name.is_empty() {
        ".".to_string()
    } else {
        name
    };
    let mode = info.permissions().mode() & 0o7777;
    if info.is_dir() {
        rows.insert(name, ("dir".to_string(), mode, Vec::new()));
        let mut children: Vec<PathBuf> = std::fs::read_dir(path)
            .expect("a directory")
            .map(|entry| entry.expect("entry").path())
            .collect();
        children.sort();
        for child in children {
            walk(root, &child, rows);
        }
    } else {
        assert!(
            info.is_file(),
            "only files and directories are expected: {name}"
        );
        // A file nobody may read is observed by its size (the saved fixture of that case is empty).
        let bytes = if mode & 0o400 == 0 {
            assert_eq!(info.len(), 0, "an unreadable file is empty");
            Vec::new()
        } else {
            std::fs::read(path).expect("bytes")
        };
        rows.insert(name, ("file".to_string(), mode, bytes));
    }
}

/// The whole tree under `root`.
fn snapshot(root: &Path) -> Tree {
    let mut rows = Tree::new();
    walk(root, root, &mut rows);
    rows
}

fn tree_difference(want: &Tree, got: &Tree) -> Vec<String> {
    let mut problems = Vec::new();
    for (path, entry) in want {
        match got.get(path) {
            None => problems.push(format!("missing {path}")),
            Some(found) if found != entry => problems.push(format!("changed {path}")),
            Some(_) => {}
        }
    }
    for path in got.keys() {
        if !want.contains_key(path) {
            problems.push(format!("extra {path}"));
        }
    }
    problems
}

// ── running programs ─────────────────────────────────────────────────────────────────────────────

struct Case {
    _area: PrivateDirectory,
    area: PathBuf,
    root: PathBuf,
}

/// A new private area with the case root rebuilt from the saved tree (and the tree it must end as).
fn new_case(bundle: &PyValue, name: &str, before: &PyValue) -> Case {
    let area = PrivateDirectory::create(&format!("memory-{name}-")).expect("a private directory");
    let root = area.path().join("case");
    std::fs::create_dir(&root).expect("the case root");
    build_tree(bundle, before, &root);
    Case {
        area: area.path().to_path_buf(),
        _area: area,
        root,
    }
}

fn write_json(path: &Path, value: &PyValue) {
    std::fs::write(path, value.dumps_ascii().expect("JSON")).expect("a file");
}

fn root_text(case: &Case) -> String {
    case.root.to_str().expect("a path").to_string()
}

/// The control record the program wrote (a new file it was told to create).
fn read_control(path: &Path) -> PyValue {
    let bytes = std::fs::read(path).expect("the control record");
    loads_text(&PyStr::from_surrogateescape(&bytes)).expect("control JSON")
}

fn fixture_environment(command: &mut Command, case: &Case) {
    command
        .env_clear()
        .env("TMPDIR", case.root.join("private-tmp"))
        .current_dir(&case.root)
        .stdin(Stdio::null());
}

/// What descriptor 0 of a service started over a protocol descriptor is: a file it may read, or one it can only write to (so that
/// any read of standard input would fail loudly).
enum StandardInput<'a> {
    Reads(&'a Path),
    WriteOnly(&'a Path),
}

/// Starts `program args...` the way a caller passes a protocol descriptor (Python's `pass_fds`): the pipe the caller writes to is
/// descriptor 3 of the program, and descriptor 0 is `standard_input`, different bytes altogether. Returns the command (streams
/// piped) and the write end of the protocol pipe.
fn over_descriptor(
    program: &str,
    args: &[&str],
    standard_input: &StandardInput<'_>,
) -> (Command, std::io::PipeWriter) {
    let (reader, writer) = std::io::pipe().expect("a pipe");
    let (redirect, path) = match standard_input {
        StandardInput::Reads(path) => ("0<", path),
        StandardInput::WriteOnly(path) => ("0>", path),
    };
    let script = format!("exec \"$@\" 3<&0 {redirect}\"$F0\"");
    let mut command = Command::new("/bin/sh");
    command
        .args(["-c", &script, "sh", program])
        .args(args)
        .env_clear()
        .env("F0", path)
        .stdin(Stdio::from(reader))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    (command, writer)
}

// ── saved traceback projection ───────────────────────────────────────────────────────────────────

const HEADER: &[u8] = b"Traceback (most recent call last):\n";

/// The saved standard error with the original's traceback frames removed and nothing else: the text before the header stays,
/// each frame of the original script or of the collector (a file the original run was started through) goes with its source
/// lines, and the whole final block stays. A frame of any other file, a header without frames or a frame without a final block is
/// refused.
fn project_traceback(saved: &[u8]) -> Result<Vec<u8>, String> {
    let header = if saved.starts_with(HEADER) {
        Some(0)
    } else {
        saved
            .windows(HEADER.len() + 1)
            .position(|window| window[0] == b'\n' && &window[1..] == HEADER)
            .map(|at| at + 1)
    };
    let Some(start) = header else {
        return Ok(saved.to_vec());
    };
    let mut kept = saved[..start].to_vec();
    let mut rest = &saved[start + HEADER.len()..];
    let allowed = |file: &[u8]| -> bool {
        file == b"@CASE@/node/.claude/skills/vllm-recipe-explorer/scripts/preload_ram_gate.py"
            || file.ends_with(b"/r27_original_one_call.py")
    };
    let mut frames = 0;
    while rest.starts_with(b"  File \"") {
        let end = rest
            .iter()
            .position(|&byte| byte == b'\n')
            .map_or(rest.len(), |at| at + 1);
        let line = &rest[..end];
        let after = &line[b"  File \"".len()..];
        let Some(quote) = after.iter().position(|&byte| byte == b'"') else {
            return Err("a frame without a file".to_string());
        };
        if !allowed(&after[..quote]) {
            return Err(format!(
                "a frame outside the original's modules: {}",
                String::from_utf8_lossy(line)
            ));
        }
        frames += 1;
        rest = &rest[end..];
        while rest.starts_with(b"    ") {
            let end = rest
                .iter()
                .position(|&byte| byte == b'\n')
                .map_or(rest.len(), |at| at + 1);
            rest = &rest[end..];
        }
    }
    if frames == 0 || rest.is_empty() || rest.starts_with(b" ") {
        return Err(
            "no frame, no final block, or a final block that starts with a space".to_string(),
        );
    }
    kept.extend_from_slice(rest);
    Ok(kept)
}

// ── reference cases ──────────────────────────────────────────────────────────────────────────────

mod reference_cases {
    use super::*;

    /// Runs one function case through the fixture API; the problems found (empty = the saved observation reproduced).
    fn api_problems(bundle: &PyValue, case: &PyValue) -> Vec<String> {
        let request = member(case, "request");
        let id = text_of(member(request, "id"));
        let work = new_case(bundle, &id, member(case, "before"));
        let (spec, control) = (work.area.join("spec.json"), work.area.join("control.json"));
        write_json(&spec, request);
        let mut command = Command::new(env!("CARGO_BIN_EXE_recipe_memory_fixture_api"));
        command
            .arg("case")
            .arg("--spec")
            .arg(&spec)
            .arg("--root")
            .arg(&work.root)
            .arg("--control")
            .arg(&control);
        fixture_environment(&mut command, &work);
        let output = command.output().expect("run the fixture API");
        let mut problems = Vec::new();
        let wanted_status = match member(case, "exit_code") {
            PyValue::Int(number) => number.to_string(),
            other => panic!("{}", other.repr()),
        };
        if output.status.code().map(|code| code.to_string()) != Some(wanted_status.clone()) {
            problems.push(format!(
                "status {:?} want {wanted_status}",
                output.status.code()
            ));
        }
        let root = root_text(&work);
        let (want_out, want_err) = (
            replace_bytes(
                &hex_decode(&text_of(member(case, "stdout_hex"))),
                "@CASE@",
                root.as_bytes(),
            ),
            replace_bytes(
                &hex_decode(&text_of(member(case, "stderr_hex"))),
                "@CASE@",
                root.as_bytes(),
            ),
        );
        if output.stdout != want_out || output.stderr != want_err {
            problems.push(format!(
                "streams differ: {:?} {:?}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        if control.exists() {
            let got = read_control(&control);
            let want = rebase(member(case, "original"), &root);
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
        let after = expected_tree(bundle, member(case, "after"));
        problems.extend(tree_difference(&after, &snapshot(&work.root)));
        problems
            .into_iter()
            .map(|problem| format!("{id}: {problem}"))
            .collect()
    }

    #[test]
    fn the_178_saved_function_calls_are_reproduced_whole_with_their_trace_and_tree() {
        let bundle = load("bundle.json");
        let cases = items(member(&bundle, "api"));
        assert_eq!(cases.len(), 178);
        let failures: Vec<String> = cases
            .iter()
            .flat_map(|case| api_problems(&bundle, case))
            .collect();
        assert!(
            failures.is_empty(),
            "{} problems:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    fn cli_problems(bundle: &PyValue, case: &PyValue) -> Vec<String> {
        let request = member(case, "request");
        let id = text_of(member(request, "id"));
        let work = new_case(bundle, &id, member(case, "before"));
        let (spec, control) = (work.area.join("spec.json"), work.area.join("control.json"));
        write_json(&spec, request);
        let mut command = Command::new(env!("CARGO_BIN_EXE_preload_ram_gate_fixture"));
        command
            .arg("--spec")
            .arg(&spec)
            .arg("--root")
            .arg(&work.root)
            .arg("--control")
            .arg(&control);
        fixture_environment(&mut command, &work);
        let output = command.output().expect("run the fixture command line");
        let root = root_text(&work);
        let root_bytes = root.as_bytes();
        let mut problems = Vec::new();
        let wanted_status = match member(case, "exit_code") {
            PyValue::Int(number) => number.to_string(),
            other => panic!("{}", other.repr()),
        };
        if output.status.code().map(|code| code.to_string()) != Some(wanted_status.clone()) {
            problems.push(format!(
                "status {:?} want {wanted_status}",
                output.status.code()
            ));
        }
        let want_out = replace_bytes(
            &hex_decode(&text_of(member(case, "stdout_hex"))),
            "@CASE@",
            root_bytes,
        );
        if output.stdout != want_out {
            problems.push(format!(
                "stdout differs:\n--- want\n{}\n--- got\n{}",
                String::from_utf8_lossy(&want_out),
                String::from_utf8_lossy(&output.stdout)
            ));
        }
        let saved_err = hex_decode(&text_of(member(case, "stderr_hex")));
        match project_traceback(&saved_err) {
            Ok(projected) => {
                let want_err = replace_bytes(&projected, "@CASE@", root_bytes);
                if output.stderr != want_err {
                    problems.push(format!(
                        "stderr differs:\n--- want\n{}\n--- got\n{}",
                        String::from_utf8_lossy(&want_err),
                        String::from_utf8_lossy(&output.stderr)
                    ));
                }
            }
            Err(problem) => problems.push(format!(
                "the saved diagnostic is not in the expected shape: {problem}"
            )),
        }
        if control.exists() {
            let got = read_control(&control);
            let want = rebase(member(case, "original"), &root);
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
        let after = expected_tree(bundle, member(case, "after"));
        problems.extend(tree_difference(&after, &snapshot(&work.root)));
        problems
            .into_iter()
            .map(|problem| format!("{id} {}: {problem}", member(request, "args").repr()))
            .collect()
    }

    #[test]
    fn the_25_saved_command_lines_end_as_the_original_did_with_their_trace_streams_and_tree() {
        let bundle = load("bundle.json");
        let cases = items(member(&bundle, "cli"));
        assert_eq!(cases.len(), 25);
        let failures: Vec<String> = cases
            .iter()
            .flat_map(|case| cli_problems(&bundle, case))
            .collect();
        assert!(
            failures.is_empty(),
            "{} problems:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    #[test]
    fn the_diagnostic_comparator_removes_only_original_frames_and_keeps_the_whole_final_block() {
        let saved: &[u8] = b"[preload-gate] first\nTraceback (most recent call last):\n  File \"/x/r27_original_one_call.py\", line 9, in <module>\n    x()\n    ^^^\n  File \"@CASE@/node/.claude/skills/vllm-recipe-explorer/scripts/preload_ram_gate.py\", line 1, in f\n    y\nTypeError: a\n    indented second line\nthird\n";
        assert_eq!(
            project_traceback(saved).expect("projected"),
            b"[preload-gate] first\nTypeError: a\n    indented second line\nthird\n".to_vec()
        );
        assert_eq!(
            project_traceback(b"usage: x\n").expect("plain"),
            b"usage: x\n".to_vec()
        );
        assert!(project_traceback(b"Traceback (most recent call last):\n  File \"/elsewhere/other.py\", line 1, in f\n    x\nE: m\n").is_err());
        assert!(project_traceback(b"Traceback (most recent call last):\n  File \"/x/r27_original_one_call.py\", line 1, in f\n    x\n").is_err());
        assert!(project_traceback(b"Traceback (most recent call last):\nE: m\n").is_err());
        // The record comparison is strict about types, key order and every value.
        let record = loads_text(&PyStr::new(r#"{"a": 1, "b": [true, null]}"#)).expect("json");
        for changed in [
            r#"{"a": 1.0, "b": [true, null]}"#,
            r#"{"b": [true, null], "a": 1}"#,
            r#"{"a": 1, "b": [1, null]}"#,
        ] {
            let other = loads_text(&PyStr::new(changed)).expect("json");
            assert!(!same_value(&record, &other), "{changed}");
        }
    }
}

// ── the production service ───────────────────────────────────────────────────────────────────────

mod production_cases {
    use super::*;

    fn run_service(input: &str, arguments: &[&str]) -> std::process::Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_recipe_memory_api"))
            .args(arguments)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start the service");
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(input.as_bytes())
            .expect("send");
        child.wait_with_output().expect("the service ends")
    }

    fn records(output: &std::process::Output) -> Vec<PyValue> {
        String::from_utf8(output.stdout.clone())
            .expect("text")
            .lines()
            .map(|line| loads_text(&PyStr::new(line)).expect("a record"))
            .collect()
    }

    #[test]
    fn the_constants_are_the_ones_the_original_source_declares() {
        let bundle = load("bundle.json");
        let output = run_service("", &["constants"]);
        assert_eq!(output.status.code(), Some(0));
        let got = loads_text(&PyStr::from_surrogateescape(&output.stdout)).expect("constants");
        assert!(
            same_value(member(&bundle, "constants"), &got),
            "{}",
            got.repr()
        );
    }

    /// The saved calls that never reach the machine (model sizes, binding errors) through the production service, one process each.
    #[test]
    fn the_saved_model_size_and_binding_calls_agree_through_the_production_service() {
        let bundle = load("bundle.json");
        let cases = items(member(&bundle, "api"));
        let mut failures = Vec::new();
        let mut run = 0;
        for case in cases {
            let request = member(case, "request");
            let id = text_of(member(request, "id"));
            let number: usize = id.trim_start_matches("api-").parse().expect("a number");
            if !(146..=177).contains(&number) {
                continue;
            }
            run += 1;
            let work = new_case(&bundle, &id, member(case, "before"));
            let root = root_text(&work);
            let mut line = PyDict::new();
            line.set("id", member(request, "id").clone());
            line.set(
                "operation",
                PyValue::str(&format!(
                    "preload_ram_gate.{}",
                    text_of(member(request, "function"))
                )),
            );
            line.set("args", rebase(member(request, "args"), &root));
            line.set("kwargs", rebase(member(request, "kwargs"), &root));
            let text = PyValue::Dict(line).dumps_ascii().expect("a request");
            let output = run_service(&format!("{text}\n"), &["evaluate-jsonl"]);
            let got = records(&output);
            let want = rebase(
                &without(member(case, "original"), &["trace", "context_after"]),
                &root,
            );
            if output.status.code() != Some(0) || got.len() != 1 || !same_value(&want, &got[0]) {
                failures.push(format!(
                    "{id}: want {} got {:?}",
                    want.repr(),
                    got.iter().map(PyValue::repr).collect::<Vec<_>>()
                ));
            }
        }
        assert_eq!(run, 32);
        assert!(
            failures.is_empty(),
            "{} differ:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    /// Nothing in a request can substitute the machine: every member beyond the four is refused, before anything runs.
    #[test]
    fn a_request_that_names_anything_beyond_the_four_members_is_refused_without_running() {
        let requests = [
            r#"{"id": 1, "operation": "preload_ram_gate.gate", "args": [1], "context": {"mem": [99999999]}}"#,
            r#"{"id": 2, "operation": "preload_ram_gate.gate", "args": [1], "provider": "fixed"}"#,
            r#"{"id": 3, "operation": "preload_ram_gate.mem_available_mib", "meminfo": "/tmp/x"}"#,
            r#"{"id": 4, "operation": "preload_ram_gate.gate", "args": [1], "log": {"kind": "callable"}}"#,
        ];
        let output = run_service(&format!("{}\n", requests.join("\n")), &["evaluate-jsonl"]);
        assert_eq!(output.status.code(), Some(0));
        let got = records(&output);
        assert_eq!(got.len(), 4);
        for (record, number) in got.iter().zip(1..) {
            assert_eq!(text_of(member(record, "kind")), "exception");
            assert_eq!(text_of(member(record, "type")), "RequestError");
            assert!(
                text_of(member(record, "message"))
                    .starts_with("the request has a member this service does not take: "),
                "{number}"
            );
            assert!(same_value(member(record, "id"), &PyValue::int(number)));
        }
        for arguments in [
            vec!["fixture"],
            vec!["evaluate-jsonl", "--provider", "fixed"],
            vec![],
        ] {
            let output = run_service("", &arguments);
            assert_eq!(output.status.code(), Some(2), "{arguments:?}");
            assert!(output.stdout.is_empty());
        }
        // The session reads its request from a separate descriptor (standard input is not the protocol): refused the same way.
        let area = PrivateDirectory::create("memory-refuse-").expect("a private directory");
        let zero = area.path().join("zero");
        let (mut command, mut writer) = over_descriptor(
            env!("CARGO_BIN_EXE_recipe_memory_api"),
            &["gate-session", "--input-fd", "3"],
            &StandardInput::WriteOnly(&zero),
        );
        writer
            .write_all(b"{\"id\": 5, \"operation\": \"preload_ram_gate.gate\", \"args\": [1], \"context\": {\"mem\": [1]}}\n")
            .expect("the request");
        drop(writer);
        let output = command.output().expect("run the session");
        assert_eq!(output.status.code(), Some(0));
        let got = records(&output);
        assert_eq!(got.len(), 1);
        assert_eq!(text_of(member(&got[0], "type")), "RequestError");
    }

    /// The runtime sources hold no provider selector, no fixture module and no operational capture path.
    #[test]
    fn the_production_sources_have_no_provider_selector_and_no_operational_path() {
        let sources = [
            ("memory.rs", include_str!("../src/memory.rs")),
            ("memory_api.rs", include_str!("../src/memory_api.rs")),
            ("memory_bench.rs", include_str!("../src/memory_bench.rs")),
            ("memory_cli.rs", include_str!("../src/memory_cli.rs")),
            (
                "memory_selftest.rs",
                include_str!("../src/memory_selftest.rs"),
            ),
            (
                "memory_session.rs",
                include_str!("../src/memory_session.rs"),
            ),
            ("memory_system.rs", include_str!("../src/memory_system.rs")),
            (
                "bin/preload_ram_gate.rs",
                include_str!("../src/bin/preload_ram_gate.rs"),
            ),
            (
                "bin/recipe_memory_api.rs",
                include_str!("../src/bin/recipe_memory_api.rs"),
            ),
            (
                "bin/recipe_memory_bench.rs",
                include_str!("../src/bin/recipe_memory_bench.rs"),
            ),
        ];
        let mut found = Vec::new();
        for (name, text) in sources {
            for forbidden in [
                "env::var",
                "getenv",
                "memory_fixture",
                "real-worker",
                "val_agent",
                "CARGO_MANIFEST_DIR",
                "BoundaryViolation",
            ] {
                // The command-line wrappers read the terminal width variable the argument parser documents; nothing else.
                let allowed = forbidden == "env::var"
                    && name.starts_with("bin/")
                    && text.matches("env::var").count() == 1
                    && text.contains("\"COLUMNS\"");
                if text.contains(forbidden) && !allowed {
                    found.push(format!("{name}: {forbidden}"));
                }
            }
        }
        assert!(found.is_empty(), "{}", found.join("\n"));
    }
}

// ── the gate session ─────────────────────────────────────────────────────────────────────────────

mod session_cases {
    use super::*;

    enum Reply {
        Continue,
        Abort,
        Raw(&'static str),
        CloseInput,
    }

    struct Ended {
        events: Vec<PyValue>,
        last: PyValue,
        control: PyValue,
    }

    fn counters(ended: &Ended) -> (i64, i64) {
        let state = member(&ended.control, "context_after");
        let number = |name: &str| match member(state, name) {
            PyValue::Int(value) => i64::try_from(value.clone()).expect("a counter"),
            other => panic!("{}", other.repr()),
        };
        (number("mem_consumed"), number("drop_calls"))
    }

    fn provider_events(ended: &Ended) -> Vec<String> {
        items(member(&ended.control, "trace"))
            .iter()
            .map(|event| text_of(member(event, "event")))
            .collect()
    }

    /// One session over standard input: the request sent, the events answered by `respond` (index from 1, the event), the final record.
    fn drive(context: &str, request: &str, respond: impl FnMut(usize, &PyValue) -> Reply) -> Ended {
        drive_with(None, context, request, respond)
    }

    /// The same, over a separate protocol descriptor when `descriptor_zero` names the file standard input is given instead.
    fn drive_with(
        descriptor_zero: Option<&Path>,
        context: &str,
        request: &str,
        mut respond: impl FnMut(usize, &PyValue) -> Reply,
    ) -> Ended {
        let bundle = load("bundle.json");
        let work = new_case(&bundle, "session", &PyValue::int(0));
        let (spec, control) = (work.area.join("spec.json"), work.area.join("control.json"));
        std::fs::write(
            &spec,
            format!("{{\"id\": \"session\", \"context\": {context}}}"),
        )
        .expect("a spec");
        let (spec_text, root_text, control_text) = (
            spec.to_str().expect("a path"),
            work.root.to_str().expect("a path"),
            control.to_str().expect("a path"),
        );
        let session_arguments = [
            "session",
            "--spec",
            spec_text,
            "--root",
            root_text,
            "--control",
            control_text,
        ];
        let program = env!("CARGO_BIN_EXE_recipe_memory_fixture_api");
        let (mut child, mut input): (std::process::Child, Option<Box<dyn Write>>) =
            match descriptor_zero {
                None => {
                    let mut command = Command::new(program);
                    command.args(session_arguments);
                    fixture_environment(&mut command, &work);
                    let mut child = command
                        .stdin(Stdio::piped())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .spawn()
                        .expect("start the session");
                    let pipe: Box<dyn Write> = Box::new(child.stdin.take().expect("stdin"));
                    (child, Some(pipe))
                }
                Some(zero) => {
                    let mut arguments = session_arguments.to_vec();
                    arguments.extend(["--input-fd", "3"]);
                    let (mut command, writer) =
                        over_descriptor(program, &arguments, &StandardInput::WriteOnly(zero));
                    command.env("TMPDIR", work.root.join("private-tmp"));
                    let child = command.spawn().expect("start the session");
                    let pipe: Box<dyn Write> = Box::new(writer);
                    (child, Some(pipe))
                }
            };
        input
            .as_mut()
            .expect("the protocol input")
            .write_all(format!("{request}\n").as_bytes())
            .expect("the request");
        let mut lines = std::io::BufReader::new(child.stdout.take().expect("stdout"));
        let (mut events, mut last) = (Vec::new(), None);
        loop {
            let mut line = String::new();
            if lines.read_line(&mut line).expect("a line") == 0 {
                break;
            }
            let value = loads_text(&PyStr::new(&line)).expect("a JSON line");
            let PyValue::Dict(members) = &value else {
                panic!("not an object: {line}")
            };
            if !members.contains_key("event") {
                last = Some(value);
                break;
            }
            events.push(value.clone());
            let sequence = match member(&value, "seq") {
                PyValue::Int(number) => number.to_string(),
                other => panic!("{}", other.repr()),
            };
            let answer = match respond(events.len(), &value) {
                Reply::Continue => {
                    Some(format!("{{\"ack\": {sequence}, \"action\": \"continue\"}}"))
                }
                Reply::Abort => Some(format!("{{\"ack\": {sequence}, \"action\": \"abort\"}}")),
                Reply::Raw(text) => Some(text.to_string()),
                Reply::CloseInput => {
                    input = None;
                    None
                }
            };
            if let (Some(answer), Some(pipe)) = (answer, input.as_mut()) {
                pipe.write_all(format!("{answer}\n").as_bytes())
                    .expect("an acknowledgement");
            }
        }
        drop(input);
        let output = child.wait_with_output().expect("the session ends");
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        Ended {
            events,
            last: last.expect("a final record"),
            control: read_control(&control),
        }
    }

    const MEM_TWICE: &str = r#"{"mem": [0, 0], "drop": true}"#;
    const SHORT_GATE: &str = r#"{"id": "g", "operation": "preload_ram_gate.gate", "args": [21474836480], "kwargs": {"log": null}, "log": {"kind": "callable"}}"#;

    fn saved(number: usize) -> PyValue {
        let bundle = load("bundle.json");
        items(member(&bundle, "api"))[number].clone()
    }

    fn callback_messages(case: &PyValue) -> Vec<String> {
        items(member(member(case, "original"), "trace"))
            .iter()
            .filter(|event| text_of(member(event, "event")) == "callback")
            .map(|event| text_of(member(event, "message")))
            .collect()
    }

    #[test]
    fn every_event_is_answered_before_the_next_machine_step_and_the_final_record_is_the_services() {
        let ended = drive(MEM_TWICE, SHORT_GATE, |_, _| Reply::Continue);
        let kinds: Vec<String> = ended
            .events
            .iter()
            .map(|event| text_of(member(event, "event")))
            .collect();
        assert_eq!(kinds, ["log", "log", "log"]);
        let messages: Vec<String> = ended
            .events
            .iter()
            .map(|event| text_of(member(event, "message")))
            .collect();
        assert_eq!(messages, callback_messages(&saved(106)));
        for (event, number) in ended.events.iter().zip(1..) {
            assert!(same_value(member(event, "seq"), &PyValue::int(number)));
        }
        let want = loads_text(&PyStr::new(&format!(
            "{{\"id\": \"g\", \"kind\": \"value\", \"value\": {}, \"stdout\": \"\", \"stderr\": \"\", \"args_after\": [21474836480], \"kwargs_after\": {{\"log\": null}}}}",
            member(member(&saved(106), "original"), "value").dumps_ascii().expect("json")
        )))
        .expect("the expected record");
        assert!(same_value(&want, &ended.last), "{}", ended.last.repr());
        assert_eq!(counters(&ended), (2, 1));
        assert_eq!(provider_events(&ended), ["mem", "drop", "mem"]);
    }

    #[test]
    fn an_abort_stops_the_gate_before_any_further_machine_step() {
        // The saved callbacks that raised on the n-th call: what the machine had been asked by then.
        for (stop, saved_case) in [(1usize, 107usize), (2, 108), (3, 109)] {
            let ended = drive(MEM_TWICE, SHORT_GATE, |index, _| {
                if index == stop {
                    Reply::Abort
                } else {
                    Reply::Continue
                }
            });
            let original = member(&saved(saved_case), "original").clone();
            let state = member(&original, "context_after");
            let want = |name: &str| match member(state, name) {
                PyValue::Int(value) => i64::try_from(value.clone()).expect("a counter"),
                other => panic!("{}", other.repr()),
            };
            assert_eq!(
                counters(&ended),
                (want("mem_consumed"), want("drop_calls")),
                "abort at {stop}"
            );
            assert_eq!(ended.events.len(), stop);
            let reason = loads_text(&PyStr::new(&format!(
                "{{\"id\": \"g\", \"kind\": \"aborted\", \"reason\": \"abort\", \"seq\": {stop}}}"
            )))
            .expect("json");
            assert!(same_value(&reason, &ended.last), "{}", ended.last.repr());
        }
        // An abort at the skip notice stops before the memory is read at all (the saved callback that raised on the first call).
        let ended = drive(
            r#"{"mem": [20480], "drop": false}"#,
            r#"{"id": "s", "operation": "preload_ram_gate.gate", "args": [null], "kwargs": {"log": null}, "log": {"kind": "callable"}}"#,
            |_, _| Reply::Abort,
        );
        assert_eq!(counters(&ended), (0, 0));
        assert_eq!(text_of(member(&ended.last, "kind")), "aborted");
        assert_eq!(
            text_of(member(&ended.events[0], "message")),
            callback_messages(&saved(111))[0]
        );
    }

    #[test]
    fn an_end_of_input_and_a_malformed_acknowledgement_stop_the_gate_the_same_way() {
        let ended = drive(MEM_TWICE, SHORT_GATE, |_, _| Reply::CloseInput);
        assert_eq!(text_of(member(&ended.last, "reason")), "eof");
        assert_eq!(counters(&ended), (1, 0));
        for malformed in [
            "{\"ack\": 1}",
            "{\"ack\": 2, \"action\": \"continue\"}",
            "not json",
            "{\"ack\": 1, \"action\": \"go\"}",
            "{\"ack\": 1, \"action\": \"continue\", \"extra\": 1}",
            "[]",
        ] {
            let ended = drive(MEM_TWICE, SHORT_GATE, |_, _| Reply::Raw(malformed));
            assert_eq!(
                text_of(member(&ended.last, "reason")),
                "malformed_ack",
                "{malformed}"
            );
            assert!(same_value(member(&ended.last, "seq"), &PyValue::int(1)));
            assert_eq!(counters(&ended), (1, 0), "{malformed}");
            assert_eq!(ended.events.len(), 1);
        }
    }

    #[test]
    fn the_default_log_is_relayed_as_standard_error_events_and_a_non_callable_log_is_the_gates_type_error(
    ) {
        let ended = drive(
            r#"{"mem": [20480], "drop": false}"#,
            r#"{"id": "d", "operation": "preload_ram_gate.gate", "args": [21474836480], "kwargs": {"log": null}, "log": {"kind": "default"}}"#,
            |_, _| Reply::Continue,
        );
        let texts: Vec<String> = ended
            .events
            .iter()
            .map(|event| text_of(member(event, "text")))
            .collect();
        let want: Vec<String> = items(member(member(&saved(11), "original"), "trace"))
            .iter()
            .filter(|event| text_of(member(event, "event")) == "stderr")
            .map(|event| text_of(member(event, "text")))
            .collect();
        assert_eq!(texts, want);
        assert!(ended
            .events
            .iter()
            .all(|event| text_of(member(event, "event")) == "stderr"));
        let want_value = member(member(&saved(11), "original"), "value").clone();
        assert!(
            same_value(&want_value, member(&ended.last, "value")),
            "{}",
            ended.last.repr()
        );
        assert_eq!(counters(&ended), (1, 1));
        // A truthy object that is not callable: `TypeError` when the gate logs, no event, nothing asked of the machine.
        let saved_error = member(&saved(84), "original").clone();
        for request in [
            r#"{"id": "n", "operation": "preload_ram_gate.gate", "args": [null], "kwargs": {"log": null}, "log": {"kind": "noncallable", "type": "bool"}}"#,
            r#"{"id": "n", "operation": "preload_ram_gate.gate", "args": [null], "kwargs": {"log": true}}"#,
        ] {
            let ended = drive(r#"{"mem": [20480], "drop": false}"#, request, |_, _| {
                Reply::Continue
            });
            assert!(ended.events.is_empty());
            assert_eq!(text_of(member(&ended.last, "kind")), "exception");
            assert_eq!(
                text_of(member(&ended.last, "type")),
                text_of(member(&saved_error, "type"))
            );
            assert_eq!(
                text_of(member(&ended.last, "message")),
                text_of(member(&saved_error, "message"))
            );
            assert_eq!(counters(&ended), (0, 0));
        }
    }

    #[test]
    fn a_request_the_session_cannot_serve_is_answered_with_a_request_error_and_the_machine_is_untouched(
    ) {
        let refused = [
            r#"{"id": "a", "operation": "preload_ram_gate.gate", "args": [1], "log": {"kind": "callable"}}"#,
            r#"{"id": "b", "operation": "preload_ram_gate.try_drop_caches", "log": {"kind": "callable"}}"#,
            r#"{"id": "c", "operation": "preload_ram_gate.mem_available_mib"}"#,
            r#"{"id": "d", "operation": "preload_ram_gate.gate", "args": [1], "kwargs": {"log": null}, "log": {"kind": "weird"}}"#,
            r#"{"id": "e", "operation": "preload_ram_gate.gate", "args": [1], "kwargs": {"log": null}, "log": {"kind": "callable", "type": "x"}}"#,
            r#"{"id": "f", "operation": "preload_ram_gate.gate", "args": [1], "kwargs": {"log": null}, "log": "callable"}"#,
            r#"{"id": "g", "operation": "preload_ram_gate.gate", "args": [1], "context": {}}"#,
        ];
        for request in refused {
            let ended = drive(r#"{"mem": [20480], "drop": false}"#, request, |_, _| {
                Reply::Continue
            });
            assert!(ended.events.is_empty(), "{request}");
            assert_eq!(
                text_of(member(&ended.last, "type")),
                "RequestError",
                "{request}"
            );
            assert_eq!(counters(&ended), (0, 0), "{request}");
        }
        // Python's own binding errors stay the gate's: a positional log and a keyword log at once.
        let ended = drive(
            r#"{"mem": [20480], "drop": false}"#,
            r#"{"id": "h", "operation": "preload_ram_gate.gate", "args": [1, 1, 10240, true, null], "kwargs": {"log": null}, "log": {"kind": "default"}}"#,
            |_, _| Reply::Continue,
        );
        assert_eq!(text_of(member(&ended.last, "type")), "TypeError");
        assert_eq!(
            text_of(member(&ended.last, "message")),
            "gate() got multiple values for argument 'log'"
        );
    }

    /// The same service over the descriptor a caller passes (standard input is a file nobody can read from): events ordered, one
    /// outstanding at a time, abort and end of input stop it before the next machine step, the machine counters say so.
    #[test]
    fn a_session_over_a_separate_descriptor_orders_its_events_and_stops_on_abort_and_end_of_input()
    {
        let area = PrivateDirectory::create("memory-fdsession-").expect("a private directory");
        let zero = area.path().join("zero");
        let ended = drive_with(Some(&zero), MEM_TWICE, SHORT_GATE, |_, _| Reply::Continue);
        let kinds: Vec<String> = ended
            .events
            .iter()
            .map(|event| text_of(member(event, "event")))
            .collect();
        assert_eq!(kinds, ["log", "log", "log"]);
        let messages: Vec<String> = ended
            .events
            .iter()
            .map(|event| text_of(member(event, "message")))
            .collect();
        assert_eq!(messages, callback_messages(&saved(106)));
        assert_eq!(text_of(member(&ended.last, "kind")), "value");
        assert_eq!(counters(&ended), (2, 1));
        assert_eq!(provider_events(&ended), ["mem", "drop", "mem"]);
        for (stop, want) in [(1usize, (1i64, 0i64)), (2, (2, 1))] {
            let ended = drive_with(Some(&zero), MEM_TWICE, SHORT_GATE, |index, _| {
                if index == stop {
                    Reply::Abort
                } else {
                    Reply::Continue
                }
            });
            assert_eq!(
                text_of(member(&ended.last, "reason")),
                "abort",
                "abort at {stop}"
            );
            assert_eq!(counters(&ended), want, "abort at {stop}");
        }
        let ended = drive_with(Some(&zero), MEM_TWICE, SHORT_GATE, |_, _| Reply::CloseInput);
        assert_eq!(text_of(member(&ended.last, "reason")), "eof");
        assert_eq!(counters(&ended), (1, 0));
        let ended = drive_with(Some(&zero), MEM_TWICE, SHORT_GATE, |_, _| {
            Reply::Raw("not json")
        });
        assert_eq!(text_of(member(&ended.last, "reason")), "malformed_ack");
        assert_eq!(counters(&ended), (1, 0));
    }
}

// ── the operating-system adapters ────────────────────────────────────────────────────────────────

mod system_cases {
    use super::*;
    use recipe_runtime::memory::{mem_available_mib, HelperRun, MeminfoSource};
    use recipe_runtime::memory_system::{run_with_timeout, FileMeminfo, HelperInput};
    use std::time::{Duration, Instant};

    fn expand(recipe: &PyValue) -> Vec<u8> {
        let mut bytes = Vec::new();
        for part in items(member(recipe, "parts")) {
            let PyValue::Dict(map) = part else {
                panic!("a part")
            };
            if let Some(PyValue::Str(text)) = map.get("ascii") {
                bytes.extend_from_slice(text.to_utf8().expect("text").as_bytes());
            } else if let Some(PyValue::Str(letter)) = map.get("repeat") {
                let count = match member(part, "count") {
                    PyValue::Int(number) => usize::try_from(number.clone()).expect("a count"),
                    other => panic!("{}", other.repr()),
                };
                bytes.extend_from_slice(letter.to_utf8().expect("text").repeat(count).as_bytes());
            } else {
                bytes.extend_from_slice(&hex_decode(&text_of(member(part, "hex"))));
            }
        }
        bytes
    }

    fn outcome_text(source: &mut dyn MeminfoSource) -> String {
        match mem_available_mib(source) {
            Ok(PyValue::None) => "null".to_string(),
            Ok(value) => value.repr(),
            Err(failure) => failure.line().to_utf8().expect("text"),
        }
    }

    /// Chunk edges, carriage returns and bad bytes: the expectations were produced by one stdlib probe of `open(..., encoding="ascii")`
    /// line iteration over the same recipes (`oracle/meminfo_chunk_probe.py`), not by the original module.
    #[test]
    fn the_file_parser_reads_chunks_lines_and_bad_bytes_as_the_language_primitive_does() {
        let recipes = load("chunk_recipes.json");
        let expected = load("chunk_expected.json");
        let area = PrivateDirectory::create("memory-chunks-").expect("a private directory");
        let mut wrong = Vec::new();
        for recipe in items(&recipes) {
            let name = text_of(member(recipe, "name"));
            let path = area.path().join(format!("{name}.txt"));
            std::fs::write(&path, expand(recipe)).expect("a file");
            let mut source =
                FileMeminfo::new(PyStr::from_surrogateescape(path.as_os_str().as_bytes()));
            let got = outcome_text(&mut source);
            let want = match member(&expected, &name) {
                PyValue::None => "null".to_string(),
                PyValue::Str(text) => text.to_utf8().expect("text"),
                other => other.repr(),
            };
            if got != want {
                wrong.push(format!("{name}: want {want} got {got}"));
            }
        }
        assert_eq!(items(&recipes).len(), 9);
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    }

    #[test]
    fn the_helper_runner_waits_reports_statuses_and_kills_and_reaps_past_the_limit() {
        let shell = |script: &str, limit: Duration, input: HelperInput| {
            run_with_timeout(&["/bin/sh", "-c", script], limit, input)
        };
        let long = Duration::from_secs(20);
        assert_eq!(
            shell("exit 0", long, HelperInput::Null),
            HelperRun::Exited(0)
        );
        assert_eq!(
            shell("exit 3", long, HelperInput::Null),
            HelperRun::Exited(3)
        );
        assert_eq!(
            shell("kill -9 $$", long, HelperInput::Null),
            HelperRun::Exited(-9)
        );
        // The null device is the helper's standard input (end of input at once), and its output is discarded.
        assert_eq!(
            shell(
                "if read line; then exit 5; else exit 6; fi",
                long,
                HelperInput::Null
            ),
            HelperRun::Exited(6)
        );
        assert_eq!(
            shell("echo out; echo err >&2; exit 0", long, HelperInput::Null),
            HelperRun::Exited(0)
        );
        assert_eq!(
            run_with_timeout(&["/nonexistent/helper-program"], long, HelperInput::Null),
            HelperRun::Failed
        );
        assert_eq!(
            run_with_timeout(&[], long, HelperInput::Null),
            HelperRun::Failed
        );
        // Past the limit the command is killed and reaped: the call returns near the limit, not near the command's own length.
        let started = Instant::now();
        let ended = shell("sleep 30", Duration::from_millis(300), HelperInput::Null);
        let waited = started.elapsed();
        assert_eq!(ended, HelperRun::Failed);
        assert!(
            waited >= Duration::from_millis(300) && waited < Duration::from_secs(10),
            "{waited:?}"
        );
    }
}

// ── the protocol descriptor ──────────────────────────────────────────────────────────────────────

mod transport_cases {
    use super::*;

    fn records_of(output: &std::process::Output) -> Vec<PyValue> {
        String::from_utf8(output.stdout.clone())
            .expect("text")
            .lines()
            .map(|line| loads_text(&PyStr::new(line)).expect("a record"))
            .collect()
    }

    /// Runs `recipe_memory_api <arguments>` over descriptor 3 carrying `protocol`, standard input being `zero` (write-only).
    fn service_over_descriptor(
        arguments: &[&str],
        protocol: &str,
        zero: &Path,
    ) -> std::process::Output {
        let (mut command, mut writer) = over_descriptor(
            env!("CARGO_BIN_EXE_recipe_memory_api"),
            arguments,
            &StandardInput::WriteOnly(zero),
        );
        writer
            .write_all(protocol.as_bytes())
            .expect("the protocol bytes");
        drop(writer);
        command.output().expect("run the service")
    }

    /// The saved calls that never reach the machine, sent over the passed descriptor while standard input cannot be read at all.
    #[test]
    fn the_requests_are_read_from_the_passed_descriptor_and_standard_input_is_never_read() {
        let bundle = load("bundle.json");
        let cases = items(member(&bundle, "api"));
        let work = new_case(&bundle, "descriptor", member(&cases[153], "before"));
        let root = root_text(&work);
        let (mut protocol, mut wanted) = (String::new(), Vec::new());
        for number in [146usize, 153, 156, 167, 175] {
            let case = &cases[number];
            let request = member(case, "request");
            let mut line = PyDict::new();
            line.set("id", member(request, "id").clone());
            line.set(
                "operation",
                PyValue::str(&format!(
                    "preload_ram_gate.{}",
                    text_of(member(request, "function"))
                )),
            );
            line.set("args", rebase(member(request, "args"), &root));
            line.set("kwargs", rebase(member(request, "kwargs"), &root));
            protocol.push_str(&PyValue::Dict(line).dumps_ascii().expect("a request"));
            protocol.push('\n');
            wanted.push(rebase(
                &without(member(case, "original"), &["trace", "context_after"]),
                &root,
            ));
        }
        let zero = work.area.join("zero");
        let output =
            service_over_descriptor(&["evaluate-jsonl", "--input-fd", "3"], &protocol, &zero);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        let got = records_of(&output);
        assert_eq!(got.len(), wanted.len());
        for (want, got) in wanted.iter().zip(&got) {
            assert!(
                same_value(want, got),
                "want {} got {}",
                want.repr(),
                got.repr()
            );
        }
        // The session service reads its request from the descriptor as well (a request it refuses, so the machine is not touched).
        let refused = "{\"id\": 9, \"operation\": \"preload_ram_gate.gate\", \"args\": [1], \"context\": {}}\n";
        let output = service_over_descriptor(&["gate-session", "--input-fd", "3"], refused, &zero);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            text_of(member(&records_of(&output)[0], "type")),
            "RequestError"
        );
    }

    /// A missing, closed, invalid or unreadable descriptor, and every malformed option, fail loudly and run nothing.
    #[test]
    fn a_missing_closed_invalid_or_unreadable_descriptor_fails_without_running_anything() {
        let area = PrivateDirectory::create("memory-badfd-").expect("a private directory");
        let program = env!("CARGO_BIN_EXE_recipe_memory_api");
        let run = |arguments: &[&str]| {
            Command::new(program)
                .args(arguments)
                .env_clear()
                .stdin(Stdio::null())
                .output()
                .expect("run the service")
        };
        // Usage errors (status 2): the session needs the descriptor; the descriptor is decimal digits above 2; once; not for `constants`.
        for arguments in [
            vec!["gate-session"],
            vec!["evaluate-jsonl", "--input-fd"],
            vec!["evaluate-jsonl", "--input-fd", "0"],
            vec!["evaluate-jsonl", "--input-fd", "1"],
            vec!["evaluate-jsonl", "--input-fd", "2"],
            vec!["evaluate-jsonl", "--input-fd", "-1"],
            vec!["evaluate-jsonl", "--input-fd", "+3"],
            vec!["evaluate-jsonl", "--input-fd", "3x"],
            vec!["evaluate-jsonl", "--input-fd", ""],
            vec!["evaluate-jsonl", "--input-fd", "99999999999"],
            vec!["evaluate-jsonl", "--input-fd", "3", "--input-fd", "4"],
            vec!["gate-session", "--input"],
            vec!["constants", "--input-fd", "3"],
        ] {
            let output = run(&arguments);
            assert_eq!(
                output.status.code(),
                Some(2),
                "{arguments:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.stdout.is_empty(), "{arguments:?}");
        }
        // A descriptor that is not open in the process (status 1, named in the message), for both services.
        for arguments in [
            vec!["evaluate-jsonl", "--input-fd", "3"],
            vec!["gate-session", "--input-fd", "7"],
        ] {
            let output = run(&arguments);
            assert_eq!(output.status.code(), Some(1), "{arguments:?}");
            assert!(output.stdout.is_empty());
            assert!(
                String::from_utf8_lossy(&output.stderr).contains("not an open descriptor"),
                "{arguments:?}"
            );
        }
        // A descriptor opened for writing only is refused as unreadable.
        let file = area.path().join("only-write");
        let output = Command::new("/bin/sh")
            .args([
                "-c",
                "exec \"$@\" 3>\"$F3\"",
                "sh",
                program,
                "evaluate-jsonl",
                "--input-fd",
                "3",
            ])
            .env_clear()
            .env("F3", &file)
            .stdin(Stdio::null())
            .output()
            .expect("run the service");
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stderr).contains("not readable"));
    }

    /// The helper runner the production machine uses gives the helper the process's own standard input (distinct bytes from the
    /// protocol), never the protocol channel: status 11 means the helper read exactly `alpha`, `beta` from descriptor 0 and could not
    /// open descriptor 3; the protocol bytes are still all there afterwards.
    #[test]
    fn the_helper_runner_gives_the_helper_standard_input_and_never_the_protocol_channel() {
        let area = PrivateDirectory::create("memory-probe-").expect("a private directory");
        let helper = "IFS= read -r a; IFS= read -r b; if { true <&3; } 2>/dev/null; then exit 21; fi; [ \"$a/$b\" = alpha/beta ] && exit 11; exit 12";
        let probe = |zero_bytes: &[u8]| {
            let zero = area.path().join("zero");
            std::fs::write(&zero, zero_bytes).expect("standard input bytes");
            let (mut command, mut writer) = over_descriptor(
                env!("CARGO_BIN_EXE_recipe_memory_fixture_api"),
                &[
                    "runner-probe",
                    "--input-fd",
                    "3",
                    "--",
                    "/bin/sh",
                    "-c",
                    helper,
                ],
                &StandardInput::Reads(&zero),
            );
            writer
                .write_all(b"ACK-ONE\nACK-TWO\n")
                .expect("the protocol bytes");
            drop(writer);
            let output = command.output().expect("run the probe");
            assert_eq!(
                output.status.code(),
                Some(0),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            records_of(&output).remove(0)
        };
        let row = probe(b"alpha\nbeta\n");
        assert!(
            same_value(member(&row, "status"), &PyValue::int(11)),
            "{}",
            row.repr()
        );
        assert_eq!(text_of(member(&row, "protocol")), "ACK-ONE\nACK-TWO\n");
        // The probe tells standard-input bytes apart: different bytes, different status; the protocol is intact either way.
        let row = probe(b"gamma\ndelta\n");
        assert!(
            same_value(member(&row, "status"), &PyValue::int(12)),
            "{}",
            row.repr()
        );
        assert_eq!(text_of(member(&row, "protocol")), "ACK-ONE\nACK-TWO\n");
    }
}

// ── a standard error that cannot be written ──────────────────────────────────────────────────────

mod stderr_cases {
    use super::*;
    use recipe_runtime::memory_cli::write_text_to;

    /// A sink that takes `accepted` bytes and then fails with the operating-system error `errno`.
    struct Failing {
        accepted: usize,
        errno: i32,
        delivered: Vec<u8>,
    }

    impl Write for Failing {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.delivered.len() >= self.accepted {
                return Err(std::io::Error::from_raw_os_error(self.errno));
            }
            let room = (self.accepted - self.delivered.len()).min(bytes.len());
            self.delivered.extend_from_slice(&bytes[..room]);
            Ok(room)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// The writer every standard-error write goes through: a failed write is the `OSError` Python raises (class and message of the
    /// existing standard-output helper), after the bytes a partial write delivered; unencodable text is `backslashreplace`.
    #[test]
    fn a_failed_standard_error_write_is_returned_as_the_os_error_after_any_partial_delivery() {
        let mut sink = Failing {
            accepted: 0,
            errno: 28,
            delivered: Vec::new(),
        };
        let failure =
            write_text_to(&mut sink, &PyStr::new("notice\n")).expect_err("a failed write");
        assert_eq!(
            (failure.class, failure.message.to_utf8().expect("text")),
            ("OSError", "[Errno 28] No space left on device".to_string())
        );
        let mut sink = Failing {
            accepted: 3,
            errno: 32,
            delivered: Vec::new(),
        };
        let failure =
            write_text_to(&mut sink, &PyStr::new("notice\n")).expect_err("a failed write");
        assert_eq!(
            (failure.class, failure.message.to_utf8().expect("text")),
            ("BrokenPipeError", "[Errno 32] Broken pipe".to_string())
        );
        assert_eq!(sink.delivered, b"not");
        let mut sink = Failing {
            accepted: 100,
            errno: 28,
            delivered: Vec::new(),
        };
        let text = PyStr::from_code_points(vec![u32::from('a'), 0xD800, u32::from('\n')]);
        write_text_to(&mut sink, &text).expect("a working sink");
        assert_eq!(sink.delivered, b"a\\ud800\n");
    }

    /// The saved command line `index` run through the fixture program whose standard error is a pipe nobody reads (every write to it
    /// fails with a broken pipe): the exit status and the control record.
    fn with_broken_stderr(index: usize) -> (Option<i32>, Vec<u8>, PyValue) {
        let bundle = load("bundle.json");
        let case = &items(member(&bundle, "cli"))[index];
        let request = member(case, "request");
        let work = new_case(&bundle, "brokenstderr", member(case, "before"));
        let (spec, control) = (work.area.join("spec.json"), work.area.join("control.json"));
        write_json(&spec, request);
        let mut command = Command::new(env!("CARGO_BIN_EXE_preload_ram_gate_fixture"));
        command
            .arg("--spec")
            .arg(&spec)
            .arg("--root")
            .arg(&work.root)
            .arg("--control")
            .arg(&control);
        fixture_environment(&mut command, &work);
        let (reader, writer) = std::io::pipe().expect("a pipe");
        drop(reader);
        let output = command
            .stdout(Stdio::piped())
            .stderr(Stdio::from(writer))
            .spawn()
            .expect("start the program")
            .wait_with_output()
            .expect("the program ends");
        (output.status.code(), output.stdout, read_control(&control))
    }

    fn counters(record: &PyValue) -> (PyValue, PyValue) {
        let state = member(record, "context_after");
        (
            member(state, "mem_consumed").clone(),
            member(state, "drop_calls").clone(),
        )
    }

    /// Saved case 6 (memory short, a drop would succeed): the first notice fails to write, so the program ends with the `OSError`,
    /// status 1, the memory read once and the cache never dropped, and no standard-error event is recorded for the failed write.
    #[test]
    fn a_failed_notice_stops_the_gate_before_the_cache_drop() {
        let (status, stdout, record) = with_broken_stderr(6);
        assert_eq!(status, Some(1));
        assert!(stdout.is_empty());
        assert_eq!(text_of(member(&record, "kind")), "exception");
        assert_eq!(text_of(member(&record, "type")), "BrokenPipeError");
        assert_eq!(
            text_of(member(&record, "message")),
            "[Errno 32] Broken pipe"
        );
        assert!(same_value(member(&record, "status"), &PyValue::int(1)));
        let events: Vec<String> = items(member(&record, "trace"))
            .iter()
            .map(|event| text_of(member(event, "event")))
            .collect();
        assert_eq!(events, ["mem"]);
        assert!(
            same_value(&counters(&record).0, &PyValue::int(1))
                && same_value(&counters(&record).1, &PyValue::int(0))
        );
    }

    /// Saved case 2 (unknown checkpoint size): the skip notice fails before any memory read, and nothing was asked of the machine.
    #[test]
    fn a_failed_skip_notice_stops_before_any_memory_read() {
        let (status, _, record) = with_broken_stderr(2);
        assert_eq!(status, Some(1));
        assert_eq!(text_of(member(&record, "type")), "BrokenPipeError");
        assert!(items(member(&record, "trace")).is_empty());
        assert!(
            same_value(&counters(&record).0, &PyValue::int(0))
                && same_value(&counters(&record).1, &PyValue::int(0))
        );
    }
}

// ── standard output and error that were closed at start ─────────────────────────────────────────

/// Source-derived (the original's source plus stdlib behaviour of `print`, `sys.stderr = None` and `argparse._print_message`), not a
/// capture of the original command line: with descriptor 2 closed when the program starts, `print(..., file=sys.stderr)` falls back
/// to `sys.stdout` (and to nowhere when that is closed too), the argument parser's own write to `sys.stderr` is dropped, and the
/// interpreter's uncaught-exception report is dropped; none of it is an error and none is standard error output.
mod startup_cases {
    use super::*;

    struct Ran {
        output: std::process::Output,
        control: PyValue,
    }

    /// The saved command line `index` through the fixture program with the given shell redirections applied before it starts.
    fn run_closed(index: usize, redirections: &str) -> Ran {
        let bundle = load("bundle.json");
        let request = member(&items(member(&bundle, "cli"))[index], "request").clone();
        run_request(&bundle, index, &request, redirections)
    }

    /// A request (the saved case `index` with other arguments, or as saved) with the redirections applied before it starts.
    fn run_request(bundle: &PyValue, index: usize, request: &PyValue, redirections: &str) -> Ran {
        let case = &items(member(bundle, "cli"))[index];
        let work = new_case(bundle, "closed", member(case, "before"));
        let (spec, control) = (work.area.join("spec.json"), work.area.join("control.json"));
        write_json(&spec, request);
        let script = format!("exec \"$@\" {redirections}");
        let mut command = Command::new("/bin/sh");
        command
            .args([
                "-c",
                &script,
                "sh",
                env!("CARGO_BIN_EXE_preload_ram_gate_fixture"),
            ])
            .arg("--spec")
            .arg(&spec)
            .arg("--root")
            .arg(&work.root)
            .arg("--control")
            .arg(&control);
        fixture_environment(&mut command, &work);
        let output = command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .expect("run the program");
        Ran {
            output,
            control: read_control(&control),
        }
    }

    /// The saved case `index` with the arguments replaced.
    fn with_arguments(index: usize, args: &[&str]) -> PyValue {
        let bundle = load("bundle.json");
        let saved = member(&items(member(&bundle, "cli"))[index], "request");
        let PyValue::Dict(map) = saved else {
            panic!("a request")
        };
        let mut copy = PyDict::new();
        for (key, value) in map.iter() {
            copy.insert(key.clone(), value.clone());
        }
        copy.set(
            "args",
            PyValue::List(args.iter().map(|argument| PyValue::str(argument)).collect()),
        );
        PyValue::Dict(copy)
    }

    fn saved(index: usize, stream: &str) -> Vec<u8> {
        let bundle = load("bundle.json");
        hex_decode(&text_of(member(
            &items(member(&bundle, "cli"))[index],
            stream,
        )))
    }

    fn events(ran: &Ran) -> Vec<String> {
        items(member(&ran.control, "trace"))
            .iter()
            .map(|event| text_of(member(event, "event")))
            .collect()
    }

    fn counters(ran: &Ran) -> (PyValue, PyValue) {
        let state = member(&ran.control, "context_after");
        (
            member(state, "mem_consumed").clone(),
            member(state, "drop_calls").clone(),
        )
    }

    fn assert_counters(ran: &Ran, mem: i64, drop: i64) {
        let (got_mem, got_drop) = counters(ran);
        assert!(
            same_value(&got_mem, &PyValue::int(mem)) && same_value(&got_drop, &PyValue::int(drop)),
            "{}",
            ran.control.repr()
        );
    }

    /// Standard error closed at start: the saved normal standard-error text is exactly what arrives on standard output (the skip
    /// notice and verdict of case 2, the whole drop sequence of case 6), the provider flow is the normal one, and standard error
    /// carries nothing (it does not exist); the fallback text is not a recorded standard-error event.
    #[test]
    fn with_standard_error_closed_the_logs_are_printed_on_standard_output_and_the_flow_is_unchanged(
    ) {
        for (index, mem, drop, kinds) in [
            (2usize, 1i64, 0i64, vec!["mem"]),
            (6, 2, 1, vec!["mem", "drop", "mem"]),
        ] {
            let ran = run_closed(index, "2>&-");
            assert_eq!(ran.output.status.code(), Some(0), "case {index}");
            assert_eq!(
                ran.output.stdout,
                saved(index, "stderr_hex"),
                "case {index}"
            );
            assert!(ran.output.stderr.is_empty());
            assert_eq!(events(&ran), kinds, "case {index}");
            assert_counters(&ran, mem, drop);
        }
        // Case 21 (the self-test) prints on standard output anyway: the same bytes as with every descriptor open.
        let ran = run_closed(21, "2>&-");
        assert_eq!(ran.output.stdout, saved(21, "stdout_hex"));
    }

    /// Neither descriptor open at start: the prints go nowhere, the flow and the exit are the same.
    #[test]
    fn with_both_standard_descriptors_closed_the_prints_are_discarded_and_the_flow_is_unchanged() {
        for (index, mem, drop) in [(2usize, 1i64, 0i64), (6, 2, 1)] {
            let ran = run_closed(index, "1>&- 2>&-");
            assert_eq!(ran.output.status.code(), Some(0), "case {index}");
            assert!(ran.output.stdout.is_empty() && ran.output.stderr.is_empty());
            assert_counters(&ran, mem, drop);
        }
        // Standard output closed alone changes nothing about standard error.
        let ran = run_closed(2, "1>&-");
        assert_eq!(ran.output.stderr, saved(2, "stderr_hex"));
        assert_eq!(ran.output.status.code(), Some(0));
        assert_eq!(events(&ran), ["stderr", "mem", "stderr"]);
    }

    /// The usage the parser generated for saved case 1 (a missing required argument): its saved standard-error text without the final
    /// error line (a literal suffix of the saved text, not a newline guess).
    fn saved_usage_of_case_1() -> Vec<u8> {
        let all = saved(1, "stderr_hex");
        let error_line = "preload_ram_gate.py: error: one of the arguments --model-host-path --checkpoint-bytes is required\n";
        assert!(all.ends_with(error_line.as_bytes()));
        all[..all.len() - error_line.len()].to_vec()
    }

    /// `argparse.error` with no standard error: `print_usage(sys.stderr)` falls back to `sys.stdout`, so the usage (alone) is
    /// printed there, while the error line goes through `exit` to `_print_message(None)` and is dropped; status 2. With neither
    /// stream, nothing. (Correction of the v4 expectation, which dropped the usage too: see PORTING.)
    #[test]
    fn a_usage_error_with_standard_error_closed_prints_the_usage_alone_on_standard_output() {
        let ran = run_closed(1, "2>&-");
        assert_eq!(ran.output.status.code(), Some(2));
        assert_eq!(ran.output.stdout, saved_usage_of_case_1());
        assert!(events(&ran).is_empty());
        let ran = run_closed(1, "1>&- 2>&-");
        assert_eq!(ran.output.status.code(), Some(2));
        assert!(ran.output.stdout.is_empty() && ran.output.stderr.is_empty());
        // The usage is split off by the formatter's own text: an error text with a newline or an `error:` in it is never truncated.
        let tricky = with_arguments(1, &["--checkpoint-bytes", "0", "--bad\nerror: x"]);
        let bundle = load("bundle.json");
        let ran = run_request(&bundle, 1, &tricky, "");
        let mut want = saved_usage_of_case_1();
        want.extend_from_slice(
            b"preload_ram_gate.py: error: unrecognized arguments: --bad\nerror: x\n",
        );
        assert_eq!(ran.output.stderr, want);
        assert_eq!(ran.output.status.code(), Some(2));
        let ran = run_request(&bundle, 1, &tricky, "2>&-");
        assert_eq!(ran.output.stdout, saved_usage_of_case_1());
    }

    /// `print_help()` writes to `sys.stdout`; with it closed `_print_message(None)` falls back to `sys.stderr`; with both closed
    /// the help is dropped. Status 0 in all three.
    #[test]
    fn the_help_goes_to_standard_error_when_standard_output_was_closed_and_nowhere_when_both_were()
    {
        let ran = run_closed(0, "1>&-");
        assert_eq!(ran.output.status.code(), Some(0));
        assert_eq!(ran.output.stderr, saved(0, "stdout_hex"));
        let ran = run_closed(0, "1>&- 2>&-");
        assert_eq!(ran.output.status.code(), Some(0));
        assert!(ran.output.stdout.is_empty() && ran.output.stderr.is_empty());
        // With standard error closed alone the help is on standard output as saved.
        let ran = run_closed(0, "2>&-");
        assert_eq!(ran.output.stdout, saved(0, "stdout_hex"));
    }

    /// `_print_message` suppresses write errors: a usage error written to a standard error that fails (a pipe nobody reads) is
    /// still status 2 with nothing recorded as written. (This is not a claim about the interpreter's later flush at shutdown.)
    #[test]
    fn a_usage_error_written_to_a_failing_standard_error_is_suppressed_like_argparse_does() {
        let bundle = load("bundle.json");
        let case = &items(member(&bundle, "cli"))[1];
        let work = new_case(&bundle, "parserfail", member(case, "before"));
        let (spec, control) = (work.area.join("spec.json"), work.area.join("control.json"));
        write_json(&spec, member(case, "request"));
        let mut command = Command::new(env!("CARGO_BIN_EXE_preload_ram_gate_fixture"));
        command
            .arg("--spec")
            .arg(&spec)
            .arg("--root")
            .arg(&work.root)
            .arg("--control")
            .arg(&control);
        fixture_environment(&mut command, &work);
        let (reader, writer) = std::io::pipe().expect("a pipe");
        drop(reader);
        let output = command
            .stdout(Stdio::piped())
            .stderr(Stdio::from(writer))
            .output()
            .expect("run");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let record = read_control(&control);
        assert!(items(member(&record, "trace")).is_empty());
    }

    /// An uncaught exception's report is the interpreter's, written to standard error only when it exists; with it closed it is
    /// dropped (never on standard output). The notice before the failure is a `print` and goes to standard output.
    #[test]
    fn with_standard_error_closed_the_uncaught_report_is_dropped() {
        let ran = run_closed(8, "2>&-");
        assert_eq!(ran.output.status.code(), Some(1));
        let saved_err = saved(8, "stderr_hex");
        let notice_end = saved_err
            .iter()
            .position(|&byte| byte == b'\n')
            .expect("a first line")
            + 1;
        assert_eq!(ran.output.stdout, saved_err[..notice_end].to_vec());
        assert_eq!(text_of(member(&ran.control, "type")), "TypeError");
        assert_eq!(events(&ran), ["mem", "drop", "mem"]);
        assert_counters(&ran, 2, 1);
    }
}

// ── the benchmark program ────────────────────────────────────────────────────────────────────────

mod bench_cases {
    use super::*;

    fn api_original(bundle: &PyValue, number: usize) -> PyValue {
        member(&items(member(bundle, "api"))[number], "original").clone()
    }

    fn fixture_text(bundle: &PyValue, meminfo: &Path, mutated: bool) -> String {
        let pure = api_original(bundle, 6);
        let mut value = member(&pure, "value").clone();
        if mutated {
            if let PyValue::Dict(map) = &mut value {
                map.set("required_mib", PyValue::int(1));
            }
        }
        let io = api_original(bundle, 113);
        let rows = loads_text(&PyStr::new(&format!(
            "[{{\"metric\": \"pure\", \"operation\": \"preload_ram_gate.gate\", \"args\": [1], \"kwargs\": {{}}, \"context\": {{\"mem\": 20480}}, \"expected\": {}}}, {{\"metric\": \"io\", \"operation\": \"preload_ram_gate.mem_available_mib\", \"args\": [], \"kwargs\": {{}}, \"context\": {{\"meminfo_file\": {}}}, \"expected\": {}}}]",
            value.dumps_ascii().expect("json"),
            PyValue::str(meminfo.to_str().expect("a path")).dumps_ascii().expect("json"),
            member(&io, "value").dumps_ascii().expect("json"),
        )))
        .expect("the fixture rows");
        rows.dumps_ascii().expect("json")
    }

    fn meminfo_file(bundle: &PyValue, area: &Path) -> PathBuf {
        let case = &items(member(bundle, "api"))[113];
        let entries = expected_tree(bundle, member(case, "before"));
        let (_, (_, _, bytes)) = entries
            .iter()
            .find(|(path, _)| path.as_str() == "meminfo")
            .expect("the saved meminfo");
        let path = area.join("meminfo");
        std::fs::write(&path, bytes).expect("a file");
        path
    }

    fn run_bench(fixture: &Path, extra: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_recipe_memory_bench"))
            .arg("--fixture")
            .arg(fixture)
            .args(extra)
            .env_clear()
            .stdin(Stdio::null())
            .output()
            .expect("run the benchmark")
    }

    #[test]
    fn the_benchmark_reports_controlled_timings_only_after_every_value_matched() {
        let bundle = load("bundle.json");
        let area = PrivateDirectory::create("memory-bench-").expect("a private directory");
        let meminfo = meminfo_file(&bundle, area.path());
        let fixture = area.path().join("fixture.json");
        std::fs::write(&fixture, fixture_text(&bundle, &meminfo, false)).expect("a fixture");
        let output = run_bench(&fixture, &["--rounds", "2", "--iterations", "3"]);
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
        for (line, metric) in lines[1..].iter().zip(["pure", "io"]) {
            let row = loads_text(&PyStr::new(line)).expect("a report row");
            assert_eq!(text_of(member(&row, "metric")), metric);
            assert_eq!(text_of(member(&row, "unit")), "ns_per_call");
            assert!(same_value(member(&row, "round_count"), &PyValue::int(2)));
            assert_eq!(items(member(&row, "rounds")).len(), 2);
        }
        // A value that differs from the saved one ends the run before any timer: no report, status 1.
        std::fs::write(
            area.path().join("wrong.json"),
            fixture_text(&bundle, &meminfo, true),
        )
        .expect("a fixture");
        let output = run_bench(
            &area.path().join("wrong.json"),
            &["--rounds", "2", "--iterations", "3"],
        );
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr)
            .contains("differs from the saved original result"));
        // A machine that is not exactly one of the two controlled shapes is refused.
        let both = fixture_text(&bundle, &meminfo, false).replace(
            "{\"mem\": 20480}",
            "{\"mem\": 20480, \"meminfo_file\": \"/x\"}",
        );
        std::fs::write(area.path().join("both.json"), both).expect("a fixture");
        let output = run_bench(&area.path().join("both.json"), &[]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr)
            .contains("the context is exactly one of mem, meminfo_file"));
        let output = run_bench(&area.path().join("both.json"), &["--rounds", "0"]);
        assert_eq!(output.status.code(), Some(2));
    }
}
