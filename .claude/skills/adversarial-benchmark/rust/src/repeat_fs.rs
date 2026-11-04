//! The file-writing operations of the repeat axis: [`record_run`] writes a run's boundary facts and
//! [`aggregate_level`] adds the repeat axis to a level's representative `measured.json`, in place.
//!
//! Observable behaviour kept from the original: `record_run` creates the run directory (and its parents) before it
//! writes, so an unwritable file leaves the directory behind; both writers open (create or truncate) the file
//! *before* the text is encoded, so text with a lone surrogate leaves an empty file; every document is
//! `json.dumps(doc, ensure_ascii=False, indent=2) + "\n"`; `aggregate_level` is idempotent because it drops the
//! keys it owns before it adds them again (at the end of the document, in a fixed order).

use crate::guidellm::Failure;
use crate::pyfs;
use crate::pystr::PyStr;
use crate::pyvalue::{DumpError, PyDict, PyValue};
use crate::repeat_axis::{
    band_pct, repeat_axis_error, AGGREGATE_KEYS, BAND_FORMULA, BAND_METRIC,
    IMPLEMENTED_REPEAT_KINDS, REPEAT_KINDS, RUN_META_NAME, RUN_METRIC_KEYS,
};
use crate::repeat_text::cat;
use num_bigint::BigInt;

const MEASURED_NAME: &str = "measured.json";
const RUN_PROVENANCE: &str = "measured(sweep_bench 반복 루프 · 시각은 run 경계에서 잰 벽시계)";
const REPRESENTATIVE_RUN_RULE: &str =
    "첫 완주 run — 최상위 측정 필드·판정점·인증서가 이 run 하나에 묶인다(합성 ✗)";

/// `Path(value)`: a string is the path; any other type is the `TypeError` pathlib raises.
pub fn path_from_value(value: &PyValue) -> Result<PyStr, Failure> {
    match value.peel() {
        PyValue::Str(text) => Ok(pyfs::normalize(text)),
        other => Err(Failure::new(
            "TypeError",
            &format!(
                "argument should be a str or an os.PathLike object where __fspath__ returns a str, not '{}'",
                other.type_name()
            ),
        )),
    }
}

fn dump_failure(error: DumpError) -> Failure {
    let text = error.to_string();
    let message = text
        .strip_prefix(&format!("{}: ", error.python_type()))
        .unwrap_or(&text);
    Failure::new(error.python_type(), message)
}

/// `json.dumps(document, ensure_ascii=False, indent=2) + "\n"`.
fn document_text(document: PyValue) -> Result<PyStr, Failure> {
    let mut text = document.dumps_pretty().map_err(dump_failure)?;
    text.push_str("\n");
    Ok(text)
}

// ── record_run ──────────────────────────────────────────────────────────────────

/// A run's boundary facts as the sweep driver reports them. The values are written as given: the original does
/// not look at their type.
#[derive(Clone, Debug)]
pub struct RunBoundary {
    pub level: PyValue,
    pub run: PyValue,
    pub started_utc: PyValue,
    pub ended_utc: PyValue,
    pub run_bench_rc: PyValue,
    pub parse_rc: PyValue,
}

impl RunBoundary {
    fn to_document(&self) -> PyValue {
        PyValue::Dict(PyDict::from_distinct_entries([
            (PyStr::new("schema_version"), PyValue::int(1)),
            (PyStr::new("level"), self.level.clone()),
            (PyStr::new("run"), self.run.clone()),
            (PyStr::new("started_utc"), self.started_utc.clone()),
            (PyStr::new("ended_utc"), self.ended_utc.clone()),
            (PyStr::new("run_bench_rc"), self.run_bench_rc.clone()),
            (PyStr::new("parse_rc"), self.parse_rc.clone()),
            (PyStr::new("provenance"), PyValue::str(RUN_PROVENANCE)),
        ]))
    }
}

/// Writes `repeat_run.json` into `run_dir` (created with its parents if needed) and returns the file's path.
///
/// Failure state: a directory that could not be made leaves what was made; a file that could not be written leaves
/// the directory (and, for text that cannot be encoded, an empty file).
pub fn record_run(run_dir: &PyValue, boundary: &RunBoundary) -> Result<PyStr, Failure> {
    let run_dir = path_from_value(run_dir)?;
    pyfs::mkdir_parents_exist_ok(&run_dir)?;
    let text = document_text(boundary.to_document())?;
    let out = pyfs::join(&run_dir, RUN_META_NAME);
    pyfs::write_text(&out, &text)?;
    Ok(out)
}

