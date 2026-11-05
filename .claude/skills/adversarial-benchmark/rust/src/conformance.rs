//! Core of the `full ⊇ lite` conformance checker: a Rust port of
//! `.claude/skills/adversarial-benchmark/scripts/conformance_full_superset_lite.py`.
//!
//! The original is a list of static source checks plus a few checks that run the installed Rust owners. This port keeps
//! that shape: [`crate::conformance_checks::main`] is the check list, [`crate::conformance_native`] holds the two
//! helpers that call the installed launchers (`native_value`, `native_lite_certificate_gates`) and the transport to
//! the lite metrics owner, and this file holds what they share: where the scripts directory is ([`Context`]), the
//! two file readers of the original ([`read`], [`read_optional_strict`]), the report printer ([`emit`]) and the few
//! Python behaviours the checks lean on (equality, indexing, `str.find` positions).
//!
//! Failures are [`Raised`] values: the class and message that the transport reports, plus the `repr()` the original
//! prints when it catches an exception and writes `repr(exc)` into a check's detail.

use crate::dataset_prep::Console;
use crate::guidellm::Failure;
use crate::pyjson::{decode_file_text, LoadError};
use crate::pystr::PyStr;
use crate::pyvalue::{bytes_repr, PyDict, PyValue};
use num_bigint::BigInt;
use num_traits::Zero;
use std::path::Path;

/// The one environment variable that selects the scripts directory (`SDIR` of the original).
pub const SCRIPTS_DIR_ENV: &str = "CONFORMANCE_SCRIPTS_DIR";

/// The five metrics lite shows a person (`references/lite-and-publication.md` §1). The checks below fail on a full
/// artifact that lacks one, so this list is the single place that grows with lite.
pub const LITE_METRIC_KEYS: [&str; 5] = [
    "gen_tps",
    "cold_ttft_ms",
    "kv_gib",
    "capacity",
    "accept_len",
];

/// The lite fields a published certificate must carry.
pub const CERT_LITE_FIELDS: [&str; 7] = [
    "lite_included",
    "lite_gen_tps_warm",
    "lite_cold_ttft_ms",
    "lite_kv_gib",
    "lite_gpu_occupancy",
    "lite_ram_occupancy",
    "lite_accept_len",
];

// ── context ─────────────────────────────────────────────────────────────────────

/// Where the checker looks: the directory of the installed launcher scripts (`SDIR`), as an absolute, normalised
/// path (`os.path.dirname(os.path.abspath(__file__))` in the original), and the module state of the checker run
/// that uses it.
///
/// The module state ([`ModuleCache`]) stands for the original's `sys.modules`: the lite metrics module is imported
/// once per process, and a second `main()` in the same process reuses it. A `Context` and every clone of it share one
/// state; a separately built `Context` starts with an empty one. (The interpreter keys the cache by module name, not by
/// directory; that identity across different scripts directories in one process is not modelled.)
#[derive(Clone, Debug)]
pub struct Context {
    scripts_dir: PyStr,
    modules: std::sync::Arc<crate::conformance_native::ModuleCache>,
}

impl Context {
    /// The context for a scripts directory; a relative path is made absolute against the current directory.
    pub fn from_path(scripts_dir: &PyStr) -> Result<Context, Failure> {
        Ok(Context {
            scripts_dir: crate::ospath::abspath(scripts_dir)?,
            modules: Default::default(),
        })
    }

    /// The directory named by `CONFORMANCE_SCRIPTS_DIR`, when it is set and not empty.
    pub fn from_environment() -> Option<Context> {
        use std::os::unix::ffi::OsStrExt;
        let value = std::env::var_os(SCRIPTS_DIR_ENV)?;
        if value.is_empty() {
            return None;
        }
        Context::from_path(&PyStr::from_surrogateescape(value.as_bytes())).ok()
    }

