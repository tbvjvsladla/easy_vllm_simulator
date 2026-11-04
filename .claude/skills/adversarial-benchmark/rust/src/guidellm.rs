//! The GuideLLM report parser: a Rust port of `.claude/skills/adversarial-benchmark/scripts/parse_guidellm.py`.
//!
//! [`build`] turns a loaded `benchmarks.json` ([`PyValue`]) into the measurement object the verdict rule consumes
//! (the same keys as `parse_bench`, plus the tool fingerprint, the error split and the declared limits).
//! [`classify_errors`] separates the tool's own stream-parser rejections from server failures, and
//! [`inherit_accept_len`] takes the speculative-decoding axis from a `vllm bench serve` document.
//!
//! The domain follows the Python program line by line, including the order in which conversions run, because the
//! order decides which exception is reported when several inputs are odd. Documents come from `json.load`, so the
//! values are JSON types; a value of the wrong type fails the way the Python operation on it fails
//! (`AttributeError` for `.get` on a non-mapping, `TypeError` for `len` of a number, and so on), and the command
//! line decides which of those are `ValueError`s (exit 2) and which end the program (exit 1).

use crate::pynumber::{float_from_int, format_fixed, round_to_two_places, PyException};
use crate::pystr::{lit, PyStr};
use crate::pyvalue::{PyDict, PyValue};
use num_bigint::BigInt;
use num_traits::{Signed, ToPrimitive, Zero};

/// The measurement-tool token this parser puts into its output; `classify_cell` binds the full-bench tool
/// condition to the same token (queried with `parse_guidellm bench-tool`, never typed a second time).
pub const BENCH_TOOL: &str = "guidellm";
/// Only successful requests enter the statistics; errored/incomplete ones are counted separately.
const STAT_SET: &str = "successful";

/// Messages of the tool-boundary rejections that are not server faults (lower-case; matched against `lower()`).
const TOOL_BOUNDARY_ERROR_PATTERNS: [&str; 4] = [
    "while expecting start token",
    "unexpected token",
    "jsondecodeerror",
    "expecting value",
];

/// The keys carried over from the prefix-cache observation, in output order.
pub const PREFIX_CACHE_KEYS: [&str; 8] = [
    "provenance",
    "window_start_utc",
    "queries_delta",
    "hits_delta",
    "requests_delta",
    "hit_rate_run",
    "reason",
    "source",
];
/// Hit rate from which a warning is written (a file-local constant of the original).
const PREFIX_CACHE_WARN: f64 = 0.5;

/// A Python exception whose message may hold characters a Rust `String` cannot (argument bytes that are not
/// UTF-8 survive as lone surrogates in the message).
#[derive(Clone, Debug, PartialEq)]
pub struct Failure {
    pub class: &'static str,
    pub message: PyStr,
}

impl Failure {
    pub fn new(class: &'static str, message: &str) -> Self {
        Failure {
            class,
            message: PyStr::new(message),
        }
    }

    pub fn with_message(class: &'static str, message: PyStr) -> Self {
        Failure { class, message }
    }

    /// `except ValueError` catches this one; everything else escapes the original's handlers.
    pub fn is_value_error(&self) -> bool {
        self.class == "ValueError"
    }

    /// `Class: message`, or just the class name when the message is empty, as Python prints it.
    pub fn line(&self) -> PyStr {
        let mut text = PyStr::new(self.class);
        if !self.message.is_empty() {
            text.push_str(": ");
            text.push_py(&self.message);
        }
        text
    }
}

impl From<PyException> for Failure {
    fn from(error: PyException) -> Self {
        Failure::new(error.class, &error.message)
    }
}

type Outcome<T> = Result<T, Failure>;

fn attribute_error_get(value: &PyValue) -> Failure {
    Failure::new(
        "AttributeError",
        &format!("'{}' object has no attribute 'get'", value.type_name()),
    )
}

/// `parent.get(key)` where `None` stands for the `{}` of an `x or {}` that was falsy. A real value that is not a
/// mapping has no `get`.
fn dict_get<'a>(parent: Option<&'a PyValue>, key: &str) -> Outcome<Option<&'a PyValue>> {
    match parent {
        None => Ok(None),
        Some(PyValue::Dict(map)) => Ok(map.get(key)),
        Some(other) => Err(attribute_error_get(other)),
    }
}

/// `parent.get(key) or {}`: the value when it is truthy, otherwise the empty mapping (`None`).
fn dict_get_or_empty<'a>(parent: Option<&'a PyValue>, key: &str) -> Outcome<Option<&'a PyValue>> {
    Ok(dict_get(parent, key)?.filter(|value| value.is_truthy()))
}

