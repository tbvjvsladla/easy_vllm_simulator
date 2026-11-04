//! The `os.path`, `fnmatch` and `glob` operations the classifier performs, with the observable behaviour of CPython
//! 3.12.3's `posixpath`, `fnmatch.translate` and `glob.glob` (non-recursive, no `root_dir`).
//!
//! Unlike [`crate::pyfs`] (which follows `pathlib`), these keep `os.path` semantics: `normpath` resolves `..`,
//! `abspath` joins the working directory, and `glob.glob` treats `*`, `?` and `[` in a path as patterns, so a
//! repository or node name that contains one is expanded exactly as the original expands it.

use crate::guidellm::Failure;
use crate::pyfs::os_string;
use crate::pystr::PyStr;
use std::path::Path;

const SLASH: u32 = '/' as u32;
const DOT: u32 = '.' as u32;

fn points(text: &PyStr) -> &[u32] {
    text.code_points()
}

fn from_points(points: &[u32]) -> PyStr {
    PyStr::from_code_points(points.to_vec())
}

/// `os.fspath(value)` for the value kinds a JSON document can hold: only a `str` is a path.
pub fn fspath(value: &crate::pyvalue::PyValue) -> Result<PyStr, Failure> {
    match value.peel() {
        crate::pyvalue::PyValue::Str(text) => Ok(text.clone()),
        other => Err(Failure::new(
            "TypeError",
            &format!(
                "expected str, bytes or os.PathLike object, not {}",
                other.type_name()
            ),
        )),
    }
}

/// `posixpath.join(a, *parts)`.
pub fn join(first: &PyStr, rest: &[&PyStr]) -> PyStr {
    let mut path: Vec<u32> = points(first).to_vec();
    for part in rest {
        let part = points(part);
        if part.first() == Some(&SLASH) {
            path = part.to_vec();
        } else if path.is_empty() || path.last() == Some(&SLASH) {
            path.extend_from_slice(part);
        } else {
            path.push(SLASH);
            path.extend_from_slice(part);
        }
    }
    PyStr::from_code_points(path)
}

/// `posixpath.join` of plain text parts.
pub fn join_all(first: &PyStr, rest: &[&str]) -> PyStr {
    let owned: Vec<PyStr> = rest.iter().map(|part| PyStr::new(part)).collect();
    let refs: Vec<&PyStr> = owned.iter().collect();
    join(first, &refs)
}

/// `posixpath.split(path)` -> `(head, tail)`.
pub fn split(path: &PyStr) -> (PyStr, PyStr) {
    let all = points(path);
    let at = all
        .iter()
        .rposition(|&p| p == SLASH)
        .map_or(0, |index| index + 1);
    let (mut head, tail) = (&all[..at], &all[at..]);
    if !head.is_empty() && !head.iter().all(|&p| p == SLASH) {
        let kept = head.iter().rposition(|&p| p != SLASH).map_or(0, |i| i + 1);
        head = &head[..kept];
    }
    (from_points(head), from_points(tail))
}

/// `posixpath.basename(path)`.
pub fn basename(path: &PyStr) -> PyStr {
    split(path).1
}

/// `posixpath.dirname(path)`.
pub fn dirname(path: &PyStr) -> PyStr {
    split(path).0
}

/// `posixpath.normpath(path)`.
pub fn normpath(path: &PyStr) -> PyStr {
    let all = points(path);
    if all.is_empty() {
        return PyStr::new(".");
    }
    let mut initial_slashes = usize::from(all[0] == SLASH);
    if initial_slashes == 1
        && all.len() >= 2
        && all[1] == SLASH
        && !(all.len() >= 3 && all[2] == SLASH)
    {
        initial_slashes = 2;
    }
    let dotdot = [DOT, DOT];
    let mut kept: Vec<&[u32]> = Vec::new();
    for component in all.split(|&p| p == SLASH) {
        if component.is_empty() || component == [DOT] {
            continue;
        }
        let keep = component != dotdot
            || (initial_slashes == 0 && kept.is_empty())
            || kept.last().is_some_and(|last| *last == dotdot);
        if keep {
            kept.push(component);
        } else if !kept.is_empty() {
            kept.pop();
        }
    }
    let mut out: Vec<u32> = vec![SLASH; initial_slashes];
    for (index, component) in kept.iter().enumerate() {
        if index > 0 {
            out.push(SLASH);
        }
        out.extend_from_slice(component);
    }
    if out.is_empty() {
        PyStr::new(".")
    } else {
        PyStr::from_code_points(out)
    }
}

