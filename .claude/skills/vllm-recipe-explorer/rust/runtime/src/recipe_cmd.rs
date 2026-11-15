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

const TOP_USAGE: &str = "usage: recipe.py [-h] {estimate,generate,simulate} ...";

/// `recipe.py ...`: `main(argv)`. `skill_root` is the deployed skill (the binary's anchor).
pub fn recipe_main(args: &[PyStr], columns: Option<&str>, skill_root: &PyStr) -> Run {
    let mut streams = Streams::default();
    let command = args.first().and_then(|a| a.to_utf8());
    match command.as_deref() {
        Some("estimate") | Some("generate") => {}
        Some("simulate") => {
            streams.eprint(
                &Message::new()
                    .lit("[recipe] 미이식: `simulate` 는 아직 Rust 로 옮기지 않았다(recipe.py 가 원본 · run_trial·lockset writer 선행 이식 대기).")
                    .done(),
            );
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
