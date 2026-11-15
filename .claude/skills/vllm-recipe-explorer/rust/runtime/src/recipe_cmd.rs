//! `recipe.py estimate`: the orchestrator command line of the vllm-recipe-explorer skill, ported for the one subcommand a caller
//! starts today (`smoke_clone.sh` A8, the fresh-clone Flag gate). The order is the original's: the terraforming-Flag gate
//! (exit 4/5) runs before the configuration is read, then `estimate` parses the model, ranks the candidates in process through the
//! package's own `model_config`, `rank_recipes` and `estimate_vram` ports, prints the report and saves `feedback/.last_ranking.json`.
//!
//! `generate` (the `.last_ranking.json` entry → the three-file serving set + a feedback line) is ported; `simulate` is NOT yet (it
//! needs `run_trial`, the lockset writer and the budget session): it stops with status 2 and names the original program, never a
//! silent substitute. `--help`/argument errors of the top level are approximate:
//! the sub-command parsers are the original's (through `cliparse`), the top-level usage line is a fixed text.

use crate::cli::Run;
use crate::cliparse::{parse as parse_arguments, Kind, Opt, Outcome, Spec, Value};
use crate::feedback::{self, dirname, Clock};
use crate::recipe_set::{self, Request};
use crate::model_cli::Streams;
use crate::model_config;
use crate::model_dyn::Message;
use crate::model_files::{is_dir, is_file, join, load_json_file, read_bytes};
use crate::pyexpr::{float_of, get, get_or, int_of, is_none, Res};
use crate::rank_recipes::{auto_candidates, rank, render_report};
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use agent_runtime_support::pyyaml;
use num_bigint::BigInt;
use std::process::Command;

const DEFAULT_NAS_HOST_ROOT: &str = "/mnt/models";
const DEFAULT_SAFETY_MARGIN: f64 = 0.90;
const DEFAULT_KV_BYTES: i64 = 2;

/// `_die(msg, code)`; code -1 marks an uncaught exception line (status 1, no `[recipe] 중단:` prefix).
struct Die {
    code: i32,
    text: PyStr,
}

fn die(code: i32, text: &str) -> Die {
    Die { code, text: PyStr::new(text) }
}

fn die_py(code: i32, text: PyStr) -> Die {
    Die { code, text }
}

fn raised(failure: Failure) -> Die {
    Die { code: -1, text: failure.line() }
}

type Step<T> = Result<T, Die>;

fn stop(mut streams: Streams, d: Die) -> Run {
    streams.eprint(&Message::new().lit("[recipe] 중단: ").text(&d.text).done());
    streams.finish(d.code)
}

fn s(text: &str) -> PyStr {
    PyStr::new(text)
}

fn git_out(root: &PyStr, args: &[&str]) -> Option<String> {
    let root = root.to_utf8()?;
    let out = Command::new("git").arg("-C").arg(root).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// `_repo_root(skill_root)`: the git toplevel of the skill, else three directories up.
fn repo_root(skill_root: &PyStr) -> PyStr {
    if let Some(top) = git_out(skill_root, &["rev-parse", "--show-toplevel"]) {
        if !top.is_empty() {
            return PyStr::new(&top);
        }
    }
    let mut root = skill_root.clone();
    for _ in 0..3 {
        root = dirname(&root);
    }
    root
}

fn topology_of(root: &PyStr) -> &'static str {
    if git_out(root, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default() == "multi-node" {
        "multi"
    } else {
        "single"
    }
}

fn str_of(value: &PyValue) -> Option<String> {
    match value.peel() {
        PyValue::Str(text) => text.to_utf8(),
        _ => None,
    }
}

fn starts_with(value: &PyValue, prefix: &str) -> bool {
    str_of(value).map_or(false, |text| text.starts_with(prefix))
}

fn is_true(value: &PyValue) -> bool {
    matches!(value.peel(), PyValue::Bool(true))
}

fn yaml_failure(error: pyyaml::YamlError) -> Failure {
    Failure::with_message(error.class, error.message)
}

fn empty_dict() -> PyValue {
    PyValue::Dict(PyDict::new())
}

/// `_read_manifest(repo_root)`: `(manifest, path)`; an absent file gives an empty mapping, an unparseable one status 5.
fn read_manifest(root: &PyStr) -> Step<(PyValue, PyStr)> {
    let path = join(
        &join(&join(root, &s("output")), &s(topology_of(root))),
        &s("manifest.yaml"),
    );
    if !is_file(&path) {
        return Ok((empty_dict(), path));
    }
    let loaded = read_bytes(&path).and_then(|bytes| pyyaml::safe_load(&bytes, &path).map_err(yaml_failure));
    match loaded {
        Ok(PyValue::None) => Ok((empty_dict(), path)),
        Ok(value) => Ok((value, path)),
        Err(failure) => Err(die_py(
            5,
            Message::new()
                .lit("manifest 파싱 실패(")
                .text(&path)
                .lit("): ")
                .lit(failure.class)
                .lit(": ")
                .text(&failure.message)
                .lit(" — 손상된 HW 사실은 부재가 아니다. 파일을 고치거나 terraforming_node 로 재생성하세요.")
                .done(),
        )),
    }
}

fn manifest_gpus(man: &PyValue, path: &PyStr) -> Step<i64> {
    let raw = get(man, "gpus_per_node").unwrap_or(PyValue::None);
    let gpus = int_of(&raw).ok().and_then(|n| i64::try_from(n).ok());
    match gpus {
        Some(n) if n >= 1 => Ok(n),
        _ => Err(die_py(
            5,
            Message::new()
                .lit("manifest(")
                .text(path)
                .lit(") 의 gpus_per_node 가 없거나 유효하지 않다(")
                .val(&raw)
                .lit(") — TP 는 HW 사실이지 기본값이 아니다. terraforming_node 스캔으로 채우세요(fail-loud · 침묵 폴백 ✗ · audit_26090515 B4).")
                .done(),
        )),
    }
}

fn node_count(man: &PyValue) -> i64 {
    match get(man, "nodes").map(|n| n.peel().clone()) {
        Ok(PyValue::List(nodes)) => nodes.len() as i64,
        _ => 0,
    }
}

/// `_require_terraform_flag(repo_root)`.
fn require_terraform_flag(root: &PyStr) -> Step<()> {
    let (man, path) = read_manifest(root)?;
    let empty = match man.peel() {
        PyValue::Dict(d) => d.is_empty(),
        _ => true,
    };
    if empty {
        return Err(die_py(
            4,
            Message::new()
                .lit("manifest 부재(")
                .text(&path)
                .lit(") — 테라포밍 미완(fail-closed). HW 사실 없이 deliverable 생성 ✗.")
                .done(),
        ));
    }
    let terra = get(&man, "terraforming").unwrap_or(PyValue::None);
    let terra = if terra.is_truthy() { terra } else { empty_dict() };
    let flag = |key: &str| get(&terra, key).unwrap_or(PyValue::None);
    if !is_true(&flag("complete")) || !is_true(&flag("branch_verified")) {
        return Err(die(
            4,
            "테라포밍 완수 Flag 미발급(terraforming.complete/branch_verified != true) — info-only.",
        ));
    }
    if get(&man, "self_role").map_or(false, |r| str_of(&r).as_deref() == Some("sub"))
        && str_of(&flag("issued_by")).as_deref() != Some("main")
    {
        return Err(die(
            4,
            "self_role=sub 이지만 terraforming.issued_by != 'main' — 메인 발급 서브 manifest가 아니다(fail-closed).",
        ));
    }
    let missing: Vec<&str> = ["topology", "gpus_per_node"]
        .into_iter()
        .filter(|key| !get(&man, key).map_or(false, |v| v.is_truthy()))
        .collect();
    if !missing.is_empty() {
        return Err(die(
            5,
            &format!("필수 HW필드 누락({}) — terraforming_node 스캔 완수 먼저(info-only).", missing.join(", ")),
        ));
    }
    manifest_gpus(&man, &path)?;
    let source = get(&man, "model_source").unwrap_or(PyValue::None);
    if !matches!(str_of(&source).as_deref(), Some("managed" | "ephemeral" | "custom")) {
        return Err(die_py(
            5,
            Message::new()
                .lit("model_source 미설정/오류(")
                .val(&source)
                .lit(") — terraforming_node 에서 획득모드 지정 먼저(info-only).")
                .done(),
        ));
    }
    Ok(())
}

/// `resolve_tp(cfg, repo_root)`.
fn resolve_tp(cfg: &PyValue, root: &PyStr) -> Step<i64> {
    let explicit = get(cfg, "tensor_parallel_size").unwrap_or(PyValue::None);
    if !is_none(&explicit) {
        return int_of(&explicit)
            .map_err(raised)
            .and_then(|n| i64::try_from(n).map_err(|_| die(1, "config.tensor_parallel_size 범위 초과")));
    }
    let (man, path) = read_manifest(root)?;
    let gpus = manifest_gpus(&man, &path)?;
    let topo = get(&man, "topology").unwrap_or(PyValue::None);
    let mtopo = if topo.is_truthy() { str_of(&topo).unwrap_or_default() } else { topology_of(root).to_string() };
    if mtopo.starts_with("single") {
        return Ok(gpus);
    }
    let nodes = node_count(&man);
    Ok(if nodes > 0 { nodes * gpus } else { 1 })
}

/// `_guard_tp(tp, repo_root)`.
fn guard_tp(tp: i64, root: &PyStr) -> Step<()> {
    let (man, path) = read_manifest(root)?;
    let gpus = manifest_gpus(&man, &path)?;
    let nodes = node_count(&man);
    let total = if starts_with(&get(&man, "topology").unwrap_or(PyValue::None), "single") || nodes == 0 {
        gpus
    } else {
        nodes * gpus
    };
    if tp > total {
        return Err(die(
            6,
            &format!(
                "tp={} 가 가용 GPU 합 {} 초과(GPU 초과) — config.tensor_parallel_size 또는 manifest(gpus_per_node·nodes) 확인.",
                tp, total
            ),
        ));
    }
    Ok(())
}

/// `_guard_kv_heads(parsed, tp)`.
fn guard_kv_heads(parsed: &PyValue, tp: i64) -> Step<()> {
    let kvh = get(parsed, "num_key_value_heads").unwrap_or(PyValue::None);
    let Ok(kvh_i) = int_of(&kvh) else { return Ok(()) };
    let tp_i = BigInt::from(tp);
    let zero = BigInt::from(0);
    if kvh_i == zero || tp_i == zero {
        return Ok(());
    }
    let divisible = if kvh_i >= tp_i { &kvh_i % &tp_i == zero } else { &tp_i % &kvh_i == zero };
    if !divisible {
        return Err(die_py(
            6,
            Message::new()
                .lit("num_key_value_heads=")
                .val(&kvh)
                .lit(&format!(" 가 tp={} 와 상호 분배 불가(양방향 나눗셈 모두 비정수) — tp 조정 필요.", tp))
                .done(),
        ));
    }
    Ok(())
}

/// `load_config(config_path)`.
fn load_config(path: &PyStr) -> Step<PyValue> {
    if !is_file(path) {
        return Err(die_py(1, Message::new().lit("config 파일 없음: ").text(path).done()));
    }
    let loaded = read_bytes(path).and_then(|bytes| pyyaml::safe_load(&bytes, path).map_err(yaml_failure));
    match loaded {
        Ok(PyValue::None) => Ok(empty_dict()),
        Ok(value) => match value.peel() {
            PyValue::Dict(_) => Ok(value),
            _ => Err(die_py(1, Message::new().lit("config 형식 오류(매핑 아님): ").text(path).done())),
        },
        Err(failure) => Err(raised(failure)),
    }
}

fn estimate_spec() -> Spec {
    let opt = |flags: &'static [&'static str], dest: &'static str, kind: Kind, help: Option<&'static str>, required: bool, group: Option<usize>| Opt {
        flags,
        dest,
        kind,
        help,
        required,
        group,
    };
    Spec {
        prog: "recipe.py estimate",
        description: "",
        options: vec![
            opt(&["-h", "--help"], "help", Kind::Help, None, false, None),
            opt(
                &["--config"],
                "config",
                Kind::Text,
                Some("입력 config.yaml 경로. **필수다 — 루트 기본값은 2026-09-06 에 제거했다**."),
                true,
                None,
            ),
            opt(&["--auto"], "auto", Kind::Flag, Some("결정론 기본 그리드 후보 사용"), false, Some(0)),
            opt(
                &["--candidates"],
                "candidates",
                Kind::Text,
                Some("LLM 생성 후보 JSON(list) 경로"),
                false,
                Some(0),
            ),
        ],
        group_required: true,
        positional: None,
    }
}

