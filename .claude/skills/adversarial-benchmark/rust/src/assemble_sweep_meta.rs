//! `assemble_sweep_meta`: the sweep index and its `meta` block, as the deployed `sweep_bench.sh` assembles them after a
//! sweep. This is the Rust owner of what used to be the Python program embedded in that script.
//!
//! The producer reads the serving files of one cell (config yaml, env file, node manifest, the engine log of the first
//! completed level and the per-level sidecars), decides every identity and provenance field once, and writes
//! `sweep_index.json`. Its contract is the original's:
//!
//! * The files are read with the original's naive expressions ([`crate::sweep_meta_scan`]); there is no YAML parser.
//! * Values measured by the run beat declared ones, and the declaration is kept beside them with a mismatch note.
//! * A reassembly (`REASSEMBLE=1`) inherits what the measurement made - its time, the measured image, the spec
//!   declaration, the repeat request, the measurement shape - and never reads Docker or the clock again.
//! * Naming (`gpu_key`), the repeat summary (`summarize`) and the lite figures (`build`, `read_lite_verdict`) are the
//!   crate's own owners, called directly; nothing here repeats their rules.
//! * The index is written with `json.dump(index, f, ensure_ascii=False, indent=2)` semantics: no final newline, UTF-8,
//!   the file is opened (and so emptied) before the text is produced, and text that cannot be encoded leaves the
//!   chunks that preceded it on disk.
//!
//! The environment is read through [`EnvSource`], and only the names the original reads (it never lists the
//! environment). `SDIR` is not among them: the original used it to import its Python helpers.

use crate::guidellm::Failure;
use crate::pydyn::{as_get_target, equals, or_none};
use crate::pyjson::{decode_file_text, loads_text};
use crate::pynumber::int_from_str;
use crate::pystr::PyStr;
use crate::pyvalue::{DumpError, PyDict, PyValue};
use crate::repeat_text::cat;
use crate::sweep_meta_scan as scan;
use num_bigint::BigInt;
use std::os::unix::ffi::OsStrExt;

/// The program's name in its messages.
pub const PROGRAM: &str = "assemble_sweep_meta";

/// The number the interpreter refuses to print or parse beyond (`sys.get_int_max_str_digits()`).
const MAX_INT_DIGITS: usize = 4300;

/// Where the producer reads its environment variables from.
pub trait EnvSource {
    /// `os.environ.get(name)`: the value (an empty one is still a value), or `None` when the name is not set.
    fn get(&self, name: &str) -> Option<PyStr>;
}

/// The environment variables the producer reads - the names the original program reads, in the order it first reads
/// them, and nothing else. The deployed boundary removes exactly these from an inherited environment before it sets
/// its own, and a check compares what a run actually asked for with this list.
pub const ENV_NAMES: [&str; 20] = [
    "CONFIG",
    "TOPO",
    "SWEEPDIR",
    "COMPLETED",
    "CFGYAML",
    "EF",
    "MANIFEST",
    "REASSEMBLE",
    "IMAGE_TAG_ACTUAL",
    "IMAGE_DIGEST_ACTUAL",
    "SERVE_PLANE",
    "VLLM_VER",
    "EASY_VLLM_VERSION",
    "LITE_RAW",
    "REPEATS",
    "REPEATS_SOURCE",
    "REPEAT_KIND",
    "CLAMP_LEVEL",
    "KNOB_RECORD",
    "ILEN",
];

/// `os.environ.get(name)`, for the names the producer is allowed to read.
fn var(env: &dyn EnvSource, name: &str) -> Option<PyStr> {
    debug_assert!(
        ENV_NAMES.contains(&name),
        "{name} is not a producer variable"
    );
    env.get(name)
}

/// The environment of this process.
pub struct ProcessEnv;

impl EnvSource for ProcessEnv {
    fn get(&self, name: &str) -> Option<PyStr> {
        std::env::var_os(name).map(|value| PyStr::from_surrogateescape(value.as_bytes()))
    }
}

/// A fixed set of variables (the self-test and the checks use it; nothing else is visible through it).
#[derive(Default)]
pub struct MapEnv {
    entries: Vec<(String, PyStr)>,
}

impl MapEnv {
    pub fn new() -> Self {
        Self::default()
    }

    /// The variables as `(name, value)` text, in the order they were set.
    pub fn pairs(&self) -> Vec<(&str, String)> {
        self.entries
            .iter()
            .map(|(name, value)| (name.as_str(), value.to_utf8().unwrap_or_default()))
            .collect()
    }

    pub fn with(mut self, name: &str, value: &str) -> Self {
        self.entries.retain(|(own, _)| own != name);
        self.entries.push((name.to_string(), PyStr::new(value)));
        self
    }
}

impl EnvSource for MapEnv {
    fn get(&self, name: &str) -> Option<PyStr> {
        self.entries
            .iter()
            .find(|(own, _)| own == name)
            .map(|(_, value)| value.clone())
    }
}

/// How a run ended.
#[derive(Debug, PartialEq)]
pub enum Ending {
    /// The index was written; the line to print on standard output (no newline).
    Done(PyStr),
    /// `sys.exit(status)`: nothing more is printed.
    Status(i32),
    /// `raise SystemExit("reason")`: the reason goes to standard error with a newline, the status is 1.
    Reason(PyStr),
    /// An exception nothing catches: the traceback and the status 1.
    Uncaught(Failure),
}

/// Everything a run produced: the standard-error text written before it ended, and how it ended.
#[derive(Debug, PartialEq)]
pub struct Produced {
    pub stderr: PyStr,
    pub ending: Ending,
}

/// What stops the run before the index line.
enum Stop {
    Status(i32),
    Reason(PyStr),
    Uncaught(Failure),
}

impl From<Failure> for Stop {
    fn from(failure: Failure) -> Self {
        Stop::Uncaught(failure)
    }
}

type Flow<T> = Result<T, Stop>;

fn text(value: &str) -> PyValue {
    PyValue::str(value)
}

fn key_error(name: &str) -> Failure {
    Failure::new("KeyError", &PyStr::new(name).repr())
}

fn required(env: &dyn EnvSource, name: &str) -> Flow<PyStr> {
    var(env, name).ok_or_else(|| key_error(name).into())
}

/// `os.environ.get(name) or None`: an unset and an empty variable are both nothing.
fn non_empty(env: &dyn EnvSource, name: &str) -> Option<PyStr> {
    var(env, name).filter(|value| !value.is_empty())
}

/// The `AttributeError` of `value.name` for a value that has no such attribute.
fn attribute_error(value: &PyValue, name: &str) -> Failure {
    Failure::new(
        "AttributeError",
        &format!(
            "'{}' object has no attribute '{name}'",
            value.peel().type_name()
        ),
    )
}

/// `value.get(key)` on a JSON value that should be an object.
fn get_of(value: &PyValue, key: &str) -> Result<PyValue, Failure> {
    Ok(or_none(as_get_target(value)?.get(key)))
}