    /// The scripts directory of the installation a release binary sits in: a binary at
    /// `<skill>/rust/target/release/<name>` belongs to `<skill>/scripts`. A binary anywhere else (a copy, a test
    /// executable) names no installation.
    pub fn from_installed_binary(executable: &Path) -> Option<Context> {
        use std::os::unix::ffi::OsStrExt;
        let release = executable.parent()?;
        let target = release.parent()?;
        let rust = target.parent()?;
        let skill = rust.parent()?;
        let named = |directory: &Path, name: &str| directory.file_name().is_some_and(|n| n == name);
        if !(named(release, "release") && named(target, "target") && named(rust, "rust")) {
            return None;
        }
        let scripts = skill.join("scripts");
        Context::from_path(&PyStr::from_surrogateescape(scripts.as_os_str().as_bytes())).ok()
    }

    /// The explicit environment first, then the installation of the running binary.
    pub fn resolve() -> Option<Context> {
        Context::from_environment().or_else(|| {
            let executable = std::env::current_exe().ok()?;
            Context::from_installed_binary(&executable)
        })
    }

    /// The imported-module state of this context (see the type's description).
    pub fn modules(&self) -> &crate::conformance_native::ModuleCache {
        &self.modules
    }

    /// `SDIR`.
    pub fn scripts_dir(&self) -> &PyStr {
        &self.scripts_dir
    }

    /// `os.path.join(SDIR, name)`.
    pub fn join(&self, name: &PyStr) -> PyStr {
        crate::ospath::join(&self.scripts_dir, &[name])
    }

    /// `os.path.dirname(SDIR)`: the skill directory (the checker finds `fixtures/` and `rust/src/` there).
    pub fn skill_dir(&self) -> PyStr {
        crate::ospath::dirname(&self.scripts_dir)
    }

    /// `os.path.dirname(os.path.dirname(SDIR))`: the directory holding all skills.
    pub fn skills_dir(&self) -> PyStr {
        crate::ospath::dirname(&self.skill_dir())
    }
}

// ── raised exceptions ───────────────────────────────────────────────────────────

/// A Python exception raised by a step of the original: what a transport reports (class and message) and the
/// `repr()` that `except Exception as exc: ... repr(exc)` writes into a check detail.
#[derive(Clone, Debug)]
pub struct Raised {
    failure: Failure,
    repr: String,
    /// The name a traceback prints for the class when it is not the bare `__name__`: `module.qualname` of an
    /// exception that came from another module (`None` = the bare class name; see [`Raised::qualified_class`]).
    qualified: Option<String>,
}

impl Raised {
    pub fn new(class: &'static str, message: &str) -> Raised {
        Raised::from_failure(Failure::new(class, message))
    }

    /// An exception built from one message argument: `Class('message')`.
    pub fn from_failure(failure: Failure) -> Raised {
        let repr = format!("{}({})", failure.class, failure.message.repr());
        Raised {
            failure,
            repr,
            qualified: None,
        }
    }

    /// An exception whose `repr()` is not `Class(message)` (several arguments, or none).
    pub fn with_repr(failure: Failure, repr: String) -> Raised {
        Raised {
            failure,
            repr,
            qualified: None,
        }
    }

    /// An `OSError` of the file `path`: its message is `[Errno N] text: 'path'` and its arguments are
    /// `(errno, text)` (the file name is an attribute, not an argument).
    pub fn os_error(error: &std::io::Error, path: &PyStr) -> Raised {
        let failure = Failure::new(
            crate::pyfs::os_error_class(error),
            &crate::cli::os_error_text(error, path),
        );
        let detail = error.to_string();
        let text = detail.split(" (os error").next().unwrap_or(&detail);
        let repr = match error.raw_os_error() {
            Some(errno) => format!("{}({errno}, {})", failure.class, PyStr::new(text).repr()),
            None => format!("{}({})", failure.class, PyStr::new(text).repr()),
        };
        Raised {
            failure,
            repr,
            qualified: None,
        }
    }

