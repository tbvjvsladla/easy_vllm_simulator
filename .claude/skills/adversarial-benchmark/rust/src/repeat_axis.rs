//! The pure API of the full-bench repeat axis: a Rust port of the operations of
//! `.claude/skills/adversarial-benchmark/scripts/repeat_axis.py` that touch no file.
//!
//! * [`parse_repeats`] - the value domain of a repeat count (an integer at or above [`FULL_REPEATS_MIN`]);
//! * [`band_pct`] - the reproducibility band `(max-min)/mean*100` of the completed runs' decode throughput;
//! * [`summarize`] - the per-level repeat summary and the place where a sweep stopped;
//! * [`repetition_established`] - whether the repeat condition of the full definition stands at the judgment point.
//!
//! The Python functions accept any JSON value, so the arguments here are [`PyValue`]s and the errors are
//! [`Failure`]s carrying the exception class and message the original raises. Python semantics that show: `bool`
//! is an `int` for `isinstance` but is excluded where the original excludes it, `sum` of floats is CPython 3.12's
//! compensated summation, `sorted` compares keys in CPython's order, and dict keys are Python keys.
//!
//! The file operations live in [`crate::repeat_fs`], the campaign declaration in [`crate::repeat_owner`].

use crate::guidellm::Failure;
use crate::pynumber::{float_from_int, int_from_str, round_to_places};
use crate::pyops::{compare, Compare};
use crate::pysort::{py_eq, sorted_by_key};
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use crate::repeat_text::cat;
use crate::FULL_REPEATS_MIN;
use num_bigint::BigInt;

// ── constants of the original module ────────────────────────────────────────────

/// Campaign-declaration owner, relative to the repository root.
pub const CAMPAIGN_INIT_REL: &str = ".claude/skills/terraforming_node/scripts/campaign_init.sh";
/// The campaign schema whose `budgets.repeats.minimum` is cross-checked against [`FULL_REPEATS_MIN`].
pub const SCHEMA_REL: &str = "campaigns/_template/campaign.schema.json";
pub const REPEAT_KIND_WARM: &str = "warm-rerun";
pub const REPEAT_KIND_COLD: &str = "cold-restart";
pub const REPEAT_KINDS: [&str; 2] = [REPEAT_KIND_WARM, REPEAT_KIND_COLD];
/// The kinds an executor really produces; `cold-restart` is a declared slot only.
pub const IMPLEMENTED_REPEAT_KINDS: [&str; 1] = [REPEAT_KIND_WARM];
pub const RUN_META_NAME: &str = "repeat_run.json";
/// The pattern of a run directory name, `^run_(\d{2,})$` (the original's `RUN_DIR_RE`).
pub const RUN_DIR_PATTERN: &str = r"^run_(\d{2,})$";
pub const SIGNAL_RUN_FAILED: &str = "run_failed";
pub const STOP_KIND_REPEAT_BREAK: &str = "repeat-break";
pub const STOP_KIND_CLAMP: &str = "clamp";
/// The metrics copied into every `runs[]` entry.
pub const RUN_METRIC_KEYS: [&str; 9] = [
    "decode_tps",
    "output_throughput",
    "ttft_ms_median",
    "itl_ms_median",
    "tpot_ms_median",
    "completed",
    "failed",
    "server_error_rate",
    "accept_len",
];
pub const BAND_METRIC: &str = "decode_tps";
pub const BAND_FORMULA: &str = "(max-min)/mean*100";
/// The keys `aggregate_level` owns in a representative `measured.json` (the original's `_AGG_KEYS`).
pub const AGGREGATE_KEYS: [&str; 11] = [
    "runs",
    "repeats_requested",
    "repeats_requested_source",
    "repeats_completed",
    "repeat_kind",
    "representative_run",
    "representative_run_rule",
    "repro_band_pct",
    "repro_band_source",
    "repro_band_metric",
    "repro_band_formula",
];
const SUMMARY_NOTE: &str =
    "raw 요약과 즉시 신호다 — bench_mode(full|lite)·downgrade_reason 확정은 classify_cell.sh(post-hoc)";
const REPEAT_BREAK_WINDOW_RULE: &str = "직전 run 시작 ~ 끊긴 run 끝(사인 확정은 classify_cell)";
const CLAMP_WINDOW_RULE: &str =
    "직전 run(아래 측정 레벨의 마지막 run) 시작 ~ 클램프 run 끝(사인 확정은 classify_cell)";

