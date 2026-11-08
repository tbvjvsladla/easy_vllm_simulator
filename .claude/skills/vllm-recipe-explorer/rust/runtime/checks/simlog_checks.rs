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
        .join("simlog_reference")
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
            // By code point: a saved text may hold a lone surrogate.
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
    let area = PrivateDirectory::create(&format!("simlog-{name}-")).expect("a private directory");
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

/// Starts `program args...` the way a caller passes a protocol descriptor (Python's `pass_fds`): the pipe the caller writes to is
/// descriptor 3 of the program, and descriptor 0 is a file that can only be written to (so any read of standard input fails loudly);
/// the umask is 022 and the working directory `directory`. Returns the command (streams piped) and the write end of the protocol pipe.
fn over_descriptor(
    program: &str,
    args: &[&str],
    zero: &Path,
    directory: &Path,
) -> (Command, std::io::PipeWriter) {
    let (reader, writer) = std::io::pipe().expect("a pipe");
    let mut command = Command::new("/bin/sh");
    command
        .args(["-c", "umask 022; exec \"$@\" 3<&0 0>\"$F0\"", "sh", program])
        .args(args)
        .env_clear()
        .env("F0", zero)
        .current_dir(directory)
        .stdin(Stdio::from(reader))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    (command, writer)
}

// ── commands ─────────────────────────────────────────────────────────────────────────────────────