    /// The failure of decoding `bytes` (a stream or a file): a UTF-8 error carries its five arguments (codec,
    /// the bytes, start, end, reason) in its `repr()`; JSON errors are their class and message.
    pub fn decoding(bytes: &[u8], error: &LoadError) -> Raised {
        let failure = crate::repeat_api_cli::load_failure(error);
        match (error, std::str::from_utf8(bytes)) {
            (LoadError::Encoding(_), Err(utf8)) => {
                let start = utf8.valid_up_to();
                let (length, reason) = match utf8.error_len() {
                    None => (bytes.len() - start, "unexpected end of data"),
                    Some(length) if (0xC2..=0xF4).contains(&bytes[start]) => {
                        (length, "invalid continuation byte")
                    }
                    Some(length) => (length, "invalid start byte"),
                };
                let repr = format!(
                    "UnicodeDecodeError('utf-8', {}, {start}, {}, '{reason}')",
                    bytes_repr(bytes),
                    start + length
                );
                Raised {
                    failure,
                    repr,
                    qualified: None,
                }
            }
            _ => Raised::from_failure(failure),
        }
    }

    pub fn failure(&self) -> &Failure {
        &self.failure
    }

    /// Records the class name as a traceback prints it (`module.qualname`, bare for `builtins` and `__main__`),
    /// for an exception raised by a Python owner. `failure().class` stays the bare `__name__`.
    pub fn with_qualified_class(mut self, name: String) -> Raised {
        self.qualified = Some(name);
        self
    }

    /// The class name a traceback prints when it differs from the bare one.
    pub fn qualified_class(&self) -> Option<&str> {
        self.qualified.as_deref()
    }

    /// Whether this is `SystemExit`: the one class a lite metrics record may name that is **not** an `Exception`
    /// subclass, so no `except Exception` of the original catches it. It ends the checker instead of becoming
    /// the detail of a failed check.
    pub fn is_system_exit(&self) -> bool {
        self.failure.class == "SystemExit"
    }

    pub fn into_failure(self) -> Failure {
        self.failure
    }

    /// `repr(exception)`.
    pub fn repr(&self) -> &str {
        &self.repr
    }
}

// ── the two readers of the original ─────────────────────────────────────────────

pub(crate) fn read_file_bytes(path: &PyStr) -> Result<Vec<u8>, Raised> {
    let os = crate::pyfs::os_string(path)
        .map_err(|failure| Raised::from_failure(failure.into_failure()))?;
    std::fs::read(Path::new(&os)).map_err(|error| Raised::os_error(&error, path))
}

/// Universal newlines: `\r\n` and a lone `\r` read as `\n`.
fn translate_newlines(text: &str) -> String {
    if !text.contains('\r') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        if character == '\r' {
            if characters.peek() == Some(&'\n') {
                characters.next();
            }
            out.push('\n');
        } else {
            out.push(character);
        }
    }
    out
}

/// `read(name)`: the text of `SDIR/name` (UTF-8 with undecodable bytes replaced, universal newlines), or `None`
/// when it is not a regular file. Reading a file that exists but cannot be read is an error (`OSError`).
pub fn read(context: &Context, name: &PyStr) -> Result<Option<String>, Raised> {
    let path = context.join(name);
    if !crate::ospath::isfile(&path) {
        return Ok(None);
    }
    let bytes = read_file_bytes(&path)?;
    Ok(Some(translate_newlines(&String::from_utf8_lossy(&bytes))))
}

/// The optional source reads of the original: `open(path, encoding="utf-8")` with a handler for `OSError` only.
/// A file that cannot be opened or read gives the empty text; undecodable bytes are not an `OSError` and escape
/// as `UnicodeDecodeError`.
pub fn read_optional_strict(path: &PyStr) -> Result<String, Raised> {
    let Ok(bytes) = read_file_bytes(path) else {
        return Ok(String::new());
    };
    match decode_file_text(&bytes) {
        Ok(text) => Ok(text
            .to_utf8()
            .expect("strict decoding yields no surrogates")),
        Err(error) => Err(Raised::decoding(&bytes, &error)),
    }
}

// ── the report ──────────────────────────────────────────────────────────────────

/// What `for name, good, detail in results` iterates: the items of a list or tuple, the characters of a string, the
/// keys of a dict. Anything else is not iterable.
pub fn iterate_values(value: &PyValue) -> Result<Vec<PyValue>, Raised> {
    match value.peel() {
        PyValue::List(items) | PyValue::Tuple(items) => Ok(items.clone()),
        PyValue::Str(text) => Ok(text
            .code_points()
            .iter()
            .map(|&point| PyValue::Str(PyStr::from_code_points(vec![point])))
            .collect()),
        PyValue::Dict(map) => Ok(map
            .iter()
            .map(|(key, _)| PyValue::Str(key.clone()))
            .collect()),
        other => Err(Raised::new(
            "TypeError",
            &format!("'{}' object is not iterable", other.type_name()),
        )),
    }
}

