//! The performance report renderer: a port of `render_report.py`.
//!
//! Two documents come out of here, both deterministic text with no judgement of their own:
//!
//! * the **full report** ([`build_md`]) of a sweep (`sweep_index.json` plus the verdict point), always published,
//!   whatever the verdict was;
//! * the **lite report** ([`build_lite_md`]) of a cell that only ran the quick lite measurement, with its naming
//!   identity derived from the files of the run ([`lite_identity`]).
//!
//! Both carry a *measurement configuration* table ([`measurement_config_section`]) whose title, key names and row
//! shape are a parsing contract of the hint publisher; the values come from the classification owner
//! (`classify_cell`) and are only copied here.
//!
//! Every renderer reads dynamically typed JSON: a missing or `null` field is `N/A`, and a field of an unexpected
//! type fails with the exception Python would raise (usually `AttributeError` from a `.get` on a non-dict), so
//! the functions take and return [`PyValue`]s.

use crate::bench_naming::gpu_key;
use crate::classify_cell::{bench_mode_kind, declared_lite_record, events_scan_summary};
use crate::guidellm::Failure;
use crate::pydyn::{as_get_target, equals, iterate, join_values, subscript};
use crate::pynumber::int_from_str;
use crate::pystr::{lit, PyStr};
use crate::pyvalue::{PyDict, PyValue};
use crate::report_support::{
    display, fnum_default, grep_env, grep_yaml, na, na_with_suffix, py, read_text_or_empty,
    table_cell, Res,
};
use std::sync::LazyLock;

pub const LITE_ONLY_ERROR_CLASS: &str = "LiteIdentityError";

pub const MEASUREMENT_CONFIG_TITLE: &str =
    "## 측정 구성 — bench_mode · 도구 · 반복 (기재 · 게이트 아님)";
/// Row order of the measurement configuration table (a contract with the hint publisher's parser).
pub const MEASUREMENT_CONFIG_KEYS: [&str; 21] = [
    "bench_mode",
    "bench_mode_kind",
    "bench_mode_source",
    "downgrade_reason",
    "downgrade_reason_source",
    "bench_tool",
    "bench_tool_version",
    "bench_tool_version_source",
    "repeats",
    "repeats_source",
    "repeats_completed",
    "dataset_kind",
    "dataset_source",
    "dataset_id",
    "dataset_revision",
    "dataset_source_sha256",
    "dataset_column_map",
    "dataset_preprocess_strategy",
    "dataset_seed",
    "dataset_duplicate_sampling",
    "dataset_tokenizer",
];
const DATASET_KEYS: [&str; 10] = [
    "dataset_kind",
    "dataset_source",
    "dataset_id",
    "dataset_revision",
    "dataset_source_sha256",
    "dataset_column_map",
    "dataset_preprocess_strategy",
    "dataset_seed",
    "dataset_duplicate_sampling",
    "dataset_tokenizer",
];
/// The lite leg always measures with `vllm bench serve` (spelled as the sweep assembler spells it).
pub const LITE_BENCH_TOOL: &str = "vllm-bench-serve";
pub const LITE_BENCH_TOOL_VERSION_SOURCE: &str = "declared(도구가 버전을 자기보고하지 않는다)";
pub const LITE_DECLARED_DETAIL: &str =
    "lite_bench --publish-report · lite-only 셀(full 을 시도하지 않았다)";
pub const LITE_REPEATS_SOURCE: &str =
    "lite(cold 1회 + warm burst 1회 · 반복 축 없음 — full 정의 밖)";
const ABSENT_RECORD_STATUS: &str = "absent(판정 기록을 넘기지 않았다)";

/// The document being assembled: its lines are values because the original appends whatever a field held (a
/// non-string table makes the final join fail like Python's).
#[derive(Default)]
struct Lines(Vec<PyValue>);

impl Lines {
    fn add(&mut self, line: PyStr) {
        self.0.push(PyValue::Str(line));
    }

    fn raw(&mut self, value: PyValue) {
        self.0.push(value);
    }

    fn extend(&mut self, other: Vec<PyStr>) {
        self.0.extend(other.into_iter().map(PyValue::Str));
    }

    /// `"\n".join(lines) + "\n"`.
    fn into_document(self) -> Res<PyStr> {
        let mut text = join_values("\n", &self.0)?;
        text.push_str("\n");
        Ok(text)
    }
}

/// `"%s"`-style assembly of a line from literal pieces and values.
struct Row(PyStr);

impl Row {
    /// Starts a line with room for the values that follow, so appending does not regrow the buffer.
    fn of(head: PyStr) -> Row {
        let mut points = Vec::with_capacity(head.len() + 96);
        points.extend_from_slice(head.code_points());
        Row(PyStr::from_code_points(points))
    }

    fn lit(mut self, text: &str) -> Row {
        self.0.push_str(text);
        self
    }

    fn fixed(mut self, text: PyStr) -> Row {
        self.0.push_py(&text);
        self
    }

    fn val(mut self, value: &PyValue) -> Row {
        match value.peel() {
            PyValue::Str(text) => self.0.push_py(text),
            PyValue::Int(number) => self.0.push_str(&number.to_string()),
            other => self.0.push_py(&other.display_text()),
        }
        self
    }

    fn text(mut self, text: &PyStr) -> Row {
        self.0.push_py(text);
        self
    }

    fn done(self) -> PyStr {
        self.0
    }
}

static NONE: PyValue = PyValue::None;
static EMPTY_DICT: LazyLock<PyValue> = LazyLock::new(|| PyValue::Dict(PyDict::new()));
static EMPTY_LIST: LazyLock<PyValue> = LazyLock::new(|| PyValue::List(Vec::new()));
static WORD_NA: LazyLock<PyValue> = LazyLock::new(|| PyValue::str("NA"));
static WORD_N_SLASH_A: LazyLock<PyValue> = LazyLock::new(|| PyValue::str("N/A"));
static WORD_EMPTY: LazyLock<PyValue> = LazyLock::new(|| PyValue::str(""));
static WORD_REASON_NA: LazyLock<PyValue> = LazyLock::new(|| PyValue::str("사유 N/A"));

fn empty_dict() -> &'static PyValue {
    &EMPTY_DICT
}

fn empty_list() -> &'static PyValue {
    &EMPTY_LIST
}

/// `source.get(key)`: the value or `None`; `source` must be a dict. Borrowed: reading never copies the document.
fn get<'a>(source: &'a PyValue, key: &str) -> Res<&'a PyValue> {
    Ok(as_get_target(source)?.get(key).unwrap_or(&NONE))
}

/// `source.get(key, default)`.
fn get_or<'a>(source: &'a PyValue, key: &str, default: &'a PyValue) -> Res<&'a PyValue> {
    Ok(as_get_target(source)?.get(key).unwrap_or(default))
}

/// `source.get(key) or fallback`.
fn get_or_else<'a>(source: &'a PyValue, key: &str, fallback: &'a PyValue) -> Res<&'a PyValue> {
    let value = get(source, key)?;
    Ok(if value.is_truthy() { value } else { fallback })
}

fn is_dict(value: &PyValue) -> bool {
    matches!(value.peel(), PyValue::Dict(_))
}

fn list_of(value: &PyValue, key: &str) -> Res<Vec<PyValue>> {
    iterate(get_or(value, key, empty_list())?)
}

// ── bench_mode view and the repetition section ────────────────────────────────