fn truthy(value: Option<&PyValue>) -> bool {
    value.is_some_and(PyValue::is_truthy)
}

fn optional_float(number: Option<f64>) -> PyValue {
    number.map_or(PyValue::None, PyValue::Float)
}

/// `float(value)` for a value that is an `int` or `float` (never a `bool`): `None` for anything else.
fn real_number(value: Option<&PyValue>) -> Outcome<Option<f64>> {
    match value {
        Some(PyValue::Int(number)) => Ok(Some(float_from_int(number)?)),
        Some(PyValue::Float(number)) => Ok(Some(*number)),
        _ => Ok(None),
    }
}

// ── metric readers ──────────────────────────────────────────────────────────────

/// `_stat`: `metrics[path]["successful"][field]` as a float, `None` when absent or not a number.
fn stat(metrics: &PyDict, path: &str, field: &str) -> Outcome<Option<f64>> {
    let Some(PyValue::Dict(node)) = metrics.get(path) else {
        return Ok(None);
    };
    let Some(PyValue::Dict(bucket)) = node.get(STAT_SET) else {
        return Ok(None);
    };
    real_number(bucket.get(field))
}

/// `_pct`: `metrics[path]["successful"]["percentiles"]["p99"]` as a float, `None` when absent or not a number.
pub(crate) fn p99(metrics: &PyDict, path: &str) -> Outcome<Option<f64>> {
    let Some(PyValue::Dict(node)) = metrics.get(path) else {
        return Ok(None);
    };
    let Some(PyValue::Dict(bucket)) = node.get(STAT_SET) else {
        return Ok(None);
    };
    let Some(PyValue::Dict(percentiles)) = bucket.get("percentiles") else {
        return Ok(None);
    };
    real_number(percentiles.get("p99"))
}

/// `_count`: `metrics["request_totals"][key]` when it is an `int` (not a `bool`).
fn count(metrics: &PyDict, key: &str) -> Option<BigInt> {
    let Some(PyValue::Dict(totals)) = metrics.get("request_totals") else {
        return None;
    };
    match totals.get(key) {
        Some(PyValue::Int(number)) => Some(number.clone()),
        _ => None,
    }
}

/// `_inv`: `round(1000.0 / ms, 2)` for a positive time per token.
fn inverse_rate(milliseconds: Option<f64>) -> Option<f64> {
    match milliseconds {
        Some(value) if value > 0.0 => Some(round_to_two_places(1000.0 / value)),
        _ => None,
    }
}

// ── error classification ────────────────────────────────────────────────────────

/// What [`classify_errors`] found: tool-boundary rejections against server faults.
#[derive(Clone, Debug, PartialEq)]
pub struct ErrorSplit {
    pub tool_boundary: usize,
    pub server: usize,
    /// Up to three reasons (first 200 characters) of the requests counted as server faults.
    pub unclassified_samples: Vec<PyStr>,
    /// The observation exactly as the caller gave it (the original stores whatever value arrived).
    pub server_alive_at_bench_end: PyValue,
}

/// What a liveness observation says. The original compares by identity (`is False`, `is True`), so only the JSON
/// booleans classify; `0`, `1`, `""`, `null` and every other value are an unobserved server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Liveness {
    Dead,
    Alive,
    Unobserved,
}

impl Liveness {
    pub fn of(observation: &PyValue) -> Self {
        match observation {
            PyValue::Bool(false) => Liveness::Dead,
            PyValue::Bool(true) => Liveness::Alive,
            _ => Liveness::Unobserved,
        }
    }
}

impl ErrorSplit {
    /// The `boundary_exemption` label: whether the tool-boundary exemption was allowed to apply.
    pub fn boundary_exemption(&self) -> PyStr {
        match Liveness::of(&self.server_alive_at_bench_end) {
            Liveness::Dead => lit!("disabled(server dead at bench end)"),
            Liveness::Alive => lit!("enabled(server alive)"),
            Liveness::Unobserved => lit!("enabled(liveness unobserved)"),
        }
    }

    pub fn to_value(&self) -> PyValue {
        let mut out = PyDict::with_capacity(5);
        out.insert(lit!("tool_boundary"), PyValue::int(self.tool_boundary));
        out.insert(lit!("server"), PyValue::int(self.server));
        out.insert(
            lit!("unclassified_samples"),
            PyValue::List(
                self.unclassified_samples
                    .iter()
                    .cloned()
                    .map(PyValue::Str)
                    .collect(),
            ),
        );
        out.insert(
            lit!("server_alive_at_bench_end"),
            self.server_alive_at_bench_end.clone(),
        );
        out.insert(
            lit!("boundary_exemption"),
            PyValue::Str(self.boundary_exemption()),
        );
        PyValue::Dict(out)
    }
}

