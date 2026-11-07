//! The registered checks of the generation owner (`gen_recipe_set.py`, `feedback_log.py`): the saved original results,
//! observed whole, and the behaviour the saved results do not pin down.
//!
//! * `reference_cases` runs the 128 saved function calls through the real `recipe_generation_api` binary and the 26 saved
//!   command lines through the real `gen_recipe_set` / `feedback_log` binaries, each in its own new private directory made
//!   from the saved declarative setup, and compares the complete answer (values, types, key order, tuples, exception class and
//!   message, standard streams, the arguments after the call) and the complete file tree before and after (type, bytes, mode,
//!   link target, partial writes) with the saved original's. The only rebasing is the token `@CASE@`, replaced by this run's
//!   private directory. The original modules are never run: the expected values are the saved observations of the capture.
//!   A command line's standard error is compared after the original's Python traceback frames (the interpreter's own, from
//!   exactly the two scripts and the two `json` modules) were removed and nothing else: the native program writes the complete final
//!   `Class: message` block only.
//! * `anchor_cases`, `clock_cases`, `path_cases` check the pieces the saved cases cannot reach: the deployed-root anchoring, the
//!   production clock's text, and `normpath`/`split` against Python's own `posixpath` (the interpreter is used only as the oracle of
//!   those stdlib primitives, never of the original modules).

use agent_runtime_support::pyjson::loads_text;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use recipe_runtime::model_files::PrivateDirectory;
use recipe_runtime::observe::same_value;
use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// A program is started only while no thread of these checks has a file open for writing, and a program file is written only
/// while none is being started: the checks run in parallel threads, and a child forked while another thread still holds a
/// freshly written program open for writing could not exec that program (`ETXTBSY`).
static START: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn locked() -> std::sync::MutexGuard<'static, ()> {
    START
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// `Command::spawn()` under the start lock.
fn spawn_locked(command: &mut Command) -> std::io::Result<std::process::Child> {
    let _guard = locked();
    command.spawn()
}

/// `Command::output()` under the start lock (standard input is empty, the two output streams are captured).
fn output_locked(command: &mut Command) -> std::io::Result<std::process::Output> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    spawn_locked(command)?.wait_with_output()
}

/// `std::fs::copy` under the start lock.
fn copy_locked(from: &str, to: &Path) -> std::io::Result<u64> {
    let _guard = locked();
    std::fs::copy(from, to)
}

mod support {
    use super::*;

    pub fn data(relative: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("checks")
            .join("data")
            .join("generation_reference")
            .join(relative)
    }

    pub fn load(relative: &str) -> PyValue {
        let bytes = std::fs::read(data(relative)).expect("saved data");
        loads_text(&PyStr::from_surrogateescape(&bytes)).expect("saved JSON")
    }

    pub fn member<'a>(value: &'a PyValue, key: &str) -> &'a PyValue {
        match value {
            PyValue::Dict(map) => map.get(key).unwrap_or_else(|| panic!("missing {key}")),
            other => panic!("not an object: {}", other.repr()),
        }
    }

    pub fn has(value: &PyValue, key: &str) -> bool {
        matches!(value, PyValue::Dict(map) if map.contains_key(key))
    }

    pub fn items(value: &PyValue) -> &[PyValue] {
        match value {
            PyValue::List(items) => items,
            other => panic!("not a list: {}", other.repr()),
        }
    }

    pub fn text_of(value: &PyValue) -> PyStr {
        match value {
            PyValue::Str(text) => text.clone(),
            other => panic!("not text: {}", other.repr()),
        }
    }

    pub fn native(text: &PyStr) -> PathBuf {
        PathBuf::from(OsString::from_vec(
            text.to_surrogateescape_bytes().expect("a path"),
        ))
    }

    /// `text.replace(token, with)` over code points (the text may hold lone surrogates).
    pub fn replace_token(text: &PyStr, token: &str, with: &PyStr) -> PyStr {
        let needle: Vec<u32> = token.chars().map(u32::from).collect();
        let points = text.code_points();
        let mut out: Vec<u32> = Vec::with_capacity(points.len());
        let mut at = 0;
        while at < points.len() {
            if points[at..].starts_with(&needle) {
                out.extend_from_slice(with.code_points());
                at += needle.len();
            } else {
                out.push(points[at]);
                at += 1;
            }
        }
        PyStr::from_code_points(out)
    }

    /// Every string of the value with `@CASE@` replaced by `root`.
    pub fn rebased(value: &PyValue, root: &PyStr) -> PyValue {
        match value {
            PyValue::Str(text) => PyValue::Str(replace_token(text, "@CASE@", root)),
            PyValue::List(items) => {
                PyValue::List(items.iter().map(|item| rebased(item, root)).collect())
            }
            PyValue::Tuple(items) => {
                PyValue::Tuple(items.iter().map(|item| rebased(item, root)).collect())
            }
            PyValue::Dict(map) => {
                let mut copy = PyDict::new();
                for (key, item) in map.iter() {
                    copy.insert(replace_token(key, "@CASE@", root), rebased(item, root));
                }
                PyValue::Dict(copy)
            }
            other => other.clone(),
        }
    }

    pub fn replace_bytes(bytes: &[u8], token: &str, with: &[u8]) -> Vec<u8> {
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

    pub fn base64_decode(body: &str) -> Vec<u8> {
        let mut out = Vec::new();
        let (mut accumulator, mut bits) = (0u32, 0u32);
        for byte in body.bytes().filter(|byte| *byte != b'=') {
            let value = match byte {
                b'A'..=b'Z' => byte - b'A',
                b'a'..=b'z' => byte - b'a' + 26,
                b'0'..=b'9' => byte - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                _ => continue,
            };
            accumulator = (accumulator << 6) | u32::from(value);
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push((accumulator >> bits) as u8);
                accumulator &= (1 << bits) - 1;
            }
        }
        out
    }

    pub fn hex_decode(text: &str) -> Vec<u8> {
        (0..text.len() / 2)
            .map(|at| u8::from_str_radix(&text[2 * at..2 * at + 2], 16).expect("hex"))
            .collect()
    }
}

/// One file-tree entry as the saved snapshots describe it.
#[derive(Debug, PartialEq, Eq, Clone)]
struct Entry {
    path: PyStr,
    mode: u32,
    kind: String,
    target: Option<PyStr>,
    bytes: Option<Vec<u8>>,
}

mod tree {
    use super::support::*;
    use super::*;

    /// The tree below `root` as `snapshot()` of the capture lists it: depth first, children by the bytes of their names.
    pub fn snapshot(root: &Path) -> Vec<Entry> {
        let mut rows = Vec::new();
        visit(root, root, &mut rows);
        rows
    }

    fn visit(root: &Path, path: &Path, rows: &mut Vec<Entry>) {
        let info = std::fs::symlink_metadata(path).expect("an entry");
        let kind = info.file_type();
        let relative = path.strip_prefix(root).expect("below the root");
        let shown = if relative.as_os_str().is_empty() {
            PyStr::new(".")
        } else {
            PyStr::from_surrogateescape(relative.as_os_str().as_bytes())
        };
        let mode = info.permissions().mode() & 0o7777;
        if kind.is_symlink() {
            let target = std::fs::read_link(path).expect("a link target");
            rows.push(Entry {
                path: shown,
                mode,
                kind: "link".to_string(),
                target: Some(PyStr::from_surrogateescape(target.as_os_str().as_bytes())),
                bytes: None,
            });
        } else if kind.is_dir() {
            rows.push(Entry {
                path: shown,
                mode,
                kind: "dir".to_string(),
                target: None,
                bytes: None,
            });
            let mut children: Vec<PathBuf> = std::fs::read_dir(path)
                .expect("a directory")
                .map(|child| child.expect("a child").path())
                .collect();
            children.sort_by(|a, b| {
                a.file_name()
                    .unwrap()
                    .as_bytes()
                    .cmp(b.file_name().unwrap().as_bytes())
            });
            for child in children {
                visit(root, &child, rows);
            }
        } else if kind.is_file() {
            rows.push(Entry {
                path: shown,
                mode,
                kind: "file".to_string(),
                target: None,
                bytes: Some(std::fs::read(path).expect("file bytes")),
            });
        } else {
            panic!("unexpected private fixture kind: {path:?}");
        }
    }

    /// The saved snapshot, with the two original scripts (stored once as data) put back where the saved tree names them.
    pub fn saved(relative: &str) -> Vec<Entry> {
        items(&load(relative))
            .iter()
            .map(|row| {
                let kind = text_of(member(row, "kind")).to_utf8().expect("text");
                let bytes = match kind.as_str() {
                    "file" => Some(if has(row, "source") {
                        let name = text_of(member(row, "source")).to_utf8().expect("text");
                        std::fs::read(data(&format!("source/{name}"))).expect("a saved source")
                    } else {
                        base64_decode(&text_of(member(row, "base64")).to_utf8().expect("text"))
                    }),
                    _ => None,
                };
                let mode = match member(row, "mode") {
                    PyValue::Int(number) => u32::try_from(number.clone()).expect("a mode"),
                    other => panic!("{}", other.repr()),
                };
                Entry {
                    path: text_of(member(row, "path")),
                    mode,
                    kind,
                    target: has(row, "target").then(|| text_of(member(row, "target"))),
                    bytes,
                }
            })
            .collect()
    }
}

