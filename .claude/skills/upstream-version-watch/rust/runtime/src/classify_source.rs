//! The two public functions of `classify_failure.py` (`load_patterns`, `matched_lines`) over the Python-compatible regular-expression engine
//! ([`crate::py_regex_engine`]) and the codec/text pieces of this crate.
//!
//! **`load_patterns(path)`** reads the registry as `open(path, encoding="utf-8")` does (8 KiB chunk decoding, universal newlines, the file's own
//! errno errors, a directory refused at `open`), recognises its three line forms with the source's own three regular expressions (run by the
//! engine, not hand-parsed), and decodes each signature with `.encode().decode("unicode_escape")` ([`crate::unicode_escape`]). The registry file is
//! read from the file system; nothing about its content is embedded here.
//!
//! **`matched_lines(text, pattern, max_lines=5, max_chars=1000)`** keeps the source's order of operations: compile first, then the subject type check,
//! then `finditer`, the `rfind`/`find` line bounds, `strip`, the de-duplication, the `>= max_lines` test after every candidate and the final
//! `[:max_chars]` slice, each with the exceptions CPython raises for the argument types the source does not guard.
//!
//! **Identity.** CPython returns the very caller string in some cases (a full slice, a `strip` that removes nothing, a one-element `join`, a full
//! `[:n]`), and the one shared object for the empty string and for a one-character Latin-1 string. [`Origin`] tracks exactly those rules through the
//! string operations, so the public API can name the caller object a returned string *is*, without comparing values.

use crate::http_direct::{ErrorValue, PyException};
use crate::py_errors::{builtin, os_error_with_filename, with_message};
use crate::py_regex_engine::{compile, CompiledPattern, EngineFailure, PatternSource};
use crate::py_regex_parse::{ParseFailure, ReError};
use crate::textfile::{StreamFailure, TextStream};
use crate::unicode_escape::{decode, EscapeFailure, InvalidEscape};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use num_bigint::BigInt;
use num_traits::{Signed, ToPrimitive};
use std::collections::HashSet;
use std::sync::OnceLock;

/// What a source call ended in other than a value or a Python exception.
#[derive(Debug)]
pub enum SourceFailure {
    Raised(Box<PyException>),
    /// A situation outside the named profile (never an invented Python exception).
    OutOfScope {
        boundary: &'static str,
        detail: String,
    },
}

impl From<Box<PyException>> for SourceFailure {
    fn from(error: Box<PyException>) -> SourceFailure {
        SourceFailure::Raised(error)
    }
}

fn raised(error: PyException) -> SourceFailure {
    SourceFailure::Raised(Box::new(error))
}

fn out_of_scope(boundary: &'static str, detail: impl Into<String>) -> SourceFailure {
    SourceFailure::OutOfScope {
        boundary,
        detail: detail.into(),
    }
}

pub fn type_name(value: &PyValue) -> &'static str {
    value.peel().type_name()
}

fn type_error(message: &str) -> SourceFailure {
    raised(builtin("TypeError", message))
}

/// `re.error` as `re/_constants.py` builds it, attributes in the interpreter's order (`msg`, `pattern`, `pos`, `lineno`, `colno`).
pub fn re_error_exception(error: &ReError) -> PyException {
    let mut message = error.msg.to_utf8().unwrap_or_default();
    let pattern_value = match &error.pattern {
        Some(pattern) if pattern.is_text => {
            PyValue::Str(PyStr::from_code_points(pattern.points.clone()))
        }
        Some(pattern) => PyValue::Bytes(pattern.points.iter().map(|&p| p as u8).collect()),
        None => PyValue::None,
    };
    let (lineno, colno) = match (&error.pattern, error.pos) {
        (Some(pattern), Some(pos)) => {
            let len = pattern.points.len() as i64;
            let end = if pos < 0 {
                (len + pos).max(0)
            } else {
                pos.min(len)
            } as usize;
            let before = &pattern.points[..end];
            let lineno = before.iter().filter(|&&c| c == 10).count() as i64 + 1;
            let last = before
                .iter()
                .rposition(|&c| c == 10)
                .map_or(-1, |index| index as i64);
            let colno = pos - last;
            message = format!("{message} at position {pos}");
            if pattern.points.contains(&10) {
                message = format!("{message} (line {lineno}, column {colno})");
            }
            (PyValue::int(lineno), PyValue::int(colno))
        }
        _ => (PyValue::None, PyValue::None),
    };
    PyException {
        class: "error",
        module: "re",
        message: PyStr::new(&message),
        args: vec![PyValue::str(&message).into()],
        attributes: vec![
            ("msg", PyValue::Str(error.msg.clone()).into()),
            ("pattern", pattern_value.into()),
            ("pos", error.pos.map_or(PyValue::None, PyValue::int).into()),
            ("lineno", lineno.into()),
            ("colno", colno.into()),
        ],
        http_headers: None,
        link: None,
    }
}

