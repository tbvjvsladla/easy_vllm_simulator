//! `quant_table.py`: the quant byte tables (vendored from whichllm) and the functions that resolve a vLLM quantization
//! flag or a dtype name to bytes per weight. The tables keep the original's keys, values and order; `None` stands for the
//! original's `None` entries of `VLLM_QUANT_BYTES` (the weight is then the caller's `serve_bpw`).

use crate::pyexpr::{stripped_lower, text, Res};
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};

/// `QUANT_BYTES_PER_WEIGHT`: the GGUF table (reference only).
pub const QUANT_BYTES_PER_WEIGHT: [(&str, f64); 30] = [
    ("F32", 4.0),
    ("F16", 2.0),
    ("BF16", 2.0),
    ("Q8_0", 1.0625),
    ("Q6_K", 0.8125),
    ("Q5_K_M", 0.6875),
    ("Q5_K_S", 0.6875),
    ("Q5_0", 0.625),
    ("Q4_K_M", 0.5625),
    ("Q4_K_S", 0.5625),
    ("Q4_0", 0.5),
    ("Q3_K_M", 0.4375),
    ("Q3_K_S", 0.4375),
    ("Q3_K_L", 0.4375),
    ("Q2_K", 0.3125),
    ("IQ4_XS", 0.5),
    ("IQ3_XXS", 0.375),
    ("IQ2_XXS", 0.25),
    ("Q1_0", 0.28),
    ("Q2_0", 0.28),
    ("TQ1_0", 0.21),
    ("TQ2_0", 0.28),
    ("IQ1_S", 0.21),
    ("IQ1_M", 0.22),
    ("IQ2_S", 0.275),
    ("IQ2_M", 0.30),
    ("IQ3_S", 0.40),
    ("IQ3_M", 0.42),
    ("IQ3_XS", 0.41),
    ("IQ4_NL", 0.5),
];

/// `VLLM_QUANT_BYTES`: vLLM `--quantization` flag (lower-case) to bytes per weight; `None` = use the caller's dtype weight.
pub const VLLM_QUANT_BYTES: [(&str, Option<f64>); 27] = [
    ("none", None),
    ("null", None),
    ("", None),
    ("awq", Some(0.5)),
    ("gptq", Some(0.5)),
    ("bitsandbytes", Some(0.5)),
    ("bnb", Some(0.5)),
    ("int4", Some(0.5)),
    ("mxfp4", Some(0.5)),
    ("nvfp4", Some(0.5)),
    ("compressed-tensors", Some(0.5)),
    ("awq_marlin", Some(0.5)),
    ("gptq_marlin", Some(0.5)),
    ("marlin", Some(0.5)),
    ("bitblas", Some(0.5)),
    ("gptq_bitblas", Some(0.5)),
    ("fp8", Some(1.0)),
    ("int8", Some(1.0)),
    ("modelopt", Some(1.0)),
    ("modelopt_fp8", Some(1.0)),
    ("fbgemm_fp8", Some(1.0)),
    ("quark", Some(1.0)),
    ("experts_int8", Some(1.0)),
    ("bf16", Some(2.0)),
    ("fp16", Some(2.0)),
    ("float16", Some(2.0)),
    ("bfloat16", Some(2.0)),
];

/// `OFFLINE_ONLY_QUANT`: methods that need a pre-quantized checkpoint (a frozenset; listed sorted).
pub const OFFLINE_ONLY_QUANT: [&str; 7] = [
    "awq",
    "awq_marlin",
    "bitblas",
    "gptq",
    "gptq_bitblas",
    "gptq_marlin",
    "marlin",
];

/// `FRAMEWORK_OVERHEAD_BYTES`: resident framework overhead (~500 MB).
pub const FRAMEWORK_OVERHEAD_BYTES: i64 = 500_000_000;

/// `DTYPE_BPW`: on-disk / serve dtype name to bytes per weight.
pub const DTYPE_BPW: [(&str, f64); 12] = [
    ("float32", 4.0),
    ("fp32", 4.0),
    ("f32", 4.0),
    ("float16", 2.0),
    ("fp16", 2.0),
    ("f16", 2.0),
    ("bfloat16", 2.0),
    ("bf16", 2.0),
    ("float8_e4m3fn", 1.0),
    ("float8", 1.0),
    ("fp8", 1.0),
    ("f8", 1.0),
];

/// Quantization methods vLLM registers whose bytes per weight are not one number (plan_26111708 B L9). `modelopt_mixed` is
/// ModelOpt MIXED_PRECISION: a per-layer `quant_algo` (FP8, NVFP4, W4A16_NVFP4, MXFP8, ...) read from
/// `quantization_config.quantized_layers` (vllm 0.32.0 `model_executor/layers/quantization/modelopt.py:1543-1551`, registered
/// at `quantization/__init__.py:166`), and it exists only in a pre-quantized ModelOpt checkpoint — `_from_config` raises when
/// `quantized_layers` is empty (`modelopt.py:1653-1658`). Deliberately not a row of [`VLLM_QUANT_BYTES`]: there is no single
/// bytes-per-weight to put there, so the callers name the reason instead of reporting an unknown method.
pub const MIXED_PRECISION_QUANT: [&str; 1] = ["modelopt_mixed"];

/// Whether `quant` is one of [`MIXED_PRECISION_QUANT`] (after `str().strip().lower()`; `None` is not).
pub fn is_mixed_precision_quant(quant: &PyValue) -> Res<bool> {
    if matches!(quant.peel(), PyValue::None) {
        return Ok(false);
    }
    let key = stripped_lower(quant)?;
    Ok(MIXED_PRECISION_QUANT.iter().any(|name| key.equals(name)))
}