/// `RepeatAxisError`: the repeat-axis input does not hold; callers get the refusal instead of a default.
pub fn repeat_axis_error(message: PyStr) -> Failure {
    Failure::with_message("RepeatAxisError", message)
}

fn floor_value() -> BigInt {
    BigInt::from(FULL_REPEATS_MIN)
}

// ── small Python helpers ────────────────────────────────────────────────────────

fn dict_of(value: &PyValue) -> Option<&PyDict> {
    match value.peel() {
        PyValue::Dict(map) => Some(map),
        _ => None,
    }
}

fn truthy(value: Option<&PyValue>) -> bool {
    value.is_some_and(PyValue::is_truthy)
}

fn or_none(value: Option<&PyValue>) -> PyValue {
    value.cloned().unwrap_or(PyValue::None)
}

fn show(value: Option<&PyValue>) -> PyStr {
    value.map_or_else(|| PyStr::new("None"), PyValue::display_text)
}

fn is_text(value: Option<&PyValue>, text: &str) -> bool {
    matches!(value.map(PyValue::peel), Some(PyValue::Str(own)) if own.equals(text))
}

fn is_true(value: Option<&PyValue>) -> bool {
    matches!(value.map(PyValue::peel), Some(PyValue::Bool(true)))
}

/// `isinstance(value, int) and not isinstance(value, bool)`.
fn plain_int(value: &PyValue) -> Option<&BigInt> {
    value.as_plain_int()
}

/// `isinstance(value, int)` (a `bool` counts) as the integer it is.
fn int_or_bool(value: &PyValue) -> Option<BigInt> {
    match value.peel() {
        PyValue::Int(number) => Some(number.clone()),
        PyValue::Bool(flag) => Some(BigInt::from(u8::from(*flag))),
        _ => None,
    }
}

fn not_iterable(value: &PyValue) -> Failure {
    Failure::new(
        "TypeError",
        &format!("'{}' object is not iterable", value.type_name()),
    )
}

/// The elements `for x in value` yields, as values. A dict yields its keys and a string its characters.
fn elements_of(value: &PyValue) -> Result<Vec<PyValue>, Failure> {
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
        other => Err(not_iterable(other)),
    }
}

// ── parse_repeats ───────────────────────────────────────────────────────────────

/// The value domain of a repeat count: an integer at or above [`FULL_REPEATS_MIN`], as an `int` or as text of
/// ASCII digits (surrounding whitespace allowed). Anything below the floor is refused, not marked `lite`: a lower
/// count would mix "the repeat did not hold" (a machine event) with "fewer runs were asked for" (a declaration).
/// `where_` names the source in the message.
pub fn parse_repeats(raw: &PyValue, where_: &PyValue) -> Result<BigInt, Failure> {
    let value = match raw.peel() {
        PyValue::Str(text) => {
            let trimmed = text.strip();
            let ascii_digits = !trimmed.is_empty()
                && trimmed
                    .code_points()
                    .iter()
                    .all(|point| (u32::from('0')..=u32::from('9')).contains(point));
            if !ascii_digits {
                return Err(not_an_integer(raw, where_));
            }
            int_from_str(&trimmed)?
        }
        PyValue::Int(number) => number.clone(),
        _ => return Err(not_an_integer(raw, where_)),
    };
    if value < floor_value() {
        return Err(repeat_axis_error(cat!(
            where_.display_text(),
            "=",
            value.to_string(),
            format!(
                " 는 full 정의(반복 ≥{FULL_REPEATS_MIN}) 위반이다 — 반복 수를 낮춰 full 을 선언하지 않는다. 반복이 성립하지 않으면 선언이 아니라 기계 이벤트(run 실패·블랙박스 kill)가 셀을 lite 로 강등한다(classify_cell). lite 만 재려면 lite_bench.sh 를 쓰라."
            )
        )));
    }
    Ok(value)
}

fn not_an_integer(raw: &PyValue, where_: &PyValue) -> Failure {
    repeat_axis_error(cat!(
        where_.display_text(),
        "=",
        raw.repr(),
        " 는 정수가 아니다"
    ))
}