/// `value.get(key) or default`.
fn get_or(value: &PyValue, key: &str, default: PyValue) -> Result<PyValue, Failure> {
    let found = get_of(value, key)?;
    Ok(if found.is_truthy() { found } else { default })
}

/// `isinstance(value, dict)`.
fn is_dict(value: &PyValue) -> bool {
    matches!(value.peel(), PyValue::Dict(_))
}

/// `read(path)`: the text of a file (invalid UTF-8 replaced, newlines universal), or nothing for any `OSError`.
fn read(path: &PyStr) -> Vec<char> {
    match crate::pyfs::read_bytes(path) {
        Ok(bytes) => crate::lite_engine_log::decode_log_text(&bytes),
        Err(_) => Vec::new(),
    }
}

/// A JSON file that may be absent or broken: `try: json.load(...) except (OSError, ValueError): ...`. Anything the
/// handler does not catch (a document nested beyond the recursion limit) is the failure.
fn load_json(path: &PyStr) -> Flow<Option<PyValue>> {
    Ok(crate::pyfs::load_json_or_none(path)?)
}

fn join(base: &PyStr, parts: &[&PyStr]) -> PyStr {
    crate::ospath::join(base, parts)
}

/// `"level_%02d" % level`.
fn level_dir(level: &BigInt) -> PyStr {
    let shown = level.to_string();
    let (sign, digits) = match shown.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", shown.as_str()),
    };
    let padding = 2usize.saturating_sub(sign.len() + digits.len());
    PyStr::new(&format!("level_{sign}{}{digits}", "0".repeat(padding)))
}

fn level_file(sweep_dir: &PyStr, level: &BigInt, name: &PyStr) -> PyStr {
    join(sweep_dir, &[&level_dir(level), name])
}

/// `str.isdigit()`.
pub(crate) fn is_digit_text(value: &PyStr) -> bool {
    !value.is_empty()
        && value.code_points().iter().all(|&point| {
            crate::pystr::is_decimal_digit(point)
                || crate::unicode_digit_tables::DIGIT_ONLY
                    .iter()
                    .any(|&(first, last)| (first..=last).contains(&point))
        })
}

fn int_failure(error: crate::pynumber::PyException) -> Failure {
    Failure::new(error.class, &error.message)
}

fn scan_failure(failure: scan::ScanFailure) -> Failure {
    Failure::new(failure.class, &failure.message)
}

fn pystr_of(value: &str) -> PyStr {
    PyStr::new(value)
}

/// `or "NA"` over an optional text that is nothing when absent or empty.
fn or_na(value: Option<String>) -> PyValue {
    match value {
        Some(own) if !own.is_empty() => text(&own),
        _ => text("NA"),
    }
}

/// The four fields of a value that is measured first: value, source, declaration, mismatch note.
struct Measured {
    value: String,
    source: &'static str,
    declared: String,
    mismatch: String,
}

/// `_norm_measured`: `None` is the measured default, so the token is `none`, never `NA`.
fn normalise_measured(value: &str) -> String {
    let stripped = PyStr::new(value).strip().to_utf8().unwrap_or_default();
    if stripped == "None" {
        "none".to_string()
    } else {
        crate::pylower::python_lower(&stripped)
    }
}

/// `_measured_first(meas, decl)`: a measurement wins and the declaration is kept beside it; a mismatch is only judged
/// when both are known.
fn measured_first(measured: Option<&str>, declared: Option<&str>) -> Measured {
    let measured = measured.map(normalise_measured);
    let declared = declared
        .map(|own| PyStr::new(own).strip().to_utf8().unwrap_or_default())
        .filter(|own| !own.is_empty());
    let (value, source) = match (&measured, &declared) {
        (Some(own), _) => (own.clone(), "measured(engine log)"),
        (None, Some(own)) => (own.clone(), "declared(config yaml)"),
        (None, None) => ("NA".to_string(), "absent(both)"),
    };
    let mismatch = match (&measured, &declared) {
        (Some(m), Some(d)) => {
            if *m == crate::pylower::python_lower(d) {
                "no".to_string()
            } else {
                format!("YES(measured={m} declared={d})")
            }
        }
        _ => "unknown".to_string(),
    };
    Measured {
        value,
        source,
        declared: declared.unwrap_or_else(|| "NA".to_string()),
        mismatch,
    }
}

fn append_measured(meta: &mut PyDict, name: &str, field: &Measured) {
    meta.set(name, text(&field.value));
    meta.set(&format!("{name}_source"), text(field.source));
    meta.set(&format!("{name}_declared"), text(&field.declared));
    meta.set(&format!("{name}_mismatch"), text(&field.mismatch));
}

/// `str(big)` for an integer the interpreter would print; beyond its digit limit it raises `ValueError`.
fn int_text(number: &BigInt) -> Option<String> {
    let shown = number.to_string();
    (shown.trim_start_matches('-').len() <= MAX_INT_DIGITS).then_some(shown)
}

/// The tensor-parallel size when the config does not declare one: from the manifest's per-node GPU count, with a
/// single-node topology counting one node (a `role: sub` entry of a single manifest is a control peer, not a worker).
fn derived_tensor_parallel(manifest: &[char], topology: &PyStr) -> String {
    let gpus = scan::grep_yaml(manifest, "gpus_per_node");
    let manifest_topology = scan::grep_yaml(manifest, "topology")
        .filter(|own| !own.is_empty())
        .map_or_else(|| topology.clone(), |own| PyStr::new(&own));
    let parse = |value: &str| int_from_str(&PyStr::new(value));
    let derived = || -> Option<String> {
        let shown = manifest_topology.strip();
        let points = shown.code_points();
        let unquoted = points
            .iter()
            .position(|&p| p != u32::from('"'))
            .map_or(&points[points.len()..], |first| &points[first..]);
        let single = unquoted.starts_with(&"single".chars().map(u32::from).collect::<Vec<_>>());
        if single {
            return match gpus.as_deref().filter(|own| !own.is_empty()) {
                Some(count) => int_text(&parse(count).ok()?.max(BigInt::from(1))),
                None => Some("1".to_string()),
            };
        }
        let nodes = scan::role_entries(manifest);
        match gpus.as_deref().filter(|own| !own.is_empty()) {
            Some(count) if nodes > 0 => {
                int_text(&(BigInt::from(nodes.max(1)) * parse(count).ok()?))
            }
            _ => Some("1".to_string()),
        }
    };
    derived().unwrap_or_else(|| "1".to_string())
}

/// The recorded fields of the first level (in level order) whose sidecar can be read.
fn first_readable(sweep_dir: &PyStr, levels: &[BigInt], name: &PyStr) -> Flow<Option<PyValue>> {
    for level in levels {
        if let Some(found) = load_json(&level_file(sweep_dir, level, name))? {
            return Ok(Some(found));
        }
    }
    Ok(None)
}

