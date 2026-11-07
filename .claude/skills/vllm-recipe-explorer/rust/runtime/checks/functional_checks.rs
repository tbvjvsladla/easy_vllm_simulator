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
        .join("functional_reference")
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
        PrivateDirectory::create(&format!("functional-{name}-")).expect("a private directory");
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

// ── the owned peer: one ephemeral IPv4 loopback HTTP server, with explicit faults ────────────────

use std::io::Read;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// One response of the peer's cyclic list: the status, the body bytes and an optional fault (`abort`, `stall_headers`, `stall_body`).
#[derive(Clone)]
struct Answer {
    status: u16,
    body: Vec<u8>,
    behavior: Option<String>,
    /// Bytes sent instead of the status line and headers (a raw, possibly invalid, answer).
    raw: Option<Vec<u8>>,
}

#[derive(Clone)]
struct Seen {
    index: usize,
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

struct Peer {
    base: String,
    host: String,
    seen: Arc<Mutex<Vec<Seen>>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

fn phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        302 => "Found",
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Unknown",
    }
}

fn serve_one(
    mut stream: TcpStream,
    answers: &[Answer],
    record: bool,
    seen: &Mutex<Vec<Seen>>,
    count: &AtomicUsize,
) {
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(5)));
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        if let Some(at) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            break at + 4;
        }
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(read) => buffer.extend_from_slice(&chunk[..read]),
        }
    };
    let head = String::from_utf8_lossy(&buffer[..head_end]).into_owned();
    let mut lines = head.split("\r\n");
    let request_line = lines.next().unwrap_or_default().to_string();
    let mut parts = request_line.split(' ');
    let (method, path) = (
        parts.next().unwrap_or_default().to_string(),
        parts.next().unwrap_or_default().to_string(),
    );
    let headers: Vec<(String, String)> = lines
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            line.split_once(": ")
                .map(|(name, value)| (name.to_string(), value.to_string()))
        })
        .collect();
    let length = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = buffer[head_end..].to_vec();
    while body.len() < length {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(read) => body.extend_from_slice(&chunk[..read]),
        }
    }
    let index = count.fetch_add(1, Ordering::SeqCst);
    if record {
        seen.lock().expect("lock").push(Seen {
            index,
            method,
            path,
            headers,
            body,
        });
    }
    let answer = &answers[index % answers.len()];
    if answer.behavior.as_deref() == Some("abort") {
        return;
    }
    if let Some(raw) = &answer.raw {
        let _ = stream.write_all(raw);
        let _ = stream.flush();
        return;
    }
    if answer.behavior.as_deref() != Some("stall_headers") {
        let extra = usize::from(answer.behavior.as_deref() == Some("stall_body"));
        let head = format!(
            "HTTP/1.0 {} {}\r\nServer: BaseHTTP/0.6 Python/3.12.3\r\nDate: Fri, 06 Nov 2026 00:00:00 GMT\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            answer.status,
            phrase(answer.status),
            answer.body.len() + extra
        );
        let _ = stream.write_all(head.as_bytes());
        let _ = stream.write_all(&answer.body);
        let _ = stream.flush();
    }
    if matches!(
        answer.behavior.as_deref(),
        Some("stall_headers" | "stall_body")
    ) {
        // Never send what the client waits for: it ends its own read by its timeout (or the peer ends after three seconds).
        let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(3)));
        while matches!(stream.read(&mut chunk), Ok(read) if read > 0) {}
    }
}

impl Peer {
    fn start(answers: Vec<Answer>, record: bool) -> Peer {
        let listener = TcpListener::bind("127.0.0.1:0").expect("an ephemeral loopback port");
        listener.set_nonblocking(true).expect("non-blocking accept");
        let port = listener.local_addr().expect("an address").port();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let count = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let answers = Arc::new(answers);
        let (seen_t, count_t, stop_t) = (seen.clone(), count.clone(), stop.clone());
        let thread = std::thread::spawn(move || {
            let mut workers = Vec::new();
            while !stop_t.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let _ = stream.set_nonblocking(false);
                        let (answers, seen, count) =
                            (answers.clone(), seen_t.clone(), count_t.clone());
                        workers.push(std::thread::spawn(move || {
                            serve_one(stream, &answers, record, &seen, &count)
                        }));
                    }
                    Err(_) => std::thread::sleep(std::time::Duration::from_millis(2)),
                }
            }
            for worker in workers {
                let _ = worker.join();
            }
        });
        Peer {
            base: format!("http://127.0.0.1:{port}"),
            host: format!("127.0.0.1:{port}"),
            seen,
            stop,
            thread: Some(thread),
        }
    }

    fn requests(&self) -> Vec<Seen> {
        self.seen.lock().expect("lock").clone()
    }
}

