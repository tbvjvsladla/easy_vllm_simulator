//! The full-bench knob preset contract: a Rust port of
//! `.claude/skills/adversarial-benchmark/scripts/bench_preset.py`, the single owner of the preset schema, its
//! loader, the knob resolution order and the record block `bench_knobs`.
//!
//! Every function takes and returns the values the original's callers pass, as [`PyValue`]s: a preset document is
//! whatever `yaml.safe_load` produced, and the checks below look at it with Python's rules (`==` across `bool`/`int`/
//! `float`, truthiness, dict and list membership, `repr`/`str` in messages) so that a wrong document is rejected
//! with the same text, and an exception happens at the same operation, as in the original.
//!
//! * [`validate_preset`] returns the list of contract violations (empty = pass); [`load_bindings`], [`preset_path`],
//!   [`load_preset`] and [`serving_caps`] read files; [`resolve`] is the whole resolution.
//! * The repeat count is resolved by the repeat-axis library ([`crate::repeat_owner::resolve_repeats`]), whose
//!   campaign declaration is read through the campaign owner it is given; nothing here re-implements those rules.
//! * The original located itself by its file path (`SKILL_DIR`, `REPO_ROOT`). A native build has no such file: the
//!   skill directory and the repository root are parameters ([`SkillDir`], [`RepoRoot`]), taken by the programs from
//!   `BENCH_PRESET_SKILL_DIR` and `BENCH_PRESET_REPO_ROOT`, and an operation that needs one without it fails by name.

use crate::guidellm::Failure;
use crate::pyfs;
use crate::pyjson::{decode_file_text, loads_text_objects};
use crate::pyops::{compare, Compare};
use crate::pysort::{py_eq, same_or_eq, sorted_by_key};
use crate::pystr::{is_python_space, PyStr};
use crate::pyvalue::{DumpError, NonStringKey, Opaque, PyDict, PyValue, Typed};
use crate::pyyaml;
use crate::repeat_axis::REPEAT_KIND_WARM;
use crate::repeat_owner::{
    load_campaign_budgets, resolve_repeats, CampaignOwner, RepoRoot, ResolveRequest,
};
use crate::repeat_text::cat;
use crate::FULL_REPEATS_MIN;
use num_bigint::BigInt;
use num_traits::Zero;
use ring::digest::{Context, SHA256};
use std::borrow::Cow;
use std::collections::HashSet;
use std::io::Read;

// ── constants ───────────────────────────────────────────────────────────────────

pub const BINDINGS_REL: &str = "config/bench_bindings.yaml";
pub const FULL_BENCH: &str = "full-bench";
pub const DEFAULT_PRESET: &str = "default_preset";
pub const SCHEMA_VERSION: i64 = 1;
pub const GRADES: [&str; 3] = ["auto", "confirm", "escalate"];
pub const OWNERS: [&str; 2] = ["agent", "user"];
pub const CAP_LE_MAX_NUM_SEQS: &str = "le_max_num_seqs";
pub const CAP_INPUT_PLUS_OUTPUT: &str = "input_plus_output_le_max_model_len";
pub const CAP_NONE: &str = "none";
pub const HUMAN_PIN_REASON: &str = "human_pin";
pub const DEFAULT_REASON: &str = "preset_default";
/// The verdict point (concurrency 1); `sweep_bench` reuses it.
pub const VERDICT_LEVEL: i64 = 1;
/// Exception class of the original's `PresetError` (a `ValueError`).
pub const PRESET_ERROR: &str = "PresetError";
/// Exception class when an operation needs the skill directory and none is configured.
pub const CONFIGURATION_ERROR: &str = "BenchPresetConfigurationError";
pub const SKILL_DIR_ENV: &str = "BENCH_PRESET_SKILL_DIR";
pub const REPO_ROOT_ENV: &str = "BENCH_PRESET_REPO_ROOT";

pub const TOP_KEYS: [&str; 5] = ["schema_version", "bench", "tool", "reason_codes", "knobs"];
pub const KNOB_KEYS: [&str; 6] = [
    "owner",
    "kind",
    "default",
    "serving_cap",
    "options",
    "reasons",
];
pub const OPTION_KEYS: [&str; 2] = ["value", "grade"];
pub const DATASET_KNOBS: [&str; 9] = [
    "dataset_kind",
    "dataset_source",
    "dataset_id",
    "dataset_path",
    "dataset_revision",
    "source_sha256",
    "column_map",
    "preprocess_strategy",
    "seed",
];
pub const DATASET_SYNTHETIC: &str = "synthetic";
pub const DATASET_KINDS: [&str; 4] = [DATASET_SYNTHETIC, "parquet_file", "json_file", "csv_file"];
pub const DATASET_SOURCES: [&str; 3] = ["managed", "ephemeral", "custom"];
pub const PREPROCESS_STRATEGIES: [&str; 4] = ["concatenate", "ignore", "pad", "error"];
pub const COLUMN_MAP_KEYS: [&str; 1] = ["text_column"];
pub const KNOWN_TOOLS: [&str; 2] = ["vllm", "guidellm"];

/// The Python type of a knob's values (`typ` in the original's table).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueType {
    Int,
    Str,
    Dict,
}

impl ValueType {
    /// `typ.__name__`
    pub fn name(self) -> &'static str {
        match self {
            ValueType::Int => "int",
            ValueType::Str => "str",
            ValueType::Dict => "dict",
        }
    }

    /// `_typed(value, typ)`: an `int` excludes `bool`.
    pub fn accepts(self, value: &PyValue) -> bool {
        matches!(
            (self, value.peel()),
            (ValueType::Int, PyValue::Int(_))
                | (ValueType::Str, PyValue::Str(_))
                | (ValueType::Dict, PyValue::Dict(_))
        )
    }
}

/// One row of the closed knob table.
#[derive(Debug)]
pub struct KnobSpec {
    pub name: &'static str,
    pub kind: &'static str,
    pub value_type: ValueType,
    /// The serving-cap filters the knob allows.
    pub caps: &'static [&'static str],
    /// Whether the preset must declare no default (a missing value refuses the start).
    pub no_default: bool,
    /// The sweep flag of the knob; the dataset knobs have none and are set by `--pin`.
    pub flag: Option<&'static str>,
    /// The key of the knob in the printed `KEY<US>VALUE` lines.
    pub shell_key: Option<&'static str>,
}

const fn knob(
    name: &'static str,
    kind: &'static str,
    value_type: ValueType,
    caps: &'static [&'static str],
    no_default: bool,
    flag: Option<&'static str>,
    shell_key: Option<&'static str>,
) -> KnobSpec {
    KnobSpec {
        name,
        kind,
        value_type,
        caps,
        no_default,
        flag,
        shell_key,
    }
}

const CAPS_SEQS: &[&str] = &[CAP_LE_MAX_NUM_SEQS, CAP_NONE];
const CAPS_MODEL_LEN: &[&str] = &[CAP_INPUT_PLUS_OUTPUT, CAP_NONE];
const CAPS_NONE: &[&str] = &[CAP_NONE];

/// `KNOB_SPECS` of the original, in its order.
pub const KNOB_SPECS: [KnobSpec; 17] = [
    knob(
        "levels",
        "list",
        ValueType::Int,
        CAPS_SEQS,
        false,
        Some("--levels"),
        Some("LEVELS"),
    ),
    knob(
        "input_len",
        "scalar",
        ValueType::Int,
        CAPS_MODEL_LEN,
        false,
        Some("--input-len"),
        Some("ILEN"),
    ),
    knob(
        "output_len",
        "scalar",
        ValueType::Int,
        CAPS_NONE,
        false,
        Some("--output-len"),
        Some("OLEN"),
    ),
    knob(
        "num_prompts",
        "scalar",
        ValueType::Int,
        CAPS_NONE,
        false,
        Some("--num-prompts"),
        Some("NPROMPTS"),
    ),
    knob(
        "warmups",
        "scalar",
        ValueType::Int,
        CAPS_NONE,
        false,
        Some("--warmups"),
        Some("WARMUPS"),
    ),
    knob(
        "repeats",
        "scalar",
        ValueType::Int,
        CAPS_NONE,
        false,
        Some("--repeats"),
        Some("REPEATS"),
    ),
    knob(
        "backend",
        "scalar",
        ValueType::Str,
        CAPS_NONE,
        true,
        Some("--backend"),
        Some("BACKEND"),
    ),
    knob(
        "bench_budget_mib",
        "scalar",
        ValueType::Int,
        CAPS_NONE,
        true,
        Some("--bench-budget-mib"),
        Some("BENCH_BUDGET_MIB"),
    ),
    knob(
        "dataset_kind",
        "scalar",
        ValueType::Str,
        CAPS_NONE,
        false,
        None,
        None,
    ),
    knob(
        "dataset_source",
        "scalar",
        ValueType::Str,
        CAPS_NONE,
        false,
        None,
        None,
    ),
    knob(
        "dataset_id",
        "text",
        ValueType::Str,
        CAPS_NONE,
        false,
        None,
        None,
    ),
    knob(
        "dataset_path",
        "path",
        ValueType::Str,
        CAPS_NONE,
        false,
        None,
        None,
    ),
    knob(
        "dataset_revision",
        "hex40",
        ValueType::Str,
        CAPS_NONE,
        false,
        None,
        None,
    ),
    knob(
        "source_sha256",
        "hex64",
        ValueType::Str,
        CAPS_NONE,
        false,
        None,
        None,
    ),
    knob(
        "column_map",
        "map",
        ValueType::Dict,
        CAPS_NONE,
        false,
        None,
        None,
    ),
    knob(
        "preprocess_strategy",
        "scalar",
        ValueType::Str,
        CAPS_NONE,
        false,
        None,
        None,
    ),
    knob(
        "seed",
        "scalar",
        ValueType::Int,
        CAPS_NONE,
        false,
        None,
        None,
    ),
];

/// The spec of a knob name, as `KNOB_SPECS[name]`.
pub fn knob_spec(name: &str) -> Option<&'static KnobSpec> {
    KNOB_SPECS.iter().find(|spec| spec.name == name)
}

/// The sweep-flag text of a knob (`FLAG_OF[name]`): a flag, or `--pin <knob>` for the dataset knobs.
fn flag_of(name: &str) -> String {
    match knob_spec(name).and_then(|spec| spec.flag) {
        Some(flag) => flag.to_string(),
        None => format!("--pin {name}"),
    }
}

fn is_dataset_knob(name: &str) -> bool {
    DATASET_KNOBS.contains(&name)
}

// ── failures and Python helpers ─────────────────────────────────────────────────

/// `PresetError(message)`.
pub fn preset_error(message: PyStr) -> Failure {
    Failure::with_message(PRESET_ERROR, message)
}

fn type_error(message: &str) -> Failure {
    Failure::new("TypeError", message)
}

fn key_error(key: &str) -> Failure {
    Failure::with_message("KeyError", PyStr::new(&PyStr::new(key).repr()))
}

fn strs(names: &[&str]) -> PyValue {
    PyValue::List(names.iter().map(|name| PyValue::str(name)).collect())
}

/// `str(list_of_names)`.
fn names_repr(names: &[&str]) -> String {
    strs(names).repr()
}

fn is_str(value: &PyValue, text: &str) -> bool {
    matches!(value.peel(), PyValue::Str(own) if own.equals(text))
}

/// A document value that is absent reads as this (`dict.get(key)` without a default).
const NONE: &PyValue = &PyValue::None;

/// `map.get(key)` of a dict, `None` when absent.
fn field_of<'a>(map: &'a PyDict, key: &str) -> &'a PyValue {
    map.get(key).unwrap_or(NONE)
}

/// Whether the dict's keys are exactly the strings `wanted` (in any order). The common case, the keys in the order
/// of `wanted`, is one pass without lookups.
fn keys_exactly(map: &PyDict, wanted: &[&str]) -> bool {
    if map.len() != wanted.len() {
        return false;
    }
    let in_order = map
        .keyed_entries()
        .zip(wanted)
        .all(|((other, key, _), name)| other.is_none() && key.equals(name));
    in_order || wanted.iter().all(|name| map.get(name).is_some())
}

/// `value == number` for a small integer (`bool`, `int` and integral `float` compare equal to it).
fn is_small_int(value: &PyValue, number: i64) -> bool {
    use num_traits::ToPrimitive;
    match value.peel() {
        PyValue::Int(own) => own.to_i64() == Some(number),
        PyValue::Bool(flag) => i64::from(*flag) == number,
        PyValue::Float(own) => *own == number as f64,
        _ => false,
    }
}

fn list_contains(items: &[PyValue], needle: &PyValue) -> bool {
    items.iter().any(|item| same_or_eq(item, needle))
}

fn text_contains(items: &[&str], value: &PyValue) -> bool {
    items.iter().any(|item| is_str(value, item))
}

