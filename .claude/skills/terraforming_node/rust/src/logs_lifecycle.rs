//! `node_blackbox/logs_lifecycle.py` — lifetime enforcement of `docs/logs` (plan_26073109 §2.4).
//!
//! Order is the safety: 1) rollup (daily envelope statistics first) 2) compress raw days older than `raw_days`
//! (zstd when present, else gzip; the method is recorded) 3) evict compressed copies older than `compressed_days`
//! 4) capacity — oldest samples first while the node total exceeds the cap. `events/` and `rollup/` are permanent.
//! Every deletion/compression is written to events. Time comes from `--now` only.
//! Exit: 0 success · 1 argument/input error · 2 self-test failure (argparse usage errors are 2).

use crate::argp::{self, flag, store, Outcome, Spec, HELP};
use crate::py;
use crate::pytime::{strptime, DateTime};
use agent_runtime_support::pynumber::{float_from_str, round_to_places};
use agent_runtime_support::pystr::{decimal_digit_value, PyStr};
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: i64 = 1;
pub const DEFAULT_RAW_DAYS: i64 = 7;
pub const DEFAULT_COMPRESSED_DAYS: i64 = 30;
pub const DEFAULT_CAP_MIB: i64 = 512;

pub fn parse_now(text: &str) -> Result<DateTime, String> {
    strptime(text, "%Y-%m-%dT%H:%M:%SZ").ok_or_else(|| {
        format!(
            "logs_lifecycle: --now must be 'YYYY-MM-DDTHH:MM:SSZ', got {}",
            PyStr::new(text).repr()
        )
    })
}

/// `_DAY_RE.match(name)`: `^(\d{4}-\d{2}-\d{2})\.csv(\.(gz|zst))?$` -> (day, compressed).
fn day_of(name: &str) -> Option<(String, bool)> {
    let c: Vec<char> = name.chars().collect();
    let d = |i: usize| {
        c.get(i)
            .is_some_and(|&x| decimal_digit_value(x as u32).is_some())
    };
    let ok = (0..4).all(d)
        && c.get(4) == Some(&'-')
        && d(5)
        && d(6)
        && c.get(7) == Some(&'-')
        && d(8)
        && d(9);
    if !ok {
        return None;
    }
    let day: String = c[..10].iter().collect();
    let rest: String = c[10..].iter().collect();
    let rest = rest.strip_suffix('\n').unwrap_or(&rest).to_string();
    match rest.as_str() {
        ".csv" => Some((day, false)),
        ".csv.gz" | ".csv.zst" => Some((day, true)),
        _ => None,
    }
}

pub fn list_sample_files(samples_dir: &Path) -> Vec<(String, PathBuf, bool)> {
    let mut out = vec![];
    let Ok(rd) = std::fs::read_dir(samples_dir) else {
        return out;
    };
    if !samples_dir.is_dir() {
        return out;
    }
    let mut names: Vec<String> = rd
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    for n in names {
        if let Some((day, comp)) = day_of(&n) {
            out.push((day, samples_dir.join(&n), comp));
        }
    }
    out
}

/// `os.walk` + `os.path.getsize` total (symlinked directories are not entered; unreadable entries are skipped).
pub fn dir_size_bytes(path: &Path) -> u64 {
    let mut total = 0;
    let Ok(rd) = std::fs::read_dir(path) else {
        return 0;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if !e.file_type().is_ok_and(|t| t.is_symlink()) {
                total += dir_size_bytes(&p);
            }
        } else if let Ok(m) = std::fs::metadata(&p) {
            total += m.len();
        }
    }
    total
}

fn num(t: &str) -> Option<f64> {
    float_from_str(&PyStr::new(t)).ok()
}

fn sorted(v: &[f64]) -> Vec<f64> {
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    s
}

