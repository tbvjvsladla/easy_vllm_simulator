//! `check_smoke_model.py`: the no-download NAS presence check of a smoke model, the early rope-extension and speculative-layout checks, the pre-load
//! RAM gate and the `BUDGET_PARAMS` / `GATE_PARAMS` emission. Exit status: 0 present (and gate passed), 2 model missing, 3 configuration, 7 RAM gate
//! refused, 8 speculative layout, 9 rope extension needed.
//!
//! The RAM gate and the checkpoint figures are the recipe owner's library (`recipe_runtime::memory`, not copied); the rope translator and the layout
//! check are run as their own programs, as the original ran them. Standard output is block-buffered as CPython's is when it is not a terminal
//! (a child that writes straight to the descriptor comes out before it).

use crate::gate_cli::{self, Values};
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;
use recipe_runtime::cliparse::{exit_with_text, Kind, Opt, Positional, Spec};
use recipe_runtime::memory::{self, GateArguments, Host, LogChoice, Provider};
use recipe_runtime::memory_system::System;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const PROGRAM: &str = "check_smoke_model.sh";
const MIB: i128 = 1024 * 1024;

fn opt(
    flags: &'static [&'static str],
    dest: &'static str,
    kind: Kind,
    help: Option<&'static str>,
) -> Opt {
    Opt {
        flags,
        dest,
        kind,
        help,
        required: false,
        group: None,
    }
}

pub fn spec() -> Spec {
    Spec {
        prog: PROGRAM,
        description: "",
        options: vec![
            opt(&["-h", "--help"], "help", Kind::Help, None),
            opt(&["--repo"], "repo", Kind::Text, None),
            opt(
                &["--topology"],
                "topology",
                Kind::Choice(&["single", "multi"]),
                Some("산출물 통로 output/<topology>/ 선택"),
            ),
            opt(
                &["--base"],
                "base",
                Kind::Text,
                Some("configs/·docker-compose.yaml 을 담은 디렉토리 직접 지정(--topology 보다 우선)"),
            ),
            opt(
                &["--no-ram-gate"],
                "no_ram_gate",
                Kind::Flag,
                Some("로드-전 RAM 게이트 생략(plan_26071019 §2.6 — 진단/강제 시)"),
            ),
            opt(
                &["--no-spec-layout-check"],
                "no_spec_layout_check",
                Kind::Flag,
                Some("spec-decoder 레이아웃 선검사 생략(testlog_26081419 §7 후속 ① — 진단 시)"),
            ),
            opt(
                &["--emit-gate-params"],
                "emit_gate_params",
                Kind::Flag,
                Some("게이트 통과 시 stdout 에 'GATE_PARAMS required_mib=<n>' 출력(멀티노드 스모크가 슬레이브 노드 동일-문턱 검사에 재사용 — §2.6 예방 대칭)"),
            ),
        ],
        group_required: false,
        positional: Some(Positional {
            dest: "config_name",
            help: None,
        }),
    }
}

/// Standard output as CPython buffers it for a pipe: kept until 8 KiB are pending or the program ends.
struct Console {
    buf: Vec<u8>,
}

impl Console {
    fn out(&mut self, text: &str) {
        self.buf.extend_from_slice(text.as_bytes());
        if self.buf.len() >= 8192 {
            self.flush();
        }
    }
    fn flush(&mut self) {
        let mut stdout = std::io::stdout().lock();
        let _ = stdout.write_all(&self.buf);
        let _ = stdout.flush();
        self.buf.clear();
    }
}

fn err(text: &str) {
    gate_cli::write_err(text);
}

// ── the configuration readers (line patterns of the original, no YAML) ──────────────────────────────

fn read_lines(path: &Path) -> Option<Vec<String>> {
    let bytes = std::fs::read(path).ok()?;
    Some(
        String::from_utf8_lossy(&bytes)
            .split_inclusive('\n')
            .map(|s| s.to_string())
            .collect(),
    )
}

/// `re.match(r"\s*<key words joined by [-_]>\s*<sep>\s*", line)`: the rest of the line after the separator, or `None`.
fn after_key<'a>(line: &'a str, words: &[&str], sep: char) -> Option<&'a str> {
    let mut rest = line.trim_start();
    for (at, word) in words.iter().enumerate() {
        if at > 0 {
            rest = rest.strip_prefix(['-', '_'])?;
        }
        rest = rest.strip_prefix(word)?;
    }
    rest = rest.trim_start().strip_prefix(sep)?;
    Some(rest.trim_start())
}