/// `name, good, detail = item`.
fn unpack_triple(item: &PyValue) -> Result<[PyValue; 3], Raised> {
    let parts = match item.peel() {
        PyValue::List(_) | PyValue::Tuple(_) | PyValue::Str(_) | PyValue::Dict(_) => {
            iterate_values(item)?
        }
        other => {
            return Err(Raised::new(
                "TypeError",
                &format!("cannot unpack non-iterable {} object", other.type_name()),
            ))
        }
    };
    match parts.len() {
        3 => {
            let mut parts = parts.into_iter();
            Ok([
                parts.next().expect("three parts"),
                parts.next().expect("three parts"),
                parts.next().expect("three parts"),
            ])
        }
        count if count < 3 => Err(Raised::new(
            "ValueError",
            &format!("not enough values to unpack (expected 3, got {count})"),
        )),
        _ => Err(Raised::new(
            "ValueError",
            "too many values to unpack (expected 3)",
        )),
    }
}

/// `_emit(results)`: one `PASS`/`FAIL` line per `(name, good, detail)` and a summary line; the verdict is whether all
/// were good.
///
/// A line is printed as soon as its row is read, so a malformed later row leaves the earlier lines on the console.
/// The detail of a failed row is appended with `"  — " + detail`: only a string can be, so a failed row whose detail is
/// any other truthy value (the checks hand over a dict for the D1 verdicts) ends the report with a `TypeError`
/// after the lines already printed. That is the original's behaviour and is kept.
pub fn emit(results: &PyValue, console: &mut dyn Console) -> Result<bool, Raised> {
    let mut all_good = true;
    for item in iterate_values(results)? {
        let [name, good, detail] = unpack_triple(&item)?;
        let good = good.is_truthy();
        let mut line = PyStr::new(if good { "PASS " } else { "FAIL " });
        line.push_py(&name.display_text());
        if detail.is_truthy() && !good {
            let PyValue::Str(text) = detail.peel() else {
                return Err(Raised::new(
                    "TypeError",
                    &format!(
                        "can only concatenate str (not \"{}\") to str",
                        detail.type_name()
                    ),
                ));
            };
            line.push_str("  — ");
            line.push_py(text);
        }
        console.print(&line).map_err(Raised::from_failure)?;
        all_good = all_good && good;
    }
    let rows = iterate_values(results)?;
    let passed = rows
        .iter()
        .filter(|row| unpack_triple(row).is_ok_and(|[_, good, _]| good.is_truthy()))
        .count();
    let summary = format!(
        "\nfull ⊇ lite conformance: {passed}/{} {}",
        rows.len(),
        if all_good { "PASS" } else { "FAIL" }
    );
    console
        .print(&PyStr::new(&summary))
        .map_err(Raised::from_failure)?;
    Ok(all_good)
}

// ── Python behaviours the checks lean on ────────────────────────────────────────

/// Python `==` between two JSON-shaped values: `int`, `float` and `bool` compare as numbers (`1 == True`,
/// `6 == 6.0`), a list never equals a tuple, dicts compare without regard to order.
pub fn py_equals(left: &PyValue, right: &PyValue) -> bool {
    match (left.peel(), right.peel()) {
        (PyValue::None, PyValue::None) => true,
        (PyValue::Str(a), PyValue::Str(b)) => a == b,
        (PyValue::List(a), PyValue::List(b)) | (PyValue::Tuple(a), PyValue::Tuple(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| py_equals(x, y))
        }
        (PyValue::Dict(a), PyValue::Dict(b)) => {
            a.len() == b.len() && a.iter().all(|(key, value)| key_equals(b, key, value))
        }
        (PyValue::Bytes(a), PyValue::Bytes(b)) => a == b,
        (a, b) => match (number_of(a), number_of(b)) {
            (Some(x), Some(y)) => numbers_equal(&x, &y),
            _ => false,
        },
    }
}