/// `os.getcwd()` as text.
fn current_directory() -> Result<PyStr, Failure> {
    use std::os::unix::ffi::OsStrExt;
    let directory = std::env::current_dir().map_err(|error| {
        Failure::new(
            crate::pyfs::os_error_class(&error),
            &crate::cli::os_error_text(&error, &PyStr::new("")).replacen(": ''", "", 1),
        )
    })?;
    Ok(PyStr::from_surrogateescape(
        directory.as_os_str().as_bytes(),
    ))
}

/// `posixpath.abspath(path)`.
pub fn abspath(path: &PyStr) -> Result<PyStr, Failure> {
    let absolute = if points(path).first() == Some(&SLASH) {
        path.clone()
    } else {
        join(&current_directory()?, &[path])
    };
    Ok(normpath(&absolute))
}

/// `posixpath.relpath(path, start)` for text arguments.
pub fn relpath(path: &PyStr, start: &PyStr) -> Result<PyStr, Failure> {
    if path.is_empty() {
        return Err(Failure::new("ValueError", "no path specified"));
    }
    let start_abs = abspath(start)?;
    let path_abs = abspath(path)?;
    let pieces = |text: &PyStr| -> Vec<Vec<u32>> {
        points(text)
            .split(|&p| p == SLASH)
            .filter(|piece| !piece.is_empty())
            .map(<[u32]>::to_vec)
            .collect()
    };
    let (start_list, path_list) = (pieces(&start_abs), pieces(&path_abs));
    let common = start_list
        .iter()
        .zip(path_list.iter())
        .take_while(|(a, b)| a == b)
        .count();
    let mut parts: Vec<Vec<u32>> = vec![vec![DOT, DOT]; start_list.len() - common];
    parts.extend(path_list[common..].iter().cloned());
    if parts.is_empty() {
        return Ok(PyStr::new("."));
    }
    let mut out: Vec<u32> = Vec::new();
    for (index, part) in parts.iter().enumerate() {
        if index > 0 {
            out.push(SLASH);
        }
        out.extend_from_slice(part);
    }
    Ok(PyStr::from_code_points(out))
}

fn stat_of(path: &PyStr, follow: bool) -> Option<std::fs::Metadata> {
    let os = os_string(path).ok()?;
    let target = Path::new(&os);
    if follow {
        std::fs::metadata(target).ok()
    } else {
        std::fs::symlink_metadata(target).ok()
    }
}

/// `os.path.isfile(path)`: any failure to stat is `False`.
pub fn isfile(path: &PyStr) -> bool {
    stat_of(path, true).is_some_and(|data| data.is_file())
}

/// `os.path.isdir(path)`.
pub fn isdir(path: &PyStr) -> bool {
    stat_of(path, true).is_some_and(|data| data.is_dir())
}

/// `os.path.lexists(path)`.
pub fn lexists(path: &PyStr) -> bool {
    stat_of(path, false).is_some()
}

// ── fnmatch ─────────────────────────────────────────────────────────────────────

/// One element of a translated `fnmatch` pattern.
#[derive(Clone, Debug)]
enum Token {
    Star,
    AnyOne,
    Literal(u32),
    /// A bracket expression: members and ranges, possibly negated; an empty one never matches.
    Class {
        negated: bool,
        singles: Vec<u32>,
        ranges: Vec<(u32, u32)>,
        never: bool,
    },
}

