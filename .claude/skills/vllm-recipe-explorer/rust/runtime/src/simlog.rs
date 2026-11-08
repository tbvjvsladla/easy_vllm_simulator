//! `simlog_writer.py`: the simulation-log writer of the recipe explorer, five functions over the user's own paths.
//!
//! * `new_run_dir(repo_root, run_id)`: `os.path.join(repo_root, "docs", "simlog", run_id)`, `os.makedirs(path, exist_ok=True)`, the path.
//! * `_trial_prefix(trial_number)`: `"trial%02d" % int(trial_number)`.
//! * `write_trial(run_dir, trial_number, vllm_log_text, profile_dict, candidate_dict, smoke_dict)`: `makedirs(run_dir)` first, then the
//!   prefix, then four files in order: the log, the profile JSON, the candidate (YAML when the module's `_yaml` is set, else JSON) and the
//!   smoke JSON; each is opened (and so created or truncated) before its content is prepared, written, closed; the prefix.
//! * `append_correction(run_dir, record_dict)`: `makedirs`, the line `json.dumps(record, ensure_ascii=False)` built before the open,
//!   `"a"`, one write of `line + "\n"`, the path.
//! * `write_summary(run_dir, summary_dict)`: `makedirs`, open `"w"`, `json.dump(..., ensure_ascii=False, indent=2)`, the path.
//!
//! The behaviour is the original's, including what it does not do: no atomic write, no rollback, no lock, no bounds on the trial
//! number; an earlier file or the beginning of a failed one stays. `json.dump` writes the chunks of the pure-Python encoder one by one
//! (see `PyValue::json_dump_chunks`), so what a file holds when a chunk cannot be encoded as UTF-8 (a lone surrogate) is every chunk
//! before it; a text file's `write` encodes its chunk first, so the failing chunk is not written and the bytes of the earlier ones are
//! flushed when the file is closed. `yaml.safe_dump` fails (if at all) before its first write, and the YAML it writes never needs the
//! encoding to fail (every character it cannot write is escaped), so its text is written as one piece.
//!
//! Observation: every file-system step is the real one; a [`Host`] may be told about them (the checks' control record), and production
//! passes one that is not interested. Nothing here selects a provider or looks anything up.

use crate::escalation::Raised;
use crate::model_files::{exists, fspath, is_dir, join, os_failure};
use crate::pyexpr::int_of;
use crate::recipe_set::split_path;
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pycall::{required, Param, Signature};
use agent_runtime_support::pystr::{EncodeFailure, PyStr};
use agent_runtime_support::pyvalue::{DumpError, PyDict, PyValue, StreamError};
use agent_runtime_support::pyyaml::{safe_dump_pieces, Piece};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::PathBuf;

pub const MODULE: &str = "simlog_writer";

/// The default of the module global `_yaml`: PyYAML 6.0.3 imported (`true`), or the original's `_yaml = None` fallback (`false`).
pub const YAML_VERSION: &str = "6.0.3";

/// What the checks observe of one call; production observes nothing.
pub trait Host {
    /// Whether [`Host::record`] wants its events (the event is not built otherwise).
    fn enabled(&self) -> bool {
        false
    }
    /// One file-system step, in order: `makedirs`, `makedirs_error`, `file_open`, `file_open_error`, `file_write_error`,
    /// `file_close_error`, `file_close`.
    fn record(&mut self, _name: &str, _fields: Vec<(&str, PyValue)>) {}
    /// The raw end of a file just opened. A production host keeps the real file; the checks' host may wrap it in a controlled one.
    fn raw_file(&mut self, _path: &PyStr, file: std::fs::File) -> Box<dyn Raw> {
        Box::new(FsRaw::new(file))
    }
}

/// A host that observes nothing.
pub struct Plain;

impl Host for Plain {}

/// The five arguments of a `UnicodeEncodeError` (`encoding` is always `utf-8`, the reason `surrogates not allowed`).
#[derive(Clone, Debug)]
pub struct EncodeArgs {
    pub object: PyStr,
    pub start: usize,
    pub end: usize,
}