fn pct(vals: &[f64], q: f64) -> PyValue {
    if vals.is_empty() {
        return PyValue::None;
    }
    let s = sorted(vals);
    let r = (q * (s.len() as f64 - 1.0)).round_ties_even() as i64;
    let i = (s.len() as i64 - 1).min(r.max(0)) as usize;
    PyValue::Float(round_to_places(s[i], 3))
}
fn fmin(v: &[f64]) -> PyValue {
    v.iter()
        .copied()
        .reduce(|a, b| if b < a { b } else { a })
        .map_or(PyValue::None, PyValue::Float)
}
fn fmax(v: &[f64]) -> PyValue {
    v.iter()
        .copied()
        .reduce(|a, b| if b > a { b } else { a })
        .map_or(PyValue::None, PyValue::Float)
}
fn int(n: usize) -> PyValue {
    PyValue::int(n as i64)
}

fn read_lines(path: &Path) -> Option<Vec<String>> {
    let name = path.to_string_lossy();
    let raw = if name.ends_with(".zst") {
        let o = std::process::Command::new("zstd")
            .arg("-dc")
            .arg(path)
            .stdin(std::process::Stdio::null())
            .output()
            .ok()?;
        o.stdout
    } else if name.ends_with(".gz") {
        crate::gz::decompress(&std::fs::read(path).ok()?).ok()?
    } else {
        std::fs::read(path).ok()?
    };
    let text = py::decode_replace(&raw);
    let text = if name.ends_with(".zst") {
        text
    } else {
        text.replace("\r\n", "\n").replace('\r', "\n")
    };
    Some(py::splitlines(&text))
}

/// Raw CSV -> daily envelope statistics (compressed copies are read too).
pub fn build_rollup(csv_path: &Path, day: &str) -> Option<PyValue> {
    let lines = read_lines(csv_path)?;
    if lines.is_empty() {
        return None;
    }
    let header: Vec<&str> = lines[0].split(',').collect();
    let mut idx: Vec<(&str, usize)> = vec![];
    for (i, k) in header.iter().enumerate() {
        match idx.iter_mut().find(|(n, _)| n == k) {
            Some(e) => e.1 = i,
            None => idx.push((k, i)),
        }
    }
    let at = |k: &str| idx.iter().find(|(n, _)| *n == k).map(|(_, i)| *i);
    let need = ["mem_avail", "mem_rate", "gpu_temp", "gpu_pwr"];
    if !need.iter().all(|k| at(k).is_some()) {
        return None;
    }
    let (im, ir, it, iw) = (
        at("mem_avail").unwrap(),
        at("mem_rate").unwrap(),
        at("gpu_temp").unwrap(),
        at("gpu_pwr").unwrap(),
    );
    let isoc = at("soc_temp");
    let (mut mem, mut falling, mut temp, mut pwr, mut soc) =
        (vec![], vec![], vec![], vec![], vec![]);
    let mut n = 0usize;
    for line in &lines[1..] {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < header.len() {
            continue;
        }
        n += 1;
        if let Some(si) = isoc {
            if si < parts.len() {
                if let Some(x) = num(parts[si]) {
                    soc.push(x);
                }
            }
        }
        if let Some(x) = num(parts[im]) {
            mem.push(x);
        }
        if let Some(x) = num(parts[ir]) {
            if x > 0.0 {
                falling.push(x);
            }
        }
        if let Some(x) = num(parts[it]) {
            temp.push(x);
        }
        if let Some(x) = num(parts[iw]) {
            pwr.push(x);
        }
    }
    let d = |pairs: Vec<(&str, PyValue)>| py::dict(pairs);
    Some(d(vec![
        ("schema_version", PyValue::int(SCHEMA_VERSION)),
        ("day", py::s(day)),
        ("samples", int(n)),
        (
            "mem_avail_mib",
            d(vec![
                ("min", fmin(&mem)),
                ("p01", pct(&mem, 0.01)),
                ("p50", pct(&mem, 0.50)),
                ("max", fmax(&mem)),
            ]),
        ),
        (
            "descent_rate_mib_s",
            d(vec![
                ("falling_samples", int(falling.len())),
                ("p50", pct(&falling, 0.50)),
                ("p90", pct(&falling, 0.90)),
                ("p99", pct(&falling, 0.99)),
                ("max", fmax(&falling)),
            ]),
        ),
        (
            "gpu_temp_c",
            d(vec![("p50", pct(&temp, 0.50)), ("max", fmax(&temp))]),
        ),
        (
            "gpu_power_w",
            d(vec![("p50", pct(&pwr, 0.50)), ("max", fmax(&pwr))]),
        ),
        (
            "soc_temp_c",
            d(vec![
                ("samples", int(soc.len())),
                ("p50", pct(&soc, 0.50)),
                ("p95", pct(&soc, 0.95)),
                ("p99", pct(&soc, 0.99)),
                ("max", fmax(&soc)),
                ("column_present", PyValue::Bool(isoc.is_some())),
            ]),
        ),
    ]))
}

