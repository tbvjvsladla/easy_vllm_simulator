//! Measurement-tool container pin resolution: the library of `resolve_bench_tool.py`.
//!
//! The pin file (`bench_tool_pin.json`) declares the image of a measurement tool; this module reads one row of it
//! ([`load_pin`]), says which digests the row knows ([`accepted_digests`]), compares them with what the local
//! engine reports ([`classify`]) and asks the engine for that report ([`inspect`]). The digest is a provenance
//! fact, not a gate: a digest the record does not know is returned as `matches_record == Some(false)` and never
//! refused. Only an image that is **absent** is a failure, and the image is never pulled.
//!
//! Values that come from the pin file or from a caller keep Python's dynamic typing ([`PyValue`]) because the
//! original accepts any JSON shape and fails with the exception Python raises for it (`.get` on a list, hashing a
//! dict, iterating an integer). The decisions themselves - which row, which digest, which status - are typed here.

use crate::captured_process::{captured_text, run_captured_within, seconds_text, Captured};
use crate::dataset_commands::exec_argument;
use crate::dataset_prep::{is_os_or_value_error, read_text};
use crate::guidellm::Failure;
use crate::pydyn::{as_get_target, ensure_hashable, iterate};
use crate::pyjson::{loads_text, loads_text_objects, LoadError};
use crate::pysort::py_eq;
use crate::pystr::PyStr;
use crate::pyvalue::{DictKey, NonStringKey, PyDict, PyValue};
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::time::Duration;

/// The tool that is resolved when none is named.
pub const DEFAULT_TOOL: &str = "guidellm";
/// The image is on this node: its digest is recorded and the run proceeds.
pub const RC_OK: i32 = 0;
/// Argument or record-file error.
pub const RC_ARGS: i32 = 2;
/// The image is absent: it must be staged beforehand (no automatic pull, the airgap-safe invariant).
pub const RC_ABSENT: i32 = 3;
/// What the digest in a result was measured with.
pub const DIGEST_SOURCE: &str = "measured(docker image inspect .RepoDigests)";
/// Environment variable naming the default pin file. The launcher sets it to the file that ships beside the
/// scripts; a direct run of the binary falls back to the pin file of the crate's parent folder in the node's tree.
pub const PIN_PATH_ENV: &str = "BENCH_TOOL_PIN_PATH";
/// The class of every refusal of a malformed pin record (`PinError(ValueError)`).
pub const PIN_ERROR: &str = "PinError";

const INSPECT_TIMEOUT: Duration = Duration::from_secs(120);

/// Where the pin file is when the caller names none (`PIN_RELATIVE` of the original).
pub fn default_pin_path() -> PyStr {
    match std::env::var_os(PIN_PATH_ENV) {
        Some(path) if !path.is_empty() => {
            PyStr::from_surrogateescape(std::os::unix::ffi::OsStrExt::as_bytes(path.as_os_str()))
        }
        _ => PyStr::new(&format!(
            "{}/../bench_tool_pin.json",
            env!("CARGO_MANIFEST_DIR")
        )),
    }
}

fn pin_error(message: PyStr) -> Failure {
    Failure::with_message(PIN_ERROR, message)
}

/// `except PinError`: the failures [`load_pin`] raises for a record that cannot be judged.
pub fn is_pin_error(failure: &Failure) -> bool {
    failure.class == PIN_ERROR
}

// ── dict(...) ───────────────────────────────────────────────────────────────────

fn sequence_error(index: usize, length: usize) -> Failure {
    Failure::new(
        "ValueError",
        &format!("dictionary update sequence element #{index} has length {length}; 2 is required"),
    )
}

