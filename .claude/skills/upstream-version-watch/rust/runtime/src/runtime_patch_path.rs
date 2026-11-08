//! The path semantics `validate_runtime_patch.py` gets from CPython 3.12.3's `pathlib` and `posixpath`, and the file-system calls it makes
//! through them. Paths are raw bytes (the interpreter's `surrogateescape` text maps one-to-one onto them; the only separator is `/`).
//!
//! What is modelled, with the line ranges of the pinned standard library it follows:
//! - `PurePosixPath` text (`str(Path(raw))`): the root (`/` or exactly two slashes), empty and `.` components dropped, `..` kept;
//! - `posixpath.join` / `split` / `normpath` / `abspath` and the non-strict `realpath` (`posixpath.py:428-505`, `_joinrealpath`), including the
//!   `seen` cache, the unresolved-loop return and the recursion through link targets;
//! - `Path.resolve()` (`pathlib.py:1230-1255`): the realpath, then a `stat` whose `ELOOP` becomes `RuntimeError("Symlink loop from ...")`;
//! - `Path.is_file/is_dir` (the ignored errors `ENOENT`, `ENOTDIR`, `EBADF`, `ELOOP`), `Path.mkdir(parents=True, exist_ok=True)`
//!   (`pathlib.py:1308-1323`) and `os.path.exists`.
//!
//! The recursion of `_joinrealpath` through link targets is bounded ([`MAX_LINK_DEPTH`]); the interpreter's own limit is a stack depth, so a
//! chain beyond the bound is named ([`PathFailure::TooDeep`]) and not decided.

use agent_runtime_support::pystr::PyStr;
use recipe_runtime::stdio::os_failure;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::io;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::DirBuilderExt;
use std::path::Path;

/// `errno` values `Path.is_file` / `is_dir` treat as "not there": `ENOENT`, `ENOTDIR`, `EBADF`, `ELOOP`.
const IGNORED_ERRNOS: [i32; 4] = [2, 20, 9, 40];
const ENOENT: i32 = 2;
const ELOOP: i32 = 40;
/// How many link targets one `realpath` follows through nested calls before the port stops deciding (the interpreter's bound is its stack).
pub const MAX_LINK_DEPTH: usize = 400;

pub type PathBytes = Vec<u8>;

fn os_path(bytes: &[u8]) -> &Path {
    Path::new(OsStr::from_bytes(bytes))
}

/// An `OSError` with the file names the failing call was given (`[Errno N] reason: 'a' -> 'b'`).
#[derive(Debug)]
pub struct OsFault {
    pub error: io::Error,
    pub files: Vec<PathBytes>,
}

impl OsFault {
    pub fn on(error: io::Error, files: &[&[u8]]) -> OsFault {
        OsFault {
            error,
            files: files.iter().map(|file| file.to_vec()).collect(),
        }
    }

    pub fn errno(&self) -> Option<i32> {
        self.error.raw_os_error()
    }

    /// `str(exc)`.
    pub fn message(&self) -> PyStr {
        let mut text = os_failure(&self.error).message;
        for (index, file) in self.files.iter().enumerate() {
            text.push_str(if index == 0 { ": " } else { " -> " });
            text.push_str(&PyStr::from_surrogateescape(file).repr());
        }
        text
    }
}

/// Why a path operation did not produce a path.
#[derive(Debug)]
pub enum PathFailure {
    /// An `OSError` the caller reports (its `str`).
    Os(OsFault),
    /// `Path.resolve` raised `RuntimeError("Symlink loop from <filename>")`.
    SymlinkLoop { filename: PathBytes },
    /// A chain of link targets deeper than [`MAX_LINK_DEPTH`].
    TooDeep,
}

impl From<OsFault> for PathFailure {
    fn from(fault: OsFault) -> PathFailure {
        PathFailure::Os(fault)
    }
}

fn root_length(raw: &[u8]) -> usize {
    if !raw.starts_with(b"/") {
        0
    } else if raw.starts_with(b"//") && !raw.starts_with(b"///") {
        2
    } else {
        1
    }
}

