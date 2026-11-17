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

/// The port vLLM listens on inside the container of the single compose: its `vllm-serve` service publishes
/// `"${SERVING_PORT:-8080}:8000"` over the bridge network, so the container side of that mapping is the owner of this value
/// and [`compose_port_parity`] fails whenever a tracked compose source says otherwise. `SERVING_PORT` (the env file) stays the
/// host side. plan_26111708 B L4 (10-14 F1 재발: 생성기가 호스트 포트를 yaml `port:` 에 넣어 health 불가).
pub const SINGLE_COMPOSE_CONTAINER_PORT: i64 = 8000;

/// The compose sources that serve the single topology (repo-relative): the template first (required), then the files rendered
/// or kept from it (checked when present on this checkout — the other branch's channel may be absent).
pub const SINGLE_COMPOSE_SOURCES: [&str; 3] = [
    ".claude/skills/upstream-version-watch/templates/docker-compose.single.template.yaml",
    "docker-compose.template.yaml",
    "output/single/docker-compose.yaml",
];

/// The compose sources that serve the multi topology (repo-relative), the template first (required).
pub const MULTI_COMPOSE_SOURCES: [&str; 2] = [
    ".claude/skills/upstream-version-watch/templates/docker-compose.multi.template.yaml",
    "output/multi/docker-compose.yaml",
];

/// `listen_port(topology, host_port)`: the `port:` the serving yaml gives vLLM. single (and any non-`multi` text, as
/// [`build_env`] reads it): the container side of the compose port mapping, [`SINGLE_COMPOSE_CONTAINER_PORT`]. multi: the host
/// port itself — the multi compose runs `network_mode: host` and publishes no mapping, so vLLM must listen on `SERVING_PORT`
/// directly (the multi smoke polls that port). plan_26111708 B L4.
pub fn listen_port(topology: &PyValue, host_port: &PyValue) -> PyValue {
    if is_text(topology, "multi") {
        host_port.clone()
    } else {
        PyValue::int(SINGLE_COMPOSE_CONTAINER_PORT)
    }
}

/// A compose line without its trailing `# comment` and surrounding white space.
fn compose_line(line: &str) -> &str {
    line.split(" #").next().unwrap_or("").trim()
}

/// The `ports:` items that publish `SERVING_PORT` (`- "${SERVING_PORT:-8080}:8000"`, quoted or not), as `(line number, item)`.
fn serving_port_items(text: &str) -> Vec<(usize, String)> {
    text.lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let item = compose_line(line).strip_prefix('-')?.trim();
            let item = item.trim_matches(|c| c == '"' || c == '\'');
            item.starts_with("${SERVING_PORT").then(|| (index + 1, item.to_string()))
        })
        .collect()
}

/// The container side of the one `SERVING_PORT` mapping of a single compose text; an error names what is wrong (no mapping,
/// more than one, a container side that is not a number).
pub fn single_serving_container_port(text: &str) -> Result<i64, String> {
    let items = serving_port_items(text);
    let [(line, item)] = items.as_slice() else {
        return Err(format!(
            "SERVING_PORT 포트매핑이 정확히 1개여야 한다(발견 {}개: {:?})",
            items.len(),
            items
        ));
    };
    let container = item.rsplit(':').next().unwrap_or("");
    container
        .parse::<i64>()
        .map_err(|_| format!("{line}행 '{item}' 의 컨테이너 쪽 '{container}' 가 포트 번호가 아니다"))
}

/// Whether a multi compose text serves on the host network with no published `SERVING_PORT` mapping (what makes the yaml
/// `port:` equal to the host port); an error names the missing or contradicting line.
pub fn multi_serving_host_network(text: &str) -> Result<(), String> {
    if !text.lines().any(|line| compose_line(line) == "network_mode: host") {
        return Err("`network_mode: host` 가 없다 — multi yaml port 를 호스트 포트로 둘 근거가 사라졌다".to_string());
    }
    match serving_port_items(text).first() {
        Some((line, item)) => Err(format!(
            "{line}행 '{item}' — host 네트워크인데 SERVING_PORT 를 포트매핑한다(컨테이너 포트와 호스트 포트가 갈린다)"
        )),
        None => Ok(()),
    }
}