use support::*;

/// Directories the fixtures create get the mode of the capture's machinery under its umask (`0o775`), whatever the process's
/// umask is; the programs under test run under that same umask (see [`run_under_capture_umask`]).
fn make_dirs(path: &Path) {
    let mut missing: Vec<PathBuf> = Vec::new();
    let mut current = path.to_path_buf();
    while !current.exists() {
        missing.push(current.clone());
        if !current.pop() {
            break;
        }
    }
    for directory in missing.into_iter().rev() {
        std::fs::create_dir(&directory).expect("a fixture directory");
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o775))
            .expect("a mode");
    }
}

extern "C" {
    fn umask(mask: u32) -> u32;
}

/// The program runs with the umask `002` the capture ran under (set in the child only, before it starts).
fn run_under_capture_umask(command: &mut Command) -> &mut Command {
    use std::os::unix::process::CommandExt;
    // SAFETY: `umask` is async-signal-safe and touches only the child process's own state.
    unsafe {
        command.pre_exec(|| {
            umask(0o002);
            Ok(())
        });
    }
    command
}

/// The directory of one case: new, private, and removed (with what the case wrote into it) at the end.
struct Case {
    _directory: PrivateDirectory,
    root: PathBuf,
    root_text: PyStr,
}

impl Case {
    fn new(label: &str) -> Case {
        let directory =
            PrivateDirectory::create(&format!("generation-{label}-")).expect("a private directory");
        let root = directory.path().join("case");
        make_dirs(&root);
        let root_text = PyStr::from_surrogateescape(root.as_os_str().as_bytes());
        Case {
            _directory: directory,
            root,
            root_text,
        }
    }
}

/// `setup(case, root, cli)` of the capture's fixture machinery, declaratively.
fn setup(case: &PyValue, root: &Path, command_line: bool) {
    let write = |path: &Path, bytes: &[u8]| {
        std::fs::write(path, bytes).expect("write a fixture file");
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o664)).expect("a mode");
    };
    if command_line {
        let scripts = root.join("node/.claude/skills/vllm-recipe-explorer/scripts");
        make_dirs(&scripts);
        for name in ["gen_recipe_set.py", "feedback_log.py"] {
            let target = scripts.join(name);
            write(
                &target,
                &std::fs::read(data(&format!("source/{name}"))).expect("a saved source"),
            );
            std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o664))
                .expect("a mode");
        }
        if text_of(member(case, "owner")).equals("gen_recipe_set") {
            let defaults = load("default-inputs.json");
            if !(has(case, "missing_parsed")) {
                let parsed: Vec<u8> = if has(case, "parsed_bytes") {
                    hex_decode(
                        &text_of(member(case, "parsed_bytes"))
                            .to_utf8()
                            .expect("hex"),
                    )
                } else {
                    let value = if has(case, "parsed") {
                        member(case, "parsed")
                    } else {
                        member(&defaults, "parsed")
                    };
                    value.dumps_ascii().expect("JSON").into_bytes()
                };
                write(&root.join("parsed.json"), &parsed);
            }
            let recipe = if has(case, "recipe") {
                member(case, "recipe")
            } else {
                member(&defaults, "recipe")
            };
            write(
                &root.join("recipe.json"),
                recipe.dumps_ascii().expect("JSON").as_bytes(),
            );
            if has(case, "companion") {
                let companion = text_of(member(case, "companion")).to_utf8().expect("text");
                match companion.as_str() {
                    "missing" => {}
                    "actual" => write(
                        &scripts.join("run_trial.py"),
                        &std::fs::read(data("run_trial-source.txt"))
                            .expect("the companion source text"),
                    ),
                    other => write(&scripts.join("run_trial.py"), other.as_bytes()),
                }
            }
        }
    }
    if has(case, "setup") {
        for item in items(member(case, "setup")) {
            let parts = items(item);
            let kind = text_of(&parts[0]).to_utf8().expect("text");
            let path = root.join(native(&text_of(&parts[1])));
            assert!(
                !text_of(&parts[1])
                    .code_points()
                    .windows(2)
                    .any(|pair| pair == [46, 46])
                    && !text_of(&parts[1]).equals("")
                    && !text_of(&parts[1])
                        .to_utf8()
                        .unwrap_or_default()
                        .starts_with('/'),
                "a setup path stays inside the case"
            );
            make_dirs(path.parent().expect("a parent"));
            let mode = |at: usize| match &parts[at] {
                PyValue::Int(number) => u32::try_from(number.clone()).expect("a mode"),
                other => panic!("{}", other.repr()),
            };
            match kind.as_str() {
                "dir" => {
                    make_dirs(&path);
                    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode(2)))
                        .expect("a mode");
                }
                "link" => {
                    let target = native(&text_of(&parts[2]));
                    symlink(target, &path).expect("a fixture link");
                }
                "file" | "file-text" | "file-hex" => {
                    let bytes = if kind == "file-hex" {
                        hex_decode(&text_of(&parts[2]).to_utf8().expect("hex"))
                    } else {
                        text_of(&parts[2]).to_utf8().expect("text").into_bytes()
                    };
                    write(&path, &bytes);
                    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode(3)))
                        .expect("a mode");
                }
                other => panic!("{other}"),
            }
        }
    }
}

