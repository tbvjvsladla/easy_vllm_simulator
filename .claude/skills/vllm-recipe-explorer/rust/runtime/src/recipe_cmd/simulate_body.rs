//! `cmd_simulate` of recipe.py (the trial loop): lockset vocabulary, candidate, batch plan, trial loop (RAM gate → run_trial →
//! sim_classify → adjust → repeat, two-phase convergence), the converged path (three-file set, lockset stamp, summary, feedback)
//! and the Model-C HITL report. Numerics live in `simulate_math`; the rest wires the package's other ports.

use super::*;
use crate::classifier;
use crate::estimate_vram::required_kv_bytes;
use crate::memory::{self, GateArguments, LogChoice, Provider};
use crate::memory_system::System;
use crate::run_trial::{mock_result, PROVENANCE_DRY_RUN, PROVENANCE_MOCK};
use crate::run_trial_session::{pre_session_gates, run_session, SessionError, SessionOpts};
use crate::simlog::{self, Plain};
use crate::simulate_math::{self as sm, Plan, Profile};
use std::collections::BTreeMap;

const BATCH_DERIVED: [&str; 2] = ["declared-requirement", "kv-fit-measured"];
const KV_DERIVED: [&str; 1] = ["measured-clamp"];
const GIB: f64 = 1073741824.0;
const VOCAB_REL: &str = ".claude/skills/terraforming_node/scripts/campaign_template_validator.py";

/// What ends the run early: a `_die` or a bare process exit (text already formatted for stderr).
pub enum Stop {
    Die(Die),
    Raw(i32, String),
}

impl From<Die> for Stop {
    fn from(d: Die) -> Stop {
        Stop::Die(d)
    }
}

type R<T> = Result<T, Stop>;

fn g(v: &PyValue, k: &str) -> PyValue {
    get(v, k).unwrap_or(PyValue::None)
}

fn cg(d: &PyDict, k: &str) -> PyValue {
    d.get(k).cloned().unwrap_or(PyValue::None)
}

fn dict_of(v: &PyValue) -> PyDict {
    match v.peel() {
        PyValue::Dict(d) => d.clone(),
        _ => PyDict::new(),
    }
}

fn dv(d: &PyDict) -> PyValue {
    PyValue::Dict(d.clone())
}

fn disp(v: &PyValue) -> String {
    v.display_text().to_utf8().unwrap_or_default()
}

fn to_i64(v: &PyValue) -> Option<i64> {
    match v.peel() {
        PyValue::Int(n) => i64::try_from(n.clone()).ok(),
        PyValue::Float(f) => Some(f.trunc() as i64),
        PyValue::Bool(b) => Some(*b as i64),
        _ => None,
    }
}

fn to_f64(v: &PyValue) -> Option<f64> {
    float_of(v).ok().filter(|_| !is_none(v))
}

fn fl(x: f64) -> PyValue {
    PyValue::Float(x)
}

fn oi(x: Option<i64>) -> PyValue {
    x.map_or(PyValue::None, PyValue::int)
}

fn os(x: Option<&str>) -> PyValue {
    x.map_or(PyValue::None, PyValue::str)
}

fn sl<T>(r: simlog::Done<T>) -> R<T> {
    r.map_err(|t| Stop::Die(die(-1, &format!("simlog 기록 실패: {:?}", t.raised))))
}

fn raise<T>(r: Res<T>) -> R<T> {
    r.map_err(|f| Stop::Die(raised(f)))
}

/// `a + b` over numbers (int + int stays int).
fn padd(a: &PyValue, b: &PyValue) -> PyValue {
    match (a.peel(), b.peel()) {
        (PyValue::Int(x), PyValue::Int(y)) => PyValue::Int(x + y),
        _ => fl(to_f64(a).unwrap_or(0.0) + to_f64(b).unwrap_or(0.0)),
    }
}

// ── lockset vocabulary (owner: campaign_template_validator.py, read as text — no Python) ─────────────────────────────────

pub struct Vocab {
    provenance: Vec<String>,
    knobs: BTreeMap<String, Vec<String>>,
}

fn quoted(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut parts = text.split('"');
    parts.next();
    while let (Some(inside), _) = (parts.next(), parts.next()) {
        out.push(inside.to_string());
    }
    out
}

fn lockset_vocab(root: &PyStr) -> Step<Vocab> {
    let path = join(root, &s(VOCAB_REL));
    if !is_file(&path) {
        return Err(die(5, &format!("lockset 출처 어휘의 소유자가 없다: {} — explorer 는 provenance·*_source 를 각인하므로 어휘 없이 진행하지 않는다(사본을 두지 않는다 · 메인이면 저장소 손상, explorer 가 tool_plane 으로 열린 서브면 재배달).", VOCAB_REL)));
    }
    let text = read_bytes(&path).map(|b| String::from_utf8_lossy(&b).into_owned()).map_err(raised)?;
    let section = |marker: &str, close: &str| -> Option<String> {
        let at = text.find(marker)? + marker.len();
        let end = text[at..].find(close)?;
        Some(text[at..at + end].to_string())
    };
    let missing = |name: &str| die(5, &format!("lockset 출처 어휘의 소유자({})에 {} 가 없다 — 옛 판 검증기다(explorer 와 캠페인 도구가 같은 배달로 수렴하지 않았다). explorer 는 어휘 없이 각인하지 않는다 — 메인이면 저장소를 확인하고, explorer 가 tool_plane 으로 열린 서브면 메인이 두 파일을 함께 재배달한다.", VOCAB_REL, name));
    let prov = section("LOCKSET_PROVENANCE = (", ")").ok_or_else(|| missing("LOCKSET_PROVENANCE"))?;
    let knobs_text = section("LOCKSET_KNOB_SOURCES = {", "\n}").ok_or_else(|| missing("LOCKSET_KNOB_SOURCES"))?;
    let mut knobs = BTreeMap::new();
    for line in knobs_text.lines() {
        if let Some(at) = line.find("\": (") {
            let key = line[..at].trim().trim_start_matches('"').to_string();
            knobs.insert(key, quoted(&line[at + 3..]));
        }
    }
    Ok(Vocab { provenance: quoted(&prov), knobs })
}

/// `(LOCKSET_PROVENANCE, LOCKSET_KNOB_SOURCES)` of the lockset vocabulary owner.
pub type LocksetVocabulary = (Vec<String>, BTreeMap<String, Vec<String>>);

/// The owner's lockset vocabulary below `root`, read the way `simulate` reads it; the stop text when it cannot be read (for the
/// self-test, which judges stamped locksets against the same owner).
pub fn lockset_vocabulary(root: &PyStr) -> Result<LocksetVocabulary, String> {
    lockset_vocab(root).map(|v| (v.provenance, v.knobs)).map_err(|d| d.text.to_utf8().unwrap_or_default())
}

impl Vocab {
    fn knob(&self, key: &str) -> Vec<String> {
        self.knobs.get(key).cloned().unwrap_or_default()
    }

    /// `_stamp(key, value)`: the value cross-checked against the owner's vocabulary (`None` stays `None`).
    fn stamp(&self, key: &str, value: Option<&str>) -> Step<PyValue> {
        let Some(v) = value else { return Ok(PyValue::None) };
        let allowed = if key == "provenance" { self.provenance.clone() } else { self.knob(key) };
        if !allowed.iter().any(|a| a == v) {
            return Err(die(5, &format!("lockset 각인 값이 어휘 밖이다: {}='{}' (허용 {:?} · 소유 {}) — explorer 내부 결함이다.", key, v, allowed, VOCAB_REL)));
        }
        Ok(PyValue::str(v))
    }

    /// `_input_source(key, value)`: `(value in vocabulary or None, anomaly sentence)`.
    fn input_source(&self, key: &str, value: &PyValue) -> (Option<String>, Option<String>) {
        if is_none(value) {
            return (None, None);
        }
        let allowed = self.knob(key);
        if let Some(v) = str_of(value) {
            if allowed.iter().any(|a| *a == v) {
                return (Some(v), None);
            }
        }
        (
            None,
            Some(format!("입력 lockset {}={} 는 어휘 밖이다(허용 {:?} · 소유 {}) — 출처 미상의 사람 값으로 본다", key, value.repr(), allowed, VOCAB_REL)),
        )
    }
}

// ── candidate · declared axes · batch plan ──────────────────────────────────────────────────────────────────────────

fn load_candidate(path: &PyStr) -> Step<PyDict> {
    if !is_file(path) {
        return Err(die_py(1, Message::new().lit("--candidate 파일 없음: ").text(path).done()));
    }
    let value = load_json_file(path).map_err(|f| {
        let name = path.to_utf8().unwrap_or_default();
        let hint = if name.ends_with(".yaml") || name.ends_with(".yml") {
            " — YAML 로 보인다. --candidate 는 **JSON** 만 받는다(simlog 의 trial*_candidate.yaml 은 사람이 읽으라고 만든 *산출물*이지 입력 형식이 아니다)."
        } else {
            ""
        };
        die_py(1, Message::new().lit("--candidate 파싱 실패: ").text(path).lit(" (").text(&f.message).lit(")").lit(hint).done())
    })?;
    let PyValue::Dict(mut cand) = value.peel().clone() else {
        return Err(die(1, "--candidate JSON 은 lock-set dict 여야 함"));
    };
    if !cand.contains_key("id") {
        cand.set("id", PyValue::str("s1"));
    }
    Ok(cand)
}

