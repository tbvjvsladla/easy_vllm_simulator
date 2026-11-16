//! `regen_envelope.py` — regenerate `envelope.json` from rollups + 1 Hz samples and enforce key consistency with
//! `blackbox_eta::DEFAULTS` (plan_26081415 §3 C2). Same sub-commands (`regen`, `check-keys`, `staleness`), flags, exit
//! codes, texts and files as the original. The key list, `runway_s`, `max_rate_mib_s`, `read_csv_lines` and the
//! /proc/meminfo reader are `blackbox_eta`'s (derived, never copied). Times are `--now` only; raw samples are read only.
//!
//! Exit codes: 0 = success · 1 = argument/input refused · 2 = self-test failed · 3 = key consistency violated.

use crate::argp::{self, flag, store, Arg, Outcome, Parsed, Spec, HELP};
use crate::bb::{self, repr, Fail, R};
use crate::blackbox_eta::{self as eta, num};
use crate::py;
use crate::pyre::Regex;
use crate::raise::Raised;
use agent_runtime_support::pynumber;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::io::Write;

pub const SCHEMA_VERSION: i64 = 2;
const PASSTHROUGH_DEFAULT_KEYS: [&str; 2] = ["agent_act_s", "agent_notify_s"];
const NODE_ID_PATTERN: &str = r"^[a-z][a-z0-9-]{0,31}$";

fn iso_ok(s: &str) -> bool {
    Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$").match_(s).is_some()
}

fn parse_now(value: Option<&str>) -> R<String> {
    match value {
        Some(v) if iso_ok(v) => Ok(v.to_string()),
        Some(v) => Err(Fail::Exit(format!(
            "--now 는 YYYY-MM-DDTHH:MM:SSZ 형식이어야 한다: {}",
            repr(v)
        ))),
        None => Err(Fail::Exit(
            "--now 는 YYYY-MM-DDTHH:MM:SSZ 형식이어야 한다: None".into(),
        )),
    }
}

fn epoch(iso: &str) -> Result<i64, Raised> {
    crate::pytime::strptime_checked(iso, "%Y-%m-%dT%H:%M:%SZ")
        .map(|d| d.epoch())
        .map_err(|m| Raised::new("ValueError", &m))
}

fn num_opt(s: &str) -> Option<f64> {
    pynumber::float_from_str(&PyStr::new(s)).ok()
}

/// `_pct(sorted_vals, q)`.
fn pct(sorted_vals: &[f64], q: f64) -> PyValue {
    if sorted_vals.is_empty() {
        return PyValue::None;
    }
    let n = sorted_vals.len();
    let r = (q * (n - 1) as f64).round_ties_even() as i64;
    let i = (n as i64 - 1).min(r.max(0)) as usize;
    PyValue::Float(pynumber::round_to_places(sorted_vals[i], 3))
}

fn sample_path(node_dir: &str, day: &str) -> Option<String> {
    for ext in [".csv", ".csv.zst", ".csv.gz"] {
        let p = bb::join(&bb::join(node_dir, "samples"), &format!("{day}{ext}"));
        if bb::is_file(&p) {
            return Some(p);
        }
    }
    None
}

struct Day {
    mem: Vec<f64>,
    falling: Vec<f64>,
    n: i64,
    absent: Vec<String>,
}

fn scan_samples_day(path: &str) -> Option<Day> {
    let lines = eta::read_csv_lines(path)?;
    if lines.is_empty() {
        return None;
    }
    let header: Vec<&str> = lines[0].split(',').collect();
    // `{k: i for i, k in enumerate(header)}`: insertion order of first appearance, last index wins
    let mut idx: Vec<(String, usize)> = vec![];
    for (i, k) in header.iter().enumerate() {
        match idx.iter_mut().find(|x| x.0 == *k) {
            Some(x) => x.1 = i,
            None => idx.push((k.to_string(), i)),
        }
    }
    let get = |k: &str| idx.iter().find(|x| x.0 == k).map(|x| x.1);
    let (Some(im), Some(ir)) = (get("mem_avail"), get("mem_rate")) else {
        return None;
    };
    let (mut mem, mut falling, mut n) = (vec![], vec![], 0i64);
    let mut present: Vec<&str> = vec![];
    for line in &lines[1..] {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < header.len() {
            continue;
        }
        n += 1;
        if let Some(v) = num_opt(parts[im]) {
            mem.push(v);
        }
        if let Some(r) = num_opt(parts[ir]) {
            if r > 0.0 {
                falling.push(r);
            }
        }
        for (k, i) in &idx {
            if *i < parts.len() && !parts[*i].is_empty() && !present.contains(&k.as_str()) {
                present.push(k);
            }
        }
    }
    let mut absent: Vec<String> = idx
        .iter()
        .map(|x| x.0.clone())
        .filter(|k| !present.contains(&k.as_str()))
        .collect();
    absent.sort();
    Some(Day {
        mem,
        falling,
        n,
        absent,
    })
}

fn attr(v: &PyValue) -> Raised {
    Raised::new(
        "AttributeError",
        &format!("'{}' object has no attribute 'get'", v.type_name()),
    )
}

/// `r.get(k)` on a decoded JSON record (a non-dict record raises like Python's attribute lookup).
fn rget<'a>(r: &'a PyValue, k: &str) -> Result<Option<&'a PyValue>, Raised> {
    match r.peel() {
        PyValue::Dict(_) => Ok(py::get(r, k)),
        _ => Err(attr(r)),
    }
}

fn str_of(v: Option<&PyValue>) -> Option<String> {
    match v.map(|x| x.peel()) {
        Some(PyValue::Str(s)) => Some(py::text(s)),
        _ => None,
    }
}

