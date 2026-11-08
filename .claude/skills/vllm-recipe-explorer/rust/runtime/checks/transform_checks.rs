use agent_runtime_support::pyjson::loads_text;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use recipe_runtime::model_files::PrivateDirectory;
use recipe_runtime::observe::same_value;
use std::collections::BTreeMap;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// ── data ─────────────────────────────────────────────────────────────────────────────────────────

fn data(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("checks")
        .join("data")
        .join("transform_reference")
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
        PyValue::Str(text) => {
            // By code point: a saved text may hold a lone surrogate (an undecodable input byte kept by the original).
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
    let area =
        PrivateDirectory::create(&format!("transform-{name}-")).expect("a private directory");
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
        .stdin(Stdio::piped());
}

/// Starts `program args...` the way a caller passes a protocol descriptor (Python's `pass_fds`): the pipe the caller writes to is
/// descriptor 3 of the program, and descriptor 0 is a file that can only be written to (so any read of standard input fails loudly). Returns the command (streams
/// piped) and the write end of the protocol pipe.
fn over_descriptor(program: &str, args: &[&str], zero: &Path) -> (Command, std::io::PipeWriter) {
    let (reader, writer) = std::io::pipe().expect("a pipe");
    let script = "exec \"$@\" 3<&0 0>\"$F0\"";
    let mut command = Command::new("/bin/sh");
    command
        .args(["-c", script, "sh", program])
        .args(args)
        .env_clear()
        .env("F0", zero)
        .stdin(Stdio::from(reader))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    (command, writer)
}

// ── running with a standard input ────────────────────────────────────────────────────────────────

/// Runs the command with `input` as its standard input (written whole, then closed) and collects its streams.
fn run_with_input(mut command: Command, input: &[u8]) -> std::process::Output {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("start the program");
    let mut pipe = child.stdin.take().expect("stdin");
    let bytes = input.to_vec();
    let feeder = std::thread::spawn(move || {
        let _ = pipe.write_all(&bytes);
    });
    let output = child.wait_with_output().expect("the program ends");
    feeder.join().expect("the feeder");
    output
}

fn stdin_of(request: &PyValue) -> Vec<u8> {
    match request {
        PyValue::Dict(map) => map
            .get("stdin_hex")
            .map(|hex| hex_decode(&text_of(hex)))
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

// ── saved traceback projection ───────────────────────────────────────────────────────────────────

const HEADER: &[u8] = b"Traceback (most recent call last):\n";

/// The saved standard error with the original's traceback frames removed and nothing else: for each traceback section the header and
/// the frames of a known file (the two original scripts, the collector, the interpreter's frozen codec module) go with their source
/// lines; every other byte stays, the separators between chained exceptions and every complete final block included. A frame of any
/// other file, or a header without frames, is refused.
fn project_traceback(saved: &[u8]) -> Result<Vec<u8>, String> {
    let allowed = |file: &[u8]| -> bool {
        file == b"@CASE@/node/.claude/skills/vllm-recipe-explorer/scripts/rope_scaling_translate.py"
            || file == b"@CASE@/node/.claude/skills/vllm-recipe-explorer/scripts/parse_vllm_log.py"
            || file.ends_with(b"/r29_original_one_call.py")
            || file == b"<frozen codecs>"
    };
    let mut kept = Vec::new();
    let mut rest = saved;
    loop {
        let found = if rest.starts_with(HEADER) {
            Some(0)
        } else {
            rest.windows(HEADER.len() + 1)
                .position(|window| window[0] == b'\n' && &window[1..] == HEADER)
                .map(|at| at + 1)
        };
        let Some(start) = found else {
            kept.extend_from_slice(rest);
            return Ok(kept);
        };
        kept.extend_from_slice(&rest[..start]);
        rest = &rest[start + HEADER.len()..];
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
    }
}

fn status_of(case: &PyValue) -> String {
    match member(case, "exit_code") {
        PyValue::Int(number) => number.to_string(),
        other => panic!("{}", other.repr()),
    }
}

// ── reference cases ──────────────────────────────────────────────────────────────────────────────

mod reference_cases {
    use super::*;

    fn api_problems(bundle: &PyValue, case: &PyValue) -> Vec<String> {
        let request = member(case, "request");
        let id = text_of(member(request, "id"));
        let work = new_case(bundle, &id, member(case, "before"));
        let (spec, control) = (work.area.join("spec.json"), work.area.join("control.json"));
        write_json(&spec, request);
        let mut command = Command::new(env!("CARGO_BIN_EXE_recipe_transform_fixture_api"));
        command
            .arg("case")
            .arg("--spec")
            .arg(&spec)
            .arg("--root")
            .arg(&work.root)
            .arg("--control")
            .arg(&control);
        fixture_environment(&mut command, &work);
        let output = run_with_input(command, b"");
        let mut problems = Vec::new();
        if output.status.code().map(|code| code.to_string()) != Some(status_of(case)) {
            problems.push(format!("status {:?}", output.status.code()));
        }
        let root = root_text(&work);
        let want_out = replace_bytes(
            &hex_decode(&text_of(member(case, "stdout_hex"))),
            "@CASE@",
            root.as_bytes(),
        );
        let want_err = replace_bytes(
            &hex_decode(&text_of(member(case, "stderr_hex"))),
            "@CASE@",
            root.as_bytes(),
        );
        if output.stdout != want_out || output.stderr != want_err {
            problems.push(format!(
                "streams differ: {:?}",
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
    fn the_167_saved_function_calls_are_reproduced_whole_with_their_values_types_key_order_exceptions_globals_and_tree(
    ) {
        let bundle = load("bundle.json");
        let cases = items(member(&bundle, "api"));
        assert_eq!(cases.len(), 167);
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
        let mut command = Command::new(env!("CARGO_BIN_EXE_recipe_transform_fixture"));
        command
            .arg("--spec")
            .arg(&spec)
            .arg("--root")
            .arg(&work.root)
            .arg("--control")
            .arg(&control);
        fixture_environment(&mut command, &work);
        let output = run_with_input(command, &stdin_of(request));
        let root = root_text(&work);
        let mut problems = Vec::new();
        if output.status.code().map(|code| code.to_string()) != Some(status_of(case)) {
            problems.push(format!(
                "status {:?} want {}",
                output.status.code(),
                status_of(case)
            ));
        }
        let want_out = replace_bytes(
            &hex_decode(&text_of(member(case, "stdout_hex"))),
            "@CASE@",
            root.as_bytes(),
        );
        if output.stdout != want_out {
            problems.push(format!(
                "stdout differs:\n--- want\n{}\n--- got\n{}",
                String::from_utf8_lossy(&want_out),
                String::from_utf8_lossy(&output.stdout)
            ));
        }
        match project_traceback(&hex_decode(&text_of(member(case, "stderr_hex")))) {
            Ok(projected) => {
                let want_err = replace_bytes(&projected, "@CASE@", root.as_bytes());
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
            .map(|problem| format!("{id}: {problem}"))
            .collect()
    }

    #[test]
    fn the_50_saved_command_lines_end_as_the_original_did_with_their_trace_streams_status_and_tree()
    {
        let bundle = load("bundle.json");
        let cases = items(member(&bundle, "cli"));
        assert_eq!(cases.len(), 50);
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
    fn the_diagnostic_comparator_removes_only_original_frames_and_keeps_chains_and_final_blocks() {
        let saved: &[u8] = b"Traceback (most recent call last):\n  File \"@CASE@/node/.claude/skills/vllm-recipe-explorer/scripts/parse_vllm_log.py\", line 1, in <module>\n    x\nRuntimeError: a\nsecond line\n\nDuring handling of the above exception, another exception occurred:\n\nTraceback (most recent call last):\n  File \"/x/r29_original_one_call.py\", line 9, in <module>\n    y\n  File \"<frozen codecs>\", line 322, in decode\nUnicodeDecodeError: a\nsecond line\n";
        assert_eq!(
            project_traceback(saved).expect("projected"),
            b"RuntimeError: a\nsecond line\n\nDuring handling of the above exception, another exception occurred:\n\nUnicodeDecodeError: a\nsecond line\n".to_vec()
        );
        assert_eq!(
            project_traceback(b"[rope-translate] x\n").expect("plain"),
            b"[rope-translate] x\n".to_vec()
        );
        assert!(project_traceback(b"Traceback (most recent call last):\n  File \"/elsewhere/other.py\", line 1, in f\n    x\nE: m\n").is_err());
        assert!(project_traceback(b"Traceback (most recent call last):\nE: m\n").is_err());
        let record = loads_text(&PyStr::new(r#"{"a": 1, "b": [true, null]}"#)).expect("json");
        for changed in [
            r#"{"a": 1.0, "b": [true, null]}"#,
            r#"{"b": [true, null], "a": 1}"#,
            r#"{"a": 1, "b": [1, null]}"#,
        ] {
            assert!(
                !same_value(&record, &loads_text(&PyStr::new(changed)).expect("json")),
                "{changed}"
            );
        }
    }
}

// ── the production programs ──────────────────────────────────────────────────────────────────────

fn production_command(program: &str) -> Command {
    let mut command = Command::new(program);
    command.env_clear();
    command
}

fn program_of(owner: &str) -> &'static str {
    match owner {
        "rope_scaling_translate" => env!("CARGO_BIN_EXE_rope_scaling_translate"),
        "parse_vllm_log" => env!("CARGO_BIN_EXE_parse_vllm_log"),
        other => panic!("{other}"),
    }
}

mod production_cases {
    use super::*;

    fn run_over_descriptor(
        arguments: &[&str],
        protocol: &str,
        zero: &Path,
    ) -> std::process::Output {
        let mut command = {
            let (command, mut writer) =
                over_descriptor(env!("CARGO_BIN_EXE_recipe_transform_api"), arguments, zero);
            writer
                .write_all(protocol.as_bytes())
                .expect("the protocol bytes");
            drop(writer);
            command
        };
        command.output().expect("run the service")
    }

    fn records_of(output: &std::process::Output) -> Vec<PyValue> {
        String::from_utf8(output.stdout.clone())
            .expect("text")
            .lines()
            .map(|line| loads_text(&PyStr::new(line)).expect("a record"))
            .collect()
    }

    /// Every saved function call, asked of the production service through `evaluate-jsonl --input-fd 3` (standard input is a file that
    /// can only be written, so any read of it would fail): the record the original gave minus the collector's observations; the
    /// globals a case replaced travel as the member of their own.
    #[test]
    fn the_167_saved_calls_agree_through_the_production_service() {
        let bundle = load("bundle.json");
        let area = PrivateDirectory::create("transform-service-").expect("a private directory");
        let mut protocol = String::new();
        let mut wanted = Vec::new();
        let root = area.path().join("case");
        let root_text = root.to_str().expect("a path").to_string();
        for case in items(member(&bundle, "api")) {
            let request = member(case, "request");
            let owner = text_of(member(request, "owner"));
            let mut line = PyDict::new();
            line.set("id", member(request, "id").clone());
            line.set(
                "operation",
                PyValue::str(&format!("{owner}.{}", text_of(member(request, "function")))),
            );
            line.set("args", rebase(member(request, "args"), &root_text));
            line.set("kwargs", rebase(member(request, "kwargs"), &root_text));
            if matches!(member(request, "context"), PyValue::Dict(context) if !context.is_empty()) {
                line.set(
                    "globals",
                    member(member(case, "original"), "globals_before").clone(),
                );
            }
            protocol.push_str(&PyValue::Dict(line).dumps_ascii().expect("a request"));
            protocol.push('\n');
            wanted.push(without(
                member(case, "original"),
                &["owner", "trace", "globals_before", "globals_after"],
            ));
        }
        let zero = area.path().join("zero");
        let output = run_over_descriptor(&["evaluate-jsonl", "--input-fd", "3"], &protocol, &zero);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        let got = records_of(&output);
        assert_eq!(got.len(), wanted.len());
        let wrong: Vec<String> = wanted
            .iter()
            .zip(&got)
            .map(|(want, got)| (rebase(want, &root_text), got))
            .filter(|(want, got)| !same_value(want, got))
            .map(|(want, got)| format!("want {}\n  got {}", want.repr(), got.repr()))
            .collect();
        assert!(
            wrong.is_empty(),
            "{} differ:\n{}",
            wrong.len(),
            wrong.join("\n")
        );
    }

    #[test]
    fn the_constants_are_both_owners_saved_values_and_a_request_member_beyond_the_five_is_refused()
    {
        let bundle = load("bundle.json");
        let output = Command::new(env!("CARGO_BIN_EXE_recipe_transform_api"))
            .arg("constants")
            .env_clear()
            .output()
            .expect("run");
        let got = loads_text(&PyStr::from_surrogateescape(&output.stdout)).expect("constants");
        assert!(
            same_value(member(member(&bundle, "constants"), "owners"), &got),
            "{}",
            got.repr()
        );
        let area = PrivateDirectory::create("transform-refuse-").expect("a private directory");
        let zero = area.path().join("zero");
        let requests = [
            r#"{"id": 1, "operation": "parse_vllm_log.parse_log", "args": [""], "context": {}}"#,
            r#"{"id": 2, "operation": "parse_vllm_log.parse_log", "args": [""], "globals": {"_NUM": "x"}}"#,
            r#"{"id": 3, "operation": "rope_scaling_translate.other", "args": []}"#,
            r#"{"id": 4, "operation": "other.translate", "args": []}"#,
            r#"{"id": 5, "operation": "rope_scaling_translate.translate", "args": [{}, 1], "globals": {"KILL": []}}"#,
            r#"{"id": 6, "operation": "rope_scaling_translate.translate", "args": [{"max_position_embeddings": 10, "rope_scaling": {}}, 20], "globals": {"ROPE_CONTAINERS": [[]]}}"#,
        ];
        let output = run_over_descriptor(
            &["evaluate-jsonl", "--input-fd", "3"],
            &format!("{}\n", requests.join("\n")),
            &zero,
        );
        let got = records_of(&output);
        let kinds: Vec<(String, String)> = got
            .iter()
            .map(|row| match row {
                PyValue::Dict(map) if map.contains_key("type") => (
                    text_of(member(row, "type")),
                    text_of(member(row, "message")),
                ),
                _ => (String::new(), String::new()),
            })
            .collect();
        assert_eq!(kinds[0].0, "RequestError");
        assert!(kinds[0]
            .1
            .starts_with("the request has a member this service does not take: "));
        assert_eq!(
            kinds[1],
            (
                "RequestError".to_string(),
                "parse_vllm_log has no replaceable globals".to_string()
            )
        );
        assert_eq!(
            kinds[2],
            (
                "AttributeError".to_string(),
                "module 'rope_scaling_translate' has no attribute 'other'".to_string()
            )
        );
        assert_eq!(
            kinds[3],
            (
                "ModuleNotFoundError".to_string(),
                "No module named 'other'".to_string()
            )
        );
        assert_eq!(kinds[4].0, "RequestError");
        // A replaced `ROPE_CONTAINERS` with the empty path reads the config itself (a global a request may set).
        assert_eq!(
            text_of(member(&got[5], "kind")),
            "value",
            "{}",
            got[5].repr()
        );
        assert_eq!(
            text_of(member(member(&got[5], "value"), "container_path")),
            ""
        );
        for arguments in [
            vec!["evaluate-jsonl", "--input-fd", "1"],
            vec!["constants", "x"],
            vec![],
        ] {
            let output = Command::new(env!("CARGO_BIN_EXE_recipe_transform_api"))
                .args(&arguments)
                .env_clear()
                .stdin(Stdio::null())
                .output()
                .expect("run");
            assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        }
        let output = Command::new(env!("CARGO_BIN_EXE_recipe_transform_api"))
            .args(["evaluate-jsonl", "--input-fd", "3"])
            .env_clear()
            .stdin(Stdio::null())
            .output()
            .expect("run");
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stderr).contains("not an open descriptor"));
    }

    /// The production command lines over the real files of an owned root: every saved case that is not a collector-only trace, with the
    /// saved stdout, the projected saved stderr and the saved status.
    #[test]
    fn the_50_saved_command_lines_give_the_same_streams_and_status_as_the_production_programs() {
        let bundle = load("bundle.json");
        let mut failures = Vec::new();
        for case in items(member(&bundle, "cli")) {
            let request = member(case, "request");
            let id = text_of(member(request, "id"));
            let work = new_case(&bundle, &id, member(case, "before"));
            let root = root_text(&work);
            let args: Vec<String> = items(member(request, "args"))
                .iter()
                .map(|arg| text_of(&rebase(arg, &root)))
                .collect();
            let mut command = production_command(program_of(&text_of(member(request, "owner"))));
            command
                .args(&args)
                .current_dir(&work.root)
                .env("TMPDIR", work.root.join("private-tmp"));
            let output = run_with_input(command, &stdin_of(request));
            if output.status.code().map(|code| code.to_string()) != Some(status_of(case)) {
                failures.push(format!("{id}: status {:?}", output.status.code()));
            }
            let want_out = replace_bytes(
                &hex_decode(&text_of(member(case, "stdout_hex"))),
                "@CASE@",
                root.as_bytes(),
            );
            if output.stdout != want_out {
                failures.push(format!("{id}: stdout differs"));
            }
            let projected = project_traceback(&hex_decode(&text_of(member(case, "stderr_hex"))))
                .expect("a known shape");
            if output.stderr != replace_bytes(&projected, "@CASE@", root.as_bytes()) {
                failures.push(format!(
                    "{id}: stderr differs: {:?}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    /// Reading the source: nothing selects another file, provider or path, and no operational capture path is in the programs.
    #[test]
    fn the_sources_have_no_selector_no_fixture_module_and_no_operational_path() {
        let sources = [
            (
                "rope_translate.rs",
                include_str!("../src/rope_translate.rs"),
            ),
            ("vllm_log.rs", include_str!("../src/vllm_log.rs")),
            ("transform_api.rs", include_str!("../src/transform_api.rs")),
            (
                "transform_bench.rs",
                include_str!("../src/transform_bench.rs"),
            ),
            ("transform_cli.rs", include_str!("../src/transform_cli.rs")),
            (
                "bin/rope_scaling_translate.rs",
                include_str!("../src/bin/rope_scaling_translate.rs"),
            ),
            (
                "bin/parse_vllm_log.rs",
                include_str!("../src/bin/parse_vllm_log.rs"),
            ),
            (
                "bin/recipe_transform_api.rs",
                include_str!("../src/bin/recipe_transform_api.rs"),
            ),
            (
                "bin/recipe_transform_bench.rs",
                include_str!("../src/bin/recipe_transform_bench.rs"),
            ),
        ];
        let mut found = Vec::new();
        for (name, text) in sources {
            for forbidden in [
                "env::var",
                "getenv",
                "transform_fixture",
                "real-worker",
                "val_agent",
                "CARGO_MANIFEST_DIR",
                "BoundaryViolation",
                "Command::new",
                "TcpStream",
            ] {
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

// ── new checks: the dynamic corners and the sinks ────────────────────────────────────────────────

mod new_cases {
    use super::*;

    fn call(owner: &str, function: &str, args: &str) -> PyValue {
        let area = PrivateDirectory::create("transform-new-").expect("a private directory");
        let root = area.path().join("case");
        std::fs::create_dir(&root).expect("a root");
        let spec = format!(
            r#"{{"id": "n", "owner": "{owner}", "function": "{function}", "args": {args}, "kwargs": {{}}, "context": {{}}}}"#
        );
        std::fs::write(area.path().join("spec.json"), spec).expect("a spec");
        let control = area.path().join("control.json");
        let mut command = Command::new(env!("CARGO_BIN_EXE_recipe_transform_fixture_api"));
        command
            .args(["case", "--spec"])
            .arg(area.path().join("spec.json"))
            .arg("--root")
            .arg(&root)
            .arg("--control")
            .arg(&control)
            .env_clear();
        let output = run_with_input(command, b"");
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        read_control(&control)
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

    fn log_value(text: &str) -> PyValue {
        let args = PyValue::List(vec![PyValue::str(text)])
            .dumps_ascii()
            .expect("json");
        call("parse_vllm_log", "parse_log", &args)
    }

    fn number(value: &PyValue, key: &str) -> f64 {
        match member(value, key) {
            PyValue::Float(number) => *number,
            other => panic!("{key}: {}", other.repr()),
        }
    }

    #[test]
    fn the_log_patterns_are_unicode_aware_case_insensitive_and_take_the_leftmost_longest_free_match(
    ) {
        // `\\s` is the Unicode whitespace class of a str pattern (no-break space, em space), IGNORECASE folds the letters, and the digits
        // group of `[\\d,]` takes a comma-grouped number that `float` accepts once the commas are removed.
        let record = log_value("MODEL\u{a0}WEIGHTS\u{2003}TAKE 1,234.50 gib");
        let value = value_of(&record);
        assert_eq!(number(value, "weights_gib"), 1234.5);
        // The long s folds to `s` under IGNORECASE for a str pattern.
        let record = log_value("Model\u{a0}weight\u{17f} take 2.00GiB");
        assert_eq!(number(value_of(&record), "weights_gib"), 2.0);
        // Nothing matches: every field is null and `raw_matched` is empty (a dict, not null).
        let record = log_value("nothing here");
        let value = value_of(&record);
        assert!(matches!(member(value, "weights_gib"), PyValue::None));
        assert!(matches!(member(value, "raw_matched"), PyValue::Dict(map) if map.is_empty()));
    }

    fn overhead_of(total: &str, weights: &str, kv: &str) -> PyValue {
        let text = format!(
            "Model weights take {weights}GiB; the rest of the memory reserved for KV Cache is {kv}GiB.\ntotal_gpu_memory (95.00GiB) x gpu_memory_utilization (0.90) = {total}GiB\n"
        );
        let record = log_value(&text);
        member(value_of(&record), "non_kv_overhead_gib").clone()
    }

    /// The derived overhead is the interpreter's float subtraction, kept at exactly zero and dropped (null) only when negative; the
    /// rounding residue of `0.30 - 0.10 - 0.20` is kept as the tiny positive float it is.
    #[test]
    fn the_overhead_is_kept_at_zero_dropped_when_negative_and_carries_the_floating_point_residue() {
        assert!(
            matches!(overhead_of("10.00", "4.00", "6.00"), PyValue::Float(number) if number == 0.0)
        );
        assert!(matches!(
            overhead_of("10.00", "6.00", "6.00"),
            PyValue::None
        ));
        // `0.30 - 0.10 - 0.20` is a tiny negative float (-2.7755575615628914e-17): the rounding residue counts as negative.
        assert!(matches!(overhead_of("0.30", "0.10", "0.20"), PyValue::None));
        // `0.04 - 0.03 - 0.01` is a tiny positive float (1.734723475976807e-18), kept as it is.
        assert!(
            matches!(overhead_of("0.04", "0.03", "0.01"), PyValue::Float(number) if number == 0.04 - 0.03 - 0.01)
        );
    }

    #[test]
    fn a_log_argument_that_is_not_text_is_the_interpreters_type_error() {
        for (argument, name) in [
            ("null", "NoneType"),
            ("1", "int"),
            ("1.5", "float"),
            ("[]", "list"),
            ("{}", "dict"),
            ("true", "bool"),
        ] {
            let record = call("parse_vllm_log", "parse_log", &format!("[{argument}]"));
            assert_eq!(text_of(member(&record, "kind")), "exception");
            assert_eq!(text_of(member(&record, "type")), "TypeError", "{argument}");
            assert_eq!(
                text_of(member(&record, "message")),
                format!("expected string or bytes-like object, got '{name}'")
            );
        }
    }

    #[test]
    fn translate_keeps_integer_and_float_types_and_the_original_key_order() {
        let cfg = r#"{"max_position_embeddings": 1000, "rope_scaling": {"rope_type": "default"}}"#;
        let record = call(
            "rope_scaling_translate",
            "translate",
            &format!("[{cfg}, 4000, 5.0]"),
        );
        let value = value_of(&record);
        let keys: Vec<String> = match value {
            PyValue::Dict(map) => map
                .iter()
                .map(|(key, _)| key.to_utf8().expect("text"))
                .collect(),
            other => panic!("{}", other.repr()),
        };
        assert_eq!(
            keys,
            [
                "target_len",
                "native_max_position_embeddings",
                "native_source",
                "needed",
                "hf_overrides",
                "container_path",
                "factor",
                "unknowns",
                "reasons"
            ]
        );
        assert!(matches!(member(value, "factor"), PyValue::Float(number) if *number == 5.0));
        // A bool target is an integer (True is 1) and a bool native size is an integer too.
        let record = call(
            "rope_scaling_translate",
            "translate",
            &format!("[{cfg}, true]"),
        );
        assert!(matches!(
            member(value_of(&record), "needed"),
            PyValue::Bool(false)
        ));
        let record = call(
            "rope_scaling_translate",
            "translate",
            &format!("[{cfg}, \"4000\"]"),
        );
        // `int("4000")` is 4000: the target is converted, not compared as it came.
        assert!(
            matches!(member(value_of(&record), "target_len"), PyValue::Int(number) if *number == 4000.into())
        );
        for (argument, class, message) in [
            ("\"abc\"", "ValueError", "invalid literal for int() with base 10: 'abc'"),
            ("null", "TypeError", "int() argument must be a string, a bytes-like object or a real number, not 'NoneType'"),
        ] {
            let record = call("rope_scaling_translate", "translate", &format!("[{cfg}, {argument}]"));
            assert_eq!(text_of(member(&record, "kind")), "exception");
            assert_eq!(text_of(member(&record, "type")), class);
            assert_eq!(text_of(member(&record, "message")), message);
        }
    }

    fn production_run(program: &str, args: &[&str], close: &[u32]) -> std::process::Output {
        let mut script = String::from("exec \"$@\"");
        for descriptor in close {
            script.push_str(&format!(" {descriptor}>&-"));
        }
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", &script, "sh", program])
            .args(args)
            .env_clear();
        run_with_input(command, b"")
    }

    #[test]
    fn a_closed_standard_error_sends_the_guard_message_to_standard_output_and_the_parser_message_nowhere(
    ) {
        let rope = env!("CARGO_BIN_EXE_rope_scaling_translate");
        let open = production_run(rope, &[], &[]);
        assert_eq!(open.status.code(), Some(2));
        assert!(open.stdout.is_empty());
        let message = String::from_utf8(open.stderr.clone()).expect("text");
        assert!(message.starts_with("[rope-translate] FAIL"));
        let closed = production_run(rope, &[], &[2]);
        assert_eq!(closed.status.code(), Some(2));
        assert_eq!(String::from_utf8(closed.stdout).expect("text"), message);
        // `argparse`'s error with no standard error: the usage reaches standard output, the message is dropped.
        let closed = production_run(rope, &["--bogus"], &[2]);
        assert_eq!(closed.status.code(), Some(2));
        let text = String::from_utf8(closed.stdout).expect("text");
        assert!(
            text.starts_with("usage: rope_scaling_translate.py"),
            "{text}"
        );
        assert!(!text.contains("unrecognized arguments"));
    }

    #[test]
    fn a_closed_standard_output_loses_the_result_and_the_status_is_the_writers() {
        let log = env!("CARGO_BIN_EXE_parse_vllm_log");
        let open = production_run(log, &[], &[]);
        assert_eq!(open.status.code(), Some(0));
        assert!(String::from_utf8(open.stdout)
            .expect("text")
            .contains("\"weights_gib\": null"));
        let closed = production_run(log, &[], &[1]);
        assert!(closed.stdout.is_empty());
    }
}

// ── the benchmark program ────────────────────────────────────────────────────────────────────────

mod bench_cases {
    use super::*;

    fn rows(bundle: &PyValue, picks: &[usize], mutate: bool) -> String {
        let mut rows = Vec::new();
        for pick in picks {
            let case = &items(member(bundle, "api"))[*pick];
            let request = member(case, "request");
            let mut expected = member(member(case, "original"), "value").clone();
            if mutate {
                if let PyValue::Dict(map) = &mut expected {
                    map.set("needed", PyValue::Bool(false));
                    map.set("weights_gib", PyValue::str("changed"));
                }
            }
            let mut row = PyDict::new();
            row.set("metric", PyValue::str(&format!("case_{pick}")));
            row.set(
                "operation",
                PyValue::str(&format!(
                    "{}.{}",
                    text_of(member(request, "owner")),
                    text_of(member(request, "function"))
                )),
            );
            row.set("args", member(request, "args").clone());
            row.set("kwargs", member(request, "kwargs").clone());
            row.set("expected", expected);
            rows.push(PyValue::Dict(row));
        }
        PyValue::List(rows).dumps_ascii().expect("json")
    }

    fn run_bench(fixture: &Path, extra: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_recipe_transform_bench"))
            .arg("--fixture")
            .arg(fixture)
            .args(extra)
            .env_clear()
            .stdin(Stdio::null())
            .output()
            .expect("run the benchmark")
    }

    #[test]
    fn the_benchmark_checks_whole_values_first_and_reports_one_line_per_function() {
        let bundle = load("bundle.json");
        let area = PrivateDirectory::create("transform-bench-").expect("a private directory");
        let good = area.path().join("good.json");
        std::fs::write(&good, rows(&bundle, &[2, 95], false)).expect("a fixture");
        let output = run_bench(&good, &["--rounds", "2", "--iterations", "3"]);
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
        for (line, metric) in lines[1..].iter().zip(["case_2", "case_95"]) {
            let report = loads_text(&PyStr::new(line)).expect("a row");
            assert_eq!(text_of(member(&report, "metric")), metric);
            assert_eq!(items(member(&report, "rounds")).len(), 2);
        }
        // A differing whole value (or one of another type) ends the run before any timer: no report at all.
        let wrong = area.path().join("wrong.json");
        std::fs::write(&wrong, rows(&bundle, &[2, 95], true)).expect("a fixture");
        let output = run_bench(&wrong, &["--rounds", "2", "--iterations", "3"]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr)
            .contains("differs from the saved original result"));
        // A failing call is an error, not a sample; an unknown operation or a bad option stops the run.
        let failing = area.path().join("failing.json");
        let failing_row = r#"[{"metric": "bad", "operation": "parse_vllm_log.parse_log", "args": [1], "kwargs": {}, "expected": {}}]"#;
        std::fs::write(&failing, failing_row).expect("a fixture");
        let output = run_bench(&failing, &[]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let unknown = area.path().join("unknown.json");
        std::fs::write(
            &unknown,
            failing_row.replace("parse_vllm_log.parse_log", "parse_vllm_log.nothing"),
        )
        .expect("a fixture");
        assert_eq!(run_bench(&unknown, &[]).status.code(), Some(1));
        assert_eq!(run_bench(&good, &["--rounds", "0"]).status.code(), Some(2));
    }
}

// ── the review delta: a literal NaN in the globals, and a standard input that was closed at start ─

mod delta_cases {
    use super::*;

    fn saved(relative: &str) -> Vec<u8> {
        std::fs::read(data(&format!("review_delta/{relative}"))).expect("saved delta data")
    }

    fn json_line(request: &PyValue) -> String {
        request.dumps_ascii().expect("a request")
    }

    /// One production `evaluate-jsonl` run (`--input-fd 3`, standard input a write-only file) over request lines.
    fn serve(lines: &[String]) -> Vec<PyValue> {
        let area =
            PrivateDirectory::create("transform-delta-service-").expect("a private directory");
        let zero = area.path().join("zero");
        let (mut command, mut writer) = over_descriptor(
            env!("CARGO_BIN_EXE_recipe_transform_api"),
            &["evaluate-jsonl", "--input-fd", "3"],
            &zero,
        );
        writer
            .write_all(format!("{}\n", lines.join("\n")).as_bytes())
            .expect("the protocol bytes");
        drop(writer);
        let output = command.output().expect("run the service");
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        String::from_utf8(output.stdout)
            .expect("text")
            .lines()
            .map(|line| loads_text(&PyStr::new(line)).expect("a record"))
            .collect()
    }

    /// The reviewer's saved case: `EXTENDABLE_ROPE_TYPES = [NaN]` (a JSON scalar float), a `default` rope type that is not in it.
    /// The original answered with the ordinary value record (the not-extendable refusal); the first product refused the request.
    fn n1_request(spec: &PyValue, globals: &PyValue) -> PyValue {
        let mut line = PyDict::new();
        line.set("id", member(spec, "id").clone());
        line.set(
            "operation",
            PyValue::str("rope_scaling_translate.translate"),
        );
        line.set("args", member(spec, "args").clone());
        line.set("kwargs", member(spec, "kwargs").clone());
        line.set("globals", globals.clone());
        PyValue::Dict(line)
    }

    #[test]
    fn a_literal_nan_in_the_globals_is_a_json_scalar_and_the_service_answers_as_the_original_did() {
        let spec =
            loads_text(&PyStr::from_surrogateescape(&saved("n1-case.json"))).expect("the case");
        let original = loads_text(&PyStr::from_surrogateescape(&saved(
            "n1-original-record.json",
        )))
        .expect("the record");
        let request = n1_request(&spec, member(&spec, "context"));
        let text = json_line(&request);
        assert!(text.contains("NaN"), "{text}");
        let got = serve(&[text]);
        let want = without(
            &original,
            &["owner", "trace", "globals_before", "globals_after"],
        );
        assert!(
            same_value(&want, &got[0]),
            "want {}\n  got {}",
            want.repr(),
            got[0].repr()
        );
        assert_eq!(text_of(member(&got[0], "kind")), "value");
        assert!(matches!(
            member(member(&got[0], "value"), "hf_overrides"),
            PyValue::None
        ));
    }

    #[test]
    fn the_whole_saved_record_is_reproduced_through_the_fixture_with_the_nan_globals_before_and_after(
    ) {
        let area =
            PrivateDirectory::create("transform-delta-fixture-").expect("a private directory");
        let root = area.path().join("case");
        std::fs::create_dir(&root).expect("a root");
        let (spec, control) = (
            area.path().join("spec.json"),
            area.path().join("control.json"),
        );
        std::fs::write(&spec, saved("n1-case.json")).expect("a spec");
        let mut command = Command::new(env!("CARGO_BIN_EXE_recipe_transform_fixture_api"));
        command
            .args(["case", "--spec"])
            .arg(&spec)
            .arg("--root")
            .arg(&root)
            .arg("--control")
            .arg(&control)
            .env_clear();
        let output = run_with_input(command, b"");
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let original = loads_text(&PyStr::from_surrogateescape(&saved(
            "n1-original-record.json",
        )))
        .expect("the record");
        let got = read_control(&control);
        assert!(
            same_value(&original, &got),
            "want {}\n  got {}",
            original.repr(),
            got.repr()
        );
    }

    /// Source-derived (not an original capture): one request line decodes every literal `NaN` to the one `NaN` object the JSON decoder
    /// has, as Python's `json` does, so a rope type NaN is found by identity in an extendable tuple that holds the NaN of the same
    /// document (`x in tuple` is identity or equality) and the extension proceeds. No other alias or identity is claimed.
    #[test]
    fn a_nan_rope_type_is_found_in_the_extendable_types_of_the_same_request_by_the_decoders_one_nan_object(
    ) {
        let line = r#"{"id": "n1-identity", "operation": "rope_scaling_translate.translate", "args": [{"max_position_embeddings": 8, "rope_scaling": {"rope_type": NaN}}, 16], "globals": {"EXTENDABLE_ROPE_TYPES": [NaN]}}"#;
        let got = serve(&[line.to_string()]);
        assert_eq!(
            text_of(member(&got[0], "kind")),
            "value",
            "{}",
            got[0].repr()
        );
        let overrides = member(member(&got[0], "value"), "hf_overrides");
        let scaling = member(overrides, "rope_scaling");
        assert_eq!(text_of(member(scaling, "rope_type")), "yarn");
        assert!(matches!(member(scaling, "factor"), PyValue::Int(number) if *number == 2.into()));
    }

    /// Runs the program with descriptor 0 closed before it starts (Python's `sys.stdin` is then `None`).
    fn run_with_closed_input(
        program: &str,
        args: &[&std::ffi::OsStr],
        directory: &Path,
    ) -> std::process::Output {
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", "exec \"$@\" 0<&-", "sh", program])
            .args(args)
            .env_clear()
            .current_dir(directory)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command.output().expect("run the program")
    }

    #[test]
    fn a_closed_standard_input_is_the_attribute_error_of_none_read_before_anything_is_read() {
        let area = PrivateDirectory::create("transform-delta-stdin-").expect("a private directory");
        let output = run_with_closed_input(env!("CARGO_BIN_EXE_parse_vllm_log"), &[], area.path());
        // The reviewer's saved original run: status 1, nothing on standard output, the report ends with that exception.
        let execution = loads_text(&PyStr::from_surrogateescape(&saved(
            "n2-original-execution.json",
        )))
        .expect("the execution");
        assert!(
            matches!(member(&execution, "exit_code"), PyValue::Int(number) if *number == 1.into())
        );
        assert!(matches!(
            member(&execution, "fd0_closed"),
            PyValue::Bool(true)
        ));
        let cwd = text_of(member(&execution, "cwd"));
        let diagnostic = replace_bytes(&saved("n2-original-stderr.bin"), &cwd, b"@CASE@");
        let want = project_traceback(&diagnostic).expect("a known shape");
        assert_eq!(
            want,
            b"AttributeError: 'NoneType' object has no attribute 'read'\n".to_vec()
        );
        assert_eq!(output.status.code(), Some(1));
        assert!(
            output.stdout.is_empty(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert_eq!(output.stderr, want);
        // The file branch never reads standard input: the saved `--log FILE` case gives its saved output with descriptor 0 closed.
        let bundle = load("bundle.json");
        let case = items(member(&bundle, "cli"))
            .iter()
            .find(|case| text_of(member(member(case, "request"), "id")) == "cli-033")
            .expect("the saved log-file case");
        let work = new_case(&bundle, "cli-033-closed", member(case, "before"));
        let root = root_text(&work);
        let args: Vec<std::ffi::OsString> = items(member(member(case, "request"), "args"))
            .iter()
            .map(|arg| std::ffi::OsString::from(text_of(&rebase(arg, &root))))
            .collect();
        let arguments: Vec<&std::ffi::OsStr> =
            args.iter().map(std::ffi::OsString::as_os_str).collect();
        let output =
            run_with_closed_input(env!("CARGO_BIN_EXE_parse_vllm_log"), &arguments, &work.root);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        assert_eq!(
            output.stdout,
            replace_bytes(
                &hex_decode(&text_of(member(case, "stdout_hex"))),
                "@CASE@",
                root.as_bytes()
            )
        );
    }

    #[test]
    fn the_fixture_records_the_closed_input_as_a_read_then_its_error_in_the_trace() {
        let area = PrivateDirectory::create("transform-delta-trace-").expect("a private directory");
        let root = area.path().join("case");
        std::fs::create_dir(&root).expect("a root");
        let (spec, control) = (
            area.path().join("spec.json"),
            area.path().join("control.json"),
        );
        std::fs::write(
            &spec,
            r#"{"id": "n2", "owner": "parse_vllm_log", "args": [], "kwargs": {}, "context": {}}"#,
        )
        .expect("a spec");
        let arguments = [
            std::ffi::OsString::from("--spec"),
            spec.clone().into_os_string(),
            std::ffi::OsString::from("--root"),
            root.clone().into_os_string(),
            std::ffi::OsString::from("--control"),
            control.clone().into_os_string(),
        ];
        let arguments: Vec<&std::ffi::OsStr> = arguments
            .iter()
            .map(std::ffi::OsString::as_os_str)
            .collect();
        let output = run_with_closed_input(
            env!("CARGO_BIN_EXE_recipe_transform_fixture"),
            &arguments,
            &root,
        );
        assert_eq!(
            output.status.code(),
            Some(1),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty());
        let record = read_control(&control);
        assert_eq!(text_of(member(&record, "kind")), "exception");
        assert_eq!(text_of(member(&record, "type")), "AttributeError");
        assert_eq!(
            text_of(member(&record, "message")),
            "'NoneType' object has no attribute 'read'"
        );
        let trace = items(member(&record, "trace"));
        assert_eq!(trace.len(), 2);
        assert_eq!(text_of(member(&trace[0], "event")), "stdin_read");
        assert_eq!(text_of(member(&trace[1], "event")), "stdin_read_error");
        assert_eq!(text_of(member(&trace[1], "type")), "AttributeError");
    }
}
