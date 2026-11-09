//! P1 file reading of `scan_node.py`: the flat `KEY=VALUE` / `KEY: value` greps of the compose and `.env.interconnect` files
//! (`_parse_nccl_lines`, `cross_validate`) and of `manifest.yaml` (`_read_manifest_line`, `read_manifest_field`,
//! `read_manifest_topology`). The bytes come through [`Files`], the explicit file boundary; the decoding (`open(path, encoding="utf-8")`:
//! strict UTF-8, universal newlines), the line grep and the decisions (absent is not unreadable; unreadable is `ManifestUnreadable`)
//! are here.

use crate::scan_node::{
    dict_of, none_if_absent, raise, shown, sorted_values, subscript, text, unsupported, Failure,
    Raised, Res, NCCL_KEYS,
};
use agent_runtime_support::pydyn::{iterate, Text};
use agent_runtime_support::pysort::py_eq;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};

/// The decoder's chunk: CPython's text layer decodes 8192 bytes at a time, so a decode error's `object`/positions are relative to that
/// chunk. Files longer than one chunk with invalid text are reported as not modelled.
const DECODE_CHUNK: usize = 8192;

/// An operating-system failure of `open()`/`read()`: `errno` and `strerror` as `OSError` carries them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileFault {
    pub errno: i32,
    pub strerror: String,
}

/// The file boundary of P1. `path` is the file-system encoding of the argument (surrogate-escaped bytes). A real implementation reads the
/// operating system; checks hand in owned fixtures. No path is ever rewritten here.
pub trait Files {
    /// `os.path.exists(path)` after the embedded-NUL and type guards: any `stat` failure is "not there".
    fn exists(&self, path: &[u8]) -> bool;
    /// `open(path, encoding="utf-8")` followed by `read()`: the whole file, or the failure of the open/read.
    fn read(&self, path: &[u8]) -> Result<Vec<u8>, FileFault>;
}

fn type_name_error(prefix: &str, value: &PyValue) -> Failure {
    raise("TypeError", &format!("{prefix}, not {}", value.type_name()))
}

/// The `str` path of a call, or the error CPython raises for another type (`open`'s wording when `reading`, `os.stat`'s otherwise).
fn path_text(path: &PyValue, reading: bool) -> Res<PyStr> {
    match path.peel() {
        PyValue::Str(own) => Ok(own.clone()),
        PyValue::Bool(_) | PyValue::Int(_) => Err(unsupported(
            "an integer path is a file descriptor in the original and is not modelled",
        )),
        PyValue::Bytes(_) => Err(unsupported("a bytes path is not modelled")),
        other => Err(type_name_error(
            if reading {
                "expected str, bytes or os.PathLike object"
            } else {
                "stat: path should be string, bytes, os.PathLike or integer"
            },
            other,
        )),
    }
}

fn fs_bytes(path: &PyStr) -> Res<Vec<u8>> {
    path.to_surrogateescape_bytes()
        .map_err(|error| raise("UnicodeEncodeError", &error.message()))
}

fn has_nul(path: &PyStr) -> bool {
    path.code_points().contains(&0)
}

/// `os.path.exists(path)`.
fn path_exists(files: &dyn Files, path: &PyValue) -> Res<bool> {
    let own = path_text(path, false)?;
    if has_nul(&own) {
        return Ok(false);
    }
    Ok(files.exists(&fs_bytes(&own)?))
}

/// The lines of a decoded text file (universal newlines) and the decode error that is raised only if the reader gets to the end.
struct Lines {
    lines: Vec<PyStr>,
    deferred: Option<Raised>,
}

fn unicode_decode_error(object: &[u8], start: usize, end: usize, reason: &str) -> Raised {
    let message = if end == start + 1 {
        format!(
            "'utf-8' codec can't decode byte 0x{:02x} in position {start}: {reason}",
            object[start]
        )
    } else {
        format!(
            "'utf-8' codec can't decode bytes in position {start}-{}: {reason}",
            end - 1
        )
    };
    Raised {
        class: "UnicodeDecodeError",
        message: PyStr::new(&message),
        ctor: vec![
            text("utf-8"),
            PyValue::Bytes(object.to_vec()),
            PyValue::int(start),
            PyValue::int(end),
            text(reason),
        ],
        ctor_kwargs: Vec::new(),
        cause: None,
    }
}

fn split_lines(valid: &str) -> Vec<PyStr> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut chars = valid.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\n' => lines.push(PyStr::new(&std::mem::take(&mut current))),
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                lines.push(PyStr::new(&std::mem::take(&mut current)));
            }
            other => current.push(other),
        }
    }
    if !current.is_empty() {
        lines.push(PyStr::new(&current));
    }
    lines
}

