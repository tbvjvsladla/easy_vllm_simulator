//! Rust port of `adversarial-benchmark/scripts/dataset_prep.py` (the full-bench dataset axis): the pure helpers and the
//! two file readers the commands are built from. The commands themselves are in [`crate::dataset_commands`], the
//! command line in [`crate::dataset_prep_cli`], the Rust-only transport in [`crate::dataset_prep_api`].
//!
//! Everything follows the original's Python semantics on dynamically typed input (JSON fields are whatever the
//! document holds): `isinstance(c, int)` accepts a `bool`, a dictionary keeps the first of two equal keys (`True` and
//! `1`), `c == target` accepts a float equal to an integer, and every failing operation raises the exception
//! (class and message) CPython raises. Paths are `pathlib` paths: their text is normalised and an `OSError` names the
//! normalised text.

use crate::guidellm::Failure;
use crate::pyfs;
use crate::pyjson::{decode_file_text, loads_text_objects, LoadError};
use crate::pyops::{compare, subtract, Compare};
use crate::pysort::py_eq;
use crate::pystr::PyStr;
use crate::pyvalue::{DumpError, PyDict, PyValue};
use crate::repeat_api_cli::load_failure;
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use ring::digest::{Context, SHA256};
use std::collections::HashMap;

/// The prepared dataset and its key record inside a cache folder.
pub const PREP_NAME: &str = "prepared.jsonl";
pub const PREP_META: &str = "prepared_meta.json";
/// The columns `guidellm preprocess` adds.
pub const PROMPT_COUNT_COL: &str = "prompt_tokens_count";
pub const OUTPUT_COUNT_COL: &str = "output_tokens_count";
/// A target length farther than this from the requested one means the preprocessing missed it.
const TARGET_TOLERANCE: i64 = 8;

/// Where the original program writes `sys.stdout` and `sys.stderr` while a command runs.
pub trait Console {
    /// `print(line)`: the text is encoded for the stream first, so a lone surrogate is a `UnicodeEncodeError`.
    fn print(&mut self, line: &PyStr) -> Result<(), Failure>;
    /// `parser.print_help()`: one write of the whole text.
    fn help(&mut self, text: &str);
    /// `sys.stderr.write(text)`.
    fn error(&mut self, text: &PyStr);
}

/// A console that keeps what was written (the transport and the self-test use it).
#[derive(Default)]
pub struct Recorder {
    pub stdout: PyStr,
    pub stderr: PyStr,
}

impl Console for Recorder {
    fn print(&mut self, line: &PyStr) -> Result<(), Failure> {
        if let Err(failure) = line.to_surrogateescape_bytes() {
            return Err(Failure::new("UnicodeEncodeError", &failure.message()));
        }
        self.stdout.push_py(line);
        self.stdout.push_str("\n");
        Ok(())
    }

    fn help(&mut self, text: &str) {
        self.stdout.push_str(text);
    }

    fn error(&mut self, text: &PyStr) {
        self.stderr.push_py(text);
    }
}

// ── Python plumbing shared by the commands ──────────────────────────────────────

/// `_die(rc, msg)`: the message on standard error, then `SystemExit(rc)`.
pub(crate) fn die(console: &mut dyn Console, rc: i32, message: &PyStr) -> Failure {
    let mut text = PyStr::new("[dataset_prep] ");
    text.push_py(message);
    text.push_str("\n");
    console.error(&text);
    Failure::new("SystemExit", &rc.to_string())
}

/// `Path(value)` rendered as `str(path)`: only a string (or path-like) is accepted, and the text is normalised.
pub(crate) fn path_text(value: &PyValue) -> Result<PyStr, Failure> {
    match value.peel() {
        PyValue::Str(text) => Ok(pyfs::normalize(text)),
        other => Err(Failure::new(
            "TypeError",
            &format!(
                "argument should be a str or an os.PathLike object where __fspath__ returns a str, not {}",
                PyStr::new(other.type_name()).repr()
            ),
        )),
    }
}

/// `Path.name`.
pub(crate) fn path_name(path: &PyStr) -> PyStr {
    let points = path.code_points();
    if points == [u32::from('.')] {
        return PyStr::default();
    }
    let start = points
        .iter()
        .rposition(|&point| point == u32::from('/'))
        .map_or(0, |at| at + 1);
    PyStr::from_code_points(points[start..].to_vec())
}