fn float_value(value: &PyValue) -> Step<PyValue> {
    Ok(PyValue::Float(float_of(value).map_err(raised)?))
}

/// `os.makedirs(dir, exist_ok=True)` then write `text` to `path` (UTF-8).
fn write_text(dir: &PyStr, path: &PyStr, text: &PyStr) -> Res<()> {
    use crate::model_files::{os_failure, path_bytes};
    use std::os::unix::ffi::OsStrExt;
    let dir_path = std::path::PathBuf::from(std::ffi::OsStr::from_bytes(&path_bytes(dir)?));
    if !is_dir(dir) {
        std::fs::create_dir_all(&dir_path).map_err(|e| os_failure(&e, Some(dir)))?;
    }
    let bytes = text
        .to_surrogateescape_bytes()
        .map_err(|e| Failure::new("UnicodeEncodeError", &e.message()))?;
    let file = std::path::PathBuf::from(std::ffi::OsStr::from_bytes(&path_bytes(path)?));
    std::fs::write(file, bytes).map_err(|e| os_failure(&e, Some(path)))
}

/// `cmd_estimate(args)`, after the Flag gate.
fn cmd_estimate(
    streams: &mut Streams,
    config_path: &PyStr,
    candidates_path: Option<&PyStr>,
    auto: bool,
    root: &PyStr,
    feedback_dir: &PyStr,
) -> Step<()> {
    let cfg = load_config(config_path)?;
    if get(&cfg, "target_gpu").map_or(false, |v| v.is_truthy()) {
        return Err(die(
            7,
            "config.target_gpu 정의됨 — Phase-1(estimate)은 gmu-only 라 절대 KV 클램프를 emit 하지 않는다(헌법 §KV 절대클램프 따름정리 · plan_26070809_47_07 §2). `recipe.py simulate --config <config> --candidate <lockset.json>` 로 Phase-2 측정경로를 사용하세요(타겟 예산은 config.target_gpu 가 그대로 소비됨).",
        ));
    }
    let target = get(&cfg, "target_model").unwrap_or(PyValue::None);
    let target = if target.is_truthy() { target } else { empty_dict() };
    let model_path = get(&target, "path").unwrap_or(PyValue::None);
    if !model_path.is_truthy() {
        return Err(die(1, "config.target_model.path 누락"));
    }
    let (man, _mpath) = read_manifest(root)?;
    let nas_root = {
        let from_cfg = get(&cfg, "nas_host_root").unwrap_or(PyValue::None);
        if from_cfg.is_truthy() {
            from_cfg
        } else {
            match std::env::var("NAS_MODEL_PATH") {
                Ok(v) if !v.is_empty() => PyValue::str(&v),
                _ => {
                    let from_man = get(&man, "nas_model_path").unwrap_or(PyValue::None);
                    if from_man.is_truthy() { from_man } else { PyValue::str(DEFAULT_NAS_HOST_ROOT) }
                }
            }
        }
    };
    let budget_raw = get(&cfg, "vram_budget_gb").unwrap_or(PyValue::None);
    if is_none(&budget_raw) {
        return Err(die(1, "config.vram_budget_gb 누락"));
    }
    let budget = float_value(&budget_raw)?;
    let margin = float_value(&get_or(&cfg, "safety_margin", PyValue::Float(DEFAULT_SAFETY_MARGIN)).map_err(raised)?)?;
    let kv_bytes = PyValue::int(
        int_of(&get_or(&cfg, "kv_cache_dtype_bytes", PyValue::int(DEFAULT_KV_BYTES)).map_err(raised)?).map_err(raised)?,
    );
    let container_root = get_or(&cfg, "nas_container_root", PyValue::str("/app/models")).map_err(raised)?;

    let tp = resolve_tp(&cfg, root)?;
    guard_tp(tp, root)?;

    let parsed = match model_config::parse(&model_path, &nas_root, &container_root) {
        Ok(parsed) => PyValue::Dict(parsed),
        Err(failure) if failure.class == "FileNotFoundError" => return Err(die_py(1, failure.message)),
        Err(failure) if failure.is_value_error() => {
            return Err(die_py(1, Message::new().lit("모델 config 해소 실패: ").text(&failure.message).done()))
        }
        Err(failure) => return Err(raised(failure)),
    };
    guard_kv_heads(&parsed, tp)?;

    let tp_value = PyValue::int(BigInt::from(tp));
    let candidates = if let Some(path) = candidates_path {
        if !is_file(path) {
            return Err(die_py(1, Message::new().lit("--candidates 파일 없음: ").text(path).done()));
        }
        let PyValue::List(mut list) = load_json_file(path).map_err(raised)? else {
            return Err(die(1, "--candidates JSON 은 후보 리스트여야 함"));
        };
        for (index, cand) in list.iter_mut().enumerate() {
            if let PyValue::Dict(dict) = cand {
                if !dict.contains_key("id") {
                    dict.set("id", PyValue::str(&format!("r{}", index + 1)));
                }
            }
        }
        PyValue::List(list)
    } else if auto {
        auto_candidates(&parsed, &tp_value).map_err(raised)?
    } else {
        return Err(die(1, "후보 소스 미지정: --auto 또는 --candidates <file> 중 하나 필요"));
    };

    let result = rank(&parsed, &candidates, &tp_value, &budget, &margin, &kv_bytes).map_err(raised)?;
    let report = render_report(&result, &parsed, &budget, &margin, &tp_value).map_err(raised)?;
    streams.print(&report);

    let mut snapshot = PyDict::new();
    snapshot.set("model_id", get(&parsed, "model_id").unwrap_or(PyValue::None));
    snapshot.set("model_path_container", get(&parsed, "model_path_container").unwrap_or(PyValue::None));
    snapshot.set("tp", tp_value.clone());
    snapshot.set("vram_budget_gb", budget.clone());
    snapshot.set("safety_margin", margin.clone());
    snapshot.set("kv_cache_dtype_bytes", kv_bytes.clone());
    snapshot.set("result", result);
    let text = PyValue::Dict(snapshot)
        .dumps_pretty()
        .map_err(|e| die(-1, &format!("{}: json dump 실패", e.python_type())))?;
    let last_ranking = join(feedback_dir, &s(".last_ranking.json"));
    write_text(feedback_dir, &last_ranking, &text).map_err(raised)?;
    let mut note = PyStr::new("\n[recipe] 랭킹 저장: ");
    note.push_py(&last_ranking);
    streams.eprint(&note);
    let mut hint = PyStr::new("[recipe] generate 로 선택: python3 recipe.py generate --config ");
    hint.push_py(config_path);
    hint.push_str(" --recipe-id <rN>");
    streams.eprint(&hint);
    streams.eprint(&PyStr::new("[recipe] ⚠ Phase-1(estimate→generate)은 max-num-seqs·절대 KV 클램프(kv-cache-memory-bytes)를 emit하지 않는다(공식 per-token이 sliding-window/GQA에서 부정확 — 결함#4). max-num-seqs 는 simulate(Phase-2)가 min(declared_axes.concurrency_requirement, KV_fit@typical_request_tokens) 로 엔진 보고 kv_cache_tokens 에서 산정한다(references/kv-clamp.md §3 · plan_26091407 §4.2)."));
    Ok(())
}