/// `_declared_axis(cfg, key)` strict: `(value, state, source sentence)`.
fn declared_axis(cfg: &PyValue, key: &str) -> Step<(Option<i64>, &'static str, String)> {
    let axes = g(cfg, "declared_axes");
    if is_none(&axes) {
        return Ok((None, "absent", "config.declared_axes 블록 없음".into()));
    }
    let PyValue::Dict(d) = axes.peel() else {
        return Err(die(5, "config.declared_axes 는 매핑이어야 한다(비매핑)"));
    };
    let Some(val) = d.get(key) else {
        return Ok((None, "absent", format!("config.declared_axes.{} 키 없음", key)));
    };
    if is_none(val) {
        return Ok((None, "null", format!("config.declared_axes.{}=null(선언하지 않았다)", key)));
    }
    match val.peel() {
        PyValue::Int(n) if *n >= num_bigint::BigInt::from(1) => {
            Ok((i64::try_from(n.clone()).ok(), "declared", format!("config.declared_axes.{}", key)))
        }
        _ => Err(die_py(
            5,
            Message::new()
                .lit(&format!("config.declared_axes.{}={}", key, val.repr()))
                .lit(" 는 양의 정수 또는 null 이어야 한다 — 모르면 null 로 '선언하지 않았다'를 적는다(campaigns/README.md §셀 값의 두 계층).")
                .done(),
        )),
    }
}

pub struct BatchPlan {
    derive: bool,
    hand_batch: Option<i64>,
    requirement: Option<i64>,
    requirement_state: &'static str,
    requirement_source: String,
    typical: Option<i64>,
    typical_state: &'static str,
    typical_source: String,
    prior_batch: Option<PyValue>,
    prior_kv: Option<PyValue>,
    anomalies: Vec<String>,
}

fn batch_plan(cand: &mut PyDict, cfg: &PyValue, vocab: &Vocab) -> Step<BatchPlan> {
    for (key, derived) in [("batch_source", &BATCH_DERIVED[..]), ("kv_source", &KV_DERIVED[..])] {
        let knob = vocab.knob(key);
        if !derived.iter().all(|d| knob.iter().any(|k| k == d)) {
            return Err(die(5, &format!("explorer 파생 분류 {}={:?} 가 소유자 어휘 {:?} 밖이다 — 검증기 어휘가 바뀌었는데 분류가 따라가지 않았다({}).", key, derived, knob, VOCAB_REL)));
        }
    }
    let lock_batch = cg(cand, "batch");
    if !is_none(&lock_batch) {
        let bad = match lock_batch.peel() {
            PyValue::Bool(_) => true,
            PyValue::Int(n) => *n < num_bigint::BigInt::from(1),
            PyValue::Float(f) => *f < 1.0 || f.trunc() != *f,
            _ => true,
        };
        if bad {
            return Err(die_py(5, Message::new().lit(&format!("lockset batch={}", lock_batch.repr())).lit(" 는 양의 정수 또는 null 이어야 한다 — 비워 두면(null) explorer 가 min(concurrency_requirement, KV_fit@L) 로 산출하고, 사람이 적은 수는 손레버로 보존된다(references/kv-clamp.md §3).").done()));
        }
    }
    let (lock_src, src_anomaly) = vocab.input_source("batch_source", &cg(cand, "batch_source"));
    let lock_kv = cg(cand, "kv_cache_memory_bytes");
    let (kv_src, kv_anomaly) = vocab.input_source("kv_source", &cg(cand, "kv_source"));
    let (req, req_state, req_src) = declared_axis(cfg, "concurrency_requirement")?;
    let (typ, typ_state, typ_src) = declared_axis(cfg, "typical_request_tokens")?;
    let max_len = cg(cand, "max_model_len");
    if let (Some(t), false) = (typ, is_none(&max_len)) {
        let m = to_i64(&max_len).unwrap_or(0);
        if t > m {
            return Err(die_py(5, Message::new().lit(&format!("config.declared_axes.typical_request_tokens={} > max_model_len=", t)).val(&max_len).lit(" — 요청 한 건은 max_model_len 을 넘을 수 없다(선언 결함).").done()));
        }
    }
    let mut plan = BatchPlan {
        derive: true,
        hand_batch: None,
        requirement: req,
        requirement_state: req_state,
        requirement_source: req_src,
        typical: typ,
        typical_state: typ_state,
        typical_source: typ_src,
        prior_batch: None,
        prior_kv: None,
        anomalies: [src_anomaly, kv_anomaly].into_iter().flatten().collect(),
    };
    if !is_none(&lock_batch) && !lock_src.as_deref().map_or(false, |s| BATCH_DERIVED.contains(&s)) {
        plan.derive = false;
        plan.hand_batch = to_i64(&lock_batch);
    } else if !is_none(&lock_batch) {
        plan.prior_batch = Some(lock_batch);
        cand.set("batch", PyValue::None);
    }
    if !is_none(&lock_kv) && kv_src.as_deref().map_or(false, |s| KV_DERIVED.contains(&s)) {
        plan.prior_kv = Some(lock_kv);
        cand.set("kv_cache_memory_bytes", PyValue::None);
    }
    Ok(plan)
}

fn kv_fit_length(plan: &BatchPlan, cand: &PyDict) -> Option<i64> {
    if !plan.derive || is_none(&cg(cand, "batch")) {
        return None;
    }
    plan.typical.or_else(|| to_i64(&cg(cand, "max_model_len")))
}

fn math_plan(plan: &BatchPlan) -> Plan {
    Plan {
        derive_mode: plan.derive,
        requirement: plan.requirement,
        requirement_state: plan.requirement_state.to_string(),
        requirement_source: plan.requirement_source.clone(),
        typical: plan.typical,
        typical_source: plan.typical_source.clone(),
        prior_batch: plan.prior_batch.as_ref().and_then(to_i64),
    }
}

/// `derive_batch(...)` as the original's dict (key order is the original's).
fn derivation_dict(plan: &BatchPlan, d: &sm::Derivation) -> PyDict {
    let mut r = PyDict::new();
    r.set("formula", PyValue::str("max_num_seqs = min(concurrency_requirement, KV_fit@L) · KV_fit@L = floor(kv_fit_tokens ÷ L)"));
    r.set("mode", PyValue::str(if plan.derive { "derive" } else { "hand-lever" }));
    r.set("applied", PyValue::Bool(d.applied));
    r.set("concurrency_requirement", oi(plan.requirement));
    r.set("concurrency_requirement_state", PyValue::str(plan.requirement_state));
    r.set("concurrency_requirement_source", PyValue::str(&plan.requirement_source));
    r.set("kv_fit_length", oi(d.kv_fit_length));
    r.set("kv_fit_length_source", PyValue::str(&d.kv_fit_length_source));
    r.set("kv_fit_tokens", oi(d.kv_fit_tokens));
    r.set("kv_fit_tokens_source", PyValue::str(&d.kv_fit_tokens_source));
    r.set("kv_fit", oi(d.kv_fit));
    r.set("batch", oi(d.batch));
    r.set("batch_source", os(d.batch_source));
    r.set("verified", d.verified.map_or(PyValue::None, PyValue::Bool));
    r.set("reason", PyValue::str(&d.reason));
    r.set("prior_batch", plan.prior_batch.clone().unwrap_or(PyValue::None));
    r
}

// ── profile helpers ─────────────────────────────────────────────────────────────────────────────────────────────────

fn profile_of(d: &PyDict) -> Profile {
    let f = |k: &str| d.get(k).and_then(to_f64);
    Profile {
        weights_gib: f("weights_gib"),
        kv_cache_gib: f("kv_cache_gib"),
        kv_cache_tokens: d.get("kv_cache_tokens").and_then(to_f64).map(|v| v as i64),
        non_kv_overhead_gib: f("non_kv_overhead_gib"),
        gmu_trial: f("gmu_trial"),
        device_total_gib: f("device_total_gib"),
        gmu_used: f("gmu_used"),
        gmu_source: None,
    }
}

/// `_enrich_overhead` in place on the dict (the added keys keep the original's order).
fn enrich_in_place(profile: &mut PyDict, device_total: f64, gmu_fallback: Option<f64>, explicit: bool) {
    let mut p = profile_of(profile);
    let had = p.non_kv_overhead_gib.is_some();
    sm::enrich_overhead(&mut p, device_total, gmu_fallback, explicit);
    if !had {
        if let Some(o) = p.non_kv_overhead_gib {
            profile.set("non_kv_overhead_gib", fl(o));
            profile.set("device_total_gib", fl(device_total));
            profile.set("gmu_used", fl(p.gmu_used.unwrap_or(0.0)));
            profile.set("gmu_source", PyValue::str(p.gmu_source.as_deref().unwrap_or("")));
        }
    }
}

fn is_unified_memory(man: &PyValue, root: &PyStr, device_total: f64) -> Option<bool> {
    let (mm, _) = memory_model(man, root);
    if let Some(m) = mm {
        return Some(m == "unified");
    }
    if device_total == 0.0 {
        return None;
    }
    let meminfo = std::fs::read_to_string("/proc/meminfo").ok()?;
    let line = meminfo.lines().find(|l| l.starts_with("MemTotal:"))?;
    let kib: i64 = line.split_whitespace().nth(1)?.parse().ok()?;
    let total = kib as f64 / (1024.0 * 1024.0);
    if total <= 0.0 {
        return None;
    }
    Some((device_total - total).abs() / total <= 0.05)
}

fn is_campaign_cell_lockset(root: &PyStr, path: &PyStr) -> bool {
    let (Some(p), Some(r)) = (path.to_utf8(), root.to_utf8()) else { return false };
    let real = |x: &str| std::fs::canonicalize(x).unwrap_or_else(|_| std::path::PathBuf::from(x));
    let (rp, rr) = (real(&p), real(&r));
    let Ok(rel) = rp.strip_prefix(&rr) else { return false };
    let parts: Vec<String> = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    parts.len() == 5 && parts[0] == "campaigns" && parts[1] != "_template" && parts[2] == "cells" && parts[4] == "lockset.json"
}