/// `Path / name` for a name that holds no separator.
pub(crate) fn child_path(base: &PyStr, name: &PyStr) -> PyStr {
    pyfs::join_text(base, name)
}

pub(crate) fn dump_failure(error: DumpError) -> Failure {
    let text = error.to_string();
    let prefix = format!("{}: ", error.python_type());
    Failure::new(
        error.python_type(),
        text.strip_prefix(&prefix).unwrap_or(&text),
    )
}

/// `json.dumps(value, ensure_ascii=False, indent=2) + "\n"`.
pub(crate) fn pretty_document(value: &PyValue) -> Result<PyStr, Failure> {
    let mut text = value.dumps_pretty().map_err(dump_failure)?;
    text.push_str("\n");
    Ok(text)
}

/// `Path.read_text(encoding="utf-8")`.
pub(crate) fn read_text(path: &PyStr) -> Result<PyStr, Failure> {
    let bytes = pyfs::read_bytes(path)?;
    decode_file_text(&bytes).map_err(|error| load_failure(&error))
}

/// `json.loads(path.read_text(encoding="utf-8"))`; the literal `NaN` is the decoder's shared object.
pub(crate) fn read_json(path: &PyStr) -> Result<PyValue, Failure> {
    let text = read_text(path)?;
    loads_text_objects(&text).map_err(|error: LoadError| load_failure(&error))
}

/// `except (OSError, ValueError)`: the exception classes that clause catches (`UnicodeError` and `JSONDecodeError`
/// are `ValueError`s).
pub(crate) fn is_os_or_value_error(class: &str) -> bool {
    matches!(
        class,
        "ValueError"
            | "UnicodeDecodeError"
            | "UnicodeEncodeError"
            | "JSONDecodeError"
            | "OSError"
            | "PermissionError"
            | "FileNotFoundError"
            | "ProcessLookupError"
            | "InterruptedError"
            | "ChildProcessError"
            | "BlockingIOError"
            | "FileExistsError"
            | "NotADirectoryError"
            | "IsADirectoryError"
            | "BrokenPipeError"
            | "ConnectionAbortedError"
            | "ConnectionResetError"
            | "TimeoutError"
            | "ConnectionRefusedError"
    )
}

/// `value.get(key)`: a dictionary answers, anything else has no such method.
pub(crate) fn dict_get(value: &PyValue, key: &str) -> Result<PyValue, Failure> {
    match value.peel() {
        PyValue::Dict(map) => Ok(map.get(key).cloned().unwrap_or(PyValue::None)),
        other => Err(Failure::new(
            "AttributeError",
            &format!("'{}' object has no attribute 'get'", other.type_name()),
        )),
    }
}

// ── _load_spec ──────────────────────────────────────────────────────────────────

/// `_load_spec(path)`: the dataset resolution document, which must be an object that has `dataset_kind`. An unreadable
/// or undecodable file and a document without the key end the program with `SystemExit(2)` and a message.
pub fn load_spec(path: &PyValue, console: &mut dyn Console) -> Result<PyValue, Failure> {
    let normalised = path_text(path)?;
    let spec = match read_json(&normalised) {
        Ok(spec) => spec,
        Err(failure) if is_os_or_value_error(failure.class) => {
            let message = crate::pydyn::Text::new()
                .lit("데이터셋 해소값(")
                .val(path)
                .lit(")을 읽지 못했다: ")
                .str(&failure.message)
                .done();
            return Err(die(console, 2, &message));
        }
        Err(failure) => return Err(failure),
    };
    let has_kind = matches!(spec.peel(), PyValue::Dict(map) if map.contains_key("dataset_kind"));
    if !has_kind {
        let message = crate::pydyn::Text::new()
            .lit("데이터셋 해소값에 dataset_kind 가 없다: ")
            .val(path)
            .done();
        return Err(die(console, 2, &message));
    }
    Ok(spec)
}

