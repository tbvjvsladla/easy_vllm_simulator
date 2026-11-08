//! `gen_recipe_set.py`: the deterministic generator of the three files of a serving set (`configs/<name>.yaml`,
//! `configs/<name>.sh`, `envs/.env.<name>`) from a parsed model and a chosen recipe, the single projection from a trial
//! candidate to a recipe, the parity check that every knob a trial reads reaches the three files, and the file handling of
//! `generate`.
//!
//! The inputs are JSON-shaped values of any type (`parsed`, `recipe`, `candidate` are dicts when things go right, and
//! whatever the caller passed when they do not), so every `.get`, `str()`, `int()`, truthiness test and `format` is the
//! Python operation on a [`PyValue`], with the exception Python raises, in the order Python evaluates them. The generated
//! text is exactly what the original builds: comments, knob order and unusual values are part of the contract (the text is
//! never quoted, escaped or repaired; the generated files are not executed here).
//!
//! The file handling is real and deliberately plain: both directories are created before the guard for existing files; the
//! three texts are rendered (and the warnings written) before the first file is opened; the files are opened for writing
//! in the order yaml, sh, env, each created or truncated before its text is encoded, so a text that cannot be encoded leaves
//! the earlier files complete, its own file empty and the later ones untouched. There is no atomic write, no rollback, no
//! path check, no lock. Links are followed as `open` and `os.path.exists` follow them.

use crate::model_dyn::{contains, is_os_error, Message};
use crate::model_files::{exists, is_dir, join_values, os_failure, path_bytes};
use crate::pyexpr::{get, get_or, int_of, lower, Res};
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pydyn::is_text;
use agent_runtime_support::pystr::{EncodeFailure, PyStr};
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use num_bigint::BigInt;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::PathBuf;

/// Values of a recipe that mean "no quantization" (after `str().strip().lower()`); `None` is native as well.
pub const NATIVE_QUANT_TEXTS: [&str; 10] = [
    "", "none", "null", "native", "bf16", "fp16", "float16", "bfloat16", "float32", "fp32",
];

/// The fields a candidate hands to the recipe, in the order of the projection (`SERVE_KNOB_KEYS`).
pub const SERVE_KNOB_KEYS: [&str; 16] = [
    "quantization",
    "max_model_len",
    "batch",
    "kv_cache_memory_bytes",
    "kv_cache_quant",
    "attention_backend",
    "tool_call_parser",
    "reasoning_parser",
    "moe_backend",
    "gdn_prefill_backend",
    "max_num_batched_tokens",
    "enforce_eager",
    "language_model_only",
    "serve_env",
    "tensor_parallel_size",
    "distributed_executor_backend",
];

/// Fields a trial reads that the generator deliberately does not carry (`_PARITY_EXEMPT`), in declaration order.
pub const PARITY_EXEMPT: [&str; 11] = [
    "model_path",
    "model_path_container",
    "model_capabilities",
    "id",
    "attention_backend",
    "tool_call_parser",
    "reasoning_parser",
    "served_model_name",
    "model_id",
    "extra_env",
    "gpu_memory_utilization",
];

/// The representative values the parity check gives a field (`_PROBE`); any other field gets `PROBE<FIELD>VALUE`.
pub fn probe_value(field: &str) -> PyValue {
    match field {
        "enforce_eager" | "language_model_only" => PyValue::Bool(true),
        "max_num_batched_tokens" | "batch" | "max_model_len" | "kv_cache_memory_bytes" => {
            PyValue::int(4242)
        }
        "gpu_memory_utilization" => PyValue::Float(0.77),
        other => PyValue::str(&format!(
            "PROBE{}VALUE",
            other.to_uppercase().replace('_', "")
        )),
    }
}

/// The names of `_PROBE`, in declaration order.
pub const PROBE_NAMES: [&str; 7] = [
    "enforce_eager",
    "language_model_only",
    "max_num_batched_tokens",
    "batch",
    "max_model_len",
    "kv_cache_memory_bytes",
    "gpu_memory_utilization",
];

fn stripped_lower_text(value: &PyValue) -> PyStr {
    lower(&value.display_text().strip())
}

