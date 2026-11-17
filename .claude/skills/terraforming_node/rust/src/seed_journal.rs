//! `node_blackbox/seed_from_journal.py` — Phase 0 evidence harvester (plan_26073109 §Phase 0): the legacy mem_watchdog
//! systemd journal becomes the node blackbox `docs/logs/` **seed** (15 s HB reconstruction, not 1 s samples; absent fields
//! are declared in `seed/README.json`). Outputs: `<logs_root>/<node_id>/seed/{memwatch-journal.txt, samples_15s.csv,
//! README.json}`, `events/<YYYY-MM>.jsonl`, `envelope.json`. Time comes from `--generated-utc` only.
//! Exit: 0 success · 1 input/argument error · 2 gate failure (retention, expected trips) or self-test failure.

use crate::argp::{self, flag, store, Outcome, Spec, HELP};
use crate::logs_lifecycle::sort_keys;
use crate::py::{self, s};
use crate::pyre::Regex;
use crate::pytime::{strptime_checked, DateTime};
use agent_runtime_support::pynumber::{format_fixed, round_to_places};
use agent_runtime_support::pyvalue::PyValue;
use std::path::Path;

pub const SCHEMA_VERSION: i64 = 1;
const CSV_HEADER: &str = "ts_utc,mem_avail_mib,mem_min_mib,dt_s,mem_rate_mib_s\n";
const ABSENT: [&str; 7] = [
    "gpu_temp", "gpu_pwr", "gpu_sm", "gpu_util", "gpu_mem", "load1", "ctr_n",
];

struct Res {
    hb: Regex,
    trip: Regex,
    trip_nomatch: Regex,
    start: Regex,
    prefix: Regex,
    unit: Regex,
    unit_state: Regex,
    boot: Regex,
    kill_ack: Regex,
    accounting: Regex,
}

fn res() -> Res {
    let ts = r"(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z)";
    Res {
        hb: Regex::new(&format!(
            r"\[mem-watchdog\]\s+HB\s+MemAvailable=(\d+)MiB(?:\s+min=(\d+)MiB)?\s+{ts}"
        )),
        trip: Regex::new(&format!(
            r"\[mem-watchdog\]\s+TRIP\s+MemAvailable=(\d+)MiB\s+<\s+(\d+)MiB\s+\S+\s+docker kill\s+(\S+)\s+{ts}"
        )),
        trip_nomatch: Regex::new(&format!(
            r"\[mem-watchdog\]\s+TRIP-nomatch\s+MemAvailable=(\d+)MiB\s+<\s+(\d+)MiB.*?{ts}"
        )),
        start: Regex::new(&format!(
            r"\[mem-watchdog\]\s+start\s+filter='([^']*)'\s+threshold=(\d+)MiB\s+interval=(\d+)s\s+heartbeat=(\d+)s\s+pid=(\d+)\s+{ts}"
        )),
        prefix: Regex::new(
            r"^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}[+-]\d{2}:\d{2})\s+(\S+)\s+([^:]+):\s*(.*)$",
        ),
        unit: Regex::new(r"^(Started|Stopped|Stopping)\b"),
        unit_state: Regex::new(r"^\S+\.service:\s+(Deactivated successfully|Scheduled restart)"),
        boot: Regex::new(r"^--\s+Boot\s+([0-9a-f]+)\s+--\s*$"),
        kill_ack: Regex::new(r"^\[mem-watchdog\]\s+([0-9a-f]{8,})\s*$"),
        accounting: Regex::new(r"^\S+\.service:\s+Consumed\b"),
    }
}

/// Python `int(text)` of a `\d+` capture (Unicode decimal digits).
fn int_of(t: &str) -> i64 {
    t.chars().fold(0i64, |a, c| {
        a.saturating_mul(10).saturating_add(
            agent_runtime_support::pystr::decimal_digit_value(c as u32).unwrap_or(0) as i64,
        )
    })
}

/// A raised ValueError of `_parse_iso_z` (exit 1 through the traceback).
pub struct Raised(pub String);

fn parse_iso_z(t: &str) -> Result<i64, Raised> {
    strptime_checked(t, "%Y-%m-%dT%H:%M:%SZ")
        .map(|d| d.epoch())
        .map_err(|m| Raised(format!("ValueError: {m}")))
}

