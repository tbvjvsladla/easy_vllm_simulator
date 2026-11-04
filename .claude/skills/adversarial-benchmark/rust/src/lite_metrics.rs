//! `lite_metrics.py`: the deterministic parser, renderer and verdict of the lite benchmark's five metrics.
//!
//! The Python original reads JSON documents written by `lite_bench.sh`, so every input here is a [`PyValue`] and
//! every operation keeps Python's behaviour on the value it is actually given: truthiness (`not bench`), the
//! `AttributeError` of `.get` on a non-dict, `bool` counting as `int`, `float(v)` accepting numeric text, `NaN` and
//! `inf` flowing through `round` and `format`, and the exception a wrong type raises *at the operation that
//! raises it*. A failure is returned as a [`Failure`] (class and message); `SystemExit` is its own class.
//!
//! Results are typed where the original returns a fixed shape ([`GenTps`], [`AcceptLength`], [`EngineLog`],
//! [`NodeCapacity`], [`LiteMetrics`], [`Judgement`]); each has a `to_value` for the JSON the original prints.
//!
//! Readers: every JSON file is read with [`pyfs::load_json_or_none`]'s decoder (`json.load`: strict UTF-8, `NaN`
//! accepted). The `NaN` identity decoders (`pyjson::loads_objects`) are not used: no rule here asks whether two
//! `NaN`s are the same object, so plain floats are exact. An engine log is decoded with replacement and universal
//! newlines ([`crate::lite_engine_log`]).
//!
//! Known original behaviours kept on purpose (the original's callers rely on them or they have not been judged a
//! defect): a `NaN` GPU figure formats as `nan GiB (nan%)` and counts as a measured value in the verdict; `bool`
//! request counts pass `_requests_ok`; `build` and `--judge` read the files they name at the moment they run.

use crate::guidellm::Failure;
use crate::lite_engine_log::{
    decode_log_text, first_kv_memory_group, kv_tokens_group, nontorch_group, weights_group,
};
use crate::pyfs::{self, FsFailure};
use crate::pynumber::{
    float_call, float_from_int, float_from_str, format_fixed, int_from_str, round_to_places,
    round_to_two_places,
};
use crate::pyops::{compare, is_positive_number, multiply, num_of, Compare, Num};
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use num_bigint::{BigInt, Sign};
use num_traits::{ToPrimitive, Zero};

/// `GIB = 1024.0 ** 3`.
pub const GIB: f64 = 1_073_741_824.0;
/// `LITE_VERDICTS`, in the original's order.
pub const LITE_VERDICT_NAMES: [&str; 3] = ["pass", "measurement_path_failed", "server_failed"];
/// `LITE_EXIT`.
pub const LITE_EXIT_CODES: [(&str, i64); 3] = [
    ("pass", 0),
    ("measurement_path_failed", 6),
    ("server_failed", 7),
];
/// `_INITIAL_TEST_FAILED`.
pub const INITIAL_TEST_FAILED: &str = "Initial test run failed";
/// The alternatives of `_CONNECT_ERROR_RE` (`re.I`), in the original's order.
pub const CONNECT_ERROR_ALTERNATIVES: [&str; 4] = [
    "Cannot connect to host",
    "Connect call failed",
    "Connection refused",
    "ClientConnectorError",
];
/// `_CONNECT_ERROR_RE.pattern`.
pub const CONNECT_ERROR_PATTERN: &str =
    "Cannot connect to host|Connect call failed|Connection refused|ClientConnectorError";
/// `_KV_PATTERNS`, `_KV_TOKENS_PAT`, `_WEIGHTS_PAT` and `_NONTORCH_PAT` as the original spells them (the matcher is
/// [`crate::lite_engine_log`]).
pub const KV_PATTERNS: [&str; 6] = [
    r#"kv[_ ]cache[_ ]memory[_ ]?bytes['\"]?\s*[=:]\s*([\d,]+)"#,
    r"[Aa]vailable KV cache memory[:= ]+([\d.]+)\s*GiB",
    r"reserved for KV [Cc]ache[:= ]+([\d.]+)\s*GiB",
    r"reserved\s+([\d.]+)\s*GiB\s+memory\s+for\s+KV\s+[Cc]ache",
    r"GPU KV cache (?:size|memory)[:= ]+([\d.]+)\s*GiB",
    r"KV [Cc]ache[^\n]*?([\d.]+)\s*GiB",
];
pub const KV_TOKENS_PATTERN: &str = r"GPU KV cache size[:= ]+([\d,]+)\s*tokens";
pub const WEIGHTS_PATTERN: &str = r"[Mm]odel (?:weights take|loading took)[:= ]*([\d.]+)\s*GiB";
pub const NONTORCH_PATTERN: &str = r"non[_-]?torch[^\n]*?([\d.]+)\s*GiB";
/// `_verdict`'s `lite_verdict_source`; the label is part of every stored raw and certificate.
pub const VERDICT_SOURCE: &str = "lite_metrics.judge_lite(raw)";

const NO_GPU_FIGURE: &str = "N/A (통합메모리 nvidia-smi 미보고 · serve-log 분해 없음)";
const PROBE_FAILED: &str = "— (SSH probe 실패)";

static NONE: PyValue = PyValue::None;

// ── Python semantics of the operations the original applies to raw values ────────────────

fn failure(class: &'static str, message: impl AsRef<str>) -> Failure {
    Failure::new(class, message.as_ref())
}

fn is_none(value: &PyValue) -> bool {
    matches!(value.peel(), PyValue::None)
}

/// `receiver.get(key)`: a dict answers, anything else has no `get`. An absent key is `None`.
fn dict_get<'a>(receiver: &'a PyValue, key: &str) -> Result<&'a PyValue, Failure> {
    match receiver.peel() {
        PyValue::Dict(map) => Ok(map.get(key).unwrap_or(&NONE)),
        other => Err(no_get(other)),
    }
}

/// `receiver.get(key, default)`.
fn dict_get_or<'a>(
    receiver: &'a PyValue,
    key: &str,
    default: &'a PyValue,
) -> Result<&'a PyValue, Failure> {
    match receiver.peel() {
        PyValue::Dict(map) => Ok(map.get(key).unwrap_or(default)),
        other => Err(no_get(other)),
    }
}

fn no_get(receiver: &PyValue) -> Failure {
    failure(
        "AttributeError",
        format!("'{}' object has no attribute 'get'", receiver.type_name()),
    )
}