// ── band_pct ────────────────────────────────────────────────────────────────────

/// The reproducibility band and where it comes from.
#[derive(Clone, Debug, PartialEq)]
pub struct Band {
    /// `None` when fewer than two completed runs (or a non-positive mean) leave it undefined.
    pub percent: Option<f64>,
    pub source: PyStr,
}

impl Band {
    /// The Python return value `(pct, source)`.
    pub fn to_value(&self) -> PyValue {
        PyValue::Tuple(vec![
            self.percent.map_or(PyValue::None, PyValue::Float),
            PyValue::Str(self.source.clone()),
        ])
    }
}

/// CPython 3.12 `sum(floats)`: the first value is added to the integer start `0`, then Neumaier-compensated.
fn python_float_sum(values: &[f64]) -> f64 {
    let Some((&first, rest)) = values.split_first() else {
        return 0.0;
    };
    let mut total = 0.0 + first;
    let mut compensation = 0.0;
    for &item in rest {
        let next = total + item;
        if total.abs() >= item.abs() {
            compensation += (total - next) + item;
        } else {
            compensation += (item - next) + total;
        }
        total = next;
    }
    if compensation != 0.0 && compensation.is_finite() {
        total += compensation;
    }
    total
}

/// The finite `int`/`float` members of `values` (not `bool`), as floats; `None` for an element the iteration
/// cannot reach. An `int` beyond the double range is an `OverflowError` at the element that has it.
fn finite_numbers(values: &PyValue) -> Result<Vec<f64>, Failure> {
    let mut numbers = Vec::new();
    let mut take = |item: &PyValue| -> Result<(), Failure> {
        let number = match item.peel() {
            PyValue::Int(value) => float_from_int(value)?,
            PyValue::Float(value) => *value,
            _ => return Ok(()),
        };
        if number.is_finite() {
            numbers.push(number);
        }
        Ok(())
    };
    match values.peel() {
        PyValue::List(items) | PyValue::Tuple(items) => {
            for item in items {
                take(item)?;
            }
        }
        // A dict yields its keys and a string its characters: none is a number.
        PyValue::Dict(_) | PyValue::Str(_) => {}
        PyValue::Bytes(bytes) => {
            for &byte in bytes {
                take(&PyValue::int(byte))?;
            }
        }
        other => return Err(not_iterable(other)),
    }
    Ok(numbers)
}

/// The reproducibility band `(max-min)/mean*100` of the completed runs' throughput. It is undefined below two
/// runs. The formula is the one the campaigns wrote their "spread" with (audit_26091323 §2.3: 11.759/11.971/11.301
/// -> 5.74%); a different formula under the same name would silently break comparison with old records.
pub fn band_pct(values: &PyValue) -> Result<Band, Failure> {
    let numbers = finite_numbers(values)?;
    let count = numbers.len();
    if count < 2 {
        return Ok(Band {
            percent: None,
            source: cat!(format!(
                "unavailable(n={count} — 산포는 완주 2회 이상에서만 정의된다)"
            )),
        });
    }
    let mean = python_float_sum(&numbers) / count as f64;
    if mean <= 0.0 {
        return Ok(Band {
            percent: None,
            source: cat!(format!(
                "unavailable(n={count} — 평균이 0 이하라 상대 산포가 정의되지 않는다)"
            )),
        });
    }
    let largest = numbers.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let smallest = numbers.iter().copied().fold(f64::INFINITY, f64::min);
    Ok(Band {
        percent: Some(round_to_places((largest - smallest) / mean * 100.0, 4)),
        source: cat!(format!("measured(n={count})")),
    })
}

// ── summarize ───────────────────────────────────────────────────────────────────

/// Where a sweep stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopKind {
    /// A run at or above the second of a level the sweep had measured broke.
    RepeatBreak,
    /// The first run of a level above the measured ones broke: the adaptive upper clamp.
    Clamp,
}

impl StopKind {
    pub fn as_str(self) -> &'static str {
        match self {
            StopKind::RepeatBreak => STOP_KIND_REPEAT_BREAK,
            StopKind::Clamp => STOP_KIND_CLAMP,
        }
    }
}