/// The position `index` selects in a list of `length` items (`list[index]`), `None` where Python raises
/// `IndexError`/`TypeError`: only `int` and `bool` are valid list indices, and negative ones count from the end.
fn list_position(index: &PyValue, length: usize) -> Option<usize> {
    let number = match index {
        PyValue::Int(number) => number.clone(),
        PyValue::Bool(flag) => BigInt::from(u8::from(*flag)),
        _ => return None,
    };
    let length = BigInt::from(length);
    let position = if number.is_negative() {
        number + &length
    } else {
        number
    };
    if position.is_negative() || position >= length {
        return None;
    }
    position.to_usize()
}

/// `run["benchmarks"][index]["requests"]["errored"]`, `None` where the original's `except (KeyError, IndexError,
/// TypeError)` would answer `None`. `index` is the caller's value as it is: a list needs an integer, a mapping
/// needs a (string) key.
fn errored_records<'a>(run: &'a PyValue, index: &PyValue) -> Option<&'a PyValue> {
    let PyValue::Dict(top) = run else { return None };
    let selected = match top.get("benchmarks")? {
        PyValue::List(runs) => &runs[list_position(index, runs.len())?],
        PyValue::Dict(map) => {
            let PyValue::Str(key) = index else {
                return None;
            };
            &map.iter().find(|(name, _)| name == key)?.1
        }
        // A string indexes to a string that cannot be subscripted again.
        _ => return None,
    };
    let PyValue::Dict(run) = selected else {
        return None;
    };
    let PyValue::Dict(requests) = run.get("requests")? else {
        return None;
    };
    requests.get("errored")
}

/// Python `str.lower()` for the purpose of matching ASCII patterns: the characters whose lower-case form holds
/// ASCII letters are the ASCII capitals, U+0130 (`i` plus a combining dot) and the Kelvin sign U+212A (`k`).
/// Everything else maps to code points that cannot be part of an ASCII pattern, and lone surrogates stay as they
/// are.
fn lower_code_points(text: &PyStr) -> Vec<char> {
    let mut lowered = Vec::with_capacity(text.len());
    for &point in text.code_points() {
        match char::from_u32(point) {
            Some(character) => lowered.extend(character.to_lowercase()),
            None => lowered.push('\u{fffd}'),
        }
    }
    lowered
}

fn contains_pattern(haystack: &[char], pattern: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    haystack
        .windows(pattern.len())
        .any(|window| window == pattern.as_slice())
}

/// `classify_errors`: splits `requests.errored[]` into tool-boundary rejections and server faults.
///
/// A rejection is the tool's only when its message matches the closed pattern list **and** the request produced
/// output tokens; with `server_alive_at_bench_end is False` (the JSON `false`, not any falsy value) the exemption is void altogether. A document
/// without `requests.errored` is not split (`None`): unknown is not exempt.
pub fn classify_errors(
    run: &PyValue,
    benchmark_index: &PyValue,
    server_alive_at_bench_end: &PyValue,
) -> Outcome<Option<ErrorSplit>> {
    let Some(errored) = errored_records(run, benchmark_index) else {
        return Ok(None);
    };
    let exemption_allowed = Liveness::of(server_alive_at_bench_end) != Liveness::Dead;
    let mut tool_boundary = 0;
    let mut server = 0;
    let mut samples = Vec::new();
    let PyValue::List(records) = errored else {
        return Ok(Some(ErrorSplit {
            tool_boundary,
            server,
            unclassified_samples: samples,
            server_alive_at_bench_end: server_alive_at_bench_end.clone(),
        }));
    };
    for record in records {
        let info = dict_get_or_empty(Some(record), "info")?;
        let message = match dict_get(info, "error")? {
            Some(value) if value.is_truthy() => value.display_text(),
            _ => PyStr::default(),
        };
        let produced = if truthy(dict_get(Some(record), "output_tokens")?) {
            true
        } else {
            let metrics = dict_get_or_empty(Some(record), "output_metrics")?;
            truthy(dict_get(metrics, "text_tokens")?)
        };
        let lowered = lower_code_points(&message);
        let hit = TOOL_BOUNDARY_ERROR_PATTERNS
            .iter()
            .any(|pattern| contains_pattern(&lowered, pattern));
        if hit && produced && exemption_allowed {
            tool_boundary += 1;
        } else {
            server += 1;
            if !message.is_empty() && samples.len() < 3 {
                samples.push(PyStr::from_code_points(
                    message.code_points().iter().copied().take(200).collect(),
                ));
            }
        }
    }
    Ok(Some(ErrorSplit {
        tool_boundary,
        server,
        unclassified_samples: samples,
        server_alive_at_bench_end: server_alive_at_bench_end.clone(),
    }))
}