/// `_bench_mode_view`: one line about the bench-mode decision record, saying so (and why) when it is missing,
/// unreadable or belongs to another measurement.
pub fn bench_mode_view(record: &PyValue, record_status: &PyValue) -> Res<PyStr> {
    if !is_dict(record) {
        return Ok(Row::of(lit!("미확정 — ")).val(record_status).done());
    }
    let mode = get(record, "bench_mode")?;
    let reason = get(record, "downgrade_reason")?;
    if matches!(mode.peel(), PyValue::None) {
        let source = get_or_else(record, "bench_mode_source", &WORD_REASON_NA)?;
        return Ok(Row::of(lit!("미확정 — ")).val(source).done());
    }
    let correlation = get(record, "downgrade_correlation")?;
    let scope = events_scan_summary(get(record, "events_scan")?)?;
    let unseen = if matches!(correlation.peel(), PyValue::Str(text) if text.equals("not_scanned") || text.equals("unavailable"))
    {
        " · ⚠ **대조 불가**(보지 못했음 — 사살 없음이 아니다 · 강등 트리거 아님 · 기재)"
    } else {
        ""
    };
    let source = if reason.is_truthy() {
        get(record, "downgrade_reason_source")?
    } else {
        get(record, "bench_mode_source")?
    };
    let source_text = if source.is_truthy() {
        display(source)
    } else {
        PyStr::default()
    };
    let tail = if scope.is_truthy() {
        let PyValue::Str(scope_text) = scope.peel() else {
            return Err(Failure::new(
                "TypeError",
                &format!(
                    "'in <string>' requires string as left operand, not {}",
                    scope.type_name()
                ),
            ));
        };
        if contains(source_text.code_points(), scope_text.code_points()) {
            PyStr::default()
        } else {
            Row::of(lit!(" · ")).text(scope_text).done()
        }
    } else {
        PyStr::default()
    };
    Ok(if reason.is_truthy() {
        Row::of(lit!("**"))
            .val(mode)
            .fixed(lit!("** — 반복 불성립(기계 이벤트)으로 **강등** · 사유 `"))
            .val(reason)
            .fixed(lit!("` · 사살 대조 `"))
            .val(correlation)
            .fixed(lit!("`"))
            .lit(unseen)
            .text(&tail)
            .fixed(lit!(" ("))
            .val(source)
            .fixed(lit!(")"))
            .done()
    } else {
        Row::of(lit!("**"))
            .val(mode)
            .fixed(lit!("** · 사살 대조 `"))
            .val(correlation)
            .fixed(lit!("`"))
            .lit(unseen)
            .text(&tail)
            .fixed(lit!(" ("))
            .val(source)
            .fixed(lit!(")"))
            .done()
    })
}

