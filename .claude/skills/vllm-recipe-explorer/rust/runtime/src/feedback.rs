//! `feedback_log.py`: the append-only JSONL feedback log of the recipe explorer. A record carries the estimated fields the
//! generation step fills and the nullable fields the serving and measuring steps will fill later; `append` normalizes a record
//! (timestamps, the reserved fields), creates the log's directory and appends one JSON line.
//!
//! The behaviour is the original's, including what it does not do: no lock, no repair of a missing final newline, no coercion of
//! fields, no atomic write. The record may be any value: it is read through `dict(record)`, so a list of pairs is a record and
//! a number is a `TypeError`. A line that cannot be encoded as UTF-8 fails after the log file was created or opened.
//!
//! Time comes from a [`Clock`]: the process's local clock in production (`datetime.now().isoformat()`), or one fixed text a
//! caller supplies for one call (the functional fixture of the API; nothing global is changed).

use crate::model_files::{fspath, os_failure};
use crate::pyexpr::Res;
use crate::recipe_set::{makedirs, split_path, write_text};
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pydyn::{dict_key, iterate};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::os::unix::ffi::OsStrExt;

/// The fields the generation step estimates (`ESTIMATED_FIELDS`).
pub const ESTIMATED_FIELDS: [&str; 10] = [
    "timestamp",
    "model_id",
    "quantization",
    "max_model_len",
    "gpu_memory_utilization",
    "vram_budget_gb",
    "estimated_vram_gb",
    "tensor_parallel_size",
    "safety_margin_threshold",
    "selection_timestamp",
];

/// The fields the serving and measuring steps will fill, `null` until then (`NULLABLE_FIELDS`).
pub const NULLABLE_FIELDS: [&str; 12] = [
    "actual_vram_gb",
    "serve_success",
    "error_type",
    "tokens_per_sec",
    "batch",
    "kv_cache_memory_bytes",
    "attention_backend",
    "tool_call_parser",
    "reasoning_parser",
    "converged",
    "trial_count",
    "correction_history",
];

/// Where `datetime.now().isoformat()` comes from.
#[derive(Clone, Debug)]
pub enum Clock {
    /// The process's local time (the production clock).
    Local,
    /// One fixed text for the call.
    Fixed(PyStr),
}

impl Clock {
    pub fn now_iso(&self) -> PyStr {
        match self {
            Clock::Local => PyStr::new(&local_now_iso()),
            Clock::Fixed(text) => text.clone(),
        }
    }
}

#[repr(C)]
struct BrokenDownTime {
    seconds: i32,
    minutes: i32,
    hours: i32,
    day: i32,
    month: i32,
    year: i32,
    weekday: i32,
    yearday: i32,
    daylight: i32,
    gmt_offset: i64,
    zone: *const u8,
}

extern "C" {
    fn localtime_r(seconds: *const i64, out: *mut BrokenDownTime) -> *mut BrokenDownTime;
}

/// `datetime.fromtimestamp(seconds + microseconds / 1e6).isoformat()`: the local civil time of an instant as
/// `YYYY-MM-DDTHH:MM:SS`, with `.ffffff` (six digits) only when the microseconds are not zero, and no time zone suffix.
pub fn iso_text(seconds: i64, microseconds: u32) -> String {
    let mut local = BrokenDownTime {
        seconds: 0,
        minutes: 0,
        hours: 0,
        day: 0,
        month: 0,
        year: 0,
        weekday: 0,
        yearday: 0,
        daylight: 0,
        gmt_offset: 0,
        zone: std::ptr::null(),
    };
    // SAFETY: `localtime_r` writes a complete `struct tm` (glibc, LP64 layout) into `local` and reads `seconds`; it keeps no
    // pointer to either.
    let filled = unsafe { localtime_r(&seconds, &mut local) };
    if filled.is_null() {
        return String::from("1970-01-01T00:00:00");
    }
    let mut text = format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
        local.year + 1900,
        local.month + 1,
        local.day,
        local.hours,
        local.minutes,
        local.seconds
    );
    if microseconds != 0 {
        text.push_str(&format!(".{microseconds:06}"));
    }
    text
}

/// `datetime.now().isoformat()`: [`iso_text`] of the system clock (the microseconds are floored from its nanoseconds).
pub fn local_now_iso() -> String {
    let since_epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    iso_text(since_epoch.as_secs() as i64, since_epoch.subsec_micros())
}

/// `build_record(model_id, quantization, max_model_len, gpu_memory_utilization, vram_budget_gb, estimated_vram_gb,
/// tensor_parallel_size, safety_margin_threshold, selection_timestamp=None)`.
pub struct Estimate<'a> {
    pub model_id: &'a PyValue,
    pub quantization: &'a PyValue,
    pub max_model_len: &'a PyValue,
    pub gpu_memory_utilization: &'a PyValue,
    pub vram_budget_gb: &'a PyValue,
    pub estimated_vram_gb: &'a PyValue,
    pub tensor_parallel_size: &'a PyValue,
    pub safety_margin_threshold: &'a PyValue,
    pub selection_timestamp: &'a PyValue,
}

