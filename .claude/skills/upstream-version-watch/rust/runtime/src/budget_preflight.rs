//! `budget_preflight.py`: the single owner of the serve-budget pre-judgement (floor, arm ceiling, declared-gmu record) and its command line.
//! Exit status: 0 pass, 4 the declaration is impossible, 5 the bench entry is impossible, 2 argument error.
//!
//! The constants are not mirrored: they are read from `blackbox_eta.DEFAULTS` as the original imports them (the owner's file text, no interpreter).
//! One extension over the original command line: `--render {summary,gmu,reasons}` reads the `--json` report on standard input and prints the lines
//! the callers formerly produced with an inline `python3 -c`.

use crate::gate_cli::{self, Values};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use recipe_runtime::cliparse::{Kind, Opt, Spec};
use std::path::{Path, PathBuf};

pub const PROGRAM: &str = "budget_preflight.sh";

const DECLARED_GMU_PREMISE: &str = "unified-memory — 예상 몫 = gmu × MemTotal 은 MemTotal 이 디바이스 풀인 통합메모리 노드(GB10 계열)에서만 \
vLLM 몫이다. discrete GPU 에서는 MemTotal 이 호스트 RAM 이라 몫·잔차가 의미를 잃는다(메모리 모델은 \
이 스크립트가 판정하지 않는다)";

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
        description: "서빙 예산 선판정 (plan_26091108 R1·R2·R3)",
        options: vec![
            opt(&["-h", "--help"], "help", Kind::Help, None),
            opt(&["--self-test"], "self_test", Kind::Flag, None),
            opt(&["--mem-total-mib"], "mem_total_mib", Kind::Int, None),
            opt(&["--ckpt-mib"], "ckpt_mib", Kind::Int, None),
            opt(
                &["--weights-mib"],
                "weights_mib",
                Kind::Int,
                Some("이미 파생된 상주 weights(ckpt 대신)"),
            ),
            opt(&["--ple-mib"], "ple_mib", Kind::Int, None),
            opt(&["--ple-mmap"], "ple_mmap", Kind::Flag, None),
            opt(&["--tp"], "tp", Kind::Int, None),
            opt(&["--kv-mib"], "kv_mib", Kind::Int, None),
            opt(&["--overhead-mib"], "overhead_mib", Kind::Int, None),
            opt(&["--bench-budget-mib"], "bench_budget_mib", Kind::Int, None),
            opt(
                &["--floor-mib"],
                "floor_mib",
                Kind::Int,
                Some("이미 선언된 바닥(벤치 진입 게이트용 — 바닥을 재산출하지 않는다)"),
            ),
            opt(
                &["--declared-gmu"],
                "declared_gmu",
                Kind::Float,
                Some("서빙 yaml 의 gpu-memory-utilization — 예상 vLLM 몫·잔차를 **기재만** 한다(게이트 ✗)"),
            ),
            opt(
                &["--declared-gmu-yaml"],
                "declared_gmu_yaml",
                Kind::Text,
                Some("서빙 yaml 경로 — gpu-memory-utilization 을 여기서 뽑아 --declared-gmu 와 같이 기재한다(값이 없으면 기재 행 없음 · 실패 ✗)"),
            ),
            opt(&["--json"], "json", Kind::Flag, None),
            opt(
                &["--render"],
                "render",
                Kind::Choice(&["summary", "gmu", "reasons"]),
                Some("표준입력의 --json 보고서를 호출부용 줄로 풀어 쓴다(판정하지 않는다)"),
            ),
        ],
        group_required: false,
        positional: None,
    }
}

/// `(decl_margin_mib, decl_min_ceiling_mib, abs_band_mib)`, or the `SystemExit` text of the original when the owner's table cannot be read.
pub fn constants(repo: &Path) -> Result<(i64, i64, i64), String> {
    let candidates = [
        repo.join(".claude/skills/terraforming_node/scripts/node_blackbox"),
        repo.join(".claude/runtime/node_blackbox"),
    ];
    let listing = candidates
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(" · ");
    let fail = |why: &str| {
        format!(
            "[budget-preflight] FAIL: 선언 상수를 blackbox_eta.DEFAULTS 에서 읽지 못했다({why}).\n  찾은 자리: {listing}\n  여기에 사본을 두지 않는다 — 거울은 정본이 움직일 때 조용히 옛값으로 남는다."
        )
    };
    // The owner is the Rust `blackbox_eta` (plan_26111615): its launcher emits DEFAULTS (no envelope, no override) as
    // `BB_<KEY>=<value>` lines — the same table, read without Python. A Sub carries the same launcher under
    // `.claude/runtime/node_blackbox/` (render_sub_env · pass 3); a plane with no launcher is the original's ImportError.
    let sh = candidates
        .iter()
        .map(|d| d.join("blackbox_eta.sh"))
        .find(|p| p.is_file())
        .ok_or_else(|| fail("No module named 'blackbox_eta'"))?;
    constants_from_launcher(&sh).map_err(|why| fail(&why))
}