fn int_value(number: i64) -> PyValue {
    PyValue::int(number)
}

/// `value[key]`: a dict item, or the error Python raises.
fn item(value: &PyValue, key: &str) -> Result<PyValue, Failure> {
    match value.peel() {
        PyValue::Dict(map) => map.get(key).cloned().ok_or_else(|| key_error(key)),
        PyValue::List(_) | PyValue::Tuple(_) => Err(type_error(&format!(
            "{} indices must be integers or slices, not str",
            value.type_name()
        ))),
        PyValue::Str(_) => Err(type_error("string indices must be integers, not 'str'")),
        other => Err(type_error(&format!(
            "'{}' object is not subscriptable",
            other.type_name()
        ))),
    }
}

/// `value.get(key)` on a dict; any other type has no `get`.
fn get_or_none(value: &PyValue, key: &str) -> Result<PyValue, Failure> {
    match value.peel() {
        PyValue::Dict(map) => Ok(map.get(key).cloned().unwrap_or(PyValue::None)),
        other => Err(Failure::new(
            "AttributeError",
            &format!("'{}' object has no attribute 'get'", other.type_name()),
        )),
    }
}

/// The keys of a dict as the values Python sees (a non-string key is rebuilt from the `repr` it was kept by).
fn dict_keys(map: &PyDict) -> Vec<PyValue> {
    map.keyed_entries()
        .map(|(other, key, _)| match other {
            Some(other) => rebuild_key(other),
            None => PyValue::Str(key.clone()),
        })
        .collect()
}

fn rebuild_key(key: &NonStringKey) -> PyValue {
    let repr = key.repr.as_str();
    match repr {
        "None" => PyValue::None,
        "True" => PyValue::Bool(true),
        "False" => PyValue::Bool(false),
        "inf" => PyValue::Float(f64::INFINITY),
        "-inf" => PyValue::Float(f64::NEG_INFINITY),
        "nan" => PyValue::Float(f64::NAN),
        other => {
            if let Some(number) = BigInt::parse_bytes(other.as_bytes(), 10) {
                PyValue::Int(number)
            } else if let Ok(number) = other.parse::<f64>() {
                PyValue::Float(number)
            } else {
                let type_name = if other.starts_with("datetime.datetime") {
                    "datetime.datetime"
                } else if other.starts_with("datetime.date") {
                    "datetime.date"
                } else {
                    "bytes"
                };
                PyValue::Opaque(Opaque {
                    type_name,
                    repr: other.to_string(),
                    truthy: true,
                    typed: Typed::Plain,
                    identity: None,
                })
            }
        }
    }
}

/// `key in dict`: a hash lookup, so a list or dict key is a `TypeError`.
fn dict_has_key(map: &PyDict, key: &PyValue) -> Result<bool, Failure> {
    check_hashable(key)?;
    Ok(dict_keys(map).iter().any(|own| same_or_eq(own, key)))
}

/// The names of the keys that are strings equal to one of `wanted`, and the other keys of the dict sorted as
/// `sorted(set(d) - set(wanted))` does (which can fail on keys that do not compare).
struct KeyContract {
    missing: Vec<&'static str>,
    extra: Vec<PyValue>,
}

impl KeyContract {
    fn of(map: &PyDict, wanted: &[&'static str]) -> Result<Self, Failure> {
        let keys = dict_keys(map);
        let mut extra: Vec<PyValue> = Vec::new();
        for key in &keys {
            let known = wanted.iter().any(|name| is_str(key, name));
            if !known {
                extra.push(key.clone());
            }
        }
        let mut missing: Vec<&'static str> = wanted
            .iter()
            .copied()
            .filter(|name| !keys.iter().any(|key| is_str(key, name)))
            .collect();
        missing.sort_unstable();
        let extra = if extra.is_empty() {
            extra
        } else {
            let sort_keys = extra.clone();
            sorted_by_key(extra, &sort_keys)?
        };
        Ok(KeyContract { missing, extra })
    }

    fn holds(&self) -> bool {
        self.missing.is_empty() && self.extra.is_empty()
    }

    fn missing_repr(&self) -> String {
        names_repr(&self.missing)
    }

    fn extra_repr(&self) -> String {
        PyValue::List(self.extra.clone()).repr()
    }
}

// ── hashing of dict keys ────────────────────────────────────────────────────────

/// A hashable value is any value Python can hash: a list, dict or set is not (`TypeError`), a tuple is when its
/// items are.
fn check_hashable(value: &PyValue) -> Result<(), Failure> {
    match value.peel() {
        PyValue::List(_) => Err(type_error("unhashable type: 'list'")),
        PyValue::Dict(_) => Err(type_error("unhashable type: 'dict'")),
        PyValue::Opaque(opaque) if opaque.type_name == "set" => {
            Err(type_error("unhashable type: 'set'"))
        }
        PyValue::Tuple(items) => items.iter().try_for_each(check_hashable),
        _ => Ok(()),
    }
}

/// An insertion-ordered dict keyed by hashable values: `grade_of` of the original. Keys compare as a Python dict
/// compares them (`==` for hashable values, so `1`, `1.0` and `True` are one key); the maps here hold a handful of
/// entries, so a lookup is a scan. Keys and grades are borrowed from the document being read where they can be.
#[derive(Default)]
struct GradeMap<'a> {
    entries: Vec<(Cow<'a, PyValue>, Cow<'a, PyValue>)>,
}

impl<'a> GradeMap<'a> {
    fn position(&self, key: &PyValue) -> Result<Option<usize>, Failure> {
        check_hashable(key)?;
        Ok(self
            .entries
            .iter()
            .position(|(own, _)| same_or_eq(own, key)))
    }

    /// `key in grade_of`
    fn contains(&self, key: &PyValue) -> Result<bool, Failure> {
        Ok(self.position(key)?.is_some())
    }

    /// `grade_of[key]`
    fn get(&self, key: &PyValue) -> Result<Option<&PyValue>, Failure> {
        Ok(self.position(key)?.map(|at| &*self.entries[at].1))
    }

    /// `grade_of[key] = grade`: an existing key keeps its place and its first spelling.
    fn set(&mut self, key: Cow<'a, PyValue>, grade: Cow<'a, PyValue>) -> Result<(), Failure> {
        match self.position(&key)? {
            Some(at) => self.entries[at].1 = grade,
            None => self.entries.push((key, grade)),
        }
        Ok(())
    }

    fn iter(&self) -> impl Iterator<Item = (&PyValue, &PyValue)> {
        self.entries.iter().map(|(key, grade)| (&**key, &**grade))
    }
}

// ── json.dumps(value, sort_keys=True) ───────────────────────────────────────────

fn dump_failure(error: DumpError) -> Failure {
    let text = error.to_string();
    let message = text
        .strip_prefix(&format!("{}: ", error.python_type()))
        .unwrap_or(&text)
        .to_string();
    Failure::new(error.python_type(), &message)
}

/// `json.dumps(value, sort_keys=True)`: default separators, ASCII output, the keys of every dict sorted (the C
/// encoder sorts a dict's items when it reaches it, so a sort that fails does so at that point of the walk).
pub fn dumps_sorted(value: &PyValue) -> Result<String, Failure> {
    let mut out = String::new();
    write_sorted(value, &mut out)?;
    Ok(out)
}

fn write_sorted(value: &PyValue, out: &mut String) -> Result<(), Failure> {
    match value.peel() {
        PyValue::List(items) | PyValue::Tuple(items) => {
            out.push('[');
            for (index, element) in items.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                write_sorted(element, out)?;
            }
            out.push(']');
        }
        PyValue::Dict(map) => {
            let mut entries: Vec<(PyValue, &PyValue)> = map
                .keyed_entries()
                .map(|(other, key, value)| {
                    (
                        match other {
                            Some(other) => rebuild_key(other),
                            None => PyValue::Str(key.clone()),
                        },
                        value,
                    )
                })
                .collect();
            let keys: Vec<PyValue> = entries.iter().map(|(key, _)| key.clone()).collect();
            let order = sorted_by_key((0..entries.len()).collect(), &keys)?;
            let mut sorted: Vec<(PyValue, &PyValue)> = Vec::with_capacity(entries.len());
            for index in order {
                sorted.push(std::mem::replace(
                    &mut entries[index],
                    (PyValue::None, &PyValue::None),
                ));
            }
            out.push('{');
            for (index, (key, entry)) in sorted.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                let spelled = match key {
                    PyValue::Str(_) => key.clone(),
                    PyValue::None => PyValue::str("null"),
                    PyValue::Bool(true) => PyValue::str("true"),
                    PyValue::Bool(false) => PyValue::str("false"),
                    PyValue::Int(number) => PyValue::str(&number.to_string()),
                    PyValue::Float(_) => PyValue::str(&key.dumps_ascii().map_err(dump_failure)?),
                    other => {
                        return Err(type_error(&format!(
                            "keys must be str, int, float, bool or None, not {}",
                            other.type_name().rsplit('.').next().unwrap_or("")
                        )))
                    }
                };
                out.push_str(&spelled.dumps_ascii().map_err(dump_failure)?);
                out.push_str(": ");
                write_sorted(entry, out)?;
            }
            out.push('}');
        }
        scalar => out.push_str(&scalar.dumps_ascii().map_err(dump_failure)?),
    }
    Ok(())
}

/// `_norm(v)` that keeps a value that needs no conversion where it is.
fn norm_cow(value: &PyValue) -> Result<Cow<'_, PyValue>, Failure> {
    match value.peel() {
        PyValue::Dict(_) => Ok(Cow::Owned(PyValue::str(&dumps_sorted(value)?))),
        _ => Ok(Cow::Borrowed(value)),
    }
}

/// `_norm(v)` for callers outside this module.
pub fn norm_value(value: &PyValue) -> Result<PyValue, Failure> {
    norm(value)
}

/// `_norm(v)`: a dict is compared by its sorted-key JSON text (a dict is not hashable).
fn norm(value: &PyValue) -> Result<PyValue, Failure> {
    match value.peel() {
        PyValue::Dict(_) => Ok(PyValue::str(&dumps_sorted(value)?)),
        _ => Ok(value.clone()),
    }
}

// ── configured locations ────────────────────────────────────────────────────────

/// The skill directory (`SKILL_DIR`): where `config/` lives.
#[derive(Clone, Debug, Default)]
pub struct SkillDir(Location);

#[derive(Clone, Debug, Default)]
enum Location {
    #[default]
    Unset,
    Given(PyStr),
    Invalid(String),
}

impl SkillDir {
    pub fn given(path: PyStr) -> Self {
        SkillDir(Location::Given(path))
    }

    pub fn unset() -> Self {
        SkillDir(Location::Unset)
    }

    /// A directory as a caller's JSON value gives it: only a string is a path.
    pub fn from_value(value: &PyValue) -> Self {
        match value.peel() {
            PyValue::Str(path) => SkillDir::given(path.clone()),
            other => SkillDir(Location::Invalid(format!(
                "argument should be a str or an os.PathLike object where __fspath__ returns a str, not '{}'",
                other.type_name()
            ))),
        }
    }

    /// The directory named by [`SKILL_DIR_ENV`], canonicalised as the original's `Path(__file__).resolve()`.
    pub fn from_environment() -> Self {
        use std::os::unix::ffi::OsStrExt;
        match std::env::var_os(SKILL_DIR_ENV).filter(|value| !value.is_empty()) {
            Some(value) => {
                SkillDir::given(resolve_text(&PyStr::from_surrogateescape(value.as_bytes())))
            }
            None => SkillDir::unset(),
        }
    }

    pub fn get(&self) -> Result<&PyStr, Failure> {
        match &self.0 {
            Location::Given(path) => Ok(path),
            Location::Invalid(message) => Err(type_error(message)),
            Location::Unset => Err(Failure::new(
                CONFIGURATION_ERROR,
                &format!("the skill directory is not configured; set {SKILL_DIR_ENV} (the original derived it from its own file location)"),
            )),
        }
    }
}

/// The repository root of the preset programs: [`REPO_ROOT_ENV`], else the repeat axis's own variable (the same
/// repository), canonicalised as the original's `Path(__file__).resolve().parents[3]`.
pub fn repo_root_from_environment() -> RepoRoot {
    use std::os::unix::ffi::OsStrExt;
    match std::env::var_os(REPO_ROOT_ENV).filter(|value| !value.is_empty()) {
        Some(value) => {
            RepoRoot::given(resolve_text(&PyStr::from_surrogateescape(value.as_bytes())))
        }
        None => RepoRoot::from_environment(),
    }
}