// ── aggregate_level ─────────────────────────────────────────────────────────────

/// One run directory of a level: its number and its path. Run 1 is the level directory itself (the layout older
/// consumers expect); `run_NN` directories with `NN >= 2` are the others.
struct RunDirectory {
    number: BigInt,
    path: PyStr,
    /// The `dir` field of the run's entry: `.` for the level directory, the directory name otherwise.
    label: PyStr,
    is_level_dir: bool,
}

/// `^run_(\d{2,})$` on a directory name: the run number. `$` also matches before one trailing newline.
fn run_directory_number(name: &PyStr) -> Option<BigInt> {
    let points = name.code_points();
    let body = points.strip_suffix(&[0x0A]).unwrap_or(points);
    let prefix: Vec<u32> = "run_".chars().map(u32::from).collect();
    let digits = body.strip_prefix(prefix.as_slice())?;
    if digits.len() < 2
        || !digits
            .iter()
            .all(|&point| crate::pystr::is_decimal_digit(point))
    {
        return None;
    }
    crate::pynumber::int_from_str(&PyStr::from_code_points(digits.to_vec())).ok()
}

fn run_directories(level_dir: &PyStr) -> Result<Vec<RunDirectory>, Failure> {
    let mut found = vec![RunDirectory {
        number: BigInt::from(1),
        path: level_dir.clone(),
        label: PyStr::new("."),
        is_level_dir: true,
    }];
    if pyfs::is_dir(level_dir)? {
        for name in pyfs::list_dir(level_dir)? {
            let path = pyfs::join_text(level_dir, &name);
            let number = run_directory_number(&name);
            if pyfs::is_dir(&path)? {
                if let Some(number) = number.filter(|number| *number >= BigInt::from(2)) {
                    found.push(RunDirectory {
                        number,
                        path,
                        label: name,
                        is_level_dir: false,
                    });
                }
            }
        }
    }
    found.sort_by(|left, right| left.number.cmp(&right.number));
    Ok(found)
}

/// `value == 0` for a JSON value (`0`, `0.0` and `False` are equal to it).
fn is_zero(value: Option<&PyValue>) -> bool {
    value.is_some_and(|value| value.equals_int(&BigInt::from(0)))
}

fn index_error() -> Failure {
    Failure::new("IndexError", "list index out of range")
}

/// What the aggregation asks of a level: the request the sweep recorded and the repeat kind.
#[derive(Clone, Copy, Debug)]
pub struct AggregateRequest<'a> {
    pub requested: &'a PyValue,
    pub requested_source: &'a PyValue,
    pub kind: &'a PyValue,
}

fn kind_is_implemented(kind: &PyValue) -> bool {
    matches!(kind.peel(), PyValue::Str(text) if IMPLEMENTED_REPEAT_KINDS.iter().any(|known| text.equals(known)))
}

fn tuple_text(names: &[&str]) -> String {
    let inner: Vec<String> = names.iter().map(|name| PyStr::new(name).repr()).collect();
    if inner.len() == 1 {
        format!("({},)", inner[0])
    } else {
        format!("({})", inner.join(", "))
    }
}