fn table_entry(key: &PyStr) -> Option<Option<f64>> {
    VLLM_QUANT_BYTES
        .iter()
        .find(|(name, _)| key.equals(name))
        .map(|&(_, value)| value)
}

/// `is_offline_only_quant(quant)`.
pub fn is_offline_only_quant(quant: &PyValue) -> Res<bool> {
    if matches!(quant.peel(), PyValue::None) {
        return Ok(false);
    }
    let key = stripped_lower(quant)?;
    Ok(OFFLINE_ONLY_QUANT.iter().any(|name| key.equals(name)))
}

/// The sorted known flags of the error message (`sorted(k for k in VLLM_QUANT_BYTES if k)`), as a list repr.
fn known_flags() -> String {
    let mut names: Vec<&str> = VLLM_QUANT_BYTES
        .iter()
        .map(|&(name, _)| name)
        .filter(|name| !name.is_empty())
        .collect();
    names.sort_unstable();
    let shown: Vec<String> = names.iter().map(|name| PyStr::new(name).repr()).collect();
    format!("[{}]", shown.join(", "))
}

/// `vllm_quant_bpw(quant, serve_bpw)`: the flag's bytes per weight, or `serve_bpw` (returned as it is, whatever it is)
/// for no flag and for the `none`/`null`/empty flags; an unknown flag is a `ValueError`.
pub fn vllm_quant_bpw(quant: &PyValue, serve_bpw: &PyValue) -> Res<PyValue> {
    if matches!(quant.peel(), PyValue::None) {
        return Ok(serve_bpw.clone());
    }
    let key = stripped_lower(quant)?;
    if key.is_empty() || key.equals("none") {
        return Ok(serve_bpw.clone());
    }
    match table_entry(&key) {
        None => {
            let shown = quant.repr();
            Err(Failure::new(
                "ValueError",
                &format!(
                    "unknown vLLM quantization flag: {shown} (known: {})",
                    known_flags()
                ),
            ))
        }
        Some(None) => Ok(serve_bpw.clone()),
        Some(Some(bpw)) => Ok(PyValue::Float(bpw)),
    }
}

/// `vllm_quant_bpw_or_none(quant, serve_bpw)`: `(bpw, None)` or, for an unknown flag, `(None, warning)`; the
/// non-`ValueError` failures (a quant that is not a string) still propagate.
pub fn vllm_quant_bpw_or_none(quant: &PyValue, serve_bpw: &PyValue) -> Res<PyValue> {
    match vllm_quant_bpw(quant, serve_bpw) {
        Ok(bpw) => Ok(PyValue::Tuple(vec![bpw, PyValue::None])),
        Err(failure) if failure.is_value_error() => Ok(PyValue::Tuple(vec![
            PyValue::None,
            text(&format!(
                "unknown quant_method {} (테이블 미등록) → vllm_quant_bpw 산출 불가, 폴백 필요",
                quant.repr()
            )),
        ])),
        Err(failure) => Err(failure),
    }
}

/// `dtype_bpw(dtype)`: `(bytes per weight, warning or None)`; unknown, empty and missing dtypes give 2.0 and a warning.
pub fn dtype_bpw(dtype: &PyValue) -> Res<PyValue> {
    let pair = |bpw: f64, warning: Option<String>| {
        PyValue::Tuple(vec![
            PyValue::Float(bpw),
            warning.map_or(PyValue::None, |w| text(&w)),
        ])
    };
    if matches!(dtype.peel(), PyValue::None) {
        return Ok(pair(
            2.0,
            Some("dtype is None; defaulting bpw=2.0".to_string()),
        ));
    }
    let key = stripped_lower(dtype)?;
    if key.is_empty() {
        return Ok(pair(
            2.0,
            Some("dtype is empty; defaulting bpw=2.0".to_string()),
        ));
    }
    if let Some(&(_, bpw)) = DTYPE_BPW.iter().find(|(name, _)| key.equals(name)) {
        return Ok(pair(bpw, None));
    }
    Ok(pair(
        2.0,
        Some(format!(
            "unknown dtype {}; defaulting bpw=2.0",
            dtype.repr()
        )),
    ))
}

/// The constants as the original module exposes them (names, values, key order); `OFFLINE_ONLY_QUANT` as a sorted array.
pub fn constants() -> PyDict {
    let table = |rows: &[(&str, f64)]| {
        let mut dict = PyDict::new();
        for &(name, value) in rows {
            dict.set(name, PyValue::Float(value));
        }
        PyValue::Dict(dict)
    };
    let mut vllm = PyDict::new();
    for &(name, value) in VLLM_QUANT_BYTES.iter() {
        vllm.set(name, value.map_or(PyValue::None, PyValue::Float));
    }
    let mut all = PyDict::new();
    all.set("QUANT_BYTES_PER_WEIGHT", table(&QUANT_BYTES_PER_WEIGHT));
    all.set("VLLM_QUANT_BYTES", PyValue::Dict(vllm));
    all.set("DTYPE_BPW", table(&DTYPE_BPW));
    all.set(
        "FRAMEWORK_OVERHEAD_BYTES",
        PyValue::int(FRAMEWORK_OVERHEAD_BYTES),
    );
    all.set(
        "OFFLINE_ONLY_QUANT",
        PyValue::List(OFFLINE_ONLY_QUANT.iter().map(|name| text(name)).collect()),
    );
    all
}
