//! `rank_recipes.py`: the default candidate grid, the ranking of estimated candidates by the hard gate, and the report
//! table. Values are the dynamic JSON values the original handled; ordering is a stable sort of the passing results by
//! (headroom, max_model_len) descending, so equal keys keep their candidate order.

use crate::estimate_vram::estimate;
use crate::pyexpr::{big, float_of, get, get_or, int_of, is_none, items_of, mul, object, Res};
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pynumber::format_fixed;
use agent_runtime_support::pysort::sorted_by_key;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;
use num_bigint::BigInt;
use num_traits::{Signed, ToPrimitive};

/// `auto_candidates(parsed, tp)`: the deterministic grid (quant set x context lengths x utilizations), ids `r1..rN`.
/// `tp` is accepted and not used, as in the original.
pub fn auto_candidates(parsed: &PyValue, _tp: &PyValue) -> Res<PyValue> {
    let quant_set: Vec<PyValue> = if get(parsed, "prequantized")?.is_truthy() {
        vec![get(parsed, "quant_method_native")?]
    } else {
        vec![PyValue::str("none"), PyValue::str("fp8")]
    };
    let max_position = get(parsed, "max_position_embeddings")?;
    let max_lens: Vec<BigInt> = if max_position.is_truthy() {
        let cap = int_of(&max_position)?.min(big(131072));
        let mut lens = Vec::new();
        let mut length = big(4096);
        while length <= cap {
            lens.push(length.clone());
            length *= 2;
        }
        if lens.is_empty() {
            lens.push(cap);
        }
        lens
    } else {
        [4096, 8192, 16384, 32768].into_iter().map(big).collect()
    };
    let mut candidates = Vec::new();
    for quant in &quant_set {
        for length in &max_lens {
            for gmu in [0.85, 0.90, 0.95] {
                let number = candidates.len() + 1;
                candidates.push(PyValue::Dict(object(vec![
                    ("id", PyValue::str(&format!("r{number}"))),
                    ("quantization", quant.clone()),
                    ("max_model_len", PyValue::Int(length.clone())),
                    ("gpu_memory_utilization", PyValue::Float(gmu)),
                ])));
            }
        }
    }
    Ok(PyValue::List(candidates))
}

/// The sort key of a passing result: the tuple `(headroom_gib, max_model_len)` with the original's defaults.
fn rank_key(result: &PyValue) -> Res<PyValue> {
    Ok(PyValue::Tuple(vec![
        get_or(result, "headroom_gib", PyValue::Float(f64::NEG_INFINITY))?,
        get_or(result, "max_model_len", PyValue::int(0))?,
    ]))
}

/// `rank(parsed, candidates, tp, budget_gib, margin, kv_bytes=2)`: estimate every candidate, split by the gate, and
/// order the passing ones by (headroom desc, max_model_len desc). Result `{ranked, failed, all}`; ids are kept.
pub fn rank(
    parsed: &PyValue,
    candidates: &PyValue,
    tp: &PyValue,
    budget_gib: &PyValue,
    margin: &PyValue,
    kv_bytes: &PyValue,
) -> Res<PyValue> {
    let mut all_results = Vec::new();
    for candidate in items_of(candidates)? {
        let result = estimate(parsed, &candidate, tp, budget_gib, margin, kv_bytes)?;
        all_results.push(result);
    }
    let passed = |result: &PyValue| get(result, "gate_pass").map(|flag| flag.is_truthy());
    let mut ranked = Vec::new();
    let mut failed = Vec::new();
    for result in &all_results {
        if passed(result)? {
            ranked.push(result.clone());
        }
    }
    for result in &all_results {
        if !passed(result)? {
            failed.push(result.clone());
        }
    }
    // `list.sort(key=..., reverse=True)`: reverse the list, run the ascending stable sort on the tuple keys with Python's own
    // `<` (a NaN is incomparable, so the result is whatever CPython's comparison schedule makes of it), reverse again.
    ranked.reverse();
    let mut keys = Vec::with_capacity(ranked.len());
    for result in &ranked {
        keys.push(rank_key(result)?);
    }
    let mut ranked = sorted_by_key(ranked, &keys)?;
    ranked.reverse();
    Ok(PyValue::Dict(object(vec![
        ("ranked", PyValue::List(ranked)),
        ("failed", PyValue::List(failed)),
        ("all", PyValue::List(all_results)),
    ])))
}

