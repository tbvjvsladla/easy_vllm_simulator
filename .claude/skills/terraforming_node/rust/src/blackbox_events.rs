//! `blackbox_events.py` — the scattered kill paths (memory watchdogs, ETA watchdog, thermal/power watchdog, liveness,
//! earlyoom, kernel OOM) folded into one event plane (`events/<YYYY-MM>.jsonl`), harvested incrementally from journald
//! with per-source cursors (`events/.cursors.json`), plus the unclean-boot scan. Same flags, exit codes, texts and
//! files as the original; journald is read through `journalctl` exactly as the original calls it.
//!
//! Exit codes: 0 = success · 2 = usage error / self-test failed.

use crate::argp::{self, flag, store, Arg, Outcome, Spec, HELP};
use crate::bb::{self, Fail, R};
use crate::py;
use crate::pyre::{Match, Regex};
use crate::raise::Raised;
use agent_runtime_support::pynumber;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::sync::OnceLock;

pub const SCHEMA_VERSION: i64 = 1;
pub const CURSOR_FILE: &str = ".cursors.json";

/// `_num_or_none(text)`: '90' -> 90 · '90.5' -> 90.5 · 'na' (absent) -> None (never folded into 0).
pub fn num_or_none(text: Option<&str>) -> PyValue {
    let Some(t) = text.filter(|t| *t != "na") else {
        return PyValue::None;
    };
    if !t.contains('.') {
        match pynumber::int_from_str(&PyStr::new(t)) {
            Ok(n) => PyValue::Int(n),
            Err(_) => PyValue::None,
        }
    } else {
        match pynumber::float_from_str(&PyStr::new(t)) {
            Ok(x) => PyValue::Float(x),
            Err(_) => PyValue::None,
        }
    }
}

fn int_g(m: &Match, i: usize) -> PyValue {
    // groups are `\d+` / `-?\d+`: Python `int()` of them always succeeds
    match pynumber::int_from_str(&PyStr::new(m.group(i).unwrap_or(""))) {
        Ok(n) => PyValue::Int(n),
        Err(_) => PyValue::None,
    }
}
fn s_g(m: &Match, i: usize) -> PyValue {
    py::s(m.group(i).unwrap_or(""))
}
fn strip_g(m: &Match, i: usize) -> PyValue {
    py::s(py::strip(m.group(i).unwrap_or("")))
}