impl Drop for Peer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The peer a saved case describes (`http.responses`), or none.
fn peer_for(request: &PyValue) -> Option<Peer> {
    let PyValue::Dict(members) = request else {
        return None;
    };
    let Some(PyValue::Dict(http)) = members.get("http") else {
        return None;
    };
    let answers = items(http.get("responses").expect("responses"))
        .iter()
        .map(|each| Answer {
            status: match each {
                PyValue::Dict(map) => match map.get("status") {
                    Some(PyValue::Int(number)) => u16::try_from(number.clone()).expect("a status"),
                    _ => 200,
                },
                _ => 200,
            },
            body: match each {
                PyValue::Dict(map) => map
                    .get("body_hex")
                    .map(|hex| hex_decode(&text_of(hex)))
                    .unwrap_or_default(),
                _ => Vec::new(),
            },
            behavior: match each {
                PyValue::Dict(map) => map.get("behavior").map(text_of),
                _ => None,
            },
            raw: match each {
                PyValue::Dict(map) => map.get("raw_hex").map(|hex| hex_decode(&text_of(hex))),
                _ => None,
            },
        })
        .collect();
    Some(Peer::start(answers, true))
}

fn dummy_base() -> String {
    "http://127.0.0.1:9".to_string()
}

/// The saved record of the peer against what the peer saw: count, order, method, path, every header in order, body bytes.
fn peer_problems(case: &PyValue, peer: Option<&Peer>) -> Vec<String> {
    let saved = member(case, "peer");
    let mut problems = Vec::new();
    match (saved, peer) {
        (PyValue::None, None) => {}
        (PyValue::Dict(_), None) => {
            if !matches!(member(saved, "request_count"), PyValue::Int(count) if *count == 0.into())
            {
                problems.push("the saved peer record has requests but no peer ran".to_string());
            }
        }
        (PyValue::Dict(_), Some(peer)) => {
            let seen = peer.requests();
            let want = items(member(saved, "requests"));
            if seen.len() != want.len()
                || !matches!(member(saved, "request_count"), PyValue::Int(count) if *count == seen.len().into())
            {
                problems.push(format!(
                    "the peer saw {} requests, saved {}",
                    seen.len(),
                    want.len()
                ));
            }
            for (got, saved) in seen.iter().zip(want) {
                let headers: Vec<(String, String)> = items(member(saved, "headers"))
                    .iter()
                    .map(|pair| {
                        (
                            text_of(&items(pair)[0]),
                            text_of(&items(pair)[1]).replace("@HOST@", &peer.host),
                        )
                    })
                    .collect();
                let body = hex_decode(&text_of(member(saved, "body_hex")));
                if got.method != text_of(member(saved, "method"))
                    || got.path != text_of(member(saved, "path"))
                    || got.headers != headers
                    || got.body != body
                {
                    problems.push(format!(
                        "request {} differs: {:?} {:?} {:?}",
                        got.index, got.method, got.path, got.headers
                    ));
                }
            }
        }
        _ => problems.push("the peer and its saved record do not agree on existing".to_string()),
    }
    problems
}

fn replacements(root: &str, peer: Option<&Peer>) -> Vec<(&'static str, String)> {
    let (base, host) = peer.map_or((dummy_base(), "127.0.0.1:9".to_string()), |peer| {
        (peer.base.clone(), peer.host.clone())
    });
    vec![
        ("@CASE@", root.to_string()),
        ("@BASE@", base),
        ("@HOST@", host),
    ]
}

fn bytes_replace_all(bytes: &[u8], replacements: &[(&str, String)]) -> Vec<u8> {
    replacements
        .iter()
        .fold(bytes.to_vec(), |bytes, (marker, with)| {
            replace_bytes(&bytes, marker, with.as_bytes())
        })
}

fn status_text(output: &std::process::Output) -> Option<String> {
    output.status.code().map(|code| code.to_string())
}

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

fn declared_frames(bundle: &PyValue, root: &str) -> Vec<(String, i64, String)> {
    items(member(bundle, "allowed_frames"))
        .iter()
        .map(|frame| {
            let parts = items(frame);
            let number = match &parts[1] {
                PyValue::Int(number) => i64::try_from(number.clone()).expect("a line"),
                other => panic!("{}", other.repr()),
            };
            (
                text_of(&parts[0]).replace("@CASE@", root),
                number,
                text_of(&parts[2]),
            )
        })
        .collect()
}

// ── reference cases ──────────────────────────────────────────────────────────────────────────────

mod reference_cases {
    use super::*;

    fn collector_path() -> String {
        // The collector's path inside the saved stderr is replaced by this marker text in the data; frames name it the same way.
        "@COLLECTOR@".to_string()
    }