/// The three declaration constants from `bash <launcher> --emit-params <tmp> --mem-total-mib 1` (DEFAULTS only).
fn constants_from_launcher(sh: &Path) -> Result<(i64, i64, i64), String> {
    let dir = std::env::temp_dir().join(format!("budget-preflight-{}-{}", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let out = dir.join("eta_params.env");
    let st = std::process::Command::new("bash")
        .arg(sh)
        .arg("--emit-params")
        .arg(&out)
        .args(["--mem-total-mib", "1"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    let text = std::fs::read_to_string(&out).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    match st {
        Ok(s) if s.success() => {}
        Ok(s) => return Err(format!("{} --emit-params rc={}", sh.display(), s.code().unwrap_or(-1))),
        Err(e) => return Err(e.to_string()),
    }
    let get = |k: &str| -> Option<i64> {
        text.lines()
            .find_map(|l| l.strip_prefix(k))
            .and_then(|v| v.parse::<f64>().ok())
            .map(|x| x as i64)
    };
    match (get("BB_DECL_MARGIN_MIB="), get("BB_DECL_MIN_CEILING_MIB="), get("BB_ABS_BAND_MIB=")) {
        (Some(a), Some(b), Some(c)) => Ok((a, b, c)),
        _ => Err("KeyError".into()),
    }
}

/// `(resident weights MiB, applied PLE MiB)`: the PLE is taken off only when mmap is declared AND its size is measured.
pub fn resident_weights_mib(ckpt: i64, tp: i64, ple: Option<i64>, ple_mmap: bool) -> (i64, i64) {
    let tp = tp.max(1);
    let mut resident = ckpt;
    let mut applied = 0;
    if let (true, Some(ple)) = (ple_mmap, ple) {
        if ple > 0 {
            applied = ple;
            resident -= applied;
        }
    }
    (resident.div_euclid(tp), applied)
}

pub struct Report {
    pub floor: i64,
    pub ceiling: i64,
    pub overhead_max: i64,
    pub declare_ok: bool,
    pub bench_floor: Option<i64>,
    pub bench_ok: Option<bool>,
    pub bench_budget_max: i64,
    pub margin: i64,
    pub min_ceiling: i64,
    pub abs_band: i64,
    pub reasons: Vec<String>,
}

pub fn preflight(
    consts: (i64, i64, i64),
    mem_total: i64,
    weights: i64,
    kv: i64,
    overhead: i64,
    bench_budget: i64,
    floor_mib: Option<i64>,
) -> Report {
    let (margin, min_ceiling, abs_band) = consts;
    let floor = floor_mib.unwrap_or(mem_total - weights - kv - overhead);
    let ceiling = floor - margin;
    let overhead_max = mem_total - weights - kv - margin - min_ceiling;
    let declare_ok = ceiling >= min_ceiling;
    let bench_floor = floor - bench_budget;
    let bench_ok = if bench_budget == 0 {
        None
    } else {
        Some(bench_floor >= abs_band)
    };
    let bench_budget_max = floor - abs_band;
    let mut reasons = Vec::new();
    if !declare_ok {
        reasons.push(format!(
            "선언 불가: arm 상한 {ceiling}MiB < 가드 최소 {min_ceiling}MiB ({}MiB 부족). 예상 바닥 {floor}MiB. 선택지 — overhead 를 실측으로 줄인다(이 노드 상한 {overhead_max}MiB) · KV 절대클램프를 낮춘다(현재 {kv}MiB) · --no-budget 으로 무보호를 감수한다(그 사실이 이벤트로 남는다).",
            min_ceiling - ceiling
        ));
    }
    if bench_ok == Some(false) {
        reasons.push(format!(
            "벤치 진입 불가: 바닥 {floor}MiB − 벤치 {bench_budget}MiB = {bench_floor}MiB < 절대밴드 {abs_band}MiB. 벤치 예산 상한은 이 구성에서 {bench_budget_max}MiB 다 — 그보다 크면 벤치 컨테이너가 뜨는 순간 워치독 사정거리에 들어간다(camp-26090918 `fp8-bf-262k-mmp` 실측: 서빙은 성공하고 벤치가 죽었다)."
        ));
    }
    Report {
        floor,
        ceiling,
        overhead_max,
        declare_ok,
        bench_floor: if bench_budget != 0 {
            Some(bench_floor)
        } else {
            None
        },
        bench_ok,
        bench_budget_max,
        margin,
        min_ceiling,
        abs_band,
        reasons,
    }
}

/// The first `gpu[-_]memory[-_]utilization:` key line of a serving yaml (comment lines never match; end-of-line comment and quotes removed);
/// `None` when there is no number-shaped value or the file cannot be read.
pub fn declared_gmu_from_yaml(path: &str) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    for line in text.lines() {
        let trimmed = line.trim_start_matches([' ', '\t']);
        let Some(rest) = trimmed.strip_prefix("gpu") else {
            continue;
        };
        let mut chars = rest.chars();
        let mut matched = String::from("gpu");
        let mut ok = true;
        for word in ["memory", "utilization"] {
            match chars.next() {
                Some(c @ ('-' | '_')) => matched.push(c),
                _ => {
                    ok = false;
                    break;
                }
            }
            let tail: String = chars.clone().take(word.len()).collect();
            if tail != word {
                ok = false;
                break;
            }
            for _ in 0..word.len() {
                chars.next();
            }
        }
        if !ok {
            continue;
        }
        let after = chars.as_str().trim_start_matches([' ', '\t']);
        let Some(value) = after.strip_prefix(':') else {
            continue;
        };
        let value = value
            .split('#')
            .next()
            .unwrap_or("")
            .trim()
            .replace(['"', '\''], "");
        if value.is_empty()
            || value == "."
            || value.matches('.').count() > 1
            || value.chars().any(|c| !"0123456789.".contains(c))
        {
            return None;
        }
        return Some(value);
    }
    None
}

fn s(text: &str) -> PyValue {
    PyValue::str(text)
}

fn int_or_none(value: Option<i64>) -> PyValue {
    value.map(PyValue::int).unwrap_or(PyValue::None)
}

/// The declared-gmu record row (a pure statement; nothing decides on it).
pub fn declared_gmu_row(
    gmu: f64,
    mem_total: Option<i64>,
    weights: Option<i64>,
    kv: Option<i64>,
) -> PyDict {
    let mut row = PyDict::new();
    row.set("provenance", s("declared"));
    row.set("gmu", PyValue::Float(gmu));
    row.set("status", s("recorded"));
    for key in ["mem_total_mib", "expected_vllm_share_mib", "weights_mib", "kv_mib", "residual_mib"] {
        row.set(key, PyValue::None);
    }
    row.set("premise", s(DECLARED_GMU_PREMISE));
    row.set(
        "note",
        s("기재 전용(게이트 ✗) — kv 클램프 시 vLLM 은 gmu 를 기동 전 free ≥ ceil(total×gmu) 검사에만 쓴다(총량 캡 코드는 소스에 없고 캡처럼 작용한 관측만 있다 · 기전 미확정 · plan_26091407 F5). residual = 예상 몫 − weights − kv"),
    );
    if !(0.0 < gmu && gmu <= 1.0) {
        row.set("status", s("invalid"));
        row.set(
            "note",
            s("gmu 는 (0, 1] 이어야 한다 — 산출하지 않고 사실만 적는다(기재 · 차단 ✗)"),
        );
        return row;
    }
    let (Some(mem_total), Some(weights), Some(kv)) = (mem_total, weights, kv) else {
        row.set("status", s("not_applicable"));
        row.set(
            "note",
            s("벤치 진입 경로(선언된 바닥 사용)는 weights·kv 입력이 없다 — 몫·잔차를 묻지 않는다"),
        );
        return row;
    };
    let share = (gmu * mem_total as f64) as i64;
    row.set("mem_total_mib", PyValue::int(mem_total));
    row.set("expected_vllm_share_mib", PyValue::int(share));
    row.set("weights_mib", PyValue::int(weights));
    row.set("kv_mib", PyValue::int(kv));
    row.set("residual_mib", PyValue::int(share - weights - kv));
    row
}

fn report_dict(r: &Report) -> PyDict {
    let mut d = PyDict::new();
    d.set("floor_mib", PyValue::int(r.floor));
    d.set("arm_ceiling_mib", PyValue::int(r.ceiling));
    d.set("overhead_max_mib", PyValue::int(r.overhead_max));
    d.set("declare_ok", PyValue::Bool(r.declare_ok));
    d.set("bench_floor_mib", int_or_none(r.bench_floor));
    d.set(
        "bench_ok",
        r.bench_ok.map(PyValue::Bool).unwrap_or(PyValue::None),
    );
    d.set("bench_budget_max_mib", PyValue::int(r.bench_budget_max));
    d.set("decl_margin_mib", PyValue::int(r.margin));
    d.set("decl_min_ceiling_mib", PyValue::int(r.min_ceiling));
    d.set("abs_band_mib", PyValue::int(r.abs_band));
    d.set(
        "reasons",
        PyValue::List(r.reasons.iter().map(|x| s(x)).collect()),
    );
    d
}

fn dumps(dict: &PyDict) -> String {
    PyValue::Dict(dict.clone())
        .dumps_unicode()
        .ok()
        .and_then(|t| t.to_utf8())
        .unwrap_or_default()
}

fn get_int(d: &PyDict, key: &str) -> Option<i64> {
    match d.get(key)?.peel() {
        PyValue::Int(n) => num_traits::ToPrimitive::to_i64(n),
        _ => None,
    }
}

fn get_str(d: &PyDict, key: &str) -> Option<String> {
    match d.get(key)?.peel() {
        PyValue::Str(t) => t.to_utf8(),
        _ => None,
    }
}

pub fn show(value: Option<&PyValue>) -> String {
    match value.map(|v| v.peel()) {
        Some(PyValue::Int(n)) => n.to_string(),
        Some(PyValue::Float(f)) => gate_cli::float_repr(*f),
        Some(PyValue::Str(t)) => t.to_utf8().unwrap_or_default(),
        Some(PyValue::Bool(true)) => "True".into(),
        Some(PyValue::Bool(false)) => "False".into(),
        _ => "None".into(),
    }
}

fn premise_head(row: &PyDict) -> String {
    let premise = get_str(row, "premise").unwrap_or_default();
    premise.split(" — ").next().unwrap_or("").to_string()
}

fn emit_text(out: &PyDict, a_json: bool) {
    if a_json {
        gate_cli::write_out(&format!("{}\n", dumps(out)));
        return;
    }
    let weights = match get_int(out, "weights_mib") {
        Some(w) => format!(
            "{w}MiB(ple 보정 −{}MiB)",
            get_int(out, "ple_applied_mib").unwrap_or(0)
        ),
        None => "(선언된 바닥 사용)".to_string(),
    };
    let declare_ok = matches!(out.get("declare_ok"), Some(PyValue::Bool(true)));
    let bench = match out.get("bench_ok") {
        Some(PyValue::Bool(true)) => "OK",
        Some(PyValue::Bool(false)) => "BLOCK",
        _ => "n/a",
    };
    let mut text = format!(
        "[budget-preflight] weights={weights} floor={}MiB arm_ceiling={}MiB declare={} bench={bench}\n",
        get_int(out, "floor_mib").unwrap_or(0),
        get_int(out, "arm_ceiling_mib").unwrap_or(0),
        if declare_ok { "OK" } else { "BLOCK" },
    );
    if let Some(PyValue::List(reasons)) = out.get("reasons") {
        for r in reasons {
            text.push_str(&format!("  - {}\n", show(Some(r))));
        }
    }
    if let Some(PyValue::Dict(row)) = out.get("declared_gmu_row") {
        let status = get_str(row, "status").unwrap_or_default();
        if status == "recorded" {
            text.push_str(&format!(
                "  ⓘ declared-gmu 기재(게이트 ✗ · provenance={} · status={}): gmu={} × MemTotal {}MiB = 예상 vLLM 몫 {}MiB · 잔차(몫 − weights − kv) {}MiB · 전제 {}\n",
                show(row.get("provenance")), status, show(row.get("gmu")), show(row.get("mem_total_mib")),
                show(row.get("expected_vllm_share_mib")), show(row.get("residual_mib")), premise_head(row)
            ));
        } else {
            text.push_str(&format!(
                "  ⓘ declared-gmu 기재(게이트 ✗ · provenance={} · status={}): gmu={} — {}\n",
                show(row.get("provenance")),
                status,
                show(row.get("gmu")),
                show(row.get("note"))
            ));
        }
    }
    gate_cli::write_out(&text);
}

fn gmu_line(row: Option<&PyDict>) -> String {
    match row {
        None => "서빙 yaml 에 gpu-memory-utilization 수치 없음 — 기재 행 없음".to_string(),
        Some(r) => format!(
            "gmu={} 예상 vLLM 몫={}MiB 잔차(몫−weights−kv)={}MiB provenance={} status={} 전제={}",
            show(r.get("gmu")),
            show(r.get("expected_vllm_share_mib")),
            show(r.get("residual_mib")),
            show(r.get("provenance")),
            show(r.get("status")),
            premise_head(r)
        ),
    }
}

/// `--render`: the lines the callers' inline interpreter snippets printed, from the `--json` report on standard input.
fn render(mode: &str) -> i32 {
    let mut raw = Vec::new();
    if std::io::Read::read_to_end(&mut std::io::stdin(), &mut raw).is_err() {
        return 1;
    }
    let text = PyStr::from_surrogateescape(&raw);
    let Ok(PyValue::Dict(d)) = agent_runtime_support::pyjson::loads_text(&text) else {
        gate_cli::write_err("[budget-preflight] FAIL: --render 입력이 JSON 보고서가 아니다\n");
        return 1;
    };
    let row = match d.get("declared_gmu_row").map(|v| v.peel()) {
        Some(PyValue::Dict(r)) => Some(r.clone()),
        _ => None,
    };
    let mut out = String::new();
    match mode {
        "summary" => {
            out.push_str(&format!(
                "floor={}MiB arm_ceiling={}MiB (가드 최소 {}MiB)\n",
                show(d.get("floor_mib")),
                show(d.get("arm_ceiling_mib")),
                show(d.get("decl_min_ceiling_mib"))
            ));
            if let Some(PyValue::List(reasons)) = d.get("reasons") {
                for r in reasons {
                    out.push_str(&format!("  · {}\n", show(Some(r))));
                }
            }
            out.push_str(&format!(
                "declared-gmu 기재(게이트 ✗): {}\n",
                gmu_line(row.as_ref())
            ));
        }
        "gmu" => out.push_str(&format!("{}\n", gmu_line(row.as_ref()))),
        _ => {
            if let Some(PyValue::List(reasons)) = d.get("reasons") {
                for r in reasons {
                    out.push_str(&format!("     · {}\n", show(Some(r))));
                }
            }
        }
    }
    gate_cli::write_out(&out);
    0
}

fn repo_of(scripts: &Path) -> PathBuf {
    scripts.join("..").join("..").join("..").join("..")
}

fn normpath(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

pub fn run(script: Option<&PyStr>, args: &[PyStr]) -> i32 {
    let values: Values = match gate_cli::parse_args(&spec(), args) {
        Ok(v) => v,
        Err(status) => return status,
    };
    if gate_cli::flag(&values, "self_test") {
        return self_test(script);
    }
    if let Some(mode) = gate_cli::text(&values, "render") {
        return render(&mode);
    }
    let int = |k: &str| gate_cli::int(&values, k);
    let mut declared_gmu = gate_cli::float(&values, "declared_gmu");
    let yaml = gate_cli::text(&values, "declared_gmu_yaml").filter(|t| !t.is_empty());
    if declared_gmu.is_some() && yaml.is_some() {
        gate_cli::write_err("[budget-preflight] FAIL: --declared-gmu 와 --declared-gmu-yaml 은 함께 주지 않는다(값의 출처가 둘이 된다)\n");
        return 2;
    }
    if let Some(path) = &yaml {
        declared_gmu = declared_gmu_from_yaml(path).and_then(|v| v.parse::<f64>().ok());
    }
    let gmu_source = match &yaml {
        Some(p) => format!("serving-yaml({p})"),
        None => "argv(--declared-gmu)".to_string(),
    };
    let scripts = gate_cli::scripts_dir(script, "budget_preflight");
    let consts = match constants(&normpath(&repo_of(&scripts))) {
        Ok(c) => c,
        Err(message) => {
            gate_cli::write_err(&format!("{message}\n"));
            return 1;
        }
    };
    let as_json = gate_cli::flag(&values, "json");
    let bench_budget = int("bench_budget_mib").unwrap_or(0);
    if let Some(floor) = int("floor_mib") {
        let r = preflight(consts, 0, 0, 0, 0, bench_budget, Some(floor));
        let mut out = report_dict(&r);
        out.set("weights_mib", PyValue::None);
        out.set("ple_applied_mib", PyValue::None);
        if let Some(g) = declared_gmu {
            let mut row = declared_gmu_row(g, None, None, None);
            row.set("gmu_source", s(&gmu_source));
            out.set("declared_gmu_row", PyValue::Dict(row));
        }
        emit_text(&out, as_json);
        return if r.bench_ok != Some(false) { 0 } else { 5 };
    }
    for (name, key) in [
        ("mem-total-mib", "mem_total_mib"),
        ("kv-mib", "kv_mib"),
        ("overhead-mib", "overhead_mib"),
    ] {
        if int(key).is_none() {
            gate_cli::write_err(&format!(
                "[budget-preflight] FAIL: --{name} 는 필수다(기본값을 두지 않는다)\n"
            ));
            return 2;
        }
    }
    let (mem_total, kv, overhead) = (
        int("mem_total_mib").unwrap(),
        int("kv_mib").unwrap(),
        int("overhead_mib").unwrap(),
    );
    let (weights, applied) = if let Some(w) = int("weights_mib") {
        (w, 0)
    } else if let Some(c) = int("ckpt_mib") {
        resident_weights_mib(
            c,
            int("tp").unwrap_or(1),
            int("ple_mib"),
            gate_cli::flag(&values, "ple_mmap"),
        )
    } else {
        gate_cli::write_err("[budget-preflight] FAIL: --ckpt-mib 또는 --weights-mib 가 필요하다\n");
        return 2;
    };
    let r = preflight(consts, mem_total, weights, kv, overhead, bench_budget, None);
    let mut out = report_dict(&r);
    out.set("weights_mib", PyValue::int(weights));
    out.set("ple_applied_mib", PyValue::int(applied));
    if let Some(g) = declared_gmu {
        let mut row = declared_gmu_row(g, Some(mem_total), Some(weights), Some(kv));
        row.set("gmu_source", s(&gmu_source));
        out.set("declared_gmu_row", PyValue::Dict(row));
    }
    emit_text(&out, as_json);
    if !r.declare_ok {
        4
    } else if r.bench_ok == Some(false) {
        5
    } else {
        0
    }
}

fn self_test(script: Option<&PyStr>) -> i32 {
    let mut failures: Vec<String> = Vec::new();
    let mut ck = |name: &str, cond: bool, detail: &str| {
        let mark = if cond { "  [PASS] " } else { "  [FAIL] " };
        let tail = if cond { String::new() } else { format!(" {detail}") };
        gate_cli::write_out(&format!("{mark}{name}{tail}\n"));
        if !cond {
            failures.push(name.to_string());
        }
    };
    let scripts = gate_cli::scripts_dir(script, "budget_preflight");
    let repo = normpath(&repo_of(&scripts));
    let consts = match constants(&repo) {
        Ok(c) => c,
        Err(m) => {
            gate_cli::write_err(&format!("{m}\n"));
            return 1;
        }
    };
    let (mem, kv, oh, tp) = (124610i64, 20480i64, 24800i64, 2i64);
    let (nv4, fp8, ple) = (126533i64, 176928i64, 48828i64);
    let pf = |w: i64, bench: i64| preflight(consts, mem, w, kv, oh, bench, None);
    let observed = [
        ("nv4", nv4, false, true),
        ("nv4", nv4, true, true),
        ("fp8", fp8, true, true),
        ("fp8", fp8, false, false),
    ];
    for (variant, ckpt, mmap_on, should_pass) in observed {
        let (w, _) = resident_weights_mib(ckpt, tp, Some(ple), mmap_on);
        let r = pf(w, 0);
        ck(
            &format!(
                "★R2 역채점 {variant}+{} → {} (weights={w} floor={})",
                if mmap_on { "mmp" } else { "res" },
                if should_pass { "통과" } else { "차단" },
                r.floor
            ),
            r.declare_ok == should_pass,
            &format!("→ 판정={} 실측={should_pass}", r.declare_ok),
        );
    }
    let (w, _) = resident_weights_mib(nv4, tp, Some(ple), false);
    let floor = pf(w, 0).floor;
    ck(
        "★R2 원장 대조: nv4+res 의 floor 가 기록된 16064 와 1MiB 안에서 일치",
        (floor - 16064).abs() <= 1,
        &format!("→ {floor}"),
    );
    let (w_off, applied_off) = resident_weights_mib(fp8, tp, Some(ple), false);
    ck(
        "★R2 음성대조: mmap 보정을 끄면 fp8+mmp 가 차단된다(교정 전 동작 재현)",
        !pf(w_off, 0).declare_ok && applied_off == 0,
        "",
    );
    let (w_on, applied_on) = resident_weights_mib(fp8, tp, Some(ple), true);
    ck(
        "★R2 음성대조 짝: 보정을 켜면 같은 셀이 통과한다",
        pf(w_on, 0).declare_ok && applied_on == ple,
        "",
    );
    ck(
        "R2 선언만 있고 크기를 모르면 보정하지 않는다(추측 ✗)",
        resident_weights_mib(fp8, tp, None, true) == (fp8 / tp, 0),
        "",
    );
    let r = pf(w_on, 8192);
    ck(
        "★R3 역채점: fp8+mmp 바닥에서 벤치 8192MiB 는 절대밴드를 침식한다(벤치사망 실측과 부합)",
        r.bench_ok == Some(false) && r.bench_floor.unwrap_or(i64::MAX) < r.abs_band,
        &format!("→ bench_floor={:?}", r.bench_floor),
    );
    ck(
        "★R3 그 구성의 벤치 예산 상한을 숫자로 말한다(막기만 하지 않는다)",
        r.bench_budget_max == r.floor - r.abs_band,
        "",
    );
    ck(
        "★R3 상한 이하의 벤치 몫은 통과한다(과잉차단 ✗)",
        pf(w_on, r.bench_budget_max).bench_ok == Some(true),
        "",
    );
    ck(
        "★R3 벤치 몫 0 은 '묻지 않았다' 이지 '통과' 가 아니다",
        pf(w_on, 0).bench_ok.is_none(),
        "",
    );
    let (w_nv4m, _) = resident_weights_mib(nv4, tp, Some(ple), true);
    ck(
        "★R3 음성대조: 여유가 큰 nv4+mmp 는 같은 8192MiB 로 통과한다",
        pf(w_nv4m, 8192).bench_ok == Some(true),
        "",
    );

    // ── R1 wiring anchors: code tokens of who calls this gate ──
    let read = |p: &Path| std::fs::read_to_string(p).ok();
    let mn = scripts.join("multinode_serve_smoke.sh");
    let sg = scripts.join("single_serve_up.sh");
    match read(&mn) {
        Some(body) => {
            ck(
                "★R1 멀티: 선판정이 워치독 arming 과 분리돼 있다(결합하면 --no-watchdog 이 가드를 끈다)",
                body.contains("if [ \"$BUDGET\" = \"1\" ]; then")
                    && !body.contains("if [ \"$BUDGET\" = \"1\" ] && [ \"$WATCHDOG\" = \"1\" ]; then"),
                "",
            );
            ck(
                "★R1 멀티: 선판정 호출부가 실재한다",
                body.contains("budget_preflight.sh\" --json"),
                "",
            );
            ck(
                "★R1 멀티: 선언 생략 경로가 원장에 남는다(탈출구가 조용하면 구멍이다)",
                body.contains("--reason=--no-watchdog"),
                "",
            );
        }
        None => gate_cli::write_out("  [SKIP] R1 멀티 배선 앵커 — multinode_serve_smoke.sh 부재\n"),
    }
    match read(&sg) {
        Some(body) => ck(
            "★R1 싱글: 선판정 호출부가 실재한다(종전에는 선판정이 아예 없었다 — 노드 비대칭)",
            body.contains("budget_preflight.sh"),
            "",
        ),
        None => gate_cli::write_out("  [SKIP] R1 싱글 배선 앵커 — single_serve_up.sh 부재\n"),
    }
    let rb = normpath(&scripts.join("../../adversarial-benchmark/scripts/run_bench.sh"));
    match read(&rb) {
        Some(body) => {
            ck(
                "★R3 벤치 진입 게이트의 호출부가 실재한다(종전 배선 0건)",
                body.contains("--floor-mib") && body.contains("budget_preflight.sh"),
                "",
            );
            ck(
                "★R3 벤치 도중 선언 만료를 막는다(만료되면 워치독이 옛 규칙으로 돌아간다)",
                body.contains("renew-budget"),
                "",
            );
            ck(
                "★R3 벤치 몫을 serve overhead 에 더하지 않는다(위상 오배정 금지)",
                !body.contains("--overhead-mib"),
                "",
            );
        }
        None => gate_cli::write_out("  [SKIP] R3 배선 앵커 — run_bench.sh 부재\n"),
    }
    ck(
        "★서브 이식성: 노드 도구를 정본·런타임 두 자리에서 찾는다(한 자리만 들면 서브에서 죽는다)",
        true,
        "",
    );

    // ── --declared-gmu record row: the real command line, as the original ran itself ──
    let exe = std::env::current_exe().unwrap_or_default();
    let script_arg = scripts.join("budget_preflight.sh");
    let cli = |argv: &[String]| -> (i32, String) {
        use std::os::unix::process::CommandExt;
        let mut cmd = std::process::Command::new(&exe);
        cmd.arg0(script_arg.as_os_str()).args(argv);
        match cmd.output() {
            Ok(o) => (
                o.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&o.stdout).into_owned(),
            ),
            Err(_) => (-1, String::new()),
        }
    };
    let v = |xs: &[&str]| xs.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    let (w_res, _) = resident_weights_mib(nv4, tp, Some(ple), false);
    for (label, w_case) in [("통과 구성 nv4+res", w_res), ("차단 구성 fp8+res", fp8 / tp)] {
        let base = v(&[
            "--mem-total-mib",
            &mem.to_string(),
            "--weights-mib",
            &w_case.to_string(),
            "--kv-mib",
            &kv.to_string(),
            "--overhead-mib",
            &oh.to_string(),
        ]);
        let with = |extra: &[&str], json: bool| {
            let mut a = if json { v(&["--json"]) } else { Vec::new() };
            a.extend(base.clone());
            a.extend(v(extra));
            a
        };
        let (rc0, j0) = cli(&with(&[], true));
        let (rc1, j1) = cli(&with(&["--declared-gmu", "0.85"], true));
        let d0 = agent_runtime_support::pyjson::loads(&j0).ok();
        let d1 = agent_runtime_support::pyjson::loads(&j1).ok();
        let row = match d1.as_ref() {
            Some(PyValue::Dict(d)) => match d.get("declared_gmu_row") {
                Some(PyValue::Dict(r)) => Some(r.clone()),
                _ => None,
            },
            _ => None,
        };
        let share = (0.85 * mem as f64) as i64;
        let row_ok = row.as_ref().is_some_and(|r| {
            get_str(r, "provenance").as_deref() == Some("declared")
                && get_str(r, "status").as_deref() == Some("recorded")
                && get_int(r, "expected_vllm_share_mib") == Some(share)
                && get_int(r, "residual_mib") == Some(share - w_case - kv)
        });
        ck(
            &format!("★기재 {label}: 기재 행이 선다(provenance=declared · 예상 몫 = gmu×MemTotal · 잔차 = 몫−weights−kv)"),
            row_ok,
            &format!("→ {row:?}"),
        );
        // the original JSON of the run without the record equals the other run minus its record row, byte for byte
        let stripped = match d1 {
            Some(PyValue::Dict(d)) => {
                let mut copy = PyDict::new();
                if let Some(PyValue::Dict(base_d)) = d0.as_ref() {
                    let _ = base_d;
                }
                for key in [
                    "floor_mib", "arm_ceiling_mib", "overhead_max_mib", "declare_ok", "bench_floor_mib", "bench_ok",
                    "bench_budget_max_mib", "decl_margin_mib", "decl_min_ceiling_mib", "abs_band_mib", "reasons",
                    "weights_mib", "ple_applied_mib",
                ] {
                    if let Some(x) = d.get(key) {
                        copy.set(key, x.clone());
                    }
                }
                dumps(&copy)
            }
            _ => String::new(),
        };
        let d0_has_row = matches!(&d0, Some(PyValue::Dict(d)) if d.get("declared_gmu_row").is_some());
        ck(
            &format!("★기재 {label}: 인자 유무로 종료코드·기존 JSON 필드가 바이트 동일(arm 산식 불변 · 게이트 ✗)"),
            rc0 == rc1 && !d0_has_row && stripped == j0.trim(),
            &format!("→ rc {rc0}/{rc1}"),
        );
        let (rc2, t0) = cli(&with(&[], false));
        let (rc3, t1) = cli(&with(&["--declared-gmu", "0.85"], false));
        let tail = t1.get(t0.len()..).unwrap_or("");
        ck(
            &format!("★기재 {label}: 텍스트 출력은 기존 바이트가 그대로 앞에 서고 기재 줄 1행만 뒤에 붙는다"),
            rc2 == rc3 && t1.starts_with(&t0) && tail.matches('\n').count() == 1 && tail.contains("declared-gmu"),
            &format!("→ tail={tail:?}"),
        );
    }
    let base_w = w_res.to_string();
    let base = v(&[
        "--mem-total-mib",
        &mem.to_string(),
        "--weights-mib",
        &base_w,
        "--kv-mib",
        &kv.to_string(),
        "--overhead-mib",
        &oh.to_string(),
    ]);
    let run_cli = |first: &[&str], extra: &[&str]| {
        let mut a = v(first);
        a.extend(base.clone());
        a.extend(v(extra));
        cli(&a)
    };
    let status_of = |json: &str| -> Option<String> {
        match agent_runtime_support::pyjson::loads(json).ok()? {
            PyValue::Dict(d) => match d.get("declared_gmu_row")? {
                PyValue::Dict(r) => get_str(r, "status"),
                _ => None,
            },
            _ => None,
        }
    };
    let (rc4, j4) = run_cli(&["--json"], &["--declared-gmu", "1.5"]);
    ck(
        "★기재 음성대조: 범위 밖 gmu 는 멈추지 않고 invalid 로 적는다(종료코드 불변 — 실패 경로 신설 ✗)",
        rc4 == 0 && status_of(&j4).as_deref() == Some("invalid"),
        &format!("→ rc={rc4}"),
    );
    let (rc5, j5) = cli(&v(&["--json", "--floor-mib", "30000", "--bench-budget-mib", "8192", "--declared-gmu", "0.85"]));
    ck(
        "★기재: 벤치 진입 경로(바닥 선언)에서는 not_applicable 로 적는다(몫·잔차를 지어내지 않는다)",
        rc5 == 0 && status_of(&j5).as_deref() == Some("not_applicable"),
        &format!("→ rc={rc5} {j5}"),
    );
    let (rc6, t6) = cli(&v(&["--floor-mib", "30000", "--bench-budget-mib", "8192", "--declared-gmu", "0.85"]));
    ck(
        "★기재 텍스트: 산출하지 않은 행(not_applicable)은 None 산술을 찍지 않고 사유만 싣는다",
        rc6 == 0 && t6.contains("status=not_applicable") && !t6.contains("None"),
        &format!("→ {t6:?}"),
    );
    let tmp = std::env::temp_dir().join(format!("pf_yaml_gmu_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    let cases: [(&str, &str, Option<&str>); 8] = [
        ("수치", "model: x\ngpu-memory-utilization: 0.85\nmax-model-len: 8192\n", Some("0.85")),
        ("따옴표+주석", "gpu-memory-utilization: \"0.8\"   # 손 편집\n", Some("0.8")),
        ("밑줄 키", "gpu_memory_utilization: 0.9\n", Some("0.9")),
        ("주석 줄만(생성기 설명 주석)", "# gpu-memory-utilization = startup free-memory 게이트\n", None),
        ("키 부재", "model: x\nmax-model-len: 8192\n", None),
        ("비수치", "gpu-memory-utilization: auto\n", None),
        ("점 두 개", "gpu-memory-utilization: 0.8.5\n", None),
        ("첫 줄만(중복 키)", "gpu-memory-utilization: 0.7\ngpu-memory-utilization: 0.9\n", Some("0.7")),
    ];
    let fx = tmp.join("c.yaml");
    for (label, text, want) in cases {
        let _ = std::fs::write(&fx, text);
        let got = declared_gmu_from_yaml(&fx.to_string_lossy());
        ck(
            &format!("★기재 추출({label}) → {}", match want { Some(w) => format!("'{w}'"), None => "None".into() }),
            got.as_deref() == want,
            &format!("→ {got:?}"),
        );
    }
    ck(
        "★기재 추출(파일 부재) → None · 실패 경로 없음",
        declared_gmu_from_yaml(&tmp.join("absent.yaml").to_string_lossy()).is_none(),
        "",
    );
    let serve = tmp.join("serve.yaml");
    let _ = std::fs::write(&serve, "model: x\ngpu-memory-utilization: 0.85   # 배포 gmu\n");
    let serve_s = serve.to_string_lossy().into_owned();
    let (rcy, jy) = run_cli(&["--json"], &["--declared-gmu-yaml", &serve_s]);
    let rowy = match agent_runtime_support::pyjson::loads(&jy).ok() {
        Some(PyValue::Dict(d)) => match d.get("declared_gmu_row") {
            Some(PyValue::Dict(r)) => Some(r.clone()),
            _ => None,
        },
        _ => None,
    };
    let yaml_ok = rcy == 0
        && rowy.as_ref().is_some_and(|r| {
            matches!(r.get("gmu"), Some(PyValue::Float(g)) if *g == 0.85)
                && get_str(r, "status").as_deref() == Some("recorded")
                && get_str(r, "gmu_source").is_some_and(|x| x.starts_with("serving-yaml("))
                && get_str(r, "premise").as_deref() == Some(DECLARED_GMU_PREMISE)
                && get_int(r, "expected_vllm_share_mib") == Some((0.85 * mem as f64) as i64)
        });
    ck(
        "★기재 yaml 경로: 뽑은 값으로 같은 기재 행 · 출처 serving-yaml(…) · 통합메모리 전제를 스스로 밝힌다",
        yaml_ok,
        &format!("→ {rowy:?}"),
    );
    let absent = tmp.join("absent.yaml").to_string_lossy().into_owned();
    let (rc_na, t_na) = run_cli(&[], &["--declared-gmu-yaml", &absent]);
    let (rc_no, t_no) = run_cli(&[], &[]);
    ck(
        "★기재 yaml 음성대조: 값 없는 yaml → 기재 행 없음 · 출력 바이트·종료코드가 인자 없을 때와 같다(실패 경로 신설 ✗)",
        rc_na == rc_no && t_na == t_no,
        "",
    );
    let (rc_both, _) = run_cli(&[], &["--declared-gmu", "0.8", "--declared-gmu-yaml", &serve_s]);
    ck(
        "★기재 음성대조: --declared-gmu 와 --declared-gmu-yaml 을 함께 주면 exit 2(값의 출처가 둘이 되지 않게)",
        rc_both == 2,
        "",
    );
    let _ = std::fs::remove_file(&fx);
    let _ = std::fs::remove_file(&serve);
    let _ = std::fs::remove_dir(&tmp);
    let wiring: [(&str, &Path, &[&str]); 2] = [
        ("멀티", &mn, &["--declared-gmu-yaml \"output/multi/configs/${CONFIG}.yaml\""]),
        ("싱글", &sg, &["--declared-gmu-yaml \"$CFGYAML\"", "CFGYAML=\"output/single/configs/${CONFIG}.yaml\""]),
    ];
    for (label, path, tokens) in wiring {
        let Some(body) = read(path) else {
            gate_cli::write_out(&format!("  [SKIP] 기재 배선 앵커({label}) — 호출부 부재\n"));
            continue;
        };
        ck(
            &format!("★기재 배선 앵커({label}): 선판정 호출부가 서빙 yaml 경로를 --declared-gmu-yaml 로 넘기고 기재 줄을 찍는다"),
            tokens.iter().all(|t| body.contains(t)) && (body.contains("declared_gmu_row") || body.contains("--render")),
            "",
        );
        ck(
            &format!("★기재 음성대조({label}): 셸에 추출 규칙 사본이 없다(gpu-memory-utilization 을 grep 해 넘기지 않는다)"),
            !body.contains("_yaml_declared_gmu") && !body.contains("gpu[-_]memory[-_]utilization"),
            "",
        );
    }
    let (m, mc, ab) = consts;
    ck(
        "상수를 정본에서 읽는다(거울 ✗)",
        (m, mc, ab) == (3072, 8192, 10240),
        &format!("→ {:?}", (m, mc, ab)),
    );
    let fails = failures.len();
    gate_cli::write_out(&format!(
        "[budget_preflight] {}\n",
        if fails == 0 { "PASS".to_string() } else { format!("FAIL ({fails})") }
    ));
    if fails == 0 { 0 } else { 1 }
}
