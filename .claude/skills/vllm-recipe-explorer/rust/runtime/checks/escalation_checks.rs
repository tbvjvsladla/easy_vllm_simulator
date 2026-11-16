//! The registered checks of the escalation owner (`escalation_predicate.py`): the saved original observations, observed whole, over the
//! real classifier dependency, and the programs in an installed layout.
//!
//! These checks need the `fixture` feature (`required-features`). Nothing here reads the real `/proc`, memory, a node, the network or a GPU;
//! the only process the programs start is the existing canonical classifier through its launcher (the borrowed ELF beside the skill).
//!
//! * `reference_cases`: the 107 saved `judge` calls through `recipe_escalation_fixture_api case` and the 24 saved command lines through
//!   `escalation_predicate_fixture`, each in its own private directory rebuilt from the saved before-tree: the whole control record (value or
//!   exception with its cause chain, ordered loader/event/stderr/launcher trace, every launcher run's argv, input, status and streams,
//!   globals before and after, arguments after), exit status, raw streams (a traceback stripped only of known frames) and the complete tree
//!   after. Only the token `@CASE@` is rebased. The original is never run. The four interop helper cases belong to the Python layer.
//! * `production_cases`: the installed programs (the layout the skill root and the asset are derived from).
//! * `session_cases`: the loader session with a scripted caller.
//! * `bench_cases`: the benchmark program on a tiny schedule (diagnostic children).

use agent_runtime_support::pyjson::loads_text;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use recipe_runtime::model_files::PrivateDirectory;
use recipe_runtime::observe::same_value;
use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// ── data ─────────────────────────────────────────────────────────────────────────────────────────

fn data(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("checks")
        .join("data")
        .join("escalation_reference")
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

/// What descriptor 0 of a service started over a protocol descriptor is: a file it can only write to (so that any read of standard
/// input would fail loudly).
enum StandardInput<'a> {
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

// ── the dependency's environment ─────────────────────────────────────────────────────────────────

/// The canonical classifier ELF borrowed beside the skill (`<skills>/adversarial-benchmark/rust/target/release`).
fn canonical_elf() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("adversarial-benchmark")
        .join("rust")
        .join("target")
        .join("release")
        .join("classify_cell_api")
}

/// The environment the original's collector gave the launcher: the backend (the canonical ELF, or the case's owned backend), the
/// scripts directory and the role adapter path.
fn dependency_environment(command: &mut Command, case: &Case, request: &PyValue) {
    let backend = match request {
        PyValue::Dict(map) => match map.get("context") {
            Some(PyValue::Dict(context)) => context.get("backend").cloned(),
            _ => None,
        },
        _ => None,
    };
    let backend_path = if backend.is_some() {
        case.root.join("owned-backend")
    } else {
        canonical_elf()
    };
    let scripts = case
        .root
        .join("node/.claude/skills/adversarial-benchmark/scripts");
    command
        .env("CLASSIFY_CELL_API_BIN", backend_path)
        .env("CLASSIFY_CELL_SCRIPTS_DIR", scripts)
        .env(
            "CLASSIFY_CELL_ROLE_ADAPTER",
            case.root.join("owned-unavailable-role-adapter.py"),
        );
}

// ── saved traceback projection ───────────────────────────────────────────────────────────────────

const HEADER: &[u8] = b"Traceback (most recent call last):\n";