fn decode_lines(bytes: &[u8]) -> Res<Lines> {
    match std::str::from_utf8(bytes) {
        Ok(valid) => Ok(Lines {
            lines: split_lines(valid),
            deferred: None,
        }),
        Err(error) => {
            if bytes.len() > DECODE_CHUNK {
                return Err(unsupported(
                    "invalid UTF-8 in a file longer than the 8192-byte decoder chunk is not modelled",
                ));
            }
            let start = error.valid_up_to();
            match error.error_len() {
                Some(length) => {
                    let lead = bytes[start];
                    let reason = if (0x80..=0xC1).contains(&lead) || lead >= 0xF5 {
                        "invalid start byte"
                    } else {
                        "invalid continuation byte"
                    };
                    let end = if reason == "invalid start byte" {
                        start + 1
                    } else {
                        start + length
                    };
                    Err(Failure::Raised(unicode_decode_error(
                        bytes, start, end, reason,
                    )))
                }
                None => {
                    // An incomplete sequence at the end waits in the incremental decoder; it fails only when the reader reaches the end.
                    let valid = std::str::from_utf8(&bytes[..start]).unwrap_or("");
                    let pending = &bytes[start..];
                    Ok(Lines {
                        lines: split_lines(valid),
                        deferred: Some(unicode_decode_error(
                            pending,
                            0,
                            pending.len(),
                            "unexpected end of data",
                        )),
                    })
                }
            }
        }
    }
}

fn strip_quotes(value: &PyStr) -> PyStr {
    let points = value.code_points();
    let quote = u32::from(b'"');
    let start = points
        .iter()
        .position(|&p| p != quote)
        .unwrap_or(points.len());
    let end = points
        .iter()
        .rposition(|&p| p != quote)
        .map_or(start, |i| i + 1);
    PyStr::from_code_points(points[start..end.max(start)].to_vec())
}

fn starts_with(text: &PyStr, prefix: &PyStr) -> bool {
    text.code_points().starts_with(prefix.code_points())
}

/// `text.split(sep, 1)` as `(head, tail)` for a non-empty separator (`None` when the separator is absent).
fn split_once_str(text: &PyStr, sep: &[u32]) -> Option<(PyStr, PyStr)> {
    let points = text.code_points();
    if sep.is_empty() || points.len() < sep.len() {
        return None;
    }
    let at = (0..=points.len() - sep.len()).find(|&at| points[at..at + sep.len()] == *sep)?;
    Some((
        PyStr::from_code_points(points[..at].to_vec()),
        PyStr::from_code_points(points[at + sep.len()..].to_vec()),
    ))
}

fn split_once_at(text: &PyStr, sep: char) -> Option<(PyStr, PyStr)> {
    split_once_str(text, &[u32::from(sep)])
}

fn head_before(text: &PyStr, sep: char) -> PyStr {
    split_once_at(text, sep).map_or_else(|| text.clone(), |(head, _)| head)
}

/// The error text of `OSError(errno, strerror, filename)`: `[Errno N] strerror: 'filename'`.
fn os_error(fault: &FileFault, filename: &PyStr) -> Raised {
    let message = format!(
        "[Errno {}] {}: {}",
        fault.errno,
        fault.strerror,
        filename.repr()
    );
    Raised {
        class: "OSError",
        message: PyStr::new(&message),
        ctor: vec![
            PyValue::int(fault.errno),
            text(&fault.strerror),
            PyValue::Str(filename.clone()),
        ],
        ctor_kwargs: Vec::new(),
        cause: None,
    }
}

/// `_parse_nccl_lines(path, sep)`: the three NCCL keys of a flat file (an unopenable file is an empty answer, as the original's `except OSError`).
pub fn parse_nccl_lines(files: &dyn Files, path: &PyValue, sep: &PyValue) -> Res<PyDict> {
    let Some(sep) = crate::scan_node::as_str(sep) else {
        return Err(unsupported(
            "`key + sep` with a non-string separator is not modelled",
        ));
    };
    let own = path_text(path, true)?;
    if has_nul(&own) {
        return Err(raise("ValueError", "embedded null byte"));
    }
    let mut found = PyDict::new();
    let bytes = match files.read(&fs_bytes(&own)?) {
        Ok(bytes) => bytes,
        Err(_) => return Ok(found),
    };
    let decoded = decode_lines(&bytes)?;
    for line in &decoded.lines {
        let stripped = line.strip();
        for key in NCCL_KEYS {
            let mut prefix = PyStr::new(key);
            prefix.push_py(sep);
            if starts_with(&stripped, &prefix) {
                let Some((_, rest)) = split_once_str(&stripped, sep.code_points()) else {
                    continue;
                };
                let value = strip_quotes(&rest.strip()).strip();
                let value = strip_quotes(&head_before(&value, '#').strip());
                found.insert(PyStr::new(key), PyValue::Str(value));
            }
        }
    }
    if let Some(error) = decoded.deferred {
        return Err(Failure::Raised(error));
    }
    Ok(found)
}