/// `sort_keys=True` applied to every mapping.
pub fn sort_keys(v: &PyValue) -> PyValue {
    match v.peel() {
        PyValue::Dict(d) => {
            let mut items: Vec<(PyStr, PyValue)> =
                d.iter().map(|(k, x)| (k.clone(), sort_keys(x))).collect();
            items.sort_by(|a, b| a.0.code_points().cmp(b.0.code_points()));
            let mut o = PyDict::new();
            for (k, x) in items {
                o.insert(k, x);
            }
            PyValue::Dict(o)
        }
        PyValue::List(l) => PyValue::List(l.iter().map(sort_keys).collect()),
        other => other.clone(),
    }
}

fn io(e: std::io::Error, p: &Path) -> String {
    crate::pathfmt::os_error(&e, &p.display().to_string())
}

pub fn append_event(
    node_dir: &Path,
    now: &DateTime,
    kind: &str,
    extra: Vec<(&str, PyValue)>,
) -> Result<(), String> {
    let ev = node_dir.join("events");
    std::fs::create_dir_all(&ev).map_err(|e| io(e, &ev))?;
    let mut rec = vec![
        ("ts", py::s(&now.iso_z())),
        ("kind", py::s(kind)),
        ("source", py::s("logs_lifecycle")),
    ];
    rec.extend(extra);
    let line = py::dumps_unicode(&sort_keys(&py::dict(rec)));
    let p = ev.join(format!("{}-{:02}.jsonl", now.year, now.month));
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&p)
        .map_err(|e| io(e, &p))?;
    f.write_all(format!("{line}\n").as_bytes())
        .map_err(|e| io(e, &p))
}

/// (new_path, method): zstd when it is on PATH, otherwise gzip.
pub fn compress_file(path: &Path) -> Result<(PathBuf, &'static str), String> {
    let base = path.to_string_lossy().into_owned();
    if crate::proc::which("zstd").is_some() {
        let new = PathBuf::from(format!("{base}.zst"));
        let argv = [
            "zstd".to_string(),
            "-q".into(),
            "-19".into(),
            "--rm".into(),
            base.clone(),
            "-o".into(),
            new.to_string_lossy().into_owned(),
        ];
        let ok = std::process::Command::new(&argv[0])
            .args(&argv[1..])
            .status()
            .is_ok_and(|s| s.success());
        if ok {
            return Ok((new, "zstd"));
        }
        if new.exists() {
            std::fs::remove_file(&new).map_err(|e| io(e, &new))?;
        }
    }
    let new = PathBuf::from(format!("{base}.gz"));
    let data = std::fs::read(path).map_err(|e| io(e, path))?;
    let fname = new
        .file_name()
        .map(|n| n.to_string_lossy().trim_end_matches(".gz").to_string())
        .unwrap_or_default();
    let mtime = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as u32);
    std::fs::write(&new, crate::gz::compress(&data, &fname, mtime)).map_err(|e| io(e, &new))?;
    std::fs::remove_file(path).map_err(|e| io(e, path))?;
    Ok((new, "gzip"))
}

#[derive(Debug, Default)]
pub struct Actions {
    pub rolled_up: Vec<String>,
    /// (day, age_days)
    pub compressed: Vec<(String, i64)>,
    /// (day, age_days, bytes)
    pub evicted_age: Vec<(String, i64, u64)>,
    /// (day, bytes)
    pub evicted_capacity: Vec<(String, u64)>,
    pub total_bytes_after: u64,
    pub cap_bytes: i128,
    pub over_cap: bool,
}