const COMPOSE_NAME_RULE: &str = "[a-z0-9][a-z0-9_-]*";

/// `_COMPOSE_NAME_RE.match(name or "")`: `^[a-z0-9][a-z0-9_-]*$` (a trailing newline does not match `$` here: Python's `$` would
/// accept one; the names that reach here come from YAML scalars, so the exact Python `$` quirk is kept).
fn compose_name_ok(name: &str) -> bool {
    let name = name.strip_suffix('\n').unwrap_or(name);
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() || c.is_ascii_digit() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

fn generate_spec() -> Spec {
    let opt = |flags: &'static [&'static str], dest: &'static str, kind: Kind, help: Option<&'static str>, required: bool| Opt {
        flags,
        dest,
        kind,
        help,
        required,
        group: None,
    };
    const CONFIG_HELP: &str = "입력 config.yaml 경로. **필수다 — 루트 기본값은 2026-09-06 에 제거했다**(plan_26090616): 기본값이 `config.yaml` 이었기 때문에 캠페인이 셀마다 만든 변형이 저장소 루트에 쌓였고 21개가 공개 원격까지 추적 누출됐다. 자리는 `campaign_init.py --derive config --cell <cell-id>` 로 파생하라.";
    Spec {
        prog: "recipe.py generate",
        description: "",
        options: vec![
            opt(&["-h", "--help"], "help", Kind::Help, None, false),
            opt(&["--config"], "config", Kind::Text, Some(CONFIG_HELP), true),
            opt(&["--recipe-id"], "recipe_id", Kind::Text, Some("선택 레시피 id(예: r3)"), true),
            opt(&["--force"], "force", Kind::Flag, Some("기존 파일 덮어쓰기 허용"), false),
            opt(
                &["--image"],
                "image",
                Kind::Text,
                Some("컨테이너 이미지 태그 → env 의 IMAGE_TAG. 생략하면 env 에 미지정 표시가 박히고 경고가 나간다(compose 는 비면 낡은 기본값으로 조용히 폴백한다 — D8)."),
                false,
            ),
        ],
        group_required: false,
        positional: None,
    }
}

// ── simulate: GPU roles (`_memory_model` · `_lookup_gpu_spec` · `resolve_target_gpu_budget` · `_target_tp` ·
//    `_resolve_device_total_gib` · `_host_target_gpu` · `_gpu_roles`) ───────────────────────────────────────────────────

/// Tolerance of the host-flow `vram_budget_gb` ↔ node per-card VRAM agreement (`_HOST_BUDGET_MATCH_TOLERANCE`).
const HOST_BUDGET_MATCH_TOLERANCE: f64 = 0.05;

fn num(value: &PyValue) -> Step<f64> {
    float_of(value).map_err(raised)
}

fn truthy_get(value: &PyValue, key: &str) -> PyValue {
    get(value, key).unwrap_or(PyValue::None)
}

/// `str(man.get(key) or "").strip().strip('"')`.
fn stripped_field(man: &PyValue, key: &str) -> String {
    let v = truthy_get(man, key);
    if !v.is_truthy() {
        return String::new();
    }
    str_of(&v).unwrap_or_default().trim().trim_matches('"').to_string()
}

/// `SystemExit(message)`: the message on stderr, status 1 (a `-1` Die prints its text bare).
fn system_exit(text: &str) -> Die {
    Die { code: -1, text: PyStr::new(text) }
}

