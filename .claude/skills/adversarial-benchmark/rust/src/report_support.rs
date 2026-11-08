//! Shared building blocks of the report, certificate, map and naming ports: the value formatters the renderers
//! use (`na`, `fnum`, `scalar`, table cells), the small text readers (`_read_text`, the two key lookups, the
//! manifest path labels), the JSON loader that ends the program with status 2, and the repository-root walk.
//!
//! All of it mirrors Python on dynamically typed [`PyValue`]s: a wrong-typed argument fails with the exception
//! CPython raises, and text read from files keeps universal-newline translation and the decoder's error handling.

use crate::cli::os_error_text;
use crate::guidellm::Failure;
use crate::ospath;
use crate::pyformat::{float_remainder, percent_float};
use crate::pyfs;
use crate::pynumber::float_call;
use crate::pyops::num_of;
use crate::pystr::{is_decimal_digit, is_python_space, lit, PyStr};
use crate::pyvalue::PyValue;
use crate::repeat_api_cli::load_failure;
use crate::unicode_digit_tables::DIGIT_ONLY;

pub type Res<T> = Result<T, Failure>;

pub fn py(text: &str) -> PyStr {
    PyStr::new(text)
}

/// `str(value)`.
pub fn display(value: &PyValue) -> PyStr {
    value.display_text()
}

/// `SystemExit(code)` as the failure the API transport reports (`type` `SystemExit`, `message` the code).
pub fn system_exit(code: i32) -> Failure {
    Failure::new("SystemExit", &code.to_string())
}

/// What a program writes to `sys.stderr` while it runs. Kept apart from stdout so a failing call still reports
/// everything written before it failed.
#[derive(Default, Clone)]
pub struct Diagnostics {
    text: PyStr,
}

impl Diagnostics {
    pub fn write(&mut self, text: &PyStr) {
        self.text.push_py(text);
    }

    pub fn write_str(&mut self, text: &str) {
        self.text.push_str(text);
    }

    pub fn text(&self) -> &PyStr {
        &self.text
    }

    pub fn into_text(self) -> PyStr {
        self.text
    }
}

// ── whitespace and digit predicates ───────────────────────────────────────────

