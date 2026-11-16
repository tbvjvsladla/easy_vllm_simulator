//! `blackbox_thermal.py` — the thermal/power envelope trip engine of the node blackbox (plan_26082319 §5.2 · §6.3),
//! sister of `blackbox_eta`. Offline/periodic: validates thresholds, replays samples and emits the constants the 1 s
//! hot loop (`thermal_watchdog.sh`) sources. Same command line, exit codes, texts and files as the original; the
//! leaky-bucket state machine (`step`) and every threshold, cap and debounce are the original's integer arithmetic.
//!
//! The SoC plausibility band is now single-owned by `blackbox_collect::SOC_TEMP_PLAUSIBLE_C` (both ports live in one
//! crate, so the cross-check the original made by parsing the collector's source becomes a direct comparison; the
//! self-test label is kept). A hidden `--print-uncalibrated` (not in the help text) prints the comma-joined
//! UNCALIBRATED keys so shell callers read the table from this program instead of grepping a source file.
//!
//! Exit codes: 0 = success · 1 = argument/input refused · 2 = self-test failed (argparse usage errors are 2 as well).

use crate::argp::{self, flag, store, Arg, Outcome, Spec, HELP};
use crate::bb::{self, fr, repr, R};
use crate::py;
use crate::raise::Raised;
use agent_runtime_support::pynumber;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::io::Write;

pub const SCHEMA_VERSION: i64 = 1;
/// Absent/unreadable sentinel (0 would read as "0 W").
pub const ABSENT: i64 = -1;
/// The sensor plausibility band; its owner is the collector.
pub const SOC_PLAUSIBLE_C: (i64, i64) = crate::blackbox_collect::SOC_TEMP_PLAUSIBLE_C;

/// `DEFAULTS` in the original's key order.
pub const DEFAULTS: [(&str, i64); 9] = [
    ("gpu_pwr_w", 80),
    ("gpu_sustain_s", 60),
    ("soc_warn_c", 97),
    ("soc_sustain_s", 30),
    ("soc_hard_c", 99),
    ("soc_hard_polls", 3),
    ("bucket_decay", 1),
    ("poll_interval_s", 1),
    ("stale_after_s", 10),
];

fn defaults() -> &'static [(&'static str, i64)] {
    &DEFAULTS
}

pub fn is_default_key(k: &str) -> bool {
    defaults().iter().any(|(n, _)| *n == k)
}

/// `PARAM_PROVENANCE`, in the original's key order.
pub const PARAM_PROVENANCE: [(&str, &str); 10] = [
    ("gpu_pwr_w", "measured:docs/logs/main/samples 23일 1Hz 전수(2026-07-31~08-23) 그리드 탐색"),
    ("gpu_sustain_s", "measured:생존22일 최대버킷 13 vs 사건일 309 의 기하평균 부근"),
    ("bucket_decay", "measured:decay=1 이 분리비 23.8배로 최대(decay=2 는 23.7, 연속스트릭은 21.8)"),
    ("soc_warn_c", "calibrated:2026-09-09 — EC 팬 커브 100%@97°C(포럼 377044 펌웨어 복구 · 3버전 동일) + 이 유닛 8일 682k samples(max 97°C·무사고) · testlog_26090904 후속"),
    ("soc_sustain_s", "calibrated:2026-09-09 — 술어(버킷 지속초)는 유지 · warn 재기준으로 진성 이탈만 걸림 · 장애 시그니처는 급상승(2°C/s)이라 hard 축이 담당"),
    ("soc_hard_c", "calibrated:2026-09-09 — 장애 스파이크 종점 97.8°C(acpitz) + 마진 · 포럼 하드오프 사례는 센서 결함 진단 동반(정상 관리 상승과 다른 사건)"),
    ("soc_hard_polls", "inherited:blackbox_eta.DEFAULTS['debounce_polls'] 와 동일 개념(1Hz x 3폴)"),
    ("bucket_cap_s", "derived:2 x gpu/soc sustain — 회복이 유계이도록(손으로 적지 않는다)"),
    ("poll_interval_s", "design:수집기와 동일 격자"),
    ("stale_after_s", "derived:수집기 1Hz 격자 + 여유"),
];

pub const CALIBRATED_GPU_MODELS: [&str; 1] = ["NVIDIA GB10"];

fn provenance(k: &str) -> Option<&'static str> {
    PARAM_PROVENANCE.iter().find(|(n, _)| *n == k).map(|x| x.1)
}

/// `UNCALIBRATED`: parameters whose provenance is an external report (sorted).
pub fn uncalibrated() -> Vec<&'static str> {
    let mut v: Vec<&str> = PARAM_PROVENANCE
        .iter()
        .filter(|(_, p)| p.starts_with("external_report:"))
        .map(|x| x.0)
        .collect();
    v.sort();
    v
}

/// `gpu_axis_armed(gpu_model, declared_pwr)`: (armed, reason).
pub fn gpu_axis_armed(gpu_model: Option<&str>, declared_pwr: Option<i64>) -> (bool, String) {
    if declared_pwr.is_some_and(|d| d != 0) {
        return (true, "declared:--set gpu_pwr_w(사람 선언)".into());
    }
    let Some(m) = gpu_model.filter(|m| !m.is_empty()) else {
        return (true, "unknown-gpu:보정 판정 불가 → 종전 무장 유지".into());
    };
    let m = py::strip(m);
    if CALIBRATED_GPU_MODELS.contains(&m) {
        return (true, format!("calibrated:{m}"));
    }
    (
        false,
        format!("uncalibrated-gpu:{m} — 보정값(GB10 실측)을 옮기지 않는다"),
    )
}