static NAN_KEYS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A value used as a dict key: strings stay strings; the other hashable JSON values are keys told apart by
/// equality (`1`, `1.0` and `True` are one key). Lists and dicts are unhashable.
pub(crate) fn dict_key(value: &PyValue) -> Result<DictKey, Failure> {
    ensure_hashable(value)?;
    let (identity, repr) = match value.peel() {
        PyValue::Str(text) => return Ok(DictKey::Str(text.clone())),
        PyValue::None => ("None".to_string(), "None".to_string()),
        PyValue::Bool(flag) => (format!("n:{}", u8::from(*flag)), value.repr()),
        PyValue::Int(number) => (format!("n:{number}"), value.repr()),
        PyValue::Float(number) if number.is_nan() => (
            format!(
                "nan:{}",
                NAN_KEYS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ),
            value.repr(),
        ),
        PyValue::Float(number) if number.is_finite() && number.fract() == 0.0 => {
            (format!("n:{number:.0}"), value.repr())
        }
        other => (format!("v:{}", other.repr()), other.repr()),
    };
    Ok(DictKey::Other(NonStringKey { identity, repr, value: None }))
}

/// One element of the iterable handed to `dict(...)`: a two-item sequence (a list, a two-character string, or a
/// two-key dict, whose keys are what iterating it yields).
fn pair_of(index: usize, element: &PyValue) -> Result<(PyValue, PyValue), Failure> {
    let items = match element.peel() {
        PyValue::List(_) | PyValue::Tuple(_) | PyValue::Dict(_) | PyValue::Str(_) => {
            iterate(element)?
        }
        _ => {
            return Err(Failure::new(
                "TypeError",
                &format!(
                    "cannot convert dictionary update sequence element #{index} to a sequence"
                ),
            ))
        }
    };
    match <[PyValue; 2]>::try_from(items) {
        Ok([key, value]) => Ok((key, value)),
        Err(items) => Err(sequence_error(index, items.len())),
    }
}

/// `dict(value)`: a dict is copied (keys and order kept); any other iterable must yield two-item sequences.
fn dict_from(value: &PyValue) -> Result<PyDict, Failure> {
    match value.peel() {
        PyValue::Dict(map) => Ok(map.clone()),
        PyValue::List(_) | PyValue::Tuple(_) | PyValue::Str(_) => {
            let mut row = PyDict::new();
            for (index, element) in iterate(value)?.iter().enumerate() {
                let (key, item) = pair_of(index, element)?;
                row.insert_key(dict_key(&key)?, item);
            }
            Ok(row)
        }
        other => Err(Failure::new(
            "TypeError",
            &format!("'{}' object is not iterable", other.type_name()),
        )),
    }
}

// ── load_pin ────────────────────────────────────────────────────────────────────

/// `open(path)` accepts text paths here; any other type is the `TypeError` Python raises for a path. An integer
/// would be a file descriptor, which is not modelled and is refused as such.
fn pin_path_text(path: &PyValue) -> Result<PyStr, Failure> {
    match path.peel() {
        PyValue::Str(text) => Ok(text.clone()),
        PyValue::Int(_) | PyValue::Bool(_) => Err(Failure::new(
            "NotImplementedError",
            "an integer file descriptor as the pin path is not modelled",
        )),
        other => Err(Failure::new(
            "TypeError",
            &format!(
                "expected str, bytes or os.PathLike object, not {}",
                other.type_name()
            ),
        )),
    }
}

/// `json.load(open(path, encoding="utf-8"))`: an unreadable file, undecodable text or malformed JSON is a
/// `PinError`; anything else (nesting beyond the recursion limit) escapes.
fn read_pin_document(path: &PyValue) -> Result<PyValue, Failure> {
    let text_path = pin_path_text(path)?;
    let read = read_text(&text_path).and_then(|text| {
        loads_text_objects(&text).map_err(|error| crate::repeat_api_cli::load_failure(&error))
    });
    match read {
        Ok(document) => Ok(document),
        Err(failure) if is_os_or_value_error(failure.class) => Err(pin_error(
            crate::pydyn::Text::new()
                .lit("기록 파일을 읽을 수 없다(")
                .val(path)
                .lit("): ")
                .str(&failure.message)
                .done(),
        )),
        Err(failure) => Err(failure),
    }
}

/// `sorted(mapping)` as `str(list)`: the keys in code point order, shown the way a list of strings is.
fn sorted_keys_text(map: &PyDict) -> String {
    let mut keys: Vec<&PyStr> = map.iter().map(|(key, _)| key).collect();
    keys.sort_by(|left, right| left.code_points().cmp(right.code_points()));
    PyValue::List(
        keys.into_iter()
            .map(|key| PyValue::Str(key.clone()))
            .collect(),
    )
    .repr()
}