fn key_equals(other: &PyDict, key: &PyStr, value: &PyValue) -> bool {
    key.to_utf8()
        .and_then(|text| other.get(&text).map(|found| py_equals(value, found)))
        .unwrap_or(false)
}

enum Number {
    Whole(BigInt),
    Fractional(f64),
}

fn number_of(value: &PyValue) -> Option<Number> {
    match value {
        PyValue::Bool(flag) => Some(Number::Whole(BigInt::from(u8::from(*flag)))),
        PyValue::Int(number) => Some(Number::Whole(number.clone())),
        PyValue::Float(number) => Some(Number::Fractional(*number)),
        _ => None,
    }
}

fn numbers_equal(left: &Number, right: &Number) -> bool {
    match (left, right) {
        (Number::Whole(a), Number::Whole(b)) => a == b,
        (Number::Fractional(a), Number::Fractional(b)) => a == b,
        (Number::Whole(whole), Number::Fractional(fraction))
        | (Number::Fractional(fraction), Number::Whole(whole)) => {
            fraction.is_finite()
                && fraction.fract() == 0.0
                && num_traits::FromPrimitive::from_f64(*fraction).as_ref() == Some(whole)
        }
    }
}

/// `value == 0`.
pub fn equals_zero(value: &PyValue) -> bool {
    py_equals(value, &PyValue::Int(BigInt::zero()))
}

/// `value == number`.
pub fn equals_int(value: &PyValue, number: i64) -> bool {
    py_equals(value, &PyValue::int(number))
}

/// `value == "text"`.
pub fn equals_text(value: &PyValue, text: &str) -> bool {
    matches!(value.peel(), PyValue::Str(own) if own.equals(text))
}

/// `value["key"]`: a dict is indexed; every other kind raises the `TypeError` or `KeyError` Python does.
pub fn subscript(value: &PyValue, key: &str) -> Result<PyValue, Raised> {
    match value.peel() {
        PyValue::Dict(map) => map
            .get(key)
            .cloned()
            .ok_or_else(|| Raised::new("KeyError", &PyStr::new(key).repr())),
        PyValue::List(_) => Err(Raised::new(
            "TypeError",
            "list indices must be integers or slices, not str",
        )),
        PyValue::Tuple(_) => Err(Raised::new(
            "TypeError",
            "tuple indices must be integers or slices, not str",
        )),
        PyValue::Str(_) => Err(Raised::new(
            "TypeError",
            "string indices must be integers, not 'str'",
        )),
        other => Err(Raised::new(
            "TypeError",
            &format!("'{}' object is not subscriptable", other.type_name()),
        )),
    }
}

/// `value.get("key")`: only a dict has `get`.
pub fn dict_get(value: &PyValue, key: &str) -> Result<PyValue, Raised> {
    match value.peel() {
        PyValue::Dict(map) => Ok(map.get(key).cloned().unwrap_or(PyValue::None)),
        other => Err(Raised::new(
            "AttributeError",
            &format!("'{}' object has no attribute 'get'", other.type_name()),
        )),
    }
}

/// The `TypeError` of `os.path.join(str, value)` for a `value` that is not a string.
pub fn join_argument_error(value: &PyValue) -> Raised {
    Raised::new(
        "TypeError",
        &format!(
            "join() argument must be str, bytes, or os.PathLike object, not '{}'",
            value.type_name()
        ),
    )
}

/// `os.path.join(base, value)` for a value read from JSON.
pub fn join_value(base: &PyStr, value: &PyValue) -> Result<PyStr, Raised> {
    match value.peel() {
        PyValue::Str(text) => Ok(crate::ospath::join(base, &[text])),
        other => Err(join_argument_error(other)),
    }
}

/// `text.find(needle)` as Python reports it: the position in characters (not bytes), `-1` when absent.
pub fn find_position(text: &str, needle: &str) -> i64 {
    text.find(needle)
        .map_or(-1, |byte| text[..byte].chars().count() as i64)
}

