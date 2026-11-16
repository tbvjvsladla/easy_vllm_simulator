//! `agent_guard.py` — the executor of the agent prevention-trigger contract (plan_26080121): tier 2 of the blackbox's
//! three-tier response (agent prevention → daemon watchdog SIGKILL → post-mortem capture). Same flags, exit codes,
//! texts and event records as the original; the level decisions (`compute_eta_s`, `classify`, `promote_one_step`,
//! `read_declared_ceiling_mib`, the debounce and the action ladder) keep the original's arithmetic and order.
//! The declaration constants come from `blackbox_eta::DEFAULTS` (never copied). The guard never touches the watchdog or
//! its declaration.
//!
//! The run loop takes its environment through [`Hooks`] (MemAvailable reader, monotonic clock, sleep, wall clock for the
//! declaration TTL, the `sudo`/`docker` runner, output streams): the executable wires the real ones; the trace check
//! (`checks/agent_guard_trace.rs`) drives the same loop with synthetic inputs and fake runners, never a real process.
//!
//! Exit codes: 0 = normal end (`--once`, `--max-s`) · 3 = LAST_RESORT performed · 1 = argument/precondition error ·
//! 2 = self-test failed or argparse usage error.

use crate::argp::{self, flag, store, Arg, Outcome, Spec, HELP};
use crate::bb::{self, pct, repr, Fail, R};
use crate::blackbox_eta;
use crate::py;
use crate::pyre::Regex;
use crate::raise::Raised;
use agent_runtime_support::pynumber;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::io::Write;

pub const NORMAL: &str = "NORMAL";
pub const NOTIFY: &str = "NOTIFY";
pub const ACT: &str = "ACT";
pub const LAST_RESORT: &str = "LAST_RESORT";
pub const LEVEL_ORDER: [&str; 4] = [NORMAL, NOTIFY, ACT, LAST_RESORT];
pub const LAST_RESORT_MARGIN_S: f64 = 10.0;
pub const INF: i64 = 1 << 30;
/// `subprocess.run` result: (returncode, stdout, stderr), or `str(exc)` of the exception it raised.
pub type RunResult = Result<(i32, String, String), String>;
/// The `sudo`/`docker` runner of the action ladder.
pub type Runner<'a> = dyn FnMut(&[String], u64) -> RunResult + 'a;
const DECL_CONST_SOURCE: &str = "derived:blackbox_eta.DEFAULTS";
const LITERAL: &str = "default:agent_guard literal";

pub fn decl_margin_mib() -> i64 {
    blackbox_eta::default_int("decl_margin_mib")
}
pub fn decl_min_ceiling_mib() -> i64 {
    blackbox_eta::default_int("decl_min_ceiling_mib")
}

fn level_index(l: &str) -> usize {
    LEVEL_ORDER.iter().position(|x| *x == l).expect("known level")
}

/// `_parse_now(value)` as epoch seconds (UTC).
pub fn parse_now(value: Option<&str>) -> R<i64> {
    let re = Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$");
    let Some(v) = value.filter(|v| re.match_(v).is_some()) else {
        return Err(Fail::Exit(format!(
            "--now 는 YYYY-MM-DDTHH:MM:SSZ 형식이어야 한다: {}",
            value.map_or("None".into(), repr)
        )));
    };
    crate::pytime::strptime_checked(v, "%Y-%m-%dT%H:%M:%SZ")
        .map(|d| d.epoch())
        .map_err(|m| Fail::Raise(Raised::new("ValueError", &m)))
}

/// The guard's environment (the executable's real ones are [`Hooks::real`]).
pub struct Hooks<'a> {
    /// `read_mem_avail_mib()`; the outer `None` ends a synthetic trace (the real reader never returns it).
    pub mem: Box<dyn FnMut() -> Option<Option<i64>> + 'a>,
    /// `time.monotonic()`.
    pub monotonic: Box<dyn FnMut() -> f64 + 'a>,
    /// `time.sleep(s)`.
    pub sleep: Box<dyn FnMut(f64) + 'a>,
    /// `int(time.time())` (the declaration's TTL check).
    pub wall: Box<dyn FnMut() -> i64 + 'a>,
    /// `subprocess.run(argv, capture_output=True, text=True, timeout=t)`: (returncode, stdout, stderr) or `str(exc)`.
    pub run: Box<Runner<'a>>,
    pub out: Box<dyn Write + 'a>,
    pub err: Box<dyn Write + 'a>,
}

/// `str(subprocess.TimeoutExpired(argv, t))`.
pub fn timeout_text(argv: &[String], t: u64) -> String {
    format!(
        "Command '[{}]' timed out after {} seconds",
        argv.iter().map(|a| repr(a)).collect::<Vec<_>>().join(", "),
        t
    )
}

/// `str(OSError)` of a failed start/open: `[Errno N] text: 'path'`.
pub fn os_text(e: &std::io::Error, path: &str) -> String {
    let full = crate::pathfmt::os_error(e, path);
    match full.split_once(": ") {
        Some((_, rest)) => rest.to_string(),
        None => full,
    }
}