/// `_is_native_or_none(quant)`.
pub fn is_native_or_none(quant: &PyValue) -> Res<bool> {
    if matches!(quant.peel(), PyValue::None) {
        return Ok(true);
    }
    let text = stripped_lower_text(quant);
    Ok(NATIVE_QUANT_TEXTS.iter().any(|native| text.equals(native)))
}

/// `_build_vram_breakdown_block(breakdown)`: the comment lines of the VRAM breakdown; empty for an absent or empty one.
pub fn build_vram_breakdown_block(breakdown: &PyValue) -> Res<Vec<PyStr>> {
    if !breakdown.is_truthy() {
        return Ok(Vec::new());
    }
    let mut lines = vec![PyStr::new(
        "# ── VRAM 분해 (Phase 2 절대 KV 클램프 기준) ──",
    )];
    for (key, label) in [
        ("weights_gib", "weights"),
        ("kv_gib", "kv_cache"),
        ("overhead_gib", "overhead"),
        ("total_gib", "total"),
        ("budget_gib", "budget"),
        ("headroom_gib", "headroom"),
    ] {
        let value = get(breakdown, key)?;
        if !matches!(value.peel(), PyValue::None) {
            lines.push(
                Message::new()
                    .lit("#   ")
                    .lit(label)
                    .lit(": ")
                    .val(&value)
                    .lit(" GiB")
                    .done(),
            );
        }
    }
    Ok(lines)
}

fn some(value: &PyValue) -> bool {
    !matches!(value.peel(), PyValue::None)
}

/// `"\n".join(lines) + "\n"`.
fn joined(lines: &[PyStr]) -> PyStr {
    let mut text = PyStr::default();
    for (index, line) in lines.iter().enumerate() {
        if index > 0 {
            text.push_str("\n");
        }
        text.push_py(line);
    }
    text.push_str("\n");
    text
}

fn line(text: &str) -> PyStr {
    PyStr::new(text)
}

