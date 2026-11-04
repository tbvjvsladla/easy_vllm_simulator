//! Incremental JSON writing with the chunk boundaries of CPython's pure-Python encoder.
//!
//! `json.dump(value, handle, ensure_ascii=False, indent=N)` does not build the document first: it iterates the
//! encoder (`json.encoder._make_iterencode`) and hands each chunk to `handle.write`, which encodes it as UTF-8 on
//! its own. Two observable consequences are kept here:
//!
//! * **Failure timing.** A chunk that cannot be encoded (a lone surrogate) or a value that cannot be written (an
//!   integer past the digit limit, an object JSON cannot carry) fails *at that chunk*; every chunk before it has
//!   already been written. The `UnicodeEncodeError` position is local to the failing chunk, whose boundaries are
//!   the encoder's: a dict key, the `": "` after it, a dict value and each separator are chunks of their own, while
//!   a string or number that is a *list item* shares its chunk with the separator in front of it.
//! * **Recursion.** Each nested container is one generator frame of the interpreter, so a document nested deeper
//!   than the interpreter's frame limit fails with `RecursionError` when the too-deep container is created -
//!   after the chunks that precede it were written, before anything of it is. [`PYTHON_RECURSION_LIMIT`] and the
//!   `top_frame` argument (the frame number of the outermost container, which depends on who called `json.dump`)
//!   reproduce that.
//!
//! [`dump_indented`] knows nothing of files: chunks go to a [`ChunkSink`]. [`FileSink`] is the sink of a file
//! opened the way `open(path, "w", encoding="utf-8")` does.

use crate::cli::os_error_text;
use crate::guidellm::Failure;
use crate::pyfs::{os_error_class, os_string};
use crate::pystr::{EncodeFailure, PyStr};
use crate::pyvalue::PyValue;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

/// `sys.getrecursionlimit()` of the interpreter the originals ran on.
pub const PYTHON_RECURSION_LIMIT: usize = 1000;

type Res<T> = Result<T, Failure>;

/// Receives the chunks of a document in order. A failure ends the dump; chunks already accepted stay accepted.
pub trait ChunkSink {
    fn write(&mut self, chunk: &PyStr) -> Res<()>;
}

/// `RecursionError("maximum recursion depth exceeded")`.
pub fn recursion_error() -> Failure {
    Failure::new("RecursionError", "maximum recursion depth exceeded")
}

/// Writes `value` as `json.dump(value, fp, ensure_ascii=False, indent=indent)` does. `top_frame` is the interpreter
/// frame number of the outermost container's generator (for a call made by `main()` of a script it is 5: module,
/// `main`, `dump`, `_iterencode`, `_iterencode_dict`).
pub fn dump_indented(
    value: &PyValue,
    indent: usize,
    top_frame: usize,
    sink: &mut dyn ChunkSink,
) -> Res<()> {
    let mut encoder = Encoder {
        indent: " ".repeat(indent),
        sink,
    };
    encoder.value(value, 0, top_frame)
}

struct Encoder<'a> {
    indent: String,
    sink: &'a mut dyn ChunkSink,
}

fn text(piece: &str) -> PyStr {
    PyStr::new(piece)
}

/// Entering a generator at `frame` fails when it is deeper than the interpreter allows.
fn enter(frame: usize) -> Res<()> {
    if frame > PYTHON_RECURSION_LIMIT {
        Err(recursion_error())
    } else {
        Ok(())
    }
}

/// The frame of a generator created by the container whose own frame is `parent`.
fn child_frame(parent: usize) -> Res<usize> {
    enter(parent + 1)?;
    Ok(parent + 1)
}