/// `_parse_compose_nccl(path)`: the NCCL keys of a compose file (`KEY: value`).
pub fn parse_compose_nccl(files: &dyn Files, path: &PyValue) -> Res<PyDict> {
    parse_nccl_lines(files, path, &text(":"))
}

/// `_parse_env_file_nccl(path)`: the NCCL keys of an env file (`KEY=value`).
pub fn parse_env_file_nccl(files: &dyn Files, path: &PyValue) -> Res<PyDict> {
    parse_nccl_lines(files, path, &text("="))
}

/// `lstrip("=")`.
fn lstrip_equals(value: &PyStr) -> PyStr {
    let points = value.code_points();
    let start = points
        .iter()
        .position(|&p| p != u32::from(b'='))
        .unwrap_or(points.len());
    PyStr::from_code_points(points[start..].to_vec())
}

fn str_value(value: &PyValue) -> PyStr {
    value.display_text()
}

/// `cross_validate(ic, compose_path, env_interconnect_path)`: the scanned interconnect against the deployed NCCL values of the two sources.
pub fn cross_validate(
    ic: &PyValue,
    compose_path: &PyValue,
    env_interconnect_path: &PyValue,
    files: &dyn Files,
) -> Res<PyValue> {
    let nccl = parse_compose_nccl(files, compose_path)?;
    let env_nccl = if env_interconnect_path.is_truthy() {
        parse_env_file_nccl(files, env_interconnect_path)?
    } else {
        PyDict::new()
    };
    let mut merged = PyDict::new();
    let mut sources = PyDict::new();
    for (key, value) in env_nccl.iter() {
        merged.insert(key.clone(), value.clone());
        sources.insert(key.clone(), text("env.interconnect"));
    }
    for (key, value) in nccl.iter() {
        merged.insert(key.clone(), value.clone());
        sources.insert(key.clone(), text("compose"));
    }
    if merged.is_empty() {
        let env_shown = if env_interconnect_path.is_truthy() {
            shown(env_interconnect_path)
        } else {
            PyStr::new("(미지정)")
        };
        let reason = Text::new()
            .lit("NCCL env 부재(compose+env.interconnect 둘 다): compose=")
            .val(compose_path)
            .lit(" env=")
            .str(&env_shown)
            .done();
        return Ok(dict_of(vec![
            ("checked", PyValue::Bool(false)),
            ("reason", PyValue::Str(reason)),
        ]));
    }
    let merged_get = |key: &str| -> PyValue { none_if_absent(merged.get(key)) };
    let source_of = |key: &str| -> PyValue { none_if_absent(sources.get(key)) };
    let mut checks: Vec<PyValue> = Vec::new();
    let mut matches: Vec<bool> = Vec::new();
    if merged.contains_key("NCCL_IB_HCA") {
        let raw = match merged_get("NCCL_IB_HCA").peel() {
            PyValue::Str(own) => lstrip_equals(own),
            _ => unreachable!("a parsed NCCL value is a string"),
        };
        let mut deployed: Vec<PyValue> = raw
            .code_points()
            .split(|&p| p == u32::from(b','))
            .filter(|piece| !piece.is_empty())
            .map(|piece| PyValue::Str(PyStr::from_code_points(piece.to_vec())))
            .collect();
        deployed = sorted_values(deployed)?;
        let hcas = subscript(ic, "hca_devices")?;
        let scanned_sorted = sorted_values(iterate(&subscript(ic, "hca_devices")?)?)?;
        let matched = py_eq(
            &PyValue::List(scanned_sorted),
            &PyValue::List(deployed.clone()),
        );
        matches.push(matched);
        checks.push(dict_of(vec![
            ("field", text("hca_devices")),
            ("scanned", hcas),
            ("deployed", PyValue::List(deployed)),
            ("source", source_of("NCCL_IB_HCA")),
            ("match", PyValue::Bool(matched)),
        ]));
    }
    if merged.contains_key("NCCL_IB_GID_INDEX") {
        let deployed = merged_get("NCCL_IB_GID_INDEX");
        let scanned = PyValue::Str(str_value(&subscript(ic, "gid_index")?));
        let matched = py_eq(
            &PyValue::Str(str_value(&subscript(ic, "gid_index")?)),
            &deployed,
        );
        matches.push(matched);
        checks.push(dict_of(vec![
            ("field", text("gid_index")),
            ("scanned", scanned),
            ("deployed", deployed),
            ("source", source_of("NCCL_IB_GID_INDEX")),
            ("match", PyValue::Bool(matched)),
        ]));
    }
    if merged.contains_key("NCCL_SOCKET_IFNAME") {
        let scanned = subscript(ic, "socket_iface")?;
        let deployed = merged_get("NCCL_SOCKET_IFNAME");
        let matched = py_eq(&subscript(ic, "socket_iface")?, &deployed);
        matches.push(matched);
        checks.push(dict_of(vec![
            ("field", text("socket_iface")),
            ("scanned", scanned),
            ("deployed", deployed),
            ("source", source_of("NCCL_SOCKET_IFNAME")),
            ("match", PyValue::Bool(matched)),
        ]));
    }
    Ok(dict_of(vec![
        ("checked", PyValue::Bool(true)),
        ("all_match", PyValue::Bool(matches.iter().all(|m| *m))),
        ("sources", PyValue::Dict(sources.clone())),
        ("checks", PyValue::List(checks)),
    ]))
}