/// `str(Path(text).resolve())` (non-strict: a path that does not exist is only normalised).
pub fn resolve_text(text: &PyStr) -> PyStr {
    use std::os::unix::ffi::OsStrExt;
    let absolute = if text.code_points().first() == Some(&u32::from('/')) {
        pyfs::normalize(text)
    } else {
        pyfs::join_text(&current_dir(), text)
    };
    let Ok(os) = pyfs::os_string(&absolute) else {
        return absolute;
    };
    match std::fs::canonicalize(&os) {
        Ok(real) => PyStr::from_surrogateescape(real.as_os_str().as_bytes()),
        Err(_) => absolute,
    }
}

/// `Path.cwd()`
fn current_dir() -> PyStr {
    use std::os::unix::ffi::OsStrExt;
    match std::env::current_dir() {
        Ok(path) => PyStr::from_surrogateescape(path.as_os_str().as_bytes()),
        Err(_) => PyStr::new("."),
    }
}

/// `Path(value)` for a caller's value: a string, normalised, or the `TypeError` of any other type.
fn path_of(value: &PyValue) -> Result<PyStr, Failure> {
    match value.peel() {
        PyValue::Str(text) => Ok(pyfs::normalize(text)),
        other => Err(type_error(&format!(
            "expected str, bytes or os.PathLike object, not {}",
            other.type_name()
        ))),
    }
}

// ── loading ─────────────────────────────────────────────────────────────────────

/// The number of Python frames the original keeps open around `yaml.safe_load` (module, `main`, the command,
/// `load_preset`, `_load_yaml`, ...): the recursion budget of a deeply nested document.
#[derive(Clone, Copy, Debug)]
pub struct CallDepth(pub usize);

impl CallDepth {
    /// `<module>`, `main`, `load_preset`, `_load_yaml` — the `validate` and `default` commands.
    pub const PRESET_COMMAND: CallDepth = CallDepth(4);
    /// The same under `_cmd_resolve`.
    pub const RESOLVE_COMMAND: CallDepth = CallDepth(5);
    /// A direct call of a public function (the frame of the caller counts as one).
    pub const API: CallDepth = CallDepth(1);
}

/// `_load_yaml(path)`: an unreadable file or a YAML error is a `PresetError`; a document that is not UTF-8 is not.
fn load_yaml(path: &PyStr, depth: CallDepth) -> Result<PyValue, Failure> {
    let os = pyfs::os_string(path).map_err(|failure| failure.into_failure())?;
    let bytes = match std::fs::read(std::path::Path::new(&os)) {
        Ok(bytes) => bytes,
        Err(error) => {
            return Err(preset_error(cat!(
                path,
                " 를 읽지 못했다: ",
                crate::cli::os_error_text(&error, path)
            )))
        }
    };
    let text = decode_file_text(&bytes).map_err(|error| {
        let text = error.to_string();
        Failure::new("UnicodeDecodeError", &text)
    })?;
    match pyyaml::safe_load_text(&text, depth.0) {
        Ok(document) => Ok(document),
        Err(error) if is_yaml_error(error.class) => Err(preset_error(cat!(
            path,
            " 가 yaml 이 아니다: ",
            error.message
        ))),
        Err(error) => Err(Failure::with_message(error.class, error.message)),
    }
}

/// The classes of `yaml.YAMLError` the original's `_load_yaml` catches.
fn is_yaml_error(class: &str) -> bool {
    matches!(
        class,
        "ReaderError" | "ScannerError" | "ParserError" | "ComposerError" | "ConstructorError"
    )
}

/// `str.fullmatch(r"[a-z0-9][a-z0-9._-]*")`
fn is_folder_name(text: &PyStr) -> bool {
    let points = text.code_points();
    let lower_or_digit = |p: u32| {
        (u32::from('a')..=u32::from('z')).contains(&p)
            || (u32::from('0')..=u32::from('9')).contains(&p)
    };
    match points.split_first() {
        Some((&first, rest)) => {
            lower_or_digit(first)
                && rest.iter().all(|&p| {
                    lower_or_digit(p)
                        || p == u32::from('.')
                        || p == u32::from('_')
                        || p == u32::from('-')
                })
        }
        None => false,
    }
}

/// `str.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*")`
fn is_preset_name(text: &PyStr) -> bool {
    let points = text.code_points();
    let alnum = |p: u32| {
        (u32::from('a')..=u32::from('z')).contains(&p)
            || (u32::from('A')..=u32::from('Z')).contains(&p)
            || (u32::from('0')..=u32::from('9')).contains(&p)
    };
    match points.split_first() {
        Some((&first, rest)) => {
            alnum(first)
                && rest.iter().all(|&p| {
                    alnum(p) || p == u32::from('.') || p == u32::from('_') || p == u32::from('-')
                })
        }
        None => false,
    }
}

/// `load_bindings(skill_dir)`: the bindings document, when it is exactly `{full-bench: <existing tool folder>}`.
pub fn load_bindings(skill_dir: &SkillDir, depth: CallDepth) -> Result<PyValue, Failure> {
    let skill = pyfs::normalize(skill_dir.get()?);
    let path = pyfs::join_text(&skill, &PyStr::new(BINDINGS_REL));
    let document = load_yaml(&path, depth)?;
    let single = match document.peel() {
        PyValue::Dict(map) => {
            let keys = dict_keys(map);
            keys.len() == 1 && is_str(&keys[0], FULL_BENCH)
        }
        _ => false,
    };
    if !single {
        let shown = match document.peel() {
            PyValue::Dict(map) => {
                let keys = dict_keys(map);
                let order = sorted_by_key(keys.clone(), &keys)?;
                PyValue::List(order).repr()
            }
            other => other.repr(),
        };
        return Err(preset_error(cat!(
            BINDINGS_REL,
            " 는 정확히 키 '",
            FULL_BENCH,
            "' 하나여야 한다(지금: ",
            shown,
            ")"
        )));
    }
    let folder = item(&document, FULL_BENCH)?;
    let valid_name = matches!(folder.peel(), PyValue::Str(name) if is_folder_name(name));
    if !valid_name {
        return Err(preset_error(cat!(
            BINDINGS_REL,
            " 의 ",
            FULL_BENCH,
            " 값은 도구 폴더 이름이어야 한다: ",
            folder.repr()
        )));
    }
    let PyValue::Str(folder_name) = folder.peel() else {
        unreachable!("checked above")
    };
    let tool_dir = pyfs::join_text(&pyfs::join(&skill, "config"), folder_name);
    if !pyfs::is_dir(&tool_dir)? {
        return Err(preset_error(cat!(
            BINDINGS_REL,
            " 가 가리키는 도구 폴더 config/",
            folder_name,
            "/ 가 없다"
        )));
    }
    Ok(document)
}

/// `Path.suffix` of a normalised path text.
fn suffix_of(path: &PyStr) -> PyStr {
    let points = path.code_points();
    let name_start = points
        .iter()
        .rposition(|&p| p == u32::from('/'))
        .map_or(0, |at| at + 1);
    let name = &points[name_start..];
    match name.iter().rposition(|&p| p == u32::from('.')) {
        Some(at) if at > 0 && at < name.len() - 1 => PyStr::from_code_points(name[at..].to_vec()),
        _ => PyStr::default(),
    }
}

/// The names `Path.glob("*.yaml")` yields in a folder, as stems (an unreadable folder yields none).
fn yaml_stems(folder: &PyStr) -> Vec<PyStr> {
    let Ok(names) = pyfs::list_dir(folder) else {
        return Vec::new();
    };
    names
        .into_iter()
        .filter(|name| {
            let points = name.code_points();
            let tail: Vec<u32> = ".yaml".chars().map(u32::from).collect();
            points.ends_with(&tail)
        })
        .map(|name| {
            let suffix = suffix_of(&name);
            let points = name.code_points();
            PyStr::from_code_points(points[..points.len() - suffix.len()].to_vec())
        })
        .collect()
}

/// `preset_path(name, skill_dir, bindings)`: a preset by name (under the bound tool folder) or by path. No name is
/// the default preset. `bindings` falsy means they are loaded here.
pub fn preset_path(
    name: &PyValue,
    skill_dir: &SkillDir,
    bindings: &PyValue,
    depth: CallDepth,
) -> Result<PyStr, Failure> {
    let bindings = if bindings.is_truthy() {
        bindings.clone()
    } else {
        load_bindings(skill_dir, depth)?
    };
    let skill = pyfs::normalize(skill_dir.get()?);
    let bound = item(&bindings, FULL_BENCH)?;
    let PyValue::Str(bound_name) = bound.peel() else {
        return Err(type_error(&format!(
            "unsupported operand type(s) for /: 'PosixPath' and '{}'",
            bound.type_name()
        )));
    };
    let folder = pyfs::join_text(&pyfs::join(&skill, "config"), bound_name);
    let name = if name.is_truthy() {
        name.clone()
    } else {
        PyValue::str(DEFAULT_PRESET)
    };
    let candidate = path_of(&name)?;
    let shown = name.display_text();
    let has_slash = shown.code_points().contains(&u32::from('/'));
    let suffix = suffix_of(&candidate);
    if has_slash || suffix.equals(".yaml") || suffix.equals(".yml") {
        let absolute = if candidate.code_points().first() == Some(&u32::from('/')) {
            candidate
        } else {
            pyfs::join_text(&current_dir(), &candidate)
        };
        if !pyfs::is_file(&absolute)? {
            return Err(preset_error(cat!(
                "프리셋 파일이 없다: ",
                name.display_text()
            )));
        }
        return Ok(resolve_text(&absolute));
    }
    if !is_preset_name(&shown) {
        return Err(preset_error(cat!(
            "프리셋 이름은 [A-Za-z0-9._-] 만 쓴다: ",
            name.repr()
        )));
    }
    let file = pyfs::join_text(&folder, &cat!(&shown, ".yaml"));
    if !pyfs::is_file(&file)? {
        let mut stems = yaml_stems(&folder);
        let keys: Vec<PyValue> = stems
            .iter()
            .map(|stem| PyValue::Str(stem.clone()))
            .collect();
        stems = sorted_by_key(stems, &keys)?;
        let available = PyValue::List(stems.into_iter().map(PyValue::Str).collect());
        return Err(preset_error(cat!(
            "프리셋 '",
            shown,
            "' 이 config/",
            bound_name,
            "/ 에 없다(있는 것: ",
            available.repr(),
            ")"
        )));
    }
    Ok(file)
}

/// What [`load_preset`] returns: the path read, the validated document and the bindings.
pub struct LoadedPreset {
    pub path: PyStr,
    pub document: PyValue,
    pub bindings: PyValue,
}

/// `load_preset(name, skill_dir)`
pub fn load_preset(
    name: &PyValue,
    skill_dir: &SkillDir,
    depth: CallDepth,
) -> Result<LoadedPreset, Failure> {
    let bindings = load_bindings(skill_dir, depth)?;
    let path = preset_path(name, skill_dir, &bindings, depth)?;
    let document = load_yaml(&path, depth)?;
    let problems = validate_preset(&document, &bindings)?;
    if !problems.is_empty() {
        let mut joined = PyStr::default();
        for (index, problem) in problems.iter().enumerate() {
            if index > 0 {
                joined.push_str("\n  - ");
            }
            joined.push_py(problem);
        }
        return Err(preset_error(cat!(
            "프리셋 ",
            path,
            " 계약 위반 ",
            problems.len().to_string(),
            "건:\n  - ",
            joined
        )));
    }
    Ok(LoadedPreset {
        path,
        document,
        bindings,
    })
}

// ── dataset knobs ───────────────────────────────────────────────────────────────

fn is_hex(text: &PyStr, length: usize) -> bool {
    let points = text.code_points();
    points.len() == length
        && points.iter().all(|&p| {
            (u32::from('0')..=u32::from('9')).contains(&p)
                || (u32::from('a')..=u32::from('f')).contains(&p)
        })
}

fn is_column_name(text: &PyStr) -> bool {
    let points = text.code_points();
    !points.is_empty()
        && points.iter().all(|&p| {
            (u32::from('a')..=u32::from('z')).contains(&p)
                || (u32::from('A')..=u32::from('Z')).contains(&p)
                || (u32::from('0')..=u32::from('9')).contains(&p)
                || p == u32::from('_')
                || p == u32::from('.')
                || p == u32::from('-')
        })
}