fn lite_charges(root: &PyStr, cand_path: &PyStr) -> Step<(i64, Option<String>, Option<String>)> {
    if !is_cell_lockset(root, cand_path) {
        return Ok((0, None, None));
    }
    let dir = dirname(&PyStr::new(&std::fs::canonicalize(cand_path.to_utf8().unwrap_or_default()).map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()));
    let st_path = join(&dir, &s("cell.status.json"));
    let rel = st_path.to_utf8().unwrap_or_default();
    let rel = rel.strip_prefix(&format!("{}/", root.to_utf8().unwrap_or_default())).unwrap_or(&rel).to_string();
    if !is_file(&st_path) {
        return Ok((0, None, Some(format!("{} 부재(셀 상태 미기록 — 차감 0)", rel))));
    }
    let st = load_json_file(&st_path).map_err(|f| die_py(5, Message::new().lit(&format!("셀 상태 {} 를 읽을 수 없다(", rel)).text(&f.message).lit(") — lite ② 차감을 모르면 cap 을 정할 수 없다(차감 0 으로 접지 않는다)").done()))?;
    let rec = dict_of(&g(&st, "reconciliation"));
    let spent = match rec.get("charges").map(|c| c.peel().clone()) {
        Some(PyValue::List(items)) => items.iter().filter(|i| matches!(i.peel(), PyValue::Dict(_))).count() as i64,
        _ => 0,
    };
    let reentry = dict_of(&g(&st, "reentry"));
    Ok((spent, reentry.get("status").and_then(str_of), Some(format!("{} reconciliation.charges", rel))))
}

fn is_cell_lockset(root: &PyStr, p: &PyStr) -> bool {
    is_campaign_cell_lockset(root, p)
}

// ── RAM gate host ───────────────────────────────────────────────────────────────────────────────────────────────────

struct GateHost<'a> {
    system: System,
    streams: &'a mut Streams,
}

impl Provider for GateHost<'_> {
    fn mem_available_mib(&mut self) -> Res<PyValue> {
        self.system.mem_available_mib()
    }
    fn try_drop_caches(&mut self) -> Res<bool> {
        self.system.try_drop_caches()
    }
}

impl memory::Host for GateHost<'_> {
    fn write_stderr(&mut self, text: &PyStr) -> Res<()> {
        self.streams.stderr.push_py(text);
        Ok(())
    }
    fn call_log(&mut self, _message: &PyStr) -> Res<()> {
        Ok(())
    }
}

// ── run_trial (mock / dry-run in process; the real docker session through run_trial_session) ──────────────────────────

struct TrialOpts {
    image: PyStr,
    timeout: f64,
    dry_run: bool,
    mock_profile: Option<PyStr>,
    served_model_name: Option<PyStr>,
    port: Option<i64>,
    nas_mount: PyStr,
    nas_container_root: PyStr,
    tiktoken_host_path: Option<PyStr>,
    jit_cache_root: PyStr,
    max_jobs: Option<i64>,
    manifest: PyStr,
    tp: i64,
    checkpoint_bytes: Option<i64>,
    unified_memory: Option<bool>,
    overhead_mib: Option<i64>,
}

fn run_trial(cand: &PyDict, run_dir: &PyStr, n: i64, o: &TrialOpts, root: &PyStr) -> R<PyDict> {
    let base = run_dir.to_utf8().unwrap_or_default();
    let log_path = if base.ends_with('/') { format!("{}trial{:02}_vllm.log", base, n) } else { format!("{}/trial{:02}_vllm.log", base, n) };
    if o.dry_run || o.mock_profile.is_some() {
        let (mut profile, mut functional) = (PyValue::None, PyValue::None);
        if let Some(path) = &o.mock_profile {
            let blob = raise(load_json_file(path))?;
            let wrapped = matches!(blob.peel(), PyValue::Dict(d) if d.contains_key("vllm_profile") || d.contains_key("functional"));
            if wrapped {
                profile = g(&blob, "vllm_profile");
                functional = g(&blob, "functional");
            } else {
                profile = blob;
            }
        }
        let prov = if o.mock_profile.is_some() { PROVENANCE_MOCK } else { PROVENANCE_DRY_RUN };
        let r = raise(mock_result(&dv(cand), n, profile, functional, &PyStr::new(&log_path), prov))?;
        return Ok(dict_of(&r));
    }
    let opts = SessionOpts {
        image: Some(o.image.clone()),
        timeout: Some(o.timeout),
        port: o.port,
        served_model_name: o.served_model_name.clone(),
        nas_mount: Some(o.nas_mount.clone()),
        nas_container_root: Some(o.nas_container_root.clone()),
        tiktoken_host_path: o.tiktoken_host_path.clone(),
        jit_cache_root: Some(o.jit_cache_root.clone()),
        max_jobs: o.max_jobs,
        manifest: Some(o.manifest.clone()),
        tp: Some(o.tp),
        checkpoint_bytes: o.checkpoint_bytes,
        unified_memory: o.unified_memory,
        overhead_mib: o.overhead_mib,
        ..SessionOpts::default()
    };
    let _ = root;
    let stop = |e: SessionError| match e {
        SessionError::Stop { status, text } => Stop::Raw(status, text),
        SessionError::Failure(f) => Stop::Die(raised(f)),
    };
    pre_session_gates(&crate::run_trial_session::repo_root(), &opts).map_err(stop)?;
    let r = run_session(&dv(cand), run_dir, n, &opts).map_err(stop)?;
    Ok(dict_of(&r))
}

// ── the command ─────────────────────────────────────────────────────────────────────────────────────────────────────

pub struct Args {
    pub config: PyStr,
    pub candidate: PyStr,
    pub dry_run: bool,
    pub mock_profile: Option<PyStr>,
    pub run_id: Option<PyStr>,
    pub topic: Option<PyStr>,
    pub cap: i64,
    pub image: PyStr,
    pub timeout: i64,
    pub overhead_mib: Option<i64>,
    pub force: bool,
    pub lockset_out: Option<PyStr>,
}

fn gmu_roles_record(roles: &Roles, trial: Option<&PyDict>, cand: &PyDict) -> PyDict {
    let mut r = PyDict::new();
    r.set("flow", PyValue::str(roles.flow));
    r.set("deploy_gmu", fl(roles.deploy_gmu));
    r.set("deploy_gmu_source", PyValue::str(&roles.deploy_gmu_source));
    r.set("deploy_gmu_declared", fl(roles.deploy_gmu_declared));
    r.set("deploy_gmu_role", PyValue::str("배포 yaml gpu-memory-utilization · 클램프 산식 per_card_vram × deploy_gmu − weights/TP − overhead/TP"));
    r.set("gate_margin", fl(roles.gate_margin));
    r.set("gate_margin_source", PyValue::str(&roles.gate_margin_source));
    r.set("gate_margin_role", PyValue::str("예산 검증 게이트 승수(sim_classify 천장 = budget × gate_margin)"));
    r.set("budget_gib", fl(roles.budget_gib));
    r.set("budget_source", PyValue::str(&roles.budget_source));
    r.set("gpu_model", PyValue::str(&roles.gpu_model));
    r.set("tp_divisor", PyValue::int(roles.tp_divisor));
    if let Some(t) = trial {
        let eff = cg(t, "effective_gmu");
        if !is_none(&eff) {
            r.set("trial_gmu", eff);
            r.set("trial_gmu_source", PyValue::str("run_trial.effective_gmu(실제 emit 값)"));
        } else {
            r.set("trial_gmu", cg(cand, "gpu_memory_utilization"));
            r.set("trial_gmu_source", PyValue::str("candidate.gpu_memory_utilization(트라이얼 요청값)"));
        }
    }
    r
}

fn now_iso() -> String {
    Clock::Local.now_iso().to_utf8().unwrap_or_default()
}

fn write_json_file(path: &PyStr, value: &PyValue) -> R<()> {
    let mut text = raise(value.dumps_pretty().map_err(|e| Failure::new(e.python_type(), "json dump 실패")))?;
    text.push_str("\n");
    if let Some(parent) = path.to_utf8().and_then(|p| std::path::Path::new(&p).parent().map(|x| x.to_path_buf())) {
        let _ = std::fs::create_dir_all(parent);
    }
    raise(crate::recipe_set::write_text(path, &text, false))
}