/// `_read_manifest_line(path, prefix)`: the value of the first line starting with `prefix`; `None` when the file is absent or has no such line;
/// `ManifestUnreadable` (from the `OSError`) when it exists and cannot be read.
pub fn read_manifest_line_of(files: &dyn Files, path: &PyValue, prefix: &PyValue) -> Res<PyValue> {
    let Some(prefix) = crate::scan_node::as_str(prefix) else {
        return Err(unsupported(
            "`line.startswith(prefix)` with a non-string prefix is not modelled",
        ));
    };
    read_manifest_line(files, path, prefix)
}

fn read_manifest_line(files: &dyn Files, path: &PyValue, prefix: &PyStr) -> Res<PyValue> {
    if !path_exists(files, path)? {
        return Ok(PyValue::None);
    }
    let own = path_text(path, true)?;
    if has_nul(&own) {
        return Err(raise("ValueError", "embedded null byte"));
    }
    let bytes = match files.read(&fs_bytes(&own)?) {
        Ok(bytes) => bytes,
        Err(fault) => {
            let cause = os_error(&fault, &own);
            let mut message = own.clone();
            message.push_str(": ");
            message.push_py(&cause.message);
            return Err(Failure::Raised(Raised {
                class: "ManifestUnreadable",
                ctor: vec![PyValue::Str(message.clone())],
                ctor_kwargs: Vec::new(),
                message,
                cause: Some(Box::new(cause)),
            }));
        }
    };
    let decoded = decode_lines(&bytes)?;
    for line in &decoded.lines {
        let stripped = line.strip();
        if starts_with(&stripped, prefix) {
            let Some((_, after)) = split_once_at(&stripped, ':') else {
                continue;
            };
            let value = strip_quotes(&head_before(&after, '#').strip());
            return Ok(PyValue::Str(value));
        }
    }
    if let Some(error) = decoded.deferred {
        return Err(Failure::Raised(error));
    }
    Ok(PyValue::None)
}

/// `read_manifest_field(path, key)`: the top-level `key: value` of the manifest.
pub fn read_manifest_field(files: &dyn Files, path: &PyValue, key: &PyValue) -> Res<PyValue> {
    let prefix = match key.peel() {
        PyValue::Str(own) => {
            let mut joined = own.clone();
            joined.push_str(":");
            joined
        }
        other @ (PyValue::List(_) | PyValue::Tuple(_)) => {
            return Err(raise(
                "TypeError",
                &format!(
                    "can only concatenate {name} (not \"str\") to {name}",
                    name = other.type_name()
                ),
            ))
        }
        other => {
            return Err(raise(
                "TypeError",
                &format!(
                    "unsupported operand type(s) for +: '{}' and 'str'",
                    other.type_name()
                ),
            ))
        }
    };
    read_manifest_line(files, path, &prefix)
}

/// `read_manifest_topology(path)`: the manifest's `topology:` value.
pub fn read_manifest_topology(files: &dyn Files, path: &PyValue) -> Res<PyValue> {
    read_manifest_line(files, path, &PyStr::new("topology:"))
}