/// `datetime.fromisoformat(text).astimezone(timezone.utc)` for `YYYY-MM-DDTHH:MM:SS±HH:MM` (ASCII digits only, as CPython).
fn from_iso_offset(t: &str) -> Option<i64> {
    let b = t.as_bytes();
    if b.len() != 25 || !t.is_ascii() {
        return None;
    }
    let n = |r: std::ops::Range<usize>| -> Option<u32> {
        t.get(r)?.parse::<u32>().ok().filter(|_| true)
    };
    let digits = |r: std::ops::Range<usize>| b[r].iter().all(|c| c.is_ascii_digit());
    if !(digits(0..4)
        && digits(5..7)
        && digits(8..10)
        && digits(11..13)
        && digits(14..16)
        && digits(17..19)
        && digits(20..22)
        && digits(23..25))
    {
        return None;
    }
    let (y, mo, d, h, mi, se, oh, om) = (
        n(0..4)? as i64,
        n(5..7)?,
        n(8..10)?,
        n(11..13)?,
        n(14..16)?,
        n(17..19)?,
        n(20..22)?,
        n(23..25)?,
    );
    if y < 1
        || !(1..=12).contains(&mo)
        || d < 1
        || d > crate::pytime::days_in_month(y, mo)
        || h > 23
        || mi > 59
        || se > 59
        || oh > 23
        || om > 59
    {
        return None;
    }
    let off = (oh * 3600 + om * 60) as i64 * if b[19] == b'-' { -1 } else { 1 };
    let dt = DateTime {
        year: y,
        month: mo,
        day: d,
        hour: h,
        minute: mi,
        second: se,
    };
    Some(dt.epoch() - off)
}

fn fmt_z(t: i64) -> String {
    DateTime::from_epoch(t).iso_z()
}

#[derive(Clone)]
pub struct Sample {
    pub ts: i64,
    pub mem_avail: i64,
    pub mem_min: Option<i64>,
    pub dt_s: Option<f64>,
    pub mem_rate: Option<f64>,
}

