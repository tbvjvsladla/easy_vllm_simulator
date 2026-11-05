//! The file-system operations the repeat axis performs, with the observable behaviour of CPython 3.12.3's
//! `pathlib` and `open()`: path text normalisation, `Path.mkdir(parents=True, exist_ok=True)`, `Path.is_dir()`,
//! `Path.write_text()` and `Path.read_text()`.
//!
//! Failures are returned as [`Failure`]s carrying the exception class and `str(exception)` the original raises.
//! Mutation order matters and is kept: `write_text` creates or truncates the file *before* it encodes the text, so
//! text that cannot be encoded leaves an empty file behind, exactly as the original does.

use crate::cli::os_error_text;
use crate::guidellm::Failure;
use crate::pyjson::{decode_file_text, loads_text, LoadError};
use crate::pystr::{EncodeFailure, PyStr};
use crate::pyvalue::PyValue;
use std::ffi::OsString;
use std::io::Write;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::Path;

/// `errno` values `Path.is_dir()` and `Path.is_file()` treat as "not that kind of file" instead of raising.
const IGNORED_ERRNOS: [i32; 4] = [2, 20, 9, 40]; // ENOENT, ENOTDIR, EBADF, ELOOP

/// A failed file-system call, before it is turned into the exception the original shows.
#[derive(Debug)]
pub enum FsFailure {
    Os {
        error: std::io::Error,
        path: PyStr,
    },
    /// `ValueError: embedded null byte`.
    EmbeddedNul,
    /// The path text holds a lone surrogate that `os.fsencode` cannot encode.
    Encode(EncodeFailure),
}

impl FsFailure {
    pub fn into_failure(self) -> Failure {
        match self {
            FsFailure::Os { error, path } => {
                Failure::new(os_error_class(&error), &os_error_text(&error, &path))
            }
            FsFailure::EmbeddedNul => Failure::new("ValueError", "embedded null byte"),
            FsFailure::Encode(failure) => Failure::new("UnicodeEncodeError", &failure.message()),
        }
    }

    /// Whether `Path.is_dir()`/`is_file()` report `False` instead of raising.
    fn is_ignored_by_stat_predicates(&self) -> bool {
        match self {
            FsFailure::Os { error, .. } => error
                .raw_os_error()
                .is_some_and(|errno| IGNORED_ERRNOS.contains(&errno)),
            FsFailure::EmbeddedNul | FsFailure::Encode(_) => true,
        }
    }
}

/// The `OSError` subclass Python raises for an operating-system error number (Linux numbering).
pub fn os_error_class(error: &std::io::Error) -> &'static str {
    match error.raw_os_error() {
        Some(1 | 13) => "PermissionError",
        Some(2) => "FileNotFoundError",
        Some(3) => "ProcessLookupError",
        Some(4) => "InterruptedError",
        Some(10) => "ChildProcessError",
        Some(11) => "BlockingIOError",
        Some(17) => "FileExistsError",
        Some(20) => "NotADirectoryError",
        Some(21) => "IsADirectoryError",
        Some(32 | 108) => "BrokenPipeError",
        Some(103) => "ConnectionAbortedError",
        Some(104) => "ConnectionResetError",
        Some(110) => "TimeoutError",
        Some(111) => "ConnectionRefusedError",
        _ => "OSError",
    }
}

/// `os.fsencode(path)`: UTF-8 with `surrogateescape`; an embedded NUL is a `ValueError`.
pub fn os_string(path: &PyStr) -> Result<OsString, FsFailure> {
    let bytes = path.to_surrogateescape_bytes().map_err(FsFailure::Encode)?;
    if bytes.contains(&0) {
        return Err(FsFailure::EmbeddedNul);
    }
    Ok(OsString::from_vec(bytes))
}

fn os_failure(error: std::io::Error, path: &PyStr) -> FsFailure {
    FsFailure::Os {
        error,
        path: path.clone(),
    }
}

// ── pathlib text ────────────────────────────────────────────────────────────────

/// `str(PurePosixPath(text))`: empty and `.` components dropped, `..` kept, a leading `//` (exactly two slashes)
/// preserved, an empty result spelled `.`.
pub fn normalize(text: &PyStr) -> PyStr {
    let slash = u32::from('/');
    let points = text.code_points();
    let leading = points.iter().take_while(|&&p| p == slash).count();
    let root: &[u32] = match leading {
        0 => &[],
        2 => &[slash, slash],
        _ => &[slash],
    };
    let mut out = root.to_vec();
    let mut first = true;
    for part in points[leading..].split(|&p| p == slash) {
        if part.is_empty() || part == [u32::from('.')] {
            continue;
        }
        if !first {
            out.push(slash);
        }
        out.extend_from_slice(part);
        first = false;
    }
    if out.is_empty() {
        return PyStr::new(".");
    }
    PyStr::from_code_points(out)
}