/// `_lookup_gpu_spec(gpu_model)`: per-card VRAM (GiB) and the unified-memory mark from the `### … <gpu_model> …` header of
/// references.md §4 (`None, None` when the file or the section is absent).
fn lookup_gpu_spec(gpu_model: &str, root: &PyStr) -> (Option<f64>, Option<bool>) {
    let key = gpu_model.trim().to_lowercase();
    let path = join(root, &s(".claude/skills/wiki-desk/reference/references.md"));
    if gpu_model.is_empty() || !is_file(&path) {
        return (None, None);
    }
    let Ok(bytes) = read_bytes(&path) else { return (None, None) };
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let lines: Vec<&str> = text.split('\n').collect();
    let is_header = |l: &str| {
        let rest = l.strip_prefix("###");
        rest.map_or(false, |r| r.chars().next().map_or(false, char::is_whitespace))
    };
    let Some(at) = lines.iter().position(|l| is_header(l) && l.to_lowercase().contains(&key)) else {
        return (None, None);
    };
    let is_unified = lines[at].contains("통합메모리");
    let is_heading = |l: &str| {
        let hashes = l.chars().take_while(|c| *c == '#').count();
        (1..=3).contains(&hashes) && l.chars().nth(hashes).map_or(false, char::is_whitespace)
    };
    let end = lines[at + 1..].iter().position(|l| is_heading(l)).map_or(lines.len(), |p| at + 1 + p);
    let section = lines[at + 1..end].join("\n");
    let mut per_card = None;
    let mut from = 0;
    while let Some(p) = section[from..].find("per-card VRAM") {
        let mut rest = section[from + p + "per-card VRAM".len()..].trim_start();
        if let Some(r) = rest.strip_prefix("(GiB)") {
            rest = r.trim_start();
            if let Some(r) = rest.strip_prefix(':') {
                let r = r.trim_start();
                let digits: String = r.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
                if !digits.is_empty() {
                    per_card = digits.parse::<f64>().ok();
                    break;
                }
            }
        }
        from += p + "per-card VRAM".len();
    }
    (per_card, Some(is_unified))
}

/// `_memory_model(man)`: `unified`/`discrete`/unknown from observed facts only, and the source sentence.
fn memory_model(man: &PyValue, root: &PyStr) -> (Option<&'static str>, String) {
    let mm = stripped_field(man, "gpu_memory_model");
    if mm == "unified" {
        return (Some("unified"), "manifest.gpu_memory_model".to_string());
    }
    if mm == "discrete" {
        return (Some("discrete"), "manifest.gpu_memory_model".to_string());
    }
    let gpu_model = stripped_field(man, "gpu_model");
    let (_, unified) = lookup_gpu_spec(&gpu_model, root);
    match unified {
        Some(u) => (
            Some(if u { "unified" } else { "discrete" }),
            format!("references.md §4 헤더(gpu_model={})", gpu_model),
        ),
        None => (None, "미상(manifest gpu_memory_model 부재 · references.md 미등재)".to_string()),
    }
}

/// `resolve_target_gpu_budget`: `(per_card_vram_gib, deploy_gmu, gpu_model)`; the unified-memory hard clamp warns on stderr.
fn resolve_target_gpu_budget(
    streams: &mut Streams,
    gpu_model: &PyValue,
    target_gmu: &PyValue,
    per_card: &PyValue,
    root: &PyStr,
) -> Step<(f64, f64, String)> {
    if !gpu_model.is_truthy() {
        return Err(die(1, "config.target_gpu.gpu_model 누락(타겟 GPU 이식 경로엔 필수 — plan_26070809_47_07 §4.1)"));
    }
    if is_none(target_gmu) {
        return Err(die(
            5,
            "config.target_gpu.target_gmu 누락 — 배포 gmu(safe_gmu)의 정본이며 기본값을 두지 않는다(2026-09-14 · plan_26091407 §4.3). 타겟에서 돌 gpu-memory-utilization 을 선언하라(통합메모리 타겟은 ≤ 0.90). 예산 검증 게이트 승수는 별개 칸 config.safety_margin 이다.",
        ));
    }
    let mut gmu = num(target_gmu)?;
    let name = str_of(gpu_model).unwrap_or_default();
    let (looked_up, unified) = lookup_gpu_spec(&name, root);
    let per_card_value = if is_none(per_card) {
        match looked_up {
            Some(v) => PyValue::Float(v),
            None => PyValue::None,
        }
    } else {
        per_card.clone()
    };
    if is_none(&per_card_value) {
        return Err(die_py(
            1,
            Message::new()
                .lit("target_gpu.gpu_model=")
                .val(gpu_model)
                .lit(" 의 per-card VRAM 을 references.md §4 에서 찾지 못함 — config.target_gpu.per_card_vram_gib 를 명시하세요(quick-win 우회, plan_26070809_47_07 §6).")
                .done(),
        ));
    }
    let per_card_gib = num(&per_card_value)?;
    if unified == Some(true) && gmu > 0.90 {
        streams.eprint(&PyStr::new(&format!(
            "[recipe] 경고: 통합메모리 타겟(gpu_model={})은 target_gmu ≤ 0.90 하드클램프 — {:.2} → 0.90 하향(HITL 재확인 요망).",
            name, gmu
        )));
        gmu = 0.90;
    }
    Ok((per_card_gib, gmu, name))
}

/// `_target_tp`: `target_gpu.cards_per_node × node_count` (node_count 1 on a single topology).
fn target_tp(cfg: &PyValue, root: &PyStr) -> Step<i64> {
    let tgt = truthy_get(cfg, "target_gpu");
    let tgt = if tgt.is_truthy() { tgt } else { empty_dict() };
    let cards = get_or(&tgt, "cards_per_node", PyValue::int(1)).map_err(raised)?;
    let cards = int_of(&cards).map_err(raised).and_then(|n| i64::try_from(n).map_err(|_| die(1, "cards_per_node 범위 초과")))?;
    let (man, _) = read_manifest(root)?;
    if starts_with(&truthy_get(&man, "topology"), "single") {
        return Ok(cards);
    }
    Ok(cards * node_count(&man).max(1))
}

/// `_resolve_device_total_gib(cfg)`: device memory total (GiB) and its source; never a hardware constant.
fn resolve_device_total_gib(cfg: &PyValue, root: &PyStr) -> Step<(f64, String)> {
    let declared = truthy_get(cfg, "test_device_total_gib");
    if declared.is_truthy() {
        return Ok((num(&declared)?, "declared(config test_device_total_gib)".to_string()));
    }
    let (man, _) = read_manifest(root)?;
    let (mm, mm_src) = memory_model(&man, root);
    if mm == Some("discrete") {
        let vram = truthy_get(&man, "gpu_vram_mib");
        if vram.is_truthy() {
            return Ok((num(&vram)? / 1024.0, format!("measured(manifest gpu_vram_mib · discrete ← {})", mm_src)));
        }
        if let (Some(v), _) = lookup_gpu_spec(&stripped_field(&man, "gpu_model"), root) {
            return Ok((v, format!("references.md §4 per-card VRAM(discrete ← {})", mm_src)));
        }
        return Err(system_exit(&format!(
            "[recipe] FAIL: discrete GPU({})인데 manifest 에 gpu_vram_mib 가 없다 — 호스트 RAM 을 VRAM 으로 쓰지 않는다.\n  → terraforming_node 재스캔으로 채우거나 config 에 `test_device_total_gib` 를 선언하라.",
            mm_src
        )));
    }
    if mm != Some("unified") {
        return Err(system_exit(&format!(
            "[recipe] FAIL: 디바이스 메모리 total 을 알 수 없다 — 메모리 모델 {}.\n  → config 에 `test_device_total_gib: <실측 GiB>` 를 선언하거나 terraforming_node 재스캔으로 manifest gpu_memory_model 을 채워라(통합메모리일 때만 /proc/meminfo 를 쓴다).",
            mm_src
        )));
    }
    if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
        for line in meminfo.lines() {
            if line.starts_with("MemTotal:") {
                if let Some(kib) = line.split_whitespace().nth(1).and_then(|v| v.parse::<i64>().ok()) {
                    return Ok((kib as f64 / (1024.0 * 1024.0), format!("measured(/proc/meminfo MemTotal · 통합메모리 ← {})", mm_src)));
                }
                break;
            }
        }
    }
    Err(system_exit(
        "[recipe] FAIL: 디바이스 메모리 total 을 알 수 없다 — 상수를 쓰지 않는다(2026-09-05 · G-B5).\n  → config 에 `test_device_total_gib: <실측 GiB>` 를 선언하거나, 통합메모리 노드에서 /proc/meminfo 를 읽을 수 있게 하라.",
    ))
}