fn age_days(today: i64, day: &str) -> Option<i64> {
    strptime(day, "%Y-%m-%d").map(|d| today - d.ordinal_days())
}

pub struct Limits {
    pub raw_days: i64,
    pub compressed_days: i64,
    pub cap_mib: i64,
}

pub fn enforce(
    node_dir: &Path,
    now: &DateTime,
    lim: &Limits,
    apply: bool,
) -> Result<Actions, String> {
    let samples = node_dir.join("samples");
    let rollup = node_dir.join("rollup");
    let mut a = Actions::default();
    let today_iso = now.date_iso();
    let today = now.ordinal_days();
    if apply {
        std::fs::create_dir_all(&rollup).map_err(|e| io(e, &rollup))?;
    }
    for (day, path, _) in list_sample_files(&samples) {
        let target = rollup.join(format!("{day}.json"));
        if target.exists() || day == today_iso {
            continue;
        }
        let Some(stats) = build_rollup(&path, &day) else {
            continue;
        };
        a.rolled_up.push(day.clone());
        if apply {
            let text = py::dumps_pretty(&sort_keys(&stats));
            std::fs::write(&target, format!("{text}\n")).map_err(|e| io(e, &target))?;
        }
    }
    for (day, path, comp) in list_sample_files(&samples) {
        if comp {
            continue;
        }
        let Some(age) = age_days(today, &day) else {
            continue;
        };
        if age > lim.raw_days {
            a.compressed.push((day.clone(), age));
            if apply {
                let (_, method) = compress_file(&path)?;
                append_event(
                    node_dir,
                    now,
                    "log_compressed",
                    vec![
                        ("day", py::s(&day)),
                        ("age_days", PyValue::int(age)),
                        ("method", py::s(method)),
                    ],
                )?;
            }
        }
    }
    for (day, path, comp) in list_sample_files(&samples) {
        if !comp {
            continue;
        }
        let Some(age) = age_days(today, &day) else {
            continue;
        };
        if age > lim.compressed_days {
            let size = if path.exists() {
                std::fs::metadata(&path)
                    .map(|m| m.len())
                    .map_err(|e| io(e, &path))?
            } else {
                0
            };
            a.evicted_age.push((day.clone(), age, size));
            if apply {
                std::fs::remove_file(&path).map_err(|e| io(e, &path))?;
                append_event(
                    node_dir,
                    now,
                    "log_evicted",
                    vec![
                        ("day", py::s(&day)),
                        ("reason", py::s("age")),
                        ("age_days", PyValue::int(age)),
                        ("bytes", PyValue::int(size)),
                    ],
                )?;
            }
        }
    }
    let cap_bytes = lim.cap_mib as i128 * 1024 * 1024;
    let mut total = dir_size_bytes(node_dir) as i128;
    if total > cap_bytes {
        for (day, path, _) in list_sample_files(&samples) {
            if total <= cap_bytes {
                break;
            }
            if day == today_iso {
                continue;
            }
            let size = if path.exists() {
                std::fs::metadata(&path)
                    .map(|m| m.len())
                    .map_err(|e| io(e, &path))?
            } else {
                0
            };
            a.evicted_capacity.push((day.clone(), size));
            total -= size as i128;
            if apply {
                std::fs::remove_file(&path).map_err(|e| io(e, &path))?;
                append_event(
                    node_dir,
                    now,
                    "log_evicted",
                    vec![
                        ("day", py::s(&day)),
                        ("reason", py::s("capacity")),
                        ("bytes", PyValue::int(size)),
                        ("cap_mib", PyValue::int(lim.cap_mib)),
                    ],
                )?;
            }
        }
    }
    a.total_bytes_after = dir_size_bytes(node_dir);
    a.cap_bytes = cap_bytes;
    a.over_cap = (a.total_bytes_after as i128) > cap_bytes;
    Ok(a)
}