/// `/bin/sh -c 'umask 022; exec "$@"'`: the child-local umask the collector set before the public call, the working directory the root.
fn umask_command(program: &str, directory: &Path) -> Command {
    let mut command = Command::new("/bin/sh");
    command
        .args(["-c", "umask 022; exec \"$@\"", "sh", program])
        .env_clear()
        .env("TMPDIR", directory.join("private-tmp"))
        .current_dir(directory)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

fn status_text(output: &std::process::Output) -> Option<String> {
    output.status.code().map(|code| code.to_string())
}

// ── reference cases ──────────────────────────────────────────────────────────────────────────────

mod reference_cases {
    use super::*;

    fn fixture_problems(bundle: &PyValue, case: &PyValue) -> Vec<String> {
        let request = member(case, "request");
        let id = text_of(member(request, "id"));
        let work = new_case(bundle, &id, member(case, "before"));
        let (spec, control) = (work.area.join("spec.json"), work.area.join("control.json"));
        write_json(&spec, request);
        let mut command =
            umask_command(env!("CARGO_BIN_EXE_recipe_simlog_fixture_api"), &work.root);
        command
            .arg("case")
            .arg("--spec")
            .arg(&spec)
            .arg("--root")
            .arg(&work.root)
            .arg("--control")
            .arg(&control);
        let output = command.output().expect("run the fixture API");
        let mut problems = Vec::new();
        if status_text(&output).as_deref() != Some("0") {
            problems.push(format!(
                "status {:?}: {}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr)
            ));
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
    fn the_140_saved_calls_are_reproduced_whole_with_values_types_exceptions_constructor_args_trace_globals_and_files(
    ) {
        let bundle = load("bundle.json");
        let cases = items(member(&bundle, "api"));
        assert_eq!(cases.len(), 140);
        let failures: Vec<String> = cases
            .iter()
            .flat_map(|case| fixture_problems(&bundle, case))
            .collect();
        assert!(
            failures.is_empty(),
            "{} problems:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    /// One production `evaluate-jsonl` run (descriptor 3, umask 022, working directory the case root) over one saved call.
    fn service_problems(bundle: &PyValue, case: &PyValue) -> Vec<String> {
        let request = member(case, "request");
        let id = text_of(member(request, "id"));
        let work = new_case(bundle, &format!("service-{id}"), member(case, "before"));
        let root = root_text(&work);
        let mut line = PyDict::new();
        line.set("id", member(request, "id").clone());
        line.set(
            "operation",
            PyValue::str(&format!(
                "simlog_writer.{}",
                text_of(member(request, "function"))
            )),
        );
        line.set("args", rebase(member(request, "args"), &root));
        line.set("kwargs", rebase(member(request, "kwargs"), &root));
        if matches!(member(request, "context"), PyValue::Dict(context) if !context.is_empty()) {
            line.set("globals", member(request, "context").clone());
        }
        let zero = work.area.join("zero");
        let (mut command, mut writer) = over_descriptor(
            env!("CARGO_BIN_EXE_recipe_simlog_api"),
            &["evaluate-jsonl", "--input-fd", "3"],
            &zero,
            &work.root,
        );
        writer
            .write_all(
                format!(
                    "{}\n",
                    PyValue::Dict(line).dumps_ascii().expect("a request")
                )
                .as_bytes(),
            )
            .expect("the request");
        drop(writer);
        let output = command.output().expect("run the service");
        let mut problems = Vec::new();
        if status_text(&output).as_deref() != Some("0") || !output.stderr.is_empty() {
            problems.push(format!(
                "service ended {:?}: {}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        let text = String::from_utf8_lossy(&output.stdout).into_owned();
        match loads_text(&PyStr::new(text.trim_end())) {
            Ok(got) => {
                let want = rebase(
                    &without(
                        member(case, "original"),
                        &["owner", "trace", "globals_before", "globals_after"],
                    ),
                    &root,
                );
                if !same_value(&want, &got) {
                    problems.push(format!(
                        "record differs:\n  want {}\n  got  {}",
                        want.repr(),
                        got.repr()
                    ));
                }
            }
            Err(_) => problems.push(format!("no record: {text}")),
        }
        let after = expected_tree(bundle, member(case, "after"));
        problems.extend(tree_difference(&after, &snapshot(&work.root)));
        problems
            .into_iter()
            .map(|problem| format!("{id}: {problem}"))
            .collect()
    }

    #[test]
    fn the_140_saved_calls_agree_through_the_production_service_with_their_files() {
        let bundle = load("bundle.json");
        let cases = items(member(&bundle, "api"));
        let failures: Vec<String> = cases
            .iter()
            .flat_map(|case| service_problems(&bundle, case))
            .collect();
        assert!(
            failures.is_empty(),
            "{} problems:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
}

// ── the service's own contract ───────────────────────────────────────────────────────────────────

fn run_service(directory: &Path, lines: &[String]) -> (std::process::Output, Vec<PyValue>) {
    let zero = directory.join("zero");
    let (mut command, mut writer) = over_descriptor(
        env!("CARGO_BIN_EXE_recipe_simlog_api"),
        &["evaluate-jsonl", "--input-fd", "3"],
        &zero,
        directory,
    );
    writer
        .write_all(format!("{}\n", lines.join("\n")).as_bytes())
        .expect("the protocol bytes");
    drop(writer);
    let output = command.output().expect("run the service");
    let records = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| loads_text(&PyStr::new(line)).expect("a record"))
        .collect();
    (output, records)
}

mod contract_cases {
    use super::*;

    #[test]
    fn the_constants_name_the_profile_and_the_formats_and_the_refusals_are_records() {
        let output = Command::new(env!("CARGO_BIN_EXE_recipe_simlog_api"))
            .arg("constants")
            .env_clear()
            .output()
            .expect("run");
        let got = loads_text(&PyStr::from_surrogateescape(&output.stdout)).expect("constants");
        let module = member(&got, "simlog_writer");
        assert!(matches!(
            member(member(module, "globals"), "yaml_enabled"),
            PyValue::Bool(true)
        ));
        assert_eq!(
            text_of(member(member(module, "globals"), "yaml_version")),
            "6.0.3"
        );
        assert_eq!(
            text_of(member(
                member(member(module, "formats"), "candidate_yaml"),
                "emitter"
            )),
            "PyYAML 6.0.3 SafeDumper"
        );
        let area = PrivateDirectory::create("simlog-refuse-").expect("a private directory");
        let requests = [
            r#"{"id": 1, "operation": "simlog_writer._trial_prefix", "args": [1], "context": {}}"#,
            r#"{"id": 2, "operation": "simlog_writer._trial_prefix", "args": [1], "globals": {"yaml_enabled": 1}}"#,
            r#"{"id": 3, "operation": "simlog_writer._trial_prefix", "args": [1], "globals": {"other": true}}"#,
            r#"{"id": 4, "operation": "simlog_writer.other", "args": []}"#,
            r#"{"id": 5, "operation": "other.write_trial", "args": []}"#,
            r#"{"id": 6, "operation": "simlog_writer._trial_prefix", "args": [], "kwargs": {}}"#,
            r#"{"id": 7, "operation": "simlog_writer._trial_prefix", "args": [7], "globals": {"yaml_enabled": false}}"#,
            r#"{"id": 8, "operation": "simlog_writer._trial_prefix", "args": [-5]}"#,
        ];
        let (output, got) = run_service(
            area.path(),
            &requests
                .iter()
                .map(|text| text.to_string())
                .collect::<Vec<_>>(),
        );
        assert_eq!(output.status.code(), Some(0));
        let kind = |row: &PyValue| {
            (
                text_of(member(row, "type")),
                text_of(member(row, "message")),
            )
        };
        assert!(kind(&got[0])
            .1
            .starts_with("the request has a member this service does not take: "));
        assert_eq!(
            kind(&got[1]),
            (
                "RequestError".to_string(),
                "yaml_enabled must be a bool".to_string()
            )
        );
        assert_eq!(kind(&got[2]).0, "RequestError");
        assert_eq!(
            kind(&got[3]),
            (
                "AttributeError".to_string(),
                "module 'simlog_writer' has no attribute 'other'".to_string()
            )
        );
        assert_eq!(
            kind(&got[4]),
            (
                "ModuleNotFoundError".to_string(),
                "No module named 'other'".to_string()
            )
        );
        assert_eq!(
            kind(&got[5]),
            (
                "TypeError".to_string(),
                "_trial_prefix() missing 1 required positional argument: 'trial_number'"
                    .to_string()
            )
        );
        assert_eq!(text_of(member(&got[6], "value")), "trial07");
        assert_eq!(text_of(member(&got[7], "value")), "trial-5");
        for arguments in [
            vec!["evaluate-jsonl", "--input-fd", "1"],
            vec!["constants", "x"],
            vec![],
        ] {
            let output = Command::new(env!("CARGO_BIN_EXE_recipe_simlog_api"))
                .args(&arguments)
                .env_clear()
                .stdin(Stdio::null())
                .output()
                .expect("run");
            assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        }
    }

    /// Reading the source: no provider or case lookup, no process spawn, no environment, no operational path, no Python.
    #[test]
    fn the_sources_have_no_selector_no_fixture_module_no_spawn_and_no_operational_path() {
        let sources = [
            ("simlog.rs", include_str!("../src/simlog.rs")),
            ("simlog_api.rs", include_str!("../src/simlog_api.rs")),
            ("simlog_bench.rs", include_str!("../src/simlog_bench.rs")),
            (
                "bin/recipe_simlog_api.rs",
                include_str!("../src/bin/recipe_simlog_api.rs"),
            ),
            (
                "bin/recipe_simlog_bench.rs",
                include_str!("../src/bin/recipe_simlog_bench.rs"),
            ),
        ];
        let mut found = Vec::new();
        for (name, text) in sources {
            for forbidden in [
                "env::var",
                "getenv",
                "simlog_fixture",
                "real-worker",
                "val_agent",
                "CARGO_MANIFEST_DIR",
                "BoundaryViolation",
                "Command::new",
                "TcpStream",
                "/usr/bin/python",
                "python3",
                "expected_files_of",
            ] {
                if text.contains(forbidden)
                    && !(name == "simlog_bench.rs" && forbidden == "expected_files_of")
                {
                    found.push(format!("{name}: {forbidden}"));
                }
            }
        }
        assert!(found.is_empty(), "{}", found.join("\n"));
    }
}

// ── new checks: the emitter, the chunks and the failure order beyond the saved calls ─────────────

mod new_cases {
    use super::*;

    /// One service call in a fresh private directory; the directory and the record.
    fn call(function: &str, args: &str, globals: Option<&str>) -> (PrivateDirectory, PyValue) {
        let area = PrivateDirectory::create("simlog-new-").expect("a private directory");
        let root = area.path().to_str().expect("a path").to_string();
        let args = args.replace("@ROOT@", &root);
        let globals = globals
            .map(|text| format!(", \"globals\": {text}"))
            .unwrap_or_default();
        let line = format!(
            r#"{{"id": "n", "operation": "simlog_writer.{function}", "args": {args}{globals}}}"#
        );
        let (output, mut records) = run_service(area.path(), &[line]);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let record = records.remove(0);
        (area, record)
    }

    fn file_text(area: &PrivateDirectory, relative: &str) -> String {
        String::from_utf8(std::fs::read(area.path().join(relative)).expect("a file"))
            .expect("UTF-8")
    }

    /// The candidate file `write_trial` leaves for a JSON-valued candidate.
    fn candidate(value: &str, yaml_enabled: bool) -> String {
        let args = format!(r#"["@ROOT@/run", 1, "", {{}}, {value}, {{}}]"#);
        let globals = format!("{{\"yaml_enabled\": {yaml_enabled}}}");
        let (area, record) = call("write_trial", &args, Some(&globals));
        assert_eq!(
            text_of(member(&record, "kind")),
            "value",
            "{}",
            record.repr()
        );
        file_text(&area, "run/trial01_candidate.yaml")
    }

    /// Source-derived expectations (PyYAML 6.0.3 `SafeDumper`, `allow_unicode=True`, `sort_keys=False`): not original captures.
    #[test]
    fn the_yaml_emitter_writes_the_documented_block_style_for_collections_scalars_and_quoting() {
        let cases: &[(&str, &str)] = &[
            ("[]", "[]\n"),
            ("{}", "{}\n"),
            ("\"abc\"", "abc\n...\n"),
            ("null", "null\n...\n"),
            ("5", "5\n...\n"),
            ("[[1, 2], [3]]", "- - 1\n  - 2\n- - 3\n"),
            (r#"{"a": [], "b": {}}"#, "a: []\nb: {}\n"),
            (r#"{"a": [1, 2]}"#, "a:\n- 1\n- 2\n"),
            (r#"{"a": {"b": {"c": 1}}}"#, "a:\n  b:\n    c: 1\n"),
            (r#"["yes", "1", "", "a: b", " x", "null", "1.5", "-", "a b"]"#, "- 'yes'\n- '1'\n- ''\n- 'a: b'\n- ' x'\n- 'null'\n- '1.5'\n- '-'\n- a b\n"),
            (r#"["a\nb", "tab\there", "\ud800"]"#, "- 'a\n\n  b'\n- \"tab\\there\"\n- \"\\uD800\"\n"),
            ("[1e17, -0.0, 1.5, 1e-7, NaN, Infinity, -Infinity, true, false, null]", "- 1.0e+17\n- -0.0\n- 1.5\n- 1.0e-07\n- .nan\n- .inf\n- -.inf\n- true\n- false\n- null\n"),
            (r#"["한글", "😀"]"#, "- 한글\n- 😀\n"),
        ];
        for (value, want) in cases {
            assert_eq!(&candidate(value, true), want, "{value}");
        }
        // A key too long for a simple key (the tag counts toward the 128): the complex-key form.
        let key = "k".repeat(130);
        assert_eq!(
            candidate(&format!(r#"{{"{key}": 1}}"#), true),
            format!("? {key}\n: 1\n")
        );
        // Without the YAML module the candidate is the JSON of the other files.
        assert_eq!(
            candidate(r#"{"a": [1, "x"]}"#, false),
            "{\n  \"a\": [\n    1,\n    \"x\"\n  ]\n}"
        );
    }

    /// Source-derived (`json.encoder._make_iterencode` chunking): the chunks before the failing one are what the file holds.
    #[test]
    fn a_failing_json_chunk_leaves_the_earlier_chunks_and_carries_the_chunk_as_the_encode_error_object(
    ) {
        let summary = |value: &str| {
            call(
                "write_summary",
                &format!(r#"["@ROOT@/run", {value}]"#),
                None,
            )
        };
        let object_of = |record: &PyValue| -> (Vec<u32>, i64, i64) {
            let row = &items(member(record, "exception_chain"))[0];
            let args = items(member(row, "constructor_args"));
            let PyValue::Str(object) = &args[1] else {
                panic!("an object text")
            };
            let number = |value: &PyValue| match value {
                PyValue::Int(number) => i64::try_from(number.clone()).expect("a position"),
                other => panic!("{}", other.repr()),
            };
            assert_eq!(text_of(&args[0]), "utf-8");
            assert_eq!(text_of(&args[4]), "surrogates not allowed");
            (
                object.code_points().to_vec(),
                number(&args[2]),
                number(&args[3]),
            )
        };
        // A list item joins its separator: the object is `,\n  "b\ud800"`, the first item stays.
        let (area, record) = summary(r#"["a", "b\ud800"]"#);
        assert_eq!(text_of(member(&record, "type")), "UnicodeEncodeError");
        assert_eq!(file_text(&area, "run/run_summary.json"), "[\n  \"a\"");
        let (object, start, end) = object_of(&record);
        let mut want: Vec<u32> = ",\n  \"b".chars().map(u32::from).collect();
        want.extend([0xD800, u32::from('"')]);
        assert_eq!((object, start, end), (want, 6, 7));
        assert_eq!(
            text_of(member(&record, "message")),
            "'utf-8' codec can't encode character '\\ud800' in position 6: surrogates not allowed"
        );
        // A dict value is a chunk of its own after its key and `: `.
        let (area, record) = summary(r#"{"a": "x", "b": "bad\ud800"}"#);
        assert_eq!(
            file_text(&area, "run/run_summary.json"),
            "{\n  \"a\": \"x\",\n  \"b\": "
        );
        let (object, start, end) = object_of(&record);
        assert_eq!((object.len(), start, end), (6, 4, 5));
        // A run of two surrogates is one position range.
        let (_, record) = summary(r#"["\ud800\ud800"]"#);
        let (_, start, end) = object_of(&record);
        assert_eq!((start, end), (5, 7));
        assert_eq!(
            text_of(member(&record, "message")),
            "'utf-8' codec can't encode characters in position 5-6: surrogates not allowed"
        );
        // A 4300-digit integer is the largest a request can carry (and `json` prints): the file holds all of it.
        let ok = format!("1{}", "0".repeat(4299));
        let (area, record) = summary(&format!("[{ok}]"));
        assert_eq!(text_of(member(&record, "kind")), "value");
        assert!(file_text(&area, "run/run_summary.json").len() > 4300);
    }

    /// A later failure keeps the earlier files: the YAML escapes a lone surrogate (no failure), the smoke JSON then fails after its
    /// own first chunks.
    #[test]
    fn a_later_file_failure_keeps_the_earlier_files_and_the_yaml_escapes_a_lone_surrogate() {
        let args = r#"["@ROOT@/run", 2, "log", {"p": 1}, {"c": "\ud800"}, {"s": "\ud800"}]"#;
        let (area, record) = call("write_trial", args, None);
        assert_eq!(text_of(member(&record, "type")), "UnicodeEncodeError");
        assert_eq!(file_text(&area, "run/trial02_vllm.log"), "log");
        assert_eq!(
            file_text(&area, "run/trial02_profile.json"),
            "{\n  \"p\": 1\n}"
        );
        assert_eq!(
            file_text(&area, "run/trial02_candidate.yaml"),
            "c: \"\\uD800\"\n"
        );
        assert_eq!(file_text(&area, "run/trial02_smoke.json"), "{\n  \"s\": ");
        let trace = member(&record, "exception_chain");
        assert_eq!(items(trace).len(), 1);
    }
}

// ── the benchmark program ────────────────────────────────────────────────────────────────────────

mod bench_cases {
    use super::*;

    fn rows(
        bundle: &PyValue,
        root: &str,
        picks: &[usize],
        mutate_value: bool,
        mutate_file: bool,
    ) -> String {
        let mut rows = Vec::new();
        for pick in picks {
            let case = &items(member(bundle, "api"))[*pick];
            let request = member(case, "request");
            let mut expected = member(member(case, "original"), "value").clone();
            if mutate_value {
                expected = PyValue::str("trial99");
            }
            let mut files = Vec::new();
            let after = expected_tree(bundle, member(case, "after"));
            for (path, (kind, _, bytes)) in &after {
                if kind == "file" && path.starts_with("run/") {
                    let mut bytes = bytes.clone();
                    if mutate_file {
                        bytes.push(b'!');
                    }
                    let mut entry = PyDict::new();
                    entry.set("path", PyValue::str(&format!("{root}/{path}")));
                    entry.set(
                        "bytes_hex",
                        PyValue::str(
                            &bytes
                                .iter()
                                .map(|byte| format!("{byte:02x}"))
                                .collect::<String>(),
                        ),
                    );
                    files.push(PyValue::Dict(entry));
                }
            }
            let mut row = PyDict::new();
            row.set("metric", PyValue::str(&format!("case_{pick}")));
            row.set(
                "operation",
                PyValue::str(&format!(
                    "simlog_writer.{}",
                    text_of(member(request, "function"))
                )),
            );
            row.set("args", rebase(member(request, "args"), root));
            row.set("kwargs", rebase(member(request, "kwargs"), root));
            row.set("globals", member(request, "context").clone());
            row.set("expected", rebase(&expected, root));
            row.set("expected_files", PyValue::List(files));
            rows.push(PyValue::Dict(row));
        }
        PyValue::List(rows).dumps_ascii().expect("json")
    }

    fn run_bench(directory: &Path, fixture: &Path, extra: &[&str]) -> std::process::Output {
        let mut command = umask_command(env!("CARGO_BIN_EXE_recipe_simlog_bench"), directory);
        command.arg("--fixture").arg(fixture).args(extra);
        command.output().expect("run the benchmark")
    }

    #[test]
    fn the_benchmark_calls_the_real_writer_once_to_check_value_and_files_then_measures_whole_calls()
    {
        let bundle = load("bundle.json");
        let cases = items(member(&bundle, "api"));
        // api-052 (the YAML four-file workload) and api-053 (the JSON one) write the same run directory: each is checked right after its own preflight call.
        let ids: Vec<usize> = [52usize, 53].to_vec();
        assert_eq!(
            text_of(member(member(&cases[52], "request"), "id")),
            "api-052"
        );
        assert_eq!(
            text_of(member(member(&cases[53], "request"), "id")),
            "api-053"
        );
        let work = new_case(&bundle, "bench", member(&cases[52], "before"));
        let root = root_text(&work);
        let fixture = work.area.join("fixture.json");
        std::fs::write(&fixture, rows(&bundle, &root, &ids, false, false)).expect("a fixture");
        let output = run_bench(
            &work.root,
            &fixture,
            &["--rounds", "2", "--iterations", "3"],
        );
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
        for (line, metric) in lines[1..].iter().zip(["case_52", "case_53"]) {
            let report = loads_text(&PyStr::new(line)).expect("a row");
            assert_eq!(text_of(member(&report, "metric")), metric);
            assert_eq!(items(member(&report, "rounds")).len(), 2);
        }
        // The last row wrote the JSON candidate: the files are the real ones of the final preflight and loops.
        assert!(work.root.join("run/trial01_vllm.log").exists());
        // A wrong value or a wrong file ends the run before any timer: no report.
        for (name, wrong_value, wrong_file) in [("value", true, false), ("file", false, true)] {
            let work = new_case(
                &bundle,
                &format!("bench-{name}"),
                member(&cases[52], "before"),
            );
            let root = root_text(&work);
            let path = work.area.join("wrong.json");
            std::fs::write(&path, rows(&bundle, &root, &[52], wrong_value, wrong_file))
                .expect("a fixture");
            let output = run_bench(&work.root, &path, &["--rounds", "2", "--iterations", "3"]);
            assert_eq!(output.status.code(), Some(1), "{name}");
            assert!(output.stdout.is_empty(), "{name}");
            let message = String::from_utf8_lossy(&output.stderr).into_owned();
            assert!(
                message.contains("differs from the saved original"),
                "{name}: {message}"
            );
        }
        // A failing call is an error, not a sample; a bad option or operation stops the run.
        let failing = work.area.join("failing.json");
        std::fs::write(
            &failing,
            r#"[{"metric": "bad", "operation": "simlog_writer._trial_prefix", "args": ["x"], "kwargs": {}, "expected": "trial00", "expected_files": []}]"#,
        )
        .expect("a fixture");
        let output = run_bench(&work.root, &failing, &[]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let unknown = work.area.join("unknown.json");
        std::fs::write(&unknown, r#"[{"metric": "bad", "operation": "simlog_writer.nothing", "args": [], "kwargs": {}, "expected": null, "expected_files": []}]"#).expect("a fixture");
        assert_eq!(run_bench(&work.root, &unknown, &[]).status.code(), Some(1));
        assert_eq!(
            run_bench(&work.root, &fixture, &["--rounds", "0"])
                .status
                .code(),
            Some(2)
        );
    }
}

// ── the common helpers the owner reaches: the streamed JSON chunks and the YAML text ─────────────

mod helper_cases {
    use super::*;
    use agent_runtime_support::pyvalue::StreamError;
    use agent_runtime_support::pyyaml::safe_dump_text;

    fn value(text: &str) -> PyValue {
        loads_text(&PyStr::new(text)).expect("JSON")
    }

    fn chunks(value: &PyValue) -> Vec<String> {
        let mut seen = Vec::new();
        let result = value.json_dump_chunks::<()>(2, &mut |chunk: &PyStr| {
            seen.push(chunk.to_utf8().expect("text"));
            Ok(())
        });
        assert!(result.is_ok());
        seen
    }

    /// Source-derived (`json.encoder._make_iterencode`, indent 2): the list item joins its separator, a container item is preceded by
    /// the separator as a chunk of its own, a dict yields its brace, newline, key, `: ` and value separately, an empty container is one chunk.
    #[test]
    fn the_streamed_json_chunks_are_the_pure_python_encoders_chunks() {
        assert_eq!(
            chunks(&value(r#"{"a": [1, {"b": null}], "c": []}"#)),
            [
                "{", "\n  ", "\"a\"", ": ", "[\n    1", ",\n    ", "{", "\n      ", "\"b\"", ": ",
                "null", "\n    ", "}", "\n  ", "]", ",\n  ", "\"c\"", ": ", "[]", "\n", "}"
            ]
        );
        assert_eq!(chunks(&value("[]")), ["[]"]);
        assert_eq!(chunks(&value("{}")), ["{}"]);
        assert_eq!(chunks(&value(r#""x\u0001""#)), ["\"x\\u0001\""]);
        assert_eq!(
            chunks(&value("[[], \"s\", true, 1.5, null]")),
            [
                "[\n  ",
                "[]",
                ",\n  \"s\"",
                ",\n  true",
                ",\n  1.5",
                ",\n  null",
                "\n",
                "]"
            ]
        );
        // Joined, the chunks are `json.dumps(..., indent=2)`.
        let sample = value(r#"{"k": [1, 2, {"z": "한글"}], "e": {}, "n": [[]]}"#);
        assert_eq!(
            chunks(&sample).concat(),
            sample
                .dumps_pretty()
                .expect("dump")
                .to_utf8()
                .expect("text")
        );
    }

    #[test]
    fn a_failing_sink_ends_the_stream_at_that_chunk_and_the_chunks_before_it_were_handed_over() {
        let mut taken = 0;
        let result = value(r#"["a", "b", "c"]"#).json_dump_chunks(2, &mut |_: &PyStr| {
            taken += 1;
            if taken == 3 {
                Err("stop")
            } else {
                Ok(())
            }
        });
        assert!(matches!(result, Err(StreamError::Sink("stop"))));
        assert_eq!(taken, 3);
        // Nesting past the encoder's depth is refused, after the chunks before it.
        let mut deep = PyValue::List(Vec::new());
        for _ in 0..1000 {
            deep = PyValue::List(vec![deep]);
        }
        let mut count = 0;
        let result = deep.json_dump_chunks::<()>(2, &mut |_: &PyStr| {
            count += 1;
            Ok(())
        });
        assert!(matches!(result, Err(StreamError::Dump(_))));
        assert!(count > 900);
    }

    fn yaml(text: &str) -> String {
        safe_dump_text(&value(text))
            .expect("dump")
            .to_utf8()
            .expect("text")
    }

    /// Source-derived (PyYAML 6.0.3 `SafeDumper`): block collections, empty collections in flow style, the quoted forms.
    #[test]
    fn the_yaml_text_follows_the_emitters_block_layout() {
        assert_eq!(
            yaml(r#"{"a": [{"b": 1, "c": 2}, {"d": 3}]}"#),
            "a:\n- b: 1\n  c: 2\n- d: 3\n"
        );
        assert_eq!(yaml("[[]]"), "- []\n");
        assert_eq!(
            yaml(r#"{"a": {"b": [1, [2, 3]]}}"#),
            "a:\n  b:\n  - 1\n  - - 2\n    - 3\n"
        );
        assert_eq!(
            yaml(r##"["a'b", "it's: x", "#c", "a #b", "- x", "?", "x:"]"##),
            "- a'b\n- 'it''s: x'\n- '#c'\n- 'a #b'\n- '- x'\n- '?'\n- 'x:'\n"
        );
        assert_eq!(yaml(r#"[true, "true", "True", "off", "0o7", "1_000", "2001-01-01", "=", "<<"]"#), "- true\n- 'true'\n- 'True'\n- 'off'\n- 0o7\n- '1_000'\n- '2001-01-01'\n- '='\n- '<<'\n");
        let tuple = PyValue::Tuple(vec![PyValue::int(1), PyValue::str("x")]);
        assert_eq!(
            safe_dump_text(&tuple)
                .expect("dump")
                .to_utf8()
                .expect("text"),
            "- 1\n- x\n"
        );
        let error =
            safe_dump_text(&PyValue::Bytes(vec![1])).expect_err("bytes are outside the profile");
        assert_eq!(error.class, "RepresenterError");
    }
}

// ── the text file's finite buffering and the close chain ─────────────────────────────────────────

mod buffering_cases {
    use super::*;
    use recipe_runtime::simlog::{
        run_in_block, write_summary, write_trial, Done, FsRaw, Host, Plain, Raw, TextFile,
    };
    use recipe_runtime::simlog_api::chain_of;
    use std::cell::RefCell;
    use std::rc::Rc;

    /// The plan of a controlled raw file, as the saved primitive describes it: raw writes (by attempt number) that fail or accept
    /// only a few bytes, every write failing, the close failing.
    #[derive(Clone, Default)]
    struct Plan {
        fail_at: Vec<usize>,
        partial: Vec<(usize, usize)>,
        always: bool,
        close_fails: bool,
    }

    impl Plan {
        fn from_saved(plan: &PyValue) -> Plan {
            let PyValue::Dict(map) = plan else {
                panic!("a plan")
            };
            let mut parsed = Plan::default();
            for (key, action) in map.iter() {
                let key = key.to_utf8().expect("text");
                match (key.as_str(), action) {
                    ("always", PyValue::Str(_)) => parsed.always = true,
                    ("close", PyValue::Str(_)) => parsed.close_fails = true,
                    (index, PyValue::Str(_)) => {
                        parsed.fail_at.push(index.parse().expect("an index"))
                    }
                    (index, PyValue::Int(size)) => parsed.partial.push((
                        index.parse().expect("an index"),
                        usize::try_from(size.clone()).expect("a size"),
                    )),
                    other => panic!("a plan entry {other:?}"),
                }
            }
            parsed
        }
    }

    #[derive(Default)]
    struct Log {
        calls: Vec<usize>,
        attempts: usize,
        close_calls: usize,
    }

    /// The saved primitive's raw object: it records the size of every accepted write and follows the plan.
    struct Fake {
        log: Rc<RefCell<Log>>,
        plan: Plan,
        size: usize,
    }

    fn eio() -> std::io::Error {
        std::io::Error::from_raw_os_error(5)
    }

    impl Raw for Fake {
        fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
            let mut log = self.log.borrow_mut();
            let index = log.attempts;
            log.attempts += 1;
            if self.plan.always || self.plan.fail_at.contains(&index) {
                return Err(eio());
            }
            let size = self
                .plan
                .partial
                .iter()
                .find(|(at, _)| *at == index)
                .map_or(data.len(), |(_, size)| data.len().min(*size));
            log.calls.push(size);
            Ok(size)
        }

        fn close(&mut self) -> std::io::Result<()> {
            self.log.borrow_mut().close_calls += 1;
            if self.plan.close_fails {
                return Err(std::io::Error::other("close failed"));
            }
            Ok(())
        }

        fn buffer_size(&self) -> usize {
            self.size
        }
    }

    fn int(number: usize) -> PyValue {
        PyValue::int(number)
    }

    fn text_of_size(chars: usize, wide: bool) -> PyStr {
        PyStr::new(&(if wide { "\u{e9}" } else { "x" }).repeat(chars))
    }

    fn saved_list(name: &str) -> Vec<PyValue> {
        items(&load(name)).to_vec()
    }

    fn calls_value(calls: &[usize]) -> PyValue {
        PyValue::List(calls.iter().map(|&size| int(size)).collect())
    }

    /// The saved primitive 1: a controlled `io.TextIOWrapper` over `io.BufferedWriter` over a recording raw object. The raw writes the model
    /// makes after every chunk, before the close and after it are the saved ones for every sequence.
    #[test]
    fn the_modelled_buffers_make_the_raw_writes_the_stdlib_primitive_recorded() {
        let saved = load("io/primitive_1_sequences.json");
        assert_eq!(text_of(member(&saved, "module")), "_io");
        let scenarios = items(member(&saved, "scenarios"));
        assert_eq!(scenarios.len(), 21);
        for scenario in scenarios {
            let name = text_of(member(scenario, "name"));
            let size = match member(scenario, "buffer_size") {
                PyValue::Int(number) => usize::try_from(number.clone()).expect("a size"),
                other => panic!("{}", other.repr()),
            };
            let chars: Vec<usize> = items(member(scenario, "chunk_chars"))
                .iter()
                .map(|value| match value {
                    PyValue::Int(number) => usize::try_from(number.clone()).expect("a count"),
                    other => panic!("{}", other.repr()),
                })
                .collect();
            let bytes: Vec<usize> = items(member(scenario, "chunk_bytes"))
                .iter()
                .map(|value| match value {
                    PyValue::Int(number) => usize::try_from(number.clone()).expect("a count"),
                    other => panic!("{}", other.repr()),
                })
                .collect();
            let flush_after: Vec<usize> = items(member(scenario, "flush_after"))
                .iter()
                .map(|value| match value {
                    PyValue::Int(number) => usize::try_from(number.clone()).expect("an index"),
                    other => panic!("{}", other.repr()),
                })
                .collect();
            let log = Rc::new(RefCell::new(Log::default()));
            let file = TextFile::new(
                PyStr::new("fake"),
                Box::new(Fake {
                    log: log.clone(),
                    plan: Plan::default(),
                    size,
                }),
            );
            let mut after_each = Vec::new();
            let mut before_close = Vec::new();
            run_in_block(&mut Plain, file, |host, file| {
                for (index, count) in chars.iter().enumerate() {
                    let wide = bytes[index] == 2 * count;
                    file.write_text(host, &text_of_size(*count, wide))
                        .expect("a write");
                    if flush_after.contains(&index) {
                        file.flush().expect("a flush");
                    }
                    after_each.push(calls_value(&log.borrow().calls));
                }
                before_close = log.borrow().calls.clone();
                Ok(())
            })
            .expect("the block");
            let got_after_close = calls_value(&log.borrow().calls);
            assert!(
                same_value(
                    member(scenario, "raw_after_each_chunk"),
                    &PyValue::List(after_each)
                ),
                "{name} {size}: after each chunk"
            );
            assert!(
                same_value(
                    member(scenario, "raw_before_close"),
                    &calls_value(&before_close)
                ),
                "{name} {size}: before the close"
            );
            assert!(
                same_value(member(scenario, "raw_after_close"), &got_after_close),
                "{name} {size}: after the close"
            );
        }
    }

    fn chunks_of(name: &str) -> Vec<PyStr> {
        let bad = PyStr::from_code_points(vec![u32::from('c'), 0xD800]);
        let body = PyStr::from_code_points(vec![u32::from('b'), 0xD800]);
        let a = |count: usize| PyStr::new(&"a".repeat(count));
        let b = |count: usize| PyStr::new(&"b".repeat(count));
        match name.split(' ').next().expect("a name") {
            "F1" | "F1b" => vec![a(6000), b(6000), bad],
            "F2" | "F3" | "F5" | "F8" => vec![a(100), body],
            "F4" | "F4b" | "F5b" => vec![a(100)],
            "F6" => vec![a(6000)],
            "F7" => (0..5).map(|_| PyStr::new(&"k".repeat(3000))).collect(),
            other => panic!("{other}"),
        }
    }

    /// The saved primitive 2: failing raw writes and closes under a `with` block. What each write reports, the raw writes made and
    /// attempted, and the whole exception chain (class, text, implicit context) are the saved ones for every scenario.
    #[test]
    fn the_modelled_writes_closes_and_exception_chains_are_the_stdlib_primitives() {
        let scenarios = saved_list("io/primitive_2_faults.json");
        assert_eq!(scenarios.len(), 11);
        for scenario in &scenarios {
            let name = text_of(member(scenario, "name"));
            let plan = Plan::from_saved(member(scenario, "plan"));
            let log = Rc::new(RefCell::new(Log::default()));
            let file = TextFile::new(
                PyStr::new("fake"),
                Box::new(Fake {
                    log: log.clone(),
                    plan,
                    size: 4096,
                }),
            );
            let mut steps = Vec::new();
            let outcome: Done<()> = run_in_block(&mut Plain, file, |host, file| {
                for (index, chunk) in chunks_of(&name).iter().enumerate() {
                    match file.write_text(host, chunk) {
                        Ok(()) => steps.push(PyValue::List(vec![
                            int(index),
                            PyValue::str("ok"),
                            calls_value(&log.borrow().calls),
                            int(log.borrow().attempts),
                        ])),
                        Err(failure) => {
                            steps.push(PyValue::List(vec![
                                int(index),
                                PyValue::str(&failure.raised.class),
                                PyValue::Str(failure.raised.message.clone()),
                                calls_value(&log.borrow().calls),
                                int(log.borrow().attempts),
                            ]));
                            return Err(failure);
                        }
                    }
                }
                Ok(())
            });
            assert!(
                same_value(member(scenario, "steps"), &PyValue::List(steps)),
                "{name}: steps"
            );
            let rows = match &outcome {
                Ok(()) => PyValue::None,
                Err(failure) => PyValue::List(
                    items(&chain_of(failure))
                        .iter()
                        .map(|row| {
                            PyValue::List(vec![
                                member(row, "type").clone(),
                                member(row, "message").clone(),
                                member(row, "relation").clone(),
                            ])
                        })
                        .collect(),
                ),
            };
            // The controlled close failure has no errno of its own here: its text is `close failed` without the `[Errno 5] ` prefix.
            let want = text_of_chain(member(scenario, "result"));
            assert_eq!(text_of_chain(&rows), want, "{name}: chain");
            assert!(
                same_value(
                    member(scenario, "raw_calls"),
                    &calls_value(&log.borrow().calls)
                ),
                "{name}: raw writes"
            );
            let attempts = match member(scenario, "raw_attempts") {
                PyValue::Int(number) => usize::try_from(number.clone()).expect("a count"),
                other => panic!("{}", other.repr()),
            };
            assert_eq!(log.borrow().attempts, attempts, "{name}: attempts");
            assert_eq!(log.borrow().close_calls, 1, "{name}: one raw close");
        }
    }

    fn text_of_chain(rows: &PyValue) -> Vec<(String, String, String)> {
        match rows {
            PyValue::None => Vec::new(),
            other => items(other)
                .iter()
                .map(|row| {
                    let parts = items(row);
                    (
                        text_of(&parts[0]),
                        text_of(&parts[1]).replace("[Errno 5] close failed", "close failed"),
                        match &parts[2] {
                            PyValue::None => "none".to_string(),
                            relation => text_of(relation),
                        },
                    )
                })
                .collect(),
        }
    }

    // ── the owner over real files with a controlled raw end ──

    /// A raw end that writes through to the real file and fails as planned (attempts counted per file).
    struct Flaky {
        inner: FsRaw,
        plan: Plan,
        attempts: usize,
    }

    impl Raw for Flaky {
        fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
            let index = self.attempts;
            self.attempts += 1;
            if self.plan.always || self.plan.fail_at.contains(&index) {
                return Err(eio());
            }
            self.inner.write(data)
        }

        fn close(&mut self) -> std::io::Result<()> {
            self.inner.close()?;
            if self.plan.close_fails {
                return Err(std::io::Error::other("close failed"));
            }
            Ok(())
        }

        fn buffer_size(&self) -> usize {
            self.inner.buffer_size()
        }
    }

    /// Records the observed events as `(name, [path, type, message])` and wraps the files whose name ends as the plan says.
    struct Controlled {
        plans: Vec<(&'static str, Plan)>,
        events: Vec<(String, Vec<String>)>,
    }

    impl Host for Controlled {
        fn enabled(&self) -> bool {
            true
        }

        fn record(&mut self, name: &str, fields: Vec<(&str, PyValue)>) {
            let shown = fields
                .iter()
                .filter(|(key, _)| ["type", "message"].contains(key))
                .map(|(_, value)| text_of(value))
                .collect();
            self.events.push((name.to_string(), shown));
        }

        fn raw_file(&mut self, path: &PyStr, file: std::fs::File) -> Box<dyn Raw> {
            let name = path.to_utf8().expect("a path");
            match self.plans.iter().find(|(suffix, _)| name.ends_with(suffix)) {
                Some((_, plan)) => Box::new(Flaky {
                    inner: FsRaw::new(file),
                    plan: plan.clone(),
                    attempts: 0,
                }),
                None => Box::new(FsRaw::new(file)),
            }
        }
    }

    fn plan(fail_at: &[usize], always: bool, close_fails: bool) -> Plan {
        Plan {
            fail_at: fail_at.to_vec(),
            partial: Vec::new(),
            always,
            close_fails,
        }
    }

    fn summary_run(
        summary: &str,
        controlled: Plan,
    ) -> (PrivateDirectory, Controlled, Done<PyValue>) {
        let area = PrivateDirectory::create("simlog-buffering-").expect("a private directory");
        let run_dir = PyValue::str(area.path().join("run").to_str().expect("a path"));
        let mut host = Controlled {
            plans: vec![("run_summary.json", controlled)],
            events: Vec::new(),
        };
        let value = loads_text(&PyStr::new(summary)).expect("JSON");
        let outcome = write_summary(&mut host, &run_dir, &value);
        (area, host, outcome)
    }

    fn names(host: &Controlled) -> Vec<&str> {
        host.events.iter().map(|(name, _)| name.as_str()).collect()
    }

    fn file_bytes(area: &PrivateDirectory) -> Vec<u8> {
        std::fs::read(area.path().join("run/run_summary.json")).expect("the summary file")
    }

    #[test]
    fn a_failing_flush_of_a_large_prefix_stops_the_body_before_a_later_encoding_error() {
        // Two items of 7000 bytes reach the 8192-byte text chunk at the second one: its flush fails, so the chunk with the lone surrogate is never written.
        let summary = format!(
            r#"["{}", "{}", "c\ud800"]"#,
            "a".repeat(7000),
            "b".repeat(7000)
        );
        let (area, host, outcome) = summary_run(&summary, plan(&[0], false, false));
        let failure = outcome.expect_err("the write fails");
        assert_eq!(failure.raised.class, "OSError");
        assert_eq!(
            failure.raised.message.to_utf8().expect("text"),
            "[Errno 5] Input/output error"
        );
        assert_eq!(
            names(&host),
            ["makedirs", "file_open", "file_write_error", "file_close"]
        );
        assert_eq!(
            host.events[2].1,
            ["OSError", "[Errno 5] Input/output error"]
        );
        assert!(file_bytes(&area).is_empty());
    }

    #[test]
    fn a_close_failure_after_a_body_failure_wins_and_keeps_the_body_failure_with_its_constructor_arguments_as_context(
    ) {
        let summary = r#"["a", "b\ud800"]"#;
        let object: Vec<u32> = ",\n  \"b"
            .chars()
            .map(u32::from)
            .chain([0xD800, u32::from('"')])
            .collect();
        let check_args = |failure: &recipe_runtime::simlog::Thrown, row: usize| {
            let chain = chain_of(failure);
            let args = items(member(&items(&chain)[row], "constructor_args"));
            assert_eq!(text_of(&args[0]), "utf-8");
            assert!(
                matches!(&args[1], PyValue::Str(text) if text.code_points() == object.as_slice())
            );
            assert!(same_value(&args[2], &int(6)) && same_value(&args[3], &int(7)));
            assert_eq!(text_of(&args[4]), "surrogates not allowed");
        };
        // The first flush at the close fails, the second succeeds: that failure is the block's, the body's error its context.
        let (area, host, outcome) = summary_run(summary, plan(&[0], false, false));
        let failure = outcome.expect_err("the close fails");
        let rows: Vec<String> = items(&chain_of(&failure))
            .iter()
            .map(|row| {
                format!(
                    "{}/{}",
                    text_of(member(row, "type")),
                    member(row, "relation").repr()
                )
            })
            .collect();
        assert_eq!(rows, ["OSError/'context'", "UnicodeEncodeError/None"]);
        check_args(&failure, 1);
        assert!(!items(&chain_of(&failure))[0]
            .repr()
            .contains("constructor_args"));
        assert_eq!(
            names(&host),
            [
                "makedirs",
                "file_open",
                "file_write_error",
                "file_close_error"
            ]
        );
        assert_eq!(host.events[2].1[0], "UnicodeEncodeError");
        assert_eq!(
            host.events[3].1,
            ["OSError", "[Errno 5] Input/output error"]
        );
        assert_eq!(file_bytes(&area), b"[\n  \"a\"");
        // Every raw write fails: the second close failure chains the first, which keeps the body's error as its context.
        let (area, host, outcome) = summary_run(summary, plan(&[], true, false));
        let failure = outcome.expect_err("the close fails");
        let rows: Vec<String> = items(&chain_of(&failure))
            .iter()
            .map(|row| text_of(member(row, "type")))
            .collect();
        assert_eq!(rows, ["OSError", "OSError", "UnicodeEncodeError"]);
        check_args(&failure, 2);
        assert_eq!(names(&host).last(), Some(&"file_close_error"));
        assert!(file_bytes(&area).is_empty());
        // The raw close fails after a good body: a failure alone, with no context, and the bytes were written.
        let (area, host, outcome) = summary_run(r#"{"a": 1}"#, plan(&[], false, true));
        let failure = outcome.expect_err("the raw close fails");
        assert_eq!(
            failure.raised.message.to_utf8().expect("text"),
            "close failed"
        );
        assert!(failure.raised.cause.is_none());
        assert_eq!(names(&host).last(), Some(&"file_close_error"));
        assert_eq!(file_bytes(&area), b"{\n  \"a\": 1\n}");
    }

    #[test]
    fn a_failing_yaml_flush_is_not_a_write_event_and_the_close_then_succeeds() {
        let area = PrivateDirectory::create("simlog-yaml-flush-").expect("a private directory");
        let run_dir = PyValue::str(area.path().join("run").to_str().expect("a path"));
        let mut host = Controlled {
            plans: vec![("_candidate.yaml", plan(&[0], false, false))],
            events: Vec::new(),
        };
        let value = |text: &str| loads_text(&PyStr::new(text)).expect("JSON");
        let (profile, candidate, smoke) =
            (value(r#"{"p": 1}"#), value(r#"{"c": [1, 2]}"#), value("{}"));
        let (number, log) = (PyValue::int(1), PyValue::str("log"));
        let outcome = write_trial(
            &mut host,
            true,
            [&run_dir, &number, &log, &profile, &candidate, &smoke],
        );
        let failure = outcome.expect_err("the flush fails");
        assert_eq!(failure.raised.class, "OSError");
        assert!(failure.raised.cause.is_none());
        // The emitter's flush is the file's own `flush` (not the wrapper's `write`): no write event; the retry at the close wrote the bytes.
        assert!(!host
            .events
            .iter()
            .any(|(name, _)| name == "file_write_error"));
        assert_eq!(
            host.events.last().map(|(name, _)| name.as_str()),
            Some("file_close")
        );
        assert_eq!(
            std::fs::read_to_string(area.path().join("run/trial01_candidate.yaml"))
                .expect("the candidate"),
            "c:\n- 1\n- 2\n"
        );
        assert!(!area.path().join("run/trial01_smoke.json").exists());
    }
}

// ── the descriptor close of a real file ──────────────────────────────────────────────────────────

mod close_cases {
    use super::*;
    use recipe_runtime::simlog::{write_summary, FsRaw, Host, Raw};
    use recipe_runtime::simlog_api::chain_of;
    use std::cell::Cell;
    use std::rc::Rc;

    extern "C" {
        fn close(descriptor: i32) -> i32;
        fn fcntl(descriptor: i32, command: i32, ...) -> i32;
        fn __errno_location() -> *mut i32;
    }

    /// `F_GETFD`: -1 for a descriptor that is not open.
    fn is_open(descriptor: i32) -> bool {
        // SAFETY: `F_GETFD` only queries the flags of a descriptor number.
        unsafe { fcntl(descriptor, 1) >= 0 }
    }

    /// The controlled stand-in for the system call, over a private descriptor this test owns: it counts its calls, closes that
    /// descriptor exactly once (the real `close`), and then reports `errno` as a failure (or success when `errno` is 0).
    fn stand_in(
        errno: i32,
        calls: &Rc<Cell<usize>>,
        seen: &Rc<Cell<i32>>,
    ) -> Box<dyn FnOnce(i32) -> i32> {
        let (calls, seen) = (calls.clone(), seen.clone());
        Box::new(move |descriptor| {
            calls.set(calls.get() + 1);
            seen.set(descriptor);
            // Still open when the call is made: nothing closed it before (a drop would have), and the real close below succeeds.
            assert!(
                is_open(descriptor),
                "the descriptor is open when the close is called"
            );
            // SAFETY: the descriptor belongs to the private file this test opened and handed over; it is closed here once.
            let result = unsafe { close(descriptor) };
            assert_eq!(result, 0, "the private descriptor closes");
            if errno == 0 {
                return 0;
            }
            // SAFETY: `__errno_location` is the calling thread's `errno`; it is set last, as the failing system call would leave it.
            unsafe { *__errno_location() = errno };
            -1
        })
    }

    fn private_file(area: &PrivateDirectory, name: &str) -> std::fs::File {
        std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(area.path().join(name))
            .expect("a private file")
    }

    #[test]
    fn the_file_is_consumed_once_its_descriptor_released_and_the_systems_error_returned_without_a_retry(
    ) {
        let area = PrivateDirectory::create("simlog-close-").expect("a private directory");
        // Success: the descriptor is released by the one call; closing again is nothing (no second close), writing is an error.
        let (calls, seen) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(-1)));
        let mut raw = FsRaw::with_close_call(private_file(&area, "ok"), stand_in(0, &calls, &seen));
        raw.close().expect("the close succeeds");
        assert_eq!(calls.get(), 1);
        raw.close().expect("closing a closed file is nothing");
        assert_eq!(calls.get(), 1);
        assert!(raw.write(b"x").is_err());
        // An error: the errno the call left is the error, the descriptor was released all the same, and the file is closed afterwards.
        for (errno, kind) in [
            (5, std::io::ErrorKind::Other),
            (9, std::io::ErrorKind::Other),
            (4, std::io::ErrorKind::Interrupted),
        ] {
            let (calls, seen) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(-1)));
            let mut raw = FsRaw::with_close_call(
                private_file(&area, &format!("e{errno}")),
                stand_in(errno, &calls, &seen),
            );
            let error = raw.close().expect_err("the close fails");
            assert_eq!(error.raw_os_error(), Some(errno));
            if errno == 4 {
                assert_eq!(error.kind(), kind, "EINTR is returned as it is");
            }
            assert_eq!(calls.get(), 1, "errno {errno}: the close is not retried");
            raw.close().expect("a second close is nothing, not a retry");
            assert_eq!(calls.get(), 1);
        }
    }

    /// Wraps the real file; the writes may fail every time; the close is the real file's (and so the stand-in's).
    struct Wrapped {
        inner: FsRaw,
        writes_fail: bool,
    }

    impl Raw for Wrapped {
        fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
            if self.writes_fail {
                return Err(std::io::Error::from_raw_os_error(5));
            }
            self.inner.write(data)
        }

        fn close(&mut self) -> std::io::Result<()> {
            self.inner.close()
        }

        fn buffer_size(&self) -> usize {
            self.inner.buffer_size()
        }
    }

    struct CloseHost {
        errno: i32,
        writes_fail: bool,
        calls: Rc<Cell<usize>>,
        seen: Rc<Cell<i32>>,
        events: Vec<String>,
    }

    impl Host for CloseHost {
        fn enabled(&self) -> bool {
            true
        }

        fn record(&mut self, name: &str, _fields: Vec<(&str, PyValue)>) {
            self.events.push(name.to_string());
        }

        fn raw_file(&mut self, _path: &PyStr, file: std::fs::File) -> Box<dyn Raw> {
            Box::new(Wrapped {
                inner: FsRaw::with_close_call(file, stand_in(self.errno, &self.calls, &self.seen)),
                writes_fail: self.writes_fail,
            })
        }
    }

    fn run(
        errno: i32,
        writes_fail: bool,
        summary: &str,
    ) -> (
        PrivateDirectory,
        CloseHost,
        Result<PyValue, Box<recipe_runtime::simlog::Thrown>>,
    ) {
        let area = PrivateDirectory::create("simlog-close-run-").expect("a private directory");
        let run_dir = PyValue::str(area.path().join("run").to_str().expect("a path"));
        let mut host = CloseHost {
            errno,
            writes_fail,
            calls: Rc::new(Cell::new(0)),
            seen: Rc::new(Cell::new(-1)),
            events: Vec::new(),
        };
        let value = loads_text(&PyStr::new(summary)).expect("JSON");
        let outcome = write_summary(&mut host, &run_dir, &value);
        (area, host, outcome)
    }

    #[test]
    fn a_close_error_of_the_descriptor_reaches_the_caller_with_the_systems_text_and_no_file_name() {
        // A good body: the bytes were written, the close error is the exception, observed as `file_close_error`, never as `file_close`.
        let (area, host, outcome) = run(5, false, r#"{"a": 1}"#);
        let failure = outcome.expect_err("the close fails");
        assert_eq!(failure.raised.class, "OSError");
        assert_eq!(
            failure.raised.message.to_utf8().expect("text"),
            "[Errno 5] Input/output error"
        );
        assert!(failure.raised.cause.is_none());
        assert_eq!(
            host.events.last().map(String::as_str),
            Some("file_close_error")
        );
        assert!(!host.events.iter().any(|name| name == "file_close"));
        assert_eq!(host.calls.get(), 1);
        assert_eq!(
            std::fs::read(area.path().join("run/run_summary.json")).expect("the file"),
            b"{\n  \"a\": 1\n}"
        );
        // Other errnos keep their own text and class; EINTR is not retried.
        let (_, host, outcome) = run(9, false, "{}");
        let failure = outcome.expect_err("the close fails");
        assert_eq!(
            failure.raised.message.to_utf8().expect("text"),
            "[Errno 9] Bad file descriptor"
        );
        assert_eq!(failure.raised.class, "OSError");
        let (_, host_eintr, outcome) = run(4, false, "{}");
        let failure = outcome.expect_err("the close fails");
        assert_eq!(failure.raised.class, "InterruptedError");
        assert_eq!(
            failure.raised.message.to_utf8().expect("text"),
            "[Errno 4] Interrupted system call"
        );
        assert_eq!((host.calls.get(), host_eintr.calls.get()), (1, 1));
        // No error: the ordinary `file_close`.
        let (_, host, outcome) = run(0, false, "{}");
        outcome.expect("the summary is written");
        assert_eq!(host.events.last().map(String::as_str), Some("file_close"));
        assert_eq!(host.calls.get(), 1);
    }

    #[test]
    fn the_descriptor_is_released_and_its_error_chained_when_the_body_or_the_flush_failed_first() {
        // A body encoding error, then the close error: the close error is the exception, the body's error (with its constructor arguments) its context.
        let (_, host, outcome) = run(5, false, r#"["a", "b\ud800"]"#);
        let failure = outcome.expect_err("the close fails");
        let chain = chain_of(&failure);
        let rows: Vec<String> = items(&chain)
            .iter()
            .map(|row| {
                format!(
                    "{}/{}",
                    text_of(member(row, "type")),
                    member(row, "relation").repr()
                )
            })
            .collect();
        assert_eq!(rows, ["OSError/'context'", "UnicodeEncodeError/None"]);
        assert!(items(&chain)[1].repr().contains("constructor_args"));
        assert_eq!(
            host.events.last().map(String::as_str),
            Some("file_close_error")
        );
        assert_eq!(host.calls.get(), 1);
        // Every flush fails and so does the close: the descriptor is still released (once), the close error chains the first flush error.
        let (_, host, outcome) = run(5, true, r#"{"a": 1}"#);
        let failure = outcome.expect_err("the flush and the close fail");
        let rows: Vec<String> = items(&chain_of(&failure))
            .iter()
            .map(|row| text_of(member(row, "type")))
            .collect();
        assert_eq!(rows, ["OSError", "OSError"]);
        assert_eq!(host.calls.get(), 1);
    }
}
