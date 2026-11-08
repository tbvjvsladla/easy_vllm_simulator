//! Pieces the judge, lite and sweep-meta verification programs share: the printed check list, and the few Python
//! evaluations their conditions are written in (`dict.get`, `or`, `str()`, `len()`, ordering, `startswith`).
//!
//! The programs print one line per check, `  [PASS] name` or `  [FAIL] name <detail>`, and keep the failed names for
//! the closing summary. They differ in how a failure's detail is shown ([`DetailStyle`]).

use crate::conformance::{py_equals, subscript, Raised};
use crate::dataset_prep::Console;
use crate::pystr::{is_python_space, PyStr};
use crate::pyvalue::PyValue;
use crate::verify_fixture::{get, or_default, tail};
use num_traits::ToPrimitive;

/// How the detail of a failed check follows its name.
#[derive(Clone, Copy)]
pub enum DetailStyle {
    /// `name detail`: the judge program, whole text.
    Plain,
    /// `name — detail[-900:]`: the lite programs, last 900 characters of `str(detail)`.
    TailedWithDash,
    /// `name — detail[-N:]`: the repeat sweep program, last `N` (700) characters of `str(detail)`.
    TailedWithDashOf(usize),
}

/// The `check(name, cond, detail)` of the programs and the failure summary data.
pub struct BracketChecks<'a> {
    console: &'a mut dyn Console,
    failures: Vec<String>,
    style: DetailStyle,
}

impl<'a> BracketChecks<'a> {
    pub fn new(console: &'a mut dyn Console, style: DetailStyle) -> Self {
        BracketChecks {
            console,
            failures: Vec::new(),
            style,
        }
    }

    /// The console, for the calls that replay an owner's output on the same streams.
    pub fn console(&mut self) -> &mut dyn Console {
        &mut *self.console
    }

    /// One check. As in the original `check(name, cond, detail)` call, the detail is an argument: it is evaluated
    /// right after the condition whether or not the check passes, and a failure while building it propagates. Only a
    /// failed check shows it.
    pub fn check<D: Detail>(
        &mut self,
        name: &str,
        condition: bool,
        detail: impl FnOnce() -> D,
    ) -> Result<(), Raised> {
        let detail = detail().into_text()?;
        let line = if condition {
            format!("  [PASS] {name}")
        } else {
            let shown = match self.style {
                DetailStyle::Plain => format!(" {detail}"),
                DetailStyle::TailedWithDash => format!(" — {}", tail(&detail, 900)),
                DetailStyle::TailedWithDashOf(count) => format!(" — {}", tail(&detail, count)),
            };
            self.failures.push(name.to_string());
            format!("  [FAIL] {name}{shown}")
        };
        self.print(&line)
    }

    pub fn failures(&self) -> &[String] {
        &self.failures
    }

    pub fn print(&mut self, line: &str) -> Result<(), Raised> {
        self.console
            .print(&crate::pystr::PyStr::new(line))
            .map_err(Raised::from_failure)
    }

    /// `sys.stderr.write(text)` / `print(text, file=sys.stderr)`: `text` carries its own newline.
    pub fn write_error(&mut self, text: &str) {
        self.console.error(&crate::pystr::PyStr::new(text));
    }

    /// `"[...] FAIL %d 건: %s" % (len(failures), failures)`: the list as Python shows it.
    pub fn failure_list_repr(&self) -> String {
        PyValue::List(
            self.failures
                .iter()
                .map(|name| PyValue::str(name))
                .collect(),
        )
        .repr()
    }
}

/// What a detail expression can produce: its text, or the exception raised while evaluating it.
pub trait Detail {
    fn into_text(self) -> Result<String, Raised>;
}

impl Detail for String {
    fn into_text(self) -> Result<String, Raised> {
        Ok(self)
    }
}

impl Detail for Result<String, Raised> {
    fn into_text(self) -> Result<String, Raised> {
        self
    }
}

// ── Python evaluations ───────────────────────────────────────────────────────────

pub fn st(text: &str) -> PyValue {
    PyValue::str(text)
}

pub fn int(number: i64) -> PyValue {
    PyValue::int(number)
}

pub fn float(number: f64) -> PyValue {
    PyValue::Float(number)
}

pub fn tuple(items: Vec<PyValue>) -> PyValue {
    PyValue::Tuple(items)
}

/// `value == literal`.
pub fn eq(value: &PyValue, literal: &PyValue) -> bool {
    py_equals(value, literal)
}

/// `value.get(key) == literal`.
pub fn get_eq(value: &PyValue, key: &str, literal: &PyValue) -> Result<bool, Raised> {
    Ok(py_equals(&get(value, key)?, literal))
}

/// `value.get(key) == "text"`.
pub fn get_eq_text(value: &PyValue, key: &str, text: &str) -> Result<bool, Raised> {
    get_eq(value, key, &st(text))
}

/// `value is None`.
pub fn is_none(value: &PyValue) -> bool {
    matches!(value.peel(), PyValue::None)
}

/// `(value.get(key) or {})`.
pub fn get_or_empty(value: &PyValue, key: &str) -> Result<PyValue, Raised> {
    Ok(or_default(
        get(value, key)?,
        PyValue::Dict(crate::pyvalue::PyDict::new()),
    ))
}

/// `value["a"]["b"]...`.
pub fn chain(value: &PyValue, keys: &[&str]) -> Result<PyValue, Raised> {
    let mut current = value.clone();
    for key in keys {
        current = subscript(&current, key)?;
    }
    Ok(current)
}