/// The host-flow `target_gpu` block `_host_target_gpu` fills from this node's facts, with the source of each cell.
struct HostBlock {
    gpu_model: String,
    per_card_vram_gib: f64,
    target_gmu: f64,
    per_card_src: String,
    target_gmu_src: String,
}

fn host_target_gpu(cfg: &PyValue, root: &PyStr) -> Step<HostBlock> {
    let (man, mpath) = read_manifest(root)?;
    let gpu_model = stripped_field(&man, "gpu_model");
    if gpu_model.is_empty() {
        return Err(die_py(
            5,
            Message::new()
                .lit("host 흐름(target_gpu 미정의)인데 manifest(")
                .text(&mpath)
                .lit(") 에 gpu_model 이 없다 — 이 노드 GPU 의 per-card VRAM·통합메모리 여부를 정할 근거가 없다(추측 ✗). terraforming_node 스캔으로 채우거나 config.target_gpu 블록을 선언하라(plan_26091407 §4.3).")
                .done(),
        ));
    }
    let (looked_up, _) = lookup_gpu_spec(&gpu_model, root);
    let (mm, mm_src) = memory_model(&man, root);
    let declared_total = truthy_get(cfg, "test_device_total_gib");
    let (per_card, per_card_src) = if mm == Some("unified") {
        let (v, src) = resolve_device_total_gib(cfg, root)?;
        (v, format!("통합메모리({}) → {}", mm_src, src))
    } else if declared_total.is_truthy() {
        (num(&declared_total)?, "declared(config test_device_total_gib)".to_string())
    } else if mm == Some("discrete") && truthy_get(&man, "gpu_vram_mib").is_truthy() {
        (num(&truthy_get(&man, "gpu_vram_mib"))? / 1024.0, "measured(manifest gpu_vram_mib)".to_string())
    } else if let Some(v) = looked_up {
        (v, format!("references.md §4 역룩업(gpu_model={})", gpu_model))
    } else {
        return Err(die_py(
            5,
            Message::new()
                .lit("host 흐름: gpu_model=")
                .val(&text_value(&gpu_model))
                .lit(" 의 per-card VRAM 을 정할 수 없다 — references.md §4 미등재 · 통합메모리 표지 없음 · config.test_device_total_gib 미선언. 셋 중 하나를 채워라(호스트 RAM 을 VRAM 으로 가정하지 않는다).")
                .done(),
        ));
    };
    let margin = truthy_get(cfg, "safety_margin");
    if is_none(&margin) {
        return Err(die(
            5,
            "host 흐름: 배포 gmu 를 정할 선언이 없다 — config.target_gpu.target_gmu(타겟 흐름)도 config.safety_margin 명시값(host 흐름 승계)도 없다. 기본값 0.90 을 배포 gmu 로 쓰지 않는다(2026-09-14 · plan_26091407 §4.3).",
        ));
    }
    manifest_gpus(&man, &mpath)?;
    Ok(HostBlock {
        gpu_model,
        per_card_vram_gib: per_card,
        target_gmu: num(&margin)?,
        per_card_src,
        target_gmu_src: "config.safety_margin 승계(host 흐름 — target_gpu.target_gmu 칸이 없다 · gate_margin 과 같은 값을 배포에 쓴다)".to_string(),
    })
}

fn text_value(text: &str) -> PyValue {
    crate::pyexpr::text(text)
}

/// `_gpu_roles` result (the fields `cmd_simulate` reads).
struct Roles {
    flow: &'static str,
    tp_divisor: i64,
    gate_margin: f64,
    gate_margin_source: String,
    budget_gib: f64,
    budget_source: String,
    deploy_gmu: f64,
    deploy_gmu_source: String,
    gmu_source: &'static str,
    gpu_model: String,
}

fn gpu_roles(streams: &mut Streams, cfg: &PyValue, tp: i64, root: &PyStr) -> Step<Roles> {
    let margin = truthy_get(cfg, "safety_margin");
    let declared = !is_none(&margin);
    let gate_margin = if declared { num(&margin)? } else { DEFAULT_SAFETY_MARGIN };
    let gate_margin_source = if declared {
        "config.safety_margin".to_string()
    } else {
        "DEFAULT_SAFETY_MARGIN(config.safety_margin 미선언 — Phase-1 과 같은 기본값)".to_string()
    };
    let target = truthy_get(cfg, "target_gpu");
    let (flow, tp_divisor, budget, deploy_gmu, gpu_model, declared_gmu, mut budget_source, mut deploy_src, gmu_source);
    if target.is_truthy() {
        let ttp = target_tp(cfg, root)?;
        if ttp != tp {
            return Err(die(
                6,
                &format!(
                    "측정 TP({}) ≠ 타겟 TP({}, cards_per_node×node_count) — 1→N 외삽 금지(plan_26070809_47_07 §4.3). config.tensor_parallel_size 를 타겟에 맞추거나 manifest nodes[]/target_gpu.cards_per_node 를 정합시키세요.",
                    tp, ttp
                ),
            ));
        }
        let gmu_decl = truthy_get(&target, "target_gmu");
        let per_card = truthy_get(&target, "per_card_vram_gib");
        let (b, g, m) = resolve_target_gpu_budget(streams, &truthy_get(&target, "gpu_model"), &gmu_decl, &per_card, root)?;
        flow = "target";
        tp_divisor = tp;
        budget = b;
        deploy_gmu = g;
        declared_gmu = num(&gmu_decl)?;
        budget_source = if !is_none(&per_card) {
            "config target_gpu.per_card_vram_gib".to_string()
        } else {
            format!("references.md §4 역룩업(gpu_model={})", m)
        };
        deploy_src = "config target_gpu.target_gmu".to_string();
        gmu_source = "target_gmu";
        gpu_model = m;
    } else {
        let block = host_target_gpu(cfg, root)?;
        let (b, g, m) = resolve_target_gpu_budget(
            streams,
            &text_value(&block.gpu_model),
            &PyValue::Float(block.target_gmu),
            &PyValue::Float(block.per_card_vram_gib),
            root,
        )?;
        flow = "host";
        tp_divisor = 1;
        budget = b;
        deploy_gmu = g;
        declared_gmu = block.target_gmu;
        budget_source = format!("host 흐름 · {}", block.per_card_src);
        deploy_src = block.target_gmu_src.clone();
        gmu_source = "hand";
        gpu_model = m;
        let vb = truthy_get(cfg, "vram_budget_gb");
        if !is_none(&vb) && budget != 0.0 {
            let gap = (num(&vb)? - budget).abs() / budget;
            if gap > HOST_BUDGET_MATCH_TOLERANCE {
                return Err(die_py(
                    5,
                    Message::new()
                        .lit("host 흐름 simulate: config.vram_budget_gb=")
                        .val(&vb)
                        .lit(&format!(" 가 이 노드 per_card_vram={:.2} GiB(", budget))
                        .lit(&block.per_card_src)
                        .lit(&format!(
                            ")와 {:.1}% 다르다(허용 {:.0}%) — 선언 예산과 노드 사실 중 어느 쪽을 클램프 천장·검증 게이트에 쓸지 추측하지 않는다. carve-out(가상 예산)이면 target_gpu 블록(gpu_model·per_card_vram_gib·target_gmu)으로 선언하고, 이 노드 전체가 예산이면 vram_budget_gb 를 per_card_vram 에 맞춰라(plan_26091407 §4.3).",
                            100.0 * gap,
                            100.0 * HOST_BUDGET_MATCH_TOLERANCE
                        ))
                        .done(),
                ));
            }
            budget_source.push_str(" · config.vram_budget_gb=");
            budget_source.push_str(&Message::new().val(&vb).done().to_utf8().unwrap_or_default());
            budget_source.push_str(&format!(" 와 정합(차 {:.1}%)", 100.0 * gap));
        }
    }
    if (declared_gmu - deploy_gmu).abs() > 1e-9 {
        deploy_src.push_str(&format!(" · 통합메모리 하드클램프 {:.2}→{:.2}", declared_gmu, deploy_gmu));
    }
    Ok(Roles {
        flow,
        tp_divisor,
        gate_margin,
        gate_margin_source,
        budget_gib: budget,
        budget_source,
        deploy_gmu,
        deploy_gmu_source: deploy_src,
        gmu_source,
        gpu_model,
    })
}