/// An event: its time and the other fields in the original's insertion order.
#[derive(Clone)]
pub struct Event {
    pub ts: i64,
    pub fields: Vec<(&'static str, PyValue)>,
}

impl Event {
    fn kind(&self) -> String {
        self.fields
            .iter()
            .find(|(k, _)| *k == "kind")
            .map(|(_, v)| py::display(v))
            .unwrap_or_default()
    }
    fn get(&self, k: &str) -> Option<&PyValue> {
        self.fields.iter().find(|(n, _)| *n == k).map(|(_, v)| v)
    }
}

pub const COUNTERS: [&str; 10] = [
    "total",
    "hb",
    "trip",
    "trip_nomatch",
    "start",
    "unit",
    "boot",
    "kill_ack",
    "ignored_known",
    "unmatched",
];

pub struct Counters(pub [i64; 10]);
impl Counters {
    fn inc(&mut self, k: &str) {
        self.0[COUNTERS.iter().position(|x| *x == k).unwrap()] += 1;
    }
    pub fn get(&self, k: &str) -> i64 {
        self.0[COUNTERS.iter().position(|x| *x == k).unwrap()]
    }
    fn value(&self) -> PyValue {
        py::dict(
            COUNTERS
                .iter()
                .zip(self.0.iter())
                .map(|(k, v)| (*k, PyValue::int(*v)))
                .collect(),
        )
    }
}

const SRC: &str = "seed:memwatch_journal";

pub fn parse_journal(lines: &[String]) -> Result<(Vec<Sample>, Vec<Event>, Counters), Raised> {
    let r = res();
    let (mut samples, mut events) = (vec![], vec![]);
    let mut c = Counters([0; 10]);
    let mut pending: Vec<String> = vec![];
    for raw in lines {
        let line = raw.trim_end_matches('\n');
        if py::strip(line).is_empty() {
            continue;
        }
        c.inc("total");
        if let Some(m) = r.boot.match_(line) {
            c.inc("boot");
            pending.push(m.group(1).unwrap_or("").to_string());
            continue;
        }
        let pm = r.prefix.match_(line);
        let jts = pm
            .as_ref()
            .and_then(|m| from_iso_offset(m.group(1).unwrap_or("")));
        if let Some(t) = jts {
            for id in pending.drain(..) {
                events.push(Event {
                    ts: t,
                    fields: vec![
                        ("kind", s("boot_marker")),
                        ("boot_id", s(&id)),
                        ("ts_approx", PyValue::Bool(true)),
                        ("source", s(SRC)),
                    ],
                });
            }
        }
        let body: String = pm
            .as_ref()
            .map_or(line.to_string(), |m| m.group(4).unwrap_or("").to_string());
        if let (Some(m), Some(t)) = (r.kill_ack.match_(&body), jts) {
            c.inc("kill_ack");
            events.push(Event {
                ts: t,
                fields: vec![
                    ("kind", s("watchdog_kill_ack")),
                    ("target", s(m.group(1).unwrap_or(""))),
                    ("source", s(SRC)),
                ],
            });
            continue;
        }
        if r.accounting.match_(&body).is_some() {
            c.inc("ignored_known");
            continue;
        }
        if let (Some(m), Some(t)) = (r.unit_state.match_(&body), jts) {
            c.inc("unit");
            let first = m
                .group(1)
                .unwrap_or("")
                .split(py::is_space)
                .next()
                .unwrap_or("")
                .to_lowercase();
            events.push(Event {
                ts: t,
                fields: vec![
                    ("kind", s(&format!("watchdog_unit_{first}"))),
                    ("source", s(SRC)),
                ],
            });
            continue;
        }
        if let Some(m) = r.hb.search(line) {
            c.inc("hb");
            samples.push(Sample {
                ts: parse_iso_z(m.group(3).unwrap_or(""))?,
                mem_avail: int_of(m.group(1).unwrap_or("")),
                mem_min: m.group(2).filter(|x| !x.is_empty()).map(int_of),
                dt_s: None,
                mem_rate: None,
            });
            continue;
        }
        if let Some(m) = r.trip.search(line) {
            c.inc("trip");
            events.push(Event {
                ts: parse_iso_z(m.group(4).unwrap_or(""))?,
                fields: vec![
                    ("kind", s("watchdog_trip")),
                    (
                        "mem_avail_mib",
                        PyValue::int(int_of(m.group(1).unwrap_or(""))),
                    ),
                    (
                        "threshold_mib",
                        PyValue::int(int_of(m.group(2).unwrap_or(""))),
                    ),
                    ("target", s(m.group(3).unwrap_or(""))),
                    ("action", s("docker_kill")),
                    ("source", s(SRC)),
                ],
            });
            continue;
        }
        if let Some(m) = r.trip_nomatch.search(line) {
            c.inc("trip_nomatch");
            events.push(Event {
                ts: parse_iso_z(m.group(3).unwrap_or(""))?,
                fields: vec![
                    ("kind", s("watchdog_trip_nomatch")),
                    (
                        "mem_avail_mib",
                        PyValue::int(int_of(m.group(1).unwrap_or(""))),
                    ),
                    (
                        "threshold_mib",
                        PyValue::int(int_of(m.group(2).unwrap_or(""))),
                    ),
                    ("action", s("none")),
                    ("source", s(SRC)),
                ],
            });
            continue;
        }
        if let Some(m) = r.start.search(line) {
            c.inc("start");
            let g = |i: usize| int_of(m.group(i).unwrap_or(""));
            events.push(Event {
                ts: parse_iso_z(m.group(6).unwrap_or(""))?,
                fields: vec![
                    ("kind", s("watchdog_start")),
                    ("filter", s(m.group(1).unwrap_or(""))),
                    ("threshold_mib", PyValue::int(g(2))),
                    ("interval_s", PyValue::int(g(3))),
                    ("heartbeat_s", PyValue::int(g(4))),
                    ("pid", PyValue::int(g(5))),
                    ("source", s(SRC)),
                ],
            });
            continue;
        }
        if let (Some(m), Some(t)) = (r.unit.match_(&body), jts) {
            c.inc("unit");
            events.push(Event {
                ts: t,
                fields: vec![
                    (
                        "kind",
                        s(&format!(
                            "watchdog_unit_{}",
                            m.group(1).unwrap_or("").to_lowercase()
                        )),
                    ),
                    ("source", s(SRC)),
                ],
            });
            continue;
        }
        c.inc("unmatched");
    }
    samples.sort_by_key(|x| x.ts);
    events.sort_by_key(|x| x.ts);
    Ok((samples, events, c))
}

pub fn compute_rates(samples: &mut [Sample]) {
    for i in 0..samples.len() {
        if i == 0 {
            samples[i].dt_s = None;
            samples[i].mem_rate = None;
            continue;
        }
        let (prev_ts, prev_mem) = (samples[i - 1].ts, samples[i - 1].mem_avail);
        let dt = (samples[i].ts - prev_ts) as f64;
        if dt <= 0.0 || dt > 120.0 {
            samples[i].dt_s = if dt > 0.0 { Some(dt) } else { None };
            samples[i].mem_rate = None;
        } else {
            samples[i].dt_s = Some(dt);
            samples[i].mem_rate = Some(round_to_places(
                (samples[i].mem_avail - prev_mem) as f64 / dt,
                2,
            ));
        }
    }
}

fn pct<T: Copy>(sorted: &[T], q: f64) -> Option<T> {
    if sorted.is_empty() {
        return None;
    }
    let r = (q * (sorted.len() as f64 - 1.0)).round_ties_even() as i64;
    Some(sorted[(sorted.len() as i64 - 1).min(r.max(0)) as usize])
}

pub struct Latency {
    trip_utc: String,
    target: PyValue,
    trip_mem_mib: i64,
    recovered_mem_mib: i64,
    latency: f64,
}

impl Latency {
    fn value(&self) -> PyValue {
        py::dict(vec![
            ("trip_utc", s(&self.trip_utc)),
            ("target", self.target.clone()),
            ("trip_mem_mib", PyValue::int(self.trip_mem_mib)),
            ("recovered_mem_mib", PyValue::int(self.recovered_mem_mib)),
            ("latency_upper_bound_s", PyValue::Float(self.latency)),
        ])
    }
}

pub fn kill_latency_bounds(
    samples: &[Sample],
    events: &[Event],
    recovery_mib: i64,
) -> Vec<Latency> {
    let mut out = vec![];
    for ev in events {
        if ev.kind() != "watchdog_trip" {
            continue;
        }
        let base = match ev.get("mem_avail_mib") {
            Some(PyValue::Int(n)) => i64::try_from(n.clone()).unwrap_or(0),
            _ => 0,
        };
        for smp in samples {
            if smp.ts <= ev.ts {
                continue;
            }
            if (smp.ts - ev.ts) as f64 > 300.0 {
                break;
            }
            if smp.mem_avail - base >= recovery_mib {
                out.push(Latency {
                    trip_utc: fmt_z(ev.ts),
                    target: ev.get("target").cloned().unwrap_or(PyValue::None),
                    trip_mem_mib: base,
                    recovered_mem_mib: smp.mem_avail,
                    latency: (smp.ts - ev.ts) as f64,
                });
                break;
            }
        }
    }
    out
}

fn of<T: Copy>(v: Option<T>, f: impl Fn(T) -> PyValue) -> PyValue {
    v.map_or(PyValue::None, f)
}

pub fn build_envelope(
    node_id: &str,
    generated: &str,
    samples: &[Sample],
    events: &[Event],
    c: &Counters,
    lat: &[Latency],
) -> PyValue {
    let mut falling: Vec<f64> = samples
        .iter()
        .filter_map(|x| x.mem_rate.filter(|r| *r < 0.0).map(|r| -r))
        .collect();
    falling.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut mem: Vec<i64> = samples.iter().map(|x| x.mem_avail).collect();
    mem.sort();
    let l: Vec<f64> = lat.iter().map(|x| x.latency).collect();
    let trips: Vec<&Event> = events
        .iter()
        .filter(|e| e.kind() == "watchdog_trip")
        .collect();
    let fl = |q: f64| of(pct(&falling, q), PyValue::Float);
    let mm = |q: f64| of(pct(&mem, q), PyValue::int);
    let rated = falling.len()
        + samples
            .iter()
            .filter(|x| x.mem_rate.is_some_and(|r| r >= 0.0))
            .count();
    py::dict(vec![
        ("schema_version", PyValue::int(SCHEMA_VERSION)),
        ("node_id", s(node_id)),
        ("generated_utc", s(generated)),
        ("state", s("seed")),
        (
            "provenance",
            py::dict(vec![
                ("source", s("mem_watchdog systemd journal (legacy)")),
                ("resolution_s", PyValue::int(15)),
                ("absent_fields", PyValue::List(ABSENT.iter().map(|x| s(x)).collect())),
                ("caveat", s("15s 해상도 재구성 -- 순간 최대 하강률은 과소평가되고 kill 지연은 과대평가된다. 1초 실샘플로 대체될 때까지 보수적으로만 사용한다.")),
            ]),
        ),
        (
            "samples",
            py::dict(vec![
                ("count", PyValue::int(samples.len() as i64)),
                ("first_utc", samples.first().map_or(PyValue::None, |x| s(&fmt_z(x.ts)))),
                ("last_utc", samples.last().map_or(PyValue::None, |x| s(&fmt_z(x.ts)))),
                ("rated", PyValue::int(rated as i64)),
            ]),
        ),
        (
            "descent_rate_mib_s",
            py::dict(vec![
                ("falling_samples", PyValue::int(falling.len() as i64)),
                ("p50", fl(0.50)),
                ("p90", fl(0.90)),
                ("p99", fl(0.99)),
                ("max", of(falling.last().copied(), PyValue::Float)),
            ]),
        ),
        (
            "mem_avail_mib",
            py::dict(vec![("min", of(mem.first().copied(), PyValue::int)), ("p01", mm(0.01)), ("p50", mm(0.50)), ("max", of(mem.last().copied(), PyValue::int))]),
        ),
        (
            "kill",
            py::dict(vec![
                ("trips", PyValue::int(trips.len() as i64)),
                ("threshold_mib", trips.first().and_then(|t| t.get("threshold_mib").cloned()).unwrap_or(PyValue::None)),
                ("trip_mem_mib", PyValue::List(trips.iter().map(|t| t.get("mem_avail_mib").cloned().unwrap_or(PyValue::None)).collect())),
                (
                    "latency_upper_bound_s",
                    py::dict(vec![
                        ("observed", PyValue::List(l.iter().map(|x| PyValue::Float(*x)).collect())),
                        ("max", of(l.iter().copied().reduce(|a, b| if b > a { b } else { a }), PyValue::Float)),
                    ]),
                ),
            ]),
        ),
        (
            "eta_params",
            py::dict(vec![
                ("daemon_kill_s", PyValue::int(6)),
                ("agent_act_s", PyValue::int(300)),
                ("agent_notify_s", PyValue::int(900)),
                ("eta_floor_s", PyValue::Float(6.0)),
                ("provenance", s("plan_26073109 §2.2 초기 임의값 -- Plan B 캠페인에서 실측 근거로 교체")),
            ]),
        ),
        ("counters", c.value()),
    ])
}

/// Python `"%g" % x`.
pub fn fmt_g(x: f64) -> String {
    if x.is_nan() {
        return "nan".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "inf".into() } else { "-inf".into() };
    }
    if x == 0.0 {
        return if x.is_sign_negative() {
            "-0".into()
        } else {
            "0".into()
        };
    }
    let e = format!("{:.5e}", x);
    let (mant, exp) = e.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    let strip = |t: &str| {
        if t.contains('.') {
            t.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            t.to_string()
        }
    };
    if (-4..6).contains(&exp) {
        strip(&format!("{:.*}", (5 - exp) as usize, x))
    } else {
        format!(
            "{}e{}{:02}",
            strip(mant),
            if exp < 0 { '-' } else { '+' },
            exp.abs()
        )
    }
}

