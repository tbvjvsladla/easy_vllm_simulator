//! The Max-mode report renderer: a port of `render_max_report.py`.
//!
//! Max mode steps the context length up until a level no longer serves and passes the smoke test; the last good
//! level is the *safe ceiling*. The report states that ceiling, one row per level, what was truncated above it and
//! the environment it holds for. It is deterministic, shows the ceiling without judging it (inform-only) and
//! carries the same carry-forward warning as the other reports: a different driver or hardware invalidates it.

use crate::pydyn::{as_get_target, iterate, join_values};
use crate::pystr::{lit, PyStr};
use crate::pyvalue::PyValue;
use crate::report_support::{display, na, py, Res};

const SNAPSHOT_KEYS: [&str; 9] = [
    "model",
    "gpu_model",
    "vllm_version",
    "topology",
    "tensor_parallel_size",
    "driver_version",
    "quantization",
    "moe_backend",
    "image_tag",
];

fn get(source: &PyValue, key: &str) -> Res<PyValue> {
    Ok(as_get_target(source)?
        .get(key)
        .cloned()
        .unwrap_or(PyValue::None))
}

fn get_or(source: &PyValue, key: &str, default: PyValue) -> Res<PyValue> {
    Ok(as_get_target(source)?.get(key).cloned().unwrap_or(default))
}

fn line(head: &str, parts: &[&PyStr], tail: &str) -> PyValue {
    let mut text = py(head);
    for part in parts {
        text.push_py(part);
    }
    text.push_str(tail);
    PyValue::Str(text)
}

/// The envelope report of one Max run (`max_index.json`) as markdown; it ends with a newline.
pub fn build_md(index: &PyValue) -> Res<PyStr> {
    let missing = || PyValue::str("NA");
    let meta = get_or(index, "meta", PyValue::Dict(crate::pyvalue::PyDict::new()))?;
    let model = get_or(&meta, "model", missing())?;
    let gpu = get_or(&meta, "gpu_model", missing())?;
    let vllm = get_or(&meta, "vllm_version", missing())?;
    let ceiling = get(index, "safe_ceiling_max_model_len")?;
    let mut lines: Vec<PyValue> = Vec::new();
    let mut add = |value: PyValue| lines.push(value);
    let text = |value: &str| PyValue::Str(py(value));
    add(line(
        "# Max 모드 — HW 안전-최대 컨텍스트 envelope: `",
        &[
            &display(&model),
            &lit!("` @ "),
            &display(&gpu),
            &lit!(" · vLLM "),
            &display(&vllm),
        ],
        "",
    ));
    add(text(""));
    add(line(
        "> ⚠ **CARRY-FORWARD 재검증**: 이 envelope 은 아래 환경 한정 특성화다(그때-그 HW·드라이버·이미지). 다른 세션·하드웨어·**드라이버 버전**에서 이 안전상한을 그대로 신뢰하지 말 것 — 재측정. **inform-only**(안전상한 특성화 표시 · 판정 게이트 아님). 별도 오퍼레이션(벤치마커 인프라 공유). 생성일 ",
        &[&display(&get_or(index, "generated_utc", PyValue::str("N/A"))?)],
        ".",
    ));
    add(text(""));
    add(text("## 안전-최대 컨텍스트 (핵심 결과)"));
    add(text(""));
    add(text("| 항목 | 값 |"));
    add(text("|---|---|"));
    add(line(
        "| **안전상한 max-model-len** | **",
        &[
            &na(&ceiling),
            &py(if ceiling.is_truthy() { " tokens" } else { "" }),
        ],
        "** |",
    ));
    add(line(
        "| 스텝업 축 | ",
        &[&na(&get(index, "axis")?)],
        " (안전측 오름차순) |",
    ));
    add(line(
        "| 원 config max-model-len | ",
        &[&na(&get(index, "original_max_model_len")?)],
        " |",
    ));
    add(text(""));
    add(text("> 안전상한 = **serve+smoke 를 통과한 최대 컨텍스트**(그 위 레벨은 실패/트립/미시도). 각 레벨은 재서빙(reload)+기능 스모크 + 협역 워치독/로드-전 RAM 게이트 관측으로 판정. **선-기록 후-위험**(레벨 결과는 다음 시도 전 기록)."));
    add(text(""));
    add(text("## 레벨별 결과 (컨텍스트 스텝업)"));
    add(text(""));
    add(text("| max-model-len | serve+smoke | 판정 |"));
    add(text("|---|---|---|"));
    for level in iterate(&get_or(index, "safe_levels", PyValue::List(Vec::new()))?)? {
        let exit = get(&level, "serve_smoke_exit")?;
        add(line(
            "| ",
            &[
                &na(&get(&level, "max_model_len")?),
                &lit!(" | exit "),
                &na(&exit),
            ],
            " | ✅ 안전 |",
        ));
    }
    add(text(""));
    let truncated = get_or(index, "truncated", PyValue::List(Vec::new()))?;
    if truncated.is_truthy() {
        add(text("**⚠ 안전상한 위 / 미시도 (silent truncation 금지):**"));
        for item in iterate(&truncated)? {
            add(line("- ", &[&display(&item)], ""));
        }
        add(text(""));
    } else {
        add(text("_요청 전 레벨이 안전(안전상한을 넘는 실패 없음 — 상한 위를 더 probe하려면 --levels 확장)._"));
        add(text(""));
    }
    add(text("## 측정 환경 스냅샷"));
    add(text(""));
    add(text("| 키 | 값 |"));
    add(text("|---|---|"));
    for key in SNAPSHOT_KEYS {
        add(line(
            "| ",
            &[&py(key), &lit!(" | "), &na(&get(&meta, key)?)],
            " |",
        ));
    }
    add(text(""));
    add(text("---"));
    add(text("_Max = 별도 오퍼레이션(벤치마커 측정 인프라만 공유 · native 모드 아님) · 결정론 렌더 `render_max_report.py`(LLM 표저작 ✗) · 안전 = 헌법 §호스트 안전체계 따름정리 · plan_26071510._"));
    let mut document = join_values("\n", &lines)?;
    document.push_str("\n");
    Ok(document)
}