type Build = fn(&Match) -> Vec<(&'static str, PyValue)>;

fn parsers() -> &'static Vec<(Regex, Build)> {
    static P: OnceLock<Vec<(Regex, Build)>> = OnceLock::new();
    P.get_or_init(|| {
        let v: Vec<(&str, Build)> = vec![
            (
                r"\[(?:mem|bb)-watchdog\]\s+TRIP\s+MemAvailable=(\d+)MiB\s+<\s+(\d+)MiB.*?docker kill\s+(\S+)",
                |m| {
                    vec![
                        ("kind", py::s("watchdog_trip")),
                        ("mem_avail_mib", int_g(m, 1)),
                        ("threshold_mib", int_g(m, 2)),
                        ("target", s_g(m, 3)),
                        ("target_kind", py::s("container")),
                        ("action", py::s("docker_kill")),
                        ("rule", py::s("absolute")),
                    ]
                },
            ),
            (
                r"\[mem-watchdog\]\s+TRIP\s+MemAvailable=(\d+)MiB\s+<\s+(\d+)MiB\s+→\s+target_kind=pgid\s+pgid=(\d+)\s+starttime=(\d+)",
                |m| {
                    vec![
                        ("kind", py::s("watchdog_trip")),
                        ("mem_avail_mib", int_g(m, 1)),
                        ("threshold_mib", int_g(m, 2)),
                        ("target", py::s(&format!("pgid:{}", m.group(3).unwrap_or("")))),
                        ("target_kind", py::s("pgid")),
                        ("pgid", int_g(m, 3)),
                        ("starttime", int_g(m, 4)),
                        ("action", py::s("pgid_term_kill")),
                        ("rule", py::s("absolute")),
                    ]
                },
            ),
            (
                r"\[bb-watchdog\]\s+TRIP\s+mem=(\d+)MiB\s+rate=(-?\d+)MiB/s\s+streak=(\d+)\s+→\s+docker kill\s+(.+?)\s+\d{4}-",
                |m| {
                    vec![
                        ("kind", py::s("watchdog_trip")),
                        ("mem_avail_mib", int_g(m, 1)),
                        ("rate_mib_s", int_g(m, 2)),
                        ("streak", int_g(m, 3)),
                        ("target", strip_g(m, 4)),
                        ("target_kind", py::s("container")),
                        ("action", py::s("docker_kill")),
                        ("rule", py::s("eta")),
                    ]
                },
            ),
            (
                r"\[(?:mem|bb)-watchdog\]\s+TRIP-nomatch\s+(?:MemAvailable=|mem=)(\d+)MiB",
                |m| {
                    vec![
                        ("kind", py::s("watchdog_trip_nomatch")),
                        ("mem_avail_mib", int_g(m, 1)),
                        ("action", py::s("none")),
                    ]
                },
            ),
            (
                r"\[bb-watchdog\]\s+TRIP-ARM\s+mem=(\d+)MiB\s+rate=(-?\d+)MiB/s",
                |m| {
                    vec![
                        ("kind", py::s("watchdog_trip_arm")),
                        ("mem_avail_mib", int_g(m, 1)),
                        ("rate_mib_s", int_g(m, 2)),
                    ]
                },
            ),
            (
                r"\[tp-watchdog\]\s+TRIP\s+rule=(\S+)\s+gpu=(\S+?)W\s+soc=(\S+?)C\s+buckets=(\d+)/(\d+)\s+streak=(\d+)\s+→\s+docker kill\s+(.+?)\s+\d{4}-",
                |m| {
                    vec![
                        ("kind", py::s("thermal_trip")),
                        ("rule", s_g(m, 1)),
                        ("gpu_pwr_w", num_or_none(m.group(2))),
                        ("soc_temp_c", num_or_none(m.group(3))),
                        ("gpu_bucket", int_g(m, 4)),
                        ("soc_bucket", int_g(m, 5)),
                        ("soc_hard_streak", int_g(m, 6)),
                        ("target", strip_g(m, 7)),
                        ("action", py::s("docker_kill")),
                    ]
                },
            ),
            (
                r"\[tp-watchdog\]\s+TRIP-DRYRUN\s+rule=(\S+)\s+gpu=(\S+?)W\s+soc=(\S+?)C\s+buckets=(\d+)/(\d+)",
                |m| {
                    vec![
                        ("kind", py::s("thermal_trip_dryrun")),
                        ("rule", s_g(m, 1)),
                        ("gpu_pwr_w", num_or_none(m.group(2))),
                        ("soc_temp_c", num_or_none(m.group(3))),
                        ("gpu_bucket", int_g(m, 4)),
                        ("soc_bucket", int_g(m, 5)),
                        ("action", py::s("none")),
                    ]
                },
            ),
            (
                r"\[tp-watchdog\]\s+TRIP-nomatch\s+rule=(\S+)\s+gpu=(\S+?)W\s+soc=(\S+?)C",
                |m| {
                    vec![
                        ("kind", py::s("thermal_trip_nomatch")),
                        ("rule", s_g(m, 1)),
                        ("gpu_pwr_w", num_or_none(m.group(2))),
                        ("soc_temp_c", num_or_none(m.group(3))),
                        ("action", py::s("none")),
                    ]
                },
            ),
            (
                r"\[tp-watchdog\]\s+GPU-STALE\s+수집기 꼬리가\s+(\d+)s\s+뒤짐",
                |m| {
                    vec![
                        ("kind", py::s("thermal_gpu_stale")),
                        ("lag_s", int_g(m, 1)),
                        ("axis", py::s("frozen")),
                    ]
                },
            ),
            (r"\[tp-watchdog\]\s+GPU-STALE\s+해제", |_| {
                vec![
                    ("kind", py::s("thermal_gpu_stale_clear")),
                    ("axis", py::s("resumed")),
                ]
            }),
            (
                r"\[tp-watchdog\]\s+start\s+mode=(\S+)\s+filter='([^']*)'",
                |m| {
                    vec![
                        ("kind", py::s("thermal_watchdog_start")),
                        ("mode", s_g(m, 1)),
                        ("filter", s_g(m, 2)),
                    ]
                },
            ),
            (
                r"\[liveness\]\s+TRIP\s+엔진교착.*?정체\s+(\d+)s.*?docker kill\s+(\S+)",
                |m| {
                    vec![
                        ("kind", py::s("liveness_trip")),
                        ("stalled_s", int_g(m, 1)),
                        ("target", s_g(m, 2)),
                        ("action", py::s("docker_kill")),
                    ]
                },
            ),
            (
                r#"sending SIG(TERM|KILL) to process (\d+)\s+uid\s+\d+\s+"([^"]*)""#,
                |m| {
                    vec![
                        ("kind", py::s("earlyoom_kill")),
                        ("signal", py::s(&format!("SIG{}", m.group(1).unwrap_or("")))),
                        ("pid", int_g(m, 2)),
                        ("process", s_g(m, 3)),
                        ("action", py::s("signal")),
                    ]
                },
            ),
            (
                r"Out of memory: Killed process (\d+) \(([^)]*)\)",
                |m| {
                    vec![
                        ("kind", py::s("kernel_oom_kill")),
                        ("pid", int_g(m, 1)),
                        ("process", s_g(m, 2)),
                        ("action", py::s("kernel_kill")),
                    ]
                },
            ),
            (
                r"\[(?:mem|bb)-watchdog\]\s+start\s+filter='([^']*)'",
                |m| {
                    let f = m.group(1).unwrap_or("");
                    vec![
                        ("kind", py::s("watchdog_start")),
                        ("filter", py::s(f)),
                        (
                            "target_kind",
                            py::s(if f.starts_with("pgid:") { "pgid" } else { "container" }),
                        ),
                    ]
                },
            ),
        ];
        v.into_iter().map(|(p, b)| (Regex::new(p), b)).collect()
    })
}

fn re(pat: &'static str, cell: &'static OnceLock<Regex>) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pat))
}