/// The parity of [`listen_port`] with the compose sources below `repo_root`: every present single source maps `SERVING_PORT`
/// to [`SINGLE_COMPOSE_CONTAINER_PORT`] and every present multi source runs on the host network. A missing template (the first
/// entry of each list) is a failure; a missing rendered file is listed as absent. `Ok` carries one line per source looked at,
/// `Err` every disagreement (plan_26111708 B L4 — the constant has one owner, the compose mapping).
pub fn compose_port_parity(repo_root: &std::path::Path) -> Result<Vec<String>, Vec<String>> {
    let mut seen = Vec::new();
    let mut problems = Vec::new();
    let sources = SINGLE_COMPOSE_SOURCES
        .iter()
        .enumerate()
        .map(|(index, path)| (*path, index == 0, true))
        .chain(
            MULTI_COMPOSE_SOURCES
                .iter()
                .enumerate()
                .map(|(index, path)| (*path, index == 0, false)),
        );
    for (relative, required, single) in sources {
        let text = match std::fs::read_to_string(repo_root.join(relative)) {
            Ok(text) => text,
            Err(error) if required => {
                problems.push(format!("{relative}: 읽지 못했다({error}) — 포트 소유자(템플릿)가 없다"));
                continue;
            }
            Err(_) => {
                seen.push(format!("{relative}: 부재(이 체크아웃에 없음)"));
                continue;
            }
        };
        let verdict = if single {
            single_serving_container_port(&text).and_then(|port| {
                if port == SINGLE_COMPOSE_CONTAINER_PORT {
                    Ok(format!("{relative}: SERVING_PORT → 컨테이너 {port}"))
                } else {
                    Err(format!(
                        "컨테이너 포트 {port} ≠ SINGLE_COMPOSE_CONTAINER_PORT {SINGLE_COMPOSE_CONTAINER_PORT}"
                    ))
                }
            })
        } else {
            multi_serving_host_network(&text).map(|()| format!("{relative}: network_mode host · 포트매핑 없음"))
        };
        match verdict {
            Ok(line) => seen.push(line),
            Err(why) => problems.push(format!("{relative}: {why}")),
        }
    }
    if problems.is_empty() {
        Ok(seen)
    } else {
        Err(problems)
    }
}

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

/// Every `[a-z_]+` name that follows `prefix` and is followed by one of `after` in `points`, in order of appearance, repeats
/// included; a match resumes the scan after the matched `after` text.
fn names_between(points: &[u32], prefix: &str, after: &[&str]) -> Vec<String> {
    let prefix: Vec<u32> = prefix.chars().map(u32::from).collect();
    let after: Vec<Vec<u32>> = after
        .iter()
        .map(|text| text.chars().map(u32::from).collect())
        .collect();
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
            if end > start {
                if let Some(tail) = after.iter().find(|tail| points[end..].starts_with(tail)) {
                    found.push(
                        points[start..end]
                            .iter()
                            .map(|&point| char::from_u32(point).expect("ASCII"))
                            .collect(),
                    );
                    at = end + tail.len();
                    continue;
                }
            }
        }
        at += 1;
    }
    found
}

/// The `field` names `candidate.get("<field>")` of a text (`re.findall(r'candidate\.get\("([a-z_]+)"\)', text)`), in order of
/// appearance, repeats included.
pub fn consumed_fields(source: &PyStr) -> Vec<String> {
    names_between(source.code_points(), "candidate.get(\"", &["\")"])
}