/// An exception of a call, with the constructor arguments the builtin `UnicodeEncodeError` needs for each row of its chain (the row of
/// `raised` first, then its context, ...; `None` for a row of another class).
#[derive(Clone, Debug)]
pub struct Thrown {
    pub raised: Raised,
    pub row_args: Vec<Option<EncodeArgs>>,
}

impl Thrown {
    /// An exception whose chain has no `UnicodeEncodeError` row.
    pub fn plain(raised: Raised) -> Box<Thrown> {
        Box::new(Thrown {
            raised,
            row_args: Vec::new(),
        })
    }

    /// `top.__context__ = context`: the implicit chain, the context replacing whatever context `top` had (as the interpreter's own
    /// `_PyErr_ChainExceptions` does when a close error is chained onto the error that came before it).
    fn with_context(mut self: Box<Thrown>, context: &Thrown) -> Box<Thrown> {
        let top = self.row_args.first().cloned().flatten();
        self.raised.cause = Some(Box::new(context.raised.clone()));
        self.raised.via_context = true;
        self.raised.suppress_context = false;
        self.row_args = std::iter::once(top)
            .chain(context.row_args.iter().cloned())
            .collect();
        if self.row_args.iter().all(Option::is_none) {
            self.row_args.clear();
        }
        self
    }
}

pub type Done<T> = Result<T, Box<Thrown>>;

fn thrown(raised: Raised) -> Box<Thrown> {
    Thrown::plain(raised)
}

fn from_failure(failure: Failure) -> Box<Thrown> {
    thrown(Raised::from(failure))
}

fn encode_error(object: &PyStr, failure: EncodeFailure) -> Box<Thrown> {
    Box::new(Thrown {
        raised: Raised::plain("UnicodeEncodeError", &failure.message()),
        row_args: vec![Some(EncodeArgs {
            object: object.clone(),
            start: failure.start,
            end: failure.end,
        })],
    })
}

/// What `json` raised, as the message Python prints (without the class name the display adds).
fn dump_error(error: DumpError) -> Box<Thrown> {
    let class = error.python_type();
    let shown = error.to_string();
    let message = shown
        .strip_prefix(&format!("{class}: "))
        .unwrap_or(&shown)
        .to_string();
    thrown(Raised::plain(class, &message))
}

fn type_error(message: String) -> Box<Thrown> {
    thrown(Raised::plain("TypeError", &message))
}

// ── paths ────────────────────────────────────────────────────────────────────────────────────────

/// The path as the operating system sees it: `os.fsencode` (a lone surrogate that is not an escaped byte is the
/// `UnicodeEncodeError` for the whole path text), an embedded NUL is the `ValueError` of the system call.
fn fs_path(path: &PyStr) -> Done<PathBuf> {
    use std::os::unix::ffi::OsStrExt;
    let bytes = path
        .to_surrogateescape_bytes()
        .map_err(|failure| encode_error(path, failure))?;
    if bytes.contains(&0) {
        return Err(from_failure(Failure::new(
            "ValueError",
            "embedded null byte",
        )));
    }
    Ok(PathBuf::from(std::ffi::OsStr::from_bytes(&bytes)))
}

/// `os.mkdir(name, 0o777)` as `os.makedirs(..., exist_ok=True)` treats it: an existing directory is fine, any other `OSError` is raised.
fn mkdir(name: &PyStr) -> Done<()> {
    let native = fs_path(name)?;
    match std::fs::DirBuilder::new().mode(0o777).create(&native) {
        Ok(()) => Ok(()),
        Err(error) => {
            if is_dir(name) {
                Ok(())
            } else {
                Err(from_failure(os_failure(&error, Some(name))))
            }
        }
    }
}

/// `os.makedirs(name, exist_ok=True)` (default mode, the process umask): the missing parents first, as the original does.
fn makedirs_text(name: &PyStr) -> Done<()> {
    let (mut head, mut tail) = split_path(name);
    if tail.is_empty() {
        (head, tail) = split_path(&head);
    }
    if !head.is_empty() && !tail.is_empty() && !exists(&head) {
        match makedirs_text(&head) {
            Ok(()) => {}
            Err(failure) if failure.raised.class == "FileExistsError" => {}
            Err(failure) => return Err(failure),
        }
        if tail.equals(".") {
            return Ok(());
        }
    }
    mkdir(name)
}