fn is_digit_char(code_point: u32) -> bool {
    is_decimal_digit(code_point)
        || DIGIT_ONLY
            .binary_search_by(|&(first, last)| {
                if code_point < first {
                    std::cmp::Ordering::Greater
                } else if code_point > last {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .is_ok()
}

/// `text.strip().upper()` is one of the given upper-case ASCII spellings (no non-ASCII character upper-cases to an
/// ASCII letter or `/`, so comparing the ASCII-folded form is exact). Nothing is allocated.
fn stripped_upper_is(text: &PyStr, spellings: &[&str]) -> bool {
    let points = text.code_points();
    let start = points
        .iter()
        .position(|&p| !is_python_space(p))
        .unwrap_or(points.len());
    let end = points
        .iter()
        .rposition(|&p| !is_python_space(p))
        .map_or(start, |last| last + 1);
    let core = &points[start..end];
    spellings.iter().any(|spelling| {
        spelling.len() == core.len()
            && spelling
                .bytes()
                .zip(core)
                .all(|(expected, &actual)| fold_ascii_upper(actual) == u32::from(expected))
    })
}

fn fold_ascii_upper(code_point: u32) -> u32 {
    if (u32::from('a')..=u32::from('z')).contains(&code_point) {
        code_point - 32
    } else {
        code_point
    }
}

/// `v is None or (isinstance(v, str) and v.strip().upper() in ("", "NA", "N/A"))`.
fn is_missing(value: &PyValue) -> bool {
    match value.peel() {
        PyValue::None => true,
        PyValue::Str(text) => stripped_upper_is(text, &["", "NA", "N/A"]),
        _ => false,
    }
}

/// `na(v, suffix)` of `render_report`: `N/A` for a missing value, else `"%s%s" % (v, suffix)`.
pub fn na_with_suffix(value: &PyValue, suffix: &PyValue) -> PyStr {
    if is_missing(value) {
        return lit!("N/A");
    }
    let mut text = display(value);
    text.push_py(&display(suffix));
    text
}

/// `na(v)` of `render_report`/`render_max_report` without a suffix: `N/A` or `str(v)`.
pub fn na(value: &PyValue) -> PyStr {
    if is_missing(value) {
        lit!("N/A")
    } else {
        display(value)
    }
}

/// `_cell(v)`: `N/A` or the text with whitespace runs collapsed and `|` escaped, so a table row cannot break.
pub fn table_cell(value: &PyValue) -> PyStr {
    if is_missing(value) {
        return lit!("N/A");
    }
    let shown = display(value);
    let mut out: Vec<u32> = Vec::with_capacity(shown.len());
    let mut first = true;
    for word in shown
        .code_points()
        .split(|&p| is_python_space(p))
        .filter(|word| !word.is_empty())
    {
        if !first {
            out.push(u32::from(' '));
        }
        first = false;
        for &p in word {
            if p == u32::from('|') {
                out.push(u32::from('\\'));
            }
            out.push(p);
        }
    }
    PyStr::from_code_points(out)
}

/// `fnum(v, fmt, suffix)`: `(fmt % float(v)) + suffix`, or `N/A` when that raises `TypeError` or `ValueError`.
/// Any other exception (an integer too large for a float, a zero divisor, ...) is not caught by the original.
pub fn fnum(value: &PyValue, fmt: &PyValue, suffix: &PyValue) -> Res<PyValue> {
    let not_available = || Ok(PyValue::Str(lit!("N/A")));
    let number = match float_call(value.peel()) {
        Ok(number) => number,
        Err(error) if error.is_type_or_value_error() => return not_available(),
        Err(error) => return Err(error.into()),
    };
    let formatted = match fmt.peel() {
        PyValue::Str(layout) => match percent_float(layout.code_points(), number) {
            Ok(points) => PyValue::Str(PyStr::from_code_points(points)),
            Err(failure) if is_type_or_value(&failure) => return not_available(),
            Err(failure) => return Err(failure),
        },
        other => match num_of(other) {
            Some(left) => float_remainder(&left, number)?,
            None => return not_available(),
        },
    };
    match (formatted, suffix.peel()) {
        (PyValue::Str(mut head), PyValue::Str(tail)) => {
            head.push_py(tail);
            Ok(PyValue::Str(head))
        }
        (PyValue::Float(head), tail) => match num_of(tail) {
            Some(addend) => Ok(PyValue::Float(
                head + addend.to_f64().map_err(Failure::from)?,
            )),
            None => not_available(),
        },
        _ => not_available(),
    }
}

fn is_type_or_value(failure: &Failure) -> bool {
    matches!(failure.class, "TypeError" | "ValueError")
}

/// `fnum(v)` with the default format `%.2f` and no suffix, as a string.
pub fn fnum_default(value: &PyValue) -> Res<PyStr> {
    match fnum(
        value,
        &PyValue::str("%.2f"),
        &PyValue::Str(PyStr::default()),
    )? {
        PyValue::Str(text) => Ok(text),
        other => Ok(display(&other)),
    }
}

/// `scalar(v)` of the certificate writer: a flat-YAML scalar. `N/A` for missing, `true`/`false` for booleans, the
/// text otherwise, double-quoted when it holds a character YAML would read as syntax (and is not a plain number).
pub fn yaml_scalar(value: &PyValue) -> PyStr {
    match value.peel() {
        PyValue::None => return lit!("N/A"),
        PyValue::Bool(flag) => return py(if *flag { "true" } else { "false" }),
        _ => {}
    }
    let text = display(value);
    if stripped_upper_is(&text, &["", "NA"]) {
        return lit!("N/A");
    }
    let needs_quote = text
        .code_points()
        .iter()
        .any(|&p| matches!(char::from_u32(p), Some(':' | '#' | ' ' | '\'' | '"')));
    if needs_quote {
        let digits_only: Vec<u32> = text
            .code_points()
            .iter()
            .copied()
            .filter(|&p| p != u32::from('.') && p != u32::from('-'))
            .collect();
        let is_plain_number =
            !digits_only.is_empty() && digits_only.iter().all(|&p| is_digit_char(p));
        if !is_plain_number {
            let mut out = vec![u32::from('"')];
            for &p in text.code_points() {
                if p == u32::from('"') {
                    out.push(u32::from('\\'));
                }
                out.push(p);
            }
            out.push(u32::from('"'));
            return PyStr::from_code_points(out);
        }
    }
    text
}

// ── paths ─────────────────────────────────────────────────────────────────────

/// `os.stat(value)`'s argument check for a value that is not a string: integers are file descriptors (never a
/// directory here), anything else is the `TypeError` `os.path.isdir` lets through.
pub fn stat_path_argument(value: &PyValue) -> Res<()> {
    match value.peel() {
        PyValue::Int(_) | PyValue::Bool(_) => Ok(()),
        other => Err(Failure::new(
            "TypeError",
            &format!(
                "stat: path should be string, bytes, os.PathLike or integer, not {}",
                other.type_name()
            ),
        )),
    }
}

fn current_directory() -> PyStr {
    use std::os::unix::ffi::OsStrExt;
    std::env::current_dir()
        .map(|dir| PyStr::from_surrogateescape(dir.as_os_str().as_bytes()))
        .unwrap_or_default()
}

/// `repo_root(start)`: the nearest ancestor of `abspath(start)` holding a `.git` directory, else the working
/// directory. (The original loops forever on a path whose parent is itself, such as `//x`; this stops and returns
/// the working directory.)
pub fn repo_root(start: &PyValue) -> Res<PyStr> {
    let mut directory = ospath::abspath(&ospath::fspath(start)?)?;
    while !directory.equals("/") {
        if ospath::isdir(&ospath::join_all(&directory, &[".git"])) {
            return Ok(directory);
        }
        let parent = ospath::dirname(&directory);
        if parent == directory {
            break;
        }
        directory = parent;
    }
    Ok(current_directory())
}

// ── reading text ──────────────────────────────────────────────────────────────

/// Universal newlines: `\r\n` and a lone `\r` read as `\n`.
fn universal_newlines(points: Vec<u32>) -> PyStr {
    let mut out = Vec::with_capacity(points.len());
    let mut index = 0;
    while index < points.len() {
        if points[index] == 13 {
            out.push(10);
            if points.get(index + 1) == Some(&10) {
                index += 1;
            }
        } else {
            out.push(points[index]);
        }
        index += 1;
    }
    PyStr::from_code_points(out)
}

/// `open(path, encoding="utf-8", errors="ignore").read()`: undecodable bytes are dropped.
pub fn lossy_text_ignoring_errors(bytes: &[u8]) -> PyStr {
    let mut points = Vec::with_capacity(bytes.len());
    let mut rest = bytes;
    loop {
        match std::str::from_utf8(rest) {
            Ok(text) => {
                points.extend(text.chars().map(u32::from));
                break;
            }
            Err(error) => {
                let (valid, after) = rest.split_at(error.valid_up_to());
                points.extend(
                    std::str::from_utf8(valid)
                        .expect("prefix is valid")
                        .chars()
                        .map(u32::from),
                );
                match error.error_len() {
                    Some(length) => rest = &after[length..],
                    None => break,
                }
            }
        }
    }
    universal_newlines(points)
}

/// `open(path, encoding="utf-8", errors="replace").read()`: each undecodable run becomes `U+FFFD`.
pub fn lossy_text_replacing_errors(bytes: &[u8]) -> PyStr {
    let text = String::from_utf8_lossy(bytes);
    universal_newlines(text.chars().map(u32::from).collect())
}

/// `_read_text(path)`: the file's text (undecodable bytes replaced), or `""` for anything that is an `OSError` or a
/// `TypeError` (a missing file, a directory, a path that is not text).
pub fn read_text_or_empty(path: &PyValue) -> Res<PyStr> {
    let Ok(path) = ospath::fspath(path) else {
        return Ok(PyStr::default());
    };
    match pyfs::read_bytes(&path) {
        Ok(bytes) => Ok(lossy_text_replacing_errors(&bytes)),
        Err(failure) if failure.class == "ValueError" => Err(failure),
        Err(_) => Ok(PyStr::default()),
    }
}

/// `open(path, encoding="utf-8").read()` with strict decoding: `Ok(None)` for an `OSError`, the decoder's failure
/// otherwise.
pub fn read_text_strict_or_none(path: &PyValue) -> Res<Option<PyStr>> {
    let path = ospath::fspath(path)?;
    match pyfs::read_bytes(&path) {
        Ok(bytes) => match crate::pyjson::decode_file_text(&bytes) {
            Ok(text) => Ok(Some(text)),
            Err(error) => Err(load_failure(&error)),
        },
        Err(failure) if failure.class == "ValueError" => Err(failure),
        Err(_) => Ok(None),
    }
}

/// `load(path, label)` of the three renderers: the JSON document of a file, or — for any exception — a line
/// `"<tag> ERROR <label>: <reason>"` on stderr and `SystemExit(2)`.
pub fn load_json_or_exit(
    path: &PyValue,
    tag: &str,
    label: &PyValue,
    diagnostics: &mut Diagnostics,
) -> Res<PyValue> {
    let reason: PyStr = match ospath::fspath(path) {
        Err(failure) => failure.message,
        Ok(path) => match pyfs::read_bytes(&path) {
            Err(failure) => {
                if failure.class == "ValueError" {
                    failure.message
                } else {
                    // the same text `str(OSError)` gives
                    match crate::pyfs::os_string(&path) {
                        Ok(os) => match std::fs::read(std::path::Path::new(&os)) {
                            Err(error) => py(&os_error_text(&error, &path)),
                            Ok(_) => failure.message,
                        },
                        Err(_) => failure.message,
                    }
                }
            }
            Ok(bytes) => match crate::pyjson::decode_file_text(&bytes) {
                Err(error) => load_failure(&error).message,
                Ok(text) => match crate::pyjson::loads_text(&text) {
                    Ok(document) => return Ok(document),
                    Err(error) => load_failure(&error).message,
                },
            },
        },
    };
    let mut line = py(tag);
    line.push_str(" ERROR ");
    line.push_py(&display(label));
    line.push_str(": ");
    line.push_py(&reason);
    line.push_str("\n");
    diagnostics.write(&line);
    Err(system_exit(2))
}

// ── key lookups in YAML-ish and env-ish text ──────────────────────────────────

fn at_line_start(text: &[u32], position: usize) -> bool {
    position == 0 || text[position - 1] == 10
}

fn skip_space(text: &[u32], mut position: usize) -> usize {
    while position < text.len() && is_python_space(text[position]) {
        position += 1;
    }
    position
}

fn strip_quote_chars(text: &[u32], quote: u32) -> &[u32] {
    let start = text.iter().position(|&p| p != quote).unwrap_or(text.len());
    let end = text
        .iter()
        .rposition(|&p| p != quote)
        .map_or(start, |i| i + 1);
    &text[start..end.max(start)]
}

/// The value text with the surrounding blanks and one pass each of `"` and `'` stripped, as
/// `.strip().strip('"').strip("'")` leaves it.
fn unquoted(points: &[u32]) -> PyStr {
    let stripped = PyStr::from_code_points(points.to_vec()).strip();
    let inner = strip_quote_chars(stripped.code_points(), u32::from('"'));
    PyStr::from_code_points(strip_quote_chars(inner, u32::from('\'')).to_vec())
}

/// Where `^\s*KEY\s*SEP\s*` ends for the first line start that has it, with the whitespace-run limits needed to
/// backtrack: `(value_start_max, value_start_min)`.
fn after_key_and_separator(
    text: &[u32],
    start: usize,
    key: &[u32],
    separator: char,
) -> Option<(usize, usize)> {
    let key_at = skip_space(text, start);
    if !text[key_at..].starts_with(key) {
        return None;
    }
    let separator_at = skip_space(text, key_at + key.len());
    if text.get(separator_at) != Some(&u32::from(separator)) {
        return None;
    }
    let after = separator_at + 1;
    Some((skip_space(text, after), after))
}

/// `_grep_yaml(text, key)`: the first `KEY: value` (value = what is left of `#` on the line, at least one
/// character) as stripped text, or `None`. Whitespace between the colon and the value may include line breaks, as
/// the original's `\s*` does.
pub fn grep_yaml(text: &PyStr, key: &PyStr) -> Option<PyStr> {
    let text = text.code_points();
    for start in (0..=text.len()).filter(|&p| at_line_start(text, p)) {
        let Some((value_max, value_min)) =
            after_key_and_separator(text, start, key.code_points(), ':')
        else {
            continue;
        };
        let is_value_char = |p: u32| p != 10 && p != u32::from('#');
        let value_start = if value_max < text.len() && is_value_char(text[value_max]) {
            Some(value_max)
        } else {
            (value_min..value_max).rev().find(|&at| text[at] != 10)
        };
        if let Some(at) = value_start {
            let end = text[at..]
                .iter()
                .position(|&p| !is_value_char(p))
                .map_or(text.len(), |offset| at + offset);
            return Some(unquoted(&text[at..end]));
        }
    }
    None
}

/// `_grep_env(text, key)`: the first `KEY=value` (value possibly empty) as stripped text, `None` when absent or
/// empty after stripping.
pub fn grep_env(text: &PyStr, key: &PyStr) -> Option<PyStr> {
    let text = text.code_points();
    for start in (0..=text.len()).filter(|&p| at_line_start(text, p)) {
        let Some((value_start, _)) = after_key_and_separator(text, start, key.code_points(), '=')
        else {
            continue;
        };
        let end = text[value_start..]
            .iter()
            .position(|&p| p == 10 || p == u32::from('#'))
            .map_or(text.len(), |offset| value_start + offset);
        let value = unquoted(&text[value_start..end]);
        return if value.is_empty() { None } else { Some(value) };
    }
    None
}

/// `manifest_path_labels(manifest_path)`: for each top-level `<name>_path: /absolute/value` of a manifest, the
/// pair `(value without trailing slashes, "<manifest.<name>>")`, longest value first. A manifest that cannot be
/// opened gives no labels; one that is not UTF-8 is the decoder's failure.
pub fn manifest_path_labels(manifest_path: &PyValue) -> Res<Vec<(PyStr, PyStr)>> {
    let Some(text) = read_text_strict_or_none(manifest_path)? else {
        return Ok(Vec::new());
    };
    let text = text.code_points();
    let mut found: Vec<(PyStr, PyStr)> = Vec::new();
    let mut position = 0;
    while position <= text.len() {
        if !at_line_start(text, position) {
            position += 1;
            continue;
        }
        match label_match_at(text, position) {
            Some((name, value, end)) => {
                let mut prefix = value.to_vec();
                while prefix.last() == Some(&u32::from('/')) {
                    prefix.pop();
                }
                let mut label = lit!("<manifest.");
                label.push_py(&PyStr::from_code_points(name.to_vec()));
                label.push_str(">");
                found.push((PyStr::from_code_points(prefix), label));
                position = end;
            }
            None => position += 1,
        }
    }
    found.sort_by_key(|(prefix, _)| std::cmp::Reverse(prefix.len()));
    Ok(found)
}

/// `^([a-z_]+_path):\s*"?(/[^"#\s]+)` at a line start: (name, value, end).
fn label_match_at(text: &[u32], start: usize) -> Option<(&[u32], &[u32], usize)> {
    let is_name_char =
        |p: u32| p == u32::from('_') || (u32::from('a')..=u32::from('z')).contains(&p);
    let name_end = start
        + text[start..]
            .iter()
            .take_while(|&&p| is_name_char(p))
            .count();
    let name = &text[start..name_end];
    let suffix: Vec<u32> = "_path".chars().map(u32::from).collect();
    if name.len() <= suffix.len()
        || !name.ends_with(&suffix)
        || text.get(name_end) != Some(&u32::from(':'))
    {
        return None;
    }
    let mut at = skip_space(text, name_end + 1);
    if text.get(at) == Some(&u32::from('"')) {
        at += 1;
    }
    if text.get(at) != Some(&u32::from('/')) {
        return None;
    }
    let is_value_char = |p: u32| p != u32::from('"') && p != u32::from('#') && !is_python_space(p);
    let run = text[at + 1..]
        .iter()
        .take_while(|&&p| is_value_char(p))
        .count();
    if run == 0 {
        return None;
    }
    let end = at + 1 + run;
    Some((name, &text[at..end], end))
}