// ── speculative-decoding axis ───────────────────────────────────────────────────

/// The `spec` argument of [`build`]: the acceptance length (verbatim) and where it came from.
#[derive(Clone, Debug, PartialEq)]
pub struct SpecAxis {
    pub accept_len: PyValue,
    pub source: PyValue,
}

impl SpecAxis {
    /// `spec=None`: both entries absent.
    pub fn unspecified() -> Self {
        SpecAxis {
            accept_len: PyValue::None,
            source: PyValue::None,
        }
    }

    /// The axis declared absent on the command line: no acceptance length, provenance `declared-absent`.
    pub fn declared_absent() -> Self {
        SpecAxis {
            accept_len: PyValue::None,
            source: PyValue::Str(lit!("declared-absent")),
        }
    }

    /// `spec = spec or {}; spec.get("accept_len"), spec.get("source")` for whatever value the caller passed: a
    /// falsy value is the empty mapping, a mapping is read, and any other value has no `get`.
    pub fn from_argument(spec: &PyValue) -> Outcome<Self> {
        match spec {
            value if !value.is_truthy() => Ok(SpecAxis::unspecified()),
            PyValue::Dict(map) => Ok(SpecAxis {
                accept_len: map.get("accept_len").cloned().unwrap_or(PyValue::None),
                source: map.get("source").cloned().unwrap_or(PyValue::None),
            }),
            other => Err(attribute_error_get(other)),
        }
    }

    /// The mapping form (`{"accept_len": ..., "source": ...}`) a caller hands to [`build`].
    pub fn to_value(&self) -> PyValue {
        let mut map = PyDict::with_capacity(2);
        map.insert(lit!("accept_len"), self.accept_len.clone());
        map.insert(lit!("source"), self.source.clone());
        PyValue::Dict(map)
    }
}

/// A Python number as the comparison operators see it: `bool` is an `int`.
enum Real {
    Int(BigInt),
    Float(f64),
}

impl Real {
    fn of(value: &PyValue) -> Option<Real> {
        match value {
            PyValue::Int(number) => Some(Real::Int(number.clone())),
            PyValue::Bool(flag) => Some(Real::Int(BigInt::from(u8::from(*flag)))),
            PyValue::Float(number) => Some(Real::Float(*number)),
            _ => None,
        }
    }
}

/// `rate <= limit` for a float `rate` and any value the caller declared as the ceiling: exact float/int
/// comparison, `TypeError` for a value that does not order against a float (including `None`).
fn rate_within(rate: f64, limit: &PyValue) -> Outcome<bool> {
    match Real::of(limit) {
        Some(Real::Float(limit)) => Ok(rate <= limit),
        Some(Real::Int(limit)) => Ok(if rate.is_nan() {
            false
        } else if rate.is_infinite() {
            rate < 0.0
        } else {
            // rate <= n  <=>  ceil(rate) <= n for an integer n.
            num_traits::FromPrimitive::from_f64(rate.ceil())
                .is_some_and(|ceiling: BigInt| ceiling <= limit)
        }),
        None => Err(Failure::new(
            "TypeError",
            &format!(
                "'<=' not supported between instances of 'float' and '{}'",
                limit.type_name()
            ),
        )),
    }
}

/// `bool(accept_len and accept_len > 1.05)`.
fn spec_enabled(accept_len: &PyValue) -> Outcome<bool> {
    if !accept_len.is_truthy() {
        return Ok(false);
    }
    match accept_len {
        PyValue::Float(number) => Ok(*number > 1.05),
        PyValue::Int(number) => Ok(*number > BigInt::from(1)),
        // `True > 1.05` is false.
        PyValue::Bool(_) => Ok(false),
        other => Err(Failure::new(
            "TypeError",
            &format!(
                "'>' not supported between instances of '{}' and 'float'",
                other.type_name()
            ),
        )),
    }
}