/// `receiver["key"]` for a string key.
fn subscript<'a>(receiver: &'a PyValue, key: &str) -> Result<&'a PyValue, Failure> {
    match receiver.peel() {
        PyValue::Dict(map) => map
            .get(key)
            .ok_or_else(|| Failure::with_message("KeyError", PyStr::new(&PyStr::new(key).repr()))),
        PyValue::List(_) => Err(failure(
            "TypeError",
            "list indices must be integers or slices, not str",
        )),
        PyValue::Tuple(_) => Err(failure(
            "TypeError",
            "tuple indices must be integers or slices, not str",
        )),
        PyValue::Str(_) => Err(failure(
            "TypeError",
            "string indices must be integers, not 'str'",
        )),
        PyValue::Bytes(_) => Err(failure(
            "TypeError",
            "byte indices must be integers or slices, not str",
        )),
        other => Err(failure(
            "TypeError",
            format!("'{}' object is not subscriptable", other.type_name()),
        )),
    }
}

/// `for item in value`: the items a loop sees.
fn iterate(value: &PyValue) -> Result<Vec<PyValue>, Failure> {
    match value.peel() {
        PyValue::List(items) | PyValue::Tuple(items) => Ok(items.clone()),
        PyValue::Dict(map) => Ok(map
            .iter()
            .map(|(key, _)| PyValue::Str(key.clone()))
            .collect()),
        PyValue::Str(text) => Ok(text
            .code_points()
            .iter()
            .map(|&point| PyValue::Str(PyStr::from_code_points(vec![point])))
            .collect()),
        PyValue::Bytes(bytes) => Ok(bytes.iter().map(|&byte| PyValue::int(byte)).collect()),
        other => Err(failure(
            "TypeError",
            format!("'{}' object is not iterable", other.type_name()),
        )),
    }
}

fn is_text(value: &PyValue, expected: &str) -> bool {
    matches!(value.peel(), PyValue::Str(text) if text.equals(expected))
}

fn text_of(value: &PyValue) -> PyStr {
    value.display_text()
}

/// `format(value, ".<places>f")` for a value of any type.
fn format_places(value: &PyValue, places: u32) -> Result<String, Failure> {
    match value.peel() {
        PyValue::Float(number) => Ok(format_fixed(*number, places)),
        PyValue::Bool(flag) => Ok(format_fixed(f64::from(u8::from(*flag)), places)),
        PyValue::Int(number) => Ok(format_fixed(float_from_int(number)?, places)),
        PyValue::Str(_) => Err(failure(
            "ValueError",
            "Unknown format code 'f' for object of type 'str'",
        )),
        other => Err(failure(
            "TypeError",
            format!(
                "unsupported format string passed to {}.__format__",
                other.type_name()
            ),
        )),
    }
}

/// `"%d" % value`.
fn percent_d(value: &PyValue) -> Result<String, Failure> {
    match value.peel() {
        PyValue::Int(number) => Ok(number.to_string()),
        PyValue::Bool(flag) => Ok(u8::from(*flag).to_string()),
        PyValue::Float(number) if number.is_nan() => {
            Err(failure("ValueError", "cannot convert float NaN to integer"))
        }
        PyValue::Float(number) if number.is_infinite() => Err(failure(
            "OverflowError",
            "cannot convert float infinity to integer",
        )),
        PyValue::Float(number) => Ok(crate::pynumber::int_from_float(*number)?.to_string()),
        other => Err(failure(
            "TypeError",
            format!(
                "%d format: a real number is required, not {}",
                other.type_name()
            ),
        )),
    }
}

/// `a == b` for two real numbers (`bool`, `int`, `float`); a non-number is never equal to a number.
fn numbers_equal(left: &PyValue, right: &PyValue) -> bool {
    matches!(
        (
            compare(Compare::LessEqual, left, right),
            compare(Compare::GreaterEqual, left, right)
        ),
        (Ok(true), Ok(true))
    )
}

/// `left / right` as Python divides two operands: `int / int` is exact, anything with a float is IEEE.
fn true_divide(left: &PyValue, right: &PyValue) -> Result<f64, Failure> {
    let (Some(a), Some(b)) = (num_of(left.peel()), num_of(right.peel())) else {
        return Err(failure(
            "TypeError",
            format!(
                "unsupported operand type(s) for /: '{}' and '{}'",
                left.type_name(),
                right.type_name()
            ),
        ));
    };
    match (a, b) {
        (Num::Int(a), Num::Int(b)) => integer_ratio(&a, &b),
        (a, b) => {
            let (a, b) = (a.to_f64()?, b.to_f64()?);
            if b == 0.0 {
                Err(failure("ZeroDivisionError", "float division by zero"))
            } else {
                Ok(a / b)
            }
        }
    }
}

/// `int / int`: the correctly rounded quotient (CPython's `long_true_divide`), exact beyond 2**53.
fn integer_ratio(numerator: &BigInt, denominator: &BigInt) -> Result<f64, Failure> {
    if denominator.is_zero() {
        return Err(failure("ZeroDivisionError", "division by zero"));
    }
    let exact_limit = BigInt::from(1u64 << 53);
    if numerator.magnitude() <= exact_limit.magnitude()
        && denominator.magnitude() <= exact_limit.magnitude()
    {
        return Ok(
            numerator.to_f64().unwrap_or(f64::NAN) / denominator.to_f64().unwrap_or(f64::NAN)
        );
    }
    let negative = (numerator.sign() == Sign::Minus) != (denominator.sign() == Sign::Minus);
    let (top, bottom) = (
        BigInt::from(numerator.magnitude().clone()),
        BigInt::from(denominator.magnitude().clone()),
    );
    if top.is_zero() {
        return Ok(if negative { -0.0 } else { 0.0 });
    }
    let shift = 64 + bottom.bits() as i64 - top.bits() as i64;
    let (scaled_top, scaled_bottom) = if shift >= 0 {
        (top << (shift as usize), bottom)
    } else {
        (top, bottom << ((-shift) as usize))
    };
    let quotient = &scaled_top / &scaled_bottom;
    let sticky = u128::from(!(&scaled_top % &scaled_bottom).is_zero());
    let quotient: u128 = quotient.to_u128().unwrap_or(u128::MAX);
    // 65..66 significant bits plus a sticky bit: one conversion rounds to nearest-even correctly.
    let mut value = ((quotient << 1) | sticky) as f64;
    let mut exponent = -(shift + 1);
    while exponent > 1000 {
        value *= 2f64.powi(1000);
        exponent -= 1000;
    }
    while exponent < -1000 {
        value *= 2f64.powi(-1000);
        exponent += 1000;
    }
    value *= 2f64.powi(exponent as i32);
    if value.is_infinite() {
        return Err(failure(
            "OverflowError",
            "integer division result too large for a float",
        ));
    }
    Ok(if negative { -value } else { value })
}