/// `_dataset_shape_problem(name, value)`: the form of one dataset knob value (existence is checked at measurement).
/// `None` = fine.
pub fn dataset_shape_problem(name: &str, value: &PyValue) -> Result<Option<PyStr>, Failure> {
    if matches!(value.peel(), PyValue::None) {
        return Ok(None);
    }
    let kind = knob_spec(name).ok_or_else(|| key_error(name))?.kind;
    let outside =
        |allowed: &[&str]| Some(cat!(value.repr(), " 는 ", names_repr(allowed), " 밖이다"));
    if name == "dataset_kind" && !text_contains(&DATASET_KINDS, value) {
        return Ok(outside(&DATASET_KINDS));
    }
    if name == "dataset_source" && !text_contains(&DATASET_SOURCES, value) {
        return Ok(outside(&DATASET_SOURCES));
    }
    if name == "preprocess_strategy" && !text_contains(&PREPROCESS_STRATEGIES, value) {
        return Ok(outside(&PREPROCESS_STRATEGIES));
    }
    let text = match value.peel() {
        PyValue::Str(text) => Some(text),
        _ => None,
    };
    if kind == "path" {
        let ok = text.is_some_and(|text| {
            text.code_points().first() == Some(&u32::from('/'))
                && !text
                    .code_points()
                    .split(|&p| p == u32::from('/'))
                    .any(|part| part == [u32::from('.'), u32::from('.')])
        });
        if !ok {
            return Ok(Some(cat!(value.repr(), " 는 '..' 없는 절대경로여야 한다")));
        }
    }
    if kind == "hex40" && !text.is_some_and(|text| is_hex(text, 40)) {
        return Ok(Some(cat!(
            value.repr(),
            " 는 40자 소문자 hex(HF 커밋 SHA)여야 한다"
        )));
    }
    if kind == "hex64" && !text.is_some_and(|text| is_hex(text, 64)) {
        return Ok(Some(cat!(
            value.repr(),
            " 는 64자 소문자 hex(sha256)여야 한다"
        )));
    }
    if kind == "text"
        && !text
            .is_some_and(|text| !text.is_blank() && !text.code_points().contains(&u32::from('\n')))
    {
        return Ok(Some(cat!(
            value.repr(),
            " 는 비어 있지 않은 한 줄 문자열이어야 한다"
        )));
    }
    if kind == "map" {
        let PyValue::Dict(map) = value.peel() else {
            return Ok(Some(map_shape(value)));
        };
        if !KeyContract::of(map, &COLUMN_MAP_KEYS)?.holds() {
            return Ok(Some(map_shape(value)));
        }
        let names_ok = map.iter().all(
            |(_, entry)| matches!(entry.peel(), PyValue::Str(column) if is_column_name(column)),
        );
        if !names_ok {
            return Ok(Some(cat!(
                value.repr(),
                " 의 열 이름은 [A-Za-z0-9_.-] 문자열이다"
            )));
        }
    }
    Ok(None)
}

fn map_shape(value: &PyValue) -> PyStr {
    cat!(
        value.repr(),
        " 는 정확히 키 ",
        names_repr(&COLUMN_MAP_KEYS),
        " 를 가진 객체여야 한다"
    )
}

/// `dataset_cross_problems(values)`: `synthetic` means every other dataset knob is null, anything else means none
/// is. `values` is read with `.get`, so a knob that is absent counts as null.
pub fn dataset_cross_problems(values: &PyValue) -> Result<Vec<PyStr>, Failure> {
    match values.peel() {
        PyValue::Dict(map) => Ok(cross_problems(&|name| field_of(map, name))),
        other => Err(Failure::new(
            "AttributeError",
            &format!("'{}' object has no attribute 'get'", other.type_name()),
        )),
    }
}

/// The cross rule over a reader of the dataset knob values.
fn cross_problems<'a>(get: &dyn Fn(&str) -> &'a PyValue) -> Vec<PyStr> {
    let kind = get("dataset_kind");
    let rest = &DATASET_KNOBS[1..];
    if is_str(kind, DATASET_SYNTHETIC) {
        let bad: Vec<&str> = rest
            .iter()
            .copied()
            .filter(|name| !matches!(get(name).peel(), PyValue::None))
            .collect();
        return if bad.is_empty() {
            Vec::new()
        } else {
            vec![cat!(
                "dataset_kind=synthetic 인데 ",
                names_repr(&bad),
                " 가 null 이 아니다 — 합성 입력에는 출처 값이 없다"
            )]
        };
    }
    let missing: Vec<&str> = rest
        .iter()
        .copied()
        .filter(|name| matches!(get(name).peel(), PyValue::None))
        .collect();
    if missing.is_empty() {
        Vec::new()
    } else {
        vec![cat!(
            "dataset_kind=",
            kind.repr(),
            " 인데 ",
            names_repr(&missing),
            " 가 비었다 — 실제 데이터셋은 출처 전부를 선언한다"
        )]
    }
}

/// `check_dataset_runtime(values)`: at measurement time, the source exists, is readable and has the declared
/// sha256. The record it returns is attached to the provenance fields.
///
/// The file is read in 1 MiB pieces and hashed as it is read; nothing is remembered between calls.
pub fn check_dataset_runtime(values: &PyValue) -> Result<PyValue, Failure> {
    let get = |name: &str| get_or_none(values, name);
    let record = |text: PyStr| {
        let mut map = PyDict::new();
        map.set("checked", PyValue::Str(text));
        PyValue::Dict(map)
    };
    if py_eq(&get("dataset_kind")?, &PyValue::str(DATASET_SYNTHETIC)) {
        return Ok(record(PyStr::new("not_applicable(synthetic)")));
    }
    if py_eq(&get("dataset_source")?, &PyValue::str("ephemeral")) {
        return Err(preset_error(PyStr::new(
            "dataset_source=ephemeral 은 이번 개정에서 경로가 없다 — 벤치 컨테이너는 HF_HUB_OFFLINE=1 이고 임시 다운로드 경로를 아직 만들지 않았다(managed|custom 으로 원본을 두고 다시)",
        )));
    }
    let path = path_of(&item(values, "dataset_path")?)?;
    if !pyfs::is_file(&path)? {
        return Err(preset_error(cat!(
            "dataset_path 가 없다(또는 파일이 아니다): ",
            path
        )));
    }
    let os = pyfs::os_string(&path).map_err(|failure| failure.into_failure())?;
    if rustix::fs::access(os.as_os_str(), rustix::fs::Access::READ_OK).is_err() {
        return Err(preset_error(cat!("dataset_path 를 읽을 수 없다: ", path)));
    }
    let mut file = std::fs::File::open(&os).map_err(|error| {
        Failure::new(
            crate::pyfs::os_error_class(&error),
            &crate::cli::os_error_text(&error, &path),
        )
    })?;
    let mut digest = Context::new(&SHA256);
    let mut chunk = vec![0u8; 1 << 20];
    loop {
        let filled = read_piece(&mut file, &mut chunk)?;
        if filled == 0 {
            break;
        }
        digest.update(&chunk[..filled]);
    }
    let measured = hex_lower(digest.finish().as_ref());
    let declared = item(values, "source_sha256")?;
    if !py_eq(&PyValue::str(&measured), &declared) {
        return Err(preset_error(cat!(
            "dataset_path 의 sha256 이 프리셋 선언과 다르다(선언 ",
            declared.display_text(),
            " · 실측 ",
            measured,
            ") — 다른 바이트로 재지 않는다"
        )));
    }
    let size = std::fs::metadata(&os)
        .map_err(|error| {
            Failure::new(
                crate::pyfs::os_error_class(&error),
                &crate::cli::os_error_text(&error, &path),
            )
        })?
        .len();
    Ok(record(cat!(
        "measured(sha256 일치 · ",
        size.to_string(),
        " bytes)"
    )))
}

/// `f.read(1 << 20)`: a full piece unless the file ends.
fn read_piece(file: &mut std::fs::File, buffer: &mut [u8]) -> Result<usize, Failure> {
    let mut filled = 0;
    while filled < buffer.len() {
        match file.read(&mut buffer[filled..]) {
            Ok(0) => break,
            Ok(count) => filled += count,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => {
                let detail = error.to_string();
                let message = detail.split(" (os error").next().unwrap_or(&detail);
                let text = match error.raw_os_error() {
                    Some(errno) => format!("[Errno {errno}] {message}"),
                    None => message.to_string(),
                };
                return Err(Failure::new(crate::pyfs::os_error_class(&error), &text));
            }
        }
    }
    Ok(filled)
}

fn hex_lower(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(char::from(DIGITS[usize::from(byte >> 4)]));
        text.push(char::from(DIGITS[usize::from(byte & 0x0F)]));
    }
    text
}

// ── validation ──────────────────────────────────────────────────────────────────

/// `validate_preset(doc, bindings)`: the violations of the closed schema (empty = pass). `bindings` is not read.
///
/// The document is only read: a value is looked at where it lies, and a message is built only for a violation.
pub fn validate_preset(doc: &PyValue, _bindings: &PyValue) -> Result<Vec<PyStr>, Failure> {
    let mut problems: Vec<PyStr> = Vec::new();
    let PyValue::Dict(top) = doc.peel() else {
        return Ok(vec![PyStr::new("프리셋 최상위가 객체가 아니다")]);
    };
    if !keys_exactly(top, &TOP_KEYS) {
        let top_contract = KeyContract::of(top, &TOP_KEYS)?;
        problems.push(cat!(
            "최상위 키 계약 위반(누락 ",
            top_contract.missing_repr(),
            " · 미지 ",
            top_contract.extra_repr(),
            ") — 값만 조정한다. 꺼 둔 블록·다른 도구 블록을 실제 키로 남기지 않는다"
        ));
    }
    let version = field_of(top, "schema_version");
    if !is_small_int(version, SCHEMA_VERSION) {
        problems.push(cat!(
            "schema_version 은 ",
            SCHEMA_VERSION.to_string(),
            " 이어야 한다(지금 ",
            version.repr(),
            ")"
        ));
    }
    let bench = field_of(top, "bench");
    if !is_str(bench, FULL_BENCH) {
        problems.push(cat!(
            "bench 는 바인딩 키 '",
            FULL_BENCH,
            "' 이어야 한다(지금 ",
            bench.repr(),
            ")"
        ));
    }
    let tool = field_of(top, "tool");
    if !text_contains(&KNOWN_TOOLS, tool) {
        problems.push(cat!(
            "tool 은 ",
            names_repr(&KNOWN_TOOLS),
            " 중 하나다(지금 ",
            tool.repr(),
            ")"
        ));
    }
    let empty = PyDict::new();
    let reasons_map: &PyDict = match field_of(top, "reason_codes").peel() {
        PyValue::Dict(map) if !map.is_empty() => map,
        _ => {
            problems.push(PyStr::new("reason_codes 는 비어 있지 않은 객체여야 한다"));
            &empty
        }
    };
    for (other, code, description) in reasons_map.keyed_entries() {
        let code_ok = other.is_none() && is_reason_code(code);
        if !code_ok {
            let shown = match other {
                Some(other) => other.repr.clone(),
                None => code.repr(),
            };
            problems.push(cat!(
                "근거 코드 이름 ",
                shown,
                " 는 [a-z][a-z0-9_]* 여야 한다"
            ));
        }
        let description_ok = matches!(description.peel(), PyValue::Str(text) if !text.is_blank());
        if !description_ok {
            let shown = match other {
                Some(other) => other.repr.clone(),
                None => code.repr(),
            };
            problems.push(cat!("근거 코드 ", shown, " 의 설명이 비었다"));
        }
    }
    for need in [HUMAN_PIN_REASON, DEFAULT_REASON] {
        if reasons_map.get(need).is_none() {
            problems.push(cat!(
                "reason_codes 에 필수 코드 '",
                need,
                "' 가 없다(해소기가 사람 핀·기본값에 붙인다)"
            ));
        }
    }
    let PyValue::Dict(knobs) = field_of(top, "knobs").peel() else {
        problems.push(PyStr::new("knobs 는 객체여야 한다"));
        return Ok(problems);
    };
    let spec_names: Vec<&'static str> = KNOB_SPECS.iter().map(|spec| spec.name).collect();
    if !keys_exactly(knobs, &spec_names) {
        let knob_contract = KeyContract::of(knobs, &spec_names)?;
        problems.push(cat!(
            "knobs 키 계약 위반(누락 ",
            knob_contract.missing_repr(),
            " · 미지 ",
            knob_contract.extra_repr(),
            ")"
        ));
    }
    for (position, (other, name_text, knob_value)) in knobs.keyed_entries().enumerate() {
        if other.is_some() {
            continue;
        }
        // the table is usually in the document's order: try the spec at the same position first
        let spec = KNOB_SPECS
            .get(position)
            .filter(|spec| name_text.equals(spec.name))
            .or_else(|| KNOB_SPECS.iter().find(|spec| name_text.equals(spec.name)));
        let Some(spec) = spec else {
            continue;
        };
        validate_knob(spec, knob_value, reasons_map, &mut problems)?;
    }
    let all_datasets = DATASET_KNOBS
        .iter()
        .all(|name| matches!(knobs.get(name).map(PyValue::peel), Some(PyValue::Dict(_))));
    if all_datasets {
        let default_of = |name: &str| match knobs.get(name).map(PyValue::peel) {
            Some(PyValue::Dict(knob)) => field_of(knob, "default"),
            _ => NONE,
        };
        for problem in cross_problems(&default_of) {
            problems.push(cat!("knobs 기본값 ", problem));
        }
    }
    Ok(problems)
}