/// `fnmatch.translate(pattern)` as tokens (CPython 3.12.3, including its range-chunk rules).
fn translate(pattern: &[u32]) -> Vec<Token> {
    let n = pattern.len();
    let mut out: Vec<Token> = Vec::new();
    let mut i = 0;
    while i < n {
        let c = pattern[i];
        i += 1;
        match char::from_u32(c) {
            Some('*') => {
                if !matches!(out.last(), Some(Token::Star)) {
                    out.push(Token::Star);
                }
            }
            Some('?') => out.push(Token::AnyOne),
            Some('[') => {
                let mut j = i;
                if j < n && pattern[j] == '!' as u32 {
                    j += 1;
                }
                if j < n && pattern[j] == ']' as u32 {
                    j += 1;
                }
                while j < n && pattern[j] != ']' as u32 {
                    j += 1;
                }
                if j >= n {
                    out.push(Token::Literal('[' as u32));
                    continue;
                }
                let stuff_start = i;
                let hyphen = '-' as u32;
                let mut chunks: Vec<Vec<u32>> = Vec::new();
                if pattern[stuff_start..j].contains(&hyphen) {
                    let mut start = stuff_start;
                    let mut k = if pattern[start] == '!' as u32 {
                        start + 2
                    } else {
                        start + 1
                    };
                    while let Some(at) = (k..j).find(|&index| pattern[index] == hyphen) {
                        chunks.push(pattern[start..at].to_vec());
                        start = at + 1;
                        k = at + 3;
                    }
                    let chunk = pattern[start..j].to_vec();
                    if chunk.is_empty() {
                        if let Some(last) = chunks.last_mut() {
                            last.push(hyphen);
                        }
                    } else {
                        chunks.push(chunk);
                    }
                    // Remove empty ranges (invalid in a regular expression): a range whose start is above its end
                    // swallows the join.
                    for k in (1..chunks.len()).rev() {
                        let (previous_last, next_first) =
                            (chunks[k - 1].last().copied(), chunks[k].first().copied());
                        if let (Some(a), Some(b)) = (previous_last, next_first) {
                            if a > b {
                                let tail: Vec<u32> = chunks[k][1..].to_vec();
                                let previous = &mut chunks[k - 1];
                                previous.pop();
                                previous.extend(tail);
                                chunks.remove(k);
                            }
                        }
                    }
                } else {
                    chunks.push(pattern[stuff_start..j].to_vec());
                }
                i = j + 1;
                let flat_len: usize =
                    chunks.iter().map(Vec::len).sum::<usize>() + chunks.len().saturating_sub(1);
                let first_is_bang =
                    chunks.first().and_then(|chunk| chunk.first()) == Some(&('!' as u32));
                if flat_len == 0 {
                    out.push(Token::Class {
                        negated: false,
                        singles: Vec::new(),
                        ranges: Vec::new(),
                        never: true,
                    });
                    continue;
                }
                if flat_len == 1 && first_is_bang {
                    out.push(Token::AnyOne);
                    continue;
                }
                let negated = first_is_bang;
                if negated {
                    chunks[0].remove(0);
                }
                let (mut singles, mut ranges) = (Vec::new(), Vec::new());
                let mut pending_start: Option<u32> = None;
                for chunk in &chunks {
                    let mut members = chunk.clone();
                    if let Some(start) = pending_start.take() {
                        if members.is_empty() {
                            singles.push(start);
                        } else {
                            ranges.push((start, members.remove(0)));
                        }
                    }
                    if let Some(last) = members.pop() {
                        pending_start = Some(last);
                    }
                    singles.extend(members);
                }
                singles.extend(pending_start);
                out.push(Token::Class {
                    negated,
                    singles,
                    ranges,
                    never: false,
                });
            }
            _ => out.push(Token::Literal(c)),
        }
    }
    out
}

fn class_matches(
    negated: bool,
    singles: &[u32],
    ranges: &[(u32, u32)],
    never: bool,
    c: u32,
) -> bool {
    if never {
        return false;
    }
    let inside = singles.contains(&c) || ranges.iter().any(|&(lo, hi)| lo <= c && c <= hi);
    inside != negated
}

fn token_matches(token: &Token, c: u32) -> bool {
    match token {
        Token::AnyOne => true,
        Token::Literal(literal) => *literal == c,
        Token::Class {
            negated,
            singles,
            ranges,
            never,
        } => class_matches(*negated, singles, ranges, *never, c),
        Token::Star => false,
    }
}

/// `fnmatch.fnmatchcase(name, pattern)` (the pattern matches the whole name; `*` and `?` also match a newline).
pub fn fnmatchcase(name: &[u32], pattern: &[u32]) -> bool {
    let tokens = translate(pattern);
    let (mut t, mut s) = (0usize, 0usize);
    let mut star: Option<(usize, usize)> = None;
    while s < name.len() {
        if t < tokens.len() && matches!(tokens[t], Token::Star) {
            star = Some((t, s));
            t += 1;
        } else if t < tokens.len() && token_matches(&tokens[t], name[s]) {
            t += 1;
            s += 1;
        } else if let Some((star_t, star_s)) = star {
            t = star_t + 1;
            s = star_s + 1;
            star = Some((star_t, star_s + 1));
        } else {
            return false;
        }
    }
    while t < tokens.len() && matches!(tokens[t], Token::Star) {
        t += 1;
    }
    t == tokens.len()
}

// ── glob ────────────────────────────────────────────────────────────────────────

/// `glob.has_magic(text)`.
fn has_magic(text: &PyStr) -> bool {
    points(text)
        .iter()
        .any(|&p| p == '*' as u32 || p == '?' as u32 || p == '[' as u32)
}

fn is_hidden(name: &[u32]) -> bool {
    name.first() == Some(&DOT)
}

/// `os.scandir(dirname)` names (an `OSError` is an empty listing); `dirs_only` keeps directories.
fn listing(dirname: &PyStr, dirs_only: bool) -> Vec<PyStr> {
    let target = if dirname.is_empty() {
        PyStr::new(".")
    } else {
        dirname.clone()
    };
    let Ok(os) = os_string(&target) else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(Path::new(&os)) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for entry in entries.flatten() {
        use std::os::unix::ffi::OsStrExt;
        if dirs_only && !entry.path().is_dir() {
            continue;
        }
        names.push(PyStr::from_surrogateescape(entry.file_name().as_bytes()));
    }
    names
}