/// The immediate signal of the place a sweep stopped. Which cause broke the run is the classifier's to decide.
#[derive(Clone, Debug, PartialEq)]
pub struct Stop {
    pub kind: StopKind,
    pub level: PyValue,
    pub run: PyValue,
    pub detail: PyStr,
    pub window_start_utc: PyValue,
    pub window_end_utc: PyValue,
    pub window_rule: &'static str,
}

impl Stop {
    pub fn to_value(&self) -> PyValue {
        PyValue::Dict(PyDict::from_distinct_entries([
            (PyStr::new("kind"), PyValue::str(self.kind.as_str())),
            (PyStr::new("level"), self.level.clone()),
            (PyStr::new("run"), self.run.clone()),
            (PyStr::new("signal"), PyValue::str(SIGNAL_RUN_FAILED)),
            (PyStr::new("detail"), PyValue::Str(self.detail.clone())),
            (
                PyStr::new("window_start_utc"),
                self.window_start_utc.clone(),
            ),
            (PyStr::new("window_end_utc"), self.window_end_utc.clone()),
            (PyStr::new("window_rule"), PyValue::str(self.window_rule)),
        ]))
    }
}

/// Everything [`summarize`] reads: the sweep index's `levels[]` and the caller's repeat request, kept as given.
#[derive(Clone, Copy, Debug)]
pub struct SummarizeRequest<'a> {
    pub levels: &'a PyValue,
    pub requested: &'a PyValue,
    pub requested_source: &'a PyValue,
    pub kind: &'a PyValue,
    /// The boundary fact of a level whose first run broke (the sweep driver reads it from that run's metadata).
    pub clamp_run: &'a PyValue,
}

/// A dict keyed by `str(level)` that keeps the position of a key assigned twice.
#[derive(Clone, Debug, PartialEq)]
struct KeyedByLevel<V>(Vec<(PyStr, V)>);

impl<V> KeyedByLevel<V> {
    fn assign(&mut self, key: PyStr, value: V) {
        match self.0.iter_mut().find(|(existing, _)| *existing == key) {
            Some(slot) => slot.1 = value,
            None => self.0.push((key, value)),
        }
    }
}

/// The repeat summary of a sweep. Field order is the key order of the original's dict.
#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    pub requested: PyValue,
    pub requested_source: PyValue,
    pub kind: PyValue,
    /// `str(level)` -> completed runs.
    completed_by_level: KeyedByLevel<usize>,
    pub short_levels: Vec<BigInt>,
    pub runs_attempted: usize,
    /// `str(level)` -> the level's recorded band (whatever the measurement file held).
    band_pct_by_level: KeyedByLevel<PyValue>,
    pub stop: Option<Stop>,
    /// The caller's clamp fact, passed through when it is an object.
    pub clamp_run: Option<PyValue>,
    pub unrecorded_levels: Vec<PyValue>,
}

impl Summary {
    pub fn completed_min(&self) -> Option<usize> {
        self.completed_by_level
            .0
            .iter()
            .map(|(_, count)| *count)
            .min()
    }

    pub fn completed_by_level(&self) -> impl Iterator<Item = (&PyStr, usize)> {
        self.completed_by_level
            .0
            .iter()
            .map(|(level, count)| (level, *count))
    }

    pub fn to_value(&self) -> PyValue {
        let mut by_level = PyDict::with_capacity(self.completed_by_level.0.len());
        for (level, count) in &self.completed_by_level.0 {
            by_level.insert(level.clone(), PyValue::int(*count));
        }
        let mut bands = PyDict::with_capacity(self.band_pct_by_level.0.len());
        for (level, band) in &self.band_pct_by_level.0 {
            bands.insert(level.clone(), band.clone());
        }
        let mut out = PyDict::with_capacity(15);
        out.set("requested", self.requested.clone());
        out.set("requested_source", self.requested_source.clone());
        out.set("kind", self.kind.clone());
        out.set("full_min", PyValue::int(FULL_REPEATS_MIN));
        out.set("completed_by_level", PyValue::Dict(by_level));
        out.set(
            "completed_min",
            self.completed_min().map_or(PyValue::None, PyValue::int),
        );
        out.set(
            "short_levels",
            PyValue::List(
                self.short_levels
                    .iter()
                    .cloned()
                    .map(PyValue::Int)
                    .collect(),
            ),
        );
        out.set("runs_attempted", PyValue::int(self.runs_attempted));
        out.set("band_pct_by_level", PyValue::Dict(bands));
        out.set("band_metric", PyValue::str(BAND_METRIC));
        out.set("band_formula", PyValue::str(BAND_FORMULA));
        out.set(
            "stop",
            self.stop.as_ref().map_or(PyValue::None, Stop::to_value),
        );
        out.set("clamp_run", self.clamp_run.clone().unwrap_or(PyValue::None));
        out.set(
            "unrecorded_levels",
            PyValue::List(self.unrecorded_levels.clone()),
        );
        out.set("_note", PyValue::str(SUMMARY_NOTE));
        PyValue::Dict(out)
    }
}