// ── _dir_digest ─────────────────────────────────────────────────────────────────

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// `_dir_digest(d)`: SHA-256 over, for each regular file directly inside the folder in name order (code point order),
/// the file name and the SHA-256 digest of the whole file. `d` is a `pathlib.Path` (a string on the wire); anything
/// else has no `iterdir`.
pub fn dir_digest(folder: &PyValue) -> Result<String, Failure> {
    let PyValue::Str(text) = folder.peel() else {
        return Err(Failure::new(
            "AttributeError",
            &format!(
                "'{}' object has no attribute 'iterdir'",
                folder.peel().type_name()
            ),
        ));
    };
    let base = pyfs::normalize(text);
    let mut names = pyfs::list_dir(&base)?;
    names.sort_by(|left, right| left.code_points().cmp(right.code_points()));
    let mut outer = Context::new(&SHA256);
    for name in &names {
        let path = child_path(&base, name);
        if !pyfs::is_file(&path)? {
            continue;
        }
        let encoded = pyfs::strict_utf8(name)
            .map_err(|failure| Failure::new("UnicodeEncodeError", &failure.message()))?;
        outer.update(&encoded);
        let mut inner = Context::new(&SHA256);
        pyfs::read_in_blocks(&path, |block| inner.update(block))?;
        outer.update(inner.finish().as_ref());
    }
    Ok(hex(outer.finish().as_ref()))
}

// ── argument builders ───────────────────────────────────────────────────────────

/// `json.dumps(value, separators=(",", ":"))` (ASCII output).
fn tight_dumps(value: PyValue) -> Result<PyStr, Failure> {
    value
        .dumps_ascii_tight()
        .map(|text| PyStr::new(&text))
        .map_err(dump_failure)
}

/// `column_mapper_arg(spec, with_counts)`: the GuideLLM column mapper as one JSON object. Giving
/// `column_mappings` replaces GuideLLM's whole default mapping, so the measuring form names all three columns.
pub fn column_mapper_arg(spec: &PyValue, with_counts: &PyValue) -> Result<PyStr, Failure> {
    let column_map = crate::pydyn::subscript(spec, "column_map")?;
    let text_column = crate::pydyn::subscript(column_map, "text_column")?;
    let mut mappings = PyDict::new();
    mappings.set("text_column", text_column.clone());
    if with_counts.is_truthy() {
        mappings.set("prompt_tokens_count_column", PyValue::str(PROMPT_COUNT_COL));
        mappings.set("output_tokens_count_column", PyValue::str(OUTPUT_COUNT_COL));
    }
    let mut mapper = PyDict::new();
    mapper.set("kind", PyValue::str("generative_column_mapper"));
    mapper.set("column_mappings", PyValue::Dict(mappings));
    tight_dumps(PyValue::Dict(mapper))
}

/// `data_arg(kind, path)`: the `--data` argument as one JSON object; a single file is read as a `Dataset`
/// (`split=train`).
pub fn data_arg(kind: &PyValue, path: &PyValue) -> Result<PyStr, Failure> {
    let mut split = PyDict::new();
    split.set("split", PyValue::str("train"));
    let mut data = PyDict::new();
    data.set("kind", kind.clone());
    data.set("path", path.clone());
    data.set("load_kwargs", PyValue::Dict(split));
    tight_dumps(PyValue::Dict(data))
}

// ── target_rows ─────────────────────────────────────────────────────────────────

/// `(target, pool)`: the target length (the most frequent integer count, or `None`) and the lines of that length.
#[derive(Clone, Debug)]
pub struct TargetRows {
    pub target: PyValue,
    pub pool: Vec<PyValue>,
}

impl TargetRows {
    /// The Python tuple `(target, pool)`.
    pub fn to_value(&self) -> PyValue {
        PyValue::Tuple(vec![self.target.clone(), PyValue::List(self.pool.clone())])
    }
}

/// A value that is either borrowed from the caller's rows or built while unpacking (a string or dictionary row).
enum Item<'a> {
    Borrowed(&'a PyValue),
    Owned(PyValue),
}

impl Item<'_> {
    fn get(&self) -> &PyValue {
        match self {
            Item::Borrowed(value) => value,
            Item::Owned(value) => value,
        }
    }
}