/// The saved standard error with the original's traceback frames removed and nothing else: for each traceback section the header and
/// the frames of a known file (the original scripts, the collector, the interpreter's frozen import and codec modules) go with their
/// source lines; every other byte stays, the separators between chained exceptions and every complete final block included. A frame of
/// any other file, or a header without frames, is refused.
fn project_traceback(saved: &[u8]) -> Result<Vec<u8>, String> {
    let allowed = |file: &[u8]| -> bool {
        file == b"@CASE@/node/.claude/skills/vllm-recipe-explorer/scripts/escalation_predicate.py"
            || file == b"@CASE@/node/.claude/skills/adversarial-benchmark/scripts/classify_cell_native.py"
            || file.ends_with(b"/r28_original_one_call.py")
            || file == b"<frozen codecs>"
            || file == b"<frozen importlib._bootstrap_external>"
            || file == b"<frozen importlib._bootstrap>"
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
        let mut command = Command::new(env!("CARGO_BIN_EXE_recipe_escalation_fixture_api"));
        command
            .arg("case")
            .arg("--spec")
            .arg(&spec)
            .arg("--root")
            .arg(&work.root)
            .arg("--control")
            .arg(&control);
        fixture_environment(&mut command, &work);
        dependency_environment(&mut command, &work, request);
        let output = command.output().expect("run the fixture API");
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
    fn the_107_saved_judge_calls_are_reproduced_whole_with_their_loader_trace_launcher_runs_globals_and_tree(
    ) {
        let bundle = load("bundle.json");
        let cases = items(member(&bundle, "api"));
        assert_eq!(cases.len(), 107);
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
        let mut command = Command::new(env!("CARGO_BIN_EXE_escalation_predicate_fixture"));
        command
            .arg("--spec")
            .arg(&spec)
            .arg("--root")
            .arg(&work.root)
            .arg("--control")
            .arg(&control);
        fixture_environment(&mut command, &work);
        dependency_environment(&mut command, &work, request);
        let output = command.output().expect("run the fixture command line");
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
    fn the_24_saved_command_lines_end_as_the_original_did_with_their_trace_launcher_runs_streams_and_tree(
    ) {
        let bundle = load("bundle.json");
        let cases = items(member(&bundle, "cli"));
        assert_eq!(cases.len(), 24);
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
        let saved: &[u8] = b"Traceback (most recent call last):\n  File \"@CASE@/node/.claude/skills/adversarial-benchmark/scripts/classify_cell_native.py\", line 1, in <module>\n    x\nRuntimeError: a\nsecond line\n\nThe above exception was the direct cause of the following exception:\n\nTraceback (most recent call last):\n  File \"/x/r28_original_one_call.py\", line 9, in <module>\n    y\n  File \"<frozen importlib._bootstrap>\", line 488, in f\nImportError: a\nsecond line\n";
        assert_eq!(
            project_traceback(saved).expect("projected"),
            b"RuntimeError: a\nsecond line\n\nThe above exception was the direct cause of the following exception:\n\nImportError: a\nsecond line\n".to_vec()
        );
        assert_eq!(
            project_traceback(b"[escalation] x\n").expect("plain"),
            b"[escalation] x\n".to_vec()
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

// ── the production programs in an installed layout ───────────────────────────────────────────────

/// `<area>/skills/{vllm-recipe-explorer/rust/runtime/target/release,adversarial-benchmark/scripts}`: the layout the programs derive
/// their skill root and the classifier asset from. Returns the skill root; the program is linked under the release directory.
fn installed_layout(bundle: &PyValue, area: &Path, program: &str) -> PathBuf {
    let skills = area.join("skills");
    let release = skills.join("vllm-recipe-explorer/rust/runtime/target/release");
    std::fs::create_dir_all(&release).expect("the release directory");
    let scripts = skills.join("adversarial-benchmark/scripts");
    std::fs::create_dir_all(&scripts).expect("the scripts directory");
    for name in ["classify_cell_native.py", "classify_cell_api.sh"] {
        let bytes = hex_decode(&text_of(member(member(bundle, "sources"), name)));
        std::fs::write(scripts.join(name), bytes).expect("an asset");
    }
    let target = release.join(Path::new(program).file_name().expect("a name"));
    // A link, not a copy: the program locates its skill root from the path it was started by (lexically, links unresolved), and a
    // freshly written executable could be refused as busy while another test forks.
    std::os::unix::fs::symlink(program, &target).expect("the program link");
    skills.join("vllm-recipe-explorer")
}

fn production_command(program: &Path, area: &Path) -> Command {
    let mut command = Command::new(program);
    command
        .env_clear()
        .env("TMPDIR", area)
        .env("CLASSIFY_CELL_API_BIN", canonical_elf())
        .stdin(Stdio::null());
    command
}

mod production_cases {
    use super::*;

    fn run_over_descriptor(
        program: &Path,
        arguments: &[&str],
        protocol: &str,
        zero: &Path,
    ) -> std::process::Output {
        let mut command = {
            let (mut command, mut writer) = over_descriptor(
                program.to_str().expect("a path"),
                arguments,
                &StandardInput::WriteOnly(zero),
            );
            command.env("CLASSIFY_CELL_API_BIN", canonical_elf());
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

    /// Whether the saved case can be asked of the installed layout as it is: the standard assets and backend, whatever globals.
    fn is_standard(request: &PyValue) -> bool {
        match member(request, "context") {
            PyValue::Dict(context) => context.iter().all(|(key, _)| {
                ["model_add", "host_add", "refuted"]
                    .iter()
                    .any(|name| key.equals(name))
            }),
            _ => false,
        }
    }

    /// The saved judge calls that run against the standard installed classifier, through `evaluate-jsonl` of the installed program
    /// (the asset located from the layout, the real `constants` call of the existing Rust classifier each judgment): the record the
    /// original gave, minus the collector's observations, the globals passed as a member of their own.
    #[test]
    fn the_saved_calls_agree_through_the_installed_service_with_the_real_classifier() {
        let bundle = load("bundle.json");
        let area = PrivateDirectory::create("escalation-installed-").expect("a private directory");
        installed_layout(
            &bundle,
            area.path(),
            env!("CARGO_BIN_EXE_recipe_escalation_api"),
        );
        let program = area
            .path()
            .join("skills/vllm-recipe-explorer/rust/runtime/target/release/recipe_escalation_api");
        let mut protocol = String::new();
        let mut wanted = Vec::new();
        for case in items(member(&bundle, "api")) {
            let request = member(case, "request");
            if !is_standard(request) {
                continue;
            }
            let mut line = PyDict::new();
            line.set("id", member(request, "id").clone());
            line.set("operation", PyValue::str("escalation_predicate.judge"));
            line.set("args", member(request, "args").clone());
            line.set("kwargs", member(request, "kwargs").clone());
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
                &["trace", "rpc", "globals_before", "globals_after"],
            ));
        }
        assert_eq!(wanted.len(), 107 - 14);
        let zero = area.path().join("zero");
        let output = run_over_descriptor(
            &program,
            &["evaluate-jsonl", "--input-fd", "3"],
            &protocol,
            &zero,
        );
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
    fn the_constants_are_this_owners_own_strings_and_a_request_member_beyond_the_five_is_refused() {
        let bundle = load("bundle.json");
        let output = Command::new(env!("CARGO_BIN_EXE_recipe_escalation_api"))
            .arg("constants")
            .env_clear()
            .output()
            .expect("run");
        let got = loads_text(&PyStr::from_surrogateescape(&output.stdout)).expect("constants");
        let saved = load("bundle.json");
        let reference = member(&saved, "constants");
        for name in [
            "MODEL_PLANE_SYMPTOMS",
            "HOST_PLANE_SIGNS",
            "REFUTED_BY_KILL",
        ] {
            assert!(
                same_value(member(reference, name), member(&got, name)),
                "{name}"
            );
        }
        assert!(!got.repr().contains("KILL_EVENT_KINDS"));
        let area = PrivateDirectory::create("escalation-refuse-").expect("a private directory");
        installed_layout(
            &bundle,
            area.path(),
            env!("CARGO_BIN_EXE_recipe_escalation_api"),
        );
        let program = area
            .path()
            .join("skills/vllm-recipe-explorer/rust/runtime/target/release/recipe_escalation_api");
        let zero = area.path().join("zero");
        let requests = [
            r#"{"id": 1, "operation": "escalation_predicate.judge", "args": [[], []], "context": {}}"#,
            r#"{"id": 2, "operation": "escalation_predicate.judge", "args": [[], []], "globals": {"KILL_EVENT_KINDS": []}}"#,
            r#"{"id": 3, "operation": "escalation_predicate.other", "args": []}"#,
            r#"{"id": 4, "operation": "other.judge", "args": []}"#,
        ];
        let output = run_over_descriptor(
            &program,
            &["evaluate-jsonl", "--input-fd", "3"],
            &format!("{}\n", requests.join("\n")),
            &zero,
        );
        let got = records_of(&output);
        let kinds: Vec<(String, String)> = got
            .iter()
            .map(|row| {
                (
                    text_of(member(row, "type")),
                    text_of(member(row, "message")),
                )
            })
            .collect();
        assert_eq!(kinds[0].0, "RequestError");
        assert!(kinds[0]
            .1
            .starts_with("the request has a member this service does not take: "));
        assert_eq!(kinds[1].0, "RequestError");
        assert_eq!(
            kinds[2],
            (
                "AttributeError".to_string(),
                "module 'escalation_predicate' has no attribute 'other'".to_string()
            )
        );
        assert_eq!(
            kinds[3],
            (
                "ModuleNotFoundError".to_string(),
                "No module named 'other'".to_string()
            )
        );
        for arguments in [
            vec!["judge-session"],
            vec!["evaluate-jsonl", "--input-fd", "1"],
            vec!["constants", "x"],
            vec![],
        ] {
            let output = Command::new(&program)
                .args(&arguments)
                .env_clear()
                .stdin(Stdio::null())
                .output()
                .expect("run");
            assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        }
        let output = Command::new(&program)
            .args(["evaluate-jsonl", "--input-fd", "3"])
            .env_clear()
            .stdin(Stdio::null())
            .output()
            .expect("run");
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stderr).contains("not an open descriptor"));
    }

    /// The installed command line, Python-free: the real classifier through its launcher, the saved text of case 4 (a symptom alone) and
    /// of case 5 (a host sign alone), the self-test's nine checks and its nine launcher runs.
    #[test]
    fn the_installed_command_line_judges_and_self_tests_through_the_real_classifier() {
        let bundle = load("bundle.json");
        let area = PrivateDirectory::create("escalation-cli-").expect("a private directory");
        installed_layout(
            &bundle,
            area.path(),
            env!("CARGO_BIN_EXE_escalation_predicate"),
        );
        let program = area
            .path()
            .join("skills/vllm-recipe-explorer/rust/runtime/target/release/escalation_predicate");
        let cli = items(member(&bundle, "cli"));
        for (index, status) in [(2usize, "0"), (4, "0"), (5, "1"), (6, "1")] {
            let request = member(&cli[index], "request");
            let args: Vec<String> = items(member(request, "args")).iter().map(text_of).collect();
            let output = production_command(&program, area.path())
                .args(&args)
                .output()
                .expect("run");
            assert_eq!(
                output.status.code().map(|code| code.to_string()).as_deref(),
                Some(status),
                "case {index} {args:?}"
            );
            assert_eq!(
                output.stdout,
                hex_decode(&text_of(member(&cli[index], "stdout_hex"))),
                "case {index}"
            );
            assert!(output.stderr.is_empty(), "case {index}");
        }
    }

    /// A production install needs no `.py` adapter: with `classify_cell_native.py` removed the command line still judges through the
    /// launcher, and with the launcher removed it refuses (exit 1 · the owner's path named) instead of judging.
    #[test]
    fn the_installed_command_line_needs_the_launcher_and_not_the_python_adapter() {
        let bundle = load("bundle.json");
        let area = PrivateDirectory::create("escalation-nopy-").expect("a private directory");
        installed_layout(&bundle, area.path(), env!("CARGO_BIN_EXE_escalation_predicate"));
        let scripts = area.path().join("skills/adversarial-benchmark/scripts");
        std::fs::remove_file(scripts.join("classify_cell_native.py")).expect("remove the adapter");
        let program = area
            .path()
            .join("skills/vllm-recipe-explorer/rust/runtime/target/release/escalation_predicate");
        let cli = items(member(&bundle, "cli"));
        let request = member(&cli[2], "request");
        let args: Vec<String> = items(member(request, "args")).iter().map(text_of).collect();
        let output = production_command(&program, area.path()).args(&args).output().expect("run");
        assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(output.stdout, hex_decode(&text_of(member(&cli[2], "stdout_hex"))));
        std::fs::remove_file(scripts.join("classify_cell_api.sh")).expect("remove the launcher");
        let output = production_command(&program, area.path()).args(&args).output().expect("run");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_ne!(output.status.code(), Some(0), "{stderr}");
        assert!(stderr.contains("classify_cell_api.sh") && stderr.contains("정본을 찾지 못했다"), "{stderr}");
    }

    /// Reading the source: nothing selects another classifier, provider or path, and no operational capture path is in the programs.
    #[test]
    fn the_sources_have_no_selector_no_fixture_module_and_no_operational_path() {
        let sources = [
            ("escalation.rs", include_str!("../src/escalation.rs")),
            (
                "escalation_api.rs",
                include_str!("../src/escalation_api.rs"),
            ),
            (
                "escalation_bench.rs",
                include_str!("../src/escalation_bench.rs"),
            ),
            (
                "escalation_bridge.rs",
                include_str!("../src/escalation_bridge.rs"),
            ),
            (
                "escalation_cli.rs",
                include_str!("../src/escalation_cli.rs"),
            ),
            (
                "bin/escalation_predicate.rs",
                include_str!("../src/bin/escalation_predicate.rs"),
            ),
            (
                "bin/recipe_escalation_api.rs",
                include_str!("../src/bin/recipe_escalation_api.rs"),
            ),
            (
                "bin/recipe_escalation_bench.rs",
                include_str!("../src/bin/recipe_escalation_bench.rs"),
            ),
        ];
        let mut found = Vec::new();
        for (name, text) in sources {
            for forbidden in [
                "env::var",
                "getenv",
                "escalation_fixture",
                "real-worker",
                "val_agent",
                "CARGO_MANIFEST_DIR",
                "BoundaryViolation",
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

// ── the loader session ───────────────────────────────────────────────────────────────────────────

mod session_cases {
    use super::*;

    enum Reply {
        Loaded(&'static str),
        Raise(String),
        Abort,
        Raw(&'static str),
        CloseInput,
    }

    struct Ended {
        events: Vec<PyValue>,
        last: PyValue,
    }

    const KINDS: &str = r#"["watchdog_kill_ack", "thermal_kill_ack", "earlyoom_kill"]"#;

    fn saved(number: usize) -> PyValue {
        items(member(&load("bundle.json"), "api"))[number].clone()
    }

    fn drive(number: usize, reply: Reply) -> Ended {
        let case = saved(number);
        let request = member(&case, "request");
        let area = PrivateDirectory::create("escalation-session-").expect("a private directory");
        let zero = area.path().join("zero");
        let mut line = PyDict::new();
        line.set("id", member(request, "id").clone());
        line.set("operation", PyValue::str("escalation_predicate.judge"));
        line.set("args", member(request, "args").clone());
        line.set("kwargs", member(request, "kwargs").clone());
        let (mut command, mut writer) = over_descriptor(
            env!("CARGO_BIN_EXE_recipe_escalation_api"),
            &["judge-session", "--input-fd", "3"],
            &StandardInput::WriteOnly(&zero),
        );
        let mut child = command.spawn().expect("start the session");
        writer
            .write_all(
                format!("{}\n", PyValue::Dict(line).dumps_ascii().expect("request")).as_bytes(),
            )
            .expect("the request");
        let mut input = Some(writer);
        let mut lines = std::io::BufReader::new(child.stdout.take().expect("stdout"));
        let (mut events, mut last) = (Vec::new(), None);
        let mut replies = Some(reply);
        loop {
            let mut text = String::new();
            if lines.read_line(&mut text).expect("a line") == 0 {
                break;
            }
            let value = loads_text(&PyStr::new(&text)).expect("a JSON line");
            let PyValue::Dict(members) = &value else {
                panic!("not an object")
            };
            if !members.contains_key("event") {
                last = Some(value);
                break;
            }
            events.push(value);
            let answer = match replies.take().expect("one event") {
                Reply::Loaded(kinds) => Some(format!(
                    "{{\"ack\": 1, \"action\": \"loaded\", \"kill_event_kinds\": {kinds}}}"
                )),
                Reply::Raise(chain) => Some(format!(
                    "{{\"ack\": 1, \"action\": \"raise\", \"error\": {chain}}}"
                )),
                Reply::Abort => Some("{\"ack\": 1, \"action\": \"abort\"}".to_string()),
                Reply::Raw(raw) => Some(raw.to_string()),
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
        Ended {
            events,
            last: last.expect("a final record"),
        }
    }

    fn kind_of(ended: &Ended) -> String {
        text_of(member(&ended.last, "kind"))
    }

    /// The final record is the service's record for the saved call: the saved original minus the collector's observations.
    fn expected(number: usize) -> PyValue {
        without(
            member(&saved(number), "original"),
            &["trace", "rpc", "globals_before", "globals_after"],
        )
    }

    #[test]
    fn the_loader_is_asked_once_at_the_original_load_point_and_its_answer_is_used() {
        // Saved case 57 (duplicates, host sign, unknowns, two executed kills): one event, the caller's tuple, the saved whole value.
        let ended = drive(57, Reply::Loaded(KINDS));
        assert_eq!(ended.events.len(), 1);
        assert!(same_value(
            &loads_text(&PyStr::new(r#"{"event": "load", "seq": 1}"#)).expect("json"),
            &ended.events[0]
        ));
        assert!(
            same_value(&expected(57), &ended.last),
            "{}",
            ended.last.repr()
        );
        // A kill hit that is not a dictionary fails after the load (saved case 65: the string's characters).
        let ended = drive(65, Reply::Loaded(KINDS));
        assert_eq!(ended.events.len(), 1);
        assert!(
            same_value(&expected(65), &ended.last),
            "{}",
            ended.last.repr()
        );
        // The caller's tuple decides what is executed: none known, nothing executed.
        let ended = drive(57, Reply::Loaded("[]"));
        let executed = member(member(&ended.last, "value"), "kill_events_executed");
        assert!(items(executed).is_empty());
    }

    #[test]
    fn a_binding_or_list_failure_ends_the_call_before_any_loader_event() {
        for number in [58usize, 59, 60, 99, 100, 101, 103, 104, 105] {
            let ended = drive(number, Reply::Abort);
            assert!(ended.events.is_empty(), "case {number}");
            assert!(
                same_value(&expected(number), &ended.last),
                "case {number}: {}",
                ended.last.repr()
            );
        }
    }

    #[test]
    fn a_loader_that_raised_is_the_calls_exception_with_its_cause_chain() {
        let binding = saved(91);
        let chain = member(&binding, "original");
        let chain_json = member(chain, "exception_chain")
            .dumps_ascii()
            .expect("json");
        let ended = drive(0, Reply::Raise(chain_json));
        assert_eq!(kind_of(&ended), "exception");
        assert_eq!(text_of(member(&ended.last, "type")), "ImportError");
        assert!(same_value(
            member(chain, "exception_chain"),
            member(&ended.last, "exception_chain")
        ));
        let binding = saved(85);
        let exit = member(&binding, "original");
        let ended = drive(
            0,
            Reply::Raise(member(exit, "exception_chain").dumps_ascii().expect("json")),
        );
        assert_eq!(text_of(member(&ended.last, "type")), "SystemExit");
        assert!(same_value(
            member(exit, "exception_chain"),
            member(&ended.last, "exception_chain")
        ));
    }

    #[test]
    fn an_abort_end_of_input_or_malformed_acknowledgement_stops_the_call() {
        for (reply, reason) in [
            (Reply::Abort, "abort"),
            (Reply::CloseInput, "eof"),
            (Reply::Raw("not json"), "malformed_ack"),
            (
                Reply::Raw("{\"ack\": 2, \"action\": \"loaded\", \"kill_event_kinds\": []}"),
                "malformed_ack",
            ),
            (
                Reply::Raw("{\"ack\": 1, \"action\": \"loaded\"}"),
                "malformed_ack",
            ),
            (
                Reply::Raw("{\"ack\": 1, \"action\": \"raise\", \"error\": []}"),
                "malformed_ack",
            ),
        ] {
            let ended = drive(57, reply);
            assert_eq!(kind_of(&ended), "aborted", "{reason}");
            assert_eq!(text_of(member(&ended.last, "reason")), reason);
            assert!(same_value(member(&ended.last, "seq"), &PyValue::int(1)));
        }
    }
}

// ── the benchmark program ────────────────────────────────────────────────────────────────────────

mod bench_cases {
    use super::*;

    fn row(bundle: &PyValue, provider: &str, mutate: bool) -> String {
        let case = &items(member(bundle, "api"))[57];
        let request = member(case, "request");
        let mut expected = without(
            member(case, "original"),
            &[
                "trace",
                "rpc",
                "globals_before",
                "globals_after",
                "id",
                "kind",
                "stdout",
                "stderr",
                "args_after",
                "kwargs_after",
            ],
        );
        let expected_value = match &mut expected {
            PyValue::Dict(map) => map.get("value").cloned().expect("a value"),
            _ => panic!("a record"),
        };
        let mut expected_value = expected_value;
        if mutate {
            if let PyValue::Dict(map) = &mut expected_value {
                map.set("predicate_1", PyValue::Bool(true));
            }
        }
        let text = |value: &PyValue| value.dumps_ascii().expect("json");
        format!(
            "[{{\"metric\": \"judge\", \"operation\": \"escalation_predicate.judge\", \"args\": {}, \"kwargs\": {}, \"provider\": {provider}, \"expected\": {}}}]",
            text(member(request, "args")),
            text(member(request, "kwargs")),
            text(&expected_value)
        )
    }

    fn run_bench(fixture: &Path, extra: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_recipe_escalation_bench"))
            .arg("--fixture")
            .arg(fixture)
            .args(extra)
            .env_clear()
            .env("CLASSIFY_CELL_API_BIN", canonical_elf())
            .stdin(Stdio::null())
            .output()
            .expect("run the benchmark")
    }

    #[test]
    fn the_benchmark_checks_whole_values_first_and_counts_launcher_runs_without_a_growing_trace() {
        let bundle = load("bundle.json");
        let area = PrivateDirectory::create("escalation-bench-").expect("a private directory");
        installed_layout(
            &bundle,
            area.path(),
            env!("CARGO_BIN_EXE_recipe_escalation_bench"),
        );
        let asset = area
            .path()
            .join("skills/adversarial-benchmark/scripts/classify_cell_native.py");
        let fixed = row(
            &bundle,
            r#"{"kind": "fixed-classifier", "kill_event_kinds": ["watchdog_kill_ack", "thermal_kill_ack", "earlyoom_kill"]}"#,
            false,
        );
        let canonical = row(
            &bundle,
            &format!(
                "{{\"kind\": \"canonical-classifier\", \"classifier\": {}}}",
                PyValue::str(asset.to_str().expect("a path"))
                    .dumps_ascii()
                    .expect("json")
            ),
            false,
        );
        for (name, fixture, runs) in [
            ("fixed", fixed, 0usize),
            ("canonical", canonical, 1 + 2 * 3),
        ] {
            let path = area.path().join(format!("{name}.json"));
            std::fs::write(&path, fixture).expect("a fixture");
            let output = run_bench(&path, &["--rounds", "2", "--iterations", "3"]);
            assert_eq!(
                output.status.code(),
                Some(0),
                "{name}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let text = String::from_utf8(output.stdout).expect("text");
            let lines: Vec<&str> = text.lines().collect();
            assert_eq!(lines.len(), 2, "{text}");
            assert_eq!(lines[0], "{\"rounds\": 2, \"iterations\": 3}");
            let report = loads_text(&PyStr::new(lines[1])).expect("a row");
            let counts = member(&report, "dependency_counts");
            let number = |scope: &str, key: &str| match member(member(counts, scope), key) {
                PyValue::Int(value) => usize::try_from(value.clone()).expect("a count"),
                other => panic!("{}", other.repr()),
            };
            assert_eq!(
                (
                    number("preflight", "launcher_runs"),
                    number("loop", "launcher_runs")
                ),
                (runs.min(1), runs - runs.min(1)),
                "{name}"
            );
            assert_eq!(number("loop", "succeeded"), number("loop", "launcher_runs"));
            assert_eq!(
                text_of(member(&report, "provider")),
                format!("{name}-classifier")
            );
        }
        // A differing whole value ends the run before any timer: no report.
        let wrong = area.path().join("wrong.json");
        std::fs::write(&wrong, row(&bundle, r#"{"kind": "fixed-classifier", "kill_event_kinds": ["watchdog_kill_ack", "thermal_kill_ack", "earlyoom_kill"]}"#, true)).expect("a fixture");
        let output = run_bench(&wrong, &["--rounds", "2", "--iterations", "3"]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr)
            .contains("differs from the saved original result"));
        let output = run_bench(&wrong, &["--rounds", "0"]);
        assert_eq!(output.status.code(), Some(2));
    }
}

// ── the two boundaries of the independent review ─────────────────────────────────────────────────

mod boundary_cases {
    use super::*;

    /// Runs the installed program with a failing owned backend: `CLASSIFY_CELL_API_BIN` is `/bin/sh`, which the launcher starts as
    /// `/bin/sh constants`, a script of the test's own in the working directory that writes `stderr_text` to standard error and exits 2.
    /// (The shell reads the script; no freshly written executable is started.)
    fn failing_backend(area: &Path, stderr_text: &str) {
        let script = format!("cat >&2 <<'END_OF_STDERR'\n{stderr_text}\nEND_OF_STDERR\nexit 2\n");
        std::fs::write(area.join("constants"), script).expect("the backend script");
    }

    fn message_after_rstrip(stderr_text: &str) -> String {
        let trimmed = stderr_text.trim_end_matches(|c: char| c.is_whitespace());
        format!("classifier native transport failed (2): {trimmed}")
    }

    /// The Unicode whitespace `str.rstrip()` removes at the end of the backend's standard error (here `char::is_whitespace` agrees with
    /// Python for the characters used, ASCII included), the leading whitespace and every non-space character kept, the same text in
    /// both blocks of the cause chain.
    #[test]
    fn the_backends_standard_error_is_right_stripped_as_python_strips_it() {
        let bundle = load("bundle.json");
        for (name, text) in [
            ("ideographic", "ERR\u{3000}\n"),
            (
                "leading kept",
                "\u{3000}lead ERR\u{2028}\u{202f}\u{2003}\u{2029}\u{205f}\u{1680}\u{200a}",
            ),
            ("ascii", "ERR \t\u{b}\u{c}"),
            ("multiline", "line1\nline2 \u{3000}\n\u{85}\u{a0}"),
            ("non-space unicode", "ERR한\u{3000}"),
        ] {
            let area = PrivateDirectory::create("escalation-rstrip-").expect("a private directory");
            installed_layout(
                &bundle,
                area.path(),
                env!("CARGO_BIN_EXE_escalation_predicate"),
            );
            let program = area.path().join(
                "skills/vllm-recipe-explorer/rust/runtime/target/release/escalation_predicate",
            );
            failing_backend(area.path(), text);
            let output = Command::new(&program)
                .args(["--symptom", "config_arch_unsupported", "--json"])
                .env_clear()
                .env("TMPDIR", area.path())
                .env("CLASSIFY_CELL_API_BIN", "/bin/sh")
                .current_dir(area.path())
                .stdin(Stdio::null())
                .output()
                .expect("run");
            let message = message_after_rstrip(text);
            let want = format!(
                "RuntimeError: {message}\n\nThe above exception was the direct cause of the following exception:\n\nImportError: {message}\n"
            );
            assert_eq!(output.status.code(), Some(1), "{name}");
            assert!(output.stdout.is_empty(), "{name}");
            assert_eq!(String::from_utf8_lossy(&output.stderr), want, "{name}");
        }
    }

    /// The same boundary through the service record: the chain rows carry the stripped text, both of them.
    #[test]
    fn the_stripped_text_is_the_same_in_the_service_records_chain() {
        let bundle = load("bundle.json");
        let area = PrivateDirectory::create("escalation-rstrip-api-").expect("a private directory");
        installed_layout(
            &bundle,
            area.path(),
            env!("CARGO_BIN_EXE_recipe_escalation_api"),
        );
        let program = area
            .path()
            .join("skills/vllm-recipe-explorer/rust/runtime/target/release/recipe_escalation_api");
        let text = "\u{3000}lead\nERR\u{3000}\u{2028}";
        failing_backend(area.path(), text);
        let zero = area.path().join("zero");
        let (mut command, mut writer) = over_descriptor(
            program.to_str().expect("a path"),
            &["evaluate-jsonl", "--input-fd", "3"],
            &StandardInput::WriteOnly(&zero),
        );
        command
            .env("CLASSIFY_CELL_API_BIN", "/bin/sh")
            .current_dir(area.path());
        writer
            .write_all(
                b"{\"id\": 1, \"operation\": \"escalation_predicate.judge\", \"args\": [[], []]}\n",
            )
            .expect("the request");
        drop(writer);
        let output = command.output().expect("run");
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let record = loads_text(&PyStr::from_surrogateescape(&output.stdout)).expect("a record");
        let message = message_after_rstrip(text);
        assert_eq!(text_of(member(&record, "type")), "ImportError");
        assert_eq!(text_of(member(&record, "message")), message);
        let chain = items(member(&record, "exception_chain"));
        assert_eq!(chain.len(), 2);
        assert_eq!(text_of(member(&chain[0], "type")), "ImportError");
        assert_eq!(text_of(member(&chain[1], "type")), "RuntimeError");
        assert!(chain
            .iter()
            .all(|row| text_of(member(row, "message")) == message));
    }
}

// ── the strict decoding of a raise acknowledgement ───────────────────────────────────────────────

mod raise_decoding_cases {
    use super::*;

    const ROW: &str = r#"{"type": "ValueError", "module": "builtins", "message": "x", "suppress_context": false, "relation": null}"#;

    fn drive_raise(chain: &str) -> PyValue {
        let area = PrivateDirectory::create("escalation-raise-").expect("a private directory");
        let zero = area.path().join("zero");
        let (mut command, mut writer) = over_descriptor(
            env!("CARGO_BIN_EXE_recipe_escalation_api"),
            &["judge-session", "--input-fd", "3"],
            &StandardInput::WriteOnly(&zero),
        );
        let mut child = command.spawn().expect("start the session");
        writer
            .write_all(b"{\"id\": \"review\", \"operation\": \"escalation_predicate.judge\", \"args\": [[], []]}\n")
            .expect("the request");
        let mut lines = std::io::BufReader::new(child.stdout.take().expect("stdout"));
        let mut event = String::new();
        lines.read_line(&mut event).expect("the load event");
        assert!(event.contains("\"event\": \"load\""), "{event}");
        writer
            .write_all(
                format!("{{\"ack\": 1, \"action\": \"raise\", \"error\": {chain}}}\n").as_bytes(),
            )
            .expect("the acknowledgement");
        drop(writer);
        let mut last = String::new();
        lines.read_line(&mut last).expect("the final record");
        let output = child.wait_with_output().expect("the session ends");
        assert_eq!(output.status.code(), Some(0));
        loads_text(&PyStr::new(&last)).expect("a record")
    }

    fn is_malformed(chain: &str) -> bool {
        let last = drive_raise(chain);
        text_of(member(&last, "kind")) == "aborted"
            && text_of(member(&last, "reason")) == "malformed_ack"
            && matches!(member(&last, "seq"), PyValue::Int(number) if *number == 1.into())
    }

    /// Every chain that is not exactly the encoding `exception_chain` writes ends the call as a malformed acknowledgement.
    #[test]
    fn a_malformed_chain_is_a_malformed_acknowledgement_never_an_ordinary_exception() {
        let wrong_type = |key: &str, value: &str| {
            ROW.replacen(
                &format!("\"{key}\": "),
                &format!("\"{key}\": {value}, \"was_{key}\": "),
                1,
            )
        };
        let cases: Vec<(&str, String)> = vec![
            (
                "suppress_context a string",
                ROW.replace(
                    "\"suppress_context\": false",
                    "\"suppress_context\": \"WRONG\"",
                ),
            ),
            (
                "suppress_context an integer",
                ROW.replace("\"suppress_context\": false", "\"suppress_context\": 0"),
            ),
            (
                "relation unknown",
                ROW.replace("\"relation\": null", "\"relation\": \"bogus\""),
            ),
            (
                "both wrong (the reviewed chain)",
                ROW.replace("false", "\"WRONG\"")
                    .replace("null", "\"bogus\""),
            ),
            (
                "module missing",
                ROW.replace("\"module\": \"builtins\", ", ""),
            ),
            ("relation missing", ROW.replace(", \"relation\": null", "")),
            ("extra member", ROW.replace("}", ", \"extra\": 1}")),
            (
                "code on a non-SystemExit",
                ROW.replace("}", ", \"code\": 2}"),
            ),
            (
                "SystemExit without code",
                ROW.replace("ValueError", "SystemExit"),
            ),
            ("type not text", ROW.replace("\"ValueError\"", "1")),
            ("message not text", ROW.replace("\"x\"", "null")),
            ("module not text", ROW.replace("\"builtins\"", "[]")),
            ("duplicate-typed keys", wrong_type("message", "\"y\"")),
        ];
        let rows = |relation: &str, suppress: &str, last_relation: &str| {
            format!(
                "[{}, {}]",
                ROW.replace(
                    "\"suppress_context\": false",
                    &format!("\"suppress_context\": {suppress}")
                )
                .replace("\"relation\": null", &format!("\"relation\": {relation}")),
                ROW.replace(
                    "\"relation\": null",
                    &format!("\"relation\": {last_relation}")
                )
            )
        };
        let mut chains: Vec<(String, String)> = cases
            .into_iter()
            .map(|(name, row)| (name.to_string(), format!("[{row}]")))
            .collect();
        for (name, chain) in [
            ("an empty list", "[]".to_string()),
            ("not a list", ROW.to_string()),
            ("a row that is not an object", "[1]".to_string()),
            ("a null chain", "null".to_string()),
            ("a link without a relation", rows("null", "false", "null")),
            (
                "a context that suppresses the context",
                rows("\"context\"", "true", "null"),
            ),
            (
                "a last row with a relation",
                rows("\"cause\"", "true", "\"cause\""),
            ),
            (
                "a good row after a bad one",
                format!(
                    "[{ROW}, {}]",
                    ROW.replace("\"relation\": null", "\"relation\": 5")
                ),
            ),
        ] {
            chains.push((name.to_string(), chain));
        }
        let wrong: Vec<&str> = chains
            .iter()
            .filter(|(_, chain)| !is_malformed(chain))
            .map(|(name, _)| name.as_str())
            .collect();
        assert!(wrong.is_empty(), "accepted: {wrong:?}");
    }

    /// The legal chains still decode, with their relations, `suppress_context` values and `code` as given (positive controls).
    #[test]
    fn a_valid_chain_is_the_calls_exception_exactly_as_encoded() {
        let bundle = load("bundle.json");
        let api = items(member(&bundle, "api"));
        let cli = items(member(&bundle, "cli"));
        let mut valid: Vec<String> = Vec::new();
        for chain in [
            member(member(&api[91], "original"), "exception_chain"), // ImportError raised from RuntimeError
            member(member(&api[85], "original"), "exception_chain"), // SystemExit with its code
            member(member(&cli[20], "original"), "exception_chain"), // SystemExit, ArgumentError, ValueError by implicit context
        ] {
            valid.push(chain.dumps_ascii().expect("json"));
        }
        // `raise ... from None`: a last row that suppresses its context is legal.
        valid.push(format!(
            "[{}]",
            ROW.replace("\"suppress_context\": false", "\"suppress_context\": true")
        ));
        // An explicit cause whose `suppress_context` was set back to false is legal too (the earlier expectation that it is invalid was
        // invented by the first decoder, not taken from the frozen original or the standard library): the flag is kept as given.
        valid.push(format!(
            "[{}, {ROW}]",
            ROW.replace("\"relation\": null", "\"relation\": \"cause\"")
        ));
        for chain in valid {
            let last = drive_raise(&chain);
            assert_eq!(text_of(member(&last, "kind")), "exception", "{chain}");
            let want = loads_text(&PyStr::new(&chain)).expect("json");
            assert!(
                same_value(&want, member(&last, "exception_chain")),
                "{chain}\n{}",
                last.repr()
            );
        }
    }
}

// ── an error class the existing classifier reports ───────────────────────────────────────────────

mod classifier_error_cases {
    use super::*;

    /// The record the existing canonical classifier answered, byte for byte, to a `kill_events_in_window` call whose single event line
    /// was 9998 nested empty arrays (one level past its JSON reader's depth limit): a `RecursionError` with this message. Provenance:
    /// the independent review's one direct call of the canonical Rust owner (`probes/recursion-class/canonical-leaf/stdout.bin`); it is
    /// replayed here by an owned stand-in backend, a script the shell reads, so no deep value is built and the classifier is not run.
    const CAPTURED_LEAF: &str = r#"{"id": "python-consumer", "encoding": "json-values-v1", "kind": "exception", "type": "RecursionError", "message": "maximum recursion depth exceeded while decoding a JSON array from a unicode string"}"#;
    const MESSAGE: &str =
        "maximum recursion depth exceeded while decoding a JSON array from a unicode string";

    /// Runs the installed command line with `--events`: the stand-in backend answers `constants` with the canonical vocabulary, `_utc`
    /// with a time and `kill_events_in_window` with `leaf`; returns the exit status and standard error.
    fn run_with_leaf(leaf: &str) -> (Option<i32>, String, Vec<u8>) {
        let bundle = load("bundle.json");
        let area = PrivateDirectory::create("escalation-leaf-").expect("a private directory");
        installed_layout(
            &bundle,
            area.path(),
            env!("CARGO_BIN_EXE_escalation_predicate"),
        );
        let program = area
            .path()
            .join("skills/vllm-recipe-explorer/rust/runtime/target/release/escalation_predicate");
        std::fs::write(
            area.path().join("constants"),
            "printf '%s\\n' '{\"KILL_EVENT_KINDS\": [\"watchdog_kill_ack\", \"thermal_kill_ack\", \"earlyoom_kill\"]}'\n",
        )
        .expect("the constants stand-in");
        let script = format!(
            "IFS= read -r line\ncase \"$line\" in\n  *'\"operation\": \"_utc\"'*) printf '%s\\n' '{{\"id\": \"python-consumer\", \"encoding\": \"json-values-v1\", \"kind\": \"value\", \"value_json\": \"{{\\\"datetime_utc\\\": \\\"2026-09-10T05:20:50+00:00\\\"}}\"}}' ;;\n  *'\"operation\": \"kill_events_in_window\"'*) printf '%s\\n' '{leaf}' ;;\nesac\n"
        );
        std::fs::write(area.path().join("evaluate-jsonl"), script).expect("the transport stand-in");
        let events = area.path().join("events.jsonl");
        std::fs::write(&events, "{}\n").expect("an event file");
        let output = Command::new(&program)
            .args(["--symptom", "serve_init_immediate_death", "--events"])
            .arg(&events)
            .args([
                "--started-utc",
                "2026-09-10T05:20:50Z",
                "--ended-utc",
                "2026-09-10T05:21:00Z",
                "--json",
            ])
            .env_clear()
            .env("TMPDIR", area.path())
            .env("CLASSIFY_CELL_API_BIN", "/bin/sh")
            .current_dir(area.path())
            .stdin(Stdio::null())
            .output()
            .expect("run");
        (
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
            output.stdout,
        )
    }

    /// The classifier's `RecursionError` keeps its class, module and message like every other builtin error it can report; a class name
    /// the module cannot build still falls back to the original's `RuntimeError("classifier native owner reported ...")`.
    #[test]
    fn a_recursion_error_reported_by_the_classifier_is_a_recursion_error() {
        let (status, stderr, stdout) = run_with_leaf(CAPTURED_LEAF);
        assert_eq!(status, Some(1));
        assert!(stdout.is_empty());
        assert_eq!(stderr, format!("RecursionError: {MESSAGE}\n"));
        let unknown = CAPTURED_LEAF.replace("RecursionError", "NotARealError");
        let (status, stderr, _) = run_with_leaf(&unknown);
        assert_eq!(status, Some(1));
        assert_eq!(
            stderr,
            format!("RuntimeError: classifier native owner reported NotARealError: {MESSAGE}\n")
        );
    }
}