/// `_build_yaml(parsed, recipe, served_model_name, port=8000)`: the text of `configs/<name>.yaml` (the served model name is
/// accepted and not used).
pub fn build_yaml(parsed: &PyValue, recipe: &PyValue, port: &PyValue) -> Res<PyStr> {
    let container_path = get(parsed, "model_path_container")?;
    let model_id = get_or(parsed, "model_id", PyValue::str(""))?;
    let quant = get(recipe, "quantization")?;
    let gmu = get(recipe, "gpu_memory_utilization")?;
    let max_model_len = get(recipe, "max_model_len")?;
    let batch = get(recipe, "batch")?;
    let kv_bytes = get(recipe, "kv_cache_memory_bytes")?;
    let kv_quant = get(recipe, "kv_cache_quant")?;
    let breakdown = get(recipe, "vram_breakdown")?;

    let mut lines: Vec<PyStr> = Vec::new();
    lines.push(
        Message::new()
            .lit("# ")
            .val(&model_id)
            .lit(" 서빙 설정 (vllm-recipe-explorer 생성)")
            .done(),
    );
    lines.push(
        Message::new()
            .lit("# 레시피: quant=")
            .val(&quant)
            .lit(" max_model_len=")
            .val(&max_model_len)
            .lit(" gpu_mem_util=")
            .val(&gmu)
            .done(),
    );
    let mut provenance: Vec<PyStr> = Vec::new();
    let batch_source = get(recipe, "batch_source")?;
    if batch_source.is_truthy() {
        provenance.push(
            Message::new()
                .lit("max-num-seqs ← batch_source=")
                .val(&batch_source)
                .done(),
        );
    }
    let gmu_source = get(recipe, "gmu_source")?;
    if gmu_source.is_truthy() {
        provenance.push(
            Message::new()
                .lit("gpu-memory-utilization ← gmu_source=")
                .val(&gmu_source)
                .done(),
        );
    }
    if !provenance.is_empty() {
        let mut text = PyStr::new("# 출처(explorer 각인): ");
        for (index, bit) in provenance.iter().enumerate() {
            if index > 0 {
                text.push_str(" · ");
            }
            text.push_py(bit);
        }
        lines.push(text);
    }
    lines.extend(build_vram_breakdown_block(&breakdown)?);
    let target_gpu = get(recipe, "target_gpu")?;
    if target_gpu.is_truthy() {
        let gpu_model = get(&target_gpu, "gpu_model")?;
        lines.push(
            Message::new()
                .lit("# ── 타겟-GPU 이식 클램프 (host≠target, gpu_model=")
                .val(&gpu_model)
                .lit(") ──")
                .done(),
        );
        lines.push(line("# host-측정 weights/overhead 이전값. target≠host arch 면 target-margin 이 쿠션(정량보증 아님)."));
        lines.push(line("# 가능하면 타겟에서 재측정(Phase-2)을 권장. per-token-KV·weights 는 GPU-불변, overhead 는 런타임 성질 일부 포함."));
    }
    lines.push(Message::new().lit("model: ").val(&container_path).done());
    lines.push(line("host: 0.0.0.0"));
    lines.push(Message::new().lit("port: ").val(port).done());
    if some(&kv_bytes) {
        lines.push(line("# gpu-memory-utilization = startup free-memory 게이트(기동 전 free ≥ ceil(total×gmu) · 통합메모리 ≤0.90) + 총량 cap 은 관측된 작용(한 셀 관측 · 유효맥락 kv-clamp.md §1 · vLLM 소스에서 기전 미확정); 실제 KV·이식성은 kv-cache-memory-bytes 절대 클램프가 제어"));
    }
    lines.push(
        Message::new()
            .lit("gpu-memory-utilization: ")
            .val(&gmu)
            .done(),
    );
    lines.push(
        Message::new()
            .lit("max-model-len: ")
            .val(&max_model_len)
            .done(),
    );
    if some(&batch) {
        lines.push(Message::new().lit("max-num-seqs: ").val(&batch).done());
    }
    if some(&kv_bytes) {
        lines.push(
            Message::new()
                .lit("kv-cache-memory-bytes: ")
                .val(&kv_bytes)
                .done(),
        );
    }
    if some(&kv_quant) && !is_native_or_none(&kv_quant)? {
        lines.push(
            Message::new()
                .lit("kv-cache-dtype: ")
                .text(&stripped_lower_text(&kv_quant))
                .done(),
        );
    }
    if !is_native_or_none(&quant)? {
        lines.push(
            Message::new()
                .lit("quantization: ")
                .text(&stripped_lower_text(&quant))
                .done(),
        );
    }
    let tp = get(recipe, "tensor_parallel_size")?;
    if some(&tp) && int_of(&tp)? > BigInt::from(1) {
        lines.push(
            Message::new()
                .lit("tensor-parallel-size: ")
                .int(&int_of(&tp)?)
                .done(),
        );
        let backend = get(recipe, "distributed_executor_backend")?;
        if backend.is_truthy() {
            lines.push(
                Message::new()
                    .lit("distributed-executor-backend: ")
                    .text(&stripped_lower_text(&backend))
                    .done(),
            );
        }
    }
    for (key, flag, is_bool) in [
        ("moe_backend", "moe-backend", false),
        ("gdn_prefill_backend", "gdn-prefill-backend", false),
        ("max_num_batched_tokens", "max-num-batched-tokens", false),
        ("enforce_eager", "enforce-eager", true),
        ("language_model_only", "language-model-only", true),
    ] {
        let value = get(recipe, key)?;
        if is_bool {
            if value.is_truthy() {
                lines.push(Message::new().lit(flag).lit(": true").done());
            }
        } else if some(&value) {
            let text = stripped_lower_text(&value);
            if !["", "auto", "none"].iter().any(|plain| text.equals(plain)) {
                lines.push(Message::new().lit(flag).lit(": ").val(&value).done());
            }
        }
    }
    Ok(joined(&lines))
}

/// `recipe_from_candidate(candidate, **extra)`: the one projection from a trial candidate to the recipe the generator reads;
/// the extra keywords replace or add keys after it.
pub fn recipe_from_candidate(candidate: &PyValue, extra: &[(&PyStr, &PyValue)]) -> Res<PyDict> {
    let mut recipe = PyDict::new();
    for key in SERVE_KNOB_KEYS {
        recipe.set(key, get(candidate, key)?);
    }
    recipe.set("id", get(candidate, "id")?);
    for (key, value) in extra {
        recipe.insert((*key).clone(), (*value).clone());
    }
    Ok(recipe)
}