/// `inherit_accept_len`: the spec axis from a `vllm bench serve` document. `path` is only named in messages and
/// in the provenance.
///
/// The source must look like a bench result: a document without an acceptance length *and* without `completed`
/// (for example a lite raw pointer document) is refused, because inheriting `None` from it silently was the root
/// of every spec-declared sweep being recorded as missing.
pub fn inherit_accept_len(src: &PyValue, path: &PyValue) -> Outcome<SpecAxis> {
    let with_path = |prefix: &str, suffix: &str| {
        let mut text = PyStr::new(prefix);
        text.push_py(&path.display_text());
        text.push_str(suffix);
        text
    };
    let PyValue::Dict(document) = src else {
        return Err(Failure::with_message(
            "ValueError",
            with_path("--accept-len-src 는 JSON 객체여야 한다: ", ""),
        ));
    };
    let mut value = document.get("spec_decode_acceptance_length");
    if matches!(value, None | Some(PyValue::None)) {
        value = document.get("accept_len");
    }
    let absent = matches!(value, None | Some(PyValue::None));
    if absent && !document.contains_key("completed") {
        let hint = match document.get("bench_warm_json") {
            Some(pointer) => format!(
                " — lite raw 포인터 문서로 보인다(bench_warm_json={}). 가리키는 warm JSON 을 넘겨라",
                pointer.repr()
            ),
            None => String::new(),
        };
        return Err(Failure::with_message(
            "ValueError",
            with_path(
                "--accept-len-src 가 `vllm bench serve` 결과가 아니다(수용길이 키도 completed 도 없다): ",
                &hint,
            ),
        ));
    }
    let accept_len = match value {
        None | Some(PyValue::None) => PyValue::None,
        Some(PyValue::Int(number)) => PyValue::Float(float_from_int(number)?),
        Some(PyValue::Float(number)) => PyValue::Float(*number),
        Some(other) => {
            return Err(Failure::new(
                "ValueError",
                &format!(
                    "--accept-len-src 의 수용길이가 수가 아니다: {}",
                    other.repr()
                ),
            ))
        }
    };
    Ok(SpecAxis {
        accept_len,
        source: PyValue::Str(with_path("inherited(", ")")),
    })
}

// ── build ───────────────────────────────────────────────────────────────────────

/// The arguments of [`build`] besides the document, each exactly as the Python caller passed it.
///
/// The original function takes arbitrary JSON values here and either uses them, stores them in the output or
/// fails at the operation that needs them, in a fixed order; nothing is validated up front. So every field is a
/// raw [`PyValue`] and the checks live at their operation (`benchmark_index` in the range test, `engine_max`
/// in the division, `spec` at its `.get`, `max_error_rate` in the validity comparison). The command line's own
/// typed values (`int`, `float`, `bool`, `None`) go through the same fields.
#[derive(Clone, Debug)]
pub struct BuildRequest {
    pub benchmark_index: PyValue,
    /// The highest generation throughput the engine log reports (`None` when there is none).
    pub engine_max: PyValue,
    pub spec: PyValue,
    pub max_error_rate: PyValue,
    pub server_alive_at_bench_end: PyValue,
}

impl Default for BuildRequest {
    /// The original signature's defaults: `0`, `None`, `None`, `0.0`, `None`.
    fn default() -> Self {
        BuildRequest {
            benchmark_index: PyValue::int(0),
            engine_max: PyValue::None,
            spec: PyValue::None,
            max_error_rate: PyValue::Float(0.0),
            server_alive_at_bench_end: PyValue::None,
        }
    }
}

/// `if not 0 <= benchmark_index < len(runs)` then `runs[benchmark_index]`, with the original's exceptions in
/// the original's order: `TypeError` when the value does not order against an int, `ValueError` for a number
/// out of range (its `%d` rendering truncates a float), `TypeError` from list indexing for a float that was
/// in range.
fn select_run(index: &PyValue, length: usize) -> Outcome<BigInt> {
    let Some(number) = Real::of(index) else {
        return Err(Failure::new(
            "TypeError",
            &format!(
                "'<=' not supported between instances of 'int' and '{}'",
                index.type_name()
            ),
        ));
    };
    let length_big = BigInt::from(length);
    let inside = match &number {
        Real::Int(value) => !value.is_negative() && *value < length_big,
        Real::Float(value) => *value >= 0.0 && *value < length as f64,
    };
    if !inside {
        let shown = match &number {
            Real::Int(value) => value.to_string(),
            Real::Float(value) if value.is_nan() => {
                return Err(Failure::new(
                    "ValueError",
                    "cannot convert float NaN to integer",
                ))
            }
            Real::Float(value) if value.is_infinite() => {
                return Err(Failure::new(
                    "OverflowError",
                    "cannot convert float infinity to integer",
                ))
            }
            Real::Float(value) => crate::pynumber::int_from_float(*value)?.to_string(),
        };
        return Err(Failure::new(
            "ValueError",
            &format!("--benchmark-index {shown} 가 범위 밖이다(benchmarks {length}건)"),
        ));
    }
    match number {
        Real::Int(value) => Ok(value),
        Real::Float(_) => Err(Failure::new(
            "TypeError",
            "list indices must be integers or slices, not float",
        )),
    }
}