/// `cmd_simulate(args)` — the exit status (0 converged · 3 Model-C), or what stops it.
pub fn cmd_simulate(streams: &mut Streams, a: &Args, root: &PyStr, skill_root: &PyStr) -> R<i32> {
    let cfg = load_config(&a.config)?;
    let target = g(&cfg, "target_model");
    let target = if target.is_truthy() { target } else { empty_dict() };
    let model_path = g(&target, "path");
    if !model_path.is_truthy() {
        return Err(die(1, "config.target_model.path 누락").into());
    }
    let (man, mpath) = read_manifest(root)?;
    let nas_root = {
        let c = g(&cfg, "nas_host_root");
        if c.is_truthy() {
            c
        } else {
            match std::env::var("NAS_MODEL_PATH") {
                Ok(v) if !v.is_empty() => PyValue::str(&v),
                _ => {
                    let m = g(&man, "nas_model_path");
                    if m.is_truthy() { m } else { PyValue::str(DEFAULT_NAS_HOST_ROOT) }
                }
            }
        }
    };
    if is_none(&g(&cfg, "vram_budget_gb")) {
        return Err(die(1, "config.vram_budget_gb 누락").into());
    }
    let container_root = raise(get_or(&cfg, "nas_container_root", PyValue::str("/app/models")))?;
    let tp = resolve_tp(&cfg, root)?;
    guard_tp(tp, root)?;
    let vocab = lockset_vocab(root)?;
    let mut lockset_out = a.lockset_out.clone();
    if lockset_out.is_none() && is_campaign_cell_lockset(root, &a.candidate) {
        lockset_out = Some(a.candidate.clone());
        streams.eprint(&Message::new().lit("[recipe] ⓘ --candidate 가 캠페인 셀 lockset 이다 — 수렴 각인을 그 파일에 쓴다(--lockset-out 기본값): ").text(&a.candidate).done());
    }
    if let Some(out) = &lockset_out {
        let real = |p: &PyStr| std::fs::canonicalize(p.to_utf8().unwrap_or_default()).ok();
        if std::path::Path::new(&out.to_utf8().unwrap_or_default()).exists() && !a.force && real(out) != real(&a.candidate) {
            return Err(die_py(1, Message::new().lit("--lockset-out 대상이 이미 있다: ").text(out).lit(" — --candidate 와 같은 파일(셀 lockset materialize)이 아니면 --force 로만 덮어쓴다.").done()).into());
        }
    }

    let roles = gpu_roles(streams, &cfg, tp, root)?;
    let budget = roles.budget_gib;
    let gate_margin = roles.gate_margin;
    let deploy_gmu = roles.deploy_gmu;
    let tp_divisor = roles.tp_divisor;
    streams.eprint(&PyStr::new(&format!(
        "[recipe] gmu 역할({} 흐름): deploy_gmu={:.3} ({}) · gate_margin={:.3} ({}) · budget=per_card_vram {:.2} GiB ({}) · gpu_model={} · TP={}(÷{}) · lockset gmu_source={}",
        roles.flow, deploy_gmu, roles.deploy_gmu_source, gate_margin, roles.gate_margin_source, budget, roles.budget_source, roles.gpu_model, tp, tp_divisor, roles.gmu_source
    )));
    if roles.gmu_source != "target_gmu" {
        streams.eprint(&PyStr::new(&format!("[recipe] ⚠ 배포 gmu 가 정본 칸(target_gpu.target_gmu)에서 오지 않았다 — {}. lockset gmu_source={} 로 각인한다.", roles.deploy_gmu_source, roles.gmu_source)));
    }
    if deploy_gmu > gate_margin + 1e-9 {
        streams.eprint(&PyStr::new(&format!("[recipe] ⓘ deploy_gmu {:.3} > gate_margin {:.3} — 클램프 산식 천장(budget×deploy_gmu)이 검증 게이트 천장(budget×gate_margin)보다 높다. 게이트는 weights+overhead 만 본다(기재 · 차단 ✗).", deploy_gmu, gate_margin)));
    }
    let (dtp, dtp_state, dtp_src) = declared_axis_tp(&cfg);
    let mut tpc = PyDict::new();
    tpc.set("declared", oi(dtp));
    tpc.set("declared_state", PyValue::str(dtp_state));
    tpc.set("declared_source", PyValue::str(&dtp_src));
    tpc.set("resolved", PyValue::int(tp));
    tpc.set("resolved_source", PyValue::str("resolve_tp(config override > manifest)"));
    tpc.set("match", dtp.map_or(PyValue::None, |d| PyValue::Bool(d == tp)));
    let mut declared_axes_check = PyDict::new();
    declared_axes_check.set("tp", dv(&tpc));
    match dtp {
        Some(d) if d != tp => streams.eprint(&PyStr::new(&format!("[recipe] ⚠ declared_axes.tp={} ≠ 해소 TP={} — TP 의 권위는 manifest 다. 선언을 고쳐라(기재 · 차단 ✗).", d, tp))),
        None if dtp_state == "malformed" => streams.eprint(&PyStr::new(&format!("[recipe] ⚠ {} — 해소 TP={} 와 대조하지 못했다(기재 · 차단 ✗).", dtp_src, tp))),
        _ => {}
    }

    let parsed = match model_config::parse(&model_path, &nas_root, &container_root) {
        Ok(p) => PyValue::Dict(p),
        Err(f) if f.class == "FileNotFoundError" => return Err(die_py(1, f.message).into()),
        Err(f) if f.is_value_error() => return Err(die_py(1, Message::new().lit("모델 config 해소 실패: ").text(&f.message).done()).into()),
        Err(f) => return Err(raised(f).into()),
    };

    let mut cand = load_candidate(&a.candidate)?;
    let raw_lockset = cand.clone();
    let quant = cg(&cand, "kv_cache_quant");
    let kv_dtype_bytes = sm::kv_dtype_bytes(str_of(&quant).as_deref());
    let plan = batch_plan(&mut cand, &cfg, &vocab)?;
    let mut kv_origin: Option<&str> = if !is_none(&cg(&cand, "kv_cache_memory_bytes")) { Some("input") } else { None };
    {
        let tail1 = if !plan.derive {
            format!(" · 손레버 batch={} 유지(덮어쓰기 ✗ · KV-fit 은 대조 기재)", plan.hand_batch.unwrap_or(0))
        } else if let Some(pb) = &plan.prior_batch {
            format!(" · 직전 산출 batch={} 는 승계하지 않는다", disp(pb))
        } else {
            String::new()
        };
        let tail2 = plan.prior_kv.as_ref().map_or(String::new(), |k| format!(" · 직전 산출 KV 클램프={} 는 승계하지 않는다(trial1 을 언클램프 실측으로 되돌린다)", disp(k)));
        let showi = |v: Option<i64>| v.map_or("None".to_string(), |x| x.to_string());
        streams.eprint(&PyStr::new(&format!(
            "[recipe] max-num-seqs 계획: mode={} · 요구={}({}) · 대표 길이={}({}){}{}",
            if plan.derive { "derive" } else { "hand-lever" }, showi(plan.requirement), plan.requirement_state, showi(plan.typical), plan.typical_state, tail1, tail2
        )));
    }
    for an in &plan.anomalies {
        streams.eprint(&PyStr::new(&format!("[recipe] ⚠ {}(기재 · 차단 ✗)", an)));
    }

    // trial inputs
    for (k, v) in [("model_path_container", g(&parsed, "model_path_container")), ("model_id", g(&parsed, "model_id"))] {
        if !cand.contains_key(k) {
            cand.set(k, v);
        }
    }
    let serving0 = { let v = g(&cfg, "serving"); if v.is_truthy() { v } else { empty_dict() } };
    if g(&serving0, "served_model_name").is_truthy() && !cand.contains_key("served_model_name") {
        cand.set("served_model_name", g(&serving0, "served_model_name"));
    }
    if !cand.contains_key("gpu_memory_utilization") {
        cand.set("gpu_memory_utilization", fl(deploy_gmu));
    }
    let (device_total, device_total_src) = resolve_device_total_gib(&cfg, root)?;
    streams.eprint(&PyStr::new(&format!("[recipe] device_total_gib={:.2} ({})", device_total, device_total_src)));

    let run_id: PyStr = if let Some(r) = &a.run_id {
        r.clone()
    } else {
        let topic = a.topic.clone().unwrap_or_else(|| {
            let mid = g(&parsed, "model_id");
            let t = if mid.is_truthy() { disp(&mid) } else { "sim".into() };
            PyStr::new(&t.replace('/', "_"))
        });
        let iso = now_iso();
        let stamp: String = format!("{}{}{}{}", &iso[0..4], &iso[5..7], &iso[8..10], &iso[11..13]);
        let mut r = PyStr::new(&format!("{}_1_", stamp));
        r.push_py(&topic);
        r
    };
    let run_dir_v = sl(simlog::new_run_dir(&mut Plain, &PyValue::Str(root.clone()), &PyValue::Str(run_id.clone())))?;
    let run_dir = match run_dir_v.peel() { PyValue::Str(p) => p.clone(), _ => PyStr::default() };
    streams.eprint(&Message::new().lit("[recipe] simlog run_dir: ").text(&run_dir).done());

    let serving = serving0.clone();
    let overhead_cli = a.overhead_mib.filter(|v| *v != 0);
    let overhead_cfg = g(&cfg, "budget_overhead_mib");
    let topo = topology_of(root);
    let opts = TrialOpts {
        image: a.image.clone(),
        timeout: a.timeout as f64,
        dry_run: a.dry_run,
        mock_profile: a.mock_profile.clone(),
        served_model_name: str_of(&g(&serving, "served_model_name")).map(|t| PyStr::new(&t)),
        port: { let p = g(&serving, "port"); if is_none(&p) { None } else { to_i64(&p) } },
        nas_mount: match nas_root.peel() { PyValue::Str(p) => p.clone(), _ => PyStr::default() },
        nas_container_root: match container_root.peel() { PyValue::Str(p) => p.clone(), _ => PyStr::new("/app/models") },
        tiktoken_host_path: str_of(&g(&cfg, "tiktoken_host_path")).map(|t| PyStr::new(&t)),
        jit_cache_root: { let c = g(&cfg, "jit_cache_root"); match (c.is_truthy(), str_of(&c)) { (true, Some(t)) => PyStr::new(&t), _ => join(&join(&join(root, &s("output")), &s(topo)), &s("cache")) } },
        max_jobs: Some(to_i64(&raise(get_or(&cfg, "max_jobs", PyValue::int(4)))?).unwrap_or(4)),
        manifest: mpath.clone(),
        tp,
        checkpoint_bytes: to_i64(&g(&parsed, "native_weight_bytes")),
        unified_memory: is_unified_memory(&man, root, device_total),
        overhead_mib: overhead_cli.or_else(|| if overhead_cfg.is_truthy() { to_i64(&overhead_cfg) } else { None }),
    };

    let cap_declared = a.cap;
    let (lite_spent, reentry_status, spent_src) = lite_charges(root, &a.candidate)?;
    let cap = cap_declared - lite_spent;
    if lite_spent != 0 {
        streams.eprint(&PyStr::new(&format!("[recipe] ⓘ lite ② 차감 {}건({}) → cap {} − {} = {}", lite_spent, spent_src.clone().unwrap_or_default(), cap_declared, lite_spent, cap)));
        if reentry_status.as_deref() != Some("approved") {
            streams.eprint(&PyStr::new(&format!("[recipe] ⚠ 재발동 결정 기록이 없다(reentry.status={}) — 사람 승인을 받았으면 `campaign_init.py --reentry-decide <cell> --decision approve --route explorer --approved-by … --utc …` 로 남겨라", reentry_status.as_deref().map_or("None".to_string(), |s| format!("'{}'", s)))));
        }
    }
    if cap <= 0 {
        streams.eprint(&PyStr::new(&format!("[recipe] ── Model-C (HITL) — reconciliation_cap 소진(선언 {} · lite ② 차감 {}) · 트라이얼 없이 멈춘다 ──", cap_declared, lite_spent)));
        streams.eprint(&PyStr::new(&format!("[recipe]   이 셀은 서빙은 됐으나 lite ② 실사용 불가가 cap 만큼 반복됐다 — 서빙전략 재탐색이 아니라 사람 판단(upstream 재빌드 · 음성정직 · 셀 폐기)이 다음이다. 출처: {}", spent_src.unwrap_or_default())));
        return Ok(3);
    }

    let mut history: Vec<PyValue> = Vec::new();
    let mut converged = false;
    let mut final_trial: Option<PyDict> = None;
    let mut final_class: Option<PyDict> = None;
    let mut batch_decided = !plan.derive;
    let mut clamp_provisional = false;
    let mut derivation: Option<PyDict> = None;
    let mut engine_obs: Vec<PyDict> = Vec::new();
    let mplan = math_plan(&plan);

    let clamp_of = |prof: &PyDict, cand: &PyDict| -> sm::Clamp {
        let max_len = to_i64(&cg(cand, "max_model_len")).unwrap_or(0);
        let batch = to_i64(&cg(cand, "batch"));
        let formula = |tokens: i64| -> i64 {
            required_kv_bytes(&parsed, &PyValue::int(tokens), &PyValue::int(1), &PyValue::int(kv_dtype_bytes))
                .ok().and_then(|b| i64::try_from(b).ok()).unwrap_or(i64::MAX)
        };
        sm::resolve_clamp_kv(&profile_of(prof), max_len, batch, budget, deploy_gmu, tp_divisor, kv_fit_length(&plan, cand), &formula)
    };
    let fail_dict = |class: &str, adjust: Option<&str>, note: &str| -> PyDict {
        let mut d = PyDict::new();
        d.set("failure_class", PyValue::str(class));
        d.set("adjust_target", os(adjust));
        d.set("note", PyValue::str(note));
        d
    };
    let append_record = |rd: &PyDict, history: &mut Vec<PyValue>| -> R<()> {
        history.push(dv(rd));
        sl(simlog::append_correction(&mut Plain, &PyValue::Str(run_dir.clone()), &dv(rd)))?;
        Ok(())
    };

    for trial_number in 1..=cap {
        streams.eprint(&Message::new().lit(&format!("[recipe] trial {}/{} — candidate id=", trial_number, cap)).val(&cg(&cand, "id")).lit(" max_len=").val(&cg(&cand, "max_model_len")).lit(" batch=").val(&cg(&cand, "batch")).lit(" kv_bytes=").val(&cg(&cand, "kv_cache_memory_bytes")).done());

        if !a.dry_run && a.mock_profile.is_none() {
            let ckpt = g(&parsed, "native_weight_bytes");
            let tpv = PyValue::int(tp);
            let floor = PyValue::int(memory::DEFAULT_FLOOR_MIB);
            let gate = {
                let mut host = GateHost { system: System::new(), streams };
                raise(memory::gate(&mut host, &GateArguments { checkpoint_bytes: &ckpt, tp: &tpv, floor_mib: &floor, auto_drop: &PyValue::Bool(true), log: LogChoice::Default }))?
            };
            if !gate.get("ok").map_or(false, |v| v.is_truthy()) {
                let gv = |k: &str| gate.get(k).cloned().unwrap_or(PyValue::None);
                let mut sum = PyDict::new();
                sum.set("run_id", PyValue::Str(run_id.clone()));
                sum.set("converged", PyValue::Bool(false));
                sum.set("failure_class", PyValue::str("preload_ram_gate_refused"));
                sum.set("note", PyValue::Str(Message::new().lit("MemAvailable=").val(&gv("avail_after_mib")).lit("MiB < required=").val(&gv("required_mib")).lit(&format!("MiB (ckpt÷tp={}+floor) @trial {} — 잔존 컨테이너/페이지캐시 정리 후 재시도", tp, trial_number)).done()));
                sum.set("trial_count", PyValue::int(history.len() as i64));
                sum.set("candidate", dv(&cand));
                sum.set("ram_gate", PyValue::Dict(gate.clone()));
                sum.set("correction_history", PyValue::List(history.clone()));
                sl(simlog::write_summary(&mut Plain, &PyValue::Str(run_dir.clone()), &PyValue::Dict(sum)))?;
                let mut summary_path = run_dir.clone();
                summary_path.push_str("/run_summary.json");
                return Err(die_py(7, Message::new().lit(&format!("로드-전 RAM 게이트 거부(trial {}): MemAvailable=", trial_number)).val(&gv("avail_after_mib")).lit("MiB < required=").val(&gv("required_mib")).lit("MiB (ckpt÷tp+floor) — 잔존 컨테이너/페이지캐시 정리 후 재시도. 헌법 호스트 안전체계 따름정리 · plan_26071019 §2.6 · run_summary=").text(&summary_path).done()).into());
            }
        }

        let mut trial = run_trial(&cand, &run_dir, trial_number, &opts, root)?;
        let prov = cg(&trial, "provenance");
        if !matches!(str_of(&prov).as_deref(), Some("measured" | "mock" | "dry-run")) {
            return Err(die_py(7, Message::new().lit(&format!("trial provenance 미표기/미지값({})", prov.repr())).lit(" — run_trial 계약 위반. 출처 미상 결과는 판정에 쓰지 않는다(plan_26081314 D1).").done()).into());
        }
        let mut prof = dict_of(&cg(&trial, "vllm_profile"));
        let has_profile = !is_none(&cg(&trial, "vllm_profile")) && matches!(cg(&trial, "vllm_profile").peel(), PyValue::Dict(_));
        {
            let eff = to_f64(&cg(&trial, "effective_gmu"));
            let fallback = eff.or_else(|| to_f64(&cg(&cand, "gpu_memory_utilization")));
            if has_profile {
                enrich_in_place(&mut prof, device_total, fallback, !is_none(&cg(&cand, "kv_cache_memory_bytes")));
                trial.set("vllm_profile", dv(&prof));
            }
        }
        let mut obs = PyDict::new();
        obs.set("trial_number", PyValue::int(trial_number));
        obs.set("clamped", PyValue::Bool(!is_none(&cg(&cand, "kv_cache_memory_bytes"))));
        obs.set("batch", cg(&cand, "batch"));
        obs.set("kv_cache_tokens", cg(&prof, "kv_cache_tokens"));
        obs.set("max_concurrency", cg(&prof, "max_concurrency"));
        engine_obs.push(obs);

        let log_text = {
            let lp = cg(&trial, "log_path");
            match str_of(&lp) {
                Some(p) if is_file(&PyStr::new(&p)) => read_bytes(&PyStr::new(&p)).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default(),
                _ => String::new(),
            }
        };
        sl(simlog::write_trial(&mut Plain, true, [&PyValue::Str(run_dir.clone()), &PyValue::int(trial_number), &PyValue::str(&log_text), &cg(&trial, "vllm_profile"), &dv(&cand), &cg(&trial, "functional")]))?;

        let typical_arg = if batch_decided { oi(kv_fit_length(&plan, &cand)) } else { PyValue::None };
        let verdict = raise(classifier::classify(&dv(&trial), &fl(budget), &fl(gate_margin), &typical_arg))?;
        final_trial = Some(trial.clone());
        final_class = Some(verdict.clone());
        let fclass = str_of(&cg(&verdict, "failure_class")).unwrap_or_default();
        let adjust = str_of(&cg(&verdict, "adjust_target"));
        streams.eprint(&Message::new().lit(&format!("[recipe] classify → {} (adjust=", fclass)).val(&cg(&verdict, "adjust_target")).lit(") :: ").val(&cg(&verdict, "note")).done());

        if fclass == "none" {
            let clamp_arg = if clamp_provisional { None } else { to_i64(&cg(&cand, "kv_cache_memory_bytes")).filter(|_| !is_none(&cg(&cand, "kv_cache_memory_bytes"))) };
            let (fit_tokens, fit_src) = sm::kv_fit_tokens(&profile_of(&prof), budget, deploy_gmu, tp_divisor, clamp_arg);
            let batch_before = cg(&cand, "batch");
            if !batch_decided {
                let d = sm::derive_batch(&mplan, to_i64(&cg(&cand, "max_model_len")), fit_tokens, &fit_src);
                derivation = Some(derivation_dict(&plan, &d));
                batch_decided = true;
                cand.set("batch", oi(d.batch));
                streams.eprint(&Message::new().lit("[recipe] max-num-seqs 산출 → batch=").val(&cg(&cand, "batch")).lit(&format!(" ({}) :: {}", d.batch_source.unwrap_or("None"), d.reason)).done());
                if clamp_provisional && is_none(&cg(&cand, "batch")) {
                    converged = true;
                    break;
                }
                if clamp_provisional {
                    let clamp_before = cg(&cand, "kv_cache_memory_bytes");
                    let res = clamp_of(&prof, &cand);
                    let mut rec = PyDict::new();
                    rec.set("trial_number", PyValue::int(trial_number));
                    rec.set("failure_class", PyValue::str("measure"));
                    rec.set("adjust_target", PyValue::str("kv_cache_memory_bytes"));
                    rec.set("note", PyValue::str("잠정 클램프(batch 산출 전 vram_oom 재산정분) 트라이얼 통과 → max-num-seqs 산출 → 산출 batch 로 클램프 재산정 후 재검증 트라이얼."));
                    let mut before = PyDict::new();
                    before.set("kv_cache_memory_bytes", clamp_before.clone());
                    before.set("batch", batch_before.clone());
                    rec.set("before", dv(&before));
                    if res.fail.is_some() || res.kv.is_none() {
                        let fc = match &res.fail {
                            Some((c, n)) => fail_dict(c, None, n),
                            None => fail_dict("unknown", None, "잠정 클램프 트라이얼 통과했으나 weights/overhead 실측 부재 → 클램프 재산정 불가(HITL)."),
                        };
                        rec.set("after_note", cg(&fc, "note"));
                        final_class = Some(fc);
                        append_record(&rec, &mut history)?;
                        break;
                    }
                    clamp_provisional = false;
                    kv_origin = Some("trial-loop");
                    let kv = res.kv.unwrap();
                    if Some(kv) == to_i64(&clamp_before) && cg(&cand, "batch") == batch_before {
                        converged = true;
                        break;
                    }
                    cand.set("kv_cache_memory_bytes", PyValue::int(kv));
                    let mut after = PyDict::new();
                    after.set("kv_cache_memory_bytes", PyValue::int(kv));
                    after.set("batch", cg(&cand, "batch"));
                    rec.set("after", dv(&after));
                    rec.set("after_note", PyValue::str(&res.note));
                    append_record(&rec, &mut history)?;
                    continue;
                }
                if !is_none(&cg(&cand, "kv_cache_memory_bytes")) {
                    if cg(&cand, "batch") != batch_before {
                        let mut rec = PyDict::new();
                        rec.set("trial_number", PyValue::int(trial_number));
                        rec.set("failure_class", PyValue::str("measure"));
                        rec.set("adjust_target", PyValue::str("batch"));
                        rec.set("note", PyValue::str("통과 트라이얼에서 max-num-seqs 산출 → batch 고정 재검증 트라이얼."));
                        let mut b = PyDict::new();
                        b.set("batch", batch_before.clone());
                        rec.set("before", dv(&b));
                        let mut af = PyDict::new();
                        af.set("batch", cg(&cand, "batch"));
                        rec.set("after", dv(&af));
                        rec.set("after_note", PyValue::str(&d.reason));
                        append_record(&rec, &mut history)?;
                        continue;
                    }
                    converged = true;
                    break;
                }
            } else if derivation.is_none() && !plan.derive {
                let d = sm::derive_batch(&mplan, to_i64(&cg(&cand, "max_model_len")), fit_tokens, &fit_src);
                let mut dd = derivation_dict(&plan, &d);
                let would = cg(&dd, "batch");
                let would_src = cg(&dd, "batch_source");
                let reason = cg(&dd, "reason");
                dd.set("derived_batch_would_be", would);
                dd.set("derived_batch_source_would_be", would_src);
                dd.set("batch", cg(&cand, "batch"));
                dd.set("batch_source", PyValue::str("hand-lever"));
                dd.set("applied", PyValue::Bool(false));
                dd.set("reason", PyValue::Str(Message::new().lit("손레버 batch=").val(&cg(&cand, "batch")).lit(" 유지(측정 산물 아님) · 산식 대조: ").val(&reason).done()));
                derivation = Some(dd);
            }
            if adjust.as_deref() == Some("batch") && !is_none(&cg(&verdict, "kv_fit")) && plan.derive {
                let kvf = to_i64(&cg(&verdict, "kv_fit")).unwrap_or(1);
                let new_batch = kvf.max(1);
                let mut rec = PyDict::new();
                rec.set("trial_number", PyValue::int(trial_number));
                rec.set("failure_class", PyValue::str(&fclass));
                rec.set("adjust_target", PyValue::str("batch"));
                rec.set("note", cg(&verdict, "note"));
                let mut b = PyDict::new();
                b.set("batch", batch_before.clone());
                rec.set("before", dv(&b));
                let mut af = PyDict::new();
                af.set("batch", PyValue::int(new_batch));
                rec.set("after", dv(&af));
                rec.set("after_note", Message::new().lit("KV-fit 재검증 → batch ").val(&batch_before).lit(&format!("→{} 하향 후 재검증 트라이얼.", new_batch)).done().into_value());
                cand.set("batch", PyValue::int(new_batch));
                let mut der = derivation.clone().unwrap_or_default();
                let mut adjustments = match der.get("phase2_adjustments").map(|v| v.peel().clone()) { Some(PyValue::List(l)) => l, _ => Vec::new() };
                let mut adj = PyDict::new();
                adj.set("trial_number", PyValue::int(trial_number));
                adj.set("batch_before", batch_before.clone());
                adj.set("batch_after", PyValue::int(new_batch));
                adj.set("kv_fit_check", cg(&verdict, "kv_fit_check"));
                adjustments.push(dv(&adj));
                der.set("phase2_adjustments", PyValue::List(adjustments));
                der.set("batch", PyValue::int(new_batch));
                der.set("batch_source", PyValue::str("kv-fit-measured"));
                der.set("kv_fit", PyValue::int(kvf));
                der.set("verified", PyValue::None);
                let prev_reason = derivation.as_ref().map_or(PyValue::None, |d| cg(d, "reason"));
                der.set("reason", Message::new().lit(&format!("클램프 트라이얼 {} 의 엔진 보고 KV 토큰으로 KV-fit {} 재측정 → batch ", trial_number, kvf)).val(&batch_before).lit(" 에서 낮춤(위상 1: ").val(&prev_reason).lit(")").done().into_value());
                derivation = Some(der);
                append_record(&rec, &mut history)?;
                if trial_number == cap {
                    let mut fc = verdict.clone();
                    fc.set("note", Message::new().lit(&format!("cap({}) 소진 — 위상 2 가 batch 를 ", cap)).val(&batch_before).lit(&format!("→{} 로 낮췄으나 재검증 트라이얼을 돌 칸이 없다(트라이얼 자체는 통과). --cap 을 늘려 다시 돈다 — 산식 경로는 측정·클램프 검증·위상 2 재검증까지 최소 3칸이 든다. 원 분류: ", new_batch)).val(&cg(&verdict, "note")).done().into_value());
                    final_class = Some(fc);
                }
                continue;
            }
            if is_none(&cg(&cand, "kv_cache_memory_bytes")) {
                let res = clamp_of(&prof, &cand);
                let mut rec = PyDict::new();
                rec.set("trial_number", PyValue::int(trial_number));
                rec.set("failure_class", PyValue::str("measure"));
                rec.set("adjust_target", PyValue::str("kv_cache_memory_bytes"));
                rec.set("note", PyValue::str("측정 트라이얼 통과 → (max-num-seqs 산출) → 절대 KV 클램프 산정 후 검증 트라이얼."));
                let mut b = PyDict::new();
                b.set("kv_cache_memory_bytes", PyValue::None);
                b.set("batch", batch_before.clone());
                rec.set("before", dv(&b));
                if let Some((c, n)) = &res.fail {
                    rec.set("after_note", PyValue::str(n));
                    final_class = Some(fail_dict(c, None, n));
                    append_record(&rec, &mut history)?;
                    break;
                }
                let Some(kv) = res.kv else {
                    let fc = fail_dict("unknown", None, "측정 트라이얼 통과했으나 weights/overhead 실측 부재 → 클램프 산정 불가(HITL).");
                    rec.set("after_note", cg(&fc, "note"));
                    final_class = Some(fc);
                    append_record(&rec, &mut history)?;
                    break;
                };
                cand.set("kv_cache_memory_bytes", PyValue::int(kv));
                kv_origin = Some("trial-loop");
                let mut af = PyDict::new();
                af.set("kv_cache_memory_bytes", PyValue::int(kv));
                af.set("batch", cg(&cand, "batch"));
                rec.set("after", dv(&af));
                rec.set("after_note", PyValue::str(&res.note));
                append_record(&rec, &mut history)?;
                continue;
            }
            converged = true;
            break;
        }

        if fclass == "vram_infeasible" || fclass == "unknown" {
            break;
        }

        let mut rec = PyDict::new();
        rec.set("trial_number", PyValue::int(trial_number));
        rec.set("failure_class", PyValue::str(&fclass));
        rec.set("adjust_target", os(adjust.as_deref()));
        rec.set("note", cg(&verdict, "note"));
        let mut before = PyDict::new();
        for k in ["kv_cache_memory_bytes", "attention_backend", "tool_call_parser", "reasoning_parser"] {
            before.set(k, cg(&cand, k));
        }
        rec.set("before", dv(&before));

        match adjust.as_deref() {
            Some("kv_cache_memory_bytes") => {
                let res = clamp_of(&prof, &cand);
                if let Some((c, n)) = &res.fail {
                    rec.set("after_note", PyValue::str(n));
                    final_class = Some(fail_dict(c, None, n));
                    append_record(&rec, &mut history)?;
                    break;
                }
                let new_kv: i64;
                match res.kv {
                    None => {
                        let cur = cg(&cand, "kv_cache_memory_bytes");
                        if is_none(&cur) {
                            let fc = fail_dict("unknown", None, "OOM 인데 vllm_profile 실측 부재 + 기존 KV 미설정 → Model-C(HITL).");
                            rec.set("after_note", cg(&fc, "note"));
                            final_class = Some(fc);
                            append_record(&rec, &mut history)?;
                            break;
                        }
                        new_kv = to_i64(&cur).unwrap_or(0).div_euclid(2);
                        rec.set("after_note", PyValue::str("실측 부재 → 직전 KV 절반 축소(보수적)."));
                    }
                    Some(kv) => {
                        new_kv = kv;
                        rec.set("after_note", PyValue::str(&res.note));
                    }
                }
                cand.set("kv_cache_memory_bytes", PyValue::int(new_kv));
                kv_origin = Some("trial-loop");
                clamp_provisional = !batch_decided;
                let mut af = PyDict::new();
                af.set("kv_cache_memory_bytes", PyValue::int(new_kv));
                rec.set("after", dv(&af));
                if clamp_provisional {
                    let prev = disp(&cg(&rec, "after_note"));
                    rec.set("after_note", PyValue::str(&format!("{} (잠정 클램프 — batch 산출 전이다. 통과 트라이얼에서 산출 batch 로 재산정한다)", prev)));
                }
            }
            Some(t @ ("attention_backend" | "tool_call_parser" | "reasoning_parser")) => {
                let cands = match cg(&cand, &format!("{}_candidates", t)).peel().clone() { PyValue::List(l) => l, _ => Vec::new() };
                let cur = cg(&cand, t);
                let nxt = if cands.is_empty() { None } else if let Some(i) = cands.iter().position(|c| *c == cur) { cands.get(i + 1).cloned() } else { cands.first().cloned() };
                let Some(nxt) = nxt else {
                    let fc = fail_dict("functional", Some(t), &format!("{} 폴백 후보 소진 → Model-C(HITL).", t));
                    rec.set("after_note", cg(&fc, "note"));
                    final_class = Some(fc);
                    append_record(&rec, &mut history)?;
                    break;
                };
                cand.set(t, nxt.clone());
                let mut af = PyDict::new();
                af.set(t, nxt.clone());
                rec.set("after", dv(&af));
                rec.set("after_note", Message::new().lit(&format!("{} → '", t)).val(&nxt).lit("' 폴백 후보로 교체.").done().into_value());
            }
            _ => {
                final_class = Some(fail_dict(&fclass, None, "조정 대상 미정 → Model-C(HITL)."));
                append_record(&rec, &mut history)?;
                break;
            }
        }
        append_record(&rec, &mut history)?;
    }

    if let Some(der) = derivation.as_mut() {
        let mut trials = Vec::new();
        for o in &engine_obs {
            let mut t = PyDict::new();
            t.set("trial_number", cg(o, "trial_number"));
            t.set("clamped", cg(o, "clamped"));
            t.set("batch", cg(o, "batch"));
            t.set("max_concurrency", cg(o, "max_concurrency"));
            t.set("conservative_floor", to_i64(&cg(o, "max_concurrency")).map_or(PyValue::None, PyValue::int));
            t.set("kv_cache_tokens", cg(o, "kv_cache_tokens"));
            trials.push(dv(&t));
        }
        let mut emc = PyDict::new();
        emc.set("note", PyValue::str("엔진 보고 max_concurrency 는 max_model_len 최악 길이 기준이다 — 보수 하한으로 기재(산식 입력 ✗)"));
        emc.set("trials", PyValue::List(trials));
        der.set("engine_max_concurrency", dv(&emc));
        let chk = final_class.as_ref().map_or(PyValue::None, |f| cg(f, "kv_fit_check"));
        der.set("verification", chk.clone());
        if plan.derive && !is_none(&cg(der, "batch")) {
            let chk_d = dict_of(&chk);
            let verified = chk.is_truthy() && cg(&chk_d, "checked").is_truthy() && !cg(&chk_d, "exceeds").is_truthy();
            der.set("verified", PyValue::Bool(verified));
            if !verified {
                let r = cg(&chk_d, "reason");
                der.set("verified_gap", if r.is_truthy() { r } else { PyValue::str("클램프 트라이얼 재검증 기록이 없다(수렴 전 종료 또는 위상 2 미도달)") });
            }
        }
        if !plan.anomalies.is_empty() {
            der.set("input_source_anomalies", PyValue::List(plan.anomalies.iter().map(|x| PyValue::str(x)).collect()));
        }
        if let Some(pk) = &plan.prior_kv {
            der.set("prior_kv", pk.clone());
        }
        der.set("out_of_scope", PyValue::str("무릎(동시성 대비 처리량 포화)·열벽은 explorer 가 재지 않는다 — adversarial-benchmark 스윕·노드 블랙박스가 재서 escalation_candidates / hint input 으로 넘긴다(트리플렛 직접 쓰기 ✗). max-num-batched-tokens 모델링 범위 밖."));
    }

    let ctx = Ctx { raw: raw_lockset, derivation, kv_origin, declared_axes_check, out: lockset_out, hand: !plan.derive };
    if converged {
        let trial = final_trial.clone().unwrap_or_default();
        converged_path(streams, a, &cfg, &parsed, &cand, &trial, tp, &roles, &run_dir, &run_id, &history, &ctx, &vocab, root, skill_root)?;
        return Ok(0);
    }
    hitl(streams, &run_dir, &run_id, &cand, final_trial.as_ref(), final_class.as_ref(), &history, cap, &roles, &ctx);
    Ok(3)
}