/// `serve_env_pairs(value)`: the declared serve environment as `(name, value)` pairs of text. A dict is sorted by name; a
/// list or tuple (or one scalar) is read as `NAME=VALUE` items; an empty or absent value is no pairs.
pub fn serve_env_pairs(value: &PyValue) -> Res<Vec<(PyStr, PyStr)>> {
    if !value.is_truthy() {
        return Ok(Vec::new());
    }
    if let PyValue::Dict(map) = value.peel() {
        let mut pairs: Vec<(PyStr, PyStr)> = map
            .keyed_entries()
            .map(|(_, key, item)| (key.clone(), item.display_text()))
            .collect();
        pairs.sort_by(|a, b| crate::model_dyn::compare_text(&a.0, &b.0));
        return Ok(pairs);
    }
    let items: Vec<PyValue> = match value.peel() {
        PyValue::List(items) | PyValue::Tuple(items) => items.clone(),
        other => vec![other.clone()],
    };
    let equals = u32::from('=');
    Ok(items
        .iter()
        .map(|item| {
            let text = item.display_text();
            let points = text.code_points();
            match points.iter().position(|&point| point == equals) {
                Some(at) => (
                    PyStr::from_code_points(points[..at].to_vec()).strip(),
                    PyStr::from_code_points(points[at + 1..].to_vec()),
                ),
                None => (text.strip(), PyStr::default()),
            }
        })
        .collect())
}

/// `_build_sh(name, served_model_name, recipe=None)`: the text of `configs/<name>.sh`.
pub fn build_sh(name: &PyValue, recipe: &PyValue) -> Res<PyStr> {
    let none_recipe = PyValue::Dict(PyDict::new());
    let recipe = if matches!(recipe.peel(), PyValue::None) {
        &none_recipe
    } else {
        recipe
    };
    let attention = get(recipe, "attention_backend")?;
    let tool_parser = get(recipe, "tool_call_parser")?;
    let reasoning_parser = get(recipe, "reasoning_parser")?;
    let mut lines: Vec<PyStr> = vec![line("#!/bin/bash")];
    lines.push(
        Message::new()
            .lit("# ")
            .val(name)
            .lit(" 서빙 스크립트 (vllm-recipe-explorer 생성, gpt-oss-20b-normal.sh 구조 동일)")
            .done(),
    );
    lines.push(line(
        "# env 파일에서 주입된 변수: CONFIG_FILE, SERVING_MODEL_NAME, TIKTOKEN_ENABLED",
    ));
    lines.push(line(""));
    lines.push(line("# TIKTOKEN 환경변수 설정"));
    lines.push(line("if [ \"$TIKTOKEN_ENABLED\" = \"true\" ]; then"));
    lines.push(line("    export TIKTOKEN_ENCODINGS_BASE=/encodings"));
    lines.push(line("    export TIKTOKEN_RS_CACHE_DIR=/encodings"));
    lines.push(line("fi"));
    lines.push(line(""));
    lines.push(line("# 모델구동 런타임 패치 arming (configs/${CONFIG_FILE}_patch.py 존재 시; 메인 저작 arm_patch.sh)"));
    lines.push(line(
        "if [ -f /app/configs/arm_patch.sh ]; then source /app/configs/arm_patch.sh; fi",
    ));
    lines.push(line(""));
    if some(&attention) {
        lines.push(line("# attention backend 고정"));
        lines.push(
            Message::new()
                .lit("export VLLM_ATTENTION_BACKEND=")
                .val(&attention)
                .done(),
        );
        lines.push(line(""));
    }
    let env_pairs = serve_env_pairs(&get(recipe, "serve_env")?)?;
    if !env_pairs.is_empty() {
        lines.push(line("# 선언된 서빙 env (recipe.serve_env)"));
        for (key, value) in &env_pairs {
            lines.push(
                Message::new()
                    .lit("export ")
                    .text(key)
                    .lit("=")
                    .text(value)
                    .done(),
            );
        }
        lines.push(line(""));
    }
    lines.push(line("# vllm serve 실행"));
    let mut serve: Vec<PyStr> = vec![
        line("vllm serve --config \"/app/configs/${CONFIG_FILE}.yaml\" \\"),
        line("    --served-model-name \"$SERVING_MODEL_NAME\""),
    ];
    let continued = |serve: &mut Vec<PyStr>| {
        if let Some(last) = serve.last_mut() {
            last.push_str(" \\");
        }
    };
    if some(&tool_parser) {
        continued(&mut serve);
        serve.push(line("    --enable-auto-tool-choice \\"));
        serve.push(
            Message::new()
                .lit("    --tool-call-parser ")
                .val(&tool_parser)
                .done(),
        );
    }
    if some(&reasoning_parser) {
        continued(&mut serve);
        serve.push(
            Message::new()
                .lit("    --reasoning-parser ")
                .val(&reasoning_parser)
                .done(),
        );
    }
    lines.extend(serve);
    Ok(joined(&lines))
}