/// `sum(parts)` over floats, starting from the integer `0`: CPython 3.12 adds the first float to `0` and the rest
/// with Neumaier compensation.
fn python_float_sum(parts: &[f64]) -> f64 {
    let Some((&first, rest)) = parts.split_first() else {
        return 0.0;
    };
    let mut total = 0.0 + first;
    let mut compensation = 0.0;
    for &part in rest {
        let next = total + part;
        if total.abs() >= part.abs() {
            compensation += (total - next) + part;
        } else {
            compensation += (part - next) + total;
        }
        total = next;
    }
    if compensation != 0.0 && compensation.is_finite() {
        total += compensation;
    }
    total
}

// ── bench JSON ────────────────────────────────────────────────────────────────────────

/// What `open(path)` is asked to open: a path, or an inherited file descriptor (an `int`).
pub(crate) enum OpenTarget {
    Path(PyStr),
    Descriptor(i32),
}

/// The checks `open(path, ...)` makes of its argument before it touches a file.
pub(crate) fn open_target(path: &PyValue) -> Result<OpenTarget, Failure> {
    let not_a_path = |value: &PyValue| {
        failure(
            "TypeError",
            format!(
                "expected str, bytes or os.PathLike object, not {}",
                value.type_name()
            ),
        )
    };
    match path.peel() {
        PyValue::Str(text) => Ok(OpenTarget::Path(text.clone())),
        PyValue::Bool(flag) => Ok(OpenTarget::Descriptor(i32::from(*flag))),
        PyValue::Int(number) => match number.to_i32() {
            Some(descriptor) if descriptor < 0 => {
                Err(failure("ValueError", "negative file descriptor"))
            }
            Some(descriptor) => Ok(OpenTarget::Descriptor(descriptor)),
            None => Err(not_a_path(path.peel())),
        },
        other => Err(not_a_path(other)),
    }
}

/// `open(fd, encoding=...).read()` for a descriptor number named by a raw reading: the bytes from the **current
/// offset** of the descriptor's shared open file description to its end (the offset stays there for every holder of
/// the description), after which the descriptor number is closed, as the file object that took ownership closes it.
///
/// What `open(fd)` does before it owns the number is kept, and is decided **before** any ownership is taken: the
/// number must be open (the kernel's `stat` of `/proc/self/fd/N`, the probe the R4 `spec_source` read also uses;
/// a number that is not open is `EBADF`) and must not name a directory (`EISDIR`, Python's `IsADirectoryError`).
/// Such a number is returned as an error and is **never closed**. Only an accepted, non-directory descriptor is
/// taken over; from then on it is closed whatever happens next, a failing read included (a write-only
/// descriptor is `EBADF` on `read`). The caller treats every `OSError` as fail-soft. The file is not re-opened:
/// the read uses the descriptor itself, so the shared offset is the one the holder set.
///
/// This is one of the two places the crate needs `unsafe` (the other is the R4 `spec_source` read): a descriptor
/// *number* has no owner the type system knows. `File::from_raw_fd` requires a valid, open descriptor that
/// nothing else owns. The first part is the validation above. The second is the original's contract, kept
/// explicit: a raw reading that names a number hands that number over to this one read, and while this function
/// runs no other thread opens, closes or reads that number (the program holds no other owner of an inherited
/// number; the standard streams are never closed by the standard library, and a Rust value that owned the same
/// number would see it closed - which is why the checks run the closing cases in isolated child processes and
/// never name the runner's own descriptors). The check and the take-over are two steps, so a concurrent close
/// between them is exactly the use this contract excludes. The number is consumed once: the drop of the `File`
/// closes it.
pub(crate) fn read_descriptor(descriptor: i32) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    use std::os::fd::FromRawFd;
    match std::fs::metadata(format!("/proc/self/fd/{descriptor}")) {
        Ok(status) if status.is_dir() => return Err(std::io::Error::from_raw_os_error(21)), // EISDIR
        Ok(_) => {}
        Err(_) => return Err(std::io::Error::from_raw_os_error(9)), // EBADF
    }
    // SAFETY: `descriptor` was just observed open and is not a directory; the function documentation states the
    // ownership contract (no other owner, no concurrent use). Ownership ends with the drop of `file`.
    let mut file = unsafe { std::fs::File::from_raw_fd(descriptor) };
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// `_load_json(path)`: the JSON document, or `None` when there is no path, the file cannot be read or decoded, or
/// the text is not JSON. A path of a type `open` rejects raises, and so does a document nested beyond the
/// recursion limit.
pub fn load_json(path: &PyValue) -> Result<Option<PyValue>, Failure> {
    if !path.is_truthy() {
        return Ok(None);
    }
    match open_target(path) {
        Ok(OpenTarget::Path(text)) => pyfs::load_json_or_none(&text),
        Ok(OpenTarget::Descriptor(descriptor)) => match read_descriptor(descriptor) {
            Ok(bytes) => decode_document(&bytes),
            Err(_) => Ok(None),
        },
        Err(error) if error.class == "ValueError" => Ok(None),
        Err(error) => Err(error),
    }
}

fn decode_document(bytes: &[u8]) -> Result<Option<PyValue>, Failure> {
    use crate::pyjson::{decode_file_text, loads_text, LoadError};
    let Ok(text) = decode_file_text(bytes) else {
        return Ok(None);
    };
    match loads_text(&text) {
        Ok(value) => Ok(Some(value)),
        Err(error @ LoadError::TooDeep { .. }) => {
            let text = error.to_string();
            Err(failure(
                "RecursionError",
                text.strip_prefix("RecursionError: ").unwrap_or(&text),
            ))
        }
        Err(_) => Ok(None),
    }
}

/// Where a throughput figure came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenTpsSource {
    MedianTpot,
    OutputThroughput,
}

impl GenTpsSource {
    pub fn label(self) -> &'static str {
        match self {
            GenTpsSource::MedianTpot => "median_tpot",
            GenTpsSource::OutputThroughput => "output_throughput",
        }
    }
}

/// The warm-leg decode throughput in tokens per second and its source.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GenTps {
    pub tokens_per_second: f64,
    pub source: GenTpsSource,
}

/// `gen_tps_from_bench(bench)`: `1000 / median_tpot_ms`, else `output_throughput`, else nothing.
pub fn gen_tps_from_bench(bench: &PyValue) -> Result<Option<GenTps>, Failure> {
    if !bench.is_truthy() {
        return Ok(None);
    }
    let tpot = dict_get(bench, "median_tpot_ms")?;
    if is_positive_number(tpot)? {
        let divisor = num_of(tpot.peel()).expect("a positive number").to_f64()?;
        return Ok(Some(GenTps {
            tokens_per_second: round_to_two_places(1000.0 / divisor),
            source: GenTpsSource::MedianTpot,
        }));
    }
    let throughput = dict_get(bench, "output_throughput")?;
    if is_positive_number(throughput)? {
        let value = num_of(throughput.peel())
            .expect("a positive number")
            .to_f64()?;
        return Ok(Some(GenTps {
            tokens_per_second: round_to_two_places(value),
            source: GenTpsSource::OutputThroughput,
        }));
    }
    Ok(None)
}

