//! Naming of the benchmark documents (`bench_report`, `benchmark`, `max_envelope`): a port of the benchmark
//! skill's `doc_naming.py`, the single owner of the file-name convention
//! `<kind>_<YYMMDDHH>[_<MM>_<SS>]_<model>_<gpu>_<vllm>.<ext>`.
//!
//! * The hour token is KST (UTC+9). `_MM_SS` appears only when a *different measurement* already holds the
//!   unsuffixed name; re-publishing the same measurement reuses its name. "Same measurement" is decided by the
//!   timestamp inside the existing file, never by its name, and a file whose timestamp cannot be read is never
//!   treated as the same (the original's fail-closed rule).
//! * A slot (token, optional minute/second, combination) is judged as the union over all three kinds, so the
//!   report, certificate and envelope of one measurement always take the same slot.
//! * [`bench_filename`] touches no file; [`scan_bench_dir`] is the only reader.
//!
//! Values arrive as [`PyValue`]s because the original is dynamically typed and its callers (and the API transport)
//! hand it whatever a JSON document held: every wrong-type call fails with the class and message CPython raises.
//! The two naming exceptions are plain [`Failure`]s whose `class` is the exception name, which is the identity a
//! bridge re-exports.

use crate::guidellm::Failure;
use crate::ospath;
use crate::pydyn::{as_get_target, ensure_hashable};
use crate::pyfs;
use crate::pystr::{is_decimal_digit, lit, PyStr};
use crate::pyvalue::{PyDict, PyValue};
use crate::report_support::{display, lossy_text_ignoring_errors, py, stat_path_argument};
use std::sync::LazyLock;

pub const NAMING_COLLISION_EXHAUSTED: &str = "NamingCollisionExhausted";
pub const NAMING_SOURCE_UNREADABLE: &str = "NamingSourceUnreadable";

/// `(kind, default extension)` in the original's dict order.
pub const KIND_DEFAULT_EXTENSIONS: [(&str, &str); 3] = [
    ("bench_report", "md"),
    ("benchmark", "yaml"),
    ("max_envelope", "md"),
];

type Res<T> = Result<T, Failure>;

fn cp(text: &str) -> Vec<u32> {
    text.chars().map(u32::from).collect()
}

// ── kst_tokens ─────────────────────────────────────────────────────────────────

/// The `NA` placeholder triple the original returns for anything that is not a real UTC second.
fn sentinel_tokens() -> (PyStr, PyStr, PyStr) {
    (lit!("NA"), lit!("00"), lit!("00"))
}

fn digits_to_number(points: &[u32]) -> i64 {
    points.iter().fold(0i64, |acc, &p| {
        acc * 10 + i64::from(crate::pystr::decimal_digit_value(p).expect("checked decimal digit"))
    })
}

/// The six fields of `^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})Z$` (`\d` is any Unicode decimal digit and
/// `$` also matches before one final newline).
fn utc_fields(text: &[u32]) -> Option<[i64; 6]> {
    let body = match text.last() {
        Some(&10) => &text[..text.len() - 1],
        _ => text,
    };
    let shape: [(usize, Option<char>); 6] = [
        (4, Some('-')),
        (2, Some('-')),
        (2, Some('T')),
        (2, Some(':')),
        (2, Some(':')),
        (2, None),
    ];
    let mut at = 0;
    let mut fields = [0i64; 6];
    for (slot, (width, separator)) in shape.iter().enumerate() {
        let chunk = body.get(at..at + width)?;
        if !chunk.iter().all(|&p| is_decimal_digit(p)) {
            return None;
        }
        fields[slot] = digits_to_number(chunk);
        at += width;
        if let Some(separator) = separator {
            if body.get(at) != Some(&u32::from(*separator)) {
                return None;
            }
            at += 1;
        }
    }
    (body.get(at) == Some(&u32::from('Z')) && at + 1 == body.len()).then_some(fields)
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                29
            } else {
                28
            }
        }
    }
}