impl Encoder<'_> {
    fn chunk(&mut self, piece: &PyStr) -> Res<()> {
        self.sink.write(piece)
    }

    fn newline_indent(&self, level: usize) -> String {
        format!("\n{}", self.indent.repeat(level))
    }

    /// `_iterencode(o, level)` for the outermost value.
    fn value(&mut self, value: &PyValue, level: usize, frame: usize) -> Res<()> {
        match value.peel() {
            PyValue::List(items) | PyValue::Tuple(items) => {
                enter(frame)?;
                self.list(items, level, frame)
            }
            PyValue::Dict(_) => {
                enter(frame)?;
                self.dict(value, level, frame)
            }
            scalar => {
                let piece = scalar_chunk(scalar, frame)?;
                self.chunk(&piece)
            }
        }
    }

    fn dict(&mut self, value: &PyValue, level: usize, frame: usize) -> Res<()> {
        let PyValue::Dict(map) = value.peel() else {
            unreachable!("caller checked the kind")
        };
        if map.is_empty() {
            return self.chunk(&text("{}"));
        }
        self.chunk(&text("{"))?;
        let level = level + 1;
        let newline = self.newline_indent(level);
        let separator = text(&format!(",{newline}"));
        self.chunk(&text(&newline))?;
        for (position, (key, item)) in map.iter().enumerate() {
            if position > 0 {
                self.chunk(&separator)?;
            }
            let mut encoded_key = PyStr::default();
            key.write_json(&mut encoded_key, false);
            self.chunk(&encoded_key)?;
            self.chunk(&text(": "))?;
            self.item(item, level, frame)?;
        }
        let closing = self.newline_indent(level - 1);
        self.chunk(&text(&closing))?;
        self.chunk(&text("}"))
    }

    /// A dict value: scalars are chunks of their own, containers are generators one frame deeper.
    fn item(&mut self, item: &PyValue, level: usize, frame: usize) -> Res<()> {
        match item.peel() {
            PyValue::List(items) | PyValue::Tuple(items) => {
                let child = child_frame(frame)?;
                self.list(items, level, child)
            }
            PyValue::Dict(_) => {
                let child = child_frame(frame)?;
                self.dict(item, level, child)
            }
            scalar => {
                let piece = scalar_chunk(scalar, frame)?;
                self.chunk(&piece)
            }
        }
    }

    fn list(&mut self, items: &[PyValue], level: usize, frame: usize) -> Res<()> {
        if items.is_empty() {
            return self.chunk(&text("[]"));
        }
        let level = level + 1;
        let newline = self.newline_indent(level);
        let separator = format!(",{newline}");
        let mut lead = format!("[{newline}");
        for (position, item) in items.iter().enumerate() {
            if position > 0 {
                lead.clone_from(&separator);
            }
            match item.peel() {
                PyValue::List(inner) | PyValue::Tuple(inner) => {
                    self.chunk(&text(&lead))?;
                    let child = child_frame(frame)?;
                    self.list(inner, level, child)?;
                }
                PyValue::Dict(_) => {
                    self.chunk(&text(&lead))?;
                    let child = child_frame(frame)?;
                    self.dict(item, level, child)?;
                }
                scalar => {
                    // a scalar item shares its chunk with the separator in front of it
                    let mut piece = text(&lead);
                    piece.push_py(&scalar_chunk(scalar, frame)?);
                    self.chunk(&piece)?;
                }
            }
        }
        let closing = self.newline_indent(level - 1);
        self.chunk(&text(&closing))?;
        self.chunk(&text("]"))
    }
}

/// The chunk of a scalar (`_encoder(str)`, `null`, `true`, `false`, `int.__repr__`, `float.__repr__`), or the
/// failure of a value JSON cannot carry. `frame` is the frame of the container writing it: a float goes through the
/// encoder's Python helper `floatstr`, one frame deeper, while strings and integers are built-in calls.
fn scalar_chunk(value: &PyValue, frame: usize) -> Res<PyStr> {
    if matches!(value, PyValue::Float(_)) {
        enter(frame + 1)?;
    }
    match value {
        PyValue::Str(string) => {
            let mut out = PyStr::default();
            string.write_json(&mut out, false);
            Ok(out)
        }
        PyValue::None | PyValue::Bool(_) | PyValue::Int(_) | PyValue::Float(_) => value
            .dumps_ascii()
            .map(|rendered| text(&rendered))
            .map_err(|error| {
                let rendered = error.to_string();
                Failure::new(
                    error.python_type(),
                    rendered
                        .strip_prefix(&format!("{}: ", error.python_type()))
                        .unwrap_or(&rendered),
                )
            }),
        other => Err(Failure::new(
            "TypeError",
            &format!(
                "Object of type {} is not JSON serializable",
                other.type_name()
            ),
        )),
    }
}

/// A file opened as `open(path, "w", encoding="utf-8")` opens it: created or truncated in place (through a symbolic
/// link, keeping the file's identity) before anything is encoded.
pub fn open_for_writing(path: &PyStr) -> Res<File> {
    let os = os_string(path).map_err(crate::pyfs::FsFailure::into_failure)?;
    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(Path::new(&os))
        .map_err(|error| Failure::new(os_error_class(&error), &os_error_text(&error, path)))
}

fn encode_failure(failure: &EncodeFailure) -> Failure {
    Failure::new("UnicodeEncodeError", &failure.message())
}

fn write_failure(error: &std::io::Error) -> Failure {
    let detail = error.to_string();
    let message = detail
        .split(" (os error")
        .next()
        .unwrap_or(&detail)
        .to_string();
    let message = match error.raw_os_error() {
        Some(errno) => format!("[Errno {errno}] {message}"),
        None => message,
    };
    Failure::new(os_error_class(error), &message)
}