/// `re.fullmatch(r"[a-z][a-z0-9_]*", code)`
fn is_reason_code(code: &PyStr) -> bool {
    let points = code.code_points();
    match points.split_first() {
        Some((&first, rest)) => {
            (u32::from('a')..=u32::from('z')).contains(&first)
                && rest.iter().all(|&p| {
                    (u32::from('a')..=u32::from('z')).contains(&p)
                        || (u32::from('0')..=u32::from('9')).contains(&p)
                        || p == u32::from('_')
                })
        }
        None => false,
    }
}

fn validate_knob(
    spec: &KnobSpec,
    knob_doc: &PyValue,
    reason_codes: &PyDict,
    problems: &mut Vec<PyStr>,
) -> Result<(), Failure> {
    let name = spec.name;
    let where_ = || format!("knobs.{name}");
    let PyValue::Dict(knob_map) = knob_doc.peel() else {
        problems.push(cat!(where_(), " 는 객체여야 한다"));
        return Ok(());
    };
    if !keys_exactly(knob_map, &KNOB_KEYS) {
        let contract = KeyContract::of(knob_map, &KNOB_KEYS)?;
        problems.push(cat!(
            where_(),
            " 키 계약 위반(누락 ",
            contract.missing_repr(),
            " · 미지 ",
            contract.extra_repr(),
            ")"
        ));
        return Ok(());
    }
    let owner = field_of(knob_map, "owner");
    if !text_contains(&OWNERS, owner) {
        problems.push(cat!(
            where_(),
            ".owner 는 ",
            names_repr(&OWNERS),
            " 중 하나다"
        ));
    }
    let kind = field_of(knob_map, "kind");
    if !is_str(kind, spec.kind) {
        problems.push(cat!(
            where_(),
            ".kind 는 ",
            spec.kind,
            " 여야 한다(지금 ",
            kind.repr(),
            ")"
        ));
    }
    let serving_cap = field_of(knob_map, "serving_cap");
    if !text_contains(spec.caps, serving_cap) {
        problems.push(cat!(
            where_(),
            ".serving_cap 은 ",
            names_repr(spec.caps),
            " 중 하나다(지금 ",
            serving_cap.repr(),
            ")"
        ));
    }
    let mut grade_of = GradeMap::default();
    let option_list: &[PyValue] = match field_of(knob_map, "options").peel() {
        PyValue::List(list) if !list.is_empty() => list,
        _ => {
            problems.push(cat!(where_(), ".options 는 비어 있지 않은 목록이어야 한다"));
            &[]
        }
    };
    for (index, option) in option_list.iter().enumerate() {
        let option_map = match option.peel() {
            PyValue::Dict(map) if keys_exactly(map, &OPTION_KEYS) => map,
            PyValue::Dict(map) => {
                // the contract check of the original sorts the stray keys, which can fail
                KeyContract::of(map, &OPTION_KEYS)?;
                problems.push(cat!(
                    where_(),
                    ".options[",
                    index.to_string(),
                    "] 는 정확히 {value, grade} 여야 한다"
                ));
                continue;
            }
            _ => {
                problems.push(cat!(
                    where_(),
                    ".options[",
                    index.to_string(),
                    "] 는 정확히 {value, grade} 여야 한다"
                ));
                continue;
            }
        };
        let value = field_of(option_map, "value");
        let grade = field_of(option_map, "grade");
        let nullable = is_dataset_knob(name) && matches!(value.peel(), PyValue::None);
        if !nullable && !spec.value_type.accepts(value) {
            problems.push(cat!(
                where_(),
                ".options[",
                index.to_string(),
                "].value=",
                value.repr(),
                " 의 타입이 ",
                spec.value_type.name(),
                " 가 아니다"
            ));
            continue;
        }
        if is_dataset_knob(name) {
            if let Some(bad) = dataset_shape_problem(name, value)? {
                problems.push(cat!(
                    where_(),
                    ".options[",
                    index.to_string(),
                    "].value ",
                    bad
                ));
                continue;
            }
        }
        let value = norm_cow(value)?;
        if grade_of.contains(&value)? {
            problems.push(cat!(
                where_(),
                ".options 에 값 ",
                value.repr(),
                " 가 중복된다"
            ));
        }
        if !text_contains(&GRADES, grade) {
            problems.push(cat!(
                where_(),
                ".options[",
                index.to_string(),
                "].grade 는 ",
                names_repr(&GRADES),
                " 중 하나다"
            ));
        }
        grade_of.set(value, Cow::Borrowed(grade))?;
    }
    let reasons: &[PyValue] = match field_of(knob_map, "reasons").peel() {
        PyValue::List(list) if !list.is_empty() => list,
        _ => {
            problems.push(cat!(where_(), ".reasons 는 비어 있지 않은 목록이어야 한다"));
            &[]
        }
    };
    for reason in reasons {
        let defined = match reason.peel() {
            PyValue::Str(text) => reason_codes
                .keyed_entries()
                .any(|(other, key, _)| other.is_none() && key == text),
            _ => dict_has_key(reason_codes, reason)?,
        };
        if !defined {
            problems.push(cat!(
                where_(),
                ".reasons 의 ",
                reason.repr(),
                " 가 reason_codes 에 정의돼 있지 않다(정의 자리는 하나)"
            ));
        }
    }
    let has_reason = |wanted: &str| reasons.iter().any(|reason| is_str(reason, wanted));
    if !has_reason(HUMAN_PIN_REASON) {
        problems.push(cat!(
            where_(),
            ".reasons 에 '",
            HUMAN_PIN_REASON,
            "' 가 없다 — 사람 핀은 어느 knob 에나 열려 있다"
        ));
    }
    let default = field_of(knob_map, "default");
    if spec.no_default {
        if !matches!(default.peel(), PyValue::None) {
            problems.push(cat!(
                where_(),
                ".default 는 null 이어야 한다(기본값 없음 결정 유지 — 값이 없으면 기동 거부)"
            ));
        }
    } else {
        let is_list_kind = spec.kind == "list";
        let raw_defaults: &[PyValue] = if is_list_kind {
            match default.peel() {
                PyValue::List(list) if !list.is_empty() => list,
                _ => {
                    problems.push(cat!(where_(), ".default 는 비어 있지 않은 목록이어야 한다"));
                    &[]
                }
            }
        } else {
            std::slice::from_ref(default)
        };
        if !has_reason(DEFAULT_REASON) {
            problems.push(cat!(
                where_(),
                ".reasons 에 '",
                DEFAULT_REASON,
                "' 가 없다(기본값이 있는 knob)"
            ));
        }
        let default_values: Vec<Cow<'_, PyValue>> = raw_defaults
            .iter()
            .map(norm_cow)
            .collect::<Result<_, Failure>>()?;
        for value in &default_values {
            match grade_of.get(value)? {
                None => problems.push(cat!(
                    where_(),
                    ".default 의 ",
                    value.repr(),
                    " 가 options 밖이다"
                )),
                Some(grade) if !is_str(grade, "auto") => problems.push(cat!(
                    where_(),
                    ".default 의 ",
                    value.repr(),
                    " 는 auto 여야 한다(기본값은 누구나 열 수 있어야 한다)"
                )),
                Some(_) => {}
            }
        }
        if name == "levels"
            && !default_values
                .iter()
                .any(|value| is_small_int(value, VERDICT_LEVEL))
        {
            problems.push(cat!(
                where_(),
                ".default 는 판정점 ",
                VERDICT_LEVEL.to_string(),
                " 을 포함해야 한다"
            ));
        }
        if is_str(owner, "user") {
            for (value, grade) in grade_of.iter() {
                let is_default = default_values.iter().any(|own| same_or_eq(own, value));
                if !is_default && !is_str(grade, "escalate") {
                    problems.push(cat!(
                        where_(),
                        " 는 owner: user 인데 비기본 값 ",
                        value.repr(),
                        " 가 ",
                        grade.display_text(),
                        " 다 — 사용자 knob 의 비기본 값은 escalate(사람 핀으로만)여야 한다"
                    ));
                }
            }
        }
    }
    if name == "levels"
        && !grade_of
            .iter()
            .any(|(value, _)| is_small_int(value, VERDICT_LEVEL))
    {
        problems.push(cat!(
            where_(),
            ".options 에 판정점 ",
            VERDICT_LEVEL.to_string(),
            " 이 없다"
        ));
    }
    if name == "repeats" {
        for (value, _) in grade_of.iter() {
            if let PyValue::Int(number) = value.peel() {
                use num_traits::ToPrimitive;
                let below = match number.to_i64() {
                    Some(own) => own < i64::from(FULL_REPEATS_MIN),
                    None => number.sign() == num_bigint::Sign::Minus,
                };
                if below {
                    problems.push(cat!(
                        where_(),
                        ".options 의 ",
                        value.repr(),
                        " 는 full 정의 하한 ",
                        FULL_REPEATS_MIN.to_string(),
                        " 미만이다(repeat_axis.FULL_REPEATS_MIN 교차검증)"
                    ));
                }
            }
        }
    }
    if name == "backend" && !is_str(owner, "agent") {
        problems.push(cat!(where_(), ".owner 는 agent 다(모델 계열에서 파생)"));
    }
    if is_dataset_knob(name) && !is_str(owner, "user") {
        problems.push(cat!(
            where_(),
            ".owner 는 user 다(데이터셋은 측정 축 — 프리셋 작성자가 정한다)"
        ));
    }
    Ok(())
}

// ── serving caps ────────────────────────────────────────────────────────────────

/// `_grep_yaml_int(text, key)`: the value of a `key: value` line (a `# comment` cut off, quotes stripped), an
/// integer when it is all digits, else the text; `None` when no such line. The dash spelling of the key is tried
/// first, then the underscore one.
pub fn grep_yaml_int(text: &PyStr, key: &str) -> Result<PyValue, Failure> {
    let underscored = key.replace('-', "_");
    for spelled in [key, underscored.as_str()] {
        if let Some(raw) = find_key_value(text.code_points(), spelled) {
            let stripped = PyStr::from_code_points(raw).strip();
            let trimmed = strip_char(&strip_char(&stripped, u32::from('"')), u32::from('\''));
            let digits = !trimmed.is_empty()
                && trimmed
                    .code_points()
                    .iter()
                    .all(|&p| (u32::from('0')..=u32::from('9')).contains(&p));
            return Ok(if digits {
                PyValue::Int(crate::pynumber::int_from_str(&trimmed).map_err(Failure::from)?)
            } else {
                PyValue::Str(trimmed)
            });
        }
    }
    Ok(PyValue::None)
}

fn strip_char(text: &PyStr, wanted: u32) -> PyStr {
    let points = text.code_points();
    let start = points
        .iter()
        .position(|&p| p != wanted)
        .unwrap_or(points.len());
    let end = points
        .iter()
        .rposition(|&p| p != wanted)
        .map_or(start, |last| last + 1);
    PyStr::from_code_points(points[start..end].to_vec())
}

/// `re.search(r"(?m)^\s*KEY\s*:\s*([^\n#]+)", text)` group 1 with the regex engine's backtracking: the first line
/// start (the text start, or after a `\n`) where the pattern matches, `\s*` taking whitespace greedily and giving it
/// back one character at a time when the value group cannot start.
fn find_key_value(text: &[u32], key: &str) -> Option<Vec<u32>> {
    let literal: Vec<u32> = key.chars().map(u32::from).collect();
    let newline = u32::from('\n');
    let hash = u32::from('#');
    let mut starts = vec![0usize];
    starts.extend(
        text.iter()
            .enumerate()
            .filter(|(_, &p)| p == newline)
            .map(|(at, _)| at + 1),
    );
    let skip_space = |from: usize| {
        let mut at = from;
        while at < text.len() && is_python_space(text[at]) {
            at += 1;
        }
        at
    };
    for start in starts {
        let key_at = skip_space(start);
        if !text[key_at..].starts_with(&literal) {
            continue;
        }
        let after_key = key_at + literal.len();
        let colon_at = skip_space(after_key);
        if text.get(colon_at) != Some(&u32::from(':')) {
            continue;
        }
        let first = colon_at + 1;
        let widest = skip_space(first);
        let mut begin = widest;
        loop {
            if let Some(&p) = text.get(begin) {
                if p != newline && p != hash {
                    let end = text[begin..]
                        .iter()
                        .position(|&q| q == newline || q == hash)
                        .map_or(text.len(), |offset| begin + offset);
                    return Some(text[begin..end].to_vec());
                }
            }
            if begin == first {
                break;
            }
            begin -= 1;
        }
    }
    None
}