fn simulate_spec() -> Spec {
    let opt = |flags: &'static [&'static str], dest: &'static str, kind: Kind, help: Option<&'static str>, required: bool| Opt {
        flags,
        dest,
        kind,
        help,
        required,
        group: None,
    };
    const CONFIG_HELP: &str = "입력 config.yaml 경로. **필수다 — 루트 기본값은 2026-09-06 에 제거했다**(plan_26090616): 기본값이 `config.yaml` 이었기 때문에 캠페인이 셀마다 만든 변형이 저장소 루트에 쌓였고 21개가 공개 원격까지 추적 누출됐다. 자리는 `campaign_init.py --derive config --cell <cell-id>` 로 파생하라.";
    Spec {
        prog: "recipe.py simulate",
        description: "",
        options: vec![
            opt(&["-h", "--help"], "help", Kind::Help, None, false),
            opt(&["--config"], "config", Kind::Text, Some(CONFIG_HELP), true),
            opt(&["--candidate"], "candidate", Kind::Text, Some("lock-set 후보 JSON 경로"), true),
            opt(&["--dry-run"], "dry_run", Kind::Flag, Some("docker 없이 mock_profile 로 배선 검증"), false),
            opt(&["--mock-profile"], "mock_profile", Kind::Text, Some("dry-run 시 사용할 vllm_profile+functional JSON 경로"), false),
            opt(&["--run-id"], "run_id", Kind::Text, Some("simlog run_id(미지정 시 <YYYYMMDDHH>_1_<topic> 자동)"), false),
            opt(&["--topic"], "topic", Kind::Text, Some("run_id 자동생성 시 주제(미지정 시 model_id)"), false),
            opt(&["--cap"], "cap", Kind::Int, Some("reconciliation_cap(기본 3)"), false),
            opt(&["--image"], "image", Kind::Text, Some("run_trial docker 이미지"), false),
            opt(&["--timeout"], "timeout", Kind::Int, Some("/health 폴링 타임아웃(초)"), false),
            opt(&["--overhead-mib"], "overhead_mib", Kind::Int, Some("예산 선언 overhead(MiB). 미지정 시 config.budget_overhead_mib → run_trial 기본값 순. 실측값이 있으면 넘긴다"), false),
            opt(&["--force"], "force", Kind::Flag, Some("수렴 시 3종 세트 덮어쓰기 허용"), false),
            opt(&["--lockset-out"], "lockset_out", Kind::Text, Some("수렴 시 기계 각인한 lockset(provenance=explorer-phase2 · batch_source·gmu_source·kv_source · trial_provenance)을 쓸 경로. 캠페인 셀이면 `campaign_init.py --derive lockset --cell <id>` 로 파생한 셀 lockset(보통 --candidate 와 같은 파일)을 준다. 다른 기존 파일은 --force 로만 덮어쓴다. 미지정이면: --candidate 가 캠페인 셀 lockset(campaigns/<id>/cells/<cell>/lockset.json)이면 그 파일에 쓰고, 그 밖이면 run_summary.json 의 lockset 칸에만 남는다(plan_26091407 §4.2)"), false),
        ],
        group_required: false,
        positional: None,
    }
}

/// `_find_recipe(snapshot, recipe_id)`: the candidate and its bucket (ranked → failed → all).
fn find_recipe(snapshot: &PyValue, recipe_id: &PyStr) -> Res<Option<(PyValue, String)>> {
    let result = get(snapshot, "result")?;
    let result = if result.is_truthy() { result } else { empty_dict() };
    for bucket in ["ranked", "failed", "all"] {
        let list = get(&result, bucket)?;
        if !list.is_truthy() {
            continue;
        }
        for cand in crate::pyexpr::items_of(&list)? {
            let id = get(&cand, "id")?;
            if let PyValue::Str(text) = id.peel() {
                if text == recipe_id {
                    return Ok(Some((cand, bucket.to_string())));
                }
            }
        }
    }
    Ok(None)
}