/// A complete record: the estimated fields filled and the nullable ones reserved.
pub fn build_record(estimate: &Estimate<'_>, clock: &Clock) -> PyDict {
    let now = PyValue::Str(clock.now_iso());
    let mut record = PyDict::new();
    record.set("timestamp", now.clone());
    record.set("model_id", estimate.model_id.clone());
    record.set("quantization", estimate.quantization.clone());
    record.set("max_model_len", estimate.max_model_len.clone());
    record.set(
        "gpu_memory_utilization",
        estimate.gpu_memory_utilization.clone(),
    );
    record.set("vram_budget_gb", estimate.vram_budget_gb.clone());
    record.set("estimated_vram_gb", estimate.estimated_vram_gb.clone());
    record.set(
        "tensor_parallel_size",
        estimate.tensor_parallel_size.clone(),
    );
    record.set(
        "safety_margin_threshold",
        estimate.safety_margin_threshold.clone(),
    );
    record.set(
        "selection_timestamp",
        if matches!(estimate.selection_timestamp.peel(), PyValue::None) {
            now
        } else {
            estimate.selection_timestamp.clone()
        },
    );
    for key in NULLABLE_FIELDS {
        record.set(key, PyValue::None);
    }
    record
}

/// `dict(record)`: a dict is copied; anything else must be an iterable of pairs.
pub fn dict_of(record: &PyValue) -> Res<PyDict> {
    if let PyValue::Dict(map) = record.peel() {
        return Ok(map.clone());
    }
    let mut out = PyDict::new();
    for (index, item) in iterate(record)?.iter().enumerate() {
        let Ok(pair) = iterate(item) else {
            return Err(Failure::new(
                "TypeError",
                &format!(
                    "cannot convert dictionary update sequence element #{index} to a sequence"
                ),
            ));
        };
        if pair.len() != 2 {
            return Err(Failure::new(
                "ValueError",
                &format!(
                    "dictionary update sequence element #{index} has length {}; 2 is required",
                    pair.len()
                ),
            ));
        }
        static FRESH: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let key = dict_key(
            &pair[0],
            FRESH.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        )?;
        out.insert_key(key, pair[1].clone());
    }
    Ok(out)
}

/// `_normalize(record)`: a copy with the timestamps filled in when empty and the reserved fields present. The clock is read
/// once, after the record was copied, whether or not it is needed.
pub fn normalize(record: &PyValue, clock: &Clock) -> Res<PyDict> {
    let mut out = dict_of(record)?;
    let now = PyValue::Str(clock.now_iso());
    if !out.get("timestamp").is_some_and(PyValue::is_truthy) {
        out.set("timestamp", now);
    }
    if !out
        .get("selection_timestamp")
        .is_some_and(PyValue::is_truthy)
    {
        let timestamp = out
            .get("timestamp")
            .cloned()
            .expect("the timestamp was just set");
        out.set("selection_timestamp", timestamp);
    }
    for key in NULLABLE_FIELDS {
        if !out.contains_key(key) {
            out.set(key, PyValue::None);
        }
    }
    Ok(out)
}

/// `os.getcwd()`.
fn current_directory() -> Res<PyStr> {
    std::env::current_dir()
        .map(|path| PyStr::from_surrogateescape(path.as_os_str().as_bytes()))
        .map_err(|error| os_failure(&error, None))
}

/// `posixpath.normpath(path)`.
pub fn normpath(path: &PyStr) -> PyStr {
    let slash = u32::from('/');
    let dot = u32::from('.');
    let points = path.code_points();
    if points.is_empty() {
        return PyStr::new(".");
    }
    let leading = points.iter().take_while(|&&point| point == slash).count();
    // Exactly two leading slashes are kept; one, or three and more, are one.
    let initial = if leading == 2 { 2 } else { leading.min(1) };
    let mut parts: Vec<&[u32]> = Vec::new();
    for component in points.split(|&point| point == slash) {
        if component.is_empty() || component == [dot] {
            continue;
        }
        let dotdot = component == [dot, dot];
        if !dotdot
            || (initial == 0 && parts.is_empty())
            || parts.last().is_some_and(|last| *last == [dot, dot])
        {
            parts.push(component);
        } else if !parts.is_empty() {
            parts.pop();
        }
    }
    let mut out: Vec<u32> = vec![slash; initial];
    for (index, part) in parts.iter().enumerate() {
        if index > 0 {
            out.push(slash);
        }
        out.extend_from_slice(part);
    }
    if out.is_empty() {
        return PyStr::new(".");
    }
    PyStr::from_code_points(out)
}

/// `os.path.abspath(path)` for a path of any type (a non-path is the `TypeError` of `os.fspath`).
pub fn abspath(path: &PyValue) -> Res<PyStr> {
    let text = fspath(path, false)?;
    if text.code_points().first() == Some(&u32::from('/')) {
        return Ok(normpath(text));
    }
    let mut joined = current_directory()?;
    if !joined
        .code_points()
        .last()
        .is_some_and(|&p| p == u32::from('/'))
    {
        joined.push_str("/");
    }
    joined.push_py(text);
    Ok(normpath(&joined))
}

/// `os.path.dirname(path)` of a text.
pub fn dirname(path: &PyStr) -> PyStr {
    split_path(path).0
}

/// `append(record, path)`: normalizes the record, creates the log's directory and appends one JSON line (with a final newline,
/// UTF-8). The result is the `path` that was given.
pub fn append(record: &PyValue, path: &PyValue, clock: &Clock) -> Res<PyValue> {
    let normalized = normalize(record, clock)?;
    let log_dir = dirname(&abspath(path)?);
    if !log_dir.is_empty() {
        makedirs(&log_dir)?;
    }
    let mut line = PyValue::Dict(normalized).dumps_unicode().map_err(|error| {
        let text = error.to_string();
        let prefix = format!("{}: ", error.python_type());
        Failure::new(
            error.python_type(),
            text.strip_prefix(&prefix).unwrap_or(&text),
        )
    })?;
    line.push_str("\n");
    // `open(path, "a")` takes the path as given (a `str`; any other type is refused above by `abspath`).
    let target = fspath(path, false)?;
    write_text(target, &line, true)?;
    Ok(path.clone())
}