fn mkday(sdir: &Path, day: &str, rows: usize, mem0: i64, soc: Option<&[i64]>) -> PathBuf {
    std::fs::create_dir_all(sdir).expect("fixture dir");
    let p = sdir.join(format!("{day}.csv"));
    let mut t = String::new();
    match soc {
        None => {
            t.push_str(
                "ts,mem_avail,mem_rate,gpu_temp,gpu_pwr,gpu_sm,gpu_util,gpu_mem,load1,ctr_n\n",
            );
            for i in 0..rows {
                t.push_str(&format!(
                    "{},{},{},49,4.8,208,0,,0.3,1\n",
                    1785 + i,
                    mem0 - i as i64 * 100,
                    100
                ));
            }
        }
        Some(s) => {
            t.push_str("ts,mem_avail,mem_rate,gpu_temp,gpu_pwr,gpu_sm,gpu_util,gpu_mem,load1,ctr_n,soc_temp,soc_zone\n");
            for i in 0..rows {
                t.push_str(&format!(
                    "{},{},{},49,4.8,208,0,,0.3,1,{},5\n",
                    1785 + i,
                    mem0 - i as i64 * 100,
                    100,
                    s[i % s.len()]
                ));
            }
        }
    }
    std::fs::write(&p, t).expect("fixture file");
    p
}

fn temp_dir() -> PathBuf {
    let base = std::env::temp_dir();
    for k in 0..1000u32 {
        let p = base.join(format!("tmp-logs-lifecycle-{}-{k}", std::process::id()));
        if std::fs::create_dir(&p).is_ok() {
            return p;
        }
    }
    panic!("no temporary directory");
}