/// `os.makedirs(path, exist_ok=True)` of the module's own `os` (the call the checks observe at the top level only).
fn makedirs(host: &mut dyn Host, path: &PyValue) -> Done<()> {
    if host.enabled() {
        let mut keywords = PyDict::new();
        keywords.set("exist_ok", PyValue::Bool(true));
        host.record(
            "makedirs",
            vec![
                ("path", path.clone()),
                ("args", PyValue::List(Vec::new())),
                ("kwargs", PyValue::Dict(keywords)),
            ],
        );
    }
    let outcome = match path.peel() {
        PyValue::Str(text) => makedirs_text(text),
        other => Err(type_error(format!(
            "expected str, bytes or os.PathLike object, not {}",
            other.type_name()
        ))),
    };
    if let Err(failure) = &outcome {
        if host.enabled() {
            host.record(
                "makedirs_error",
                vec![
                    ("type", PyValue::str(&failure.raised.class)),
                    ("message", PyValue::Str(failure.raised.message.clone())),
                ],
            );
        }
    }
    outcome
}

/// `os.path.join(a, b)` for the run directory (a `str` by then: `makedirs` accepted it) and a file name.
fn join_name(run_dir: &PyValue, name: &PyStr) -> PyStr {
    match run_dir.peel() {
        PyValue::Str(dir) => join(dir, name),
        _ => unreachable!("a run directory that is not text failed in makedirs"),
    }
}

// ── text files ───────────────────────────────────────────────────────────────────────────────────

/// The operating-system end of a file: what `io.FileIO` is under a text file. The real one is [`FsRaw`]; the checks may put a
/// controlled one in its place through [`Host::raw_file`] (a production host never does).
pub trait Raw {
    /// `FileIO.write`: the number of bytes the system accepted (possibly fewer than given).
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize>;
    /// `FileIO.close`.
    fn close(&mut self) -> std::io::Result<()>;
    /// The buffer size `open()` gives the buffered writer: the file's `st_blksize` when it is above 1, else 8 KiB.
    fn buffer_size(&self) -> usize {
        8192
    }
}

/// A real file.
pub struct FsRaw {
    file: Option<std::fs::File>,
    buffer_size: usize,
    /// The checks' stand-in for the system `close` call (the one seam, absent from a production build).
    #[cfg(feature = "fixture")]
    closer: Option<Box<dyn FnOnce(std::os::fd::RawFd) -> i32>>,
}

extern "C" {
    fn close(descriptor: i32) -> i32;
}

/// The system `close(2)` of a descriptor the caller owns.
fn system_close(descriptor: std::os::fd::RawFd) -> i32 {
    // SAFETY: the descriptor was just taken out of an owned `File` by `close_owned` (`into_raw_fd`), so nothing else holds or will close
    // it, and this is the only call that closes it; `close` takes no pointer.
    unsafe { close(descriptor) }
}

/// `FileIO.close`: the file is consumed exactly once and its descriptor is released by the one `call` (never by a `Drop`, so it is not
/// closed twice); a negative result is the operating system's own error, read from `errno` right away, and is not retried (after
/// `EINTR` the descriptor may already be released, and another thread may have been given its number).
fn close_owned(
    file: std::fs::File,
    call: impl FnOnce(std::os::fd::RawFd) -> i32,
) -> std::io::Result<()> {
    use std::os::fd::IntoRawFd;
    let descriptor = file.into_raw_fd();
    if call(descriptor) < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

impl FsRaw {
    pub fn new(file: std::fs::File) -> FsRaw {
        use std::os::unix::fs::MetadataExt;
        let buffer_size = match file.metadata().map(|metadata| metadata.blksize()) {
            Ok(size) if size > 1 => usize::try_from(size).unwrap_or(8192),
            _ => 8192,
        };
        FsRaw {
            file: Some(file),
            buffer_size,
            #[cfg(feature = "fixture")]
            closer: None,
        }
    }

    /// A real file whose close is made by `call` (it must release the descriptor and may return a negative result with `errno` set):
    /// for the checks only (`--features fixture`).
    #[cfg(feature = "fixture")]
    pub fn with_close_call(
        file: std::fs::File,
        call: Box<dyn FnOnce(std::os::fd::RawFd) -> i32>,
    ) -> FsRaw {
        let mut raw = FsRaw::new(file);
        raw.closer = Some(call);
        raw
    }
}

impl Raw for FsRaw {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        let file = self
            .file
            .as_mut()
            .ok_or_else(|| std::io::Error::other("write to a closed file"))?;
        loop {
            match file.write(data) {
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                other => return other,
            }
        }
    }

    /// Closing the closed is nothing; otherwise the descriptor is released whatever came before, and the system's error is returned.
    fn close(&mut self) -> std::io::Result<()> {
        let Some(file) = self.file.take() else {
            return Ok(());
        };
        #[cfg(feature = "fixture")]
        if let Some(call) = self.closer.take() {
            return close_owned(file, call);
        }
        close_owned(file, system_close)
    }

    fn buffer_size(&self) -> usize {
        self.buffer_size
    }
}