struct Ctx {
    raw: PyDict,
    derivation: Option<PyDict>,
    kv_origin: Option<&'static str>,
    declared_axes_check: PyDict,
    out: Option<PyStr>,
    hand: bool,
}

fn build_lockset(cand: &PyDict, ctx: &Ctx, prov: &PyValue, trial: &PyDict, roles: &Roles, vocab: &Vocab) -> Step<PyDict> {
    let mut out = ctx.raw.clone();
    for k in ["batch", "kv_cache_memory_bytes", "attention_backend", "tool_call_parser", "reasoning_parser"] {
        out.set(k, cg(cand, k));
    }
    let batch_source = if ctx.hand { Some("hand-lever".to_string()) } else { ctx.derivation.as_ref().and_then(|d| str_of(&cg(d, "batch_source"))) };
    let kv_source = if is_none(&cg(cand, "kv_cache_memory_bytes")) { None } else if ctx.kv_origin == Some("trial-loop") { Some("measured-clamp") } else { Some("hand") };
    out.set("provenance", vocab.stamp("provenance", Some("explorer-phase2"))?);
    out.set("batch_source", vocab.stamp("batch_source", batch_source.as_deref())?);
    out.set("kv_source", vocab.stamp("kv_source", kv_source)?);
    out.set("gmu_source", vocab.stamp("gmu_source", Some(roles.gmu_source))?);
    out.set("_explorer_stamp_howto", PyValue::str("recipe.py simulate 수렴이 기계 각인한 칸(2026-09-14 · plan_26091407 §4.2·§4.3): provenance·batch·batch_source·kv_cache_memory_bytes·kv_source·gmu_source·trial_provenance·batch_derivation·gmu_roles. *_source 는 값을 낸 규칙이고, 그 규칙의 입력이 실측이었는지는 trial_provenance 가 말한다. kv_source=hand 는 입력 lockset 클램프를 트라이얼이 바꾸지 않고 검증만 했다는 뜻이다."));
    out.set("trial_provenance", prov.clone());
    out.set("measured", PyValue::Bool(str_of(prov).as_deref() == Some("measured")));
    out.set("batch_derivation", ctx.derivation.as_ref().map_or(PyValue::None, dv));
    out.set("gmu_roles", dv(&gmu_roles_record(roles, Some(trial), cand)));
    out.set("declared_axes_check", dv(&ctx.declared_axes_check));
    Ok(out)
}

