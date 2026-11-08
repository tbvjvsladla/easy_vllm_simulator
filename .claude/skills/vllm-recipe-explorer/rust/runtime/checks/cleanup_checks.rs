//! Checks of the `cleanup_docker` owner (R32). Every real program runs against an owned stand-in `docker` (a Bash script made here from the
//! saved exchange description; builtins only) that is the ONLY `docker` its `PATH` knows; nothing here (nor in the programs) reaches a
//! real Docker, daemon, network or `/proc`.
//!
//! * `reference_cases`: the 103 saved API calls and the 14 saved command lines, whole (value, exception chain, trace, streams, status, file
//!   tree after, the stand-in's counter and argument log), through the fixture programs, the production service and the production command.
//! * `contract_cases`: constants (default `REPO` from the installed layout), `globals`, refusals, argument binding, source scan.
//! * `new_cases`: behaviour the saved cases do not pin (regular expressions, `str.splitlines`, argparse forms, `%-Ns` padding, the child
//!   process: output sizes, signals, `PATH` search, the apply sequence), expectations derived from the original source and CPython 3.12.
//! * `bench_cases`: the frozen benchmark.

use agent_runtime_support::pyjson::loads_text;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use recipe_runtime::model_files::PrivateDirectory;
use recipe_runtime::observe::same_value;
use std::collections::BTreeMap;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// ── data ─────────────────────────────────────────────────────────────────────────────────────────

fn data(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("checks")
        .join("data")
        .join("cleanup_reference")
        .join(relative)
}