/// `x.get(key) or 0` as a sort key.
fn key_or_zero(map: &PyDict, key: &str) -> PyValue {
    match map.get(key) {
        Some(value) if value.is_truthy() => value.clone(),
        _ => PyValue::int(0),
    }
}

/// The dict members of `levels or []`: `levels` is iterated only when it is truthy.
fn dict_members(members: &PyValue) -> Result<Vec<&PyDict>, Failure> {
    if !members.is_truthy() {
        return Ok(Vec::new());
    }
    match members.peel() {
        PyValue::List(items) | PyValue::Tuple(items) => {
            Ok(items.iter().filter_map(dict_of).collect())
        }
        // Keys of a dict, characters of a string and the bytes of a byte string are never dicts.
        PyValue::Dict(_) | PyValue::Str(_) | PyValue::Bytes(_) => Ok(Vec::new()),
        other => Err(not_iterable(other)),
    }
}

/// `sorted(members, key=lambda x: x.get(sort_key) or 0)`.
fn sorted_by_member_key<'a>(
    members: Vec<&'a PyDict>,
    sort_key: &str,
) -> Result<Vec<&'a PyDict>, Failure> {
    let keys: Vec<PyValue> = members
        .iter()
        .map(|member| key_or_zero(member, sort_key))
        .collect();
    sorted_by_key(members, &keys)
}

/// Python dict keys: hashable values equal by `==`; a list or dict key is a `TypeError`.
struct LastRunByLevel<'a>(Vec<(PyValue, &'a PyDict)>);

impl<'a> LastRunByLevel<'a> {
    fn assign(&mut self, level: &PyValue, run: &'a PyDict) -> Result<(), Failure> {
        if matches!(level.peel(), PyValue::List(_) | PyValue::Dict(_)) {
            return Err(Failure::new(
                "TypeError",
                &format!("unhashable type: '{}'", level.type_name()),
            ));
        }
        match self.0.iter_mut().find(|(key, _)| py_eq(key, level)) {
            Some(slot) => slot.1 = run,
            None => self.0.push((level.clone(), run)),
        }
        Ok(())
    }

    fn get(&self, level: &PyValue) -> Option<&'a PyDict> {
        self.0
            .iter()
            .find(|(key, _)| py_eq(key, level))
            .map(|(_, run)| *run)
    }
}

/// `(run or 0) < 2`.
fn run_number_below_two(run: &PyDict) -> Result<bool, Failure> {
    let number = key_or_zero(run, "run");
    Ok(compare(Compare::Less, &number, &PyValue::int(2))?)
}

fn repeat_break_at(level: &PyValue, ordered: &[&PyDict], index: usize) -> Stop {
    let run = ordered[index];
    let previous = if index > 0 { ordered[index - 1] } else { run };
    Stop {
        kind: StopKind::RepeatBreak,
        level: level.clone(),
        run: or_none(run.get("run")),
        detail: cat!(
            "run_bench rc=",
            show(run.get("run_bench_rc")),
            " · parse rc=",
            show(run.get("parse_rc")),
            " · measurement_ok=false"
        ),
        window_start_utc: or_none(previous.get("started_utc")),
        window_end_utc: or_none(run.get("ended_utc")),
        window_rule: REPEAT_BREAK_WINDOW_RULE,
    }
}

