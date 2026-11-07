//! The file-system and text-file behaviour of Python that the two model scripts rely on: `os.path` joins and tests, `glob.glob`
//! in directory order, `open(...)` for text and bytes, universal newlines, the `OSError` hierarchy by errno, `json.loads(bytes)`
//! with its encoding detection, and `str.splitlines`. Every function returns the exception (class and message) CPython raises.
//! Nothing here decides a rule of the model scripts.

use crate::pyexpr::Res;
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pyjson::{
    decode_file_text, loads_text_objects, loads_text_unique, LoadError, UniqueError,
};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

/// The class of an `OSError` by errno, as Python's `OSError.__new__` maps it.
fn os_error_class(errno: Option<i32>) -> &'static str {
    match errno {
        Some(1 | 13) => "PermissionError",
        Some(2) => "FileNotFoundError",
        Some(3) => "ProcessLookupError",
        Some(4) => "InterruptedError",
        Some(10) => "ChildProcessError",
        Some(11 | 114 | 115) => "BlockingIOError",
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

/// `str(OSError)` for an operating-system failure, with the file name when there is one: `[Errno 2] No such file or
/// directory: 'path'`.
pub fn os_failure(error: &std::io::Error, path: Option<&PyStr>) -> Failure {
    let class = os_error_class(error.raw_os_error());
    let detail = error.to_string();
    let strerror = detail
        .split(" (os error")
        .next()
        .unwrap_or(&detail)
        .to_string();
    let mut message = match error.raw_os_error() {
        Some(errno) => format!("[Errno {errno}] {strerror}"),
        None => strerror,
    };
    if let Some(path) = path {
        message.push_str(": ");
        message.push_str(&path.repr());
    }
    Failure::new(class, &message)
}

/// The bytes Python hands to the operating system for a `str` path (`os.fsencode`: UTF-8 with `surrogateescape`).
pub fn path_bytes(path: &PyStr) -> Res<Vec<u8>> {
    path.to_surrogateescape_bytes()
        .map_err(|error| Failure::new("UnicodeEncodeError", &error.message()))
}

/// The path as the operating system sees it. An embedded NUL is the `ValueError` of the system call wrappers.
fn os_path(path: &PyStr) -> Res<PathBuf> {
    let bytes = path_bytes(path)?;
    if bytes.contains(&0) {
        return Err(Failure::new("ValueError", "embedded null byte"));
    }
    Ok(PathBuf::from(std::ffi::OsStr::from_bytes(&bytes)))
}

/// `os.fspath(value)` for the places that take a path: only `str` is a path here (JSON has no `bytes` or path objects).
pub fn fspath(value: &PyValue, joining: bool) -> Res<&PyStr> {
    match value.peel() {
        PyValue::Str(text) => Ok(text),
        other => Err(Failure::new(
            "TypeError",
            &if joining {
                format!(
                    "join() argument must be str, bytes, or os.PathLike object, not '{}'",
                    other.type_name()
                )
            } else {
                format!(
                    "expected str, bytes or os.PathLike object, not {}",
                    other.type_name()
                )
            },
        )),
    }
}

fn point(c: char) -> u32 {
    u32::from(c)
}

/// `posixpath.join(a, b)`.
pub fn join(first: &PyStr, second: &PyStr) -> PyStr {
    let slash = point('/');
    let mut path = first.clone();
    if second.code_points().first() == Some(&slash) {
        return second.clone();
    }
    if path.is_empty() || path.code_points().last() == Some(&slash) {
        path.push_py(second);
    } else {
        path.push_str("/");
        path.push_py(second);
    }
    path
}

/// `os.path.join(a, b)` for values of any type (the `TypeError` is raised for the first one that is not a path).
pub fn join_values(first: &PyValue, second: &PyValue) -> Res<PyStr> {
    let a = fspath(first, true)?;
    let b = fspath(second, true)?;
    Ok(join(a, b))
}

/// `os.path.basename(path)`: the text after the last `/`.
pub fn basename(path: &PyStr) -> PyStr {
    let points = path.code_points();
    let start = points
        .iter()
        .rposition(|&c| c == point('/'))
        .map_or(0, |at| at + 1);
    PyStr::from_code_points(points[start..].to_vec())
}

/// `os.path.isfile(path)`: follows links, false on any error.
pub fn is_file(path: &PyStr) -> bool {
    os_path(path)
        .ok()
        .and_then(|path| std::fs::metadata(path).ok())
        .is_some_and(|metadata| metadata.is_file())
}

/// `os.path.isdir(path)`.
pub fn is_dir(path: &PyStr) -> bool {
    os_path(path)
        .ok()
        .and_then(|path| std::fs::metadata(path).ok())
        .is_some_and(|metadata| metadata.is_dir())
}

/// `os.path.exists(path)` (follows links).
pub fn exists(path: &PyStr) -> bool {
    os_path(path)
        .ok()
        .is_some_and(|path| std::fs::metadata(path).is_ok())
}

/// `os.path.lexists(path)` (does not follow a final link).
fn lexists(path: &PyStr) -> bool {
    os_path(path)
        .ok()
        .is_some_and(|path| std::fs::symlink_metadata(path).is_ok())
}

/// `os.path.getsize(path)`.
pub fn file_size(path: &PyStr) -> Res<u64> {
    let native = os_path(path)?;
    std::fs::metadata(native)
        .map(|metadata| metadata.len())
        .map_err(|error| os_failure(&error, Some(path)))
}

/// The whole file as bytes (`open(path, "rb").read()`).
pub fn read_bytes(path: &PyStr) -> Res<Vec<u8>> {
    let native = os_path(path)?;
    std::fs::read(native).map_err(|error| os_failure(&error, Some(path)))
}

/// `open(path, "r", encoding="utf-8").read()`: strict UTF-8 and universal newlines.
pub fn read_text(path: &PyStr) -> Res<PyStr> {
    let bytes = read_bytes(path)?;
    decode_file_text(&bytes).map_err(load_failure)
}

/// `open(path, encoding="utf-8", errors="replace").read()`: undecodable bytes become U+FFFD, then universal newlines.
pub fn read_text_replacing(path: &PyStr) -> Res<PyStr> {
    let bytes = read_bytes(path)?;
    let text = String::from_utf8_lossy(&bytes);
    let mut points: Vec<u32> = Vec::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            points.push(point('\n'));
        } else {
            points.push(point(c));
        }
    }
    Ok(PyStr::from_code_points(points))
}