fn io_raise(e: std::io::Error, p: &Path) -> Raised {
    Raised(crate::pathfmt::os_error(&e, &p.display().to_string()))
}

fn join(a: &str, b: &str) -> String {
    if b.starts_with('/') {
        b.to_string()
    } else if a.is_empty() || a.ends_with('/') {
        format!("{a}{b}")
    } else {
        format!("{a}/{b}")
    }
}

pub fn write_outputs(
    root: &str,
    node_id: &str,
    journal: &str,
    samples: &[Sample],
    events: &[Event],
    envelope: &PyValue,
    readme: &PyValue,
) -> Result<String, Raised> {
    let node_dir = join(root, node_id);
    let nd = Path::new(&node_dir);
    let (seed, evd) = (nd.join("seed"), nd.join("events"));
    for d in [&seed, &evd] {
        std::fs::create_dir_all(d).map_err(|e| io_raise(e, d))?;
    }
    let w = |p: std::path::PathBuf, t: &str| std::fs::write(&p, t).map_err(|e| io_raise(e, &p));
    w(seed.join("memwatch-journal.txt"), journal)?;
    let mut csv = String::from(CSV_HEADER);
    for x in samples {
        csv.push_str(&format!(
            "{},{},{},{},{}\n",
            fmt_z(x.ts),
            x.mem_avail,
            x.mem_min.map_or(String::new(), |v| v.to_string()),
            x.dt_s.map_or(String::new(), fmt_g),
            x.mem_rate.map_or(String::new(), fmt_g)
        ));
    }
    w(seed.join("samples_15s.csv"), &csv)?;
    let mut months: Vec<(String, Vec<&Event>)> = vec![];
    for e in events {
        let d = DateTime::from_epoch(e.ts);
        let key = format!("{}-{:02}", d.year, d.month);
        match months.iter_mut().find(|(k, _)| *k == key) {
            Some(m) => m.1.push(e),
            None => months.push((key, vec![e])),
        }
    }
    months.sort_by(|a, b| a.0.cmp(&b.0));
    for (month, evs) in months {
        let p = evd.join(format!("{month}.jsonl"));
        let mut text = String::new();
        for e in evs {
            let mut f = vec![("ts", s(&fmt_z(e.ts)))];
            f.extend(e.fields.iter().cloned());
            text.push_str(&py::dumps_unicode(&sort_keys(&py::dict(f))));
            text.push('\n');
        }
        use std::io::Write;
        let mut fh = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&p)
            .map_err(|e| io_raise(e, &p))?;
        fh.write_all(text.as_bytes()).map_err(|e| io_raise(e, &p))?;
    }
    w(
        seed.join("README.json"),
        &format!("{}\n", py::dumps_pretty(&sort_keys(readme))),
    )?;
    w(
        nd.join("envelope.json"),
        &format!("{}\n", py::dumps_pretty(&sort_keys(envelope))),
    )?;
    Ok(node_dir)
}