/// `key in mapping` for a key of any type: unhashable keys are a `TypeError`; only a string can be a key of a
/// JSON object.
fn member<'a>(map: &'a PyDict, key: &PyValue) -> Result<Option<&'a PyValue>, Failure> {
    ensure_hashable(key)?;
    let PyValue::Str(text) = key.peel() else {
        return Ok(None);
    };
    Ok(map
        .keyed_entries()
        .find(|(other, name, _)| other.is_none() && *name == text)
        .map(|(_, _, value)| value))
}

/// The entry of one tool and version of a pin record (schema version 2, `versions{}` form), as a row of the
/// record's own keys with `version` added last.
///
/// An empty `version` (any falsy value) means the record's `default_version`, the version staged last; resolving
/// the newest release is the staging plane's job, never done here (the airgap-safe invariant).
pub fn load_pin(path: &PyValue, tool: &PyValue, version: &PyValue) -> Result<PyDict, Failure> {
    use crate::pydyn::Text;
    let document = read_pin_document(path)?;
    let tools = as_get_target(&document)?.get("tools");
    let known_tools = tools.and_then(crate::pydyn::dict_of);
    let entry = match known_tools {
        Some(known) => member(known, tool)?,
        None => None,
    };
    let (Some(_), Some(entry)) = (known_tools, entry) else {
        let listed = known_tools.map_or_else(|| "없음".to_string(), sorted_keys_text);
        return Err(pin_error(
            Text::new()
                .lit("기록에 도구 ")
                .lit(&tool.repr())
                .lit(" 이 없다(있는 것: ")
                .lit(&listed)
                .lit(")")
                .done(),
        ));
    };
    let tool_name = tool.display_text();
    let entry_map = as_get_target(entry)?;
    let versions = entry_map.get("versions").and_then(crate::pydyn::dict_of);
    let Some(versions) = versions.filter(|versions| !versions.is_empty()) else {
        return Err(pin_error(
            Text::new()
                .lit("기록 ")
                .str(&tool_name)
                // plan_26111708 B L8: the remedy names the live `resolve_guidellm.sh` launcher.
                .lit(".versions 가 비었다 — 스테이징한 버전이 없다(resolve_guidellm.sh --record 로 먼저 해소·스테이징하라)")
                .done(),
        ));
    };
    let wanted = if version.is_truthy() {
        version.clone()
    } else {
        crate::pydyn::or_none(entry_map.get("default_version"))
    };
    let Some(chosen) = member(versions, &wanted)? else {
        return Err(pin_error(
            Text::new()
                .lit("기록에 ")
                .str(&tool_name)
                .lit(" 버전 ")
                .lit(&wanted.repr())
                .lit(" 이 없다(있는 것: ")
                .lit(&sorted_keys_text(versions))
                .lit(") — 버전을 스테이징하라")
                .done(),
        ));
    };
    let wanted_name = wanted.display_text();
    let mut row = dict_from(chosen)?;
    row.set("version", wanted.clone());
    let image_ref_usable =
        matches!(row.get("image_ref"), Some(PyValue::Str(text)) if !text.is_blank());
    if !image_ref_usable {
        return Err(pin_error(
            Text::new()
                .lit("기록 ")
                .str(&tool_name)
                .lit(".")
                .str(&wanted_name)
                .lit(".image_ref 가 비었다")
                .done(),
        ));
    }
    let index_digest = crate::pydyn::or_none(row.get("index_digest"));
    let index_ok = match index_digest.peel() {
        PyValue::None => true,
        PyValue::Str(text) => text
            .code_points()
            .starts_with(PyStr::static_points("sha256:")),
        _ => false,
    };
    if !index_ok {
        return Err(pin_error(
            Text::new()
                .lit("기록 ")
                .str(&tool_name)
                .lit(".")
                .str(&wanted_name)
                .lit(".index_digest 가 sha256: 로 시작하지 않는다(")
                .lit(&index_digest.repr())
                .lit(")")
                .done(),
        ));
    }
    Ok(row)
}

