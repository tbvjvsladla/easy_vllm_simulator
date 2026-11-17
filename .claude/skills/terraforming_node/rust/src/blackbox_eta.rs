//! `blackbox_eta.py` — the ETA engine and envelope feedback of the node blackbox (plan_26073109 §2.2, dual rule
//! plan_26081415 C1-A). Same command line, exit codes, texts and files as the original; the decision functions
//! (`compute_eta`, `max_rate_mib_s`, `trip_threshold_mib`, `validate_params`, `replay_samples`) keep the original's
//! arithmetic (IEEE doubles where Python used `float`, integer floor of the dual-rule ceiling) so every TRIP decision is
//! the same. This module is also the single owner of `DEFAULTS` for the other blackbox ports (they derive, never copy).
//!
//! Exit codes: 0 = success · 1 = argument/input refused · 2 = self-test failed (argparse usage errors are 2 as well).

use crate::argp::{self, flag, store, Arg, Outcome, Spec, HELP};
use crate::bb::{self, fr, pct, repr, Fail, R};
use crate::py;
use crate::raise::Raised;
use agent_runtime_support::pynumber;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::io::Write;

pub const SCHEMA_VERSION: i64 = 1;
pub const ETA_FLOOR_MULTIPLIER: f64 = 1.5;
pub const MAX_RATE_DISABLED: i64 = 0;
pub const ENVELOPE_META_KEYS: [&str; 1] = ["provenance"];

/// One default: an int or a float, as written in the original's `DEFAULTS` dict.
#[derive(Clone, Copy)]
pub enum D {
    F(f64),
    I(i64),
}

/// `DEFAULTS`, in the original's key order (initial arbitrary values; see the original's header).
pub const DEFAULTS: [(&str, D); 11] = [
    ("kill_latency_s", D::F(4.0)),
    ("detect_margin_s", D::F(2.0)),
    ("debounce_polls", D::F(3.0)),
    ("agent_act_s", D::F(300.0)),
    ("agent_notify_s", D::F(900.0)),
    ("min_rate_mib_s", D::F(1.0)),
    ("poll_interval_s", D::F(1.0)),
    ("decl_margin_mib", D::I(3072)),
    ("decl_min_ceiling_mib", D::I(8192)),
    ("hard_floor_mib", D::F(5120.0)),
    ("abs_band_mib", D::I(10240)),
];

pub fn is_default_key(k: &str) -> bool {
    DEFAULTS.iter().any(|(n, _)| *n == k)
}

fn dv(d: D) -> PyValue {
    match d {
        D::F(x) => PyValue::Float(x),
        D::I(n) => PyValue::int(n),
    }
}

/// The integer value of a default (`int(DEFAULTS[k])`).
pub fn default_int(k: &str) -> i64 {
    match DEFAULTS.iter().find(|(n, _)| *n == k).map(|x| x.1) {
        Some(D::I(n)) => n,
        Some(D::F(x)) => x as i64,
        None => panic!("no default {k}"),
    }
}

/// `dict(DEFAULTS)`.
pub fn defaults() -> PyDict {
    let mut d = PyDict::new();
    for (k, v) in DEFAULTS {
        d.set(k, dv(v));
    }
    d
}

/// `p = dict(DEFAULTS); p.update({k: v for k, v in params.items() if k in DEFAULTS})`.
pub fn merged(params: Option<&PyDict>) -> PyDict {
    let mut p = defaults();
    if let Some(params) = params {
        for (k, v) in params.iter() {
            let key = py::text(k);
            if is_default_key(&key) {
                p.set(&key, v.clone());
            }
        }
    }
    p
}

fn exc(e: pynumber::PyException) -> Raised {
    Raised::new(e.class, &e.message)
}

/// `float(v)`.
pub fn fl(v: &PyValue) -> Result<f64, Raised> {
    pynumber::float_call(v.peel()).map_err(exc)
}

/// A number used directly in a comparison or arithmetic (int, float or bool).
pub fn num(v: &PyValue) -> Result<f64, Raised> {
    match v.peel() {
        PyValue::Int(_) | PyValue::Float(_) | PyValue::Bool(_) => fl(v),
        other => Err(Raised::new(
            "TypeError",
            &format!(
                "'<=' not supported between instances of 'float' and '{}'",
                other.type_name()
            ),
        )),
    }
}

fn g<'a>(p: &'a PyDict, k: &str) -> &'a PyValue {
    p.get(k).expect("merged params hold every default")
}

/// `runway_s(p)`: kill latency + detect margin + the (N-1) polls the debounce delays the kill.
pub fn runway_s(p: &PyDict) -> Result<f64, Raised> {
    Ok(fl(g(p, "kill_latency_s"))?
        + fl(g(p, "detect_margin_s"))?
        + (fl(g(p, "debounce_polls"))? - 1.0) * fl(g(p, "poll_interval_s"))?)
}

/// Python `a // b` for floats.
pub fn floordiv(a: f64, b: f64) -> f64 {
    let m = a % b;
    let mut div = (a - m) / b;
    if m != 0.0 && ((b < 0.0) != (m < 0.0)) {
        div -= 1.0;
    }
    if div != 0.0 {
        let f = div.floor();
        if div - f > 0.5 {
            f + 1.0
        } else {
            f
        }
    } else {
        0.0_f64.copysign(a / b)
    }
}

/// `max_rate_mib_s(mem_total_mib, params)`: the dual-rule switch rate `int(mem_total // runway)`; None when unknown.
pub fn max_rate_mib_s(mem_total_mib: Option<i64>, params: Option<&PyDict>) -> Result<Option<i64>, Raised> {
    let Some(mt) = mem_total_mib.filter(|m| *m > 0) else {
        return Ok(None);
    };
    let p = merged(params);
    Ok(Some(floordiv(mt as f64, runway_s(&p)?) as i64))
}

fn opt_int(v: Option<i64>) -> PyValue {
    v.map_or(PyValue::None, PyValue::int)
}
fn opt_f(v: Option<f64>) -> PyValue {
    v.map_or(PyValue::None, PyValue::Float)
}
pub fn round3(x: f64) -> f64 {
    pynumber::round_to_places(x, 3)
}

/// The verdict of `compute_eta`.
#[derive(Clone, Debug)]
pub struct Eta {
    pub max_rate_mib_s: Option<i64>,
    pub abs_band_mib: PyValue,
    pub eta_zero_s: Option<f64>,
    pub eta_actionable_s: Option<f64>,
    pub band: &'static str,
    pub trip: bool,
    pub reason: &'static str,
    pub rule: &'static str,
}

impl Eta {
    pub fn to_py(&self) -> PyValue {
        py::dict(vec![
            ("max_rate_mib_s", opt_int(self.max_rate_mib_s)),
            ("abs_band_mib", self.abs_band_mib.clone()),
            ("eta_zero_s", opt_f(self.eta_zero_s)),
            ("eta_actionable_s", opt_f(self.eta_actionable_s)),
            ("band", py::s(self.band)),
            ("trip", PyValue::Bool(self.trip)),
            ("reason", py::s(self.reason)),
            ("rule", py::s(self.rule)),
        ])
    }
}

