//! `estimate_vram.py`: the deterministic VRAM estimate and hard gate (`estimate`), the absolute-KV-clamp model
//! (`estimate_absolute` and its helpers) and the derived limits (`max_feasible_*`). Arithmetic keeps the original's
//! types and order (integers stay arbitrary-precision, floats are IEEE doubles, `int()` truncates), and every function
//! accepts the dynamic values the original accepted and fails with the exception it raised.

use crate::pyexpr::{
    big, compare, div_by_4096, f, float_div, float_of, float_of_int, get, get_or,
    int_div_power_of_two, int_of, is_none, item, mul, object, sub, text, Res,
};
use crate::quant_table::{
    is_offline_only_quant, vllm_quant_bpw, vllm_quant_bpw_or_none, FRAMEWORK_OVERHEAD_BYTES,
};
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pyops::Compare;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use num_bigint::BigInt;
use num_traits::{Signed, ToPrimitive, Zero};

/// `GIB = 1024 ** 3`.
pub const GIB: i64 = 1024 * 1024 * 1024;

/// `KV_BLOCK_ALIGN_BUFFER`: the 2% rounding buffer of the KV block alignment (single owner of the value).
pub const KV_BLOCK_ALIGN_BUFFER: f64 = 1.02;

fn framework_overhead() -> BigInt {
    big(FRAMEWORK_OVERHEAD_BYTES)
}

/// `kv_bearing_layers(parsed)`: how many layers hold KV cache (the hybrid model's full-attention layers when the count
/// is plausible, otherwise every layer); `None` when the total is unknown and no count applies.
pub fn kv_bearing_layers(parsed: &PyValue) -> Res<Option<BigInt>> {
    let total = get(parsed, "num_hidden_layers")?;
    let n_full = get(parsed, "num_full_attention_layers")?;
    let total_or_none = |total: &PyValue| -> Res<Option<BigInt>> {
        if is_none(total) {
            Ok(None)
        } else {
            int_of(total).map(Some)
        }
    };
    if is_none(&n_full) {
        return total_or_none(&total);
    }
    let n_full = match int_of(&n_full) {
        Ok(number) => number,
        Err(failure) if failure.class == "TypeError" || failure.class == "ValueError" => {
            return total_or_none(&total);
        }
        Err(failure) => return Err(failure),
    };
    if is_none(&total) {
        return Ok((n_full > BigInt::zero()).then_some(n_full));
    }
    let total = int_of(&total)?;
    if !(BigInt::zero() < n_full && n_full <= total) {
        return Ok(Some(total));
    }
    Ok(Some(n_full))
}

/// `kv_layers_source(parsed)`: which axis `kv_bearing_layers` used.
pub fn kv_layers_source(parsed: &PyValue) -> Res<&'static str> {
    let total = get(parsed, "num_hidden_layers")?;
    let total = if is_none(&total) {
        None
    } else {
        Some(int_of(&total)?)
    };
    Ok(if kv_bearing_layers(parsed)? == total {
        "num_hidden_layers"
    } else {
        "num_full_attention_layers"
    })
}

/// `_activation_bytes(eff_params, max_model_len)`: activation/scratch bytes (400 MB floor, 0.08 byte per parameter,
/// 150 MB per 4K of context), each term truncated by `int()` as the original does.
pub fn activation_bytes(eff_params: &PyValue, max_model_len: &PyValue) -> Res<BigInt> {
    let base = big(400_000_000);
    let param_term = int_of(&mul(eff_params, &f(0.08))?)?;
    let context = div_by_4096(max_model_len)? * 150_000_000.0;
    let ctx_term = int_of(&f(context))?;
    Ok(base + param_term + ctx_term)
}

/// The estimated overhead (activation plus framework) for `num_params` parameters at `max_model_len`.
///
/// The original converts `num_params` first and the context length second (`eff_params = int(num_params)`, then
/// `_activation_bytes(eff_params, int(length))`), so the length is produced by a closure that runs after the first
/// conversion: when both are bad, the first failure is the `num_params` one.
fn estimated_overhead(
    num_params: &PyValue,
    max_model_len: impl FnOnce() -> Res<PyValue>,
) -> Res<BigInt> {
    let eff_params = if is_none(num_params) {
        BigInt::zero()
    } else {
        int_of(num_params)?
    };
    Ok(activation_bytes(&PyValue::Int(eff_params), &max_model_len()?)? + framework_overhead())
}