fn load(relative: &str) -> PyValue {
    let bytes = std::fs::read(data(relative)).expect("saved data");
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

/// Every text of the value with each marker replaced (by code point: a saved text may hold a lone surrogate).
fn rebase_all(value: &PyValue, replacements: &[(&str, String)]) -> PyValue {
    match value {
        PyValue::Str(text) => {
            let mut points: Vec<u32> = text.code_points().to_vec();
            for (marker, with) in replacements {
                let marker: Vec<u32> = marker.chars().map(u32::from).collect();
                let replacement: Vec<u32> = with.chars().map(u32::from).collect();
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
                points = out;
            }
            PyValue::Str(PyStr::from_code_points(points))
        }
        PyValue::List(entries) => PyValue::List(
            entries
                .iter()
                .map(|each| rebase_all(each, replacements))
                .collect(),
        ),
        PyValue::Dict(map) => {
            let mut copy = PyDict::new();
            for (key, each) in map.iter() {
                copy.insert(key.clone(), rebase_all(each, replacements));
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

// ── trees (the part of a case root the program can see: `node/`) ────────────────────────────────

type Tree = BTreeMap<String, (String, u32, Vec<u8>)>;

fn expected_tree(bundle: &PyValue, index: &PyValue) -> Tree {
    let at = usize::try_from(int_of(index)).expect("an index");
    items(&items(member(bundle, "trees"))[at])
        .iter()
        .map(|entry| {
            let bytes = if has(entry, "bytes_hex") {
                hex_decode(&text_of(member(entry, "bytes_hex")))
            } else {
                Vec::new()
            };
            (
                text_of(member(entry, "path")),
                (
                    text_of(member(entry, "kind")),
                    u32::try_from(int_of(member(entry, "mode"))).expect("mode"),
                    bytes,
                ),
            )
        })
        .collect()
}

/// Writes the saved tree `index` into the case root.
fn build_tree(bundle: &PyValue, index: &PyValue, root: &Path) {
    let expected = expected_tree(bundle, index);
    let mut directories: Vec<(PathBuf, u32)> = Vec::new();
    for (path, (kind, mode, bytes)) in &expected {
        let target = root.join(path);
        if kind == "dir" {
            std::fs::create_dir(&target).expect("a directory");
            directories.push((target, *mode));
        } else {
            std::fs::write(&target, bytes).expect("a file");
            std::fs::set_permissions(&target, std::fs::Permissions::from_mode(*mode))
                .expect("a mode");
        }
    }
    for (target, mode) in directories.into_iter().rev() {
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(mode)).expect("a mode");
    }
}

fn walk(root: &Path, path: &Path, rows: &mut Tree) {
    let info = std::fs::symlink_metadata(path).expect("an entry");
    let name = path
        .strip_prefix(root)
        .expect("below the root")
        .to_string_lossy()
        .into_owned();
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
        assert!(info.is_file(), "only files and directories: {name}");
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

fn snapshot_node(root: &Path) -> Tree {
    let mut rows = Tree::new();
    walk(root, &root.join("node"), &mut rows);
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

// ── the case area ────────────────────────────────────────────────────────────────────────────────

struct Area {
    _held: PrivateDirectory,
    dir: PathBuf,
    root: PathBuf,
}

/// A new private area with an empty case root (`root/private-tmp` exists, `root/node` is not made).
fn area(name: &str) -> Area {
    let held = PrivateDirectory::create(&format!("cleanup-{name}-")).expect("a private directory");
    let dir = held.path().to_path_buf();
    let root = dir.join("case");
    std::fs::create_dir(&root).expect("the case root");
    std::fs::create_dir(root.join("private-tmp")).expect("private-tmp");
    Area {
        _held: held,
        dir,
        root,
    }
}

fn root_text(area: &Area) -> String {
    area.root.to_str().expect("a path").to_string()
}

fn write_json(path: &Path, value: &PyValue) {
    std::fs::write(path, value.dumps_ascii().expect("JSON")).expect("a file");
}

fn read_control(path: &Path) -> PyValue {
    let bytes = std::fs::read(path).expect("the control record");
    loads_text(&PyStr::from_surrogateescape(&bytes)).expect("control JSON")
}

// ── the owned stand-in `docker` ──────────────────────────────────────────────────────────────────

#[derive(Clone, Default)]
struct Exchange {
    argv: Vec<String>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    code: i64,
    behaviour: Option<String>,
}

fn exchange(argv: &[&str], stdout: &[u8], code: i64) -> Exchange {
    Exchange {
        argv: argv.iter().map(|each| each.to_string()).collect(),
        stdout: stdout.to_vec(),
        code,
        ..Exchange::default()
    }
}

fn exchanges_of(process: &PyValue, marks: &[(&str, String)]) -> Vec<Exchange> {
    let process = rebase_all(process, marks);
    let PyValue::Dict(map) = &process else {
        panic!("a process description")
    };
    let Some(PyValue::List(rows)) = map.get("exchanges") else {
        return Vec::new();
    };
    rows.iter()
        .map(|row| Exchange {
            argv: items(member(row, "argv")).iter().map(text_of).collect(),
            stdout: hex_decode(&text_of(member(row, "stdout_hex"))),
            stderr: hex_decode(&text_of(member(row, "stderr_hex"))),
            code: int_of(member(row, "code")),
            behaviour: if has(row, "behaviour") {
                Some(text_of(member(row, "behaviour")))
            } else {
                None
            },
        })
        .collect()
}

extern "C" {
    fn mkfifo(path: *const std::os::raw::c_char, mode: u32) -> i32;
}

fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

fn escaped(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("\\x{byte:02x}")).collect()
}

/// The stand-in: a Bash script (builtins only) that counts its calls in a 20-digit file, logs its arguments NUL-separated, checks them
/// against the exchange of its turn and answers it (output, exit code or a signal); `repeat` cycles through the exchanges.
struct Standin {
    peer: PathBuf,
    bin: PathBuf,
    _fifo: Option<std::fs::File>,
}

fn install(area: &Area, exchanges: &[Exchange], repeat: bool) -> Standin {
    let peer = area.dir.join("peer");
    std::fs::create_dir(&peer).expect("the peer directory");
    std::fs::write(peer.join("counter"), "00000000000000000000\n").expect("counter");
    std::fs::write(peer.join("argv.bin"), b"").expect("argv log");
    let bin = area.root.join("bin");
    std::fs::create_dir(&bin).expect("bin");
    let p = |name: &str| quote(peer.join(name).to_str().expect("a path"));
    let mismatch = format!("printf \"%s\" mismatch > {}; exit 97", p("mismatch"));
    let mut script = vec![
        "#!/bin/bash".to_string(),
        "set -u".to_string(),
        format!("IFS= read -r n < {}", p("counter")),
        "n=$((10#$n))".to_string(),
        format!("printf '%020d\\n' \"$((n+1))\" > {}", p("counter")),
        format!("printf '%s\\0' \"$#\" \"$@\" >> {}", p("argv.bin")),
        if repeat {
            format!("slot=$((n%{}))", exchanges.len().max(1))
        } else {
            "slot=$n".to_string()
        },
        "case \"$slot\" in".to_string(),
    ];
    let mut fifo = None;
    for (index, row) in exchanges.iter().enumerate() {
        script.push(format!("{index})"));
        script.push(format!(
            "if [ \"$#\" -ne {} ]; then {mismatch}; fi",
            row.argv.len()
        ));
        for (at, arg) in row.argv.iter().enumerate() {
            script.push(format!(
                "if [ \"${{{}}}\" != {} ]; then {mismatch}; fi",
                at + 1,
                quote(arg)
            ));
        }
        script.push(format!("printf '%b' {}", quote(&escaped(&row.stdout))));
        script.push(format!("printf '%b' {} >&2", quote(&escaped(&row.stderr))));
        match row.behaviour.as_deref() {
            Some("read_stdin") => {
                script.push("IFS= read -r input_line || true".to_string());
                script.push("printf '%s' \"$input_line\"".to_string());
            }
            Some("stall") => {
                let path = peer.join("owned-stall");
                let c = std::ffi::CString::new(path.to_str().expect("a path")).expect("no NUL");
                // SAFETY: `c` is a valid NUL-terminated path of a file this check is creating inside its own private directory.
                assert_eq!(unsafe { mkfifo(c.as_ptr(), 0o600) }, 0, "mkfifo");
                fifo = Some(
                    std::fs::OpenOptions::new()
                        .read(true)
                        .write(true)
                        .open(&path)
                        .expect("hold the FIFO"),
                );
                script.push(format!("IFS= read -r unused < {}", p("owned-stall")));
            }
            Some(other) => panic!("unknown behaviour {other}"),
            None => {}
        }
        if row.code < 0 {
            script.push(format!("kill -s {} \"$$\"", -row.code));
            script.push("exit 98".to_string());
        } else {
            script.push(format!("exit {}", row.code));
        }
        script.push(";;".to_string());
    }
    script.push(format!("*) {mismatch} ;;"));
    script.push("esac".to_string());
    let path = bin.join("docker");
    std::fs::write(&path, format!("{}\n", script.join("\n"))).expect("the stand-in");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("mode");
    Standin {
        peer,
        bin,
        _fifo: fifo,
    }
}

struct PeerState {
    counter: Option<u64>,
    argv: Vec<Vec<String>>,
    partial: bool,
    mismatch: bool,
}

impl Standin {
    fn state(&self) -> PeerState {
        let counter_raw = std::fs::read(self.peer.join("counter")).expect("counter");
        let counter = (counter_raw.len() == 21
            && counter_raw[..20].iter().all(u8::is_ascii_digit)
            && counter_raw[20] == b'\n')
            .then(|| {
                String::from_utf8_lossy(&counter_raw[..20])
                    .parse::<u64>()
                    .expect("digits")
            });
        let raw = std::fs::read(self.peer.join("argv.bin")).expect("argv log");
        let mut fields: Vec<&[u8]> = raw.split(|byte| *byte == 0).collect();
        let terminated = raw.is_empty() || fields.last().is_some_and(|last| last.is_empty());
        if raw.is_empty() {
            fields.clear();
        } else if terminated {
            fields.pop();
        }
        let mut partial = !terminated;
        let mut argv = Vec::new();
        while !fields.is_empty() {
            let Some(count) = std::str::from_utf8(fields[0])
                .ok()
                .filter(|text| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit()))
                .and_then(|text| text.parse::<usize>().ok())
            else {
                partial = true;
                break;
            };
            if fields.len() < count + 1 {
                partial = true;
                break;
            }
            argv.push(
                fields[1..=count]
                    .iter()
                    .map(|each| String::from_utf8_lossy(each).into_owned())
                    .collect(),
            );
            fields.drain(..=count);
        }
        PeerState {
            counter,
            argv,
            partial,
            mismatch: self.peer.join("mismatch").exists(),
        }
    }

    fn calls(&self) -> Vec<Vec<String>> {
        self.state().argv
    }
}

fn peer_problems(case: &PyValue, standin: &Standin) -> Vec<String> {
    let want = member(case, "peer");
    let got = standin.state();
    let mut problems = Vec::new();
    if got.counter != Some(u64::try_from(int_of(member(want, "counter"))).expect("a count")) {
        problems.push(format!("stand-in counter {:?}", got.counter));
    }
    let want_argv: Vec<Vec<String>> = items(member(want, "argv"))
        .iter()
        .map(|call| items(call).iter().map(text_of).collect())
        .collect();
    if got.argv != want_argv {
        problems.push(format!("stand-in saw {:?} want {want_argv:?}", got.argv));
    }
    if got.partial || got.mismatch {
        problems.push(format!(
            "stand-in partial={} mismatch={}",
            got.partial, got.mismatch
        ));
    }
    problems
}

// ── running programs ─────────────────────────────────────────────────────────────────────────────

fn command(program: &str, area: &Area, path: &str) -> Command {
    let mut command = Command::new(program);
    command
        .env_clear()
        .env("PATH", path)
        .env("TMPDIR", area.root.join("private-tmp"))
        .current_dir(&area.root);
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

fn bin_path(standin: &Standin) -> String {
    standin.bin.to_str().expect("a path").to_string()
}

/// The installed-layout name a production binary is started by (`argv[0]`), so its default `REPO` is `root/node`.
fn layout_name(root: &Path, program: &str) -> String {
    format!(
        "{}/node/.claude/skills/vllm-recipe-explorer/rust/runtime/target/release/{program}",
        root.to_str().expect("a path")
    )
}

/// Starts `program args...` the way a caller passes a protocol descriptor (`pass_fds`): the pipe the caller writes to is descriptor 3 of
/// the program and descriptor 0 is the file `stdin` of the area; the umask is 022.
fn over_descriptor(
    program: &str,
    args: &[&str],
    area: &Area,
    stdin: &[u8],
    path: &str,
) -> (Command, std::io::PipeWriter) {
    let (reader, writer) = std::io::pipe().expect("a pipe");
    let zero = area.dir.join("stdin");
    std::fs::write(&zero, stdin).expect("a stdin file");
    let mut command = Command::new("/bin/sh");
    command
        .args(["-c", "umask 022; exec \"$@\" 3<&0 0<\"$F0\"", "sh", program])
        .args(args)
        .env_clear()
        .env("F0", &zero)
        .env("PATH", path)
        .env("TMPDIR", area.root.join("private-tmp"))
        .current_dir(&area.root)
        .stdin(Stdio::from(reader))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    (command, writer)
}

fn serve_lines(
    area: &Area,
    lines: &[String],
    stdin: &[u8],
    path: &str,
) -> (std::process::Output, Vec<PyValue>) {
    let (mut command, mut writer) = over_descriptor(
        env!("CARGO_BIN_EXE_recipe_cleanup_api"),
        &["evaluate-jsonl", "--input-fd", "3"],
        area,
        stdin,
        path,
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

// ── saved traceback projection: exactly the declared frames ──────────────────────────────────────

const HEADER: &[u8] = b"Traceback (most recent call last):\n";

/// The saved standard error with only the declared `(file, line, function)` frames removed with their source and caret lines and the
/// traceback headers; every other byte stays (class, message, chains, separators). A frame that is not declared is refused.
fn project_traceback(saved: &[u8], frames: &[(String, i64, String)]) -> Result<Vec<u8>, String> {
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
        let mut count = 0;
        while rest.starts_with(b"  File \"") {
            let end = rest
                .iter()
                .position(|&byte| byte == b'\n')
                .map_or(rest.len(), |at| at + 1);
            let line = String::from_utf8_lossy(&rest[..end]).into_owned();
            let declared = frames.iter().any(|(file, number, function)| {
                line.trim_end() == format!("  File \"{file}\", line {number}, in {function}")
            });
            if !declared {
                return Err(format!("a frame that is not declared: {line}"));
            }
            count += 1;
            rest = &rest[end..];
            while rest.starts_with(b"    ") {
                let end = rest
                    .iter()
                    .position(|&byte| byte == b'\n')
                    .map_or(rest.len(), |at| at + 1);
                rest = &rest[end..];
            }
        }
        if count == 0 || rest.is_empty() || rest.starts_with(b" ") {
            return Err(
                "no frame, no final block, or a final block that starts with a space".to_string(),
            );
        }
    }
}

fn declared_frames(bundle: &PyValue) -> Vec<(String, i64, String)> {
    items(member(bundle, "allowed_frames"))
        .iter()
        .map(|frame| {
            let parts = items(frame);
            (text_of(&parts[0]), int_of(&parts[1]), text_of(&parts[2]))
        })
        .collect()
}

fn status_text(output: &std::process::Output) -> Option<String> {
    output.status.code().map(|code| code.to_string())
}

// ── reference cases ──────────────────────────────────────────────────────────────────────────────

/// An encode failure names a position INSIDE a path that begins with the case root, so the saved numbers belong to the saved root's length.
/// A case that records `root_len` (the saved root's length in characters) has them moved by the difference to this run's root; the
/// text `in position N:` of the messages moves with them. Every other value is as saved.
fn positioned(case: &PyValue, want: PyValue, root: &str) -> PyValue {
    if !has(case, "root_len") {
        return want;
    }
    let delta = root.chars().count() as i64 - int_of(member(case, "root_len"));
    fn walk(value: &PyValue, delta: i64, from: &mut Option<i64>) -> PyValue {
        match value {
            PyValue::Dict(map) => {
                let encode = matches!(map.get("kind"), Some(PyValue::Str(kind)) if kind.equals("unicode_encode"));
                let mut copy = PyDict::new();
                for (key, each) in map.iter() {
                    let moved = if encode && (key.equals("start") || key.equals("end")) {
                        if key.equals("start") {
                            *from = Some(int_of(each));
                        }
                        PyValue::int(int_of(each) + delta)
                    } else {
                        walk(each, delta, from)
                    };
                    copy.insert(key.clone(), moved);
                }
                PyValue::Dict(copy)
            }
            PyValue::List(rows) => {
                PyValue::List(rows.iter().map(|row| walk(row, delta, from)).collect())
            }
            other => other.clone(),
        }
    }
    // the constructor comes after the messages in the chain: find the saved start first, then move every message
    let mut start = None;
    let moved = walk(&want, delta, &mut start);
    let Some(start) = start else { return moved };
    let (old, new) = (
        format!("in position {start}:"),
        format!("in position {}:", start + delta),
    );
    fn texts(value: &PyValue, old: &str, new: &str) -> PyValue {
        match value {
            PyValue::Str(text) => match text.to_utf8() {
                Some(plain) if plain.contains(old) => PyValue::str(&plain.replace(old, new)),
                _ => value.clone(),
            },
            PyValue::List(rows) => {
                PyValue::List(rows.iter().map(|row| texts(row, old, new)).collect())
            }
            PyValue::Dict(map) => {
                let mut copy = PyDict::new();
                for (key, each) in map.iter() {
                    copy.insert(key.clone(), texts(each, old, new));
                }
                PyValue::Dict(copy)
            }
            other => other.clone(),
        }
    }
    texts(&moved, &old, &new)
}

/// A saved case set up in a new area: the tree, the stand-in; `marks` are the root replacements.
struct Prepared {
    area: Area,
    standin: Standin,
    marks: Vec<(&'static str, String)>,
}

fn prepare(bundle: &PyValue, case: &PyValue, prefix: &str) -> Prepared {
    let request = member(case, "request");
    let id = text_of(member(request, "id"));
    let work = area(&format!("{prefix}-{id}"));
    build_tree(bundle, member(case, "before"), &work.root);
    let marks = vec![("@CASE@", root_text(&work))];
    let standin = install(
        &work,
        &exchanges_of(member(request, "process"), &marks),
        false,
    );
    Prepared {
        area: work,
        standin,
        marks,
    }
}

fn bytes_replace_all(bytes: &[u8], replacements: &[(&str, String)]) -> Vec<u8> {
    replacements
        .iter()
        .fold(bytes.to_vec(), |bytes, (marker, with)| {
            replace_bytes(&bytes, marker, with.as_bytes())
        })
}

/// Runs a saved case's program on the standard-output channel the original had: a pipe whose reader is already closed (the writer is kept
/// open by this check until the program ended) for `broken_pipe`, the ordinary captured pipe otherwise.
fn finish_for(mut run: Command, request: &PyValue, case: &PyValue) -> std::process::Output {
    if has(case, "stdout_channel") && text_of(member(case, "stdout_channel")) == "broken_pipe" {
        let (reader, writer) = std::io::pipe().expect("a pipe");
        drop(reader);
        run.stdout(Stdio::from(writer))
            .stderr(Stdio::piped())
            .stdin(Stdio::null());
        let child = run.spawn().expect("start the program");
        let output = child.wait_with_output().expect("wait for the program");
        drop(run);
        return output;
    }
    finish(run, stdin_of(request).as_deref())
}

fn stdin_of(request: &PyValue) -> Option<Vec<u8>> {
    has(request, "stdin_hex").then(|| hex_decode(&text_of(member(request, "stdin_hex"))))
}

/// The globals a saved API case runs under: `REPO` (default `root/node`, or the case's own) and `argv` (the script path, then `context.argv`).
fn case_globals(request: &PyValue, marks: &[(&str, String)]) -> PyValue {
    let context = member(request, "context");
    let root = &marks[0].1;
    let repo = if has(context, "REPO") {
        rebase_all(member(context, "REPO"), marks)
    } else {
        PyValue::str(&format!("{root}/node"))
    };
    let mut argv = vec![PyValue::str(&format!(
        "{root}/node/.claude/skills/vllm-recipe-explorer/scripts/cleanup_docker.py"
    ))];
    if has(context, "argv") {
        if let PyValue::List(more) = rebase_all(member(context, "argv"), marks) {
            argv.extend(more);
        }
    }
    let mut globals = PyDict::new();
    globals.set("REPO", repo);
    globals.set("argv", PyValue::List(argv));
    PyValue::Dict(globals)
}

mod reference_cases {
    use super::*;

    pub(super) fn fixture_problems(bundle: &PyValue, case: &PyValue, cli: bool) -> Vec<String> {
        let request = member(case, "request");
        let id = text_of(member(request, "id"));
        let work = prepare(bundle, case, "fixture");
        let (spec, control) = (
            work.area.dir.join("spec.json"),
            work.area.dir.join("control.json"),
        );
        write_json(&spec, request);
        let mut run = command(
            if cli {
                env!("CARGO_BIN_EXE_recipe_cleanup_fixture_cli")
            } else {
                env!("CARGO_BIN_EXE_recipe_cleanup_fixture_api")
            },
            &work.area,
            &bin_path(&work.standin),
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
        let output = finish_for(run, request, case);
        let mut problems = Vec::new();
        let want_status = if cli {
            int_of(member(case, "exit_code")).to_string()
        } else {
            "0".to_string()
        };
        if status_text(&output).as_deref() != Some(want_status.as_str()) {
            problems.push(format!(
                "status {:?} want {want_status}: {}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        let want_out = bytes_replace_all(
            &hex_decode(&text_of(member(case, "stdout_hex"))),
            &work.marks,
        );
        if output.stdout != want_out {
            problems.push(format!(
                "stdout differs:\n--- want\n{}\n--- got\n{}",
                String::from_utf8_lossy(&want_out),
                String::from_utf8_lossy(&output.stdout)
            ));
        }
        let saved_err = hex_decode(&text_of(member(case, "stderr_hex")));
        let want_err = if cli {
            match project_traceback(&saved_err, &declared_frames(bundle)) {
                Ok(projected) => bytes_replace_all(&projected, &work.marks),
                Err(problem) => {
                    problems.push(format!(
                        "the saved diagnostic has an unexpected shape: {problem}"
                    ));
                    Vec::new()
                }
            }
        } else {
            bytes_replace_all(&saved_err, &work.marks)
        };
        if cli && output.stderr != want_err {
            problems.push(format!(
                "stderr differs:\n--- want\n{}\n--- got\n{}",
                String::from_utf8_lossy(&want_err),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        if !cli && !output.stderr.is_empty() {
            problems.push(format!(
                "unexpected stderr {:?}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        if control.exists() {
            let got = read_control(&control);
            let want = positioned(
                case,
                rebase_all(member(case, "original"), &work.marks),
                &work.marks[0].1,
            );
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
        problems.extend(tree_difference(
            &expected_tree(bundle, member(case, "after")),
            &snapshot_node(&work.area.root),
        ));
        problems.extend(peer_problems(case, &work.standin));
        problems
            .into_iter()
            .map(|problem| format!("{id}: {problem}"))
            .collect()
    }

    fn all_or_fail(failures: Vec<String>) {
        assert!(
            failures.is_empty(),
            "{} problems:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    #[test]
    fn the_103_saved_calls_are_reproduced_whole_with_values_exceptions_the_trace_the_streams_and_the_stand_ins_log(
    ) {
        let bundle = load("bundle.json");
        let cases = items(member(&bundle, "api"));
        assert_eq!(cases.len(), 103);
        all_or_fail(
            cases
                .iter()
                .flat_map(|case| fixture_problems(&bundle, case, false))
                .collect(),
        );
    }

    #[test]
    fn the_14_saved_command_lines_end_as_the_original_did_with_their_trace_streams_status_files_and_stand_in_log(
    ) {
        let bundle = load("bundle.json");
        let cases = items(member(&bundle, "cli"));
        assert_eq!(cases.len(), 14);
        all_or_fail(
            cases
                .iter()
                .flat_map(|case| fixture_problems(&bundle, case, true))
                .collect(),
        );
    }

    pub(super) fn service_problems(bundle: &PyValue, case: &PyValue) -> Vec<String> {
        let request = member(case, "request");
        let id = text_of(member(request, "id"));
        let work = prepare(bundle, case, "service");
        let mut line = PyDict::new();
        line.set("id", member(request, "id").clone());
        line.set(
            "operation",
            PyValue::str(&format!(
                "cleanup_docker.{}",
                text_of(member(request, "function"))
            )),
        );
        line.set("args", rebase_all(member(request, "args"), &work.marks));
        line.set("kwargs", rebase_all(member(request, "kwargs"), &work.marks));
        line.set("globals", case_globals(request, &work.marks));
        let (mut command, mut writer) = over_descriptor(
            env!("CARGO_BIN_EXE_recipe_cleanup_api"),
            &["evaluate-jsonl", "--input-fd", "3"],
            &work.area,
            &stdin_of(request).unwrap_or_default(),
            &bin_path(&work.standin),
        );
        let running = command.spawn().expect("start the service");
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
        let output = running.wait_with_output().expect("wait");
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
                let want = positioned(
                    case,
                    rebase_all(
                        &without(
                            member(case, "original"),
                            &["owner", "trace", "globals_before", "globals_after"],
                        ),
                        &work.marks,
                    ),
                    &work.marks[0].1,
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
        problems.extend(peer_problems(case, &work.standin));
        problems
            .into_iter()
            .map(|problem| format!("{id}: {problem}"))
            .collect()
    }

    #[test]
    fn the_103_saved_calls_agree_through_the_production_service() {
        let bundle = load("bundle.json");
        all_or_fail(
            items(member(&bundle, "api"))
                .iter()
                .flat_map(|case| service_problems(&bundle, case))
                .collect(),
        );
    }

    pub(super) fn program_problems(bundle: &PyValue, case: &PyValue) -> Vec<String> {
        let request = member(case, "request");
        let id = text_of(member(request, "id"));
        let work = prepare(bundle, case, "program");
        let args: Vec<String> = items(member(request, "args"))
            .iter()
            .map(|arg| text_of(&rebase_all(arg, &work.marks)))
            .collect();
        // The binary is started by its installed-layout name: its default REPO is `root/node`, as the original's `__file__` made it.
        let mut run = command(
            env!("CARGO_BIN_EXE_cleanup_docker"),
            &work.area,
            &bin_path(&work.standin),
        );
        run.arg0(layout_name(&work.area.root, "cleanup_docker"));
        run.args(&args);
        let output = finish_for(run, request, case);
        let mut problems = Vec::new();
        let want_status = int_of(member(case, "exit_code")).to_string();
        if status_text(&output).as_deref() != Some(want_status.as_str()) {
            problems.push(format!(
                "status {:?} want {want_status}",
                output.status.code()
            ));
        }
        if output.stdout
            != bytes_replace_all(
                &hex_decode(&text_of(member(case, "stdout_hex"))),
                &work.marks,
            )
        {
            problems.push("stdout differs".to_string());
        }
        let projected = project_traceback(
            &hex_decode(&text_of(member(case, "stderr_hex"))),
            &declared_frames(bundle),
        )
        .expect("a declared shape");
        if output.stderr != bytes_replace_all(&projected, &work.marks) {
            problems.push(format!(
                "stderr differs: {:?}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        problems.extend(tree_difference(
            &expected_tree(bundle, member(case, "after")),
            &snapshot_node(&work.area.root),
        ));
        problems.extend(peer_problems(case, &work.standin));
        problems
            .into_iter()
            .map(|problem| format!("{id}: {problem}"))
            .collect()
    }

    #[test]
    fn the_14_saved_command_lines_give_the_same_streams_status_and_requests_through_the_production_program(
    ) {
        let bundle = load("bundle.json");
        all_or_fail(
            items(member(&bundle, "cli"))
                .iter()
                .flat_map(|case| program_problems(&bundle, case))
                .collect(),
        );
    }
}

// ── a small laboratory for the checks that are not saved cases ───────────────────────────────────

struct Lab {
    area: Area,
    standin: Standin,
}

fn put(root: &Path, relative: &str, bytes: &[u8]) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("directories");
    std::fs::write(path, bytes).expect("a file");
}

const RESOLUTION: &str =
    "node/.claude/skills/upstream-version-watch/assets/current-production-resolution.json";
const TEMPLATE: &str = "node/.claude/skills/upstream-version-watch/templates/Dockerfile.template";
const SOURCE_TEMPLATE: &str =
    "node/.claude/skills/upstream-version-watch/templates/Dockerfile.source-build.template";
const BASE_REF: &str = "nvcr.io/nvidia/pytorch:26.01-py3";

/// The three files `preserve_set` needs under any `REPO`: one NGC tag and the two templates, each with one `FROM`.
fn canonical(root: &Path) {
    put(root, RESOLUTION, br#"{"ngc_base": {"tag": "26.01-py3"}}"#);
    put(root, TEMPLATE, format!("FROM {BASE_REF}\n").as_bytes());
    put(
        root,
        SOURCE_TEMPLATE,
        format!("FROM {BASE_REF}\n").as_bytes(),
    );
}

fn lab(name: &str, exchanges: &[Exchange], repeat: bool) -> Lab {
    let work = area(name);
    let standin = install(&work, exchanges, repeat);
    canonical(&work.root);
    Lab {
        area: work,
        standin,
    }
}

/// A stand-in that answers every `docker ps` with failure: no containers are known.
fn no_containers() -> Vec<Exchange> {
    vec![exchange(&["ps", "-a", "--format", "{{.Image}}"], b"", 1)]
}

fn globals_text(lab: &Lab) -> String {
    format!(
        "{{\"REPO\": {}, \"argv\": [\"cleanup_docker.py\"]}}",
        jstr(&format!("{}/node", root_text(&lab.area)))
    )
}

fn request(id: i64, operation: &str, args: &str, kwargs: &str, globals: &str) -> String {
    format!(
        "{{\"id\": {id}, \"operation\": \"cleanup_docker.{operation}\", \"args\": {args}, \"kwargs\": {kwargs}, \"globals\": {globals}}}"
    )
}

fn serve(lab: &Lab, lines: Vec<String>) -> Vec<PyValue> {
    let (output, records) = serve_lines(&lab.area, &lines, b"", &bin_path(&lab.standin));
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(records.len(), lines.len());
    records
}

fn call1(lab: &Lab, operation: &str, args: &str, kwargs: &str) -> PyValue {
    serve(
        lab,
        vec![request(1, operation, args, kwargs, &globals_text(lab))],
    )
    .remove(0)
}

fn kind_of(record: &PyValue) -> String {
    text_of(member(record, "kind"))
}

fn value_of(record: &PyValue) -> &PyValue {
    assert_eq!(kind_of(record), "value", "{}", record.repr());
    member(record, "value")
}

fn exception_of(record: &PyValue) -> (String, String) {
    assert_eq!(kind_of(record), "exception", "{}", record.repr());
    (
        text_of(member(record, "type")),
        text_of(member(record, "message")),
    )
}

fn keep_of(record: &PyValue) -> Vec<String> {
    let value = value_of(record);
    assert_eq!(text_of(member(value, "kind")), "preserve_tuple");
    items(member(value, "keep")).iter().map(text_of).collect()
}

fn why_of(record: &PyValue, reference: &str) -> String {
    text_of(member(member(value_of(record), "why"), reference))
}

fn sorted_set(refs: &[&str]) -> Vec<String> {
    let mut all: Vec<String> = refs.iter().map(|each| each.to_string()).collect();
    all.sort_by(|a, b| {
        a.chars()
            .map(u32::from)
            .collect::<Vec<_>>()
            .cmp(&b.chars().map(u32::from).collect::<Vec<_>>())
    });
    all
}

fn run_value(record: &PyValue) -> (i64, String) {
    let value = value_of(record);
    assert_eq!(text_of(member(value, "kind")), "run_tuple");
    let pair = items(member(value, "items"));
    (int_of(&pair[0]), text_of(&pair[1]))
}

// ── the service's own contract ───────────────────────────────────────────────────────────────────

mod contract_cases {
    use super::*;

    /// A production program started by an installed-layout name (`argv[0]`); `stdin` is its protocol input.
    fn started_by(
        program: &str,
        name: &str,
        args: &[&str],
        directory: &Path,
        stdin: &[u8],
    ) -> std::process::Output {
        let mut run = Command::new(program);
        run.arg0(name)
            .args(args)
            .env_clear()
            .current_dir(directory)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = run.spawn().expect("start");
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(stdin)
            .expect("input");
        child.wait_with_output().expect("wait")
    }

    #[test]
    fn the_default_repo_comes_from_the_installed_layout_of_the_binary_and_nothing_else() {
        let work = area("layout");
        let program = env!("CARGO_BIN_EXE_recipe_cleanup_api");
        let tail =
            ".claude/skills/vllm-recipe-explorer/rust/runtime/target/release/recipe_cleanup_api";
        let want = |repo: &str| format!("{{\"cleanup_docker\": {{\"REPO\": {}}}}}\n", jstr(repo));
        for (name, repo) in [
            (format!("/x/y/z/{tail}"), "/x/y/z".to_string()),
            (format!("/x/y/w/../z/{tail}"), "/x/y/z".to_string()),
            (format!("/x//y/./z/{tail}"), "/x/y/z".to_string()),
            (
                format!("./a/{tail}"),
                format!("{}/a", work.dir.to_str().expect("a path")),
            ),
        ] {
            let output = started_by(program, &name, &["constants"], &work.dir, b"");
            assert_eq!(output.status.code(), Some(0), "{name}");
            assert_eq!(
                String::from_utf8_lossy(&output.stdout),
                want(&repo),
                "{name}"
            );
        }
        // The same default is the `REPO` of a request without `globals`, and the working directory plays no part in it.
        canonical(&work.root);
        let standin = install(&work, &no_containers(), true);
        let layout = layout_name(&work.root, "recipe_cleanup_api");
        let line = "{\"id\": 1, \"operation\": \"cleanup_docker.preserve_set\", \"args\": [], \"kwargs\": {}}\n";
        let mut run = command(program, &work, &bin_path(&standin));
        run.arg0(&layout).arg("evaluate-jsonl").current_dir("/");
        let output = finish(run, Some(line.as_bytes()));
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let record = loads_text(&PyStr::new(
            String::from_utf8_lossy(&output.stdout).trim_end(),
        ))
        .expect("a record");
        assert_eq!(keep_of(&record), [BASE_REF]);
        assert_eq!(
            why_of(&record, BASE_REF),
            "shared production resolution (default)"
        );
    }

    #[test]
    fn the_globals_member_the_binding_and_the_refusals_are_as_documented() {
        let work = lab("contract", &no_containers(), true);
        let globals = globals_text(&work);
        let lines = vec![
            r#"{"id": 1, "operation": "cleanup_docker.list_images", "args": [], "context": {}}"#
                .to_string(),
            request(2, "list_images", "[]", "{}", r#"{"other": 1}"#),
            request(3, "other", "[]", "{}", &globals),
            r#"{"id": 4, "operation": "other.main", "args": []}"#.to_string(),
            request(5, "_run", "[]", "{}", &globals),
            request(6, "list_images", "[1]", "{}", &globals),
            request(7, "preserve_set", "[]", r#"{"x": 1}"#, &globals),
            request(8, "main", "[1, 2, 3]", "{}", &globals),
            request(9, "preserve_set", "[]", "{}", r#"{"argv": "x"}"#),
            request(10, "preserve_set", "[]", "{}", r#"{"argv": ["a", 1]}"#),
            request(11, "preserve_set", "[]", "{}", "[]"),
            r#"{"id": 12, "operation": "cleanup_docker", "args": []}"#.to_string(),
            request(13, "_run", r#"[["docker"]]"#, r#"{"cmd": 1}"#, &globals),
            request(
                14,
                "_run",
                "[]",
                r#"{"cmd": ["docker"], "timeout": null}"#,
                &globals,
            ),
        ];
        let got = serve(&work, lines);
        assert!(text_of(member(&got[0], "message"))
            .starts_with("the request has a member this service does not take: "));
        for index in [1, 8, 9, 10] {
            assert_eq!(exception_of(&got[index]).0, "RequestError", "{index}");
        }
        assert_eq!(
            exception_of(&got[2]),
            (
                "AttributeError".to_string(),
                "module 'cleanup_docker' has no attribute 'other'".to_string()
            )
        );
        assert_eq!(
            exception_of(&got[3]),
            (
                "ModuleNotFoundError".to_string(),
                "No module named 'other'".to_string()
            )
        );
        for (index, message) in [
            (4, "_run() missing 1 required positional argument: 'cmd'"),
            (
                5,
                "list_images() takes 0 positional arguments but 1 was given",
            ),
            (6, "preserve_set() got an unexpected keyword argument 'x'"),
            (
                7,
                "main() takes from 0 to 1 positional arguments but 3 were given",
            ),
            (12, "_run() got multiple values for argument 'cmd'"),
        ] {
            let (class, text) = exception_of(&got[index]);
            assert_eq!(
                (class.as_str(), text.as_str()),
                ("TypeError", message),
                "{index}"
            );
        }
        assert_eq!(
            exception_of(&got[11]),
            (
                "AttributeError".to_string(),
                "module 'cleanup_docker' has no attribute ''".to_string()
            )
        );
        // `timeout=None` waits without a deadline; the stand-in has no exchange for a bare `docker`, so it answers 97 (mismatch) and exits.
        assert_eq!(run_value(&got[13]).0, 97);
        // Arguments are never changed by a call.
        let after = member(&got[4], "args_after");
        assert!(same_value(after, &j("[]")));
    }

    /// Reading the source: no provider or case lookup, no fixture module, no environment, no operational path, no Python, no real Docker
    /// socket, and the one place that starts a child.
    #[test]
    fn the_sources_have_no_selector_no_fixture_module_no_operational_path_and_one_spawn_site() {
        let sources = [
            (
                "cleanup_process.rs",
                include_str!("../src/cleanup_process.rs"),
            ),
            (
                "cleanup_events.rs",
                include_str!("../src/cleanup_events.rs"),
            ),
            (
                "cleanup_docker.rs",
                include_str!("../src/cleanup_docker.rs"),
            ),
            ("cleanup_cli.rs", include_str!("../src/cleanup_cli.rs")),
            ("cleanup_api.rs", include_str!("../src/cleanup_api.rs")),
            ("cleanup_bench.rs", include_str!("../src/cleanup_bench.rs")),
            (
                "bin/cleanup_docker.rs",
                include_str!("../src/bin/cleanup_docker.rs"),
            ),
            (
                "bin/recipe_cleanup_api.rs",
                include_str!("../src/bin/recipe_cleanup_api.rs"),
            ),
            (
                "bin/recipe_cleanup_bench.rs",
                include_str!("../src/bin/recipe_cleanup_bench.rs"),
            ),
        ];
        let mut found = Vec::new();
        for (name, text) in sources {
            for forbidden in [
                "env::var",
                "getenv",
                "set_var",
                "remove_var",
                "cleanup_fixture",
                "real-worker",
                "val_agent",
                "CARGO_MANIFEST_DIR",
                "BoundaryViolation",
                "/usr/bin/python",
                "python3",
                "/usr/bin/docker",
                "docker.sock",
                "DOCKER_HOST",
                "\"api-",
                "\"cli-",
                "@CASE@",
                "expected_lookup",
            ] {
                // The bins read the terminal width once (`COLUMNS`); the process runner reads the child's search path once (`PATH`, as
                // `os.get_exec_path` does); the original's own hint text names `python3`.
                let allowed = (forbidden == "env::var"
                    && name.starts_with("bin/")
                    && text.matches("env::var").count() == 1
                    && text.contains("\"COLUMNS\""))
                    || (forbidden == "env::var"
                        && name == "cleanup_process.rs"
                        && text.matches("env::var").count() == 1
                        && text.contains("env::var_os(\"PATH\")"))
                    || (forbidden == "python3"
                        && name == "cleanup_docker.rs"
                        && text.matches("python3").count() == 1
                        && text.contains("DRY-RUN 종료 — 표 검토 후: python3 .claude/skills/vllm-recipe-explorer/scripts/cleanup_docker.py"));
                if text.contains(forbidden) && !allowed {
                    found.push(format!("{name}: {forbidden}"));
                }
            }
            if name != "cleanup_process.rs"
                && (text.contains("Command::new") || text.contains("process::Command"))
            {
                found.push(format!(
                    "{name}: starts a process outside cleanup_process.rs"
                ));
            }
        }
        assert!(found.is_empty(), "{}", found.join("\n"));
        let spawns = include_str!("../src/cleanup_process.rs")
            .matches("Command::new")
            .count();
        assert_eq!(spawns, 1, "one spawn site");
    }
}

// ── new checks: behaviour the saved calls do not pin (expectations from the original source and CPython 3.12) ─────────────────────

mod new_cases {
    use super::*;

    fn sorted_keep(extra: &[&str]) -> Vec<String> {
        let mut all = vec![BASE_REF];
        all.extend_from_slice(extra);
        sorted_set(&all)
    }

    #[test]
    fn image_tag_lines_are_found_as_a_multiline_regular_expression_finds_them_in_text_with_universal_newlines(
    ) {
        let work = lab("tags", &no_containers(), true);
        let envs = "node/output/single/envs";
        let text = "IMAGE_TAG=v1\nIMAGE_TAG=reg/x:y\nXIMAGE_TAG=no\n IMAGE_TAG=no2\nIMAGE_TAG=\nIMAGE_TAG=a b\n\
                    IMAGE_TAG=\u{2003}z\nIMAGE_TAG=t1\u{a0}t2\r\nIMAGE_TAG=cr\rIMAGE_TAG=crr\nIMAGE_TAG=:\nIMAGE_TAG=/\n\
                    IMAGE_TAG=q\u{1c}r\nIMAGE_TAG=\u{85}z\nIMAGE_TAG=w\u{200b}x\nimage_tag=lower\nIMAGE_TAG=last";
        put(&work.area.root, &format!("{envs}/.env.a"), text.as_bytes());
        put(
            &work.area.root,
            &format!("{envs}/.envrc"),
            b"IMAGE_TAG=rc\n",
        );
        put(
            &work.area.root,
            &format!("{envs}/not.env"),
            b"IMAGE_TAG=no\n",
        );
        put(&work.area.root, &format!("{envs}/env.x"), b"IMAGE_TAG=no\n");
        put(
            &work.area.root,
            &format!("{envs}/.env.bad"),
            b"IMAGE_TAG=a\xffb\n",
        );
        std::fs::create_dir(work.area.root.join(envs).join(".envdir")).expect("a directory");
        let record = call1(&work, "preserve_set", "[]", "{}");
        assert_eq!(
            keep_of(&record),
            sorted_keep(&[
                "easy-vllm:v1",
                "reg/x:y",
                "easy-vllm:a",
                "easy-vllm:t1",
                "easy-vllm:cr",
                "easy-vllm:crr",
                ":",
                "/",
                "easy-vllm:q",
                "easy-vllm:w\u{200b}x",
                "easy-vllm:last",
                "easy-vllm:rc",
                "easy-vllm:a\u{fffd}b",
            ])
        );
        assert_eq!(why_of(&record, "easy-vllm:v1"), "env single/.env.a");
        assert_eq!(why_of(&record, "easy-vllm:rc"), "env single/.envrc");
        assert_eq!(
            why_of(&record, "easy-vllm:a\u{fffd}b"),
            "env single/.env.bad"
        );
        assert_eq!(
            why_of(&record, BASE_REF),
            "shared production resolution (default)"
        );
    }

    #[test]
    fn from_instructions_are_found_across_line_breaks_and_unicode_space_and_unresolved_ones_are_ignored(
    ) {
        let work = lab("from", &no_containers(), true);
        let template = "FROM\n  nvcr.io/nvidia/pytorch:A AS base\nFROM $BASE\nFROM x{{y}}\nfrom lower\n FROM indented\n\
                        FROMage cheese\nFROM  \t spaced:ref\nRUN echo\nFROM\r\nwin:ref\nFROM\u{a0}nb:ref\nFROM ${BASE}:t\n\
                        FROM a$b\nFROM end:ref";
        put(&work.area.root, TEMPLATE, template.as_bytes());
        let record = call1(&work, "preserve_set", "[]", "{}");
        assert_eq!(
            keep_of(&record),
            sorted_keep(&[
                "nvcr.io/nvidia/pytorch:A",
                "spaced:ref",
                "win:ref",
                "nb:ref",
                "a$b",
                "end:ref"
            ])
        );
        assert_eq!(
            why_of(&record, "spaced:ref"),
            "FROM shared/Dockerfile.template"
        );
        assert_eq!(
            why_of(&record, BASE_REF),
            "shared production resolution (default)"
        );
        // A template whose only FROM is unresolved still has one (the search succeeds) and adds nothing.
        put(&work.area.root, TEMPLATE, b"FROM {{X}}\n");
        assert_eq!(
            keep_of(&call1(&work, "preserve_set", "[]", "{}")),
            sorted_keep(&[])
        );
    }

    #[test]
    fn a_defective_canonical_template_is_a_runtime_error_with_its_cause_and_the_constructor_fields()
    {
        let cases: Vec<(&str, Vec<u8>, String)> = vec![
            ("empty", Vec::new(), "missing FROM instruction".to_string()),
            (
                "blank",
                b"   \n\t\n".to_vec(),
                "missing FROM instruction".to_string(),
            ),
            (
                "bare",
                b"FROM\n".to_vec(),
                "missing FROM instruction".to_string(),
            ),
            (
                "none",
                b"RUN x\n".to_vec(),
                "missing FROM instruction".to_string(),
            ),
            (
                "bytes",
                b"FROM \xff".to_vec(),
                "'utf-8' codec can't decode byte 0xff in position 5: invalid start byte"
                    .to_string(),
            ),
        ];
        for (name, bytes, message) in cases {
            let work = lab(&format!("template-{name}"), &no_containers(), true);
            put(&work.area.root, TEMPLATE, &bytes);
            let record = call1(&work, "preserve_set", "[]", "{}");
            let (class, text) = exception_of(&record);
            assert_eq!(class, "RuntimeError", "{name}");
            assert_eq!(
                text,
                format!(
                    "canonical shared template invalid (shared/Dockerfile.template): {message}"
                ),
                "{name}"
            );
            let chain = items(member(&record, "exception_chain"));
            if name == "bytes" {
                assert_eq!(chain.len(), 2);
                assert_eq!(text_of(member(&chain[0], "relation")), "cause");
                assert!(matches!(
                    member(&chain[0], "suppress_context"),
                    PyValue::Bool(true)
                ));
                assert_eq!(text_of(member(&chain[1], "type")), "UnicodeDecodeError");
                assert!(same_value(
                    member(&chain[1], "constructor"),
                    &j(
                        r#"{"kind": "unicode_decode", "encoding": "utf-8", "object_hex": "46524f4d20ff", "start": 5, "end": 6, "reason": "invalid start byte"}"#
                    )
                ));
            } else {
                assert_eq!(chain.len(), 1, "{name}");
            }
        }
        // A directory in place of the second template, and a missing one: wrapped OSErrors named with the label and the path.
        let work = lab("template-os", &no_containers(), true);
        std::fs::remove_file(work.area.root.join(SOURCE_TEMPLATE)).expect("remove");
        let record = call1(&work, "preserve_set", "[]", "{}");
        let path = format!("{}/{SOURCE_TEMPLATE}", root_text(&work.area));
        assert_eq!(
            exception_of(&record).1,
            format!("canonical shared template invalid (shared/Dockerfile.source-build.template): [Errno 2] No such file or directory: '{path}'")
        );
        std::fs::create_dir(work.area.root.join(SOURCE_TEMPLATE)).expect("a directory");
        let record = call1(&work, "preserve_set", "[]", "{}");
        assert_eq!(
            exception_of(&record).1,
            format!("canonical shared template invalid (shared/Dockerfile.source-build.template): [Errno 21] Is a directory: '{path}'")
        );
    }

    #[test]
    fn resolved_json_strings_are_found_in_the_ascii_dump_by_the_original_regular_expression() {
        let work = lab("resolved", &no_containers(), true);
        put(
            &work.area.root,
            "node/resolved.json",
            br#"{"k": ["easy-vllm:a", "nvcr.io/nvidia/pytorch:b", "easy-vllm:caf\u00e9", "easy-vllm:q\"x", "xeasy-vllm:no", {"easy-vllm:key": 1}, "nvcr.io/nvidia/pytorch", "easy-vllm:pad  "], "easy-vllm:dup": "easy-vllm:a", "n": null, "t": "easy-vllm:\ud83d\ude00"}"#,
        );
        let record = call1(&work, "preserve_set", "[]", "{}");
        let want = [
            "easy-vllm:a",
            "nvcr.io/nvidia/pytorch:b",
            "easy-vllm:caf\\u00e9",
            "easy-vllm:q\\",
            "easy-vllm:key",
            "nvcr.io/nvidia/pytorch",
            "easy-vllm:pad",
            "easy-vllm:dup",
            "easy-vllm:\\ud83d\\ude00",
        ];
        assert_eq!(keep_of(&record), sorted_keep(&want));
        // `why` keeps insertion order: the file's strings first (a string seen again keeps its first place), then the resolution.
        let order: Vec<String> = match member(value_of(&record), "why") {
            PyValue::Dict(map) => map
                .iter()
                .map(|(key, _)| key.to_utf8().expect("text"))
                .collect(),
            other => panic!("{}", other.repr()),
        };
        let mut expected: Vec<String> = want.iter().map(|each| each.to_string()).collect();
        expected.push(BASE_REF.to_string());
        assert_eq!(order, expected);
        assert_eq!(why_of(&record, "easy-vllm:key"), "resolved.json");
    }

    #[test]
    fn canonical_only_is_a_truthiness_test_and_leaves_only_the_canonical_files() {
        let work = lab("canonical-only", &no_containers(), true);
        put(
            &work.area.root,
            "node/output/single/envs/.env.z",
            b"IMAGE_TAG=zzz\n",
        );
        put(
            &work.area.root,
            "node/resolved.json",
            br#"["easy-vllm:rjson"]"#,
        );
        put(
            &work.area.root,
            "node/output/multi/Dockerfile",
            b"FROM df:ref\n",
        );
        let all = sorted_keep(&["easy-vllm:zzz", "easy-vllm:rjson", "df:ref"]);
        for falsy in ["false", "0", "\"\"", "null", "[]", "{}", "0.0"] {
            assert_eq!(
                keep_of(&call1(&work, "preserve_set", &format!("[{falsy}]"), "{}")),
                all,
                "{falsy}"
            );
        }
        for truthy in ["true", "1", "\"x\"", "[0]", "{\"a\": 1}", "1.5", "-1"] {
            assert_eq!(
                keep_of(&call1(&work, "preserve_set", &format!("[{truthy}]"), "{}")),
                sorted_keep(&[]),
                "{truthy}"
            );
        }
        assert_eq!(
            keep_of(&call1(
                &work,
                "preserve_set",
                "[]",
                "{\"canonical_only\": true}"
            )),
            sorted_keep(&[])
        );
    }

    fn nested(depth: usize, leaf: &str) -> String {
        format!("{}{leaf}{}", "[".repeat(depth), "]".repeat(depth))
    }

    #[test]
    fn json_nested_beyond_the_recursion_limit_is_skipped_in_resolved_json_and_unwrapped_in_the_resolution(
    ) {
        let work = lab("deep", &no_containers(), true);
        put(
            &work.area.root,
            "node/resolved.json",
            nested(100, "\"easy-vllm:deep\"").as_bytes(),
        );
        assert_eq!(
            keep_of(&call1(&work, "preserve_set", "[]", "{}")),
            sorted_keep(&["easy-vllm:deep"])
        );
        // The decoder accepts nesting up to the C recursion limit (9 997 levels measured on CPython 3.12.3).
        put(
            &work.area.root,
            "node/resolved.json",
            nested(9000, "\"easy-vllm:deep\"").as_bytes(),
        );
        assert_eq!(
            keep_of(&call1(&work, "preserve_set", "[]", "{}")),
            sorted_keep(&["easy-vllm:deep"])
        );
        put(
            &work.area.root,
            "node/resolved.json",
            nested(20_000, "\"easy-vllm:deep\"").as_bytes(),
        );
        assert_eq!(
            keep_of(&call1(&work, "preserve_set", "[]", "{}")),
            sorted_keep(&[])
        );
        // The resolution is mandatory: the recursion error is not one of the wrapped classes.
        put(&work.area.root, RESOLUTION, nested(20_000, "1").as_bytes());
        let (class, message) = exception_of(&call1(&work, "preserve_set", "[]", "{}"));
        assert_eq!(class, "RecursionError");
        assert_eq!(
            message,
            "maximum recursion depth exceeded while decoding a JSON array from a unicode string"
        );
    }

    #[test]
    fn dockerfiles_the_unreadable_text_and_the_order_of_the_reasons_follow_the_original() {
        let work = lab("order", &no_containers(), true);
        put(
            &work.area.root,
            "node/output/multi/envs/.env.m",
            b"IMAGE_TAG=mm\n",
        );
        put(
            &work.area.root,
            "node/output/single/envs/.env.s",
            b"IMAGE_TAG=ss\n",
        );
        put(
            &work.area.root,
            "node/output/multi/Dockerfile",
            b"FROM bad\xff:x\nFROM ok:1\n",
        );
        std::fs::create_dir(
            work.area
                .root
                .join("node/output/single/Dockerfile.source-build"),
        )
        .expect("dir");
        let record = call1(&work, "preserve_set", "[]", "{}");
        let order: Vec<String> = match member(value_of(&record), "why") {
            PyValue::Dict(map) => map
                .iter()
                .map(|(key, _)| key.to_utf8().expect("text"))
                .collect(),
            other => panic!("{}", other.repr()),
        };
        assert_eq!(
            order,
            [
                "easy-vllm:mm",
                "easy-vllm:ss",
                BASE_REF,
                "bad\u{fffd}:x",
                "ok:1"
            ]
        );
        assert_eq!(why_of(&record, "easy-vllm:mm"), "env multi/.env.m");
        assert_eq!(why_of(&record, "ok:1"), "FROM multi/Dockerfile");
    }

    #[test]
    fn container_images_are_split_as_str_splitlines_splits_and_stripped_as_str_strip_strips() {
        let ps = exchange(
            &["ps", "-a", "--format", "{{.Image}}"],
            b"img:a\n  \n\xc2\xa0b:c\xc2\xa0\r\n\x0bd:e\x1cf:g\xe2\x80\xa8h:i",
            0,
        );
        let work = lab("ps", &[ps], true);
        let record = call1(&work, "preserve_set", "[]", "{}");
        assert_eq!(
            keep_of(&record),
            sorted_keep(&["img:a", "b:c", "d:e", "f:g", "h:i"])
        );
        assert_eq!(why_of(&record, "b:c"), "container in use");
    }

    #[test]
    fn list_images_reads_one_json_value_per_line_split_as_str_splitlines_splits() {
        let ls = exchange(
            &["image", "ls", "-a", "--format", "{{json .}}"],
            b"{\"a\": 1}\x0b{\"a\": 2}\r\n\r{\"b\": 3}\xe2\x80\xa8[1]\n  \n\"str\"\x1c7\x1d8\x1e\xc2\x859\x0c\xc2\xa0\n\xe3\x80\x80\n\x1f\n\"tail\"",
            0,
        );
        let work = lab("list", &[ls], true);
        let record = call1(&work, "list_images", "[]", "{}");
        let value = value_of(&record);
        assert!(same_value(
            value,
            &j(
                r#"{"kind": "json", "data": [{"a": 1}, {"a": 2}, {"b": 3}, [1], "str", 7, 8, 9, "tail"]}"#
            )
        ));
    }

    #[test]
    fn a_line_that_is_not_json_is_the_decoder_error_with_its_constructor_and_a_failed_listing_exits_with_2(
    ) {
        let ls = exchange(
            &["image", "ls", "-a", "--format", "{{json .}}"],
            b"  {\"ok\": 1}\n  bad line\n",
            0,
        );
        let work = lab("badline", &[ls], true);
        let record = call1(&work, "list_images", "[]", "{}");
        assert_eq!(
            exception_of(&record),
            (
                "JSONDecodeError".to_string(),
                "Expecting value: line 1 column 3 (char 2)".to_string()
            )
        );
        let row = &items(member(&record, "exception_chain"))[0];
        assert!(matches!(
            member(row, "suppress_context"),
            PyValue::Bool(true)
        ));
        assert!(same_value(
            member(row, "constructor"),
            &j(
                r#"{"kind": "json_decode", "msg": "Expecting value", "doc": "  bad line", "pos": 2}"#
            )
        ));
        let failing = lab(
            "failed-listing",
            &[exchange(
                &["image", "ls", "-a", "--format", "{{json .}}"],
                b" bad news \n",
                1,
            )],
            true,
        );
        let record = call1(&failing, "list_images", "[]", "{}");
        assert_eq!(
            exception_of(&record),
            ("SystemExit".to_string(), "2".to_string())
        );
        assert_eq!(
            int_of(member(
                &items(member(&record, "exception_chain"))[0],
                "code"
            )),
            2
        );
        assert_eq!(text_of(member(&record, "stdout")), "");
        assert_eq!(
            text_of(member(&record, "stderr")),
            "[cleanup] FAIL: docker image ls 실패 — bad news\n"
        );
    }

    fn big(byte: u8, count: usize) -> Vec<u8> {
        vec![byte; count]
    }

    #[test]
    fn the_child_is_run_as_subprocess_run_runs_it_sizes_signals_decoding_and_argument_forms() {
        let mut plan = Vec::new();
        let mut first = exchange(&["big"], &big(b'a', 200_000), 0);
        first.stderr = big(b'b', 100_000);
        plan.push(first);
        plan.push(exchange(
            &["utf8"],
            b"\xe2\x82|\xc0\x80|\xed\xa0\x80|\xf0\x9f\x98|a\xffb\xc3",
            0,
        ));
        plan.push(exchange(&["term"], b"partial", -15));
        plan.push(exchange(&["kill"], b"", -9));
        plan.push(exchange(&["code"], b"x", 255));
        plan.push(exchange(&[], b"z", 0));
        plan.push(exchange(&["bool"], b"t", 3));
        plan.push(exchange(&["float"], b"f", 0));
        let work = lab("child", &plan, false);
        let lines: Vec<String> = [
            ("[[\"docker\", \"big\"]]", "{}"),
            ("[[\"docker\", \"utf8\"]]", "{}"),
            ("[[\"docker\", \"term\"]]", "{}"),
            ("[[\"docker\", \"kill\"], 30]", "{}"),
            ("[[\"docker\", \"code\"]]", "{\"timeout\": null}"),
            ("[\"docker\"]", "{}"),
            ("[[\"docker\", \"bool\"], true]", "{}"),
            ("[[\"docker\", \"float\"], 12.5]", "{}"),
        ]
        .iter()
        .enumerate()
        .map(|(at, (args, kwargs))| request(at as i64, "_run", args, kwargs, &globals_text(&work)))
        .collect();
        let got = serve(&work, lines);
        assert_eq!(run_value(&got[0]), (0, "a".repeat(200_000)));
        assert_eq!(
            run_value(&got[1]),
            (
                0,
                "\u{fffd}|\u{fffd}\u{fffd}|\u{fffd}\u{fffd}\u{fffd}|\u{fffd}|a\u{fffd}b\u{fffd}"
                    .to_string()
            )
        );
        assert_eq!(run_value(&got[2]), (-15, "partial".to_string()));
        assert_eq!(run_value(&got[3]), (-9, String::new()));
        assert_eq!(run_value(&got[4]), (255, "x".to_string()));
        assert_eq!(run_value(&got[5]), (0, "z".to_string()));
        assert_eq!(run_value(&got[6]), (3, "t".to_string()));
        assert_eq!(run_value(&got[7]), (0, "f".to_string()));
        let state = work.standin.state();
        assert_eq!(state.counter, Some(8));
        assert!(!state.mismatch && !state.partial, "{:?}", state.argv);
        assert_eq!(state.argv[5], Vec::<String>::new());
        assert_eq!(state.argv[6], ["bool"]);
    }

    /// The executable is searched in the child's own `PATH`: a candidate that cannot be executed is skipped when a later one runs, and the
    /// error is the first one that is not "not found"/"not a directory" (else the last).
    #[test]
    fn the_executable_is_searched_in_the_path_the_way_the_original_runtime_searches_it() {
        let work = lab("path", &[exchange(&["probe"], b"ok", 0)], true);
        let base = work.area.dir.clone();
        let (denied, empty, plain) = (base.join("denied"), base.join("empty"), base.join("plain"));
        for dir in [&denied, &empty, &plain] {
            std::fs::create_dir(dir).expect("a directory");
        }
        std::fs::write(denied.join("docker"), "#!/bin/sh\nexit 0\n").expect("a file");
        std::fs::set_permissions(
            denied.join("docker"),
            std::fs::Permissions::from_mode(0o644),
        )
        .expect("mode");
        std::fs::write(plain.join("docker"), "x").expect("a file");
        let join = |dirs: &[&Path]| {
            dirs.iter()
                .map(|dir| dir.to_str().expect("a path").to_string())
                .collect::<Vec<_>>()
                .join(":")
        };
        let stand = work.standin.bin.clone();
        let line = request(
            1,
            "_run",
            "[[\"docker\", \"probe\"]]",
            "{}",
            &globals_text(&work),
        );
        let ask = |path: String| {
            let (output, records) =
                serve_lines(&work.area, std::slice::from_ref(&line), b"", &path);
            assert_eq!(output.status.code(), Some(0));
            run_value(&records[0])
        };
        let ok = (0, "ok".to_string());
        assert_eq!(ask(join(&[&denied, &stand])), ok);
        assert_eq!(ask(join(&[&empty, &denied, &stand])), ok);
        assert_eq!(
            ask(join(&[&denied])),
            (1, "[Errno 13] Permission denied: 'docker'".to_string())
        );
        assert_eq!(
            ask(join(&[&empty, &denied, &empty])),
            (1, "[Errno 13] Permission denied: 'docker'".to_string())
        );
        assert_eq!(
            ask(join(&[&empty])),
            (
                1,
                "[Errno 2] No such file or directory: 'docker'".to_string()
            )
        );
        // A file used as a directory is "not a directory": ignored like "not found", and the last of them is the error.
        let through_file = plain.join("docker");
        assert_eq!(
            ask(join(&[&through_file, &empty])),
            (
                1,
                "[Errno 2] No such file or directory: 'docker'".to_string()
            )
        );
        assert_eq!(
            ask(join(&[&empty, &through_file])),
            (1, "[Errno 20] Not a directory: 'docker'".to_string())
        );
        // Only the successful runs reached the stand-in.
        assert_eq!(work.standin.state().counter, Some(2));
    }

    fn images() -> Vec<u8> {
        let lines = [
            r#"{"Repository":"easy-vllm","Tag":"keepme","ID":"sha256:1","Size":"5GB","CreatedSince":"1 day ago"}"#,
            r#"{"Repository":"<none>","Tag":"<none>","ID":"sha256:dang","Size":"1GB","CreatedSince":"2 days ago"}"#,
            r#"{"Repository":"x","Tag":"<none>","ID":"sha256:half","Size":"2GB","CreatedSince":"3 days ago"}"#,
            r#"{"Repository":"a","Tag":"b"}"#,
            r#"{"Repository":5,"Tag":["x"],"Size":1.5,"CreatedSince":true,"ID":7}"#,
            "{}",
            r#"{"Repository":"가나다","Tag":"t","Size":"x"}"#,
            r#"{"Repository":"nvcr.io/nvidia/pytorch","Tag":"26.01-py3"}"#,
        ];
        format!("{}\n", lines.join("\n")).into_bytes()
    }

    fn table_exchanges() -> Vec<Exchange> {
        vec![
            exchange(&["ps", "-a", "--format", "{{.Image}}"], b"busy/img:1\n", 0),
            exchange(
                &["image", "ls", "-a", "--format", "{{json .}}"],
                &images(),
                0,
            ),
            exchange(&["system", "df"], b"TYPE  TOTAL\nImages  3\n", 0),
        ]
    }

    fn keep_lines() -> String {
        let mut text = "[cleanup] 보존리스트(4):\n".to_string();
        for (reference, reason) in [
            ("busy/img:1", "container in use"),
            ("easy-vllm:keepme", "--keep 수동"),
            ("extra:1", "--keep 수동"),
            (BASE_REF, "shared production resolution (default)"),
        ] {
            text.push_str(&format!("  KEEP  {reference:<60} ({reason})\n"));
        }
        text.push_str("\n[cleanup] 이미지 삭제 후보(6) — 보존리스트 밖:\n");
        for (reference, size, created, dangling) in [
            ("<none>:<none>", "1GB", "2 days ago", "  [dangling]"),
            ("x:<none>", "2GB", "3 days ago", "  [dangling]"),
            ("a:b", "None", "None", ""),
            ("5:['x']", "1.5", "True", ""),
            (":", "None", "None", ""),
            ("가나다:t", "x", "None", ""),
        ] {
            text.push_str(&format!(
                "  RM    {reference:<60} {size:<10} {created}{dangling}\n"
            ));
        }
        text
    }

    fn production(work: &Lab, args: &[&str]) -> std::process::Output {
        let mut run = command(
            env!("CARGO_BIN_EXE_cleanup_docker"),
            &work.area,
            &bin_path(&work.standin),
        );
        run.arg0(layout_name(&work.area.root, "cleanup_docker"))
            .args(args);
        finish(run, None)
    }

    #[test]
    fn the_dry_run_table_is_the_original_text_with_code_point_padding_str_of_any_json_value_and_dangling_targets(
    ) {
        let work = lab("table", &table_exchanges(), false);
        let output = production(&work, &["--k", "extra:1", "--ke", "easy-vllm:keepme"]);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        let want = format!(
            "{}\n[cleanup] 현 사용량:\n  TYPE  TOTAL\n  Images  3\n\n[cleanup] 빌드캐시: 336h 미접근분 prune 대상 (docker builder prune --filter unused-for=336h)\n\n[cleanup] DRY-RUN 종료 — 표 검토 후: python3 .claude/skills/vllm-recipe-explorer/scripts/cleanup_docker.py --apply [--keep <ref>]\n",
            keep_lines()
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout), want);
        assert_eq!(
            work.standin.calls(),
            [
                vec!["ps", "-a", "--format", "{{.Image}}"],
                vec!["image", "ls", "-a", "--format", "{{json .}}"],
                vec!["system", "df"]
            ]
        );
        assert!(!work.standin.state().mismatch);
    }

    #[test]
    fn apply_removes_each_candidate_by_its_target_in_order_then_prunes_and_reports_the_failures() {
        let mut plan = table_exchanges();
        for (target, stdout, code) in [
            ("sha256:dang", "Untagged: a\nDeleted: b\n", 0),
            ("sha256:half", "", 1),
            ("a:b", "", 0),
            ("5:['x']", "x\n\n  last line  \n", 1),
            (":", "Deleted: z", 0),
            ("가나다:t", "é\n", 0),
        ] {
            plan.push(exchange(&["rmi", target], stdout.as_bytes(), code));
        }
        plan.push(exchange(
            &["builder", "prune", "-f", "--filter", "unused-for=5h"],
            b"a\nb\nc\nd\ne\n",
            0,
        ));
        plan.push(exchange(&["system", "df"], b"TYPE\nImages  4\n", 0));
        let work = lab("apply", &plan, false);
        let output = production(
            &work,
            &[
                "--apply",
                "--cache-age-hours",
                "5",
                "--keep",
                "extra:1",
                "--keep=easy-vllm:keepme",
            ],
        );
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let rmi = |target: &str, tag: &str, last: &str| {
            format!("[cleanup] rmi {target:<60} {tag} {last}\n")
        };
        let want = format!(
            "{}\n[cleanup] 현 사용량:\n  TYPE  TOTAL\n  Images  3\n\n[cleanup] 빌드캐시: 5h 미접근분 prune 대상 (docker builder prune --filter unused-for=5h)\n{}{}{}{}{}{}[cleanup] builder prune: OK\n  c\n  d\n  e\n[cleanup] 정리 후 사용량:\n  TYPE\n  Images  4\n[cleanup] ⚠ rmi 실패 2건(자식 레이어 참조 등) — 표 재검토\n",
            keep_lines(),
            rmi("sha256:dang", "OK", "Deleted: b"),
            rmi("sha256:half", "FAIL", ""),
            rmi("a:b", "OK", ""),
            rmi("5:['x']", "FAIL", "  last line"),
            rmi(":", "OK", "Deleted: z"),
            rmi("가나다:t", "OK", "é"),
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout), want);
        let state = work.standin.state();
        // three queries, six removals, one prune, one usage report
        assert_eq!(state.counter, Some(11));
        assert!(!state.mismatch && !state.partial);
    }

    #[test]
    fn argument_forms_and_errors_are_argparses_and_the_hours_are_python_ints() {
        let plan = vec![
            exchange(&["ps", "-a", "--format", "{{.Image}}"], b"", 1),
            exchange(&["image", "ls", "-a", "--format", "{{json .}}"], b"", 0),
            exchange(&["system", "df"], b"", 0),
        ];
        let work = lab("arguments", &plan, true);
        let hours = |value: &str| -> String {
            let output = production(&work, &["--cache-age-hours", value]);
            assert_eq!(
                output.status.code(),
                Some(0),
                "{value}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let text = String::from_utf8_lossy(&output.stdout).into_owned();
            text.lines()
                .find(|line| line.starts_with("[cleanup] 빌드캐시: "))
                .expect("the cache line")
                .to_string()
        };
        for (given, shown) in [
            ("007", "7"),
            (" 8 ", "8"),
            ("+9", "9"),
            ("-5", "-5"),
            ("1_0", "10"),
            ("٣", "3"),
            ("０７", "7"),
            ("99999999999999999999", "99999999999999999999"),
        ] {
            assert_eq!(
                hours(given),
                format!("[cleanup] 빌드캐시: {shown}h 미접근분 prune 대상 (docker builder prune --filter unused-for={shown}h)"),
                "{given:?}"
            );
        }
        let usage = "usage: cleanup_docker.py [-h] [--apply] [--cache-age-hours CACHE_AGE_HOURS]\n                         [--keep KEEP]\n";
        let before = work.standin.state().counter;
        for (args, message) in [
            (
                vec!["--apply=x"],
                "argument --apply: ignored explicit argument 'x'",
            ),
            (
                vec!["--cache-age-hours"],
                "argument --cache-age-hours: expected one argument",
            ),
            (
                vec!["--cache-age-hours", "1.5"],
                "argument --cache-age-hours: invalid int value: '1.5'",
            ),
            (
                vec!["--cache-age-hours=x"],
                "argument --cache-age-hours: invalid int value: 'x'",
            ),
            (vec!["extra"], "unrecognized arguments: extra"),
            (
                vec!["--a", "--bogus", "--k"],
                "argument --keep: expected one argument",
            ),
        ] {
            let output = production(&work, &args);
            assert_eq!(output.status.code(), Some(2), "{args:?}");
            assert!(output.stdout.is_empty(), "{args:?}");
            assert_eq!(
                String::from_utf8_lossy(&output.stderr),
                format!("{usage}cleanup_docker.py: error: {message}\n"),
                "{args:?}"
            );
        }
        // No query was made by any of the refused command lines.
        let after = work.standin.state().counter;
        assert_eq!(after.map(|n| n - before.unwrap_or(0)), Some(0));
        // `--keep ""` and `--keep " x "` keep their exact text (the manual keep is not stripped).
        let output = production(&work, &["--keep", "", "--keep", " x "]);
        let text = String::from_utf8_lossy(&output.stdout).into_owned();
        assert!(text.contains(&format!("[cleanup] 보존리스트(3):\n  KEEP  {:<60} (--keep 수동)\n  KEEP  {:<60} (--keep 수동)\n", "", " x ")), "{text}");
    }
}

// ── the terminal width the command line reads once ───────────────────────────────────────────────

mod width_cases {
    use super::*;

    #[test]
    fn the_usage_is_wrapped_to_the_columns_variable_the_process_was_given() {
        let work = lab("columns", &no_containers(), true);
        let run = |columns: &str| {
            let mut run = command(
                env!("CARGO_BIN_EXE_cleanup_docker"),
                &work.area,
                &bin_path(&work.standin),
            );
            run.env("COLUMNS", columns).arg("--bogus");
            let output = finish(run, None);
            assert_eq!(output.status.code(), Some(2), "{columns}");
            String::from_utf8_lossy(&output.stderr).into_owned()
        };
        assert_eq!(
            run("200"),
            "usage: cleanup_docker.py [-h] [--apply] [--cache-age-hours CACHE_AGE_HOURS] [--keep KEEP]\ncleanup_docker.py: error: unrecognized arguments: --bogus\n"
        );
        assert_eq!(
            run("80"),
            "usage: cleanup_docker.py [-h] [--apply] [--cache-age-hours CACHE_AGE_HOURS]\n                         [--keep KEEP]\ncleanup_docker.py: error: unrecognized arguments: --bogus\n"
        );
        // A width too small for the name (24 columns of prefix and name exceed 0.75 of 28): the name alone, then the options wrapped
        // to the indent of the prefix, one oversized part on a line of its own (argparse's `format_usage`).
        assert_eq!(
            run("30"),
            "usage: cleanup_docker.py\n       [-h] [--apply]\n       [--cache-age-hours CACHE_AGE_HOURS]\n       [--keep KEEP]\ncleanup_docker.py: error: unrecognized arguments: --bogus\n"
        );
        assert_eq!(work.standin.state().counter, Some(0));
    }
}

// ── the frozen benchmark ─────────────────────────────────────────────────────────────────────────

mod bench_cases {
    use super::*;

    fn run_bench(work: &Lab, fixture: &Path, extra: &[&str]) -> std::process::Output {
        let mut run = command(
            env!("CARGO_BIN_EXE_recipe_cleanup_bench"),
            &work.area,
            &bin_path(&work.standin),
        );
        run.arg("--fixture").arg(fixture).args(extra);
        finish(run, None)
    }

    fn row(work: &Lab, metric: &str, operation: &str, args: &str, expected: &str) -> String {
        format!(
            "{{\"metric\": \"{metric}\", \"operation\": \"cleanup_docker.{operation}\", \"args\": {args}, \"kwargs\": {{}}, \"globals\": {}, \"expected\": {expected}}}",
            globals_text(work)
        )
    }

    fn write_rows(work: &Lab, name: &str, rows: &[String]) -> PathBuf {
        let path = work.area.dir.join(name);
        std::fs::write(&path, format!("[{}]", rows.join(","))).expect("a fixture");
        path
    }

    /// One real call to check the value, then `rounds * iterations` timed calls: the stand-in counts how many real children were run.
    fn series(
        name: &str,
        plan: Vec<Exchange>,
        operation: &str,
        args: &str,
        expected: &str,
        calls_per_operation: u64,
    ) {
        let work = lab(&format!("bench-{name}"), &plan, true);
        let good = write_rows(
            &work,
            "good.json",
            &[row(&work, name, operation, args, expected)],
        );
        let output = run_bench(&work, &good, &["--rounds", "2", "--iterations", "3"]);
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
        assert_eq!(text_of(member(&report, "metric")), name);
        assert_eq!(text_of(member(&report, "unit")), "ns_per_call");
        assert_eq!(items(member(&report, "rounds")).len(), 2);
        assert_eq!(int_of(member(&report, "iterations")), 3);
        // One preflight call and 2 x 3 measured calls, each of them real child processes.
        let state = work.standin.state();
        assert_eq!(state.counter, Some(calls_per_operation * 7), "{name}");
        assert!(!state.mismatch && !state.partial, "{name}");
        // A wrong value ends the run before any timer: no report and only the preflight call's children.
        let wrong_work = lab(&format!("bench-{name}-wrong"), &plan, true);
        let wrong = write_rows(
            &wrong_work,
            "wrong.json",
            &[row(
                &wrong_work,
                name,
                operation,
                args,
                "{\"kind\": \"none\", \"extra\": 1}",
            )],
        );
        let output = run_bench(&wrong_work, &wrong, &["--rounds", "2", "--iterations", "3"]);
        assert_eq!(output.status.code(), Some(1), "{name}");
        assert!(output.stdout.is_empty(), "{name}");
        assert_eq!(
            wrong_work.standin.state().counter,
            Some(calls_per_operation),
            "{name}"
        );
    }

    #[test]
    fn the_benchmark_calls_each_real_function_once_to_check_the_whole_dto_then_measures_whole_calls_with_real_children(
    ) {
        let ps = |stdout: &[u8], code: i64| {
            exchange(&["ps", "-a", "--format", "{{.Image}}"], stdout, code)
        };
        let ls =
            |stdout: &[u8]| exchange(&["image", "ls", "-a", "--format", "{{json .}}"], stdout, 0);
        series(
            "run",
            vec![exchange(&["probe"], b"ok\n", 0)],
            "_run",
            "[[\"docker\", \"probe\"]]",
            "{\"kind\": \"run_tuple\", \"items\": [0, \"ok\\n\"]}",
            1,
        );
        series(
            "list",
            vec![ls(b"{\"Repository\": \"a\", \"Tag\": \"b\"}\n")],
            "list_images",
            "[]",
            "{\"kind\": \"json\", \"data\": [{\"Repository\": \"a\", \"Tag\": \"b\"}]}",
            1,
        );
        series(
            "preserve",
            vec![ps(b"busy/x:1\n", 0)],
            "preserve_set",
            "[]",
            &format!(
                "{{\"kind\": \"preserve_tuple\", \"keep\": [\"busy/x:1\", \"{BASE_REF}\"], \"why\": {{\"{BASE_REF}\": \"shared production resolution (default)\", \"busy/x:1\": \"container in use\"}}}}"
            ),
            1,
        );
        series(
            "main",
            vec![
                ps(b"busy/x:1\n", 0),
                ls(b""),
                exchange(&["system", "df"], b"T\n", 0),
            ],
            "main",
            "[]",
            "{\"kind\": \"none\"}",
            3,
        );
    }

    #[test]
    fn a_failing_call_an_unknown_operation_and_a_bad_option_end_the_run_without_a_report() {
        let work = lab(
            "bench-refusals",
            &[exchange(
                &["image", "ls", "-a", "--format", "{{json .}}"],
                b"bad\n",
                1,
            )],
            true,
        );
        // `list_images` exits with 2 when the listing fails: an error, not a sample.
        let failing = write_rows(
            &work,
            "failing.json",
            &[row(
                &work,
                "bad",
                "list_images",
                "[]",
                "{\"kind\": \"json\", \"data\": []}",
            )],
        );
        let output = run_bench(&work, &failing, &[]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let unknown = write_rows(
            &work,
            "unknown.json",
            &[row(&work, "bad", "nothing", "[]", "null")],
        );
        assert_eq!(run_bench(&work, &unknown, &[]).status.code(), Some(1));
        let other_module = write_rows(
            &work,
            "module.json",
            &[row(&work, "bad", "x", "[]", "null").replace("cleanup_docker.x", "other.x")],
        );
        assert_eq!(run_bench(&work, &other_module, &[]).status.code(), Some(1));
        let no_expected = write_rows(&work, "none.json", &["{\"metric\": \"m\", \"operation\": \"cleanup_docker._run\", \"args\": [], \"kwargs\": {}}".to_string()]);
        assert_eq!(run_bench(&work, &no_expected, &[]).status.code(), Some(1));
        for extra in [
            vec!["--rounds", "0"],
            vec!["--iterations", "x"],
            vec!["--bogus", "1"],
        ] {
            assert_eq!(
                run_bench(&work, &failing, &extra).status.code(),
                Some(2),
                "{extra:?}"
            );
        }
        let missing = work.area.dir.join("absent.json");
        assert_eq!(run_bench(&work, &missing, &[]).status.code(), Some(1));
    }
}

// ── controls: the comparators of these checks do detect a difference ─────────────────────────────

mod control_cases {
    use super::*;

    #[test]
    fn the_comparators_report_a_changed_value_tree_traceback_and_stand_in_log() {
        let bundle = load("bundle.json");
        let case = &items(member(&bundle, "api"))[40];
        let original = member(case, "original");
        // A changed saved value, a changed key order and a changed type are all differences.
        assert!(same_value(original, &original.clone()));
        let mut changed = original.clone();
        if let PyValue::Dict(map) = &mut changed {
            map.set("value", PyValue::str("other"));
        }
        assert!(!same_value(original, &changed));
        let order = j(r#"{"a": 1, "b": 2}"#);
        assert!(!same_value(&order, &j(r#"{"b": 2, "a": 1}"#)));
        assert!(!same_value(&j(r#"[1]"#), &j(r#"[1.0]"#)));
        // The tree comparison names what is missing, changed and extra.
        let before = expected_tree(&bundle, member(case, "before"));
        let mut other = before.clone();
        let (path, entry) = before
            .iter()
            .find(|(_, (kind, _, _))| kind == "file")
            .map(|(path, entry)| (path.clone(), entry.clone()))
            .expect("a file");
        other.get_mut(&path).expect("entry").2.push(b'x');
        other.insert("node/extra".to_string(), entry);
        other.remove("node/output");
        let problems = tree_difference(&before, &other);
        assert!(problems.iter().any(|each| each.starts_with("changed ")));
        assert!(problems.iter().any(|each| each == "extra node/extra"));
        assert!(problems.iter().any(|each| each == "missing node/output"));
        assert!(tree_difference(&before, &before).is_empty());
        // A traceback frame that is not declared is refused, and a declared one is projected away.
        let frames = declared_frames(&bundle);
        let saved = b"Traceback (most recent call last):\n  File \"/elsewhere/x.py\", line 1, in f\n    y\nValueError: a\n";
        assert!(project_traceback(saved, &frames).is_err());
        let declared = format!(
            "Traceback (most recent call last):\n  File \"{}\", line {}, in {}\n    y\nValueError: a\n",
            frames[0].0, frames[0].1, frames[0].2
        );
        assert_eq!(
            project_traceback(declared.as_bytes(), &frames).expect("declared"),
            b"ValueError: a\n"
        );
        assert!(project_traceback(
            b"Traceback (most recent call last):\nValueError: a\n",
            &frames
        )
        .is_err());
        // The stand-in's log is what the program really asked: a wrong expectation is reported.
        let work = prepare(&bundle, case, "control");
        let mut wrong = case.clone();
        if let PyValue::Dict(map) = &mut wrong {
            let mut peer = PyDict::new();
            peer.set("counter", PyValue::int(1));
            peer.set("argv", j(r#"[["x"]]"#));
            map.set("peer", PyValue::Dict(peer));
        }
        let problems = peer_problems(&wrong, &work.standin);
        assert_eq!(problems.len(), 2, "{problems:?}");
        // The stand-in answers 97 and records a mismatch when it is asked something other than its exchange says.
        let output = {
            let mut run = command("/bin/sh", &work.area, &bin_path(&work.standin));
            run.args(["-c", "docker unexpected"]);
            finish(run, None)
        };
        assert_eq!(output.status.code(), Some(97));
        assert!(work.standin.state().mismatch);
    }
}

// ── R32 stdio repair: live standard output (N1) and the typed encode constructor (N2) ────────────

fn load_stdio() -> PyValue {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("checks")
        .join("data")
        .join("cleanup_stdio_reference")
        .join("bundle.json");
    let bytes = std::fs::read(path).expect("saved stdio data");
    loads_text(&PyStr::from_surrogateescape(&bytes)).expect("saved stdio JSON")
}

fn all_or_fail(failures: Vec<String>) {
    assert!(
        failures.is_empty(),
        "{} problems:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

mod stdio_cases {
    use super::*;

    fn cases(bundle: &PyValue) -> &[PyValue] {
        let rows = items(member(bundle, "cases"));
        assert_eq!(rows.len(), 5);
        rows
    }

    fn is_cli(case: &PyValue) -> bool {
        has(member(case, "request"), "args") && !has(member(case, "request"), "function")
    }

    #[test]
    fn the_5_saved_stdio_originals_are_reproduced_whole_through_the_fixtures() {
        let bundle = load_stdio();
        let failures: Vec<String> = cases(&bundle)
            .iter()
            .flat_map(|case| reference_cases::fixture_problems(&bundle, case, is_cli(case)))
            .collect();
        all_or_fail(failures);
    }

    #[test]
    fn the_4_saved_command_lines_end_as_the_original_did_through_the_production_program_and_the_saved_api_call_through_the_service(
    ) {
        let bundle = load_stdio();
        let mut failures = Vec::new();
        for case in cases(&bundle) {
            if is_cli(case) {
                failures.extend(reference_cases::program_problems(&bundle, case));
            } else {
                failures.extend(reference_cases::service_problems(&bundle, case));
            }
        }
        all_or_fail(failures);
    }

    #[test]
    fn the_saved_small_broken_pipe_run_does_all_its_work_and_fails_only_at_the_final_flush_while_the_large_ones_stop_at_the_print(
    ) {
        let bundle = load_stdio();
        let rows = cases(&bundle);
        // The saved facts the checks above compare, stated once: the large failures stop before any listing or removal (one `ps`), the
        // small one runs all seven commands and exits 120 although `main` returned.
        for (index, calls, status) in [(0, 1, 1), (1, 1, 1), (2, 7, 120), (3, 7, 0)] {
            let case = &rows[index];
            assert_eq!(int_of(member(member(case, "peer"), "counter")), calls);
            assert_eq!(int_of(member(case, "exit_code")), status);
        }
        assert_eq!(
            text_of(member(member(&rows[2], "original"), "kind")),
            "return"
        );
        assert_eq!(int_of(member(member(&rows[2], "original"), "status")), 0);
        assert_eq!(
            text_of(member(member(&rows[0], "original"), "kind")),
            "exception"
        );
    }

    fn canonical_exchanges() -> Vec<Exchange> {
        vec![
            exchange(&["ps", "-a", "--format", "{{.Image}}"], b"", 1),
            exchange(&["image", "ls", "-a", "--format", "{{json .}}"], b"", 0),
            exchange(&["system", "df"], b"", 0),
        ]
    }

    /// A fixture-command-line case made here: `args` and the files of a canonical tree under `root`.
    fn fixture_cli(
        work: &Lab,
        args: &str,
        wrapper: Option<&str>,
    ) -> (std::process::Output, PyValue) {
        let spec = work.area.dir.join("spec.json");
        let control = work.area.dir.join("control.json");
        std::fs::write(
            &spec,
            format!("{{\"id\": \"made\", \"owner\": \"cleanup_docker\", \"args\": {args}, \"kwargs\": {{}}, \"context\": {{}}}}"),
        )
        .expect("a spec");
        let program = env!("CARGO_BIN_EXE_recipe_cleanup_fixture_cli");
        let mut run = match wrapper {
            // the wrapper closes one standard descriptor before the program starts
            Some(redirect) => {
                let mut run = command("/bin/sh", &work.area, &bin_path(&work.standin));
                run.args(["-c", &format!("exec \"$0\" \"$@\" {redirect}"), program]);
                run
            }
            None => command(program, &work.area, &bin_path(&work.standin)),
        };
        run.arg("--spec")
            .arg(&spec)
            .arg("--root")
            .arg(&work.area.root)
            .arg("--control")
            .arg(&control);
        let output = finish(run, None);
        let record = read_control(&control);
        (output, record)
    }

    #[test]
    fn a_small_dry_run_on_a_broken_pipe_does_everything_and_fails_at_the_final_flush_with_status_120(
    ) {
        let work = lab("small-broken", &canonical_exchanges(), false);
        let spec = work.area.dir.join("spec.json");
        let control = work.area.dir.join("control.json");
        std::fs::write(
            &spec,
            r#"{"id": "made", "owner": "cleanup_docker", "args": [], "kwargs": {}, "context": {}}"#,
        )
        .expect("a spec");
        let mut run = command(
            env!("CARGO_BIN_EXE_recipe_cleanup_fixture_cli"),
            &work.area,
            &bin_path(&work.standin),
        );
        run.arg("--spec")
            .arg(&spec)
            .arg("--root")
            .arg(&work.area.root)
            .arg("--control")
            .arg(&control);
        let (reader, writer) = std::io::pipe().expect("a pipe");
        drop(reader);
        run.stdout(Stdio::from(writer))
            .stderr(Stdio::piped())
            .stdin(Stdio::null());
        let output = run
            .spawn()
            .expect("start")
            .wait_with_output()
            .expect("wait");
        drop(run);
        assert_eq!(output.status.code(), Some(120));
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            "Exception ignored in: <_io.TextIOWrapper name='<stdout>' mode='w' encoding='utf-8'>\nBrokenPipeError: [Errno 32] Broken pipe\n"
        );
        assert_eq!(work.standin.state().counter, Some(3));
        let record = read_control(&control);
        assert_eq!(text_of(member(&record, "kind")), "return");
        assert_eq!(int_of(member(&record, "status")), 0);
    }

    #[test]
    fn a_failing_stdout_write_stops_the_business_code_at_that_print_and_what_was_buffered_before_it_is_still_flushed_at_exit(
    ) {
        // `--keep` with a lone surrogate (JSON can say it; a command line cannot): the print of its KEEP line cannot be encoded.
        let work = lab(
            "encode-print",
            &[exchange(&["ps", "-a", "--format", "{{.Image}}"], b"", 1)],
            false,
        );
        let (output, record) = fixture_cli(&work, r#"["--keep", "\ud800"]"#, None);
        assert_eq!(output.status.code(), Some(1));
        let want_line =
            format!("  KEEP  {BASE_REF:<60} (shared production resolution (default))\n");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            format!("[cleanup] 보존리스트(2):\n{want_line}")
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            "UnicodeEncodeError: 'utf-8' codec can't encode character '\\ud800' in position 8: surrogates not allowed\n"
        );
        // Only the container query ran: nothing after the failing print.
        assert_eq!(
            work.standin.calls(),
            [["ps", "-a", "--format", "{{.Image}}"]]
        );
        assert_eq!(text_of(member(&record, "kind")), "exception");
        let chain = items(member(&record, "exception_chain"));
        assert_eq!(chain.len(), 1);
        let want = j(
            r#"{"kind": "unicode_encode", "encoding": "utf-8", "start": 8, "end": 9, "reason": "surrogates not allowed"}"#,
        );
        let got = member(&chain[0], "constructor");
        assert_eq!(text_of(member(got, "kind")), text_of(member(&want, "kind")));
        assert_eq!(int_of(member(got, "start")), 8);
        assert_eq!(int_of(member(got, "end")), 9);
        assert_eq!(text_of(member(got, "reason")), "surrogates not allowed");
        assert_eq!(text_of(member(got, "encoding")), "utf-8");
        // The object is the text of the write that failed, lone surrogate included.
        let PyValue::Str(failed) = member(got, "object") else {
            panic!("object is text")
        };
        assert_eq!(
            failed.code_points().len(),
            8 + 60 + 1 + "(--keep 수동)".chars().count()
        );
        assert_eq!(failed.code_points()[8], 0xD800);
    }

    #[test]
    fn a_stdout_closed_at_start_makes_print_a_no_op_and_a_stderr_closed_at_start_sends_the_failed_listing_to_stdout(
    ) {
        let work = lab("closed", &canonical_exchanges(), false);
        let (output, record) = fixture_cli(&work, "[]", Some(">&-"));
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        assert_eq!(work.standin.state().counter, Some(3));
        assert_eq!(text_of(member(&record, "kind")), "return");
        // No standard error: `print(..., file=sys.stderr)` is a print to `sys.stdout`; the exit is still `sys.exit(2)`.
        let failing = lab(
            "closed-stderr",
            &[
                exchange(&["ps", "-a", "--format", "{{.Image}}"], b"", 1),
                exchange(
                    &["image", "ls", "-a", "--format", "{{json .}}"],
                    b"gone\n",
                    1,
                ),
            ],
            false,
        );
        let (output, record) = fixture_cli(&failing, "[]", Some("2>&-"));
        assert_eq!(output.status.code(), Some(2));
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            format!("[cleanup] 보존리스트(1):\n  KEEP  {BASE_REF:<60} (shared production resolution (default))\n[cleanup] FAIL: docker image ls 실패 — gone\n")
        );
        assert_eq!(text_of(member(&record, "kind")), "exit");
    }

    /// `repo` is the JSON text of the `REPO` value (a Rust string cannot hold a lone surrogate, JSON can say it).
    fn service_record(work: &Lab, repo: &str, canonical_only: &str) -> PyValue {
        let globals = format!("{{\"REPO\": {repo}, \"argv\": [\"cleanup_docker.py\"]}}");
        serve(
            work,
            vec![request(
                1,
                "preserve_set",
                &format!("[{canonical_only}]"),
                "{}",
                &globals,
            )],
        )
        .remove(0)
    }

    #[test]
    fn the_encode_failure_of_a_path_carries_the_constructor_fields_of_the_encoders_own_range() {
        let work = lab("encode-range", &no_containers(), true);
        let tail =
            "/.claude/skills/upstream-version-watch/assets/current-production-resolution.json";
        // two lone surrogates: one error over both; the message names the positions
        let record = service_record(&work, r#""/x\ud800\ud801y""#, "true");
        let (class, message) = exception_of(&record);
        assert_eq!(class, "RuntimeError");
        assert_eq!(
            message,
            "canonical shared production resolution invalid: 'utf-8' codec can't encode characters in position 2-3: surrogates not allowed"
        );
        let chain = items(member(&record, "exception_chain"));
        assert_eq!(chain.len(), 2);
        assert_eq!(text_of(member(&chain[0], "relation")), "cause");
        assert!(matches!(
            member(&chain[0], "suppress_context"),
            PyValue::Bool(true)
        ));
        assert!(!has(&chain[0], "constructor"));
        assert_eq!(text_of(member(&chain[1], "type")), "UnicodeEncodeError");
        assert_eq!(text_of(member(&chain[1], "module")), "builtins");
        let ctor = member(&chain[1], "constructor");
        let mut want_object: Vec<u32> = "/x".chars().map(u32::from).collect();
        want_object.extend([0xD800, 0xD801, u32::from('y')]);
        want_object.extend(tail.chars().map(u32::from));
        let PyValue::Str(object) = member(ctor, "object") else {
            panic!("text")
        };
        assert_eq!(object.code_points(), want_object.as_slice());
        assert_eq!(int_of(member(ctor, "start")), 2);
        assert_eq!(int_of(member(ctor, "end")), 4);
        assert_eq!(text_of(member(ctor, "reason")), "surrogates not allowed");
        // a surrogate that can be escaped (U+DC80) is emitted as its byte; the error starts at the first one that cannot
        let mixed = service_record(&work, r#""/x\udc80\ud800y""#, "true");
        let ctor = member(&items(member(&mixed, "exception_chain"))[1], "constructor");
        assert_eq!(int_of(member(ctor, "start")), 3);
        assert_eq!(int_of(member(ctor, "end")), 4);
        // an escapable surrogate alone is no error: the path is only missing
        let escaped = service_record(&work, r#""/x\udc80y""#, "true");
        assert!(
            text_of(member(&escaped, "message")).contains("[Errno 2] No such file or directory")
        );
        // the decode constructor stays distinct
        put(&work.area.root, TEMPLATE, b"FROM \xff");
        let decode = call1(&work, "preserve_set", "[]", "{}");
        let ctor = member(&items(member(&decode, "exception_chain"))[1], "constructor");
        assert_eq!(text_of(member(ctor, "kind")), "unicode_decode");
        assert!(has(ctor, "object_hex") && !has(ctor, "object"));
    }
}

// ── the synchronous console: `evaluate-events` (a parent that performs the writes) ──────────────

mod events_cases {
    use super::*;
    use std::io::{BufRead, BufReader, Read};

    /// What the parent (the caller of the program) saw and did.
    struct Session {
        /// `(sequence, stream, text)` of every write event, in order.
        writes: Vec<(i64, String, PyStr)>,
        result: Option<PyValue>,
        status: Option<i32>,
        stderr: String,
        /// Lines of standard output that were neither a write event nor the result.
        stray: Vec<String>,
    }

    fn ok(sequence: i64) -> String {
        format!("{{\"event\": \"write_ack\", \"sequence\": {sequence}, \"kind\": \"ok\"}}")
    }

    /// The `builtin_bases` a CPython parent reflects from the real MRO of a builtin exception class (`getattr(builtins, base.__name__) is base`,
    /// in MRO order, `object` last as the reflection lists it): the names of the classes of the builtin hierarchy this check uses.
    fn default_bases(class: &str) -> Vec<&str> {
        let mut bases = vec![class];
        match class {
            "BrokenPipeError"
            | "ConnectionResetError"
            | "ConnectionAbortedError"
            | "ConnectionRefusedError" => bases.extend(["ConnectionError", "OSError"]),
            "ConnectionError" | "FileNotFoundError" | "PermissionError" | "TimeoutError"
            | "BlockingIOError" | "InterruptedError" | "IsADirectoryError"
            | "NotADirectoryError" | "FileExistsError" => bases.push("OSError"),
            "UnicodeEncodeError" | "UnicodeDecodeError" => {
                bases.extend(["UnicodeError", "ValueError"])
            }
            "UnicodeError" => bases.push("ValueError"),
            _ => {}
        }
        if class != "Exception" {
            bases.push("Exception");
        }
        bases.extend(["BaseException", "object"]);
        bases
    }

    /// An exception acknowledgement of any module with the descriptor given (`None` leaves the member out: the old schema).
    fn failure_full(
        sequence: i64,
        class: &str,
        module: &str,
        message: &str,
        constructor: Option<&str>,
        bases: Option<&str>,
    ) -> String {
        let tail = constructor.map_or(String::new(), |ctor| format!(", \"constructor\": {ctor}"));
        let descriptor = bases.map_or(String::new(), |list| format!(", \"builtin_bases\": {list}"));
        format!(
            "{{\"event\": \"write_ack\", \"sequence\": {sequence}, \"kind\": \"exception\", \"type\": {}, \"module\": {}, \"message\": {}{descriptor}{tail}}}",
            jstr(class),
            jstr(module),
            jstr(message)
        )
    }

    fn bases_json(bases: &[&str]) -> String {
        format!(
            "[{}]",
            bases
                .iter()
                .map(|base| jstr(base))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }

    fn failure(sequence: i64, class: &str, message: &str, constructor: Option<&str>) -> String {
        let bases = bases_json(&default_bases(class));
        failure_full(
            sequence,
            class,
            "builtins",
            message,
            constructor,
            Some(&bases),
        )
    }

    fn broken(sequence: i64) -> String {
        failure(sequence, "BrokenPipeError", "[Errno 32] Broken pipe", None)
    }

    /// Runs one request through `evaluate-events`; `ack(sequence, stream, text)` is the parent's answer line (`None`: it goes away).
    fn session(
        work: &Lab,
        request_line: &str,
        stdin: &[u8],
        mut ack: impl FnMut(i64, &str, &PyStr) -> Option<String>,
    ) -> Session {
        let (mut command, mut writer) = over_descriptor(
            env!("CARGO_BIN_EXE_recipe_cleanup_api"),
            &["evaluate-events", "--input-fd", "3"],
            &work.area,
            stdin,
            &bin_path(&work.standin),
        );
        let mut child = command.spawn().expect("start the program");
        drop(command);
        let mut writer = Some({
            writeln!(writer, "{request_line}").expect("the request");
            writer.flush().expect("flush");
            std::mem::replace(&mut writer, std::io::pipe().expect("a spare pipe").1)
        });
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
            let event = match loads_text(&PyStr::from_code_points(
                text.code_points()
                    .iter()
                    .copied()
                    .filter(|&point| point != u32::from('\n'))
                    .collect(),
            )) {
                Ok(event) => event,
                Err(_) => {
                    seen.stray.push(String::from_utf8_lossy(&raw).into_owned());
                    continue;
                }
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
                    match ack(sequence, &stream, written) {
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
        Some(ok(sequence))
    }

    fn main_request(work: &Lab, args: &[&str]) -> String {
        let literals: Vec<String> = args.iter().map(|each| jstr(each)).collect();
        main_request_json(work, &literals)
    }

    /// The same with the arguments as JSON texts (a lone surrogate can be said in JSON only).
    fn main_request_json(work: &Lab, args: &[String]) -> String {
        let mut argv = vec![jstr("cleanup_docker.py")];
        argv.extend(args.iter().cloned());
        request(
            1,
            "main",
            "[]",
            "{}",
            &format!(
                "{{\"REPO\": {}, \"argv\": [{}]}}",
                jstr(&format!("{}/node", root_text(&work.area))),
                argv.join(", ")
            ),
        )
    }

    fn table_lab(name: &str) -> Lab {
        let plan = vec![
            exchange(&["ps", "-a", "--format", "{{.Image}}"], b"busy/img:1\n", 0),
            exchange(
                &["image", "ls", "-a", "--format", "{{json .}}"],
                b"{\"Repository\":\"a\",\"Tag\":\"b\",\"Size\":\"1GB\",\"CreatedSince\":\"1 day\"}\n{\"Repository\":\"<none>\",\"Tag\":\"<none>\",\"ID\":\"sha256:d\",\"Size\":\"2GB\",\"CreatedSince\":\"2 days\"}\n",
                0,
            ),
            exchange(&["system", "df"], b"TYPE\nImages  2\n", 0),
            exchange(&["rmi", "a:b"], b"Deleted: a\n", 0),
            exchange(&["rmi", "sha256:d"], b"Deleted: d\n", 0),
            exchange(&["builder", "prune", "-f", "--filter", "unused-for=336h"], b"done\n", 0),
            exchange(&["system", "df"], b"TYPE\nImages  0\n", 0),
        ];
        lab(name, &plan, false)
    }

    fn record_of(seen: &Session) -> &PyValue {
        seen.result.as_ref().expect("a result event")
    }

    #[test]
    fn every_print_is_two_announced_writes_in_order_and_the_result_carries_empty_streams() {
        let bundle = load_stdio();
        let case = &items(member(&bundle, "cases"))[3];
        let work = reference_cases_prepare(&bundle, case);
        let request_row = member(case, "request");
        let mut argv = vec![format!(
            "{}/node/.claude/skills/vllm-recipe-explorer/scripts/cleanup_docker.py",
            root_text(&work.area)
        )];
        for arg in items(member(request_row, "args")) {
            argv.push(text_of(arg));
        }
        let globals = format!(
            "{{\"REPO\": {}, \"argv\": [{}]}}",
            jstr(&format!("{}/node", root_text(&work.area))),
            argv.iter()
                .map(|each| jstr(each))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let line = request(1, "main", "[]", "{}", &globals);
        let work = Lab {
            area: work.area,
            standin: work.standin,
        };
        let seen = session(&work, &line, b"", accept_all);
        assert_eq!(seen.status, Some(0), "{}", seen.stderr);
        assert!(seen.stderr.is_empty() && seen.stray.is_empty());
        for (at, (sequence, stream, text)) in seen.writes.iter().enumerate() {
            assert_eq!(*sequence, at as i64);
            assert_eq!(stream, "stdout");
            // text, then its newline, as separate writes
            if at % 2 == 1 {
                assert_eq!(text.code_points(), [u32::from('\n')]);
            } else {
                assert!(!text.is_empty() || at > 0);
            }
        }
        assert_eq!(seen.writes.len() % 2, 0);
        let joined: Vec<u8> = seen
            .writes
            .iter()
            .flat_map(|(_, _, text)| text.to_utf8().expect("text").into_bytes())
            .collect();
        let saved = bytes_replace_all(
            &hex_decode(&text_of(member(case, "stdout_hex"))),
            &[("@CASE@", root_text(&work.area))],
        );
        assert_eq!(joined, saved);
        let record = record_of(&seen);
        assert!(same_value(
            &without(record, &["args_after", "kwargs_after"]),
            &j(
                r#"{"id": 1, "kind": "value", "value": {"kind": "none"}, "stdout": "", "stderr": ""}"#
            )
        ));
        assert_eq!(work.standin.state().counter, Some(7));
    }

    /// `Prepared` of the earlier checks, for a saved case of the stdio bundle.
    fn reference_cases_prepare(bundle: &PyValue, case: &PyValue) -> Prepared {
        prepare(bundle, case, "events")
    }

    #[test]
    fn a_write_that_fails_stops_the_business_code_at_that_print_before_the_next_command() {
        // first failing write: the text of the first removal line
        let work = table_lab("events-rmi");
        let line = main_request(&work, &["--apply"]);
        let mut at = None;
        let seen = session(&work, &line, b"", |sequence, _stream, text| {
            if at.is_none()
                && text
                    .to_utf8()
                    .is_some_and(|t| t.starts_with("[cleanup] rmi "))
            {
                at = Some(sequence);
                Some(broken(sequence))
            } else {
                Some(ok(sequence))
            }
        });
        assert_eq!(seen.status, Some(0), "{}", seen.stderr);
        let failed = at.expect("a removal line was written");
        assert_eq!(
            seen.writes.last().map(|(sequence, _, _)| *sequence),
            Some(failed)
        );
        assert_eq!(
            exception_of(record_of(&seen)),
            (
                "BrokenPipeError".to_string(),
                "[Errno 32] Broken pipe".to_string()
            )
        );
        // the container query, the listing, the usage report and the FIRST removal ran: the failed print was the one after it
        assert_eq!(
            work.standin.calls(),
            [
                vec!["ps", "-a", "--format", "{{.Image}}"],
                vec!["image", "ls", "-a", "--format", "{{json .}}"],
                vec!["system", "df"],
                vec!["rmi", "a:b"]
            ]
        );
        assert_eq!(text_of(member(record_of(&seen), "stdout")), "");
        // the newline of the very first print is its own write: failing it stops before everything else
        let first = table_lab("events-newline");
        let seen = session(
            &first,
            &main_request(&first, &["--apply"]),
            b"",
            |sequence, _stream, _text| {
                Some(if sequence == 1 {
                    broken(sequence)
                } else {
                    ok(sequence)
                })
            },
        );
        assert_eq!(seen.writes.len(), 2);
        assert_eq!(
            first.standin.calls(),
            [["ps", "-a", "--format", "{{.Image}}"]]
        );
        assert_eq!(exception_of(record_of(&seen)).0, "BrokenPipeError");
    }

    #[test]
    fn an_encode_failure_acknowledged_with_its_constructor_is_the_exception_of_that_write_with_the_fields_given(
    ) {
        let work = table_lab("events-encode");
        let ctor = r#"{"kind": "unicode_encode", "encoding": "utf-8", "object": "[cleanup] x\ud800", "start": 11, "end": 12, "reason": "surrogates not allowed"}"#;
        let message =
            "'utf-8' codec can't encode character '\\ud800' in position 11: surrogates not allowed";
        let seen = session(
            &work,
            &main_request(&work, &[]),
            b"",
            |sequence, _stream, _text| {
                Some(if sequence == 0 {
                    failure(sequence, "UnicodeEncodeError", message, Some(ctor))
                } else {
                    ok(sequence)
                })
            },
        );
        assert_eq!(seen.status, Some(0), "{}", seen.stderr);
        assert_eq!(
            exception_of(record_of(&seen)),
            ("UnicodeEncodeError".to_string(), message.to_string())
        );
        let row = &items(member(record_of(&seen), "exception_chain"))[0];
        assert_eq!(text_of(member(row, "module")), "builtins");
        assert!(same_value(member(row, "constructor"), &j(ctor)));
        assert_eq!(seen.writes.len(), 1);
        assert_eq!(work.standin.calls().len(), 1);
        // a decode constructor stays a decode constructor
        let dctor = r#"{"kind": "unicode_decode", "encoding": "ascii", "object_hex": "00ff", "start": 1, "end": 2, "reason": "ordinal not in range(128)"}"#;
        let again = table_lab("events-decode");
        let seen = session(
            &again,
            &main_request(&again, &[]),
            b"",
            |sequence, _stream, _text| {
                Some(if sequence == 0 {
                    failure(sequence, "UnicodeDecodeError", "'ascii' codec can't decode byte 0xff in position 1: ordinal not in range(128)", Some(dctor))
                } else {
                    ok(sequence)
                })
            },
        );
        let row = &items(member(record_of(&seen), "exception_chain"))[0];
        assert!(same_value(member(row, "constructor"), &j(dctor)));
    }

    #[test]
    fn a_missing_or_invalid_acknowledgement_abandons_the_run_with_no_result_and_no_later_effect() {
        type Answer = Box<dyn Fn(i64) -> Option<String>>;
        let cases: Vec<(&str, Answer)> = vec![
            ("eof", Box::new(|_| None)),
            ("sequence", Box::new(|n| Some(ok(n + 1)))),
            ("json", Box::new(|_| Some("not json".to_string()))),
            (
                "kind",
                Box::new(|n| {
                    Some(format!(
                        "{{\"event\": \"write_ack\", \"sequence\": {n}, \"kind\": \"maybe\"}}"
                    ))
                }),
            ),
            (
                "extra",
                Box::new(|n| {
                    Some(format!("{{\"event\": \"write_ack\", \"sequence\": {n}, \"kind\": \"ok\", \"x\": 1}}"))
                }),
            ),
            (
                "event",
                Box::new(|n| {
                    Some(format!(
                        "{{\"event\": \"other\", \"sequence\": {n}, \"kind\": \"ok\"}}"
                    ))
                }),
            ),
            (
                "no-ctor",
                Box::new(|n| Some(failure(n, "UnicodeEncodeError", "m", None))),
            ),
            (
                "wrong-ctor",
                Box::new(|n| {
                    Some(failure(
                        n,
                        "OSError",
                        "m",
                        Some(
                            r#"{"kind": "unicode_decode", "encoding": "u", "object_hex": "00", "start": 0, "end": 1, "reason": "r"}"#,
                        ),
                    ))
                }),
            ),
            (
                "bad-index",
                Box::new(|n| {
                    Some(failure(
                        n,
                        "UnicodeEncodeError",
                        "m",
                        Some(
                            r#"{"kind": "unicode_encode", "encoding": "u", "object": "x", "start": -1, "end": 1, "reason": "r"}"#,
                        ),
                    ))
                }),
            ),
        ];
        for (name, make) in cases {
            let work = table_lab(&format!("events-abandon-{name}"));
            let seen = session(
                &work,
                &main_request(&work, &["--apply"]),
                b"",
                |sequence, _stream, _text| make(sequence),
            );
            assert_eq!(seen.status, Some(3), "{name}: {}", seen.stderr);
            assert!(seen.result.is_none(), "{name}");
            assert!(!seen.stderr.is_empty(), "{name}");
            // only the container query that precedes the first print ran; no write event followed the bad acknowledgement
            assert_eq!(work.standin.state().counter, Some(1), "{name}");
            assert_eq!(seen.writes.len(), 1, "{name}");
        }
    }

    #[test]
    fn the_parsers_own_writes_are_announced_and_their_failure_stays_suppressed() {
        let work = table_lab("events-parser");
        let seen = session(
            &work,
            &main_request(&work, &["--bogus"]),
            b"",
            |sequence, _stream, _text| Some(broken(sequence)),
        );
        assert_eq!(seen.status, Some(0), "{}", seen.stderr);
        assert_eq!(seen.writes.len(), 2);
        assert!(seen.writes.iter().all(|(_, stream, _)| stream == "stderr"));
        assert!(seen.writes[0]
            .2
            .to_utf8()
            .expect("text")
            .starts_with("usage: cleanup_docker.py"));
        assert_eq!(
            seen.writes[1].2.to_utf8().expect("text"),
            "cleanup_docker.py: error: unrecognized arguments: --bogus\n"
        );
        let (class, message) = exception_of(record_of(&seen));
        assert_eq!((class.as_str(), message.as_str()), ("SystemExit", "2"));
        assert_eq!(
            int_of(member(
                &items(member(record_of(&seen), "exception_chain"))[0],
                "code"
            )),
            2
        );
        assert_eq!(work.standin.state().counter, Some(0));
        // help is one write to standard output, also suppressed
        let help = table_lab("events-help");
        let seen = session(
            &help,
            &main_request(&help, &["-h"]),
            b"",
            |sequence, _stream, _text| Some(broken(sequence)),
        );
        assert_eq!(seen.writes.len(), 1);
        assert_eq!(seen.writes[0].1, "stdout");
        assert_eq!(exception_of(record_of(&seen)).1, "0");
    }

    #[test]
    fn a_failed_listing_is_two_stderr_writes_and_a_failing_one_replaces_the_exit() {
        let plan = vec![
            exchange(&["ps", "-a", "--format", "{{.Image}}"], b"", 1),
            exchange(
                &["image", "ls", "-a", "--format", "{{json .}}"],
                b"gone\n",
                1,
            ),
        ];
        let work = lab("events-listing", &plan, false);
        let seen = session(&work, &main_request(&work, &[]), b"", accept_all);
        let stderr_writes: Vec<String> = seen
            .writes
            .iter()
            .filter(|(_, stream, _)| stream == "stderr")
            .map(|(_, _, text)| text.to_utf8().expect("text"))
            .collect();
        assert_eq!(
            stderr_writes,
            ["[cleanup] FAIL: docker image ls 실패 — gone", "\n"]
        );
        assert_eq!(
            exception_of(record_of(&seen)),
            ("SystemExit".to_string(), "2".to_string())
        );
        // sequences run on across the two streams without a gap
        for (at, (sequence, _, _)) in seen.writes.iter().enumerate() {
            assert_eq!(*sequence, at as i64);
        }
        let again = lab("events-listing-broken", &plan, false);
        let seen = session(
            &again,
            &main_request(&again, &[]),
            b"",
            |sequence, stream, _text| {
                Some(if stream == "stderr" {
                    broken(sequence)
                } else {
                    ok(sequence)
                })
            },
        );
        assert_eq!(
            exception_of(record_of(&seen)),
            (
                "BrokenPipeError".to_string(),
                "[Errno 32] Broken pipe".to_string()
            )
        );
    }

    #[test]
    fn requests_are_answered_by_legacy_records_descriptor_zero_stays_the_callers_and_a_lone_surrogate_is_announced_as_text(
    ) {
        let work = table_lab("events-misc");
        // an invalid request and a refused globals member: a result and no write
        for line in [
            "{\"id\": 1, \"operation\": \"cleanup_docker.main\", \"args\": [], \"globals\": {\"other\": 1}}".to_string(),
            "not json".to_string(),
            r#"{"id": 2, "operation": "other.main"}"#.to_string(),
        ] {
            let seen = session(&work, &line, b"", accept_all);
            assert_eq!(seen.status, Some(0), "{line}: {}", seen.stderr);
            assert!(seen.writes.is_empty());
            assert_eq!(text_of(member(record_of(&seen), "kind")), "exception");
        }
        // a child inherits descriptor 0 untouched by the protocol: its line comes from the caller's own standard input
        let reading = lab(
            "events-stdin",
            &[Exchange {
                argv: vec!["probe".to_string()],
                behaviour: Some("read_stdin".to_string()),
                ..Exchange::default()
            }],
            false,
        );
        let line = request(
            1,
            "_run",
            "[[\"docker\", \"probe\"], 5]",
            "{}",
            &globals_text(&reading),
        );
        let seen = session(&reading, &line, b"from the caller\n", accept_all);
        assert_eq!(
            run_value(record_of(&seen)),
            (0, "from the caller".to_string())
        );
        // a lone surrogate in a printed text reaches the parent as text (JSON-escaped); the parent's stream decides
        let lone = table_lab("events-surrogate");
        let argv = main_request_json(&lone, &[jstr("--keep"), r#""\ud800""#.to_string()]);
        let seen = session(&lone, &argv, b"", accept_all);
        assert_eq!(seen.status, Some(0), "{}", seen.stderr);
        assert!(seen
            .writes
            .iter()
            .any(|(_, _, text)| text.code_points().contains(&0xD800)));
        assert_eq!(text_of(member(record_of(&seen), "kind")), "value");
    }

    #[test]
    fn the_operation_needs_a_protocol_descriptor_above_2() {
        let work = table_lab("events-usage");
        for args in [
            vec!["evaluate-events"],
            vec!["evaluate-events", "--input-fd", "1"],
            vec!["evaluate-events", "--input-fd", "x"],
            vec!["evaluate-events", "extra"],
        ] {
            let mut run = command(
                env!("CARGO_BIN_EXE_recipe_cleanup_api"),
                &work.area,
                &bin_path(&work.standin),
            );
            run.args(&args);
            let output = finish(run, None);
            assert_eq!(output.status.code(), Some(2), "{args:?}");
            assert!(output.stdout.is_empty(), "{args:?}");
        }
    }

    // ── N3: argparse's `_print_message` swallows only AttributeError and OSError ─────────────────

    /// An exception acknowledgement of an arbitrary module whose real builtin ancestry is `bases` (as reflected, `object` last).
    fn failure_in(
        sequence: i64,
        class: &str,
        module: &str,
        message: &str,
        bases: &[&str],
    ) -> String {
        failure_full(
            sequence,
            class,
            module,
            message,
            None,
            Some(&bases_json(bases)),
        )
    }

    /// What a CPython `TextIOWrapper(encoding="ascii")` raises for `text`: the constructor fields and message of the actual
    /// `UnicodeEncodeError` (the first run of characters above 127), from the pinned encoder's rules.
    fn ascii_failure(sequence: i64, text: &PyStr) -> String {
        let points = text.code_points();
        let start = points
            .iter()
            .position(|&point| point > 127)
            .expect("a non-ASCII character");
        let end = start
            + points[start..]
                .iter()
                .take_while(|&&point| point > 127)
                .count();
        let message = if end - start == 1 {
            format!(
                "'ascii' codec can't encode character '\\u{:04x}' in position {start}: ordinal not in range(128)",
                points[start]
            )
        } else {
            format!(
                "'ascii' codec can't encode characters in position {start}-{}: ordinal not in range(128)",
                end - 1
            )
        };
        let object = PyValue::Str(text.clone()).dumps_ascii().expect("text");
        let ctor = format!(
            "{{\"kind\": \"unicode_encode\", \"encoding\": \"ascii\", \"object\": {object}, \"start\": {start}, \"end\": {end}, \"reason\": \"ordinal not in range(128)\"}}"
        );
        failure(sequence, "UnicodeEncodeError", &message, Some(&ctor))
    }

    fn rows(record: &PyValue) -> &[PyValue] {
        items(member(record, "exception_chain"))
    }

    #[test]
    fn a_help_write_that_fails_with_a_class_argparse_does_not_swallow_is_the_exception_and_no_systemexit_follows(
    ) {
        for (class, message) in [
            ("ValueError", "bad value"),
            ("TypeError", "bad type"),
            ("RuntimeError", "bad run"),
            ("UnicodeError", "bad unicode"),
            ("ProtocolError", "not swallowed"),
        ] {
            let work = table_lab(&format!("help-{class}"));
            let seen = session(
                &work,
                &main_request(&work, &["-h"]),
                b"",
                |sequence, _s, _t| Some(failure(sequence, class, message, None)),
            );
            assert_eq!(seen.status, Some(0), "{class}: {}", seen.stderr);
            assert_eq!(
                seen.writes.len(),
                1,
                "{class}: nothing is written after the failing help write"
            );
            assert_eq!(
                exception_of(record_of(&seen)),
                (class.to_string(), message.to_string())
            );
            assert_eq!(
                rows(record_of(&seen)).len(),
                1,
                "{class}: no context, no SystemExit"
            );
            assert_eq!(work.standin.state().counter, Some(0));
        }
        // the actual encode error of a parent whose stream is ASCII: the Korean help text cannot be written; constructor, message, range
        let work = table_lab("help-ascii");
        let mut shown = None;
        let seen = session(
            &work,
            &main_request(&work, &["-h"]),
            b"",
            |sequence, _stream, text| {
                shown = Some(text.clone());
                Some(ascii_failure(sequence, text))
            },
        );
        let text = shown.expect("the help text");
        assert!(text
            .to_utf8()
            .expect("text")
            .starts_with("usage: cleanup_docker.py"));
        assert_eq!(seen.writes.len(), 1);
        let (class, message) = exception_of(record_of(&seen));
        assert_eq!(class, "UnicodeEncodeError");
        assert!(
            message.starts_with("'ascii' codec can't encode characters in position "),
            "{message}"
        );
        let row = &rows(record_of(&seen))[0];
        let ctor = member(row, "constructor");
        assert_eq!(text_of(member(ctor, "kind")), "unicode_encode");
        assert_eq!(text_of(member(ctor, "encoding")), "ascii");
        let PyValue::Str(object) = member(ctor, "object") else {
            panic!("text")
        };
        assert_eq!(object.code_points(), text.code_points());
        let (start, end) = (
            int_of(member(ctor, "start")) as usize,
            int_of(member(ctor, "end")) as usize,
        );
        assert!(text.code_points()[start] > 127 && end > start);
        assert_eq!(rows(record_of(&seen)).len(), 1);
    }

    #[test]
    fn a_usage_or_error_write_that_fails_with_such_a_class_stops_the_parser_with_that_exception_and_the_argument_error_as_context(
    ) {
        // `--keep` without a value: an error about a named argument, raised inside argparse's `except ArgumentError` handler
        for (call, kept, message_text) in [
            ("usage", 0_i64, "argument --keep: expected one argument"),
            ("message", 1, "argument --keep: expected one argument"),
        ] {
            let work = table_lab(&format!("named-{call}"));
            let seen = session(
                &work,
                &main_request(&work, &["--keep"]),
                b"",
                |sequence, _s, _t| {
                    Some(if sequence == kept {
                        failure(sequence, "ValueError", "write refused", None)
                    } else {
                        ok(sequence)
                    })
                },
            );
            assert_eq!(seen.status, Some(0), "{call}: {}", seen.stderr);
            // the usage, and the message only when the usage write did not fail
            assert_eq!(seen.writes.len() as i64, kept + 1, "{call}");
            assert_eq!(
                exception_of(record_of(&seen)),
                ("ValueError".to_string(), "write refused".to_string())
            );
            let chain = rows(record_of(&seen));
            assert_eq!(chain.len(), 2, "{call}");
            assert_eq!(text_of(member(&chain[0], "relation")), "context");
            assert!(matches!(
                member(&chain[0], "suppress_context"),
                PyValue::Bool(false)
            ));
            assert_eq!(text_of(member(&chain[1], "type")), "ArgumentError");
            assert_eq!(text_of(member(&chain[1], "module")), "argparse");
            assert_eq!(text_of(member(&chain[1], "message")), message_text);
            assert_eq!(work.standin.state().counter, Some(0));
        }
        // an invalid int: the ArgumentError has its own ValueError context, the write failure is outside them
        let work = table_lab("named-int");
        let seen = session(
            &work,
            &main_request(&work, &["--cache-age-hours", "x"]),
            b"",
            |sequence, _s, _t| Some(failure(sequence, "TypeError", "no", None)),
        );
        let chain = rows(record_of(&seen));
        let kinds: Vec<String> = chain
            .iter()
            .map(|row| text_of(member(row, "type")))
            .collect();
        assert_eq!(kinds, ["TypeError", "ArgumentError", "ValueError"]);
        assert_eq!(seen.writes.len(), 1);
        // an error `parse_args` itself reports is not raised from an ArgumentError: no context
        let work = table_lab("plain-error");
        let seen = session(
            &work,
            &main_request(&work, &["--bogus"]),
            b"",
            |sequence, _s, _t| Some(failure(sequence, "TypeError", "no", None)),
        );
        assert_eq!(rows(record_of(&seen)).len(), 1);
        assert_eq!(seen.writes.len(), 1);
        // the encode error of an ASCII parent at the usage write of an error whose message cannot be written either way
        let work = table_lab("usage-ascii");
        let seen = session(
            &work,
            &main_request_json(&work, &[r#""--keep""#.to_string()]),
            b"",
            |sequence, _s, text| {
                Some(if text.code_points().iter().any(|&point| point > 127) {
                    ascii_failure(sequence, text)
                } else {
                    ok(sequence)
                })
            },
        );
        // the usage and the message are ASCII: both written, the exit is the SystemExit
        assert_eq!(exception_of(record_of(&seen)).0, "SystemExit");
    }

    #[test]
    fn the_oserror_family_and_attributeerror_stay_swallowed_and_the_parser_ends_as_it_did() {
        let swallowed: [(&str, &str, Vec<&str>); 11] = [
            ("OSError", "builtins", default_bases("OSError")),
            (
                "FileNotFoundError",
                "builtins",
                default_bases("FileNotFoundError"),
            ),
            (
                "PermissionError",
                "builtins",
                default_bases("PermissionError"),
            ),
            (
                "BrokenPipeError",
                "builtins",
                default_bases("BrokenPipeError"),
            ),
            ("TimeoutError", "builtins", default_bases("TimeoutError")),
            (
                "ConnectionError",
                "builtins",
                default_bases("ConnectionError"),
            ),
            (
                "ConnectionResetError",
                "builtins",
                default_bases("ConnectionResetError"),
            ),
            (
                "BlockingIOError",
                "builtins",
                default_bases("BlockingIOError"),
            ),
            (
                "InterruptedError",
                "builtins",
                default_bases("InterruptedError"),
            ),
            (
                "AttributeError",
                "builtins",
                default_bases("AttributeError"),
            ),
            // `io.UnsupportedOperation(OSError, ValueError)`
            (
                "UnsupportedOperation",
                "io",
                vec![
                    "OSError",
                    "ValueError",
                    "Exception",
                    "BaseException",
                    "object",
                ],
            ),
        ];
        for (class, module, bases) in swallowed {
            let work = table_lab(&format!("swallow-{class}"));
            let seen = session(
                &work,
                &main_request(&work, &["-h"]),
                b"",
                |sequence, _s, _t| Some(failure_in(sequence, class, module, "swallowed", &bases)),
            );
            assert_eq!(
                exception_of(record_of(&seen)),
                ("SystemExit".to_string(), "0".to_string()),
                "{class}"
            );
            assert_eq!(seen.writes.len(), 1);
            // the usage write fails: the message is still written, then the SystemExit(2)
            let again = table_lab(&format!("swallow-usage-{class}"));
            let seen = session(
                &again,
                &main_request(&again, &["--bogus"]),
                b"",
                |sequence, _s, _t| Some(failure_in(sequence, class, module, "swallowed", &bases)),
            );
            assert_eq!(seen.writes.len(), 2, "{class}");
            assert_eq!(
                exception_of(record_of(&seen)),
                ("SystemExit".to_string(), "2".to_string()),
                "{class}"
            );
        }
    }

    /// Classification is by the real ancestry the caller reflected, never by the immediate class and module name.
    #[test]
    fn the_real_builtin_ancestry_decides_the_suppression_of_external_classes_and_a_look_alike_name_decides_nothing(
    ) {
        let oserror = vec!["OSError", "Exception", "BaseException", "object"];
        let attribute = vec!["AttributeError", "Exception", "BaseException", "object"];
        let value = vec!["ValueError", "Exception", "BaseException", "object"];
        let swallowed: [(&str, &str, &Vec<&str>); 5] = [
            ("SSLError", "ssl", &oserror),
            ("MyIOError", "mine.io", &oserror),
            ("OSError", "mine", &oserror),
            ("FrozenInstanceError", "dataclasses", &attribute),
            ("AttributeError", "mine", &attribute),
        ];
        for (class, module, bases) in swallowed {
            let help = table_lab(&format!("anc-help-{class}-{module}"));
            let seen = session(
                &help,
                &main_request(&help, &["-h"]),
                b"",
                |sequence, _s, _t| Some(failure_in(sequence, class, module, "swallowed", bases)),
            );
            assert_eq!(
                exception_of(record_of(&seen)),
                ("SystemExit".to_string(), "0".to_string()),
                "{module}.{class}"
            );
            let usage = table_lab(&format!("anc-usage-{class}-{module}"));
            let seen = session(
                &usage,
                &main_request(&usage, &["--bogus"]),
                b"",
                |sequence, _s, _t| Some(failure_in(sequence, class, module, "swallowed", bases)),
            );
            assert_eq!(seen.writes.len(), 2, "{module}.{class}");
            assert_eq!(
                exception_of(record_of(&seen)),
                ("SystemExit".to_string(), "2".to_string()),
                "{module}.{class}"
            );
        }
        // an external ValueError subtype is not an OSError: it propagates with its own type, module and message; inside the handler
        // of an argument error it has that error as context
        let work = table_lab("anc-value-help");
        let seen = session(
            &work,
            &main_request(&work, &["-h"]),
            b"",
            |sequence, _s, _t| {
                Some(failure_in(
                    sequence,
                    "MyValueError",
                    "mine",
                    "mine refused",
                    &value,
                ))
            },
        );
        assert_eq!(seen.writes.len(), 1);
        assert_eq!(
            exception_of(record_of(&seen)),
            ("MyValueError".to_string(), "mine refused".to_string())
        );
        let row = &rows(record_of(&seen))[0];
        assert_eq!(text_of(member(row, "module")), "mine");
        assert_eq!(rows(record_of(&seen)).len(), 1);
        assert!(
            !has(row, "builtin_bases"),
            "the descriptor is not part of the result chain"
        );
        let named = table_lab("anc-value-named");
        let seen = session(
            &named,
            &main_request(&named, &["--keep"]),
            b"",
            |sequence, _s, _t| {
                Some(failure_in(
                    sequence,
                    "MyValueError",
                    "mine",
                    "mine refused",
                    &value,
                ))
            },
        );
        assert_eq!(
            seen.writes.len(),
            1,
            "no second write after the failing usage write"
        );
        let chain = rows(record_of(&seen));
        assert_eq!(chain.len(), 2);
        assert_eq!(text_of(member(&chain[0], "relation")), "context");
        assert_eq!(text_of(member(&chain[0], "module")), "mine");
        assert_eq!(text_of(member(&chain[1], "type")), "ArgumentError");
        // a class that only LOOKS like an OSError (name `OSError`, module `mine`, no OSError in its ancestry) is not swallowed
        let spoof = table_lab("anc-spoof");
        let seen = session(
            &spoof,
            &main_request(&spoof, &["-h"]),
            b"",
            |sequence, _s, _t| {
                Some(failure_in(
                    sequence,
                    "OSError",
                    "mine",
                    "pretending",
                    &value,
                ))
            },
        );
        assert_eq!(
            exception_of(record_of(&seen)),
            ("OSError".to_string(), "pretending".to_string())
        );
        assert_eq!(rows(record_of(&seen)).len(), 1);
        // the same name in module `builtins` is still only a serialised tag: `type("OSError", (Exception,), {"__module__": "builtins"})` is a real
        // class whose canonical ancestry is Exception, BaseException. The descriptor is valid; the exception propagates as it is, unsuppressed.
        let lie = table_lab("anc-lookalike-builtins");
        let seen = session(
            &lie,
            &main_request(&lie, &["-h"]),
            b"",
            |sequence, _s, _t| {
                Some(failure_in(
                    sequence,
                    "OSError",
                    "builtins",
                    "look-alike",
                    &["Exception", "BaseException"],
                ))
            },
        );
        assert_eq!(seen.status, Some(0), "{}", seen.stderr);
        assert_eq!(
            exception_of(record_of(&seen)),
            ("OSError".to_string(), "look-alike".to_string())
        );
        assert_eq!(seen.writes.len(), 1);
        assert_eq!(
            text_of(member(&rows(record_of(&seen))[0], "module")),
            "builtins"
        );
    }

    /// An ordinary `print` failure (outside argparse) of a class the Rust owner cannot know is that exception, with its real module and type.
    #[test]
    fn an_external_failure_of_an_ordinary_print_is_the_business_exception_with_its_own_class_and_a_constructor_class_is_judged_by_its_ancestry(
    ) {
        let work = table_lab("anc-print");
        let seen = session(&work, &main_request(&work, &[]), b"", |sequence, _s, _t| {
            Some(failure_in(
                sequence,
                "UnsupportedOperation",
                "io",
                "not writable",
                &[
                    "OSError",
                    "ValueError",
                    "Exception",
                    "BaseException",
                    "object",
                ],
            ))
        });
        assert_eq!(
            exception_of(record_of(&seen)),
            (
                "UnsupportedOperation".to_string(),
                "not writable".to_string()
            )
        );
        assert_eq!(text_of(member(&rows(record_of(&seen))[0], "module")), "io");
        assert_eq!(
            work.standin.state().counter,
            Some(1),
            "stopped at that print"
        );
        // a subclass of UnicodeEncodeError of another module: its constructor is accepted by the ancestry, and refused without it
        let ctor = r#"{"kind": "unicode_encode", "encoding": "ascii", "object": "x", "start": 0, "end": 1, "reason": "r"}"#;
        let bases = bases_json(&[
            "UnicodeEncodeError",
            "UnicodeError",
            "ValueError",
            "Exception",
            "BaseException",
            "object",
        ]);
        let sub = table_lab("anc-sub");
        let seen = session(&sub, &main_request(&sub, &[]), b"", |sequence, _s, _t| {
            Some(failure_full(
                sequence,
                "MyEncodeError",
                "mine",
                "m",
                Some(ctor),
                Some(&bases),
            ))
        });
        assert_eq!(exception_of(record_of(&seen)).0, "MyEncodeError");
        assert!(same_value(
            member(&rows(record_of(&seen))[0], "constructor"),
            &j(ctor)
        ));
        let without = table_lab("anc-sub-bare");
        let seen = session(
            &without,
            &main_request(&without, &[]),
            b"",
            |sequence, _s, _t| {
                Some(failure_full(
                    sequence,
                    "MyEncodeError",
                    "mine",
                    "m",
                    None,
                    Some(&bases),
                ))
            },
        );
        assert_eq!(seen.status, Some(3));
        assert!(seen.result.is_none());
        // an exception that is not a Unicode class has no constructor
        let plain = table_lab("anc-plain-ctor");
        let seen = session(
            &plain,
            &main_request(&plain, &[]),
            b"",
            |sequence, _s, _t| {
                Some(failure_full(
                    sequence,
                    "MyError",
                    "mine",
                    "m",
                    Some(ctor),
                    Some(&bases_json(&["Exception", "BaseException", "object"])),
                ))
            },
        );
        assert_eq!(seen.status, Some(3));
    }

    #[test]
    fn a_missing_or_malformed_ancestry_descriptor_abandons_the_run_whatever_the_write() {
        let good = ["ValueError", "Exception", "BaseException", "object"];
        let cases: Vec<(&str, Option<String>)> = vec![
            ("missing", None),
            ("not-a-list", Some("\"ValueError\"".to_string())),
            ("null", Some("null".to_string())),
            ("empty", Some("[]".to_string())),
            (
                "number",
                Some("[\"ValueError\", 1, \"BaseException\"]".to_string()),
            ),
            (
                "empty-name",
                Some("[\"ValueError\", \"\", \"BaseException\"]".to_string()),
            ),
            (
                "duplicate",
                Some(bases_json(&[
                    "ValueError",
                    "Exception",
                    "Exception",
                    "BaseException",
                ])),
            ),
            (
                "no-base-exception",
                Some(bases_json(&["ValueError", "Exception"])),
            ),
            (
                "tail",
                Some(bases_json(&["ValueError", "BaseException", "Exception"])),
            ),
            (
                "object-first",
                Some(bases_json(&["object", "ValueError", "BaseException"])),
            ),
        ];
        for (name, descriptor) in cases {
            for (target, args) in [
                ("help", vec!["-h"]),
                ("usage", vec!["--bogus"]),
                ("print", vec![]),
            ] {
                let work = table_lab(&format!("bad-bases-{name}-{target}"));
                let seen = session(
                    &work,
                    &main_request(&work, &args),
                    b"",
                    |sequence, _s, _t| {
                        Some(failure_full(
                            sequence,
                            "ValueError",
                            "builtins",
                            "m",
                            None,
                            descriptor.as_deref(),
                        ))
                    },
                );
                assert_eq!(seen.status, Some(3), "{name}/{target}: {}", seen.stderr);
                assert!(seen.result.is_none(), "{name}/{target}");
                assert_eq!(seen.writes.len(), 1, "{name}/{target}: no later write");
                assert!(!seen.stderr.is_empty());
            }
        }
        // a descriptor that contradicts the class: an OSError-family builtin without OSError
        let work = table_lab("bad-bases-oserror-family");
        let seen = session(
            &work,
            &main_request(&work, &["-h"]),
            b"",
            |sequence, _s, _t| {
                Some(failure_full(
                    sequence,
                    "BrokenPipeError",
                    "builtins",
                    "m",
                    None,
                    Some(&bases_json(&[
                        "BrokenPipeError",
                        "ConnectionError",
                        "Exception",
                        "BaseException",
                    ])),
                ))
            },
        );
        assert_eq!(seen.status, Some(3));
        // the accepted forms: with and without the trailing `object`, and an external class
        for (module, bases) in [
            ("builtins", bases_json(&good[..3])),
            ("builtins", bases_json(&good)),
            (
                "mine",
                bases_json(&["ValueError", "Exception", "BaseException"]),
            ),
        ] {
            let ok_work = table_lab(&format!("good-bases-{module}-{}", bases.len()));
            let seen = session(
                &ok_work,
                &main_request(&ok_work, &["-h"]),
                b"",
                |sequence, _s, _t| {
                    Some(failure_full(
                        sequence,
                        "ValueError",
                        module,
                        "m",
                        None,
                        Some(&bases),
                    ))
                },
            );
            assert_eq!(seen.status, Some(0), "{module} {bases}: {}", seen.stderr);
            assert_eq!(exception_of(record_of(&seen)).0, "ValueError");
        }
    }

    #[test]
    fn a_missing_or_invalid_acknowledgement_at_a_parser_write_still_abandons_the_run() {
        for (name, args) in [
            ("help", vec!["-h"]),
            ("usage", vec!["--bogus"]),
            ("named", vec!["--keep"]),
        ] {
            for (kind, make) in [
                ("eof", None),
                ("json", Some("nope".to_string())),
                ("sequence", Some(ok(7))),
            ] {
                let work = table_lab(&format!("parser-abort-{name}-{kind}"));
                let seen = session(
                    &work,
                    &main_request(&work, &args),
                    b"",
                    |_sequence, _s, _t| make.clone(),
                );
                assert_eq!(seen.status, Some(3), "{name}/{kind}: {}", seen.stderr);
                assert!(seen.result.is_none(), "{name}/{kind}");
                assert_eq!(seen.writes.len(), 1, "{name}/{kind}: no later write");
                assert!(!seen.stderr.is_empty());
            }
        }
    }

    /// Canonical identity is the reported ancestry only: classes that merely carry a builtin's name and module are judged by their descriptor.
    #[test]
    fn builtin_looking_custom_classes_are_judged_by_their_reported_ancestry_alone() {
        let plain = ["Exception", "BaseException"];
        // (name, module, descriptor, suppressed?): `type(name, (Exception,), {"__module__": "builtins"})` and the like
        let cases: Vec<(&str, &str, Vec<&str>, bool)> = vec![
            ("OSError", "builtins", plain.to_vec(), false),
            ("BrokenPipeError", "builtins", plain.to_vec(), false),
            ("AttributeError", "builtins", plain.to_vec(), false),
            ("UnicodeEncodeError", "builtins", plain.to_vec(), false),
            ("UnicodeDecodeError", "builtins", plain.to_vec(), false),
            ("ConnectionError", "builtins", plain.to_vec(), false),
            ("OSError", "io", plain.to_vec(), false),
            // a custom class NAMED like a builtin that really derives the builtin
            (
                "OSError",
                "builtins",
                vec!["OSError", "Exception", "BaseException"],
                true,
            ),
            (
                "AttributeError",
                "builtins",
                vec!["AttributeError", "Exception", "BaseException"],
                true,
            ),
            (
                "ValueError",
                "builtins",
                vec!["OSError", "ValueError", "Exception", "BaseException"],
                true,
            ),
            // a ValueError subclass named after an OSError family member that derives only ValueError
            (
                "BrokenPipeError",
                "mine",
                vec!["ValueError", "Exception", "BaseException"],
                false,
            ),
            // the standard builtins stay as they were
            (
                "BrokenPipeError",
                "builtins",
                default_bases("BrokenPipeError"),
                true,
            ),
            (
                "AttributeError",
                "builtins",
                default_bases("AttributeError"),
                true,
            ),
            ("ValueError", "builtins", default_bases("ValueError"), false),
        ];
        for (at, (class, module, bases, swallowed)) in cases.iter().enumerate() {
            let help = table_lab(&format!("identity-help-{at}"));
            let seen = session(
                &help,
                &main_request(&help, &["-h"]),
                b"",
                |sequence, _s, _t| Some(failure_in(sequence, class, module, "tag", bases)),
            );
            assert_eq!(
                seen.status,
                Some(0),
                "{module}.{class} {bases:?}: {}",
                seen.stderr
            );
            assert_eq!(seen.writes.len(), 1, "{module}.{class}");
            let got = exception_of(record_of(&seen));
            if *swallowed {
                assert_eq!(
                    got,
                    ("SystemExit".to_string(), "0".to_string()),
                    "{module}.{class} {bases:?}"
                );
            } else {
                assert_eq!(
                    got,
                    (class.to_string(), "tag".to_string()),
                    "{module}.{class} {bases:?}"
                );
                assert_eq!(rows(record_of(&seen)).len(), 1);
                assert_eq!(
                    text_of(member(&rows(record_of(&seen))[0], "module")),
                    *module
                );
                assert!(
                    !has(&rows(record_of(&seen))[0], "constructor"),
                    "no guessed constructor"
                );
            }
            // the same at the usage of an error about a named argument: the failure is either swallowed (SystemExit 2 after the
            // message) or the business exception with the handler's ArgumentError context and no second write
            let named = table_lab(&format!("identity-named-{at}"));
            let seen = session(
                &named,
                &main_request(&named, &["--keep"]),
                b"",
                |sequence, _s, _t| Some(failure_in(sequence, class, module, "tag", bases)),
            );
            assert_eq!(seen.status, Some(0), "{module}.{class}: {}", seen.stderr);
            if *swallowed {
                assert_eq!(seen.writes.len(), 2);
                assert_eq!(
                    exception_of(record_of(&seen)),
                    ("SystemExit".to_string(), "2".to_string())
                );
            } else {
                assert_eq!(seen.writes.len(), 1, "{module}.{class}: no second write");
                let chain = rows(record_of(&seen));
                assert_eq!(chain.len(), 2, "{module}.{class}");
                assert_eq!(text_of(member(&chain[0], "relation")), "context");
                assert_eq!(text_of(member(&chain[1], "type")), "ArgumentError");
            }
        }
        // a Unicode-looking class with no Unicode ancestor has no constructor and needs none; with one it is malformed
        let ctor = r#"{"kind": "unicode_encode", "encoding": "ascii", "object": "x", "start": 0, "end": 1, "reason": "r"}"#;
        let bare = table_lab("identity-ctor-bare");
        let seen = session(&bare, &main_request(&bare, &[]), b"", |sequence, _s, _t| {
            Some(failure_full(
                sequence,
                "UnicodeEncodeError",
                "builtins",
                "m",
                None,
                Some(&bases_json(&plain)),
            ))
        });
        assert_eq!(exception_of(record_of(&seen)).0, "UnicodeEncodeError");
        let wrong = table_lab("identity-ctor-wrong");
        let seen = session(
            &wrong,
            &main_request(&wrong, &[]),
            b"",
            |sequence, _s, _t| {
                Some(failure_full(
                    sequence,
                    "UnicodeEncodeError",
                    "builtins",
                    "m",
                    Some(ctor),
                    Some(&bases_json(&plain)),
                ))
            },
        );
        assert_eq!(seen.status, Some(3));
        assert!(seen.result.is_none());
        // a real encode error keeps its constructor (named by its true ancestry)
        let real = table_lab("identity-ctor-real");
        let seen = session(&real, &main_request(&real, &[]), b"", |sequence, _s, _t| {
            Some(failure_full(
                sequence,
                "UnicodeEncodeError",
                "builtins",
                "m",
                Some(ctor),
                Some(&bases_json(&default_bases("UnicodeEncodeError"))),
            ))
        });
        assert!(same_value(
            member(&rows(record_of(&seen))[0], "constructor"),
            &j(ctor)
        ));
        // a descriptor that names an OSError-family class without OSError, or a Unicode class without its parents, is still malformed
        for bases in [
            bases_json(&["ConnectionError", "Exception", "BaseException"]),
            bases_json(&[
                "UnicodeDecodeError",
                "ValueError",
                "Exception",
                "BaseException",
            ]),
        ] {
            let bad = table_lab("identity-malformed");
            let seen = session(
                &bad,
                &main_request(&bad, &["-h"]),
                b"",
                |sequence, _s, _t| {
                    Some(failure_full(
                        sequence,
                        "MyError",
                        "mine",
                        "m",
                        None,
                        Some(&bases),
                    ))
                },
            );
            assert_eq!(seen.status, Some(3), "{bases}");
        }
    }
}