/// The speculative-decoding acceptance length exactly as the bench file reports it, and where it was read.
#[derive(Clone, Debug, PartialEq)]
pub struct AcceptLength {
    /// A JSON number (`int` or `float`, never `bool`), unrounded.
    pub value: PyValue,
    /// `<leg>.spec_decode_acceptance_length`.
    pub source: PyStr,
}

/// `accept_len_from_bench(bench, leg)`.
pub fn accept_len_from_bench(
    bench: &PyValue,
    leg: &PyValue,
) -> Result<Option<AcceptLength>, Failure> {
    if !bench.is_truthy() {
        return Ok(None);
    }
    let value = dict_get(bench, "spec_decode_acceptance_length")?;
    match value.peel() {
        PyValue::Int(_) | PyValue::Float(_) => {
            let mut source = text_of(leg);
            source.push_str(".spec_decode_acceptance_length");
            Ok(Some(AcceptLength {
                value: value.clone(),
                source,
            }))
        }
        _ => Ok(None),
    }
}

/// `cold_ttft_ms(bench_cold)`: `median_ttft_ms` (else `mean_ttft_ms`) when positive, else the first of `ttfts`.
pub fn cold_ttft_ms(bench_cold: &PyValue) -> Result<Option<f64>, Failure> {
    if !bench_cold.is_truthy() {
        return Ok(None);
    }
    let median = dict_get(bench_cold, "median_ttft_ms")?;
    let figure = if median.is_truthy() {
        median
    } else {
        dict_get(bench_cold, "mean_ttft_ms")?
    };
    if is_positive_number(figure)? {
        let value = num_of(figure.peel()).expect("a positive number").to_f64()?;
        return Ok(Some(round_to_places(value, 1)));
    }
    if let PyValue::List(items) = dict_get(bench_cold, "ttfts")?.peel() {
        if let Some(first) = items.first() {
            return Ok(Some(round_to_places(float_call(first.peel())?, 1)));
        }
    }
    Ok(None)
}

// ── engine log ────────────────────────────────────────────────────────────────────────

/// The figures `parse_engine_log` finds; `None` is the original's `null` (fail-soft).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EngineLog {
    pub kv_gib: Option<f64>,
    pub kv_tokens: Option<BigInt>,
    pub weights_gib: Option<f64>,
    pub nontorch_gib: Option<f64>,
    pub reserved_total_gib: Option<f64>,
}

fn float_or_null(value: Option<f64>) -> PyValue {
    value.map_or(PyValue::None, PyValue::Float)
}

impl EngineLog {
    pub fn to_value(&self) -> PyValue {
        PyValue::Dict(PyDict::from_distinct_entries([
            (PyStr::new("kv_gib"), float_or_null(self.kv_gib)),
            (
                PyStr::new("kv_tokens"),
                self.kv_tokens.clone().map_or(PyValue::None, PyValue::Int),
            ),
            (PyStr::new("weights_gib"), float_or_null(self.weights_gib)),
            (PyStr::new("nontorch_gib"), float_or_null(self.nontorch_gib)),
            (
                PyStr::new("reserved_total_gib"),
                float_or_null(self.reserved_total_gib),
            ),
        ]))
    }

    /// The per-key merge `build` applies: the head's figure where it has one, else the tail's.
    fn or_from_tail(self, tail: EngineLog) -> EngineLog {
        EngineLog {
            kv_gib: self.kv_gib.or(tail.kv_gib),
            kv_tokens: self.kv_tokens.or(tail.kv_tokens),
            weights_gib: self.weights_gib.or(tail.weights_gib),
            nontorch_gib: self.nontorch_gib.or(tail.nontorch_gib),
            reserved_total_gib: self.reserved_total_gib.or(tail.reserved_total_gib),
        }
    }
}

/// The figure of a captured group (`float(raw)` or nothing for text `float` refuses), rounded to two places.
fn rounded_group(group: Option<String>) -> Option<f64> {
    float_from_str(&PyStr::new(&group?))
        .ok()
        .map(round_to_two_places)
}

/// `parse_engine_log(path)`: KV cache size, weights and non-torch figures of a serve log. A missing path, an
/// unreadable file or a missing line is `None` for the figures concerned.
pub fn parse_engine_log(path: &PyValue) -> Result<EngineLog, Failure> {
    let mut engine = EngineLog::default();
    if !path.is_truthy() {
        return Ok(engine);
    }
    let bytes = match open_target(path)? {
        OpenTarget::Path(text) => {
            let os = pyfs::os_string(&text).map_err(FsFailure::into_failure)?;
            std::fs::read(os)
        }
        OpenTarget::Descriptor(descriptor) => read_descriptor(descriptor),
    };
    let Ok(bytes) = bytes else {
        return Ok(engine);
    };
    let text = decode_log_text(&bytes);
    if let Some(raw) = first_kv_memory_group(&text) {
        if let Ok(value) = float_from_str(&PyStr::new(&raw.replace(',', ""))) {
            // Bytes (large numbers) become GiB; a figure already in GiB stays.
            engine.kv_gib = Some(if value > 1e6 {
                round_to_two_places(value / GIB)
            } else {
                round_to_two_places(value)
            });
        }
    }
    engine.kv_tokens = kv_tokens_group(&text)
        .and_then(|raw| int_from_str(&PyStr::new(&raw.replace(',', ""))).ok());
    engine.weights_gib = rounded_group(weights_group(&text));
    engine.nontorch_gib = rounded_group(nontorch_group(&text));
    let parts: Vec<f64> = [engine.weights_gib, engine.nontorch_gib, engine.kv_gib]
        .into_iter()
        .flatten()
        .collect();
    if !parts.is_empty() {
        engine.reserved_total_gib = Some(round_to_two_places(python_float_sum(&parts)));
    }
    Ok(engine)
}

// ── per-node capacity ─────────────────────────────────────────────────────────────────