/// One parameter value as Python holds it (an int, or a float only reachable through the self-test's guard inputs).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pv {
    I(i64),
    F(f64),
}
impl Pv {
    fn f(self) -> f64 {
        match self {
            Pv::I(n) => n as f64,
            Pv::F(x) => x,
        }
    }
    /// `int(v)`.
    fn i(self) -> i64 {
        match self {
            Pv::I(n) => n,
            Pv::F(x) => x.trunc() as i64,
        }
    }
    /// `repr(v)` / `str(v)`.
    fn s(self) -> String {
        match self {
            Pv::I(n) => n.to_string(),
            Pv::F(x) => fr(x),
        }
    }
}

/// Effective parameters: DEFAULTS with overrides (keys outside DEFAULTS ignored), in DEFAULTS order.
#[derive(Clone, Debug)]
pub struct P(pub Vec<(&'static str, Pv)>);
impl P {
    pub fn get(&self, k: &str) -> Pv {
        self.0.iter().find(|(n, _)| *n == k).map(|x| x.1).expect("known key")
    }
    pub fn i(&self, k: &str) -> i64 {
        self.get(k).i()
    }
}

pub fn effective(params: &[(&str, Pv)]) -> P {
    let mut p: Vec<(&'static str, Pv)> = defaults().iter().map(|(k, v)| (*k, Pv::I(*v))).collect();
    for (k, v) in params {
        if let Some(slot) = p.iter_mut().find(|(n, _)| n == k) {
            slot.1 = *v;
        }
    }
    P(p)
}

pub fn bucket_cap(sustain_s: i64) -> i64 {
    sustain_s * 2
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct State {
    pub gpu_bucket: i64,
    pub soc_bucket: i64,
    pub soc_hard_streak: i64,
}

#[derive(Clone, Copy, Debug)]
pub struct Verdict {
    pub trip: bool,
    pub rule: &'static str,
    pub gpu_bucket: i64,
    pub soc_bucket: i64,
    pub soc_hard_streak: i64,
    pub gpu_stale: bool,
}
impl Verdict {
    fn to_py(self) -> PyValue {
        py::dict(vec![
            ("trip", PyValue::Bool(self.trip)),
            ("rule", py::s(self.rule)),
            ("gpu_bucket", PyValue::int(self.gpu_bucket)),
            ("soc_bucket", PyValue::int(self.soc_bucket)),
            ("soc_hard_streak", PyValue::int(self.soc_hard_streak)),
            ("gpu_stale", PyValue::Bool(self.gpu_stale)),
        ])
    }
}

/// `step(state, gpu_pwr_dw, soc_temp_c, params, gpu_stale)`: one poll of the leaky buckets + the verdict.
pub fn step(st: &mut State, gpu_pwr_dw: i64, soc_temp_c: i64, p: &P, gpu_stale: bool) -> Verdict {
    let decay = p.i("bucket_decay");
    let gcap = bucket_cap(p.i("gpu_sustain_s"));
    let scap = bucket_cap(p.i("soc_sustain_s"));
    let gpu_thr_dw = p.i("gpu_pwr_w") * 10;
    if gpu_stale || gpu_pwr_dw == ABSENT {
        // frozen: neither judged nor updated
    } else if gpu_pwr_dw >= gpu_thr_dw {
        st.gpu_bucket = gcap.min(st.gpu_bucket + 1);
    } else {
        st.gpu_bucket = 0.max(st.gpu_bucket - decay);
    }
    if soc_temp_c != ABSENT {
        if soc_temp_c >= p.i("soc_warn_c") {
            st.soc_bucket = scap.min(st.soc_bucket + 1);
        } else {
            st.soc_bucket = 0.max(st.soc_bucket - decay);
        }
        if soc_temp_c >= p.i("soc_hard_c") {
            st.soc_hard_streak += 1;
        } else {
            st.soc_hard_streak = 0;
        }
    }
    let (rule, trip) = if st.soc_hard_streak >= p.i("soc_hard_polls") {
        ("soc_hard_ceiling", true)
    } else if st.soc_bucket >= p.i("soc_sustain_s") {
        ("soc_temp_sustained", true)
    } else if st.gpu_bucket >= p.i("gpu_sustain_s") {
        ("gpu_pwr_sustained", true)
    } else {
        ("none", false)
    };
    Verdict {
        trip,
        rule,
        gpu_bucket: st.gpu_bucket,
        soc_bucket: st.soc_bucket,
        soc_hard_streak: st.soc_hard_streak,
        gpu_stale,
    }
}

/// `validate_params(params)`: the floor guards; never corrects a value silently.
pub fn validate_params(params: &[(&str, Pv)]) -> (bool, Vec<String>) {
    let mut errs: Vec<String> = vec![];
    let p = effective(params);
    for k in [
        "gpu_pwr_w",
        "gpu_sustain_s",
        "soc_warn_c",
        "soc_sustain_s",
        "soc_hard_c",
        "soc_hard_polls",
        "bucket_decay",
        "poll_interval_s",
        "stale_after_s",
    ] {
        let pv = p.get(k);
        let v = pv.f();
        if v != v.trunc() {
            errs.push(format!(
                "{k} must be an integer (핫루프가 정수 산술만 쓴다) — got {}",
                pv.s()
            ));
        }
        if v <= 0.0 {
            errs.push(format!("{k} must be > 0"));
        }
    }
    if errs.is_empty() {
        if p.i("soc_hard_c") <= p.i("soc_warn_c") {
            errs.push(format!(
                "soc_hard_c({}) must exceed soc_warn_c({}) — 즉시 계층이 지속 계층보다 낮으면 지속 계층이 조용히 무의미해진다",
                p.get("soc_hard_c").s(),
                p.get("soc_warn_c").s()
            ));
        }
        if p.i("bucket_decay") > 1 && p.i("gpu_sustain_s") <= p.i("bucket_decay") {
            errs.push(format!(
                "gpu_sustain_s({}) must exceed bucket_decay({}) — 감소량이 임계보다 크면 버킷이 임계에 도달할 수 없다",
                p.get("gpu_sustain_s").s(),
                p.get("bucket_decay").s()
            ));
        }
        if p.i("stale_after_s") <= p.i("poll_interval_s") {
            errs.push(format!(
                "stale_after_s({}) must exceed poll_interval_s({}) — 정상 격자가 상시 stale 로 읽히면 GPU 축이 영구 동결된다",
                p.get("stale_after_s").s(),
                p.get("poll_interval_s").s()
            ));
        }
    }
    (errs.is_empty(), errs)
}

/// `emit_shell_params(params, path, gpu_axis)`: the `BB_TP_*` constants (power boundary in integer deciwatts).
pub fn emit_shell_params(params: &[(&str, Pv)], path: &str, gpu_axis: &(bool, String)) -> Result<(), Raised> {
    let p = effective(params);
    let mut lines: Vec<String> = vec![
        "# generated by blackbox_thermal.py -- 편집 금지(재생성으로 갱신)".into(),
        "# 규칙: 값>=임계면 bucket+1(상한 cap), 아니면 bucket-decay(하한 0). TRIP <=> bucket>=sustain".into(),
        "# 축은 서로 독립이다 -- 어느 하나라도 TRIP 이면 kill(하드다운은 어느 축으로든 온다).".into(),
        "# ⚠ GPU '온도'는 트립 축이 **아니다**(실측 반증 — blackbox_thermal.py 헤더 §음성정직 1).".into(),
    ];
    let mut items = p.0.clone();
    items.sort_by(|a, b| a.0.cmp(b.0));
    for (k, v) in &items {
        lines.push(format!("# 출처: {}", provenance(k).unwrap_or("unknown")));
        lines.push(format!("BB_TP_{}={}", k.to_uppercase(), v.i()));
    }
    lines.push(format!(
        "# 파생값 — 손으로 적지 않는다(출처: {})",
        provenance("bucket_cap_s").unwrap()
    ));
    lines.push(format!("BB_TP_GPU_PWR_DW={}", p.i("gpu_pwr_w") * 10));
    lines.push(format!("BB_TP_SOC_PLAUSIBLE_MIN_C={}", SOC_PLAUSIBLE_C.0));
    lines.push(format!("BB_TP_SOC_PLAUSIBLE_MAX_C={}", SOC_PLAUSIBLE_C.1));
    lines.push(format!(
        "BB_TP_GPU_BUCKET_CAP={}",
        bucket_cap(p.i("gpu_sustain_s"))
    ));
    lines.push(format!(
        "BB_TP_SOC_BUCKET_CAP={}",
        bucket_cap(p.i("soc_sustain_s"))
    ));
    lines.push("# ⚠ 미교정 파라미터(외부 보고 역산 — 이 노드 실측 분포 없음):".into());
    let unc = uncalibrated();
    for k in &unc {
        lines.push(format!("#     {} = {}", k, p.get(k).s()));
    }
    lines.push(format!("BB_TP_UNCALIBRATED=\"{}\"", unc.join(" ")));
    lines.push(format!("# GPU 전력 축 무장: {}", gpu_axis.1));
    lines.push(format!(
        "BB_TP_GPU_AXIS={}",
        if gpu_axis.0 { 1 } else { 0 }
    ));
    bb::write_atomic(path, &(lines.join("\n") + "\n"))
}

/// Python `int(round(x))`.
fn round_int(x: f64) -> Result<i64, Raised> {
    if x.is_nan() {
        return Err(Raised::new("ValueError", "cannot convert float NaN to integer"));
    }
    if x.is_infinite() {
        return Err(Raised::new(
            "OverflowError",
            "cannot convert float infinity to integer",
        ));
    }
    let r = x.round();
    let v = if (x - x.trunc()).abs() == 0.5 {
        let t = x.trunc();
        if t % 2.0 == 0.0 {
            t
        } else {
            r
        }
    } else {
        r
    };
    Ok(v as i64)
}

/// Python `int(x)` of a float.
fn trunc_int(x: f64) -> Result<i64, Raised> {
    if x.is_nan() {
        return Err(Raised::new("ValueError", "cannot convert float NaN to integer"));
    }
    if x.is_infinite() {
        return Err(Raised::new(
            "OverflowError",
            "cannot convert float infinity to integer",
        ));
    }
    Ok(x.trunc() as i64)
}

fn is_value_error(r: &Raised) -> bool {
    r.0.starts_with("ValueError:")
}

fn round2(x: f64) -> f64 {
    pynumber::round_to_places(x, 2)
}

/// The original's `replay_samples` result.
#[derive(Clone, Debug)]
pub struct Replay {
    pub path: String,
    pub rows: i64,
    pub kills: i64,
    pub first_kill: Option<(i64, &'static str, Option<f64>, Option<i64>)>,
    pub peak_gpu_bucket: i64,
    pub peak_soc_bucket: i64,
    pub gpu_sustain_s: i64,
    pub soc_sustain_s: i64,
    pub max_gpu_pwr_w: Option<f64>,
    pub max_soc_temp_c: Option<i64>,
    pub soc_axis_evaluated: bool,
}

impl Replay {
    fn to_py(&self) -> PyValue {
        let fk = match &self.first_kill {
            None => PyValue::None,
            Some((ts, rule, w, c)) => py::dict(vec![
                ("ts", PyValue::int(*ts)),
                ("rule", py::s(rule)),
                ("gpu_pwr_w", w.map_or(PyValue::None, PyValue::Float)),
                ("soc_temp_c", c.map_or(PyValue::None, PyValue::int)),
            ]),
        };
        py::dict(vec![
            ("path", py::s(&self.path)),
            ("rows", PyValue::int(self.rows)),
            ("kills", PyValue::int(self.kills)),
            ("first_kill", fk),
            ("peak_gpu_bucket", PyValue::int(self.peak_gpu_bucket)),
            ("peak_soc_bucket", PyValue::int(self.peak_soc_bucket)),
            ("gpu_sustain_s", PyValue::int(self.gpu_sustain_s)),
            ("soc_sustain_s", PyValue::int(self.soc_sustain_s)),
            (
                "max_gpu_pwr_w",
                self.max_gpu_pwr_w.map_or(PyValue::None, PyValue::Float),
            ),
            (
                "max_soc_temp_c",
                self.max_soc_temp_c.map_or(PyValue::None, PyValue::int),
            ),
            ("soc_axis_evaluated", PyValue::Bool(self.soc_axis_evaluated)),
        ])
    }
}

/// `str.isdigit()` for the characters a timestamp cell can hold (decimal digits; documented residual: the original
/// also accepts other Unicode digit characters, which `int()` then refuses).
fn isdigit(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| agent_runtime_support::pystr::is_decimal_digit(c as u32))
}

/// `replay_samples(path, params)`: the hot loop's state machine over a samples CSV (header may change mid-file).
pub fn replay_samples(path: &str, p: &P) -> Result<Option<Replay>, Raised> {
    let Some(lines) = crate::blackbox_eta::read_csv_lines(path) else {
        return Ok(None);
    };
    if lines.is_empty() {
        return Ok(None);
    }
    let mut idx: std::collections::HashMap<String, usize> = Default::default();
    let mut st = State::default();
    let (mut rows, mut kills) = (0i64, 0i64);
    let mut first_kill = None;
    let (mut peak_gpu, mut peak_soc) = (0i64, 0i64);
    let mut max_pwr_dw = -1i64;
    let mut max_soc = -1i64;
    let mut have_soc = false;
    for line in &lines {
        let parts: Vec<&str> = line.split(',').collect();
        if py::strip(parts[0]) == "ts" {
            idx.clear();
            for (i, k) in parts.iter().enumerate() {
                idx.insert(py::strip(k).to_string(), i);
            }
            continue;
        }
        if idx.is_empty() {
            continue;
        }
        let cell = |key: &str| -> Option<String> {
            let i = *idx.get(key)?;
            let v = py::strip(parts.get(i)?);
            if v.is_empty() {
                None
            } else {
                Some(v.to_string())
            }
        };
        let Some(ts) = cell("ts").filter(|t| isdigit(t)) else {
            continue;
        };
        let pwr_dw = match cell("gpu_pwr") {
            None => ABSENT,
            Some(w) => match pynumber::float_from_str(&PyStr::new(&w)) {
                Err(_) => ABSENT,
                Ok(x) => match round_int(x * 10.0) {
                    Ok(n) => n,
                    Err(e) if is_value_error(&e) => ABSENT,
                    Err(e) => return Err(e),
                },
            },
        };
        let soc_c = match cell("soc_temp") {
            None => ABSENT,
            Some(s) => match pynumber::float_from_str(&PyStr::new(&s)) {
                Err(_) => ABSENT,
                Ok(x) => match trunc_int(x) {
                    Ok(n) => n,
                    Err(e) if is_value_error(&e) => ABSENT,
                    Err(e) => return Err(e),
                },
            },
        };
        have_soc = have_soc || soc_c != ABSENT;
        rows += 1;
        max_pwr_dw = max_pwr_dw.max(pwr_dw);
        max_soc = max_soc.max(soc_c);
        let r = step(&mut st, pwr_dw, soc_c, p, false);
        peak_gpu = peak_gpu.max(r.gpu_bucket);
        peak_soc = peak_soc.max(r.soc_bucket);
        if r.trip {
            kills += 1;
            if first_kill.is_none() {
                let t = bb::py_int_text(&ts).unwrap_or(0);
                first_kill = Some((
                    t,
                    r.rule,
                    if pwr_dw != ABSENT {
                        Some(round2(pwr_dw as f64 / 10.0))
                    } else {
                        None
                    },
                    if soc_c != ABSENT { Some(soc_c) } else { None },
                ));
            }
            st = State::default();
        }
    }
    Ok(Some(Replay {
        path: path.to_string(),
        rows,
        kills,
        first_kill,
        peak_gpu_bucket: peak_gpu,
        peak_soc_bucket: peak_soc,
        gpu_sustain_s: p.i("gpu_sustain_s"),
        soc_sustain_s: p.i("soc_sustain_s"),
        max_gpu_pwr_w: if max_pwr_dw != ABSENT {
            Some(round2(max_pwr_dw as f64 / 10.0))
        } else {
            None
        },
        max_soc_temp_c: if max_soc != ABSENT { Some(max_soc) } else { None },
        soc_axis_evaluated: have_soc,
    }))
}

fn opt_i_repr(v: Option<i64>) -> String {
    v.map_or("None".into(), |n| n.to_string())
}

fn self_test() -> i32 {
    let mut checks: Vec<(String, bool)> = vec![];
    let mut chk = |n: &str, b: bool| checks.push((n.to_string(), b));

    // the band's owner is the collector (same crate: a direct comparison)
    chk(
        "타당성 밴드가 정본(blackbox_collect.SOC_TEMP_PLAUSIBLE_C)과 일치",
        crate::blackbox_collect::SOC_TEMP_PLAUSIBLE_C == SOC_PLAUSIBLE_C,
    );

    // the hot loop's built-in defaults must not drift from DEFAULTS
    let sh = bb::join(
        &bb::dirname(
            &crate::owner::owner_path("node_blackbox/blackbox_thermal.py")
                .display()
                .to_string(),
        ),
        "thermal_watchdog.sh",
    );
    let mut shvals: Vec<(String, i64)> = vec![];
    let mut mismatch: Vec<String> = vec![];
    match std::fs::read(&sh) {
        Ok(bytes) => {
            let text = bb::universal_newlines(&String::from_utf8_lossy(&bytes));
            for ln in bb::text_lines(&text) {
                let ln = py::strip(ln);
                if !ln.starts_with("BB_TP_") {
                    continue;
                }
                for asgn in ln.split(';') {
                    let asgn = py::strip(asgn);
                    if !asgn.starts_with("BB_TP_") || !asgn.contains('=') {
                        continue;
                    }
                    let (k, v) = asgn.split_once('=').unwrap();
                    let digits = v.trim_start_matches('-');
                    if !shvals.iter().any(|(n, _)| n == k) && isdigit(digits) {
                        if let Some(n) = bb::py_int_text(v) {
                            shvals.push((k.to_string(), n));
                        }
                    }
                }
            }
        }
        Err(_) => mismatch.push("thermal_watchdog.sh 판독 불가".into()),
    }
    let mut expect: Vec<(String, i64)> = vec![
        ("BB_TP_GPU_PWR_DW".into(), 800),
        ("BB_TP_GPU_AXIS".into(), 1),
        ("BB_TP_GPU_BUCKET_CAP".into(), bucket_cap(60)),
        ("BB_TP_SOC_BUCKET_CAP".into(), bucket_cap(30)),
        ("BB_TP_SOC_PLAUSIBLE_MIN_C".into(), SOC_PLAUSIBLE_C.0),
        ("BB_TP_SOC_PLAUSIBLE_MAX_C".into(), SOC_PLAUSIBLE_C.1),
    ];
    for (k, v) in defaults() {
        if *k == "gpu_pwr_w" {
            continue;
        }
        let key = format!("BB_TP_{}", k.to_uppercase());
        if !expect.iter().any(|(n, _)| *n == key) {
            expect.push((key, *v));
        }
    }
    expect.sort();
    for (k, v) in &expect {
        let got = shvals.iter().find(|(n, _)| n == k).map(|x| x.1);
        if got != Some(*v) {
            mismatch.push(format!("{}: 셸={} 정본={}", k, opt_i_repr(got), v));
        }
    }
    chk(
        &format!(
            "핫루프 내장 기본값이 정본 DEFAULTS 와 일치{}",
            if mismatch.is_empty() {
                String::new()
            } else {
                format!(" — {}", mismatch.join("; "))
            }
        ),
        mismatch.is_empty(),
    );

    chk(
        "버킷 상한은 sustain 의 2배(파생)",
        bucket_cap(60) == 120 && bucket_cap(30) == 60,
    );
    let (ok, errs) = validate_params(&[]);
    chk("기본값은 하한 가드 통과", ok && errs.is_empty());
    let (ok, errs) = validate_params(&[("soc_hard_c", Pv::I(85))]);
    chk(
        "즉시선 <= 지속선 -> 거부",
        !ok && errs.iter().any(|e| e.contains("soc_hard_c")),
    );
    chk("0 임계 -> 거부", !validate_params(&[("gpu_pwr_w", Pv::I(0))]).0);
    chk(
        "비정수 -> 거부(핫루프는 정수 산술)",
        !validate_params(&[("gpu_sustain_s", Pv::F(60.5))]).0,
    );
    chk(
        "stale <= poll -> 거부(GPU 축 영구 동결 방지)",
        !validate_params(&[("stale_after_s", Pv::I(1))]).0,
    );

    let p = effective(&[]);
    let mut st = State::default();
    let mut r = step(&mut st, 900, ABSENT, &p, false);
    for _ in 1..59 {
        r = step(&mut st, 900, ABSENT, &p, false);
    }
    chk(
        "59 폴 고전력 -> 아직 미발동(경계 아래)",
        !r.trip && r.gpu_bucket == 59,
    );
    r = step(&mut st, 900, ABSENT, &p, false);
    chk(
        "60 폴 고전력 -> TRIP(rule=gpu_pwr_sustained)",
        r.trip && r.rule == "gpu_pwr_sustained",
    );
    st = State::default();
    for _ in 0..10 {
        r = step(&mut st, 1008, ABSENT, &p, false);
    }
    chk(
        "★음성대조: 100.8W 10초 버스트 -> 미발동",
        !r.trip && r.gpu_bucket == 10,
    );
    for _ in 0..10 {
        r = step(&mut st, 130, ABSENT, &p, false);
    }
    chk("버스트 종료 후 버킷 배출", r.gpu_bucket == 0);
    st = State::default();
    r = step(&mut st, 800, ABSENT, &p, false);
    chk("80.0W 경계는 고부하(>=)", r.gpu_bucket == 1);
    st = State::default();
    r = step(&mut st, 799, ABSENT, &p, false);
    chk("79.9W 는 고부하 아님", r.gpu_bucket == 0);
    st = State::default();
    for i in 0..80 {
        r = step(&mut st, if i % 8 == 7 { 300 } else { 900 }, ABSENT, &p, false);
    }
    chk(
        "dropout 진동에도 지속으로 인식(연속 스트릭이었으면 미발동)",
        r.trip,
    );
    st = State::default();
    for _ in 0..3600 {
        r = step(&mut st, 450, ABSENT, &p, false);
    }
    chk("45W 1시간 -> 미발동", !r.trip && r.gpu_bucket == 0);

    st = State::default();
    for _ in 0..29 {
        r = step(&mut st, ABSENT, 96, &p, false);
    }
    chk("SoC 96도(warn 미만) 29폴 -> 미발동", !r.trip);
    let _ = step(&mut st, ABSENT, 97, &p, false);
    let mut r2 = r;
    for _ in 0..29 {
        r2 = step(&mut st, ABSENT, 97, &p, false);
    }
    chk(
        "SoC 97도(warn 이상) 30폴 -> TRIP(soc_temp_sustained)",
        r2.trip && r2.rule == "soc_temp_sustained",
    );
    st = State::default();
    for _ in 0..3 {
        r = step(&mut st, ABSENT, 99, &p, false);
    }
    chk(
        "SoC 99도(hard 이상) 3폴 -> 즉시계층 TRIP(soc_hard_ceiling)",
        r.trip && r.rule == "soc_hard_ceiling",
    );
    st = State::default();
    let _ = step(&mut st, ABSENT, 99, &p, false);
    let _ = step(&mut st, ABSENT, 60, &p, false);
    r = step(&mut st, ABSENT, 99, &p, false);
    chk("SoC 즉시계층은 연속이어야 한다(끊기면 스트릭 리셋)", !r.trip);
    st = State::default();
    for _ in 0..600 {
        r = step(&mut st, ABSENT, 47, &p, false);
    }
    chk("SoC 평시(47도) -> 미발동", !r.trip && r.soc_bucket == 0);

    st = State::default();
    for _ in 0..600 {
        r = step(&mut st, ABSENT, ABSENT, &p, false);
    }
    chk(
        "양축 부재 -> 버킷 불변(0 취급 금지)",
        r.gpu_bucket == 0 && r.soc_bucket == 0 && !r.trip,
    );
    st = State::default();
    for _ in 0..30 {
        step(&mut st, 900, ABSENT, &p, false);
    }
    let before = st.gpu_bucket;
    for _ in 0..50 {
        r = step(&mut st, 900, ABSENT, &p, true);
    }
    chk(
        "stale -> GPU 버킷 동결(더하지도 빼지도 않는다)",
        st.gpu_bucket == before && !r.trip && r.gpu_stale,
    );
    st = State::default();
    for _ in 0..30 {
        step(&mut st, 900, ABSENT, &p, false);
    }
    for _ in 0..40 {
        let _ = step(&mut st, ABSENT, ABSENT, &p, false);
    }
    chk(
        "부재로 바뀌어도 축적분이 사라지지 않는다(조용한 무장해제 금지)",
        st.gpu_bucket == 30,
    );
    st = State::default();
    for _ in 0..30 {
        r = step(&mut st, 100, 97, &p, false);
    }
    chk(
        "한 축만 차도 TRIP(축 독립)",
        r.trip && r.rule == "soc_temp_sustained",
    );

    {
        let td = bb::TempDir::new();
        let env = bb::join(td.path(), "thermal_params.env");
        let _ = emit_shell_params(&[], &env, &(true, "default".into()));
        let txt = std::fs::read_to_string(&env).unwrap_or_default();
        chk("emit: 데시와트 정수 경계", txt.contains("BB_TP_GPU_PWR_DW=800"));
        chk(
            "emit: 파생 버킷 상한",
            txt.contains("BB_TP_GPU_BUCKET_CAP=120") && txt.contains("BB_TP_SOC_BUCKET_CAP=60"),
        );
        chk(
            "emit: 모든 파라미터에 출처 주석",
            txt.matches("# 출처:").count() == defaults().len(),
        );
        chk(
            "emit: 센서 타당성 밴드 동봉(핫루프가 잡음을 먹지 않게)",
            txt.contains("BB_TP_SOC_PLAUSIBLE_MIN_C=-40")
                && txt.contains("BB_TP_SOC_PLAUSIBLE_MAX_C=150"),
        );
        chk(
            "emit: 미교정 목록을 데이터로 실어 본다(2026-09-09 전수 교정 — 빈 목록이 정답)",
            txt.contains("BB_TP_UNCALIBRATED=\"\""),
        );
        chk(
            "emit: GPU 온도가 축이 아님을 상수파일이 밝힌다",
            txt.contains("GPU '온도'는 트립 축이"),
        );
        chk("emit: 기본은 GPU 전력 축 무장", txt.contains("BB_TP_GPU_AXIS=1"));
        let _ = emit_shell_params(
            &[],
            &env,
            &gpu_axis_armed(Some("NVIDIA GeForce RTX 5090"), None),
        );
        let t2 = std::fs::read_to_string(&env).unwrap_or_default();
        chk(
            "emit: 보정 밖 GPU(RTX 5090) → GPU 전력 축 비무장 + 사유 기재",
            t2.contains("BB_TP_GPU_AXIS=0") && t2.contains("uncalibrated-gpu"),
        );
    }
    chk(
        "gpu_axis: GB10 = 보정 무장",
        gpu_axis_armed(Some("NVIDIA GB10"), None).0,
    );
    chk(
        "gpu_axis: 미상 GPU = 종전 무장(fail-safe)",
        gpu_axis_armed(None, None).0,
    );
    chk(
        "gpu_axis: 보정 밖 + 사람 선언 = 무장",
        gpu_axis_armed(Some("NVIDIA GeForce RTX 5090"), Some(450)).0,
    );
    {
        let td = bb::TempDir::new();
        let env = bb::join(td.path(), "thermal_params.env");
        chk(
            "emit: 원자적 교체(임시파일 잔재 없음)",
            std::fs::metadata(format!("{env}.tmp")).is_err(),
        );
        let hdr12 = "ts,mem_avail,mem_rate,gpu_temp,gpu_pwr,gpu_sm,gpu_util,gpu_mem,load1,ctr_n,soc_temp,soc_zone\n";
        let hdr10 = "ts,mem_avail,mem_rate,gpu_temp,gpu_pwr,gpu_sm,gpu_util,gpu_mem,load1,ctr_n\n";
        let csv = bb::join(td.path(), "incident.csv");
        let mut body = hdr12.to_string();
        for i in 0..90 {
            body.push_str(&format!("{},19251,0,63,90.33,2457,96,,1.0,1,47,0\n", 1787477017 + i));
        }
        let _ = std::fs::write(&csv, body);
        let r = replay_samples(&csv, &p).ok().flatten();
        chk(
            "재생: 사건 축약본 -> kill 발생",
            r.as_ref().is_some_and(|r| r.kills >= 1),
        );
        let fk = r.as_ref().and_then(|r| r.first_kill);
        chk(
            "재생: 첫 kill 이 60 폴째(트립레벨)",
            fk.is_some_and(|f| f.0 == 1787477017 + 59),
        );
        chk(
            "재생: 규칙 라벨 보존",
            fk.is_some_and(|f| f.1 == "gpu_pwr_sustained"),
        );
        chk(
            "재생: SoC 축이 평가됐음을 표기",
            r.as_ref().is_some_and(|r| r.soc_axis_evaluated),
        );
        let csv2 = bb::join(td.path(), "benign.csv");
        let mut body = hdr12.to_string();
        let mut t = 1787400000i64;
        for _ in 0..20 {
            for _ in 0..10 {
                body.push_str(&format!("{t},60000,0,70,96.5,2400,96,,1.0,1,50,0\n"));
                t += 1;
            }
            for _ in 0..60 {
                body.push_str(&format!("{t},60000,0,50,13.0,2400,0,,1.0,1,47,0\n"));
                t += 1;
            }
        }
        let _ = std::fs::write(&csv2, body);
        let r2 = replay_samples(&csv2, &p).ok().flatten();
        chk(
            "★음성대조 재생: 반복 순간버스트 -> kill 0회",
            r2.as_ref().is_some_and(|r| r.kills == 0),
        );
        chk(
            "음성대조 버킷 최고치가 트립레벨의 절반 이하",
            r2.as_ref()
                .is_some_and(|r| r.peak_gpu_bucket * 2 <= r.gpu_sustain_s),
        );
        let csv3 = bb::join(td.path(), "legacy.csv");
        let mut body = hdr10.to_string();
        for i in 0..30 {
            body.push_str(&format!("{},60000,0,70,45.0,2400,50,,1.0,1\n", 1787400000 + i));
        }
        let _ = std::fs::write(&csv3, body);
        let r3 = replay_samples(&csv3, &p).ok().flatten();
        chk(
            "옛 스키마: SoC 축 미평가를 정직 표기(안전의 증거가 아니다)",
            r3.as_ref()
                .is_some_and(|r| !r.soc_axis_evaluated && r.rows == 30),
        );
        let csv4 = bb::join(td.path(), "drift.csv");
        let body = format!(
            "{hdr10}1787400000,60000,0,70,45.0,2400,50,,1.0,1\n{hdr12}1787400001,60000,0,70,45.0,2400,50,,1.0,1,93,5\n"
        );
        let _ = std::fs::write(&csv4, body);
        let r4 = replay_samples(&csv4, &p).ok().flatten();
        chk(
            "드리프트 파일: 재헤더 뒤 SoC 열을 올바로 읽는다",
            r4.as_ref()
                .is_some_and(|r| r.rows == 2 && r.max_soc_temp_c == Some(93)),
        );
        chk(
            "판독 불가 경로 -> None(조용한 빈 결과 금지)",
            matches!(replay_samples(&bb::join(td.path(), "nope.csv"), &p), Ok(None)),
        );
    }

    let ok = bb::print_checks(&checks);
    println!(
        "self-test: {} ({} 케이스)",
        if ok { "PASS" } else { "FAIL" },
        checks.len()
    );
    if ok {
        0
    } else {
        2
    }
}

const A_EMIT: usize = 1;
const A_SET: usize = 2;
const A_EXPLAIN: usize = 3;
const A_REPLAY: usize = 4;
const A_GPU: usize = 5;
const A_SELF: usize = 6;
const A_UNCAL: usize = 7;

static ARGS: [Arg; 8] = [
    HELP,
    store(&["--emit-params"]),
    store(&["--set"]).append(),
    store(&["--explain"]).float().nargs(2),
    store(&["--replay"]).nargs_plus(),
    store(&["--gpu-model"]),
    flag(&["--self-test"]),
    // not in the help text (an addition of the port for shell callers)
    flag(&["--print-uncalibrated"]),
];

pub static SPEC: Spec = Spec {
    prog: "blackbox_thermal.py",
    usage: include_str!("../help/blackbox_thermal.usage"),
    help: include_str!("../help/blackbox_thermal.help"),
    args: &ARGS,
    subs: None,
};

fn sorted_keys() -> String {
    let mut v: Vec<&str> = defaults().iter().map(|x| x.0).collect();
    v.sort();
    v.join(", ")
}

fn pdict(p: &P) -> PyValue {
    let mut d = PyDict::new();
    for (k, v) in &p.0 {
        d.set(
            k,
            match v {
                Pv::I(n) => PyValue::int(*n),
                Pv::F(x) => PyValue::Float(*x),
            },
        );
    }
    PyValue::Dict(d)
}

fn uncal_py() -> PyValue {
    PyValue::List(uncalibrated().iter().map(|k| py::s(k)).collect())
}

fn axes_summary(p: &P) -> String {
    format!(
        "gpu>={}W {}s 지속 · soc>={}C {}s 지속 · soc>={}C {}폴",
        p.get("gpu_pwr_w").s(),
        p.get("gpu_sustain_s").s(),
        p.get("soc_warn_c").s(),
        p.get("soc_sustain_s").s(),
        p.get("soc_hard_c").s(),
        p.get("soc_hard_polls").s()
    )
}

fn run(a: &argp::Parsed) -> R<i32> {
    if a.flag(A_SELF) {
        return Ok(self_test());
    }
    if a.flag(A_UNCAL) {
        println!("{}", uncalibrated().join(","));
        return Ok(0);
    }
    let mut params: Vec<(&str, Pv)> = vec![];
    for kv in a.many(A_SET) {
        let Some((k, v)) = kv.split_once('=') else {
            return Err(bb::ap_error(&SPEC, &format!("--set 는 K=V 형식: {}", repr(kv))));
        };
        let Some(key) = defaults().iter().find(|(n, _)| *n == k).map(|x| x.0) else {
            return Err(bb::ap_error(
                &SPEC,
                &format!("알 수 없는 파라미터 {} (가능: {})", repr(k), sorted_keys()),
            ));
        };
        let x = pynumber::float_from_str(&PyStr::new(v))
            .map_err(|e| Raised::new(e.class, &e.message))?;
        let n = trunc_int(x)?;
        match params.iter_mut().find(|(n0, _)| *n0 == key) {
            Some(slot) => slot.1 = Pv::I(n),
            None => params.push((key, Pv::I(n))),
        }
    }
    let (ok, errs) = validate_params(&params);
    if !ok {
        let mut e = std::io::stderr().lock();
        let _ = writeln!(e, "[thermal] 하한 가드 위반 — 거부:");
        for x in errs {
            let _ = writeln!(e, "  - {x}");
        }
        return Ok(1);
    }
    let p = effective(&params);

    if let Some(ex) = a.floats(A_EXPLAIN) {
        let (w, c) = (ex[0], ex[1]);
        let mut st = State::default();
        let mut polls = 0i64;
        let pwr_dw = round_int(w * 10.0)?;
        let soc_c = trunc_int(c)?;
        let limit = 4 * p.i("gpu_sustain_s").max(p.i("soc_sustain_s")) + 10;
        let mut res: Option<Verdict> = None;
        while polls < limit && !res.is_some_and(|r| r.trip) {
            res = Some(step(&mut st, pwr_dw, soc_c, &p, false));
            polls += 1;
        }
        let trip = res.is_some_and(|r| r.trip);
        let verdict = match res {
            Some(r) => r.to_py(),
            None => py::dict(vec![("trip", PyValue::Bool(false)), ("rule", py::s("none"))]),
        };
        let mut prov = PyDict::new();
        for (k, v) in PARAM_PROVENANCE {
            prov.set(k, py::s(v));
        }
        let doc = py::dict(vec![
            (
                "input",
                py::dict(vec![
                    ("gpu_pwr_w", PyValue::Float(w)),
                    ("soc_temp_c", PyValue::Float(c)),
                ]),
            ),
            ("params", pdict(&p)),
            ("provenance", PyValue::Dict(prov)),
            ("uncalibrated", uncal_py()),
            ("verdict", verdict),
            (
                "polls_to_trip",
                if trip { PyValue::int(polls) } else { PyValue::None },
            ),
            (
                "note",
                py::s(&if trip {
                    format!("이 입력이 유지되면 {polls} 초 뒤 TRIP")
                } else {
                    format!("이 입력이 유지돼도 TRIP 하지 않는다({limit} 폴까지 확인)")
                }),
            ),
        ]);
        println!("{}", py::dumps_pretty(&doc));
        return Ok(0);
    }

    let replay = a.many(A_REPLAY);
    if !replay.is_empty() {
        let mut out: Vec<PyValue> = vec![];
        let mut total = 0i64;
        for path in replay {
            let Some(r) = replay_samples(path, &p)? else {
                eprintln!("[thermal] ⚠ 재생 불가(판독 실패): {path}");
                out.push(py::dict(vec![("path", py::s(path)), ("error", py::s("unreadable"))]));
                continue;
            };
            total += r.kills;
            let maxw = r.max_gpu_pwr_w.map_or("None".into(), fr);
            let maxsoc = if r.soc_axis_evaluated {
                opt_i_repr(r.max_soc_temp_c)
            } else {
                "n/a".into()
            };
            println!(
                "[thermal] {} rows={} maxW={} maxSoC={} peak(gpu/soc)={}/{} kill={}{}{}",
                bb::ljust(&bb::basename(path), 22),
                bb::ljust(&r.rows.to_string(), 6),
                bb::ljust(&maxw, 6),
                bb::ljust(&maxsoc, 4),
                r.peak_gpu_bucket,
                r.peak_soc_bucket,
                r.kills,
                match r.first_kill {
                    None => String::new(),
                    Some((ts, rule, _, _)) => format!("  ← 최초 ts={ts} rule={rule}"),
                },
                if r.soc_axis_evaluated {
                    ""
                } else {
                    "  [SoC 축 미평가]"
                }
            );
            out.push(r.to_py());
        }
        println!("[thermal] 합계 kill={} ({})", total, axes_summary(&p));
        let doc = py::dict(vec![
            ("files", PyValue::List(out)),
            ("total_kills", PyValue::int(total)),
            ("params", pdict(&p)),
            ("uncalibrated", uncal_py()),
        ]);
        println!("{}", py::dumps_pretty(&doc));
        return Ok(0);
    }

    if let Some(path) = a.text(A_EMIT).filter(|s| !s.is_empty()) {
        let declared = params.iter().find(|(k, _)| *k == "gpu_pwr_w").map(|x| x.1.i());
        let axis = gpu_axis_armed(a.text(A_GPU), declared);
        emit_shell_params(&params, path, &axis)?;
        if !axis.0 {
            eprintln!("[thermal] ⚠ GPU 전력 축 비무장 — {}", axis.1);
        }
        println!("[thermal] emit {} ({})", path, axes_summary(&p));
        let unc = uncalibrated();
        if !unc.is_empty() {
            eprintln!(
                "[thermal] ⚠ 미교정(외부 보고 역산 — 이 노드 실측 분포 없음): {}",
                unc.join(", ")
            );
        }
        return Ok(0);
    }
    Err(bb::ap_error(
        &SPEC,
        "--explain / --emit-params / --replay / --self-test 중 하나가 필요합니다",
    ))
}

pub fn main(argv: &[String]) -> i32 {
    match argp::parse(&SPEC, argv) {
        Outcome::Exit(c) => c,
        Outcome::Ok(a) => bb::finish(run(&a)),
    }
}
