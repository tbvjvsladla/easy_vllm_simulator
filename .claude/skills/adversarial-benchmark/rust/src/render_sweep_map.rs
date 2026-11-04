//! The Broad Search exploration map renderer: a port of `render_sweep_map.py`.
//!
//! The map **never ranks**. Cells are listed in execution order only, every axis (the whole concurrency vector,
//! capacity next to speed, repetition bands) is carried side by side, and folding axes into one number would be a
//! use decision that belongs to a person. The ban is enforced, not described: [`assert_no_ranking`] refuses any
//! output holding a ranking key at any depth, and the markdown is checked for ranking columns by the self-test.
//! An incomplete map is published too (`sweep_status: incomplete`), because hiding it would make "stopped
//! half-way" and "never ran" indistinguishable.
//!
//! The documents are dynamically typed JSON: missing fields stay `null`, and unexpected types fail the way the
//! original's Python operations would.

use crate::guidellm::Failure;
use crate::pydyn::{as_get_target, iterate, join, subscript};
use crate::pystr::{lit, PyStr};
use crate::pyvalue::{PyDict, PyValue};
use crate::report_support::{display, py, Res};
use std::sync::LazyLock;

pub const RENDER_ERROR: &str = "RenderError";

/// Keys no output may carry (a closed tripwire list; keys are compared lower-cased, values are not looked at).
pub const FORBIDDEN_KEYS: [&str; 12] = [
    "winner",
    "rank",
    "ranking",
    "best",
    "top",
    "score",
    "recommended",
    "recommendation",
    "champion",
    "leader",
    "dominates",
    "pareto_front",
];
/// Table headers that would be a ranking column.
pub const FORBIDDEN_HEADERS: [&str; 6] = ["순위", "1위", "추천", "winner", "rank", "best"];

fn render_error(message: &str) -> Failure {
    Failure::new(RENDER_ERROR, message)
}

fn lowercase(key: &PyStr) -> Option<String> {
    key.to_utf8().map(|text| text.to_lowercase())
}