/// `streams[...]`: the declared concurrency of the selected run, where `streams` is whatever the document holds
/// (truthy). A list yields its element, a string its character, a mapping fails with `KeyError`.
fn declared_concurrency(
    streams: Option<&PyValue>,
    index: &BigInt,
    index_repr: &str,
) -> Outcome<Option<PyValue>> {
    let Some(streams) = streams else {
        return Ok(None);
    };
    let length = match streams {
        PyValue::List(items) => items.len(),
        PyValue::Str(text) => text.len(),
        PyValue::Dict(map) => map.len(),
        other => {
            return Err(Failure::new(
                "TypeError",
                &format!("object of type '{}' has no len()", other.type_name()),
            ))
        }
    };
    let (position, position_repr) = if *index < BigInt::from(length) {
        (index.clone(), index_repr.to_string())
    } else {
        (BigInt::zero(), "0".to_string())
    };
    match streams {
        PyValue::List(items) => Ok(items
            .get(position.to_usize().unwrap_or(usize::MAX))
            .cloned()),
        PyValue::Str(text) => {
            let at = position.to_usize().unwrap_or(usize::MAX);
            Ok(text
                .code_points()
                .get(at)
                .map(|&point| PyValue::Str(PyStr::from_code_points(vec![point]))))
        }
        _ => Err(Failure::new("KeyError", &position_repr)),
    }
}

/// `float(a) / float(b)` for ints, `b` non-zero.
fn quotient(numerator: &BigInt, denominator: &BigInt) -> Outcome<f64> {
    Ok(float_from_int(numerator)? / float_from_int(denominator)?)
}