/// `for c, ln in rows`: the two items of one row, unpacked the way Python unpacks any iterable. A two-item list or
/// tuple (the only shape the callers produce) is borrowed, not copied.
fn unpack_pair(row: &PyValue) -> Result<(Item<'_>, Item<'_>), Failure> {
    if let PyValue::List(items) | PyValue::Tuple(items) = row.peel() {
        if let [first, second] = items.as_slice() {
            return Ok((Item::Borrowed(first), Item::Borrowed(second)));
        }
    }
    let items: Vec<PyValue> = match row.peel() {
        PyValue::List(items) | PyValue::Tuple(items) => items.clone(),
        PyValue::Dict(_) | PyValue::Str(_) | PyValue::Bytes(_) => crate::pydyn::iterate(row)?,
        other => {
            return Err(Failure::new(
                "TypeError",
                &format!("cannot unpack non-iterable {} object", other.type_name()),
            ))
        }
    };
    match <[PyValue; 2]>::try_from(items) {
        Ok([first, second]) => Ok((Item::Owned(first), Item::Owned(second))),
        Err(items) if items.len() < 2 => Err(Failure::new(
            "ValueError",
            &format!(
                "not enough values to unpack (expected 2, got {})",
                items.len()
            ),
        )),
        Err(_) => Err(Failure::new(
            "ValueError",
            "too many values to unpack (expected 2)",
        )),
    }
}

/// The key of `counts[c]` for an `int` or `bool` count: equal values are one key (`True` is `1`).
#[derive(Clone, PartialEq, Eq, Hash)]
enum CountKey {
    Small(i128),
    Large(BigInt),
}

impl CountKey {
    fn of(count: &PyValue) -> Option<CountKey> {
        match count.peel() {
            PyValue::Int(number) => Some(
                number
                    .to_i128()
                    .map_or_else(|| CountKey::Large(number.clone()), CountKey::Small),
            ),
            PyValue::Bool(flag) => Some(CountKey::Small(i128::from(*flag))),
            _ => None,
        }
    }

    fn order(&self, other: &CountKey) -> std::cmp::Ordering {
        match (self, other) {
            (CountKey::Small(left), CountKey::Small(right)) => left.cmp(right),
            _ => self.big().cmp(&other.big()),
        }
    }

    fn big(&self) -> BigInt {
        match self {
            CountKey::Small(number) => BigInt::from(*number),
            CountKey::Large(number) => number.clone(),
        }
    }
}

/// How many distinct counts a row list may show before the lookup switches from a scan to a hash table.
const SCAN_LIMIT: usize = 8;

/// `target_rows(rows, input_len)`: `rows` holds `(prompt_tokens_count, line)` pairs. The target is the most frequent
/// integer count (the smallest one on a tie), because the preprocessing makes nearly every row that length and
/// tokenizers that count a special token shift it; a target more than 8 from `input_len` means the preprocessing
/// missed the length, so the pool is empty and the caller fails closed.
pub fn target_rows(rows: &PyValue, input_len: &PyValue) -> Result<TargetRows, Failure> {
    let owned: Vec<PyValue>;
    let list: &[PyValue] = match rows.peel() {
        PyValue::List(items) | PyValue::Tuple(items) => items,
        _ => {
            owned = crate::pydyn::iterate(rows)?;
            &owned
        }
    };
    let pairs: Vec<(Item<'_>, Item<'_>)> =
        list.iter().map(unpack_pair).collect::<Result<_, _>>()?;
    // `counts[c] = counts.get(c, 0) + 1` for every `int` (a `bool` is one): equal keys are one entry and the
    // first spelling stays (`True` and `1`).
    let mut counts: Vec<(CountKey, &PyValue, usize)> = Vec::new();
    let mut position: Option<HashMap<CountKey, usize>> = None;
    for (count, _) in &pairs {
        let count = count.get();
        let Some(key) = CountKey::of(count) else {
            continue;
        };
        let found = match &position {
            Some(table) => table.get(&key).copied(),
            None => counts.iter().position(|entry| entry.0 == key),
        };
        match found {
            Some(at) => counts[at].2 += 1,
            None => {
                let at = counts.len();
                if let Some(table) = position.as_mut() {
                    table.insert(key.clone(), at);
                }
                counts.push((key, count, 1));
                if position.is_none() && counts.len() > SCAN_LIMIT {
                    position = Some(
                        counts
                            .iter()
                            .enumerate()
                            .map(|(at, entry)| (entry.0.clone(), at))
                            .collect(),
                    );
                }
            }
        }
    }
    if counts.is_empty() {
        return Ok(TargetRows {
            target: PyValue::None,
            pool: Vec::new(),
        });
    }
    // `max(sorted(counts), key=counts.get)`: the first of the largest counts in ascending key order.
    counts.sort_by(|left, right| left.0.order(&right.0));
    let mut best = 0;
    for (index, entry) in counts.iter().enumerate() {
        if entry.2 > counts[best].2 {
            best = index;
        }
    }
    let target = counts[best].1.peel().clone();
    let distance = match subtract(&target, input_len.peel())? {
        PyValue::Int(number) => PyValue::Int(num_traits::Signed::abs(&number)),
        PyValue::Float(number) => PyValue::Float(number.abs()),
        other => other,
    };
    if compare(Compare::Greater, &distance, &PyValue::int(TARGET_TOLERANCE))? {
        return Ok(TargetRows {
            target,
            pool: Vec::new(),
        });
    }
    let pool = pairs
        .iter()
        .filter(|(count, _)| py_eq(count.get(), &target))
        .map(|(_, line)| line.get().clone())
        .collect();
    Ok(TargetRows { target, pool })
}