/// The `field` names `candidate.get("<field>")` of a text (`re.findall(r'candidate\.get\("([a-z_]+)"\)', text)`), in order of
/// appearance, repeats included.
fn consumed_fields(source: &PyStr) -> Vec<String> {
    let prefix: Vec<u32> = "candidate.get(\"".chars().map(u32::from).collect();
    let points = source.code_points();
    let mut found = Vec::new();
    let mut at = 0;
    while at < points.len() {
        if points[at..].starts_with(&prefix) {
            let start = at + prefix.len();
            let mut end = start;
            while end < points.len()
                && (points[end] == u32::from('_')
                    || (u32::from('a')..=u32::from('z')).contains(&points[end]))
            {
                end += 1;
            }
            if end > start
                && points.get(end) == Some(&u32::from('"'))
                && points.get(end + 1) == Some(&u32::from(')'))
            {
                found.push(
                    points[start..end]
                        .iter()
                        .map(|&point| char::from_u32(point).expect("ASCII"))
                        .collect(),
                );
                at = end + 2;
                continue;
            }
        }
        at += 1;
    }
    found
}

/// `assert_serve_knob_parity(run_trial_source, raise_on_gap=True)`: the fields the trial source reads from a candidate
/// (`candidate.get("<field>")`) that do not appear in the generated files when a probe value is given to them, sorted. With a
/// gap and `raise_on_gap` truthy, the `AssertionError` of the original; otherwise the gaps.
pub fn assert_serve_knob_parity(source: &PyValue, raise_on_gap: &PyValue) -> Res<Vec<PyStr>> {
    let PyValue::Str(source) = source.peel() else {
        return Err(Failure::new(
            "TypeError",
            &format!(
                "expected string or bytes-like object, got '{}'",
                source.peel().type_name()
            ),
        ));
    };
    let mut consumed = consumed_fields(source);
    consumed.sort();
    consumed.dedup();
    let mut parsed = PyDict::new();
    parsed.set("model_id", PyValue::str("probe/model"));
    parsed.set("container_path", PyValue::str("/app/models/probe/model"));
    let parsed = PyValue::Dict(parsed);
    let port = PyValue::int(8000);
    let mut gaps: Vec<PyStr> = Vec::new();
    for field in consumed
        .iter()
        .filter(|field| !PARITY_EXEMPT.contains(&field.as_str()))
    {
        let probe = probe_value(field);
        let mut candidate = PyDict::new();
        candidate.set("id", PyValue::str("probe"));
        candidate.set(field, probe.clone());
        let candidate = PyValue::Dict(candidate);
        let gmu = PyValue::Float(0.9);
        let gmu_key = PyStr::new("gpu_memory_utilization");
        let extra: Vec<(&PyStr, &PyValue)> = if field == "gpu_memory_utilization" {
            Vec::new()
        } else {
            vec![(&gmu_key, &gmu)]
        };
        let recipe = PyValue::Dict(recipe_from_candidate(&candidate, &extra)?);
        let mut blob = build_yaml(&parsed, &recipe, &port)?;
        blob.push_py(&build_sh(&PyValue::str("probe"), &recipe)?);
        let needle = if matches!(probe, PyValue::Bool(true)) {
            PyStr::new("true")
        } else {
            probe.display_text()
        };
        let needle = lower(&needle);
        let haystack = lower(&blob);
        let needle_text = needle.to_utf8().unwrap_or_default();
        if !contains(&haystack, &needle_text) {
            gaps.push(PyStr::new(field));
        }
    }
    if !gaps.is_empty() && raise_on_gap.is_truthy() {
        let names: Vec<String> = gaps.iter().filter_map(PyStr::to_utf8).collect();
        return Err(Failure::new(
            "AssertionError",
            &format!(
                "serve 노브 파리티 위반 — run_trial 이 쓰는데 3종 세트에 도달하지 않는 필드: {}. 검증된 레시피와 배포된 레시피가 갈린다. SERVE_KNOB_KEYS/_build_yaml 에 추가하거나 _PARITY_EXEMPT 에 근거와 함께 등록하라.",
                names.join(", ")
            ),
        ));
    }
    Ok(gaps)
}