/// `build`: a GuideLLM report document to the measurement object, in the program's key order.
pub fn build(doc: &PyValue, request: &BuildRequest) -> Outcome<PyValue> {
    let PyValue::Dict(document) = doc else {
        return Err(Failure::new(
            "ValueError",
            "benchmarks.json 은 JSON 객체여야 한다",
        ));
    };
    let runs = match document.get("benchmarks") {
        Some(PyValue::List(runs)) if !runs.is_empty() => runs,
        _ => {
            return Err(Failure::new(
                "ValueError",
                "benchmarks[] 가 비었다 — 측정이 없다",
            ))
        }
    };
    let index = select_run(&request.benchmark_index, runs.len())?;
    let index = &index;
    let run = &runs[index.to_usize().expect("index is within the run list")];
    let PyValue::Dict(run_map) = run else {
        return Err(attribute_error_get(run));
    };
    let Some(PyValue::Dict(metrics)) = run_map.get("metrics") else {
        return Err(Failure::new(
            "ValueError",
            &format!("benchmarks[{index}].metrics 가 없다"),
        ));
    };

    let tpot_median = stat(metrics, "time_per_output_token_ms", "median")?;
    let tpot_mean = stat(metrics, "time_per_output_token_ms", "mean")?;
    let request_latency_p99_s = p99(metrics, "request_latency")?;
    let decode_tps = inverse_rate(tpot_median);

    let successful = count(metrics, "successful");
    let errored = count(metrics, "errored");
    let incomplete = count(metrics, "incomplete");
    let total = count(metrics, "total");
    // `incomplete` is neither success nor failure: it counts as failed for validity, and stays listed apart.
    let failed = match (&errored, &incomplete) {
        (Some(errored), Some(incomplete)) => Some(errored + incomplete),
        _ => None,
    };

    let config = dict_get_or_empty(Some(run), "config")?;
    let profile = dict_get_or_empty(config, "profile")?;
    let streams = dict_get_or_empty(profile, "streams")?;
    let mut max_concurrency =
        declared_concurrency(streams, index, &request.benchmark_index.repr())?
            .unwrap_or(PyValue::None);
    if matches!(max_concurrency, PyValue::None) {
        max_concurrency = optional_float(stat(metrics, "request_concurrency", "median")?);
    }

    // `if engine_max and decode_tps:` - the engine figure is whatever the caller gave (a number, a bool, or a
    // value that only fails when it is divided).
    let agreement = match decode_tps {
        Some(client) if request.engine_max.is_truthy() && client != 0.0 => {
            let engine = match &request.engine_max {
                PyValue::Float(number) => *number,
                PyValue::Int(number) => float_from_int(number)?,
                PyValue::Bool(flag) => f64::from(u8::from(*flag)),
                other => {
                    return Err(Failure::new(
                        "TypeError",
                        &format!(
                            "unsupported operand type(s) for /: '{}' and 'float'",
                            other.type_name()
                        ),
                    ))
                }
            };
            let mut map = PyDict::with_capacity(3);
            map.insert(lit!("engine_max"), request.engine_max.clone());
            map.insert(lit!("client_decode_tps"), PyValue::Float(client));
            map.insert(
                lit!("ratio"),
                PyValue::Float(round_to_two_places(engine / client)),
            );
            PyValue::Dict(map)
        }
        _ => PyValue::None,
    };

    let total_is_nonzero = total.as_ref().filter(|total| !total.is_zero());
    let error_rate = match (&failed, total_is_nonzero) {
        (Some(failed), Some(total)) => quotient(failed, total)?,
        _ => 0.0,
    };
    // A tool-boundary rejection is not a server fault: split them, but swallow neither.
    let split = classify_errors(
        doc,
        &request.benchmark_index,
        &request.server_alive_at_bench_end,
    )?;
    let (server_failed, error_split_source) = match &split {
        None => (
            failed.clone(),
            lit!("unavailable(requests.errored 부재 — 전량을 서버 오류로 센다)"),
        ),
        Some(split) => {
            // The list may be longer than the aggregate (warm-up requests are left out of the latter); summing
            // them over-counts server faults, which is the safe direction.
            let incomplete_count = incomplete.clone().unwrap_or_else(BigInt::zero);
            (
                Some(BigInt::from(split.server) + incomplete_count),
                lit!("measured(requests.errored[].info.error + output_tokens)"),
            )
        }
    };
    let server_error_rate = match (&server_failed, total_is_nonzero) {
        (Some(server_failed), Some(total)) => quotient(server_failed, total)?,
        _ => 0.0,
    };

    // `spec = spec or {}` and its `.get`s come after the error split and before anything of the output.
    let spec = SpecAxis::from_argument(&request.spec)?;
    let tool_version = match document.get("metadata") {
        Some(PyValue::Dict(metadata)) => metadata.get("guidellm_version").cloned(),
        _ => None,
    }
    .unwrap_or(PyValue::None);
    let version_known = tool_version.is_truthy();

    let int_or_none = |number: &Option<BigInt>| number.clone().map_or(PyValue::None, PyValue::Int);
    let (tool_boundary_errors, server_errors, unclassified_samples) = match &split {
        None => (PyValue::None, PyValue::None, PyValue::None),
        Some(split) => (
            PyValue::int(split.tool_boundary),
            PyValue::int(split.server),
            PyValue::List(
                split
                    .unclassified_samples
                    .iter()
                    .cloned()
                    .map(PyValue::Str)
                    .collect(),
            ),
        ),
    };
    let (server_alive, exemption) = match &split {
        None => (PyValue::None, lit!("n/a(분류 불가)")),
        Some(split) => (
            split.server_alive_at_bench_end.clone(),
            split.boundary_exemption(),
        ),
    };

    Ok(PyValue::Dict(PyDict::from_distinct_entries([
        (lit!("decode_tps"), optional_float(decode_tps)),
        (
            lit!("decode_tps_mean"),
            optional_float(inverse_rate(tpot_mean)),
        ),
        (
            lit!("output_throughput"),
            optional_float(stat(metrics, "output_tokens_per_second", "mean")?),
        ),
        (
            lit!("total_token_throughput"),
            optional_float(stat(metrics, "tokens_per_second", "mean")?),
        ),
        (
            lit!("ttft_ms_median"),
            optional_float(stat(metrics, "time_to_first_token_ms", "median")?),
        ),
        (
            lit!("ttft_ms_mean"),
            optional_float(stat(metrics, "time_to_first_token_ms", "mean")?),
        ),
        (
            lit!("itl_ms_median"),
            optional_float(stat(metrics, "inter_token_latency_ms", "median")?),
        ),
        (lit!("tpot_ms_median"), optional_float(tpot_median)),
        (
            lit!("ttft_ms_p99"),
            optional_float(p99(metrics, "time_to_first_token_ms")?),
        ),
        (
            lit!("ttft_ms_std"),
            optional_float(stat(metrics, "time_to_first_token_ms", "std_dev")?),
        ),
        (
            lit!("itl_ms_p99"),
            optional_float(p99(metrics, "inter_token_latency_ms")?),
        ),
        (
            lit!("itl_ms_std"),
            optional_float(stat(metrics, "inter_token_latency_ms", "std_dev")?),
        ),
        (
            lit!("tpot_ms_p99"),
            optional_float(p99(metrics, "time_per_output_token_ms")?),
        ),
        (
            lit!("tpot_ms_std"),
            optional_float(stat(metrics, "time_per_output_token_ms", "std_dev")?),
        ),
        // GuideLLM reports request latency in seconds; the output contract is milliseconds.
        (
            lit!("request_latency_ms_p99"),
            optional_float(request_latency_p99_s.map(|seconds| seconds * 1000.0)),
        ),
        (lit!("accept_len"), spec.accept_len.clone()),
        (
            lit!("spec_on"),
            PyValue::Bool(spec_enabled(&spec.accept_len)?),
        ),
        (lit!("completed"), int_or_none(&successful)),
        (lit!("failed"), int_or_none(&failed)),
        (lit!("max_concurrency"), max_concurrency),
        (lit!("num_prompts"), int_or_none(&total)),
        (
            lit!("engine_gen_throughput_max"),
            request.engine_max.clone(),
        ),
        (lit!("client_engine_agreement"), agreement),
        (lit!("error_rate"), PyValue::Float(error_rate)),
        (lit!("server_error_rate"), PyValue::Float(server_error_rate)),
        (lit!("tool_boundary_errors"), tool_boundary_errors),
        (lit!("server_errors"), server_errors),
        (lit!("unclassified_error_samples"), unclassified_samples),
        (lit!("error_split_source"), PyValue::Str(error_split_source)),
        (lit!("server_alive_at_bench_end"), server_alive),
        (lit!("boundary_exemption"), PyValue::Str(exemption)),
        (
            lit!("max_error_rate_declared"),
            request.max_error_rate.clone(),
        ),
        // `bool(successful and decode_tps is not None and server_error_rate <= max_error_rate)`: the ceiling is
        // compared only when the first two hold, so a ceiling that does not order fails only then.
        (
            lit!("measurement_ok"),
            PyValue::Bool(match &successful {
                Some(successful) if !successful.is_zero() && decode_tps.is_some() => {
                    rate_within(server_error_rate, &request.max_error_rate)?
                }
                _ => false,
            }),
        ),
        (lit!("bench_tool"), PyValue::Str(lit!("guidellm"))),
        (lit!("bench_tool_version"), tool_version),
        (
            lit!("bench_tool_version_source"),
            PyValue::Str(if version_known {
                lit!("measured(benchmarks.json metadata.guidellm_version)")
            } else {
                lit!("unavailable")
            }),
        ),
        (lit!("spec_axis_source"), spec.source.clone()),
        (lit!("stat_set"), PyValue::Str(lit!("successful"))),
        (lit!("errored"), int_or_none(&errored)),
        (lit!("incomplete"), int_or_none(&incomplete)),
    ])))
}