fn show(v: &PyValue) -> String {
    py::display(v)
}

fn self_test() -> i32 {
    let fixture = [
        "2026-07-22T17:44:56+09:00 h easy-vllm-memwatch[1]: [mem-watchdog] HB MemAvailable=40104MiB min=39537MiB 2026-07-22T08:44:56Z",
        "2026-07-22T17:45:11+09:00 h easy-vllm-memwatch[1]: [mem-watchdog] HB MemAvailable=11419MiB min=11419MiB 2026-07-22T08:45:11Z",
        "2026-07-22T17:45:14+09:00 h easy-vllm-memwatch[1]: [mem-watchdog] TRIP MemAvailable=10186MiB < 10240MiB → docker kill deadbeef 2026-07-22T08:45:14Z",
        "2026-07-22T17:45:15+09:00 h easy-vllm-memwatch[1]: [mem-watchdog] deadbeef",
        "2026-07-22T17:45:26+09:00 h easy-vllm-memwatch[1]: [mem-watchdog] HB MemAvailable=117283MiB min=10186MiB 2026-07-22T08:45:26Z",
        "2026-07-22T17:45:41+09:00 h easy-vllm-memwatch[1]: [mem-watchdog] HB MemAvailable=117300MiB 2026-07-22T08:45:41Z",
        "2026-07-22T17:45:42+09:00 h easy-vllm-memwatch[1]: [mem-watchdog] start filter='@vllm' threshold=10240MiB interval=1s heartbeat=15s pid=99 2026-07-22T08:45:42Z",
        "2026-07-22T17:45:43+09:00 h systemd[1]: Stopped easy-vllm-memwatch.service - x.",
        "2026-07-22T17:45:44+09:00 h systemd[1]: easy-vllm-memwatch.service: Consumed 1.2s CPU time.",
        "-- Boot abc123def456 --",
        "2026-07-22T17:50:00+09:00 h easy-vllm-memwatch[2]: [mem-watchdog] HB MemAvailable=118000MiB min=118000MiB 2026-07-22T08:50:00Z",
    ];
    let lines: Vec<String> = py::splitlines(&fixture.join("\n"));
    let Ok((mut samples, events, c)) = parse_journal(&lines) else {
        return 2;
    };
    compute_rates(&mut samples);
    let lat = kill_latency_bounds(&samples, &events, 20000);
    let boots: Vec<&Event> = events
        .iter()
        .filter(|e| e.kind() == "boot_marker")
        .collect();
    let rate1 = samples.get(1).and_then(|x| x.mem_rate);
    let checks: Vec<(&str, bool)> = vec![
        ("HB 5건", c.get("hb") == 5),
        ("TRIP 1건", c.get("trip") == 1),
        ("start 1건", c.get("start") == 1),
        ("unit 1건", c.get("unit") == 1),
        ("kill_ack 1건", c.get("kill_ack") == 1),
        ("boot 마커 1건", c.get("boot") == 1 && boots.len() == 1),
        (
            "boot 시각=직후 라인, 근사표기",
            boots.first().is_some_and(|b| {
                matches!(b.get("ts_approx"), Some(PyValue::Bool(true)))
                    && fmt_z(b.ts).ends_with("T08:50:00Z")
            }),
        ),
        (
            "자원회계는 ignored_known(미인식 아님)",
            c.get("ignored_known") == 1,
        ),
        ("미인식 0", c.get("unmatched") == 0),
        (
            "min 부재 정직 표기",
            samples.get(3).is_some_and(|x| x.mem_min.is_none()),
        ),
        (
            "하강률 = (11419-40104)/15",
            rate1 == Some(round_to_places((11419.0 - 40104.0) / 15.0, 2)),
        ),
        (
            "kill 지연 상한 12s",
            lat.first().is_some_and(|l| l.latency == 12.0),
        ),
        (
            "이벤트 시각 정렬",
            events.windows(2).all(|w| w[0].ts <= w[1].ts),
        ),
    ];
    let mut ok = true;
    for (name, passed) in &checks {
        println!("  [{}] {}", if *passed { "PASS" } else { "FAIL" }, name);
        ok = ok && *passed;
    }
    println!("self-test: {}", if ok { "PASS" } else { "FAIL" });
    if ok {
        0
    } else {
        2
    }
}