/// `load_events(node_dir)`: every `events/*.jsonl`, de-duplicated on (ts, kind, targets|target), sorted by ts.
pub fn load_events(node_dir: &str) -> Result<Vec<PyValue>, Raised> {
    let d = bb::join(node_dir, "events");
    let mut paths: Vec<String> = match std::fs::read_dir(&d) {
        Ok(rd) => rd
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| !n.starts_with('.') && n.ends_with(".jsonl"))
            .map(|n| bb::join(&d, &n))
            .collect(),
        Err(_) => vec![],
    };
    paths.sort();
    let mut seen: Vec<String> = vec![];
    let mut rows: Vec<PyValue> = vec![];
    for p in paths {
        let Ok(bytes) = std::fs::read(&p) else {
            continue;
        };
        let text = std::str::from_utf8(&bytes)
            .map_err(|e| Raised::new("UnicodeDecodeError", &e.to_string()))?;
        let text = bb::universal_newlines(text);
        for ln in bb::text_lines(&text) {
            let ln = py::strip(ln);
            if ln.is_empty() {
                continue;
            }
            let Ok(r) = agent_runtime_support::pyjson::loads(ln) else {
                continue;
            };
            let tgt = match rget(&r, "targets")?.filter(|v| v.is_truthy()) {
                Some(v) => bb::s(v),
                None => match rget(&r, "target")?.filter(|v| v.is_truthy()) {
                    Some(v) => bb::s(v),
                    None => String::new(),
                },
            };
            let key = format!(
                "{}\u{0}{}\u{0}{}",
                rget(&r, "ts")?.map_or("None".into(), |v| v.repr()),
                rget(&r, "kind")?.map_or("None".into(), |v| v.repr()),
                tgt
            );
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);
            rows.push(r);
        }
    }
    rows.sort_by(|a, b| {
        let ka = str_of(py::get(a, "ts")).unwrap_or_default();
        let kb = str_of(py::get(b, "ts")).unwrap_or_default();
        ka.cmp(&kb)
    });
    Ok(rows)
}

/// `measure_kill_latency(events, since_utc)`: seconds from `watchdog_trip` to the next `watchdog_kill_ack`.
pub fn measure_kill_latency(events: &[PyValue], since: Option<&str>) -> Result<Vec<f64>, Raised> {
    let mut obs = vec![];
    let mut pending: Option<String> = None;
    for r in events {
        let Some(ts) = str_of(py::get(r, "ts")).filter(|t| !t.is_empty()) else {
            continue;
        };
        if !iso_ok(&ts) {
            continue;
        }
        if let Some(s) = since.filter(|s| !s.is_empty()) {
            if ts.as_str() < s {
                continue;
            }
        }
        let kind = str_of(py::get(r, "kind")).unwrap_or_default();
        if kind == "watchdog_trip" {
            pending = Some(ts);
        } else if kind == "watchdog_kill_ack" {
            if let Some(p) = pending.take() {
                let d = epoch(&ts)? - epoch(&p)?;
                if (0..=120).contains(&d) {
                    obs.push(d as f64);
                }
            }
        }
    }
    Ok(obs)
}

/// Python `max(values, default=None)` / `min(...)` over numbers, keeping the first extreme (its own int/float type).
fn extreme(values: &[PyValue], want_max: bool) -> Result<Option<PyValue>, Raised> {
    let mut best: Option<(f64, PyValue)> = None;
    for v in values {
        let x = num(v)?;
        match &best {
            None => best = Some((x, v.clone())),
            Some((b, _)) => {
                if (want_max && x > *b) || (!want_max && x < *b) {
                    best = Some((x, v.clone()));
                }
            }
        }
    }
    Ok(best.map(|b| b.1))
}

fn sub_get(s: &PyValue, outer: &str, inner: &str) -> Result<Option<PyValue>, Raised> {
    let o = match rget(s, outer)? {
        None => return Ok(None),
        Some(v) => v,
    };
    match o.peel() {
        PyValue::Dict(_) => Ok(py::get(o, inner)
            .filter(|v| !matches!(v, PyValue::None))
            .cloned()),
        _ => Err(attr(o)),
    }
}

fn d(pairs: Vec<(&str, PyValue)>) -> PyValue {
    py::dict(pairs)
}
fn strs(v: &[String]) -> PyValue {
    py::list_str(v)
}
fn floats(v: &[f64]) -> PyValue {
    PyValue::List(v.iter().map(|x| PyValue::Float(*x)).collect())
}
fn opt(v: Option<PyValue>) -> PyValue {
    v.unwrap_or(PyValue::None)
}