/// `str(Path(base) / name)` for a relative `name` without separators.
pub fn join(base: &PyStr, name: &str) -> PyStr {
    let mut joined = base.clone();
    if !joined.ends_with_code_point(u32::from('/')) {
        joined.push_str("/");
    }
    joined.push_str(name);
    normalize(&joined)
}

/// `str(Path(base) / child)` where `child` is any string (an absolute `child` replaces `base`).
pub fn join_text(base: &PyStr, child: &PyStr) -> PyStr {
    if child.code_points().first() == Some(&u32::from('/')) {
        return normalize(child);
    }
    let mut joined = base.clone();
    joined.push_str("/");
    joined.push_py(child);
    normalize(&joined)
}

/// `str(Path(path).parent)`.
pub fn parent(path: &PyStr) -> PyStr {
    let points = path.code_points();
    let slash = u32::from('/');
    match points.iter().rposition(|&p| p == slash) {
        None => PyStr::new("."),
        Some(0) => PyStr::new("/"),
        Some(at) => PyStr::from_code_points(points[..at].to_vec()),
    }
}

// ── stat predicates ─────────────────────────────────────────────────────────────

fn metadata(path: &PyStr) -> Result<std::fs::Metadata, FsFailure> {
    let os = os_string(path)?;
    std::fs::metadata(Path::new(&os)).map_err(|error| os_failure(error, path))
}

/// `Path.is_dir()`; an error other than the ignored ones (for example `EACCES`) is raised.
pub fn is_dir(path: &PyStr) -> Result<bool, Failure> {
    match metadata(path) {
        Ok(data) => Ok(data.is_dir()),
        Err(failure) if failure.is_ignored_by_stat_predicates() => Ok(false),
        Err(failure) => Err(failure.into_failure()),
    }
}

/// `Path.is_file()`.
pub fn is_file(path: &PyStr) -> Result<bool, Failure> {
    match metadata(path) {
        Ok(data) => Ok(data.is_file()),
        Err(failure) if failure.is_ignored_by_stat_predicates() => Ok(false),
        Err(failure) => Err(failure.into_failure()),
    }
}

// ── mkdir ───────────────────────────────────────────────────────────────────────

/// `Path(path).mkdir(parents=True, exist_ok=True)` (CPython 3.12.3 `pathlib.Path.mkdir`): try the directory, create
/// the parent recursively when it is missing, and accept an existing directory.
pub fn mkdir_parents_exist_ok(path: &PyStr) -> Result<(), Failure> {
    create_directory(path, true)
}

fn create_directory(path: &PyStr, parents: bool) -> Result<(), Failure> {
    let os = os_string(path).map_err(FsFailure::into_failure)?;
    match std::fs::create_dir(Path::new(&os)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let up = parent(path);
            if !parents || up == *path {
                return Err(os_failure(error, path).into_failure());
            }
            create_directory(&up, true)?;
            // `self.mkdir(mode, parents=False, exist_ok=exist_ok)`
            create_directory(path, false).or_else(|raised| existing_directory_is_fine(path, raised))
        }
        Err(error) => existing_directory_is_fine(path, os_failure(error, path).into_failure()),
    }
}

/// `except OSError: if not exist_ok or not self.is_dir(): raise`
fn existing_directory_is_fine(path: &PyStr, raised: Failure) -> Result<(), Failure> {
    if is_dir(path)? {
        Ok(())
    } else {
        Err(raised)
    }
}

// ── read and write text ─────────────────────────────────────────────────────────

/// `Path.write_text(text, encoding="utf-8")`: open (create or truncate) first, then encode strictly, then write.
pub fn write_text(path: &PyStr, text: &PyStr) -> Result<(), Failure> {
    let os = os_string(path).map_err(FsFailure::into_failure)?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(Path::new(&os))
        .map_err(|error| os_failure(error, path).into_failure())?;
    let bytes = strict_utf8(text)
        .map_err(|failure| Failure::new("UnicodeEncodeError", &failure.message()))?;
    file.write_all(&bytes).map_err(|error| {
        let detail = error.to_string();
        let message = detail
            .split(" (os error")
            .next()
            .unwrap_or(&detail)
            .to_string();
        let text = match error.raw_os_error() {
            Some(errno) => format!("[Errno {errno}] {message}"),
            None => message,
        };
        Failure::new(os_error_class(&error), &text)
    })
}

/// [`strict_utf8`] appended to `out` without a buffer of its own. On failure `out` is left as it was and the error is the
/// one [`strict_utf8`] reports.
pub fn strict_utf8_into(text: &PyStr, out: &mut Vec<u8>) -> Result<(), EncodeFailure> {
    let mark = out.len();
    let mut buffer = [0_u8; 4];
    for &point in text.code_points() {
        match char::from_u32(point) {
            Some(character) => out.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes()),
            None => {
                out.truncate(mark);
                return Err(strict_utf8(text).expect_err("a surrogate was found"));
            }
        }
    }
    Ok(())
}