/// `TextIOWrapper._CHUNK_SIZE`.
const TEXT_CHUNK: usize = 8192;

/// `open(path, mode, encoding="utf-8")` as CPython builds it: a `TextIOWrapper` (pending bytes up to its chunk size) over a
/// `BufferedWriter` (its own buffer of `buffer_size` bytes) over the raw file. Writes follow the C implementation's thresholds
/// (observed with a controlled `io.TextIOWrapper` over `io.BufferedWriter` over a recording raw object, retained in the checks'
/// data): a text write is encoded first; a chunk of 8192 bytes or more flushes what was pending before it is added; the pending bytes
/// are handed to the buffered writer when they reach 8192; the buffered writer copies what fits, otherwise flushes its buffer and
/// writes directly whatever is larger than its buffer. Nothing is flushed by `Drop`: a file the caller does not close keeps its bytes.
pub struct TextFile {
    path: PyStr,
    raw: Box<dyn Raw>,
    buffer_size: usize,
    buffer: Vec<u8>,
    flushed: usize,
    pending: Vec<u8>,
}

fn io_error(error: &std::io::Error, handled: Option<&Thrown>) -> Box<Thrown> {
    let failure = thrown(Raised::from(os_failure(error, None)));
    match handled {
        Some(handled) => failure.with_context(handled),
        None => failure,
    }
}

impl TextFile {
    pub fn new(path: PyStr, raw: Box<dyn Raw>) -> TextFile {
        let buffer_size = raw.buffer_size();
        TextFile {
            path,
            raw,
            buffer_size,
            buffer: Vec::new(),
            flushed: 0,
            pending: Vec::new(),
        }
    }

    /// `raw.write(data)` once; a count of zero would loop for ever in CPython and is an error here.
    fn raw_write(&mut self, data: &[u8], handled: Option<&Thrown>) -> Done<usize> {
        match self.raw.write(data) {
            Ok(0) if !data.is_empty() => Err(io_error(
                &std::io::Error::from(std::io::ErrorKind::WriteZero),
                handled,
            )),
            Ok(count) => Ok(count.min(data.len())),
            Err(error) => Err(io_error(&error, handled)),
        }
    }

    /// `BufferedWriter.flush`: the buffered bytes reach the raw file; a failure keeps what was not written.
    fn buffered_flush(&mut self, handled: Option<&Thrown>) -> Done<()> {
        while self.flushed < self.buffer.len() {
            let data = self.buffer[self.flushed..].to_vec();
            let written = self.raw_write(&data, handled)?;
            self.flushed += written;
        }
        self.buffer.clear();
        self.flushed = 0;
        Ok(())
    }

    /// `BufferedWriter.write`.
    fn buffered_write(&mut self, data: &[u8], handled: Option<&Thrown>) -> Done<()> {
        let available = self
            .buffer_size
            .saturating_sub(self.buffer.len() - self.flushed);
        if data.len() <= available {
            self.buffer.extend_from_slice(data);
            return Ok(());
        }
        self.buffered_flush(handled)?;
        let mut written = 0;
        while data.len() - written > self.buffer_size {
            written += self.raw_write(&data[written..], handled)?;
        }
        if written < data.len() {
            self.buffer.extend_from_slice(&data[written..]);
        }
        Ok(())
    }