fn leading_digits(text: &str) -> Option<u64> {
    let digits: String = text.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

fn read_config_uint(cfg: &Path, words: &[&str]) -> Option<u64> {
    for line in read_lines(cfg)? {
        if let Some(rest) = after_key(&line, words, ':') {
            if let Some(n) = leading_digits(rest) {
                return Some(n);
            }
        }
    }
    None
}

fn read_config_tp(cfg: &Path) -> Option<u64> {
    read_config_uint(cfg, &["tensor", "parallel", "size"])
}

fn read_config_kv_mib(cfg: &Path) -> Option<u64> {
    read_config_uint(cfg, &["kv", "cache", "memory", "bytes"]).map(|b| b / (1024 * 1024))
}

fn read_config_max_model_len(cfg: &Path) -> Option<u64> {
    read_config_uint(cfg, &["max", "model", "len"])
}

fn read_config_declares_engram_file(cfg: &Path) -> bool {
    for line in read_lines(cfg).unwrap_or_default() {
        if line.trim_start().starts_with('#') {
            continue;
        }
        if after_key(&line, &["hf", "overrides"], ':').is_some() && line.contains("engram_table_dir") {
            return true;
        }
    }
    false
}

fn read_env_declares_ple_mmap(env: &Path) -> bool {
    for line in read_lines(env).unwrap_or_default() {
        if line.trim_start().starts_with('#') {
            continue;
        }
        if let Some(rest) = after_key(&line, &["VLLM_PLE_MMAP"], '=') {
            let value: String = rest.chars().take_while(|c| !c.is_whitespace()).collect();
            let value = value.trim_matches(['\'', '"']).to_lowercase();
            if ["1", "true", "yes"].contains(&value.as_str()) {
                return true;
            }
        }
    }
    false
}

fn read_env_declares_engram_disk(env: &Path) -> bool {
    if !env.exists() {
        return false;
    }
    for line in read_lines(env).unwrap_or_default() {
        let s = line.trim();
        if s.starts_with('#') || !s.contains('=') {
            continue;
        }
        let (k, v) = s.split_once('=').unwrap();
        if k.trim() == "DSV41_ENGRAM_DISK" && v.trim().trim_matches(['"', '\'']) == "1" {
            return true;
        }
    }
    false
}

fn triplet_declares_rope_override(cfg: &Path) -> bool {
    for line in read_lines(cfg).unwrap_or_default() {
        if line.trim_start().starts_with('#') {
            continue;
        }
        for key in ["hf-overrides", "hf_overrides", "rope-scaling", "rope_scaling"] {
            let mut from = 0;
            while let Some(at) = line[from..].find(key) {
                let tail = &line[from + at + key.len()..];
                if tail.trim_start().starts_with(':') {
                    return true;
                }
                from += at + key.len();
            }
        }
        if line.contains("VLLM_ALLOW_LONG_MAX_MODEL_LEN") {
            return true;
        }
    }
    false
}

fn read_manifest_tp(base: &Path) -> Option<u64> {
    let path = base.join("manifest.yaml");
    if !path.is_file() {
        return None;
    }
    let lines = read_lines(&path)?;
    let gpus = lines
        .iter()
        .find_map(|l| after_key(l, &["gpus_per_node"], ':').and_then(leading_digits))?;
    let topo = lines.iter().find_map(|l| {
        after_key(l, &["topology"], ':').map(|rest| {
            rest.chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect::<String>()
        })
    });
    if topo.as_deref().is_some_and(|t| !t.is_empty() && t.trim() == "single") {
        return Some(gpus);
    }
    let roles = lines
        .iter()
        .filter(|l| {
            let mut rest = l.trim_start();
            if let Some(r) = rest.strip_prefix('-') {
                rest = r.trim_start();
            }
            let Some(rest) = rest.strip_prefix("role") else {
                return false;
            };
            let Some(rest) = rest.trim_start().strip_prefix(':') else {
                return false;
            };
            let rest = rest.trim_start();
            ["main", "sub"].iter().any(|w| {
                rest.strip_prefix(w)
                    .is_some_and(|t| !t.chars().next().is_some_and(|c| c.is_alphanumeric() || c == '_'))
            })
        })
        .count() as u64;
    Some(gpus * roles.max(1))
}

fn read_model_path(cfg: &Path) -> Option<String> {
    for line in read_lines(cfg)? {
        if let Some(rest) = after_key(&line, &["model"], ':') {
            let token: String = rest.chars().take_while(|c| !c.is_whitespace()).collect();
            if !token.is_empty() {
                return Some(token.trim().trim_matches(['\'', '"']).to_string());
            }
        }
    }
    None
}

/// `re.search(r"-\s*(\S+):<root>(?::[a-z]+)?\s*(?:#.*)?$", line)` over each line: the token of the first line that matches.
fn read_app_models_host_token(compose: &Path, root: &str) -> Option<String> {
    let needle = format!(":{root}");
    for raw in read_lines(compose)? {
        let line = raw.strip_suffix('\n').unwrap_or(&raw);
        for (dash, _) in line.match_indices('-') {
            let after = line[dash + 1..].trim_start();
            let run: &str = after.split(char::is_whitespace).next().unwrap_or("");
            if run.is_empty() {
                continue;
            }
            let run_end = line.len() - after.len() + run.len();
            let beyond = &line[run_end..];
            // the largest split of the run into token + ":<root>" + mode whose tail is blank-or-comment
            let mut best: Option<usize> = None;
            for (k, _) in run.match_indices(&needle) {
                if k == 0 {
                    continue;
                }
                let mut tail = &run[k + needle.len()..];
                if let Some(m) = tail.strip_prefix(':') {
                    let n = m.chars().take_while(|c| c.is_ascii_lowercase()).count();
                    if n > 0 {
                        tail = &m[n..];
                    }
                }
                let whole = format!("{tail}{beyond}");
                let trimmed = whole.trim_start();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    best = Some(k);
                }
            }
            if let Some(k) = best {
                return Some(run[..k].to_string());
            }
        }
    }
    None
}