/// The measurement-tool block: the tool the parser saw, its version, the declared error budget.
fn tool_fields(measured: Option<&PyValue>) -> Flow<(PyValue, PyValue, PyValue, PyValue)> {
    let Some(sidecar) = measured else {
        return Ok((text("NA"), text("NA"), text("unavailable"), text("NA")));
    };
    let tool = get_or(sidecar, "bench_tool", text("vllm-bench-serve"))?;
    let version = get_or(sidecar, "bench_tool_version", text("NA"))?;
    let source = get_or(
        sidecar,
        "bench_tool_version_source",
        text("declared(도구가 버전을 자기보고하지 않는다)"),
    )?;
    let budget = get_of(sidecar, "max_error_rate_declared")?;
    let error_rate = if matches!(budget.peel(), PyValue::None) {
        text("NA")
    } else {
        budget
    };
    Ok((tool, version, source, error_rate))
}

/// `json.dumps(value, ensure_ascii=False, sort_keys=True)`.
fn sorted_dumps(value: &PyValue) -> Result<PyStr, Failure> {
    fn sorted(value: &PyValue) -> PyValue {
        match value.peel() {
            PyValue::Dict(map) => {
                let mut entries: Vec<&(PyStr, PyValue)> = map.iter().collect();
                entries.sort_by(|left, right| left.0.code_points().cmp(right.0.code_points()));
                let mut out = PyDict::with_capacity(entries.len());
                for (key, member) in entries {
                    out.insert(key.clone(), sorted(member));
                }
                PyValue::Dict(out)
            }
            PyValue::List(items) => PyValue::List(items.iter().map(sorted).collect()),
            PyValue::Tuple(items) => PyValue::Tuple(items.iter().map(sorted).collect()),
            other => other.clone(),
        }
    }
    sorted(value).dumps_unicode().map_err(dump_failure)
}

fn dump_failure(error: DumpError) -> Failure {
    let shown = error.to_string();
    Failure::new(
        error.python_type(),
        shown
            .strip_prefix(&format!("{}: ", error.python_type()))
            .unwrap_or(&shown),
    )
}

/// The dataset-provenance sidecars of every completed level: the first one that is not empty, whether any run sampled
/// with duplicates, and how many were read. Returns `None` when no sidecar exists.
struct DatasetScan {
    first: PyValue,
    duplicate: bool,
    runs: usize,
}

fn dataset_scan(sweep_dir: &PyStr, levels: &[BigInt], cfg: &PyStr) -> Flow<Option<DatasetScan>> {
    let file_name = cat!("dataset_", cfg, ".json");
    let mut first: Option<PyValue> = None;
    let mut duplicate = false;
    let mut runs = 0usize;
    for level in levels {
        let level_path = join(sweep_dir, &[&level_dir(level)]);
        let mut candidates = vec![join(&level_path, &[&file_name])];
        let mut run_files: Vec<PyStr> = if crate::ospath::isdir(&level_path) {
            crate::pyfs::list_dir(&level_path)?
                .into_iter()
                .filter(|name| {
                    name.code_points()
                        .starts_with(&"run_".chars().map(u32::from).collect::<Vec<_>>())
                })
                .map(|name| join(&level_path, &[&name, &file_name]))
                .collect()
        } else {
            Vec::new()
        };
        run_files.sort_by(|left, right| left.code_points().cmp(right.code_points()));
        candidates.extend(run_files);
        for path in candidates {
            let Some(document) = load_json(&path)? else {
                continue;
            };
            runs += 1;
            first = Some(match first.take() {
                Some(kept) if kept.is_truthy() => kept,
                _ => document.clone(),
            });
            if !duplicate {
                duplicate = get_of(&document, "duplicate_sampling")?.is_truthy();
            }
        }
    }
    Ok(first.map(|first| DatasetScan {
        first,
        duplicate,
        runs,
    }))
}

/// The lite block of the index: `lite_metrics` run on the lite leg's raw record, as the original does it. Any error
/// but `SystemExit` becomes a recorded failure instead of ending the run.
fn lite_block(env: &dyn EnvSource) -> Flow<PyValue> {
    let Some(raw_path) = non_empty(env, "LITE_RAW") else {
        return Ok(PyValue::None);
    };
    if !crate::ospath::isfile(&raw_path) {
        return Ok(PyValue::None);
    }
    match lite_attempt(&raw_path) {
        Ok(block) => Ok(block),
        Err(failure) if failure.class == "SystemExit" => Err(Stop::Reason(failure.message)),
        Err(failure) => {
            let mut map = PyDict::new();
            map.set("raw_json", PyValue::Str(raw_path));
            map.set(
                "error",
                PyValue::Str(cat!("lite_metrics 산정 실패: ", failure.message)),
            );
            Ok(PyValue::Dict(map))
        }
    }
}

fn lite_attempt(raw_path: &PyStr) -> Result<PyValue, Failure> {
    let bytes = crate::pyfs::read_bytes(raw_path)?;
    let document = decode_file_text(&bytes)
        .and_then(|decoded| loads_text(&decoded))
        .map_err(|error| crate::repeat_api_cli::load_failure(&error))?;
    let built = crate::lite_metrics::build(&document)?.to_value();
    let verdict = crate::lite_metrics::read_lite_verdict(&document)
        .map_or(PyValue::None, |own| PyValue::str(own.label()));
    let field = |name: &str| or_none(as_get_target(&built).ok().and_then(|map| map.get(name)));
    let mut block = PyDict::new();
    block.set("raw_json", PyValue::Str(raw_path.clone()));
    block.set("lite_verdict", verdict);
    for name in [
        "gen_tps",
        "gen_src",
        "cold_ttft_ms",
        "kv_gib",
        "capacity",
        "accept_len",
        "accept_len_src",
        "table",
        "engine",
    ] {
        block.set(name, field(name));
    }
    Ok(PyValue::Dict(block))
}

/// The repeat request of this assembly. A reassembly takes it from the measurement-time index; a fresh one reads the
/// sweep driver's variables, and a level whose first run broke leaves its boundary fact to be read from that run.
struct RepeatRequest {
    requested: PyValue,
    source: PyValue,
    kind: PyValue,
    clamp: PyValue,
}