    fn fixture_problems(bundle: &PyValue, case: &PyValue, cli: bool) -> Vec<String> {
        let request = member(case, "request");
        let id = text_of(member(request, "id"));
        let work = new_case(bundle, &id, member(case, "before"));
        let (spec, control) = (work.area.join("spec.json"), work.area.join("control.json"));
        write_json(&spec, request);
        let peer = peer_for(request);
        let root = root_text(&work);
        let marks = replacements(&root, peer.as_ref());
        let mut command = umask_command(
            if cli {
                env!("CARGO_BIN_EXE_recipe_functional_fixture_cli")
            } else {
                env!("CARGO_BIN_EXE_recipe_functional_fixture_api")
            },
            &work.root,
        );
        if !cli {
            command.arg("case");
        }
        command
            .arg("--spec")
            .arg(&spec)
            .arg("--root")
            .arg(&work.root)
            .arg("--control")
            .arg(&control)
            .arg("--base")
            .arg(&marks[1].1);
        let output = command.output().expect("run the fixture");
        let mut problems = Vec::new();
        let want_status = if cli {
            match member(case, "exit_code") {
                PyValue::Int(number) => number.to_string(),
                other => panic!("{}", other.repr()),
            }
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
        let want_out = bytes_replace_all(&hex_decode(&text_of(member(case, "stdout_hex"))), &marks);
        if output.stdout != want_out {
            problems.push(format!(
                "stdout differs:\n--- want\n{}\n--- got\n{}",
                String::from_utf8_lossy(&want_out),
                String::from_utf8_lossy(&output.stdout)
            ));
        }
        let saved_err = hex_decode(&text_of(member(case, "stderr_hex")));
        let want_err = if cli {
            match project_traceback(&saved_err, &declared_frames(bundle, "@CASE@")) {
                Ok(projected) => bytes_replace_all(&projected, &marks),
                Err(problem) => {
                    problems.push(format!(
                        "the saved diagnostic is not in the declared shape: {problem}"
                    ));
                    Vec::new()
                }
            }
        } else {
            bytes_replace_all(&saved_err, &marks)
        };
        if !cli && output.stderr != want_err {
            problems.push(format!(
                "stderr differs: {:?}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        if cli && !want_err.is_empty() && output.stderr != want_err {
            problems.push(format!(
                "stderr differs:\n--- want\n{}\n--- got\n{}",
                String::from_utf8_lossy(&want_err),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        if cli && want_err.is_empty() && !output.stderr.is_empty() {
            problems.push(format!(
                "unexpected stderr {:?}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        if control.exists() {
            let got = read_control(&control);
            let saved = without(member(case, "original"), &["python_routing"]);
            let want = rebase_all(&saved, &marks);
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
        problems.extend(peer_problems(case, peer.as_ref()));
        let _ = collector_path();
        problems
            .into_iter()
            .map(|problem| format!("{id}: {problem}"))
            .collect()
    }

    #[test]
    fn the_164_saved_calls_are_reproduced_whole_with_values_exceptions_the_http_trace_and_the_peers_requests(
    ) {
        let bundle = load("bundle.json");
        let cases = items(member(&bundle, "api"));
        assert_eq!(cases.len(), 164);
        let failures: Vec<String> = cases
            .iter()
            .flat_map(|case| fixture_problems(&bundle, case, false))
            .collect();
        assert!(
            failures.is_empty(),
            "{} problems:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    #[test]
    fn the_20_saved_command_lines_end_as_the_original_did_with_their_trace_streams_status_files_and_requests(
    ) {
        let bundle = load("bundle.json");
        let cases = items(member(&bundle, "cli"));
        assert_eq!(cases.len(), 20);
        let failures: Vec<String> = cases
            .iter()
            .flat_map(|case| fixture_problems(&bundle, case, true))
            .collect();
        assert!(
            failures.is_empty(),
            "{} problems:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    fn service_problems(bundle: &PyValue, case: &PyValue) -> Vec<String> {
        let request = member(case, "request");
        let id = text_of(member(request, "id"));
        let work = new_case(bundle, &format!("service-{id}"), member(case, "before"));
        let peer = peer_for(request);
        let root = root_text(&work);
        let marks = replacements(&root, peer.as_ref());
        let mut line = PyDict::new();
        line.set("id", member(request, "id").clone());
        line.set(
            "operation",
            PyValue::str(&format!(
                "functional_smoke.{}",
                text_of(member(request, "function"))
            )),
        );
        line.set("args", rebase_all(member(request, "args"), &marks));
        line.set("kwargs", rebase_all(member(request, "kwargs"), &marks));
        let zero = work.area.join("zero");
        let (mut command, mut writer) = over_descriptor(
            env!("CARGO_BIN_EXE_recipe_functional_api"),
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
                let want = rebase_all(
                    &without(
                        member(case, "original"),
                        &[
                            "owner",
                            "trace",
                            "globals_before",
                            "globals_after",
                            "python_routing",
                        ],
                    ),
                    &marks,
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
        problems.extend(peer_problems(case, peer.as_ref()));
        problems
            .into_iter()
            .map(|problem| format!("{id}: {problem}"))
            .collect()
    }

    #[test]
    fn the_164_saved_calls_agree_through_the_production_service() {
        let bundle = load("bundle.json");
        let failures: Vec<String> = items(member(&bundle, "api"))
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

    fn program_problems(bundle: &PyValue, case: &PyValue) -> Vec<String> {
        let request = member(case, "request");
        let id = text_of(member(request, "id"));
        let work = new_case(bundle, &format!("program-{id}"), member(case, "before"));
        let peer = peer_for(request);
        let root = root_text(&work);
        let marks = replacements(&root, peer.as_ref());
        let args: Vec<String> = items(member(request, "args"))
            .iter()
            .map(|arg| text_of(&rebase_all(arg, &marks)))
            .collect();
        let mut command = umask_command(env!("CARGO_BIN_EXE_functional_smoke"), &work.root);
        command.args(&args);
        let output = command.output().expect("run the program");
        let mut problems = Vec::new();
        let want_status = match member(case, "exit_code") {
            PyValue::Int(number) => number.to_string(),
            other => panic!("{}", other.repr()),
        };
        if status_text(&output).as_deref() != Some(want_status.as_str()) {
            problems.push(format!(
                "status {:?} want {want_status}",
                output.status.code()
            ));
        }
        if output.stdout
            != bytes_replace_all(&hex_decode(&text_of(member(case, "stdout_hex"))), &marks)
        {
            problems.push("stdout differs".to_string());
        }
        let projected = project_traceback(
            &hex_decode(&text_of(member(case, "stderr_hex"))),
            &declared_frames(bundle, "@CASE@"),
        )
        .expect("a declared shape");
        if output.stderr != bytes_replace_all(&projected, &marks) {
            problems.push(format!(
                "stderr differs: {:?}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        problems.extend(tree_difference(
            &expected_tree(bundle, member(case, "after")),
            &snapshot(&work.root),
        ));
        problems.extend(peer_problems(case, peer.as_ref()));
        problems
            .into_iter()
            .map(|problem| format!("{id}: {problem}"))
            .collect()
    }

    /// The six repair inputs saved after the independent source review (chunk framing and the escape of an unencodable request-line character):
    /// the whole saved records through the fixture and the saved values through the production service, with the peers' requests.
    #[test]
    fn the_6_saved_repair_inputs_are_reproduced_whole_through_the_fixture_and_the_production_service(
    ) {
        let bundle = load("repair_bundle.json");
        let cases = items(member(&bundle, "api"));
        assert_eq!(cases.len(), 6);
        let mut failures: Vec<String> = cases
            .iter()
            .flat_map(|case| fixture_problems(&bundle, case, false))
            .collect();
        failures.extend(
            cases
                .iter()
                .flat_map(|case| service_problems(&bundle, case)),
        );
        assert!(
            failures.is_empty(),
            "{} problems:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    #[test]
    fn the_20_saved_command_lines_give_the_same_streams_and_status_as_the_production_program() {
        let bundle = load("bundle.json");
        let failures: Vec<String> = items(member(&bundle, "cli"))
            .iter()
            .flat_map(|case| program_problems(&bundle, case))
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
        env!("CARGO_BIN_EXE_recipe_functional_api"),
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
    fn the_constants_the_globals_member_and_the_refusals_are_as_documented() {
        let output = Command::new(env!("CARGO_BIN_EXE_recipe_functional_api"))
            .arg("constants")
            .env_clear()
            .output()
            .expect("run");
        let got = loads_text(&PyStr::from_surrogateescape(&output.stdout)).expect("constants");
        let module = member(&got, "functional_smoke");
        let names: Vec<String> = match module {
            PyValue::Dict(map) => map
                .iter()
                .map(|(key, _)| key.to_utf8().expect("text"))
                .collect(),
            other => panic!("{}", other.repr()),
        };
        assert_eq!(
            names,
            [
                "_COMPLETION_PROMPT",
                "_REASONING_PROMPT",
                "_TOOL_PROMPT",
                "_WEATHER_TOOL"
            ]
        );
        let bundle = load("bundle.json");
        let first = member(
            member(&items(member(&bundle, "api"))[0], "original"),
            "globals_before",
        );
        assert!(same_value(first, module));
        let area = PrivateDirectory::create("functional-contract-").expect("a private directory");
        let requests = [
            r#"{"id": 1, "operation": "functional_smoke._check_completion", "args": [{}], "context": {}}"#,
            r#"{"id": 2, "operation": "functional_smoke._check_completion", "args": [{}], "globals": {"other": 1}}"#,
            r#"{"id": 3, "operation": "functional_smoke.other", "args": []}"#,
            r#"{"id": 4, "operation": "other.smoke", "args": []}"#,
            r#"{"id": 5, "operation": "functional_smoke._check_completion", "args": []}"#,
            r#"{"id": 6, "operation": "functional_smoke._first_choice", "args": [{"choices": [{"a": 1}]}], "globals": {"_TOOL_PROMPT": "x"}}"#,
            r#"{"id": 7, "operation": "functional_smoke.smoke", "args": ["u", "m", {"model_capabilities": {"tool_call": true}}], "kwargs": {"mock_responses": {"completion": {"choices": [{"message": {"content": "hi"}}]}}}, "globals": {"_TOOL_PROMPT": "other"}}"#,
            r#"{"id": 8, "operation": "functional_smoke.smoke", "args": ["u", "m", {}, 60.0, 0]}"#,
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
        assert_eq!(kind(&got[1]).0, "RequestError");
        assert_eq!(
            kind(&got[2]),
            (
                "AttributeError".to_string(),
                "module 'functional_smoke' has no attribute 'other'".to_string()
            )
        );
        assert_eq!(
            kind(&got[3]),
            (
                "ModuleNotFoundError".to_string(),
                "No module named 'other'".to_string()
            )
        );
        assert_eq!(
            kind(&got[4]),
            (
                "TypeError".to_string(),
                "_check_completion() missing 1 required positional argument: 'choice'".to_string()
            )
        );
        assert_eq!(text_of(member(&got[5], "kind")), "value");
        // A falsy non-None mock is the mock path (`0.get` is the AttributeError), never live HTTP.
        assert_eq!(
            kind(&got[7]),
            (
                "AttributeError".to_string(),
                "'int' object has no attribute 'get'".to_string()
            )
        );
        // A mock pass is a value: completion true, tool call false, so not passed.
        let value = member(&got[6], "value");
        assert!(
            matches!(member(value, "tool_call_ok"), PyValue::Bool(false))
                && matches!(member(value, "passed"), PyValue::Bool(false))
        );
        for arguments in [
            vec!["evaluate-jsonl", "--input-fd", "1"],
            vec!["constants", "x"],
            vec![],
        ] {
            let output = Command::new(env!("CARGO_BIN_EXE_recipe_functional_api"))
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
            (
                "functional_http.rs",
                include_str!("../src/functional_http.rs"),
            ),
            (
                "functional_smoke.rs",
                include_str!("../src/functional_smoke.rs"),
            ),
            (
                "functional_cli.rs",
                include_str!("../src/functional_cli.rs"),
            ),
            (
                "functional_api.rs",
                include_str!("../src/functional_api.rs"),
            ),
            (
                "functional_bench.rs",
                include_str!("../src/functional_bench.rs"),
            ),
            (
                "bin/functional_smoke.rs",
                include_str!("../src/bin/functional_smoke.rs"),
            ),
            (
                "bin/recipe_functional_api.rs",
                include_str!("../src/bin/recipe_functional_api.rs"),
            ),
            (
                "bin/recipe_functional_bench.rs",
                include_str!("../src/bin/recipe_functional_bench.rs"),
            ),
        ];
        let mut found = Vec::new();
        for (name, text) in sources {
            for forbidden in [
                "env::var",
                "getenv",
                "functional_fixture",
                "real-worker",
                "val_agent",
                "CARGO_MANIFEST_DIR",
                "BoundaryViolation",
                "Command::new",
                "/usr/bin/python",
                "python3",
                "hf_http",
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

// ── new checks: HTTP and URL behaviour beyond the saved calls (source-derived expectations) ──────

mod new_cases {
    use super::*;

    fn post(base: &str, timeout: &str) -> PyValue {
        let area = PrivateDirectory::create("functional-new-").expect("a private directory");
        let line = format!(
            r#"{{"id": "n", "operation": "functional_smoke._post_chat", "args": [{}, {{"model": "m"}}, {timeout}]}}"#,
            PyValue::str(base).dumps_ascii().expect("json")
        );
        let (output, mut records) = run_service(area.path(), &[line]);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        records.remove(0)
    }

    fn error_text(record: &PyValue) -> String {
        assert_eq!(
            text_of(member(record, "kind")),
            "value",
            "{}",
            record.repr()
        );
        text_of(member(member(record, "value"), "_error"))
    }

    fn raw_peer(bytes: &[u8]) -> Peer {
        Peer::start(
            vec![Answer {
                status: 200,
                body: Vec::new(),
                behavior: None,
                raw: Some(bytes.to_vec()),
            }],
            true,
        )
    }

    #[test]
    fn answers_by_chunks_to_the_end_of_the_stream_and_with_a_one_hundred_are_read_as_http_client_reads_them(
    ) {
        let chunked = raw_peer(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\n{\"a\"\r\n3;ext=1\r\n: 1\r\n1\r\n}\r\n0\r\nTrailer: x\r\n\r\n");
        let record = post(&chunked.base, "2.0");
        assert_eq!(text_of(member(&record, "kind")), "value");
        assert!(
            same_value(
                member(&record, "value"),
                &loads_text(&PyStr::new(r#"{"a": 1}"#)).expect("json")
            ),
            "{}",
            record.repr()
        );
        let until_end = raw_peer(b"HTTP/1.0 200 OK\r\n\r\n[1, 2]");
        assert!(same_value(
            member(&post(&until_end.base, "2.0"), "value"),
            &loads_text(&PyStr::new("[1, 2]")).expect("json")
        ));
        let hundred = raw_peer(
            b"HTTP/1.1 100 Continue\r\nX: y\r\n\r\nHTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}",
        );
        assert!(same_value(
            member(&post(&hundred.base, "2.0"), "value"),
            &loads_text(&PyStr::new("{}")).expect("json")
        ));
        // Not an object: any JSON value is the answer.
        let scalar = raw_peer(b"HTTP/1.0 200 OK\r\nContent-Length: 4\r\n\r\nnull");
        assert!(matches!(
            member(&post(&scalar.base, "2.0"), "value"),
            PyValue::None
        ));
    }

    #[test]
    fn a_malformed_or_shortened_answer_is_the_exception_http_client_raises() {
        let cases: [(&[u8], &str); 7] = [
            (b"garbage\r\n\r\n", "exception: BadStatusLine: garbage\r\n"),
            (b"HTTP/1.1 abc OK\r\n\r\n", "exception: BadStatusLine: HTTP/1.1 abc OK\r\n"),
            (b"HTTP/2.0 200 OK\r\n\r\n", "exception: UnknownProtocol: HTTP/2.0"),
            (b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\n12345", "exception: IncompleteRead: IncompleteRead(5 bytes read, 5 more expected)"),
            // The first version of this expectation said 2 bytes: wrong. `_read_chunked` discards the partial bytes of the current short chunk and
            // re-raises with the completed chunks only (saved repair-002 and repair-003).
            (b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nab", "exception: IncompleteRead: IncompleteRead(0 bytes read)"),
            (b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n\xff\xfe", "exception: UnicodeDecodeError: 'utf-8' codec can't decode byte 0xff in position 0: invalid start byte"),
            (b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/\r\nContent-Length: 2\r\n\r\nhi", "http_302: hi"),
        ];
        for (bytes, want) in cases {
            let peer = raw_peer(bytes);
            assert_eq!(
                error_text(&post(&peer.base, "2.0")),
                want,
                "{:?}",
                String::from_utf8_lossy(bytes)
            );
        }
    }

    /// Source-derived (`int(line, 16)` and `_read_chunked`): the spellings of a chunk size Python accepts, the ones it refuses, and a negative size.
    #[test]
    fn chunk_sizes_are_read_as_int_with_base_16_reads_them_and_a_refused_one_is_an_incomplete_read_of_the_completed_chunks(
    ) {
        let answer = |size: &str, rest: &str| {
            format!("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n{size}\r\n{rest}")
                .into_bytes()
        };
        for size in [
            "2", "02", "0x2", "0X2", "+2", " 2 ", "2;ext=1", "0_2", "0x_2",
        ] {
            let peer = raw_peer(&answer(size, "{}\r\n0\r\n\r\n"));
            let record = post(&peer.base, "2.0");
            assert!(
                same_value(
                    member(&record, "value"),
                    &loads_text(&PyStr::new("{}")).expect("json")
                ),
                "{size:?}: {}",
                record.repr()
            );
        }
        for size in ["", "g", "_2", "2_", "2__0", "0x", "1 2", "--2"] {
            let peer = raw_peer(&answer(size, "{}\r\n0\r\n\r\n"));
            assert_eq!(
                error_text(&post(&peer.base, "2.0")),
                "exception: IncompleteRead: IncompleteRead(0 bytes read)",
                "{size:?}"
            );
        }
        // Completed chunks count; a refused size or a short chunk after them keeps their bytes only.
        let peer = raw_peer(&answer("3", "abc\r\n5\r\nxy"));
        assert_eq!(
            error_text(&post(&peer.base, "2.0")),
            "exception: IncompleteRead: IncompleteRead(3 bytes read)"
        );
        let peer = raw_peer(&answer("3", "abc\r\n"));
        assert_eq!(
            error_text(&post(&peer.base, "2.0")),
            "exception: IncompleteRead: IncompleteRead(3 bytes read)"
        );
        let peer = raw_peer(&answer("3", "abc"));
        assert_eq!(
            error_text(&post(&peer.base, "2.0")),
            "exception: IncompleteRead: IncompleteRead(3 bytes read)"
        );
        // A negative size reads to the end of the stream; the missing terminator then ends it with those bytes counted.
        let peer = raw_peer(&answer("-2", "abc"));
        assert_eq!(
            error_text(&post(&peer.base, "2.0")),
            "exception: IncompleteRead: IncompleteRead(3 bytes read)"
        );
    }

    /// Source-derived (`UnicodeEncodeError.__str__`): a single character is shown as `\xNN`, `\uNNNN` or `\UNNNNNNNN`, a run by its range; the
    /// positions are code points of the whole request line `POST <selector> HTTP/1.1`.
    #[test]
    fn an_unencodable_request_line_names_its_characters_by_their_width_and_positions_by_code_point()
    {
        let base = "http://127.0.0.1:9";
        let one = |suffix: &str| error_text(&post(&format!("{base}/{suffix}"), "2.0"));
        assert_eq!(one("\u{e9}"), "exception: UnicodeEncodeError: 'ascii' codec can't encode character '\\xe9' in position 6: ordinal not in range(128)");
        assert_eq!(one("\u{80}"), "exception: UnicodeEncodeError: 'ascii' codec can't encode character '\\x80' in position 6: ordinal not in range(128)");
        assert_eq!(one("\u{ff}"), "exception: UnicodeEncodeError: 'ascii' codec can't encode character '\\xff' in position 6: ordinal not in range(128)");
        assert_eq!(one("\u{100}"), "exception: UnicodeEncodeError: 'ascii' codec can't encode character '\\u0100' in position 6: ordinal not in range(128)");
        assert_eq!(one("a\u{d55c}"), "exception: UnicodeEncodeError: 'ascii' codec can't encode character '\\ud55c' in position 7: ordinal not in range(128)");
        assert_eq!(one("\u{ffff}"), "exception: UnicodeEncodeError: 'ascii' codec can't encode character '\\uffff' in position 6: ordinal not in range(128)");
        assert_eq!(one("\u{10000}"), "exception: UnicodeEncodeError: 'ascii' codec can't encode character '\\U00010000' in position 6: ordinal not in range(128)");
        assert_eq!(one("x\u{1f600}"), "exception: UnicodeEncodeError: 'ascii' codec can't encode character '\\U0001f600' in position 7: ordinal not in range(128)");
        assert_eq!(one("\u{e9}\u{1f600}z"), "exception: UnicodeEncodeError: 'ascii' codec can't encode characters in position 6-7: ordinal not in range(128)");
    }

    #[test]
    fn urls_that_urllib_refuses_or_cannot_reach_are_values_or_the_exceptions_it_raises() {
        let closed = {
            let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
            format!(
                "http://127.0.0.1:{}",
                listener.local_addr().expect("an address").port()
            )
        };
        assert_eq!(
            error_text(&post(&closed, "2.0")),
            "urlerror: [Errno 111] Connection refused"
        );
        assert_eq!(
            error_text(&post("bad://host", "2.0")),
            "urlerror: unknown url type: bad"
        );
        assert_eq!(error_text(&post("http:", "2.0")), "urlerror: no host given");
        assert_eq!(
            error_text(&post("http://127.0.0.1:abc", "2.0")),
            "exception: InvalidURL: nonnumeric port: 'abc'"
        );
        assert_eq!(
            error_text(&post("http://127.0.0.1:99999", "2.0")),
            "exception: OverflowError: getsockaddrarg: port must be 0-65535."
        );
        assert_eq!(error_text(&post("http://127.0.0.1 x", "2.0")), "exception: InvalidURL: URL can't contain control characters. '127.0.0.1 x' (found at least ' ')");
        assert!(error_text(&post("http://127.0.0.1/a b", "2.0")).starts_with("exception: InvalidURL: URL can't contain control characters. '/a b/v1/chat/completions' (found at least ' ')"));
        assert!(error_text(&post("https://127.0.0.1:1", "2.0"))
            .starts_with("exception: NotImplementedError: the https: scheme is outside"));
        assert_eq!(
            error_text(&post("http://127.0.0.1:1", "-1")),
            "exception: ValueError: Timeout value out of range"
        );
        // A request path, a fragment (never sent) and the selector with its query reach the peer as urllib builds them.
        let peer = Peer::start(
            vec![Answer {
                status: 200,
                body: b"{}".to_vec(),
                behavior: None,
                raw: None,
            }],
            true,
        );
        let base = format!("{}/api?x=1#frag", peer.base);
        assert_eq!(text_of(member(&post(&base, "2.0"), "kind")), "value");
        // The suffix is appended after the fragment, so it is part of the fragment: the selector is the path and query alone.
        assert_eq!(peer.requests()[0].path, "/api?x=1");
        let nothing = format!("{}?q", peer.base);
        let _ = post(&nothing, "2.0");
        assert_eq!(peer.requests()[1].path, "/?q/v1/chat/completions");
    }
}

// ── the benchmark program ────────────────────────────────────────────────────────────────────────

mod bench_cases {
    use super::*;

    fn rows(bundle: &PyValue, picks: &[usize], peer: &Peer, mutate: bool) -> String {
        let mut rows = Vec::new();
        for pick in picks {
            let case = &items(member(bundle, "api"))[*pick];
            let request = member(case, "request");
            let mut expected = member(member(case, "original"), "value").clone();
            if mutate {
                if let PyValue::Dict(map) = &mut expected {
                    map.set("passed", PyValue::str("changed"));
                }
            }
            let marks = vec![
                ("@CASE@", "/nonexistent".to_string()),
                ("@BASE@", peer.base.clone()),
                ("@HOST@", peer.host.clone()),
            ];
            let mut row = PyDict::new();
            row.set("metric", PyValue::str(&format!("case_{pick}")));
            row.set(
                "operation",
                PyValue::str(&format!(
                    "functional_smoke.{}",
                    text_of(member(request, "function"))
                )),
            );
            row.set("args", rebase_all(member(request, "args"), &marks));
            row.set("kwargs", rebase_all(member(request, "kwargs"), &marks));
            row.set("expected", rebase_all(&expected, &marks));
            rows.push(PyValue::Dict(row));
        }
        PyValue::List(rows).dumps_ascii().expect("json")
    }

    fn run_bench(fixture: &Path, extra: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_recipe_functional_bench"))
            .arg("--fixture")
            .arg(fixture)
            .args(extra)
            .env_clear()
            .stdin(Stdio::null())
            .output()
            .expect("run the benchmark")
    }

    #[test]
    fn the_benchmark_calls_the_real_smoke_once_to_check_the_value_then_measures_whole_calls_with_real_exchanges(
    ) {
        let bundle = load("bundle.json");
        let cases = items(member(&bundle, "api"));
        assert_eq!(
            text_of(member(member(&cases[105], "request"), "id")),
            "api-105"
        );
        assert_eq!(
            text_of(member(member(&cases[162], "request"), "id")),
            "api-162"
        );
        let area = PrivateDirectory::create("functional-bench-").expect("a private directory");
        let peer = peer_for(member(&cases[162], "request")).expect("a peer");
        let good = area.path().join("good.json");
        std::fs::write(&good, rows(&bundle, &[105, 162], &peer, false)).expect("a fixture");
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
        for (line, metric) in lines[1..].iter().zip(["case_105", "case_162"]) {
            let report = loads_text(&PyStr::new(line)).expect("a row");
            assert_eq!(text_of(member(&report, "metric")), metric);
            assert_eq!(items(member(&report, "rounds")).len(), 2);
        }
        // One preflight call and 2 x 3 measured calls of the HTTP row, three real requests each: 21; the mock row sent none.
        assert_eq!(peer.requests().len(), 21);
        // A wrong value ends the run before any timer: no report, and the HTTP row made no request at all.
        let second = peer_for(member(&cases[162], "request")).expect("a peer");
        let wrong = area.path().join("wrong.json");
        std::fs::write(&wrong, rows(&bundle, &[162], &second, true)).expect("a fixture");
        let output = run_bench(&wrong, &["--rounds", "2", "--iterations", "3"]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr)
            .contains("differs from the saved original result"));
        assert_eq!(
            second.requests().len(),
            3,
            "only the preflight call's three requests"
        );
        // A failing call is an error, not a sample; an unknown operation or a bad option stops the run.
        let failing = area.path().join("failing.json");
        std::fs::write(&failing, r#"[{"metric": "bad", "operation": "functional_smoke._check_completion", "args": [], "kwargs": {}, "expected": false}]"#).expect("a fixture");
        let output = run_bench(&failing, &[]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let unknown = area.path().join("unknown.json");
        std::fs::write(&unknown, r#"[{"metric": "bad", "operation": "functional_smoke.nothing", "args": [], "kwargs": {}, "expected": null}]"#).expect("a fixture");
        assert_eq!(run_bench(&unknown, &[]).status.code(), Some(1));
        assert_eq!(run_bench(&good, &["--rounds", "0"]).status.code(), Some(2));
    }
}
