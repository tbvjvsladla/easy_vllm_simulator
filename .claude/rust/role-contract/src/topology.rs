//! The four-way topology parity predicate (`topology_parity.py`): branch, specialised-constitution header, manifest and
//! active campaign. A port of its functions `read_layer_header`, `layer_files`, `_git`, `_load_manifest`, the campaign leg
//! (over [`crate::campaign`]) and `evaluate`, plus the typed exceptions the original lets escape ([`Crash`]).
//!
//! Git stays a real process (`git -C <repo> ...`, 30 s limit, strict UTF-8 and universal newlines for its output): nothing is
//! read from `.git` and no answer is computed from a fixture. The manifest is YAML (the common `safe_load` port in `Q`), the
//! campaign declaration is JSON (`campaign.yaml` is deliberately not YAML).
//!
//! A [`Crash`] is the typed exception of the original with the *logical* traceback context of its source (file, line,
//! function, source line and caret range of each frame). Native code runs no Python and has no Python stack: the frames
//! are source metadata, and a site whose frames were not observed says so ([`Where::Opaque`]).

use crate::campaign::{self, is_file, os_class, os_error_text, ReadFailure};
use agent_runtime_support::pyjson::utf8_error_text;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use agent_runtime_support::pyyaml::safe_load;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const LAYER_PATHSPEC: &str = "*.topology.md";
pub const HEADER_WINDOW: usize = 10;
pub const TOPOLOGIES: [&str; 2] = ["single", "multi"];
/// `_git`'s `timeout=30`.
pub const GIT_TIMEOUT: Duration = Duration::from_secs(30);

/// One logical traceback frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub file: Vec<u8>,
    pub line: u32,
    pub function: &'static str,
    /// The source line as Python shows it (stripped); empty when no source is shown (frozen modules).
    pub code: &'static str,
    /// The sub-expression the carets mark (`""`: none; the whole line: Python omits the carets).
    pub expr: &'static str,
}

fn frame(
    file: &[u8],
    line: u32,
    function: &'static str,
    code: &'static str,
    expr: &'static str,
) -> Frame {
    Frame {
        file: file.to_vec(),
        line,
        function,
        code,
        expr,
    }
}

impl Frame {
    pub fn pathlib_open() -> Vec<Frame> {
        vec![
            frame(
                b"/usr/lib/python3.12/pathlib.py",
                1029,
                "read_text",
                "with self.open(mode='r', encoding=encoding, errors=errors) as f:",
                "self.open(mode='r', encoding=encoding, errors=errors)",
            ),
            frame(
                b"/usr/lib/python3.12/pathlib.py",
                1015,
                "open",
                "return io.open(self, mode, buffering, encoding, errors, newline)",
                "io.open(self, mode, buffering, encoding, errors, newline)",
            ),
        ]
    }
    pub fn pathlib_read() -> Vec<Frame> {
        vec![
            frame(
                b"/usr/lib/python3.12/pathlib.py",
                1030,
                "read_text",
                "return f.read()",
                "f.read()",
            ),
            frame(b"<frozen codecs>", 322, "decode", "", ""),
        ]
    }
    /// `Path.is_file()` whose `stat` fails with an error `pathlib` does not swallow (the 3.12 frames of `is_file` and `stat`).
    pub fn pathlib_is_file() -> Vec<Frame> {
        Self::pathlib_stat_failure(
            894,
            "is_file",
            "return S_ISREG(self.stat().st_mode)",
            "self.stat()",
        )
    }
    /// `Path.is_dir()` whose `stat` fails the same way.
    pub fn pathlib_is_dir() -> Vec<Frame> {
        Self::pathlib_stat_failure(
            877,
            "is_dir",
            "return S_ISDIR(self.stat().st_mode)",
            "self.stat()",
        )
    }
    fn pathlib_stat_failure(
        line: u32,
        function: &'static str,
        code: &'static str,
        expr: &'static str,
    ) -> Vec<Frame> {
        vec![
            frame(
                b"/usr/lib/python3.12/pathlib.py",
                line,
                function,
                code,
                expr,
            ),
            frame(
                b"/usr/lib/python3.12/pathlib.py",
                842,
                "stat",
                "return os.stat(self, follow_symlinks=follow_symlinks)",
                "os.stat(self, follow_symlinks=follow_symlinks)",
            ),
        ]
    }
    /// `Path(...).parents[index]` past the last parent: the `__getitem__` frame that raises `IndexError(idx)`.
    pub fn pathlib_parents_index() -> Vec<Frame> {
        vec![frame(
            b"/usr/lib/python3.12/pathlib.py",
            281,
            "__getitem__",
            "raise IndexError(idx)",
            "",
        )]
    }
    fn subprocess_stderr() -> Vec<Frame> {
        vec![
            frame(
                b"/usr/lib/python3.12/subprocess.py",
                550,
                "run",
                "stdout, stderr = process.communicate(input, timeout=timeout)",
                "process.communicate(input, timeout=timeout)",
            ),
            frame(
                b"/usr/lib/python3.12/subprocess.py",
                1209,
                "communicate",
                "stdout, stderr = self._communicate(input, endtime, timeout)",
                "self._communicate(input, endtime, timeout)",
            ),
            frame(
                b"/usr/lib/python3.12/subprocess.py",
                2157,
                "_communicate",
                "stderr = self._translate_newlines(stderr,",
                "self._translate_newlines(stderr,",
            ),
            frame(
                b"/usr/lib/python3.12/subprocess.py",
                1086,
                "_translate_newlines",
                "data = data.decode(encoding, errors)",
                "data.decode(encoding, errors)",
            ),
        ]
    }
}

/// Where in the original a crash arose (for the logical traceback).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Where {
    /// `evaluate` line `line` (+ expression), reached through `main`.
    Evaluate {
        line: u32,
        code: &'static str,
        expr: &'static str,
    },
    /// `_git` (called from `evaluate` line 178 or 179) in `subprocess.run`, with the decode failure of stderr.
    GitStderr {
        call_line: u32,
        call_code: &'static str,
        call_expr: &'static str,
    },
    /// A `campaign_init.py` frame (the module path and the frames below it).
    Campaign { module: Vec<u8>, frames: Vec<Frame> },
    /// The report's `print` raised while encoding.
    Print,
    /// A site whose frames were not observed: the traceback says so instead of inventing them.
    Opaque,
}