/// UTC `'YYYY-MM-DDTHH:MM:SSZ'` -> `(YYMMDDHH, MM, SS)` in KST. `None`, a malformed text or a date the calendar does
/// not have all give `("NA", "00", "00")`; nothing raises.
pub fn kst_tokens(generated_utc: &PyValue) -> (PyStr, PyStr, PyStr) {
    if !generated_utc.is_truthy() {
        return sentinel_tokens();
    }
    let PyValue::Str(text) = generated_utc.peel() else {
        // `str()` of a non-string never has the shape of a UTC second.
        return sentinel_tokens();
    };
    let Some([year, month, day, hour, minute, second]) = utc_fields(text.code_points()) else {
        return sentinel_tokens();
    };
    let valid = (1..=9999).contains(&year)
        && (1..=12).contains(&month)
        && (1..=days_in_month(year, month)).contains(&day)
        && hour < 24
        && minute < 60
        && second < 60;
    if !valid {
        return sentinel_tokens();
    }
    let (mut year, mut month, mut day, mut hour) = (year, month, day, hour + 9);
    if hour >= 24 {
        hour -= 24;
        day += 1;
        if day > days_in_month(year, month) {
            day = 1;
            month += 1;
            if month > 12 {
                month = 1;
                year += 1;
            }
        }
    }
    if year > 9999 {
        return sentinel_tokens();
    }
    (
        py(&format!(
            "{:02}{month:02}{day:02}{hour:02}",
            year.rem_euclid(100)
        )),
        py(&format!("{minute:02}")),
        py(&format!("{second:02}")),
    )
}

// ── gpu_key / measurement_ts ──────────────────────────────────────────────────

/// Free-text GPU model -> file-name-safe key: `"NVIDIA GB10"` -> `"GB10"`; nothing usable -> `"NA"`.
pub fn gpu_key(gpu_model: &PyValue) -> PyStr {
    if !gpu_model.is_truthy() {
        return lit!("NA");
    }
    let shown = display(gpu_model).code_points().to_vec();
    let without_vendor = replace_all(&shown, &cp("NVIDIA"), &[]);
    let kept: Vec<u32> = without_vendor
        .into_iter()
        .filter(|&p| char::from_u32(p).is_some_and(|c| c.is_ascii_alphanumeric()))
        .collect();
    if kept.is_empty() {
        lit!("NA")
    } else {
        PyStr::from_code_points(kept)
    }
}

fn replace_all(text: &[u32], needle: &[u32], with: &[u32]) -> Vec<u32> {
    let mut out = Vec::with_capacity(text.len());
    let mut at = 0;
    while at < text.len() {
        if text[at..].starts_with(needle) {
            out.extend_from_slice(with);
            at += needle.len();
        } else {
            out.push(text[at]);
            at += 1;
        }
    }
    out
}

const MEASUREMENT_KEYWORDS: [&str; 3] = ["measured_utc", "generated_utc", "생성일"];
static MEASUREMENT_KEYWORD_POINTS: LazyLock<Vec<Vec<u32>>> = LazyLock::new(|| {
    MEASUREMENT_KEYWORDS
        .iter()
        .map(|keyword| cp(keyword))
        .collect()
});
const STAMP_SHAPE: [(usize, Option<char>); 6] = [
    (4, Some('-')),
    (2, Some('-')),
    (2, Some('T')),
    (2, Some(':')),
    (2, Some(':')),
    (2, None),
];

/// The 20 code points of `\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z` at `at`, if they are there.
fn stamp_at(text: &[u32], at: usize) -> Option<&[u32]> {
    let mut position = at;
    for (width, separator) in STAMP_SHAPE {
        let chunk = text.get(position..position + width)?;
        if !chunk.iter().all(|&p| is_decimal_digit(p)) {
            return None;
        }
        position += width;
        if let Some(separator) = separator {
            if text.get(position) != Some(&u32::from(separator)) {
                return None;
            }
            position += 1;
        }
    }
    (text.get(position) == Some(&u32::from('Z'))).then(|| &text[at..=position])
}

/// The measurement time a document names (`measured_utc`, `generated_utc` or `생성일`, then non-digits, then the UTC
/// second): the first such place in the text, else `None`. A non-string argument is the `TypeError` `re` raises.
pub fn measurement_ts(text: &PyValue) -> Res<Option<PyStr>> {
    if !text.is_truthy() {
        return Ok(None);
    }
    let PyValue::Str(text) = text.peel() else {
        return Err(Failure::new(
            "TypeError",
            &format!(
                "expected string or bytes-like object, got '{}'",
                text.type_name()
            ),
        ));
    };
    Ok(measurement_stamp(text.code_points()).map(|stamp| PyStr::from_code_points(stamp.to_vec())))
}