fn self_test() -> i32 {
    let mut checks: Vec<(String, bool)> = vec![];
    let mut ck = |n: &str, c: bool| checks.push((n.to_string(), c));
    let now = parse_now("2026-07-31T00:00:00Z").expect("fixed time");
    let td = temp_dir();
    let lim = |cap: i64| Limits {
        raw_days: DEFAULT_RAW_DAYS,
        compressed_days: DEFAULT_COMPRESSED_DAYS,
        cap_mib: cap,
    };
    let mut run = || -> Result<(), String> {
        let nd = td.join("spark-x");
        let sd = nd.join("samples");
        for d in ["2026-07-30", "2026-07-20", "2026-06-01", "2026-07-31"] {
            mkday(&sd, d, 5, 117000, None);
        }
        let plan = enforce(&nd, &now, &lim(DEFAULT_CAP_MIB), false)?;
        ck(
            "dry-run 은 파일을 안 바꾼다",
            sd.join("2026-07-20.csv").exists(),
        );
        ck(
            "오늘은 rollup 유보",
            !plan.rolled_up.iter().any(|d| d == "2026-07-31"),
        );
        ck(
            "과거일은 rollup 대상",
            plan.rolled_up.iter().any(|d| d == "2026-07-30"),
        );
        ck(
            "7일 초과는 압축 대상",
            plan.compressed.iter().any(|c| c.0 == "2026-07-20"),
        );
        ck(
            "1일 전은 압축 아님",
            !plan.compressed.iter().any(|c| c.0 == "2026-07-30"),
        );
        let sd2 = td.join("spark-soc").join("samples");
        mkday(
            &sd2,
            "2026-07-30",
            10,
            117000,
            Some(&[46, 47, 91, 92, 93, 47, 46, 46, 47, 95]),
        );
        let r_soc =
            build_rollup(&sd2.join("2026-07-30.csv"), "2026-07-30").unwrap_or(PyValue::None);
        let sc = py::get(&r_soc, "soc_temp_c")
            .cloned()
            .unwrap_or(PyValue::None);
        ck(
            "SoC 열 rollup: 열 존재 표기",
            matches!(py::get(&sc, "column_present"), Some(PyValue::Bool(true))),
        );
        ck(
            "SoC 열 rollup: 최대값",
            matches!(py::get(&sc, "max"), Some(PyValue::Float(x)) if *x == 95.0),
        );
        ck(
            "SoC 열 rollup: 표본수",
            py::get(&sc, "samples") == Some(&PyValue::int(10)),
        );
        ck(
            "SoC 열 rollup: 꼬리 분위(교정 입력)",
            !matches!(py::get(&sc, "p95"), Some(PyValue::None))
                && !matches!(py::get(&sc, "p99"), Some(PyValue::None)),
        );
        let r_old = build_rollup(&sd.join("2026-07-30.csv"), "2026-07-30").unwrap_or(PyValue::None);
        let so = py::get(&r_old, "soc_temp_c")
            .cloned()
            .unwrap_or(PyValue::None);
        ck(
            "옛 스키마: SoC 열 부재를 정직 표기",
            matches!(py::get(&so, "column_present"), Some(PyValue::Bool(false)))
                && matches!(py::get(&so, "max"), Some(PyValue::None))
                && py::get(&so, "samples") == Some(&PyValue::int(0)),
        );
        let gp = py::get(&r_old, "gpu_power_w")
            .cloned()
            .unwrap_or(PyValue::None);
        let mm = py::get(&r_old, "mem_avail_mib")
            .cloned()
            .unwrap_or(PyValue::None);
        ck(
            "옛 스키마: SoC 부재가 기존 통계를 버리지 않는다(회귀 고정)",
            !matches!(py::get(&gp, "max"), Some(PyValue::None))
                && !matches!(py::get(&mm, "min"), Some(PyValue::None)),
        );
        enforce(&nd, &now, &lim(DEFAULT_CAP_MIB), true)?;
        ck(
            "rollup 파일 생성",
            nd.join("rollup").join("2026-07-30.json").exists(),
        );
        let r = agent_runtime_support::pyjson::loads(
            &std::fs::read_to_string(nd.join("rollup").join("2026-07-30.json"))
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let dr = py::get(&r, "descent_rate_mib_s")
            .cloned()
            .unwrap_or(PyValue::None);
        ck(
            "rollup 통계 내용",
            py::get(&r, "samples") == Some(&PyValue::int(5))
                && matches!(py::get(&dr, "max"), Some(PyValue::Float(x)) if *x == 100.0),
        );
        ck(
            "rollup 이 압축보다 먼저(원시 잃어도 통계 남음)",
            nd.join("rollup").join("2026-07-20.json").exists(),
        );
        let left: Vec<String> = list_sample_files(&sd)
            .into_iter()
            .map(|(_, p, _)| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        ck(
            "압축 수행",
            left.iter().any(|n| n.starts_with("2026-07-20.csv.")),
        );
        ck(
            "60일 전은 삭제(같은 실행에서 압축→나이삭제)",
            !left.iter().any(|n| n.starts_with("2026-06-01")),
        );
        let ev = std::fs::read_to_string(nd.join("events").join("2026-07.jsonl"))
            .map_err(|e| e.to_string())?;
        ck("압축이 events 에 기록", ev.contains("\"log_compressed\""));
        ck(
            "삭제가 events 에 기록(침묵 삭제 금지)",
            ev.contains("\"log_evicted\""),
        );
        let res2 = enforce(&nd, &now, &lim(DEFAULT_CAP_MIB), true)?;
        ck("멱등: 재실행 시 압축 0", res2.compressed.is_empty());
        ck("멱등: 재실행 시 rollup 0", res2.rolled_up.is_empty());
        let nd2 = td.join("spark-y");
        let sd2 = nd2.join("samples");
        for d in ["2026-07-28", "2026-07-29", "2026-07-30", "2026-07-31"] {
            mkday(&sd2, d, 8000, 117000, None);
        }
        ck(
            "용량 fixture 가 상한을 초과",
            dir_size_bytes(&nd2) > 1024 * 1024,
        );
        let res3 = enforce(&nd2, &now, &lim(1), true)?;
        ck("용량 초과 시 삭제 발생", !res3.evicted_capacity.is_empty());
        ck(
            "가장 오래된 것부터 삭제",
            res3.evicted_capacity
                .first()
                .is_some_and(|x| x.0 == "2026-07-28"),
        );
        ck("오늘 것은 보존", sd2.join("2026-07-31.csv").exists());
        ck("상한 이하로 수렴", !res3.over_cap);
        let ev2 = std::fs::read_to_string(nd2.join("events").join("2026-07.jsonl"))
            .map_err(|e| e.to_string())?;
        ck("용량 삭제도 events 기록", ev2.contains("\"capacity\""));
        ck(
            "events 는 삭제되지 않음",
            nd2.join("events").join("2026-07.jsonl").exists(),
        );
        ck("rollup 은 삭제되지 않음", nd2.join("rollup").is_dir());
        let stray = sd2.join("README.txt");
        std::fs::write(&stray, "x").map_err(|e| e.to_string())?;
        enforce(&nd2, &now, &lim(1), true)?;
        ck("규칙 밖 파일 불가침", stray.exists());
        Ok(())
    };
    let res = run();
    let _ = std::fs::remove_dir_all(&td);
    if let Err(m) = res {
        return crate::raise::exit(&crate::raise::Raised(m));
    }
    let mut ok = true;
    for (name, passed) in &checks {
        println!("  [{}] {}", if *passed { "PASS" } else { "FAIL" }, name);
        ok = ok && *passed;
    }
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

pub static SPEC: Spec = Spec {
    prog: "logs_lifecycle.py",
    usage: include_str!("../help/logs_lifecycle.usage"),
    help: include_str!("../help/logs_lifecycle.help"),
    args: &[
        HELP,
        store(&["--node-dir"]),
        store(&["--now"]),
        store(&["--raw-days"]).int(),
        store(&["--compressed-days"]).int(),
        store(&["--cap-mib"]).int(),
        flag(&["--apply"]),
        flag(&["--self-test"]),
    ],
    subs: None,
};

pub fn main(args: &[String]) -> i32 {
    let p = match argp::parse(&SPEC, args) {
        Outcome::Ok(p) => p,
        Outcome::Exit(c) => return c,
    };
    if p.flag(7) {
        return self_test();
    }
    let node_dir = p.text(1).unwrap_or("");
    let now = p.text(2).unwrap_or("");
    let missing: Vec<&str> = [("--node-dir", node_dir), ("--now", now)]
        .iter()
        .filter(|(_, v)| v.is_empty())
        .map(|(n, _)| *n)
        .collect();
    if !missing.is_empty() {
        return argp::error(&SPEC, &format!("required: {}", missing.join(", ")));
    }
    let now = match parse_now(now) {
        Ok(n) => n,
        Err(m) => {
            eprintln!("{m}");
            return 1;
        }
    };
    let lim = Limits {
        raw_days: p.int(3).unwrap_or(DEFAULT_RAW_DAYS),
        compressed_days: p.int(4).unwrap_or(DEFAULT_COMPRESSED_DAYS),
        cap_mib: p.int(5).unwrap_or(DEFAULT_CAP_MIB),
    };
    let apply = p.flag(6);
    let res = match enforce(Path::new(node_dir), &now, &lim, apply) {
        Ok(r) => r,
        Err(m) => return crate::raise::exit(&crate::raise::Raised(m)),
    };
    println!(
        "[lifecycle] {} node={}",
        if apply { "APPLY" } else { "DRY-RUN" },
        node_dir
    );
    println!(
        "[lifecycle] rollup {} · 압축 {} · 나이삭제 {} · 용량삭제 {}",
        res.rolled_up.len(),
        res.compressed.len(),
        res.evicted_age.len(),
        res.evicted_capacity.len()
    );
    let mib = |b: f64| b / 1048576.0;
    println!(
        "[lifecycle] 총량 {} MiB / 상한 {} MiB{}",
        agent_runtime_support::pynumber::format_fixed(mib(res.total_bytes_after as f64), 1),
        agent_runtime_support::pynumber::format_fixed(mib(res.cap_bytes as f64), 0),
        if res.over_cap {
            "  ★상한 초과 지속"
        } else {
            ""
        }
    );
    0
}