/// An exception of the original that escapes the predicate.
#[derive(Clone, Debug, PartialEq)]
pub struct Crash {
    pub class: &'static str,
    pub message: PyStr,
    pub site: Where,
    /// The constructor arguments and public attributes of the exception object beyond its message.
    pub detail: Box<Detail>,
}

/// What a typed API observation needs of an exception object: its constructor arguments and public attributes.
#[derive(Clone, Debug, PartialEq)]
pub enum Detail {
    /// `args == (message,)` and no public attributes.
    Plain,
    /// An `OSError` family member raised by a system call: `args == (errno, strerror)`, attributes `errno`, `strerror`,
    /// `filename`, `filename2`.
    Os {
        errno: i32,
        strerror: String,
        filename: Vec<u8>,
    },
    /// `UnicodeDecodeError(encoding, object, start, end, reason)`.
    Decode {
        encoding: &'static str,
        object: Vec<u8>,
        start: usize,
        end: usize,
        reason: String,
    },
    /// `AttributeError` with its `name` and `obj` attributes.
    Attribute { name: String, obj: PyValue },
    /// `UnicodeEncodeError(encoding, object, start, end, reason)`.
    Encode {
        encoding: &'static str,
        object: PyStr,
        start: usize,
        end: usize,
        reason: String,
    },
    /// `RuntimeError("Symlink loop from ...")` that `Path.resolve` raises inside the `except OSError` handler: its implicit
    /// context is the `ELOOP` OSError of `stat(filename)`.
    SymlinkLoop { filename: Vec<u8> },
    /// Not a Python exception: the campaign module of the checkout is not the reviewed source, so no native answer exists.
    /// Callers turn it into their own unmet condition; it never reaches a business record.
    SourceUnmatched,
}

/// The `UnicodeEncodeError` crash of encoding `object` for the file system (typed constructor parts attached).
pub fn encode_crash(object: &PyStr, failure: &crate::pyprofile::EncodeFailure) -> Crash {
    Crash::new(
        "UnicodeEncodeError",
        &PyStr::new(&failure.message),
        Where::Opaque,
    )
    .with_detail(Detail::Encode {
        encoding: failure.encoding,
        object: object.clone(),
        start: failure.start,
        end: failure.end,
        reason: failure.reason.clone(),
    })
}

/// `os.path.abspath(path)`: relative to the current directory, `..` collapsed lexically.
pub fn abspath(path: &[u8]) -> Vec<u8> {
    if path.first() == Some(&b'/') {
        return posix_normpath(path);
    }
    let cwd = std::env::current_dir()
        .map(|p| p.as_os_str().as_bytes().to_vec())
        .unwrap_or_default();
    posix_normpath(&posix_join(&cwd, path))
}

impl Crash {
    pub fn new(class: &'static str, message: &PyStr, site: Where) -> Crash {
        Crash {
            class,
            message: message.clone(),
            site,
            detail: Box::new(Detail::Plain),
        }
    }

    pub fn with_detail(mut self, detail: Detail) -> Crash {
        self.detail = Box::new(detail);
        self
    }
}

// ── helpers over bytes / values ─────────────────────────────────────────────────

fn s(text: &str) -> PyValue {
    PyValue::Str(PyStr::new(text))
}

fn as_surrogate_str(bytes: &[u8]) -> PyStr {
    crate::pyprofile::fs_decode(bytes)
}

fn put(dict: &mut PyDict, key: &str, value: PyValue) {
    dict.set(key, value);
}

/// `posixpath.join(a, b)` for a relative or absolute `b`.
fn posix_join(a: &[u8], b: &[u8]) -> Vec<u8> {
    if b.first() == Some(&b'/') {
        return b.to_vec();
    }
    let mut out = a.to_vec();
    if !out.is_empty() && out.last() != Some(&b'/') {
        out.push(b'/');
    }
    out.extend_from_slice(b);
    out
}

/// `posixpath.split(path)`: `(head, tail)`.
fn posix_split(path: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let at = path.iter().rposition(|b| *b == b'/').map_or(0, |i| i + 1);
    let (mut head, tail) = (path[..at].to_vec(), path[at..].to_vec());
    if !head.is_empty() && head.iter().any(|b| *b != b'/') {
        while head.last() == Some(&b'/') {
            head.pop();
        }
    }
    (head, tail)
}

/// `posixpath.normpath(path)` (lexical: `..` pops the previous component).
fn posix_normpath(path: &[u8]) -> Vec<u8> {
    if path.is_empty() {
        return b".".to_vec();
    }
    let leading = path.iter().take_while(|b| **b == b'/').count();
    let initial = if leading == 2 {
        2
    } else if leading > 0 {
        1
    } else {
        0
    };
    let mut comps: Vec<&[u8]> = Vec::new();
    for comp in path.split(|b| *b == b'/') {
        if comp.is_empty() || comp == b"." {
            continue;
        }
        if comp != b".." || (initial == 0 && comps.is_empty()) || comps.last() == Some(&&b".."[..])
        {
            comps.push(comp);
        } else if !comps.is_empty() {
            comps.pop();
        }
    }
    let mut out = vec![b'/'; initial];
    for (index, comp) in comps.iter().enumerate() {
        if index > 0 {
            out.push(b'/');
        }
        out.extend_from_slice(comp);
    }
    if out.is_empty() {
        out.push(b'.');
    }
    out
}