fn read_manifest_field(base: &Path, field: &str) -> Option<String> {
    let path = base.join("manifest.yaml");
    if !path.is_file() {
        return None;
    }
    for line in read_lines(&path)? {
        if let Some(rest) = after_key(&line, &[field], ':') {
            let token: String = rest.chars().take_while(|c| !c.is_whitespace()).collect();
            if !token.is_empty() {
                return Some(token.trim().trim_matches(['\'', '"']).to_string());
            }
        }
    }
    None
}

fn resolve_app_models_host_root(token: &str, base: &Path) -> Option<String> {
    // re.fullmatch(r"\$\{(\w+)(?::-([^}]*))?\}", token)
    let inner = token.strip_prefix("${").and_then(|t| t.strip_suffix('}'));
    let parsed = inner.and_then(|inner| {
        let name_len = inner
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .map(char::len_utf8)
            .sum::<usize>();
        if name_len == 0 {
            return None;
        }
        let (name, rest) = inner.split_at(name_len);
        if rest.is_empty() {
            Some((name, None))
        } else {
            let default = rest.strip_prefix(":-")?;
            if default.contains('}') {
                return None;
            }
            Some((name, Some(default)))
        }
    });
    let Some((var, default)) = parsed else {
        return if token.is_empty() { None } else { Some(token.to_string()) };
    };
    if let Ok(v) = std::env::var(var) {
        if !v.is_empty() {
            return Some(v);
        }
    }
    if let Some(m) = read_manifest_field(base, &var.to_lowercase()) {
        return Some(m);
    }
    default.filter(|d| !d.is_empty()).map(str::to_string)
}

// ── the RAM gate (the recipe owner's library) ──────────────────────────────────────────────────────

struct GateHost(System);

impl Provider for GateHost {
    fn mem_available_mib(&mut self) -> memory_res::Res<PyValue> {
        self.0.mem_available_mib()
    }
    fn try_drop_caches(&mut self) -> memory_res::Res<bool> {
        self.0.try_drop_caches()
    }
}

impl Host for GateHost {
    fn write_stderr(&mut self, text: &PyStr) -> memory_res::Res<()> {
        gate_cli::write_err(&text.to_utf8().unwrap_or_default());
        Ok(())
    }
    fn call_log(&mut self, _message: &PyStr) -> memory_res::Res<()> {
        Err(Failure::new("RuntimeError", "no callable log"))
    }
}

mod memory_res {
    pub type Res<T> = Result<T, agent_runtime_support::failure::Failure>;
}

fn as_int(value: &PyValue) -> Option<i128> {
    match value.peel() {
        PyValue::Int(n) => num_traits::ToPrimitive::to_i128(n),
        _ => None,
    }
}

fn warn_list(warns: &PyValue) -> Vec<String> {
    match warns {
        PyValue::List(items) => items
            .iter()
            .map(|w| crate::budget_preflight::show(Some(w)))
            .collect(),
        _ => Vec::new(),
    }
}

