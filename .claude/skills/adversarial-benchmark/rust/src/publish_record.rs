//! The benchmark certificate writer: a port of `publish_benchmark_record.py`.
//!
//! A certificate is the *flat*, machine-readable record "this model was tested on this hardware and configuration
//! and passed". Flat means no nesting, so any later tool can read it with a line scanner; the file is emitted
//! directly rather than through a YAML library to keep that guarantee. It carries the carry-forward header: six
//! strong identity keys (a mismatch voids the certificate) and soft fingerprints (a mismatch only warns).
//!
//! Two grades exist. The **full** certificate (`benchmark_mode: full`, verdict `PASS`) is written only when the
//! sweep passed *and* the bench-mode decision record confirms the full definition (repetitions) was met. The
//! **lite** certificate (`benchmark_mode: lite`, `verdict: not_applicable`) records that serving held up in the
//! minimal scenario; it carries no performance verdict and is the ground for publishing a hint, never for claiming
//! performance. Nothing here judges: every verdict and mode is read from the owners that decide them.

use crate::pydyn::{as_get_target, iterate, join_values};
use crate::pystr::{lit, PyStr};
use crate::pyvalue::{PyDict, PyValue};
use crate::report_support::{display, py, yaml_scalar, Res};

pub const ENTRY_LITE_ONLY: &str = "beta_lite_only_cell";
pub const ENTRY_DEMOTED: &str = "gamma_demoted_lite";
/// Soft fingerprints of a lite certificate (the lite leg measures with `vllm bench serve` only).
pub const LITE_CERT_SOFT_KEYS: [&str; 10] = [
    "image_tag",
    "image_digest",
    "max_model_len",
    "max_num_seqs",
    "kv_cache_memory_bytes",
    "kv_cache_dtype",
    "gpu_memory_utilization",
    "moe_backend",
    "attention_backend",
    "ple_mode",
];
const STRONG_KEYS: [&str; 6] = [
    "model",
    "gpu_model",
    "vllm_version",
    "quantization",
    "topology",
    "tensor_parallel_size",
];
const FULL_SOFT_KEYS: [&str; 32] = [
    "driver_version",
    "cuda_version",
    "image_tag",
    "image_digest",
    "max_model_len",
    "max_num_seqs",
    "kv_cache_memory_bytes",
    "kv_cache_dtype",
    "gpu_memory_utilization",
    "moe_backend",
    "enforce_eager",
    "bench_tool",
    "bench_tool_version",
    "bench_tool_version_source",
    "attention_backend",
    "attention_backend_source",
    "attention_backend_mismatch",
    "moe_backend_source",
    "moe_backend_mismatch",
    "ple_mode",
    "ple_mode_source",
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
    "dataset_provenance",
];

fn get(source: &PyValue, key: &str) -> Res<PyValue> {
    Ok(as_get_target(source)?
        .get(key)
        .cloned()
        .unwrap_or(PyValue::None))
}

fn get_or_else(source: &PyValue, key: &str, fallback: PyValue) -> Res<PyValue> {
    let value = get(source, key)?;
    Ok(if value.is_truthy() { value } else { fallback })
}

struct Record(Vec<PyStr>);

impl Record {
    fn add(&mut self, line: PyStr) {
        self.0.push(line);
    }

    fn field(&mut self, key: &str, value: &PyValue) {
        let scalar = yaml_scalar(value);
        let mut points = Vec::with_capacity(key.len() + 2 + scalar.len());
        points.extend(key.chars().map(u32::from));
        points.extend([u32::from(':'), u32::from(' ')]);
        points.extend_from_slice(scalar.code_points());
        self.0.push(PyStr::from_code_points(points));
    }

    fn field_text(&mut self, key: &str, value: &str) {
        let mut text = py(key);
        text.push_str(": ");
        text.push_str(value);
        self.0.push(text);
    }

    fn into_text(self) -> Res<PyStr> {
        let values: Vec<PyValue> = self.0.into_iter().map(PyValue::Str).collect();
        let mut text = join_values("\n", &values)?;
        text.push_str("\n");
        Ok(text)
    }
}

/// `(lite.get("capacity") or {}).get("main") or {}`.
fn main_capacity(lite: &PyValue) -> Res<PyValue> {
    let capacity = get_or_else(lite, "capacity", PyValue::Dict(PyDict::new()))?;
    get_or_else(&capacity, "main", PyValue::Dict(PyDict::new()))
}