/// The arguments of `_build_env`.
pub struct EnvSettings<'a> {
    pub name: &'a PyValue,
    pub served_model_name: &'a PyValue,
    pub port: &'a PyValue,
    pub image: &'a PyValue,
    pub topology: &'a PyValue,
    pub vllm_version: &'a PyValue,
    pub build_dockerfile: &'a PyValue,
}

/// `_build_env(name, served_model_name, port, image=None, topology="single", vllm_version=None, build_dockerfile=None)`: the
/// text of `envs/.env.<name>`. The warnings the original writes to standard error while building are appended to `stderr`.
pub fn build_env(settings: &EnvSettings<'_>, stderr: &mut PyStr) -> Res<PyStr> {
    let EnvSettings {
        name,
        served_model_name,
        port,
        image,
        topology,
        vllm_version,
        build_dockerfile,
    } = *settings;
    let mut lines: Vec<PyStr> = Vec::new();
    let separator: String = format!("# {}", "═".repeat(69));
    lines.push(line(&separator));
    lines.push(
        Message::new()
            .lit("# vLLM 서버 환경 설정 — ")
            .val(name)
            .lit(" (vllm-recipe-explorer 생성)")
            .done(),
    );
    lines.push(line(&separator));
    lines.push(line(""));
    lines.push(line(
        "# ─────────────── 0) 프로젝트, 컨테이너, env파일 이름 ─────────────────────",
    ));
    lines.push(
        Message::new()
            .lit("COMPOSE_PROJECT_NAME=vllm_")
            .val(name)
            .lit("_project")
            .done(),
    );
    let multi = is_text(topology, "multi");
    if multi {
        lines.push(
            Message::new()
                .lit("MASTER_CONTAINER_NAME=")
                .val(name)
                .lit("-master-container")
                .done(),
        );
        lines.push(
            Message::new()
                .lit("SLAVE_CONTAINER_NAME=")
                .val(name)
                .lit("-slave-container")
                .done(),
        );
    } else {
        lines.push(
            Message::new()
                .lit("CONTAINER_NAME=")
                .val(name)
                .lit("-serving-container")
                .done(),
        );
    }
    lines.push(line("VERSION=1.0.0"));
    lines.push(line("NVIDIA_VISIBLE_DEVICES=all"));
    lines.push(line(""));
    lines.push(line(
        "# ─────────────── 1) 외부 노출 설정 (docker-compose 포트매핑) ─────────────",
    ));
    lines.push(line("SERVING_IP=0.0.0.0"));
    lines.push(Message::new().lit("SERVING_PORT=").val(port).done());
    lines.push(line(""));
    lines.push(line(
        "# ─────────────── 2) LLM 서빙 설정 ─────────────────────────────────────",
    ));
    lines.push(line("TIKTOKEN_ENABLED=true"));
    lines.push(
        Message::new()
            .lit("SERVING_MODEL_NAME=")
            .val(served_model_name)
            .done(),
    );
    lines.push(Message::new().lit("CONFIG_FILE=").val(name).done());
    lines.push(line(""));
    lines.push(line(
        "# ─────────────── 3) 컨테이너 이미지 (필수) ───────────────────────────",
    ));
    lines.push(line(
        "# 비우면 compose 가 낡은 기본값으로 조용히 폴백해 **다른 vLLM 버전을 측정**한다.",
    ));
    if image.is_truthy() {
        lines.push(Message::new().lit("IMAGE_TAG=").val(image).done());
    } else {
        lines.push(line("# IMAGE_TAG=<미지정 — 반드시 채울 것>"));
        stderr.push_str("[gen_recipe_set] WARN: image 미지정 → env 에 IMAGE_TAG 를 쓰지 못했다. compose 가 낡은 기본 이미지로 폴백하므로 서빙 전에 직접 채워라.\n");
    }
    if multi {
        lines.push(line(""));
        lines.push(line(
            "# ─────────────── 4) 분산(멀티노드) 전용 ──────────────────────────────",
        ));
        if build_dockerfile.is_truthy() {
            lines.push(
                Message::new()
                    .lit("BUILD_DOCKERFILE=")
                    .val(build_dockerfile)
                    .done(),
            );
        } else {
            lines.push(line("# BUILD_DOCKERFILE=<미지정 — 반드시 채울 것>"));
            stderr
                .push_str("[gen_recipe_set] WARN: topology=multi 인데 build_dockerfile 미지정.\n");
        }
        if vllm_version.is_truthy() {
            lines.push(Message::new().lit("VLLM_VERSION=").val(vllm_version).done());
        } else {
            lines.push(line("# VLLM_VERSION=<미지정 — 반드시 채울 것>"));
            stderr.push_str("[gen_recipe_set] WARN: topology=multi 인데 vllm_version 미지정 — 이미지 정체성 변수다(틀리면 다른 버전을 측정하고도 모른다).\n");
        }
    }
    Ok(joined(&lines))
}