/// `text.encode("utf-8")`: any surrogate is a `UnicodeEncodeError`.
pub fn strict_utf8(text: &PyStr) -> Result<Vec<u8>, EncodeFailure> {
    let points = text.code_points();
    let is_surrogate = |p: u32| (0xD800..=0xDFFF).contains(&p);
    if let Some(start) = points.iter().position(|&p| is_surrogate(p)) {
        let end = start
            + points[start..]
                .iter()
                .take_while(|&&p| is_surrogate(p))
                .count();
        return Err(EncodeFailure {
            start,
            end,
            first: points[start],
        });
    }
    Ok(text
        .to_utf8()
        .expect("text without surrogates is UTF-8")
        .into_bytes())
}

/// The names in a directory in the order the operating system lists them (`os.listdir`).
pub fn list_dir(path: &PyStr) -> Result<Vec<PyStr>, Failure> {
    let os = os_string(path).map_err(FsFailure::into_failure)?;
    let entries = std::fs::read_dir(Path::new(&os))
        .map_err(|error| os_failure(error, path).into_failure())?;
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| os_failure(error, path).into_failure())?;
        names.push(PyStr::from_surrogateescape(entry.file_name().as_bytes()));
    }
    Ok(names)
}

/// `_load_json`: the JSON document of a file, or `None` for any `OSError` and `ValueError` (unreadable, not UTF-8,
/// not JSON). A document nested beyond the interpreter's recursion limit is a `RecursionError`, which the original
/// does not catch.
pub fn load_json_or_none(path: &PyStr) -> Result<Option<PyValue>, Failure> {
    let Ok(os) = os_string(path) else {
        return Ok(None);
    };
    let Ok(bytes) = std::fs::read(Path::new(&os)) else {
        return Ok(None);
    };
    let Ok(text) = decode_file_text(&bytes) else {
        return Ok(None);
    };
    match loads_text(&text) {
        Ok(value) => Ok(Some(value)),
        Err(error @ LoadError::TooDeep { .. }) => {
            let text = error.to_string();
            Err(Failure::new(
                "RecursionError",
                text.strip_prefix("RecursionError: ").unwrap_or(&text),
            ))
        }
        Err(_) => Ok(None),
    }
}

/// `Path.read_bytes()`.
pub fn read_bytes(path: &PyStr) -> Result<Vec<u8>, Failure> {
    use std::io::Read;
    let os = os_string(path).map_err(FsFailure::into_failure)?;
    // open, then read until the end: the same errors at the same steps as `std::fs::read`, but without the size query
    // (`statx`, `lseek`) it makes first. A record file is a few hundred bytes, so the first block takes it whole and the
    // next read sees the end; a larger file grows the buffer.
    let mut file = std::fs::File::open(Path::new(&os))
        .map_err(|error| os_failure(error, path).into_failure())?;
    let mut bytes = vec![0_u8; 4096];
    let mut filled = 0;
    loop {
        if filled == bytes.len() {
            bytes.resize(filled * 2, 0);
        }
        match file.read(&mut bytes[filled..]) {
            Ok(0) => break,
            Ok(count) => filled += count,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => return Err(os_failure(error, path).into_failure()),
        }
    }
    bytes.truncate(filled);
    Ok(bytes)
}

/// The bytes of a file handed over in blocks, for a reader that folds them (a digest) and does not need them all at
/// once: the open, read and error behaviour is that of [`read_bytes`] (`Path.read_bytes()`), without holding the file.
pub fn read_in_blocks(path: &PyStr, mut consume: impl FnMut(&[u8])) -> Result<(), Failure> {
    use std::io::Read;
    let os = os_string(path).map_err(FsFailure::into_failure)?;
    let mut file = std::fs::File::open(Path::new(&os))
        .map_err(|error| os_failure(error, path).into_failure())?;
    let mut block = vec![0_u8; 64 * 1024];
    loop {
        match file.read(&mut block) {
            Ok(0) => return Ok(()),
            Ok(count) => consume(&block[..count]),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => return Err(os_failure(error, path).into_failure()),
        }
    }
}

/// `Path.write_bytes(data)`: create or truncate, then write.
pub fn write_bytes(path: &PyStr, data: &[u8]) -> Result<(), Failure> {
    let os = os_string(path).map_err(FsFailure::into_failure)?;
    std::fs::write(Path::new(&os), data).map_err(|error| os_failure(error, path).into_failure())
}

/// `Path.mkdir()` without parents and without `exist_ok`.
pub fn mkdir_plain(path: &PyStr) -> Result<(), Failure> {
    let os = os_string(path).map_err(FsFailure::into_failure)?;
    std::fs::create_dir(Path::new(&os)).map_err(|error| os_failure(error, path).into_failure())
}

/// `Path.unlink()`.
pub fn unlink(path: &PyStr) -> Result<(), Failure> {
    let os = os_string(path).map_err(FsFailure::into_failure)?;
    std::fs::remove_file(Path::new(&os)).map_err(|error| os_failure(error, path).into_failure())
}