/// The failure of the common JSON reader or text decoder as the Python exception it stands for.
pub fn load_failure(error: LoadError) -> Failure {
    match error {
        LoadError::Decode { .. } => Failure::new("JSONDecodeError", &error.to_string()),
        LoadError::IntegerTooLong { .. } => Failure::new("ValueError", &error.to_string()),
        LoadError::Encoding(message) => Failure::new("UnicodeDecodeError", &message),
        LoadError::TooDeep { container } => Failure::new(
            "RecursionError",
            &format!(
                "maximum recursion depth exceeded while decoding a JSON {container} from a unicode string"
            ),
        ),
    }
}

/// `json.load(open(path, encoding="utf-8"))`.
pub fn load_json_file(path: &PyStr) -> Res<PyValue> {
    let text = read_text(path)?;
    loads_text_objects(&text).map_err(load_failure)
}

/// The exception a repeated key raises in the model scripts' `unique_keys` hook.
pub const DUPLICATE_KEY_PREFIX: &str = "MXFP4_HEADER_LAYOUT_UNSUPPORTED: JSON 중복 키: ";

fn unique_failure(error: UniqueError) -> Failure {
    match error {
        UniqueError::Load(error) => load_failure(error),
        UniqueError::DuplicateKey(key) => {
            let mut message = PyStr::new(DUPLICATE_KEY_PREFIX);
            message.push_py(&key);
            Failure::with_message("ValueError", message)
        }
    }
}

/// `json.load(open(path, encoding="utf-8"), object_pairs_hook=unique_keys)`.
pub fn load_json_file_unique(path: &PyStr) -> Res<PyValue> {
    let text = read_text(path)?;
    loads_text_unique(&text).map_err(unique_failure)
}