    /// `TextIOWrapper._textiowrapper_writeflush`: the pending bytes go to the buffered writer; they are gone even if that fails.
    fn write_flush(&mut self, handled: Option<&Thrown>) -> Done<()> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let data = std::mem::take(&mut self.pending);
        self.buffered_write(&data, handled)
    }

    /// `fh.write(chunk)` for already encoded bytes: the text wrapper's thresholds.
    fn write_bytes(&mut self, bytes: &[u8]) -> Done<()> {
        if bytes.len() >= TEXT_CHUNK && !self.pending.is_empty() {
            self.write_flush(None)?;
        }
        self.pending.extend_from_slice(bytes);
        if self.pending.len() >= TEXT_CHUNK {
            self.write_flush(None)?;
        }
        Ok(())
    }

    fn record_failed_write(&self, host: &mut dyn Host, failure: &Thrown) {
        if host.enabled() {
            host.record(
                "file_write_error",
                vec![
                    ("path", PyValue::Str(self.path.clone())),
                    ("type", PyValue::str(&failure.raised.class)),
                    ("message", PyValue::Str(failure.raised.message.clone())),
                ],
            );
        }
    }

    /// `fh.write(chunk)` for text: the chunk is encoded first (a lone surrogate fails the whole chunk), then it is written through the
    /// buffers; any failure is a `file_write_error` of the observed file.
    pub fn write_text(&mut self, host: &mut dyn Host, chunk: &PyStr) -> Done<()> {
        let outcome = encode_chunk(chunk).and_then(|bytes| self.write_bytes(&bytes));
        if let Err(failure) = &outcome {
            self.record_failed_write(host, failure);
        }
        outcome
    }

    /// `fh.write(value)` for a value of any type: only text is accepted.
    pub fn write_value(&mut self, host: &mut dyn Host, value: &PyValue) -> Done<()> {
        match value.peel() {
            PyValue::Str(text) => self.write_text(host, text),
            other => {
                let failure = type_error(format!(
                    "write() argument must be str, not {}",
                    other.type_name()
                ));
                self.record_failed_write(host, &failure);
                Err(failure)
            }
        }
    }

    /// `fh.flush()`: the pending bytes, then the buffer, reach the raw file. The collector's file wrapper forwards `flush` untouched, so a
    /// failure here is not a `file_write_error` event.
    pub fn flush(&mut self) -> Done<()> {
        self.write_flush(None)?;
        self.buffered_flush(None)
    }

    /// `TextIOWrapper.close`: flush; then the buffered writer's close (a second flush, then the raw close); a failure of the first
    /// is the context of a failure of the second (the later error keeps no older context), and `handled` (the exception of the
    /// `with` body) is the context of every error raised while closing, the chained ones replacing it as the interpreter does.
    fn close_chain(&mut self, handled: Option<&Thrown>) -> Done<()> {
        let first = match self.write_flush(handled) {
            Ok(()) => self.buffered_flush(handled).err(),
            Err(failure) => Some(failure),
        };
        let second_flush = self.buffered_flush(handled).err();
        let raw_close = self
            .raw
            .close()
            .map_err(|error| io_error(&error, handled))
            .err();
        let buffered = match (second_flush, raw_close) {
            (None, None) => None,
            (Some(flush), None) => Some(flush),
            (None, Some(close)) => Some(close),
            (Some(flush), Some(close)) => Some(close.with_context(&flush)),
        };
        match (first, buffered) {
            (None, None) => Ok(()),
            (Some(first), None) => Err(first),
            (None, Some(buffered)) => Err(buffered),
            (Some(first), Some(buffered)) => Err(buffered.with_context(&first)),
        }
    }

    /// The end of the `with` block: closes the file and reports it as the observed `file_close` / `file_close_error`.
    fn close(mut self, host: &mut dyn Host, handled: Option<&Thrown>) -> Done<()> {
        let outcome = self.close_chain(handled);
        if host.enabled() {
            match &outcome {
                Ok(()) => host.record(
                    "file_close",
                    vec![("path", PyValue::Str(self.path.clone()))],
                ),
                Err(failure) => host.record(
                    "file_close_error",
                    vec![
                        ("path", PyValue::Str(self.path.clone())),
                        ("type", PyValue::str(&failure.raised.class)),
                        ("message", PyValue::Str(failure.raised.message.clone())),
                    ],
                ),
            }
        }
        outcome
    }
}