fn contains(haystack: &[u32], needle: &[u32]) -> bool {
    needle.is_empty()
        || haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

/// `_repetition_section`: the repetition-axis and reproducibility-band section, a table of its own (the sweep table
/// is parsed by the hint publisher and must keep its shape).
pub fn repetition_section(
    index: &PyValue,
    bench_mode_record: &PyValue,
    bench_mode_status: &PyValue,
) -> Res<Vec<PyStr>> {
    let mut out: Vec<PyStr> = Vec::new();
    let mut add = |line: PyStr| out.push(line);
    let repetition = get(index, "repetition")?;
    let rep = if is_dict(repetition) {
        Some(repetition)
    } else {
        None
    };
    add(py(
        "## 반복 축 · 재현 밴드 (full 정의 = lite ∪ GuideLLM × 반복 ≥3)",
    ));
    add(PyStr::default());
    add(lit!("> full 정의는 레벨마다 같은 running serve 에 **반복 ≥3** 이다. 위 스윕 표와 판정점·인증서는 **대표 run(첫 완주 run) 하나**의 값이다(평균·합성 ✗). 재현 밴드는 **기재**다 — 판정 게이트는 불변이고 분산은 강등 사유가 아니다. 반복 불성립(판정점 run 실패 · 스윕이 멈춘 자리의 블랙박스 kill)만 셀을 lite 로 강등하며 확정은 `classify_cell.py` 가 한다. 포화 경계에서 스윕이 멈춘 것(적응 상한 클램프)은 강등이 아니다."));
    add(PyStr::default());
    add(Row::of(lit!("- bench_mode: "))
        .text(&bench_mode_view(bench_mode_record, bench_mode_status)?)
        .done());
    let Some(rep) = rep else {
        add(PyStr::default());
        add(lit!("> ⚠ **반복 축 기록 없음** — 반복 축 신설(2026-09-14) 전 산출물이거나 재조립 원천에 runs[] 가 없다. 이 리포트의 수치는 단일 run 이며 **산포 추정치가 없다**(다른 도구·조건의 밴드를 빌려 쓰지 말 것)."));
        add(PyStr::default());
        return Ok(out);
    };
    add(Row::of(lit!("- 요청 반복: "))
        .text(&na(get(rep, "requested")?))
        .fixed(lit!(" ("))
        .text(&na(get(rep, "requested_source")?))
        .fixed(lit!(") · 종류 `"))
        .text(&na(get(rep, "kind")?))
        .fixed(lit!("` · 레벨별 완주 min "))
        .text(&na(get(rep, "completed_min")?))
        .done());
    let stop = get(rep, "stop")?;
    if is_dict(stop) {
        let kind = get(stop, "kind")?;
        let what = if matches!(kind.peel(), PyValue::Str(text) if text.equals("repeat-break")) {
            "반복 중단"
        } else {
            "적응 상한 클램프(레벨 첫 run 실패)"
        };
        add(Row::of(lit!("- ⚠ **스윕이 멈춘 자리 — "))
            .lit(what)
            .fixed(lit!("**: level "))
            .val(get(stop, "level")?)
            .fixed(lit!(" run "))
            .val(get(stop, "run")?)
            .fixed(lit!(" — "))
            .text(&na(get(stop, "detail")?))
            .fixed(lit!(
                " (남은 반복·상위 레벨은 측정하지 않았다 · 조용히 자르지 않는다)"
            ))
            .done());
    }
    let unrecorded = get(rep, "unrecorded_levels")?;
    if unrecorded.is_truthy() {
        add(Row::of(lit!("- ⚠ runs[] 가 없는 레벨: "))
            .val(unrecorded)
            .fixed(lit!(
                " (집계 실패 또는 이전 산출물 — 완주 0 회로 읽지 말 것)"
            ))
            .done());
    }
    add(PyStr::default());
    add(py(
        "| 동시성 | 완주/요청 | decode t/s (run 순) | 재현 밴드 % | 밴드 출처 |",
    ));
    add(lit!("|---|---|---|---|---|"));
    for level in list_of(index, "levels")? {
        let measured = get_or_else(&level, "measured", empty_dict())?;
        let runs_value = get(measured, "runs")?;
        let level_name = get(&level, "level")?;
        let PyValue::List(runs) = runs_value.peel() else {
            add(Row::of(lit!("| "))
                .val(level_name)
                .fixed(lit!(" | N/A | N/A | N/A | runs[] 없음 |"))
                .done());
            continue;
        };
        let mut sequence: Vec<PyValue> = Vec::with_capacity(runs.len());
        for run in runs {
            let shown = if get(run, "measurement_ok")?.is_truthy() {
                fnum_default(get(run, "decode_tps")?)?
            } else {
                Row::of(lit!("✗(run "))
                    .val(get(run, "run")?)
                    .fixed(lit!(")"))
                    .done()
            };
            sequence.push(PyValue::Str(shown));
        }
        add(Row::of(lit!("| "))
            .val(level_name)
            .fixed(lit!(" | "))
            .text(&na(get(measured, "repeats_completed")?))
            .fixed(lit!("/"))
            .text(&na(get(measured, "repeats_requested")?))
            .fixed(lit!(" | "))
            .text(&join_values(" / ", &sequence)?)
            .fixed(lit!(" | "))
            .text(&fnum_default(get(measured, "repro_band_pct")?)?)
            .fixed(lit!(" | "))
            .text(&na(get(measured, "repro_band_source")?))
            .fixed(lit!(" |"))
            .done());
    }
    add(PyStr::default());
    add(Row::of(lit!("_밴드 = "))
        .val(get_or_else(rep, "band_formula", &WORD_N_SLASH_A)?)
        .fixed(lit!(" · 지표 "))
        .val(get_or_else(rep, "band_metric", &WORD_N_SLASH_A)?)
        .fixed(lit!(
            " · 완주 run 만 · 2회 미만이면 정의되지 않는다(N/A) · 레벨 run 시도 합 "
        ))
        .text(&na(get(rep, "runs_attempted")?))
        .fixed(lit!("._"))
        .done());
    add(PyStr::default());
    Ok(out)
}

// ── measurement configuration table ───────────────────────────────────────────

/// `measurement_config_section(values)`: the table lines for a dict of configuration values (key order fixed).
pub fn measurement_config_section(values: &PyValue) -> Res<Vec<PyStr>> {
    let mut out = vec![
        py(MEASUREMENT_CONFIG_TITLE),
        PyStr::default(),
        lit!("> 같은 모델·하드웨어라도 도구·반복·모드가 다르면 수치를 나란히 놓지 않는다. `bench_mode=lite` 는 full 정의(lite ∪ GuideLLM × 반복 ≥3)를 충족하지 않은 측정이다 — 선언된 lite-only 이거나 반복 불성립으로 강등된 셀이다(강등이면 `downgrade_reason`). 이 표는 기재이지 판정이 아니다."),
        PyStr::default(),
        lit!("| 키 | 값 |"),
        lit!("|---|---|"),
    ];
    for key in MEASUREMENT_CONFIG_KEYS {
        out.push(
            Row::of(lit!("| "))
                .lit(key)
                .fixed(lit!(" | "))
                .text(&table_cell(get(values, key)?))
                .fixed(lit!(" |"))
                .done(),
        );
    }
    out.push(PyStr::default());
    Ok(out)
}

fn put(map: &mut PyDict, key: &str, value: PyValue) {
    map.set(key, value);
}

/// `full_measurement_config`: the configuration values of a full sweep. The bench mode itself is the
/// classification owner's decision record; nothing is decided here.
pub fn full_measurement_config(
    index: &PyValue,
    bench_mode_record: &PyValue,
    bench_mode_status: &PyValue,
) -> Res<PyValue> {
    let meta = get_or_else(index, "meta", empty_dict())?;
    let repetition_value = get(index, "repetition")?;
    let rep = if is_dict(repetition_value) {
        repetition_value
    } else {
        empty_dict()
    };
    let record = if is_dict(bench_mode_record) {
        Some(bench_mode_record)
    } else {
        None
    };
    let has_record = record.is_some_and(PyValue::is_truthy);
    let has_rep = rep.is_truthy();
    let record_get = |key: &str| -> Res<PyValue> {
        match record {
            Some(record) if has_record => get(record, key).cloned(),
            _ => Ok(PyValue::None),
        }
    };
    let verdict_point = get(index, "verdict_point_level")?;
    let completed = if has_rep {
        let by_level = get_or_else(rep, "completed_by_level", empty_dict())?;
        let level_key = PyValue::Str(display(verdict_point));
        match level_key.peel() {
            PyValue::Str(name) => get(by_level, &name.to_utf8().unwrap_or_default())?.clone(),
            _ => PyValue::None,
        }
    } else {
        PyValue::None
    };
    let mut values = PyDict::new();
    put(&mut values, "bench_mode", record_get("bench_mode")?.clone());
    put(
        &mut values,
        "bench_mode_kind",
        PyValue::str(bench_mode_kind(record.unwrap_or(&PyValue::None))),
    );
    put(
        &mut values,
        "bench_mode_source",
        if has_record {
            record_get("bench_mode_source")?
        } else {
            PyValue::Str(Row::of(lit!("미확정 — ")).val(bench_mode_status).done())
        },
    );
    put(
        &mut values,
        "downgrade_reason",
        record_get("downgrade_reason")?,
    );
    put(
        &mut values,
        "downgrade_reason_source",
        record_get("downgrade_reason_source")?,
    );
    put(&mut values, "bench_tool", get(meta, "bench_tool")?.clone());
    put(
        &mut values,
        "bench_tool_version",
        get(meta, "bench_tool_version")?.clone(),
    );
    put(
        &mut values,
        "bench_tool_version_source",
        get(meta, "bench_tool_version_source")?.clone(),
    );
    put(
        &mut values,
        "repeats",
        if has_rep {
            get(rep, "requested")?.clone()
        } else {
            PyValue::None
        },
    );
    put(
        &mut values,
        "repeats_source",
        if has_rep {
            get(rep, "requested_source")?.clone()
        } else {
            PyValue::str("absent(반복 축 기록 없음 — 신설 전 산출물)")
        },
    );
    put(&mut values, "repeats_completed", completed);
    for key in DATASET_KEYS {
        put(&mut values, key, get(meta, key)?.clone());
    }
    Ok(PyValue::Dict(values))
}

// ── manifest path labels ──────────────────────────────────────────────────────

/// `_label_path(value, labels)`: a host path starting with a manifest path prefix is shown as the manifest field
/// that holds it (the report shows no operator path). Only the display changes.
fn label_path(value: &PyValue, labels: &PyValue) -> Res<PyValue> {
    let PyValue::Str(text) = value.peel() else {
        return Ok(value.clone());
    };
    if !labels.is_truthy() {
        return Ok(value.clone());
    }
    for pair in iterate(labels)? {
        let parts = match iterate(&pair) {
            Ok(parts) => parts,
            Err(_) => {
                return Err(Failure::new(
                    "TypeError",
                    &format!("cannot unpack non-iterable {} object", pair.type_name()),
                ))
            }
        };
        let [prefix, label] = parts.as_slice() else {
            let got = parts.len();
            return Err(Failure::new(
                "ValueError",
                &if got > 2 {
                    "too many values to unpack (expected 2)".to_string()
                } else {
                    format!("not enough values to unpack (expected 2, got {got})")
                },
            ));
        };
        if equals(value, prefix) {
            return concatenate(label, text.code_points().len(), text, prefix);
        }
        let PyValue::Str(prefix_text) = prefix.peel() else {
            return Err(crate::bench_naming::concat_failure(prefix));
        };
        let mut with_slash = prefix_text.clone();
        with_slash.push_str("/");
        if text.code_points().starts_with(with_slash.code_points()) {
            return concatenate(label, prefix_text.len(), text, prefix);
        }
    }
    Ok(value.clone())
}

/// `label + value[len(prefix):]`.
fn concatenate(label: &PyValue, skip: usize, value: &PyStr, prefix: &PyValue) -> Res<PyValue> {
    let skip = match prefix.peel() {
        PyValue::Str(text) => text.len(),
        _ => skip.min(value.len()),
    };
    let PyValue::Str(head) = label.peel() else {
        return Err(crate::bench_naming::concat_failure(label));
    };
    let mut text = head.clone();
    text.push_py(&PyStr::from_code_points(
        value.code_points()[skip.min(value.len())..].to_vec(),
    ));
    Ok(PyValue::Str(text))
}

// ── the full report ───────────────────────────────────────────────────────────

const SNAPSHOT_KEYS: [&str; 21] = [
    "model",
    "model_source",
    "serving_config",
    "gpu_model",
    "vllm_version",
    "quantization",
    "quantization_source",
    "topology",
    "tensor_parallel_size",
    "driver_version",
    "cuda_version",
    "image_tag",
    "max_model_len",
    "max_num_seqs",
    "kv_cache_memory_bytes",
    "kv_cache_dtype",
    "kv_cache_dtype_source",
    "gpu_memory_utilization",
    "moe_backend",
    "enforce_eager",
    "model_path",
];

/// `a.get(key) or b.get(key)`: the second lookup happens only when the first value is falsy.
fn first_truthy<'a>(rubric: &'a PyValue, roofline: &'a PyValue, key: &str) -> Res<&'a PyValue> {
    let first = get(rubric, key)?;
    if first.is_truthy() {
        Ok(first)
    } else {
        get(roofline, key)
    }
}

/// The full report of one sweep, as text.
///
/// `index` is the sweep index, `verdict` the verdict-point judgement (displayed, never decided here), `roofline`
/// an optional supplement, `bench_mode_record`/`bench_mode_status` the bench-mode decision record and how it was
/// read, and `path_labels` the manifest path labels (`None` for none). The result always ends with a newline.
pub fn build_md(
    index: &PyValue,
    verdict: &PyValue,
    roofline: &PyValue,
    bench_mode_record: &PyValue,
    bench_mode_status: &PyValue,
    path_labels: &PyValue,
) -> Res<PyStr> {
    let meta = get_or(index, "meta", empty_dict())?;
    let model = get_or(meta, "model", &WORD_NA)?;
    let gpu = get_or(meta, "gpu_model", &WORD_NA)?;
    let vllm = get_or(meta, "vllm_version", &WORD_NA)?;
    let mut lines = Lines::default();
    lines.add(
        Row::of(lit!("# 성능 보고서 — `"))
            .val(model)
            .fixed(lit!("` @ "))
            .val(gpu)
            .fixed(lit!(" · vLLM "))
            .val(vllm)
            .done(),
    );
    lines.add(lit!(""));
    lines.add(
        Row::of(lit!("> ⚠ **CARRY-FORWARD 재검증**: 이 보고서는 아래 '측정 환경' 한정 계측이다(그때-그 HW/config). 다른 세션·하드웨어·드라이버·이미지에서 이 수치를 그대로 신뢰하지 말 것 — 소비 전 재측정. 이 문서는 **inform-only**(판정 권한 없음 · verdict_rule 독점). 생성일 "))
            .val(get_or(index, "generated_utc", &WORD_N_SLASH_A)?)
            .fixed(lit!("."))
            .done(),
    );
    lines.add(lit!(""));
    lines.extend(measurement_config_section(&full_measurement_config(
        index,
        bench_mode_record,
        bench_mode_status,
    )?)?);

    let verdict_name = get_or(verdict, "verdict", &WORD_N_SLASH_A)?;
    let rubric = get_or_else(verdict, "rubric", empty_dict())?;
    lines.add(lit!("## 판정 (표시만 — verdict_rule.py 결과)"));
    lines.add(lit!(""));
    lines.add(lit!("| 항목 | 값 |"));
    lines.add(lit!("|---|---|"));
    lines.add(
        Row::of(lit!("| verdict | **"))
            .val(verdict_name)
            .fixed(lit!("** |"))
            .done(),
    );
    lines.add(
        Row::of(lit!("| 측정 decode t/s (동시성1) | "))
            .text(&na_with_suffix(
                get(verdict, "measured_decode_tps")?,
                &PyValue::str(" t/s"),
            ))
            .fixed(lit!(" |"))
            .done(),
    );
    lines.add(
        Row::of(lit!("| 루브릭 권한 | "))
            .text(&na(get(rubric, "authority")?))
            .fixed(lit!(" |"))
            .done(),
    );
    lines.add(
        Row::of(lit!("| 루브릭 primary | "))
            .text(&na_with_suffix(
                get(rubric, "primary")?,
                &PyValue::str(" t/s"),
            ))
            .fixed(lit!(" ("))
            .text(&na(get(rubric, "source")?))
            .fixed(lit!(") |"))
            .done(),
    );
    lines.add(
        Row::of(lit!("| floor (primary×(1−tol)) | "))
            .text(&na_with_suffix(
                get(rubric, "floor")?,
                &PyValue::str(" t/s"),
            ))
            .fixed(lit!(" |"))
            .done(),
    );
    lines.add(
        Row::of(lit!("| ratio (M/primary) | "))
            .text(&na(get(rubric, "ratio_M_over_primary")?))
            .fixed(lit!(" |"))
            .done(),
    );
    lines.add(
        Row::of(lit!("| tolerance | "))
            .text(&na(get(rubric, "tolerance")?))
            .fixed(lit!(" |"))
            .done(),
    );
    lines.add(
        Row::of(lit!("| E-search 상태 | "))
            .text(&na(get(verdict, "e_search")?))
            .fixed(lit!(" |"))
            .done(),
    );
    let warning = get(verdict, "warning")?;
    if warning.is_truthy() {
        lines.add(
            Row::of(lit!("| ⚠ warning | "))
                .val(warning)
                .fixed(lit!(" |"))
                .done(),
        );
    }
    let balance = get(verdict, "balance")?;
    if balance.is_truthy() {
        let gates = get(balance, "gates_verdict")?.is_truthy();
        let state = match (get(balance, "pass")?.is_truthy(), gates) {
            (true, true) => "PASS",
            (true, false) => "정상(서술)",
            (false, true) => "REFUTE",
            (false, false) => "편차 초과(서술 — explore 에서 게이트 아님)",
        };
        lines.add(
            Row::of(lit!("| 노드간 VRAM 밸런스 | dev="))
                .text(&na(get(balance, "balance_dev")?))
                .fixed(lit!(" (tol="))
                .text(&na(get(balance, "tolerance")?))
                .fixed(lit!(") → "))
                .lit(state)
                .fixed(lit!(" |"))
                .done(),
        );
    }
    lines.add(lit!(""));
    let is_refute = matches!(verdict_name.peel(), PyValue::Str(text) if text.equals("REFUTE"));
    if is_refute && get(verdict, "refuted_claims")?.is_truthy() {
        lines.add(lit!("**기각 사유(사람 참고):**"));
        for claim in iterate(&subscript(verdict, "refuted_claims")?.clone())? {
            lines.add(
                Row::of(lit!("- "))
                    .val(get_or(&claim, "claim", &WORD_EMPTY)?)
                    .fixed(lit!(" — "))
                    .val(get_or(&claim, "reason", &WORD_EMPTY)?)
                    .done(),
            );
        }
        for hint in iterate(get_or(verdict, "diagnosis_hint", empty_list())?)? {
            lines.add(Row::of(lit!("  - 진단 힌트: ")).val(&hint).done());
        }
        lines.add(lit!(""));
    }

    let lite = get(index, "lite")?;
    lines.add(lit!("## lite 지표 (full ⊇ lite — 열 집합 중첩 보장)"));
    lines.add(lit!(""));
    if is_dict(lite) && get(lite, "table")?.is_truthy() {
        lines.add(lit!("> full 런이 lite 를 **포함해서 실행**한 결과다(별도 재측정 아님). cold-start TTFT 는 warmup 0 단일요청 측정이라 스윕(warmup 有)이 잴 수 없는 축이다."));
        lines.add(lit!(""));
        lines.raw(subscript(lite, "table")?.clone());
    } else if is_dict(lite) && get(lite, "error")?.is_truthy() {
        lines.add(
            Row::of(lit!("> ⚠ lite 산정 실패: "))
                .val(subscript(lite, "error")?)
                .fixed(lit!(" — 이 report 는 lite 열이 **결손**이다."))
                .done(),
        );
    } else {
        lines.add(lit!("> ⚠ **lite 미포함** — 이 full 런은 lite 를 수집하지 못했다(열 집합 결손). lite 만 돈 다른 모델과 직접 비교하지 마라."));
    }
    lines.add(lit!(""));

    lines.add(lit!("## 부하 스윕 곡선 (client-load · reload 없음)"));
    lines.add(lit!(""));
    lines.add(lit!("> 단일 running serve 에 동시 요청 수만 변화(동시성 축). **동시성=1 행이 판정점**(verdict 재사용). request-rate=inf(각 레벨 포화 측정). config-space(batch×maxlen)는 이 스윕 밖(Max 모드/explorer 소관)."));
    lines.add(lit!(""));
    lines.add(lit!(
        "| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |"
    ));
    lines.add(lit!("|---|---|---|---|---|---|---|"));
    let verdict_point = get(index, "verdict_point_level")?;
    for level in list_of(index, "levels")? {
        let measured = get_or_else(&level, "measured", empty_dict())?;
        let level_name = get(&level, "level")?;
        let marker = if equals(get(&level, "level")?, verdict_point) {
            " ★판정점"
        } else {
            ""
        };
        lines.add(
            Row::of(lit!("| "))
                .val(level_name)
                .lit(marker)
                .fixed(lit!(" | "))
                .text(&na(get(measured, "decode_tps")?))
                .fixed(lit!(" | "))
                .text(&na(get(measured, "output_throughput")?))
                .fixed(lit!(" | "))
                .text(&na(get(measured, "total_token_throughput")?))
                .fixed(lit!(" | "))
                .text(&na(get(measured, "ttft_ms_median")?))
                .fixed(lit!(" | "))
                .text(&na(get(measured, "itl_ms_median")?))
                .fixed(lit!(" | "))
                .text(&na(get(measured, "completed")?))
                .fixed(lit!("/"))
                .text(&na(get(measured, "failed")?))
                .fixed(lit!(" |"))
                .done(),
        );
    }
    lines.add(lit!(""));
    let truncated = get_or(index, "truncated", empty_list())?;
    if truncated.is_truthy() {
        lines.add(lit!(
            "**⚠ 절삭된 부하 레벨(적응 상한 클램프 — 조용히 자르지 않음):**"
        ));
        for item in iterate(truncated)? {
            lines.add(Row::of(lit!("- ")).val(&item).done());
        }
        lines.add(lit!(""));
    } else {
        lines.add(lit!("_절삭된 레벨 없음(요청 전 레벨 완주)._"));
        lines.add(lit!(""));
    }

    lines.extend(repetition_section(
        index,
        bench_mode_record,
        bench_mode_status,
    )?);

    lines.add(lit!(
        "## 루프라인 컨텍스트 (결정론 상한 — 의심 임계, SLA 아님)"
    ));
    lines.add(lit!(""));
    let roofline_map = if roofline.is_truthy() {
        roofline
    } else {
        empty_dict()
    };
    lines.add(lit!("| 지표 | 값 |"));
    lines.add(lit!("|---|---|"));
    let tps = PyValue::str(" t/s");
    lines.add(
        Row::of(lit!("| R_fp (forward-pass/s 상한, 100% MBU) | "))
            .text(&na_with_suffix(
                first_truthy(rubric, roofline_map, "R_fp")?,
                &tps,
            ))
            .fixed(lit!(" |"))
            .done(),
    );
    lines.add(
        Row::of(lit!("| R_token (spec, accept_len×R_fp) | "))
            .text(&na_with_suffix(
                first_truthy(rubric, roofline_map, "R_token")?,
                &tps,
            ))
            .fixed(lit!(" |"))
            .done(),
    );
    lines.add(
        Row::of(lit!("| expected_achievable (루프라인×MBU) | "))
            .text(&na_with_suffix(
                first_truthy(rubric, roofline_map, "expected_achievable")?,
                &tps,
            ))
            .fixed(lit!(" |"))
            .done(),
    );
    lines.add(
        Row::of(lit!("| spec(MTP) on (측정) | "))
            .text(&na(get(verdict, "measured_spec_on")?))
            .fixed(lit!(" (accept_len="))
            .text(&na(get(verdict, "measured_accept_len")?))
            .fixed(lit!(") |"))
            .done(),
    );
    lines.add(lit!(""));

    lines.add(lit!("## 측정 환경 스냅샷"));
    lines.add(lit!(""));
    lines.add(lit!("| 키 | 값 |"));
    lines.add(lit!("|---|---|"));
    for key in SNAPSHOT_KEYS {
        let value = get(meta, key)?;
        let labelled;
        let shown = if key == "model_path" {
            labelled = label_path(value, path_labels)?;
            &labelled
        } else {
            value
        };
        lines.add(
            Row::of(lit!("| "))
                .lit(key)
                .fixed(lit!(" | "))
                .text(&na(shown))
                .fixed(lit!(" |"))
                .done(),
        );
    }
    lines.add(
        Row::of(lit!("| 입력 길이(sweep) | "))
            .text(&na(get(index, "input_len")?))
            .fixed(lit!(" |"))
            .done(),
    );
    lines.add(lit!(""));
    lines.add(lit!("---"));
    lines.add(lit!("_결정론 렌더 = `render_report.py`(LLM 표저작 ✗) · 참조 체인: bench JSON → 이 report → testlog(판정) → devlog(서사)._"));
    lines.into_document()
}

// ── the lite report ───────────────────────────────────────────────────────────

/// `lite_measurement_config()`: the configuration values of a cell declared lite-only (the record's shape is the
/// classification owner's).
pub fn lite_measurement_config() -> Res<PyValue> {
    let record = declared_lite_record(&PyValue::str(LITE_DECLARED_DETAIL))?;
    let mut values = PyDict::new();
    put(
        &mut values,
        "bench_mode",
        subscript(&record, "bench_mode")?.clone(),
    );
    put(
        &mut values,
        "bench_mode_kind",
        PyValue::str(bench_mode_kind(&record)),
    );
    put(
        &mut values,
        "bench_mode_source",
        subscript(&record, "bench_mode_source")?.clone(),
    );
    put(
        &mut values,
        "downgrade_reason",
        subscript(&record, "downgrade_reason")?.clone(),
    );
    put(
        &mut values,
        "downgrade_reason_source",
        subscript(&record, "downgrade_reason_source")?.clone(),
    );
    put(&mut values, "bench_tool", PyValue::str(LITE_BENCH_TOOL));
    put(&mut values, "bench_tool_version", PyValue::None);
    put(
        &mut values,
        "bench_tool_version_source",
        PyValue::str(LITE_BENCH_TOOL_VERSION_SOURCE),
    );
    put(&mut values, "repeats", PyValue::int(1));
    put(
        &mut values,
        "repeats_source",
        PyValue::str(LITE_REPEATS_SOURCE),
    );
    put(&mut values, "repeats_completed", PyValue::int(1));
    put(&mut values, "dataset_kind", PyValue::str("synthetic"));
    Ok(PyValue::Dict(values))
}

/// The lite report: the judgement the raw reading carries (moved, not re-decided), the configuration table, the
/// metrics table exactly as the lite owner rendered it, and the environment snapshot of the naming identity.
pub fn build_lite_md(raw: &PyValue, lite: &PyValue, identity: &PyValue) -> Res<PyStr> {
    let mut lines = Lines::default();
    lines.add(
        Row::of(lit!("# 경량 성능 보고서(lite) — `"))
            .val(subscript(identity, "model")?)
            .fixed(lit!("` @ "))
            .val(subscript(identity, "gpu_model")?)
            .fixed(lit!(" · vLLM "))
            .val(subscript(identity, "vllm_version")?)
            .done(),
    );
    lines.add(lit!(""));
    lines.add(lit!("mode: lite"));
    lines.add(lit!(""));
    lines.add(
        Row::of(lit!("> ⚠ **CARRY-FORWARD 재검증**: 이 보고서는 아래 '측정 환경' 한정의 **lite 스냅샷**이다(그때-그 HW/config). 성능 판정·루프라인·동시성 곡선·반복이 **없다** — lite 는 verdict_rule 에 투입하지 않는다. 성능 baseline 으로 승격하지 말고 소비 전 재측정할 것. 생성일 "))
            .val(get(raw, "measured_utc")?)
            .fixed(lit!("."))
            .done(),
    );
    lines.add(lit!(""));
    lines.add(lit!("## lite 판정 (성립 · 성능 판정 아님)"));
    lines.add(lit!(""));
    lines.add(lit!("| 키 | 값 |"));
    lines.add(lit!("|---|---|"));
    lines.add(
        Row::of(lit!("| lite_verdict | "))
            .text(&na(get(raw, "lite_verdict")?))
            .fixed(lit!(" |"))
            .done(),
    );
    for reason in iterate(get_or_else(raw, "lite_verdict_reasons", empty_list())?)? {
        lines.add(
            Row::of(lit!("| 사유 | "))
                .text(&replace_text(&display(&reason), '|', "/"))
                .fixed(lit!(" |"))
                .done(),
        );
    }
    lines.add(lit!(""));
    lines.extend(measurement_config_section(&lite_measurement_config()?)?);
    lines.add(lit!("## lite 지표 (서빙 성공 직후 스냅샷 · inform-only)"));
    lines.add(lit!(""));
    lines.add(
        Row::of(lit!("> cold(warmup 0 단일요청 → cold TTFT) 1회 + warm burst(N="))
            .text(&na(get(raw, "burst_n")?))
            .fixed(lit!(" · conc=1 · warmup 1) 1회. 산정·표 렌더는 `lite_metrics.py`(결정론)이며 이 문서는 그 표를 옮기지 않고 그대로 싣는다. 결측은 `N/A`(대체값 ✗)."))
            .done(),
    );
    lines.add(lit!(""));
    let table = get(lite, "table")?;
    if table.is_truthy() {
        lines.raw(table.clone());
    } else {
        lines.add(lit!(
            "> ⚠ lite 표 산정 실패 — 이 리포트는 lite 지표가 **결손**이다."
        ));
    }
    lines.add(lit!(""));
    lines.add(lit!("## 측정 환경 스냅샷"));
    lines.add(lit!(""));
    lines.add(lit!("| 키 | 값 |"));
    lines.add(lit!("|---|---|"));
    for key in [
        "model",
        "model_source",
        "serving_config",
        "gpu_model",
        "vllm_version",
        "vllm_version_source",
        "vllm_build",
        "topology",
        "quantization",
        "quantization_source",
        "tensor_parallel_size",
    ] {
        lines.add(
            Row::of(lit!("| "))
                .lit(key)
                .fixed(lit!(" | "))
                .text(&table_cell(get(identity, key)?))
                .fixed(lit!(" |"))
                .done(),
        );
    }
    for key in ["burst_n", "backend", "endpoint", "gen_src"] {
        let source = if key == "gen_src" { lite } else { raw };
        lines.add(
            Row::of(lit!("| "))
                .lit(key)
                .fixed(lit!(" | "))
                .text(&table_cell(get(source, key)?))
                .fixed(lit!(" |"))
                .done(),
        );
    }
    lines.add(
        Row::of(lit!("| lite_accept_len | "))
            .text(&table_cell(get(lite, "accept_len")?))
            .fixed(lit!(" |"))
            .done(),
    );
    lines.add(
        Row::of(lit!("| lite_accept_len_src | "))
            .text(&table_cell(get(lite, "accept_len_src")?))
            .fixed(lit!(" |"))
            .done(),
    );
    lines.add(lit!(""));
    lines.add(lit!("---"));
    lines.add(lit!("_결정론 렌더 = `render_report.py --lite-only`(LLM 표저작 ✗) · 참조 체인: lite raw → 이 report → testlog(판정) → devlog(서사)._"));
    lines.into_document()
}

fn replace_text(text: &PyStr, from: char, to: &str) -> PyStr {
    let mut out = PyStr::default();
    let from = u32::from(from);
    let mut run: Vec<u32> = Vec::new();
    for &point in text.code_points() {
        if point == from {
            out.push_py(&PyStr::from_code_points(std::mem::take(&mut run)));
            out.push_str(to);
        } else {
            run.push(point);
        }
    }
    out.push_py(&PyStr::from_code_points(run));
    out
}

// ── lite identity ─────────────────────────────────────────────────────────────

fn identity_failure(message: &str) -> Failure {
    Failure::new(LITE_ONLY_ERROR_CLASS, message)
}

fn word_character(code_point: u32) -> bool {
    char::from_u32(code_point).is_some_and(|c| c == '_' || c.is_alphanumeric())
}

fn ascii_digits(text: &[u32], from: usize) -> usize {
    text[from..]
        .iter()
        .take_while(|&&p| (u32::from('0')..=u32::from('9')).contains(&p))
        .count()
}

/// `[0-9]+\.[0-9]+\.[0-9]+` at `at`: the end of the match.
fn dotted_triple(text: &[u32], at: usize) -> Option<usize> {
    let mut position = at;
    for part in 0..3 {
        let digits = ascii_digits(text, position);
        if digits == 0 {
            return None;
        }
        position += digits;
        if part < 2 {
            if text.get(position) != Some(&u32::from('.')) {
                return None;
            }
            position += 1;
        }
    }
    Some(position)
}

fn starts_at(text: &[u32], at: usize, literal: &str) -> bool {
    let literal: Vec<u32> = literal.chars().map(u32::from).collect();
    text.len() >= at && text[at..].starts_with(&literal)
}

/// The engine-reported build: `\bv(X.Y.Z...)` up to a blank or `)` — the first such word in the log.
fn engine_build(log: &[u32]) -> Option<PyStr> {
    for start in 0..log.len() {
        if log[start] != u32::from('v') || (start > 0 && word_character(log[start - 1])) {
            continue;
        }
        if let Some(triple_end) = dotted_triple(log, start + 1) {
            let end = triple_end
                + log[triple_end..]
                    .iter()
                    .take_while(|&&p| p != u32::from(')') && !crate::pystr::is_python_space(p))
                    .count();
            return Some(PyStr::from_code_points(log[start + 1..end].to_vec()));
        }
    }
    None
}

/// `easy-vllm:X.Y.Z` anywhere in the text.
fn image_tag_version(text: &[u32]) -> Option<PyStr> {
    (0..text.len())
        .filter(|&at| starts_at(text, at, "easy-vllm:"))
        .find_map(|at| {
            let from = at + "easy-vllm:".len();
            dotted_triple(text, from).map(|end| PyStr::from_code_points(text[from..end].to_vec()))
        })
}

/// `vLLM\s*X.Y.Z` anywhere in the text.
fn config_comment_version(text: &[u32]) -> Option<PyStr> {
    (0..text.len())
        .filter(|&at| starts_at(text, at, "vLLM"))
        .find_map(|at| {
            let mut from = at + 4;
            while from < text.len() && crate::pystr::is_python_space(text[from]) {
                from += 1;
            }
            dotted_triple(text, from).map(|end| PyStr::from_code_points(text[from..end].to_vec()))
        })
}

/// The `quantization=` value of the engine's config line: `Initializing a V1 LLM engine ... with config: ...`.
fn engine_quantization(log: &[u32]) -> Option<PyStr> {
    let config = (0..log.len())
        .filter(|&at| starts_at(log, at, "Initializing a V1 LLM engine"))
        .find_map(|at| {
            let line_end = log[at..]
                .iter()
                .position(|&p| p == 10)
                .map_or(log.len(), |o| at + o);
            let from = at + "Initializing a V1 LLM engine".len();
            (from..line_end)
                .find(|&candidate| starts_at(&log[..line_end], candidate, "with config:"))
                .map(|candidate| &log[candidate + "with config:".len()..line_end])
        })?;
    let key: Vec<u32> = "quantization=".chars().map(u32::from).collect();
    let is_boundary = |p: u32| {
        crate::pystr::is_python_space(p)
            || [',', '(', '{', '['].contains(&char::from_u32(p).unwrap_or('\0'))
    };
    for at in 0..config.len() {
        let value_at = if at == 0 && config.starts_with(&key) {
            Some(key.len())
        } else if is_boundary(config[at]) && config[at + 1..].starts_with(&key) {
            Some(at + 1 + key.len())
        } else {
            None
        };
        if let Some(from) = value_at {
            let stop = |p: u32| {
                crate::pystr::is_python_space(p)
                    || [',', ')', ']', '}'].contains(&char::from_u32(p).unwrap_or('\0'))
            };
            let run = config[from..].iter().take_while(|&&p| !stop(p)).count();
            if run > 0 {
                return Some(PyStr::from_code_points(config[from..from + run].to_vec()));
            }
        }
    }
    None
}

/// `str.strip(chars)` for the two quote characters.
fn strip_both_quotes(text: &PyStr) -> PyStr {
    let points = text.code_points();
    let is_quote = |p: &u32| *p == u32::from('"') || *p == u32::from('\'');
    let start = points
        .iter()
        .position(|p| !is_quote(p))
        .unwrap_or(points.len());
    let end = points
        .iter()
        .rposition(|p| !is_quote(p))
        .map_or(start, |i| i + 1);
    PyStr::from_code_points(points[start..end.max(start)].to_vec())
}

/// The number of `^\s*-\s*role:\s*` matches (a list item with a `role:` key) in a manifest.
fn role_item_count(text: &[u32]) -> usize {
    let mut count = 0;
    let mut position = 0;
    while position <= text.len() {
        if position > 0 && text[position - 1] != 10 {
            position += 1;
            continue;
        }
        let mut at = position;
        let space = |at: &mut usize| {
            while *at < text.len() && crate::pystr::is_python_space(text[*at]) {
                *at += 1;
            }
        };
        space(&mut at);
        if text.get(at) == Some(&u32::from('-')) {
            at += 1;
            space(&mut at);
            if starts_at(text, at, "role:") {
                at += 5;
                space(&mut at);
                count += 1;
                position = at;
                continue;
            }
        }
        position += 1;
    }
    count
}

fn lower(text: &PyStr) -> PyStr {
    match text.to_utf8() {
        Some(utf8) => py(&utf8.to_lowercase()),
        None => text.clone(),
    }
}

fn or_text(value: Option<PyStr>, fallback: &str) -> PyValue {
    PyValue::Str(
        value
            .filter(|text| !text.is_empty())
            .unwrap_or_else(|| py(fallback)),
    )
}

/// Where `lite_identity` finds environment variables: the process, or an injected mapping.
pub enum Environment<'a> {
    Process,
    Mapping(&'a PyValue),
}

impl Environment<'_> {
    fn get(&self, name: &str) -> Res<PyValue> {
        match self {
            Environment::Process => Ok(std::env::var_os(name).map_or(PyValue::None, |value| {
                use std::os::unix::ffi::OsStrExt;
                PyValue::Str(PyStr::from_surrogateescape(value.as_bytes()))
            })),
            Environment::Mapping(map) => get(map, name).cloned(),
        }
    }
}