/// The text `bytes.decode(encoding, "surrogatepass")` gives, or the `UnicodeDecodeError` message.
fn decode_surrogatepass(bytes: &[u8], encoding: &str) -> Result<PyStr, String> {
    match encoding {
        "utf-8" => decode_utf8_surrogatepass(bytes, 0),
        "utf-8-sig" => decode_utf8_surrogatepass(&bytes[3..], 0),
        "utf-16" | "utf-16-le" | "utf-16-be" => decode_utf16(bytes, encoding),
        _ => decode_utf32(bytes, encoding),
    }
}

fn decode_utf8_surrogatepass(bytes: &[u8], _offset: usize) -> Result<PyStr, String> {
    let mut points: Vec<u32> = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        match std::str::from_utf8(&bytes[at..]) {
            Ok(rest) => {
                points.extend(rest.chars().map(u32::from));
                break;
            }
            Err(error) => {
                let good = at + error.valid_up_to();
                // SAFETY-free: the prefix was validated by `from_utf8`.
                if let Ok(prefix) = std::str::from_utf8(&bytes[at..good]) {
                    points.extend(prefix.chars().map(u32::from));
                }
                let b = &bytes[good..];
                if b.len() >= 3
                    && b[0] == 0xED
                    && (0xA0..=0xBF).contains(&b[1])
                    && (0x80..=0xBF).contains(&b[2])
                {
                    points.push(0xD000 | (u32::from(b[1] & 0x3F) << 6) | u32::from(b[2] & 0x3F));
                    at = good + 3;
                    continue;
                }
                let (length, reason) = match error.error_len() {
                    None => (bytes.len() - good, "unexpected end of data"),
                    Some(length) if (0xC2..=0xF4).contains(&bytes[good]) => {
                        (length, "invalid continuation byte")
                    }
                    Some(length) => (length, "invalid start byte"),
                };
                let subject = if length == 1 {
                    format!("byte 0x{:02x} in position {good}", bytes[good])
                } else {
                    format!("bytes in position {good}-{}", good + length - 1)
                };
                return Err(format!("'utf-8' codec can't decode {subject}: {reason}"));
            }
        }
    }
    Ok(PyStr::from_code_points(points))
}

fn decode_utf16(bytes: &[u8], encoding: &str) -> Result<PyStr, String> {
    // With a byte-order mark the codec picks the order from it (and names itself `utf-16-le` / `utf-16-be`).
    let (little, data) = match encoding {
        "utf-16-le" => (true, bytes),
        "utf-16-be" => (false, bytes),
        _ => (bytes.starts_with(&[0xFF, 0xFE]), &bytes[2..]),
    };
    let name = if little { "utf-16-le" } else { "utf-16-be" };
    let mut points = Vec::with_capacity(data.len() / 2);
    let units: Vec<u32> = data
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            if little {
                u32::from(pair[0]) | (u32::from(pair[1]) << 8)
            } else {
                u32::from(pair[1]) | (u32::from(pair[0]) << 8)
            }
        })
        .collect();
    let mut index = 0;
    while index < units.len() {
        let unit = units[index];
        if (0xD800..0xDC00).contains(&unit)
            && index + 1 < units.len()
            && (0xDC00..0xE000).contains(&units[index + 1])
        {
            points.push(0x10000 + ((unit - 0xD800) << 10) + (units[index + 1] - 0xDC00));
            index += 2;
        } else {
            points.push(unit);
            index += 1;
        }
    }
    if !data.len().is_multiple_of(2) {
        let position = bytes.len() - 1;
        return Err(format!(
            "'{name}' codec can't decode byte 0x{:02x} in position {position}: truncated data",
            bytes[position]
        ));
    }
    Ok(PyStr::from_code_points(points))
}