fn repeat_request(
    env: &dyn EnvSource,
    reassemble: bool,
    prior: &PyValue,
    sweep_dir: &PyStr,
) -> Flow<RepeatRequest> {
    if reassemble {
        let recorded = get_of(prior, "repetition")?;
        let recorded = if is_dict(&recorded) {
            recorded
        } else {
            PyValue::Dict(PyDict::new())
        };
        return Ok(RepeatRequest {
            requested: get_of(&recorded, "requested")?,
            source: get_or(
                &recorded,
                "requested_source",
                text("absent(기존 index 에 반복 축 기록 없음 — 신설 전 조립)"),
            )?,
            kind: get_of(&recorded, "kind")?,
            clamp: get_of(&recorded, "clamp_run")?,
        });
    }
    let requested = match non_empty(env, "REPEATS").filter(is_digit_text) {
        Some(digits) => PyValue::Int(int_from_str(&digits).map_err(int_failure)?),
        None => PyValue::None,
    };
    let optional = |name: &str| non_empty(env, name).map_or(PyValue::None, PyValue::Str);
    let mut clamp = PyValue::None;
    if let Some(level) = non_empty(env, "CLAMP_LEVEL").filter(is_digit_text) {
        let number = int_from_str(&level).map_err(int_failure)?;
        let record = join(
            sweep_dir,
            &[
                &level_dir(&number),
                &PyStr::new(crate::repeat_axis::RUN_META_NAME),
            ],
        );
        clamp = match read_clamp_record(&record)? {
            Ok(document) => document,
            Err(reason) => {
                let mut map = PyDict::new();
                map.set("level", PyValue::Int(number));
                map.set("run", PyValue::int(1));
                map.set("unreadable", PyValue::Str(cat!(record, ": ", reason)));
                PyValue::Dict(map)
            }
        };
    }
    Ok(RepeatRequest {
        requested,
        source: optional("REPEATS_SOURCE"),
        kind: optional("REPEAT_KIND"),
        clamp,
    })
}

/// The boundary record of the first run of the clamped level: its object, or the text of the error that made it
/// unreadable (`OSError`, `ValueError`, or the object check).
fn read_clamp_record(path: &PyStr) -> Flow<Result<PyValue, PyStr>> {
    Ok(match read_json_file(path)? {
        Ok(document) if is_dict(&document) => Ok(document),
        Ok(_) => Err(PyStr::new("객체가 아니다")),
        Err(reason) => Err(reason),
    })
}

/// The measurement-shape record the sweep driver left (`bench_knobs`): a reassembly takes the recorded one, a fresh
/// assembly reads the record file and refuses to write an index without it.
fn knob_record(env: &dyn EnvSource, reassemble: bool, prior: &PyValue) -> Flow<PyValue> {
    if reassemble {
        let recorded = get_of(prior, "bench_knobs")?;
        return Ok(if recorded.is_truthy() {
            recorded
        } else {
            let mut absent = PyDict::new();
            absent.set(
                "absent",
                text("측정 시점 index 에 bench_knobs 없음(프리셋 신설 전 측정 · plan_26101011)"),
            );
            PyValue::Dict(absent)
        });
    }
    let reason = match var(env, "KNOB_RECORD") {
        None => PyStr::new(&PyStr::new("KNOB_RECORD").repr()),
        Some(path) => match read_json_file(&path)? {
            Ok(document) => return Ok(document),
            Err(reason) => reason,
        },
    };
    Err(Stop::Reason(cat!(
        "[sweep_bench] bench_knobs 기록을 읽지 못했다(",
        reason,
        ") — 해소 기록 없는 index 를 내지 않는다"
    )))
}

/// `json.load(open(path, encoding="utf-8"))` with the error text of what `except (OSError, ValueError)` catches. A
/// nesting beyond the recursion limit is not caught and ends the run.
fn read_json_file(path: &PyStr) -> Flow<Result<PyValue, PyStr>> {
    let os = match crate::pyfs::os_string(path) {
        Ok(os) => os,
        Err(failure) => return Ok(Err(failure.into_failure().message)),
    };
    let bytes = match std::fs::read(&os) {
        Ok(bytes) => bytes,
        Err(error) => return Ok(Err(PyStr::new(&crate::cli::os_error_text(&error, path)))),
    };
    match decode_file_text(&bytes).and_then(|decoded| loads_text(&decoded)) {
        Ok(document) => Ok(Ok(document)),
        Err(error) if error.is_value_error() => Ok(Err(PyStr::new(&error.to_string()))),
        Err(error) => Err(crate::repeat_api_cli::load_failure(&error).into()),
    }
}

/// `datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")` for the current call.
pub fn current_utc_text() -> String {
    crate::utc::UtcInstant::now().evaluated_text()
}

/// Assembles the sweep index of one cell and writes it. `clock` is called once, for a fresh measurement, and not at
/// all for a reassembly (which inherits the measurement time).
pub fn assemble(env: &dyn EnvSource, clock: &dyn Fn() -> String) -> Produced {
    let mut stderr = PyStr::default();
    let ending = match assemble_inner(env, clock, &mut stderr) {
        Ok(line) => Ending::Done(line),
        Err(Stop::Status(status)) => Ending::Status(status),
        Err(Stop::Reason(reason)) => Ending::Reason(reason),
        Err(Stop::Uncaught(failure)) => Ending::Uncaught(failure),
    };
    Produced { stderr, ending }
}

/// What the engine said about itself, side by side with what the config declared: quantization, KV dtype, attention
/// and MoE backends (value, source, declaration, mismatch), and the attention candidates the engine chose among.
struct EngineAxes {
    quantization: Measured,
    kv_dtype: Measured,
    attention: Measured,
    moe: Measured,
    attention_candidates: Option<Vec<String>>,
}

/// The measured-first axes of the engine log. A disagreement between a declaration and the measurement, and an
/// attention axis with no choice, are said on standard error (never judged here).
fn engine_axes(
    cfg_text: &[char],
    engine_log: &[char],
    runner: &[char],
    stderr: &mut PyStr,
) -> EngineAxes {
    // Quantization and KV dtype: the engine's own configuration line is the only authority.
    let engine_config: Vec<char> = scan::engine_config_line(engine_log)
        .map(|own| own.chars().collect())
        .unwrap_or_default();
    let config_value = |key: &str| scan::engine_config_value(&engine_config, key);
    let quantization = measured_first(
        config_value("quantization").as_deref(),
        scan::grep_yaml(cfg_text, "quantization").as_deref(),
    );
    let kv_dtype = measured_first(
        config_value("kv_cache_dtype").as_deref(),
        scan::grep_yaml(cfg_text, "kv-cache-dtype").as_deref(),
    );

    // Attention and MoE backends: the request and the effective choice can differ.
    let attention_measured = scan::attention_backend(engine_log);
    let attention_candidates = scan::attention_candidates(engine_log);
    let attention_declared_raw = scan::declared_attention_backend(runner);
    let attention = measured_first(
        attention_measured.as_deref(),
        attention_declared_raw.as_deref(),
    );
    let moe_measured = scan::moe_backend(engine_log);
    let moe = measured_first(
        moe_measured.as_deref(),
        scan::grep_yaml(cfg_text, "moe-backend").as_deref(),
    );
    if [&quantization, &kv_dtype, &attention, &moe]
        .iter()
        .any(|field| field.mismatch.starts_with("YES"))
    {
        stderr.push_str(&format!(
            "[sweep_bench] ⚠ 선언↔실측 불일치 — quantization:{} · kv_cache_dtype:{} · attention_backend:{} · moe_backend:{}\n",
            quantization.mismatch, kv_dtype.mismatch, attention.mismatch, moe.mismatch
        ));
    }
    if let Some(candidates) = &attention_candidates {
        if candidates.len() <= 1 {
            let shown = PyValue::List(candidates.iter().map(|own| text(own)).collect()).repr();
            let requested = attention_declared_raw
                .clone()
                .filter(|own| !own.is_empty())
                .unwrap_or_else(|| "미선언".to_string());
            stderr.push_str(&format!(
                "[sweep_bench] ⓘ 어텐션 축 무선택 — 후보 {shown} (요청 {requested} 는 실효 없음)\n"
            ));
        }
    }
    EngineAxes {
        quantization,
        kv_dtype,
        attention,
        moe,
        attention_candidates,
    }
}