// ── accepted_digests / classify ─────────────────────────────────────────────────

/// The digests a record knows: the index digest and every platform manifest digest. The set is for comparison, not
/// for judgement, and is not narrowed to a platform: the question is "is this the pinned tool", not "is the
/// architecture right" (the engine enforces the latter itself).
#[derive(Debug, Default)]
pub struct AcceptedDigests {
    /// The digests that are strings, in code point order (what `sorted()` of them gives).
    texts: BTreeSet<Vec<u32>>,
    /// A truthy index digest that is not a string still makes the set non-empty, and can never match a digest.
    has_non_text_member: bool,
}

impl AcceptedDigests {
    /// `not digests`.
    pub fn is_empty(&self) -> bool {
        self.texts.is_empty() && !self.has_non_text_member
    }

    pub fn contains(&self, digest: &[u32]) -> bool {
        self.texts.contains(digest)
    }

    pub fn texts(&self) -> impl Iterator<Item = PyStr> + '_ {
        self.texts
            .iter()
            .map(|points| PyStr::from_code_points(points.clone()))
    }

    fn add(&mut self, value: &PyValue) {
        match value.peel() {
            PyValue::Str(text) => {
                self.texts.insert(text.code_points().to_vec());
            }
            _ => self.has_non_text_member = true,
        }
    }
}

/// `accepted_digests(entry)`: `index_digest` when truthy (it must then be hashable) plus the non-empty string
/// values of `platform_digests` when that is an object.
pub fn accepted_digests(entry: &PyValue) -> Result<AcceptedDigests, Failure> {
    let record = as_get_target(entry)?;
    let mut digests = AcceptedDigests::default();
    if let Some(index) = record.get("index_digest").filter(|index| index.is_truthy()) {
        ensure_hashable(index)?;
        digests.add(index);
    }
    if let Some(PyValue::Dict(per_platform)) = record.get("platform_digests").map(PyValue::peel) {
        for (_, value) in per_platform.iter() {
            if matches!(value.peel(), PyValue::Str(text) if !text.is_empty()) {
                digests.add(value);
            }
        }
    }
    Ok(digests)
}

/// What a classification decided.
#[derive(Debug, PartialEq, Eq)]
pub enum Resolution {
    Resolved,
    Absent,
}

impl Resolution {
    pub fn code(&self) -> i32 {
        match self {
            Resolution::Resolved => RC_OK,
            Resolution::Absent => RC_ABSENT,
        }
    }

    pub fn status(&self) -> &'static str {
        match self {
            Resolution::Resolved => "resolved",
            Resolution::Absent => "absent",
        }
    }
}

/// `(rc, observed digest, matches record)`.
#[derive(Debug, PartialEq, Eq)]
pub struct Classification {
    pub resolution: Resolution,
    /// The digest that was seen: the smallest observed one the record knows, else the smallest observed one.
    pub observed_digest: Option<PyStr>,
    /// `Some(true)`: a known digest was observed; `Some(false)`: the record knows digests, none was observed;
    /// `None`: the comparison did not take place (nothing observable, or the record knows no digest).
    pub matches_record: Option<bool>,
}

impl Classification {
    /// The tuple the original returns.
    pub fn to_value(&self) -> PyValue {
        PyValue::Tuple(vec![
            PyValue::int(self.resolution.code()),
            self.observed_digest
                .clone()
                .map_or(PyValue::None, PyValue::Str),
            self.matches_record.map_or(PyValue::None, PyValue::Bool),
        ])
    }
}

/// The digests (the part after the first `@`) of every item of a `RepoDigests` list that has one, in code point
/// order and without repeats. Items follow Python's `"@" in item` and `item.split("@", 1)`.
fn observed_digests(repo_digests: &PyValue) -> Result<BTreeSet<Vec<u32>>, Failure> {
    let at = u32::from('@');
    let mut observed = BTreeSet::new();
    for item in iterate(repo_digests)? {
        match item.peel() {
            PyValue::Str(text) => {
                if let Some(position) = text.code_points().iter().position(|&point| point == at) {
                    observed.insert(text.code_points()[position + 1..].to_vec());
                }
            }
            PyValue::List(_) | PyValue::Tuple(_) | PyValue::Dict(_) => {
                let holds_at = match item.peel() {
                    PyValue::Dict(map) => map.contains_key("@"),
                    _ => iterate(&item)?
                        .iter()
                        .any(|element| py_eq(element, &PyValue::str("@"))),
                };
                if holds_at {
                    return Err(Failure::new(
                        "AttributeError",
                        &format!("'{}' object has no attribute 'split'", item.type_name()),
                    ));
                }
            }
            other => {
                return Err(Failure::new(
                    "TypeError",
                    &format!("argument of type '{}' is not iterable", other.type_name()),
                ))
            }
        }
    }
    Ok(observed)
}