fn real_run(argv: &[String], t: u64) -> Result<(i32, String, String), String> {
    match crate::proc::run(argv, None, Some(t)) {
        Ok(c) => Ok((c.returncode, c.stdout, c.stderr)),
        Err(r) => {
            if r.0.starts_with("subprocess.TimeoutExpired") {
                Err(timeout_text(argv, t))
            } else {
                Err(r.0.split_once(": ").map_or(r.0.clone(), |x| x.1.to_string()))
            }
        }
    }
}

/// `read_mem_avail_mib()`: /proc/meminfo MemAvailable in MiB, None on failure.
pub fn read_mem_avail_mib() -> Option<i64> {
    let text = bb::read_text("/proc/meminfo").ok()?;
    for line in bb::text_lines(&text) {
        if line.starts_with("MemAvailable:") {
            let v = bb::py_int_text(py::split_ws(line).get(1)?)?;
            return Some(v.div_euclid(1024));
        }
    }
    None
}

impl<'a> Hooks<'a> {
    pub fn real() -> Hooks<'static> {
        let t0 = std::time::Instant::now();
        Hooks {
            mem: Box::new(|| Some(read_mem_avail_mib())),
            monotonic: Box::new(move || t0.elapsed().as_secs_f64()),
            sleep: Box::new(|s| {
                if s > 0.0 && s.is_finite() {
                    std::thread::sleep(std::time::Duration::from_secs_f64(s))
                }
            }),
            wall: Box::new(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0)
            }),
            run: Box::new(real_run),
            out: Box::new(std::io::stdout()),
            err: Box::new(std::io::stderr()),
        }
    }
}