fn decode_utf32(bytes: &[u8], encoding: &str) -> Result<PyStr, String> {
    let (little, skip) = match encoding {
        "utf-32-le" => (true, 0),
        "utf-32-be" => (false, 0),
        _ => (bytes.starts_with(&[0xFF, 0xFE, 0x00, 0x00]), 4),
    };
    let name = if little { "utf-32-le" } else { "utf-32-be" };
    let data = &bytes[skip..];
    let mut points = Vec::with_capacity(data.len() / 4);
    for (index, quad) in data.as_chunks::<4>().0.iter().enumerate() {
        let value = if little {
            u32::from_le_bytes(*quad)
        } else {
            u32::from_be_bytes(*quad)
        };
        if value > 0x10FFFF {
            let start = skip + index * 4;
            return Err(format!(
                "'{name}' codec can't decode bytes in position {start}-{}: code point not in range(0x110000)",
                start + 3
            ));
        }
        points.push(value);
    }
    if !data.len().is_multiple_of(4) {
        let start = skip + data.len() / 4 * 4;
        let end = bytes.len() - 1;
        let subject = if start == end {
            format!("byte 0x{:02x} in position {start}", bytes[start])
        } else {
            format!("bytes in position {start}-{end}")
        };
        return Err(format!(
            "'{name}' codec can't decode {subject}: truncated data"
        ));
    }
    Ok(PyStr::from_code_points(points))
}

/// `json.detect_encoding(bytes)`.
fn detect_encoding(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(&[0xFF, 0xFE, 0x00, 0x00]) || bytes.starts_with(&[0x00, 0x00, 0xFE, 0xFF])
    {
        return "utf-32";
    }
    if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        return "utf-16";
    }
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return "utf-8-sig";
    }
    if bytes.len() >= 4 {
        if bytes[0] == 0 {
            return if bytes[1] != 0 {
                "utf-16-be"
            } else {
                "utf-32-be"
            };
        }
        if bytes[1] == 0 {
            return if bytes[2] != 0 || bytes[3] != 0 {
                "utf-16-le"
            } else {
                "utf-32-le"
            };
        }
    } else if bytes.len() == 2 {
        if bytes[0] == 0 {
            return "utf-16-be";
        }
        if bytes[1] == 0 {
            return "utf-16-le";
        }
    }
    "utf-8"
}

/// `json.loads(bytes)` (encoding detected, decoded with `surrogatepass`).
pub fn loads_bytes(bytes: &[u8]) -> Res<PyValue> {
    let text = decode_surrogatepass(bytes, detect_encoding(bytes))
        .map_err(|message| Failure::new("UnicodeDecodeError", &message))?;
    loads_text_objects(&text).map_err(load_failure)
}

/// `json.loads(bytes, object_pairs_hook=unique_keys)`.
pub fn loads_bytes_unique(bytes: &[u8]) -> Res<PyValue> {
    let text = decode_surrogatepass(bytes, detect_encoding(bytes))
        .map_err(|message| Failure::new("UnicodeDecodeError", &message))?;
    loads_text_unique(&text).map_err(unique_failure)
}

/// `bytes.decode("utf-8")`: strict, no `surrogatepass`.
pub fn decode_utf8(bytes: &[u8]) -> Res<PyStr> {
    agent_runtime_support::pyjson::decode_utf8_strict(bytes).map_err(load_failure)
}

/// `json.loads(text)`.
pub fn loads_str(text: &PyStr) -> Res<PyValue> {
    loads_text_objects(text).map_err(load_failure)
}

/// The size of the file behind an open handle (`os.fstat(f.fileno()).st_size`).
pub fn handle_size(file: &std::fs::File) -> Res<u64> {
    file.metadata()
        .map(|metadata| metadata.len())
        .map_err(|error| os_failure(&error, None))
}

/// `open(path, "rb")`.
pub fn open_binary(path: &PyStr) -> Res<std::fs::File> {
    let native = os_path(path)?;
    std::fs::File::open(native).map_err(|error| os_failure(&error, Some(path)))
}

/// `f.read(count)`: up to `count` bytes (fewer at the end of the file).
pub fn read_up_to(file: &mut std::fs::File, count: u64) -> Res<Vec<u8>> {
    let mut bytes = Vec::new();
    file.take(count)
        .read_to_end(&mut bytes)
        .map_err(|error| os_failure(&error, None))?;
    Ok(bytes)
}