pub(crate) fn measurement_stamp(text: &[u32]) -> Option<&[u32]> {
    for start in 0..text.len() {
        for keyword in MEASUREMENT_KEYWORD_POINTS.iter() {
            if !text[start..].starts_with(keyword) {
                continue;
            }
            let mut at = start + keyword.len();
            while at < text.len() && !is_decimal_digit(text[at]) {
                at += 1;
            }
            if let Some(stamp) = stamp_at(text, at) {
                return Some(stamp);
            }
        }
    }
    None
}

// ── scan_bench_dir ────────────────────────────────────────────────────────────

/// `{file name: measurement time | None}` of the regular files of `out_dir` (those starting with `kind_` when a kind
/// is given). A file that cannot be read, or names no time, maps to `None` — the mark [`bench_filename`] refuses on.
/// This is the module's only reader. Entries come in the order the operating system lists them.
pub fn scan_bench_dir(out_dir: &PyValue, kind: &PyValue) -> Res<PyDict> {
    let mut found = PyDict::new();
    if !out_dir.is_truthy() {
        return Ok(found);
    }
    let directory = match out_dir.peel() {
        PyValue::Str(path) => path.clone(),
        other => {
            stat_path_argument(other)?;
            return Ok(found);
        }
    };
    if !ospath::isdir(&directory) {
        return Ok(found);
    }
    let filter = if kind.is_truthy() {
        Some(match kind.peel() {
            PyValue::Str(text) => {
                let mut prefix = text.clone();
                prefix.push_str("_");
                prefix
            }
            other => return Err(concat_failure(other)),
        })
    } else {
        None
    };
    for name in pyfs::list_dir(&directory)? {
        if let Some(prefix) = &filter {
            if !name.code_points().starts_with(prefix.code_points()) {
                continue;
            }
        }
        let path = ospath::join(&directory, &[&name]);
        if !ospath::isfile(&path) {
            continue;
        }
        let stamp = match pyfs::read_bytes(&path) {
            Ok(bytes) => {
                let text = lossy_text_ignoring_errors(&bytes);
                measurement_stamp(text.code_points()).map_or(PyValue::None, |stamp| {
                    PyValue::Str(PyStr::from_code_points(stamp.to_vec()))
                })
            }
            Err(_) => PyValue::None,
        };
        found.insert(name, stamp);
    }
    Ok(found)
}

/// `value + "_"` for a value that is not a string.
pub(crate) fn concat_failure(value: &PyValue) -> Failure {
    match value.peel() {
        PyValue::List(_) => Failure::new(
            "TypeError",
            "can only concatenate list (not \"str\") to list",
        ),
        PyValue::Tuple(_) => Failure::new(
            "TypeError",
            "can only concatenate tuple (not \"str\") to tuple",
        ),
        other => Failure::new(
            "TypeError",
            &format!(
                "unsupported operand type(s) for +: '{}' and 'str'",
                other.type_name()
            ),
        ),
    }
}

// ── bench_filename ────────────────────────────────────────────────────────────

/// A name that follows the naming convention, split into its slot parts.
struct ConventionalName {
    token: Vec<u32>,
    suffix: Option<(Vec<u32>, Vec<u32>)>,
    combination: Vec<u32>,
}

/// `^(?:bench_report|benchmark|max_envelope)_(\d{8}|NA)(?:_(\d\d)_(\d\d))?_(.+)\.(?:md|yaml)$` on a whole name.
fn parse_conventional(name: &[u32]) -> Option<ConventionalName> {
    let rest = KIND_DEFAULT_EXTENSIONS.iter().find_map(|(kind, _)| {
        let prefix = cp(&format!("{kind}_"));
        name.strip_prefix(prefix.as_slice())
    })?;
    let (token, after_token) = if rest.len() >= 8 && rest[..8].iter().all(|&p| is_decimal_digit(p))
    {
        (&rest[..8], &rest[8..])
    } else if rest.starts_with(&cp("NA")) {
        (&rest[..2], &rest[2..])
    } else {
        return None;
    };
    let with_suffix = (|| {
        let tail = after_token.strip_prefix(&[u32::from('_')])?;
        let minute = tail
            .get(..2)
            .filter(|d| d.iter().all(|&p| is_decimal_digit(p)))?;
        let tail = tail[2..].strip_prefix(&[u32::from('_')])?;
        let second = tail
            .get(..2)
            .filter(|d| d.iter().all(|&p| is_decimal_digit(p)))?;
        let combination = combination_of(&tail[2..])?;
        Some(ConventionalName {
            token: token.to_vec(),
            suffix: Some((minute.to_vec(), second.to_vec())),
            combination,
        })
    })();
    with_suffix.or_else(|| {
        Some(ConventionalName {
            token: token.to_vec(),
            suffix: None,
            combination: combination_of(after_token)?,
        })
    })
}