fn result_skeleton(
    cand_id: PyValue,
    quantization: PyValue,
    max_model_len: &BigInt,
    gmu: f64,
) -> PyDict {
    object(vec![
        ("id", cand_id),
        ("quantization", quantization),
        ("max_model_len", PyValue::Int(max_model_len.clone())),
        ("gpu_memory_utilization", f(gmu)),
        ("weight_gib", PyValue::None),
        ("kv_gib", PyValue::None),
        ("overhead_gib", PyValue::None),
        ("estimated_total_gib", PyValue::None),
        ("headroom_gib", PyValue::None),
        ("gate_pass", PyValue::Bool(false)),
    ])
}

fn failed(mut result: PyDict, error: &str) -> PyValue {
    result.set("error", text(error));
    result.set("gate_pass", PyValue::Bool(false));
    PyValue::Dict(result)
}

/// `estimate(parsed, candidate, tp, budget_gib, safety_margin, kv_cache_dtype_bytes=2)`: the VRAM estimate and hard
/// gate of one candidate, `estimated_total = (weight_bytes/tp + KV + overhead) / gpu_memory_utilization`.
pub fn estimate(
    parsed: &PyValue,
    candidate: &PyValue,
    tp: &PyValue,
    budget_gib: &PyValue,
    safety_margin: &PyValue,
    kv_cache_dtype_bytes: &PyValue,
) -> Res<PyValue> {
    let cand_id = get(candidate, "id")?;
    let quantization = get(candidate, "quantization")?;
    let max_model_len = int_of(&item(candidate, "max_model_len")?)?;
    let gmu = float_of(&item(candidate, "gpu_memory_utilization")?)?;
    let mut result = result_skeleton(cand_id, quantization.clone(), &max_model_len, gmu);

    let serve_bpw = get(parsed, "serve_bpw")?;
    let num_params = get(parsed, "num_params")?;
    let prequantized = get(parsed, "prequantized")?.is_truthy();

    let weight_bytes: f64 = if prequantized {
        let native = get(parsed, "native_weight_bytes")?;
        if is_none(&native) {
            return Ok(failed(result, "native_weight_bytes_unknown"));
        }
        float_of(&native)?
    } else {
        if is_none(&num_params) {
            return Ok(failed(result, "num_params_unknown"));
        }
        if is_offline_only_quant(&quantization)? {
            result.set(
                "warning",
                text("offline_quant_on_non_prequantized_checkpoint"),
            );
        }
        if crate::quant_table::is_mixed_precision_quant(&quantization)? {
            // plan_26111708 B L9: a named reason instead of the generic unknown-method error — vLLM's MIXED_PRECISION reads a
            // per-layer recipe that only a pre-quantized ModelOpt checkpoint carries (quantization/modelopt.py:1653-1658).
            result.set("error", text("mixed_precision_requires_prequantized_checkpoint"));
            result.set(
                "note",
                text("modelopt_mixed = 층별 혼합 정밀도(vLLM ModelOptMixedPrecisionConfig) — quantization_config.quantized_layers 를 가진 사전양자화 ModelOpt 체크포인트에서만 성립한다(온라인 양자화 불가 · 단일 bpw 없음)"),
            );
            result.set("gate_pass", PyValue::Bool(false));
            return Ok(PyValue::Dict(result));
        }
        let PyValue::Tuple(pair) = vllm_quant_bpw_or_none(&quantization, &serve_bpw)? else {
            unreachable!("the resolver returns a pair")
        };
        if is_none(&pair[0]) {
            result.set("error", text("unknown_quant_method"));
            result.set(
                "note",
                text("quant 테이블 미등록 ≠ vLLM 미지원 — vLLM 소스/HF 카드로 외부 확인 필요(HITL surface)"),
            );
            result.set("gate_pass", PyValue::Bool(false));
            return Ok(PyValue::Dict(result));
        }
        let scaled = mul(&f(float_of(&num_params)?), &pair[0])?;
        float_of(&scaled)?
    };

    let kv_layers = kv_bearing_layers(parsed)?;
    let num_key_value_heads = get(parsed, "num_key_value_heads")?;
    let head_dim = get(parsed, "head_dim")?;
    let Some(kv_layers) =
        kv_layers.filter(|_| !is_none(&num_key_value_heads) && !is_none(&head_dim))
    else {
        return Ok(failed(result, "kv_dims_unknown"));
    };
    let kv_cache = BigInt::from(2)
        * &kv_layers
        * int_of(&num_key_value_heads)?
        * int_of(&head_dim)?
        * int_of(kv_cache_dtype_bytes)?
        * &max_model_len;

    let overhead = estimated_overhead(&num_params, || Ok(PyValue::Int(max_model_len.clone())))?;

    // weight is divided by tp (tensor parallelism); KV and overhead are not, as the plan's formula states.
    let per_device = float_div(weight_bytes, tp)? + float_of_int(&kv_cache)?;
    let estimated_total = float_div_by_value(per_device + float_of_int(&overhead)?, gmu)?;

    let estimated_total_gib = estimated_total / GIB as f64;
    let headroom_gib = sub(budget_gib, &f(estimated_total_gib))?;
    let ceiling = mul(budget_gib, safety_margin)?;
    let gate_pass = compare(Compare::LessEqual, &f(estimated_total_gib), &ceiling)?;

    result.set("weight_gib", f(weight_bytes / GIB as f64));
    result.set("kv_gib", f(int_div_power_of_two(&kv_cache, 30)?));
    result.set("kv_layers", PyValue::Int(kv_layers));
    result.set("kv_layers_source", text(kv_layers_source(parsed)?));
    result.set("overhead_gib", f(int_div_power_of_two(&overhead, 30)?));
    result.set("estimated_total_gib", f(estimated_total_gib));
    result.set("headroom_gib", headroom_gib);
    result.set("gate_pass", PyValue::Bool(gate_pass));
    Ok(PyValue::Dict(result))
}