/// `_fmt_num(value, ndigits=2)`: `-` for `None`, the fixed-point number for anything `float()` accepts, otherwise the
/// value's own text. The formatting is inside the original's `try`, so a bad `ndigits` also gives the value's text.
pub fn fmt_num(value: &PyValue, ndigits: &PyValue) -> Res<PyStr> {
    if is_none(value) {
        return Ok(PyStr::new("-"));
    }
    let number = match float_of(value) {
        Ok(number) => number,
        Err(failure) if failure.class == "TypeError" || failure.class == "ValueError" => {
            return Ok(value.display_text());
        }
        Err(failure) => return Err(failure),
    };
    let places = match ndigits.peel() {
        PyValue::Int(count) => count.clone(),
        PyValue::Bool(flag) => BigInt::from(u8::from(*flag)),
        _ => return Ok(value.display_text()),
    };
    // The precision is a C int: outside its signed range is an `OverflowError` for either sign; a negative one in range
    // formats with no decimals.
    let places = match places.to_i32() {
        Some(places) => u32::try_from(places).unwrap_or(0),
        None => {
            return Err(Failure::new(
                "OverflowError",
                "Python int too large to convert to C int",
            ));
        }
    };
    Ok(PyStr::new(&format_fixed(number, places)))
}

fn fmt2(value: &PyValue) -> Res<PyStr> {
    fmt_num(value, &PyValue::int(2))
}

/// `"{:,}".format(number)`.
fn with_thousands(number: &BigInt) -> String {
    let digits = number.abs().to_string();
    let mut grouped = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    if number.is_negative() {
        format!("-{grouped}")
    } else {
        grouped
    }
}

/// `"%d" % value`. (Visible to the crate: the RAM gate formats its raw values with the same conversion.)
pub(crate) fn percent_d(value: &PyValue) -> Res<String> {
    match value.peel() {
        PyValue::Int(number) => Ok(number.to_string()),
        PyValue::Bool(flag) => Ok(u8::from(*flag).to_string()),
        PyValue::Float(number) => int_of(&PyValue::Float(*number)).map(|n| n.to_string()),
        other => Err(Failure::new(
            "TypeError",
            &format!(
                "%d format: a real number is required, not {}",
                other.type_name()
            ),
        )),
    }
}