/// `_num(v)`: `float(v)`, or `None` for what `float` refuses as `TypeError`/`ValueError`. An `int` too large for a
/// double is an `OverflowError` and escapes.
pub fn number_or_none(value: &PyValue) -> Result<Option<f64>, Failure> {
    match float_call(value.peel()) {
        Ok(number) => Ok(Some(number)),
        Err(error) if error.is_type_or_value_error() => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// The two displayed capacity strings of one node.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeCapacity {
    pub gpu: PyStr,
    pub ram: PyStr,
}

impl NodeCapacity {
    fn of_text(gpu: &str, ram: &str) -> NodeCapacity {
        NodeCapacity {
            gpu: PyStr::new(gpu),
            ram: PyStr::new(ram),
        }
    }

    pub fn to_value(&self) -> PyValue {
        PyValue::Dict(PyDict::from_distinct_entries([
            (PyStr::new("gpu"), PyValue::Str(self.gpu.clone())),
            (PyStr::new("ram"), PyValue::Str(self.ram.clone())),
        ]))
    }
}

/// `node_capacity(node, engine, is_master)`: VRAM from `nvidia-smi`, else the serve log's reserved total on the
/// master, else `N/A`; RAM from `/proc/meminfo`. A missing figure is never replaced by a made-up one.
pub fn node_capacity(
    node: &PyValue,
    engine: &PyValue,
    is_master: &PyValue,
) -> Result<NodeCapacity, Failure> {
    let used = number_or_none(dict_get(node, "gpu_smi_used_mib")?)?;
    let total = number_or_none(dict_get(node, "gpu_smi_total_mib")?)?;
    let measured = used.zip(total).filter(|&(_, total)| total > 0.0);
    let gpu = if let Some((used, total)) = measured {
        format!(
            "{} GiB ({}%)",
            format_fixed(used / 1024.0, 1),
            format_fixed(100.0 * used / total, 0)
        )
    } else if is_master.is_truthy() && !is_none(dict_get(engine, "reserved_total_gib")?) {
        format!(
            "~{} GiB (serve-log 분해; nvidia-smi N/A)",
            format_places(subscript(engine, "reserved_total_gib")?, 1)?
        )
    } else {
        NO_GPU_FIGURE.to_string()
    };
    let ram_total = number_or_none(dict_get(node, "ram_total_kib")?)?;
    let ram_available = number_or_none(dict_get(node, "ram_avail_kib")?)?;
    let ram = match ram_total
        .zip(ram_available)
        .filter(|&(total, _)| total > 0.0)
    {
        Some((total, available)) => {
            let used = total - available;
            format!(
                "{} GiB ({}%)",
                format_fixed(used / 1024.0 / 1024.0, 1),
                format_fixed(100.0 * used / total, 0)
            )
        }
        None => "N/A".to_string(),
    };
    Ok(NodeCapacity::of_text(&gpu, &ram))
}

// ── rendering ─────────────────────────────────────────────────────────────────────────

fn markdown_row(table: &mut PyStr, cells: &[&PyStr]) {
    table.push_str("\n|");
    for cell in cells {
        table.push_str(" ");
        table.push_py(cell);
        table.push_str(" |");
    }
}

/// `f"{value:.<places>f}{unit}"` when there is a value, else `absent`.
fn figure_text(value: &PyValue, places: u32, unit: &str, absent: &str) -> Result<PyStr, Failure> {
    if is_none(value) {
        Ok(PyStr::new(absent))
    } else {
        Ok(PyStr::new(&format!(
            "{}{unit}",
            format_places(value, places)?
        )))
    }
}

/// `render_single(...)`: the five-row single-node table.
pub fn render_single(
    gen: &PyValue,
    cap: &PyValue,
    ttft: &PyValue,
    kv_gib: &PyValue,
    gpu_total_gib: &PyValue,
) -> Result<PyStr, Failure> {
    let mut kv = figure_text(
        kv_gib,
        1,
        " GiB",
        "N/A (serve 로그 KV 라인 부재 — fail-soft)",
    )?;
    if !is_none(kv_gib) && gpu_total_gib.is_truthy() {
        let share = true_divide(&multiply(&PyValue::int(100), kv_gib.peel())?, gpu_total_gib)?;
        kv.push_str(&format!(" ({}%)", format_fixed(share, 0)));
    }
    let gen_cell = figure_text(gen, 2, " t/s", "N/A")?;
    let ttft_cell = figure_text(ttft, 0, " ms", "N/A")?;
    let gpu_cell = text_of(subscript(cap, "gpu")?);
    let ram_cell = text_of(subscript(cap, "ram")?);
    let mut table = PyStr::new("| 메트릭 | 값 |\n|---|---|");
    markdown_row(
        &mut table,
        &[&PyStr::new("gen tokens/sec (warm)"), &gen_cell],
    );
    markdown_row(&mut table, &[&PyStr::new("cold-start TTFT"), &ttft_cell]);
    markdown_row(&mut table, &[&PyStr::new("GPU VRAM 점유"), &gpu_cell]);
    markdown_row(&mut table, &[&PyStr::new("KV cache 점유"), &kv]);
    markdown_row(&mut table, &[&PyStr::new("시스템 RAM 점유"), &ram_cell]);
    Ok(table)
}

/// `render_multi(...)`: the merged Main | Sub table (speed and cold TTFT on the master row only).
pub fn render_multi(
    gen: &PyValue,
    ttft: &PyValue,
    caps: &PyValue,
    kv_gib: &PyValue,
) -> Result<PyStr, Failure> {
    let empty = PyValue::Dict(PyDict::new());
    let main = dict_get_or(caps, "main", &empty)?;
    let sub = dict_get(caps, "sub")?;
    let kv_main = figure_text(kv_gib, 1, " GiB/GPU (main 로그)", "N/A (fail-soft)")?;
    let gen_cell = figure_text(gen, 2, " t/s", "N/A")?;
    let ttft_cell = figure_text(ttft, 0, " ms", "N/A")?;
    let sub_column = |key: &str| -> Result<PyStr, Failure> {
        if sub.is_truthy() {
            Ok(text_of(dict_get(sub, key)?))
        } else {
            Ok(PyStr::new("— (probe 실패)"))
        }
    };
    let na = PyValue::str("N/A");
    let main_gpu = text_of(dict_get_or(main, "gpu", &na)?);
    let sub_gpu = sub_column("gpu")?;
    let main_ram = text_of(dict_get_or(main, "ram", &na)?);
    let sub_ram = sub_column("ram")?;
    let dash = PyStr::new("—");
    let mut table = PyStr::new("| 메트릭 | Main | Sub |\n|---|---|---|");
    markdown_row(
        &mut table,
        &[
            &PyStr::new("gen tokens/sec (warm) [master]"),
            &gen_cell,
            &dash,
        ],
    );
    markdown_row(
        &mut table,
        &[&PyStr::new("cold-start TTFT [master]"), &ttft_cell, &dash],
    );
    markdown_row(
        &mut table,
        &[&PyStr::new("GPU VRAM 점유"), &main_gpu, &sub_gpu],
    );
    markdown_row(
        &mut table,
        &[
            &PyStr::new("KV cache 점유"),
            &kv_main,
            &PyStr::new("N/A (sub KV 로그 미수집)"),
        ],
    );
    markdown_row(
        &mut table,
        &[&PyStr::new("시스템 RAM 점유"), &main_ram, &sub_ram],
    );
    Ok(table)
}

// ── build ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Topology {
    Single,
    Multi,
}