/// `posixpath._joinrealpath(path, rest, strict=False, seen)`: `(path, ok)`.
fn join_realpath(
    mut path: Vec<u8>,
    rest: &[u8],
    seen: &mut std::collections::HashMap<Vec<u8>, Option<Vec<u8>>>,
) -> Result<(Vec<u8>, bool), std::io::Error> {
    let mut rest: &[u8] = rest;
    if rest.first() == Some(&b'/') {
        rest = &rest[1..];
        path = b"/".to_vec();
    }
    while !rest.is_empty() {
        let (name, tail) = match rest.iter().position(|b| *b == b'/') {
            Some(i) => (&rest[..i], &rest[i + 1..]),
            None => (rest, &rest[rest.len()..]),
        };
        rest = tail;
        if name.is_empty() || name == b"." {
            continue;
        }
        if name == b".." {
            if !path.is_empty() {
                let (head, last) = posix_split(&path);
                path = head;
                if last == b".." {
                    path = posix_join(&posix_join(&path, b".."), b"..");
                }
            } else {
                path = b"..".to_vec();
            }
            continue;
        }
        let newpath = posix_join(&path, name);
        let is_link = std::fs::symlink_metadata(Path::new(std::ffi::OsStr::from_bytes(&newpath)))
            .map(|meta| meta.file_type().is_symlink())
            .unwrap_or(false);
        if !is_link {
            path = newpath;
            continue;
        }
        if let Some(cached) = seen.get(&newpath) {
            match cached {
                Some(resolved) => {
                    path = resolved.clone();
                    continue;
                }
                None => return Ok((posix_join(&newpath, rest), false)),
            }
        }
        seen.insert(newpath.clone(), None);
        let target = std::fs::read_link(Path::new(std::ffi::OsStr::from_bytes(&newpath)))?;
        let (resolved, ok) = join_realpath(path.clone(), target.as_os_str().as_bytes(), seen)?;
        if !ok {
            return Ok((posix_join(&resolved, rest), false));
        }
        seen.insert(newpath, Some(resolved.clone()));
        path = resolved;
    }
    Ok((path, true))
}

/// `Path(repo).resolve()` (non-strict): `os.path.realpath` then `abspath`, and the `stat` that turns a symlink loop into the
/// `RuntimeError("Symlink loop from ...")` of Python 3.12. An `OSError` of `readlink` is raised as such.
pub fn resolve_checked(repo: &[u8]) -> Result<Vec<u8>, Crash> {
    let given = campaign::normalize(repo);
    let (joined, _) = join_realpath(Vec::new(), &given, &mut std::collections::HashMap::new())
        .map_err(|error| {
            Crash::new(
                os_class(&error),
                &os_error_text(&error, &given),
                Where::Opaque,
            )
            .with_detail(campaign::os_detail(&error, &given))
        })?;
    let absolute = if joined.first() == Some(&b'/') {
        joined
    } else {
        let cwd = std::env::current_dir()
            .map(|p| p.as_os_str().as_bytes().to_vec())
            .unwrap_or_default();
        posix_join(&cwd, &joined)
    };
    let resolved = posix_normpath(&absolute);
    let normalized = campaign::normalize(&resolved);
    if let Err(error) = std::fs::metadata(Path::new(std::ffi::OsStr::from_bytes(&normalized))) {
        if error.raw_os_error() == Some(40) {
            let mut message = PyStr::new("Symlink loop from ");
            message.push_str(&as_surrogate_str(&normalized).repr());
            return Err(
                Crash::new("RuntimeError", &message, Where::Opaque).with_detail(
                    Detail::SymlinkLoop {
                        filename: normalized.clone(),
                    },
                ),
            );
        }
    }
    Ok(normalized)
}

/// [`resolve_checked`] for callers that cannot report a crash (a symlink loop or an unreadable link is then the path as is).
pub fn resolve(repo: &[u8]) -> Vec<u8> {
    resolve_checked(repo).unwrap_or_else(|_| campaign::normalize(repo))
}

// ── git ─────────────────────────────────────────────────────────────────────────

enum GitRun {
    /// stdout (stripped text for `_git`) of a process that exited 0.
    Ok(Vec<u8>),
    /// A failure `_git` turns into `None` (cannot start, non-zero exit, timeout).
    None,
    /// Output that is not valid UTF-8 (the `UnicodeDecodeError` of `subprocess.run(text=True)`): stdout then stderr.
    Undecodable {
        failure: crate::pyprofile::DecodeFailure,
        object: Vec<u8>,
        in_stderr: bool,
    },
}

fn git_run(repo: &[u8], args: &[&str]) -> GitRun {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(std::ffi::OsStr::from_bytes(repo))
        .args(args);
    command
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let Ok(mut child) = command.spawn() else {
        return GitRun::None;
    };
    let drain = |mut pipe: Box<dyn std::io::Read + Send>| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = pipe.read_to_end(&mut bytes);
            bytes
        })
    };
    let out = drain(Box::new(child.stdout.take().expect("piped")));
    let err = drain(Box::new(child.stderr.take().expect("piped")));
    let deadline = Instant::now() + GIT_TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() >= deadline => break None,
            Ok(None) => std::thread::sleep(Duration::from_millis(2)),
            Err(_) => break None,
        }
    };
    let Some(status) = status else {
        let _ = child.kill();
        let _ = child.wait();
        return GitRun::None;
    };
    let stdout = out.join().unwrap_or_default();
    let stderr = err.join().unwrap_or_default();
    for (bytes, in_stderr) in [(&stdout, false), (&stderr, true)] {
        if let Err(failure) = crate::pyprofile::decode_preferred_detail(bytes) {
            return GitRun::Undecodable {
                failure,
                object: bytes.clone(),
                in_stderr,
            };
        }
    }
    if status.success() {
        GitRun::Ok(stdout)
    } else {
        GitRun::None
    }
}

fn translate_newlines(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .replace("\r\n", "\n")
        .replace('\r', "\n")
}