/// `str(PurePosixPath(raw))`.
pub fn path_text(raw: &[u8]) -> PathBytes {
    let root = root_length(raw);
    let mut text = vec![b'/'; root];
    let parts: Vec<&[u8]> = raw[root..]
        .split(|byte| *byte == b'/')
        .filter(|part| !part.is_empty() && *part != b".")
        .collect();
    text.extend(parts.join(&b'/'));
    if text.is_empty() {
        text.push(b'.');
    }
    text
}

/// `str(Path(base) / name)` for a base already in [`path_text`] form and a single component.
pub fn child_text(base: &[u8], name: &[u8]) -> PathBytes {
    if base == b"." {
        return path_text(name);
    }
    let mut joined = base.to_vec();
    if !joined.ends_with(b"/") {
        joined.push(b'/');
    }
    joined.extend_from_slice(name);
    path_text(&joined)
}

/// `str(Path(text).parent)` for a text in [`path_text`] form.
pub fn parent_text(text: &[u8]) -> PathBytes {
    let root = root_length(text);
    let rest = &text[root..];
    match rest.iter().rposition(|byte| *byte == b'/') {
        Some(at) => [&text[..root], &rest[..at]].concat(),
        None if root > 0 => text[..root].to_vec(),
        None => b".".to_vec(),
    }
}

/// The components of a text in [`path_text`] form, root first when there is one (`PurePath.parts`).
pub fn components(text: &[u8]) -> Vec<PathBytes> {
    let root = root_length(text);
    let mut parts = Vec::new();
    if root > 0 {
        parts.push(vec![b'/'; root]);
    }
    if text != b"." {
        parts.extend(
            text[root..]
                .split(|byte| *byte == b'/')
                .filter(|part| !part.is_empty())
                .map(<[u8]>::to_vec),
        );
    }
    parts
}

/// `Path.name`: the last component (empty for a root and for `.`).
pub fn name_of(text: &[u8]) -> PathBytes {
    match components(text).pop() {
        Some(last) if last.first() != Some(&b'/') => last,
        _ => Vec::new(),
    }
}

/// `PurePath.is_relative_to(base)` for two texts in [`path_text`] form: `base` is `path` or one of its parents.
pub fn is_relative_to(path: &[u8], base: &[u8]) -> bool {
    let (path, base) = (components(path), components(base));
    path.starts_with(&base)
}

/// `posixpath.join(a, b)`.
fn join(first: &[u8], second: &[u8]) -> PathBytes {
    if second.starts_with(b"/") {
        return second.to_vec();
    }
    let mut joined = first.to_vec();
    if !joined.is_empty() && !joined.ends_with(b"/") {
        joined.push(b'/');
    }
    joined.extend_from_slice(second);
    joined
}

/// `posixpath.split(path)`.
fn split(path: &[u8]) -> (PathBytes, PathBytes) {
    let at = path
        .iter()
        .rposition(|byte| *byte == b'/')
        .map_or(0, |at| at + 1);
    let (head, tail) = (&path[..at], &path[at..]);
    let mut head = head.to_vec();
    if !head.is_empty() && head.iter().any(|byte| *byte != b'/') {
        while head.ends_with(b"/") {
            head.pop();
        }
    }
    (head, tail.to_vec())
}

/// `posixpath.normpath(path)`.
fn normpath(path: &[u8]) -> PathBytes {
    if path.is_empty() {
        return b".".to_vec();
    }
    let root = root_length(path);
    let mut kept: Vec<&[u8]> = Vec::new();
    for part in path[root..].split(|byte| *byte == b'/') {
        if part.is_empty() || part == b"." {
            continue;
        }
        let keeps = part != b".."
            || (root == 0 && kept.is_empty())
            || kept.last().is_some_and(|last| *last == b"..");
        if keeps {
            kept.push(part);
        } else {
            kept.pop();
        }
    }
    let mut text = vec![b'/'; root];
    text.extend(kept.join(&b'/'));
    if text.is_empty() {
        text.push(b'.');
    }
    text
}

/// `os.getcwd()`.
fn current_directory() -> Result<PathBytes, OsFault> {
    std::env::current_dir()
        .map(|dir| dir.into_os_string().into_vec())
        .map_err(|error| OsFault::on(error, &[]))
}