/// `_(.+)\.(md|yaml)$`: the combination between the separating underscore and the extension.
fn combination_of(rest: &[u32]) -> Option<Vec<u32>> {
    let rest = rest.strip_prefix(&[u32::from('_')])?;
    let body = match rest.last() {
        Some(&10) => &rest[..rest.len() - 1],
        _ => rest,
    };
    for extension in [".md", ".yaml"] {
        let extension = cp(extension);
        if let Some(combination) = body.strip_suffix(extension.as_slice()) {
            if !combination.is_empty() && !combination.contains(&10) {
                return Some(combination.to_vec());
            }
        }
    }
    None
}

/// The `existing` argument as the original treats it: only a dict has usable `.items()`.
enum Existing<'a> {
    Absent,
    Map(&'a PyDict),
    Other(&'a PyValue),
}

impl<'a> Existing<'a> {
    fn of(value: &'a PyValue) -> Self {
        if !value.is_truthy() {
            return Existing::Absent;
        }
        match value.peel() {
            PyValue::Dict(map) => Existing::Map(map),
            other => Existing::Other(other),
        }
    }
}

/// `name in existing` / `existing[name]` for a container that is not a dict: always a failure of the original.
fn non_dict_failure(existing: &PyValue, name: &PyStr) -> Failure {
    let contains = match existing.peel() {
        PyValue::List(items) | PyValue::Tuple(items) => Some(
            items
                .iter()
                .any(|item| matches!(item.peel(), PyValue::Str(text) if text == name)),
        ),
        PyValue::Str(text) => Some(contains_run(text.code_points(), name.code_points())),
        _ => None,
    };
    match contains {
        None => Failure::new(
            "TypeError",
            &format!(
                "argument of type '{}' is not iterable",
                existing.type_name()
            ),
        ),
        Some(true) => Failure::new(
            "TypeError",
            &match existing.peel() {
                PyValue::Str(_) => "string indices must be integers, not 'str'".to_string(),
                other => format!(
                    "{} indices must be integers or slices, not str",
                    other.type_name()
                ),
            },
        ),
        Some(false) => Failure::new(
            "AttributeError",
            &format!("'{}' object has no attribute 'items'", existing.type_name()),
        ),
    }
}

fn contains_run(haystack: &[u32], needle: &[u32]) -> bool {
    needle.is_empty()
        || haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

struct Occupant<'a> {
    name: &'a PyStr,
    stamp: &'a PyValue,
}

/// The files occupying the slot `(token[, minute, second], combination)` across all kinds.
fn slot_occupants<'a>(
    existing: &'a PyDict,
    token: &PyStr,
    combination: &PyStr,
    suffix: Option<(&PyStr, &PyStr)>,
) -> Vec<Occupant<'a>> {
    existing
        .iter()
        .filter_map(|(name, stamp)| {
            let parsed = parse_conventional(name.code_points())?;
            if parsed.token != token.code_points()
                || parsed.combination != combination.code_points()
            {
                return None;
            }
            match (&suffix, &parsed.suffix) {
                (None, None) => {}
                (Some((minute, second)), Some((m, s)))
                    if m == minute.code_points() && s == second.code_points() => {}
                _ => return None,
            }
            Some(Occupant { name, stamp })
        })
        .collect()
}

enum Alignment {
    Free,
    SameMeasurement,
    OtherMeasurement,
}