fn print_new_warnings(warns: &PyValue, from: usize) {
    for w in warn_list(warns).iter().skip(from) {
        err(&format!("[NAS-check] ⚠ {w}\n"));
    }
}

/// The whole RAM-gate block. `Ok(Some(7))` = refused; an internal error is the caller's fail-open warning.
#[allow(clippy::too_many_arguments)]
fn ram_gate(
    console: &mut Console,
    host_path: &str,
    cfg: &Path,
    base: &Path,
    config_name: &str,
    tp: u64,
    emit: bool,
) -> Result<Option<i32>, Failure> {
    let hp = PyValue::str(host_path);
    let mut warns = PyValue::List(Vec::new());
    let ckpt = memory::checkpoint_bytes_for(&hp, Some(&mut warns))?;
    print_new_warnings(&warns, 0);
    let mut eng: Option<i128> = None;
    let env_cell = base.join("envs").join(format!(".env.{config_name}"));
    if read_config_declares_engram_file(cfg) || read_env_declares_engram_disk(&env_cell) {
        let n1 = warn_list(&warns).len();
        let v = memory::offloadable_bytes_for(&hp, &PyValue::str("engram_file"), Some(&mut warns))?;
        eng = as_int(&v);
        print_new_warnings(&warns, n1);
    }
    let ckpt_i = as_int(&ckpt).filter(|c| *c != 0);
    let mut ckpt_gate = ckpt.clone();
    if let (Some(c), Some(e)) = (ckpt_i, eng) {
        if e > 0 {
            ckpt_gate = PyValue::int(i64::try_from((c - e).max(1)).unwrap_or(i64::MAX));
            err(&format!(
                "[NAS-check] engram=file-backed 선언 → 게이트 ckpt 에서 −{}MiB(상주 ✗)\n",
                e / MIB
            ));
        }
    }
    if as_int(&ckpt_gate).is_some_and(|c| c != 0) && read_env_declares_ple_mmap(&env_cell) {
        let n2 = warn_list(&warns).len();
        let v = memory::offloadable_bytes_for(&hp, &PyValue::str("ple_mmap"), Some(&mut warns))?;
        print_new_warnings(&warns, n2);
        if let Some(p) = as_int(&v).filter(|p| *p > 0) {
            let g = as_int(&ckpt_gate).unwrap_or(0);
            ckpt_gate = PyValue::int(i64::try_from((g - p).max(1)).unwrap_or(i64::MAX));
            err(&format!(
                "[NAS-check] PLE=mmap 선언 → 게이트 ckpt 에서 −{}MiB(상주 ✗)\n",
                p / MIB
            ));
        }
    }
    let mut host = GateHost(System::new());
    let tp_v = PyValue::int(tp as i64);
    let floor = PyValue::int(10240);
    let auto_drop = PyValue::Bool(true);
    let res = memory::gate(
        &mut host,
        &GateArguments {
            checkpoint_bytes: &ckpt_gate,
            tp: &tp_v,
            floor_mib: &floor,
            auto_drop: &auto_drop,
            log: LogChoice::Default,
        },
    )?;
    if emit && ckpt_i.is_some() {
        let kvm = read_config_kv_mib(cfg);
        let n0 = warn_list(&warns).len();
        let ple = memory::offloadable_bytes_for(&hp, &PyValue::str("ple_mmap"), Some(&mut warns))?;
        print_new_warnings(&warns, n0);
        console.out(&format!(
            "[NAS-check] BUDGET_PARAMS ckpt_mib={} tp={} kv_mib={} ple_mib={} engram_mib={}\n",
            ckpt_i.unwrap() / MIB,
            tp,
            kvm.map(|k| k.to_string()).unwrap_or_else(|| "none".into()),
            as_int(&ple).map(|p| (p / MIB).to_string()).unwrap_or_else(|| "unknown".into()),
            match eng {
                Some(e) if e != 0 => (e / MIB).to_string(),
                _ => "none".into(),
            },
        ));
    }
    let show = |k: &str| crate::budget_preflight::show(res.get(k));
    let ok = matches!(res.get("ok"), Some(PyValue::Bool(true)));
    let skipped = matches!(res.get("skipped"), Some(PyValue::Bool(true)));
    if ok && !skipped {
        console.out(&format!(
            "[NAS-check] RAM-gate PASS: MemAvailable={}MiB ≥ required={}MiB(ckpt÷tp={}+floor)\n",
            show("avail_after_mib"),
            show("required_mib"),
            tp
        ));
        if emit {
            if let Some(r) = res.get("required_mib").and_then(as_int).filter(|r| *r != 0) {
                console.out(&format!("[NAS-check] GATE_PARAMS required_mib={r}\n"));
            }
        }
    }
    if !ok {
        err(&format!(
            "[NAS-check] STOP: 로드-전 RAM 게이트 거부 — MemAvailable={}MiB < required={}MiB(ckpt÷tp={}+floor). 잔존 컨테이너/페이지캐시 정리 후 재시도 (헌법 호스트 안전체계 따름정리)\n",
            show("avail_after_mib"),
            show("required_mib"),
            tp
        ));
        return Ok(Some(7));
    }
    Ok(None)
}