/// `value.startswith(prefix)` for a value that must be a string.
pub fn starts_with(value: &PyValue, prefix: &str) -> Result<bool, Raised> {
    match value.peel() {
        PyValue::Str(text) => Ok(text
            .to_utf8()
            .is_some_and(|plain| plain.starts_with(prefix))),
        other => Err(Raised::new(
            "AttributeError",
            &format!(
                "'{}' object has no attribute 'startswith'",
                other.type_name()
            ),
        )),
    }
}

/// `value > 0` for a loaded JSON value.
pub fn greater_than_zero(value: &PyValue) -> Result<bool, Raised> {
    match value.peel() {
        PyValue::Int(number) => Ok(number.to_f64().is_none_or(|plain| plain > 0.0)),
        PyValue::Float(number) => Ok(*number > 0.0),
        PyValue::Bool(flag) => Ok(*flag),
        other => Err(Raised::new(
            "TypeError",
            &format!(
                "'>' not supported between instances of '{}' and 'int'",
                other.type_name()
            ),
        )),
    }
}

/// `len(value)` for the kinds a loaded JSON value can have.
pub fn py_len(value: &PyValue) -> Result<usize, Raised> {
    match value.peel() {
        PyValue::Str(text) => Ok(text.len()),
        PyValue::List(items) | PyValue::Tuple(items) => Ok(items.len()),
        PyValue::Dict(map) => Ok(map.len()),
        PyValue::Bytes(bytes) => Ok(bytes.len()),
        other => Err(Raised::new(
            "TypeError",
            &format!("object of type '{}' has no len()", other.type_name()),
        )),
    }
}

/// `str(value)`.
pub fn py_str(value: &PyValue) -> String {
    value.display_text().to_utf8().unwrap_or_default()
}

/// `value and build()`: the value itself when it is falsy, else what `build` gives; an exception of `build`
/// propagates (a detail expression that raises is the program's exception).
pub fn and_then_value(
    value: &PyValue,
    build: impl FnOnce() -> Result<PyValue, Raised>,
) -> Result<PyValue, Raised> {
    if value.is_truthy() {
        build()
    } else {
        Ok(value.clone())
    }
}

/// `text.strip()` with Python's white space set (not Rust's `trim`).
pub fn py_strip(text: &str) -> String {
    PyStr::new(text).strip().to_utf8().unwrap_or_default()
}

/// `text.lstrip()` with Python's white space set.
pub fn py_lstrip(text: &str) -> String {
    text.trim_start_matches(|character: char| is_python_space(u32::from(character)))
        .to_string()
}

/// A detail that is the `str()` of a tuple.
pub fn tuple_text(items: Vec<PyValue>) -> String {
    tuple(items).repr()
}

/// `dict(name: value, ...)` for details: a dict literal.
pub fn dict_of(entries: Vec<(&str, PyValue)>) -> PyValue {
    crate::verify_fixture::object(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataset_prep::Recorder;

    #[test]
    fn strip_uses_the_python_white_space_set() {
        // U+001C..U+001F are white space to Python and not to Rust's `trim`.
        assert_eq!(py_strip("\u{1c}x\u{1f} \u{2003}"), "x");
        assert_eq!("\u{1c}x".trim(), "\u{1c}x");
        assert_eq!(py_lstrip("\u{1d}\u{a0}bash x "), "bash x ");
        assert_eq!(py_strip(""), "");
    }

    #[test]
    fn a_detail_is_evaluated_for_a_passing_check_and_its_failure_propagates() {
        let mut console = Recorder::default();
        let mut checks = BracketChecks::new(&mut console, DetailStyle::Plain);
        let mut evaluated = false;
        checks
            .check("passes", true, || {
                evaluated = true;
                String::from("never shown")
            })
            .expect("a passing check prints its line");
        assert!(
            evaluated,
            "the detail is an argument: evaluated whether or not the check passes"
        );
        let raised = checks
            .check("passes too", true, || -> Result<String, Raised> {
                Err(Raised::new("KeyError", "'rubric'"))
            })
            .expect_err("an exception of the detail is the program's exception");
        assert_eq!(raised.failure().class, "KeyError");
        drop(checks);
        assert_eq!(console.stdout.to_utf8().expect("text"), "  [PASS] passes\n");
    }

    #[test]
    fn only_a_failed_check_shows_its_detail_in_the_style_of_its_program() {
        let mut console = Recorder::default();
        let mut checks = BracketChecks::new(&mut console, DetailStyle::TailedWithDash);
        checks
            .check("bad", false, || "x".repeat(1000))
            .expect("printed");
        assert_eq!(checks.failures(), ["bad"]);
        drop(checks);
        let line = console.stdout.to_utf8().expect("text");
        assert_eq!(line, format!("  [FAIL] bad — {}\n", "x".repeat(900)));
    }

    #[test]
    fn a_failed_check_keeps_the_tail_length_its_program_names() {
        let mut console = Recorder::default();
        let mut checks = BracketChecks::new(&mut console, DetailStyle::TailedWithDashOf(700));
        checks
            .check("bad", false, || {
                format!("{}{}", "a".repeat(300), "b".repeat(700))
            })
            .expect("printed");
        drop(checks);
        let line = console.stdout.to_utf8().expect("text");
        assert_eq!(line, format!("  [FAIL] bad — {}\n", "b".repeat(700)));
    }

    #[test]
    fn a_console_write_adds_no_newline() {
        let mut console = Recorder::default();
        console.write(&PyStr::new("partial")).expect("written");
        console.write(&PyStr::new("-tail")).expect("written");
        assert_eq!(console.stdout.to_utf8().expect("text"), "partial-tail");
    }
}