/// `classify(entry, repo_digests)`: the local observation against the record. Pure (the engine is not asked).
/// `repo_digests` is `None` when the image is absent; any other value is iterated as the original does.
pub fn classify(entry: &PyValue, repo_digests: &PyValue) -> Result<Classification, Failure> {
    if matches!(repo_digests.peel(), PyValue::None) {
        return Ok(Classification {
            resolution: Resolution::Absent,
            observed_digest: None,
            matches_record: None,
        });
    }
    let observed = observed_digests(repo_digests)?;
    let known = accepted_digests(entry)?;
    if let Some(hit) = observed.iter().find(|digest| known.contains(digest)) {
        return Ok(Classification {
            resolution: Resolution::Resolved,
            observed_digest: Some(PyStr::from_code_points(hit.clone())),
            matches_record: Some(true),
        });
    }
    Ok(Classification {
        resolution: Resolution::Resolved,
        observed_digest: observed
            .iter()
            .next()
            .map(|digest| PyStr::from_code_points(digest.clone())),
        matches_record: if known.is_empty() { None } else { Some(false) },
    })
}

// ── inspect ─────────────────────────────────────────────────────────────────────

/// `subprocess.run(command, capture_output=True, timeout=120)`.
fn run_captured(command: &[OsString], shown: &[PyValue]) -> Result<Captured, Failure> {
    run_captured_within(
        command,
        shown,
        INSPECT_TIMEOUT,
        &seconds_text(INSPECT_TIMEOUT),
    )
}

/// `_inspect(image_ref)`: the `RepoDigests` of the local image, or `None` when the engine does not have it (or
/// answers with anything but a JSON list). **Never pulls.** Both streams are decoded before the exit status is
/// looked at, so undecodable output is an error even for a failed command.
pub fn inspect(image_ref: &PyValue) -> Result<Option<Vec<PyValue>>, Failure> {
    let program = OsString::from("docker");
    let reference = exec_argument(image_ref)?;
    let command = [
        program,
        OsString::from("image"),
        OsString::from("inspect"),
        reference,
        OsString::from("--format"),
        OsString::from("{{json .RepoDigests}}"),
    ];
    let shown = [
        PyValue::str("docker"),
        PyValue::str("image"),
        PyValue::str("inspect"),
        image_ref.peel().clone(),
        PyValue::str("--format"),
        PyValue::str("{{json .RepoDigests}}"),
    ];
    let captured = run_captured(&command, &shown)?;
    let stdout = captured_text(&captured.stdout)?;
    captured_text(&captured.stderr)?;
    if captured.status != 0 {
        return Ok(None);
    }
    let stripped = stdout.strip();
    let document = if stripped.is_empty() {
        PyStr::new("null")
    } else {
        stripped
    };
    match loads_text(&document) {
        Ok(PyValue::List(items)) => Ok(Some(items)),
        Ok(_) => Ok(None),
        Err(error @ LoadError::TooDeep { .. }) => Err(crate::repeat_api_cli::load_failure(&error)),
        Err(_) => Ok(None),
    }
}

#[cfg(test)]
mod capture_tests {
    use super::*;

    #[test]
    fn the_production_limit_is_120_seconds_and_integral_seconds_have_no_fraction() {
        assert_eq!(INSPECT_TIMEOUT, Duration::from_secs(120));
        assert_eq!(seconds_text(INSPECT_TIMEOUT), "120");
        assert_eq!(seconds_text(Duration::from_millis(300)), "0.3");
    }
}