/// `build_envelope(node_id, node_dir, now_utc, window_days, mem_total_mib, mem_total_source)`.
pub fn build_envelope(
    node_id: &str,
    node_dir: &str,
    now_utc: &str,
    window_days: i64,
    mem_total_mib: Option<i64>,
    mem_total_source: &str,
) -> Result<PyValue, Raised> {
    let day0 = epoch(now_utc)?;
    let mut days: Vec<String> = (0..window_days.max(0))
        .map(|i| crate::pytime::DateTime::from_epoch(day0 - i * 86400).date_iso())
        .collect();
    days.reverse();
    let (mut mem_all, mut falling_all) = (vec![], vec![]);
    let (mut from_samples, mut from_rollup, mut missing) = (vec![], vec![], vec![]);
    let mut rollup_only: Vec<PyValue> = vec![];
    let mut absent_union: Vec<String> = vec![];
    let mut n_samples: i64 = 0;
    let (mut first_day, mut last_day): (Option<String>, Option<String>) = (None, None);
    for day in &days {
        if let Some(sp) = sample_path(node_dir, day) {
            if let Some(dd) = scan_samples_day(&sp).filter(|x| x.n != 0) {
                mem_all.extend(dd.mem);
                falling_all.extend(dd.falling);
                n_samples += dd.n;
                for a in dd.absent {
                    if !absent_union.contains(&a) {
                        absent_union.push(a);
                    }
                }
                from_samples.push(day.clone());
                if first_day.is_none() {
                    first_day = Some(day.clone());
                }
                last_day = Some(day.clone());
                continue;
            }
        }
        let rp = bb::join(&bb::join(node_dir, "rollup"), &format!("{day}.json"));
        if bb::is_file(&rp) {
            if let Ok(v) = bb::load_json(&rp) {
                rollup_only.push(v);
                from_rollup.push(day.clone());
                if first_day.is_none() {
                    first_day = Some(day.clone());
                }
                last_day = Some(day.clone());
                continue;
            }
        }
        missing.push(day.clone());
    }
    let fsort = |v: &mut Vec<f64>| v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    fsort(&mut mem_all);
    fsort(&mut falling_all);

    let mut r_desc_max = vec![];
    let mut r_mem_min = vec![];
    let mut r_mem_max = vec![];
    for s in &rollup_only {
        if let Some(v) = sub_get(s, "descent_rate_mib_s", "max")? {
            r_desc_max.push(v);
        }
    }
    for s in &rollup_only {
        if let Some(v) = sub_get(s, "mem_avail_mib", "min")? {
            r_mem_min.push(v);
        }
    }
    for s in &rollup_only {
        if let Some(v) = sub_get(s, "mem_avail_mib", "max")? {
            r_mem_max.push(v);
        }
    }
    for s in &rollup_only {
        let v = rget(s, "samples")?.filter(|v| v.is_truthy());
        if let Some(v) = v {
            let n = pynumber::int_call(v.peel()).map_err(|e| Raised::new(e.class, &e.message))?;
            n_samples += i64::try_from(n).unwrap_or(0);
        }
    }
    let first: Vec<PyValue> = falling_all.last().map(|x| PyValue::Float(*x)).into_iter().collect();
    let dm0 = extreme(&first, true)?;
    let mut cand = vec![];
    if let Some(x) = dm0 {
        cand.push(x);
    }
    if let Some(x) = extreme(&r_desc_max, true)? {
        cand.push(x);
    }
    let desc_max = extreme(&cand, true)?;
    let mut cand = vec![];
    if let Some(x) = mem_all.first() {
        cand.push(PyValue::Float(*x));
    }
    if let Some(x) = extreme(&r_mem_min, false)? {
        cand.push(x);
    }
    let mem_min = extreme(&cand, false)?;
    let mut cand = vec![];
    if let Some(x) = mem_all.last() {
        cand.push(PyValue::Float(*x));
    }
    if let Some(x) = extreme(&r_mem_max, true)? {
        cand.push(x);
    }
    let mem_max = extreme(&cand, true)?;
    let percentiles_exact = from_rollup.is_empty();

    let events = load_events(node_dir)?;
    let Some(d0) = days.first() else {
        return Err(Raised::new("IndexError", "list index out of range"));
    };
    let since = format!("{d0}T00:00:00Z");
    let lat = measure_kill_latency(&events, Some(&since))?;
    let lat_all = measure_kill_latency(&events, None)?;
    let (lat_used, lat_window) = if lat.is_empty() {
        (lat_all, "all-time")
    } else {
        (lat, "window")
    };
    let mut trips: Vec<&PyValue> = vec![];
    for r in &events {
        if str_of(py::get(r, "kind")).as_deref() == Some("watchdog_trip") {
            let ts = match py::get(r, "ts") {
                None => String::new(),
                Some(v) => str_of(Some(v)).ok_or_else(|| {
                    Raised::new(
                        "TypeError",
                        &format!(
                            "'>=' not supported between instances of '{}' and 'str'",
                            v.type_name()
                        ),
                    )
                })?,
            };
            if ts >= since {
                trips.push(r);
            }
        }
    }
    let lat_max = lat_used
        .iter()
        .cloned()
        .fold(None, |m: Option<f64>, x| Some(m.map_or(x, |m| if x > m { x } else { m })));

    let mut eta_params = PyDict::new();
    let mut eta_src = PyDict::new();
    if let Some(m) = lat_max {
        eta_params.set("kill_latency_s", PyValue::Float(m));
        eta_src.set(
            "kill_latency_s",
            py::s(&format!(
                "measured:events watchdog_trip→watchdog_kill_ack max (n={} · 1s 해상도 · {})",
                lat_used.len(),
                lat_window
            )),
        );
    } else {
        eta_params.set("kill_latency_s", PyValue::Float(eta::fl(eta::defaults().get("kill_latency_s").unwrap())?));
        eta_src.set(
            "kill_latency_s",
            py::s("default:blackbox_eta.DEFAULTS (실측 표본 0)"),
        );
    }
    for k in PASSTHROUGH_DEFAULT_KEYS {
        eta_params.set(k, PyValue::Float(eta::fl(eta::defaults().get(k).unwrap())?));
        eta_src.set(k, py::s("default:blackbox_eta.DEFAULTS"));
    }

    let derived = match mem_total_mib.filter(|m| *m != 0) {
        None => PyValue::Dict(PyDict::new()),
        Some(mt) => {
            let mut p = eta::defaults();
            for (k, v) in eta_params.iter() {
                p.set(&py::text(k), v.clone());
            }
            let runway = eta::runway_s(&p)?;
            let utr = mt as f64 / runway;
            let exceeds = match &desc_max {
                Some(v) => num(v)? >= utr,
                None => false,
            };
            d(vec![
                ("mem_total_mib", PyValue::int(mt)),
                ("mem_total_source", py::s(mem_total_source)),
                ("runway_s", PyValue::Float(eta::round3(runway))),
                (
                    "runway_source",
                    py::s("derived:blackbox_eta.runway_s(envelope.eta_params + DEFAULTS)"),
                ),
                (
                    "unconditional_trip_rate_mib_s",
                    PyValue::Float(pynumber::round_to_places(utr, 1)),
                ),
                (
                    "unconditional_trip_rate_source",
                    py::s("derived:mem_total/runway — 이 하강률 이상에서 ETA 는 잔량과 무관하게 항상 TRIP"),
                ),
                ("observed_max_rate_exceeds_unconditional", PyValue::Bool(exceeds)),
                (
                    "max_rate_mib_s",
                    eta::max_rate_mib_s(Some(mt), Some(&p))?.map_or(PyValue::None, PyValue::int),
                ),
                (
                    "max_rate_source",
                    py::s("derived:int(mem_total // runway) — 셸 정수 산술과 동일 경계"),
                ),
                (
                    "abs_band_mib",
                    PyValue::int(eta::default_int("abs_band_mib")),
                ),
                (
                    "abs_band_source",
                    py::s("default:blackbox_eta.DEFAULTS.abs_band_mib (레거시 절대임계 재사용)"),
                ),
            ])
        }
    };
    absent_union.sort();
    let mut lat_sorted = lat_used.clone();
    fsort(&mut lat_sorted);
    Ok(d(vec![
        ("schema_version", PyValue::int(SCHEMA_VERSION)),
        ("node_id", py::s(node_id)),
        ("generated_utc", py::s(now_utc)),
        (
            "state",
            py::s(if !from_samples.is_empty() || !from_rollup.is_empty() {
                "measured"
            } else {
                "empty"
            }),
        ),
        (
            "provenance",
            d(vec![
                ("source", py::s("rollup+1Hz samples")),
                ("resolution_s", PyValue::int(1)),
                ("window_days", PyValue::int(window_days)),
                ("days_from_samples", strs(&from_samples)),
                ("days_from_rollup", strs(&from_rollup)),
                ("days_missing", strs(&missing)),
                ("percentiles_exact", PyValue::Bool(percentiles_exact)),
                ("absent_fields", strs(&absent_union)),
                ("generator", py::s("regen_envelope.py")),
                (
                    "caveat",
                    py::s("백분위수는 원시 samples 가 남은 날에서만 정확하다. rollup 대체분이 있으면 `percentiles_exact=false` 이며 max/min 만 정확하다(최대의 최대·최소의 최소). kill 지연은 events 평면 1초 해상도 실측이라 seed 의 15초 재구성값을 대체한다."),
                ),
            ]),
        ),
        (
            "samples",
            d(vec![
                ("count", PyValue::int(n_samples)),
                ("first_day", py::opt_str(first_day.as_deref())),
                ("last_day", py::opt_str(last_day.as_deref())),
                (
                    "source",
                    py::s("measured:samples/*.csv[.zst|.gz] + rollup/*.json"),
                ),
            ]),
        ),
        (
            "descent_rate_mib_s",
            d(vec![
                ("falling_samples", PyValue::int(falling_all.len() as i64)),
                ("p50", pct(&falling_all, 0.50)),
                ("p90", pct(&falling_all, 0.90)),
                ("p99", pct(&falling_all, 0.99)),
                ("max", opt(desc_max)),
                ("max_source", py::s("measured:max-of-daily-max (정확)")),
                (
                    "percentile_source",
                    py::s(if percentiles_exact {
                        "measured:1Hz 원시 전수"
                    } else {
                        "partial:원시 결손일은 백분위수 미반영"
                    }),
                ),
            ]),
        ),
        (
            "mem_avail_mib",
            d(vec![
                ("min", opt(mem_min)),
                ("p01", pct(&mem_all, 0.01)),
                ("p50", pct(&mem_all, 0.50)),
                ("max", opt(mem_max)),
                ("min_source", py::s("measured:min-of-daily-min (정확)")),
            ]),
        ),
        (
            "kill",
            d(vec![
                ("trips", PyValue::int(trips.len() as i64)),
                (
                    "trip_mem_mib",
                    PyValue::List(
                        trips
                            .iter()
                            .map(|r| py::get(r, "mem_avail_mib").cloned().unwrap_or(PyValue::None))
                            .collect(),
                    ),
                ),
                (
                    "latency_s",
                    d(vec![
                        ("observed", floats(&lat_used)),
                        ("max", lat_max.map_or(PyValue::None, PyValue::Float)),
                        (
                            "p50",
                            if lat_used.is_empty() {
                                PyValue::None
                            } else {
                                pct(&lat_sorted, 0.50)
                            },
                        ),
                        ("n", PyValue::int(lat_used.len() as i64)),
                        ("scope", py::s(lat_window)),
                        (
                            "source",
                            py::s("measured:events watchdog_trip→watchdog_kill_ack (1s 해상도)"),
                        ),
                    ]),
                ),
            ]),
        ),
        ("eta_params", PyValue::Dict(eta_params)),
        ("eta_params_source", PyValue::Dict(eta_src)),
        ("derived", derived),
    ]))
}