/// `value / divisor` for two floats.
fn float_div_by_value(value: f64, divisor: f64) -> Res<f64> {
    float_div(value, &PyValue::Float(divisor))
}

/// `per_token_kv_bytes(parsed, kv_dtype_bytes)`: KV bytes of one token (K and V, GQA heads).
pub fn per_token_kv_bytes(parsed: &PyValue, kv_dtype_bytes: &PyValue) -> Res<BigInt> {
    let Some(layers) = kv_bearing_layers(parsed)? else {
        return Err(crate::pyexpr::missing_key("num_hidden_layers"));
    };
    Ok(BigInt::from(2)
        * layers
        * int_of(&item(parsed, "num_key_value_heads")?)?
        * int_of(&item(parsed, "head_dim")?)?
        * int_of(kv_dtype_bytes)?)
}

/// `required_kv_bytes(parsed, max_model_len, batch, kv_dtype_bytes)`: KV bytes for `batch` requests of `max_model_len`
/// tokens plus the 2% block-alignment buffer, truncated.
pub fn required_kv_bytes(
    parsed: &PyValue,
    max_model_len: &PyValue,
    batch: &PyValue,
    kv_dtype_bytes: &PyValue,
) -> Res<BigInt> {
    let per_token = per_token_kv_bytes(parsed, kv_dtype_bytes)?;
    let base = per_token * int_of(max_model_len)? * int_of(batch)?;
    int_of(&f(float_of_int(&base)? * KV_BLOCK_ALIGN_BUFFER))
}

/// `max_safe_kv_bytes(budget_gib, safety_margin, weights_bytes, overhead_bytes)`: the KV bytes that fit under the
/// ceiling; negative when weights and overhead alone exceed it.
pub fn max_safe_kv_bytes(
    budget_gib: &PyValue,
    safety_margin: &PyValue,
    weights_bytes: &PyValue,
    overhead_bytes: &PyValue,
) -> Res<BigInt> {
    let ceiling = mul(&mul(budget_gib, safety_margin)?, &PyValue::int(GIB))?;
    Ok(int_of(&ceiling)? - int_of(weights_bytes)? - int_of(overhead_bytes)?)
}