fn clamp_stop(clamp_run: &PyDict, last_runs: &LastRunByLevel<'_>) -> Stop {
    let level = or_none(clamp_run.get("level"));
    let below: Vec<(BigInt, &PyValue)> = last_runs
        .0
        .iter()
        .filter_map(|(key, _)| {
            let own = int_or_bool(key)?;
            let boundary = int_or_bool(&level)?;
            (own < boundary).then_some((own, key))
        })
        .collect();
    // `max(below)` keeps the first of equal maxima.
    let highest = below
        .iter()
        .fold(None::<&(BigInt, &PyValue)>, |best, candidate| match best {
            Some(current) if candidate.0 <= current.0 => Some(current),
            _ => Some(candidate),
        });
    let previous = highest
        .and_then(|(_, key)| last_runs.get(key))
        .unwrap_or(clamp_run);
    let unreadable = clamp_run
        .get("unreadable")
        .filter(|value| value.is_truthy());
    Stop {
        kind: StopKind::Clamp,
        level,
        run: or_none(clamp_run.get("run")),
        detail: match unreadable {
            Some(reason) => cat!("경계 사실 판독 불가: ", reason.display_text()),
            None => cat!(
                "run_bench rc=",
                show(clamp_run.get("run_bench_rc")),
                " · parse rc=",
                show(clamp_run.get("parse_rc")),
                " · measurement_ok=false(레벨 첫 run — 적응 상한 클램프)"
            ),
        },
        window_start_utc: if unreadable.is_some() {
            PyValue::None
        } else {
            or_none(previous.get("started_utc"))
        },
        window_end_utc: if unreadable.is_some() {
            PyValue::None
        } else {
            or_none(clamp_run.get("ended_utc"))
        },
        window_rule: CLAMP_WINDOW_RULE,
    }
}

/// The repeat summary of the sweep index's `levels[]` (each level's representative `measured.json` included) and
/// the place the sweep stopped, the immediate signal.
///
/// The stop is one place (a sweep halts when a run breaks): `repeat-break` is the first run at or above the second
/// that broke, found in `runs[]`; `clamp` is a level above the measured ones whose first run broke - it is not in
/// the index, so the boundary fact comes from `clamp_run`. The time window to compare against is *the previous
/// run's start to the broken run's end*: a kill executed in the tail of a completed run (after the measurement,
/// before the next run starts) can break the next run, while a window over the whole sweep would pin an unrelated
/// earlier kill on it. `runs_attempted` is the sum of level run attempts (the lite leg is not a level run).
pub fn summarize(request: &SummarizeRequest<'_>) -> Result<Summary, Failure> {
    let mut completed_by_level = KeyedByLevel(Vec::new());
    let mut band_pct_by_level = KeyedByLevel(Vec::new());
    let mut unrecorded_levels = Vec::new();
    let mut runs_attempted = 0usize;
    let mut stop: Option<Stop> = None;
    let mut last_runs = LastRunByLevel(Vec::new());

    for level_entry in sorted_by_member_key(dict_members(request.levels)?, "level")? {
        let level = or_none(level_entry.get("level"));
        let measured = level_entry.get("measured").and_then(dict_of);
        let runs = measured
            .and_then(|map| map.get("runs"))
            .map(PyValue::peel)
            .and_then(|value| match value {
                PyValue::List(items) => Some(items),
                _ => None,
            });
        let Some(runs) = runs else {
            unrecorded_levels.push(level);
            continue;
        };
        let ordered = sorted_by_member_key(runs.iter().filter_map(dict_of).collect(), "run")?;
        let label = level.display_text();
        let completed = ordered
            .iter()
            .filter(|run| is_true(run.get("measurement_ok")))
            .count();
        completed_by_level.assign(label.clone(), completed);
        band_pct_by_level.assign(
            label,
            or_none(measured.and_then(|map| map.get("repro_band_pct"))),
        );
        runs_attempted += ordered.len();
        if let Some(&last) = ordered.last() {
            last_runs.assign(&level, last)?;
        }
        if stop.is_some() {
            continue;
        }
        for (index, run) in ordered.iter().enumerate() {
            if is_true(run.get("measurement_ok")) || run_number_below_two(run)? {
                continue;
            }
            stop = Some(repeat_break_at(&level, &ordered, index));
            break;
        }
    }

    let clamp_run = dict_of(request.clamp_run);
    if let Some(clamp_run) = clamp_run {
        runs_attempted += 1;
        if stop.is_none() {
            stop = Some(clamp_stop(clamp_run, &last_runs));
        }
    }

    let mut short_levels = Vec::new();
    for (label, completed) in &completed_by_level.0 {
        if *completed < FULL_REPEATS_MIN as usize {
            short_levels.push(int_from_str(label)?);
        }
    }
    short_levels.sort();

    Ok(Summary {
        requested: request.requested.clone(),
        requested_source: request.requested_source.clone(),
        kind: request.kind.clone(),
        completed_by_level,
        short_levels,
        runs_attempted,
        band_pct_by_level,
        stop,
        clamp_run: clamp_run.map(|map| PyValue::Dict(map.clone())),
        unrecorded_levels,
    })
}