fn compile_failure(failure: EngineFailure) -> SourceFailure {
    match failure {
        EngineFailure::Parse(ParseFailure::Re(error)) => raised(re_error_exception(&error)),
        EngineFailure::Parse(ParseFailure::Overflow(message)) => {
            raised(builtin("OverflowError", &message))
        }
        EngineFailure::Parse(ParseFailure::Value(message)) => {
            raised(builtin("ValueError", &message))
        }
        EngineFailure::Parse(ParseFailure::Unsupported { boundary, detail })
        | EngineFailure::Unsupported { boundary, detail } => out_of_scope(boundary, detail),
    }
}

pub fn unicode_decode_exception(
    encoding: &'static str,
    object: &[u8],
    start: usize,
    end: usize,
    reason: &str,
) -> PyException {
    let subject = if end - start == 1 {
        format!("byte 0x{:02x} in position {start}", object[start])
    } else {
        format!("bytes in position {start}-{}", end - 1)
    };
    let message = format!("'{encoding}' codec can't decode {subject}: {reason}");
    PyException {
        class: "UnicodeDecodeError",
        module: "builtins",
        message: PyStr::new(&message),
        args: vec![
            PyValue::str(encoding).into(),
            PyValue::Bytes(object.to_vec()).into(),
            PyValue::int(start).into(),
            PyValue::int(end).into(),
            PyValue::str(reason).into(),
        ],
        attributes: vec![
            ("reason", PyValue::str(reason).into()),
            ("encoding", PyValue::str(encoding).into()),
            ("object", PyValue::Bytes(object.to_vec()).into()),
            ("start", PyValue::int(start).into()),
            ("end", PyValue::int(end).into()),
        ],
        http_headers: None,
        link: None,
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// load_patterns

struct LineForms {
    signature: CompiledPattern,
    class: CompiledPattern,
    note: CompiledPattern,
}

/// The three regular expressions of `load_patterns`, compiled once from the source's own pattern text.
fn line_forms() -> &'static LineForms {
    static FORMS: OnceLock<LineForms> = OnceLock::new();
    FORMS.get_or_init(|| {
        let build = |text: &str| {
            compile(PatternSource::Text(&PyStr::new(text))).unwrap_or_else(|failure| {
                panic!("the source's own pattern {text:?} must compile: {failure:?}")
            })
        };
        LineForms {
            signature: build(r#"\s*-\s*signature:\s*"(.*)"\s*$"#),
            class: build(r"\s*class:\s*(\S+)"),
            note: build(r#"\s*note:\s*"(.*)"\s*$"#),
        }
    })
}

fn group_text(line: &PyStr, spans: &[Option<(usize, usize)>], group: usize) -> PyStr {
    let (start, end) = spans[group].unwrap_or((0, 0));
    PyStr::from_code_points(line.code_points()[start..end].to_vec())
}

/// Result of a registry read.
#[derive(Debug)]
pub struct Loaded {
    pub patterns: Vec<PyDict>,
    /// The first escape the interpreter warns about while decoding a signature (a `DeprecationWarning`; hidden for an imported module, shown for
    /// the script run as `__main__`).
    pub first_invalid_escape: Option<InvalidEscape>,
}

/// The path argument of `open`: a `str` here; the other types are told by their `TypeError` or named as outside the profile.
fn open_path(path: &PyValue) -> Result<PyStr, SourceFailure> {
    match path.peel() {
        PyValue::Str(text) => Ok(text.clone()),
        PyValue::Bool(_) | PyValue::Int(_) => Err(out_of_scope(
            "open_file_descriptor",
            "an integer path is a file descriptor for open(); not modelled",
        )),
        PyValue::Bytes(_) => Err(out_of_scope(
            "open_bytes_path",
            "a bytes path names its file in bytes in the errors; not modelled",
        )),
        other => Err(type_error(&format!(
            "expected str, bytes or os.PathLike object, not {}",
            type_name(other)
        ))),
    }
}

fn open_registry(name: &PyStr) -> Result<std::fs::File, SourceFailure> {
    let bytes = name.to_surrogateescape_bytes().map_err(|_| {
        out_of_scope(
            "path_encoding",
            "the path cannot be encoded for the file system",
        )
    })?;
    if bytes.contains(&0) {
        return Err(raised(builtin("ValueError", "embedded null byte")));
    }
    use std::os::unix::ffi::OsStringExt;
    let path = std::path::PathBuf::from(std::ffi::OsString::from_vec(bytes));
    let file =
        std::fs::File::open(&path).map_err(|error| match os_error_with_filename(&error, name) {
            Some(exception) => raised(exception),
            None => out_of_scope("open_failure", error.to_string()),
        })?;
    let is_directory = file.metadata().map(|m| m.is_dir()).unwrap_or(false);
    if is_directory {
        let error = std::io::Error::from_raw_os_error(21);
        return Err(raised(
            os_error_with_filename(&error, name).expect("EISDIR has an errno"),
        ));
    }
    Ok(file)
}

/// `load_patterns(path)`.
pub fn load_patterns(path: &PyValue) -> Result<Loaded, SourceFailure> {
    let name = open_path(path)?;
    let file = open_registry(&name)?;
    let mut stream = TextStream::from_reader(file);
    let forms = line_forms();
    let mut patterns: Vec<PyDict> = Vec::new();
    let mut current: Option<PyDict> = None;
    let mut first_invalid_escape = None;
    loop {
        let line = match stream.next_line_checked() {
            Ok(Some(line)) => line,
            Ok(None) => break,
            Err(StreamFailure::Decode(failure)) => {
                return Err(raised(unicode_decode_exception(
                    "utf-8",
                    &failure.object,
                    failure.start,
                    failure.end,
                    failure.reason,
                )));
            }
            Err(StreamFailure::Read(error)) => {
                return Err(match os_error_with_filename(&error, &name) {
                    Some(exception) => raised(exception),
                    None => out_of_scope("read_failure", error.to_string()),
                });
            }
        };
        let line = PyStr::new(&line);
        if let Some(spans) = forms
            .signature
            .match_at_start(&line)
            .map_err(compile_failure)?
        {
            if let Some(done) = current.take() {
                patterns.push(done);
            }
            let signature = group_text(&line, &spans, 1);
            let encoded = signature.to_utf8().ok_or_else(|| {
                out_of_scope(
                    "surrogate_signature",
                    "a signature with a lone surrogate cannot be encoded",
                )
            })?;
            let decoded = match decode(encoded.as_bytes()) {
                Ok(decoded) => decoded,
                Err(EscapeFailure::Error { start, end, reason }) => {
                    return Err(raised(unicode_decode_exception(
                        "unicodeescape",
                        encoded.as_bytes(),
                        start,
                        end,
                        reason,
                    )));
                }
                Err(EscapeFailure::NamedCharacter) => {
                    return Err(out_of_scope(
                        "unicode_character_name",
                        "a \\N{...} escape needs the Unicode name database, which this port does not ship",
                    ));
                }
            };
            if first_invalid_escape.is_none() {
                first_invalid_escape = decoded.first_invalid_escape;
            }
            let mut record = PyDict::new();
            record.set("signature", PyValue::Str(decoded.text));
            current = Some(record);
            continue;
        }
        if let Some(record) = current.as_mut() {
            if let Some(spans) = forms.class.match_at_start(&line).map_err(compile_failure)? {
                record.set("class", PyValue::Str(group_text(&line, &spans, 1)));
                continue;
            }
        }
        if let Some(record) = current.as_mut() {
            if let Some(spans) = forms.note.match_at_start(&line).map_err(compile_failure)? {
                record.set("note", PyValue::Str(group_text(&line, &spans, 1)));
            }
        }
    }
    if let Some(done) = current.take() {
        patterns.push(done);
    }
    Ok(Loaded {
        patterns,
        first_invalid_escape,
    })
}

// ---------------------------------------------------------------------------------------------------------------------------------
// matched_lines

/// Which object a returned string is, by CPython's own rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// A new object.
    Fresh,
    /// The caller's `text` object itself.
    Text,
    /// The interpreter's shared object for the empty string or a one-character Latin-1 string.
    Singleton,
}

fn singleton_like(points: &[u32]) -> bool {
    points.is_empty() || (points.len() == 1 && points[0] < 256)
}

#[derive(Debug)]
pub struct MatchedLines {
    pub value: PyValue,
    pub origin: Origin,
}

/// `len(lines) >= max_lines` with the types Python compares.
fn at_least(count: usize, limit: &PyValue) -> Result<bool, SourceFailure> {
    let count_big = BigInt::from(count);
    match limit.peel() {
        PyValue::Int(value) => Ok(count_big >= *value),
        PyValue::Bool(value) => Ok(count >= usize::from(*value)),
        PyValue::Float(value) => Ok((count as f64) >= *value),
        other => Err(type_error(&format!(
            "'>=' not supported between instances of 'int' and '{}'",
            type_name(other)
        ))),
    }
}

/// The stop of `[:max_chars]`.
fn slice_stop(limit: &PyValue, length: usize) -> Result<usize, SourceFailure> {
    let clamp = |value: &BigInt| -> usize {
        if value.is_negative() {
            let back = (-value).to_usize().unwrap_or(usize::MAX);
            length.saturating_sub(back)
        } else {
            value.to_usize().map_or(length, |v| v.min(length))
        }
    };
    match limit.peel() {
        PyValue::None => Ok(length),
        PyValue::Int(value) => Ok(clamp(value)),
        PyValue::Bool(value) => Ok(usize::from(*value).min(length)),
        _ => Err(type_error(
            "slice indices must be integers or None or have an __index__ method",
        )),
    }
}

fn compiled_pattern(pattern: &PyValue) -> Result<CompiledPattern, SourceFailure> {
    match pattern.peel() {
        PyValue::Str(text) => compile(PatternSource::Text(text)).map_err(compile_failure),
        PyValue::Bytes(bytes) => compile(PatternSource::Bytes(bytes)).map_err(compile_failure),
        PyValue::List(_) | PyValue::Dict(_) => Err(type_error(&format!(
            "unhashable type: '{}'",
            type_name(pattern)
        ))),
        PyValue::Tuple(items) => {
            if let Some(inner) = items
                .iter()
                .find(|item| matches!(item.peel(), PyValue::List(_) | PyValue::Dict(_)))
            {
                return Err(type_error(&format!(
                    "unhashable type: '{}'",
                    type_name(inner)
                )));
            }
            Err(type_error(
                "first argument must be string or compiled pattern",
            ))
        }
        _ => Err(type_error(
            "first argument must be string or compiled pattern",
        )),
    }
}

fn find_newline_before(points: &[u32], end: usize) -> Option<usize> {
    points[..end].iter().rposition(|&c| c == 10)
}

fn find_newline_from(points: &[u32], start: usize) -> Option<usize> {
    points[start..]
        .iter()
        .position(|&c| c == 10)
        .map(|i| i + start)
}

/// `matched_lines(text, pattern, max_lines, max_chars)`.
pub fn matched_lines(
    text: &PyValue,
    pattern: &PyValue,
    max_lines: &PyValue,
    max_chars: &PyValue,
) -> Result<MatchedLines, SourceFailure> {
    let compiled = compiled_pattern(pattern)?;
    let subject = match (compiled.is_text, text.peel()) {
        (true, PyValue::Str(subject)) => Some(subject.clone()),
        (false, PyValue::Bytes(_)) => None,
        (true, PyValue::Bytes(_)) => {
            return Err(type_error(
                "cannot use a string pattern on a bytes-like object",
            ));
        }
        (false, PyValue::Str(_)) => {
            return Err(type_error(
                "cannot use a bytes pattern on a string-like object",
            ));
        }
        (_, other) => {
            return Err(type_error(&format!(
                "expected string or bytes-like object, got '{}'",
                type_name(other)
            )));
        }
    };
    let Some(subject) = subject else {
        // bytes pattern over bytes: the first match reaches `text.rfind("\n", ...)`, which a bytes object answers with a TypeError for a str argument
        let PyValue::Bytes(bytes) = text.peel() else {
            unreachable!("checked above")
        };
        let mut matches = compiled.finditer_bytes(bytes);
        if matches.next_span().map_err(compile_failure)?.is_some() {
            return Err(type_error(
                "argument should be integer or bytes-like object, not 'str'",
            ));
        }
        let stop = slice_stop(max_chars, 0)?;
        let _ = stop;
        return Ok(MatchedLines {
            value: PyValue::str(""),
            origin: Origin::Singleton,
        });
    };
    let points = subject.code_points();
    let length = points.len();
    let mut lines: Vec<(PyStr, Origin)> = Vec::new();
    let mut seen: HashSet<Vec<u32>> = HashSet::new();
    let mut matches = compiled.finditer_text(&subject).map_err(compile_failure)?;
    while let Some((match_start, match_end)) = matches.next_span().map_err(compile_failure)? {
        let start = find_newline_before(points, match_start).map_or(0, |i| i + 1);
        let end = find_newline_from(points, match_end).unwrap_or(length);
        let slice = PyStr::from_code_points(points[start..end].to_vec());
        let line = slice.strip();
        let same_object = start == 0 && end == length && line.len() == slice.len();
        if !line.is_empty() && seen.insert(line.code_points().to_vec()) {
            let origin = if same_object {
                Origin::Text
            } else if singleton_like(line.code_points()) {
                Origin::Singleton
            } else {
                Origin::Fresh
            };
            lines.push((line, origin));
        }
        if at_least(lines.len(), max_lines)? {
            break;
        }
    }
    let (joined, joined_origin) = match lines.len() {
        0 => (PyStr::new(""), Origin::Singleton),
        1 => (lines[0].0.clone(), lines[0].1),
        _ => {
            let mut points: Vec<u32> = Vec::new();
            for (index, (line, _)) in lines.iter().enumerate() {
                if index > 0 {
                    points.push(10);
                }
                points.extend_from_slice(line.code_points());
            }
            (PyStr::from_code_points(points), Origin::Fresh)
        }
    };
    let stop = slice_stop(max_chars, joined.len())?;
    let (value, origin) = if stop == joined.len() {
        (joined, joined_origin)
    } else {
        let cut = PyStr::from_code_points(joined.code_points()[..stop].to_vec());
        let origin = if singleton_like(cut.code_points()) {
            Origin::Singleton
        } else {
            Origin::Fresh
        };
        (cut, origin)
    };
    Ok(MatchedLines {
        value: PyValue::Str(value),
        origin,
    })
}

#[allow(dead_code)]
fn unused(_: ErrorValue) -> PyException {
    with_message("Exception", "builtins", "")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(text: &str) -> PyValue {
        PyValue::str(text)
    }
    fn lines(text: &str, pattern: &str) -> MatchedLines {
        matched_lines(&s(text), &s(pattern), &PyValue::int(5), &PyValue::int(1000)).unwrap()
    }
    fn value(result: &MatchedLines) -> String {
        match &result.value {
            PyValue::Str(text) => text.to_utf8().unwrap(),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn evidence_is_the_whole_stripped_line_deduplicated_and_ordered() {
        assert_eq!(
            value(&lines("  prefix FAIL suffix  ", "FAIL")),
            "prefix FAIL suffix"
        );
        assert_eq!(
            value(&lines("A FAIL\nA FAIL\nB FAIL\n", "FAIL")),
            "A FAIL\nB FAIL"
        );
        assert_eq!(
            value(&lines("pre alpha\nbeta post\n", "alpha\\nbeta")),
            "pre alpha\nbeta post"
        );
        assert_eq!(value(&lines("alpha\nbeta", "")), "alpha\nbeta");
        assert_eq!(value(&lines("a\nb", "a.b")), "");
    }

    #[test]
    fn identity_follows_the_cpython_object_rules() {
        assert_eq!(lines("ababa", "(?=aba)").origin, Origin::Text);
        assert_eq!(lines("prefixAB tail", "(?<=prefix)AB").origin, Origin::Text);
        assert_eq!(
            lines("  prefix FAIL suffix  ", "FAIL").origin,
            Origin::Fresh
        );
        assert_eq!(lines("A FAIL\nB FAIL", "FAIL").origin, Origin::Fresh);
        assert_eq!(lines("a\nb", "a.b").origin, Origin::Singleton);
        let cut = matched_lines(&s("한글x"), &s("x"), &PyValue::int(5), &PyValue::int(-1)).unwrap();
        assert_eq!(value(&cut), "한글");
        assert_eq!(cut.origin, Origin::Fresh);
    }

    #[test]
    fn limits_follow_the_source_order_and_the_python_types() {
        let two = |max_lines: PyValue, max_chars: PyValue| {
            matched_lines(&s("x\ny"), &s("x|y"), &max_lines, &max_chars)
        };
        assert_eq!(
            value(&two(PyValue::int(0), PyValue::int(1000)).unwrap()),
            "x"
        );
        assert_eq!(
            value(&two(PyValue::int(-1), PyValue::int(1000)).unwrap()),
            "x"
        );
        assert_eq!(value(&two(PyValue::int(5), PyValue::int(0)).unwrap()), "");
        match two(PyValue::Bool(true), PyValue::Float(1.0)) {
            Err(SourceFailure::Raised(error)) => assert_eq!(
                error.message.to_utf8().unwrap(),
                "slice indices must be integers or None or have an __index__ method"
            ),
            other => panic!("{other:?}"),
        }
        match two(PyValue::str("2"), PyValue::int(5)) {
            Err(SourceFailure::Raised(error)) => assert_eq!(
                error.message.to_utf8().unwrap(),
                "'>=' not supported between instances of 'int' and 'str'"
            ),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn type_errors_and_regex_errors_use_the_interpreters_text_and_attributes() {
        let message = |text: PyValue, pattern: PyValue| match matched_lines(
            &text,
            &pattern,
            &PyValue::int(5),
            &PyValue::int(1000),
        ) {
            Err(SourceFailure::Raised(error)) => error.message.to_utf8().unwrap(),
            other => panic!("{other:?}"),
        };
        assert_eq!(
            message(PyValue::None, s("x")),
            "expected string or bytes-like object, got 'NoneType'"
        );
        assert_eq!(
            message(s("text"), PyValue::Bytes(b"x".to_vec())),
            "cannot use a bytes pattern on a string-like object"
        );
        assert_eq!(
            message(
                PyValue::Bytes(b"x\n".to_vec()),
                PyValue::Bytes(b"x".to_vec())
            ),
            "argument should be integer or bytes-like object, not 'str'"
        );
        assert_eq!(
            message(s("text"), s("[")),
            "unterminated character set at position 0"
        );
        match matched_lines(&s("text"), &s("["), &PyValue::int(5), &PyValue::int(1000)) {
            Err(SourceFailure::Raised(error)) => {
                assert_eq!((error.class, error.module), ("error", "re"));
                let names: Vec<&str> = error.attributes.iter().map(|(n, _)| *n).collect();
                assert_eq!(names, ["msg", "pattern", "pos", "lineno", "colno"]);
            }
            other => panic!("{other:?}"),
        }
    }

    fn registry(content: &[u8]) -> (std::path::PathBuf, PyValue) {
        let dir = std::env::temp_dir().join(format!(
            "classify-source-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("patterns.yaml");
        std::fs::write(&path, content).unwrap();
        let name = PyValue::str(&path.to_string_lossy());
        (dir, name)
    }

    fn dict_texts(loaded: &Loaded) -> Vec<Vec<(String, String)>> {
        loaded
            .patterns
            .iter()
            .map(|record| {
                record
                    .iter()
                    .map(|(k, v)| match v {
                        PyValue::Str(t) => (
                            k.to_utf8().unwrap(),
                            t.code_points()
                                .iter()
                                .map(|&c| char::from_u32(c).unwrap_or('?'))
                                .collect(),
                        ),
                        other => panic!("{other:?}"),
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn registry_blocks_follow_the_three_line_forms_and_last_assignment_wins() {
        let (dir, path) = registry(b"- signature: \"A\"\nclass: first\nclass: second\nnote: \"one\"\nnote: \"two\"\n- signature: \"B\"\nclass: x trailing\nnote: \"left \\\"inner\\\" right\"\n");
        let loaded = load_patterns(&path).unwrap();
        let rows = dict_texts(&loaded);
        assert_eq!(
            rows[0],
            vec![
                ("signature".to_string(), "A".to_string()),
                ("class".into(), "second".into()),
                ("note".into(), "two".into())
            ]
        );
        assert_eq!(rows[1][1], ("class".to_string(), "x".to_string()));
        assert_eq!(rows[1][2].1, "left \\\"inner\\\" right");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn orphans_bom_and_escapes_are_decided_like_the_source() {
        let (dir, path) = registry(
            b"class: ignored\nnote: \"ignored\"\n- signature: \"A\"\nclass: requirements-fixable\n",
        );
        assert_eq!(load_patterns(&path).unwrap().patterns.len(), 1);
        let (dir2, path2) = registry(b"\xef\xbb\xbf- signature: \"A\"\nclass: arbitrary\n");
        assert_eq!(load_patterns(&path2).unwrap().patterns.len(), 0);
        let (dir3, path3) = registry(b"- signature: \"\\u0041\\nB\"\nclass: arbitrary\n");
        assert_eq!(dict_texts(&load_patterns(&path3).unwrap())[0][0].1, "A\nB");
        let (dir4, path4) = registry(b"- signature: \"\\x0\"\nclass: arbitrary\n");
        match load_patterns(&path4) {
            Err(SourceFailure::Raised(error)) => assert_eq!(
                error.message.to_utf8().unwrap(),
                "'unicodeescape' codec can't decode bytes in position 0-2: truncated \\xXX escape"
            ),
            other => panic!("{other:?}"),
        }
        let (dir5, path5) = registry(b"- signature: \"A\"\nclass: arbitrary\n\xff");
        match load_patterns(&path5) {
            Err(SourceFailure::Raised(error)) => assert_eq!(error.class, "UnicodeDecodeError"),
            other => panic!("{other:?}"),
        }
        for dir in [dir, dir2, dir3, dir4, dir5] {
            std::fs::remove_dir_all(dir).unwrap();
        }
    }

    #[test]
    fn open_failures_carry_the_files_errno_and_the_type_error_of_a_bad_path() {
        match load_patterns(&s("/definitely/not/here.yaml")) {
            Err(SourceFailure::Raised(error)) => assert_eq!(error.class, "FileNotFoundError"),
            other => panic!("{other:?}"),
        }
        match load_patterns(&PyValue::str(&std::env::temp_dir().to_string_lossy())) {
            Err(SourceFailure::Raised(error)) => assert_eq!(error.class, "IsADirectoryError"),
            other => panic!("{other:?}"),
        }
        match load_patterns(&PyValue::None) {
            Err(SourceFailure::Raised(error)) => assert_eq!(
                error.message.to_utf8().unwrap(),
                "expected str, bytes or os.PathLike object, not NoneType"
            ),
            other => panic!("{other:?}"),
        }
    }
}