/// The naming and snapshot identity of a lite run, derived from the files its raw reading points at: the model axis
/// (checkpoint basename, lower-cased), the GPU (manifest), the vLLM version (injected environment, image tag,
/// engine log, config comment — first that has one), quantization, tensor parallelism and measuring node. Each
/// value carries a `*_source` saying where it came from. A run whose manifest has no GPU, or that has no model
/// input at all, is refused with `LiteIdentityError` instead of being named `NA`.
pub fn lite_identity(raw: &PyValue, environment: &Environment<'_>) -> Res<PyValue> {
    let config_name = {
        let value = get(raw, "config_name")?;
        if value.is_truthy() {
            display(value).strip()
        } else {
            PyStr::default()
        }
    };
    let config_text = read_text_or_empty(get(raw, "config_yaml")?)?;
    let env_text = read_text_or_empty(get(raw, "env_file")?)?;
    let manifest_text = read_text_or_empty(get(raw, "manifest")?)?;
    let engine_text = read_text_or_empty(get(raw, "engine_log")?)?;
    let Some(gpu_model) = grep_yaml(&manifest_text, &lit!("gpu_model")).filter(|g| !g.is_empty())
    else {
        return Err(identity_failure(&format!(
            "manifest({}) 에 gpu_model 이 없다 — 리포트 이름의 gpu 축을 NA 로 짓지 않는다",
            display(get(raw, "manifest")?).to_utf8().unwrap_or_default()
        )));
    };
    let model_path = grep_yaml(&config_text, &lit!("model")).unwrap_or_default();
    let mut trimmed = model_path.code_points().to_vec();
    while trimmed.last() == Some(&u32::from('/')) {
        trimmed.pop();
    }
    let base = crate::ospath::basename(&PyStr::from_code_points(trimmed)).strip();
    let serving_name = grep_env(&env_text, &lit!("SERVING_MODEL_NAME"));
    let (model, model_source) = if !base.is_empty() {
        (
            lower(&base),
            lit!("derived(model_path basename · lowercased)"),
        )
    } else if let Some(name) = serving_name.clone() {
        (name, lit!("fallback(serving_model_name)"))
    } else if !config_name.is_empty() {
        (config_name.clone(), lit!("fallback(config_name)"))
    } else {
        return Err(identity_failure(
            "모델 축을 세울 입력이 없다(config yaml model · SERVING_MODEL_NAME · config_name 전부 부재)",
        ));
    };
    let vllm_build = engine_build(engine_text.code_points());
    let declared_version = environment.get("EASY_VLLM_VERSION")?;
    let image_tag = grep_env(&env_text, &lit!("IMAGE_TAG"));
    let mut version: Option<(PyValue, PyStr)> = None;
    if declared_version.is_truthy() {
        version = Some((declared_version, lit!("declared(env EASY_VLLM_VERSION)")));
    }
    if version.is_none() {
        if let Some(found) = image_tag
            .as_ref()
            .and_then(|tag| image_tag_version(tag.code_points()))
        {
            version = Some((PyValue::Str(found), lit!("derived(envfile IMAGE_TAG)")));
        }
    }
    if version.is_none() {
        if let Some(build) = &vllm_build {
            if let Some(end) = dotted_triple(build.code_points(), 0) {
                version = Some((
                    PyValue::Str(PyStr::from_code_points(build.code_points()[..end].to_vec())),
                    lit!("measured(engine log 자기보고)"),
                ));
            }
        }
    }
    let (vllm, vllm_source) = match version {
        Some(found) => found,
        None => match config_comment_version(config_text.code_points()) {
            Some(found) => (PyValue::Str(found), lit!("declared(config yaml comment)")),
            None => (
                PyValue::str("NA"),
                lit!("absent(env·envfile·엔진 로그·config 모두)"),
            ),
        },
    };
    let measured_quant =
        engine_quantization(engine_text.code_points()).map(|q| strip_both_quotes(&q.strip()));
    let declared_quant = grep_yaml(&config_text, &lit!("quantization")).filter(|q| !q.is_empty());
    let (quant, quant_source) = if let Some(measured) = measured_quant {
        let value = if measured.equals("None") {
            lit!("none")
        } else {
            lower(&measured)
        };
        (value, lit!("measured(engine log)"))
    } else if let Some(declared) = declared_quant {
        (declared, lit!("declared(config yaml)"))
    } else {
        (lit!("NA"), lit!("absent(both)"))
    };
    let topology = get(raw, "topology")?;
    let tensor_parallel = match grep_yaml(&config_text, &lit!("tensor-parallel-size")) {
        Some(value) => value,
        None => infer_tensor_parallel(&manifest_text, topology)?,
    };
    let self_role = grep_yaml(&manifest_text, &lit!("self_role"));
    let (node, node_source) = if matches!(topology.peel(), PyValue::Str(t) if t.equals("multi")) {
        (
            lit!("cluster"),
            lit!("derived(topology=multi — 쌍이 하나의 측정 정체성)"),
        )
    } else if self_role.as_ref().is_some_and(|r| r.equals("sub")) {
        (lit!("sub"), lit!("derived(manifest.self_role=sub)"))
    } else if let Some(role) = self_role.filter(|r| !r.is_empty()) {
        (
            lit!("main"),
            Row::of(lit!("derived(manifest.self_role="))
                .text(&role)
                .fixed(lit!(")"))
                .done(),
        )
    } else {
        (lit!("main"), lit!("defaulted(self_role absent)"))
    };
    let mut identity = PyDict::new();
    put(&mut identity, "model", PyValue::Str(model));
    put(&mut identity, "model_source", PyValue::Str(model_source));
    put(
        &mut identity,
        "serving_config",
        PyValue::Str(
            serving_name
                .filter(|s| !s.is_empty())
                .or_else(|| (!config_name.is_empty()).then(|| config_name.clone()))
                .unwrap_or_else(|| lit!("NA")),
        ),
    );
    put(&mut identity, "gpu_model", PyValue::Str(gpu_model.clone()));
    put(
        &mut identity,
        "gpu_key",
        PyValue::Str(gpu_key(&PyValue::Str(gpu_model))),
    );
    put(&mut identity, "vllm_version", vllm);
    put(
        &mut identity,
        "vllm_version_source",
        PyValue::Str(vllm_source),
    );
    put(&mut identity, "vllm_build", or_text(vllm_build, "NA"));
    put(&mut identity, "topology", topology.clone());
    put(&mut identity, "quantization", PyValue::Str(quant));
    put(
        &mut identity,
        "quantization_source",
        PyValue::Str(quant_source),
    );
    put(
        &mut identity,
        "tensor_parallel_size",
        PyValue::Str(tensor_parallel),
    );
    put(&mut identity, "measured_node", PyValue::Str(node));
    put(
        &mut identity,
        "measured_node_source",
        PyValue::Str(node_source),
    );
    put(&mut identity, "image_tag", or_text(image_tag, "NA"));
    for (key, source) in [
        ("max_model_len", "max-model-len"),
        ("max_num_seqs", "max-num-seqs"),
        ("kv_cache_memory_bytes", "kv-cache-memory-bytes"),
        ("gpu_memory_utilization", "gpu-memory-utilization"),
    ] {
        put(
            &mut identity,
            key,
            or_text(grep_yaml(&config_text, &py(source)), "NA"),
        );
    }
    Ok(PyValue::Dict(identity))
}