fn sorted_names() -> String {
    let mut v: Vec<&str> = eta::DEFAULTS.iter().map(|x| x.0).collect();
    v.sort();
    v.join(", ")
}

/// `x in container` for the `eta_params_source` container (dict keys, list items or substring of a str).
fn contains(container: &PyValue, k: &str) -> bool {
    match container.peel() {
        PyValue::Dict(dd) => dd.contains_key(k),
        PyValue::List(l) => l.iter().any(|x| *x == py::s(k)),
        PyValue::Str(s) => py::text(s).contains(k),
        _ => false,
    }
}

/// `check_keys(envelope)`: (ok, errors, unknown_keys). `blackbox_eta.DEFAULTS` is the only key list.
pub fn check_keys(envelope: &PyValue) -> Result<(bool, Vec<String>, Vec<String>), Raised> {
    let mut errs = vec![];
    let params = match rget(envelope, "eta_params")? {
        None | Some(PyValue::None) => {
            return Ok((false, vec!["envelope 에 eta_params 가 없다".into()], vec![]))
        }
        Some(p) => p.clone(),
    };
    let PyValue::Dict(pd) = params.peel() else {
        return Ok((
            false,
            vec![format!(
                "eta_params 가 dict 가 아니다: {}",
                repr(params.type_name())
            )],
            vec![],
        ));
    };
    let unknown = eta::unknown_envelope_keys(Some(envelope));
    for k in &unknown {
        errs.push(format!(
            "eta_params.{k} 가 blackbox_eta.DEFAULTS 에 없다 — 현행 `if k in DEFAULTS` 필터가 **조용히 버린다**(가능: {})",
            sorted_names()
        ));
    }
    let src = rget(envelope, "eta_params_source")?
        .filter(|v| v.is_truthy())
        .cloned()
        .unwrap_or(PyValue::Dict(PyDict::new()));
    let mut keys: Vec<(String, PyValue)> = pd.iter().map(|(k, v)| (py::text(k), v.clone())).collect();
    keys.sort_by(|a, b| a.0.cmp(&b.0));
    for (k, v) in keys {
        if eta::ENVELOPE_META_KEYS.contains(&k.as_str()) {
            errs.push(format!(
                "eta_params.{k} 는 값이 아니라 메타다 — eta_params_source 로 이관하라"
            ));
            continue;
        }
        if unknown.contains(&k) {
            continue;
        }
        if !matches!(v.peel(), PyValue::Int(_) | PyValue::Float(_)) {
            errs.push(format!("eta_params.{k} 는 수치여야 한다: {}", v.repr()));
        }
        if !contains(&src, &k) {
            errs.push(format!(
                "eta_params.{k} 에 출처(eta_params_source)가 없다 — 헌법 §결정론 규율: 값 옆에 출처를 둔다"
            ));
        }
    }
    Ok((errs.is_empty(), errs, unknown))
}

#[derive(Default, Clone)]
pub struct A {
    pub node_dir: Option<String>,
    pub now: Option<String>,
    pub node_id: Option<String>,
    pub window_days: i64,
    pub mem_total_mib: Option<i64>,
    pub output: Option<String>,
    pub dry_run: bool,
    pub envelope: Option<String>,
    pub max_age_days: f64,
}