fn show(entries: &[Entry]) -> String {
    entries
        .iter()
        .map(|entry| {
            format!(
                "{:?} {:o} {} {:?} {}",
                entry.path.repr(),
                entry.mode,
                entry.kind,
                entry.target.as_ref().map(PyStr::repr),
                entry
                    .bytes
                    .as_ref()
                    .map_or(String::new(), |bytes| String::from_utf8_lossy(bytes)
                        .chars()
                        .take(80)
                        .collect())
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn tree_difference(label: &str, expected: &[Entry], actual: &[Entry]) -> Option<String> {
    if expected == actual {
        None
    } else {
        Some(format!(
            "{label}: the file tree differs\n--- want\n{}\n--- got\n{}",
            show(expected),
            show(actual)
        ))
    }
}

mod reference_cases {
    use super::*;

    /// A command line's saved standard error without the frames of the original's traceback: the header and each frame (`File`
    /// line, source line, caret marks) of the two original scripts or of the `json` modules; anything else is refused.
    fn strip_original_frames(saved: &[u8], scripts: &[u8]) -> Result<Vec<u8>, String> {
        const HEADER: &[u8] = b"Traceback (most recent call last):\n";
        let Some(mut rest) = saved.strip_prefix(HEADER) else {
            return Ok(saved.to_vec());
        };
        let allowed = |file: &[u8]| -> bool {
            [
                b"gen_recipe_set.py".as_slice(),
                b"feedback_log.py".as_slice(),
            ]
            .iter()
            .any(|name| file == [scripts, b"/", name].concat().as_slice())
                || file.ends_with(b"/json/__init__.py")
                || file.ends_with(b"/json/decoder.py")
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
        Ok(rest.to_vec())
    }

    fn api_request(case: &PyValue, root: &PyStr) -> PyValue {
        let mut request = PyDict::new();
        request.set("id", member(case, "id").clone());
        let mut operation = text_of(member(case, "owner"));
        operation.push_str(".");
        operation.push_py(&text_of(member(case, "function")));
        request.set("operation", PyValue::Str(operation));
        request.set("args", member(case, "args").clone());
        request.set("kwargs", member(case, "kwargs").clone());
        if has(case, "context") {
            request.set("context", member(case, "context").clone());
        }
        rebased(&PyValue::Dict(request), root)
    }

    fn run_api(case: &PyValue) -> Option<String> {
        let id = text_of(member(case, "id")).to_utf8().expect("text");
        let directory = Case::new(&id);
        setup(case, &directory.root, false);
        let before = tree::snapshot(&directory.root);
        let want_before = tree::saved(&format!("api/{id}/before.json"));
        if let Some(problem) =
            tree_difference(&format!("{id} fixture before"), &want_before, &before)
        {
            return Some(problem);
        }
        let request = api_request(case, &directory.root_text);
        let line = request.dumps_ascii().expect("a request line");
        let mut command = Command::new(env!("CARGO_BIN_EXE_recipe_generation_api"));
        run_under_capture_umask(&mut command);
        command
            .arg("evaluate-jsonl")
            .current_dir(&directory.root)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = spawn_locked(&mut command).expect("start the service");
        {
            use std::io::Write;
            let mut input = child.stdin.take().expect("stdin");
            input
                .write_all(line.as_bytes())
                .and_then(|()| input.write_all(b"\n"))
                .expect("send the request");
        }
        let output = child.wait_with_output().expect("the service ends");
        if output.status.code() != Some(0) || !output.stderr.is_empty() {
            return Some(format!(
                "{id}: transport {:?} {}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        let rows: Vec<&[u8]> = output
            .stdout
            .split(|&byte| byte == b'\n')
            .filter(|row| !row.is_empty())
            .collect();
        if rows.len() != 1 {
            return Some(format!("{id}: {} records", rows.len()));
        }
        let got = loads_text(&PyStr::from_surrogateescape(rows[0])).expect("a record");
        let expected = rebased(
            &load(&format!("api/{id}/original.json")),
            &directory.root_text,
        );
        if !same_value(&expected, &got) {
            return Some(format!(
                "{id}: the record differs\n  want {}\n  got  {}",
                expected.repr(),
                got.repr()
            ));
        }
        let after = tree::snapshot(&directory.root);
        tree_difference(&id, &tree::saved(&format!("api/{id}/after.json")), &after)
    }

    #[test]
    fn the_128_saved_function_calls_are_reproduced_whole_with_their_file_trees() {
        let cases = load("cases.json");
        assert_eq!(items(&cases).len(), 128);
        let failures: Vec<String> = items(&cases).iter().filter_map(run_api).collect();
        assert!(
            failures.is_empty(),
            "{} of 128 differ:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    /// A timestamp `YYYY-MM-DDTHH:MM:SS[.ffffff]` as a comparable tuple.
    fn instant(text: &str) -> Option<(u32, u32, u32, u32, u32, u32, u32)> {
        let (date, time) = text.split_once('T')?;
        let mut date = date.split('-');
        let mut time_parts = time.split(':');
        let (seconds, micros) = match time_parts.clone().nth(2)?.split_once('.') {
            Some((whole, fraction)) if fraction.len() == 6 => (whole, fraction.parse().ok()?),
            Some(_) => return None,
            None => (time_parts.clone().nth(2)?, 0),
        };
        Some((
            date.next()?.parse().ok()?,
            date.next()?.parse().ok()?,
            date.next()?.parse().ok()?,
            time_parts.next()?.parse().ok()?,
            time_parts.next()?.parse().ok()?,
            seconds.parse().ok()?,
            micros,
        ))
    }

    fn run_command_line(case: &PyValue) -> Option<String> {
        let id = text_of(member(case, "id")).to_utf8().expect("text");
        let owner = text_of(member(case, "owner")).to_utf8().expect("text");
        let directory = Case::new(&id);
        setup(case, &directory.root, true);
        // The program is staged where the deployed skill keeps its executables: a copy, never a link.
        let installed_dir = directory
            .root
            .join("node/.claude/skills/vllm-recipe-explorer/rust/runtime/target/release");
        make_dirs(&installed_dir);
        let binary = installed_dir.join(&owner);
        let built = match owner.as_str() {
            "gen_recipe_set" => env!("CARGO_BIN_EXE_gen_recipe_set"),
            _ => env!("CARGO_BIN_EXE_feedback_log"),
        };
        copy_locked(built, &binary).expect("stage the program");
        let staged = |entries: Vec<Entry>| -> (Vec<Entry>, Vec<Entry>) {
            let skip = [
                "node/.claude/skills/vllm-recipe-explorer/rust",
                "node/.claude/skills/vllm-recipe-explorer/rust/runtime",
                "node/.claude/skills/vllm-recipe-explorer/rust/runtime/target",
                "node/.claude/skills/vllm-recipe-explorer/rust/runtime/target/release",
            ];
            let binary_path = format!(
                "node/.claude/skills/vllm-recipe-explorer/rust/runtime/target/release/{owner}"
            );
            entries.into_iter().partition(|entry| {
                let path = entry.path.to_utf8().unwrap_or_default();
                skip.contains(&path.as_str()) || path == binary_path
            })
        };
        let (installed_before, before) = staged(tree::snapshot(&directory.root));
        if let Some(problem) = tree_difference(
            &format!("{id} fixture before"),
            &tree::saved(&format!("cli/{id}/before.json")),
            &before,
        ) {
            return Some(problem);
        }
        let saved = load(&format!("cli/{id}/original.json"));
        let arguments: Vec<OsString> = items(member(case, "args"))
            .iter()
            .map(|argument| {
                let rebased = replace_token(&text_of(argument), "@CASE@", &directory.root_text);
                OsString::from_vec(rebased.to_surrogateescape_bytes().expect("an argument"))
            })
            .collect();
        let began = recipe_runtime::feedback::local_now_iso();
        let mut command = Command::new(&binary);
        run_under_capture_umask(&mut command);
        command
            .args(&arguments)
            .current_dir(&directory.root)
            .env_clear();
        let output = output_locked(&mut command).expect("run the program");
        let ended = recipe_runtime::feedback::local_now_iso();
        let root_bytes = directory.root.as_os_str().as_bytes();
        let want_out = replace_bytes(
            &std::fs::read(data(&format!("cli/{id}/stdout.bin"))).expect("saved stdout"),
            "@CASE@",
            root_bytes,
        );
        let saved_err = std::fs::read(data(&format!("cli/{id}/stderr.bin"))).expect("saved stderr");
        let scripts = b"@CASE@/node/.claude/skills/vllm-recipe-explorer/scripts";
        let want_err = match strip_original_frames(&saved_err, scripts) {
            Ok(rest) => replace_bytes(&rest, "@CASE@", root_bytes),
            Err(problem) => {
                return Some(format!(
                    "{id}: the saved diagnostic is not in the expected shape: {problem}"
                ))
            }
        };
        let wanted_status = match member(&saved, "exit_code") {
            PyValue::Int(number) => number.to_string(),
            other => panic!("{}", other.repr()),
        };
        let mut problems: Vec<String> = Vec::new();
        if output.status.code().map(|code| code.to_string()) != Some(wanted_status.clone()) {
            problems.push(format!(
                "status {:?} want {wanted_status}",
                output.status.code()
            ));
        }
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
        let (installed_after, after) = staged(tree::snapshot(&directory.root));
        if installed_before != installed_after {
            problems.push("the staged program changed".to_string());
        }
        let mut expected_after = tree::saved(&format!("cli/{id}/after.json"));
        let mut actual_after = after;
        if has(case, "clock_dynamic") {
            // The production clock: both timestamps are the same ISO text inside the execution interval; everything else is exact.
            let log = "logs/feedback.jsonl";
            let at = actual_after
                .iter()
                .position(|entry| entry.path.equals(log))
                .expect("the log");
            let want_at = expected_after
                .iter()
                .position(|entry| entry.path.equals(log))
                .expect("the saved log");
            let raw = actual_after[at].bytes.clone().expect("log bytes");
            let want_raw = expected_after[want_at]
                .bytes
                .clone()
                .expect("saved log bytes");
            let record = loads_text(&PyStr::from_surrogateescape(&raw)).expect("a record");
            let want = loads_text(&PyStr::from_surrogateescape(&want_raw)).expect("a saved record");
            let stamp = text_of(member(&record, "timestamp"))
                .to_utf8()
                .expect("text");
            let keys = |value: &PyValue| match value {
                PyValue::Dict(map) => map.iter().map(|(key, _)| key.clone()).collect::<Vec<_>>(),
                other => panic!("{}", other.repr()),
            };
            let interval = instant(&began) <= instant(&stamp) && instant(&stamp) <= instant(&ended);
            if !(stamp
                == text_of(member(&record, "selection_timestamp"))
                    .to_utf8()
                    .expect("text")
                && instant(&stamp).is_some()
                && interval
                && keys(&record) == keys(&want))
            {
                problems.push(format!(
                    "the production clock's record is wrong: {stamp} in [{began}, {ended}]"
                ));
            }
            let mut projected = String::from_utf8(raw).expect("UTF-8 log");
            for key in ["timestamp", "selection_timestamp"] {
                let shown = |value: &PyValue| {
                    PyValue::Dict({
                        let mut one = PyDict::new();
                        one.set(key, member(value, key).clone());
                        one
                    })
                    .dumps_unicode()
                    .expect("JSON")
                    .to_utf8()
                    .expect("text")
                };
                let (needle, replacement) = (shown(&record), shown(&want));
                let needle = needle
                    .trim_start_matches('{')
                    .trim_end_matches('}')
                    .to_string();
                let replacement = replacement
                    .trim_start_matches('{')
                    .trim_end_matches('}')
                    .to_string();
                assert_eq!(projected.matches(&needle).count(), 1, "{needle}");
                projected = projected.replacen(&needle, &replacement, 1);
            }
            actual_after[at].bytes = Some(projected.into_bytes());
            expected_after[want_at].bytes = Some(want_raw);
        }
        if let Some(problem) = tree_difference(&id, &expected_after, &actual_after) {
            problems.push(problem);
        }
        if problems.is_empty() {
            None
        } else {
            Some(format!("{id} {arguments:?}: {}", problems.join("; ")))
        }
    }

    #[test]
    fn the_26_saved_command_lines_end_as_the_originals_did_with_their_file_trees() {
        let cases = load("cli-cases.json");
        assert_eq!(items(&cases).len(), 26);
        let failures: Vec<String> = items(&cases).iter().filter_map(run_command_line).collect();
        assert!(
            failures.is_empty(),
            "{} of 26 differ:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    #[test]
    fn the_diagnostic_comparator_removes_only_original_frames_and_keeps_the_whole_final_block() {
        let scripts = b"@CASE@/s";
        let saved: &[u8] = b"Traceback (most recent call last):\n  File \"@CASE@/s/gen_recipe_set.py\", line 9, in <module>\n    x()\n    ^^^\n  File \"/usr/lib/python3.12/json/decoder.py\", line 1, in f\n    y\nValueError: a\n    indented second line\nthird\n";
        assert_eq!(
            strip_original_frames(saved, scripts).expect("stripped"),
            b"ValueError: a\n    indented second line\nthird\n".to_vec()
        );
        assert_eq!(
            strip_original_frames(b"usage: x\n", scripts).expect("plain"),
            b"usage: x\n".to_vec()
        );
        assert!(strip_original_frames(b"Traceback (most recent call last):\n  File \"/elsewhere/other.py\", line 1, in f\n    x\nE: m\n", scripts).is_err());
        assert!(strip_original_frames(b"Traceback (most recent call last):\n  File \"@CASE@/s/feedback_log.py\", line 1, in f\n    x\n", scripts).is_err());
        assert!(
            strip_original_frames(b"Traceback (most recent call last):\nE: m\n", scripts).is_err()
        );
    }
}

/// Runs a short script of the reference interpreter (isolated mode) with the given text on standard input and the given
/// environment additions; the interpreter is only the oracle of stdlib primitives.
fn python(script: &str, input: &str, environment: &[(&str, &str)]) -> String {
    use std::io::Write;
    let mut command = Command::new("python3");
    command
        .args(["-I", "-W", "ignore", "-c", script])
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (name, value) in environment {
        command.env(name, value);
    }
    let mut child = spawn_locked(&mut command).unwrap_or_else(|error| {
        panic!("NOT_RUN: the interpreter is the oracle of this check: {error}")
    });
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(input.as_bytes())
        .expect("send the input");
    let output = child.wait_with_output().expect("the interpreter ends");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("text")
}

fn json_text(items: &[&str]) -> String {
    PyValue::List(items.iter().map(|item| PyValue::str(item)).collect())
        .dumps_ascii()
        .expect("JSON")
}

mod parser_cases {
    use super::*;
    use recipe_runtime::cliparse::{parse, Outcome, Value};
    use recipe_runtime::generation_cli::{feedback_spec, generator_spec};

    /// `argparse` itself (stdlib only; the parsers are written out here as the two programs define them, not imported from them).
    const PROBE: &str = r#"
import argparse, contextlib, io, json, os, sys
os.environ['COLUMNS'] = '80'
def generator():
    p = argparse.ArgumentParser(prog='gen_recipe_set.py', description="DGX Spark 서빙용 3종 세트(.yaml+.sh+.env) 생성")
    p.add_argument("--parsed", required=True, help="parse_model_config 결과 JSON 경로")
    p.add_argument("--recipe", required=True, help="선택된 레시피 후보 JSON 경로")
    p.add_argument("--name", required=True, help="3종 세트 base 이름")
    p.add_argument("--repo", required=True, help="레포 루트(configs/, envs/)")
    p.add_argument("--port", type=int, required=True, help="SERVING_PORT")
    p.add_argument("--served-model-name", required=True, help="--served-model-name 값")
    p.add_argument("--image", help="컨테이너 이미지 태그 → env 의 IMAGE_TAG")
    p.add_argument("--topology", default="single", choices=("single", "multi"), help="env 형태를 가른다. multi = master/slave 컨테이너 쌍 + 분산 키")
    p.add_argument("--vllm-version", help="multi env 의 VLLM_VERSION(이미지 정체성)")
    p.add_argument("--build-dockerfile", help="multi env 의 BUILD_DOCKERFILE")
    p.add_argument("--force", action="store_true", help="기존 파일 덮어쓰기 허용")
    return p
def feedback():
    p = argparse.ArgumentParser(prog='feedback_log.py', description="vllm-recipe-explorer 되먹임 로그(JSONL) append.")
    p.add_argument("--path", default="DEFAULT", help="JSONL 로그 경로(기본=feedback/recipe_feedback.jsonl)")
    p.add_argument("--model-id", required=True)
    p.add_argument("--quantization", required=True)
    p.add_argument("--max-model-len", type=int, required=True)
    p.add_argument("--gpu-memory-utilization", type=float, required=True)
    p.add_argument("--vram-budget-gb", type=float, required=True)
    p.add_argument("--estimated-vram-gb", type=float, required=True)
    p.add_argument("--tensor-parallel-size", type=int, required=True)
    p.add_argument("--safety-margin-threshold", type=float, required=True)
    p.add_argument("--selection-timestamp", default=None, help="미지정 시 now()")
    p.add_argument("--record", default=None, help="완성 record JSON 문자열(주면 개별 인자 대신 이 record 를 append)")
    return p
KEYS = {
 'gen': ['parsed','recipe','name','repo','port','served_model_name','image','topology','vllm_version','build_dockerfile','force'],
 'fb': ['path','model_id','quantization','max_model_len','gpu_memory_utilization','vram_budget_gb','estimated_vram_gb','tensor_parallel_size','safety_margin_threshold','selection_timestamp','record'],
}
for which, argv in json.load(sys.stdin):
    parser = generator() if which == 'gen' else feedback()
    out, err = io.StringIO(), io.StringIO()
    try:
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            ns = parser.parse_args(argv)
        print('ok ' + json.dumps([getattr(ns, k) for k in KEYS[which]]))
    except SystemExit as e:
        print('exit %s %s %s' % (e.code, json.dumps(out.getvalue()), json.dumps(err.getvalue())))
"#;

    fn minimal_generator() -> Vec<&'static str> {
        vec![
            "--parsed",
            "p",
            "--recipe",
            "r",
            "--name",
            "n",
            "--repo",
            "d",
            "--port",
            "80",
            "--served-model-name",
            "s",
        ]
    }

    fn minimal_feedback() -> Vec<&'static str> {
        vec![
            "--model-id",
            "m",
            "--quantization",
            "q",
            "--max-model-len",
            "4096",
            "--gpu-memory-utilization",
            "0.9",
            "--vram-budget-gb",
            "80",
            "--estimated-vram-gb",
            "22",
            "--tensor-parallel-size",
            "1",
            "--safety-margin-threshold",
            "0.9",
        ]
    }

    fn with(base: Vec<&'static str>, more: &[&'static str]) -> Vec<&'static str> {
        let mut all = base;
        all.extend_from_slice(more);
        all
    }

    fn lists() -> Vec<(&'static str, Vec<&'static str>)> {
        let g = minimal_generator;
        let f = minimal_feedback;
        let mut cases: Vec<(&'static str, Vec<&'static str>)> = vec![
            ("gen", vec![]),
            ("gen", vec!["--help"]),
            ("gen", vec!["-h"]),
            ("gen", vec!["-h", "--bogus"]),
            ("gen", vec!["--bogus", "-h"]),
            ("gen", g()),
            ("gen", with(g(), &["--force"])),
            ("gen", with(g(), &["--force=1"])),
            (
                "gen",
                with(
                    g(),
                    &[
                        "--topology",
                        "multi",
                        "--image",
                        "i",
                        "--vllm-version",
                        "v",
                        "--build-dockerfile",
                        "b",
                    ],
                ),
            ),
            ("gen", with(g(), &["--topology", "other"])),
            ("gen", with(g(), &["--topology=multi"])),
            ("gen", with(g(), &["--topology"])),
            ("gen", with(g(), &["--image"])),
            ("gen", with(g(), &["--image", "--force"])),
            ("gen", with(g(), &["--image", "-"])),
            ("gen", with(g(), &["--image", "--"])),
            ("gen", with(g(), &["--image=--x"])),
            ("gen", with(g(), &["--", "x"])),
            ("gen", with(g(), &["extra"])),
            ("gen", with(g(), &["--bogus"])),
            ("gen", with(g(), &["-x"])),
            ("gen", vec!["--p", "1"]),
            ("gen", vec!["--pa", "x"]),
            ("gen", vec!["--por", "5"]),
            ("gen", vec!["--po=5"]),
            ("gen", vec!["--s", "x"]),
            ("gen", vec!["--se", "x"]),
            ("gen", vec!["--serve", "x"]),
            ("gen", vec!["--v", "x"]),
            ("gen", vec!["--f"]),
            ("gen", vec!["--fo"]),
            ("gen", with(g(), &["--port", "81"])),
            (
                "gen",
                vec![
                    "--parsed",
                    "p",
                    "--recipe",
                    "r",
                    "--name",
                    "n",
                    "--repo",
                    "d",
                    "--port",
                    "0x10",
                    "--served-model-name",
                    "s",
                ],
            ),
            (
                "gen",
                vec![
                    "--parsed",
                    "p",
                    "--recipe",
                    "r",
                    "--name",
                    "n",
                    "--repo",
                    "d",
                    "--port",
                    " 7 ",
                    "--served-model-name",
                    "s",
                ],
            ),
            (
                "gen",
                vec![
                    "--parsed",
                    "p",
                    "--recipe",
                    "r",
                    "--name",
                    "n",
                    "--repo",
                    "d",
                    "--port",
                    "1_0",
                    "--served-model-name",
                    "s",
                ],
            ),
            (
                "gen",
                vec![
                    "--parsed",
                    "p",
                    "--recipe",
                    "r",
                    "--name",
                    "n",
                    "--repo",
                    "d",
                    "--port",
                    "-5",
                    "--served-model-name",
                    "s",
                ],
            ),
            (
                "gen",
                vec![
                    "--parsed",
                    "p",
                    "--recipe",
                    "r",
                    "--name",
                    "n",
                    "--repo",
                    "d",
                    "--port",
                    "",
                    "--served-model-name",
                    "s",
                ],
            ),
            (
                "gen",
                vec![
                    "--parsed",
                    "p",
                    "--recipe",
                    "r",
                    "--name",
                    "n",
                    "--repo",
                    "d",
                    "--port",
                    "8.0",
                    "--served-model-name",
                    "s",
                ],
            ),
            ("gen", vec!["--parsed", "--recipe", "r"]),
            (
                "gen",
                vec![
                    "--parsed=",
                    "--recipe=",
                    "--name=",
                    "--repo=",
                    "--port=1",
                    "--served-model-name=",
                ],
            ),
            (
                "gen",
                vec![
                    "--name",
                    "a b",
                    "--parsed",
                    "p",
                    "--recipe",
                    "r",
                    "--repo",
                    "d",
                    "--port",
                    "1",
                    "--served-model-name",
                    "한글",
                ],
            ),
            ("gen", vec!["--parsed", "p"]),
            ("fb", vec![]),
            ("fb", vec!["--help"]),
            ("fb", vec!["-h"]),
            ("fb", f()),
            ("fb", with(f(), &["--path", "x.jsonl"])),
            ("fb", with(f(), &["--path="])),
            ("fb", with(f(), &["--selection-timestamp", ""])),
            ("fb", with(f(), &["--record", "{}"])),
            ("fb", vec!["--record", "{}"]),
            ("fb", with(f(), &["--m", "x"])),
            ("fb", with(f(), &["--ma", "7"])),
            ("fb", with(f(), &["--model", "x"])),
            ("fb", with(f(), &["--g", "1"])),
            ("fb", with(f(), &["--v", "1"])),
            ("fb", with(f(), &["--s", "1"])),
            ("fb", with(f(), &["--se", "1"])),
            ("fb", with(f(), &["--sa", "1"])),
            ("fb", with(f(), &["--t", "1"])),
            ("fb", with(f(), &["--e", "1"])),
            ("fb", with(f(), &["--r", "x"])),
            ("fb", with(f(), &["--p", "x"])),
            ("fb", with(f(), &["--max-model-len", "1e3"])),
            ("fb", with(f(), &["--gpu-memory-utilization", "nan"])),
            ("fb", with(f(), &["--gpu-memory-utilization", "inf"])),
            ("fb", with(f(), &["--gpu-memory-utilization", "1e3"])),
            ("fb", with(f(), &["--gpu-memory-utilization", "0x1p3"])),
            ("fb", with(f(), &["--gpu-memory-utilization", "-1.5"])),
            ("fb", with(f(), &["--gpu-memory-utilization", "x"])),
            ("fb", with(f(), &["--tensor-parallel-size", "2.0"])),
            ("fb", with(f(), &["--vram-budget-gb", "--"])),
            ("fb", with(f(), &["--", "x"])),
            ("fb", with(f(), &["x"])),
            ("fb", with(f(), &["--bogus", "x"])),
        ];
        cases.push(("fb", vec!["--model-id", "m", "--quantization"]));
        cases
    }

    fn rust_answer(which: &str, argv: &[&str]) -> String {
        let spec = if which == "gen" {
            generator_spec()
        } else {
            feedback_spec()
        };
        let args: Vec<PyStr> = argv.iter().map(|argument| PyStr::new(argument)).collect();
        match parse(&spec, &args, Some("80")) {
            Outcome::Values(values) => {
                let keys: &[&str] = if which == "gen" {
                    &[
                        "parsed",
                        "recipe",
                        "name",
                        "repo",
                        "port",
                        "served_model_name",
                        "image",
                        "topology",
                        "vllm_version",
                        "build_dockerfile",
                        "force",
                    ]
                } else {
                    &[
                        "path",
                        "model_id",
                        "quantization",
                        "max_model_len",
                        "gpu_memory_utilization",
                        "vram_budget_gb",
                        "estimated_vram_gb",
                        "tensor_parallel_size",
                        "safety_margin_threshold",
                        "selection_timestamp",
                        "record",
                    ]
                };
                let row: Vec<PyValue> = keys
                    .iter()
                    .map(|key| match values.get(key) {
                        Some(Value::Text(text)) => PyValue::Str(text.clone()),
                        Some(Value::Int(number)) => PyValue::Int(number.clone()),
                        Some(Value::Float(number)) => PyValue::Float(*number),
                        Some(Value::Flag) => PyValue::Bool(true),
                        Some(Value::Texts(list)) => {
                            PyValue::List(list.iter().cloned().map(PyValue::Str).collect())
                        }
                        None if *key == "topology" => PyValue::str("single"),
                        None if *key == "path" => PyValue::str("DEFAULT"),
                        None if *key == "force" => PyValue::Bool(false),
                        None => PyValue::None,
                    })
                    .collect();
                format!("ok {}", PyValue::List(row).dumps_ascii().expect("JSON"))
            }
            Outcome::Exit {
                status,
                stdout,
                stderr,
            } => format!(
                "exit {status} {} {}",
                PyValue::Str(stdout).dumps_ascii().expect("JSON"),
                PyValue::Str(stderr).dumps_ascii().expect("JSON")
            ),
        }
    }

    #[test]
    fn the_two_parsers_agree_with_pythons_argparse_on_help_errors_abbreviations_markers_and_values()
    {
        let cases = lists();
        let request = PyValue::List(
            cases
                .iter()
                .map(|(which, argv)| {
                    PyValue::List(vec![
                        PyValue::str(which),
                        PyValue::List(argv.iter().map(|argument| PyValue::str(argument)).collect()),
                    ])
                })
                .collect(),
        )
        .dumps_ascii()
        .expect("JSON");
        let answers = python(PROBE, &request, &[]);
        let rows: Vec<&str> = answers.lines().collect();
        assert_eq!(rows.len(), cases.len());
        let mut wrong: Vec<String> = Vec::new();
        for ((which, argv), want) in cases.iter().zip(rows) {
            let got = rust_answer(which, argv);
            if got != want {
                wrong.push(format!("{which} {argv:?}:\n  want {want}\n  got  {got}"));
            }
        }
        assert!(
            wrong.is_empty(),
            "{} of {} argument lists differ:\n{}",
            wrong.len(),
            cases.len(),
            wrong.join("\n")
        );
        // `--check-parity` and the two raw `--` forms are ordinary text to the parser: see the saved command lines.
        let _ = json_text(&[]);
    }
}

mod clock_cases {
    use super::*;
    use recipe_runtime::feedback::{iso_text, local_now_iso};

    const NOW: &str = "import datetime,sys\nprint(datetime.datetime.now().isoformat())\n";

    fn parts(text: &str) -> (String, u32) {
        match text.split_once('.') {
            Some((whole, micros)) => (whole.to_string(), micros.parse().expect("microseconds")),
            None => (text.to_string(), 0),
        }
    }

    fn key(text: &str) -> (String, u32) {
        parts(text)
    }

    #[test]
    fn the_production_clock_reads_like_datetime_now_isoformat_in_the_process_s_zone() {
        for zone in [
            None,
            Some("UTC"),
            Some("Asia/Seoul"),
            Some("America/New_York"),
            Some("Pacific/Auckland"),
        ] {
            let environment: Vec<(&str, &str)> =
                zone.map(|zone| vec![("TZ", zone)]).unwrap_or_default();
            let first = python(NOW, "", &environment).trim().to_string();
            // The program under test is the real API binary reading the real clock for `build_record` (no context at all).
            let request = r#"{"id":"c","operation":"feedback_log.build_record","args":["m","q",1,0.5,1,1,1,0.5]}"#;
            let mut command = Command::new(env!("CARGO_BIN_EXE_recipe_generation_api"));
            command
                .arg("evaluate-jsonl")
                .env_clear()
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            for (name, value) in &environment {
                command.env(name, value);
            }
            let mut child = spawn_locked(&mut command).expect("start the service");
            {
                use std::io::Write;
                child
                    .stdin
                    .take()
                    .expect("stdin")
                    .write_all(format!("{request}\n").as_bytes())
                    .expect("send");
            }
            let output = child.wait_with_output().expect("ends");
            let second = python(NOW, "", &environment).trim().to_string();
            let record = loads_text(&PyStr::from_surrogateescape(
                output.stdout.split(|&b| b == b'\n').next().expect("a line"),
            ))
            .expect("a record");
            let stamp = text_of(member(member(&record, "value"), "timestamp"))
                .to_utf8()
                .expect("text");
            assert_eq!(
                stamp,
                text_of(member(member(&record, "value"), "selection_timestamp"))
                    .to_utf8()
                    .expect("text")
            );
            assert!(
                key(&first) <= key(&stamp) && key(&stamp) <= key(&second),
                "zone {zone:?}: {first} <= {stamp} <= {second}"
            );
        }
    }

    #[test]
    fn instants_are_written_like_fromtimestamp_with_the_microseconds_only_when_they_are_not_zero() {
        // Python's own local-time conversion of the same instants (the process's zone, the same in both).
        let instants: [(i64, u32); 8] = [
            (0, 0),
            (1, 5),
            (951_782_400, 999_999),
            (1_709_164_800, 0),
            (1_730_000_000, 100_000),
            (1_740_000_000, 10),
            (1_767_225_599, 1),
            (4_102_444_800, 0),
        ];
        let request = PyValue::List(
            instants
                .iter()
                .map(|(seconds, micros)| {
                    PyValue::List(vec![PyValue::int(*seconds), PyValue::int(*micros)])
                })
                .collect(),
        )
        .dumps_ascii()
        .expect("JSON");
        let answers = python(
            "import datetime,json,sys\nfor s,u in json.load(sys.stdin):\n print(datetime.datetime.fromtimestamp(s).replace(microsecond=u).isoformat())\n",
            &request,
            &[],
        );
        for ((seconds, micros), want) in instants.iter().zip(answers.lines()) {
            assert_eq!(iso_text(*seconds, *micros), want);
        }
        let shown = local_now_iso();
        assert!(shown.len() == 19 || shown.len() == 26, "{shown}");
    }
}

mod path_cases {
    use super::*;
    use recipe_runtime::feedback::normpath;
    use recipe_runtime::recipe_set::{makedirs, split_path};

    #[test]
    fn normpath_and_split_agree_with_posixpath() {
        let paths = [
            "",
            ".",
            "..",
            "/",
            "//",
            "///",
            "/a",
            "//a",
            "///a//b",
            "a/b/c",
            "a//b",
            "a/./b",
            "a/../b",
            "../a",
            "../../a",
            "/..",
            "/../a",
            "//..",
            "a/b/..",
            "a/b/../..",
            "a/b/../../..",
            "a/",
            "a//",
            "/a/",
            "/a//",
            "./",
            ".//a/",
            "a/..",
            "한글/./b/../c",
            "a/b/./",
            "x/../..",
            "/a/b/../../../c",
            "///.//..//a",
            "a/.../b",
            "...",
            "a/..b/c",
        ];
        let request = PyValue::List(paths.iter().map(|path| PyValue::str(path)).collect())
            .dumps_ascii()
            .expect("JSON");
        let answers = python(
            "import json,posixpath,sys\nfor p in json.load(sys.stdin):\n h,t=posixpath.split(p)\n print(json.dumps([posixpath.normpath(p),h,t]))\n",
            &request,
            &[],
        );
        let mut wrong = Vec::new();
        for (path, want) in paths.iter().zip(answers.lines()) {
            let text = PyStr::new(path);
            let (head, tail) = split_path(&text);
            let got = PyValue::List(vec![
                PyValue::Str(recipe_runtime::feedback::normpath(&text)),
                PyValue::Str(head),
                PyValue::Str(tail),
            ])
            .dumps_ascii()
            .expect("JSON");
            if got != want {
                wrong.push(format!("{path:?}: want {want} got {got}"));
            }
        }
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
        let _ = normpath;
    }

    /// `os.makedirs(path, exist_ok=True)` on trees made the same way for the interpreter and for this program: the same
    /// directories result and the same failure (class and text, the base directory aside) is raised.
    #[test]
    fn makedirs_makes_the_same_directories_and_fails_the_same_way_as_os_makedirs() {
        let paths = [
            "a/b/c",
            "a/b/c/",
            "a//b",
            "a/./b",
            "a/../b",
            "a/b/.",
            "file",
            "file/sub",
            "file/sub/deeper",
            "dir",
            "dir/new",
            "link",
            "link/new",
            "dangling",
            "dangling/new",
            "no/such/deep/tree",
            "x/../y",
            "x/y/..",
            "plain/",
        ];
        let prepare = |base: &Path| {
            std::fs::create_dir_all(base.join("dir")).expect("dir");
            std::fs::write(base.join("file"), b"x").expect("file");
            symlink("dir", base.join("link")).expect("link");
            symlink("missing", base.join("dangling")).expect("dangling");
        };
        let ours = PrivateDirectory::create("generation-makedirs-").expect("a private directory");
        let theirs =
            PrivateDirectory::create("generation-makedirs-oracle-").expect("a private directory");
        prepare(ours.path());
        prepare(theirs.path());
        let base_ours = ours.path().to_string_lossy().into_owned();
        let base_theirs = theirs.path().to_string_lossy().into_owned();
        let request = PyValue::List(paths.iter().map(|path| PyValue::str(path)).collect())
            .dumps_ascii()
            .expect("JSON");
        let script = format!(
            "import json,os,sys\nbase={base_theirs:?}\nfor p in json.load(sys.stdin):\n try:\n  os.makedirs(base+'/'+p, exist_ok=True); print(json.dumps(['ok']))\n except OSError as e:\n  print(json.dumps([type(e).__name__, str(e).replace(base,'@BASE@')]))\n"
        );
        let answers = python(&script, &request, &[]);
        let mut wrong = Vec::new();
        for (path, want) in paths.iter().zip(answers.lines()) {
            let full = PyStr::new(&format!("{base_ours}/{path}"));
            let got = match makedirs(&full) {
                Ok(()) => PyValue::List(vec![PyValue::str("ok")]),
                Err(failure) => PyValue::List(vec![
                    PyValue::str(failure.class),
                    PyValue::str(
                        &failure
                            .message
                            .to_utf8()
                            .unwrap_or_default()
                            .replace(&base_ours, "@BASE@"),
                    ),
                ]),
            }
            .dumps_ascii()
            .expect("JSON");
            if got != want {
                wrong.push(format!("{path:?}: want {want} got {got}"));
            }
        }
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
        let list = |base: &Path| {
            tree::snapshot(base)
                .into_iter()
                .map(|entry| {
                    (
                        entry.path.to_utf8().unwrap_or_default(),
                        entry.kind,
                        entry
                            .target
                            .map(|target| target.to_utf8().unwrap_or_default()),
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(list(ours.path()), list(theirs.path()));
    }
}

mod anchor_cases {
    use super::*;
    use recipe_runtime::anchor::{default_log_path, skill_root_of, trial_source_path};

    #[test]
    fn the_skill_root_is_five_directories_above_the_program_and_its_two_places_hang_off_it() {
        let root = skill_root_of(&PyStr::new(
            "/n/.claude/skills/vllm-recipe-explorer/rust/runtime/target/release/gen_recipe_set",
        ));
        assert!(root.equals("/n/.claude/skills/vllm-recipe-explorer"));
        assert!(default_log_path(&root)
            .equals("/n/.claude/skills/vllm-recipe-explorer/feedback/recipe_feedback.jsonl"));
        assert!(trial_source_path(&root)
            .equals("/n/.claude/skills/vllm-recipe-explorer/scripts/run_trial.py"));
        // A shallower path just ends at the top.
        assert!(skill_root_of(&PyStr::new("/a/b")).equals("/"));
    }

    /// A staged program learns its root from the path it was started by, lexically: links in that path are not resolved and a
    /// relative start is made absolute against the working directory it was started in.
    #[test]
    fn a_staged_program_derives_its_root_lexically_from_how_it_was_started() {
        let area = PrivateDirectory::create("generation-anchor-").expect("a private directory");
        let real = area.path().join("real");
        let staged = real.join("skill/rust/runtime/target/release");
        make_dirs(&staged);
        let program = staged.join("recipe_generation_api");
        copy_locked(env!("CARGO_BIN_EXE_recipe_generation_api"), &program)
            .expect("stage the program");
        symlink(&real, area.path().join("link")).expect("a link");
        let ask = |argv0: &Path, directory: &Path| -> String {
            let mut command = Command::new(argv0);
            command.arg("constants").current_dir(directory).env_clear();
            let output = output_locked(&mut command).expect("run the program");
            assert_eq!(
                output.status.code(),
                Some(0),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let record =
                loads_text(&PyStr::from_surrogateescape(&output.stdout)).expect("constants");
            text_of(member(&record, "DEFAULT_LOG_PATH"))
                .to_utf8()
                .expect("text")
        };
        let base = area.path().to_string_lossy().into_owned();
        // started through the link: the link stays in the path
        let through_link = area
            .path()
            .join("link/skill/rust/runtime/target/release/recipe_generation_api");
        assert_eq!(
            ask(&through_link, area.path()),
            format!("{base}/link/skill/feedback/recipe_feedback.jsonl")
        );
        // started by a relative path with dots: made absolute and normalized, not resolved
        let relative =
            Path::new("./link/skill/rust/../rust/runtime/target/release/recipe_generation_api");
        assert_eq!(
            ask(relative, area.path()),
            format!("{base}/link/skill/feedback/recipe_feedback.jsonl")
        );
        // started by the real path
        assert_eq!(
            ask(&program, area.path()),
            format!("{base}/real/skill/feedback/recipe_feedback.jsonl")
        );
    }

    /// `append` without a path uses the module default: the explicit root of the call when given, else the deployed root.
    #[test]
    fn append_without_a_path_uses_the_default_log_of_the_explicit_root_of_the_call() {
        let area = Case::new("default-log");
        let request = |extra: &str| {
            format!(
                r#"{{"id":"d","operation":"feedback_log.append","args":[{{"timestamp":"t","selection_timestamp":"s"}}],"kwargs":{{}},"context":{{"now_iso":"2026-11-06T01:02:03","skill_root":{extra}}}}}"#
            )
        };
        let root_json = PyValue::Str(area.root_text.clone())
            .dumps_ascii()
            .expect("JSON");
        let mut command = Command::new(env!("CARGO_BIN_EXE_recipe_generation_api"));
        command
            .arg("evaluate-jsonl")
            .current_dir(&area.root)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = spawn_locked(&mut command).expect("start");
        {
            use std::io::Write;
            child
                .stdin
                .take()
                .expect("stdin")
                .write_all(format!("{}\n", request(&root_json)).as_bytes())
                .expect("send");
        }
        let output = child.wait_with_output().expect("ends");
        assert_eq!(output.status.code(), Some(0));
        let record = loads_text(&PyStr::from_surrogateescape(
            output.stdout.split(|&b| b == b'\n').next().expect("a line"),
        ))
        .expect("a record");
        let expected_path = format!("{}/feedback/recipe_feedback.jsonl", area.root.display());
        assert_eq!(
            text_of(member(&record, "value")).to_utf8().expect("text"),
            expected_path
        );
        let written = std::fs::read_to_string(&expected_path).expect("the default log");
        assert!(
            written.starts_with(
                "{\"timestamp\": \"t\", \"selection_timestamp\": \"s\", \"actual_vram_gb\": null"
            ),
            "{written}"
        );
        assert!(written.ends_with("}\n"));
    }
}

/// The frozen post-quality measurement program, on a tiny schedule (2 rounds of 3 calls): these are diagnostic children of the
/// checks, not official measurements. Its rows come from saved original results: `_build_yaml` of a full recipe, `serve_env_pairs`
/// of a dict, `_normalize` of a fully timestamped record (the real clock still runs inside it), `generate` with `force` over
/// three existing files, and `append` of a complete record to a real JSONL log.
mod bench_cases {
    use super::*;

    fn case_of(id: &str) -> PyValue {
        let cases = load("cases.json");
        items(&cases)
            .iter()
            .find(|case| text_of(member(case, "id")).equals(id))
            .unwrap_or_else(|| panic!("{id}"))
            .clone()
    }

    fn saved_value(id: &str, root: &PyStr) -> PyValue {
        rebased(
            member(&load(&format!("api/{id}/original.json")), "value"),
            root,
        )
    }

    fn saved_text(id: &str, key: &str) -> PyStr {
        text_of(member(&load(&format!("api/{id}/original.json")), key))
    }

    fn row(metric: &str, id: &str, root: &PyStr, keep_context: bool) -> PyDict {
        let case = rebased(&case_of(id), root);
        let mut row = PyDict::new();
        row.set("metric", PyValue::str(metric));
        let mut operation = text_of(member(&case, "owner"));
        operation.push_str(".");
        operation.push_py(&text_of(member(&case, "function")));
        row.set("operation", PyValue::Str(operation));
        row.set("args", member(&case, "args").clone());
        row.set("kwargs", member(&case, "kwargs").clone());
        if keep_context && has(&case, "context") {
            row.set("context", member(&case, "context").clone());
        }
        row.set("expected", saved_value(id, root));
        row.set("expected_stderr", PyValue::Str(saved_text(id, "stderr")));
        row
    }

    /// `files` / `log` expectations from the saved tree after the call.
    fn file_expectation(id: &str, path: &str, root: &PyStr) -> PyValue {
        let entry = tree::saved(&format!("api/{id}/after.json"))
            .into_iter()
            .find(|entry| entry.path.equals(path))
            .unwrap_or_else(|| panic!("{path}"));
        let mut item = PyDict::new();
        let mut absolute = root.clone();
        absolute.push_str("/");
        absolute.push_py(&entry.path);
        item.set("path", PyValue::Str(absolute));
        item.set(
            "text",
            PyValue::str(&String::from_utf8(entry.bytes.expect("bytes")).expect("UTF-8")),
        );
        item.set("mode", PyValue::int(entry.mode));
        PyValue::Dict(item)
    }

    struct Prepared {
        case: Case,
        fixture: PathBuf,
    }

    fn prepare(edit: impl Fn(&mut Vec<PyDict>)) -> Prepared {
        let case = Case::new("bench");
        // The existing three files of the force row are made the way the saved case made them.
        setup(&case_of("api-083"), &case.root, false);
        let root = &case.root_text;
        let mut rows = vec![
            row("pure.yaml", "api-023", root, true),
            row("pure.pairs", "api-039", root, true),
            row("pure.normalize", "api-072", root, false),
        ];
        let mut generate = row("io.generate", "api-083", root, true);
        generate.set(
            "files",
            PyValue::List(vec![
                file_expectation("api-083", "node/configs/generated.yaml", root),
                file_expectation("api-083", "node/configs/generated.sh", root),
                file_expectation("api-083", "node/envs/.env.generated", root),
            ]),
        );
        rows.push(generate);
        let mut append = row("io.append", "api-101", root, true);
        let line = file_expectation("api-101", "logs/nested/feedback.jsonl", root);
        let mut log = PyDict::new();
        log.set("path", member(&line, "path").clone());
        log.set("line", member(&line, "text").clone());
        log.set("mode", member(&line, "mode").clone());
        append.set("log", PyValue::Dict(log));
        rows.push(append);
        edit(&mut rows);
        let fixture = case.root.join("fixture.json");
        let text = PyValue::List(rows.into_iter().map(PyValue::Dict).collect())
            .dumps_ascii()
            .expect("JSON");
        std::fs::write(&fixture, text).expect("the fixture");
        Prepared { case, fixture }
    }

    fn run(prepared: &Prepared) -> (Option<i32>, String, String) {
        let mut command = Command::new(env!("CARGO_BIN_EXE_recipe_generation_bench"));
        command
            .args(["--fixture"])
            .arg(&prepared.fixture)
            .args(["--rounds", "2", "--iterations", "3"])
            .env_clear();
        let output = output_locked(&mut command).expect("run the benchmark program");
        (
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }

    #[test]
    fn the_whole_program_measures_after_every_whole_value_stream_and_file_check_and_the_log_ends_with_one_line_per_call(
    ) {
        let prepared = prepare(|_| {});
        let (code, out, err) = run(&prepared);
        assert_eq!(code, Some(0), "{err}");
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 6, "{out}");
        assert_eq!(lines[0], "{\"rounds\": 2, \"iterations\": 3}");
        for (line, metric) in lines[1..].iter().zip([
            "pure.yaml",
            "pure.pairs",
            "pure.normalize",
            "io.generate",
            "io.append",
        ]) {
            let report = loads_text(&PyStr::new(line)).expect("a series");
            assert_eq!(
                text_of(member(&report, "metric")).to_utf8().expect("text"),
                metric
            );
            assert_eq!(items(member(&report, "rounds")).len(), 2);
        }
        // The warm call wrote one line and the log was then cleared outside the timers; the 2 x 3 measured calls left six.
        let log = prepared.case.root.join("logs/nested/feedback.jsonl");
        let written = std::fs::read(&log).expect("the log");
        let want = file_expectation(
            "api-101",
            "logs/nested/feedback.jsonl",
            &prepared.case.root_text,
        );
        let line = text_of(member(&want, "text")).to_utf8().expect("text");
        assert_eq!(String::from_utf8(written).expect("UTF-8"), line.repeat(6));
        assert_eq!(
            std::fs::metadata(&log)
                .expect("the log")
                .permissions()
                .mode()
                & 0o7777,
            0o664
        );
        // The generated files keep their expected text and modes (the force row rewrote them each time).
        for (path, mode) in [
            ("node/configs/generated.yaml", 0o640),
            ("node/configs/generated.sh", 0o755),
            ("node/envs/.env.generated", 0o600),
        ] {
            let info = std::fs::metadata(prepared.case.root.join(path)).expect("a generated file");
            assert_eq!(info.permissions().mode() & 0o7777, mode, "{path}");
        }
    }

    fn assert_no_report(prepared: &Prepared, fragment: &str) {
        let (code, out, err) = run(prepared);
        assert_eq!(code, Some(1), "{err}");
        assert!(
            out.is_empty(),
            "nothing is timed or reported after a failed check: {out}"
        );
        assert!(err.contains(fragment), "{err}");
    }

    #[test]
    fn a_wrong_value_a_wrong_stream_a_wrong_file_or_a_failing_call_ends_the_run_before_anything_is_reported(
    ) {
        // A later row with a wrong whole value: nothing is reported even though the first rows are right.
        assert_no_report(
            &prepare(|rows| {
                rows[1].set("expected", PyValue::List(vec![]));
            }),
            "differs from the saved original result",
        );
        // The standard error is part of the original result.
        assert_no_report(
            &prepare(|rows| {
                rows[3].set("expected_stderr", PyValue::str("unexpected warning\n"));
            }),
            "differs from the saved original result",
        );
        // A file the call must leave behind with other text.
        assert_no_report(
            &prepare(|rows| {
                let mut files = match rows[3].get("files") {
                    Some(PyValue::List(items)) => items.clone(),
                    _ => panic!(),
                };
                if let PyValue::Dict(first) = &mut files[0] {
                    first.set("text", PyValue::str("other"));
                }
                rows[3].set("files", PyValue::List(files));
            }),
            "differs from the saved original result",
        );
        // A log whose appended line is not the expected one.
        assert_no_report(
            &prepare(|rows| {
                let mut log = match rows[4].get("log") {
                    Some(PyValue::Dict(map)) => map.clone(),
                    _ => panic!(),
                };
                log.set("line", PyValue::str("{}\n"));
                rows[4].set("log", PyValue::Dict(log));
            }),
            "does not hold exactly 1 expected line",
        );
        // A call that fails before measurement.
        assert_no_report(
            &prepare(|rows| {
                rows[2].set("args", PyValue::List(vec![PyValue::int(3)]));
            }),
            "fails before measurement",
        );
    }
}

mod constants_cases {
    use super::*;

    /// The module constants as one flat object: tuples and sets are arrays (the declared conversion of the Python wrappers is
    /// theirs), the probe table is an object, in declaration order; values equal the original's definitions (re-read from
    /// the frozen source text that the checks keep as data).
    #[test]
    fn the_constants_are_one_flat_object_equal_to_the_originals_definitions() {
        let mut command = Command::new(env!("CARGO_BIN_EXE_recipe_generation_api"));
        command.arg("constants").env_clear();
        let output = output_locked(&mut command).expect("run the service");
        assert_eq!(output.status.code(), Some(0));
        let constants =
            loads_text(&PyStr::from_surrogateescape(&output.stdout)).expect("constants");
        let keys: Vec<String> = match &constants {
            PyValue::Dict(map) => map
                .iter()
                .map(|(key, _)| key.to_utf8().expect("text"))
                .collect(),
            other => panic!("{}", other.repr()),
        };
        assert_eq!(
            keys,
            [
                "SERVE_KNOB_KEYS",
                "_NATIVE_QUANT_VALUES",
                "_PARITY_EXEMPT",
                "_PROBE",
                "ESTIMATED_FIELDS",
                "NULLABLE_FIELDS",
                "DEFAULT_LOG_PATH"
            ]
        );
        let strings = |value: &PyValue| -> Vec<String> {
            items(value)
                .iter()
                .map(|item| text_of(item).to_utf8().expect("text"))
                .collect()
        };
        let source =
            std::fs::read_to_string(data("source/gen_recipe_set.py")).expect("the saved source");
        let feedback =
            std::fs::read_to_string(data("source/feedback_log.py")).expect("the saved source");
        // Every name the original's tuples list appears, in this order, in the original text of that tuple.
        let tuple_text = |text: &str, name: &str| -> String {
            let start = text
                .find(&format!("{name} = ("))
                .expect("a tuple definition");
            let end = text[start..].find("\n)").expect("its end");
            text[start..start + end].to_string()
        };
        let in_order = |text: &str, names: Vec<String>| {
            let mut from = 0;
            for name in names {
                let at = text[from..]
                    .find(&format!("\"{name}\""))
                    .unwrap_or_else(|| panic!("{name} in order"));
                from += at + name.len();
            }
        };
        in_order(
            &tuple_text(&source, "SERVE_KNOB_KEYS"),
            strings(member(&constants, "SERVE_KNOB_KEYS")),
        );
        assert_eq!(
            strings(member(&constants, "SERVE_KNOB_KEYS")).len(),
            tuple_text(&source, "SERVE_KNOB_KEYS").matches('"').count() / 2
        );
        in_order(
            &tuple_text(&feedback, "ESTIMATED_FIELDS"),
            strings(member(&constants, "ESTIMATED_FIELDS")),
        );
        assert_eq!(
            strings(member(&constants, "ESTIMATED_FIELDS")).len(),
            tuple_text(&feedback, "ESTIMATED_FIELDS")
                .matches('"')
                .count()
                / 2
        );
        in_order(
            &tuple_text(&feedback, "NULLABLE_FIELDS"),
            strings(member(&constants, "NULLABLE_FIELDS")),
        );
        assert_eq!(
            strings(member(&constants, "NULLABLE_FIELDS")).len(),
            tuple_text(&feedback, "NULLABLE_FIELDS")
                .matches('"')
                .count()
                / 2
        );
        // The sets: the same members as the original's set displays.
        let native = items(member(&constants, "_NATIVE_QUANT_VALUES"));
        assert_eq!(native.len(), 11);
        assert!(native.iter().any(|item| matches!(item, PyValue::None)));
        let exempt = strings(member(&constants, "_PARITY_EXEMPT"));
        assert_eq!(exempt.len(), 11);
        for name in &exempt {
            assert!(source.contains(&format!("\"{name}\"")), "{name}");
        }
        let probe = member(&constants, "_PROBE");
        assert_eq!(member(probe, "enforce_eager"), &PyValue::Bool(true));
        assert_eq!(member(probe, "batch"), &PyValue::int(4242));
        assert_eq!(
            member(probe, "gpu_memory_utilization"),
            &PyValue::Float(0.77)
        );
        assert!(text_of(member(&constants, "DEFAULT_LOG_PATH"))
            .to_utf8()
            .expect("text")
            .ends_with("/feedback/recipe_feedback.jsonl"));
    }
}

/// The parity repairs of the independent review (I1 raw keyword text, I2 the one NaN object, I3 the typed root of `_normalize`),
/// as raw request lines and raw reply lines authored in `data/generation_parity/cases.json` (built with `json.dumps` defaults by a
/// stdlib-only script; no original module is involved). The reply is compared as text, so a repeated raw key cannot hide behind a
/// parser that would collapse it.
mod parity_cases {
    use super::*;

    #[test]
    fn the_raw_replies_of_the_repaired_cases_are_exactly_the_authored_lines() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("checks")
            .join("data")
            .join("generation_parity")
            .join("cases.json");
        let bytes = std::fs::read(path).expect("the authored cases");
        let cases = loads_text(&PyStr::from_surrogateescape(&bytes)).expect("authored JSON");
        assert_eq!(items(&cases).len(), 14);
        let mut wrong: Vec<String> = Vec::new();
        for case in items(&cases) {
            let id = text_of(member(case, "id")).to_utf8().expect("text");
            let request = text_of(member(case, "request")).to_utf8().expect("text");
            let expected = text_of(member(case, "expected")).to_utf8().expect("text");
            let mut command = Command::new(env!("CARGO_BIN_EXE_recipe_generation_api"));
            command
                .arg("evaluate-jsonl")
                .env_clear()
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            let mut child = spawn_locked(&mut command).expect("start the service");
            {
                use std::io::Write;
                child
                    .stdin
                    .take()
                    .expect("stdin")
                    .write_all(format!("{request}\n").as_bytes())
                    .expect("send");
            }
            let output = child.wait_with_output().expect("the service ends");
            let got = String::from_utf8_lossy(&output.stdout).into_owned();
            if output.status.code() != Some(0)
                || !output.stderr.is_empty()
                || got != format!("{expected}\n")
            {
                wrong.push(format!(
                    "{id}\n  want {expected}\n  got  {}",
                    got.trim_end()
                ));
            }
        }
        assert!(
            wrong.is_empty(),
            "{} of 14 differ:\n{}",
            wrong.len(),
            wrong.join("\n")
        );
    }

    /// The measurement program reads its input with the same decoder, so a NaN key written twice is one key there too. A JSON
    /// `expected` cannot name a non-string key, so the row is refused by the preflight, and the refusal shows the value the call
    /// returned: one `nan` key holding the later value (a plain-float decoder would show two).
    #[test]
    fn the_measurement_programs_input_keeps_the_one_nan_object_too() {
        let case = Case::new("bench-nan");
        let fixture = case.root.join("fixture.json");
        let row = r#"[{"metric": "pure.normalize", "operation": "feedback_log._normalize", "args": [[[NaN, 1], [NaN, 2], ["timestamp", "provided"], ["selection_timestamp", "kept"]]], "kwargs": {}, "expected": {}}]"#;
        std::fs::write(&fixture, row).expect("the fixture");
        let mut command = Command::new(env!("CARGO_BIN_EXE_recipe_generation_bench"));
        command
            .args(["--fixture"])
            .arg(&fixture)
            .args(["--rounds", "2", "--iterations", "3"])
            .env_clear();
        let output = output_locked(&mut command).expect("run the benchmark program");
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let message = String::from_utf8_lossy(&output.stderr).into_owned();
        assert!(
            message.contains("{nan: 2, 'timestamp': 'provided', 'selection_timestamp': 'kept'"),
            "{message}"
        );
    }
}