/// `_git(repo, *args)`: the stripped stdout of a git that succeeded, else `None`.
fn git(
    repo: &[u8],
    args: &[&str],
    call_line: u32,
    call_code: &'static str,
    call_expr: &'static str,
) -> Result<Option<PyStr>, Crash> {
    match git_run(repo, args) {
        GitRun::Ok(bytes) => Ok(Some(PyStr::new(&translate_newlines(&bytes)).strip())),
        GitRun::None => Ok(None),
        GitRun::Undecodable {
            failure,
            object,
            in_stderr,
        } => {
            let site = if in_stderr {
                Where::GitStderr {
                    call_line,
                    call_code,
                    call_expr,
                }
            } else {
                Where::Opaque
            };
            Err(
                Crash::new("UnicodeDecodeError", &PyStr::new(&failure.message), site).with_detail(
                    Detail::Decode {
                        encoding: failure.encoding,
                        object,
                        start: failure.start,
                        end: failure.end,
                        reason: failure.reason.clone(),
                    },
                ),
            )
        }
    }
}

// ── layer files and headers ─────────────────────────────────────────────────────

/// `layer_files(repo)`: the tracked `*.topology.md` files under `repo`, sorted as `pathlib` sorts (component by component).
pub fn layer_files(repo: &[u8]) -> Result<Vec<Vec<u8>>, Crash> {
    let output = match git_run(repo, &["ls-files", "-z", "--", LAYER_PATHSPEC]) {
        GitRun::Ok(bytes) => bytes,
        GitRun::None => return Ok(Vec::new()),
        GitRun::Undecodable {
            failure, object, ..
        } => {
            return Err(Crash::new(
                "UnicodeDecodeError",
                &PyStr::new(&failure.message),
                Where::Opaque,
            )
            .with_detail(Detail::Decode {
                encoding: failure.encoding,
                object,
                start: failure.start,
                end: failure.end,
                reason: failure.reason.clone(),
            }))
        }
    };
    let text = translate_newlines(&output);
    let mut paths: Vec<Vec<u8>> = text
        .split('\0')
        .filter(|rel| !rel.is_empty())
        .map(|rel| campaign::join(repo, rel.as_bytes()))
        .collect();
    paths.sort_by(|a, b| components(a).cmp(&components(b)));
    Ok(paths)
}

fn components(path: &[u8]) -> Vec<&[u8]> {
    path.split(|b| *b == b'/')
        .filter(|part| !part.is_empty())
        .collect()
}

/// A text file read line by line the way `open(path, encoding="utf-8")` does: 8192-byte chunks through an incremental
/// UTF-8 decoder, universal newlines.
struct LineReader {
    source: Box<dyn std::io::Read>,
    pending: Vec<u8>,
    text: Vec<char>,
    cursor: usize,
    carriage: bool,
    done: bool,
}

const CHUNK: usize = 8192;

impl LineReader {
    fn new(source: Box<dyn std::io::Read>) -> Self {
        LineReader {
            source,
            pending: Vec::new(),
            text: Vec::new(),
            cursor: 0,
            carriage: false,
            done: false,
        }
    }

    /// One `read1(8192)` of the text wrapper and its incremental decode: a read that returns nothing is the end of the file and
    /// decodes what is pending with `final=True` (an incomplete tail is then "unexpected end of data", positioned within the
    /// pending bytes only); every other chunk is decoded with `final=False`, keeping an incomplete tail for the next one.
    fn feed(&mut self) -> Result<(), String> {
        // one raw read of up to CHUNK bytes (a short read is a short chunk; zero bytes is the end of the file); an I/O error
        // of the read carries no file name in the original's message
        let mut chunk = vec![0u8; CHUNK];
        let read = loop {
            match self.source.read(&mut chunk) {
                Ok(n) => break n,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => {
                    let number = error.raw_os_error().unwrap_or(0);
                    let full = error.to_string();
                    let text = full
                        .strip_suffix(&format!(" (os error {number})"))
                        .unwrap_or(&full)
                        .to_string();
                    return Err(format!("[Errno {number}] {text}"));
                }
            }
        };
        let eof = read == 0;
        let mut input = std::mem::take(&mut self.pending);
        input.extend_from_slice(&chunk[..read]);
        let valid = match std::str::from_utf8(&input) {
            Ok(text) => text.len(),
            Err(error) if error.error_len().is_none() && !eof => error.valid_up_to(),
            Err(error) => return Err(utf8_error_text(&input, &error)),
        };
        let decoded = std::str::from_utf8(&input[..valid]).expect("valid prefix");
        self.pending = input[valid..].to_vec();
        for c in decoded.chars() {
            if self.carriage {
                self.carriage = false;
                if c == '\n' {
                    continue;
                }
            }
            if c == '\r' {
                self.carriage = true;
                self.text.push('\n');
            } else {
                self.text.push(c);
            }
        }
        if eof {
            self.done = true;
        }
        Ok(())
    }

    /// The next line including its newline, `""` at the end.
    fn readline(&mut self) -> Result<String, String> {
        loop {
            if let Some(offset) = self.text[self.cursor..].iter().position(|c| *c == '\n') {
                let line: String = self.text[self.cursor..=self.cursor + offset]
                    .iter()
                    .collect();
                self.cursor += offset + 1;
                return Ok(line);
            }
            if self.done {
                let rest: String = self.text[self.cursor..].iter().collect();
                self.cursor = self.text.len();
                return Ok(rest);
            }
            self.feed()?;
        }
    }
}

/// `_HEADER_DECL.match(line)`: `^\*\*topology:[ \t]*(single|multi)\*\*`.
fn declared_topology(line: &str) -> Option<&'static str> {
    let rest = line.strip_prefix("**topology:")?;
    let rest = rest.trim_start_matches([' ', '\t']);
    for topology in TOPOLOGIES {
        if let Some(after) = rest.strip_prefix(topology) {
            if after.starts_with("**") {
                return Some(topology);
            }
        }
    }
    None
}