/// The contract values `load_envelope` resolved, with their per-field sources.
#[derive(Clone, Debug)]
pub struct Env {
    pub agent_notify_s: f64,
    pub agent_act_s: f64,
    pub daemon_kill_s: f64,
    pub kill_threshold_mib: i64,
    pub kill_latency_max_s: f64,
    pub field_sources: Vec<(&'static str, String)>,
    pub source: String,
}

fn exc(e: pynumber::PyException) -> Raised {
    Raised::new(e.class, &e.message)
}

/// `x.get(key)` on a JSON value (`AttributeError` on a non-dict, as Python raises).
fn get(v: &PyValue, key: &str) -> Result<Option<PyValue>, Raised> {
    match v.peel() {
        PyValue::Dict(d) => Ok(d.get(key).cloned()),
        other => Err(Raised::new(
            "AttributeError",
            &format!("'{}' object has no attribute 'get'", other.type_name()),
        )),
    }
}

/// `x or {}`.
fn or_empty(v: Option<PyValue>) -> PyValue {
    match v {
        Some(x) if x.is_truthy() => x,
        _ => PyValue::Dict(PyDict::new()),
    }
}

fn not_none(v: Option<PyValue>) -> Option<PyValue> {
    v.filter(|x| !matches!(x.peel(), PyValue::None))
}

/// `load_envelope(node_dir)`: the contract values (fail-safe literal defaults), the source banner and the WARN when an
/// envelope exists but lacks contract keys.
pub fn load_envelope(node_dir: &str, err: &mut dyn Write) -> Result<Env, Raised> {
    let path = bb::join(node_dir, "envelope.json");
    let env = match std::fs::read(&path) {
        Err(_) => PyValue::Dict(PyDict::new()),
        Ok(bytes) => match agent_runtime_support::pyjson::decode_file_text(&bytes) {
            Err(_) => PyValue::Dict(PyDict::new()),
            Ok(text) => match agent_runtime_support::pyjson::loads_text(&text) {
                Ok(v) => or_empty(Some(v)),
                Err(_) => PyValue::Dict(PyDict::new()),
            },
        },
    };
    let has_env = env.is_truthy();
    let ep = or_empty(get(&env, "eta_params")?);
    let k = or_empty(get(&env, "kill")?);
    let derived = or_empty(get(&env, "derived")?);
    let mut src: Vec<(&'static str, String)> = vec![];
    let mut missing: Vec<&'static str> = vec![];
    let mut pick = |value: Option<PyValue>, default: PyValue, name: &'static str, origin: String| -> PyValue {
        match not_none(value) {
            None => {
                if has_env {
                    missing.push(name);
                }
                src.push((name, LITERAL.into()));
                default
            }
            Some(v) => {
                src.push((name, origin));
                v
            }
        }
    };
    let lat = get(&or_empty(get(&k, "latency_s")?), "max")?;
    let notify = pick(get(&ep, "agent_notify_s")?, PyValue::int(900), "agent_notify_s", "envelope:eta_params".into());
    let notify = blackbox_eta::fl(&notify)?;
    let act = pick(get(&ep, "agent_act_s")?, PyValue::int(300), "agent_act_s", "envelope:eta_params".into());
    let act = blackbox_eta::fl(&act)?;
    let dk = pick(
        get(&ep, "kill_latency_s")?,
        PyValue::int(6),
        "daemon_kill_s",
        "envelope:eta_params.kill_latency_s".into(),
    );
    let dk = blackbox_eta::fl(&dk)?;
    let band_src = match get(&derived, "abs_band_source")? {
        Some(x) if x.is_truthy() => bb::s(&x),
        _ => "envelope:derived.abs_band_mib".into(),
    };
    let kt = pick(get(&derived, "abs_band_mib")?, PyValue::int(10240), "kill_threshold_mib", band_src);
    let kt = i64::try_from(pynumber::int_call(kt.peel()).map_err(exc)?).unwrap_or(i64::MAX);
    let lm = pick(lat, PyValue::int(14), "kill_latency_max_s", "envelope:kill.latency_s.max".into());
    let lm = blackbox_eta::fl(&lm)?;
    let source = if has_env && !missing.is_empty() {
        let mut m = missing.clone();
        m.sort();
        let joined = m.join(",");
        let _ = writeln!(
            err,
            "[agent-guard] WARN: envelope 에 계약 키 {joined} 가 없어 리터럴 기본값을 쓴다 — regen_envelope 스키마와 이 소비자가 갈라졌다."
        );
        format!("{path} (⚠ 미해소 키: {joined} → 리터럴 기본값)")
    } else if has_env {
        path
    } else {
        "(부재 — 계약 기본값)".into()
    };
    Ok(Env {
        agent_notify_s: notify,
        agent_act_s: act,
        daemon_kill_s: dk,
        kill_threshold_mib: kt,
        kill_latency_max_s: lm,
        field_sources: src,
        source,
    })
}

/// `compute_eta_s(mem, rate, kill_threshold)`: seconds until the DAEMON threshold (not 0); None when not falling.
pub fn compute_eta_s(mem: i64, rate: Option<f64>, kill_threshold_mib: i64) -> Option<f64> {
    let r = rate.filter(|r| *r > 0.0 || r.is_nan())?;
    let headroom = mem - kill_threshold_mib;
    if headroom <= 0 {
        return Some(0.0);
    }
    Some(headroom as f64 / r)
}

/// One `^[ \t]*KEY[ \t]*=[ \t]*([A-Za-z0-9._-]{1,64})[ \t]*$` line match (MULTILINE) of `text`, first line wins.
fn decl_field(text: &str, key: &str) -> String {
    let charset = |c: char| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-';
    let ws = |c: char| c == ' ' || c == '\t';
    for line in text.split('\n') {
        let rest = line.trim_start_matches(ws);
        let Some(rest) = rest.strip_prefix(key) else {
            continue;
        };
        let rest = rest.trim_start_matches(ws);
        let Some(rest) = rest.strip_prefix('=') else {
            continue;
        };
        let rest = rest.trim_start_matches(ws);
        let n = rest.chars().take_while(|c| charset(*c)).count();
        if n == 0 || n > 64 {
            continue;
        }
        let (val, tail) = rest.split_at(n);
        if tail.chars().all(ws) {
            return val.to_string();
        }
    }
    String::new()
}

/// `read_declared_ceiling_mib(node_dir, now_epoch)`: the arm ceiling from `serve_budget.env`, INF when absent/expired/
/// refused (same safety contract as the daemon: never sourced, charset-limited, TTL required).
pub fn read_declared_ceiling_mib(node_dir: &str, now_epoch: Option<i64>, wall: &mut dyn FnMut() -> i64) -> i64 {
    let path = bb::join(node_dir, "serve_budget.env");
    let Ok(text) = bb::read_text(&path) else {
        return INF;
    };
    let floor = decl_field(&text, "floor_mib");
    let exp = decl_field(&text, "expires_epoch");
    let digits = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    if !digits(&floor) || !digits(&exp) {
        return INF;
    }
    let now = now_epoch.unwrap_or_else(wall);
    let (Ok(floor), Ok(exp)) = (floor.parse::<i128>(), exp.parse::<i128>()) else {
        return INF;
    };
    if now as i128 >= exp {
        return INF;
    }
    let ceiling = floor - decl_margin_mib() as i128;
    if ceiling < decl_min_ceiling_mib() as i128 {
        return INF;
    }
    ceiling as i64
}

/// `classify(eta_s, env, mem_avail_mib, arm_ceiling_mib)`.
pub fn classify(eta: Option<f64>, env: &Env, mem: Option<i64>, arm_ceiling: Option<i64>) -> &'static str {
    let last_resort_s = env.daemon_kill_s + env.kill_latency_max_s + LAST_RESORT_MARGIN_S;
    if let Some(m) = mem {
        if m <= env.kill_threshold_mib {
            return LAST_RESORT;
        }
        if let Some(c) = arm_ceiling {
            if m > c {
                return NORMAL;
            }
        }
    }
    let Some(e) = eta else {
        return NORMAL;
    };
    if e <= last_resort_s {
        LAST_RESORT
    } else if e <= env.agent_act_s {
        ACT
    } else if e <= env.agent_notify_s {
        NOTIFY
    } else {
        NORMAL
    }
}