fn env_path(a: &A) -> String {
    match a.envelope.as_deref().filter(|e| !e.is_empty()) {
        Some(e) => e.to_string(),
        None => bb::join(a.node_dir.as_deref().unwrap_or(""), "envelope.json"),
    }
}

/// The text `", ".join(sorted(env.get("eta_params") or {}))` prints.
fn param_names(env: &PyValue) -> Result<String, Raised> {
    let p = rget(env, "eta_params")?.filter(|v| v.is_truthy());
    let mut v: Vec<String> = match p.map(|x| x.peel()) {
        None => vec![],
        Some(PyValue::Dict(dd)) => dd.iter().map(|(k, _)| py::text(k)).collect(),
        Some(PyValue::Str(s)) => py::text(s).chars().map(|c| c.to_string()).collect(),
        Some(PyValue::List(l)) => {
            let mut out = vec![];
            for x in l {
                match x.peel() {
                    PyValue::Str(s) => out.push(py::text(s)),
                    other => {
                        return Err(Raised::new(
                            "TypeError",
                            &format!(
                                "sequence item 0: expected str instance, {} found",
                                other.type_name()
                            ),
                        ))
                    }
                }
            }
            out
        }
        Some(other) => {
            return Err(Raised::new(
                "TypeError",
                &format!("'{}' object is not iterable", other.type_name()),
            ))
        }
    };
    v.sort();
    Ok(v.join(", "))
}

pub fn cmd_check_keys(a: &A) -> R<i32> {
    let path = env_path(a);
    if !bb::is_file(&path) {
        eprintln!("[regen] FAIL: envelope 이 없다: {path}");
        return Ok(1);
    }
    let env = bb::load_json(&path)?;
    let (ok, errs, unknown) = check_keys(&env)?;
    println!("[regen] 키 정합 검증 → {path}");
    println!("        정본 키(blackbox_eta.DEFAULTS) {} 종", eta::DEFAULTS.len());
    let names = param_names(&env)?;
    println!("        envelope.eta_params 키: {names}");
    if ok {
        println!("[regen] PASS — 불일치 0");
        return Ok(0);
    }
    let _ = std::io::stdout().flush();
    for e in &errs {
        eprintln!("  ✗ {e}");
    }
    eprintln!(
        "[regen] FAIL — 불일치 {} 건(그중 미지 키 {} 건)",
        errs.len(),
        unknown.len()
    );
    Ok(3)
}

/// `envelope_age_days(envelope, now_utc)`.
pub fn envelope_age_days(envelope: &PyValue, now_utc: &str) -> Result<Option<f64>, Raised> {
    let g = rget(envelope, "generated_utc")?.filter(|v| v.is_truthy());
    let Some(g) = g else {
        return Ok(None);
    };
    let Some(gs) = str_of(Some(g)) else {
        return Err(Raised::new(
            "TypeError",
            "expected string or bytes-like object",
        ));
    };
    if !iso_ok(&gs) {
        return Ok(None);
    }
    Ok(Some((epoch(now_utc)? - epoch(&gs)?) as f64 / 86400.0))
}

/// `repr(OSError)` from the traceback text `Class: [Errno N] message: 'path'` (e.g. `PermissionError(13, 'Permission denied')`).
fn os_error_repr(text: &str) -> String {
    let Some((class, rest)) = text.split_once(": ") else {
        return text.to_string();
    };
    if let Some(r) = rest.strip_prefix("[Errno ") {
        if let Some((n, msg)) = r.split_once("] ") {
            let msg = msg.rsplit_once(": ").map_or(msg, |x| x.0);
            return format!("{class}({n}, {})", repr(msg));
        }
    }
    format!("{class}({})", repr(rest))
}

fn emit_event(node_dir: Option<&str>, now: &str, kind: &str, extra: Vec<(&str, PyValue)>) {
    let Some(nd) = node_dir.filter(|n| !n.is_empty()) else {
        return;
    };
    let dir = bb::join(nd, "events");
    let mut rec = PyDict::new();
    rec.set("ts", py::s(now));
    rec.set("kind", py::s(kind));
    rec.set("source", py::s("regen_envelope"));
    for (k, v) in extra {
        rec.set(k, v);
    }
    let month: String = now.chars().take(7).collect();
    let r = bb::makedirs(&dir, 0o777).and_then(|_| {
        bb::append(
            &bb::join(&dir, &format!("{month}.jsonl")),
            &(bb::dumps_sorted(&PyValue::Dict(rec)) + "\n"),
        )
    });
    if let Err(e) = r {
        eprintln!("[regen] ⚠ 이벤트 기록 실패({kind}): {}", os_error_repr(&e.0));
    }
}

pub fn cmd_staleness(a: &A) -> R<i32> {
    let path = env_path(a);
    let now = parse_now(a.now.as_deref())?;
    let nd = a.node_dir.as_deref();
    if !bb::is_file(&path) {
        println!("[regen] ⚠ envelope 부재 — stale 로 간주: {path}");
        emit_event(
            nd,
            &now,
            "envelope_stale",
            vec![("reason", py::s("absent")), ("path", py::s(&path))],
        );
        return Ok(0);
    }
    let env = bb::load_json(&path)?;
    let age = envelope_age_days(&env, &now)?;
    let state = rget(&env, "state")?.cloned().unwrap_or(PyValue::None);
    let Some(age) = age else {
        println!("[regen] ⚠ generated_utc 파싱 불가 — stale 로 간주");
        emit_event(
            nd,
            &now,
            "envelope_stale",
            vec![("reason", py::s("unparsable_generated_utc"))],
        );
        return Ok(0);
    };
    let stale = age > a.max_age_days || state != py::s("measured");
    let line = format!(
        "[regen] envelope age={}일 state={} (임계 {}일)",
        bb::pct("%.2f", age),
        bb::s(&state),
        bb::pct("%g", a.max_age_days)
    );
    if stale {
        println!("{line} → ⚠ STALE (경고 · 차단 아님)");
        emit_event(
            nd,
            &now,
            "envelope_stale",
            vec![
                ("age_days", PyValue::Float(eta::round3(age))),
                ("state", state),
                ("max_age_days", PyValue::Float(a.max_age_days)),
            ],
        );
    } else {
        println!("{line} → OK");
    }
    Ok(0)
}