/// Which node measured, in the vocabulary of the hint tag's node axis (`main`, `sub`, `cluster`), and where the answer
/// came from: an absent `self_role` is a default and says so, never a derivation.
fn measuring_node(topology: &PyStr, manifest: &[char]) -> (String, String) {
    let self_role = scan::grep_yaml(manifest, "self_role");
    if topology.equals("multi") {
        (
            "cluster".to_string(),
            "derived(topology=multi — 쌍이 하나의 측정 정체성)".to_string(),
        )
    } else if self_role.as_deref() == Some("sub") {
        (
            "sub".to_string(),
            "derived(manifest.self_role=sub)".to_string(),
        )
    } else if let Some(role) = self_role.as_deref().filter(|own| !own.is_empty()) {
        (
            "main".to_string(),
            format!("derived(manifest.self_role={role})"),
        )
    } else {
        (
            "main".to_string(),
            "defaulted(self_role absent)".to_string(),
        )
    }
}

/// The measurement-tool and dataset provenance the runs left beside their results: the tool the parser saw, the image
/// the tool really ran from, and the dataset sidecars. Each is read from the first level that has the sidecar; a level
/// without it is skipped, and a cell without any leaves the fields at their "unavailable" values (or, for the dataset
/// block, out of the index).
fn append_sidecar_provenance(
    meta: &mut PyDict,
    sweep_dir: &PyStr,
    ordered: &[BigInt],
    cfg: &PyStr,
) -> Flow<()> {
    // The measurement tool, as the parser saw it in the first readable `measured.json`.
    let measured_name = PyStr::new("measured.json");
    let (tool, tool_version, tool_version_source, error_rate) =
        tool_fields(first_readable(sweep_dir, ordered, &measured_name)?.as_ref())?;
    meta.set("bench_tool", tool);
    meta.set("bench_tool_version", tool_version);
    meta.set("bench_tool_version_source", tool_version_source);
    // Whether the server was alive when the bench ended: a second, separate read of the first readable
    // `measured.json` (the original loops over the levels twice). The file is not declared immutable, so the first
    // read is not reused: a file that changed or broke in between is seen as the second read finds it.
    let (alive, exemption) = match first_readable(sweep_dir, ordered, &measured_name)? {
        Some(sidecar) => (
            get_of(&sidecar, "server_alive_at_bench_end")?,
            get_of(&sidecar, "boundary_exemption")?,
        ),
        None => (PyValue::None, PyValue::None),
    };
    meta.set("server_alive_at_bench_end", alive);
    meta.set("boundary_exemption", exemption);
    meta.set("bench_max_error_rate", error_rate);

    // The measurement tool of the run: the image it really used, read from the run's own sidecar.
    let tool_name = cat!("bench_tool_", cfg, ".json");
    let (image_ref, image_digest_tool, endpoint) =
        match first_readable(sweep_dir, ordered, &tool_name)? {
            Some(sidecar) => (
                get_or(&sidecar, "image_ref", text("NA"))?,
                get_or(&sidecar, "observed_digest", text("NA"))?,
                get_or(&sidecar, "endpoint", text("NA"))?,
            ),
            None => (text("NA"), text("NA"), text("NA")),
        };
    meta.set("bench_tool_image_ref", image_ref);
    let tool_digest_known = !equals(&image_digest_tool, &text("NA"));
    meta.set("bench_tool_image_digest", image_digest_tool);
    meta.set(
        "bench_tool_image_digest_source",
        text(if tool_digest_known {
            "measured(docker image inspect .RepoDigests)"
        } else {
            "unavailable"
        }),
    );
    meta.set("bench_endpoint", endpoint);

    // The dataset provenance of the runs: inherited from the per-run sidecars; no sidecar leaves the keys out.
    if let Some(datasets) = dataset_scan(sweep_dir, ordered, cfg)? {
        let first = &datasets.first;
        for (field, name) in [
            ("dataset_kind", "dataset_kind"),
            ("dataset_source", "dataset_source"),
            ("dataset_id", "dataset_id"),
            ("dataset_revision", "dataset_revision"),
            ("source_sha256", "dataset_source_sha256"),
            ("preprocess_strategy", "dataset_preprocess_strategy"),
            ("seed", "dataset_seed"),
        ] {
            meta.set(name, get_of(first, field)?);
        }
        let column_map = get_of(first, "column_map")?;
        meta.set(
            "dataset_column_map",
            if get_of(first, "column_map")?.is_truthy() {
                PyValue::Str(sorted_dumps(&column_map)?)
            } else {
                PyValue::None
            },
        );
        let synthetic = equals(&get_of(first, "dataset_kind")?, &text("synthetic"));
        meta.set(
            "dataset_duplicate_sampling",
            if synthetic {
                PyValue::None
            } else {
                PyValue::Bool(datasets.duplicate)
            },
        );
        meta.set("dataset_tokenizer", get_of(first, "tokenizer")?);
        meta.set(
            "dataset_provenance",
            PyValue::Str(cat!(
                "measured(런별 사이드카 dataset_",
                cfg,
                ".json · ",
                datasets.runs.to_string(),
                " 런)"
            )),
        );
    }
    Ok(())
}

/// One entry per completed level, in level order: its measurement (or none when unreadable) and the path of the
/// level's raw bench result.
fn level_records(sweep_dir: &PyStr, cfg: &PyStr, ordered: &[BigInt]) -> Flow<Vec<PyValue>> {
    let measured_name = PyStr::new("measured.json");
    let mut levels = Vec::with_capacity(ordered.len());
    for level in ordered {
        let measured = load_json(&level_file(sweep_dir, level, &measured_name))?;
        let mut entry = PyDict::new();
        entry.set("level", PyValue::Int(level.clone()));
        entry.set("status", text("ok"));
        entry.set("measured", measured.unwrap_or(PyValue::None));
        entry.set(
            "bench_json",
            PyValue::Str(level_file(sweep_dir, level, &cat!("bench_", cfg, ".json"))),
        );
        levels.push(PyValue::Dict(entry));
    }
    Ok(levels)
}

/// The lines of `truncation.log` that name a truncated level.
fn truncation_lines(sweep_dir: &PyStr) -> Vec<PyValue> {
    let log = read(&join(sweep_dir, &[&PyStr::new("truncation.log")]));
    scan::split_lines(&log)
        .iter()
        .map(|line| scan::to_string(line))
        .filter_map(|line| {
            let line = line.trim_matches(|c: char| crate::pystr::is_python_space(u32::from(c)));
            (line.starts_with("level ") && line.contains("truncated")).then(|| text(line))
        })
        .collect()
}