impl Topology {
    pub fn label(self) -> &'static str {
        match self {
            Topology::Single => "single",
            Topology::Multi => "multi",
        }
    }
}

/// The node roles a raw reading may carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeRole {
    Main,
    Sub,
}

impl NodeRole {
    pub fn label(self) -> &'static str {
        match self {
            NodeRole::Main => "main",
            NodeRole::Sub => "sub",
        }
    }
}

/// What `build` returns: the five metrics, the capacity strings per node role and the rendered table.
#[derive(Clone, Debug, PartialEq)]
pub struct LiteMetrics {
    pub topology: Topology,
    pub gen_tps: Option<GenTps>,
    pub cold_ttft_ms: Option<f64>,
    pub engine: EngineLog,
    pub accept_len: Option<AcceptLength>,
    pub capacity: Vec<(NodeRole, NodeCapacity)>,
    pub table: PyStr,
}

impl LiteMetrics {
    /// The dict the original returns, keys in its order. The `engine` object lists its five keys in one fixed order;
    /// the original's order there comes from a set and differs between processes.
    pub fn to_value(&self) -> PyValue {
        let mut capacity = PyDict::new();
        for (role, node) in &self.capacity {
            capacity.set(role.label(), node.to_value());
        }
        let mut map = PyDict::new();
        map.set("topology", PyValue::str(self.topology.label()));
        map.set(
            "gen_tps",
            float_or_null(self.gen_tps.map(|gen| gen.tokens_per_second)),
        );
        map.set(
            "gen_src",
            self.gen_tps
                .map_or(PyValue::None, |gen| PyValue::str(gen.source.label())),
        );
        map.set("cold_ttft_ms", float_or_null(self.cold_ttft_ms));
        map.set("kv_gib", float_or_null(self.engine.kv_gib));
        map.set("engine", self.engine.to_value());
        map.set(
            "accept_len",
            self.accept_len
                .as_ref()
                .map_or(PyValue::None, |accept| accept.value.clone()),
        );
        map.set(
            "accept_len_src",
            self.accept_len
                .as_ref()
                .map_or(PyValue::None, |accept| PyValue::Str(accept.source.clone())),
        );
        map.set("capacity", PyValue::Dict(capacity));
        map.set("table", PyValue::Str(self.table.clone()));
        PyValue::Dict(map)
    }
}

fn system_exit(message: String) -> Failure {
    Failure::with_message("SystemExit", PyStr::new(&message))
}

fn declared_topology(raw: &PyValue) -> Result<Topology, Failure> {
    let declared = dict_get(raw, "topology")?;
    if is_text(declared, "single") {
        Ok(Topology::Single)
    } else if is_text(declared, "multi") {
        Ok(Topology::Multi)
    } else {
        Err(system_exit(format!(
            "[lite_metrics] FAIL: topology 가 선언되지 않았다({}) — 기본값을 쓰지 않는다. 호출부가 manifest/인터뷰에서 읽어 넘겨라.",
            declared.repr()
        )))
    }
}

fn node_role(node: &PyValue) -> Result<NodeRole, Failure> {
    let declared = dict_get(node, "role")?;
    if is_text(declared, "main") {
        Ok(NodeRole::Main)
    } else if is_text(declared, "sub") {
        Ok(NodeRole::Sub)
    } else {
        Err(system_exit(format!(
            "[lite_metrics] FAIL: nodes[] 에 role 이 없다({}) — 추측하지 않는다.",
            declared.repr()
        )))
    }
}

/// The GPU memory of a node in GiB for the KV share, `None` when the node reports no positive total.
fn gpu_total_gib(node: &PyValue) -> Result<Option<f64>, Failure> {
    Ok(number_or_none(dict_get(node, "gpu_smi_total_mib")?)?
        .filter(|&total| total != 0.0)
        .map(|total| total / 1024.0))
}

/// `build(raw)`: the five metrics and their table from one raw reading. The topology and every node's role must be
/// declared; there is no default for either.
pub fn build(raw: &PyValue) -> Result<LiteMetrics, Failure> {
    let topology = declared_topology(raw)?;
    let warm = load_json(dict_get(raw, "bench_warm_json")?)?.unwrap_or(PyValue::None);
    let cold = load_json(dict_get(raw, "bench_cold_json")?)?.unwrap_or(PyValue::None);
    // The start-up lines print once, so a long-running server's tail has lost them: the head capture comes first
    // and the tail fills what the head did not have.
    let head = parse_engine_log(dict_get(raw, "engine_log_head")?)?;
    let tail = parse_engine_log(dict_get(raw, "engine_log")?)?;
    let engine = head.or_from_tail(tail);
    let gen_tps = gen_tps_from_bench(&warm)?;
    let cold_ttft = cold_ttft_ms(&cold)?;
    let empty_nodes = PyValue::List(Vec::new());
    let nodes = iterate(dict_get_or(raw, "nodes", &empty_nodes)?)?;
    let mut main_node: Option<&PyValue> = None;
    let mut sub_node: Option<&PyValue> = None;
    for node in &nodes {
        match node_role(node)? {
            NodeRole::Main => main_node = Some(node),
            NodeRole::Sub => sub_node = Some(node),
        }
    }
    let leg = PyValue::str("warm");
    let mut accept_len = accept_len_from_bench(&warm, &leg)?;
    if accept_len.is_none() {
        accept_len = accept_len_from_bench(&cold, &PyValue::str("cold"))?;
    }
    let engine_value = engine.to_value();
    let gen_value = float_or_null(gen_tps.map(|gen| gen.tokens_per_second));
    let ttft_value = float_or_null(cold_ttft);
    let kv_value = float_or_null(engine.kv_gib);
    let mut capacity = Vec::new();
    let table;
    match topology {
        Topology::Multi => {
            for (role, node) in [(NodeRole::Main, main_node), (NodeRole::Sub, sub_node)] {
                let Some(node) = node else { continue };
                // A failed SSH probe is a different cause from a unified-memory N/A and is marked as such.
                let probe_failed = role == NodeRole::Sub
                    && matches!(dict_get(node, "probe_ok")?.peel(), PyValue::Bool(false));
                capacity.push((
                    role,
                    if probe_failed {
                        NodeCapacity::of_text(PROBE_FAILED, PROBE_FAILED)
                    } else {
                        node_capacity(node, &engine_value, &PyValue::Bool(role == NodeRole::Main))?
                    },
                ));
            }
            let mut caps = PyDict::new();
            for (role, node) in &capacity {
                caps.set(role.label(), node.to_value());
            }
            table = render_multi(&gen_value, &ttft_value, &PyValue::Dict(caps), &kv_value)?;
        }
        Topology::Single => {
            let empty_node = PyValue::Dict(PyDict::new());
            let node = main_node.or(nodes.first()).unwrap_or(&empty_node);
            let cap = node_capacity(node, &engine_value, &PyValue::Bool(true))?;
            let total = float_or_null(gpu_total_gib(node)?);
            table = render_single(&gen_value, &cap.to_value(), &ttft_value, &kv_value, &total)?;
            capacity.push((NodeRole::Main, cap));
        }
    }
    Ok(LiteMetrics {
        topology,
        gen_tps,
        cold_ttft_ms: cold_ttft,
        engine,
        accept_len,
        capacity,
        table,
    })
}