/// `posixpath.abspath(path)`.
pub fn abspath(path: &[u8]) -> Result<PathBytes, OsFault> {
    if path.starts_with(b"/") {
        return Ok(normpath(path));
    }
    Ok(normpath(&join(&current_directory()?, path)))
}

type Seen = HashMap<PathBytes, Option<PathBytes>>;

/// `posixpath._joinrealpath(path, rest, strict=False, seen)`: the path and whether every link was resolved.
fn join_realpath(
    mut path: PathBytes,
    rest: &[u8],
    seen: &mut Seen,
    depth: usize,
) -> Result<(PathBytes, bool), PathFailure> {
    if depth > MAX_LINK_DEPTH {
        return Err(PathFailure::TooDeep);
    }
    let mut rest: &[u8] = rest;
    if rest.starts_with(b"/") {
        rest = &rest[1..];
        path = b"/".to_vec();
    }
    while !rest.is_empty() {
        let (name, after) = match rest.iter().position(|byte| *byte == b'/') {
            Some(at) => (&rest[..at], &rest[at + 1..]),
            None => (rest, &rest[rest.len()..]),
        };
        rest = after;
        if name.is_empty() || name == b"." {
            continue;
        }
        if name == b".." {
            if path.is_empty() {
                path = b"..".to_vec();
            } else {
                let (head, tail) = split(&path);
                path = head;
                if tail == b".." {
                    path = join(&join(&path, b".."), b"..");
                }
            }
            continue;
        }
        let newpath = join(&path, name);
        // `os.lstat` failing is "not a link" (the error is ignored in non-strict mode)
        let is_link = std::fs::symlink_metadata(os_path(&newpath))
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false);
        if !is_link {
            path = newpath;
            continue;
        }
        if let Some(known) = seen.get(&newpath) {
            match known {
                Some(resolved) => {
                    path = resolved.clone();
                    continue;
                }
                None => return Ok((join(&newpath, rest), false)),
            }
        }
        seen.insert(newpath.clone(), None);
        let target = std::fs::read_link(os_path(&newpath))
            .map(|target| target.into_os_string().into_vec())
            .map_err(|error| OsFault::on(error, &[&newpath]))?;
        let (resolved, ok) = join_realpath(path, &target, seen, depth + 1)?;
        path = resolved;
        if !ok {
            return Ok((join(&path, rest), false));
        }
        seen.insert(newpath, Some(path.clone()));
    }
    Ok((path, true))
}

/// `posixpath.realpath(filename)` (non-strict).
fn realpath(filename: &[u8]) -> Result<PathBytes, PathFailure> {
    let (path, _) = join_realpath(Vec::new(), filename, &mut Seen::new(), 0)?;
    Ok(abspath(&path)?)
}

/// `Path(text).resolve()` for a text in [`path_text`] form; the result is again in [`path_text`] form.
pub fn resolve(text: &[u8]) -> Result<PathBytes, PathFailure> {
    let resolved = path_text(&realpath(text)?);
    // non-strict `realpath` returns on a loop; the `stat` is what turns it into an exception
    if let Err(error) = std::fs::metadata(os_path(&resolved)) {
        if error.raw_os_error() == Some(ELOOP) {
            return Err(PathFailure::SymlinkLoop { filename: resolved });
        }
    }
    Ok(resolved)
}

fn stat_kind(text: &[u8], wants: fn(&std::fs::Metadata) -> bool) -> Result<bool, OsFault> {
    match std::fs::metadata(os_path(text)) {
        Ok(metadata) => Ok(wants(&metadata)),
        Err(error)
            if error
                .raw_os_error()
                .is_some_and(|errno| IGNORED_ERRNOS.contains(&errno)) =>
        {
            Ok(false)
        }
        Err(error) => Err(OsFault::on(error, &[text])),
    }
}

/// `Path.is_file()`.
pub fn is_file(text: &[u8]) -> Result<bool, OsFault> {
    stat_kind(text, std::fs::Metadata::is_file)
}