/// The full certificate of a passing sweep, from the sweep index and the verdict.
pub fn build_yaml(index: &PyValue, verdict: &PyValue) -> Res<PyStr> {
    let meta = as_get_target(index)?
        .get("meta")
        .cloned()
        .unwrap_or_else(|| PyValue::Dict(PyDict::new()));
    let rubric = get_or_else(verdict, "rubric", PyValue::Dict(PyDict::new()))?;
    let levels = as_get_target(index)?
        .get("levels")
        .cloned()
        .unwrap_or_else(|| PyValue::List(Vec::new()));
    let mut completed: Vec<PyValue> = Vec::new();
    for level in iterate(&levels)? {
        if matches!(get(&level, "status")?.peel(), PyValue::Str(text) if text.equals("ok")) {
            completed.push(get(&level, "level")?);
        }
    }
    let truncated = as_get_target(index)?
        .get("truncated")
        .cloned()
        .unwrap_or_else(|| PyValue::List(Vec::new()));
    let mut out = Record(Vec::new());
    out.add(lit!("# ⚠ CARRY-FORWARD 재검증 필수 — 이건 '그때-그 환경' 한정 측정이다(지도≠정답). 소비 전 재확인하라."));
    out.add(lit!(
        "#   강한 일치 키(model/gpu/vllm/quant/topology/tp): 정확일치 실패 시 이 인증서 무효."
    ));
    out.add(lit!("#   model = 체크포인트에서 파생한 **모델 축**(소문자). 운영 조합명은 serving_config 축에 따로 산다."));
    out.add(lit!("#   소프트 지문(driver/cuda/image/max-len/kv-bytes/gmu/moe): 불일치 시 stale — 재측정 권고."));
    out.add(lit!("#   발행 = full 모드 verdict==PASS 시만(결정론 publish_benchmark_record.py · inform-record)."));
    out.add(lit!("schema_version: 1"));
    out.add(lit!("record_type: benchmark_certificate"));
    out.add(lit!("verdict: PASS"));
    out.add(lit!(""));
    out.add(lit!("# --- 강한 일치 키 (정확일치 필요) ---"));
    for key in STRONG_KEYS {
        out.field(key, &get(&meta, key)?);
    }
    out.add(lit!(""));
    out.add(lit!("# --- 운영 조합명(모델 축 아님 · 정본 필드) ---"));
    out.field("serving_config", &get(&meta, "serving_config")?);
    out.field("model_source", &get(&meta, "model_source")?);
    out.add(lit!(""));
    out.add(lit!("# --- 소프트 지문 (불일치 시 stale 경고) ---"));
    for key in FULL_SOFT_KEYS {
        out.field(key, &get(&meta, key)?);
    }
    out.field("ngc_base_tag", &get(&meta, "ngc_base_tag")?);
    out.add(lit!(""));
    out.add(lit!("# --- 검증 결과(인증서 본문) ---"));
    out.add(lit!("benchmark_mode: full"));
    out.field("decode_tps_conc1", &get(verdict, "measured_decode_tps")?);
    out.field("rubric_authority", &get(&rubric, "authority")?);
    out.field("primary_source", &get(&rubric, "source")?);
    out.field("primary_tps", &get(&rubric, "primary")?);
    out.field("floor_tps", &get(&rubric, "floor")?);
    out.field("tolerance", &get(&rubric, "tolerance")?);
    out.field(
        "ratio_M_over_primary",
        &get(&rubric, "ratio_M_over_primary")?,
    );
    out.field("e_search", &get(verdict, "e_search")?);
    out.field("spec_on", &get(verdict, "measured_spec_on")?);
    out.field("accept_len", &get(verdict, "measured_accept_len")?);
    let levels_text = if completed.is_empty() {
        PyValue::None
    } else {
        let shown: Vec<PyValue> = completed
            .iter()
            .map(|level| PyValue::Str(display(level)))
            .collect();
        PyValue::Str(join_values(",", &shown)?)
    };
    out.field("sweep_levels", &levels_text);
    let truncated_text = if truncated.is_truthy() {
        PyValue::Str(crate::pydyn::join("; ", &truncated)?)
    } else {
        PyValue::None
    };
    out.field("sweep_truncated", &truncated_text);
    let lite_value = get(index, "lite")?;
    let lite = if matches!(lite_value.peel(), PyValue::Dict(_)) {
        lite_value
    } else {
        PyValue::Dict(PyDict::new())
    };
    let capacity = main_capacity(&lite)?;
    out.field_text(
        "lite_included",
        if get(&lite, "table")?.is_truthy() {
            "true"
        } else {
            "false"
        },
    );
    out.field("lite_gen_tps_warm", &get(&lite, "gen_tps")?);
    out.field("lite_gen_src", &get(&lite, "gen_src")?);
    out.field("lite_cold_ttft_ms", &get(&lite, "cold_ttft_ms")?);
    out.field("lite_kv_gib", &get(&lite, "kv_gib")?);
    out.field("lite_gpu_occupancy", &get(&capacity, "gpu")?);
    out.field("lite_ram_occupancy", &get(&capacity, "ram")?);
    out.field("lite_accept_len", &get(&lite, "accept_len")?);
    out.field("lite_accept_len_src", &get(&lite, "accept_len_src")?);
    let balance = get(verdict, "balance")?;
    if balance.is_truthy() {
        out.field("node_vram_balance_dev", &get(&balance, "balance_dev")?);
        out.field("node_vram_balance_pass", &get(&balance, "pass")?);
    }
    out.field("measured_utc", &get(index, "generated_utc")?);
    out.field("measured_node", &get(&meta, "measured_node")?);
    out.field("measured_node_source", &get(&meta, "measured_node_source")?);
    out.field(
        "server_alive_at_bench_end",
        &get(&meta, "server_alive_at_bench_end")?,
    );
    out.field("boundary_exemption", &get(&meta, "boundary_exemption")?);
    out.into_text()
}