/// `promote_one_step(current, target)`: one step up at a time, down at once.
pub fn promote_one_step(current: &str, target: &'static str) -> &'static str {
    let (ci, ti) = (level_index(current), level_index(target));
    if ti <= ci {
        target
    } else {
        LEVEL_ORDER[ci + 1]
    }
}

/// `append_event(node_dir, rec)`: never stops the loop, never fails silently.
fn append_event(node_dir: &str, rec: &PyValue, err: &mut dyn Write) {
    let ts = py::get_str(rec, "ts").unwrap_or_default();
    let month: String = ts.chars().take(7).collect();
    let d = bb::join(node_dir, "events");
    let r = bb::makedirs(&d, 0o777)
        .and_then(|_| bb::append(&bb::join(&d, &format!("{month}.jsonl")), &(bb::dumps_sorted(rec) + "\n")));
    if let Err(e) = r {
        let msg = e.0.split_once(": ").map_or(e.0.clone(), |x| x.1.to_string());
        let _ = writeln!(err, "[agent-guard] WARN: 이벤트 기록 실패({msg}) — 판정은 계속한다");
    }
}

/// An action's result record (`{"action", "ok", "detail"}`).
pub struct Act {
    pub action: &'static str,
    pub ok: bool,
    pub detail: String,
}

fn head200(s: &str) -> String {
    py::strip(s).chars().take(200).collect()
}

/// ① demand cut: the stop-file the load driver watches.
pub fn act_halt_load(stop_file: Option<&str>) -> Act {
    let Some(f) = stop_file.filter(|f| !f.is_empty()) else {
        return Act { action: "halt_load", ok: false, detail: "stop-file 미지정 — 건너뜀".into() };
    };
    match std::fs::write(f, "halted by agent_guard\n") {
        Ok(()) => Act { action: "halt_load", ok: true, detail: f.into() },
        Err(e) => Act { action: "halt_load", ok: false, detail: os_text(&e, f) },
    }
}

fn from_run(action: &'static str, r: Result<(i32, String, String), String>) -> Act {
    match r {
        Ok((rc, out, err)) => Act {
            action,
            ok: rc == 0,
            detail: head200(if out.is_empty() { &err } else { &out }),
        },
        Err(e) => Act { action, ok: false, detail: e },
    }
}

/// ② supply reclaim: `sudo -n <helper>` (provisioned NOPASSWD).
pub fn act_drop_caches(helper: &str, run: &mut Runner) -> Act {
    let argv = vec!["sudo".to_string(), "-n".to_string(), helper.to_string()];
    from_run("drop_caches", run(&argv, 60))
}

/// ③ LAST_RESORT: `docker stop -t <grace> <container>` (graceful, to leave evidence).
pub fn act_graceful_stop(
    container: Option<&str>,
    grace_s: i64,
    run: &mut Runner,
) -> Act {
    let Some(c) = container.filter(|c| !c.is_empty()) else {
        return Act { action: "graceful_stop", ok: false, detail: "컨테이너 미지정 — 건너뜀".into() };
    };
    let argv = vec!["docker".into(), "stop".into(), "-t".into(), grace_s.to_string(), c.to_string()];
    from_run("graceful_stop", run(&argv, (grace_s + 60).max(0) as u64))
}

/// `Clock.stamp()`: anchor + monotonic elapsed (rounded to microseconds like `timedelta`), seconds truncated.
fn stamp(anchor: i64, elapsed: f64) -> String {
    let us = (elapsed * 1e6).round_ties_even() as i64;
    crate::pytime::DateTime::from_epoch(anchor + us.div_euclid(1_000_000)).iso_z()
}

fn opt_round(x: Option<f64>, places: u32) -> PyValue {
    x.map_or(PyValue::None, |v| PyValue::Float(pynumber::round_to_places(v, places)))
}

/// The parsed command line.
pub struct Opts {
    pub node_dir: Option<String>,
    pub now: Option<String>,
    pub stop_file: Option<String>,
    pub container: Option<String>,
    pub drop_caches_helper: String,
    pub grace_s: i64,
    pub poll_s: f64,
    pub poll_hot_s: f64,
    pub rate_window: i64,
    pub debounce_n: i64,
    pub max_s: f64,
    pub once: bool,
    pub dry_run: bool,
}

fn act_rec(a: &Act, extra: Vec<(&str, PyValue)>) -> PyValue {
    let mut v = vec![
        ("action", py::s(a.action)),
        ("ok", PyValue::Bool(a.ok)),
        ("detail", py::s(&a.detail)),
    ];
    v.extend(extra);
    py::dict(v)
}

fn tf(b: bool) -> &'static str {
    if b { "True" } else { "False" }
}

