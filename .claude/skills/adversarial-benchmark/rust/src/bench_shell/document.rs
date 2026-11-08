//! The reading and writing of JSON documents as the originals did it (`json.load(open(path))`, `json.loads(text)`,
//! `json.dump(value, open(path, "w"), ...)`), and the few Python expressions every operation shares.

use crate::cli::os_error_text;
use crate::guidellm::Failure;
use crate::pydyn::{as_get_target, or_none};
use crate::pyfs::os_error_class;
use crate::pyjson::{decode_file_text, loads_text, LoadError};
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use std::io::Write;

/// The exception a failed `json.load`/`json.loads` raises.
pub(crate) fn load_failure(error: LoadError) -> Failure {
    match error {
        LoadError::Decode { .. } => Failure::new("JSONDecodeError", &error.to_string()),
        LoadError::IntegerTooLong { .. } => Failure::new("ValueError", &error.to_string()),
        LoadError::Encoding(message) => Failure::new("UnicodeDecodeError", &message),
        LoadError::TooDeep { .. } => {
            let text = error.to_string();
            Failure::new(
                "RecursionError",
                text.strip_prefix("RecursionError: ").unwrap_or(&text),
            )
        }
    }
}

/// `json.loads(text)`.
pub(crate) fn load_text(text: &PyStr) -> Result<PyValue, Failure> {
    loads_text(text).map_err(load_failure)
}

/// `json.load(open(path))`: strict UTF-8, universal newlines; every failure is the exception of the step that failed.
pub(crate) fn load_file(path: &PyStr) -> Result<PyValue, Failure> {
    let bytes = crate::pyfs::read_bytes(path)?;
    let text = decode_file_text(&bytes).map_err(load_failure)?;
    load_text(&text)
}

/// `value.get(key)`: the member, `None` when absent; a value that is not an object has no `get`.
pub(crate) fn get(value: &PyValue, key: &str) -> Result<PyValue, Failure> {
    Ok(or_none(as_get_target(value)?.get(key)))
}

/// `value.get(key) or {}`.
pub(crate) fn get_or_empty_object(value: &PyValue, key: &str) -> Result<PyValue, Failure> {
    let found = get(value, key)?;
    Ok(if found.is_truthy() {
        found
    } else {
        PyValue::Dict(PyDict::new())
    })
}

/// `print(value or "")`'s text: a falsy value prints nothing, any other its `str()`.
pub(crate) fn or_empty(value: &PyValue) -> PyStr {
    if value.is_truthy() {
        value.display_text()
    } else {
        PyStr::default()
    }
}

/// `text.startswith(prefix)`.
pub(crate) fn starts_with(text: &PyStr, prefix: &str) -> bool {
    let wanted: Vec<u32> = prefix.chars().map(u32::from).collect();
    text.code_points().starts_with(&wanted)
}

/// `"%-<width>s" % value`: the `str()` of the value, padded on the right to `width` characters.
pub(crate) fn left_justified(value: &PyValue, width: usize) -> PyStr {
    let mut text = value.display_text();
    for _ in text.len()..width {
        text.push_str(" ");
    }
    text
}

/// `value["key"] = item` on a value that should be an object.
pub(crate) fn set_member(value: &mut PyValue, key: &str, item: PyValue) -> Result<(), Failure> {
    match value {
        PyValue::Dict(map) => {
            map.set(key, item);
            Ok(())
        }
        PyValue::List(_) => Err(Failure::new(
            "TypeError",
            "list indices must be integers or slices, not str",
        )),
        other => Err(Failure::new(
            "TypeError",
            &format!(
                "'{}' object does not support item assignment",
                other.type_name()
            ),
        )),
    }
}

fn write_failure(error: &std::io::Error, path: &PyStr) -> Failure {
    Failure::new(os_error_class(error), &os_error_text(error, path))
}

/// `with open(path, "w", encoding="utf-8") as f: json.dump(value, f, ensure_ascii=False, indent=2)`, then optionally
/// `f.write("\n")` after a dump that completed.
pub(crate) fn write_pretty(
    path: &PyStr,
    value: &PyValue,
    final_newline: bool,
) -> Result<(), Failure> {
    crate::assemble_sweep_meta::write_pretty_document(path, value, final_newline)
}

/// `json.dump(value, open(path, "w", encoding="utf-8"), ensure_ascii=False)`: the default separators, no newline.
///
/// The file is opened (created or emptied) first. The pure-Python encoder hands the file one chunk at a time and a
/// chunk holding a lone surrogate cannot be encoded: what came before it is in the file, the chunk and what follows
/// are not, and the error names the position inside that chunk. A string that is a list item shares its chunk with
/// the separator in front of it (`[` or `, `); a key or a member value is a chunk of its own.
pub(crate) fn write_compact(path: &PyStr, value: &PyValue) -> Result<(), Failure> {
    let mut file = crate::json_stream::open_for_writing(path)?;
    let dumped = value
        .dumps_unicode()
        .map_err(crate::dataset_prep::dump_failure)?;
    match crate::pyfs::strict_utf8(&dumped) {
        Ok(bytes) => file
            .write_all(&bytes)
            .map_err(|error| write_failure(&error, path)),
        Err(_) => {
            let (kept, failing) = split_at_failing_chunk(&dumped);
            let prefix = crate::pyfs::strict_utf8(&kept)
                .expect("the chunks before the first lone surrogate are encodable");
            file.write_all(&prefix)
                .map_err(|error| write_failure(&error, path))?;
            let failure = crate::pyfs::strict_utf8(&failing)
                .expect_err("the failing chunk holds a lone surrogate");
            Err(Failure::new("UnicodeEncodeError", &failure.message()))
        }
    }
}

fn is_surrogate(point: u32) -> bool {
    (0xD800..=0xDFFF).contains(&point)
}

/// The text before the encoder chunk that holds the first lone surrogate of a compact document, and that chunk.
fn split_at_failing_chunk(dumped: &PyStr) -> (PyStr, PyStr) {
    let points = dumped.code_points();
    let first = points
        .iter()
        .position(|&point| is_surrogate(point))
        .expect("the document holds a lone surrogate");
    let (quote, backslash) = (u32::from('"'), u32::from('\\'));
    let mut containers: Vec<u32> = Vec::new();
    let mut at = 0;
    while at < points.len() {
        let point = points[at];
        if point == quote {
            let start = at;
            at += 1;
            while points[at] != quote {
                at += if points[at] == backslash { 2 } else { 1 };
            }
            let end = at + 1;
            if first < end {
                let in_list = containers.last() == Some(&u32::from('['));
                let after_open = start > 0 && points[start - 1] == u32::from('[');
                let chunk_start = match (in_list, after_open) {
                    (true, true) => start - 1,
                    (true, false) => start - 2,
                    (false, _) => start,
                };
                return (
                    PyStr::from_code_points(points[..chunk_start].to_vec()),
                    PyStr::from_code_points(points[chunk_start..end].to_vec()),
                );
            }
            at = end;
            continue;
        }
        if point == u32::from('[') || point == u32::from('{') {
            containers.push(point);
        } else if point == u32::from(']') || point == u32::from('}') {
            containers.pop();
        }
        at += 1;
    }
    unreachable!("a lone surrogate sits inside a string")
}