// ── verdict ───────────────────────────────────────────────────────────────────────────

/// The three outcomes of a lite run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiteVerdict {
    Pass,
    /// The request never reached the server or the measurer produced no value: a harness defect.
    MeasurementPathFailed,
    /// The server answered with failures or died while measured.
    ServerFailed,
}

impl LiteVerdict {
    pub fn label(self) -> &'static str {
        match self {
            LiteVerdict::Pass => "pass",
            LiteVerdict::MeasurementPathFailed => "measurement_path_failed",
            LiteVerdict::ServerFailed => "server_failed",
        }
    }

    pub fn exit_code(self) -> i64 {
        match self {
            LiteVerdict::Pass => 0,
            LiteVerdict::MeasurementPathFailed => 6,
            LiteVerdict::ServerFailed => 7,
        }
    }
}

/// A verdict with the reasons that led to it.
#[derive(Clone, Debug, PartialEq)]
pub struct Judgement {
    pub verdict: LiteVerdict,
    pub reasons: Vec<PyStr>,
}

impl Judgement {
    fn new(verdict: LiteVerdict, reasons: Vec<PyStr>) -> Judgement {
        Judgement { verdict, reasons }
    }

    /// The four keys `_verdict` returns (and `--judge` writes into the raw reading).
    pub fn to_value(&self) -> PyValue {
        PyValue::Dict(PyDict::from_distinct_entries([
            (
                PyStr::new("lite_verdict"),
                PyValue::str(self.verdict.label()),
            ),
            (
                PyStr::new("lite_exit"),
                PyValue::int(self.verdict.exit_code()),
            ),
            (
                PyStr::new("lite_verdict_reasons"),
                PyValue::List(self.reasons.iter().cloned().map(PyValue::Str).collect()),
            ),
            (
                PyStr::new("lite_verdict_source"),
                PyValue::str(VERDICT_SOURCE),
            ),
        ]))
    }
}

/// `_verdict(v, reasons)` for any `v` and `reasons` a caller passes.
pub fn verdict_record(verdict: &PyValue, reasons: &PyValue) -> Result<PyValue, Failure> {
    let known = match verdict.peel() {
        PyValue::Str(text) => LITE_EXIT_CODES
            .iter()
            .find(|(name, _)| text.equals(name))
            .map(|&(_, code)| code),
        PyValue::List(_) | PyValue::Dict(_) => {
            return Err(failure(
                "TypeError",
                format!("unhashable type: '{}'", verdict.type_name()),
            ))
        }
        _ => None,
    };
    let Some(code) = known else {
        return Err(Failure::with_message(
            "KeyError",
            PyStr::new(&verdict.repr()),
        ));
    };
    Ok(PyValue::Dict(PyDict::from_distinct_entries([
        (PyStr::new("lite_verdict"), verdict.clone()),
        (PyStr::new("lite_exit"), PyValue::int(code)),
        (PyStr::new("lite_verdict_reasons"), reasons.clone()),
        (
            PyStr::new("lite_verdict_source"),
            PyValue::str(VERDICT_SOURCE),
        ),
    ])))
}

/// Whether one bench file shows every request completed with tokens generated, or why not.
#[derive(Clone, Debug, PartialEq)]
pub enum RequestsCheck {
    AllSucceeded,
    Rejected(PyStr),
}

impl RequestsCheck {
    fn rejection(&self) -> Option<&PyStr> {
        match self {
            RequestsCheck::AllSucceeded => None,
            RequestsCheck::Rejected(reason) => Some(reason),
        }
    }

    pub fn to_value(&self) -> PyValue {
        match self {
            RequestsCheck::AllSucceeded => PyValue::Tuple(vec![PyValue::Bool(true), PyValue::None]),
            RequestsCheck::Rejected(reason) => {
                PyValue::Tuple(vec![PyValue::Bool(false), PyValue::Str(reason.clone())])
            }
        }
    }
}

fn is_integer_value(value: &PyValue) -> bool {
    matches!(value.peel(), PyValue::Int(_) | PyValue::Bool(_))
}

/// `_requests_ok(bench, expected)`.
pub fn requests_ok(bench: &PyValue, expected: &PyValue) -> Result<RequestsCheck, Failure> {
    let reject = |text: String| Ok(RequestsCheck::Rejected(PyStr::new(&text)));
    if !matches!(bench.peel(), PyValue::Dict(_)) {
        return reject("결과 파일 없음".to_string());
    }
    let completed = dict_get(bench, "completed")?;
    let failed = dict_get(bench, "failed")?;
    let output_tokens = dict_get(bench, "total_output_tokens")?;
    if !is_integer_value(completed) || !numbers_equal(completed, expected) {
        return reject(format!(
            "completed={} ≠ 요청 {}",
            completed.repr(),
            percent_d(expected)?
        ));
    }
    let failed_is_zero_or_none = is_none(failed) || numbers_equal(failed, &PyValue::int(0));
    if !failed_is_zero_or_none {
        return reject(format!("failed={}", failed.repr()));
    }
    if !is_positive_number(output_tokens)? {
        return reject(format!(
            "total_output_tokens={}(생성 없음)",
            output_tokens.repr()
        ));
    }
    Ok(RequestsCheck::AllSucceeded)
}

/// `_CONNECT_ERROR_RE.search(tail)`: any alternative, ignoring case, anywhere.
fn mentions_connect_error(tail: &PyStr) -> bool {
    let text: Vec<char> = tail
        .code_points()
        .iter()
        .map(|&point| char::from_u32(point).unwrap_or('\u{fffd}'))
        .collect();
    CONNECT_ERROR_ALTERNATIVES.iter().any(|alternative| {
        let pattern: Vec<char> = alternative.chars().collect();
        text.windows(pattern.len()).any(|window| {
            window
                .iter()
                .zip(&pattern)
                .all(|(&seen, &wanted)| crate::engine_log::matches_ignoring_case(seen, wanted))
        })
    })
}

fn contains_text(haystack: &PyStr, needle: &str) -> bool {
    let needle: Vec<u32> = needle.chars().map(u32::from).collect();
    haystack
        .code_points()
        .windows(needle.len())
        .any(|window| window == needle)
}