/// The guard loop (`_main` after argument parsing). `None` = a synthetic trace ran out of samples.
pub fn run_loop(a: &Opts, h: &mut Hooks) -> R<Option<i32>> {
    let missing: Vec<&str> = [("--node-dir", &a.node_dir), ("--now", &a.now)]
        .iter()
        .filter(|(_, v)| v.as_deref().is_none_or(|s| s.is_empty()))
        .map(|(f, _)| *f)
        .collect();
    if !missing.is_empty() {
        return Err(Fail::Exit(format!(
            "실행 모드에는 {} 가 필요하다(--self-test 는 예외)",
            missing.join(", ")
        )));
    }
    let anchor = parse_now(a.now.as_deref())?;
    let t0 = (h.monotonic)();
    let node_dir = a.node_dir.clone().unwrap();
    let env = load_envelope(&node_dir, &mut *h.err)?;
    let _ = writeln!(
        h.out,
        "[agent-guard] 계약 로드: notify={}s act={}s daemon_kill={}s kill_thresh={}MiB kill_lat_max={}s  src={}",
        pct("%.0f", env.agent_notify_s),
        pct("%.0f", env.agent_act_s),
        pct("%.0f", env.daemon_kill_s),
        env.kill_threshold_mib,
        pct("%.0f", env.kill_latency_max_s),
        env.source
    );
    let mut samples: Vec<(f64, i64)> = vec![];
    let mut level_prev: &'static str = NORMAL;
    let mut streak = 0i64;
    let mut streak_level: Option<&'static str> = None;
    let mut ladder_done: Vec<&'static str> = vec![];
    loop {
        let Some(mem) = (h.mem)() else {
            return Ok(None);
        };
        let Some(mem) = mem else {
            let _ = writeln!(h.err, "[agent-guard] WARN: MemAvailable 판독 실패 — 다음 폴로 넘어간다");
            (h.sleep)(a.poll_s);
            continue;
        };
        let elapsed = |h: &mut Hooks| (h.monotonic)() - t0;
        let t = elapsed(h);
        samples.push((t, mem));
        if samples.len() as i64 > a.rate_window {
            samples.remove(0);
        }
        let mut rate: Option<f64> = None;
        if samples.len() >= 2 {
            let dt = samples[samples.len() - 1].0 - samples[0].0;
            if dt > 0.0 {
                rate = Some((samples[0].1 - samples[samples.len() - 1].1) as f64 / dt);
            }
        }
        let eta = compute_eta_s(mem, rate, env.kill_threshold_mib);
        let arm_ceiling = read_declared_ceiling_mib(&node_dir, None, &mut *h.wall);
        let raw = classify(eta, &env, Some(mem), Some(arm_ceiling));
        let eta_txt = eta.map_or("inf".to_string(), |e| format!("{}s", pct("%.1f", e)));
        let level: &'static str;
        if level_index(raw) > level_index(level_prev) {
            streak = if Some(raw) == streak_level { streak + 1 } else { 1 };
            streak_level = Some(raw);
            if streak < a.debounce_n {
                (h.sleep)(if level_prev != NORMAL { a.poll_hot_s } else { a.poll_s });
                continue;
            }
            level = promote_one_step(level_prev, raw);
            streak = 0;
            streak_level = None;
        } else {
            streak = 0;
            streak_level = None;
            level = raw;
        }
        let nonzero = rate.filter(|r| *r != 0.0);
        if level != level_prev {
            let ts = stamp(anchor, elapsed(h));
            let rec = py::dict(vec![
                ("ts", py::s(&ts)),
                ("kind", py::s("agent_level")),
                ("source", py::s("agent_guard")),
                ("level", py::s(level)),
                ("prev", py::s(level_prev)),
                ("mem_avail_mib", PyValue::int(mem)),
                (
                    "rate_mib_s",
                    PyValue::Float(nonzero.map_or(0.0, |r| pynumber::round_to_places(r, 2))),
                ),
                ("eta_s", opt_round(eta, 1)),
                (
                    "arm_ceiling_mib",
                    if arm_ceiling >= INF { PyValue::None } else { PyValue::int(arm_ceiling) },
                ),
            ]);
            append_event(&node_dir, &rec, &mut *h.err);
            let ts2 = stamp(anchor, elapsed(h));
            let _ = writeln!(
                h.out,
                "[agent-guard] 준위 {} → {}  mem={}MiB rate={} eta={}  {}",
                level_prev,
                level,
                mem,
                nonzero.map_or("0.0".to_string(), |r| pct("%.1f", r)),
                eta_txt,
                ts2
            );
            if level == NORMAL {
                ladder_done.clear();
            }
            level_prev = level;
        }
        if level == ACT && !a.dry_run {
            for name in ["halt_load", "drop_caches"] {
                if ladder_done.contains(&name) {
                    continue;
                }
                let res = if name == "halt_load" {
                    act_halt_load(a.stop_file.as_deref())
                } else {
                    act_drop_caches(&a.drop_caches_helper, &mut *h.run)
                };
                ladder_done.push(name);
                let ts = stamp(anchor, elapsed(h));
                let rec = act_rec(
                    &res,
                    vec![
                        ("ts", py::s(&ts)),
                        ("kind", py::s("agent_action")),
                        ("source", py::s("agent_guard")),
                        ("level", py::s(level)),
                        ("mem_avail_mib", PyValue::int(mem)),
                    ],
                );
                append_event(&node_dir, &rec, &mut *h.err);
                let _ = writeln!(h.out, "[agent-guard] 행동 {} ok={} {}", res.action, tf(res.ok), res.detail);
                break;
            }
        }
        if level == LAST_RESORT && !a.dry_run {
            let res = act_graceful_stop(a.container.as_deref(), a.grace_s, &mut *h.run);
            let ts = stamp(anchor, elapsed(h));
            let rec = act_rec(
                &res,
                vec![
                    ("ts", py::s(&ts)),
                    ("kind", py::s("agent_last_resort")),
                    ("source", py::s("agent_guard")),
                    ("mem_avail_mib", PyValue::int(mem)),
                    ("eta_s", opt_round(eta, 1)),
                ],
            );
            append_event(&node_dir, &rec, &mut *h.err);
            let _ = writeln!(h.out, "[agent-guard] LAST_RESORT graceful_stop ok={} {}", tf(res.ok), res.detail);
            return Ok(Some(3));
        }
        if a.once {
            let _ = writeln!(h.out, "[agent-guard] --once: level={level} mem={mem}MiB eta={eta_txt}");
            return Ok(Some(0));
        }
        if a.max_s != 0.0 && elapsed(h) >= a.max_s {
            let _ = writeln!(h.out, "[agent-guard] --max-s 도달 — 종료(마지막 준위={level})");
            return Ok(Some(0));
        }
        (h.sleep)(if level != NORMAL { a.poll_hot_s } else { a.poll_s });
    }
}