/// `read_layer_header(path)`: `(topology or None, reason)`.
pub fn read_layer_header(path: &[u8]) -> (Option<&'static str>, PyStr) {
    let failure = |text: PyStr| {
        let mut why = PyStr::new("읽기 실패: ");
        why.push_py(&text);
        (None, why)
    };
    let file = match std::fs::File::open(Path::new(std::ffi::OsStr::from_bytes(path))) {
        Ok(file) => {
            if file.metadata().map(|m| m.is_dir()).unwrap_or(false) {
                return failure(os_error_text(&std::io::Error::from_raw_os_error(21), path));
            }
            file
        }
        Err(error) => return failure(os_error_text(&error, path)),
    };
    // the window is read lazily: no more than the chunks the first ten lines need are read
    let mut reader = LineReader::new(Box::new(file));
    let mut hits: Vec<&'static str> = Vec::new();
    for _ in 0..HEADER_WINDOW {
        match reader.readline() {
            Err(message) => return failure(PyStr::new(&message)),
            Ok(line) if line.is_empty() => break,
            Ok(line) => {
                if let Some(topology) = declared_topology(&line) {
                    hits.push(topology);
                }
            }
        }
    }
    match hits.len() {
        0 => (None, PyStr::new(&format!("머리 {HEADER_WINDOW}줄 안에 자기선언 헤더가 없다 (`**topology: single|multi**` 로 시작하는 줄이 있어야 한다)"))),
        1 => (Some(hits[0]), PyStr::new("ok")),
        n => {
            let list: Vec<String> = hits.iter().map(|h| format!("'{h}'")).collect();
            (None, PyStr::new(&format!("자기선언 헤더가 {n}개다 -- 어느 것이 선언인지 모른다: [{}]", list.join(", "))))
        }
    }
}

// ── evaluate ────────────────────────────────────────────────────────────────────

fn crash_eval(
    class: &'static str,
    message: String,
    line: u32,
    code: &'static str,
    expr: &'static str,
) -> Crash {
    Crash::new(
        class,
        &PyStr::new(&message),
        Where::Evaluate { line, code, expr },
    )
}

/// `x.get(...)` on a value that is not a dict (the original's `AttributeError`), at `evaluate` line `line`.
fn attribute_error(value: &PyValue, line: u32, code: &'static str, expr: &'static str) -> Crash {
    let crash = {
        crash_eval(
            "AttributeError",
            format!("'{}' object has no attribute 'get'", value.type_name()),
            line,
            code,
            expr,
        )
    };
    crash.with_detail(Detail::Attribute {
        name: "get".to_string(),
        obj: value.clone(),
    })
}

fn truthy_or(value: Option<&PyValue>, default: PyValue) -> PyValue {
    match value {
        Some(v) if v.is_truthy() => v.clone(),
        _ => default,
    }
}

fn reason(reasons: &mut Vec<PyStr>, text: String) {
    reasons.push(PyStr::new(&text));
}

fn repr_of(value: &PyValue) -> String {
    value.repr()
}

fn repr_opt(text: &Option<PyStr>) -> String {
    text.as_ref().map_or("None".to_string(), |t| t.repr())
}

fn dict_get(value: &PyValue, key: &str) -> Option<PyValue> {
    match value.peel() {
        PyValue::Dict(dict) => dict.get(key).cloned(),
        _ => None,
    }
}

fn manifest_leg(
    root: &[u8],
    mtopo: Option<&PyStr>,
    leg3: &mut PyDict,
    reasons: &mut Vec<PyStr>,
) -> Result<(), Crash> {
    let Some(mtopo) = mtopo else { return Ok(()) };
    let topo_bytes =
        crate::pyprofile::fs_encode(mtopo).map_err(|failure| encode_crash(mtopo, &failure))?;
    let mpath = campaign::join(
        &campaign::join(&campaign::join(root, b"output"), &topo_bytes),
        b"manifest.yaml",
    );
    let present = is_file(&mpath).map_err(|error| {
        Crash::new(
            os_class_name(&error),
            &os_error_text(&error, &mpath),
            Where::Opaque,
        )
        .with_detail(campaign::os_detail(&error, &mpath))
    })?;
    if !present {
        put(leg3, "status", s("absent"));
        put(leg3, "detail", s("manifest 가 없다 -- 비추적 산출물이라 미테라포밍 클론·워크트리에는 없는 것이 정상이다. **존재**는 policy:TERRAFORM_FLAG_GATE 가 요구하고, 이 술어는 **일치**만 본다"));
        return Ok(());
    }
    let loaded: Result<PyValue, PyStr> =
        match std::fs::File::open(Path::new(std::ffi::OsStr::from_bytes(&mpath))) {
            Err(error) => Err(os_error_text(&error, &mpath)),
            Ok(file) => {
                if file.metadata().map(|m| m.is_dir()).unwrap_or(false) {
                    Err(os_error_text(
                        &std::io::Error::from_raw_os_error(21),
                        &mpath,
                    ))
                } else {
                    let mut bytes = Vec::new();
                    match std::io::Read::read_to_end(&mut &file, &mut bytes) {
                        Err(error) => Err(os_error_text(&error, &mpath)),
                        Ok(_) => {
                            safe_load(&bytes, &as_surrogate_str(&mpath)).map_err(|e| e.message)
                        }
                    }
                }
            }
        };
    let man = match loaded {
        Err(message) => {
            put(leg3, "status", s("red"));
            let mut text = PyStr::new(&format!(
                "MANIFEST_UNREADABLE: output/{}/manifest.yaml 파싱 실패: ",
                mtopo.to_utf8().unwrap_or_default()
            ));
            text.push_py(&message);
            reasons.push(text);
            None
        }
        // `yaml.safe_load(fh) or {}`
        Ok(value) => Some(if value.is_truthy() {
            value
        } else {
            PyValue::Dict(PyDict::new())
        }),
    };
    if let Some(man) = man {
        if let PyValue::Dict(dict) = man.peel() {
            let value = dict.get("topology").cloned().unwrap_or(PyValue::None);
            let terra = truthy_or(dict.get("terraforming"), PyValue::Dict(PyDict::new()));
            let PyValue::Dict(terra_dict) = terra.peel() else {
                return Err(attribute_error(
                    &terra,
                    245,
                    "leg3[\"terraforming_complete\"] = bool(terra.get(\"complete\"))",
                    "terra.get",
                ));
            };
            put(leg3, "value", value.clone());
            put(
                leg3,
                "terraforming_complete",
                PyValue::Bool(terra_dict.get("complete").is_some_and(PyValue::is_truthy)),
            );
            put(
                leg3,
                "branch_verified",
                PyValue::Bool(
                    terra_dict
                        .get("branch_verified")
                        .is_some_and(PyValue::is_truthy),
                ),
            );
            let same = matches!(value.peel(), PyValue::Str(text) if text == mtopo);
            if !same {
                put(leg3, "status", s("red"));
                reason(reasons, format!(
                    "MANIFEST_MISMATCH: output/{0}/manifest.yaml#topology={1} 인데 통로는 {2} 다 -- manifest 가 권위이므로 통로 선택이 틀렸거나 manifest 가 오염됐다",
                    mtopo.to_utf8().unwrap_or_default(), repr_of(&value), mtopo.repr()
                ));
            }
        }
    }
    Ok(())
}