#[allow(clippy::too_many_arguments)]
fn converged_path(
    streams: &mut Streams, a: &Args, cfg: &PyValue, parsed: &PyValue, cand: &PyDict, trial: &PyDict, tp: i64, roles: &Roles,
    run_dir: &PyStr, run_id: &PyStr, history: &[PyValue], ctx: &Ctx, vocab: &Vocab, root: &PyStr, skill_root: &PyStr,
) -> R<()> {
    let budget = roles.budget_gib;
    let deploy_gmu = roles.deploy_gmu;
    let profile = dict_of(&cg(trial, "vllm_profile"));
    let weights = cg(&profile, "weights_gib");
    let overhead = cg(&profile, "non_kv_overhead_gib");
    let clamp = cg(cand, "kv_cache_memory_bytes");
    let kv_gib = if !is_none(&clamp) { fl(to_f64(&clamp).unwrap_or(0.0) / GIB) } else { cg(&profile, "kv_cache_gib") };
    let total = if !is_none(&weights) && !is_none(&overhead) && !is_none(&kv_gib) { padd(&padd(&weights, &overhead), &kv_gib) } else { cg(&profile, "total_pool_gib") };

    let serving = { let v = g(cfg, "serving"); if v.is_truthy() { v } else { empty_dict() } };
    let name = g(&serving, "config_name");
    let port = g(&serving, "port");
    let served = g(&serving, "served_model_name");
    if !name.is_truthy() {
        return Err(die(1, "config.serving.config_name 누락").into());
    }
    if !str_of(&name).map_or(false, |n| compose_name_ok(&n)) {
        return Err(die_py(1, Message::new().lit("config.serving.config_name '").text(&name.display_text()).lit(&format!("' 은 docker compose 프로젝트명 규칙 위반 ({} · 점/대문자/공백 불가). 모델명의 점을 빼라(예: glm-4.7 → glm-47). 지금 막지 않으면 3종 세트는 생성되고 compose up 에서만 터진다.", COMPOSE_NAME_RULE)).done()).into());
    }
    if is_none(&port) {
        return Err(die(1, "config.serving.port 누락").into());
    }
    if !served.is_truthy() {
        return Err(die(1, "config.serving.served_model_name 누락").into());
    }
    let port = PyValue::int(raise(int_of(&port))?);

    let gmu_rec = gmu_roles_record(roles, Some(trial), cand);
    let trial_gmu = cg(&gmu_rec, "trial_gmu");
    if let Some(tg) = to_f64(&trial_gmu) {
        if (tg - deploy_gmu).abs() > 1e-9 {
            streams.eprint(&Message::new().lit(&format!("[recipe] ⚠ gmu 분기: trial_gmu {:.3} (", tg)).val(&cg(&gmu_rec, "trial_gmu_source")).lit(&format!(") ↔ deploy_gmu {:.3} ({}) — 검증한 값과 배포되는 값이 다르다. 의도한 것이 아니면 lockset 의 gpu_memory_utilization 을 지우거나 target_gpu.target_gmu 를 맞춰라.", deploy_gmu, roles.deploy_gmu_source)).done());
        }
    }
    let final_prov = cg(trial, "provenance");
    let lockset = build_lockset(cand, ctx, &final_prov, trial, roles, vocab)?;
    let mut vb = PyDict::new();
    vb.set("weights_gib", weights.clone());
    vb.set("kv_gib", kv_gib.clone());
    vb.set("overhead_gib", overhead.clone());
    vb.set("total_gib", total.clone());
    vb.set("budget_gib", fl(budget));
    vb.set("headroom_gib", if is_none(&total) { PyValue::None } else { fl(budget - to_f64(&total).unwrap_or(0.0)) });
    let (k_gm, k_bs, k_gs, k_tg, k_vb) = (s("gpu_memory_utilization"), s("batch_source"), s("gmu_source"), s("target_gpu"), s("vram_breakdown"));
    let (v_gm, v_bs, v_gs, v_tg, v_vb) = (fl(deploy_gmu), cg(&lockset, "batch_source"), cg(&lockset, "gmu_source"), g(cfg, "target_gpu"), dv(&vb));
    let recipe = PyValue::Dict(raise(crate::recipe_set::recipe_from_candidate(&dv(cand), &[(&k_gm, &v_gm), (&k_bs, &v_bs), (&k_gs, &v_gs), (&k_tg, &v_tg), (&k_vb, &v_vb)]))?);

    let out_root = PyValue::Str(join(&join(root, &s("output")), &s(topology_of(root))));
    let force = PyValue::Bool(a.force);
    let topology = PyValue::str("single");
    let none = PyValue::None;
    let image = { let c = cg(cand, "image"); if c.is_truthy() { c } else { PyValue::Str(a.image.clone()) } };
    let request = crate::recipe_set::Request { parsed, recipe: &recipe, name: &name, repo_root: &out_root, port: &port, served_model_name: &served, force: &force, topology: &topology, vllm_version: &none, build_dockerfile: &none, image: &image };
    let mut warnings = PyStr::default();
    let outcome = crate::recipe_set::generate(&request, &mut warnings);
    streams.stderr.push_py(&warnings);
    let paths = match outcome {
        Ok(p) => p,
        Err(f) if f.class == "FileExistsError" => {
            let mut t = f.message.clone();
            t.push_str(" (덮어쓰려면 --force)");
            return Err(die_py(1, t).into());
        }
        Err(f) => return Err(raised(f).into()),
    };
    streams.print(&PyStr::new("[recipe] 수렴 — 생성된 3종 세트:"));
    for p in &paths {
        let mut line = PyStr::new("  - ");
        line.push_py(p);
        streams.print(&line);
    }
    if let Some(out) = &ctx.out {
        write_json_file(out, &dv(&lockset))?;
        let mut line = PyStr::new(&format!("[recipe] lockset 각인(provenance=explorer-phase2 · batch_source={} · gmu_source={} · kv_source={} · trial_provenance={}): ", disp(&cg(&lockset, "batch_source")), disp(&cg(&lockset, "gmu_source")), disp(&cg(&lockset, "kv_source")), disp(&final_prov)));
        line.push_py(out);
        streams.print(&line);
    } else {
        streams.eprint(&PyStr::new("[recipe] ⓘ --lockset-out 미지정 ∧ --candidate 가 캠페인 셀 lockset 이 아니다 — 각인된 lockset 은 run_summary.json 의 lockset 칸에만 남는다(셀 lockset 에 남기려면 --lockset-out 으로 경로를 넘겨라)."));
    }
    let measured = str_of(&final_prov).as_deref() == Some("measured");
    let mut sum = PyDict::new();
    sum.set("run_id", PyValue::Str(run_id.clone()));
    sum.set("converged", PyValue::Bool(true));
    sum.set("trial_count", PyValue::int(history.len() as i64 + 1));
    sum.set("candidate", dv(cand));
    sum.set("vram_breakdown", dv(&vb));
    sum.set("generated_paths", PyValue::List(paths.iter().map(|p| PyValue::Str(p.clone())).collect()));
    sum.set("correction_history", PyValue::List(history.to_vec()));
    sum.set("provenance", final_prov.clone());
    sum.set("measured", PyValue::Bool(measured));
    sum.set("gmu_roles", dv(&gmu_rec));
    sum.set("batch_derivation", cg(&lockset, "batch_derivation"));
    sum.set("lockset", dv(&lockset));
    sum.set("lockset_out", ctx.out.as_ref().map_or(PyValue::None, |p| PyValue::Str(p.clone())));
    if !measured {
        streams.eprint(&PyStr::new(&format!("[recipe] ⚠ 이 레시피는 실측이 아니다(provenance={}) — 서빙 판정·성능 인증의 근거로 쓰지 마라(plan_26081314 D1).", disp(&final_prov))));
    }
    sl(simlog::write_summary(&mut Plain, &PyValue::Str(run_dir.clone()), &dv(&sum)))?;
    let mut sp = run_dir.clone();
    sp.push_str("/run_summary.json");
    streams.eprint(&Message::new().lit("[recipe] simlog 요약: ").text(&sp).done());

    let mut rec = PyDict::new();
    rec.set("timestamp", PyValue::str(&now_iso()));
    rec.set("model_id", g(parsed, "model_id"));
    rec.set("quantization", cg(cand, "quantization"));
    rec.set("max_model_len", cg(cand, "max_model_len"));
    rec.set("gpu_memory_utilization", fl(deploy_gmu));
    rec.set("vram_budget_gb", fl(budget));
    rec.set("estimated_vram_gb", total.clone());
    rec.set("tensor_parallel_size", PyValue::int(tp));
    rec.set("safety_margin_threshold", fl(roles.gate_margin));
    rec.set("batch_source", cg(&lockset, "batch_source"));
    rec.set("gmu_source", cg(&lockset, "gmu_source"));
    rec.set("selection_timestamp", PyValue::str(&now_iso()));
    rec.set("actual_vram_gb", total);
    rec.set("serve_success", PyValue::Bool(true));
    rec.set("error_type", PyValue::None);
    rec.set("tokens_per_sec", PyValue::None);
    rec.set("batch", cg(cand, "batch"));
    rec.set("kv_cache_memory_bytes", cg(cand, "kv_cache_memory_bytes"));
    rec.set("attention_backend", cg(cand, "attention_backend"));
    rec.set("tool_call_parser", cg(cand, "tool_call_parser"));
    rec.set("reasoning_parser", cg(cand, "reasoning_parser"));
    rec.set("converged", PyValue::Bool(true));
    rec.set("trial_count", PyValue::int(history.len() as i64 + 1));
    rec.set("correction_history", PyValue::List(history.to_vec()));
    if a.dry_run || a.mock_profile.is_some() {
        streams.eprint(&PyStr::new("[recipe] (dry-run) 되먹임 로그 기록 생략 — 실측 아님."));
    } else {
        let jsonl = join(&join(skill_root, &s("feedback")), &s("recipe_feedback.jsonl"));
        raise(feedback::append(&dv(&rec), &PyValue::Str(jsonl.clone()), &Clock::Local))?;
        streams.eprint(&Message::new().lit("[recipe] 되먹임 로그 기록: ").text(&jsonl).done());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn hitl(streams: &mut Streams, run_dir: &PyStr, run_id: &PyStr, cand: &PyDict, trial: Option<&PyDict>, fc: Option<&PyDict>, history: &[PyValue], cap: i64, roles: &Roles, ctx: &Ctx) {
    let fclass = fc.and_then(|f| str_of(&cg(f, "failure_class"))).unwrap_or_else(|| "unknown".into());
    let note = fc.map_or(PyValue::str(""), |f| { let n = cg(f, "note"); if f.contains_key("note") { n } else { PyValue::str("") } });
    let log_path = trial.map_or(PyValue::None, |t| cg(t, "log_path"));
    let prov = trial.map_or(PyValue::None, |t| cg(t, "provenance"));
    let mut sum = PyDict::new();
    sum.set("run_id", PyValue::Str(run_id.clone()));
    sum.set("converged", PyValue::Bool(false));
    sum.set("failure_class", PyValue::str(&fclass));
    sum.set("note", note.clone());
    sum.set("trial_count", PyValue::int(history.len() as i64 + if trial.is_some() { 1 } else { 0 }));
    sum.set("candidate", dv(cand));
    sum.set("last_trial_log", log_path.clone());
    sum.set("correction_history", PyValue::List(history.to_vec()));
    sum.set("provenance", prov.clone());
    sum.set("measured", PyValue::Bool(str_of(&prov).as_deref() == Some("measured")));
    sum.set("gmu_roles", dv(&gmu_roles_record(roles, trial, cand)));
    sum.set("batch_derivation", ctx.derivation.as_ref().map_or(PyValue::None, dv));
    let _ = sl(simlog::write_summary(&mut Plain, &PyValue::Str(run_dir.clone()), &dv(&sum)));
    streams.eprint(&PyStr::new("[recipe] ── Model-C (HITL) — 자동 수렴 실패 ──"));
    streams.eprint(&Message::new().lit(&format!("[recipe] failure_class={} :: ", fclass)).val(&note).done());
    if fclass == "vram_infeasible" {
        streams.eprint(&PyStr::new("[recipe] 구조적 VRAM 초과 — 예산 상향 / max_model_len·batch 하향 / KV quant(fp8) / weight quant 검토 필요."));
    } else if fclass == "unknown" {
        streams.eprint(&PyStr::new("[recipe] 알려진 시그니처 미매칭 — 로그를 사람이 분류해야 함."));
    } else {
        streams.eprint(&PyStr::new(&format!("[recipe] cap({}) 소진 또는 폴백 소진 — 추가 조정은 사람이 판단.", cap)));
    }
    streams.eprint(&Message::new().lit("[recipe] 증거 simlog 경로: ").text(run_dir).done());
    if log_path.is_truthy() {
        streams.eprint(&Message::new().lit("[recipe]   마지막 트라이얼 로그: ").val(&log_path).done());
    }
    streams.eprint(&Message::new().lit("[recipe]   요약: ").text(run_dir).lit("/run_summary.json").done());
}

trait IntoValue {
    fn into_value(self) -> PyValue;
}

impl IntoValue for PyStr {
    fn into_value(self) -> PyValue {
        PyValue::Str(self)
    }
}