pub static SPEC: Spec = Spec {
    prog: "seed_from_journal.sh",
    usage: include_str!("../help/seed_from_journal.usage"),
    help: include_str!("../help/seed_from_journal.help"),
    args: &[
        HELP,
        store(&["--node-id"]),
        store(&["--journal"]),
        store(&["--logs-root"]),
        store(&["--generated-utc"]),
        store(&["--min-retention"]).float(),
        store(&["--expect-trips"]).int(),
        flag(&["--self-test"]),
    ],
    subs: None,
};

fn read_journal(path: &str) -> Result<String, Raised> {
    if path == "-" {
        // sys.stdin: no newline translation (CPython opens the standard streams with newline="\n" on POSIX)
        let mut b = vec![];
        use std::io::Read;
        std::io::stdin()
            .read_to_end(&mut b)
            .map_err(|e| Raised(format!("OSError: {e}")))?;
        return Ok(py::decode_replace(&b));
    }
    let raw = std::fs::read(path).map_err(|e| io_raise(e, Path::new(path)))?;
    // open(path, encoding="utf-8", errors="replace"): text mode with universal newlines
    Ok(py::decode_replace(&raw)
        .replace("\r\n", "\n")
        .replace('\r', "\n"))
}

pub fn main(args: &[String]) -> i32 {
    let p = match argp::parse(&SPEC, args) {
        Outcome::Ok(p) => p,
        Outcome::Exit(c) => return c,
    };
    if p.flag(7) {
        return self_test();
    }
    let node_id = p.text(1).unwrap_or("").to_string();
    let generated = p.text(4).unwrap_or("").to_string();
    let missing: Vec<&str> = [("--node-id", &node_id), ("--generated-utc", &generated)]
        .iter()
        .filter(|(_, v)| v.is_empty())
        .map(|(n, _)| *n)
        .collect();
    if !missing.is_empty() {
        return argp::error(
            &SPEC,
            &format!(
                "the following arguments are required: {}",
                missing.join(", ")
            ),
        );
    }
    if strptime_checked(&generated, "%Y-%m-%dT%H:%M:%SZ").is_err() {
        eprintln!(
            "seed_from_journal: --generated-utc must be 'YYYY-MM-DDTHH:MM:SSZ', got {}",
            argp::repr(&generated)
        );
        return 1;
    }
    match run(&p, &node_id, &generated) {
        Ok(c) => c,
        Err(Raised(m)) => crate::raise::exit(&crate::raise::Raised(m)),
    }
}