/// `Path.is_dir()`.
pub fn is_dir(text: &[u8]) -> Result<bool, OsFault> {
    stat_kind(text, std::fs::Metadata::is_dir)
}

/// `os.path.exists(path)`: any failure to `stat` is "no".
pub fn exists(text: &[u8]) -> bool {
    std::fs::metadata(os_path(text)).is_ok()
}

/// `Path.read_bytes()`.
pub fn read_bytes(text: &[u8]) -> Result<Vec<u8>, OsFault> {
    std::fs::read(os_path(text)).map_err(|error| OsFault::on(error, &[text]))
}

/// `Path(text).mkdir(parents=True, exist_ok=True)`: the recursion through the parent on `FileNotFoundError`, and the `is_dir` test of any other
/// failure (the parent chain is the text's own, `..` components included).
pub fn make_directories(text: &[u8]) -> Result<(), OsFault> {
    make_directory(text, true, true)
}

fn make_directory(text: &[u8], parents: bool, exist_ok: bool) -> Result<(), OsFault> {
    match std::fs::DirBuilder::new().mode(0o777).create(os_path(text)) {
        Ok(()) => Ok(()),
        Err(error) if error.raw_os_error() == Some(ENOENT) => {
            let parent = parent_text(text);
            if !parents || parent == text {
                return Err(OsFault::on(error, &[text]));
            }
            make_directory(&parent, true, true)?;
            make_directory(text, false, exist_ok)
        }
        Err(error) => {
            if !exist_ok || !is_dir(text)? {
                return Err(OsFault::on(error, &[text]));
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_text_drops_empty_and_dot_components_keeps_dotdot_and_two_leading_slashes() {
        assert_eq!(path_text(b"a//b/./c/"), b"a/b/c");
        assert_eq!(path_text(b""), b".");
        assert_eq!(path_text(b"./"), b".");
        assert_eq!(path_text(b"configs/../configs"), b"configs/../configs");
        assert_eq!(path_text(b"//a"), b"//a");
        assert_eq!(path_text(b"///a"), b"/a");
        assert_eq!(path_text(b"/"), b"/");
    }

    #[test]
    fn child_and_parent_follow_the_path_class() {
        assert_eq!(child_text(b".", b"x"), b"x");
        assert_eq!(child_text(b"/", b"x"), b"/x");
        assert_eq!(child_text(b"a/b", b"x"), b"a/b/x");
        assert_eq!(parent_text(b"a/b"), b"a");
        assert_eq!(parent_text(b"a"), b".");
        assert_eq!(parent_text(b"/a"), b"/");
        assert_eq!(parent_text(b"//a"), b"//");
        assert_eq!(parent_text(b"/"), b"/");
        assert_eq!(parent_text(b"."), b".");
        assert_eq!(name_of(b"/a/b.py"), b"b.py");
        assert_eq!(name_of(b"/"), b"");
    }

    #[test]
    fn normpath_collapses_dotdot_lexically_and_keeps_leading_ones_of_a_relative_path() {
        assert_eq!(normpath(b"/a/b/../c"), b"/a/c");
        assert_eq!(normpath(b"../../a"), b"../../a");
        assert_eq!(normpath(b"/.."), b"/");
        assert_eq!(normpath(b"//a/.."), b"//");
        assert_eq!(normpath(b"///a"), b"/a");
        assert_eq!(normpath(b""), b".");
    }

    #[test]
    fn relative_to_compares_components_not_text_prefixes() {
        assert!(is_relative_to(b"/a/bc/d", b"/a/bc"));
        assert!(!is_relative_to(b"/a/bcd", b"/a/bc"));
        assert!(is_relative_to(b"/a", b"/a"));
        assert!(is_relative_to(b"/a", b"/"));
    }

    #[test]
    fn split_keeps_a_root_head() {
        assert_eq!(split(b"/a"), (b"/".to_vec(), b"a".to_vec()));
        assert_eq!(split(b"a/b"), (b"a".to_vec(), b"b".to_vec()));
        assert_eq!(split(b"a"), (Vec::new(), b"a".to_vec()));
        assert_eq!(split(b"/"), (b"/".to_vec(), Vec::new()));
    }
}