/// Judgement over the occupants of one slot: free when empty, the same measurement when every occupant holds this
/// measurement's time, otherwise taken by another one. An unreadable occupant (no time) refuses outright.
fn align_or_fail(
    occupants: &[Occupant<'_>],
    generated: &PyStr,
    slot_description: &PyStr,
) -> Res<Alignment> {
    if occupants.is_empty() {
        return Ok(Alignment::Free);
    }
    for occupant in occupants {
        ensure_hashable(occupant.stamp)?;
    }
    if occupants
        .iter()
        .any(|occupant| matches!(occupant.stamp.peel(), PyValue::None))
    {
        let unreadable: Vec<String> = occupants
            .iter()
            .filter(|occupant| matches!(occupant.stamp.peel(), PyValue::None))
            .map(|occupant| occupant.name.repr())
            .collect();
        return Err(Failure::with_message(
            NAMING_SOURCE_UNREADABLE,
            message(&[
                "bench_filename: ",
                &slot_description.to_utf8().unwrap_or_default(),
                " 슬롯 점유 파일 [",
                &unreadable.join(", "),
                "] 의 측정시각을 읽지 못한다 — 같은 측정인지 판정 불가라 덮어쓰기·접미 모두 거부",
            ]),
        ));
    }
    let same = occupants
        .iter()
        .all(|occupant| matches!(occupant.stamp.peel(), PyValue::Str(text) if text == generated));
    Ok(if same {
        Alignment::SameMeasurement
    } else {
        Alignment::OtherMeasurement
    })
}

fn message(parts: &[&str]) -> PyStr {
    py(&parts.concat())
}

fn stamp_equals(stamp: &PyValue, generated: &PyStr) -> bool {
    matches!(stamp.peel(), PyValue::Str(text) if text == generated)
}

/// The file name for one document of a measurement.
///
/// `kind` is one of `bench_report`, `benchmark`, `max_envelope`; `meta` holds `model`, `gpu_key` and `vllm_version`
/// (absent ones read as the literal `NA`); `existing` is a [`scan_bench_dir`] result (or nothing for stdout mode);
/// `ext` overrides the kind's default extension.
///
/// Fails with `ValueError` for an unknown kind, [`NAMING_SOURCE_UNREADABLE`] when a file that would have to be
/// overwritten or suffixed holds no readable measurement time, and [`NAMING_COLLISION_EXHAUSTED`] when both the
/// unsuffixed name and the one `_MM_SS` candidate belong to other measurements.
pub fn bench_filename(
    kind: &PyValue,
    meta: &PyValue,
    generated_utc: &PyValue,
    existing: &PyValue,
    ext: &PyValue,
) -> Res<PyStr> {
    ensure_hashable(kind)?;
    let kind_text = match kind.peel() {
        PyValue::Str(text)
            if KIND_DEFAULT_EXTENSIONS
                .iter()
                .any(|(known, _)| text.equals(known)) =>
        {
            text.to_utf8().expect("known kind is UTF-8")
        }
        other => {
            let known: Vec<String> = KIND_DEFAULT_EXTENSIONS
                .iter()
                .map(|(known, _)| format!("'{known}'"))
                .collect();
            return Err(Failure::new(
                "ValueError",
                &format!(
                    "bench_filename: kind must be one of ({}), got {}",
                    known.join(", "),
                    other.repr()
                ),
            ));
        }
    };
    let default_extension = KIND_DEFAULT_EXTENSIONS
        .iter()
        .find(|(known, _)| *known == kind_text)
        .map(|(_, extension)| *extension)
        .expect("kind was validated");
    let extension = if ext.is_truthy() {
        display(ext)
    } else {
        py(default_extension)
    };
    let empty = PyDict::new();
    let meta_map = if meta.is_truthy() {
        as_get_target(meta)?
    } else {
        &empty
    };
    let (token, minute, second) = kst_tokens(generated_utc);
    let part = |key: &str| meta_map.get(key).map_or_else(|| lit!("NA"), display);
    let mut combination = part("model");
    combination.push_str("_");
    combination.push_py(&part("gpu_key"));
    combination.push_str("_");
    combination.push_py(&part("vllm_version"));
    let generated = display(generated_utc);
    let name_of = |suffix: Option<(&PyStr, &PyStr)>| -> PyStr {
        let mut name = py(&kind_text);
        name.push_str("_");
        name.push_py(&token);
        if let Some((m, s)) = suffix {
            name.push_str("_");
            name.push_py(m);
            name.push_str("_");
            name.push_py(s);
        }
        name.push_str("_");
        name.push_py(&combination);
        name.push_str(".");
        name.push_py(&extension);
        name
    };
    let base = name_of(None);
    let existing_map = match Existing::of(existing) {
        Existing::Absent => return Ok(base),
        Existing::Map(map) => map,
        Existing::Other(other) => {
            // Every path of the original fails for a container that is not a dict.
            let first_probe = non_dict_failure(other, &base);
            return Err(first_probe);
        }
    };
    let base_stamp = existing_map
        .get(&base.to_utf8().unwrap_or_default())
        .filter(|_| existing_map.iter().any(|(name, _)| name == &base));
    if let Some(stamp) = base_stamp {
        if matches!(stamp.peel(), PyValue::None) {
            return Err(Failure::with_message(
                NAMING_SOURCE_UNREADABLE,
                message(&[
                    "bench_filename: ",
                    &base.repr(),
                    " exists but its measurement timestamp is unreadable — cannot decide same-vs-different measurement, refusing to overwrite or suffix",
                ]),
            ));
        }
        if stamp_equals(stamp, &generated) {
            return Ok(base);
        }
    } else {
        let slot = py(&format!(
            "base({},{})",
            token.to_utf8().unwrap_or_default(),
            combination.to_utf8().unwrap_or_default()
        ));
        let occupants = slot_occupants(existing_map, &token, &combination, None);
        if !matches!(
            align_or_fail(&occupants, &generated, &slot)?,
            Alignment::OtherMeasurement
        ) {
            // A pair already placed in a `_MM_SS` slot of this measurement keeps the pair together.
            for (name, stamp) in existing_map.iter() {
                if let Some(parsed) = parse_conventional(name.code_points()) {
                    if let Some((m, s)) = &parsed.suffix {
                        if parsed.token == token.code_points()
                            && parsed.combination == combination.code_points()
                            && stamp_equals(stamp, &generated)
                        {
                            return Ok(name_of(Some((
                                &PyStr::from_code_points(m.clone()),
                                &PyStr::from_code_points(s.clone()),
                            ))));
                        }
                    }
                }
            }
            return Ok(base);
        }
    }
    let suffixed = name_of(Some((&minute, &second)));
    if let Some((_, stamp2)) = existing_map.iter().find(|(name, _)| name == &suffixed) {
        if matches!(stamp2.peel(), PyValue::None) {
            return Err(Failure::with_message(
                NAMING_SOURCE_UNREADABLE,
                message(&[
                    "bench_filename: ",
                    &suffixed.repr(),
                    " exists but its measurement timestamp is unreadable",
                ]),
            ));
        }
        if stamp_equals(stamp2, &generated) {
            return Ok(suffixed);
        }
        let first = base_stamp.map_or_else(|| lit!("-"), display);
        return Err(Failure::with_message(
            NAMING_COLLISION_EXHAUSTED,
            message(&[
                "bench_filename: both ",
                &base.repr(),
                " and ",
                &suffixed.repr(),
                " are occupied by OTHER measurements (",
                &first.to_utf8().unwrap_or_default(),
                ", ",
                &display(stamp2).to_utf8().unwrap_or_default(),
                ") — canon permits only unsuffixed or a single _MM_SS suffix",
            ]),
        ));
    }
    let occupants = slot_occupants(existing_map, &token, &combination, Some((&minute, &second)));
    let slot = py(&format!(
        "suffix({}_{}_{},{})",
        token.to_utf8().unwrap_or_default(),
        minute.to_utf8().unwrap_or_default(),
        second.to_utf8().unwrap_or_default(),
        combination.to_utf8().unwrap_or_default()
    ));
    if !matches!(
        align_or_fail(&occupants, &generated, &slot)?,
        Alignment::OtherMeasurement
    ) {
        return Ok(suffixed);
    }
    let names: Vec<String> = occupants.iter().map(|o| o.name.repr()).collect();
    Err(Failure::new(
        NAMING_COLLISION_EXHAUSTED,
        &format!(
            "bench_filename: suffix slot {} is occupied by OTHER measurement(s) [{}] of a sibling kind — canon permits only unsuffixed or a single _MM_SS suffix",
            suffixed.repr(),
            names.join(", ")
        ),
    ))
}