/// `serving_caps(serving_yaml, dry_run)`: the limits the serving yaml declares (`max-num-seqs`, `max-model-len`).
/// No file is an error, except in a dry run, which records the absence.
pub fn serving_caps(serving_yaml: &PyValue, dry_run: &PyValue) -> Result<PyValue, Failure> {
    let missing = !serving_yaml.is_truthy() || !pyfs::is_file(&path_of(serving_yaml)?)?;
    if missing {
        if dry_run.is_truthy() {
            let mut map = PyDict::new();
            map.set("max_num_seqs", PyValue::None);
            map.set("max_model_len", PyValue::None);
            map.set(
                "source",
                PyValue::Str(cat!(
                    "unavailable(dry-run · 서빙 yaml 부재: ",
                    serving_yaml.display_text(),
                    ")"
                )),
            );
            return Ok(PyValue::Dict(map));
        }
        return Err(preset_error(cat!(
            "서빙 yaml 이 없다(",
            serving_yaml.display_text(),
            ") — 상한 필터 없이 측정하지 않는다(explorer 산출 configs/<config>.yaml)"
        )));
    }
    let path = path_of(serving_yaml)?;
    let bytes = pyfs::read_bytes(&path)?;
    let text = decode_replace(&bytes);
    let mut map = PyDict::new();
    map.set(
        "source",
        PyValue::Str(cat!("declared(", serving_yaml.display_text(), ")")),
    );
    for (key, dashed) in [
        ("max_num_seqs", "max-num-seqs"),
        ("max_model_len", "max-model-len"),
    ] {
        let value = grep_yaml_int(&text, dashed)?;
        if !matches!(value.peel(), PyValue::None) && !matches!(value.peel(), PyValue::Int(_)) {
            return Err(preset_error(cat!(
                "서빙 yaml ",
                serving_yaml.display_text(),
                " 의 ",
                dashed,
                "=",
                value.repr(),
                " 를 정수로 읽지 못했다"
            )));
        }
        map.set(key, value);
    }
    Ok(PyValue::Dict(map))
}

/// `bytes.decode("utf-8", errors="replace")` with universal newlines, as `Path.read_text` reads it.
fn decode_replace(bytes: &[u8]) -> PyStr {
    let decoded = String::from_utf8_lossy(bytes);
    let mut points: Vec<u32> = Vec::with_capacity(decoded.len());
    let mut chars = decoded.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            points.push(u32::from('\n'));
        } else {
            points.push(u32::from(c));
        }
    }
    PyStr::from_code_points(points)
}

// ── CLI value parsing ───────────────────────────────────────────────────────────

fn all_digits(text: &PyStr) -> bool {
    !text.is_empty()
        && text
            .code_points()
            .iter()
            .all(|&p| (u32::from('0')..=u32::from('9')).contains(&p))
}

/// `_parse_cli_value(name, raw)`: the text of a command-line value as the knob's type. Nothing (or only blanks) is
/// `None`.
pub fn parse_cli_value(name: &PyValue, raw: &PyValue) -> Result<PyValue, Failure> {
    let spec = match name.peel() {
        PyValue::Str(text) => KNOB_SPECS.iter().find(|spec| text.equals(spec.name)),
        _ => None,
    };
    let Some(spec) = spec else {
        check_hashable(name)?;
        return Err(Failure::with_message("KeyError", PyStr::new(&name.repr())));
    };
    // only a string can be blank (`str(value)` of anything else has characters)
    if matches!(raw.peel(), PyValue::None)
        || matches!(raw.peel(), PyValue::Str(text) if text.is_blank())
    {
        return Ok(PyValue::None);
    }
    let flag = || flag_of(spec.name);
    let text = raw.display_text();
    if spec.kind == "list" {
        let mut values = Vec::new();
        for token in split_on(&text, u32::from(',')) {
            let token = token.strip();
            if token.is_empty() {
                continue;
            }
            if !all_digits(&token) {
                return Err(preset_error(cat!(flag(), " 원소는 정수만: ", token.repr())));
            }
            values.push(PyValue::Int(
                crate::pynumber::int_from_str(&token).map_err(Failure::from)?,
            ));
        }
        return Ok(PyValue::List(values));
    }
    if spec.value_type == ValueType::Int {
        let stripped = text.strip();
        if !all_digits(&stripped) {
            return Err(preset_error(cat!(flag(), " 는 정수다: ", raw.repr())));
        }
        return Ok(PyValue::Int(
            crate::pynumber::int_from_str(&stripped).map_err(Failure::from)?,
        ));
    }
    if spec.value_type == ValueType::Dict {
        let refused = || preset_error(cat!(flag(), " 는 JSON 객체다: ", raw.repr()));
        return match loads_text_objects(&text) {
            Ok(value @ PyValue::Dict(_)) => Ok(value),
            Ok(_) => Err(refused()),
            Err(error) if error.is_value_error() => Err(refused()),
            Err(error) => {
                let text = error.to_string();
                Err(Failure::new(
                    "RecursionError",
                    text.strip_prefix("RecursionError: ").unwrap_or(&text),
                ))
            }
        };
    }
    Ok(PyValue::Str(text.strip()))
}

/// `left + right` for the numbers a resolution adds (`bool` counts as 0 or 1); other types are the `TypeError`.
fn add_numbers(left: &PyValue, right: &PyValue) -> Result<PyValue, Failure> {
    let integer = |value: &PyValue| match value.peel() {
        PyValue::Int(number) => Some(number.clone()),
        PyValue::Bool(flag) => Some(BigInt::from(u8::from(*flag))),
        _ => None,
    };
    if let (Some(a), Some(b)) = (integer(left), integer(right)) {
        return Ok(PyValue::Int(a + b));
    }
    let real = |value: &PyValue| match value.peel() {
        PyValue::Float(number) => Some(*number),
        other => integer(other).and_then(|number| crate::pynumber::float_from_int(&number).ok()),
    };
    match (real(left), real(right)) {
        (Some(a), Some(b)) => Ok(PyValue::Float(a + b)),
        _ => Err(type_error(&format!(
            "unsupported operand type(s) for +: '{}' and '{}'",
            left.type_name(),
            right.type_name()
        ))),
    }
}

/// `text.split(separator)` for one code point.
fn split_on(text: &PyStr, separator: u32) -> Vec<PyStr> {
    text.code_points()
        .split(|&p| p == separator)
        .map(|part| PyStr::from_code_points(part.to_vec()))
        .collect()
}

/// `str.split(sep, 1)`: the text before and after the first separator.
fn split_once(text: &PyStr, separator: u32) -> Option<(PyStr, PyStr)> {
    let points = text.code_points();
    let at = points.iter().position(|&p| p == separator)?;
    Some((
        PyStr::from_code_points(points[..at].to_vec()),
        PyStr::from_code_points(points[at + 1..].to_vec()),
    ))
}

/// The items of a repeated option as texts: `it in items or []`.
fn item_texts<'a>(items: &'a PyValue, option: &str) -> Result<Vec<&'a PyStr>, Failure> {
    let _ = option;
    match items.peel() {
        PyValue::None => Ok(Vec::new()),
        PyValue::List(list) | PyValue::Tuple(list) => list
            .iter()
            .map(|entry| match entry.peel() {
                PyValue::Str(text) => Ok(text),
                other => Err(type_error(&format!(
                    "argument of type '{}' is not iterable",
                    other.type_name()
                ))),
            })
            .collect(),
        other if !other.is_truthy() => Ok(Vec::new()),
        other => Err(type_error(&format!(
            "'{}' object is not iterable",
            other.type_name()
        ))),
    }
}

/// `_parse_knob_reasons(items)`: `KNOB=CODE` texts as a dict (the same knob twice must agree).
pub fn parse_knob_reasons(items: &PyValue) -> Result<PyDict, Failure> {
    let mut out = PyDict::new();
    for text in item_texts(items, "--knob-reason")? {
        let Some((knob, code)) = split_once(text, u32::from('=')) else {
            return Err(preset_error(cat!(
                "--knob-reason 은 KNOB=CODE 모양이다: ",
                text.repr()
            )));
        };
        let (knob, code) = (knob.strip(), code.strip());
        let known = knob
            .to_utf8()
            .as_deref()
            .is_some_and(|name| knob_spec(name).is_some());
        if !known {
            let names: Vec<&str> = sorted_knob_names();
            return Err(preset_error(cat!(
                "--knob-reason 의 knob ",
                knob.repr(),
                " 를 모른다(",
                names_repr(&names),
                ")"
            )));
        }
        let key = knob.to_utf8().unwrap_or_default();
        if let Some(existing) = out.get(&key) {
            if !py_eq(existing, &PyValue::Str(code.clone())) {
                return Err(preset_error(cat!(
                    "--knob-reason ",
                    knob,
                    " 가 두 번 다른 값으로 왔다"
                )));
            }
        }
        out.insert(knob, PyValue::Str(code));
    }
    Ok(out)
}

/// `sorted(KNOB_SPECS)`
fn sorted_knob_names() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = KNOB_SPECS.iter().map(|spec| spec.name).collect();
    names.sort_unstable();
    names
}

/// `_parse_pins(items)`: `KNOB=VALUE` texts as a dict (each knob once); the values stay raw text.
pub fn parse_pins(items: &PyValue) -> Result<PyDict, Failure> {
    let mut out = PyDict::new();
    for text in item_texts(items, "--pin")? {
        let Some((knob, raw)) = split_once(text, u32::from('=')) else {
            return Err(preset_error(cat!(
                "--pin 은 KNOB=VALUE 모양이다: ",
                text.repr()
            )));
        };
        let (knob, raw) = (knob.strip(), raw.strip());
        let known = knob
            .to_utf8()
            .as_deref()
            .is_some_and(|name| knob_spec(name).is_some());
        if !known {
            return Err(preset_error(cat!(
                "--pin 의 knob ",
                knob.repr(),
                " 를 모른다(",
                names_repr(&sorted_knob_names()),
                ")"
            )));
        }
        let key = knob.to_utf8().unwrap_or_default();
        if out.get(&key).is_some() {
            return Err(preset_error(cat!("--pin ", knob, " 가 두 번 왔다")));
        }
        out.insert(knob, PyValue::Str(raw));
    }
    Ok(out)
}

// ── resolve ─────────────────────────────────────────────────────────────────────

/// What `_campaign_pins` found: the campaign (if one was read), its `bench_pins` and why there are none.
pub struct CampaignPins {
    pub campaign: Option<PyValue>,
    pub pins: PyDict,
    pub note: PyStr,
}

/// `_campaign_pins(campaign_id, repo_root)`: the knob pins of the campaign declaration. A declaration that exists
/// and is damaged is the repeat axis's own refusal (`RepeatAxisError`, not a `PresetError`).
pub fn campaign_pins(
    campaign_id: &PyValue,
    repo_root: &RepoRoot,
    owner: &dyn CampaignOwner,
) -> Result<CampaignPins, Failure> {
    let found = load_campaign_budgets(campaign_id, repo_root, owner)?;
    let Some(budgets) = &found.budgets else {
        return Ok(CampaignPins {
            campaign: None,
            pins: PyDict::new(),
            note: found.absence,
        });
    };
    let campaign = found.campaign.clone().unwrap_or(PyValue::None);
    let pins = match budgets.get("bench_pins") {
        None => PyDict::new(),
        Some(value) => match value.peel() {
            PyValue::Dict(map) => map.clone(),
            _ => {
                return Err(preset_error(cat!(
                    "campaigns/",
                    campaign.display_text(),
                    "/campaign.yaml budgets.bench_pins 가 객체가 아니다"
                )))
            }
        },
    };
    for (other, key, value) in pins.keyed_entries() {
        let key_value = match other {
            Some(other) => rebuild_key(other),
            None => PyValue::Str(key.clone()),
        };
        if is_str(&key_value, "repeats") {
            return Err(preset_error(PyStr::new(
                "budgets.bench_pins.repeats 는 두 번째 자리다 — 반복 수 핀은 budgets.repeats 하나다",
            )));
        }
        let spec = match &key_value {
            PyValue::Str(text) => text.to_utf8().as_deref().and_then(knob_spec),
            _ => None,
        };
        let Some(spec) = spec else {
            return Err(preset_error(cat!(
                "budgets.bench_pins 의 ",
                key_value.repr(),
                " 를 모른다(",
                names_repr(&sorted_knob_names()),
                ")"
            )));
        };
        let well_formed = if spec.kind == "list" {
            matches!(value.peel(), PyValue::List(items)
                if !items.is_empty() && items.iter().all(|entry| ValueType::Int.accepts(entry)))
        } else {
            spec.value_type.accepts(value)
        };
        if !well_formed {
            return Err(preset_error(cat!(
                "budgets.bench_pins.",
                key_value.display_text(),
                "=",
                value.repr(),
                " 의 모양이 knob 과 다르다"
            )));
        }
    }
    Ok(CampaignPins {
        campaign: Some(campaign),
        pins,
        note: PyStr::default(),
    })
}