// ── repetition_established ──────────────────────────────────────────────────────

/// The verdict on the repeat condition and its reason. `established` is `None` when it cannot be judged.
#[derive(Clone, Debug, PartialEq)]
pub struct RepetitionVerdict {
    pub established: Option<bool>,
    pub reason: PyStr,
}

impl RepetitionVerdict {
    fn undetermined(reason: PyStr) -> Self {
        RepetitionVerdict {
            established: None,
            reason,
        }
    }

    /// The Python return value `(established, reason)`.
    pub fn to_value(&self) -> PyValue {
        PyValue::Tuple(vec![
            self.established.map_or(PyValue::None, PyValue::Bool),
            PyValue::Str(self.reason.clone()),
        ])
    }
}

fn attribute_error_get(value: &PyValue) -> Failure {
    Failure::new(
        "AttributeError",
        &format!("'{}' object has no attribute 'get'", value.type_name()),
    )
}

/// `str(level)` of a level the summary was keyed with.
fn level_label(level: &BigInt) -> PyStr {
    PyStr::new(&level.to_string())
}

/// The repeat condition of the full definition: whether the judgment point (the level the verdict and certificate
/// bind to) completed at least [`FULL_REPEATS_MIN`] runs. Blackbox causes are not looked at; that is the
/// classifier's.
///
/// * `Some(true)` - the judgment point completed enough runs (and no level other than it and the boundary is
///   below the definition);
/// * `Some(false)` - fewer completed *and* the repeat broke at the judgment point (a machine event);
/// * `None` - undeterminable: a level without `runs[]`, no judgment point, an unmeasured one, a shortfall without
///   a break, or a shortfall off the boundary.
pub fn repetition_established(
    summary: &PyValue,
    judgment_level: &PyValue,
) -> Result<RepetitionVerdict, Failure> {
    let Some(summary) = dict_of(summary) else {
        return Ok(RepetitionVerdict::undetermined(cat!(
            "not_evaluated(반복 요약 없음)"
        )));
    };
    if truthy(summary.get("unrecorded_levels")) {
        let unrecorded = summary
            .get("unrecorded_levels")
            .map(PyValue::display_text)
            .unwrap_or_default();
        return Ok(RepetitionVerdict::undetermined(cat!(
            "not_evaluated(runs[] 부재 레벨 ",
            unrecorded,
            " — 반복 축 신설 전 산출물이거나 집계 실패 · 강등 사유를 발명하지 않는다)"
        )));
    }
    let Some(judgment) = plain_int(judgment_level) else {
        return Ok(RepetitionVerdict::undetermined(cat!(
            "not_evaluated(판정점 레벨(verdict_point_level) 부재 — 반복 조건을 볼 레벨이 없다)"
        )));
    };
    let completed_by_level = summary
        .get("completed_by_level")
        .filter(|value| value.is_truthy());
    let completed_at_judgment = match completed_by_level.map(PyValue::peel) {
        None => None,
        Some(PyValue::Dict(map)) => {
            let label = level_label(judgment);
            map.iter()
                .find(|(key, _)| *key == label)
                .map(|(_, value)| value)
        }
        Some(other) => return Err(attribute_error_get(other)),
    };
    let Some(completed) = completed_at_judgment.and_then(plain_int) else {
        return Ok(RepetitionVerdict::undetermined(cat!(format!(
            "not_evaluated(판정점 레벨 {judgment} 가 측정 레벨에 없다)"
        ))));
    };

    let stop = summary
        .get("stop")
        .and_then(dict_of)
        .filter(|map| !map.is_empty());
    let stop_is = |kind: &str| stop.is_some_and(|map| is_text(map.get("kind"), kind));
    let stop_level = stop.and_then(|map| map.get("level"));
    let at_judgment_level =
        |value: Option<&PyValue>| value.is_some_and(|level| level.equals_int(judgment));
    let boundary: Option<PyValue> =
        if stop_is(STOP_KIND_REPEAT_BREAK) && !at_judgment_level(stop_level) {
            stop_level
                .filter(|level| !matches!(level.peel(), PyValue::None))
                .cloned()
        } else {
            None
        };

    let judgment_value = PyValue::Int(judgment.clone());
    let boundary_value = boundary.clone().unwrap_or(PyValue::None);
    let short_levels = summary
        .get("short_levels")
        .filter(|value| value.is_truthy());
    let mut stray = Vec::new();
    if let Some(levels) = short_levels {
        for level in elements_of(levels)? {
            if !py_eq(&level, &judgment_value) && !py_eq(&level, &boundary_value) {
                stray.push(level);
            }
        }
    }
    if !stray.is_empty() {
        return Ok(RepetitionVerdict::undetermined(cat!(
            "unclassifiable(레벨 ",
            PyValue::List(stray).repr(),
            " 이 반복 정의 미만인데 그 자리에서 스윕이 멈추지 않았다 — 기계 이벤트가 아니므로 강등 사유를 발명하지 않는다)"
        )));
    }

    let stop_run = || show(stop.and_then(|map| map.get("run")));
    if completed >= &floor_value() {
        let tail = if let Some(boundary) = &boundary {
            cat!(
                " · 경계 level ",
                boundary.display_text(),
                " run ",
                stop_run(),
                " 반복 중단 = 적응 상한 클램프(상위 레벨 미측정 · 판정점 반복은 성립)"
            )
        } else if stop_is(STOP_KIND_REPEAT_BREAK) {
            cat!(
                " · 판정점 level ",
                show(stop_level),
                " run ",
                stop_run(),
                " 에서 반복 중단(완주는 정의 충족 · 상위 레벨 미측정)"
            )
        } else if stop_is(STOP_KIND_CLAMP) {
            cat!(
                " · level ",
                show(stop_level),
                " 첫 run 에서 적응 상한 클램프"
            )
        } else {
            PyStr::default()
        };
        return Ok(RepetitionVerdict {
            established: Some(true),
            reason: cat!(
                format!(
                    "runs[] 집계(판정점 level {judgment} 완주 {completed} ≥ full 정의 {FULL_REPEATS_MIN})"
                ),
                tail
            ),
        });
    }
    if stop_is(STOP_KIND_REPEAT_BREAK) && at_judgment_level(stop_level) {
        return Ok(RepetitionVerdict {
            established: Some(false),
            reason: cat!(
                format!(
                    "runs[] 집계(판정점 level {judgment} 완주 {completed} < full 정의 {FULL_REPEATS_MIN} · run "
                ),
                stop_run(),
                " 에서 반복 중단)"
            ),
        });
    }
    Ok(RepetitionVerdict::undetermined(cat!(format!(
        "unclassifiable(판정점 완주 {completed} < full 정의 {FULL_REPEATS_MIN} 인데 판정점에서 끊긴 run 이 없다 — 요청 자체가 정의 미만인 산출물 · 강등 사유를 발명하지 않는다)"
    ))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pyjson::loads;

    fn json(text: &str) -> PyValue {
        loads(text).expect("valid JSON")
    }

    #[test]
    fn band_matches_the_campaign_ledger_value() {
        let band = band_pct(&json("[11.759, 11.971, 11.301]")).unwrap();
        assert_eq!(band.percent, Some(5.7378));
        assert_eq!(band.source.to_utf8().unwrap(), "measured(n=3)");
    }

    #[test]
    fn float_sum_is_compensated_like_python_3_12() {
        // 3.12: sum([0.1]*10) == 1.0 (naive left-to-right gives 0.9999999999999999)
        assert_eq!(python_float_sum(&[0.1; 10]), 1.0);
    }

    #[test]
    fn repeat_count_domain() {
        let where_ = PyValue::str("x");
        assert_eq!(
            parse_repeats(&PyValue::str(" 5 "), &where_).unwrap(),
            BigInt::from(5)
        );
        assert!(parse_repeats(&PyValue::Bool(true), &where_).is_err());
        assert!(parse_repeats(&PyValue::int(2), &where_).is_err());
    }
}