/// Fails with `RenderError` when `payload` holds a ranking key anywhere (any depth, dict keys only).
///
/// The original recurses once per nested value (scalars included), one interpreter frame each, so a document nested
/// deeper than the interpreter's frame limit fails with `RecursionError` before anything is written.
/// `frames` is the frame number of this call: 3 when the program's `main()` calls it (module, `main`, this), 2 when
/// a script calls it directly.
pub fn assert_no_ranking_at(payload: &PyValue, path: &str, frames: usize) -> Res<()> {
    if frames > crate::json_stream::PYTHON_RECURSION_LIMIT {
        return Err(crate::json_stream::recursion_error());
    }
    match payload.peel() {
        PyValue::Dict(map) => {
            for (key, value) in map.iter() {
                if lowercase(key).is_some_and(|lower| FORBIDDEN_KEYS.contains(&lower.as_str())) {
                    return Err(render_error(&format!(
                        "순위 필드 금지 위반: {path}.{} — 이 지도는 순위를 만들지 않는다(§4.5). 축을 접는 것은 용처 결정이며 사람 몫이다.",
                        key.to_utf8().unwrap_or_default()
                    )));
                }
                assert_no_ranking_at(
                    value,
                    &format!("{path}.{}", key.to_utf8().unwrap_or_default()),
                    frames + 1,
                )?;
            }
        }
        PyValue::List(items) => {
            for (position, value) in items.iter().enumerate() {
                assert_no_ranking_at(value, &format!("{path}[{position}]"), frames + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// [`assert_no_ranking_at`] called directly by a script (frame 2).
pub fn assert_no_ranking(payload: &PyValue, path: &str) -> Res<()> {
    assert_no_ranking_at(payload, path, 2)
}

/// Output key and source key of each cell row entry after `order`; `axis_citation_present` is derived.
const ROW_SOURCES: [&str; 17] = [
    "cell_key",
    "coordinates",
    "cell_outcome",
    "concurrency_vector",
    "capacity",
    "verdict_narrative",
    "measurement",
    "repetition",
    "bench_mode",
    "bench_mode_source",
    "downgrade_reason",
    "downgrade_reason_source",
    "downgrade_correlation",
    "void_reason",
    "void_reason_source",
    "note",
    "axis_citation",
];
const ROW_KEY_NAMES: [&str; 19] = [
    "order",
    "cell_key",
    "coordinates",
    "cell_outcome",
    "concurrency_vector",
    "capacity",
    "verdict_narrative",
    "measurement",
    "repetition",
    "bench_mode",
    "bench_mode_source",
    "downgrade_reason",
    "downgrade_reason_source",
    "downgrade_correlation",
    "void_reason",
    "void_reason_source",
    "failure_note",
    "axis_citation",
    "axis_citation_present",
];
static ROW_KEYS: LazyLock<Vec<PyStr>> = LazyLock::new(|| {
    ROW_KEY_NAMES
        .iter()
        .map(|name| PyStr::from_static(PyStr::static_points(name)))
        .collect()
});

fn get(source: &PyValue, key: &str) -> PyValue {
    match source.peel() {
        PyValue::Dict(map) => map.get(key).cloned().unwrap_or(PyValue::None),
        _ => PyValue::None,
    }
}

fn get_checked(source: &PyValue, key: &str) -> Res<PyValue> {
    Ok(as_get_target(source)?
        .get(key)
        .cloned()
        .unwrap_or(PyValue::None))
}

/// The map document for a sweep state and its stop judgement: cells in execution order, nothing evaluated.
pub fn build(state: &PyValue, stop: &PyValue) -> Res<PyValue> {
    if !matches!(state.peel(), PyValue::Dict(_)) || !matches!(stop.peel(), PyValue::Dict(_)) {
        return Err(render_error(
            "state 와 stop-json 은 모두 JSON 객체여야 한다",
        ));
    }
    let cells = get(state, "cells");
    let PyValue::List(cells) = cells.peel() else {
        return Err(render_error("state.cells 는 리스트여야 한다"));
    };
    let mut rows: Vec<PyValue> = Vec::with_capacity(cells.len());
    let mut uncited: Vec<PyValue> = Vec::new();
    for (position, cell) in cells.iter().enumerate() {
        if !matches!(cell.peel(), PyValue::Dict(_)) {
            return Err(render_error(&format!("cells[{position}] 는 객체여야 한다")));
        }
        let mut row = PyDict::with_capacity(ROW_KEYS.len());
        row.insert(ROW_KEYS[0].clone(), PyValue::int(position as i64 + 1));
        for (key, source) in ROW_KEYS[1..].iter().zip(ROW_SOURCES) {
            row.insert(key.clone(), get(cell, source));
        }
        let cited = row.get("axis_citation").is_some_and(PyValue::is_truthy);
        if !cited {
            uncited.push(get(cell, "cell_key"));
        }
        row.insert(ROW_KEYS[ROW_KEYS.len() - 1].clone(), PyValue::Bool(cited));
        rows.push(PyValue::Dict(row));
    }
    let mut doc = PyDict::new();
    doc.set("schema_version", PyValue::int(1));
    doc.set("kind", PyValue::str("broad_search_sweep_map"));
    doc.set("sweep_id", get(state, "sweep_id"));
    for key in [
        "sweep_status",
        "stopped_by",
        "cells_planned",
        "cells_attempted",
        "cells_remaining",
        "consecutive_failures",
        "elapsed_s",
        "declared_budget",
        "budget_declared_by",
        "budget_basis",
        "budget_repeats_source",
        "runs_attempted",
        "repeats_mismatch",
        "repeats_unrecorded",
    ] {
        doc.set(key, get(stop, key));
    }
    doc.set("rubric_authority", get(state, "rubric_authority"));
    doc.set("control_variable", get(state, "control_variable"));
    doc.set("cells", PyValue::List(rows));
    doc.set("axis_citation_missing", PyValue::List(uncited));
    doc.set("ordering", PyValue::str("execution_order"));
    doc.set(
        "ordering_note",
        PyValue::str("실행 순서다. 성능 순서가 아니며 이 문서는 순위를 만들지 않는다(§4.5)."),
    );
    Ok(PyValue::Dict(doc))
}

/// `x or fallback` for display.
fn or_else(value: PyValue, fallback: &str) -> PyValue {
    if value.is_truthy() {
        value
    } else {
        PyValue::str(fallback)
    }
}

/// `na(value)`: `N/A` for `None`, otherwise the value itself.
fn na_value(value: PyValue) -> PyValue {
    if matches!(value.peel(), PyValue::None) {
        PyValue::str("N/A")
    } else {
        value
    }
}

/// `json.dumps(value, ensure_ascii=False)`.
fn dumps(value: &PyValue) -> Res<PyStr> {
    value.dumps_unicode().map_err(|error| {
        let text = error.to_string();
        Failure::new(
            error.python_type(),
            text.strip_prefix(&format!("{}: ", error.python_type()))
                .unwrap_or(&text),
        )
    })
}

/// `len(value)`.
fn length(value: &PyValue) -> Res<usize> {
    match value.peel() {
        PyValue::List(items) | PyValue::Tuple(items) => Ok(items.len()),
        PyValue::Dict(map) => Ok(map.len()),
        PyValue::Str(text) => Ok(text.len()),
        PyValue::Bytes(bytes) => Ok(bytes.len()),
        other => Err(Failure::new(
            "TypeError",
            &format!("object of type '{}' has no len()", other.type_name()),
        )),
    }
}

/// `"%d" % value`.
fn decimal(value: &PyValue) -> Res<PyStr> {
    match value.peel() {
        PyValue::Int(number) => Ok(py(&number.to_string())),
        PyValue::Bool(flag) => Ok(py(if *flag { "1" } else { "0" })),
        PyValue::Float(number) => {
            if number.is_nan() {
                return Err(Failure::new(
                    "ValueError",
                    "cannot convert float NaN to integer",
                ));
            }
            if number.is_infinite() {
                return Err(Failure::new(
                    "OverflowError",
                    "cannot convert float infinity to integer",
                ));
            }
            let integer = crate::pynumber::int_from_float(*number).map_err(Failure::from)?;
            Ok(py(&integer.to_string()))
        }
        other => Err(Failure::new(
            "TypeError",
            &format!(
                "%d format: a real number is required, not {}",
                other.type_name()
            ),
        )),
    }
}

struct Doc(Vec<PyStr>);

impl Doc {
    fn add(&mut self, line: PyStr) {
        self.0.push(line);
    }

    fn join_each(items: &PyValue, wrap: impl Fn(&PyValue) -> PyStr) -> Res<PyStr> {
        let parts: Vec<PyValue> = iterate(items)?
            .iter()
            .map(|item| PyValue::Str(wrap(item)))
            .collect();
        crate::pydyn::join_values(", ", &parts)
    }
}

fn cat(parts: &[&PyStr]) -> PyStr {
    let mut out = PyStr::default();
    for part in parts {
        out.push_py(part);
    }
    out
}

fn tick(value: &PyValue) -> PyStr {
    let mut out = lit!("`");
    out.push_py(&display(value));
    out.push_str("`");
    out
}

fn line(head: &str, value: &PyStr, tail: &str) -> PyStr {
    cat(&[&py(head), value, &py(tail)])
}

/// One line about the repetition axis of a cell: stops, missing records and undetermined modes each in their own
/// words (an empty cell would hide them).
fn repetition_line(row: &PyValue) -> Res<PyStr> {
    let repetition = get_checked(row, "repetition")?;
    let mode = get_checked(row, "bench_mode")?;
    let reason = get_checked(row, "downgrade_reason")?;
    let mode_text = if matches!(mode.peel(), PyValue::None) {
        line(
            "bench_mode 미확정(`",
            &display(&or_else(
                get_checked(row, "bench_mode_source")?,
                "기록 없음",
            )),
            "`)",
        )
    } else if reason.is_truthy() {
        cat(&[
            &lit!("bench_mode=`"),
            &display(&mode),
            &lit!("` · **강등** 사유 `"),
            &display(&reason),
            &lit!("` · 사살 대조 `"),
            &display(&get_checked(row, "downgrade_correlation")?),
            &lit!("` (출처 `"),
            &display(&get_checked(row, "downgrade_reason_source")?),
            &lit!("`)"),
        ])
    } else {
        cat(&[
            &lit!("bench_mode=`"),
            &display(&mode),
            &lit!("` · 사살 대조 `"),
            &display(&get_checked(row, "downgrade_correlation")?),
            &lit!("` (출처 `"),
            &display(&get_checked(row, "bench_mode_source")?),
            &lit!("`)"),
        ])
    };
    if !matches!(repetition.peel(), PyValue::Dict(_)) {
        return Ok(line("반복 축: 기록 없음 · ", &mode_text, ""));
    }
    let stop = get_checked(&repetition, "stop")?;
    let mut stop_text = PyStr::default();
    if matches!(stop.peel(), PyValue::Dict(_)) {
        let kind = get_checked(&stop, "kind")?;
        let what = if matches!(kind.peel(), PyValue::Str(text) if text.equals("repeat-break")) {
            "반복 중단"
        } else {
            "적응 상한 클램프"
        };
        stop_text = cat(&[
            &lit!(" · **스윕이 멈춘 자리 — "),
            &py(what),
            &lit!("**(level "),
            &display(&get_checked(&stop, "level")?),
            &lit!(" run "),
            &display(&get_checked(&stop, "run")?),
            &lit!(" — 남은 반복·상위 레벨 미측정)"),
        ]);
    }
    Ok(cat(&[
        &lit!("반복 축(요청 "),
        &display(&na_value(get_checked(&repetition, "requested")?)),
        &lit!(" · "),
        &display(&or_else(
            get_checked(&repetition, "requested_source")?,
            "출처 N/A",
        )),
        &lit!(" · "),
        &display(&or_else(get_checked(&repetition, "kind")?, "종류 N/A")),
        &lit!("): 레벨별 완주 `"),
        &dumps(&get_checked(&repetition, "completed_by_level")?)?,
        &lit!("` · 재현 밴드 %("),
        &display(&or_else(get_checked(&repetition, "band_formula")?, "N/A")),
        &lit!(") `"),
        &dumps(&get_checked(&repetition, "band_pct_by_level")?)?,
        &lit!("`"),
        &stop_text,
        &lit!(" · "),
        &mode_text,
    ]))
}

/// The map as markdown (deterministic text; it ends with a newline).
pub fn render_markdown(doc: &PyValue) -> Res<PyStr> {
    let mut out = Doc(Vec::new());
    out.add(line(
        "# 광의의 탐색 지도 — ",
        &display(&or_else(get_checked(doc, "sweep_id")?, "N/A")),
        "",
    ));
    out.add(lit!(""));
    out.add(lit!(
        "> **이 문서는 순위를 만들지 않는다.** 셀은 **실행 순서**로 나열되며 성능 순서가 아니다."
    ));
    out.add(lit!(
        "> 평가축이 여러 개이므로 상황에 맞는 레시피 선택과 의사결정은 사람이 한다 —"
    ));
    out.add(lit!(
        "> 여기 실린 것은 레시피별 **진실된 측정 결과**뿐이다."
    ));
    out.add(lit!(""));
    out.add(lit!("| 항목 | 값 |"));
    out.add(lit!("|---|---|"));
    out.add(line(
        "| 상태 | `",
        &display(&get_checked(doc, "sweep_status")?),
        "` |",
    ));
    let stopped_by = or_else_list(get_checked(doc, "stopped_by")?);
    let reasons = join(", ", &stopped_by)?;
    out.add(line(
        "| 정지 사유 | ",
        &if reasons.is_empty() {
            lit!("—")
        } else {
            reasons
        },
        " |",
    ));
    let remaining = or_else_list(get_checked(doc, "cells_remaining")?);
    out.add(cat(&[
        &lit!("| 셀 계획/시도/남음 | "),
        &display(&get_checked(doc, "cells_planned")?),
        &lit!(" / "),
        &display(&get_checked(doc, "cells_attempted")?),
        &lit!(" / "),
        &py(&length(&remaining)?.to_string()),
        &lit!(" |"),
    ]));
    out.add(line(
        "| 연속 실패 | ",
        &display(&get_checked(doc, "consecutive_failures")?),
        " |",
    ));
    out.add(line(
        "| 경과 | ",
        &display(&get_checked(doc, "elapsed_s")?),
        "s |",
    ));
    out.add(line(
        "| 통제변인 | ",
        &display(&or_else(get_checked(doc, "control_variable")?, "N/A")),
        " |",
    ));
    out.add(line(
        "| 루브릭 권한 | `",
        &display(&or_else(get_checked(doc, "rubric_authority")?, "N/A")),
        "` |",
    ));
    out.add(line(
        "| 선언 예산 | `",
        &dumps(&get_checked(doc, "declared_budget")?)?,
        "` |",
    ));
    out.add(line(
        "| 예산 승인 | ",
        &display(&or_else(get_checked(doc, "budget_declared_by")?, "N/A")),
        " |",
    ));
    out.add(line(
        "| 예산 근거 | ",
        &display(&or_else(get_checked(doc, "budget_basis")?, "N/A")),
        " |",
    ));
    out.add(line(
        "| 반복 수 출처 | ",
        &display(&or_else(get_checked(doc, "budget_repeats_source")?, "N/A")),
        " |",
    ));
    out.add(line(
        "| 레벨 run 시도 합 | ",
        &display(&na_value(get_checked(doc, "runs_attempted")?)),
        " |",
    ));
    out.add(lit!(""));
    let mismatch = get_checked(doc, "repeats_mismatch")?;
    if mismatch.is_truthy() {
        let joined = Doc::join_each(&subscript(doc, "repeats_mismatch")?.clone(), |entry| {
            cat(&[
                &lit!("`"),
                &display(&get(entry, "cell_key")),
                &lit!("`(요청 "),
                &display(&get(entry, "requested")),
                &lit!(" · 선언 "),
                &display(&get(entry, "declared_budget_repeats")),
                &lit!(")"),
            ])
        });
        // The original reads `m.get` on every entry: a non-dict entry is an AttributeError.
        for entry in iterate(&mismatch)? {
            as_get_target(&entry)?;
        }
        out.add(line(
            "⚠ **선언 예산과 다른 반복으로 잰 셀**: ",
            &joined?,
            "",
        ));
        out.add(lit!("> 예산 근거(벽시계 = 셀 × 레벨 × 반복)가 그 셀에서 달라졌다 — 정지 조건은 아니고 기재다."));
        out.add(lit!(""));
    }
    let declared = get_checked(doc, "declared_budget")?;
    let declared = if declared.is_truthy() {
        declared
    } else {
        PyValue::Dict(PyDict::new())
    };
    if matches!(
        as_get_target(&declared)?.get("repeats").map(PyValue::peel),
        Some(PyValue::None)
    ) {
        out.add(lit!("⚠ **예산 선언에 반복 수가 없다**(반복 축 신설 전 상태) — 벽시계 예산 근거에 반복 배수가 없고, 새 셀 진입은 거부된다."));
        out.add(lit!(""));
    }
    let unrecorded = get_checked(doc, "repeats_unrecorded")?;
    if unrecorded.is_truthy() {
        let joined = Doc::join_each(&unrecorded, tick)?;
        out.add(cat(&[
            &lit!("⚠ **반복 기록이 없는 측정 셀**: "),
            &joined,
            &lit!(" — 반복 축 신설 전 산출물이거나 집계가 실패했다(0 회로 읽지 말 것)."),
        ]));
        out.add(lit!(""));
    }
    if get_checked(doc, "cells_remaining")?.is_truthy() {
        let joined = Doc::join_each(&subscript(doc, "cells_remaining")?.clone(), tick)?;
        out.add(line("**미완이다.** 남은 셀: ", &joined, ""));
        out.add(lit!(""));
    }
    if get_checked(doc, "axis_citation_missing")?.is_truthy() {
        let joined = Doc::join_each(&subscript(doc, "axis_citation_missing")?.clone(), tick)?;
        out.add(line("⚠ **도서관 인용 누락 셀**: ", &joined, ""));
        out.add(lit!("> 인용 없는 축 판단은 거짓이 아니라 **누락**이다(헌법 불변식 B). 지도에 남겨 보이게 한다."));
        out.add(lit!(""));
    }
    out.add(lit!("## 셀 (실행 순서)"));
    out.add(lit!(""));
    for row in iterate(&or_else_list(get_checked(doc, "cells")?))? {
        out.add(cat(&[
            &lit!("### "),
            &decimal(subscript(&row, "order")?)?,
            &lit!(". `"),
            &display(subscript(&row, "cell_key")?),
            &lit!("` — `"),
            &display(subscript(&row, "cell_outcome")?),
            &lit!("`"),
        ]));
        out.add(lit!(""));
        out.add(line(
            "- 좌표: `",
            &dumps(&get_checked(&row, "coordinates")?)?,
            "`",
        ));
        out.add(line(
            "- 동시성 축(전 벡터): `",
            &dumps(&get_checked(&row, "concurrency_vector")?)?,
            "`",
        ));
        out.add(line(
            "- 용량 축(속도와 합치지 않는다): `",
            &dumps(&get_checked(&row, "capacity")?)?,
            "`",
        ));
        out.add(line(
            "- 측정 조건·도구: `",
            &dumps(&get_checked(&row, "measurement")?)?,
            "`",
        ));
        out.add(line("- ", &repetition_line(&row)?, ""));
        if get_checked(&row, "verdict_narrative")?.is_truthy() {
            out.add(line(
                "- 판정 서술: ",
                &display(subscript(&row, "verdict_narrative")?),
                "",
            ));
        }
        if get_checked(&row, "void_reason")?.is_truthy() {
            out.add(cat(&[
                &lit!("- 측정 무효 사인: `"),
                &display(subscript(&row, "void_reason")?),
                &lit!("` (출처 `"),
                &display(&get_checked(&row, "void_reason_source")?),
                &lit!("`)"),
            ]));
        }
        if get_checked(&row, "failure_note")?.is_truthy() {
            out.add(line(
                "- 비고: ",
                &display(subscript(&row, "failure_note")?),
                "",
            ));
        }
        out.add(line(
            "- 축 근거(도서관 인용): ",
            &display(&or_else(get_checked(&row, "axis_citation")?, "**누락**")),
            "",
        ));
        out.add(lit!(""));
    }
    let values: Vec<PyValue> = out.0.into_iter().map(PyValue::Str).collect();
    let mut text = crate::pydyn::join_values("\n", &values)?;
    text.push_str("\n");
    Ok(text)
}

/// `value or []`.
fn or_else_list(value: PyValue) -> PyValue {
    if value.is_truthy() {
        value
    } else {
        PyValue::List(Vec::new())
    }
}