/// `compute_eta(mem_avail_mib, rate_mib_s, params, mem_total_mib)` — pure. `rate` positive = falling.
pub fn compute_eta(
    mem: f64,
    rate: Option<f64>,
    params: Option<&PyDict>,
    mem_total_mib: Option<i64>,
) -> Result<Eta, Raised> {
    let p = merged(params);
    if mem < 0.0 {
        return Err(Raised::new("ValueError", "mem_avail_mib must be >= 0"));
    }
    let ceiling = max_rate_mib_s(mem_total_mib, Some(&p))?;
    let band_v = g(&p, "abs_band_mib").clone();
    let mk = |eta_zero_s, eta_actionable_s, band, trip, reason, rule| Eta {
        max_rate_mib_s: ceiling,
        abs_band_mib: band_v.clone(),
        eta_zero_s,
        eta_actionable_s,
        band,
        trip,
        reason,
        rule,
    };
    let min_rate = num(g(&p, "min_rate_mib_s"))?;
    if mem <= num(g(&p, "hard_floor_mib"))? {
        let ez = match rate {
            Some(r) if r != 0.0 && r >= min_rate => Some(round3(mem / r)),
            _ => None,
        };
        return Ok(mk(ez, None, "trip", true, "hard_floor", "hard_floor"));
    }
    // `not (rate < min_rate)` on purpose: a NaN rate goes on to the ETA rule, as in the original.
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    let r = match rate {
        Some(r) if !(r < min_rate) => r,
        _ => return Ok(mk(None, None, "green", false, "rate_below_min", "min_rate")),
    };
    let eta_zero = mem / r;
    if let Some(c) = ceiling {
        if r >= c as f64 {
            let trip = mem <= num(g(&p, "abs_band_mib"))?;
            return Ok(mk(
                Some(round3(eta_zero)),
                None,
                if trip { "trip" } else { "red" },
                trip,
                if trip { "abs_band" } else { "abs_band_hold" },
                "abs_band",
            ));
        }
    }
    let dm = num(g(&p, "detect_margin_s"))?;
    let eta_act = eta_zero - (runway_s(&p)? - dm);
    let trip = eta_act <= dm;
    let band = if trip {
        "trip"
    } else if eta_act <= num(g(&p, "agent_act_s"))? {
        "red"
    } else if eta_act <= num(g(&p, "agent_notify_s"))? {
        "amber"
    } else {
        "green"
    };
    Ok(mk(
        Some(round3(eta_zero)),
        Some(round3(eta_act)),
        band,
        trip,
        "ok",
        "eta",
    ))
}

/// `trip_threshold_mib(rate, params, mem_total_mib)`: the MemAvailable at which this rate trips.
pub fn trip_threshold_mib(
    rate: Option<f64>,
    params: Option<&PyDict>,
    mem_total_mib: Option<i64>,
) -> Result<Option<f64>, Raised> {
    let p = merged(params);
    let Some(r) = rate else {
        return Ok(None);
    };
    if r < num(g(&p, "min_rate_mib_s"))? {
        return Ok(None);
    }
    let ceiling = max_rate_mib_s(mem_total_mib, Some(&p))?;
    if let Some(c) = ceiling {
        if r >= c as f64 {
            return Ok(Some(fl(g(&p, "abs_band_mib"))?));
        }
    }
    Ok(Some(pynumber::round_to_places(r * runway_s(&p)?, 1)))
}

fn raw_get(params: &PyDict, k: &str) -> PyValue {
    params
        .get(k)
        .cloned()
        .unwrap_or_else(|| dv(DEFAULTS.iter().find(|(n, _)| *n == k).unwrap().1))
}

/// `validate_params(params)`: the floor guards. `(ok, reasons)`; never corrects a value silently.
pub fn validate_params(params: &PyDict) -> Result<(bool, Vec<String>), Raised> {
    let mut errs: Vec<String> = vec![];
    let p = merged(Some(params));
    let kl = fl(g(&p, "kill_latency_s"))?;
    let dm = fl(g(&p, "detect_margin_s"))?;
    let db = fl(g(&p, "debounce_polls"))?;
    let floor = kl * ETA_FLOOR_MULTIPLIER;
    let runway = runway_s(&p)?;
    if kl <= 0.0 {
        errs.push("kill_latency_s must be > 0".into());
    }
    if dm <= 0.0 {
        errs.push("detect_margin_s must be > 0".into());
    }
    let bad_db = if db < 1.0 {
        true
    } else {
        let t = pynumber::int_from_float(db).map_err(exc)?;
        let back = pynumber::float_from_int(&t).map_err(exc)?;
        db != back
    };
    if bad_db {
        errs.push(format!(
            "debounce_polls must be an integer >= 1 (got {})",
            fr(db)
        ));
    }
    if fl(g(&p, "poll_interval_s"))? <= 0.0 {
        errs.push("poll_interval_s must be > 0".into());
    }
    if runway < floor {
        errs.push(format!(
            "runway(kill_latency+detect_margin)={}s < floor(kill_latency*{})={}s",
            pct("%.2f", runway),
            pct("%.1f", ETA_FLOOR_MULTIPLIER),
            pct("%.2f", floor)
        ));
    }
    if fl(g(&p, "decl_margin_mib"))? < 0.0 {
        errs.push("decl_margin_mib must be >= 0".into());
    }
    if fl(g(&p, "decl_min_ceiling_mib"))? <= fl(g(&p, "hard_floor_mib"))? {
        errs.push(format!(
            "decl_min_ceiling_mib({}) must exceed hard_floor_mib({}) — 최소상한이 절대바닥 이하면 어떤 선언도 거부되지 않아 가드가 무력해진다",
            bb::s(g(&p, "decl_min_ceiling_mib")),
            bb::s(g(&p, "hard_floor_mib"))
        ));
    }
    if fl(g(&p, "abs_band_mib"))? <= fl(g(&p, "hard_floor_mib"))? {
        errs.push(format!(
            "abs_band_mib({}) must exceed hard_floor_mib({}) — 밴드가 절대바닥 이하면 이중규칙이 상한 위 구간을 조용히 무장해제한다",
            bb::s(g(&p, "abs_band_mib")),
            bb::s(g(&p, "hard_floor_mib"))
        ));
    }
    for k in ["agent_act_s", "agent_notify_s"] {
        if fl(&raw_get(params, k))? <= runway {
            errs.push(format!(
                "{k} must exceed runway {}s (에이전트 구간이 데몬 구간보다 안쪽일 수 없음)",
                pct("%.2f", runway)
            ));
        }
    }
    if fl(&raw_get(params, "agent_notify_s"))? <= fl(&raw_get(params, "agent_act_s"))? {
        errs.push("agent_notify_s must exceed agent_act_s".into());
    }
    Ok((errs.is_empty(), errs))
}

/// Python `round(x)` to an int (half to even).
fn round_int(x: f64) -> i64 {
    let r = x.round();
    if (x - x.trunc()).abs() == 0.5 {
        let t = x.trunc();
        if (t as i64) % 2 == 0 {
            t as i64
        } else {
            r as i64
        }
    } else {
        r as i64
    }
}