// ── private temporary directories ───────────────────────────────────────────────────────────────

/// A directory this process created for itself (`tempfile.TemporaryDirectory(prefix=...)`): created exclusively with mode 0700 under
/// a name that did not exist (a name that does exist, as a directory, file or link, is never touched: another is tried), and
/// removed when dropped, together with what was written into it and nothing else.
pub struct PrivateDirectory {
    path: PathBuf,
}

impl PrivateDirectory {
    /// A new directory under the temporary-file directory, `prefix` and eight random characters.
    pub fn create(prefix: &str) -> std::io::Result<PrivateDirectory> {
        use std::hash::{BuildHasher, Hasher};
        let random = std::collections::hash_map::RandomState::new();
        let names = std::iter::from_fn(move || {
            let mut bits = random.build_hasher().finish();
            let alphabet = b"abcdefghijklmnopqrstuvwxyz0123456789_";
            let mut text = String::new();
            for _ in 0..8 {
                text.push(char::from(
                    alphabet[(bits % alphabet.len() as u64) as usize],
                ));
                bits /= alphabet.len() as u64;
            }
            Some(text)
        });
        PrivateDirectory::create_with(&std::env::temp_dir(), prefix, names.take(10_000))
    }

    /// The same under `parent`, trying the given name suffixes in order until one can be created.
    pub fn create_with(
        parent: &Path,
        prefix: &str,
        suffixes: impl IntoIterator<Item = String>,
    ) -> std::io::Result<PrivateDirectory> {
        use std::os::unix::fs::DirBuilderExt;
        for suffix in suffixes {
            let path = parent.join(format!("{prefix}{suffix}"));
            match std::fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => return Ok(PrivateDirectory { path }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "No usable temporary directory name found",
        ))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for PrivateDirectory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.path) {
            eprintln!("could not remove {}: {error}", self.path.display());
        }
    }
}

// ── glob ─────────────────────────────────────────────────────────────────────────────────────────

fn has_magic(text: &PyStr) -> bool {
    text.code_points()
        .iter()
        .any(|&c| c == point('*') || c == point('?') || c == point('['))
}

/// `os.path.split(path)` for the head/tail `glob` uses.
fn split(path: &PyStr) -> (PyStr, PyStr) {
    let points = path.code_points();
    let slash = point('/');
    let cut = points
        .iter()
        .rposition(|&c| c == slash)
        .map_or(0, |at| at + 1);
    let (mut head, tail) = (points[..cut].to_vec(), points[cut..].to_vec());
    if !head.is_empty() && head.iter().any(|&c| c != slash) {
        while head.last() == Some(&slash) {
            head.pop();
        }
    }
    (PyStr::from_code_points(head), PyStr::from_code_points(tail))
}

/// One element of a translated `fnmatch` pattern (`fnmatch.translate`).
enum Token {
    /// `*`
    Star,
    /// `?` (any character, a line break included)
    Any,
    Literal(u32),
    /// `[...]`: the ranges of the set and whether it is negated
    Set {
        negate: bool,
        ranges: Vec<(u32, u32)>,
    },
    /// `[]`-like set that can never match (`(?!)`)
    Never,
}

/// The characters and ranges of a regular-expression set body as `fnmatch.translate` writes it: `\x` is the character `x`, a plain
/// `-` between two items is a range.
fn parse_set(body: &[u32]) -> Vec<(u32, u32)> {
    let mut ranges = Vec::new();
    let mut at = 0;
    let item = |at: &mut usize| -> Option<(u32, bool)> {
        let &first = body.get(*at)?;
        if first == point('\\') && *at + 1 < body.len() {
            *at += 2;
            Some((body[*at - 1], true))
        } else {
            *at += 1;
            Some((first, false))
        }
    };
    while let Some((low, _)) = item(&mut at) {
        if body.get(at) == Some(&point('-')) && at + 1 < body.len() {
            at += 1;
            if let Some((high, _)) = item(&mut at) {
                ranges.push((low, high));
                continue;
            }
        }
        ranges.push((low, low));
    }
    ranges
}