/// Whether `c` is white space for the regular expression `\s` on a `str` (`str.isspace()`): Rust's `White_Space`
/// plus the four information separators.
fn is_regex_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// `re.search(r'"KEY":\s*_built\.get', source) is not None`.
///
/// The original also tries the longer `"KEY":\s*_built\.get\("KEY"\)` first; every match of that is a match of
/// this, so this alone decides the result.
pub fn sets_key_from_built(source: &str, key: &str) -> bool {
    let opening = format!("\"{key}\":");
    let mut from = 0;
    while let Some(found) = source[from..].find(&opening) {
        let after = from + found + opening.len();
        if source[after..]
            .trim_start_matches(is_regex_space)
            .starts_with("_built.get")
        {
            return true;
        }
        // continue after the opening quote of this occurrence (an ASCII byte, so a character boundary)
        from += found + 1;
    }
    false
}

/// `str.rstrip()` of the text.
pub fn rstrip(text: &PyStr) -> PyStr {
    let points = text.code_points();
    let end = points
        .iter()
        .rposition(|&point| char::from_u32(point).is_none_or(|c| !is_regex_space(c)))
        .map_or(0, |last| last + 1);
    PyStr::from_code_points(points[..end].to_vec())
}

/// `text[-count:]`.
pub fn tail(text: &PyStr, count: usize) -> PyStr {
    let points = text.code_points();
    PyStr::from_code_points(points[points.len().saturating_sub(count)..].to_vec())
}