// ── prefix-cache observation ────────────────────────────────────────────────────

/// What [`carry_prefix_cache`] decided: the record for the output and the warning, if one is due.
#[derive(Debug, PartialEq)]
pub struct PrefixCacheCarry {
    pub record: PyValue,
    pub warning: Option<PyStr>,
}

/// `prefix_cache_run` after the file has been loaded: a mapping yields a record holding only the closed key list
/// (in that list's order); anything else yields `None`. A hit rate of at least 0.5 only warns (never blocks).
/// `isinstance(rate, (int, float))` includes `bool`, as in Python.
pub fn carry_prefix_cache(observation: &PyValue) -> Outcome<PrefixCacheCarry> {
    let PyValue::Dict(source) = observation else {
        return Ok(PrefixCacheCarry {
            record: PyValue::None,
            warning: None,
        });
    };
    let mut record = PyDict::with_capacity(PREFIX_CACHE_KEYS.len());
    for key in PREFIX_CACHE_KEYS {
        if let Some(value) = source.get(key) {
            record.set(key, value.clone());
        }
    }
    let rate = match record.get("hit_rate_run") {
        Some(PyValue::Float(number)) if *number >= PREFIX_CACHE_WARN => Some(*number),
        Some(PyValue::Int(number)) if *number >= BigInt::from(1) => Some(float_from_int(number)?),
        Some(PyValue::Bool(true)) => Some(1.0),
        _ => None,
    };
    let warning = rate.map(|rate| {
        PyStr::new(&format!(
            "[parse_guidellm] WARN prefix cache 실행 적중률 {} — 같은 프롬프트 반복 또는 다른 트래픽 혼입 의심(requests_delta={}). TTFT 가 cache 도움을 받았을 수 있다.\n",
            format_fixed(rate, 2),
            record.get("requests_delta").map_or_else(|| "None".to_string(), PyValue::repr),
        ))
    });
    Ok(PrefixCacheCarry {
        record: PyValue::Dict(record),
        warning,
    })
}