/// `fnmatch.translate(pattern)` as tokens (the character-set logic is a transcription of the standard library's: the
/// treatment of `]` first in a set, `!` negation, ranges split at `-`, empty ranges removed, a lone `[` as itself).
fn translate(pattern: &[u32]) -> Vec<Token> {
    let n = pattern.len();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < n {
        let c = pattern[i];
        i += 1;
        if c == point('*') {
            if !matches!(tokens.last(), Some(Token::Star)) {
                tokens.push(Token::Star);
            }
        } else if c == point('?') {
            tokens.push(Token::Any);
        } else if c == point('[') {
            let mut j = i;
            if j < n && pattern[j] == point('!') {
                j += 1;
            }
            if j < n && pattern[j] == point(']') {
                j += 1;
            }
            while j < n && pattern[j] != point(']') {
                j += 1;
            }
            if j >= n {
                tokens.push(Token::Literal(point('[')));
                continue;
            }
            let escape = |text: &[u32]| -> Vec<u32> {
                let mut out = Vec::new();
                for &c in text {
                    if c == point('\\') || c == point('-') {
                        out.push(point('\\'));
                    }
                    out.push(c);
                }
                out
            };
            let mut stuff: Vec<u32> = if !pattern[i..j].contains(&point('-')) {
                let mut out = Vec::new();
                for &c in &pattern[i..j] {
                    if c == point('\\') {
                        out.push(point('\\'));
                    }
                    out.push(c);
                }
                out
            } else {
                let mut chunks: Vec<Vec<u32>> = Vec::new();
                let mut start = i;
                let mut k = if pattern[i] == point('!') {
                    i + 2
                } else {
                    i + 1
                };
                while let Some(found) = pattern[k.min(j)..j].iter().position(|&c| c == point('-')) {
                    let hyphen = k.min(j) + found;
                    chunks.push(pattern[start..hyphen].to_vec());
                    start = hyphen + 1;
                    k = hyphen + 3;
                }
                let chunk = pattern[start..j].to_vec();
                if !chunk.is_empty() {
                    chunks.push(chunk);
                } else if let Some(last) = chunks.last_mut() {
                    last.push(point('-'));
                }
                // Remove empty ranges: they are invalid in a regular expression.
                let mut index = chunks.len();
                while index > 1 {
                    index -= 1;
                    let (before, after) = (
                        chunks[index - 1].last().copied(),
                        chunks[index].first().copied(),
                    );
                    if let (Some(before), Some(after)) = (before, after) {
                        if before > after {
                            let mut merged = chunks[index - 1].clone();
                            merged.pop();
                            merged.extend_from_slice(&chunks[index][1..]);
                            chunks[index - 1] = merged;
                            chunks.remove(index);
                        }
                    }
                }
                let mut out = Vec::new();
                for (position, chunk) in chunks.iter().enumerate() {
                    if position > 0 {
                        out.push(point('-'));
                    }
                    out.extend(escape(chunk));
                }
                out
            };
            // Escape set operations (`&&`, `~~` and `||`).
            let mut escaped = Vec::new();
            for &c in &stuff {
                if c == point('&') || c == point('~') || c == point('|') {
                    escaped.push(point('\\'));
                }
                escaped.push(c);
            }
            stuff = escaped;
            i = j + 1;
            if stuff.is_empty() {
                tokens.push(Token::Never);
            } else if stuff == [point('!')] {
                tokens.push(Token::Any);
            } else {
                let negate = stuff[0] == point('!');
                if negate {
                    stuff.remove(0);
                } else if stuff[0] == point('^') || stuff[0] == point('[') {
                    stuff.insert(0, point('\\'));
                }
                tokens.push(Token::Set {
                    negate,
                    ranges: parse_set(&stuff),
                });
            }
        } else {
            tokens.push(Token::Literal(c));
        }
    }
    tokens
}