/// `str.splitlines()` (no line ends kept): the separators Python splits on, `\r\n` as one.
pub fn splitlines(text: &PyStr) -> Vec<PyStr> {
    const SEPARATORS: [u32; 10] = [
        0x0A, 0x0B, 0x0C, 0x0D, 0x1C, 0x1D, 0x1E, 0x85, 0x2028, 0x2029,
    ];
    let points = text.code_points();
    let mut lines = Vec::new();
    let mut start = 0;
    let mut index = 0;
    while index < points.len() {
        if SEPARATORS.contains(&points[index]) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataset_prep::Recorder;
    use crate::pyjson::loads;

    fn rows(text: &str) -> PyValue {
        loads(text).expect("literal JSON")
    }

    #[test]
    fn find_positions_count_characters_not_bytes() {
        assert_eq!(find_position("한글 gate", "gate"), 3);
        assert_eq!(find_position("abc", "z"), -1);
    }

    #[test]
    fn the_built_get_search_follows_the_regular_expression() {
        assert!(sets_key_from_built(
            "\"kv_gib\":  _built.get(\"kv_gib\")",
            "kv_gib"
        ));
        assert!(sets_key_from_built("\"kv_gib\":\n\t_built.get", "kv_gib"));
        assert!(!sets_key_from_built("\"kv_gib\": built.get", "kv_gib"));
        assert!(!sets_key_from_built("kv_gib: _built.get", "kv_gib"));
        // a first occurrence that does not match does not hide a later one
        assert!(sets_key_from_built(
            "\"a\": x\n\"a\": _built.get(\"a\")",
            "a"
        ));
        assert!(sets_key_from_built("\"a\":\u{1c}_built.get", "a"));
        assert!(!sets_key_from_built("\"a\":\u{200b}_built.get", "a"));
    }

    #[test]
    fn python_equality_treats_numbers_and_containers_as_python_does() {
        assert!(py_equals(&rows("6"), &rows("6.0")));
        assert!(py_equals(&rows("true"), &rows("1")));
        assert!(!py_equals(
            &rows("[1]"),
            &PyValue::Tuple(vec![PyValue::int(1)])
        ));
        assert!(py_equals(
            &rows("{\"a\": 1, \"b\": 2}"),
            &rows("{\"b\": 2.0, \"a\": 1}")
        ));
        assert!(!py_equals(
            &rows("{\"a\": 1}"),
            &rows("{\"a\": 1, \"b\": 2}")
        ));
        assert!(!py_equals(&rows("\"1\""), &rows("1")));
        assert!(equals_zero(&rows("false")));
        assert!(!equals_zero(&PyValue::None));
    }

    #[test]
    fn indexing_failures_carry_the_python_messages() {
        let message = |value: &str| {
            subscript(&rows(value), "k")
                .unwrap_err()
                .failure()
                .message
                .to_utf8()
                .unwrap()
        };
        assert_eq!(
            message("[]"),
            "list indices must be integers or slices, not str"
        );
        assert_eq!(
            message("\"s\""),
            "string indices must be integers, not 'str'"
        );
        assert_eq!(message("1"), "'int' object is not subscriptable");
        assert_eq!(message("null"), "'NoneType' object is not subscriptable");
        assert_eq!(message("{}"), "'k'");
        assert_eq!(
            dict_get(&rows("[]"), "k")
                .unwrap_err()
                .failure()
                .message
                .to_utf8()
                .unwrap(),
            "'list' object has no attribute 'get'"
        );
    }

    #[test]
    fn splitlines_matches_python() {
        let lines = |text: &str| -> Vec<String> {
            splitlines(&PyStr::new(text))
                .iter()
                .map(|l| l.to_utf8().unwrap())
                .collect()
        };
        assert_eq!(lines("a\nb"), ["a", "b"]);
        assert_eq!(lines("a\r\nb\n"), ["a", "b"]);
        assert_eq!(lines(""), Vec::<String>::new());
        assert_eq!(lines("\n"), [""]);
        assert_eq!(lines("a\u{2028}b\u{85}c\u{1c}d"), ["a", "b", "c", "d"]);
    }

    #[test]
    fn rstrip_and_tail_follow_str_semantics() {
        assert_eq!(rstrip(&PyStr::new("x \n\t")).to_utf8().unwrap(), "x");
        assert_eq!(rstrip(&PyStr::new("  ")).to_utf8().unwrap(), "");
        assert_eq!(tail(&PyStr::new("한글abc"), 4).to_utf8().unwrap(), "글abc");
        assert_eq!(tail(&PyStr::new("ab"), 600).to_utf8().unwrap(), "ab");
    }

    #[test]
    fn emit_prints_rows_before_a_later_failure() {
        let mut console = Recorder::default();
        let error = emit(
            &rows(r#"[["a", true, ""], ["short", false]]"#),
            &mut console,
        )
        .unwrap_err();
        assert_eq!(console.stdout.to_utf8().unwrap(), "PASS a\n");
        assert_eq!(
            error.failure().message.to_utf8().unwrap(),
            "not enough values to unpack (expected 3, got 2)"
        );
    }

    #[test]
    fn emit_refuses_a_failed_row_whose_detail_is_not_text() {
        let mut console = Recorder::default();
        let error = emit(
            &rows(r#"[["a", true, ""], ["b", false, {"x": 1}]]"#),
            &mut console,
        )
        .unwrap_err();
        assert_eq!(console.stdout.to_utf8().unwrap(), "PASS a\n");
        assert_eq!(error.failure().class, "TypeError");
        assert_eq!(
            error.failure().message.to_utf8().unwrap(),
            "can only concatenate str (not \"dict\") to str"
        );
    }

    #[test]
    fn emit_unpacks_strings_and_counts_truthiness() {
        // "abc" unpacks into the one-character values 'a', 'b', 'c': a truthy `good`, so the row passes
        let mut console = Recorder::default();
        let verdict = emit(
            &rows(r#"["abc", ["n", 0, "why"], ["m", "x", 5]]"#),
            &mut console,
        )
        .unwrap();
        assert!(!verdict);
        assert_eq!(
            console.stdout.to_utf8().unwrap(),
            "PASS a\nFAIL n  — why\nPASS m\n\nfull ⊇ lite conformance: 2/3 FAIL\n"
        );
    }

    #[test]
    fn installed_binary_names_its_scripts_directory() {
        let context = Context::from_installed_binary(Path::new(
            "/skills/adversarial-benchmark/rust/target/release/conformance_full_superset_lite",
        ))
        .expect("installed layout");
        assert_eq!(
            context.scripts_dir().to_utf8().unwrap(),
            "/skills/adversarial-benchmark/scripts"
        );
        assert!(Context::from_installed_binary(Path::new("/elsewhere/deps/bin")).is_none());
        assert!(
            Context::from_installed_binary(Path::new("/skills/s/rust/target/debug/bin")).is_none()
        );
    }
}