/// `weights_bytes_for(parsed, quantization)`: the model's weight bytes (the checkpoint's own bytes when pre-quantized,
/// otherwise parameters times the candidate quantization's bytes per weight).
pub fn weights_bytes_for(parsed: &PyValue, quantization: &PyValue) -> Res<f64> {
    if get(parsed, "prequantized")?.is_truthy() {
        let native = get(parsed, "native_weight_bytes")?;
        if is_none(&native) {
            return Err(Failure::new("ValueError", "native_weight_bytes_unknown"));
        }
        return float_of(&native);
    }
    let num_params = get(parsed, "num_params")?;
    if is_none(&num_params) {
        return Err(Failure::new("ValueError", "num_params_unknown"));
    }
    if crate::quant_table::is_mixed_precision_quant(quantization)? {
        // plan_26111708 B L9: the same named reason as `estimate` (no single bytes-per-weight · pre-quantized only).
        return Err(Failure::new("ValueError", "mixed_precision_requires_prequantized_checkpoint"));
    }
    let serve_bpw = get(parsed, "serve_bpw")?;
    let bpw = vllm_quant_bpw(quantization, &serve_bpw)?;
    float_of(&mul(&f(float_of(&num_params)?), &bpw)?)
}

/// `estimate_absolute(parsed, candidate, weights_bytes=None, overhead_bytes=None, kv_dtype_bytes=2)`: total VRAM of the
/// absolute KV clamp model, `weights + non-KV overhead + KV` (not divided by the utilization).
pub fn estimate_absolute(
    parsed: &PyValue,
    candidate: &PyValue,
    weights_bytes: &PyValue,
    overhead_bytes: &PyValue,
    kv_dtype_bytes: &PyValue,
) -> Res<PyValue> {
    let quantization = get(candidate, "quantization")?;
    let max_model_len = int_of(&item(candidate, "max_model_len")?)?;
    let batch = int_of(&get_or(candidate, "batch", PyValue::int(1))?)?;

    let weights = if is_none(weights_bytes) {
        weights_bytes_for(parsed, &quantization)?
    } else {
        float_of(weights_bytes)?
    };
    let overhead = if is_none(overhead_bytes) {
        let num_params = get(parsed, "num_params")?;
        float_of_int(&estimated_overhead(&num_params, || {
            Ok(PyValue::Int(max_model_len.clone()))
        })?)?
    } else {
        float_of(overhead_bytes)?
    };
    let kv_given = get(candidate, "kv_cache_memory_bytes")?;
    let kv = if is_none(&kv_given) {
        float_of_int(&required_kv_bytes(
            parsed,
            &PyValue::Int(max_model_len.clone()),
            &PyValue::Int(batch),
            kv_dtype_bytes,
        )?)?
    } else {
        float_of(&kv_given)?
    };
    let total = weights + overhead + kv;
    let mut result = object(vec![
        ("weight_gib", f(weights / GIB as f64)),
        ("overhead_gib", f(overhead / GIB as f64)),
        ("kv_gib", f(kv / GIB as f64)),
        ("total_gib", f(total / GIB as f64)),
        ("headroom_gib", PyValue::None),
    ]);
    let budget = get(candidate, "budget_gib")?;
    if !is_none(&budget) {
        result.set("headroom_gib", f(float_of(&budget)? - total / GIB as f64));
    }
    Ok(PyValue::Dict(result))
}

/// The weights and overhead the `max_feasible_*` functions use when the caller gives none.
fn limit_inputs(
    parsed: &PyValue,
    quantization: &PyValue,
    weights_bytes: &PyValue,
    overhead_bytes: &PyValue,
    overhead_length: impl FnOnce() -> Res<PyValue>,
) -> Res<(PyValue, PyValue)> {
    let weights = if is_none(weights_bytes) {
        f(weights_bytes_for(parsed, quantization)?)
    } else {
        weights_bytes.clone()
    };
    let overhead = if is_none(overhead_bytes) {
        let num_params = get(parsed, "num_params")?;
        PyValue::Int(estimated_overhead(&num_params, overhead_length)?)
    } else {
        overhead_bytes.clone()
    };
    Ok((weights, overhead))
}

/// The inputs of the two derived limits that are not the parsed model itself: the budget and the multiplier of its
/// ceiling, the candidate quantization and KV dtype width, and the caller's own weights and overhead when it has them.
pub struct Ceiling<'a> {
    pub budget_gib: &'a PyValue,
    pub safety_margin: &'a PyValue,
    pub quantization: &'a PyValue,
    pub kv_dtype_bytes: &'a PyValue,
    pub weights_bytes: &'a PyValue,
    pub overhead_bytes: &'a PyValue,
}