fn run(p: &argp::Parsed, node_id: &str, generated: &str) -> Result<i32, Raised> {
    let journal = read_journal(p.text(2).unwrap_or("-"))?;
    let lines = py::splitlines(&journal);
    let (mut samples, events, c) = parse_journal(&lines)?;
    compute_rates(&mut samples);
    let lat = kill_latency_bounds(&samples, &events, 20000);
    let envelope = build_envelope(node_id, generated, &samples, &events, &c, &lat);
    let total = c.get("total");
    let recognized = total - c.get("unmatched");
    let retention = if total != 0 {
        recognized as f64 / total as f64
    } else {
        0.0
    };
    let readme = py::dict(vec![
        ("schema_version", PyValue::int(SCHEMA_VERSION)),
        ("node_id", s(node_id)),
        ("generated_utc", s(generated)),
        ("purpose", s("plan_26073109 Phase 0 -- 안전체계 제거 전 포락선 초기 데이터 보존")),
        ("source", s("journalctl -u easy-vllm-memwatch --no-pager -o short-iso")),
        ("resolution_s", PyValue::int(15)),
        ("absent_fields", PyValue::List(ABSENT.iter().map(|x| s(x)).collect())),
        ("counters", c.value()),
        ("retention", PyValue::Float(round_to_places(retention, 6))),
        ("kill_latency_upper_bounds", PyValue::List(lat.iter().map(Latency::value).collect())),
        ("note", s("samples_15s.csv 는 canonical samples/ 가 아니다 -- 1초 실샘플과 혼동 금지. events/ 는 canonical 로 적재된다(실제 발생 이벤트이므로).")),
    ]);
    let root = p.text(3).unwrap_or("docs/logs");
    let node_dir = write_outputs(
        root, node_id, &journal, &samples, &events, &envelope, &readme,
    )?;
    let trips = c.get("trip");
    println!("[seed] node={node_id} -> {node_dir}");
    println!(
        "[seed] 라인 {} (인식 {} · 미인식 {}) · 보존율 {}",
        total,
        recognized,
        c.get("unmatched"),
        format_fixed(retention, 4)
    );
    println!(
        "[seed] HB {} · TRIP {} · kill_ack {} · boot {} · start {} · unit {} · TRIP-nomatch {} · 알려진무시 {}",
        c.get("hb"),
        trips,
        c.get("kill_ack"),
        c.get("boot"),
        c.get("start"),
        c.get("unit"),
        c.get("trip_nomatch"),
        c.get("ignored_known")
    );
    let d = py::get(&envelope, "descent_rate_mib_s")
        .cloned()
        .unwrap_or(PyValue::None);
    let g = |k: &str| py::get(&d, k).map_or("None".to_string(), show);
    println!(
        "[seed] 하강률 MiB/s: p50={} p90={} p99={} max={} (falling {})",
        g("p50"),
        g("p90"),
        g("p99"),
        g("max"),
        g("falling_samples")
    );
    let observed = py::get(&envelope, "kill")
        .and_then(|k| py::get(k, "latency_upper_bound_s"))
        .and_then(|l| py::get(l, "observed"))
        .cloned()
        .unwrap_or(PyValue::None);
    println!("[seed] kill 지연 상한(s): {}", observed.repr());
    let min_ret = p.float(5).unwrap_or(0.99);
    let mut failed = vec![];
    if retention < min_ret {
        failed.push(format!(
            "보존율 {} < {}",
            format_fixed(retention, 4),
            format_fixed(min_ret, 4)
        ));
    }
    if let Some(e) = p.int(6) {
        if trips != e {
            failed.push(format!("TRIP {trips} != 기대 {e}"));
        }
    }
    if !failed.is_empty() {
        eprintln!("[seed] GATE FAIL: {}", failed.join(" · "));
        return Ok(2);
    }
    println!("[seed] GATE PASS");
    Ok(0)
}