/// `emit_shell_params(params, path, mem_total_mib, mem_total_source)`: the constants the 1 s hot loop sources.
pub fn emit_shell_params(
    params: Option<&PyDict>,
    path: &str,
    mem_total_mib: Option<i64>,
    mem_total_source: Option<&str>,
) -> Result<i64, Raised> {
    let p = merged(params);
    let mut lines: Vec<String> = vec![
        "# generated by blackbox_eta.py -- 편집 금지(재생성으로 갱신)".into(),
        "# 산식: eta_actionable = mem_avail/rate - kill_latency ; TRIP <=> eta_actionable <= detect_margin".into(),
        "# 핫루프는 등가 형태를 정수로 쓴다: TRIP <=> mem_avail_mib*1000 <= rate_mib_s*RUNWAY_MS".into(),
        "# 이중규칙(C1-A): rate >= BB_MAX_RATE_MIB_S 이면 위 식 대신 TRIP <=> mem <= BB_ABS_BAND_MIB".into(),
    ];
    let runway_ms = round_int(runway_s(&p)? * 1000.0);
    let max_rate = max_rate_mib_s(mem_total_mib, Some(&p))?;
    let mut items: Vec<(String, PyValue)> =
        p.iter().map(|(k, v)| (py::text(k), v.clone())).collect();
    items.sort_by(|a, b| a.0.cmp(&b.0));
    for (k, v) in &items {
        lines.push(format!("BB_{}={}", k.to_uppercase(), bb::s(v)));
    }
    lines.push(format!("BB_RUNWAY_MS={runway_ms}"));
    let deb = pynumber::int_call(g(&p, "debounce_polls").peel()).map_err(exc)?;
    lines.push(format!("BB_DEBOUNCE_N={deb}"));
    lines.push(format!(
        "# mem_total 출처: {}",
        mem_total_source.filter(|s| !s.is_empty()).unwrap_or("unknown")
    ));
    lines.push(format!("BB_MEM_TOTAL_MIB={}", mem_total_mib.unwrap_or(0)));
    if max_rate.is_none() {
        lines.push("# ⚠ mem_total 미상 → 이중규칙 OFF. 상한 위 구간은 옛 규칙(무조건 트립)이다.".into());
    }
    lines.push(format!(
        "BB_MAX_RATE_MIB_S={}",
        max_rate.unwrap_or(MAX_RATE_DISABLED)
    ));
    bb::write_atomic(path, &(lines.join("\n") + "\n"))?;
    Ok(runway_ms)
}

/// `load_envelope(node_dir)`: `{}` when absent.
pub fn load_envelope(node_dir: &str) -> Result<PyValue, Raised> {
    let path = bb::join(node_dir, "envelope.json");
    if std::fs::metadata(&path).is_err() {
        return Ok(PyValue::Dict(PyDict::new()));
    }
    bb::load_json(&path)
}

/// `read_mem_total_mib()`: /proc/meminfo MemTotal in MiB; None on failure.
pub fn read_mem_total_mib() -> Option<i64> {
    read_mem_total_from("/proc/meminfo")
}

pub fn read_mem_total_from(path: &str) -> Option<i64> {
    let text = bb::read_text(path).ok()?;
    meminfo_mib(&text, "MemTotal")
}

/// One `/proc/meminfo` field (`<key>: <kB> kB`) in MiB from the file's text; None when the first `<key>:` line has no
/// integer value or there is no such line. The single parse of that file — the serving-budget co-resident stacking
/// (`blackbox_session`, plan_26111708 B L6) reads MemTotal/MemAvailable through it instead of a second parser.
pub fn meminfo_mib(text: &str, key: &str) -> Option<i64> {
    let head = format!("{key}:");
    for ln in bb::text_lines(text) {
        if ln.starts_with(&head) {
            let f = py::split_ws(ln);
            let v = bb::py_int_text(f.get(1)?)?;
            return Some((v as f64 / 1024.0) as i64);
        }
    }
    None
}

/// `read_csv_lines(path)`: raw, `.gz` or `.zst` samples as lines; None on failure.
pub fn read_csv_lines(path: &str) -> Option<Vec<String>> {
    if path.ends_with(".zst") {
        let out = crate::proc::run(
            &["zstd".to_string(), "-dc".to_string(), path.to_string()],
            None,
            Some(300),
        )
        .ok()?;
        if out.returncode != 0 {
            return None;
        }
        return Some(py::splitlines(&out.stdout));
    }
    let bytes = std::fs::read(path).ok()?;
    let data = if path.ends_with(".gz") {
        crate::gz::decompress(&bytes).ok()?
    } else {
        bytes
    };
    Some(py::splitlines(&bb::universal_newlines(
        &String::from_utf8_lossy(&data),
    )))
}

/// `resolve_mem_total(node_dir, explicit)`: (MiB, source) — explicit > /proc/meminfo > envelope.derived.
pub fn resolve_mem_total(node_dir: Option<&str>, explicit: Option<i64>) -> Result<(Option<i64>, String), Raised> {
    resolve_mem_total_with(node_dir, explicit, read_mem_total_mib())
}

pub fn resolve_mem_total_with(
    node_dir: Option<&str>,
    explicit: Option<i64>,
    measured: Option<i64>,
) -> Result<(Option<i64>, String), Raised> {
    if let Some(e) = explicit.filter(|e| *e != 0) {
        return Ok((Some(e), "explicit:--mem-total-mib".into()));
    }
    if let Some(mt) = measured.filter(|m| *m != 0) {
        return Ok((Some(mt), "measured:/proc/meminfo MemTotal".into()));
    }
    if let Some(nd) = node_dir.filter(|n| !n.is_empty()) {
        let env = load_envelope(nd)?;
        let d = match env.peel() {
            PyValue::Dict(_) => py::get(&env, "derived").cloned(),
            PyValue::List(l) if l.is_empty() => None,
            other if !other.is_truthy() => None,
            other => {
                return Err(Raised::new(
                    "AttributeError",
                    &format!("'{}' object has no attribute 'get'", other.type_name()),
                ))
            }
        };
        if let Some(d) = d.filter(|d| d.is_truthy()) {
            if let PyValue::Dict(_) = d.peel() {
                if let Some(mt) = py::get(&d, "mem_total_mib").filter(|v| v.is_truthy()) {
                    let n = pynumber::int_call(mt).map_err(exc)?;
                    let src = py::get(&d, "mem_total_source")
                        .filter(|v| v.is_truthy())
                        .map(bb::s)
                        .unwrap_or_else(|| "derived".into());
                    return Ok((Some(i64::try_from(n).unwrap_or(0)), format!("envelope:{src}")));
                }
            } else {
                return Err(Raised::new(
                    "AttributeError",
                    &format!("'{}' object has no attribute 'get'", d.type_name()),
                ));
            }
        }
    }
    Ok((
        None,
        "unknown:/proc/meminfo 판독 실패 · envelope.derived 부재".into(),
    ))
}

#[derive(Default, Clone)]
struct RuleState {
    streak: i64,
    kills: Vec<(String, f64, f64)>,
    skip_rate: bool,
    trip_polls: i64,
    max_streak: i64,
}