/// The arguments of [`resolve`], each as the caller gave it (`None` is "not given").
pub struct ResolveArguments<'a> {
    pub preset_doc: &'a PyValue,
    pub preset_name: &'a PyValue,
    pub tool: &'a PyValue,
    /// Sweep-flag values: the Agent's choices.
    pub cli: &'a PyValue,
    /// `--pin KNOB=VALUE`: human pins.
    pub cli_pins: &'a PyValue,
    pub pinned_by: &'a PyValue,
    pub knob_reasons: &'a PyValue,
    pub approved_by: &'a PyValue,
    pub campaign_id: &'a PyValue,
    pub serving: &'a PyValue,
    pub repo_root: &'a RepoRoot,
    pub repeats_source: &'a PyValue,
    pub dry_run: &'a PyValue,
}

/// The resolved values and the record block of one resolution.
pub struct Resolution {
    pub values: PyDict,
    pub block: PyDict,
}

fn dict_or_empty(value: &PyValue) -> Result<PyDict, Failure> {
    match value.peel() {
        PyValue::Dict(map) => Ok(map.clone()),
        other if !other.is_truthy() => Ok(PyDict::new()),
        other => Err(type_error(&format!(
            "'{}' object is not iterable",
            other.type_name()
        ))),
    }
}

fn dict_value(map: PyDict) -> PyValue {
    PyValue::Dict(map)
}

/// The record of one knob while it is being built; it stays open until the end because the `input_len` cap is
/// recorded after every knob is known.
struct KnobRecord {
    name: PyStr,
    entries: PyDict,
}

/// `resolve(preset_doc, preset_name, ...)`: the resolution order is CLI pin > campaign pin > Agent choice > preset
/// default. Returns the values and the record block; a violation is a `PresetError` (the start is refused).
pub fn resolve(
    arguments: &ResolveArguments<'_>,
    owner: &dyn CampaignOwner,
) -> Result<Resolution, Failure> {
    let ResolveArguments {
        preset_doc,
        preset_name,
        tool,
        cli,
        cli_pins,
        pinned_by,
        knob_reasons,
        approved_by,
        campaign_id,
        serving,
        repo_root,
        repeats_source,
        dry_run,
    } = arguments;
    if !text_contains(&KNOWN_TOOLS, tool) {
        return Err(preset_error(cat!(
            "--tool 은 ",
            names_repr(&KNOWN_TOOLS),
            " 중 하나다: ",
            tool.repr()
        )));
    }
    let knob_reasons = dict_or_empty(knob_reasons)?;
    let cli_pins = dict_or_empty(cli_pins)?;
    let pinned_text_present = pinned_by.is_truthy() && !pinned_by.display_text().strip().is_empty();
    if !cli_pins.is_empty() && !pinned_text_present {
        return Err(preset_error(PyStr::new(
            "--pin 은 --pinned-by <사람 발화 전사> 와 함께만 쓴다(귀속 없는 핀은 Agent 선택이다)",
        )));
    }
    if pinned_by.is_truthy() && cli_pins.is_empty() {
        return Err(preset_error(PyStr::new(
            "--pinned-by 는 --pin KNOB=VALUE 와 함께 쓴다(무엇을 핀했는지 없다)",
        )));
    }
    let knobs_value = item(preset_doc, "knobs")?;
    let campaign = campaign_pins(campaign_id, repo_root, owner)?;
    let preset_label = preset_name.display_text();
    let mut values = PyDict::new();
    let mut records: Vec<KnobRecord> = Vec::new();
    let knob_entries: Vec<(PyStr, PyValue)> = match knobs_value.peel() {
        PyValue::Dict(map) => map.iter().cloned().collect(),
        other => {
            return Err(Failure::new(
                "AttributeError",
                &format!("'{}' object has no attribute 'items'", other.type_name()),
            ))
        }
    };
    for (name_text, spec) in &knob_entries {
        let name_value = PyValue::Str(name_text.clone());
        let name_utf8 = name_text.to_utf8().unwrap_or_default();
        let kind = item(spec, "kind")?;
        let is_list = is_str(&kind, "list");
        let mut grade_of = GradeMap::default();
        for option in list_items(&item(spec, "options")?)? {
            grade_of.set(
                Cow::Owned(norm(&item(&option, "value")?)?),
                Cow::Owned(item(&option, "grade")?),
            )?;
        }
        let cli_raw = get_or_none(cli, &name_utf8)?;
        let cli_value = parse_cli_value(&name_value, &cli_raw)?;
        let pin_given = cli_pins.get(&name_utf8).is_some();
        let pin_value = if pin_given {
            parse_cli_value(
                &name_value,
                &get_or_none(&dict_value(cli_pins.clone()), &name_utf8)?,
            )?
        } else {
            PyValue::None
        };
        let pin_present = !matches!(pin_value, PyValue::None);
        let cli_present = !matches!(cli_value, PyValue::None);
        if pin_present && cli_present && !py_eq(&cli_value, &pin_value) {
            return Err(preset_error(cat!(
                flag_of(&name_utf8),
                "=",
                cli_value.repr(),
                "(Agent) 와 --pin ",
                name_utf8.as_str(),
                "=",
                pin_value.repr(),
                "(사람)가 다르다 — 사람 핀을 조용히 덮지 않는다"
            )));
        }
        let mut rec = PyDict::new();
        rec.set("owner", item(spec, "owner")?);
        let campaign_pin = campaign.pins.get(&name_utf8).cloned();
        let campaign_pin_listed = campaign_pin.is_some();
        if name_utf8 == "bench_budget_mib"
            && !is_str(tool, "guidellm")
            && !cli_present
            && !pin_present
            && !campaign_pin_listed
        {
            rec.set("value", PyValue::None);
            rec.set("source", PyValue::str("not_applicable"));
            rec.set("grade", PyValue::None);
            rec.set("reason", PyValue::None);
            rec.set(
                "note",
                PyValue::Str(cat!(
                    "--tool ",
                    tool.display_text(),
                    " 는 벤치 컨테이너를 띄우지 않는다(예산은 guidellm 전용)"
                )),
            );
            records.push(KnobRecord {
                name: name_text.clone(),
                entries: rec,
            });
            values.insert(name_text.clone(), PyValue::None);
            continue;
        }
        let source: &str;
        let value: PyValue;
        if name_utf8 == "repeats" {
            let attributed = repeats_source.is_truthy() || pin_present;
            let repeats_value = if pin_present {
                pin_value.clone()
            } else {
                cli_value.clone()
            };
            let source_text = if repeats_source.is_truthy() {
                (*repeats_source).clone()
            } else if pin_present {
                PyValue::Str(cat!("pinned-by: ", pinned_by.display_text()))
            } else {
                PyValue::None
            };
            let request = ResolveRequest {
                argv_value: repeats_value,
                argv_source: source_text,
                campaign_id: (*campaign_id).clone(),
                preset_default: item(spec, "default")?,
                preset_source: (*preset_name).clone(),
            };
            let resolved = match resolve_repeats(&request, repo_root, owner) {
                Ok(resolved) => resolved,
                Err(failure) if failure.class == "RepeatAxisError" => {
                    return Err(preset_error(cat!("반복 수: ", failure.message)))
                }
                Err(failure) => return Err(failure),
            };
            let text = resolved.source.to_utf8().unwrap_or_default();
            source = if text.starts_with("declared(argv") {
                if attributed {
                    "cli_pin"
                } else {
                    "agent"
                }
            } else if text.starts_with("declared(campaigns") {
                "campaign"
            } else {
                "preset"
            };
            value = PyValue::Int(resolved.count.clone());
            rec.set("repeats_source", PyValue::Str(resolved.source));
        } else if pin_present {
            source = "cli_pin";
            value = pin_value.clone();
            if let Some(listed) = &campaign_pin {
                if !matches!(listed.peel(), PyValue::None) && !py_eq(listed, &pin_value) {
                    rec.set("overrode_campaign_pin", listed.clone());
                }
            }
        } else if let Some(listed) = campaign_pin
            .as_ref()
            .filter(|v| !matches!(v.peel(), PyValue::None))
        {
            if cli_present && !py_eq(&cli_value, listed) {
                return Err(preset_error(cat!(
                    flag_of(&name_utf8),
                    ": Agent 선택 ",
                    cli_value.repr(),
                    " 가 사람 핀(campaigns/",
                    campaign
                        .campaign
                        .as_ref()
                        .map_or(PyStr::new("None"), PyValue::display_text),
                    " budgets.bench_pins.",
                    name_utf8.as_str(),
                    "=",
                    listed.repr(),
                    ")과 다르다 — 조용히 덮어쓰지 않는다(사람 핀이면 --pinned-by 로 귀속을 밝혀라)"
                )));
            }
            source = "campaign";
            value = listed.clone();
        } else if cli_present {
            source = "agent";
            value = cli_value.clone();
        } else if !matches!(item(spec, "default")?.peel(), PyValue::None)
            || is_dataset_knob(&name_utf8)
        {
            source = "preset";
            value = item(spec, "default")?;
        } else {
            let choices: Vec<PyValue> = list_items(&item(spec, "options")?)?
                .iter()
                .map(|option| item(option, "value"))
                .collect::<Result<_, _>>()?;
            return Err(preset_error(cat!(
                flag_of(&name_utf8),
                " 는 필수다 — 프리셋 ",
                preset_label,
                " 에 기본값이 없다(기본값 없음 결정). 선택지: ",
                PyValue::List(choices).repr()
            )));
        }
        let mut vals: Vec<PyValue> = if is_list {
            list_items(&value)?
        } else {
            vec![value]
        };
        if name_utf8 == "levels" && !list_contains(&vals, &int_value(VERDICT_LEVEL)) {
            vals.insert(0, int_value(VERDICT_LEVEL));
            rec.set("verdict_point_added", PyValue::Bool(true));
        }
        if is_list {
            vals = sorted_unique(vals)?;
        }
        let option_values = |spec: &PyValue| -> Result<PyValue, Failure> {
            let mut list = Vec::new();
            for option in list_items(&item(spec, "options")?)? {
                list.push(item(&option, "value")?);
            }
            Ok(PyValue::List(list))
        };
        let mut outside = Vec::new();
        for entry in &vals {
            if !grade_of.contains(&norm(entry)?)? {
                outside.push(entry.clone());
            }
        }
        if !outside.is_empty() {
            return Err(preset_error(cat!(
                flag_of(&name_utf8),
                "=",
                PyValue::List(outside).repr(),
                " 가 프리셋 ",
                preset_label,
                " 의 선택지 밖이다(선택지 ",
                option_values(spec)?.repr(),
                ") — 선택지를 늘리려면 프리셋을 고친다(리뷰 대상)"
            )));
        }
        let mut grades = GradeMap::default();
        for entry in &vals {
            let normed = norm(entry)?;
            let grade = grade_of.get(&normed)?.cloned().unwrap_or(PyValue::None);
            grades.set(Cow::Owned(normed), Cow::Owned(grade))?;
        }
        let reason: PyValue;
        if source == "cli_pin" || source == "campaign" {
            reason = PyValue::str(HUMAN_PIN_REASON);
        } else if source == "preset" {
            reason = PyValue::str(DEFAULT_REASON);
        } else {
            let reasons_allowed = list_items(&item(spec, "reasons")?)?;
            let agent_reasons: Vec<PyValue> = reasons_allowed
                .iter()
                .filter(|code| !is_str(code, HUMAN_PIN_REASON))
                .cloned()
                .collect();
            let mut given = knob_reasons
                .get(&name_utf8)
                .cloned()
                .unwrap_or(PyValue::None);
            let default = item(spec, "default")?;
            let is_default = if matches!(default.peel(), PyValue::None) {
                false
            } else if is_list {
                let mut expected = vec![int_value(VERDICT_LEVEL)];
                expected.extend(list_items(&default)?);
                let expected = sorted_unique(expected)?;
                py_eq(&PyValue::List(vals.clone()), &PyValue::List(expected))
            } else {
                py_eq(&norm(&vals[0])?, &norm(&default)?)
            };
            if matches!(given.peel(), PyValue::None) {
                if !is_default {
                    let joined: Vec<PyStr> =
                        agent_reasons.iter().map(PyValue::display_text).collect();
                    let mut text = PyStr::default();
                    for (index, code) in joined.iter().enumerate() {
                        if index > 0 {
                            text.push_str("|");
                        }
                        text.push_py(code);
                    }
                    let shown = if is_list {
                        PyValue::List(vals.clone()).display_text()
                    } else {
                        vals[0].display_text()
                    };
                    return Err(preset_error(cat!(
                        flag_of(&name_utf8),
                        "=",
                        shown,
                        " 는 Agent 선택인데 근거 코드가 없다 — --knob-reason ",
                        name_utf8.as_str(),
                        "=<",
                        text,
                        ">"
                    )));
                }
                given = PyValue::str(DEFAULT_REASON);
            }
            if !list_contains(&reasons_allowed, &given) || is_str(&given, HUMAN_PIN_REASON) {
                return Err(preset_error(cat!(
                    "--knob-reason ",
                    name_utf8.as_str(),
                    "=",
                    given.display_text(),
                    " 는 이 knob 의 Agent 근거 코드가 아니다(허용 ",
                    PyValue::List(agent_reasons).repr(),
                    ")"
                )));
            }
            reason = given;
            let escalated: Vec<PyValue> = grades
                .iter()
                .filter(|(_, grade)| is_str(grade, "escalate"))
                .map(|(key, _)| key.clone())
                .collect();
            if !escalated.is_empty() {
                let owner_note = if is_str(&item(spec, "owner")?, "user") {
                    " (owner: user knob)"
                } else {
                    ""
                };
                return Err(preset_error(cat!(
                    flag_of(&name_utf8),
                    "=",
                    PyValue::List(escalated).display_text(),
                    " 는 escalate 등급이다 — 사람 핀(--pinned-by · campaign.yaml budgets.bench_pins)으로만 열린다",
                    owner_note
                )));
            }
            let confirm: Vec<PyValue> = grades
                .iter()
                .filter(|(_, grade)| is_str(grade, "confirm"))
                .map(|(key, _)| key.clone())
                .collect();
            if !confirm.is_empty() && !approved_by.is_truthy() {
                return Err(preset_error(cat!(
                    flag_of(&name_utf8),
                    "=",
                    PyValue::List(confirm).display_text(),
                    " 는 confirm 등급이다 — 셀별 사전 승인(사람 발화 전사 --approved-by)이 있을 때만 Agent 가 고른다"
                )));
            }
            if !confirm.is_empty() {
                rec.set("approved_by", (*approved_by).clone());
            }
        }
        // the serving cap filter
        let mut filtered: Vec<PyValue> = Vec::new();
        let cap = item(spec, "serving_cap")?;
        if is_str(&cap, CAP_LE_MAX_NUM_SEQS) {
            let limit = get_or_none(serving, "max_num_seqs")?;
            let source_text = get_or_none(serving, "source")?;
            if matches!(limit.peel(), PyValue::None) {
                rec.set(
                    "serving_cap_applied",
                    PyValue::Str(cat!(
                        "not_applied(서빙 yaml 에 max-num-seqs 선언 없음 · ",
                        source_text.display_text(),
                        ")"
                    )),
                );
            } else {
                let mut keep = Vec::new();
                for entry in &vals {
                    if compare(Compare::LessEqual, entry.peel(), limit.peel())
                        .map_err(Failure::from)?
                        || py_eq(entry, &int_value(VERDICT_LEVEL))
                    {
                        keep.push(entry.clone());
                    }
                }
                for entry in &vals {
                    if !list_contains(&keep, entry) {
                        let mut dropped = PyDict::new();
                        dropped.set("value", entry.clone());
                        dropped.set(
                            "reason",
                            PyValue::Str(cat!(
                                "> max-num-seqs ",
                                percent_d(&limit)?,
                                " (",
                                source_text.display_text(),
                                ")"
                            )),
                        );
                        filtered.push(dict_value(dropped));
                    }
                }
                vals = keep;
                rec.set(
                    "serving_cap_applied",
                    PyValue::Str(cat!("le_max_num_seqs(", percent_d(&limit)?, ")")),
                );
                if vals.is_empty() {
                    return Err(preset_error(cat!(
                        flag_of(&name_utf8),
                        ": 상한 필터 뒤 남은 레벨이 없다"
                    )));
                }
            }
        }
        rec.set(
            "value",
            if is_list {
                PyValue::List(vals.clone())
            } else {
                vals[0].clone()
            },
        );
        rec.set("source", PyValue::str(source));
        let grade_value = if is_list {
            let mut map = PyDict::new();
            for entry in &vals {
                let key = entry.display_text();
                let grade = grades.get(&norm(entry)?)?.cloned().unwrap_or(PyValue::None);
                map.insert(key, grade);
            }
            dict_value(map)
        } else {
            grades
                .get(&norm(&vals[0])?)?
                .cloned()
                .unwrap_or(PyValue::None)
        };
        rec.set("grade", grade_value);
        rec.set("reason", reason);
        if !filtered.is_empty() {
            rec.set("filtered_out", PyValue::List(filtered));
        }
        values.insert(
            name_text.clone(),
            if is_list {
                PyValue::List(vals)
            } else {
                vals.remove(0)
            },
        );
        records.push(KnobRecord {
            name: name_text.clone(),
            entries: rec,
        });
    }
    // input_len cap, once both lengths are known
    let input_len_spec = item(&knobs_value, "input_len")?;
    if is_str(
        &item(&input_len_spec, "serving_cap")?,
        CAP_INPUT_PLUS_OUTPUT,
    ) {
        let model_len = get_or_none(serving, "max_model_len")?;
        let source_text = get_or_none(serving, "source")?;
        let record = records
            .iter_mut()
            .find(|record| record.name.equals("input_len"))
            .ok_or_else(|| key_error("input_len"))?;
        if matches!(model_len.peel(), PyValue::None) {
            record.entries.set(
                "serving_cap_applied",
                PyValue::Str(cat!(
                    "not_applied(서빙 yaml 에 max-model-len 선언 없음 · ",
                    source_text.display_text(),
                    ")"
                )),
            );
        } else {
            let input_len = values.get("input_len").cloned().unwrap_or(PyValue::None);
            let output_len = values
                .get("output_len")
                .cloned()
                .ok_or_else(|| key_error("output_len"))?;
            let need = add_numbers(&input_len, &output_len)?;
            if compare(Compare::Greater, need.peel(), model_len.peel()).map_err(Failure::from)? {
                return Err(preset_error(cat!(
                    "--input-len ",
                    percent_d(&input_len)?,
                    " + --output-len ",
                    percent_d(&output_len)?,
                    " = ",
                    percent_d(&need)?,
                    " > max-model-len ",
                    percent_d(&model_len)?,
                    " (",
                    source_text.display_text(),
                    ") — 서빙 context window 밖은 측정하지 않는다"
                )));
            }
            record.entries.set(
                "serving_cap_applied",
                PyValue::Str(cat!(
                    "input_plus_output_le_max_model_len(",
                    percent_d(&model_len)?,
                    ")"
                )),
            );
        }
    }
    // the dataset axis: the cross rule, the tool pairing and the measurement-time check of the source
    let mut dataset = PyDict::new();
    for name in DATASET_KNOBS {
        let found = values.get(name).cloned().ok_or_else(|| key_error(name))?;
        dataset.set(name, found);
    }
    let dataset_value = dict_value(dataset.clone());
    let cross = dataset_cross_problems(&dataset_value)?;
    if !cross.is_empty() {
        let mut joined = PyStr::default();
        for (index, problem) in cross.iter().enumerate() {
            if index > 0 {
                joined.push_str("; ");
            }
            joined.push_py(problem);
        }
        return Err(preset_error(cat!("데이터셋 knob 해소값 ", joined)));
    }
    let dataset_kind = dataset
        .get("dataset_kind")
        .cloned()
        .unwrap_or(PyValue::None);
    if !is_str(tool, "guidellm") && !is_str(&dataset_kind, DATASET_SYNTHETIC) {
        return Err(preset_error(cat!(
            "--tool ",
            tool.display_text(),
            " 는 데이터셋 knob 을 받지 않는다(dataset_kind=",
            dataset_kind.repr(),
            ") — 실제 데이터셋 입력은 full-bench (--tool guidellm) 전용이다. 조용히 synthetic 으로 재지 않는다"
        )));
    }
    let runtime_check = if dry_run.is_truthy() {
        let mut map = PyDict::new();
        map.set(
            "checked",
            PyValue::str("skipped(dry-run · 측정 시점에 원본 실재·sha256 을 본다)"),
        );
        dict_value(map)
    } else {
        check_dataset_runtime(&dataset_value)?
    };
    let agent_chosen: HashSet<Vec<u32>> = records
        .iter()
        .filter(|record| matches!(record.entries.get("source"), Some(source) if is_str(source, "agent")))
        .map(|record| record.name.code_points().to_vec())
        .collect();
    let mut stray: Vec<PyValue> = knob_reasons
        .iter()
        .filter(|(name, _)| !agent_chosen.contains(name.code_points()))
        .map(|(name, _)| PyValue::Str(name.clone()))
        .collect();
    if !stray.is_empty() {
        let keys = stray.clone();
        stray = sorted_by_key(stray, &keys)?;
        return Err(preset_error(cat!(
            "--knob-reason ",
            PyValue::List(stray).repr(),
            " 의 knob 은 Agent 선택이 아니다(사람 핀·기본값 · 근거 코드가 붙지 않는다)"
        )));
    }
    let mut record_map = PyDict::new();
    for record in records {
        record_map.insert(record.name, dict_value(record.entries));
    }
    let mut dataset_record = dataset;
    dataset_record.set("runtime_check", runtime_check);
    let mut block = PyDict::new();
    block.set("bench", PyValue::str(FULL_BENCH));
    block.set("measured_tool", (*tool).clone());
    block.set("preset_tool", item(preset_doc, "tool")?);
    block.set("preset", (*preset_name).clone());
    block.set(
        "campaign",
        campaign.campaign.clone().unwrap_or(PyValue::None),
    );
    block.set(
        "campaign_pins_note",
        if campaign.note.is_empty() {
            PyValue::None
        } else {
            PyValue::Str(campaign.note.clone())
        },
    );
    block.set("pinned_by", (*pinned_by).clone());
    block.set("approved_by", (*approved_by).clone());
    block.set("serving_cap", (*serving).clone());
    block.set("knobs", dict_value(record_map));
    block.set("dataset", dict_value(dataset_record));
    block.set(
        "resolution_order",
        PyValue::str("cli_pin > campaign > agent > preset"),
    );
    Ok(Resolution { values, block })
}