fn open_text(host: &mut dyn Host, path: &PyStr, append: bool) -> Done<TextFile> {
    if host.enabled() {
        let mut keywords = PyDict::new();
        keywords.set("encoding", PyValue::str("utf-8"));
        host.record(
            "file_open",
            vec![
                ("path", PyValue::Str(path.clone())),
                (
                    "args",
                    PyValue::List(vec![PyValue::str(if append { "a" } else { "w" })]),
                ),
                ("kwargs", PyValue::Dict(keywords)),
            ],
        );
    }
    let opened = fs_path(path).and_then(|native| {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).mode(0o666);
        if append {
            options.append(true);
        } else {
            options.truncate(true);
        }
        options
            .open(&native)
            .map_err(|error| from_failure(os_failure(&error, Some(path))))
    });
    match opened {
        Ok(file) => {
            let raw = host.raw_file(path, file);
            Ok(TextFile::new(path.clone(), raw))
        }
        Err(failure) => {
            if host.enabled() {
                host.record(
                    "file_open_error",
                    vec![
                        ("path", PyValue::Str(path.clone())),
                        ("type", PyValue::str(&failure.raised.class)),
                        ("message", PyValue::Str(failure.raised.message.clone())),
                    ],
                );
            }
            Err(failure)
        }
    }
}

/// `text.encode("utf-8")` as `TextIOWrapper.write` does it: a lone surrogate is a `UnicodeEncodeError` (the whole chunk is the object,
/// the first run of surrogates the positions), and nothing of the chunk is accepted.
fn encode_chunk(chunk: &PyStr) -> Done<Vec<u8>> {
    let points = chunk.code_points();
    let surrogate = |point: u32| (0xD800..=0xDFFF).contains(&point);
    if let Some(start) = points.iter().position(|&point| surrogate(point)) {
        let end = start
            + points[start..]
                .iter()
                .take_while(|&&point| surrogate(point))
                .count();
        return Err(encode_error(
            chunk,
            EncodeFailure {
                start,
                end,
                first: points[start],
            },
        ));
    }
    Ok(chunk.to_utf8().expect("no surrogate").into_bytes())
}

/// `with open(path, mode, encoding="utf-8") as fh: body(fh)`: the file is closed whether or not the body raised. A body that raised and
/// a close that fails both end the block with the close failure, the body's exception as its context (as `__exit__` raising inside the
/// handler does); a close failure alone has no context; the body's exception alone is the body's.
fn with_text<T>(
    host: &mut dyn Host,
    path: &PyStr,
    append: bool,
    body: impl FnOnce(&mut dyn Host, &mut TextFile) -> Done<T>,
) -> Done<T> {
    let file = open_text(host, path, append)?;
    run_in_block(host, file, body)
}

/// The block of a `with` over an open file: `body`, then the close, with the exceptions combined as described at [`with_text`].
pub fn run_in_block<T>(
    host: &mut dyn Host,
    mut file: TextFile,
    body: impl FnOnce(&mut dyn Host, &mut TextFile) -> Done<T>,
) -> Done<T> {
    let outcome = body(host, &mut file);
    let handled = outcome.as_ref().err().map(|failure| &**failure);
    let closed = file.close(host, handled);
    match (outcome, closed) {
        (Ok(value), Ok(())) => Ok(value),
        (Ok(_), Err(failure)) | (Err(_), Err(failure)) => Err(failure),
        (Err(failure), Ok(())) => Err(failure),
    }
}

/// `json.dump(value, fh, ensure_ascii=False, indent=2)`: the encoder's chunks go to the file one by one.
fn dump_json(host: &mut dyn Host, file: &mut TextFile, value: &PyValue) -> Done<()> {
    match value.json_dump_chunks(2, &mut |chunk: &PyStr| file.write_text(host, chunk)) {
        Ok(()) => Ok(()),
        Err(StreamError::Sink(failure)) => Err(failure),
        Err(StreamError::Dump(error)) => Err(dump_error(error)),
    }
}

// ── the five functions ───────────────────────────────────────────────────────────────────────────