/// `cmd_generate(args)`, after the Flag gate.
fn cmd_generate(
    streams: &mut Streams,
    config_path: &PyStr,
    recipe_id: &PyStr,
    force: bool,
    image: Option<&PyStr>,
    root: &PyStr,
    feedback_dir: &PyStr,
) -> Step<()> {
    let cfg = load_config(config_path)?;
    if get(&cfg, "target_gpu").map_or(false, |v| v.is_truthy()) {
        return Err(die(
            7,
            "config.target_gpu 정의됨 — Phase-1(generate)은 gmu-only 라 절대 KV 클램프를 emit 하지 않는다(헌법 §KV 절대클램프 따름정리 · plan_26070809_47_07 §2). `recipe.py simulate --config <config> --candidate <lockset.json>` 로 Phase-2 측정경로를 사용하세요(타겟 예산은 config.target_gpu 가 그대로 소비됨).",
        ));
    }
    let last_ranking = join(feedback_dir, &s(".last_ranking.json"));
    if !is_file(&last_ranking) {
        return Err(die_py(
            1,
            Message::new()
                .lit("이전 estimate 산출물 없음: ")
                .text(&last_ranking)
                .lit(". 먼저 `recipe.py estimate` 를 실행하라.")
                .done(),
        ));
    }
    let snapshot = load_json_file(&last_ranking).map_err(raised)?;
    let Some((recipe, bucket)) = find_recipe(&snapshot, recipe_id).map_err(raised)? else {
        return Err(die_py(
            1,
            Message::new().lit("recipe-id 미발견: ").text(recipe_id).lit(" (in ").text(&last_ranking).lit(")").done(),
        ));
    };
    if bucket != "ranked" {
        let mut warn = PyStr::new("[recipe] 경고: ");
        warn.push_py(recipe_id);
        warn.push_str(&format!(" 는 '{}' 버킷(게이트 통과 아님).", bucket));
        streams.eprint(&warn);
    }

    let serving = get(&cfg, "serving").map_err(raised)?;
    let serving = if serving.is_truthy() { serving } else { empty_dict() };
    let name = get(&serving, "config_name").map_err(raised)?;
    let port = get(&serving, "port").map_err(raised)?;
    let served = get(&serving, "served_model_name").map_err(raised)?;
    if !name.is_truthy() {
        return Err(die(1, "config.serving.config_name 누락"));
    }
    let name_ok = str_of(&name).map_or(false, |n| compose_name_ok(&n));
    if !name_ok {
        return Err(die_py(
            1,
            Message::new()
                .lit("config.serving.config_name '")
                .text(&name.display_text())
                .lit(&format!("' 은 docker compose 프로젝트명 규칙 위반 ({} · 점/대문자/공백 불가). 모델명의 점을 빼라(예: glm-4.7 → glm-47). 지금 막지 않으면 3종 세트는 생성되고 compose up 에서만 터진다.", COMPOSE_NAME_RULE))
                .done(),
        ));
    }
    if is_none(&port) {
        return Err(die(1, "config.serving.port 누락"));
    }
    if !served.is_truthy() {
        return Err(die(1, "config.serving.served_model_name 누락"));
    }
    let port = PyValue::int(int_of(&port).map_err(raised)?);

    // `_cfg_common(cfg, REPO_ROOT)`
    let target = get(&cfg, "target_model").unwrap_or(PyValue::None);
    let target = if target.is_truthy() { target } else { empty_dict() };
    let model_path = get(&target, "path").unwrap_or(PyValue::None);
    if !model_path.is_truthy() {
        return Err(die(1, "config.target_model.path 누락"));
    }
    let (man, _mpath) = read_manifest(root)?;
    let nas_root = {
        let from_cfg = get(&cfg, "nas_host_root").unwrap_or(PyValue::None);
        if from_cfg.is_truthy() {
            from_cfg
        } else {
            match std::env::var("NAS_MODEL_PATH") {
                Ok(v) if !v.is_empty() => PyValue::str(&v),
                _ => {
                    let from_man = get(&man, "nas_model_path").unwrap_or(PyValue::None);
                    if from_man.is_truthy() { from_man } else { PyValue::str(DEFAULT_NAS_HOST_ROOT) }
                }
            }
        }
    };
    let budget_raw = get(&cfg, "vram_budget_gb").unwrap_or(PyValue::None);
    if is_none(&budget_raw) {
        return Err(die(1, "config.vram_budget_gb 누락"));
    }
    let budget = float_value(&budget_raw)?;
    let margin = float_value(&get_or(&cfg, "safety_margin", PyValue::Float(DEFAULT_SAFETY_MARGIN)).map_err(raised)?)?;
    let _kv_bytes = int_of(&get_or(&cfg, "kv_cache_dtype_bytes", PyValue::int(DEFAULT_KV_BYTES)).map_err(raised)?).map_err(raised)?;
    let container_root = get_or(&cfg, "nas_container_root", PyValue::str("/app/models")).map_err(raised)?;

    let tp = match get(&snapshot, "tp") {
        Ok(v) if snapshot_has(&snapshot, "tp") => v,
        _ => PyValue::int(BigInt::from(resolve_tp(&cfg, root)?)),
    };
    let parsed = match model_config::parse(&model_path, &nas_root, &container_root) {
        Ok(parsed) => PyValue::Dict(parsed),
        Err(failure) if failure.class == "FileNotFoundError" => return Err(die_py(1, failure.message)),
        Err(failure) if failure.is_value_error() => {
            return Err(die_py(1, Message::new().lit("모델 config 해소 실패: ").text(&failure.message).done()))
        }
        Err(failure) => return Err(raised(failure)),
    };

    let recipe_warning = get(&recipe, "warning").map_err(raised)?;
    if str_of(&recipe_warning).as_deref() == Some("offline_quant_on_non_prequantized_checkpoint") {
        let mut warn = PyStr::new("[recipe] 경고: ");
        warn.push_py(recipe_id);
        warn.push_str(" 는 비prequantized 체크포인트에 사전양자화 전용 quant(");
        warn.push_py(&get(&recipe, "quantization").map_err(raised)?.display_text());
        warn.push_str(") — `vllm serve` 시 사전양자화 가중치 부재로 서빙 실패 가능(네트워크 무관). (fp8·bitsandbytes 는 온라인 양자화 가능.)");
        streams.eprint(&warn);
    }

    let out_root = join(&join(root, &s("output")), &s(topology_of(root)));
    let out_root_value = PyValue::Str(out_root);
    let force_value = PyValue::Bool(force);
    let topology = PyValue::str("single");
    let none = PyValue::None;
    let image_value = image.map_or(PyValue::None, |i| PyValue::Str(i.clone()));
    let request = Request {
        parsed: &parsed,
        recipe: &recipe,
        name: &name,
        repo_root: &out_root_value,
        port: &port,
        served_model_name: &served,
        force: &force_value,
        topology: &topology,
        vllm_version: &none,
        build_dockerfile: &none,
        image: &image_value,
    };
    let mut warnings = PyStr::default();
    let outcome = recipe_set::generate(&request, &mut warnings);
    streams.stderr.push_py(&warnings);
    let paths = match outcome {
        Ok(paths) => paths,
        Err(failure) if failure.class == "FileExistsError" => {
            let mut text = failure.message.clone();
            text.push_str(" (덮어쓰려면 --force)");
            return Err(die_py(1, text));
        }
        Err(failure) => return Err(raised(failure)),
    };
    streams.print(&PyStr::new("[recipe] 생성된 3종 세트:"));
    for path in &paths {
        let mut line = PyStr::new("  - ");
        line.push_py(path);
        streams.print(&line);
    }

    let now = || PyValue::Str(Clock::Local.now_iso());
    let model_id = {
        let from_parsed = get(&parsed, "model_id").map_err(raised)?;
        if from_parsed.is_truthy() { from_parsed } else { get(&snapshot, "model_id").map_err(raised)? }
    };
    let mut record = PyDict::new();
    record.set("timestamp", now());
    record.set("model_id", model_id);
    record.set("quantization", get(&recipe, "quantization").map_err(raised)?);
    record.set("max_model_len", get(&recipe, "max_model_len").map_err(raised)?);
    record.set("gpu_memory_utilization", get(&recipe, "gpu_memory_utilization").map_err(raised)?);
    record.set("vram_budget_gb", get_or(&snapshot, "vram_budget_gb", budget.clone()).map_err(raised)?);
    record.set("estimated_vram_gb", get(&recipe, "estimated_total_gib").map_err(raised)?);
    record.set("tensor_parallel_size", tp);
    record.set("safety_margin_threshold", get_or(&snapshot, "safety_margin", margin.clone()).map_err(raised)?);
    record.set("selection_timestamp", now());
    record.set("actual_vram_gb", PyValue::None);
    record.set("serve_success", PyValue::None);
    record.set("error_type", recipe_warning);
    record.set("tokens_per_sec", PyValue::None);
    let jsonl = join(feedback_dir, &s("recipe_feedback.jsonl"));
    feedback::append(&PyValue::Dict(record), &PyValue::Str(jsonl.clone()), &Clock::Local).map_err(raised)?;
    let mut note = PyStr::new("[recipe] 되먹임 로그 기록: ");
    note.push_py(&jsonl);
    streams.eprint(&note);
    Ok(())
}

fn snapshot_has(snapshot: &PyValue, key: &str) -> bool {
    match snapshot.peel() {
        PyValue::Dict(d) => d.contains_key(key),
        _ => false,
    }
}

fn exited(outcome: Outcome) -> Result<std::collections::BTreeMap<&'static str, Value>, Run> {
    match outcome {
        Outcome::Values(values) => Ok(values),
        Outcome::Exit { status, stdout, stderr } => {
            let writes = if stdout.is_empty() { Vec::new() } else { vec![stdout.clone()] };
            Err(Run { status, stdout, stderr, writes })
        }
    }
}