// ── the file system, as Python's `os` and `open` see it ──────────────────────────────────────────

/// The path as the operating system sees it (an embedded NUL is the `ValueError` of the system call wrappers).
pub fn native_path(path: &PyStr) -> Res<PathBuf> {
    use std::os::unix::ffi::OsStrExt;
    let bytes = path_bytes(path)?;
    if bytes.contains(&0) {
        return Err(Failure::new("ValueError", "embedded null byte"));
    }
    Ok(PathBuf::from(std::ffi::OsStr::from_bytes(&bytes)))
}

/// `posixpath.split(path)`.
pub fn split_path(path: &PyStr) -> (PyStr, PyStr) {
    let slash = u32::from('/');
    let points = path.code_points();
    let cut = points
        .iter()
        .rposition(|&point| point == slash)
        .map_or(0, |at| at + 1);
    let mut head: Vec<u32> = points[..cut].to_vec();
    if !head.is_empty() && !head.iter().all(|&point| point == slash) {
        while head.last() == Some(&slash) {
            head.pop();
        }
    }
    (
        PyStr::from_code_points(head),
        PyStr::from_code_points(points[cut..].to_vec()),
    )
}

/// `os.makedirs(name, exist_ok=True)` with the default mode: the missing parents are created, a directory that is already
/// there is fine, anything else is the `OSError` of the failing `mkdir` (the path in the message is the one that failed).
pub fn makedirs(name: &PyStr) -> Res<()> {
    let (mut head, mut tail) = split_path(name);
    if tail.is_empty() {
        (head, tail) = split_path(&head);
    }
    if !head.is_empty() && !tail.is_empty() && !exists(&head) {
        match makedirs(&head) {
            Ok(()) => {}
            Err(failure) if failure.class == "FileExistsError" => {}
            Err(failure) => return Err(failure),
        }
        if tail.equals(".") {
            return Ok(());
        }
    }
    let native = native_path(name)?;
    match std::fs::DirBuilder::new().mode(0o777).create(&native) {
        Ok(()) => Ok(()),
        Err(error) => {
            if is_dir(name) {
                Ok(())
            } else {
                Err(os_failure(&error, Some(name)))
            }
        }
    }
}

/// `text.encode("utf-8")` as a text file's `write` does it: any lone surrogate is a `UnicodeEncodeError`, reported for the
/// first run of them.
pub fn strict_utf8(text: &PyStr) -> Res<Vec<u8>> {
    let points = text.code_points();
    let is_surrogate = |point: u32| (0xD800..=0xDFFF).contains(&point);
    if let Some(start) = points.iter().position(|&point| is_surrogate(point)) {
        let end = start
            + points[start..]
                .iter()
                .take_while(|&&point| is_surrogate(point))
                .count();
        let failure = EncodeFailure {
            start,
            end,
            first: points[start],
        };
        return Err(Failure::new("UnicodeEncodeError", &failure.message()));
    }
    Ok(text.to_utf8().expect("no surrogate").into_bytes())
}