fn assemble_inner(
    env: &dyn EnvSource,
    clock: &dyn Fn() -> String,
    stderr: &mut PyStr,
) -> Flow<PyStr> {
    let cfg = required(env, "CONFIG")?;
    let topo = required(env, "TOPO")?;
    let sweep_dir = required(env, "SWEEPDIR")?;
    let mut completed: Vec<BigInt> = Vec::new();
    if let Some(listed) = var(env, "COMPLETED") {
        for word in split_words(&listed) {
            completed.push(int_from_str(&word).map_err(int_failure)?);
        }
    }

    let cfg_yaml = required(env, "CFGYAML")?;
    let cfg_text = read(&cfg_yaml);
    let env_file = required(env, "EF")?;
    let env_text = read(&env_file);
    let manifest_path = required(env, "MANIFEST")?;
    let manifest = read(&manifest_path);

    // A reassembly inherits what the measurement made; the clock is not read for it.
    let reassemble = env.get("REASSEMBLE").is_some_and(|value| value.equals("1"));
    let mut prior = PyValue::Dict(PyDict::new());
    if reassemble {
        let index_path = join(&sweep_dir, &[&PyStr::new("sweep_index.json")]);
        prior = load_json(&index_path)?.unwrap_or_else(|| PyValue::Dict(PyDict::new()));
        if !is_dict(&prior) || !get_of(&prior, "generated_utc")?.is_truthy() {
            return Err(Stop::Reason(PyStr::new(
                "[sweep_bench] --reassemble-only: 기존 index 의 generated_utc 부재 — 측정시각 승계 불가(날조 금지) → 중단",
            )));
        }
    }

    // The image: declared by the env file, measured by `docker inspect` (handed in), or inherited when reassembling.
    let mut image = scan::grep_env(&env_text, "IMAGE_TAG").unwrap_or_default();
    let measured_text = |name: &str| -> PyValue {
        let value = var(env, name).unwrap_or_else(|| PyStr::new("NA"));
        let stripped = value.strip();
        PyValue::Str(if stripped.is_empty() {
            PyStr::new("NA")
        } else {
            stripped
        })
    };
    let mut image_actual = measured_text("IMAGE_TAG_ACTUAL");
    let mut image_digest = measured_text("IMAGE_DIGEST_ACTUAL");
    if reassemble {
        let recorded = get_or(&prior, "meta", PyValue::Dict(PyDict::new()))?;
        let inherited = |value_key: &str, source_key: &str| -> Flow<PyValue> {
            let source = get_of_default(&recorded, source_key, text(""))?;
            if source
                .display_text()
                .to_utf8()
                .is_some_and(|own| own.starts_with("measured"))
            {
                get_or(&recorded, value_key, text("NA")).map_err(Stop::from)
            } else {
                Ok(text("NA"))
            }
        };
        image_actual = inherited("image_tag", "image_tag_source")?;
        image_digest = inherited("image_digest", "image_digest_source")?;
    }
    let na = text("NA");
    let mut native_image: Option<String> = None;
    if var(env, "SERVE_PLANE").is_some_and(|plane| plane.equals("native"))
        && equals(&image_actual, &na)
    {
        let proof_path = join(
            &crate::ospath::dirname(&sweep_dir),
            &[&cat!("serve_proof_", cfg, ".json")],
        );
        let proof = load_json(&proof_path)?;
        if let Some(proof) = proof.filter(is_dict) {
            if equals(&get_of(&proof, "plane")?, &text("native"))
                && equals(&get_of(&proof, "status")?, &text("PASS"))
            {
                let house = get_or(&proof, "wheelhouse", PyValue::Dict(PyDict::new()))?;
                let tag = get_or(&house, "source_image_tag", text(""))?;
                let PyValue::Str(tag) = tag.peel() else {
                    return Err(attribute_error(&tag, "strip").into());
                };
                let stripped = tag.strip().to_utf8().unwrap_or_default();
                native_image = (!stripped.is_empty()).then_some(stripped);
            }
        }
        if let Some(own) = &native_image {
            image = own.clone();
        }
    }

    // The engine log of the first completed level that has one, and what the run said about itself.
    let mut ordered = completed.clone();
    ordered.sort();
    let mut engine_log: Vec<char> = Vec::new();
    for level in &ordered {
        let path = level_file(&sweep_dir, level, &cat!("engine_", cfg, ".log"));
        if crate::ospath::isfile(&path) {
            engine_log = read(&path);
            break;
        }
    }
    let engine_build = scan::engine_version(&engine_log);
    let vllm_build = engine_build.clone().unwrap_or_else(|| "NA".to_string());
    let image_chars: Vec<char> = image.chars().collect();
    let mut vllm: Option<PyStr> =
        non_empty(env, "VLLM_VER").or_else(|| non_empty(env, "EASY_VLLM_VERSION"));
    if vllm.is_none() {
        vllm = scan::image_line_version(&image_chars).map(|own| pystr_of(&own));
    }
    if vllm.is_none() {
        if let Some(build) = &engine_build {
            let build_chars: Vec<char> = build.chars().collect();
            vllm = scan::leading_version(&build_chars).map(|own| pystr_of(&own));
        }
    }
    let vllm = vllm
        .or_else(|| scan::comment_version(&cfg_text).map(|own| pystr_of(&own)))
        .unwrap_or_else(|| PyStr::new("NA"));

    // The GPU model is the manifest's alone; without it the certificate key would be `NA`, so the run stops here.
    let gpu_model = scan::grep_yaml(&manifest, "gpu_model").filter(|own| !own.is_empty());
    let Some(gpu_model) = gpu_model else {
        // plan_26111708 B L8: the remedy names the live `scan_node.sh` launcher (scan_node.py is retired).
        stderr.push_py(&cat!(
            "[sweep_bench] FAIL: manifest(",
            var(env, "MANIFEST").unwrap_or_else(|| PyStr::new("?")),
            ") 에 gpu_model 이 없다 — 인증서 강한 키 gpu 를 NA 로 발행하지 않는다. terraforming_node 스캔(서브면 scan_node.sh --emit-sub-manifest)으로 채워라.\n"
        ));
        return Err(Stop::Status(2));
    };
    let gpu_key = crate::bench_naming::gpu_key(&text(&gpu_model));

    let tensor_parallel = scan::grep_yaml(&cfg_text, "tensor-parallel-size")
        .unwrap_or_else(|| derived_tensor_parallel(&manifest, &topo));

    // The model axis is derived from the checkpoint directory name; the operating name is its own axis.
    let model_path = scan::grep_yaml(&cfg_text, "model").unwrap_or_default();
    let model_base = crate::ospath::basename(&PyStr::new(model_path.trim_end_matches('/')))
        .strip()
        .to_utf8()
        .unwrap_or_default();
    let serving_name = scan::grep_env(&env_text, "SERVING_MODEL_NAME");
    let serving_config = serving_name
        .as_deref()
        .map_or_else(|| cfg.clone(), pystr_of);
    let (model_key, model_source) = if !model_base.is_empty() {
        (
            pystr_of(&crate::pylower::python_lower(&model_base)),
            "derived(model_path basename · lowercased)",
        )
    } else if let Some(name) = &serving_name {
        (pystr_of(name), "fallback(serving_model_name)")
    } else {
        (cfg.clone(), "fallback(config_name)")
    };
    if model_base.is_empty() {
        stderr.push_py(&cat!(
            "[sweep_bench] ⚠ config yaml 에 model 경로가 없어 강한키를 조합명으로 폴백했다 (model_source=",
            model_source,
            ") — 인증서 모델 축이 조합명이 된다\n"
        ));
    }

    let runner_path = join(&crate::ospath::dirname(&cfg_yaml), &[&cat!(cfg, ".sh")]);
    let runner = read(&runner_path);
    let EngineAxes {
        quantization,
        kv_dtype,
        attention,
        moe,
        attention_candidates,
    } = engine_axes(&cfg_text, &engine_log, &runner, stderr);

    // The speculative-decoding declaration; a reassembly inherits the one of the measurement time.
    let declaration = scan::spec_declaration(crate::ospath::isfile(&cfg_yaml), &cfg_text, &runner)
        .map_err(scan_failure)?;
    let mut spec_state = text(declaration.state);
    let mut spec_k = declaration.k.map_or(PyValue::None, PyValue::Int);
    let mut spec_source = text(&declaration.source);
    if reassemble {
        let recorded = get_or(&prior, "meta", PyValue::Dict(PyDict::new()))?;
        let state = get_of(&recorded, "spec_declared")?;
        let known = ["on", "off", "unknown"]
            .iter()
            .any(|name| equals(&state, &text(name)));
        if known {
            spec_state = state;
            spec_k = get_of(&recorded, "spec_declared_k")?;
            spec_source = get_or(
                &recorded,
                "spec_declared_source",
                text("inherited(기존 sweep_index · 출처 미기재)"),
            )?;
        } else {
            spec_source = PyValue::Str(cat!(
                "retro(reassemble · 현재 config 기준 · 측정 시점 보장 없음) ",
                declaration.source
            ));
        }
    }

    let (measured_node, measured_node_source) = measuring_node(&topo, &manifest);

    let yaml_or_na = |key: &str| or_na(scan::grep_yaml(&cfg_text, key));
    let manifest_or_na = |key: &str| or_na(scan::grep_yaml(&manifest, key));
    let image_declared = scan::grep_env(&env_text, "IMAGE_TAG");
    let image_actual_known = !equals(&image_actual, &na);
    let ple_mmap = scan::grep_env(&env_text, "VLLM_PLE_MMAP").as_deref() == Some("1");

    let mut meta = PyDict::new();
    meta.set("model", PyValue::Str(model_key));
    meta.set("measured_node", text(&measured_node));
    meta.set("measured_node_source", text(&measured_node_source));
    meta.set("model_source", text(model_source));
    meta.set("serving_config", PyValue::Str(serving_config));
    meta.set("config_name", PyValue::Str(cfg.clone()));
    meta.set("gpu_model", text(&gpu_model));
    meta.set("gpu_key", PyValue::Str(gpu_key));
    meta.set("vllm_version", PyValue::Str(vllm));
    meta.set("vllm_build", text(&vllm_build));
    append_measured(&mut meta, "quantization", &quantization);
    meta.set("topology", PyValue::Str(topo.clone()));
    meta.set("tensor_parallel_size", text(&tensor_parallel));
    meta.set("driver_version", manifest_or_na("driver_version"));
    meta.set("cuda_version", manifest_or_na("cuda_version"));
    meta.set(
        "image_tag",
        if image_actual_known {
            image_actual.clone()
        } else {
            native_image
                .clone()
                .filter(|own| !own.is_empty())
                .or_else(|| image_declared.clone())
                .map_or_else(|| text("NA"), |own| text(&own))
        },
    );
    meta.set(
        "image_tag_source",
        text(if image_actual_known {
            "measured(docker inspect)"
        } else if native_image.is_some() {
            "measured(native serve proof · wheelhouse 원천 이미지)"
        } else {
            "declared(envfile)"
        }),
    );
    meta.set("image_tag_declared", or_na(image_declared));
    let digest_known = !equals(&image_digest, &na);
    meta.set("image_digest", image_digest);
    meta.set(
        "image_digest_source",
        text(if digest_known {
            "measured(docker inspect .Image)"
        } else {
            "unavailable"
        }),
    );
    meta.set("max_model_len", yaml_or_na("max-model-len"));
    meta.set("max_num_seqs", yaml_or_na("max-num-seqs"));
    meta.set("kv_cache_memory_bytes", yaml_or_na("kv-cache-memory-bytes"));
    append_measured(&mut meta, "kv_cache_dtype", &kv_dtype);
    meta.set(
        "gpu_memory_utilization",
        yaml_or_na("gpu-memory-utilization"),
    );
    meta.set("ple_mode", text(if ple_mmap { "mmap" } else { "resident" }));
    meta.set(
        "ple_mode_source",
        text(if ple_mmap {
            "declared(envfile VLLM_PLE_MMAP=1)"
        } else {
            "defaulted(envfile VLLM_PLE_MMAP absent/0 = stock resident)"
        }),
    );
    append_measured(&mut meta, "moe_backend", &moe);
    append_measured(&mut meta, "attention_backend", &attention);
    meta.set(
        "attention_backend_candidates",
        attention_candidates.map_or(PyValue::None, |list| {
            PyValue::List(list.iter().map(|own| text(own)).collect())
        }),
    );
    meta.set("enforce_eager", yaml_or_na("enforce-eager"));
    meta.set("spec_declared", spec_state);
    meta.set("spec_declared_k", spec_k);
    meta.set("spec_declared_source", spec_source);
    meta.set("serving_model_name", or_na(serving_name));
    meta.set("model_path", yaml_or_na("model"));

    append_sidecar_provenance(&mut meta, &sweep_dir, &ordered, &cfg)?;

    let levels = level_records(&sweep_dir, &cfg, &ordered)?;
    let truncated = truncation_lines(&sweep_dir);

    let lite = lite_block(env)?;

    let request = repeat_request(env, reassemble, &prior, &sweep_dir)?;
    let levels_value = PyValue::List(levels);
    let repetition = crate::repeat_axis::summarize(&crate::repeat_axis::SummarizeRequest {
        levels: &levels_value,
        requested: &request.requested,
        requested_source: &request.source,
        kind: &request.kind,
        clamp_run: &request.clamp,
    })?
    .to_value();

    let bench_knobs = knob_record(env, reassemble, &prior)?;

    let generated_utc = if reassemble {
        get_of(&prior, "generated_utc")?
    } else {
        text(&clock())
    };
    let input_len = if reassemble {
        get_of(&prior, "input_len")?
    } else {
        let given = required(env, "ILEN")?;
        PyValue::Int(int_from_str(&given).map_err(int_failure)?)
    };
    let level_numbers = PyValue::List(
        ordered
            .iter()
            .map(|level| PyValue::Int(level.clone()))
            .collect(),
    );
    let truncated_count = truncated.len();

    let mut index = PyDict::new();
    index.set("config", PyValue::Str(cfg));
    index.set("topology", PyValue::Str(topo));
    index.set("generated_utc", generated_utc);
    index.set(
        "reassembled_from_raw",
        if reassemble {
            PyValue::Bool(true)
        } else {
            PyValue::None
        },
    );
    index.set("verdict_point_level", PyValue::int(1));
    index.set("meta", PyValue::Dict(meta));
    index.set("lite", lite);
    index.set("levels", levels_value);
    index.set("repetition", repetition);
    index.set("truncated", PyValue::List(truncated));
    index.set("input_len", input_len);
    index.set("bench_knobs", bench_knobs);

    let output = join(&sweep_dir, &[&PyStr::new("sweep_index.json")]);
    write_index(&output, &PyValue::Dict(index))?;
    Ok(cat!(
        "[sweep_bench] sweep_index.json → ",
        output,
        " (완료 레벨 ",
        level_numbers.display_text(),
        " · 절삭 ",
        truncated_count.to_string(),
        "건)"
    ))
}