/// `os.path.join(repo_root, "docs", "simlog", run_id)`: a `repo_root` that is not a path fails first (`os.fspath`), then the first
/// component that is not (`join() argument must be ...`, raised `from None`).
fn join_run(repo_root: &PyValue, run_id: &PyValue) -> Done<PyStr> {
    let root = fspath(repo_root, false).map_err(from_failure)?;
    let id = match run_id.peel() {
        PyValue::Str(text) => text,
        other => {
            let mut failure = Raised::plain(
                "TypeError",
                &format!(
                    "join() argument must be str, bytes, or os.PathLike object, not '{}'",
                    other.type_name()
                ),
            );
            failure.suppress_context = true;
            return Err(thrown(failure));
        }
    };
    let docs = join(root, &PyStr::new("docs"));
    let simlog = join(&docs, &PyStr::new("simlog"));
    Ok(join(&simlog, id))
}

pub fn new_run_dir(host: &mut dyn Host, repo_root: &PyValue, run_id: &PyValue) -> Done<PyValue> {
    let run_dir = PyValue::Str(join_run(repo_root, run_id)?);
    makedirs(host, &run_dir)?;
    Ok(run_dir)
}

/// `"trial%02d" % int(trial_number)`.
pub fn trial_prefix(trial_number: &PyValue) -> Done<PyStr> {
    let number = int_of(trial_number.peel()).map_err(from_failure)?;
    let digits = number.to_string();
    let magnitude = digits.trim_start_matches('-');
    if magnitude.len() > 4300 {
        return Err(thrown(Raised::plain(
            "ValueError",
            "Exceeds the limit (4300 digits) for integer string conversion; use sys.set_int_max_str_digits() to increase the limit",
        )));
    }
    let shown = if digits.starts_with('-') || magnitude.len() >= 2 {
        digits
    } else {
        format!("0{magnitude}")
    };
    Ok(PyStr::new(&format!("trial{shown}")))
}

/// The candidate's serialization: YAML when the module's `_yaml` is set, else the JSON the other files use.
pub fn write_trial(
    host: &mut dyn Host,
    yaml_enabled: bool,
    arguments: [&PyValue; 6],
) -> Done<PyValue> {
    let [run_dir, trial_number, log_text, profile, candidate, smoke] = arguments;
    makedirs(host, run_dir)?;
    let prefix = trial_prefix(trial_number)?;
    let named = |suffix: &str| {
        let mut name = prefix.clone();
        name.push_str(suffix);
        join_name(run_dir, &name)
    };

    let log_path = named("_vllm.log");
    with_text(host, &log_path, false, |host, file| {
        if matches!(log_text.peel(), PyValue::None) {
            file.write_text(host, &PyStr::default())
        } else {
            file.write_value(host, log_text)
        }
    })?;

    let profile_path = named("_profile.json");
    with_text(host, &profile_path, false, |host, file| {
        dump_json(host, file, profile)
    })?;

    let candidate_path = named("_candidate.yaml");
    with_text(host, &candidate_path, false, |host, file| {
        if yaml_enabled {
            let pieces = safe_dump_pieces(candidate).map_err(|error| {
                let module = match error.class {
                    "RepresenterError" => "yaml.representer",
                    _ => "builtins",
                };
                thrown(Raised::new(error.class, module, error.message))
            })?;
            for piece in pieces {
                match piece {
                    Piece::Write(text) => file.write_text(host, &text)?,
                    Piece::Flush => file.flush()?,
                }
            }
            Ok(())
        } else {
            dump_json(host, file, candidate)
        }
    })?;

    let smoke_path = named("_smoke.json");
    with_text(host, &smoke_path, false, |host, file| {
        dump_json(host, file, smoke)
    })?;

    Ok(PyValue::Str(prefix))
}

pub fn append_correction(
    host: &mut dyn Host,
    run_dir: &PyValue,
    record: &PyValue,
) -> Done<PyValue> {
    makedirs(host, run_dir)?;
    let path = join_name(run_dir, &PyStr::new("correction_history.jsonl"));
    let mut line = record.dumps_unicode().map_err(dump_error)?;
    with_text(host, &path, true, |host, file| {
        line.push_str("\n");
        file.write_text(host, &line)
    })?;
    Ok(PyValue::Str(path))
}