fn os_class_name(error: &std::io::Error) -> &'static str {
    os_class(error)
}

/// Iteration over a Python value (`for n in value`): list items, dict keys, string characters; anything else is the
/// `TypeError` the original raises.
fn iterate(value: &PyValue) -> Result<Vec<PyValue>, PyValue> {
    match value.peel() {
        PyValue::List(items) | PyValue::Tuple(items) => Ok(items.clone()),
        PyValue::Dict(dict) => Ok(dict
            .iter()
            .map(|(key, _)| PyValue::Str(key.clone()))
            .collect()),
        PyValue::Str(text) => Ok(text
            .code_points()
            .iter()
            .map(|c| PyValue::Str(PyStr::from_code_points(vec![*c])))
            .collect()),
        other => Err(other.clone()),
    }
}

fn campaign_leg(
    root: &[u8],
    topology: Option<&str>,
    expect: Option<&str>,
    leg4: &mut PyDict,
    reasons: &mut Vec<PyStr>,
) -> Result<(), Crash> {
    let module = campaign::join(root, campaign::MODULE_REL.as_bytes());
    let present = is_file(&module).map_err(|error| {
        Crash::new(
            os_class_name(&error),
            &os_error_text(&error, &module),
            Where::Opaque,
        )
        .with_detail(campaign::os_detail(&error, &module))
    })?;
    if present {
        // the native campaign read side is the REVIEWED source's semantics: a different program is not answered here
        match crate::source_identity::identify(&module, crate::source_identity::REVIEWED_CAMPAIGN) {
            crate::source_identity::Identity::Reviewed => {}
            other => {
                return Err(Crash::new(
                    "SourceUnmatched",
                    &PyStr::new(&format!("campaign_init.py of the checkout is not the reviewed source ({other:?}); only the original program can answer for it")),
                    Where::Opaque,
                )
                .with_detail(Detail::SourceUnmatched))
            }
        }
    }
    if !present {
        put(leg4, "status", s("absent"));
        put(leg4, "detail", s("campaign_init.py 를 적재하지 못했다 -- ACTIVE 해소 규약의 단일 권위가 없으면 이 다리는 판정하지 않는다"));
        return Ok(());
    }
    let camp_id =
        campaign::active_campaign_id(Some(root), root, &module).map_err(|mut crash| {
            crash.site = match crash.site {
                Where::Campaign { module, frames } => Where::Campaign { module, frames },
                other => other,
            };
            crash
        })?;
    put(leg4, "campaign_id", PyValue::Str(camp_id.clone()));
    if camp_id.equals(campaign::BOOTSTRAP) {
        put(leg4, "status", s("not-applicable"));
        put(leg4, "detail", s("활성 캠페인이 없다(ACTIVE=_bootstrap) -- 캠페인 밖 평시 작업에서는 정상이다. 이번 판정은 이 다리를 빼고 이뤄졌다"));
        return Ok(());
    }
    let camp_bytes = crate::pyprofile::fs_encode(&camp_id)
        .map_err(|failure| encode_crash(&camp_id, &failure))?;
    let base = campaign::join(&campaign::join(root, b"campaigns"), &camp_bytes);
    let decl = PyValue::Dict(campaign::read_declaration(&base)?);
    let cvs = truthy_or(
        dict_get(&decl, "control_variables").as_ref(),
        PyValue::Dict(PyDict::new()),
    );
    let PyValue::Dict(cv_dict) = cvs.peel() else {
        return Err(attribute_error(
            &cvs,
            270,
            "cv = (decl.get(\"control_variables\") or {}).get(\"topology\")",
            "(decl.get(\"control_variables\") or {}).get",
        ));
    };
    let cv = cv_dict.get("topology").cloned().unwrap_or(PyValue::None);
    let nodes_value = truthy_or(dict_get(&decl, "nodes").as_ref(), PyValue::List(Vec::new()));
    let items = iterate(&nodes_value).map_err(|bad| {
        crash_eval("TypeError", format!("'{}' object is not iterable", bad.type_name()), 271,
            "nodes = [[n.get(\"node_id\"), n.get(\"topology\")] for n in (decl.get(\"nodes\") or [])]", "(decl.get(\"nodes\") or [])")
    })?;
    let mut nodes: Vec<(PyValue, PyValue)> = Vec::new();
    for item in &items {
        let PyValue::Dict(node) = item.peel() else {
            return Err(attribute_error(item, 271, "nodes = [[n.get(\"node_id\"), n.get(\"topology\")] for n in (decl.get(\"nodes\") or [])]", "n.get"));
        };
        nodes.push((
            node.get("node_id").cloned().unwrap_or(PyValue::None),
            node.get("topology").cloned().unwrap_or(PyValue::None),
        ));
    }
    put(leg4, "value", cv.clone());
    put(
        leg4,
        "nodes",
        PyValue::List(
            nodes
                .iter()
                .map(|(a, b)| PyValue::List(vec![a.clone(), b.clone()]))
                .collect(),
        ),
    );
    let target = topology.or(expect);
    let cv_is_topology =
        matches!(cv.peel(), PyValue::Str(text) if TOPOLOGIES.iter().any(|t| text.equals(t)));
    let camp_text = camp_id.to_utf8().unwrap_or_default();
    if !cv_is_topology {
        put(leg4, "status", s("red"));
        reason(reasons, format!("CAMPAIGN_TOPOLOGY_UNDECLARED: {camp_text} 의 control_variables.topology={} -- 선언이 비면 실행자가 저장소 기본값으로 되돌아간다", repr_of(&cv)));
    } else if let Some(target) = target {
        if !matches!(cv.peel(), PyValue::Str(text) if text.equals(target)) {
            put(leg4, "status", s("red"));
            reason(reasons, format!("CAMPAIGN_MISMATCH: 활성 캠페인 {camp_text} 는 topology={} 로 선언됐는데 체크아웃은 {} 통로다 -- 이 조합이 2026-09-11 사고의 형태다", repr_of(&cv), PyStr::new(target).repr()));
        }
    }
    let bad: Vec<&(PyValue, PyValue)> = nodes
        .iter()
        .filter(|(_, topo)| topo.is_truthy() && cv.is_truthy() && topo != &cv)
        .collect();
    if !bad.is_empty() {
        put(leg4, "status", s("red"));
        let shown: Vec<String> = bad
            .iter()
            .map(|(a, b)| format!("[{}, {}]", repr_of(a), repr_of(b)))
            .collect();
        reason(
            reasons,
            format!(
                "CAMPAIGN_NODE_MISMATCH: 노드 선언이 캠페인 통제변인과 어긋난다: [{}]",
                shown.join(", ")
            ),
        );
    }
    Ok(())
}