/// The candidate fields the Rust trial owner reads, in order of appearance, repeats included: the accessor calls
/// `get(candidate, "<field>")` / `truthy(&candidate, "<field>")` (any call whose arguments start `candidate, "<field>"`) and the
/// field-to-flag tables `("<field>", "--<flag>")` / `("<field>", "VLLM_<NAME>=")` the trial iterates over with `get(candidate, key)`,
/// and the dictionary form `candidate.get("<field>")`.
pub fn rust_trial_consumed_fields(source: &PyStr) -> Vec<String> {
    let points = source.code_points();
    let mut found = names_between(points, "candidate, \"", &["\""]);
    found.extend(names_between(points, "(\"", &["\", \"--", "\", \"VLLM_"]));
    found.extend(names_between(points, "candidate.get(\"", &["\")"]));
    found
}

/// The parity check over the Rust trial owner's sources (`gen_recipe_set --check-parity`): the gaps of
/// [`rust_trial_consumed_fields`] of the joined text, sorted, never raised.
pub fn rust_trial_serve_knob_gaps(source: &PyStr) -> Res<Vec<PyStr>> {
    serve_knob_gaps(rust_trial_consumed_fields(source), &PyValue::Bool(false))
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
    serve_knob_gaps(consumed_fields(source), raise_on_gap)
}

/// The fields of `consumed` that do not reach the generated files when a probe value is given to them, sorted (exempt fields
/// skipped). With a gap and `raise_on_gap` truthy, the original's `AssertionError`; otherwise the gaps.
fn serve_knob_gaps(mut consumed: Vec<String>, raise_on_gap: &PyValue) -> Res<Vec<PyStr>> {
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

/// The build coordinates a stock image tag implies (see [`stock_build_coordinates`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildCoordinates {
    /// `BUILD_DOCKERFILE` (`Dockerfile` for the wheel track, `Dockerfile.source-build` for the source track).
    pub dockerfile: &'static str,
    /// The version identity key of the track: `VLLM_VERSION` (wheel) or `VLLM_REF` (source).
    pub version_key: &'static str,
    /// Its value: the tag's version (wheel) or `v<version>` (source).
    pub version_value: String,
}

/// The build coordinates a **stock** image tag implies under the image naming invariant `easy-vllm:{vllm}-cu{cuda}-{arch}-{track}`
/// (CLAUDE.md §이미지 네이밍): exactly the pair the multi smoke's track and version gates demand before a build
/// (`upstream-version-watch/scripts/multinode_serve_smoke.sh` §빌드 트랙 ↔ 이미지 태그 정합 · §버전 정합: wheel →
/// `BUILD_DOCKERFILE=Dockerfile` + `VLLM_VERSION=<ver>`; source → `BUILD_DOCKERFILE=Dockerfile.source-build` + `VLLM_REF=v<ver>`;
/// the version is the `[0-9][0-9.]*` piece the gate reads). Any other tag — a variant or fork suffix (`-source-sm12x`,
/// `-source-deps2`, `-source-fork<PR>`), a pre-release version, a foreign name — gives `None`: those coordinates live in
/// `.claude/policies/arch_variant_ledger.json` or with the person, not in the tag (plan_26111708 B L10).
pub fn stock_build_coordinates(image: &str) -> Option<BuildCoordinates> {
    let rest = image.strip_prefix("easy-vllm:")?;
    let parts: Vec<&str> = rest.split('-').collect();
    let [version, cuda, arch, track] = parts.as_slice() else {
        return None;
    };
    let version_ok = version.starts_with(|c: char| c.is_ascii_digit())
        && version.chars().all(|c| c.is_ascii_digit() || c == '.');
    let cuda_ok = cuda
        .strip_prefix("cu")
        .map_or(false, |digits| !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()));
    let arch_ok = !arch.is_empty() && arch.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !(version_ok && cuda_ok && arch_ok) {
        return None;
    }
    match *track {
        "wheel" => Some(BuildCoordinates {
            dockerfile: "Dockerfile",
            version_key: "VLLM_VERSION",
            version_value: version.to_string(),
        }),
        "source" => Some(BuildCoordinates {
            dockerfile: "Dockerfile.source-build",
            version_key: "VLLM_REF",
            version_value: format!("v{version}"),
        }),
        _ => None,
    }
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
    /// The source track's version identity (`VLLM_REF` of the multi compose build args); `None` when not given
    /// (plan_26111708 B L10).
    pub vllm_ref: &'a PyValue,
}