fn node_id_ok(s: &str) -> bool {
    Regex::new(NODE_ID_PATTERN).match_(s).is_some()
}

pub fn cmd_regen(a: &A) -> R<i32> {
    cmd_regen_with(a, eta::read_mem_total_mib)
}

fn cmd_regen_with(a: &A, mem_reader: fn() -> Option<i64>) -> R<i32> {
    let now = parse_now(a.now.as_deref())?;
    let node_dir = match a.node_dir.as_deref().filter(|n| !n.is_empty()) {
        Some(n) if bb::is_dir(n) => n.to_string(),
        _ => {
            eprintln!(
                "[regen] FAIL: --node-dir 이 없다: {}",
                a.node_dir.as_deref().map_or("None".into(), repr)
            );
            return Ok(1);
        }
    };
    let node_id = match a.node_id.as_deref().filter(|n| !n.is_empty()) {
        Some(n) => n.to_string(),
        None => bb::basename(&bb::normpath(&node_dir)),
    };
    if !node_id_ok(&node_id) {
        eprintln!(
            "[regen] FAIL: node_id {} 가 스킴 위반이다 — {}",
            repr(&node_id),
            NODE_ID_PATTERN
        );
        eprintln!("[regen]       node_id ::= manifest nodes[].role 슬러그(terraforming_node SKILL.md §2.7.6). hostname 파생은 폐지됐다.");
        eprintln!("[regen]       경로를 docs/logs/<role> 로 마이그레이션하거나 --node-id=<slug> 로 명시하라.");
        return Ok(1);
    }
    let (mt, mts) = match a.mem_total_mib.filter(|m| *m != 0) {
        Some(m) => (Some(m), "explicit:--mem-total-mib".to_string()),
        None => {
            let mt = mem_reader();
            (
                mt,
                if mt.is_none() {
                    "unknown:/proc/meminfo 판독 실패".into()
                } else {
                    "measured:/proc/meminfo MemTotal".into()
                },
            )
        }
    };
    let env = build_envelope(&node_id, &node_dir, &now, a.window_days, mt, &mts)?;
    let (ok, errs, _) = check_keys(&env)?;
    if !ok {
        eprintln!("[regen] FAIL: 생성한 envelope 이 키 정합을 위반했다(생성기 결함):");
        for e in errs {
            eprintln!("  ✗ {e}");
        }
        return Ok(3);
    }
    let out = match a.output.as_deref().filter(|o| !o.is_empty()) {
        Some(o) => o.to_string(),
        None => bb::join(&node_dir, "envelope.json"),
    };
    let body = bb::dumps_sorted_pretty(&env) + "\n";
    if a.dry_run {
        println!("{body}");
        eprintln!("[regen] (dry-run) 쓰지 않았다 → {out}");
        return Ok(0);
    }
    bb::write_atomic(&out, &body)?;
    let g = |path: &[&str]| -> PyValue {
        let mut v = env.clone();
        for k in path {
            v = py::get(&v, k).cloned().unwrap_or(PyValue::None);
        }
        v
    };
    let len = |path: &[&str]| match g(path) {
        PyValue::List(l) => l.len(),
        _ => 0,
    };
    println!("[regen] {node_id} → {out}");
    println!(
        "        state={} · samples={} · 원시일={} · rollup일={} · 결손일={}",
        bb::s(&g(&["state"])),
        bb::s(&g(&["samples", "count"])),
        len(&["provenance", "days_from_samples"]),
        len(&["provenance", "days_from_rollup"]),
        len(&["provenance", "days_missing"])
    );
    println!(
        "        하강률 max={} p99={} p50={} (정확백분위={})",
        bb::s(&g(&["descent_rate_mib_s", "max"])),
        bb::s(&g(&["descent_rate_mib_s", "p99"])),
        bb::s(&g(&["descent_rate_mib_s", "p50"])),
        bb::s(&g(&["provenance", "percentiles_exact"]))
    );
    println!(
        "        kill 지연 실측 max={} p50={} n={} ({})",
        bb::s(&g(&["kill", "latency_s", "max"])),
        bb::s(&g(&["kill", "latency_s", "p50"])),
        bb::s(&g(&["kill", "latency_s", "n"])),
        bb::s(&g(&["kill", "latency_s", "scope"]))
    );
    if g(&["derived"]).is_truthy() {
        println!(
            "        파생: runway={}s · 무조건-트립 경계={} MiB/s{}",
            bb::s(&g(&["derived", "runway_s"])),
            bb::s(&g(&["derived", "unconditional_trip_rate_mib_s"])),
            if g(&["derived", "observed_max_rate_exceeds_unconditional"]).is_truthy() {
                "  ⚠ 관측 최대 하강률이 이 경계를 넘는다"
            } else {
                ""
            }
        );
    }
    emit_event(
        Some(&node_dir),
        &now,
        "envelope_regenerated",
        vec![
            ("state", g(&["state"])),
            ("descent_max_mib_s", g(&["descent_rate_mib_s", "max"])),
            ("kill_latency_max_s", g(&["kill", "latency_s", "max"])),
            ("window_days", PyValue::int(a.window_days)),
        ],
    );
    Ok(0)
}