/// `_declared_axis(cfg, key, strict=False)` for the recorded-only `tp` axis: `(value, state, source sentence)`.
fn declared_axis_tp(cfg: &PyValue) -> (Option<i64>, &'static str, String) {
    let axes = truthy_get(cfg, "declared_axes");
    if is_none(&axes) {
        return (None, "absent", "config.declared_axes 블록 없음".to_string());
    }
    if !matches!(axes.peel(), PyValue::Dict(_)) {
        return (None, "malformed", "config.declared_axes 가 매핑이 아니다(비매핑)".to_string());
    }
    let PyValue::Dict(d) = axes.peel() else { unreachable!() };
    if d.get("tp").is_none() {
        return (None, "absent", "config.declared_axes.tp 키 없음".to_string());
    }
    let val = truthy_get(&axes, "tp");
    if is_none(&val) {
        return (None, "null", "config.declared_axes.tp=null(선언하지 않았다)".to_string());
    }
    match val.peel() {
        PyValue::Int(n) if !matches!(val.peel(), PyValue::Bool(_)) && *n >= BigInt::from(1) => {
            (i64::try_from(n.clone()).ok(), "declared", "config.declared_axes.tp".to_string())
        }
        _ => (
            None,
            "malformed",
            Message::new()
                .lit("config.declared_axes.tp=")
                .val(&val)
                .lit(" 는 양의 정수 또는 null 이 아니다(기재 · 차단 ✗)")
                .done()
                .to_utf8()
                .unwrap_or_default(),
        ),
    }
}

/// The front of `cmd_simulate` that is ported: config, TP, the two gmu roles and the declared-TP note (all on stderr).
fn simulate_front(streams: &mut Streams, config: &PyStr, root: &PyStr) -> Step<()> {
    let cfg = load_config(config)?;
    let target = truthy_get(&cfg, "target_model");
    let target = if target.is_truthy() { target } else { empty_dict() };
    if !truthy_get(&target, "path").is_truthy() {
        return Err(die(1, "config.target_model.path 누락"));
    }
    if is_none(&truthy_get(&cfg, "vram_budget_gb")) {
        return Err(die(1, "config.vram_budget_gb 누락"));
    }
    let tp = resolve_tp(&cfg, root)?;
    guard_tp(tp, root)?;
    let roles = gpu_roles(streams, &cfg, tp, root)?;
    streams.eprint(&PyStr::new(&format!(
        "[recipe] gmu 역할({} 흐름): deploy_gmu={:.3} ({}) · gate_margin={:.3} ({}) · budget=per_card_vram {:.2} GiB ({}) · gpu_model={} · TP={}(÷{}) · lockset gmu_source={}",
        roles.flow, roles.deploy_gmu, roles.deploy_gmu_source, roles.gate_margin, roles.gate_margin_source,
        roles.budget_gib, roles.budget_source, roles.gpu_model, tp, roles.tp_divisor, roles.gmu_source
    )));
    if roles.gmu_source != "target_gmu" {
        streams.eprint(&PyStr::new(&format!(
            "[recipe] ⚠ 배포 gmu 가 정본 칸(target_gpu.target_gmu)에서 오지 않았다 — {}. lockset gmu_source={} 로 각인한다.",
            roles.deploy_gmu_source, roles.gmu_source
        )));
    }
    if roles.deploy_gmu > roles.gate_margin + 1e-9 {
        streams.eprint(&PyStr::new(&format!(
            "[recipe] ⓘ deploy_gmu {:.3} > gate_margin {:.3} — 클램프 산식 천장(budget×deploy_gmu)이 검증 게이트 천장(budget×gate_margin)보다 높다. 게이트는 weights+overhead 만 본다(기재 · 차단 ✗).",
            roles.deploy_gmu, roles.gate_margin
        )));
    }
    let (dtp, dtp_state, dtp_src) = declared_axis_tp(&cfg);
    match dtp {
        Some(d) if d != tp => streams.eprint(&PyStr::new(&format!(
            "[recipe] ⚠ declared_axes.tp={} ≠ 해소 TP={} — TP 의 권위는 manifest 다. 선언을 고쳐라(기재 · 차단 ✗).",
            d, tp
        ))),
        None if dtp_state == "malformed" => streams.eprint(&PyStr::new(&format!(
            "[recipe] ⚠ {} — 해소 TP={} 와 대조하지 못했다(기재 · 차단 ✗).",
            dtp_src, tp
        ))),
        _ => {}
    }
    Ok(())
}

const TOP_USAGE: &str = "usage: recipe.py [-h] {estimate,generate,simulate} ...";

/// `recipe.py ...`: `main(argv)`. `skill_root` is the deployed skill (the binary's anchor).
pub fn recipe_main(args: &[PyStr], columns: Option<&str>, skill_root: &PyStr) -> Run {
    let mut streams = Streams::default();
    let command = args.first().and_then(|a| a.to_utf8());
    match command.as_deref() {
        Some("estimate") | Some("generate") => {}
        Some("simulate") => {
            let values = match exited(parse_arguments(&simulate_spec(), &args[1..], columns)) {
                Ok(values) => values,
                Err(run) => return run,
            };
            let root = repo_root(skill_root);
            // The Flag gate runs after the command line parses, as for estimate/generate.
            if let Err(d) = require_terraform_flag(&root) {
                return stop(streams, d);
            }
            let config = match values.get("config") {
                Some(Value::Text(t)) => t.clone(),
                _ => unreachable!("--config is required"),
            };
            if let Err(d) = simulate_front(&mut streams, &config, &root) {
                if d.code == -1 {
                    streams.stderr.push_py(&d.text);
                    streams.stderr.push_str("\n");
                    return streams.finish(1);
                }
                return stop(streams, d);
            }
            // The rest of cmd_simulate (lockset vocabulary, batch plan, trial loop, stamping) is still owned by recipe.py.
            streams.eprint(&PyStr::new("[recipe] simulate 본문 나머지(배치계획·트라이얼 루프·lockset 각인)는 아직 Rust 로 옮기지 않았다 — 설정·gmu 역할까지만 한다(recipe.py 가 원본)."));
            return streams.finish(2);
        }
        Some("-h") | Some("--help") => {
            streams.print(&PyStr::new(TOP_USAGE));
            return streams.finish(0);
        }
        _ => {
            streams.eprint(&PyStr::new(TOP_USAGE));
            streams.eprint(&PyStr::new("recipe.py: error: the following arguments are required: command"));
            return streams.finish(2);
        }
    }
    let is_generate = command.as_deref() == Some("generate");
    let spec = if is_generate { generate_spec() } else { estimate_spec() };
    let values = match exited(parse_arguments(&spec, &args[1..], columns)) {
        Ok(values) => values,
        Err(run) => return run,
    };
    let root = repo_root(skill_root);
    // The Flag gate runs after the command line parses and before anything else is read.
    if let Err(d) = require_terraform_flag(&root) {
        return stop(streams, d);
    }
    let text = |dest: &str| match values.get(dest) {
        Some(Value::Text(t)) => Some(t.clone()),
        _ => None,
    };
    let config = text("config").expect("a required option is present");
    if is_generate {
        let recipe_id = text("recipe_id").expect("a required option is present");
        let force = matches!(values.get("force"), Some(Value::Flag));
        let image = text("image");
        let feedback_dir = join(skill_root, &s("feedback"));
        return match cmd_generate(&mut streams, &config, &recipe_id, force, image.as_ref(), &root, &feedback_dir) {
            Ok(()) => streams.finish(0),
            Err(d) if d.code == -1 => {
                streams.stderr.push_py(&d.text);
                streams.stderr.push_str("\n");
                streams.finish(1)
            }
            Err(d) => stop(streams, d),
        };
    }
    let candidates = text("candidates");
    let auto = matches!(values.get("auto"), Some(Value::Flag));
    let feedback_dir = join(skill_root, &s("feedback"));
    match cmd_estimate(&mut streams, &config, candidates.as_ref(), auto, &root, &feedback_dir) {
        Ok(()) => streams.finish(0),
        Err(d) if d.code == -1 => {
            streams.stderr.push_py(&d.text);
            streams.stderr.push_str("\n");
            streams.finish(1)
        }
        Err(d) => stop(streams, d),
    }
}