/// `dict.get(key, default)` on a JSON value that should be an object.
fn get_of_default(value: &PyValue, key: &str, default: PyValue) -> Result<PyValue, Failure> {
    Ok(as_get_target(value)?.get(key).cloned().unwrap_or(default))
}

/// `os.environ["COMPLETED"].split()`: the words between whitespace of any Unicode kind.
fn split_words(value: &PyStr) -> Vec<PyStr> {
    let mut words = Vec::new();
    let mut current: Vec<u32> = Vec::new();
    for &point in value.code_points() {
        if crate::pystr::is_python_space(point) {
            if !current.is_empty() {
                words.push(PyStr::from_code_points(std::mem::take(&mut current)));
            }
        } else {
            current.push(point);
        }
    }
    if !current.is_empty() {
        words.push(PyStr::from_code_points(current));
    }
    words
}

// ── writing the index ─────────────────────────────────────────────────────────────

/// `with open(path, "w", encoding="utf-8") as f: json.dump(index, f, ensure_ascii=False, indent=2)`.
///
/// The file is opened (created or emptied) before anything is encoded. `json.dump` writes the encoder's chunks one by
/// one, and a chunk that holds a lone surrogate cannot be encoded: the chunks before it have been written when the
/// error ends the run, the chunk itself and everything after it have not.
fn write_index(path: &PyStr, index: &PyValue) -> Result<(), Failure> {
    write_pretty_document(path, index, false)
}