fn self_test() -> i32 {
    let mut checks: Vec<(String, bool)> = vec![];
    let mut c = |n: &str, b: bool| checks.push((n.to_string(), b));
    let td = bb::TempDir::new();
    let nd = bb::join(td.path(), "node-x");
    for sub in ["samples", "events", "rollup"] {
        let _ = std::fs::create_dir_all(bb::join(&nd, sub));
    }
    let hdr = "ts,mem_avail,mem_rate,gpu_temp,gpu_pwr,gpu_sm,gpu_util,gpu_mem,load1,ctr_n\n";
    let mut body = hdr.to_string();
    for (i, (m, r)) in [
        (100000, 0.0),
        (90000, 23313.5),
        (80000, 5.0),
        (70000, -3.0),
        (60000, 1.0),
    ]
    .iter()
    .enumerate()
    {
        body.push_str(&format!(
            "{},{},{},50,10,1,0,,0.5,1\n",
            1000 + i,
            m,
            bb::pct("%g", *r)
        ));
    }
    let _ = std::fs::write(bb::join(&nd, "samples/2026-07-31.csv"), body);
    let _ = std::fs::write(
        bb::join(&nd, "rollup/2026-07-30.json"),
        r#"{"day": "2026-07-30", "samples": 10, "mem_avail_mib": {"min": 5000.0, "max": 111111.0}, "descent_rate_mib_s": {"max": 99.0}}"#,
    );
    let evs = "{\"ts\": \"2026-07-31T00:00:10Z\", \"kind\": \"watchdog_trip\", \"mem_avail_mib\": 31487, \"targets\": \"abc\"}\n{\"ts\": \"2026-07-31T00:00:13Z\", \"kind\": \"watchdog_kill_ack\", \"targets\": \"abc\"}\n";
    let _ = std::fs::write(bb::join(&nd, "events/2026-07.jsonl"), evs);
    let _ = std::fs::write(bb::join(&nd, "events/watchdog.jsonl"), evs);

    let g = |env: &PyValue, path: &[&str]| -> PyValue {
        let mut v = env.clone();
        for k in path {
            v = py::get(&v, k).cloned().unwrap_or(PyValue::None);
        }
        v
    };
    let env = build_envelope("node-x", &nd, "2026-07-31T12:00:00Z", 3, Some(124610), "explicit:self-test")
        .unwrap_or(PyValue::None);
    let f = PyValue::Float;
    c("state=measured", g(&env, &["state"]) == py::s("measured"));
    c(
        "실측 하강률 max 보존(23313.5)",
        g(&env, &["descent_rate_mib_s", "max"]) == f(23313.5),
    );
    c(
        "rollup 대체분의 max 도 합쳐진다(최대의 최대)",
        g(&env, &["mem_avail_mib", "max"]) == f(111111.0),
    );
    c(
        "rollup 대체분의 min 도 합쳐진다",
        g(&env, &["mem_avail_mib", "min"]) == f(5000.0),
    );
    c(
        "rollup 섞이면 percentiles_exact=False",
        g(&env, &["provenance", "percentiles_exact"]) == PyValue::Bool(false),
    );
    c(
        "음수 rate(상승)는 하강 표본에서 제외",
        g(&env, &["descent_rate_mib_s", "falling_samples"]) == PyValue::int(3),
    );
    c(
        "kill 지연 실측 3초·중복 제거로 n=1",
        g(&env, &["kill", "latency_s", "max"]) == f(3.0)
            && g(&env, &["kill", "latency_s", "n"]) == PyValue::int(1),
    );
    let starts = |v: PyValue, p: &str| matches!(v, PyValue::Str(s) if py::text(&s).starts_with(p));
    c(
        "kill_latency_s 출처가 measured 로 표기",
        starts(g(&env, &["eta_params_source", "kill_latency_s"]), "measured:"),
    );
    c(
        "기본값 통과분은 default 로 표기",
        starts(g(&env, &["eta_params_source", "agent_act_s"]), "default:"),
    );
    c(
        "상시 공란 컬럼이 absent_fields 로 잡힌다",
        matches!(g(&env, &["provenance", "absent_fields"]), PyValue::List(l) if l.contains(&py::s("gpu_mem"))),
    );
    c(
        "파생 runway 가 측정 kill 지연을 반영",
        g(&env, &["derived", "runway_s"]) == f(7.0),
    );
    c(
        "무조건-트립 경계가 mem_total 에서 파생",
        g(&env, &["derived", "unconditional_trip_rate_mib_s"]) == f(17801.4),
    );
    c(
        "관측 최대가 경계를 넘으면 표시",
        g(&env, &["derived", "observed_max_rate_exceeds_unconditional"]) == PyValue::Bool(true),
    );
    let env2 = build_envelope("node-x", &nd, "2026-07-31T12:00:00Z", 3, Some(62305), "explicit:self-test")
        .unwrap_or(PyValue::None);
    c(
        "경계가 mem_total 변경을 따라간다",
        g(&env2, &["derived", "unconditional_trip_rate_mib_s"]) == f(8900.7),
    );
    c(
        "이중규칙 정수 상한이 envelope 에 실린다(124610/7=17801)",
        g(&env, &["derived", "max_rate_mib_s"]) == PyValue::int(17801),
    );
    c(
        "정수 상한도 mem_total 을 따라간다",
        g(&env2, &["derived", "max_rate_mib_s"]) == PyValue::int(8900),
    );
    c(
        "절대 밴드가 정본(blackbox_eta.DEFAULTS)에서 온다",
        g(&env, &["derived", "abs_band_mib"]) == PyValue::int(eta::default_int("abs_band_mib")),
    );
    let ck = |v: &PyValue| check_keys(v).unwrap_or((false, vec![], vec![]));
    let (ok, errs, _) = ck(&env);
    c("생성 산출물은 키 정합 통과", ok && errs.is_empty());
    let legacy = py::dict(vec![
        (
            "eta_params",
            py::dict(vec![
                ("agent_act_s", PyValue::int(300)),
                ("agent_notify_s", PyValue::int(900)),
                ("daemon_kill_s", PyValue::int(6)),
                ("eta_floor_s", f(6.0)),
                ("provenance", py::s("초기 임의값")),
            ]),
        ),
        ("eta_params_source", PyValue::Dict(PyDict::new())),
    ]);
    let (ok2, errs2, mut unk2) = ck(&legacy);
    unk2.sort();
    c(
        "★ seed envelope 의 미지 키 2건 검출(daemon_kill_s·eta_floor_s)",
        !ok2 && unk2 == vec!["daemon_kill_s", "eta_floor_s"],
    );
    c(
        "eta_params.provenance 는 이관 요구로 잡힌다",
        errs2.iter().any(|e| e.contains("이관")),
    );
    c("출처 누락도 잡힌다", errs2.iter().any(|e| e.contains("출처")));
    let (ok3, errs3, _) = ck(&py::dict(vec![
        ("eta_params", py::dict(vec![("kill_latency_s", py::s("4.0"))])),
        ("eta_params_source", py::dict(vec![("kill_latency_s", py::s("x"))])),
    ]));
    c("문자열 값 거부", !ok3 && errs3.iter().any(|e| e.contains("수치")));
    let (ok4, _, _) = ck(&py::dict(vec![
        ("eta_params", py::dict(vec![("kill_latency_s", f(4.0))])),
        (
            "eta_params_source",
            py::dict(vec![("kill_latency_s", py::s("measured:x"))]),
        ),
    ]));
    c("정상 최소 envelope 통과", ok4);
    c(
        "age 계산(0.5일)",
        envelope_age_days(
            &py::dict(vec![("generated_utc", py::s("2026-07-31T00:00:00Z"))]),
            "2026-07-31T12:00:00Z",
        )
        .ok()
        .flatten()
        .is_some_and(|x| (x - 0.5).abs() < 1e-9),
    );
    c(
        "generated_utc 불량 → None",
        matches!(
            envelope_age_days(&py::dict(vec![("generated_utc", py::s("bad"))]), "2026-07-31T12:00:00Z"),
            Ok(None)
        ),
    );
    let ls = |p: &str| bb::listdir_sorted(p).unwrap_or_default();
    let before = ls(&bb::join(&nd, "samples"));
    let mk = |node_dir: &str, node_id: Option<&str>| A {
        node_dir: Some(node_dir.to_string()),
        node_id: node_id.map(String::from),
        now: Some("2026-07-31T12:00:00Z".into()),
        window_days: 3,
        mem_total_mib: Some(124610),
        output: None,
        dry_run: false,
        ..Default::default()
    };
    let rc = |r: R<i32>| match r {
        Ok(c) => c,
        Err(Fail::Code(c)) => c,
        Err(Fail::Exit(_)) => 1,
        Err(Fail::Raise(e)) => panic!("self-test raised {}", e.0),
    };
    let r = rc(cmd_regen(&mk(&nd, None)));
    c("regen 성공", r == 0);
    c("envelope.json 기록됨", bb::is_file(&bb::join(&nd, "envelope.json")));
    c("원시 samples 미편집", ls(&bb::join(&nd, "samples")) == before);
    c(
        ".tmp 잔재 없음",
        std::fs::metadata(bb::join(&nd, "envelope.json.tmp")).is_err(),
    );
    let wrote = bb::load_json(&bb::join(&nd, "envelope.json")).unwrap_or(PyValue::None);
    c("기록본이 키 정합 통과", ck(&wrote).0);
    let evs = load_events(&nd).unwrap_or_default();
    c(
        "재생성 이벤트 기록",
        evs.iter()
            .any(|e| py::get_str(e, "kind").as_deref() == Some("envelope_regenerated")),
    );
    c("node_id 정규식 양성", node_id_ok("sub2"));
    c("node_id 정규식 음성(hostname 잔재)", !node_id_ok("spark-A73E"));
    let bad_nd = bb::join(td.path(), "Spark-BAD");
    let _ = std::fs::create_dir_all(bb::join(&bad_nd, "samples"));
    let rc_bad = rc(cmd_regen(&mk(&bad_nd, None)));
    c("스킴 위반 node_dir 거부(rc=1)", rc_bad == 1);
    c(
        "거부 시 envelope 미기록",
        std::fs::metadata(bb::join(&bad_nd, "envelope.json")).is_err(),
    );
    c(
        "--node-id 명시로 우회 가능",
        rc(cmd_regen(&mk(&bad_nd, Some("sub")))) == 0,
    );
    c(
        "비ISO --now 거부",
        parse_now(Some("2026-07-31 12:00:00")).is_err(),
    );
    drop(td);
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

static REGEN_ARGS: [Arg; 7] = [
    HELP,
    store(&["--now"]).required(),
    store(&["--node-id"]),
    store(&["--window-days"]).int(),
    store(&["--mem-total-mib"]).int(),
    store(&["-o", "--output"]),
    flag(&["--dry-run"]),
];
static CHECK_ARGS: [Arg; 2] = [HELP, store(&["--envelope"])];
static STALE_ARGS: [Arg; 4] = [
    HELP,
    store(&["--now"]).required(),
    store(&["--envelope"]),
    store(&["--max-age-days"]).float(),
];

static REGEN: Spec = Spec {
    prog: "regen_envelope.py regen",
    usage: include_str!("../help/regen_envelope.regen.usage"),
    help: include_str!("../help/regen_envelope.regen.help"),
    args: &REGEN_ARGS,
    subs: None,
};
static CHECK: Spec = Spec {
    prog: "regen_envelope.py check-keys",
    usage: include_str!("../help/regen_envelope.check-keys.usage"),
    help: include_str!("../help/regen_envelope.check-keys.help"),
    args: &CHECK_ARGS,
    subs: None,
};
static STALE: Spec = Spec {
    prog: "regen_envelope.py staleness",
    usage: include_str!("../help/regen_envelope.staleness.usage"),
    help: include_str!("../help/regen_envelope.staleness.help"),
    args: &STALE_ARGS,
    subs: None,
};

static TOP_ARGS: [Arg; 3] = [HELP, store(&["--node-dir"]), flag(&["--self-test"])];

pub static SPEC: Spec = Spec {
    prog: "regen_envelope.py",
    usage: include_str!("../help/regen_envelope.usage"),
    help: include_str!("../help/regen_envelope.help"),
    args: &TOP_ARGS,
    subs: Some((
        "cmd",
        &[("regen", &REGEN), ("check-keys", &CHECK), ("staleness", &STALE)],
    )),
};

fn run(p: &Parsed) -> R<i32> {
    if p.flag(2) {
        return Ok(self_test());
    }
    let Some((name, sp)) = p.sub.as_ref() else {
        argp::print_help_stdout(&SPEC);
        return Ok(1);
    };
    let mut a = A {
        node_dir: p.text(1).map(String::from),
        ..Default::default()
    };
    match name.as_str() {
        "regen" => {
            a.now = sp.text(1).map(String::from);
            a.node_id = sp.text(2).map(String::from);
            a.window_days = sp.int(3).unwrap_or(14);
            a.mem_total_mib = sp.int(4);
            a.output = sp.text(5).map(String::from);
            a.dry_run = sp.flag(6);
            cmd_regen(&a)
        }
        "check-keys" => {
            a.envelope = sp.text(1).map(String::from);
            cmd_check_keys(&a)
        }
        _ => {
            a.now = sp.text(1).map(String::from);
            a.envelope = sp.text(2).map(String::from);
            a.max_age_days = sp.float(3).unwrap_or(7.0);
            cmd_staleness(&a)
        }
    }
}

pub fn main(argv: &[String]) -> i32 {
    match argp::parse(&SPEC, argv) {
        Outcome::Exit(c) => c,
        Outcome::Ok(p) => bb::finish(run(&p)),
    }
}