/// `parse_message(msg)`: a journald MESSAGE body -> normalized event, or None.
pub fn parse_message(msg: &str) -> Option<PyDict> {
    if msg.is_empty() {
        return None;
    }
    let mk = |pairs: Vec<(&str, PyValue)>| {
        let mut d = PyDict::new();
        for (k, v) in pairs {
            d.set(k, v);
        }
        d
    };
    for (rx, build) in parsers() {
        if let Some(m) = rx.search(msg) {
            return Some(mk(build(&m)));
        }
    }
    static ACK: OnceLock<Regex> = OnceLock::new();
    static TP_ACK: OnceLock<Regex> = OnceLock::new();
    let stripped = py::strip(msg);
    if let Some(m) = re(r"^\[(?:mem|bb)-watchdog\]\s+([0-9a-f]{8,})\s*$", &ACK).match_(stripped) {
        return Some(mk(vec![
            ("kind", py::s("watchdog_kill_ack")),
            ("target", s_g(&m, 1)),
        ]));
    }
    if let Some(m) = re(r"^\[tp-watchdog\]\s+([0-9a-f]{8,})\s*$", &TP_ACK).match_(stripped) {
        return Some(mk(vec![
            ("kind", py::s("thermal_kill_ack")),
            ("target", s_g(&m, 1)),
        ]));
    }
    None
}

/// `re.search(CLEAN_SHUTDOWN, text, re.IGNORECASE)`.
fn clean_shutdown(text: &str) -> bool {
    static CS: OnceLock<Regex> = OnceLock::new();
    let lower = text.to_lowercase();
    re(
        r"(systemd-shutdown|reached target\s+(power-off|reboot|shutdown|halt)|shutting down|powering off|rebooting)",
        &CS,
    )
    .search(&lower)
    .is_some()
}

/// `datetime.fromtimestamp(us / 1e6, tz=utc).strftime("%Y-%m-%dT%H:%M:%SZ")`, None out of range.
fn iso_from_seconds(x: f64) -> Option<String> {
    if !x.is_finite() {
        return None;
    }
    let mut t = x.trunc();
    let frac = x - t;
    let us = (frac * 1e6).round_ties_even();
    if us >= 1e6 {
        t += 1.0;
    } else if us < 0.0 {
        t -= 1.0;
    }
    if !(-62135596800.0..=253402300799.0).contains(&t) {
        return None;
    }
    Some(crate::pytime::DateTime::from_epoch(t as i64).iso_z())
}

fn int_value(v: &PyValue) -> Option<i64> {
    match pynumber::int_call(v.peel()) {
        Ok(n) => i64::try_from(n).ok(),
        Err(_) => None,
    }
}

/// `_us_to_iso(value)`: journald integer microseconds -> ISO-Z (a string passes through).
pub fn us_to_iso(value: &PyValue) -> PyValue {
    match value.peel() {
        PyValue::None => PyValue::None,
        PyValue::Str(_) => value.clone(),
        _ => match int_value(value).and_then(|n| iso_from_seconds(n as f64 / 1e6)) {
            Some(s) => py::s(&s),
            None => PyValue::None,
        },
    }
}

fn ts_from_record(rec: &PyValue) -> Result<Option<String>, Raised> {
    let Some(us) = py::get(rec, "__REALTIME_TIMESTAMP").filter(|v| v.is_truthy()) else {
        return Ok(None);
    };
    match pynumber::int_call(us.peel()) {
        Ok(n) => Ok(i64::try_from(n)
            .ok()
            .and_then(|n| iso_from_seconds(n as f64 / 1e6))),
        Err(e) if e.class == "TypeError" => Err(Raised::new(e.class, &e.message)),
        Err(_) => Ok(None),
    }
}

/// `normalize(rec, source)`: a journald record -> event (MESSAGE may be a byte list).
pub fn normalize(rec: &PyValue, source: &str) -> Result<Option<PyDict>, Raised> {
    let msg = match py::get(rec, "MESSAGE") {
        Some(PyValue::List(l)) => {
            let mut bytes = vec![];
            let mut ok = true;
            for x in l {
                match int_value(x).filter(|_| matches!(x.peel(), PyValue::Int(_) | PyValue::Bool(_))) {
                    Some(n) if (0..=255).contains(&n) => bytes.push(n as u8),
                    _ => {
                        ok = false;
                        break;
                    }
                }
            }
            if ok {
                String::from_utf8_lossy(&bytes).into_owned()
            } else {
                String::new()
            }
        }
        Some(PyValue::Str(s)) => py::text(s),
        Some(v) if !v.is_truthy() => String::new(),
        None => String::new(),
        Some(v) => {
            return Err(Raised::new(
                "TypeError",
                &format!("expected string or bytes-like object, got '{}'", v.type_name()),
            ))
        }
    };
    let Some(mut ev) = parse_message(&msg) else {
        return Ok(None);
    };
    let ts = ts_from_record(rec)?;
    ev.set("ts", py::opt_str(ts.as_deref()));
    ev.set("source", py::s(source));
    if let Some(b) = py::get(rec, "_BOOT_ID").filter(|v| v.is_truthy()) {
        ev.set("boot_id", b.clone());
    }
    Ok(Some(ev))
}

/// One boot of `collect_boot_summaries`.
#[derive(Clone)]
pub struct Boot {
    pub boot_id: PyValue,
    pub is_current: bool,
    pub first_ts: PyValue,
    pub last_ts: PyValue,
    pub tail_messages: Vec<String>,
}