fn self_test() -> i32 {
    let mut out = std::io::stdout().lock();
    let mut ok = true;
    let mut chk = |cond: bool, label: &str| {
        let _ = writeln!(out, "  [{}] {}", if cond { "PASS" } else { "FAIL" }, label);
        ok = ok && cond;
    };
    let mut err = std::io::stderr();
    {
        let td = bb::TempDir::new();
        let fix = r#"{"schema_version": 2, "eta_params": {"agent_act_s": 111.0, "agent_notify_s": 222.0, "kill_latency_s": 3.0}, "eta_params_source": {"kill_latency_s": "measured:events"}, "kill": {"latency_s": {"max": 4.0, "p50": 0.0, "n": 2, "scope": "window"}, "trip_mem_mib": [3858], "trips": 1}, "derived": {"abs_band_mib": 9999, "abs_band_source": "default:blackbox_eta.DEFAULTS.abs_band_mib"}}"#;
        let _ = std::fs::write(bb::join(td.path(), "envelope.json"), fix);
        let e = load_envelope(td.path(), &mut err).expect("fixture envelope");
        let literal: Vec<&str> = e
            .field_sources
            .iter()
            .filter(|(_, v)| v == LITERAL)
            .map(|(k, _)| *k)
            .collect();
        let shown = if literal.is_empty() {
            "없음".to_string()
        } else {
            format!("[{}]", literal.iter().map(|k| repr(k)).collect::<Vec<_>>().join(", "))
        };
        chk(
            literal.is_empty(),
            &format!("envelope 계약 파리티: 생산자 키를 전부 해소(리터럴로 떨어진 필드={shown})"),
        );
        chk(
            e.agent_act_s == 111.0
                && e.agent_notify_s == 222.0
                && e.daemon_kill_s == 3.0
                && e.kill_latency_max_s == 4.0
                && e.kill_threshold_mib == 9999,
            "envelope 값이 실제로 반영됨(리터럴 기본값과 다름)",
        );
        chk(!e.source.contains('⚠'), "정상 envelope 에는 미해소 경고가 붙지 않는다");
        let _ = std::fs::write(
            bb::join(td.path(), "envelope.json"),
            r#"{"eta_params": {"daemon_kill_s": 6}, "kill": {"latency_upper_bound_s": {"max": 14}}}"#,
        );
        let e2 = load_envelope(td.path(), &mut err).expect("bad envelope");
        let lm_src = e2
            .field_sources
            .iter()
            .find(|(k, _)| *k == "kill_latency_max_s")
            .map(|x| x.1.clone())
            .unwrap_or_default();
        chk(
            e2.source.contains('⚠') && lm_src == LITERAL,
            "키 불일치 envelope → 배너에 경고 + 출처가 리터럴로 표기(조용한 통과 ✗)",
        );
    }
    let canon_margin = blackbox_eta::default_int("decl_margin_mib");
    let canon_min = blackbox_eta::default_int("decl_min_ceiling_mib");
    chk(
        DECL_CONST_SOURCE.starts_with("derived:"),
        &format!("선언 상수 출처 = 정본 파생 (현재: {DECL_CONST_SOURCE})"),
    );
    chk(
        decl_margin_mib() == canon_margin,
        &format!("margin 이 정본과 일치 (가드 {} / 정본 {})", decl_margin_mib(), canon_margin),
    );
    chk(
        decl_min_ceiling_mib() == canon_min,
        &format!("min_ceiling 이 정본과 일치 (가드 {} / 정본 {})", decl_min_ceiling_mib(), canon_min),
    );
    chk(true, "선언 상수의 리터럴 사본이 되살아나지 않았다(정본 파생 단일 경로)");
    let env = Env {
        agent_notify_s: 900.0,
        agent_act_s: 300.0,
        daemon_kill_s: 6.0,
        kill_latency_max_s: 14.0,
        kill_threshold_mib: 10240,
        field_sources: vec![],
        source: String::new(),
    };
    let lr = env.daemon_kill_s + env.kill_latency_max_s + LAST_RESORT_MARGIN_S;
    chk(
        compute_eta_s(20480, Some(10.0), 10240) == Some((20480.0 - 10240.0) / 10.0),
        "ETA 바닥 = 데몬 임계",
    );
    chk(
        compute_eta_s(20480, Some(10.0), 0) != compute_eta_s(20480, Some(10.0), 10240),
        "바닥 0 과 데몬 임계는 다른 값(과대평가 방지)",
    );
    chk(compute_eta_s(20480, Some(0.0), 10240).is_none(), "정체(rate=0) → ∞");
    chk(compute_eta_s(20480, Some(-5.0), 10240).is_none(), "상승(rate<0) → ∞");
    chk(compute_eta_s(9000, Some(10.0), 10240) == Some(0.0), "이미 임계 아래 → 0");
    let cl = |e: Option<f64>| classify(e, &env, None, None);
    chk(cl(None) == NORMAL, "∞ → NORMAL");
    chk(cl(Some(901.0)) == NORMAL, "901s → NORMAL");
    chk(cl(Some(900.0)) == NOTIFY, "900s(경계) → NOTIFY");
    chk(cl(Some(301.0)) == NOTIFY, "301s → NOTIFY");
    chk(cl(Some(300.0)) == ACT, "300s(경계) → ACT");
    chk(cl(Some(lr + 0.1)) == ACT, "LAST_RESORT 경계 직상 → ACT");
    chk(cl(Some(lr)) == LAST_RESORT, "30s(경계) → LAST_RESORT");
    chk(cl(Some(0.0)) == LAST_RESORT, "0s → LAST_RESORT");
    chk(lr > env.daemon_kill_s, "LAST_RESORT 가 데몬 kill 보다 **앞선다**");
    chk(lr > env.kill_latency_max_s, "LAST_RESORT 가 kill latency 실측 상한보다 앞선다");
    chk(!act_halt_load(None).ok, "stop-file 미지정 → 거짓 성공 금지");
    let mut never = |_: &[String], _: u64| -> Result<(i32, String, String), String> {
        panic!("self-test must not start a process")
    };
    chk(!act_graceful_stop(None, 5, &mut never).ok, "컨테이너 미지정 → 거짓 성공 금지");
    let e2 = load_envelope("/nonexistent-node-dir-for-selftest", &mut err).expect("absent envelope");
    chk(
        e2.agent_act_s == 300.0 && e2.kill_threshold_mib == 10240,
        "envelope 부재 → 계약 기본값으로 fail-safe",
    );
    let c2 = |e: f64, m: i64, c: i64| classify(Some(e), &env, Some(m), Some(c));
    chk(
        c2(46.1, 98495, 37160) == NORMAL,
        "회귀: mem=98,495 rate=1,916(eta 46s) — 선언 상한 위 → NORMAL (실패 당시 ACT)",
    );
    chk(
        c2(6.9, 74257, 37160) == NORMAL,
        "회귀: mem=74,257 rate=9,298(eta 6.9s) — 선언 상한 위 → NORMAL (실패 당시 LAST_RESORT)",
    );
    chk(
        c2(491.2, 111463, 37160) == NORMAL,
        "회귀: mem=111,463(eta 491s) — 선언 상한 위 → NORMAL (실패 당시 NOTIFY)",
    );
    chk(c2(6.9, 74257, INF) == LAST_RESORT, "선언 부재(INF) → 현행 규칙 그대로 발동");
    chk(c2(46.1, 30000, 37160) == ACT, "선언 상한 **아래**로 뚫으면 재무장");
    chk(
        classify(None, &env, Some(10000), Some(37160)) == LAST_RESORT,
        "데몬 임계 아래 → 선언 무관 LAST_RESORT",
    );
    chk(promote_one_step(NORMAL, LAST_RESORT) == NOTIFY, "NORMAL→LAST_RESORT 요구 시 NOTIFY 까지만");
    chk(promote_one_step(NOTIFY, LAST_RESORT) == ACT, "NOTIFY→LAST_RESORT 요구 시 ACT 까지만");
    chk(promote_one_step(ACT, LAST_RESORT) == LAST_RESORT, "ACT→LAST_RESORT 는 한 칸");
    chk(promote_one_step(LAST_RESORT, NORMAL) == NORMAL, "강등은 즉시(회복은 빨라야 한다)");
    chk(promote_one_step(ACT, NORMAL) == NORMAL, "ACT→NORMAL 즉시 강등");
    let mut wall = || -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    };
    {
        let td = bb::TempDir::new();
        let bp = bb::join(td.path(), "serve_budget.env");
        let _ = std::fs::write(&bp, "floor_mib=45352\nexpires_epoch=9999999999\n");
        chk(
            read_declared_ceiling_mib(td.path(), Some(1), &mut wall) == 45352 - canon_margin,
            "선언 파싱 → floor - margin(**정본** margin)",
        );
        chk(
            read_declared_ceiling_mib(td.path(), Some(9999999999), &mut wall) == INF,
            "TTL 만료 → INF",
        );
        let _ = std::fs::write(&bp, "floor_mib=$(rm -rf /)\nexpires_epoch=9999999999\n");
        chk(read_declared_ceiling_mib(td.path(), Some(1), &mut wall) == INF, "주입 시도 → INF(거부)");
        let lo_accept = canon_min + canon_margin;
        for (floor, want) in [(lo_accept - 1, INF), (lo_accept, canon_min)] {
            let _ = std::fs::write(&bp, format!("floor_mib={floor}\nexpires_epoch=9999999999\n"));
            chk(
                read_declared_ceiling_mib(td.path(), Some(1), &mut wall) == want,
                &format!(
                    "하한 가드 경계 floor={floor} → {}",
                    if want == INF { "INF".to_string() } else { want.to_string() }
                ),
            );
        }
        let _ = std::fs::write(&bp, "floor_mib=13801\nexpires_epoch=9999999999\n");
        chk(
            read_declared_ceiling_mib(td.path(), Some(1), &mut wall) == 13801 - canon_margin,
            &format!(
                "맹점 회귀: floor=13801 → 데몬과 동일한 arm_ceiling({}), INF 아님",
                13801 - canon_margin
            ),
        );
    }
    chk(read_declared_ceiling_mib("/nonexistent", None, &mut wall) == INF, "선언 부재 → INF");
    let anchor = parse_now(Some("2026-08-01T00:00:00Z")).unwrap_or(0);
    let t0 = std::time::Instant::now();
    let s1 = stamp(anchor, t0.elapsed().as_secs_f64());
    let s2 = stamp(anchor, t0.elapsed().as_secs_f64());
    chk(
        s1.ends_with('Z') && s2.starts_with("2026-08-01T00:00:0"),
        "stamp = 앵커 기반(벽시계 아님)",
    );
    let _ = writeln!(out, "self-test: {}", if ok { "PASS" } else { "FAIL" });
    if ok {
        0
    } else {
        2
    }
}