/// Adds the repeat axis to the representative run's `measured.json` of `level_dir`, in place, and returns the
/// document written. The representative run is the first completed run - always run 1, since a level whose first
/// run broke is a clamp, not a measured level - so the top-level fields the judgment point and the certificate read
/// stay that run's parser output (never an average).
///
/// Failure state: nothing is written when the aggregation fails; a write that fails after the file was opened
/// leaves it truncated (see the module documentation).
pub fn aggregate_level(
    level_dir: &PyValue,
    request: &AggregateRequest<'_>,
) -> Result<PyValue, Failure> {
    let level_dir = path_from_value(level_dir)?;
    if !kind_is_implemented(request.kind) {
        return Err(repeat_axis_error(cat!(
            "repeat_kind=",
            request.kind.repr(),
            " 는 구현되지 않았다(구현: ",
            tuple_text(&IMPLEMENTED_REPEAT_KINDS),
            " · 선언 슬롯: ",
            tuple_text(&REPEAT_KINDS),
            ")"
        )));
    }
    let measured_path = pyfs::join(&level_dir, MEASURED_NAME);
    let representative = match pyfs::load_json_or_none(&measured_path)? {
        Some(PyValue::Dict(document))
            if matches!(document.get("measurement_ok"), Some(PyValue::Bool(true))) =>
        {
            document
        }
        _ => {
            return Err(repeat_axis_error(cat!(
                measured_path,
                " 가 완주한 대표 run 이 아니다 — 절삭 레벨은 집계 대상이 아니다"
            )))
        }
    };

    let mut runs: Vec<PyValue> = Vec::new();
    let mut completed: Vec<usize> = Vec::new();
    for directory in run_directories(&level_dir)? {
        let meta_path = pyfs::join(&directory.path, RUN_META_NAME);
        let meta = match pyfs::load_json_or_none(&meta_path)? {
            Some(PyValue::Dict(meta)) => meta,
            _ => {
                return Err(repeat_axis_error(cat!(
                    meta_path,
                    " 부재·파손 — run 경계 사실이 없으면 시각 대조가 성립하지 않는다"
                )))
            }
        };
        let loaded;
        let measured: Option<&PyDict> = if directory.is_level_dir {
            Some(&representative)
        } else {
            loaded = pyfs::load_json_or_none(&pyfs::join(&directory.path, MEASURED_NAME))?;
            match &loaded {
                Some(PyValue::Dict(map)) => Some(map),
                _ => None,
            }
        };
        let ok = is_zero(meta.get("run_bench_rc"))
            && is_zero(meta.get("parse_rc"))
            && measured
                .is_some_and(|map| matches!(map.get("measurement_ok"), Some(PyValue::Bool(true))));
        let mut entry = PyDict::with_capacity(7 + RUN_METRIC_KEYS.len());
        entry.set("run", PyValue::Int(directory.number.clone()));
        entry.set("dir", PyValue::Str(directory.label.clone()));
        for key in ["started_utc", "ended_utc", "run_bench_rc", "parse_rc"] {
            entry.set(key, meta.get(key).cloned().unwrap_or(PyValue::None));
        }
        entry.set("measurement_ok", PyValue::Bool(ok));
        for key in RUN_METRIC_KEYS {
            entry.set(
                key,
                measured
                    .and_then(|map| map.get(key))
                    .cloned()
                    .unwrap_or(PyValue::None),
            );
        }
        if ok {
            completed.push(runs.len());
        }
        runs.push(PyValue::Dict(entry));
    }

    let metric_values: Vec<PyValue> = completed
        .iter()
        .map(|&index| entry_field(&runs[index], BAND_METRIC))
        .collect();
    let band = band_pct(&PyValue::List(metric_values))?;
    let first_completed = completed.first().ok_or_else(index_error)?;

    let mut document = PyDict::with_capacity(representative.len() + AGGREGATE_KEYS.len());
    for (key, value) in representative.iter() {
        if !AGGREGATE_KEYS.iter().any(|owned| key.equals(owned)) {
            document.insert(key.clone(), value.clone());
        }
    }
    document.set("runs", PyValue::List(runs.clone()));
    document.set("repeats_requested", request.requested.clone());
    document.set("repeats_requested_source", request.requested_source.clone());
    document.set("repeats_completed", PyValue::int(completed.len()));
    document.set("repeat_kind", request.kind.clone());
    document.set(
        "representative_run",
        entry_field(&runs[*first_completed], "run"),
    );
    document.set(
        "representative_run_rule",
        PyValue::str(REPRESENTATIVE_RUN_RULE),
    );
    document.set(
        "repro_band_pct",
        band.percent.map_or(PyValue::None, PyValue::Float),
    );
    document.set("repro_band_source", PyValue::Str(band.source.clone()));
    document.set("repro_band_metric", PyValue::str(BAND_METRIC));
    document.set("repro_band_formula", PyValue::str(BAND_FORMULA));

    let document = PyValue::Dict(document);
    let text = document_text(document.clone())?;
    pyfs::write_text(&measured_path, &text)?;
    Ok(document)
}

fn entry_field(entry: &PyValue, key: &str) -> PyValue {
    match entry {
        PyValue::Dict(map) => map.get(key).cloned().unwrap_or(PyValue::None),
        _ => PyValue::None,
    }
}