fn rope_check(scripts: &Path, host_path: &str, cfg: &Path, mml: u64) -> Option<i32> {
    let elf = scripts
        .join("../../vllm-recipe-explorer/rust/runtime/target/release/rope_scaling_translate");
    if !elf.is_file() {
        err("[NAS-check] ⚠ rope 번역기 부재 — 컨텍스트 확장 선판정 생략\n");
        return None;
    }
    let output = Command::new(&elf)
        .args(["--model-dir", host_path, "--target-len", &mml.to_string(), "--json"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    let Ok(PyValue::Dict(rope)) = agent_runtime_support::pyjson::loads(&text) else {
        return None;
    };
    let truthy = |k: &str| rope.get(k).is_some_and(|v| v.is_truthy());
    if truthy("needed") && !triplet_declares_rope_override(cfg) {
        let show = |k: &str| crate::budget_preflight::show(rope.get(k));
        err(&format!(
            "[NAS-check] STOP: max-model-len={mml} 이 모델 네이티브 {}({})를 넘는데 트리플렛에 rope 확장 인자가 없다.\n",
            show("native_max_position_embeddings"),
            show("native_source")
        ));
        err("     ⇒ 이대로 로드하면 vLLM 이 ValidationError 로 즉사한다. 로드는 **0초도 시작하지 않았다**.\n");
        if truthy("hf_overrides") {
            err("     트리플렛에 이 줄을 넣어라(기존 rope 키를 전부 실은 병합 결과다 — 갈아끼우면 mrope·partial rotary 가 사라진다):\n");
            let dumped = rope
                .get("hf_overrides")
                .and_then(|v| v.dumps_ascii_tight().ok())
                .unwrap_or_default();
            err(&format!("       hf-overrides: '{dumped}'\n"));
        }
        for (key, prefix) in [("unknowns", "     ⚠ 미검증: "), ("reasons", "     · ")] {
            if key == "reasons" || key == "unknowns" {
                if let Some(PyValue::List(items)) = rope.get(key) {
                    for item in items {
                        err(&format!("{prefix}{}\n", crate::budget_preflight::show(Some(item))));
                    }
                }
            }
        }
        return Some(9);
    }
    None
}

pub fn run(script: Option<&PyStr>, args: &[PyStr]) -> i32 {
    let mut console = Console { buf: Vec::new() };
    let code = run_inner(&mut console, script, args);
    console.flush();
    code
}

fn run_inner(console: &mut Console, script: Option<&PyStr>, args: &[PyStr]) -> i32 {
    let values: Values = match gate_cli::parse_args(&spec(), args) {
        Ok(v) => v,
        Err(status) => return status,
    };
    let Some(config_name) = gate_cli::text(&values, "config_name") else {
        let columns = gate_cli::columns();
        if let recipe_runtime::cliparse::Outcome::Exit { stderr, .. } = exit_with_text(
            &spec(),
            recipe_runtime::cliparse::terminal_width(Some(&columns)),
            &PyStr::new("the following arguments are required: config_name"),
        ) {
            gate_gli_emit(&stderr);
        }
        return 2;
    };
    let repo = gate_cli::text(&values, "repo").unwrap_or_else(|| ".".into());
    let join = |a: &str, b: &str| -> PathBuf {
        let b = Path::new(b);
        if b.is_absolute() {
            b.to_path_buf()
        } else {
            Path::new(a).join(b)
        }
    };
    let base: PathBuf = if let Some(b) = gate_cli::text(&values, "base").filter(|b| !b.is_empty()) {
        join(&repo, &b)
    } else if let Some(t) = gate_cli::text(&values, "topology") {
        Path::new(&repo).join("output").join(t)
    } else {
        err("[NAS-check] FAIL: --topology {single|multi} 또는 --base 필요 (configs·compose 는 output/<topology>/ 통로에 있음 — plan_26062312)\n");
        return 3;
    };
    let cfg = base.join("configs").join(format!("{config_name}.yaml"));
    let compose = base.join("docker-compose.yaml");
    if !cfg.exists() {
        err(&format!("[NAS-check] FAIL: 트리플릿 yaml 없음 — {}\n", cfg.display()));
        return 3;
    }
    let Some(model_ctr) = read_model_path(&cfg) else {
        err(&format!("[NAS-check] FAIL: {} 에 model: 없음\n", cfg.display()));
        return 3;
    };
    if !compose.exists() {
        err(&format!(
            "[NAS-check] FAIL: docker-compose.yaml 없음 — {} (render 산출물 통로 확인 — output/<topology>/)\n",
            compose.display()
        ));
        return 3;
    }
    let parts: Vec<&str> = model_ctr.split('/').collect();
    let container_root = if parts.len() >= 3 {
        parts[..3].join("/")
    } else {
        "/app/models".to_string()
    };
    let Some(token) = read_app_models_host_token(&compose, &container_root) else {
        err(&format!("[NAS-check] FAIL: docker-compose 에서 {container_root} 호스트 마운트 못 찾음\n"));
        return 3;
    };
    let Some(host_root) = resolve_app_models_host_root(&token, &base) else {
        err(&format!(
            "[NAS-check] FAIL: {container_root} 호스트 루트 해소 실패(token={token}) — manifest.nas_model_path 또는 NAS_MODEL_PATH env 확인\n"
        ));
        return 3;
    };
    let host_path = model_ctr.replacen(&container_root, &host_root, 1);
    let scripts = gate_cli::scripts_dir(script, "check_smoke_model");
    if Path::new(&host_path).is_dir() {
        console.out(&format!("[NAS-check] OK: {config_name} → {host_path} 존재\n"));
        if let Some(mml) = read_config_max_model_len(&cfg) {
            if let Some(code) = rope_check(&scripts, &host_path, &cfg, mml) {
                return code;
            }
        }
        if !gate_cli::flag(&values, "no_spec_layout_check") {
            let spec_script = scripts.join("check_spec_layout.sh");
            if spec_script.is_file() {
                let status = Command::new("bash")
                    .arg(&spec_script)
                    .args(["--model-dir", &host_path, "--config"])
                    .arg(&cfg)
                    .status();
                let rc = status.ok().and_then(|s| s.code()).unwrap_or(-1);
                if rc == 8 {
                    err("[NAS-check] STOP: spec-decoder 레이아웃 불일치 — 위 사유 참조 (로드 전 차단)\n");
                    return 8;
                } else if rc != 0 {
                    err(&format!("[NAS-check] ⚠ spec 레이아웃 선검사 비정상 종료(rc={rc}) — 차단하지 않음\n"));
                }
            } else {
                err("[NAS-check] ⚠ check_spec_layout.sh 부재 — spec 레이아웃 선검사 생략\n");
            }
        }
        if !gate_cli::flag(&values, "no_ram_gate") {
            let tp = read_config_tp(&cfg).or_else(|| read_manifest_tp(&base));
            match tp {
                None => err("[NAS-check] ⚠ serve TP 해소 실패(트리플렛·manifest 모두) — RAM 게이트 생략(음성정직·false-block 금지)\n"),
                Some(tp) => {
                    let emit = gate_cli::flag(&values, "emit_gate_params");
                    match ram_gate(console, &host_path, &cfg, &base, &config_name, tp, emit) {
                        Ok(Some(code)) => return code,
                        Ok(None) => {}
                        Err(f) => err(&format!(
                            "[NAS-check] ⚠ RAM 게이트 내부 오류 — 생략(모델 실재는 OK): {}({:?})\n",
                            f.class,
                            f.message.to_utf8().unwrap_or_default()
                        )),
                    }
                }
            }
        }
        return 0;
    }
    err(&format!(
        "[NAS-check] STOP: 모델 부재 — {host_path}\n  (config_name={config_name}, container={model_ctr})\n  → 규칙: 다운로드하지 않음. 사람이 모델을 NAS 에 사전 다운로드해야 함.\n"
    ));
    2
}

fn gate_gli_emit(text: &PyStr) {
    gate_cli::write_err(&text.to_utf8().unwrap_or_default());
}