/// `list(value)` for the sequences a document holds; any other type is not iterable.
fn list_items(value: &PyValue) -> Result<Vec<PyValue>, Failure> {
    match value.peel() {
        PyValue::List(items) | PyValue::Tuple(items) => Ok(items.clone()),
        PyValue::Str(text) => Ok(text
            .code_points()
            .iter()
            .map(|&point| PyValue::Str(PyStr::from_code_points(vec![point])))
            .collect()),
        PyValue::Dict(map) => Ok(dict_keys(map)),
        other => Err(type_error(&format!(
            "'{}' object is not iterable",
            other.type_name()
        ))),
    }
}

/// `sorted(dict.fromkeys(values))`: the distinct values (as a dict sees them), in order.
fn sorted_unique(values: Vec<PyValue>) -> Result<Vec<PyValue>, Failure> {
    let mut unique: Vec<PyValue> = Vec::new();
    for entry in values {
        check_hashable(&entry)?;
        if !unique.iter().any(|kept| same_or_eq(kept, &entry)) {
            unique.push(entry);
        }
    }
    let keys = unique.clone();
    sorted_by_key(unique, &keys)
}

/// `"%d" % value`: the integer text of a number (a `float` truncates, a `bool` is 0 or 1); anything else is the
/// `TypeError` Python raises.
fn percent_d(value: &PyValue) -> Result<PyStr, Failure> {
    match value.peel() {
        PyValue::Int(number) => Ok(PyStr::new(&number.to_string())),
        PyValue::Bool(flag) => Ok(PyStr::new(if *flag { "1" } else { "0" })),
        PyValue::Float(number) => {
            let integer = crate::pynumber::int_from_float(*number).map_err(Failure::from)?;
            Ok(PyStr::new(&integer.to_string()))
        }
        other => Err(type_error(&format!(
            "%d format: a real number is required, not {}",
            other.type_name()
        ))),
    }
}

/// The `KEY<US>VALUE` text of a resolved value for the shell lines (`,`-joined list, empty for `None`).
pub fn shell_text(value: &PyValue) -> PyStr {
    match value.peel() {
        PyValue::List(items) => {
            let mut text = PyStr::default();
            for (index, entry) in items.iter().enumerate() {
                if index > 0 {
                    text.push_str(",");
                }
                text.push_py(&entry.display_text());
            }
            text
        }
        PyValue::None => PyStr::default(),
        other => other.display_text(),
    }
}

/// The repeat kind the programs print (the warm re-run is the only implemented kind).
pub const REPEAT_KIND: &str = REPEAT_KIND_WARM;

/// Whether the integer text is zero (used by the self-test fixtures).
pub fn is_zero(number: &BigInt) -> bool {
    number.is_zero()
}