/// `replay_samples(path, params, mem_total_mib, arm_ceiling_mib)` as the original's result dict; None when unreadable.
pub fn replay_samples(
    path: &str,
    params: Option<&PyDict>,
    mem_total_mib: Option<i64>,
    arm_ceiling_mib: Option<i64>,
) -> Result<Option<PyValue>, Raised> {
    let Some(lines) = read_csv_lines(path) else {
        return Ok(None);
    };
    if lines.is_empty() {
        return Ok(None);
    }
    let header: Vec<&str> = lines[0].split(',').collect();
    let mut idx: std::collections::HashMap<&str, usize> = Default::default();
    for (i, k) in header.iter().enumerate() {
        idx.insert(k, i);
    }
    if !idx.contains_key("mem_avail") || !idx.contains_key("mem_rate") {
        return Ok(None);
    }
    let p = merged(params);
    let debounce = i64::try_from(pynumber::int_call(g(&p, "debounce_polls").peel()).map_err(exc)?)
        .unwrap_or(i64::MAX);
    let ceil_arm = match arm_ceiling_mib.filter(|a| *a != 0) {
        Some(a) => a as f64,
        None => f64::INFINITY,
    };
    let rules: [(&str, Option<i64>); 2] = [("eta_only", None), ("dual", mem_total_mib)];
    let mut state = [RuleState::default(), RuleState::default()];
    let mut rows = 0i64;
    let mut max_rate = 0.0f64;
    let mut min_mem: Option<f64> = None;
    let hard = num(g(&p, "hard_floor_mib"))?;
    for line in &lines[1..] {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < header.len() {
            continue;
        }
        let Ok(mem) = pynumber::float_from_str(&PyStr::new(parts[idx["mem_avail"]])) else {
            continue;
        };
        let rate = pynumber::float_from_str(&PyStr::new(parts[idx["mem_rate"]])).unwrap_or(0.0);
        rows += 1;
        if rate > max_rate {
            max_rate = rate;
        }
        min_mem = Some(match min_mem {
            None => mem,
            Some(m) => {
                if mem < m {
                    mem
                } else {
                    m
                }
            }
        });
        let ts = idx.get("ts").map(|i| parts[*i].to_string()).unwrap_or_default();
        for (k, (_, mt)) in rules.iter().enumerate() {
            let st = &mut state[k];
            let r = if st.skip_rate { 0.0 } else { rate };
            st.skip_rate = false;
            let trip = if mem > hard && mem > ceil_arm {
                false
            } else {
                compute_eta(mem, Some(r), Some(&p), *mt)?.trip
            };
            if trip {
                st.streak += 1;
                st.trip_polls += 1;
                st.max_streak = st.max_streak.max(st.streak);
                if st.streak >= debounce {
                    st.kills.push((ts.clone(), mem, rate));
                    st.streak = 0;
                    st.skip_rate = true;
                }
            } else {
                st.streak = 0;
            }
        }
    }
    let mut out = PyDict::new();
    out.set("path", py::s(path));
    out.set("rows", PyValue::int(rows));
    out.set("max_rate_mib_s", PyValue::Float(max_rate));
    out.set("min_mem_avail_mib", opt_f(min_mem));
    out.set(
        "max_rate_ceiling_mib_s",
        opt_int(max_rate_mib_s(mem_total_mib, Some(&p))?),
    );
    out.set("abs_band_mib", g(&p, "abs_band_mib").clone());
    out.set("runway_s", PyValue::Float(runway_s(&p)?));
    out.set(
        "arm_ceiling_mib",
        if ceil_arm.is_infinite() && ceil_arm > 0.0 {
            PyValue::None
        } else {
            PyValue::Float(ceil_arm)
        },
    );
    for (k, (name, _)) in rules.iter().enumerate() {
        let st = &state[k];
        let fk = st.kills.first().map_or(PyValue::None, |(ts, m, r)| {
            py::dict(vec![
                ("ts", py::s(ts)),
                ("mem_avail_mib", PyValue::Float(*m)),
                ("rate_mib_s", PyValue::Float(*r)),
            ])
        });
        out.set(
            name,
            py::dict(vec![
                ("kills", PyValue::int(st.kills.len() as i64)),
                ("first_kill", fk),
                ("trip_polls", PyValue::int(st.trip_polls)),
                ("max_streak", PyValue::int(st.max_streak)),
                ("debounce_polls", PyValue::int(debounce)),
            ]),
        );
    }
    Ok(Some(PyValue::Dict(out)))
}

/// `unknown_envelope_keys(envelope)`: eta_params keys outside DEFAULTS (silently dropped keys), sorted.
pub fn unknown_envelope_keys(envelope: Option<&PyValue>) -> Vec<String> {
    let Some(env) = envelope.filter(|e| e.is_truthy()) else {
        return vec![];
    };
    let Some(params) = py::get(env, "eta_params").filter(|v| v.is_truthy()) else {
        return vec![];
    };
    let PyValue::Dict(d) = params.peel() else {
        return vec![];
    };
    let mut v: Vec<String> = d
        .iter()
        .map(|(k, _)| py::text(k))
        .filter(|k| !is_default_key(k) && !ENVELOPE_META_KEYS.contains(&k.as_str()))
        .collect();
    v.sort();
    v
}

fn pd(pairs: &[(&str, PyValue)]) -> PyDict {
    let mut d = PyDict::new();
    for (k, v) in pairs {
        d.set(k, v.clone());
    }
    d
}
fn pf(x: f64) -> PyValue {
    PyValue::Float(x)
}

fn ce(mem: f64, rate: f64) -> Eta {
    compute_eta(mem, Some(rate), None, None).expect("compute_eta")
}
fn cem(mem: f64, rate: f64, mt: i64) -> Eta {
    compute_eta(mem, Some(rate), None, Some(mt)).expect("compute_eta")
}
fn vp(pairs: &[(&str, PyValue)]) -> (bool, Vec<String>) {
    validate_params(&pd(pairs)).expect("validate_params")
}