pub fn write_summary(host: &mut dyn Host, run_dir: &PyValue, summary: &PyValue) -> Done<PyValue> {
    makedirs(host, run_dir)?;
    let path = join_name(run_dir, &PyStr::new("run_summary.json"));
    with_text(host, &path, false, |host, file| {
        dump_json(host, file, summary)
    })?;
    Ok(PyValue::Str(path))
}

// ── binding ──────────────────────────────────────────────────────────────────────────────────────

/// The functions of the module the service and the checks reach.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Function {
    NewRunDir,
    TrialPrefix,
    WriteTrial,
    AppendCorrection,
    WriteSummary,
}

const NEW_RUN_DIR: &[Param] = &[required("repo_root"), required("run_id")];
const TRIAL_PREFIX: &[Param] = &[required("trial_number")];
const WRITE_TRIAL: &[Param] = &[
    required("run_dir"),
    required("trial_number"),
    required("vllm_log_text"),
    required("profile_dict"),
    required("candidate_dict"),
    required("smoke_dict"),
];
const APPEND_CORRECTION: &[Param] = &[required("run_dir"), required("record_dict")];
const WRITE_SUMMARY: &[Param] = &[required("run_dir"), required("summary_dict")];

impl Function {
    /// Resolves the name of a function of the module (`new_run_dir`, `_trial_prefix`, ...).
    pub fn named(name: &str) -> Option<Function> {
        Some(match name {
            "new_run_dir" => Function::NewRunDir,
            "_trial_prefix" => Function::TrialPrefix,
            "write_trial" => Function::WriteTrial,
            "append_correction" => Function::AppendCorrection,
            "write_summary" => Function::WriteSummary,
            _ => return None,
        })
    }

    fn signature(self) -> Signature {
        match self {
            Function::NewRunDir => Signature {
                name: "new_run_dir",
                params: NEW_RUN_DIR,
                rest: false,
            },
            Function::TrialPrefix => Signature {
                name: "_trial_prefix",
                params: TRIAL_PREFIX,
                rest: false,
            },
            Function::WriteTrial => Signature {
                name: "write_trial",
                params: WRITE_TRIAL,
                rest: false,
            },
            Function::AppendCorrection => Signature {
                name: "append_correction",
                params: APPEND_CORRECTION,
                rest: false,
            },
            Function::WriteSummary => Signature {
                name: "write_summary",
                params: WRITE_SUMMARY,
                rest: false,
            },
        }
    }
}

/// Binds the arguments as Python does and runs the function; `yaml_enabled` is the module's `_yaml` (only `write_trial` reads it).
pub fn call(
    function: Function,
    host: &mut dyn Host,
    yaml_enabled: bool,
    args: &[PyValue],
    kwargs: &PyDict,
) -> Done<PyValue> {
    let signature = function.signature();
    let bound = signature.bind(args, kwargs).map_err(from_failure)?;
    match function {
        Function::NewRunDir => new_run_dir(host, &bound[0], &bound[1]),
        Function::TrialPrefix => trial_prefix(&bound[0]).map(PyValue::Str),
        Function::WriteTrial => write_trial(
            host,
            yaml_enabled,
            [
                &bound[0], &bound[1], &bound[2], &bound[3], &bound[4], &bound[5],
            ],
        ),
        Function::AppendCorrection => append_correction(host, &bound[0], &bound[1]),
        Function::WriteSummary => write_summary(host, &bound[0], &bound[1]),
    }
}

/// `yaml_version` of the profile: the module's `_yaml.__version__`, or `None` for `_yaml = None`.
pub fn yaml_version(yaml_enabled: bool) -> PyValue {
    if yaml_enabled {
        PyValue::str(YAML_VERSION)
    } else {
        PyValue::None
    }
}

/// The module global as a value: `{"yaml_enabled", "yaml_version"}`.
pub fn profile_value(yaml_enabled: bool) -> PyValue {
    let mut profile = PyDict::new();
    profile.set("yaml_enabled", PyValue::Bool(yaml_enabled));
    profile.set("yaml_version", yaml_version(yaml_enabled));
    PyValue::Dict(profile)
}