/// Writes one text in one `handle.write(text)`: encoded whole first (a lone surrogate fails with nothing written),
/// then written.
pub fn write_text_once(file: &mut File, text: &PyStr) -> Res<()> {
    let bytes = crate::pyfs::strict_utf8(text).map_err(|failure| encode_failure(&failure))?;
    file.write_all(&bytes)
        .map_err(|error| write_failure(&error))
}

/// The sink of a file being written chunk by chunk. Each chunk is encoded on its own (strictly), so a failing chunk
/// leaves exactly the chunks before it in the file; [`FileSink::close`] flushes what was accepted, as closing a
/// file does, whether or not the dump failed.
pub struct FileSink {
    writer: BufWriter<File>,
}

impl FileSink {
    pub fn new(file: File) -> FileSink {
        FileSink {
            writer: BufWriter::with_capacity(8192, file),
        }
    }

    pub fn close(mut self) -> Res<()> {
        self.writer.flush().map_err(|error| write_failure(&error))
    }
}

impl ChunkSink for FileSink {
    fn write(&mut self, chunk: &PyStr) -> Res<()> {
        let bytes = crate::pyfs::strict_utf8(chunk).map_err(|failure| encode_failure(&failure))?;
        self.writer
            .write_all(&bytes)
            .map_err(|error| write_failure(&error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pyjson::loads;

    struct Collect(Vec<String>);

    impl ChunkSink for Collect {
        fn write(&mut self, chunk: &PyStr) -> Res<()> {
            self.0.push(chunk.to_utf8().expect("UTF-8"));
            Ok(())
        }
    }

    #[test]
    fn chunks_follow_the_encoder_and_join_to_json_dumps_with_indent() {
        let value = loads(r#"{"a": [1, "x", {"b": null}, []], "c": {}, "d": 1.5}"#).unwrap();
        let mut sink = Collect(Vec::new());
        dump_indented(&value, 2, 5, &mut sink).unwrap();
        assert_eq!(
            sink.0.concat(),
            value.dumps_pretty().unwrap().to_utf8().unwrap()
        );
        // a list item and the separator in front of it are one chunk; a dict value is its own chunk
        assert!(sink.0.contains(&"[\n    1".to_string()));
        assert!(sink.0.contains(&",\n    \"x\"".to_string()));
        assert!(sink.0.contains(&"1.5".to_string()));
    }

    #[test]
    fn a_container_past_the_frame_limit_fails_after_the_chunks_before_it() {
        let mut value = PyValue::Dict(crate::pyvalue::PyDict::new());
        for _ in 0..5 {
            let mut outer = crate::pyvalue::PyDict::new();
            outer.set("x", value);
            value = PyValue::Dict(outer);
        }
        let mut sink = Collect(Vec::new());
        // the outermost container is frame 998, so only two levels below it exist (999, 1000)
        let failure = dump_indented(&value, 2, 998, &mut sink).unwrap_err();
        assert_eq!(failure.class, "RecursionError");
        assert_eq!(
            failure.message.to_utf8().unwrap(),
            "maximum recursion depth exceeded"
        );
        assert_eq!(
            sink.0.last().unwrap(),
            ": ",
            "the key separator is already written"
        );
        let mut deep_enough = Collect(Vec::new());
        assert!(dump_indented(&value, 2, 995, &mut deep_enough).is_ok());
    }

    struct Strict(usize);

    impl ChunkSink for Strict {
        fn write(&mut self, chunk: &PyStr) -> Res<()> {
            crate::pyfs::strict_utf8(chunk)
                .map(|bytes| self.0 += bytes.len())
                .map_err(|failure| encode_failure(&failure))
        }
    }

    #[test]
    fn the_failing_position_is_local_to_the_chunk_whose_boundaries_the_encoder_sets() {
        // a dict value is its own chunk: the position counts from its opening quote
        let in_dict = loads(r#"{"k": "before\ud800after"}"#).unwrap();
        let failure = dump_indented(&in_dict, 2, 5, &mut Strict(0)).unwrap_err();
        assert!(
            failure
                .message
                .to_utf8()
                .unwrap()
                .contains("in position 7:"),
            "{:?}",
            failure.message
        );
        // a list item shares its chunk with the separator in front of it ("[\n    " is 6 characters)
        let in_list = loads(r#"{"k": ["a\ud800"]}"#).unwrap();
        let failure = dump_indented(&in_list, 2, 5, &mut Strict(0)).unwrap_err();
        assert!(
            failure
                .message
                .to_utf8()
                .unwrap()
                .contains("in position 8:"),
            "{:?}",
            failure.message
        );
        // an earlier, fine chunk was accepted before the failure
        let mut sink = Strict(0);
        let _ = dump_indented(&in_dict, 2, 5, &mut sink);
        assert!(sink.0 > 0);
    }
}