/// `max_feasible_max_len(parsed, budget_gib, safety_margin, batch, quantization, kv_dtype_bytes, weights_bytes=None,
/// overhead_bytes=None)`: the largest power-of-two context under the ceiling, capped by `max_position_embeddings`; 0
/// when none fits.
pub fn max_feasible_max_len(
    parsed: &PyValue,
    batch: &PyValue,
    ceiling: &Ceiling<'_>,
) -> Res<BigInt> {
    let weights = if is_none(ceiling.weights_bytes) {
        f(weights_bytes_for(parsed, ceiling.quantization)?)
    } else {
        ceiling.weights_bytes.clone()
    };
    let cap = get(parsed, "max_position_embeddings")?;
    let overhead = if is_none(ceiling.overhead_bytes) {
        let num_params = get(parsed, "num_params")?;
        PyValue::Int(estimated_overhead(&num_params, || {
            Ok(PyValue::Int(if is_none(&cap) {
                BigInt::zero()
            } else {
                int_of(&cap)?
            }))
        })?)
    } else {
        ceiling.overhead_bytes.clone()
    };
    let safe_kv = max_safe_kv_bytes(
        ceiling.budget_gib,
        ceiling.safety_margin,
        &weights,
        &overhead,
    )?;
    if !safe_kv.is_positive() {
        return Ok(BigInt::zero());
    }
    let per_token = per_token_kv_bytes(parsed, ceiling.kv_dtype_bytes)?;
    if !per_token.is_positive() {
        return Ok(BigInt::zero());
    }
    let batch = int_of(batch)?;
    if !batch.is_positive() {
        return Ok(BigInt::zero());
    }
    let raw_len = &safe_kv / (per_token * batch);
    if !raw_len.is_positive() {
        return Ok(BigInt::zero());
    }
    let mut power = BigInt::from(1) << (raw_len.bits() as usize - 1);
    if !is_none(&cap) {
        let capped = int_of(&cap)?;
        power = power.min(capped);
    }
    Ok(power)
}

/// `max_feasible_batch(parsed, budget_gib, safety_margin, max_model_len, quantization, kv_dtype_bytes,
/// weights_bytes=None, overhead_bytes=None)`: the largest batch for the given context under the ceiling; 0 when none.
pub fn max_feasible_batch(
    parsed: &PyValue,
    max_model_len: &PyValue,
    ceiling: &Ceiling<'_>,
) -> Res<BigInt> {
    let (weights, overhead) = limit_inputs(
        parsed,
        ceiling.quantization,
        ceiling.weights_bytes,
        ceiling.overhead_bytes,
        || Ok(PyValue::Int(int_of(max_model_len)?)),
    )?;
    let safe_kv = max_safe_kv_bytes(
        ceiling.budget_gib,
        ceiling.safety_margin,
        &weights,
        &overhead,
    )?;
    if !safe_kv.is_positive() {
        return Ok(BigInt::zero());
    }
    let per_token = per_token_kv_bytes(parsed, ceiling.kv_dtype_bytes)?;
    let denominator = per_token * int_of(max_model_len)?;
    if !denominator.is_positive() {
        return Ok(BigInt::zero());
    }
    Ok(safe_kv / denominator)
}

// ── self test ────────────────────────────────────────────────────────────────

/// Per-token KV bytes measured on 2026-08-22 for the Qwen3.8-27B shape (`52.28 GiB / 846,926 tokens`), as the original
/// records it.
const MEASURED_PER_TOKEN_QWEN38_27B: f64 = 66290.0;