fn glob_in_directory(dirname: &PyStr, pattern: &PyStr, dirs_only: bool) -> Vec<PyStr> {
    if !has_magic(pattern) {
        // `_glob0`
        if pattern.is_empty() {
            return if isdir(dirname) {
                vec![pattern.clone()]
            } else {
                Vec::new()
            };
        }
        let whole = if dirname.is_empty() {
            pattern.clone()
        } else {
            join(dirname, &[pattern])
        };
        return if lexists(&whole) {
            vec![pattern.clone()]
        } else {
            Vec::new()
        };
    }
    // `_glob1`
    let hidden_pattern = is_hidden(points(pattern));
    listing(dirname, dirs_only)
        .into_iter()
        .filter(|name| hidden_pattern || !is_hidden(points(name)))
        .filter(|name| fnmatchcase(points(name), points(pattern)))
        .collect()
}

fn iglob(pathname: &PyStr, dirs_only: bool) -> Vec<PyStr> {
    let (dirname, basename) = split(pathname);
    if !has_magic(pathname) {
        return if !basename.is_empty() {
            if lexists(pathname) {
                vec![pathname.clone()]
            } else {
                Vec::new()
            }
        } else if isdir(&dirname) {
            vec![pathname.clone()]
        } else {
            Vec::new()
        };
    }
    if dirname.is_empty() {
        return glob_in_directory(&PyStr::default(), &basename, dirs_only);
    }
    let directories = if dirname != *pathname && has_magic(&dirname) {
        iglob(&dirname, true)
    } else {
        vec![dirname.clone()]
    };
    let mut found = Vec::new();
    for directory in directories {
        for name in glob_in_directory(&directory, &basename, dirs_only) {
            found.push(join(&directory, &[&name]));
        }
    }
    found
}

/// `glob.glob(pathname)` (not recursive). The order is the operating system's listing order, as the original's;
/// callers that need an order sort the result.
pub fn glob(pathname: &PyStr) -> Vec<PyStr> {
    iglob(pathname, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(value: &str) -> PyStr {
        PyStr::new(value)
    }

    #[test]
    fn normpath_follows_posixpath() {
        for (input, expected) in [
            ("", "."),
            ("a//b/./c/..", "a/b"),
            ("/../a", "/a"),
            ("//a", "//a"),
            ("///a", "/a"),
            ("../../a", "../../a"),
            ("a/../..", ".."),
            ("/", "/"),
        ] {
            assert_eq!(
                normpath(&text(input)).to_utf8().unwrap(),
                expected,
                "{input}"
            );
        }
    }

    #[test]
    fn split_and_join_follow_posixpath() {
        let (head, tail) = split(&text("/a/b//c"));
        assert_eq!(
            (head.to_utf8().unwrap(), tail.to_utf8().unwrap()),
            ("/a/b".into(), "c".into())
        );
        let (head, tail) = split(&text("//x"));
        assert_eq!(
            (head.to_utf8().unwrap(), tail.to_utf8().unwrap()),
            ("//".into(), "x".into())
        );
        assert_eq!(
            join_all(&text("a"), &["b", "/c", "d"]).to_utf8().unwrap(),
            "/c/d"
        );
        assert_eq!(join_all(&text(""), &["b"]).to_utf8().unwrap(), "b");
    }

    #[test]
    fn relpath_counts_parents() {
        let r = |a: &str, b: &str| relpath(&text(a), &text(b)).unwrap().to_utf8().unwrap();
        assert_eq!(r("/a/b/c", "/a/x"), "../b/c");
        assert_eq!(r("/a/b", "/a/b"), ".");
        assert!(relpath(&text(""), &text("/a")).is_err());
    }

    #[test]
    fn fnmatch_translates_brackets() {
        let m = |name: &str, pattern: &str| {
            let name: Vec<u32> = name.chars().map(u32::from).collect();
            let pattern: Vec<u32> = pattern.chars().map(u32::from).collect();
            fnmatchcase(&name, &pattern)
        };
        assert!(m("a.jsonl", "*.jsonl"));
        assert!(!m("a.json", "*.jsonl"));
        assert!(m("b", "[a-c]"));
        assert!(!m("d", "[a-c]"));
        assert!(m("d", "[!a-c]"));
        assert!(m("-", "[a-]"));
        assert!(m("[", "["));
        assert!(m("]", "[]]"));
        assert!(!m("x", "[]"));
        assert!(m("a\nb", "a?b"));
        assert!(m("ab", "a**b"));
    }
}