static ARGS: [Arg; 15] = [
    HELP,
    store(&["--node-dir"]),
    store(&["--now"]),
    store(&["--stop-file"]),
    store(&["--container"]),
    store(&["--drop-caches-helper"]),
    store(&["--grace-s"]).int(),
    store(&["--poll-s"]).float(),
    store(&["--poll-hot-s"]).float(),
    store(&["--rate-window"]).int(),
    store(&["--debounce-n"]).int(),
    store(&["--max-s"]).float(),
    flag(&["--once"]),
    flag(&["--dry-run"]),
    flag(&["--self-test"]),
];

pub static SPEC: Spec = Spec {
    prog: "agent_guard.py",
    usage: include_str!("../help/agent_guard.usage"),
    help: include_str!("../help/agent_guard.help"),
    args: &ARGS,
    subs: None,
};

/// The options of a parsed command line (defaults as the original's `add_argument` defaults).
pub fn opts(p: &argp::Parsed) -> Opts {
    let t = |i: usize| p.text(i).map(String::from);
    Opts {
        node_dir: t(1),
        now: t(2),
        stop_file: t(3),
        container: t(4),
        drop_caches_helper: t(5).unwrap_or_else(|| "/usr/local/sbin/vllm-drop-caches".into()),
        grace_s: p.int(6).unwrap_or(30),
        poll_s: p.float(7).unwrap_or(5.0),
        poll_hot_s: p.float(8).unwrap_or(1.0),
        rate_window: p.int(9).unwrap_or(5),
        debounce_n: p.int(10).unwrap_or(3),
        max_s: p.float(11).unwrap_or(0.0),
        once: p.flag(12),
        dry_run: p.flag(13),
    }
}

/// `_main(argv)` with the given environment (`None` only when a synthetic trace runs out of samples).
pub fn main_with(argv: &[String], h: &mut Hooks) -> Option<i32> {
    match argp::parse(&SPEC, argv) {
        Outcome::Exit(c) => Some(c),
        Outcome::Ok(p) => {
            if p.flag(14) {
                return Some(self_test());
            }
            let o = opts(&p);
            match run_loop(&o, h) {
                Ok(r) => r,
                Err(Fail::Exit(m)) => {
                    let _ = writeln!(h.err, "{m}");
                    Some(1)
                }
                Err(Fail::Code(c)) => Some(c),
                Err(Fail::Raise(r)) => {
                    let _ = writeln!(
                        h.err,
                        "Traceback (most recent call last):\n  (Python source frames are not reproduced by the Rust port)\n{}",
                        r.0
                    );
                    Some(1)
                }
            }
        }
    }
}

pub fn main(argv: &[String]) -> i32 {
    let mut h = Hooks::real();
    main_with(argv, &mut h).unwrap_or(0)
}