/// What `--self-test` printed: the status and both streams.
pub struct SelfTest {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

fn shape(extra: &[(&str, PyValue)]) -> PyValue {
    let mut dict = object(vec![
        ("num_hidden_layers", PyValue::int(64)),
        ("is_hybrid", PyValue::Bool(true)),
        ("num_key_value_heads", PyValue::int(4)),
        ("head_dim", PyValue::int(256)),
        ("max_position_embeddings", PyValue::int(262144)),
        ("num_params", PyValue::int(27_000_000_000_i64)),
        ("prequantized", PyValue::Bool(false)),
        ("serve_bpw", f(2.0)),
    ]);
    for (key, value) in extra {
        dict.set(key, value.clone());
    }
    PyValue::Dict(dict)
}

/// `_self_test()`: the E1-E8 regression of the hybrid-aware KV layer count (no model or hardware needed). The expected
/// values are computed by the formulas, as in the original.
pub fn self_test() -> SelfTest {
    let failures: std::cell::RefCell<Vec<String>> = std::cell::RefCell::new(Vec::new());
    let check = |name: &str, got: PyValue, want: PyValue| {
        if got != want {
            failures
                .borrow_mut()
                .push(format!("{name}: got={} want={}", got.repr(), want.repr()));
        }
    };
    let two = || PyValue::int(2);
    let dense = {
        let mut value = shape(&[("is_hybrid", PyValue::Bool(false))]);
        if let PyValue::Dict(dict) = &mut value {
            dict.set("is_hybrid", PyValue::Bool(false));
        }
        value
    };
    let hybrid = {
        let mut dict = match shape(&[]) {
            PyValue::Dict(dict) => dict,
            _ => unreachable!("an object"),
        };
        // The hybrid shape carries the full-attention count right after the layer count, as in the original literal.
        let mut ordered = PyDict::new();
        for (key, value) in dict.iter() {
            ordered.set(&key.to_utf8().expect("ascii key"), value.clone());
            if key.equals("num_hidden_layers") {
                ordered.set("num_full_attention_layers", PyValue::int(16));
            }
        }
        dict = ordered;
        PyValue::Dict(dict)
    };
    let cand = PyValue::Dict(object(vec![
        ("id", text("selftest")),
        ("quantization", PyValue::None),
        ("max_model_len", PyValue::int(262144)),
        ("gpu_memory_utilization", f(0.90)),
    ]));
    let (one, budget, margin) = (PyValue::int(1), f(115.0), f(0.90));
    let run = |parsed: &PyValue| -> PyDict {
        match estimate(parsed, &cand, &one, &budget, &margin, &two())
            .expect("the self-test estimate computes")
        {
            PyValue::Dict(dict) => dict,
            _ => unreachable!("an object"),
        }
    };
    let field = |dict: &PyDict, key: &str| dict.get(key).cloned().unwrap_or(PyValue::None);

    let r_h = run(&hybrid);
    check("E1.kv_gib", field(&r_h, "kv_gib"), f(16.0));
    check("E1.kv_layers", field(&r_h, "kv_layers"), PyValue::int(16));
    check(
        "E1.kv_layers_source",
        field(&r_h, "kv_layers_source"),
        text("num_full_attention_layers"),
    );

    let r_d = run(&dense);
    check("E2.kv_gib", field(&r_d, "kv_gib"), f(64.0));
    check("E2.kv_layers", field(&r_d, "kv_layers"), PyValue::int(64));
    check(
        "E2.kv_layers_source",
        field(&r_d, "kv_layers_source"),
        text("num_hidden_layers"),
    );
    let quarter = |dict: &PyDict| match field(dict, "kv_gib") {
        PyValue::Float(value) => value * 4.0,
        _ => f64::NAN,
    };
    check(
        "E2.ratio_is_exactly_quarter",
        f(quarter(&r_h)),
        field(&r_d, "kv_gib"),
    );
    for key in ["weight_gib", "overhead_gib"] {
        check(
            &format!("E2.{key}_unchanged"),
            field(&r_h, key),
            field(&r_d, key),
        );
    }

    let with_full = |base: &PyValue, value: PyValue| {
        let mut dict = match base {
            PyValue::Dict(dict) => dict.clone(),
            _ => unreachable!("an object"),
        };
        dict.set("num_full_attention_layers", value);
        PyValue::Dict(dict)
    };
    let layers = |parsed: &PyValue| match kv_bearing_layers(parsed).expect("computes") {
        Some(number) => PyValue::Int(number),
        None => PyValue::None,
    };
    let all_full = with_full(&dense, PyValue::int(64));
    check("E3.layers", layers(&all_full), PyValue::int(64));
    check(
        "E3.source",
        text(kv_layers_source(&all_full).expect("computes")),
        text("num_hidden_layers"),
    );

    check(
        "E4.zero",
        layers(&with_full(&dense, PyValue::int(0))),
        PyValue::int(64),
    );
    check(
        "E4.over",
        layers(&with_full(&dense, PyValue::int(99))),
        PyValue::int(64),
    );
    check(
        "E4.garbage",
        layers(&with_full(&dense, text("x"))),
        PyValue::int(64),
    );

    let kv2 = PyValue::int(2);
    let big_value = |number: Res<BigInt>| PyValue::Int(number.expect("computes"));
    check(
        "E5.per_token_hybrid",
        big_value(per_token_kv_bytes(&hybrid, &kv2)),
        PyValue::int(65536),
    );
    check(
        "E5.per_token_dense",
        big_value(per_token_kv_bytes(&dense, &kv2)),
        PyValue::int(262144),
    );
    let expected_required = (65536.0 * 262144.0 * KV_BLOCK_ALIGN_BUFFER).trunc();
    check(
        "E5.required_hybrid",
        big_value(required_kv_bytes(
            &hybrid,
            &PyValue::int(262144),
            &PyValue::int(1),
            &kv2,
        )),
        PyValue::Int(BigInt::from(expected_required as i64)),
    );

    let per_token = |parsed: &PyValue| {
        per_token_kv_bytes(parsed, &kv2)
            .expect("computes")
            .to_f64()
            .expect("fits")
    };
    let error_of = |parsed: &PyValue| {
        (per_token(parsed) - MEASURED_PER_TOKEN_QWEN38_27B).abs() / MEASURED_PER_TOKEN_QWEN38_27B
    };
    let (err_h, err_d) = (error_of(&hybrid), error_of(&dense));
    if !matches!(
        err_h.partial_cmp(&0.05),
        Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
    ) {
        failures.borrow_mut().push(format!(
            "E6.hybrid_vs_measured: 오차 {} > 0.05 (공식이 실측과 어긋난다)",
            agent_runtime_support::pynumber::format_fixed(err_h, 3)
        ));
    }
    if !matches!(err_d.partial_cmp(&1.0), Some(std::cmp::Ordering::Greater)) {
        failures.borrow_mut().push(format!(
            "E6.dense_vs_measured: 전층 공식 오차 {} — 결함 A 전제가 무너졌다",
            agent_runtime_support::pynumber::format_fixed(err_d, 3)
        ));
    }

    check(
        "E7.hybrid_gate",
        field(&r_h, "gate_pass"),
        PyValue::Bool(true),
    );
    check(
        "E7.dense_gate",
        field(&r_d, "gate_pass"),
        PyValue::Bool(false),
    );

    let weights = f(51.1 * GIB as f64);
    let overhead = f(2.0 * GIB as f64);
    let none = PyValue::None;
    let ceiling = Ceiling {
        budget_gib: &budget,
        safety_margin: &margin,
        quantization: &none,
        kv_dtype_bytes: &kv2,
        weights_bytes: &weights,
        overhead_bytes: &overhead,
    };
    let max_len =
        |parsed: &PyValue| max_feasible_max_len(parsed, &one, &ceiling).expect("computes");
    let (len_h, len_d) = (max_len(&hybrid), max_len(&dense));
    let position_cap = BigInt::from(262144);
    if !(len_h >= &len_d * 4 || len_h == position_cap) {
        failures.borrow_mut().push(format!(
            "E8.max_len: hybrid={len_h} dense={len_d} — 파생 함수가 층수 인지를 안 탄다"
        ));
    }
    let batch = |parsed: &PyValue| {
        max_feasible_batch(parsed, &PyValue::int(32768), &ceiling).expect("computes")
    };
    let (b_h, b_d) = (batch(&hybrid), batch(&dense));
    if !(&b_d * 4 <= b_h && b_h < (&b_d + 1) * 4) {
        failures.borrow_mut().push(format!(
            "E8.batch: hybrid={b_h} dense={b_d} — 4배 대역 밖(층수 인지 미적용 의심)"
        ));
    }

    let failures = failures.into_inner();
    if failures.is_empty() {
        return SelfTest {
            status: 0,
            stdout: "[estimate_vram --self-test] OK — E1~E8 통과 (하이브리드 KV 층수 인지 · plan_26082223 결함 A)\n"
                .to_string(),
            stderr: String::new(),
        };
    }
    let mut stderr = format!("[estimate_vram --self-test] FAIL {} 건:\n", failures.len());
    for failure in &failures {
        stderr.push_str(&format!("  - {failure}\n"));
    }
    SelfTest {
        status: 1,
        stdout: String::new(),
        stderr,
    }
}