/// `with open(path, mode, encoding="utf-8") as f: f.write(text)` for the modes `"w"` (create or truncate) and `"a"`
/// (create or append): the file is opened (and so created or truncated) first, then the text is encoded; an encoding
/// failure leaves the file as the open left it.
pub fn write_text(path: &PyStr, text: &PyStr, append: bool) -> Res<()> {
    let native = native_path(path)?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).mode(0o666);
    if append {
        options.append(true);
    } else {
        options.truncate(true);
    }
    let mut file = options
        .open(&native)
        .map_err(|error| os_failure(&error, Some(path)))?;
    let bytes = strict_utf8(text)?;
    file.write_all(&bytes)
        .and_then(|()| file.flush())
        .map_err(|error| os_failure(&error, None))
}

/// `generate(parsed, recipe, name, repo_root, port, served_model_name, force=False, topology="single", vllm_version=None,
/// build_dockerfile=None, image=None)`: the three files of a serving set; the paths of `[yaml, sh, env]`.
pub struct Request<'a> {
    pub parsed: &'a PyValue,
    pub recipe: &'a PyValue,
    pub name: &'a PyValue,
    pub repo_root: &'a PyValue,
    pub port: &'a PyValue,
    pub served_model_name: &'a PyValue,
    pub force: &'a PyValue,
    pub topology: &'a PyValue,
    pub vllm_version: &'a PyValue,
    pub build_dockerfile: &'a PyValue,
    pub image: &'a PyValue,
}

pub fn generate(request: &Request<'_>, stderr: &mut PyStr) -> Res<Vec<PyStr>> {
    let configs = PyValue::str("configs");
    let envs = PyValue::str("envs");
    let configs_dir = join_values(request.repo_root, &configs)?;
    let envs_dir = join_values(request.repo_root, &envs)?;
    makedirs(&configs_dir)?;
    makedirs(&envs_dir)?;
    let name = request.name.display_text();
    let mut yaml_name = name.clone();
    yaml_name.push_str(".yaml");
    let mut sh_name = name.clone();
    sh_name.push_str(".sh");
    let mut env_name = PyStr::new(".env.");
    env_name.push_py(&name);
    let yaml_path = join_values(&PyValue::Str(configs_dir.clone()), &PyValue::Str(yaml_name))?;
    let sh_path = join_values(&PyValue::Str(configs_dir), &PyValue::Str(sh_name))?;
    let env_path = join_values(&PyValue::Str(envs_dir), &PyValue::Str(env_name))?;
    let targets = [yaml_path, sh_path, env_path];
    if !request.force.is_truthy() {
        let existing: Vec<&PyStr> = targets.iter().filter(|path| exists(path)).collect();
        if !existing.is_empty() {
            let mut message = PyStr::new("이미 존재하는 파일(덮어쓰려면 --force): ");
            for (index, path) in existing.iter().enumerate() {
                if index > 0 {
                    message.push_str(", ");
                }
                message.push_py(path);
            }
            return Err(Failure::with_message("FileExistsError", message));
        }
    }
    let yaml_text = build_yaml(request.parsed, request.recipe, request.port)?;
    let sh_text = build_sh(request.name, request.recipe)?;
    let env_text = build_env(
        &EnvSettings {
            name: request.name,
            served_model_name: request.served_model_name,
            port: request.port,
            image: request.image,
            topology: request.topology,
            vllm_version: request.vllm_version,
            build_dockerfile: request.build_dockerfile,
        },
        stderr,
    )?;
    write_text(&targets[0], &yaml_text, false)?;
    write_text(&targets[1], &sh_text, false)?;
    write_text(&targets[2], &env_text, false)?;
    if let Ok(native) = native_path(&targets[1]) {
        // `os.chmod` failures (an `OSError`) are the one error the original swallows.
        let _ = std::fs::set_permissions(native, std::fs::Permissions::from_mode(0o755));
    }
    Ok(Vec::from(targets))
}

/// `_load_json(path)`: `json.load(open(path, "r", encoding="utf-8"))`. (A path given as an integer would be a file descriptor
/// for `open`; that transport is not supported and is the `TypeError` of any non-path.)
pub fn load_json(path: &PyValue) -> Res<PyValue> {
    let path = crate::model_files::fspath(path, false)?;
    crate::model_files::load_json_file(path)
}

/// Whether the failure is an `OSError` of any class (what an `except OSError` catches).
pub fn is_os_failure(failure: &Failure) -> bool {
    is_os_error(failure.class)
}