/// [`write_index`] for any document, with the `f.write("\n")` some producers add inside the same `with` block (it
/// follows only a dump that completed).
pub(crate) fn write_pretty_document(
    path: &PyStr,
    index: &PyValue,
    final_newline: bool,
) -> Result<(), Failure> {
    use std::io::Write;
    let os = crate::pyfs::os_string(path).map_err(crate::pyfs::FsFailure::into_failure)?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(std::path::Path::new(&os))
        .map_err(|error| {
            Failure::new(
                crate::pyfs::os_error_class(&error),
                &crate::cli::os_error_text(&error, path),
            )
        })?;
    let dumped = index.dumps_pretty().map_err(dump_failure)?;
    let write_failure = |error: std::io::Error| {
        Failure::new(
            crate::pyfs::os_error_class(&error),
            &crate::cli::os_error_text(&error, path),
        )
    };
    match crate::pyfs::strict_utf8(&dumped) {
        Ok(mut bytes) => {
            if final_newline {
                bytes.push(b'\n');
            }
            file.write_all(&bytes).map_err(write_failure)
        }
        Err(_) => {
            let (kept, failing) = split_at_failing_chunk(index, &dumped);
            let prefix = crate::pyfs::strict_utf8(&kept)
                .expect("the chunks before the first lone surrogate are encodable");
            file.write_all(&prefix).map_err(write_failure)?;
            let failure = crate::pyfs::strict_utf8(&failing)
                .expect_err("the failing chunk holds a lone surrogate");
            Err(Failure::new("UnicodeEncodeError", &failure.message()))
        }
    }
}

fn is_surrogate(point: u32) -> bool {
    (0xD800..=0xDFFF).contains(&point)
}

fn has_surrogate(value: &PyStr) -> bool {
    value.code_points().iter().copied().any(is_surrogate)
}

/// Whether the first string (in output order) that holds a lone surrogate is an element of a list. The encoder
/// yields such an element together with the separator that precedes it; a key or a dict value is a chunk of its own.
fn first_surrogate_is_list_element(value: &PyValue) -> Option<bool> {
    match value.peel() {
        PyValue::List(items) | PyValue::Tuple(items) => {
            for item in items {
                match item.peel() {
                    PyValue::Str(own) if has_surrogate(own) => return Some(true),
                    other => {
                        if let Some(found) = first_surrogate_is_list_element(other) {
                            return Some(found);
                        }
                    }
                }
            }
            None
        }
        PyValue::Dict(map) => {
            for (key, member) in map.iter() {
                if has_surrogate(key) {
                    return Some(false);
                }
                match member.peel() {
                    PyValue::Str(own) if has_surrogate(own) => return Some(false),
                    other => {
                        if let Some(found) = first_surrogate_is_list_element(other) {
                            return Some(found);
                        }
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// The dumped text split where the failing chunk begins: what `json.dump` has written before it, and the chunk.
fn split_at_failing_chunk(index: &PyValue, dumped: &PyStr) -> (PyStr, PyStr) {
    let points = dumped.code_points();
    let quote = u32::from('"');
    let backslash = u32::from('\\');
    let is_closing_quote = |at: usize| {
        points[at] == quote
            && points[..at]
                .iter()
                .rev()
                .take_while(|&&p| p == backslash)
                .count()
                % 2
                == 0
    };
    let surrogate_at = points
        .iter()
        .position(|&p| is_surrogate(p))
        .expect("the text holds a lone surrogate");
    let opening = (0..surrogate_at)
        .rev()
        .find(|&at| is_closing_quote(at))
        .expect("a string token opens before its content");
    let closing = (surrogate_at..points.len())
        .find(|&at| is_closing_quote(at))
        .expect("a string token closes after its content");
    let mut start = opening;
    if first_surrogate_is_list_element(index) == Some(true) {
        // `[` or `,`, a newline and the indent: the separator the encoder yields in the same chunk
        while start > 0 && points[start - 1] == u32::from(' ') {
            start -= 1;
        }
        if start > 0 && points[start - 1] == u32::from('\n') {
            start -= 1;
        }
        start = start.saturating_sub(1);
    }
    (
        PyStr::from_code_points(points[..start].to_vec()),
        PyStr::from_code_points(points[start..=closing].to_vec()),
    )
}