/// `_build_env(name, served_model_name, port, image=None, topology="single", vllm_version=None, build_dockerfile=None)`: the
/// text of `envs/.env.<name>`. The warnings the original writes to standard error while building are appended to `stderr`.
///
/// plan_26111708 B L10: the original's unconditional `TIKTOKEN_ENABLED=true` is no longer written — no model-specific ground
/// was found for it (vLLM 0.32.0 reads no `TIKTOKEN_*` variable itself), and both compose templates already set
/// `TIKTOKEN_ENCODINGS_BASE=/encodings` for the serving services (`docker-compose.single.template.yaml` `vllm-serve.environment`,
/// `docker-compose.multi.template.yaml` `vllm-master-serve.environment`). The generated `.sh` still honours the variable when a
/// person sets it. For multi, the build coordinates a stock `image` tag implies fill the keys that were not given explicitly
/// ([`stock_build_coordinates`], marked by a provenance comment line).
pub fn build_env(settings: &EnvSettings<'_>, stderr: &mut PyStr) -> Res<PyStr> {
    let EnvSettings {
        name,
        served_model_name,
        port,
        image,
        topology,
        vllm_version,
        build_dockerfile,
        vllm_ref,
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
        let derived = match image.peel() {
            PyValue::Str(tag) if image.is_truthy() => tag.to_utf8().and_then(|t| stock_build_coordinates(&t)),
            _ => None,
        };
        let version_given = vllm_version.is_truthy() || vllm_ref.is_truthy();
        let mut derived_keys: Vec<&str> = Vec::new();
        if let Some(coords) = &derived {
            if !build_dockerfile.is_truthy() {
                derived_keys.push("BUILD_DOCKERFILE");
            }
            if !version_given {
                derived_keys.push(coords.version_key);
            }
        }
        if !derived_keys.is_empty() {
            lines.push(line(&format!(
                "# 빌드 좌표 출처(IMAGE_TAG 파생 · 이미지 네이밍 불변식 easy-vllm:{{vllm}}-cu{{cuda}}-{{arch}}-{{track}} · multinode_serve_smoke 트랙/버전 게이트가 요구하는 짝 · plan_26111708 B L10): {}",
                derived_keys.join(", ")
            )));
        }
        if build_dockerfile.is_truthy() {
            lines.push(
                Message::new()
                    .lit("BUILD_DOCKERFILE=")
                    .val(build_dockerfile)
                    .done(),
            );
        } else if let Some(coords) = &derived {
            lines.push(line(&format!("BUILD_DOCKERFILE={}", coords.dockerfile)));
        } else {
            lines.push(line("# BUILD_DOCKERFILE=<미지정 — 반드시 채울 것>"));
            stderr
                .push_str("[gen_recipe_set] WARN: topology=multi 인데 build_dockerfile 미지정.\n");
        }
        if vllm_version.is_truthy() {
            lines.push(Message::new().lit("VLLM_VERSION=").val(vllm_version).done());
        }
        if vllm_ref.is_truthy() {
            lines.push(Message::new().lit("VLLM_REF=").val(vllm_ref).done());
        }
        if !version_given {
            if let Some(coords) = &derived {
                lines.push(line(&format!("{}={}", coords.version_key, coords.version_value)));
            } else {
                lines.push(line("# VLLM_VERSION=<미지정 — 반드시 채울 것>"));
                lines.push(line("# VLLM_REF=<미지정 — source 트랙이면 VLLM_VERSION 대신 이것이 이미지 정체성이다(compose multi 빌드 args)>"));
                stderr.push_str("[gen_recipe_set] WARN: topology=multi 인데 vllm_version 미지정 — 이미지 정체성 변수다(틀리면 다른 버전을 측정하고도 모른다).\n");
            }
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
///
/// `port` is the host port (`SERVING_PORT` of the env); the yaml gets [`listen_port`] of it (plan_26111708 B L4).
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
    /// See [`EnvSettings::vllm_ref`].
    pub vllm_ref: &'a PyValue,
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
    // plan_26111708 B L4: the yaml gets the port vLLM listens on (container side for single, the host port for host-network
    // multi); the env keeps the host port as SERVING_PORT.
    let yaml_text = build_yaml(
        request.parsed,
        request.recipe,
        &listen_port(request.topology, request.port),
    )?;
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
            vllm_ref: request.vllm_ref,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_root() -> std::path::PathBuf {
        // `.claude/skills/vllm-recipe-explorer/rust/runtime` → the repository root.
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(5).expect("repo root").to_path_buf()
    }

    fn env_text(topology: &str, image: PyValue, version: PyValue, dockerfile: PyValue, vllm_ref: PyValue) -> (String, String) {
        let mut stderr = PyStr::default();
        let text = build_env(
            &EnvSettings {
                name: &PyValue::str("cell"),
                served_model_name: &PyValue::str("served"),
                port: &PyValue::int(18080),
                image: &image,
                topology: &PyValue::str(topology),
                vllm_version: &version,
                build_dockerfile: &dockerfile,
                vllm_ref: &vllm_ref,
            },
            &mut stderr,
        )
        .expect("env");
        (text.to_utf8().expect("utf8"), stderr.to_utf8().expect("utf8"))
    }

    /// plan_26111708 B L4: single listens on the compose container side, multi (host network) on the host port.
    #[test]
    fn the_yaml_port_is_the_listen_port_of_the_topology() {
        let host = PyValue::int(18080);
        assert_eq!(listen_port(&PyValue::str("single"), &host), PyValue::int(SINGLE_COMPOSE_CONTAINER_PORT));
        assert_eq!(listen_port(&PyValue::str("other"), &host), PyValue::int(SINGLE_COMPOSE_CONTAINER_PORT));
        assert_eq!(listen_port(&PyValue::str("multi"), &host), host);
    }

    /// plan_26111708 B L4: the constant agrees with every tracked compose source of this checkout (one owner: the mapping).
    #[test]
    fn the_container_port_constant_matches_the_compose_mapping() {
        match compose_port_parity(&repo_root()) {
            Ok(seen) => assert!(seen.iter().any(|line| line.contains("docker-compose.single.template.yaml")), "{seen:?}"),
            Err(problems) => panic!("compose 포트 파리티 위반: {problems:?}"),
        }
    }

    #[test]
    fn a_compose_that_disagrees_with_the_constant_is_named() {
        let mapped = "    ports:\n      - \"${SERVING_PORT:-8080}:9000\"\n";
        assert_eq!(single_serving_container_port(mapped), Ok(9000));
        assert!(single_serving_container_port("    ports:\n      - \"8081:8000\" # debug\n").is_err());
        let two = format!("{mapped}{mapped}");
        assert!(single_serving_container_port(&two).is_err());
        assert!(single_serving_container_port("      - \"${SERVING_PORT:-8080}:abc\"\n").is_err());
        assert_eq!(multi_serving_host_network("  network_mode: host\n    environment:\n      SERVING_PORT: ${SERVING_PORT:-8080}\n"), Ok(()));
        assert!(multi_serving_host_network("  ipc: host\n").is_err());
        assert!(multi_serving_host_network("  network_mode: host\n    ports:\n      - \"${SERVING_PORT:-8080}:8000\"\n").is_err());
    }

    /// plan_26111708 B L10: only a stock tag implies build coordinates (the multi smoke's gate pair).
    #[test]
    fn stock_tags_imply_the_gate_pair_and_variants_do_not() {
        let source = stock_build_coordinates("easy-vllm:0.32.0-cu133-aarch64-source").expect("stock source");
        assert_eq!((source.dockerfile, source.version_key, source.version_value.as_str()), ("Dockerfile.source-build", "VLLM_REF", "v0.32.0"));
        let wheel = stock_build_coordinates("easy-vllm:0.18.0-cu132-x86_64-wheel").expect("stock wheel");
        assert_eq!((wheel.dockerfile, wheel.version_key, wheel.version_value.as_str()), ("Dockerfile", "VLLM_VERSION", "0.18.0"));
        for tag in [
            "easy-vllm:0.32.0-cu133-aarch64-source-deps2",
            "easy-vllm:0.23.0-cu132-aarch64-source-sm12x",
            "easy-vllm:0.29.0rc6-cu133-aarch64-source",
            "easy-vllm:0.32.0-cu133-aarch64-nightly",
            "vllm-src-022:clean",
            "image:tag",
        ] {
            assert_eq!(stock_build_coordinates(tag), None, "{tag}");
        }
    }

    #[test]
    fn the_env_drops_the_unconditional_tiktoken_flag_and_keeps_the_host_port() {
        let (single, _) = env_text("single", PyValue::str("img:tag"), PyValue::None, PyValue::None, PyValue::None);
        assert!(!single.contains("TIKTOKEN_ENABLED"), "{single}");
        assert!(single.contains("\nSERVING_PORT=18080\n") && single.contains("\nCONTAINER_NAME=cell-serving-container\n"));
        assert!(!single.contains("MASTER_CONTAINER_NAME"));
    }

    #[test]
    fn a_multi_env_carries_the_master_slave_names_and_the_build_coordinates() {
        let (derived, warnings) = env_text("multi", PyValue::str("easy-vllm:0.32.0-cu133-aarch64-source"), PyValue::None, PyValue::None, PyValue::None);
        for line in [
            "MASTER_CONTAINER_NAME=cell-master-container",
            "SLAVE_CONTAINER_NAME=cell-slave-container",
            "BUILD_DOCKERFILE=Dockerfile.source-build",
            "VLLM_REF=v0.32.0",
            "SERVING_PORT=18080",
        ] {
            assert!(derived.lines().any(|l| l == line), "{line}\n{derived}");
        }
        assert!(derived.contains("빌드 좌표 출처(IMAGE_TAG 파생") && derived.contains("): BUILD_DOCKERFILE, VLLM_REF\n"), "{derived}");
        assert!(!derived.contains("VLLM_VERSION=") && warnings.is_empty(), "{warnings}");
        // explicit values win and are not marked as derived
        let (explicit, _) = env_text(
            "multi",
            PyValue::str("easy-vllm:0.32.0-cu133-aarch64-source"),
            PyValue::None,
            PyValue::str("Dockerfile.source-build-deps2"),
            PyValue::str("5df63a7203cdb9328d824a27b682e77c7b9928be"),
        );
        assert!(explicit.contains("\nBUILD_DOCKERFILE=Dockerfile.source-build-deps2\n") && explicit.contains("\nVLLM_REF=5df63a7203cdb9328d824a27b682e77c7b9928be\n"));
        assert!(!explicit.contains("빌드 좌표 출처"), "{explicit}");
        // a variant tag implies nothing: explicit placeholders and the warnings, never a guessed stock coordinate
        let (variant, warnings) = env_text("multi", PyValue::str("easy-vllm:0.32.0-cu133-aarch64-source-deps2"), PyValue::None, PyValue::None, PyValue::None);
        assert!(variant.contains("# BUILD_DOCKERFILE=<미지정") && variant.contains("# VLLM_REF=<미지정") && !variant.contains("\nVLLM_REF="), "{variant}");
        assert!(warnings.contains("build_dockerfile 미지정") && warnings.contains("vllm_version 미지정"), "{warnings}");
    }
}