fn self_test() -> i32 {
    let mut checks: Vec<(String, bool)> = vec![];
    let mut c = |n: &str, b: bool| checks.push((n.to_string(), b));

    let r = ce(50000.0, 0.0);
    c(
        "정지 -> ETA None·green·무트립",
        r.eta_zero_s.is_none() && r.band == "green" && !r.trip,
    );
    let th = trip_threshold_mib(Some(6241.07), None, None).unwrap().unwrap();
    c("6.1GiB/s 트립 지점 ~49.9GiB(디바운스 포함)", (th - 49928.6).abs() < 1.0);
    let p1 = pd(&[("debounce_polls", pf(1.0))]);
    let th1 = trip_threshold_mib(Some(6241.07), Some(&p1), None).unwrap().unwrap();
    c("디바운스 1폴 -> 활주로 6s -> ~37.4GiB", (th1 - 37446.4).abs() < 1.0);
    c("디바운스↑ -> 트립 지점↑(지연분 보상)", th > th1);
    let r = ce(10186.0, 44.0);
    c("44MiB/s·10.2GiB -> 무트립(오발 제거)", !r.trip);
    c(
        "같은 지점 eta_zero ~231s",
        (r.eta_zero_s.unwrap() - 10186.0 / 44.0).abs() < 0.01,
    );
    let slow = trip_threshold_mib(Some(1000.0), Some(&pd(&[("kill_latency_s", pf(4.0))])), None)
        .unwrap()
        .unwrap();
    let fast = trip_threshold_mib(Some(1000.0), Some(&pd(&[("kill_latency_s", pf(10.0))])), None)
        .unwrap()
        .unwrap();
    c("kill 지연↑ -> 트립 지점↑(보수)", fast > slow);
    let r = ce(5000.0, 2000.0);
    c("2GiB/s·5GiB -> TRIP", r.trip && r.band == "trip");
    c("500s -> amber", ce(100000.0, 200.0).band == "amber");
    c("196s -> red", ce(100000.0, 500.0).band == "red");
    c("2000s -> green", ce(100000.0, 50.0).band == "green");
    let (ok, _) = vp(&[("kill_latency_s", pf(4.0)), ("detect_margin_s", pf(2.0))]);
    c("정상 파라미터 통과", ok);
    let (ok2, errs2) = vp(&[("kill_latency_s", pf(10.0)), ("detect_margin_s", pf(0.5))]);
    c(
        "runway < floor 거부",
        !ok2 && errs2.iter().any(|e| e.contains("floor")),
    );
    let (ok3, _) = vp(&[
        ("kill_latency_s", pf(4.0)),
        ("detect_margin_s", pf(2.0)),
        ("agent_act_s", pf(3.0)),
    ]);
    c("에이전트 구간이 데몬 안쪽이면 거부", !ok3);
    let (ok4, _) = vp(&[("agent_act_s", pf(900.0)), ("agent_notify_s", pf(300.0))]);
    c("notify <= act 거부", !ok4);
    let (ok5, errs5) = vp(&[("debounce_polls", pf(0.0))]);
    c(
        "디바운스 0폴 거부",
        !ok5 && errs5.iter().any(|e| e.contains("debounce")),
    );
    let (ok6, errs6) = vp(&[("debounce_polls", pf(2.5))]);
    c(
        "디바운스 비정수 거부",
        !ok6 && errs6.iter().any(|e| e.contains("debounce")),
    );
    let r = ce(4000.0, 0.2);
    c("느린 누수라도 바닥 밑이면 TRIP", r.trip && r.reason == "hard_floor");
    let r = ce(6000.0, 0.2);
    c("바닥 위 느린 하강은 무트립", !r.trip && r.reason == "rate_below_min");
    c("7/22 오발 지점(10,186)은 바닥 밖", !ce(10186.0, 44.0).trip);
    let r = ce(5120.0, 0.0);
    c("바닥 경계값 포함", r.trip && r.reason == "hard_floor");

    let td = bb::TempDir::new();
    let p = bb::join(td.path(), "eta_params.env");
    let runway_ms = emit_shell_params(Some(&PyDict::new()), &p, None, None).unwrap();
    let txt = std::fs::read_to_string(&p).unwrap_or_default();
    let floor = 5120.0;
    let mut agree = true;
    let mut dis: Vec<String> = vec![];
    for (mem, rate) in [
        (5000.0, 2000.0),
        (10186.0, 44.0),
        (37000.0, 6241.07),
        (120000.0, 1.0),
        (4000.0, 0.2),
        (6000.0, 0.2),
        (5120.0, 0.0),
        (49000.0, 6241.07),
    ] {
        let pyv = ce(mem, rate).trip;
        let sh = (mem <= floor) || ((mem * 1000.0) <= (rate * runway_ms as f64));
        if pyv != sh {
            agree = false;
            dis.push(format!(
                "({}, {}, {}, {})",
                mem as i64,
                fr(rate),
                if pyv { "True" } else { "False" },
                if sh { "True" } else { "False" }
            ));
        }
    }
    c(
        &format!(
            "셸 등가식 == 파이썬 판정 (8 케이스){}",
            if agree {
                String::new()
            } else {
                format!(" 불일치=[{}]", dis.join(", "))
            }
        ),
        agree,
    );
    let mt = 124610i64;
    let emit2 = bb::join(td.path(), "eta_params_dual.env");
    let runway_ms2 =
        emit_shell_params(Some(&PyDict::new()), &emit2, Some(mt), Some("explicit:self-test")).unwrap();
    let txt2 = std::fs::read_to_string(&emit2).unwrap_or_default();
    let sh_max = max_rate_mib_s(Some(mt), None).unwrap().unwrap();
    let band = 10240.0;
    c(
        "emit 에 BB_MAX_RATE_MIB_S 파생값",
        txt2.contains(&format!("BB_MAX_RATE_MIB_S={sh_max}\n")),
    );
    c(
        "emit 에 BB_ABS_BAND_MIB=10240(정수)",
        txt2.contains("BB_ABS_BAND_MIB=10240\n"),
    );
    c(
        "emit 에 BB_MEM_TOTAL_MIB 실값",
        txt2.contains(&format!("BB_MEM_TOTAL_MIB={mt}\n")),
    );
    let mut agree2 = true;
    let mut dis2: Vec<String> = vec![];
    let dual_cases: [(f64, f64); 15] = [
        (50772.0, 21943.0),
        (35158.0, 22799.0),
        (46641.0, 23313.5),
        (74181.0, 27893.0),
        (28686.0, 23162.0),
        (9000.0, 20000.0),
        (10240.0, 20000.0),
        (10241.0, 20000.0),
        (60000.0, sh_max as f64),
        (60000.0, (sh_max - 1) as f64),
        (60000.0, (sh_max + 1) as f64),
        (5000.0, 30000.0),
        (37000.0, 6241.07),
        (120000.0, 1.0),
        (10186.0, 44.0),
    ];
    for (mem, rate) in dual_cases {
        let pyv = cem(mem, rate, mt).trip;
        let sh = (mem <= floor)
            || (rate >= 1.0
                && (if sh_max != 0 && rate >= sh_max as f64 {
                    mem <= band
                } else {
                    (mem * 1000.0) <= (rate * runway_ms2 as f64)
                }));
        if pyv != sh {
            agree2 = false;
            dis2.push(format!(
                "({}, {}, {}, {})",
                mem as i64,
                fr(rate),
                if pyv { "True" } else { "False" },
                if sh { "True" } else { "False" }
            ));
        }
    }
    c(
        &format!(
            "★ 이중규칙 셸 등가식 == 파이썬 ({} 케이스){}",
            dual_cases.len(),
            if agree2 {
                String::new()
            } else {
                format!(" 불일치=[{}]", dis2.join(", "))
            }
        ),
        agree2,
    );
    let emit3 = bb::join(td.path(), "eta_params_nomt.env");
    emit_shell_params(Some(&PyDict::new()), &emit3, None, Some("unknown:self-test")).unwrap();
    let txt3 = std::fs::read_to_string(&emit3).unwrap_or_default();
    c(
        "mem_total 미상 → BB_MAX_RATE_MIB_S=0(이중규칙 OFF)",
        txt3.contains("BB_MAX_RATE_MIB_S=0\n") && txt3.contains("이중규칙 OFF"),
    );
    c(
        "mem_total 미상 → 판정이 옛 규칙과 동일(더 죽이는 쪽)",
        compute_eta(50772.0, Some(21943.0), None, None).unwrap().trip,
    );
    c("emit 파일에 RUNWAY_MS=8000", txt.contains("BB_RUNWAY_MS=8000"));
    c("emit 파일에 DEBOUNCE_N=3", txt.contains("BB_DEBOUNCE_N=3"));
    let (dm, dc) = (default_int("decl_margin_mib"), default_int("decl_min_ceiling_mib"));
    c(
        "emit 에 DECL_MARGIN_MIB=정본",
        txt.contains(&format!("BB_DECL_MARGIN_MIB={dm}\n")),
    );
    c(
        "emit 에 DECL_MIN_CEILING_MIB=정본",
        txt.contains(&format!("BB_DECL_MIN_CEILING_MIB={dc}\n")),
    );
    c(
        "선언 상수가 정수 표기",
        !txt.contains(&format!("BB_DECL_MARGIN_MIB={dm}.0"))
            && !txt.contains(&format!("BB_DECL_MIN_CEILING_MIB={dc}.0")),
    );
    let (bad, _) = vp(&[("decl_min_ceiling_mib", PyValue::int(5120))]);
    c("최소상한 <= 절대바닥 거부", !bad);
    let (bad2, _) = vp(&[("decl_margin_mib", PyValue::int(-1))]);
    c("음수 여유 거부", !bad2);
    drop(td);

    let seed_env = py::dict(vec![(
        "eta_params",
        py::dict(vec![
            ("agent_act_s", PyValue::int(300)),
            ("agent_notify_s", PyValue::int(900)),
            ("daemon_kill_s", PyValue::int(6)),
            ("eta_floor_s", pf(6.0)),
            ("provenance", py::s("plan_26073109 §2.2 초기 임의값")),
        ]),
    )]);
    c(
        "★ seed envelope 미지 키 = daemon_kill_s·eta_floor_s 2건",
        unknown_envelope_keys(Some(&seed_env)) == vec!["daemon_kill_s", "eta_floor_s"],
    );
    c(
        "eta_params.provenance 는 미지 키로 세지 않는다(메타)",
        !unknown_envelope_keys(Some(&seed_env)).contains(&"provenance".to_string()),
    );
    c(
        "정본 키만 있으면 미지 키 0",
        unknown_envelope_keys(Some(&py::dict(vec![(
            "eta_params",
            py::dict(vec![("kill_latency_s", pf(6.0))]),
        )])))
        .is_empty(),
    );
    c(
        "envelope 부재/빈 dict 도 안전",
        unknown_envelope_keys(Some(&PyValue::Dict(PyDict::new()))).is_empty()
            && unknown_envelope_keys(None).is_empty(),
    );
    let seed_keys = ["agent_act_s", "agent_notify_s", "daemon_kill_s", "eta_floor_s"];
    let mut inter: Vec<&str> = seed_keys
        .iter()
        .copied()
        .filter(|k| is_default_key(k))
        .collect();
    inter.sort();
    c(
        "seed 키의 정본 교집합은 알림 밴드 2종뿐",
        inter == vec!["agent_act_s", "agent_notify_s"],
    );
    let trip_keys = [
        "kill_latency_s",
        "detect_margin_s",
        "debounce_polls",
        "poll_interval_s",
        "min_rate_mib_s",
        "hard_floor_mib",
    ];
    c(
        "★ seed 키 중 트립 판정에 쓰이는 키는 0개",
        !seed_keys.iter().any(|k| trip_keys.contains(k)),
    );

    let mt = 124610i64;
    let ceil = max_rate_mib_s(Some(mt), None).unwrap();
    c("상한이 mem_total/runway 에서 파생(15,576)", ceil == Some(15576));
    c(
        "상한이 mem_total 변경을 따라간다(하드코딩 아님)",
        max_rate_mib_s(Some(mt / 2), None).unwrap() == Some(7788),
    );
    c(
        "kill 지연이 늘면 상한이 내려간다(runway 종속)",
        max_rate_mib_s(Some(mt), Some(&pd(&[("kill_latency_s", pf(6.0))]))).unwrap()
            == Some(12461),
    );
    c(
        "mem_total 미상 → 상한 None(이중규칙 OFF)",
        max_rate_mib_s(None, None).unwrap().is_none()
            && max_rate_mib_s(Some(0), None).unwrap().is_none(),
    );
    for (mem, rate, tag) in [(50772.0, 21943.0, "R0 시도①"), (35158.0, 22799.0, "R0 시도②")] {
        let r = cem(mem, rate, mt);
        c(
            &format!(
                "★ {tag} 사살점({}@{}) → 이중규칙 미발동",
                mem as i64, rate as i64
            ),
            !r.trip && r.rule == "abs_band",
        );
        c("  같은 점이 옛 규칙에서는 발동했다(대조)", ce(mem, rate).trip);
    }
    c(
        "★ R0 시도③ 반사실 지점(46,641@23,313) → 이중규칙 미발동",
        !cem(46641.0, 23313.5, mt).trip,
    );
    c(
        "상한 위·밴드 경계값(10,240) 포함 → 발동",
        cem(10240.0, 20000.0, mt).trip,
    );
    c(
        "상한 위·밴드 위 1MiB → 미발동",
        !cem(10241.0, 20000.0, mt).trip,
    );
    c(
        "상한 위·바닥 밑 → hard_floor 로 발동(밴드와 무관하게 항상 무장)",
        cem(5000.0, 30000.0, mt).rule == "hard_floor",
    );
    for (mem, rate) in [
        (37000.0, 6241.07),
        (10186.0, 44.0),
        (120000.0, 1.0),
        (5000.0, 2000.0),
    ] {
        c(
            &format!(
                "상한 아래 회귀({}@{}) — 옛 판정과 동일",
                mem as i64,
                pct("%g", rate)
            ),
            cem(mem, rate, mt).trip == ce(mem, rate).trip,
        );
    }
    let balloon = [(74181.0, 27893.0), (51848.0, 22333.0), (28686.0, 23162.0)];
    c(
        "⚠ 진성 KV벌룬은 이중규칙에서 **미발동**(설계상 포기 · 방어는 선언/earlyoom)",
        balloon.iter().all(|(m, r)| !cem(*m, *r, mt).trip),
    );
    c(
        "  같은 궤적이 옛 규칙에서는 전부 발동했다(무엇을 포기했는지 대조)",
        balloon.iter().all(|(m, r)| ce(*m, *r).trip),
    );
    c(
        "  벌룬이 밴드까지 내려오면 되찾는다(10,000@23,162 → 발동)",
        cem(10000.0, 23162.0, mt).trip,
    );
    c(
        "상한 위 역산 = 절대 밴드",
        trip_threshold_mib(Some(20000.0), None, Some(mt)).unwrap() == Some(10240.0),
    );
    c(
        "상한 아래 역산 = rate x runway(기존)",
        trip_threshold_mib(Some(1000.0), None, Some(mt)).unwrap() == Some(8000.0),
    );
    let (bad_band, errs_b) = vp(&[("abs_band_mib", PyValue::int(5120))]);
    c(
        "밴드 <= 절대바닥 거부(조용한 무장해제 방지)",
        !bad_band && errs_b.iter().any(|e| e.contains("abs_band_mib")),
    );
    c(
        "정상 밴드 통과",
        vp(&[("abs_band_mib", PyValue::int(10240))]).0,
    );

    let legacy = bb::join(
        &bb::dirname(&crate::owner::owner_path("node_blackbox/blackbox_eta.py").display().to_string()),
        "../host_safety/mem_watchdog.sh",
    );
    if bb::is_file(&legacy) {
        let t = std::fs::read(&legacy).unwrap_or_default();
        let t = String::from_utf8_lossy(&t);
        c(
            "★ 레거시 워치독 절대임계와 abs_band_mib 일치(교차검증)",
            t.contains(&format!(
                "THRESH_MIB=\"${{2:-{}}}\"",
                default_int("abs_band_mib")
            )),
        );
    } else {
        println!(
            "  [INFO] 레거시 워치독 부재({legacy}) — 교차검증 생략. abs_band_mib 의 정본이 이 파일 하나뿐인지 확인하라."
        );
    }

    let td2 = bb::TempDir::new();
    let hdr = "ts,mem_avail,mem_rate,gpu_temp,gpu_pwr,gpu_sm,gpu_util,gpu_mem,load1,ctr_n\n";
    let write = |name: &str, rows: &[(f64, f64)]| -> String {
        let p = bb::join(td2.path(), name);
        let mut body = hdr.to_string();
        for (i, (m, r)) in rows.iter().enumerate() {
            body.push_str(&format!(
                "{},{},{},50,10,1,0,,0.5,1\n",
                1000 + i,
                pct("%g", *m),
                pct("%g", *r)
            ));
        }
        std::fs::write(&p, body).unwrap();
        p
    };
    let kills = |r: &PyValue, rule: &str| -> i64 {
        py::get(py::get(r, rule).unwrap(), "kills")
            .and_then(|v| match v {
                PyValue::Int(n) => i64::try_from(n.clone()).ok(),
                _ => None,
            })
            .unwrap_or(-1)
    };
    let load = write(
        "load.csv",
        &[
            (112195.0, 0.0),
            (90000.0, 22195.0),
            (68000.0, 22000.0),
            (46000.0, 22000.0),
            (24000.0, 22000.0),
            (24100.0, -100.0),
            (24100.0, 0.0),
        ],
    );
    let r = replay_samples(&load, None, Some(mt), None).unwrap().unwrap();
    c(
        "★ 재생: 대형 로드 — 옛 규칙 kill>=1 · 이중규칙 kill=0",
        kills(&r, "eta_only") >= 1 && kills(&r, "dual") == 0,
    );
    c(
        "  재생 요약이 상한·밴드를 함께 보고(출처 표시)",
        py::get(&r, "max_rate_ceiling_mib_s") == Some(&opt_int(ceil))
            && py::get(&r, "abs_band_mib") == Some(&PyValue::int(10240)),
    );
    let thru = write(
        "through.csv",
        &[
            (30000.0, 0.0),
            (20000.0, 20000.0),
            (10000.0, 20000.0),
            (9000.0, 20000.0),
            (8000.0, 20000.0),
        ],
    );
    let r2 = replay_samples(&thru, None, Some(mt), None).unwrap().unwrap();
    c("재생: 밴드 관통 → 이중규칙도 kill>=1", kills(&r2, "dual") >= 1);
    let idle = write(
        "idle.csv",
        &[
            (100000.0, 0.0),
            (99990.0, 10.0),
            (99980.0, 10.0),
            (99970.0, 10.0),
        ],
    );
    let r3 = replay_samples(&idle, None, Some(mt), None).unwrap().unwrap();
    c(
        "재생: 유휴 궤적 — 두 규칙 모두 kill=0",
        kills(&r3, "eta_only") == 0 && kills(&r3, "dual") == 0,
    );
    let two = write(
        "two.csv",
        &[
            (30000.0, 0.0),
            (18000.0, 12000.0),
            (30000.0, -12000.0),
            (30000.0, 0.0),
        ],
    );
    let r4 = replay_samples(&two, None, Some(mt), None).unwrap().unwrap();
    c(
        "재생: 2폴만 지속 → 미발동(디바운스 보존)",
        kills(&r4, "eta_only") == 0 && kills(&r4, "dual") == 0,
    );
    let r5 = replay_samples(&load, None, Some(mt), Some(32768)).unwrap().unwrap();
    c(
        "재생: arm 상한 적용 시 옛 규칙도 kill=0(선언 효과 재현)",
        kills(&r5, "eta_only") == 0,
    );

    {
        use std::os::unix::fs::MetadataExt;
        let d = bb::TempDir::new();
        let target = format!("{}/proj/docs/logs/sub/samples", d.path());
        let made = bb::makedirs_as_ancestor_owner(&target).unwrap_or_default();
        c(
            "없는 체인을 전부 만든다",
            made.len() == 5 && bb::is_dir(&target),
        );
        let uid = |p: &str| std::fs::metadata(p).map(|m| m.uid()).unwrap_or(u32::MAX);
        c(
            "만든 경로가 조상 소유자를 물려받는다",
            uid(&format!("{}/proj", d.path())) == uid(d.path()),
        );
        c(
            "멱등 — 이미 있으면 신규 0건",
            bb::makedirs_as_ancestor_owner(&target)
                .map(|v| v.is_empty())
                .unwrap_or(false),
        );
        let pre = format!("{}/preexisting", d.path());
        let _ = std::fs::create_dir_all(&pre);
        let before = uid(&pre);
        let _ = bb::makedirs_as_ancestor_owner(&format!("{pre}/child"));
        c("기존 경로의 소유권은 변경하지 않는다", uid(&pre) == before);
        c(
            "비-root 에서도 예외 없이 동작",
            bb::is_dir(&format!("{pre}/child")),
        );
        let f = format!("{pre}/child/x.csv");
        let _ = std::fs::write(&f, "");
        let rr = bb::inherit_dir_owner(&f, &bb::dirname(&f));
        c(
            "파일 소유 상속: 비-root 는 no-op(False) · 파일 보존",
            !rr && bb::is_file(&f),
        );
        c(
            "파일 소유 상속: 없는 파일도 예외 없이 False",
            !bb::inherit_dir_owner(&format!("{pre}/none.csv"), &pre),
        );
        c(
            "재생: 판독 불가 경로는 None(조용한 0 아님)",
            replay_samples(&bb::join(td2.path(), "없다.csv"), None, None, None)
                .map(|r| r.is_none())
                .unwrap_or(false),
        );
    }
    drop(td2);

    let ok_all = bb::print_checks(&checks);
    println!(
        "self-test: {} ({} 케이스)",
        if ok_all { "PASS" } else { "FAIL" },
        checks.len()
    );
    if ok_all {
        0
    } else {
        2
    }
}