// ── line reading ────────────────────────────────────────────────────────────────

/// `TextIOWrapper._CHUNK_SIZE`: the raw reads a line-by-line iteration decodes one after another.
const READ_CHUNK: usize = 8192;

/// The lines `for line in path.open(encoding="utf-8")` yields, and the error that ended the iteration early.
///
/// Iterating decodes the file one `_CHUNK_SIZE` read at a time (not all at once as `read()` does), so a bad byte
/// loses only its own chunk: complete lines before it are still produced, and the error's position counts from the
/// start of the pending bytes of that chunk. Universal newlines turn `\r\n` and `\r` into `\n`; a `\r` that ends a
/// chunk waits for the next chunk.
pub struct TextLines {
    pub lines: Vec<PyStr>,
    pub failure: Option<Failure>,
}

/// `path.open(encoding="utf-8")` read to its end; the open itself can fail (`FileNotFoundError`, ...).
pub fn text_lines(path: &PyStr) -> Result<TextLines, Failure> {
    let bytes = pyfs::read_bytes(path)?;
    Ok(decode_lines(&bytes))
}

pub(crate) fn decode_lines(bytes: &[u8]) -> TextLines {
    let newline = u32::from('\n');
    let carriage_return = u32::from('\r');
    let mut lines: Vec<PyStr> = Vec::new();
    let mut current: Vec<u32> = Vec::new();
    let mut pending: Vec<u8> = Vec::new();
    let mut held_return = false;
    let push_text =
        |text: &str, current: &mut Vec<u32>, lines: &mut Vec<PyStr>, held: &mut bool| {
            for character in text.chars() {
                let point = u32::from(character);
                if *held {
                    *held = false;
                    current.push(newline);
                    lines.push(PyStr::from_code_points(std::mem::take(current)));
                    if point == newline {
                        continue;
                    }
                }
                if point == carriage_return {
                    *held = true;
                } else if point == newline {
                    current.push(newline);
                    lines.push(PyStr::from_code_points(std::mem::take(current)));
                } else {
                    current.push(point);
                }
            }
        };
    for chunk in bytes.chunks(READ_CHUNK) {
        let mut buffer = std::mem::take(&mut pending);
        buffer.extend_from_slice(chunk);
        let valid_to = match std::str::from_utf8(&buffer) {
            Ok(_) => buffer.len(),
            Err(error) if error.error_len().is_none() => error.valid_up_to(),
            Err(error) => {
                return TextLines {
                    lines,
                    failure: Some(Failure::new(
                        "UnicodeDecodeError",
                        &crate::pyjson::utf8_error_text(&buffer, &error),
                    )),
                };
            }
        };
        let text = std::str::from_utf8(&buffer[..valid_to]).expect("validated prefix");
        push_text(text, &mut current, &mut lines, &mut held_return);
        pending = buffer[valid_to..].to_vec();
    }
    if !pending.is_empty() {
        let error = std::str::from_utf8(&pending).expect_err("an incomplete sequence");
        return TextLines {
            lines,
            failure: Some(Failure::new(
                "UnicodeDecodeError",
                &crate::pyjson::utf8_error_text(&pending, &error),
            )),
        };
    }
    if held_return {
        current.push(newline);
        lines.push(PyStr::from_code_points(std::mem::take(&mut current)));
    }
    if !current.is_empty() {
        lines.push(PyStr::from_code_points(current));
    }
    TextLines {
        lines,
        failure: None,
    }
}

/// `line.endswith("\n")`.
pub(crate) fn ends_with_newline(line: &PyStr) -> bool {
    line.ends_with_code_point(u32::from('\n'))
}