/// `len(value)`.
fn length_of(value: &PyValue) -> Res<usize> {
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

/// `value[0]` of a truthy sequence (a list, a string or an object).
fn first_item(value: &PyValue) -> Res<PyValue> {
    match value.peel() {
        PyValue::List(items) | PyValue::Tuple(items) => Ok(items[0].clone()),
        PyValue::Str(text) => Ok(PyValue::Str(PyStr::from_code_points(vec![
            text.code_points()[0],
        ]))),
        PyValue::Dict(_) => Err(Failure::new("KeyError", "0")),
        other => Err(Failure::new(
            "TypeError",
            &format!("'{}' object is not subscriptable", other.type_name()),
        )),
    }
}

/// `"{:<width}".format(text)`.
fn padded(text: PyStr, width: usize) -> PyStr {
    let mut out = text;
    while out.len() < width {
        out.push_str(" ");
    }
    out
}

const WIDTHS: [usize; 7] = [5, 18, 8, 6, 13, 13, 7];

/// `row_fmt.format(*cells)`: the cells left-aligned in the seven column widths, two spaces apart.
fn row(cells: [PyStr; 7]) -> PyStr {
    let mut line = PyStr::default();
    for (index, cell) in cells.into_iter().enumerate() {
        if index > 0 {
            line.push_str("  ");
        }
        line.push_py(&padded(cell, WIDTHS[index]));
    }
    line
}

fn text_of(value: PyValue) -> PyStr {
    value.display_text()
}

/// One table row of a result (`_row` of the original).
fn result_row(result: &PyValue) -> Res<PyStr> {
    let mut verdict = PyStr::new(if get(result, "gate_pass")?.is_truthy() {
        "PASS"
    } else {
        "FAIL"
    });
    let error = get(result, "error")?;
    if get(result, "warning")?.is_truthy() {
        verdict.push_str("(!)");
    }
    let mut estimated = fmt2(&get(result, "estimated_total_gib")?)?;
    let headroom = fmt2(&get(result, "headroom_gib")?)?;
    if error.is_truthy() && estimated.equals("-") {
        estimated = PyStr::new("ERR");
    }
    let id = text_of(get_or(result, "id", PyValue::str("-"))?);
    let quantization = text_of(get_or(result, "quantization", PyValue::str("-"))?);
    let max_len = text_of(get_or(result, "max_model_len", PyValue::str("-"))?);
    let gmu = fmt2(&get(result, "gpu_memory_utilization")?)?;
    Ok(row([
        id,
        quantization,
        max_len,
        gmu,
        estimated,
        headroom,
        verdict,
    ]))
}

fn rule(character: &str, count: usize) -> PyStr {
    PyStr::new(&character.repeat(count))
}

fn line(parts: &[&PyStr], literal: &[&str]) -> PyStr {
    let mut out = PyStr::default();
    for (index, text) in literal.iter().enumerate() {
        out.push_str(text);
        if let Some(part) = parts.get(index) {
            out.push_py(part);
        }
    }
    out
}

/// `render_report(result, parsed, budget_gib, margin, tp)`: the header and the ranking table as text.
pub fn render_report(
    result: &PyValue,
    parsed: &PyValue,
    budget_gib: &PyValue,
    margin: &PyValue,
    tp: &PyValue,
) -> Res<PyStr> {
    let mut lines: Vec<PyStr> = Vec::new();
    lines.push(rule("=", 72));
    let model_id = get_or(parsed, "model_id", PyValue::str("?"))?.display_text();
    lines.push(line(&[&model_id], &["vLLM Recipe Ranking — "]));
    lines.push(rule("=", 72));

    let num_params = get(parsed, "num_params")?;
    let params = if is_none(&num_params) {
        "unknown".to_string()
    } else {
        with_thousands(&int_of(&num_params)?)
    };
    lines.push(line(
        &[&get_or(parsed, "model_id", PyValue::str("?"))?.display_text()],
        &["model_id     : "],
    ));
    lines.push(PyStr::new(&format!("num_params   : {params}")));
    let budget_text = fmt2(budget_gib)?;
    let margin_text = fmt2(margin)?;
    let gate_text = fmt2(&mul(budget_gib, margin)?)?;
    lines.push(line(
        &[&budget_text, &margin_text, &gate_text],
        &[
            "budget (GiB) : ",
            "   safety_margin: ",
            "   hard-gate: ",
            " GiB",
        ],
    ));
    lines.push(PyStr::new(&format!(
        "tensor_parallel_size: {}",
        percent_d(tp)?
    )));

    let warnings = get(parsed, "warnings")?;
    if warnings.is_truthy() {
        lines.push(PyStr::new("warnings:"));
        for warning in items_of(&warnings)? {
            lines.push(line(&[&warning.display_text()], &["  - "]));
        }
    }
    lines.push(rule("-", 72));

    let header = [
        "id",
        "quant",
        "max_len",
        "gmu",
        "est_vram_gib",
        "headroom_gib",
        "verdict",
    ]
    .map(PyStr::new);
    lines.push(row(header));
    lines.push(row(WIDTHS.map(|width| rule("-", width))));

    let ranked = get_or(result, "ranked", PyValue::List(Vec::new()))?;
    let failed = get_or(result, "failed", PyValue::List(Vec::new()))?;
    for item in items_of(&ranked)? {
        lines.push(result_row(&item)?);
    }
    if failed.is_truthy() {
        if ranked.is_truthy() {
            lines.push(PyStr::new(&format!("  {}", "-".repeat(68))));
        }
        for item in items_of(&failed)? {
            lines.push(result_row(&item)?);
        }
    }

    let mut warned = Vec::new();
    for item in items_of(&get_or(result, "all", PyValue::List(Vec::new()))?)? {
        if get(&item, "warning")?.is_truthy() {
            warned.push(item);
        }
    }
    if !warned.is_empty() {
        lines.push(rule("-", 72));
        lines.push(PyStr::new("recipe warnings (결정론 계층):"));
        for item in &warned {
            let id = get_or(item, "id", PyValue::str("-"))?.display_text();
            let quantization = get_or(item, "quantization", PyValue::str("-"))?.display_text();
            let warning = get(item, "warning")?.display_text();
            lines.push(line(
                &[&id, &quantization, &warning],
                &["  ! ", " quant=", ": "],
            ));
        }
        let mut offline = false;
        for item in &warned {
            if matches!(get(item, "warning")?.peel(), PyValue::Str(own) if own.equals("offline_quant_on_non_prequantized_checkpoint"))
            {
                offline = true;
                break;
            }
        }
        if offline {
            lines.push(PyStr::new(
                "    (awq/gptq 류는 사전양자화 체크포인트 필수 — 이 비prequantized 모델에 지정 시 `vllm serve` 실패 가능. fp8·bitsandbytes 는 온라인 가능.)",
            ));
        }
    }

    lines.push(rule("-", 72));
    let passed_count = length_of(&ranked)?;
    let total_count = length_of(&get_or(result, "all", PyValue::List(Vec::new()))?)?;
    lines.push(PyStr::new(&format!(
        "PASS: {passed_count} / {total_count} candidates"
    )));
    if ranked.is_truthy() {
        let top = first_item(&ranked)?;
        let id = get(&top, "id")?.display_text();
        let quantization = get(&top, "quantization")?.display_text();
        let max_len = get(&top, "max_model_len")?.display_text();
        let gmu = fmt2(&get(&top, "gpu_memory_utilization")?)?;
        let headroom = fmt2(&get(&top, "headroom_gib")?)?;
        lines.push(line(
            &[&id, &quantization, &max_len, &gmu, &headroom],
            &[
                "top recipe: ",
                " (quant=",
                ", max_len=",
                ", gmu=",
                ", headroom=",
                " GiB)",
            ],
        ));
    } else {
        lines.push(PyStr::new(
            "top recipe: (none passed the hard-gate — relax budget/margin or quantize)",
        ));
    }
    lines.push(rule("=", 72));

    let mut report = PyStr::default();
    for (index, text) in lines.iter().enumerate() {
        if index > 0 {
            report.push_str("\n");
        }
        report.push_py(text);
    }
    Ok(report)
}