/// The lite-grade certificate: strong identity, soft fingerprints, the lite metrics and how the cell got here
/// (`entry_path`). `lite` is the lite metrics (or the lite block of a sweep index).
pub fn build_lite_yaml(
    meta: &PyValue,
    lite: &PyValue,
    measured_utc: &PyValue,
    entry_path: &PyValue,
    verdict_source: &PyValue,
) -> Res<PyStr> {
    let mut out = Record(Vec::new());
    out.add(lit!("# ⚠ CARRY-FORWARD 재검증 필수 — 이건 '그때-그 환경' 한정 측정이다(지도≠정답). 소비 전 재확인하라."));
    out.add(lit!(
        "#   강한 일치 키(model/gpu/vllm/quant/topology/tp): 정확일치 실패 시 이 인증서 무효."
    ));
    out.add(lit!("#   ★ lite 등급 인증서 — 서빙이 실사용 최소 시나리오(cold 1 + warm burst · 동시성 1)에서 **성립**했다는 기록이다."));
    out.add(lit!("#     성능 판정(verdict)이 아니다 · 성능 완료 선언의 근거가 될 수 없다(benchmark_mode=full 만). hint 발행 자격의 근거다."));
    out.add(lit!(
        "#   발행 = lite_verdict==pass 시만(결정론 publish_benchmark_record.py · inform-record)."
    ));
    out.add(lit!("schema_version: 1"));
    out.add(lit!("record_type: benchmark_certificate"));
    out.add(lit!("verdict: not_applicable"));
    out.add(lit!(""));
    out.add(lit!("# --- 강한 일치 키 (정확일치 필요) ---"));
    for key in STRONG_KEYS {
        out.field(key, &get(meta, key)?);
    }
    out.add(lit!(""));
    out.add(lit!("# --- 운영 조합명(모델 축 아님 · 정본 필드) ---"));
    out.field("serving_config", &get(meta, "serving_config")?);
    out.field("model_source", &get(meta, "model_source")?);
    out.field("quantization_source", &get(meta, "quantization_source")?);
    out.add(lit!(""));
    out.add(lit!(
        "# --- 소프트 지문 (불일치 시 stale 경고 · 결측 N/A) ---"
    ));
    for key in LITE_CERT_SOFT_KEYS {
        out.field(key, &get(meta, key)?);
    }
    out.add(lit!("bench_tool: vllm-bench-serve"));
    out.add(lit!(""));
    out.add(lit!("# --- 등급(인증서 본문) ---"));
    out.add(lit!("benchmark_mode: lite"));
    out.add(lit!("lite_verdict: pass"));
    out.field("lite_verdict_source", verdict_source);
    let mut entry = lit!("entry_path: ");
    entry.push_py(&display(entry_path));
    out.0.push(entry);
    let capacity = main_capacity(lite)?;
    out.add(lit!("lite_included: true"));
    out.field("lite_gen_tps_warm", &get(lite, "gen_tps")?);
    out.field("lite_gen_src", &get(lite, "gen_src")?);
    out.field("lite_cold_ttft_ms", &get(lite, "cold_ttft_ms")?);
    out.field("lite_kv_gib", &get(lite, "kv_gib")?);
    out.field("lite_gpu_occupancy", &get(&capacity, "gpu")?);
    out.field("lite_ram_occupancy", &get(&capacity, "ram")?);
    out.field("lite_accept_len", &get(lite, "accept_len")?);
    out.field("lite_accept_len_src", &get(lite, "accept_len_src")?);
    out.field("measured_utc", measured_utc);
    out.field("measured_node", &get(meta, "measured_node")?);
    out.field("measured_node_source", &get(meta, "measured_node_source")?);
    out.into_text()
}