const A_NODE: usize = 1;
const A_EMIT: usize = 2;
const A_SET: usize = 3;
const A_ALLOW: usize = 4;
const A_EXPLAIN: usize = 5;
const A_MT: usize = 6;
const A_REPLAY: usize = 7;
const A_ARM: usize = 8;
const A_SELF: usize = 9;

static ARGS: [Arg; 10] = [
    HELP,
    store(&["--node-dir"]),
    store(&["--emit-params"]),
    store(&["--set"]).append(),
    flag(&["--allow-unknown-envelope-keys"]),
    store(&["--explain"]).float().nargs(2),
    store(&["--mem-total-mib"]).int(),
    store(&["--replay"]).nargs_plus(),
    store(&["--replay-arm-ceiling"]).int(),
    flag(&["--self-test"]),
];

pub static SPEC: Spec = Spec {
    prog: "blackbox_eta.py",
    usage: include_str!("../help/blackbox_eta.usage"),
    help: include_str!("../help/blackbox_eta.help"),
    args: &ARGS,
    subs: None,
};

fn bool_s(b: bool) -> &'static str {
    if b {
        "True"
    } else {
        "False"
    }
}

fn run(a: &argp::Parsed) -> R<i32> {
    if a.flag(A_SELF) {
        return Ok(self_test());
    }
    let mut params = PyDict::new();
    let node_dir = a.text(A_NODE).filter(|s| !s.is_empty());
    if let Some(nd) = node_dir {
        let env = load_envelope(nd)?;
        let unknown = unknown_envelope_keys(Some(&env));
        if !unknown.is_empty() && !a.flag(A_ALLOW) {
            let mut e = std::io::stderr().lock();
            let all: Vec<&str> = {
                let mut v: Vec<&str> = DEFAULTS.iter().map(|x| x.0).collect();
                v.sort();
                v
            };
            let _ = writeln!(
                e,
                "[eta] envelope.eta_params 에 정본에 없는 키가 있다 — 거부(조용히 버리지 않는다):"
            );
            for k in &unknown {
                let _ = writeln!(e, "  - {} (가능: {})", k, all.join(", "));
            }
            let _ = writeln!(e, "  해소: regen_envelope.py regen 으로 재생성하거나, 정본 키로 이관하라. (임시 통과는 --allow-unknown-envelope-keys)");
            return Ok(1);
        }
        if !unknown.is_empty() {
            eprintln!(
                "[eta] ⚠ 미지 키 {} 를 무시하고 진행(--allow-unknown-envelope-keys)",
                unknown.join(", ")
            );
        }
        let ep = match env.peel() {
            PyValue::Dict(_) => py::get(&env, "eta_params").cloned(),
            other => {
                return Err(Fail::Raise(Raised::new(
                    "AttributeError",
                    &format!("'{}' object has no attribute 'get'", other.type_name()),
                )))
            }
        };
        if let Some(ep) = ep.filter(|v| v.is_truthy()) {
            match ep.peel() {
                PyValue::Dict(d) => {
                    for (k, v) in d.iter() {
                        let key = py::text(k);
                        if is_default_key(&key) {
                            params.set(&key, v.clone());
                        }
                    }
                }
                other => {
                    return Err(Fail::Raise(Raised::new(
                        "AttributeError",
                        &format!("'{}' object has no attribute 'items'", other.type_name()),
                    )))
                }
            }
        }
    }
    for kv in a.many(A_SET) {
        let Some((k, v)) = kv.split_once('=') else {
            return Err(bb::ap_error(&SPEC, &format!("--set 는 K=V 형식: {}", repr(kv))));
        };
        if !is_default_key(k) {
            let mut all: Vec<&str> = DEFAULTS.iter().map(|x| x.0).collect();
            all.sort();
            return Err(bb::ap_error(
                &SPEC,
                &format!(
                    "알 수 없는 파라미터 {} (가능: {})",
                    repr(k),
                    all.join(", ")
                ),
            ));
        }
        let x = pynumber::float_from_str(&PyStr::new(v)).map_err(exc)?;
        params.set(k, PyValue::Float(x));
    }
    let (ok, errs) = validate_params(&params)?;
    if !ok {
        let mut e = std::io::stderr().lock();
        let _ = writeln!(e, "[eta] 하한 가드 위반 — 거부:");
        for x in errs {
            let _ = writeln!(e, "  - {x}");
        }
        return Ok(1);
    }
    let (mem_total, mt_src) = resolve_mem_total(node_dir, a.int(A_MT))?;

    if let Some(ex) = a.floats(A_EXPLAIN) {
        let (mem, rate) = (ex[0], ex[1]);
        let r = compute_eta(mem, Some(rate), Some(&params), mem_total)?;
        let mut eff = defaults();
        for (k, v) in params.iter() {
            eff.set(&py::text(k), v.clone());
        }
        let doc = py::dict(vec![
            (
                "input",
                py::dict(vec![("mem_avail_mib", pf(mem)), ("rate_mib_s", pf(rate))]),
            ),
            ("params", PyValue::Dict(eff)),
            ("result", r.to_py()),
            ("mem_total_mib", opt_int(mem_total)),
            ("mem_total_source", py::s(&mt_src)),
            (
                "max_rate_mib_s",
                opt_int(max_rate_mib_s(mem_total, Some(&params))?),
            ),
            (
                "trip_threshold_mib_at_this_rate",
                opt_f(trip_threshold_mib(Some(rate), Some(&params), mem_total)?),
            ),
        ]);
        println!("{}", py::dumps_pretty(&doc));
        return Ok(0);
    }

    let replay = a.many(A_REPLAY);
    if !replay.is_empty() {
        let mut out: Vec<PyValue> = vec![];
        let geti = |v: Option<&PyValue>| -> i64 {
            match v.map(|x| x.peel()) {
                Some(PyValue::Int(n)) => i64::try_from(n.clone()).unwrap_or(0),
                _ => 0,
            }
        };
        for path in replay {
            let r = replay_samples(path, Some(&params), mem_total, a.int(A_ARM))?;
            let Some(r) = r else {
                eprintln!("[eta] ⚠ 재생 불가(판독 실패 또는 컬럼 부재): {path}");
                out.push(py::dict(vec![("path", py::s(path)), ("error", py::s("unreadable"))]));
                continue;
            };
            let dual = py::get(&r, "dual").unwrap().clone();
            let fk = py::get(&dual, "first_kill").cloned().unwrap_or(PyValue::None);
            let maxr = fl(py::get(&r, "max_rate_mib_s").unwrap())?;
            let minm = match py::get(&r, "min_mem_avail_mib") {
                Some(PyValue::Float(x)) if *x != 0.0 => *x,
                _ => 0.0,
            };
            let tail = if matches!(fk, PyValue::None) {
                String::new()
            } else {
                format!(
                    "  ← 최초 {} mem={} rate={}",
                    bb::s(py::get(&fk, "ts").unwrap()),
                    bb::s(py::get(&fk, "mem_avail_mib").unwrap()),
                    bb::s(py::get(&fk, "rate_mib_s").unwrap())
                )
            };
            println!(
                "[eta] {} rows={} maxrate={} minmem={} | ETA전용 kill={} · 이중규칙 kill={}{}",
                bb::basename(path),
                geti(py::get(&r, "rows")),
                pct("%.0f", maxr),
                pct("%.0f", minm),
                geti(py::get(py::get(&r, "eta_only").unwrap(), "kills")),
                geti(py::get(&dual, "kills")),
                tail
            );
            out.push(r);
        }
        let tot = |rule: &str| -> i64 {
            out.iter()
                .map(|x| py::get(x, rule).map_or(0, |d| geti(py::get(d, "kills"))))
                .sum()
        };
        let (tot_e, tot_d) = (tot("eta_only"), tot("dual"));
        let mr = max_rate_mib_s(mem_total, Some(&params))?;
        println!(
            "[eta] 합계: ETA전용 kill={} · 이중규칙 kill={} (mem_total={} · 상한={} MiB/s · {})",
            tot_e,
            tot_d,
            mem_total.map_or("None".into(), |m| m.to_string()),
            mr.map_or("None".into(), |m| m.to_string()),
            mt_src
        );
        let doc = py::dict(vec![
            ("files", PyValue::List(out)),
            ("total_eta_only_kills", PyValue::int(tot_e)),
            ("total_dual_kills", PyValue::int(tot_d)),
            ("mem_total_mib", opt_int(mem_total)),
            ("mem_total_source", py::s(&mt_src)),
            ("max_rate_mib_s", opt_int(mr)),
        ]);
        println!("{}", py::dumps_pretty(&doc));
        return Ok(0);
    }

    if let Some(path) = a.text(A_EMIT).filter(|s| !s.is_empty()) {
        let runway_ms = emit_shell_params(Some(&params), path, mem_total, Some(&mt_src))?;
        let mr = max_rate_mib_s(mem_total, Some(&params))?;
        println!(
            "[eta] emit {} (runway={}ms · mem_total={}[{}] · 이중규칙 상한={})",
            path,
            runway_ms,
            mem_total.map_or("None".into(), |m| m.to_string()),
            mt_src,
            match mr {
                Some(m) if m != 0 => format!("{m} MiB/s"),
                _ => "OFF(미상)".into(),
            }
        );
        if mr.is_none() {
            eprintln!("[eta] ⚠ mem_total 미상 → 이중규칙 OFF. 상한 위 구간은 옛 규칙(무조건 트립)이다.");
        }
        return Ok(0);
    }
    let _ = bool_s;
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