/// `str(raw.get(key) or default)`.
fn text_or(raw: &PyValue, key: &str, default: &str) -> Result<PyStr, Failure> {
    let value = dict_get(raw, key)?;
    Ok(if value.is_truthy() {
        text_of(value)
    } else {
        PyStr::new(default)
    })
}

fn reason(text: String) -> PyStr {
    PyStr::new(&text)
}

/// `judge_lite(raw, built)`: whether the run was established. `raw` carries the inputs the shell script recorded;
/// anything missing is a measurement-path failure, never a pass.
pub fn judge_lite(raw: &PyValue, built: &PyValue) -> Result<Judgement, Failure> {
    let mut reasons: Vec<PyStr> = Vec::new();
    let burst_n = dict_get(raw, "burst_n")?;
    let burst_ok = is_integer_value(burst_n) && !compare(Compare::Less, burst_n, &PyValue::int(1))?;
    if !burst_ok {
        reasons.push(reason(format!(
            "판정 입력 burst_n 부재·무효({})",
            burst_n.repr()
        )));
        return Ok(Judgement::new(LiteVerdict::MeasurementPathFailed, reasons));
    }
    let cold = load_json(dict_get(raw, "bench_cold_json")?)?.unwrap_or(PyValue::None);
    let warm = load_json(dict_get(raw, "bench_warm_json")?)?.unwrap_or(PyValue::None);
    let cold_check = requests_ok(&cold, &PyValue::int(1))?;
    let warm_check = requests_ok(&warm, burst_n)?;
    if cold_check == RequestsCheck::AllSucceeded && warm_check == RequestsCheck::AllSucceeded {
        let empty = PyValue::Dict(PyDict::new());
        let capacity = dict_get(built, "capacity")?;
        let capacity = if capacity.is_truthy() {
            capacity
        } else {
            &empty
        };
        let main = dict_get(capacity, "main")?;
        let main = if main.is_truthy() { main } else { &empty };
        let mut missing: Vec<&str> = Vec::new();
        for key in ["gen_tps", "cold_ttft_ms", "kv_gib"] {
            if is_none(dict_get(built, key)?) {
                missing.push(key);
            }
        }
        for key in ["gpu", "ram"] {
            let shown = dict_get(main, key)?;
            let text = if shown.is_truthy() {
                text_of(shown)
            } else {
                PyStr::new("N/A")
            };
            let points = text.code_points();
            let starts = |prefix: &str| {
                let prefix: Vec<u32> = prefix.chars().map(u32::from).collect();
                points.starts_with(&prefix)
            };
            if starts("N/A") || starts("—") {
                missing.push(key);
            }
        }
        if !missing.is_empty() {
            reasons.push(reason(format!(
                "요청은 전부 성공했지만 메인 지표 N/A: {} — 측정기가 값을 못 냈다",
                missing.join(",")
            )));
            return Ok(Judgement::new(LiteVerdict::MeasurementPathFailed, reasons));
        }
        let count = percent_d(burst_n)?;
        reasons.push(reason(format!(
            "cold 1/1 · warm {count}/{count} 성공 · 토큰 생성 · 메인 5지표 실측"
        )));
        return Ok(Judgement::new(LiteVerdict::Pass, reasons));
    }
    for (label, check) in [("cold", &cold_check), ("warm", &warm_check)] {
        if let Some(why) = check.rejection() {
            let mut text = PyStr::new(&format!("{label}: "));
            text.push_py(why);
            reasons.push(text);
        }
    }
    let host = text_or(raw, "host_health_after", "")?;
    let client = text_or(raw, "client_plane_health", "")?;
    let mut health = PyStr::new("측정 뒤 health — 호스트 평면 ");
    health.push_py(&if host.is_empty() {
        PyStr::new("미관측")
    } else {
        host.clone()
    });
    health.push_str(" · 클라이언트 평면 ");
    health.push_py(&if client.is_empty() {
        PyStr::new("미관측")
    } else {
        client.clone()
    });
    reasons.push(health);
    if !host.equals("200") {
        reasons.push(PyStr::new(
            "서버가 측정 뒤 health 에 응답하지 않는다 — 측정 중 서버 실패",
        ));
        return Ok(Judgement::new(LiteVerdict::ServerFailed, reasons));
    }
    if !client.equals("200") {
        reasons.push(PyStr::new(
            "서버는 살아 있는데 클라이언트가 도는 평면에서 닿지 않는다 — 측정 경로 불성립(하네스)",
        ));
        return Ok(Judgement::new(LiteVerdict::MeasurementPathFailed, reasons));
    }
    let tail = text_or(raw, "bench_stderr_tail", "")?;
    let have_result =
        matches!(cold.peel(), PyValue::Dict(_)) || matches!(warm.peel(), PyValue::Dict(_));
    // `vllm bench serve` exits 0 and writes a result file (completed 0, failed N) even when the connection is
    // refused: a failure that is a connection error never reached the server.
    if mentions_connect_error(&tail) {
        reasons.push(PyStr::new(
            "실패 요청이 연결 오류다(요청이 서버에 닿지 않았다) — 측정 경로 불성립(하네스)",
        ));
        return Ok(Judgement::new(LiteVerdict::MeasurementPathFailed, reasons));
    }
    if have_result {
        reasons.push(PyStr::new(
            "서버에 닿았고 결과 파일이 실패 요청·빈 출력을 말한다 — 서버 응답 실패",
        ));
        return Ok(Judgement::new(LiteVerdict::ServerFailed, reasons));
    }
    if contains_text(&tail, INITIAL_TEST_FAILED) {
        reasons.push(reason(format!(
            "결과 파일 없음 · 초기 시험 요청이 서버 응답으로 실패('{INITIAL_TEST_FAILED}') — 서버 응답 실패"
        )));
        return Ok(Judgement::new(LiteVerdict::ServerFailed, reasons));
    }
    reasons.push(PyStr::new(
        "결과 파일 없음 · 서버 응답 실패의 증거도 없다 — 클라이언트(측정기) 실패로 읽는다",
    ));
    Ok(Judgement::new(LiteVerdict::MeasurementPathFailed, reasons))
}

/// `read_lite_verdict(raw)`: the one rule callers use to read a stored verdict. Not a dict, or a verdict outside
/// the vocabulary, is `None`; callers read `None` as "not passed".
pub fn read_lite_verdict(raw: &PyValue) -> Option<LiteVerdict> {
    let PyValue::Dict(map) = raw.peel() else {
        return None;
    };
    let PyValue::Str(text) = map.get("lite_verdict")?.peel() else {
        return None;
    };
    [
        LiteVerdict::Pass,
        LiteVerdict::MeasurementPathFailed,
        LiteVerdict::ServerFailed,
    ]
    .into_iter()
    .find(|verdict| text.equals(verdict.label()))
}