/// Tensor parallelism when the config does not state it: one node's GPUs on a single-node topology, the node
/// list times the GPUs per node otherwise; `1` whenever the manifest cannot say.
fn infer_tensor_parallel(manifest_text: &PyStr, raw_topology: &PyValue) -> Res<PyStr> {
    let gpus_per_node = grep_yaml(manifest_text, &lit!("gpus_per_node")).filter(|g| !g.is_empty());
    let topology = match grep_yaml(manifest_text, &lit!("topology")).filter(|t| !t.is_empty()) {
        Some(declared) => PyValue::Str(declared),
        None => raw_topology.clone(),
    };
    let shown = display(&topology).strip();
    let shown = {
        let points = shown.code_points();
        let start = points
            .iter()
            .position(|&p| p != u32::from('"'))
            .unwrap_or(points.len());
        let end = points
            .iter()
            .rposition(|&p| p != u32::from('"'))
            .map_or(start, |i| i + 1);
        PyStr::from_code_points(points[start..end.max(start)].to_vec())
    };
    let integer = |text: &PyStr| match int_from_str(text) {
        Ok(number) => Ok(Some(number)),
        Err(error) if error.is_type_or_value_error() => Ok(None),
        Err(error) => Err(Failure::from(error)),
    };
    let fallback = lit!("1");
    if shown
        .code_points()
        .starts_with(&"single".chars().map(u32::from).collect::<Vec<_>>())
    {
        return Ok(match gpus_per_node {
            Some(gpus) => match integer(&gpus)? {
                Some(count) => py(&count.max(1.into()).to_string()),
                None => fallback,
            },
            None => fallback,
        });
    }
    let nodes = role_item_count(manifest_text.code_points());
    Ok(match (nodes, gpus_per_node) {
        (0, _) | (_, None) => fallback,
        (nodes, Some(gpus)) => match integer(&gpus)? {
            Some(count) => py(&(num_bigint::BigInt::from(nodes.max(1)) * count).to_string()),
            None => fallback,
        },
    })
}

pub const DEFAULT_RECORD_STATUS: &str = ABSENT_RECORD_STATUS;

/// `manifest_path_labels(manifest_path)` as the value the original returns: a list of `(prefix, label)` tuples.
pub fn manifest_path_labels_value(manifest_path: &PyValue) -> Res<PyValue> {
    Ok(PyValue::List(
        crate::report_support::manifest_path_labels(manifest_path)?
            .into_iter()
            .map(|(prefix, label)| PyValue::Tuple(vec![PyValue::Str(prefix), PyValue::Str(label)]))
            .collect(),
    ))
}

/// `_label_path(value, labels)`.
pub fn label_path_value(value: &PyValue, labels: &PyValue) -> Res<PyValue> {
    label_path(value, labels)
}