/// `fnmatch.fnmatchcase(name, pattern)` (what `glob` applies on POSIX).
pub fn fnmatch(name: &[u32], pattern: &[u32]) -> bool {
    let tokens = translate(pattern);
    // reachable[t]: the first `t` tokens can match the characters consumed so far.
    let mut reachable = vec![false; tokens.len() + 1];
    reachable[0] = true;
    let close_stars = |reachable: &mut Vec<bool>| {
        for t in 0..tokens.len() {
            if matches!(tokens[t], Token::Star) && reachable[t] {
                reachable[t + 1] = true;
            }
        }
    };
    close_stars(&mut reachable);
    for &c in name {
        let mut next = vec![false; tokens.len() + 1];
        for t in 0..tokens.len() {
            if !reachable[t] {
                continue;
            }
            let hit = match &tokens[t] {
                Token::Star => {
                    next[t] = true;
                    false
                }
                Token::Any => true,
                Token::Literal(expected) => *expected == c,
                Token::Set { negate, ranges } => {
                    ranges.iter().any(|&(low, high)| low <= c && c <= high) != *negate
                }
                Token::Never => false,
            };
            if hit {
                next[t + 1] = true;
            }
        }
        close_stars(&mut next);
        reachable = next;
    }
    reachable[tokens.len()]
}

/// The entry names of a directory in the order the operating system lists them (`os.scandir`), as text; an unreadable
/// directory is empty, as `glob` treats it.
fn list_directory(directory: &PyStr) -> Vec<PyStr> {
    let path = if directory.is_empty() {
        PathBuf::from(".")
    } else if let Ok(path) = os_path(directory) {
        path
    } else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(path) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| PyStr::from_surrogateescape(entry.file_name().as_bytes()))
        .collect()
}

fn is_hidden(name: &PyStr) -> bool {
    name.code_points().first() == Some(&point('.'))
}

/// `glob.glob(pathname)`: the matches in directory order (what `os.scandir` lists), joined to the directory part.
pub fn glob(pathname: &PyStr) -> Vec<PyStr> {
    if !has_magic(pathname) {
        return if lexists(pathname) {
            vec![pathname.clone()]
        } else {
            Vec::new()
        };
    }
    let (directory, basename) = split(pathname);
    if directory.is_empty() {
        return glob_in(&directory, &basename);
    }
    let directories = if directory != *pathname && has_magic(&directory) {
        glob(&directory)
    } else {
        vec![directory]
    };
    let mut results = Vec::new();
    for directory in directories {
        for name in glob_in(&directory, &basename) {
            results.push(join(&directory, &name));
        }
    }
    results
}

fn glob_in(directory: &PyStr, basename: &PyStr) -> Vec<PyStr> {
    if has_magic(basename) {
        let hidden_allowed = is_hidden(basename);
        list_directory(directory)
            .into_iter()
            .filter(|name| hidden_allowed || !is_hidden(name))
            .filter(|name| fnmatch(name.code_points(), basename.code_points()))
            .collect()
    } else if lexists(&join(directory, basename)) {
        vec![basename.clone()]
    } else {
        Vec::new()
    }
}

// ── str.splitlines ───────────────────────────────────────────────────────────────────────────────

/// `text.splitlines()`: split at `\n`, `\r`, `\r\n`, `\v`, `\f`, `\x1c`-`\x1e`, `\x85`, ` `, ` `.
pub fn splitlines(text: &PyStr) -> Vec<PyStr> {
    let points = text.code_points();
    let mut lines = Vec::new();
    let mut start = 0;
    let mut index = 0;
    let is_break = |c: u32| {
        matches!(
            c,
            0x0A | 0x0B | 0x0C | 0x0D | 0x1C | 0x1D | 0x1E | 0x85 | 0x2028 | 0x2029
        )
    };
    while index < points.len() {
        if is_break(points[index]) {
            lines.push(PyStr::from_code_points(points[start..index].to_vec()));
            if points[index] == 0x0D && points.get(index + 1) == Some(&0x0A) {
                index += 1;
            }
            start = index + 1;
        }
        index += 1;
    }
    if start < points.len() {
        lines.push(PyStr::from_code_points(points[start..].to_vec()));
    }
    lines
}

/// `text.lower()` (the common Unicode 15 mapping).
pub fn lower(text: &PyStr) -> PyStr {
    crate::pyexpr::lower(text)
}