/// The predicate of the reviewed current sources (the campaign leg refuses a different `campaign_init.py`, see
/// [`crate::source_identity`]).
#[derive(Default)]
pub struct Predicate;

/// `evaluate(repo, expect=)`: the verdict document (read only).
pub fn evaluate(repo: &[u8], expect: Option<&str>) -> Result<PyValue, Crash> {
    Predicate.evaluate(repo, expect)
}

impl Predicate {
    pub fn evaluate(&self, repo: &[u8], expect: Option<&str>) -> Result<PyValue, Crash> {
        let root = resolve_checked(repo)?;
        let mut legs = PyDict::new();
        let mut reasons: Vec<PyStr> = Vec::new();

        // leg 1: branch
        let first = git(
            &root,
            &["symbolic-ref", "--quiet", "--short", "HEAD"],
            178,
            "branch = (_git(root, \"symbolic-ref\", \"--quiet\", \"--short\", \"HEAD\")",
            "_git(root, \"symbolic-ref\", \"--quiet\", \"--short\", \"HEAD\")",
        )?;
        let branch = match first.filter(|b| !b.code_points().is_empty()) {
            Some(b) => Some(b),
            None => git(
                &root,
                &["rev-parse", "--abbrev-ref", "HEAD"],
                179,
                "or _git(root, \"rev-parse\", \"--abbrev-ref\", \"HEAD\"))",
                "_git(root, \"rev-parse\", \"--abbrev-ref\", \"HEAD\")",
            )?,
        };
        let branch = branch.filter(|b| !b.code_points().is_empty()).or(None);
        let topology: Option<&'static str> = branch.as_ref().and_then(|b| {
            if b.equals("single-node") {
                Some("single")
            } else if b.equals("multi-node") {
                Some("multi")
            } else {
                None
            }
        });
        let mut leg1 = PyDict::new();
        put(&mut leg1, "value", topology.map_or(PyValue::None, s));
        put(
            &mut leg1,
            "raw",
            branch.clone().map_or(PyValue::None, PyValue::Str),
        );
        put(
            &mut leg1,
            "source",
            s("git symbolic-ref --short HEAD (fallback: rev-parse --abbrev-ref)"),
        );
        put(
            &mut leg1,
            "status",
            s(if topology.is_some() { "ok" } else { "red" }),
        );
        put(&mut legs, "branch", PyValue::Dict(leg1));
        if topology.is_none() {
            reason(&mut reasons, format!("BRANCH_UNKNOWN: 브랜치 {} 에서 토폴로지 통로를 고를 수 없다 -- `single-node`/`multi-node` 중 하나를 체크아웃하거나 호출부가 --expect 를 명시해야 한다 (unknown 을 single 로 삼던 침묵 기본값은 폐지됐다)", repr_opt(&branch)));
        }

        // leg 2: layer headers
        let found = layer_files(&root)?;
        let mut headers: Vec<(String, Option<&'static str>, PyStr)> = Vec::new();
        for path in &found {
            let (value, why) = read_layer_header(path);
            let rel = String::from_utf8_lossy(&path[root.len().min(path.len())..])
                .trim_start_matches('/')
                .to_string();
            headers.push((rel, value, why));
        }
        let mut declared: Vec<&'static str> = headers.iter().filter_map(|(_, v, _)| *v).collect();
        declared.sort_unstable();
        declared.dedup();
        let mut layer_status = "ok";
        let mut layer = PyDict::new();
        put(
            &mut layer,
            "value",
            if declared.len() == 1 {
                s(declared[0])
            } else {
                PyValue::None
            },
        );
        put(
            &mut layer,
            "files",
            PyValue::List(
                headers
                    .iter()
                    .map(|(path, value, why)| {
                        let mut d = PyDict::new();
                        put(&mut d, "path", s(path));
                        put(&mut d, "topology", value.map_or(PyValue::None, s));
                        put(&mut d, "detail", PyValue::Str(why.clone()));
                        PyValue::Dict(d)
                    })
                    .collect(),
            ),
        );
        put(&mut layer, "count", PyValue::int(found.len() as i64));
        put(
            &mut layer,
            "source",
            s(&format!("git ls-files -- {LAYER_PATHSPEC} (본문 자기선언)")),
        );
        if found.is_empty() {
            layer_status = "red";
            reason(&mut reasons, format!("LAYER_ABSENT: 특화헌법 파일이 0개다 -- 추적 트리에 `{LAYER_PATHSPEC}` 가 하나도 없으면 이 체크아웃은 어느 토폴로지의 헌법을 읽고 있는지 스스로 말하지 못한다"));
        } else {
            for (path, value, why) in &headers {
                match value {
                    None => {
                        layer_status = "red";
                        let mut text = PyStr::new(&format!("LAYER_HEADER_MALFORMED: {path} -- "));
                        text.push_py(why);
                        reasons.push(text);
                    }
                    Some(declared_topology) => {
                        if let Some(branch_topology) = topology {
                            if *declared_topology != branch_topology {
                                layer_status = "red";
                                reason(&mut reasons, format!("LAYER_HEADER_MISMATCH: {path} 는 topology={declared_topology} 를 선언하는데 체크아웃 브랜치는 {}({branch_topology}) 다 -- 반대 브랜치의 특화 파일이 흘러들어왔다", branch.as_ref().map(|b| b.to_utf8().unwrap_or_default()).unwrap_or_default()));
                            }
                        }
                    }
                }
            }
            if declared.len() > 1 {
                layer_status = "red";
                let list: Vec<String> = declared.iter().map(|d| format!("'{d}'")).collect();
                reason(&mut reasons, format!("LAYER_HEADER_SPLIT: 한 체크아웃에 서로 다른 topology 선언이 섞여 있다: [{}]", list.join(", ")));
            }
        }
        put(&mut layer, "status", s(layer_status));
        put(&mut legs, "layer", PyValue::Dict(layer));

        // leg 3: manifest
        let mtopo: Option<PyStr> = topology.map(PyStr::new).or_else(|| expect.map(PyStr::new));
        let mut leg3 = PyDict::new();
        put(&mut leg3, "source", s("output/<t>/manifest.yaml"));
        put(&mut leg3, "status", s("ok"));
        put(&mut leg3, "value", PyValue::None);
        put(
            &mut leg3,
            "path",
            mtopo.as_ref().map_or(PyValue::None, |m| {
                s(&format!(
                    "output/{}/manifest.yaml",
                    m.to_utf8().unwrap_or_default()
                ))
            }),
        );
        if mtopo.is_none() {
            put(&mut leg3, "status", s("skipped"));
            put(
                &mut leg3,
                "detail",
                s("통로가 정해지지 않아 읽을 manifest 가 없다(다리 1 RED 에 종속)"),
            );
        } else {
            manifest_leg(&root, mtopo.as_ref(), &mut leg3, &mut reasons)?;
        }
        put(&mut legs, "manifest", PyValue::Dict(leg3));

        // leg 4: campaign
        let mut leg4 = PyDict::new();
        put(&mut leg4, "source", s("campaigns/<ACTIVE>/campaign.yaml"));
        put(&mut leg4, "status", s("ok"));
        put(&mut leg4, "value", PyValue::None);
        campaign_leg(&root, topology, expect, &mut leg4, &mut reasons)?;
        put(&mut legs, "campaign", PyValue::Dict(leg4));

        // caller expectation
        if let Some(expect) = expect {
            if !TOPOLOGIES.contains(&expect) {
                reason(
                    &mut reasons,
                    format!(
                        "EXPECT_INVALID: --expect {} 는 single|multi 가 아니다",
                        PyStr::new(expect).repr()
                    ),
                );
            } else if let Some(t) = topology {
                if expect != t {
                    reason(
                        &mut reasons,
                        format!(
                            "EXPECT_MISMATCH: 호출부는 {} 를 기대하는데 체크아웃 통로는 {} 다",
                            PyStr::new(expect).repr(),
                            PyStr::new(t).repr()
                        ),
                    );
                }
            }
        }

        let mut judged: Vec<String> = Vec::new();
        if let Some(entries) = Some(&legs) {
            for (name, leg) in entries.iter() {
                let status = dict_get(leg, "status");
                let skip = matches!(status.as_ref().map(PyValue::peel), Some(PyValue::Str(t)) if t.equals("skipped") || t.equals("absent") || t.equals("not-applicable"));
                if !skip {
                    judged.push(name.to_utf8().unwrap_or_default());
                }
            }
        }
        judged.sort();
        let verdict = if reasons.is_empty() { "PASS" } else { "RED" };
        let mut out = PyDict::new();
        put(&mut out, "schema_version", PyValue::int(1));
        put(&mut out, "predicate", s("topology_parity"));
        put(&mut out, "policy", s("BRANCH_CONSTITUTION_LAYERING.C5"));
        put(&mut out, "verdict", s(verdict));
        put(&mut out, "topology", topology.map_or(PyValue::None, s));
        put(&mut out, "legs_checked", PyValue::int(judged.len() as i64));
        put(
            &mut out,
            "legs_judged",
            PyValue::List(judged.iter().map(|j| s(j)).collect()),
        );
        put(&mut out, "legs", PyValue::Dict(legs));
        put(
            &mut out,
            "reason_codes",
            PyValue::List(
                reasons
                    .iter()
                    .map(|r| {
                        let text = r.to_utf8().unwrap_or_default();
                        s(text.split(':').next().unwrap_or(""))
                    })
                    .collect(),
            ),
        );
        put(
            &mut out,
            "reasons",
            PyValue::List(reasons.iter().map(|r| PyValue::Str(r.clone())).collect()),
        );
        put(
            &mut out,
            "remediation",
            if reasons.is_empty() {
                PyValue::None
            } else {
                s("자동 교정하지 않는다 -- 체크아웃 전환인지 선언 수정인지는 사람이 정한다. 고친 뒤 이 술어를 다시 돌려라.")
            },
        );
        Ok(PyValue::Dict(out))
    }
}

/// `read_text` failures of the campaign leg as [`Crash`] helpers are in [`crate::campaign`].
pub fn read_failure_class(failure: &ReadFailure) -> &'static str {
    match failure {
        ReadFailure::Os(error) => os_class(error),
        ReadFailure::Decode(_) => "UnicodeDecodeError",
    }
}

pub fn subprocess_frames() -> Vec<Frame> {
    Frame::subprocess_stderr()
}