/// `detect_unclean_boots(boots)`: non-current boots whose tail has no clean shutdown marker.
pub fn detect_unclean_boots(boots: &[Boot]) -> Vec<PyDict> {
    let mut out = vec![];
    for b in boots {
        if b.is_current {
            continue;
        }
        if !clean_shutdown(&b.tail_messages.join("\n")) {
            let mut d = PyDict::new();
            d.set("kind", py::s("unclean_boot"));
            d.set("boot_id", b.boot_id.clone());
            d.set("first_ts", b.first_ts.clone());
            d.set("last_ts", b.last_ts.clone());
            d.set("reason", py::s("no_clean_shutdown_marker"));
            out.push(d);
        }
    }
    out
}

fn run_capture(cmd: &[String], timeout: u64) -> Option<String> {
    crate::proc::run(cmd, None, Some(timeout)).ok().map(|c| c.stdout)
}

/// `journal_read(unit, kernel, after_cursor)`: (records, last cursor); a failed run is ([], after_cursor).
fn journal_read(
    unit: Option<&str>,
    kernel: bool,
    after: Option<&PyValue>,
) -> Result<(Vec<PyValue>, Option<PyValue>), Raised> {
    let mut cmd: Vec<String> = ["journalctl", "-o", "json", "--no-pager"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    if kernel {
        cmd.push("-k".into());
    }
    if let Some(u) = unit {
        cmd.push("-u".into());
        cmd.push(u.into());
    }
    match after.filter(|a| a.is_truthy()) {
        Some(a) => {
            cmd.push("--after-cursor".into());
            cmd.push(bb::s(a));
        }
        None => {
            cmd.push("-n".into());
            cmd.push("20000".into());
        }
    }
    let Some(out) = run_capture(&cmd, 120) else {
        return Ok((vec![], after.cloned()));
    };
    let mut rows = vec![];
    let mut cursor = after.cloned();
    for line in py::splitlines(&out) {
        let line = py::strip(&line);
        if line.is_empty() {
            continue;
        }
        let Ok(rec) = agent_runtime_support::pyjson::loads(line) else {
            continue;
        };
        if !matches!(rec.peel(), PyValue::Dict(_)) {
            return Err(Raised::new(
                "AttributeError",
                &format!("'{}' object has no attribute 'get'", rec.type_name()),
            ));
        }
        if let Some(c) = py::get(&rec, "__CURSOR") {
            cursor = Some(c.clone());
        }
        rows.push(rec);
    }
    Ok((rows, cursor))
}

fn collect_boot_summaries(tail_n: usize) -> Vec<Boot> {
    let cmd: Vec<String> = ["journalctl", "--list-boots", "-o", "json", "--no-pager"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let Some(raw) = run_capture(&cmd, 60) else {
        return vec![];
    };
    let boots = if py::strip(&raw).is_empty() {
        PyValue::List(vec![])
    } else {
        match agent_runtime_support::pyjson::loads(&raw) {
            Ok(v) => v,
            Err(_) => return vec![],
        }
    };
    let PyValue::List(boots) = boots.peel() else {
        return vec![];
    };
    let mut out = vec![];
    for b in boots {
        let idx = py::get(b, "index").cloned().unwrap_or(PyValue::None);
        let bid = py::get(b, "boot_id").cloned().unwrap_or(PyValue::None);
        let tail = run_capture(
            &[
                "journalctl".into(),
                "-b".into(),
                bb::s(&idx),
                "-n".into(),
                tail_n.to_string(),
                "-o".into(),
                "cat".into(),
                "--no-pager".into(),
            ],
            60,
        )
        .unwrap_or_default();
        let is_current = match idx.peel() {
            PyValue::Int(_) | PyValue::Bool(_) => int_value(&idx) == Some(0),
            PyValue::Float(x) => *x == 0.0,
            _ => false,
        };
        out.push(Boot {
            boot_id: bid,
            is_current,
            first_ts: us_to_iso(py::get(b, "first_entry").unwrap_or(&PyValue::None)),
            last_ts: us_to_iso(py::get(b, "last_entry").unwrap_or(&PyValue::None)),
            tail_messages: py::splitlines(&tail),
        });
    }
    out
}

/// `load_cursors(events_dir)`: `{}` when absent or unreadable.
pub fn load_cursors(events_dir: &str) -> PyValue {
    let p = bb::join(events_dir, CURSOR_FILE);
    if std::fs::metadata(&p).is_ok() {
        return bb::load_json(&p).unwrap_or_else(|_| PyValue::Dict(PyDict::new()));
    }
    PyValue::Dict(PyDict::new())
}

/// `save_cursors(events_dir, cursors)`: sorted-key indent-2 JSON, atomic, owner inherited.
pub fn save_cursors(events_dir: &str, cursors: &PyValue) -> Result<(), Raised> {
    bb::makedirs_as_ancestor_owner(events_dir)?;
    let p = bb::join(events_dir, CURSOR_FILE);
    bb::write_atomic(&p, &(bb::dumps_sorted_pretty(cursors) + "\n"))?;
    bb::inherit_dir_owner(&p, events_dir);
    Ok(())
}

/// `append_events(events_dir, events)`: grouped by month, months in sorted order.
pub fn append_events(events_dir: &str, events: &[PyDict]) -> Result<(), Raised> {
    bb::makedirs_as_ancestor_owner(events_dir)?;
    let mut by_month: Vec<(String, Vec<&PyDict>)> = vec![];
    for e in events {
        let ts = match e.get("ts") {
            Some(v) if v.is_truthy() => bb::s(v),
            _ => "0000-00".into(),
        };
        let month: String = ts.chars().take(7).collect();
        match by_month.iter_mut().find(|(m, _)| *m == month) {
            Some((_, v)) => v.push(e),
            None => by_month.push((month, vec![e])),
        }
    }
    by_month.sort_by(|a, b| a.0.cmp(&b.0));
    for (month, evs) in by_month {
        let path = bb::join(events_dir, &format!("{month}.jsonl"));
        let mut body = String::new();
        for e in evs {
            body.push_str(&bb::dumps_sorted(&PyValue::Dict(e.clone())));
            body.push('\n');
        }
        bb::append(&path, &body)?;
        bb::inherit_dir_owner(&path, events_dir);
    }
    Ok(())
}

pub const DEFAULT_SOURCES: [(Option<&str>, bool); 7] = [
    (Some("easy-vllm-blackbox-watchdog"), false),
    (Some("easy-vllm-blackbox-thermal"), false),
    (Some("easy-vllm-memwatch"), false),
    (Some("easy-vllm-blackbox-lifecycle"), false),
    (Some("easy-vllm-blackbox-collect"), false),
    (Some("earlyoom"), false),
    (None, true),
];

fn attr_err(v: &PyValue) -> Raised {
    Raised::new(
        "AttributeError",
        &format!("'{}' object has no attribute 'get'", v.type_name()),
    )
}

/// `collect(node_dir, detect_boots)`: harvest every source after its cursor; returns the new events.
pub fn collect(node_dir: &str, detect_boots: bool) -> Result<Vec<PyDict>, Raised> {
    let events_dir = bb::join(node_dir, "events");
    let cv = load_cursors(&events_dir);
    let PyValue::Dict(mut cursors) = cv.peel().clone() else {
        return Err(attr_err(&cv));
    };
    let mut found: Vec<PyDict> = vec![];
    for (unit, kernel) in DEFAULT_SOURCES {
        let key = if kernel {
            "kernel".to_string()
        } else {
            format!("unit:{}", unit.unwrap_or("None"))
        };
        let (rows, cursor) = journal_read(unit, kernel, cursors.get(&key))?;
        for rec in &rows {
            if let Some(ev) = normalize(rec, &key)? {
                found.push(ev);
            }
        }
        if let Some(c) = cursor.filter(|c| c.is_truthy()) {
            cursors.set(&key, c);
        }
    }
    if detect_boots {
        let mut seen: Vec<PyValue> = match cursors.get("_unclean_boots") {
            None => vec![],
            Some(PyValue::List(l)) => {
                let mut v: Vec<PyValue> = vec![];
                for x in l {
                    if !v.contains(x) {
                        v.push(x.clone());
                    }
                }
                v
            }
            Some(PyValue::Str(s)) => {
                let mut v: Vec<PyValue> = vec![];
                for c in py::text(s).chars() {
                    let x = py::s(&c.to_string());
                    if !v.contains(&x) {
                        v.push(x);
                    }
                }
                v
            }
            Some(other) => {
                return Err(Raised::new(
                    "TypeError",
                    &format!("'{}' object is not iterable", other.type_name()),
                ))
            }
        };
        for mut ub in detect_unclean_boots(&collect_boot_summaries(40)) {
            let bid = ub.get("boot_id").cloned().unwrap_or(PyValue::None);
            if seen.contains(&bid) {
                continue;
            }
            seen.push(bid);
            ub.set("source", py::s("boot_scan"));
            let lt = ub.get("last_ts").cloned().unwrap_or(PyValue::None);
            ub.set("ts", lt);
            found.push(ub);
        }
        // `sorted(seen)`: boot ids are strings (journald); mixed types would make Python raise
        let mut texts: Vec<(Vec<u32>, PyValue)> = vec![];
        for v in seen {
            match v.peel() {
                PyValue::Str(s) => texts.push((s.code_points().to_vec(), v.clone())),
                other => {
                    if texts.is_empty() && !matches!(other, PyValue::Str(_)) {
                        texts.push((vec![], v.clone()));
                    } else {
                        return Err(Raised::new(
                            "TypeError",
                            "'<' not supported between instances",
                        ));
                    }
                }
            }
        }
        texts.sort_by(|a, b| a.0.cmp(&b.0));
        cursors.set(
            "_unclean_boots",
            PyValue::List(texts.into_iter().map(|x| x.1).collect()),
        );
    }
    if !found.is_empty() {
        append_events(&events_dir, &found)?;
    }
    save_cursors(&events_dir, &PyValue::Dict(cursors))?;
    Ok(found)
}

fn boot(id: &str, cur: bool, first: PyValue, last: PyValue, tail: &[&str]) -> Boot {
    Boot {
        boot_id: py::s(id),
        is_current: cur,
        first_ts: first,
        last_ts: last,
        tail_messages: tail.iter().map(|s| s.to_string()).collect(),
    }
}

fn self_test() -> i32 {
    let mut checks: Vec<(String, bool)> = vec![];
    let mut c = |n: &str, b: bool| checks.push((n.to_string(), b));
    let gs = |e: &Option<PyDict>, k: &str| -> Option<String> {
        e.as_ref()
            .and_then(|d| d.get(k))
            .and_then(|v| match v.peel() {
                PyValue::Str(s) => Some(py::text(s)),
                _ => None,
            })
    };
    let gi = |e: &Option<PyDict>, k: &str| -> Option<i64> {
        e.as_ref().and_then(|d| d.get(k)).and_then(|v| match v.peel() {
            PyValue::Int(n) => i64::try_from(n.clone()).ok(),
            _ => None,
        })
    };
    let kind_of = |m: &str| gs(&parse_message(m), "kind");
    let is = |e: &Option<PyDict>, k: &str, v: &str| gs(e, k).as_deref() == Some(v);

    let e = parse_message("[mem-watchdog] TRIP MemAvailable=10186MiB < 10240MiB → docker kill 249d24630725 2026-07-22T08:48:59Z");
    c(
        "레거시 TRIP 파싱",
        is(&e, "kind", "watchdog_trip")
            && gi(&e, "mem_avail_mib") == Some(10186)
            && is(&e, "target", "249d24630725")
            && is(&e, "rule", "absolute"),
    );
    let e = parse_message("[bb-watchdog] TRIP mem=37000MiB rate=6241MiB/s streak=3 → docker kill abc123 def456 2026-07-31T00:00:00Z");
    c(
        "ETA TRIP 파싱(rate·streak 보존)",
        is(&e, "kind", "watchdog_trip")
            && is(&e, "rule", "eta")
            && gi(&e, "rate_mib_s") == Some(6241)
            && gi(&e, "streak") == Some(3)
            && is(&e, "target", "abc123 def456"),
    );
    let e = parse_message("[bb-watchdog] TRIP-ARM mem=40000MiB rate=6000MiB/s (디바운스 1/3) 2026-07-31T00:00:00Z");
    c("TRIP-ARM(디바운스 진행) 파싱", is(&e, "kind", "watchdog_trip_arm"));
    c(
        "TRIP-nomatch 파싱",
        kind_of("[bb-watchdog] TRIP-nomatch mem=5000MiB rate=10MiB/s (filter='@vllm' 매칭 0)").as_deref()
            == Some("watchdog_trip_nomatch"),
    );
    c(
        "kill-ack 파싱",
        kind_of("[mem-watchdog] 249d24630725").as_deref() == Some("watchdog_kill_ack"),
    );
    c(
        "start 파싱",
        kind_of("[bb-watchdog] start filter='@vllm' interval=1s").as_deref() == Some("watchdog_start"),
    );
    let e = parse_message("[mem-watchdog] TRIP MemAvailable=9000MiB < 10240MiB → target_kind=pgid pgid=4242 starttime=123456 SIGTERM→2s→SIGKILL 2026-09-23T00:00:00Z");
    c(
        "pgid TRIP 파싱(target_kind=pgid · docker_kill 아님)",
        is(&e, "kind", "watchdog_trip")
            && is(&e, "target_kind", "pgid")
            && gi(&e, "pgid") == Some(4242)
            && gi(&e, "starttime") == Some(123456)
            && is(&e, "action", "pgid_term_kill"),
    );
    let e = parse_message("[mem-watchdog] TRIP MemAvailable=10186MiB < 10240MiB → docker kill 249d24630725 2026-07-22T08:48:59Z");
    c("컨테이너 TRIP 은 target_kind=container", is(&e, "target_kind", "container"));
    let e = parse_message("[mem-watchdog] start filter='pgid:4242' target_kind=pgid pgid=4242 starttime=1 threshold=10240MiB");
    c(
        "pgid start 파싱 target_kind=pgid",
        is(&e, "kind", "watchdog_start") && is(&e, "target_kind", "pgid"),
    );
    let e = parse_message("[tp-watchdog] TRIP rule=gpu_pwr_sustained gpu=90W soc=47C buckets=60/0 streak=0 → docker kill cafe1234 2026-08-23T09:24:36Z");
    c(
        "열·전력 TRIP 파싱(축·버킷 보존)",
        is(&e, "kind", "thermal_trip")
            && is(&e, "rule", "gpu_pwr_sustained")
            && gi(&e, "gpu_pwr_w") == Some(90)
            && gi(&e, "soc_temp_c") == Some(47)
            && gi(&e, "gpu_bucket") == Some(60)
            && is(&e, "target", "cafe1234")
            && is(&e, "action", "docker_kill"),
    );
    let e = parse_message("[tp-watchdog] TRIP rule=soc_hard_ceiling gpu=45W soc=97C buckets=0/12 streak=3 → docker kill a1b2c3 d4e5f6 2026-08-23T09:24:36Z");
    c(
        "열·전력 TRIP: SoC 즉시계층 규칙 라벨·다중 타깃",
        is(&e, "rule", "soc_hard_ceiling")
            && gi(&e, "soc_hard_streak") == Some(3)
            && is(&e, "target", "a1b2c3 d4e5f6"),
    );
    let e = parse_message("[tp-watchdog] TRIP rule=gpu_pwr_sustained gpu=90W soc=naC buckets=60/0 streak=0 → docker kill z9 2026-08-23T09:24:36Z");
    c(
        "열·전력 TRIP: 부재(na)는 None(0 으로 접지 않는다)",
        e.as_ref()
            .is_some_and(|d| matches!(d.get("soc_temp_c"), Some(PyValue::None)))
            && gi(&e, "gpu_pwr_w") == Some(90),
    );
    let e = parse_message("[tp-watchdog] TRIP-DRYRUN rule=gpu_pwr_sustained gpu=90W soc=47C buckets=60/0 streak=0 → 실무장이었으면 docker kill cafe1234 2026-08-23T09:24:36Z");
    c(
        "열·전력 dry-run 은 별도 kind(하류 오독 방지)",
        is(&e, "kind", "thermal_trip_dryrun") && is(&e, "action", "none"),
    );
    c(
        "★dry-run 이 실무장 kind 로 새지 않는다",
        kind_of("[tp-watchdog] TRIP-DRYRUN rule=gpu_pwr_sustained gpu=90W soc=47C buckets=60/0 streak=0 → 실무장이었으면 docker kill x 2026-08-23T09:24:36Z").as_deref()
            != Some("thermal_trip"),
    );
    c(
        "열·전력 TRIP-nomatch 파싱",
        kind_of("[tp-watchdog] TRIP-nomatch rule=gpu_pwr_sustained gpu=90W soc=47C (filter='@vllm' 매칭 0) 2026-08-23T09:24:36Z").as_deref()
            == Some("thermal_trip_nomatch"),
    );
    let e = parse_message("[tp-watchdog] GPU-STALE 수집기 꼬리가 37s 뒤짐(>10s) — 전력 축 동결. 수집기 확인: systemctl status easy-vllm-blackbox-collect 2026-08-23T09:24:36Z");
    c(
        "전력 축 동결(stale) 파싱 — 조용한 무장해제를 사후에 본다",
        is(&e, "kind", "thermal_gpu_stale") && gi(&e, "lag_s") == Some(37) && is(&e, "axis", "frozen"),
    );
    c(
        "stale 해제 파싱",
        kind_of("[tp-watchdog] GPU-STALE 해제 — 전력 축 재개 2026-08-23T09:24:36Z").as_deref()
            == Some("thermal_gpu_stale_clear"),
    );
    let e = parse_message("[tp-watchdog] start mode=armed filter='@vllm' interval=1s params=/etc/easy-vllm/thermal_params.env node=/x/y");
    c(
        "열·전력 워치독 start 파싱(mode 보존)",
        is(&e, "kind", "thermal_watchdog_start") && is(&e, "mode", "armed") && is(&e, "filter", "@vllm"),
    );
    c(
        "열·전력 kill-ack 파싱(축별로 갈린다)",
        kind_of("[tp-watchdog] cafe1234deadbeef").as_deref() == Some("thermal_kill_ack"),
    );
    c(
        "RAM 축 kill-ack 은 여전히 RAM kind(축 혼선 없음)",
        kind_of("[bb-watchdog] cafe1234deadbeef").as_deref() == Some("watchdog_kill_ack"),
    );
    c(
        "★열·전력 유닛이 수집 소스에 배선됨(배선 실증)",
        DEFAULT_SOURCES.contains(&(Some("easy-vllm-blackbox-thermal"), false)),
    );
    let e = parse_message("[liveness] TRIP 엔진교착: running=1 reachable=1 progress 정체 600s(>=600) → docker kill mn-x 2026-07-31T00:00:00Z");
    c(
        "liveness TRIP 파싱",
        is(&e, "kind", "liveness_trip") && gi(&e, "stalled_s") == Some(600) && is(&e, "target", "mn-x"),
    );
    let e = parse_message("earlyoom[123]: sending SIGTERM to process 4567 uid 0 \"VLLM::EngineCore\": badness 100, VmRSS 90000 MiB");
    c(
        "earlyoom 파싱",
        is(&e, "kind", "earlyoom_kill") && gi(&e, "pid") == Some(4567) && is(&e, "process", "VLLM::EngineCore"),
    );
    let e = parse_message("Out of memory: Killed process 8901 (python3) total-vm:100000kB, anon-rss:50000kB");
    c(
        "커널 OOM 파싱",
        is(&e, "kind", "kernel_oom_kill") && gi(&e, "pid") == Some(8901),
    );
    c("무관 라인 -> None", parse_message("Started something.service").is_none());
    c("빈 입력 -> None", parse_message("").is_none());

    let boots = [
        boot("b1", false, PyValue::int(1), PyValue::int(2), &["foo", "systemd-shutdown[1]: Powering off.", "bar"]),
        boot("b2", false, PyValue::int(3), PyValue::int(4), &["[mem-watchdog] HB MemAvailable=24000MiB", "kernel: something"]),
        boot("b3", true, PyValue::int(5), PyValue::int(6), &["still running"]),
    ];
    let ub = detect_unclean_boots(&boots);
    let has = |id: &str| ub.iter().any(|x| x.get("boot_id") == Some(&py::s(id)));
    c("정상 종료 부팅은 제외", !has("b1"));
    c("종료 흔적 없는 부팅 = 부정클린", has("b2"));
    c("현재 부팅은 판정 제외", !has("b3"));
    c(
        "'Reached target Reboot' 도 정상으로 인정",
        detect_unclean_boots(&[boot("b4", false, PyValue::None, PyValue::None, &["Reached target Reboot."])]).is_empty(),
    );
    c(
        "epoch 0 -> 1970-01-01T00:00:00Z",
        us_to_iso(&PyValue::int(0)) == py::s("1970-01-01T00:00:00Z"),
    );
    let probe: i64 = 1785464193;
    let back = match us_to_iso(&PyValue::int(probe * 1_000_000)) {
        PyValue::Str(s) => crate::pytime::strptime(&py::text(&s), "%Y-%m-%dT%H:%M:%SZ").map(|d| d.epoch()),
        _ => None,
    };
    c("정수 μs 변환 왕복 일치", back == Some(probe));
    c(
        "문자열은 그대로 통과",
        us_to_iso(&py::s("2026-07-31T00:00:00Z")) == py::s("2026-07-31T00:00:00Z"),
    );
    c("None 은 None", matches!(us_to_iso(&PyValue::None), PyValue::None));
    let ub_int = detect_unclean_boots(&[boot(
        "b5",
        false,
        PyValue::None,
        us_to_iso(&PyValue::int(1785464193000000i64)),
        &["nothing clean here"],
    )]);
    c(
        "부정클린 이벤트의 ts 가 문자열",
        !ub_int.is_empty() && matches!(ub_int[0].get("last_ts"), Some(PyValue::Str(_))),
    );

    let td = bb::TempDir::new();
    let nd = bb::join(td.path(), "spark-x");
    let ed = bb::join(&nd, "events");
    let rec = py::dict(vec![
        (
            "MESSAGE",
            py::s("[bb-watchdog] TRIP mem=5000MiB rate=2000MiB/s streak=3 → docker kill zz 2026-07-31T00:00:00Z"),
        ),
        ("__REALTIME_TIMESTAMP", py::s("1785464193000000")),
        ("_BOOT_ID", py::s("bootA")),
        ("__CURSOR", py::s("s=1;i=2")),
    ]);
    let ev = normalize(&rec, "unit:test").unwrap_or(None);
    c(
        "정규화: ts UTC 변환",
        gs(&ev, "ts").is_some_and(|t| t.ends_with('Z') && t.starts_with("2026-")),
    );
    c("정규화: boot_id 보존", is(&ev, "boot_id", "bootA"));
    c("정규화: source 부착", is(&ev, "source", "unit:test"));
    let rec_bytes = py::dict(vec![
        (
            "MESSAGE",
            PyValue::List(
                b"Out of memory: Killed process 1 (x)"
                    .iter()
                    .map(|b| PyValue::int(*b as i64))
                    .collect(),
            ),
        ),
        ("__REALTIME_TIMESTAMP", py::s("1785464193000000")),
    ]);
    let nb = normalize(&rec_bytes, "k").unwrap_or(None);
    c("MESSAGE 바이트열 처리", is(&nb, "kind", "kernel_oom_kill"));
    if let Some(ev) = &ev {
        let _ = append_events(&ed, std::slice::from_ref(ev));
        let _ = append_events(&ed, std::slice::from_ref(ev));
    }
    let n = std::fs::read_to_string(bb::join(&ed, "2026-07.jsonl"))
        .map(|t| py::strip(&t).split('\n').count())
        .unwrap_or(0);
    c("append 는 월별 파일에 누적", n == 2);
    let _ = save_cursors(&ed, &py::dict(vec![("unit:test", py::s("s=1;i=2"))]));
    c(
        "커서 저장/복원",
        py::get_str(&load_cursors(&ed), "unit:test").as_deref() == Some("s=1;i=2"),
    );
    c(
        "커서 파일 부재 시 빈 dict",
        matches!(load_cursors(&bb::join(td.path(), "nope")), PyValue::Dict(d) if d.is_empty()),
    );
    drop(td);

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

static ARGS: [Arg; 4] = [
    HELP,
    store(&["--node-dir"]),
    flag(&["--no-boot-scan"]),
    flag(&["--self-test"]),
];

pub static SPEC: Spec = Spec {
    prog: "blackbox_events.py",
    usage: include_str!("../help/blackbox_events.usage"),
    help: include_str!("../help/blackbox_events.help"),
    args: &ARGS,
    subs: None,
};

fn run(a: &argp::Parsed) -> R<i32> {
    if a.flag(3) {
        return Ok(self_test());
    }
    let Some(nd) = a.text(1).filter(|s| !s.is_empty()) else {
        return Err(bb::ap_error(&SPEC, "--node-dir 가 필요합니다"));
    };
    let found = collect(nd, !a.flag(2)).map_err(Fail::Raise)?;
    let mut by_kind: Vec<(String, i64)> = vec![];
    for e in &found {
        let k = e.get("kind").map(bb::s).unwrap_or_default();
        match by_kind.iter_mut().find(|x| x.0 == k) {
            Some(x) => x.1 += 1,
            None => by_kind.push((k, 1)),
        }
    }
    by_kind.sort();
    println!("[events] 신규 {}건 -> {}/events", found.len(), nd);
    for (k, v) in by_kind {
        println!("           {} {}", bb::ljust(&k, 24), v);
    }
    Ok(0)
}

pub fn main(argv: &[String]) -> i32 {
    match argp::parse(&SPEC, argv) {
        Outcome::Exit(c) => c,
        Outcome::Ok(a) => bb::finish(run(&a)),
    }
}
